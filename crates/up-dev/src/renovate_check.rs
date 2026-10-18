//! Renovate config check (SUPPLY-06): automerge must be off everywhere.
//!
//! Not implemented yet: until plan 01-10 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("renovate-check", "01-10"))
}
