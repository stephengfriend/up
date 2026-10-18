//! Reviewed exemptions from the negative-AC rule (D1-12, D1-14).
//!
//! `docs/spec-exemptions.yml` is optional; absent means no exemptions. It can
//! exempt the negative-AC rule and nothing else: there is no baseline and no
//! allowlist for any other rule. Only a human adds or edits entries.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use up_config::yaml;

use super::Finding;

pub const FILE: &str = "docs/spec-exemptions.yml";

/// Shortest accepted reason, in characters.
const MIN_REASON_CHARS: usize = 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExemptionsFile {
    schema_version: u32,
    negative_ac: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    reason: String,
}

/// What the exemptions need to know about a defined requirement.
pub struct Subject {
    pub has_negative_ac: bool,
    pub matches_keyword: bool,
}

fn finding(line: usize, message: String) -> Finding {
    Finding {
        file: FILE.to_string(),
        line,
        rule: "exemption",
        message,
    }
}

/// First line mentioning `needle`, or 1.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|l| l.contains(needle))
        .map_or(1, |i| i + 1)
}

/// Validate the exemptions file against the defined requirements. Returns the
/// IDs it lists: those are exempt from the negative-AC rule, whatever else is
/// wrong with the entry (the entry's own problem is reported separately).
pub fn check(
    text: &str,
    subjects: &BTreeMap<String, Subject>,
    out: &mut Vec<Finding>,
) -> BTreeSet<String> {
    let mut listed = BTreeSet::new();
    let file: ExemptionsFile = match yaml::from_str(text, FILE) {
        Ok(file) => file,
        Err(e) => {
            out.push(finding(1, e.detail()));
            return listed;
        }
    };
    if file.schema_version != 1 {
        out.push(finding(
            line_of(text, "schema_version"),
            format!("schema_version must be 1, found {}", file.schema_version),
        ));
    }
    for entry in &file.negative_ac {
        let line = line_of(text, &entry.id);
        if !listed.insert(entry.id.clone()) {
            out.push(finding(
                line_of_nth(text, &entry.id, 2),
                format!("{} is listed more than once", entry.id),
            ));
            continue;
        }
        if entry.reason.trim().chars().count() < MIN_REASON_CHARS {
            out.push(finding(
                line,
                format!(
                    "the reason for {} must be at least {MIN_REASON_CHARS} characters",
                    entry.id
                ),
            ));
        }
        match subjects.get(&entry.id) {
            None => out.push(finding(
                line,
                format!("{} is not a requirement defined in docs/design", entry.id),
            )),
            Some(s) if s.has_negative_ac => out.push(finding(
                line,
                format!(
                    "stale exemption: {} already has a (negative) AC; remove the entry",
                    entry.id
                ),
            )),
            Some(s) if !s.matches_keyword => out.push(finding(
                line,
                format!(
                    "stale exemption: {} matches no keyword, so the negative-AC rule does not apply; remove the entry",
                    entry.id
                ),
            )),
            Some(_) => {}
        }
    }
    listed
}

/// Line of the `n`th line mentioning `needle`, or the first one.
fn line_of_nth(text: &str, needle: &str, n: usize) -> usize {
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.contains(needle))
        .nth(n - 1)
        .map_or_else(|| line_of(text, needle), |(i, _)| i + 1)
}
