//! Helpers for generating default genesis manifests aligned with Kagami defaults.

use std::{path::PathBuf, sync::LazyLock};

use iroha_crypto::PublicKey;
use iroha_data_model::{
    isi::{Grant, Mint, SetParameter, Transfer},
    metadata::Metadata,
    parameter::{
        Parameter, Parameters,
        custom::{CustomParameter, CustomParameterId},
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    },
    prelude::{AccountId, AssetDefinitionId, AssetId, ChainId, DomainId, NumericSpec},
};
use iroha_executor_data_model::permission::parameter::CanSetParameters;
use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};

static SAMPLE_ROSE_DEFINITION_ID: LazyLock<AssetDefinitionId> = LazyLock::new(|| {
    let wonderland_id =
        DomainId::try_new("wonderland", "universal").expect("sample wonderland domain is valid");
    let rose = "rose".parse().expect("sample rose asset name is valid");
    AssetDefinitionId::new(wonderland_id, rose)
});

static SAMPLE_CABBAGE_DEFINITION_ID: LazyLock<AssetDefinitionId> = LazyLock::new(|| {
    let garden_id = DomainId::try_new("garden_of_live_flowers", "universal")
        .expect("sample garden domain is valid");
    let cabbage = "cabbage"
        .parse()
        .expect("sample cabbage asset name is valid");
    AssetDefinitionId::new(garden_id, cabbage)
});

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
        isi::SetParameter,
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
