//! Build exact Soracloud uploaded-model and private-runtime request payloads for desktop clients.

use std::{
    env, fs,
    io::{self, Read as _},
    num::NonZeroU32,
    path::PathBuf,
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{ExposedPrivateKey, Hash, PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1, SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1, SoraUploadedModelBundleV1,
        SoraUploadedModelEncryptionRecipientV1, SoraUploadedModelKeyEncapsulationV1,
        SoraUploadedModelKeyWrapAeadV1, SoraUploadedModelPricingPolicyV1,
        SoraUploadedModelRuntimeFormatV1, SoraUploadedModelWrappedKeyV1,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
    },
    sorafs::pin_registry::ManifestDigest,
};
use norito::{json, to_bytes};

#[derive(Debug, norito::json::JsonDeserialize)]
struct SignUploadInput {
    manifest_path: String,
    authority: String,
    private_key: String,
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
    sorafs_manifest_digest: String,
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
struct UploadedModelFinalizePayload {
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: Hash,
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
struct UploadInitOutput {
    bundle_root: Hash,
    sorafs_manifest_digest: ManifestDigest,
    chunk_manifest_root: Hash,
    chunk_count: u32,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    request: SignedUploadedModelBundleInitRequest,
}

#[derive(Debug, norito::json::JsonSerialize)]
struct UploadFinalizeOutput {
    bundle_root: Hash,
    chunk_manifest_root: Hash,
    weight_artifact_hash: Hash,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    finalize_request: SignedUploadedModelFinalizeRequest,
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
    ManifestDigest,
    u64,
    u64,
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
    sorafs_manifest_digest: ManifestDigest,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
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
        serialize_tuple_field(&mut writer, &self.sorafs_manifest_digest)?;
        serialize_tuple_field(&mut writer, &self.plaintext_bytes)?;
        serialize_tuple_field(&mut writer, &self.ciphertext_bytes)?;
        serialize_tuple_field(&mut writer, &self.chunk_manifest_root)?;
        serialize_tuple_field(&mut writer, self.upload_recipient)?;
        serialize_tuple_field(&mut writer, self.wrapped_bundle_key)?;
        serialize_tuple_field(&mut writer, self.pricing_policy)?;
        serialize_tuple_field(&mut writer, &self.decryption_policy_ref)?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct DerivedChunk {
    ordinal: u32,
    offset_bytes: u64,
    plaintext_len: u32,
    ciphertext_len: u32,
    ciphertext_hash: Hash,
}

struct DerivedUploadBundle {
    bundle: SoraUploadedModelBundleV1,
    chunks: Vec<DerivedChunk>,
    bundle_root: Hash,
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
        return Err(
            "usage: soracloud_request_signer <sign-upload-init|sign-upload-finalize>".to_string(),
        );
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
                provenance: signer.provenance(&payload_bytes)?,
                authority: Some(signer.authority),
                private_key: Some(signer.private_key),
            };
            write_stdout_json(&UploadInitOutput {
                bundle_root: derived.bundle_root,
                sorafs_manifest_digest: derived.bundle.sorafs_manifest_digest,
                chunk_manifest_root: derived.chunk_manifest_root,
                chunk_count: u32::try_from(derived.chunks.len()).unwrap_or(u32::MAX),
                plaintext_bytes: derived.plaintext_bytes,
                ciphertext_bytes: derived.ciphertext_bytes,
                request,
            })
        }
        "sign-upload-finalize" => {
            let input: SignUploadInput = read_stdin_json()?;
            let signer = parse_signer(&input.authority, &input.private_key)?;
            let manifest = load_stage_manifest(&input.manifest_path)?;
            let derived = derive_upload_bundle(&manifest)?;

            let finalize_payload = UploadedModelFinalizePayload {
                service_name: manifest.service_name.clone(),
                model_name: manifest.model_name.clone(),
                model_id: manifest.model_id.clone(),
                artifact_id: manifest.artifact_id.clone(),
                weight_version: manifest.weight_version.clone(),
                bundle_root: derived.bundle_root,
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
                finalize_payload.weight_artifact_hash,
                finalize_payload.dataset_ref.as_str(),
                finalize_payload.training_config_hash,
                finalize_payload.reproducibility_hash,
                finalize_payload.provenance_attestation_hash,
            )
            .map_err(|err| format!("failed to encode upload finalize payload: {err}"))?;
            let finalize_request = SignedUploadedModelFinalizeRequest {
                payload: finalize_payload,
                provenance: signer.provenance(&finalize_bytes)?,
                authority: Some(signer.authority.clone()),
                private_key: Some(signer.private_key.clone()),
            };

            write_stdout_json(&UploadFinalizeOutput {
                bundle_root: derived.bundle_root,
                chunk_manifest_root: derived.chunk_manifest_root,
                weight_artifact_hash: derived.weight_artifact_hash,
                training_config_hash: derived.training_config_hash,
                reproducibility_hash: derived.reproducibility_hash,
                provenance_attestation_hash: derived.provenance_attestation_hash,
                finalize_request,
            })
        }
        "sign-upload-chunk" | "sign-private-run" | "sign-private-output-release" => Err(format!(
            "command `{command}` was removed from Soracloud production V1; use the SoraFS-backed upload register/finalize flow"
        )),
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
    fn provenance(&self, payload: &[u8]) -> Result<ManifestProvenance, String> {
        Ok(ManifestProvenance {
            signer: self.public_key.clone(),
            signature: Signature::try_new(&self.private_key.0, payload)
                .map_err(|err| format!("failed to sign Soracloud provenance payload: {err}"))?,
        })
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
    let sorafs_manifest_digest = parse_manifest_digest(&manifest.sorafs_manifest_digest)?;
    let compile_profile_hash = hash_encoded(&(
        "soracloud-upload-compile-profile",
        manifest.compile_profile.family.as_str(),
        manifest.compile_profile.quantization.as_str(),
        manifest.compile_profile.opset_version.as_str(),
        manifest.compile_profile.max_context,
        manifest.compile_profile.max_images,
        manifest.compile_profile.vision_patch_policy.as_str(),
        manifest.compile_profile.fhe_param_set.as_str(),
        manifest.compile_profile.execution_policy.as_str(),
    ))?;
    let upload_recipient = parse_upload_recipient(&manifest.upload_recipient)?;
    let wrapped_bundle_key = parse_wrapped_bundle_key(&manifest.wrapped_bundle_key)?;

    let mut chunks = Vec::with_capacity(manifest.chunks.len());
    let mut plaintext_bytes = 0_u64;
    let mut ciphertext_bytes = 0_u64;
    for chunk in &manifest.chunks {
        validate_chunk_encryption_metadata(chunk)?;
        let ciphertext_path = PathBuf::from(&chunk.ciphertext_path);
        let ciphertext = fs::read(&ciphertext_path).map_err(|err| {
            format!(
                "failed to read ciphertext chunk `{}`: {err}",
                ciphertext_path.display()
            )
        })?;
        let ciphertext_len = u32::try_from(ciphertext.len())
            .map_err(|_| format!("ciphertext chunk `{}` exceeds u32 length", chunk.ordinal))?;
        let ciphertext_hash = Hash::new(ciphertext.as_slice());
        plaintext_bytes = plaintext_bytes.saturating_add(u64::from(chunk.plaintext_len));
        ciphertext_bytes = ciphertext_bytes.saturating_add(u64::from(ciphertext_len));
        chunks.push(DerivedChunk {
            ordinal: chunk.ordinal,
            offset_bytes: chunk.offset_bytes,
            plaintext_len: chunk.plaintext_len,
            ciphertext_len,
            ciphertext_hash,
        });
    }
    chunks.sort_by_key(|chunk| chunk.ordinal);
    let chunk_manifest_root = compute_chunk_manifest_root(&chunks)?;
    let runtime_format = parse_runtime_format(&manifest.runtime_format);
    let _legacy_private_runtime_pricing = (
        manifest.pricing_policy.compile_xor_nanos,
        manifest.pricing_policy.runtime_step_xor_nanos,
        manifest.pricing_policy.decrypt_release_xor_nanos,
    );
    let pricing_policy = SoraUploadedModelPricingPolicyV1 {
        storage_xor_nanos: manifest.pricing_policy.storage_xor_nanos,
    };
    let bundle_root = compute_bundle_root(
        &service_name,
        manifest,
        plaintext_root,
        runtime_format,
        sorafs_manifest_digest,
        plaintext_bytes,
        ciphertext_bytes,
        chunk_manifest_root,
        &upload_recipient,
        &wrapped_bundle_key,
        &pricing_policy,
    )?;

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
        sorafs_manifest_digest,
        chunk_count: u32::try_from(chunks.len()).unwrap_or(u32::MAX),
        plaintext_bytes,
        ciphertext_bytes,
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
        compile_profile_hash,
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
        chunks,
        bundle_root,
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
    sorafs_manifest_digest: ManifestDigest,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
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
        sorafs_manifest_digest,
        plaintext_bytes,
        ciphertext_bytes,
        chunk_manifest_root,
        upload_recipient,
        wrapped_bundle_key,
        pricing_policy,
        decryption_policy_ref: manifest.decryption_policy_ref.as_str(),
    })
}

fn compute_chunk_manifest_root(chunks: &[DerivedChunk]) -> Result<Hash, String> {
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
        "deterministic-quantized-cpu-v1" | "deterministic_quantized_cpu_v1" | "cpu-v1" => {
            SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1
        }
        _ => SoraUploadedModelRuntimeFormatV1::HuggingFaceSafetensors,
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

fn parse_hash_like(raw: &str) -> Hash {
    Hash::from_str(raw.trim()).unwrap_or_else(|_| Hash::new(raw.trim().as_bytes()))
}

fn parse_manifest_digest(raw: &str) -> Result<ManifestDigest, String> {
    let trimmed = raw.trim();
    let hex_body = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    match hex::decode(hex_body) {
        Ok(bytes) => {
            let digest = <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
                format!(
                    "sorafs_manifest_digest must be 32 bytes, decoded {} bytes",
                    bytes.len()
                )
            })?;
            Ok(ManifestDigest::new(digest))
        }
        Err(_) => {
            let digest = Hash::new(trimmed.as_bytes());
            Ok(ManifestDigest::new(*digest.as_ref()))
        }
    }
}

fn validate_chunk_encryption_metadata(chunk: &StageChunk) -> Result<(), String> {
    if chunk.key_id.trim().is_empty() {
        return Err(format!("chunk {} key_id must not be empty", chunk.ordinal));
    }
    NonZeroU32::new(chunk.key_version).ok_or_else(|| {
        format!(
            "chunk {} key_version must be greater than zero",
            chunk.ordinal
        )
    })?;
    BASE64
        .decode(chunk.nonce_base64.as_bytes())
        .map_err(|err| format!("failed to decode chunk {} nonce: {err}", chunk.ordinal))?;
    if let Some(seed) = &chunk.aad_seed {
        let _ = Hash::new(seed.as_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn mutation_signer_provenance_signature_verifies() {
        let private_key = ExposedPrivateKey(
            iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &[0x42; 32])
                .expect("private key"),
        );
        let public_key = PublicKey::from(private_key.0.clone());
        let signer = MutationSigner {
            authority: AccountId::new(public_key.clone()),
            private_key,
            public_key,
        };
        let payload = b"soracloud-upload-provenance";

        let provenance = signer.provenance(payload).expect("sign provenance");

        provenance
            .signature
            .verify(&provenance.signer, payload)
            .expect("provenance signature verifies");
    }

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
            sorafs_manifest_digest: hex::encode([0xA5_u8; 32]),
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
        assert_eq!(
            derived.bundle.sorafs_manifest_digest.as_bytes(),
            &[0xA5; 32]
        );
        assert_eq!(derived.chunks.len(), 1);
        assert_eq!(derived.chunks[0].ciphertext_len, 16);
        derived.bundle.validate().expect("bundle validates");
    }
}
