//! JSON CLI for FASTPQ measurement, proof generation, and verification.

use std::{
    collections::BTreeMap,
    fs,
    num::NonZeroU64,
    path::PathBuf,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Parser, Subcommand};
use fastpq_prover::gadgets::transfer::decode_transcripts;
use fastpq_prover::{
    AXT_DEFAULT_PARAMETER, OperationKind, Proof, Prover, PublicInputs, StateTransition,
    TransitionBatch, batch_manifest_sha256 as axt_batch_manifest_sha256, bind_axt_batch,
    canonicalize_binding, encode_axt_fastpq_payload, transition_batch_from_model, verify,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    fastpq::{FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY, normalized_numeric_to_u64},
    nexus::{
        AxtDescriptor, AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, AxtTouchSpec,
        LANE_RELAY_FASTPQ_EFFECT_TYPE, LaneFastpqProofMaterial, LaneId, LaneRelayEnvelope,
        ProofBlob, TouchManifest, lane_relay_fastpq_claim_digest,
    },
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json, to_bytes,
};
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(name = "fastpq_json")]
#[command(about = "FASTPQ JSON helper for measured budgets and receipt-bound proofs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Measure {
        #[arg(long)]
        input: PathBuf,
    },
    Prove {
        #[arg(long)]
        input: PathBuf,
    },
    Verify {
        #[arg(long)]
        input: PathBuf,
    },
    InspectTransfers {
        #[arg(long)]
        input: PathBuf,
    },
}

#[derive(Debug, Clone, JsonDeserialize)]
struct MeasureInput {
    #[norito(default)]
    dataspace: String,
    #[norito(default)]
    verifier_id: String,
    #[norito(default)]
    verifier_version: String,
    claim_types: Vec<String>,
    #[norito(default)]
    fixtures: Vec<ProofRequest>,
    #[norito(default)]
    parameter: String,
}

#[derive(Debug, Clone, JsonSerialize)]
struct MeasureOutput {
    dataspace: String,
    measurement_mode: String,
    sample_count: usize,
    parameter: String,
    benchmarks: BTreeMap<String, BenchmarkResult>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct BenchmarkResult {
    sample_count: usize,
    proof_bytes_p50: usize,
    proof_bytes_p95: usize,
    prove_ms_p50: f64,
    prove_ms_p95: f64,
    verify_ms_p50: f64,
    verify_ms_p95: f64,
    verifier_id: String,
    verifier_version: String,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct ProofRequest {
    #[norito(default)]
    parameter: String,
    source_dsid: u64,
    #[norito(default)]
    source_dataspace: String,
    #[norito(default)]
    source_receipt_id: String,
    #[norito(default)]
    target_dsids: Vec<u64>,
    source_tx_commitment: String,
    claim_type: String,
    claim_digest: String,
    witness_commitment: String,
    policy_commitment: String,
    verified_effect_type: String,
    #[norito(default)]
    corridor: String,
    #[norito(default)]
    verifier_id: String,
    #[norito(default)]
    verifier_version: String,
    #[norito(default)]
    source_lane_id: u32,
    #[norito(default = "default_relay_block_height")]
    relay_block_height: u64,
    #[norito(default)]
    batch_base64: String,
    #[norito(default)]
    effect_binding: Option<EffectBindingRequest>,
}

#[derive(Debug, Clone, JsonDeserialize, JsonSerialize)]
struct EffectBindingRequest {
    #[norito(default)]
    destination_domain: Option<String>,
    #[norito(default)]
    destination_account_id: Option<String>,
    #[norito(default)]
    vault_account_id: Option<String>,
    #[norito(default)]
    issuance_account_id: Option<String>,
    #[norito(default)]
    source_asset_definition_id: Option<String>,
    #[norito(default)]
    destination_asset_definition_id: Option<String>,
    #[norito(default)]
    source_amount_i64: Option<i64>,
    #[norito(default)]
    destination_amount_i64: Option<i64>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct ProofResponse {
    passed: bool,
    parameter: String,
    proof_bytes_base64: String,
    proof_sha256: String,
    proof_bytes_len: usize,
    prove_ms: f64,
    verify_ms: f64,
    trace_commitment: String,
    batch_manifest_sha256: String,
    dataspace_id_hex: String,
    axt_descriptor_hex: String,
    touch_manifest_hex: String,
    effect_proof_blob_hex: String,
    proof_blob_hex: String,
    manifest_root_hex: String,
    relay_envelope_hex: String,
    relay_ref: RelayRefJson,
}

#[derive(Debug, Clone, JsonSerialize)]
struct RelayRefJson {
    dataspace_id: u64,
    lane_id: u32,
    block_height: u64,
    settlement_hash: String,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct VerifyInput {
    request: ProofRequest,
    proof_bytes_base64: String,
}

#[derive(Debug, Clone, JsonSerialize)]
struct VerifyResponse {
    passed: bool,
    parameter: String,
    proof_sha256: String,
    proof_bytes_len: usize,
    verify_ms: f64,
    trace_commitment: String,
    batch_manifest_sha256: String,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct InspectTransfersInput {
    batch_base64: String,
    #[norito(default)]
    source_account_id: Option<String>,
    #[norito(default)]
    destination_account_id: Option<String>,
    #[norito(default)]
    asset_definition_id: Option<String>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct InspectTransfersOutput {
    transcripts_present: bool,
    transfer_count: usize,
    transfers: Vec<TransferInspectionRecord>,
}

#[derive(Debug, Clone, JsonSerialize)]
struct TransferInspectionRecord {
    transcript_index: usize,
    delta_index: usize,
    batch_hash: String,
    from_account_id: String,
    to_account_id: String,
    asset_definition_id: String,
    normalized_scale: u32,
    amount: String,
    amount_units: Option<u64>,
    from_balance_before: String,
    from_balance_before_units: Option<u64>,
    from_balance_after: String,
    from_balance_after_units: Option<u64>,
    to_balance_before: String,
    to_balance_before_units: Option<u64>,
    to_balance_after: String,
    to_balance_after_units: Option<u64>,
}

fn default_relay_block_height() -> u64 {
    1
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Measure { input } => {
            let request: MeasureInput = read_json(&input)?;
            let response = handle_measure(request)?;
            print_json(&response)
        }
        Command::Prove { input } => {
            let request: ProofRequest = read_json(&input)?;
            let response = handle_prove(request)?;
            print_json(&response)
        }
        Command::Verify { input } => {
            let request: VerifyInput = read_json(&input)?;
            let response = handle_verify(request)?;
            print_json(&response)
        }
        Command::InspectTransfers { input } => {
            let request: InspectTransfersInput = read_json(&input)?;
            let response = handle_inspect_transfers(request)?;
            print_json(&response)
        }
    }
}

fn read_json<T: json::JsonDeserialize>(path: &PathBuf) -> Result<T, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    json::from_str(&raw).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn print_json<T: json::JsonSerialize>(payload: &T) -> Result<(), String> {
    let encoded =
        json::to_string_pretty(payload).map_err(|err| format!("json encode failed: {err}"))?;
    println!("{encoded}");
    Ok(())
}

fn handle_measure(request: MeasureInput) -> Result<MeasureOutput, String> {
    let parameter = normalized_parameter(&request.parameter);
    let verifier_id = normalized_verifier_id(&request.verifier_id);
    let verifier_version = normalized_verifier_version(&request.verifier_version);
    if request.fixtures.is_empty() {
        return Err(
            "measure requires maintained fixtures with execution-captured batch_base64; \
             synthetic descriptor-only requests are not supported"
                .to_string(),
        );
    }
    let mut benchmarks = BTreeMap::new();
    for claim_type in &request.claim_types {
        let claim_type = normalized_claim_type(claim_type)?;
        let proof_requests: Vec<ProofRequest> = request
            .fixtures
            .iter()
            .filter_map(|fixture| {
                let normalized = normalized_claim_type(&fixture.claim_type).ok()?;
                if normalized == claim_type {
                    Some(ProofRequest {
                        parameter: normalized_parameter(&fixture.parameter),
                        ..fixture.clone()
                    })
                } else {
                    None
                }
            })
            .collect();
        if proof_requests.is_empty() {
            return Err(format!(
                "measure input is missing maintained fixtures for claim_type: {claim_type}"
            ));
        }
        let mut proof_sizes = Vec::with_capacity(proof_requests.len());
        let mut prove_ms = Vec::with_capacity(proof_requests.len());
        let mut verify_ms = Vec::with_capacity(proof_requests.len());
        for proof_request in &proof_requests {
            let prove = prove_request(proof_request)?;
            proof_sizes.push(prove.0.len());
            prove_ms.push(prove.1.as_secs_f64() * 1000.0);
            verify_ms.push(prove.2.as_secs_f64() * 1000.0);
        }
        proof_sizes.sort_unstable();
        prove_ms.sort_by(f64::total_cmp);
        verify_ms.sort_by(f64::total_cmp);
        benchmarks.insert(
            claim_type,
            BenchmarkResult {
                sample_count: proof_requests.len(),
                proof_bytes_p50: percentile_usize(&proof_sizes, 50, 100),
                proof_bytes_p95: percentile_usize(&proof_sizes, 95, 100),
                prove_ms_p50: percentile_f64(&prove_ms, 50, 100),
                prove_ms_p95: percentile_f64(&prove_ms, 95, 100),
                verify_ms_p50: percentile_f64(&verify_ms, 50, 100),
                verify_ms_p95: percentile_f64(&verify_ms, 95, 100),
                verifier_id: verifier_id.clone(),
                verifier_version: verifier_version.clone(),
            },
        );
    }
    Ok(MeasureOutput {
        dataspace: request.dataspace,
        measurement_mode: "fastpq_prover_fixture_replay".to_string(),
        sample_count: request.fixtures.len(),
        parameter,
        benchmarks,
    })
}

fn handle_prove(request: ProofRequest) -> Result<ProofResponse, String> {
    let parameter = normalized_parameter(&request.parameter);
    let response_parameter = parameter.clone();
    let normalized_request = ProofRequest {
        parameter,
        ..request
    };
    let (proof_bytes, prove_time, verify_time, trace_commitment, batch_manifest_sha256) =
        prove_request(&normalized_request)?;
    let axt = build_axt_materials(&normalized_request, &proof_bytes)?;
    Ok(ProofResponse {
        passed: true,
        parameter: response_parameter,
        proof_sha256: sha256_hex(&proof_bytes),
        proof_bytes_len: proof_bytes.len(),
        proof_bytes_base64: BASE64_STANDARD.encode(&proof_bytes),
        prove_ms: duration_ms(prove_time),
        verify_ms: duration_ms(verify_time),
        trace_commitment,
        batch_manifest_sha256,
        dataspace_id_hex: axt.dataspace_id,
        axt_descriptor_hex: axt.descriptor,
        touch_manifest_hex: axt.touch_manifest,
        effect_proof_blob_hex: axt.effect_proof_blob,
        proof_blob_hex: axt.proof_blob,
        manifest_root_hex: axt.manifest_root,
        relay_envelope_hex: axt.relay_envelope,
        relay_ref: axt.relay_ref,
    })
}

struct AxtArtifacts {
    dataspace_id: String,
    descriptor: String,
    touch_manifest: String,
    effect_proof_blob: String,
    proof_blob: String,
    manifest_root: String,
    relay_envelope: String,
    relay_ref: RelayRefJson,
}

fn handle_verify(input: VerifyInput) -> Result<VerifyResponse, String> {
    let request = ProofRequest {
        parameter: normalized_parameter(&input.request.parameter),
        ..input.request
    };
    let batch = build_batch_from_request(&request)?;
    let proof_bytes = BASE64_STANDARD
        .decode(input.proof_bytes_base64.as_bytes())
        .map_err(|err| format!("invalid proof_bytes_base64: {err}"))?;
    let proof: Proof = norito::decode_from_bytes(&proof_bytes)
        .map_err(|err| format!("failed to decode proof bytes: {err}"))?;
    let started = Instant::now();
    verify(&batch, &proof).map_err(|err| format!("FASTPQ verification failed: {err}"))?;
    let verify_time = started.elapsed();
    Ok(VerifyResponse {
        passed: true,
        parameter: request.parameter.clone(),
        proof_sha256: sha256_hex(&proof_bytes),
        proof_bytes_len: proof_bytes.len(),
        verify_ms: duration_ms(verify_time),
        trace_commitment: proof.commitment().to_string(),
        batch_manifest_sha256: batch_manifest_sha256(&request),
    })
}

fn handle_inspect_transfers(
    input: InspectTransfersInput,
) -> Result<InspectTransfersOutput, String> {
    let batch = decode_request_batch(&input.batch_base64)?;
    let transcripts = decode_transcripts(&batch.metadata)
        .map_err(|err| format!("failed to decode transfer transcripts: {err}"))?;
    let Some(transcripts) = transcripts else {
        return Ok(InspectTransfersOutput {
            transcripts_present: false,
            transfer_count: 0,
            transfers: Vec::new(),
        });
    };

    let source_filter = trimmed_filter(input.source_account_id);
    let destination_filter = trimmed_filter(input.destination_account_id);
    let asset_filter = trimmed_filter(input.asset_definition_id);
    let mut transfers = Vec::new();
    for (transcript_index, transcript) in transcripts.iter().enumerate() {
        for (delta_index, delta) in transcript.deltas.iter().enumerate() {
            let from_account_id = delta.from_account.to_string();
            let to_account_id = delta.to_account.to_string();
            let asset_definition_id = delta.asset_definition.to_string();
            if let Some(expected) = &source_filter
                && &from_account_id != expected
            {
                continue;
            }
            if let Some(expected) = &destination_filter
                && &to_account_id != expected
            {
                continue;
            }
            if let Some(expected) = &asset_filter
                && &asset_definition_id != expected
            {
                continue;
            }
            let normalized_scale = delta.normalized_scale();
            transfers.push(TransferInspectionRecord {
                transcript_index,
                delta_index,
                batch_hash: transcript.batch_hash.to_string(),
                from_account_id,
                to_account_id,
                asset_definition_id,
                normalized_scale,
                amount: delta.amount.to_string(),
                amount_units: normalized_numeric_to_u64(&delta.amount, normalized_scale),
                from_balance_before: delta.from_balance_before.to_string(),
                from_balance_before_units: normalized_numeric_to_u64(
                    &delta.from_balance_before,
                    normalized_scale,
                ),
                from_balance_after: delta.from_balance_after.to_string(),
                from_balance_after_units: normalized_numeric_to_u64(
                    &delta.from_balance_after,
                    normalized_scale,
                ),
                to_balance_before: delta.to_balance_before.to_string(),
                to_balance_before_units: normalized_numeric_to_u64(
                    &delta.to_balance_before,
                    normalized_scale,
                ),
                to_balance_after: delta.to_balance_after.to_string(),
                to_balance_after_units: normalized_numeric_to_u64(
                    &delta.to_balance_after,
                    normalized_scale,
                ),
            });
        }
    }

    Ok(InspectTransfersOutput {
        transcripts_present: true,
        transfer_count: transfers.len(),
        transfers,
    })
}

fn trimmed_filter(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn prove_request(
    request: &ProofRequest,
) -> Result<(Vec<u8>, Duration, Duration, String, String), String> {
    let batch = build_batch_from_request(request)?;
    let prover = Prover::canonical(&request.parameter)
        .map_err(|err| format!("failed to construct FASTPQ prover: {err}"))?;
    let prove_started = Instant::now();
    let proof = prover
        .prove(&batch)
        .map_err(|err| format!("FASTPQ prove failed: {err}"))?;
    let prove_time = prove_started.elapsed();
    let verify_started = Instant::now();
    verify(&batch, &proof).map_err(|err| format!("FASTPQ verification failed: {err}"))?;
    let verify_time = verify_started.elapsed();
    let proof_bytes =
        norito::to_bytes(&proof).map_err(|err| format!("proof encode failed: {err}"))?;
    Ok((
        proof_bytes,
        prove_time,
        verify_time,
        proof.commitment().to_string(),
        batch_manifest_sha256(request),
    ))
}

fn build_batch_from_request(request: &ProofRequest) -> Result<TransitionBatch, String> {
    let binding = request_to_binding(request);
    let mut batch = if request.batch_base64.trim().is_empty() {
        descriptor_fixture_batch(request)?
    } else {
        decode_request_batch(&request.batch_base64)?
    };
    bind_axt_batch(&mut batch, &binding)
        .map_err(|err| format!("failed to bind AXT metadata to FASTPQ batch: {err}"))?;
    Ok(batch)
}

fn descriptor_fixture_batch(request: &ProofRequest) -> Result<TransitionBatch, String> {
    let parameter = normalized_parameter(&request.parameter);
    let claim_type = normalized_claim_type(&request.claim_type)?;
    let seed = format!(
        "{}:{}:{}:{}:{}:{}",
        request.source_dsid,
        request.source_tx_commitment,
        request.claim_digest,
        request.witness_commitment,
        request.policy_commitment,
        request.verified_effect_type
    );
    let mut batch = TransitionBatch::new(
        parameter,
        PublicInputs {
            dsid: dsid_bytes(request.source_dsid),
            slot: request.relay_block_height.max(1),
            old_root: digest32(format!("old:{seed}").as_bytes()),
            new_root: digest32(format!("new:{seed}").as_bytes()),
            perm_root: digest32(format!("perm:{seed}").as_bytes()),
            tx_set_hash: digest32(format!("tx:{seed}").as_bytes()),
        },
    );

    if matches!(claim_type.as_str(), "tx_predicate" | "value_conservation") {
        let amount = request
            .effect_binding
            .as_ref()
            .and_then(|binding| binding.source_amount_i64)
            .unwrap_or(1)
            .unsigned_abs()
            .max(1);
        let destination_amount = request
            .effect_binding
            .as_ref()
            .and_then(|binding| binding.destination_amount_i64)
            .unwrap_or_else(|| amount.cast_signed())
            .unsigned_abs()
            .max(1);
        batch.push(StateTransition::new(
            format!("fastpq/fixture/source/{seed}").into_bytes(),
            amount.to_le_bytes().to_vec(),
            0_u64.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            format!("fastpq/fixture/destination/{seed}").into_bytes(),
            0_u64.to_le_bytes().to_vec(),
            destination_amount.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            digest32(format!("transfer-transcripts:{seed}").as_bytes()).to_vec(),
        );
    } else {
        batch.push(StateTransition::new(
            format!("fastpq/fixture/{claim_type}/{seed}").into_bytes(),
            vec![0],
            vec![1],
            OperationKind::MetaSet,
        ));
    }

    batch.metadata.insert(
        "entry_hash".into(),
        decode_hex_digest(&request.source_tx_commitment, "source_tx_commitment")?.to_vec(),
    );
    batch.sort();
    Ok(batch)
}

fn decode_request_batch(encoded: &str) -> Result<TransitionBatch, String> {
    let bytes = BASE64_STANDARD
        .decode(encoded.as_bytes())
        .map_err(|err| format!("invalid batch_base64: {err}"))?;
    if let Ok(dto) = norito::decode_from_bytes::<FastpqTransitionBatch>(&bytes) {
        return Ok(transition_batch_from_model(&dto));
    }
    norito::decode_from_bytes::<TransitionBatch>(&bytes).map_err(|err| {
        format!("failed to decode batch_base64 as FastpqTransitionBatch or TransitionBatch: {err}")
    })
}

fn build_axt_materials(request: &ProofRequest, proof_bytes: &[u8]) -> Result<AxtArtifacts, String> {
    let dsid = DataSpaceId::new(request.source_dsid);
    let (read_key, write_key) = axt_manifest_keys(request);
    let touch_manifest = TouchManifest::from_read_write([read_key], [write_key]);
    let descriptor = AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![AxtTouchSpec {
            dsid,
            read: touch_manifest.read.clone(),
            write: touch_manifest.write.clone(),
        }],
    };
    let manifest_root_hash = Hash::new(
        to_bytes(&touch_manifest).map_err(|err| format!("touch manifest encode failed: {err}"))?,
    );
    let manifest_root_hex = manifest_root_hash.to_string();
    let manifest_root: [u8; 32] = manifest_root_hash.into();
    let fastpq_binding = canonicalize_binding(&request_to_binding(request))
        .map_err(|err| format!("canonical binding failed: {err}"))?;
    let batch = build_batch_from_request(request)?;
    let proof: Proof = norito::decode_from_bytes(proof_bytes)
        .map_err(|err| format!("failed to decode proof bytes for AXT payload: {err}"))?;
    let fastpq_payload = encode_axt_fastpq_payload(&batch, proof)
        .map_err(|err| format!("failed to encode AXT FASTPQ payload: {err}"))?;
    let proof_envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: Some(hex_digest32(
            &batch_manifest_sha256(request),
            "batch_manifest_sha256",
        )?),
        proof: fastpq_payload,
        fastpq_binding: Some(fastpq_binding),
        committed_amount: None,
        amount_commitment: Some(hex_digest32(
            &request.policy_commitment,
            "policy_commitment",
        )?),
    };
    let effect_proof_blob = ProofBlob {
        payload: to_bytes(&proof_envelope)
            .map_err(|err| format!("AXT proof envelope encode failed: {err}"))?,
        expiry_slot: Some(4_294_967_295),
    };
    let relay = build_relay_artifacts(request, manifest_root)?;
    Ok(AxtArtifacts {
        dataspace_id: norito_hex(&dsid)?,
        descriptor: norito_hex(&descriptor)?,
        touch_manifest: norito_hex(&touch_manifest)?,
        effect_proof_blob: norito_hex(&effect_proof_blob)?,
        proof_blob: relay.proof_blob_hex,
        manifest_root: manifest_root_hex,
        relay_envelope: relay.relay_envelope_hex,
        relay_ref: relay.relay_ref,
    })
}

struct RelayArtifacts {
    relay_envelope_hex: String,
    proof_blob_hex: String,
    relay_ref: RelayRefJson,
}

fn build_relay_artifacts(
    request: &ProofRequest,
    manifest_root: [u8; 32],
) -> Result<RelayArtifacts, String> {
    let lane_id = LaneId::new(request.source_lane_id);
    let dataspace_id = DataSpaceId::new(request.source_dsid);
    let block_height = request.relay_block_height.max(1);
    let block_header = BlockHeader::new(
        NonZeroU64::new(block_height)
            .ok_or_else(|| "relay block height must be non-zero".to_string())?,
        None,
        None,
        None,
        block_height.saturating_mul(1_000),
        0,
    );
    let effect_binding = request
        .effect_binding
        .clone()
        .unwrap_or_else(|| EffectBindingRequest {
            destination_domain: Some("lane".to_string()),
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some("source_asset".to_string()),
            destination_asset_definition_id: Some("destination_asset".to_string()),
            source_amount_i64: Some(1),
            destination_amount_i64: Some(1),
        });
    let settlement_commitment = LaneBlockCommitment {
        block_height,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: u128::try_from(effect_binding.source_amount_i64.unwrap_or(1).max(1))
            .unwrap_or(1),
        total_xor_due_micro: u128::try_from(
            effect_binding.destination_amount_i64.unwrap_or(1).max(1),
        )
        .unwrap_or(1),
        total_xor_after_haircut_micro: u128::try_from(
            effect_binding.destination_amount_i64.unwrap_or(1).max(1),
        )
        .unwrap_or(1),
        total_xor_variance_micro: 0,
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let base = LaneRelayEnvelope::new(block_header, None, None, settlement_commitment, 0)
        .map_err(|err| format!("failed to build lane relay envelope: {err}"))?
        .with_manifest_root(Some(manifest_root));
    let proof_blob = build_lane_relay_proof_blob(request, &base, manifest_root)?;
    let envelope = base.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
        proof_digest: Hash::new(proof_blob.payload.as_slice()),
        verified_at_height: block_height,
    }));
    let relay_ref = envelope.relay_ref();
    let relay_envelope_hex = norito_hex(&envelope)?;
    let relay_ref_json = RelayRefJson {
        dataspace_id: relay_ref.dataspace_id.as_u64(),
        lane_id: relay_ref.lane_id.as_u32(),
        block_height: relay_ref.block_height,
        settlement_hash: relay_ref.settlement_hash.to_string(),
    };
    Ok(RelayArtifacts {
        relay_envelope_hex,
        proof_blob_hex: norito_hex(&proof_blob)?,
        relay_ref: relay_ref_json,
    })
}

fn build_lane_relay_proof_blob(
    request: &ProofRequest,
    envelope: &LaneRelayEnvelope,
    manifest_root: [u8; 32],
) -> Result<ProofBlob, String> {
    let relay_ref = envelope.relay_ref();
    let relay_ref_bytes =
        to_bytes(&relay_ref).map_err(|err| format!("lane relay ref encode failed: {err}"))?;
    let source_tx_commitment = digest32_with_domain(
        b"fastpq-json:lane-relay-source-tx:v1",
        &[relay_ref_bytes.as_slice()],
    );
    let claim_digest = lane_relay_fastpq_claim_digest(envelope)
        .map_err(|err| format!("lane relay claim digest failed: {err}"))?;
    let witness_commitment = digest32_with_domain(
        b"fastpq-json:lane-relay-witness:v1",
        &[envelope.settlement_hash.as_ref()],
    );
    let policy_commitment =
        digest32_with_domain(b"fastpq-json:lane-relay-policy:v1", &[&manifest_root]);
    let binding = AxtFastpqBinding {
        parameter: normalized_parameter(&request.parameter),
        source_dsid: request.source_dsid,
        source_dataspace: if request.source_dataspace.trim().is_empty() {
            format!("dataspace-{}", request.source_dsid)
        } else {
            request.source_dataspace.trim().to_string()
        },
        source_receipt_id: format!("relay-{}", hex::encode(&relay_ref_bytes)),
        source_tx_commitment: hex::encode(source_tx_commitment),
        claim_type: "authorization".to_string(),
        claim_digest: claim_digest.to_string(),
        witness_commitment: hex::encode(witness_commitment),
        policy_commitment: hex::encode(policy_commitment),
        verified_effect_type: LANE_RELAY_FASTPQ_EFFECT_TYPE.to_string(),
        corridor: if request.corridor.trim().is_empty() {
            "lane-relay".to_string()
        } else {
            request.corridor.trim().to_string()
        },
        verifier_id: normalized_verifier_id(&request.verifier_id),
        verifier_version: normalized_verifier_version(&request.verifier_version),
        target_dsids: if request.target_dsids.is_empty() {
            vec![request.source_dsid]
        } else {
            request.target_dsids.clone()
        },
        effect_binding: None,
    };
    let mut batch = TransitionBatch::new(
        normalized_parameter(&request.parameter),
        PublicInputs {
            dsid: dsid_bytes(request.source_dsid),
            slot: envelope.block_height.max(1),
            old_root: digest32_with_domain(
                b"fastpq-json:lane-relay-old-root:v1",
                &[relay_ref_bytes.as_slice()],
            ),
            new_root: manifest_root,
            perm_root: digest32_with_domain(
                b"fastpq-json:lane-relay-perm-root:v1",
                &[&manifest_root],
            ),
            tx_set_hash: digest32_with_domain(
                b"fastpq-json:lane-relay-tx-set:v1",
                &[claim_digest.as_ref()],
            ),
        },
    );
    batch.push(StateTransition::new(
        b"axt/nexus/lane-relay".to_vec(),
        relay_ref_bytes,
        claim_digest.as_ref().to_vec(),
        OperationKind::MetaSet,
    ));
    batch.sort();
    batch
        .metadata
        .insert("entry_hash".to_string(), source_tx_commitment.to_vec());
    bind_axt_batch(&mut batch, &binding)
        .map_err(|err| format!("failed to bind lane relay AXT metadata: {err}"))?;
    let prover = Prover::canonical(&request.parameter)
        .map_err(|err| format!("failed to construct lane relay FASTPQ prover: {err}"))?;
    let proof = prover
        .prove(&batch)
        .map_err(|err| format!("lane relay FASTPQ prove failed: {err}"))?;
    verify(&batch, &proof)
        .map_err(|err| format!("lane relay FASTPQ verification failed: {err}"))?;
    let payload = encode_axt_fastpq_payload(&batch, proof)
        .map_err(|err| format!("failed to encode lane relay AXT FASTPQ payload: {err}"))?;
    let proof_envelope = AxtProofEnvelope {
        dsid: DataSpaceId::new(request.source_dsid),
        manifest_root,
        da_commitment: None,
        proof: payload,
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    };
    Ok(ProofBlob {
        payload: to_bytes(&proof_envelope)
            .map_err(|err| format!("lane relay proof envelope encode failed: {err}"))?,
        expiry_slot: Some(4_294_967_295),
    })
}

fn axt_manifest_keys(request: &ProofRequest) -> (String, String) {
    let corridor = if request.corridor.trim().is_empty() {
        "corridor".to_string()
    } else {
        request.corridor.trim().to_string()
    };
    let read_key = format!(
        "{corridor}/read/{}/{}",
        request.claim_type.trim(),
        &request.source_tx_commitment[..16]
    );
    let write_key = format!(
        "{corridor}/write/{}/{}",
        request.verified_effect_type.trim(),
        &request.claim_digest[..16]
    );
    (read_key, write_key)
}

fn norito_hex<T: norito::NoritoSerialize>(value: &T) -> Result<String, String> {
    let bytes = to_bytes(value).map_err(|err| format!("Norito encode failed: {err}"))?;
    Ok(hex::encode(bytes))
}

fn hex_digest32(value: &str, field: &str) -> Result<[u8; 32], String> {
    decode_hex_digest(value, field)
}

fn normalized_parameter(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        AXT_DEFAULT_PARAMETER.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_verifier_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "fastpq".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_verifier_version(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "v1".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_claim_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "authorization" | "compliance" | "tx_predicate" | "value_conservation" => Ok(normalized),
        _ => Err(format!("unsupported claim_type: {value}")),
    }
}

fn decode_hex_digest(value: &str, field: &str) -> Result<[u8; 32], String> {
    let normalized = value.trim().to_ascii_lowercase();
    let bytes = hex::decode(&normalized).map_err(|err| format!("{field} must be hex: {err}"))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{field} must be 32 bytes of hex"))?;
    Ok(array)
}

fn digest32(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut output = [0_u8; 32];
    output.copy_from_slice(&digest);
    output
}

fn digest32_with_domain(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut output = [0_u8; 32];
    output.copy_from_slice(&digest);
    output
}

fn dsid_bytes(source_dsid: u64) -> [u8; 16] {
    let mut output = [0_u8; 16];
    output[..8].copy_from_slice(&DataSpaceId::new(source_dsid).as_u64().to_le_bytes());
    output
}

fn batch_manifest_sha256(request: &ProofRequest) -> String {
    axt_batch_manifest_sha256(&request_to_binding(request))
        .unwrap_or_else(|err| panic!("batch manifest sha256 failed: {err}"))
}

fn request_to_binding(request: &ProofRequest) -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: normalized_parameter(&request.parameter),
        source_dsid: request.source_dsid,
        source_dataspace: if request.source_dataspace.trim().is_empty() {
            "restricted".to_string()
        } else {
            request.source_dataspace.trim().to_string()
        },
        source_receipt_id: if request.source_receipt_id.trim().is_empty() {
            sha256_hex(request.source_tx_commitment.as_bytes())
        } else {
            request.source_receipt_id.trim().to_string()
        },
        source_tx_commitment: request.source_tx_commitment.trim().to_ascii_lowercase(),
        claim_type: request.claim_type.trim().to_ascii_lowercase(),
        claim_digest: request.claim_digest.trim().to_ascii_lowercase(),
        witness_commitment: request.witness_commitment.trim().to_ascii_lowercase(),
        policy_commitment: request.policy_commitment.trim().to_ascii_lowercase(),
        verified_effect_type: request.verified_effect_type.trim().to_string(),
        corridor: request.corridor.trim().to_string(),
        verifier_id: normalized_verifier_id(&request.verifier_id),
        verifier_version: normalized_verifier_version(&request.verifier_version),
        target_dsids: request.target_dsids.clone(),
        effect_binding: request.effect_binding.as_ref().map(effect_binding_to_model),
    }
}

fn effect_binding_to_model(binding: &EffectBindingRequest) -> AxtEffectBinding {
    AxtEffectBinding {
        destination_domain: binding.destination_domain.clone(),
        destination_account_id: binding.destination_account_id.clone(),
        vault_account_id: binding.vault_account_id.clone(),
        issuance_account_id: binding.issuance_account_id.clone(),
        source_asset_definition_id: binding.source_asset_definition_id.clone(),
        destination_asset_definition_id: binding.destination_asset_definition_id.clone(),
        source_amount_i64: binding.source_amount_i64,
        destination_amount_i64: binding.destination_amount_i64,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    sha256_hex_raw(bytes)
}

fn sha256_hex_raw(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn percentile_usize(sorted_values: &[usize], numerator: usize, denominator: usize) -> usize {
    if sorted_values.is_empty() {
        return 0;
    }
    let index = percentile_index(sorted_values.len(), numerator, denominator);
    sorted_values[index]
}

fn percentile_f64(sorted_values: &[f64], numerator: usize, denominator: usize) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let index = percentile_index(sorted_values.len(), numerator, denominator);
    sorted_values[index]
}

fn percentile_index(len: usize, numerator: usize, denominator: usize) -> usize {
    let max_index = len.saturating_sub(1);
    if max_index == 0 || denominator == 0 {
        return 0;
    }
    let scaled = max_index.saturating_mul(numerator);
    scaled
        .saturating_add(denominator / 2)
        .checked_div(denominator)
        .unwrap_or(0)
        .min(max_index)
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_batch_base64() -> String {
        let batch = TransitionBatch::new(
            AXT_DEFAULT_PARAMETER.to_string(),
            PublicInputs {
                dsid: [0; 16],
                slot: 1,
                old_root: [1; 32],
                new_root: [2; 32],
                perm_root: [3; 32],
                tx_set_hash: [4; 32],
            },
        );
        BASE64_STANDARD.encode(to_bytes(&batch).expect("encode transition batch"))
    }

    #[test]
    fn inspect_transfers_reports_absent_metadata() {
        let output = handle_inspect_transfers(InspectTransfersInput {
            batch_base64: empty_batch_base64(),
            source_account_id: None,
            destination_account_id: None,
            asset_definition_id: None,
        })
        .expect("inspect transfers");

        assert!(!output.transcripts_present);
        assert_eq!(output.transfer_count, 0);
        assert!(output.transfers.is_empty());
    }

    #[test]
    fn trimmed_filter_drops_empty_values() {
        assert_eq!(trimmed_filter(Some("  ".into())), None);
        assert_eq!(
            trimmed_filter(Some("  alice  ".into())),
            Some("alice".into())
        );
        assert_eq!(trimmed_filter(None), None);
    }
}
