//! Authoritative SoraFS moderation commit/reveal ledger handlers.

use std::{str::FromStr, sync::OnceLock};

use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            FinalizeSorafsModerationCase, FinalizeSorafsModerationSortition,
            RaiseSorafsModerationChallenge, RegisterSorafsModerationJurorEligibility,
            ResolveSorafsModerationChallenge, SetSorafsModerationPolicy,
            SubmitSorafsModerationAppeal,
            SubmitSorafsModerationCommit, SubmitSorafsModerationReveal,
        },
    },
    name::Name,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsModerationAppeal, FindSorafsModerationCase,
            FindSorafsModerationChallenge, FindSorafsModerationCommit,
            FindSorafsModerationJurorEligibility, FindSorafsModerationNoShow,
            FindSorafsModerationOutcome, FindSorafsModerationPolicy, FindSorafsModerationReveal,
            FindSorafsModerationStatus,
        },
    },
    sorafs::{
        moderation::{
            SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1, SoraFsModerationBallotCommitV1,
            SoraFsModerationBallotContextV1, SoraFsModerationBallotRevealV1,
            SoraFsModerationVoteChoice,
        },
        moderation_ledger::{
            MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1, MODERATION_LEDGER_MAX_NONCE_BYTES_V1,
            MODERATION_LEDGER_MAX_PANEL_SIZE_V1, MODERATION_LEDGER_MAX_REASON_BYTES_V1,
            MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1, ModerationAppealRecordV1,
            ModerationAppealStatusV1, ModerationCaseRecordV1, ModerationCaseSpecV1,
            ModerationCaseStatusV1, ModerationChallengeDecisionV1, ModerationChallengeRecordV1,
            ModerationCommitRecordV1, ModerationJurorEligibilityClassV1,
            ModerationJurorEligibilityRecordV1, ModerationJurorReplacementV1,
            ModerationLedgerPolicyRecord, ModerationLedgerStatusV1, ModerationNoShowKindV1,
            ModerationNoShowRecordV1, ModerationOutcomeKindV1, ModerationOutcomeRecordV1,
            ModerationPanelSelectionV1, ModerationPoPRegistrySnapshotV1,
            ModerationRevealRecordV1, ModerationSortitionError, ModerationVoteCountsV1,
            sorafs_moderation_panel_roster_hash_v1, sorafs_moderation_pop_challenge_v1,
            sorafs_moderation_pop_verifier_context_v1, sorafs_moderation_select_panel_v1,
            sorafs_moderation_sortition_digest_v1, sorafs_moderation_sortition_seed_v1,
        },
    },
};
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::pop_credentials::{
    POP_MEMBERSHIP_PROOF_MAX_BYTES_V1, PopEligibilityClassV1, PopMembershipProofV1,
    verify_pop_membership_proof_v1,
};

use super::*;
use crate::{
    smartcontracts::ValidSingularQuery,
    smartcontracts::isi::sorafs_pop_registry::read_active_publications,
    state::{StateTransaction, WorldReadOnly},
};

const POLICY_STATE_KEY: &str = "sorafs_moderation_policy_v1";
const STATUS_STATE_KEY: &str = "sorafs_moderation_status_v1";
const APPEAL_STATE_KEY_PREFIX: &str = "sorafs_moderation_appeal_v1_";
const APPEAL_DEPOSIT_STATE_KEY_PREFIX: &str = "sorafs_moderation_appeal_deposit_v1_";
const APPEAL_PROOF_TOKEN_STATE_KEY_PREFIX: &str =
    "sorafs_moderation_appeal_proof_token_v1_";
const ELIGIBILITY_STATE_KEY_PREFIX: &str = "sorafs_moderation_eligibility_v1_";
const NULLIFIER_STATE_KEY_PREFIX: &str = "sorafs_moderation_pop_nullifier_v1_";
const CASE_STATE_KEY_PREFIX: &str = "sorafs_moderation_case_v1_";
const COMMIT_STATE_KEY_PREFIX: &str = "sorafs_moderation_commit_v1_";
const REVEAL_STATE_KEY_PREFIX: &str = "sorafs_moderation_reveal_v1_";
const CHALLENGE_STATE_KEY_PREFIX: &str = "sorafs_moderation_challenge_v1_";
const OUTCOME_STATE_KEY_PREFIX: &str = "sorafs_moderation_outcome_v1_";
const NO_SHOW_STATE_KEY_PREFIX: &str = "sorafs_moderation_no_show_v1_";
const CASE_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.case-state-key.v1";
const JUROR_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.juror-state-key.v1";
const CHALLENGE_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.challenge-state-key.v1";
const NULLIFIER_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-nullifier-state-key.v1";
const APPEAL_DEPOSIT_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.appeal-deposit-state-key.v1";
const APPEAL_PROOF_TOKEN_KEY_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.appeal-proof-token-state-key.v1";
const PROOF_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-proof-payload.v1";
const STATE_MAX_BYTES: usize = 512 * 1024;
const PAYLOAD_MAX_BYTES: usize = 64 * 1024;
const STATE_LIMITS: DecodeLimits =
    DecodeLimits::new(256, STATE_MAX_BYTES, 4_096, 2 * STATE_MAX_BYTES, 64);
const PAYLOAD_LIMITS: DecodeLimits =
    DecodeLimits::new(256, PAYLOAD_MAX_BYTES, 2_048, 2 * PAYLOAD_MAX_BYTES, 64);
const PROOF_LIMITS: DecodeLimits = DecodeLimits::new(
    256,
    POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 + 32 * 1024,
    4_096,
    2 * (POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 + 32 * 1024),
    64,
);
const MANAGE_PERMISSION: &str = "CanManageSorafsModeration";

#[derive(Clone, Debug, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct AppealDepositBindingStateV1 {
    deposit_lock_digest: [u8; 32],
    case_id: String,
    round_id: String,
    intake_digest: [u8; 32],
}

#[derive(Clone, Debug, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct AppealProofTokenBindingStateV1 {
    proof_token_digest: [u8; 32],
    case_id: String,
    round_id: String,
    intake_digest: [u8; 32],
}

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn corrupt_state(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn require_manage_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<(), InstructionExecutionError> {
    let direct = state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|candidate| candidate.name() == MANAGE_PERMISSION)
        });
    let role = state_transaction
        .world
        .account_roles_iter(authority)
        .filter_map(|role_id| state_transaction.world.roles.get(role_id))
        .any(|role| {
            role.permissions()
                .any(|candidate| candidate.name() == MANAGE_PERMISSION)
        });
    let permitted = direct || role;
    if permitted {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "permission {MANAGE_PERMISSION} required for authoritative SoraFS moderation operation"
        )))
    }
}

fn block_time_ms(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let now = state_transaction.block_unix_timestamp_ms();
    if now == 0 {
        return Err(invalid_parameter(
            "authoritative moderation operations require a non-zero block timestamp",
        ));
    }
    Ok(now)
}

fn active_pop_snapshot(
    state_transaction: &StateTransaction<'_, '_>,
    now: u64,
) -> Result<
    (
        ModerationPoPRegistrySnapshotV1,
        super::sorafs_pop_registry::ActivePopPublicationsV1,
    ),
    InstructionExecutionError,
> {
    let active = read_active_publications(state_transaction.world())?.ok_or_else(|| {
        invalid_parameter("active SoraFS PoP root and revocation publication are required")
    })?;
    let randomness_anchor = state_transaction
        .block_hashes()
        .last()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            invalid_parameter(
                "moderation appeal intake requires an already committed parent block",
            )
        })?;
    let registry_audit_head = active.status.audit_head.ok_or_else(|| {
        corrupt_state("active PoP registry status is missing its audit-chain head")
    })?;
    let snapshot = ModerationPoPRegistrySnapshotV1 {
        issuer_policy_digest: active.issuer_policy_digest,
        commitment_root: active.root.root_digest,
        commitment_tree_version: active.root.tree_version,
        revocation_root: active.revocations.revocation_root,
        revocation_list_version: active.revocations.list_version,
        registry_audit_sequence: active.status.audit_sequence,
        registry_audit_head,
        captured_at_unix_ms: now,
        randomness_anchor,
    };
    snapshot.validate().map_err(|error| {
        corrupt_state(format!("active PoP registry snapshot is invalid: {error}"))
    })?;
    Ok((snapshot, active))
}

fn require_active_pop_snapshot(
    state_transaction: &StateTransaction<'_, '_>,
    snapshot: &ModerationPoPRegistrySnapshotV1,
) -> Result<super::sorafs_pop_registry::ActivePopPublicationsV1, InstructionExecutionError> {
    let active = read_active_publications(state_transaction.world())?.ok_or_else(|| {
        invalid_parameter("active SoraFS PoP root and revocation publication are required")
    })?;
    let audit_head = active.status.audit_head.ok_or_else(|| {
        corrupt_state("active PoP registry status is missing its audit-chain head")
    })?;
    if active.issuer_policy_digest != snapshot.issuer_policy_digest
        || active.root.root_digest != snapshot.commitment_root
        || active.root.tree_version != snapshot.commitment_tree_version
        || active.revocations.revocation_root != snapshot.revocation_root
        || active.revocations.list_version != snapshot.revocation_list_version
        || active.status.audit_sequence != snapshot.registry_audit_sequence
        || audit_head != snapshot.registry_audit_head
    {
        return Err(invalid_parameter(
            "moderation appeal PoP snapshot is stale or differs from the active registry roots",
        ));
    }
    Ok(active)
}

fn eligibility_class(class: PopEligibilityClassV1) -> ModerationJurorEligibilityClassV1 {
    match class {
        PopEligibilityClassV1::General => ModerationJurorEligibilityClassV1::General,
        PopEligibilityClassV1::Regional => ModerationJurorEligibilityClassV1::Regional,
        PopEligibilityClassV1::Expert => ModerationJurorEligibilityClassV1::Expert,
        PopEligibilityClassV1::Emergency => ModerationJurorEligibilityClassV1::Emergency,
        PopEligibilityClassV1::Observer => ModerationJurorEligibilityClassV1::Observer,
    }
}

fn policy_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| Name::from_str(POLICY_STATE_KEY).expect("static state key is valid"))
}

fn status_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| Name::from_str(STATUS_STATE_KEY).expect("static state key is valid"))
}

fn digest_key(prefix: &str, digest: [u8; 32]) -> Name {
    Name::from_str(&format!("{prefix}{}", hex::encode(digest)))
        .expect("static prefix plus lowercase hex is a valid state key")
}

fn update_string(hasher: &mut blake3::Hasher, value: &str) {
    let length = u64::try_from(value.len()).expect("state-key material length fits u64");
    hasher.update(&length.to_le_bytes());
    hasher.update(value.as_bytes());
}

fn validate_lookup_identifiers(
    case_id: &str,
    round_id: &str,
) -> Result<(), InstructionExecutionError> {
    for (field, value) in [("case_id", case_id), ("round_id", round_id)] {
        if value.is_empty()
            || value.len() > MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1
            || !value.is_ascii()
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@')
            })
        {
            return Err(invalid_parameter(format!(
                "invalid moderation {field} identifier"
            )));
        }
    }
    Ok(())
}

fn case_digest(case_id: &str, round_id: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CASE_KEY_DOMAIN_V1);
    update_string(&mut hasher, case_id);
    update_string(&mut hasher, round_id);
    *hasher.finalize().as_bytes()
}

fn juror_digest(case_id: &str, round_id: &str, juror: &AccountId) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(JUROR_KEY_DOMAIN_V1);
    update_string(&mut hasher, case_id);
    update_string(&mut hasher, round_id);
    update_string(&mut hasher, &juror.to_string());
    *hasher.finalize().as_bytes()
}

fn challenge_digest(case_id: &str, round_id: &str, challenge_id: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHALLENGE_KEY_DOMAIN_V1);
    update_string(&mut hasher, case_id);
    update_string(&mut hasher, round_id);
    update_string(&mut hasher, challenge_id);
    *hasher.finalize().as_bytes()
}

fn nullifier_digest(nullifier: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NULLIFIER_KEY_DOMAIN_V1);
    hasher.update(&nullifier);
    *hasher.finalize().as_bytes()
}

fn appeal_deposit_digest(deposit_lock_digest: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(APPEAL_DEPOSIT_KEY_DOMAIN_V1);
    hasher.update(&deposit_lock_digest);
    *hasher.finalize().as_bytes()
}

fn appeal_proof_token_digest(proof_token_digest: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(APPEAL_PROOF_TOKEN_KEY_DOMAIN_V1);
    hasher.update(&proof_token_digest);
    *hasher.finalize().as_bytes()
}

fn appeal_key(case_id: &str, round_id: &str) -> Name {
    digest_key(APPEAL_STATE_KEY_PREFIX, case_digest(case_id, round_id))
}

fn eligibility_key(case_id: &str, round_id: &str, juror: &AccountId) -> Name {
    digest_key(
        ELIGIBILITY_STATE_KEY_PREFIX,
        juror_digest(case_id, round_id, juror),
    )
}

fn nullifier_key(nullifier: [u8; 32]) -> Name {
    digest_key(NULLIFIER_STATE_KEY_PREFIX, nullifier_digest(nullifier))
}

fn appeal_deposit_key(deposit_lock_digest: [u8; 32]) -> Name {
    digest_key(
        APPEAL_DEPOSIT_STATE_KEY_PREFIX,
        appeal_deposit_digest(deposit_lock_digest),
    )
}

fn appeal_proof_token_key(proof_token_digest: [u8; 32]) -> Name {
    digest_key(
        APPEAL_PROOF_TOKEN_STATE_KEY_PREFIX,
        appeal_proof_token_digest(proof_token_digest),
    )
}

fn case_key(case_id: &str, round_id: &str) -> Name {
    digest_key(CASE_STATE_KEY_PREFIX, case_digest(case_id, round_id))
}

fn commit_key(case_id: &str, round_id: &str, juror: &AccountId) -> Name {
    digest_key(
        COMMIT_STATE_KEY_PREFIX,
        juror_digest(case_id, round_id, juror),
    )
}

fn reveal_key(case_id: &str, round_id: &str, juror: &AccountId) -> Name {
    digest_key(
        REVEAL_STATE_KEY_PREFIX,
        juror_digest(case_id, round_id, juror),
    )
}

fn challenge_key(case_id: &str, round_id: &str, challenge_id: &str) -> Name {
    digest_key(
        CHALLENGE_STATE_KEY_PREFIX,
        challenge_digest(case_id, round_id, challenge_id),
    )
}

fn outcome_key(case_id: &str, round_id: &str) -> Name {
    digest_key(OUTCOME_STATE_KEY_PREFIX, case_digest(case_id, round_id))
}

fn no_show_key(case_id: &str, round_id: &str, juror: &AccountId) -> Name {
    digest_key(
        NO_SHOW_STATE_KEY_PREFIX,
        juror_digest(case_id, round_id, juror),
    )
}

fn encode_state<T: norito::core::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, InstructionExecutionError> {
    norito::to_bytes(value)
        .map_err(|error| corrupt_state(format!("failed to encode {label}: {error}")))
}

fn decode_state<T>(bytes: &[u8], label: &str) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} state exceeds {STATE_MAX_BYTES} bytes"
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, STATE_LIMITS)
        .map_err(|error| corrupt_state(format!("failed to decode {label}: {error}")))?;
    if encode_state(&value, label)? != bytes {
        return Err(corrupt_state(format!(
            "{label} state is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn decode_payload<T>(bytes: &[u8], label: &str) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.is_empty() || bytes.len() > PAYLOAD_MAX_BYTES {
        return Err(invalid_parameter(format!(
            "{label} payload length {} is outside 1..={PAYLOAD_MAX_BYTES}",
            bytes.len()
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, PAYLOAD_LIMITS).map_err(|error| {
        invalid_parameter(format!("invalid canonical {label} payload: {error}"))
    })?;
    let canonical = norito::to_bytes(&value)
        .map_err(|error| invalid_parameter(format!("failed to canonicalize {label}: {error}")))?;
    if canonical != bytes {
        return Err(invalid_parameter(format!(
            "{label} payload is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn decode_membership_proof(
    bytes: &[u8],
) -> Result<PopMembershipProofV1, InstructionExecutionError> {
    let maximum = POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 + 32 * 1024;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid_parameter(format!(
            "moderation PoP membership proof length {} is outside 1..={maximum}",
            bytes.len()
        )));
    }
    let proof = decode_from_bytes_with_limits::<PopMembershipProofV1>(bytes, PROOF_LIMITS)
        .map_err(|error| {
            invalid_parameter(format!(
                "invalid canonical moderation PoP membership proof: {error}"
            ))
        })?;
    let canonical = norito::to_bytes(&proof).map_err(|error| {
        invalid_parameter(format!(
            "failed to canonicalize moderation PoP membership proof: {error}"
        ))
    })?;
    if canonical != bytes {
        return Err(invalid_parameter(
            "moderation PoP membership proof is not exact canonical Norito",
        ));
    }
    Ok(proof)
}

fn read_policy(
    world: &impl WorldReadOnly,
) -> Result<Option<ModerationLedgerPolicyRecord>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(policy_key()) else {
        return Ok(None);
    };
    let record: ModerationLedgerPolicyRecord = decode_state(bytes, "moderation policy")?;
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored moderation policy: {error}")))?;
    let digest = record
        .policy
        .digest()
        .map_err(|error| corrupt_state(format!("failed to digest stored policy: {error}")))?;
    if digest != record.policy_digest
        || record.policy_digest == [0; 32]
        || record.activated_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored moderation policy provenance is inconsistent",
        ));
    }
    Ok(Some(record))
}

fn read_status(
    world: &impl WorldReadOnly,
) -> Result<Option<ModerationLedgerStatusV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(status_key()) else {
        return Ok(None);
    };
    let status: ModerationLedgerStatusV1 = decode_state(bytes, "moderation status")?;
    if status.updated_at_unix_ms == 0
        || status.finalized_cases != status.outcomes
        || status.panel_selections > status.appeal_intakes
        || status.failed_panel_formations > status.appeal_intakes
        || status.assignment_acceptances > status.eligibility_proofs
        || status.failover_replacements > status.eligibility_proofs
        || status
            .open_cases
            .checked_add(status.finalized_cases)
            .is_none()
    {
        return Err(corrupt_state("stored moderation status is invalid"));
    }
    Ok(Some(status))
}

fn status_for_mutation(
    world: &impl WorldReadOnly,
    now: u64,
) -> Result<ModerationLedgerStatusV1, InstructionExecutionError> {
    let status = read_status(world)?
        .ok_or_else(|| corrupt_state("moderation policy exists without ledger status"))?;
    if now < status.updated_at_unix_ms {
        return Err(invalid_parameter(format!(
            "moderation ledger block-time rollback: {now} precedes {}",
            status.updated_at_unix_ms
        )));
    }
    Ok(status)
}

fn canonical_account_list(accounts: &[AccountId]) -> bool {
    let mut previous: Option<String> = None;
    accounts.iter().all(|account| {
        let current = account.to_string();
        let valid = previous.as_ref().is_none_or(|value| value < &current);
        previous = Some(current);
        valid
    })
}

fn unique_account_list(accounts: &[AccountId]) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    accounts
        .iter()
        .all(|account| seen.insert(account.to_string()))
}

fn read_appeal(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
) -> Result<Option<ModerationAppealRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&appeal_key(case_id, round_id))
    else {
        return Ok(None);
    };
    let record: ModerationAppealRecordV1 = decode_state(bytes, "moderation appeal")?;
    record
        .intake
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored moderation appeal: {error}")))?;
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored appeal policy: {error}")))?;
    record.pop_snapshot.validate().map_err(|error| {
        corrupt_state(format!("invalid stored moderation PoP snapshot: {error}"))
    })?;
    let intake_digest = record.intake.digest().map_err(|error| {
        corrupt_state(format!("failed to digest stored moderation appeal: {error}"))
    })?;
    let snapshot_digest = record.pop_snapshot.digest().map_err(|error| {
        corrupt_state(format!("failed to digest stored moderation PoP snapshot: {error}"))
    })?;
    if record.intake.case_id != case_id
        || record.intake.round_id != round_id
        || record.intake.appellant != record.submitted_by
        || record.intake_digest != intake_digest
        || record.pop_snapshot_digest != snapshot_digest
        || record.intake.policy_digest
            != record
                .policy
                .digest()
                .map_err(|error| corrupt_state(format!("failed to digest appeal policy: {error}")))?
        || record.submitted_at_unix_ms == 0
        || record.submitted_at_unix_ms != record.pop_snapshot.captured_at_unix_ms
        || record.submitted_at_unix_ms >= record.intake.registration_deadline_unix_ms
        || record.eligible_jurors.len() > usize::from(record.policy.max_candidate_pool_size)
        || !canonical_account_list(&record.eligible_jurors)
        || !canonical_account_list(&record.accepted_jurors)
        || record
            .eligible_jurors
            .iter()
            .any(|juror| record.intake.exclusions.binary_search_by(|candidate| {
                candidate.to_string().cmp(&juror.to_string())
            }).is_ok())
        || record
            .accepted_jurors
            .iter()
            .any(|juror| !record.eligible_jurors.contains(juror))
    {
        return Err(corrupt_state(
            "stored moderation appeal metadata is inconsistent",
        ));
    }
    if let Some(selection) = &record.selection {
        if selection.seed_digest == [0; 32]
            || selection.seed_digest
                != sorafs_moderation_sortition_seed_v1(
                    record.intake_digest,
                    record.pop_snapshot_digest,
                    record.pop_snapshot.randomness_anchor,
                )
            || selection.jurors.len() != usize::from(record.intake.panel_size)
            || selection.waitlist.len() > usize::from(record.intake.waitlist_size)
            || !unique_account_list(&selection.jurors)
            || !unique_account_list(&selection.waitlist)
            || selection
                .jurors
                .iter()
                .any(|juror| selection.waitlist.contains(juror))
            || selection
                .jurors
                .iter()
                .chain(selection.waitlist.iter())
                .any(|juror| !record.eligible_jurors.contains(juror))
            || selection.selected_at_unix_ms <= record.intake.registration_deadline_unix_ms
            || selection.selected_at_unix_ms > record.intake.acceptance_deadline_unix_ms
            || selection.sortition_digest
                != sorafs_moderation_sortition_digest_v1(
                    record.pop_snapshot_digest,
                    selection.seed_digest,
                    &selection.jurors,
                    &selection.waitlist,
                    record.intake.quorum,
                )
            || record
                .accepted_jurors
                .iter()
                .any(|juror| !selection.jurors.contains(juror))
        {
            return Err(corrupt_state(
                "stored moderation panel selection is inconsistent",
            ));
        }
    } else if !record.accepted_jurors.is_empty() || !record.replacements.is_empty() {
        return Err(corrupt_state(
            "stored moderation appeal has assignment state without sortition",
        ));
    }
    let mut absent = std::collections::BTreeSet::new();
    let mut replacements = std::collections::BTreeSet::new();
    if record.replacements.iter().any(|replacement| {
        replacement.absent_juror == replacement.replacement_juror
            || !absent.insert(replacement.absent_juror.to_string())
            || !replacements.insert(replacement.replacement_juror.to_string())
            || record
                .selection
                .as_ref()
                .is_none_or(|selection| {
                    !selection.jurors.contains(&replacement.absent_juror)
                        || !selection.waitlist.contains(&replacement.replacement_juror)
                })
    }) {
        return Err(corrupt_state(
            "stored moderation failover replacements are inconsistent",
        ));
    }
    let (expected_replacements, failover_exhausted) = record.selection.as_ref().map_or_else(
        || (Vec::new(), false),
        |selection| {
            let missing_primaries = selection
                .jurors
                .iter()
                .filter(|juror| !record.accepted_jurors.contains(juror))
                .collect::<Vec<_>>();
            let replacements = missing_primaries
                .iter()
                .zip(selection.waitlist.iter())
                .map(|(absent, replacement)| ModerationJurorReplacementV1 {
                    absent_juror: (*absent).clone(),
                    replacement_juror: replacement.clone(),
                })
                .collect::<Vec<_>>();
            (replacements, missing_primaries.len() > selection.waitlist.len())
        },
    );
    let lifecycle_valid = match record.status {
        ModerationAppealStatusV1::RegisteringJurors => {
            record.selection.is_none()
                && record.accepted_jurors.is_empty()
                && record.replacements.is_empty()
                && record.activated_at_unix_ms.is_none()
                && record.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::AwaitingAcceptance => {
            record.selection.is_some()
                && record.replacements.is_empty()
                && record.activated_at_unix_ms.is_none()
                && record.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::BallotOpen => {
            record.selection.is_some()
                && !failover_exhausted
                && record.replacements == expected_replacements
                && record.activated_at_unix_ms.is_some_and(|time| {
                    time > record.intake.acceptance_deadline_unix_ms
                        && time < record.intake.commit_deadline_unix_ms
                })
                && record.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::InsufficientEligiblePool => {
            record.selection.is_none()
                && record.activated_at_unix_ms.is_none()
                && record.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::FailoverExhausted => {
            record.selection.is_some()
                && failover_exhausted
                && record.replacements == expected_replacements
                && record.activated_at_unix_ms.is_none()
                && record.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::Finalized => {
            record.selection.is_some()
                && !failover_exhausted
                && record.replacements == expected_replacements
                && record.activated_at_unix_ms.is_some()
                && record.finalized_at_unix_ms.is_some_and(|time| {
                    time > record.intake.reveal_deadline_unix_ms
                        && record.activated_at_unix_ms.is_some_and(|opened| time > opened)
                })
        }
    };
    if !lifecycle_valid {
        return Err(corrupt_state(
            "stored moderation appeal lifecycle is inconsistent",
        ));
    }
    Ok(Some(record))
}

fn required_appeal(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
) -> Result<ModerationAppealRecordV1, InstructionExecutionError> {
    read_appeal(world, case_id, round_id)?.ok_or_else(|| {
        invalid_parameter(format!(
            "moderation appeal `{case_id}` round `{round_id}` does not exist"
        ))
    })
}

fn read_appeal_deposit_binding(
    world: &impl WorldReadOnly,
    deposit_lock_digest: [u8; 32],
) -> Result<Option<AppealDepositBindingStateV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&appeal_deposit_key(deposit_lock_digest))
    else {
        return Ok(None);
    };
    let binding: AppealDepositBindingStateV1 =
        decode_state(bytes, "moderation appeal deposit binding")?;
    if binding.deposit_lock_digest != deposit_lock_digest
        || binding.intake_digest == [0; 32]
        || validate_lookup_identifiers(&binding.case_id, &binding.round_id).is_err()
    {
        return Err(corrupt_state(
            "stored moderation appeal deposit binding is inconsistent",
        ));
    }
    let primary = read_appeal(world, &binding.case_id, &binding.round_id)?
        .ok_or_else(|| corrupt_state("moderation appeal deposit binding has no appeal"))?;
    if primary.intake_digest != binding.intake_digest
        || primary.intake.appeal_deposit_lock_digest != deposit_lock_digest
    {
        return Err(corrupt_state(
            "moderation appeal deposit binding disagrees with its appeal",
        ));
    }
    Ok(Some(binding))
}

fn read_appeal_proof_token_binding(
    world: &impl WorldReadOnly,
    proof_token_digest: [u8; 32],
) -> Result<Option<AppealProofTokenBindingStateV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&appeal_proof_token_key(proof_token_digest))
    else {
        return Ok(None);
    };
    let binding: AppealProofTokenBindingStateV1 =
        decode_state(bytes, "moderation appeal proof-token binding")?;
    if binding.proof_token_digest != proof_token_digest
        || binding.intake_digest == [0; 32]
        || validate_lookup_identifiers(&binding.case_id, &binding.round_id).is_err()
    {
        return Err(corrupt_state(
            "stored moderation appeal proof-token binding is inconsistent",
        ));
    }
    let primary = read_appeal(world, &binding.case_id, &binding.round_id)?
        .ok_or_else(|| corrupt_state("moderation appeal proof-token binding has no appeal"))?;
    if primary.intake_digest != binding.intake_digest
        || primary.intake.proof_token_digest != proof_token_digest
    {
        return Err(corrupt_state(
            "moderation appeal proof-token binding disagrees with its appeal",
        ));
    }
    Ok(Some(binding))
}

fn read_eligibility(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
    juror: &AccountId,
) -> Result<Option<ModerationJurorEligibilityRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&eligibility_key(case_id, round_id, juror))
    else {
        return Ok(None);
    };
    let record: ModerationJurorEligibilityRecordV1 =
        decode_state(bytes, "moderation juror eligibility")?;
    if record.case_id != case_id
        || record.round_id != round_id
        || &record.juror != juror
        || record.eligibility_class == ModerationJurorEligibilityClassV1::Observer
        || record.proof_digest == [0; 32]
        || record.nullifier == [0; 32]
        || record.pop_snapshot_digest == [0; 32]
        || record.credential_expires_at_epoch == 0
        || record.registered_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored moderation juror eligibility is inconsistent",
        ));
    }
    let appeal = read_appeal(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("moderation eligibility has no appeal"))?;
    if record.pop_snapshot_digest != appeal.pop_snapshot_digest
        || !appeal.eligible_jurors.contains(juror)
        || record.registered_at_unix_ms < appeal.submitted_at_unix_ms
        || record.registered_at_unix_ms > appeal.intake.registration_deadline_unix_ms
        || record.credential_expires_at_epoch
            <= appeal.intake.reveal_deadline_unix_ms.saturating_add(999) / 1_000
    {
        return Err(corrupt_state(
            "stored moderation eligibility does not match its appeal",
        ));
    }
    Ok(Some(record))
}

fn read_nullifier(
    world: &impl WorldReadOnly,
    nullifier: [u8; 32],
) -> Result<Option<ModerationJurorEligibilityRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&nullifier_key(nullifier)) else {
        return Ok(None);
    };
    let record: ModerationJurorEligibilityRecordV1 =
        decode_state(bytes, "moderation PoP nullifier")?;
    if record.nullifier != nullifier {
        return Err(corrupt_state(
            "stored moderation PoP nullifier key is inconsistent",
        ));
    }
    let primary = read_eligibility(world, &record.case_id, &record.round_id, &record.juror)?
        .ok_or_else(|| corrupt_state("moderation PoP nullifier has no eligibility record"))?;
    if primary != record {
        return Err(corrupt_state(
            "moderation PoP nullifier disagrees with eligibility record",
        ));
    }
    Ok(Some(record))
}

fn read_case(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
) -> Result<Option<ModerationCaseRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&case_key(case_id, round_id))
    else {
        return Ok(None);
    };
    let record: ModerationCaseRecordV1 = decode_state(bytes, "moderation case")?;
    record
        .spec
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored moderation case: {error}")))?;
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored case policy: {error}")))?;
    let digest = record
        .policy
        .digest()
        .map_err(|error| corrupt_state(format!("failed to digest stored case policy: {error}")))?;
    let roster_size = u32::try_from(record.spec.jurors.len())
        .map_err(|_| corrupt_state("stored moderation roster length does not fit u32"))?;
    let total_window = record
        .spec
        .reveal_deadline_unix_ms
        .checked_sub(record.opened_at_unix_ms)
        .ok_or_else(|| corrupt_state("stored moderation case reveal deadline precedes opening"))?;
    if record.spec.context.case_id != case_id
        || record.spec.round_id != round_id
        || record.spec.policy_digest != digest
        || record.opened_at_unix_ms == 0
        || record.opened_at_unix_ms >= record.spec.commit_deadline_unix_ms
        || total_window > record.policy.max_total_window_ms
        || record.spec.jurors.len() > usize::from(record.policy.max_panel_size)
        || record.commitment_count > roster_size
        || record.reveal_count > record.commitment_count
        || record.challenge_count > u32::from(record.policy.max_challenges_per_case)
        || record.pending_challenge_count > record.challenge_count
        || record.accepted_challenge_count > record.challenge_count
        || record
            .pending_challenge_count
            .checked_add(record.accepted_challenge_count)
            .is_none_or(|resolved_or_pending| resolved_or_pending > record.challenge_count)
    {
        return Err(corrupt_state(
            "stored moderation case metadata is inconsistent",
        ));
    }
    match record.status {
        ModerationCaseStatusV1::Open if record.accepted_challenge_count == 0 => {}
        ModerationCaseStatusV1::Challenged if record.accepted_challenge_count > 0 => {}
        ModerationCaseStatusV1::Finalized => {}
        _ => {
            return Err(corrupt_state(
                "stored moderation case status/challenge state is inconsistent",
            ));
        }
    }
    let outcome = read_outcome(world, case_id, round_id)?;
    match (record.status, outcome) {
        (ModerationCaseStatusV1::Finalized, Some(outcome))
            if outcome.quorum == record.spec.quorum
                && outcome.finalized_at_unix_ms > record.spec.reveal_deadline_unix_ms
                && outcome.votes_total <= roster_size
                && outcome.votes_total == record.reveal_count
                && record.pending_challenge_count == 0
                && matches!(outcome.kind, ModerationOutcomeKindV1::Challenged)
                    == (record.accepted_challenge_count > 0)
                && (matches!(outcome.kind, ModerationOutcomeKindV1::Challenged)
                    && outcome.no_show_count == 0
                    || !matches!(outcome.kind, ModerationOutcomeKindV1::Challenged)
                        && roster_size.checked_sub(outcome.votes_total)
                            == Some(outcome.no_show_count)) => {}
        (ModerationCaseStatusV1::Open | ModerationCaseStatusV1::Challenged, None) => {}
        _ => {
            return Err(corrupt_state(
                "stored moderation case and terminal outcome are inconsistent",
            ));
        }
    }
    Ok(Some(record))
}

fn read_commit(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
    juror: &AccountId,
) -> Result<Option<ModerationCommitRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&commit_key(case_id, round_id, juror))
    else {
        return Ok(None);
    };
    let record: ModerationCommitRecordV1 = decode_state(bytes, "moderation commit")?;
    let commit: SoraFsModerationBallotCommitV1 =
        decode_payload(&record.canonical_commit, "stored moderation commit")
            .map_err(|error| corrupt_state(error.to_string()))?;
    commit
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored moderation commit: {error}")))?;
    if record.case_id != case_id
        || record.round_id != round_id
        || &record.juror != juror
        || commit.context.case_id != case_id
        || commit.round_id != round_id
        || commit.juror_id != juror.to_string()
        || commit.committed_at_unix_ms != record.accepted_at_unix_ms
        || record.accepted_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored moderation commitment provenance is inconsistent",
        ));
    }
    let case = read_case(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("stored moderation commitment has no authoritative case"))?;
    if commit.context != case.spec.context
        || !case.spec.jurors.iter().any(|candidate| candidate == juror)
        || record.accepted_at_unix_ms < case.opened_at_unix_ms
        || record.accepted_at_unix_ms > case.spec.commit_deadline_unix_ms
    {
        return Err(corrupt_state(
            "stored moderation commitment does not match authoritative case state",
        ));
    }
    Ok(Some(record))
}

fn read_reveal(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
    juror: &AccountId,
) -> Result<Option<ModerationRevealRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&reveal_key(case_id, round_id, juror))
    else {
        return Ok(None);
    };
    let record: ModerationRevealRecordV1 = decode_state(bytes, "moderation reveal")?;
    let reveal: SoraFsModerationBallotRevealV1 =
        decode_payload(&record.canonical_reveal, "stored moderation reveal")
            .map_err(|error| corrupt_state(error.to_string()))?;
    reveal
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored moderation reveal: {error}")))?;
    if record.case_id != case_id
        || record.round_id != round_id
        || &record.juror != juror
        || reveal.context.case_id != case_id
        || reveal.round_id != round_id
        || reveal.juror_id != juror.to_string()
        || reveal.revealed_at_unix_ms != record.accepted_at_unix_ms
        || record.accepted_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored moderation reveal provenance is inconsistent",
        ));
    }
    let case = read_case(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("stored moderation reveal has no authoritative case"))?;
    let commit = read_commit(world, case_id, round_id, juror)?
        .ok_or_else(|| corrupt_state("stored moderation reveal has no commitment"))?;
    let commit: SoraFsModerationBallotCommitV1 =
        decode_payload(&commit.canonical_commit, "stored moderation commit")
            .map_err(|error| corrupt_state(error.to_string()))?;
    if reveal.context != case.spec.context
        || !case.spec.jurors.iter().any(|candidate| candidate == juror)
        || record.accepted_at_unix_ms <= case.spec.challenge_deadline_unix_ms
        || record.accepted_at_unix_ms > case.spec.reveal_deadline_unix_ms
        || commit.verify_reveal(&reveal).is_err()
    {
        return Err(corrupt_state(
            "stored moderation reveal does not match authoritative case state",
        ));
    }
    Ok(Some(record))
}

fn read_challenge(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
    challenge_id: &str,
) -> Result<Option<ModerationChallengeRecordV1>, InstructionExecutionError> {
    let Some(bytes) =
        world
            .smart_contract_state()
            .get(&challenge_key(case_id, round_id, challenge_id))
    else {
        return Ok(None);
    };
    let record: ModerationChallengeRecordV1 = decode_state(bytes, "moderation challenge")?;
    if record.case_id != case_id
        || record.round_id != round_id
        || record.challenge_id != challenge_id
        || record.challenge_id.trim().is_empty()
        || record.challenge_id != record.challenge_id.trim()
        || record.challenge_id.len() > MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1
        || record.evidence_digest == [0; 32]
        || record.raised_at_unix_ms == 0
        || record.reason.trim().is_empty()
        || record.reason != record.reason.trim()
        || record.reason.len() > MODERATION_LEDGER_MAX_REASON_BYTES_V1
        || record.kind.requires_target_juror() && record.target_juror.is_none()
    {
        return Err(corrupt_state(
            "stored moderation challenge metadata is inconsistent",
        ));
    }
    match (
        record.decision,
        record.resolved_by.as_ref(),
        record.resolved_at_unix_ms,
    ) {
        (None, None, None) => {}
        (Some(_), Some(_), Some(resolved_at)) if resolved_at >= record.raised_at_unix_ms => {}
        _ => {
            return Err(corrupt_state(
                "stored moderation challenge resolution is inconsistent",
            ));
        }
    }
    let case = read_case(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("stored moderation challenge has no authoritative case"))?;
    if record.raised_at_unix_ms <= case.spec.commit_deadline_unix_ms
        || record.raised_at_unix_ms > case.spec.challenge_deadline_unix_ms
        || record
            .target_juror
            .as_ref()
            .is_some_and(|target| !case.spec.jurors.iter().any(|juror| juror == target))
    {
        return Err(corrupt_state(
            "stored moderation challenge does not match authoritative case state",
        ));
    }
    Ok(Some(record))
}

fn read_outcome(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
) -> Result<Option<ModerationOutcomeRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&outcome_key(case_id, round_id))
    else {
        return Ok(None);
    };
    let record: ModerationOutcomeRecordV1 = decode_state(bytes, "moderation outcome")?;
    let kind_consistent = match record.kind {
        ModerationOutcomeKindV1::Decided(choice) => {
            record.votes_total >= u32::from(record.quorum)
                && record.counts.winning_choice() == Some(choice)
        }
        ModerationOutcomeKindV1::Contested => {
            record.votes_total >= u32::from(record.quorum)
                && record.counts.winning_choice().is_none()
        }
        ModerationOutcomeKindV1::QuorumNotMet => record.votes_total < u32::from(record.quorum),
        ModerationOutcomeKindV1::Challenged => record.votes_total == 0 && record.no_show_count == 0,
    };
    if record.case_id != case_id
        || record.round_id != round_id
        || record.finalized_at_unix_ms == 0
        || record.quorum == 0
        || record.counts.checked_total() != Some(record.votes_total)
        || !kind_consistent
    {
        return Err(corrupt_state(
            "stored moderation outcome metadata is inconsistent",
        ));
    }
    Ok(Some(record))
}

#[allow(clippy::too_many_lines)]
fn read_no_show(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
    juror: &AccountId,
) -> Result<Option<ModerationNoShowRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&no_show_key(case_id, round_id, juror))
    else {
        return Ok(None);
    };
    let record: ModerationNoShowRecordV1 = decode_state(bytes, "moderation no-show")?;
    if record.case_id != case_id
        || record.round_id != round_id
        || &record.juror != juror
        || record.penalty_points == 0
        || record.policy_digest == [0; 32]
        || record.recorded_at_unix_ms == 0
    {
        return Err(corrupt_state(
            "stored moderation no-show metadata is inconsistent",
        ));
    }
    let case = read_case(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("stored moderation no-show has no authoritative case"))?;
    let outcome = read_outcome(world, case_id, round_id)?
        .ok_or_else(|| corrupt_state("stored moderation no-show has no terminal outcome"))?;
    let expected_penalty = match record.kind {
        ModerationNoShowKindV1::MissingCommit => case.policy.missing_commit_penalty_points,
        ModerationNoShowKindV1::UnrevealedCommit => case.policy.unrevealed_commit_penalty_points,
    };
    let has_commit = read_commit(world, case_id, round_id, juror)?.is_some();
    let has_reveal = read_reveal(world, case_id, round_id, juror)?.is_some();
    if case.status != ModerationCaseStatusV1::Finalized
        || !case.spec.jurors.iter().any(|candidate| candidate == juror)
        || outcome.no_show_count == 0
        || matches!(outcome.kind, ModerationOutcomeKindV1::Challenged)
        || record.policy_digest != case.spec.policy_digest
        || record.penalty_points != expected_penalty
        || has_reveal
        || matches!(record.kind, ModerationNoShowKindV1::MissingCommit) && has_commit
        || matches!(record.kind, ModerationNoShowKindV1::UnrevealedCommit) && !has_commit
    {
        return Err(corrupt_state(
            "stored moderation no-show does not match authoritative ballot state",
        ));
    }
    Ok(Some(record))
}

fn required_case(
    world: &impl WorldReadOnly,
    case_id: &str,
    round_id: &str,
) -> Result<ModerationCaseRecordV1, InstructionExecutionError> {
    read_case(world, case_id, round_id)?.ok_or_else(|| {
        invalid_parameter(format!(
            "moderation case `{case_id}` round `{round_id}` does not exist"
        ))
    })
}

fn ensure_case_open(case: &ModerationCaseRecordV1) -> Result<(), InstructionExecutionError> {
    if case.status == ModerationCaseStatusV1::Open {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "moderation case `{}` round `{}` is not open",
            case.spec.context.case_id, case.spec.round_id
        )))
    }
}

fn ensure_juror(
    case: &ModerationCaseRecordV1,
    juror: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if case.spec.jurors.iter().any(|candidate| candidate == juror) {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "account {juror} is not an eligible juror for moderation case `{}` round `{}`",
            case.spec.context.case_id, case.spec.round_id
        )))
    }
}

fn checked_inc(value: u64, label: &str) -> Result<u64, InstructionExecutionError> {
    value
        .checked_add(1)
        .ok_or_else(|| corrupt_state(format!("moderation {label} counter overflow")))
}

fn checked_add(value: u64, addend: u64, label: &str) -> Result<u64, InstructionExecutionError> {
    value
        .checked_add(addend)
        .ok_or_else(|| corrupt_state(format!("moderation {label} counter overflow")))
}

fn encode_status(status: &ModerationLedgerStatusV1) -> Result<Vec<u8>, InstructionExecutionError> {
    encode_state(status, "moderation status")
}

impl Execute for SetSorafsModerationPolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        self.policy.validate().map_err(|error| {
            invalid_parameter(format!("invalid SoraFS moderation policy: {error}"))
        })?;
        let now = block_time_ms(state_transaction)?;
        let digest = self.policy.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest moderation policy: {error}"))
        })?;

        let current = read_policy(state_transaction.world())?;
        let mut status = match current.as_ref() {
            None => {
                if self.policy.revision != 1 || self.policy.predecessor_policy_digest.is_some() {
                    return Err(invalid_parameter(
                        "first moderation policy must be revision one without a predecessor",
                    ));
                }
                if read_status(state_transaction.world())?.is_some() {
                    return Err(corrupt_state(
                        "moderation status exists without an active policy",
                    ));
                }
                ModerationLedgerStatusV1 {
                    updated_at_unix_ms: now,
                    ..ModerationLedgerStatusV1::default()
                }
            }
            Some(current) => {
                let status = status_for_mutation(state_transaction.world(), now)?;
                let expected_revision = current
                    .policy
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("moderation policy revision overflow"))?;
                if self.policy.revision != expected_revision {
                    return Err(invalid_parameter(format!(
                        "moderation policy revision {} must follow active revision {}",
                        self.policy.revision, current.policy.revision
                    )));
                }
                if self.policy.predecessor_policy_digest != Some(current.policy_digest) {
                    return Err(invalid_parameter(
                        "moderation policy predecessor does not match the active digest",
                    ));
                }
                status
            }
        };
        status.updated_at_unix_ms = now;

        let record = ModerationLedgerPolicyRecord {
            policy: self.policy,
            policy_digest: digest,
            activated_at_unix_ms: now,
            activated_by: authority.clone(),
        };
        let encoded_policy = encode_state(&record, "moderation policy")?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(policy_key().clone(), encoded_policy);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for SubmitSorafsModerationAppeal {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        self.intake.validate().map_err(|error| {
            invalid_parameter(format!("invalid SoraFS moderation appeal intake: {error}"))
        })?;
        if &self.intake.appellant != authority {
            return Err(invalid_parameter(
                "moderation appeal appellant must equal the transaction authority",
            ));
        }
        if state_transaction.world.accounts.get(authority).is_none() {
            return Err(invalid_parameter(
                "moderation appeal appellant account is not registered",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let policy = read_policy(state_transaction.world())?.ok_or_else(|| {
            invalid_parameter("authoritative moderation policy is not configured")
        })?;
        if self.intake.policy_digest != policy.policy_digest {
            return Err(invalid_parameter(
                "moderation appeal policy digest does not match the active policy",
            ));
        }
        if self.intake.panel_size > policy.policy.max_panel_size
            || self.intake.waitlist_size > policy.policy.max_waitlist_size
            || self.intake.exclusions.len()
                > usize::from(policy.policy.max_exclusions_per_case)
        {
            return Err(invalid_parameter(
                "moderation appeal panel, waitlist, or exclusion bounds exceed active policy",
            ));
        }
        if now >= self.intake.registration_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation appeal registration deadline must be later than intake",
            ));
        }
        let total_window = self
            .intake
            .reveal_deadline_unix_ms
            .checked_sub(now)
            .ok_or_else(|| invalid_parameter("moderation appeal reveal deadline is in the past"))?;
        if total_window > policy.policy.max_total_window_ms {
            return Err(invalid_parameter(format!(
                "moderation appeal total window {total_window} ms exceeds active policy limit {} ms",
                policy.policy.max_total_window_ms
            )));
        }
        if read_appeal(
            state_transaction.world(),
            &self.intake.case_id,
            &self.intake.round_id,
        )?
        .is_some()
            || read_case(
                state_transaction.world(),
                &self.intake.case_id,
                &self.intake.round_id,
            )?
            .is_some()
            || read_outcome(
                state_transaction.world(),
                &self.intake.case_id,
                &self.intake.round_id,
            )?
            .is_some()
        {
            return Err(invalid_parameter(format!(
                "moderation appeal `{}` round `{}` already exists",
                self.intake.case_id, self.intake.round_id
            )));
        }
        if read_appeal_deposit_binding(
            state_transaction.world(),
            self.intake.appeal_deposit_lock_digest,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "moderation appeal deposit lock was already consumed by another intake",
            ));
        }
        if read_appeal_proof_token_binding(
            state_transaction.world(),
            self.intake.proof_token_digest,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "moderation appeal proof token was already consumed by another intake",
            ));
        }
        let (pop_snapshot, _) = active_pop_snapshot(state_transaction, now)?;
        let pop_snapshot_digest = pop_snapshot.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest active PoP snapshot: {error}"))
        })?;
        let intake_digest = self.intake.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest moderation appeal intake: {error}"))
        })?;
        let record = ModerationAppealRecordV1 {
            intake: self.intake,
            intake_digest,
            policy: policy.policy,
            pop_snapshot,
            pop_snapshot_digest,
            status: ModerationAppealStatusV1::RegisteringJurors,
            submitted_by: authority.clone(),
            submitted_at_unix_ms: now,
            eligible_jurors: Vec::new(),
            selection: None,
            accepted_jurors: Vec::new(),
            replacements: Vec::new(),
            activated_at_unix_ms: None,
            finalized_at_unix_ms: None,
        };
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.appeal_intakes = checked_inc(status.appeal_intakes, "appeal-intake")?;
        status.updated_at_unix_ms = now;
        let key = appeal_key(&record.intake.case_id, &record.intake.round_id);
        let encoded_record = encode_state(&record, "moderation appeal")?;
        let deposit_binding = AppealDepositBindingStateV1 {
            deposit_lock_digest: record.intake.appeal_deposit_lock_digest,
            case_id: record.intake.case_id.clone(),
            round_id: record.intake.round_id.clone(),
            intake_digest: record.intake_digest,
        };
        let encoded_deposit_binding =
            encode_state(&deposit_binding, "moderation appeal deposit binding")?;
        let proof_token_binding = AppealProofTokenBindingStateV1 {
            proof_token_digest: record.intake.proof_token_digest,
            case_id: record.intake.case_id.clone(),
            round_id: record.intake.round_id.clone(),
            intake_digest: record.intake_digest,
        };
        let encoded_proof_token_binding = encode_state(
            &proof_token_binding,
            "moderation appeal proof-token binding",
        )?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(key, encoded_record);
        state_transaction.world.smart_contract_state.insert(
            appeal_deposit_key(deposit_binding.deposit_lock_digest),
            encoded_deposit_binding,
        );
        state_transaction.world.smart_contract_state.insert(
            appeal_proof_token_key(proof_token_binding.proof_token_digest),
            encoded_proof_token_binding,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for RegisterSorafsModerationJurorEligibility {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        validate_lookup_identifiers(&self.case_id, &self.round_id)?;
        if state_transaction.world.accounts.get(authority).is_none() {
            return Err(invalid_parameter(
                "moderation juror eligibility authority is not a registered account",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let mut appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::RegisteringJurors {
            return Err(invalid_parameter(
                "moderation juror eligibility registration is not in the registration phase",
            ));
        }
        if now > appeal.intake.registration_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation juror eligibility registration window is closed",
            ));
        }
        if appeal.eligible_jurors.len() >= usize::from(appeal.policy.max_candidate_pool_size) {
            return Err(invalid_parameter(
                "moderation juror candidate pool reached the active policy bound",
            ));
        }
        if appeal.intake.exclusions.binary_search_by(|candidate| {
            candidate.to_string().cmp(&authority.to_string())
        }).is_ok() {
            return Err(invalid_parameter(
                "moderation juror is excluded by the immutable conflict list",
            ));
        }
        if read_eligibility(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
            authority,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "moderation juror eligibility was already registered",
            ));
        }
        let active = require_active_pop_snapshot(state_transaction, &appeal.pop_snapshot)?;
        let proof = decode_membership_proof(&self.membership_proof_payload)?;
        if read_nullifier(state_transaction.world(), proof.nullifier)?.is_some() {
            return Err(invalid_parameter(
                "moderation juror PoP membership proof nullifier was already consumed",
            ));
        }
        let now_epoch = now / 1_000;
        let challenge =
            sorafs_moderation_pop_challenge_v1(appeal.intake_digest, appeal.pop_snapshot_digest);
        let verifier_context = sorafs_moderation_pop_verifier_context_v1(appeal.intake_digest);
        verify_pop_membership_proof_v1(
            &proof,
            &active.root,
            &active.revocations,
            challenge,
            &verifier_context,
            now_epoch,
            &[],
        )
        .map_err(|error| {
            invalid_parameter(format!(
                "moderation juror PoP membership proof failed: {error}"
            ))
        })?;
        let class = eligibility_class(proof.eligibility_class);
        if class == ModerationJurorEligibilityClassV1::Observer {
            return Err(invalid_parameter(
                "observer-only PoP credentials cannot enter a moderation voting panel",
            ));
        }
        let reveal_deadline_epoch = appeal
            .intake
            .reveal_deadline_unix_ms
            .saturating_add(999)
            / 1_000;
        if proof.expires_at_epoch <= reveal_deadline_epoch {
            return Err(invalid_parameter(
                "moderation juror PoP credential expires before the reveal deadline",
            ));
        }
        let mut proof_hasher = blake3::Hasher::new();
        proof_hasher.update(PROOF_DIGEST_DOMAIN_V1);
        proof_hasher.update(&self.membership_proof_payload);
        let record = ModerationJurorEligibilityRecordV1 {
            case_id: self.case_id,
            round_id: self.round_id,
            juror: authority.clone(),
            eligibility_class: class,
            proof_digest: *proof_hasher.finalize().as_bytes(),
            nullifier: proof.nullifier,
            pop_snapshot_digest: appeal.pop_snapshot_digest,
            credential_expires_at_epoch: proof.expires_at_epoch,
            registered_at_unix_ms: now,
        };
        let account = authority.to_string();
        let position = appeal
            .eligible_jurors
            .binary_search_by(|candidate| candidate.to_string().cmp(&account))
            .unwrap_or_else(|position| position);
        appeal.eligible_jurors.insert(position, authority.clone());
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.eligibility_proofs =
            checked_inc(status.eligibility_proofs, "eligibility-proof")?;
        status.updated_at_unix_ms = now;
        let encoded_record = encode_state(&record, "moderation juror eligibility")?;
        let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
        let encoded_status = encode_status(&status)?;
        state_transaction.world.smart_contract_state.insert(
            eligibility_key(&record.case_id, &record.round_id, authority),
            encoded_record.clone(),
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(nullifier_key(record.nullifier), encoded_record);
        state_transaction.world.smart_contract_state.insert(
            appeal_key(&record.case_id, &record.round_id),
            encoded_appeal,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for FinalizeSorafsModerationSortition {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        validate_lookup_identifiers(&self.case_id, &self.round_id)?;
        if self.proposed_jurors.len() > usize::from(MODERATION_LEDGER_MAX_PANEL_SIZE_V1)
            || self.proposed_waitlist.len()
                > usize::from(MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1)
        {
            return Err(invalid_parameter(
                "proposed moderation roster or waitlist exceeds hard bounds",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let mut appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::RegisteringJurors {
            return Err(invalid_parameter(
                "moderation appeal sortition is not in the registration phase",
            ));
        }
        if now <= appeal.intake.registration_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation appeal registration must close before sortition",
            ));
        }
        if now > appeal.intake.acceptance_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation appeal sortition missed the acceptance window",
            ));
        }
        if self.pop_snapshot_digest != appeal.pop_snapshot_digest {
            return Err(invalid_parameter(
                "moderation sortition PoP snapshot digest does not match the admitted appeal",
            ));
        }
        require_active_pop_snapshot(state_transaction, &appeal.pop_snapshot)?;
        let mut candidates = Vec::with_capacity(appeal.eligible_jurors.len());
        for juror in &appeal.eligible_jurors {
            candidates.push(
                read_eligibility(
                    state_transaction.world(),
                    &self.case_id,
                    &self.round_id,
                    juror,
                )?
                .ok_or_else(|| {
                    corrupt_state("appeal candidate index references missing eligibility")
                })?,
            );
        }
        let selection = sorafs_moderation_select_panel_v1(
            appeal.intake_digest,
            appeal.pop_snapshot_digest,
            appeal.pop_snapshot.randomness_anchor,
            &candidates,
            appeal.intake.panel_size,
            appeal.intake.waitlist_size,
            appeal.intake.quorum,
        );
        let (jurors, waitlist, seed_digest, sortition_digest) = match selection {
            Ok(selection) => selection,
            Err(ModerationSortitionError::InsufficientEligiblePool { .. }) => {
                if !self.proposed_jurors.is_empty() || !self.proposed_waitlist.is_empty() {
                    return Err(invalid_parameter(
                        "insufficient moderation pool cannot accept a proposed roster",
                    ));
                }
                appeal.status = ModerationAppealStatusV1::InsufficientEligiblePool;
                let mut status = status_for_mutation(state_transaction.world(), now)?;
                status.failed_panel_formations = checked_inc(
                    status.failed_panel_formations,
                    "failed-panel-formation",
                )?;
                status.updated_at_unix_ms = now;
                let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
                let encoded_status = encode_status(&status)?;
                state_transaction.world.smart_contract_state.insert(
                    appeal_key(&self.case_id, &self.round_id),
                    encoded_appeal,
                );
                state_transaction
                    .world
                    .smart_contract_state
                    .insert(status_key().clone(), encoded_status);
                return Ok(());
            }
            Err(error) => {
                return Err(corrupt_state(format!(
                    "stored moderation eligibility cannot be sorted: {error}"
                )));
            }
        };
        if !unique_account_list(&self.proposed_jurors)
            || !unique_account_list(&self.proposed_waitlist)
            || self
                .proposed_jurors
                .iter()
                .any(|juror| self.proposed_waitlist.contains(juror))
            || self.proposed_jurors != jurors
            || self.proposed_waitlist != waitlist
        {
            return Err(invalid_parameter(
                "proposed moderation roster is duplicated, biased, or differs from deterministic sortition",
            ));
        }
        appeal.selection = Some(ModerationPanelSelectionV1 {
            seed_digest,
            jurors,
            waitlist,
            sortition_digest,
            selected_at_unix_ms: now,
            selected_by: authority.clone(),
        });
        appeal.status = ModerationAppealStatusV1::AwaitingAcceptance;
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.panel_selections = checked_inc(status.panel_selections, "panel-selection")?;
        status.updated_at_unix_ms = now;
        let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
        let encoded_status = encode_status(&status)?;
        state_transaction.world.smart_contract_state.insert(
            appeal_key(&self.case_id, &self.round_id),
            encoded_appeal,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for AcceptSorafsModerationJurorAssignment {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        validate_lookup_identifiers(&self.case_id, &self.round_id)?;
        let now = block_time_ms(state_transaction)?;
        let mut appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::AwaitingAcceptance {
            return Err(invalid_parameter(
                "moderation juror assignment is not in the acceptance phase",
            ));
        }
        if now > appeal.intake.acceptance_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation juror assignment acceptance window is closed",
            ));
        }
        require_active_pop_snapshot(state_transaction, &appeal.pop_snapshot)?;
        let selection = appeal
            .selection
            .as_ref()
            .ok_or_else(|| corrupt_state("acceptance-phase appeal has no panel selection"))?;
        if self.sortition_digest != selection.sortition_digest {
            return Err(invalid_parameter(
                "moderation assignment sortition digest mismatch",
            ));
        }
        if !selection.jurors.contains(authority) {
            return Err(invalid_parameter(
                "only a selected primary juror can accept this assignment",
            ));
        }
        let account = authority.to_string();
        let position = match appeal
            .accepted_jurors
            .binary_search_by(|candidate| candidate.to_string().cmp(&account))
        {
            Ok(_) => {
                return Err(invalid_parameter(
                    "moderation juror assignment was already accepted",
                ));
            }
            Err(position) => position,
        };
        appeal.accepted_jurors.insert(position, authority.clone());
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.assignment_acceptances = checked_inc(
            status.assignment_acceptances,
            "assignment-acceptance",
        )?;
        status.updated_at_unix_ms = now;
        let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
        let encoded_status = encode_status(&status)?;
        state_transaction.world.smart_contract_state.insert(
            appeal_key(&self.case_id, &self.round_id),
            encoded_appeal,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for ActivateSorafsModerationCase {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        validate_lookup_identifiers(&self.case_id, &self.round_id)?;
        let now = block_time_ms(state_transaction)?;
        let mut appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::AwaitingAcceptance {
            return Err(invalid_parameter(
                "moderation case activation is not in the acceptance phase",
            ));
        }
        if now <= appeal.intake.acceptance_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation assignment acceptance must close before activation",
            ));
        }
        if now >= appeal.intake.commit_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation case activation missed the commit window",
            ));
        }
        require_active_pop_snapshot(state_transaction, &appeal.pop_snapshot)?;
        let selection = appeal
            .selection
            .as_ref()
            .ok_or_else(|| corrupt_state("activation-phase appeal has no panel selection"))?;
        if self.sortition_digest != selection.sortition_digest {
            return Err(invalid_parameter(
                "moderation activation sortition digest mismatch",
            ));
        }
        if read_case(state_transaction.world(), &self.case_id, &self.round_id)?.is_some() {
            return Err(corrupt_state(
                "moderation ballot exists before appeal activation",
            ));
        }
        let mut waitlist = selection.waitlist.iter();
        let mut jurors = Vec::with_capacity(selection.jurors.len());
        let mut replacements = Vec::new();
        for primary in &selection.jurors {
            if appeal.accepted_jurors.contains(primary) {
                jurors.push(primary.clone());
                continue;
            }
            let Some(replacement) = waitlist.next() else {
                appeal.status = ModerationAppealStatusV1::FailoverExhausted;
                appeal.replacements = replacements;
                let mut status = status_for_mutation(state_transaction.world(), now)?;
                status.failed_panel_formations = checked_inc(
                    status.failed_panel_formations,
                    "failed-panel-formation",
                )?;
                status.updated_at_unix_ms = now;
                let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
                let encoded_status = encode_status(&status)?;
                state_transaction.world.smart_contract_state.insert(
                    appeal_key(&self.case_id, &self.round_id),
                    encoded_appeal,
                );
                state_transaction
                    .world
                    .smart_contract_state
                    .insert(status_key().clone(), encoded_status);
                return Ok(());
            };
            jurors.push(replacement.clone());
            replacements.push(ModerationJurorReplacementV1 {
                absent_juror: primary.clone(),
                replacement_juror: replacement.clone(),
            });
        }
        if !unique_account_list(&jurors) || jurors.len() != usize::from(appeal.intake.panel_size) {
            return Err(corrupt_state(
                "deterministic moderation failover produced an invalid roster",
            ));
        }
        let context = SoraFsModerationBallotContextV1 {
            version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            case_id: appeal.intake.case_id.clone(),
            evidence_bundle_digest: appeal.intake.evidence_bundle_digest,
            appeal_finance_config_version: appeal
                .intake
                .appeal_finance_config_version
                .clone(),
            panel_roster_hash: sorafs_moderation_panel_roster_hash_v1(
                &jurors,
                appeal.intake.quorum,
            ),
            policy_reference: appeal.intake.policy_reference.clone(),
            evidence_uri: appeal.intake.evidence_uri.clone(),
        };
        let spec = ModerationCaseSpecV1 {
            version: iroha_data_model::sorafs::moderation_ledger::MODERATION_LEDGER_CASE_VERSION_V1,
            context,
            round_id: appeal.intake.round_id.clone(),
            jurors,
            quorum: appeal.intake.quorum,
            commit_deadline_unix_ms: appeal.intake.commit_deadline_unix_ms,
            challenge_deadline_unix_ms: appeal.intake.challenge_deadline_unix_ms,
            reveal_deadline_unix_ms: appeal.intake.reveal_deadline_unix_ms,
            policy_digest: appeal.intake.policy_digest,
        };
        spec.validate().map_err(|error| {
            corrupt_state(format!("deterministic moderation case is invalid: {error}"))
        })?;
        let case = ModerationCaseRecordV1 {
            spec,
            policy: appeal.policy,
            status: ModerationCaseStatusV1::Open,
            opened_at_unix_ms: now,
            opened_by: authority.clone(),
            commitment_count: 0,
            reveal_count: 0,
            challenge_count: 0,
            pending_challenge_count: 0,
            accepted_challenge_count: 0,
        };
        appeal.status = ModerationAppealStatusV1::BallotOpen;
        appeal.replacements = replacements;
        appeal.activated_at_unix_ms = Some(now);
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.open_cases = checked_inc(status.open_cases, "open-case")?;
        status.failover_replacements = checked_add(
            status.failover_replacements,
            appeal.replacements.len() as u64,
            "failover-replacement",
        )?;
        status.updated_at_unix_ms = now;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
        let encoded_status = encode_status(&status)?;
        state_transaction.world.smart_contract_state.insert(
            case_key(&self.case_id, &self.round_id),
            encoded_case,
        );
        state_transaction.world.smart_contract_state.insert(
            appeal_key(&self.case_id, &self.round_id),
            encoded_appeal,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for SubmitSorafsModerationCommit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let mut commit: SoraFsModerationBallotCommitV1 =
            decode_payload(&self.commit_payload, "moderation commit")?;
        commit.validate().map_err(|error| {
            invalid_parameter(format!("invalid moderation commitment: {error}"))
        })?;
        if commit.commitment_blake2b_256 == [0; 32] {
            return Err(invalid_parameter(
                "moderation commitment digest must be non-zero",
            ));
        }
        if commit.juror_id != authority.to_string() {
            return Err(invalid_parameter(
                "moderation commitment juror must equal the transaction authority",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let mut case = required_case(
            state_transaction.world(),
            &commit.context.case_id,
            &commit.round_id,
        )?;
        ensure_case_open(&case)?;
        ensure_juror(&case, authority)?;
        if commit.context != case.spec.context {
            return Err(invalid_parameter(
                "moderation commitment context does not match the authoritative case",
            ));
        }
        if now > case.spec.commit_deadline_unix_ms {
            return Err(invalid_parameter(format!(
                "moderation commitment phase closed at {}",
                case.spec.commit_deadline_unix_ms
            )));
        }
        if read_commit(
            state_transaction.world(),
            &commit.context.case_id,
            &commit.round_id,
            authority,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "duplicate moderation commitment for this juror and round",
            ));
        }
        commit.committed_at_unix_ms = now;
        let canonical_commit = norito::to_bytes(&commit).map_err(|error| {
            invalid_parameter(format!("failed to canonicalize moderation commit: {error}"))
        })?;
        let record = ModerationCommitRecordV1 {
            case_id: commit.context.case_id.clone(),
            round_id: commit.round_id.clone(),
            juror: authority.clone(),
            canonical_commit,
            accepted_at_unix_ms: now,
        };
        case.commitment_count = case
            .commitment_count
            .checked_add(1)
            .ok_or_else(|| corrupt_state("moderation case commitment counter overflow"))?;
        let roster_size = u32::try_from(case.spec.jurors.len())
            .map_err(|_| corrupt_state("moderation roster length does not fit u32"))?;
        if case.commitment_count > roster_size {
            return Err(corrupt_state(
                "moderation case commitment counter exceeds roster",
            ));
        }
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.commitments = checked_inc(status.commitments, "commitment")?;
        status.updated_at_unix_ms = now;

        let record_key = commit_key(&record.case_id, &record.round_id, authority);
        let case_key = case_key(&record.case_id, &record.round_id);
        let encoded_record = encode_state(&record, "moderation commit")?;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(record_key, encoded_record);
        state_transaction
            .world
            .smart_contract_state
            .insert(case_key, encoded_case);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for RaiseSorafsModerationChallenge {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        for (field, value) in [
            ("case_id", self.case_id.as_str()),
            ("round_id", self.round_id.as_str()),
            ("challenge_id", self.challenge_id.as_str()),
        ] {
            if value.trim().is_empty()
                || value != value.trim()
                || value.len() > MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1
            {
                return Err(invalid_parameter(format!(
                    "moderation challenge {field} is empty, padded, or too long"
                )));
            }
        }
        if self.evidence_digest == [0; 32] {
            return Err(invalid_parameter(
                "moderation challenge evidence digest must be non-zero",
            ));
        }
        if self.reason.trim().is_empty()
            || self.reason != self.reason.trim()
            || self.reason.len() > MODERATION_LEDGER_MAX_REASON_BYTES_V1
        {
            return Err(invalid_parameter(
                "moderation challenge reason is empty, padded, or too long",
            ));
        }
        if self.kind.requires_target_juror() && self.target_juror.is_none() {
            return Err(invalid_parameter(
                "juror-scoped moderation challenge requires target_juror",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let mut case = required_case(state_transaction.world(), &self.case_id, &self.round_id)?;
        ensure_case_open(&case)?;
        if now <= case.spec.commit_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation challenge phase has not opened",
            ));
        }
        if now > case.spec.challenge_deadline_unix_ms {
            return Err(invalid_parameter("moderation challenge phase is closed"));
        }
        if case.reveal_count != 0 {
            return Err(corrupt_state(
                "moderation case has reveals before challenge phase closure",
            ));
        }
        if let Some(target) = self.target_juror.as_ref() {
            ensure_juror(&case, target)?;
        }
        if read_challenge(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
            &self.challenge_id,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "duplicate moderation challenge id for this case and round",
            ));
        }
        if case.challenge_count >= u32::from(case.policy.max_challenges_per_case) {
            return Err(invalid_parameter(format!(
                "moderation case reached active policy challenge limit {}",
                case.policy.max_challenges_per_case
            )));
        }
        case.challenge_count = case
            .challenge_count
            .checked_add(1)
            .ok_or_else(|| corrupt_state("moderation case challenge counter overflow"))?;
        case.pending_challenge_count = case
            .pending_challenge_count
            .checked_add(1)
            .ok_or_else(|| corrupt_state("moderation pending-challenge counter overflow"))?;
        let record = ModerationChallengeRecordV1 {
            case_id: self.case_id,
            round_id: self.round_id,
            challenge_id: self.challenge_id,
            challenger: authority.clone(),
            kind: self.kind,
            target_juror: self.target_juror,
            evidence_digest: self.evidence_digest,
            reason: self.reason,
            raised_at_unix_ms: now,
            decision: None,
            resolved_by: None,
            resolved_at_unix_ms: None,
        };
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.challenges = checked_inc(status.challenges, "challenge")?;
        status.updated_at_unix_ms = now;

        let record_key = challenge_key(&record.case_id, &record.round_id, &record.challenge_id);
        let case_key = case_key(&record.case_id, &record.round_id);
        let encoded_record = encode_state(&record, "moderation challenge")?;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(record_key, encoded_record);
        state_transaction
            .world
            .smart_contract_state
            .insert(case_key, encoded_case);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for ResolveSorafsModerationChallenge {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        let now = block_time_ms(state_transaction)?;
        let appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::BallotOpen {
            return Err(invalid_parameter(
                "moderation appeal is not in the open-ballot phase",
            ));
        }
        let mut case = required_case(state_transaction.world(), &self.case_id, &self.round_id)?;
        if case.status == ModerationCaseStatusV1::Finalized {
            return Err(invalid_parameter(
                "finalized moderation case cannot resolve challenges",
            ));
        }
        if case.reveal_count != 0 {
            return Err(invalid_parameter(
                "moderation challenge cannot be resolved after reveals were accepted",
            ));
        }
        let mut record = read_challenge(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
            &self.challenge_id,
        )?
        .ok_or_else(|| invalid_parameter("moderation challenge does not exist"))?;
        if record.decision.is_some() {
            return Err(invalid_parameter(
                "moderation challenge is already resolved",
            ));
        }
        case.pending_challenge_count = case
            .pending_challenge_count
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("moderation pending-challenge counter underflow"))?;
        if self.decision == ModerationChallengeDecisionV1::Accepted {
            case.accepted_challenge_count = case
                .accepted_challenge_count
                .checked_add(1)
                .ok_or_else(|| corrupt_state("moderation accepted-challenge counter overflow"))?;
            case.status = ModerationCaseStatusV1::Challenged;
        }
        record.decision = Some(self.decision);
        record.resolved_by = Some(authority.clone());
        record.resolved_at_unix_ms = Some(now);
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.updated_at_unix_ms = now;
        let encoded_record = encode_state(&record, "moderation challenge")?;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_status = encode_status(&status)?;
        state_transaction.world.smart_contract_state.insert(
            challenge_key(&self.case_id, &self.round_id, &self.challenge_id),
            encoded_record,
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(case_key(&self.case_id, &self.round_id), encoded_case);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for SubmitSorafsModerationReveal {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let mut reveal: SoraFsModerationBallotRevealV1 =
            decode_payload(&self.reveal_payload, "moderation reveal")?;
        reveal
            .validate()
            .map_err(|error| invalid_parameter(format!("invalid moderation reveal: {error}")))?;
        if reveal.nonce.len() > MODERATION_LEDGER_MAX_NONCE_BYTES_V1 {
            return Err(invalid_parameter(format!(
                "moderation reveal nonce exceeds {} bytes",
                MODERATION_LEDGER_MAX_NONCE_BYTES_V1
            )));
        }
        if reveal.juror_id != authority.to_string() {
            return Err(invalid_parameter(
                "moderation reveal juror must equal the transaction authority",
            ));
        }
        let now = block_time_ms(state_transaction)?;
        let mut case = required_case(
            state_transaction.world(),
            &reveal.context.case_id,
            &reveal.round_id,
        )?;
        ensure_case_open(&case)?;
        ensure_juror(&case, authority)?;
        if reveal.context != case.spec.context {
            return Err(invalid_parameter(
                "moderation reveal context does not match the authoritative case",
            ));
        }
        if now <= case.spec.challenge_deadline_unix_ms {
            return Err(invalid_parameter("moderation reveal phase has not opened"));
        }
        if now > case.spec.reveal_deadline_unix_ms {
            return Err(invalid_parameter("moderation reveal phase is closed"));
        }
        if case.pending_challenge_count != 0 || case.accepted_challenge_count != 0 {
            return Err(invalid_parameter(
                "pending or accepted moderation challenge blocks reveals",
            ));
        }
        let commit_record = read_commit(
            state_transaction.world(),
            &reveal.context.case_id,
            &reveal.round_id,
            authority,
        )?
        .ok_or_else(|| invalid_parameter("moderation reveal has no accepted commitment"))?;
        if read_reveal(
            state_transaction.world(),
            &reveal.context.case_id,
            &reveal.round_id,
            authority,
        )?
        .is_some()
        {
            return Err(invalid_parameter(
                "duplicate moderation reveal for this juror and round",
            ));
        }
        let commit: SoraFsModerationBallotCommitV1 =
            decode_payload(&commit_record.canonical_commit, "stored moderation commit")
                .map_err(|error| corrupt_state(error.to_string()))?;
        commit.verify_reveal(&reveal).map_err(|error| {
            invalid_parameter(format!(
                "moderation reveal does not match commitment: {error}"
            ))
        })?;

        reveal.revealed_at_unix_ms = now;
        let canonical_reveal = norito::to_bytes(&reveal).map_err(|error| {
            invalid_parameter(format!("failed to canonicalize moderation reveal: {error}"))
        })?;
        let record = ModerationRevealRecordV1 {
            case_id: reveal.context.case_id.clone(),
            round_id: reveal.round_id.clone(),
            juror: authority.clone(),
            canonical_reveal,
            accepted_at_unix_ms: now,
        };
        case.reveal_count = case
            .reveal_count
            .checked_add(1)
            .ok_or_else(|| corrupt_state("moderation case reveal counter overflow"))?;
        if case.reveal_count > case.commitment_count {
            return Err(corrupt_state(
                "moderation case reveal counter exceeds commitments",
            ));
        }
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.reveals = checked_inc(status.reveals, "reveal")?;
        status.updated_at_unix_ms = now;

        let record_key = reveal_key(&record.case_id, &record.round_id, authority);
        let case_key = case_key(&record.case_id, &record.round_id);
        let encoded_record = encode_state(&record, "moderation reveal")?;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(record_key, encoded_record);
        state_transaction
            .world
            .smart_contract_state
            .insert(case_key, encoded_case);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

fn increment_choice(
    counts: &mut ModerationVoteCountsV1,
    choice: SoraFsModerationVoteChoice,
) -> Result<(), InstructionExecutionError> {
    let counter = match choice {
        SoraFsModerationVoteChoice::Uphold => &mut counts.uphold,
        SoraFsModerationVoteChoice::Overturn => &mut counts.overturn,
        SoraFsModerationVoteChoice::Modify => &mut counts.modify,
        SoraFsModerationVoteChoice::Escalate => &mut counts.escalate,
    };
    *counter = counter
        .checked_add(1)
        .ok_or_else(|| corrupt_state("moderation vote counter overflow"))?;
    Ok(())
}

impl Execute for FinalizeSorafsModerationCase {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        let now = block_time_ms(state_transaction)?;
        let mut appeal = required_appeal(
            state_transaction.world(),
            &self.case_id,
            &self.round_id,
        )?;
        if appeal.status != ModerationAppealStatusV1::BallotOpen {
            return Err(invalid_parameter(
                "moderation appeal is not in the open-ballot phase",
            ));
        }
        let mut case = required_case(state_transaction.world(), &self.case_id, &self.round_id)?;
        if case.status == ModerationCaseStatusV1::Finalized
            || read_outcome(state_transaction.world(), &self.case_id, &self.round_id)?.is_some()
        {
            return Err(invalid_parameter("moderation case is already finalized"));
        }
        if now <= case.spec.reveal_deadline_unix_ms {
            return Err(invalid_parameter(format!(
                "moderation case cannot finalize until after reveal deadline {}",
                case.spec.reveal_deadline_unix_ms
            )));
        }
        if case.pending_challenge_count != 0 {
            return Err(invalid_parameter(
                "pending moderation challenges must be resolved before finalization",
            ));
        }

        let challenged = case.accepted_challenge_count != 0;
        let mut counts = ModerationVoteCountsV1::default();
        let mut no_shows = Vec::new();
        if !challenged {
            for juror in &case.spec.jurors {
                if let Some(reveal_record) = read_reveal(
                    state_transaction.world(),
                    &self.case_id,
                    &self.round_id,
                    juror,
                )? {
                    let reveal: SoraFsModerationBallotRevealV1 =
                        decode_payload(&reveal_record.canonical_reveal, "stored moderation reveal")
                            .map_err(|error| corrupt_state(error.to_string()))?;
                    increment_choice(&mut counts, reveal.choice)?;
                    continue;
                }
                let (kind, penalty_points) = if read_commit(
                    state_transaction.world(),
                    &self.case_id,
                    &self.round_id,
                    juror,
                )?
                .is_some()
                {
                    (
                        ModerationNoShowKindV1::UnrevealedCommit,
                        case.policy.unrevealed_commit_penalty_points,
                    )
                } else {
                    (
                        ModerationNoShowKindV1::MissingCommit,
                        case.policy.missing_commit_penalty_points,
                    )
                };
                no_shows.push(ModerationNoShowRecordV1 {
                    case_id: self.case_id.clone(),
                    round_id: self.round_id.clone(),
                    juror: juror.clone(),
                    kind,
                    penalty_points,
                    policy_digest: case.spec.policy_digest,
                    recorded_at_unix_ms: now,
                });
            }
        }

        let votes_total = counts
            .checked_total()
            .ok_or_else(|| corrupt_state("moderation vote-total overflow"))?;
        if votes_total != case.reveal_count {
            return Err(corrupt_state(format!(
                "moderation case reveal counter {} does not match stored reveal total {votes_total}",
                case.reveal_count
            )));
        }
        let kind = if challenged {
            if votes_total != 0 || !no_shows.is_empty() {
                return Err(corrupt_state(
                    "challenged moderation case unexpectedly contains tally material",
                ));
            }
            ModerationOutcomeKindV1::Challenged
        } else if votes_total < u32::from(case.spec.quorum) {
            ModerationOutcomeKindV1::QuorumNotMet
        } else if let Some(choice) = counts.winning_choice() {
            ModerationOutcomeKindV1::Decided(choice)
        } else {
            ModerationOutcomeKindV1::Contested
        };
        let no_show_count = u32::try_from(no_shows.len())
            .map_err(|_| corrupt_state("moderation no-show count does not fit u32"))?;
        let outcome = ModerationOutcomeRecordV1 {
            case_id: self.case_id.clone(),
            round_id: self.round_id.clone(),
            kind,
            counts,
            votes_total,
            quorum: case.spec.quorum,
            no_show_count,
            finalized_at_unix_ms: now,
            finalized_by: authority.clone(),
        };
        case.status = ModerationCaseStatusV1::Finalized;
        appeal.status = ModerationAppealStatusV1::Finalized;
        appeal.finalized_at_unix_ms = Some(now);
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.open_cases = status
            .open_cases
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("moderation open-case counter underflow"))?;
        status.finalized_cases = checked_inc(status.finalized_cases, "finalized-case")?;
        status.outcomes = checked_inc(status.outcomes, "outcome")?;
        status.no_shows = checked_add(status.no_shows, u64::from(no_show_count), "no-show")?;
        status.updated_at_unix_ms = now;

        let encoded_outcome = encode_state(&outcome, "moderation outcome")?;
        let encoded_case = encode_state(&case, "moderation case")?;
        let encoded_appeal = encode_state(&appeal, "moderation appeal")?;
        let encoded_status = encode_status(&status)?;
        let mut encoded_no_shows = Vec::with_capacity(no_shows.len());
        for no_show in &no_shows {
            if read_no_show(
                state_transaction.world(),
                &self.case_id,
                &self.round_id,
                &no_show.juror,
            )?
            .is_some()
            {
                return Err(corrupt_state(
                    "moderation no-show record exists before case finalization",
                ));
            }
            encoded_no_shows.push((
                no_show_key(&self.case_id, &self.round_id, &no_show.juror),
                encode_state(no_show, "moderation no-show")?,
            ));
        }

        state_transaction
            .world
            .smart_contract_state
            .insert(outcome_key(&self.case_id, &self.round_id), encoded_outcome);
        state_transaction
            .world
            .smart_contract_state
            .insert(case_key(&self.case_id, &self.round_id), encoded_case);
        state_transaction.world.smart_contract_state.insert(
            appeal_key(&self.case_id, &self.round_id),
            encoded_appeal,
        );
        for (key, encoded) in encoded_no_shows {
            state_transaction
                .world
                .smart_contract_state
                .insert(key, encoded);
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

fn query_failure(error: InstructionExecutionError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

impl ValidSingularQuery for FindSorafsModerationPolicy {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationLedgerPolicyRecord, QueryExecutionFail> {
        read_policy(state_ro.world())
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsModerationPolicy))
    }
}

impl ValidSingularQuery for FindSorafsModerationAppeal {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationAppealRecordV1, QueryExecutionFail> {
        read_appeal(state_ro.world(), &self.case_id, &self.round_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationAppeal(format!(
                    "{} round {}",
                    self.case_id, self.round_id
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationJurorEligibility {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationJurorEligibilityRecordV1, QueryExecutionFail> {
        read_eligibility(state_ro.world(), &self.case_id, &self.round_id, &self.juror)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationJurorEligibility(format!(
                    "{} round {} juror {}",
                    self.case_id, self.round_id, self.juror
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationCase {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationCaseRecordV1, QueryExecutionFail> {
        read_case(state_ro.world(), &self.case_id, &self.round_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationCase(format!(
                    "{} round {}",
                    self.case_id, self.round_id
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationCommit {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationCommitRecordV1, QueryExecutionFail> {
        read_commit(state_ro.world(), &self.case_id, &self.round_id, &self.juror)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationCommit(format!(
                    "{} round {} juror {}",
                    self.case_id, self.round_id, self.juror
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationReveal {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationRevealRecordV1, QueryExecutionFail> {
        read_reveal(state_ro.world(), &self.case_id, &self.round_id, &self.juror)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationReveal(format!(
                    "{} round {} juror {}",
                    self.case_id, self.round_id, self.juror
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationChallenge {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationChallengeRecordV1, QueryExecutionFail> {
        read_challenge(
            state_ro.world(),
            &self.case_id,
            &self.round_id,
            &self.challenge_id,
        )
        .map_err(query_failure)?
        .ok_or_else(|| {
            QueryExecutionFail::Find(FindError::SorafsModerationChallenge(format!(
                "{} for {} round {}",
                self.challenge_id, self.case_id, self.round_id
            )))
        })
    }
}

impl ValidSingularQuery for FindSorafsModerationOutcome {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationOutcomeRecordV1, QueryExecutionFail> {
        read_outcome(state_ro.world(), &self.case_id, &self.round_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationOutcome(format!(
                    "{} round {}",
                    self.case_id, self.round_id
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationNoShow {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationNoShowRecordV1, QueryExecutionFail> {
        read_no_show(state_ro.world(), &self.case_id, &self.round_id, &self.juror)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsModerationNoShow(format!(
                    "{} round {} juror {}",
                    self.case_id, self.round_id, self.juror
                )))
            })
    }
}

impl ValidSingularQuery for FindSorafsModerationStatus {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ModerationLedgerStatusV1, QueryExecutionFail> {
        let policy = read_policy(state_ro.world()).map_err(query_failure)?;
        let status = read_status(state_ro.world()).map_err(query_failure)?;
        match (policy, status) {
            (Some(_), Some(status)) => Ok(status),
            (None, None) => Err(QueryExecutionFail::Find(FindError::SorafsModerationStatus)),
            _ => Err(QueryExecutionFail::Conversion(
                "authoritative SoraFS moderation policy/status state is inconsistent".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::too_many_lines)]

    use core::num::NonZeroU64;
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        account::{Account, AccountId},
        block::BlockHeader,
        isi::sorafs::{
            CommitSorafsPopCredentialBatch, PublishSorafsPopRevocationList,
            SetSorafsPopIssuerPolicy,
        },
        permission::{Permission, Permissions},
        sorafs::{
            moderation::{
                SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
                SoraFsModerationBallotContextV1, SoraFsModerationBallotRevealV1,
            },
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_CASE_VERSION_V1,
                MODERATION_LEDGER_POLICY_VERSION_V1, ModerationAppealIntakeV1,
                ModerationCaseSpecV1, ModerationChallengeDecisionV1,
                ModerationChallengeKindV1, ModerationLedgerPolicyV1, ModerationNoShowKindV1,
                ModerationOutcomeKindV1, sorafs_moderation_panel_roster_hash_v1,
            },
            pop_registry::{
                POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1, POP_ISSUER_POLICY_VERSION_V1,
                PopCredentialCommitmentBatchV1, PopCredentialCommitmentV1, PopIssuerPolicyV1,
                pop_credential_payload_commitment_v1, pop_revocation_nonce_commitment_v1,
            },
        },
    };
    use iroha_primitives::json::Json;
    use sorafs_manifest::pop_credentials::{
        POP_COMMITMENT_ROOT_VERSION_V1, POP_CREDENTIAL_TREE_DEPTH_V1,
        POP_CREDENTIAL_VERSION_V1, POP_REVOCATION_LIST_VERSION_V1,
        POP_REVOCATION_TREE_DEPTH_V1, PopCommitmentRootV1, PopCredentialAttributeV1,
        PopCredentialMerklePathV1, PopCredentialV1, PopMembershipProofV1,
        PopMembershipWitnessV1, PopRevocationEntryV1, PopRevocationListV1,
        PopRevocationNonMembershipPathV1, PopRevocationReasonV1, PopSignatureAlgorithmV1,
        PopSignatureV1,
        build_pop_revocation_non_membership_path_v1, derive_pop_holder_commitment_v1,
        pop_commitment_root_signature_digest_v1, pop_credential_leaf_v1,
        pop_credential_root_from_path_v1, pop_credential_signature_digest_v1,
        pop_revocation_list_signature_digest_v1, pop_revocation_root_v1,
        prove_pop_membership_v1, verify_pop_commitment_root_signature_v1,
        verify_pop_credential_signature_v1, verify_pop_revocation_list_signature_v1,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const OPENED_AT: u64 = 1_000;
    const COMMIT_DEADLINE: u64 = 2_000;
    const CHALLENGE_DEADLINE: u64 = 3_000;
    const REVEAL_DEADLINE: u64 = 4_000;

    fn keypair(seed: u8) -> KeyPair {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("valid deterministic Ed25519 seed");
        KeyPair::from_private_key(private).expect("derive deterministic keypair")
    }

    fn account(keypair: &KeyPair) -> AccountId {
        AccountId::new(keypair.public_key().clone())
    }

    fn policy() -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            max_panel_size: 8,
            max_candidate_pool_size: 32,
            max_waitlist_size: 8,
            max_exclusions_per_case: 16,
            max_total_window_ms: 10_000,
            max_challenges_per_case: 2,
            missing_commit_penalty_points: 11,
            unrevealed_commit_penalty_points: 23,
        }
    }

    fn context(jurors: &[AccountId], quorum: u16) -> SoraFsModerationBallotContextV1 {
        SoraFsModerationBallotContextV1 {
            version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            case_id: "case-1".to_owned(),
            evidence_bundle_digest: [0x41; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            panel_roster_hash: sorafs_moderation_panel_roster_hash_v1(jurors, quorum),
            policy_reference: "policy-v1".to_owned(),
            evidence_uri: Some("ipfs://evidence".to_owned()),
        }
    }

    fn spec(jurors: Vec<AccountId>, quorum: u16) -> ModerationCaseSpecV1 {
        ModerationCaseSpecV1 {
            version: MODERATION_LEDGER_CASE_VERSION_V1,
            context: context(&jurors, quorum),
            round_id: "round-1".to_owned(),
            jurors,
            quorum,
            commit_deadline_unix_ms: COMMIT_DEADLINE,
            challenge_deadline_unix_ms: CHALLENGE_DEADLINE,
            reveal_deadline_unix_ms: REVEAL_DEADLINE,
            policy_digest: policy().digest().expect("policy digest"),
        }
    }

    fn reveal(
        spec: &ModerationCaseSpecV1,
        juror: &AccountId,
        choice: SoraFsModerationVoteChoice,
        nonce_byte: u8,
    ) -> SoraFsModerationBallotRevealV1 {
        SoraFsModerationBallotRevealV1 {
            version: SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1,
            context: spec.context.clone(),
            round_id: spec.round_id.clone(),
            juror_id: juror.to_string(),
            choice,
            nonce: vec![nonce_byte; 32],
            revealed_at_unix_ms: 0,
        }
    }

    fn commit(reveal: &SoraFsModerationBallotRevealV1) -> SoraFsModerationBallotCommitV1 {
        SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms: 0,
        }
    }

    fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::to_bytes(value).expect("encode fixture")
    }

    fn state(accounts: &[&KeyPair], manager: &AccountId) -> State {
        let mut world = World::new();
        for keypair in accounts {
            let id = account(keypair);
            let (id, value) = Account::new(id.clone()).build(&id).into_key_value();
            world.accounts.insert(id, value);
        }
        let mut permissions = Permissions::new();
        for permission in [
            MANAGE_PERMISSION,
            "CanManageSorafsPopRegistry",
            "CanOperateSorafsPopIssuer",
        ] {
            permissions.insert(Permission::new(permission.to_owned(), Json::new(())));
        }
        world
            .account_permissions
            .insert(manager.clone(), permissions);
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn header(height: u64, now: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            now,
            0,
        )
    }

    fn transact(
        state: &mut State,
        height: u64,
        now: u64,
        operation: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<(), InstructionExecutionError>,
    ) -> Result<(), InstructionExecutionError> {
        let mut block = state.block(header(height, now));
        let mut transaction = block.transaction();
        operation(&mut transaction)?;
        transaction.apply();
        block.commit().expect("commit test block");
        Ok(())
    }

    fn scalar(value: u64) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    fn pop_nonce(value: u128) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[..16].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    fn public_key_bytes(keypair: &KeyPair) -> [u8; 32] {
        let (_, bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key bytes");
        bytes.try_into().expect("Ed25519 public key length")
    }

    fn empty_pop_signature(keypair: &KeyPair) -> PopSignatureV1 {
        PopSignatureV1 {
            algorithm: PopSignatureAlgorithmV1::Ed25519,
            public_key: public_key_bytes(keypair).to_vec(),
            signature: Vec::new(),
        }
    }

    fn sign_pop_digest(keypair: &KeyPair, digest: [u8; 32]) -> Vec<u8> {
        Signature::try_new(keypair.private_key(), &digest)
            .expect("sign PoP fixture digest")
            .payload()
            .to_vec()
    }

    fn sign_pop_credential(
        mut credential: PopCredentialV1,
        keypair: &KeyPair,
    ) -> PopCredentialV1 {
        credential.issuer_signature = empty_pop_signature(keypair);
        let digest = pop_credential_signature_digest_v1(&credential)
            .expect("credential signature digest");
        credential.issuer_signature.signature = sign_pop_digest(keypair, digest);
        verify_pop_credential_signature_v1(&credential).expect("credential signature verifies");
        credential
    }

    fn sign_pop_root(
        mut root: PopCommitmentRootV1,
        keypair: &KeyPair,
    ) -> PopCommitmentRootV1 {
        root.publisher_signature = empty_pop_signature(keypair);
        let digest =
            pop_commitment_root_signature_digest_v1(&root).expect("root signature digest");
        root.publisher_signature.signature = sign_pop_digest(keypair, digest);
        verify_pop_commitment_root_signature_v1(&root).expect("root signature verifies");
        root
    }

    fn sign_pop_revocations(
        mut revocations: PopRevocationListV1,
        keypair: &KeyPair,
    ) -> PopRevocationListV1 {
        revocations.publisher_signature = empty_pop_signature(keypair);
        let digest = pop_revocation_list_signature_digest_v1(&revocations)
            .expect("revocation signature digest");
        revocations.publisher_signature.signature = sign_pop_digest(keypair, digest);
        verify_pop_revocation_list_signature_v1(&revocations)
            .expect("revocation signature verifies");
        revocations
    }

    struct PopMaterial {
        credential: PopCredentialV1,
        root: PopCommitmentRootV1,
        revocations: PopRevocationListV1,
        holder_secret: [u8; 32],
        credential_path: PopCredentialMerklePathV1,
        revocation_path: PopRevocationNonMembershipPathV1,
    }

    impl PopMaterial {
        fn proof(
            &self,
            challenge: [u8; 32],
            verifier_context: &str,
            now_epoch: u64,
        ) -> PopMembershipProofV1 {
            let witness = PopMembershipWitnessV1 {
                holder_secret: self.holder_secret,
                credential_path: self.credential_path.clone(),
                revocation_path: self.revocation_path.clone(),
            };
            prove_pop_membership_v1(
                &self.credential,
                &self.root,
                &self.revocations,
                &witness,
                challenge,
                verifier_context,
                now_epoch,
            )
            .expect("create moderation PoP proof")
        }
    }

    fn pop_material(issuer: &KeyPair) -> PopMaterial {
        let holder_secret = scalar(0x1234_5678);
        let credential_id = scalar(0x8765_4321);
        let holder_commitment = derive_pop_holder_commitment_v1(holder_secret, credential_id)
            .expect("holder commitment");
        let nonce = pop_nonce(0xfeed_beef_dead_cafe_1234_5678_9abc_def0);
        let mut credential = PopCredentialV1 {
            version: POP_CREDENTIAL_VERSION_V1,
            credential_id,
            holder_commitment,
            eligibility_class: PopEligibilityClassV1::General,
            attributes: vec![PopCredentialAttributeV1 {
                key: "residency".to_owned(),
                value_commitment: [0x13; 32],
            }],
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issued_at_epoch: 900,
            expires_at_epoch: 2_000,
            renewal_at_epoch: 1_800,
            revocation_nonce: nonce,
            commitment_root: scalar(1),
            commitment_tree_version: 1,
            revocation_list_version: 1,
            issuer_signature: empty_pop_signature(issuer),
        };
        credential = sign_pop_credential(credential, issuer);
        let credential_path = PopCredentialMerklePathV1 {
            siblings: vec![scalar(0); usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
            directions: (0..usize::from(POP_CREDENTIAL_TREE_DEPTH_V1))
                .map(|level| level % 3 == 1)
                .collect(),
        };
        let leaf = pop_credential_leaf_v1(&credential).expect("credential leaf");
        let root_digest = pop_credential_root_from_path_v1(leaf, &credential_path)
            .expect("credential root");
        credential.commitment_root = root_digest;
        credential = sign_pop_credential(credential, issuer);
        let root = sign_pop_root(
            PopCommitmentRootV1 {
                version: POP_COMMITMENT_ROOT_VERSION_V1,
                root_digest,
                tree_size: 1,
                tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
                tree_version: 1,
                issuer_id: "pop-issuer-sora-foundation".to_owned(),
                published_at_epoch: 999,
                previous_root_digest: None,
                governance_event_digest: [0x17; 32],
                publisher_signature: empty_pop_signature(issuer),
            },
            issuer,
        );
        let entries = Vec::new();
        let revocation_root = pop_revocation_root_v1(&entries).expect("empty revocation root");
        let revocations = sign_pop_revocations(
            PopRevocationListV1 {
                version: POP_REVOCATION_LIST_VERSION_V1,
                list_version: 1,
                commitment_root: root_digest,
                revocation_root,
                revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
                issuer_id: "pop-issuer-sora-foundation".to_owned(),
                published_at_epoch: 999,
                entries,
                publisher_signature: empty_pop_signature(issuer),
            },
            issuer,
        );
        let revocation_path = build_pop_revocation_non_membership_path_v1(
            &revocations.entries,
            credential.revocation_nonce,
        )
        .expect("revocation non-membership path");
        PopMaterial {
            credential,
            root,
            revocations,
            holder_secret,
            credential_path,
            revocation_path,
        }
    }

    fn shared_pop_material() -> &'static PopMaterial {
        static MATERIAL: std::sync::OnceLock<PopMaterial> = std::sync::OnceLock::new();
        MATERIAL.get_or_init(|| pop_material(&keypair(0x51)))
    }

    fn proof_for_appeal(appeal: &ModerationAppealRecordV1) -> PopMembershipProofV1 {
        static PROOF: std::sync::OnceLock<PopMembershipProofV1> = std::sync::OnceLock::new();
        let challenge =
            sorafs_moderation_pop_challenge_v1(appeal.intake_digest, appeal.pop_snapshot_digest);
        let context = sorafs_moderation_pop_verifier_context_v1(appeal.intake_digest);
        let proof = PROOF.get_or_init(|| {
            shared_pop_material().proof(challenge, &context, appeal.submitted_at_unix_ms / 1_000)
        });
        assert_eq!(proof.challenge_digest, challenge);
        assert_eq!(proof.verifier_context, context);
        proof.clone()
    }

    fn pop_policy(issuer: &KeyPair) -> PopIssuerPolicyV1 {
        PopIssuerPolicyV1 {
            version: POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_account: account(issuer),
            issuer_public_key: public_key_bytes(issuer),
            max_credentials_per_batch: 16,
            max_revocations_per_publication: 16,
            max_credential_lifetime_secs: 10_000,
            max_future_clock_skew_secs: 5,
            paused: false,
        }
    }

    fn pop_batch(issuer: &KeyPair, material: &PopMaterial) -> PopCredentialCommitmentBatchV1 {
        let canonical_credential = encode(&material.credential);
        PopCredentialCommitmentBatchV1 {
            version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
            issuer_policy_digest: pop_policy(issuer).digest().expect("PoP policy digest"),
            commitment_root_payload: encode(&material.root),
            revocation_list_payload: encode(&material.revocations),
            commitments: vec![PopCredentialCommitmentV1 {
                credential_commitment: pop_credential_payload_commitment_v1(
                    &canonical_credential,
                ),
                revocation_nonce_commitment: pop_revocation_nonce_commitment_v1(
                    material.credential.revocation_nonce,
                ),
                commitment_root: material.root.root_digest,
                commitment_tree_version: material.root.tree_version,
                revocation_list_version: material.revocations.list_version,
                issued_at_epoch: material.credential.issued_at_epoch,
                expires_at_epoch: material.credential.expires_at_epoch,
            }],
        }
    }

    fn setup_panel_foundations(
        state: &mut State,
        manager: &KeyPair,
        material: &PopMaterial,
    ) {
        let manager_id = account(manager);
        transact(state, 1, 1_000_000, |transaction| {
            SetSorafsPopIssuerPolicy::new(pop_policy(manager))
                .execute(&manager_id, transaction)?;
            CommitSorafsPopCredentialBatch::new(encode(&pop_batch(manager, material)))
                .execute(&manager_id, transaction)?;
            SetSorafsModerationPolicy::new(policy()).execute(&manager_id, transaction)
        })
        .expect("activate PoP registry and moderation policy");
        state.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header(1, 1_000_000)));
    }

    fn panel_intake(
        manager: &KeyPair,
        case_id: &str,
        panel_size: u16,
        waitlist_size: u16,
        quorum: u16,
        deposit_byte: u8,
    ) -> ModerationAppealIntakeV1 {
        let manager_id = account(manager);
        ModerationAppealIntakeV1 {
            version: MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: case_id.to_owned(),
            round_id: "round-1".to_owned(),
            appellant: manager_id.clone(),
            appealed_decision_digest: [0x31; 32],
            proof_token_digest: [0x32; 32],
            evidence_bundle_digest: [0x33; 32],
            appeal_deposit_lock_digest: [deposit_byte; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "policy-v1".to_owned(),
            evidence_uri: Some("ipfs://appeal-evidence".to_owned()),
            panel_size,
            waitlist_size,
            quorum,
            exclusions: vec![manager_id],
            registration_deadline_unix_ms: 1_003_000,
            acceptance_deadline_unix_ms: 1_005_000,
            commit_deadline_unix_ms: 1_007_000,
            challenge_deadline_unix_ms: 1_009_000,
            reveal_deadline_unix_ms: 1_011_000,
            policy_digest: policy().digest().expect("moderation policy digest"),
        }
    }

    struct PanelFixture {
        manager: KeyPair,
        juror: KeyPair,
        outsider: KeyPair,
        state: State,
        next_height: u64,
    }

    impl PanelFixture {
        fn new() -> Self {
            let manager = keypair(0x51);
            let juror = keypair(0x61);
            let outsider = keypair(0x71);
            let manager_id = account(&manager);
            let mut state = state(
                &[&manager, &juror, &outsider],
                &manager_id,
            );
            setup_panel_foundations(&mut state, &manager, shared_pop_material());
            Self {
                manager,
                juror,
                outsider,
                state,
                next_height: 2,
            }
        }

        fn manager_id(&self) -> AccountId {
            account(&self.manager)
        }

        fn juror_id(&self) -> AccountId {
            account(&self.juror)
        }

        fn outsider_id(&self) -> AccountId {
            account(&self.outsider)
        }

        fn run(
            &mut self,
            now: u64,
            operation: impl FnOnce(
                &mut StateTransaction<'_, '_>,
            ) -> Result<(), InstructionExecutionError>,
        ) -> Result<(), InstructionExecutionError> {
            let result = transact(&mut self.state, self.next_height, now, operation);
            if result.is_ok() {
                self.next_height += 1;
            }
            result
        }

        fn submit(&mut self, panel_size: u16, waitlist_size: u16, quorum: u16) {
            let intake = panel_intake(
                &self.manager,
                "panel-case",
                panel_size,
                waitlist_size,
                quorum,
                0x91,
            );
            let manager = self.manager_id();
            self.run(1_001_000, |transaction| {
                SubmitSorafsModerationAppeal::new(intake).execute(&manager, transaction)
            })
            .expect("submit panel appeal");
        }

        fn appeal(&self) -> ModerationAppealRecordV1 {
            FindSorafsModerationAppeal::new("panel-case".to_owned(), "round-1".to_owned())
                .execute(&self.state.view())
                .expect("panel appeal query")
        }

        fn register_juror(&mut self) {
            let proof = proof_for_appeal(&self.appeal());
            let juror = self.juror_id();
            self.run(1_002_000, |transaction| {
                RegisterSorafsModerationJurorEligibility::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    encode(&proof),
                )
                .execute(&juror, transaction)
            })
            .expect("register panel juror eligibility");
        }

        fn finalize_single_juror_sortition(&mut self) -> [u8; 32] {
            let manager = self.manager_id();
            let juror = self.juror_id();
            let snapshot_digest = self.appeal().pop_snapshot_digest;
            self.run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    vec![juror],
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .expect("finalize deterministic panel");
            self.appeal()
                .selection
                .expect("selected panel")
                .sortition_digest
        }
    }

    fn seed_activated_case(
        transaction: &mut StateTransaction<'_, '_>,
        manager: &AccountId,
        spec: ModerationCaseSpecV1,
    ) -> Result<(), InstructionExecutionError> {
        let mut eligible_jurors = spec.jurors.clone();
        eligible_jurors.sort_by_key(ToString::to_string);
        let pop_snapshot = ModerationPoPRegistrySnapshotV1 {
            issuer_policy_digest: [0x81; 32],
            commitment_root: [0x82; 32],
            commitment_tree_version: 1,
            revocation_root: [0x83; 32],
            revocation_list_version: 1,
            registry_audit_sequence: 1,
            registry_audit_head: [0x84; 32],
            captured_at_unix_ms: 700,
            randomness_anchor: [0x85; 32],
        };
        let intake = iroha_data_model::sorafs::moderation_ledger::ModerationAppealIntakeV1 {
            version: iroha_data_model::sorafs::moderation_ledger::MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: spec.context.case_id.clone(),
            round_id: spec.round_id.clone(),
            appellant: manager.clone(),
            appealed_decision_digest: [0x31; 32],
            proof_token_digest: [0x32; 32],
            evidence_bundle_digest: spec.context.evidence_bundle_digest,
            appeal_deposit_lock_digest: [0x33; 32],
            appeal_finance_config_version: spec.context.appeal_finance_config_version.clone(),
            policy_reference: spec.context.policy_reference.clone(),
            evidence_uri: spec.context.evidence_uri.clone(),
            panel_size: spec.jurors.len() as u16,
            waitlist_size: 0,
            quorum: spec.quorum,
            exclusions: vec![manager.clone()],
            registration_deadline_unix_ms: 800,
            acceptance_deadline_unix_ms: 900,
            commit_deadline_unix_ms: spec.commit_deadline_unix_ms,
            challenge_deadline_unix_ms: spec.challenge_deadline_unix_ms,
            reveal_deadline_unix_ms: spec.reveal_deadline_unix_ms,
            policy_digest: spec.policy_digest,
        };
        intake
            .validate()
            .map_err(|error| corrupt_state(format!("fixture appeal invalid: {error}")))?;
        let intake_digest = intake
            .digest()
            .map_err(|error| corrupt_state(format!("fixture appeal digest: {error}")))?;
        let pop_snapshot_digest = pop_snapshot
            .digest()
            .map_err(|error| corrupt_state(format!("fixture snapshot digest: {error}")))?;
        let seed_digest = [0x91; 32];
        let sortition_digest = sorafs_moderation_sortition_digest_v1(
            pop_snapshot_digest,
            seed_digest,
            &spec.jurors,
            &[],
            spec.quorum,
        );
        let appeal = ModerationAppealRecordV1 {
            intake,
            intake_digest,
            policy: policy(),
            pop_snapshot,
            pop_snapshot_digest,
            status: ModerationAppealStatusV1::BallotOpen,
            submitted_by: manager.clone(),
            submitted_at_unix_ms: 700,
            eligible_jurors: eligible_jurors.clone(),
            selection: Some(ModerationPanelSelectionV1 {
                seed_digest,
                jurors: spec.jurors.clone(),
                waitlist: Vec::new(),
                sortition_digest,
                selected_at_unix_ms: 850,
                selected_by: manager.clone(),
            }),
            accepted_jurors: eligible_jurors,
            replacements: Vec::new(),
            activated_at_unix_ms: Some(OPENED_AT),
            finalized_at_unix_ms: None,
        };
        let case = ModerationCaseRecordV1 {
            spec,
            policy: policy(),
            status: ModerationCaseStatusV1::Open,
            opened_at_unix_ms: OPENED_AT,
            opened_by: manager.clone(),
            commitment_count: 0,
            reveal_count: 0,
            challenge_count: 0,
            pending_challenge_count: 0,
            accepted_challenge_count: 0,
        };
        let mut status = status_for_mutation(transaction.world(), OPENED_AT)?;
        status.appeal_intakes = 1;
        status.eligibility_proofs = case.spec.jurors.len() as u64;
        status.panel_selections = 1;
        status.assignment_acceptances = case.spec.jurors.len() as u64;
        status.open_cases = 1;
        transaction.world.smart_contract_state.insert(
            appeal_key(&case.spec.context.case_id, &case.spec.round_id),
            encode_state(&appeal, "fixture moderation appeal")?,
        );
        transaction.world.smart_contract_state.insert(
            case_key(&case.spec.context.case_id, &case.spec.round_id),
            encode_state(&case, "fixture moderation case")?,
        );
        transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encode_status(&status)?);
        Ok(())
    }

    struct Fixture {
        manager: KeyPair,
        jurors: [KeyPair; 3],
        outsider: KeyPair,
        state: State,
        spec: ModerationCaseSpecV1,
        next_height: u64,
    }

    impl Fixture {
        fn new(quorum: u16) -> Self {
            let manager = keypair(0x11);
            let jurors = [keypair(0x21), keypair(0x22), keypair(0x23)];
            let outsider = keypair(0x31);
            let manager_id = account(&manager);
            let juror_ids = jurors.iter().map(account).collect::<Vec<_>>();
            let spec = spec(juror_ids, quorum);
            let mut state = state(
                &[&manager, &jurors[0], &jurors[1], &jurors[2], &outsider],
                &manager_id,
            );
            transact(&mut state, 1, OPENED_AT, |transaction| {
                SetSorafsModerationPolicy::new(policy()).execute(&manager_id, transaction)?;
                seed_activated_case(transaction, &manager_id, spec.clone())
            })
            .expect("activate policy and open case");
            Self {
                manager,
                jurors,
                outsider,
                state,
                spec,
                next_height: 2,
            }
        }

        fn manager_id(&self) -> AccountId {
            account(&self.manager)
        }

        fn juror_id(&self, index: usize) -> AccountId {
            account(&self.jurors[index])
        }

        fn run(
            &mut self,
            now: u64,
            operation: impl FnOnce(
                &mut StateTransaction<'_, '_>,
            ) -> Result<(), InstructionExecutionError>,
        ) -> Result<(), InstructionExecutionError> {
            let height = self.next_height;
            let result = transact(&mut self.state, height, now, operation);
            if result.is_ok() {
                self.next_height += 1;
            }
            result
        }
    }

    #[test]
    fn successful_commit_reveal_finalization_persists_queries_and_no_show() {
        let mut fixture = Fixture::new(2);
        let juror0 = fixture.juror_id(0);
        let juror1 = fixture.juror_id(1);
        let juror2 = fixture.juror_id(2);
        let reveal0 = reveal(
            &fixture.spec,
            &juror0,
            SoraFsModerationVoteChoice::Uphold,
            1,
        );
        let reveal1 = reveal(
            &fixture.spec,
            &juror1,
            SoraFsModerationVoteChoice::Uphold,
            2,
        );
        let commit0 = commit(&reveal0);
        let commit1 = commit(&reveal1);
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&commit0))
                    .execute(&juror0, transaction)?;
                SubmitSorafsModerationCommit::new(encode(&commit1)).execute(&juror1, transaction)
            })
            .unwrap();
        fixture
            .run(3_500, |transaction| {
                SubmitSorafsModerationReveal::new(encode(&reveal0))
                    .execute(&juror0, transaction)?;
                SubmitSorafsModerationReveal::new(encode(&reveal1)).execute(&juror1, transaction)
            })
            .unwrap();
        let manager = fixture.manager_id();
        fixture
            .run(4_001, |transaction| {
                FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                    .execute(&manager, transaction)
            })
            .unwrap();

        let case = FindSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(case.status, ModerationCaseStatusV1::Finalized);
        assert_eq!(case.commitment_count, 2);
        assert_eq!(case.reveal_count, 2);
        let outcome = FindSorafsModerationOutcome::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(
            outcome.kind,
            ModerationOutcomeKindV1::Decided(SoraFsModerationVoteChoice::Uphold)
        );
        assert_eq!(outcome.votes_total, 2);
        assert_eq!(outcome.no_show_count, 1);
        let no_show =
            FindSorafsModerationNoShow::new("case-1".to_owned(), "round-1".to_owned(), juror2)
                .execute(&fixture.state.view())
                .unwrap();
        assert_eq!(no_show.kind, ModerationNoShowKindV1::MissingCommit);
        assert_eq!(no_show.penalty_points, 11);
        assert!(
            FindSorafsModerationNoShow::new("case-1".to_owned(), "round-1".to_owned(), juror0,)
                .execute(&fixture.state.view())
                .is_err()
        );
        let status = FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(status.open_cases, 0);
        assert_eq!(status.finalized_cases, 1);
        assert_eq!(status.commitments, 2);
        assert_eq!(status.reveals, 2);
        assert_eq!(status.outcomes, 1);
        assert_eq!(status.no_shows, 1);
    }

    #[test]
    fn duplicate_wrong_authority_phase_and_mismatched_reveal_are_atomic() {
        let mut fixture = Fixture::new(1);
        let juror = fixture.juror_id(0);
        let outsider = account(&fixture.outsider);
        let juror_reveal = reveal(
            &fixture.spec,
            &juror,
            SoraFsModerationVoteChoice::Overturn,
            3,
        );
        let juror_commit = commit(&juror_reveal);
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&juror_commit))
                    .execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(1_501, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&juror_commit))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_501, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&juror_commit))
                        .execute(&outsider, transaction)
                })
                .is_err()
        );
        let other = fixture.juror_id(1);
        let other_reveal = reveal(&fixture.spec, &other, SoraFsModerationVoteChoice::Uphold, 4);
        let other_commit = commit(&other_reveal);
        assert!(
            fixture
                .run(2_001, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&other_commit))
                        .execute(&other, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(2_500, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&juror_reveal))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        let mut mismatched = juror_reveal.clone();
        mismatched.choice = SoraFsModerationVoteChoice::Modify;
        assert!(
            fixture
                .run(3_500, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&mismatched))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        fixture
            .run(3_500, |transaction| {
                SubmitSorafsModerationReveal::new(encode(&juror_reveal))
                    .execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(3_501, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&juror_reveal))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        let case = FindSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(case.commitment_count, 1);
        assert_eq!(case.reveal_count, 1);
        let status = FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(status.commitments, 1);
        assert_eq!(status.reveals, 1);
    }

    #[test]
    fn pending_and_accepted_challenges_block_reveal_and_close_without_penalties() {
        let mut fixture = Fixture::new(1);
        let juror = fixture.juror_id(0);
        let challenger = account(&fixture.outsider);
        let reveal = reveal(&fixture.spec, &juror, SoraFsModerationVoteChoice::Uphold, 5);
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&commit(&reveal)))
                    .execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(1_600, |transaction| {
                    RaiseSorafsModerationChallenge::new(
                        "case-1".to_owned(),
                        "round-1".to_owned(),
                        "challenge-1".to_owned(),
                        ModerationChallengeKindV1::EvidenceMismatch,
                        None,
                        [0x51; 32],
                        "wrong evidence".to_owned(),
                    )
                    .execute(&challenger, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(2_500, |transaction| {
                    RaiseSorafsModerationChallenge::new(
                        "case-1".to_owned(),
                        "round-1".to_owned(),
                        "missing-target".to_owned(),
                        ModerationChallengeKindV1::DuplicateCommit,
                        None,
                        [0x50; 32],
                        "target required".to_owned(),
                    )
                    .execute(&challenger, transaction)
                })
                .is_err()
        );
        fixture
            .run(2_500, |transaction| {
                RaiseSorafsModerationChallenge::new(
                    "case-1".to_owned(),
                    "round-1".to_owned(),
                    "challenge-1".to_owned(),
                    ModerationChallengeKindV1::EvidenceMismatch,
                    None,
                    [0x51; 32],
                    "wrong evidence".to_owned(),
                )
                .execute(&challenger, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(2_501, |transaction| {
                    RaiseSorafsModerationChallenge::new(
                        "case-1".to_owned(),
                        "round-1".to_owned(),
                        "challenge-1".to_owned(),
                        ModerationChallengeKindV1::EvidenceMismatch,
                        None,
                        [0x52; 32],
                        "duplicate".to_owned(),
                    )
                    .execute(&challenger, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(3_500, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&reveal)).execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(3_500, |transaction| {
                    ResolveSorafsModerationChallenge::new(
                        "case-1".to_owned(),
                        "round-1".to_owned(),
                        "challenge-1".to_owned(),
                        ModerationChallengeDecisionV1::Accepted,
                    )
                    .execute(&challenger, transaction)
                })
                .is_err()
        );
        let manager = fixture.manager_id();
        fixture
            .run(3_500, |transaction| {
                ResolveSorafsModerationChallenge::new(
                    "case-1".to_owned(),
                    "round-1".to_owned(),
                    "challenge-1".to_owned(),
                    ModerationChallengeDecisionV1::Accepted,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(3_501, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&reveal)).execute(&juror, transaction)
                })
                .is_err()
        );
        fixture
            .run(4_001, |transaction| {
                FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                    .execute(&manager, transaction)
            })
            .unwrap();
        let outcome = FindSorafsModerationOutcome::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(outcome.kind, ModerationOutcomeKindV1::Challenged);
        assert_eq!(outcome.no_show_count, 0);
        assert_eq!(
            FindSorafsModerationStatus
                .execute(&fixture.state.view())
                .unwrap()
                .no_shows,
            0
        );
    }

    #[test]
    fn rejected_challenge_unblocks_reveals_and_tied_quorum_is_contested() {
        let mut fixture = Fixture::new(2);
        let juror0 = fixture.juror_id(0);
        let juror1 = fixture.juror_id(1);
        let challenger = account(&fixture.outsider);
        let reveal0 = reveal(
            &fixture.spec,
            &juror0,
            SoraFsModerationVoteChoice::Uphold,
            10,
        );
        let reveal1 = reveal(
            &fixture.spec,
            &juror1,
            SoraFsModerationVoteChoice::Overturn,
            11,
        );
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&commit(&reveal0)))
                    .execute(&juror0, transaction)?;
                SubmitSorafsModerationCommit::new(encode(&commit(&reveal1)))
                    .execute(&juror1, transaction)
            })
            .unwrap();
        fixture
            .run(2_500, |transaction| {
                RaiseSorafsModerationChallenge::new(
                    "case-1".to_owned(),
                    "round-1".to_owned(),
                    "challenge-rejected".to_owned(),
                    ModerationChallengeKindV1::PayloadMismatch,
                    Some(juror0.clone()),
                    [0x71; 32],
                    "payload reviewed".to_owned(),
                )
                .execute(&challenger, transaction)
            })
            .unwrap();
        let manager = fixture.manager_id();
        fixture
            .run(2_600, |transaction| {
                ResolveSorafsModerationChallenge::new(
                    "case-1".to_owned(),
                    "round-1".to_owned(),
                    "challenge-rejected".to_owned(),
                    ModerationChallengeDecisionV1::Rejected,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        fixture
            .run(3_500, |transaction| {
                SubmitSorafsModerationReveal::new(encode(&reveal0))
                    .execute(&juror0, transaction)?;
                SubmitSorafsModerationReveal::new(encode(&reveal1)).execute(&juror1, transaction)
            })
            .unwrap();
        fixture
            .run(4_001, |transaction| {
                FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                    .execute(&manager, transaction)
            })
            .unwrap();

        let challenge = FindSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-rejected".to_owned(),
        )
        .execute(&fixture.state.view())
        .unwrap();
        assert_eq!(
            challenge.decision,
            Some(ModerationChallengeDecisionV1::Rejected)
        );
        let outcome = FindSorafsModerationOutcome::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(outcome.kind, ModerationOutcomeKindV1::Contested);
        assert_eq!(outcome.votes_total, 2);
    }

    #[test]
    fn missed_quorum_persists_distinct_no_show_penalties() {
        let mut fixture = Fixture::new(3);
        let juror0 = fixture.juror_id(0);
        let juror1 = fixture.juror_id(1);
        let juror2 = fixture.juror_id(2);
        let reveal0 = reveal(
            &fixture.spec,
            &juror0,
            SoraFsModerationVoteChoice::Modify,
            6,
        );
        let reveal1 = reveal(
            &fixture.spec,
            &juror1,
            SoraFsModerationVoteChoice::Modify,
            7,
        );
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&commit(&reveal0)))
                    .execute(&juror0, transaction)?;
                SubmitSorafsModerationCommit::new(encode(&commit(&reveal1)))
                    .execute(&juror1, transaction)
            })
            .unwrap();
        fixture
            .run(3_500, |transaction| {
                SubmitSorafsModerationReveal::new(encode(&reveal0)).execute(&juror0, transaction)
            })
            .unwrap();
        let manager = fixture.manager_id();
        assert!(
            fixture
                .run(REVEAL_DEADLINE, |transaction| {
                    FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                        .execute(&manager, transaction)
                })
                .is_err()
        );
        let outsider = account(&fixture.outsider);
        assert!(
            fixture
                .run(REVEAL_DEADLINE + 1, |transaction| {
                    FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                        .execute(&outsider, transaction)
                })
                .is_err()
        );
        fixture
            .run(4_001, |transaction| {
                FinalizeSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
                    .execute(&manager, transaction)
            })
            .unwrap();
        let outcome = FindSorafsModerationOutcome::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(outcome.kind, ModerationOutcomeKindV1::QuorumNotMet);
        assert_eq!(outcome.votes_total, 1);
        assert_eq!(outcome.no_show_count, 2);
        let unrevealed =
            FindSorafsModerationNoShow::new("case-1".to_owned(), "round-1".to_owned(), juror1)
                .execute(&fixture.state.view())
                .unwrap();
        assert_eq!(unrevealed.kind, ModerationNoShowKindV1::UnrevealedCommit);
        assert_eq!(unrevealed.penalty_points, 23);
        let missing =
            FindSorafsModerationNoShow::new("case-1".to_owned(), "round-1".to_owned(), juror2)
                .execute(&fixture.state.view())
                .unwrap();
        assert_eq!(missing.kind, ModerationNoShowKindV1::MissingCommit);
        assert_eq!(missing.penalty_points, 11);
    }

    #[test]
    fn bounds_permissions_and_counter_overflow_reject_without_partial_case() {
        let manager_pair = keypair(0x41);
        let outsider_pair = keypair(0x42);
        let manager = account(&manager_pair);
        let outsider = account(&outsider_pair);
        let mut state = state(&[&manager_pair, &outsider_pair], &manager);
        assert!(
            transact(&mut state, 1, OPENED_AT, |transaction| {
                SetSorafsModerationPolicy::new(policy()).execute(&outsider, transaction)
            })
            .is_err()
        );
        transact(&mut state, 1, OPENED_AT, |transaction| {
            SetSorafsModerationPolicy::new(policy()).execute(&manager, transaction)
        })
        .unwrap();
        let mut bad_revision = policy();
        bad_revision.revision = 2;
        bad_revision.predecessor_policy_digest = Some([0xFF; 32]);
        assert!(
            transact(&mut state, 2, OPENED_AT + 1, |transaction| {
                SetSorafsModerationPolicy::new(bad_revision).execute(&manager, transaction)
            })
            .is_err()
        );
        assert_eq!(
            FindSorafsModerationPolicy
                .execute(&state.view())
                .unwrap()
                .policy
                .revision,
            1
        );

        let mut fixture = Fixture::new(1);
        let juror = fixture.juror_id(0);
        let rollback_reveal = reveal(&fixture.spec, &juror, SoraFsModerationVoteChoice::Uphold, 8);
        assert!(
            fixture
                .run(OPENED_AT - 1, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&commit(&rollback_reveal)))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_500, |transaction| {
                    SubmitSorafsModerationCommit::new(vec![0xAA; PAYLOAD_MAX_BYTES + 1])
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        let mut oversized_nonce =
            reveal(&fixture.spec, &juror, SoraFsModerationVoteChoice::Uphold, 9);
        oversized_nonce.nonce = vec![9; MODERATION_LEDGER_MAX_NONCE_BYTES_V1 + 1];
        let oversized_commit = commit(&oversized_nonce);
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&oversized_commit))
                    .execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(3_500, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&oversized_nonce))
                        .execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            FindSorafsModerationReveal::new("case-1".to_owned(), "round-1".to_owned(), juror,)
                .execute(&fixture.state.view())
                .is_err()
        );
    }

    #[test]
    fn appeal_intake_is_authority_bound_replay_safe_and_transaction_atomic() {
        let mut fixture = PanelFixture::new();
        let manager = fixture.manager_id();
        let outsider = fixture.outsider_id();
        let mut malformed = panel_intake(&fixture.manager, "panel-case", 1, 0, 1, 0x91);
        malformed.proof_token_digest = [0; 32];
        assert!(
            fixture
                .run(1_001_000, |transaction| {
                    SubmitSorafsModerationAppeal::new(malformed)
                        .execute(&manager, transaction)
                })
                .is_err()
        );
        let intake = panel_intake(&fixture.manager, "panel-case", 1, 0, 1, 0x91);
        assert!(
            fixture
                .run(1_001_000, |transaction| {
                    SubmitSorafsModerationAppeal::new(intake.clone())
                        .execute(&outsider, transaction)
                })
                .is_err()
        );

        assert!(
            fixture
                .run(1_001_000, |transaction| {
                    SubmitSorafsModerationAppeal::new(intake.clone())
                        .execute(&manager, transaction)?;
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        vec![0xAA],
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert!(
            FindSorafsModerationAppeal::new("panel-case".to_owned(), "round-1".to_owned())
                .execute(&fixture.state.view())
                .is_err()
        );
        assert_eq!(
            FindSorafsModerationStatus
                .execute(&fixture.state.view())
                .unwrap()
                .appeal_intakes,
            0
        );

        fixture.submit(1, 0, 1);
        assert!(
            fixture
                .run(1_001_001, |transaction| {
                    SubmitSorafsModerationAppeal::new(intake.clone())
                        .execute(&manager, transaction)
                })
                .is_err()
        );
        let replayed_deposit =
            panel_intake(&fixture.manager, "different-case", 1, 0, 1, 0x91);
        assert!(
            fixture
                .run(1_001_001, |transaction| {
                    SubmitSorafsModerationAppeal::new(replayed_deposit)
                        .execute(&manager, transaction)
                })
                .is_err()
        );
        let replayed_proof_token =
            panel_intake(&fixture.manager, "proof-replay-case", 1, 0, 1, 0x92);
        assert!(
            fixture
                .run(1_001_001, |transaction| {
                    SubmitSorafsModerationAppeal::new(replayed_proof_token)
                        .execute(&manager, transaction)
                })
                .is_err()
        );
        let status = FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .unwrap();
        assert_eq!(status.appeal_intakes, 1);
        assert_eq!(status.eligibility_proofs, 0);

        let mut excluded = PanelFixture::new();
        let excluded_manager = excluded.manager_id();
        let excluded_juror = excluded.juror_id();
        let mut excluded_intake =
            panel_intake(&excluded.manager, "panel-case", 1, 0, 1, 0x91);
        excluded_intake.exclusions.push(excluded_juror.clone());
        excluded_intake.exclusions.sort_by_key(ToString::to_string);
        excluded
            .run(1_001_000, |transaction| {
                SubmitSorafsModerationAppeal::new(excluded_intake)
                    .execute(&excluded_manager, transaction)
            })
            .unwrap();
        assert!(
            excluded
                .run(1_002_000, |transaction| {
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        vec![0xAA],
                    )
                    .execute(&excluded_juror, transaction)
                })
                .is_err()
        );
        assert!(excluded.appeal().eligible_jurors.is_empty());
    }

    #[test]
    fn private_pop_proof_sortition_and_activation_reject_adversarial_inputs() {
        let mut fixture = PanelFixture::new();
        fixture.submit(1, 0, 1);
        let appeal = fixture.appeal();
        let juror = fixture.juror_id();
        let outsider = fixture.outsider_id();
        let mut wrong_root = proof_for_appeal(&appeal);
        wrong_root.commitment_root[0] ^= 1;
        assert!(
            fixture
                .run(1_002_000, |transaction| {
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        encode(&wrong_root),
                    )
                    .execute(&juror, transaction)
                })
                .is_err()
        );
        assert_eq!(fixture.appeal().eligible_jurors.len(), 0);

        fixture.register_juror();
        let proof = proof_for_appeal(&fixture.appeal());
        assert!(
            fixture
                .run(1_002_001, |transaction| {
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        encode(&proof),
                    )
                    .execute(&outsider, transaction)
                })
                .is_err()
        );
        assert!(
            FindSorafsModerationJurorEligibility::new(
                "panel-case".to_owned(),
                "round-1".to_owned(),
                outsider.clone(),
            )
            .execute(&fixture.state.view())
            .is_err()
        );
        let snapshot_digest = fixture.appeal().pop_snapshot_digest;
        let manager = fixture.manager_id();
        assert!(
            fixture
                .run(1_003_000, |transaction| {
                    FinalizeSorafsModerationSortition::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        snapshot_digest,
                        vec![juror.clone()],
                        Vec::new(),
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_004_000, |transaction| {
                    FinalizeSorafsModerationSortition::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        snapshot_digest,
                        vec![juror.clone()],
                        Vec::new(),
                    )
                    .execute(&outsider, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_004_000, |transaction| {
                    FinalizeSorafsModerationSortition::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        snapshot_digest,
                        vec![outsider.clone()],
                        Vec::new(),
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_004_000, |transaction| {
                    FinalizeSorafsModerationSortition::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        snapshot_digest,
                        vec![juror.clone(), juror.clone()],
                        Vec::new(),
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert_eq!(
            fixture.appeal().status,
            ModerationAppealStatusV1::RegisteringJurors
        );

        let sortition_digest = fixture.finalize_single_juror_sortition();
        assert!(
            fixture
                .run(1_004_001, |transaction| {
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        encode(&proof),
                    )
                    .execute(&outsider, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_004_001, |transaction| {
                    AcceptSorafsModerationJurorAssignment::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&outsider, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_004_001, |transaction| {
                    AcceptSorafsModerationJurorAssignment::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        [0xFF; 32],
                    )
                    .execute(&juror, transaction)
                })
                .is_err()
        );
        fixture
            .run(1_004_001, |transaction| {
                AcceptSorafsModerationJurorAssignment::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(1_004_002, |transaction| {
                    AcceptSorafsModerationJurorAssignment::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_005_000, |transaction| {
                    ActivateSorafsModerationCase::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_006_000, |transaction| {
                    ActivateSorafsModerationCase::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        [0xFE; 32],
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_006_000, |transaction| {
                    ActivateSorafsModerationCase::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&outsider, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_006_000, |transaction| {
                    let mut status =
                        status_for_mutation(transaction.world(), 1_006_000)?;
                    status.open_cases = u64::MAX;
                    transaction
                        .world
                        .smart_contract_state
                        .insert(status_key().clone(), encode_status(&status)?);
                    ActivateSorafsModerationCase::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        assert_eq!(
            fixture.appeal().status,
            ModerationAppealStatusV1::AwaitingAcceptance
        );
        assert!(
            FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
                .execute(&fixture.state.view())
                .is_err()
        );
        fixture
            .run(1_006_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        let appeal = fixture.appeal();
        assert_eq!(appeal.status, ModerationAppealStatusV1::BallotOpen);
        assert!(appeal.replacements.is_empty());
        let case = FindSorafsModerationCase::new(
            "panel-case".to_owned(),
            "round-1".to_owned(),
        )
        .execute(&fixture.state.view())
        .unwrap();
        assert_eq!(case.spec.jurors, vec![juror]);
        assert_eq!(case.spec.context.panel_roster_hash, sorafs_moderation_panel_roster_hash_v1(&case.spec.jurors, 1));
    }

    #[test]
    fn insufficient_pool_and_no_show_failover_exhaustion_are_terminal() {
        let mut insufficient = PanelFixture::new();
        insufficient.submit(1, 0, 1);
        let snapshot_digest = insufficient.appeal().pop_snapshot_digest;
        let manager = insufficient.manager_id();
        insufficient
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    Vec::new(),
                    Vec::new(),
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        assert_eq!(
            insufficient.appeal().status,
            ModerationAppealStatusV1::InsufficientEligiblePool
        );
        assert!(
            FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
                .execute(&insufficient.state.view())
                .is_err()
        );
        assert!(
            insufficient
                .run(1_004_001, |transaction| {
                    FinalizeSorafsModerationSortition::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        snapshot_digest,
                        Vec::new(),
                        Vec::new(),
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );

        let mut no_show = PanelFixture::new();
        no_show.submit(1, 0, 1);
        no_show.register_juror();
        let sortition_digest = no_show.finalize_single_juror_sortition();
        let manager = no_show.manager_id();
        no_show
            .run(1_006_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        assert_eq!(
            no_show.appeal().status,
            ModerationAppealStatusV1::FailoverExhausted
        );
        assert!(
            FindSorafsModerationCase::new("panel-case".to_owned(), "round-1".to_owned())
                .execute(&no_show.state.view())
                .is_err()
        );
        assert!(
            no_show
                .run(1_006_001, |transaction| {
                    ActivateSorafsModerationCase::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        sortition_digest,
                    )
                    .execute(&manager, transaction)
                })
                .is_err()
        );
        let status = FindSorafsModerationStatus
            .execute(&no_show.state.view())
            .unwrap();
        assert_eq!(status.failed_panel_formations, 1);
        assert_eq!(status.open_cases, 0);
    }

    #[test]
    fn primary_no_show_uses_next_unique_waitlist_juror_atomically() {
        let mut fixture = PanelFixture::new();
        let intake = panel_intake(&fixture.manager, "panel-case", 1, 1, 1, 0x92);
        let manager = fixture.manager_id();
        fixture
            .run(1_001_000, |transaction| {
                SubmitSorafsModerationAppeal::new(intake).execute(&manager, transaction)
            })
            .unwrap();
        let mut appeal = fixture.appeal();
        let juror = fixture.juror_id();
        let outsider = fixture.outsider_id();
        let records = [
            ModerationJurorEligibilityRecordV1 {
                case_id: "panel-case".to_owned(),
                round_id: "round-1".to_owned(),
                juror: juror.clone(),
                eligibility_class: ModerationJurorEligibilityClassV1::General,
                proof_digest: [0xA1; 32],
                nullifier: [0xB1; 32],
                pop_snapshot_digest: appeal.pop_snapshot_digest,
                credential_expires_at_epoch: 2_000,
                registered_at_unix_ms: 1_002_000,
            },
            ModerationJurorEligibilityRecordV1 {
                case_id: "panel-case".to_owned(),
                round_id: "round-1".to_owned(),
                juror: outsider.clone(),
                eligibility_class: ModerationJurorEligibilityClassV1::General,
                proof_digest: [0xA2; 32],
                nullifier: [0xB2; 32],
                pop_snapshot_digest: appeal.pop_snapshot_digest,
                credential_expires_at_epoch: 2_000,
                registered_at_unix_ms: 1_002_000,
            },
        ];
        appeal.eligible_jurors = vec![juror, outsider];
        appeal.eligible_jurors.sort_by_key(ToString::to_string);
        fixture
            .run(1_002_000, |transaction| {
                let mut status = status_for_mutation(transaction.world(), 1_002_000)?;
                status.eligibility_proofs = 2;
                status.updated_at_unix_ms = 1_002_000;
                transaction.world.smart_contract_state.insert(
                    appeal_key("panel-case", "round-1"),
                    encode_state(&appeal, "synthetic verified appeal")?,
                );
                for record in &records {
                    let encoded = encode_state(record, "synthetic verified eligibility")?;
                    transaction.world.smart_contract_state.insert(
                        eligibility_key("panel-case", "round-1", &record.juror),
                        encoded.clone(),
                    );
                    transaction
                        .world
                        .smart_contract_state
                        .insert(nullifier_key(record.nullifier), encoded);
                }
                transaction
                    .world
                    .smart_contract_state
                    .insert(status_key().clone(), encode_status(&status)?);
                Ok(())
            })
            .unwrap();
        let (expected_jurors, expected_waitlist, _, _) = sorafs_moderation_select_panel_v1(
            appeal.intake_digest,
            appeal.pop_snapshot_digest,
            appeal.pop_snapshot.randomness_anchor,
            &records,
            1,
            1,
            1,
        )
        .unwrap();
        let snapshot_digest = appeal.pop_snapshot_digest;
        fixture
            .run(1_004_000, |transaction| {
                FinalizeSorafsModerationSortition::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    snapshot_digest,
                    expected_jurors.clone(),
                    expected_waitlist.clone(),
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        let sortition_digest = fixture
            .appeal()
            .selection
            .expect("selection")
            .sortition_digest;
        fixture
            .run(1_006_000, |transaction| {
                ActivateSorafsModerationCase::new(
                    "panel-case".to_owned(),
                    "round-1".to_owned(),
                    sortition_digest,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        let appeal = fixture.appeal();
        assert_eq!(appeal.status, ModerationAppealStatusV1::BallotOpen);
        assert_eq!(appeal.replacements.len(), 1);
        assert_eq!(appeal.replacements[0].absent_juror, expected_jurors[0]);
        assert_eq!(
            appeal.replacements[0].replacement_juror,
            expected_waitlist[0]
        );
        let case = FindSorafsModerationCase::new(
            "panel-case".to_owned(),
            "round-1".to_owned(),
        )
        .execute(&fixture.state.view())
        .unwrap();
        assert_eq!(case.spec.jurors, expected_waitlist);
        assert_eq!(
            FindSorafsModerationStatus
                .execute(&fixture.state.view())
                .unwrap()
                .failover_replacements,
            1
        );
    }

    #[test]
    fn active_pop_root_rotation_invalidates_pending_appeal_snapshot() {
        let mut fixture = PanelFixture::new();
        fixture.submit(1, 0, 1);
        let manager = fixture.manager_id();
        let material = shared_pop_material();
        let entries = vec![PopRevocationEntryV1 {
            nonce: material.credential.revocation_nonce,
            revoked_at_epoch: 1_001,
            reason: PopRevocationReasonV1::GovernanceSuspension,
        }];
        let revocation_root = pop_revocation_root_v1(&entries).expect("rotated revocation root");
        let publication = sign_pop_revocations(
            PopRevocationListV1 {
                version: POP_REVOCATION_LIST_VERSION_V1,
                list_version: 2,
                commitment_root: material.root.root_digest,
                revocation_root,
                revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
                issuer_id: "pop-issuer-sora-foundation".to_owned(),
                published_at_epoch: 1_001,
                entries,
                publisher_signature: empty_pop_signature(&fixture.manager),
            },
            &fixture.manager,
        );
        let issuer_policy_digest = pop_policy(&fixture.manager)
            .digest()
            .expect("policy digest");
        fixture
            .run(1_001_500, |transaction| {
                PublishSorafsPopRevocationList::new(
                    encode(&publication),
                    issuer_policy_digest,
                )
                .execute(&manager, transaction)
            })
            .unwrap();
        let proof = proof_for_appeal(&fixture.appeal());
        let juror = fixture.juror_id();
        assert!(
            fixture
                .run(1_002_000, |transaction| {
                    RegisterSorafsModerationJurorEligibility::new(
                        "panel-case".to_owned(),
                        "round-1".to_owned(),
                        encode(&proof),
                    )
                    .execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(fixture.appeal().eligible_jurors.is_empty());
        assert_eq!(
            FindSorafsModerationStatus
                .execute(&fixture.state.view())
                .unwrap()
                .eligibility_proofs,
            0
        );
    }
}
