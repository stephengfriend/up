//! The PUB-09 rules. Each rule is one function that pushes findings.

use std::collections::BTreeMap;

use super::Finding;
use super::parse::Tree;

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
        out.push(Finding {
            file: file.to_string(),
            line,
            rule: "duplicate-id",
            message: format!("{id} is defined {} times: {listed}", locations.len()),
        });
    }
}
