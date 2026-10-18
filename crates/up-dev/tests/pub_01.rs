//! PUB-01: the repo skeleton check, on synthetic tracked sets (no git, no
//! filesystem writes).

use std::collections::BTreeSet;
use std::path::Path;

use up_dev::repo_files::{Kind, M0_PATHS, check_paths};

/// A tracked set holding every M0 path (a directory gets one file under it).
fn full_set() -> BTreeSet<String> {
    M0_PATHS
        .iter()
        .map(|r| match r.kind {
            Kind::File => r.path.to_string(),
            Kind::Dir => format!("{}x", r.path),
        })
        .collect()
}

#[test]
fn test_pub_01_all_present_passes() {
    assert_eq!(check_paths(&full_set(), M0_PATHS), Vec::<String>::new());
}

#[test]
fn test_pub_01_missing_paths_reported_in_list_order() {
    let mut tracked = full_set();
    // Remove a later entry first and an earlier one second: the report must
    // still follow the M0 list order, not removal or alphabetical order.
    tracked.remove("prompts/x");
    tracked.remove("docs/roadmap.md");
    let messages = check_paths(&tracked, M0_PATHS);
    assert_eq!(
        messages,
        vec![
            "PUB-01: missing docs/roadmap.md (expected file)".to_string(),
            "PUB-01: missing prompts/ (expected directory)".to_string(),
        ]
    );
}

#[test]
fn test_pub_01_dir_tracked_as_file_fails_naming_kind() {
    let mut tracked = full_set();
    tracked.retain(|p| !p.starts_with("docs/reviews/"));
    tracked.insert("docs/reviews".to_string());
    let messages = check_paths(&tracked, M0_PATHS);
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("docs/reviews/"), "{messages:?}");
    assert!(messages[0].contains("expected directory"), "{messages:?}");
    assert!(messages[0].starts_with("PUB-01:"), "{messages:?}");
}

#[test]
fn test_pub_01_file_path_is_directory_fails_naming_kind() {
    let mut tracked = full_set();
    tracked.remove("AGENTS.md");
    tracked.insert("AGENTS.md/x".to_string());
    let messages = check_paths(&tracked, M0_PATHS);
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].contains("AGENTS.md"), "{messages:?}");
    assert!(messages[0].contains("expected file"), "{messages:?}");
    assert!(messages[0].contains("wrong kind"), "{messages:?}");
}

#[test]
fn test_pub_01_untracked_path_counts_as_missing() {
    // README.md exists on disk, but the tracked set (the git index) lacks it.
    let on_disk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    assert!(
        on_disk.is_file(),
        "fixture assumption: README.md exists on disk"
    );
    let mut tracked = full_set();
    tracked.remove("README.md");
    let messages = check_paths(&tracked, M0_PATHS);
    assert_eq!(
        messages,
        vec!["PUB-01: missing README.md (expected file)".to_string()]
    );
}

/// Every path in the public-repo.md contents table. SECURITY.md and
/// CONTRIBUTING.md are listed separately: plan 01-02 creates them and moves
/// them into M0_PATHS; until then they may sit in either place.
const CONTENTS_TABLE: &[&str] = &[
    "up.sh",
    "crates/up/",
    "ansible/",
    "chezmoi/",
    "images/",
    "containers/",
    "schema/",
    "toolchain/",
    "defaults.yml",
    "versions.yml",
    "hooks/",
    "guard/",
    "template/",
    "docs/README.md",
    "docs/design/",
    "docs/design/threat-model.md",
    "docs/roadmap.md",
    "docs/decisions.md",
    "docs/glossary.md",
    "docs/open-questions.md",
    "docs/reviews/",
    "docs/guides/",
    "docs/reference/",
    "AGENTS.md",
    "prompts/",
    "CLAUDE.md",
    ".claude/commands/",
    "README.md",
    "CHANGELOG.md",
    "LICENSE",
    ".up-engine",
];
const PENDING_PLAN_01_02: &[&str] = &["CONTRIBUTING.md", "SECURITY.md"];

#[test]
fn test_pub_01_m0_list_covers_contents_table() {
    use up_dev::repo_files::DEFERRED_TO_M4;
    let in_m0 = |p: &str| M0_PATHS.iter().any(|r| r.path == p);
    for path in CONTENTS_TABLE.iter().chain(PENDING_PLAN_01_02) {
        let known =
            in_m0(path) || DEFERRED_TO_M4.contains(path) || PENDING_PLAN_01_02.contains(path);
        assert!(
            known,
            "contents table path {path} is in neither M0_PATHS nor DEFERRED_TO_M4"
        );
    }
    // The M0 subset is everything except the M4-deferred paths (and the two
    // files plan 01-02 owns): nothing else may hide in the deferred list.
    for path in CONTENTS_TABLE {
        if DEFERRED_TO_M4.contains(path) {
            assert!(
                !in_m0(path),
                "{path} is deferred to M4 and must not be enforced in M0"
            );
        } else {
            assert!(in_m0(path), "{path} must be enforced from M0");
        }
    }
    for path in PENDING_PLAN_01_02 {
        assert!(
            in_m0(path) || !DEFERRED_TO_M4.contains(path),
            "{path} is an M0 file and must not be deferred to M4"
        );
    }
}
