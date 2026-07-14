//! Consensus-state resolver for authenticated Kagemusha V3 terminal-verifier material.
//!
//! The release record and every framed verifier artifact are stored under
//! deterministic, content-addressed smart-contract-state keys.  The state
//! record is deliberately not a trust root: release signatures are checked
//! against the policy embedded into this `iroha_core` build.

use std::io::Cursor;

use iroha_data_model::{
    name::Name,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1, KagemushaAuthenticatedReleaseV3,
        KagemushaPastaCycleArtifactKindV3, KagemushaPastaCycleArtifactV3,
        KagemushaPastaCycleParityV1, KagemushaPastaCycleProofEnvelopeV3,
        KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendArtifactManifestV3,
        KagemushaRecursiveSpendReleaseAttestationV1, KagemushaRecursiveSpendReleasePolicyV1,
    },
    proof::VerifyingKeyRecord,
};
use norito::codec::{Decode, Encode};

use crate::zk::{
    kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV3,
    kagemusha_v2::{
        KagemushaPastaCycleVerifierArtifactsV3, read_kagemusha_pasta_cycle_artifact_v3,
    },
};

const RELEASE_TRUST_ROOT_ENV: &str = "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX";
const EMBEDDED_RELEASE_TRUST_ROOT_HEX: Option<&str> =
    option_env!("IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX");
const TERMINAL_RELEASE_REGISTRY_SCHEMA_V1: &str =
    "kagemusha.offline.recursive_spend.terminal_release_registry.v1";
const TERMINAL_RELEASE_STATE_KEY_PREFIX: &str = "kagemusha_terminal_release_v1_";
const TERMINAL_ARTIFACT_STATE_KEY_PREFIX: &str = "kagemusha_terminal_artifact_v1_";
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
const MAX_RELEASE_RECORD_BYTES: usize = 2 * KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    + MAX_MANIFEST_BYTES
    + MAX_ATTESTATION_BYTES;

/// Canonical metadata and evidence stored for one authenticated release.
///
/// The trusted policy is intentionally absent.  Accepting a policy from this
/// record would let a state writer choose both the release and its signers.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaTerminalReleaseRegistryRecordV1 {
    pub(crate) schema: String,
    pub(crate) version: u16,
    pub(crate) manifest: KagemushaRecursiveSpendArtifactManifestV3,
    pub(crate) release_attestation: KagemushaRecursiveSpendReleaseAttestationV1,
    pub(crate) benchmark_evidence: Vec<u8>,
    pub(crate) cryptographic_review: Vec<u8>,
}

/// Fully authenticated release and exact four-role verifier material.
#[derive(Debug)]
pub(crate) struct ResolvedKagemushaTerminalVerifierV3 {
    release: KagemushaAuthenticatedReleaseV3,
    artifacts: KagemushaPastaCycleVerifierArtifactsV3,
}

impl ResolvedKagemushaTerminalVerifierV3 {
    pub(crate) fn release(&self) -> &KagemushaAuthenticatedReleaseV3 {
        &self.release
    }

    pub(crate) fn artifacts(&self) -> &KagemushaPastaCycleVerifierArtifactsV3 {
        &self.artifacts
    }
}

/// Return the deterministic state key for a manifest-bound release record.
pub(crate) fn release_state_key(
    binding: &KagemushaRecursiveSpendArtifactBindingV3,
) -> Result<Name, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha artifact binding: {error}"))?;
    format!(
        "{TERMINAL_RELEASE_STATE_KEY_PREFIX}{}",
        hex::encode(binding.manifest_sha256)
    )
    .parse()
    .map_err(|_| "Kagemusha terminal release state key is invalid".to_owned())
}

/// Return the content-addressed state key for one complete framed artifact.
pub(crate) fn artifact_state_key(
    descriptor: &KagemushaPastaCycleArtifactV3,
) -> Result<Name, String> {
    descriptor
        .validate()
        .map_err(|error| format!("invalid Kagemusha artifact descriptor: {error}"))?;
    format!(
        "{TERMINAL_ARTIFACT_STATE_KEY_PREFIX}{}",
        hex::encode(descriptor.sha256)
    )
    .parse()
    .map_err(|_| "Kagemusha terminal artifact state key is invalid".to_owned())
}

/// Encode one canonical release record for deterministic registry installation.
pub(crate) fn encode_release_record(
    record: &KagemushaTerminalReleaseRegistryRecordV1,
) -> Result<Vec<u8>, String> {
    let bytes = norito::to_bytes(record)
        .map_err(|error| format!("failed to encode Kagemusha terminal release record: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_RELEASE_RECORD_BYTES {
        return Err("Kagemusha terminal release record exceeds its bound".to_owned());
    }
    Ok(bytes)
}

/// Decode the build-embedded, locally trusted release policy.
pub(crate) fn embedded_release_policy_bytes() -> Result<Vec<u8>, String> {
    let encoded = EMBEDDED_RELEASE_TRUST_ROOT_HEX.ok_or_else(|| {
        format!(
            "iroha_core was built without {RELEASE_TRUST_ROOT_ENV}; authenticated Kagemusha terminal verification is unavailable"
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
            "embedded Kagemusha release trust root is not canonical lowercase Norito hex"
                .to_owned(),
        );
    }
    let decoded = hex::decode(encoded)
        .map_err(|_| "failed to decode embedded release trust root".to_owned())?;
    if decoded.is_empty() || decoded.iter().all(|byte| *byte == 0) {
        return Err("embedded Kagemusha release trust root is empty or all zero".to_owned());
    }
    Ok(decoded)
}

/// Resolve and authenticate one exact Eq/Ep terminal-verifier release.
///
/// `lookup` must read the consensus smart-contract-state snapshot used for
/// the surrounding transaction.  The explicit trusted-policy argument exists
/// for deterministic tests; production passes [`embedded_release_policy_bytes`].
pub(crate) fn resolve_with_trusted_policy<'a, F>(
    binding: &KagemushaRecursiveSpendArtifactBindingV3,
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
    trusted_policy_bytes: &[u8],
    mut lookup: F,
) -> Result<ResolvedKagemushaTerminalVerifierV3, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let policy = decode_trusted_policy(trusted_policy_bytes)?;
    let release_key = release_state_key(binding)?;
    let release_bytes = lookup(&release_key)
        .ok_or_else(|| "Kagemusha authenticated terminal release is not installed".to_owned())?;
    let record = decode_release_record(release_bytes)?;

    let manifest_bytes = norito::to_bytes(&record.manifest)
        .map_err(|error| format!("failed to encode Kagemusha release manifest: {error}"))?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err("Kagemusha release manifest exceeds its bound".to_owned());
    }
    binding
        .validate_manifest(&record.manifest, &manifest_bytes)
        .map_err(|error| format!("Kagemusha terminal release binding mismatch: {error}"))?;

    let attestation_bytes = norito::to_bytes(&record.release_attestation)
        .map_err(|error| format!("failed to encode Kagemusha release attestation: {error}"))?;
    if attestation_bytes.len() > MAX_ATTESTATION_BYTES {
        return Err("Kagemusha release attestation exceeds its bound".to_owned());
    }
    let release = KagemushaAuthenticatedReleaseV3::verify(
        &record.manifest,
        &policy,
        &record.release_attestation,
        &record.benchmark_evidence,
        &record.cryptographic_review,
    )
    .map_err(|error| format!("Kagemusha terminal release authentication failed: {error}"))?;
    if release.manifest_sha256() != binding.manifest_sha256 {
        return Err("Kagemusha authenticated terminal release digest mismatch".to_owned());
    }

    ensure_record_release_window(step_eq_record, release.manifest(), "Eq")?;
    ensure_record_release_window(step_ep_record, release.manifest(), "Ep")?;

    let step_eq_parameters = read_role(
        release.manifest(),
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV3::Parameters,
        &mut lookup,
    )?;
    let step_eq_verifying_key = read_role(
        release.manifest(),
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
        &mut lookup,
    )?;
    let step_ep_parameters = read_role(
        release.manifest(),
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV3::Parameters,
        &mut lookup,
    )?;
    let step_ep_verifying_key = read_role(
        release.manifest(),
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
        &mut lookup,
    )?;

    ensure_state_vk_matches(step_eq_record, step_eq_verifying_key.payload(), "Eq")?;
    ensure_state_vk_matches(step_ep_record, step_ep_verifying_key.payload(), "Ep")?;

    let artifacts = KagemushaPastaCycleVerifierArtifactsV3::new(
        release.manifest(),
        step_eq_parameters,
        step_eq_verifying_key,
        step_ep_parameters,
        step_ep_verifying_key,
    )?;
    if artifacts.manifest_sha256() != release.manifest_sha256() {
        return Err("Kagemusha terminal verifier material changed release identity".to_owned());
    }
    Ok(ResolvedKagemushaTerminalVerifierV3 { release, artifacts })
}

/// Canonically decode the outer proof envelope and its exact Eq/Ep proof pair.
pub(crate) fn decode_proof_pair(
    envelope_bytes: &[u8],
) -> Result<
    (
        KagemushaPastaCycleProofEnvelopeV3,
        KagemushaPastaCycleProofPairV3,
    ),
    String,
> {
    let envelope: KagemushaPastaCycleProofEnvelopeV3 = norito::decode_from_bytes(envelope_bytes)
        .map_err(|_| "Kagemusha terminal proof envelope is malformed".to_owned())?;
    if norito::to_bytes(&envelope)
        .map_err(|error| format!("failed to encode Kagemusha terminal proof envelope: {error}"))?
        != envelope_bytes
    {
        return Err("Kagemusha terminal proof envelope is not canonical".to_owned());
    }
    envelope
        .validate()
        .map_err(|error| format!("Kagemusha terminal proof envelope is invalid: {error}"))?;
    let pair: KagemushaPastaCycleProofPairV3 = norito::decode_from_bytes(&envelope.proof.bytes)
        .map_err(|_| "Kagemusha terminal Eq/Ep proof pair is malformed".to_owned())?;
    if norito::to_bytes(&pair)
        .map_err(|error| format!("failed to encode Kagemusha terminal proof pair: {error}"))?
        != envelope.proof.bytes
    {
        return Err("Kagemusha terminal Eq/Ep proof pair is not canonical".to_owned());
    }
    pair.validate()?;
    Ok((envelope, pair))
}

fn decode_trusted_policy(bytes: &[u8]) -> Result<KagemushaRecursiveSpendReleasePolicyV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_POLICY_BYTES || bytes.iter().all(|byte| *byte == 0) {
        return Err("Kagemusha trusted release policy is empty or exceeds its bound".to_owned());
    }
    let policy: KagemushaRecursiveSpendReleasePolicyV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha trusted release policy is malformed".to_owned())?;
    if norito::to_bytes(&policy)
        .map_err(|error| format!("failed to encode Kagemusha trusted release policy: {error}"))?
        != bytes
    {
        return Err("Kagemusha trusted release policy is not canonical".to_owned());
    }
    policy
        .validate()
        .map_err(|error| format!("Kagemusha trusted release policy is invalid: {error}"))?;
    Ok(policy)
}

fn decode_release_record(bytes: &[u8]) -> Result<KagemushaTerminalReleaseRegistryRecordV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_RELEASE_RECORD_BYTES {
        return Err("Kagemusha terminal release record is empty or exceeds its bound".to_owned());
    }
    let record: KagemushaTerminalReleaseRegistryRecordV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha terminal release record is malformed".to_owned())?;
    if encode_release_record(&record)? != bytes {
        return Err("Kagemusha terminal release record is not canonical".to_owned());
    }
    if record.schema != TERMINAL_RELEASE_REGISTRY_SCHEMA_V1
        || record.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1
        || record.benchmark_evidence.is_empty()
        || record.cryptographic_review.is_empty()
        || record.benchmark_evidence.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
        || record.cryptographic_review.len()
            > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    {
        return Err("Kagemusha terminal release registry record shape mismatch".to_owned());
    }
    Ok(record)
}

fn artifact_descriptor(
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV3,
) -> Result<&KagemushaPastaCycleArtifactV3, String> {
    manifest.validate().map_err(|error| error.to_string())?;
    let profile = manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or_else(|| "Kagemusha terminal verifier parity is absent".to_owned())?;
    profile
        .artifacts
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .ok_or_else(|| "Kagemusha terminal verifier artifact role is absent".to_owned())
}

fn read_role<'a, F>(
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV3,
    lookup: &mut F,
) -> Result<crate::zk::kagemusha_v2::KagemushaValidatedArtifactPayloadV3, String>
where
    F: FnMut(&Name) -> Option<&'a [u8]>,
{
    let descriptor = artifact_descriptor(manifest, parity, kind)?;
    let key = artifact_state_key(descriptor)?;
    let bytes = lookup(&key).ok_or_else(|| {
        format!(
            "Kagemusha terminal artifact `{}` is not installed",
            descriptor.file_name
        )
    })?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).ok() != Some(descriptor.size_bytes)
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3)
    {
        return Err(format!(
            "Kagemusha terminal artifact `{}` has an invalid stored length",
            descriptor.file_name
        ));
    }
    read_kagemusha_pasta_cycle_artifact_v3(&mut Cursor::new(bytes), manifest, descriptor)
}

fn ensure_state_vk_matches(
    record: &VerifyingKeyRecord,
    authenticated_vk: &[u8],
    role: &str,
) -> Result<(), String> {
    let state_vk = record
        .key
        .as_ref()
        .ok_or_else(|| format!("Kagemusha {role} verifier key is not available inline"))?;
    if state_vk.bytes.as_slice() != authenticated_vk
        || u32::try_from(authenticated_vk.len()).ok() != Some(record.vk_len)
    {
        return Err(format!(
            "Kagemusha {role} state verifier key does not equal the authenticated release payload"
        ));
    }
    Ok(())
}

fn ensure_record_release_window(
    record: &VerifyingKeyRecord,
    manifest: &KagemushaRecursiveSpendArtifactManifestV3,
    role: &str,
) -> Result<(), String> {
    if record.activation_height != Some(manifest.activation_height)
        || record.withdraw_height != Some(manifest.withdrawal_height)
        || record.max_proof_bytes != manifest.max_proof_bytes
    {
        return Err(format!(
            "Kagemusha {role} verifier activation window does not equal the authenticated release"
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
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
            KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2, KAGEMUSHA_VERIFIER_NAMESPACE,
            KagemushaPastaCycleProofProfileV1, KagemushaRecursiveSpendReleaseApprovalRoleV1,
            KagemushaRecursiveSpendReleaseApprovalV1, KagemushaRecursiveSpendReleaseRolePolicyV1,
            KagemushaTopUpFinalityRosterArtifactReferenceV2,
            kagemusha_recursive_spend_release_sha256,
            kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3,
            kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3,
        },
        proof::VerifyingKeyBox,
        zk::BackendTag,
    };

    use super::*;
    use crate::zk::kagemusha_v2::{
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
        KagemushaRecursiveSpendPastaCycleArtifactsV3,
    };

    struct Fixture {
        binding: KagemushaRecursiveSpendArtifactBindingV3,
        policy_bytes: Vec<u8>,
        state: BTreeMap<Name, Vec<u8>>,
        release_key: Name,
        eq_parameters_key: Name,
        eq_verifier_key: Name,
        ep_verifier_key: Name,
        eq_record: VerifyingKeyRecord,
        ep_record: VerifyingKeyRecord,
        ep_parameters_frame: Vec<u8>,
        ep_verifier_frame: Vec<u8>,
        ep_verifier_payload: Vec<u8>,
    }

    fn fixture() -> Fixture {
        let generation = "terminal-registry-test-release";
        let parameter_generation = "terminal-registry-test-params";
        let benchmark_evidence = b"signed physical-device benchmark evidence".to_vec();
        let cryptographic_review = b"independent cryptographic review evidence".to_vec();
        let roles = [
            (
                KagemushaPastaCycleParityV1::StepEq,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
                [
                    (
                        KagemushaPastaCycleArtifactKindV3::Parameters,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::ProvingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3,
                    ),
                ],
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
                [
                    (
                        KagemushaPastaCycleArtifactKindV3::Parameters,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::ProvingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3,
                    ),
                    (
                        KagemushaPastaCycleArtifactKindV3::VerifyingKey,
                        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3,
                    ),
                ],
            ),
        ];

        let mut inventory = Vec::with_capacity(6);
        let mut profiles = Vec::with_capacity(2);
        for (parity_index, (parity, circuit_id, role_specs)) in roles.into_iter().enumerate() {
            let mut artifacts = Vec::with_capacity(3);
            for (role_index, (kind, file_name)) in role_specs.into_iter().enumerate() {
                let seed = u8::try_from(1 + parity_index * 3 + role_index).expect("fixture seed");
                let payload = vec![seed; 48 + role_index];
                let payload_sha256 = kagemusha_recursive_spend_release_sha256(&payload);
                let header = KagemushaRecursiveSpendPastaCycleArtifactsV3 {
                    version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
                    manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3
                        .to_owned(),
                    bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
                    proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
                    transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1
                        .to_owned(),
                    generation: generation.to_owned(),
                    parity,
                    circuit_id: circuit_id.to_owned(),
                    parameter_generation: parameter_generation.to_owned(),
                    ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                    kind,
                    payload_size_bytes: u64::try_from(payload.len()).expect("payload length"),
                    payload_sha256,
                };
                let header_bytes = norito::to_bytes(&header).expect("artifact header");
                let mut frame = Vec::new();
                frame.extend_from_slice(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3);
                frame.extend_from_slice(
                    &u32::try_from(header_bytes.len())
                        .expect("header length")
                        .to_le_bytes(),
                );
                frame.extend_from_slice(&header_bytes);
                frame.extend_from_slice(&payload);
                let descriptor = KagemushaPastaCycleArtifactV3 {
                    kind,
                    file_name: file_name.to_owned(),
                    size_bytes: u64::try_from(frame.len()).expect("frame length"),
                    sha256: kagemusha_recursive_spend_release_sha256(&frame),
                    payload_size_bytes: u64::try_from(payload.len()).expect("payload length"),
                    payload_sha256,
                };
                artifacts.push(descriptor.clone());
                inventory.push((parity, kind, descriptor, frame, payload));
            }
            profiles.push(KagemushaPastaCycleProofProfileV1 {
                parity,
                circuit_id: circuit_id.to_owned(),
                parameter_generation: parameter_generation.to_owned(),
                ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                artifacts,
            });
        }

        let mut manifest = KagemushaRecursiveSpendArtifactManifestV3 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
            bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
            generation: generation.to_owned(),
            source_commit: "1234567890abcdef1234567890abcdef12345678".to_owned(),
            source_tree_sha256: [0x51; 32],
            source_repo_dirty: true,
            chain_id: ChainId::from("terminal-registry-test-chain"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").expect("asset domain"),
                "rose".parse().expect("asset name"),
            ),
            asset_scale: 9,
            activation_height: 7,
            withdrawal_height: 100,
            max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
            profiles,
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2.to_owned(),
                size_bytes: 128,
                sha256: kagemusha_recursive_spend_release_sha256(b"terminal-test-roster"),
                artifact_generation: generation.to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
            },
            benchmark_evidence_sha256: kagemusha_recursive_spend_release_sha256(
                &benchmark_evidence,
            ),
            cryptographic_review_sha256: kagemusha_recursive_spend_release_sha256(
                &cryptographic_review,
            ),
            release_attestation_sha256: [0xA5; 32],
        };

        let key_pairs = [
            KeyPair::from_seed(vec![11; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519),
        ];
        let approval_roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "terminal-registry-test-policy".to_owned(),
            roles: approval_roles
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
        let subject = manifest
            .release_attestation_subject()
            .expect("release subject");
        let attestation = KagemushaRecursiveSpendReleaseAttestationV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            subject: subject.clone(),
            approvals: approval_roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV1 {
                        role,
                        public_key: key_pair.public_key().clone(),
                        signature: SignatureOf::try_new(
                            key_pair.private_key(),
                            &subject.approval_payload(role),
                        )
                        .expect("release signature"),
                    },
                )
                .collect(),
        };
        manifest.release_attestation_sha256 = kagemusha_recursive_spend_release_sha256(
            &norito::to_bytes(&attestation).expect("release attestation"),
        );
        manifest.validate().expect("release manifest");

        let binding = KagemushaRecursiveSpendArtifactBindingV3 {
            generation: generation.to_owned(),
            manifest_sha256: kagemusha_recursive_spend_release_sha256(
                &norito::to_bytes(&manifest).expect("manifest bytes"),
            ),
        };
        let release_key = release_state_key(&binding).expect("release state key");
        let record = KagemushaTerminalReleaseRegistryRecordV1 {
            schema: TERMINAL_RELEASE_REGISTRY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            manifest: manifest.clone(),
            release_attestation: attestation,
            benchmark_evidence,
            cryptographic_review,
        };
        let mut state = BTreeMap::new();
        state.insert(
            release_key.clone(),
            encode_release_record(&record).expect("release record"),
        );

        let mut eq_parameters_key = None;
        let mut eq_verifier_key = None;
        let mut ep_verifier_key = None;
        let mut eq_verifier_payload = None;
        let mut ep_verifier_payload = None;
        let mut ep_parameters_frame = None;
        let mut ep_verifier_frame = None;
        for (parity, kind, descriptor, frame, payload) in inventory {
            if kind == KagemushaPastaCycleArtifactKindV3::ProvingKey {
                continue;
            }
            let key = artifact_state_key(&descriptor).expect("artifact key");
            if kind == KagemushaPastaCycleArtifactKindV3::Parameters {
                match parity {
                    KagemushaPastaCycleParityV1::StepEq => {
                        eq_parameters_key = Some(key.clone());
                    }
                    KagemushaPastaCycleParityV1::StepEp => {
                        ep_parameters_frame = Some(frame.clone());
                    }
                }
            } else if kind == KagemushaPastaCycleArtifactKindV3::VerifyingKey {
                match parity {
                    KagemushaPastaCycleParityV1::StepEq => {
                        eq_verifier_key = Some(key.clone());
                        eq_verifier_payload = Some(payload);
                    }
                    KagemushaPastaCycleParityV1::StepEp => {
                        ep_verifier_key = Some(key.clone());
                        ep_verifier_payload = Some(payload);
                        ep_verifier_frame = Some(frame.clone());
                    }
                }
            }
            state.insert(key, frame);
        }
        let eq_verifier_payload = eq_verifier_payload.expect("Eq verifier payload");
        let ep_verifier_payload = ep_verifier_payload.expect("Ep verifier payload");
        let eq_record = verifier_record(
            &manifest,
            KagemushaPastaCycleParityV1::StepEq,
            eq_verifier_payload,
        );
        let ep_record = verifier_record(
            &manifest,
            KagemushaPastaCycleParityV1::StepEp,
            ep_verifier_payload.clone(),
        );

        Fixture {
            binding,
            policy_bytes: norito::to_bytes(&policy).expect("policy bytes"),
            state,
            release_key,
            eq_parameters_key: eq_parameters_key.expect("Eq parameters key"),
            eq_verifier_key: eq_verifier_key.expect("Eq verifier key"),
            ep_verifier_key: ep_verifier_key.expect("Ep verifier key"),
            eq_record,
            ep_record,
            ep_parameters_frame: ep_parameters_frame.expect("Ep parameters frame"),
            ep_verifier_frame: ep_verifier_frame.expect("Ep verifier frame"),
            ep_verifier_payload,
        }
    }

    fn verifier_record(
        manifest: &KagemushaRecursiveSpendArtifactManifestV3,
        parity: KagemushaPastaCycleParityV1,
        bytes: Vec<u8>,
    ) -> VerifyingKeyRecord {
        let (circuit, curve, schema_hash) = match parity {
            KagemushaPastaCycleParityV1::StepEq => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3,
                kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v3(),
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3,
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3,
                kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v3(),
            ),
        };
        let key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1
                .parse()
                .expect("key backend"),
            bytes,
        );
        let mut record = VerifyingKeyRecord::new_with_owner(
            1,
            circuit,
            None,
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            curve,
            schema_hash,
            crate::zk::hash_vk(&key),
        );
        record.vk_len = u32::try_from(key.bytes.len()).expect("VK length");
        record.max_proof_bytes = manifest.max_proof_bytes;
        record.activation_height = Some(manifest.activation_height);
        record.withdraw_height = Some(manifest.withdrawal_height);
        record.key = Some(key);
        record.status = ConfidentialStatus::Active;
        record
    }

    fn resolve(fixture: &Fixture) -> Result<ResolvedKagemushaTerminalVerifierV3, String> {
        resolve_with_trusted_policy(
            &fixture.binding,
            &fixture.eq_record,
            &fixture.ep_record,
            &fixture.policy_bytes,
            |key| fixture.state.get(key).map(Vec::as_slice),
        )
    }

    #[test]
    fn exact_manifest_roles_and_state_keys_resolve() {
        let fixture = fixture();
        let resolved = resolve(&fixture).expect("authenticated terminal verifier material");
        assert_eq!(
            resolved.release().manifest_sha256(),
            fixture.binding.manifest_sha256
        );
        assert_eq!(
            resolved.artifacts().manifest_sha256(),
            fixture.binding.manifest_sha256
        );
        assert_ne!(fixture.eq_verifier_key, fixture.ep_verifier_key);
    }

    #[test]
    fn framed_eq_ep_role_substitution_is_rejected() {
        let mut parameter_fixture = fixture();
        parameter_fixture.state.insert(
            parameter_fixture.eq_parameters_key.clone(),
            parameter_fixture.ep_parameters_frame.clone(),
        );
        assert!(
            resolve(&parameter_fixture).is_err(),
            "Ep parameters under the Eq digest key must reject"
        );

        let mut verifier_fixture = fixture();
        verifier_fixture.state.insert(
            verifier_fixture.eq_verifier_key.clone(),
            verifier_fixture.ep_verifier_frame.clone(),
        );
        let error =
            resolve(&verifier_fixture).expect_err("Ep frame under Eq digest key must reject");
        assert!(
            error.contains("length") || error.contains("digest") || error.contains("mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn active_state_vk_substitution_is_rejected() {
        let mut fixture = fixture();
        let substituted = fixture.ep_verifier_payload.clone();
        fixture.eq_record.vk_len = u32::try_from(substituted.len()).expect("VK length");
        fixture.eq_record.key.as_mut().expect("inline Eq key").bytes = substituted;
        let error = resolve(&fixture).expect_err("state VK substitution must reject");
        assert!(error.contains("does not equal the authenticated release payload"));
    }

    #[test]
    fn canonical_but_substituted_release_evidence_is_rejected() {
        let mut fixture = fixture();
        let mut record: KagemushaTerminalReleaseRegistryRecordV1 = norito::decode_from_bytes(
            fixture
                .state
                .get(&fixture.release_key)
                .expect("release record"),
        )
        .expect("decode release record");
        record.cryptographic_review = b"substituted review evidence".to_vec();
        fixture.state.insert(
            fixture.release_key.clone(),
            encode_release_record(&record).expect("modified record"),
        );
        let error = resolve(&fixture).expect_err("evidence substitution must reject");
        assert!(error.contains("authentication failed"));
    }

    #[test]
    fn manifest_binding_substitution_is_rejected_even_when_record_is_copied() {
        let mut fixture = fixture();
        let original_record = fixture
            .state
            .get(&fixture.release_key)
            .expect("release record")
            .clone();
        fixture.binding.manifest_sha256[0] ^= 1;
        let substituted_key = release_state_key(&fixture.binding).expect("substituted key");
        fixture.state.insert(substituted_key, original_record);
        let error = resolve(&fixture).expect_err("manifest substitution must reject");
        assert!(error.contains("binding mismatch"));
    }

    #[test]
    fn noncanonical_release_record_and_artifact_trailing_bytes_are_rejected() {
        let mut release_fixture = fixture();
        release_fixture
            .state
            .get_mut(&release_fixture.release_key)
            .expect("release record")
            .push(0);
        assert!(resolve(&release_fixture).is_err());

        let mut artifact_fixture = fixture();
        artifact_fixture
            .state
            .get_mut(&artifact_fixture.ep_verifier_key)
            .expect("Ep verifier frame")
            .push(0);
        assert!(resolve(&artifact_fixture).is_err());
    }

    #[test]
    fn noncanonical_trusted_policy_is_rejected_before_state_selection() {
        let mut fixture = fixture();
        fixture.policy_bytes.push(0);
        let error = resolve(&fixture).expect_err("trailing trusted-policy bytes must reject");
        assert!(error.contains("policy"));
    }

    #[test]
    fn canonical_but_different_trusted_policy_is_rejected() {
        let mut fixture = fixture();
        let mut substituted_policy: KagemushaRecursiveSpendReleasePolicyV1 =
            norito::decode_from_bytes(&fixture.policy_bytes).expect("decode trusted policy");
        for (index, role) in substituted_policy.roles.iter_mut().enumerate() {
            let replacement = KeyPair::from_seed(
                vec![u8::try_from(0x40 + index).expect("replacement seed"); 32],
                Algorithm::Ed25519,
            );
            role.authorized_signers = vec![replacement.public_key().clone()];
        }
        substituted_policy
            .validate()
            .expect("canonical substituted policy");
        fixture.policy_bytes =
            norito::to_bytes(&substituted_policy).expect("substituted policy bytes");

        let error = resolve(&fixture).expect_err("different trusted signers must reject release");
        assert!(error.contains("authentication failed"));
    }

    #[test]
    fn eq_ep_activation_metadata_substitution_is_rejected() {
        for mutation in [
            "eq_activation",
            "eq_withdrawal",
            "ep_activation",
            "ep_withdrawal",
        ] {
            let mut fixture = fixture();
            let field = match mutation {
                "eq_activation" => &mut fixture.eq_record.activation_height,
                "eq_withdrawal" => &mut fixture.eq_record.withdraw_height,
                "ep_activation" => &mut fixture.ep_record.activation_height,
                "ep_withdrawal" => &mut fixture.ep_record.withdraw_height,
                _ => unreachable!(),
            };
            *field = field.and_then(|height| height.checked_add(1));
            let error = resolve(&fixture)
                .expect_err("substituted verifier activation metadata must reject");
            assert!(
                error.contains("activation window"),
                "unexpected {mutation} error: {error}"
            );
        }
    }

    #[test]
    fn verifier_window_is_activation_inclusive_and_withdrawal_exclusive() {
        let fixture = fixture();
        for record in [&fixture.eq_record, &fixture.ep_record] {
            let activation = record.activation_height.expect("activation height");
            let withdrawal = record.withdraw_height.expect("withdrawal height");
            assert!(!record.is_active_at(activation - 1));
            assert!(record.is_active_at(activation));
            assert!(record.is_active_at(withdrawal - 1));
            assert!(!record.is_active_at(withdrawal));
        }
    }

    #[test]
    fn missing_release_or_artifact_state_key_is_rejected() {
        let mut release_fixture = fixture();
        release_fixture.state.remove(&release_fixture.release_key);
        let error = resolve(&release_fixture).expect_err("missing release record must reject");
        assert!(error.contains("release is not installed"));

        let mut artifact_fixture = fixture();
        artifact_fixture
            .state
            .remove(&artifact_fixture.eq_parameters_key);
        let error = resolve(&artifact_fixture).expect_err("missing Eq parameters must reject");
        assert!(error.contains("artifact") && error.contains("not installed"));
    }
}
