//! Workflow lint (SUPPLY-02): SHA-pinned actions, read-only token, no pull_request_target with fork checkouts, no self-hosted runners.
//!
//! `run` loads every tracked workflow strictly and applies the engine's own
//! rules ([`check_workflow`]). Plan 01-09 layers actionlint, zizmor and schema
//! validation on top of the same function.

mod rules;

pub use rules::{Violation, check_workflow};

use crate::check::{CheckResult, Ctx, Failure, tracked_files};

/// Tracked workflow files: `.github/workflows/*.yml` and `*.yaml`, sorted.
pub fn workflow_files(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut files: Vec<String> = tracked
        .into_iter()
        .filter(|p| {
            p.strip_prefix(".github/workflows/").is_some_and(|rest| {
                !rest.contains('/') && (rest.ends_with(".yml") || rest.ends_with(".yaml"))
            })
        })
        .collect();
    files.sort();
    files
}

fn rust_rules(ctx: &Ctx, files: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for file in files {
        match up_config::yaml::load_file(&ctx.root.join(file)) {
            Ok(doc) => {
                for v in check_workflow(&doc) {
                    problems.push(format!(
                        "SUPPLY-02: {file}: {}: [{}] {}",
                        v.location, v.rule, v.message
                    ));
                }
            }
            Err(e) => problems.push(format!("SUPPLY-02: {file}: {e}")),
        }
    }
    problems
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let files = workflow_files(tracked_files(&ctx.root)?);
    if files.is_empty() {
        // A pass over zero workflows would be vacuous.
        return Err(Failure::one("workflow-lint: no workflows found"));
    }
    let problems = rust_rules(ctx, &files);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(problems))
    }
}
