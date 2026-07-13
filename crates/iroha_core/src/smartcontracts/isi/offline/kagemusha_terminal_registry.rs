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
        KagemushaPastaCycleParityV1, KagemushaPastaCycleProofEnvelopeV1,
        KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendArtifactManifestV3,
        KagemushaRecursiveSpendReleaseAttestationV1, KagemushaRecursiveSpendReleasePolicyV1,
    },
    proof::VerifyingKeyRecord,
};
use norito::codec::{Decode, Encode};

use crate::zk::{
    kagemusha_recursion_adapter::KagemushaPastaCycleProofPairV1,
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
        KagemushaPastaCycleProofEnvelopeV1,
        KagemushaPastaCycleProofPairV1,
    ),
    String,
> {
    let envelope: KagemushaPastaCycleProofEnvelopeV1 = norito::decode_from_bytes(envelope_bytes)
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
    let pair: KagemushaPastaCycleProofPairV1 = norito::decode_from_bytes(&envelope.proof.bytes)
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
