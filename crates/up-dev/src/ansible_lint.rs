//! Ansible lint (CHG-07 lint stage, IMG-01): runs the hash-locked ansible-lint.
//!
//! Not implemented yet: until plan 01-08 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("ansible-lint", "01-08"))
}
