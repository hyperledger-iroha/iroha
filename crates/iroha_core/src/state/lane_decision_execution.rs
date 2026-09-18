//! Authority checks and disposable execution for native decided inputs.
//!
//! The whole group batch is checked before execution mutates its overlay. Native
//! votes certify immutable inputs; this carrier alone supplies the economic base.
//! TODO: activate this executor with the replacement global consumer and retire
//! the old MergeLaneExecution source and independent frontier writers together.

use std::collections::BTreeSet;

use iroha_crypto::Hash;

use super::{State, StateBlock, VerifiedLaneDecisionGroupV1};

impl StateBlock<'_> {
    /// Check all immutable sources against this exact pre-execution overlay.
    ///
    /// No disk I/O, economic mutation, publication permission or reducer
    /// completion occurs here. The caller must immediately execute on this same
    /// overlay and retain the normal global candidate publication gate. Dropping
    /// the overlay leaves every admitted obligation and native Apply owner live.
    pub(crate) fn preflight_lane_decision_execution_inputs(
        &self,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<(), String> {
        let base_height = u64::try_from(self.block_hashes.len())
            .map_err(|_| "native execution base height overflows".to_owned())?;
        let base_hash = self.block_hashes.last().copied();
        if base_height.checked_add(1) != Some(self._curr_block.height().get())
            || base_hash.is_none()
            || self._curr_block.prev_block_hash() != base_hash
        {
            return Err("native execution carrier does not extend its exact State base".into());
        }
        if groups.len() > self.lane_consensus_contexts.get().contexts.len() {
            return Err("native execution batch exceeds its distinct open route count".into());
        }
        let mut previous_priority = None;
        let mut entrypoints = BTreeSet::new();
        let mut signed_identities = BTreeSet::new();
        let mut sealed_commitments = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for group in groups {
            let body = group.body();
            let payload = body.payload();
            let priority = payload.descriptor.admission_priority;
            if previous_priority.is_some_and(|previous| previous >= priority) {
                return Err(
                    "native execution groups are not in strict first-admission order".into(),
                );
            }
            previous_priority = Some(priority);
            let first_index = priority
                .carrier_height
                .checked_sub(1)
                .and_then(|height| usize::try_from(height).ok())
                .ok_or_else(|| {
                    "native execution source has an invalid first-carrier height".to_owned()
                })?;
            if priority.carrier_height > base_height
                || self.block_hashes.get(first_index).copied()
                    != Some(payload.descriptor.admission_carrier_hash)
                || body.source().source().finality().height_context.network_id != self.network_id
            {
                return Err("native execution source is outside its exact carrier history".into());
            }
            let entrypoint = &payload.input.entrypoint;
            if !entrypoints.insert(entrypoint.hash()) {
                return Err("native execution batch repeats an entrypoint".into());
            }
            if let Some(signed) = crate::tx::exact_signed_transaction_hash(entrypoint)
                && !signed_identities.insert(signed)
            {
                return Err("native execution batch repeats a signed transaction".into());
            }
            // Distinct reveals can enclose different signed identities while
            // competing for the same sealed commitment. The same commitment
            // cannot be created/revealed twice in one economic batch.
            let commitment = match entrypoint {
                iroha_data_model::transaction::signed::TransactionEntrypoint::SealedCommitment(
                    value,
                ) => Some(value.payload().commitment),
                iroha_data_model::transaction::signed::TransactionEntrypoint::SealedReveal(
                    value,
                ) => Some(value.commitment),
                _ => None,
            };
            if commitment.is_some_and(|hash| !sealed_commitments.insert(hash)) {
                return Err("native execution batch repeats a sealed commitment".into());
            }
            let binding = &payload.input.certificate.binding;
            let binding_hash = binding.canonical_hash();
            let required = self
                .required_queue_plan_pending_obligations_for_entrypoints(std::iter::once(
                    entrypoint.clone(),
                ))
                .map_err(|error| error.to_string())?;
            if required.as_slice() != [(entrypoint.hash(), binding_hash)] {
                return Err("native execution input has no exact pending admission owner".into());
            }
            if group.contexts().len() != payload.descriptor.slots.len() {
                return Err("native execution source lost a verified route context".into());
            }
            for (slot, verified) in payload.descriptor.slots.iter().zip(group.contexts()) {
                let frozen = verified.frozen();
                if !routes.insert(slot.route_key()) {
                    return Err("native execution groups contend for the same route slot".into());
                }
                if Hash::from(verified.instance_id().0) != slot.instance_id
                    || !self
                        .lane_consensus_contexts
                        .get()
                        .contexts
                        .iter()
                        .any(|current| current == frozen)
                    || self.lane_incarnations.get(&slot.route.lane_id)
                        != Some(&slot.lane_incarnation)
                {
                    return Err(
                        "native execution source no longer owns its exact frozen route".into(),
                    );
                }
                let actual = State::canonical_merged_lane_frontier_with_anchor_from_world(
                    &self.world,
                    slot.route.lane_id,
                    slot.route.dataspace_id,
                    slot.lane_incarnation,
                )
                .map_err(|error| error.to_string())?;
                if actual
                    != (
                        frozen.predecessor_height,
                        frozen.predecessor_hash,
                        frozen.predecessor_applied_global_height,
                    )
                {
                    return Err(
                        "native execution frontier advanced without settling its group".into(),
                    );
                }
                let head = State::queue_plan_pending_route_head_at_admission_cut(
                    self,
                    slot.route.lane_id,
                    slot.route.dataspace_id,
                    slot.lane_incarnation,
                    base_height,
                    base_height,
                )?
                .ok_or_else(|| "native execution route has no pending head".to_owned())?;
                if head.binding != *binding || head.priority != priority {
                    return Err(
                        "native execution input is not the exact oldest group on every route"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }
}

use super::WorldReadOnly as _;
/// Actual deterministic economic output for one independently decided input.
/// This private scratch result supplies no publication or application completion.
#[derive(Debug)]
pub(crate) struct PreexecutedLaneDecisionGroupV1 {
    /// Single original input and all native Commit decisions, without old vote translation.
    pub(crate) source: iroha_data_model::block::lane_input::LaneDecisionGroupV1,
    /// Actual result computed at the applying global carrier's base.
    pub(crate) result: super::TransactionResult,
    /// Signed replay alias authenticated by the real sealed-reveal execution.
    pub(crate) authenticated_signed_replay_alias: Option<Hash>,
    /// Actual fee and settlement evidence; native source authority is in `source`.
    pub(crate) settlement_commitment: super::LaneBlockCommitment,
    /// Canonical digest of the exact economic settlement.
    pub(crate) settlement_hash: iroha_crypto::HashOf<super::LaneBlockCommitment>,
    /// Snapshot of actual native outputs; map and captures remain owned by the same
    /// StateBlock until common inventory sealing and canonical result extraction.
    pub(crate) fastpq_transcripts: Vec<super::TransferTranscriptBundle>,
}

impl State {
    /// Execute a complete native group batch in a disposable global candidate overlay.
    ///
    /// Every rejection drops the whole owned overlay. On success the caller must
    /// still project the actual outputs through the global result/witness and
    /// execution commitment, then use the normal publication gate. No local lane Apply is
    /// acknowledged here, including for a successful economic result.
    /// TODO: replace the old merge source DTO and consumer with this actual
    /// transition before activating the process-lived lane instances.
    pub(crate) fn preexecute_lane_decision_groups<'state>(
        &'state self,
        application_block_header: super::BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<
        (Box<StateBlock<'state>>, Vec<PreexecutedLaneDecisionGroupV1>),
        super::MergeLedgerCommitError,
    > {
        // This standalone scratch owner must isolate the entire constructor:
        // start hooks run before native economics and may record real transfers.
        // Never acquire/reset a recorder here; the caller may already own one.
        let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
        self.with_native_lane_execution(application_block_header, groups, |_, executions| {
            Ok(executions)
        })
    }

    /// Constructor-owned transition: preflight on exact applying pre-State,
    /// shared start effects once, then economics under H-effective policies.
    /// The private continuation is never minted from a post-hook overlay.
    /// This kernel neither acquires/resets nor suppresses a witness recorder.
    /// Standalone scratch wrappers isolate their whole lifetime; a future sole
    /// canonical consumer must supply its own complete witness lifecycle.
    /// TODO: qualify that lifecycle and rollback before enabling native publication.
    pub(super) fn with_native_lane_execution<'state, R>(
        &'state self,
        header: super::BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
        finish: impl FnOnce(
            &mut StateBlock<'state>,
            Vec<PreexecutedLaneDecisionGroupV1>,
        ) -> Result<R, super::MergeLedgerCommitError>,
    ) -> Result<(Box<StateBlock<'state>>, R), super::MergeLedgerCommitError> {
        let generation = self.state_view_generation();
        self.block_with_owned_start_stages(
            header,
            |overlay| {
                let invalid = super::MergeLedgerCommitError::ExecutionBatchInvalid;
                if !super::is_stable_state_view_generation(generation, self.state_view_generation())
                    || overlay.start_of_block_effects_applied
                    || overlay.native_lane_stage.is_some()
                    || overlay.staged_merge_entry.is_some()
                    || !overlay.staged_queue_plan_admissions.is_empty()
                    || !overlay.world.merge_execution_write_set_bytes().is_empty()
                {
                    return Err(invalid(
                        "native pre-State constructor authority changed".into(),
                    ));
                }
                overlay
                    .preflight_lane_decision_execution_inputs(groups)
                    .map_err(invalid)?;
                Ok(NativeLaneAfterStartV1 { groups })
            },
            |overlay, preflight| {
                // A due hook owns real global effects, not native input receipts.
                // Preserve its accumulator for the eventual sole global consumer;
                // the native per-input drain must not attribute it to the first input.
                let start_settlement = std::mem::take(&mut overlay.settlement_accumulator);
                let executions =
                    overlay.execute_preflighted_lane_decision_groups(preflight.groups)?;
                if !overlay.settlement_accumulator.is_empty() {
                    return Err(super::MergeLedgerCommitError::ExecutionDivergence(
                        "native execution retained unbound settlement receipts".into(),
                    ));
                }
                overlay.settlement_accumulator = start_settlement;
                let result = finish(overlay, executions)?;
                if !super::is_stable_state_view_generation(generation, self.state_view_generation())
                {
                    return Err(super::MergeLedgerCommitError::ExecutionBatchInvalid(
                        "native applying publication changed during ordered execution".into(),
                    ));
                }
                Ok(result)
            },
        )
    }
}

/// Only the constructor's pristine callback creates this value. Its consumer
/// executes on that constructor's same overlay after the shared hooks finish.
struct NativeLaneAfterStartV1<'groups> {
    groups: &'groups [VerifiedLaneDecisionGroupV1],
}

impl StateBlock<'_> {
    // Called only by the owning noncommitting scope above; an error must drop it.
    fn execute_preflighted_lane_decision_groups(
        &mut self,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<Vec<PreexecutedLaneDecisionGroupV1>, super::MergeLedgerCommitError> {
        use super::{
            AppliedMergeLaneFrontierMarker, LaneExecutionSettlementInput, MergeLedgerCommitError,
            TransactionEntrypoint, TransactionResult,
        };
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        // Freeze canonical stateless classification before any group can mutate
        // governed limits or crypto policy. Exact QueuePlan admission time owns
        // TTL; actual carrier time/height still govern block placement and all
        // stateful effects. A certified but non-executable input is a terminal
        // deterministic rejection, never an endlessly rejected carrier head.
        let mut accepted = groups
            .iter()
            .map(|group| self.accept_native_group_entrypoint(group))
            .collect::<Result<Vec<_>, _>>()?;
        let mut reserved_gas = 0u64;
        for (index, classification) in accepted.iter_mut().enumerate() {
            let Ok(transaction) = classification else {
                continue;
            };
            let cost = match crate::queue::Queue::compute_proposal_gas_cost(transaction) {
                Ok(cost)
                    if crate::gas::gas_components_fit_block_limit(
                        self.gas_limit_per_block,
                        [cost],
                    ) =>
                {
                    cost
                }
                result => {
                    let reason = match result {
                        Ok(cost) => format!(
                            "native input gas reservation {cost} exceeds current whole-block limit {}",
                            self.gas_limit_per_block
                        ),
                        Err(error) => {
                            format!("native input has invalid proposal gas accounting: {error}")
                        }
                    };
                    *classification = Err(iroha_data_model::transaction::error::TransactionRejectionReason::LimitCheck(
                        iroha_data_model::transaction::error::TransactionLimitError { reason },
                    ));
                    continue;
                }
            };
            let Some(next) = reserved_gas.checked_add(cost).filter(|next| {
                crate::gas::gas_components_fit_block_limit(
                    self.gas_limit_per_block,
                    [self.gas_used_in_block, *next],
                )
            }) else {
                return Err(MergeLedgerCommitError::ExecutionBatchFull {
                    fitting_prefix: index,
                    gas_limit: self.gas_limit_per_block,
                    gas_used: self.gas_used_in_block,
                });
            };
            reserved_gas = next;
        }
        // Preserve canonical mixed-block sealed fairness: reorder only reveal
        // execution positions by immutable pre-block commitment order. Sources,
        // pending priority and result vector retain original admission positions.
        let mut execution_order = (0..groups.len()).collect::<Vec<_>>();
        let reveal_positions = execution_order
            .iter()
            .copied()
            .filter(|&index| {
                matches!(
                    groups[index].body().payload().input.entrypoint,
                    TransactionEntrypoint::SealedReveal(_)
                )
            })
            .collect::<Vec<_>>();
        let mut ordered_reveals = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                if let TransactionEntrypoint::SealedReveal(reveal) =
                    &group.body().payload().input.entrypoint
                {
                    Some((crate::tx::sealed_reveal_execution_key(self, reveal), index))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        ordered_reveals.sort_by_key(|(key, _)| *key);
        for (position, (_, source)) in reveal_positions.into_iter().zip(ordered_reveals) {
            execution_order[position] = source;
        }
        let mut accepted = accepted.into_iter().map(Some).collect::<Vec<_>>();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let mut executions = (0..groups.len()).map(|_| None).collect::<Vec<_>>();
        let mut required = Vec::with_capacity(groups.len());
        let mut signed_terminal = BTreeSet::new();
        let mut frontier_markers = Vec::new();
        let application_height = self._curr_block.height().get();
        for index in execution_order {
            let group = &groups[index];
            let accepted = accepted[index]
                .take()
                .ok_or_else(|| invalid("native execution permutation repeats a source".into()))?;
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
            let stateless_accepted = accepted.is_ok();
            // The helper authenticates against pre-block commitment state. Capture
            // it before execution/removal, and never broaden a bad-signature
            // rejection into authority over the enclosed signed replay identity.
            let authenticated_signed_replay_alias = stateless_accepted
                .then(|| {
                    crate::tx::authenticated_signed_replay_alias(self, entrypoint).map(Hash::from)
                })
                .flatten();
            let (actual_hash, result) = match accepted {
                Ok(accepted) => self
                    .validate_transaction_with_entrypoint_index_and_routing_context(
                        accepted,
                        &mut ivm_cache,
                        index,
                        route,
                    ),
                Err(rejection) => (entrypoint.hash(), Err(rejection)),
            };
            if actual_hash != entrypoint.hash() {
                return Err(MergeLedgerCommitError::ExecutionDivergence(
                    "native group executor returned another entrypoint identity".into(),
                ));
            }
            let mut result = TransactionResult::new(result);
            self.take_merge_lane_batch_transfer_outcomes(
                std::slice::from_ref(entrypoint),
                std::slice::from_mut(&mut result),
            )?;
            let fastpq_transcripts =
                self.retain_native_lane_fastpq_outputs(std::slice::from_ref(entrypoint))?;
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
            self.stage_merge_carrier_entrypoints(membership);
            let settlement_commitment =
                self.drain_lane_execution_settlement(LaneExecutionSettlementInput {
                    route,
                    lane_incarnation: slot.lane_incarnation,
                    lane_height: slot.lane_height,
                    entrypoints: std::slice::from_ref(entrypoint),
                    native_amx_receipts: &[None],
                    atomic_group: matches!(plan, crate::queue::RoutingPlan::NativeAmx(_)),
                })?;
            let settlement_hash = super::canonical_merge_settlement_hash(&settlement_commitment)?;
            required.push((
                entrypoint.hash(),
                input.certificate.binding.canonical_hash(),
            ));
            let descriptor_hash = payload.descriptor.canonical_hash().map_err(invalid)?;
            for (route_slot, context) in payload.descriptor.slots.iter().zip(group.contexts()) {
                let frozen = context.frozen();
                State::validate_lane_frontier_successor(
                    &self.world,
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
        self.resolve_required_queue_plan_pending_obligations(required, signed_terminal)?;
        self.stage_lane_execution_nexus_fee_settlement(executions.iter().map(|execution| {
            (
                &execution.settlement_commitment,
                execution.settlement_hash,
                application_height,
            )
        }))?;
        self.stage_merge_lane_frontier_markers(frontier_markers)?;
        Ok(executions)
    }
}

impl StateBlock<'_> {
    /// Classify an exact admitted input without mutating economics or charging fees.
    /// Structural/source contradictions reject the disposable batch; ordinary
    /// deterministic admission failure becomes that input's terminal result.
    fn accept_native_group_entrypoint(
        &self,
        group: &VerifiedLaneDecisionGroupV1,
    ) -> Result<
        Result<
            crate::tx::AcceptedTransaction<'static>,
            iroha_data_model::transaction::error::TransactionRejectionReason,
        >,
        super::MergeLedgerCommitError,
    > {
        use super::{MergeLedgerCommitError, TransactionEntrypoint};
        use iroha_data_model::transaction::error::TransactionRejectionReason;
        let input = &group.body().payload().input;
        let entrypoint = &input.entrypoint;
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let plan = input.routing_plan().map_err(invalid)?;
        // The owning scope is already a coherent parent snapshot; using it
        // avoids reacquiring State/MV views under a live StateBlock. Preflight
        // excludes same-carrier admission and checks the original ranked owner.
        let binding = State::pending_queue_plan_binding_for_execution(
            self,
            entrypoint,
            &plan,
            self._curr_block.height().get(),
        )
        .map_err(invalid)?
        .ok_or_else(|| invalid("native input lost exact parent admission".into()))?;
        if binding != input.certificate.binding {
            return Err(invalid(
                "native input differs from its exact parent admission".into(),
            ));
        }
        let reject = |message: String| {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
                message,
            ))
        };
        let signed = match entrypoint {
            TransactionEntrypoint::External(tx) => Some(tx),
            TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
            TransactionEntrypoint::SealedCommitment(_) => None,
            TransactionEntrypoint::Time(_) => {
                return Err(invalid(
                    "native input cannot be a global time entrypoint".into(),
                ));
            }
        };
        if signed.is_some_and(|tx| tx.creation_time() >= self._curr_block.creation_time()) {
            return Ok(Err(reject(
                "native input creation time must precede its actual carrier".into(),
            )));
        }
        if let TransactionEntrypoint::SealedCommitment(commitment) = entrypoint {
            if commitment.payload().reveal_after_height <= self._curr_block.height().get() {
                return Ok(Err(reject(
                    "sealed reveal_after_height must be greater than its actual commit height"
                        .into(),
                )));
            }
        }
        let parameters = self.world.parameters();
        Ok(crate::tx::AcceptedTransaction::accept_entrypoint_at_time(
            entrypoint.clone(),
            &self.network_id,
            parameters.sumeragi().max_clock_drift(),
            parameters.transaction(),
            self.crypto.as_ref(),
            std::time::Duration::from_millis(binding.enqueue_timestamp_ms),
        )
        .map_err(|error| match error {
            crate::tx::AcceptTransactionFail::TransactionLimit(limit) => {
                TransactionRejectionReason::LimitCheck(limit)
            }
            other => reject(format!("native input stateless acceptance failed: {other}")),
        }))
    }
}
