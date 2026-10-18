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

fn refused(rel: &str) -> YamlError {
    yaml::load_file(&fixture(rel)).expect_err("fixture must be refused")
}

#[test]
fn test_arch_11_alias_rejected() {
    let err = refused("invalid/alias.yml");
    assert!(matches!(err.kind(), "anchor" | "alias"), "got {err:?}");
}

#[test]
fn test_arch_11_alias_without_anchor_rejected() {
    let err = yaml::load_str("copy: *missing\n", "inline").expect_err("alias must be refused");
    assert!(matches!(err.kind(), "alias" | "syntax"), "got {err:?}");
}

#[test]
fn test_arch_11_merge_key_rejected() {
    let err = refused("invalid/merge-inline.yml");
    assert_eq!(err.kind(), "merge-key", "got {err:?}");
    assert!(err.to_string().contains("/derived"), "{err}");
    let aliased = refused("invalid/merge-alias.yml");
    assert!(
        matches!(aliased.kind(), "anchor" | "merge-key" | "alias"),
        "got {aliased:?}"
    );
}

#[test]
fn test_arch_11_merge_key_quoted_rejected() {
    let err = yaml::load_str("derived:\n  \"<<\": {a: 1}\n", "inline").expect_err("merge key");
    assert_eq!(err.kind(), "merge-key", "got {err:?}");
}

#[test]
fn test_arch_11_tag_rejected() {
    for rel in ["invalid/tag-core.yml", "invalid/tag-custom.yml"] {
        let err = refused(rel);
        assert_eq!(err.kind(), "tag", "{rel}: got {err:?}");
        assert!(err.to_string().contains("/value"), "{rel}: {err}");
    }
}

#[test]
fn test_arch_11_tag_text_not_echoed() {
    let err = yaml::load_str("value: !custom 12\n", "inline").expect_err("tag");
    assert_eq!(err.kind(), "tag", "got {err:?}");
    let shown = err.to_string();
    assert!(!shown.contains("custom"), "{shown}");
    assert!(shown.contains("line 1, column 8"), "tag position: {shown}");
}

#[test]
fn test_arch_11_duplicate_key_rejected_adjacent() {
    assert_eq!(
        refused("invalid/duplicate-adjacent.yml").kind(),
        "duplicate-key"
    );
}

#[test]
fn test_arch_11_duplicate_key_rejected_separated() {
    assert_eq!(
        refused("invalid/duplicate-separated.yml").kind(),
        "duplicate-key"
    );
}

#[test]
fn test_arch_11_duplicate_key_rejected_nested() {
    let err = refused("invalid/duplicate-nested.yml");
    assert_eq!(err.kind(), "duplicate-key", "got {err:?}");
    let shown = err.to_string();
    assert!(shown.contains("/engine"), "parent path missing: {shown}");
    assert!(
        shown.contains("line 5"),
        "second occurrence line missing: {shown}"
    );
}

#[test]
fn test_arch_11_duplicate_key_rejected_quoted_vs_plain() {
    assert_eq!(
        refused("invalid/duplicate-quoted-vs-plain.yml").kind(),
        "duplicate-key"
    );
}

#[test]
fn test_arch_11_same_key_in_different_mappings_loads() {
    let value = yaml::load_str("one:\n  commit: a\ntwo:\n  commit: b\n", "inline").expect("loads");
    assert_eq!(value["one"]["commit"], "a");
    assert_eq!(value["two"]["commit"], "b");
}

#[test]
fn test_arch_11_multiple_documents_rejected() {
    assert_eq!(
        refused("invalid/multi-doc.yml").kind(),
        "multiple-documents"
    );
}

#[test]
fn test_arch_11_empty_document_rejected() {
    assert_eq!(refused("invalid/empty.yml").kind(), "empty");
    assert_eq!(refused("invalid/null-doc.yml").kind(), "empty");
    let err = yaml::load_str("", "inline").expect_err("empty string");
    assert_eq!(err.kind(), "empty", "got {err:?}");
    let blank = yaml::load_str("   \n\n", "inline").expect_err("blank string");
    assert_eq!(blank.kind(), "empty", "got {blank:?}");
}

#[test]
fn test_arch_11_root_must_be_mapping() {
    assert_eq!(
        refused("invalid/root-sequence.yml").kind(),
        "root-not-mapping"
    );
    let scalar = yaml::load_str("just text\n", "inline").expect_err("scalar root");
    assert_eq!(scalar.kind(), "root-not-mapping", "got {scalar:?}");
}

#[test]
fn test_arch_11_non_string_key_rejected() {
    assert_eq!(
        refused("invalid/non-string-key.yml").kind(),
        "non-string-key"
    );
}

#[test]
fn test_arch_11_depth_limit() {
    let err = refused("invalid/deep.yml");
    assert_eq!(err.kind(), "limit", "got {err:?}");
}

fn nested(depth: usize) -> String {
    format!("{}1{}\n", "{a: ".repeat(depth), "}".repeat(depth))
}

#[test]
fn test_arch_11_depth_at_limit_loads() {
    // The root mapping is depth 1, so 64 nested mappings is exactly the limit.
    assert!(yaml::load_str(&nested(64), "inline").is_ok());
    let err = yaml::load_str(&nested(65), "inline").expect_err("one level too deep");
    assert_eq!(err.kind(), "limit", "got {err:?}");
}

#[test]
fn test_arch_11_size_limit() {
    let src = format!("key: {}\n", "x".repeat(5 * 1024 * 1024));
    let err = yaml::load_str(&src, "inline").expect_err("oversized input");
    assert_eq!(err.kind(), "limit", "got {err:?}");
}

#[test]
fn test_arch_11_node_limit() {
    let limits = yaml::Limits {
        max_nodes: 10,
        ..yaml::Limits::default()
    };
    let src: String = (0..20).map(|i| format!("k{i}: v\n")).collect();
    let err = yaml::load_str_with(&src, "inline", &limits).expect_err("too many nodes");
    assert_eq!(err.kind(), "limit", "got {err:?}");
}

#[test]
fn test_arch_11_scalar_limit() {
    let limits = yaml::Limits {
        max_scalar_bytes: 8,
        ..yaml::Limits::default()
    };
    let err = yaml::load_str_with("key: 0123456789\n", "inline", &limits).expect_err("scalar");
    assert_eq!(err.kind(), "limit", "got {err:?}");
}

#[test]
fn test_arch_11_yaml11_forms_stay_strings() {
    let value = yaml::load_file(&fixture("valid/yaml11-forms.yml")).expect("loads");
    for key in ["a", "b", "c", "d"] {
        assert!(value[key].is_string(), "{key} was {:?}", value[key]);
    }
    assert_eq!(value["a"], "yes");
    assert_eq!(value["d"], "off");
}

#[test]
fn test_arch_11_error_escapes_control_characters() {
    // The parent key contains U+001B; the duplicate sits inside that mapping.
    let src = "\"x\\e\":\n  a: 1\n  a: 2\n";
    let err = yaml::load_str(src, "inline").expect_err("duplicate key");
    assert_eq!(err.kind(), "duplicate-key", "got {err:?}");
    let shown = err.to_string();
    assert!(shown.contains("\\u{001B}"), "escape missing: {shown}");
    assert!(shown.is_ascii(), "non-ASCII in error: {shown}");
    assert!(
        !shown.chars().any(char::is_control),
        "raw control byte in: {shown:?}"
    );
}

#[test]
fn test_arch_11_error_escapes_non_ascii_origin() {
    let err = yaml::load_str("", "caf\u{e9}\u{7}.yml").expect_err("empty");
    let shown = err.to_string();
    assert!(shown.is_ascii(), "{shown}");
    assert!(!shown.chars().any(char::is_control), "{shown:?}");
}

#[test]
fn test_arch_11_error_never_includes_scalar_values() {
    let err = yaml::load_str("token: &t hunter2\n", "inline").expect_err("anchor");
    assert!(!err.to_string().contains("hunter2"), "{err}");
    let dup = yaml::load_str("a: hunter2\na: hunter3\n", "inline").expect_err("duplicate");
    let shown = dup.to_string();
    assert!(!shown.contains("hunter"), "{shown}");
}

#[test]
fn test_arch_11_from_str_deserializes_into_type() {
    #[derive(Debug, serde::Deserialize)]
    struct Cfg {
        name: String,
    }
    let cfg: Cfg = yaml::from_str("name: fleet\n", "inline").expect("typed load");
    assert_eq!(cfg.name, "fleet");
    let err = yaml::from_str::<Cfg>("name: [a]\n", "inline").expect_err("wrong shape");
    assert_eq!(err.kind(), "deserialize", "got {err:?}");
}

#[test]
fn test_arch_11_load_file_missing_is_io_error() {
    let err = yaml::load_file(&fixture("valid/does-not-exist.yml")).expect_err("missing file");
    assert_eq!(err.kind(), "io", "got {err:?}");
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Fast,
    Slow,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    #[allow(dead_code)]
    level: u32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    #[allow(dead_code)]
    mode: Mode,
    #[allow(dead_code)]
    settings: Inner,
}

/// ARCH-11: a typed-deserialization failure names where and what kind of error it is,
/// and never copies scalar or key text from the source.
#[test]
fn test_arch_11_deserialize_error_does_not_echo_values() {
    let cases: [(&str, &str, &str, &str); 5] = [
        (
            "mode: hunter2variant\nsettings:\n  level: 1\n",
            "hunter2variant",
            "unknown variant",
            "/mode",
        ),
        (
            "mode: fast\nsettings:\n  level: 1\n  hunter2key: 2\n",
            "hunter2key",
            "unknown field",
            "/settings",
        ),
        (
            "mode: fast\nsettings:\n  level: hunter2value\n",
            "hunter2value",
            "invalid",
            "/settings/level",
        ),
        (
            "mode: fast\nhunter2key: 1\nsettings:\n  level: 1\n",
            "hunter2key",
            "unknown field",
            "",
        ),
        ("mode: fast\n", "hunter2", "missing field", ""),
    ];
    for (src, token, category, path) in cases {
        let err = yaml::from_str::<Probe>(src, "inline").expect_err("must not deserialize");
        assert_eq!(err.kind(), "deserialize", "got {err:?}");
        let shown = err.to_string();
        let debug = format!("{err:?}");
        assert!(!shown.contains(token), "Display echoes {token}: {shown}");
        assert!(!debug.contains(token), "Debug echoes {token}: {debug}");
        assert!(
            shown.contains(category),
            "category `{category}` missing: {shown}"
        );
        if !path.is_empty() {
            assert!(shown.contains(path), "path `{path}` missing: {shown}");
        }
        assert!(shown.contains("line"), "position missing: {shown}");
    }
    // Wrong container type: a sequence where a scalar belongs.
    let err = yaml::from_str::<Probe>("mode: [hunter2variant]\nsettings:\n  level: 1\n", "inline")
        .expect_err("wrong shape");
    assert!(!err.to_string().contains("hunter2"), "{err}");
    assert!(!format!("{err:?}").contains("hunter2"), "{err:?}");
}

// ---- scalar non-string keys (ARCH-11 MUST, CR-02) ----

/// Plain scalars that PyYAML (YAML 1.1) or the YAML 1.2 core schema resolve to a
/// non-string: booleans, null, integers, floats and timestamps.
const NON_STRING_PLAIN_KEYS: &[&str] = &[
    // booleans, YAML 1.1 and 1.2 spellings in any case
    "yes",
    "Yes",
    "YES",
    "no",
    "No",
    "NO",
    "y",
    "Y",
    "n",
    "N",
    "on",
    "On",
    "ON",
    "off",
    "Off",
    "OFF",
    "true",
    "True",
    "TRUE",
    "false",
    "False",
    "FALSE",
    "yEs",
    "tRuE",
    // null
    "null",
    "Null",
    "NULL",
    "~",
    "",
    // integers
    "0",
    "1",
    "01",
    "007",
    "09",
    "-1",
    "+1",
    "0x1",
    "0X1F",
    "0b101",
    "0o17",
    "1_000",
    "1__0",
    "1:30",
    "190:20:30",
    // floats
    "1.5",
    "-1.5",
    ".5",
    "1.",
    "1e3",
    "1.5e+3",
    "1_0.5",
    "1:30.5",
    ".inf",
    "-.inf",
    "+.inf",
    ".Inf",
    ".INF",
    ".nan",
    ".NaN",
    ".NAN",
    // timestamps
    "2024-01-01",
    "2024-1-1",
    "2024-01-01T10:20:30Z",
    "2024-01-01 10:20:30 +5",
];

#[test]
fn test_arch_11_scalar_non_string_keys_rejected() {
    for key in NON_STRING_PLAIN_KEYS {
        let src = format!("{key}: value\n");
        let err = yaml::load_str(&src, "inline")
            .expect_err(&format!("plain key {key:?} must be refused"));
        assert_eq!(err.kind(), "non-string-key", "{key:?}: got {err:?}");
        // The message is the same for every key, so none of the key text can be in it.
        assert_eq!(
            err.to_string(),
            "inline: non-string key at / (line 1, column 1)",
            "{key:?}"
        );
    }
}

#[test]
fn test_arch_11_scalar_non_string_keys_rejected_nested_and_flow() {
    let nested =
        yaml::load_str("engine:\n  settings:\n    true: a\n", "inline").expect_err("nested");
    assert_eq!(nested.kind(), "non-string-key", "got {nested:?}");
    assert!(nested.to_string().contains("/engine/settings"), "{nested}");
    for src in [
        "{1: a}\n",
        "{yes: a}\n",
        "outer: {null: a}\n",
        "list:\n  - 0x1: a\n",
    ] {
        let err = yaml::load_str(src, "inline").expect_err(src);
        assert_eq!(err.kind(), "non-string-key", "{src:?}: got {err:?}");
    }
}

#[test]
fn test_arch_11_scalar_key_fixtures_rejected() {
    for rel in [
        "invalid/scalar-bool-key.yml",
        "invalid/scalar-bool-collision.yml",
        "invalid/scalar-int-key.yml",
        "invalid/scalar-int-collision.yml",
        "invalid/scalar-null-key.yml",
        "invalid/scalar-float-key.yml",
    ] {
        assert_eq!(refused(rel).kind(), "non-string-key", "{rel}");
    }
}

/// `yes`/`true` and `1`/`01` collapse to one key under PyYAML; the loader must stop at the
/// first of them rather than report two distinct keys.
#[test]
fn test_arch_11_pyyaml_key_collision_is_refused() {
    for src in ["yes: a\ntrue: b\n", "1: a\n01: b\n", "on: a\nTrue: b\n"] {
        let err = yaml::load_str(src, "inline").expect_err(src);
        assert_eq!(err.kind(), "non-string-key", "{src:?}: got {err:?}");
    }
}

#[test]
fn test_arch_11_quoted_and_string_shaped_keys_load() {
    let value = yaml::load_file(&fixture("valid/quoted-scalar-keys.yml")).expect("loads");
    assert_eq!(value["1"], "one");
    assert_eq!(value["yes"], "affirmative");
    assert_eq!(value["null"], "nothing");
    assert_eq!(value["0x1"], "hex");
    assert_eq!(value[""], "empty");
    assert_eq!(value["1.2.3"], "version");
    assert_eq!(value["v1"], "prefixed");
    assert_eq!(value["10-20"], "range");
    assert_eq!(value["x"], "coordinate");
    for src in [
        "yess: a\n",
        "nul: a\n",
        "_1: a\n",
        "1_x: a\n",
        "0x: a\n",
        "0xZ: a\n",
        "1e: a\n",
        ".: a\n",
        "..: a\n",
        ".i: a\n",
        "1.2.3.4: a\n",
        "2024-01-01x: a\n",
        "on_: a\n",
        "-: a\n",
    ] {
        yaml::load_str(src, "inline").unwrap_or_else(|e| panic!("{src:?} must load: {e}"));
    }
}

/// The repository's own engine YAML must keep loading under the stricter profile.
#[test]
fn test_arch_11_engine_yaml_files_still_load() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for rel in [
        "defaults.yml",
        "versions.yml",
        ".yamllint.yml",
        ".github/zizmor.yml",
    ] {
        yaml::load_file(&root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    }
}

/// GitHub workflows are not `up`-parsed (not in ARCH-11's file list) and always have a
/// top-level `on:`. The default profile refuses it; the workflow-specific entry point
/// accepts exactly that root key and nothing else.
#[test]
fn test_arch_11_workflow_on_key_only_through_explicit_root_allowance() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ci = root.join(".github/workflows/ci.yml");
    let err = yaml::load_file(&ci).expect_err("default profile refuses `on:`");
    assert_eq!(err.kind(), "non-string-key", "got {err:?}");
    let doc = yaml::load_file_allowing_root_keys(&ci, &["on"]).expect("allowed root key");
    assert!(doc.get("on").is_some());
    // Only the named key, only at the root.
    let other = yaml::load_str_allowing_root_keys("on: a\nyes: b\n", "inline", &["on"])
        .expect_err("other bool key");
    assert_eq!(other.kind(), "non-string-key", "got {other:?}");
    let nested = yaml::load_str_allowing_root_keys("jobs:\n  on: a\n", "inline", &["on"])
        .expect_err("nested key");
    assert_eq!(nested.kind(), "non-string-key", "got {nested:?}");
}
