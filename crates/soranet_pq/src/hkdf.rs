use hkdf::Hkdf;
use sha3::{Sha3_256, Sha3_512};
use zeroize::Zeroizing;

/// Enumerates the supported HKDF suites used by the `SoraNet` handshake.
#[derive(Clone, Copy, Debug)]
pub enum HkdfSuite {
    /// HKDF based on SHA3-256.
    Sha3_256,
    /// HKDF based on SHA3-512.
    Sha3_512,
}

impl HkdfSuite {
    fn expand(self, salt: Option<&[u8]>, ikm: &[u8]) -> HkdfVariant {
        match self {
            HkdfSuite::Sha3_256 => HkdfVariant::Sha3_256(Hkdf::<Sha3_256>::new(salt, ikm)),
            HkdfSuite::Sha3_512 => HkdfVariant::Sha3_512(Hkdf::<Sha3_512>::new(salt, ikm)),
        }
    }
}

enum HkdfVariant {
    Sha3_256(Hkdf<Sha3_256>),
    Sha3_512(Hkdf<Sha3_512>),
}

const HKDF_LABEL_SEPARATOR: &[u8] = b"\0";

impl HkdfVariant {
    fn expand_multi_info(
        self,
        info_components: &[&[u8]],
        length: usize,
    ) -> Result<Zeroizing<Vec<u8>>, hkdf::InvalidLength> {
        match self {
            HkdfVariant::Sha3_256(inner) => {
                let mut okm = Zeroizing::new(vec![0_u8; length]);
                inner.expand_multi_info(info_components, &mut okm)?;
                Ok(okm)
            }
            HkdfVariant::Sha3_512(inner) => {
                let mut okm = Zeroizing::new(vec![0_u8; length]);
                inner.expand_multi_info(info_components, &mut okm)?;
                Ok(okm)
            }
        }
    }
}

/// Namespaces the HKDF derivation with a protocol label and sub-label.
#[derive(Clone, Copy, Debug)]
pub struct HkdfDomain<'a> {
    namespace: &'a str,
    label: &'a str,
}

impl<'a> HkdfDomain<'a> {
    /// Construct a new domain by providing both namespace and label.
    pub const fn new(namespace: &'a str, label: &'a str) -> Self {
        Self { namespace, label }
    }

    /// Shortcut for the canonical `SoraNet` post-quantum namespace.
    pub const fn soranet(label: &'a str) -> Self {
        Self {
            namespace: "soranet/pq",
            label,
        }
    }

    /// Returns the namespace component.
    #[must_use]
    pub const fn namespace(&self) -> &'a str {
        self.namespace
    }

    /// Returns the label component.
    #[must_use]
    pub const fn label(&self) -> &'a str {
        self.label
    }
}

/// Derive HKDF keying material with protocol domain separation.
///
/// The `domain` tuple avoids cross-talk between the Diffie-Hellman portion of
/// the handshake (`DH/es`, `DH/ss`, …) and the ML-KEM stages (`KEM/1`, `KEM/2`,
/// …) while allowing higher layers to thread additional context such as
/// transcript hashes.
///
/// # Errors
/// Returns an error if the requested output length exceeds the HKDF limits.
pub fn derive_labeled_hkdf(
    suite: HkdfSuite,
    salt: Option<&[u8]>,
    ikm: &[u8],
    domain: HkdfDomain<'_>,
    context: &[u8],
    length: usize,
) -> Result<Zeroizing<Vec<u8>>, hkdf::InvalidLength> {
    suite.expand(salt, ikm).expand_multi_info(
        &[
            domain.namespace.as_bytes(),
            HKDF_LABEL_SEPARATOR,
            domain.label.as_bytes(),
            HKDF_LABEL_SEPARATOR,
            context,
        ],
        length,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use hex::ToHex;

    use super::*;

    static IKM: LazyLock<Vec<u8>> = LazyLock::new(|| (0_u8..16).collect());
    static SALT: LazyLock<Vec<u8>> = LazyLock::new(|| (32_u8..48).collect());

    #[test]
    fn varying_domain_changes_output() {
        let base = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("DH/es"),
            b"transcript-hash",
            32,
        )
        .unwrap();

        let other = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("KEM/1"),
            b"transcript-hash",
            32,
        )
        .unwrap();

        assert_ne!(base.as_slice(), other.as_slice());
    }

    #[test]
    fn sha3_512_matches_expected_length() {
        let okm = derive_labeled_hkdf(
            HkdfSuite::Sha3_512,
            None,
            &IKM,
            HkdfDomain::new("custom", "label"),
            b"",
            64,
        )
        .unwrap();

        assert_eq!(okm.len(), 64);
    }

    #[test]
    fn domain_accessors_preserve_namespace_and_label() {
        let custom = HkdfDomain::new("custom-namespace", "custom-label");
        assert_eq!(custom.namespace(), "custom-namespace");
        assert_eq!(custom.label(), "custom-label");

        let soranet = HkdfDomain::soranet("KEM/2");
        assert_eq!(soranet.namespace(), "soranet/pq");
        assert_eq!(soranet.label(), "KEM/2");
    }

    #[test]
    fn zero_length_output_is_allowed() {
        let okm = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("empty-okm"),
            b"context",
            0,
        )
        .unwrap();

        assert!(okm.is_empty());
    }

    #[test]
    fn invalid_length_is_reported() {
        let err = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("too-long-okm"),
            b"context",
            (255 * 32) + 1,
        )
        .expect_err("HKDF-SHA3-256 output is limited to 255 hash blocks");

        assert!(err.to_string().contains("too large output"));
    }

    #[test]
    fn labeled_hkdf_matches_legacy_contiguous_info_layout() {
        let domain = HkdfDomain::soranet("KEM/final");
        let context = b"transcript-hash";
        let mut legacy_info = Vec::new();
        legacy_info.extend_from_slice(domain.namespace().as_bytes());
        legacy_info.push(0);
        legacy_info.extend_from_slice(domain.label().as_bytes());
        legacy_info.push(0);
        legacy_info.extend_from_slice(context);

        let labeled =
            derive_labeled_hkdf(HkdfSuite::Sha3_512, Some(&SALT), &IKM, domain, context, 64)
                .expect("labeled HKDF output");
        let legacy = Hkdf::<Sha3_512>::new(Some(&SALT), &IKM);
        let mut legacy_okm = [0_u8; 64];
        legacy
            .expand(&legacy_info, &mut legacy_okm)
            .expect("legacy contiguous HKDF output");

        assert_eq!(labeled.as_slice(), legacy_okm);
    }

    #[test]
    fn salt_and_context_are_domain_separating() {
        let base = derive_labeled_hkdf(
            HkdfSuite::Sha3_512,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("KEM/final"),
            b"context-a",
            64,
        )
        .unwrap();
        let different_context = derive_labeled_hkdf(
            HkdfSuite::Sha3_512,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("KEM/final"),
            b"context-b",
            64,
        )
        .unwrap();
        let different_salt = derive_labeled_hkdf(
            HkdfSuite::Sha3_512,
            Some(b"different salt"),
            &IKM,
            HkdfDomain::soranet("KEM/final"),
            b"context-a",
            64,
        )
        .unwrap();

        assert_ne!(base.as_slice(), different_context.as_slice());
        assert_ne!(base.as_slice(), different_salt.as_slice());
    }

    #[test]
    fn hkdf_suite_choice_is_domain_separating() {
        let sha3_256 = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("suite-choice"),
            b"context",
            32,
        )
        .unwrap();
        let sha3_512 = derive_labeled_hkdf(
            HkdfSuite::Sha3_512,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("suite-choice"),
            b"context",
            32,
        )
        .unwrap();

        assert_ne!(sha3_256.as_slice(), sha3_512.as_slice());
    }

    #[test]
    fn derive_is_deterministic() {
        let first = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("DH/es"),
            b"ctx",
            32,
        )
        .unwrap();

        let second = derive_labeled_hkdf(
            HkdfSuite::Sha3_256,
            Some(&SALT),
            &IKM,
            HkdfDomain::soranet("DH/es"),
            b"ctx",
            32,
        )
        .unwrap();

        assert_eq!(first.encode_hex::<String>(), second.encode_hex::<String>());
    }
}
