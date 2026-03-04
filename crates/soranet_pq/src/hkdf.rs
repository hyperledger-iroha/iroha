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

impl HkdfVariant {
    fn expand(self, info: &[u8], length: usize) -> Result<Zeroizing<Vec<u8>>, hkdf::InvalidLength> {
        match self {
            HkdfVariant::Sha3_256(inner) => {
                let mut okm = Zeroizing::new(vec![0_u8; length]);
                inner.expand(info, &mut okm)?;
                Ok(okm)
            }
            HkdfVariant::Sha3_512(inner) => {
                let mut okm = Zeroizing::new(vec![0_u8; length]);
                inner.expand(info, &mut okm)?;
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
    let mut info =
        Vec::with_capacity(domain.namespace.len() + domain.label.len() + context.len() + 2);
    info.extend_from_slice(domain.namespace.as_bytes());
    info.push(0);
    info.extend_from_slice(domain.label.as_bytes());
    info.push(0);
    info.extend_from_slice(context);

    suite.expand(salt, ikm).expand(&info, length)
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
