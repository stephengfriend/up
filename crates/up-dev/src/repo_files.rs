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
    file("LICENSE"),
    file(".up-engine"),
    file("defaults.yml"),
    file("versions.yml"),
    dir("docs/design/"),
    dir("docs/reviews/"),
    dir("prompts/"),
    dir(".claude/commands/"),
    dir("crates/up/"),
    dir("ansible/"),
    dir("chezmoi/"),
    dir("images/"),
    dir("containers/"),
    dir("schema/"),
    dir("toolchain/"),
    dir("hooks/"),
    dir("guard/"),
    dir("template/"),
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

/// The MIT grant and its condition paragraph (whitespace-normalized on use).
const MIT_PERMISSION: &str = "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions: The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.";

/// The MIT warranty disclaimer (whitespace-normalized on use).
const MIT_WARRANTY: &str = "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.";

/// Collapse every run of whitespace to one space and trim.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True for `Copyright (c) <4 digits> <non-empty holder>`.
fn is_copyright_line(line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("Copyright (c) ") else {
        return false;
    };
    let Some((year, holder)) = rest.split_once(' ') else {
        return false;
    };
    year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()) && !holder.trim().is_empty()
}

/// PUB-10: the LICENSE text must be the MIT license. Each failure message
/// starts with `PUB-10:` and names LICENSE.
pub fn check_license(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return vec!["PUB-10: LICENSE is empty".to_string()];
    }
    let mut messages = Vec::new();
    if text.lines().next().map(str::trim) != Some("MIT License") {
        messages.push("PUB-10: LICENSE must start with the line \"MIT License\"".to_string());
    }
    if !text.lines().any(is_copyright_line) {
        messages
            .push("PUB-10: LICENSE has no line like \"Copyright (c) <year> <holder>\"".to_string());
    }
    let normalized = normalize(text);
    if !normalized.contains(&normalize(MIT_PERMISSION)) {
        messages.push("PUB-10: LICENSE permission text differs from the MIT license".to_string());
    }
    if !normalized.contains(&normalize(MIT_WARRANTY)) {
        messages.push("PUB-10: LICENSE warranty text differs from the MIT license".to_string());
    }
    messages
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let tracked = tracked_files(&ctx.root)?;
    let mut messages = check_paths(&tracked, M0_PATHS);
    match std::fs::read_to_string(ctx.root.join("LICENSE")) {
        Ok(text) => messages.extend(check_license(&text)),
        Err(_) => messages.push("PUB-10: LICENSE unreadable".to_string()),
    }
    if messages.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(messages))
    }
}
