//! Ansible lint (CHG-07 lint stage, IMG-01): runs the hash-locked ansible-lint.
//!
//! ansible-lint comes from the venv built out of `tools/ci-requirements.txt` by
//! [`crate::python_tools`] (SUPPLY-07). It runs offline with the production
//! profile; the IMG-01 phase-tag rule is added with the first role in Phase 9.

use std::io::Write;
use std::process::Stdio;

use crate::check::{CheckResult, Ctx, Failure, tracked_files};
use crate::python_tools;

/// Tracked `*.yml` files under `ansible/`. Sorted.
pub fn ansible_files(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut files: Vec<String> = tracked
        .into_iter()
        .filter(|p| p.starts_with("ansible/") && p.ends_with(".yml"))
        .collect();
    files.sort();
    files
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let files = ansible_files(tracked_files(&ctx.root)?);
    if files.is_empty() {
        // A pass over zero files would be vacuous.
        return Err(Failure::one("ansible-lint: no Ansible content found"));
    }
    let bin_dir = python_tools::ensure_venv(ctx)?;
    let scratch = ctx.root.join("target/up-dev-tools");
    // ansible-config aborts when an inherited stdio handle is non-blocking, which
    // some terminals and CI wrappers hand down. Giving the child fresh pipes
    // (blocking) and relaying its output keeps the check independent of that.
    let output = python_tools::tool_command(&bin_dir, "ansible-lint")
        .arg("-c")
        .arg(ctx.root.join(".ansible-lint"))
        .arg("ansible/")
        .env("ANSIBLE_HOME", scratch.join("ansible-home"))
        .env("ANSIBLE_LOCAL_TEMP", scratch.join("ansible-tmp"))
        .current_dir(&ctx.root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            Failure::one(format!(
                "ansible-lint: cannot run {}: {e}",
                bin_dir.join("ansible-lint").display()
            ))
        })?;
    // Relay failures to the terminal are best effort; the exit status decides.
    let _ = std::io::stdout().write_all(&output.stdout);
    let _ = std::io::stderr().write_all(&output.stderr);
    let status = output.status;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one("ansible-lint: findings above"))
    }
}
