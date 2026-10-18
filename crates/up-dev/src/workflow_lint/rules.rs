//! SUPPLY-02 workflow rules over a loaded workflow document.
//!
//! These rules are the engine's own statement of the SUPPLY-02 negative AC.
//! They hold independently of what zizmor enables by default, so a change in
//! an external linter's defaults cannot silently drop one of them.

use serde_json::Value;

/// One rule violation, located inside the workflow document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: &'static str,
    pub location: String,
    pub message: String,
}

pub const RULE_UNPINNED: &str = "unpinned-action";

/// Check one workflow document against every SUPPLY-02 rule.
pub fn check_workflow(doc: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    check_unpinned_actions(doc, &mut out);
    out
}

fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether a `uses:` value is acceptable under SUPPLY-02.
///
/// - `./path` is local to the repository and allowed.
/// - `docker://image@sha256:<64 lowercase hex>` is pinned by digest.
/// - `owner/repo[/path]@<40 lowercase hex>` is pinned by full commit SHA.
///
/// Everything else (tags, branches, short or upper-case SHAs, `../`) is refused.
pub fn uses_is_pinned(uses: &str) -> bool {
    if uses.starts_with("./") {
        return true;
    }
    if let Some(image) = uses.strip_prefix("docker://") {
        return match image.rsplit_once("@sha256:") {
            Some((name, digest)) => !name.is_empty() && is_lower_hex(digest, 64),
            None => false,
        };
    }
    let Some((name, reference)) = uses.split_once('@') else {
        return false;
    };
    let mut segments = name.split('/');
    let owner = segments.next().unwrap_or("");
    let repo = segments.next().unwrap_or("");
    if owner.is_empty() || repo.is_empty() || segments.any(str::is_empty) {
        return false;
    }
    is_lower_hex(reference, 40)
}

fn check_uses(uses: &Value, location: String, out: &mut Vec<Violation>) {
    match uses.as_str() {
        Some(s) if uses_is_pinned(s) => {}
        Some(s) => out.push(Violation {
            rule: RULE_UNPINNED,
            location,
            message: format!(
                "`{s}` is not pinned to a 40-character lowercase commit SHA (docker: a sha256 digest)"
            ),
        }),
        None => out.push(Violation {
            rule: RULE_UNPINNED,
            location,
            message: "`uses` is not a string".to_string(),
        }),
    }
}

fn check_unpinned_actions(doc: &Value, out: &mut Vec<Violation>) {
    let Some(jobs) = doc.get("jobs").and_then(Value::as_object) else {
        return;
    };
    for (id, job) in jobs {
        if let Some(uses) = job.get("uses") {
            check_uses(uses, format!("jobs.{id}"), out);
        }
        let Some(steps) = job.get("steps").and_then(Value::as_array) else {
            continue;
        };
        for (n, step) in steps.iter().enumerate() {
            if let Some(uses) = step.get("uses") {
                check_uses(uses, format!("jobs.{id}.steps[{n}]"), out);
            }
        }
    }
}
