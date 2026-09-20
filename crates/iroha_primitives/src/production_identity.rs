//! Canonical public identity grammar and explicit non-production namespace markers.
//!
//! These checks admit public labels only. They establish neither hardware custody nor
//! administrative independence, trusted provenance or permission to use a signing key.

/// Whether a label has a reserved, delimiter-separated non-production component.
///
/// Components contain ASCII letters and digits; every other character is a delimiter.
/// Exactly `null`, `mock`, `test`, `dev`, `demo`, `fake`, `dummy` and `placeholder`
/// are reserved, compared without ASCII case. Substrings such as `attester`,
/// `attestation`, `latest` and `contest` are not reserved components.
/// This predicate does not validate syntax or normalize the supplied bytes.
#[must_use]
pub fn has_reserved_nonproduction_component_v1(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            [
                "null",
                "mock",
                "test",
                "dev",
                "demo",
                "fake",
                "dummy",
                "placeholder",
            ]
            .iter()
            .any(|reserved| component.eq_ignore_ascii_case(reserved))
        })
}

/// Whether a public V1 identity obeys its owner's byte ceiling and canonical grammar.
///
/// Identities contain one or more ASCII letters, digits, `.`, `_`, `-` or `:` and
/// no reserved non-production component. The caller supplies its existing byte bound.
/// Bytes remain case-sensitive and are never trimmed, folded or rewritten.
#[must_use]
pub fn is_production_identity_v1(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
        && !has_reserved_nonproduction_component_v1(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_markers_are_exact_case_insensitive_delimiter_components() {
        for reserved in [
            "null",
            "mock",
            "test",
            "dev",
            "demo",
            "fake",
            "dummy",
            "placeholder",
        ] {
            for component in [reserved.to_owned(), reserved.to_ascii_uppercase()] {
                assert!(has_reserved_nonproduction_component_v1(&component));
                for delimiter in ['.', '_', '-', ':', '/', '@', ' ', '\0', 'é'] {
                    let surrounded = format!("production{delimiter}{component}{delimiter}primary");
                    assert!(
                        has_reserved_nonproduction_component_v1(&surrounded),
                        "{surrounded:?}"
                    );
                }
                assert!(!has_reserved_nonproduction_component_v1(&format!(
                    "x{component}"
                )));
                assert!(!has_reserved_nonproduction_component_v1(&format!(
                    "{component}1"
                )));
            }
        }
    }

    #[test]
    fn identities_preserve_real_words_case_and_owner_byte_bounds() {
        for identity in [
            "attester",
            "attestation",
            "latest",
            "contest",
            "Account-Attester",
            "production:primary_1.alpha",
        ] {
            assert!(is_production_identity_v1(identity, identity.len()));
            assert!(is_production_identity_v1(
                &identity.to_ascii_uppercase(),
                identity.len()
            ));
            assert!(!is_production_identity_v1(identity, identity.len() - 1));
        }
        for bound in [1, 128, 256] {
            assert!(is_production_identity_v1(&"a".repeat(bound), bound));
            assert!(!is_production_identity_v1(&"a".repeat(bound + 1), bound));
        }
        assert!(!is_production_identity_v1("a", 0));
    }

    #[test]
    fn identities_reject_malformed_bytes_and_every_reserved_component() {
        for identity in [
            "",
            " a",
            "a ",
            "a/b",
            "a@b",
            "a?b",
            "a#b",
            "a%2fb",
            "a;b",
            "a=b",
            "a\\b",
            "a\0b",
            "a\nb",
            "é",
            "テスト",
        ] {
            assert!(!is_production_identity_v1(identity, 128), "{identity:?}");
        }
        for reserved in [
            "null",
            "mock",
            "test",
            "dev",
            "demo",
            "fake",
            "dummy",
            "placeholder",
        ] {
            for delimiter in ['.', '_', '-', ':'] {
                let identity = format!(
                    "production{delimiter}{}{delimiter}primary",
                    reserved.to_ascii_uppercase()
                );
                assert!(!is_production_identity_v1(&identity, 128), "{identity:?}");
            }
        }
    }
}
