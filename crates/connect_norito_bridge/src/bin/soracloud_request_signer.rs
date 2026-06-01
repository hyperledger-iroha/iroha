//! Build exact Soracloud uploaded-model and private-runtime request payloads for desktop clients.

use std::{
    env, fs,
    io::{self, Read as _},
    num::NonZeroU32,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{ExposedPrivateKey, Hash, PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SECRET_ENVELOPE_VERSION_V1, SORA_PRIVATE_COMPILE_PROFILE_VERSION_V1,
        SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1, SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1,
        SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1, SORA_UPLOADED_MODEL_CHUNK_VERSION_V1,
        SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1, SecretEnvelopeEncryptionV1, SecretEnvelopeV1,
        SoraModelPrivacyModeV1, SoraPrivateCompileProfileV1, SoraPrivateInferenceCheckpointV1,
        SoraPrivateInferenceSessionStatusV1, SoraPrivateInferenceSessionV1,
        SoraUploadedModelBundleV1, SoraUploadedModelChunkV1,
        SoraUploadedModelEncryptionRecipientV1, SoraUploadedModelKeyEncapsulationV1,
        SoraUploadedModelKeyWrapAeadV1, SoraUploadedModelPricingPolicyV1,
        SoraUploadedModelRuntimeFormatV1, SoraUploadedModelWrappedKeyV1,
        encode_private_compile_profile_provenance_payload,
        encode_private_inference_checkpoint_provenance_payload,
        encode_private_inference_output_release_provenance_payload,
        encode_private_inference_start_provenance_payload,
        encode_uploaded_model_allow_provenance_payload,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_chunk_append_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
    },
};
use norito::{json, to_bytes};

#[derive(Debug, norito::json::JsonDeserialize)]
struct SignUploadInput {
    manifest_path: String,
    authority: String,
    private_key: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct SignUploadChunkInput {
    manifest_path: String,
    ordinal: u32,
    authority: String,
    private_key: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct SignPrivateRunInput {
    authority: String,
    private_key: String,
    allow: StageAllowInput,
    run: StageRunInput,
    checkpoint: StageCheckpointInput,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct SignPrivateOutputReleaseInput {
    authority: String,
    private_key: String,
    session_id: String,
    decrypt_request_id: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
#[allow(dead_code)]
struct StageUploadManifest {
    #[norito(default)]
    schema_version: u16,
    #[norito(default)]
    label: Option<String>,
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    family: String,
    modalities: Vec<String>,
    runtime_format: String,
    privacy_mode: String,
    plaintext_root: String,
    #[norito(default)]
    source_path: Option<String>,
    upload_recipient: StageUploadRecipient,
    wrapped_bundle_key: StageWrappedBundleKey,
    decryption_policy_ref: String,
    pricing_policy: StagePricingPolicy,
    compile_profile: StageCompileProfile,
    finalize: StageFinalize,
    chunks: Vec<StageChunk>,
    #[norito(default)]
    created_at: Option<String>,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StagePricingPolicy {
    storage_xor_nanos: u128,
    compile_xor_nanos: u128,
    runtime_step_xor_nanos: u128,
    decrypt_release_xor_nanos: u128,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageCompileProfile {
    family: String,
    quantization: String,
    opset_version: String,
    max_context: u32,
    max_images: u16,
    vision_patch_policy: String,
    fhe_param_set: String,
    execution_policy: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageFinalize {
    dataset_ref: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageChunk {
    ordinal: u32,
    offset_bytes: u64,
    plaintext_len: u32,
    key_id: String,
    key_version: u32,
    nonce_base64: String,
    ciphertext_path: String,
    #[norito(default)]
    aad_seed: Option<String>,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageUploadRecipient {
    key_id: String,
    key_version: u32,
    kem: String,
    aead: String,
    public_key_bytes_base64: String,
    public_key_fingerprint: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageWrappedBundleKey {
    recipient_key_id: String,
    recipient_key_version: u32,
    kem: String,
    aead: String,
    ephemeral_public_key_base64: String,
    nonce_base64: String,
    wrapped_key_ciphertext_base64: String,
    aad_digest: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageAllowInput {
    apartment_name: String,
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: String,
    compile_profile_hash: String,
    privacy_mode: String,
    require_model_inference: bool,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageRunInput {
    session_id: String,
    apartment: String,
    service_name: String,
    model_id: String,
    weight_version: String,
    bundle_root: String,
    input_commitment_seeds: Vec<String>,
    token_budget: u32,
    image_budget: u16,
    initial_status: String,
    initial_receipt_seed: String,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct StageCheckpointInput {
    step: u32,
    status: String,
    ciphertext_state_seed: String,
    receipt_seed: String,
    decrypt_request_id: String,
    #[norito(default)]
    released_token: Option<String>,
    compute_units: u64,
    updated_at_ms: u64,
    xor_cost_nanos: u128,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadedModelBundleInitPayload {
    bundle: SoraUploadedModelBundleV1,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedUploadedModelBundleInitRequest {
    payload: UploadedModelBundleInitPayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadedModelChunkPayload {
    chunk: SoraUploadedModelChunkV1,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedUploadedModelChunkRequest {
    payload: UploadedModelChunkPayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadedModelFinalizePayload {
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: Hash,
    privacy_mode: SoraModelPrivacyModeV1,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedUploadedModelFinalizeRequest {
    payload: UploadedModelFinalizePayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateCompilePayload {
    service_name: String,
    model_id: String,
    weight_version: String,
    bundle_root: Hash,
    compile_profile: SoraPrivateCompileProfileV1,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedPrivateCompileRequest {
    payload: PrivateCompilePayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadedModelAllowPayload {
    apartment_name: String,
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: Hash,
    compile_profile_hash: Hash,
    privacy_mode: SoraModelPrivacyModeV1,
    require_model_inference: bool,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedUploadedModelAllowRequest {
    payload: UploadedModelAllowPayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateInferenceRunPayload {
    session: SoraPrivateInferenceSessionV1,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedPrivateInferenceRunRequest {
    payload: PrivateInferenceRunPayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateInferenceCheckpointPayload {
    session_id: String,
    status: SoraPrivateInferenceSessionStatusV1,
    receipt_root: Hash,
    xor_cost_nanos: u128,
    checkpoint: SoraPrivateInferenceCheckpointV1,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedPrivateInferenceCheckpointRequest {
    payload: PrivateInferenceCheckpointPayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateInferenceOutputReleasePayload {
    session_id: String,
    decrypt_request_id: String,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct SignedPrivateInferenceOutputReleaseRequest {
    payload: PrivateInferenceOutputReleasePayload,
    provenance: ManifestProvenance,
    authority: Option<AccountId>,
    private_key: Option<ExposedPrivateKey>,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadInitOutput {
    bundle_root: Hash,
    compile_profile_hash: Hash,
    chunk_manifest_root: Hash,
    chunk_count: u32,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    request: SignedUploadedModelBundleInitRequest,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadChunkOutput {
    ordinal: u32,
    request: SignedUploadedModelChunkRequest,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadFinalizeOutput {
    bundle_root: Hash,
    compile_profile_hash: Hash,
    chunk_manifest_root: Hash,
    weight_artifact_hash: Hash,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    finalize_request: SignedUploadedModelFinalizeRequest,
    compile_request: SignedPrivateCompileRequest,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateRunOutput {
    session_id: String,
    initial_receipt_root: Hash,
    checkpoint_receipt_root: Hash,
    allow_request: SignedUploadedModelAllowRequest,
    run_request: SignedPrivateInferenceRunRequest,
    checkpoint_request: SignedPrivateInferenceCheckpointRequest,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct PrivateOutputReleaseOutput {
    session_id: String,
    decrypt_request_id: String,
    request: SignedPrivateInferenceOutputReleaseRequest,
}

struct MutationSigner {
    authority: AccountId,
    private_key: ExposedPrivateKey,
    public_key: PublicKey,
}

type UploadedModelBundleRootTuple<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    Vec<String>,
    Hash,
    SoraUploadedModelRuntimeFormatV1,
    u64,
    u64,
    Hash,
    Hash,
    SoraUploadedModelEncryptionRecipientV1,
    SoraUploadedModelWrappedKeyV1,
    SoraUploadedModelPricingPolicyV1,
    &'a str,
);

/// Canonical preimage for uploaded-model bundle roots.
///
/// This keeps the historical flat-tuple field order without depending on
/// Norito's current tuple arity limit.
struct UploadedModelBundleRootPayload<'a> {
    service_name: &'a str,
    model_id: &'a str,
    weight_version: &'a str,
    family: &'a str,
    modalities: &'a Vec<String>,
    plaintext_root: Hash,
    runtime_format: SoraUploadedModelRuntimeFormatV1,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    compile_profile_hash: Hash,
    chunk_manifest_root: Hash,
    upload_recipient: &'a SoraUploadedModelEncryptionRecipientV1,
    wrapped_bundle_key: &'a SoraUploadedModelWrappedKeyV1,
    pricing_policy: &'a SoraUploadedModelPricingPolicyV1,
    decryption_policy_ref: &'a str,
}

impl norito::core::NoritoSerialize for UploadedModelBundleRootPayload<'_> {
    fn schema_hash() -> [u8; 16]
    where
        Self: Sized,
    {
        norito::core::type_name_schema_hash::<UploadedModelBundleRootTuple<'static>>()
    }

    fn serialize<W: io::Write>(&self, mut writer: W) -> Result<(), norito::Error> {
        let current = norito::core::get_decode_flags();
        let defaults = norito::core::default_encode_flags();
        let dynamic_mask = norito::core::header_flags::PACKED_SEQ;
        let static_defaults = defaults & !dynamic_mask;
        let merged = if current == 0 {
            defaults
        } else {
            let current_dynamic = current & dynamic_mask;
            let current_static = current & !dynamic_mask;
            let effective_static = if current_static == 0 {
                static_defaults
            } else {
                current_static | static_defaults
            };
            current_dynamic | effective_static
        };
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(merged, merged);

        serialize_tuple_field(&mut writer, &self.service_name)?;
        serialize_tuple_field(&mut writer, &self.model_id)?;
        serialize_tuple_field(&mut writer, &self.weight_version)?;
        serialize_tuple_field(&mut writer, &self.family)?;
        serialize_tuple_field(&mut writer, self.modalities)?;
        serialize_tuple_field(&mut writer, &self.plaintext_root)?;
        serialize_tuple_field(&mut writer, &self.runtime_format)?;
        serialize_tuple_field(&mut writer, &self.plaintext_bytes)?;
        serialize_tuple_field(&mut writer, &self.ciphertext_bytes)?;
        serialize_tuple_field(&mut writer, &self.compile_profile_hash)?;
        serialize_tuple_field(&mut writer, &self.chunk_manifest_root)?;
        serialize_tuple_field(&mut writer, self.upload_recipient)?;
        serialize_tuple_field(&mut writer, self.wrapped_bundle_key)?;
        serialize_tuple_field(&mut writer, self.pricing_policy)?;
        serialize_tuple_field(&mut writer, &self.decryption_policy_ref)?;

        Ok(())
    }
}

struct DerivedUploadBundle {
    bundle: SoraUploadedModelBundleV1,
    compile_profile: SoraPrivateCompileProfileV1,
    chunks: Vec<SoraUploadedModelChunkV1>,
    bundle_root: Hash,
    compile_profile_hash: Hash,
    chunk_manifest_root: Hash,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    weight_artifact_hash: Hash,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err("usage: soracloud_request_signer <sign-upload-init|sign-upload-chunk|sign-upload-finalize|sign-private-run|sign-private-output-release>".to_string());
    };

    match command.as_str() {
        "sign-upload-init" => {
            let input: SignUploadInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let manifest = load_stage_manifest(&input.manifest_path)?;
            let derived = derive_upload_bundle(&manifest)?;
            let payload_bytes =
                encode_uploaded_model_bundle_register_provenance_payload(derived.bundle.clone())
                    .map_err(|err| format!("failed to encode upload init payload: {err}"))?;
            let request = SignedUploadedModelBundleInitRequest {
                payload: UploadedModelBundleInitPayload {
                    bundle: derived.bundle.clone(),
                },
                provenance: signer.provenance(&payload_bytes),
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };
            write_stdout_json(&UploadInitOutput {
                bundle_root: derived.bundle_root,
                compile_profile_hash: derived.compile_profile_hash,
                chunk_manifest_root: derived.chunk_manifest_root,
                chunk_count: u32::try_from(derived.chunks.len()).unwrap_or(u32::MAX),
                plaintext_bytes: derived.plaintext_bytes,
                ciphertext_bytes: derived.ciphertext_bytes,
                request,
            })
        }
        "sign-upload-chunk" => {
            let input: SignUploadChunkInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let manifest = load_stage_manifest(&input.manifest_path)?;
            let derived = derive_upload_bundle(&manifest)?;
            let chunk = derived
                .chunks
                .iter()
                .find(|chunk| chunk.ordinal == input.ordinal)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "chunk ordinal {} not found in stage manifest",
                        input.ordinal
                    )
                })?;
            let payload_bytes =
                encode_uploaded_model_chunk_append_provenance_payload(chunk.clone())
                    .map_err(|err| format!("failed to encode upload chunk payload: {err}"))?;
            let request = SignedUploadedModelChunkRequest {
                payload: UploadedModelChunkPayload { chunk },
                provenance: signer.provenance(&payload_bytes),
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };
            write_stdout_json(&UploadChunkOutput {
                ordinal: input.ordinal,
                request,
            })
        }
        "sign-upload-finalize" => {
            let input: SignUploadInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let manifest = load_stage_manifest(&input.manifest_path)?;
            let derived = derive_upload_bundle(&manifest)?;
            let privacy_mode = parse_privacy_mode(&manifest.privacy_mode);

            let finalize_payload = UploadedModelFinalizePayload {
                service_name: manifest.service_name.clone(),
                model_name: manifest.model_name.clone(),
                model_id: manifest.model_id.clone(),
                artifact_id: manifest.artifact_id.clone(),
                weight_version: manifest.weight_version.clone(),
                bundle_root: derived.bundle_root,
                privacy_mode,
                weight_artifact_hash: derived.weight_artifact_hash,
                dataset_ref: manifest.finalize.dataset_ref.clone(),
                training_config_hash: derived.training_config_hash,
                reproducibility_hash: derived.reproducibility_hash,
                provenance_attestation_hash: derived.provenance_attestation_hash,
            };
            let finalize_bytes = encode_uploaded_model_finalize_provenance_payload(
                finalize_payload.service_name.as_str(),
                finalize_payload.model_name.as_str(),
                finalize_payload.model_id.as_str(),
                finalize_payload.artifact_id.as_str(),
                finalize_payload.weight_version.as_str(),
                finalize_payload.bundle_root,
                finalize_payload.privacy_mode,
                finalize_payload.weight_artifact_hash,
                finalize_payload.dataset_ref.as_str(),
                finalize_payload.training_config_hash,
                finalize_payload.reproducibility_hash,
                finalize_payload.provenance_attestation_hash,
            )
            .map_err(|err| format!("failed to encode upload finalize payload: {err}"))?;
            let finalize_request = SignedUploadedModelFinalizeRequest {
                payload: finalize_payload,
                provenance: signer.provenance(&finalize_bytes),
                authority: Some(signer.authority.clone()),
                private_key: Some(signer.private_key.clone()),
            };

            let compile_payload = PrivateCompilePayload {
                service_name: manifest.service_name.clone(),
                model_id: manifest.model_id.clone(),
                weight_version: manifest.weight_version.clone(),
                bundle_root: derived.bundle_root,
                compile_profile: derived.compile_profile.clone(),
            };
            let compile_bytes = encode_private_compile_profile_provenance_payload(
                compile_payload.service_name.as_str(),
                compile_payload.model_id.as_str(),
                compile_payload.weight_version.as_str(),
                compile_payload.bundle_root,
                compile_payload.compile_profile.clone(),
            )
            .map_err(|err| format!("failed to encode private compile payload: {err}"))?;
            let compile_request = SignedPrivateCompileRequest {
                payload: compile_payload,
                provenance: signer.provenance(&compile_bytes),
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };

            write_stdout_json(&UploadFinalizeOutput {
                bundle_root: derived.bundle_root,
                compile_profile_hash: derived.compile_profile_hash,
                chunk_manifest_root: derived.chunk_manifest_root,
                weight_artifact_hash: derived.weight_artifact_hash,
                training_config_hash: derived.training_config_hash,
                reproducibility_hash: derived.reproducibility_hash,
                provenance_attestation_hash: derived.provenance_attestation_hash,
                finalize_request,
                compile_request,
            })
        }
        "sign-private-run" => {
            let input: SignPrivateRunInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let allow_privacy_mode = parse_privacy_mode(&input.allow.privacy_mode);
            let bundle_root = parse_hash_like(&input.allow.bundle_root);
            let compile_profile_hash = parse_hash_like(&input.allow.compile_profile_hash);

            let allow_payload = UploadedModelAllowPayload {
                apartment_name: input.allow.apartment_name.clone(),
                service_name: input.allow.service_name.clone(),
                model_name: input.allow.model_name.clone(),
                model_id: input.allow.model_id.clone(),
                artifact_id: input.allow.artifact_id.clone(),
                weight_version: input.allow.weight_version.clone(),
                bundle_root,
                compile_profile_hash,
                privacy_mode: allow_privacy_mode,
                require_model_inference: input.allow.require_model_inference,
            };
            let allow_bytes = encode_uploaded_model_allow_provenance_payload(
                allow_payload.apartment_name.as_str(),
                allow_payload.service_name.as_str(),
                allow_payload.model_name.as_str(),
                allow_payload.model_id.as_str(),
                allow_payload.artifact_id.as_str(),
                allow_payload.weight_version.as_str(),
                allow_payload.bundle_root,
                allow_payload.compile_profile_hash,
                allow_payload.privacy_mode,
                allow_payload.require_model_inference,
            )
            .map_err(|err| format!("failed to encode uploaded model allow payload: {err}"))?;
            let allow_request = SignedUploadedModelAllowRequest {
                payload: allow_payload,
                provenance: signer.provenance(&allow_bytes),
                authority: Some(signer.authority.clone()),
                private_key: Some(signer.private_key.clone()),
            };

            let initial_receipt_root = Hash::new(input.run.initial_receipt_seed.as_bytes());
            let run_status = parse_private_session_status(&input.run.initial_status);
            let run_payload = PrivateInferenceRunPayload {
                session: SoraPrivateInferenceSessionV1 {
                    schema_version: SORA_PRIVATE_INFERENCE_SESSION_VERSION_V1,
                    session_id: input.run.session_id.clone(),
                    apartment: parse_name(&input.run.apartment, "run.apartment")?,
                    service_name: parse_name(&input.run.service_name, "run.service_name")?,
                    model_id: input.run.model_id.clone(),
                    weight_version: input.run.weight_version.clone(),
                    bundle_root: parse_hash_like(&input.run.bundle_root),
                    input_commitments: input
                        .run
                        .input_commitment_seeds
                        .iter()
                        .map(|seed| Hash::new(seed.as_bytes()))
                        .collect(),
                    token_budget: input.run.token_budget,
                    image_budget: input.run.image_budget,
                    status: run_status,
                    receipt_root: initial_receipt_root,
                    xor_cost_nanos: 0,
                },
            };
            let run_bytes =
                encode_private_inference_start_provenance_payload(run_payload.session.clone())
                    .map_err(|err| {
                        format!("failed to encode private inference run payload: {err}")
                    })?;
            let run_request = SignedPrivateInferenceRunRequest {
                payload: run_payload,
                provenance: signer.provenance(&run_bytes),
                authority: Some(signer.authority.clone()),
                private_key: Some(signer.private_key.clone()),
            };

            let checkpoint_status = parse_private_session_status(&input.checkpoint.status);
            let checkpoint_receipt_root = Hash::new(input.checkpoint.receipt_seed.as_bytes());
            let checkpoint = SoraPrivateInferenceCheckpointV1 {
                schema_version: SORA_PRIVATE_INFERENCE_CHECKPOINT_VERSION_V1,
                session_id: input.run.session_id.clone(),
                step: input.checkpoint.step,
                ciphertext_state_root: Hash::new(input.checkpoint.ciphertext_state_seed.as_bytes()),
                receipt_hash: Hash::new(
                    format!(
                        "{}:{}:{}",
                        input.run.session_id, input.checkpoint.step, input.checkpoint.receipt_seed
                    )
                    .as_bytes(),
                ),
                decrypt_request_id: input.checkpoint.decrypt_request_id.clone(),
                released_token: input.checkpoint.released_token.clone(),
                compute_units: input.checkpoint.compute_units,
                updated_at_ms: input.checkpoint.updated_at_ms,
            };
            let checkpoint_bytes = encode_private_inference_checkpoint_provenance_payload(
                input.run.session_id.as_str(),
                checkpoint_status,
                checkpoint_receipt_root,
                input.checkpoint.xor_cost_nanos,
                checkpoint.clone(),
            )
            .map_err(|err| {
                format!("failed to encode private inference checkpoint payload: {err}")
            })?;
            let checkpoint_request = SignedPrivateInferenceCheckpointRequest {
                payload: PrivateInferenceCheckpointPayload {
                    session_id: input.run.session_id.clone(),
                    status: checkpoint_status,
                    receipt_root: checkpoint_receipt_root,
                    xor_cost_nanos: input.checkpoint.xor_cost_nanos,
                    checkpoint,
                },
                provenance: signer.provenance(&checkpoint_bytes),
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };

            write_stdout_json(&PrivateRunOutput {
                session_id: input.run.session_id,
                initial_receipt_root,
                checkpoint_receipt_root,
                allow_request,
                run_request,
                checkpoint_request,
            })
        }
        "sign-private-output-release" => {
            let input: SignPrivateOutputReleaseInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let payload = PrivateInferenceOutputReleasePayload {
                session_id: input.session_id.clone(),
                decrypt_request_id: input.decrypt_request_id.clone(),
            };
            let payload_bytes = encode_private_inference_output_release_provenance_payload(
                payload.session_id.as_str(),
                payload.decrypt_request_id.as_str(),
            )
            .map_err(|err| {
                format!("failed to encode private inference output release payload: {err}")
            })?;
            let request = SignedPrivateInferenceOutputReleaseRequest {
                payload,
                provenance: signer.provenance(&payload_bytes),
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };
            write_stdout_json(&PrivateOutputReleaseOutput {
                session_id: input.session_id,
                decrypt_request_id: input.decrypt_request_id,
                request,
            })
        }
        other => Err(format!("unsupported command `{other}`")),
    }
}

fn read_stdin_json<T>() -> Result<T, String>
where
    T: norito::json::JsonDeserialize,
{
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| format!("failed to read stdin: {err}"))?;
    json::from_str(&raw).map_err(|err| format!("failed to parse stdin JSON: {err}"))
}

fn write_stdout_json<T>(value: &T) -> Result<(), String>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    let json_value =
        json::to_value(value).map_err(|err| format!("failed to encode JSON value: {err}"))?;
    let output =
        json::to_string(&json_value).map_err(|err| format!("failed to encode JSON: {err}"))?;
    println!("{output}");
    Ok(())
}

fn parse_signer(authority: &str, private_key: &str) -> Result<MutationSigner, String> {
    let authority = AccountId::parse_encoded(authority)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| format!("invalid authority account id: {err}"))?;
    let private_key = private_key
        .parse::<ExposedPrivateKey>()
        .map_err(|err| format!("invalid private key: {err}"))?;
    let public_key: PublicKey = private_key.0.clone().into();
    Ok(MutationSigner {
        authority,
        private_key,
        public_key,
    })
}

impl MutationSigner {
    fn provenance(&self, payload: &[u8]) -> ManifestProvenance {
        ManifestProvenance {
            signer: self.public_key.clone(),
            signature: Signature::new(&self.private_key.0, payload),
        }
    }
}

fn load_stage_manifest(manifest_path: &str) -> Result<StageUploadManifest, String> {
    let raw = fs::read_to_string(manifest_path)
        .map_err(|err| format!("failed to read stage manifest `{manifest_path}`: {err}"))?;
    let manifest: StageUploadManifest = json::from_str(&raw)
        .map_err(|err| format!("failed to decode stage manifest `{manifest_path}`: {err}"))?;
    if manifest.chunks.is_empty() {
        return Err("stage manifest must include at least one chunk".to_string());
    }
    Ok(manifest)
}

fn derive_upload_bundle(manifest: &StageUploadManifest) -> Result<DerivedUploadBundle, String> {
    let service_name = parse_name(&manifest.service_name, "service_name")?;
    let plaintext_root = parse_hash_like(&manifest.plaintext_root);
    let compile_profile = SoraPrivateCompileProfileV1 {
        schema_version: SORA_PRIVATE_COMPILE_PROFILE_VERSION_V1,
        family: manifest.compile_profile.family.clone(),
        quantization: manifest.compile_profile.quantization.clone(),
        opset_version: manifest.compile_profile.opset_version.clone(),
        max_context: manifest.compile_profile.max_context,
        max_images: manifest.compile_profile.max_images,
        vision_patch_policy: manifest.compile_profile.vision_patch_policy.clone(),
        fhe_param_set: manifest.compile_profile.fhe_param_set.clone(),
        execution_policy: manifest.compile_profile.execution_policy.clone(),
    };
    let compile_profile_hash = hash_encoded(&compile_profile)?;
    let upload_recipient = parse_upload_recipient(&manifest.upload_recipient)?;
    let wrapped_bundle_key = parse_wrapped_bundle_key(&manifest.wrapped_bundle_key)?;

    let mut chunks = Vec::with_capacity(manifest.chunks.len());
    let mut plaintext_bytes = 0_u64;
    let mut ciphertext_bytes = 0_u64;
    for chunk in &manifest.chunks {
        let ciphertext_path = PathBuf::from(&chunk.ciphertext_path);
        let ciphertext = fs::read(&ciphertext_path).map_err(|err| {
            format!(
                "failed to read ciphertext chunk `{}`: {err}",
                ciphertext_path.display()
            )
        })?;
        let ciphertext_len = u32::try_from(ciphertext.len())
            .map_err(|_| format!("ciphertext chunk `{}` exceeds u32 length", chunk.ordinal))?;
        let key_version = NonZeroU32::new(chunk.key_version).ok_or_else(|| {
            format!(
                "chunk {} key_version must be greater than zero",
                chunk.ordinal
            )
        })?;
        let nonce = BASE64
            .decode(chunk.nonce_base64.as_bytes())
            .map_err(|err| format!("failed to decode chunk {} nonce: {err}", chunk.ordinal))?;
        let encrypted_payload = SecretEnvelopeV1 {
            schema_version: SECRET_ENVELOPE_VERSION_V1,
            encryption: SecretEnvelopeEncryptionV1::ClientCiphertext,
            key_id: chunk.key_id.clone(),
            key_version,
            nonce,
            commitment: Hash::new(ciphertext.as_slice()),
            aad_digest: chunk
                .aad_seed
                .as_ref()
                .map(|seed| Hash::new(seed.as_bytes())),
            ciphertext,
        };
        let ciphertext_hash = Hash::new(encrypted_payload.ciphertext.as_slice());
        plaintext_bytes = plaintext_bytes.saturating_add(u64::from(chunk.plaintext_len));
        ciphertext_bytes = ciphertext_bytes.saturating_add(u64::from(ciphertext_len));
        chunks.push(SoraUploadedModelChunkV1 {
            schema_version: SORA_UPLOADED_MODEL_CHUNK_VERSION_V1,
            service_name: service_name.clone(),
            model_id: manifest.model_id.clone(),
            weight_version: manifest.weight_version.clone(),
            bundle_root: Hash::new(b"pending-bundle-root"),
            ordinal: chunk.ordinal,
            offset_bytes: chunk.offset_bytes,
            plaintext_len: chunk.plaintext_len,
            ciphertext_len,
            ciphertext_hash,
            encrypted_payload,
        });
    }
    chunks.sort_by_key(|chunk| chunk.ordinal);
    let chunk_manifest_root = compute_chunk_manifest_root(&chunks)?;
    let runtime_format = parse_runtime_format(&manifest.runtime_format);
    let pricing_policy = SoraUploadedModelPricingPolicyV1 {
        storage_xor_nanos: manifest.pricing_policy.storage_xor_nanos,
        compile_xor_nanos: manifest.pricing_policy.compile_xor_nanos,
        runtime_step_xor_nanos: manifest.pricing_policy.runtime_step_xor_nanos,
        decrypt_release_xor_nanos: manifest.pricing_policy.decrypt_release_xor_nanos,
    };
    let bundle_root = compute_bundle_root(
        &service_name,
        manifest,
        plaintext_root,
        runtime_format,
        plaintext_bytes,
        ciphertext_bytes,
        compile_profile_hash,
        chunk_manifest_root,
        &upload_recipient,
        &wrapped_bundle_key,
        &pricing_policy,
    )?;
    for chunk in &mut chunks {
        chunk.bundle_root = bundle_root;
    }

    let bundle = SoraUploadedModelBundleV1 {
        schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        service_name,
        model_id: manifest.model_id.clone(),
        weight_version: manifest.weight_version.clone(),
        family: manifest.family.clone(),
        modalities: manifest.modalities.clone(),
        plaintext_root,
        runtime_format,
        bundle_root,
        chunk_count: u32::try_from(chunks.len()).unwrap_or(u32::MAX),
        plaintext_bytes,
        ciphertext_bytes,
        compile_profile_hash,
        chunk_manifest_root,
        upload_recipient,
        wrapped_bundle_key,
        pricing_policy,
        decryption_policy_ref: manifest.decryption_policy_ref.clone(),
    };

    let weight_artifact_hash = hash_encoded(&(
        "uploaded-model-weight",
        manifest.service_name.as_str(),
        manifest.model_name.as_str(),
        manifest.model_id.as_str(),
        manifest.artifact_id.as_str(),
        manifest.weight_version.as_str(),
        bundle_root,
    ))?;
    let training_config_hash = hash_encoded(&(
        "private-compile-profile",
        compile_profile.clone(),
        manifest.runtime_format.as_str(),
    ))?;
    let reproducibility_hash = hash_encoded(&(
        "chunk-reproducibility",
        manifest.service_name.as_str(),
        manifest.model_id.as_str(),
        chunks
            .iter()
            .map(|chunk| {
                (
                    chunk.ordinal,
                    chunk.offset_bytes,
                    chunk.plaintext_len,
                    chunk.ciphertext_len,
                    chunk.ciphertext_hash,
                )
            })
            .collect::<Vec<_>>(),
    ))?;
    let provenance_attestation_hash = hash_encoded(&(
        "uploaded-model-attestation",
        weight_artifact_hash,
        training_config_hash,
        reproducibility_hash,
        manifest.finalize.dataset_ref.as_str(),
    ))?;

    Ok(DerivedUploadBundle {
        bundle,
        compile_profile,
        chunks,
        bundle_root,
        compile_profile_hash,
        chunk_manifest_root,
        plaintext_bytes,
        ciphertext_bytes,
        weight_artifact_hash,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    })
}

fn hash_encoded<T>(value: &T) -> Result<Hash, String>
where
    T: norito::core::NoritoSerialize,
{
    let encoded = to_bytes(value).map_err(|err| format!("failed to encode hash payload: {err}"))?;
    Ok(Hash::new(encoded))
}

fn serialize_tuple_field<W, T>(writer: &mut W, value: &T) -> Result<(), norito::Error>
where
    W: io::Write,
    T: norito::core::NoritoSerialize + ?Sized,
{
    let mut payload = Vec::new();
    value.serialize(&mut payload)?;
    let len = u64::try_from(payload.len()).map_err(|_| norito::Error::LengthMismatch)?;
    norito::core::write_len(writer, len)?;
    writer.write_all(&payload)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compute_bundle_root(
    service_name: &Name,
    manifest: &StageUploadManifest,
    plaintext_root: Hash,
    runtime_format: SoraUploadedModelRuntimeFormatV1,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    compile_profile_hash: Hash,
    chunk_manifest_root: Hash,
    upload_recipient: &SoraUploadedModelEncryptionRecipientV1,
    wrapped_bundle_key: &SoraUploadedModelWrappedKeyV1,
    pricing_policy: &SoraUploadedModelPricingPolicyV1,
) -> Result<Hash, String> {
    hash_encoded(&UploadedModelBundleRootPayload {
        service_name: service_name.as_ref(),
        model_id: manifest.model_id.as_str(),
        weight_version: manifest.weight_version.as_str(),
        family: manifest.family.as_str(),
        modalities: &manifest.modalities,
        plaintext_root,
        runtime_format,
        plaintext_bytes,
        ciphertext_bytes,
        compile_profile_hash,
        chunk_manifest_root,
        upload_recipient,
        wrapped_bundle_key,
        pricing_policy,
        decryption_policy_ref: manifest.decryption_policy_ref.as_str(),
    })
}

fn compute_chunk_manifest_root(chunks: &[SoraUploadedModelChunkV1]) -> Result<Hash, String> {
    let manifest = chunks
        .iter()
        .map(|chunk| {
            (
                chunk.ordinal,
                chunk.offset_bytes,
                chunk.plaintext_len,
                chunk.ciphertext_len,
                chunk.ciphertext_hash,
            )
        })
        .collect::<Vec<_>>();
    let encoded =
        to_bytes(&manifest).map_err(|err| format!("failed to encode chunk manifest: {err}"))?;
    Ok(Hash::new(encoded))
}

fn parse_name(raw: &str, field: &str) -> Result<Name, String> {
    raw.parse::<Name>()
        .map_err(|err| format!("invalid {field}: {err}"))
}

fn parse_runtime_format(raw: &str) -> SoraUploadedModelRuntimeFormatV1 {
    match raw.trim().to_ascii_lowercase().as_str() {
        "hf" | "hf-safetensors" | "huggingface-safetensors" | "hugging_face_safetensors" => {
            SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors
        }
        _ => SoraUploadedModelRuntimeFormatV1::SoracloudPrivateIr,
    }
}

fn parse_uploaded_model_kem(raw: &str) -> SoraUploadedModelKeyEncapsulationV1 {
    match raw.trim().to_ascii_lowercase().as_str() {
        "x25519-hkdf-sha256" | "x25519_hkdf_sha256" | "x25519hkdfsha256" => {
            SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256
        }
        _ => SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
    }
}

fn parse_uploaded_model_aead(raw: &str) -> SoraUploadedModelKeyWrapAeadV1 {
    match raw.trim().to_ascii_lowercase().as_str() {
        "aes-256-gcm" | "aes_256_gcm" | "aes256gcm" => SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
        _ => SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
    }
}

fn parse_upload_recipient(
    recipient: &StageUploadRecipient,
) -> Result<SoraUploadedModelEncryptionRecipientV1, String> {
    let key_version = NonZeroU32::new(recipient.key_version)
        .ok_or_else(|| "upload recipient key_version must be greater than zero".to_string())?;
    let public_key_bytes = BASE64
        .decode(recipient.public_key_bytes_base64.as_bytes())
        .map_err(|err| format!("failed to decode upload recipient public key: {err}"))?;
    Ok(SoraUploadedModelEncryptionRecipientV1 {
        schema_version: SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        key_id: recipient.key_id.clone(),
        key_version,
        kem: parse_uploaded_model_kem(&recipient.kem),
        aead: parse_uploaded_model_aead(&recipient.aead),
        public_key_fingerprint: parse_hash_like(&recipient.public_key_fingerprint),
        public_key_bytes,
    })
}

fn parse_wrapped_bundle_key(
    wrapped_key: &StageWrappedBundleKey,
) -> Result<SoraUploadedModelWrappedKeyV1, String> {
    let recipient_key_version =
        NonZeroU32::new(wrapped_key.recipient_key_version).ok_or_else(|| {
            "wrapped bundle key recipient_key_version must be greater than zero".to_string()
        })?;
    let ephemeral_public_key = BASE64
        .decode(wrapped_key.ephemeral_public_key_base64.as_bytes())
        .map_err(|err| format!("failed to decode wrapped bundle key ephemeral key: {err}"))?;
    let nonce = BASE64
        .decode(wrapped_key.nonce_base64.as_bytes())
        .map_err(|err| format!("failed to decode wrapped bundle key nonce: {err}"))?;
    let wrapped_key_ciphertext = BASE64
        .decode(wrapped_key.wrapped_key_ciphertext_base64.as_bytes())
        .map_err(|err| format!("failed to decode wrapped bundle key ciphertext: {err}"))?;
    Ok(SoraUploadedModelWrappedKeyV1 {
        schema_version: SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
        recipient_key_id: wrapped_key.recipient_key_id.clone(),
        recipient_key_version,
        kem: parse_uploaded_model_kem(&wrapped_key.kem),
        aead: parse_uploaded_model_aead(&wrapped_key.aead),
        ephemeral_public_key,
        nonce,
        ciphertext_hash: Hash::new(wrapped_key_ciphertext.as_slice()),
        wrapped_key_ciphertext,
        aad_digest: parse_hash_like(&wrapped_key.aad_digest),
    })
}

fn parse_privacy_mode(raw: &str) -> SoraModelPrivacyModeV1 {
    match raw.trim().to_ascii_lowercase().as_str() {
        "private" | "private-execution" | "private_execution" => {
            SoraModelPrivacyModeV1::PrivateExecution
        }
        _ => SoraModelPrivacyModeV1::PublicCommitments,
    }
}

fn parse_private_session_status(raw: &str) -> SoraPrivateInferenceSessionStatusV1 {
    match raw.trim().to_ascii_lowercase().as_str() {
        "running" => SoraPrivateInferenceSessionStatusV1::Running,
        "awaiting-decryption" | "awaiting_decryption" => {
            SoraPrivateInferenceSessionStatusV1::AwaitingDecryption
        }
        "completed" => SoraPrivateInferenceSessionStatusV1::Completed,
        "failed" => SoraPrivateInferenceSessionStatusV1::Failed,
        "revoked" => SoraPrivateInferenceSessionStatusV1::Revoked,
        _ => SoraPrivateInferenceSessionStatusV1::Admitted,
    }
}

fn parse_hash_like(raw: &str) -> Hash {
    Hash::from_str(raw.trim()).unwrap_or_else(|_| Hash::new(raw.trim().as_bytes()))
}

#[allow(dead_code)]
fn _assert_stage_paths_absolute(manifest_path: &Path, chunk_path: &Path) -> Result<(), String> {
    if !manifest_path.is_absolute() || !chunk_path.is_absolute() {
        return Err("stage manifest and chunk paths must be absolute".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn derive_upload_bundle_hashes_bundle_root_without_tuple_arity_regression() {
        let tempdir = tempdir().expect("create temp dir");
        let chunk_path = tempdir.path().join("chunk.bin");
        fs::write(&chunk_path, b"ciphertext-chunk").expect("write ciphertext chunk");
        let recipient_public_key_bytes = [7_u8; 32];

        let manifest = StageUploadManifest {
            schema_version: 1,
            label: None,
            service_name: "service".to_string(),
            model_name: "model".to_string(),
            model_id: "model-id".to_string(),
            artifact_id: "artifact-id".to_string(),
            weight_version: "weights-v1".to_string(),
            family: "test-family".to_string(),
            modalities: vec!["text".to_string()],
            runtime_format: "hf".to_string(),
            privacy_mode: "private".to_string(),
            plaintext_root: "plaintext-root".to_string(),
            source_path: None,
            upload_recipient: StageUploadRecipient {
                key_id: "recipient-key".to_string(),
                key_version: 1,
                kem: "x25519-hkdf-sha256".to_string(),
                aead: "aes-256-gcm".to_string(),
                public_key_bytes_base64: BASE64.encode(recipient_public_key_bytes),
                public_key_fingerprint: Hash::new(recipient_public_key_bytes.as_slice())
                    .to_string(),
            },
            wrapped_bundle_key: StageWrappedBundleKey {
                recipient_key_id: "recipient-key".to_string(),
                recipient_key_version: 1,
                kem: "x25519-hkdf-sha256".to_string(),
                aead: "aes-256-gcm".to_string(),
                ephemeral_public_key_base64: BASE64.encode([9_u8; 32]),
                nonce_base64: BASE64.encode([11_u8; 12]),
                wrapped_key_ciphertext_base64: BASE64.encode([13_u8; 48]),
                aad_digest: "wrapped-key-aad".to_string(),
            },
            decryption_policy_ref: "policy-ref".to_string(),
            pricing_policy: StagePricingPolicy {
                storage_xor_nanos: 11,
                compile_xor_nanos: 22,
                runtime_step_xor_nanos: 33,
                decrypt_release_xor_nanos: 44,
            },
            compile_profile: StageCompileProfile {
                family: "test-family".to_string(),
                quantization: "q8".to_string(),
                opset_version: "1".to_string(),
                max_context: 2048,
                max_images: 1,
                vision_patch_policy: "tiles".to_string(),
                fhe_param_set: "fhe-v1".to_string(),
                execution_policy: "deterministic".to_string(),
            },
            finalize: StageFinalize {
                dataset_ref: "dataset-ref".to_string(),
            },
            chunks: vec![StageChunk {
                ordinal: 0,
                offset_bytes: 0,
                plaintext_len: 15,
                key_id: "chunk-key".to_string(),
                key_version: 1,
                nonce_base64: BASE64.encode([17_u8; 12]),
                ciphertext_path: chunk_path.display().to_string(),
                aad_seed: Some("aad-seed".to_string()),
            }],
            created_at: None,
        };

        let derived = derive_upload_bundle(&manifest).expect("derive upload bundle");

        assert_eq!(derived.bundle.bundle_root, derived.bundle_root);
        assert_eq!(
            derived.bundle.chunk_manifest_root,
            derived.chunk_manifest_root
        );
        assert_eq!(derived.chunks.len(), 1);
        assert_eq!(derived.chunks[0].bundle_root, derived.bundle_root);
        derived.bundle.validate().expect("bundle validates");
    }
}
