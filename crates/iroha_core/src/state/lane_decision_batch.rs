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
    membership: HashSet<HashOf<TransactionEntrypoint>>,
    application_write_set_root: Hash,
    write_set_root: Hash,
    completed_write_set_root: Option<Hash>,
    settlements: Vec<super::LaneBlockCommitment>,
    fastpq: super::native_lane_fastpq::NativeLaneFastpqSeal,
}

/// One start/native overlay and its actual outputs, transferable only to the
/// canonical global owner. Transfer alone grants no publication authority.
pub(crate) struct PreparedLaneDecisionBatchV1<'state> {
    overlay: Box<StateBlock<'state>>,
    batch: Arc<LaneDecisionBatchV1>,
    executions: Vec<Execution>,
}
impl<'state> PreparedLaneDecisionBatchV1<'state> {
    fn from_stage(overlay: Box<StateBlock<'state>>, executions: Vec<Execution>) -> Result<Self> {
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
        Ok(Self {
            overlay,
            batch,
            executions,
        })
    }
    /// Exact input-only proposal source, independent of the outputs below.
    pub(crate) fn batch(&self) -> &LaneDecisionBatchV1 {
        &self.batch
    }
    /// Actual outputs for the eventual sole standard result projection.
    pub(crate) fn executions(&self) -> &[Execution] {
        &self.executions
    }
    /// Transfer sole execution custody to the canonical global output finalizer.
    /// This does not authorize publication; the retained native/output seals
    /// still require actual witness capture and exact durable global finality.
    pub(crate) fn into_overlay(self) -> Box<StateBlock<'state>> {
        self.overlay
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
    /// Construct bounded input-only proposal data without running any instruction.
    /// Source authority still requires exact first-carrier and Decision validation
    /// in the consumer; this portable value grants no execution/publication token.
    pub(crate) fn prepare_lane_decision_batch(
        &self,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<LaneDecisionBatchV1> {
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
    pub(crate) fn replay_lane_decision_batch(
        &self,
        carrier: &BlockHeader,
        batch: &LaneDecisionBatchV1,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        with_stable_observation(self, || {
            let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
            batch.canonical_hash().map_err(invalid)?;
            if self.prepare_lane_decision_batch(groups)? != *batch {
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
    /// without taking/resetting an unrelated caller's execution-witness recorder.
    pub(super) fn prepare_native_batch_on_carrier(
        &self,
        header: BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        with_stable_observation(self, || {
            let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
            let batch = self.prepare_lane_decision_batch(groups)?;
            let (overlay, executions) =
                self.with_native_lane_execution(header, groups, |overlay, results| {
                    overlay.seal_native_lane_decision_batch(results, batch)
                })?;
            PreparedLaneDecisionBatchV1::from_stage(overlay, executions)
        })
    }

    /// Canonical native constructor under an already-owned witness recorder.
    /// Source authentication occurs before this method; the pristine callback
    /// applies only independently authenticated carrier controls after preflight.
    pub(super) fn prepare_native_batch_with_pristine_stage<'state>(
        &'state self,
        header: BlockHeader,
        batch: &LaneDecisionBatchV1,
        groups: &[VerifiedLaneDecisionGroupV1],
        pristine: impl FnOnce(&mut StateBlock<'state>) -> Result<()>,
    ) -> Result<PreparedLaneDecisionBatchV1<'state>> {
        with_stable_observation(self, || {
            if self.prepare_lane_decision_batch(groups)? != *batch {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "native canonical source differs from exact applying pre-State".into(),
                ));
            }
            let (overlay, executions) = self.with_native_lane_execution_and_pristine_stage(
                header,
                groups,
                pristine,
                |overlay, results| overlay.seal_native_lane_decision_batch(results, batch.clone()),
            )?;
            PreparedLaneDecisionBatchV1::from_stage(overlay, executions)
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
            || !self.staged_queue_plan_admissions.is_empty()
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
        self.native_lane_stage = Some(Box::new(NativeLaneStageSealV1 {
            carrier: self._curr_block.clone(),
            batch: Arc::new(batch),
            batch_hash,
            authenticated_aliases,
            membership,
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
            || !self.staged_queue_plan_admissions.is_empty()
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
            .ok_or("native tail lost its stage")?;
        if seal.completed_write_set_root.is_some() {
            return Err("native tail was completed twice".into());
        }
        seal.completed_write_set_root = Some(completed);
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

    /// Actual settlements retained from the original native Network execution.
    /// The global finalizer projects these same receipts; it never reexecutes or
    /// accepts caller-supplied native economic results.
    pub(crate) fn native_lane_settlement_commitments(
        &self,
    ) -> std::result::Result<Option<&[super::LaneBlockCommitment]>, String> {
        if self.native_lane_stage.is_none() {
            return Ok(None);
        }
        self.validate_native_lane_stage_membership()
            .map_err(|error| error.to_string())?;
        Ok(self
            .native_lane_stage
            .as_ref()
            .map(|seal| seal.settlements.as_slice()))
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
