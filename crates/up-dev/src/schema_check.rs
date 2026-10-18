//! Schema check (ARCH-05, FND-03): strict YAML loading, JSON Schema validation and hostile-value fixtures.
//!
//! Not implemented yet: until plan 01-03 lands this check fails closed, so it
//! never passes vacuously. The signature of `run` stays as it is.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("schema-check", "01-03"))
}
