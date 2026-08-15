//! Profile presets for Iroha 3 genesis manifests.
use clap::ValueEnum;
use color_eyre::eyre::{Result, eyre};
use core::num::NonZeroU64;
use iroha_crypto::Hash;
use iroha_data_model::{asset::AssetDefinitionId, prelude::ChainId};
/// Canonical I105 discriminant for the public Taira testnet.
pub const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
/// Canonical I105 discriminant for the public Nexus mainnet.
pub const NEXUS_CHAIN_DISCRIMINANT: u16 = 753;
/// Live public Taira XOR asset definition id.
pub const TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
/// Public XOR alias selector used by Nexus/Taira configs.
pub const PUBLIC_XOR_ALIAS: &str = "xor#universal";
/// Public XOR domain registered in public-profile genesis manifests.
pub const PUBLIC_XOR_DOMAIN: &str = "universal.universal";
const PUBLIC_TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const ARCHIVED_TAIRA_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
const PUBLIC_NEXUS_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000753";
const PK2_NEXUS_CHAIN_ID: &str = "cbdc16";
/// Profile presets for `kagami genesis`/`kagami verify`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Keep network names explicit.
pub enum GenesisProfile {
    /// Local-only developer network.
    Iroha3Dev,
    /// Public Sora test network.
    Iroha3Taira,
    /// Sora Nexus main network.
    Iroha3Nexus,
}
/// Default knobs and validation rules derived from a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDefaults {
    /// Expected chain identifier.
    pub chain_id: ChainId,
    /// Optional canonical I105 chain discriminant to use when rendering account literals.
    pub chain_discriminant: Option<u16>,
    /// Genesis-selected target block cadence in milliseconds.
    pub block_cadence_ms: NonZeroU64,
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
            chain_discriminant: None,
            block_cadence_ms: NonZeroU64::new(100).unwrap(),
            min_peers: 4,
            seed_policy: SeedPolicy::DerivedFromChain,
        },
        GenesisProfile::Iroha3Taira => ProfileDefaults {
            chain_id: ChainId::from("iroha3-taira"),
            chain_discriminant: Some(TAIRA_CHAIN_DISCRIMINANT),
            block_cadence_ms: NonZeroU64::new(1_000).unwrap(),
            min_peers: 4,
            seed_policy: SeedPolicy::RequireExplicit,
        },
        GenesisProfile::Iroha3Nexus => ProfileDefaults {
            chain_id: ChainId::from("iroha3-nexus"),
            chain_discriminant: Some(NEXUS_CHAIN_DISCRIMINANT),
            block_cadence_ms: NonZeroU64::new(100).unwrap(),
            min_peers: 4,
            seed_policy: SeedPolicy::RequireExplicit,
        },
    }
}
/// Whether the profile targets the public Sora Nexus dataspace (NPoS-only).
#[must_use]
pub fn profile_requires_npos(profile: GenesisProfile) -> bool {
    matches!(profile, GenesisProfile::Iroha3Nexus)
}
/// Whether the profile represents a public network whose XOR must be canonical.
#[must_use]
pub fn profile_uses_public_xor(profile: GenesisProfile) -> bool {
    matches!(
        profile,
        GenesisProfile::Iroha3Taira | GenesisProfile::Iroha3Nexus
    )
}
/// Return the public profile associated with a known public chain id.
#[must_use]
pub fn public_xor_profile_for_chain_id(chain_id: &str) -> Option<GenesisProfile> {
    match chain_id {
        "iroha3-taira" | PUBLIC_TAIRA_CHAIN_ID => Some(GenesisProfile::Iroha3Taira),
        "iroha3-nexus" | PUBLIC_NEXUS_CHAIN_ID | PK2_NEXUS_CHAIN_ID => {
            Some(GenesisProfile::Iroha3Nexus)
        }
        _ => None,
    }
}
/// Default canonical XOR asset id for public profiles where it is known.
///
/// # Errors
///
/// Returns an error if the built-in literal stops parsing as a canonical asset definition id.
pub fn default_public_xor_asset_definition_id(
    profile: GenesisProfile,
) -> Result<Option<AssetDefinitionId>> {
    match profile {
        GenesisProfile::Iroha3Taira => Ok(Some(
            AssetDefinitionId::parse_address_literal(TAIRA_XOR_ASSET_DEFINITION_ID)
                .map_err(|err| eyre!("built-in Taira XOR asset definition id is invalid: {err}"))?,
        )),
        GenesisProfile::Iroha3Nexus | GenesisProfile::Iroha3Dev => Ok(None),
    }
}
/// Resolve the canonical public XOR id selected for a profile run.
///
/// # Errors
///
/// Returns an error when a supplied literal is not canonical Base58, when a public Nexus NPoS
/// manifest omits the id, or when the flag is used for a non-public/non-NPoS manifest.
pub fn resolve_public_xor_asset_definition_id(
    profile: Option<GenesisProfile>,
    configured: Option<&str>,
    wants_npos: bool,
) -> Result<Option<AssetDefinitionId>> {
    let Some(profile) = profile else {
        if configured.is_some() {
            return Err(eyre!(
                "`--xor-asset-definition-id` is only supported with public Iroha3 profiles"
            ));
        }
        return Ok(None);
    };
    if !profile_uses_public_xor(profile) {
        if configured.is_some() {
            return Err(eyre!(
                "`--xor-asset-definition-id` applies only to iroha3-taira/iroha3-nexus profiles"
            ));
        }
        return Ok(None);
    }
    if !wants_npos {
        if configured.is_some() {
            return Err(eyre!(
                "`--xor-asset-definition-id` applies only to NPoS public-profile manifests"
            ));
        }
        return Ok(None);
    }
    if let Some(configured) = configured {
        return AssetDefinitionId::parse_address_literal(configured)
            .map(Some)
            .map_err(|err| {
                eyre!(
                    "invalid --xor-asset-definition-id `{configured}`: {err}; expected canonical unprefixed Base58 asset definition id, not an alias such as `{PUBLIC_XOR_ALIAS}`"
                )
            });
    }
    if let Some(default) = default_public_xor_asset_definition_id(profile)? {
        return Ok(Some(default));
    }
    Err(eyre!(
        "profile {profile:?} requires `--xor-asset-definition-id <BASE58>` so public XOR is bound to a real canonical asset definition"
    ))
}
/// Return the canonical I105 chain discriminant for well-known network ids.
#[must_use]
pub fn known_chain_discriminant_for_chain_id(chain_id: &str) -> Option<u16> {
    match chain_id {
        // Keep the archived UUID readable with its historical I105 prefix. It is
        // deliberately excluded from `public_xor_profile_for_chain_id`, so new
        // public-profile genesis bundles cannot select the retired network.
        "iroha3-taira" | PUBLIC_TAIRA_CHAIN_ID | ARCHIVED_TAIRA_CHAIN_ID => {
            Some(TAIRA_CHAIN_DISCRIMINANT)
        }
        "iroha3-nexus" | PUBLIC_NEXUS_CHAIN_ID | PK2_NEXUS_CHAIN_ID => {
            Some(NEXUS_CHAIN_DISCRIMINANT)
        }
        _ => None,
    }
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
        assert_eq!(dev.chain_discriminant, None);
        assert_eq!(dev.block_cadence_ms.get(), 100);
        assert_eq!(dev.min_peers, 4);
        let taira = profile_defaults(GenesisProfile::Iroha3Taira);
        assert_eq!(taira.chain_id, ChainId::from("iroha3-taira"));
        assert_eq!(taira.chain_discriminant, Some(TAIRA_CHAIN_DISCRIMINANT));
        assert_eq!(taira.block_cadence_ms.get(), 1_000);
        assert_eq!(taira.min_peers, 4);
        let nexus = profile_defaults(GenesisProfile::Iroha3Nexus);
        assert_eq!(nexus.chain_id, ChainId::from("iroha3-nexus"));
        assert_eq!(nexus.chain_discriminant, Some(NEXUS_CHAIN_DISCRIMINANT));
        assert_eq!(nexus.block_cadence_ms.get(), 100);
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
        assert!(!profile_requires_npos(GenesisProfile::Iroha3Taira));
        assert!(profile_requires_npos(GenesisProfile::Iroha3Nexus));
    }
    #[test]
    fn public_xor_profiles_are_classified() {
        assert!(!profile_uses_public_xor(GenesisProfile::Iroha3Dev));
        assert!(profile_uses_public_xor(GenesisProfile::Iroha3Taira));
        assert!(profile_uses_public_xor(GenesisProfile::Iroha3Nexus));
        assert_eq!(
            public_xor_profile_for_chain_id("iroha3-taira"),
            Some(GenesisProfile::Iroha3Taira)
        );
    }
    #[test]
    fn public_xor_resolver_defaults_taira_and_requires_nexus() {
        let taira =
            resolve_public_xor_asset_definition_id(Some(GenesisProfile::Iroha3Taira), None, true)
                .expect("Taira should use built-in live XOR id")
                .expect("Taira should resolve an id");
        assert_eq!(taira.to_string(), TAIRA_XOR_ASSET_DEFINITION_ID);
        let err =
            resolve_public_xor_asset_definition_id(Some(GenesisProfile::Iroha3Nexus), None, true)
                .expect_err("Nexus must require an explicit XOR id");
        assert!(
            err.to_string().contains("--xor-asset-definition-id"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn public_xor_resolver_rejects_alias_literal() {
        let err = resolve_public_xor_asset_definition_id(
            Some(GenesisProfile::Iroha3Taira),
            Some(PUBLIC_XOR_ALIAS),
            true,
        )
        .expect_err("alias selector is not a canonical Base58 id");
        assert!(
            err.to_string()
                .contains("expected canonical unprefixed Base58"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn known_chain_discriminant_maps_taira_and_nexus() {
        assert_eq!(
            known_chain_discriminant_for_chain_id("fc56984b-2be7-431d-840e-21514d1883f0"),
            Some(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            known_chain_discriminant_for_chain_id(ARCHIVED_TAIRA_CHAIN_ID),
            Some(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            known_chain_discriminant_for_chain_id("iroha3-taira"),
            Some(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            known_chain_discriminant_for_chain_id("00000000-0000-0000-0000-000000000753"),
            Some(NEXUS_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            known_chain_discriminant_for_chain_id("iroha3-nexus"),
            Some(NEXUS_CHAIN_DISCRIMINANT)
        );
        assert_eq!(
            known_chain_discriminant_for_chain_id("cbdc16"),
            Some(NEXUS_CHAIN_DISCRIMINANT)
        );
        assert_eq!(known_chain_discriminant_for_chain_id("unknown"), None);
    }
}
