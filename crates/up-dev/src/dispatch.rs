//! The `up-dev` subcommand table: the single place that maps a subcommand name to its runner.
//! `main` dispatches through it and the CHG-07 tests compare it with `tools/lint.sh`.

use crate::check::{CheckResult, Ctx};
use crate::{
    ansible_lint, cargo_deny, gitleaks, renovate_check, repo_files, schema_check, shellcheck,
    spec_check, tools, workflow_lint, yamllint,
};

/// A subcommand implementation.
pub type Runner = fn(&Ctx, &[String]) -> CheckResult;

/// Every subcommand, in help order.
pub const SUBCOMMANDS: &[(&str, Runner)] = &[
    ("repo-files", repo_files::run),
    ("schema-check", schema_check::run),
    ("spec-check", spec_check::run),
    ("workflow-lint", workflow_lint::run),
    ("renovate-check", renovate_check::run),
    ("shellcheck", shellcheck::run),
    ("gitleaks", gitleaks::run),
    ("cargo-deny", cargo_deny::run),
    ("yamllint", yamllint::run),
    ("ansible-lint", ansible_lint::run),
    ("tool-path", tools::run_tool_path),
];

/// The subcommand names, in table order.
pub fn subcommand_names() -> Vec<&'static str> {
    SUBCOMMANDS.iter().map(|(name, _)| *name).collect()
}
