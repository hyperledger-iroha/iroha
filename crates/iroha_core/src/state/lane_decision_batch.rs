//! Execute authenticated native inputs under the actual proposal header.
//!
//! Proposal bytes contain only source Decisions and the exact applying pre-State.
//! Actual outputs and prefix roots have private ownership until the common global
//! result/witness projection; they never feed back into the executing block hash.
//! The canonical global owner consumes these sources through its full suffix,
//! witness and exact durable finality before State publication. Standalone
//! scratch execution retains no such publication authority.

use super::{
    MergeLedgerCommitError, State, StateBlock, TransactionEntrypoint, VerifiedLaneDecisionGroupV1,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, lane_decision_batch::LaneDecisionBatchV1};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use std::{collections::HashSet, sync::Arc};

type Result<T> = std::result::Result<T, MergeLedgerCommitError>;
type Execution = super::lane_decision_execution::PreexecutedLaneDecisionGroupV1;

/// Preserve stable source errors, but discard either result after a publication.
/// This finite outer fence also covers early errors from nested observations.
pub(super) fn with_stable_observation<T>(
    state: &State,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let generation = state.state_view_generation();
    if generation % 2 != 0 {
        return Err(MergeLedgerCommitError::ExecutionObservationChanged);
    }
    let result = operation();
    if !super::is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Err(MergeLedgerCommitError::ExecutionObservationChanged);
    }
    result
}

/// Private identity and actual prefix roots, minted only by ordered execution.
/// This seal cannot authenticate a full global witness or authorize State commit.
pub(super) struct NativeLaneStageSealV1 {
    carrier: BlockHeader,
    batch: Arc<LaneDecisionBatchV1>,
    batch_hash: Hash,
    authenticated_aliases: Vec<Option<Hash>>,
    settlement_hashes: Vec<HashOf<super::LaneBlockCommitment>>,
    membership: HashSet<HashOf<TransactionEntrypoint>>,
    queue_plan_admissions_hash: HashOf<Vec<Vec<u8>>>,
    npos_effects_hash: Option<HashOf<iroha_data_model::consensus::NposConsensusEffects>>,
    application_write_set_root: Hash,
    write_set_root: Hash,
    completed_write_set_root: Option<Hash>,
    settlements: Vec<super::LaneBlockCommitment>,
    fastpq: super::native_lane_fastpq::NativeLaneFastpqSeal,
}

/// One disposable start/native overlay, actual outputs and original verified sources.
/// The exact first-carrier/body/context owners survive execution without reconstruction.
/// Production has no mutable/consuming overlay publication accessor.
pub(crate) struct PreparedLaneDecisionBatchV1<'state> {
    overlay: Box<StateBlock<'state>>,
    batch: Arc<LaneDecisionBatchV1>,
    executions: Vec<Execution>,
    sources: Vec<VerifiedLaneDecisionGroupV1>,
}

/// Actual source-owned Native outputs and complete local execution witness.
/// Global proposal validation, resource admission and State publication remain
/// separate requirements; this owner has no mutable or committing escape.
pub(crate) struct RecordedNativeLaneBatchV1<'state> {
    prepared: PreparedLaneDecisionBatchV1<'state>,
    carrier: iroha_data_model::block::SignedBlock,
    context: crate::sumeragi::v2::VerifiedHeightContext,
}

impl<'state> RecordedNativeLaneBatchV1<'state> {
    /// Move the original execution and authenticated context into the canonical
    /// preparation owner. This grants no global validation or publication by itself.
    pub(crate) fn into_preparation_parts(
        self,
    ) -> std::result::Result<
        (
            iroha_data_model::block::SignedBlock,
            Box<StateBlock<'state>>,
            NativeExecutionCustody,
        ),
        String,
    > {
        self.prepared
            .verify_source_binding()
            .map_err(|error| error.to_string())?;
        // Common metadata may legitimately extend the native execution prefix.
        // The complete output/witness seal owns this later cut, while the same
        // immutable native stage retains its original source and settlements.
        self.prepared
            .overlay
            .verify_execution_output_seal(&self.carrier)?;
        self.prepared
            .overlay
            .validate_native_output_source(&self.carrier)?;
        let seal = Arc::clone(
            self.prepared
                .overlay
                .native_lane_stage
                .as_ref()
                .ok_or("Native preparation lost its actual stage")?,
        );
        let PreparedLaneDecisionBatchV1 {
            overlay,
            executions,
            sources,
            ..
        } = self.prepared;
        Ok((
            self.carrier,
            overlay,
            NativeExecutionCustody {
                seal,
                sources,
                executions,
                context: self.context,
            },
        ))
    }

    /// The result-bearing carrier produced by the retained execution owner.
    pub(crate) fn carrier(&self) -> &iroha_data_model::block::SignedBlock {
        &self.carrier
    }

    /// Qualification may inspect actual sources and the captured witness without
    /// reconstructing an overlay or granting publication authority.
    #[cfg(test)]
    pub(super) fn prepared_for_test(&self) -> &PreparedLaneDecisionBatchV1<'_> {
        &self.prepared
    }
}
/// Original Native source, execution and frozen context retained through the
/// common tail and detached journals. The stage is immutable after capture;
/// sharing its allocation with State preserves the same membership authority.
pub(crate) struct NativeExecutionCustody {
    seal: Arc<NativeLaneStageSealV1>,
    sources: Vec<VerifiedLaneDecisionGroupV1>,
    executions: Vec<Execution>,
    context: crate::sumeragi::v2::VerifiedHeightContext,
}
impl NativeExecutionCustody {
    /// Borrow the same privately authenticated sources retained by execution.
    /// These immutable observations do not grant source release or live signing.
    pub(in crate::state) fn sources(&self) -> &[VerifiedLaneDecisionGroupV1] {
        &self.sources
    }

    #[cfg(test)]
    pub(in crate::state) fn sources_for_test(&self) -> &[VerifiedLaneDecisionGroupV1] {
        &self.sources
    }

    /// Original authenticated context carried through execution and suffix opening.
    pub(crate) fn context(&self) -> &crate::sumeragi::v2::VerifiedHeightContext {
        &self.context
    }

    /// Rejoin immutable Native custody after State journals have been detached.
    pub(in crate::state) fn retains_carrier(
        &self,
        block: &iroha_data_model::block::SignedBlock,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
    ) -> bool {
        self.context.context() == context
            && self.seal.completed_write_set_root.is_some()
            && self.seal.carrier == block.header()
            && block.header().npos_effects_hash() == self.seal.npos_effects_hash
            && block
                .execution_context()
                .map(|bundle| HashOf::new(&bundle.queue_plan_admissions))
                == Some(self.seal.queue_plan_admissions_hash)
            && block.external_entrypoints_slice().is_empty()
            && block
                .execution_context()
                .and_then(|bundle| bundle.native_lane_decisions.as_deref())
                == Some(self.seal.batch.as_ref())
            && self.sources.len() == self.seal.batch.groups.len()
            && self.executions.len() == self.sources.len()
            && self
                .sources
                .iter()
                .zip(&self.seal.batch.groups)
                .all(|(source, wire)| {
                    source.body().payload() == &wire.payload && source.decisions() == wire.decisions
                })
    }

    pub(in crate::state) fn retains_state(&self, state: &StateBlock<'_>) -> bool {
        state
            .native_lane_stage
            .as_ref()
            .is_some_and(|seal| Arc::ptr_eq(seal, &self.seal))
            && self.context.context().height == state._curr_block.height().get()
            && self.context.context().network_id == state.network_id
            && self.sources.len() == self.seal.batch.groups.len()
            && self.executions.len() == self.sources.len()
            && self
                .sources
                .iter()
                .zip(&self.seal.batch.groups)
                .all(|(source, wire)| {
                    source.body().payload() == &wire.payload && source.decisions() == wire.decisions
                })
            && state.validate_native_lane_stage_membership().is_ok()
    }
}

impl<'state> PreparedLaneDecisionBatchV1<'state> {
    pub(super) fn from_stage(
        overlay: Box<StateBlock<'state>>,
        executions: Vec<Execution>,
        sources: Vec<VerifiedLaneDecisionGroupV1>,
    ) -> Result<Self> {
        overlay.validate_native_lane_stage_membership()?;
        let batch = Arc::clone(
            &overlay
                .native_lane_stage
                .as_ref()
                .ok_or_else(|| {
                    MergeLedgerCommitError::ExecutionBatchInvalid(
                        "native stage has no source seal".into(),
                    )
                })?
                .batch,
        );
        let prepared = Self {
            overlay,
            batch,
            executions,
            sources,
        };
        prepared.verify_source_binding()?;
        Ok(prepared)
    }
    fn verify_source_binding(&self) -> Result<()> {
        if self.sources.len() != self.batch.groups.len()
            || self
                .sources
                .iter()
                .zip(&self.batch.groups)
                .any(|(source, wire)| {
                    source.body().payload() != &wire.payload || source.decisions() != wire.decisions
                })
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native stage lost its original verified source groups".into(),
            ));
        }
        Ok(())
    }
    /// Exact input-only proposal source, independent of the outputs below.
    pub(crate) fn batch(&self) -> &LaneDecisionBatchV1 {
        &self.batch
    }
    /// Actual outputs for the eventual sole standard result projection.
    pub(crate) fn executions(&self) -> &[Execution] {
        &self.executions
    }
    /// Borrow the original all-route source evidence retained by this actual stage.
    /// These checked observations grant neither current signing nor State publication.
    #[cfg(test)]
    pub(super) fn sources_for_test(&self) -> &[VerifiedLaneDecisionGroupV1] {
        &self.sources
    }
    /// Adversarial qualification only; production has no mutable overlay escape.
    #[cfg(test)]
    pub(super) fn overlay_mut_for_test(&mut self) -> &mut StateBlock<'state> {
        &mut self.overlay
    }
    /// Exercise the rejecting commit boundary without a production escape.
    #[cfg(test)]
    pub(super) fn into_overlay_for_test(self) -> Box<StateBlock<'state>> {
        self.overlay
    }
    /// Read-only qualification access, without publication authority.
    #[cfg(test)]
    pub(super) fn overlay(&self) -> &StateBlock<'_> {
        &self.overlay
    }
    /// Actual prefix roots retained only in the private stage seal.
    #[cfg(test)]
    pub(super) fn prefix_roots_for_test(&self) -> (Hash, Hash) {
        let seal = self
            .overlay
            .native_lane_stage
            .as_ref()
            .expect("constructed seal");
        (seal.application_write_set_root, seal.write_set_root)
    }
}

/// Replay markers bind immutable inputs to the actual executing block identity.
/// No output or marker-inclusive root is hashed into proposal inputs.
pub(super) fn native_application_identity(carrier: &BlockHeader, batch_hash: Hash) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha:lane-consensus:application:v1\0",
        carrier.hash().as_ref(),
        batch_hash.as_ref(),
    ])
}

impl State {
    /// Record one authenticated Native source on its exact applying pre-State.
    /// This consumes the actual source groups and executes each phase once under
    /// one recorder. It does not construct a ValidBlock or authorize publication.
    pub(super) fn record_native_lane_decision_batch(
        &self,
        mut carrier: iroha_data_model::block::SignedBlock,
        groups: Vec<VerifiedLaneDecisionGroupV1>,
        context: crate::sumeragi::v2::VerifiedHeightContext,
    ) -> Result<RecordedNativeLaneBatchV1<'_>> {
        crate::sumeragi::witness::ensure_exec_witness_capture_available()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        with_stable_observation(self, || {
            let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
            if !carrier.is_resultless_proposal() {
                return Err(invalid(
                    "recorded Native execution requires a resultless proposal".into(),
                ));
            }
            let expected =
                crate::block::native_lane_batch_for_execution(&carrier).map_err(invalid)?;
            let controls = crate::block::ValidBlock::prepare_native_execution_controls(
                &carrier, self, context,
            )
            .map_err(|error| invalid(error.to_string()))?;
            let batch = self.prepare_lane_decision_batch(&groups)?;
            if &batch != expected {
                return Err(invalid(
                    "recorded Native execution lost its exact source or pre-State".into(),
                ));
            }
            let (overlay, (executions, context)) = self.with_native_lane_execution_scope(
                carrier.header(),
                &groups,
                |overlay| {
                    let recorder = crate::sumeragi::witness::begin_exec_witness_capture()
                        .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
                    let context = controls
                        .apply(overlay)
                        .map_err(|error| invalid(error.to_string()))?;
                    Ok((recorder, context))
                },
                |overlay, results| overlay.seal_native_lane_decision_batch(results, batch),
                |overlay, executions, (recorder, context)| {
                    crate::block::ValidBlock::seal_native_execution_outputs(
                        &mut carrier,
                        overlay,
                        &executions,
                    )
                    .map_err(|error| invalid(error.to_string()))?;
                    crate::block::ValidBlock::finalize_native_execution_contexts(
                        &carrier, overlay, &context,
                    )
                    .map_err(|error| invalid(error.to_string()))?;
                    overlay.capture_exec_witness().map_err(invalid)?;
                    overlay
                        .verify_execution_output_seal(&carrier)
                        .map_err(invalid)?;
                    drop(recorder);
                    Ok((executions, context))
                },
            )?;
            let prepared = PreparedLaneDecisionBatchV1::from_stage(overlay, executions, groups)?;
            Ok(RecordedNativeLaneBatchV1 {
                prepared,
                carrier,
                context,
            })
        })
    }

    /// Construct bounded input-only proposal data without running any instruction.
    /// Source authority still requires exact first-carrier and Decision validation
    /// in the consumer; this portable value grants no execution/publication token.
    pub(crate) fn prepare_lane_decision_batch(
        &self,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<LaneDecisionBatchV1> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        with_stable_observation(self, || {
            let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
            let captured = crate::snapshot::CapturedStateSnapshot::capture(self)?;
            let batch = LaneDecisionBatchV1 {
                base_state_height: u64::try_from(captured.height())
                    .map_err(|_| invalid("native execution base height overflows".into()))?,
                base_state_hash: HashOf::from_untyped_unchecked(captured.canonical_hash()?),
                groups: groups
                    .iter()
                    .map(VerifiedLaneDecisionGroupV1::to_wire)
                    .collect(),
            };
            batch.canonical_hash().map_err(invalid)?;
            Ok(batch)
        })
    }

    /// Execute verified sources under their actual carrier, after shared start hooks.
    /// Full economic outputs are authenticated by global execution, never proposal claims.
    /// Move the authenticated groups into the result; no cloned wire projection replaces them.
    pub(crate) fn replay_lane_decision_batch(
        &self,
        carrier: &BlockHeader,
        batch: &LaneDecisionBatchV1,
        groups: Vec<VerifiedLaneDecisionGroupV1>,
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        with_stable_observation(self, || {
            let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
            batch.canonical_hash().map_err(invalid)?;
            if self.prepare_lane_decision_batch(&groups)? != *batch {
                return Err(invalid(
                    "native source differs from its exact verified inputs or applying pre-State"
                        .into(),
                ));
            }
            let prepared = self.prepare_native_batch_on_carrier(carrier.clone(), groups)?;
            if prepared.batch() != batch {
                return Err(invalid(
                    "native applying pre-State changed before execution".into(),
                ));
            }
            Ok(prepared)
        })
    }

    /// Standalone actual-header scratch constructor; callers authenticate the
    /// containing proposal/source before invoking this private stage transition.
    /// Scratch isolation covers start hooks, native economics and private markers,
    /// and rejects recorder-owning callers before any State read or acquisition.
    pub(super) fn prepare_native_batch_on_carrier(
        &self,
        header: BlockHeader,
        groups: Vec<VerifiedLaneDecisionGroupV1>,
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        with_stable_observation(self, || {
            let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
            let batch = self.prepare_lane_decision_batch(&groups)?;
            let (overlay, executions) =
                self.with_native_lane_execution(header, &groups, |overlay, results| {
                    overlay.seal_native_lane_decision_batch(results, batch)
                })?;
            PreparedLaneDecisionBatchV1::from_stage(overlay, executions, groups)
        })
    }
}

impl StateBlock<'_> {
    /// Bind the retained source stage across witness, finality and publication.
    /// This identity preserves the original native prefix cuts without confusing
    /// them with the complete global metadata/publication State cut.
    pub(super) fn native_output_publication_identity(
        &self,
    ) -> std::result::Result<Option<Hash>, String> {
        let Some(seal) = self.native_lane_stage.as_ref() else {
            return Ok(None);
        };
        self.validate_native_lane_stage_membership()
            .map_err(|error| error.to_string())?;
        let mut membership = seal.membership.iter().copied().collect::<Vec<_>>();
        membership.sort();
        let aliases = Hash::new(
            &norito::encode_canonical(&seal.authenticated_aliases)
                .map_err(|error| error.to_string())?,
        );
        let settlements = Hash::new(
            &norito::encode_canonical(&seal.settlements).map_err(|error| error.to_string())?,
        );
        let bytes = norito::encode_canonical(&(
            seal.carrier,
            seal.batch_hash,
            aliases,
            membership,
            seal.application_write_set_root,
            seal.write_set_root,
            seal.completed_write_set_root,
            settlements,
        ))
        .map_err(|error| error.to_string())?;
        Ok(Some(Hash::new_from_chunks(&[
            b"iroha:native-output-publication-source:v1\0",
            &bytes,
        ])))
    }

    /// Check control custody without rereading State while holding its writers.
    /// Only the original unchanged State and pristine constructor cut may consume
    /// these controls; a same-header overlay from another State is insufficient.
    pub(crate) fn validate_native_pristine_control_owner(
        &self,
        state: &State,
        generation: u64,
        header: &BlockHeader,
    ) -> std::result::Result<(), String> {
        if !std::ptr::eq(self.state_ref, state)
            || !super::is_stable_state_view_generation(generation, state.state_view_generation())
            || &self._curr_block != header
            || self.start_of_block_effects_applied
            || self.applied_npos_consensus_effects_hash.is_some()
            || !self.staged_queue_plan_admissions.is_empty()
            || self.staged_merge_entry.is_some()
            || self.native_lane_stage.is_some()
            || !self.world.merge_execution_write_set_bytes().is_empty()
        {
            return Err("Native controls lost their original pristine State owner".into());
        }
        Ok(())
    }

    /// Seal actual results supplied only by the constructor-owned after-start kernel.
    fn seal_native_lane_decision_batch(
        &mut self,
        results: Vec<Execution>,
        batch: LaneDecisionBatchV1,
    ) -> Result<Vec<Execution>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        if !self.start_of_block_effects_applied
            || self.native_lane_stage.is_some()
            || self.staged_merge_entry.is_some()
            || self.canonical_wsv_merge_commit_authorization.is_some()
            || self
                .canonical_carrier_commit_metadata_authorization
                .is_some()
            || results.len() != batch.groups.len()
            || results
                .iter()
                .zip(&batch.groups)
                .any(|(result, source)| result.source != *source)
        {
            return Err(invalid(
                "native prefix lost its constructor-owned stage or exact source".into(),
            ));
        }
        let batch_hash = batch.canonical_hash().map_err(invalid)?;
        let authenticated_aliases = results
            .iter()
            .map(|result| result.authenticated_signed_replay_alias)
            .collect::<Vec<_>>();
        let settlement_hashes = results
            .iter()
            .map(|result| result.settlement_hash)
            .collect();
        let membership = Self::native_batch_membership(&batch, &authenticated_aliases)?;
        if membership != self.merge_carrier_entrypoints {
            return Err(invalid(
                "native execution membership differs from executed source and aliases".into(),
            ));
        }
        let fastpq = self.seal_native_lane_fastpq_outputs(&results)?;
        let application_write_set_root = self.merge_execution_write_set_root();
        self.stage_lane_decision_application_markers(
            &batch,
            native_application_identity(&self._curr_block, batch_hash),
        )?;
        let write_set_root = self.merge_execution_write_set_root();
        self.native_lane_stage = Some(Arc::new(NativeLaneStageSealV1 {
            carrier: self._curr_block.clone(),
            batch: Arc::new(batch),
            batch_hash,
            authenticated_aliases,
            settlement_hashes,
            membership,
            queue_plan_admissions_hash: HashOf::new(&self.staged_queue_plan_admissions),
            npos_effects_hash: self.applied_npos_consensus_effects_hash,
            application_write_set_root,
            write_set_root,
            completed_write_set_root: None,
            settlements: results
                .iter()
                .map(|result| result.settlement_commitment.clone())
                .collect(),
            fastpq,
        }));
        self.validate_native_lane_execution()?;
        Ok(results)
    }

    fn native_batch_membership(
        batch: &LaneDecisionBatchV1,
        aliases: &[Option<Hash>],
    ) -> Result<HashSet<HashOf<TransactionEntrypoint>>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        if aliases.len() != batch.groups.len() {
            return Err(invalid(
                "native source aliases lost their exact positions".into(),
            ));
        }
        let mut membership = HashSet::new();
        for (group, alias) in batch.groups.iter().zip(aliases) {
            let input = &group.payload.input.entrypoint;
            membership.insert(input.hash());
            if let Some(alias) = alias {
                match input {
                    TransactionEntrypoint::SealedReveal(reveal)
                        if Hash::from(reveal.signed_transaction().hash()) == *alias => {}
                    _ => {
                        return Err(invalid(
                            "native stage alias differs from its authenticated sealed input".into(),
                        ));
                    }
                }
                membership.insert(HashOf::from_untyped_unchecked(*alias));
            }
        }
        Ok(membership)
    }

    /// Check immutable source/membership without treating prefix roots as the
    /// whole-block witness. Full publication authorization remains mandatory.
    pub(super) fn validate_native_lane_stage_membership(&self) -> Result<()> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let seal = self
            .native_lane_stage
            .as_ref()
            .ok_or_else(|| invalid("native stage has no source seal".into()))?;
        if self.staged_merge_entry.is_some()
            || HashOf::new(&self.staged_queue_plan_admissions) != seal.queue_plan_admissions_hash
            || self.applied_npos_consensus_effects_hash != seal.npos_effects_hash
            || self.canonical_wsv_merge_commit_authorization.is_some()
            || self
                .canonical_carrier_commit_metadata_authorization
                .is_some()
            || self._curr_block != seal.carrier
            || self.applied_npos_consensus_effects_hash != seal.carrier.npos_effects_hash()
            || seal.batch.canonical_hash().map_err(invalid)? != seal.batch_hash
            || Self::native_batch_membership(&seal.batch, &seal.authenticated_aliases)?
                != seal.membership
            || self.merge_carrier_entrypoints != seal.membership
        {
            return Err(invalid(
                "native stage source, carrier or exact membership changed".into(),
            ));
        }
        Ok(())
    }

    /// Borrow the same constructor-owned sources and actual FASTPQ prefix seal.
    /// This is local execution custody, never global publication authority.
    pub(super) fn native_lane_stage_for_inventory(
        &self,
    ) -> Result<
        Option<(
            &LaneDecisionBatchV1,
            &super::native_lane_fastpq::NativeLaneFastpqSeal,
        )>,
    > {
        if self.native_lane_stage.is_none() {
            return Ok(None);
        }
        self.validate_native_lane_stage_membership()?;
        Ok(self
            .native_lane_stage
            .as_ref()
            .map(|seal| (seal.batch.as_ref(), &seal.fastpq)))
    }

    /// Check the owned native execution cut: the prefix before internal phases,
    /// or the complete actual tail once the common producer has finished.
    pub(super) fn validate_native_lane_execution(&self) -> Result<()> {
        self.validate_native_lane_stage_membership()?;
        if !self.start_of_block_effects_applied
            || self.merge_execution_write_set_root()
                != self
                    .native_lane_stage
                    .as_ref()
                    .expect("checked native seal")
                    .completed_write_set_root
                    .unwrap_or_else(|| {
                        self.native_lane_stage
                            .as_ref()
                            .expect("checked native seal")
                            .write_set_root
                    })
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native start+execution prefix writes changed or hooks did not run".into(),
            ));
        }
        Ok(())
    }

    /// Bind the actual completed tail once, using the common producer's private
    /// source inventory. Scratch-only execution without a native stage remains
    /// nonpublishable and cannot later manufacture this seal.
    pub(super) fn complete_native_output_tail(
        &mut self,
        sources: &super::output_capacity::OwnedExecutionSources,
    ) -> std::result::Result<(), String> {
        if !sources.is_native() || sources.proposal() != self._curr_block.hash() {
            return Err("native tail has a foreign producer source".into());
        }
        if self.native_lane_stage.is_none() {
            return Ok(());
        }
        self.validate_native_lane_stage_membership()
            .map_err(|error| error.to_string())?;
        self.verify_native_owned_fastpq_output_join(sources)?;
        let completed = self.merge_execution_write_set_root();
        let seal = self
            .native_lane_stage
            .as_mut()
            .and_then(Arc::get_mut)
            .ok_or("native tail lost exclusive ownership of its stage")?;
        if seal.completed_write_set_root.is_some() {
            return Err("native tail was completed twice".into());
        }
        seal.completed_write_set_root = Some(completed);
        Ok(())
    }

    /// Rejoin the complete actual native owner with the source-only proposal.
    /// Equal Network hashes alone cannot substitute different route Decisions.
    pub(crate) fn verify_native_execution_metadata(
        &self,
        block: &iroha_data_model::block::SignedBlock,
        executions: &[Execution],
    ) -> std::result::Result<(), String> {
        self.validate_native_output_carrier(block)?;
        if !self.settlement_accumulator.is_empty() {
            return Err("native metadata retains unbound start or tail settlement evidence".into());
        }
        let seal = self
            .native_lane_stage
            .as_ref()
            .ok_or("native metadata lost its stage")?;
        if executions.len() != seal.batch.groups.len()
            || executions.len() != seal.settlement_hashes.len()
        {
            return Err("native metadata lost its exact execution positions".into());
        }
        for (((execution, source), alias), settlement) in executions
            .iter()
            .zip(&seal.batch.groups)
            .zip(&seal.authenticated_aliases)
            .zip(&seal.settlement_hashes)
        {
            if execution.source != *source
                || execution.authenticated_signed_replay_alias != *alias
                || execution.settlement_hash != *settlement
                || super::canonical_merge_settlement_hash(&execution.settlement_commitment)
                    .map_err(|error| error.to_string())?
                    != *settlement
            {
                return Err("native metadata differs from its actual source or settlement".into());
            }
        }
        Ok(())
    }

    /// Rejoin the complete actual native owner with the source-only proposal.
    /// Equal Network hashes alone cannot substitute different route Decisions.
    pub(super) fn validate_native_output_carrier(
        &self,
        block: &iroha_data_model::block::SignedBlock,
    ) -> std::result::Result<(), String> {
        self.validate_native_lane_execution()
            .map_err(|error| error.to_string())?;
        self.validate_native_output_source(block)
    }

    /// Recheck retained immutable native authority after the global metadata
    /// finalizer. The complete World/event seal, witness and finality own that
    /// later cut; the earlier native prefix root cannot authorize publication.
    pub(crate) fn validate_native_output_source(
        &self,
        block: &iroha_data_model::block::SignedBlock,
    ) -> std::result::Result<(), String> {
        self.validate_native_lane_stage_membership()
            .map_err(|error| error.to_string())?;
        let seal = self
            .native_lane_stage
            .as_ref()
            .ok_or("native output has no stage")?;
        if seal.completed_write_set_root.is_none()
            || block.header() != seal.carrier
            || block.header().npos_effects_hash() != seal.npos_effects_hash
            || block
                .execution_context()
                .map(|bundle| HashOf::new(&bundle.queue_plan_admissions))
                != Some(seal.queue_plan_admissions_hash)
            || !block.external_entrypoints_slice().is_empty()
            || block
                .execution_context()
                .and_then(|context| context.native_lane_decisions.as_deref())
                != Some(seal.batch.as_ref())
        {
            return Err("native output carrier differs from its actual source/tail".into());
        }
        Ok(())
    }

    /// Install source/application markers after actual economics; errors discard
    /// the owning overlay, including all earlier hooks and route writes.
    fn stage_lane_decision_application_markers(
        &mut self,
        batch: &LaneDecisionBatchV1,
        identity: Hash,
    ) -> Result<()> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let encoded =
            norito::encode_canonical(&identity).map_err(|error| invalid(error.to_string()))?;
        let mut keys = vec![format!(
            "native_lane_application_{}",
            hex::encode(identity.as_ref())
        )];
        for group in &batch.groups {
            for slot in &group.payload.descriptor.slots {
                keys.push(format!(
                    "native_lane_applied_instance_{}",
                    hex::encode(slot.instance_id.as_ref())
                ));
            }
        }
        let paths = keys
            .into_iter()
            .map(|key| {
                let path: StatePath = key.parse().map_err(|_| {
                    invalid("native application marker is not a canonical State path".into())
                })?;
                if self.world.smart_contract_state.get(&path).is_some() {
                    return Err(MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                        "native instance/application marker {path} already exists"
                    )));
                }
                Ok(path)
            })
            .collect::<Result<Vec<_>>>()?;
        for path in paths {
            self.world
                .smart_contract_state
                .insert(path, encoded.clone());
        }
        Ok(())
    }
}
