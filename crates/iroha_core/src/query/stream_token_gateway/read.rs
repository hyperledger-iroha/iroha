//! Shared read-only predicates over exact native gateway rows.
//!
//! The supplied reader must authenticate typed rows and their original policy in one World.
//! These predicates produce no finality capability, permission grant or mutation delta.

use iroha_data_model::sorafs::stream_token_gateway::native::{
    StreamTokenGatewayExecutionV1 as Execution, StreamTokenGatewayPolicyV1 as Policy,
};
use sorafs_manifest::token::{
    STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1, STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1,
    STREAM_TOKEN_MAX_STREAMS_V1,
};

use super::{
    rows::*,
    transition::{lease_expiry, quota_expiry, request_digest, token_identity},
};

/// Actual execution prefix or complete committed block cut; neither variant invents an execution.
#[derive(Clone, Copy)]
pub(crate) enum GatewayReadCut<'a> {
    /// Exact current native instruction context, supplied only by the executor.
    Execution(&'a Execution),
    /// All successful instructions at or before an actual committed State height.
    Committed { height: u64, now_unix_ms: u64 },
}

impl GatewayReadCut<'_> {
    /// Actual block time for execution, or independently supplied current read time.
    pub(crate) fn now(self) -> u64 {
        match self {
            Self::Execution(execution) => execution.recorded_at_unix_ms,
            Self::Committed { now_unix_ms, .. } => now_unix_ms,
        }
    }

    /// Reject future or malformed retained execution coordinates.
    pub(crate) fn contains(self, execution: &Execution) -> bool {
        if execution.height == 0
            || execution.transaction_hash == [0; 32]
            || execution.recorded_at_unix_ms == 0
            || execution.recorded_at_unix_ms > self.now()
        {
            return false;
        }
        match self {
            Self::Execution(at) => position(execution) <= position(at),
            Self::Committed { height, .. } => execution.height <= height,
        }
    }
}

fn position(execution: &Execution) -> (u64, u32, u32) {
    (
        execution.height,
        execution.entry_index,
        execution.instruction_index,
    )
}

/// Shared lazy row lookup, including a transition's staged writes when called by that owner.
pub(crate) type ReadResult = Result<Option<GatewayRow>, TransitionError>;

/// Load and validate one permanent original admission and its exact replay index.
pub(crate) fn admission(
    policy: &Policy,
    head: GatewayHeadV1,
    cut: GatewayReadCut<'_>,
    sequence: u64,
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<AdmissionRowV1, TransitionError> {
    let Some(GatewayRow::Admission(row)) = get(&GatewayRowKey::Admission(sequence))? else {
        return Err(TransitionError::CorruptHistory);
    };
    if sequence == 0
        || sequence > head.high_water_sequence
        || row.record.outcome.binding.gateway_sequence != sequence
        || !cut.contains(&row.execution)
        || row.execution.recorded_at_unix_ms < row.request.validated_at_unix_ms
        || row.execution.recorded_at_unix_ms > head.last_execution_unix_ms
    {
        return Err(TransitionError::CorruptHistory);
    }
    row.record
        .validate_for_request(&row.request, policy.qualification)
        .map_err(|_| TransitionError::CorruptHistory)?;
    let context = row
        .request
        .context
        .digest()
        .map_err(|_| TransitionError::CorruptHistory)?;
    if get(&GatewayRowKey::Context(context))?
        != Some(GatewayRow::Context(ContextRowV1 {
            sequence,
            request_digest: request_digest(&row.request)?,
        }))
    {
        return Err(TransitionError::CorruptHistory);
    }
    Ok(row)
}

/// Load a quota incarnation, preserving the distinction between retirement and missing state.
pub(crate) fn quota(
    scope: [u8; 32],
    now: u64,
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<Option<QuotaRowV1>, TransitionError> {
    let lifecycle = lifecycle(scope, &get)?;
    match get(&GatewayRowKey::Quota(scope))? {
        Some(GatewayRow::Quota(row)) => {
            if !lifecycle.active
                || row.generation != lifecycle.generation
                || row.generation == 0
                || row.request_window_start_ms == 0
                || row.byte_window_start_ms == 0
                || row.request_window_start_ms > now
                || row.byte_window_start_ms > now
                || row.retire_at_unix_ms <= row.request_window_start_ms
                || row.retire_at_unix_ms <= row.byte_window_start_ms
                || row.active_leases > STREAM_TOKEN_MAX_STREAMS_V1
                || row.requests_used > STREAM_TOKEN_MAX_REQUESTS_PER_MINUTE_V1
                || row.bytes_used > STREAM_TOKEN_MAX_RATE_LIMIT_BYTES_V1
            {
                return Err(TransitionError::CorruptHistory);
            }
            let key = quota_expiry(scope, row.retire_at_unix_ms);
            if get(&GatewayRowKey::Expiry(key))? != Some(GatewayRow::Expiry(key)) {
                return Err(TransitionError::CorruptHistory);
            }
            Ok(Some(row))
        }
        None if !lifecycle.active => Ok(None),
        _ => Err(TransitionError::CorruptHistory),
    }
}

/// Load the permanent lifecycle marker independently of any live row.
pub(crate) fn lifecycle(
    scope: [u8; 32],
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<QuotaLifecycleV1, TransitionError> {
    match get(&GatewayRowKey::QuotaLifecycle(scope))? {
        Some(GatewayRow::QuotaLifecycle(row)) if !row.active || row.generation != 0 => Ok(row),
        _ => Err(TransitionError::CorruptHistory),
    }
}

/// Authenticate the original live lease, immutable identity and active quota incarnation.
pub(crate) fn live_lease(
    original: &AdmissionRowV1,
    now: u64,
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<LeaseRowV1, TransitionError> {
    let id = original
        .record
        .lease_id
        .ok_or(TransitionError::Unavailable)?;
    if get(&GatewayRowKey::LeaseTerminal(id))?.is_some()
        || original
            .record
            .lease_expires_at_unix_ms
            .is_none_or(|expiry| expiry <= now)
    {
        return Err(TransitionError::Unavailable);
    }
    let Some(GatewayRow::Lease(lease)) = get(&GatewayRowKey::Lease(id))? else {
        return Err(TransitionError::CorruptHistory);
    };
    let (scope, identity) = token_identity(&original.request)?;
    let quota = quota(scope, now, &get)?.ok_or(TransitionError::CorruptHistory)?;
    let expiry = lease_expiry(id, lease.expires_at_unix_ms);
    if lease.sequence != original.record.outcome.binding.gateway_sequence
        || Some(lease.expires_at_unix_ms) != original.record.lease_expires_at_unix_ms
        || lease.token_scope != scope
        || lease.quota_generation != quota.generation
        || quota.active_leases == 0
        || quota.retire_at_unix_ms <= now
        || identity.expires_at_unix_ms <= now
        || get(&GatewayRowKey::TokenIdentity(scope))? != Some(GatewayRow::TokenIdentity(identity))
        || get(&GatewayRowKey::Expiry(expiry))? != Some(GatewayRow::Expiry(expiry))
    {
        return Err(TransitionError::CorruptHistory);
    }
    Ok(lease)
}

/// Load the permanent first ordered acknowledgement, retaining its original execution.
pub(crate) fn acknowledgement(
    original: &AdmissionRowV1,
    head: GatewayHeadV1,
    cut: GatewayReadCut<'_>,
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<AcknowledgementRowV1, TransitionError> {
    let record = original.record;
    let sequence = record.outcome.binding.gateway_sequence;
    if sequence == 0 || sequence > head.acknowledged_through_sequence {
        return Err(TransitionError::Unavailable);
    }
    let Some(GatewayRow::Acknowledgement(ack)) = get(&GatewayRowKey::Acknowledgement(sequence))?
    else {
        return Err(TransitionError::CorruptHistory);
    };
    if ack.reputation_delivery.source_digest == [0; 32]
        || matches!(ack.reputation_delivery.disposition,
            iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryDispositionV1::Pending)
        || matches!(ack.reputation_delivery.disposition,
            iroha_data_model::sorafs::reputation::stream_token_delivery::StreamTokenReputationDeliveryDispositionV1::Excluded)
            == record.outcome.status.counts_for_provider()
        || ack.record != record
        || ack.policy_revision == 0
        || !cut.contains(&ack.execution)
        || position(&ack.execution) < position(&original.execution)
        || ack.execution.recorded_at_unix_ms < original.execution.recorded_at_unix_ms
        || ack.execution.recorded_at_unix_ms > head.last_execution_unix_ms
    {
        return Err(TransitionError::CorruptHistory);
    }
    Ok(ack)
}

/// Load an exact terminal grant; a missing terminal is not evidence of completed release.
pub(crate) fn terminal(
    original: &AdmissionRowV1,
    head: GatewayHeadV1,
    cut: GatewayReadCut<'_>,
    get: impl Fn(&GatewayRowKey) -> ReadResult,
) -> Result<Option<LeaseTerminalV1>, TransitionError> {
    let id = original.record.lease_id.ok_or(TransitionError::Invalid)?;
    let Some(GatewayRow::Lease(lease)) = get(&GatewayRowKey::Lease(id))? else {
        return Err(TransitionError::CorruptHistory);
    };
    if lease.sequence != original.record.outcome.binding.gateway_sequence
        || original.record.lease_expires_at_unix_ms != Some(lease.expires_at_unix_ms)
        || token_identity(&original.request)?.0 != lease.token_scope
    {
        return Err(TransitionError::CorruptHistory);
    }
    match get(&GatewayRowKey::LeaseTerminal(id))? {
        Some(GatewayRow::LeaseTerminal(terminal)) => {
            if terminal.grant != lease
                || terminal.policy_revision == 0
                || !cut.contains(&terminal.execution)
                || position(&terminal.execution) < position(&original.execution)
                || terminal.execution.recorded_at_unix_ms < original.execution.recorded_at_unix_ms
                || terminal.execution.recorded_at_unix_ms > head.last_execution_unix_ms
                || (terminal.expired
                    && terminal.execution.recorded_at_unix_ms < lease.expires_at_unix_ms)
                || get(&GatewayRowKey::Expiry(lease_expiry(
                    id,
                    lease.expires_at_unix_ms,
                )))?
                .is_some()
            {
                return Err(TransitionError::CorruptHistory);
            }
            Ok(Some(terminal))
        }
        None => Ok(None),
        Some(_) => Err(TransitionError::CorruptHistory),
    }
}
