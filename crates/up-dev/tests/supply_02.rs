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
    let doc = up_config::yaml::load_file(path)
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
    assert!(seen >= 1, "at least one `# expect: pass` fixture must exist");
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
