//! Mochi-specific extensions for Kagami-generated genesis manifests.
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetDefinition},
    domain::Domain,
    isi::{Grant, GrantBox, Mint, MintBox, Register, RegisterBox},
    metadata::Metadata,
    nexus::DataSpaceId,
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
    Ok(builder.build_raw())
}
/// Attach topology information to a genesis manifest inside a dedicated transaction.
pub fn with_topology(
    manifest: RawGenesisTransaction,
    topology: Vec<GenesisTopologyEntry>,
) -> RawGenesisTransaction {
    manifest
        .into_builder()
        .next_transaction()
        .set_topology(topology)
        .build_raw()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        isi::{GrantBox, MintBox, RegisterBox},
        prelude::ChainId,
    };
    use iroha_genesis::GenesisBuilder;
    use iroha_test_samples::ALICE_ID;
    #[test]
    fn local_onboarding_bootstrap_is_exact_funded_and_idempotent() {
        const EXPECTED_CANONICAL_XOR_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
        let chain_id: ChainId = "local-onboarding".parse().expect("infallible chain id");
        let manifest = GenesisBuilder::new_without_executor(chain_id, ".").build_raw();
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
