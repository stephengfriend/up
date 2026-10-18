//! SUPPLY-01 (CI fetch path): CI binaries are installed only after their SHA-256
//! matches the pin in versions.yml. Network access is replaced by fake fetchers;
//! the one that must never be called panics if it is.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use up_dev::check::Failure;
use up_dev::tools::{
    Asset, Fetcher, Target, ToolPin, VersionsFile, ensure_tool_with, load_versions, parse_versions,
    sha256_file,
};

const GOOD_URL: &str = "https://github.com/example/tool/releases/download/v1.0.0/tool.tar.gz";
const ZERO_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Serves fixed bytes and counts calls.
struct BytesFetcher {
    bytes: Vec<u8>,
    calls: Cell<usize>,
}

impl BytesFetcher {
    fn new(bytes: Vec<u8>) -> BytesFetcher {
        BytesFetcher {
            bytes,
            calls: Cell::new(0),
        }
    }
}

impl Fetcher for BytesFetcher {
    fn fetch(&self, _url: &str, dest: &Path) -> Result<(), Failure> {
        self.calls.set(self.calls.get() + 1);
        fs::write(dest, &self.bytes).map_err(|e| Failure::one(format!("fake fetch: {e}")))
    }
}

/// Fails the test if the installer reaches the network.
struct PanicFetcher;

impl Fetcher for PanicFetcher {
    fn fetch(&self, url: &str, _dest: &Path) -> Result<(), Failure> {
        panic!("fetcher must not be called, but was asked for {url}");
    }
}

fn one_tool_yaml(url: &str, sha: &str, bin: &str, extra_asset_line: &str) -> String {
    format!(
        "schema_version: 1\n\
         tools:\n\
         \x20 demo:\n\
         \x20   version: 1.0.0\n\
         \x20   use: [ci]\n\
         \x20   assets:\n\
         \x20     aarch64-apple-darwin:\n\
         \x20       url: {url}\n\
         \x20       sha256: \"{sha}\"\n\
         \x20       bin: {bin}\n\
         {extra_asset_line}"
    )
}

fn pin_for(sha: &str) -> VersionsFile {
    parse_versions(
        &one_tool_yaml(GOOD_URL, sha, "demo-1.0.0/demo", ""),
        "versions.yml",
    )
    .expect("a well-formed pin parses")
}

/// Build `demo-1.0.0/demo` (an executable script) into a tar.gz and return its bytes.
fn demo_archive(scratch: &Path) -> Vec<u8> {
    let src = scratch.join("src");
    fs::create_dir_all(src.join("demo-1.0.0")).unwrap();
    let bin = src.join("demo-1.0.0/demo");
    fs::write(&bin, "#!/bin/sh\necho demo\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    let archive = scratch.join("demo.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&src)
        .arg("demo-1.0.0")
        .status()
        .expect("tar runs");
    assert!(status.success(), "tar -czf failed");
    fs::read(&archive).unwrap()
}

fn sha_of(bytes: &[u8], scratch: &Path) -> String {
    let p = scratch.join("hash-input");
    fs::write(&p, bytes).unwrap();
    sha256_file(&p).expect("hashing works")
}

fn tools_dir(root: &Path) -> PathBuf {
    root.join("target/up-dev-tools")
}

fn msg(f: &Failure) -> String {
    f.messages.join("\n")
}

#[test]
fn test_supply_01_versions_yml_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../versions.yml");
    let versions = load_versions(&path).expect("the committed versions.yml loads");
    assert_eq!(versions.schema_version, 1);
    let mut downloads = 0;
    for (name, pin) in &versions.tools {
        if let Some(assets) = &pin.assets {
            downloads += 1;
            for target in Target::ALL {
                assert!(
                    assets.contains_key(&target),
                    "{name} has no pin for {}",
                    target.as_str()
                );
            }
            assert_eq!(assets.len(), 3, "{name} has keys beyond the three targets");
        }
    }
    assert!(downloads >= 1, "versions.yml pins no download tool");
}

#[test]
fn test_supply_01_ci_tool_hash_mismatch_rejected_before_extract() {
    let root = tempfile::tempdir().unwrap();
    let versions = pin_for(ZERO_SHA);
    let fetcher = BytesFetcher::new(b"definitely not the pinned bytes".to_vec());
    let err = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &fetcher,
    )
    .expect_err("a hash mismatch is refused");
    let text = msg(&err);
    assert!(text.contains("demo"), "names the tool: {text}");
    assert!(
        text.contains("aarch64-apple-darwin"),
        "names the target: {text}"
    );
    assert!(text.contains("sha256 mismatch"), "says why: {text}");
    assert!(text.contains(ZERO_SHA), "shows the pin: {text}");
    assert_eq!(fetcher.calls.get(), 1);

    let tools = tools_dir(root.path());
    assert!(
        !tools
            .join(format!("demo-1.0.0-{}", &ZERO_SHA[..12]))
            .exists(),
        "no extraction directory after a mismatch"
    );
    let leftovers: Vec<_> = fs::read_dir(&tools)
        .map(|rd| rd.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(
        leftovers.iter().all(|n| n == "cache"),
        "only the cache directory may exist: {leftovers:?}"
    );
    let cached: Vec<_> = fs::read_dir(tools.join("cache"))
        .map(|rd| rd.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(
        cached.is_empty(),
        "no archive kept in the cache: {cached:?}"
    );
}

#[test]
fn test_supply_01_ci_tool_verified_archive_extracts() {
    let scratch = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = demo_archive(scratch.path());
    let sha = sha_of(&bytes, scratch.path());
    let versions = pin_for(&sha);
    let fetcher = BytesFetcher::new(bytes);
    let bin = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &fetcher,
    )
    .expect("a verified archive installs");
    assert!(bin.is_absolute(), "{}", bin.display());
    assert!(bin.starts_with(tools_dir(root.path())), "{}", bin.display());
    let meta = fs::metadata(&bin).unwrap();
    assert!(meta.is_file());
    assert_ne!(meta.permissions().mode() & 0o111, 0, "executable");
    let out = Command::new(&bin)
        .output()
        .expect("the installed tool runs");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "demo");
    assert_eq!(fetcher.calls.get(), 1);
}

#[test]
fn test_supply_01_ci_tool_missing_target_pin_rejected() {
    let root = tempfile::tempdir().unwrap();
    let versions = pin_for(ZERO_SHA);
    let err = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::X8664LinuxMusl,
        &PanicFetcher,
    )
    .expect_err("no pin for the target");
    assert_eq!(
        msg(&err),
        "SUPPLY-01: no pin for demo on x86_64-unknown-linux-musl"
    );
}

#[test]
fn test_supply_01_ci_tool_unknown_tool_rejected() {
    let root = tempfile::tempdir().unwrap();
    let versions = pin_for(ZERO_SHA);
    let err = ensure_tool_with(
        root.path(),
        &versions,
        "nope",
        Target::Aarch64AppleDarwin,
        &PanicFetcher,
    )
    .expect_err("unknown tool");
    assert!(msg(&err).contains("nope"), "{}", msg(&err));
}

#[test]
fn test_supply_01_ci_tool_non_https_url_rejected() {
    let text = one_tool_yaml(
        "http://github.com/example/tool/releases/download/v1/tool.tar.gz",
        ZERO_SHA,
        "demo/demo",
        "",
    );
    let err = parse_versions(&text, "versions.yml").expect_err("http is refused");
    assert!(msg(&err).contains("https://github.com/"), "{}", msg(&err));

    // A hand-built file is refused too, before the fetcher is reached.
    let mut versions = pin_for(ZERO_SHA);
    asset_mut(&mut versions).url = "http://github.com/x/y.tar.gz".to_string();
    let root = tempfile::tempdir().unwrap();
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &PanicFetcher,
    )
    .expect_err("validated again before download");
}

#[test]
fn test_supply_01_ci_tool_foreign_host_rejected() {
    for url in [
        "https://example.com/tool.tar.gz",
        "https://github.com.example.org/tool.tar.gz",
        "https://github.com@example.org/tool.tar.gz",
        "https://raw.githubusercontent.com/tool.tar.gz",
        "https://github.com/tool.tar.gz with space",
    ] {
        let text = one_tool_yaml(url, ZERO_SHA, "demo/demo", "");
        let err = parse_versions(&text, "versions.yml")
            .expect_err(&format!("{url} must be refused at validation"));
        assert!(msg(&err).contains("demo"), "{}", msg(&err));

        let mut versions = pin_for(ZERO_SHA);
        asset_mut(&mut versions).url = url.to_string();
        let root = tempfile::tempdir().unwrap();
        ensure_tool_with(
            root.path(),
            &versions,
            "demo",
            Target::Aarch64AppleDarwin,
            &PanicFetcher,
        )
        .expect_err(&format!("{url} must be refused before download"));
    }
}

#[test]
fn test_supply_01_ci_tool_bin_path_traversal_rejected() {
    for bin in [
        "../x",
        "/usr/bin/x",
        "a/../x",
        "a//x",
        "./x",
        "a/./x",
        "a/",
        "\"\"",
    ] {
        let text = one_tool_yaml(GOOD_URL, ZERO_SHA, bin, "");
        let err =
            parse_versions(&text, "versions.yml").expect_err(&format!("bin {bin} must be refused"));
        assert!(msg(&err).contains("bin"), "{}", msg(&err));
    }
    let mut versions = pin_for(ZERO_SHA);
    asset_mut(&mut versions).bin = "../x".to_string();
    let root = tempfile::tempdir().unwrap();
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &PanicFetcher,
    )
    .expect_err("validated again before download");
}

#[test]
fn test_supply_01_ci_tool_bad_sha_format_rejected() {
    let sixty_three = &ZERO_SHA[..63];
    let upper = "ABCDEF0000000000000000000000000000000000000000000000000000000000";
    let non_hex = "g000000000000000000000000000000000000000000000000000000000000000";
    for sha in [sixty_three, upper, non_hex, ""] {
        let text = one_tool_yaml(GOOD_URL, sha, "demo/demo", "");
        let err = parse_versions(&text, "versions.yml")
            .expect_err(&format!("sha256 {sha:?} must be refused"));
        assert!(msg(&err).contains("sha256"), "{}", msg(&err));
    }
}

#[test]
fn test_supply_01_ci_tool_unknown_key_rejected() {
    let text = one_tool_yaml(
        GOOD_URL,
        ZERO_SHA,
        "demo/demo",
        "        mirror: https://example.com/x\n",
    );
    parse_versions(&text, "versions.yml").expect_err("an extra key in an asset is refused");

    let unknown_target = "schema_version: 1\ntools:\n  demo:\n    version: 1.0.0\n    use: [ci]\n    assets:\n      riscv64-unknown-linux-musl:\n        url: https://github.com/a/b/c.tar.gz\n        sha256: \"0000000000000000000000000000000000000000000000000000000000000000\"\n        bin: demo\n";
    parse_versions(unknown_target, "versions.yml").expect_err("an unknown target key is refused");
}

#[test]
fn test_supply_01_ci_tool_structure_rejected() {
    // Wrong schema_version, bad name, empty use, unknown use value, two kinds at once.
    let ok = one_tool_yaml(GOOD_URL, ZERO_SHA, "demo/demo", "");
    parse_versions(&ok, "versions.yml").expect("baseline parses");
    for (label, text) in [
        (
            "schema_version",
            ok.replace("schema_version: 1", "schema_version: 2"),
        ),
        ("tool name", ok.replace("  demo:", "  Demo_X:")),
        ("empty use", ok.replace("use: [ci]", "use: []")),
        ("unknown use", ok.replace("use: [ci]", "use: [prod]")),
        (
            "assets and minimum",
            ok.replace(
                "    version: 1.0.0\n",
                "    version: 1.0.0\n    minimum: \"1\"\n",
            ),
        ),
        ("missing version", ok.replace("    version: 1.0.0\n", "")),
    ] {
        assert!(
            parse_versions(&text, "versions.yml").is_err(),
            "{label} must be refused"
        );
    }
}

#[test]
fn test_supply_01_ci_tool_cached_archive_reverified() {
    let scratch = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = demo_archive(scratch.path());
    let sha = sha_of(&bytes, scratch.path());
    let versions = pin_for(&sha);

    let first = BytesFetcher::new(bytes.clone());
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &first,
    )
    .expect("first install");
    assert_eq!(first.calls.get(), 1);

    // An intact cache entry is reused: no second download.
    let offline = PanicFetcher;
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &offline,
    )
    .expect("intact cache is reused without fetching");

    // Alter the cached archive: it must not be extracted.
    let cached = tools_dir(root.path()).join("cache").join(&sha);
    assert!(cached.is_file(), "archive cached under its pinned hash");
    fs::write(&cached, b"tampered after caching").unwrap();

    // Offline: the altered bytes are refused, never used.
    let err = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &BytesFetcher::new(b"still wrong".to_vec()),
    )
    .expect_err("tampered cache plus bad download fails");
    assert!(msg(&err).contains("sha256 mismatch"), "{}", msg(&err));

    // With a good download available, the tampered entry is replaced and re-verified.
    fs::write(&cached, b"tampered again").unwrap();
    let again = BytesFetcher::new(bytes);
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &again,
    )
    .expect("re-fetched and re-verified");
    assert_eq!(again.calls.get(), 1);
    assert_eq!(sha256_file(&cached).unwrap(), sha);
}

fn asset_mut(versions: &mut VersionsFile) -> &mut Asset {
    let pin: &mut ToolPin = versions.tools.get_mut("demo").unwrap();
    let assets: &mut BTreeMap<Target, Asset> = pin.assets.as_mut().unwrap();
    assets.get_mut(&Target::Aarch64AppleDarwin).unwrap()
}

/// Pack `src/demo-1.0.0` (built by `build`, given that directory) into a tar.gz,
/// serve it, and install it with `bin` as the pinned binary path.
fn install_with_layout(
    bin: &str,
    build: impl FnOnce(&Path),
) -> (Result<PathBuf, Failure>, tempfile::TempDir) {
    let scratch = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let top = scratch.path().join("src/demo-1.0.0");
    fs::create_dir_all(&top).unwrap();
    build(&top);
    let archive = scratch.path().join("demo.tar.gz");
    // tar stores symlinks as links (no -h), so escape attempts reach extract_into intact.
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(scratch.path().join("src"))
        .arg("demo-1.0.0")
        .status()
        .expect("tar runs");
    assert!(status.success(), "tar -czf failed");
    let bytes = fs::read(&archive).unwrap();
    let sha = sha_of(&bytes, scratch.path());
    let versions = parse_versions(&one_tool_yaml(GOOD_URL, &sha, bin, ""), "versions.yml")
        .expect("a well-formed pin parses");
    let result = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &BytesFetcher::new(bytes),
    );
    (result, root)
}

#[test]
fn test_supply_01_extract_absolute_symlink_escape_refused() {
    let (result, _root) = install_with_layout("demo-1.0.0/demo", |top| {
        std::os::unix::fs::symlink("/bin/sh", top.join("demo")).unwrap();
    });
    let err = result.expect_err("an absolute symlink out of the archive is refused");
    assert!(
        msg(&err).contains("resolves outside the archive"),
        "{}",
        msg(&err)
    );
}

#[test]
fn test_supply_01_extract_directory_symlink_escape_refused() {
    let (result, _root) = install_with_layout("demo-1.0.0/d/true", |top| {
        std::os::unix::fs::symlink("/usr/bin", top.join("d")).unwrap();
    });
    let err = result.expect_err("bin through a directory symlink out of the archive is refused");
    assert!(
        msg(&err).contains("resolves outside the archive"),
        "{}",
        msg(&err)
    );
}

#[test]
fn test_supply_01_extract_directory_bin_refused() {
    let (result, _root) = install_with_layout("demo-1.0.0/demo", |top| {
        fs::create_dir(top.join("demo")).unwrap();
        fs::write(top.join("demo/file"), "x").unwrap();
    });
    let err = result.expect_err("a directory is not a binary");
    assert!(msg(&err).contains("not a regular file"), "{}", msg(&err));
}

#[test]
fn test_supply_01_extract_in_archive_symlink_to_regular_file_accepted() {
    let (result, root) = install_with_layout("demo-1.0.0/demo", |top| {
        fs::create_dir(top.join("real")).unwrap();
        let real = top.join("real/demo-real");
        fs::write(&real, "#!/bin/sh\necho demo\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o644)).unwrap();
        std::os::unix::fs::symlink("real/demo-real", top.join("demo")).unwrap();
    });
    let bin = result.expect("a symlink to a regular file inside the archive installs");
    assert!(bin.starts_with(tools_dir(root.path())), "{}", bin.display());
    let meta = fs::metadata(&bin).unwrap();
    assert!(meta.is_file());
    assert_ne!(meta.permissions().mode() & 0o111, 0, "made executable");
}

// ---- idempotent, race-safe installs (WR-04) ----

use std::sync::atomic::{AtomicUsize, Ordering};

/// A fetcher that can be shared between threads.
struct SyncBytesFetcher {
    bytes: Vec<u8>,
    calls: AtomicUsize,
}

impl Fetcher for SyncBytesFetcher {
    fn fetch(&self, _url: &str, dest: &Path) -> Result<(), Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        fs::write(dest, &self.bytes).map_err(|e| Failure::one(format!("fake fetch: {e}")))
    }
}

fn install_dir(root: &Path, sha: &str) -> PathBuf {
    tools_dir(root).join(format!("demo-1.0.0-{}", &sha[..12]))
}

#[test]
fn test_supply_01_install_is_idempotent_and_keeps_verified_dir() {
    let scratch = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = demo_archive(scratch.path());
    let sha = sha_of(&bytes, scratch.path());
    let versions = pin_for(&sha);
    let fetcher = BytesFetcher::new(bytes);

    let first = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &fetcher,
    )
    .expect("first install");
    let dir = install_dir(root.path(), &sha);
    assert_eq!(
        fs::read_to_string(dir.join(".complete")).unwrap().trim(),
        sha,
        "the install carries a completion marker holding the archive hash"
    );
    // Something that lives in the verified directory (as a venv symlink target would).
    fs::write(dir.join("sentinel"), "still here").unwrap();

    let second = ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &PanicFetcher,
    )
    .expect("second install reuses the verified directory");
    assert_eq!(first, second);
    assert!(
        dir.join("sentinel").exists(),
        "a verified install directory must not be deleted and recreated"
    );
}

#[test]
fn test_supply_01_install_without_valid_marker_is_replaced() {
    let scratch = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let bytes = demo_archive(scratch.path());
    let sha = sha_of(&bytes, scratch.path());
    let versions = pin_for(&sha);
    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &BytesFetcher::new(bytes.clone()),
    )
    .expect("first install");
    let dir = install_dir(root.path(), &sha);
    fs::write(dir.join(".complete"), "not the pinned hash\n").unwrap();
    fs::write(dir.join("sentinel"), "tamper").unwrap();

    ensure_tool_with(
        root.path(),
        &versions,
        "demo",
        Target::Aarch64AppleDarwin,
        &BytesFetcher::new(bytes),
    )
    .expect("an unverified directory is reinstalled from the verified archive");
    assert_eq!(
        fs::read_to_string(dir.join(".complete")).unwrap().trim(),
        sha
    );
    assert!(
        !dir.join("sentinel").exists(),
        "reinstalled from the archive"
    );
}

#[test]
fn test_supply_01_concurrent_installs_all_succeed() {
    let scratch = tempfile::tempdir().unwrap();
    let bytes = demo_archive(scratch.path());
    let sha = sha_of(&bytes, scratch.path());
    let versions = pin_for(&sha);
    let fetcher = SyncBytesFetcher {
        bytes,
        calls: AtomicUsize::new(0),
    };
    for _round in 0..5 {
        let root = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        ensure_tool_with(
                            root.path(),
                            &versions,
                            "demo",
                            Target::Aarch64AppleDarwin,
                            &fetcher,
                        )
                    })
                })
                .collect();
            for h in handles {
                let bin = h.join().unwrap().expect("every racer installs or reuses");
                let out = Command::new(&bin).output().expect("tool runs");
                assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "demo");
            }
        });
        let leftovers: Vec<_> = fs::read_dir(tools_dir(root.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp dirs left behind: {leftovers:?}");
    }
}
