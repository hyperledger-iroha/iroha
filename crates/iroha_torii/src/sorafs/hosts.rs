//! Deterministic host mapping scaffolding backed by shared manifest helpers.

use iroha_data_model::prelude::ChainId;
use sorafs_manifest::hosts::{
    DirectCarLocator, HostMappingInput as ManifestHostInput, HostMappingSummary as ManifestSummary,
};

/// Input parameters used to derive canonical and vanity hostnames.
#[derive(Debug, Clone)]
pub struct HostMappingInput<'a> {
    /// Network identifier (chain id).
    pub chain_id: &'a ChainId,
    /// Provider identifier recognised by governance.
    pub provider_id: &'a [u8; 32],
}

impl HostMappingInput<'_> {
    fn as_manifest_input(&self) -> ManifestHostInput<'_> {
        ManifestHostInput {
            chain_id: self.chain_id.as_str(),
            provider_id: self.provider_id,
        }
    }

    /// Compute a canonical host binding providers to their governance hash.
    #[must_use]
    pub fn canonical_host(&self) -> String {
        self.as_manifest_input().canonical_host()
    }

    /// Compute a developer-friendly vanity host seeded by the provider id.
    #[must_use]
    pub fn vanity_host(&self) -> String {
        self.as_manifest_input().vanity_host()
    }

    /// Render both canonical and vanity hosts as a struct suitable for JSON or templates.
    #[must_use]
    pub fn to_summary(&self) -> HostMappingSummary {
        self.as_manifest_input().to_summary()
    }

    /// Produce direct-CAR endpoints for the supplied manifest digest.
    ///
    /// # Errors
    /// Returns an error if the manifest digest or scheme cannot be mapped to a host.
    pub fn direct_car_locator(
        &self,
        scheme: &str,
        manifest_digest_hex: &str,
    ) -> Result<DirectCarLocator, HostMappingError> {
        self.as_manifest_input()
            .direct_car_locator(scheme, manifest_digest_hex)
    }
}

/// Helper structure used when serialising host mapping results.
pub type HostMappingSummary = ManifestSummary;
pub use sorafs_manifest::hosts::HostMappingError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_deterministic_hosts() {
        let chain_id: ChainId = "nexus".parse().expect("chain id");
        let provider = [0xAB; 32];
        let input = HostMappingInput {
            chain_id: &chain_id,
            provider_id: &provider,
        };

        assert_eq!(input.canonical_host(), "abababab.nexus.sorafs");
        assert_eq!(input.vanity_host(), "abab.nexus.direct.sorafs");

        let summary = input.to_summary();
        assert_eq!(summary.canonical, "abababab.nexus.sorafs");
        assert_eq!(summary.vanity, "abab.nexus.direct.sorafs");
    }

    #[test]
    fn shares_direct_car_locator() {
        let chain_id: ChainId = "nexus".parse().expect("chain id");
        let provider = [0xCD; 32];
        let input = HostMappingInput {
            chain_id: &chain_id,
            provider_id: &provider,
        };
        let locator = input
            .direct_car_locator("https", "feedface")
            .expect("locator");
        assert!(
            locator
                .canonical_url
                .starts_with("https://cdcdcdcd.nexus.sorafs/direct/v1/car/feedface")
        );
        assert!(
            locator
                .vanity_url
                .starts_with("https://cdcd.nexus.direct.sorafs/direct/v1/car/feedface")
        );
    }
}
