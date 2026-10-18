//! CHG-07: Engine CI MUST implement the stages in the Testing table.
//! AC: Each stage exists as a named workflow job.
//!
//! This test verifies that:
//! - ci.yml contains exactly the ten expected job ids
//! - Each job invokes the correct tools/lint.sh check
//! - All job ids correspond to known checks

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// The ten required lint-stage jobs in ci.yml
const REQUIRED_JOBS: &[&str] = &[
    "rust",
    "shellcheck",
    "yamllint",
    "ansible-lint",
    "gitleaks",
    "workflow-lint",
    "spec-check",
    "schema-check",
    "repo-files",
    "renovate-check",
];

/// Known checks that tools/lint.sh accepts (from tools/lint.sh CHECKS variable)
const KNOWN_CHECKS: &[&str] = &[
    "rust",
    "cargo-deny",
    "repo-files",
    "schema-check",
    "spec-check",
    "workflow-lint",
    "renovate-check",
    "shellcheck",
    "yamllint",
    "ansible-lint",
    "gitleaks",
];

/// The up-dev subcommands that accept the check name as a CLI argument
const UPDEV_SUBCOMMANDS: &[&str] = &[
    "repo-files",
    "schema-check",
    "spec-check",
    "workflow-lint",
    "renovate-check",
    "shellcheck",
    "gitleaks",
    "cargo-deny",
    "yamllint",
    "ansible-lint",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn load_ci_yml() -> serde_json::Value {
    let path = repo_root().join(".github/workflows/ci.yml");
    up_dev::workflow_lint::load_workflow(&path)
        .unwrap_or_else(|e| panic!("ci.yml must load: {e}"))
}

/// Extract the job ids and their configurations from a workflow document.
fn extract_jobs(doc: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    doc["jobs"]
        .as_object()
        .expect("jobs must be an object")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Extract the `run:` value from the first step that has a `run:` key.
/// Returns None if no step has a `run:` key.
fn find_run_step(steps: &serde_json::Value) -> Option<String> {
    steps
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .find_map(|step| {
            step["run"]
                .as_str()
                .map(|s| s.to_string())
        })
}

#[test]
fn test_chg_07_ci_yml_has_exactly_ten_jobs() {
    let doc = load_ci_yml();
    let jobs = extract_jobs(&doc);
    assert_eq!(
        jobs.len(),
        10,
        "ci.yml must have exactly 10 jobs, found: {:?}",
        jobs.keys().collect::<Vec<_>>()
    );
}

#[test]
fn test_chg_07_all_required_jobs_exist() {
    let doc = load_ci_yml();
    let jobs = extract_jobs(&doc);
    let job_ids: BTreeSet<_> = jobs.keys().cloned().collect();

    for required in REQUIRED_JOBS {
        assert!(
            job_ids.contains(*required),
            "required job '{}' missing from ci.yml. jobs present: {:?}",
            required,
            job_ids
        );
    }
}

#[test]
fn test_chg_07_no_extra_jobs() {
    let doc = load_ci_yml();
    let jobs = extract_jobs(&doc);
    let job_ids: BTreeSet<_> = jobs.keys().cloned().collect();
    let required: BTreeSet<_> = REQUIRED_JOBS.iter().map(|s| s.to_string()).collect();

    let extra: Vec<_> = job_ids.difference(&required).collect();
    assert!(
        extra.is_empty(),
        "ci.yml has unexpected extra jobs: {:?}",
        extra
    );
}

#[test]
fn test_chg_07_each_job_runs_correct_check() {
    let doc = load_ci_yml();
    let jobs = extract_jobs(&doc);

    for job_id in REQUIRED_JOBS {
        let job = &jobs[&job_id.to_string()];
        let steps = &job["steps"];

        let run_step = find_run_step(steps).unwrap_or_else(|| {
            panic!(
                "job '{}' has no step with a 'run:' key",
                job_id
            )
        });

        // Special case: the rust job also runs cargo-deny in a conditional step
        if *job_id == "rust" {
            // The first run step should be `tools/lint.sh rust`
            assert!(
                run_step.contains("tools/lint.sh rust"),
                "job 'rust' first run step must invoke 'tools/lint.sh rust', got: {}",
                run_step
            );
        } else {
            // Other jobs should run `tools/lint.sh <job-id>`
            let expected = format!("tools/lint.sh {}", job_id);
            assert!(
                run_step.contains(&expected),
                "job '{}' must invoke '{}', got: {}",
                job_id,
                expected,
                run_step
            );
        }
    }
}

#[test]
fn test_chg_07_each_job_id_is_a_known_check() {
    let doc = load_ci_yml();
    let jobs = extract_jobs(&doc);

    for job_id in jobs.keys() {
        assert!(
            KNOWN_CHECKS.contains(&job_id.as_str()),
            "job '{}' is not in the list of known checks: {:?}",
            job_id,
            KNOWN_CHECKS
        );
    }
}

#[test]
fn test_chg_07_missing_job_is_detected() {
    // Load the actual workflow
    let mut doc = load_ci_yml();

    // Remove one job from the document: schema-check
    let job_name = "schema-check";
    if let Some(jobs_obj) = doc["jobs"].as_object_mut() {
        jobs_obj.remove(job_name);

        // Now verify that our job extraction would show it's missing
        let jobs = extract_jobs(&doc);
        let job_ids: Vec<_> = jobs.keys().collect();

        assert!(
            !job_ids.iter().any(|id| *id == job_name),
            "modified document should not contain the removed job '{}'",
            job_name
        );
        assert!(
            jobs.len() < 10,
            "modified document should have fewer than 10 jobs after removal"
        );
    } else {
        panic!("jobs must be an object");
    }
}

#[test]
fn test_chg_07_updev_subcommands_match_required_jobs() {
    // Every REQUIRED_JOBS except "rust" should either be a direct UPDEV_SUBCOMMAND
    // or be handled as cargo-deny (rust job's special case)
    for job_id in REQUIRED_JOBS {
        if *job_id != "rust" {
            assert!(
                UPDEV_SUBCOMMANDS.contains(job_id),
                "job '{}' is not a known up-dev subcommand. \
                 This means tools/lint.sh cannot dispatch it to up-dev.",
                job_id
            );
        }
    }
}
