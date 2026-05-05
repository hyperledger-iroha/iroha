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
use fastpq_prover::{
    AXT_DEFAULT_PARAMETER, Proof, Prover, TransitionBatch,
    batch_manifest_sha256 as axt_batch_manifest_sha256, build_batch_from_binding,
    canonicalize_binding, verify,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    nexus::{
        AxtDescriptor, AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, AxtTouchSpec,
        LaneFastpqProofMaterial, LaneId, LaneRelayEnvelope, ProofBlob, TouchManifest,
    },
};
use norito::to_bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DEFAULT_SAMPLE_COUNT: usize = 7;

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
}

#[derive(Debug, Clone, Deserialize)]
struct MeasureInput {
    #[serde(default)]
    dataspace: String,
    #[serde(default)]
    source_dsid: u64,
    #[serde(default)]
    verifier_id: String,
    #[serde(default)]
    verifier_version: String,
    #[serde(default = "default_sample_count")]
    sample_count: usize,
    claim_types: Vec<String>,
    #[serde(default)]
    fixtures: Vec<ProofRequest>,
    #[serde(default)]
    parameter: String,
}

#[derive(Debug, Clone, Serialize)]
struct MeasureOutput {
    dataspace: String,
    measurement_mode: String,
    sample_count: usize,
    parameter: String,
    benchmarks: BTreeMap<String, BenchmarkResult>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Deserialize)]
struct ProofRequest {
    #[serde(default)]
    parameter: String,
    source_dsid: u64,
    #[serde(default)]
    source_dataspace: String,
    #[serde(default)]
    source_receipt_id: String,
    #[serde(default)]
    target_dsids: Vec<u64>,
    source_tx_commitment: String,
    claim_type: String,
    claim_digest: String,
    witness_commitment: String,
    policy_commitment: String,
    verified_effect_type: String,
    #[serde(default)]
    corridor: String,
    #[serde(default)]
    verifier_id: String,
    #[serde(default)]
    verifier_version: String,
    #[serde(default)]
    source_lane_id: u32,
    #[serde(default = "default_relay_block_height")]
    relay_block_height: u64,
    #[serde(default)]
    effect_binding: Option<EffectBindingRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct EffectBindingRequest {
    #[serde(default)]
    destination_domain: Option<String>,
    #[serde(default)]
    destination_account_id: Option<String>,
    #[serde(default)]
    vault_account_id: Option<String>,
    #[serde(default)]
    issuance_account_id: Option<String>,
    #[serde(default)]
    source_asset_definition_id: Option<String>,
    #[serde(default)]
    destination_asset_definition_id: Option<String>,
    #[serde(default)]
    source_amount_i64: Option<i64>,
    #[serde(default)]
    destination_amount_i64: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
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
    proof_blob_hex: String,
    manifest_root_hex: String,
    relay_envelope_hex: String,
    relay_ref: RelayRefJson,
}

#[derive(Debug, Clone, Serialize)]
struct RelayRefJson {
    dataspace_id: u64,
    lane_id: u32,
    block_height: u64,
    settlement_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VerifyInput {
    request: ProofRequest,
    proof_bytes_base64: String,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyResponse {
    passed: bool,
    parameter: String,
    proof_sha256: String,
    proof_bytes_len: usize,
    verify_ms: f64,
    trace_commitment: String,
    batch_manifest_sha256: String,
}

fn default_sample_count() -> usize {
    DEFAULT_SAMPLE_COUNT
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
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn print_json<T: Serialize>(payload: &T) -> Result<(), String> {
    let encoded = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("json encode failed: {err}"))?;
    println!("{encoded}");
    Ok(())
}

fn handle_measure(request: MeasureInput) -> Result<MeasureOutput, String> {
    let parameter = normalized_parameter(&request.parameter);
    let verifier_id = normalized_verifier_id(&request.verifier_id);
    let verifier_version = normalized_verifier_version(&request.verifier_version);
    let sample_count = request.sample_count.max(1);
    let fixture_mode = !request.fixtures.is_empty();
    let mut benchmarks = BTreeMap::new();
    for claim_type in &request.claim_types {
        let claim_type = normalized_claim_type(claim_type)?;
        let mut proof_sizes = Vec::with_capacity(sample_count);
        let mut prove_ms = Vec::with_capacity(sample_count);
        let mut verify_ms = Vec::with_capacity(sample_count);
        let proof_requests: Vec<ProofRequest> = if fixture_mode {
            request
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
                .collect()
        } else {
            (0..sample_count)
                .map(|sample_index| {
                    synthetic_proof_request(
                        &parameter,
                        request.source_dsid,
                        &claim_type,
                        &request.dataspace,
                        sample_index,
                        &verifier_id,
                        &verifier_version,
                    )
                })
                .collect()
        };
        if proof_requests.is_empty() {
            return Err(format!(
                "measure input is missing maintained fixtures for claim_type: {claim_type}"
            ));
        }
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
        measurement_mode: if fixture_mode {
            "fastpq_prover_fixture_replay".to_string()
        } else {
            "fastpq_prover".to_string()
        },
        sample_count: if fixture_mode {
            request.fixtures.len().max(1)
        } else {
            sample_count
        },
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
    build_batch_from_binding(&binding).map_err(|err| err.to_string())
}

fn synthetic_proof_request(
    parameter: &str,
    source_dsid: u64,
    claim_type: &str,
    dataspace: &str,
    sample_index: usize,
    verifier_id: &str,
    verifier_version: &str,
) -> ProofRequest {
    let seed = format!("{dataspace}:{claim_type}:{sample_index}");
    ProofRequest {
        parameter: parameter.to_string(),
        source_dsid,
        source_dataspace: dataspace.to_string(),
        source_receipt_id: sha256_hex(format!("{seed}:receipt").as_bytes()),
        target_dsids: vec![source_dsid.saturating_add(1)],
        source_tx_commitment: sha256_hex(seed.as_bytes()),
        claim_type: claim_type.to_string(),
        claim_digest: sha256_hex(format!("{seed}:claim").as_bytes()),
        witness_commitment: sha256_hex(format!("{seed}:witness").as_bytes()),
        policy_commitment: sha256_hex(format!("{seed}:policy").as_bytes()),
        verified_effect_type: "receipt_bound_effect".to_string(),
        corridor: dataspace.to_string(),
        verifier_id: verifier_id.to_string(),
        verifier_version: verifier_version.to_string(),
        source_lane_id: u32::try_from(source_dsid).unwrap_or_default(),
        relay_block_height: u64::try_from(sample_index)
            .unwrap_or_default()
            .saturating_add(1),
        effect_binding: Some(EffectBindingRequest {
            destination_domain: Some("lane".to_string()),
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(format!("source_asset_{sample_index}")),
            destination_asset_definition_id: Some(format!("destination_asset_{sample_index}")),
            source_amount_i64: Some(50_000 + i64::try_from(sample_index).unwrap_or_default()),
            destination_amount_i64: Some(50_000 + i64::try_from(sample_index).unwrap_or_default()),
        }),
    }
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
    let proof_envelope = AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: Some(hex_digest32(
            &batch_manifest_sha256(request),
            "batch_manifest_sha256",
        )?),
        proof: proof_bytes.to_vec(),
        fastpq_binding: Some(fastpq_binding),
        committed_amount: None,
        amount_commitment: Some(hex_digest32(
            &request.policy_commitment,
            "policy_commitment",
        )?),
    };
    let proof_blob = ProofBlob {
        payload: to_bytes(&proof_envelope)
            .map_err(|err| format!("AXT proof envelope encode failed: {err}"))?,
        expiry_slot: Some(4_294_967_295),
    };
    let relay = build_relay_artifacts(request, manifest_root, proof_blob.payload.as_slice())?;
    Ok(AxtArtifacts {
        dataspace_id: norito_hex(&dsid)?,
        descriptor: norito_hex(&descriptor)?,
        touch_manifest: norito_hex(&touch_manifest)?,
        proof_blob: norito_hex(&proof_blob)?,
        manifest_root: manifest_root_hex,
        relay_envelope: relay.0,
        relay_ref: relay.1,
    })
}

fn build_relay_artifacts(
    request: &ProofRequest,
    manifest_root: [u8; 32],
    proof_payload: &[u8],
) -> Result<(String, RelayRefJson), String> {
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
    };
    let base = LaneRelayEnvelope::new(block_header, None, None, settlement_commitment, 0)
        .map_err(|err| format!("failed to build lane relay envelope: {err}"))?
        .with_manifest_root(Some(manifest_root));
    let proof_digest = base.expected_fastpq_proof_digest(Some(block_height));
    let envelope = base.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
        proof_digest,
        verified_at_height: Some(block_height),
    }));
    let relay_ref = envelope.relay_ref();
    let relay_envelope_hex = norito_hex(&envelope)?;
    let relay_ref_json = RelayRefJson {
        dataspace_id: relay_ref.dataspace_id.as_u64(),
        lane_id: relay_ref.lane_id.as_u32(),
        block_height: relay_ref.block_height,
        settlement_hash: relay_ref.settlement_hash.to_string(),
    };
    let _ = proof_payload;
    Ok((relay_envelope_hex, relay_ref_json))
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
