//! Execute authenticated native inputs under the actual proposal header.
//!
//! Proposal bytes contain only source Decisions and the exact applying pre-State.
//! Actual outputs and prefix roots have private ownership until the common global
//! result/witness projection; they never feed back into the executing block hash.
//! TODO: integrate the full suffix/witness and publication/Apply authorization
//! before accepting native carriers in ValidBlock or State commit.

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
    fastpq: super::native_lane_fastpq::NativeLaneFastpqSeal,
}

/// One disposable start/native overlay and its actual outputs.
/// Production has no mutable/consuming overlay publication accessor.
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
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Err(invalid("native execution base is being published".into()));
        }
        let batch = LaneDecisionBatchV1 {
            base_state_height: u64::try_from(self.view().block_hashes.len())
                .map_err(|_| invalid("native execution base height overflows".into()))?,
            base_state_hash: self.lane_execution_state_hash(),
            groups: groups
                .iter()
                .map(VerifiedLaneDecisionGroupV1::to_wire)
                .collect(),
        };
        batch.canonical_hash().map_err(invalid)?;
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err(invalid(
                "native execution base changed during source preparation".into(),
            ));
        }
        Ok(batch)
    }

    /// Execute verified sources under their actual carrier, after shared start hooks.
    /// Full economic outputs are authenticated by global execution, never proposal claims.
    pub(crate) fn replay_lane_decision_batch(
        &self,
        carrier: &BlockHeader,
        batch: &LaneDecisionBatchV1,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        batch.canonical_hash().map_err(invalid)?;
        if self.prepare_lane_decision_batch(groups)? != *batch {
            return Err(invalid(
                "native source differs from its exact verified inputs or applying pre-State".into(),
            ));
        }
        let prepared = self.prepare_native_batch_on_carrier(carrier.clone(), groups)?;
        if prepared.batch() != batch {
            return Err(invalid(
                "native applying pre-State changed before execution".into(),
            ));
        }
        Ok(prepared)
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
        let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
        let generation = self.state_view_generation();
        let batch = self.prepare_lane_decision_batch(groups)?;
        let (overlay, executions) =
            self.with_native_lane_execution(header, groups, |overlay, results| {
                overlay.seal_native_lane_decision_batch(results, batch)
            })?;
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native execution base changed during preparation".into(),
            ));
        }
        PreparedLaneDecisionBatchV1::from_stage(overlay, executions)
    }
}

impl StateBlock<'_> {
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
        self.stage_merge_metadata_values(&[], crate::merge::reduce_merge_hint_roots(&[]));
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
            fastpq,
        }));
        self.validate_native_lane_execution_prefix()?;
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

    /// Check actual start+native writes at the constructor's sealed prefix.
    pub(super) fn validate_native_lane_execution_prefix(&self) -> Result<()> {
        self.validate_native_lane_stage_membership()?;
        if !self.start_of_block_effects_applied
            || self.merge_execution_write_set_root()
                != self
                    .native_lane_stage
                    .as_ref()
                    .expect("checked native seal")
                    .write_set_root
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native start+execution prefix writes changed or hooks did not run".into(),
            ));
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
