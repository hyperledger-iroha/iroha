//! Seal the actual native economic overlay into one portable global transcript.
//!
//! This inactive builder grants no publication or lane ApplicationCompleted.
//! TODO: replace the old merge consumer and wire it to actual global execution
//! replay before the process-lived native owner becomes the sole live signer.

use iroha_model_base::state_path::StatePath;

use super::{
    MergeExecutionCommitSurface, MergeLedgerCommitError, State, StateBlock,
    VerifiedLaneDecisionGroupV1,
};
use iroha_crypto::Hash;
use iroha_data_model::block::{
    BlockHeader,
    lane_execution::{LaneDecisionExecutionBatchV1, LaneDecisionExecutionV1},
};

type Result<T> = std::result::Result<T, MergeLedgerCommitError>;

/// Private result pairs the exact scratch transition with its committed wire claims.
/// Dropping it discards every economic/pending/frontier/marker change together.
pub(crate) struct PreparedLaneDecisionBatchV1<'state> {
    overlay: StateBlock<'state>,
    batch: LaneDecisionExecutionBatchV1,
}
impl PreparedLaneDecisionBatchV1<'_> {
    /// Untrusted portable representation for eventual global candidate assembly.
    pub(crate) fn batch(&self) -> &LaneDecisionExecutionBatchV1 {
        &self.batch
    }
    /// Read-only scratch state for checks; no commit authority is exposed here.
    #[cfg(test)]
    pub(super) fn overlay(&self) -> &StateBlock<'_> {
        &self.overlay
    }
}

impl State {
    /// Replay a portable batch from separately authenticated current input groups.
    ///
    /// Every claim is recomputed on the actual carrier base before the private
    /// overlay is returned. This grants neither historical source authority nor
    /// publication permission; the caller must retain the global commit gate.
    pub(crate) fn replay_lane_decision_execution_batch(
        &self,
        carrier: &BlockHeader,
        batch: &LaneDecisionExecutionBatchV1,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        batch.canonical_hash().map_err(invalid)?;
        if batch.application_block_header
            != LaneDecisionExecutionBatchV1::application_header_from_carrier(carrier)
            || batch.executions.len() != groups.len()
            || batch
                .executions
                .iter()
                .zip(groups)
                .any(|(execution, group)| execution.source != group.to_wire())
        {
            return Err(invalid(
                "native execution source or application context differs from its verified carrier input".into(),
            ));
        }
        let prepared = self.prepare_lane_decision_execution_batch(
            batch.application_block_header.clone(),
            groups,
        )?;
        if prepared.batch() != batch {
            return Err(MergeLedgerCommitError::ExecutionDivergence(
                "native execution results, settlement, base or writes differ from replay".into(),
            ));
        }
        Ok(prepared)
    }

    /// Execute, bind and seal one actual economic transition at a coherent base.
    ///
    /// An observation race drops the entire owned overlay. The caller must still
    /// pass normal candidate signing/replay/publication checks; a prepared batch
    /// is not global finality and cannot close any lane instance.
    pub(crate) fn prepare_lane_decision_execution_batch(
        &self,
        header: BlockHeader,
        groups: &[VerifiedLaneDecisionGroupV1],
    ) -> Result<PreparedLaneDecisionBatchV1<'_>> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        if header != LaneDecisionExecutionBatchV1::application_header_from_carrier(&header) {
            return Err(invalid(
                "native application header retains payload-dependent commitments".into(),
            ));
        }
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Err(invalid("native execution base is being published".into()));
        }
        let base_state_hash = self.lane_execution_state_hash();
        let (mut overlay, results) =
            self.preexecute_lane_decision_groups(header.clone(), groups)?;
        let base_state_height = u64::try_from(overlay.block_hashes.len())
            .map_err(|_| invalid("native execution base height overflows".into()))?;
        let executions = results
            .into_iter()
            .map(|result| LaneDecisionExecutionV1 {
                source: result.source,
                result: result.result,
                authenticated_signed_replay_alias: result.authenticated_signed_replay_alias,
                settlement: result.settlement_commitment,
                fastpq_transcripts: result.fastpq_transcripts,
            })
            .collect();
        // This global merge field is derived by the same existing metadata owner;
        // native route frontiers were already staged atomically by the executor.
        overlay.stage_merge_metadata_values(&[], crate::merge::reduce_merge_hint_roots(&[]));
        overlay.validate_merge_execution_commit_surface(MergeExecutionCommitSurface::Pristine)?;
        let mut batch = LaneDecisionExecutionBatchV1 {
            base_state_height,
            base_state_hash,
            application_block_header: header,
            executions,
            application_write_set_root: overlay.merge_execution_write_set_root(),
            write_set_root: Hash::prehashed([0; Hash::LENGTH]),
        };
        let identity = batch.application_identity().map_err(invalid)?;
        overlay.stage_lane_decision_application_markers(&batch, identity)?;
        batch.write_set_root = overlay.merge_execution_write_set_root();
        // Final hashing validates shape and exact complete wire size. This is not
        // the eventual enclosing carrier/framing budget, which is checked later.
        batch.canonical_hash().map_err(invalid)?;
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err(invalid(
                "native execution base changed during preparation".into(),
            ));
        }
        Ok(PreparedLaneDecisionBatchV1 { overlay, batch })
    }
}

impl StateBlock<'_> {
    /// Install batch and per-instance replay markers only after the full economic delta.
    /// Any failure drops the owning scratch block, including all prior route writes.
    fn stage_lane_decision_application_markers(
        &mut self,
        batch: &LaneDecisionExecutionBatchV1,
        identity: Hash,
    ) -> Result<()> {
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let mut markers = vec![(
            format!("native_lane_application_{}", hex::encode(identity.as_ref())),
            norito::encode_canonical(&identity).map_err(|error| invalid(error.to_string()))?,
        )];
        for execution in &batch.executions {
            for slot in &execution.source.payload.descriptor.slots {
                markers.push((
                    format!(
                        "native_lane_applied_instance_{}",
                        hex::encode(slot.instance_id.as_ref())
                    ),
                    norito::encode_canonical(&identity)
                        .map_err(|error| invalid(error.to_string()))?,
                ));
            }
        }
        let markers = markers
            .into_iter()
            .map(|(key, value)| {
                let path: StatePath = key.parse().map_err(|_| {
                    invalid("native application marker is not a canonical State path".into())
                })?;
                if self.world.smart_contract_state.get(&path).is_some() {
                    return Err(MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                        "native instance/application marker {path} already exists"
                    )));
                }
                Ok((path, value))
            })
            .collect::<Result<Vec<_>>>()?;
        for (path, value) in markers {
            self.world.smart_contract_state.insert(path, value);
        }
        Ok(())
    }
}
