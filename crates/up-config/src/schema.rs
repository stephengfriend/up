//! JSON Schema validation of configuration files (ARCH-05, FND-03).
//!
//! The schemas under `schema/` are embedded with `include_str!` and compiled from a
//! local registry. Nothing is read from disk or fetched at run time: the validator is
//! built with retrieval switched off, so a `$ref` to anything that is not embedded
//! fails to compile.
//!
//! Violations name the JSON pointer of the offending value and the failed keyword.
//! They never carry the rejected value, and the pointer itself is escaped to a short
//! printable ASCII form because object keys are attacker-influenced text.

use std::fmt::{self, Write as _};
use std::sync::OnceLock;

use jsonschema::{Draft, PatternOptions, Registry, Validator};
use serde_json::Value;

use crate::yaml::{self, YamlError};

/// The configuration file kinds that have a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Host,
}

const ALL_KINDS: &[Kind] = &[Kind::Host];

impl Kind {
    /// Number of kinds; sizes the per-kind validator cache.
    const COUNT: usize = ALL_KINDS.len();

    /// Every kind, in a stable order.
    pub fn all() -> &'static [Kind] {
        ALL_KINDS
    }

    /// The schema base name: the file is `schema/<base>.schema.json`.
    pub fn base(&self) -> &'static str {
        match self {
            Kind::Host => "host",
        }
    }

    /// The schema `$id`: `urn:up:schema:<base>`.
    pub fn id(&self) -> &'static str {
        match self {
            Kind::Host => "urn:up:schema:host",
        }
    }

    fn index(&self) -> usize {
        match self {
            Kind::Host => 0,
        }
    }
}

/// `$id` of the shared definitions resource.
pub const DEFS_ID: &str = "urn:up:schema:defs";

/// One failed check: where, and which keyword. Never the rejected value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// JSON pointer to the offending value; empty for the document root.
    pub pointer: String,
    /// The failed schema keyword, such as `pattern` or `additionalProperties`.
    pub keyword: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pointer = if self.pointer.is_empty() {
            "/"
        } else {
            &self.pointer
        };
        write!(f, "{pointer}: {}", self.keyword)
    }
}

/// Why a document was refused.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error(transparent)]
    Yaml(#[from] YamlError),
    #[error("{}", render_invalid(origin, violations))]
    Invalid {
        origin: String,
        violations: Vec<Violation>,
    },
}

fn render_invalid(origin: &str, violations: &[Violation]) -> String {
    let mut out = format!("{} does not match its schema", escape_ascii(origin, 256));
    for v in violations {
        let _ = write!(out, "\n  {v}");
    }
    out
}

/// File name and JSON text of every embedded schema, in a stable order.
pub fn embedded_sources() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "defs.schema.json",
            include_str!("../../../schema/defs.schema.json"),
        ),
        (
            "host.schema.json",
            include_str!("../../../schema/host.schema.json"),
        ),
    ]
}

/// Escape to printable ASCII and cap the length, so error text cannot carry control
/// characters, line breaks or lookalikes from attacker-influenced keys.
fn escape_ascii(text: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if out.len() >= max {
            out.push_str("...");
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-' | '/' | '~' | '@' | ' ') {
            out.push(ch);
        } else {
            let _ = write!(out, "\\u{{{:x}}}", ch as u32);
        }
    }
    out
}

/// The registry holding every embedded schema, keyed by `$id`.
fn registry() -> Result<&'static Registry<'static>, String> {
    static REGISTRY: OnceLock<Result<Registry<'static>, String>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let mut registry = Registry::new();
            for (file, text) in embedded_sources() {
                let value: Value = serde_json::from_str(text)
                    .map_err(|e| format!("embedded schema {file} is not JSON: {e}"))?;
                let id = value
                    .get("$id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("embedded schema {file} has no $id"))?
                    .to_string();
                registry = registry
                    .add(id, value)
                    .map_err(|e| format!("embedded schema {file} cannot be registered: {e}"))?;
            }
            registry
                .prepare()
                .map_err(|e| format!("embedded schemas cannot be prepared: {e}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Build a validator for `schema` against the embedded registry only.
///
/// Retrieval is switched off and patterns use the linear-time regex engine, which has no
/// lookaround and no backtracking blowup.
fn build(schema: &Value) -> Result<Validator, String> {
    let registry = registry()?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(registry)
        .offline()
        .with_pattern_options(PatternOptions::regex())
        .build(schema)
        .map_err(|e| format!("schema does not compile: {e}"))
}

/// Compile `schema` the way embedded schemas are compiled; `Err` names why it cannot be.
///
/// A `$ref` to anything outside the embedded registry is an error here, never a fetch.
pub fn compile_check(schema: &Value) -> Result<(), String> {
    build(schema).map(|_| ())
}

fn validator(kind: Kind) -> Result<&'static Validator, String> {
    static CACHE: [OnceLock<Result<Validator, String>>; Kind::COUNT] =
        [const { OnceLock::new() }; Kind::COUNT];
    CACHE[kind.index()]
        .get_or_init(|| {
            let root = serde_json::json!({ "$ref": kind.id() });
            build(&root)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn violation(pointer: &str, keyword: &str) -> Violation {
    Violation {
        pointer: escape_ascii(pointer, 256),
        keyword: keyword.to_string(),
    }
}

fn finish(mut violations: Vec<Violation>) -> Vec<Violation> {
    violations.sort();
    violations.dedup();
    violations
}

/// Validate a parsed document against the schema for `kind`.
///
/// The error list is sorted and never contains the rejected values.
pub fn validate(kind: Kind, instance: &Value) -> Result<(), Vec<Violation>> {
    let validator = match validator(kind) {
        Ok(v) => v,
        Err(_) => return Err(vec![violation("", "schema-unavailable")]),
    };
    let violations: Vec<Violation> = validator
        .iter_errors(instance)
        .map(|e| violation(e.instance_path().as_str(), e.kind().keyword()))
        .collect();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(finish(violations))
    }
}

/// Load `src` with the strict YAML loader, then validate it against `kind`.
pub fn validate_str(kind: Kind, src: &str, origin: &str) -> Result<(), SchemaError> {
    let value = yaml::load_str(src, origin)?;
    validate(kind, &value).map_err(|violations| SchemaError::Invalid {
        origin: origin.to_string(),
        violations,
    })
}

/// Check a schema document against the draft 2020-12 meta-schema.
pub fn check_meta(schema: &Value) -> Result<(), Vec<Violation>> {
    let violations: Vec<Violation> = jsonschema::draft202012::meta::validator()
        .iter_errors(schema)
        .map(|e| violation(e.instance_path().as_str(), e.kind().keyword()))
        .collect();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(finish(violations))
    }
}
