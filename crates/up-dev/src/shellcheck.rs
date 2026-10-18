//! Shell lint (CHG-07 lint stage): runs the pinned shellcheck over the engine shell scripts.
//!
//! The binary comes from `versions.yml` through the verify-then-install path in
//! [`crate::tools`] (SUPPLY-01); nothing here downloads or trusts a system shellcheck.

use std::process::Command;

use crate::check::{CheckResult, Ctx, Failure, tracked_files};
use crate::tools;

/// Tracked files shellcheck covers: every `*.sh`, plus everything under `hooks/`
/// except the `.gitkeep` placeholder. Sorted, so runs are reproducible.
pub fn shell_files(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut files: Vec<String> = tracked
        .into_iter()
        .filter(|p| p.ends_with(".sh") || (p.starts_with("hooks/") && !p.ends_with(".gitkeep")))
        .collect();
    files.sort();
    files
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let bin = tools::ensure_tool(ctx, "shellcheck")?;
    let files = shell_files(tracked_files(&ctx.root)?);
    if files.is_empty() {
        // A pass over zero files would be vacuous.
        return Err(Failure::one("shellcheck: no shell files found"));
    }
    let status = Command::new(&bin)
        .args(["--norc", "--severity=style"])
        .args(&files)
        .current_dir(&ctx.root)
        .status()
        .map_err(|e| Failure::one(format!("shellcheck: cannot run {}: {e}", bin.display())))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one("shellcheck: findings above"))
    }
}
