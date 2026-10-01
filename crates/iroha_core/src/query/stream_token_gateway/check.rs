//! Deterministic no-write gateway Check predicates and exact current subject reads.
//!
//! This owner checks consensus-visible committed frames, never node-local quorum certificates.
//! Successful predicates and returned values are claims until the separate proof owner verifies
//! the exact directly signed Check, original native history, fresh challenge and finality.

use iroha_data_model::{
    account::AccountId,
    permission::Permission,
    sorafs::{
        reputation::StreamTokenValidationStatusV1 as Status,
        stream_token_gateway::{
            STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1,
            StreamTokenGatewayAdmissionDeliveryStateV1 as Delivery,
            StreamTokenGatewayAdmissionReadbackV1 as Readback,
            StreamTokenGatewayAdmissionRecordV1 as Record,
            StreamTokenGatewayAdmissionResultV1 as AdmissionResult,
            native::{
                STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1,
                StreamTokenGatewayActionV1 as Action, StreamTokenGatewayCheckSubjectV1 as Subject,
                StreamTokenGatewayCheckV1 as Check, StreamTokenGatewayExecutionV1 as Execution,
                StreamTokenGatewayFinalityFloorV1 as Floor, StreamTokenGatewayRequestV1 as Request,
                stream_token_gateway_pending_readback_digest_v1,
            },
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamTokenGateway, CanOperateSorafsStreamTokenGateway,
};
use mv::storage::StorageReadOnly;

use super::{
    read::{self, GatewayReadCut},
    rows::*,
    storage::{self, GatewayCurrentV1, WorldGatewayRows},
    transition::request_digest,
};
use crate::{
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
    sumeragi::certified_chain::committed_block,
};
use TransitionError as Error;

/// Exact deterministic subject value, carrying no verified proof or serving capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GatewayCheckedValueV1 {
    /// Current qualification; enabled admission is a separate predicate.
    Qualification,
    /// Immutable original admission with its exact current delivery state; never serving evidence.
    Admission(AdmissionResult),
    /// Exact acknowledged Accepted original whose original lease remains live.
    Serving(AdmissionResult),
    /// Complete requested oldest pending prefix, including authoritative empty readback.
    Pending(Readback),
    /// Exact original ordered acknowledgement remains committed.
    Acknowledged(Record),
    /// Exact original grant has a committed terminal fact.
    Released(Record),
}

/// Same-World inputs and subject claim; no cryptographic finality is implied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GatewayCheckedReadV1 {
    /// Independently selected current policy and operational head.
    pub current: GatewayCurrentV1,
    /// Exact requested value at this read cut.
    pub value: GatewayCheckedValueV1,
}

fn has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    permission: Permission,
) -> bool {
    world.accounts().get(authority).is_some()
        && (world.account_contains_inherent_permission(authority, &permission)
            || world
                .account_roles_iter(authority)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(|token| token == &permission)))
}

fn current(
    state: &impl StateReadOnly,
    request: &Request,
    cut: GatewayReadCut<'_>,
) -> Result<GatewayCurrentV1, Error> {
    request.validate().map_err(|_| Error::Invalid)?;
    if request.network_id != *state.network_id() {
        return Err(Error::BindingMismatch);
    }
    let current = storage::read_current(state.world(), &request.network_id, request.gateway_id)?
        .ok_or(Error::Unavailable)?;
    if current.policy.policy.qualification.revision != request.expected_policy_revision
        || current.policy.policy.qualification.policy_digest != request.expected_policy_digest
    {
        return Err(Error::Conflict);
    }
    current_at_cut(state, &current, cut)?;
    Ok(current)
}

fn current_at_cut(
    state: &impl StateReadOnly,
    current: &GatewayCurrentV1,
    cut: GatewayReadCut<'_>,
) -> Result<(), Error> {
    if cut.now() == 0
        || cut.now() == u64::MAX
        || !cut.contains(&current.policy.execution)
        || current.head.head.last_execution_unix_ms > cut.now()
    {
        return Err(Error::Unavailable);
    }
    if current.head.head.revision != 0 {
        let mutation = storage::read_mutation(
            state.world(),
            state.network_id(),
            current.policy.policy.qualification.gateway_id,
            current.head.head.revision,
        )?;
        if !cut.contains(&mutation.execution) {
            return Err(Error::CorruptHistory);
        }
    }
    Ok(())
}

fn roles(
    state: &impl StateReadOnly,
    current: &GatewayCurrentV1,
    check: &Check,
) -> Result<(), Error> {
    let policy = &current.policy.policy;
    let gateway_id = policy.qualification.gateway_id;
    if !policy.observers.contains(&check.expected_observer)
        || !policy.operators.contains(&check.expected_operator)
        || !has_permission(
            state.world(),
            &check.expected_observer,
            CanCheckSorafsStreamTokenGateway { gateway_id }.into(),
        )
        || !has_permission(
            state.world(),
            &check.expected_operator,
            CanOperateSorafsStreamTokenGateway { gateway_id }.into(),
        )
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn floor(state: &impl StateReadOnly, floor: Floor) -> Result<(), Error> {
    let block = committed_block(state, floor.height).map_err(|_| Error::Unavailable)?;
    if *block.block_hash().as_ref() != floor.block_hash || block.id() != floor.context_id {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn committed_cut(
    state: &impl StateReadOnly,
    now_ms: u64,
) -> Result<GatewayReadCut<'static>, Error> {
    let height = u64::try_from(state.block_hashes().len()).map_err(|_| Error::Invalid)?;
    if height == 0 || now_ms == 0 || now_ms == u64::MAX {
        return Err(Error::Unavailable);
    }
    Ok(GatewayReadCut::Committed {
        height,
        now_unix_ms: now_ms,
    })
}

/// Read a retained admission from the same exact current World and actual committed cut.
pub(crate) fn read_admission(
    state: &impl StateReadOnly,
    current: &GatewayCurrentV1,
    sequence: u64,
    now_ms: u64,
) -> Result<AdmissionRowV1, Error> {
    exact_current(state, current)?;
    let cut = committed_cut(state, now_ms)?;
    current_at_cut(state, current, cut)?;
    let rows = WorldGatewayRows::new(
        state.world(),
        state.network_id(),
        current.policy.policy.qualification.gateway_id,
    )?;
    read::admission(
        &current.policy.policy,
        current.head.head,
        cut,
        sequence,
        |key| rows.read(key),
    )
}

fn exact_current(state: &impl StateReadOnly, current: &GatewayCurrentV1) -> Result<(), Error> {
    if storage::read_current(
        state.world(),
        state.network_id(),
        current.policy.policy.qualification.gateway_id,
    )?
    .as_ref()
        != Some(current)
    {
        return Err(Error::Conflict);
    }
    Ok(())
}

/// Construct the exact complete oldest-pending prefix before committing an independent challenge.
/// This construction does not authorize a caller or authenticate a remote readback.
pub(crate) fn read_pending(
    state: &impl StateReadOnly,
    current: &GatewayCurrentV1,
    max_items: u32,
    now_ms: u64,
) -> Result<Readback, Error> {
    exact_current(state, current)?;
    let cut = committed_cut(state, now_ms)?;
    current_at_cut(state, current, cut)?;
    let rows = WorldGatewayRows::new(
        state.world(),
        state.network_id(),
        current.policy.policy.qualification.gateway_id,
    )?;
    pending(current, &rows, cut, max_items)
}

fn pending(
    current: &GatewayCurrentV1,
    rows: &impl GatewayRows,
    cut: GatewayReadCut<'_>,
    max_items: u32,
) -> Result<Readback, Error> {
    if max_items == 0 || max_items > STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1 {
        return Err(Error::Invalid);
    }
    let head = current.head.head;
    let remaining = head
        .high_water_sequence
        .checked_sub(head.acknowledged_through_sequence)
        .ok_or(Error::CorruptHistory)?;
    let count = remaining.min(u64::from(max_items));
    // Fixed-size public records only. Each original request is bounded/decoded lazily by the
    // World reader, validated and dropped; no vector of original payloads is accumulated.
    let mut readback = Readback {
        acknowledged_through_sequence: head.acknowledged_through_sequence,
        high_water_sequence: head.high_water_sequence,
        records: Vec::new(),
    };
    let mut aggregate = norito::canonical_frame_len(&readback).map_err(|_| Error::Invalid)?;
    for offset in 1..=count {
        let sequence = head
            .acknowledged_through_sequence
            .checked_add(offset)
            .ok_or(Error::Invalid)?;
        let original = read::admission(&current.policy.policy, head, cut, sequence, |key| {
            rows.read(key)
        })?;
        aggregate = aggregate
            .checked_add(norito::canonical_frame_len(&original.record).map_err(|_| Error::Invalid)?)
            .filter(|length| *length <= STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1)
            .ok_or(Error::Capacity)?;
        readback.records.push(original.record);
    }
    readback
        .validate(max_items, current.policy.policy.qualification)
        .map_err(|_| Error::CorruptHistory)?;
    Ok(readback)
}

fn subject(
    current: &GatewayCurrentV1,
    rows: &impl GatewayRows,
    cut: GatewayReadCut<'_>,
    subject: &Subject,
) -> Result<GatewayCheckedValueV1, Error> {
    let policy = &current.policy.policy;
    let head = current.head.head;
    Ok(match subject {
        Subject::Qualification => GatewayCheckedValueV1::Qualification,
        Subject::Admission {
            request_digest: expected,
            result,
        }
        | Subject::Serving {
            request_digest: expected,
            result,
        } => {
            let original = read::admission(
                policy,
                head,
                cut,
                result.record.outcome.binding.gateway_sequence,
                |key| rows.read(key),
            )?;
            if request_digest(&original.request)? != *expected || original.record != result.record {
                return Err(Error::Conflict);
            }
            let sequence = original.record.outcome.binding.gateway_sequence;
            let delivery = if sequence <= head.acknowledged_through_sequence {
                read::acknowledgement(&original, head, cut, |key| rows.read(key))?;
                Delivery::AcknowledgedExactReplay {
                    acknowledged_through_sequence: head.acknowledged_through_sequence,
                }
            } else {
                Delivery::Pending {
                    predecessor_sequence: sequence - 1,
                }
            };
            if result.delivery_state != delivery {
                return Err(Error::Conflict);
            }
            if matches!(subject, Subject::Serving { .. }) {
                let ack = read::acknowledgement(&original, head, cut, |key| rows.read(key))?;
                if !matches!(ack.reputation_delivery.disposition,
                    iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryDispositionV1::Delivered { .. }) {
                    return Err(Error::Unavailable);
                }
                if original.record.outcome.status != Status::Accepted
                    || !policy.allows_admission_at(cut.now())
                    || !matches!(delivery, Delivery::AcknowledgedExactReplay { .. })
                {
                    return Err(Error::Unavailable);
                }
                read::live_lease(&original, cut.now(), |key| rows.read(key))?;
                GatewayCheckedValueV1::Serving(*result)
            } else {
                GatewayCheckedValueV1::Admission(*result)
            }
        }
        Subject::Pending {
            max_items,
            readback_digest,
        } => {
            let readback = pending(current, rows, cut, *max_items)?;
            if stream_token_gateway_pending_readback_digest_v1(
                policy.qualification,
                *max_items,
                &readback,
            )
            .map_err(|_| Error::Invalid)?
                != *readback_digest
            {
                return Err(Error::Conflict);
            }
            GatewayCheckedValueV1::Pending(readback)
        }
        Subject::Acknowledged { record } => {
            let original = read::admission(
                policy,
                head,
                cut,
                record.outcome.binding.gateway_sequence,
                |key| rows.read(key),
            )?;
            if original.record != *record {
                return Err(Error::Conflict);
            }
            read::acknowledgement(&original, head, cut, |key| rows.read(key))?;
            GatewayCheckedValueV1::Acknowledged(*record)
        }
        Subject::Released { record } => {
            let original = read::admission(
                policy,
                head,
                cut,
                record.outcome.binding.gateway_sequence,
                |key| rows.read(key),
            )?;
            if original.record != *record {
                return Err(Error::Conflict);
            }
            read::terminal(&original, head, cut, |key| rows.read(key))?
                .ok_or(Error::Unavailable)?;
            GatewayCheckedValueV1::Released(*record)
        }
    })
}

/// Re-evaluate current policy, permissions and exact subject at an independently sampled time.
/// The caller must still authenticate the signed challenged Check and historical executions.
pub(crate) fn evaluate_current(
    state: &impl StateReadOnly,
    request: &Request,
    now_ms: u64,
) -> Result<GatewayCheckedReadV1, Error> {
    let read = evaluate_current_rows(state, request, now_ms)?;
    let Action::Check(check) = &request.action else {
        return Err(Error::Invalid);
    };
    floor(state, check.floor)?;
    Ok(read)
}

/// Re-evaluate only current in-memory World rows, permissions and exact subject eligibility.
///
/// This does not read Kura or authenticate the submitted floor. The purpose-owned publisher must
/// first verify durable floor, Check and original-history evidence outside its publication lease,
/// then bind this fresh State view to that same proven source and lease generation. The returned
/// claim is not a proof or capability and cannot replace that verification.
pub(crate) fn evaluate_current_rows(
    state: &impl StateReadOnly,
    request: &Request,
    now_ms: u64,
) -> Result<GatewayCheckedReadV1, Error> {
    let cut = committed_cut(state, now_ms)?;
    let current = current(state, request, cut)?;
    let Action::Check(check) = &request.action else {
        return Err(Error::Invalid);
    };
    roles(state, &current, check)?;
    let rows = WorldGatewayRows::new(state.world(), state.network_id(), request.gateway_id)?;
    let value = subject(&current, &rows, cut, &check.subject)?;
    Ok(GatewayCheckedReadV1 { current, value })
}

/// Evaluate the exact direct native Check without writing any World row.
/// Native authorization must derive `execution` from the actual signed source instruction.
pub(crate) fn evaluate_check(
    tx: &StateTransaction<'_, '_>,
    request: &Request,
    execution: &Execution,
) -> Result<GatewayCheckedReadV1, Error> {
    request.validate().map_err(|_| Error::Invalid)?;
    let Action::Check(check) = &request.action else {
        return Err(Error::Invalid);
    };
    if execution.authority != check.expected_observer
        || execution.transaction_hash == [0; 32]
        || execution.height != tx._curr_block.height().get()
        || execution.recorded_at_unix_ms != tx.block_unix_timestamp_ms()
        || check.floor.height >= execution.height
    {
        return Err(Error::BindingMismatch);
    }
    floor(tx, check.floor)?;
    let cut = GatewayReadCut::Execution(execution);
    let current = current(tx, request, cut)?;
    roles(tx, &current, check)?;
    let rows = WorldGatewayRows::new(tx.world(), tx.network_id(), request.gateway_id)?;
    let value = subject(&current, &rows, cut, &check.subject)?;
    Ok(GatewayCheckedReadV1 { current, value })
}
