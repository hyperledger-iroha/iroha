//! Integration coverage for staged consensus mode cutovers.
use std::{convert::TryFrom, fs, path::PathBuf, time::Duration};

use eyre::{Result, bail, ensure};
use hex::{decode as hex_decode, encode as hex_encode};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_core::sumeragi::consensus::{
    ConsensusGenesisParams, NPOS_TAG, NposGenesisParams, PERMISSIONED_TAG,
    compute_consensus_fingerprint_from_params,
};
use iroha_data_model::{
    ChainId, Level,
    isi::{Log, SetParameter},
    parameter::{
        Parameter, Parameters,
        system::{
            SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter, consensus_metadata,
        },
    },
    transaction::Executable,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder};
use iroha_test_network::{NetworkBuilder, NetworkPeer, chain_id, init_instruction_registry};
use tempfile::tempdir;
use tokio::time::sleep;

const ACTIVATION_HEIGHT: u64 = 4;
const EPOCH_LENGTH_BLOCKS: u64 = 6;
const STATUS_POLL_LIMIT: usize = 30;
const NPOS_BLS_DOMAIN: &str = "bls-iroha2:npos-sumeragi:v1";
const MODE_POLL_DELAY: Duration = Duration::from_millis(200);

fn staged_npos_params() -> SumeragiNposParameters {
    SumeragiNposParameters {
        epoch_length_blocks: EPOCH_LENGTH_BLOCKS,
        block_time_ms: 500,
        timeout_commit_ms: 500,
        timeout_prevote_ms: 500,
        timeout_precommit_ms: 500,
        timeout_propose_ms: 500,
        timeout_da_ms: 500,
        timeout_aggregator_ms: 500,
        vrf_commit_window_blocks: 2,
        vrf_reveal_window_blocks: 4,
        ..SumeragiNposParameters::default()
    }
}

fn cutover_builder(peers: usize, npos_params: SumeragiNposParameters) -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(peers)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["sumeragi", "collectors_k"], 1_i64)
                .write(["sumeragi", "collectors_redundant_send_r"], 1_i64)
                .write(["sumeragi", "da_enabled"], true)
                .write(
                    ["sumeragi", "epoch_length_blocks"],
                    i64::try_from(EPOCH_LENGTH_BLOCKS)
                        .expect("epoch length fits in i64 for test configuration"),
                );
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
            npos_params.into_custom_parameter(),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::NextMode(SumeragiConsensusMode::Npos),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(ACTIVATION_HEIGHT),
        )))
}

fn epoch_length(status: &norito::json::Value) -> u64 {
    status
        .get("epoch_length_blocks")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or_default()
}

fn collectors_consensus_mode(value: &norito::json::Value) -> Option<String> {
    value
        .get("consensus_mode")
        .and_then(|v| v.as_str().map(str::to_string))
}

struct HandshakeMeta {
    mode: String,
    bls_domain: String,
    fingerprint_hex: String,
}

fn extract_handshake_meta(genesis: &GenesisBlock) -> Result<HandshakeMeta> {
    for tx in genesis.0.external_transactions() {
        if let Executable::Instructions(batch) = tx.instructions() {
            for instruction in batch {
                if let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>()
                    && let Parameter::Custom(custom) = set_param.inner()
                    && custom.id() == &consensus_metadata::handshake_meta_id()
                {
                    let payload: norito::json::Value = custom
                        .payload()
                        .try_into_any()
                        .map_err(|err| eyre::eyre!("failed to decode handshake payload: {err}"))?;
                    if let norito::json::Value::Object(mut map) = payload {
                        let mode = map
                            .remove("mode")
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_default();
                        let bls_domain = map
                            .remove("bls_domain")
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_default();
                        let fingerprint_hex = map
                            .remove("consensus_fingerprint")
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_default();
                        return Ok(HandshakeMeta {
                            mode,
                            bls_domain,
                            fingerprint_hex,
                        });
                    }
                }
            }
        }
    }

    Err(eyre::eyre!(
        "consensus handshake metadata missing from genesis block"
    ))
}

fn consensus_params_for_mode(
    params: &Parameters,
    bls_domain: &str,
    mode: SumeragiConsensusMode,
) -> ConsensusGenesisParams {
    let npos_payload = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);

    let npos = if matches!(mode, SumeragiConsensusMode::Npos) {
        npos_payload.map(|payload| NposGenesisParams {
            block_time_ms: payload.block_time_ms,
            timeout_propose_ms: payload.timeout_propose_ms,
            timeout_prevote_ms: payload.timeout_prevote_ms,
            timeout_precommit_ms: payload.timeout_precommit_ms,
            timeout_commit_ms: payload.timeout_commit_ms,
            timeout_da_ms: payload.timeout_da_ms,
            timeout_aggregator_ms: payload.timeout_aggregator_ms,
            k_aggregators: payload.k_aggregators,
            redundant_send_r: payload.redundant_send_r,
            epoch_seed: payload.epoch_seed,
            vrf_commit_window_blocks: payload.vrf_commit_window_blocks,
            vrf_reveal_window_blocks: payload.vrf_reveal_window_blocks,
            max_validators: payload.max_validators,
            min_self_bond: payload.min_self_bond,
            min_nomination_bond: payload.min_nomination_bond,
            max_nominator_concentration_pct: payload.max_nominator_concentration_pct,
            seat_band_pct: payload.seat_band_pct,
            max_entity_correlation_pct: payload.max_entity_correlation_pct,
            finality_margin_blocks: payload.finality_margin_blocks,
            evidence_horizon_blocks: payload.evidence_horizon_blocks,
            activation_lag_blocks: payload.activation_lag_blocks,
            slashing_delay_blocks: payload.slashing_delay_blocks,
        })
    } else {
        None
    };

    let epoch_length_blocks = if matches!(mode, SumeragiConsensusMode::Npos) {
        npos_payload.map_or(0, |payload| payload.epoch_length_blocks())
    } else {
        0
    };

    ConsensusGenesisParams {
        block_time_ms: params.sumeragi().block_time_ms,
        commit_time_ms: params.sumeragi().commit_time_ms,
        max_clock_drift_ms: params.sumeragi().max_clock_drift_ms,
        collectors_k: params.sumeragi().collectors_k,
        redundant_send_r: params.sumeragi().collectors_redundant_send_r,
        block_max_transactions: params.block().max_transactions().get(),
        da_enabled: params.sumeragi().da_enabled,
        epoch_length_blocks,
        bls_domain: bls_domain.to_string(),
        npos,
    }
}

fn consensus_fingerprint_bytes(
    chain_id: &ChainId,
    params: &Parameters,
    bls_domain: &str,
    mode: SumeragiConsensusMode,
) -> Vec<u8> {
    let mode_tag = match mode {
        SumeragiConsensusMode::Permissioned => PERMISSIONED_TAG,
        SumeragiConsensusMode::Npos => NPOS_TAG,
    };
    let canon = consensus_params_for_mode(params, bls_domain, mode);
    compute_consensus_fingerprint_from_params(chain_id, &canon, mode_tag).to_vec()
}

async fn wait_for_collectors_mode_all(clients: &[Client], expected: &str) -> Result<()> {
    for attempt in 0..STATUS_POLL_LIMIT {
        let all_match = clients.iter().all(|client| {
            client
                .get_sumeragi_collectors_json()
                .ok()
                .and_then(|value| collectors_consensus_mode(&value))
                .is_some_and(|mode| mode.eq_ignore_ascii_case(expected))
        });
        if all_match {
            return Ok(());
        }
        if attempt + 1 >= STATUS_POLL_LIMIT {
            break;
        }
        sleep(MODE_POLL_DELAY).await;
    }

    bail!("collectors mode never reached expected value `{expected}`")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissioned_to_npos_cutover_switches_mode_at_activation_height() -> Result<()> {
    init_instruction_registry();

    let builder = cutover_builder(1, staged_npos_params());

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_to_npos_cutover_switches_mode_at_activation_height),
    )
    .await?
    else {
        return Ok(());
    };
    network
        .ensure_blocks_with(|height| height.total >= 1)
        .await?;

    let client = network.client();
    let pre_status = client.get_sumeragi_status_json()?;
    ensure!(
        epoch_length(&pre_status) == 0,
        "epoch_length_blocks should reflect permissioned mode before activation, got {pre_status:?}"
    );

    let target_height = ACTIVATION_HEIGHT.saturating_add(1);
    let status = client.get_status()?;
    for idx in status.blocks..target_height {
        client.submit_blocking(Log::new(Level::INFO, format!("cutover seed {idx}")))?;
    }
    network
        .ensure_blocks_with(|height| height.total > ACTIVATION_HEIGHT)
        .await?;

    let mut attempts = 0;
    let post_status = loop {
        let status = client.get_sumeragi_status_json()?;
        if epoch_length(&status) == EPOCH_LENGTH_BLOCKS {
            break status;
        }
        attempts += 1;
        ensure!(
            attempts < STATUS_POLL_LIMIT,
            "epoch_length_blocks never reached {EPOCH_LENGTH_BLOCKS} after cutover, last={status:?}"
        );
        sleep(Duration::from_millis(200)).await;
    };

    let params = client.get_sumeragi_params_json()?;
    ensure!(
        params
            .get("mode_activation_height")
            .and_then(norito::json::Value::as_u64)
            == Some(ACTIVATION_HEIGHT),
        "params should expose the staged activation height, got {params:?}"
    );
    ensure!(
        params
            .get("next_mode")
            .and_then(|value| value.as_str().map(str::to_string))
            == Some("Npos".to_string()),
        "params should advertise staged next_mode Npos, got {params:?}"
    );
    ensure!(
        epoch_length(&post_status) == EPOCH_LENGTH_BLOCKS,
        "post-activation status should reflect NPoS epoch length, got {post_status:?}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn staged_cutover_recomputes_consensus_fingerprint() -> Result<()> {
    init_instruction_registry();

    let builder = cutover_builder(2, staged_npos_params());
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(staged_cutover_recomputes_consensus_fingerprint),
    )
    .await?
    else {
        return Ok(());
    };
    let chain_id = network.chain_id();
    let handshake = extract_handshake_meta(&network.genesis())?;
    ensure!(
        handshake.mode.eq_ignore_ascii_case("permissioned"),
        "genesis handshake should advertise the active permissioned mode, got {}",
        handshake.mode
    );
    let handshake_bytes = hex_decode(handshake.fingerprint_hex.trim_start_matches("0x"))
        .map_err(|err| eyre::eyre!("failed to decode handshake fingerprint: {err}"))?;
    ensure!(
        handshake_bytes.len() == 32,
        "handshake fingerprint must be 32 bytes, got {}",
        handshake_bytes.len()
    );

    let clients: Vec<Client> = network.peers().iter().map(NetworkPeer::client).collect();
    ensure!(
        !clients.is_empty(),
        "expected at least one client from the test network"
    );

    let mut pre_activation = Vec::new();
    for client in &clients {
        let params = client.get_parameters()?;
        let computed = consensus_fingerprint_bytes(
            &chain_id,
            &params,
            &handshake.bls_domain,
            SumeragiConsensusMode::Permissioned,
        );
        ensure!(
            computed == handshake_bytes,
            "pre-activation fingerprint must match handshake metadata (computed={}, handshake={})",
            hex_encode(&computed),
            handshake.fingerprint_hex
        );
        if let Some(mode) = client
            .get_sumeragi_collectors_json()?
            .get("consensus_mode")
            .and_then(|value| value.as_str().map(str::to_string))
        {
            ensure!(
                mode.eq_ignore_ascii_case("permissioned"),
                "collectors endpoint should advertise permissioned mode before activation, got {mode}"
            );
        }
        pre_activation.push(computed);
    }
    let first_pre = pre_activation
        .first()
        .cloned()
        .expect("pre-activation fingerprints exist");
    ensure!(
        pre_activation.iter().all(|fp| *fp == first_pre),
        "all peers must agree on the pre-activation fingerprint: {:?}",
        pre_activation
    );

    let client = network.client();
    let target_height = ACTIVATION_HEIGHT.saturating_add(1);
    let status = client.get_status()?;
    for idx in status.blocks..target_height {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("cutover fingerprint seed {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total > ACTIVATION_HEIGHT)
        .await?;
    wait_for_collectors_mode_all(&clients, "npos").await?;

    let post_activation: Vec<Vec<u8>> = clients
        .iter()
        .map(|client| {
            let params = client.get_parameters()?;
            Ok(consensus_fingerprint_bytes(
                &chain_id,
                &params,
                NPOS_BLS_DOMAIN,
                SumeragiConsensusMode::Npos,
            ))
        })
        .collect::<Result<_>>()?;

    let first_post = post_activation
        .first()
        .cloned()
        .expect("post-activation fingerprints exist");
    ensure!(
        post_activation.iter().all(|fp| *fp == first_post),
        "all peers must agree on the post-activation fingerprint: {:?}",
        post_activation
    );
    ensure!(
        first_post != handshake_bytes,
        "activation should change consensus fingerprint: pre={} post={}",
        hex_encode(&handshake_bytes),
        hex_encode(&first_post)
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manifest_fingerprint_mismatch_blocks_startup() -> Result<()> {
    init_instruction_registry();

    let npos_params = staged_npos_params();
    let manifest_builder = GenesisBuilder::new_without_executor(chain_id(), PathBuf::from("."))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::CollectorsK(1)))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::RedundantSendR(1)))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::DaEnabled(true)))
        .append_parameter(Parameter::Custom(npos_params.into_custom_parameter()))
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
            SumeragiConsensusMode::Npos,
        )))
        .append_parameter(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(ACTIVATION_HEIGHT),
        ));
    let manifest = manifest_builder
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned)
        .with_consensus_meta();

    let base_fp = manifest
        .consensus_fingerprint()
        .unwrap_or_default()
        .trim_start_matches("0x");
    let mut fp_bytes =
        hex_decode(base_fp).map_err(|err| eyre::eyre!("decode manifest fingerprint: {err}"))?;
    if let Some(first) = fp_bytes.first_mut() {
        *first ^= 0xFF;
    }
    let mut bad_manifest = norito::json::value::to_value(&manifest)?;
    if let norito::json::Value::Object(ref mut map) = bad_manifest {
        map.insert(
            "consensus_fingerprint".to_string(),
            norito::json::Value::String(format!("0x{}", hex_encode(fp_bytes))),
        );
    }

    let dir = tempdir()?;
    let manifest_path = dir.path().join("manifest.json");
    let manifest_bytes = norito::json::to_vec_pretty(&bad_manifest)?;
    fs::write(&manifest_path, manifest_bytes)?;

    let builder = cutover_builder(1, npos_params).with_config_layer(|layer| {
        layer.write(
            ["genesis", "manifest_json"],
            manifest_path.display().to_string(),
        );
    });

    match sandbox::start_network_async_or_skip(
        builder,
        stringify!(manifest_fingerprint_mismatch_blocks_startup),
    )
    .await
    {
        Ok(Some(network)) => {
            network.shutdown().await;
            bail!("network should reject mismatched consensus fingerprint in manifest");
        }
        Ok(None) => Ok(()),
        Err(err) => {
            ensure!(
                err.to_string().contains("consensus_fingerprint mismatch"),
                "unexpected startup error: {err:?}"
            );
            Ok(())
        }
    }
}
