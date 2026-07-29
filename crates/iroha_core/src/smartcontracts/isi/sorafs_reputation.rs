//! Authoritative native SoraFS reputation journal.
//!
//! Every policy, event, idempotency index, and source head is stored inside
//! consensus-owned `smart_contract_state`.  This uses the same checkpointed,
//! state-root-covered storage path as the other first-release SoraFS ledgers;
//! no daemon database is authoritative.

use std::{str::FromStr, sync::OnceLock};

use iroha_data_model::{
    account::AccountId,
    events::data::sorafs::{
        SorafsGatewayEvent, SorafsReputationJournalEntryCommittedV1, SorafsReputationJournalEvent,
        SorafsReputationJournalPolicyActivatedV1,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
            ResolveSorafsCapacityDispute, SetSorafsReputationJournalAuthorityPolicy,
        },
    },
    name::Name,
    permission::Permission,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsReputationJournalAuthorityPolicy, FindSorafsReputationJournalEvents,
        },
    },
    sorafs::{
        capacity::{
            CapacityDisputeRecord, CapacityDisputeResolution, CapacityDisputeStatus, ProviderId,
        },
        reputation::{
            ProviderDisputeEventV1, ProviderDisputeKindV1, ProviderDisputeResolutionV1,
            ProviderDisputeStatusV1, REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1, ReputationJournalAuthorityPolicyRecordV1,
            ReputationJournalCommittedEventRecordV1, ReputationJournalEntryV1,
            ReputationJournalEventIdV1, ReputationJournalFinalizedCursorV1,
            ReputationJournalFinalizedEventCursorV1, ReputationJournalFinalizedEventPageV1,
            ReputationJournalFinalizedEventV1, ReputationJournalPayloadV1,
            ReputationJournalSourceHeadV1, ReputationJournalSourceIdV1,
            ReputationJournalSourceKindV1,
        },
    },
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};

use super::*;
use crate::{
    smartcontracts::ValidSingularQuery,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

const ACTIVE_POLICY_STATE_KEY: &str = "sorafs_reputation_policy_active_v1";
const POLICY_HISTORY_STATE_KEY_PREFIX: &str = "sorafs_reputation_policy_history_v1_";
const JOURNAL_HEAD_STATE_KEY: &str = "sorafs_reputation_journal_head_v1";
const EVENT_STATE_KEY_PREFIX: &str = "sorafs_reputation_event_v1_";
const EVENT_ID_STATE_KEY_PREFIX: &str = "sorafs_reputation_event_id_v1_";
const SOURCE_HEAD_STATE_KEY_PREFIX: &str = "sorafs_reputation_source_head_v1_";

const CAN_MANAGE_POLICY: &str = "CanManageSorafsReputationJournalPolicy";
const CAN_RECORD_ENTRY: &str = "CanRecordSorafsReputationJournal";
const CAN_RESOLVE_DISPUTE: &str = "CanResolveSorafsCapacityDispute";

const STATE_MAX_BYTES: usize = 128 * 1024;
const STATE_LIMITS: DecodeLimits = DecodeLimits::new(
    STATE_MAX_BYTES,
    STATE_MAX_BYTES,
    2 * STATE_MAX_BYTES,
    STATE_MAX_BYTES * 2,
    64,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ReputationJournalHeadStateV1 {
    last_sequence: u64,
    last_target_block_height: u64,
    last_event_index: u32,
}

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn corrupt_state(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn query_failure(error: InstructionExecutionError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

fn encode_state<T: norito::core::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, InstructionExecutionError> {
    let bytes = norito::to_bytes(value)
        .map_err(|error| corrupt_state(format!("failed to encode {label}: {error}")))?;
    if bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} encodes to {} bytes; maximum is {STATE_MAX_BYTES}",
            bytes.len()
        )));
    }
    Ok(bytes)
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

fn active_policy_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        Name::from_str(ACTIVE_POLICY_STATE_KEY).expect("static reputation policy key is valid")
    })
}

fn journal_head_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        Name::from_str(JOURNAL_HEAD_STATE_KEY).expect("static reputation head key is valid")
    })
}

fn digest_key(prefix: &str, digest: &[u8; 32]) -> Name {
    Name::from_str(&format!("{prefix}{}", hex::encode(digest)))
        .expect("static prefix plus lowercase hex is a valid state key")
}

fn policy_history_key(digest: &[u8; 32]) -> Name {
    digest_key(POLICY_HISTORY_STATE_KEY_PREFIX, digest)
}

fn event_key(sequence: u64) -> Name {
    Name::from_str(&format!("{EVENT_STATE_KEY_PREFIX}{sequence:016x}"))
        .expect("static event prefix plus fixed-width lowercase hex is a valid state key")
}

fn event_id_key(event_id: ReputationJournalEventIdV1) -> Name {
    digest_key(EVENT_ID_STATE_KEY_PREFIX, event_id.as_bytes())
}

fn source_head_key(source_id: ReputationJournalSourceIdV1) -> Name {
    digest_key(SOURCE_HEAD_STATE_KEY_PREFIX, source_id.as_bytes())
}

fn state_prefix_has_any(world: &impl WorldReadOnly, prefix: &str) -> bool {
    let start = Name::from_str(prefix).expect("static state prefix is valid");
    world
        .smart_contract_state()
        .range(start..)
        .next()
        .is_some_and(|(key, _)| key.to_string().starts_with(prefix))
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> bool {
    let required = Permission::new(permission.to_owned(), Json::new(()));
    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.iter().any(|candidate| candidate == &required))
    {
        return true;
    }
    state_transaction
        .world
        .account_roles_iter(authority)
        .filter_map(|role_id| state_transaction.world.roles.get(role_id))
        .any(|role| role.permissions().any(|candidate| candidate == &required))
}

fn require_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> Result<(), InstructionExecutionError> {
    if state_transaction.world.accounts.get(authority).is_none() {
        return Err(invalid_parameter(
            "authoritative SoraFS reputation operation requires a registered transaction authority",
        ));
    }
    if has_permission(state_transaction, authority, permission) {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "permission {permission} is required for authoritative SoraFS reputation operation"
        )))
    }
}

fn block_time_ms(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let timestamp = state_transaction.block_unix_timestamp_ms();
    if timestamp == 0 || timestamp == u64::MAX {
        return Err(invalid_parameter(
            "authoritative reputation operations require a finite non-zero block timestamp",
        ));
    }
    Ok(timestamp)
}

fn read_policy_history(
    world: &impl WorldReadOnly,
    digest: &[u8; 32],
) -> Result<Option<ReputationJournalAuthorityPolicyRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&policy_history_key(digest))
    else {
        return Ok(None);
    };
    let record: ReputationJournalAuthorityPolicyRecordV1 =
        decode_state(bytes, "reputation recorder-policy history")?;
    record.validate().map_err(|error| {
        corrupt_state(format!(
            "stored reputation recorder-policy history is invalid: {error}"
        ))
    })?;
    if &record.policy_digest != digest {
        return Err(corrupt_state(
            "reputation recorder-policy history key does not match its digest",
        ));
    }
    Ok(Some(record))
}

fn validate_policy_predecessor(
    world: &impl WorldReadOnly,
    record: &ReputationJournalAuthorityPolicyRecordV1,
) -> Result<(), InstructionExecutionError> {
    let Some(predecessor_digest) = record.policy.predecessor_policy_digest else {
        if record.policy.revision == 1 {
            return Ok(());
        }
        return Err(corrupt_state(
            "reputation recorder policy is missing its predecessor digest",
        ));
    };
    let predecessor = read_policy_history(world, &predecessor_digest)?.ok_or_else(|| {
        corrupt_state("reputation recorder policy predecessor is missing from immutable history")
    })?;
    if predecessor.policy.revision.checked_add(1) != Some(record.policy.revision)
        || predecessor.activated_at_unix_ms > record.activated_at_unix_ms
    {
        return Err(corrupt_state(
            "reputation recorder policy predecessor revision or activation order is invalid",
        ));
    }
    Ok(())
}

fn read_active_policy(
    world: &impl WorldReadOnly,
) -> Result<Option<ReputationJournalAuthorityPolicyRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(active_policy_key()) else {
        if state_prefix_has_any(world, POLICY_HISTORY_STATE_KEY_PREFIX) {
            return Err(corrupt_state(
                "reputation recorder-policy history exists without an active policy",
            ));
        }
        return Ok(None);
    };
    let record: ReputationJournalAuthorityPolicyRecordV1 =
        decode_state(bytes, "active reputation recorder policy")?;
    record.validate().map_err(|error| {
        corrupt_state(format!(
            "stored active reputation recorder policy is invalid: {error}"
        ))
    })?;
    let history = read_policy_history(world, &record.policy_digest)?.ok_or_else(|| {
        corrupt_state("active reputation recorder policy is missing immutable history")
    })?;
    if history != record {
        return Err(corrupt_state(
            "active reputation recorder policy differs from immutable history",
        ));
    }
    validate_policy_predecessor(world, &record)?;
    Ok(Some(record))
}

/// Return the complete active recorder-policy predecessor chain in ascending
/// revision order from one immutable world view.
///
/// This is a crate-internal capture primitive for the finalized reputation
/// archive. It never substitutes current state for the supplied view and
/// fails closed when the active record, any predecessor artifact, or the
/// direct revision/digest/activation ordering is malformed. `maximum_records`
/// is a hard allocation and traversal bound.
pub(crate) fn read_reputation_authority_policy_history(
    world: &impl WorldReadOnly,
    maximum_records: usize,
) -> Result<Vec<ReputationJournalAuthorityPolicyRecordV1>, InstructionExecutionError> {
    if maximum_records == 0 {
        return Err(invalid_parameter(
            "reputation recorder-policy history bound must be non-zero",
        ));
    }
    let mut current = read_active_policy(world)?
        .ok_or_else(|| invalid_parameter("SoraFS reputation recorder policy is not configured"))?;
    let mut descending = Vec::new();
    loop {
        if descending.len() >= maximum_records {
            return Err(corrupt_state(
                "reputation recorder-policy history exceeds the finalized capture bound",
            ));
        }
        validate_policy_predecessor(world, &current)?;
        let revision = current.policy.revision;
        let predecessor_digest = current.policy.predecessor_policy_digest;
        descending.push(current);
        match (revision, predecessor_digest) {
            (1, None) => break,
            (1, Some(_)) => {
                return Err(corrupt_state(
                    "first reputation recorder policy unexpectedly has a predecessor",
                ));
            }
            (_, None) => {
                return Err(corrupt_state(
                    "reputation recorder-policy history ended before revision one",
                ));
            }
            (_, Some(predecessor_digest)) => {
                let predecessor =
                    read_policy_history(world, &predecessor_digest)?.ok_or_else(|| {
                        corrupt_state(
                            "reputation recorder-policy predecessor is missing from immutable history",
                        )
                    })?;
                let successor = descending.last().ok_or_else(|| {
                    corrupt_state("reputation recorder-policy traversal lost its current successor")
                })?;
                if predecessor.policy.revision.checked_add(1) != Some(successor.policy.revision)
                    || successor.policy.predecessor_policy_digest != Some(predecessor.policy_digest)
                    || predecessor.activated_at_unix_ms > successor.activated_at_unix_ms
                {
                    return Err(corrupt_state(
                        "reputation recorder-policy history is skipped, substituted, or non-monotonic",
                    ));
                }
                current = predecessor;
            }
        }
    }
    descending.reverse();
    Ok(descending)
}

fn read_policy_at_source_time(
    world: &impl WorldReadOnly,
    active: ReputationJournalAuthorityPolicyRecordV1,
    policy_digest: [u8; 32],
    source_time_unix_ms: u64,
) -> Result<ReputationJournalAuthorityPolicyRecordV1, InstructionExecutionError> {
    let requested = read_policy_history(world, &policy_digest)?.ok_or_else(|| {
        invalid_parameter("reputation entry references unknown recorder-policy history")
    })?;
    if requested.policy.revision > active.policy.revision {
        return Err(invalid_parameter(
            "reputation entry references a recorder policy newer than the active policy",
        ));
    }

    let mut cursor = active;
    let mut successor_activated_at = None;
    loop {
        validate_policy_predecessor(world, &cursor)?;
        if cursor.policy.revision == requested.policy.revision {
            if cursor != requested {
                return Err(corrupt_state(
                    "reputation recorder-policy history is not on the active predecessor chain",
                ));
            }
            if source_time_unix_ms < cursor.activated_at_unix_ms
                || successor_activated_at
                    .is_some_and(|activated_at| source_time_unix_ms >= activated_at)
            {
                return Err(invalid_parameter(
                    "reputation source observation falls outside its recorder-policy activation interval",
                ));
            }
            return Ok(cursor);
        }
        let predecessor_digest = cursor.policy.predecessor_policy_digest.ok_or_else(|| {
            corrupt_state(
                "active reputation recorder-policy lineage ended before the requested revision",
            )
        })?;
        successor_activated_at = Some(cursor.activated_at_unix_ms);
        cursor = read_policy_history(world, &predecessor_digest)?.ok_or_else(|| {
            corrupt_state("active reputation recorder-policy predecessor history is missing")
        })?;
    }
}

fn read_journal_head(
    world: &impl WorldReadOnly,
) -> Result<Option<ReputationJournalHeadStateV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(journal_head_key()) else {
        if state_prefix_has_any(world, EVENT_STATE_KEY_PREFIX)
            || state_prefix_has_any(world, EVENT_ID_STATE_KEY_PREFIX)
            || state_prefix_has_any(world, SOURCE_HEAD_STATE_KEY_PREFIX)
        {
            return Err(corrupt_state(
                "reputation journal indexes exist without a journal head",
            ));
        }
        return Ok(None);
    };
    let head: ReputationJournalHeadStateV1 = decode_state(bytes, "reputation journal head")?;
    if head.last_sequence == 0 || head.last_target_block_height == 0 {
        return Err(corrupt_state("stored reputation journal head is inert"));
    }
    Ok(Some(head))
}

fn read_event(
    world: &impl WorldReadOnly,
    sequence: u64,
) -> Result<Option<ReputationJournalCommittedEventRecordV1>, InstructionExecutionError> {
    if sequence == 0 {
        return Err(corrupt_state(
            "reputation journal sequence zero cannot be read",
        ));
    }
    let Some(bytes) = world.smart_contract_state().get(&event_key(sequence)) else {
        return Ok(None);
    };
    let record: ReputationJournalCommittedEventRecordV1 =
        decode_state(bytes, "reputation committed event")?;
    record
        .validate()
        .map_err(|error| corrupt_state(format!("stored reputation event is invalid: {error}")))?;
    if record.sequence != sequence {
        return Err(corrupt_state(
            "reputation event key does not match its global sequence",
        ));
    }
    Ok(Some(record))
}

fn read_event_id_sequence(
    world: &impl WorldReadOnly,
    event_id: ReputationJournalEventIdV1,
) -> Result<Option<u64>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&event_id_key(event_id)) else {
        return Ok(None);
    };
    let sequence: u64 = decode_state(bytes, "reputation event-id index")?;
    if sequence == 0 {
        return Err(corrupt_state(
            "reputation event-id index points to sequence zero",
        ));
    }
    Ok(Some(sequence))
}

fn read_source_head(
    world: &impl WorldReadOnly,
    source_id: ReputationJournalSourceIdV1,
) -> Result<Option<ReputationJournalSourceHeadV1>, InstructionExecutionError> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&source_head_key(source_id))
    else {
        return Ok(None);
    };
    let head: ReputationJournalSourceHeadV1 = decode_state(bytes, "reputation source head")?;
    head.validate().map_err(|error| {
        corrupt_state(format!("stored reputation source head is invalid: {error}"))
    })?;
    Ok(Some(head))
}

fn validate_provider_binding(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<(), InstructionExecutionError> {
    if world.capacity_declarations().get(&provider_id).is_none() {
        return Err(invalid_parameter(format!(
            "reputation entry references unknown capacity provider {}",
            hex::encode(provider_id.as_bytes())
        )));
    }
    let Some(owner) = world.provider_owners().get(&provider_id) else {
        return Err(invalid_parameter(format!(
            "reputation entry references provider {} without an owner binding",
            hex::encode(provider_id.as_bytes())
        )));
    };
    if world.accounts().get(owner).is_none() {
        return Err(corrupt_state(format!(
            "reputation entry references provider {} whose owner account is missing",
            hex::encode(provider_id.as_bytes())
        )));
    }
    Ok(())
}

fn validate_event_successor(
    previous: Option<&ReputationJournalCommittedEventRecordV1>,
    current: &ReputationJournalCommittedEventRecordV1,
) -> Result<(), InstructionExecutionError> {
    let Some(previous) = previous else {
        if current.sequence != 1 || current.event_index != 0 {
            return Err(corrupt_state(
                "reputation journal must begin at sequence one and block index zero",
            ));
        }
        return Ok(());
    };
    let expected_sequence = previous
        .sequence
        .checked_add(1)
        .ok_or_else(|| corrupt_state("reputation journal sequence overflow"))?;
    if current.sequence != expected_sequence {
        return Err(corrupt_state(
            "reputation journal sequence is not globally contiguous",
        ));
    }
    if current.recorded_at_unix_ms < previous.recorded_at_unix_ms {
        return Err(corrupt_state(
            "reputation journal committing timestamps are reordered",
        ));
    }
    match previous
        .target_block_height
        .cmp(&current.target_block_height)
    {
        core::cmp::Ordering::Less if current.event_index == 0 => Ok(()),
        core::cmp::Ordering::Equal
            if previous.event_index.checked_add(1) == Some(current.event_index) =>
        {
            Ok(())
        }
        _ => Err(corrupt_state(
            "reputation journal block height or event index is not contiguous",
        )),
    }
}

fn validate_event_indexes(
    world: &impl WorldReadOnly,
    record: &ReputationJournalCommittedEventRecordV1,
) -> Result<(), InstructionExecutionError> {
    let active = read_active_policy(world)?.ok_or_else(|| {
        corrupt_state("reputation event exists without an active recorder policy")
    })?;
    let policy = read_policy_at_source_time(
        world,
        active,
        record.entry.authority_policy_digest,
        record.entry.source_time_unix_ms,
    )
    .map_err(|error| {
        corrupt_state(format!(
            "reputation event violates recorder-policy activation history: {error}"
        ))
    })?;
    record
        .entry
        .validate_at_commit(&policy.policy, record.recorded_at_unix_ms)
        .map_err(|error| {
            corrupt_state(format!(
                "reputation event violates its immutable recorder policy: {error}"
            ))
        })?;
    let indexed_sequence = read_event_id_sequence(world, record.entry.event_id)?
        .ok_or_else(|| corrupt_state("reputation event is missing its event-id index"))?;
    if indexed_sequence != record.sequence {
        return Err(corrupt_state(
            "reputation event-id index points to another sequence",
        ));
    }
    let source_head = read_source_head(world, record.entry.source_id)?
        .ok_or_else(|| corrupt_state("reputation event is missing its source head"))?;
    if source_head.source_kind != record.entry.source_kind()
        || source_head.sequence < record.sequence
        || source_head.source_revision < record.entry.source_revision
    {
        return Err(corrupt_state(
            "reputation source head regresses behind a committed event",
        ));
    }
    if source_head.sequence == record.sequence {
        if source_head.source_revision != record.entry.source_revision
            || source_head.event_id != record.entry.event_id
        {
            return Err(corrupt_state(
                "reputation source head does not identify its latest event",
            ));
        }
    } else {
        let terminal = read_event(world, source_head.sequence)?.ok_or_else(|| {
            corrupt_state("reputation source head points to a missing terminal event")
        })?;
        if terminal.entry.source_id != record.entry.source_id
            || terminal.entry.provider_id != record.entry.provider_id
            || terminal.entry.source_revision != record.entry.source_revision.saturating_add(1)
            || terminal.entry.predecessor_event_id != Some(record.entry.event_id)
            || source_head.event_id != terminal.entry.event_id
        {
            return Err(corrupt_state(
                "reputation source head does not form an exact predecessor chain",
            ));
        }
        // Validate only after binding the terminal to this exact source. This
        // keeps corrupt cross-source heads from creating an unbounded recursive
        // validation walk.
        validate_event_indexes(world, &terminal)?;
    }
    Ok(())
}

fn ensure_no_event_after_head(
    world: &impl WorldReadOnly,
    head: Option<ReputationJournalHeadStateV1>,
) -> Result<(), InstructionExecutionError> {
    let prefix = Name::from_str(EVENT_STATE_KEY_PREFIX).expect("static event prefix is valid");
    let first = world
        .smart_contract_state()
        .range(prefix..)
        .next()
        .and_then(|(key, _)| {
            key.to_string()
                .starts_with(EVENT_STATE_KEY_PREFIX)
                .then_some(key)
        });
    let head = match (head, first) {
        (None, None) => return Ok(()),
        (None, Some(_)) => {
            return Err(corrupt_state(
                "reputation journal contains events without a journal head",
            ));
        }
        (Some(head), Some(key)) if key == &event_key(1) => head,
        (Some(_), _) => {
            return Err(corrupt_state(
                "reputation journal does not begin at global sequence one",
            ));
        }
    };
    let terminal_key = event_key(head.last_sequence);
    for (key, _) in world.smart_contract_state().range(terminal_key.clone()..) {
        if !key.to_string().starts_with(EVENT_STATE_KEY_PREFIX) {
            break;
        }
        if key == &terminal_key {
            continue;
        }
        return Err(corrupt_state(
            "reputation event exists beyond the journal head",
        ));
    }
    Ok(())
}

fn validate_journal_head(
    world: &impl WorldReadOnly,
) -> Result<Option<ReputationJournalHeadStateV1>, InstructionExecutionError> {
    // The journal and its immutable policy history are one authoritative
    // state machine; neither may remain usable after losing the active policy.
    read_active_policy(world)?;
    let head = read_journal_head(world)?;
    ensure_no_event_after_head(world, head)?;
    let Some(head) = head else {
        return Ok(None);
    };
    let terminal = read_event(world, head.last_sequence)?
        .ok_or_else(|| corrupt_state("reputation journal head points to a missing event"))?;
    if terminal.target_block_height != head.last_target_block_height
        || terminal.event_index != head.last_event_index
    {
        return Err(corrupt_state(
            "reputation journal head differs from its terminal event",
        ));
    }
    let predecessor = if terminal.sequence == 1 {
        None
    } else {
        let predecessor_sequence = terminal.sequence - 1;
        let predecessor = read_event(world, predecessor_sequence)?.ok_or_else(|| {
            corrupt_state(format!(
                "reputation journal terminal event is missing predecessor sequence {predecessor_sequence}"
            ))
        })?;
        validate_event_indexes(world, &predecessor)?;
        Some(predecessor)
    };
    validate_event_successor(predecessor.as_ref(), &terminal)?;
    validate_event_indexes(world, &terminal)?;
    Ok(Some(head))
}

fn exact_entry_replay(
    world: &impl WorldReadOnly,
    entry: &ReputationJournalEntryV1,
) -> Result<Option<u64>, InstructionExecutionError> {
    let journal_head = validate_journal_head(world)?;
    let indexed = read_event_id_sequence(world, entry.event_id)?;
    let source_head = read_source_head(world, entry.source_id)?;
    match indexed {
        Some(sequence) => {
            if journal_head.is_none_or(|head| sequence > head.last_sequence) {
                return Err(corrupt_state(
                    "reputation event-id index points beyond the journal head",
                ));
            }
            let record = read_event(world, sequence)?.ok_or_else(|| {
                corrupt_state("reputation event-id index points to a missing event")
            })?;
            if &record.entry != entry {
                return Err(corrupt_state(
                    "reputation event-id index aliases different canonical content",
                ));
            }
            validate_event_indexes(world, &record)?;
            Ok(Some(sequence))
        }
        None => {
            if source_head.is_some() {
                return Err(invalid_parameter(
                    "reputation source revision is already occupied by different content",
                ));
            }
            Ok(None)
        }
    }
}

fn validate_new_source_revision(
    world: &impl WorldReadOnly,
    entry: &ReputationJournalEntryV1,
) -> Result<(), InstructionExecutionError> {
    if read_event_id_sequence(world, entry.event_id)?.is_some() {
        return Err(corrupt_state(
            "new reputation entry unexpectedly has an event-id index",
        ));
    }
    match (
        entry.source_revision,
        read_source_head(world, entry.source_id)?,
    ) {
        (1, None) => Ok(()),
        (1, Some(_)) => Err(invalid_parameter(
            "reputation source revision one is already occupied",
        )),
        (2, Some(head))
            if head.source_kind == entry.source_kind()
                && head.source_revision == 1
                && entry.predecessor_event_id == Some(head.event_id) =>
        {
            let predecessor = read_event(world, head.sequence)?.ok_or_else(|| {
                corrupt_state("reputation source head points to a missing predecessor")
            })?;
            if predecessor.entry.event_id != head.event_id
                || predecessor.entry.source_id != entry.source_id
                || predecessor.entry.provider_id != entry.provider_id
            {
                return Err(corrupt_state(
                    "reputation source predecessor binding is corrupt",
                ));
            }
            Ok(())
        }
        (2, Some(_)) => Err(invalid_parameter(
            "reputation revision two does not extend the exact source head",
        )),
        (2, None) => Err(invalid_parameter(
            "reputation revision two is missing its committed predecessor",
        )),
        _ => Err(invalid_parameter(
            "reputation source revision is outside the first-release lifecycle",
        )),
    }
}

fn validate_entry_commit_context(
    world: &impl WorldReadOnly,
    entry: &ReputationJournalEntryV1,
    recorded_at_unix_ms: u64,
) -> Result<(), InstructionExecutionError> {
    let active = read_active_policy(world)?
        .ok_or_else(|| invalid_parameter("SoraFS reputation recorder policy is not configured"))?;
    if active.activated_at_unix_ms > recorded_at_unix_ms {
        return Err(corrupt_state(
            "active reputation policy activation is later than the executing block",
        ));
    }
    let policy = read_policy_at_source_time(
        world,
        active,
        entry.authority_policy_digest,
        entry.source_time_unix_ms,
    )?;
    entry
        .validate_at_commit(&policy.policy, recorded_at_unix_ms)
        .map_err(|error| {
            invalid_parameter(format!(
                "reputation entry does not match its source-time recorder policy: {error}"
            ))
        })
}

fn append_validated_entry(
    state_transaction: &mut StateTransaction<'_, '_>,
    entry: ReputationJournalEntryV1,
) -> Result<u64, InstructionExecutionError> {
    let recorded_at_unix_ms = block_time_ms(state_transaction)?;
    validate_entry_commit_context(state_transaction.world(), &entry, recorded_at_unix_ms)?;
    validate_new_source_revision(state_transaction.world(), &entry)?;
    let head = validate_journal_head(state_transaction.world())?;

    let committed_parent_height = u64::try_from(state_transaction.block_hashes().len())
        .map_err(|_| corrupt_state("committed reputation parent height does not fit into u64"))?;
    let target_block_height = committed_parent_height
        .checked_add(1)
        .ok_or_else(|| corrupt_state("reputation event target block height overflow"))?;
    if target_block_height != state_transaction._curr_block.height().get() {
        return Err(corrupt_state(
            "reputation event target height differs from the executing block",
        ));
    }

    let (sequence, event_index, previous) = match head {
        None => (1, 0, None),
        Some(head) => {
            let previous =
                read_event(state_transaction.world(), head.last_sequence)?.ok_or_else(|| {
                    corrupt_state("reputation journal head points to a missing event")
                })?;
            let sequence = head
                .last_sequence
                .checked_add(1)
                .ok_or_else(|| corrupt_state("reputation journal sequence overflow"))?;
            let event_index = match head.last_target_block_height.cmp(&target_block_height) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => head
                    .last_event_index
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("reputation block event-index overflow"))?,
                core::cmp::Ordering::Greater => {
                    return Err(corrupt_state(
                        "reputation event target height regressed behind the journal head",
                    ));
                }
            };
            (sequence, event_index, Some(previous))
        }
    };

    let record = ReputationJournalCommittedEventRecordV1 {
        sequence,
        target_block_height,
        event_index,
        recorded_at_unix_ms,
        entry: entry.clone(),
    };
    record
        .validate()
        .map_err(|error| invalid_parameter(format!("invalid reputation journal entry: {error}")))?;
    validate_event_successor(previous.as_ref(), &record)?;
    let next_source_head = ReputationJournalSourceHeadV1 {
        source_kind: entry.source_kind(),
        source_revision: entry.source_revision,
        event_id: entry.event_id,
        sequence,
    };
    next_source_head
        .validate()
        .map_err(|error| corrupt_state(format!("invalid next reputation source head: {error}")))?;
    let next_head = ReputationJournalHeadStateV1 {
        last_sequence: sequence,
        last_target_block_height: target_block_height,
        last_event_index: event_index,
    };

    let event_bytes = encode_state(&record, "reputation committed event")?;
    let event_id_bytes = encode_state(&sequence, "reputation event-id index")?;
    let source_head_bytes = encode_state(&next_source_head, "reputation source head")?;
    let journal_head_bytes = encode_state(&next_head, "reputation journal head")?;
    if state_transaction
        .world
        .smart_contract_state
        .get(&event_key(sequence))
        .is_some()
        || state_transaction
            .world
            .smart_contract_state
            .get(&event_id_key(entry.event_id))
            .is_some()
    {
        return Err(corrupt_state(
            "reputation append would overwrite an authoritative event index",
        ));
    }

    state_transaction
        .world
        .smart_contract_state
        .insert(event_key(sequence), event_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(event_id_key(entry.event_id), event_id_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(source_head_key(entry.source_id), source_head_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(journal_head_key().clone(), journal_head_bytes);

    let event = SorafsReputationJournalEntryCommittedV1 {
        sequence,
        event_id: entry.event_id,
        source_id: entry.source_id,
        source_kind: entry.source_kind(),
        source_revision: entry.source_revision,
        provider_id: entry.provider_id,
        policy_digest: entry.authority_policy_digest,
        authority: entry.recorded_by,
        source_time_unix_ms: entry.source_time_unix_ms,
        recorded_at_unix_ms,
    };
    state_transaction
        .world
        .emit_events(Some(SorafsGatewayEvent::ReputationJournal(
            SorafsReputationJournalEvent::EntryCommitted(event),
        )));
    Ok(sequence)
}

fn validate_new_entry(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    entry: &ReputationJournalEntryV1,
    expected_kind: ReputationJournalSourceKindV1,
) -> Result<(), InstructionExecutionError> {
    let now = block_time_ms(state_transaction)?;
    if entry.source_kind() != expected_kind {
        return Err(invalid_parameter(format!(
            "reputation instruction accepts only {expected_kind:?} entries"
        )));
    }
    validate_entry_commit_context(state_transaction.world(), entry, now)?;
    if &entry.recorded_by != authority {
        return Err(invalid_parameter(
            "reputation entry recorded_by must equal the transaction authority",
        ));
    }
    validate_provider_binding(state_transaction.world(), entry.provider_id)
}

fn execute_standalone_append(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    entry: ReputationJournalEntryV1,
    expected_kind: ReputationJournalSourceKindV1,
) -> Result<(), InstructionExecutionError> {
    require_permission(state_transaction, authority, CAN_RECORD_ENTRY)?;
    entry.validate().map_err(|error| {
        invalid_parameter(format!("invalid canonical reputation entry: {error}"))
    })?;
    if entry.source_kind() != expected_kind {
        return Err(invalid_parameter(format!(
            "reputation instruction accepts only {expected_kind:?} entries"
        )));
    }
    if exact_entry_replay(state_transaction.world(), &entry)?.is_some() {
        if &entry.recorded_by != authority {
            return Err(invalid_parameter(
                "idempotent reputation replay authority differs from recorded_by",
            ));
        }
        return Ok(());
    }
    validate_new_entry(state_transaction, authority, &entry, expected_kind)?;
    append_validated_entry(state_transaction, entry)?;
    Ok(())
}

fn provider_dispute_kind(kind: u8) -> Result<ProviderDisputeKindV1, InstructionExecutionError> {
    match kind {
        1 => Ok(ProviderDisputeKindV1::ReplicationShortfall),
        2 => Ok(ProviderDisputeKindV1::UptimeBreach),
        3 => Ok(ProviderDisputeKindV1::ProofFailure),
        4 => Ok(ProviderDisputeKindV1::FeeDispute),
        255 => Ok(ProviderDisputeKindV1::Other),
        _ => Err(invalid_parameter(format!(
            "capacity dispute kind {kind} is outside the canonical V1 vocabulary"
        ))),
    }
}

fn opened_dispute_entry(
    world: &impl WorldReadOnly,
    record: &CapacityDisputeRecord,
) -> Result<Option<ReputationJournalEntryV1>, InstructionExecutionError> {
    let source_id = ReputationJournalSourceIdV1::for_provider_dispute(record.dispute_id);
    let Some(source_head) = read_source_head(world, source_id)? else {
        return Ok(None);
    };
    let latest = read_event(world, source_head.sequence)?
        .ok_or_else(|| corrupt_state("provider-dispute source head points to a missing event"))?;
    let opened = if latest.entry.source_revision == 1 {
        latest
    } else if latest.entry.source_revision == 2 {
        let predecessor = latest.entry.predecessor_event_id.ok_or_else(|| {
            corrupt_state("resolved provider dispute has no predecessor event id")
        })?;
        let sequence = read_event_id_sequence(world, predecessor)?.ok_or_else(|| {
            corrupt_state("resolved provider dispute predecessor has no event-id index")
        })?;
        read_event(world, sequence)?.ok_or_else(|| {
            corrupt_state("resolved provider dispute predecessor event is missing")
        })?
    } else {
        return Err(corrupt_state(
            "provider-dispute source head has an unsupported revision",
        ));
    };
    validate_event_indexes(world, &opened)?;
    let ReputationJournalPayloadV1::ProviderDispute(dispute) = &opened.entry.payload else {
        return Err(corrupt_state(
            "provider-dispute source head points to another source family",
        ));
    };
    if opened.entry.source_revision != 1
        || opened.entry.provider_id != record.provider_id
        || dispute.dispute_id != record.dispute_id
        || dispute.kind != provider_dispute_kind(record.kind)?
        || dispute.evidence_digest != record.evidence.digest
        || dispute.submitted_at_unix_ms / 1_000 != record.submitted_epoch
        || !matches!(&dispute.status, ProviderDisputeStatusV1::Opened)
    {
        return Err(corrupt_state(
            "capacity dispute differs from its authoritative opened journal entry",
        ));
    }
    match (&record.status, source_head.source_revision) {
        (CapacityDisputeStatus::Pending, 1) => {}
        (CapacityDisputeStatus::Resolved(resolution), 2) => {
            let terminal = read_event(world, source_head.sequence)?.ok_or_else(|| {
                corrupt_state("resolved capacity dispute is missing its terminal journal event")
            })?;
            validate_event_indexes(world, &terminal)?;
            let ReputationJournalPayloadV1::ProviderDispute(terminal_payload) =
                &terminal.entry.payload
            else {
                return Err(corrupt_state(
                    "capacity-dispute terminal source has another payload family",
                ));
            };
            if terminal_payload.dispute_id != dispute.dispute_id
                || terminal_payload.kind != dispute.kind
                || terminal_payload.evidence_digest != dispute.evidence_digest
                || terminal_payload.submitted_at_unix_ms != dispute.submitted_at_unix_ms
            {
                return Err(corrupt_state(
                    "capacity-dispute terminal revision changes immutable opened material",
                ));
            }
            let ProviderDisputeStatusV1::Resolved(journal_resolution) = &terminal_payload.status
            else {
                return Err(corrupt_state(
                    "capacity-dispute terminal revision is not resolved",
                ));
            };
            if journal_resolution.outcome != resolution.outcome
                || journal_resolution.resolved_at_unix_ms / 1_000 != resolution.resolved_epoch
                || journal_resolution.rationale != resolution.notes
            {
                return Err(corrupt_state(
                    "capacity-dispute terminal journal differs from authoritative lifecycle state",
                ));
            }
        }
        _ => {
            return Err(corrupt_state(
                "capacity-dispute lifecycle and reputation source revision disagree",
            ));
        }
    }
    Ok(Some(opened.entry))
}

/// Validate an exact `RegisterCapacityDispute` replay against the already
/// committed journal lifecycle without manufacturing missing authoritative
/// state.
pub(super) fn validate_capacity_dispute_opened_replay(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    record: &CapacityDisputeRecord,
) -> Result<(), InstructionExecutionError> {
    validate_journal_head(state_transaction.world())?;
    let opened = opened_dispute_entry(state_transaction.world(), record)?.ok_or_else(|| {
        corrupt_state(
            "existing capacity dispute is missing its authoritative opened journal revision",
        )
    })?;
    if &opened.recorded_by != authority {
        return Err(invalid_parameter(
            "idempotent capacity-dispute replay authority differs from the committed recorder",
        ));
    }
    Ok(())
}

/// Append the `Opened` reputation revision atomically with a new authoritative
/// `RegisterCapacityDispute` record.
pub(super) fn append_capacity_dispute_opened(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    record: &CapacityDisputeRecord,
) -> Result<(), InstructionExecutionError> {
    validate_journal_head(state_transaction.world())?;
    if opened_dispute_entry(state_transaction.world(), record)?.is_some() {
        return Err(corrupt_state(
            "capacity-dispute journal source exists before authoritative record insertion",
        ));
    }
    if !matches!(&record.status, CapacityDisputeStatus::Pending) {
        return Err(corrupt_state(
            "terminal capacity dispute is missing its authoritative opened journal revision",
        ));
    }
    let now = block_time_ms(state_transaction)?;
    if record.submitted_epoch != now / 1_000 {
        return Err(invalid_parameter(
            "capacity-dispute submitted_epoch must identify the exact committing block second",
        ));
    }
    let policy = read_active_policy(state_transaction.world())?
        .ok_or_else(|| invalid_parameter("SoraFS reputation recorder policy is not configured"))?;
    if &policy.policy.dispute_recorder_authority != authority {
        return Err(invalid_parameter(
            "capacity dispute transaction authority is not the governed dispute recorder",
        ));
    }
    validate_provider_binding(state_transaction.world(), record.provider_id)?;
    let entry = ReputationJournalEntryV1::try_new(
        record.provider_id,
        policy.policy_digest,
        authority.clone(),
        now,
        None,
        ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
            dispute_id: record.dispute_id,
            kind: provider_dispute_kind(record.kind)?,
            evidence_digest: record.evidence.digest,
            submitted_at_unix_ms: now,
            status: ProviderDisputeStatusV1::Opened,
        }),
    )
    .map_err(|error| {
        invalid_parameter(format!(
            "capacity dispute cannot form a canonical reputation entry: {error}"
        ))
    })?;
    append_validated_entry(state_transaction, entry)?;
    Ok(())
}

fn resolved_dispute_replay_matches(
    world: &impl WorldReadOnly,
    opened: &ReputationJournalEntryV1,
    instruction: &ResolveSorafsCapacityDispute,
    authority: &AccountId,
) -> Result<bool, InstructionExecutionError> {
    let source_head = read_source_head(world, opened.source_id)?
        .ok_or_else(|| corrupt_state("opened capacity dispute is missing its source head"))?;
    if source_head.source_revision == 1 {
        return Ok(false);
    }
    if source_head.source_revision != 2 {
        return Err(corrupt_state(
            "capacity-dispute reputation source exceeded its terminal revision",
        ));
    }
    let terminal = read_event(world, source_head.sequence)?.ok_or_else(|| {
        corrupt_state("resolved capacity-dispute source head points to a missing event")
    })?;
    validate_event_indexes(world, &terminal)?;
    let ReputationJournalPayloadV1::ProviderDispute(dispute) = &terminal.entry.payload else {
        return Err(corrupt_state(
            "resolved capacity-dispute source points to another source family",
        ));
    };
    let ProviderDisputeStatusV1::Resolved(resolution) = &dispute.status else {
        return Err(corrupt_state(
            "capacity-dispute revision two is not terminal",
        ));
    };
    Ok(terminal.entry.predecessor_event_id == Some(opened.event_id)
        && &terminal.entry.recorded_by == authority
        && terminal.entry.authority_policy_digest == instruction.expected_authority_policy_digest
        && resolution.outcome == instruction.outcome
        && resolution.decision_digest == instruction.decision_digest
        && resolution.rationale == instruction.rationale)
}

impl Execute for SetSorafsReputationJournalAuthorityPolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_permission(state_transaction, authority, CAN_MANAGE_POLICY)?;
        validate_journal_head(state_transaction.world())?;
        self.policy.validate().map_err(|error| {
            invalid_parameter(format!("invalid reputation recorder policy: {error}"))
        })?;
        let policy_digest = self.policy.canonical_digest().map_err(|error| {
            invalid_parameter(format!("invalid reputation recorder policy: {error}"))
        })?;
        let current = read_active_policy(state_transaction.world())?;
        if let Some(existing) = read_policy_history(state_transaction.world(), &policy_digest)? {
            validate_policy_predecessor(state_transaction.world(), &existing)?;
            // The first commit owns the activation timestamp. A later exact
            // policy/authority replay must not manufacture another revision.
            if existing.policy == self.policy && &existing.activated_by == authority {
                return Ok(());
            }
            return Err(corrupt_state(
                "reputation recorder-policy history aliases different activation content",
            ));
        }

        for (label, recorder) in [
            ("PoR", &self.policy.por_recorder_authority),
            ("capacity-dispute", &self.policy.dispute_recorder_authority),
            ("stream-token", &self.policy.token_recorder_authority),
        ] {
            if state_transaction.world.accounts.get(recorder).is_none() {
                return Err(invalid_parameter(format!(
                    "{label} reputation recorder is not a registered account"
                )));
            }
        }
        let now = block_time_ms(state_transaction)?;
        let candidate =
            ReputationJournalAuthorityPolicyRecordV1::try_new(self.policy, authority.clone(), now)
                .map_err(|error| {
                    invalid_parameter(format!(
                        "invalid reputation recorder-policy activation: {error}"
                    ))
                })?;
        if candidate.policy_digest != policy_digest {
            return Err(corrupt_state(
                "reputation recorder policy digest changed during canonical activation",
            ));
        }

        match current {
            None => {
                if candidate.policy.revision != 1
                    || candidate.policy.predecessor_policy_digest.is_some()
                {
                    return Err(invalid_parameter(
                        "first reputation recorder policy must be revision one without a predecessor",
                    ));
                }
            }
            Some(current) => {
                let expected_revision = current
                    .policy
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("reputation policy revision overflow"))?;
                if candidate.policy.revision != expected_revision {
                    return Err(invalid_parameter(format!(
                        "reputation policy revision {} must exactly follow active revision {}",
                        candidate.policy.revision, current.policy.revision
                    )));
                }
                if candidate.policy.predecessor_policy_digest != Some(current.policy_digest) {
                    return Err(invalid_parameter(
                        "reputation policy predecessor does not match the active policy digest",
                    ));
                }
            }
        }
        validate_policy_predecessor(state_transaction.world(), &candidate)?;
        let encoded = encode_state(&candidate, "reputation recorder-policy activation")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(active_policy_key().clone(), encoded.clone());
        state_transaction
            .world
            .smart_contract_state
            .insert(policy_history_key(&candidate.policy_digest), encoded);
        state_transaction
            .world
            .emit_events(Some(SorafsGatewayEvent::ReputationJournal(
                SorafsReputationJournalEvent::PolicyActivated(
                    SorafsReputationJournalPolicyActivatedV1 {
                        policy_digest: candidate.policy_digest,
                        revision: candidate.policy.revision,
                        authority: authority.clone(),
                        occurred_at_unix_ms: now,
                    },
                ),
            )));
        Ok(())
    }
}

impl Execute for AppendSorafsPorReputationJournalEntry {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        execute_standalone_append(
            state_transaction,
            authority,
            self.entry,
            ReputationJournalSourceKindV1::Por,
        )
    }
}

impl Execute for AppendSorafsStreamTokenReputationJournalEntry {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        execute_standalone_append(
            state_transaction,
            authority,
            self.entry,
            ReputationJournalSourceKindV1::StreamToken,
        )
    }
}

impl Execute for ResolveSorafsCapacityDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_permission(state_transaction, authority, CAN_RESOLVE_DISPUTE)?;
        validate_journal_head(state_transaction.world())?;
        if self.decision_digest == [0; 32] {
            return Err(invalid_parameter(
                "capacity-dispute decision digest must be non-zero",
            ));
        }
        let record = state_transaction
            .world
            .capacity_disputes
            .get(&self.dispute_id)
            .cloned()
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "capacity dispute {} is not registered",
                    hex::encode(self.dispute_id.as_bytes())
                ))
            })?;
        let opened =
            opened_dispute_entry(state_transaction.world(), &record)?.ok_or_else(|| {
                corrupt_state("capacity dispute has no authoritative opened reputation entry")
            })?;

        if matches!(&record.status, CapacityDisputeStatus::Resolved(_)) {
            if resolved_dispute_replay_matches(
                state_transaction.world(),
                &opened,
                &self,
                authority,
            )? {
                return Ok(());
            }
            return Err(invalid_parameter(
                "capacity dispute already has a different terminal decision",
            ));
        }

        let now = block_time_ms(state_transaction)?;
        let policy = read_active_policy(state_transaction.world())?.ok_or_else(|| {
            invalid_parameter("SoraFS reputation recorder policy is not configured")
        })?;
        if self.expected_authority_policy_digest != policy.policy_digest {
            return Err(invalid_parameter(
                "capacity-dispute resolution expected policy digest is stale",
            ));
        }
        if &policy.policy.dispute_recorder_authority != authority {
            return Err(invalid_parameter(
                "capacity-dispute resolution authority is not the governed dispute recorder",
            ));
        }
        let ReputationJournalPayloadV1::ProviderDispute(opened_payload) = &opened.payload else {
            return Err(corrupt_state(
                "capacity dispute opened entry has another source payload",
            ));
        };
        let terminal_entry = ReputationJournalEntryV1::try_new(
            record.provider_id,
            policy.policy_digest,
            authority.clone(),
            now,
            Some(opened.event_id),
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: record.dispute_id,
                kind: opened_payload.kind,
                evidence_digest: opened_payload.evidence_digest,
                submitted_at_unix_ms: opened_payload.submitted_at_unix_ms,
                status: ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
                    outcome: self.outcome,
                    resolved_at_unix_ms: now,
                    decision_digest: self.decision_digest,
                    rationale: self.rationale.clone(),
                }),
            }),
        )
        .map_err(|error| {
            invalid_parameter(format!(
                "invalid capacity-dispute terminal reputation entry: {error}"
            ))
        })?;
        validate_new_entry(
            state_transaction,
            authority,
            &terminal_entry,
            ReputationJournalSourceKindV1::ProviderDispute,
        )?;

        let resolved_epoch = now / 1_000;
        if resolved_epoch == 0 {
            return Err(invalid_parameter(
                "capacity-dispute resolution epoch must be non-zero",
            ));
        }
        let mut updated = record;
        updated.status = CapacityDisputeStatus::Resolved(CapacityDisputeResolution {
            resolved_epoch,
            outcome: self.outcome,
            notes: self.rationale,
        });

        append_validated_entry(state_transaction, terminal_entry)?;
        state_transaction
            .world
            .capacity_disputes
            .insert(self.dispute_id, updated);
        Ok(())
    }
}

fn checked_query_limit(limit: u32) -> Result<usize, QueryExecutionFail> {
    let limit = usize::try_from(limit).map_err(|_| {
        QueryExecutionFail::Conversion(
            "reputation journal query limit does not fit into usize".to_owned(),
        )
    })?;
    if limit == 0 || limit > REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "reputation journal query limit must be within 1..={REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1}"
        )));
    }
    Ok(limit)
}

fn finalized_cursor(
    state_ro: &impl StateReadOnly,
) -> Result<ReputationJournalFinalizedCursorV1, QueryExecutionFail> {
    let height = u64::try_from(state_ro.block_hashes().len()).map_err(|_| {
        QueryExecutionFail::Conversion(
            "finalized reputation height does not fit into u64".to_owned(),
        )
    })?;
    let block = state_ro.latest_block().ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "reputation journal queries require the exact latest finalized Kura block".to_owned(),
        )
    })?;
    let block_hash = *block.hash().as_ref();
    let finalized_at_unix_ms = block.header().creation_time_ms;
    if height == 0
        || block.header().height().get() != height
        || block_hash == [0; 32]
        || finalized_at_unix_ms == 0
        || finalized_at_unix_ms == u64::MAX
        || state_ro.latest_block_hash().map(|hash| *hash.as_ref()) != Some(block_hash)
    {
        return Err(QueryExecutionFail::Conversion(
            "finalized reputation Kura block does not match the immutable state anchor".to_owned(),
        ));
    }
    Ok(ReputationJournalFinalizedCursorV1 {
        height,
        block_hash,
        finalized_at_unix_ms,
    })
}

fn resolve_finalized_event(
    state_ro: &impl StateReadOnly,
    record: &ReputationJournalCommittedEventRecordV1,
) -> Result<ReputationJournalFinalizedEventV1, QueryExecutionFail> {
    let index = record
        .target_block_height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reputation event target height cannot index finalized hashes".to_owned(),
            )
        })?;
    let block_hash = state_ro
        .block_hashes()
        .get(index)
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(format!(
                "reputation event {} targets non-finalized height {}",
                record.sequence, record.target_block_height
            ))
        })?;
    if block_hash == [0; 32] {
        return Err(QueryExecutionFail::Conversion(format!(
            "reputation event {} resolves a zero block hash",
            record.sequence
        )));
    }
    Ok(ReputationJournalFinalizedEventV1 {
        sequence: record.sequence,
        block_height: record.target_block_height,
        block_hash,
        event_index: record.event_index,
        recorded_at_unix_ms: record.recorded_at_unix_ms,
        entry: record.entry.clone(),
    })
}

fn load_cursor_event(
    state_ro: &impl StateReadOnly,
    cursor: ReputationJournalFinalizedEventCursorV1,
) -> Result<ReputationJournalCommittedEventRecordV1, QueryExecutionFail> {
    cursor
        .validate()
        .map_err(|error| QueryExecutionFail::Conversion(error.to_string()))?;
    let record = read_event(state_ro.world(), cursor.sequence)
        .map_err(query_failure)?
        .ok_or(QueryExecutionFail::Expired)?;
    let predecessor = if cursor.sequence == 1 {
        None
    } else {
        let sequence = cursor.sequence - 1;
        let predecessor = read_event(state_ro.world(), sequence)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(format!(
                    "reputation journal is missing cursor predecessor sequence {sequence}"
                ))
            })?;
        validate_event_indexes(state_ro.world(), &predecessor).map_err(query_failure)?;
        Some(predecessor)
    };
    validate_event_successor(predecessor.as_ref(), &record).map_err(query_failure)?;
    validate_event_indexes(state_ro.world(), &record).map_err(query_failure)?;
    if resolve_finalized_event(state_ro, &record)?.cursor() != cursor {
        return Err(QueryExecutionFail::Expired);
    }
    Ok(record)
}

fn query_event_page(
    query: &FindSorafsReputationJournalEvents,
    state_ro: &impl StateReadOnly,
) -> Result<ReputationJournalFinalizedEventPageV1, QueryExecutionFail> {
    let limit = checked_query_limit(query.limit)?;
    let finalized_cursor = finalized_cursor(state_ro)?;
    if query
        .expected_finalized_cursor
        .is_some_and(|expected| expected != finalized_cursor)
    {
        return Err(QueryExecutionFail::Expired);
    }

    let head = validate_journal_head(state_ro.world()).map_err(query_failure)?;
    let Some(head) = head else {
        if query.after.is_some() {
            return Err(QueryExecutionFail::Expired);
        }
        let page = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor,
            events: Vec::new(),
            has_more: false,
            next_after: None,
        };
        page.validate_after(query.after)
            .map_err(|error| QueryExecutionFail::Conversion(error.to_string()))?;
        return Ok(page);
    };
    let head_record = read_event(state_ro.world(), head.last_sequence)
        .map_err(query_failure)?
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reputation journal head points to a missing event".to_owned(),
            )
        })?;
    if head_record.target_block_height != head.last_target_block_height
        || head_record.event_index != head.last_event_index
    {
        return Err(QueryExecutionFail::Conversion(
            "reputation journal head differs from its terminal event".to_owned(),
        ));
    }
    validate_event_indexes(state_ro.world(), &head_record).map_err(query_failure)?;

    let mut previous = query
        .after
        .map(|cursor| load_cursor_event(state_ro, cursor))
        .transpose()?;
    let start_sequence = query.after.map_or(Ok(1), |cursor| {
        cursor.sequence.checked_add(1).ok_or_else(|| {
            QueryExecutionFail::Conversion("reputation query cursor overflow".to_owned())
        })
    })?;
    if start_sequence > head.last_sequence.saturating_add(1) {
        return Err(QueryExecutionFail::Expired);
    }

    let mut events = Vec::with_capacity(limit);
    let payload_budget = REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1.saturating_sub(4 * 1_024);
    let mut encoded_event_bytes = 0usize;
    let mut sequence = start_sequence;
    while sequence <= head.last_sequence && events.len() < limit {
        let record = read_event(state_ro.world(), sequence)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(format!(
                    "reputation journal is missing sequence {sequence}"
                ))
            })?;
        validate_event_successor(previous.as_ref(), &record).map_err(query_failure)?;
        validate_event_indexes(state_ro.world(), &record).map_err(query_failure)?;
        let resolved = resolve_finalized_event(state_ro, &record)?;
        let resolved_bytes = norito::to_bytes(&resolved)
            .map_err(|error| {
                QueryExecutionFail::Conversion(format!(
                    "failed to encode finalized reputation event: {error}"
                ))
            })?
            .len();
        let next_event_bytes =
            encoded_event_bytes
                .checked_add(resolved_bytes)
                .ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "reputation event-page byte counter overflow".to_owned(),
                    )
                })?;
        if next_event_bytes > payload_budget {
            if events.is_empty() {
                return Err(QueryExecutionFail::Conversion(format!(
                    "one reputation event cannot fit within the {REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1}-byte page budget"
                )));
            }
            break;
        }
        encoded_event_bytes = next_event_bytes;
        events.push(resolved);
        previous = Some(record);
        sequence = sequence.checked_add(1).ok_or_else(|| {
            QueryExecutionFail::Conversion("reputation journal sequence overflow".to_owned())
        })?;
    }
    let has_more = sequence <= head.last_sequence;
    let next_after = has_more.then(|| {
        events
            .last()
            .expect("a continuing reputation page must contain an event")
            .cursor()
    });
    let page = ReputationJournalFinalizedEventPageV1 {
        finalized_cursor,
        events,
        has_more,
        next_after,
    };
    page.validate_after(query.after)
        .map_err(|error| QueryExecutionFail::Conversion(error.to_string()))?;
    let encoded_len = norito::to_bytes(&page)
        .map_err(|error| {
            QueryExecutionFail::Conversion(format!(
                "failed to encode reputation event page: {error}"
            ))
        })?
        .len();
    if encoded_len > REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "reputation event page has {encoded_len} bytes; maximum is {REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1}"
        )));
    }
    Ok(page)
}

impl ValidSingularQuery for FindSorafsReputationJournalAuthorityPolicy {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<ReputationJournalAuthorityPolicyRecordV1, QueryExecutionFail> {
        read_active_policy(state_ro.world())
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsReputationJournalAuthorityPolicy)
            })
    }
}

impl ValidSingularQuery for FindSorafsReputationJournalEvents {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<ReputationJournalFinalizedEventPageV1, QueryExecutionFail> {
        query_event_page(self, state_ro)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, SignatureOf};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        account::Account,
        block::{BlockHeader, BlockSignature, SignedBlock},
        events::data::DataEvent,
        metadata::Metadata,
        permission::Permissions,
        sorafs::{
            capacity::CapacityDeclarationRecord,
            reputation::{
                PorTerminalOutcomeV1, PorTerminalStatusV1,
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1, ReputationJournalAuthorityPolicyV1,
                StreamTokenValidationOutcomeV1, StreamTokenValidationStatusV1,
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanManageSorafsReputationJournalPolicy, CanRecordSorafsReputationJournal,
        CanResolveSorafsCapacityDispute,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const TEST_NOW_MS: u64 = 1_700_000_000_000;

    fn keypair(seed: u8) -> KeyPair {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("valid deterministic test key");
        KeyPair::from_private_key(private).expect("derive deterministic test keypair")
    }

    fn account(keypair: &KeyPair) -> AccountId {
        AccountId::new(keypair.public_key().clone())
    }

    fn policy(authority: &AccountId) -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority.clone(),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }

    fn token_entry(
        authority: &AccountId,
        provider_id: ProviderId,
        policy_digest: [u8; 32],
        unique: u8,
    ) -> ReputationJournalEntryV1 {
        token_entry_at(authority, provider_id, policy_digest, unique, TEST_NOW_MS)
    }

    fn token_entry_at(
        authority: &AccountId,
        provider_id: ProviderId,
        policy_digest: [u8; 32],
        unique: u8,
        source_time_unix_ms: u64,
    ) -> ReputationJournalEntryV1 {
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy_digest,
            authority.clone(),
            source_time_unix_ms,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(StreamTokenValidationOutcomeV1 {
                binding: iroha_data_model::sorafs::reputation::StreamTokenValidationBindingV1 {
                    gateway_id: [unique; 32],
                    gateway_sequence: 1,
                    request_context_digest: [unique.wrapping_add(1); 32],
                },
                token_body_digest: Some([unique.wrapping_add(2); 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: source_time_unix_ms,
                status: StreamTokenValidationStatusV1::Accepted,
            }),
        )
        .expect("canonical token reputation entry")
    }

    fn por_entry_at(
        authority: &AccountId,
        provider_id: ProviderId,
        policy_digest: [u8; 32],
        unique: u8,
        source_time_unix_ms: u64,
    ) -> ReputationJournalEntryV1 {
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy_digest,
            authority.clone(),
            source_time_unix_ms,
            None,
            ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
                challenge_id: [unique; 32],
                manifest_digest: [0x41; 32],
                epoch_id: 7,
                drand_round: 11,
                forced: false,
                sample_count: 4,
                failed_samples: 0,
                issued_at_unix_ms: source_time_unix_ms - 2_000,
                deadline_at_unix_ms: source_time_unix_ms - 500,
                responded_at_unix_ms: Some(source_time_unix_ms - 750),
                decided_at_unix_ms: source_time_unix_ms,
                proof_digest: Some([0x42; 32]),
                repair_task_id: None,
                verifier_latency_ms: Some(17),
                status: PorTerminalStatusV1::Verified,
            }),
        )
        .expect("canonical PoR reputation entry")
    }

    fn state_with_reputation_accounts() -> (State, AccountId, AccountId, ProviderId) {
        let authority = account(&keypair(1));
        let other = account(&keypair(2));
        let provider_id = ProviderId::new([0x31; 32]);
        let mut world = World::new();
        for account_id in [&authority, &other] {
            let (id, value) = Account::new(account_id.clone())
                .build(&authority)
                .into_key_value();
            world.accounts.insert(id, value);
        }
        let mut authority_permissions = Permissions::new();
        authority_permissions.insert(Permission::from(CanManageSorafsReputationJournalPolicy));
        authority_permissions.insert(Permission::from(CanRecordSorafsReputationJournal));
        authority_permissions.insert(Permission::from(CanResolveSorafsCapacityDispute));
        world
            .account_permissions
            .insert(authority.clone(), authority_permissions);
        let mut other_permissions = Permissions::new();
        other_permissions.insert(Permission::from(CanRecordSorafsReputationJournal));
        world
            .account_permissions
            .insert(other.clone(), other_permissions);
        world.provider_owners.insert(provider_id, authority.clone());
        world.capacity_declarations.insert(
            provider_id,
            CapacityDeclarationRecord::new(
                provider_id,
                vec![0x01],
                1,
                1,
                1,
                2,
                Metadata::default(),
            ),
        );
        (
            State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            ),
            authority,
            other,
            provider_id,
        )
    }

    fn transact_test(
        state: &mut State,
        height: u64,
        timestamp_ms: u64,
        operation: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<(), InstructionExecutionError>,
    ) -> Result<(), InstructionExecutionError> {
        let header = BlockHeader::new(
            height.try_into().expect("nonzero height"),
            None,
            None,
            None,
            timestamp_ms,
            0,
        );
        let mut block = state.block(header.clone());
        let mut transaction = block.transaction();
        operation(&mut transaction)?;
        transaction.apply();
        block.commit().expect("commit reputation test block");
        let block_signer = keypair(0xFE);
        let signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(block_signer.private_key(), header.hash())
                .expect("sign reputation Kura fixture block"),
        );
        let signed_block = SignedBlock::presigned(signature, header, Vec::new());
        let block_hash = signed_block.hash();
        state
            .kura()
            .store_block(Arc::new(signed_block))
            .expect("store reputation Kura fixture block");
        state.push_block_hash_for_testing(block_hash);
        Ok(())
    }

    #[test]
    fn event_keys_preserve_global_sequence_order() {
        assert!(event_key(1) < event_key(2));
        assert!(event_key(u64::MAX - 1) < event_key(u64::MAX));
    }

    #[test]
    fn capacity_dispute_kind_mapping_is_closed() {
        assert_eq!(
            provider_dispute_kind(1).expect("known kind"),
            ProviderDisputeKindV1::ReplicationShortfall
        );
        assert_eq!(
            provider_dispute_kind(255).expect("known kind"),
            ProviderDisputeKindV1::Other
        );
        assert!(provider_dispute_kind(0).is_err());
        assert!(provider_dispute_kind(5).is_err());
    }

    #[test]
    fn authority_policy_query_returns_precise_absence_error() {
        let (state, _authority, _other, _provider_id) = state_with_reputation_accounts();
        let view = state.view();
        assert_eq!(
            FindSorafsReputationJournalAuthorityPolicy.execute(&view),
            Err(QueryExecutionFail::Find(
                FindError::SorafsReputationJournalAuthorityPolicy
            ))
        );
    }

    #[test]
    fn recorder_policy_rotation_is_strict_and_historical_replay_is_idempotent() {
        let (state, authority, _other, _provider_id) = state_with_reputation_accounts();
        let header = BlockHeader::new(
            1_u64.try_into().expect("nonzero height"),
            None,
            None,
            None,
            TEST_NOW_MS,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let first = policy(&authority);
        let first_digest = first.canonical_digest().expect("first policy digest");
        SetSorafsReputationJournalAuthorityPolicy::new(first.clone())
            .execute(&authority, &mut transaction)
            .expect("activate first recorder policy");
        let first_record = FindSorafsReputationJournalAuthorityPolicy
            .execute(&transaction)
            .expect("query first active recorder policy");
        assert_eq!(first_record.policy, first);
        assert_eq!(first_record.policy_digest, first_digest);
        assert_eq!(first_record.activated_by, authority);
        assert_eq!(first_record.activated_at_unix_ms, TEST_NOW_MS);

        let mut second = first.clone();
        second.revision = 2;
        second.predecessor_policy_digest = Some(first_digest);
        let second_digest = second.canonical_digest().expect("second policy digest");
        SetSorafsReputationJournalAuthorityPolicy::new(second)
            .execute(&authority, &mut transaction)
            .expect("activate exact successor policy");
        let second_record = FindSorafsReputationJournalAuthorityPolicy
            .execute(&transaction)
            .expect("query rotated active recorder policy");
        assert_eq!(second_record.policy.revision, 2);
        assert_eq!(second_record.policy_digest, second_digest);
        assert_eq!(
            second_record.policy.predecessor_policy_digest,
            Some(first_digest)
        );
        assert_eq!(
            read_reputation_authority_policy_history(transaction.world(), 2)
                .expect("read exact bounded policy history"),
            vec![first_record.clone(), second_record.clone()]
        );
        assert!(
            read_reputation_authority_policy_history(transaction.world(), 1).is_err(),
            "an undersized immutable-history bound must fail closed"
        );

        SetSorafsReputationJournalAuthorityPolicy::new(first)
            .execute(&authority, &mut transaction)
            .expect("historical exact replay is idempotent");
        let mut fork = policy(&authority);
        fork.revision = 3;
        fork.predecessor_policy_digest = Some([0x99; 32]);
        SetSorafsReputationJournalAuthorityPolicy::new(fork)
            .execute(&authority, &mut transaction)
            .expect_err("policy fork must fail closed");
        assert_eq!(
            read_active_policy(transaction.world())
                .expect("read active policy after rejected fork")
                .expect("active policy")
                .policy_digest,
            second_digest
        );
    }

    #[test]
    fn por_append_uses_source_time_policy_across_rotation_and_replay() {
        let (mut state, authority, _other, provider_id) = state_with_reputation_accounts();
        let first = policy(&authority);
        let first_digest = first.canonical_digest().expect("first policy digest");
        transact_test(&mut state, 1, TEST_NOW_MS, |transaction| {
            SetSorafsReputationJournalAuthorityPolicy::new(first.clone())
                .execute(&authority, transaction)
        })
        .expect("activate first recorder policy");

        let queued_before_rotation = por_entry_at(
            &authority,
            provider_id,
            first_digest,
            0x61,
            TEST_NOW_MS + 100,
        );
        let mut successor = first;
        successor.revision = 2;
        successor.predecessor_policy_digest = Some(first_digest);
        let successor_digest = successor
            .canonical_digest()
            .expect("successor policy digest");
        transact_test(&mut state, 2, TEST_NOW_MS + 200, |transaction| {
            SetSorafsReputationJournalAuthorityPolicy::new(successor.clone())
                .execute(&authority, transaction)
        })
        .expect("rotate recorder policy");

        transact_test(&mut state, 3, TEST_NOW_MS + 300, |transaction| {
            AppendSorafsPorReputationJournalEntry::new(queued_before_rotation.clone())
                .execute(&authority, transaction)?;
            AppendSorafsPorReputationJournalEntry::new(queued_before_rotation.clone())
                .execute(&authority, transaction)?;
            assert_eq!(
                read_journal_head(transaction.world())?
                    .ok_or_else(|| corrupt_state("missing PoR reputation journal head"))?
                    .last_sequence,
                1,
                "an exact crash replay must remain idempotent after policy rotation"
            );
            let retained = read_event(transaction.world(), 1)?
                .ok_or_else(|| corrupt_state("missing retained historical PoR entry"))?;
            validate_event_indexes(transaction.world(), &retained)?;
            Ok(())
        })
        .expect("commit source-time-valid queued PoR terminal");

        let superseded_at_boundary = por_entry_at(
            &authority,
            provider_id,
            first_digest,
            0x62,
            TEST_NOW_MS + 200,
        );
        let current = por_entry_at(
            &authority,
            provider_id,
            successor_digest,
            0x63,
            TEST_NOW_MS + 250,
        );
        transact_test(&mut state, 4, TEST_NOW_MS + 400, |transaction| {
            let error = AppendSorafsPorReputationJournalEntry::new(superseded_at_boundary)
                .execute(&authority, transaction)
                .expect_err("the successor activation boundary belongs to the successor");
            assert!(
                error
                    .to_string()
                    .contains("outside its recorder-policy activation interval")
            );
            AppendSorafsPorReputationJournalEntry::new(current).execute(&authority, transaction)?;
            assert_eq!(
                read_journal_head(transaction.world())?
                    .ok_or_else(|| corrupt_state("missing PoR reputation journal head"))?
                    .last_sequence,
                2
            );
            Ok(())
        })
        .expect("reject stale policy material and commit successor material atomically");
    }

    #[test]
    fn query_limit_rejects_zero_and_resource_bombs() {
        assert!(checked_query_limit(0).is_err());
        assert_eq!(checked_query_limit(1).expect("minimum"), 1);
        assert_eq!(
            checked_query_limit(
                u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1).expect("bound fits")
            )
            .expect("maximum"),
            REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1
        );
        assert!(
            checked_query_limit(
                u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1 + 1).expect("bound fits")
            )
            .is_err()
        );
    }

    #[test]
    fn journal_successor_rejects_gaps_reordering_and_bad_block_indexes() {
        let account = AccountId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let policy = iroha_data_model::sorafs::reputation::ReputationJournalAuthorityPolicyV1 {
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: account.clone(),
            dispute_recorder_authority: account.clone(),
            token_recorder_authority: account.clone(),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        };
        let digest = policy.canonical_digest().expect("policy digest");
        let payload = ReputationJournalPayloadV1::StreamTokenValidation(
            iroha_data_model::sorafs::reputation::StreamTokenValidationOutcomeV1 {
                binding: iroha_data_model::sorafs::reputation::StreamTokenValidationBindingV1 {
                    gateway_id: [0x11; 32],
                    gateway_sequence: 1,
                    request_context_digest: [0x22; 32],
                },
                token_body_digest: Some([0x33; 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: 1_000,
                status:
                    iroha_data_model::sorafs::reputation::StreamTokenValidationStatusV1::Accepted,
            },
        );
        let entry = ReputationJournalEntryV1::try_new(
            ProviderId::new([0x44; 32]),
            digest,
            account,
            1_000,
            None,
            payload,
        )
        .expect("canonical entry");
        let first = ReputationJournalCommittedEventRecordV1 {
            sequence: 1,
            target_block_height: 2,
            event_index: 0,
            recorded_at_unix_ms: 1_100,
            entry: entry.clone(),
        };
        assert!(validate_event_successor(None, &first).is_ok());

        let mut gap = first.clone();
        gap.sequence = 3;
        gap.target_block_height = 3;
        assert!(validate_event_successor(Some(&first), &gap).is_err());

        let mut bad_index = first.clone();
        bad_index.sequence = 2;
        bad_index.event_index = 2;
        assert!(validate_event_successor(Some(&first), &bad_index).is_err());

        let mut next_block = first.clone();
        next_block.sequence = 2;
        next_block.target_block_height = 3;
        next_block.event_index = 0;
        assert!(validate_event_successor(Some(&first), &next_block).is_ok());
    }

    #[test]
    fn governed_token_appends_are_contiguous_and_exact_replays_are_idempotent() {
        let (state, authority, other, provider_id) = state_with_reputation_accounts();
        let header = BlockHeader::new(
            1_u64.try_into().expect("nonzero height"),
            None,
            None,
            None,
            TEST_NOW_MS,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let initial_policy = policy(&authority);
        let policy_digest = initial_policy.canonical_digest().expect("policy digest");
        SetSorafsReputationJournalAuthorityPolicy::new(initial_policy)
            .execute(&authority, &mut transaction)
            .expect("activate policy");

        let first = token_entry(&authority, provider_id, policy_digest, 0x41);
        AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&authority, &mut transaction)
            .expect("append first token event");
        AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&authority, &mut transaction)
            .expect("exact replay is idempotent");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal head")
                .expect("journal head")
                .last_sequence,
            1
        );

        let replay_error = AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&other, &mut transaction)
            .expect_err("another authority cannot replay the event");
        assert!(replay_error.to_string().contains("replay authority"));

        let second = token_entry(&authority, provider_id, policy_digest, 0x51);
        AppendSorafsStreamTokenReputationJournalEntry::new(second)
            .execute(&authority, &mut transaction)
            .expect("append second token event");
        let head = read_journal_head(transaction.world())
            .expect("read journal head")
            .expect("journal head");
        assert_eq!(head.last_sequence, 2);
        assert_eq!(head.last_event_index, 1);
        let first_record = read_event(transaction.world(), 1)
            .expect("read first event")
            .expect("first event");
        let second_record = read_event(transaction.world(), 2)
            .expect("read second event")
            .expect("second event");
        validate_event_successor(Some(&first_record), &second_record)
            .expect("events are globally contiguous");

        transaction
            .world
            .smart_contract_state
            .remove(event_key(first_record.sequence));
        assert!(
            validate_journal_head(transaction.world()).is_err(),
            "a journal with no global sequence one must fail closed"
        );
        transaction.world.smart_contract_state.insert(
            event_key(first_record.sequence),
            encode_state(&first_record, "restored first reputation event")
                .expect("encode restored first event"),
        );
        let forged_tail_key = event_key(3);
        transaction
            .world
            .smart_contract_state
            .insert(forged_tail_key.clone(), vec![0xFF]);
        assert!(
            validate_journal_head(transaction.world()).is_err(),
            "an event-prefixed key beyond the journal head must fail closed"
        );
        transaction
            .world
            .smart_contract_state
            .remove(forged_tail_key);

        let wrong_policy_entry = token_entry(&authority, provider_id, [0x99; 32], 0x61);
        AppendSorafsStreamTokenReputationJournalEntry::new(wrong_policy_entry)
            .execute(&authority, &mut transaction)
            .expect_err("stale policy digest must fail");
        let wrong_source_family = token_entry(&authority, provider_id, policy_digest, 0x71);
        AppendSorafsPorReputationJournalEntry::new(wrong_source_family)
            .execute(&authority, &mut transaction)
            .expect_err("PoR append must reject a stream-token source");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal head")
                .expect("journal head")
                .last_sequence,
            2
        );

        let mut rotated_policy = policy(&authority);
        rotated_policy.revision = 2;
        rotated_policy.predecessor_policy_digest = Some(policy_digest);
        SetSorafsReputationJournalAuthorityPolicy::new(rotated_policy)
            .execute(&authority, &mut transaction)
            .expect("rotate recorder policy");
        AppendSorafsStreamTokenReputationJournalEntry::new(first)
            .execute(&authority, &mut transaction)
            .expect("exact historical entry replay remains idempotent after rotation");
        let stale_historical_entry = token_entry(&authority, provider_id, policy_digest, 0x72);
        AppendSorafsStreamTokenReputationJournalEntry::new(stale_historical_entry)
            .execute(&authority, &mut transaction)
            .expect_err("new entries cannot use a superseded recorder policy");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal after policy rotation")
                .expect("journal head")
                .last_sequence,
            2
        );

        let forged_cross_source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::StreamToken,
            source_revision: 2,
            event_id: second_record.entry.event_id,
            sequence: second_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(&forged_cross_source_head, "forged reputation source head")
                .expect("encode forged source head"),
        );
        assert!(
            validate_event_indexes(transaction.world(), &first_record).is_err(),
            "a source head must not recurse through an event from another source"
        );
        let restored_first_source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::StreamToken,
            source_revision: 1,
            event_id: first_record.entry.event_id,
            sequence: first_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(
                &restored_first_source_head,
                "restored reputation source head",
            )
            .expect("encode restored source head"),
        );

        transaction
            .world
            .smart_contract_state
            .remove(journal_head_key().clone());
        let orphan_replay = token_entry(&authority, provider_id, policy_digest, 0x41);
        let corruption = AppendSorafsStreamTokenReputationJournalEntry::new(orphan_replay)
            .execute(&authority, &mut transaction)
            .expect_err("an orphaned journal index must fail closed on exact replay");
        assert!(matches!(
            corruption,
            InstructionExecutionError::InvariantViolation(_)
        ));
    }

    #[test]
    fn asynchronous_source_time_is_bound_while_commit_time_is_authoritative() {
        let (mut state, authority, _other, provider_id) = state_with_reputation_accounts();
        let mut journal_policy = policy(&authority);
        journal_policy.max_source_age_ms = 1_500;
        let policy_digest = journal_policy
            .canonical_digest()
            .expect("canonical recorder policy");
        transact_test(&mut state, 1, TEST_NOW_MS, |transaction| {
            SetSorafsReputationJournalAuthorityPolicy::new(journal_policy)
                .execute(&authority, transaction)
        })
        .expect("activate recorder policy");

        let source_time_unix_ms = TEST_NOW_MS + 250;
        let recorded_at_unix_ms = TEST_NOW_MS + 1_000;
        let entry = token_entry_at(
            &authority,
            provider_id,
            policy_digest,
            0x81,
            source_time_unix_ms,
        );
        transact_test(&mut state, 2, recorded_at_unix_ms, |transaction| {
            AppendSorafsStreamTokenReputationJournalEntry::new(entry.clone())
                .execute(&authority, transaction)?;
            let record = read_event(transaction.world(), 1)?
                .ok_or_else(|| corrupt_state("missing asynchronous reputation event"))?;
            assert_eq!(record.entry.source_time_unix_ms, source_time_unix_ms);
            assert_eq!(record.recorded_at_unix_ms, recorded_at_unix_ms);
            let emitted = transaction
                .world
                .internal_event_buf
                .iter()
                .find_map(|event| match event.as_ref() {
                    DataEvent::Sorafs(SorafsGatewayEvent::ReputationJournal(
                        SorafsReputationJournalEvent::EntryCommitted(committed),
                    )) if committed.event_id == entry.event_id => Some(committed),
                    _ => None,
                })
                .expect("typed committed event");
            assert_eq!(emitted.source_time_unix_ms, source_time_unix_ms);
            assert_eq!(emitted.recorded_at_unix_ms, recorded_at_unix_ms);
            Ok(())
        })
        .expect("commit delayed but fresh source observation");

        let future = token_entry_at(
            &authority,
            provider_id,
            policy_digest,
            0x82,
            TEST_NOW_MS + 2_001,
        );
        transact_test(&mut state, 3, TEST_NOW_MS + 2_000, |transaction| {
            let error = AppendSorafsStreamTokenReputationJournalEntry::new(future)
                .execute(&authority, transaction)
                .expect_err("future source observation must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("after authoritative commit time")
            );
            Ok(())
        })
        .expect("commit block after rejecting future source observation");

        let stale = token_entry_at(&authority, provider_id, policy_digest, 0x83, TEST_NOW_MS);
        transact_test(&mut state, 4, TEST_NOW_MS + 3_000, |transaction| {
            let error = AppendSorafsStreamTokenReputationJournalEntry::new(stale)
                .execute(&authority, transaction)
                .expect_err("stale source observation must fail closed");
            assert!(error.to_string().contains("exceeds 1500ms"));
            Ok(())
        })
        .expect("commit block after rejecting stale source observation");

        let view = state.view();
        let page = FindSorafsReputationJournalEvents::new(None, None, 8)
            .execute(&view)
            .expect("query finalized reputation event");
        assert_eq!(page.events.len(), 1);
        assert_eq!(
            page.events[0].entry.source_time_unix_ms,
            source_time_unix_ms
        );
        assert_eq!(page.events[0].recorded_at_unix_ms, recorded_at_unix_ms);
    }

    #[test]
    fn capacity_dispute_resolution_is_atomic_terminal_and_replay_safe() {
        let (mut state, authority, _other, provider_id) = state_with_reputation_accounts();
        let policy = policy(&authority);
        let policy_digest = policy.canonical_digest().expect("policy digest");
        let dispute_id = iroha_data_model::sorafs::capacity::CapacityDisputeId::new([0x71; 32]);
        let record = CapacityDisputeRecord::new_pending(
            dispute_id,
            provider_id,
            [0x72; 32],
            None,
            2,
            TEST_NOW_MS / 1_000,
            "governed uptime dispute".to_owned(),
            Some("restore service".to_owned()),
            iroha_data_model::sorafs::capacity::CapacityDisputeEvidence {
                digest: [0x73; 32],
                media_type: Some("application/norito".to_owned()),
                uri: None,
                size_bytes: Some(128),
            },
            vec![0x74],
        );
        transact_test(&mut state, 1, TEST_NOW_MS, |transaction| {
            SetSorafsReputationJournalAuthorityPolicy::new(policy)
                .execute(&authority, transaction)?;
            validate_capacity_dispute_opened_replay(transaction, &authority, &record)
                .expect_err("an existing dispute without its opened journal must fail closed");
            assert!(
                read_journal_head(transaction.world())
                    .expect("read journal after rejected incomplete replay")
                    .is_none(),
                "replay validation must not backfill missing authoritative state"
            );
            let mut stale_record = record.clone();
            stale_record.submitted_epoch = stale_record.submitted_epoch.saturating_add(1);
            append_capacity_dispute_opened(transaction, &authority, &stale_record)
                .expect_err("dispute intake time must bind to the committing block");
            assert!(
                read_journal_head(transaction.world())
                    .expect("read journal after rejected dispute")
                    .is_none(),
                "rejected dispute intake must not advance the journal"
            );
            append_capacity_dispute_opened(transaction, &authority, &record)?;
            transaction
                .world
                .capacity_disputes
                .insert(dispute_id, record);
            Ok(())
        })
        .expect("commit opened dispute");

        let resolution = ResolveSorafsCapacityDispute::new(
            dispute_id,
            policy_digest,
            iroha_data_model::sorafs::capacity::CapacityDisputeOutcome::Upheld,
            [0x75; 32],
            Some("evidence confirmed".to_owned()),
        );
        transact_test(&mut state, 2, TEST_NOW_MS + 1_000, |transaction| {
            resolution.clone().execute(&authority, transaction)
        })
        .expect("commit terminal dispute decision");

        {
            let view = state.view();
            let first_page = FindSorafsReputationJournalEvents::new(None, None, 1)
                .execute(&view)
                .expect("query first finalized reputation page");
            assert_eq!(first_page.events.len(), 1);
            assert!(first_page.has_more);
            let next_after = first_page.next_after.expect("continuation cursor");
            let second_page = FindSorafsReputationJournalEvents::new(
                Some(first_page.finalized_cursor),
                Some(next_after),
                1,
            )
            .execute(&view)
            .expect("query terminal finalized reputation page");
            assert_eq!(second_page.events.len(), 1);
            assert_eq!(second_page.events[0].sequence, 2);
            assert!(!second_page.has_more);
            assert!(second_page.next_after.is_none());
        }

        let header = BlockHeader::new(
            3_u64.try_into().expect("nonzero height"),
            None,
            None,
            None,
            TEST_NOW_MS + 2_000,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let rotated_policy = ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 2,
            predecessor_policy_digest: Some(policy_digest),
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority.clone(),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        };
        SetSorafsReputationJournalAuthorityPolicy::new(rotated_policy)
            .execute(&authority, &mut transaction)
            .expect("rotate dispute recorder policy before delayed replay");
        let stored = transaction
            .world
            .capacity_disputes
            .get(&dispute_id)
            .expect("resolved dispute");
        assert!(matches!(
            &stored.status,
            CapacityDisputeStatus::Resolved(resolved)
                if resolved.outcome
                    == iroha_data_model::sorafs::capacity::CapacityDisputeOutcome::Upheld
        ));
        let source_id = ReputationJournalSourceIdV1::for_provider_dispute(dispute_id);
        let source_head = read_source_head(transaction.world(), source_id)
            .expect("read source head")
            .expect("source head");
        assert_eq!(source_head.source_revision, 2);
        assert_eq!(source_head.sequence, 2);

        resolution
            .execute(&authority, &mut transaction)
            .expect("delayed exact resolution replay is idempotent");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal head")
                .expect("journal head")
                .last_sequence,
            2
        );
    }
}
