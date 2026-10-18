//! Credential scan (CHG-07 lint stage, GRD-06): runs the pinned gitleaks with redaction.
//!
//! The binary comes from `versions.yml` through [`crate::tools`] (SUPPLY-01). The scan
//! covers the full git history (CI checks out with `fetch-depth: 0`) and always passes
//! `--redact`, so a finding never prints the secret into a public log (T-01-16).
//! No allowlist, ignore file or config is added here: a human records any exception.

use std::process::Command;

use crate::check::{CheckResult, Ctx, Failure};
use crate::tools;

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let bin = tools::ensure_tool(ctx, "gitleaks")?;
    let status = Command::new(&bin)
        .args(["git", "--redact", "--no-banner", "--exit-code", "1"])
        .arg(&ctx.root)
        .current_dir(&ctx.root)
        .status()
        .map_err(|e| Failure::one(format!("gitleaks: cannot run {}: {e}", bin.display())))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one(
            "gitleaks: credential findings above (redacted); do not add allowlist entries, a human records any exception",
        ))
    }
}
