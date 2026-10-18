//! Reading the spec tree: design documents and the requirement bullets inside them.
//!
//! Everything here is read-only. Fenced code blocks are skipped, so headings
//! and bullets inside a fence are never seen as structure.

use std::path::Path;

use crate::check::Failure;

/// One requirement bullet: `- **PFX-NN** text`.
#[derive(Debug, Clone)]
pub struct Requirement {
    pub id: String,
    /// Path relative to the tree root, for example `docs/design/cli.md`.
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct DesignDoc {
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone)]
pub struct Tree {
    /// Design documents sorted by path.
    pub docs: Vec<DesignDoc>,
}

/// Read every `<docs>/design/*.md`, sorted by file name.
pub fn read_tree(docs_dir: &Path) -> Result<Tree, Failure> {
    let design = docs_dir.join("design");
    let entries = std::fs::read_dir(&design).map_err(|e| {
        Failure::one(format!(
            "up-dev spec-check: cannot read {}: {e}",
            design.display()
        ))
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| {
                Failure::one(format!(
                    "up-dev spec-check: cannot read {}: {e}",
                    design.display()
                ))
            })?
            .path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut docs = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Failure::one(format!(
                "up-dev spec-check: cannot read {}: {e}",
                path.display()
            ))
        })?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let rel = format!("docs/design/{}", name.unwrap_or_default());
        docs.push(parse_design(&rel, &text));
    }
    Ok(Tree { docs })
}

/// Index of the first line after the front matter (0 when there is none).
fn body_start(lines: &[&str]) -> usize {
    if lines.first().is_some_and(|l| l.trim_end() == "---")
        && let Some(close) = lines
            .iter()
            .skip(1)
            .position(|l| l.trim_end() == "---")
            .map(|p| p + 1)
    {
        return close + 1;
    }
    0
}

/// The marker character of a fence line (three or more backticks or tildes).
fn fence_marker(line: &str) -> Option<char> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some('`')
    } else if t.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// `- **PFX-NN**` at column 0 gives the ID text between the asterisks.
fn bullet_id(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- **")?;
    let (id, _) = rest.split_once("**")?;
    let (prefix, digits) = id.split_once('-')?;
    let well_formed = !prefix.is_empty()
        && prefix.bytes().all(|b| b.is_ascii_uppercase())
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit());
    well_formed.then_some(id)
}

pub fn parse_design(path: &str, text: &str) -> DesignDoc {
    let lines: Vec<&str> = text.lines().collect();
    let start = body_start(&lines);
    let mut fence: Option<char> = None;
    let mut in_requirements = false;
    let mut requirements = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start) {
        if let Some(marker) = fence_marker(line) {
            match fence {
                None => fence = Some(marker),
                Some(open) if open == marker => fence = None,
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        if line.starts_with("# ") || line.starts_with("## ") {
            in_requirements = line.trim_end() == "## Requirements";
            continue;
        }
        if in_requirements && let Some(id) = bullet_id(line) {
            requirements.push(Requirement {
                id: id.to_string(),
                file: path.to_string(),
                line: index + 1,
            });
        }
    }
    DesignDoc { requirements }
}
