//! Authoritative SoraFS moderation commit/reveal ledger handlers.

use std::{str::FromStr, sync::OnceLock};

use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            FinalizeSorafsModerationCase, OpenSorafsModerationCase, RaiseSorafsModerationChallenge,
            ResolveSorafsModerationChallenge, SetSorafsModerationPolicy,
            SubmitSorafsModerationCommit, SubmitSorafsModerationReveal,
        },
    },
    name::Name,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsModerationCase, FindSorafsModerationChallenge, FindSorafsModerationCommit,
            FindSorafsModerationNoShow, FindSorafsModerationOutcome, FindSorafsModerationPolicy,
            FindSorafsModerationReveal, FindSorafsModerationStatus,
        },
    },
    sorafs::{
        moderation::{
            SoraFsModerationBallotCommitV1, SoraFsModerationBallotRevealV1,
            SoraFsModerationVoteChoice,
        },
        moderation_ledger::{
            MODERATION_LEDGER_MAX_IDENTIFIER_BYTES_V1, MODERATION_LEDGER_MAX_NONCE_BYTES_V1,
            MODERATION_LEDGER_MAX_REASON_BYTES_V1, ModerationCaseRecordV1, ModerationCaseStatusV1,
            ModerationChallengeDecisionV1, ModerationChallengeRecordV1, ModerationCommitRecordV1,
            ModerationLedgerPolicyRecord, ModerationLedgerStatusV1, ModerationNoShowKindV1,
            ModerationNoShowRecordV1, ModerationOutcomeKindV1, ModerationOutcomeRecordV1,
            ModerationRevealRecordV1, ModerationVoteCountsV1,
        },
    },
};
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};

use super::*;
use crate::{
    smartcontracts::ValidSingularQuery,
    state::{StateTransaction, WorldReadOnly},
};

const POLICY_STATE_KEY: &str = "sorafs_moderation_policy_v1";
const STATUS_STATE_KEY: &str = "sorafs_moderation_status_v1";
const CASE_STATE_KEY_PREFIX: &str = "sorafs_moderation_case_v1_";
const COMMIT_STATE_KEY_PREFIX: &str = "sorafs_moderation_commit_v1_";
const REVEAL_STATE_KEY_PREFIX: &str = "sorafs_moderation_reveal_v1_";
const CHALLENGE_STATE_KEY_PREFIX: &str = "sorafs_moderation_challenge_v1_";
const OUTCOME_STATE_KEY_PREFIX: &str = "sorafs_moderation_outcome_v1_";
const NO_SHOW_STATE_KEY_PREFIX: &str = "sorafs_moderation_no_show_v1_";
const CASE_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.case-state-key.v1";
const JUROR_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.juror-state-key.v1";
const CHALLENGE_KEY_DOMAIN_V1: &[u8] = b"sorafs.moderation.challenge-state-key.v1";
const STATE_MAX_BYTES: usize = 512 * 1024;
const PAYLOAD_MAX_BYTES: usize = 64 * 1024;
const STATE_LIMITS: DecodeLimits =
    DecodeLimits::new(256, STATE_MAX_BYTES, 4_096, 2 * STATE_MAX_BYTES, 64);
const PAYLOAD_LIMITS: DecodeLimits =
    DecodeLimits::new(256, PAYLOAD_MAX_BYTES, 2_048, 2 * PAYLOAD_MAX_BYTES, 64);
const MANAGE_PERMISSION: &str = "CanManageSorafsModeration";

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

impl Execute for OpenSorafsModerationCase {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_manage_permission(state_transaction, authority)?;
        self.spec.validate().map_err(|error| {
            invalid_parameter(format!("invalid SoraFS moderation case: {error}"))
        })?;
        let now = block_time_ms(state_transaction)?;
        let policy = read_policy(state_transaction.world())?.ok_or_else(|| {
            invalid_parameter("authoritative moderation policy is not configured")
        })?;
        if self.spec.policy_digest != policy.policy_digest {
            return Err(invalid_parameter(
                "moderation case policy digest does not match the active policy",
            ));
        }
        if self.spec.jurors.len() > usize::from(policy.policy.max_panel_size) {
            return Err(invalid_parameter(format!(
                "moderation panel size {} exceeds active policy limit {}",
                self.spec.jurors.len(),
                policy.policy.max_panel_size
            )));
        }
        for juror in &self.spec.jurors {
            if state_transaction.world.accounts.get(juror).is_none() {
                return Err(invalid_parameter(format!(
                    "moderation juror account {juror} is not registered"
                )));
            }
        }
        if now >= self.spec.commit_deadline_unix_ms {
            return Err(invalid_parameter(
                "moderation case commit deadline must be later than opening block time",
            ));
        }
        let total_window = self
            .spec
            .reveal_deadline_unix_ms
            .checked_sub(now)
            .ok_or_else(|| invalid_parameter("moderation case reveal deadline is in the past"))?;
        if total_window > policy.policy.max_total_window_ms {
            return Err(invalid_parameter(format!(
                "moderation case total window {total_window} ms exceeds active policy limit {} ms",
                policy.policy.max_total_window_ms
            )));
        }
        if read_case(
            state_transaction.world(),
            &self.spec.context.case_id,
            &self.spec.round_id,
        )?
        .is_some()
        {
            return Err(invalid_parameter(format!(
                "moderation case `{}` round `{}` already exists",
                self.spec.context.case_id, self.spec.round_id
            )));
        }
        if read_outcome(
            state_transaction.world(),
            &self.spec.context.case_id,
            &self.spec.round_id,
        )?
        .is_some()
        {
            return Err(corrupt_state(
                "moderation outcome exists without its authoritative case",
            ));
        }
        let mut status = status_for_mutation(state_transaction.world(), now)?;
        status.open_cases = checked_inc(status.open_cases, "open-case")?;
        status.updated_at_unix_ms = now;
        let record = ModerationCaseRecordV1 {
            spec: self.spec,
            policy: policy.policy,
            status: ModerationCaseStatusV1::Open,
            opened_at_unix_ms: now,
            opened_by: authority.clone(),
            commitment_count: 0,
            reveal_count: 0,
            challenge_count: 0,
            pending_challenge_count: 0,
            accepted_challenge_count: 0,
        };
        let key = case_key(&record.spec.context.case_id, &record.spec.round_id);
        let encoded_record = encode_state(&record, "moderation case")?;
        let encoded_status = encode_status(&status)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(key, encoded_record);
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
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        account::{Account, AccountId},
        block::BlockHeader,
        permission::{Permission, Permissions},
        sorafs::{
            moderation::{
                SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
                SoraFsModerationBallotContextV1, SoraFsModerationBallotRevealV1,
            },
            moderation_ledger::{
                MODERATION_LEDGER_CASE_VERSION_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationCaseSpecV1, ModerationChallengeDecisionV1, ModerationChallengeKindV1,
                ModerationLedgerPolicyV1, ModerationNoShowKindV1, ModerationOutcomeKindV1,
                sorafs_moderation_panel_roster_hash_v1,
            },
        },
    };
    use iroha_primitives::json::Json;

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
        permissions.insert(Permission::new(MANAGE_PERMISSION.to_owned(), Json::new(())));
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
                OpenSorafsModerationCase::new(spec.clone()).execute(&manager_id, transaction)
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
        let manager = fixture.manager_id();
        let duplicate_spec = fixture.spec.clone();
        assert!(
            fixture
                .run(OPENED_AT + 1, |transaction| {
                    OpenSorafsModerationCase::new(duplicate_spec).execute(&manager, transaction)
                })
                .is_err()
        );
        let juror = fixture.juror_id(0);
        let outsider = account(&fixture.outsider);
        let reveal = reveal(
            &fixture.spec,
            &juror,
            SoraFsModerationVoteChoice::Overturn,
            3,
        );
        let commit = commit(&reveal);
        fixture
            .run(1_500, |transaction| {
                SubmitSorafsModerationCommit::new(encode(&commit)).execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(1_501, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&commit)).execute(&juror, transaction)
                })
                .is_err()
        );
        assert!(
            fixture
                .run(1_501, |transaction| {
                    SubmitSorafsModerationCommit::new(encode(&commit))
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
                    SubmitSorafsModerationReveal::new(encode(&reveal)).execute(&juror, transaction)
                })
                .is_err()
        );
        let mut mismatched = reveal.clone();
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
                SubmitSorafsModerationReveal::new(encode(&reveal)).execute(&juror, transaction)
            })
            .unwrap();
        assert!(
            fixture
                .run(3_501, |transaction| {
                    SubmitSorafsModerationReveal::new(encode(&reveal)).execute(&juror, transaction)
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

        let single_spec = spec(vec![outsider.clone()], 1);
        assert!(
            transact(&mut state, 2, OPENED_AT + 1, |transaction| {
                OpenSorafsModerationCase::new(single_spec.clone()).execute(&outsider, transaction)
            })
            .is_err()
        );
        {
            let mut block = state.block(header(2, OPENED_AT + 1));
            let mut transaction = block.transaction();
            let mut status = read_status(transaction.world()).unwrap().expect("status");
            status.open_cases = u64::MAX;
            transaction
                .world
                .smart_contract_state
                .insert(status_key().clone(), encode_status(&status).unwrap());
            assert!(
                OpenSorafsModerationCase::new(single_spec.clone())
                    .execute(&manager, &mut transaction)
                    .is_err()
            );
            assert!(
                read_case(transaction.world(), "case-1", "round-1")
                    .unwrap()
                    .is_none()
            );
        }

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
}
