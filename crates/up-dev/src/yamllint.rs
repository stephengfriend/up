//! YAML style lint (CHG-07 lint stage): runs the hash-locked yamllint.
//!
//! yamllint comes from the venv built out of `tools/ci-requirements.txt` by
//! [`crate::python_tools`] (SUPPLY-07); nothing here trusts a system yamllint.

use crate::check::{CheckResult, Ctx, Failure, tracked_files};
use crate::python_tools;

/// Tracked YAML files yamllint covers: every `*.yml` and `*.yaml` outside
/// `tests/fixtures/` (fixtures are deliberately malformed). Sorted.
pub fn yaml_files(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut files: Vec<String> = tracked
        .into_iter()
        .filter(|p| {
            (p.ends_with(".yml") || p.ends_with(".yaml")) && !p.starts_with("tests/fixtures/")
        })
        .collect();
    files.sort();
    files
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let files = yaml_files(tracked_files(&ctx.root)?);
    if files.is_empty() {
        // A pass over zero files would be vacuous.
        return Err(Failure::one("yamllint: no YAML files found"));
    }
    let bin_dir = python_tools::ensure_venv(ctx)?;
    let status = python_tools::tool_command(&bin_dir, "yamllint")
        .args(["--strict", "-c"])
        .arg(ctx.root.join(".yamllint.yml"))
        .args(&files)
        .current_dir(&ctx.root)
        .status()
        .map_err(|e| {
            Failure::one(format!(
                "yamllint: cannot run {}: {e}",
                bin_dir.join("yamllint").display()
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one("yamllint: findings above"))
    }
}
