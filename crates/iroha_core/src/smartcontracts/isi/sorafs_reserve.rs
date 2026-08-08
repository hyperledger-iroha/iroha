//! Chain-authoritative SoraFS reserve, rent, credit, and appeal handlers.

use std::{str::FromStr, sync::OnceLock};

use iroha_data_model::{
    account::AccountId,
    asset::AssetId,
    events::data::sorafs::{
        SorafsGatewayEvent, SorafsReserveLedgerEvent, SorafsReserveLedgerEventKind,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            AdvanceSorafsReserveLifecycle, ChargeSorafsReserveRent, DecideSorafsReserveAppeal,
            DecideSorafsReserveMovement, DrawSorafsReserveCredit, RegisterSorafsReserveAccount,
            RepaySorafsReserveCredit, RequestSorafsReserveMovement, SetSorafsReservePolicy,
            SubmitSorafsReserveAppeal,
        },
    },
    permission::Permission,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsReserveAppealById, FindSorafsReserveAppeals, FindSorafsReserveEvents,
            FindSorafsReserveMovementById, FindSorafsReserveMovements, FindSorafsReservePolicy,
            FindSorafsReserveProviderById, FindSorafsReserveProviders,
        },
    },
    sorafs::{
        capacity::ProviderId,
        pricing::ProviderCreditRecord,
        reserve::{
            RESERVE_COMMITTED_EVENT_MAX_BYTES_V1, RESERVE_MAX_REASON_BYTES_V1,
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1, RESERVE_QUERY_MAX_ITEMS_V1,
            RESERVE_RENT_BILLING_PERIOD_SECONDS_V1, RESERVE_RENT_MAX_BILLING_PERIODS_V1,
            ReserveAppealPageV1, ReserveAppealRecordV1, ReserveAppealStatusV1,
            ReserveAuthorityPolicyRecordV1, ReserveAuthorityPolicyV1, ReserveFinalizedCursorV1,
            ReserveFinalizedEventPageV1, ReserveFinalizedEventV1, ReserveLifecycleStage,
            ReserveMovementKindV1, ReserveMovementPageV1, ReserveMovementRecordV1,
            ReserveMovementStatusV1, ReserveProviderAccountPageV1, ReserveProviderAccountV1,
            ReserveTier,
        },
    },
    state_path::StatePath,
};
use iroha_primitives::{json::Json, numeric::Quantity};
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::deal::XorQuantity;

use super::*;
use crate::smartcontracts::ValidSingularQuery;
use crate::state::{StateTransaction, WorldReadOnly};

const RESERVE_STATE_KEY: &str = "sorafs_reserve_state_v1";
const PROVIDER_STATE_KEY_PREFIX: &str = "sorafs_reserve_provider_v1_";
const MOVEMENT_STATE_KEY_PREFIX: &str = "sorafs_reserve_movement_v1_";
const APPEAL_STATE_KEY_PREFIX: &str = "sorafs_reserve_appeal_v1_";
const EVENT_STATE_KEY_PREFIX: &str = "sorafs_reserve_event_v1_";
const STATE_MAX_BYTES: usize = 2 * 1024 * 1024;
const STATE_LIMITS: DecodeLimits =
    DecodeLimits::new(4_096, STATE_MAX_BYTES, 32_768, STATE_MAX_BYTES * 2, 64);

#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ReservePersistedEventV1 {
    sequence: u64,
    target_block_height: u64,
    event_index: u32,
    event: SorafsReserveLedgerEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ReserveEventJournalHeadV1 {
    last_sequence: u64,
    last_target_block_height: u64,
    last_event_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct ReserveStateV1 {
    policy: ReserveAuthorityPolicyRecordV1,
    journal_head: ReserveEventJournalHeadV1,
}

/// Non-reusable proof that the reserve state machine approved one exact
/// provider withdrawal from protocol custody.
pub(in crate::smartcontracts::isi) struct VerifiedSorafsReserveWithdrawal {
    provider_id: ProviderId,
    movement_id: [u8; 32],
    policy_digest: [u8; 32],
    expected_provider_revision: u64,
    decision_authority: AccountId,
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
}

impl VerifiedSorafsReserveWithdrawal {
    fn new(
        provider_id: ProviderId,
        movement_id: [u8; 32],
        policy_digest: [u8; 32],
        expected_provider_revision: u64,
        decision_authority: AccountId,
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            provider_id,
            movement_id,
            policy_digest,
            expected_provider_revision,
            decision_authority,
            source_id,
            destination_id,
            amount,
        }
    }

    /// Consume the proof into the exact retained-state and balance binding.
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (
        ProviderId,
        [u8; 32],
        [u8; 32],
        u64,
        AccountId,
        AssetId,
        AssetId,
        Quantity,
    ) {
        (
            self.provider_id,
            self.movement_id,
            self.policy_digest,
            self.expected_provider_revision,
            self.decision_authority,
            self.source_id,
            self.destination_id,
            self.amount,
        )
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

#[allow(clippy::too_many_arguments)]
fn emit_reserve_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    kind: SorafsReserveLedgerEventKind,
    provider_id: ProviderId,
    operation_id: Option<[u8; 32]>,
    policy_digest: [u8; 32],
    provider_revision: u64,
    authority: &AccountId,
    now_unix: u64,
) -> Result<(), InstructionExecutionError> {
    if kind == SorafsReserveLedgerEventKind::PolicyActivated {
        return Err(corrupt_state(
            "provider-specific reserve event cannot use the policy activation kind",
        ));
    }
    let account = read_provider(state_transaction.world(), provider_id)?.ok_or_else(|| {
        corrupt_state("provider-specific reserve event has no authoritative provider after-state")
    })?;
    if account.policy_digest != policy_digest
        || account.revision != provider_revision
        || account.updated_at_unix != now_unix
    {
        return Err(corrupt_state(
            "provider-specific reserve event does not match authoritative provider after-state",
        ));
    }
    let occurred_at_unix_ms = now_unix
        .checked_mul(1_000)
        .ok_or_else(|| corrupt_state("reserve event timestamp overflow"))?;
    let event = SorafsReserveLedgerEvent {
        kind,
        provider_id: Some(provider_id),
        operation_id,
        policy_digest,
        provider_revision,
        resulting_lifecycle_stage: Some(account.lifecycle_stage),
        authority: authority.clone(),
        occurred_at_unix_ms,
    };
    append_reserve_event_journal(state_transaction, &event, None)?;
    state_transaction
        .world
        .emit_events(Some(SorafsGatewayEvent::ReserveLedger(event)));
    Ok(())
}

fn emit_reserve_policy_activation(
    state_transaction: &mut StateTransaction<'_, '_>,
    policy: ReserveAuthorityPolicyRecordV1,
    authority: &AccountId,
    now_unix: u64,
) -> Result<(), InstructionExecutionError> {
    let occurred_at_unix_ms = now_unix
        .checked_mul(1_000)
        .ok_or_else(|| corrupt_state("reserve event timestamp overflow"))?;
    let event = SorafsReserveLedgerEvent {
        kind: SorafsReserveLedgerEventKind::PolicyActivated,
        provider_id: None,
        operation_id: None,
        policy_digest: policy.policy_digest,
        provider_revision: 0,
        resulting_lifecycle_stage: None,
        authority: authority.clone(),
        occurred_at_unix_ms,
    };
    append_reserve_event_journal(state_transaction, &event, Some(policy))?;
    state_transaction
        .world
        .emit_events(Some(SorafsGatewayEvent::ReserveLedger(event)));
    Ok(())
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

fn require_governance(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if has_permission(state_transaction, authority, "CanSetSorafsReservePolicy") {
        Ok(())
    } else {
        Err(invalid_parameter(
            "CanSetSorafsReservePolicy is required for authoritative reserve governance",
        ))
    }
}

fn reserve_state_key() -> &'static StatePath {
    static KEY: OnceLock<StatePath> = OnceLock::new();
    KEY.get_or_init(|| StatePath::from_str(RESERVE_STATE_KEY).expect("static state key is valid"))
}

fn digest_key(prefix: &str, digest: [u8; 32]) -> StatePath {
    StatePath::from_str(&format!("{prefix}{}", hex::encode(digest)))
        .expect("static prefix plus lowercase hex is a valid state key")
}

fn provider_key(provider_id: ProviderId) -> StatePath {
    digest_key(PROVIDER_STATE_KEY_PREFIX, *provider_id.as_bytes())
}

fn movement_key(movement_id: [u8; 32]) -> StatePath {
    digest_key(MOVEMENT_STATE_KEY_PREFIX, movement_id)
}

fn appeal_key(appeal_id: [u8; 32]) -> StatePath {
    digest_key(APPEAL_STATE_KEY_PREFIX, appeal_id)
}

fn event_key(sequence: u64) -> StatePath {
    StatePath::from_str(&format!("{EVENT_STATE_KEY_PREFIX}{sequence:016x}"))
        .expect("static prefix plus fixed-width lowercase hex is a valid state key")
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

fn validate_persisted_event(
    record: &ReservePersistedEventV1,
    expected_sequence: u64,
) -> Result<(), InstructionExecutionError> {
    if record.sequence == 0
        || record.sequence != expected_sequence
        || record.target_block_height == 0
        || record.event.occurred_at_unix_ms == 0
        || record.event.policy_digest == [0; 32]
        || record
            .event
            .provider_id
            .is_some_and(|provider_id| provider_id.as_bytes() == &[0; 32])
        || record.event.operation_id == Some([0; 32])
    {
        return Err(corrupt_state(
            "stored reserve event cursor or payload metadata is invalid",
        ));
    }
    let shape_is_valid = match record.event.kind {
        SorafsReserveLedgerEventKind::PolicyActivated => {
            record.event.provider_id.is_none()
                && record.event.operation_id.is_none()
                && record.event.provider_revision == 0
                && record.event.resulting_lifecycle_stage.is_none()
        }
        SorafsReserveLedgerEventKind::ProviderRegistered => {
            record.event.provider_id.is_some()
                && record.event.operation_id.is_none()
                && record.event.provider_revision == 1
                && record.event.resulting_lifecycle_stage.is_some()
        }
        SorafsReserveLedgerEventKind::MovementRequested
        | SorafsReserveLedgerEventKind::MovementApproved
        | SorafsReserveLedgerEventKind::MovementRejected
        | SorafsReserveLedgerEventKind::AppealSubmitted
        | SorafsReserveLedgerEventKind::AppealAccepted
        | SorafsReserveLedgerEventKind::AppealRejected => {
            record.event.provider_id.is_some()
                && record.event.operation_id.is_some()
                && record.event.provider_revision > 0
                && record.event.resulting_lifecycle_stage.is_some()
        }
        SorafsReserveLedgerEventKind::RentCharged
        | SorafsReserveLedgerEventKind::LifecycleAdvanced
        | SorafsReserveLedgerEventKind::CreditDrawn
        | SorafsReserveLedgerEventKind::CreditRepaid => {
            record.event.provider_id.is_some()
                && record.event.operation_id.is_none()
                && record.event.provider_revision > 0
                && record.event.resulting_lifecycle_stage.is_some()
        }
    };
    if !shape_is_valid {
        return Err(corrupt_state(
            "stored reserve event payload shape is invalid",
        ));
    }
    Ok(())
}

fn validate_event_successor(
    previous: Option<&ReservePersistedEventV1>,
    current: &ReservePersistedEventV1,
) -> Result<(), InstructionExecutionError> {
    let Some(previous) = previous else {
        if current.sequence != 1 || current.event_index != 0 {
            return Err(corrupt_state(
                "reserve event journal does not begin at sequence one and block index zero",
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
            "reserve event journal sequence is not contiguous",
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
            "reserve event journal block height/index ordering is invalid",
        )),
    }
}

fn read_persisted_event(
    world: &impl WorldReadOnly,
    sequence: u64,
) -> Result<Option<ReservePersistedEventV1>, InstructionExecutionError> {
    if sequence == 0 {
        return Err(corrupt_state("reserve event sequence zero cannot be read"));
    }
    let Some(bytes) = world.smart_contract_state().get(&event_key(sequence)) else {
        return Ok(None);
    };
    decode_persisted_event(bytes, sequence).map(Some)
}

fn decode_persisted_event(
    bytes: &[u8],
    sequence: u64,
) -> Result<ReservePersistedEventV1, InstructionExecutionError> {
    if bytes.len() > RESERVE_COMMITTED_EVENT_MAX_BYTES_V1 {
        return Err(corrupt_state(format!(
            "reserve committed event exceeds {RESERVE_COMMITTED_EVENT_MAX_BYTES_V1} bytes"
        )));
    }
    let record: ReservePersistedEventV1 = decode_state(bytes, "reserve committed event")?;
    validate_persisted_event(&record, sequence)?;
    Ok(record)
}

fn validate_event_journal_head(
    world: &impl WorldReadOnly,
    head: ReserveEventJournalHeadV1,
) -> Result<(), InstructionExecutionError> {
    let record = read_persisted_event(world, head.last_sequence)?
        .ok_or_else(|| corrupt_state("reserve event journal head references a missing event"))?;
    if record.target_block_height != head.last_target_block_height
        || record.event_index != head.last_event_index
    {
        return Err(corrupt_state(
            "reserve event journal head does not match its terminal event",
        ));
    }
    let predecessor = if head.last_sequence == 1 {
        None
    } else {
        let predecessor_sequence = head.last_sequence - 1;
        Some(
            read_persisted_event(world, predecessor_sequence)?.ok_or_else(|| {
                corrupt_state(format!(
                    "reserve event journal is missing terminal predecessor sequence {predecessor_sequence}"
                ))
            })?,
        )
    };
    validate_event_successor(predecessor.as_ref(), &record)?;
    Ok(())
}

fn ensure_reserve_namespace_empty(
    world: &impl WorldReadOnly,
) -> Result<(), InstructionExecutionError> {
    let prefix = StatePath::from_str("sorafs_reserve_").expect("static reserve prefix is valid");
    if world
        .smart_contract_state()
        .range(prefix..)
        .next()
        .is_some_and(|(key, _)| key.as_ref().starts_with("sorafs_reserve_"))
    {
        return Err(corrupt_state(
            "initial reserve activation requires an empty reserve state namespace",
        ));
    }
    Ok(())
}

fn ensure_no_event_after_head(
    world: &impl WorldReadOnly,
    head: Option<ReserveEventJournalHeadV1>,
) -> Result<(), InstructionExecutionError> {
    let prefix_start =
        StatePath::from_str(EVENT_STATE_KEY_PREFIX).expect("static event prefix is valid");
    let first_event_key = world
        .smart_contract_state()
        .range(prefix_start..)
        .next()
        .and_then(|(key, _)| {
            key.to_string()
                .starts_with(EVENT_STATE_KEY_PREFIX)
                .then_some(key)
        });
    match (head, first_event_key) {
        (None, None) => return Ok(()),
        (None, Some(_)) => {
            return Err(corrupt_state(
                "reserve event journal contains records without a head",
            ));
        }
        (Some(_), Some(key)) if *key == event_key(1) => {}
        (Some(_), _) => {
            return Err(corrupt_state(
                "reserve event journal does not begin at sequence one",
            ));
        }
    }
    let start = head.map_or_else(
        || StatePath::from_str(EVENT_STATE_KEY_PREFIX).expect("static event prefix is valid"),
        |head| event_key(head.last_sequence),
    );
    for (key, _) in world.smart_contract_state().range(start..) {
        let rendered = key.to_string();
        if !rendered.starts_with(EVENT_STATE_KEY_PREFIX) {
            break;
        }
        if head.is_some_and(|head| *key == event_key(head.last_sequence)) {
            continue;
        }
        return Err(corrupt_state(
            "reserve event journal contains a record beyond its head",
        ));
    }
    Ok(())
}

fn append_reserve_event_journal(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: &SorafsReserveLedgerEvent,
    next_policy: Option<ReserveAuthorityPolicyRecordV1>,
) -> Result<(), InstructionExecutionError> {
    let committed_parent_height = u64::try_from(state_transaction.block_hashes().len())
        .map_err(|_| corrupt_state("committed reserve parent height does not fit into u64"))?;
    let target_block_height = committed_parent_height
        .checked_add(1)
        .ok_or_else(|| corrupt_state("reserve event target block height overflow"))?;
    let executing_block_height = state_transaction._curr_block.height().get();
    if target_block_height != executing_block_height {
        return Err(corrupt_state(format!(
            "reserve event target height {target_block_height} does not match executing block height {executing_block_height}"
        )));
    }
    let current_state = read_reserve_state(state_transaction.world())?;
    let head = current_state.as_ref().map(|state| state.journal_head);
    ensure_no_event_after_head(state_transaction.world(), head)?;
    let (sequence, event_index) = match head {
        Some(head) => {
            let sequence = head
                .last_sequence
                .checked_add(1)
                .ok_or_else(|| corrupt_state("reserve event sequence overflow"))?;
            let event_index = match head.last_target_block_height.cmp(&target_block_height) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => head
                    .last_event_index
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("reserve event block index overflow"))?,
                core::cmp::Ordering::Greater => {
                    return Err(corrupt_state(
                        "reserve event target height regressed behind the journal head",
                    ));
                }
            };
            (sequence, event_index)
        }
        None => {
            ensure_reserve_namespace_empty(state_transaction.world())?;
            let policy = next_policy.as_ref().ok_or_else(|| {
                corrupt_state("first reserve event has no initial policy activation")
            })?;
            if event.kind != SorafsReserveLedgerEventKind::PolicyActivated
                || event.provider_id.is_some()
                || event.operation_id.is_some()
                || event.provider_revision != 0
                || event.resulting_lifecycle_stage.is_some()
                || policy.policy.revision != 1
                || policy.policy_digest != event.policy_digest
            {
                return Err(corrupt_state(
                    "reserve event journal must begin with initial policy activation",
                ));
            }
            (1, 0)
        }
    };
    let key = event_key(sequence);
    if state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .is_some()
    {
        return Err(corrupt_state(
            "reserve event journal sequence already exists",
        ));
    }
    let record = ReservePersistedEventV1 {
        sequence,
        target_block_height,
        event_index,
        event: event.clone(),
    };
    validate_persisted_event(&record, sequence)?;
    let previous = if sequence == 1 {
        None
    } else {
        read_persisted_event(state_transaction.world(), sequence - 1)?
    };
    validate_event_successor(previous.as_ref(), &record)?;
    let encoded_record = encode_state(&record, "reserve committed event")?;
    if encoded_record.len() > RESERVE_COMMITTED_EVENT_MAX_BYTES_V1 {
        return Err(corrupt_state(format!(
            "encoded reserve committed event exceeds {RESERVE_COMMITTED_EVENT_MAX_BYTES_V1} bytes"
        )));
    }
    let next_head = ReserveEventJournalHeadV1 {
        last_sequence: sequence,
        last_target_block_height: target_block_height,
        last_event_index: event_index,
    };
    let policy = match (current_state, next_policy) {
        (None, Some(policy)) => policy,
        (Some(_), Some(policy)) if event.kind == SorafsReserveLedgerEventKind::PolicyActivated => {
            policy
        }
        (Some(state), None) if event.kind != SorafsReserveLedgerEventKind::PolicyActivated => {
            state.policy
        }
        _ => {
            return Err(corrupt_state(
                "reserve policy transition does not match the journal event kind",
            ));
        }
    };
    let next_state = ReserveStateV1 {
        policy,
        journal_head: next_head,
    };
    let encoded_state = encode_state(&next_state, "reserve state")?;
    state_transaction
        .world
        .smart_contract_state
        .insert(key, encoded_record);
    state_transaction
        .world
        .smart_contract_state
        .insert(reserve_state_key().clone(), encoded_state);
    Ok(())
}

fn now_unix(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let now = state_transaction.block_unix_timestamp_ms() / 1_000;
    if now == 0 {
        return Err(invalid_parameter(
            "authoritative reserve operations require a non-zero block timestamp",
        ));
    }
    Ok(now)
}

fn read_policy(
    world: &impl WorldReadOnly,
) -> Result<Option<ReserveAuthorityPolicyRecordV1>, InstructionExecutionError> {
    Ok(read_reserve_state(world)?.map(|state| state.policy))
}

/// Return whether `asset_id` is the exact asset held by active SoraFS reserve
/// custody.
pub(super) fn is_reserve_custody_asset(
    world: &impl WorldReadOnly,
    asset_id: &AssetId,
) -> Result<bool, InstructionExecutionError> {
    Ok(read_policy(world)?.is_some_and(|record| {
        record.policy.asset_definition == *asset_id.definition()
            && record.policy.custody_account == *asset_id.account()
    }))
}

/// Return whether `account_id` is the active SoraFS reserve custody account.
pub(super) fn is_reserve_custody_account(
    world: &impl WorldReadOnly,
    account_id: &AccountId,
) -> Result<bool, InstructionExecutionError> {
    Ok(read_policy(world)?.is_some_and(|record| record.policy.custody_account == *account_id))
}

/// Return whether `definition_id` backs the active SoraFS reserve ledger.
pub(super) fn is_reserve_asset_definition(
    world: &impl WorldReadOnly,
    definition_id: &iroha_data_model::asset::AssetDefinitionId,
) -> Result<bool, InstructionExecutionError> {
    Ok(read_policy(world)?.is_some_and(|record| record.policy.asset_definition == *definition_id))
}

/// Revalidate a sealed withdrawal against the still-pending authoritative
/// movement and provider records immediately before custody is debited.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_verified_reserve_withdrawal(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
    movement_id: [u8; 32],
    policy_digest: [u8; 32],
    expected_provider_revision: u64,
    decision_authority: &AccountId,
    source_id: &AssetId,
    destination_id: &AssetId,
    amount: &Quantity,
) -> Result<(), InstructionExecutionError> {
    let policy = read_policy(world)?
        .ok_or_else(|| corrupt_state("verified reserve withdrawal has no active policy"))?;
    if policy.policy_digest != policy_digest
        || policy.policy.decision_authority != *decision_authority
        || policy.policy.asset_definition != *source_id.definition()
        || policy.policy.custody_account != *source_id.account()
    {
        return Err(corrupt_state(
            "verified reserve withdrawal does not match active policy custody",
        ));
    }
    let account = read_provider(world, provider_id)?
        .ok_or_else(|| corrupt_state("verified reserve withdrawal has no provider account"))?;
    if account.policy_digest != policy_digest
        || account.revision != expected_provider_revision
        || destination_id.definition() != source_id.definition()
        || account.terms.provider_account != *destination_id.account()
    {
        return Err(corrupt_state(
            "verified reserve withdrawal does not match the provider revision and owner",
        ));
    }
    let movement = read_movement(world, movement_id)?
        .ok_or_else(|| corrupt_state("verified reserve withdrawal has no retained movement"))?;
    if movement.provider_id != provider_id
        || movement.policy_digest != policy_digest
        || movement.kind != ReserveMovementKindV1::Withdrawal
        || movement.status != ReserveMovementStatusV1::Pending
        || movement.amount.as_quantity() != amount
    {
        return Err(corrupt_state(
            "verified reserve withdrawal does not match the pending retained movement",
        ));
    }
    Ok(())
}

fn read_reserve_state(
    world: &impl WorldReadOnly,
) -> Result<Option<ReserveStateV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(reserve_state_key()) else {
        return Ok(None);
    };
    let state = decode_reserve_state(bytes)?;
    validate_event_journal_head(world, state.journal_head)?;
    Ok(Some(state))
}

fn decode_reserve_state(bytes: &[u8]) -> Result<ReserveStateV1, InstructionExecutionError> {
    let state: ReserveStateV1 = decode_state(bytes, "reserve state")?;
    validate_policy_record(&state.policy)?;
    if state.journal_head.last_sequence == 0 || state.journal_head.last_target_block_height == 0 {
        return Err(corrupt_state(
            "stored reserve event journal head is invalid",
        ));
    }
    Ok(state)
}

fn validate_policy_record(
    record: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), InstructionExecutionError> {
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored reserve policy: {error}")))?;
    let digest = record
        .policy
        .digest()
        .map_err(|error| corrupt_state(format!("failed to digest reserve policy: {error}")))?;
    if digest != record.policy_digest || record.activated_at_unix == 0 {
        return Err(corrupt_state(
            "stored reserve policy digest or activation timestamp is invalid",
        ));
    }
    Ok(())
}

fn active_policy(
    state_transaction: &StateTransaction<'_, '_>,
    expected_digest: [u8; 32],
) -> Result<(ReserveAuthorityPolicyRecordV1, u64), InstructionExecutionError> {
    let now = now_unix(state_transaction)?;
    let record = read_policy(state_transaction.world())?
        .ok_or_else(|| invalid_parameter("SoraFS reserve policy is not configured"))?;
    if record.policy_digest != expected_digest {
        return Err(invalid_parameter(format!(
            "reserve policy digest mismatch: supplied {}, active {}",
            hex::encode(expected_digest),
            hex::encode(record.policy_digest)
        )));
    }
    if record.activated_at_unix > now {
        return Err(corrupt_state(
            "stored reserve policy activation is later than the current block",
        ));
    }
    Ok((record, now))
}

fn require_operations_authority(
    authority: &AccountId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), InstructionExecutionError> {
    if authority == &policy.policy.operations_authority {
        Ok(())
    } else {
        Err(invalid_parameter(
            "reserve transaction authority is not the exact governed operations account",
        ))
    }
}

fn require_decision_authority(
    authority: &AccountId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), InstructionExecutionError> {
    if authority == &policy.policy.decision_authority {
        Ok(())
    } else {
        Err(invalid_parameter(
            "reserve transaction authority is not the exact governed decision account",
        ))
    }
}

fn registered_provider_owner(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<AccountId, InstructionExecutionError> {
    world
        .provider_owners()
        .get(&provider_id)
        .cloned()
        .ok_or_else(|| invalid_parameter(format!("unknown SoraFS provider {provider_id}")))
}

fn credit_cap(
    policy: &ReserveAuthorityPolicyV1,
    terms: &iroha_data_model::sorafs::reserve::ReserveProviderTermsV1,
) -> Result<XorQuantity, InstructionExecutionError> {
    let quote = policy
        .economics
        .quote(
            terms.storage_class,
            terms.capacity_gib,
            terms.duration,
            terms.tier,
            XorQuantity::zero(),
        )
        .map_err(|error| invalid_parameter(format!("invalid reserve account terms: {error}")))?;
    Ok(quote.credit_line_cap.map_or_else(XorQuantity::zero, |cap| {
        XorQuantity::min(&cap, &policy.max_provider_debt)
    }))
}

/// Read one canonical authoritative reserve-provider account by exact registry id.
pub(super) fn read_provider(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<Option<ReserveProviderAccountV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&provider_key(provider_id)) else {
        return Ok(None);
    };
    decode_provider_record(bytes, provider_id).map(Some)
}

fn total_reserved_custody(
    world: &impl WorldReadOnly,
) -> Result<XorQuantity, InstructionExecutionError> {
    let start =
        StatePath::from_str(PROVIDER_STATE_KEY_PREFIX).expect("static provider prefix is valid");
    let mut total = XorQuantity::zero();
    for (key, payload) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(PROVIDER_STATE_KEY_PREFIX) {
            break;
        }
        let candidate: ReserveProviderAccountV1 =
            decode_state(payload, "reserve provider account")?;
        let provider_id = candidate.terms.provider_id;
        if provider_key(provider_id) != *key {
            return Err(corrupt_state(
                "authoritative reserve provider key does not match its account",
            ));
        }
        let account = decode_provider_record(payload, provider_id)?;
        total = total
            .checked_add(&account.reserve_balance)
            .map_err(|error| corrupt_state(format!("reserve custody total overflow: {error}")))?;
    }
    Ok(total)
}

/// Resolve collateral that is backed by the native owner-funded reserve flow.
///
/// The returned balance is not an administrator-authored credit projection.
/// It is the provider's reserve partition minus any outstanding treasury-funded
/// credit principal. Native custody must also cover the sum of every provider
/// partition, preventing the same custody balance from backing multiple bonds.
pub(super) fn verified_provider_bond(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
    expected_owner: &AccountId,
    committed_capacity_gib: u64,
) -> Result<XorQuantity, InstructionExecutionError> {
    let policy = read_policy(world)?
        .ok_or_else(|| invalid_parameter("SoraFS reserve policy is not configured"))?;
    let account = read_provider(world, provider_id)?.ok_or_else(|| {
        invalid_parameter(format!(
            "provider {provider_id} has no owner-funded reserve account"
        ))
    })?;
    if account.terms.provider_account != *expected_owner {
        return Err(invalid_parameter(format!(
            "provider {provider_id} reserve account is bound to {}, not the governed owner {expected_owner}",
            account.terms.provider_account
        )));
    }
    if account.terms.capacity_gib < committed_capacity_gib {
        return Err(invalid_parameter(format!(
            "provider {provider_id} reserve account covers {} GiB, below the declared {committed_capacity_gib} GiB",
            account.terms.capacity_gib
        )));
    }
    let custody_balance =
        provider_spendable_balance(world, &policy.policy, &policy.policy.custody_account)?;
    let reserved_custody = total_reserved_custody(world)?;
    if custody_balance < reserved_custody {
        return Err(corrupt_state(format!(
            "aggregate provider reserve partitions exceed the native custody asset balance while verifying provider {provider_id}"
        )));
    }
    account
        .reserve_balance
        .checked_sub(&account.debt_principal)
        .map_err(|error| {
            corrupt_state(format!(
                "provider {provider_id} owner-funded reserve underflow: {error}"
            ))
        })
}

fn credit_after_verified_reserve_withdrawal(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
    current_reserve: &XorQuantity,
    remaining_reserve: &XorQuantity,
    debt_principal: &XorQuantity,
) -> Result<Option<ProviderCreditRecord>, InstructionExecutionError> {
    let Some(mut credit) = world.provider_credit_ledger().get(&provider_id).cloned() else {
        return Ok(None);
    };
    let current_owner_funded = current_reserve
        .checked_sub(debt_principal)
        .map_err(|error| {
            corrupt_state(format!(
                "provider {provider_id} owner-funded reserve underflow before withdrawal: {error}"
            ))
        })?;
    let bonded = XorQuantity::try_from_quantity(credit.bonded.clone()).map_err(|error| {
        corrupt_state(format!(
            "provider {provider_id} bonded credit projection is invalid: {error}"
        ))
    })?;
    let slashed = XorQuantity::try_from_quantity(credit.slashed.clone()).map_err(|error| {
        corrupt_state(format!(
            "provider {provider_id} slash-lien projection is invalid: {error}"
        ))
    })?;
    let committed = bonded.checked_add(&slashed).map_err(|error| {
        corrupt_state(format!(
            "provider {provider_id} custody commitment overflow: {error}"
        ))
    })?;
    if committed > current_owner_funded {
        return Err(corrupt_state(format!(
            "provider {provider_id} bonded-plus-slashed credit commitment exceeds native reserve custody"
        )));
    }

    let remaining_owner_funded = remaining_reserve.checked_sub(debt_principal).map_err(
        |error| {
            invalid_parameter(format!(
                "provider {provider_id} withdrawal would consume treasury-funded principal: {error}"
            ))
        },
    )?;
    let next_bonded = remaining_owner_funded.checked_sub(&slashed).map_err(|_| {
        invalid_parameter(format!(
            "provider {provider_id} withdrawal would release custody subject to a slash lien"
        ))
    })?;
    if next_bonded.as_quantity() < &credit.required_bond {
        return Err(invalid_parameter(format!(
            "provider {provider_id} withdrawal would reduce unslashed bond {} below required {}",
            next_bonded.as_quantity(),
            credit.required_bond
        )));
    }
    if let Some(record) = world.capacity_declarations().get(&provider_id) {
        let declaration = super::sorafs::decode_capacity_declaration_payload(&record.declaration)
            .map_err(|error| {
            corrupt_state(format!(
                "provider {provider_id} capacity declaration payload is invalid: {error}"
            ))
        })?;
        let declared_stake = declaration.stake.stake_amount.as_quantity();
        if next_bonded.as_quantity() < declared_stake {
            return Err(invalid_parameter(format!(
                "provider {provider_id} withdrawal would reduce unslashed bond {} below declared stake {declared_stake}",
                next_bonded.as_quantity(),
            )));
        }
    }
    credit.bonded = next_bonded.into_quantity();
    Ok(Some(credit))
}

#[cfg(test)]
pub(super) fn seed_verified_provider_bond_for_test(
    state_transaction: &mut StateTransaction<'_, '_>,
    provider_id: ProviderId,
    owner: &AccountId,
    capacity_gib: u64,
    bonded: iroha_primitives::numeric::Quantity,
) -> Result<(), InstructionExecutionError> {
    use iroha_data_model::{IntoKeyValue, account::Account, asset::Asset};

    let policy = if let Some(policy) = read_policy(state_transaction.world())? {
        policy
    } else {
        let custody_account = AccountId::new(iroha_crypto::derive_non_signing_ed25519_public_key(
            b"iroha:sorafs:test-reserve-custody:v1",
            &[],
        ));
        if state_transaction
            .world
            .accounts()
            .get(&custody_account)
            .is_none()
        {
            let account = Account::new(custody_account.clone()).build(&custody_account);
            let (account_id, account) = account.into_key_value();
            state_transaction.world.accounts.insert(account_id, account);
        }
        let treasury_account = owner.clone();
        let policy = ReserveAuthorityPolicyV1 {
            version: iroha_data_model::sorafs::reserve::RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: iroha_data_model::sorafs::reserve::ReservePolicyV1::default(),
            asset_definition: state_transaction.gov.sorafs_pin_fee_asset_id.clone(),
            custody_account,
            treasury_account,
            operations_authority: owner.clone(),
            decision_authority: owner.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
                .expect("bounded test reserve debt"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        };
        let policy_digest = policy.digest().map_err(|error| {
            corrupt_state(format!("test reserve policy digest failed: {error}"))
        })?;
        let record = ReserveAuthorityPolicyRecordV1 {
            policy,
            policy_digest,
            activated_by: owner.clone(),
            activated_at_unix: 1,
        };
        emit_reserve_policy_activation(state_transaction, record.clone(), owner, 1)?;
        record
    };
    if policy.policy.custody_account == *owner {
        return Err(corrupt_state(
            "test reserve provider owner must differ from protocol custody",
        ));
    }

    let reserve_balance = XorQuantity::try_from_quantity(bonded)
        .map_err(|error| corrupt_state(format!("test reserve bond is invalid: {error}")))?;
    let account = ReserveProviderAccountV1 {
        terms: iroha_data_model::sorafs::reserve::ReserveProviderTermsV1 {
            provider_id,
            provider_account: owner.clone(),
            tier: ReserveTier::TierA,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
            duration: iroha_data_model::sorafs::reserve::ReserveDuration::Monthly,
            capacity_gib,
        },
        policy_digest: policy.policy_digest,
        revision: 1,
        reserve_balance: reserve_balance.clone(),
        debt_principal: XorQuantity::zero(),
        accrued_interest: XorQuantity::zero(),
        credit_cap: XorQuantity::zero(),
        lifecycle_stage: ReserveLifecycleStage::Active,
        days_past_due: 0,
        pending_movements: 0,
        open_appeals: 0,
        rent_charged_through_unix: 1,
        interest_accrued_at_unix: 1,
        updated_at_unix: 1,
    };
    state_transaction.world.smart_contract_state.insert(
        provider_key(provider_id),
        encode_state(&account, "test reserve provider account")?,
    );

    let custody_asset_id = AssetId::of(
        policy.policy.asset_definition,
        policy.policy.custody_account,
    );
    let current = state_transaction
        .world
        .assets
        .get(&custody_asset_id)
        .map_or_else(iroha_primitives::numeric::Quantity::zero, |value| {
            value.as_ref().clone()
        });
    let next = current
        .checked_add(reserve_balance.as_quantity())
        .map_err(|error| corrupt_state(format!("test reserve custody overflow: {error}")))?;
    let (asset_id, asset) = Asset::new(custody_asset_id, next).into_key_value();
    state_transaction.world.assets.insert(asset_id, asset);
    Ok(())
}

fn decode_provider_record(
    bytes: &[u8],
    provider_id: ProviderId,
) -> Result<ReserveProviderAccountV1, InstructionExecutionError> {
    let account: ReserveProviderAccountV1 = decode_state(bytes, "reserve provider account")?;
    if account.terms.provider_id != provider_id
        || account.terms.capacity_gib == 0
        || account.policy_digest == [0; 32]
        || account.revision == 0
        || account.debt_principal > account.credit_cap
        || account.pending_movements > 256
        || account.open_appeals > 16
        || account.rent_charged_through_unix == 0
        || account.interest_accrued_at_unix == 0
        || account.updated_at_unix == 0
        || account.rent_charged_through_unix > account.updated_at_unix
        || account.interest_accrued_at_unix > account.updated_at_unix
    {
        return Err(corrupt_state(
            "stored reserve provider account is inconsistent",
        ));
    }
    Ok(account)
}

fn ensure_apr_rotation_has_no_debt(
    world: &impl WorldReadOnly,
    current: &ReserveAuthorityPolicyV1,
    next: &ReserveAuthorityPolicyV1,
) -> Result<(), InstructionExecutionError> {
    let apr_changed = [ReserveTier::TierA, ReserveTier::TierB, ReserveTier::TierC]
        .into_iter()
        .try_fold(false, |changed, tier| {
            let current_apr = current
                .economics
                .tier_configuration(tier)
                .map_err(|error| corrupt_state(format!("invalid active reserve tier: {error}")))?
                .interest_apr_bps;
            let next_apr = next
                .economics
                .tier_configuration(tier)
                .map_err(|error| invalid_parameter(format!("invalid next reserve tier: {error}")))?
                .interest_apr_bps;
            Ok::<_, InstructionExecutionError>(changed || current_apr != next_apr)
        })?;
    if !apr_changed {
        return Ok(());
    }

    let start =
        StatePath::from_str(PROVIDER_STATE_KEY_PREFIX).expect("static provider prefix is valid");
    for (key, payload) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(PROVIDER_STATE_KEY_PREFIX) {
            break;
        }
        let candidate: ReserveProviderAccountV1 =
            decode_state(payload, "reserve provider account")?;
        if provider_key(candidate.terms.provider_id) != *key {
            return Err(corrupt_state(
                "authoritative reserve provider key does not match its account",
            ));
        }
        let account = read_provider(world, candidate.terms.provider_id)?.ok_or_else(|| {
            corrupt_state("authoritative reserve provider disappeared during policy validation")
        })?;
        if !account
            .total_debt()
            .map_err(|error| corrupt_state(format!("invalid provider debt: {error}")))?
            .is_zero()
        {
            return Err(invalid_parameter(
                "reserve APR changes require all provider debt and accrued interest to be settled",
            ));
        }
    }
    Ok(())
}

fn provider_for_policy(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<ReserveProviderAccountV1, InstructionExecutionError> {
    let mut account = read_provider(world, provider_id)?
        .ok_or_else(|| invalid_parameter(format!("reserve account for {provider_id} not found")))?;
    let governed_cap = credit_cap(&policy.policy, &account.terms)?;
    account.credit_cap = if account.debt_principal > governed_cap {
        account.debt_principal.clone()
    } else {
        governed_cap
    };
    if account.policy_digest != policy.policy_digest {
        account.policy_digest = policy.policy_digest;
    }
    Ok(account)
}

fn ensure_revision(
    account: &ReserveProviderAccountV1,
    expected: u64,
) -> Result<(), InstructionExecutionError> {
    if account.revision == expected {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "reserve provider revision conflict: expected {expected}, current {}",
            account.revision
        )))
    }
}

fn ensure_provider_timestamp(
    account: &ReserveProviderAccountV1,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    if now >= account.updated_at_unix {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "reserve block timestamp {now} predates provider update {}",
            account.updated_at_unix
        )))
    }
}

fn advance_provider_revision(
    account: &mut ReserveProviderAccountV1,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    ensure_provider_timestamp(account, now)?;
    if account.rent_charged_through_unix > now || account.interest_accrued_at_unix > now {
        return Err(corrupt_state(
            "reserve provider anchors cannot exceed the next update timestamp",
        ));
    }
    let next_revision = account
        .revision
        .checked_add(1)
        .ok_or_else(|| corrupt_state("reserve provider revision overflow"))?;
    account.revision = next_revision;
    account.updated_at_unix = now;
    Ok(())
}

fn accrue_interest(
    account: &mut ReserveProviderAccountV1,
    policy: &ReserveAuthorityPolicyV1,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    let tier = policy
        .economics
        .tier_configuration(account.terms.tier)
        .map_err(|error| corrupt_state(format!("invalid active reserve tier: {error}")))?;
    account
        .accrue_interest(tier.interest_apr_bps, now)
        .map_err(|error| invalid_parameter(format!("reserve interest accrual failed: {error}")))?;
    Ok(())
}

fn read_movement(
    world: &impl WorldReadOnly,
    movement_id: [u8; 32],
) -> Result<Option<ReserveMovementRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&movement_key(movement_id)) else {
        return Ok(None);
    };
    decode_movement_record(bytes, movement_id).map(Some)
}

fn decode_movement_record(
    bytes: &[u8],
    movement_id: [u8; 32],
) -> Result<ReserveMovementRecordV1, InstructionExecutionError> {
    let record: ReserveMovementRecordV1 = decode_state(bytes, "reserve movement")?;
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.movement_id != movement_id
        || record.movement_id == [0; 32]
        || record.amount.is_zero()
        || record.expected_provider_revision == 0
        || record.policy_digest == [0; 32]
        || record.requested_at_unix == 0
        || match record.status {
            ReserveMovementStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveMovementStatusV1::Approved | ReserveMovementStatusV1::Rejected => {
                !terminal_fields
            }
        }
    {
        return Err(corrupt_state("stored reserve movement is inconsistent"));
    }
    Ok(record)
}

fn read_appeal(
    world: &impl WorldReadOnly,
    appeal_id: [u8; 32],
) -> Result<Option<ReserveAppealRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&appeal_key(appeal_id)) else {
        return Ok(None);
    };
    decode_appeal_record(bytes, appeal_id).map(Some)
}

fn decode_appeal_record(
    bytes: &[u8],
    appeal_id: [u8; 32],
) -> Result<ReserveAppealRecordV1, InstructionExecutionError> {
    let record: ReserveAppealRecordV1 = decode_state(bytes, "reserve appeal")?;
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.appeal_id != appeal_id
        || record.appeal_id == [0; 32]
        || record.reason.is_empty()
        || record.reason.len() > RESERVE_MAX_REASON_BYTES_V1
        || record.expected_provider_revision == 0
        || record.submitted_at_unix == 0
        || match record.status {
            ReserveAppealStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveAppealStatusV1::Accepted | ReserveAppealStatusV1::Rejected => !terminal_fields,
        }
    {
        return Err(corrupt_state("stored reserve appeal is inconsistent"));
    }
    Ok(record)
}

fn transfer(
    state_transaction: &mut StateTransaction<'_, '_>,
    policy: &ReserveAuthorityPolicyV1,
    source: &AccountId,
    destination: &AccountId,
    amount: &XorQuantity,
) -> Result<(), InstructionExecutionError> {
    super::asset::isi::execute_user_numeric_asset_transfer(
        state_transaction,
        source,
        AssetId::of(policy.asset_definition.clone(), source.clone()),
        destination.clone(),
        amount.clone().into_quantity(),
    )
    .map_err(|error| invalid_parameter(format!("reserve custody transfer failed: {error}")))
}

fn provider_spendable_balance(
    world: &impl WorldReadOnly,
    policy: &ReserveAuthorityPolicyV1,
    provider: &AccountId,
) -> Result<XorQuantity, InstructionExecutionError> {
    let asset_id = AssetId::of(policy.asset_definition.clone(), provider.clone());
    world.assets().get(&asset_id).map_or_else(
        || Ok(XorQuantity::zero()),
        |value| {
            XorQuantity::try_from_quantity(value.as_ref().clone()).map_err(|error| {
                corrupt_state(format!(
                    "stored reserve provider spendable balance is invalid: {error}"
                ))
            })
        },
    )
}

impl Execute for SetSorafsReservePolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        self.policy
            .validate()
            .map_err(|error| invalid_parameter(format!("invalid reserve policy: {error}")))?;
        state_transaction
            .world
            .account(&self.policy.custody_account)?;
        state_transaction
            .world
            .account(&self.policy.treasury_account)?;
        state_transaction
            .world
            .asset_definition(&self.policy.asset_definition)?;
        let now = now_unix(state_transaction)?;
        let digest = self.policy.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest reserve policy: {error}"))
        })?;
        match read_policy(state_transaction.world())? {
            None => {
                if self.policy.revision != 1 || self.policy.predecessor_policy_digest.is_some() {
                    return Err(invalid_parameter(
                        "first reserve policy must be revision one without a predecessor",
                    ));
                }
            }
            Some(current) => {
                if now < current.activated_at_unix {
                    return Err(invalid_parameter(format!(
                        "reserve policy activation timestamp {now} predates active policy activation {}",
                        current.activated_at_unix
                    )));
                }
                let expected = current
                    .policy
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("reserve policy revision overflow"))?;
                if self.policy.revision != expected
                    || self.policy.predecessor_policy_digest != Some(current.policy_digest)
                {
                    return Err(invalid_parameter(
                        "reserve policy must exactly extend the active revision and digest",
                    ));
                }
                ensure_apr_rotation_has_no_debt(
                    state_transaction.world(),
                    &current.policy,
                    &self.policy,
                )?;
                if self.policy.asset_definition != current.policy.asset_definition
                    || self.policy.custody_account != current.policy.custody_account
                    || self.policy.treasury_account != current.policy.treasury_account
                {
                    return Err(invalid_parameter(
                        "reserve asset and custody accounts are immutable after activation",
                    ));
                }
            }
        }
        let record = ReserveAuthorityPolicyRecordV1 {
            policy: self.policy,
            policy_digest: digest,
            activated_by: authority.clone(),
            activated_at_unix: now,
        };
        emit_reserve_policy_activation(state_transaction, record, authority, now)?;
        Ok(())
    }
}

impl Execute for RegisterSorafsReserveAccount {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_operations_authority(authority, &policy)?;
        if self.terms.capacity_gib == 0 {
            return Err(invalid_parameter(
                "reserve account capacity must be non-zero",
            ));
        }
        if self.terms.provider_account == policy.policy.custody_account {
            return Err(invalid_parameter(
                "reserve provider account must differ from protocol custody",
            ));
        }
        let owner = registered_provider_owner(state_transaction.world(), self.terms.provider_id)?;
        if owner.subject_id() != self.terms.provider_account.subject_id() {
            return Err(invalid_parameter(
                "reserve account provider does not match the registry owner",
            ));
        }
        if read_provider(state_transaction.world(), self.terms.provider_id)?.is_some() {
            return Err(invalid_parameter(
                "reserve provider account is already registered",
            ));
        }
        let account = ReserveProviderAccountV1 {
            credit_cap: credit_cap(&policy.policy, &self.terms)?,
            terms: self.terms,
            policy_digest: policy.policy_digest,
            revision: 1,
            reserve_balance: XorQuantity::zero(),
            debt_principal: XorQuantity::zero(),
            accrued_interest: XorQuantity::zero(),
            lifecycle_stage: ReserveLifecycleStage::Warning,
            days_past_due: 0,
            pending_movements: 0,
            open_appeals: 0,
            rent_charged_through_unix: now,
            interest_accrued_at_unix: now,
            updated_at_unix: now,
        };
        let encoded = encode_state(&account, "reserve provider account")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(account.terms.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::ProviderRegistered,
            account.terms.provider_id,
            None,
            account.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for RequestSorafsReserveMovement {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        if self.movement_id == [0; 32] || self.amount.is_zero() {
            return Err(invalid_parameter(
                "reserve movement id and amount must be non-zero",
            ));
        }
        if read_movement(state_transaction.world(), self.movement_id)?.is_some() {
            return Err(invalid_parameter("reserve movement id is already recorded"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve movement authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        if account.pending_movements >= policy.policy.max_pending_movements_per_provider {
            return Err(invalid_parameter(
                "reserve provider reached the pending-movement ceiling",
            ));
        }
        account.pending_movements = account
            .pending_movements
            .checked_add(1)
            .ok_or_else(|| corrupt_state("reserve pending-movement counter overflow"))?;
        advance_provider_revision(&mut account, now)?;
        let record = ReserveMovementRecordV1 {
            movement_id: self.movement_id,
            provider_id: self.provider_id,
            kind: self.kind,
            amount: self.amount,
            requested_by: authority.clone(),
            expected_provider_revision: self.expected_provider_revision,
            policy_digest: policy.policy_digest,
            status: ReserveMovementStatusV1::Pending,
            requested_at_unix: now,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        };
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_record = encode_state(&record, "reserve movement")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(movement_key(self.movement_id), encoded_record);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::MovementRequested,
            self.provider_id,
            Some(self.movement_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for DecideSorafsReserveMovement {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.rationale.is_empty() || self.rationale.len() > RESERVE_MAX_REASON_BYTES_V1 {
            return Err(invalid_parameter(
                "reserve movement rationale is empty or oversized",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_decision_authority(authority, &policy)?;
        let mut movement = read_movement(state_transaction.world(), self.movement_id)?
            .ok_or_else(|| invalid_parameter("reserve movement not found"))?;
        if movement.status != ReserveMovementStatusV1::Pending {
            return Err(invalid_parameter("reserve movement is already decided"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), movement.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        account.pending_movements = account
            .pending_movements
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("reserve pending-movement counter underflow"))?;

        let mut updated_credit = None;
        if self.approve {
            match movement.kind {
                ReserveMovementKindV1::TopUp => {
                    account.reserve_balance = account
                        .reserve_balance
                        .checked_add(&movement.amount)
                        .map_err(|error| {
                            invalid_parameter(format!("reserve balance overflow: {error}"))
                        })?;
                }
                ReserveMovementKindV1::Withdrawal => {
                    if !account
                        .total_debt()
                        .map_err(|error| {
                            invalid_parameter(format!("invalid provider debt: {error}"))
                        })?
                        .is_zero()
                    {
                        return Err(invalid_parameter(
                            "reserve withdrawal is forbidden while provider debt is outstanding",
                        ));
                    }
                    let remaining = account
                        .reserve_balance
                        .checked_sub(&movement.amount)
                        .map_err(|_| {
                            invalid_parameter("reserve withdrawal exceeds provider partition")
                        })?;
                    let quote = policy
                        .policy
                        .economics
                        .quote(
                            account.terms.storage_class,
                            account.terms.capacity_gib,
                            account.terms.duration,
                            account.terms.tier,
                            remaining.clone(),
                        )
                        .map_err(|error| {
                            invalid_parameter(format!("reserve withdrawal quote failed: {error}"))
                        })?;
                    if !quote
                        .ledger_projection()
                        .map_err(|error| {
                            invalid_parameter(format!(
                                "reserve withdrawal projection failed: {error}"
                            ))
                        })?
                        .meets_underwriting
                    {
                        return Err(invalid_parameter(
                            "reserve withdrawal would breach underwriting",
                        ));
                    }
                    updated_credit = credit_after_verified_reserve_withdrawal(
                        state_transaction.world(),
                        movement.provider_id,
                        &account.reserve_balance,
                        &remaining,
                        &account.debt_principal,
                    )?;
                    account.reserve_balance = remaining;
                }
            }
        }
        advance_provider_revision(&mut account, now)?;
        movement.status = if self.approve {
            ReserveMovementStatusV1::Approved
        } else {
            ReserveMovementStatusV1::Rejected
        };
        movement.decided_by = Some(authority.clone());
        movement.decided_at_unix = Some(now);
        movement.rationale = Some(self.rationale);
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_movement = encode_state(&movement, "reserve movement")?;

        if self.approve {
            match movement.kind {
                ReserveMovementKindV1::TopUp => transfer(
                    state_transaction,
                    &policy.policy,
                    &account.terms.provider_account,
                    &policy.policy.custody_account,
                    &movement.amount,
                )?,
                ReserveMovementKindV1::Withdrawal => {
                    let source_id = AssetId::of(
                        policy.policy.asset_definition.clone(),
                        policy.policy.custody_account.clone(),
                    );
                    let destination_id = AssetId::of(
                        policy.policy.asset_definition.clone(),
                        account.terms.provider_account.clone(),
                    );
                    let authorization = VerifiedSorafsReserveWithdrawal::new(
                        movement.provider_id,
                        movement.movement_id,
                        policy.policy_digest,
                        self.expected_provider_revision,
                        authority.clone(),
                        source_id,
                        destination_id,
                        movement.amount.clone().into_quantity(),
                    );
                    super::asset::isi::execute_verified_sorafs_reserve_withdrawal(
                        state_transaction,
                        authorization,
                    )?;
                }
            }
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(movement.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(movement_key(movement.movement_id), encoded_movement);
        if let Some(credit) = updated_credit {
            state_transaction
                .world
                .provider_credit_ledger
                .insert(movement.provider_id, credit);
        }
        emit_reserve_event(
            state_transaction,
            if movement.status == ReserveMovementStatusV1::Approved {
                SorafsReserveLedgerEventKind::MovementApproved
            } else {
                SorafsReserveLedgerEventKind::MovementRejected
            },
            movement.provider_id,
            Some(movement.movement_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for ChargeSorafsReserveRent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !(1..=RESERVE_RENT_MAX_BILLING_PERIODS_V1).contains(&self.billing_periods) {
            return Err(invalid_parameter(
                "reserve rent billing periods exceed the native V1 bound",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_operations_authority(authority, &policy)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        ensure_provider_timestamp(&account, now)?;
        let due_periods = account.rent_periods_due_at(now).map_err(|error| {
            invalid_parameter(format!("reserve rent anchor is invalid: {error}"))
        })?;
        if u64::from(self.billing_periods) > due_periods {
            return Err(invalid_parameter(format!(
                "reserve rent charge requests {} periods but only {due_periods} are due",
                self.billing_periods
            )));
        }
        let anchor_advance = RESERVE_RENT_BILLING_PERIOD_SECONDS_V1
            .checked_mul(u64::from(self.billing_periods))
            .ok_or_else(|| corrupt_state("reserve rent anchor multiplication overflow"))?;
        let next_rent_anchor = account
            .rent_charged_through_unix
            .checked_add(anchor_advance)
            .ok_or_else(|| corrupt_state("reserve rent anchor overflow"))?;
        if next_rent_anchor > now {
            return Err(corrupt_state(
                "reserve rent charge would advance beyond the current block",
            ));
        }
        accrue_interest(&mut account, &policy.policy, now)?;
        let quote = policy
            .policy
            .economics
            .quote(
                account.terms.storage_class,
                account.terms.capacity_gib,
                account.terms.duration,
                account.terms.tier,
                account.reserve_balance.clone(),
            )
            .map_err(|error| invalid_parameter(format!("reserve rent quote failed: {error}")))?;
        let rent = quote
            .effective_rent
            .checked_mul_u64(u64::from(self.billing_periods))
            .map_err(|error| invalid_parameter(format!("reserve rent overflow: {error}")))?;
        account.days_past_due = 0;
        account.lifecycle_stage = if quote
            .ledger_projection()
            .map_err(|error| invalid_parameter(format!("reserve projection failed: {error}")))?
            .needs_top_up_alert
        {
            ReserveLifecycleStage::Warning
        } else {
            ReserveLifecycleStage::Active
        };
        account.rent_charged_through_unix = next_rent_anchor;
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        if !rent.is_zero() {
            transfer(
                state_transaction,
                &policy.policy,
                &account.terms.provider_account,
                &policy.policy.treasury_account,
                &rent,
            )?;
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::RentCharged,
            self.provider_id,
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for AdvanceSorafsReserveLifecycle {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_operations_authority(authority, &policy)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        ensure_provider_timestamp(&account, now)?;
        let derived_days_past_due = account.rent_days_past_due_at(now).map_err(|error| {
            invalid_parameter(format!("reserve lifecycle rent anchor is invalid: {error}"))
        })?;
        if self.days_past_due != derived_days_past_due {
            return Err(invalid_parameter(format!(
                "reserve lifecycle days must equal authoritative rent aging: supplied {}, expected {derived_days_past_due}",
                self.days_past_due
            )));
        }
        let quote = policy
            .policy
            .economics
            .quote(
                account.terms.storage_class,
                account.terms.capacity_gib,
                account.terms.duration,
                account.terms.tier,
                account.reserve_balance.clone(),
            )
            .map_err(|error| {
                invalid_parameter(format!("reserve lifecycle quote failed: {error}"))
            })?;
        let periods_due = account.rent_periods_due_at(now).map_err(|error| {
            invalid_parameter(format!("reserve lifecycle rent anchor is invalid: {error}"))
        })?;
        if periods_due != 0 {
            let spendable_balance = provider_spendable_balance(
                state_transaction.world(),
                &policy.policy,
                &account.terms.provider_account,
            )?;
            if quote.effective_rent.is_zero() || &spendable_balance >= &quote.effective_rent {
                return Err(invalid_parameter(
                    "reserve lifecycle cannot age while one whole rent period is affordable",
                ));
            }
        }
        let projection = quote
            .lifecycle_projection(
                self.days_past_due,
                policy.policy.grace_period_days,
                policy.policy.default_after_days,
            )
            .map_err(|error| {
                invalid_parameter(format!("reserve lifecycle projection failed: {error}"))
            })?;
        if account.lifecycle_stage == projection.stage
            && account.days_past_due == derived_days_past_due
        {
            return Err(invalid_parameter(
                "reserve lifecycle transition would not change authoritative state",
            ));
        }
        accrue_interest(&mut account, &policy.policy, now)?;
        account.lifecycle_stage = projection.stage;
        account.days_past_due = derived_days_past_due;
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::LifecycleAdvanced,
            self.provider_id,
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for DrawSorafsReserveCredit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.amount.is_zero() {
            return Err(invalid_parameter("reserve credit draw must be non-zero"));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_operations_authority(authority, &policy)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        if self.amount
            > account
                .available_credit()
                .map_err(|error| invalid_parameter(format!("invalid provider credit: {error}")))?
        {
            return Err(invalid_parameter(
                "reserve credit draw exceeds the provider credit cap",
            ));
        }
        account.debt_principal = account
            .debt_principal
            .checked_add(&self.amount)
            .map_err(|error| invalid_parameter(format!("reserve debt overflow: {error}")))?;
        account.reserve_balance = account
            .reserve_balance
            .checked_add(&self.amount)
            .map_err(|error| invalid_parameter(format!("reserve balance overflow: {error}")))?;
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        transfer(
            state_transaction,
            &policy.policy,
            &policy.policy.treasury_account,
            &policy.policy.custody_account,
            &self.amount,
        )?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::CreditDrawn,
            self.provider_id,
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for RepaySorafsReserveCredit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.amount.is_zero() {
            return Err(invalid_parameter(
                "reserve credit repayment must be non-zero",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve credit repayment authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        let total_debt = account
            .total_debt()
            .map_err(|error| invalid_parameter(format!("invalid reserve debt: {error}")))?;
        if self.amount > total_debt {
            return Err(invalid_parameter(
                "reserve credit repayment exceeds total debt",
            ));
        }
        let interest_payment = XorQuantity::min(&self.amount, &account.accrued_interest);
        account.accrued_interest = account
            .accrued_interest
            .checked_sub(&interest_payment)
            .map_err(|error| corrupt_state(format!("reserve interest underflow: {error}")))?;
        let principal_payment = self
            .amount
            .checked_sub(&interest_payment)
            .map_err(|error| corrupt_state(format!("reserve repayment underflow: {error}")))?;
        account.debt_principal = account
            .debt_principal
            .checked_sub(&principal_payment)
            .map_err(|error| corrupt_state(format!("reserve principal underflow: {error}")))?;
        let governed_cap = credit_cap(&policy.policy, &account.terms)?;
        account.credit_cap = if account.debt_principal > governed_cap {
            account.debt_principal.clone()
        } else {
            governed_cap
        };
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        transfer(
            state_transaction,
            &policy.policy,
            &account.terms.provider_account,
            &policy.policy.treasury_account,
            &self.amount,
        )?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::CreditRepaid,
            self.provider_id,
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for SubmitSorafsReserveAppeal {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        if self.appeal_id == [0; 32]
            || self.reason.is_empty()
            || self.reason.len() > RESERVE_MAX_REASON_BYTES_V1
            || self.evidence_digest == Some([0; 32])
        {
            return Err(invalid_parameter(
                "reserve appeal id, reason, or evidence digest is invalid",
            ));
        }
        if read_appeal(state_transaction.world(), self.appeal_id)?.is_some() {
            return Err(invalid_parameter("reserve appeal id is already recorded"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve appeal authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        if account.open_appeals >= policy.policy.max_open_appeals_per_provider {
            return Err(invalid_parameter(
                "reserve provider reached the open-appeal ceiling",
            ));
        }
        account.open_appeals = account
            .open_appeals
            .checked_add(1)
            .ok_or_else(|| corrupt_state("reserve open-appeal counter overflow"))?;
        advance_provider_revision(&mut account, now)?;
        let appeal = ReserveAppealRecordV1 {
            appeal_id: self.appeal_id,
            provider_id: self.provider_id,
            submitted_by: authority.clone(),
            requested_stage: self.requested_stage,
            reason: self.reason,
            evidence_digest: self.evidence_digest,
            expected_provider_revision: self.expected_provider_revision,
            status: ReserveAppealStatusV1::Pending,
            submitted_at_unix: now,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        };
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_appeal = encode_state(&appeal, "reserve appeal")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(appeal_key(self.appeal_id), encoded_appeal);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::AppealSubmitted,
            self.provider_id,
            Some(self.appeal_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

impl Execute for DecideSorafsReserveAppeal {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.rationale.is_empty() || self.rationale.len() > RESERVE_MAX_REASON_BYTES_V1 {
            return Err(invalid_parameter(
                "reserve appeal rationale is empty or oversized",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        require_decision_authority(authority, &policy)?;
        let mut appeal = read_appeal(state_transaction.world(), self.appeal_id)?
            .ok_or_else(|| invalid_parameter("reserve appeal not found"))?;
        if appeal.status != ReserveAppealStatusV1::Pending {
            return Err(invalid_parameter("reserve appeal is already decided"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), appeal.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        account.open_appeals = account
            .open_appeals
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("reserve open-appeal counter underflow"))?;
        if self.accept {
            account.lifecycle_stage = appeal.requested_stage;
        }
        advance_provider_revision(&mut account, now)?;
        appeal.status = if self.accept {
            ReserveAppealStatusV1::Accepted
        } else {
            ReserveAppealStatusV1::Rejected
        };
        appeal.decided_by = Some(authority.clone());
        appeal.decided_at_unix = Some(now);
        appeal.rationale = Some(self.rationale);
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_appeal = encode_state(&appeal, "reserve appeal")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(appeal.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(appeal_key(appeal.appeal_id), encoded_appeal);
        emit_reserve_event(
            state_transaction,
            if appeal.status == ReserveAppealStatusV1::Accepted {
                SorafsReserveLedgerEventKind::AppealAccepted
            } else {
                SorafsReserveLedgerEventKind::AppealRejected
            },
            appeal.provider_id,
            Some(appeal.appeal_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        )?;
        Ok(())
    }
}

fn query_failure(error: impl core::fmt::Display) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

const RESERVE_QUERY_MAX_EVENT_READ_BYTES_V1: usize = RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 * 4;
const RESERVE_QUERY_MAX_EVENT_STORAGE_PROBES_V1: u32 = RESERVE_QUERY_MAX_ITEMS_V1 * 2 + 24;
const RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1: usize = 1_024;
const RESERVE_QUERY_MAX_EVENT_TOTAL_KEY_BYTES_V1: usize = 128 * 1_024;
const RESERVE_QUERY_MAX_RECORD_BYTES_V1: usize = 64 * 1_024;

#[derive(Debug, Default)]
struct ReserveEventQueryBudgetV1 {
    storage_probes: u32,
    key_bytes: usize,
    read_bytes: usize,
}

impl ReserveEventQueryBudgetV1 {
    fn inspect_storage_probe(
        &mut self,
        probe_key_bytes: usize,
        returned_key_bytes: Option<usize>,
        encoded_value_bytes: usize,
    ) -> Result<(), QueryExecutionFail> {
        if probe_key_bytes > RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1
            || returned_key_bytes
                .is_some_and(|bytes| bytes > RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1)
        {
            return Err(QueryExecutionFail::Conversion(format!(
                "reserve committed-event query probe key exceeds {RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1} bytes"
            )));
        }
        self.storage_probes = self.storage_probes.checked_add(1).ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reserve committed-event query storage-probe counter overflow".to_owned(),
            )
        })?;
        self.key_bytes = self
            .key_bytes
            .checked_add(probe_key_bytes)
            .and_then(|bytes| bytes.checked_add(returned_key_bytes.unwrap_or(0)))
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "reserve committed-event query key-byte counter overflow".to_owned(),
                )
            })?;
        self.read_bytes = self
            .read_bytes
            .checked_add(encoded_value_bytes)
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "reserve committed-event query read-byte counter overflow".to_owned(),
                )
            })?;
        if self.storage_probes > RESERVE_QUERY_MAX_EVENT_STORAGE_PROBES_V1 {
            return Err(QueryExecutionFail::Conversion(format!(
                "reserve committed-event query exceeds {RESERVE_QUERY_MAX_EVENT_STORAGE_PROBES_V1} storage probes"
            )));
        }
        if self.key_bytes > RESERVE_QUERY_MAX_EVENT_TOTAL_KEY_BYTES_V1 {
            return Err(QueryExecutionFail::Conversion(format!(
                "reserve committed-event query exceeds {RESERVE_QUERY_MAX_EVENT_TOTAL_KEY_BYTES_V1} probed key bytes"
            )));
        }
        if self.read_bytes > RESERVE_QUERY_MAX_EVENT_READ_BYTES_V1 {
            return Err(QueryExecutionFail::Conversion(format!(
                "reserve committed-event query exceeds {RESERVE_QUERY_MAX_EVENT_READ_BYTES_V1} encoded read bytes"
            )));
        }
        Ok(())
    }

    fn inspect_direct_read(
        &mut self,
        key: &StatePath,
        encoded_value_bytes: usize,
    ) -> Result<(), QueryExecutionFail> {
        self.inspect_storage_probe(key.as_ref().len(), None, encoded_value_bytes)
    }

    fn inspect_finalized_hash_read(
        &mut self,
        encoded_value_bytes: usize,
    ) -> Result<(), QueryExecutionFail> {
        self.inspect_storage_probe(core::mem::size_of::<u64>(), None, encoded_value_bytes)
    }

    fn inspect_finalized_hash_metadata(&mut self) -> Result<(), QueryExecutionFail> {
        self.inspect_storage_probe(
            "finalized_block_hashes".len(),
            None,
            core::mem::size_of::<u64>(),
        )
    }

    fn inspect_range_read(
        &mut self,
        probe_key: &StatePath,
        result: Option<(&StatePath, usize)>,
    ) -> Result<(), QueryExecutionFail> {
        self.inspect_storage_probe(
            probe_key.as_ref().len(),
            result.map(|(key, _)| key.as_ref().len()),
            result.map_or(0, |(_, bytes)| bytes),
        )
    }
}

fn read_policy_for_query(
    world: &impl WorldReadOnly,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<Option<ReserveAuthorityPolicyRecordV1>, QueryExecutionFail> {
    let key = reserve_state_key();
    let bytes = world.smart_contract_state().get(key);
    budget.inspect_direct_read(key, bytes.map_or(0, |bytes| bytes.len()))?;
    bytes
        .map(|bytes| decode_reserve_state(bytes).map(|state| state.policy))
        .transpose()
        .map_err(query_failure)
}

fn checked_query_limit(limit: u32) -> Result<usize, QueryExecutionFail> {
    if !(1..=RESERVE_QUERY_MAX_ITEMS_V1).contains(&limit) {
        return Err(QueryExecutionFail::Conversion(format!(
            "SoraFS reserve event query limit {limit} is outside 1..={RESERVE_QUERY_MAX_ITEMS_V1}"
        )));
    }
    usize::try_from(limit).map_err(|_| {
        QueryExecutionFail::Conversion(
            "SoraFS reserve event query limit conversion failed".to_owned(),
        )
    })
}

fn resolve_finalized_cursor(
    state_ro: &impl crate::state::StateReadOnly,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<ReserveFinalizedCursorV1, QueryExecutionFail> {
    let block_hashes = state_ro.block_hashes();
    budget.inspect_finalized_hash_metadata()?;
    let height = u64::try_from(block_hashes.len()).map_err(|_| {
        QueryExecutionFail::Conversion("finalized reserve height does not fit into u64".to_owned())
    })?;
    let terminal_hash = block_hashes.last();
    budget.inspect_finalized_hash_read(terminal_hash.map_or(0, |_| 32))?;
    let block_hash = terminal_hash.map(|hash| *hash.as_ref()).ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "finalized reserve queries require at least one committed block".to_owned(),
        )
    })?;
    if height == 0 || block_hash == [0; 32] {
        return Err(QueryExecutionFail::Conversion(
            "finalized reserve query anchor is invalid".to_owned(),
        ));
    }
    Ok(ReserveFinalizedCursorV1 { height, block_hash })
}

fn resolve_query_finalized_cursor(
    expected: Option<ReserveFinalizedCursorV1>,
    state_ro: &impl crate::state::StateReadOnly,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<ReserveFinalizedCursorV1, QueryExecutionFail> {
    let actual = resolve_finalized_cursor(state_ro, budget)?;
    if expected.is_some_and(|expected| expected != actual) {
        return Err(QueryExecutionFail::Expired);
    }
    Ok(actual)
}

fn read_persisted_event_for_query(
    world: &impl WorldReadOnly,
    sequence: u64,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<Option<ReservePersistedEventV1>, QueryExecutionFail> {
    if sequence == 0 {
        return Err(query_failure(
            "reserve event sequence zero cannot be queried",
        ));
    }
    let key = event_key(sequence);
    let bytes = world.smart_contract_state().get(&key);
    budget.inspect_direct_read(&key, bytes.map_or(0, |bytes| bytes.len()))?;
    bytes
        .map(|bytes| decode_persisted_event(bytes, sequence))
        .transpose()
        .map_err(query_failure)
}

fn read_event_journal_head_for_query(
    world: &impl WorldReadOnly,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<Option<ReserveEventJournalHeadV1>, QueryExecutionFail> {
    let key = reserve_state_key();
    let bytes = world.smart_contract_state().get(key);
    budget.inspect_direct_read(key, bytes.map_or(0, |bytes| bytes.len()))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let head = decode_reserve_state(bytes)
        .map_err(query_failure)?
        .journal_head;
    let record =
        read_persisted_event_for_query(world, head.last_sequence, budget)?.ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reserve event journal head references a missing event".to_owned(),
            )
        })?;
    if record.target_block_height != head.last_target_block_height
        || record.event_index != head.last_event_index
    {
        return Err(QueryExecutionFail::Conversion(
            "reserve event journal head does not match its terminal event".to_owned(),
        ));
    }
    let predecessor = if head.last_sequence == 1 {
        None
    } else {
        let predecessor_sequence = head.last_sequence - 1;
        Some(
            read_persisted_event_for_query(world, predecessor_sequence, budget)?.ok_or_else(
                || {
                    QueryExecutionFail::Conversion(format!(
                        "reserve event journal is missing terminal predecessor sequence {predecessor_sequence}"
                    ))
                },
            )?,
        )
    };
    validate_event_successor(predecessor.as_ref(), &record).map_err(query_failure)?;
    Ok(Some(head))
}

fn ensure_no_event_after_head_for_query(
    world: &impl WorldReadOnly,
    head: ReserveEventJournalHeadV1,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<(), QueryExecutionFail> {
    let prefix_start =
        StatePath::from_str(EVENT_STATE_KEY_PREFIX).expect("static event prefix is valid");
    let mut first_iter = world.smart_contract_state().range(prefix_start.clone()..);
    let first = first_iter.next();
    budget.inspect_range_read(&prefix_start, first.map(|(key, value)| (key, value.len())))?;
    let Some((first_key, _)) = first else {
        return Err(query_failure(
            "reserve event journal head exists without event records",
        ));
    };
    if !first_key.as_ref().starts_with(EVENT_STATE_KEY_PREFIX) || *first_key != event_key(1) {
        return Err(query_failure(
            "reserve event journal does not begin at sequence one",
        ));
    }

    let terminal_key = event_key(head.last_sequence);
    let mut tail_iter = world.smart_contract_state().range(terminal_key.clone()..);
    let mut probe_key = terminal_key.clone();
    loop {
        let next = tail_iter.next();
        budget.inspect_range_read(&probe_key, next.map(|(key, value)| (key, value.len())))?;
        let Some((key, _)) = next else {
            break;
        };
        if *key == terminal_key {
            probe_key = key.clone();
            continue;
        }
        if key.as_ref().starts_with(EVENT_STATE_KEY_PREFIX) {
            return Err(query_failure(
                "reserve event journal contains a record beyond its head",
            ));
        }
        break;
    }
    Ok(())
}

fn resolve_committed_event(
    state_ro: &impl crate::state::StateReadOnly,
    record: &ReservePersistedEventV1,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<ReserveFinalizedEventV1, QueryExecutionFail> {
    let hash_index = record
        .target_block_height
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reserve event target height cannot index finalized block hashes".to_owned(),
            )
        })?;
    let hash = state_ro.block_hashes().get(hash_index);
    budget.inspect_finalized_hash_read(hash.map_or(0, |_| 32))?;
    let block_hash = hash.map(|hash| *hash.as_ref()).ok_or_else(|| {
        QueryExecutionFail::Conversion(format!(
            "reserve event sequence {} targets non-finalized block height {}",
            record.sequence, record.target_block_height
        ))
    })?;
    if block_hash == [0; 32] {
        return Err(QueryExecutionFail::Conversion(format!(
            "reserve event sequence {} resolved a zero block hash",
            record.sequence
        )));
    }
    Ok(ReserveFinalizedEventV1 {
        sequence: record.sequence,
        block_height: record.target_block_height,
        block_hash,
        event_index: record.event_index,
        event: record.event.clone(),
    })
}

fn query_reserve_event_page(
    query: &FindSorafsReserveEvents,
    state_ro: &impl crate::state::StateReadOnly,
    finalized_cursor: ReserveFinalizedCursorV1,
    budget: &mut ReserveEventQueryBudgetV1,
) -> Result<ReserveFinalizedEventPageV1, QueryExecutionFail> {
    let limit = checked_query_limit(query.limit)?;
    let world = state_ro.world();
    if read_policy_for_query(world, budget)?.is_none() {
        return Err(QueryExecutionFail::Find(FindError::SorafsReservePolicy));
    }
    let head = read_event_journal_head_for_query(world, budget)?.ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "active reserve state has no committed-event journal".to_owned(),
        )
    })?;
    let first = read_persisted_event_for_query(world, 1, budget)?.ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "reserve event journal is missing initial policy activation".to_owned(),
        )
    })?;
    validate_event_successor(None, &first).map_err(query_failure)?;
    if first.event.kind != SorafsReserveLedgerEventKind::PolicyActivated {
        return Err(QueryExecutionFail::Conversion(
            "reserve event journal does not begin with policy activation".to_owned(),
        ));
    }
    resolve_committed_event(state_ro, &first, budget)?;
    let terminal =
        read_persisted_event_for_query(world, head.last_sequence, budget)?.ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reserve event journal terminal record disappeared during read".to_owned(),
            )
        })?;
    resolve_committed_event(state_ro, &terminal, budget)?;
    ensure_no_event_after_head_for_query(world, head, budget)?;
    let mut previous = match query.after {
        Some(after) => {
            if after.sequence == 0 || after.sequence > head.last_sequence {
                return Err(QueryExecutionFail::Expired);
            }
            let record = read_persisted_event_for_query(world, after.sequence, budget)?
                .ok_or(QueryExecutionFail::Expired)?;
            let resolved = resolve_committed_event(state_ro, &record, budget)?;
            if resolved.cursor() != after {
                return Err(QueryExecutionFail::Expired);
            }
            let predecessor = if after.sequence == 1 {
                None
            } else {
                let predecessor_sequence = after.sequence - 1;
                Some(
                    read_persisted_event_for_query(
                        world,
                        predecessor_sequence,
                        budget,
                    )?
                    .ok_or_else(|| {
                        QueryExecutionFail::Conversion(format!(
                            "reserve event journal is missing predecessor sequence {predecessor_sequence}"
                        ))
                    })?,
                )
            };
            validate_event_successor(predecessor.as_ref(), &record).map_err(query_failure)?;
            Some(record)
        }
        None => None,
    };
    let mut sequence = query
        .after
        .map_or(Some(1), |after| after.sequence.checked_add(1));
    let mut events = Vec::with_capacity(limit);
    let mut encoded_event_bytes = 0usize;
    while let Some(current_sequence) = sequence {
        if current_sequence > head.last_sequence || events.len() >= limit {
            break;
        }
        let record =
            read_persisted_event_for_query(world, current_sequence, budget)?.ok_or_else(|| {
                QueryExecutionFail::Conversion(format!(
                    "reserve event journal is missing sequence {current_sequence}"
                ))
            })?;
        validate_event_successor(previous.as_ref(), &record).map_err(query_failure)?;
        let resolved = resolve_committed_event(state_ro, &record, budget)?;
        encoded_event_bytes = encoded_event_bytes
            .checked_add(
                norito::to_bytes(&resolved)
                    .map_err(|error| {
                        QueryExecutionFail::Conversion(format!(
                            "failed to encode committed reserve event: {error}"
                        ))
                    })?
                    .len(),
            )
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "committed reserve event page byte counter overflow".to_owned(),
                )
            })?;
        if encoded_event_bytes > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
            return Err(QueryExecutionFail::Conversion(format!(
                "committed reserve event page exceeds {RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1} bytes"
            )));
        }
        previous = Some(record);
        events.push(resolved);
        sequence = current_sequence.checked_add(1);
    }
    let has_more = events
        .last()
        .is_some_and(|event| event.sequence < head.last_sequence);
    let next_after = has_more.then(|| {
        events
            .last()
            .expect("has_more requires a non-empty reserve event page")
            .cursor()
    });
    let page = ReserveFinalizedEventPageV1 {
        finalized_cursor,
        events,
        has_more,
        next_after,
    };
    let encoded_len = norito::to_bytes(&page)
        .map_err(|error| {
            QueryExecutionFail::Conversion(format!(
                "failed to encode committed reserve event page: {error}"
            ))
        })?
        .len();
    if encoded_len > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "committed reserve event page encodes to {encoded_len} bytes, above {RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1}"
        )));
    }
    Ok(page)
}

fn scan_reserve_records<T>(
    world: &impl WorldReadOnly,
    prefix: &str,
    start: StatePath,
    limit: usize,
    budget: &mut ReserveEventQueryBudgetV1,
    mut decode: impl FnMut(&StatePath, &[u8]) -> Result<Option<T>, QueryExecutionFail>,
) -> Result<Vec<T>, QueryExecutionFail> {
    let mut records = Vec::with_capacity(limit.saturating_add(1));
    let mut probe_key = start.clone();
    let mut iter = world.smart_contract_state().range(start..);
    loop {
        let next = iter.next();
        budget.inspect_range_read(&probe_key, next.map(|(key, payload)| (key, payload.len())))?;
        let Some((key, payload)) = next else {
            break;
        };
        probe_key = key.clone();
        if !key.as_ref().starts_with(prefix) {
            break;
        }
        if payload.len() > RESERVE_QUERY_MAX_RECORD_BYTES_V1 {
            return Err(QueryExecutionFail::Conversion(format!(
                "authoritative reserve record exceeds {RESERVE_QUERY_MAX_RECORD_BYTES_V1} bytes"
            )));
        }
        if let Some(record) = decode(key, payload)? {
            records.push(record);
            if records.len() > limit {
                break;
            }
        }
    }
    Ok(records)
}

fn checked_page_records<T, I: Copy>(
    mut records: Vec<T>,
    limit: usize,
    id: impl Fn(&T) -> I,
) -> Result<(Vec<T>, bool, Option<I>), QueryExecutionFail> {
    let has_more = records.len() > limit;
    if has_more {
        records.pop();
    }
    let next_after = if has_more {
        Some(id(records.last().ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "reserve record page continuation invariant failed".to_owned(),
            )
        })?))
    } else {
        None
    };
    Ok((records, has_more, next_after))
}

fn validate_encoded_record_page<T: norito::core::NoritoSerialize>(
    page: &T,
) -> Result<(), QueryExecutionFail> {
    let encoded_len = norito::to_bytes(page)
        .map_err(|error| {
            QueryExecutionFail::Conversion(format!(
                "failed to encode authoritative reserve record page: {error}"
            ))
        })?
        .len();
    if encoded_len > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
        return Err(QueryExecutionFail::Conversion(format!(
            "authoritative reserve record page encodes to {encoded_len} bytes, above {RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1}"
        )));
    }
    Ok(())
}

impl ValidSingularQuery for FindSorafsReservePolicy {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveAuthorityPolicyRecordV1, QueryExecutionFail> {
        read_policy(state_ro.world())
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsReservePolicy))
    }
}

impl ValidSingularQuery for FindSorafsReserveProviderById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveProviderAccountV1, QueryExecutionFail> {
        read_provider(state_ro.world(), self.provider_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsReserveProvider(self.provider_id))
            })
    }
}

impl ValidSingularQuery for FindSorafsReserveMovementById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveMovementRecordV1, QueryExecutionFail> {
        read_movement(state_ro.world(), self.movement_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsReserveMovement(self.movement_id))
            })
    }
}

impl ValidSingularQuery for FindSorafsReserveAppealById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveAppealRecordV1, QueryExecutionFail> {
        read_appeal(state_ro.world(), self.appeal_id)
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsReserveAppeal(self.appeal_id)))
    }
}

impl ValidSingularQuery for FindSorafsReserveProviders {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveProviderAccountPageV1, QueryExecutionFail> {
        let limit = checked_query_limit(self.limit)?;
        let mut budget = ReserveEventQueryBudgetV1::default();
        let finalized_cursor =
            resolve_query_finalized_cursor(self.expected_finalized_cursor, state_ro, &mut budget)?;
        let world = state_ro.world();
        if read_policy_for_query(world, &mut budget)?.is_none() {
            return Err(QueryExecutionFail::Find(FindError::SorafsReservePolicy));
        }
        let after = self.after_provider_id;
        let start = after.map_or_else(
            || {
                StatePath::from_str(PROVIDER_STATE_KEY_PREFIX)
                    .expect("static provider prefix is valid")
            },
            provider_key,
        );
        let accounts = scan_reserve_records(
            world,
            PROVIDER_STATE_KEY_PREFIX,
            start,
            limit,
            &mut budget,
            |key, payload| {
                let candidate: ReserveProviderAccountV1 =
                    decode_state(payload, "reserve provider account").map_err(query_failure)?;
                let provider_id = candidate.terms.provider_id;
                if provider_key(provider_id) != *key {
                    return Err(QueryExecutionFail::Conversion(
                        "authoritative reserve provider key does not match its account".to_owned(),
                    ));
                }
                let account =
                    decode_provider_record(payload, provider_id).map_err(query_failure)?;
                Ok((!after.is_some_and(|cursor| provider_id <= cursor)).then_some(account))
            },
        )?;
        let (accounts, has_more, next_after) =
            checked_page_records(accounts, limit, |account| account.terms.provider_id)?;
        let page = ReserveProviderAccountPageV1 {
            finalized_cursor,
            accounts,
            has_more,
            next_after,
        };
        validate_encoded_record_page(&page)?;
        Ok(page)
    }
}

impl ValidSingularQuery for FindSorafsReserveMovements {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveMovementPageV1, QueryExecutionFail> {
        let limit = checked_query_limit(self.limit)?;
        let mut budget = ReserveEventQueryBudgetV1::default();
        let finalized_cursor =
            resolve_query_finalized_cursor(self.expected_finalized_cursor, state_ro, &mut budget)?;
        let world = state_ro.world();
        if read_policy_for_query(world, &mut budget)?.is_none() {
            return Err(QueryExecutionFail::Find(FindError::SorafsReservePolicy));
        }
        let after = self.after_movement_id;
        let start = after.map_or_else(
            || {
                StatePath::from_str(MOVEMENT_STATE_KEY_PREFIX)
                    .expect("static movement prefix is valid")
            },
            movement_key,
        );
        let movements = scan_reserve_records(
            world,
            MOVEMENT_STATE_KEY_PREFIX,
            start,
            limit,
            &mut budget,
            |key, payload| {
                let candidate: ReserveMovementRecordV1 =
                    decode_state(payload, "reserve movement").map_err(query_failure)?;
                let movement_id = candidate.movement_id;
                if movement_key(movement_id) != *key {
                    return Err(QueryExecutionFail::Conversion(
                        "authoritative reserve movement key does not match its record".to_owned(),
                    ));
                }
                let movement =
                    decode_movement_record(payload, movement_id).map_err(query_failure)?;
                Ok((!after.is_some_and(|cursor| movement_id <= cursor)).then_some(movement))
            },
        )?;
        let (movements, has_more, next_after) =
            checked_page_records(movements, limit, |movement| movement.movement_id)?;
        let page = ReserveMovementPageV1 {
            finalized_cursor,
            movements,
            has_more,
            next_after,
        };
        validate_encoded_record_page(&page)?;
        Ok(page)
    }
}

impl ValidSingularQuery for FindSorafsReserveAppeals {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveAppealPageV1, QueryExecutionFail> {
        let limit = checked_query_limit(self.limit)?;
        let mut budget = ReserveEventQueryBudgetV1::default();
        let finalized_cursor =
            resolve_query_finalized_cursor(self.expected_finalized_cursor, state_ro, &mut budget)?;
        let world = state_ro.world();
        if read_policy_for_query(world, &mut budget)?.is_none() {
            return Err(QueryExecutionFail::Find(FindError::SorafsReservePolicy));
        }
        let after = self.after_appeal_id;
        let start = after.map_or_else(
            || StatePath::from_str(APPEAL_STATE_KEY_PREFIX).expect("static appeal prefix is valid"),
            appeal_key,
        );
        let appeals = scan_reserve_records(
            world,
            APPEAL_STATE_KEY_PREFIX,
            start,
            limit,
            &mut budget,
            |key, payload| {
                let candidate: ReserveAppealRecordV1 =
                    decode_state(payload, "reserve appeal").map_err(query_failure)?;
                let appeal_id = candidate.appeal_id;
                if appeal_key(appeal_id) != *key {
                    return Err(QueryExecutionFail::Conversion(
                        "authoritative reserve appeal key does not match its record".to_owned(),
                    ));
                }
                let appeal = decode_appeal_record(payload, appeal_id).map_err(query_failure)?;
                Ok((!after.is_some_and(|cursor| appeal_id <= cursor)).then_some(appeal))
            },
        )?;
        let (appeals, has_more, next_after) =
            checked_page_records(appeals, limit, |appeal| appeal.appeal_id)?;
        let page = ReserveAppealPageV1 {
            finalized_cursor,
            appeals,
            has_more,
            next_after,
        };
        validate_encoded_record_page(&page)?;
        Ok(page)
    }
}

impl ValidSingularQuery for FindSorafsReserveEvents {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveFinalizedEventPageV1, QueryExecutionFail> {
        let mut budget = ReserveEventQueryBudgetV1::default();
        let finalized_cursor =
            resolve_query_finalized_cursor(self.expected_finalized_cursor, state_ro, &mut budget)?;
        query_reserve_event_page(self, state_ro, finalized_cursor, &mut budget)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey};
    use iroha_data_model::{
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{Burn, Transfer, Unregister},
        permission::{Permission, Permissions},
        sorafs::{
            pin_registry::StorageClass,
            reserve::{
                ClassRentRate, RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveDuration,
                ReservePolicyV1, ReserveProviderTermsV1, ReserveTier,
            },
        },
    };
    use iroha_primitives::{json::Json, numeric::Quantity};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const NOW: u64 = 20_000;
    const PROVIDER_ID: ProviderId = ProviderId::new([0x61; 32]);

    fn keypair(seed: u8) -> KeyPair {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("valid deterministic Ed25519 seed");
        KeyPair::from_private_key(private).expect("derive deterministic keypair")
    }

    fn account(keypair: &KeyPair) -> AccountId {
        AccountId::new(keypair.public_key().clone())
    }

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("reserve", "universal").expect("reserve domain"),
            "xor".parse().expect("reserve asset"),
        )
    }

    fn quantity_micro(micro: u128) -> Quantity {
        XorQuantity::try_from_micro(micro)
            .expect("micro-XOR fixture")
            .into_quantity()
    }

    fn xor_micro(micro: u128) -> XorQuantity {
        XorQuantity::try_from_micro(micro).expect("micro-XOR reserve fixture")
    }

    fn policy(
        revision: u64,
        predecessor_policy_digest: Option<[u8; 32]>,
        custody_account: AccountId,
        treasury_account: AccountId,
        service_authority: &AccountId,
    ) -> ReserveAuthorityPolicyV1 {
        ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision,
            predecessor_policy_digest,
            economics: ReservePolicyV1::default(),
            asset_definition: asset_definition(),
            custody_account,
            treasury_account,
            operations_authority: service_authority.clone(),
            decision_authority: service_authority.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: xor_micro(1_000_000_000),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        }
    }

    fn state_fixture(
        governance: &AccountId,
        provider: &AccountId,
        custody: &AccountId,
        treasury: &AccountId,
    ) -> State {
        state_fixture_with_provider_balance(
            governance,
            provider,
            custody,
            treasury,
            quantity_micro(100_000_000),
        )
    }

    fn state_fixture_with_provider_balance(
        governance: &AccountId,
        provider: &AccountId,
        custody: &AccountId,
        treasury: &AccountId,
        provider_balance: Quantity,
    ) -> State {
        let definition_id = asset_definition();
        let domain =
            Domain::new(DomainId::try_new("reserve", "universal").expect("reserve domain"))
                .build(governance);
        let definition = AssetDefinition::numeric(
            definition_id.clone(),
            "XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(governance);
        let provider_asset = Asset::new(
            AssetId::of(definition_id.clone(), provider.clone()),
            provider_balance,
        );
        let treasury_asset = Asset::new(
            AssetId::of(definition_id, treasury.clone()),
            quantity_micro(100_000_000),
        );
        let mut world = World::with_assets(
            [domain],
            [
                Account::new(governance.clone()).build(governance),
                Account::new(provider.clone()).build(provider),
                Account::new(custody.clone()).build(custody),
                Account::new(treasury.clone()).build(treasury),
            ],
            [definition],
            [provider_asset, treasury_asset],
            [],
        );
        let mut permissions = Permissions::new();
        permissions.insert(Permission::new(
            "CanSetSorafsReservePolicy".to_owned(),
            Json::new(()),
        ));
        world
            .account_permissions
            .insert(governance.clone(), permissions);
        world.provider_owners.insert(PROVIDER_ID, provider.clone());
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn terms(provider_account: AccountId) -> ReserveProviderTermsV1 {
        terms_for(PROVIDER_ID, provider_account)
    }

    fn terms_for(provider_id: ProviderId, provider_account: AccountId) -> ReserveProviderTermsV1 {
        ReserveProviderTermsV1 {
            provider_id,
            provider_account,
            tier: ReserveTier::TierA,
            storage_class: StorageClass::Hot,
            duration: ReserveDuration::Monthly,
            capacity_gib: 10,
        }
    }

    fn block_header_at(height: u64, now_unix: u64) -> BlockHeader {
        BlockHeader::new(
            height.try_into().expect("nonzero fixture block height"),
            None,
            None,
            None,
            now_unix * 1_000,
            0,
        )
    }

    fn transact(
        state: &mut State,
        height: u64,
        now_unix: u64,
        operation: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<(), InstructionExecutionError>,
    ) -> Result<(), InstructionExecutionError> {
        let header = block_header_at(height, now_unix);
        let mut block = state.block(header.clone());
        let mut transaction = block.transaction();
        operation(&mut transaction)?;
        transaction.apply();
        block.commit().expect("commit reserve test block");
        state.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        Ok(())
    }

    fn reserve_asset_balance(state: &State, owner: &AccountId) -> XorQuantity {
        let view = state.view();
        let asset_id = AssetId::of(asset_definition(), owner.clone());
        view.world()
            .assets()
            .get(&asset_id)
            .map_or_else(XorQuantity::zero, |value| {
                XorQuantity::try_from_quantity(value.as_ref().clone())
                    .expect("stored reserve asset is canonical")
            })
    }

    #[test]
    fn reserve_custody_rejects_user_debits_but_allows_exact_approved_withdrawal() {
        let governance = account(&keypair(0x51));
        let provider = account(&keypair(0x52));
        let custody = account(&keypair(0x53));
        let treasury = account(&keypair(0x54));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let top_up = xor_micro(50_000_000);
        let slash_lien = xor_micro(10_000_000);
        let withdrawal = xor_micro(1);

        transact(&mut state, 1, NOW, |transaction| {
            transaction.tx_call_hash = Some(Hash::prehashed([0x52; Hash::LENGTH]));
            let configured = policy(1, None, custody.clone(), treasury.clone(), &governance);
            let policy_digest = configured.digest().expect("reserve policy digest");
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider.clone()), policy_digest)
                .execute(&governance, transaction)?;
            RequestSorafsReserveMovement::new(
                [0x53; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                top_up.clone(),
                1,
                policy_digest,
            )
            .execute(&provider, transaction)?;
            DecideSorafsReserveMovement::new(
                [0x53; 32],
                2,
                policy_digest,
                true,
                "fund native reserve custody".to_owned(),
            )
            .execute(&governance, transaction)?;

            let custody_asset = AssetId::of(asset_definition(), custody.clone());
            let transfer_error = Transfer::asset_quantity(
                custody_asset.clone(),
                quantity_micro(1),
                provider.clone(),
            )
            .execute(&custody, transaction)
            .expect_err("ordinary transfer must not debit reserve custody");
            assert!(
                transfer_error
                    .to_string()
                    .contains("SoraFS reserve custody")
            );
            let burn_error = Burn::asset_quantity(quantity_micro(1), custody_asset)
                .execute(&custody, transaction)
                .expect_err("ordinary burn must not debit reserve custody");
            assert!(burn_error.to_string().contains("SoraFS reserve custody"));
            let account_error = Unregister::account(custody.clone())
                .execute(&governance, transaction)
                .expect_err("active reserve custody account must remain registered");
            assert!(account_error.to_string().contains("SoraFS reserve custody"));
            let definition_error = Unregister::asset_definition(asset_definition())
                .execute(&governance, transaction)
                .expect_err("active reserve asset definition must remain registered");
            assert!(
                definition_error
                    .to_string()
                    .contains("SoraFS reserve custody")
            );

            let mut credit = ProviderCreditRecord::new(
                PROVIDER_ID,
                Quantity::zero(),
                top_up.clone().into_quantity(),
                Quantity::zero(),
                Quantity::zero(),
                0,
                0,
                iroha_data_model::metadata::Metadata::default(),
            );
            credit
                .apply_penalty(&slash_lien.clone().into_quantity(), 1)
                .expect("apply custody-backed slash lien");
            transaction
                .world
                .provider_credit_ledger
                .insert(PROVIDER_ID, credit);

            RequestSorafsReserveMovement::new(
                [0x54; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::Withdrawal,
                withdrawal.clone(),
                3,
                policy_digest,
            )
            .execute(&provider, transaction)?;
            DecideSorafsReserveMovement::new(
                [0x54; 32],
                4,
                policy_digest,
                true,
                "release exact approved withdrawal".to_owned(),
            )
            .execute(&governance, transaction)?;
            Ok(())
        })
        .expect("reserve custody flow");

        assert_eq!(
            reserve_asset_balance(&state, &custody),
            top_up.checked_sub(&withdrawal).expect("bounded withdrawal")
        );
        assert_eq!(
            verified_provider_bond(state.view().world(), PROVIDER_ID, &provider, 10)
                .expect("remaining native reserve stays verified"),
            top_up.checked_sub(&withdrawal).expect("bounded withdrawal")
        );
        let view = state.view();
        let credit = view
            .world()
            .provider_credit_ledger()
            .get(&PROVIDER_ID)
            .expect("credit projection remains");
        assert_eq!(credit.slashed, slash_lien.clone().into_quantity());
        assert_eq!(
            credit.bonded,
            top_up
                .checked_sub(&slash_lien)
                .and_then(|bonded| bonded.checked_sub(&withdrawal))
                .expect("unslashed withdrawal projection")
                .into_quantity()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pending_operations_survive_concurrency_and_policy_rotation() {
        let governance = account(&keypair(0x71));
        let provider = account(&keypair(0x72));
        let custody = account(&keypair(0x73));
        let treasury = account(&keypair(0x74));
        let state = state_fixture(&governance, &provider, &custody, &treasury);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, NOW * 1_000, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0x91; Hash::LENGTH]));

        let first = policy(1, None, custody.clone(), treasury.clone(), &governance);
        let first_digest = first.digest().expect("first policy digest");
        assert!(
            SetSorafsReservePolicy::new(first.clone())
                .execute(&provider, &mut stx)
                .is_err(),
            "provider cannot activate reserve governance policy"
        );
        SetSorafsReservePolicy::new(first)
            .execute(&governance, &mut stx)
            .expect("activate first policy");
        RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
            .execute(&governance, &mut stx)
            .expect("register reserve account");
        stx.world.provider_owners.remove(PROVIDER_ID);
        assert!(
            read_provider(stx.world(), PROVIDER_ID)
                .expect("read provider after registry withdrawal")
                .is_some(),
            "registered reserve state remains authoritative after provider registry withdrawal"
        );

        for (id, revision) in [(0x81, 1), (0x82, 2)] {
            RequestSorafsReserveMovement::new(
                [id; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(10_000_000),
                revision,
                first_digest,
            )
            .execute(&provider, &mut stx)
            .expect("request concurrent top-up");
        }
        let pending = read_provider(stx.world(), PROVIDER_ID)
            .expect("read provider")
            .expect("provider");
        assert_eq!((pending.revision, pending.pending_movements), (3, 2));
        assert!(
            RequestSorafsReserveMovement::new(
                [0x83; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(1_000_000),
                1,
                first_digest,
            )
            .execute(&provider, &mut stx)
            .is_err(),
            "stale provider revision must fail closed"
        );
        assert_eq!(
            read_provider(stx.world(), PROVIDER_ID)
                .expect("read provider")
                .expect("provider"),
            pending
        );
        for (id, revision) in [(0x81, 3), (0x82, 4)] {
            DecideSorafsReserveMovement::new(
                [id; 32],
                revision,
                first_digest,
                true,
                "approved".to_owned(),
            )
            .execute(&governance, &mut stx)
            .expect("decide concurrent top-up");
        }

        RequestSorafsReserveMovement::new(
            [0x84; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(10_000_000),
            5,
            first_digest,
        )
        .execute(&provider, &mut stx)
        .expect("request before policy rotation");
        let second = policy(
            2,
            Some(first_digest),
            custody.clone(),
            treasury.clone(),
            &governance,
        );
        let second_digest = second.digest().expect("second policy digest");
        SetSorafsReservePolicy::new(second)
            .execute(&governance, &mut stx)
            .expect("rotate reserve policy");
        DecideSorafsReserveMovement::new(
            [0x84; 32],
            6,
            second_digest,
            true,
            "approved after rotation".to_owned(),
        )
        .execute(&governance, &mut stx)
        .expect("pending movement remains decidable after rotation");

        for (id, revision) in [(0x91_u8, 7), (0x92, 8)] {
            SubmitSorafsReserveAppeal::new(
                [id; 32],
                PROVIDER_ID,
                revision,
                ReserveLifecycleStage::Warning,
                "review lifecycle evidence".to_owned(),
                Some([id.wrapping_add(1); 32]),
                second_digest,
            )
            .execute(&provider, &mut stx)
            .expect("submit concurrent appeal");
        }
        for (id, revision) in [(0x91, 9), (0x92, 10)] {
            DecideSorafsReserveAppeal::new(
                [id; 32],
                revision,
                second_digest,
                false,
                "not substantiated".to_owned(),
            )
            .execute(&governance, &mut stx)
            .expect("decide concurrent appeal");
        }

        let before_cap_reduction = FindSorafsReserveProviderById::new(PROVIDER_ID)
            .execute(&stx)
            .expect("query provider");
        assert_eq!(before_cap_reduction.revision, 11);
        assert_eq!(before_cap_reduction.policy_digest, second_digest);
        assert_eq!(before_cap_reduction.reserve_balance, xor_micro(30_000_000));
        assert_eq!(
            verified_provider_bond(stx.world(), PROVIDER_ID, &provider, 10)
                .expect("approved native custody top-ups are verified collateral"),
            xor_micro(30_000_000)
        );
        assert_eq!(before_cap_reduction.open_appeals, 0);
        assert_eq!(
            FindSorafsReserveAppealById::new([0x91; 32])
                .execute(&stx)
                .expect("query appeal")
                .status,
            ReserveAppealStatusV1::Rejected
        );

        DrawSorafsReserveCredit::new(PROVIDER_ID, 11, xor_micro(10_000_000), second_digest)
            .execute(&governance, &mut stx)
            .expect("draw credit before cap reduction");
        assert_eq!(
            verified_provider_bond(stx.world(), PROVIDER_ID, &provider, 10)
                .expect("treasury-funded credit is held in custody but is not provider stake"),
            xor_micro(30_000_000)
        );
        let mut unsafe_apr_change = policy(
            3,
            Some(second_digest),
            custody.clone(),
            treasury.clone(),
            &governance,
        );
        unsafe_apr_change
            .economics
            .tiers
            .iter_mut()
            .find(|tier| tier.tier == ReserveTier::TierA)
            .expect("tier A fixture")
            .interest_apr_bps += 1;
        assert!(
            SetSorafsReservePolicy::new(unsafe_apr_change)
                .execute(&governance, &mut stx)
                .is_err(),
            "APR rotation with outstanding debt must fail rather than reprice it retroactively"
        );
        assert_eq!(
            read_policy(stx.world())
                .expect("read policy")
                .expect("active policy")
                .policy_digest,
            second_digest
        );
        let mut third = policy(
            3,
            Some(second_digest),
            custody.clone(),
            treasury.clone(),
            &governance,
        );
        third.max_provider_debt = xor_micro(1_000_000);
        let third_digest = third.digest().expect("third policy digest");
        SetSorafsReservePolicy::new(third)
            .execute(&governance, &mut stx)
            .expect("reduce credit cap below grandfathered principal");
        RepaySorafsReserveCredit::new(PROVIDER_ID, 12, xor_micro(10_000_000), third_digest)
            .execute(&provider, &mut stx)
            .expect("cap reduction must not brick repayment");
        let final_account = FindSorafsReserveProviderById::new(PROVIDER_ID)
            .execute(&stx)
            .expect("query final provider");
        assert_eq!(final_account.revision, 13);
        assert_eq!(final_account.policy_digest, third_digest);
        assert_eq!(final_account.reserve_balance, xor_micro(40_000_000));
        assert_eq!(final_account.debt_principal, XorQuantity::zero());
        assert_eq!(final_account.credit_cap, xor_micro(1_000_000));
        assert_eq!(
            verified_provider_bond(stx.world(), PROVIDER_ID, &provider, 10)
                .expect("repaid principal becomes owner-funded reserve"),
            xor_micro(40_000_000)
        );

        let provider_balance = stx
            .world
            .assets
            .get(&AssetId::of(asset_definition(), provider.clone()))
            .expect("provider asset")
            .as_ref()
            .clone();
        let custody_asset_id = AssetId::of(asset_definition(), custody);
        let custody_balance = stx
            .world
            .assets
            .get(&custody_asset_id)
            .expect("custody asset")
            .as_ref()
            .clone();
        assert_eq!(provider_balance, quantity_micro(60_000_000));
        assert_eq!(custody_balance, quantity_micro(40_000_000));

        stx.world.assets.remove(custody_asset_id);
        let error = verified_provider_bond(stx.world(), PROVIDER_ID, &provider, 10)
            .expect_err("an unfunded reserve partition must not qualify as bonded stake");
        assert!(matches!(
            error,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("aggregate provider reserve partitions exceed")
        ));
    }

    #[test]
    fn committed_record_queries_are_finalized_exclusive_and_deterministic() {
        let governance = account(&keypair(0x81));
        let provider = account(&keypair(0x82));
        let custody = account(&keypair(0x83));
        let treasury = account(&keypair(0x84));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let mut first = policy(1, None, custody, treasury, &governance);
        first.max_open_appeals_per_provider = 4;
        let first_digest = first.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)
        })
        .expect("commit reserve policy");

        let provider_ids = [
            ProviderId::new([0x61; 32]),
            ProviderId::new([0x62; 32]),
            ProviderId::new([0x63; 32]),
        ];
        transact(&mut state, 2, NOW + 1, |transaction| {
            for provider_id in provider_ids {
                transaction
                    .world
                    .provider_owners
                    .insert(provider_id, provider.clone());
            }
            for provider_id in [provider_ids[2], provider_ids[0], provider_ids[1]] {
                RegisterSorafsReserveAccount::new(
                    terms_for(provider_id, provider.clone()),
                    first_digest,
                )
                .execute(&governance, transaction)?;
            }
            for (movement_id, revision) in [([0xB3; 32], 1_u64), ([0xB1; 32], 2), ([0xB2; 32], 3)] {
                RequestSorafsReserveMovement::new(
                    movement_id,
                    provider_ids[0],
                    ReserveMovementKindV1::TopUp,
                    xor_micro(1_000_000),
                    revision,
                    first_digest,
                )
                .execute(&provider, transaction)?;
            }
            for (appeal_id, revision) in [([0xC3; 32], 4_u64), ([0xC1; 32], 5), ([0xC2; 32], 6)] {
                SubmitSorafsReserveAppeal::new(
                    appeal_id,
                    provider_ids[0],
                    revision,
                    ReserveLifecycleStage::Active,
                    "review deterministic reserve evidence".to_owned(),
                    Some([appeal_id[0].wrapping_add(1); 32]),
                    first_digest,
                )
                .execute(&provider, transaction)?;
            }
            Ok(())
        })
        .expect("commit authoritative reserve records");

        let view = state.view();
        let provider_first = FindSorafsReserveProviders::new(None, None, 2)
            .execute(&view)
            .expect("query first provider page");
        assert_eq!(provider_first.finalized_cursor.height, 2);
        assert_eq!(
            provider_first
                .accounts
                .iter()
                .map(|account| account.terms.provider_id)
                .collect::<Vec<_>>(),
            provider_ids[..2]
        );
        assert!(provider_first.has_more);
        assert_eq!(provider_first.next_after, Some(provider_ids[1]));
        let provider_second = FindSorafsReserveProviders::new(
            Some(provider_first.finalized_cursor),
            provider_first.next_after,
            2,
        )
        .execute(&view)
        .expect("query second provider page");
        assert_eq!(
            provider_second
                .accounts
                .iter()
                .map(|account| account.terms.provider_id)
                .collect::<Vec<_>>(),
            vec![provider_ids[2]]
        );
        assert!(!provider_second.has_more);
        assert!(provider_second.next_after.is_none());

        let movement_first = FindSorafsReserveMovements::new(None, None, 2)
            .execute(&view)
            .expect("query first movement page");
        assert_eq!(
            movement_first
                .movements
                .iter()
                .map(|movement| movement.movement_id)
                .collect::<Vec<_>>(),
            vec![[0xB1; 32], [0xB2; 32]]
        );
        assert!(movement_first.has_more);
        assert_eq!(movement_first.next_after, Some([0xB2; 32]));
        let movement_second = FindSorafsReserveMovements::new(
            Some(movement_first.finalized_cursor),
            movement_first.next_after,
            2,
        )
        .execute(&view)
        .expect("query second movement page");
        assert_eq!(
            movement_second
                .movements
                .iter()
                .map(|movement| movement.movement_id)
                .collect::<Vec<_>>(),
            vec![[0xB3; 32]]
        );
        assert!(!movement_second.has_more);
        assert!(movement_second.next_after.is_none());

        let appeal_first = FindSorafsReserveAppeals::new(None, None, 2)
            .execute(&view)
            .expect("query first appeal page");
        assert_eq!(
            appeal_first
                .appeals
                .iter()
                .map(|appeal| appeal.appeal_id)
                .collect::<Vec<_>>(),
            vec![[0xC1; 32], [0xC2; 32]]
        );
        assert!(appeal_first.has_more);
        assert_eq!(appeal_first.next_after, Some([0xC2; 32]));
        let appeal_second = FindSorafsReserveAppeals::new(
            Some(appeal_first.finalized_cursor),
            appeal_first.next_after,
            2,
        )
        .execute(&view)
        .expect("query second appeal page");
        assert_eq!(
            appeal_second
                .appeals
                .iter()
                .map(|appeal| appeal.appeal_id)
                .collect::<Vec<_>>(),
            vec![[0xC3; 32]]
        );
        assert!(!appeal_second.has_more);
        assert!(appeal_second.next_after.is_none());

        let mut stale_anchor = provider_first.finalized_cursor;
        stale_anchor.block_hash[0] ^= 0xFF;
        assert_eq!(
            FindSorafsReserveProviders::new(Some(stale_anchor), None, 1).execute(&view),
            Err(QueryExecutionFail::Expired)
        );
        assert_eq!(
            FindSorafsReserveMovements::new(Some(stale_anchor), None, 1).execute(&view),
            Err(QueryExecutionFail::Expired)
        );
        assert_eq!(
            FindSorafsReserveAppeals::new(Some(stale_anchor), None, 1).execute(&view),
            Err(QueryExecutionFail::Expired)
        );
    }

    #[test]
    fn committed_record_queries_enforce_limits_budgets_and_corruption_checks() {
        let governance = account(&keypair(0x85));
        let provider = account(&keypair(0x86));
        let custody = account(&keypair(0x87));
        let treasury = account(&keypair(0x88));

        let build_policy_state = || {
            let mut state = state_fixture(&governance, &provider, &custody, &treasury);
            let first = policy(1, None, custody.clone(), treasury.clone(), &governance);
            transact(&mut state, 1, NOW, |transaction| {
                SetSorafsReservePolicy::new(first).execute(&governance, transaction)
            })
            .expect("commit reserve policy fixture");
            state
        };

        let state = build_policy_state();
        let view = state.view();
        for invalid_limit in [0, RESERVE_QUERY_MAX_ITEMS_V1 + 1] {
            assert!(matches!(
                FindSorafsReserveProviders::new(None, None, invalid_limit).execute(&view),
                Err(QueryExecutionFail::Conversion(_))
            ));
            assert!(matches!(
                FindSorafsReserveMovements::new(None, None, invalid_limit).execute(&view),
                Err(QueryExecutionFail::Conversion(_))
            ));
            assert!(matches!(
                FindSorafsReserveAppeals::new(None, None, invalid_limit).execute(&view),
                Err(QueryExecutionFail::Conversion(_))
            ));
        }

        let mut maximum_page = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = maximum_page.block(header.clone());
            let mut transaction = block.transaction();
            for marker in 1_u8..=129 {
                let provider_id = ProviderId::new([marker; 32]);
                let account = ReserveProviderAccountV1 {
                    terms: terms_for(provider_id, provider.clone()),
                    policy_digest: read_policy(transaction.world())
                        .expect("read active policy")
                        .expect("active policy")
                        .policy_digest,
                    revision: 1,
                    reserve_balance: XorQuantity::zero(),
                    debt_principal: XorQuantity::zero(),
                    accrued_interest: XorQuantity::zero(),
                    credit_cap: xor_micro(1_000_000_000),
                    lifecycle_stage: ReserveLifecycleStage::Warning,
                    days_past_due: 0,
                    pending_movements: 0,
                    open_appeals: 0,
                    rent_charged_through_unix: NOW + 1,
                    interest_accrued_at_unix: NOW + 1,
                    updated_at_unix: NOW + 1,
                };
                transaction.world.smart_contract_state.insert(
                    provider_key(provider_id),
                    encode_state(&account, "maximum reserve provider page")
                        .expect("encode provider account"),
                );
            }
            transaction.apply();
            block.commit().expect("commit maximum provider page");
            maximum_page.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        let maximum = FindSorafsReserveProviders::new(None, None, RESERVE_QUERY_MAX_ITEMS_V1)
            .execute(&maximum_page.view())
            .expect("maximum provider page remains within query budgets");
        assert_eq!(
            maximum.accounts.len(),
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1).expect("query maximum fits usize")
        );
        assert_eq!(
            maximum.accounts[0].terms.provider_id,
            ProviderId::new([1; 32])
        );
        assert_eq!(
            maximum
                .accounts
                .last()
                .expect("terminal provider")
                .terms
                .provider_id,
            ProviderId::new([128; 32])
        );
        assert!(maximum.has_more);
        assert_eq!(maximum.next_after, Some(ProviderId::new([128; 32])));

        let mut mismatched_key = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = mismatched_key.block(header.clone());
            let mut transaction = block.transaction();
            let mut account = ReserveProviderAccountV1 {
                terms: terms_for(ProviderId::new([0x21; 32]), provider.clone()),
                policy_digest: read_policy(transaction.world())
                    .expect("read active policy")
                    .expect("active policy")
                    .policy_digest,
                revision: 1,
                reserve_balance: XorQuantity::zero(),
                debt_principal: XorQuantity::zero(),
                accrued_interest: XorQuantity::zero(),
                credit_cap: xor_micro(1_000_000),
                lifecycle_stage: ReserveLifecycleStage::Warning,
                days_past_due: 0,
                pending_movements: 0,
                open_appeals: 0,
                rent_charged_through_unix: NOW + 1,
                interest_accrued_at_unix: NOW + 1,
                updated_at_unix: NOW + 1,
            };
            account.terms.provider_id = ProviderId::new([0x22; 32]);
            transaction.world.smart_contract_state.insert(
                provider_key(ProviderId::new([0x21; 32])),
                encode_state(&account, "mismatched reserve provider")
                    .expect("encode mismatched provider"),
            );
            transaction.apply();
            block.commit().expect("commit mismatched provider key");
            mismatched_key.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        assert!(matches!(
            FindSorafsReserveProviders::new(None, None, 1).execute(&mismatched_key.view()),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("key does not match")
        ));

        let mut oversized_record = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = oversized_record.block(header.clone());
            let mut transaction = block.transaction();
            transaction.world.smart_contract_state.insert(
                provider_key(ProviderId::new([0x31; 32])),
                vec![0xFF; RESERVE_QUERY_MAX_RECORD_BYTES_V1 + 1],
            );
            transaction.apply();
            block.commit().expect("commit oversized provider record");
            oversized_record.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        assert!(matches!(
            FindSorafsReserveProviders::new(None, None, 1).execute(&oversized_record.view()),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("record exceeds")
        ));

        validate_encoded_record_page(&vec![0_u8; RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 - 64])
            .expect("record page below the response-byte ceiling is accepted");
        assert!(matches!(
            validate_encoded_record_page(&vec![
                0_u8;
                RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1
            ]),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("record page encodes")
        ));

        let mut read_budget = ReserveEventQueryBudgetV1::default();
        read_budget
            .inspect_storage_probe(1, None, RESERVE_QUERY_MAX_EVENT_READ_BYTES_V1)
            .expect("exact reserve query read-byte ceiling is accepted");
        assert!(matches!(
            read_budget.inspect_storage_probe(1, None, 1),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("read bytes")
        ));
        let mut probe_budget = ReserveEventQueryBudgetV1::default();
        for _ in 0..RESERVE_QUERY_MAX_EVENT_STORAGE_PROBES_V1 {
            probe_budget
                .inspect_storage_probe(1, None, 0)
                .expect("exact reserve query storage-probe ceiling is accepted");
        }
        assert!(matches!(
            probe_budget.inspect_storage_probe(1, None, 0),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("storage probes")
        ));
        let mut key_budget = ReserveEventQueryBudgetV1::default();
        for _ in 0..128 {
            key_budget
                .inspect_storage_probe(RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1, None, 0)
                .expect("exact reserve query key-byte ceiling is accepted");
        }
        assert!(matches!(
            key_budget.inspect_storage_probe(1, None, 0),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("probed key bytes")
        ));
    }

    #[test]
    fn persisted_reserve_event_requires_exact_lifecycle_projection_shape() {
        let authority = account(&keypair(0x89));
        let event_record =
            |kind, provider_id, operation_id, provider_revision, resulting_lifecycle_stage| {
                ReservePersistedEventV1 {
                    sequence: 1,
                    target_block_height: 1,
                    event_index: 0,
                    event: SorafsReserveLedgerEvent {
                        kind,
                        provider_id,
                        operation_id,
                        policy_digest: [0x8A; 32],
                        provider_revision,
                        resulting_lifecycle_stage,
                        authority: authority.clone(),
                        occurred_at_unix_ms: 1,
                    },
                }
            };

        let policy_activation = event_record(
            SorafsReserveLedgerEventKind::PolicyActivated,
            None,
            None,
            0,
            None,
        );
        validate_persisted_event(&policy_activation, 1)
            .expect("policy activation without provider lifecycle is valid");
        let mut policy_with_provider_stage = policy_activation;
        policy_with_provider_stage.event.resulting_lifecycle_stage =
            Some(ReserveLifecycleStage::Active);
        assert!(matches!(
            validate_persisted_event(&policy_with_provider_stage, 1),
            Err(InstructionExecutionError::InvariantViolation(_))
        ));

        for (kind, operation_id, provider_revision) in [
            (SorafsReserveLedgerEventKind::ProviderRegistered, None, 1),
            (
                SorafsReserveLedgerEventKind::MovementRequested,
                Some([0x81; 32]),
                2,
            ),
            (
                SorafsReserveLedgerEventKind::MovementApproved,
                Some([0x82; 32]),
                2,
            ),
            (
                SorafsReserveLedgerEventKind::MovementRejected,
                Some([0x83; 32]),
                2,
            ),
            (SorafsReserveLedgerEventKind::RentCharged, None, 2),
            (SorafsReserveLedgerEventKind::LifecycleAdvanced, None, 2),
            (SorafsReserveLedgerEventKind::CreditDrawn, None, 2),
            (SorafsReserveLedgerEventKind::CreditRepaid, None, 2),
            (
                SorafsReserveLedgerEventKind::AppealSubmitted,
                Some([0x84; 32]),
                2,
            ),
            (
                SorafsReserveLedgerEventKind::AppealAccepted,
                Some([0x85; 32]),
                2,
            ),
            (
                SorafsReserveLedgerEventKind::AppealRejected,
                Some([0x86; 32]),
                2,
            ),
        ] {
            let projected = event_record(
                kind,
                Some(PROVIDER_ID),
                operation_id,
                provider_revision,
                Some(ReserveLifecycleStage::Warning),
            );
            validate_persisted_event(&projected, 1)
                .expect("provider event with resulting lifecycle is valid");
            let mut missing_projection = projected;
            missing_projection.event.resulting_lifecycle_stage = None;
            assert!(matches!(
                validate_persisted_event(&missing_projection, 1),
                Err(InstructionExecutionError::InvariantViolation(_))
            ));
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn provider_event_projection_uses_exact_authoritative_after_state() {
        let governance = account(&keypair(0x8B));
        let provider = account(&keypair(0x8C));
        let custody = account(&keypair(0x8D));
        let treasury = account(&keypair(0x8E));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let first = policy(1, None, custody, treasury, &governance);
        let first_digest = first.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
                .execute(&governance, transaction)
        })
        .expect("activate policy and register reserve provider");

        let lifecycle_at = NOW + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1 + 31 * 86_400;
        let header = block_header_at(2, lifecycle_at);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 1, 31, first_digest)
            .execute(&governance, &mut transaction)
            .expect("advance provider to default");
        SubmitSorafsReserveAppeal::new(
            [0x8F; 32],
            PROVIDER_ID,
            2,
            ReserveLifecycleStage::Active,
            "restore active lifecycle".to_owned(),
            Some([0x90; 32]),
            first_digest,
        )
        .execute(&provider, &mut transaction)
        .expect("submit lifecycle appeal");
        DecideSorafsReserveAppeal::new(
            [0x8F; 32],
            3,
            first_digest,
            true,
            "appeal accepted".to_owned(),
        )
        .execute(&governance, &mut transaction)
        .expect("accept lifecycle appeal");

        assert_eq!(
            (1..=5)
                .map(|sequence| {
                    read_persisted_event(transaction.world(), sequence)
                        .expect("read reserve event")
                        .expect("reserve event exists")
                        .event
                        .resulting_lifecycle_stage
                })
                .collect::<Vec<_>>(),
            vec![
                None,
                Some(ReserveLifecycleStage::Warning),
                Some(ReserveLifecycleStage::Default),
                Some(ReserveLifecycleStage::Default),
                Some(ReserveLifecycleStage::Active),
            ]
        );

        let account = read_provider(transaction.world(), PROVIDER_ID)
            .expect("read provider after appeal")
            .expect("provider exists");
        assert_eq!(account.lifecycle_stage, ReserveLifecycleStage::Active);
        let journal_head = read_reserve_state(transaction.world())
            .expect("read reserve state")
            .expect("reserve state exists")
            .journal_head;
        let next_sequence = journal_head
            .last_sequence
            .checked_add(1)
            .expect("fixture journal sequence has room");
        let mismatched_revision = account
            .revision
            .checked_add(1)
            .expect("fixture provider revision has room");
        let mismatched_updated_at = NOW.checked_add(1).expect("fixture timestamp has room");

        for rejected in [
            emit_reserve_event(
                &mut transaction,
                SorafsReserveLedgerEventKind::RentCharged,
                PROVIDER_ID,
                None,
                [0x91; 32],
                account.revision,
                &governance,
                NOW,
            ),
            emit_reserve_event(
                &mut transaction,
                SorafsReserveLedgerEventKind::RentCharged,
                PROVIDER_ID,
                None,
                account.policy_digest,
                mismatched_revision,
                &governance,
                NOW,
            ),
            emit_reserve_event(
                &mut transaction,
                SorafsReserveLedgerEventKind::RentCharged,
                PROVIDER_ID,
                None,
                account.policy_digest,
                account.revision,
                &governance,
                mismatched_updated_at,
            ),
            emit_reserve_event(
                &mut transaction,
                SorafsReserveLedgerEventKind::PolicyActivated,
                PROVIDER_ID,
                None,
                account.policy_digest,
                account.revision,
                &governance,
                NOW,
            ),
        ] {
            assert!(matches!(
                rejected,
                Err(InstructionExecutionError::InvariantViolation(_))
            ));
        }
        assert_eq!(
            read_reserve_state(transaction.world())
                .expect("read reserve state after rejected events")
                .expect("reserve state exists")
                .journal_head,
            journal_head
        );
        assert!(
            transaction
                .world
                .smart_contract_state
                .get(&event_key(next_sequence))
                .is_none()
        );

        transaction
            .world
            .smart_contract_state
            .remove(provider_key(PROVIDER_ID));
        assert!(matches!(
            emit_reserve_event(
                &mut transaction,
                SorafsReserveLedgerEventKind::RentCharged,
                PROVIDER_ID,
                None,
                account.policy_digest,
                account.revision,
                &governance,
                NOW,
            ),
            Err(InstructionExecutionError::InvariantViolation(_))
        ));
        assert_eq!(
            read_reserve_state(transaction.world())
                .expect("read reserve state after missing provider")
                .expect("reserve state exists")
                .journal_head,
            journal_head
        );
        assert!(
            transaction
                .world
                .smart_contract_state
                .get(&event_key(next_sequence))
                .is_none()
        );
    }

    #[test]
    fn committed_event_query_is_finalized_cursor_bounded_and_deterministic() {
        let governance = account(&keypair(0x75));
        let provider = account(&keypair(0x76));
        let custody = account(&keypair(0x77));
        let treasury = account(&keypair(0x78));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let first = policy(1, None, custody, treasury, &governance);
        let first_digest = first.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)
        })
        .expect("commit reserve policy");
        transact(&mut state, 2, NOW + 1, |transaction| {
            RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
                .execute(&governance, transaction)?;
            RequestSorafsReserveMovement::new(
                [0x81; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(10_000_000),
                1,
                first_digest,
            )
            .execute(&provider, transaction)
        })
        .expect("commit provider registration and movement request");
        transact(&mut state, 3, NOW + 2, |transaction| {
            DecideSorafsReserveMovement::new(
                [0x81; 32],
                2,
                first_digest,
                false,
                "declined".to_owned(),
            )
            .execute(&governance, transaction)
        })
        .expect("commit reserve movement decision");

        let view = state.view();
        let first_page = FindSorafsReserveEvents::new(None, None, 2)
            .execute(&view)
            .expect("query first committed reserve event page");
        assert_eq!(first_page.finalized_cursor.height, 3);
        assert_eq!(first_page.events.len(), 2);
        assert!(first_page.has_more);
        assert_eq!(
            first_page
                .events
                .iter()
                .map(|event| (event.sequence, event.block_height, event.event_index))
                .collect::<Vec<_>>(),
            vec![(1, 1, 0), (2, 2, 0)]
        );
        let anchor = first_page.finalized_cursor;
        let cursor = first_page.next_after.expect("event continuation");
        let second_page = FindSorafsReserveEvents::new(Some(anchor), Some(cursor), 2)
            .execute(&view)
            .expect("query second committed reserve event page");
        assert_eq!(
            second_page
                .events
                .iter()
                .map(|event| (event.sequence, event.block_height, event.event_index))
                .collect::<Vec<_>>(),
            vec![(3, 2, 1), (4, 3, 0)]
        );
        assert!(!second_page.has_more);
        assert!(second_page.next_after.is_none());
        assert_eq!(
            first_page.events[0].event.kind,
            SorafsReserveLedgerEventKind::PolicyActivated
        );
        assert_eq!(
            second_page.events[1].event.kind,
            SorafsReserveLedgerEventKind::MovementRejected
        );
        assert_eq!(
            first_page
                .events
                .iter()
                .chain(&second_page.events)
                .map(|event| event.event.resulting_lifecycle_stage)
                .collect::<Vec<_>>(),
            vec![
                None,
                Some(ReserveLifecycleStage::Warning),
                Some(ReserveLifecycleStage::Warning),
                Some(ReserveLifecycleStage::Warning),
            ]
        );

        let expected_hashes = [
            *iroha_crypto::HashOf::new(&block_header_at(1, NOW)).as_ref(),
            *iroha_crypto::HashOf::new(&block_header_at(2, NOW + 1)).as_ref(),
            *iroha_crypto::HashOf::new(&block_header_at(3, NOW + 2)).as_ref(),
        ];
        assert_eq!(first_page.events[0].block_hash, expected_hashes[0]);
        assert_eq!(first_page.events[1].block_hash, expected_hashes[1]);
        assert_eq!(second_page.events[0].block_hash, expected_hashes[1]);
        assert_eq!(second_page.events[1].block_hash, expected_hashes[2]);
        assert_eq!(anchor.block_hash, expected_hashes[2]);

        let mut stale_anchor = anchor;
        stale_anchor.block_hash[0] ^= 0xFF;
        assert_eq!(
            FindSorafsReserveEvents::new(Some(stale_anchor), None, 1).execute(&view),
            Err(QueryExecutionFail::Expired)
        );
        let mut tampered_cursor = cursor;
        tampered_cursor.event_index += 1;
        assert_eq!(
            FindSorafsReserveEvents::new(Some(anchor), Some(tampered_cursor), 1).execute(&view),
            Err(QueryExecutionFail::Expired)
        );
        for invalid_limit in [0, RESERVE_QUERY_MAX_ITEMS_V1 + 1] {
            assert!(matches!(
                FindSorafsReserveEvents::new(Some(anchor), None, invalid_limit).execute(&view),
                Err(QueryExecutionFail::Conversion(_))
            ));
        }
    }

    #[test]
    fn committed_event_query_accepts_maximum_page_with_full_metering() {
        let governance = account(&keypair(0x7D));
        let provider = account(&keypair(0x7E));
        let custody = account(&keypair(0x7F));
        let treasury = account(&keypair(0x80));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let first = policy(1, None, custody, treasury, &governance);
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)
        })
        .expect("commit reserve policy");

        let header = block_header_at(2, NOW + 1);
        let mut block = state.block(header.clone());
        let mut transaction = block.transaction();
        let template = read_persisted_event(transaction.world(), 1)
            .expect("read initial reserve event")
            .expect("initial reserve event exists");
        let terminal_sequence = u64::from(RESERVE_QUERY_MAX_ITEMS_V1) + 2;
        for sequence in 2..=terminal_sequence {
            let mut record = template.clone();
            record.sequence = sequence;
            record.event_index = u32::try_from(sequence - 1).expect("event index fits into u32");
            transaction.world.smart_contract_state.insert(
                event_key(sequence),
                encode_state(&record, "maximum-page reserve event")
                    .expect("encode maximum-page reserve event"),
            );
        }
        let head = ReserveEventJournalHeadV1 {
            last_sequence: terminal_sequence,
            last_target_block_height: 1,
            last_event_index: u32::try_from(terminal_sequence - 1)
                .expect("terminal event index fits into u32"),
        };
        let mut reserve_state = read_reserve_state(transaction.world())
            .expect("read maximum-page reserve state")
            .expect("reserve state exists");
        reserve_state.journal_head = head;
        transaction.world.smart_contract_state.insert(
            reserve_state_key().clone(),
            encode_state(&reserve_state, "maximum-page reserve state")
                .expect("encode maximum-page reserve state"),
        );
        transaction.apply();
        block.commit().expect("commit maximum-page fixture");
        state.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));

        let view = state.view();
        let prefix = FindSorafsReserveEvents::new(None, None, 2)
            .execute(&view)
            .expect("query reserve event prefix");
        let after = prefix.events[1].cursor();
        let page = FindSorafsReserveEvents::new(
            Some(prefix.finalized_cursor),
            Some(after),
            RESERVE_QUERY_MAX_ITEMS_V1,
        )
        .execute(&view)
        .expect("maximum reserve event page remains within every budget");
        assert_eq!(
            page.events.len(),
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1).expect("query maximum fits into usize")
        );
        assert_eq!(page.events[0].sequence, 3);
        assert_eq!(page.events.last().expect("terminal event").sequence, 130);
        assert!(!page.has_more);
        assert!(page.next_after.is_none());
    }

    #[test]
    fn committed_event_queries_fail_closed_on_corruption_and_resource_exhaustion() {
        let governance = account(&keypair(0x79));
        let provider = account(&keypair(0x7A));
        let custody = account(&keypair(0x7B));
        let treasury = account(&keypair(0x7C));

        let build_policy_state = || {
            let mut state = state_fixture(&governance, &provider, &custody, &treasury);
            let first = policy(1, None, custody.clone(), treasury.clone(), &governance);
            transact(&mut state, 1, NOW, |transaction| {
                SetSorafsReservePolicy::new(first).execute(&governance, transaction)
            })
            .expect("commit reserve policy fixture");
            state
        };

        let mut missing_head = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = missing_head.block(header.clone());
            let mut transaction = block.transaction();
            let mut reserve_state = read_reserve_state(transaction.world())
                .expect("read reserve state")
                .expect("reserve state exists");
            reserve_state.journal_head.last_sequence = 0;
            transaction.world.smart_contract_state.insert(
                reserve_state_key().clone(),
                encode_state(&reserve_state, "corrupt reserve state")
                    .expect("encode corrupt reserve state"),
            );
            transaction.apply();
            block.commit().expect("commit missing-head corruption");
            missing_head.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        assert!(matches!(
            FindSorafsReserveEvents::new(None, None, 10).execute(&missing_head.view()),
            Err(QueryExecutionFail::Conversion(_))
        ));

        let mut oversized = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = oversized.block(header.clone());
            let mut transaction = block.transaction();
            transaction.world.smart_contract_state.insert(
                event_key(1),
                vec![0xFF; RESERVE_COMMITTED_EVENT_MAX_BYTES_V1 + 1],
            );
            transaction.apply();
            block.commit().expect("commit oversized-event corruption");
            oversized.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        assert!(matches!(
            FindSorafsReserveEvents::new(None, None, 10).execute(&oversized.view()),
            Err(QueryExecutionFail::Conversion(_))
        ));

        let mut orphan = build_policy_state();
        {
            let header = block_header_at(2, NOW + 1);
            let mut block = orphan.block(header.clone());
            let mut transaction = block.transaction();
            let mut record = read_persisted_event(transaction.world(), 1)
                .expect("read policy event")
                .expect("policy event exists");
            record.sequence = 2;
            record.target_block_height = 2;
            record.event_index = 0;
            transaction.world.smart_contract_state.insert(
                event_key(2),
                encode_state(&record, "orphan event").expect("encode orphan reserve event"),
            );
            transaction.apply();
            block.commit().expect("commit orphan-event corruption");
            orphan.push_block_hash_for_testing(iroha_crypto::HashOf::new(&header));
        }
        assert!(matches!(
            FindSorafsReserveEvents::new(None, None, 10).execute(&orphan.view()),
            Err(QueryExecutionFail::Conversion(_))
        ));

        let mut read_budget = ReserveEventQueryBudgetV1::default();
        read_budget
            .inspect_storage_probe(1, None, RESERVE_QUERY_MAX_EVENT_READ_BYTES_V1)
            .expect("exact reserve query read-byte ceiling is accepted");
        assert!(matches!(
            read_budget.inspect_storage_probe(1, None, 1),
            Err(QueryExecutionFail::Conversion(_))
        ));

        let mut probe_budget = ReserveEventQueryBudgetV1::default();
        for _ in 0..RESERVE_QUERY_MAX_EVENT_STORAGE_PROBES_V1 {
            probe_budget
                .inspect_storage_probe(1, None, 0)
                .expect("exact reserve query storage-probe ceiling is accepted");
        }
        assert!(matches!(
            probe_budget.inspect_storage_probe(1, None, 0),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("storage probes")
        ));

        let mut key_budget = ReserveEventQueryBudgetV1::default();
        for _ in 0..128 {
            key_budget
                .inspect_storage_probe(RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1, None, 0)
                .expect("exact reserve query total key-byte ceiling is accepted");
        }
        assert!(matches!(
            key_budget.inspect_storage_probe(1, None, 0),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("probed key bytes")
        ));
        assert!(matches!(
            ReserveEventQueryBudgetV1::default().inspect_storage_probe(
                RESERVE_QUERY_MAX_EVENT_PROBE_KEY_BYTES_V1 + 1,
                None,
                0,
            ),
            Err(QueryExecutionFail::Conversion(message)) if message.contains("probe key")
        ));
    }

    #[test]
    fn initial_policy_activation_rejects_a_nonempty_reserve_namespace_atomically() {
        let governance = account(&keypair(0x91));
        let provider = account(&keypair(0x92));
        let custody = account(&keypair(0x93));
        let treasury = account(&keypair(0x94));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let legacy_key =
            StatePath::from_str("sorafs_reserve_policy_v1").expect("legacy fixture key is valid");
        state
            .world
            .smart_contract_state
            .insert(legacy_key.clone(), vec![0xA5]);

        let result = transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(policy(1, None, custody, treasury, &governance))
                .execute(&governance, transaction)
        });

        assert!(matches!(
            result,
            Err(InstructionExecutionError::InvariantViolation(_))
        ));
        let view = state.view();
        assert_eq!(
            view.world().smart_contract_state().get(&legacy_key),
            Some(&vec![0xA5])
        );
        assert!(
            view.world()
                .smart_contract_state()
                .get(reserve_state_key())
                .is_none()
        );
        assert!(
            view.world()
                .smart_contract_state()
                .get(&event_key(1))
                .is_none()
        );
    }

    #[test]
    fn rent_charge_advances_only_due_periods_in_bounded_catchup_batches() {
        let governance = account(&keypair(0xC1));
        let provider = account(&keypair(0xC2));
        let custody = account(&keypair(0xC3));
        let treasury = account(&keypair(0xC4));
        let mut state = state_fixture_with_provider_balance(
            &governance,
            &provider,
            &custody,
            &treasury,
            quantity_micro(2_000_000_000),
        );
        let configured = policy(1, None, custody.clone(), treasury.clone(), &governance);
        let policy_digest = configured.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("activate policy and register provider");

        let catchup_at = NOW + 13 * RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;
        transact(&mut state, 2, catchup_at, |transaction| {
            ChargeSorafsReserveRent::new(
                PROVIDER_ID,
                1,
                RESERVE_RENT_MAX_BILLING_PERIODS_V1,
                policy_digest,
            )
            .execute(&governance, transaction)
        })
        .expect("settle the native maximum catchup batch");
        let after_first = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read provider after first catchup batch")
            .expect("provider remains");
        assert_eq!(after_first.revision, 2);
        assert_eq!(
            after_first.rent_charged_through_unix,
            NOW + 12 * RESERVE_RENT_BILLING_PERIOD_SECONDS_V1
        );
        assert_eq!(
            after_first
                .rent_periods_due_at(catchup_at)
                .expect("one period remains due"),
            1
        );

        assert!(
            transact(&mut state, 3, catchup_at, |transaction| {
                ChargeSorafsReserveRent::new(PROVIDER_ID, 2, 2, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "a charge cannot move the rent anchor beyond the finalized block time"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after rejected overcharge")
                .expect("provider remains"),
            after_first
        );

        transact(&mut state, 3, catchup_at, |transaction| {
            ChargeSorafsReserveRent::new(PROVIDER_ID, 2, 1, policy_digest)
                .execute(&governance, transaction)
        })
        .expect("settle the final due period");
        let caught_up = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read fully caught-up provider")
            .expect("provider remains");
        assert_eq!(caught_up.revision, 3);
        assert_eq!(caught_up.rent_charged_through_unix, catchup_at);
        assert_eq!(
            caught_up
                .rent_periods_due_at(catchup_at)
                .expect("provider is current"),
            0
        );
    }

    #[test]
    fn exact_balance_charge_succeeds_and_stale_revision_cannot_double_settle() {
        let governance = account(&keypair(0xC5));
        let provider = account(&keypair(0xC6));
        let custody = account(&keypair(0xC7));
        let treasury = account(&keypair(0xC8));
        let configured = policy(1, None, custody, treasury, &governance);
        let rent = configured
            .economics
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("rent quote")
            .effective_rent;
        let mut state = state_fixture_with_provider_balance(
            &governance,
            &provider,
            &configured.custody_account,
            &configured.treasury_account,
            rent.clone().into_quantity(),
        );
        let policy_digest = configured.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider.clone()), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("activate policy and register exact-balance provider");

        let due_at = NOW + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;
        transact(&mut state, 2, due_at, |transaction| {
            ChargeSorafsReserveRent::new(PROVIDER_ID, 1, 1, policy_digest)
                .execute(&governance, transaction)
        })
        .expect("an exact spendable balance settles rent");
        let settled = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read settled provider")
            .expect("provider remains");
        assert_eq!(settled.revision, 2);
        assert_eq!(settled.rent_charged_through_unix, due_at);
        assert!(reserve_asset_balance(&state, &provider).is_zero());

        assert!(
            transact(&mut state, 3, due_at, |transaction| {
                ChargeSorafsReserveRent::new(PROVIDER_ID, 1, 1, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "the stale compare-and-set revision cannot settle rent twice"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after stale replay")
                .expect("provider remains"),
            settled
        );
    }

    #[test]
    fn lifecycle_uses_exact_anchor_age_and_rejects_noop_or_timestamp_regression() {
        let governance = account(&keypair(0xD1));
        let provider = account(&keypair(0xD2));
        let custody = account(&keypair(0xD3));
        let treasury = account(&keypair(0xD4));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let configured = policy(1, None, custody, treasury, &governance);
        let policy_digest = configured.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("activate policy and register provider");
        let baseline = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read provider baseline")
            .expect("provider exists");
        let exact_boundary = NOW + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;

        assert!(
            transact(&mut state, 2, exact_boundary, |transaction| {
                AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 1, 1, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "the exact due boundary is day zero, never day one"
        );
        assert!(
            transact(&mut state, 2, exact_boundary, |transaction| {
                AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 1, 0, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "an exact-day lifecycle no-op cannot churn the provider revision"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after rejected boundary transitions")
                .expect("provider remains"),
            baseline
        );

        let one_day_overdue = exact_boundary + 86_400;
        transact(&mut state, 2, one_day_overdue, |transaction| {
            AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 1, 1, policy_digest)
                .execute(&governance, transaction)
        })
        .expect("the exact derived lifecycle transition succeeds");
        let overdue = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read overdue provider")
            .expect("provider remains");
        assert_eq!(overdue.days_past_due, 1);
        assert_eq!(overdue.lifecycle_stage, ReserveLifecycleStage::Grace);

        assert!(
            transact(&mut state, 3, exact_boundary, |transaction| {
                AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 2, 0, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "a later height cannot regress the provider timestamp or overdue age"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after timestamp regression")
                .expect("provider remains"),
            overdue
        );
    }

    #[test]
    fn lifecycle_requires_anchor_advancement_for_zero_or_funded_rent_periods() {
        let governance = account(&keypair(0xD5));
        let provider = account(&keypair(0xD6));
        let custody = account(&keypair(0xD7));
        let treasury = account(&keypair(0xD8));
        let mut state = state_fixture_with_provider_balance(
            &governance,
            &provider,
            &custody,
            &treasury,
            quantity_micro(1_000_000_000),
        );
        let first = policy(1, None, custody.clone(), treasury.clone(), &governance);
        let first_digest = first.digest().expect("first reserve policy digest");
        let reserve_requirement = first
            .economics
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                XorQuantity::zero(),
            )
            .expect("first reserve quote")
            .reserve_requirement;
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
                .execute(&governance, transaction)?;
            RequestSorafsReserveMovement::new(
                [0xD9; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                reserve_requirement,
                1,
                first_digest,
            )
            .execute(&provider, transaction)?;
            DecideSorafsReserveMovement::new(
                [0xD9; 32],
                2,
                first_digest,
                true,
                "fund exact underwriting requirement".to_owned(),
            )
            .execute(&governance, transaction)?;
            AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 3, 0, first_digest)
                .execute(&governance, transaction)
        })
        .expect("establish an active zero-rent provider");
        let active = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read active provider")
            .expect("provider remains");
        assert_eq!(active.revision, 4);
        assert_eq!(active.lifecycle_stage, ReserveLifecycleStage::Active);

        let first_due = NOW + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;
        assert!(
            transact(&mut state, 2, first_due, |transaction| {
                AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 4, 0, first_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "a zero-rent period must advance through ChargeRent, not lifecycle aging"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after rejected zero-rent aging")
                .expect("provider remains"),
            active
        );

        let mut second = policy(2, Some(first_digest), custody, treasury, &governance);
        second
            .economics
            .rent_rates
            .retain(|rate| rate.storage_class != StorageClass::Hot);
        second.economics.rent_rates.push(ClassRentRate::new(
            StorageClass::Hot,
            "24".parse().expect("rotated hot rent"),
        ));
        let second_digest = second.digest().expect("second reserve policy digest");
        transact(&mut state, 2, first_due, |transaction| {
            ChargeSorafsReserveRent::new(PROVIDER_ID, 4, 1, first_digest)
                .execute(&governance, transaction)?;
            SetSorafsReservePolicy::new(second).execute(&governance, transaction)
        })
        .expect("advance the zero-rent anchor and rotate pricing");
        let after_zero_rent = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read provider after zero-rent charge")
            .expect("provider remains");
        assert_eq!(after_zero_rent.revision, 5);
        assert_eq!(after_zero_rent.rent_charged_through_unix, first_due);

        let second_due = first_due + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;
        assert!(
            transact(&mut state, 3, second_due, |transaction| {
                AdvanceSorafsReserveLifecycle::new(PROVIDER_ID, 5, 0, second_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "one exactly affordable positive-rent period must be charged, never aged"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after rejected funded aging")
                .expect("provider remains"),
            after_zero_rent
        );
        transact(&mut state, 3, second_due, |transaction| {
            ChargeSorafsReserveRent::new(PROVIDER_ID, 5, 1, second_digest)
                .execute(&governance, transaction)
        })
        .expect("the funded period advances through ChargeRent");
        let settled = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read settled rotated provider")
            .expect("provider remains");
        assert_eq!(settled.revision, 6);
        assert_eq!(settled.rent_charged_through_unix, second_due);
        assert_eq!(settled.policy_digest, second_digest);
    }

    #[test]
    fn failed_transfer_and_finalized_timestamp_rollback_preserve_rent_anchor() {
        let governance = account(&keypair(0xC9));
        let provider = account(&keypair(0xCA));
        let custody = account(&keypair(0xCB));
        let treasury = account(&keypair(0xCC));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let configured = policy(1, None, custody.clone(), treasury.clone(), &governance);
        let policy_digest = configured.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW - 100, |transaction| {
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)
        })
        .expect("activate reserve policy");
        transact(&mut state, 2, NOW, |transaction| {
            RegisterSorafsReserveAccount::new(terms(provider.clone()), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("register provider at its rent anchor");
        let baseline = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read provider baseline")
            .expect("provider exists");
        let provider_balance = reserve_asset_balance(&state, &provider);
        let treasury_balance = reserve_asset_balance(&state, &treasury);

        let due_at = NOW + RESERVE_RENT_BILLING_PERIOD_SECONDS_V1;
        assert!(
            transact(&mut state, 3, due_at, |transaction| {
                ChargeSorafsReserveRent::new(PROVIDER_ID, 1, 1, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "the fixture provider cannot cover one whole rent period"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after failed transfer")
                .expect("provider remains"),
            baseline,
            "a failed custody transfer cannot advance revision, lifecycle, or rent anchor"
        );
        assert_eq!(reserve_asset_balance(&state, &provider), provider_balance);
        assert_eq!(reserve_asset_balance(&state, &treasury), treasury_balance);

        assert!(
            transact(&mut state, 3, NOW - 1, |transaction| {
                ChargeSorafsReserveRent::new(PROVIDER_ID, 1, 1, policy_digest)
                    .execute(&governance, transaction)
            })
            .is_err(),
            "a later height cannot supply a timestamp before the ledger rent anchor"
        );
        assert_eq!(
            read_provider(state.view().world(), PROVIDER_ID)
                .expect("read provider after timestamp rollback")
                .expect("provider remains"),
            baseline
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_provider_mutation_rejects_regressed_block_time() {
        let governance = account(&keypair(0xE1));
        let provider = account(&keypair(0xE2));
        let custody = account(&keypair(0xE3));
        let treasury = account(&keypair(0xE4));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let configured = policy(1, None, custody, treasury, &governance);
        let policy_digest = configured.digest().expect("reserve policy digest");
        transact(&mut state, 1, NOW, |transaction| {
            SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
            RegisterSorafsReserveAccount::new(terms(provider.clone()), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("activate policy and register provider");

        let updated_at = NOW + 100;
        transact(&mut state, 2, updated_at, |transaction| {
            RequestSorafsReserveMovement::new(
                [0xE5; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(1_000_000),
                1,
                policy_digest,
            )
            .execute(&provider, transaction)?;
            RequestSorafsReserveMovement::new(
                [0xE6; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::Withdrawal,
                xor_micro(1),
                2,
                policy_digest,
            )
            .execute(&provider, transaction)?;
            SubmitSorafsReserveAppeal::new(
                [0xE7; 32],
                PROVIDER_ID,
                3,
                ReserveLifecycleStage::Active,
                "review provider lifecycle".to_owned(),
                Some([0xE8; 32]),
                policy_digest,
            )
            .execute(&provider, transaction)?;
            DrawSorafsReserveCredit::new(PROVIDER_ID, 4, xor_micro(1_000_000), policy_digest)
                .execute(&governance, transaction)
        })
        .expect("establish pending records and a later provider timestamp");

        let baseline_provider = read_provider(state.view().world(), PROVIDER_ID)
            .expect("read provider baseline")
            .expect("provider exists");
        assert_eq!(baseline_provider.updated_at_unix, updated_at);
        assert_eq!(baseline_provider.revision, 5);
        let baseline_top_up = read_movement(state.view().world(), [0xE5; 32])
            .expect("read pending top-up")
            .expect("top-up exists");
        let baseline_withdrawal = read_movement(state.view().world(), [0xE6; 32])
            .expect("read pending withdrawal")
            .expect("withdrawal exists");
        let baseline_appeal = read_appeal(state.view().world(), [0xE7; 32])
            .expect("read pending appeal")
            .expect("appeal exists");
        let baseline_reserve_state = read_reserve_state(state.view().world())
            .expect("read reserve state")
            .expect("reserve state exists");

        let regressed_at = updated_at - 1;
        let header = block_header_at(3, regressed_at);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let assert_regression = |result: Result<(), InstructionExecutionError>, operation: &str| {
            let error = result.expect_err(operation);
            assert!(
                error.to_string().contains("predates provider update"),
                "{operation} failed for the wrong reason: {error}"
            );
        };

        assert_regression(
            RequestSorafsReserveMovement::new(
                [0xE9; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(1),
                5,
                policy_digest,
            )
            .execute(&provider, &mut transaction),
            "regressed top-up request",
        );
        assert_regression(
            RequestSorafsReserveMovement::new(
                [0xEA; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::Withdrawal,
                xor_micro(1),
                5,
                policy_digest,
            )
            .execute(&provider, &mut transaction),
            "regressed withdrawal request",
        );
        assert_regression(
            DecideSorafsReserveMovement::new(
                [0xE5; 32],
                5,
                policy_digest,
                true,
                "approve top-up".to_owned(),
            )
            .execute(&governance, &mut transaction),
            "regressed top-up decision",
        );
        assert_regression(
            DecideSorafsReserveMovement::new(
                [0xE6; 32],
                5,
                policy_digest,
                false,
                "reject withdrawal".to_owned(),
            )
            .execute(&governance, &mut transaction),
            "regressed withdrawal decision",
        );
        assert_regression(
            DrawSorafsReserveCredit::new(PROVIDER_ID, 5, xor_micro(1), policy_digest)
                .execute(&governance, &mut transaction),
            "regressed credit draw",
        );
        assert_regression(
            RepaySorafsReserveCredit::new(PROVIDER_ID, 5, xor_micro(1), policy_digest)
                .execute(&provider, &mut transaction),
            "regressed credit repayment",
        );
        assert_regression(
            SubmitSorafsReserveAppeal::new(
                [0xEB; 32],
                PROVIDER_ID,
                5,
                ReserveLifecycleStage::Warning,
                "review timestamp regression".to_owned(),
                Some([0xEC; 32]),
                policy_digest,
            )
            .execute(&provider, &mut transaction),
            "regressed appeal submission",
        );
        assert_regression(
            DecideSorafsReserveAppeal::new(
                [0xE7; 32],
                5,
                policy_digest,
                true,
                "accept appeal".to_owned(),
            )
            .execute(&governance, &mut transaction),
            "regressed appeal decision",
        );

        assert_eq!(
            read_provider(transaction.world(), PROVIDER_ID)
                .expect("read provider after rejected mutations")
                .expect("provider remains"),
            baseline_provider
        );
        assert_eq!(
            read_movement(transaction.world(), [0xE5; 32])
                .expect("read top-up after rejected mutations")
                .expect("top-up remains"),
            baseline_top_up
        );
        assert_eq!(
            read_movement(transaction.world(), [0xE6; 32])
                .expect("read withdrawal after rejected mutations")
                .expect("withdrawal remains"),
            baseline_withdrawal
        );
        assert_eq!(
            read_appeal(transaction.world(), [0xE7; 32])
                .expect("read appeal after rejected mutations")
                .expect("appeal remains"),
            baseline_appeal
        );
        assert!(
            read_movement(transaction.world(), [0xE9; 32])
                .expect("read rejected top-up request")
                .is_none()
        );
        assert!(
            read_movement(transaction.world(), [0xEA; 32])
                .expect("read rejected withdrawal request")
                .is_none()
        );
        assert!(
            read_appeal(transaction.world(), [0xEB; 32])
                .expect("read rejected appeal")
                .is_none()
        );
        assert_eq!(
            read_reserve_state(transaction.world())
                .expect("read reserve state after rejected mutations")
                .expect("reserve state remains"),
            baseline_reserve_state
        );
    }

    #[test]
    fn policy_rotation_rejects_regressed_activation_time() {
        let governance = account(&keypair(0xED));
        let provider = account(&keypair(0xEE));
        let custody = account(&keypair(0xEF));
        let treasury = account(&keypair(0xF0));
        let mut state = state_fixture(&governance, &provider, &custody, &treasury);
        let activated_at = NOW + 100;
        let first = policy(1, None, custody.clone(), treasury.clone(), &governance);
        let first_digest = first.digest().expect("first reserve policy digest");
        transact(&mut state, 1, activated_at, |transaction| {
            SetSorafsReservePolicy::new(first).execute(&governance, transaction)
        })
        .expect("activate first reserve policy");
        let baseline = read_reserve_state(state.view().world())
            .expect("read baseline reserve state")
            .expect("reserve state exists");
        let second = policy(2, Some(first_digest), custody, treasury, &governance);

        let header = block_header_at(2, activated_at - 1);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let error = SetSorafsReservePolicy::new(second)
            .execute(&governance, &mut transaction)
            .expect_err("regressed reserve policy activation must fail");
        assert!(
            error
                .to_string()
                .contains("predates active policy activation"),
            "policy rotation failed for the wrong reason: {error}"
        );
        assert_eq!(
            read_reserve_state(transaction.world())
                .expect("read reserve state after rejected policy")
                .expect("reserve state remains"),
            baseline
        );
        assert!(
            read_persisted_event(transaction.world(), 2)
                .expect("read absent policy event")
                .is_none(),
            "rejected policy rotation cannot append an event"
        );
    }

    #[test]
    fn exact_service_authorities_and_decision_cas_fail_without_mutation() {
        let governance = account(&keypair(0xA1));
        let provider = account(&keypair(0xA2));
        let decision = account(&keypair(0xA3));
        let operations = account(&keypair(0xA4));
        let state = state_fixture(&governance, &provider, &decision, &operations);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, NOW * 1_000, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction.tx_call_hash = Some(Hash::prehashed([0xA5; Hash::LENGTH]));

        let mut first = policy(1, None, decision.clone(), operations.clone(), &governance);
        first.operations_authority = operations.clone();
        first.decision_authority = decision.clone();
        let first_digest = first.digest().expect("first policy digest");
        SetSorafsReservePolicy::new(first)
            .execute(&governance, &mut transaction)
            .expect("activate first policy");

        let registration = RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest);
        assert!(
            registration
                .clone()
                .execute(&governance, &mut transaction)
                .is_err(),
            "broad reserve-governance permission must not substitute for the operations account"
        );
        assert!(
            read_provider(transaction.world(), PROVIDER_ID)
                .expect("read provider after rejected registration")
                .is_none()
        );
        registration
            .execute(&operations, &mut transaction)
            .expect("exact operations account registers provider");

        RequestSorafsReserveMovement::new(
            [0xA6; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(10_000_000),
            1,
            first_digest,
        )
        .execute(&provider, &mut transaction)
        .expect("provider requests top-up");
        let provider_before = read_provider(transaction.world(), PROVIDER_ID)
            .expect("read provider before rotation")
            .expect("registered provider");
        let movement_before = read_movement(transaction.world(), [0xA6; 32])
            .expect("read movement before rotation")
            .expect("pending movement");

        let mut second = policy(
            2,
            Some(first_digest),
            decision.clone(),
            operations.clone(),
            &governance,
        );
        second.operations_authority = operations.clone();
        second.decision_authority = decision.clone();
        let second_digest = second.digest().expect("second policy digest");
        SetSorafsReservePolicy::new(second)
            .execute(&governance, &mut transaction)
            .expect("rotate reserve policy");

        for (authority, revision, digest) in [
            (&governance, 2, second_digest),
            (&decision, 1, second_digest),
            (&decision, 2, first_digest),
        ] {
            assert!(
                DecideSorafsReserveMovement::new(
                    [0xA6; 32],
                    revision,
                    digest,
                    true,
                    "approve top-up".to_owned(),
                )
                .execute(authority, &mut transaction)
                .is_err()
            );
            assert_eq!(
                read_provider(transaction.world(), PROVIDER_ID)
                    .expect("read provider after rejected decision")
                    .expect("provider remains"),
                provider_before
            );
            assert_eq!(
                read_movement(transaction.world(), [0xA6; 32])
                    .expect("read movement after rejected decision")
                    .expect("movement remains"),
                movement_before
            );
        }

        DecideSorafsReserveMovement::new(
            [0xA6; 32],
            2,
            second_digest,
            true,
            "approve top-up".to_owned(),
        )
        .execute(&decision, &mut transaction)
        .expect("exact decision account and CAS apply top-up");
        let after_decision = read_provider(transaction.world(), PROVIDER_ID)
            .expect("read provider after decision")
            .expect("provider remains");
        assert_eq!(after_decision.revision, 3);

        assert!(
            ChargeSorafsReserveRent::new(PROVIDER_ID, 3, 1, second_digest)
                .execute(&governance, &mut transaction)
                .is_err(),
            "broad governance permission must not substitute for operations"
        );
        assert_eq!(
            read_provider(transaction.world(), PROVIDER_ID)
                .expect("read provider after rejected charge")
                .expect("provider remains"),
            after_decision
        );
    }
}
