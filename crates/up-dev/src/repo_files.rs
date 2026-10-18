//! Repository skeleton check (PUB-01).
//!
//! The M0 path list is data: [`M0_PATHS`]. A path counts only when it is in
//! the git index, so an untracked file on disk is reported missing. Plan 01-02
//! and 01-11 extend this module.

use std::collections::BTreeSet;

use crate::check::{CheckResult, Ctx, Failure, tracked_files};

/// What a required path must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
}

/// One required path. A directory is written with a trailing `/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredPath {
    pub path: &'static str,
    pub kind: Kind,
}

const fn file(path: &'static str) -> RequiredPath {
    RequiredPath {
        path,
        kind: Kind::File,
    }
}

const fn dir(path: &'static str) -> RequiredPath {
    RequiredPath {
        path,
        kind: Kind::Dir,
    }
}

/// The skeleton paths PUB-01 enforces from M0, in report order.
pub const M0_PATHS: &[RequiredPath] = &[
    file("AGENTS.md"),
    file("CLAUDE.md"),
    file("README.md"),
    file("docs/README.md"),
    file("docs/roadmap.md"),
    file("docs/decisions.md"),
    file("docs/glossary.md"),
    file("docs/open-questions.md"),
    file("docs/design/threat-model.md"),
    dir("docs/design/"),
    dir("docs/reviews/"),
    dir("prompts/"),
    dir(".claude/commands/"),
];

/// Contents-table paths enforced from M4, not M0 (documented here, not
/// enforced until Phase 20 tightens the check).
pub const DEFERRED_TO_M4: &[&str] = &["up.sh", "CHANGELOG.md", "docs/guides/", "docs/reference/"];

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::File => "file",
        Kind::Dir => "directory",
    }
}

/// Check `required` against the tracked set. Every failing entry is reported,
/// in list order; there is no early return.
pub fn check_paths(tracked: &BTreeSet<String>, required: &[RequiredPath]) -> Vec<String> {
    let mut messages = Vec::new();
    for req in required {
        let (present, wrong_kind) = match req.kind {
            Kind::File => {
                let under = format!("{}/", req.path);
                (
                    tracked.contains(req.path),
                    tracked.iter().any(|t| t.starts_with(&under)),
                )
            }
            Kind::Dir => {
                let stem = req.path.trim_end_matches('/');
                (
                    tracked.iter().any(|t| t.starts_with(req.path)),
                    tracked.contains(stem),
                )
            }
        };
        if present {
            continue;
        }
        let kind = kind_name(req.kind);
        if wrong_kind {
            messages.push(format!(
                "PUB-01: {} has the wrong kind (expected {kind})",
                req.path
            ));
        } else {
            messages.push(format!("PUB-01: missing {} (expected {kind})", req.path));
        }
    }
    messages
}

/// Stub: the real PUB-10 check lands in the GREEN step.
pub fn check_license(_text: &str) -> Vec<String> {
    Vec::new()
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let tracked = tracked_files(&ctx.root)?;
    let messages = check_paths(&tracked, M0_PATHS);
    if messages.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(messages))
    }
}
