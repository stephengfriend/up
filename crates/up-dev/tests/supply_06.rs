//! SUPPLY-06: Renovate and CI may open dependency PRs but never merge or sign
//! them, so the Renovate config must keep automerge off. Fixtures live in
//! `tests/fixtures/renovate/`; the expected outcome of each is in the tests
//! because JSON has no comments.

use std::fs;
use std::path::PathBuf;

use up_dev::renovate_check::{check_renovate_config, select_config_files};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root().join("tests/fixtures/renovate").join(name)
}

fn load(path: &std::path::Path) -> serde_json::Value {
    up_config::yaml::load_file(path).unwrap_or_else(|e| panic!("{} must load: {e}", path.display()))
}

fn messages(name: &str) -> Vec<String> {
    check_renovate_config(&load(&fixture_path(name)))
}

#[test]
fn test_supply_06_engine_config_has_automerge_disabled() {
    let doc = load(&repo_root().join("renovate.json"));
    assert_eq!(doc["automerge"], serde_json::json!(false));
    assert_eq!(doc["platformAutomerge"], serde_json::json!(false));
    let found = check_renovate_config(&doc);
    assert!(found.is_empty(), "engine config: {found:?}");
    let pass = messages("pass.json");
    assert!(pass.is_empty(), "pass.json: {pass:?}");
}

#[test]
fn test_supply_06_automerge_true_fails() {
    let found = messages("automerge-top-true.json");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("SUPPLY-06: "), "{found:?}");
    assert!(found[0].contains("/automerge"), "{found:?}");
    assert!(found[0].contains("automerge is true"), "{found:?}");
}

/// Assert exactly one message, starting `SUPPLY-06: `, containing every needle.
fn assert_one_message(name: &str, needles: &[&str]) {
    let found = messages(name);
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    assert!(found[0].starts_with("SUPPLY-06: "), "{name}: {found:?}");
    for needle in needles {
        assert!(
            found[0].contains(needle),
            "{name}: {found:?} lacks {needle}"
        );
    }
}

#[test]
fn test_supply_06_engine_config_enables_only_github_actions_and_cargo() {
    // D1-08: versions.yml and tools/ci-requirements.txt carry hashes Renovate
    // cannot recompute, so no manager that would touch them may be enabled.
    let doc = load(&repo_root().join("renovate.json"));
    assert_eq!(
        doc["enabledManagers"],
        serde_json::json!(["github-actions", "cargo"])
    );
    assert_eq!(
        doc["extends"],
        serde_json::json!([
            "config:recommended",
            "helpers:pinGitHubActionDigestsToSemver"
        ])
    );
    let text = fs::read_to_string(repo_root().join("renovate.json")).expect("renovate.json");
    for forbidden in [
        "versions.yml",
        "ci-requirements",
        "customManagers",
        "regex",
        "pip",
    ] {
        assert!(
            !text.contains(forbidden),
            "renovate.json mentions {forbidden}"
        );
    }
}

#[test]
fn test_supply_06_automerge_missing_fails() {
    assert_one_message(
        "automerge-missing.json",
        &["has to set automerge: false at the top level"],
    );
}

#[test]
fn test_supply_06_platform_automerge_missing_fails() {
    assert_one_message(
        "platform-automerge-missing.json",
        &["has to set platformAutomerge: false at the top level"],
    );
}

#[test]
fn test_supply_06_platform_automerge_true_fails() {
    assert_one_message(
        "platform-automerge-true.json",
        &["/platformAutomerge", "platformAutomerge is true"],
    );
}

#[test]
fn test_supply_06_non_boolean_automerge_fails() {
    assert_one_message(
        "automerge-string-true.json",
        &["/automerge", "automerge is not a boolean"],
    );
}

#[test]
fn test_supply_06_automerge_in_package_rule_fails_regardless_of_order() {
    // Rule 0 turns automerge on and rule 1 turns it off for the same match;
    // only rule 0 is reported, and it is reported whatever follows it.
    assert_one_message(
        "package-rule-automerge-order.json",
        &["/packageRules/0/automerge", "automerge is true"],
    );
}

#[test]
fn test_supply_06_automerge_in_lockfile_maintenance_fails() {
    assert_one_message(
        "lockfile-maintenance-automerge.json",
        &["/lockFileMaintenance/automerge"],
    );
}

#[test]
fn test_supply_06_automerge_in_update_type_fails() {
    assert_one_message("update-type-automerge.json", &["/minor/automerge"]);
}

#[test]
fn test_supply_06_auto_approve_fails() {
    assert_one_message(
        "auto-approve.json",
        &["/packageRules/0/autoApprove", "autoApprove is true"],
    );
}

#[test]
fn test_supply_06_automerge_preset_fails() {
    assert_one_message(
        "preset-automerge-all.json",
        &[
            "preset \":automergeAll\" at /extends/2",
            "is not on the reviewed list (presets can enable automerge)",
        ],
    );
}

#[test]
fn test_supply_06_unreviewed_preset_fails() {
    assert_one_message(
        "preset-unreviewed.json",
        &["preset \"config:best-practices\" at /extends/0"],
    );
    // A preset inside a packageRules entry is checked at any depth.
    assert_one_message(
        "preset-in-package-rule.json",
        &["preset \"config:best-practices\" at /packageRules/0/extends/0"],
    );
}

#[test]
fn test_supply_06_duplicate_key_rejected() {
    // A last-key-wins JSON parse would read `automerge: true` as the value;
    // a first-key-wins one would read false. The strict loader refuses both.
    let err = up_config::yaml::load_file(&fixture_path("duplicate-key.json"))
        .expect_err("duplicate keys must be refused at load time");
    assert_eq!(err.kind(), "duplicate-key", "{err}");
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| (*p).to_string()).collect()
}

#[test]
fn test_supply_06_every_tracked_config_checked() {
    let tracked = strings(&[
        "template/renovate.json",
        "renovate.json",
        ".github/renovate.json",
        "tests/fixtures/renovate/pass.json",
        "tests/fixtures/renovate/renovate.json",
        "docs/not-renovate.json",
        "crates/up-dev/src/renovate_check.rs",
    ]);
    assert_eq!(
        select_config_files(tracked),
        Ok(strings(&[
            ".github/renovate.json",
            "renovate.json",
            "template/renovate.json",
        ]))
    );
    // The root config is required; a template copy alone does not satisfy it.
    let err = select_config_files(strings(&["template/renovate.json"]))
        .expect_err("a tree without the root renovate.json fails");
    assert!(err.contains("renovate.json missing"), "{err}");
    let err = select_config_files(strings(&["tests/fixtures/renovate/renovate.json"]))
        .expect_err("fixtures do not count as the root config");
    assert!(err.contains("renovate.json missing"), "{err}");
}
