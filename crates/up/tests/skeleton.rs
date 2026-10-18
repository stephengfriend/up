//! PUB-01: the shipped `up` binary skeleton builds and answers `--version`
//! and `--help` (M0). It carries no dependencies and no dev-check code (D1-01).

use std::process::Command;

fn up(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_up"))
        .args(args)
        .output()
        .expect("run the up binary")
}

#[test]
fn test_pub_01_up_binary_skeleton_runs() {
    let version = up(&["--version"]);
    assert!(version.status.success(), "--version must exit 0");
    assert_eq!(String::from_utf8_lossy(&version.stdout), "up 0.0.0\n");

    let help = up(&["--help"]);
    assert!(help.status.success(), "--help must exit 0");
    assert!(!help.stdout.is_empty(), "--help must print usage");

    let unknown = up(&["no-such-command"]);
    assert_eq!(unknown.status.code(), Some(2), "unknown input exits 2");
    assert!(!unknown.stderr.is_empty(), "usage goes to stderr");
}

// ---- PUB-01 / D1-01: the root binary stays dependency-free ----
//
// `crates/up/Cargo.toml` and `Cargo.lock` are read with a small std-only scanner:
// the `up` crate has no dependencies, and no TOML crate is approved for the
// workspace, so this guard cannot lean on one. The scanner errs towards
// refusing: anything that looks like a dependency declaration is reported.

fn strip_quotes(segment: &str) -> &str {
    segment.trim().trim_matches(|c| c == '"' || c == '\'')
}

/// Dotted-key segments, splitting only outside single or double quotes.
fn segments(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in path.chars() {
        match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                current.push(c);
            }
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (None, '.') => out.push(std::mem::take(&mut current)),
            (_, c) => current.push(c),
        }
    }
    out.push(current);
    out.iter().map(|s| strip_quotes(s).to_string()).collect()
}

fn is_runtime_deps_name(segment: &str) -> bool {
    segment == "dependencies" || segment == "build-dependencies"
}

/// Every dependency declaration for a normal or build dependency in a `Cargo.toml`.
/// `dev-dependencies` do not ship in the binary and are not reported.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_deps_table = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_start_matches('[');
            let header = header.split(']').next().unwrap_or("");
            let parts = segments(header);
            in_deps_table = false;
            if let Some(i) = parts.iter().position(|p| is_runtime_deps_name(p)) {
                if i + 1 == parts.len() {
                    in_deps_table = true; // entries follow on later lines
                } else {
                    found.push(format!("table [{header}]")); // `[dependencies.foo]`
                }
            }
            continue;
        }
        if in_deps_table {
            found.push(format!("entry `{line}`"));
            continue;
        }
        // Dotted or inline forms outside a table: `dependencies.foo = "1"`.
        let key = line.split('=').next().unwrap_or("");
        if segments(key).iter().any(|p| is_runtime_deps_name(p)) {
            found.push(format!("key `{}`", key.trim()));
        }
    }
    found
}

/// The dependency names Cargo.lock records for package `up`.
fn locked_dependencies_of_up(lock: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_up = false;
    let mut in_list = false;
    let mut seen = false;
    for line in lock.lines() {
        if line.trim() == "[[package]]" {
            in_up = false;
            in_list = false;
        } else if line.trim() == "name = \"up\"" {
            in_up = true;
            seen = true;
        } else if in_up && line.trim_start().starts_with("dependencies") {
            in_list = true;
        } else if in_up && in_list {
            let item = line.trim().trim_end_matches(',');
            if item != "]" && !item.is_empty() {
                deps.push(item.to_string());
            }
        }
    }
    assert!(seen, "Cargo.lock must list package `up`");
    deps
}

#[test]
fn test_pub_01_dependency_scanner_catches_declarations() {
    let bad = [
        "[package]\nname = \"up\"\n[dependencies]\nserde = \"1\"\n",
        "[build-dependencies]\ncc.workspace = true\n",
        "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
        "[target.\"x86_64-unknown-linux-gnu\".build-dependencies]\ncc = \"1\"\n",
        "[dependencies.serde]\nversion = \"1\"\n",
        "[package]\nname = \"up\"\ndependencies.serde = \"1\"\n",
        "[package]\nname = \"up\"\ndependencies = { serde = \"1\" }\n",
        "[ dependencies ]\nserde = \"1\"\n",
    ];
    for manifest in bad {
        assert!(
            !declared_dependencies(manifest).is_empty(),
            "scanner missed a declaration in:\n{manifest}"
        );
    }
    let good = "[package]\nname = \"up\"\n[[bin]]\nname = \"up\"\n[dependencies]\n# none\n\n[dev-dependencies]\ntempfile = \"3\"\n";
    assert_eq!(declared_dependencies(good), Vec::<String>::new());
    let locked = "[[package]]\nname = \"up\"\nversion = \"0.0.0\"\ndependencies = [\n \"serde\",\n]\n\n[[package]]\nname = \"serde\"\nversion = \"1\"\n";
    assert_eq!(locked_dependencies_of_up(locked), ["\"serde\""]);
    let clean = "[[package]]\nname = \"up\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"up-config\"\nversion = \"0.0.0\"\ndependencies = [\n \"serde\",\n]\n";
    assert!(locked_dependencies_of_up(clean).is_empty());
}

#[test]
fn test_pub_01_up_crate_has_no_dependencies() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let toml =
        std::fs::read_to_string(format!("{manifest}/Cargo.toml")).expect("crates/up/Cargo.toml");
    let declared = declared_dependencies(&toml);
    assert!(
        declared.is_empty(),
        "crates/up/Cargo.toml must declare no [dependencies] or [build-dependencies] (D1-01): {declared:?}"
    );
    let lock = std::fs::read_to_string(format!("{manifest}/../../Cargo.lock")).expect("Cargo.lock");
    let locked = locked_dependencies_of_up(&lock);
    assert!(
        locked.is_empty(),
        "Cargo.lock lists dependencies of package `up` (D1-01): {locked:?}"
    );
}
