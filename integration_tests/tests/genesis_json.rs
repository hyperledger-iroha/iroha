#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Load `genesis.json` and ensure assets minted in genesis appear on all peers.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
use iroha_config::base::toml::WriteExt as _;
use iroha_crypto::Algorithm;
use iroha_genesis::{
    GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction, init_instruction_registry,
};
use iroha_primitives::{json::Json, numeric::NumericSpec};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::{borrow::Cow, io::Write, path::PathBuf};
use tempfile::NamedTempFile;
use tokio::time::timeout;
use toml::Table;

fn deterministic_test_genesis_topology() -> Vec<GenesisTopologyEntry> {
    (0_u8..4)
        .map(|index| {
            let key_pair = iroha_crypto::KeyPair::try_from_seed(
                vec![0x40_u8.wrapping_add(index); 32],
                Algorithm::BlsNormal,
            )
            .expect("derive deterministic integration-test genesis validator");
            let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key())
                .expect("derive integration-test validator proof of possession");
            GenesisTopologyEntry::new(PeerId::new(key_pair.public_key().clone()), pop)
        })
        .collect()
}

fn complete_test_genesis_builder_for_topology(
    builder: GenesisBuilder,
    topology: Vec<GenesisTopologyEntry>,
) -> GenesisBuilder {
    assert!(
        !topology.is_empty(),
        "integration-test genesis topology must contain validators"
    );
    let mut validators = topology
        .iter()
        .map(|entry| entry.peer.clone())
        .collect::<Vec<_>>();
    validators.sort();
    let validators = validators
        .into_iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte = 0xA0_u8.wrapping_add(
                u8::try_from(index).expect("integration-test validator index fits in one byte"),
            );
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                0,
                validator,
            )
            .expect("derive deterministic paired-Pasta integration-test validator keys")
        })
        .collect();
    let parameters =
        iroha::data_model::isi::kagemusha_v1::KagemushaMintFinalityGenesisParametersV1 {
            epoch_roster:
                iroha::data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochRosterTemplateV1 {
                    version: iroha::data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
                    epoch: 0,
                    validators,
                },
            next_epoch_roster: None,
        };
    parameters
        .validate()
        .expect("integration-test topology must form a canonical mint-finality roster");
    builder
        .set_topology(topology)
        .with_sumeragi_v2_context_parameters(
            iroha::data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(
            ),
        )
        .with_kagemusha_mint_finality_genesis_parameters(parameters)
}

fn complete_test_genesis_builder(builder: GenesisBuilder) -> GenesisBuilder {
    complete_test_genesis_builder_for_topology(builder, deterministic_test_genesis_topology())
}

fn has_legacy_domain_scoped_permission_grants(raw: &RawGenesisTransaction) -> bool {
    raw.instructions().any(|instruction| {
        let Some(grant_box) = instruction.as_any().downcast_ref::<GrantBox>() else {
            return false;
        };
        let GrantBox::Permission(grant) = grant_box else {
            return false;
        };
        matches!(grant.object().name(), "CanRegisterAccount")
            && grant.object().payload() == &Json::default()
    })
}
fn load_raw_genesis_transaction() -> RawGenesisTransaction {
    eprintln!(
        "Using an explicit topology-bound integration-test genesis; checked-in `.template.json` sources are intentionally non-signable"
    );
    fallback_raw_genesis_from_json()
}
fn fallback_raw_genesis_from_json() -> RawGenesisTransaction {
    let chain = iroha_test_network::chain_id();
    let mut builder = GenesisBuilder::new_without_executor(chain, PathBuf::from("."));
    builder = builder
        .domain(DomainId::try_new("wonderland", "universal").expect("domain"))
        .account(ALICE_KEYPAIR.public_key().clone())
        .account(BOB_KEYPAIR.public_key().clone())
        .asset("rose".parse().expect("asset"), NumericSpec::default())
        .finish_domain();
    let genesis_account = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
    let wonderland_domain: DomainId =
        DomainId::try_new("wonderland", "universal").expect("wonderland domain id");
    let rose_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("rose asset"),
        "rose".parse().expect("rose asset"),
    );
    builder = builder.append_instruction(Transfer::domain(
        genesis_account,
        wonderland_domain,
        ALICE_ID.clone(),
    ));
    builder = builder.append_instruction(Mint::asset_quantity(
        13_u32,
        AssetId::new(rose_definition_id, ALICE_ID.clone()),
    ));
    complete_test_genesis_builder(builder)
        .build_raw()
        .expect("build complete lightweight integration-test genesis fixture")
}
#[test]
fn genesis_asset_minted_across_peers() -> Result<()> {
    init_instruction_registry();
    let raw_genesis = load_raw_genesis_transaction();
    let builder = NetworkBuilder::new().with_min_peers(4).with_genesis_block(
        move |_topology, topology_entries| {
            complete_test_genesis_builder_for_topology(
                raw_genesis.clone().into_builder().next_transaction(),
                topology_entries,
            )
            .build_raw()
            .expect("rebuild integration-test genesis for the exact network topology")
            .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
            .expect("build canonical resultless custom genesis proposal")
        },
    );
    let Some((network, rt)) = sandbox::build_network_blocking_or_skip(
        builder,
        stringify!(genesis_asset_minted_across_peers),
    ) else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout();
    let block_result: Result<()> = rt.block_on(async {
        if let Err(err) = network.start_all().await {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(eyre!(
                    "sandboxed network restriction detected while starting peers: {reason}"
                ));
            }
            return Err(err);
        }
        for peer in network.peers() {
            timeout(sync_timeout, peer.once_block(1))
                .await
                .map_err(|_| eyre!("timed out waiting for genesis block 1"))?;
        }
        let asset_id = AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            ALICE_ID.clone(),
        );
        for peer in network.peers() {
            let assets = peer
                .client()
                .query(FindAssets::new())
                .execute_all()
                .unwrap();
            let asset = assets
                .into_iter()
                .find(|a| a.id() == &asset_id)
                .expect("asset not found");
            assert_eq!(asset.value(), &Quantity::from(13_u32));
        }
        Ok(())
    });
    if let Err(err) = block_result {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            eprintln!(
                "sandboxed network restriction detected while running genesis_asset_minted_across_peers; skipping ({reason})"
            );
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}
#[test]
fn malformed_genesis_file_fails() {
    init_instruction_registry();
    let mut file = tempfile::NamedTempFile::new().expect("temp file");
    file.as_file_mut()
        .write_all(b"not-json")
        .expect("write temp file");
    assert!(RawGenesisTransaction::from_path(file.path()).is_err());
}
#[test]
fn missing_genesis_file_fails() {
    init_instruction_registry();
    let path = PathBuf::from("this_file_should_not_exist.json");
    assert!(RawGenesisTransaction::from_path(path).is_err());
}
#[test]
fn legacy_domain_scoped_permission_grants_are_detected() {
    init_instruction_registry();
    let chain = iroha_test_network::chain_id();
    let legacy = complete_test_genesis_builder(
        GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from(".")).append_instruction(
            Grant::account_permission(
                Permission::new(
                    "CanRegisterAccount".parse().expect("permission name"),
                    Json::default(),
                ),
                ALICE_ID.clone(),
            ),
        ),
    )
    .build_raw()
    .expect("build complete legacy-permission genesis fixture");
    assert!(has_legacy_domain_scoped_permission_grants(&legacy));
    let typed = complete_test_genesis_builder(
        GenesisBuilder::new_without_executor(chain, PathBuf::from(".")).append_instruction(
            Grant::account_permission(
                iroha_executor_data_model::permission::account::CanRegisterAccount {
                    domain: DomainId::try_new("wonderland", "universal").expect("domain id"),
                },
                ALICE_ID.clone(),
            ),
        ),
    )
    .build_raw()
    .expect("build complete typed-permission genesis fixture");
    assert!(!has_legacy_domain_scoped_permission_grants(&typed));
}
#[test]
fn genesis_norito_bytes_roundtrip_network() -> Result<()> {
    init_instruction_registry();
    let builder = NetworkBuilder::new().with_min_peers(4);
    let Some((network, rt)) = sandbox::build_network_blocking_or_skip(
        builder,
        stringify!(genesis_norito_bytes_roundtrip_network),
    ) else {
        return Ok(());
    };
    let genesis = network.genesis();
    let framed = genesis.0.encode_wire().map_err(|err| eyre!(err))?;
    let deframed = iroha::data_model::block::deframe_versioned_signed_block_bytes(&framed)
        .map_err(|err| eyre!(err))?;
    assert_eq!(deframed.bytes.as_ref(), framed.as_slice());
    assert_eq!(deframed.bare_versioned.as_ref().first().copied(), Some(1));
    let sync_timeout = network.sync_timeout();
    let roundtrip_result: Result<()> = rt.block_on(async {
        if let Err(err) = network.start_all().await {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(eyre!(
                    "sandboxed network restriction detected while starting peers: {reason}"
                ));
            }
            return Err(err);
        }
        let peer = network.peer();
        timeout(sync_timeout, peer.once_block(1))
            .await
            .map_err(|_| eyre!("timed out waiting for genesis block 1"))?;
        let _blocks: u64 = peer.client().get_status().unwrap().blocks;
        Ok(())
    });
    if let Err(err) = roundtrip_result {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            eprintln!(
                "sandboxed network restriction detected while running genesis_norito_bytes_roundtrip_network; skipping ({reason})"
            );
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}
#[tokio::test]
async fn tampered_genesis_block_is_rejected() -> Result<()> {
    init_instruction_registry();
    let Some(network) = sandbox::build_network_or_skip(
        NetworkBuilder::new().with_min_peers(4),
        stringify!(tampered_genesis_block_is_rejected),
    ) else {
        return Ok(());
    };
    let genesis = network.genesis();
    let mut framed = genesis.0.encode_wire().map_err(|err| eyre!(err))?;
    let last = framed
        .last_mut()
        .ok_or_else(|| eyre!("expected non-empty genesis frame"))?;
    *last ^= 0xFF;
    let mut tampered_file = NamedTempFile::new()?;
    tampered_file.write_all(&framed)?;
    tampered_file.flush()?;
    let tampered_path = tampered_file.path().to_path_buf();
    let override_layer = Table::new().write(
        ["genesis", "file"],
        tampered_path.to_string_lossy().to_string(),
    );
    for peer in network.peers() {
        let start_result = peer
            .start_checked(
                network
                    .config_layers()
                    .chain(std::iter::once(Cow::Owned(override_layer.clone()))),
                None,
            )
            .await;
        assert!(
            start_result.is_err(),
            "tampered genesis must not start a peer"
        );
    }
    Ok(())
}
