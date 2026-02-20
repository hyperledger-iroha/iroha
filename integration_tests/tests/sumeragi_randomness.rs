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
use eyre::{Result, WrapErr, ensure};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_data_model::{
    Level,
    isi::{Log, SetParameter},
    parameter::{
        Parameter,
        system::{SumeragiNposParameters, SumeragiParameter},
    },
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::{self, Value};
use rand::{Rng as _, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::Client as HttpClient;
use sha2::{Digest as _, Sha256};
use tokio::time::sleep;

const EPOCH_LENGTH_BLOCKS: u64 = 6;
const VRF_COMMIT_WINDOW_BLOCKS: u64 = 2;
const VRF_REVEAL_WINDOW_BLOCKS: u64 = 4;
const BLOCK_TIME_MS: u64 = 600;
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
    let epoch = wait_for_epoch_commit_window(&client).await?;

    let http = HttpClient::new();
    let telemetry_url = client
        .torii_url
        .join("v1/sumeragi/telemetry")
        .wrap_err("compose telemetry URL")?;

    let target_signer = 1_u32;

    let reveal = random_bytes();
    let commitment = commitment_from_reveal(&reveal);

    submit_vrf_commit(&client, &http, epoch, target_signer, commitment).await?;
    client.submit_blocking(Log::new(Level::INFO, "vrf commit flush".to_owned()))?;

    // Wait until the epoch record reflects the commitment.
    let snapshot_before = wait_for_epoch_record(&client, epoch, |json| {
        json.get("participants")
            .and_then(Value::as_array)
            .is_some_and(|participants| !participants.is_empty())
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
    let reveal_cutoff_height = epoch
        .saturating_mul(EPOCH_LENGTH_BLOCKS)
        .saturating_add(VRF_REVEAL_WINDOW_BLOCKS)
        .saturating_add(1);
    wait_for_height_total_at_least(&client, reveal_cutoff_height).await?;

    submit_vrf_reveal(&client, &http, epoch, target_signer, reveal).await?;
    client.submit_blocking(Log::new(Level::INFO, "vrf reveal flush".to_owned()))?;

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
    let finalize_height = epoch.saturating_add(1).saturating_mul(EPOCH_LENGTH_BLOCKS);
    let status = client.get_status()?;
    for idx in status.blocks..finalize_height {
        client.submit_blocking(Log::new(Level::INFO, format!("vrf finalize tick {idx}")))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= finalize_height)
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
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("vrf no-participation tick {idx}"),
        ))?;
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
    ensure!(
        response.status().is_success(),
        "VRF reveal submission failed: {}",
        response.status()
    );
    Ok(())
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

async fn wait_for_epoch_commit_window(client: &Client) -> Result<u64> {
    // Align near the end of commit window so manual commits can reliably
    // override earlier automatic commitments for the same signer.
    wait_for_epoch_position(client, VRF_COMMIT_WINDOW_BLOCKS).await
}

async fn wait_for_epoch_position(client: &Client, desired_position: u64) -> Result<u64> {
    ensure!(
        (1..=EPOCH_LENGTH_BLOCKS).contains(&desired_position),
        "desired epoch position {desired_position} out of range 1..={EPOCH_LENGTH_BLOCKS}"
    );
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 60;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        let (epoch, position) = epoch_and_position_from_height(status.blocks);
        if position == desired_position {
            return Ok(epoch);
        }
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("vrf align epoch-position tick {attempt}"),
        ))?;
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("failed to align to epoch position {desired_position}")
}

async fn wait_for_height_total_at_least(client: &Client, min_height: u64) -> Result<()> {
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    const RETRIES: usize = 60;
    for attempt in 0..RETRIES {
        let status = client.get_status()?;
        if status.blocks >= min_height {
            return Ok(());
        }
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("vrf advance height tick {attempt}"),
        ))?;
        sleep(RETRY_INTERVAL).await;
    }
    eyre::bail!("failed to reach block height {min_height}")
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
