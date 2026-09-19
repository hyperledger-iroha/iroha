//! Authority checks and disposable execution for native decided inputs.
//!
//! The whole group batch is checked before execution mutates its overlay. Native
//! votes certify immutable inputs; this carrier alone supplies the economic base.
//! The after-start capability enters the common Network/Pipeline/Time producer.
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
    /// still consume the retained common outputs through global result/witness
    /// and execution commitment, then use the normal publication gate. No local lane Apply is
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
        // A caller owning a recorder is refused by the shared constructor before
        // State acquisition; suppression alone cannot prevent lock inversion.
        let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
        self.with_native_lane_execution(application_block_header, groups, |_, executions| {
            Ok(executions)
        })
    }

    /// Constructor-owned transition: preflight on exact applying pre-State,
    /// shared start effects once, then economics under H-effective policies.
    /// The private continuation is never minted from a post-hook overlay.
    /// Standalone scratch wrappers isolate their whole lifetime. Recording
    /// consumers use the same scoped kernel with an owned recorder continuation.
    pub(super) fn with_native_lane_execution<'state, R>(
        &'state self,
        header: super::BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
        finish: impl FnOnce(
            &mut StateBlock<'state>,
            Vec<PreexecutedLaneDecisionGroupV1>,
        ) -> Result<R, super::MergeLedgerCommitError>,
    ) -> Result<(Box<StateBlock<'state>>, R), super::MergeLedgerCommitError> {
        self.with_native_lane_execution_scope(
            header,
            groups,
            |_| Ok(()),
            finish,
            |_, result, ()| Ok(result),
        )
    }

    /// Acquire the execution scope only after the actual State writers and
    /// pristine source checks, and retain it through every start/output phase.
    /// The final continuation owns that same scope for metadata and capture.
    /// Failure drops it and the unpublished overlay without exposing either.
    pub(super) fn with_native_lane_execution_scope<'state, Scope, R, Finished>(
        &'state self,
        header: super::BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
        enter: impl FnOnce(&mut StateBlock<'state>) -> Result<Scope, super::MergeLedgerCommitError>,
        finish_native: impl FnOnce(
            &mut StateBlock<'state>,
            Vec<PreexecutedLaneDecisionGroupV1>,
        ) -> Result<R, super::MergeLedgerCommitError>,
        finish_scope: impl FnOnce(
            &mut StateBlock<'state>,
            R,
            Scope,
        ) -> Result<Finished, super::MergeLedgerCommitError>,
    ) -> Result<(Box<StateBlock<'state>>, Finished), super::MergeLedgerCommitError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(super::MergeLedgerCommitError::ExecutionRecorderConflict)?;
        // The constructor acquires a coherent predecessor and retains the
        // actual World, membership, hash and runtime writer guards throughout
        // this transition. Its policy projections are immutable snapshots.
        // A later diagnostic generation change (for example a manifest-cache
        // refresh) cannot replace any of those owned execution inputs.
        self.block_with_owned_start_stages(
            header,
            |overlay| {
                let invalid = super::MergeLedgerCommitError::ExecutionBatchInvalid;
                if overlay.start_of_block_effects_applied
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
                let scope = enter(overlay)?;
                Ok((
                    NativeLaneAfterStartV1 {
                        header: overlay._curr_block.clone(),
                        groups,
                    },
                    scope,
                ))
            },
            |overlay, (preflight, scope)| {
                let result = overlay.produce_native_execution_outputs(preflight, finish_native)?;
                finish_scope(overlay, result, scope)
            },
        )
    }
}

/// Only the constructor's pristine callback creates this value. Its consumer
/// executes on that constructor's same overlay after the shared hooks finish.
pub(super) struct NativeLaneAfterStartV1<'groups> {
    header: super::BlockHeader,
    groups: &'groups [VerifiedLaneDecisionGroupV1],
}

impl<'groups> NativeLaneAfterStartV1<'groups> {
    pub(super) fn into_source(
        self,
    ) -> (super::BlockHeader, &'groups [VerifiedLaneDecisionGroupV1]) {
        (self.header, self.groups)
    }
}

impl StateBlock<'_> {
    /// Classify an exact admitted input without mutating economics or charging fees.
    /// Structural/source contradictions reject the disposable batch; ordinary
    /// deterministic admission failure becomes that input's terminal result.
    pub(super) fn accept_native_group_entrypoint<'source>(
        &self,
        group: &'source VerifiedLaneDecisionGroupV1,
    ) -> Result<
        Result<
            crate::tx::AcceptedTransaction<'source>,
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
        Ok(
            crate::tx::AcceptedTransaction::accept_borrowed_entrypoint_at_time(
                entrypoint,
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
            }),
        )
    }
}
