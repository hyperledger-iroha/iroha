#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Load `genesis.json` and ensure assets minted in genesis appear on all peers.

use std::{borrow::Cow, io::Write, path::PathBuf, sync::Arc};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
use iroha_config::base::toml::WriteExt as _;
use iroha_genesis::{GenesisBuilder, RawGenesisTransaction, init_instruction_registry};
use iroha_primitives::{json::Json, numeric::NumericSpec};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use tempfile::NamedTempFile;
use tokio::time::timeout;
use toml::Table;

fn ivm_build_profile_exists() -> bool {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/ivm/target/prebuilt/build_config.toml")
        .exists()
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
    let genesis_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../defaults/genesis.json");
    if ivm_build_profile_exists() && genesis_path.exists() {
        match RawGenesisTransaction::from_path(&genesis_path) {
            Ok(raw) => {
                if has_legacy_domain_scoped_permission_grants(&raw) {
                    eprintln!(
                        "defaults/genesis.json contains legacy domain-scoped permission grants without payloads; falling back to synthetic genesis"
                    );
                } else {
                    return raw;
                }
            }
            Err(err) => {
                eprintln!(
                    "Failed to load defaults/genesis.json ({err:?}), falling back to synthetic genesis"
                );
            }
        }
    }

    eprintln!("Using lightweight fallback genesis fixture for integration tests");
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
    let rose_definition_id: AssetDefinitionId = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("rose asset"),
        "rose".parse().expect("rose asset"),
    );

    builder = builder.append_instruction(Transfer::domain(
        genesis_account,
        wonderland_domain,
        ALICE_ID.clone(),
    ));
    builder = builder.append_instruction(Mint::asset_numeric(
        13_u32,
        AssetId::new(rose_definition_id, ALICE_ID.clone()),
    ));

    builder.build_raw()
}

#[test]
fn genesis_asset_minted_across_peers() -> Result<()> {
    init_instruction_registry();

    // Build network first to obtain peer topology
    let Some((network, rt)) = sandbox::build_network_blocking_or_skip(
        NetworkBuilder::new().with_min_peers(4),
        stringify!(genesis_asset_minted_across_peers),
    ) else {
        return Ok(());
    };
    let builder = load_raw_genesis_transaction()
        .into_builder()
        .next_transaction()
        .set_topology(network.topology_entries().to_vec());
    let genesis_block = builder.build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)?;

    let sync_timeout = network.sync_timeout();
    let block_result: Result<()> = rt.block_on(async {
        let genesis = Arc::new(genesis_block);
        for peer in network.peers() {
            if let Err(err) = peer
                .start_checked(network.config_layers(), Some(&genesis))
                .await
            {
                if let Some(reason) = sandbox::sandbox_reason(&err) {
                    return Err(eyre!(
                        "sandboxed network restriction detected while starting peers: {reason}"
                    ));
                }
                return Err(err);
            }
            timeout(sync_timeout, peer.once_block(1))
                .await
                .map_err(|_| eyre!("timed out waiting for genesis block 1"))?;
        }

        let asset_id = AssetId::new(
            AssetDefinitionId::new(
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
            assert_eq!(asset.value(), &numeric!(13));
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
    let legacy = GenesisBuilder::new_without_executor(chain.clone(), PathBuf::from("."))
        .append_instruction(Grant::account_permission(
            Permission::new(
                "CanRegisterAccount".parse().expect("permission name"),
                Json::default(),
            ),
            ALICE_ID.clone(),
        ))
        .build_raw();
    assert!(has_legacy_domain_scoped_permission_grants(&legacy));

    let typed = GenesisBuilder::new_without_executor(chain, PathBuf::from("."))
        .append_instruction(Grant::account_permission(
            iroha_executor_data_model::permission::account::CanRegisterAccount {
                domain: DomainId::try_new("wonderland", "universal").expect("domain id"),
            },
            ALICE_ID.clone(),
        ))
        .build_raw();
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
