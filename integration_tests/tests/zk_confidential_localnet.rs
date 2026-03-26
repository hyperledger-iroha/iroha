#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet confidential flow coverage:
//! public-origin shield -> two 3-hop shielded sequences -> unshield -> public transfer,
//! plus dedicated shielded-asset 3-hop and unshield/transfer scenarios, and
//! restart-pressure + malformed-proof rejection fault-injection checks.

use std::time::Duration;

use eyre::{Report, Result, WrapErr as _, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{
        AnonymityPolicy, Config, DEFAULT_TORII_API_MIN_PROOF_VERSION, default_connect_queue_root,
        default_torii_api_version,
    },
    data_model::{
        ChainId,
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
use iroha_test_samples::{BOB_ID, BOB_KEYPAIR};
use sorafs_manifest::alias_cache::AliasCachePolicy;

const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;
const ALL_PEER_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const STATUS_ERROR_STREAK_FOR_MUTE: usize = 3;
const MUTED_PEER_COOLDOWN_ATTEMPTS: usize = 5;
const RESTART_PROGRESS_TIMEOUT: Duration = Duration::from_secs(45);
const RESTART_PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const PRESSURE_TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const PRESSURE_TRANSACTION_STATUS_TIMEOUT: Duration = Duration::from_secs(2);

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

fn attachment_with_corrupted_proof(seed: [u8; 32]) -> ProofAttachment {
    let mut attachment = live_halo2_attachment(seed);
    if attachment.proof.bytes.is_empty() {
        attachment.proof.bytes.push(0);
    } else {
        attachment.proof.bytes[0] ^= 0x01;
    }
    attachment
}

fn attachment_with_corrupted_vk(seed: [u8; 32]) -> ProofAttachment {
    let mut attachment = live_halo2_attachment(seed);
    let vk_inline = attachment
        .vk_inline
        .as_mut()
        .expect("live attachment must include inline verifying key");
    if vk_inline.bytes.is_empty() {
        vk_inline.bytes.push(0);
    } else {
        vk_inline.bytes[0] ^= 0x01;
    }
    attachment
}

fn select_dual_restart_indices(total_peers: usize) -> (usize, usize) {
    assert!(
        total_peers >= 3,
        "dual restart stress needs at least three peers"
    );
    let first = 1 % total_peers;
    let mut second = 2 % total_peers;
    if second == first {
        second = (second + 1) % total_peers;
    }
    (first, second)
}

fn pressure_submitter_clients(submitters: &[Client]) -> Vec<Client> {
    submitters
        .iter()
        .cloned()
        .map(|mut client| {
            client.torii_request_timeout = PRESSURE_TORII_REQUEST_TIMEOUT;
            client.transaction_status_timeout = PRESSURE_TRANSACTION_STATUS_TIMEOUT;
            client
        })
        .collect()
}

async fn restart_peer_and_wait_non_empty(
    network: &sandbox::SerializedNetwork,
    peer_index: usize,
    target_non_empty: u64,
    context: &str,
) -> Result<()> {
    let peer = network.peers().get(peer_index).ok_or_else(|| {
        eyre!(
            "{context}: restart peer index {peer_index} out of range for {} peers",
            network.peers().len()
        )
    })?;

    let _ = peer.shutdown_if_started().await;
    let config_layers = network.config_layers().collect::<Vec<_>>();
    peer.start_checked(config_layers.iter().cloned(), None)
        .await
        .wrap_err_with(|| format!("{context}: restart peer {peer_index}"))?;

    let restart_client = peer.client();
    let deadline = tokio::time::Instant::now() + RESTART_PROGRESS_TIMEOUT;

    loop {
        match restart_client.get_status() {
            Ok(status) if status.blocks_non_empty >= target_non_empty => return Ok(()),
            Ok(_) => {}
            Err(err) if is_transient_client_error(&err) => {}
            Err(err) => {
                return Err(err)
                    .wrap_err_with(|| format!("{context}: query restarted peer {peer_index}"));
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: restarted peer {peer_index} did not reach non-empty height {target_non_empty} within {:?}",
                RESTART_PROGRESS_TIMEOUT
            ));
        }

        tokio::time::sleep(RESTART_PROGRESS_POLL_INTERVAL).await;
    }
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

async fn wait_for_numeric_balance_quorum(
    clients: &[Client],
    id: AssetId,
    expected: Numeric,
    quorum: usize,
    context: &str,
) -> Result<()> {
    const ATTEMPTS: usize = 60;
    const DELAY: Duration = Duration::from_millis(200);
    let mut last_values: Vec<Numeric> = Vec::new();
    let mut last_errors: Vec<String> = Vec::new();

    for _ in 0..ATTEMPTS {
        let mut matches = 0_usize;
        let mut values = Vec::new();
        let mut errors = Vec::new();

        for client in clients {
            match numeric_balance(client, id.clone()) {
                Ok(value) => {
                    if value == expected {
                        matches = matches.saturating_add(1);
                    }
                    values.push(value);
                }
                Err(err) => errors.push(err.to_string()),
            }
        }

        if matches >= quorum {
            return Ok(());
        }

        last_values = values;
        last_errors = errors;
        tokio::time::sleep(DELAY).await;
    }

    Err(eyre!(
        "{context}: expected balance {expected:?} on quorum {quorum}, last values {last_values:?}, last errors {last_errors:?}"
    ))
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

    let public_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkpublichop".parse().unwrap(),
    );
    let shielded_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkshieldhop".parse().unwrap(),
    );

    let setup_instructions: Vec<InstructionBox> = vec![
        Register::asset_definition({
            let __asset_definition_id = public_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = shielded_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
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

    // Flow 1: public-origin -> shield -> two 3-hop shielded sequences -> unshield -> public transfer.
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

    for output_commitment in [24_u8, 25_u8, 26_u8] {
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
            "public-origin second 3-hop shielded transfer failed",
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
async fn confidential_public_two_three_hop_sequences_allow_multiple_unshields_localnet()
-> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_public_two_three_hop_sequences_allow_multiple_unshields_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkpublicdoubleunshield".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
            Mint::asset_numeric(900_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
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
        "prepare public double-unshield flow",
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
                600_u128,
                marker(61),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "double-unshield flow shield failed",
    )
    .await?;

    for output_commitment in [62_u8, 63_u8, 64_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "double-unshield flow first 3-hop transfer failed",
        )
        .await?;
    }

    for output_commitment in [65_u8, 66_u8, 67_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "double-unshield flow second 3-hop transfer failed",
        )
        .await?;
    }

    for (amount, nullifier) in [(200_u128, 68_u8), (150_u128, 69_u8)] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::Unshield::new(
                    asset_def.clone(),
                    source.clone(),
                    amount,
                    vec![marker(nullifier)],
                    live_halo2_attachment(marker(nullifier)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "double-unshield flow unshield failed",
        )
        .await?;
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Transfer::asset_numeric(
                AssetId::new(asset_def.clone(), source.clone()),
                320_u64,
                recipient.clone(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "double-unshield flow transfer failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(330_u32),
        "wait source balance after double-unshield flow",
    )
    .await?;
    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, recipient.clone()),
        Numeric::from(320_u32),
        "wait recipient balance after double-unshield flow",
    )
    .await?;

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
    let shielded_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkshieldedthreehop".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = shielded_asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
async fn confidential_shielded_asset_three_hop_then_unshield_and_transfer_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_shielded_asset_three_hop_then_unshield_and_transfer_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkshieldedunshieldflow".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
            Mint::asset_numeric(800_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
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
        "prepare shielded-asset unshield flow",
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
                500_u128,
                marker(91),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shielded-asset unshield flow shield failed",
    )
    .await?;

    for output_commitment in [92_u8, 93_u8, 94_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "shielded-asset unshield flow 3-hop transfer failed",
        )
        .await?;
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                220_u128,
                vec![marker(95)],
                live_halo2_attachment(marker(95)),
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "shielded-asset unshield flow unshield failed",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Transfer::asset_numeric(
                AssetId::new(asset_def.clone(), source.clone()),
                120_u64,
                recipient.clone(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shielded-asset unshield flow transfer failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(400_u32),
        "wait source balance after shielded-asset unshield flow",
    )
    .await?;
    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, recipient.clone()),
        Numeric::from(120_u32),
        "wait recipient balance after shielded-asset unshield flow",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_dual_restart_stress_mid_flow_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_dual_restart_stress_mid_flow_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkrestartstress".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
            Mint::asset_numeric(1_100_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
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
        "prepare dual-restart stress flow",
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
                700_u128,
                marker(131),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "dual-restart stress shield failed",
    )
    .await?;

    for output_commitment in [132_u8, 133_u8, 134_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "dual-restart stress first 3-hop transfer failed",
        )
        .await?;
    }

    let (restart_a, restart_b) = select_dual_restart_indices(network.peers().len());
    restart_peer_and_wait_non_empty(
        &network,
        restart_a,
        non_empty_target,
        "dual-restart stress first restart",
    )
    .await?;

    for output_commitment in [135_u8, 136_u8, 137_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &peer_clients,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "dual-restart stress second 3-hop transfer failed",
        )
        .await?;
    }

    restart_peer_and_wait_non_empty(
        &network,
        restart_b,
        non_empty_target,
        "dual-restart stress second restart",
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
                300_u128,
                vec![marker(138)],
                live_halo2_attachment(marker(138)),
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "dual-restart stress unshield failed",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Transfer::asset_numeric(
                AssetId::new(asset_def.clone(), source.clone()),
                200_u64,
                recipient.clone(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "dual-restart stress transfer failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(500_u32),
        "wait source balance after dual-restart stress",
    )
    .await?;
    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, recipient.clone()),
        Numeric::from(200_u32),
        "wait recipient balance after dual-restart stress",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_combined_peer_downtime_and_timeout_pressure_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_combined_peer_downtime_and_timeout_pressure_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkfaultpressure".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
            Mint::asset_numeric(900_u64, AssetId::new(asset_def.clone(), source.clone())).into(),
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
        "prepare combined downtime+timeout flow",
    )
    .await?;

    let pressure_submitters = pressure_submitter_clients(&peer_clients);
    let (restart_idx, _) = select_dual_restart_indices(network.peers().len());
    let _ = network.peers()[restart_idx].shutdown_if_started().await;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &pressure_submitters,
        vec![
            iroha_data_model::isi::zk::Shield::new(
                asset_def.clone(),
                source.clone(),
                500_u128,
                marker(151),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "combined downtime+timeout shield failed",
    )
    .await?;

    for output_commitment in [152_u8, 153_u8, 154_u8] {
        submit_and_wait_non_empty_block(
            &network,
            &tx_builder_client,
            &pressure_submitters,
            vec![
                iroha_data_model::isi::zk::ZkTransfer::new(
                    asset_def.clone(),
                    Vec::new(),
                    vec![marker(output_commitment)],
                    live_halo2_attachment(marker(output_commitment)),
                    None,
                )
                .into(),
            ],
            &mut non_empty_target,
            "combined downtime+timeout 3-hop transfer failed",
        )
        .await?;
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &pressure_submitters,
        vec![
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                220_u128,
                vec![marker(155)],
                live_halo2_attachment(marker(155)),
                None,
            )
            .into(),
        ],
        &mut non_empty_target,
        "combined downtime+timeout unshield failed",
    )
    .await?;

    restart_peer_and_wait_non_empty(
        &network,
        restart_idx,
        non_empty_target,
        "combined downtime+timeout restart peer",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Transfer::asset_numeric(
                AssetId::new(asset_def.clone(), source.clone()),
                120_u64,
                recipient.clone(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "combined downtime+timeout transfer failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(500_u32),
        "wait source balance after combined downtime+timeout flow",
    )
    .await?;
    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, recipient.clone()),
        Numeric::from(120_u32),
        "wait recipient balance after combined downtime+timeout flow",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejects_corrupted_proof_bytes_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_rejects_corrupted_proof_bytes_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkbadproofbytes".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
        "prepare corrupted-proof-bytes reject flow",
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
                marker(161),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before corrupted-proof-bytes unshield failed",
    )
    .await?;

    let denied_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(162)],
                attachment_with_corrupted_proof(marker(163)),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_tx,
        "corrupted-proof-bytes unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("proof")
                    || text.contains("verify")
                    || text.contains("invalid")
                    || text.contains("envelope")
            }),
            "expected corrupted-proof rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post corrupted-proof-bytes rejected unshield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post corrupted-proof-bytes barrier failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, source),
        Numeric::from(150_u32),
        "wait balance after corrupted-proof-bytes rejection",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejects_corrupted_vk_bytes_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_rejects_corrupted_vk_bytes_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkbadvkbytes".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
        "prepare corrupted-vk-bytes reject flow",
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
                marker(171),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before corrupted-vk-bytes unshield failed",
    )
    .await?;

    let denied_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(172)],
                attachment_with_corrupted_vk(marker(173)),
                None,
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_tx,
        "corrupted-vk-bytes unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("vk")
                    || text.contains("key")
                    || text.contains("verify")
                    || text.contains("invalid")
            }),
            "expected corrupted-vk rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post corrupted-vk-bytes rejected unshield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post corrupted-vk-bytes barrier failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, source),
        Numeric::from(150_u32),
        "wait balance after corrupted-vk-bytes rejection",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_unshield_rejects_wrong_statement_hint_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_unshield_rejects_wrong_statement_hint_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkwrongstatement".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
        "prepare wrong-statement reject flow",
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
                marker(181),
                ConfidentialEncryptedPayload::default(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "shield before wrong-statement unshield failed",
    )
    .await?;

    let denied_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(
            iroha_data_model::isi::zk::Unshield::new(
                asset_def.clone(),
                source.clone(),
                100_u128,
                vec![marker(182)],
                live_halo2_attachment(marker(183)),
                Some(marker(199)),
            ),
        )],
        iroha_data_model::metadata::Metadata::default(),
    );

    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_tx,
        "wrong-statement unshield unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("root")
                    || text.contains("statement")
                    || text.contains("stale")
                    || text.contains("invalid")
            }),
            "expected wrong-statement rejection, got: {err:?}"
        );
    }

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Log::new(
                Level::INFO,
                "post wrong-statement rejected unshield barrier".to_owned(),
            )
            .into(),
        ],
        &mut non_empty_target,
        "post wrong-statement barrier failed",
    )
    .await?;

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, source),
        Numeric::from(150_u32),
        "wait balance after wrong-statement rejection",
    )
    .await?;

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
    let zknative_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkzknativethreehop".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = zknative_asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
async fn confidential_zknative_transparent_mint_creates_public_balance_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_zknative_transparent_mint_creates_public_balance_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkzknativemintok".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
        "prepare zknative transparent-mint asset",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![Mint::asset_numeric(10_u64, AssetId::new(asset_def.clone(), source.clone())).into()],
        &mut non_empty_target,
        "zknative transparent mint failed",
    )
    .await?;

    let quorum = peer_clients.len().saturating_sub(1).max(1);
    wait_for_numeric_balance_quorum(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(10_u32),
        quorum,
        "wait zknative transparent balance after mint",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn confidential_zknative_transparent_transfer_after_mint_rejected_localnet() -> Result<()> {
    let Some(ConfidentialLocalnetCtx {
        network,
        tx_builder_client,
        peer_clients,
        mut non_empty_target,
    }) = start_confidential_localnet(stringify!(
        confidential_zknative_transparent_transfer_after_mint_rejected_localnet
    ))
    .await?
    else {
        return Ok(());
    };

    let source = tx_builder_client.account.clone();
    let recipient = BOB_ID.clone();
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkzknativetransferok".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
        "prepare zknative transparent-transfer asset",
    )
    .await?;

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![Mint::asset_numeric(25_u64, AssetId::new(asset_def.clone(), source.clone())).into()],
        &mut non_empty_target,
        "zknative transparent mint before transfer failed",
    )
    .await?;

    let quorum = peer_clients.len().saturating_sub(1).max(1);
    wait_for_numeric_balance_quorum(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(25_u32),
        quorum,
        "wait zknative source balance before transfer",
    )
    .await?;

    let denied_transfer_tx = tx_builder_client.build_transaction_from_items(
        vec![InstructionBox::from(Transfer::asset_numeric(
            AssetId::new(asset_def.clone(), source.clone()),
            10_u64,
            recipient.clone(),
        ))],
        iroha_data_model::metadata::Metadata::default(),
    );
    let submit_result = submit_transaction_on_any_peer(
        &peer_clients,
        &denied_transfer_tx,
        "zknative transparent transfer unexpectedly accepted",
    );
    if let Err(err) = submit_result {
        assert!(
            err.chain().any(|cause| {
                let text = cause.to_string().to_lowercase();
                text.contains("transparent transfer")
                    || text.contains("shielded")
                    || text.contains("not permitted")
            }),
            "expected transparent transfer rejection for zknative mode, got: {err:?}"
        );
    }

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

    wait_for_numeric_balance_quorum(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(25_u32),
        quorum,
        "wait zknative source balance after denied transparent transfer",
    )
    .await?;
    if let Ok(value) =
        numeric_balance_any(&peer_clients, AssetId::new(asset_def, recipient.clone()))
    {
        assert_eq!(
            value,
            Numeric::from(0_u32),
            "recipient should remain empty after denied zknative transparent transfer"
        );
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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkunshielddeny".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    let quorum = peer_clients.len().saturating_sub(1).max(1);
    wait_for_numeric_balance_quorum(
        &peer_clients,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(100_u32),
        quorum,
        "wait source balance after shield before disabled unshield",
    )
    .await?;

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

    wait_for_numeric_balance_quorum(
        &peer_clients,
        AssetId::new(asset_def, source),
        Numeric::from(100_u32),
        quorum,
        "wait source balance after denied unshield",
    )
    .await?;

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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkshielddeny".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zknotregistered".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkstaleroot".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkunshieldnotregistered".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zkdupnullifier".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def.clone(), source.clone()),
        Numeric::from(320_u32),
        "wait balance after first unshield in duplicate-nullifier scenario",
    )
    .await?;

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

    wait_for_numeric_balance(
        &tx_builder_client,
        AssetId::new(asset_def, source),
        Numeric::from(320_u32),
        "wait balance after duplicate-nullifier attempt",
    )
    .await?;

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
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zktransparentonly".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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
    let asset_def: AssetDefinitionId = AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "zktransfertransparentonly".parse().unwrap(),
    );

    submit_and_wait_non_empty_block(
        &network,
        &tx_builder_client,
        &peer_clients,
        vec![
            Register::asset_definition({
                let __asset_definition_id = asset_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
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

#[test]
fn select_dual_restart_indices_is_deterministic_and_distinct() {
    let (first, second) = select_dual_restart_indices(4);
    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_ne!(first, second);
}

#[test]
fn pressure_submitter_clients_applies_short_timeouts() {
    let ttl = Duration::from_secs(1);
    let config = Config {
        chain: ChainId::from("test"),
        key_pair: BOB_KEYPAIR.clone(),
        account: BOB_ID.clone(),
        torii_api_url: "http://127.0.0.1:1".parse().expect("valid url"),
        torii_api_version: default_torii_api_version(),
        torii_api_min_proof_version: DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
        torii_request_timeout: Duration::from_millis(50),
        basic_auth: None,
        transaction_add_nonce: false,
        transaction_ttl: ttl,
        transaction_status_timeout: ttl,
        connect_queue_root: default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: AliasCachePolicy::new(ttl, ttl, ttl, ttl, ttl, ttl, ttl, ttl),
        sorafs_anonymity_policy: AnonymityPolicy::default(),
        sorafs_rollout_phase: iroha_config::parameters::actual::SorafsRolloutPhase::default(),
    };
    let template = Client::new(config);
    let pressure = pressure_submitter_clients(&[template]);
    assert_eq!(pressure.len(), 1);
    assert_eq!(
        pressure[0].torii_request_timeout,
        PRESSURE_TORII_REQUEST_TIMEOUT
    );
    assert_eq!(
        pressure[0].transaction_status_timeout,
        PRESSURE_TRANSACTION_STATUS_TIMEOUT
    );
}

#[test]
fn corrupted_proof_helper_mutates_proof_bytes() {
    let seed = marker(222);
    let original = live_halo2_attachment(seed);
    let tampered = attachment_with_corrupted_proof(seed);
    assert_ne!(tampered.proof.bytes, original.proof.bytes);
}

#[test]
fn corrupted_vk_helper_mutates_vk_bytes() {
    let seed = marker(223);
    let original = live_halo2_attachment(seed);
    let tampered = attachment_with_corrupted_vk(seed);
    let original_vk = original
        .vk_inline
        .expect("live attachment should have inline vk")
        .bytes;
    let tampered_vk = tampered
        .vk_inline
        .expect("tampered attachment should have inline vk")
        .bytes;
    assert_ne!(tampered_vk, original_vk);
}
