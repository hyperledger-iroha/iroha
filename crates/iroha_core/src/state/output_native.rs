//! Native decisions execute through the common rollback/output owner.
//!
//! The consumed constructor capability binds exact admitted sources before start
//! hooks. Each actual bounded Network row supplies membership and settlement,
//! then the same owner executes Pipeline and Time and retains the complete tail.
//! Full State/witness publication remains a separate, still-closed boundary.

use super::*;
use crate::state::{
    AppliedMergeLaneFrontierMarker, LaneExecutionSettlementInput, MergeLedgerCommitError, State,
    lane_decision_execution::{NativeLaneAfterStartV1, PreexecutedLaneDecisionGroupV1},
};
use std::collections::BTreeSet;

type NativeResult<T> = Result<T, MergeLedgerCommitError>;

impl StateBlock<'_> {
    /// Consume preflight and exactly one reservation on the same constructor's
    /// overlay. The private callback seals native metadata before common tails.
    pub(in crate::state) fn produce_native_execution_outputs<R>(
        &mut self,
        preflight: NativeLaneAfterStartV1<'_>,
        finish_native: impl FnOnce(&mut Self, Vec<PreexecutedLaneDecisionGroupV1>) -> NativeResult<R>,
    ) -> NativeResult<R> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let source = ExecutionSource::native(preflight);
        let ExecutionSource::Native { groups, .. } = &source else {
            return Err(invalid("native producer lost its consumed source".into()));
        };
        if source.header() != self._curr_block || !self.start_of_block_effects_applied {
            return Err(invalid(
                "native producer differs from its exact after-start overlay".into(),
            ));
        }
        // Pin expiry is part of the actual carrier pre-execution maintenance,
        // after shared start hooks and before any Network/Pipeline/Time work.
        // Both scratch and recorded Native execution use this same transition.
        crate::smartcontracts::isi::sorafs::expire_pin_manifests_at_consensus_time(self)
            .map_err(|error| invalid(format!("Native SoraFS pin expiry failed: {error}")))?;
        self.reserve_native_execution_outputs(groups)
            .map_err(invalid)?;
        let mut producer = ExecutionOutputProducer::new(self, source).map_err(invalid)?;
        // Start-hook settlement belongs to the carrier, never the first native
        // input. An error poisons and drops this whole unpublished overlay.
        let start_settlement = std::mem::take(&mut producer.state.settlement_accumulator);
        let executions = producer.execute_native_network_sources()?;
        if !producer.state.settlement_accumulator.is_empty() {
            return Err(MergeLedgerCommitError::ExecutionDivergence(
                "native execution retained unbound settlement receipts".into(),
            ));
        }
        producer.state.settlement_accumulator = start_settlement;
        let result = finish_native(producer.state, executions)?;
        producer.execute_pipeline_outputs().map_err(invalid)?;
        producer.execute_scheduled_time_outputs().map_err(invalid)?;
        producer.finish().map_err(invalid)?;
        Ok(result)
    }
}

impl ExecutionOutputProducer<'_, '_, '_> {
    fn execute_native_network_sources(
        &mut self,
    ) -> NativeResult<Vec<PreexecutedLaneDecisionGroupV1>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        if self.failed
            || self.network_sources.is_some()
            || self.network_resolved.iter().any(|done| *done)
        {
            return Err(invalid(
                "native Network execution repeated or already started".into(),
            ));
        }
        self.network_sources = Some(self.freeze_native_network_sources()?);
        let execution_order = self
            .network_sources
            .as_mut()
            .and_then(|sources| sources.order.take())
            .ok_or_else(|| invalid("native Network order lost its owner".into()))?;
        let ExecutionSource::Native { groups, .. } = &self.source else {
            return Err(invalid(
                "native Network execution has an ordinary source".into(),
            ));
        };
        let groups = *groups;
        let mut ivm_cache =
            crate::smartcontracts::ivm::cache::IvmCache::with_prepared_contract_cache(
                self.state.pipeline.cache_size,
                self.state.pipeline_ivm_prepared_cache.clone(),
            );
        let mut executions = (0..groups.len()).map(|_| None).collect::<Vec<_>>();
        let mut required = Vec::with_capacity(groups.len());
        let mut signed_terminal = BTreeSet::new();
        let mut frontier_markers = Vec::new();
        let application_height = self.state._curr_block.height().get();
        for index in execution_order {
            let group = &groups[index];
            let payload = group.body().payload();
            let input = &payload.input;
            let entrypoint = &input.entrypoint;
            let plan = input.routing_plan().map_err(invalid)?;
            let route = plan.coordinator_route();
            let slot = payload
                .descriptor
                .slots
                .iter()
                .find(|slot| slot.route == route)
                .ok_or_else(|| invalid("native group lost its coordinator route".into()))?;
            let disposition = self
                .execute_network_source(index, &mut ivm_cache)
                .map_err(invalid)?;
            let stateless_accepted = disposition.stateless_accepted;
            let authenticated_signed_replay_alias = disposition.authenticated_signed_replay_alias;
            let ExecutionOutputV1::Network(output) = &self.rows[index] else {
                return Err(invalid(
                    "native source resolved to a non-Network row".into(),
                ));
            };
            // This diagnostic projection is copied from the actual bounded row;
            // it never creates or substitutes the producer's source/output owner.
            let result = output.result.clone();
            let fastpq_transcripts = self
                .state
                .retain_native_lane_fastpq_outputs(std::slice::from_ref(entrypoint))?;
            match entrypoint {
                TransactionEntrypoint::External(transaction) => {
                    if authenticated_signed_replay_alias.is_some() {
                        return Err(invalid(
                            "direct native input carries a sealed replay alias".into(),
                        ));
                    }
                    if stateless_accepted {
                        signed_terminal.insert(transaction.hash());
                    }
                }
                _ => {
                    if let Some(alias) = authenticated_signed_replay_alias {
                        let signed = crate::tx::exact_signed_transaction_hash(entrypoint)
                            .ok_or_else(|| {
                                invalid("native replay alias lacks a signed transaction".into())
                            })?;
                        if Hash::from(signed) != alias {
                            return Err(invalid(
                                "native replay alias differs from its exact signed identity".into(),
                            ));
                        }
                        signed_terminal.insert(signed);
                    }
                }
            }
            let mut membership = vec![entrypoint.hash()];
            if let Some(alias) = authenticated_signed_replay_alias {
                let alias = iroha_crypto::HashOf::from_untyped_unchecked(alias);
                if alias != entrypoint.hash() {
                    membership.push(alias);
                }
            }
            self.state.stage_merge_carrier_entrypoints(membership);
            let settlement_commitment =
                self.state
                    .drain_lane_execution_settlement(LaneExecutionSettlementInput {
                        route,
                        lane_incarnation: slot.lane_incarnation,
                        lane_height: slot.lane_height,
                        entrypoints: std::slice::from_ref(entrypoint),
                        native_amx_receipts: &[None],
                        atomic_group: matches!(plan, crate::queue::RoutingPlan::NativeAmx(_)),
                    })?;
            let settlement_hash =
                crate::state::canonical_merge_settlement_hash(&settlement_commitment)?;
            required.push((
                entrypoint.hash(),
                input.certificate.binding.canonical_hash(),
            ));
            let descriptor_hash = payload.descriptor.canonical_hash().map_err(invalid)?;
            for (route_slot, context) in payload.descriptor.slots.iter().zip(group.contexts()) {
                let frozen = context.frozen();
                State::validate_lane_frontier_successor(
                    &self.state.world,
                    (
                        route_slot.route.lane_id,
                        route_slot.route.dataspace_id,
                        route_slot.lane_incarnation,
                    ),
                    route_slot.lane_height,
                    frozen.predecessor_height,
                    frozen.predecessor_hash,
                )?;
                frontier_markers.push(State::encode_merge_lane_frontier_marker(
                    AppliedMergeLaneFrontierMarker {
                        version: 1,
                        lane_id: route_slot.route.lane_id,
                        dataspace_id: route_slot.route.dataspace_id,
                        lane_incarnation: route_slot.lane_incarnation,
                        lane_block_height: route_slot.lane_height,
                        lane_block_descriptor_hash: descriptor_hash,
                        applied_global_height: application_height,
                    },
                )?);
            }
            executions[index] = Some(PreexecutedLaneDecisionGroupV1 {
                source: group.to_wire(),
                result,
                authenticated_signed_replay_alias,
                settlement_commitment,
                settlement_hash,
                fastpq_transcripts,
            });
        }
        let executions = executions
            .into_iter()
            .map(|execution| {
                execution
                    .ok_or_else(|| invalid("native execution permutation omitted a source".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.state
            .resolve_required_queue_plan_pending_obligations(required, signed_terminal)?;
        self.state
            .stage_lane_execution_nexus_fee_settlement(executions.iter().map(|execution| {
                (
                    &execution.settlement_commitment,
                    execution.settlement_hash,
                    application_height,
                )
            }))?;
        self.state
            .stage_merge_lane_frontier_markers(frontier_markers)?;
        Ok(executions)
    }
}
