//! Deterministic host mapping utilities shared between gateways and tooling.

use core::fmt;

/// Deterministic host mapping inputs.
#[derive(Debug, Clone)]
pub struct HostMappingInput<'a> {
    /// Network identifier (chain id as ASCII).
    pub chain_id: &'a str,
    /// Provider identifier recognised by governance (32-byte hash).
    pub provider_id: &'a [u8; 32],
}

impl<'a> HostMappingInput<'a> {
    /// Compute the canonical hostname binding the provider hash to the network.
    ///
    /// The canonical hostname uses the first four bytes of the provider id to
    /// ensure uniqueness while keeping DNS labels short.
    #[must_use]
    pub fn canonical_host(&self) -> String {
        let prefix = hex::encode(&self.provider_id[..4]);
        format!("{prefix}.{}.sorafs", self.chain_id)
    }

    /// Compute the vanity hostname derived from the provider id.
    ///
    /// Vanity hosts shorten the prefix to two bytes and live under the
    /// `direct.sorafs` subdomain reserved for direct-mode tooling.
    #[must_use]
    pub fn vanity_host(&self) -> String {
        let prefix = hex::encode(&self.provider_id[..2]);
        format!("{prefix}.{}.direct.sorafs", self.chain_id)
    }

    /// Render both canonical and vanity hosts as a summary object.
    #[must_use]
    pub fn to_summary(&self) -> HostMappingSummary {
        HostMappingSummary {
            canonical: self.canonical_host(),
            vanity: self.vanity_host(),
        }
    }

    /// Produce direct-CAR endpoints for the supplied manifest digest.
    ///
    /// The returned URLs use the pattern `scheme://host/direct/v1/car/{digest}`.
    ///
    /// # Panics
    ///
    /// Panics if `scheme` contains characters not permitted in a URL scheme.
    #[must_use]
    pub fn direct_car_locator(&self, scheme: &str, manifest_digest_hex: &str) -> DirectCarLocator {
        validate_scheme(scheme);
        let summary = self.to_summary();
        DirectCarLocator {
            canonical_url: format!(
                "{scheme}://{}/direct/v1/car/{manifest_digest_hex}",
                summary.canonical
            ),
            vanity_url: format!(
                "{scheme}://{}/direct/v1/car/{manifest_digest_hex}",
                summary.vanity
            ),
        }
    }
}

fn validate_scheme(scheme: &str) {
    if scheme.is_empty() {
        panic!("URL scheme must not be empty");
    }
    if !scheme
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'+' | b'-' | b'.'))
    {
        panic!("invalid characters in scheme `{scheme}`");
    }
}

/// Summary struct describing deterministic hostnames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMappingSummary {
    /// Canonical hostname derived from provider id and network.
    pub canonical: String,
    /// Vanity hostname exposed for direct-mode tooling.
    pub vanity: String,
}

/// Direct-CAR endpoints derived from host mapping inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCarLocator {
    /// Direct-CAR endpoint bound to the canonical host.
    pub canonical_url: String,
    /// Direct-CAR endpoint bound to the vanity host.
    pub vanity_url: String,
}

impl fmt::Display for DirectCarLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {}", self.canonical_url, self.vanity_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_host_mapping() {
        let provider = [0xAB; 32];
        let input = HostMappingInput {
            chain_id: "nexus",
            provider_id: &provider,
        };
        let summary = input.to_summary();
        assert_eq!(summary.canonical, "abababab.nexus.sorafs");
        assert_eq!(summary.vanity, "abab.nexus.direct.sorafs");
    }

    #[test]
    fn direct_car_locator_uses_scheme_and_digest() {
        let provider = [0x11; 32];
        let input = HostMappingInput {
            chain_id: "devnet",
            provider_id: &provider,
        };
        let locator = input.direct_car_locator("https", "deadbeef");
        assert_eq!(
            locator.canonical_url,
            "https://11111111.devnet.sorafs/direct/v1/car/deadbeef"
        );
        assert_eq!(
            locator.vanity_url,
            "https://1111.devnet.direct.sorafs/direct/v1/car/deadbeef"
        );
    }

    #[test]
    #[should_panic(expected = "URL scheme must not be empty")]
    fn locator_rejects_empty_scheme() {
        let provider = [0xFF; 32];
        let input = HostMappingInput {
            chain_id: "nexus",
            provider_id: &provider,
        };
        let _ = input.direct_car_locator("", "abcd");
    }
}
