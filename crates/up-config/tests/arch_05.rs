//! ARCH-05: JSON Schemas with anchored value patterns, hostile values rejected with the field named.

use std::path::PathBuf;

use serde_json::{Value, json};
use up_config::schema::{self, Kind, SchemaError, Violation};
use up_config::yaml;

fn fixture(kind: Kind, rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/schema")
        .join(kind.base())
        .join(rel)
}

fn load(kind: Kind, rel: &str) -> Value {
    yaml::load_file(&fixture(kind, rel)).unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

/// Hostile values every patterned field must refuse (ARCH-05 negative AC; encoding and empty edges).
fn hostile_values() -> Vec<(&'static str, String)> {
    vec![
        ("path traversal", "../x".to_string()),
        ("command separator", "a;b".to_string()),
        ("embedded newline", "a\nb".to_string()),
        ("cyrillic lookalike", "n\u{0430}me".to_string()),
        ("nul", "a\0b".to_string()),
        ("line separator", "a\u{2028}b".to_string()),
        ("trailing newline", "abc\n".to_string()),
        ("empty", String::new()),
    ]
}

/// One patterned field: where it lives in a valid base fixture of `kind`.
struct Row {
    kind: Kind,
    base: &'static str,
    pointer: &'static str,
}

/// Every patterned string field. Later tasks append rows, never rewrite the tests.
fn rows() -> Vec<Row> {
    let v = |kind, base, pointer| Row {
        kind,
        base,
        pointer,
    };
    vec![
        v(Kind::Host, "valid/with-roles.yml", "/name"),
        v(Kind::Host, "valid/with-roles.yml", "/instance"),
        v(Kind::Host, "valid/with-roles.yml", "/host"),
        v(Kind::Host, "valid/with-roles.yml", "/user"),
        v(Kind::Host, "valid/with-roles.yml", "/roles/0"),
    ]
}

/// The document with the hostile value applied at the row's pointer.
fn mutate(row: &Row, hostile: &str) -> Value {
    let mut doc = load(row.kind, row.base);
    *doc.pointer_mut(row.pointer)
        .unwrap_or_else(|| panic!("no {} in {}", row.pointer, row.base)) =
        Value::String(hostile.to_string());
    doc
}

fn pointers(violations: &[Violation]) -> Vec<String> {
    violations.iter().map(ToString::to_string).collect()
}

#[test]
fn test_arch_05_host_valid_fixtures_pass() {
    for name in ["valid/minimal.yml", "valid/with-roles.yml"] {
        let doc = load(Kind::Host, name);
        if let Err(v) = schema::validate(Kind::Host, &doc) {
            panic!("{name} must validate: {:?}", pointers(&v));
        }
    }
}

#[test]
fn test_arch_05_hostile_values_rejected_host() {
    for row in rows().iter().filter(|r| r.kind == Kind::Host) {
        for (what, hostile) in hostile_values() {
            let doc = mutate(row, &hostile);
            let violations = schema::validate(row.kind, &doc)
                .expect_err(&format!("{what} accepted at {}", row.pointer));
            assert!(
                violations.iter().any(|v| v.pointer == row.pointer),
                "{what} at {}: no violation naming it, got {:?}",
                row.pointer,
                pointers(&violations)
            );
        }
    }
}

#[test]
fn test_arch_05_name_length_boundary() {
    let mut doc = load(Kind::Host, "valid/minimal.yml");
    doc["name"] = Value::String("a".repeat(31));
    schema::validate(Kind::Host, &doc).expect("31 characters validate");
    doc["name"] = Value::String("a".repeat(32));
    let violations = schema::validate(Kind::Host, &doc).expect_err("32 characters are refused");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/name" && v.keyword == "pattern"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_unknown_key_rejected_host() {
    let mut doc = load(Kind::Host, "valid/minimal.yml");
    doc["shell"] = json!("zsh");
    let violations = schema::validate(Kind::Host, &doc).expect_err("unknown key refused");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer.is_empty() && v.keyword == "additionalProperties"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_violation_does_not_echo_value() {
    let src = "schema_version: 1\nname: example\ninstance: https://github.com/example-login/example.git\nhost: host-01\nuser: \"a;b\"\n";
    let err = schema::validate_str(Kind::Host, src, "host.yml").expect_err("a;b refused");
    let SchemaError::Invalid { violations, .. } = &err else {
        panic!("expected a schema error, got {err:?}");
    };
    assert!(
        violations.iter().any(|v| v.pointer == "/user"),
        "got {:?}",
        pointers(violations)
    );
    for v in violations {
        assert!(
            !v.to_string().contains("a;b"),
            "violation echoes value: {v}"
        );
    }
    assert!(
        !err.to_string().contains("a;b"),
        "error echoes value: {err}"
    );
    assert!(err.to_string().contains("/user"), "pointer missing: {err}");
}

#[test]
fn test_arch_05_remote_ref_not_fetched() {
    let remote = json!({ "$ref": "https://example.invalid/x.json" });
    let err = schema::compile_check(&remote).expect_err("a remote $ref must not compile");
    assert!(!err.is_empty());
    // The fetch-capable resolver (jsonschema's resolve-http) is not built into the graph.
    let lock =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
            .expect("Cargo.lock");
    assert!(
        !lock.contains("name = \"reqwest\""),
        "reqwest is in the dependency graph; jsonschema must stay offline"
    );
}

#[test]
fn test_arch_05_patterns_ascii_anchored() {
    let (_, text) = schema::embedded_sources()
        .iter()
        .find(|(file, _)| *file == "defs.schema.json")
        .expect("defs embedded");
    let defs: Value = serde_json::from_str(text).expect("defs is JSON");
    let entries = defs["$defs"].as_object().expect("$defs");
    let mut patterns = 0;
    for (name, def) in entries {
        let Some(pattern) = def.get("pattern").and_then(Value::as_str) else {
            continue;
        };
        patterns += 1;
        assert!(pattern.starts_with('^'), "{name}: not anchored at start");
        assert!(pattern.ends_with('$'), "{name}: not anchored at end");
        assert!(pattern.is_ascii(), "{name}: not ASCII");
        for banned in ["(?=", "(?!", "(?<=", "(?<!"] {
            assert!(!pattern.contains(banned), "{name}: lookaround {banned}");
        }
    }
    assert!(patterns >= 20, "only {patterns} patterns found in defs");
}
