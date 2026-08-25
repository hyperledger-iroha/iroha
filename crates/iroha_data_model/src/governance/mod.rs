//! Canonical governance and SORA Parliament data model.
//!
//! The module exposes closed V1 identifiers, lifecycle states, aggregate
//! decision arithmetic, certificate bindings, and governance-domain events.
pub mod events;
/// Canonical governance and Parliament wire types.
pub mod types {
    pub use crate::parliament_types::*;
}
/// Maximum encoded length of a canonical V1 governance selector.
pub const GOVERNANCE_SELECTOR_V1_MAX_BYTES: usize = 128;
/// OpenAPI/SDK grammar for canonical V1 governance selectors.
///
/// Selectors are one RFC 3986 unreserved path segment. A leading dot is deliberately excluded so
/// intermediaries cannot reinterpret a selector as a relative-path segment.
pub const GOVERNANCE_SELECTOR_V1_PATTERN: &str = "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$";
/// Return whether `value` is a canonical V1 governance selector.
///
/// The accepted alphabet is RFC 3986 unreserved ASCII (`ALPHA`, `DIGIT`,
/// `-`, `.`, `_`, `~`), bounded to 128 bytes. The first byte may not be `.`;
/// this rejects both dot segments and leading-dot normalization aliases.
#[must_use]
pub fn is_valid_governance_selector_v1(value: &str) -> bool {
    fn is_unreserved_without_dot(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'~')
    }
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > GOVERNANCE_SELECTOR_V1_MAX_BYTES {
        return false;
    }
    is_unreserved_without_dot(bytes[0])
        && bytes[1..]
            .iter()
            .copied()
            .all(|byte| is_unreserved_without_dot(byte) || byte == b'.')
}
#[cfg(test)]
mod tests {
    use super::{GOVERNANCE_SELECTOR_V1_MAX_BYTES, is_valid_governance_selector_v1};
    #[test]
    fn governance_selector_v1_accepts_only_bounded_unreserved_path_segments() {
        let maximum = "a".repeat(GOVERNANCE_SELECTOR_V1_MAX_BYTES);
        let overlong = "a".repeat(GOVERNANCE_SELECTOR_V1_MAX_BYTES + 1);
        for valid in [
            "a",
            "referendum-1",
            "A9_selector~with.dots",
            maximum.as_str(),
        ] {
            assert!(is_valid_governance_selector_v1(valid), "{valid:?}");
        }
        for invalid in [
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a%2Fb",
            "has space",
            "投票",
            overlong.as_str(),
        ] {
            assert!(!is_valid_governance_selector_v1(invalid), "{invalid:?}");
        }
    }
}
