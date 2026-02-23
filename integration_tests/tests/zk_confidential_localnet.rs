#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet confidential flow coverage:
//! public-origin shield -> 3 shielded hops -> unshield -> public transfer,
//! plus a second shielded asset with 3 shielded hops.

use std::time::Duration;

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
use iroha_core::zk::test_utils::halo2_ivm_execution_envelope;
use iroha_data_model::proof::ProofAttachment;
use iroha_test_network::{NetworkBuilder, NetworkPeer};
use iroha_test_samples::BOB_ID;

const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;
const ALL_PEER_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const STATUS_ERROR_STREAK_FOR_MUTE: usize = 3;
const MUTED_PEER_COOLDOWN_ATTEMPTS: usize = 5;

fn marker(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn hash_for_live_proof(domain: &[u8], seed: [u8; 32]) -> iroha_crypto::Hash {
    let mut preimage = Vec::with_capacity(domain.len() + seed.len());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(&seed);
    iroha_crypto::Hash::new(preimage)
}

fn live_halo2_attachment(seed: [u8; 32]) -> ProofAttachment {
    let fixture = halo2_ivm_execution_envelope(
        hash_for_live_proof(b"zk-confidential-localnet/code", seed),
        hash_for_live_proof(b"zk-confidential-localnet/overlay", seed),
        hash_for_live_proof(b"zk-confidential-localnet/events", seed),
        hash_for_live_proof(b"zk-confidential-localnet/gas-policy", seed),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture must include a verifying key");
    let proof_box = fixture.proof_box("halo2/ipa");
    ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk_box)
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

async fn wait_for_numeric_balance(
    client: &Client,
    id: AssetId,
    expected: Numeric,
    context: &str,
) -> Result<()> {
    const ATTEMPTS: usize = 30;
    const DELAY: Duration = Duration::from_millis(200);
    let mut last_value: Option<Numeric> = None;
    let mut last_err: Option<Report> = None;

    for _ in 0..ATTEMPTS {
        match numeric_balance(client, id.clone()) {
            Ok(value) if value == expected => return Ok(()),
            Ok(value) => {
                last_value = Some(value);
                last_err = None;
            }
            Err(err) => {
                last_err = Some(err);
            }
        }
        tokio::time::sleep(DELAY).await;
    }

    if let Some(err) = last_err {
        Err(err).wrap_err_with(|| format!("{context}: expected balance {expected:?}"))
    } else {
        Err(eyre!(
            "{context}: expected balance {expected:?}, last observed {:?}",
            last_value
        ))
    }
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

fn is_transient_localnet_startup_error(err: &Report) -> bool {
    const NEEDLES: [&str; 4] = [
        "terminated within 5s post-genesis window",
        "error sending request for url",
        "Connection refused",
        "operation timed out",
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
    let all_peer_wait_error = match tokio::time::timeout(
        ALL_PEER_WAIT_TIMEOUT,
        network.ensure_blocks_with(|height| height.non_empty >= target),
    )
    .await
    {
        Ok(Ok(_)) => None,
        Ok(Err(err)) => Some(format!("{err:?}")),
        Err(err) => Some(format!(
            "timed out after {:?}: {err:?}",
            ALL_PEER_WAIT_TIMEOUT
        )),
    };

    if let Some(err) = all_peer_wait_error {
        let quorum = submitters.len().saturating_sub(1).max(1);
        wait_for_non_empty_quorum(submitters, target, quorum, context)
            .await
            .wrap_err_with(|| {
                format!(
                    "{context}: wait non-empty block {target} (all-peer wait failed first: {err:?})"
                )
            })?;
    }

    Ok(())
}

async fn wait_for_non_empty_quorum(
    clients: &[Client],
    target: u64,
    quorum: usize,
    context: &str,
) -> Result<()> {
    const ATTEMPTS: usize = 200;
    const DELAY: Duration = Duration::from_millis(300);
    let mut last_observed = Vec::new();
    let mut heights = Vec::new();
    let mut error_streaks = vec![0_usize; clients.len()];
    let mut muted_until_attempt = vec![0_usize; clients.len()];

    for attempt in 0..ATTEMPTS {
        heights.clear();
        last_observed.clear();
        let mut currently_muted = muted_peer_count(&muted_until_attempt, attempt);

        for (idx, client) in clients.iter().enumerate() {
            if is_peer_muted(&muted_until_attempt, idx, attempt) {
                heights.push(None);
                last_observed.push(format!("muted:{idx}@{}", muted_until_attempt[idx]));
                continue;
            }

            match client.get_status() {
                Ok(status) => {
                    let height = status.blocks_non_empty;
                    error_streaks[idx] = 0;
                    heights.push(Some(height));
                    last_observed.push(format!("ok:{height}"));
                }
                Err(err) => {
                    error_streaks[idx] = error_streaks[idx].saturating_add(1);
                    if should_temporarily_mute_peer(
                        error_streaks[idx],
                        currently_muted,
                        clients.len(),
                        quorum,
                    ) {
                        muted_until_attempt[idx] =
                            attempt.saturating_add(MUTED_PEER_COOLDOWN_ATTEMPTS);
                        currently_muted = currently_muted.saturating_add(1);
                        error_streaks[idx] = 0;
                    }
                    heights.push(None);
                    last_observed.push(format!("err:{err}"));
                }
            }
        }

        if count_non_empty_reached(&heights, target) >= quorum {
            return Ok(());
        }

        tokio::time::sleep(DELAY).await;
    }

    Err(eyre!(
        "{context}: expected non-empty block {target} on quorum {quorum}, last observed {:?}",
        last_observed
    ))
}

fn should_temporarily_mute_peer(
    error_streak: usize,
    currently_muted: usize,
    total_clients: usize,
    quorum: usize,
) -> bool {
    if error_streak < STATUS_ERROR_STREAK_FOR_MUTE {
        return false;
    }
    total_clients.saturating_sub(currently_muted.saturating_add(1)) >= quorum
}

fn is_peer_muted(muted_until_attempt: &[usize], index: usize, attempt: usize) -> bool {
    attempt < muted_until_attempt[index]
}

fn muted_peer_count(muted_until_attempt: &[usize], attempt: usize) -> usize {
    muted_until_attempt
        .iter()
        .filter(|&&muted_until| attempt < muted_until)
        .count()
}

fn count_non_empty_reached(heights: &[Option<u64>], target: u64) -> usize {
    heights
        .iter()
        .filter(|height| height.is_some_and(|value| value >= target))
        .count()
}

struct ConfidentialLocalnetCtx {
    network: sandbox::SerializedNetwork,
    tx_builder_client: Client,
    peer_clients: Vec<Client>,
    non_empty_target: u64,
}

async fn start_confidential_localnet(test_name: &str) -> Result<Option<ConfidentialLocalnetCtx>> {
    const START_ATTEMPTS: usize = 3;
    const RETRY_DELAY: Duration = Duration::from_millis(500);

    for attempt in 1..=START_ATTEMPTS {
        let builder = NetworkBuilder::new()
            .with_peers(4)
            .with_auto_populated_trusted_peers()
            .with_config_layer(|layer| {
                layer
                    .write(["confidential", "enabled"], true)
                    .write(["zk", "halo2", "enabled"], true)
                    .write(
                        ["confidential", "verify_timeout_ms"],
                        PROOF_VERIFY_TIMEOUT_MS,
                    );
            });

        let Some(network) = (match sandbox::start_network_async_or_skip(builder, test_name).await {
            Ok(network) => network,
            Err(err) if attempt < START_ATTEMPTS && is_transient_localnet_startup_error(&err) => {
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
            Err(err) => return Err(err).wrap_err("start confidential localnet"),
        }) else {
            return Ok(None);
        };

        network.ensure_blocks(1).await?;

        let tx_builder_client = network.client();
        let mut peer_clients = network
            .peers()
            .iter()
            .map(NetworkPeer::client)
            .collect::<Vec<_>>();
        if peer_clients.is_empty() {
            peer_clients.push(tx_builder_client.clone());
        }

        return Ok(Some(ConfidentialLocalnetCtx {
            network,
            tx_builder_client,
            peer_clients,
            non_empty_target: 1_u64,
        }));
    }

    Err(eyre!(
        "confidential localnet startup retries exhausted for {test_name}"
    ))
}

#[tokio::test]
async fn confidential_public_and_shielded_three_hop_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_public_and_shielded_three_hop_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();

    let public_asset_def: AssetDefinitionId = "zkpublichop#wonderland".parse().unwrap();
    let shielded_asset_def: AssetDefinitionId = "zkshieldhop#wonderland".parse().unwrap();

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
                    live_halo2_attachment(marker(output_commitment)),
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
                live_halo2_attachment(marker(31)),
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
                    live_halo2_attachment(marker(output_commitment)),
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
async fn confidential_shielded_asset_three_hop_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(confidential_shielded_asset_three_hop_localnet))
        .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let shielded_asset_def: AssetDefinitionId = "zkshieldedthreehop#wonderland".parse().unwrap();

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(shielded_asset_def.clone())).into(),
            Mint::asset_numeric(
                700_u64,
                AssetId::new(shielded_asset_def.clone(), source.clone()),
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
        ],
        &mut non_empty_target,
        "prepare dedicated shielded-asset flow",
    )
    .await?;

    let before_shield = numeric_balance_any(
        &peer_clients,
        AssetId::new(shielded_asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_shield, Numeric::from(700_u32));

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                shielded_asset_def.clone(),
                source.clone(),
                500_u128,
                marker(71),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "dedicated shielded-asset shield failed",
    )
    .await?;

    for output_commitment in [72_u8, 73_u8, 74_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    shielded_asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "dedicated shielded-asset 3-hop transfer failed",
        )
        .await?;
    }

    let after_three_hops =
        numeric_balance_any(&peer_clients, AssetId::new(shielded_asset_def, source))?;
    assert_eq!(after_three_hops, Numeric::from(200_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_zknative_asset_three_hop_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(confidential_zknative_asset_three_hop_localnet))
        .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let zknative_asset_def: AssetDefinitionId = "zkzknativethreehop#wonderland".parse().unwrap();

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(zknative_asset_def.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                zknative_asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::ZkNative,
                false,
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare zknative shielded-only asset",
    )
    .await?;

    for output_commitment in [81_u8, 82_u8, 83_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    zknative_asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "zknative shielded-asset 3-hop transfer failed",
        )
        .await?;
    }

    let transparent_balance =
        numeric_balance_any(&peer_clients, AssetId::new(zknative_asset_def, source));
    assert!(
        transparent_balance.is_err(),
        "zknative asset unexpectedly exposes a transparent balance"
    );

    Ok(())
}

#[tokio::test]
async fn confidential_zknative_transparent_mint_rejected_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_zknative_transparent_mint_rejected_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = "zkzknativemintok#wonderland".parse().unwrap();

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::ZkNative,
                false,
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare zknative shielded-only mint-rejection asset",
    )
    .await?;

    let denied_mint_tx = tx_builder_client.build_transaction_from_items(
        vec![Mint::asset_numeric(10_u64, AssetId::new(asset_def.clone(), source.clone())).into()],
        iroha_data_model::metadata::Metadata::default(),
    );
    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_mint_tx,
        "zknative transparent mint unexpectedly accepted",
    );
    let err = submit_result.expect_err("transparent mint should be rejected for zknative policy");
    assert!(
        err.chain().any(|cause| {
            let text = cause.to_string().to_lowercase();
            text.contains("transparent mint not permitted by policy")
                || text.contains("not permitted by policy")
                || text.contains("shielded")
        }),
        "expected transparent mint rejection signal, got: {err:?}"
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post denied zknative transparent mint barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post denied zknative transparent mint barrier failed",
    )
    .await?;

    match numeric_balance_any(&peer_clients, AssetId::new(asset_def, source)) {
        Ok(value) => assert_eq!(
            value,
            Numeric::from(0_u32),
            "zknative denied mint unexpectedly changed transparent balance"
        ),
        Err(_) => {}
    }

    Ok(())
}

#[tokio::test]
async fn confidential_zknative_transparent_transfer_rejected_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_zknative_transparent_transfer_rejected_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = "zkzknativetransferok#wonderland".parse().unwrap();

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition(AssetDefinition::numeric(asset_def.clone())).into(),
            Mint::asset_numeric(25_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
        ],
        &mut non_empty_target,
        "prepare public balance before zknative policy switch",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(25_u32),
        "wait pre-zknative source balance before transfer rejection",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::RegisterZkAsset::new(
                asset_def.clone(),
                iroha_data_model::isi::zk::ZkAssetMode::ZkNative,
                false,
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "apply zknative shielded-only policy before transfer",
    )
    .await?;

    let denied_transfer_tx = tx_builder_client.build_transaction_from_items(
        vec![
            Transfer::asset_numeric(
                AssetId::new(asset_def.clone(), source.clone()),
                10_u64,
                recipient.clone(),
            )
            .into(),
        ],
        iroha_data_model::metadata::Metadata::default(),
    );
    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_transfer_tx,
        "zknative transparent transfer unexpectedly accepted",
    );
    let err =
        submit_result.expect_err("transparent transfer should be rejected for zknative policy");
    assert!(
        err.chain().any(|cause| {
            let text = cause.to_string().to_lowercase();
            text.contains("transparent transfer not permitted by policy")
                || text.contains("policy")
                || text.contains("permitted")
        }),
        "expected transparent transfer rejection signal, got: {err:?}"
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post denied zknative transparent transfer barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post denied zknative transparent transfer barrier failed",
    )
    .await?;

    if let Ok(value) = numeric_balance_any(&peer_clients, AssetId::new(asset_def.clone(), source)) {
        assert_eq!(
            value,
            Numeric::from(25_u32),
            "zknative denied transfer unexpectedly changed source balance"
        );
    }

    match numeric_balance_any(&peer_clients, AssetId::new(asset_def, recipient)) {
        Ok(value) => assert_eq!(
            value,
            Numeric::from(0_u32),
            "zknative denied transfer unexpectedly credited recipient balance"
        ),
        Err(_) => {}
    }

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejected_when_disabled() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(confidential_unshield_rejected_when_disabled))
        .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zkunshielddeny#wonderland".parse().unwrap();

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
                live_halo2_attachment(marker(109)),
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
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) =
        start_confidential_localnet(stringify!(confidential_shield_rejected_when_disabled)).await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zkshielddeny#wonderland".parse().unwrap();

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
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_shield_rejected_without_zk_registration
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zknotregistered#wonderland".parse().unwrap();

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
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_rejected_with_stale_root_hint
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zkstaleroot#wonderland".parse().unwrap();

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

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(150_u32),
        "wait stale-root precondition after shield",
    )
    .await?;

    let stale_root_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(42)],
                live_halo2_attachment(marker(142)),
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

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, source),
        Numeric::from(150_u32),
        "wait stale-root balance after denied unshield",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejected_without_zk_registration() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_rejected_without_zk_registration
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zkunshieldnotregistered#wonderland".parse().unwrap();

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
                live_halo2_attachment(marker(112)),
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
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_duplicate_nullifier_rejected
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();

    let asset_def: AssetDefinitionId = "zkdupnullifier#wonderland".parse().unwrap();

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
                live_halo2_attachment(marker(161)),
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
                live_halo2_attachment(marker(162)),
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

#[tokio::test]
async fn confidential_shield_and_unshield_rejected_in_transparent_only_mode() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_shield_and_unshield_rejected_in_transparent_only_mode
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = "zktransparentonly#wonderland".parse().unwrap();

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
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare transparent-only confidential policy asset",
    )
    .await?;

    let initial_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(initial_balance, Numeric::from(300_u32));

    let denied_shield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                200_u128,
                marker(15),
                ConfidentialEncryptedPayload::default(),
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );
    let shield_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_shield_tx,
        "transparent-only shield unexpectedly accepted",
    );
    if let Err(err) = shield_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("policy") || text.contains("permitted")
            }),
            "expected transparent-only shield policy rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post transparent-only denied shield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post transparent-only denied shield barrier failed",
    )
    .await?;

    let after_denied_shield = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(after_denied_shield, Numeric::from(300_u32));

    let denied_unshield_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(16)],
                live_halo2_attachment(marker(116)),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );
    let unshield_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_unshield_tx,
        "transparent-only unshield unexpectedly accepted",
    );
    if let Err(err) = unshield_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("policy") || text.contains("permitted")
            }),
            "expected transparent-only unshield policy rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post transparent-only denied unshield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post transparent-only denied unshield barrier failed",
    )
    .await?;

    let after_denied_unshield =
        numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_denied_unshield, Numeric::from(300_u32));

    Ok(())
}

#[tokio::test]
async fn confidential_transfer_rejected_in_transparent_only_mode() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_transfer_rejected_in_transparent_only_mode
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = "zktransfertransparentonly#wonderland".parse().unwrap();

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
                false,
                None,
                None,
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "prepare transparent-only zk-transfer policy asset",
    )
    .await?;

    let before_balance = numeric_balance_any(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
    )?;
    assert_eq!(before_balance, Numeric::from(300_u32));

    let denied_transfer_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::ZkTransfer::new(
                asset_def.clone(),
                Vec::new(),
                vec![marker(33)],
                live_halo2_attachment(marker(133)),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let transfer_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_transfer_tx,
        "transparent-only transfer unexpectedly accepted",
    );
    if let Err(err) = transfer_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("transfer") || text.contains("policy") || text.contains("permitted")
            }),
            "expected transparent-only transfer policy rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post transparent-only denied transfer barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post transparent-only denied transfer barrier failed",
    )
    .await?;

    let after_denied_transfer =
        numeric_balance_any(&peer_clients, AssetId::new(asset_def, source))?;
    assert_eq!(after_denied_transfer, Numeric::from(300_u32));

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

#[test]
fn transient_localnet_startup_error_detector_matches_expected_messages() {
    for message in [
        "terminated within 5s post-genesis window",
        "error sending request for url",
        "Connection refused",
        "operation timed out",
    ] {
        assert!(is_transient_localnet_startup_error(&eyre!(message)));
    }
}

#[test]
fn transient_localnet_startup_error_detector_ignores_non_startup_messages() {
    for message in [
        "permission denied",
        "validation failed",
        "duplicate nullifier",
    ] {
        assert!(!is_transient_localnet_startup_error(&eyre!(message)));
    }
}

#[test]
fn count_non_empty_reached_counts_only_target_or_above() {
    let heights = vec![Some(4_u64), Some(6_u64), None, Some(7_u64), Some(5_u64)];
    assert_eq!(count_non_empty_reached(&heights, 6), 2);
    assert_eq!(count_non_empty_reached(&heights, 5), 3);
}

#[test]
fn should_temporarily_mute_peer_requires_error_streak_threshold() {
    assert!(!should_temporarily_mute_peer(2, 0, 4, 3));
    assert!(should_temporarily_mute_peer(3, 0, 4, 3));
}

#[test]
fn should_temporarily_mute_peer_preserves_quorum_capacity() {
    assert!(!should_temporarily_mute_peer(3, 1, 4, 3));
    assert!(should_temporarily_mute_peer(3, 0, 4, 3));
}

#[test]
fn muted_peer_count_counts_only_currently_muted_peers() {
    let muted_until_attempt = vec![0_usize, 3, 5, 1];
    assert_eq!(muted_peer_count(&muted_until_attempt, 0), 3);
    assert_eq!(muted_peer_count(&muted_until_attempt, 1), 2);
    assert_eq!(muted_peer_count(&muted_until_attempt, 3), 1);
    assert_eq!(muted_peer_count(&muted_until_attempt, 5), 0);
}
