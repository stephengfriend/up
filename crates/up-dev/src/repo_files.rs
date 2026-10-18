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
    file("SECURITY.md"),
    file("CONTRIBUTING.md"),
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

/// GRD-03 / SUPPLY-02 (CR-01): no tracked gitleaks configuration or ignore file, at any depth
/// (seam; implemented in the fix commit).
pub fn check_no_gitleaks_config(_tracked: &BTreeSet<String>) -> Vec<String> {
    Vec::new()
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

/// The H2 sections PUB-12 requires in SECURITY.md, in order.
pub const SECURITY_SECTIONS: &[&str] = &[
    "Reporting a vulnerability",
    "Supported versions",
    "Embargo and disclosure",
];

/// Characters stripped from both ends of a whitespace-separated token before
/// the email-shape test (markdown and prose punctuation).
const EMAIL_TRIM: &[char] = &[
    '<', '>', '(', ')', '[', ']', '{', '}', ',', ';', ':', '"', '\'', '`',
];

/// True when `token` looks like `local@domain.tld`: something before the
/// `@`, and a `.` with characters on both sides after it.
fn looks_like_email(token: &str) -> bool {
    let token = token.trim_matches(EMAIL_TRIM);
    let Some((local, domain)) = token.split_once('@') else {
        return false;
    };
    if local.is_empty() {
        return false;
    }
    let chars: Vec<char> = domain.chars().collect();
    (1..chars.len().saturating_sub(1)).any(|i| chars[i] == '.')
}

/// PUB-12: SECURITY.md must carry the three H2 sections, each with body
/// text, and must not name an email address (reports go through GitHub
/// private vulnerability reporting). A section counts only for a line that is
/// exactly `## <title>`. The address is never echoed in a message.
pub fn check_security_md(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut messages = Vec::new();
    for title in SECURITY_SECTIONS {
        let heading = format!("## {title}");
        let Some(start) = lines.iter().position(|l| *l == heading) else {
            messages.push(format!(
                "PUB-12: SECURITY.md is missing section \"{title}\""
            ));
            continue;
        };
        let has_body = lines[start + 1..]
            .iter()
            .take_while(|l| !l.starts_with('#'))
            .any(|l| !l.trim().is_empty());
        if !has_body {
            messages.push(format!("PUB-12: SECURITY.md section \"{title}\" is empty"));
        }
    }
    if text.split_whitespace().any(looks_like_email) {
        messages.push(
            "PUB-12: SECURITY.md contains an email address; use GitHub private vulnerability reporting"
                .to_string(),
        );
    }
    messages
}

/// Statements PUB-13 requires in AGENTS.md `## Rules`: (label, required phrase).
pub const AGENTS_RULES: &[(&str, &str)] = &[
    ("agents never hold signing keys", "never hold a signing key"),
    (
        "agents never hold secrets-backend sessions",
        "secrets-backend session",
    ),
    ("agents never bypass guard", "Never bypass guard"),
    ("agents never change signers", "signer sets"),
    ("agents never change recipients", "recipient lists"),
    ("agents never change guard exceptions", "guard exceptions"),
    (
        "agents never edit spec-check exemptions",
        "spec-check exemptions",
    ),
];

/// PUB-13: AGENTS.md must state each rule in [`AGENTS_RULES`] inside its
/// `## Rules` section, which runs from the line exactly `## Rules` to the next
/// line starting with `## ` (or the end of the file). The same words anywhere
/// else in the file do not count.
pub fn check_agents_md(text: &str) -> Vec<String> {
    let Some(start) = text.lines().position(|l| l == "## Rules") else {
        return vec!["PUB-13: AGENTS.md has no ## Rules section".to_string()];
    };
    let section: Vec<&str> = text
        .lines()
        .skip(start + 1)
        .take_while(|l| !l.starts_with("## "))
        .collect();
    let section = section.join("\n");
    AGENTS_RULES
        .iter()
        .filter(|(_, phrase)| !section.contains(phrase))
        .map(|(label, _)| {
            format!("PUB-13: AGENTS.md ## Rules is missing the statement \"{label}\"")
        })
        .collect()
}

pub fn run(ctx: &Ctx, _args: &[String]) -> CheckResult {
    let tracked = tracked_files(&ctx.root)?;
    let mut messages = check_paths(&tracked, M0_PATHS);
    messages.extend(check_no_gitleaks_config(&tracked));
    match std::fs::read_to_string(ctx.root.join("LICENSE")) {
        Ok(text) => messages.extend(check_license(&text)),
        Err(_) => messages.push("PUB-10: LICENSE unreadable".to_string()),
    }
    match std::fs::read_to_string(ctx.root.join("SECURITY.md")) {
        Ok(text) => messages.extend(check_security_md(&text)),
        Err(_) => messages.push("PUB-12: SECURITY.md unreadable".to_string()),
    }
    match std::fs::read_to_string(ctx.root.join("AGENTS.md")) {
        Ok(text) => messages.extend(check_agents_md(&text)),
        Err(_) => messages.push("PUB-13: AGENTS.md unreadable".to_string()),
    }
    if messages.is_empty() {
        Ok(())
    } else {
        Err(Failure::many(messages))
    }
}
