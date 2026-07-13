#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Canonical Sumeragi v2 data-availability integration coverage.
//!
//! The first-release v2 runtime has no global RBC status endpoint or persisted
//! global RBC session store. Availability is committed by the signed genesis
//! context and observed through the authoritative v2 status and committed
//! subject.

use std::{num::NonZeroU64, time::Duration};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        block::consensus_v2::SumeragiV2Status,
        isi::{Log, SetParameter},
        parameter::{Parameter, TransactionParameter},
    },
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};

const LARGE_PAYLOAD_BYTES: usize = 1024 * 1024;
const TORII_CONTENT_HEADROOM_BYTES: usize = 2 * 1024 * 1024;
const NETWORK_FRAME_BUDGET_BYTES: i64 = 128 * 1024 * 1024;
const COMMIT_WAIT_BUDGET: Duration = Duration::from_secs(480);

fn torii_max_content_len_for_payload(payload_bytes: usize) -> i64 {
    let inflated = payload_bytes.saturating_mul(12);
    let with_headroom = payload_bytes.saturating_add(TORII_CONTENT_HEADROOM_BYTES);
    i64::try_from(inflated.max(with_headroom)).unwrap_or(i64::MAX)
}

fn tx_limit_for_payload(payload_bytes: usize) -> NonZeroU64 {
    NonZeroU64::new(
        u64::try_from(torii_max_content_len_for_payload(payload_bytes)).unwrap_or(u64::MAX),
    )
    .expect("payload-driven transaction limit must be non-zero")
}

fn large_da_network_builder() -> NetworkBuilder {
    let tx_limit = tx_limit_for_payload(LARGE_PAYLOAD_BYTES);
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(
                    ["torii", "max_content_len"],
                    torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES),
                )
                .write(["network", "max_frame_bytes"], NETWORK_FRAME_BUDGET_BYTES)
                .write(
                    ["network", "max_frame_bytes_consensus"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_control"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_block_sync"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_other"],
                    NETWORK_FRAME_BUDGET_BYTES,
                )
                .write(
                    ["network", "max_frame_bytes_tx_gossip"],
                    NETWORK_FRAME_BUDGET_BYTES,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxTxBytes(tx_limit),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxDecompressedBytes(tx_limit),
        )))
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_da_payload_commits_with_consistent_v2_subject() -> Result<()> {
    init_instruction_registry();

    let Some(network) = sandbox::start_network_async_or_skip(
        large_da_network_builder(),
        stringify!(large_da_payload_commits_with_consistent_v2_subject),
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
    tokio::task::spawn_blocking(move || submit_client.submit(Log::new(Level::INFO, payload)))
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

#[test]
fn large_payload_limits_cover_transport_overhead() {
    let content_limit = torii_max_content_len_for_payload(LARGE_PAYLOAD_BYTES);
    assert!(content_limit > i64::try_from(LARGE_PAYLOAD_BYTES).expect("fixture fits i64"));
    assert_eq!(
        tx_limit_for_payload(LARGE_PAYLOAD_BYTES).get(),
        u64::try_from(content_limit).expect("positive content limit")
    );
}
