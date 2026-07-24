//! Finalized chain-authoritative SoraFS PDP and PoTR outcome handlers.

use core::convert::TryFrom;
use std::{str::FromStr, sync::OnceLock};

use iroha_crypto::{Algorithm, PublicKey, ed25519_parse_signature};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            SetSorafsProofOutcomeSignerPolicy, SorafsProofOutcomeSubmissionV1,
            SubmitSorafsProofOutcome,
        },
    },
    name::Name,
    permission::Permission,
    query::{error::QueryExecutionFail, sorafs::prelude::FindSorafsProofOutcomeEvents},
    sorafs::{
        capacity::ProviderId,
        pin_registry::ManifestDigest,
        proof_ledger::{
            PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1, PROOF_OUTCOME_MAX_PROVIDER_KEY_BYTES_V1,
            PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1, PROOF_OUTCOME_QUERY_MAX_ITEMS_V1,
            PROOF_OUTCOME_RECORD_VERSION_V1, PROOF_OUTCOME_SIGNER_POLICY_VERSION_V1,
            PdpOutcomeProjectionV1, PdpOutcomeStatusV1, PotrOutcomeProjectionV1,
            PotrOutcomeStatusV1, ProofOutcomeEd25519AttestationV1,
            ProofOutcomeFinalizedCursorV1, ProofOutcomeFinalizedEventPageV1,
            ProofOutcomeFinalizedEventV1, ProofOutcomeKindV1, ProofOutcomeProjectionV1,
            ProofOutcomeRecordV1, ProofOutcomeSignerPolicyRecordV1, ProofOutcomeSignerPolicyV1,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanManageSorafsProofOutcomePolicy, CanRecordSorafsProofOutcome,
};
use mv::storage::{StorageReadOnly, Transaction as StorageTransaction};
use norito::{DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::{
    PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1, PDP_PROOF_MAX_CANONICAL_BYTES_V1,
    PDP_PROOF_SIGNATURE_DOMAIN_V1, PdpGovernanceArchiveV1, PdpProofV1, PdpRejectionReasonV1,
    PdpTerminalDecisionV1, PotrReceiptV1, PotrStatus,
};

use super::*;
use crate::{
    smartcontracts::ValidSingularQuery,
    state::{StateTransaction, WorldReadOnly},
};

const POLICY_STATE_KEY_PREFIX: &str = "sorafs_proof_outcome_policy_v1_";
const OUTCOME_STATE_KEY_PREFIX: &str = "sorafs_proof_outcome_v1_";
const EVENT_STATE_KEY_PREFIX: &str = "sorafs_proof_outcome_event_v1_";
const EVENT_JOURNAL_HEAD_STATE_KEY: &str = "sorafs_proof_outcome_event_head_v1";
const POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.proof-outcome.signer-policy.v1\0";
const STATE_MAX_BYTES: usize = 128 * 1024;
const QUERY_MAX_STATE_READ_BYTES: usize = 16 * 1024 * 1024;
const STATE_LIMITS: DecodeLimits =
    DecodeLimits::new(128 * 1024, STATE_MAX_BYTES, 256 * 1024, 2 * STATE_MAX_BYTES, 64);
const PDP_ARCHIVE_LIMITS: DecodeLimits = DecodeLimits::new(
    128 * 1024,
    PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1,
    512 * 1024,
    2 * PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1,
    128,
);
const PDP_PROOF_LIMITS: DecodeLimits = DecodeLimits::new(
    128 * 1024,
    PDP_PROOF_MAX_CANONICAL_BYTES_V1,
    512 * 1024,
    2 * PDP_PROOF_MAX_CANONICAL_BYTES_V1,
    128,
);
const POTR_RECEIPT_LIMITS: DecodeLimits = DecodeLimits::new(
    PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    2 * PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    2 * PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    32,
);

#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ProofOutcomePersistedEventV1 {
    sequence: u64,
    target_block_height: u64,
    event_index: u32,
    outcome: ProofOutcomeRecordV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ProofOutcomeEventJournalHeadV1 {
    last_sequence: u64,
    last_target_block_height: u64,
    last_event_index: u32,
}

struct PreparedOutcome {
    identity_digest: [u8; 32],
    outcome_digest: [u8; 32],
    provider_id: ProviderId,
    manifest_digest: ManifestDigest,
    admission_envelope_digest: [u8; 32],
    projection: ProofOutcomeProjectionV1,
    has_provider_proof: bool,
}

impl PreparedOutcome {
    fn into_record(self, submitted_by: AccountId, committed_at_unix_ms: u64) -> ProofOutcomeRecordV1 {
        ProofOutcomeRecordV1 {
            version: PROOF_OUTCOME_RECORD_VERSION_V1,
            identity_digest: self.identity_digest,
            outcome_digest: self.outcome_digest,
            provider_id: self.provider_id,
            manifest_digest: self.manifest_digest,
            admission_envelope_digest: self.admission_envelope_digest,
            submitted_by,
            committed_at_unix_ms,
            projection: self.projection,
        }
    }
}

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn corrupt_state(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn policy_key(provider_id: ProviderId) -> Name {
    Name::from_str(&format!(
        "{POLICY_STATE_KEY_PREFIX}{}",
        hex::encode(provider_id.as_bytes())
    ))
    .expect("static prefix plus provider hex is a valid state key")
}

fn outcome_key(kind: ProofOutcomeKindV1, identity_digest: [u8; 32]) -> Name {
    let kind = match kind {
        ProofOutcomeKindV1::Pdp => "pdp",
        ProofOutcomeKindV1::Potr => "potr",
    };
    Name::from_str(&format!(
        "{OUTCOME_STATE_KEY_PREFIX}{kind}_{}",
        hex::encode(identity_digest)
    ))
    .expect("static prefix plus proof kind and digest is a valid state key")
}

fn event_key(sequence: u64) -> Name {
    Name::from_str(&format!("{EVENT_STATE_KEY_PREFIX}{sequence:016x}"))
        .expect("static prefix plus fixed-width lowercase hex is a valid state key")
}

fn event_journal_head_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        Name::from_str(EVENT_JOURNAL_HEAD_STATE_KEY)
            .expect("static proof-outcome event journal head key is valid")
    })
}

fn encode_state<T: norito::core::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, InstructionExecutionError> {
    let bytes = norito::to_bytes(value)
        .map_err(|error| corrupt_state(format!("failed to encode {label}: {error}")))?;
    if bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} encodes to {} bytes, above {STATE_MAX_BYTES}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

fn decode_state<T>(bytes: &[u8], label: &str) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.is_empty() || bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} length {} is outside 1..={STATE_MAX_BYTES}",
            bytes.len()
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, STATE_LIMITS)
        .map_err(|error| corrupt_state(format!("failed to decode {label}: {error}")))?;
    if encode_state(&value, label)? != bytes {
        return Err(corrupt_state(format!(
            "{label} is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn decode_payload<T>(
    bytes: &[u8],
    maximum: usize,
    limits: DecodeLimits,
    label: &str,
) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid_parameter(format!(
            "{label} length {} is outside 1..={maximum}",
            bytes.len()
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, limits)
        .map_err(|error| invalid_parameter(format!("failed to decode canonical {label}: {error}")))?;
    let canonical = norito::to_bytes(&value)
        .map_err(|error| invalid_parameter(format!("failed to encode canonical {label}: {error}")))?;
    if canonical != bytes {
        return Err(invalid_parameter(format!(
            "{label} is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn block_time_ms(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let now = state_transaction.block_unix_timestamp_ms();
    if now == 0 {
        return Err(invalid_parameter(
            "proof-outcome operations require a non-zero block timestamp",
        ));
    }
    Ok(now)
}

fn has_named_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission_name: &str,
) -> bool {
    if state_transaction._curr_block.is_genesis() {
        return true;
    }
    let direct = state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|permission| permission.name() == permission_name)
        });
    direct
        || state_transaction
            .world
            .account_roles_iter(authority)
            .filter_map(|role_id| state_transaction.world.roles.get(role_id))
            .any(|role| {
                role.permissions()
                    .any(|permission| permission.name() == permission_name)
            })
}

fn has_scheduler_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    provider_id: ProviderId,
) -> bool {
    if state_transaction._curr_block.is_genesis() {
        return true;
    }
    let required = Permission::from(CanRecordSorafsProofOutcome { provider_id });
    let direct = state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.contains(&required));
    direct
        || state_transaction
            .world
            .account_roles_iter(authority)
            .filter_map(|role_id| state_transaction.world.roles.get(role_id))
            .any(|role| role.permissions().any(|permission| permission == &required))
}

fn validate_ed25519_public_key(
    bytes: &[u8; 32],
    label: &str,
) -> Result<(), InstructionExecutionError> {
    PublicKey::from_bytes(Algorithm::Ed25519, bytes)
        .map(|_| ())
        .map_err(|error| invalid_parameter(format!("invalid {label}: {error}")))
}

fn validate_mldsa_public_key(
    bytes: &[u8],
    label: &str,
) -> Result<(), InstructionExecutionError> {
    if bytes.is_empty() || bytes.len() > PROOF_OUTCOME_MAX_PROVIDER_KEY_BYTES_V1 {
        return Err(invalid_parameter(format!(
            "{label} length {} is outside 1..={PROOF_OUTCOME_MAX_PROVIDER_KEY_BYTES_V1}",
            bytes.len()
        )));
    }
    PublicKey::from_bytes(Algorithm::MlDsa, bytes)
        .map(|_| ())
        .map_err(|error| invalid_parameter(format!("invalid {label}: {error}")))
}

fn policy_digest(
    policy: &ProofOutcomeSignerPolicyV1,
) -> Result<[u8; 32], InstructionExecutionError> {
    let bytes = norito::to_bytes(policy)
        .map_err(|error| invalid_parameter(format!("failed to encode signer policy: {error}")))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(POLICY_DIGEST_DOMAIN_V1);
    hasher.update(
        &u64::try_from(bytes.len())
            .expect("slice length fits u64")
            .to_le_bytes(),
    );
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn validate_signer_policy(
    policy: &ProofOutcomeSignerPolicyV1,
) -> Result<(), InstructionExecutionError> {
    if policy.version != PROOF_OUTCOME_SIGNER_POLICY_VERSION_V1
        || policy.provider_id.as_bytes() == &[0; 32]
        || policy.revision == 0
        || policy.admission_envelope_digest == [0; 32]
        || policy.valid_from_unix == 0
        || policy.valid_until_unix < policy.valid_from_unix
        || (policy.revision == 1) != policy.predecessor_digest.is_none()
        || policy.predecessor_digest == Some([0; 32])
    {
        return Err(invalid_parameter(
            "proof-outcome signer policy identity, revision, predecessor, or validity window is invalid",
        ));
    }
    validate_ed25519_public_key(&policy.pdp_public_key, "PDP provider public key")?;
    validate_mldsa_public_key(
        &policy.potr_mldsa_public_key,
        "PoTR provider ML-DSA public key",
    )?;
    validate_ed25519_public_key(&policy.gateway_public_key, "PoTR gateway public key")
}

fn read_signer_policy(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<Option<ProofOutcomeSignerPolicyRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&policy_key(provider_id)) else {
        return Ok(None);
    };
    let record: ProofOutcomeSignerPolicyRecordV1 =
        decode_state(bytes, "proof-outcome signer policy")?;
    validate_signer_policy(&record.policy)
        .map_err(|error| corrupt_state(format!("stored signer policy is invalid: {error}")))?;
    if record.policy.provider_id != provider_id
        || record.policy_digest == [0; 32]
        || record.policy_digest != policy_digest(&record.policy)?
        || record.activated_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored proof-outcome signer policy provenance is inconsistent",
        ));
    }
    Ok(Some(record))
}

fn map_pdp_status(decision: PdpTerminalDecisionV1) -> PdpOutcomeStatusV1 {
    match decision {
        PdpTerminalDecisionV1::Accepted => PdpOutcomeStatusV1::Accepted,
        PdpTerminalDecisionV1::Rejected(reason) => match reason {
            PdpRejectionReasonV1::DeadlineExpired => PdpOutcomeStatusV1::DeadlineExpired,
            PdpRejectionReasonV1::SubmissionLate => PdpOutcomeStatusV1::SubmissionLate,
            PdpRejectionReasonV1::FutureTimestamp => PdpOutcomeStatusV1::FutureTimestamp,
            PdpRejectionReasonV1::InvalidProof => PdpOutcomeStatusV1::InvalidProof,
            PdpRejectionReasonV1::AdmissionRevoked => PdpOutcomeStatusV1::AdmissionRevoked,
            PdpRejectionReasonV1::AdmissionInactive => PdpOutcomeStatusV1::AdmissionInactive,
            PdpRejectionReasonV1::StorageUnavailable => PdpOutcomeStatusV1::StorageUnavailable,
        },
    }
}

fn map_potr_status(status: PotrStatus) -> PotrOutcomeStatusV1 {
    match status {
        PotrStatus::Success => PotrOutcomeStatusV1::Success,
        PotrStatus::MissedDeadline => PotrOutcomeStatusV1::MissedDeadline,
        PotrStatus::ProviderError => PotrOutcomeStatusV1::ProviderError,
        PotrStatus::GatewayError => PotrOutcomeStatusV1::GatewayError,
        PotrStatus::ClientCancelled => PotrOutcomeStatusV1::ClientCancelled,
    }
}

fn prepare_pdp_outcome(
    archive_payload: &[u8],
) -> Result<PreparedOutcome, InstructionExecutionError> {
    let archive: PdpGovernanceArchiveV1 = decode_payload(
        archive_payload,
        PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1,
        PDP_ARCHIVE_LIMITS,
        "PDP governance archive",
    )?;
    archive
        .validate()
        .map_err(|error| invalid_parameter(format!("invalid PDP governance archive: {error}")))?;
    let outcome_digest = archive
        .digest()
        .map_err(|error| invalid_parameter(format!("failed to digest PDP archive: {error}")))?;
    let provider_attestation = archive
        .canonical_proof
        .as_deref()
        .map(|proof_payload| {
            let proof: PdpProofV1 = decode_payload(
                proof_payload,
                PDP_PROOF_MAX_CANONICAL_BYTES_V1,
                PDP_PROOF_LIMITS,
                "PDP proof",
            )?;
            proof
                .verify_signature()
                .map_err(|error| invalid_parameter(format!("invalid PDP proof signature: {error}")))?;
            if Some(
                proof
                    .proof_digest()
                    .map_err(|error| invalid_parameter(format!("failed to digest PDP proof: {error}")))?,
            ) != archive.proof_digest
            {
                return Err(invalid_parameter(
                    "PDP proof digest disagrees with governance archive",
                ));
            }
            Ok(ProofOutcomeEd25519AttestationV1 {
                public_key: proof.signature.public_key,
                signature: proof.signature.signature,
            })
        })
        .transpose()?;
    let status = map_pdp_status(archive.decision);
    if status.requires_proof() != provider_attestation.is_some() {
        return Err(invalid_parameter(
            "PDP terminal classification disagrees with proof presence",
        ));
    }
    Ok(PreparedOutcome {
        identity_digest: archive.challenge_id,
        outcome_digest,
        provider_id: ProviderId::new(archive.provider_id),
        manifest_digest: ManifestDigest::new(archive.manifest_digest),
        admission_envelope_digest: archive.admission_envelope_digest,
        projection: ProofOutcomeProjectionV1::Pdp(PdpOutcomeProjectionV1 {
            source_sequence: archive.sequence,
            epoch_id: archive.epoch_id,
            status,
            proof_digest: archive.proof_digest,
            provider_attestation,
            sampled_segments: archive.sampled_segments,
            sampled_hot_leaves: archive.sampled_hot_leaves,
            sampled_bytes: archive.sampled_bytes,
            issued_at_unix: archive.issued_at_unix,
            response_deadline_unix: archive.response_deadline_unix,
            decided_at_unix: archive.decided_at_unix,
        }),
        has_provider_proof: archive.proof_digest.is_some(),
    })
}

fn prepare_potr_outcome(
    receipt_payload: &[u8],
    admission_envelope_digest: [u8; 32],
) -> Result<PreparedOutcome, InstructionExecutionError> {
    if admission_envelope_digest == [0; 32] {
        return Err(invalid_parameter(
            "PoTR admission envelope digest must be non-zero",
        ));
    }
    let receipt: PotrReceiptV1 = decode_payload(
        receipt_payload,
        PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
        POTR_RECEIPT_LIMITS,
        "PoTR signed receipt",
    )?;
    receipt
        .validate()
        .map_err(|error| invalid_parameter(format!("invalid PoTR signed receipt: {error}")))?;
    let outcome_digest = receipt
        .signed_receipt_digest()
        .map_err(|error| invalid_parameter(format!("failed to digest PoTR receipt: {error}")))?;
    let identity_digest = receipt
        .request_scope_digest()
        .map_err(|error| invalid_parameter(format!("failed to scope PoTR receipt: {error}")))?;
    let gateway_public_key: [u8; 32] = receipt
        .gateway_signature
        .as_ref()
        .ok_or_else(|| invalid_parameter("PoTR receipt has no gateway signature"))?
        .public_key
        .as_slice()
        .try_into()
        .map_err(|_| invalid_parameter("PoTR gateway public key is not 32 bytes"))?;
    let governed_provider_public_key = &receipt
        .provider_signature
        .as_ref()
        .ok_or_else(|| invalid_parameter("PoTR receipt has no provider signature"))?
        .public_key;
    let governed_provider_key_digest = *blake3::hash(governed_provider_public_key).as_bytes();
    Ok(PreparedOutcome {
        identity_digest,
        outcome_digest,
        provider_id: ProviderId::new(receipt.provider_id),
        manifest_digest: ManifestDigest::new(receipt.manifest_digest),
        admission_envelope_digest,
        projection: ProofOutcomeProjectionV1::Potr(PotrOutcomeProjectionV1 {
            status: map_potr_status(receipt.status),
            deadline_ms: receipt.deadline_ms,
            latency_ms: receipt.latency_ms,
            requested_at_ms: receipt.requested_at_ms,
            responded_at_ms: receipt.responded_at_ms,
            recorded_at_ms: receipt.recorded_at_ms,
            range_start: receipt.range_start,
            range_end: receipt.range_end,
            gateway_public_key,
            governed_provider_key_digest,
            canonical_signed_receipt: receipt_payload.to_vec(),
        }),
        has_provider_proof: true,
    })
}

fn verify_pdp_attestation(
    proof_digest: [u8; 32],
    attestation: &ProofOutcomeEd25519AttestationV1,
) -> Result<(), InstructionExecutionError> {
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &attestation.public_key)
        .map_err(|error| corrupt_state(format!("stored PDP public key is invalid: {error}")))?;
    let signature = ed25519_parse_signature(&attestation.signature)
        .map_err(|error| corrupt_state(format!("stored PDP signature is invalid: {error}")))?;
    let mut message = Vec::with_capacity(PDP_PROOF_SIGNATURE_DOMAIN_V1.len() + proof_digest.len());
    message.extend_from_slice(PDP_PROOF_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&proof_digest);
    signature
        .verify(&public_key, &message)
        .map_err(|error| corrupt_state(format!("stored PDP signature failed verification: {error}")))
}

fn validate_outcome_record(record: &ProofOutcomeRecordV1) -> Result<(), InstructionExecutionError> {
    if record.version != PROOF_OUTCOME_RECORD_VERSION_V1
        || record.identity_digest == [0; 32]
        || record.outcome_digest == [0; 32]
        || record.provider_id.as_bytes() == &[0; 32]
        || record.manifest_digest.as_bytes() == &[0; 32]
        || record.admission_envelope_digest == [0; 32]
        || record.committed_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored proof-outcome identity or provenance is invalid",
        ));
    }
    match &record.projection {
        ProofOutcomeProjectionV1::Pdp(projection) => {
            if projection.source_sequence == 0
                || projection.epoch_id == 0
                || projection.sampled_segments == 0
                || projection.sampled_hot_leaves == 0
                || projection.issued_at_unix == 0
                || projection.response_deadline_unix <= projection.issued_at_unix
                || projection.decided_at_unix < projection.issued_at_unix
                || projection.proof_digest.is_some() != projection.provider_attestation.is_some()
                || projection.status.requires_proof() != projection.proof_digest.is_some()
                || (projection.status == PdpOutcomeStatusV1::Accepted)
                    != (projection.sampled_bytes > 0)
            {
                return Err(corrupt_state(
                    "stored PDP outcome projection is inconsistent",
                ));
            }
            if let (Some(proof_digest), Some(attestation)) =
                (projection.proof_digest, projection.provider_attestation)
            {
                if proof_digest == [0; 32] {
                    return Err(corrupt_state("stored PDP proof digest is zero"));
                }
                verify_pdp_attestation(proof_digest, &attestation)?;
            }
            let decided_at_ms = projection
                .decided_at_unix
                .checked_mul(1_000)
                .ok_or_else(|| corrupt_state("stored PDP decision timestamp overflow"))?;
            if decided_at_ms > record.committed_at_unix_ms {
                return Err(corrupt_state(
                    "stored PDP outcome was decided after its committing block",
                ));
            }
        }
        ProofOutcomeProjectionV1::Potr(projection) => {
            if projection.governed_provider_key_digest == [0; 32]
                || projection.gateway_public_key == [0; 32]
                || projection.canonical_signed_receipt.is_empty()
                || projection.canonical_signed_receipt.len()
                    > PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1
            {
                return Err(corrupt_state(
                    "stored PoTR outcome key or receipt metadata is invalid",
                ));
            }
            let receipt: PotrReceiptV1 = decode_state(
                &projection.canonical_signed_receipt,
                "stored canonical PoTR receipt",
            )?;
            receipt
                .validate()
                .map_err(|error| corrupt_state(format!("stored PoTR receipt is invalid: {error}")))?;
            let receipt_gateway: [u8; 32] = receipt
                .gateway_signature
                .as_ref()
                .ok_or_else(|| corrupt_state("stored PoTR receipt lost its gateway signature"))?
                .public_key
                .as_slice()
                .try_into()
                .map_err(|_| corrupt_state("stored PoTR gateway key length is invalid"))?;
            let receipt_provider = &receipt
                .provider_signature
                .as_ref()
                .ok_or_else(|| corrupt_state("stored PoTR receipt lost its provider signature"))?
                .public_key;
            if receipt.request_scope_digest().map_err(|error| {
                corrupt_state(format!("failed to scope stored PoTR receipt: {error}"))
            })? != record.identity_digest
                || receipt.signed_receipt_digest().map_err(|error| {
                    corrupt_state(format!("failed to digest stored PoTR receipt: {error}"))
                })? != record.outcome_digest
                || ProviderId::new(receipt.provider_id) != record.provider_id
                || ManifestDigest::new(receipt.manifest_digest) != record.manifest_digest
                || map_potr_status(receipt.status) != projection.status
                || receipt.deadline_ms != projection.deadline_ms
                || receipt.latency_ms != projection.latency_ms
                || receipt.requested_at_ms != projection.requested_at_ms
                || receipt.responded_at_ms != projection.responded_at_ms
                || receipt.recorded_at_ms != projection.recorded_at_ms
                || receipt.range_start != projection.range_start
                || receipt.range_end != projection.range_end
                || receipt_gateway != projection.gateway_public_key
                || *blake3::hash(receipt_provider).as_bytes()
                    != projection.governed_provider_key_digest
                || receipt.recorded_at_ms > record.committed_at_unix_ms
            {
                return Err(corrupt_state(
                    "stored PoTR outcome disagrees with its canonical signed receipt",
                ));
            }
        }
    }
    Ok(())
}

fn read_outcome(
    world: &impl WorldReadOnly,
    kind: ProofOutcomeKindV1,
    identity_digest: [u8; 32],
) -> Result<Option<ProofOutcomeRecordV1>, InstructionExecutionError> {
    let key = outcome_key(kind, identity_digest);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record: ProofOutcomeRecordV1 = decode_state(bytes, "proof-outcome record")?;
    validate_outcome_record(&record)?;
    if record.kind() != kind
        || record.identity_digest != identity_digest
        || outcome_key(record.kind(), record.identity_digest) != key
    {
        return Err(corrupt_state(
            "stored proof-outcome record disagrees with its state key",
        ));
    }
    Ok(Some(record))
}

fn same_cryptographic_outcome(
    existing: &ProofOutcomeRecordV1,
    candidate: &ProofOutcomeRecordV1,
) -> bool {
    existing.version == candidate.version
        && existing.identity_digest == candidate.identity_digest
        && existing.outcome_digest == candidate.outcome_digest
        && existing.provider_id == candidate.provider_id
        && existing.manifest_digest == candidate.manifest_digest
        && existing.admission_envelope_digest == candidate.admission_envelope_digest
        && existing.projection == candidate.projection
}

fn validate_outcome_against_current_policy(
    prepared: &PreparedOutcome,
    policy: &ProofOutcomeSignerPolicyV1,
    now_unix: u64,
) -> Result<(), InstructionExecutionError> {
    if policy.provider_id != prepared.provider_id
        || policy.admission_envelope_digest != prepared.admission_envelope_digest
        || now_unix < policy.valid_from_unix
        || now_unix > policy.valid_until_unix
    {
        return Err(invalid_parameter(
            "proof outcome does not bind the current active provider admission policy",
        ));
    }
    match &prepared.projection {
        ProofOutcomeProjectionV1::Pdp(projection) => {
            if projection.issued_at_unix < policy.valid_from_unix
                || projection.response_deadline_unix > policy.valid_until_unix
            {
                return Err(invalid_parameter(
                    "PDP challenge window is outside the current signer policy",
                ));
            }
            if let Some(attestation) = projection.provider_attestation {
                if attestation.public_key != policy.pdp_public_key {
                    return Err(invalid_parameter(
                        "PDP proof signer does not match the current governed provider key",
                    ));
                }
            }
        }
        ProofOutcomeProjectionV1::Potr(projection) => {
            let receipt: PotrReceiptV1 = decode_payload(
                &projection.canonical_signed_receipt,
                PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
                POTR_RECEIPT_LIMITS,
                "PoTR signed receipt",
            )?;
            let provider_key = &receipt
                .provider_signature
                .as_ref()
                .ok_or_else(|| invalid_parameter("PoTR receipt has no provider signature"))?
                .public_key;
            if projection.gateway_public_key != policy.gateway_public_key
                || provider_key != &policy.potr_mldsa_public_key
                || projection.requested_at_ms / 1_000 < policy.valid_from_unix
                || projection.recorded_at_ms / 1_000 > policy.valid_until_unix
            {
                return Err(invalid_parameter(
                    "PoTR receipt signers or timing do not match the current governed policy",
                ));
            }
        }
    }
    Ok(())
}

fn validate_persisted_event(
    event: &ProofOutcomePersistedEventV1,
    expected_sequence: u64,
) -> Result<(), InstructionExecutionError> {
    validate_outcome_record(&event.outcome)?;
    if event.sequence == 0
        || event.sequence != expected_sequence
        || event.target_block_height == 0
        || event.outcome.committed_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored proof-outcome event cursor metadata is invalid",
        ));
    }
    Ok(())
}

fn validate_event_successor(
    previous: Option<&ProofOutcomePersistedEventV1>,
    current: &ProofOutcomePersistedEventV1,
) -> Result<(), InstructionExecutionError> {
    let Some(previous) = previous else {
        if current.sequence != 1 || current.event_index != 0 {
            return Err(corrupt_state(
                "proof-outcome event journal does not begin at sequence one and block index zero",
            ));
        }
        return Ok(());
    };
    if previous
        .sequence
        .checked_add(1)
        .is_none_or(|next| current.sequence != next)
    {
        return Err(corrupt_state(
            "proof-outcome event journal sequence is not contiguous",
        ));
    }
    match previous
        .target_block_height
        .cmp(&current.target_block_height)
    {
        core::cmp::Ordering::Less if current.event_index == 0 => Ok(()),
        core::cmp::Ordering::Equal
            if previous
                .event_index
                .checked_add(1)
                .is_some_and(|next| current.event_index == next) =>
        {
            Ok(())
        }
        _ => Err(corrupt_state(
            "proof-outcome event journal block height/index ordering is invalid",
        )),
    }
}

fn read_persisted_event(
    world: &impl WorldReadOnly,
    sequence: u64,
) -> Result<Option<ProofOutcomePersistedEventV1>, InstructionExecutionError> {
    if sequence == 0 {
        return Err(corrupt_state(
            "proof-outcome event sequence zero cannot be read",
        ));
    }
    let Some(bytes) = world.smart_contract_state().get(&event_key(sequence)) else {
        return Ok(None);
    };
    let event: ProofOutcomePersistedEventV1 =
        decode_state(bytes, "proof-outcome committed event")?;
    validate_persisted_event(&event, sequence)?;
    Ok(Some(event))
}

fn validate_event_outcome_binding(
    world: &impl WorldReadOnly,
    event: &ProofOutcomePersistedEventV1,
) -> Result<usize, InstructionExecutionError> {
    let key = outcome_key(event.outcome.kind(), event.outcome.identity_digest);
    let state_bytes = world
        .smart_contract_state()
        .get(&key)
        .map_or(0, Vec::len);
    let outcome = read_outcome(
        world,
        event.outcome.kind(),
        event.outcome.identity_digest,
    )?
    .ok_or_else(|| {
        corrupt_state(format!(
            "proof-outcome event sequence {} references a missing outcome",
            event.sequence
        ))
    })?;
    if outcome != event.outcome {
        return Err(corrupt_state(format!(
            "proof-outcome event sequence {} disagrees with authoritative state",
            event.sequence
        )));
    }
    Ok(state_bytes)
}

fn read_event_journal_head(
    world: &impl WorldReadOnly,
) -> Result<Option<ProofOutcomeEventJournalHeadV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(event_journal_head_key())
    else {
        return Ok(None);
    };
    let head: ProofOutcomeEventJournalHeadV1 =
        decode_state(bytes, "proof-outcome event journal head")?;
    if head.last_sequence == 0 || head.last_target_block_height == 0 {
        return Err(corrupt_state(
            "stored proof-outcome event journal head is invalid",
        ));
    }
    let terminal = read_persisted_event(world, head.last_sequence)?.ok_or_else(|| {
        corrupt_state("proof-outcome event journal head references a missing event")
    })?;
    if terminal.target_block_height != head.last_target_block_height
        || terminal.event_index != head.last_event_index
    {
        return Err(corrupt_state(
            "proof-outcome event journal head disagrees with its terminal event",
        ));
    }
    let predecessor = if head.last_sequence == 1 {
        None
    } else {
        Some(
            read_persisted_event(world, head.last_sequence - 1)?.ok_or_else(|| {
                corrupt_state("proof-outcome event journal terminal predecessor is missing")
            })?,
        )
    };
    validate_event_successor(predecessor.as_ref(), &terminal)?;
    Ok(Some(head))
}

fn ensure_no_event_after_head(
    world: &impl WorldReadOnly,
    head: Option<ProofOutcomeEventJournalHeadV1>,
) -> Result<(), InstructionExecutionError> {
    let prefix =
        Name::from_str(EVENT_STATE_KEY_PREFIX).expect("static proof-outcome event prefix is valid");
    let first = world
        .smart_contract_state()
        .range(prefix.clone()..)
        .next()
        .and_then(|(key, _)| {
            key.to_string()
                .starts_with(EVENT_STATE_KEY_PREFIX)
                .then_some(key)
        });
    match (head, first) {
        (None, None) => return Ok(()),
        (None, Some(_)) => {
            return Err(corrupt_state(
                "proof-outcome event journal contains records without a head",
            ));
        }
        (Some(_), Some(key)) if *key == event_key(1) => {}
        (Some(_), _) => {
            return Err(corrupt_state(
                "proof-outcome event journal does not begin at sequence one",
            ));
        }
    }
    let start = head.map_or(prefix, |head| event_key(head.last_sequence));
    for (key, _) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(EVENT_STATE_KEY_PREFIX) {
            break;
        }
        if head.is_some_and(|head| *key == event_key(head.last_sequence)) {
            continue;
        }
        return Err(corrupt_state(
            "proof-outcome event journal contains a record beyond its head",
        ));
    }
    Ok(())
}

fn append_event_journal(
    state_transaction: &mut StateTransaction<'_, '_>,
    outcome: &ProofOutcomeRecordV1,
) -> Result<(), InstructionExecutionError> {
    let committed_parent_height =
        u64::try_from(state_transaction.block_hashes().len()).map_err(|_| {
            corrupt_state("committed proof-outcome parent height does not fit into u64")
        })?;
    let target_block_height = committed_parent_height
        .checked_add(1)
        .ok_or_else(|| corrupt_state("proof-outcome event target block height overflow"))?;
    if target_block_height != state_transaction._curr_block.height().get() {
        return Err(corrupt_state(
            "proof-outcome event target height does not match the executing block",
        ));
    }
    let head = read_event_journal_head(state_transaction.world())?;
    ensure_no_event_after_head(state_transaction.world(), head)?;
    let (sequence, event_index, previous) = match head {
        Some(head) => {
            let sequence = head
                .last_sequence
                .checked_add(1)
                .ok_or_else(|| corrupt_state("proof-outcome event sequence overflow"))?;
            let event_index = match head.last_target_block_height.cmp(&target_block_height) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => head
                    .last_event_index
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("proof-outcome block event index overflow"))?,
                core::cmp::Ordering::Greater => {
                    return Err(corrupt_state(
                        "proof-outcome event target height regressed behind the journal head",
                    ));
                }
            };
            let previous = read_persisted_event(state_transaction.world(), head.last_sequence)?
                .ok_or_else(|| {
                    corrupt_state("proof-outcome event journal lost its terminal record")
                })?;
            (sequence, event_index, Some(previous))
        }
        None => (1, 0, None),
    };
    let key = event_key(sequence);
    if state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .is_some()
    {
        return Err(corrupt_state(
            "proof-outcome event journal sequence already exists",
        ));
    }
    let event = ProofOutcomePersistedEventV1 {
        sequence,
        target_block_height,
        event_index,
        outcome: outcome.clone(),
    };
    validate_persisted_event(&event, sequence)?;
    validate_event_successor(previous.as_ref(), &event)?;
    validate_event_outcome_binding(state_transaction.world(), &event)?;
    let next_head = ProofOutcomeEventJournalHeadV1 {
        last_sequence: sequence,
        last_target_block_height: target_block_height,
        last_event_index: event_index,
    };
    state_transaction.world.smart_contract_state.insert(
        key,
        encode_state(&event, "proof-outcome committed event")?,
    );
    state_transaction.world.smart_contract_state.insert(
        event_journal_head_key().clone(),
        encode_state(&next_head, "proof-outcome event journal head")?,
    );
    Ok(())
}

impl Execute for SetSorafsProofOutcomeSignerPolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !has_named_permission(
            state_transaction,
            authority,
            Permission::from(CanManageSorafsProofOutcomePolicy).name(),
        ) {
            return Err(invalid_parameter(
                "CanManageSorafsProofOutcomePolicy permission is required",
            ));
        }
        if state_transaction.world.accounts.get(authority).is_none() {
            return Err(invalid_parameter(
                "proof-outcome policy authority is not a registered account",
            ));
        }
        validate_signer_policy(&self.policy)?;
        if state_transaction
            .world
            .provider_owners
            .get(&self.policy.provider_id)
            .is_none()
        {
            return Err(invalid_parameter(
                "proof-outcome signer policy references an unregistered provider",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        if self.policy.valid_until_unix < now / 1_000 {
            return Err(invalid_parameter(
                "proof-outcome signer policy is already expired",
            ));
        }
        let digest = policy_digest(&self.policy)?;
        if let Some(current) =
            read_signer_policy(state_transaction.world(), self.policy.provider_id)?
        {
            if current.policy_digest == digest && current.policy == self.policy {
                return Ok(());
            }
            if self.policy.revision != current.policy.revision.checked_add(1).ok_or_else(|| {
                corrupt_state("proof-outcome signer policy revision overflow")
            })? || self.policy.predecessor_digest != Some(current.policy_digest)
            {
                return Err(invalid_parameter(
                    "proof-outcome signer policy revision or predecessor is stale",
                ));
            }
        } else if self.policy.revision != 1 || self.policy.predecessor_digest.is_some() {
            return Err(invalid_parameter(
                "first proof-outcome signer policy must be revision one without a predecessor",
            ));
        }
        let record = ProofOutcomeSignerPolicyRecordV1 {
            policy: self.policy,
            policy_digest: digest,
            activated_by: authority.clone(),
            activated_at_unix_ms: now,
        };
        state_transaction.world.smart_contract_state.insert(
            policy_key(record.policy.provider_id),
            encode_state(&record, "proof-outcome signer policy")?,
        );
        Ok(())
    }
}

impl Execute for SubmitSorafsProofOutcome {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let prepared = match self.submission {
            SorafsProofOutcomeSubmissionV1::Pdp { archive_payload } => {
                prepare_pdp_outcome(&archive_payload)?
            }
            SorafsProofOutcomeSubmissionV1::Potr {
                receipt_payload,
                admission_envelope_digest,
            } => prepare_potr_outcome(&receipt_payload, admission_envelope_digest)?,
        };
        let now = block_time_ms(state_transaction)?;
        let candidate = PreparedOutcome {
            identity_digest: prepared.identity_digest,
            outcome_digest: prepared.outcome_digest,
            provider_id: prepared.provider_id,
            manifest_digest: prepared.manifest_digest,
            admission_envelope_digest: prepared.admission_envelope_digest,
            projection: prepared.projection.clone(),
            has_provider_proof: prepared.has_provider_proof,
        }
        .into_record(authority.clone(), now);
        if let Some(existing) = read_outcome(
            state_transaction.world(),
            candidate.kind(),
            candidate.identity_digest,
        )? {
            if same_cryptographic_outcome(&existing, &candidate) {
                return Ok(());
            }
            return Err(invalid_parameter(
                "proof-outcome identity was already committed with different cryptographic material",
            ));
        }
        if state_transaction.world.accounts.get(authority).is_none() {
            return Err(invalid_parameter(
                "proof-outcome submitter is not a registered account",
            ));
        }
        let policy = read_signer_policy(state_transaction.world(), prepared.provider_id)?
            .ok_or_else(|| {
                invalid_parameter("proof outcome has no current governed signer policy")
            })?;
        validate_outcome_against_current_policy(&prepared, &policy.policy, now / 1_000)?;
        if !prepared.has_provider_proof
            && !has_scheduler_permission(
                state_transaction,
                authority,
                prepared.provider_id,
            )
        {
            return Err(invalid_parameter(
                "unsigned PDP terminal outcome requires provider-scoped CanRecordSorafsProofOutcome permission",
            ));
        }
        validate_outcome_record(&candidate)?;
        let key = outcome_key(candidate.kind(), candidate.identity_digest);
        state_transaction.world.smart_contract_state.insert(
            key,
            encode_state(&candidate, "proof-outcome record")?,
        );
        append_event_journal(state_transaction, &candidate)
    }
}

fn query_failure(error: InstructionExecutionError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

fn resolve_finalized_cursor(
    state_ro: &impl crate::state::StateReadOnly,
) -> Result<ProofOutcomeFinalizedCursorV1, QueryExecutionFail> {
    let height = u64::try_from(state_ro.block_hashes().len()).map_err(|_| {
        QueryExecutionFail::Conversion(
            "finalized proof-outcome height does not fit into u64".to_owned(),
        )
    })?;
    let block_hash = state_ro
        .block_hashes()
        .last()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "proof-outcome queries require at least one committed block".to_owned(),
            )
        })?;
    if height == 0 || block_hash == [0; 32] {
        return Err(QueryExecutionFail::Conversion(
            "finalized proof-outcome query anchor is invalid".to_owned(),
        ));
    }
    Ok(ProofOutcomeFinalizedCursorV1 { height, block_hash })
}

fn resolve_committed_event(
    state_ro: &impl crate::state::StateReadOnly,
    event: &ProofOutcomePersistedEventV1,
) -> Result<ProofOutcomeFinalizedEventV1, QueryExecutionFail> {
    let hash_index = event
        .target_block_height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "proof-outcome target height cannot index finalized block hashes".to_owned(),
            )
        })?;
    let block_hash = state_ro
        .block_hashes()
        .get(hash_index)
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(format!(
                "proof-outcome event sequence {} targets non-finalized block height {}",
                event.sequence, event.target_block_height
            ))
        })?;
    if block_hash == [0; 32] {
        return Err(QueryExecutionFail::Conversion(format!(
            "proof-outcome event sequence {} resolved a zero block hash",
            event.sequence
        )));
    }
    Ok(ProofOutcomeFinalizedEventV1 {
        sequence: event.sequence,
        block_height: event.target_block_height,
        block_hash,
        event_index: event.event_index,
        outcome: event.outcome.clone(),
    })
}

fn charge_state_bytes(
    total: &mut usize,
    amount: usize,
) -> Result<(), QueryExecutionFail> {
    *total = total.checked_add(amount).ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "proof-outcome query state-read byte counter overflow".to_owned(),
        )
    })?;
    if *total > QUERY_MAX_STATE_READ_BYTES {
        return Err(QueryExecutionFail::Conversion(format!(
            "proof-outcome query inspected more than {QUERY_MAX_STATE_READ_BYTES} state bytes"
        )));
    }
    Ok(())
}

fn ensure_page_budget<T: norito::core::NoritoSerialize>(
    value: &T,
) -> Result<(), QueryExecutionFail> {
    let length = norito::to_bytes(value)
        .map_err(|error| {
            QueryExecutionFail::Conversion(format!(
                "failed to encode proof-outcome event page: {error}"
            ))
        })?
        .len();
    if length > PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "proof-outcome event page encodes to {length} bytes, above {PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1}"
        )));
    }
    Ok(())
}

fn query_event_page(
    query: &FindSorafsProofOutcomeEvents,
    state_ro: &impl crate::state::StateReadOnly,
    finalized_cursor: ProofOutcomeFinalizedCursorV1,
) -> Result<ProofOutcomeFinalizedEventPageV1, QueryExecutionFail> {
    let limit = usize::try_from(query.limit).map_err(|_| {
        QueryExecutionFail::Conversion("proof-outcome query limit does not fit usize".to_owned())
    })?;
    if limit == 0 || limit > PROOF_OUTCOME_QUERY_MAX_ITEMS_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "proof-outcome query limit {} is outside 1..={PROOF_OUTCOME_QUERY_MAX_ITEMS_V1}",
            query.limit
        )));
    }
    let world = state_ro.world();
    let mut state_read_bytes = world
        .smart_contract_state()
        .get(event_journal_head_key())
        .map_or(0, Vec::len);
    let head = read_event_journal_head(world).map_err(query_failure)?;
    ensure_no_event_after_head(world, head).map_err(query_failure)?;
    let Some(head) = head else {
        let page = ProofOutcomeFinalizedEventPageV1 {
            finalized_cursor,
            events: Vec::new(),
            has_more: false,
            next_after: None,
        };
        ensure_page_budget(&page)?;
        return Ok(page);
    };
    let terminal_state_bytes = world
        .smart_contract_state()
        .get(&event_key(head.last_sequence))
        .map_or(0, Vec::len);
    let terminal = read_persisted_event(world, head.last_sequence)
        .map_err(query_failure)?
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "proof-outcome event journal terminal record is missing".to_owned(),
            )
        })?;
    let terminal_binding_bytes =
        validate_event_outcome_binding(world, &terminal).map_err(query_failure)?;
    charge_state_bytes(
        &mut state_read_bytes,
        terminal_state_bytes
            .checked_add(terminal_binding_bytes)
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "proof-outcome terminal read-byte counter overflow".to_owned(),
                )
            })?,
    )?;
    resolve_committed_event(state_ro, &terminal)?;

    let mut previous = match query.after {
        Some(after) => {
            if after.sequence == 0 || after.sequence > head.last_sequence {
                return Err(QueryExecutionFail::Expired);
            }
            let event_bytes = world
                .smart_contract_state()
                .get(&event_key(after.sequence))
                .map_or(0, Vec::len);
            let event = read_persisted_event(world, after.sequence)
                .map_err(query_failure)?
                .ok_or(QueryExecutionFail::Expired)?;
            let binding_bytes =
                validate_event_outcome_binding(world, &event).map_err(query_failure)?;
            charge_state_bytes(
                &mut state_read_bytes,
                event_bytes.checked_add(binding_bytes).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "proof-outcome cursor read-byte counter overflow".to_owned(),
                    )
                })?,
            )?;
            if resolve_committed_event(state_ro, &event)?.cursor() != after {
                return Err(QueryExecutionFail::Expired);
            }
            let predecessor = if after.sequence == 1 {
                None
            } else {
                let sequence = after.sequence - 1;
                let bytes = world
                    .smart_contract_state()
                    .get(&event_key(sequence))
                    .map_or(0, Vec::len);
                charge_state_bytes(&mut state_read_bytes, bytes)?;
                Some(
                    read_persisted_event(world, sequence)
                        .map_err(query_failure)?
                        .ok_or_else(|| {
                            QueryExecutionFail::Conversion(format!(
                                "proof-outcome event journal is missing predecessor sequence {sequence}"
                            ))
                        })?,
                )
            };
            validate_event_successor(predecessor.as_ref(), &event).map_err(query_failure)?;
            Some(event)
        }
        None => None,
    };

    let mut sequence = query
        .after
        .map_or(Some(1), |after| after.sequence.checked_add(1));
    let mut events = Vec::with_capacity(limit);
    let payload_budget = PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1.saturating_sub(1_024);
    let mut encoded_event_bytes = 0usize;
    let mut inspected_records = 0usize;
    let inspected_budget = limit.saturating_add(4);
    while let Some(current_sequence) = sequence {
        if current_sequence > head.last_sequence || events.len() >= limit {
            break;
        }
        inspected_records = inspected_records.checked_add(1).ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "proof-outcome inspected-record counter overflow".to_owned(),
            )
        })?;
        if inspected_records > inspected_budget {
            return Err(QueryExecutionFail::Conversion(
                "proof-outcome query exceeded its inspected-record budget".to_owned(),
            ));
        }
        let event_state_bytes = world
            .smart_contract_state()
            .get(&event_key(current_sequence))
            .map_or(0, Vec::len);
        let event = read_persisted_event(world, current_sequence)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(format!(
                    "proof-outcome event journal is missing sequence {current_sequence}"
                ))
            })?;
        validate_event_successor(previous.as_ref(), &event).map_err(query_failure)?;
        let binding_bytes =
            validate_event_outcome_binding(world, &event).map_err(query_failure)?;
        charge_state_bytes(
            &mut state_read_bytes,
            event_state_bytes.checked_add(binding_bytes).ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "proof-outcome event read-byte counter overflow".to_owned(),
                )
            })?,
        )?;
        let resolved = resolve_committed_event(state_ro, &event)?;
        let resolved_bytes = norito::to_bytes(&resolved)
            .map_err(|error| {
                QueryExecutionFail::Conversion(format!(
                    "failed to encode committed proof-outcome event: {error}"
                ))
            })?
            .len();
        let next_bytes = encoded_event_bytes
            .checked_add(resolved_bytes)
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "proof-outcome event-page byte counter overflow".to_owned(),
                )
            })?;
        if next_bytes > payload_budget {
            if events.is_empty() {
                return Err(QueryExecutionFail::Conversion(format!(
                    "one proof-outcome event cannot fit within the {PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1}-byte page budget"
                )));
            }
            break;
        }
        encoded_event_bytes = next_bytes;
        previous = Some(event);
        events.push(resolved);
        sequence = current_sequence.checked_add(1);
    }
    let has_more = events
        .last()
        .is_some_and(|event| event.sequence < head.last_sequence);
    let next_after = has_more.then(|| {
        events
            .last()
            .expect("has_more requires a non-empty proof-outcome page")
            .cursor()
    });
    let page = ProofOutcomeFinalizedEventPageV1 {
        finalized_cursor,
        events,
        has_more,
        next_after,
    };
    ensure_page_budget(&page)?;
    Ok(page)
}

impl ValidSingularQuery for FindSorafsProofOutcomeEvents {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ProofOutcomeFinalizedEventPageV1, QueryExecutionFail> {
        let actual = resolve_finalized_cursor(state_ro)?;
        if self
            .expected_finalized_cursor
            .is_some_and(|expected| expected != actual)
        {
            return Err(QueryExecutionFail::Expired);
        }
        query_event_page(self, state_ro, actual)
    }
}
