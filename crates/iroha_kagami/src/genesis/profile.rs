//! Profile presets for Iroha 3 genesis manifests.
use clap::ValueEnum;
use color_eyre::eyre::{Result, eyre};
use core::num::NonZeroU64;
use iroha_crypto::Hash;
use iroha_data_model::{
    asset::{
        Asset, AssetBalancePolicy, AssetBalanceScope, AssetDefinitionAlias, AssetDefinitionId,
        AssetId, NewAssetDefinition,
    },
    domain::DomainId,
    isi::{Mint, MintBox, Register, asset_alias::SetAssetDefinitionAlias},
    nexus::DataSpaceId,
    prelude::{AssetDefinition, ChainId, Domain, NumericSpec},
};
use iroha_genesis::RawGenesisTransaction;
use iroha_primitives::numeric::Quantity;
/// Canonical I105 discriminant for the public Taira testnet.
pub const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
/// Canonical I105 discriminant for the public Nexus mainnet.
pub const NEXUS_CHAIN_DISCRIMINANT: u16 = 753;
/// Live public Taira XOR asset definition id.
pub const TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
/// Canonical decimal scale for public Taira XOR quantities.
pub const TAIRA_XOR_SCALE: u32 = 9;
/// Public XOR alias selector used by Nexus/Taira configs.
pub const PUBLIC_XOR_ALIAS: &str = "xor#universal";
/// Public XOR domain registered in public-profile genesis manifests.
pub const PUBLIC_XOR_DOMAIN: &str = "universal.universal";
/// Canonical opaque Digital Kina asset-definition id for the public Taira BPNG profile.
pub const TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID: &str = "839FV3NJC8NfgWQvghXU2hEFQm9a";
/// Exact Digital Kina alias exposed by public Taira.
pub const TAIRA_DIGITAL_KINA_ALIAS: &str = "kina#bpng";
/// Immutable owning domain used to derive the public Taira Digital Kina id.
pub const TAIRA_DIGITAL_KINA_DOMAIN: &str = "bpng.bpng";
/// Human-readable asset name bound to [`TAIRA_DIGITAL_KINA_ALIAS`].
pub const TAIRA_DIGITAL_KINA_NAME: &str = "kina";
/// Canonical decimal scale for Digital Kina quantities.
pub const TAIRA_DIGITAL_KINA_SCALE: u32 = 2;
/// Physical BPNG dataspace that owns Digital Kina balance buckets on public Taira.
pub const TAIRA_DIGITAL_KINA_DATASPACE_ID: u64 = 10;
/// Retired spellings that must never be accepted as the canonical Digital Kina alias.
pub const RETIRED_TAIRA_DIGITAL_KINA_ALIASES: &[&str] = &[
    "digital_kina#bpng",
    "digital-kina#bpng",
    "pgk#bpng",
    "kina#dpn",
];
const PUBLIC_TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
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
            chain_id: ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
            chain_discriminant: Some(TAIRA_CHAIN_DISCRIMINANT),
            block_cadence_ms: NonZeroU64::new(4_000).unwrap(),
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
/// Whether the profile targets a public Sora network, which is NPoS-only.
#[must_use]
pub fn profile_requires_npos(profile: GenesisProfile) -> bool {
    matches!(
        profile,
        GenesisProfile::Iroha3Taira | GenesisProfile::Iroha3Nexus
    )
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
        PUBLIC_TAIRA_CHAIN_ID => Some(GenesisProfile::Iroha3Taira),
        "iroha3-nexus" | PUBLIC_NEXUS_CHAIN_ID | PK2_NEXUS_CHAIN_ID => {
            Some(GenesisProfile::Iroha3Nexus)
        }
        _ => None,
    }
}

type TairaDigitalKinaIdentity = (AssetDefinitionId, AssetDefinitionAlias, DomainId);

fn canonical_taira_digital_kina_identity() -> Result<TairaDigitalKinaIdentity> {
    let expected_id =
        AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
            .map_err(|err| eyre!("built-in Taira Digital Kina id is invalid: {err}"))?;
    let expected_alias: AssetDefinitionAlias = TAIRA_DIGITAL_KINA_ALIAS.parse()?;
    let expected_domain = DomainId::parse_fully_qualified(TAIRA_DIGITAL_KINA_DOMAIN)?;
    let derived_id = AssetDefinitionId::derive_from_components(
        expected_domain.clone(),
        TAIRA_DIGITAL_KINA_NAME.parse()?,
    );
    if derived_id != expected_id {
        return Err(eyre!(
            "built-in Taira Digital Kina id `{expected_id}` does not match the canonical `{TAIRA_DIGITAL_KINA_DOMAIN}`/`{TAIRA_DIGITAL_KINA_NAME}` derivation `{derived_id}`"
        ));
    }
    Ok((expected_id, expected_alias, expected_domain))
}

fn validate_taira_digital_kina_definition(
    definition: &NewAssetDefinition,
    expected_id: &AssetDefinitionId,
    expected_alias: &AssetDefinitionAlias,
    expected_domain: &DomainId,
) -> Result<()> {
    if let Some(alias) = definition.alias.as_ref()
        && RETIRED_TAIRA_DIGITAL_KINA_ALIASES.contains(&alias.as_ref())
    {
        return Err(eyre!(
            "retired Taira Digital Kina alias `{alias}` is forbidden; use `{TAIRA_DIGITAL_KINA_ALIAS}`"
        ));
    }
    if definition.alias.as_ref() == Some(expected_alias) && &definition.id != expected_id {
        return Err(eyre!(
            "Taira Digital Kina alias `{TAIRA_DIGITAL_KINA_ALIAS}` must target `{expected_id}`, found `{}`",
            definition.id
        ));
    }
    if &definition.id != expected_id {
        return Ok(());
    }
    if definition.alias.as_ref() != Some(expected_alias) {
        let found = definition
            .alias
            .as_ref()
            .map_or_else(|| "<missing>".to_owned(), ToString::to_string);
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` registration must atomically bind `{TAIRA_DIGITAL_KINA_ALIAS}`, found `{found}`"
        ));
    }
    if definition.name != TAIRA_DIGITAL_KINA_NAME {
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` must have name `{TAIRA_DIGITAL_KINA_NAME}`, found `{}`",
            definition.name
        ));
    }
    let expected_spec = NumericSpec::fractional(TAIRA_DIGITAL_KINA_SCALE);
    if definition.spec != expected_spec {
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` must use numeric spec {expected_spec:?}, found {:?}",
            definition.spec
        ));
    }
    if definition.balance_scope_policy != AssetBalancePolicy::DataspaceRestricted {
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` must use DataspaceRestricted balances"
        ));
    }
    if definition.owning_domain.as_ref() != Some(expected_domain) {
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` must be owned by `{TAIRA_DIGITAL_KINA_DOMAIN}`"
        ));
    }
    Ok(())
}

fn validate_taira_digital_kina_mint_scope(
    destination: &AssetId,
    expected_id: &AssetDefinitionId,
) -> Result<()> {
    if destination.definition() != expected_id {
        return Ok(());
    }
    let expected_scope =
        AssetBalanceScope::Dataspace(DataSpaceId::new(TAIRA_DIGITAL_KINA_DATASPACE_ID));
    if destination.scope() != &expected_scope {
        return Err(eyre!(
            "Taira Digital Kina `{expected_id}` genesis mints must target explicit `#dataspace:{TAIRA_DIGITAL_KINA_DATASPACE_ID}` balance buckets"
        ));
    }
    Ok(())
}

fn reject_taira_base_digital_kina_definition(
    definition: &NewAssetDefinition,
    expected_id: &AssetDefinitionId,
    expected_alias: &AssetDefinitionAlias,
    expected_domain: &DomainId,
) -> Result<()> {
    let forbidden_alias = definition.alias.as_ref().is_some_and(|alias| {
        alias == expected_alias || RETIRED_TAIRA_DIGITAL_KINA_ALIASES.contains(&alias.as_ref())
    });
    if &definition.id == expected_id
        || forbidden_alias
        || (definition.name == TAIRA_DIGITAL_KINA_NAME
            && definition.owning_domain.as_ref() == Some(expected_domain))
    {
        return Err(eyre!(
            "base Taira genesis must leave Digital Kina absent for the reviewed post-health provisioning stage"
        ));
    }
    Ok(())
}

/// Prove that base Taira genesis contains the BPNG owning-domain prerequisite but no Digital Kina.
///
/// Digital Kina is deliberately stage-owned: the reviewed deployment corridor records three
/// distinct post-health transactions for definition registration, operational permission, and
/// initial mint. Embedding any part of that state in base genesis would bypass the absence proof
/// and make the provisioning stage fail closed.
///
/// # Errors
///
/// Returns an error when `bpng.bpng` is missing or duplicated, or when base genesis already
/// contains the canonical/retired alias, definition, or a balance for the pinned Digital Kina id.
pub fn validate_taira_digital_kina_base_prerequisite(
    manifest: &RawGenesisTransaction,
) -> Result<DomainId> {
    let (expected_id, expected_alias, expected_domain) = canonical_taira_digital_kina_identity()?;
    let mut domain_registration_count = 0_usize;
    for instruction in manifest.instructions() {
        if let Some(register) = instruction.as_any().downcast_ref::<Register<Domain>>() {
            if register.object.id == expected_domain {
                domain_registration_count += 1;
            }
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<Register<AssetDefinition>>()
        {
            reject_taira_base_digital_kina_definition(
                &register.object,
                &expected_id,
                &expected_alias,
                &expected_domain,
            )?;
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::register::RegisterBox>()
        {
            match register {
                iroha_data_model::isi::register::RegisterBox::Domain(register)
                    if register.object.id == expected_domain =>
                {
                    domain_registration_count += 1;
                }
                iroha_data_model::isi::register::RegisterBox::AssetDefinition(register) => {
                    reject_taira_base_digital_kina_definition(
                        &register.object,
                        &expected_id,
                        &expected_alias,
                        &expected_domain,
                    )?;
                }
                _ => {}
            }
            continue;
        }
        if let Some(mint) = instruction.as_any().downcast_ref::<Mint<Quantity, Asset>>() {
            if mint.destination.definition() == &expected_id {
                return Err(eyre!(
                    "base Taira genesis must not mint stage-owned Digital Kina"
                ));
            }
            continue;
        }
        if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
            if mint.destination().definition() == &expected_id {
                return Err(eyre!(
                    "base Taira genesis must not mint stage-owned Digital Kina"
                ));
            }
            continue;
        }
        let Some(binding) = instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
        else {
            continue;
        };
        let forbidden_alias = binding.alias.as_ref().is_some_and(|alias| {
            alias == &expected_alias || RETIRED_TAIRA_DIGITAL_KINA_ALIASES.contains(&alias.as_ref())
        });
        if forbidden_alias || binding.asset_definition_id == expected_id {
            return Err(eyre!(
                "base Taira genesis must not bind canonical or retired Digital Kina aliases before the reviewed provisioning stage"
            ));
        }
    }
    if domain_registration_count != 1 {
        return Err(eyre!(
            "base Taira genesis must register Digital Kina prerequisite domain `{expected_domain}` exactly once; found {domain_registration_count}"
        ));
    }
    Ok(expected_domain)
}

/// Prove that a post-provision Taira replay manifest registers and binds Digital Kina exactly.
///
/// This is not a base-genesis requirement. It is intended for explicit combined/replay artifacts
/// produced after the reviewed three-transaction Digital Kina stage. Taira retains canonical XOR
/// for consensus economics while BPNG payments use a separate opaque raw id, human name, and
/// alias.
///
/// # Errors
///
/// Returns an error for a missing, duplicate, substituted, or retired Digital Kina binding, or
/// when the registered asset definition does not match the pinned BPNG domain, scale, and balance
/// policy.
pub fn validate_taira_post_provision_digital_kina_binding(
    manifest: &RawGenesisTransaction,
) -> Result<AssetDefinitionId> {
    let (expected_id, expected_alias, expected_domain) = canonical_taira_digital_kina_identity()?;

    let mut domain_registration_count = 0_usize;
    let mut registration_count = 0_usize;
    let mut domain_registration_index = None;
    let mut registration_index = None;
    for (index, instruction) in manifest.instructions().enumerate() {
        if let Some(register) = instruction.as_any().downcast_ref::<Register<Domain>>() {
            if register.object.id == expected_domain {
                domain_registration_count += 1;
                domain_registration_index.get_or_insert(index);
            }
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<Register<AssetDefinition>>()
        {
            validate_taira_digital_kina_definition(
                &register.object,
                &expected_id,
                &expected_alias,
                &expected_domain,
            )?;
            if register.object.id == expected_id {
                registration_count += 1;
                registration_index.get_or_insert(index);
            }
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::register::RegisterBox>()
        {
            match register {
                iroha_data_model::isi::register::RegisterBox::Domain(register)
                    if register.object.id == expected_domain =>
                {
                    domain_registration_count += 1;
                    domain_registration_index.get_or_insert(index);
                }
                iroha_data_model::isi::register::RegisterBox::AssetDefinition(register) => {
                    validate_taira_digital_kina_definition(
                        &register.object,
                        &expected_id,
                        &expected_alias,
                        &expected_domain,
                    )?;
                    if register.object.id == expected_id {
                        registration_count += 1;
                        registration_index.get_or_insert(index);
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Some(mint) = instruction.as_any().downcast_ref::<Mint<Quantity, Asset>>() {
            validate_taira_digital_kina_mint_scope(&mint.destination, &expected_id)?;
            continue;
        }
        if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
            validate_taira_digital_kina_mint_scope(mint.destination(), &expected_id)?;
            continue;
        }
        let Some(binding) = instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
        else {
            continue;
        };
        if let Some(alias) = binding.alias.as_ref()
            && RETIRED_TAIRA_DIGITAL_KINA_ALIASES.contains(&alias.as_ref())
        {
            return Err(eyre!(
                "retired Taira Digital Kina alias `{alias}` is forbidden; use `{TAIRA_DIGITAL_KINA_ALIAS}`"
            ));
        }
        if binding.alias.as_ref() == Some(&expected_alias)
            || binding.asset_definition_id == expected_id
        {
            return Err(eyre!(
                "post-provision Digital Kina alias must be bound atomically by its reviewed Register.AssetDefinition transaction, not a separate SetAssetDefinitionAlias instruction"
            ));
        }
    }
    if domain_registration_count != 1 {
        return Err(eyre!(
            "Taira genesis must register Digital Kina owning domain `{expected_domain}` exactly once; found {domain_registration_count}"
        ));
    }
    if registration_count != 1 {
        return Err(eyre!(
            "Taira genesis must register Digital Kina `{expected_id}` exactly once; found {registration_count}"
        ));
    }
    let domain_registration_index = domain_registration_index.expect("validated count is one");
    let registration_index = registration_index.expect("validated count is one");
    if domain_registration_index >= registration_index {
        return Err(eyre!(
            "post-provision Digital Kina replay must register `{expected_domain}` before the atomic `{expected_id}`/`{TAIRA_DIGITAL_KINA_ALIAS}` definition"
        ));
    }
    Ok(expected_id)
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
        let parsed = AssetDefinitionId::parse_address_literal(configured).map_err(|err| {
                eyre!(
                    "invalid --xor-asset-definition-id `{configured}`: {err}; expected canonical unprefixed Base58 asset definition id, not an alias such as `{PUBLIC_XOR_ALIAS}`"
                )
            })?;
        if profile == GenesisProfile::Iroha3Taira
            && parsed.to_string() != TAIRA_XOR_ASSET_DEFINITION_ID
        {
            return Err(eyre!(
                "public Taira XOR asset definition id is pinned to `{TAIRA_XOR_ASSET_DEFINITION_ID}`; found `{parsed}`"
            ));
        }
        return Ok(Some(parsed));
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
        PUBLIC_TAIRA_CHAIN_ID => Some(TAIRA_CHAIN_DISCRIMINANT),
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
    use iroha_data_model::{asset::AssetBalancePolicy, prelude::Metadata};
    use iroha_genesis::GenesisBuilder;
    use iroha_test_samples::ALICE_ID;
    use std::path::PathBuf;

    fn taira_digital_kina_manifest(
        alias: &str,
        target: AssetDefinitionId,
        register_domain: bool,
        mint_scope: Option<AssetBalanceScope>,
    ) -> RawGenesisTransaction {
        let definition_id = target;
        let owning_domain = DomainId::parse_fully_qualified(TAIRA_DIGITAL_KINA_DOMAIN)
            .expect("pinned Digital Kina domain");
        let mut builder = GenesisBuilder::new_without_executor(
            ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
            PathBuf::from("."),
        );
        if register_domain {
            builder =
                builder.append_instruction(Register::domain(Domain::new(owning_domain.clone())));
        }
        builder = builder.append_instruction(Register::asset_definition(
            AssetDefinition::new(
                definition_id.clone(),
                TAIRA_DIGITAL_KINA_NAME.to_owned(),
                NumericSpec::fractional(TAIRA_DIGITAL_KINA_SCALE),
                AssetBalancePolicy::DataspaceRestricted,
                Some(owning_domain),
            )
            .with_alias(Some(alias.parse().expect("test alias")))
            .with_metadata(Metadata::default()),
        ));
        if let Some(scope) = mint_scope {
            builder = builder.append_instruction(Mint::asset_quantity(
                1_u32,
                AssetId::with_scope(definition_id, ALICE_ID.clone(), scope),
            ));
        }
        builder.build_raw()
    }
    #[test]
    fn profile_defaults_assign_expected_values() {
        let dev = profile_defaults(GenesisProfile::Iroha3Dev);
        assert_eq!(dev.chain_id, ChainId::from("iroha3-dev.local"));
        assert_eq!(dev.chain_discriminant, None);
        assert_eq!(dev.block_cadence_ms.get(), 100);
        assert_eq!(dev.min_peers, 4);
        let taira = profile_defaults(GenesisProfile::Iroha3Taira);
        assert_eq!(taira.chain_id, ChainId::from(PUBLIC_TAIRA_CHAIN_ID));
        assert_eq!(taira.chain_discriminant, Some(TAIRA_CHAIN_DISCRIMINANT));
        assert_eq!(taira.block_cadence_ms.get(), 4_000);
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
    fn public_profiles_require_npos() {
        assert!(!profile_requires_npos(GenesisProfile::Iroha3Dev));
        assert!(profile_requires_npos(GenesisProfile::Iroha3Taira));
        assert!(profile_requires_npos(GenesisProfile::Iroha3Nexus));
    }
    #[test]
    fn public_xor_profiles_are_classified() {
        assert!(!profile_uses_public_xor(GenesisProfile::Iroha3Dev));
        assert!(profile_uses_public_xor(GenesisProfile::Iroha3Taira));
        assert!(profile_uses_public_xor(GenesisProfile::Iroha3Nexus));
        assert_eq!(
            public_xor_profile_for_chain_id(PUBLIC_TAIRA_CHAIN_ID),
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
    fn public_xor_resolver_rejects_a_substituted_taira_asset() {
        let err = resolve_public_xor_asset_definition_id(
            Some(GenesisProfile::Iroha3Taira),
            Some("61CtjvNd9T3THAR65GsMVHr82Bjc"),
            true,
        )
        .expect_err("Taira XOR identity must be pinned before first release");
        assert!(
            err.to_string().contains(TAIRA_XOR_ASSET_DEFINITION_ID),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn taira_base_keeps_bpng_domain_but_defers_digital_kina_provisioning() {
        let owning_domain = DomainId::parse_fully_qualified(TAIRA_DIGITAL_KINA_DOMAIN)
            .expect("pinned Digital Kina domain");
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
            PathBuf::from("."),
        )
        .append_instruction(Register::domain(Domain::new(owning_domain.clone())))
        .build_raw();
        assert_eq!(
            validate_taira_digital_kina_base_prerequisite(&manifest)
                .expect("base prerequisite and absence"),
            owning_domain
        );
    }
    #[test]
    fn taira_base_rejects_preprovisioned_digital_kina() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest =
            taira_digital_kina_manifest(TAIRA_DIGITAL_KINA_ALIAS, expected_id, true, None);
        let error = validate_taira_digital_kina_base_prerequisite(&manifest)
            .expect_err("base genesis must preserve the stage-owned absence proof");
        assert!(
            error.to_string().contains("post-health provisioning stage"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_base_rejects_retired_digital_kina_aliases() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let owning_domain = DomainId::parse_fully_qualified(TAIRA_DIGITAL_KINA_DOMAIN)
            .expect("pinned Digital Kina domain");
        for retired in RETIRED_TAIRA_DIGITAL_KINA_ALIASES {
            let manifest = GenesisBuilder::new_without_executor(
                ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
                PathBuf::from("."),
            )
            .append_instruction(Register::domain(Domain::new(owning_domain.clone())))
            .append_instruction(SetAssetDefinitionAlias::bind(
                expected_id.clone(),
                retired.parse().expect("retired alias fixture"),
                None,
            ))
            .build_raw();
            let error = validate_taira_digital_kina_base_prerequisite(&manifest)
                .expect_err("retired aliases must fail closed before provisioning");
            assert!(
                error.to_string().contains("canonical or retired"),
                "unexpected error for {retired}: {error}"
            );
        }
    }
    #[test]
    fn taira_digital_kina_binding_pins_raw_name_alias_domain_and_scale() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest =
            taira_digital_kina_manifest(TAIRA_DIGITAL_KINA_ALIAS, expected_id.clone(), true, None);
        assert_eq!(
            validate_taira_post_provision_digital_kina_binding(&manifest)
                .expect("canonical binding"),
            expected_id
        );
    }
    #[test]
    fn taira_digital_kina_binding_rejects_a_separate_fourth_alias_instruction() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest =
            taira_digital_kina_manifest(TAIRA_DIGITAL_KINA_ALIAS, expected_id.clone(), true, None)
                .into_builder()
                .next_transaction()
                .append_instruction(SetAssetDefinitionAlias::bind(
                    expected_id,
                    TAIRA_DIGITAL_KINA_ALIAS.parse().expect("canonical alias"),
                    None,
                ))
                .build_raw();
        let error = validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect_err("the reviewed register transaction must bind the alias atomically");
        assert!(
            error.to_string().contains("not a separate"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_digital_kina_binding_rejects_retired_aliases() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        for retired in RETIRED_TAIRA_DIGITAL_KINA_ALIASES {
            let manifest = taira_digital_kina_manifest(retired, expected_id.clone(), true, None);
            let error = validate_taira_post_provision_digital_kina_binding(&manifest)
                .expect_err("retired alias must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("retired Taira Digital Kina alias"),
                "unexpected error for {retired}: {error}"
            );
        }
    }
    #[test]
    fn taira_digital_kina_binding_rejects_substituted_raw_id() {
        let substituted = AssetDefinitionId::parse_address_literal(TAIRA_XOR_ASSET_DEFINITION_ID)
            .expect("pinned XOR id");
        let manifest =
            taira_digital_kina_manifest(TAIRA_DIGITAL_KINA_ALIAS, substituted, true, None);
        let error = validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect_err("canonical alias must not select a substituted raw id");
        assert!(
            error.to_string().contains("must target"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_digital_kina_binding_rejects_missing_owning_domain_registration() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest =
            taira_digital_kina_manifest(TAIRA_DIGITAL_KINA_ALIAS, expected_id, false, None);
        let error = validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect_err("an orphaned Digital Kina owning domain must fail closed");
        assert!(
            error.to_string().contains("owning domain"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_digital_kina_binding_rejects_late_owning_domain_registration() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let owning_domain = DomainId::parse_fully_qualified(TAIRA_DIGITAL_KINA_DOMAIN)
            .expect("pinned Digital Kina domain");
        let manifest = GenesisBuilder::new_without_executor(
            ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
            PathBuf::from("."),
        )
        .append_instruction(Register::asset_definition(
            AssetDefinition::new(
                expected_id.clone(),
                TAIRA_DIGITAL_KINA_NAME.to_owned(),
                NumericSpec::fractional(TAIRA_DIGITAL_KINA_SCALE),
                AssetBalancePolicy::DataspaceRestricted,
                Some(owning_domain.clone()),
            )
            .with_alias(Some(
                TAIRA_DIGITAL_KINA_ALIAS.parse().expect("canonical alias"),
            ))
            .with_metadata(Metadata::default()),
        ))
        .append_instruction(Register::domain(Domain::new(owning_domain)))
        .build_raw();
        let error = validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect_err("late owning-domain registration must fail closed");
        assert!(
            error.to_string().contains("before the atomic"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_digital_kina_binding_rejects_implicit_global_genesis_mint() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest = taira_digital_kina_manifest(
            TAIRA_DIGITAL_KINA_ALIAS,
            expected_id,
            true,
            Some(AssetBalanceScope::Global),
        );
        let error = validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect_err("an implicit universal Kina balance bucket must fail closed");
        assert!(
            error.to_string().contains("#dataspace:10"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn taira_digital_kina_binding_accepts_explicit_bpng_genesis_mint() {
        let expected_id =
            AssetDefinitionId::parse_address_literal(TAIRA_DIGITAL_KINA_ASSET_DEFINITION_ID)
                .expect("pinned Digital Kina id");
        let manifest = taira_digital_kina_manifest(
            TAIRA_DIGITAL_KINA_ALIAS,
            expected_id,
            true,
            Some(AssetBalanceScope::Dataspace(DataSpaceId::new(
                TAIRA_DIGITAL_KINA_DATASPACE_ID,
            ))),
        );
        validate_taira_post_provision_digital_kina_binding(&manifest)
            .expect("explicit BPNG dataspace mint should remain valid");
    }
    #[test]
    fn known_chain_discriminant_maps_taira_and_nexus() {
        assert_eq!(
            known_chain_discriminant_for_chain_id(PUBLIC_TAIRA_CHAIN_ID),
            Some(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(known_chain_discriminant_for_chain_id("iroha3-taira"), None);
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
