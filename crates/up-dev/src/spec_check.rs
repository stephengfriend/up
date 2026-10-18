//! Spec checker (PUB-09, FND-04): unique IDs, front matter, roadmap coverage, negative ACs, normative words outside requirements.
//!
//! Not implemented yet: until plan 01-07 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("spec-check", "01-07"))
}
