//! Workflow lint (SUPPLY-02): SHA-pinned actions, read-only token, no pull_request_target with fork checkouts, no self-hosted runners.
//!
//! Not implemented yet: until plan 01-09 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

mod rules;

pub use rules::{Violation, check_workflow};

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("workflow-lint", "01-09"))
}
