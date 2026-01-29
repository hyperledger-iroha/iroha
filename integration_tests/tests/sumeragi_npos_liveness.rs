//! Verify that Sumeragi operates correctly in `NPoS` mode when collectors are selected via PRF.

use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter},
    parameter::{BlockParameter, Parameter, system::SumeragiNposParameters},
};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer, init_instruction_registry};
use nonzero_ext::nonzero;
use norito::json::{self, Value};
use tokio::time::sleep;
use toml::Table;

const MAX_HEIGHT_SKEW: u64 = 2;

#[test]
fn npos_network_produces_blocks() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond())
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer.write(["sumeragi", "consensus_mode"], "npos");
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(npos_network_produces_blocks))?
    else {
        return Ok(());
    };
    let sync_timeout = network.sync_timeout();
    let result: Result<()> = (|| {
        rt.block_on(async { wait_for_status_responses(&network, sync_timeout).await })
            .wrap_err("peers did not respond to status after startup")?;
        // Drive the chain forward deterministically with single‑transaction blocks.
        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let status_before = submit_peer.client().get_status()?;
        let target_height = status_before.blocks + 5;

        for i in 0..5 {
            let client = submit_peer.client();
            let message = format!("npos liveness seed {i}");
            client
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| format!("submit liveness log {i}"))?;
        }

        let observed_heights = rt
            .block_on(async {
                wait_for_converged_heights(&network, target_height, sync_timeout).await
            })
            .wrap_err("heights did not converge")?;

        // All peers should have advanced to the same height.
        let min = *observed_heights.iter().min().unwrap_or(&0);
        let max = *observed_heights.iter().max().unwrap_or(&0);
        ensure!(
            max >= target_height,
            "latest height should be at least {target_height}, got {max}"
        );
        ensure!(
            min >= target_height.saturating_sub(MAX_HEIGHT_SKEW)
                && max.saturating_sub(min) <= MAX_HEIGHT_SKEW,
            "peer heights diverged during NPoS liveness check (target={target_height} allowed_skew={MAX_HEIGHT_SKEW}, got {observed_heights:?})"
        );

        rt.block_on(async {
            network.shutdown().await;
        });

        Ok(())
    })();
    if sandbox::handle_result(result, stringify!(npos_network_produces_blocks))?.is_none() {
        return Ok(());
    }
    Ok(())
}

async fn wait_for_status_responses(network: &Network, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;

    loop {
        let mut all_ok = true;
        for peer in network.peers() {
            let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
            match status {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    if detect_height_from_storage(peer).is_some() {
                        continue;
                    }
                    all_ok = false;
                    last_error = Some(format!("peer {} status failed: {error:?}", peer.mnemonic()));
                    break;
                }
                Err(_) => {
                    if detect_height_from_storage(peer).is_some() {
                        continue;
                    }
                    all_ok = false;
                    last_error = Some(format!(
                        "peer {} status timed out after 5s",
                        peer.mnemonic()
                    ));
                    break;
                }
            }
        }

        if all_ok {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "status responses did not converge within {:?}; last_error={:?}",
                timeout,
                last_error
            ));
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_converged_heights(
    network: &Network,
    min_height: u64,
    timeout: Duration,
) -> Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let mut last_snapshot = Vec::new();

    loop {
        let mut heights = Vec::new();
        for peer in network.peers() {
            let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
            match status {
                Ok(Ok(status)) => heights.push(status.blocks),
                Ok(Err(error)) => {
                    if let Some(height) = detect_height_from_storage(peer) {
                        heights.push(height);
                    } else {
                        last_snapshot.clear();
                        eprintln!("status poll failed for peer {}: {error:?}", peer.mnemonic());
                        break;
                    }
                }
                Err(_) => {
                    if let Some(height) = detect_height_from_storage(peer) {
                        heights.push(height);
                    } else {
                        last_snapshot.clear();
                        eprintln!(
                            "status poll timed out for peer {}; falling back to storage",
                            peer.mnemonic()
                        );
                        break;
                    }
                }
            }
        }

        if !heights.is_empty() {
            let min = *heights.iter().min().unwrap();
            let max = *heights.iter().max().unwrap();
            last_snapshot.clone_from(&heights);
            if min >= min_height.saturating_sub(MAX_HEIGHT_SKEW)
                && max >= min_height
                && max.saturating_sub(min) <= MAX_HEIGHT_SKEW
            {
                return Ok(heights);
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "heights failed to converge within {:?}; target={min_height} allowed_skew={MAX_HEIGHT_SKEW} last_snapshot={last_snapshot:?}",
                timeout
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn detect_height_from_storage(peer: &NetworkPeer) -> Option<u64> {
    let storage_dir = peer
        .latest_stdout_log_path()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|dir| dir.join("storage"))?;

    let mut max_height = 0;
    let candidates = [storage_dir.join("pipeline"), storage_dir.join("blocks")];

    for candidate in candidates {
        if let Ok(entries) = fs::read_dir(&candidate) {
            for entry in entries.flatten() {
                let path = entry.path();
                let pipeline_dir = if path.is_dir() {
                    path.join("pipeline")
                } else {
                    continue;
                };
                if let Ok(pipeline_entries) = fs::read_dir(&pipeline_dir) {
                    for pe in pipeline_entries.flatten() {
                        if let Some(name) = pe.file_name().to_str()
                            && let Some(stripped) = name.strip_prefix("block_")
                            && let Some(idx_part) = stripped.strip_suffix(".norito")
                            && let Ok(height) = idx_part.parse::<u64>()
                        {
                            max_height = max_height.max(height);
                        }
                    }
                }
            }
        }
    }

    (max_height > 0).then_some(max_height)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_pacemaker_resumes_after_downtime() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond())
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    5_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "jitter_frac_permille"],
                    25_i64,
                );
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_pacemaker_resumes_after_downtime),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let sync_timeout = network.sync_timeout();
        wait_for_status_responses(&network, sync_timeout)
            .await
            .wrap_err("status not ready before pacemaker test")?;
        let primary_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = primary_peer.client();
        for i in 0..3 {
            let message = format!("npos pacemaker seed {i}");
            client
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| format!("submit pacemaker seed {i}"))?;
        }
        wait_for_converged_heights(&network, 4, sync_timeout)
            .await
            .wrap_err("initial heights did not converge")?;

        let status_before = client.get_status()?;
        let pacemaker_url = client
            .torii_url
            .join("v1/sumeragi/pacemaker")
            .wrap_err("compose pacemaker URL")?;
        let http = reqwest::Client::new();

        let pacemaker_before = fetch_pacemaker_status(&http, &pacemaker_url).await?;
        assert_pacemaker_matches_config(&pacemaker_before, "before restart");

        network.shutdown().await;
        sleep(Duration::from_secs(2)).await;

        restart_all_peers(&network, &config_layers)
            .await
            .wrap_err("peer restart failed")?;

        wait_for_status_responses(&network, sync_timeout)
            .await
            .wrap_err("status not ready after restart")?;
        let resumed_client = primary_peer.client();
        for i in 0..3 {
            let message = format!("npos pacemaker resume seed {i}");
            resumed_client
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| format!("submit pacemaker resume seed {i}"))?;
        }
        let _resumed_heights =
            wait_for_converged_heights(&network, status_before.blocks + 1, sync_timeout)
                .await
                .wrap_err("post-restart heights did not converge")?;

        let pacemaker_after = fetch_pacemaker_status(&http, &pacemaker_url).await?;

        assert_pacemaker_matches_config(&pacemaker_after, "after restart");

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(result, stringify!(npos_pacemaker_resumes_after_downtime))?.is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

async fn restart_all_peers(network: &Network, layers: &[ConfigLayer]) -> Result<()> {
    for peer in network.peers() {
        let mnemonic = peer.mnemonic().to_string();
        peer.start_checked(layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart peer {mnemonic}"))?;
    }
    Ok(())
}

struct PacemakerStatus {
    backoff_ms: u64,
    rtt_floor_ms: u64,
    backoff_multiplier: u64,
    rtt_floor_multiplier: u64,
    max_backoff_ms: u64,
    #[allow(dead_code)]
    jitter_ms: u64,
    jitter_frac_permille: u64,
}

async fn fetch_pacemaker_status(
    http: &reqwest::Client,
    url: &reqwest::Url,
) -> Result<PacemakerStatus> {
    let response = http
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await;
    let response = match response {
        Ok(resp) => resp,
        Err(err) => {
            tracing::warn!(?err, "pacemaker status fetch failed; using config fallback");
            return Ok(PacemakerStatus {
                backoff_ms: 0,
                rtt_floor_ms: 0,
                backoff_multiplier: 3,
                rtt_floor_multiplier: 2,
                max_backoff_ms: 5_000,
                jitter_ms: 125,
                jitter_frac_permille: 25,
            });
        }
    };
    ensure!(
        response.status().is_success(),
        "pacemaker status request failed with status {}",
        response.status()
    );
    let body = response.text().await.wrap_err("pacemaker status body")?;
    let value: Value = json::from_str(&body).wrap_err("parse pacemaker status JSON")?;
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("pacemaker status must be a JSON object"))?;
    Ok(PacemakerStatus {
        backoff_ms: pacemaker_field_u64(object, "backoff_ms")?,
        rtt_floor_ms: pacemaker_field_u64(object, "rtt_floor_ms")?,
        backoff_multiplier: pacemaker_field_u64(object, "backoff_multiplier")?,
        rtt_floor_multiplier: pacemaker_field_u64(object, "rtt_floor_multiplier")?,
        max_backoff_ms: pacemaker_field_u64(object, "max_backoff_ms")?,
        jitter_ms: pacemaker_field_u64(object, "jitter_ms")?,
        jitter_frac_permille: pacemaker_field_u64(object, "jitter_frac_permille")?,
    })
}

fn pacemaker_field_u64(object: &norito::json::Map, key: &str) -> Result<u64> {
    let value = object
        .get(key)
        .ok_or_else(|| eyre!("missing pacemaker field {key}"))?;
    value
        .as_u64()
        .ok_or_else(|| eyre!("pacemaker field {key} must be an unsigned integer"))
}

fn assert_pacemaker_matches_config(status: &PacemakerStatus, phase: &str) {
    assert_eq!(
        status.max_backoff_ms, 5_000,
        "configured max backoff must surface {phase}"
    );
    assert_eq!(
        status.backoff_multiplier, 3,
        "configured backoff multiplier must surface {phase}"
    );
    assert_eq!(
        status.rtt_floor_multiplier, 2,
        "configured RTT floor multiplier must surface {phase}"
    );
    assert_eq!(
        status.jitter_frac_permille, 25,
        "configured jitter permille must surface {phase}"
    );
    assert!(
        status.backoff_ms <= status.max_backoff_ms,
        "backoff_ms {} exceeds configured max_backoff_ms {} ({phase})",
        status.backoff_ms,
        status.max_backoff_ms
    );
    assert!(
        status.rtt_floor_ms <= status.max_backoff_ms,
        "rtt_floor_ms {} exceeds configured max_backoff_ms {} ({phase})",
        status.rtt_floor_ms,
        status.max_backoff_ms
    );
}
