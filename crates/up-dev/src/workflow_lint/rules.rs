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
pub const RULE_PERMISSIONS: &str = "token-permissions";
pub const RULE_PR_TARGET: &str = "pr-target-head-checkout";
pub const RULE_SELF_HOSTED: &str = "self-hosted-runner";
pub const RULE_SHAPE: &str = "workflow-shape";

/// Check one workflow document against every SUPPLY-02 rule.
pub fn check_workflow(doc: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    check_shape(doc, &mut out);
    check_unpinned_actions(doc, &mut out);
    check_token_permissions(doc, &mut out);
    check_pr_target_head_checkout(doc, &mut out);
    check_self_hosted(doc, &mut out);
    out
}

/// A workflow with no jobs has nothing to lint; passing it would be vacuous.
fn check_shape(doc: &Value, out: &mut Vec<Violation>) {
    let has_jobs = doc
        .get("jobs")
        .and_then(Value::as_object)
        .is_some_and(|jobs| !jobs.is_empty());
    if !has_jobs {
        out.push(Violation {
            rule: RULE_SHAPE,
            location: "jobs".to_string(),
            message: "workflow has no jobs (missing, empty or not a mapping)".to_string(),
        });
    }
}

/// The top-level `permissions` must exist and grant nothing beyond read.
///
/// Passes: `{}`, a mapping whose values are all `read` or `none`, `read-all`.
/// Fails: a missing key, `write-all`, any `write` value, anything else.
/// Job-level permissions are out of scope here (SUPPLY-02 grants write scopes
/// per job, reviewed by a human).
fn check_token_permissions(doc: &Value, out: &mut Vec<Violation>) {
    let location = "permissions".to_string();
    let mut fail = |location: String, message: String| {
        out.push(Violation {
            rule: RULE_PERMISSIONS,
            location,
            message,
        });
    };
    match doc.get("permissions") {
        None => fail(
            location,
            "top-level `permissions` is missing, so the token default applies".to_string(),
        ),
        Some(Value::String(s)) if s == "read-all" => {}
        Some(Value::String(s)) => fail(
            location,
            format!("`permissions: {s}` is not read-only (use a mapping or `read-all`)"),
        ),
        Some(Value::Object(map)) => {
            for (scope, level) in map {
                match level.as_str() {
                    Some("read" | "none") => {}
                    _ => fail(
                        format!("permissions.{scope}"),
                        format!("`{scope}` is not `read` or `none` at the top level"),
                    ),
                }
            }
        }
        Some(_) => fail(
            location,
            "top-level `permissions` is not a mapping or `read-all`".to_string(),
        ),
    }
}

/// Remove all whitespace and lower-case, so `${{github.event.pull_request.head.ref}}`
/// and `${{ github.event.pull_request.head.ref }}` compare equal.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Trigger names from `on`: a string, a list, or a mapping's keys.
fn triggers(doc: &Value) -> Vec<String> {
    match doc.get("on") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Some(Value::Object(map)) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

const HEAD_MARKERS: [&str; 3] = [
    "github.event.pull_request.head.",
    "github.head_ref",
    "refs/pull/",
];

/// `pull_request_target` runs with the base repository's token, so it must not
/// check out the pull request's head (fork code).
fn check_pr_target_head_checkout(doc: &Value, out: &mut Vec<Violation>) {
    if !triggers(doc).iter().any(|t| t == "pull_request_target") {
        return;
    }
    let Some(jobs) = doc.get("jobs").and_then(Value::as_object) else {
        return;
    };
    for (id, job) in jobs {
        let Some(steps) = job.get("steps").and_then(Value::as_array) else {
            continue;
        };
        for (n, step) in steps.iter().enumerate() {
            let is_checkout = step
                .get("uses")
                .and_then(Value::as_str)
                .is_some_and(|u| u.to_ascii_lowercase().starts_with("actions/checkout@"));
            if !is_checkout {
                continue;
            }
            for field in ["ref", "repository"] {
                let Some(value) = step
                    .get("with")
                    .and_then(|w| w.get(field))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let squashed = squash(value);
                if HEAD_MARKERS.iter().any(|m| squashed.contains(m)) {
                    out.push(Violation {
                        rule: RULE_PR_TARGET,
                        location: format!("jobs.{id}.steps[{n}].with.{field}"),
                        message: format!(
                            "`pull_request_target` workflow checks out the pull request head (`{field}: {}`)",
                            value.trim()
                        ),
                    });
                }
            }
        }
    }
}

/// String items of a scalar-or-list value.
fn strings_of(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The matrix key in a label of the form `${{ matrix.<key> }}`.
fn matrix_key(label: &str) -> Option<String> {
    let squashed: String = label.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = squashed.strip_prefix("${{")?.strip_suffix("}}")?;
    let key = inner.strip_prefix("matrix.")?;
    (!key.is_empty()).then(|| key.to_string())
}

/// Every label a job's `runs-on` can take, matrix values expanded.
fn runner_labels(job: &Value) -> Vec<String> {
    let labels = match job.get("runs-on") {
        Some(Value::Object(map)) => map.get("labels").map(strings_of).unwrap_or_default(),
        Some(other) => strings_of(other),
        None => Vec::new(),
    };
    let matrix = job.get("strategy").and_then(|s| s.get("matrix"));
    let mut all = Vec::new();
    for label in labels {
        if let (Some(key), Some(matrix)) = (matrix_key(&label), matrix) {
            if let Some(values) = matrix.get(&key) {
                all.extend(strings_of(values));
            }
            if let Some(include) = matrix.get("include").and_then(Value::as_array) {
                for entry in include {
                    if let Some(value) = entry.get(&key) {
                        all.extend(strings_of(value));
                    }
                }
            }
        }
        all.push(label);
    }
    all
}

/// SUPPLY-02 allows no self-hosted runners.
fn check_self_hosted(doc: &Value, out: &mut Vec<Violation>) {
    let Some(jobs) = doc.get("jobs").and_then(Value::as_object) else {
        return;
    };
    for (id, job) in jobs {
        if runner_labels(job)
            .iter()
            .any(|l| l.trim().eq_ignore_ascii_case("self-hosted"))
        {
            out.push(Violation {
                rule: RULE_SELF_HOSTED,
                location: format!("jobs.{id}.runs-on"),
                message: "job can run on a `self-hosted` runner".to_string(),
            });
        }
    }
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
