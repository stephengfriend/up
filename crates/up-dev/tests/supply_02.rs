//! SUPPLY-02: engine CI pins third-party actions by SHA, keeps the token
//! read-only, avoids `pull_request_target` fork checkouts and self-hosted runners.
//! Every bad-workflow fixture in `tests/fixtures/workflows/` starts with a
//! `# expect: <rule>` (or `# expect: pass`) header and is checked against it.

use std::fs;
use std::path::PathBuf;

use up_dev::workflow_lint::{Violation, check_workflow};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn fixtures_dir() -> PathBuf {
    repo_root().join("tests/fixtures/workflows")
}

fn check_file(path: &std::path::Path) -> Vec<Violation> {
    let doc = up_dev::workflow_lint::load_workflow(path)
        .unwrap_or_else(|e| panic!("{} must load: {e}", path.display()));
    check_workflow(&doc)
}

fn check_fixture(name: &str) -> Vec<Violation> {
    check_file(&fixtures_dir().join(name))
}

/// The value after `# expect:` on the first line.
fn expectation(name: &str) -> String {
    let text = fs::read_to_string(fixtures_dir().join(name)).expect("fixture readable");
    let first = text.lines().next().unwrap_or("");
    first
        .strip_prefix("# expect: ")
        .unwrap_or_else(|| panic!("{name}: first line must be `# expect: <rule>|pass`"))
        .trim()
        .to_string()
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures_dir())
        .expect("fixtures dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".yml"))
        .collect();
    names.sort();
    names
}

fn render(v: &[Violation]) -> String {
    v.iter()
        .map(|v| format!("[{}] {}: {}", v.rule, v.location, v.message))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Exactly one violation, of the named rule.
fn assert_exactly(name: &str, rule: &str) {
    let v = check_fixture(name);
    assert!(
        v.len() == 1 && v[0].rule == rule,
        "{name}: expected exactly one [{rule}] violation, got:\n{}",
        render(&v)
    );
}

#[test]
fn test_supply_02_engine_ci_workflow_passes() {
    let v = check_file(&repo_root().join(".github/workflows/ci.yml"));
    assert!(v.is_empty(), "ci.yml must be clean:\n{}", render(&v));
}

#[test]
fn test_supply_02_passing_workflows_pass() {
    let mut seen = 0;
    for name in fixture_names() {
        if expectation(&name) == "pass" {
            let v = check_fixture(&name);
            assert!(v.is_empty(), "{name} must pass:\n{}", render(&v));
            seen += 1;
        }
    }
    assert!(
        seen >= 1,
        "at least one `# expect: pass` fixture must exist"
    );
}

#[test]
fn test_supply_02_unpinned_action_fails() {
    assert_exactly("unpinned-tag.yml", "unpinned-action");
    let v = check_fixture("unpinned-tag.yml");
    assert!(
        v[0].location.contains("jobs.build.steps[0]"),
        "location must name the step: {}",
        v[0].location
    );
    assert!(
        v[0].message.contains("actions/checkout@v4"),
        "message must name the action: {}",
        v[0].message
    );
}

#[test]
fn test_supply_02_unpinned_action_boundaries() {
    for name in [
        "unpinned-short-sha.yml",
        "unpinned-sha-39.yml",
        "unpinned-sha-41.yml",
        "unpinned-sha-uppercase.yml",
        "unpinned-docker-tag.yml",
        "unpinned-reusable-branch.yml",
    ] {
        assert_exactly(name, "unpinned-action");
    }
    for name in [
        "local-action-pass.yml",
        "docker-digest-pass.yml",
        "reusable-pinned-pass.yml",
    ] {
        let v = check_fixture(name);
        assert!(v.is_empty(), "{name} must pass:\n{}", render(&v));
    }
}

#[test]
fn test_supply_02_missing_permissions_fails() {
    assert_exactly("permissions-missing.yml", "token-permissions");
}

#[test]
fn test_supply_02_write_default_token_fails() {
    assert_exactly("permissions-write-all.yml", "token-permissions");
    assert_exactly("permissions-contents-write.yml", "token-permissions");
    for name in [
        "permissions-empty-pass.yml",
        "permissions-read-all-pass.yml",
    ] {
        let v = check_fixture(name);
        assert!(v.is_empty(), "{name} must pass:\n{}", render(&v));
    }
}

#[test]
fn test_supply_02_pull_request_target_head_checkout_fails() {
    for name in [
        "prt-head-sha-checkout.yml",
        "prt-head-ref-spacing.yml",
        "prt-list-trigger.yml",
        "prt-refs-pull.yml",
        // CR/WR-01: other ways to name fork code or its merge with base.
        "prt-merge-commit-sha.yml",
        "prt-pr-number-ref.yml",
        "prt-event-number-ref.yml",
        "prt-bracket-head.yml",
        "workflow-run-head-sha.yml",
    ] {
        assert_exactly(name, "pr-target-head-checkout");
    }
    let v = check_fixture("prt-without-head-checkout-pass.yml");
    assert!(
        v.iter().all(|v| v.rule != "pr-target-head-checkout"),
        "prt-without-head-checkout-pass.yml must not trip the rule:\n{}",
        render(&v)
    );
}

#[test]
fn test_supply_02_self_hosted_runner_fails() {
    for name in [
        "self-hosted-string.yml",
        "self-hosted-array.yml",
        "self-hosted-labels.yml",
        "self-hosted-matrix.yml",
        "self-hosted-matrix-include.yml",
        "self-hosted-mixed-case.yml",
    ] {
        assert_exactly(name, "self-hosted-runner");
    }
}

#[test]
fn test_supply_02_runner_group_fails() {
    assert_exactly("self-hosted-group.yml", "self-hosted-runner");
    let v = check_fixture("self-hosted-group.yml");
    assert!(
        v[0].message.contains("build") && v[0].message.contains("GitHub-hosted"),
        "message must name the job and the unproven runner: {}",
        v[0].message
    );
}

#[test]
fn test_supply_02_runs_on_expression_fails() {
    for name in [
        "self-hosted-expression.yml",
        "self-hosted-matrix-unresolvable.yml",
        "self-hosted-matrix-missing-key.yml",
        "self-hosted-labels-expression.yml",
    ] {
        assert_exactly(name, "self-hosted-runner");
        let v = check_fixture(name);
        assert!(
            v[0].message.contains("build") && v[0].message.contains("GitHub-hosted"),
            "{name}: message must name the job and the unproven runner: {}",
            v[0].message
        );
    }
}

#[test]
fn test_supply_02_static_matrix_runs_on_passes() {
    let v = check_fixture("runs-on-matrix-static-ok.yml");
    assert!(v.is_empty(), "static matrix must pass:\n{}", render(&v));
    // A static matrix that resolves to `self-hosted` still fails.
    assert_exactly("self-hosted-matrix.yml", "self-hosted-runner");
    assert_exactly("self-hosted-matrix-include.yml", "self-hosted-runner");
}

#[test]
fn test_supply_02_empty_workflow_fails() {
    assert_exactly("empty-jobs.yml", "workflow-shape");
}

#[test]
fn test_supply_02_every_fixture_has_expectation() {
    const RULES: [&str; 5] = [
        "unpinned-action",
        "token-permissions",
        "pr-target-head-checkout",
        "self-hosted-runner",
        "workflow-shape",
    ];
    let names = fixture_names();
    assert!(
        names.len() >= 20,
        "expected 20 or more fixtures, found {}",
        names.len()
    );
    let mut covered: Vec<String> = Vec::new();
    for name in &names {
        let expect = expectation(name);
        if expect == "pass" {
            let v = check_fixture(name);
            assert!(v.is_empty(), "{name} must pass:\n{}", render(&v));
        } else {
            assert!(
                RULES.contains(&expect.as_str()),
                "{name}: unknown rule `{expect}` in the expect header"
            );
            assert_exactly(name, &expect);
            covered.push(expect);
        }
    }
    for rule in RULES {
        assert!(
            covered.iter().any(|c| c == rule),
            "no fixture targets rule {rule}"
        );
    }
}

/// Every key in a JSON document, recursively.
fn all_keys(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                out.push(k.clone());
                all_keys(child, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|i| all_keys(i, out)),
        _ => {}
    }
}

#[test]
fn test_supply_02_zizmor_policy_requires_hash_pins_and_has_no_ignores() {
    let doc = up_config::yaml::load_file(&repo_root().join(".github/zizmor.yml"))
        .expect("zizmor.yml must load");
    assert_eq!(
        doc["rules"]["unpinned-uses"]["config"]["policies"]["*"], "hash-pin",
        "every action must require a hash pin"
    );
    let mut keys = Vec::new();
    all_keys(&doc, &mut keys);
    for forbidden in ["ignore", "disable", "disabled"] {
        assert!(
            !keys.iter().any(|k| k == forbidden),
            "zizmor.yml must not carry `{forbidden}` entries"
        );
    }
}
