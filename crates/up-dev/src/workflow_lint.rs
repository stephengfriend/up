//! Workflow lint (SUPPLY-02): SHA-pinned actions, read-only token, no pull_request_target with fork checkouts, no self-hosted runners.
//!
//! `run` loads every tracked workflow strictly and applies four layers (D1-07):
//!
//! 1. the engine's own rules ([`check_workflow`]), which hold whatever the
//!    external linters enable by default;
//! 2. actionlint, with the pinned shellcheck for `run:` scripts;
//! 3. zizmor in the pedantic persona, offline, with `.github/zizmor.yml`;
//! 4. check-jsonschema against the vendored GitHub workflow schema.
//!
//! Every layer runs, so one invocation shows all findings. Fixtures under
//! `tests/fixtures/workflows/` are never passed to the external tools.

mod rules;

pub use rules::{Violation, check_workflow};

use std::process::Command;

use crate::check::{CheckResult, Ctx, Failure, tracked_files};
use crate::{python_tools, tools};

/// Top-level keys of a GitHub workflow that YAML 1.1 reads as non-strings. Workflows are read
/// by GitHub, not by `up` (they are not in ARCH-11's file list), and the trigger key is
/// always spelled `on`; no other key is exempt from the strict profile.
pub const WORKFLOW_ROOT_KEYS: &[&str] = &["on"];

/// Strict-load a workflow file, allowing only the top-level `on:` trigger key.
pub fn load_workflow(
    path: &std::path::Path,
) -> Result<serde_json::Value, up_config::yaml::YamlError> {
    up_config::yaml::load_file_allowing_root_keys(path, WORKFLOW_ROOT_KEYS)
}

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
        match load_workflow(&ctx.root.join(file)) {
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

/// Run an external linter with inherited output; a failure to start or a
/// non-zero exit is one problem line.
fn run_external(name: &str, mut cmd: Command, problems: &mut Vec<String>) {
    match cmd.status() {
        Ok(status) if status.success() => {}
        Ok(_) => problems.push(format!("workflow-lint: {name} findings above")),
        Err(e) => problems.push(format!("workflow-lint: cannot run {name}: {e}")),
    }
}

/// The pinned external tools the second to fourth layers need.
pub struct ExternalTools {
    pub actionlint: std::path::PathBuf,
    pub shellcheck: std::path::PathBuf,
    /// `bin/` of the hash-locked Python venv (zizmor, check-jsonschema).
    pub bin_dir: std::path::PathBuf,
}

/// Install every tool, reporting all failures rather than the first.
fn setup_tools(ctx: &Ctx) -> Result<ExternalTools, Failure> {
    let actionlint = tools::ensure_tool(ctx, "actionlint");
    let shellcheck = tools::ensure_tool(ctx, "shellcheck");
    let bin_dir = python_tools::ensure_venv(ctx);
    match (actionlint, shellcheck, bin_dir) {
        (Ok(actionlint), Ok(shellcheck), Ok(bin_dir)) => Ok(ExternalTools {
            actionlint,
            shellcheck,
            bin_dir,
        }),
        (a, s, b) => Err(Failure::many(
            [a.err(), s.err(), b.err()]
                .into_iter()
                .flatten()
                .flat_map(|f| f.messages)
                .collect(),
        )),
    }
}

/// Layers 2 to 4: actionlint, zizmor and check-jsonschema.
fn run_external_layers(
    ctx: &Ctx,
    files: &[String],
    tools: &ExternalTools,
    problems: &mut Vec<String>,
) {
    let ExternalTools {
        actionlint,
        shellcheck,
        bin_dir,
    } = tools;
    let mut cmd = Command::new(actionlint);
    cmd.args(["-no-color", "-pyflakes="])
        .arg("-shellcheck")
        .arg(shellcheck)
        .args(files)
        .current_dir(&ctx.root);
    run_external("actionlint", cmd, problems);

    // `--no-ignores` makes zizmor refuse every ignore comment and ignore rule,
    // so a finding on an engine workflow can only be fixed, never silenced.
    let mut cmd = python_tools::tool_command(bin_dir, "zizmor");
    cmd.args([
        "--persona=pedantic",
        "--offline",
        "--no-ignores",
        "--strict-collection",
        "--no-progress",
        "--color",
        "never",
        "--config",
    ])
    .arg(ctx.root.join(".github/zizmor.yml"))
    .arg(ctx.root.join(".github/workflows"))
    .current_dir(&ctx.root);
    run_external("zizmor", cmd, problems);

    let mut cmd = python_tools::tool_command(bin_dir, "check-jsonschema");
    cmd.args(["--builtin-schema", "vendor.github-workflows"])
        .args(files)
        .current_dir(&ctx.root);
    run_external("check-jsonschema", cmd, problems);
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    run_with(ctx, setup_tools)
}

/// [`run`] with the tool installation injected, so tests can fail it.
pub fn run_with(
    ctx: &Ctx,
    setup: impl FnOnce(&Ctx) -> Result<ExternalTools, Failure>,
) -> CheckResult {
    let files = workflow_files(tracked_files(&ctx.root)?);
    if files.is_empty() {
        // A pass over zero workflows would be vacuous.
        return Err(Failure::one("workflow-lint: no workflows found"));
    }
    let mut problems = rust_rules(ctx, &files);

    // A failed installation is one more problem; it must not hide the findings above.
    match setup(ctx) {
        Ok(tools) => run_external_layers(ctx, &files, &tools, &mut problems),
        Err(failure) => problems.extend(failure.messages),
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(problems))
    }
}
