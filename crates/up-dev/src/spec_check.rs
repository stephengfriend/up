//! Spec checker (PUB-09, FND-04): unique IDs, front matter, roadmap coverage, negative ACs, normative words outside requirements.

use std::fmt;
use std::path::Path;

use crate::check::{CheckResult, Ctx, Failure};

mod exemptions;
mod keywords;
mod parse;
mod rules;

pub use keywords::{KEYWORD_LIST_VERSION, KEYWORDS};

/// One problem found in the spec tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// Path relative to the tree root, for example `docs/design/cli.md`.
    pub file: String,
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: [{}] {}",
            self.file, self.line, self.rule, self.message
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

/// Check the spec tree under `docs_dir`. Read-only: nothing is written.
pub fn check_tree(docs_dir: &Path) -> Result<Report, Failure> {
    let tree = parse::read_tree(docs_dir)?;
    let mut report = Report::default();
    rules::duplicate_ids(&tree, &mut report.errors);
    rules::front_matter(&tree, &mut report.errors);
    rules::requirement_shape(&tree, &mut report.errors);
    rules::roadmap_coverage(&tree, &mut report.errors);
    rules::negative_acs(&tree, &mut report.errors);
    rules::prose_words(&tree, &mut report.errors, &mut report.warnings);
    report.errors.sort();
    report.warnings.sort();
    Ok(report)
}

/// Escape a message for a GitHub workflow command.
fn escape_annotation(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// `spec-check [--docs <dir>]`: check `<root>/docs`, or another tree.
pub fn run(ctx: &Ctx, args: &[String]) -> CheckResult {
    let docs_dir = match args {
        [] => ctx.root.join("docs"),
        [flag, dir] if flag == "--docs" => std::path::PathBuf::from(dir),
        _ => return Err(Failure::one("usage: up-dev spec-check [--docs <dir>]")),
    };
    let report = check_tree(&docs_dir)?;
    for w in &report.warnings {
        ctx.warn(&w.file, w.line, &format!("[{}] {}", w.rule, w.message));
    }
    if report.errors.is_empty() {
        return Ok(());
    }
    let mut messages = Vec::new();
    for e in &report.errors {
        if ctx.github_actions {
            messages.push(format!(
                "::error file={},line={}::{}",
                e.file,
                e.line,
                escape_annotation(&format!("[{}] {}", e.rule, e.message))
            ));
        }
        messages.push(e.to_string());
    }
    Err(Failure::many(messages))
}
