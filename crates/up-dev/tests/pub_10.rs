//! PUB-10: the LICENSE content check. The synthetic cases embed their own copy
//! of the MIT text; only `test_pub_10_license_is_mit` reads the repo file.

use up_dev::repo_files::check_license;

const GRANT: &str = "Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.";

const WARRANTY: &str =
    "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.";

fn mit(copyright: &str, grant: &str) -> String {
    format!("MIT License\n\n{copyright}\n\n{grant}\n\n{WARRANTY}\n")
}

#[test]
fn test_pub_10_license_is_mit() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE");
    let text = std::fs::read_to_string(path).expect("LICENSE exists at the repo root");
    assert_eq!(check_license(&text), Vec::<String>::new());
}

#[test]
fn test_pub_10_synthetic_mit_passes() {
    let text = mit("Copyright (c) 2026 The up authors", GRANT);
    assert_eq!(check_license(&text), Vec::<String>::new());
}

#[test]
fn test_pub_10_empty_license_fails() {
    let messages = check_license("");
    assert!(!messages.is_empty(), "an empty LICENSE must fail");
    assert!(
        messages.iter().all(|m| m.starts_with("PUB-10:")),
        "{messages:?}"
    );
    assert!(
        messages.iter().all(|m| m.contains("LICENSE")),
        "{messages:?}"
    );
}

#[test]
fn test_pub_10_altered_permission_text_fails() {
    let altered = GRANT.replace("free of charge", "for a fee");
    assert_ne!(altered, GRANT, "fixture must actually differ");
    let messages = check_license(&mit("Copyright (c) 2026 The up authors", &altered));
    assert!(!messages.is_empty(), "altered permission text must fail");
    assert!(
        messages.iter().any(|m| m.contains("permission")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .all(|m| m.starts_with("PUB-10:") && m.contains("LICENSE")),
        "{messages:?}"
    );
}

#[test]
fn test_pub_10_altered_warranty_text_fails() {
    let text = mit("Copyright (c) 2026 The up authors", GRANT)
        .replace("WITHOUT WARRANTY OF ANY KIND", "WITH WARRANTY");
    let messages = check_license(&text);
    assert!(
        messages.iter().any(|m| m.contains("warranty")),
        "{messages:?}"
    );
}

#[test]
fn test_pub_10_missing_copyright_line_fails() {
    let text = format!("MIT License\n\n{GRANT}\n\n{WARRANTY}\n");
    let messages = check_license(&text);
    assert!(!messages.is_empty(), "a missing copyright line must fail");
    assert!(
        messages.iter().any(|m| m.contains("Copyright")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .all(|m| m.starts_with("PUB-10:") && m.contains("LICENSE")),
        "{messages:?}"
    );
}
