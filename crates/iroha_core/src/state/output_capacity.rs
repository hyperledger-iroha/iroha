//! Constructor-owned agreed output policy and one retained terminal reservation.
//!
//! A terminal plan is not complete source/wire/host admission. Its unfinished
//! owner blocks publication until the actual producer consumes every reservation.
//! TODO: compose source/metadata/trace/resident reservations and the sole typed
//! producer with final State/witness authorization before replacing that
//! publication gate. No native path is enabled.

use super::{StateBlock, StateTransaction, VerifiedLaneDecisionGroupV1};
use crate::smartcontracts::isi::triggers::set::SetReadOnly;
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    block::{
        BlockHeader, SignedBlock,
        output_budget::{ExecutionOutputBudget, ExecutionOutputTerminalCeilings},
    },
    events::EventFilterBox,
    isi::error::{InstructionExecutionError, InvalidParameterError},
    parameter::{BlockParameter, ExecutionOutputPolicyV1, Parameter},
    transaction::signed::TransactionEntrypoint,
    trigger::Trigger,
};
use mv::storage::StorageReadOnly;

/// Immutable logical capacity observed after this constructor's start effects.
pub(super) struct FrozenExecutionOutputCapacity {
    policy: ExecutionOutputPolicyV1,
    time_invocations: u32,
    // Include every stored Pipeline action, including disabled/depleted ones.
    // New/replaced incarnations are deferred by the registration-height guard;
    // public metadata writes cannot forge that reserved lifecycle field.
    pipeline_candidates: u32,
    terminals: ExecutionOutputTerminalCeilings,
}

/// Sole retained terminal plan for the authenticated input projection.
/// No public scalar constructor or second mutable budget is exposed.
pub(super) struct ReservedExecutionOutputPlan {
    proposal: HashOf<BlockHeader>,
    input_root: Option<Hash>,
    network_inputs: u32,
    native: bool,
    maximum_rows: u32,
    budget: ExecutionOutputBudget,
}

/// Immutable actual invocation source, constructed only by the execution producer.
pub(super) struct OwnedExecutionSource {
    call: Hash,
    lane: Option<iroha_model_base::topology::LaneId>,
    dataspace: iroha_model_base::topology::DataSpaceId,
}

impl OwnedExecutionSource {
    pub(super) fn call(&self) -> Hash {
        self.call
    }
    pub(super) fn lane(&self) -> Option<iroha_model_base::topology::LaneId> {
        self.lane
    }
    pub(super) fn dataspace(&self) -> iroha_model_base::topology::DataSpaceId {
        self.dataspace
    }
}

/// Complete source list from actual Network/Pipeline/Time execution, including
/// rejected and zero-transcript invocations. A transcript archive cannot build it.
pub(super) struct OwnedExecutionSources {
    native: bool,
    proposal: HashOf<BlockHeader>,
    source_context: iroha_data_model::fastpq::FastpqSourceStatementContextV1,
    entries: Vec<OwnedExecutionSource>,
    network_routes: Vec<crate::queue::RoutingDecision>,
}

impl OwnedExecutionSources {
    pub(super) fn is_native(&self) -> bool {
        self.native
    }
    pub(super) fn proposal(&self) -> HashOf<BlockHeader> {
        self.proposal
    }
    pub(super) fn source_context(
        &self,
    ) -> iroha_data_model::fastpq::FastpqSourceStatementContextV1 {
        self.source_context
    }
    pub(super) fn entries(&self) -> &[OwnedExecutionSource] {
        &self.entries
    }
    pub(super) fn network_routes(&self) -> &[crate::queue::RoutingDecision] {
        &self.network_routes
    }
}

/// Publication remains blocked throughout ownership transfer, including unwind.
/// Completed rows still require the common source/witness/wire sealing owner.
pub(super) enum ExecutionOutputPlanState {
    Reserved(ReservedExecutionOutputPlan),
    Running,
    Retained(producer::RetainedExecutionOutputs),
    Sealing,
    Sealed(producer::SealedExecutionOutputs),
    Poisoned,
}

#[path = "output_producer.rs"]
mod producer;
pub(crate) use producer::{ExecutionOutputSealError, ExecutionOutputSealMetadata};

impl StateBlock<'_> {
    /// Capture once; an invalid restored policy is retained as an explicit refusal.
    pub(super) fn capture_execution_output_capacity(&mut self) {
        // Constructor capture is write-once, including an invalid restored value.
        // Later State mutation cannot replace either its capacity or its refusal.
        if self.frozen_execution_output_capacity.is_some() {
            return;
        }
        self.frozen_execution_output_capacity = Some((|| {
            let parameters = self.world.parameters.get().block();
            let policy = parameters.execution_output();
            policy.validate()?;
            let time_invocations = parameters.max_time_trigger_invocations().get();
            policy.validate_time_invocations(time_invocations)?;
            let pipeline_candidates = u32::try_from(self.world.triggers.pipeline_triggers().len())
                .map_err(|_| "Pipeline registry count exceeds u32")?;
            let time_registered = u32::try_from(self.world.triggers.time_triggers().len())
                .map_err(|_| "Time registry count exceeds u32")?;
            if pipeline_candidates > policy.max_pipeline_triggers
                || time_registered > policy.max_time_triggers
            {
                return Err("trigger registry exceeds agreed execution capacity".into());
            }
            Ok(FrozenExecutionOutputCapacity {
                policy,
                time_invocations,
                pipeline_candidates,
                terminals: ExecutionOutputTerminalCeilings::derive()?,
            })
        })());
    }

    fn frozen_output_capacity(&self) -> Result<&FrozenExecutionOutputCapacity, String> {
        self.frozen_execution_output_capacity
            .as_ref()
            .ok_or_else(|| "execution requires captured carrier output capacity".to_owned())?
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Frozen row ceiling for transaction-owned callback capture.
    pub(super) fn callback_output_byte_limit(&self) -> Result<u64, String> {
        Ok(self.frozen_output_capacity()?.policy.max_output_bytes)
    }

    /// Frozen Pipeline candidate count per source event, captured before execution.
    pub(super) fn pipeline_trigger_candidate_limit(&self) -> Result<u32, String> {
        Ok(self.frozen_output_capacity()?.pipeline_candidates)
    }

    /// Return the independent applying Time count, never a late WSV value.
    pub(super) fn time_trigger_invocation_limit(&self) -> Result<usize, String> {
        usize::try_from(self.frozen_output_capacity()?.time_invocations)
            .map_err(|_| "Time invocation limit exceeds the host index width".to_owned())
    }

    /// Reserve all terminal rows from this actual ordinary source before execution.
    /// # Errors
    /// Rejects foreign/mixed proposals, repeated ownership or infeasible capacity.
    pub(crate) fn reserve_ordinary_execution_outputs(
        &mut self,
        block: &SignedBlock,
    ) -> Result<(), String> {
        if block.header() != self._curr_block
            || block
                .execution_context()
                .is_some_and(|context| context.native_lane_decisions.is_some())
        {
            return Err("ordinary output plan differs from its applying source".into());
        }
        block.validate_proposal_commitments()?;
        let count = u32::try_from(block.network_entrypoint_count())
            .map_err(|_| "Network input count exceeds u32")?;
        let root = MerkleTree::root_from_typed_leaves(
            block.network_entrypoints().map(TransactionEntrypoint::hash),
        )
        .map(Hash::from);
        self.reserve_execution_outputs(count, root, false)
    }

    /// Native route groups contribute one output each, regardless of route count.
    /// Call only from the exact after-start continuation with preflighted sources.
    /// # Errors
    /// Rejects repeated ownership, invalid input width or infeasible capacity.
    pub(super) fn reserve_native_execution_outputs(
        &mut self,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<(), String> {
        let count = u32::try_from(groups.len()).map_err(|_| "native input count exceeds u32")?;
        let root = MerkleTree::root_from_typed_leaves(
            groups
                .iter()
                .map(|group| group.body().payload().input.entrypoint.hash()),
        )
        .map(Hash::from);
        self.reserve_execution_outputs(count, root, true)
    }

    #[cfg(test)]
    pub(super) fn reserved_output_input_count_for_test(&self) -> Option<u32> {
        match self.execution_output_plan.as_ref()? {
            ExecutionOutputPlanState::Reserved(plan) => Some(plan.network_inputs),
            _ => None,
        }
    }

    fn reserve_execution_outputs(
        &mut self,
        count: u32,
        input_root: Option<Hash>,
        native: bool,
    ) -> Result<(), String> {
        if self.execution_output_plan.is_some() {
            return Err("carrier output reservations already have an owner".into());
        }
        let frozen = self.frozen_output_capacity()?;
        let limits = frozen.policy.limits();
        let phases = frozen
            .terminals
            .envelope(frozen.pipeline_candidates, frozen.time_invocations)
            .reservations(count, &limits)?;
        let budget = ExecutionOutputBudget::new(limits, phases)?;
        let maximum_rows = phases.iter().try_fold(0_u32, |total, phase| {
            total
                .checked_add(phase.count)
                .ok_or("reserved row count overflows u32")
        })?;
        self.execution_output_plan = Some(ExecutionOutputPlanState::Reserved(
            ReservedExecutionOutputPlan {
                proposal: self._curr_block.hash(),
                input_root,
                network_inputs: count,
                native,
                maximum_rows,
                budget,
            },
        ));
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

impl StateTransaction<'_, '_> {
    /// Validate the projected agreed parameter before publishing any configuration event.
    /// # Errors
    /// Rejects post-genesis capacity replacement, malformed profiles and above-cap Time counts.
    pub(crate) fn validate_execution_output_parameter(
        &self,
        parameter: &Parameter,
    ) -> Result<(), InstructionExecutionError> {
        let current = self.world.parameters.get().block();
        match parameter {
            Parameter::Block(BlockParameter::ExecutionOutput(next)) => {
                if self.block_height() != 1 {
                    return Err(invalid(
                        "execution output capacity is immutable after genesis",
                    ));
                }
                next.validate().map_err(invalid)?;
                next.validate_time_invocations(current.max_time_trigger_invocations().get())
                    .map_err(invalid)?;
                if u32::try_from(self.world.triggers.pipeline_triggers().len())
                    .map_err(|_| invalid("Pipeline registry exceeds protocol index width"))?
                    > next.max_pipeline_triggers
                    || u32::try_from(self.world.triggers.time_triggers().len())
                        .map_err(|_| invalid("Time registry exceeds protocol index width"))?
                        > next.max_time_triggers
                {
                    return Err(invalid(
                        "execution output capacity would omit existing trigger registrations",
                    ));
                }
            }
            Parameter::Block(BlockParameter::MaxTimeTriggerInvocations(next)) => {
                current.execution_output().validate().map_err(invalid)?;
                current
                    .execution_output()
                    .validate_time_invocations(next.get())
                    .map_err(invalid)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Bound total stored Pipeline/Time actions before program validation or loading.
    /// Disabled/depleted registrations still occupy capacity; duplicate handling stays with Register.
    /// # Errors
    /// Rejects an invalid committed policy or a registry already at its agreed cap.
    pub(crate) fn validate_execution_output_registration(
        &self,
        trigger: &Trigger,
    ) -> Result<(), InstructionExecutionError> {
        let policy = self.world.parameters.get().block().execution_output();
        policy.validate().map_err(invalid)?;
        if self.world.triggers.ids().get(trigger.id()).is_some() {
            return Ok(());
        }
        let full = match trigger.action().filter() {
            EventFilterBox::Pipeline(_) => {
                u32::try_from(self.world.triggers.pipeline_triggers().len())
                    .map_err(|_| invalid("Pipeline registry exceeds protocol index width"))?
                    >= policy.max_pipeline_triggers
            }
            EventFilterBox::Time(_) => {
                u32::try_from(self.world.triggers.time_triggers().len())
                    .map_err(|_| invalid("Time registry exceeds protocol index width"))?
                    >= policy.max_time_triggers
            }
            _ => false,
        };
        if full {
            return Err(invalid(
                "trigger registration exceeds agreed execution capacity",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "output_capacity_tests.rs"]
mod tests;
