//! Credential scan (CHG-07 lint stage, GRD-06): runs the pinned gitleaks with redaction.
//!
//! The binary comes from `versions.yml` through [`crate::tools`] (SUPPLY-01). The scan
//! covers the full git history (CI checks out with `fetch-depth: 0`) and always passes
//! `--redact`, so a finding never prints the secret into a public log (T-01-16).
//! The scan is hardened against its own subject (CR-01): the config is engine-owned and
//! passed explicitly, tree ignore files and `gitleaks:allow` comments are not honoured, and
//! the environment is cleared. No allowlist is ever added: a human records any exception.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::check::{CheckResult, Ctx, Failure};
use crate::tools;

/// The engine-owned gitleaks configuration. It only extends gitleaks' built-in ruleset;
/// it must never carry an allowlist, so the scan cannot be narrowed from inside the tree
/// (CR-01). It is written to a private file at scan time rather than read from the
/// scanned tree, where a pull request could replace it.
pub const ENGINE_CONFIG: &str =
    "title = \"engine credential scan\"\n\n[extend]\nuseDefault = true\n";

/// Environment variables gitleaks (and the `git` it runs) may inherit. Everything else,
/// in particular every `GITLEAKS_*` variable, is dropped.
const KEPT_ENV: [&str; 3] = ["PATH", "HOME", "TMPDIR"];

/// Replace the child's environment with only [`KEPT_ENV`], copied from this process.
pub fn scrub_env(cmd: &mut Command) {
    cmd.env_clear();
    for key in KEPT_ENV {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
}

/// Write [`ENGINE_CONFIG`] under the engine's own `target/` directory and return its path.
fn write_engine_config(engine_root: &Path) -> Result<PathBuf, Failure> {
    let dir = engine_root.join("target/up-dev-tools");
    std::fs::create_dir_all(&dir)
        .map_err(|e| Failure::one(format!("gitleaks: cannot create {}: {e}", dir.display())))?;
    let path = dir.join(format!("gitleaks-engine-{}.toml", std::process::id()));
    std::fs::write(&path, ENGINE_CONFIG)
        .map_err(|e| Failure::one(format!("gitleaks: cannot write {}: {e}", path.display())))?;
    Ok(path)
}

/// The repository's git directory. Scanning it instead of the work tree keeps gitleaks
/// from reading `<source>/.gitleaksignore` out of the checked-out files, which it does
/// even when `--gitleaks-ignore-path` points elsewhere.
fn git_dir(repo: &Path) -> Result<PathBuf, Failure> {
    let mut cmd = Command::new("git");
    scrub_env(&mut cmd);
    let out = cmd
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .map_err(|e| Failure::one(format!("gitleaks: cannot run git: {e}")))?;
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || dir.is_empty() {
        return Err(Failure::one(format!(
            "gitleaks: {} is not a git repository: {}",
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(PathBuf::from(dir))
}

/// Scan the git history of `repo` with the gitleaks binary `bin`.
///
/// The configuration is passed explicitly (so `<repo>/.gitleaks.toml`, `GITLEAKS_CONFIG` and
/// `GITLEAKS_CONFIG_TOML` are never consulted), the ignore file points at `/dev/null`
/// (so `<repo>/.gitleaksignore` is not read), inline `gitleaks:allow` comments are ignored,
/// the environment is cleared, and the source is the git directory so no file of the
/// work tree is consulted as configuration.
pub fn scan(engine_root: &Path, bin: &Path, repo: &Path) -> Result<ExitStatus, Failure> {
    let source = git_dir(repo)?;
    let config = write_engine_config(engine_root)?;
    let mut cmd = Command::new(bin);
    scrub_env(&mut cmd);
    let result = cmd
        .args(["git", "--redact", "--no-banner", "--exit-code", "1"])
        .args([
            "--ignore-gitleaks-allow",
            "--gitleaks-ignore-path",
            "/dev/null",
        ])
        .arg("--config")
        .arg(&config)
        .arg(&source)
        .current_dir(&source)
        .status()
        .map_err(|e| Failure::one(format!("gitleaks: cannot run {}: {e}", bin.display())));
    let _ = std::fs::remove_file(&config);
    result
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let bin = tools::ensure_tool(ctx, "gitleaks")?;
    let status = scan(&ctx.root, &bin, &ctx.root)?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one(
            "gitleaks: credential findings above (redacted); do not add allowlist entries, a human records any exception",
        ))
    }
}
