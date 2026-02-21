#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet confidential flow coverage:
//! public-origin shield -> 3 shielded hops -> unshield -> public transfer,
//! plus a second shielded asset with 3 shielded hops.

use eyre::{Report, Result, WrapErr as _, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        confidential::ConfidentialEncryptedPayload,
        prelude::{
            AssetDefinition, AssetDefinitionId, AssetId, FindAssetById, InstructionBox, Level, Log,
            Mint, Numeric, Register, Transfer,
        },
        transaction::SignedTransaction,
    },
};
use iroha_data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyBox};
use iroha_test_network::{NetworkBuilder, NetworkPeer};
use iroha_test_samples::BOB_ID;

fn marker(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn debug_ok_attachment() -> ProofAttachment {
    let backend = "debug/ok";
    let proof = ProofBox::new(backend.to_owned(), vec![0x01]);
    let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0x02]);
    ProofAttachment::new_inline(backend.to_owned(), proof, vk)
}

fn numeric_balance(client: &Client, id: AssetId) -> Result<Numeric> {
    let asset = client
        .query_single(FindAssetById::new(id))
        .wrap_err("query asset balance")?;
    Ok(asset.value().clone())
}

fn numeric_balance_any(clients: &[Client], id: AssetId) -> Result<Numeric> {
    let mut last_err = None;
    for client in clients {
        match numeric_balance(client, id.clone()) {
            Ok(value) => return Ok(value),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| eyre!("no client available for balance query")))
}

fn is_transient_client_error(err: &Report) -> bool {
    const NEEDLES: [&str; 6] = [
        "Failed to send http",
        "error sending request for url",
        "operation timed out",
        "Connection refused",
        "connection closed",
        "connection reset",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn is_duplicate_tx_error(err: &Report) -> bool {
    const NEEDLES: [&str; 6] = [
        "PRTRY:ALREADY_COMMITTED",
        "PRTRY:ALREADY_ENQUEUED",
        "already_committed",
        "already_enqueued",
        "transaction already committed",
        "transaction already present in the queue",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

fn submit_transaction_on_any_peer(
    submitters: &[Client],
    tx: &SignedTransaction,
    context: &str,
) -> Result<()> {
    let mut accepted = false;
    let mut transient_last_err = None;

    for submitter in submitters {
        match submitter.submit_transaction(tx) {
            Ok(_) => accepted = true,
            Err(err) if is_duplicate_tx_error(&err) => accepted = true,
            Err(err) if is_transient_client_error(&err) => transient_last_err = Some(err),
            Err(err) => return Err(err).wrap_err(context.to_owned()),
        }
    }

    if accepted {
        Ok(())
    } else {
        Err(transient_last_err.unwrap_or_else(|| eyre!("all peers unreachable")))
            .wrap_err(context.to_owned())
    }
}

async fn submit_and_wait_non_empty_block(
    network: &sandbox::SerializedNetwork,
    tx_builder_client: &Client,
    submitters: &[Client],
    instructions: Vec<InstructionBox>,
    non_empty_target: &mut u64,
    context: &str,
) -> Result<()> {
    let tx = tx_builder_client.build_transaction_from_items(
        instructions,
        iroha_data_model::metadata::Metadata::default(),
    );

    submit_transaction_on_any_peer(submitters, &tx, context)?;

    *non_empty_target = non_empty_target.saturating_add(1);
    let target = *non_empty_target;
    network
        .ensure_blocks_with(|height| height.non_empty >= target)
        .await
        .wrap_err_with(|| format!("{context}: wait non-empty block {target}"))?;

    Ok(())
}

#[tokio::test]
async fn confidential_public_and_shielded_three_hop_localnet() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_public_and_shielded_three_hop_localnet),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let public_asset_def: AssetDefinitionId = "zkpublichop#wonderland".parse().unwrap();
    let shielded_asset_def: AssetDefinitionId = "zkshieldhop#wonderland".parse().unwrap();

    let mut non_empty_target = 1_u64;

    let setup_instructions: Vec<InstructionBox> = vec![
        Register::asset_definition(AssetDefinition::numeric(public_asset_def.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(shielded_asset_def.clone())).into(),
        Mint::asset_numeric(
            1_000_u64,
            AssetId::new(public_asset_def.clone(), source.clone()),
        )
        .into(),
        Mint::asset_numeric(
            700_u64,
            AssetId::new(shielded_asset_def.clone(), source.clone()),
        )
        .into(),
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            public_asset_def.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
        iroha_data_model::isi::zk::RegisterZkAsset::new(
            shielded_asset_def.clone(),
            iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
            true,
            true,
            None,
            None,
            None,
        )
        .into(),
    ];

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        setup_instructions,
        &mut non_empty_target,
        "prepare confidential assets",
    )
    .await?;

    // Flow 1: public-origin -> shield -> 3 shielded hops -> unshield -> public transfer.
    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                public_asset_def.clone(),
                source.clone(),
                600_u128,
                marker(1),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "public-origin shield failed",
    )
    .await?;

    for output_commitment in [21_u8, 22_u8, 23_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    public_asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    debug_ok_attachment(),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "public-origin 3-hop shielded transfer failed",
        )
        .await?;
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Unshield::new(
                public_asset_def.clone(),
                source.clone(),
                250_u128,
                vec![marker(31)],
                debug_ok_attachment(),
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "public-origin unshield failed",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Transfer::asset_numeric(
                AssetId::new(public_asset_def.clone(), source.clone()),
                250_u64,
                recipient.clone(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "public transfer after unshield failed",
    )
    .await?;

    // Flow 2: second shielded asset -> shield -> 3 shielded hops.
    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                shielded_asset_def.clone(),
                source.clone(),
                500_u128,
                marker(2),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shielded-asset shield failed",
    )
    .await?;

    for output_commitment in [51_u8, 52_u8, 53_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    shielded_asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    debug_ok_attachment(),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "shielded-asset 3-hop transfer failed",
        )
        .await?;
    }

    let source_public = numeric_balance_any(
        &peer_clients,
        AssetId::new(public_asset_def.clone(), source.clone()),
    )?;
    let recipient_public = numeric_balance_any(
        &peer_clients,
        AssetId::new(public_asset_def, recipient.clone()),
    )?;
    let source_shielded_public = numeric_balance_any(
        &peer_clients,
        AssetId::new(shielded_asset_def, source.clone()),
    )?;

    assert_eq!(source_public, Numeric::from(400_u32));
    assert_eq!(recipient_public, Numeric::from(250_u32));
    assert_eq!(source_shielded_public, Numeric::from(200_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejected_when_disabled() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_unshield_rejected_when_disabled),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zkunshielddeny#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(300_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
                true,
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare unshield-disabled zk asset",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                200_u128,
                marker(9),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before disabled unshield failed",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(100_u32));

    let denied_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(9)],
                debug_ok_attachment(),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_unshield_tx,
        "disabled unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("unshield") || text.contains("disabled") || text.contains("forbidden")
            }),
            "expected unshield-disabled rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![Log::new(Level::INFO, "post denied unshield barrier".to_owned()).into()],
        &mut non_empty_target,
        "post denied unshield barrier failed",
    )
    .await?;

    let after_balance = numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_balance, Numeric::from(100_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_shield_rejected_when_disabled() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_shield_rejected_when_disabled),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zkshielddeny#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(300_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
                false,
                true,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare shield-disabled zk asset",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(300_u32));

    let denied_shield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                200_u128,
                marker(7),
                ConfidentialEncryptedPayload::default(),
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_shield_tx,
        "disabled shield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("shield") || text.contains("disabled") || text.contains("forbidden")
            }),
            "expected shield-disabled rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![Log::new(Level::INFO, "post denied shield barrier".to_owned()).into()],
        &mut non_empty_target,
        "post denied shield barrier failed",
    )
    .await?;

    let after_balance = numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_balance, Numeric::from(300_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_shield_rejected_without_zk_registration() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_shield_rejected_without_zk_registration),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zknotregistered#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(300_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
        ],
        &mut non_empty_target,
        "prepare non-zk asset for rejected shield",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(300_u32));

    let denied_shield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                200_u128,
                marker(11),
                ConfidentialEncryptedPayload::default(),
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_shield_tx,
        "shield without zk registration unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("shield") || text.contains("policy") || text.contains("permitted")
            }),
            "expected policy rejection for non-zk shield, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post denied shield without registration barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post denied shield without registration barrier failed",
    )
    .await?;

    let after_balance = numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_balance, Numeric::from(300_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejected_with_stale_root_hint() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_unshield_rejected_with_stale_root_hint),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zkstaleroot#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(400_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
                true,
                true,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare stale-root zk asset",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                250_u128,
                marker(41),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before stale-root unshield failed",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(150_u32));

    let stale_root_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(42)],
                debug_ok_attachment(),
                Some(marker(77)),
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &stale_root_unshield_tx,
        "stale-root unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("root") || text.contains("stale") || text.contains("unknown")
            }),
            "expected stale-root rejection signal, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![Log::new(Level::INFO, "post stale-root unshield barrier".to_owned()).into()],
        &mut non_empty_target,
        "post stale-root unshield barrier failed",
    )
    .await?;

    let after_balance = numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_balance, Numeric::from(150_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejected_without_zk_registration() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_unshield_rejected_without_zk_registration),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zkunshieldnotregistered#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(300_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
        ],
        &mut non_empty_target,
        "prepare non-zk asset for rejected unshield",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(300_u32));

    let denied_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(12)],
                debug_ok_attachment(),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_unshield_tx,
        "unshield without zk registration unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("unshield") || text.contains("policy") || text.contains("permitted")
            }),
            "expected policy rejection for non-zk unshield, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post denied unshield without registration barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post denied unshield without registration barrier failed",
    )
    .await?;

    let after_balance = numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_balance, Numeric::from(300_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_duplicate_nullifier_rejected() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer.write(["confidential", "enabled"], true);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(confidential_unshield_duplicate_nullifier_rejected),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;

    let tx_builder_client = network.client();
    let source = tx_builder_client.account.clone();

    let mut peer_clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    if peer_clients.is_empty() {
        peer_clients.push(tx_builder_client.clone());
    }

    let asset_def: AssetDefinitionId = "zkdupnullifier#wonderland".parse().unwrap();
    let mut non_empty_target = 1_u64;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(500_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid,
                true,
                true,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare duplicate-nullifier zk asset",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                300_u128,
                marker(91),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before duplicate-nullifier unshield failed",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                120_u128,
                vec![marker(61)],
                debug_ok_attachment(),
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "first unshield before duplicate-nullifier check failed",
    )
    .await?;

    let after_first_unshield = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(after_first_unshield, Numeric::from(320_u32));

    let duplicate_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                80_u128,
                vec![marker(61)],
                debug_ok_attachment(),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &duplicate_unshield_tx,
        "duplicate-nullifier unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("duplicate") || text.contains("nullifier")
            }),
            "expected duplicate-nullifier rejection signal, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post duplicate-nullifier unshield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post duplicate-nullifier unshield barrier failed",
    )
    .await?;

    let after_duplicate_attempt =
        numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_duplicate_attempt, Numeric::from(320_u32));

    Ok(())
}

#[test]
fn transient_client_error_detector_matches_expected_messages() {
    for message in [
        "Failed to send http request",
        "error sending request for url",
        "operation timed out",
        "Connection refused",
        "connection closed by peer",
        "connection reset by peer",
    ] {
        assert!(is_transient_client_error(&eyre!(message)));
    }
}

#[test]
fn transient_client_error_detector_ignores_non_transient_messages() {
    for message in [
        "validation failed",
        "not enough signatures",
        "permission denied",
    ] {
        assert!(!is_transient_client_error(&eyre!(message)));
    }
}

#[test]
fn duplicate_tx_error_detector_matches_expected_messages() {
    for message in [
        "PRTRY:ALREADY_COMMITTED",
        "PRTRY:ALREADY_ENQUEUED",
        "already_committed",
        "already_enqueued",
        "transaction already committed",
        "transaction already present in the queue",
    ] {
        assert!(is_duplicate_tx_error(&eyre!(message)));
    }
}

#[test]
fn duplicate_tx_error_detector_ignores_non_duplicate_messages() {
    for message in [
        "timeout waiting for commit",
        "invalid transaction",
        "queue overflow",
    ] {
        assert!(!is_duplicate_tx_error(&eyre!(message)));
    }
}
