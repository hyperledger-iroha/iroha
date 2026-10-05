//! Closed grammars for every string a measurement record may carry.
//!
//! Phase, counter and failure labels are compile-time literals in the
//! instrumenting code. Identity text is supplied once, before the measured
//! workload starts. Neither grammar admits whitespace, quotes, control
//! characters or arbitrary byte encodings longer than its bound.

/// Longest accepted phase, counter, stage or failure-code label in bytes.
pub const MAX_LABEL_BYTES: usize = 96;
/// Longest accepted identity or provenance text in bytes.
pub const MAX_IDENTITY_BYTES: usize = 160;
/// Sentinel for an identity field that the caller did not bind to a real value.
///
/// A record containing it stays decodable so that an unbound local run is
/// retained, and the report validator rejects it as incomplete identity.
pub const UNBOUND: &str = "unbound";
/// Label substituted for a label outside the public grammar.
pub const INVALID_LABEL: &str = "invalid_label";

fn label_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
}

fn identity_byte(byte: u8) -> bool {
    label_byte(byte) || matches!(byte, b'/' | b'+' | b'@' | b'=' | b',')
}

/// Whether `text` is a public label: `[A-Za-z0-9_.:-]{1,96}`.
pub fn is_public_label(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_LABEL_BYTES && text.bytes().all(label_byte)
}

/// Whether `text` is identity text: `[A-Za-z0-9_.:/+@=,-]{1,160}`.
pub fn is_identity_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_IDENTITY_BYTES && text.bytes().all(identity_byte)
}

fn is_lower_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether `text` is a full Git object name: 40 or 64 lowercase hex digits.
pub fn is_source_commit(text: &str) -> bool {
    is_lower_hex(text, 40) || is_lower_hex(text, 64)
}

/// Whether `text` is a SHA-256 digest written as 64 lowercase hex digits.
pub fn is_sha256_hex(text: &str) -> bool {
    is_lower_hex(text, 64)
}

/// Return `label` when it is public, otherwise the fixed [`INVALID_LABEL`].
///
/// The recorder never panics and never returns a decision to instrumented
/// code, so a malformed literal is replaced and counted instead.
pub(crate) fn public_or_invalid(label: &'static str) -> (&'static str, bool) {
    if is_public_label(label) {
        (label, true)
    } else {
        (INVALID_LABEL, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_label_grammar_accepts_only_bounded_ascii_names() {
        for accepted in ["a", "prove", "Composition", "fri.layer-0:fold_2", "A1_b2"] {
            assert!(is_public_label(accepted), "{accepted}");
        }
        let longest = "x".repeat(MAX_LABEL_BYTES);
        assert!(is_public_label(&longest));
        for rejected in [
            "",
            "two words",
            "tab\there",
            "quote\"",
            "slash/path",
            "naïve",
            "nul\0",
            "brace{",
        ] {
            assert!(!is_public_label(rejected), "{rejected:?}");
        }
        assert!(!is_public_label(&"x".repeat(MAX_LABEL_BYTES + 1)));
    }

    #[test]
    fn identity_text_grammar_extends_labels_without_admitting_whitespace() {
        for accepted in [
            "Mac13,2/apple-m1-ultra/128GiB",
            "sha256:abc",
            "profile=release+lto",
            "crates/a/src/lib.rs:LIMIT",
            "user@host",
        ] {
            assert!(is_identity_text(accepted), "{accepted}");
        }
        assert!(is_identity_text(&"y".repeat(MAX_IDENTITY_BYTES)));
        for rejected in ["", "a b", "line\nbreak", "semi;colon", "back\\slash", "é"] {
            assert!(!is_identity_text(rejected), "{rejected:?}");
        }
        assert!(!is_identity_text(&"y".repeat(MAX_IDENTITY_BYTES + 1)));
    }

    #[test]
    fn source_commit_and_digest_require_exact_lowercase_hex_lengths() {
        assert!(is_source_commit(&"a".repeat(40)));
        assert!(is_source_commit(&"0123456789abcdef".repeat(4)));
        assert!(!is_source_commit(&"a".repeat(39)));
        assert!(!is_source_commit(&"A".repeat(40)));
        assert!(!is_source_commit(&"g".repeat(40)));
        assert!(!is_source_commit(UNBOUND));
        assert!(is_sha256_hex(&"f".repeat(64)));
        assert!(!is_sha256_hex(&"f".repeat(40)));
        assert!(!is_sha256_hex(&"F".repeat(64)));
    }

    #[test]
    fn malformed_literal_is_replaced_by_the_fixed_invalid_label() {
        assert_eq!(public_or_invalid("fri"), ("fri", true));
        assert_eq!(public_or_invalid("not public"), (INVALID_LABEL, false));
        assert!(is_public_label(INVALID_LABEL));
        assert!(is_public_label(UNBOUND) && is_identity_text(UNBOUND));
    }
}
