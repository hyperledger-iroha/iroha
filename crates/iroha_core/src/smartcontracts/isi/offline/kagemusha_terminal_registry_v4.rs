//! Consensus-state resolver for authenticated Kagemusha V4 verifier material.
//!
//! V4 has its own state namespaces, release-record schema, KRV4 framing, and
//! verifier identity.  Nothing in this module accepts or upgrades the V3
//! registry representation.  Release policy is supplied by the build trust
//! root; consensus state can select material, but cannot select its signers.

use std::io::Cursor;

use iroha_crypto::Hash;
use iroha_data_model::{
    name::Name,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1, KAGEMUSHA_VERIFIER_NAMESPACE,
        KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactKindV4,
        KagemushaPastaCycleArtifactV4, KagemushaPastaCycleParityV1,
        KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendArtifactManifestV4,
        KagemushaRecursiveSpendReleaseAttestationV4, KagemushaRecursiveSpendReleasePolicyV1,
    },
    proof::VerifyingKeyRecord,
    zk::BackendTag,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use crate::zk::{
    kagemusha_artifact_v4::{
        KagemushaPastaCycleVerifierArtifactsV4, KagemushaValidatedArtifactPayloadV4,
        kagemusha_artifact_descriptor_v4, read_kagemusha_pasta_cycle_artifact_v4,
    },
    kagemusha_v2::KagemushaPastaCycleOpaqueVerifierV4,
};

const RELEASE_TRUST_ROOT_ENV: &str = "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX";
const EMBEDDED_RELEASE_TRUST_ROOT_HEX: Option<&str> =
    option_env!("IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX");
const TERMINAL_RELEASE_REGISTRY_SCHEMA_V4: &str =
    "kagemusha.offline.recursive_spend.terminal_release_registry.v4";
const TERMINAL_RELEASE_STATE_KEY_PREFIX_V4: &str = "kagemusha_terminal_release_v4_";
const TERMINAL_ARTIFACT_STATE_KEY_PREFIX_V4: &str = "kagemusha_terminal_artifact_v4_";
const VERIFIER_OWNER_MANIFEST_PREFIX_V4: &str = "kagemusha-v4-";
const VERIFIER_IDENTITY_SCHEMA_V4: &str = "kagemusha.offline.recursive_spend.verifier_identity.v4";
const VERIFIER_IDENTITY_VERSION_V4: u16 = 4;
const STEP_EQ_VERIFIER_CURVE_V4: &str = "vesta";
const STEP_EP_VERIFIER_CURVE_V4: &str = "pallas";
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
const MAX_RELEASE_RECORD_BYTES: usize = 2 * KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    + MAX_MANIFEST_BYTES
    + MAX_ATTESTATION_BYTES;

/// Canonical consensus record for one V4 release.
///
/// The release policy is intentionally absent.  Including it here would let a
/// transaction choose both the release and the authorities that approve it.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaTerminalReleaseRegistryRecordV4 {
    pub(crate) schema: String,
    pub(crate) version: u16,
    pub(crate) manifest: KagemushaRecursiveSpendArtifactManifestV4,
    pub(crate) release_attestation: KagemushaRecursiveSpendReleaseAttestationV4,
    pub(crate) benchmark_evidence: Vec<u8>,
    pub(crate) cryptographic_review: Vec<u8>,
}

/// Canonical identity committed by each V4 verifier registry record.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaTerminalVerifierIdentityV4 {
    schema: String,
    version: u16,
    manifest_sha256: [u8; 32],
    parity: KagemushaPastaCycleParityV1,
    circuit_id: String,
    circuit_params_sha256: [u8; 32],
    compiled_protocol_structure_sha256: [u8; 32],
    public_input_limbs: u32,
}

/// Readiness-safe identity derived only from an authenticated V4 release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaAuthenticatedArtifactSetV4 {
    pub(crate) generation: String,
    pub(crate) manifest_sha256: [u8; 32],
    pub(crate) release_policy_sha256: [u8; 32],
    pub(crate) release_attestation_sha256: [u8; 32],
    pub(crate) activation_height: u64,
    pub(crate) withdrawal_height: u64,
    pub(crate) max_proof_bytes: u32,
    pub(crate) asset_scale: u32,
}

/// Authenticated V4 release and exact six-role verifier material.
///
/// The cryptographic parser is deliberately exposed as a fallible constructor:
/// registry authentication and expensive Halo2 key/bootstrap parsing remain
/// separate fail-closed stages, while production callers cannot obtain a V4
/// verifier without passing through both.
#[derive(Debug)]
pub(crate) struct ResolvedKagemushaTerminalVerifierV4 {
    release: KagemushaAuthenticatedReleaseV4,
    artifacts: KagemushaPastaCycleVerifierArtifactsV4,
}

impl ResolvedKagemushaTerminalVerifierV4 {
    pub(crate) fn release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.release
    }

    pub(crate) fn artifacts(&self) -> &KagemushaPastaCycleVerifierArtifactsV4 {
        &self.artifacts
    }

    pub(crate) fn artifact_set(&self) -> KagemushaAuthenticatedArtifactSetV4 {
        let manifest = self.release.manifest();
        KagemushaAuthenticatedArtifactSetV4 {
            generation: manifest.generation.clone(),
            manifest_sha256: self.release.manifest_sha256(),
            release_policy_sha256: self.release.release_policy_sha256(),
            release_attestation_sha256: self.release.release_attestation_sha256(),
            activation_height: manifest.activation_height,
            withdrawal_height: manifest.withdrawal_height,
            max_proof_bytes: manifest.max_proof_bytes,
            asset_scale: manifest.asset_scale,
        }
    }

    pub(crate) fn verifier(&self) -> Result<KagemushaPastaCycleOpaqueVerifierV4, String> {
        ensure_serialized_parameter_degree(
            self.artifacts.step_eq_parameters(),
            self.artifacts.step_eq_profile().ipa_k,
            "Eq",
        )?;
        ensure_serialized_parameter_degree(
            self.artifacts.step_ep_parameters(),
            self.artifacts.step_ep_profile().ipa_k,
            "Ep",
        )?;
        KagemushaPastaCycleOpaqueVerifierV4::from_authenticated_artifacts(&self.artifacts)
    }
}

fn ensure_serialized_parameter_degree(
    bytes: &[u8],
    expected_k: u32,
    role: &str,
) -> Result<(), String> {
    let encoded_k = bytes
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter payload is truncated"))?;
    if encoded_k != expected_k {
        return Err(format!(
            "Kagemusha V4 {role} parameter payload degree does not equal the authenticated profile"
        ));
    }
    Ok(())
}

/// Deterministic V4-only state key for an authenticated release record.
pub(crate) fn release_state_key(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<Name, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    format!(
        "{TERMINAL_RELEASE_STATE_KEY_PREFIX_V4}{}",
        hex::encode(binding.manifest_sha256)
    )
    .parse()
    .map_err(|_| "Kagemusha V4 terminal release state key is invalid".to_owned())
}

/// V4-only content-addressed key for one complete KRV4 framed artifact.
pub(crate) fn artifact_state_key(
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<Name, String> {
    descriptor
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact descriptor: {error}"))?;
    format!(
        "{TERMINAL_ARTIFACT_STATE_KEY_PREFIX_V4}{}",
        hex::encode(descriptor.sha256)
    )
    .parse()
    .map_err(|_| "Kagemusha V4 terminal artifact state key is invalid".to_owned())
}

/// Exact owner-manifest identifier required on V4 verifier records.
pub(crate) fn verifier_owner_manifest_id(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<String, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    Ok(format!(
        "{VERIFIER_OWNER_MANIFEST_PREFIX_V4}{}",
        hex::encode(binding.manifest_sha256)
    ))
}

/// Derive the release- and layout-bound public-input identity stored in a V4
/// [`VerifyingKeyRecord`].
pub(crate) fn verifier_public_inputs_schema_hash(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String> {
    manifest.validate().map_err(|error| error.to_string())?;
    let manifest_bytes = norito::to_bytes(manifest)
        .map_err(|error| format!("failed to encode Kagemusha V4 manifest: {error}"))?;
    let profile = profile(manifest, parity)?;
    let identity = KagemushaTerminalVerifierIdentityV4 {
        schema: VERIFIER_IDENTITY_SCHEMA_V4.to_owned(),
        version: VERIFIER_IDENTITY_VERSION_V4,
        manifest_sha256: Sha256::digest(manifest_bytes).into(),
        parity,
        circuit_id: profile.circuit_id.clone(),
        circuit_params_sha256: profile
            .circuit_params_sha256()
            .map_err(|error| error.to_string())?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        public_input_limbs: profile.circuit_params.public_input_limbs,
    };
    let bytes = norito::to_bytes(&identity)
        .map_err(|error| format!("failed to encode Kagemusha V4 verifier identity: {error}"))?;
    Ok(Hash::new(bytes).into())
}

/// Canonically encode a bounded V4 release record for installation.
pub(crate) fn encode_release_record(
    record: &KagemushaTerminalReleaseRegistryRecordV4,
) -> Result<Vec<u8>, String> {
    let bytes = norito::to_bytes(record).map_err(|error| {
        format!("failed to encode Kagemusha V4 terminal release record: {error}")
    })?;
    if bytes.is_empty() || bytes.len() > MAX_RELEASE_RECORD_BYTES {
        return Err("Kagemusha V4 terminal release record exceeds its bound".to_owned());
    }
    Ok(bytes)
}

/// Decode the build-embedded release policy used by production resolution.
pub(crate) fn embedded_release_policy_bytes() -> Result<Vec<u8>, String> {
    let encoded = EMBEDDED_RELEASE_TRUST_ROOT_HEX.ok_or_else(|| {
        format!(
            "iroha_core was built without {RELEASE_TRUST_ROOT_ENV}; authenticated Kagemusha V4 terminal verification is unavailable"
        )
    })?;
    if encoded.is_empty()
        || encoded.len() > MAX_POLICY_BYTES * 2
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(
            "embedded Kagemusha V4 release trust root is not canonical lowercase Norito hex"
                .to_owned(),
        );
    }
    let decoded = hex::decode(encoded)
        .map_err(|_| "failed to decode embedded Kagemusha V4 release trust root".to_owned())?;
    if decoded.is_empty() || decoded.iter().all(|byte| *byte == 0) {
        return Err("embedded Kagemusha V4 release trust root is empty or all zero".to_owned());
    }
    Ok(decoded)
}

/// Resolve a V4 release using only the build-embedded trust root.
pub(crate) fn resolve<'a, F>(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
    block_height: u64,
    lookup: F,
) -> Result<ResolvedKagemushaTerminalVerifierV4, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let policy_bytes = embedded_release_policy_bytes()?;
    let policy = decode_trusted_policy(&policy_bytes)?;
    resolve_with_policy(
        binding,
        step_eq_record,
        step_ep_record,
        block_height,
        &policy,
        lookup,
    )
}

/// Test-only resolver with an explicit policy encoding.
///
/// This entry point is absent from production builds, so transaction-selected
/// policy bytes cannot reach the V4 resolver.
#[cfg(test)]
fn resolve_with_trusted_policy<'a, F>(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
    block_height: u64,
    trusted_policy_bytes: &[u8],
    lookup: F,
) -> Result<ResolvedKagemushaTerminalVerifierV4, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let policy = decode_trusted_policy(trusted_policy_bytes)?;
    resolve_with_policy(
        binding,
        step_eq_record,
        step_ep_record,
        block_height,
        &policy,
        lookup,
    )
}

/// Resolve and authenticate one exact ABI-20 Eq/Ep verifier release.
fn resolve_with_policy<'a, F>(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
    block_height: u64,
    policy: &KagemushaRecursiveSpendReleasePolicyV1,
    mut lookup: F,
) -> Result<ResolvedKagemushaTerminalVerifierV4, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let release_key = release_state_key(binding)?;
    let release_bytes = lookup(&release_key)
        .ok_or_else(|| "Kagemusha V4 authenticated terminal release is not installed".to_owned())?;
    let record = decode_release_record(release_bytes)?;

    let manifest_bytes = norito::to_bytes(&record.manifest)
        .map_err(|error| format!("failed to encode Kagemusha V4 release manifest: {error}"))?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err("Kagemusha V4 release manifest exceeds its bound".to_owned());
    }
    binding
        .validate_manifest(&record.manifest, &manifest_bytes)
        .map_err(|error| format!("Kagemusha V4 terminal release binding mismatch: {error}"))?;

    let attestation_bytes = norito::to_bytes(&record.release_attestation)
        .map_err(|error| format!("failed to encode Kagemusha V4 release attestation: {error}"))?;
    if attestation_bytes.len() > MAX_ATTESTATION_BYTES {
        return Err("Kagemusha V4 release attestation exceeds its bound".to_owned());
    }
    let release = KagemushaAuthenticatedReleaseV4::verify(
        &record.manifest,
        policy,
        &record.release_attestation,
        &record.benchmark_evidence,
        &record.cryptographic_review,
    )
    .map_err(|error| format!("Kagemusha V4 terminal release authentication failed: {error}"))?;
    if release.manifest_sha256() != binding.manifest_sha256 {
        return Err("Kagemusha V4 authenticated terminal release digest mismatch".to_owned());
    }
    if block_height < release.manifest().activation_height
        || block_height >= release.manifest().withdrawal_height
    {
        return Err("Kagemusha V4 terminal release is outside its activation window".to_owned());
    }

    let step_eq_parameters = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::Parameters,
        &mut lookup,
    )?;
    let step_eq_verifying_key = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        &mut lookup,
    )?;
    let step_eq_bootstrap_witness = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        &mut lookup,
    )?;
    let step_ep_parameters = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::Parameters,
        &mut lookup,
    )?;
    let step_ep_verifying_key = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        &mut lookup,
    )?;
    let step_ep_bootstrap_witness = read_role(
        &release,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        &mut lookup,
    )?;

    ensure_activation_record(
        step_eq_record,
        binding,
        &release,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_verifying_key.payload(),
        block_height,
    )?;
    ensure_activation_record(
        step_ep_record,
        binding,
        &release,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_verifying_key.payload(),
        block_height,
    )?;
    if step_eq_record.commitment == step_ep_record.commitment {
        return Err("Kagemusha V4 Eq/Ep verifier record identities collide".to_owned());
    }

    let artifacts = KagemushaPastaCycleVerifierArtifactsV4::new(
        &release,
        step_eq_parameters,
        step_eq_verifying_key,
        step_eq_bootstrap_witness,
        step_ep_parameters,
        step_ep_verifying_key,
        step_ep_bootstrap_witness,
    )?;
    if artifacts.manifest_sha256() != release.manifest_sha256() {
        return Err("Kagemusha V4 terminal verifier material changed release identity".to_owned());
    }
    Ok(ResolvedKagemushaTerminalVerifierV4 { release, artifacts })
}

fn decode_trusted_policy(bytes: &[u8]) -> Result<KagemushaRecursiveSpendReleasePolicyV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_POLICY_BYTES || bytes.iter().all(|byte| *byte == 0) {
        return Err("Kagemusha V4 trusted release policy is empty or exceeds its bound".to_owned());
    }
    let policy: KagemushaRecursiveSpendReleasePolicyV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha V4 trusted release policy is malformed".to_owned())?;
    if norito::to_bytes(&policy)
        .map_err(|error| format!("failed to encode Kagemusha V4 trusted policy: {error}"))?
        != bytes
    {
        return Err("Kagemusha V4 trusted release policy is not canonical".to_owned());
    }
    policy
        .validate()
        .map_err(|error| format!("Kagemusha V4 trusted release policy is invalid: {error}"))?;
    Ok(policy)
}

fn decode_release_record(bytes: &[u8]) -> Result<KagemushaTerminalReleaseRegistryRecordV4, String> {
    if bytes.is_empty() || bytes.len() > MAX_RELEASE_RECORD_BYTES {
        return Err(
            "Kagemusha V4 terminal release record is empty or exceeds its bound".to_owned(),
        );
    }
    let record: KagemushaTerminalReleaseRegistryRecordV4 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha V4 terminal release record is malformed".to_owned())?;
    if encode_release_record(&record)? != bytes {
        return Err("Kagemusha V4 terminal release record is not canonical".to_owned());
    }
    if record.schema != TERMINAL_RELEASE_REGISTRY_SCHEMA_V4
        || record.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4
        || record.benchmark_evidence.is_empty()
        || record.cryptographic_review.is_empty()
        || record.benchmark_evidence.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
        || record.cryptographic_review.len()
            > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    {
        return Err("Kagemusha V4 terminal release registry record shape mismatch".to_owned());
    }
    Ok(record)
}

fn profile(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<&iroha_data_model::offline::KagemushaPastaCycleProofProfileV4, String> {
    manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or_else(|| "Kagemusha V4 terminal verifier parity is absent".to_owned())
}

fn read_role<'a, F>(
    release: &KagemushaAuthenticatedReleaseV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
    lookup: &mut F,
) -> Result<KagemushaValidatedArtifactPayloadV4, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let descriptor = kagemusha_artifact_descriptor_v4(release.manifest(), parity, kind)?;
    let key = artifact_state_key(descriptor)?;
    let bytes = lookup(&key).ok_or_else(|| {
        format!(
            "Kagemusha V4 terminal artifact `{}` is not installed",
            descriptor.file_name
        )
    })?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes)
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err(format!(
            "Kagemusha V4 terminal artifact `{}` has an invalid stored length",
            descriptor.file_name
        ));
    }
    read_kagemusha_pasta_cycle_artifact_v4(&mut Cursor::new(bytes), release, descriptor)
}

fn ensure_activation_record(
    record: &VerifyingKeyRecord,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    release: &KagemushaAuthenticatedReleaseV4,
    parity: KagemushaPastaCycleParityV1,
    authenticated_vk: &[u8],
    block_height: u64,
) -> Result<(), String> {
    let manifest = release.manifest();
    let profile = profile(manifest, parity)?;
    let (role, expected_curve) = match parity {
        KagemushaPastaCycleParityV1::StepEq => ("Eq", STEP_EQ_VERIFIER_CURVE_V4),
        KagemushaPastaCycleParityV1::StepEp => ("Ep", STEP_EP_VERIFIER_CURVE_V4),
    };
    let expected_owner = verifier_owner_manifest_id(binding)?;
    let expected_schema_hash = verifier_public_inputs_schema_hash(manifest, parity)?;
    if record.version == 0
        || record.circuit_id != profile.circuit_id
        || record.owner_manifest_id.as_deref() != Some(expected_owner.as_str())
        || record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE
        || record.backend != BackendTag::Halo2IpaPasta
        || record.curve != expected_curve
        || record.public_inputs_schema_hash != expected_schema_hash
        || record.commitment == [0; 32]
        || record.max_proof_bytes != manifest.max_proof_bytes
        || record.activation_height != Some(manifest.activation_height)
        || record.withdraw_height != Some(manifest.withdrawal_height)
        || !record.is_active_at(block_height)
    {
        return Err(format!(
            "Kagemusha V4 {role} verifier activation metadata or release identity mismatch"
        ));
    }
    let state_vk = record
        .key
        .as_ref()
        .ok_or_else(|| format!("Kagemusha V4 {role} verifier key is not available inline"))?;
    if state_vk.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        || state_vk.bytes.is_empty()
        || state_vk.bytes.as_slice() != authenticated_vk
        || u32::try_from(authenticated_vk.len()).ok() != Some(record.vk_len)
        || crate::zk::hash_vk(state_vk) != record.commitment
    {
        return Err(format!(
            "Kagemusha V4 {role} state verifier key does not equal the authenticated release payload"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionId,
        confidential::ConfidentialStatus,
        domain::DomainId,
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4, KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4,
            KagemushaPastaCycleFramedArtifactHeaderV4, KagemushaPastaCycleProofProfileV4,
            KagemushaPastaPublicLayoutV4, KagemushaRecursiveSpendReleaseApprovalRoleV1,
            KagemushaRecursiveSpendReleaseApprovalV4, KagemushaRecursiveSpendReleaseAttestationV4,
            KagemushaRecursiveSpendReleaseRolePolicyV1, KagemushaStepCircuitParamsV4,
            KagemushaTopUpFinalityRosterArtifactReferenceV4,
        },
        proof::VerifyingKeyBox,
    };

    use super::*;
    use crate::zk::kagemusha_artifact_v4::{
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4, kagemusha_artifact_file_name_v4,
    };

    const ACTIVATION_HEIGHT: u64 = 7;
    const WITHDRAWAL_HEIGHT: u64 = 100;
    const STEP_PROOF_BYTES: u32 = 4_096;
    const RELEASE_PROOF_BYTES: u32 = 9_000;

    #[derive(Clone)]
    struct Fixture {
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        policy_bytes: Vec<u8>,
        state: BTreeMap<Name, Vec<u8>>,
        release_key: Name,
        role_keys: BTreeMap<
            (
                KagemushaPastaCycleParityV1,
                KagemushaPastaCycleArtifactKindV4,
            ),
            Name,
        >,
        role_frames: BTreeMap<
            (
                KagemushaPastaCycleParityV1,
                KagemushaPastaCycleArtifactKindV4,
            ),
            Vec<u8>,
        >,
        eq_record: VerifyingKeyRecord,
        ep_record: VerifyingKeyRecord,
    }

    fn digest(bytes: impl AsRef<[u8]>) -> [u8; 32] {
        Sha256::digest(bytes.as_ref()).into()
    }

    fn circuit_params() -> KagemushaStepCircuitParamsV4 {
        let k = KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let layout =
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(k).expect("V4 public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: vec![8, 1, 1],
            num_lookup_advice_per_phase: vec![1, 0, 0],
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: STEP_PROOF_BYTES,
        }
    }

    fn role_index(kind: KagemushaPastaCycleArtifactKindV4) -> u8 {
        match kind {
            KagemushaPastaCycleArtifactKindV4::Parameters => 1,
            KagemushaPastaCycleArtifactKindV4::ProvingKey => 2,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey => 3,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness => 4,
        }
    }

    fn profile_and_frames(
        generation: &str,
        parity: KagemushaPastaCycleParityV1,
        seed: u8,
    ) -> (
        KagemushaPastaCycleProofProfileV4,
        Vec<(
            KagemushaPastaCycleArtifactKindV4,
            KagemushaPastaCycleArtifactV4,
            Vec<u8>,
            Vec<u8>,
        )>,
    ) {
        let circuit_params = circuit_params();
        let circuit_id = match parity {
            KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        };
        let parameter_generation = format!("v4-terminal-params-{seed}");
        let compiled_protocol_structure_sha256 = digest([b's', seed]);
        let kinds = [
            KagemushaPastaCycleArtifactKindV4::Parameters,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ];
        let mut artifacts = Vec::with_capacity(kinds.len());
        let mut framed = Vec::with_capacity(kinds.len());
        for kind in kinds {
            let index = role_index(kind);
            let payload = vec![seed.wrapping_add(index); usize::from(48 + index)];
            let header = KagemushaPastaCycleFramedArtifactHeaderV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
                manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
                bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
                proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
                generation: generation.to_owned(),
                parity,
                circuit_id: circuit_id.to_owned(),
                parameter_generation: parameter_generation.clone(),
                ipa_k: circuit_params.k,
                circuit_params_sha256: circuit_params.sha256().expect("V4 params identity"),
                compiled_protocol_structure_sha256,
                step_proof_size_bytes: STEP_PROOF_BYTES,
                kind,
                payload_size_bytes: u64::try_from(payload.len()).expect("small payload"),
                payload_sha256: digest(&payload),
            };
            let header_bytes = norito::to_bytes(&header).expect("canonical KRV4 header");
            let mut frame = Vec::with_capacity(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len()
                    + 4
                    + header_bytes.len()
                    + payload.len(),
            );
            frame.extend_from_slice(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4);
            frame.extend_from_slice(
                &u32::try_from(header_bytes.len())
                    .expect("small KRV4 header")
                    .to_le_bytes(),
            );
            frame.extend_from_slice(&header_bytes);
            frame.extend_from_slice(&payload);
            let descriptor = KagemushaPastaCycleArtifactV4 {
                kind,
                file_name: kagemusha_artifact_file_name_v4(parity, kind).to_owned(),
                size_bytes: u64::try_from(frame.len()).expect("small KRV4 frame"),
                sha256: digest(&frame),
                payload_size_bytes: u64::try_from(payload.len()).expect("small payload"),
                payload_sha256: digest(&payload),
            };
            artifacts.push(descriptor.clone());
            framed.push((kind, descriptor, frame, payload));
        }
        (
            KagemushaPastaCycleProofProfileV4 {
                parity,
                circuit_id: circuit_id.to_owned(),
                parameter_generation,
                ipa_k: circuit_params.k,
                circuit_params,
                compiled_protocol_structure_sha256,
                step_proof_size_bytes: STEP_PROOF_BYTES,
                artifacts,
            },
            framed,
        )
    }

    fn verifier_record(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
        parity: KagemushaPastaCycleParityV1,
        bytes: Vec<u8>,
    ) -> VerifyingKeyRecord {
        let (circuit_id, curve) = match parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
                STEP_EQ_VERIFIER_CURVE_V4,
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                STEP_EP_VERIFIER_CURVE_V4,
            ),
        };
        let key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
                .parse()
                .expect("V4 verifier backend"),
            bytes,
        );
        let mut record = VerifyingKeyRecord::new_with_owner(
            4,
            circuit_id,
            Some(verifier_owner_manifest_id(binding).expect("V4 manifest owner")),
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            curve,
            verifier_public_inputs_schema_hash(manifest, parity).expect("V4 verifier identity"),
            crate::zk::hash_vk(&key),
        );
        record.vk_len = u32::try_from(key.bytes.len()).expect("small VK fixture");
        record.max_proof_bytes = manifest.max_proof_bytes;
        record.activation_height = Some(manifest.activation_height);
        record.withdraw_height = Some(manifest.withdrawal_height);
        record.key = Some(key);
        record.status = ConfidentialStatus::Active;
        record
    }

    fn fixture(seed: u8) -> Fixture {
        let generation = format!("v4-terminal-registry-release-{seed}");
        let benchmark_evidence =
            format!("signed physical-device benchmark evidence {seed}").into_bytes();
        let cryptographic_review =
            format!("independent cryptographic review evidence {seed}").into_bytes();
        let (eq_profile, eq_frames) =
            profile_and_frames(&generation, KagemushaPastaCycleParityV1::StepEq, seed);
        let (ep_profile, ep_frames) = profile_and_frames(
            &generation,
            KagemushaPastaCycleParityV1::StepEp,
            seed.wrapping_add(20),
        );
        let mut manifest = KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: generation.clone(),
            source_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            source_tree_sha256: digest([b't', seed]),
            source_repo_dirty: true,
            chain_id: ChainId::from(format!("v4-terminal-registry-chain-{seed}")),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("asset domain"),
                "rose".parse().expect("asset name"),
            ),
            asset_scale: 9,
            activation_height: ACTIVATION_HEIGHT,
            withdrawal_height: WITHDRAWAL_HEIGHT,
            max_proof_bytes: RELEASE_PROOF_BYTES,
            profiles: vec![eq_profile, ep_profile],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
                size_bytes: 128,
                sha256: digest([b'r', seed]),
                artifact_generation: generation.clone(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            },
            benchmark_evidence_sha256: digest(&benchmark_evidence),
            cryptographic_review_sha256: digest(&cryptographic_review),
            release_attestation_sha256: [0; 32],
        };
        manifest
            .validate_unsigned_candidate()
            .expect("valid unsigned V4 manifest");

        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let key_pairs = [
            KeyPair::from_seed(vec![seed.wrapping_add(51); 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![seed.wrapping_add(52); 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![seed.wrapping_add(53); 32], Algorithm::Ed25519),
        ];
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: format!("v4-terminal-registry-policy-{seed}"),
            roles: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseRolePolicyV1 {
                        role,
                        threshold: 1,
                        authorized_signers: vec![key_pair.public_key().clone()],
                    },
                )
                .collect(),
        };
        policy.validate().expect("valid V4 release trust policy");
        let subject = manifest
            .release_attestation_candidate_subject()
            .expect("V4 release subject");
        let attestation = KagemushaRecursiveSpendReleaseAttestationV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            subject: subject.clone(),
            approvals: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV4 {
                        role,
                        public_key: key_pair.public_key().clone(),
                        signature: SignatureOf::try_new(
                            key_pair.private_key(),
                            &subject.approval_payload(role),
                        )
                        .expect("V4 release signature"),
                    },
                )
                .collect(),
        };
        manifest.release_attestation_sha256 =
            digest(norito::to_bytes(&attestation).expect("V4 attestation bytes"));
        manifest.validate().expect("valid finalized V4 manifest");
        KagemushaAuthenticatedReleaseV4::verify(
            &manifest,
            &policy,
            &attestation,
            &benchmark_evidence,
            &cryptographic_review,
        )
        .expect("authenticated fixture release");

        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            generation,
            manifest_sha256: digest(norito::to_bytes(&manifest).expect("V4 manifest bytes")),
        };
        let release_key = release_state_key(&binding).expect("V4 release state key");
        let record = KagemushaTerminalReleaseRegistryRecordV4 {
            schema: TERMINAL_RELEASE_REGISTRY_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            manifest: manifest.clone(),
            release_attestation: attestation,
            benchmark_evidence,
            cryptographic_review,
        };
        let mut state = BTreeMap::from([(
            release_key.clone(),
            encode_release_record(&record).expect("V4 release record"),
        )]);
        let mut role_keys = BTreeMap::new();
        let mut role_frames = BTreeMap::new();
        let mut eq_vk = None;
        let mut ep_vk = None;
        for (parity, entries) in [
            (KagemushaPastaCycleParityV1::StepEq, eq_frames),
            (KagemushaPastaCycleParityV1::StepEp, ep_frames),
        ] {
            for (kind, descriptor, frame, payload) in entries {
                if kind == KagemushaPastaCycleArtifactKindV4::ProvingKey {
                    continue;
                }
                let key = artifact_state_key(&descriptor).expect("V4 artifact state key");
                role_keys.insert((parity, kind), key.clone());
                role_frames.insert((parity, kind), frame.clone());
                if kind == KagemushaPastaCycleArtifactKindV4::VerifyingKey {
                    match parity {
                        KagemushaPastaCycleParityV1::StepEq => eq_vk = Some(payload),
                        KagemushaPastaCycleParityV1::StepEp => ep_vk = Some(payload),
                    }
                }
                state.insert(key, frame);
            }
        }
        let eq_record = verifier_record(
            &manifest,
            &binding,
            KagemushaPastaCycleParityV1::StepEq,
            eq_vk.expect("Eq VK payload"),
        );
        let ep_record = verifier_record(
            &manifest,
            &binding,
            KagemushaPastaCycleParityV1::StepEp,
            ep_vk.expect("Ep VK payload"),
        );
        Fixture {
            binding,
            policy_bytes: norito::to_bytes(&policy).expect("policy bytes"),
            state,
            release_key,
            role_keys,
            role_frames,
            eq_record,
            ep_record,
        }
    }

    fn resolve_fixture(fixture: &Fixture) -> Result<ResolvedKagemushaTerminalVerifierV4, String> {
        resolve_with_trusted_policy(
            &fixture.binding,
            &fixture.eq_record,
            &fixture.ep_record,
            ACTIVATION_HEIGHT,
            &fixture.policy_bytes,
            |key| fixture.state.get(key).map(Vec::as_slice),
        )
    }

    fn role_key(
        fixture: &Fixture,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Name {
        fixture
            .role_keys
            .get(&(parity, kind))
            .expect("fixture role key")
            .clone()
    }

    #[test]
    fn exact_v4_release_and_six_verifier_roles_resolve() {
        let fixture = fixture(1);
        let resolved = resolve_fixture(&fixture).expect("authenticated V4 verifier material");
        assert_eq!(
            resolved.release().manifest_sha256(),
            fixture.binding.manifest_sha256
        );
        assert_eq!(
            resolved.artifacts().manifest_sha256(),
            fixture.binding.manifest_sha256
        );
        let artifact_set = resolved.artifact_set();
        assert_eq!(artifact_set.generation, fixture.binding.generation);
        assert_eq!(
            artifact_set.manifest_sha256,
            fixture.binding.manifest_sha256
        );
        assert_eq!(
            artifact_set.release_policy_sha256,
            resolved.release().release_policy_sha256()
        );
        assert_eq!(
            artifact_set.release_attestation_sha256,
            resolved.release().release_attestation_sha256()
        );
        assert_eq!(artifact_set.activation_height, ACTIVATION_HEIGHT);
        assert_eq!(artifact_set.withdrawal_height, WITHDRAWAL_HEIGHT);
        assert_eq!(artifact_set.max_proof_bytes, RELEASE_PROOF_BYTES);
        assert_eq!(artifact_set.asset_scale, 9);
        assert_eq!(
            fixture.state.len(),
            7,
            "release plus exactly six verifier frames"
        );
        assert!(
            fixture
                .release_key
                .to_string()
                .starts_with(TERMINAL_RELEASE_STATE_KEY_PREFIX_V4)
        );
        assert!(!fixture.release_key.to_string().contains("release_v1"));
        assert!(fixture.role_keys.values().all(|key| {
            key.to_string()
                .starts_with(TERMINAL_ARTIFACT_STATE_KEY_PREFIX_V4)
                && !key.to_string().contains("artifact_v1")
        }));
        assert!(
            resolved.verifier().is_err(),
            "synthetic payloads must not be promoted to a cryptographic verifier"
        );
    }

    #[test]
    fn v3_magic_and_eq_ep_role_substitution_are_rejected() {
        let mut v3_magic = fixture(2);
        let eq_params_key = role_key(
            &v3_magic,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::Parameters,
        );
        v3_magic
            .state
            .get_mut(&eq_params_key)
            .expect("Eq params frame")[..8]
            .copy_from_slice(b"KRV3KEY\0");
        let error = resolve_fixture(&v3_magic).expect_err("KRV3 frame must reject in V4 state");
        assert!(
            error.contains("magic mismatch"),
            "unexpected error: {error}"
        );

        let mut role_swap = fixture(3);
        let eq_params_key = role_key(
            &role_swap,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::Parameters,
        );
        let ep_params_frame = role_swap
            .role_frames
            .get(&(
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::Parameters,
            ))
            .expect("Ep params frame")
            .clone();
        role_swap.state.insert(eq_params_key, ep_params_frame);
        assert!(
            resolve_fixture(&role_swap).is_err(),
            "Ep material under the Eq content key must reject"
        );
    }

    #[test]
    fn cross_manifest_release_and_artifact_substitution_are_rejected() {
        let mut release_swap = fixture(4);
        let other = fixture(5);
        release_swap.state.insert(
            release_swap.release_key.clone(),
            other
                .state
                .get(&other.release_key)
                .expect("other release record")
                .clone(),
        );
        let error = resolve_fixture(&release_swap).expect_err("cross-manifest release must reject");
        assert!(
            error.contains("binding mismatch"),
            "unexpected error: {error}"
        );

        let mut artifact_swap = fixture(6);
        let other = fixture(7);
        let eq_vk_key = role_key(
            &artifact_swap,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        );
        let other_eq_vk = other
            .role_frames
            .get(&(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ))
            .expect("other Eq VK frame")
            .clone();
        artifact_swap.state.insert(eq_vk_key, other_eq_vk);
        assert!(
            resolve_fixture(&artifact_swap).is_err(),
            "a KRV4 frame from another manifest must reject"
        );
    }

    #[test]
    fn missing_release_and_missing_verifier_role_are_rejected() {
        let mut missing_release = fixture(8);
        missing_release.state.remove(&missing_release.release_key);
        let error = resolve_fixture(&missing_release).expect_err("missing release must reject");
        assert!(error.contains("release is not installed"));

        let mut missing_role = fixture(9);
        let key = role_key(
            &missing_role,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        );
        missing_role.state.remove(&key);
        let error = resolve_fixture(&missing_role).expect_err("missing role must reject");
        assert!(error.contains("artifact") && error.contains("not installed"));
    }

    #[test]
    fn tampered_evidence_frame_and_noncanonical_record_are_rejected() {
        let mut evidence = fixture(10);
        let mut record: KagemushaTerminalReleaseRegistryRecordV4 = norito::decode_from_bytes(
            evidence
                .state
                .get(&evidence.release_key)
                .expect("release record"),
        )
        .expect("decode release record");
        record.cryptographic_review.push(0xA5);
        evidence.state.insert(
            evidence.release_key.clone(),
            encode_release_record(&record).expect("tampered canonical record"),
        );
        let error = resolve_fixture(&evidence).expect_err("tampered evidence must reject");
        assert!(error.contains("authentication failed"));

        let mut frame = fixture(11);
        let key = role_key(
            &frame,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        );
        *frame
            .state
            .get_mut(&key)
            .expect("bootstrap frame")
            .last_mut()
            .expect("nonempty frame") ^= 1;
        let error = resolve_fixture(&frame).expect_err("tampered frame must reject");
        assert!(
            error.contains("digest mismatch"),
            "unexpected error: {error}"
        );

        let mut noncanonical = fixture(12);
        noncanonical
            .state
            .get_mut(&noncanonical.release_key)
            .expect("release record")
            .push(0);
        let error = resolve_fixture(&noncanonical).expect_err("noncanonical record must reject");
        assert!(
            error.contains("malformed") || error.contains("not canonical"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn activation_window_and_exact_record_identity_are_enforced() {
        let fixture = fixture(13);
        for height in [ACTIVATION_HEIGHT - 1, WITHDRAWAL_HEIGHT] {
            let error = resolve_with_trusted_policy(
                &fixture.binding,
                &fixture.eq_record,
                &fixture.ep_record,
                height,
                &fixture.policy_bytes,
                |key| fixture.state.get(key).map(Vec::as_slice),
            )
            .expect_err("height outside release window must reject");
            assert!(
                error.contains("activation window"),
                "unexpected error: {error}"
            );
        }

        let mut wrong_window = fixture.clone();
        wrong_window.eq_record.activation_height = Some(ACTIVATION_HEIGHT + 1);
        let error = resolve_fixture(&wrong_window).expect_err("record window mismatch must reject");
        assert!(error.contains("activation metadata"));

        let mut wrong_owner = fixture.clone();
        wrong_owner.eq_record.owner_manifest_id = Some("kagemusha-v4-substituted".to_owned());
        let error = resolve_fixture(&wrong_owner).expect_err("record owner mismatch must reject");
        assert!(error.contains("release identity"));

        let mut wrong_schema = fixture.clone();
        wrong_schema.ep_record.public_inputs_schema_hash[0] ^= 1;
        let error =
            resolve_fixture(&wrong_schema).expect_err("layout identity mismatch must reject");
        assert!(error.contains("release identity"));

        let mut v3_circuit = fixture;
        v3_circuit.eq_record.circuit_id =
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3.to_owned();
        let error = resolve_fixture(&v3_circuit).expect_err("V3 circuit identity must reject");
        assert!(error.contains("release identity"));
    }

    #[test]
    fn state_cannot_choose_a_substitute_release_policy() {
        let release_fixture = fixture(14);
        let other_policy = fixture(15).policy_bytes;
        let error = resolve_with_trusted_policy(
            &release_fixture.binding,
            &release_fixture.eq_record,
            &release_fixture.ep_record,
            ACTIVATION_HEIGHT,
            &other_policy,
            |key| release_fixture.state.get(key).map(Vec::as_slice),
        )
        .expect_err("untrusted signer policy must reject");
        assert!(error.contains("authentication failed"));
    }

    #[test]
    fn v3_release_record_shape_is_not_reinterpreted_as_v4() {
        let mut fixture = fixture(16);
        let mut record: KagemushaTerminalReleaseRegistryRecordV4 = norito::decode_from_bytes(
            fixture
                .state
                .get(&fixture.release_key)
                .expect("release record"),
        )
        .expect("decode release record");
        record.schema = "kagemusha.offline.recursive_spend.terminal_release_registry.v1".to_owned();
        record.version = KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1;
        fixture.state.insert(
            fixture.release_key.clone(),
            encode_release_record(&record).expect("canonical wrong-version record"),
        );
        let error = resolve_fixture(&fixture).expect_err("V3-shaped record must reject");
        assert!(error.contains("shape mismatch"));
    }
}
