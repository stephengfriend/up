//! PUB-09: the spec checker. Each failing tree under `tests/fixtures/spec/<rule>/`
//! violates exactly one rule; `pass` violates none (D1-15).

use std::path::PathBuf;

use up_dev::spec_check::{Report, check_tree};

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
