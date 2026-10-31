//! One original ordinary schedule graph retained outside disposable World overlays.
//!
//! The bank lends physical custody, never State authority. Every preparation and lend checks
//! the current pristine cut and freshly verified pulse; rollback returns the same graph even
//! after its fixed post-execution fields were completed. Boundary election custody remains
//! separate: this owner cannot manufacture or substitute a frozen boundary decision.

use super::{ChainParamsRecord, NativeExecutionInputs, RetainedConsensusSchedule, ScheduleError};
use crate::{
    execution_attempt::ExecutionAttemptError,
    state::{StateReadOnly, WorldReadOnly},
    sumeragi::{epoch_beacon, epoch_election},
};
use iroha_allocation::{AllocationBudget, ChargedShared, PrepaidSharedError, RetainedPayload};
use iroha_data_model::{
    block::{BlockHeader, consensus::SumeragiRootScope},
    consensus::{FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconPulseContextV1},
};
use parking_lot::Mutex;

type Attempt<T> = Result<T, ExecutionAttemptError<ScheduleError>>;

/// One attempt's bank, allocated before any disposable World owner.
#[derive(Default)]
pub struct ContinuationPreparation {
    bank: Option<ChargedShared<Bank>>,
}

struct Bank {
    pool: AllocationBudget,
    contents: Mutex<Contents>,
}
struct Contents {
    original: Original,
    value: Value,
}
struct Original {
    header: BlockHeader,
    // This shares the existing immutable funded State graph, not a copied DTO.
    schedule: RetainedConsensusSchedule,
    params: ChainParamsRecord,
    root_scope: SumeragiRootScope,
    pulse: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    link: Option<crate::beacon::GlobalThresholdBeaconPulseLinkV1>,
}
#[expect(
    clippy::large_enum_variant,
    reason = "retain the actual paid graph without another shell"
)]
enum Value {
    Pending,
    Available(RetainedPayload<NativeExecutionInputs>),
    Lent,
    Consumed,
}

fn invalid(reason: &'static str) -> ExecutionAttemptError<ScheduleError> {
    ScheduleError::Epoch(reason.into()).into()
}
fn invariant() -> ExecutionAttemptError<ScheduleError> {
    ExecutionAttemptError::Deferred(ivm::error::ExecutionDeferral::LocalInvariantViolation.into())
}

impl Original {
    fn require(
        &self,
        state: &(impl StateReadOnly + ?Sized),
        header: &BlockHeader,
        root_scope: SumeragiRootScope,
        pulse: &epoch_beacon::VerifiedEpochPulse,
    ) -> Attempt<()> {
        if self.header != *header
            || self.schedule != *state.world().consensus_schedule()
            || self.params
                != ChainParamsRecord::from_parameters(state.world().parameters().sumeragi())
            || self.root_scope != root_scope
            || self.pulse != pulse.pulse()
            || self.link != pulse.link()
        {
            return Err(invalid(
                "ordinary schedule preparation differs from its pristine source",
            ));
        }
        if !self.schedule.belongs_to(&state.execution_budget()) {
            return Err(invariant());
        }
        Ok(())
    }
}

impl ContinuationPreparation {
    /// Prepare only ordinary successors before World acquisition. Boundary processing keeps its
    /// existing authenticated election path; an ordinary owner can never be relabelled boundary.
    pub(crate) fn prepare(
        &mut self,
        state: &(impl StateReadOnly + ?Sized),
        header: &BlockHeader,
        supplied: Option<FinalizedGlobalThresholdBeaconPulseV1>,
        expected: GlobalThresholdBeaconPulseContextV1,
    ) -> Attempt<()> {
        super::require_successor_cut(state, header)?;
        let height = header.height().get();
        let schedule = state.world().consensus_schedule();
        if schedule.tip().and_then(|tip| tip.checked_add(1)) != Some(height) {
            return Err(ScheduleError::Malformed.into());
        }
        let current = &schedule.ready(height)?.epoch;
        let root_scope = crate::sumeragi::lanes::routing::committed_root_scope(state.world())
            .ok_or_else(|| invalid("native control requires immutable root scope"))?;
        let pulse = epoch_beacon::capture(
            root_scope,
            state.world(),
            state.block_hashes(),
            current,
            height,
            supplied,
            Some(expected),
        )
        .map_err(ScheduleError::Epoch)?;
        if current.mode == iroha_data_model::parameter::system::ConsensusMode::Npos
            && height == current.authorization.last_height
        {
            // TODO: retain the full frozen boundary candidate/election owner across World
            // rollback. Ordinary continuation custody grants no boundary authority.
            if self.bank.is_some() {
                return Err(invalid(
                    "ordinary schedule owner cannot change to a boundary",
                ));
            }
            return Ok(());
        }
        let pool = state.execution_budget();
        pool.with_deferred_refund_notifications(|_| {
            if self.bank.is_none() {
                if !schedule.belongs_to(&pool) {
                    return Err(invariant());
                }
                let mut reservation = pool
                    .try_reserve(ChargedShared::<Bank>::allocation_layout())
                    .map_err(|error| ExecutionAttemptError::Deferred(error.into()))?;
                let bank = Bank {
                    pool: pool.clone(),
                    contents: Mutex::new(Contents {
                        original: Original {
                            header: *header,
                            schedule: schedule.clone(),
                            params: ChainParamsRecord::from_parameters(
                                state.world().parameters().sumeragi(),
                            ),
                            root_scope,
                            pulse: pulse.pulse(),
                            link: pulse.link(),
                        },
                        value: Value::Pending,
                    }),
                };
                self.bank = Some(
                    ChargedShared::from_reservation(bank, &mut reservation).map_err(
                        |(_, error)| match error {
                            original @ PrepaidSharedError::Allocator { .. } => {
                                ExecutionAttemptError::Deferred(
                                    iroha_allocation::AllocatorRefusal::try_from(original)
                                        .expect("matched original physical schedule shell refusal")
                                        .into(),
                                )
                            }
                            PrepaidSharedError::Reservation(_) => invariant(),
                        },
                    )?,
                );
            }
            let bank = self
                .bank
                .as_ref()
                .expect("original schedule bank installed");
            if !bank.belongs_to(&pool) {
                return Err(invariant());
            }
            let mut contents = bank.contents.lock();
            contents
                .original
                .require(state, header, root_scope, &pulse)?;
            match contents.value {
                Value::Available(_) => return Ok(()),
                Value::Lent | Value::Consumed => return Err(invariant()),
                Value::Pending => {}
            }
            // Install the shell before this admission. Refusal retains that successful owner
            // instead of refunding a prefix which could satisfy its own original wait.
            let next = height.checked_add(1).ok_or(ScheduleError::HeightOverflow)?;
            let value = epoch_election::capture_continuation(
                &contents.original.schedule.ready(height)?.epoch,
                height,
                contents.original.schedule.get(next),
                contents.original.params,
                &pool,
            )
            .map_err(super::execution::boundary_capture_attempt_error)?;
            contents.value = Value::Available(value);
            Ok(())
        })
    }

    /// Lend the sole prepared graph after the caller's independent successor authentication and
    /// fresh pulse verification. A concurrent/reentrant lend or an unprepared owner fails closed.
    pub(crate) fn lend(
        &self,
        state: &(impl StateReadOnly + ?Sized),
        header: &BlockHeader,
        root_scope: SumeragiRootScope,
        pulse: &epoch_beacon::VerifiedEpochPulse,
    ) -> Attempt<ContinuationLease> {
        super::require_successor_cut(state, header)?;
        let bank = self.bank.as_ref().ok_or_else(invariant)?;
        let pool = state.execution_budget();
        if !bank.belongs_to(&pool) {
            return Err(invariant());
        }
        pool.with_deferred_refund_notifications(|_| {
            let mut contents = bank.contents.lock();
            contents
                .original
                .require(state, header, root_scope, pulse)?;
            if !matches!(contents.value, Value::Available(_)) {
                return Err(invariant());
            }
            let Value::Available(value) = std::mem::replace(&mut contents.value, Value::Lent)
            else {
                unreachable!("available original owner checked under the same guard")
            };
            Ok(ContinuationLease {
                value: Some(value),
                bank: bank.clone(),
            })
        })
    }
}

/// The sole ordinary graph lent through Requested and Done; rollback returns its actual owner.
pub struct ContinuationLease {
    value: Option<RetainedPayload<NativeExecutionInputs>>,
    bank: ChargedShared<Bank>,
}
impl ContinuationLease {
    pub(crate) fn get(&self) -> &NativeExecutionInputs {
        self.value
            .as_ref()
            .expect("live original continuation lease")
            .get()
    }

    /// Complete only fixed post-state fields. The same lease still returns the graph if any
    /// later output-seal finalizer refuses and the enclosing World is abandoned.
    #[allow(
        unsafe_code,
        reason = "only fixed scalar fields change; original graph/ledger move unchanged"
    )]
    pub(crate) fn complete(
        mut self,
        params: ChainParamsRecord,
        pulse: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    ) -> Self {
        let value = self.value.take().expect("live original continuation lease");
        // SAFETY: no nested field is cloned, replaced, resized or exported.
        self.value = Some(unsafe {
            value.map_payload(|mut value| {
                super::execution::set_params(&mut value.schedule.after_next, params);
                value.beacon = pulse;
                value
            })
        });
        self
    }

    /// Successful validation transfers this original graph to the existing finishing owner.
    /// That owner retains it across proof/encoding refusals and does not reexecute World.
    pub(crate) fn into_inputs(mut self) -> RetainedPayload<NativeExecutionInputs> {
        let bank = self.bank.clone();
        bank.pool.with_deferred_refund_notifications(|_| {
            let mut contents = bank.contents.lock();
            debug_assert!(matches!(contents.value, Value::Lent));
            contents.value = Value::Consumed;
        });
        self.value.take().expect("live original continuation lease")
    }
}
impl Drop for ContinuationLease {
    #[allow(
        unsafe_code,
        reason = "restore only fixed scalar fields without replacing funded graph allocations"
    )]
    fn drop(&mut self) {
        let Some(value) = self.value.take() else {
            return;
        };
        let bank = self.bank.clone();
        bank.pool.with_deferred_refund_notifications(|_| {
            let mut contents = bank.contents.lock();
            if !matches!(contents.value, Value::Lent) {
                // An impossible custody violation must not mint retry credit or replace another
                // live graph. Conservatively retain the original until process recovery.
                std::mem::forget(value);
                return;
            }
            // SPEC: §13.4 HC981 discards the actual original capture on rollback. The
            // surrounding original-pool scope still defers notifications past this guard.
            if cfg!(all(test, sumeragi_core_mutation = "HC981")) {
                contents.value = Value::Pending;
                drop(value);
                return;
            }
            let height = contents.original.header.height().get();
            let next = *contents
                .original
                .schedule
                .get(height + 1)
                .expect("validated original continuation next slot")
                .params();
            let after_next = contents.original.params;
            // SAFETY: the pristine next/after-next parameter records and empty beacon contain
            // no nested storage. Every funded epoch/key/proof allocation moves unchanged.
            let value = unsafe {
                value.map_payload(|mut value| {
                    super::execution::set_params(&mut value.schedule.next, next);
                    super::execution::set_params(&mut value.schedule.after_next, after_next);
                    value.beacon = None;
                    value
                })
            };
            contents.value = Value::Available(value);
        });
    }
}

#[cfg(test)]
mod tests;
