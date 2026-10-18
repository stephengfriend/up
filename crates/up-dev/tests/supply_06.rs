//! SUPPLY-06: Renovate and CI may open dependency PRs but never merge or sign
//! them, so the Renovate config must keep automerge off. Fixtures live in
//! `tests/fixtures/renovate/`; the expected outcome of each is in the tests
//! because JSON has no comments.

use std::path::PathBuf;

use up_dev::renovate_check::check_renovate_config;

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
    up_config::yaml::load_file(path)
        .unwrap_or_else(|e| panic!("{} must load: {e}", path.display()))
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
