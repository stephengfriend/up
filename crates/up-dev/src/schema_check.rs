//! Schema check (ARCH-05, ARCH-11, FND-03): strict YAML loading, JSON Schema validation and hostile-value fixtures.
//!
//! This stage loads the engine files `up` reads with the strict loader, runs the
//! golden YAML corpus under `tests/fixtures/yaml/`, then checks the JSON Schemas
//! themselves (meta-schema, `$id`, the D1-10 unknown-key walk, `schema_version`)
//! and the golden fixtures under `tests/fixtures/schema/<kind>/{valid,invalid}`.
//! Each step is its own function that pushes messages.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use up_config::schema::{self, Kind, SchemaError};
use up_config::yaml;

use crate::check::{CheckResult, Ctx, Failure};

/// Engine files the `up` binary reads; each must load under the strict profile.
const ENGINE_FILES: &[&str] = &["defaults.yml", "versions.yml"];

const VALID_DIR: &str = "tests/fixtures/yaml/valid";
const INVALID_DIR: &str = "tests/fixtures/yaml/invalid";
const EXPECT_PREFIX: &str = "# expect: ";

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let mut problems = Vec::new();
    check_engine_files(&ctx.root, &mut problems);
    check_corpus(&ctx.root, &mut problems);
    check_schema_files(&ctx.root, &mut problems);
    check_schema_fixtures(&ctx.root, &mut problems);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(problems))
    }
}

/// Strict-load every engine YAML file.
fn check_engine_files(root: &Path, problems: &mut Vec<String>) {
    for rel in ENGINE_FILES {
        if let Err(e) = yaml::load_file(&root.join(rel)) {
            problems.push(format!("ARCH-11: {rel}: {}", e.detail()));
        }
    }
}

/// Every `*.yml` file directly under `dir`, sorted. A missing directory is an error.
fn yml_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot read directory: {e}"))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read directory: {e}"))?
            .path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yml") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Run the golden corpus: valid fixtures load, invalid ones fail with the kind in their header.
fn check_corpus(root: &Path, problems: &mut Vec<String>) {
    check_valid_fixtures(root, problems);
    check_invalid_fixtures(root, problems);
}

fn check_valid_fixtures(root: &Path, problems: &mut Vec<String>) {
    let files = match yml_files(&root.join(VALID_DIR)) {
        Ok(files) => files,
        Err(msg) => {
            problems.push(format!("ARCH-11: {VALID_DIR}: {msg}"));
            return;
        }
    };
    if files.is_empty() {
        problems.push(format!("ARCH-11: {VALID_DIR}: no *.yml fixtures found"));
    }
    for path in files {
        if let Err(e) = yaml::load_file(&path) {
            problems.push(format!(
                "ARCH-11: {}: valid fixture was refused: {}",
                rel(root, &path),
                e.detail()
            ));
        }
    }
}

fn check_invalid_fixtures(root: &Path, problems: &mut Vec<String>) {
    let files = match yml_files(&root.join(INVALID_DIR)) {
        Ok(files) => files,
        Err(msg) => {
            problems.push(format!("ARCH-11: {INVALID_DIR}: {msg}"));
            return;
        }
    };
    if files.is_empty() {
        problems.push(format!("ARCH-11: {INVALID_DIR}: no *.yml fixtures found"));
    }
    for path in files {
        let name = rel(root, &path);
        let expected = match expected_kind(&path) {
            Ok(kind) => kind,
            Err(msg) => {
                problems.push(format!("ARCH-11: {name}: {msg}"));
                continue;
            }
        };
        match yaml::load_file(&path) {
            Ok(_) => problems.push(format!(
                "ARCH-11: {name}: invalid fixture was accepted (expected {expected})"
            )),
            Err(e) if e.kind() == expected => {}
            Err(e) => problems.push(format!(
                "ARCH-11: {name}: expected {expected}, got {}: {}",
                e.kind(),
                e.detail()
            )),
        }
    }
}

/// The kind named on the first line, `# expect: <kind>`.
fn expected_kind(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read fixture: {e}"))?;
    let first = text.lines().next().unwrap_or("");
    match first.strip_prefix(EXPECT_PREFIX) {
        Some(kind) if !kind.trim().is_empty() => Ok(kind.trim().to_string()),
        _ => Err(format!(
            "first line must be `{}<kind>`",
            EXPECT_PREFIX.trim_end()
        )),
    }
}

const SCHEMA_DIR: &str = "schema";
const SCHEMA_SUFFIX: &str = ".schema.json";
const FIXTURE_ROOT: &str = "tests/fixtures/schema";
const DEFS_BASE: &str = "defs";

/// The base name of `<base>.schema.json`.
fn schema_base(file: &str) -> Option<&str> {
    file.strip_suffix(SCHEMA_SUFFIX)
}

/// Schemas on disk and embedded must be the same set; each must be a valid draft
/// 2020-12 schema with the expected `$id`, no object that allows unknown keys
/// (D1-10) and, apart from the shared definitions, a required `schema_version` of 1.
fn check_schema_files(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join(SCHEMA_DIR);
    let mut on_disk = BTreeSet::new();
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if schema_base(&name).is_some() {
                    on_disk.insert(name);
                }
            }
        }
        Err(e) => {
            problems.push(format!("ARCH-05: {SCHEMA_DIR}: cannot read directory: {e}"));
            return;
        }
    }
    let embedded: BTreeSet<String> = schema::embedded_sources()
        .iter()
        .map(|(file, _)| (*file).to_string())
        .collect();
    for file in on_disk.difference(&embedded) {
        problems.push(format!(
            "ARCH-05: {SCHEMA_DIR}/{file}: not embedded; add it to up_config::schema::embedded_sources"
        ));
    }
    for file in embedded.difference(&on_disk) {
        problems.push(format!(
            "ARCH-05: {SCHEMA_DIR}/{file}: embedded but missing on disk"
        ));
    }
    for (file, text) in schema::embedded_sources() {
        check_one_schema(file, text, problems);
    }
}

fn check_one_schema(file: &str, text: &str, problems: &mut Vec<String>) {
    let name = format!("{SCHEMA_DIR}/{file}");
    let Some(base) = schema_base(file) else {
        problems.push(format!(
            "ARCH-05: {name}: file name must end in {SCHEMA_SUFFIX}"
        ));
        return;
    };
    let doc: Value = match serde_json::from_str(text) {
        Ok(doc) => doc,
        Err(e) => {
            problems.push(format!("ARCH-05: {name}: not valid JSON: {e}"));
            return;
        }
    };
    if let Err(violations) = schema::check_meta(&doc) {
        for v in violations {
            problems.push(format!(
                "ARCH-05: {name}: fails the draft 2020-12 meta-schema at {v}"
            ));
        }
    }
    let want_id = format!("urn:up:schema:{base}");
    if doc.get("$id").and_then(Value::as_str) != Some(want_id.as_str()) {
        problems.push(format!("ARCH-05: {name}: `$id` must be `{want_id}`"));
    }
    for path in open_objects(&doc) {
        problems.push(format!(
            "ARCH-05: {name}: object at {path} allows unknown keys (D1-10)"
        ));
    }
    if base != DEFS_BASE {
        check_schema_version(&name, &doc, problems);
    }
}

/// A top-level schema requires `schema_version` and constrains it to `const: 1`.
fn check_schema_version(name: &str, doc: &Value, problems: &mut Vec<String>) {
    let required = doc
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|r| r.iter().any(|k| k == "schema_version"));
    if !required {
        problems.push(format!(
            "ARCH-05: {name}: `schema_version` must be required"
        ));
    }
    let constant = doc
        .pointer("/properties/schema_version/const")
        .and_then(Value::as_i64);
    if constant != Some(1) {
        problems.push(format!(
            "ARCH-05: {name}: `schema_version` must be constrained with `const: 1`"
        ));
    }
}

/// Keywords whose value is a map of name to subschema.
const SCHEMA_MAPS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
];
/// Keywords whose value is one subschema.
const SCHEMA_SINGLE: &[&str] = &[
    "additionalProperties",
    "unevaluatedProperties",
    "items",
    "unevaluatedItems",
    "contains",
    "propertyNames",
    "not",
    "if",
    "then",
    "else",
];
/// Keywords whose value is a list of subschemas.
const SCHEMA_LISTS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];

/// JSON paths of every object schema that allows unknown keys (D1-10).
///
/// An object schema is one with `type: object` or with `properties`. It is closed when
/// `additionalProperties` is `false`, or when it is a map: `propertyNames` plus an
/// object-valued `additionalProperties`.
pub fn open_objects(schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(schema, "#", &mut found);
    found
}

fn walk(node: &Value, path: &str, found: &mut Vec<String>) {
    let Some(obj) = node.as_object() else {
        return;
    };
    let is_object_type = match obj.get("type") {
        Some(Value::String(t)) => t == "object",
        Some(Value::Array(ts)) => ts.iter().any(|t| t == "object"),
        _ => false,
    };
    if is_object_type || obj.contains_key("properties") {
        let closed = obj.get("additionalProperties") == Some(&Value::Bool(false));
        let map = obj.contains_key("propertyNames")
            && obj
                .get("additionalProperties")
                .is_some_and(Value::is_object);
        if !closed && !map {
            found.push(path.to_string());
        }
    }
    for key in SCHEMA_MAPS {
        if let Some(children) = obj.get(*key).and_then(Value::as_object) {
            for (name, child) in children {
                walk(child, &format!("{path}/{key}/{name}"), found);
            }
        }
    }
    for key in SCHEMA_SINGLE {
        if let Some(child) = obj.get(*key) {
            walk(child, &format!("{path}/{key}"), found);
        }
    }
    for key in SCHEMA_LISTS {
        if let Some(children) = obj.get(*key).and_then(Value::as_array) {
            for (i, child) in children.iter().enumerate() {
                walk(child, &format!("{path}/{key}/{i}"), found);
            }
        }
    }
}

/// Golden fixtures for every kind: valid ones validate, invalid ones fail with the
/// violation (or YAML refusal) named in their `# expect:` header.
fn check_schema_fixtures(root: &Path, problems: &mut Vec<String>) {
    let known: BTreeSet<&str> = Kind::all().iter().map(Kind::base).collect();
    if let Ok(entries) = std::fs::read_dir(root.join(FIXTURE_ROOT)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && !known.contains(name.as_str()) {
                problems.push(format!(
                    "ARCH-05: {FIXTURE_ROOT}/{name}: fixtures for a kind that has no schema"
                ));
            }
        }
    }
    for kind in Kind::all() {
        let base = format!("{FIXTURE_ROOT}/{}", kind.base());
        let valid = fixture_files(root, &format!("{base}/valid"), problems);
        let invalid = fixture_files(root, &format!("{base}/invalid"), problems);
        if valid.is_empty() || invalid.is_empty() {
            problems.push(format!(
                "ARCH-05: {base}: needs at least one valid and one invalid fixture"
            ));
        }
        for path in valid {
            check_valid_schema_fixture(root, *kind, &path, problems);
        }
        for path in invalid {
            check_invalid_schema_fixture(root, *kind, &path, problems);
        }
    }
}

fn fixture_files(root: &Path, rel_dir: &str, problems: &mut Vec<String>) -> Vec<PathBuf> {
    match yml_files(&root.join(rel_dir)) {
        Ok(files) => files,
        Err(msg) => {
            problems.push(format!("ARCH-05: {rel_dir}: {msg}"));
            Vec::new()
        }
    }
}

fn check_valid_schema_fixture(root: &Path, kind: Kind, path: &Path, problems: &mut Vec<String>) {
    let name = rel(root, path);
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            problems.push(format!("ARCH-05: {name}: cannot read fixture: {e}"));
            return;
        }
    };
    if let Err(e) = schema::validate_str(kind, &src, &name) {
        problems.push(format!("ARCH-05: {name}: valid fixture was refused: {e}"));
    }
}

fn check_invalid_schema_fixture(root: &Path, kind: Kind, path: &Path, problems: &mut Vec<String>) {
    let name = rel(root, path);
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            problems.push(format!("ARCH-05: {name}: cannot read fixture: {e}"));
            return;
        }
    };
    let expected = match expected_kind(path) {
        Ok(expected) => expected,
        Err(msg) => {
            problems.push(format!("ARCH-05: {name}: {msg}"));
            return;
        }
    };
    let result = schema::validate_str(kind, &src, &name);
    if let Some(yaml_kind) = expected.strip_prefix("yaml ") {
        match result {
            Err(SchemaError::Yaml(e)) if e.kind() == yaml_kind.trim() => {}
            Err(e) => problems.push(format!(
                "ARCH-05: {name}: expected YAML refusal `{}`, got: {e}",
                yaml_kind.trim()
            )),
            Ok(()) => problems.push(format!(
                "ARCH-05: {name}: invalid fixture was accepted (expected {expected})"
            )),
        }
        return;
    }
    let mut parts = expected.split_whitespace();
    let (Some(pointer), Some(keyword), None) = (parts.next(), parts.next(), parts.next()) else {
        problems.push(format!(
            "ARCH-05: {name}: header must be `# expect: <pointer> <keyword>` or `# expect: yaml <kind>`"
        ));
        return;
    };
    // `/` names the document root, whose JSON pointer is the empty string.
    let pointer = if pointer == "/" { "" } else { pointer };
    match result {
        Ok(()) => problems.push(format!(
            "ARCH-05: {name}: invalid fixture was accepted (expected {expected})"
        )),
        Err(SchemaError::Yaml(e)) => problems.push(format!(
            "ARCH-05: {name}: expected {expected}, got a YAML refusal: {}",
            e.kind()
        )),
        Err(SchemaError::Invalid { violations, .. }) => {
            if !violations
                .iter()
                .any(|v| v.pointer == pointer && v.keyword == keyword)
            {
                let got: Vec<String> = violations.iter().map(ToString::to_string).collect();
                problems.push(format!(
                    "ARCH-05: {name}: expected {expected}, got {}",
                    got.join("; ")
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_arch_05_walk_flags_open_object() {
        let schema = json!({
            "type": "object",
            "properties": { "a": { "type": "object", "properties": {} } },
            "additionalProperties": false
        });
        assert_eq!(open_objects(&schema), vec!["#/properties/a".to_string()]);
    }

    #[test]
    fn test_arch_05_walk_flags_object_without_additional_properties() {
        let schema = json!({ "type": "object", "properties": { "a": { "type": "string" } } });
        assert_eq!(open_objects(&schema), vec!["#".to_string()]);
    }

    #[test]
    fn test_arch_05_walk_flags_additional_properties_true() {
        let schema = json!({ "type": "object", "additionalProperties": true });
        assert_eq!(open_objects(&schema), vec!["#".to_string()]);
    }

    #[test]
    fn test_arch_05_walk_flags_map_without_property_names() {
        let schema = json!({ "type": "object", "additionalProperties": { "type": "string" } });
        assert_eq!(open_objects(&schema), vec!["#".to_string()]);
    }

    #[test]
    fn test_arch_05_walk_accepts_closed_and_map_objects() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "m": {
                    "type": "object",
                    "propertyNames": { "pattern": "^[a-z]+$" },
                    "additionalProperties": { "type": "string" }
                },
                "list": { "type": "array", "items": { "type": "object", "additionalProperties": false } },
                "type": { "type": "string" }
            }
        });
        assert!(open_objects(&schema).is_empty());
    }

    #[test]
    fn test_arch_05_walk_descends_into_defs_and_combinators() {
        let schema = json!({
            "$defs": { "d": { "type": "object" } },
            "allOf": [ { "type": "object" } ]
        });
        assert_eq!(
            open_objects(&schema),
            vec!["#/$defs/d".to_string(), "#/allOf/0".to_string()]
        );
    }
}
