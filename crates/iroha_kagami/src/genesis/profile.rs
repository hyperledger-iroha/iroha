//! Profile presets for Iroha 3 genesis manifests.

use clap::ValueEnum;
use color_eyre::eyre::{Result, eyre};
use iroha_core::sumeragi::network_topology::redundant_send_r_from_len;
use iroha_crypto::Hash;
use iroha_data_model::prelude::ChainId;

/// Profile presets for `kagami genesis`/`kagami verify`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Keep network names explicit.
pub enum GenesisProfile {
    /// Local-only developer network.
    Iroha3Dev,
    /// Public Sora test network.
    Iroha3Testus,
    /// Sora Nexus main network.
    Iroha3Nexus,
}

/// Default knobs and validation rules derived from a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDefaults {
    /// Expected chain identifier.
    pub chain_id: ChainId,
    /// Collector quorum size.
    pub collectors_k: u16,
    /// Redundant send fanout.
    pub collectors_redundant_send_r: u8,
    /// Minimum number of unique peers (topology entries) required.
    pub min_peers: usize,
    /// How VRF seeds should be resolved for the profile.
    pub seed_policy: SeedPolicy,
}

/// VRF seed policy for a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedPolicy {
    /// Derive the VRF seed deterministically from the chain id when not provided explicitly.
    DerivedFromChain,
    /// Require an explicit seed to be supplied.
    RequireExplicit,
}

/// Return the defaults and validation rules for the given profile.
#[must_use]
pub fn profile_defaults(profile: GenesisProfile) -> ProfileDefaults {
    match profile {
        GenesisProfile::Iroha3Dev => ProfileDefaults {
            chain_id: ChainId::from("iroha3-dev.local"),
            collectors_k: 1,
            min_peers: 1,
            collectors_redundant_send_r: redundant_send_r_from_len(1),
            seed_policy: SeedPolicy::DerivedFromChain,
        },
        GenesisProfile::Iroha3Testus => ProfileDefaults {
            chain_id: ChainId::from("iroha3-testus"),
            collectors_k: 3,
            min_peers: 4,
            collectors_redundant_send_r: redundant_send_r_from_len(4),
            seed_policy: SeedPolicy::RequireExplicit,
        },
        GenesisProfile::Iroha3Nexus => ProfileDefaults {
            chain_id: ChainId::from("iroha3-nexus"),
            collectors_k: 5,
            min_peers: 4,
            collectors_redundant_send_r: redundant_send_r_from_len(4),
            seed_policy: SeedPolicy::RequireExplicit,
        },
    }
}

/// Whether the profile targets the public Sora Nexus dataspace (NPoS-only).
#[must_use]
pub fn profile_requires_npos(profile: GenesisProfile) -> bool {
    matches!(profile, GenesisProfile::Iroha3Nexus)
}

/// Parse a hex-encoded VRF seed into the fixed 32-byte array required by `SumeragiNposParameters`.
///
/// # Errors
///
/// Returns an error when the input is not valid hex or does not represent exactly 32 bytes.
pub fn parse_vrf_seed_hex(hex: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex).map_err(|err| eyre!("invalid hex for VRF seed: {err}"))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| eyre!("VRF seed must be exactly 32 bytes (hex length 64)"))?;
    Ok(seed)
}

/// Derive a deterministic VRF seed from the provided chain identifier.
#[must_use]
pub fn derive_vrf_seed_from_chain(chain_id: &ChainId) -> [u8; 32] {
    let hash = Hash::new(chain_id.as_str());
    *hash.as_ref()
}

/// Resolve the VRF seed to use for the profile according to its policy and an optional override.
///
/// # Errors
///
/// Returns an error when the profile requires an explicit seed and none was provided.
pub fn resolve_vrf_seed(
    profile: GenesisProfile,
    chain_id: &ChainId,
    override_seed: Option<[u8; 32]>,
) -> Result<[u8; 32]> {
    match (profile_defaults(profile).seed_policy, override_seed) {
        (_, Some(seed)) => Ok(seed),
        (SeedPolicy::DerivedFromChain, None) => Ok(derive_vrf_seed_from_chain(chain_id)),
        (SeedPolicy::RequireExplicit, None) => Err(eyre!(
            "profile {profile:?} requires `--vrf-seed-hex` to supply a 32-byte VRF seed"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_defaults_assign_expected_values() {
        let dev = profile_defaults(GenesisProfile::Iroha3Dev);
        assert_eq!(dev.chain_id, ChainId::from("iroha3-dev.local"));
        assert_eq!(dev.collectors_k, 1);
        assert_eq!(dev.collectors_redundant_send_r, 1);
        assert_eq!(dev.min_peers, 1);

        let testus = profile_defaults(GenesisProfile::Iroha3Testus);
        assert_eq!(testus.chain_id, ChainId::from("iroha3-testus"));
        assert_eq!(testus.collectors_k, 3);
        assert_eq!(testus.collectors_redundant_send_r, 3);
        assert_eq!(testus.min_peers, 4);

        let nexus = profile_defaults(GenesisProfile::Iroha3Nexus);
        assert_eq!(nexus.chain_id, ChainId::from("iroha3-nexus"));
        assert_eq!(nexus.collectors_k, 5);
        assert_eq!(nexus.collectors_redundant_send_r, 3);
        assert_eq!(nexus.min_peers, 4);
    }

    #[test]
    fn derived_seed_depends_on_chain_id() {
        let a = derive_vrf_seed_from_chain(&ChainId::from("chain-a"));
        let b = derive_vrf_seed_from_chain(&ChainId::from("chain-b"));
        assert_ne!(a, b, "seeds derived from different chains must differ");
    }

    #[test]
    fn require_explicit_seed_errors_without_override() {
        let err = resolve_vrf_seed(
            GenesisProfile::Iroha3Nexus,
            &ChainId::from("iroha3-nexus"),
            None,
        )
        .expect_err("explicit seed should be required for nexus");
        assert!(
            err.to_string().contains("vrf-seed-hex"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_prefers_override_when_provided() {
        let override_seed = [7u8; 32];
        let resolved = resolve_vrf_seed(
            GenesisProfile::Iroha3Dev,
            &ChainId::from("iroha3-dev.local"),
            Some(override_seed),
        )
        .expect("override should be accepted");
        assert_eq!(resolved, override_seed);
    }

    #[test]
    fn profile_requires_npos_only_for_nexus() {
        assert!(!profile_requires_npos(GenesisProfile::Iroha3Dev));
        assert!(!profile_requires_npos(GenesisProfile::Iroha3Testus));
        assert!(profile_requires_npos(GenesisProfile::Iroha3Nexus));
    }
}
