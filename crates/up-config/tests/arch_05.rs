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
fn hostile_values() -> Vec<Named> {
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

/// How a hostile value is injected at a row's pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Replace the value at the pointer.
    Value,
    /// Rename the map key named by the last pointer segment; the violation is at the map.
    Key,
}

/// What the field is allowed to contain.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    /// Patterned: every hostile value is refused.
    Patterned,
    /// Free single-line text matched only in memory: line-breaking, NUL and empty values are
    /// refused, while `../x`, `a;b` and lookalikes are legitimate content.
    SingleLine,
}

/// One string field: where it lives in a valid base fixture of `kind`.
struct Row {
    kind: Kind,
    base: &'static str,
    pointer: &'static str,
    mode: Mode,
    content: Content,
}

/// Every string field. Later tasks append rows, never rewrite the tests.
fn rows() -> Vec<Row> {
    let row = |kind, base, pointer, mode, content| Row {
        kind,
        base,
        pointer,
        mode,
        content,
    };
    let val = |kind, base, pointer| row(kind, base, pointer, Mode::Value, Content::Patterned);
    let key = |kind, base, pointer| row(kind, base, pointer, Mode::Key, Content::Patterned);
    let line = |kind, base, pointer| row(kind, base, pointer, Mode::Value, Content::SingleLine);
    const HOST: &str = "valid/with-roles.yml";
    const UP: &str = "valid/full.yml";
    const CATALOG: &str = "valid/all-backends.yml";
    const ANSWERS: &str = "valid/full.yml";
    const GUARD: &str = "valid/denylist.yml";
    vec![
        val(Kind::Host, HOST, "/name"),
        val(Kind::Host, HOST, "/instance"),
        val(Kind::Host, HOST, "/host"),
        val(Kind::Host, HOST, "/user"),
        val(Kind::Host, HOST, "/roles/0"),
        val(Kind::Up, UP, "/name"),
        val(Kind::Up, UP, "/remote"),
        val(Kind::Up, UP, "/visibility"),
        val(Kind::Up, UP, "/engine/url"),
        val(Kind::Up, UP, "/engine/version"),
        val(Kind::Up, UP, "/engine/commit"),
        val(Kind::Up, UP, "/vault/default"),
        val(Kind::Up, UP, "/vault/classes/servers"),
        key(Kind::Up, UP, "/vault/classes/servers"),
        val(Kind::Up, UP, "/taps/0"),
        val(Kind::Up, UP, "/mise_backends/0"),
        val(Kind::Up, UP, "/notify/webhook"),
        val(Kind::Up, UP, "/notify/heartbeat"),
        val(Kind::Up, UP, "/privacy/denylist"),
        val(Kind::Up, UP, "/secrets/root_backend"),
        val(Kind::Up, UP, "/secrets/user_backend"),
        val(Kind::Up, UP, "/secrets/classes/servers/root_backend"),
        key(Kind::Up, UP, "/secrets/classes/servers"),
        key(Kind::Catalog, CATALOG, "/secrets/smtp_password"),
        val(Kind::Catalog, CATALOG, "/secrets/smtp_password/op"),
        val(Kind::Catalog, CATALOG, "/secrets/smtp_password/bw"),
        val(Kind::Catalog, CATALOG, "/secrets/smtp_password/bws"),
        val(Kind::Catalog, CATALOG, "/secrets/smtp_password/sops"),
        line(Kind::Catalog, CATALOG, "/secrets/smtp_password/fake"),
        val(Kind::Answers, ANSWERS, "/name"),
        val(Kind::Answers, ANSWERS, "/login"),
        val(Kind::Answers, ANSWERS, "/visibility"),
        val(Kind::Answers, ANSWERS, "/secrets/backends/0"),
        val(Kind::Answers, ANSWERS, "/vault/default"),
        val(Kind::Answers, ANSWERS, "/vault/classes/servers"),
        key(Kind::Answers, ANSWERS, "/vault/classes/servers"),
        val(Kind::Answers, ANSWERS, "/signers/0/principal"),
        val(Kind::Answers, ANSWERS, "/signers/0/key"),
        val(Kind::Answers, ANSWERS, "/signers/0/role"),
        val(Kind::Answers, ANSWERS, "/host/alias"),
        val(Kind::Answers, ANSWERS, "/host/user"),
        val(Kind::Answers, ANSWERS, "/host/roles/0"),
        line(Kind::Guard, GUARD, "/denylist/0"),
    ]
}

/// The document with the hostile value applied at the row's pointer, with the pointer and
/// keyword the violation must carry (`None` keyword: any).
fn mutate(row: &Row, hostile: &str) -> (Value, String, Option<&'static str>) {
    let mut doc = load(row.kind, row.base);
    match row.mode {
        Mode::Value => {
            *doc.pointer_mut(row.pointer)
                .unwrap_or_else(|| panic!("no {} in {}", row.pointer, row.base)) =
                Value::String(hostile.to_string());
            (doc, row.pointer.to_string(), None)
        }
        Mode::Key => {
            let (parent, key) = row.pointer.rsplit_once('/').expect("key pointer");
            let map = doc
                .pointer_mut(parent)
                .and_then(Value::as_object_mut)
                .unwrap_or_else(|| panic!("no map at {parent}"));
            let value = map.remove(key).expect("key present");
            map.insert(hostile.to_string(), value);
            (doc, parent.to_string(), Some("propertyNames"))
        }
    }
}

/// A named hostile value.
type Named = (&'static str, String);

/// Values that are refused for this row, and values that must still validate.
fn split_hostile(row: &Row) -> (Vec<Named>, Vec<Named>) {
    let (refused, legit): (Vec<_>, Vec<_>) = hostile_values().into_iter().partition(|(what, _)| {
        row.content == Content::Patterned
            || matches!(
                *what,
                "embedded newline" | "nul" | "line separator" | "trailing newline" | "empty"
            )
    });
    (refused, legit)
}

/// Every hostile value is refused with a violation naming the row's field.
fn assert_hostile_rejected(kind: Kind) {
    let mut checked = 0;
    for row in rows().iter().filter(|r| r.kind == kind) {
        let (refused, legit) = split_hostile(row);
        for (what, hostile) in refused {
            let (doc, pointer, keyword) = mutate(row, &hostile);
            let violations = schema::validate(row.kind, &doc)
                .err()
                .unwrap_or_else(|| panic!("{what} accepted at {}", row.pointer));
            assert!(
                violations
                    .iter()
                    .any(|v| v.pointer == pointer && keyword.is_none_or(|k| v.keyword == k)),
                "{what} at {}: no violation naming it, got {:?}",
                row.pointer,
                pointers(&violations)
            );
            checked += 1;
        }
        for (what, value) in legit {
            let (doc, _, _) = mutate(row, &value);
            if let Err(v) = schema::validate(row.kind, &doc) {
                panic!(
                    "{what} is legitimate free text at {} but was refused: {:?}",
                    row.pointer,
                    pointers(&v)
                );
            }
        }
    }
    assert!(checked > 0, "no rows for {}", kind.base());
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
    assert_hostile_rejected(Kind::Host);
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

fn assert_valid_fixtures(kind: Kind, names: &[&str]) {
    for name in names {
        let doc = load(kind, name);
        if let Err(v) = schema::validate(kind, &doc) {
            panic!("{name} must validate: {:?}", pointers(&v));
        }
    }
}

fn violations_of(kind: Kind, rel: &str) -> Vec<Violation> {
    schema::validate(kind, &load(kind, rel)).expect_err(&format!("{rel} must be refused"))
}

#[test]
fn test_arch_05_up_valid_fixtures_pass() {
    assert_valid_fixtures(Kind::Up, &["valid/full.yml", "valid/minimal.yml"]);
}

#[test]
fn test_arch_05_catalog_valid_fixtures_pass() {
    assert_valid_fixtures(Kind::Catalog, &["valid/all-backends.yml"]);
}

#[test]
fn test_arch_05_hostile_values_rejected_up() {
    assert_hostile_rejected(Kind::Up);
}

#[test]
fn test_arch_05_hostile_values_rejected_catalog() {
    assert_hostile_rejected(Kind::Catalog);
}

#[test]
fn test_arch_05_literal_webhook_url_rejected() {
    // OPS-03: a literal URL in notify.webhook is refused; only a catalog reference passes.
    let violations = violations_of(Kind::Up, "invalid/literal-webhook-url.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/notify/webhook" && v.keyword == "pattern"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_plaintext_denylist_rejected() {
    // GRD-11: privacy.denylist is the path of a sops file, never a plaintext list.
    let violations = violations_of(Kind::Up, "invalid/plaintext-denylist.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/privacy/denylist" && v.keyword == "type"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_human_session_root_backend_rejected() {
    // SEC-02: a human-session backend (bw) cannot resolve secrets for root.
    let violations = violations_of(Kind::Up, "invalid/human-session-root-backend.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/secrets/root_backend" && v.keyword == "enum"),
        "got {:?}",
        pointers(&violations)
    );
    // The same refusal holds for a per-class override.
    let mut doc = load(Kind::Up, "valid/full.yml");
    doc["secrets"]["classes"]["servers"]["root_backend"] = json!("bw");
    let violations = schema::validate(Kind::Up, &doc).expect_err("bw refused for a class");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/secrets/classes/servers/root_backend"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_unknown_nested_key_rejected() {
    let violations = violations_of(Kind::Up, "invalid/unknown-nested-key.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/engine" && v.keyword == "additionalProperties"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_mirror_accepts_only_empty_object() {
    let mut doc = load(Kind::Up, "valid/full.yml");
    doc["mirror"] = json!({ "apt": "https://mirror.example.com" });
    let violations = schema::validate(Kind::Up, &doc).expect_err("mirror keys refused in v1");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/mirror" && v.keyword == "additionalProperties"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_rotation_days_bounds() {
    let mut doc = load(Kind::Up, "valid/full.yml");
    for ok in [json!(1), json!(90), json!(365)] {
        doc["rotation_days"] = ok.clone();
        schema::validate(Kind::Up, &doc).unwrap_or_else(|v| panic!("{ok} refused: {v:?}"));
    }
    for bad in [
        json!(0),
        json!(366),
        json!(-1),
        json!(1.5),
        json!("90"),
        json!(null),
    ] {
        doc["rotation_days"] = bad.clone();
        let violations = schema::validate(Kind::Up, &doc)
            .err()
            .unwrap_or_else(|| panic!("{bad} accepted"));
        assert!(
            violations.iter().any(|v| v.pointer == "/rotation_days"),
            "{bad}: got {:?}",
            pointers(&violations)
        );
    }
}

#[test]
fn test_arch_05_lists_reject_duplicates() {
    let mut doc = load(Kind::Up, "valid/full.yml");
    doc["taps"] = json!(["example-login/tools", "example-login/tools"]);
    let violations = schema::validate(Kind::Up, &doc).expect_err("duplicate taps refused");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/taps" && v.keyword == "uniqueItems"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_catalog_entry_needs_a_backend() {
    let violations = violations_of(Kind::Catalog, "invalid/empty-entry.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/secrets/smtp_password" && v.keyword == "minProperties"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_catalog_fake_length_boundary() {
    let mut doc = load(Kind::Catalog, "valid/all-backends.yml");
    doc["secrets"]["smtp_password"]["fake"] = Value::String("x".repeat(256));
    schema::validate(Kind::Catalog, &doc).expect("256 characters validate");
    doc["secrets"]["smtp_password"]["fake"] = Value::String("x".repeat(257));
    let violations = schema::validate(Kind::Catalog, &doc).expect_err("257 refused");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/secrets/smtp_password/fake" && v.keyword == "maxLength"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_answers_valid_fixtures_pass() {
    assert_valid_fixtures(Kind::Answers, &["valid/minimal.yml", "valid/full.yml"]);
}

#[test]
fn test_arch_05_guard_valid_fixtures_pass() {
    assert_valid_fixtures(Kind::Guard, &["valid/denylist.yml"]);
}

#[test]
fn test_arch_05_hostile_values_rejected_answers() {
    assert_hostile_rejected(Kind::Answers);
}

#[test]
fn test_arch_05_hostile_values_rejected_guard() {
    // ../x, a;b and a lookalike are legitimate denylist content; assert_hostile_rejected
    // checks both that they validate and that line-breaking, NUL and empty values do not.
    assert_hostile_rejected(Kind::Guard);
}

#[test]
fn test_arch_05_guard_denylist_length_bounds() {
    let mut doc = load(Kind::Guard, "valid/denylist.yml");
    for (len, ok) in [(2, false), (3, true), (256, true), (257, false)] {
        doc["denylist"] = json!(["x".repeat(len)]);
        let result = schema::validate(Kind::Guard, &doc);
        assert_eq!(result.is_ok(), ok, "{len} characters: {result:?}");
        if let Err(violations) = result {
            assert!(
                violations.iter().any(|v| v.pointer == "/denylist/0"),
                "got {:?}",
                pointers(&violations)
            );
        }
    }
}

#[test]
fn test_arch_05_answers_requires_two_signers() {
    let violations = violations_of(Kind::Answers, "invalid/one-signer.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/signers" && v.keyword == "minItems"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_answers_requires_one_primary_and_a_backup() {
    let mut doc = load(Kind::Answers, "valid/minimal.yml");
    let primary = doc["signers"][0].clone();
    let backup = doc["signers"][1].clone();
    let cases = [
        ("two primaries", vec![primary.clone(), primary.clone()]),
        ("two backups", vec![backup.clone(), backup.clone()]),
    ];
    for (what, signers) in cases {
        doc["signers"] = Value::Array(signers);
        let violations = schema::validate(Kind::Answers, &doc)
            .err()
            .unwrap_or_else(|| panic!("{what} accepted"));
        assert!(
            violations.iter().any(|v| v.pointer == "/signers"),
            "{what}: got {:?}",
            pointers(&violations)
        );
    }
}

#[test]
fn test_arch_05_answers_rejects_secret_value_field() {
    // PUB-05: the answers file never carries secret values.
    let violations = violations_of(Kind::Answers, "invalid/secret-value-field.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer.is_empty() && v.keyword == "additionalProperties"),
        "got {:?}",
        pointers(&violations)
    );
}

#[test]
fn test_arch_05_guard_rejects_multiline_entry() {
    let violations = violations_of(Kind::Guard, "invalid/multiline-entry.yml");
    assert!(
        violations
            .iter()
            .any(|v| v.pointer == "/denylist/0" && v.keyword == "pattern"),
        "got {:?}",
        pointers(&violations)
    );
}

/// Walks the embedded schemas, resolving `$ref`s inside the embedded set only.
struct Walker {
    docs: std::collections::BTreeMap<String, Value>,
}

impl Walker {
    fn new() -> Walker {
        let docs = schema::embedded_sources()
            .iter()
            .map(|(_, text)| {
                let doc: Value = serde_json::from_str(text).expect("embedded schema is JSON");
                (doc["$id"].as_str().expect("$id").to_string(), doc)
            })
            .collect();
        Walker { docs }
    }

    /// Follow `$ref` links from `node` (found in document `doc`) to the target schema.
    fn resolve<'a>(&'a self, doc: &'a str, node: &'a Value) -> (&'a str, &'a Value) {
        let (mut doc, mut node) = (doc, node);
        while let Some(reference) = node.get("$ref").and_then(Value::as_str) {
            let (id, pointer) = reference.split_once('#').unwrap_or((reference, ""));
            let (target_id, target) = if id.is_empty() {
                (doc, &self.docs[doc])
            } else {
                let (key, value) = self
                    .docs
                    .get_key_value(id)
                    .unwrap_or_else(|| panic!("$ref to a schema that is not embedded: {id}"));
                (key.as_str(), value)
            };
            doc = target_id;
            node = if pointer.is_empty() {
                target
            } else {
                target
                    .pointer(pointer)
                    .unwrap_or_else(|| panic!("dangling $ref {reference}"))
            };
        }
        (doc, node)
    }

    fn is_string(node: &Value) -> bool {
        node.get("type").and_then(Value::as_str) == Some("string")
    }

    /// Normalised paths of every string-typed value: array items and map values are `*`,
    /// and the names of a map's keys are `<map>/{key}`.
    fn string_paths(&self, kind: Kind) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let root = &self.docs[kind.id()];
        self.collect(kind.id(), root, "", &mut out);
        out
    }

    fn collect(
        &self,
        doc: &str,
        node: &Value,
        path: &str,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        let (doc, node) = self.resolve(doc, node);
        if Self::is_string(node) {
            out.insert(path.to_string());
        }
        if let Some(props) = node.get("properties").and_then(Value::as_object) {
            for (name, child) in props {
                self.collect(doc, child, &format!("{path}/{name}"), out);
            }
        }
        if let Some(items) = node.get("items") {
            self.collect(doc, items, &format!("{path}/*"), out);
        }
        if let Some(values) = node.get("additionalProperties").filter(|v| v.is_object()) {
            self.collect(doc, values, &format!("{path}/*"), out);
            if let Some(names) = node.get("propertyNames") {
                self.collect(doc, names, &format!("{path}/{{key}}"), out);
            }
        }
    }

    /// The normalised path a row's concrete pointer covers.
    fn row_path(&self, row: &Row) -> String {
        let pointer = match row.mode {
            Mode::Value => row.pointer,
            Mode::Key => row.pointer.rsplit_once('/').expect("key pointer").0,
        };
        let root = &self.docs[row.kind.id()];
        let (mut doc, mut node) = (row.kind.id(), root);
        let mut path = String::new();
        for segment in pointer.split('/').skip(1) {
            let (d, n) = self.resolve(doc, node);
            doc = d;
            let next = if let Some(child) = n.pointer(&format!("/properties/{segment}")) {
                path.push_str(&format!("/{segment}"));
                child
            } else if let Some(items) = n.get("items") {
                path.push_str("/*");
                items
            } else if let Some(values) = n.get("additionalProperties").filter(|v| v.is_object()) {
                path.push_str("/*");
                values
            } else {
                panic!(
                    "row {} does not follow the schema at {segment}",
                    row.pointer
                );
            };
            node = next;
        }
        if row.mode == Mode::Key {
            path.push_str("/{key}");
        }
        path
    }
}

#[test]
fn test_arch_05_every_string_field_covered() {
    let walker = Walker::new();
    let rows = rows();
    let mut total = 0;
    for kind in Kind::all() {
        let covered: std::collections::BTreeSet<String> = rows
            .iter()
            .filter(|r| r.kind == *kind)
            .map(|r| walker.row_path(r))
            .collect();
        let fields = walker.string_paths(*kind);
        total += fields.len();
        let missing: Vec<&String> = fields.difference(&covered).collect();
        assert!(
            missing.is_empty(),
            "{}: string fields without a hostile-value row: {missing:?}",
            kind.base()
        );
        let extra: Vec<&String> = covered.difference(&fields).collect();
        assert!(
            extra.is_empty(),
            "{}: rows that are not string fields: {extra:?}",
            kind.base()
        );
    }
    assert!(
        total >= 40,
        "only {total} string fields found; walker is broken"
    );
}

#[test]
fn test_arch_05_embedded_set_is_closed() {
    // Every $ref in every embedded schema resolves inside the embedded set.
    let walker = Walker::new();
    for kind in Kind::all() {
        let _ = walker.string_paths(*kind);
    }
}
