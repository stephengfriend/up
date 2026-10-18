//! Renovate config check (SUPPLY-06): automerge must be off everywhere.
//!
//! Renovate and CI may open dependency PRs but never merge or sign them: a
//! bump reaches hosts only through a human-signed commit (TRUST-07, CHG-11).
//! [`check_renovate_config`] is the pure rule set; [`run`] loads every tracked
//! config with the strict loader, so a duplicate key cannot hide a value, and
//! then validates each against Renovate's vendored JSON Schema.

use serde_json::Value;

use std::process::Command;

use crate::check::{CheckResult, Ctx, Failure, tracked_files};
use crate::python_tools;

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

/// A preset can enable automerge (`:automergeAll`, `config:best-practices`
/// variants, `local>` configs), so only [`REVIEWED_PRESETS`] may be extended.
fn check_presets(extends: &Value, path: &str, found: &mut Vec<String>) {
    let single;
    let items: &[Value] = match extends {
        Value::Array(items) => items,
        other => {
            single = [other.clone()];
            &single
        }
    };
    for (i, item) in items.iter().enumerate() {
        let item_path = if extends.is_array() {
            format!("{path}/{i}")
        } else {
            path.to_string()
        };
        match item.as_str() {
            Some(name) if REVIEWED_PRESETS.contains(&name) => {}
            Some(name) => found.push(format!(
                "SUPPLY-06: preset \"{}\" at {item_path} is not on the reviewed list (presets can enable automerge)",
                pointer_token(name)
            )),
            None => found.push(format!(
                "SUPPLY-06: {item_path}: extends entry is not a string"
            )),
        }
    }
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
                if key == "extends" {
                    check_presets(child, &child_path, found);
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

fn is_config_path(path: &str) -> bool {
    !path.starts_with("tests/fixtures/")
        && (path == ROOT_CONFIG || path.ends_with("/renovate.json"))
}

/// The tracked Renovate configs to check: every `renovate.json` outside
/// `tests/fixtures/`, sorted. The root one is required, so the template copy
/// is enforced the moment it is added (Phase 20) and the check can never pass
/// over zero configs.
pub fn select_config_files(
    tracked: impl IntoIterator<Item = String>,
) -> Result<Vec<String>, String> {
    let mut files: Vec<String> = tracked.into_iter().filter(|p| is_config_path(p)).collect();
    files.sort();
    files.dedup();
    if !files.iter().any(|p| p == ROOT_CONFIG) {
        return Err("renovate-check: renovate.json missing".to_string());
    }
    Ok(files)
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    run_with(ctx, python_tools::ensure_venv)
}

/// [`run`] with the venv installation injected, so tests can fail it.
pub fn run_with(
    ctx: &Ctx,
    setup: impl FnOnce(&Ctx) -> Result<std::path::PathBuf, Failure>,
) -> CheckResult {
    let files = select_config_files(tracked_files(&ctx.root)?).map_err(Failure::one)?;
    let mut problems = Vec::new();
    for file in &files {
        match up_config::yaml::load_file(&ctx.root.join(file)) {
            Ok(doc) => problems.extend(
                check_renovate_config(&doc)
                    .into_iter()
                    .map(|m| with_file(file, &m)),
            ),
            Err(e) => problems.push(format!("SUPPLY-06: {file}: {e}")),
        }
    }

    // Schema validation runs even after rule findings, so one run shows both.
    let bin_dir = setup(ctx)?;
    let mut cmd: Command = python_tools::tool_command(&bin_dir, "check-jsonschema");
    cmd.args(["--builtin-schema", "vendor.renovate"])
        .args(&files)
        .current_dir(&ctx.root);
    match cmd.status() {
        Ok(status) if status.success() => {}
        Ok(_) => problems.push("renovate-check: schema findings above".to_string()),
        Err(e) => problems.push(format!("renovate-check: cannot run check-jsonschema: {e}")),
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(problems))
    }
}
