//! Dependency policy (SUPPLY-01): runs the pinned cargo-deny against the workspace.
//!
//! The binary comes from `versions.yml` through [`crate::tools`]. Its directory is put
//! first on `PATH` for the child so `cargo deny` resolves to the verified binary, not a
//! system copy. Policy lives in `deny.toml` (bans, sources, licenses, advisories).

use std::ffi::OsString;
use std::process::Command;

use crate::check::{CheckResult, Ctx, Failure};
use crate::tools;

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let bin = tools::ensure_tool(ctx, "cargo-deny")?;
    let dir = bin
        .parent()
        .ok_or_else(|| Failure::one("cargo-deny: pinned binary has no parent directory"))?;
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let path: OsString = std::env::join_paths(paths)
        .map_err(|e| Failure::one(format!("cargo-deny: cannot build PATH: {e}")))?;
    let status = Command::new("cargo")
        .args(["deny", "check", "advisories", "bans", "licenses", "sources"])
        .env("PATH", path)
        .current_dir(&ctx.root)
        .status()
        .map_err(|e| Failure::one(format!("cargo-deny: cannot run `cargo deny`: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::one("cargo-deny: policy violations above"))
    }
}
