//! Credential scan (CHG-07 lint stage, GRD-06): runs the pinned gitleaks with redaction.
//!
//! Not implemented yet: until plan 01-05 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("gitleaks", "01-05"))
}
