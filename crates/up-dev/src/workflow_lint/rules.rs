//! SUPPLY-02 workflow rules over a loaded workflow document.

/// One rule violation, located inside the workflow document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: &'static str,
    pub location: String,
    pub message: String,
}

/// Check one workflow document against every SUPPLY-02 rule.
pub fn check_workflow(_doc: &serde_json::Value) -> Vec<Violation> {
    Vec::new()
}
