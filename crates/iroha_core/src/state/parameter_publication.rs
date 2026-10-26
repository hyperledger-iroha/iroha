//! Source-bound parameter and event preparation before any mutable MV access.
//!
//! The parameter graph and shared event graph retain complete original-pool
//! custody. Destination output vectors are physically reserved before install;
//! TODO(S8): complete their whole-lifetime finite-pool ownership in the output task.

use super::{StateTransaction, WorldTransaction};
use crate::{
    execution_attempt::ExecutionDeferred,
    pipeline_gas_policy::{
        ParameterIngressAttemptError, ParameterRetentionError, PreparedParameters,
    },
    smartcontracts::isi::sccp::params::SccpPolicyError,
};
use iroha_allocation::{AllocationRefusal, AllocatorRefusal};
use iroha_data_model::{
    block::consensus::{RootFeePolicy, RootFeePolicyError, SumeragiRootScope},
    events::{EventBox, SharedDataEvent},
    parameter::Parameter,
};
use mv::storage::StorageReadOnly;
use std::{alloc::Layout, num::NonZeroU64};

mod events;

/// Exact semantic or original local cause before parameter publication.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ParameterChangeError {
    /// Actual graph/event parsing and original resource provenance.
    #[error(transparent)]
    Ingress(#[from] ParameterIngressAttemptError),
    /// Exact source version, pool or mandatory-policy check failed.
    #[error(transparent)]
    Ownership(#[from] ParameterRetentionError),
    /// Root-specific fee policy validation failed.
    #[error(transparent)]
    Fees(#[from] RootFeePolicyError),
    /// A coherent ZK successor would strand retained confidential transitions.
    #[error(transparent)]
    ZkPending(#[from] crate::zk_validity_policy::ZkValidityPolicyChangeError),
    /// Existing SCCP liabilities cannot be supported by the candidate.
    #[error(transparent)]
    Sccp(#[from] SccpPolicyError),
    /// The instruction does not supply the required original authority.
    #[error("{0}")]
    Invalid(&'static str),
    /// The original physical output reservation failed before mutation.
    #[error(transparent)]
    Deferred(#[from] ExecutionDeferred),
}

struct Destination<T> {
    original_len: usize,
    replacement: Option<Vec<T>>,
}
impl<T> Destination<T> {
    fn prepare(current: &Vec<T>, preceding_capacity: usize) -> Result<Self, ParameterChangeError> {
        let required = current
            .len()
            .checked_add(preceding_capacity)
            .and_then(|length| length.checked_add(1))
            .ok_or_else(|| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
        let replacement = if current.capacity() >= required {
            None
        } else {
            let capacity = current
                .capacity()
                .checked_mul(2)
                .map_or(required, |grown| grown.max(required));
            let layout = Layout::array::<T>(capacity)
                .map_err(|_| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
            let mut buffer = Vec::new();
            buffer.try_reserve_exact(capacity).map_err(|original| {
                ExecutionDeferred::from(AllocatorRefusal::from_vec_reserve(original, layout))
            })?;
            Some(buffer)
        };
        Ok(Self {
            original_len: current.len(),
            replacement,
        })
    }
    fn matches(&self, current: &Vec<T>) -> bool {
        current.len() >= self.original_len
            && self.replacement.as_ref().map_or_else(
                || current.capacity() > current.len(),
                |replacement| replacement.capacity() > current.len(),
            )
    }
    fn install(self, current: &mut Vec<T>, value: T) {
        if let Some(mut next) = self.replacement {
            // Capacity and both source lengths were checked before any mutation.
            // append moves original handles; no payload or nested graph is cloned.
            next.append(current);
            *current = next;
        }
        current.push(value);
    }
}

// Only the native FX operation may place events before its final parameter notification.
// Its existing execution path emits six transfer events, at most two destination
// Asset::Created events, and at most one transfer-control metadata event per leg.
// Retail observation, UID writes and transcript publication emit no further events.
#[derive(Clone, Copy)]
enum EventSequence {
    Immediate,
    NativeFxPair,
}
impl EventSequence {
    fn preceding_capacity(self) -> usize {
        match self {
            Self::Immediate => 0,
            Self::NativeFxPair => 10,
        }
    }
    fn matches(self, external: Option<usize>, internal: Option<usize>) -> bool {
        if external != internal {
            return false;
        }
        match (self, external) {
            (Self::Immediate, Some(0)) => true,
            (Self::NativeFxPair, Some(count)) => (6..=10).contains(&count),
            _ => false,
        }
    }
}

/// Closed successor and notification prepared against one exact retained source.
/// No constructor accepts a raw graph or an already decoded unowned projection.
pub(crate) struct PreparedParameterChange {
    parameters: PreparedParameters,
    event: SharedDataEvent,
    external: Destination<EventBox>,
    internal: Destination<SharedDataEvent>,
    sequence: EventSequence,
}
impl StateTransaction<'_, '_> {
    /// Prepare all parameter/event backing and validate monetary scope before a writer.
    /// Ordinary instruction authorization and signed-handshake validation remain
    /// with the caller. A cadence is only supplied by that authenticated handshake.
    pub(crate) fn prepare_parameter_change(
        &self,
        next: &Parameter,
        cadence: Option<NonZeroU64>,
        root_scope: Option<SumeragiRootScope>,
    ) -> Result<PreparedParameterChange, ParameterChangeError> {
        self.prepare_parameter_change_with_events(
            next,
            cadence,
            root_scope,
            EventSequence::Immediate,
        )
    }

    /// Prepare native FX usage with room for the pair's existing ordered events.
    /// The final configuration event stays after those effects. All other
    /// parameter callers retain exact, unchanged output geometry.
    pub(crate) fn prepare_fx_usage_parameter_change(
        &self,
        next: &Parameter,
    ) -> Result<PreparedParameterChange, ParameterChangeError> {
        if !matches!(next, Parameter::Custom(custom)
            if custom.id().name().as_ref()
                == iroha_data_model::isi::settlement::FxCorridorPolicyRegistry::PARAMETER_ID_STR)
        {
            return Err(ParameterChangeError::Invalid(
                "native FX output preparation requires its usage registry",
            ));
        }
        self.prepare_parameter_change_with_events(next, None, None, EventSequence::NativeFxPair)
    }

    fn prepare_parameter_change_with_events(
        &self,
        next: &Parameter,
        cadence: Option<NonZeroU64>,
        root_scope: Option<SumeragiRootScope>,
        sequence: EventSequence,
    ) -> Result<PreparedParameterChange, ParameterChangeError> {
        let pool = self.world.sccp_committee_generations.allocation_budget();
        let current = self.world.parameters.get();
        let parameters = ParameterIngressAttemptError::observe(|| {
            current.prepare_parameter_with_cadence(next, cadence, pool)
        })?;
        if let Parameter::Custom(custom) = next {
            if custom.id().name().as_ref()
                == iroha_data_model::zk_validity_policy::ZkValidityPolicyV1::PARAMETER_ID_STR
            {
                crate::zk_validity_policy::validate_pending_transitions(
                    parameters.successor().zk_policy(),
                    self.world
                        .confidential_policy_transition_counts
                        .iter()
                        .map(|(height, count)| (*height, *count)),
                )?;
            }
            if self._curr_block.is_genesis()
                && custom.id().name().as_ref()
                    == iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::PARAMETER_ID_STR
                && current.crypto_policy() != parameters.successor().crypto_policy()
            {
                return Err(ParameterChangeError::Invalid(
                    "genesis instruction changes or invents its authenticated initial crypto policy",
                ));
            }
            if custom.id().name().as_ref() == RootFeePolicy::PARAMETER_ID_STR {
                let scope = root_scope.ok_or(ParameterChangeError::Invalid(
                    "root fee update requires signed root scope",
                ))?;
                let proposed =
                    parameters
                        .successor()
                        .root_fees()
                        .ok_or(ParameterChangeError::Invalid(
                            "root fee successor is absent",
                        ))?;
                proposed.validate_for_scope(&scope)?;
                if !self._curr_block.is_genesis()
                    && !cfg!(all(test, sumeragi_core_mutation = "HC399"))
                    && let Some(original) = current.root_fees()
                    && (original.fee_asset_id != proposed.fee_asset_id
                        || original.sponsor_vault_custody_account_id
                            != proposed.sponsor_vault_custody_account_id)
                {
                    return Err(ParameterChangeError::Invalid(
                        "root fee currency and sponsor custody are immutable after genesis",
                    ));
                }
            }
            if custom.id().name().as_ref()
                == iroha_data_model::block::consensus::PublicLaneStakingPolicy::PARAMETER_ID_STR
            {
                let policy = parameters
                    .successor()
                    .staking_policy()
                    .ok_or(ParameterChangeError::Invalid("staking successor is absent"))?;
                crate::smartcontracts::isi::sccp::params::validate_staking_successor(
                    &*self.world,
                    policy,
                )?;
            }
        }
        let event = ParameterIngressAttemptError::observe(|| {
            events::for_change(current, parameters.successor(), next, pool)
        })?;
        // These actual empty-vector allocations precede graph installation. S8
        // still owns their eventual transaction/block/output lifetime admission.
        let external = Destination::prepare(
            &self.world.external_event_buf,
            sequence.preceding_capacity(),
        )?;
        let internal = Destination::prepare(
            &self.world.internal_event_buf,
            sequence.preceding_capacity(),
        )?;
        Ok(PreparedParameterChange {
            parameters,
            event,
            external,
            internal,
            sequence,
        })
    }
}
impl WorldTransaction<'_, '_> {
    /// Install only after exact source and output geometry checks. No operation
    /// after extraction allocates, parses, validates or can return an error.
    pub(crate) fn install_parameter_change(
        &mut self,
        prepared: PreparedParameterChange,
    ) -> Result<(), ParameterChangeError> {
        let pool = self.sccp_committee_generations.allocation_budget();
        if !prepared.external.matches(&self.external_event_buf)
            || !prepared.internal.matches(&self.internal_event_buf)
            || !prepared.sequence.matches(
                self.external_event_buf
                    .len()
                    .checked_sub(prepared.external.original_len),
                self.internal_event_buf
                    .len()
                    .checked_sub(prepared.internal.original_len),
            )
        {
            return Err(ParameterChangeError::Ownership(
                ParameterRetentionError::StalePredecessor,
            ));
        }
        let PreparedParameterChange {
            parameters,
            event,
            external,
            internal,
            sequence: _,
        } = prepared;
        let successor = parameters
            .into_successor(self.parameters.get(), pool)
            .map_err(|(_prepared, original)| original)?;
        *self.parameters.get_mut() = successor;
        external.install(&mut self.external_event_buf, EventBox::Data(event.clone()));
        internal.install(&mut self.internal_event_buf, event);
        // Configuration events have no directory/authority projection hooks.
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry {
            telemetry.ingest_data_event(
                self.internal_event_buf
                    .last()
                    .expect("prepared event installed"),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

impl StateTransaction<'_, '_> {
    /// Project the actual preparation result at the instruction boundary while
    /// retaining every original local resource owner and forbidding publication.
    pub(crate) fn parameter_change_instruction_error(
        &mut self,
        error: ParameterChangeError,
    ) -> iroha_data_model::isi::error::InstructionExecutionError {
        use crate::execution_attempt::ExecutionAttemptError;
        use iroha_data_model::isi::error::{InstructionExecutionError, InvalidParameterError};
        let rejected = |error: ParameterChangeError| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                error.to_string(),
            ))
        };
        let attempt = match error {
            ParameterChangeError::Deferred(original) => ExecutionAttemptError::Deferred(original),
            ParameterChangeError::Ingress(actual) => {
                parameter_ingress_attempt_error(actual, |original| {
                    rejected(ParameterChangeError::Ingress(
                        ParameterIngressAttemptError::Original(original),
                    ))
                })
            }
            ParameterChangeError::Ownership(
                ParameterRetentionError::ForeignPool
                | ParameterRetentionError::StalePredecessor
                | ParameterRetentionError::PlanMismatch,
            ) => ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
            ),
            other => ExecutionAttemptError::Rejected(rejected(other)),
        };
        self.attempt_error_to_instruction_error(attempt)
    }
}

/// Preserve the actual graph operation's scoped decoder and physical owners at
/// both instruction and authenticated genesis boundaries. Error decoration runs
/// only after resource classification and cannot turn refusal into invalid input.
pub(crate) fn parameter_ingress_attempt_error<E>(
    actual: ParameterIngressAttemptError,
    invalid: impl FnOnce(crate::pipeline_gas_policy::ParameterIngressError) -> E,
) -> crate::execution_attempt::ExecutionAttemptError<E> {
    use crate::execution_attempt::ExecutionAttemptError;
    match actual {
        ParameterIngressAttemptError::Codec(original) => {
            crate::execution_attempt::canonical_decode_attempt_error(original, |original| {
                invalid(original.into_error().into())
            })
        }
        ParameterIngressAttemptError::Original(original) => match direct_resource(original) {
            Ok(original) => ExecutionAttemptError::Deferred(original),
            Err(original) => ExecutionAttemptError::Rejected(invalid(original)),
        },
    }
}

/// Only direct ingress is used by preparation. It never calls the older complete
/// raw-graph foreign-copy operation; that operation's native BigInt clone error
/// is not converted or reconstructed into this execution provenance.
fn direct_resource(
    original: crate::pipeline_gas_policy::ParameterIngressError,
) -> Result<ExecutionDeferred, crate::pipeline_gas_policy::ParameterIngressError> {
    use crate::pipeline_gas_policy::ParameterIngressError as E;
    use iroha_allocation::{
        ChargedBufferError, ChargedBufferFromChargeError, PrepaidBufferError, PrepaidSharedError,
    };
    use iroha_data_model::parameter::CustomParametersAdmissionError;
    use iroha_primitives::json::JsonRetentionError;
    fn buffer(original: ChargedBufferError) -> ExecutionDeferred {
        match original {
            ChargedBufferError::Admission(original) => original.into(),
            original @ ChargedBufferError::Allocator { .. } => AllocatorRefusal::try_from(original)
                .expect("matched actual allocator failure")
                .into(),
        }
    }
    match original {
        E::Allocation(original) => Ok(buffer(original)),
        E::Retention(ParameterRetentionError::Admission(original)) => Ok(original.into()),
        E::Retention(ParameterRetentionError::Buffer(PrepaidBufferError::Allocation(original))) => {
            Ok(buffer(original))
        }
        E::Retention(ParameterRetentionError::Shared(
            original @ PrepaidSharedError::Allocator { .. },
        )) => Ok(AllocatorRefusal::try_from(original)
            .expect("matched actual shared allocator failure")
            .into()),
        E::Retention(ParameterRetentionError::Custom(
            CustomParametersAdmissionError::Admission(original),
        )) => Ok(original.into()),
        E::Retention(ParameterRetentionError::Custom(CustomParametersAdmissionError::Buffer(
            PrepaidBufferError::Allocation(original),
        ))) => Ok(buffer(original)),
        E::Json(JsonRetentionError::Admission(original)) => Ok(original.into()),
        E::Json(JsonRetentionError::Scratch(original)) => Ok(buffer(original)),
        E::Json(JsonRetentionError::Shared(original)) => {
            Ok(AllocatorRefusal::from(original).into())
        }
        E::Json(JsonRetentionError::Text(
            original @ ChargedBufferFromChargeError::Allocator { .. },
        )) => Ok(AllocatorRefusal::try_from(original)
            .expect("matched original JSON text allocator failure")
            .into()),
        // These are contradictions of the private original-pool layout plan,
        // never invalid transaction bytes and never synthetic allocator wakes.
        E::Retention(
            ParameterRetentionError::Buffer(PrepaidBufferError::Reservation(_))
            | ParameterRetentionError::Shared(PrepaidSharedError::Reservation(_))
            | ParameterRetentionError::ForeignPool
            | ParameterRetentionError::StalePredecessor
            | ParameterRetentionError::PlanMismatch,
        )
        | E::Retention(ParameterRetentionError::Custom(
            CustomParametersAdmissionError::Buffer(PrepaidBufferError::Reservation(_))
            | CustomParametersAdmissionError::RetainedMutation
            | CustomParametersAdmissionError::KeyValueMismatch,
        ))
        | E::Json(JsonRetentionError::Text(
            ChargedBufferFromChargeError::DemandOverflow
            | ChargedBufferFromChargeError::LayoutMismatch { .. },
        )) => Ok(ivm::error::ExecutionDeferral::LocalInvariantViolation.into()),
        other => Err(other),
    }
}
