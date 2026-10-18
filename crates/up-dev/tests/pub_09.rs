//! PUB-09: the spec checker. Each failing tree under `tests/fixtures/spec/<rule>/`
//! violates exactly one rule; `pass` violates none (D1-15).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use up_dev::spec_check::{Finding, KEYWORD_LIST_VERSION, KEYWORDS, Report, check_tree};

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/spec")
        .join(name)
}

fn check(name: &str) -> Report {
    check_tree(&fixture_root(name).join("docs")).expect("the fixture tree is readable")
}

/// 1-based numbers of the lines in `rel` (relative to the fixture root) that start with `prefix`.
fn lines_starting_with(name: &str, rel: &str, prefix: &str) -> Vec<usize> {
    let text = std::fs::read_to_string(fixture_root(name).join(rel)).expect("fixture file");
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with(prefix))
        .map(|(i, _)| i + 1)
        .collect()
}

#[test]
fn test_pub_09_passing_tree_passes() {
    let report = check("pass");
    assert_eq!(report.errors, []);
    assert_eq!(report.warnings, []);
}

#[test]
fn test_pub_09_duplicate_id_fails_reporting_both_locations() {
    let report = check("duplicate-id");
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    let finding = &report.errors[0];
    assert_eq!(finding.rule, "duplicate-id");
    let lines = lines_starting_with("duplicate-id", "docs/design/alpha.md", "- **ALPHA-02**");
    assert_eq!(lines.len(), 2);
    assert_eq!(finding.file, "docs/design/alpha.md");
    assert_eq!(finding.line, lines[0]);
    assert!(
        finding.message.starts_with("ALPHA-02 "),
        "{}",
        finding.message
    );
    let both = format!(
        "docs/design/alpha.md:{}, docs/design/alpha.md:{}",
        lines[0], lines[1]
    );
    assert!(finding.message.contains(&both), "{}", finding.message);
}

#[test]
fn test_pub_09_duplicate_id_locations_are_sorted_by_path_then_line() {
    let report = check("duplicate-id-across-files");
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    let finding = &report.errors[0];
    assert_eq!(finding.rule, "duplicate-id");
    let first = lines_starting_with(
        "duplicate-id-across-files",
        "docs/design/alpha.md",
        "- **ALPHA-02**",
    );
    let second = lines_starting_with(
        "duplicate-id-across-files",
        "docs/design/extra.md",
        "- **ALPHA-02**",
    );
    let both = format!(
        "docs/design/alpha.md:{}, docs/design/extra.md:{}",
        first[0], second[0]
    );
    assert!(finding.message.contains(&both), "{}", finding.message);
    assert_eq!(
        (finding.file.as_str(), finding.line),
        ("docs/design/alpha.md", first[0])
    );
}

// ---------------------------------------------------------------------------
// Task 2 rules
// ---------------------------------------------------------------------------

/// The one error of a fixture tree, which must have the given rule and nothing else.
fn only_error(name: &str, rule: &str) -> Finding {
    let report = check(name);
    assert_eq!(report.errors.len(), 1, "{name}: {:?}", report.errors);
    assert_eq!(report.errors[0].rule, rule, "{name}: {:?}", report.errors);
    assert_eq!(report.warnings, [], "{name}");
    report.errors[0].clone()
}

fn assert_clean(name: &str) {
    let report = check(name);
    assert_eq!(report.errors, [], "{name}");
    assert_eq!(report.warnings, [], "{name}");
}

#[test]
fn test_pub_09_missing_front_matter_field_fails() {
    let f = only_error("front-matter-missing-field", "front-matter");
    assert_eq!(f.file, "docs/design/alpha.md");
    assert!(f.message.contains("depends_on"), "{}", f.message);
}

#[test]
fn test_pub_09_bad_status_fails() {
    let f = only_error("front-matter-bad-status", "front-matter");
    assert_eq!(f.file, "docs/design/alpha.md");
    assert!(f.message.contains("final"), "{}", f.message);
}

#[test]
fn test_pub_09_depends_on_unknown_doc_fails() {
    let f = only_error("front-matter-depends-on-unknown", "front-matter");
    assert_eq!(f.file, "docs/design/alpha.md");
    assert!(f.message.contains("nowhere"), "{}", f.message);
}

#[test]
fn test_pub_09_unknown_front_matter_field_fails() {
    let f = only_error("front-matter-unknown-field", "front-matter");
    // ARCH-11: the message names the error kind, not the offending key's text.
    assert!(f.message.contains("unknown field"), "{}", f.message);
    assert!(!f.message.contains("owner"), "{}", f.message);
}

#[test]
fn test_pub_09_bad_milestone_fails() {
    let f = only_error("front-matter-bad-milestone", "front-matter");
    assert!(f.message.contains("M9"), "{}", f.message);
}

#[test]
fn test_pub_09_prefix_mismatch_fails() {
    let f = only_error("prefix-mismatch", "prefix-mismatch");
    let lines = lines_starting_with("prefix-mismatch", "docs/design/alpha.md", "- **BETA-01**");
    assert_eq!(
        (f.file.as_str(), f.line),
        ("docs/design/alpha.md", lines[0])
    );
    assert!(
        f.message.contains("BETA-01") && f.message.contains("ALPHA"),
        "{}",
        f.message
    );
}

#[test]
fn test_pub_09_prefixless_doc_with_requirements_fails() {
    let f = only_error("structure-prefixless-with-requirements", "structure");
    assert_eq!(f.file, "docs/design/overview.md");
}

#[test]
fn test_pub_09_prefixed_doc_without_requirements_fails() {
    let f = only_error("structure-prefixed-without-requirements", "structure");
    assert_eq!(f.file, "docs/design/alpha.md");
}

#[test]
fn test_pub_09_two_requirements_sections_fail() {
    let f = only_error("structure-two-requirements-sections", "structure");
    assert_eq!(f.file, "docs/design/alpha.md");
}

#[test]
fn test_pub_09_requirement_without_ac_fails() {
    let f = only_error("no-ac", "no-ac");
    assert_eq!(f.file, "docs/design/alpha.md");
    assert!(f.message.starts_with("ALPHA-01"), "{}", f.message);
}

#[test]
fn test_pub_09_requirement_id_needs_two_digits() {
    let f = only_error("requirement-format-id-digits", "requirement-format");
    assert!(f.message.contains("ALPHA-003"), "{}", f.message);
}

#[test]
fn test_pub_09_plain_bullet_in_requirements_fails() {
    let f = only_error("requirement-format-plain-bullet", "requirement-format");
    let lines = lines_starting_with(
        "requirement-format-plain-bullet",
        "docs/design/alpha.md",
        "- This bullet",
    );
    assert_eq!(f.line, lines[0]);
}

#[test]
fn test_pub_09_roadmap_missing_id_fails() {
    let f = only_error("roadmap-missing", "roadmap-missing");
    let lines = lines_starting_with("roadmap-missing", "docs/design/alpha.md", "- **ALPHA-03**");
    assert_eq!(
        (f.file.as_str(), f.line),
        ("docs/design/alpha.md", lines[0])
    );
    assert!(f.message.starts_with("ALPHA-03"), "{}", f.message);
}

#[test]
fn test_pub_09_roadmap_range_is_inclusive() {
    assert_clean("roadmap-range-inclusive");
}

#[test]
fn test_pub_09_roadmap_single_element_range_and_annotations_count() {
    assert_clean("roadmap-range-single");
}

#[test]
fn test_pub_09_roadmap_undefined_id_fails() {
    let f = only_error("roadmap-undefined", "roadmap-undefined");
    let lines = lines_starting_with("roadmap-undefined", "docs/roadmap.md", "- Delivers:");
    assert_eq!((f.file.as_str(), f.line), ("docs/roadmap.md", lines[0]));
    assert!(f.message.starts_with("ALPHA-09"), "{}", f.message);
}

#[test]
fn test_pub_09_roadmap_range_end_must_be_defined() {
    let f = only_error("roadmap-undefined-in-range", "roadmap-undefined");
    assert!(f.message.starts_with("ALPHA-03"), "{}", f.message);
}

#[test]
fn test_pub_09_roadmap_descending_range_fails() {
    let f = only_error("roadmap-range-descending", "roadmap-range");
    assert_eq!(f.file, "docs/roadmap.md");
}

#[test]
fn test_pub_09_roadmap_ignores_undeclared_prefixes() {
    // `pass` mentions D-01 and Q-17 in the roadmap; neither prefix is declared by a design doc.
    assert_clean("pass");
}

#[test]
fn test_pub_09_missing_negative_ac_fails() {
    let f = only_error("negative-ac-missing", "negative-ac");
    assert!(f.message.starts_with("ALPHA-02"), "{}", f.message);
    assert!(f.message.contains("token"), "{}", f.message);
    assert!(f.message.contains("keyword list v1"), "{}", f.message);
}

#[test]
fn test_pub_09_exemption_valid_passes() {
    assert_clean("exemption-valid");
}

#[test]
fn test_pub_09_exemption_stale_fails() {
    let f = only_error("exemption-stale-negative-ac-present", "exemption");
    assert_eq!(f.file, "docs/spec-exemptions.yml");
    assert!(
        f.message.contains("ALPHA-02") && f.message.contains("stale"),
        "{}",
        f.message
    );
}

#[test]
fn test_pub_09_exemption_without_keyword_match_is_stale() {
    let f = only_error("exemption-stale-no-keyword", "exemption");
    assert!(
        f.message.contains("ALPHA-01") && f.message.contains("stale"),
        "{}",
        f.message
    );
}

#[test]
fn test_pub_09_exemption_unknown_id_fails() {
    let f = only_error("exemption-unknown-id", "exemption");
    assert!(f.message.contains("ALPHA-77"), "{}", f.message);
}

#[test]
fn test_pub_09_exemption_duplicate_id_fails() {
    let f = only_error("exemption-duplicate-id", "exemption");
    assert!(f.message.contains("ALPHA-02"), "{}", f.message);
}

#[test]
fn test_pub_09_exemption_short_reason_fails() {
    let f = only_error("exemption-short-reason", "exemption");
    assert!(f.message.contains("20"), "{}", f.message);
}

#[test]
fn test_pub_09_exemption_schema_version_must_be_1() {
    let f = only_error("exemption-bad-schema-version", "exemption");
    assert!(f.message.contains("schema_version"), "{}", f.message);
}

#[test]
fn test_pub_09_exemptions_cover_only_the_negative_ac_rule() {
    // D1-14: no baseline or allowlist for any other rule; an `rfc_keyword` list is refused.
    let f = only_error("exemption-other-rule", "exemption");
    // ARCH-11: the message names the error kind, not the offending key's text.
    assert!(f.message.contains("unknown field"), "{}", f.message);
    assert!(!f.message.contains("rfc_keyword"), "{}", f.message);
}

#[test]
fn test_pub_09_uppercase_keyword_in_prose_fails() {
    let f = only_error("rfc-keyword-prose", "rfc-keyword");
    let lines = lines_starting_with("rfc-keyword-prose", "docs/design/alpha.md", "A synthetic");
    assert_eq!(
        (f.file.as_str(), f.line),
        ("docs/design/alpha.md", lines[0])
    );
    assert!(f.message.contains("MUST"), "{}", f.message);
}

#[test]
fn test_pub_09_uppercase_in_code_passes() {
    assert_clean("rfc-keyword-in-code");
}

#[test]
fn test_pub_09_keywords_inside_requirements_pass() {
    assert_clean("keywords-in-requirement");
}

#[test]
fn test_pub_09_lowercase_never_warns_not_fails() {
    let report = check("normative-word-warning");
    assert_eq!(report.errors, []);
    assert_eq!(report.warnings.len(), 2, "{:?}", report.warnings);
    assert!(report.warnings.iter().all(|w| w.rule == "normative-word"));
    let words: Vec<&str> = report
        .warnings
        .iter()
        .map(|w| {
            if w.message.contains("never") {
                "never"
            } else {
                "only"
            }
        })
        .collect();
    assert_eq!(words, ["never", "only"]);
}

/// Recorded SHA-256 of the keyword list for each version (one word per line).
fn recorded_digest(version: u32) -> &'static str {
    match version {
        1 => "bcc66866a8413276824ddcc05817e89c664c9c5dfc8b28a29d62a82280c48337",
        v => panic!("keyword list v{v} has no recorded digest; add it here on purpose"),
    }
}

#[test]
fn test_pub_09_keyword_list_version_pinned() {
    let mut hasher = Sha256::new();
    for word in KEYWORDS {
        hasher.update(word.as_bytes());
        hasher.update(b"\n");
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        recorded_digest(KEYWORD_LIST_VERSION),
        "KEYWORDS changed without bumping KEYWORD_LIST_VERSION and recording its digest"
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every file under `dir` with its bytes and permissions, sorted by path.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>, bool)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(snapshot(&path));
        } else {
            let readonly = std::fs::metadata(&path).unwrap().permissions().readonly();
            out.push((path.clone(), std::fs::read(&path).unwrap(), readonly));
        }
    }
    out.sort();
    out
}

#[test]
fn test_pub_09_checker_is_read_only() {
    // A tree with findings of several kinds, so every rule path runs.
    let tmp = tempfile::tempdir().unwrap();
    let docs = tmp.path().join("docs");
    copy_dir(
        &fixture_root("exemption-stale-negative-ac-present").join("docs"),
        &docs,
    );
    std::fs::write(
        docs.join("design/zeta.md"),
        "---\ntitle: Zeta\nstatus: draft\nprefix: none\nmilestones: [M0]\ndepends_on: []\n---\n\nThe tool MUST never fail.\n",
    )
    .unwrap();
    let before = snapshot(tmp.path());
    let reports: Vec<Report> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..4)
            .map(|_| s.spawn(|| check_tree(&docs).expect("readable")))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(!reports[0].errors.is_empty());
    assert!(
        reports.iter().all(|r| *r == reports[0]),
        "concurrent runs differ"
    );
    assert_eq!(snapshot(tmp.path()), before, "a run changed the tree");
}
