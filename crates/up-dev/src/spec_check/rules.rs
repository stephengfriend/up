//! The PUB-09 rules. Each rule is one function that pushes findings.

use std::collections::{BTreeMap, BTreeSet};

use super::Finding;
use super::exemptions;
use super::keywords::{KEYWORD_LIST_VERSION, KEYWORDS};
use super::parse::{DesignDoc, DocPrefix, FrontMatter, Requirement, Tree, body_start, in_fence};

const ROADMAP: &str = "docs/roadmap.md";

fn finding(file: &str, line: usize, rule: &'static str, message: String) -> Finding {
    Finding {
        file: file.to_string(),
        line,
        rule,
        message,
    }
}

/// duplicate-id: an ID defined more than once. The finding sits at the first
/// occurrence and lists every location, sorted by path then line.
pub fn duplicate_ids(tree: &Tree, out: &mut Vec<Finding>) {
    let mut by_id: BTreeMap<&str, Vec<(&str, usize)>> = BTreeMap::new();
    for doc in &tree.docs {
        for req in &doc.requirements {
            by_id
                .entry(req.id.as_str())
                .or_default()
                .push((req.file.as_str(), req.line));
        }
    }
    for (id, mut locations) in by_id {
        if locations.len() < 2 {
            continue;
        }
        locations.sort();
        let listed = locations
            .iter()
            .map(|(file, line)| format!("{file}:{line}"))
            .collect::<Vec<_>>()
            .join(", ");
        let (file, line) = locations[0];
        out.push(finding(
            file,
            line,
            "duplicate-id",
            format!("{id} is defined {} times: {listed}", locations.len()),
        ));
    }
}

/// front-matter: all five fields present and well typed, and their values valid.
pub fn front_matter(tree: &Tree, out: &mut Vec<Finding>) {
    let stems: BTreeSet<&str> = tree.docs.iter().map(|d| d.stem.as_str()).collect();
    for doc in &tree.docs {
        let fm = match &doc.front {
            Ok(fm) => fm,
            Err(message) => {
                out.push(finding(&doc.path, 1, "front-matter", message.clone()));
                continue;
            }
        };
        front_matter_values(doc, fm, &stems, out);
    }
}

fn front_matter_values(
    doc: &DesignDoc,
    fm: &FrontMatter,
    stems: &BTreeSet<&str>,
    out: &mut Vec<Finding>,
) {
    let mut bad = |message: String| out.push(finding(&doc.path, 1, "front-matter", message));
    if fm.title.trim().is_empty() {
        bad("title must not be empty".to_string());
    }
    if fm.status != "draft" && fm.status != "accepted" {
        bad(format!(
            "status must be `draft` or `accepted`, found `{}`",
            fm.status
        ));
    }
    if doc.prefix == DocPrefix::Unknown {
        bad(format!(
            "prefix must be `none` or uppercase letters, found `{}`",
            fm.prefix
        ));
    }
    for m in &fm.milestones {
        let valid = m.len() == 2
            && m.starts_with('M')
            && m.as_bytes()[1].is_ascii_digit()
            && m.as_bytes()[1] <= b'8';
        if !valid {
            bad(format!("milestone `{m}` is not one of M0 to M8"));
        }
    }
    for dep in &fm.depends_on {
        if !stems.contains(dep.as_str()) {
            bad(format!(
                "depends_on entry `{dep}` has no docs/design/{dep}.md"
            ));
        }
    }
}

/// structure, prefix-mismatch, requirement-format and no-ac.
pub fn requirement_shape(tree: &Tree, out: &mut Vec<Finding>) {
    for doc in &tree.docs {
        match &doc.prefix {
            DocPrefix::NoPrefix => {
                if let Some(&line) = doc.requirements_headings.first() {
                    out.push(finding(
                        &doc.path,
                        line,
                        "structure",
                        "a `prefix: none` document must not have a `## Requirements` section"
                            .to_string(),
                    ));
                }
            }
            DocPrefix::Known(prefix) => {
                match doc.requirements_headings.as_slice() {
                    [] => out.push(finding(
                        &doc.path,
                        1,
                        "structure",
                        format!("prefix `{prefix}` needs exactly one `## Requirements` section, found none"),
                    )),
                    [_] => {}
                    [_, second, ..] => out.push(finding(
                        &doc.path,
                        *second,
                        "structure",
                        format!(
                            "prefix `{prefix}` needs exactly one `## Requirements` section, found {}",
                            doc.requirements_headings.len()
                        ),
                    )),
                }
                for req in doc.requirements.iter().filter(|r| r.prefix != *prefix) {
                    out.push(finding(
                        &doc.path,
                        req.line,
                        "prefix-mismatch",
                        format!(
                            "{} has prefix {} but this document declares prefix {prefix}",
                            req.id, req.prefix
                        ),
                    ));
                }
            }
            DocPrefix::Unknown => {}
        }
        for (line, message) in &doc.format_errors {
            out.push(finding(
                &doc.path,
                *line,
                "requirement-format",
                message.clone(),
            ));
        }
        for req in doc.requirements.iter().filter(|r| r.acs.is_empty()) {
            out.push(finding(
                &doc.path,
                req.line,
                "no-ac",
                format!("{} has no `AC:` line", req.id),
            ));
        }
    }
}

/// One ID token in a roadmap line: `PFX-NN`.
struct Token<'a> {
    prefix: &'a str,
    digits: &'a str,
    start: usize,
    end: usize,
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Every `PFX-digits` token in `line`, left to right.
fn id_tokens(line: &str) -> Vec<Token<'_>> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let starts_word = bytes[i].is_ascii_uppercase() && (i == 0 || !is_word_byte(bytes[i - 1]));
        if !starts_word {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_uppercase() {
            j += 1;
        }
        if bytes.get(j) == Some(&b'-') {
            let mut k = j + 1;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                k += 1;
            }
            if k > j + 1 && bytes.get(k).is_none_or(|b| !is_word_byte(*b)) {
                tokens.push(Token {
                    prefix: &line[i..j],
                    digits: &line[j + 1..k],
                    start: i,
                    end: k,
                });
                i = k;
                continue;
            }
        }
        i = j;
    }
    tokens
}

/// Largest accepted `PFX-NN to PFX-MM` span; a wider one is a typo.
const MAX_RANGE: u32 = 999;

/// Coverage between the design docs and the roadmap: roadmap-missing,
/// roadmap-undefined and roadmap-range.
pub fn roadmap_coverage(tree: &Tree, out: &mut Vec<Finding>) {
    // IDs the design docs define, and the prefixes they declare.
    let mut defined: BTreeMap<&str, &Requirement> = BTreeMap::new();
    let mut declared: BTreeSet<&str> = BTreeSet::new();
    for doc in &tree.docs {
        match &doc.prefix {
            DocPrefix::Known(prefix) => {
                declared.insert(prefix.as_str());
                for req in doc.requirements.iter().filter(|r| r.prefix == *prefix) {
                    defined.entry(req.id.as_str()).or_insert(req);
                }
            }
            DocPrefix::NoPrefix | DocPrefix::Unknown => {
                for req in &doc.requirements {
                    declared.insert(req.prefix.as_str());
                    defined.entry(req.id.as_str()).or_insert(req);
                }
            }
        }
    }
    let Some(roadmap) = &tree.roadmap else {
        out.push(finding(
            ROADMAP,
            1,
            "structure",
            "docs/roadmap.md not found".to_string(),
        ));
        return;
    };
    let mut mentioned: BTreeMap<String, usize> = BTreeMap::new();
    scan_roadmap(roadmap, &declared, &mut mentioned, out);
    for (id, req) in &defined {
        if !mentioned.contains_key(*id) {
            out.push(finding(
                &req.file,
                req.line,
                "roadmap-missing",
                format!("{id} is not delivered by any milestone in docs/roadmap.md"),
            ));
        }
    }
    for (id, line) in &mentioned {
        if !defined.contains_key(id.as_str()) {
            out.push(finding(
                ROADMAP,
                *line,
                "roadmap-undefined",
                format!("{id} is named in the roadmap but no design document defines it"),
            ));
        }
    }
}

/// Collect the IDs of declared prefixes that the roadmap names, expanding
/// `PFX-NN to PFX-MM` ranges inclusively at both ends.
fn scan_roadmap(
    text: &str,
    declared: &BTreeSet<&str>,
    mentioned: &mut BTreeMap<String, usize>,
    out: &mut Vec<Finding>,
) {
    let lines: Vec<&str> = text.lines().collect();
    let mut fence = None;
    for (index, line) in lines.iter().enumerate().skip(body_start(&lines)) {
        if in_fence(&mut fence, line) {
            continue;
        }
        let number = index + 1;
        let mut mention = |id: String| {
            mentioned.entry(id).or_insert(number);
        };
        let tokens = id_tokens(line);
        let mut i = 0;
        while i < tokens.len() {
            let first = &tokens[i];
            let range_end = tokens
                .get(i + 1)
                .filter(|next| line[first.end..next.start] == *" to ");
            let Some(last) = range_end else {
                if declared.contains(first.prefix) {
                    mention(format!("{}-{}", first.prefix, first.digits));
                }
                i += 1;
                continue;
            };
            i += 2;
            if !declared.contains(first.prefix) && !declared.contains(last.prefix) {
                continue;
            }
            // Both ends always count as mentioned, so a bad range reports only itself.
            mention(format!("{}-{}", first.prefix, first.digits));
            mention(format!("{}-{}", last.prefix, last.digits));
            let problem = |message: String| finding(ROADMAP, number, "roadmap-range", message);
            let shown = format!(
                "{}-{} to {}-{}",
                first.prefix, first.digits, last.prefix, last.digits
            );
            if first.prefix != last.prefix {
                out.push(problem(format!("range `{shown}` mixes two prefixes")));
                continue;
            }
            let (Ok(from), Ok(to)) = (first.digits.parse::<u32>(), last.digits.parse::<u32>())
            else {
                out.push(problem(format!(
                    "range `{shown}` has numbers that are too large"
                )));
                continue;
            };
            if from > to {
                out.push(problem(format!("range `{shown}` runs backwards")));
            } else if to - from > MAX_RANGE {
                out.push(problem(format!(
                    "range `{shown}` spans more than {MAX_RANGE} IDs"
                )));
            } else {
                for n in from..=to {
                    mention(format!("{}-{n:02}", first.prefix));
                }
            }
        }
    }
}

/// Lowercase words of `text`: maximal runs of ASCII letters, digits and underscore.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
}

/// Keyword hits in a requirement's text, sorted and unique.
fn keyword_hits(req: &Requirement) -> Vec<&'static str> {
    let mut hits = BTreeSet::new();
    for word in words(&req.text) {
        let lower = word.to_ascii_lowercase();
        if let Some(k) = KEYWORDS.iter().find(|k| **k == lower) {
            hits.insert(*k);
        }
    }
    hits.into_iter().collect()
}

/// negative-ac and exemption: a requirement matching the keyword list needs a
/// (negative) AC unless a human listed it in the exemptions file.
pub fn negative_acs(tree: &Tree, out: &mut Vec<Finding>) {
    let mut subjects: BTreeMap<String, exemptions::Subject> = BTreeMap::new();
    for doc in &tree.docs {
        for req in &doc.requirements {
            subjects
                .entry(req.id.clone())
                .or_insert_with(|| exemptions::Subject {
                    has_negative_ac: req.acs.iter().any(|a| a.negative),
                    matches_keyword: !keyword_hits(req).is_empty(),
                });
        }
    }
    let exempt = tree
        .exemptions
        .as_deref()
        .map(|text| exemptions::check(text, &subjects, out))
        .unwrap_or_default();
    for doc in &tree.docs {
        for req in &doc.requirements {
            let hits = keyword_hits(req);
            if hits.is_empty() || req.acs.iter().any(|a| a.negative) || exempt.contains(&req.id) {
                continue;
            }
            out.push(finding(
                &doc.path,
                req.line,
                "negative-ac",
                format!(
                    "{} matches keyword list v{KEYWORD_LIST_VERSION} ({}) but has no (negative) AC; add one, or a human adds a reviewed entry to {}",
                    req.id,
                    hits.join(", "),
                    exemptions::FILE
                ),
            ));
        }
    }
}

/// Remove inline code spans (a run of backticks up to the next run of the same length).
fn strip_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let run = rest[open..].bytes().take_while(|b| *b == b'`').count();
        let after = &rest[open + run..];
        let ticks = "`".repeat(run);
        match after.find(&ticks) {
            Some(close) => {
                out.push_str(&rest[..open]);
                out.push(' ');
                // Skip the closing run and any extra backticks glued to it.
                rest = &after[close + run..];
            }
            None => {
                out.push_str(&rest[..open + run]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

const UPPERCASE_KEYWORDS: &[&str] = &["MUST", "SHOULD", "MAY"];
const LOWERCASE_WORDS: &[&str] = &["never", "only"];

/// rfc-keyword (error) and normative-word (warning) in design-doc prose, D1-13.
pub fn prose_words(tree: &Tree, errors: &mut Vec<Finding>, warnings: &mut Vec<Finding>) {
    for doc in &tree.docs {
        for prose in &doc.prose {
            let text = strip_inline_code(&prose.text);
            let seen: BTreeSet<&str> = words(&text).collect();
            let upper: Vec<&str> = UPPERCASE_KEYWORDS
                .iter()
                .copied()
                .filter(|k| seen.contains(k))
                .collect();
            if !upper.is_empty() {
                errors.push(finding(
                    &doc.path,
                    prose.line,
                    "rfc-keyword",
                    format!(
                        "uppercase {} outside a requirement; reword it in lowercase or move the normative text into a requirement",
                        upper.join(", ")
                    ),
                ));
            }
            for word in LOWERCASE_WORDS.iter().filter(|w| seen.contains(**w)) {
                warnings.push(finding(
                    &doc.path,
                    prose.line,
                    "normative-word",
                    format!(
                        "lowercase \"{word}\" outside a requirement; if it states a rule, move it into a requirement"
                    ),
                ));
            }
        }
    }
}
