//! YAML style lint (CHG-07 lint stage): runs the hash-locked yamllint.
//!
//! Not implemented yet: until plan 01-08 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("yamllint", "01-08"))
}
