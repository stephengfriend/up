//! Pinned CI tool resolution (SUPPLY-01, D1-05).
//!
//! Not implemented yet: until plan 01-05 lands, `tool-path` fails closed.

use crate::check::{CheckResult, Ctx, Failure};

pub fn run_tool_path(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("tool-path", "01-05"))
}
