//! PUB-13: AGENTS.md `## Rules` must state each agent-safety rule. This is the
//! spec's negative AC: removing any one statement fails CI. Each mutation is
//! derived from the committed AGENTS.md so the tests track the real wording.

use up_dev::repo_files::{AGENTS_RULES, check_agents_md};

fn committed() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../AGENTS.md");
    std::fs::read_to_string(path).expect("AGENTS.md exists at the repo root")
}

/// Delete every occurrence of `phrase` from the `## Rules` section only.
fn without_phrase_in_rules(text: &str, phrase: &str) -> String {
    let marker = "\n## Rules\n";
    let at = text.find(marker).expect("AGENTS.md has a ## Rules section");
    let (head, tail) = text.split_at(at + marker.len());
    assert!(
        tail.contains(phrase),
        "`{phrase}` is not in the Rules section"
    );
    format!("{head}{}", tail.replace(phrase, ""))
}

fn expected_message(label: &str) -> String {
    format!("PUB-13: AGENTS.md ## Rules is missing the statement \"{label}\"")
}

/// The (label, phrase) pair for `label`, from the shipped table.
fn rule(label: &str) -> (&'static str, &'static str) {
    *AGENTS_RULES
        .iter()
        .find(|(l, _)| *l == label)
        .unwrap_or_else(|| panic!("no AGENTS_RULES entry labelled {label:?}"))
}

fn assert_removal_fails(label: &str) {
    let (label, phrase) = rule(label);
    let text = without_phrase_in_rules(&committed(), phrase);
    assert_eq!(check_agents_md(&text), [expected_message(label)]);
}

#[test]
fn test_pub_13_agents_md_rules_pass() {
    assert_eq!(check_agents_md(&committed()), Vec::<String>::new());
}

#[test]
fn test_pub_13_every_spec_statement_has_an_entry() {
    let labels: Vec<&str> = AGENTS_RULES.iter().map(|(l, _)| *l).collect();
    for needed in [
        "agents never hold signing keys",
        "agents never hold secrets-backend sessions",
        "agents never bypass guard",
        "agents never change signers",
        "agents never change recipients",
        "agents never change guard exceptions",
    ] {
        assert!(labels.contains(&needed), "missing {needed:?} in {labels:?}");
    }
}

#[test]
fn test_pub_13_removing_signing_key_statement_fails() {
    assert_removal_fails("agents never hold signing keys");
}

#[test]
fn test_pub_13_removing_secrets_backend_session_statement_fails() {
    assert_removal_fails("agents never hold secrets-backend sessions");
}

#[test]
fn test_pub_13_removing_bypass_guard_statement_fails() {
    assert_removal_fails("agents never bypass guard");
}

#[test]
fn test_pub_13_removing_signer_sets_statement_fails() {
    assert_removal_fails("agents never change signers");
}

#[test]
fn test_pub_13_removing_recipient_lists_statement_fails() {
    assert_removal_fails("agents never change recipients");
}

#[test]
fn test_pub_13_removing_guard_exceptions_statement_fails() {
    assert_removal_fails("agents never change guard exceptions");
}

#[test]
fn test_pub_13_statement_outside_rules_section_fails() {
    let (label, phrase) = rule("agents never bypass guard");
    let text = committed();
    let bullet = text
        .lines()
        .find(|l| l.contains(phrase))
        .expect("the bypass-guard bullet exists");
    let without: Vec<&str> = text.lines().filter(|l| *l != bullet).collect();
    let mut moved = Vec::new();
    for line in without {
        moved.push(line.to_string());
        if line == "## How to work" {
            moved.push(String::new());
            moved.push(bullet.to_string());
        }
    }
    let moved = moved.join("\n") + "\n";
    assert!(
        moved.contains(bullet),
        "the bullet still exists in the text"
    );
    assert_eq!(check_agents_md(&moved), [expected_message(label)]);
}

#[test]
fn test_pub_13_statement_in_later_section_does_not_count() {
    let (label, phrase) = rule("agents never bypass guard");
    let text = without_phrase_in_rules(&committed(), phrase);
    let text = format!("{text}\n## Appendix\n\n{phrase}\n");
    assert_eq!(check_agents_md(&text), [expected_message(label)]);
}

#[test]
fn test_pub_13_missing_rules_section_fails() {
    let text = committed().replace("\n## Rules\n", "\n## Policies\n");
    assert_eq!(
        check_agents_md(&text),
        ["PUB-13: AGENTS.md has no ## Rules section".to_string()]
    );
}
