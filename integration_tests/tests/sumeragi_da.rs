#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Canonical Sumeragi v2 data-availability integration coverage.
//!
//! The first-release v2 runtime has no global RBC status endpoint or persisted
//! global RBC session store. Availability is committed by the signed genesis
//! context and observed through the authoritative v2 status and committed
//! subject.
use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::future::try_join_all;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        block::consensus_v2::SumeragiV2Status,
        isi::{Log, SetParameter},
        parameter::{Parameter, TransactionParameter},
        peer::PeerId,
    },
};
use iroha_test_network::{
    ConsensusMessageControlAction, ConsensusMessageControlKind, ConsensusMessageControlRule,
    NetworkBuilder, init_instruction_registry,
};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};
const LARGE_PAYLOAD_BYTES: usize = 1024 * 1024;
const PACKET_LOSS_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;
const PACKET_LOSS_HEIGHT: u64 = 2;
const PACKET_LOSS_VIEW: u64 = 0;
const PACKET_LOSS_CHUNK_INDICES: [u32; 3] = [57, 58, 59];
const PACKET_LOSS_QUEUE_CAPACITY: usize = 16;
const PACKET_LOSS_CONTROL_TIMEOUT: Duration = Duration::from_secs(90);
const TORII_CONTENT_HEADROOM_BYTES: usize = 2 * 1024 * 1024;
const TORII_MAX_CONTENT_LEN_BYTES: i64 = 64_000_000;
const NETWORK_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const NETWORK_TOPIC_FRAME_BUDGET_BYTES: i64 = NETWORK_FRAME_BUDGET_BYTES - 28;
const NETWORK_STREAM_FRAME_BUDGET_BYTES: i64 = NETWORK_FRAME_BUDGET_BYTES + 4;
const NETWORK_DEFERRED_SEND_BUDGET_BYTES: i64 = 2 * NETWORK_STREAM_FRAME_BUDGET_BYTES;
const TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES: i64 = 1024 * 1024 * 1024;
const COMMIT_WAIT_BUDGET: Duration = Duration::from_secs(480);
fn torii_max_content_len_for_payload(payload_bytes: usize) -> i64 {
    let inflated = payload_bytes.saturating_mul(12);
    let with_headroom = payload_bytes.saturating_add(TORII_CONTENT_HEADROOM_BYTES);
    i64::try_from(inflated.max(with_headroom))
        .unwrap_or(i64::MAX)
        .min(TORII_MAX_CONTENT_LEN_BYTES)
}
fn tx_limit_for_payload(payload_bytes: usize) -> NonZeroU64 {
    NonZeroU64::new(
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX),
    )
    .expect("payload-driven transaction limit must be non-zero")
}
fn large_da_network_builder(peers: usize, payload_bytes: usize) -> NetworkBuilder {
    let tx_limit = tx_limit_for_payload(payload_bytes);
    NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(payload_bytes),
                )
                .write(["network", "max_frame_bytes"], NETWORK_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "p2p_outbound_frame_queue_max_high_bytes"],
                    NETWORK_STREAM_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "p2p_outbound_frame_queue_max_low_bytes"],
                    NETWORK_STREAM_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "deferred_send_max_bytes_per_peer"],
                    NETWORK_DEFERRED_SEND_BUDGET_BYTES,
                )
                .write(
                    ["network", "deferred_send_max_bytes_total"],
                    NETWORK_DEFERRED_SEND_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    NETWORK_TOPIC_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit),
        )))
}
fn hold_view_zero_payload_chunks(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    peer_ids
        .iter()
        .enumerate()
        .filter(|(sender_index, _)| *sender_index != receiver_index)
        .flat_map(|(_, sender)| {
            PACKET_LOSS_CHUNK_INDICES.map(|index| {
                ConsensusMessageControlRule::payload_chunk_from_proposal(
                    sender.clone(),
                    PACKET_LOSS_HEIGHT,
                    PACKET_LOSS_VIEW,
                    index,
                    ConsensusMessageControlAction::Hold,
                )
            })
        })
        .collect()
}
fn validate_committed_da_status(status: &SumeragiV2Status, expected_height: u64) -> Result<()> {
    status
        .validate()
        .map_err(|err| eyre!("invalid canonical Sumeragi v2 status: {err}"))?;
    ensure!(
        status.last_committed_height >= expected_height,
        "peer committed height {} is below expected DA height {expected_height}",
        status.last_committed_height
    );
    ensure!(
        status.last_committed_subject.is_some(),
        "peer omitted the committed v2 subject"
    );
    Ok(())
}
fn fetch_v2_status(client: Client) -> Result<SumeragiV2Status> {
    client
        .get_sumeragi_status()
        .wrap_err("fetch canonical Sumeragi v2 status")
}
async fn large_da_payload_commits_with_consistent_v2_subject_for_committee(
    peers: usize,
    scenario: &str,
) -> Result<()> {
    init_instruction_registry();
    let Some(network) = sandbox::start_network_async_or_skip(
        large_da_network_builder(peers, LARGE_PAYLOAD_BYTES),
        scenario,
    )
    .await?
    else {
        return Ok(());
    };
    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;
    let client = network.client();
    let expected_height = client.get_status()?.blocks.saturating_add(1);
    let payload = "D".repeat(LARGE_PAYLOAD_BYTES);
    let submit_client = client.clone();
    tokio::task::spawn_blocking(move || {
        submit_client.submit(
            Log::new(Level::INFO, payload),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("join large DA payload submission")??;
    tokio::time::timeout(
        COMMIT_WAIT_BUDGET,
        network.ensure_blocks_with(|height| height.total >= expected_height),
    )
    .await
    .wrap_err("large DA payload did not commit within the budget")??;
    let required = network.peers().len().saturating_sub(1).max(1);
    let mut committed_subjects = Vec::new();
    for peer in network.peers() {
        let status_client = peer.client();
        let status = tokio::task::spawn_blocking(move || fetch_v2_status(status_client))
            .await
            .wrap_err("join canonical v2 status request")??;
        if validate_committed_da_status(&status, expected_height).is_ok() {
            committed_subjects.push(status.last_committed_subject);
        }
    }
    ensure!(
        committed_subjects.len() >= required,
        "expected canonical DA commit evidence on {required} peers, observed {}",
        committed_subjects.len()
    );
    let expected_subject = committed_subjects[0];
    ensure!(
        committed_subjects
            .iter()
            .all(|subject| *subject == expected_subject),
        "quorum peers must agree on the committed DA subject: {committed_subjects:?}"
    );
    network.shutdown().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_da_payload_commits_with_consistent_v2_subject_four_peers() -> Result<()> {
    large_da_payload_commits_with_consistent_v2_subject_for_committee(
        4,
        stringify!(large_da_payload_commits_with_consistent_v2_subject_four_peers),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_da_payload_commits_with_consistent_v2_subject_seven_peers() -> Result<()> {
    large_da_payload_commits_with_consistent_v2_subject_for_committee(
        7,
        stringify!(large_da_payload_commits_with_consistent_v2_subject_seven_peers),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authenticated_payload_chunk_hold_heals_and_converges_four_peers() -> Result<()> {
    init_instruction_registry();
    let builder = large_da_network_builder(4, PACKET_LOSS_PAYLOAD_BYTES)
        .with_base_seed(stringify!(
            authenticated_payload_chunk_hold_heals_and_converges_four_peers
        ))
        .with_block_cadence(Duration::from_secs(2))
        .with_consensus_message_control();
    let scenario = stringify!(authenticated_payload_chunk_hold_heals_and_converges_four_peers);
    let Some(network) = sandbox::start_network_async_or_skip(builder, scenario).await? else {
        return Ok(());
    };
    let result = async {
        ensure!(network.peers().len() == 4, "DA loss test requires four peers");
        network.ensure_blocks_with(|height| height.total >= 1).await?;
        let peers = network.peers().to_vec();
        let peer_ids = peers.iter().map(|peer| peer.id()).collect::<Vec<_>>();
        let expected_initial_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| hold_view_zero_payload_chunks(receiver_index, &peer_ids))
            .collect::<Vec<_>>();
        try_join_all(peers.iter().zip(&expected_initial_rules).map(
            |(peer, expected_rules)| async move {
                let control = peer
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?;
                let initial = control
                    .wait_until_ready(PACKET_LOSS_CONTROL_TIMEOUT)
                    .await?;
                ensure!(
                    initial.revision == 1
                        && initial.rules.is_empty()
                        && !initial.draining
                        && !initial.fatal
                        && initial.dropped == 0
                        && initial.overflowed == 0,
                    "{} did not acknowledge its empty genesis-safe controller revision",
                    peer.mnemonic()
                );
                let ack = control
                    .apply(
                        expected_rules,
                        &[],
                        PACKET_LOSS_QUEUE_CAPACITY,
                        PACKET_LOSS_CONTROL_TIMEOUT,
                    )
                    .await?;
                ensure!(
                    ack.revision == 2
                        && ack.rules.as_slice() == expected_rules.as_slice()
                        && ack.queue_capacity == PACKET_LOSS_QUEUE_CAPACITY
                        && !ack.draining
                        && !ack.fatal
                        && ack.dropped == 0
                        && ack.overflowed == 0,
                    "{} did not acknowledge its authenticated Proposal-bound chunk Hold rules",
                    peer.mnemonic()
                );
                Ok::<(), eyre::Report>(())
            },
        ))
        .await?;

        let client = network.client();
        let expected_height = client.get_status()?.blocks.saturating_add(1);
        ensure!(
            expected_height == PACKET_LOSS_HEIGHT,
            "packet-loss rules target height {PACKET_LOSS_HEIGHT}, but the network opened {expected_height}"
        );
        let submit_client = client.clone();
        let submission = tokio::task::spawn_blocking(move || {
            submit_client.submit(
                Log::new(Level::INFO, "P".repeat(PACKET_LOSS_PAYLOAD_BYTES)),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
        });
        submission.await.wrap_err("join packet-loss payload submission")??;

        let match_deadline = Instant::now() + PACKET_LOSS_CONTROL_TIMEOUT;
        let matched_sequences = loop {
            let mut matched_sequences = vec![Vec::new(); peers.len()];
            for (receiver_index, peer) in peers.iter().enumerate() {
                let observation = peer
                    .consensus_message_control()
                    .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                    .read_observation()?;
                ensure!(
                    observation.ack.revision == 2
                        && observation.ack.queue_capacity == PACKET_LOSS_QUEUE_CAPACITY
                        && !observation.ack.draining
                        && !observation.ack.fatal
                        && observation.ack.dropped == 0
                        && observation.ack.overflowed == 0,
                    "{} drifted from its acknowledged deferred chunk selector command",
                    peer.mnemonic()
                );
                for held in &observation.ack.held {
                    if held.kind != ConsensusMessageControlKind::PayloadChunk
                        || held.height.is_some()
                        || held.view.is_some()
                        || held.block_hash.is_some()
                        || held.subject.is_some()
                        || held.execution_commitment.is_some()
                        || held.sender != held.authenticated_via
                    {
                        continue;
                    }
                    let Some((manifest_hash, Some(index))) =
                        observation.payload_coordinates(held.sequence)
                    else {
                        continue;
                    };
                    let exact_rule = observation
                        .ack
                        .rules
                        .iter()
                        .find(|rule| {
                            rule.kind == ConsensusMessageControlKind::PayloadChunk
                                && rule.sender == held.sender
                                && rule.authenticated_via == held.authenticated_via
                                && rule.height == 0
                                && rule.view == 0
                                && rule.block_hash.is_none()
                                && rule.manifest_hash == Some(manifest_hash)
                                && rule.chunk_index == Some(index)
                                && rule.proposal_height == Some(PACKET_LOSS_HEIGHT)
                                && rule.proposal_view == Some(PACKET_LOSS_VIEW)
                                && rule.action == ConsensusMessageControlAction::Hold
                        })
                        .ok_or_else(|| {
                            eyre!(
                                "held chunk {index} lacks its resolved exact authenticated manifest rule"
                            )
                        })?;
                    ensure!(
                        exact_rule.manifest_hash == Some(manifest_hash)
                            && PACKET_LOSS_CHUNK_INDICES.contains(&index),
                        "retained occurrence disagreed with its exact manifest/index selector"
                    );
                    if !matched_sequences[receiver_index]
                        .iter()
                        .any(|(sequence, _)| *sequence == held.sequence)
                    {
                        matched_sequences[receiver_index].push((held.sequence, index));
                    }
                }
            }
            if matched_sequences
                .iter()
                .filter(|matches| {
                    PACKET_LOSS_CHUNK_INDICES
                        .iter()
                        .all(|index| matches.iter().any(|(_, matched_index)| matched_index == index))
                })
                .count()
                >= 3
            {
                break matched_sequences;
            }
            ensure!(
                Instant::now() < match_deadline,
                "fewer than three receivers retained the exact authenticated RS16 chunk loss set"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        for peer in &peers {
            ensure!(
                peer.client().get_status()?.blocks < expected_height,
                "{} committed before the three-of-six RS16 loss was healed",
                peer.mnemonic()
            );
        }

        let healed = try_join_all(peers.iter().map(|peer| async move {
            peer.consensus_message_control()
                .ok_or_else(|| eyre!("{} lacks message control", peer.mnemonic()))?
                .heal_and_release_all(PACKET_LOSS_CONTROL_TIMEOUT)
                .await
        }))
        .await?;
        for ((peer, ack), matched) in peers.iter().zip(&healed).zip(&matched_sequences) {
            ensure!(
                !ack.draining
                    && ack.drain_fence == Some(ack.revision)
                    && ack.rules.is_empty()
                    && ack.held.is_empty()
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.overflowed == 0,
                "{} did not acknowledge a complete healing drain fence",
                peer.mnemonic()
            );
            ensure!(
                matched
                    .iter()
                    .all(|(sequence, _)| ack.delivered.contains(sequence)),
                "{} did not deliver every exactly matched chunk during healing: matched={matched:?}, delivered={:?}, retired={:?}",
                peer.mnemonic(),
                ack.delivered,
                ack.retired
            );
        }

        tokio::time::timeout(
            COMMIT_WAIT_BUDGET,
            network.ensure_blocks_with(|height| height.total >= expected_height),
        )
        .await
        .wrap_err("healed four-peer DA payload did not commit")??;
        let mut committed_subjects = Vec::with_capacity(peers.len());
        for peer in &peers {
            let status_client = peer.client();
            let status = tokio::task::spawn_blocking(move || fetch_v2_status(status_client))
                .await
                .wrap_err("join healed v2 status request")??;
            validate_committed_da_status(&status, expected_height)?;
            committed_subjects.push(status.last_committed_subject);
        }
        ensure!(
            committed_subjects
                .iter()
                .all(|subject| subject.is_some() && *subject == committed_subjects[0]),
            "all four peers must converge on one committed subject after chunk healing: observed={committed_subjects:?}"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
#[test]
fn large_payload_limits_cover_transport_overhead() {
    let content_limit = torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES);
    assert!(content_limit > i64::try_from(LARGE_PAYLOAD_BYTES).expect("fixture fits i64"));
    assert_eq!(
        tx_limit_for_payload(LARGE_PAYLOAD_BYTES).get(),
        u64::try_from(content_limit).expect("positive content limit")
    );
}
