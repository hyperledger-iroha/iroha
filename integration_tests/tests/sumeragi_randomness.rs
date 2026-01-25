//! Integration tests for Sumeragi VRF randomness edge cases.
//!
//! These scenarios exercise late reveals and epochs without participation to
//! ensure telemetry exposes penalty clearing behaviour and seed continuity.

use std::{collections::HashSet, time::Duration};

use eyre::{Result, WrapErr, ensure};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_data_model::{
    Level,
    isi::{Log, SetParameter},
    parameter::{
        Parameter,
        system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
    },
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::{self, Value};
use rand::{Rng as _, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::Client as HttpClient;
use tokio::time::sleep;

const EPOCH_LENGTH_BLOCKS: u64 = 6;
const VRF_COMMIT_WINDOW_BLOCKS: u64 = 2;
const VRF_REVEAL_WINDOW_BLOCKS: u64 = 4;
const TELEMETRY_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const TELEMETRY_RETRY_ATTEMPTS: usize = 30;

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

    // Ensure the network produces a couple of blocks so the epoch manager is initialised.
    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..2 {
        client.submit_blocking(Log::new(Level::INFO, format!("vrf seed {idx}")))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= 2)
        .await?;

    let http = HttpClient::new();
    let telemetry_url = client
        .torii_url
        .join("v1/sumeragi/telemetry")
        .wrap_err("compose telemetry URL")?;

    let epoch = 0_u64;
    let target_signer = 1_u32;

    let reveal = random_bytes();
    let commitment = commitment_from_reveal(&reveal);

    submit_vrf_commit(&client, &http, epoch, target_signer, commitment).await?;

    // Wait until the epoch record reflects the commitment.
    let snapshot_before = wait_for_epoch_record(&client, epoch, |json| {
        json.get("participants_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 1
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

    // Advance height until we are outside the reveal window (position > VRF_REVEAL_WINDOW_BLOCKS).
    wait_for_height_at_least(&client, VRF_REVEAL_WINDOW_BLOCKS + 1).await?;

    submit_vrf_reveal(&client, &http, epoch, target_signer, reveal).await?;

    let snapshot_after = wait_for_epoch_record(&client, epoch, |json| {
        json.get("late_reveals_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 1
    })
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

    // Wait for the epoch to finalize (height multiple of epoch length).
    let status = client.get_status()?;
    for idx in status.blocks..EPOCH_LENGTH_BLOCKS {
        client.submit_blocking(Log::new(Level::INFO, format!("vrf finalize tick {idx}")))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= EPOCH_LENGTH_BLOCKS)
        .await?;

    let penalties = wait_for_penalties(&client, epoch, |json| {
        json.get("committed_no_reveal")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
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
        "committed_no_reveal should be empty after late reveal, got {committed:?}"
    );

    // Telemetry should reflect cleared penalties and late reveal counters.
    let telemetry = wait_for_telemetry(&http, &telemetry_url, |json| {
        let vrf = json.get("vrf").and_then(Value::as_object)?;
        let late = vrf.get("late_reveals_total").and_then(Value::as_u64)?;
        let committed_total = json
            .get("vrf_committed_no_reveal_total")
            .and_then(Value::as_u64)?;
        let epoch_reported = json.get("vrf_penalty_epoch").and_then(Value::as_u64)?;
        if epoch_reported != epoch {
            return Some(false);
        }
        Some(late >= 1 && committed_total == 0)
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
        telemetry
            .get("vrf_committed_no_reveal_total")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            == 0,
        "telemetry committed_no_reveal counter should be zero"
    );

    let status_final = wait_for_sumeragi_status(&client, |json| {
        let epoch_reported = json.get("vrf_penalty_epoch")?.as_u64()?;
        let committed = json.get("vrf_committed_no_reveal_total")?.as_u64()?;
        Some(epoch_reported == epoch && committed == 0)
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

    let builder = randomness_network_builder();
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_zero_participation_epoch_reports_full_no_participation),
    )
    .await?
    else {
        return Ok(());
    };

    // Allow the first epoch to elapse without any VRF commits/reveals.
    let client = network.client();
    let target_height = EPOCH_LENGTH_BLOCKS.saturating_add(1);
    let status = client.get_status()?;
    for idx in status.blocks..target_height {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("vrf no-participation tick {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total > EPOCH_LENGTH_BLOCKS)
        .await?;

    let http = HttpClient::new();
    let telemetry_url = client
        .torii_url
        .join("v1/sumeragi/telemetry")
        .wrap_err("compose telemetry URL")?;

    let epoch = 0_u64;
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

    let telemetry = wait_for_telemetry(&http, &telemetry_url, |json| {
        let committed_total = json
            .get("vrf_committed_no_reveal_total")
            .and_then(Value::as_u64)?;
        let no_participation_total = json
            .get("vrf_no_participation_total")
            .and_then(Value::as_u64)?;
        let epoch_reported = json.get("vrf_penalty_epoch").and_then(Value::as_u64)?;
        Some(committed_total == 0 && no_participation_total == 4 && epoch_reported == epoch)
    })
    .await?;
    ensure!(
        telemetry
            .get("vrf_no_participation_total")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            == 4,
        "telemetry should report four validators without participation"
    );

    let status = wait_for_sumeragi_status(&client, |json| {
        let epoch_reported = json.get("vrf_penalty_epoch")?.as_u64()?;
        let no_participation_total = json.get("vrf_no_participation_total")?.as_u64()?;
        Some(epoch_reported == epoch && no_participation_total == 4)
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

    network.shutdown().await;
    Ok(())
}

fn randomness_network_builder() -> NetworkBuilder {
    let params = short_epoch_npos_parameters();

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
                .write(["sumeragi", "pacemaker", "backoff_multiplier"], 1_i64)
                .write(["sumeragi", "pacemaker", "rtt_floor_multiplier"], 1_i64)
                .write(["sumeragi", "pacemaker", "max_backoff_ms"], 1_000_i64);
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CollectorsK(1),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::RedundantSendR(1),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            params.into_custom_parameter(),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::NextMode(SumeragiConsensusMode::Npos),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(1),
        )))
}

fn short_epoch_npos_parameters() -> SumeragiNposParameters {
    SumeragiNposParameters {
        vrf_commit_window_blocks: VRF_COMMIT_WINDOW_BLOCKS,
        vrf_reveal_window_blocks: VRF_REVEAL_WINDOW_BLOCKS,
        epoch_length_blocks: EPOCH_LENGTH_BLOCKS,
        block_time_ms: 600,
        timeout_commit_ms: 600,
        timeout_prevote_ms: 600,
        timeout_precommit_ms: 600,
        timeout_propose_ms: 600,
        timeout_da_ms: 600,
        timeout_aggregator_ms: 600,
        ..SumeragiNposParameters::default()
    }
}

fn random_bytes() -> [u8; 32] {
    let mut rng = ChaCha8Rng::seed_from_u64(0x5355_4d45);
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    bytes
}

fn commitment_from_reveal(reveal: &[u8; 32]) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, reveal);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

async fn submit_vrf_commit(
    client: &Client,
    http: &HttpClient,
    epoch: u64,
    signer: u32,
    commitment: [u8; 32],
) -> Result<()> {
    let url = client
        .torii_url
        .join("v1/sumeragi/vrf/commit")
        .wrap_err("compose VRF commit URL")?;
    let body = format!(
        "{{\"epoch\":{epoch},\"signer\":{signer},\"commitment_hex\":\"{}\"}}",
        hex::encode(commitment)
    );
    let mut request = http
        .post(url)
        .header("content-type", "application/json")
        .body(body);
    if let Some(auth) = client.headers.get("Authorization") {
        request = request.header("Authorization", auth);
    }
    let response = request.send().await.wrap_err("submit VRF commit")?;
    ensure!(
        response.status().is_success(),
        "VRF commit submission failed: {}",
        response.status()
    );
    Ok(())
}

async fn submit_vrf_reveal(
    client: &Client,
    http: &HttpClient,
    epoch: u64,
    signer: u32,
    reveal: [u8; 32],
) -> Result<()> {
    let url = client
        .torii_url
        .join("v1/sumeragi/vrf/reveal")
        .wrap_err("compose VRF reveal URL")?;
    let body = format!(
        "{{\"epoch\":{epoch},\"signer\":{signer},\"reveal_hex\":\"{}\"}}",
        hex::encode(reveal)
    );
    let mut request = http
        .post(url)
        .header("content-type", "application/json")
        .body(body);
    if let Some(auth) = client.headers.get("Authorization") {
        request = request.header("Authorization", auth);
    }
    let response = request.send().await.wrap_err("submit VRF reveal")?;
    ensure!(
        response.status().is_success(),
        "VRF reveal submission failed: {}",
        response.status()
    );
    Ok(())
}

async fn wait_for_epoch_record<F>(client: &Client, epoch: u64, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> bool,
{
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 30;
    for attempt in 0..RETRIES {
        let value = client.get_sumeragi_vrf_epoch_json(epoch)?;
        if value.get("found").and_then(Value::as_bool).unwrap_or(false) && predicate(&value) {
            return Ok(value);
        }
        if attempt + 1 == RETRIES {
            break;
        }
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("VRF epoch record not available for epoch {epoch}")
}

async fn wait_for_penalties<F>(client: &Client, epoch: u64, predicate: F) -> Result<Value>
where
    F: Fn(&Value) -> bool,
{
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 30;
    for attempt in 0..RETRIES {
        let value = client.get_sumeragi_vrf_penalties_json(epoch)?;
        if predicate(&value) {
            return Ok(value);
        }
        if attempt + 1 == RETRIES {
            break;
        }
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("VRF penalties snapshot not available for epoch {epoch}")
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

async fn wait_for_height_at_least(client: &Client, min_pos_in_epoch: u64) -> Result<()> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 30;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        let height = status.blocks;
        if height > 0 {
            let pos = ((height - 1) % EPOCH_LENGTH_BLOCKS) + 1;
            if pos >= min_pos_in_epoch {
                return Ok(());
            }
        }
        client.submit_blocking(Log::new(Level::INFO, format!("vrf height tick {attempt}")))?;
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!(
        "failed to reach position {min_pos_in_epoch} within epoch (length {})",
        EPOCH_LENGTH_BLOCKS
    )
}
