//! Renovate config check (SUPPLY-06): automerge must be off everywhere.
//!
//! Renovate and CI may open dependency PRs but never merge or sign them: a
//! bump reaches hosts only through a human-signed commit (TRUST-07, CHG-11).
//! [`check_renovate_config`] is the pure rule set; [`run`] loads the tracked
//! config with the strict loader so a duplicate key cannot hide a value.

use serde_json::Value;

use crate::check::{CheckResult, Ctx, Failure};

/// Presets an engine or template config may extend; anything else can switch
/// automerge on behind the check's back.
pub const REVIEWED_PRESETS: &[&str] = &[
    "config:recommended",
    "helpers:pinGitHubActionDigestsToSemver",
];

/// Keys that must be present and `false` at the top level.
const REQUIRED_OFF: &[&str] = &["automerge", "platformAutomerge"];

/// Keys that must never be `true` at any depth.
const NEVER_TRUE: &[&str] = &["automerge", "platformAutomerge", "autoApprove"];

/// Escape one JSON Pointer token (RFC 6901) and any control character, so a
/// hostile key cannot garble the terminal.
fn pointer_token(token: &str) -> String {
    let mut out = String::new();
    for c in token.chars() {
        match c {
            '~' => out.push_str("~0"),
            '/' => out.push_str("~1"),
            c if c.is_control() => out.extend(c.escape_default()),
            c => out.push(c),
        }
    }
    out
}

fn walk(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}/{}", pointer_token(key));
                if NEVER_TRUE.contains(&key.as_str()) {
                    match child {
                        Value::Bool(true) => {
                            found.push(format!("SUPPLY-06: {child_path}: {key} is true"));
                        }
                        Value::Bool(false) => {}
                        _ => found.push(format!("SUPPLY-06: {child_path}: {key} is not a boolean")),
                    }
                }
                walk(child, &child_path, found);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                walk(child, &format!("{path}/{i}"), found);
            }
        }
        _ => {}
    }
}

/// Problems in one parsed Renovate config, each starting with `SUPPLY-06: `.
/// Empty means the config passes.
pub fn check_renovate_config(doc: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let Some(map) = doc.as_object() else {
        found.push("SUPPLY-06: renovate config has to be a JSON object".to_string());
        return found;
    };
    for key in REQUIRED_OFF {
        if !map.contains_key(*key) {
            found.push(format!(
                "SUPPLY-06: renovate config has to set {key}: false at the top level"
            ));
        }
    }
    walk(doc, "", &mut found);
    found
}

const ROOT_CONFIG: &str = "renovate.json";

/// `SUPPLY-06: <message>` becomes `SUPPLY-06: <file>: <message>`.
fn with_file(file: &str, message: &str) -> String {
    let rest = message.strip_prefix("SUPPLY-06: ").unwrap_or(message);
    format!("SUPPLY-06: {file}: {rest}")
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let path = ctx.root.join(ROOT_CONFIG);
    if !path.is_file() {
        return Err(Failure::one("renovate-check: renovate.json missing"));
    }
    let doc = up_config::yaml::load_file(&path)
        .map_err(|e| Failure::one(format!("SUPPLY-06: {ROOT_CONFIG}: {e}")))?;
    let found = check_renovate_config(&doc);
    if found.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(
            found
                .into_iter()
                .map(|m| with_file(ROOT_CONFIG, &m))
                .collect(),
        ))
    }
}
