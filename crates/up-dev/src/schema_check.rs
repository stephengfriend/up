//! Schema check (ARCH-05, ARCH-11, FND-03): strict YAML loading, JSON Schema validation and hostile-value fixtures.
//!
//! This stage loads the engine files `up` reads with the strict loader and runs
//! the golden YAML corpus under `tests/fixtures/yaml/`. Plan 01-06 appends the
//! JSON Schema steps; each step is its own function that pushes messages.

use std::path::{Path, PathBuf};

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
