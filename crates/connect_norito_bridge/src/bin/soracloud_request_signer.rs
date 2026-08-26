//! Build exact Soracloud uploaded-model registry request payloads for desktop clients.
//! Private signing material is consumed only by this local helper; generated
//! Torii request JSON contains signed provenance and never embeds the key.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{ExposedPrivateKey, Hash, PublicKey, Signature};
use iroha_data_model::{
    account::AccountId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1, SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
        SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1, SORACLOUD_XOR_SCALE, SoraUploadedModelBundleV1,
        SoraUploadedModelEncryptionRecipientV1, SoraUploadedModelKeyEncapsulationV1,
        SoraUploadedModelKeyWrapAeadV1, SoraUploadedModelPackageFormatV1,
        SoraUploadedModelPricingPolicyV1, SoraUploadedModelWrappedKeyV1,
        encode_uploaded_model_bundle_register_provenance_payload,
        encode_uploaded_model_finalize_provenance_payload,
    },
    sorafs::pin_registry::ManifestDigest,
};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::{json, to_bytes};
use std::{
    env, fs,
    io::{self, Read as _},
    num::NonZeroU32,
    path::PathBuf,
};
const STAGE_UPLOAD_MANIFEST_SCHEMA_VERSION_V1: u16 = 1;
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignUploadInput {
    manifest_path: String,
    authority: String,
    private_key: String,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
#[allow(dead_code)]
struct StageUploadManifest {
    schema_version: u16,
    #[norito(required)]
    label: Option<String>,
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    family: String,
    modalities: Vec<String>,
    package_format: String,
    plaintext_root: String,
    sorafs_manifest_digest: String,
    #[norito(required)]
    source_path: Option<String>,
    upload_recipient: StageUploadRecipient,
    wrapped_bundle_key: StageWrappedBundleKey,
    decryption_policy_ref: String,
    pricing_policy: StagePricingPolicy,
    compile_profile: StageCompileProfile,
    finalize: StageFinalize,
    chunks: Vec<StageChunk>,
    #[norito(required)]
    created_at: Option<String>,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StagePricingPolicy {
    storage_xor_nanos: u128,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
#[norito(deny_unknown_fields)]
struct StageFinalize {
    dataset_ref: String,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StageChunk {
    ordinal: u32,
    offset_bytes: u64,
    plaintext_len: u32,
    key_id: String,
    key_version: u32,
    nonce_base64: String,
    ciphertext_path: String,
    #[norito(required)]
    aad_seed: Option<String>,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StageUploadRecipient {
    key_id: String,
    key_version: u32,
    kem: String,
    aead: String,
    public_key_bytes_base64: String,
    public_key_fingerprint: String,
}
#[derive(Debug, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
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
    private_key: ExposedPrivateKey,
    public_key: PublicKey,
}
/// Canonical V1 preimage for uploaded-model bundle roots.
#[derive(norito::Encode)]
struct UploadedModelBundleRootPreimageV1 {
    service_name: String,
    model_id: String,
    weight_version: String,
    family: String,
    modalities: Vec<String>,
    plaintext_root: Hash,
    package_format: SoraUploadedModelPackageFormatV1,
    sorafs_manifest_digest: ManifestDigest,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    chunk_manifest_root: Hash,
    upload_recipient: SoraUploadedModelEncryptionRecipientV1,
    wrapped_bundle_key: SoraUploadedModelWrappedKeyV1,
    pricing_policy: SoraUploadedModelPricingPolicyV1,
    decryption_policy_ref: String,
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
    #[cfg(test)]
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
            };
            write_stdout_json(&UploadInitOutput {
                bundle_root: derived.bundle_root,
                sorafs_manifest_digest: derived.bundle.sorafs_manifest_digest,
                chunk_manifest_root: derived.chunk_manifest_root,
                chunk_count: derived.bundle.chunk_count,
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
        .map_err(|err| format!("invalid authority account id: {err}"))?;
    let private_key = private_key
        .parse::<ExposedPrivateKey>()
        .map_err(|err| format!("invalid private key: {err}"))?;
    let public_key: PublicKey = private_key.0.clone().into();
    if authority != AccountId::new(public_key.clone()) {
        return Err("authority account id does not match the private key".to_owned());
    }
    Ok(MutationSigner {
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
    decode_stage_manifest(&raw)
        .map_err(|err| format!("failed to decode stage manifest `{manifest_path}`: {err}"))
}
fn decode_stage_manifest(raw: &str) -> Result<StageUploadManifest, String> {
    let manifest: StageUploadManifest =
        json::from_str(raw).map_err(|err| format!("invalid JSON schema: {err}"))?;
    validate_stage_manifest_v1(&manifest)?;
    if manifest.chunks.is_empty() {
        return Err("stage manifest must include at least one chunk".to_string());
    }
    Ok(manifest)
}
fn validate_stage_manifest_v1(manifest: &StageUploadManifest) -> Result<(), String> {
    if manifest.schema_version != STAGE_UPLOAD_MANIFEST_SCHEMA_VERSION_V1 {
        return Err(format!(
            "stage manifest schema_version must equal {STAGE_UPLOAD_MANIFEST_SCHEMA_VERSION_V1}, got {}",
            manifest.schema_version
        ));
    }
    Ok(())
}
fn xor_quantity_from_nanos(value: u128) -> Result<Quantity, String> {
    Quantity::from_canonical_numeric(Numeric::new(value, SORACLOUD_XOR_SCALE))
        .map_err(|error| format!("invalid nano-XOR storage price: {error}"))
}
fn derive_upload_bundle(manifest: &StageUploadManifest) -> Result<DerivedUploadBundle, String> {
    validate_stage_manifest_v1(manifest)?;
    let service_name = parse_name(&manifest.service_name, "service_name")?;
    let plaintext_root = parse_hash(&manifest.plaintext_root, "plaintext_root")?;
    let sorafs_manifest_digest = parse_manifest_digest(&manifest.sorafs_manifest_digest)?;
    let compile_profile_hash = hash_encoded(&(
        "soracloud-uploaded-model-compile-profile-fields-v1",
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
        plaintext_bytes = plaintext_bytes
            .checked_add(u64::from(chunk.plaintext_len))
            .ok_or_else(|| "stage manifest plaintext byte total exceeds u64".to_string())?;
        ciphertext_bytes = ciphertext_bytes
            .checked_add(u64::from(ciphertext_len))
            .ok_or_else(|| "stage manifest ciphertext byte total exceeds u64".to_string())?;
        chunks.push(DerivedChunk {
            ordinal: chunk.ordinal,
            offset_bytes: chunk.offset_bytes,
            plaintext_len: chunk.plaintext_len,
            ciphertext_len,
            ciphertext_hash,
        });
    }
    chunks.sort_by_key(|chunk| chunk.ordinal);
    let chunk_count = u32::try_from(chunks.len())
        .map_err(|_| "stage manifest chunk count exceeds u32".to_string())?;
    let chunk_manifest_root = compute_chunk_manifest_root(&chunks)?;
    let package_format = parse_package_format(&manifest.package_format)?;
    let pricing_policy = SoraUploadedModelPricingPolicyV1 {
        storage_price: xor_quantity_from_nanos(manifest.pricing_policy.storage_xor_nanos)?,
    };
    let bundle_root = compute_bundle_root(
        &service_name,
        manifest,
        plaintext_root,
        package_format,
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
        package_format,
        bundle_root,
        sorafs_manifest_digest,
        chunk_count,
        plaintext_bytes,
        ciphertext_bytes,
        chunk_manifest_root,
        upload_recipient,
        wrapped_bundle_key,
        pricing_policy,
        decryption_policy_ref: manifest.decryption_policy_ref.clone(),
    };
    bundle
        .validate()
        .map_err(|error| format!("invalid V1 uploaded-model bundle: {error}"))?;
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
        "uploaded-model-compile-profile-v1",
        compile_profile_hash,
        manifest.package_format.as_str(),
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
        #[cfg(test)]
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
#[allow(clippy::too_many_arguments)]
fn compute_bundle_root(
    service_name: &Name,
    manifest: &StageUploadManifest,
    plaintext_root: Hash,
    package_format: SoraUploadedModelPackageFormatV1,
    sorafs_manifest_digest: ManifestDigest,
    plaintext_bytes: u64,
    ciphertext_bytes: u64,
    chunk_manifest_root: Hash,
    upload_recipient: &SoraUploadedModelEncryptionRecipientV1,
    wrapped_bundle_key: &SoraUploadedModelWrappedKeyV1,
    pricing_policy: &SoraUploadedModelPricingPolicyV1,
) -> Result<Hash, String> {
    hash_encoded(&UploadedModelBundleRootPreimageV1 {
        service_name: service_name.as_ref().to_owned(),
        model_id: manifest.model_id.clone(),
        weight_version: manifest.weight_version.clone(),
        family: manifest.family.clone(),
        modalities: manifest.modalities.clone(),
        plaintext_root,
        package_format,
        sorafs_manifest_digest,
        plaintext_bytes,
        ciphertext_bytes,
        chunk_manifest_root,
        upload_recipient: upload_recipient.clone(),
        wrapped_bundle_key: wrapped_bundle_key.clone(),
        pricing_policy: pricing_policy.clone(),
        decryption_policy_ref: manifest.decryption_policy_ref.clone(),
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
fn parse_package_format(raw: &str) -> Result<SoraUploadedModelPackageFormatV1, String> {
    match raw {
        "NormalizedHuggingFaceSafetensorsV1" => {
            Ok(SoraUploadedModelPackageFormatV1::NormalizedHuggingFaceSafetensorsV1)
        }
        _ => Err(format!(
            "invalid package_format `{raw}`; expected `NormalizedHuggingFaceSafetensorsV1`"
        )),
    }
}
fn parse_uploaded_model_kem(raw: &str) -> Result<SoraUploadedModelKeyEncapsulationV1, String> {
    match raw {
        "X25519HkdfSha256" => Ok(SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256),
        _ => Err(format!(
            "invalid uploaded-model KEM `{raw}`; expected `X25519HkdfSha256`"
        )),
    }
}
fn parse_uploaded_model_aead(raw: &str) -> Result<SoraUploadedModelKeyWrapAeadV1, String> {
    match raw {
        "Aes256Gcm" => Ok(SoraUploadedModelKeyWrapAeadV1::Aes256Gcm),
        _ => Err(format!(
            "invalid uploaded-model AEAD `{raw}`; expected `Aes256Gcm`"
        )),
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
        kem: parse_uploaded_model_kem(&recipient.kem)?,
        aead: parse_uploaded_model_aead(&recipient.aead)?,
        public_key_fingerprint: parse_hash(
            &recipient.public_key_fingerprint,
            "upload_recipient.public_key_fingerprint",
        )?,
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
        kem: parse_uploaded_model_kem(&wrapped_key.kem)?,
        aead: parse_uploaded_model_aead(&wrapped_key.aead)?,
        ephemeral_public_key,
        nonce,
        ciphertext_hash: Hash::new(wrapped_key_ciphertext.as_slice()),
        wrapped_key_ciphertext,
        aad_digest: parse_hash(&wrapped_key.aad_digest, "wrapped_bundle_key.aad_digest")?,
    })
}
fn parse_hash(raw: &str, field: &str) -> Result<Hash, String> {
    let hash = raw
        .parse::<Hash>()
        .map_err(|err| format!("invalid {field}: {err}"))?;
    if hash.to_string() != raw {
        return Err(format!(
            "invalid {field}: expected canonical lowercase 64-character hexadecimal"
        ));
    }
    Ok(hash)
}
fn parse_manifest_digest(raw: &str) -> Result<ManifestDigest, String> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "sorafs_manifest_digest must be canonical lowercase 64-character hexadecimal"
                .to_string(),
        );
    }
    let bytes = hex::decode(raw)
        .map_err(|err| format!("failed to decode sorafs_manifest_digest: {err}"))?;
    let digest = <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| "sorafs_manifest_digest must decode to exactly 32 bytes".to_string())?;
    Ok(ManifestDigest::new(digest))
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
    fn exact_stage_manifest_json() -> String {
        let plaintext_root = Hash::new(b"plaintext-root");
        let recipient_fingerprint = Hash::new(&[7_u8; 32]);
        let aad_digest = Hash::new(b"wrapped-key-aad");
        let manifest_digest = hex::encode([0xA5_u8; 32]);
        format!(
            r#"{{
                "schema_version":1,
                "label":null,
                "service_name":"service",
                "model_name":"model",
                "model_id":"model-id",
                "artifact_id":"artifact-id",
                "weight_version":"weights-v1",
                "family":"test-family",
                "modalities":["text"],
                "package_format":"NormalizedHuggingFaceSafetensorsV1",
                "plaintext_root":"{plaintext_root}",
                "sorafs_manifest_digest":"{manifest_digest}",
                "source_path":null,
                "upload_recipient":{{
                    "key_id":"recipient-key",
                    "key_version":1,
                    "kem":"X25519HkdfSha256",
                    "aead":"Aes256Gcm",
                    "public_key_bytes_base64":"{}",
                    "public_key_fingerprint":"{recipient_fingerprint}"
                }},
                "wrapped_bundle_key":{{
                    "recipient_key_id":"recipient-key",
                    "recipient_key_version":1,
                    "kem":"X25519HkdfSha256",
                    "aead":"Aes256Gcm",
                    "ephemeral_public_key_base64":"{}",
                    "nonce_base64":"{}",
                    "wrapped_key_ciphertext_base64":"{}",
                    "aad_digest":"{aad_digest}"
                }},
                "decryption_policy_ref":"policy-ref",
                "pricing_policy":{{"storage_xor_nanos":11}},
                "compile_profile":{{
                    "family":"test-family",
                    "quantization":"q8",
                    "opset_version":"1",
                    "max_context":2048,
                    "max_images":1,
                    "vision_patch_policy":"tiles",
                    "fhe_param_set":"fhe-v1",
                    "execution_policy":"deterministic"
                }},
                "finalize":{{"dataset_ref":"dataset-ref"}},
                "chunks":[{{
                    "ordinal":0,
                    "offset_bytes":0,
                    "plaintext_len":15,
                    "key_id":"chunk-key",
                    "key_version":1,
                    "nonce_base64":"{}",
                    "ciphertext_path":"chunk.bin",
                    "aad_seed":null
                }}],
                "created_at":null
            }}"#,
            BASE64.encode([7_u8; 32]),
            BASE64.encode([9_u8; 32]),
            BASE64.encode([11_u8; 12]),
            BASE64.encode([13_u8; 48]),
            BASE64.encode([17_u8; 12]),
        )
    }
    fn test_mutation_signer() -> MutationSigner {
        let private_key = ExposedPrivateKey(
            iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &[0x42; 32])
                .expect("private key"),
        );
        let public_key = PublicKey::from(private_key.0.clone());
        MutationSigner {
            private_key,
            public_key,
        }
    }
    fn assert_no_inline_signing_fields(request: &impl norito::json::JsonSerialize) {
        let norito::json::Value::Object(body) =
            json::to_value(request).expect("serialize signed upload request")
        else {
            panic!("signed upload request must serialize as an object");
        };
        for field in ["authority", "private_key"] {
            assert!(
                !body.contains_key(field),
                "signed upload request serialized retired field `{field}`"
            );
        }
    }
    #[test]
    fn mutation_signer_provenance_signature_verifies() {
        let signer = test_mutation_signer();
        let payload = b"soracloud-upload-provenance";
        let provenance = signer.provenance(payload).expect("sign provenance");
        provenance
            .signature
            .verify(&provenance.signer, payload)
            .expect("provenance signature verifies");
    }
    #[test]
    fn parse_signer_accepts_matching_authority_and_key() {
        let expected = test_mutation_signer();
        let authority = AccountId::new(expected.public_key.clone());
        let parsed = parse_signer(&authority.to_string(), &expected.private_key.to_string())
            .expect("matching authority and key");
        assert_eq!(parsed.public_key, expected.public_key);
        assert_eq!(parsed.private_key, expected.private_key);
    }
    #[test]
    fn parse_signer_rejects_authority_key_mismatch() {
        let signer = test_mutation_signer();
        let authority = AccountId::new(signer.public_key.clone());
        let other_private_key = ExposedPrivateKey(
            iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &[0x24; 32])
                .expect("private key"),
        );
        let error = match parse_signer(&authority.to_string(), &other_private_key.to_string()) {
            Ok(_) => panic!("mismatched authority and key must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error, "authority account id does not match the private key");
    }
    #[test]
    fn sign_upload_input_requires_its_exact_first_release_fields() {
        let exact = r#"{"manifest_path":"manifest.json","authority":"authority","private_key":"private-key"}"#;
        json::from_str::<SignUploadInput>(exact).expect("exact signer input");
        for invalid in [
            r#"{"authority":"authority","private_key":"private-key"}"#,
            r#"{"manifest_path":"manifest.json","authority":"authority"}"#,
            r#"{"manifest_path":"manifest.json","authority":"authority","private_key":"private-key","legacy_token":"secret"}"#,
        ] {
            assert!(
                json::from_str::<SignUploadInput>(invalid).is_err(),
                "non-exact signer input must be rejected: {invalid}"
            );
        }
    }
    #[test]
    fn stage_manifest_requires_explicit_nullable_fields_and_schema_version() {
        let exact = exact_stage_manifest_json();
        let manifest = decode_stage_manifest(&exact).expect("exact V1 stage manifest");
        assert_eq!(
            manifest.schema_version,
            STAGE_UPLOAD_MANIFEST_SCHEMA_VERSION_V1
        );
        assert!(manifest.label.is_none());
        assert!(manifest.source_path.is_none());
        assert!(manifest.created_at.is_none());
        assert!(manifest.chunks[0].aad_seed.is_none());

        for field_fragment in [
            "\"schema_version\":1,",
            "\"label\":null,",
            "\"source_path\":null,",
            ",\n                    \"aad_seed\":null",
            ",\n                \"created_at\":null",
        ] {
            let invalid = exact.replacen(field_fragment, "", 1);
            assert!(
                decode_stage_manifest(&invalid).is_err(),
                "missing canonical field `{field_fragment}` must be rejected"
            );
        }
    }
    #[test]
    fn stage_manifest_accepts_only_schema_v1() {
        let exact = exact_stage_manifest_json();
        for version in [0_u16, 2, u16::MAX] {
            let invalid = exact.replacen(
                "\"schema_version\":1",
                &format!("\"schema_version\":{version}"),
                1,
            );
            let error = decode_stage_manifest(&invalid)
                .expect_err("non-V1 stage schema version must be rejected");
            assert!(error.contains("schema_version must equal 1"), "{error}");
        }
    }
    #[test]
    fn stage_manifest_and_nested_objects_reject_unknown_fields() {
        let exact = exact_stage_manifest_json();
        let replacements = [
            ("{", "{\"retired_top_level\":true,"),
            (
                "\"upload_recipient\":{",
                "\"upload_recipient\":{\"retired\":true,",
            ),
            (
                "\"wrapped_bundle_key\":{",
                "\"wrapped_bundle_key\":{\"retired\":true,",
            ),
            (
                "\"pricing_policy\":{",
                "\"pricing_policy\":{\"retired\":true,",
            ),
            (
                "\"compile_profile\":{",
                "\"compile_profile\":{\"retired\":true,",
            ),
            ("\"finalize\":{", "\"finalize\":{\"retired\":true,"),
            ("\"chunks\":[{", "\"chunks\":[{\"retired\":true,"),
        ];
        for (target, replacement) in replacements {
            let invalid = exact.replacen(target, replacement, 1);
            assert!(
                decode_stage_manifest(&invalid).is_err(),
                "unknown field injected through `{target}` must be rejected"
            );
        }
    }
    #[test]
    fn stage_pricing_policy_accepts_only_the_v1_storage_price_input() {
        let policy: StagePricingPolicy =
            json::from_str(r#"{"storage_xor_nanos":11}"#).expect("exact V1 pricing policy");
        assert_eq!(policy.storage_xor_nanos, 11);

        for retired_field in [
            "compile_xor_nanos",
            "runtime_step_xor_nanos",
            "decrypt_release_xor_nanos",
        ] {
            let raw = format!(r#"{{"storage_xor_nanos":11,"{retired_field}":22}}"#);
            assert!(
                json::from_str::<StagePricingPolicy>(&raw).is_err(),
                "retired pricing field `{retired_field}` must be rejected"
            );
        }
    }
    #[test]
    fn package_format_parser_accepts_only_current_v1_label() {
        assert_eq!(
            parse_package_format("NormalizedHuggingFaceSafetensorsV1")
                .expect("normalized safetensors package format"),
            SoraUploadedModelPackageFormatV1::NormalizedHuggingFaceSafetensorsV1
        );
        for invalid in [
            "hf",
            "hf-safetensors",
            "huggingfacesafetensors",
            "HuggingFaceSafetensors",
            " NormalizedHuggingFaceSafetensorsV1",
            "NormalizedHuggingFaceSafetensorsV1 ",
            "DeterministicQuantizedCpuV1",
            "unknown",
        ] {
            assert!(
                parse_package_format(invalid).is_err(),
                "package format alias `{invalid}` must be rejected"
            );
        }
    }
    #[test]
    fn kem_parser_accepts_only_the_current_v1_label() {
        assert_eq!(
            parse_uploaded_model_kem("X25519HkdfSha256").expect("V1 KEM"),
            SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256
        );
        for invalid in [
            "x25519-hkdf-sha256",
            "x25519_hkdf_sha256",
            "x25519hkdfsha256",
            " X25519HkdfSha256",
            "X25519HkdfSha256 ",
            "unknown",
        ] {
            assert!(
                parse_uploaded_model_kem(invalid).is_err(),
                "KEM alias `{invalid}` must be rejected"
            );
        }
    }
    #[test]
    fn aead_parser_accepts_only_the_current_v1_label() {
        assert_eq!(
            parse_uploaded_model_aead("Aes256Gcm").expect("V1 AEAD"),
            SoraUploadedModelKeyWrapAeadV1::Aes256Gcm
        );
        for invalid in [
            "aes-256-gcm",
            "aes_256_gcm",
            "aes256gcm",
            " Aes256Gcm",
            "Aes256Gcm ",
            "unknown",
        ] {
            assert!(
                parse_uploaded_model_aead(invalid).is_err(),
                "AEAD alias `{invalid}` must be rejected"
            );
        }
    }
    #[test]
    fn hash_parser_requires_exact_canonical_hash_text() {
        let expected = Hash::prehashed([0xAB; Hash::LENGTH]);
        let canonical = expected.to_string();
        assert_eq!(
            parse_hash(&canonical, "digest").expect("canonical hash"),
            expected
        );
        for invalid in [
            format!(" {canonical}"),
            format!("{canonical} "),
            format!("0x{canonical}"),
            canonical.to_ascii_uppercase(),
            canonical[..canonical.len() - 2].to_string(),
            "arbitrary-text".to_string(),
        ] {
            assert!(
                parse_hash(&invalid, "digest").is_err(),
                "noncanonical hash `{invalid}` must be rejected"
            );
        }
    }
    #[test]
    fn manifest_digest_parser_requires_exact_lowercase_hex() {
        let canonical = hex::encode([0xA5_u8; 32]);
        assert_eq!(
            parse_manifest_digest(&canonical)
                .expect("canonical manifest digest")
                .as_bytes(),
            &[0xA5; 32]
        );
        for invalid in [
            format!(" {canonical}"),
            format!("{canonical} "),
            format!("0x{canonical}"),
            canonical.to_ascii_uppercase(),
            canonical[..canonical.len() - 2].to_string(),
            format!("{}zz", &canonical[..canonical.len() - 2]),
            "manifest-name".to_string(),
        ] {
            assert!(
                parse_manifest_digest(&invalid).is_err(),
                "noncanonical manifest digest `{invalid}` must be rejected"
            );
        }
    }
    #[test]
    fn derive_upload_bundle_hashes_the_direct_v1_preimage() {
        let tempdir = tempdir().expect("create temp dir");
        let chunk_path = tempdir.path().join("chunk.bin");
        fs::write(&chunk_path, b"ciphertext-chunk").expect("write ciphertext chunk");
        let recipient_public_key_bytes = [7_u8; 32];
        let manifest = StageUploadManifest {
            schema_version: STAGE_UPLOAD_MANIFEST_SCHEMA_VERSION_V1,
            label: None,
            service_name: "service".to_string(),
            model_name: "model".to_string(),
            model_id: "model-id".to_string(),
            artifact_id: "artifact-id".to_string(),
            weight_version: "weights-v1".to_string(),
            family: "test-family".to_string(),
            modalities: vec!["text".to_string()],
            package_format: "NormalizedHuggingFaceSafetensorsV1".to_string(),
            plaintext_root: Hash::new(b"plaintext-root").to_string(),
            sorafs_manifest_digest: hex::encode([0xA5_u8; 32]),
            source_path: None,
            upload_recipient: StageUploadRecipient {
                key_id: "recipient-key".to_string(),
                key_version: 1,
                kem: "X25519HkdfSha256".to_string(),
                aead: "Aes256Gcm".to_string(),
                public_key_bytes_base64: BASE64.encode(recipient_public_key_bytes),
                public_key_fingerprint: Hash::new(recipient_public_key_bytes.as_slice())
                    .to_string(),
            },
            wrapped_bundle_key: StageWrappedBundleKey {
                recipient_key_id: "recipient-key".to_string(),
                recipient_key_version: 1,
                kem: "X25519HkdfSha256".to_string(),
                aead: "Aes256Gcm".to_string(),
                ephemeral_public_key_base64: BASE64.encode([9_u8; 32]),
                nonce_base64: BASE64.encode([11_u8; 12]),
                wrapped_key_ciphertext_base64: BASE64.encode([13_u8; 48]),
                aad_digest: Hash::new(b"wrapped-key-aad").to_string(),
            },
            decryption_policy_ref: "policy-ref".to_string(),
            pricing_policy: StagePricingPolicy {
                storage_xor_nanos: 11,
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
        let expected_bundle_root = hash_encoded(&UploadedModelBundleRootPreimageV1 {
            service_name: derived.bundle.service_name.as_ref().to_owned(),
            model_id: derived.bundle.model_id.clone(),
            weight_version: derived.bundle.weight_version.clone(),
            family: derived.bundle.family.clone(),
            modalities: derived.bundle.modalities.clone(),
            plaintext_root: derived.bundle.plaintext_root,
            package_format: derived.bundle.package_format,
            sorafs_manifest_digest: derived.bundle.sorafs_manifest_digest,
            plaintext_bytes: derived.bundle.plaintext_bytes,
            ciphertext_bytes: derived.bundle.ciphertext_bytes,
            chunk_manifest_root: derived.bundle.chunk_manifest_root,
            upload_recipient: derived.bundle.upload_recipient.clone(),
            wrapped_bundle_key: derived.bundle.wrapped_bundle_key.clone(),
            pricing_policy: derived.bundle.pricing_policy.clone(),
            decryption_policy_ref: derived.bundle.decryption_policy_ref.clone(),
        })
        .expect("hash direct V1 bundle-root preimage");
        assert_eq!(derived.bundle_root, expected_bundle_root);
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
        assert_eq!(
            derived.bundle.pricing_policy.storage_price.to_string(),
            "0.000000011"
        );
        derived.bundle.validate().expect("bundle validates");
        let signer = test_mutation_signer();
        let init_request = SignedUploadedModelBundleInitRequest {
            payload: UploadedModelBundleInitPayload {
                bundle: derived.bundle.clone(),
            },
            provenance: signer.provenance(b"init").expect("sign init provenance"),
        };
        assert_no_inline_signing_fields(&init_request);
        let finalize_request = SignedUploadedModelFinalizeRequest {
            payload: UploadedModelFinalizePayload {
                service_name: manifest.service_name,
                model_name: manifest.model_name,
                model_id: manifest.model_id,
                artifact_id: manifest.artifact_id,
                weight_version: manifest.weight_version,
                bundle_root: derived.bundle_root,
                weight_artifact_hash: derived.weight_artifact_hash,
                dataset_ref: manifest.finalize.dataset_ref,
                training_config_hash: derived.training_config_hash,
                reproducibility_hash: derived.reproducibility_hash,
                provenance_attestation_hash: derived.provenance_attestation_hash,
            },
            provenance: signer
                .provenance(b"finalize")
                .expect("sign finalize provenance"),
        };
        assert_no_inline_signing_fields(&finalize_request);
    }
}
