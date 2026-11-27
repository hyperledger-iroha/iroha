//! Actual Network-derived events and bounded Pipeline invocation ownership.
//! Event positions and routing come from retained execution, never claimed outputs.

use super::internal::InternalInvocation;
use super::*;
use crate::smartcontracts::isi::triggers::set::{
    SetReadOnly, invocation_identity::pipeline_trigger_use_v1,
};
use crate::smartcontracts::isi::triggers::specialized::LoadedActionTrait as _;
use crate::state::c10_trigger_runtime::{CallbackEvent, PipelineSource};
use iroha_data_model::{
    block::execution_output::{PipelineEventPositionV1, PipelineInvocationV1},
    events::pipeline::{BlockEvent, BlockStatus, TransactionEvent, TransactionStatus},
    trigger::TriggerId,
};
use mv::storage::StorageReadOnly;

impl ExecutionOutputProducer<'_, '_, '_> {
    pub(super) fn execute_pipeline_outputs(&mut self) -> Result<(), ExecutionAttemptError<String>> {
        let result = (|| {
            if self.failed
                || self.pipeline_started
                || self.time_started
                || self.network_resolved.iter().any(|done| !done)
                || self.network_sources.is_none()
            {
                return Err("Pipeline phase requires one completed actual Network owner".into());
            }
            self.pipeline_started = true;
            self.skip_uninvoked(ExecutionOutputPhase::Pipeline, 0)?;
            let maximum = self.state.pipeline_trigger_candidate_limit()?;
            for index in 0..self.source.network_entrypoint_count() {
                let input = self
                    .source
                    .network_entrypoint_at(index)
                    .ok_or("Pipeline lost its exact Network source")?;
                let signed = match input {
                    TransactionEntrypoint::External(signed) => signed,
                    TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
                    TransactionEntrypoint::SealedCommitment(_)
                    | TransactionEntrypoint::AnonymousBallot(_) => {
                        self.skip_uninvoked(ExecutionOutputPhase::Pipeline, maximum)?;
                        continue;
                    }
                };
                use iroha_data_model::{
                    block::execution_output::TransactionResultRef,
                    ivm_c10::{
                        PipelineEventBoxC10V1, TransactionEventC10V1, TransactionStatusC10V1,
                    },
                };
                let input_index =
                    u32::try_from(index).map_err(|_| "Pipeline source index exceeds u32")?;
                let row = self
                    .rows
                    .get(index)
                    .ok_or("Pipeline event lost its Network result")?;
                if row.network_index() != Some(input_index) {
                    return Err("Pipeline event changed its Network position".into());
                }
                let route = self
                    .network_route(index)
                    .ok_or("Pipeline event lacks frozen routing")?;
                let position = PipelineEventPositionV1::Network(input_index);
                let hash = signed.hash();
                let block_height = Some(self.source.header().height());
                let lane_id = route.lane_id;
                let dataspace_id = route.dataspace_id;
                match row.result() {
                    TransactionResultRef::Baseline(result) => {
                        let status = match result.as_ref() {
                            Ok(_) => TransactionStatus::Approved,
                            Err(reason) => TransactionStatus::Rejected(Box::new(reason.clone())),
                        };
                        let event = TransactionEvent {
                            hash,
                            block_height,
                            lane_id,
                            dataspace_id,
                            status,
                        }
                        .into();
                        self.execute_pipeline_event(
                            position,
                            PipelineSource::Baseline(&event),
                            maximum,
                        )?;
                    }
                    TransactionResultRef::C10(result) => {
                        let status = match &result.0 {
                            Ok(_) => TransactionStatusC10V1::Approved,
                            Err(reason) => {
                                TransactionStatusC10V1::Rejected(Box::new(reason.clone()))
                            }
                        };
                        let event = PipelineEventBoxC10V1::Transaction(TransactionEventC10V1 {
                            hash,
                            block_height,
                            lane_id,
                            dataspace_id,
                            status,
                        });
                        self.execute_pipeline_event(
                            position,
                            PipelineSource::C10(&event),
                            maximum,
                        )?;
                    }
                }
            }
            self.execute_pipeline_event(
                PipelineEventPositionV1::BlockApproved,
                PipelineSource::Baseline(
                    &(BlockEvent {
                        header: self.source.header(),
                        status: BlockStatus::Approved,
                    }
                    .into()),
                ),
                maximum,
            )
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn execute_pipeline_event(
        &mut self,
        position: PipelineEventPositionV1,
        event: PipelineSource<'_>,
        maximum: u32,
    ) -> Result<(), ExecutionAttemptError<String>> {
        if self.state.gas_limit_per_block != 0
            && self.state.gas_used_in_block >= self.state.gas_limit_per_block
        {
            self.skip_uninvoked(ExecutionOutputPhase::Pipeline, maximum)?;
            return Ok(());
        }
        let height = self.source.header().height().get();
        let limit = usize::try_from(maximum).map_err(|_| "Pipeline capacity exceeds host width")?;
        let mut matched: Vec<TriggerId> = Vec::new();
        matched.try_reserve_exact(limit).map_err(|_| {
            ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            )
        })?;
        let eligible_baseline =
            |action: &crate::smartcontracts::isi::triggers::specialized::LoadedAction<
                iroha_data_model::events::pipeline::PipelineEventFilterBox,
            >| {
                let metadata = &action.metadata;
                crate::smartcontracts::isi::triggers::trigger_is_enabled(metadata)
                    && !action.repeats.is_depleted()
                    && crate::smartcontracts::isi::triggers::trigger_was_registered_before_block(
                        metadata, height,
                    )
                    && event.matches_baseline(&action.filter)
            };
        let baseline = self
            .state
            .world
            .triggers
            .pipeline_triggers()
            .iter()
            .filter(|(_, action)| eligible_baseline(action))
            .map(|(id, _)| id);
        let c10 = self
            .state
            .world
            .c10_trigger_candidates
            .iter()
            .filter(|(_, kind)| **kind == iroha_data_model::events::TriggeringEventType::Pipeline)
            .filter_map(|(id, _)| {
                self.state
                    .world
                    .c10_triggers
                    .get(id)
                    .filter(|row| event.matches_c10(row, height))
                    .map(|_| id)
            });
        for id in baseline.chain(c10) {
            if matched.len() == limit {
                return Err("Pipeline matcher exceeded frozen capacity".into());
            }
            matched.push(id.clone());
        }
        matched.sort_unstable();
        self.skip_uninvoked(
            ExecutionOutputPhase::Pipeline,
            maximum - u32::try_from(matched.len()).map_err(|_| "Pipeline count exceeds u32")?,
        )?;
        for (candidate_index, id) in matched.into_iter().enumerate() {
            if self.state.gas_limit_per_block != 0
                && self.state.gas_used_in_block >= self.state.gas_limit_per_block
            {
                self.skip_uninvoked(ExecutionOutputPhase::Pipeline, 1)?;
                continue;
            }
            let (authority, executable, c10_identity) =
                if let Some(row) = self.state.world.c10_triggers.get(&id) {
                    if !event.matches_c10(row, height) {
                        self.skip_uninvoked(ExecutionOutputPhase::Pipeline, 1)?;
                        continue;
                    }
                    (
                        row.trigger().action.authority().clone(),
                        crate::runtime::deferred_consent::trigger_executable_ref(
                            row.trigger().action.executable(),
                        )
                        .map_err(|e| e.to_string())?,
                        if self.lifecycle {
                            None
                        } else {
                            Some(crate::state::c10_trigger_runtime::identity(row, height)?)
                        },
                    )
                } else if let Some(action) = self
                    .state
                    .world
                    .triggers
                    .pipeline_triggers()
                    .get(&id)
                    .filter(|action| eligible_baseline(action))
                {
                    (action.authority.clone(), action.executable().clone(), None)
                } else {
                    self.skip_uninvoked(ExecutionOutputPhase::Pipeline, 1)?;
                    continue;
                };
            if !crate::runtime::deferred_consent::trigger_ready(&self.state.world, &id, height) {
                self.skip_uninvoked(ExecutionOutputPhase::Pipeline, 1)?;
                continue;
            }
            let source = if self.lifecycle {
                Some(lifecycle::internal_source(self.state, &id, height,
                    iroha_data_model::block::execution_output::TriggerLifecycleStateKindV1::Pipeline)?)
            } else {
                None
            };
            let invocation = PipelineInvocationV1 {
                event: position,
                candidate_index: u32::try_from(candidate_index)
                    .map_err(|_| "Pipeline index exceeds u32")?,
                trigger: match &source {
                    Some(source) => source.identity().clone(),
                    None => match c10_identity {
                        Some(identity) => identity,
                        None => pipeline_trigger_use_v1(&self.state.world.triggers, &id, height)?,
                    },
                },
            };
            self.try_apply_internal_family(
                InternalInvocation::Pipeline(invocation),
                CallbackEvent::Pipeline(event),
                &authority,
                &executable,
                source,
            )?;
        }
        Ok(())
    }
}
