#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for Sumeragi VRF randomness edge cases.
//!
//! These scenarios exercise late reveals and epochs without participation to
//! ensure telemetry exposes penalty clearing behaviour and seed continuity.

use std::{
    collections::HashSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_core::sumeragi::consensus::{NPOS_TAG, vrf_commit_preimage, vrf_reveal_preimage};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    ChainId, Level,
    block::consensus::{VrfCommit, VrfReveal},
    isi::{Log, SetParameter},
    parameter::{
        Parameter,
        system::{SumeragiNposParameters, SumeragiParameter},
    },
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::{self, Value};
use rand::Rng as _;
use reqwest::Client as HttpClient;
use sha2::{Digest as _, Sha256};
use tokio::time::sleep;

const EPOCH_LENGTH_BLOCKS: u64 = 10;
const VRF_COMMIT_WINDOW_BLOCKS: u64 = 4;
const VRF_REVEAL_WINDOW_BLOCKS: u64 = 0;
const VRF_LATE_REVEAL_SAFETY_BLOCKS: u64 = 1;
const BLOCK_TIME_MS: u64 = 600;
const VRF_INPUT_DOMAIN: &[u8] = b"iroha:npos:vrf:input:v1";
const TELEMETRY_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const TELEMETRY_RETRY_ATTEMPTS: usize = 30;
const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";

/// Late VRF reveal should clear penalties and leave the epoch seed unchanged.
#[allow(clippy::too_many_lines)] // Complex scenario requires sequential orchestration.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_late_vrf_reveal_clears_penalty_and_preserves_seed() -> Result<()> {
    init_instruction_registry();

    let builder = randomness_network_builder();
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_late_vrf_reveal_clears_penalty_and_preserves_seed),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let (epoch, auto_snapshot) = wait_for_epoch_commitment_snapshot(&client).await?;

    let http = HttpClient::new();
    let telemetry_url = client
        .torii_url
        .join("v1/sumeragi/telemetry")
        .wrap_err("compose telemetry URL")?;

    let chain_id = network.chain_id();
    let (target_signer, signer_key_pair, reveal, commitment) =
        find_recorded_vrf_material(network.peers(), &chain_id, epoch, &auto_snapshot)?;
    let status = client.get_sumeragi_status()?;
    let mode_tag = if status.mode_tag.is_empty() {
        NPOS_TAG
    } else {
        status.mode_tag.as_str()
    };
    let commit_sig_hex = vrf_commit_signature_hex(
        &chain_id,
        &signer_key_pair,
        epoch,
        target_signer,
        commitment,
        mode_tag,
    );
    let reveal_sig_hex = vrf_reveal_signature_hex(
        &chain_id,
        &signer_key_pair,
        epoch,
        target_signer,
        reveal,
        mode_tag,
    );

    submit_vrf_commit(
        &client,
        &http,
        epoch,
        target_signer,
        commitment,
        &commit_sig_hex,
    )
    .await?;
    submit_progress_log(&client, "vrf commit flush")?;

    let commitment_hex = hex::encode(commitment);
    // Wait until the submitted commitment is visible for the target signer.
    let snapshot_before = wait_for_epoch_record(&client, epoch, |json| {
        json.get("participants")
            .and_then(Value::as_array)
            .is_some_and(|participants| {
                participants.iter().any(|participant| {
                    participant.get("signer").and_then(Value::as_u64)
                        == Some(u64::from(target_signer))
                        && participant.get("commitment").and_then(Value::as_str)
                            == Some(commitment_hex.as_str())
                        && participant.get("reveal").is_none()
                })
            })
    })
    .await?;
    let status_before = wait_for_sumeragi_status(&client, |json| {
        let prf = json.get("prf")?.as_object()?;
        let seed = prf.get("epoch_seed")?.as_str()?;
        Some(!seed.is_empty())
    })
    .await?;
    let prf_seed_before = status_before
        .get("prf")
        .and_then(Value::as_object)
        .and_then(|prf| prf.get("epoch_seed"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let seed_before = snapshot_before
        .get("seed_hex")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();

    // Advance height well outside the reveal window.
    // `vrf_reveal_window_blocks` is relative to the commit window, so the
    // inclusive reveal deadline is `commit + reveal`.
    // Keep a safety margin because consensus message handling can lag behind
    // externally reported block height by a couple of blocks under load.
    let reveal_cutoff_height = epoch
        .saturating_mul(EPOCH_LENGTH_BLOCKS)
        .saturating_add(VRF_COMMIT_WINDOW_BLOCKS)
        .saturating_add(VRF_REVEAL_WINDOW_BLOCKS)
        .saturating_add(VRF_LATE_REVEAL_SAFETY_BLOCKS);
    let epoch_end_height = epoch.saturating_add(1).saturating_mul(EPOCH_LENGTH_BLOCKS);
    wait_for_height_total_at_least_before(&client, reveal_cutoff_height, epoch_end_height).await?;

    let snapshot_after = submit_late_reveal_until_recorded(
        &client,
        &http,
        epoch,
        target_signer,
        reveal,
        &reveal_sig_hex,
    )
    .await?;

    let seed_after = snapshot_after
        .get("seed_hex")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    ensure!(
        seed_before == seed_after,
        "late reveal must not mutate epoch seed (before={seed_before}, after={seed_after})"
    );

    let late_reveals = snapshot_after
        .get("late_reveals")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let late_signers: HashSet<u32> = late_reveals
        .iter()
        .filter_map(|entry| entry.get("signer").and_then(Value::as_u64))
        .map(|val| {
            u32::try_from(val).expect("validator identifiers must fit into u32 for the test setup")
        })
        .collect();
    ensure!(
        late_signers.contains(&target_signer),
        "late reveal snapshot must list signer {target_signer}"
    );

    let status_after_late = wait_for_sumeragi_status(&client, |json| {
        let prf = json.get("prf")?.as_object()?;
        let seed = prf.get("epoch_seed")?.as_str()?;
        if seed != prf_seed_before {
            return Some(false);
        }
        json.get("vrf_late_reveals_total")
            .and_then(Value::as_u64)
            .map(|late| late >= 1)
    })
    .await?;
    ensure!(
        status_after_late
            .get("vrf_late_reveals_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 1,
        "status endpoint should reflect late reveal acceptance"
    );
    ensure!(
        status_after_late
            .get("prf")
            .and_then(Value::as_object)
            .and_then(|prf| prf.get("epoch_seed"))
            .and_then(Value::as_str)
            .is_some_and(|seed| seed == prf_seed_before),
        "late reveal must not change PRF seed exposed via status"
    );

    // Telemetry follows the active epoch summary, so verify the late reveal
    // before epoch rollover switches the active epoch record.
    let telemetry = wait_for_telemetry(&http, &telemetry_url, |json| {
        let vrf = json.get("vrf").and_then(Value::as_object)?;
        let epoch_reported = vrf.get("epoch").and_then(Value::as_u64)?;
        let late = vrf.get("late_reveals_total").and_then(Value::as_u64)?;
        let committed_empty = vrf
            .get("committed_no_reveal")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);
        Some(epoch_reported == epoch && late >= 1 && committed_empty)
    })
    .await?;
    let vrf = telemetry
        .get("vrf")
        .and_then(Value::as_object)
        .expect("telemetry vrf summary");
    ensure!(
        vrf.get("late_reveals_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 1,
        "telemetry should record a late reveal"
    );
    ensure!(
        vrf.get("committed_no_reveal")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "telemetry committed_no_reveal list should be empty"
    );

    // Wait for the epoch to finalize (height multiple of epoch length).
    let finalize_height = epoch.saturating_add(1).saturating_mul(EPOCH_LENGTH_BLOCKS);
    let status = client.get_status()?;
    for idx in status.blocks..finalize_height {
        submit_progress_log(&client, format!("vrf finalize tick {idx}"))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= finalize_height)
        .await?;

    let penalties = wait_for_penalties(&client, epoch, |json| {
        json.get("committed_no_reveal")
            .and_then(Value::as_array)
            .is_some_and(|committed| {
                !committed
                    .iter()
                    .filter_map(Value::as_u64)
                    .any(|signer| signer == u64::from(target_signer))
            })
    })
    .await?;
    let committed: Vec<u32> = penalties
        .get("committed_no_reveal")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|val| {
            u32::try_from(val).expect("validator identifiers must fit into u32 for the test setup")
        })
        .collect();
    ensure!(
        !committed.contains(&target_signer),
        "committed_no_reveal should not include late reveal signer {target_signer}, got {committed:?}"
    );

    let status_final = wait_for_sumeragi_status(&client, |json| {
        let epoch_reported = json.get("vrf_penalty_epoch")?.as_u64()?;
        let committed = json.get("vrf_committed_no_reveal_total")?.as_u64()?;
        let late = json.get("vrf_late_reveals_total")?.as_u64()?;
        Some(
            epoch_reported == epoch
                && late >= 1
                && committed <= network.peers().len().saturating_sub(1) as u64,
        )
    })
    .await?;
    ensure!(
        status_final
            .get("vrf_late_reveals_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 1,
        "status should retain late reveal count after epoch finalization"
    );

    network.shutdown().await;
    Ok(())
}

/// Epochs without participation should register no-participation penalties only.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_zero_participation_epoch_reports_full_no_participation() -> Result<()> {
    init_instruction_registry();

    let builder = zero_participation_network_builder();
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_zero_participation_epoch_reports_full_no_participation),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let epoch = wait_for_epoch_position(&client, 1).await?;
    let target_height = epoch
        .saturating_add(1)
        .saturating_mul(EPOCH_LENGTH_BLOCKS)
        .saturating_add(1);
    let status = client.get_status()?;
    for idx in status.blocks..target_height {
        submit_progress_log(&client, format!("vrf no-participation tick {idx}"))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= target_height)
        .await?;

    let http = HttpClient::new();
    let telemetry_url = client
        .torii_url
        .join("v1/sumeragi/telemetry")
        .wrap_err("compose telemetry URL")?;

    let penalties = wait_for_penalties(&client, epoch, |json| {
        json.get("no_participation")
            .and_then(Value::as_array)
            .is_some_and(|array| array.len() == 4)
    })
    .await?;

    let committed: Vec<u32> = penalties
        .get("committed_no_reveal")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|val| {
            u32::try_from(val).expect("validator identifiers must fit into u32 for the test setup")
        })
        .collect();
    ensure!(
        committed.is_empty(),
        "no commits were emitted, committed_no_reveal should be empty, got {committed:?}"
    );

    let no_participation: HashSet<u32> = penalties
        .get("no_participation")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|val| {
            u32::try_from(val).expect("validator identifiers must fit into u32 for the test setup")
        })
        .collect();
    ensure!(
        no_participation == HashSet::from([0_u32, 1, 2, 3]),
        "no participation should list every validator, got {no_participation:?}"
    );

    let status = wait_for_sumeragi_status(&client, |json| {
        let epoch_reported = json.get("vrf_penalty_epoch")?.as_u64()?;
        let committed_total = json.get("vrf_committed_no_reveal_total")?.as_u64()?;
        let no_participation_total = json.get("vrf_no_participation_total")?.as_u64()?;
        let late_reveals_total = json.get("vrf_late_reveals_total")?.as_u64()?;
        // The penalties endpoint above already locked the epoch-specific report.
        // Status only exposes the latest penalty snapshot, so later epochs may
        // overtake this poll while still preserving the same zero-participation
        // semantics in this scenario.
        Some(
            epoch_reported >= epoch
                && committed_total == 0
                && no_participation_total == 4
                && late_reveals_total == 0,
        )
    })
    .await?;
    ensure!(
        status
            .get("vrf_late_reveals_total")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            == 0,
        "status should report zero late reveals for no-participation epoch"
    );

    let _telemetry = wait_for_telemetry(&http, &telemetry_url, |json| {
        let vrf = json.get("vrf").and_then(Value::as_object)?;
        Some(
            vrf.get("epoch").and_then(Value::as_u64).is_some()
                && vrf
                    .get("no_participation")
                    .and_then(Value::as_array)
                    .is_some(),
        )
    })
    .await?;

    network.shutdown().await;
    Ok(())
}

fn randomness_network_builder() -> NetworkBuilder {
    randomness_network_builder_with_params(short_epoch_npos_parameters())
}

fn zero_participation_network_builder() -> NetworkBuilder {
    let mut params = short_epoch_npos_parameters();
    params.vrf_commit_window_blocks = 0;
    params.vrf_reveal_window_blocks = 0;
    randomness_network_builder_with_params(params)
}

fn randomness_network_builder_with_params(params: SumeragiNposParameters) -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 1_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64)
                .write(["sumeragi", "da", "enabled"], true)
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    1_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    1_000_i64,
                );
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(1),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            params.into_custom_parameter(),
        )))
}

fn short_epoch_npos_parameters() -> SumeragiNposParameters {
    SumeragiNposParameters {
        vrf_commit_window_blocks: VRF_COMMIT_WINDOW_BLOCKS,
        vrf_reveal_window_blocks: VRF_REVEAL_WINDOW_BLOCKS,
        epoch_length_blocks: EPOCH_LENGTH_BLOCKS,
        ..SumeragiNposParameters::default()
    }
}

fn commitment_from_reveal(reveal: &[u8; 32]) -> [u8; 32] {
    iroha_crypto::Hash::new(reveal).into()
}

fn find_recorded_vrf_material(
    peers: &[iroha_test_network::NetworkPeer],
    chain_id: &ChainId,
    epoch: u64,
    snapshot: &Value,
) -> Result<(u32, KeyPair, [u8; 32], [u8; 32])> {
    let participants = snapshot
        .get("participants")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("VRF epoch snapshot is missing participants"))?;
    for participant in participants {
        let Some(commitment_hex) = participant.get("commitment").and_then(Value::as_str) else {
            continue;
        };
        let signer = participant
            .get("signer")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("VRF participant is missing signer"))?;
        let signer = u32::try_from(signer).wrap_err("VRF signer index exceeds u32")?;
        for peer in peers {
            let Some(key_pair) = peer.bls_key_pair() else {
                continue;
            };
            let (reveal, commitment) = derive_vrf_material(chain_id, key_pair, epoch, signer);
            if hex::encode(commitment) == commitment_hex {
                return Ok((signer, key_pair.clone(), reveal, commitment));
            }
        }
    }
    eyre::bail!("no local BLS key matched recorded VRF commitments for epoch {epoch}")
}

fn derive_vrf_material(
    chain_id: &ChainId,
    signer_key_pair: &KeyPair,
    epoch: u64,
    signer: u32,
) -> ([u8; 32], [u8; 32]) {
    let chain_hash = iroha_crypto::Hash::new(chain_id.clone().into_inner().as_bytes());
    let mut message = Vec::with_capacity(
        VRF_INPUT_DOMAIN.len() + chain_hash.as_ref().len() + core::mem::size_of::<u64>() * 2,
    );
    message.extend_from_slice(VRF_INPUT_DOMAIN);
    message.extend_from_slice(chain_hash.as_ref());
    message.extend_from_slice(&epoch.to_be_bytes());
    message.extend_from_slice(&u64::from(signer).to_be_bytes());
    let signature = Signature::new(signer_key_pair.private_key(), &message);
    let reveal: [u8; 32] = iroha_crypto::Hash::new(signature.payload()).into();
    let commitment = commitment_from_reveal(&reveal);
    (reveal, commitment)
}

fn vrf_commit_signature_hex(
    chain_id: &ChainId,
    signer_key_pair: &KeyPair,
    epoch: u64,
    signer: u32,
    commitment: [u8; 32],
    mode_tag: &str,
) -> String {
    let commit = VrfCommit {
        epoch,
        signer,
        commitment,
        bls_sig: Vec::new(),
    };
    let preimage = vrf_commit_preimage(chain_id, mode_tag, &commit);
    let signature = Signature::new(signer_key_pair.private_key(), &preimage);
    hex::encode(signature.payload())
}

fn vrf_reveal_signature_hex(
    chain_id: &ChainId,
    signer_key_pair: &KeyPair,
    epoch: u64,
    signer: u32,
    reveal: [u8; 32],
    mode_tag: &str,
) -> String {
    let reveal = VrfReveal {
        epoch,
        signer,
        reveal,
        bls_sig: Vec::new(),
    };
    let preimage = vrf_reveal_preimage(chain_id, mode_tag, &reveal);
    let signature = Signature::new(signer_key_pair.private_key(), &preimage);
    hex::encode(signature.payload())
}

async fn submit_vrf_commit(
    client: &Client,
    http: &HttpClient,
    epoch: u64,
    signer: u32,
    commitment: [u8; 32],
    bls_sig_hex: &str,
) -> Result<()> {
    let url = client
        .torii_url
        .join("v1/sumeragi/vrf/commit")
        .wrap_err("compose VRF commit URL")?;
    let body = format!(
        "{{\"epoch\":{epoch},\"signer\":{signer},\"commitment_hex\":\"{}\",\"bls_sig_hex\":\"{bls_sig_hex}\"}}",
        hex::encode(commitment),
    )
    .into_bytes();
    let mut request = http
        .post(url.clone())
        .header("content-type", "application/json")
        .body(body.clone());
    for (name, value) in operator_signature_headers(client, "POST", url.path(), &body) {
        request = request.header(name, value);
    }
    if let Some(auth) = client.headers.get("Authorization") {
        request = request.header("Authorization", auth);
    }
    let response = request.send().await.wrap_err("submit VRF commit")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        eyre::bail!("VRF commit submission failed: {status}: {body}");
    }
    Ok(())
}

async fn submit_vrf_reveal(
    client: &Client,
    http: &HttpClient,
    epoch: u64,
    signer: u32,
    reveal: [u8; 32],
    bls_sig_hex: &str,
) -> Result<()> {
    let url = client
        .torii_url
        .join("v1/sumeragi/vrf/reveal")
        .wrap_err("compose VRF reveal URL")?;
    let body = format!(
        "{{\"epoch\":{epoch},\"signer\":{signer},\"reveal_hex\":\"{}\",\"bls_sig_hex\":\"{bls_sig_hex}\"}}",
        hex::encode(reveal),
    )
    .into_bytes();
    let mut request = http
        .post(url.clone())
        .header("content-type", "application/json")
        .body(body.clone());
    for (name, value) in operator_signature_headers(client, "POST", url.path(), &body) {
        request = request.header(name, value);
    }
    if let Some(auth) = client.headers.get("Authorization") {
        request = request.header("Authorization", auth);
    }
    let response = request.send().await.wrap_err("submit VRF reveal")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        eyre::bail!("VRF reveal submission failed: {status}: {body}");
    }
    Ok(())
}

async fn submit_late_reveal_until_recorded(
    client: &Client,
    http: &HttpClient,
    epoch: u64,
    signer: u32,
    reveal: [u8; 32],
    bls_sig_hex: &str,
) -> Result<Value> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const PROCESSING_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const PROCESSING_POLLS: usize = 6;
    const RETRIES: usize = 300;
    const SEAL_GRACE_BLOCKS: u64 = 3;

    let mut last_snapshot = None;
    let mut epoch_finalized = false;
    let epoch_end_height = epoch.saturating_add(1).saturating_mul(EPOCH_LENGTH_BLOCKS);
    let seal_deadline_height = epoch_end_height.saturating_add(SEAL_GRACE_BLOCKS);
    let mut last_progress_height = None;
    let mut accepted_in_status = false;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        if !accepted_in_status && status.blocks < epoch_end_height {
            submit_vrf_reveal(client, http, epoch, signer, reveal, bls_sig_hex).await?;
        }

        // First poll the snapshot without forcing progress. Committing a block
        // immediately after every submit can race straight into finalization.
        for _ in 0..PROCESSING_POLLS {
            let snapshot = client.get_sumeragi_vrf_epoch_json(epoch)?;
            if snapshot
                .get("late_reveals_total")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                >= 1
            {
                return Ok(snapshot);
            }

            epoch_finalized = snapshot
                .get("finalized")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            last_snapshot = Some(snapshot);
            accepted_in_status |= client
                .get_sumeragi_status_json()
                .ok()
                .and_then(|json| {
                    json.get("vrf_late_reveals_total")
                        .and_then(Value::as_u64)
                        .map(|late| late >= 1)
                })
                .unwrap_or(false);
            if epoch_finalized && !accepted_in_status {
                break;
            }
            sleep(PROCESSING_POLL_INTERVAL).await;
        }

        let status = client.get_status()?;
        if !accepted_in_status && status.blocks >= epoch_end_height {
            break;
        }
        if epoch_finalized && !accepted_in_status {
            break;
        }
        if accepted_in_status && status.blocks > seal_deadline_height {
            break;
        }
        submit_progress_log_if_stalled(
            client,
            status.blocks,
            "vrf late-reveal progress tick",
            attempt,
            &mut last_progress_height,
        )?;
        sleep(RETRY_INTERVAL).await;
    }

    let last_payload = last_snapshot.as_ref().map_or_else(String::new, |value| {
        json::to_string_pretty(value).unwrap_or_default()
    });
    let final_blocks = client
        .get_status()
        .map(|status| status.blocks.to_string())
        .unwrap_or_else(|err| format!("unavailable: {err}"));
    let status_payload = sumeragi_status_debug_summary(client);
    eyre::bail!(
        "late reveal was not recorded for epoch {epoch}; signer={signer}; final_blocks={final_blocks}; last_payload={last_payload}; sumeragi_status={status_payload}"
    )
}

fn operator_signature_headers(
    client: &Client,
    method: &str,
    path: &str,
    body: &[u8],
) -> Vec<(&'static str, String)> {
    let Some(operator_key_pair) = client.operator_key_pair.as_ref() else {
        return Vec::new();
    };

    let timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let nonce_bytes: [u8; 12] = rand::rng().random();
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);

    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash_hex = hex::encode(hasher.finalize());
    let message = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path,
        body_hash_hex,
        timestamp_ms,
        nonce
    )
    .into_bytes();
    let signature = iroha_crypto::Signature::new(operator_key_pair.private_key(), &message);
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.payload());

    vec![
        (
            HEADER_OPERATOR_PUBLIC_KEY,
            operator_key_pair.public_key().to_string(),
        ),
        (HEADER_OPERATOR_TIMESTAMP_MS, timestamp_ms.to_string()),
        (HEADER_OPERATOR_NONCE, nonce),
        (HEADER_OPERATOR_SIGNATURE, signature_b64),
    ]
}

async fn wait_for_epoch_record<F>(client: &Client, epoch: u64, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> bool,
{
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 150;
    let mut last = None;
    for attempt in 0..RETRIES {
        let value = client.get_sumeragi_vrf_epoch_json(epoch)?;
        last = Some(value.clone());
        if value.get("found").and_then(Value::as_bool).unwrap_or(false) && predicate(&value) {
            return Ok(value);
        }
        if attempt + 1 == RETRIES {
            break;
        }
        sleep(RETRY_INTERVAL).await;
    }
    let last_payload = last.as_ref().map_or_else(String::new, |value| {
        json::to_string_pretty(value).unwrap_or_default()
    });
    eyre::bail!("VRF epoch record not available for epoch {epoch}; last_payload={last_payload}")
}

async fn wait_for_penalties<F>(client: &Client, epoch: u64, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> bool,
{
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 150;
    let mut last = None;
    for attempt in 0..RETRIES {
        let value = client.get_sumeragi_vrf_penalties_json(epoch)?;
        last = Some(value.clone());
        if predicate(&value) {
            return Ok(value);
        }
        if attempt + 1 == RETRIES {
            break;
        }
        sleep(RETRY_INTERVAL).await;
    }
    let last_payload = last.as_ref().map_or_else(String::new, |value| {
        json::to_string_pretty(value).unwrap_or_default()
    });
    eyre::bail!(
        "VRF penalties snapshot not available for epoch {epoch}; last_payload={last_payload}"
    )
}

async fn wait_for_sumeragi_status<F>(client: &Client, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> Option<bool>,
{
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 30;
    for attempt in 0..RETRIES {
        let value = client.get_sumeragi_status_json()?;
        if predicate(&value).unwrap_or(false) {
            return Ok(value);
        }
        if attempt + 1 == RETRIES {
            break;
        }
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("sumeragi status endpoint did not report expected snapshot")
}

async fn wait_for_telemetry<F>(http: &HttpClient, url: &reqwest::Url, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> Option<bool>,
{
    for attempt in 0..TELEMETRY_RETRY_ATTEMPTS {
        let response = http
            .get(url.clone())
            .header("accept", "application/json")
            .send()
            .await
            .wrap_err("fetch telemetry payload")?;
        ensure!(
            response.status().is_success(),
            "telemetry endpoint returned {}",
            response.status()
        );
        let body = response.text().await.wrap_err("telemetry body")?;
        let value: Value = json::from_str(&body)?;
        if predicate(&value).unwrap_or(false) {
            return Ok(value);
        }
        if attempt + 1 == TELEMETRY_RETRY_ATTEMPTS {
            break;
        }
        sleep(TELEMETRY_RETRY_INTERVAL).await;
    }
    eyre::bail!("telemetry endpoint did not report expected counters")
}

fn epoch_and_position_from_height(height: u64) -> (u64, u64) {
    let normalized_height = height.max(1);
    let epoch = (normalized_height - 1) / EPOCH_LENGTH_BLOCKS;
    let position = ((normalized_height - 1) % EPOCH_LENGTH_BLOCKS) + 1;
    (epoch, position)
}

fn submit_progress_log(client: &Client, message: impl Into<String>) -> Result<()> {
    client.submit(Log::new(Level::INFO, message.into()))?;
    Ok(())
}

fn submit_progress_log_if_stalled(
    client: &Client,
    current_height: u64,
    label: &str,
    attempt: usize,
    last_progress_height: &mut Option<u64>,
) -> Result<()> {
    if *last_progress_height != Some(current_height) {
        *last_progress_height = Some(current_height);
        submit_progress_log(client, format!("{label} {attempt}"))?;
    }
    Ok(())
}

async fn wait_for_epoch_position(client: &Client, desired_position: u64) -> Result<u64> {
    ensure!(
        (1..=EPOCH_LENGTH_BLOCKS).contains(&desired_position),
        "desired epoch position {desired_position} out of range 1..={EPOCH_LENGTH_BLOCKS}"
    );
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 60;
    let mut last_progress_height = None;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        let (epoch, position) = epoch_and_position_from_height(status.blocks);
        if position == desired_position {
            return Ok(epoch);
        }
        submit_progress_log_if_stalled(
            client,
            status.blocks,
            "vrf align epoch-position tick",
            attempt,
            &mut last_progress_height,
        )?;
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("failed to align to epoch position {desired_position}")
}

async fn wait_for_epoch_commitment_snapshot(client: &Client) -> Result<(u64, Value)> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 300;
    let mut last_snapshot = None;
    let mut last_progress_height = None;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        let (epoch, _) = epoch_and_position_from_height(status.blocks);
        let snapshot = client.get_sumeragi_vrf_epoch_json(epoch)?;
        let has_commitment = snapshot
            .get("found")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && snapshot
                .get("participants")
                .and_then(Value::as_array)
                .is_some_and(|participants| {
                    participants.iter().any(|participant| {
                        participant
                            .get("commitment")
                            .and_then(Value::as_str)
                            .is_some()
                    })
                });
        if has_commitment {
            return Ok((epoch, snapshot));
        }
        last_snapshot = Some(snapshot);
        submit_progress_log_if_stalled(
            client,
            status.blocks,
            "vrf commitment wait tick",
            attempt,
            &mut last_progress_height,
        )?;
        sleep(RETRY_INTERVAL).await;
    }
    let last_payload = last_snapshot.as_ref().map_or_else(String::new, |value| {
        json::to_string_pretty(value).unwrap_or_default()
    });
    eyre::bail!("failed to observe VRF commitment snapshot; last_payload={last_payload}")
}

async fn wait_for_height_total_at_least_before(
    client: &Client,
    min_height: u64,
    max_height_exclusive: u64,
) -> Result<()> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 300;
    let mut last_progress_height = None;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        ensure!(
            status.blocks < max_height_exclusive,
            "advanced to block height {} before late reveal could be submitted; epoch end is {max_height_exclusive}",
            status.blocks
        );
        if status.blocks >= min_height {
            return Ok(());
        }
        submit_progress_log_if_stalled(
            client,
            status.blocks,
            "vrf advance height tick",
            attempt,
            &mut last_progress_height,
        )?;
        sleep(RETRY_INTERVAL).await;
    }
    let final_blocks = client
        .get_status()
        .map(|status| status.blocks.to_string())
        .unwrap_or_else(|err| format!("unavailable: {err}"));
    let status_payload = sumeragi_status_debug_summary(client);
    eyre::bail!(
        "failed to reach block height {min_height}; final_blocks={final_blocks}; sumeragi_status={status_payload}"
    )
}

fn sumeragi_status_debug_summary(client: &Client) -> String {
    let Ok(value) = client.get_sumeragi_status_json() else {
        return String::new();
    };
    let keys = [
        "mode_tag",
        "prf",
        "vrf_late_reveals_total",
        "commit_qc",
        "highest_qc",
        "locked_qc",
        "tx_queue",
        "view_change_causes",
        "worker_loop",
        "pending_rbc",
    ];
    let mut entries = Vec::new();
    for key in keys {
        if let Some(entry) = value.get(key) {
            let encoded = json::to_string(entry).unwrap_or_default();
            entries.push(format!("\"{key}\":{encoded}"));
        }
    }
    format!("{{{}}}", entries.join(","))
}

#[test]
fn epoch_and_position_mapping_handles_genesis_and_boundaries() {
    assert_eq!(epoch_and_position_from_height(0), (0, 1));
    assert_eq!(epoch_and_position_from_height(1), (0, 1));
    assert_eq!(
        epoch_and_position_from_height(EPOCH_LENGTH_BLOCKS),
        (0, EPOCH_LENGTH_BLOCKS)
    );
    assert_eq!(
        epoch_and_position_from_height(EPOCH_LENGTH_BLOCKS + 1),
        (1, 1)
    );
}

#[test]
fn commitment_from_reveal_matches_runtime_hashing() {
    let reveal = [0xAB; 32];
    let expected: [u8; 32] = iroha_crypto::Hash::new(reveal).into();
    assert_eq!(commitment_from_reveal(&reveal), expected);
}
