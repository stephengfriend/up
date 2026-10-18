//! Reading the spec tree: design documents, their front matter, the
//! requirement bullets inside them and the prose around them.
//!
//! Everything here is read-only. Fenced code blocks are skipped, so headings
//! and bullets inside a fence are never seen as structure.

use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use up_config::yaml;

use crate::check::Failure;

/// The five front matter fields every design document carries.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontMatter {
    pub title: String,
    pub status: String,
    pub prefix: String,
    pub milestones: Vec<String>,
    pub depends_on: Vec<String>,
}

/// What a document says about its requirement prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocPrefix {
    /// `prefix: none`.
    NoPrefix,
    /// An uppercase prefix such as `ARCH`.
    Known(String),
    /// Missing or invalid front matter: other rules must not guess.
    Unknown,
}

/// One `  - AC:` line.
#[derive(Debug, Clone)]
pub struct Ac {
    pub negative: bool,
}

/// One requirement bullet: `- **PFX-NN** text`, with its continuation lines.
#[derive(Debug, Clone)]
pub struct Requirement {
    pub id: String,
    /// The letters before the dash.
    pub prefix: String,
    /// Path relative to the tree root, for example `docs/design/cli.md`.
    pub file: String,
    pub line: usize,
    /// The bullet text after the ID plus every non-AC continuation line.
    pub text: String,
    pub acs: Vec<Ac>,
}

/// A line of prose: not front matter, not fenced, not part of a requirement.
#[derive(Debug, Clone)]
pub struct ProseLine {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct DesignDoc {
    pub path: String,
    /// File name without `.md`, as `depends_on` entries name it.
    pub stem: String,
    pub front: Result<FrontMatter, String>,
    pub prefix: DocPrefix,
    pub requirements: Vec<Requirement>,
    /// Malformed bullets inside `## Requirements`: (line, message).
    pub format_errors: Vec<(usize, String)>,
    /// Lines of every `## Requirements` heading.
    pub requirements_headings: Vec<usize>,
    pub prose: Vec<ProseLine>,
}

#[derive(Debug, Clone)]
pub struct Tree {
    /// Design documents sorted by path.
    pub docs: Vec<DesignDoc>,
    /// Text of `docs/roadmap.md`, when it exists.
    pub roadmap: Option<String>,
    /// Text of `docs/spec-exemptions.yml`, when it exists.
    pub exemptions: Option<String>,
}

fn read_error(path: &Path, e: &std::io::Error) -> Failure {
    Failure::one(format!(
        "up-dev spec-check: cannot read {}: {e}",
        path.display()
    ))
}

/// Read a file as text; `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_error(path, &e)),
    }
}

/// Read every `<docs>/design/*.md` (sorted by file name), the roadmap and the exemptions file.
pub fn read_tree(docs_dir: &Path) -> Result<Tree, Failure> {
    let design = docs_dir.join("design");
    let entries = std::fs::read_dir(&design).map_err(|e| read_error(&design, &e))?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| read_error(&design, &e))?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut docs = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|e| read_error(&path, &e))?;
        let stem = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        docs.push(parse_design(
            &format!("docs/design/{stem}.md"),
            &stem,
            &text,
        ));
    }
    Ok(Tree {
        docs,
        roadmap: read_optional(&docs_dir.join("roadmap.md"))?,
        exemptions: read_optional(&docs_dir.join("spec-exemptions.yml"))?,
    })
}

/// Index of the first line after the front matter, and the front matter text.
/// The opening `---` becomes a comment line, so line numbers in YAML errors are
/// file line numbers. `None` when the file does not start with a closed `---` block.
fn split_front_matter(lines: &[&str]) -> Option<(usize, String)> {
    if lines.first()?.trim_end() != "---" {
        return None;
    }
    let close = lines.iter().skip(1).position(|l| l.trim_end() == "---")? + 1;
    Some((close + 1, format!("#\n{}", lines[1..close].join("\n"))))
}

/// Index of the first line after the front matter (0 when there is none).
pub fn body_start(lines: &[&str]) -> usize {
    split_front_matter(lines).map_or(0, |(start, _)| start)
}

/// The marker character of a fence line (three or more backticks or tildes).
pub fn fence_marker(line: &str) -> Option<char> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some('`')
    } else if t.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// Track fenced code. Returns true when `line` is a fence line or inside a fence.
pub fn in_fence(fence: &mut Option<char>, line: &str) -> bool {
    if let Some(marker) = fence_marker(line) {
        match *fence {
            None => *fence = Some(marker),
            Some(open) if open == marker => *fence = None,
            Some(_) => {}
        }
        return true;
    }
    fence.is_some()
}

enum Bullet<'a> {
    /// `- **PFX-NN**` with a two-digit number: the ID, its prefix and the rest of the line.
    Requirement {
        id: &'a str,
        prefix: &'a str,
        rest: &'a str,
    },
    /// Anything else starting with `- `, with the reason it is not a requirement.
    Malformed(String),
}

fn classify_bullet(line: &str) -> Bullet<'_> {
    let Some(rest) = line.strip_prefix("- **") else {
        return Bullet::Malformed(
            "a bullet in ## Requirements must start with `- **PFX-NN**`".to_string(),
        );
    };
    let Some((id, after)) = rest.split_once("**") else {
        return Bullet::Malformed("the requirement ID is not closed by `**`".to_string());
    };
    let split = id.split_once('-').filter(|(prefix, digits)| {
        !prefix.is_empty()
            && prefix.bytes().all(|b| b.is_ascii_uppercase())
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
    });
    match split {
        None => Bullet::Malformed(format!(
            "`{id}` is not a requirement ID: expected uppercase letters, a dash and two digits"
        )),
        Some((_, digits)) if digits.len() != 2 => {
            Bullet::Malformed(format!("the number in `{id}` must have exactly two digits"))
        }
        Some((prefix, _)) => Bullet::Requirement {
            id,
            prefix,
            rest: after,
        },
    }
}

/// Classify a prefix word from the front matter.
fn doc_prefix(prefix: &str) -> DocPrefix {
    if prefix == "none" {
        DocPrefix::NoPrefix
    } else if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_uppercase()) {
        DocPrefix::Known(prefix.to_string())
    } else {
        DocPrefix::Unknown
    }
}

/// Parse the front matter strictly; when that fails, still try to read `prefix`
/// leniently so one bad field does not cascade into roadmap findings.
fn read_front_matter(path: &str, lines: &[&str]) -> (Result<FrontMatter, String>, DocPrefix) {
    let Some((_, yaml_text)) = split_front_matter(lines) else {
        return (
            Err("missing front matter: the file must start with a `---` block".to_string()),
            DocPrefix::Unknown,
        );
    };
    match yaml::from_str::<FrontMatter>(&yaml_text, path) {
        Ok(front) => {
            let prefix = doc_prefix(&front.prefix);
            (Ok(front), prefix)
        }
        Err(strict) => {
            let prefix = yaml::load_str(&yaml_text, path)
                .ok()
                .and_then(|v| v.get("prefix").and_then(|p| p.as_str()).map(doc_prefix))
                .unwrap_or(DocPrefix::Unknown);
            (Err(strict.detail()), prefix)
        }
    }
}

pub fn parse_design(path: &str, stem: &str, text: &str) -> DesignDoc {
    let lines: Vec<&str> = text.lines().collect();
    let (front, prefix) = read_front_matter(path, &lines);
    let mut doc = DesignDoc {
        path: path.to_string(),
        stem: stem.to_string(),
        front,
        prefix,
        requirements: Vec::new(),
        format_errors: Vec::new(),
        requirements_headings: Vec::new(),
        prose: Vec::new(),
    };
    let mut fence: Option<char> = None;
    let mut in_requirements = false;
    // Inside a requirement or a malformed bullet, indented lines belong to it.
    let mut open_requirement = false;
    let mut open_malformed = false;
    for (index, line) in lines.iter().enumerate().skip(body_start(&lines)) {
        let number = index + 1;
        if in_fence(&mut fence, line) {
            if !line.starts_with(' ') {
                open_requirement = false;
                open_malformed = false;
            }
            continue;
        }
        if line.starts_with("# ") || line.starts_with("## ") {
            in_requirements = line.trim_end() == "## Requirements";
            if in_requirements {
                doc.requirements_headings.push(number);
            }
            open_requirement = false;
            open_malformed = false;
            doc.prose.push(ProseLine {
                line: number,
                text: (*line).to_string(),
            });
            continue;
        }
        if !in_requirements {
            doc.prose.push(ProseLine {
                line: number,
                text: (*line).to_string(),
            });
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("- ") {
            match classify_bullet(line) {
                Bullet::Requirement { id, prefix, rest } => {
                    doc.requirements.push(Requirement {
                        id: id.to_string(),
                        prefix: prefix.to_string(),
                        file: path.to_string(),
                        line: number,
                        text: rest.trim().to_string(),
                        acs: Vec::new(),
                    });
                    open_requirement = true;
                    open_malformed = false;
                }
                Bullet::Malformed(message) => {
                    doc.format_errors.push((number, message));
                    open_requirement = false;
                    open_malformed = true;
                }
            }
            continue;
        }
        if line.starts_with(' ') && (open_requirement || open_malformed) {
            if open_requirement && let Some(req) = doc.requirements.last_mut() {
                let trimmed = line.trim();
                if trimmed.starts_with("- AC:") {
                    req.acs.push(Ac {
                        negative: trimmed.contains("(negative)"),
                    });
                } else {
                    req.text.push(' ');
                    req.text.push_str(trimmed);
                }
            }
            continue;
        }
        // A column-0 line that is not a bullet ends the open bullet and is prose.
        open_requirement = false;
        open_malformed = false;
        doc.prose.push(ProseLine {
            line: number,
            text: (*line).to_string(),
        });
    }
    doc
}
