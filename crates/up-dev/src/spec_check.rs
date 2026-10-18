//! Spec checker (PUB-09, FND-04): unique IDs, front matter, roadmap coverage, negative ACs, normative words outside requirements.

use std::fmt;
use std::path::Path;

use crate::check::{CheckResult, Ctx, Failure};

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

/// Check the spec tree under `docs_dir`. Read-only.
pub fn check_tree(_docs_dir: &Path) -> Result<Report, Failure> {
    Ok(Report::default())
}

pub fn run(_ctx: &Ctx, _args: &[String]) -> CheckResult {
    Err(Failure::pending("spec-check", "01-07"))
}
