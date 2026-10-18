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
