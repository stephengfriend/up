//! Keyword list for the negative-AC rule (D1-12).
//!
//! A requirement "involves root, the network or a credential" when its text
//! contains one of these words. A false positive only forces one extra
//! negative AC. The list is versioned: a test pins a digest of it per version,
//! so editing the list means bumping [`KEYWORD_LIST_VERSION`] and recording the
//! new digest on purpose.

/// Version of [`KEYWORDS`]. Bump it, and record the new digest in
/// `tests/pub_09.rs`, whenever the list changes.
pub const KEYWORD_LIST_VERSION: u32 = 1;

/// Lowercase words, matched case-insensitively and whole-word.
pub const KEYWORDS: &[&str] = &[
    "root",
    "sudo",
    "privilege",
    "privileged",
    "privileges",
    "network",
    "networks",
    "networking",
    "fetch",
    "fetches",
    "fetched",
    "fetching",
    "download",
    "downloads",
    "downloaded",
    "downloading",
    "http",
    "https",
    "url",
    "urls",
    "token",
    "tokens",
    "credential",
    "credentials",
    "secret",
    "secrets",
    "password",
    "passwords",
    "key",
    "keys",
    "sign",
    "signs",
    "signed",
    "signing",
    "signer",
    "signers",
    "signature",
    "signatures",
];
