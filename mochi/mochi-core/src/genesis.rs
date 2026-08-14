//! Helpers for generating default genesis manifests aligned with Kagami defaults.
use std::{collections::BTreeSet, path::PathBuf, sync::LazyLock};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetDefinition},
    domain::Domain,
    isi::{Grant, GrantBox, Mint, MintBox, Register, RegisterBox, SetParameter, Transfer},
    metadata::Metadata,
    nexus::DataSpaceId,
    parameter::{
        Parameter, Parameters,
        custom::{CustomParameter, CustomParameterId},
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    },
    permission::Permission,
    prelude::{AccountId, AssetDefinitionId, AssetId, ChainId, DomainId, NumericSpec},
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount},
    nexus::CanPublishSpaceDirectoryManifestForAccountDomain,
    parameter::CanSetParameters,
};
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};
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
/// Build a default genesis manifest with Kagami-equivalent instructions and metadata.
///
/// The resulting manifest mirrors `kagami genesis generate default` and includes
/// consensus metadata. Callers can optionally override the consensus mode and
/// gas limit parameter; sensible defaults mirror Kagami's CLI. This helper is
/// also exposed via the `mochi-genesis` command-line tool for local workflows.
pub fn default_manifest(
    chain_id: ChainId,
    genesis_public_key: &PublicKey,
    ivm_dir: impl Into<PathBuf>,
    consensus_mode: SumeragiConsensusMode,
    ivm_gas_limit_per_block: Option<u64>,
) -> color_eyre::Result<RawGenesisTransaction> {
    let ivm_dir = ivm_dir.into();
    let builder = GenesisBuilder::new_without_executor(chain_id, ivm_dir);
    let genesis_account_id = AccountId::new(genesis_public_key.clone());
    let mut meta = Metadata::default();
    meta.insert("key".parse()?, Json::new("value"));
    let wonderland_id = DomainId::try_new("wonderland", "universal")?;
    let garden_id = DomainId::try_new("garden_of_live_flowers", "universal")?;
    let rose_definition = sample_rose_definition_id();
    let cabbage_definition = sample_cabbage_definition_id();
    let mut builder = builder
        .domain_with_metadata(wonderland_id.clone(), meta.clone())
        .account_with_metadata(ALICE_ID.expect_single_signatory().clone(), meta.clone())
        .account_with_metadata(BOB_ID.expect_single_signatory().clone(), meta)
        .asset("rose".parse()?, NumericSpec::default())
        .finish_domain()
        .domain(garden_id.clone())
        .account(CARPENTER_ID.expect_single_signatory().clone())
        .asset("cabbage".parse()?, NumericSpec::default())
        .finish_domain();
    let mint_rose = Mint::asset_quantity(
        13u32,
        AssetId::new(rose_definition.clone(), ALICE_ID.clone()),
    );
    let mint_cabbage =
        Mint::asset_quantity(44u32, AssetId::new(cabbage_definition, ALICE_ID.clone()));
    let grant_set_parameters = Grant::account_permission(CanSetParameters, ALICE_ID.clone());
    let transfer_rose_definition = Transfer::asset_definition(
        genesis_account_id.clone(),
        rose_definition,
        ALICE_ID.clone(),
    );
    let transfer_wonderland = Transfer::domain(
        genesis_account_id.clone(),
        wonderland_id.clone(),
        ALICE_ID.clone(),
    );
    let npos_defaults = SumeragiNposParameters::default();
    let parameters = Parameters::default();
    for parameter in parameters.parameters() {
        builder = builder.append_parameter(parameter);
    }
    builder = builder
        .next_transaction()
        .append_instruction(mint_rose)
        .append_instruction(mint_cabbage)
        .append_instruction(transfer_rose_definition)
        .append_instruction(transfer_wonderland)
        .append_instruction(grant_set_parameters);
    let gas_param_id = CustomParameterId::new("ivm_gas_limit_per_block".parse()?);
    let gas_param_val = ivm_gas_limit_per_block.unwrap_or(1_680_000u64);
    let gas_param = CustomParameter::new(gas_param_id, Json::new(gas_param_val));
    let set_npos = SetParameter::new(Parameter::Custom(npos_defaults.into()));
    let set_gas_param = SetParameter::new(Parameter::Custom(gas_param));
    let builder = builder.append_instruction(set_gas_param);
    let builder = if matches!(consensus_mode, SumeragiConsensusMode::Npos) {
        builder.append_instruction(set_npos)
    } else {
        builder
    };
    let manifest = builder.build_raw().with_consensus_mode(consensus_mode);
    Ok(manifest.with_consensus_meta())
}
/// Add the exact ledger state required by Mochi's local account-onboarding service.
///
/// The local administrator is already the operator exposed by Mochi's bootstrap
/// files. This extension registers the default Nexus fee asset, funds that
/// administrator, and grants only the scoped capabilities used by sponsored
/// account creation in the universal dataspace. Existing equivalent genesis
/// instructions are preserved without duplication.
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
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        isi::{GrantBox, MintBox, RegisterBox, SetParameter},
        parameter::{
            Parameter,
            custom::{CustomParameter, CustomParameterId},
            system::{
                SumeragiConsensusMode, SumeragiNposParameters, confidential_metadata,
                consensus_metadata, crypto_metadata,
            },
        },
        prelude::ChainId,
        transaction::Executable,
    };
    use iroha_primitives::json::Json;
    use super::*;
    #[test]
    fn local_onboarding_bootstrap_is_exact_funded_and_idempotent() {
        const EXPECTED_CANONICAL_XOR_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
        let chain_id: ChainId = "local-onboarding".parse().expect("infallible chain id");
        let keypair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tmp dir for ivm");
        let manifest = default_manifest(
            chain_id,
            keypair.public_key(),
            ivm_dir.path(),
            SumeragiConsensusMode::Permissioned,
            None,
        )
        .expect("build default manifest");
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
    fn bundled_sample_asset_ids_match_default_manifest_assets() {
        let chain_id: ChainId = "local-testnet".parse().expect("infallible chain id");
        let keypair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tmp dir for ivm");
        let manifest = default_manifest(
            chain_id,
            keypair.public_key(),
            ivm_dir.path(),
            SumeragiConsensusMode::Permissioned,
            None,
        )
        .expect("build default manifest");
        let json = norito::json::to_value(&manifest).expect("manifest json");
        let text = norito::json::to_string(&json).expect("manifest text");
        assert!(
            text.contains(&sample_rose_definition_id().to_string()),
            "default manifest should include the bundled rose asset id"
        );
        assert!(
            text.contains(&sample_cabbage_definition_id().to_string()),
            "default manifest should include the bundled cabbage asset id"
        );
    }
    #[test]
    fn default_manifest_matches_kagami_parameter_baseline() {
        let chain_id: ChainId = "local-testnet".parse().expect("infallible chain id");
        let keypair = KeyPair::random();
        let ivm_dir = tempfile::tempdir().expect("tmp dir for ivm");
        let gas_limit = 2_000_000u64;
        let manifest = default_manifest(
            chain_id.clone(),
            keypair.public_key(),
            ivm_dir.path(),
            SumeragiConsensusMode::Npos,
            Some(gas_limit),
        )
        .expect("build default manifest");
        assert_eq!(
            manifest.consensus_mode(),
            SumeragiConsensusMode::Npos,
            "default genesis manifest should advertise the requested consensus mode"
        );
        assert_eq!(
            manifest.wire_protocol_version(),
            u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION),
            "consensus metadata should populate the first-release protocol version"
        );
        let block = manifest
            .build_and_sign(&keypair)
            .expect("sign genesis from default manifest")
            .0;
        let transactions: Vec<_> = block.external_transactions().collect();
        assert!(
            transactions.len() >= 2,
            "default manifest should emit multiple transactions (saw {})",
            transactions.len()
        );
        let gas_param_id = CustomParameterId::new(
            "ivm_gas_limit_per_block"
                .parse()
                .expect("valid parameter id"),
        );
        let expected_npos =
            SetParameter::new(Parameter::Custom(SumeragiNposParameters::default().into()));
        let expected_gas = SetParameter::new(Parameter::Custom(CustomParameter::new(
            gas_param_id.clone(),
            Json::new(gas_limit),
        )));
        let expected_npos_id = match expected_npos.inner() {
            Parameter::Custom(custom) => custom.id().clone(),
            other => panic!("expected_npos should be a custom parameter, got {other:?}"),
        };
        let expected_gas_id = match expected_gas.inner() {
            Parameter::Custom(custom) => custom.id().clone(),
            other => panic!("expected_gas should be a custom parameter, got {other:?}"),
        };
        let mut saw_npos_defaults = false;
        let mut saw_gas_limit = false;
        let mut saw_handshake_meta = false;
        let mut saw_crypto_manifest = false;
        let mut saw_confidential_registry = false;
        for transaction in transactions {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                continue;
            };
            for instruction in instructions {
                let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>()
                else {
                    continue;
                };
                match set_parameter.inner() {
                    Parameter::Custom(custom) if custom.id() == &expected_npos_id => {
                        saw_npos_defaults = true;
                    }
                    Parameter::Custom(custom) if custom.id() == &expected_gas_id => {
                        saw_gas_limit = true;
                    }
                    Parameter::Custom(custom)
                        if custom.id() == &consensus_metadata::handshake_meta_id() =>
                    {
                        saw_handshake_meta = true;
                    }
                    Parameter::Custom(custom)
                        if custom.id() == &crypto_metadata::manifest_meta_id() =>
                    {
                        saw_crypto_manifest = true;
                    }
                    Parameter::Custom(custom)
                        if custom.id() == &confidential_metadata::registry_root_id() =>
                    {
                        saw_confidential_registry = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_npos_defaults,
            "genesis must include the baseline NPoS parameter payload"
        );
        assert!(
            saw_gas_limit,
            "genesis must configure the IVM gas limit custom parameter"
        );
        assert!(
            saw_handshake_meta,
            "genesis must embed consensus handshake metadata"
        );
        assert!(
            saw_crypto_manifest,
            "genesis must advertise the crypto manifest metadata parameter"
        );
        assert!(
            saw_confidential_registry,
            "genesis must emit the confidential registry root metadata"
        );
    }
}
