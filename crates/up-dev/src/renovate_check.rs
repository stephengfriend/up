//! Renovate config check (SUPPLY-06): automerge must be off everywhere.
//!
//! RED stub for plan 01-10 task 1: `check_renovate_config` finds nothing yet,
//! and `run` still fails closed so the check never passes vacuously.

use crate::check::{CheckResult, Ctx, Failure};

/// Presets an engine or template config may extend; anything else can switch
/// automerge on behind the check's back.
pub const REVIEWED_PRESETS: &[&str] = &[
    "config:recommended",
    "helpers:pinGitHubActionDigestsToSemver",
];

pub fn check_renovate_config(_doc: &serde_json::Value) -> Vec<String> {
    Vec::new()
}

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("renovate-check", "01-10"))
}
