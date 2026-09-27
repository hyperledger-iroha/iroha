//! Native decisions execute through the common rollback/output owner.
//!
//! The consumed constructor capability binds exact admitted sources before start
//! hooks. Each actual bounded Network row supplies membership and settlement,
//! then the same owner executes Pipeline and Time and retains the complete tail.
//! Full State/witness publication requires the canonical global finality owner.

use super::*;
use crate::state::{
    AppliedMergeLaneFrontierMarker, LaneExecutionSettlementInput, MergeLedgerCommitError, State,
    lane_decision_execution::{NativeLaneAfterStartV1, PreexecutedLaneDecisionGroupV1},
};
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::alloc::Layout;
use std::collections::BTreeSet;

type NativeResult<T> = Result<T, MergeLedgerCommitError>;

/// Requested Native Network scratch vectors, planned before any input executes.
#[derive(Clone, Copy)]
struct NativeNetworkScratchDemand {
    pending_executions: Layout,
    finished_executions: Layout,
    required: Layout,
    frontier: Layout,
    total_bytes: usize,
}

impl NativeNetworkScratchDemand {
    fn plan(groups: usize) -> Result<Self, AllocationRefusal> {
        let frontier_count = groups
            .checked_mul(iroha_data_model::block::lane_input::MAX_LANE_INPUT_ROUTE_SLOTS)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let pending_executions = Layout::array::<Option<PreexecutedLaneDecisionGroupV1>>(groups)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let finished_executions = Layout::array::<PreexecutedLaneDecisionGroupV1>(groups)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let required = Layout::array::<(HashOf<TransactionEntrypoint>, Hash)>(groups)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let frontier =
            Layout::array::<(iroha_model_base::state_path::StatePath, Vec<u8>)>(frontier_count)
                .map_err(|_| AllocationRefusal::DemandOverflow)?;
        let total_bytes = [pending_executions, finished_executions, required, frontier]
            .into_iter()
            .try_fold(0usize, |total, layout| total.checked_add(layout.size()))
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(Self {
            pending_executions,
            finished_executions,
            required,
            frontier,
            total_bytes,
        })
    }

    fn try_reserve(
        self,
        budget: &AllocationBudget,
    ) -> Result<NativeNetworkScratchCharges, AllocationRefusal> {
        let mut reservation = budget.try_reserve_bytes(self.total_bytes)?;
        let pending_executions = reservation
            .try_split(self.pending_executions)
            .expect("prepaid pending Native execution slots");
        let finished_executions = reservation
            .try_split(self.finished_executions)
            .expect("prepaid finished Native execution slots");
        let required = reservation
            .try_split(self.required)
            .expect("prepaid required obligation slots");
        let frontier = reservation
            .try_split(self.frontier)
            .expect("prepaid frontier marker slots");
        assert_eq!(reservation.remaining_bytes(), 0);
        Ok(NativeNetworkScratchCharges {
            _pending_executions: pending_executions,
            finished_executions: Some(finished_executions),
            _required: required,
            _frontier: frontier,
        })
    }
}

/// Retained until all temporary vectors are freed or custody takes the result.
struct NativeNetworkScratchCharges {
    _pending_executions: AllocationCharge,
    finished_executions: Option<AllocationCharge>,
    _required: AllocationCharge,
    _frontier: AllocationCharge,
}

/// A failed local child acquisition takes precedence over a later output verdict.
fn native_output_result<T>(state: &StateBlock<'_>, result: NativeResult<T>) -> NativeResult<T> {
    state.require_storage_admission()?;
    result
}

impl StateBlock<'_> {
    /// Consume preflight and exactly one reservation on the same constructor's
    /// overlay. The private callback seals native metadata before common tails.
    pub(in crate::state) fn produce_native_execution_outputs<R>(
        &mut self,
        preflight: NativeLaneAfterStartV1<'_>,
        admission: &mut crate::state::NativeExecutionResourceAdmission,
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
        let expiry =
            crate::smartcontracts::isi::sorafs::expire_pin_manifests_at_consensus_time(self)
                .map_err(|error| match error {
                    crate::smartcontracts::isi::sorafs::PinExpiryMaintenanceError::Storage(
                        error,
                    ) => MergeLedgerCommitError::StateStorageAdmission(error),
                    crate::smartcontracts::isi::sorafs::PinExpiryMaintenanceError::Instruction(
                        error,
                    ) => invalid(format!("Native SoraFS pin expiry failed: {error}")),
                });
        native_output_result(self, expiry)?;
        self.reserve_native_execution_outputs(groups)
            .map_err(invalid)?;
        let Some(ExecutionOutputPlanState::Reserved(plan)) = self.execution_output_plan.as_ref()
        else {
            return Err(invalid("Native output reservation disappeared".into()));
        };
        let maximum_rows = usize::try_from(plan.maximum_rows)
            .map_err(|_| invalid("Native output row count exceeds host width".into()))?;
        let network_inputs = usize::try_from(plan.network_inputs)
            .map_err(|_| invalid("Native Network count exceeds host width".into()))?;
        let demand = host_resources::NativeOutputProducerDemand::plan(maximum_rows, network_inputs)
            .map_err(MergeLedgerCommitError::NativeResourceAdmission)?;
        let host = demand
            .try_reserve(admission.budget())
            .map_err(MergeLedgerCommitError::NativeResourceAdmission)?;
        let mut producer =
            ExecutionOutputProducer::new(self, source, Some(host)).map_err(invalid)?;
        // Start-hook settlement belongs to the carrier, never the first native
        // input. An error poisons and drops this whole unpublished overlay.
        let start_settlement = std::mem::take(&mut producer.state.settlement_accumulator);
        let executions = producer.execute_native_network_sources(admission);
        let executions = native_output_result(producer.state, executions)?;
        if !producer.state.settlement_accumulator.is_empty() {
            return Err(MergeLedgerCommitError::ExecutionDivergence(
                "native execution retained unbound settlement receipts".into(),
            ));
        }
        producer.state.settlement_accumulator = start_settlement;
        let result = finish_native(producer.state, executions);
        let result = native_output_result(producer.state, result)?;
        let pipeline = producer.execute_pipeline_outputs().map_err(invalid);
        native_output_result(producer.state, pipeline)?;
        let time = producer.execute_scheduled_time_outputs().map_err(invalid);
        native_output_result(producer.state, time)?;
        producer.state.require_storage_admission()?;
        producer.finish().map_err(invalid)?;
        Ok(result)
    }
}

impl ExecutionOutputProducer<'_, '_, '_> {
    fn execute_native_network_sources(
        &mut self,
        admission: &mut crate::state::NativeExecutionResourceAdmission,
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
        let count = self.source.network_entrypoint_count();
        let mut scratch = NativeNetworkScratchDemand::plan(count)
            .and_then(|demand| demand.try_reserve(admission.budget()))
            .map_err(MergeLedgerCommitError::NativeResourceAdmission)?;
        admission
            .hold_executions_charge(
                scratch
                    .finished_executions
                    .take()
                    .expect("finished execution Vec charged once"),
            )
            .map_err(|_| {
                MergeLedgerCommitError::ExecutionRecorderConflict(
                    "Native execution Vec already has a retained charge".into(),
                )
            })?;
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
        let mut executions = Vec::new();
        executions.try_reserve_exact(groups.len()).map_err(|_| {
            MergeLedgerCommitError::ExecutionRecorderConflict(
                "host cannot allocate prepaid Native execution slots".into(),
            )
        })?;
        executions.resize_with(groups.len(), || None);
        let mut required = Vec::new();
        required.try_reserve_exact(groups.len()).map_err(|_| {
            MergeLedgerCommitError::ExecutionRecorderConflict(
                "host cannot allocate prepaid Native obligation slots".into(),
            )
        })?;
        let mut signed_terminal = BTreeSet::new();
        let mut frontier_markers = Vec::new();
        frontier_markers
            .try_reserve_exact(
                groups
                    .len()
                    .checked_mul(iroha_data_model::block::lane_input::MAX_LANE_INPUT_ROUTE_SLOTS)
                    .ok_or(MergeLedgerCommitError::NativeResourceAdmission(
                        AllocationRefusal::DemandOverflow,
                    ))?,
            )
            .map_err(|_| {
                MergeLedgerCommitError::ExecutionRecorderConflict(
                    "host cannot allocate prepaid Native frontier slots".into(),
                )
            })?;
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
            let alias = authenticated_signed_replay_alias
                .map(iroha_crypto::HashOf::from_untyped_unchecked)
                .filter(|alias| *alias != entrypoint.hash());
            self.state.stage_merge_carrier_entrypoints(
                [Some(entrypoint.hash()), alias].into_iter().flatten(),
            );
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
        let mut finished_executions = Vec::new();
        finished_executions
            .try_reserve_exact(groups.len())
            .map_err(|_| {
                MergeLedgerCommitError::ExecutionRecorderConflict(
                    "host cannot allocate prepaid finished Native executions".into(),
                )
            })?;
        for execution in executions {
            finished_executions.push(
                execution.ok_or_else(|| {
                    invalid("native execution permutation omitted a source".into())
                })?,
            );
        }
        let executions = finished_executions;
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

#[cfg(test)]
mod native_scratch_admission_tests {
    use super::*;

    #[test]
    fn native_execution_vectors_prepaid_before_the_first_source_runs() {
        let demand = NativeNetworkScratchDemand::plan(3).unwrap();
        let short = AllocationBudget::new(demand.total_bytes - 1);
        assert!(matches!(
            demand.try_reserve(&short),
            Err(AllocationRefusal::ExceedsLimit { .. })
        ));
        assert_eq!(short.reserved_bytes(), 0);

        let budget = AllocationBudget::new(demand.total_bytes);
        let mut charges = demand.try_reserve(&budget).unwrap();
        assert_eq!(budget.reserved_bytes(), demand.total_bytes);
        assert_eq!(charges._frontier.layout(), demand.frontier);
        let finished = charges.finished_executions.take().unwrap();
        drop(charges);
        assert_eq!(budget.reserved_bytes(), demand.finished_executions.size());
        drop(finished);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[cfg(test)]
mod local_refusal_tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{StateStorageAdmissionError, World},
    };
    use iroha_data_model::block::BlockHeader;
    use std::num::NonZeroU64;

    #[test]
    fn native_output_verdict_preserves_the_first_local_index_refusal() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let budget = state.world.operation_index_budget().clone();
        let mut block = state
            .try_block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0))
            .unwrap();
        let original_verdict =
            MergeLedgerCommitError::ExecutionBatchInvalid("later bounded output failed".into());
        assert!(matches!(
            native_output_result::<()>(&block, Err(original_verdict)),
            Err(MergeLedgerCommitError::ExecutionBatchInvalid(_))
        ));
        let mut child = block.try_transaction().unwrap();
        let occupied = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let (_, refusal) = child
            .world
            .kagemusha_mint_credit_operations
            .try_insert_admitted([17; 32], [18; 32])
            .unwrap_err();
        let expected = StateStorageAdmissionError::World(refusal);
        child.arm_local_storage_refusal(expected.clone());
        drop(child);
        let later_verdict =
            MergeLedgerCommitError::ExecutionBatchInvalid("later bounded output failed".into());
        assert!(matches!(
            native_output_result::<()>(&block, Err(later_verdict)),
            Err(MergeLedgerCommitError::StateStorageAdmission(error)) if error == expected
        ));
        drop(occupied);
    }
}
