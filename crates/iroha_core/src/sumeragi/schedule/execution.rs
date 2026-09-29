//! Original-prestate capture and atomic native epoch application inside the output seal.

use super::{ChainParamsRecord, NativeExecutionInputs, ScheduleError};
use crate::{
    state::{
        BlockHashRead, GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, StateBlock, StateReadOnly,
        WorldReadOnly,
    },
    sumeragi::{
        epoch_beacon,
        epoch_election::{self, FrozenEpochBoundary},
    },
};
use iroha_data_model::{
    block::SignedBlock,
    consensus::{FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconPulseContextV1},
};
use mv::{allocation::RetainedPayload, storage::StorageReadOnly};
use std::fmt;

/// A sealed original capture stays alive through every local post-execution refusal.
pub(crate) enum CapturedExecution {
    Boundary(FrozenEpochBoundary),
    Ordinary(RetainedPayload<NativeExecutionInputs>),
}
impl CapturedExecution {
    fn inputs(&self) -> &NativeExecutionInputs {
        match self {
            Self::Boundary(boundary) => boundary.execution_inputs(),
            Self::Ordinary(inputs) => inputs.get(),
        }
    }
    #[allow(
        unsafe_code,
        reason = "moves the unchanged retained canonical graph and assigns only fixed-size fields"
    )]
    fn finish(
        self,
        params: ChainParamsRecord,
        pulse: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    ) -> RetainedPayload<NativeExecutionInputs> {
        match self {
            Self::Boundary(boundary) => boundary.into_execution_inputs(params, pulse),
            Self::Ordinary(inputs) => {
                // SAFETY: every original nested allocation moves unchanged. Only fixed-size
                // post-state lag-two parameters and verified pulse fields are assigned.
                unsafe {
                    inputs.map_payload(|mut inputs| {
                        if inputs.schedule.height == 1 {
                            set_params(&mut inputs.schedule.next, params);
                        }
                        set_params(&mut inputs.schedule.after_next, params);
                        inputs.beacon = pulse;
                        inputs
                    })
                }
            }
        }
    }
}
fn set_params(slot: &mut super::ScheduledSlot, next: ChainParamsRecord) {
    match slot {
        super::ScheduledSlot::Ready(config) => config.params = next,
        super::ScheduledSlot::PendingBoundary { params, .. } => *params = next,
    }
}

/// One native block's sealed schedule lifecycle. The original owner is never recreated on retry.
#[derive(Default)]
pub(crate) enum ScheduleStep {
    /// This overlay has no native consensus execution request.
    #[default]
    Off,
    /// Captured from the exact pristine original State before block-start or transaction effects.
    Requested {
        /// Original complete context, guards and same-pool nested allocations.
        captured: CapturedExecution,
        /// Fixed-width transported proof authenticated against that same original cut.
        pulse: epoch_beacon::VerifiedEpochPulse,
    },
    /// Exact canonical inputs moved into the result after all finalizer checks succeeded.
    Done(RetainedPayload<NativeExecutionInputs>),
}
impl fmt::Debug for ScheduleStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => f.write_str("Off"),
            Self::Requested { captured, .. } => f
                .debug_tuple("Requested")
                .field(&captured.inputs().schedule.height)
                .finish(),
            Self::Done(inputs) => f
                .debug_tuple("Done")
                .field(&inputs.get().schedule.height)
                .finish(),
        }
    }
}
impl ScheduleStep {
    pub(crate) fn frozen_boundary(&self) -> Option<&FrozenEpochBoundary> {
        match self {
            Self::Requested {
                captured: CapturedExecution::Boundary(boundary),
                ..
            } => Some(boundary),
            _ => None,
        }
    }
}

/// Authenticate a successor's original native context at one committed State cut.
/// Reused before event attribution and by the pristine writer before schedule effects.
pub(crate) fn authenticate_successor_context(
    state: &(impl StateReadOnly + ?Sized),
    header: &iroha_data_model::block::BlockHeader,
    expected: &GlobalThresholdBeaconPulseContextV1,
) -> Result<(), ScheduleError> {
    let height = header.height().get();
    if height <= 1
        || u64::try_from(state.block_hashes().hash_count())
            .ok()
            .and_then(|cut| cut.checked_add(1))
            != Some(height)
        || header.prev_block_hash()
            != state
                .block_hashes()
                .hash_count()
                .checked_sub(1)
                .and_then(|index| state.block_hashes().hash_at(index))
                .copied()
    {
        return Err(ScheduleError::Epoch(
            "native successor differs from its original committed header/parent cut".into(),
        ));
    }
    // Canonical committed reads exclude differences in local QC signer subsets.
    let parent = crate::sumeragi::certified_chain::committed_block(state, height - 1)?;
    let genesis = crate::sumeragi::certified_chain::committed_block(state, 1)?;
    let instance =
        crate::sumeragi::node::global_instance(genesis.block(), &state.chain_id().to_string());
    let current = &state.world().consensus_schedule().ready(height)?.epoch;
    expected
        .validate()
        .map_err(|error| ScheduleError::Epoch(error.into()))?;
    if expected.instance != instance.0
        || expected.parent_consensus_hash != parent.core_hash().0
        || expected.parent_result != parent.result().0
        || expected.epoch != current.authorization.epoch
        || expected.epoch_context_id != current.context_id().map_err(ScheduleError::Epoch)?
    {
        return Err(ScheduleError::Epoch(
            "native control differs from the pristine committed instance/parent/result/epoch"
                .into(),
        ));
    }
    Ok(())
}

impl StateBlock<'_> {
    /// Capture the exact authenticated source in the pristine constructor callback, after
    /// source-generation/header validation and before any schedule or execution effect.
    /// Missing mandatory transported beacon work is refused; local aggregation is irrelevant.
    pub(crate) fn request_sumeragi_schedule(
        &mut self,
        genesis_height: u64,
        source: &SignedBlock,
        supplied_pulse: Option<FinalizedGlobalThresholdBeaconPulseV1>,
        expected_context: Option<GlobalThresholdBeaconPulseContextV1>,
    ) -> Result<(), ScheduleError> {
        if !matches!(self.sumeragi_schedule, ScheduleStep::Off) {
            return Err(ScheduleError::Epoch(
                "native schedule capture already exists".into(),
            ));
        }
        let height = self._curr_block.height().get();
        if genesis_height != 1
            || source.header() != self._curr_block
            || u64::try_from(self.block_hashes().hash_count())
                .ok()
                .and_then(|cut| cut.checked_add(1))
                != Some(height)
            || source.header().prev_block_hash()
                != self
                    .block_hashes()
                    .hash_count()
                    .checked_sub(1)
                    .and_then(|index| self.block_hashes().hash_at(index))
                    .copied()
        {
            return Err(ScheduleError::Epoch(
                "native schedule request differs from its pristine original header/parent cut"
                    .into(),
            ));
        }
        if height == genesis_height {
            if expected_context.is_some() || supplied_pulse.is_some() {
                return Err(ScheduleError::Epoch(
                    "signed genesis cannot carry native successor control".into(),
                ));
            }
        } else {
            let expected = expected_context.as_ref().ok_or_else(|| {
                ScheduleError::Epoch(
                    "native successor lacks its authenticated header context".into(),
                )
            })?;
            authenticate_successor_context(self, &self._curr_block, expected)?;
        }
        let budget = self.pipeline_ivm_prepared_cache.execution_budget();
        let params = ChainParamsRecord::from_parameters(self.world.parameters().sumeragi());
        let (captured, pulse) = if height == genesis_height {
            if !self.world.consensus_schedule().entries().is_empty() {
                return Err(ScheduleError::Epoch(
                    "genesis cannot replace retained native authority".into(),
                ));
            }
            let epoch =
                crate::sumeragi::epoch::genesis_epoch(source).map_err(ScheduleError::Epoch)?;
            let pulse = epoch_beacon::capture(
                &self.world,
                self.block_hashes(),
                &epoch,
                height,
                supplied_pulse,
                expected_context,
            )
            .map_err(ScheduleError::Epoch)?;
            let inputs =
                epoch_election::capture_continuation(&epoch, height, None, params, budget)?;
            (CapturedExecution::Ordinary(inputs), pulse)
        } else {
            let schedule = self.world.consensus_schedule();
            if schedule.tip().and_then(|tip| tip.checked_add(1)) != Some(height) {
                return Err(ScheduleError::Malformed);
            }
            let current = &schedule.ready(height)?.epoch;
            let pulse = epoch_beacon::capture(
                &self.world,
                self.block_hashes(),
                current,
                height,
                supplied_pulse,
                expected_context,
            )
            .map_err(ScheduleError::Epoch)?;
            let captured = if current.mode
                == iroha_data_model::parameter::system::ConsensusMode::Npos
                && height == current.authorization.last_height
            {
                let parameters = self.world.sumeragi_npos_parameters().ok_or_else(|| {
                    ScheduleError::Epoch("boundary lacks signed NPoS policy".into())
                })?;
                let policy =
                    iroha_data_model::nexus::ValidatorElectionPolicyV1::from_npos_parameters(
                        &parameters,
                    )
                    .map_err(ScheduleError::Epoch)?;
                let frozen = epoch_election::freeze_boundary(
                    &self.world,
                    self.block_hashes(),
                    current,
                    &policy,
                    height,
                    budget,
                )?
                .ok_or_else(|| {
                    ScheduleError::Epoch("required native boundary was not captured".into())
                })?;
                CapturedExecution::Boundary(frozen)
            } else {
                CapturedExecution::Ordinary(epoch_election::capture_continuation(
                    current,
                    height,
                    schedule.get(height.checked_add(1).ok_or(ScheduleError::HeightOverflow)?),
                    params,
                    budget,
                )?)
            };
            (captured, pulse)
        };
        // All source/proof/custody and complete retained allocation checks preceded any write.
        // This pulse is a mandatory original block-start effect, usable by Parliament in this
        // same transaction overlay; failure/drop discards its original journal with the block.
        if let (Some(value), Some(link)) = (pulse.pulse(), pulse.link()) {
            let slot = (
                iroha_data_model::governance::types::BeaconSessionId::for_network_v1(
                    &value.network_id,
                ),
                value.height,
            );
            self.world
                .global_beacon_pulses
                .insert(value.pulse_id, value);
            self.world
                .global_beacon_pulse_slots
                .insert(slot, value.pulse_id);
            self.world
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
        }
        self.sumeragi_schedule = ScheduleStep::Requested { captured, pulse };
        Ok(())
    }

    /// Finalize within the exact original output seal. Every fallible preparation precedes
    /// writes; a local refusal restores the same capture for retry instead of recomputation.
    pub(crate) fn advance_requested_sumeragi_schedule(&mut self) -> Result<(), ScheduleError> {
        let step = std::mem::take(&mut self.sumeragi_schedule);
        let ScheduleStep::Requested { captured, pulse } = step else {
            self.sumeragi_schedule = step;
            return Ok(());
        };
        let prepare = (|| {
            let source = &captured.inputs().schedule;
            let params = ChainParamsRecord::from_parameters(self.world.parameters().sumeragi());
            params.validate().map_err(ScheduleError::Params)?;
            let current_params = if source.height == 1 {
                validate_executed_genesis(&self.world, &source.current)?;
                params
            } else {
                self.world.consensus_schedule().ready(source.height)?.params
            };
            let graph = epoch_election::retain_outcome_schedule(
                source,
                current_params,
                params,
                self.pipeline_ivm_prepared_cache.execution_budget(),
            )?;
            if let CapturedExecution::Boundary(boundary) = &captured {
                self.finalize_validator_committee_boundary(boundary)
                    .map_err(ScheduleError::Epoch)?;
            }
            Ok::<_, ScheduleError>((graph, params))
        })();
        match prepare {
            Err(error) => {
                self.sumeragi_schedule = ScheduleStep::Requested { captured, pulse };
                Err(error)
            }
            Ok((graph, params)) => {
                *self.world.consensus_schedule.get_mut() = graph;
                self.sumeragi_schedule = ScheduleStep::Done(captured.finish(params, pulse.pulse()));
                Ok(())
            }
        }
    }

    /// Move the exact canonical fields and original allocation ledger into execution-result
    /// custody. The result builder retains this owner across validation, publication and replay.
    pub(crate) fn take_sumeragi_execution_inputs(
        &mut self,
    ) -> Result<RetainedPayload<NativeExecutionInputs>, ScheduleError> {
        let step = std::mem::take(&mut self.sumeragi_schedule);
        match step {
            ScheduleStep::Done(inputs) => Ok(inputs),
            other => {
                self.sumeragi_schedule = other;
                Err(ScheduleError::NotAdvanced)
            }
        }
    }
}

/// Reconcile signed genesis authority with the actual executed registration state. Later
/// epochs deliberately do not reread mutable validity when retaining original credentials.
pub(crate) fn validate_executed_genesis(
    world: &impl WorldReadOnly,
    context: &iroha_data_model::sumeragi::epoch::ValidatorEpochContextV1,
) -> Result<(), ScheduleError> {
    use iroha_data_model::{consensus::ConsensusKeyRole, parameter::system::ConsensusMode};
    context.validate().map_err(ScheduleError::Epoch)?;
    if context.authorization.epoch != 0 || context.authorization.first_height != 1 {
        return Err(ScheduleError::Epoch(
            "executed genesis has a non-genesis epoch".into(),
        ));
    }
    for member in &context.committee {
        if !world.peers().iter().any(|peer| peer == &member.validator)
            || !world.consensus_keys().iter().any(|(id, record)| {
                id == &record.id
                    && id.role == ConsensusKeyRole::Validator
                    && record.public_key == *member.validator.public_key()
                    && record.is_live_at(1, 0, 0)
                    && record.pop.as_deref() == Some(member.proof_of_possession.as_slice())
            })
        {
            return Err(ScheduleError::Epoch(
                "executed genesis omits an exact signed validator registration".into(),
            ));
        }
    }
    for peer in world.peers().iter() {
        if world.consensus_keys().iter().any(|(id, record)| {
            id == &record.id
                && id.role == ConsensusKeyRole::Validator
                && record.public_key == *peer.public_key()
                && record.is_live_at(1, 0, 0)
        }) && !context
            .committee
            .iter()
            .any(|member| &member.validator == peer)
        {
            return Err(ScheduleError::Epoch(
                "executed genesis introduces an unsigned voting registration".into(),
            ));
        }
    }
    if context.mode == ConsensusMode::Npos {
        let parameters = world.sumeragi_npos_parameters().ok_or_else(|| {
            ScheduleError::Epoch("executed NPoS genesis omits signed parameters".into())
        })?;
        if parameters.epoch_length_blocks.get() != context.authorization.last_height
            || parameters.epoch_seed != context.leader_seed
        {
            return Err(ScheduleError::Epoch(
                "executed genesis changes signed epoch geometry or seed".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
