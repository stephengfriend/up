//! Pinned CI tool resolution (SUPPLY-01, D1-05).
//!
//! `versions.yml` pins every binary CI runs. This module is the only path that
//! installs one: validate the pin, download over https with curl's own config
//! ignored, compare the SHA-256 **before** extracting, extract into a fresh
//! directory, and only then hand back the path. A tool with no pin for the host
//! target is refused; nothing falls back to an unpinned or `latest` download.
//!
//! The file format is shared with the root toolchain (Phase 7), instance
//! overrides (CHG-04) and `up versions` (M8), so it also accepts `requirements`
//! and `minimum` entries.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::check::{CheckResult, Ctx, Failure};

/// Only schema version this code understands.
const SCHEMA_VERSION: u32 = 1;
/// Consumers a tool may be listed for.
const USES: [&str; 3] = ["ci", "toolchain", "build"];
/// Every CI download must come from here (the only host Phase 1 pins use).
const URL_PREFIX: &str = "https://github.com/";
/// Hard cap on a downloaded archive (curl `--max-filesize`).
const MAX_DOWNLOAD_BYTES: &str = "536870912";
/// Seconds curl may spend connecting before it gives up.
const CONNECT_TIMEOUT_SECS: &str = "20";
/// Seconds one whole download may take; bounds a slow-drip response under the size cap.
const MAX_TIME_SECS: &str = "600";

/// The whole `versions.yml` file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionsFile {
    pub schema_version: u32,
    pub tools: BTreeMap<String, ToolPin>,
}

/// One tool: a download (`assets`), a hash-locked requirements file, or a system minimum.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPin {
    pub version: Option<String>,
    #[serde(rename = "use")]
    pub uses: Vec<String>,
    pub assets: Option<BTreeMap<Target, Asset>>,
    pub requirements: Option<String>,
    pub minimum: Option<String>,
}

/// One downloadable archive for one target.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
    /// Path of the executable inside the archive.
    pub bin: String,
}

/// The `up` release targets; they name the host platform an asset runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Target {
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64AppleDarwin,
    #[serde(rename = "aarch64-unknown-linux-musl")]
    Aarch64LinuxMusl,
    #[serde(rename = "x86_64-unknown-linux-musl")]
    X8664LinuxMusl,
}

impl Target {
    pub const ALL: [Target; 3] = [
        Target::Aarch64AppleDarwin,
        Target::Aarch64LinuxMusl,
        Target::X8664LinuxMusl,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Target::Aarch64AppleDarwin => "aarch64-apple-darwin",
            Target::Aarch64LinuxMusl => "aarch64-unknown-linux-musl",
            Target::X8664LinuxMusl => "x86_64-unknown-linux-musl",
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The target this process runs on.
pub fn host_target() -> Result<Target, Failure> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok(Target::Aarch64AppleDarwin),
        ("linux", "aarch64") => Ok(Target::Aarch64LinuxMusl),
        ("linux", "x86_64") => Ok(Target::X8664LinuxMusl),
        (os, arch) => Err(Failure::one(format!(
            "SUPPLY-01: no supported target for host {os}/{arch}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Loading and validation
// ---------------------------------------------------------------------------

/// Strict-load and validate `versions.yml` text.
pub fn parse_versions(src: &str, origin: &str) -> Result<VersionsFile, Failure> {
    let file = up_config::yaml::from_str::<VersionsFile>(src, origin)
        .map_err(|e| Failure::one(format!("SUPPLY-01: {e}")))?;
    file.validate()?;
    Ok(file)
}

/// Strict-load and validate a `versions.yml` file.
pub fn load_versions(path: &Path) -> Result<VersionsFile, Failure> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| Failure::one(format!("SUPPLY-01: cannot read {}: {e}", path.display())))?;
    parse_versions(&src, "versions.yml")
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_tool_name(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

/// Versions end up in directory names, so they cannot carry path syntax.
fn is_version(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_'))
}

/// A relative path with no empty, `.` or `..` component and no control characters.
fn is_clean_relative(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('/')
        && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !s.contains('\\')
        && s.split('/').all(|c| !c.is_empty() && c != "." && c != "..")
}

fn is_github_url(s: &str) -> bool {
    s.starts_with(URL_PREFIX)
        && s.len() > URL_PREFIX.len()
        && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

impl VersionsFile {
    /// Every rule the installer relies on, checked before any download.
    pub fn validate(&self) -> Result<(), Failure> {
        let mut problems = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            problems.push(format!(
                "SUPPLY-01: versions.yml: schema_version must be {SCHEMA_VERSION}, got {}",
                self.schema_version
            ));
        }
        for (name, pin) in &self.tools {
            pin.validate(name, &mut problems);
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(Failure::many(problems))
        }
    }
}

impl ToolPin {
    fn validate(&self, name: &str, problems: &mut Vec<String>) {
        let mut bad =
            |detail: String| problems.push(format!("SUPPLY-01: versions.yml: {name}: {detail}"));
        if !is_tool_name(name) {
            bad("tool name must be lowercase letters, digits and hyphens".to_string());
        }
        let kinds = [
            self.assets.is_some(),
            self.requirements.is_some(),
            self.minimum.is_some(),
        ]
        .iter()
        .filter(|k| **k)
        .count();
        if kinds != 1 {
            bad("exactly one of assets, requirements and minimum is required".to_string());
        }
        match (&self.version, self.minimum.is_some()) {
            (Some(v), _) if !is_version(v) => {
                bad("version may only hold letters, digits and . + - _".to_string());
            }
            (None, false) => bad("version is required".to_string()),
            _ => {}
        }
        if self.uses.is_empty() {
            bad("use must list at least one of ci, toolchain, build".to_string());
        }
        for u in &self.uses {
            if !USES.contains(&u.as_str()) {
                bad(format!(
                    "use: unknown consumer (expected one of {})",
                    USES.join(", ")
                ));
            }
        }
        if let Some(req) = &self.requirements
            && !is_clean_relative(req)
        {
            bad("requirements must be a clean repo-relative path".to_string());
        }
        if let Some(min) = &self.minimum
            && min.is_empty()
        {
            bad("minimum must not be empty".to_string());
        }
        if let Some(assets) = &self.assets {
            if assets.is_empty() {
                bad("assets must pin at least one target".to_string());
            }
            for (target, asset) in assets {
                if !is_github_url(&asset.url) {
                    bad(format!("{target}: url must be an {URL_PREFIX} https URL"));
                }
                if !is_hex64(&asset.sha256) {
                    bad(format!(
                        "{target}: sha256 must be 64 lowercase hex characters"
                    ));
                }
                if !is_clean_relative(&asset.bin) {
                    bad(format!(
                        "{target}: bin must be a relative path with no empty, . or .. component"
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Download, verify, extract
// ---------------------------------------------------------------------------

/// Downloads one URL to one file. Production uses [`CurlFetcher`]; tests use fakes.
pub trait Fetcher {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), Failure>;
}

/// https-only curl with its config files ignored (`-q` must stay first).
pub struct CurlFetcher;

/// The curl argument vector for one download (everything after the program name).
pub fn curl_args(url: &str, dest: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = [
        "-q",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--tlsv1.2",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--max-redirs",
        "5",
        "--max-filesize",
        MAX_DOWNLOAD_BYTES,
        "--connect-timeout",
        CONNECT_TIMEOUT_SECS,
        "--max-time",
        MAX_TIME_SECS,
        "--output",
    ]
    .iter()
    .map(std::ffi::OsString::from)
    .collect();
    args.push(dest.into());
    args.push(url.into());
    args
}

impl Fetcher for CurlFetcher {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), Failure> {
        let out = Command::new("curl")
            .args(curl_args(url, dest))
            .output()
            .map_err(|e| Failure::one(format!("SUPPLY-01: cannot run curl: {e}")))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(Failure::one(format!(
                "SUPPLY-01: download failed for {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }
}

/// Lowercase hex SHA-256 of a file, streamed.
pub fn sha256_file(path: &Path) -> Result<String, Failure> {
    let fail =
        |e: std::io::Error| Failure::one(format!("SUPPLY-01: cannot hash {}: {e}", path.display()));
    let mut file = std::fs::File::open(path).map_err(fail)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(fail)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut hex = String::with_capacity(64);
    for b in hasher.finalize().iter() {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

fn io_fail(what: &str, path: &Path, e: std::io::Error) -> Failure {
    Failure::one(format!("SUPPLY-01: {what} {}: {e}", path.display()))
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Install `name` for `target` under `<root>/target/up-dev-tools/` and return the absolute
/// path of its executable. The archive is hashed before it is extracted, every time.
pub fn ensure_tool_with(
    root: &Path,
    versions: &VersionsFile,
    name: &str,
    target: Target,
    fetcher: &dyn Fetcher,
) -> Result<PathBuf, Failure> {
    versions.validate()?;
    let pin = versions
        .tools
        .get(name)
        .ok_or_else(|| Failure::one(format!("SUPPLY-01: unknown tool {name}")))?;
    let assets = pin
        .assets
        .as_ref()
        .ok_or_else(|| Failure::one(format!("SUPPLY-01: {name} is not a download tool")))?;
    let asset = assets
        .get(&target)
        .ok_or_else(|| Failure::one(format!("SUPPLY-01: no pin for {name} on {target}")))?;
    // validate() guarantees a version for a download tool.
    let version = pin.version.as_deref().unwrap_or_default();

    let root = std::path::absolute(root).map_err(|e| io_fail("cannot resolve", root, e))?;
    let base = root.join("target/up-dev-tools");
    let cache = base.join("cache");
    std::fs::create_dir_all(&cache).map_err(|e| io_fail("cannot create", &cache, e))?;

    let archive = fetch_verified(&cache, name, target, asset, fetcher)?;

    let final_dir = base.join(format!("{name}-{version}-{}", &asset.sha256[..12]));
    // A directory that carries the completion marker for this archive hash was extracted
    // from a verified archive and checked; keep it. Venvs and running tools point into it, so
    // it is never deleted and recreated while it is valid.
    if install_is_valid(&final_dir, asset) {
        return Ok(final_dir.join(&asset.bin));
    }

    // Extract only now, into a directory that did not exist before, and publish it with a
    // single rename so a reader never sees a half-written install.
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = base.join(format!("tmp-{}-{n}", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| io_fail("cannot clear", &tmp, e))?;
    }
    std::fs::create_dir(&tmp).map_err(|e| io_fail("cannot create", &tmp, e))?;
    let finished = extract_into(&archive, &tmp, name, target, asset)
        .and_then(|()| write_marker(&tmp, asset))
        .and_then(|()| publish(&tmp, &final_dir, n, asset));
    // After a successful publish `tmp` is gone; after a lost race or a failure it is leftover.
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    finished.map(|()| final_dir.join(&asset.bin))
}

/// File inside an install directory, written last, holding the archive hash.
const MARKER: &str = ".complete";

fn write_marker(dir: &Path, asset: &Asset) -> Result<(), Failure> {
    let marker = dir.join(MARKER);
    std::fs::write(&marker, format!("{}\n", asset.sha256))
        .map_err(|e| io_fail("cannot write", &marker, e))
}

/// An install directory is valid when its marker holds this archive's hash and the pinned
/// binary resolves to a regular file inside it.
fn install_is_valid(dir: &Path, asset: &Asset) -> bool {
    let marked = std::fs::read_to_string(dir.join(MARKER)).is_ok_and(|m| m.trim() == asset.sha256);
    if !marked {
        return false;
    }
    let (Ok(real_dir), Ok(real_bin)) = (dir.canonicalize(), dir.join(&asset.bin).canonicalize())
    else {
        return false;
    };
    real_bin.starts_with(&real_dir) && real_bin.is_file()
}

/// Move the finished `tmp` into place. An invalid directory already there is moved aside
/// (never deleted in place) so the rename can succeed. If another process published a valid
/// install first, that one wins and this call succeeds.
fn publish(tmp: &Path, final_dir: &Path, n: u64, asset: &Asset) -> Result<(), Failure> {
    for attempt in 0..3 {
        match std::fs::rename(tmp, final_dir) {
            Ok(()) => return Ok(()),
            Err(e) => {
                if install_is_valid(final_dir, asset) {
                    return Ok(());
                }
                if attempt == 2 || !final_dir.exists() {
                    return Err(io_fail("cannot install", final_dir, e));
                }
                let aside =
                    final_dir.with_file_name(format!("stale-{}-{n}-{attempt}", std::process::id()));
                // Losing this race is fine: the next rename decides.
                if std::fs::rename(final_dir, &aside).is_ok() {
                    let _ = std::fs::remove_dir_all(&aside);
                }
            }
        }
    }
    Err(io_fail(
        "cannot install",
        final_dir,
        std::io::Error::other("lost the race three times"),
    ))
}

/// Return the cached archive for `asset`, downloading it when absent or altered.
/// A file is only ever kept in the cache under its own verified hash.
fn fetch_verified(
    cache: &Path,
    name: &str,
    target: Target,
    asset: &Asset,
    fetcher: &dyn Fetcher,
) -> Result<PathBuf, Failure> {
    let cached = cache.join(&asset.sha256);
    if let Ok(meta) = std::fs::symlink_metadata(&cached) {
        if meta.is_file() && sha256_file(&cached)? == asset.sha256 {
            return Ok(cached);
        }
        // Altered, truncated or not a regular file: never use it.
        match std::fs::remove_file(&cached) {
            Ok(()) => {}
            // Another process removed it first.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_fail("cannot remove", &cached, e)),
        }
    }
    let part = cache.join(format!(
        "{}.{}-{}.part",
        asset.sha256,
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&part);
    if let Err(e) = fetcher.fetch(&asset.url, &part) {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let actual = match sha256_file(&part) {
        Ok(h) => h,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    if actual != asset.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(Failure::one(format!(
            "SUPPLY-01: {name} ({target}): sha256 mismatch for {}: expected {}, got {actual}",
            asset.url, asset.sha256
        )));
    }
    std::fs::rename(&part, &cached).map_err(|e| io_fail("cannot cache", &cached, e))?;
    Ok(cached)
}

/// `tar -xzf` into the fresh `dest`, then check `bin` resolves to a regular file inside it.
fn extract_into(
    archive: &Path,
    dest: &Path,
    name: &str,
    target: Target,
    asset: &Asset,
) -> Result<(), Failure> {
    let out = Command::new("tar")
        .env_remove("TAR_OPTIONS")
        .arg("-xzf")
        .arg(archive)
        .arg("--no-same-owner")
        .arg("-C")
        .arg(dest)
        .output()
        .map_err(|e| Failure::one(format!("SUPPLY-01: cannot run tar: {e}")))?;
    if !out.status.success() {
        return Err(Failure::one(format!(
            "SUPPLY-01: {name} ({target}): tar failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let bin = dest.join(&asset.bin);
    // An interpreter such as python3 may be a symlink inside the archive; it must still
    // resolve to a regular file that stays within the extraction directory.
    let real_dest = dest
        .canonicalize()
        .map_err(|e| io_fail("cannot resolve", dest, e))?;
    let real_bin = bin.canonicalize().map_err(|_| {
        Failure::one(format!(
            "SUPPLY-01: {name} ({target}): bin {} not found in archive",
            asset.bin
        ))
    })?;
    if !real_bin.starts_with(&real_dest) {
        return Err(Failure::one(format!(
            "SUPPLY-01: {name} ({target}): bin {} resolves outside the archive",
            asset.bin
        )));
    }
    let meta = std::fs::metadata(&real_bin).map_err(|e| io_fail("cannot stat", &real_bin, e))?;
    if !meta.is_file() {
        return Err(Failure::one(format!(
            "SUPPLY-01: {name} ({target}): bin {} is not a regular file",
            asset.bin
        )));
    }
    std::fs::set_permissions(&real_bin, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| io_fail("cannot chmod", &real_bin, e))?;
    Ok(())
}

/// Install a tool from the repo's real `versions.yml` for the host target.
pub fn ensure_tool(ctx: &Ctx, name: &str) -> Result<PathBuf, Failure> {
    let versions = load_versions(&ctx.root.join("versions.yml"))?;
    let target = host_target()?;
    ensure_tool_with(&ctx.root, &versions, name, target, &CurlFetcher)
}

/// `up-dev tool-path <tool>`: print the verified absolute path of a pinned tool.
pub fn run_tool_path(ctx: &Ctx, args: &[String]) -> CheckResult {
    let [name] = args else {
        return Err(Failure::one("usage: up-dev tool-path <tool>"));
    };
    let path = ensure_tool(ctx, name)?;
    println!("{}", path.display());
    Ok(())
}
