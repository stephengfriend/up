//! PUB-12: SECURITY.md must carry the three required sections and name no
//! email address. Mutations are built from the committed file so the tests
//! track the real text.

use up_dev::repo_files::{SECURITY_SECTIONS, check_security_md};

fn committed() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../SECURITY.md");
    std::fs::read_to_string(path).expect("SECURITY.md exists at the repo root")
}

/// Drop the heading line `## <title>` from the text, keeping its body.
fn without_heading(text: &str, title: &str) -> String {
    let heading = format!("## {title}");
    let kept: Vec<&str> = text.lines().filter(|l| *l != heading).collect();
    assert!(
        kept.len() < text.lines().count(),
        "no `{heading}` to remove"
    );
    kept.join("\n") + "\n"
}

fn assert_single_message(messages: &[String], expected: &str) {
    assert_eq!(messages, [expected.to_string()], "{messages:?}");
}

#[test]
fn test_pub_12_three_sections_present() {
    assert_eq!(
        SECURITY_SECTIONS,
        [
            "Reporting a vulnerability",
            "Supported versions",
            "Embargo and disclosure"
        ]
    );
    assert_eq!(check_security_md(&committed()), Vec::<String>::new());
}

#[test]
fn test_pub_12_missing_reporting_section_fails() {
    let text = without_heading(&committed(), "Reporting a vulnerability");
    assert_single_message(
        &check_security_md(&text),
        "PUB-12: SECURITY.md is missing section \"Reporting a vulnerability\"",
    );
}

#[test]
fn test_pub_12_missing_supported_versions_fails() {
    let text = without_heading(&committed(), "Supported versions");
    assert_single_message(
        &check_security_md(&text),
        "PUB-12: SECURITY.md is missing section \"Supported versions\"",
    );
}

#[test]
fn test_pub_12_missing_embargo_fails() {
    let text = without_heading(&committed(), "Embargo and disclosure");
    assert_single_message(
        &check_security_md(&text),
        "PUB-12: SECURITY.md is missing section \"Embargo and disclosure\"",
    );
}

#[test]
fn test_pub_12_empty_section_fails() {
    let text = "# Security policy\n\n## Reporting a vulnerability\n\n## Supported versions\n\nLatest release.\n\n## Embargo and disclosure\n\nNinety days.\n";
    assert_single_message(
        &check_security_md(text),
        "PUB-12: SECURITY.md section \"Reporting a vulnerability\" is empty",
    );
}

#[test]
fn test_pub_12_empty_last_section_fails() {
    let text = "# Security policy\n\n## Reporting a vulnerability\n\nUse the Security tab.\n\n## Supported versions\n\nLatest release.\n\n## Embargo and disclosure\n\n";
    assert_single_message(
        &check_security_md(text),
        "PUB-12: SECURITY.md section \"Embargo and disclosure\" is empty",
    );
}

#[test]
fn test_pub_12_heading_level_mismatch_fails() {
    let text = committed().replace("## Supported versions", "### Supported versions");
    assert_single_message(
        &check_security_md(&text),
        "PUB-12: SECURITY.md is missing section \"Supported versions\"",
    );
}

#[test]
fn test_pub_12_email_address_fails() {
    let address = "reports@example.com";
    let text = format!("{}\nWrite to {address} for help.\n", committed());
    let messages = check_security_md(&text);
    assert_single_message(
        &messages,
        "PUB-12: SECURITY.md contains an email address; use GitHub private vulnerability reporting",
    );
    assert!(
        messages
            .iter()
            .all(|m| !m.contains(address) && !m.contains('@')),
        "the message must not echo the address: {messages:?}"
    );
}

#[test]
fn test_pub_12_email_in_brackets_fails() {
    let text = format!("{}\nContact <a.b@mail.example.org>.\n", committed());
    assert_eq!(check_security_md(&text).len(), 1);
}

#[test]
fn test_pub_12_at_sign_without_address_shape_passes() {
    // `@handle` and `user@` are not email addresses.
    let text = format!("{}\nPing @maintainers or user@ later.\n", committed());
    assert_eq!(check_security_md(&text), Vec::<String>::new());
}
