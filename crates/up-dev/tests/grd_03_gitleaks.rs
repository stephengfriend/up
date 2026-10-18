//! GRD-03 / SUPPLY-02 (T-01-16): the credential scan must not be weakened by files or
//! environment in the tree it scans (CR-01). Uses the real pinned gitleaks (cached under
//! `target/up-dev-tools`, downloaded and hash-checked on first use).
//!
//! The token-shaped value is generated at runtime so the repository's own scan stays clean.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};
use up_dev::check::Ctx;
use up_dev::gitleaks::{scan, scrub_env};
use up_dev::tools;

fn engine_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("engine root")
}

fn gitleaks_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let ctx = Ctx {
            root: engine_root(),
            github_actions: false,
        };
        tools::ensure_tool(&ctx, "gitleaks").expect("pinned gitleaks installs")
    })
}

/// A GitHub-PAT-shaped string that the default ruleset matches, built at runtime.
fn fake_token() -> String {
    let mut hex = String::new();
    for b in Sha256::digest(b"up-dev grd-03 fixture").iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("ghp_{}", &hex[..36])
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit_all(repo: &Path, msg: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", msg]);
}

fn leaky_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(
        dir.path().join("leak.txt"),
        format!("token = {}\n", fake_token()),
    )
    .expect("write leak");
    commit_all(dir.path(), "add leak");
    dir
}

#[test]
fn test_grd_03_control_leak_is_found() {
    let repo = leaky_repo();
    let status = scan(&engine_root(), gitleaks_bin(), repo.path()).expect("scan runs");
    assert_eq!(status.code(), Some(1), "a plain token must be reported");
}

#[test]
fn test_grd_03_tree_allowlist_config_does_not_hide_leak() {
    let repo = leaky_repo();
    std::fs::write(
        repo.path().join(".gitleaks.toml"),
        "[allowlist]\npaths = ['''.*''']\n",
    )
    .expect("write config");
    commit_all(repo.path(), "add allowlist config");
    let status = scan(&engine_root(), gitleaks_bin(), repo.path()).expect("scan runs");
    assert_eq!(
        status.code(),
        Some(1),
        "a tree .gitleaks.toml must not allowlist"
    );
}

#[test]
fn test_grd_03_tree_gitleaksignore_does_not_hide_leak() {
    let repo = leaky_repo();
    let sha = git(repo.path(), &["rev-parse", "HEAD"]);
    std::fs::write(
        repo.path().join(".gitleaksignore"),
        format!("{sha}:leak.txt:github-pat:1\n"),
    )
    .expect("write ignore");
    commit_all(repo.path(), "add ignore file");
    let status = scan(&engine_root(), gitleaks_bin(), repo.path()).expect("scan runs");
    assert_eq!(
        status.code(),
        Some(1),
        "a tree .gitleaksignore must not ignore"
    );
}

#[test]
fn test_grd_03_inline_gitleaks_allow_does_not_hide_leak() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(
        dir.path().join("leak.txt"),
        format!("token = {} # gitleaks:allow\n", fake_token()),
    )
    .expect("write leak");
    commit_all(dir.path(), "add allowed leak");
    let status = scan(&engine_root(), gitleaks_bin(), dir.path()).expect("scan runs");
    assert_eq!(
        status.code(),
        Some(1),
        "inline gitleaks:allow must be ignored"
    );
}

#[test]
fn test_grd_03_scan_environment_is_cleared() {
    let mut cmd = Command::new("env");
    cmd.env("GITLEAKS_CONFIG_TOML", "[allowlist]\npaths = ['''.*''']")
        .env("GITLEAKS_CONFIG", "/nonexistent");
    scrub_env(&mut cmd);
    let out = cmd.output().expect("env runs");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("GITLEAKS"), "environment leaked: {text}");
    for line in text.lines() {
        let key = line.split('=').next().unwrap_or_default();
        assert!(
            matches!(key, "PATH" | "HOME" | "TMPDIR"),
            "unexpected variable passed to gitleaks: {key}"
        );
    }
}

// ---- repo-files: no tracked gitleaks config or ignore file (CR-01) ----

use std::collections::BTreeSet;
use up_dev::repo_files::check_no_gitleaks_config;

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| (*p).to_string()).collect()
}

#[test]
fn test_grd_03_repo_files_clean_tree_passes() {
    let tracked = set(&["README.md", "tools/lint.sh", "docs/gitleaks-notes.md"]);
    assert_eq!(check_no_gitleaks_config(&tracked), Vec::<String>::new());
}

#[test]
fn test_grd_03_repo_files_rejects_gitleaks_toml_and_ignore_at_any_depth() {
    for bad in [
        ".gitleaks.toml",
        ".gitleaksignore",
        "gitleaks.toml",
        "gitleaks-extra.toml",
        "sub/dir/.gitleaks.toml",
        "sub/.gitleaksignore",
        "tools/gitleaks.local.toml",
    ] {
        let messages = check_no_gitleaks_config(&set(&["README.md", bad]));
        assert_eq!(messages.len(), 1, "{bad}: {messages:?}");
        assert!(messages[0].contains(bad), "{bad}: {messages:?}");
        assert!(messages[0].starts_with("GRD-03:"), "{bad}: {messages:?}");
    }
}
