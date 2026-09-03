//! Mochi-specific extensions for Kagami-generated genesis manifests.
use iroha_crypto::Hash;
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetDefinition},
    block::consensus_v2::is_valid_committee_size,
    domain::Domain,
    isi::{
        Grant, GrantBox, Mint, MintBox, Register, RegisterBox,
        offline_cash_v1::{
            OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashMintFinalityEpochRosterTemplateV1,
            OfflineCashMintFinalityGenesisParametersV1,
        },
    },
    metadata::Metadata,
    nexus::DataSpaceId,
    peer::PeerId,
    permission::Permission,
    prelude::{AccountId, AssetDefinitionId, AssetId, DomainId, NumericSpec},
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount},
    nexus::CanPublishSpaceDirectoryManifestForAccountDomain,
};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
use std::{collections::BTreeSet, sync::LazyLock};
static SAMPLE_ROSE_DEFINITION_ID: LazyLock<AssetDefinitionId> = LazyLock::new(|| {
    let wonderland_id =
        DomainId::try_new("wonderland", "universal").expect("sample wonderland domain is valid");
    let rose = "rose".parse().expect("sample rose asset name is valid");
    AssetDefinitionId::derive_from_components(wonderland_id, rose)
});
static SAMPLE_CABBAGE_DEFINITION_ID: LazyLock<AssetDefinitionId> = LazyLock::new(|| {
    let garden_id = DomainId::try_new("garden_of_live_flowers", "universal")
        .expect("sample garden domain is valid");
    let cabbage = "cabbage"
        .parse()
        .expect("sample cabbage asset name is valid");
    AssetDefinitionId::derive_from_components(garden_id, cabbage)
});
const LOCAL_ONBOARDING_ACCOUNT_DOMAIN: &str = "wonderland.universal";
const LOCAL_ONBOARDING_FEE_ASSET_DOMAIN: &str = "universal.universal";
const LOCAL_ONBOARDING_FEE_ASSET_NAME: &str = "xor";
const LOCAL_ONBOARDING_FEE_ASSET_SCALE: u32 = 9;
const LOCAL_ONBOARDING_FEE_BALANCE: u64 = 10;
/// Canonical asset definition id for the bundled `rose` sample asset.
#[must_use]
pub fn sample_rose_definition_id() -> AssetDefinitionId {
    SAMPLE_ROSE_DEFINITION_ID.clone()
}
/// Canonical asset definition id for the bundled `cabbage` sample asset.
#[must_use]
pub fn sample_cabbage_definition_id() -> AssetDefinitionId {
    SAMPLE_CABBAGE_DEFINITION_ID.clone()
}
/// Add the exact ledger state required by Mochi's local account-onboarding service.
///
/// The local administrator is already the operator exposed by Mochi's bootstrap files. This
/// extension registers the default Nexus fee asset, funds that administrator, and grants only the
/// scoped capabilities used by sponsored account creation in the universal dataspace. Existing
/// equivalent genesis instructions are preserved without duplication.
pub fn with_local_account_onboarding_bootstrap(
    manifest: RawGenesisTransaction,
    authority: &AccountId,
) -> color_eyre::Result<RawGenesisTransaction> {
    let account_domain = DomainId::parse_fully_qualified(LOCAL_ONBOARDING_ACCOUNT_DOMAIN)?;
    let fee_asset_domain = DomainId::parse_fully_qualified(LOCAL_ONBOARDING_FEE_ASSET_DOMAIN)?;
    let fee_asset_name = LOCAL_ONBOARDING_FEE_ASSET_NAME.parse()?;
    let fee_asset_id =
        AssetDefinitionId::derive_from_components(fee_asset_domain.clone(), fee_asset_name);
    let authority_fee_asset = AssetId::new(fee_asset_id.clone(), authority.clone());
    let mut registered_domains = BTreeSet::new();
    let mut registered_accounts = BTreeSet::new();
    let mut registered_asset_definitions = BTreeSet::new();
    let mut granted_permissions = BTreeSet::new();
    let mut authority_is_funded = false;
    for instruction in manifest.instructions() {
        if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
            match register {
                RegisterBox::Domain(register) => {
                    registered_domains.insert(register.object.id.clone());
                }
                RegisterBox::Account(register) => {
                    registered_accounts.insert(register.object.id.clone());
                }
                RegisterBox::AssetDefinition(register) => {
                    registered_asset_definitions.insert(register.object.id.clone());
                }
                _ => {}
            }
            continue;
        }
        if let Some(GrantBox::Permission(grant)) = instruction.as_any().downcast_ref::<GrantBox>() {
            granted_permissions.insert((grant.destination().clone(), grant.object().clone()));
            continue;
        }
        if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>()
            && mint.destination() == &authority_fee_asset
        {
            authority_is_funded = true;
        }
    }
    let permissions = [
        Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        }),
        Permission::from(CanRegisterAccount {
            domain: account_domain.clone(),
        }),
        Permission::from(CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace: DataSpaceId::UNIVERSAL,
            domain: account_domain.clone(),
        }),
    ];
    let register_account_domain = !registered_domains.contains(&account_domain);
    let register_fee_asset_domain = !registered_domains.contains(&fee_asset_domain);
    let register_authority = !registered_accounts.contains(authority);
    let register_fee_asset = !registered_asset_definitions.contains(&fee_asset_id);
    let missing_permissions = permissions
        .into_iter()
        .filter(|permission| {
            !granted_permissions.contains(&(authority.clone(), permission.clone()))
        })
        .collect::<Vec<_>>();
    if !register_account_domain
        && !register_fee_asset_domain
        && !register_authority
        && !register_fee_asset
        && authority_is_funded
        && missing_permissions.is_empty()
    {
        return Ok(manifest);
    }
    let mut builder = manifest.into_builder().next_transaction();
    if register_account_domain {
        builder = builder.append_instruction(Register::domain(Domain::new(account_domain)));
    }
    if register_fee_asset_domain {
        builder = builder.append_instruction(Register::domain(Domain::new(fee_asset_domain)));
    }
    if register_authority {
        builder = builder.append_instruction(Register::account(Account::new(authority.clone())));
    }
    if register_fee_asset {
        let definition = AssetDefinition::new(
            fee_asset_id,
            "XOR".to_owned(),
            NumericSpec::fractional(LOCAL_ONBOARDING_FEE_ASSET_SCALE),
            AssetBalancePolicy::Global,
            None,
        )
        .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
    }
    if !authority_is_funded {
        builder = builder.append_instruction(Mint::asset_quantity(
            LOCAL_ONBOARDING_FEE_BALANCE,
            authority_fee_asset,
        ));
    }
    for permission in missing_permissions {
        builder =
            builder.append_instruction(Grant::account_permission(permission, authority.clone()));
    }
    builder.build_raw()
}

/// Derive development-only mint-finality authority for one exact Mochi committee.
pub(crate) fn localnet_mint_finality_genesis_parameters(
    topology: &[GenesisTopologyEntry],
) -> color_eyre::Result<OfflineCashMintFinalityGenesisParametersV1> {
    let mut validators = topology
        .iter()
        .map(|entry| entry.peer.clone())
        .collect::<Vec<PeerId>>();
    validators.sort();
    if !is_valid_committee_size(validators.len()) {
        return Err(color_eyre::eyre::eyre!(
            "Mochi Offline Cash mint-finality authority requires an exact Sumeragi v2 3f+1 committee"
        ));
    }
    if validators.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(color_eyre::eyre::eyre!(
            "Mochi Offline Cash mint-finality authority contains duplicate validators"
        ));
    }
    let validators = validators
        .into_iter()
        .enumerate()
        .map(|(index, validator)| {
            // Mochi localnets are disposable development deployments. Keep this
            // seed independent from every consensus secret while binding the
            // public authority to the exact canonical validator identity.
            let seed: [u8; 32] = Hash::new(format!(
                "iroha:mochi:localnet:offline-cash-mint-finality:v1:epoch-0:{index}:{validator}"
            ))
            .into();
            iroha_core::zk::offline_cash_v1_recursion::derive_offline_cash_mint_finality_validator_keys_v1(
                &seed,
                0,
                validator,
            )
            .map_err(|error| {
                color_eyre::eyre::eyre!(
                    "derive Mochi Offline Cash mint-finality validator keys: {error}"
                )
            })
        })
        .collect::<color_eyre::Result<Vec<_>>>()?;
    let parameters = OfflineCashMintFinalityGenesisParametersV1 {
        epoch_roster: OfflineCashMintFinalityEpochRosterTemplateV1 {
            version: OFFLINE_CASH_CHAIN_VERSION_V1,
            epoch: 0,
            validators,
        },
        next_epoch_roster: None,
    };
    parameters.validate().map_err(|error| {
        color_eyre::eyre::eyre!("invalid Mochi Offline Cash genesis authority: {error}")
    })?;
    Ok(parameters)
}

/// Attach topology information and its matching private-finality authority to a genesis manifest.
///
/// # Errors
///
/// Returns an error unless `topology` is a unique exact `3f + 1` committee or
/// its independent paired-Pasta public keys can be derived and validated.
pub fn with_topology(
    manifest: RawGenesisTransaction,
    topology: Vec<GenesisTopologyEntry>,
) -> color_eyre::Result<RawGenesisTransaction> {
    let mint_finality = localnet_mint_finality_genesis_parameters(&topology)?;
    manifest
        .into_builder()
        .with_offline_cash_mint_finality_genesis_parameters(mint_finality)
        .next_transaction()
        .set_topology(topology)
        .build_raw()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        block::consensus_v2::SumeragiV2GenesisContextParameters,
        isi::{GrantBox, MintBox, RegisterBox},
        peer::PeerId,
        prelude::ChainId,
    };
    use iroha_genesis::GenesisBuilder;
    use iroha_test_samples::ALICE_ID;

    fn deterministic_topology(seed_base: u8) -> Vec<GenesisTopologyEntry> {
        (0_u8..4)
            .map(|index| {
                let validator = KeyPair::try_from_seed(
                    vec![seed_base.wrapping_add(index); 32],
                    Algorithm::BlsNormal,
                )
                .expect("derive deterministic Mochi test validator");
                let pop = bls_normal_pop_prove(validator.private_key())
                    .expect("derive deterministic Mochi test validator PoP");
                GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
            })
            .collect()
    }

    #[test]
    fn local_onboarding_bootstrap_is_exact_funded_and_idempotent() {
        const EXPECTED_CANONICAL_XOR_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
        let chain_id: ChainId = "local-onboarding".parse().expect("infallible chain id");
        let topology = deterministic_topology(0xD0);
        let mint_finality = localnet_mint_finality_genesis_parameters(&topology)
            .expect("derive deterministic Mochi test Offline Cash authority");
        let manifest = GenesisBuilder::new_without_executor(chain_id, ".")
            .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
            .with_offline_cash_mint_finality_genesis_parameters(mint_finality)
            .set_topology(topology)
            .build_raw()
            .expect("build complete Mochi onboarding test genesis");
        let manifest = with_local_account_onboarding_bootstrap(manifest, &ALICE_ID)
            .expect("append local onboarding bootstrap");
        let transaction_count = manifest.transactions().len();
        let manifest = with_local_account_onboarding_bootstrap(manifest, &ALICE_ID)
            .expect("reapplying local onboarding bootstrap must remain valid");
        assert_eq!(manifest.transactions().len(), transaction_count);
        let account_domain =
            DomainId::parse_fully_qualified(LOCAL_ONBOARDING_ACCOUNT_DOMAIN).expect("domain");
        let fee_domain =
            DomainId::parse_fully_qualified(LOCAL_ONBOARDING_FEE_ASSET_DOMAIN).expect("fee domain");
        let fee_asset_definition = AssetDefinitionId::derive_from_components(
            fee_domain.clone(),
            LOCAL_ONBOARDING_FEE_ASSET_NAME
                .parse()
                .expect("fee asset name"),
        );
        let fee_asset = AssetId::new(fee_asset_definition.clone(), ALICE_ID.clone());
        assert_eq!(
            fee_asset_definition.canonical_address(),
            EXPECTED_CANONICAL_XOR_ID,
            "xor in universal.universal must resolve to the configured Nexus fee asset",
        );
        let domain_registrations = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterBox>())
            .filter(|register| {
                matches!(register, RegisterBox::Domain(register) if register.object.id == fee_domain)
            })
            .count();
        let fee_asset_registrations = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterBox>())
            .filter(|register| {
                matches!(register, RegisterBox::AssetDefinition(register) if register.object.id == fee_asset_definition)
            })
            .count();
        let fee_mints = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<MintBox>())
            .filter(|mint| matches!(mint, MintBox::Asset(mint) if mint.destination() == &fee_asset))
            .count();
        assert_eq!(domain_registrations, 1);
        assert_eq!(fee_asset_registrations, 1);
        assert_eq!(fee_mints, 1);
        let fee_mint = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<MintBox>())
            .find_map(|mint| match mint {
                MintBox::Asset(mint) if mint.destination() == &fee_asset => Some(mint),
                _ => None,
            })
            .expect("funded onboarding authority");
        assert_eq!(
            fee_mint.object().to_string(),
            LOCAL_ONBOARDING_FEE_BALANCE.to_string()
        );
        let expected_permissions = BTreeSet::from([
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
            Permission::from(CanRegisterAccount {
                domain: account_domain.clone(),
            }),
            Permission::from(CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace: DataSpaceId::UNIVERSAL,
                domain: account_domain,
            }),
        ]);
        let actual_permissions = manifest
            .instructions()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
            .filter_map(|grant| match grant {
                GrantBox::Permission(grant) if grant.destination() == &*ALICE_ID => {
                    expected_permissions
                        .contains(grant.object())
                        .then(|| grant.object().clone())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_permissions, expected_permissions);
        for permission in &expected_permissions {
            let count = manifest
                .instructions()
                .filter_map(|instruction| instruction.as_any().downcast_ref::<GrantBox>())
                .filter(|grant| {
                    matches!(
                        grant,
                        GrantBox::Permission(grant)
                            if grant.destination() == &*ALICE_ID && grant.object() == permission
                    )
                })
                .count();
            assert_eq!(count, 1, "permission must be granted exactly once");
        }
    }

    #[test]
    fn topology_replacement_rebinds_the_private_finality_authority() {
        let original_topology = deterministic_topology(0xB0);
        let original_authority = localnet_mint_finality_genesis_parameters(&original_topology)
            .expect("derive original test authority");
        let manifest = GenesisBuilder::new_without_executor(ChainId::from("topology-rebind"), ".")
            .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
            .with_offline_cash_mint_finality_genesis_parameters(original_authority)
            .build_raw()
            .expect("build original complete test manifest");
        let replacement_topology = deterministic_topology(0xC0);
        let expected_authority = localnet_mint_finality_genesis_parameters(&replacement_topology)
            .expect("derive replacement test authority");
        let patched = with_topology(manifest, replacement_topology.clone())
            .expect("replace topology and private-finality authority");
        assert_eq!(
            patched.offline_cash_mint_finality_genesis_parameters(),
            &expected_authority
        );
        assert_eq!(
            patched
                .transactions()
                .last()
                .expect("topology transaction")
                .topology(),
            replacement_topology
        );
    }

    #[test]
    fn bundled_sample_asset_ids_are_canonical_and_distinct() {
        let rose = sample_rose_definition_id();
        let cabbage = sample_cabbage_definition_id();
        assert_eq!(rose.to_string().parse::<AssetDefinitionId>().unwrap(), rose);
        assert_eq!(
            cabbage.to_string().parse::<AssetDefinitionId>().unwrap(),
            cabbage
        );
        assert_ne!(rose, cabbage);
    }
}
