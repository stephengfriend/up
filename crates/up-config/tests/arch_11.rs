//! ARCH-11: strict YAML loading (anchors, aliases, merge keys, tags, duplicate keys, limits).

use std::path::PathBuf;

use up_config::yaml::{self, YamlError};

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/yaml")
        .join(rel)
}

#[test]
fn test_arch_11_plain_mapping_loads() {
    let value = yaml::load_file(&fixture("valid/plain.yml")).expect("plain.yml must load");
    let engine = value.get("engine").expect("engine key");
    assert!(engine.is_object());
    assert_eq!(engine["settings"]["retries"], 3);
    assert_eq!(value["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["label"], "quoted value");
}

#[test]
fn test_arch_11_anchor_rejected() {
    let err = yaml::load_file(&fixture("invalid/anchor.yml")).expect_err("anchor must be refused");
    assert!(matches!(err, YamlError::Anchor { .. }), "got {err:?}");
    assert_eq!(err.kind(), "anchor");
    let shown = err.to_string();
    assert!(shown.contains("/engine/settings"), "path missing: {shown}");
    assert!(shown.contains("line"), "line missing: {shown}");
}
