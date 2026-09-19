//! Consuming common output attachment, preserving State ownership on every exit.
//! Sealed actual outputs join the original witness and durable finality before
//! one deterministic metadata tail can authorize their exact journal publication.

use super::*;
use crate::queue::RoutingDecision;
use iroha_data_model::nexus::LaneFinalityStatement;

/// Block validation supplies its checked settlement projection before attachment.
/// Rows, sources, receipts, transcripts and applying policy remain State-owned.
pub(crate) struct ExecutionOutputSealMetadata {
    /// Actual fragment count after the finalizer's deterministic State changes.
    pub(crate) committed_fragment_count: u64,
    /// Complete statements derived from actual settlement and frozen source routes.
    pub(crate) lane_finality_statements: Vec<LaneFinalityStatement>,
}

/// Keep source-specific validation errors intact across State's consuming seal.
#[derive(Debug)]
pub(crate) enum ExecutionOutputSealError<E> {
    /// The State owner, retained source or canonical attachment was inconsistent.
    Owner(String),
    /// The block finalizer rejected its actual deterministic effects.
    Finalizer(E),
}

impl<E> From<String> for ExecutionOutputSealError<E> {
    fn from(error: String) -> Self {
        Self::Owner(error)
    }
}

impl<E> From<&str> for ExecutionOutputSealError<E> {
    fn from(error: &str) -> Self {
        Self::Owner(error.to_owned())
    }
}

struct SealOwner<'owner, 'state> {
    state: &'owner mut StateBlock<'state>,
    finished: bool,
}

impl Drop for SealOwner<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.state.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
        }
    }
}

impl SealedExecutionOutputs {
    /// Rejoin the original result-bearing attachment without resealing World.
    /// The deterministic carrier tail may already have changed the World delta;
    /// this check authenticates only the immutable wire captured by execution.
    pub(in crate::state) fn verify_wire_binding(&self, block: &SignedBlock) -> Result<(), String> {
        let wire = block.encode_wire().map_err(|error| error.to_string())?;
        if self.proposal != block.hash()
            || u64::try_from(wire.len()).ok() != Some(self.wire_bytes)
            || Hash::new(&wire) != self.wire_hash
        {
            return Err("execution output attachment changed after its seal".into());
        }
        Ok(())
    }
}

impl StateBlock<'_> {
    /// Bind the recorder's actual complete witness before extraction. A second
    /// capture cannot replace the first witness owned by this execution.
    pub(in crate::state) fn bind_execution_output_witness(
        &mut self,
        witness: &iroha_data_model::block::consensus::ExecWitness,
    ) -> Result<(), String> {
        if self.execution_output_plan.is_none() {
            return Ok(());
        }
        let surface = crate::state::output_publication::FinalizedPublicationSurface::capture(self)?;
        let Some(plan) = self.execution_output_plan.as_mut() else {
            return Ok(());
        };
        let ExecutionOutputPlanState::Sealed(sealed) = plan else {
            return Err("witness capture requires completed execution outputs".into());
        };
        let hash = HashOf::new(witness);
        match sealed.witness_hash {
            Some(previous) if previous != hash => {
                Err("captured execution witness changed after sealing".into())
            }
            _ => {
                if let Some(previous) = sealed.witness_surface.as_ref() {
                    if previous.as_ref() != &surface {
                        return Err("execution surface changed after witness capture".into());
                    }
                }
                sealed.witness_hash = Some(hash);
                sealed.witness_surface = Some(Box::new(surface));
                Ok(())
            }
        }
    }

    /// Authorize this exact execution only after its witness, result wire and
    /// verified finality have crossed the canonical durable Kura boundary.
    pub(crate) fn authorize_execution_output_publication(
        &mut self,
        block: &crate::block::CommittedBlock,
        witness: &iroha_data_model::block::consensus::ExecWitness,
    ) -> Result<(), String> {
        self.verify_execution_output_seal(block.as_ref())?;
        if self.native_lane_stage.is_some() {
            self.validate_native_output_source(block.as_ref())?;
        }
        let Some(ExecutionOutputPlanState::Sealed(sealed)) = self.execution_output_plan.as_ref()
        else {
            return Err("publication requires the original sealed output owner".into());
        };
        if sealed.witness_hash != Some(HashOf::new(witness)) {
            return Err("publication witness differs from the actual captured witness".into());
        }
        let artifact = block
            .verified_v2_finality_artifact()
            .ok_or("execution publication requires verified finality")?;
        let native = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            block.as_ref(), self.staged_merge_entry(),
        )?;
        let lanes = crate::sumeragi::exec::LaneFinalityManifestV1::from_result_bearing_block(
            block.as_ref(),
        )?;
        let actual = crate::sumeragi::exec::execution_commitment_from_validated_block(
            witness,
            &native,
            &lanes,
            block.as_ref(),
        )
        .map_err(str::to_owned)?;
        if actual != artifact.commit_qc.execution_commitment {
            return Err("captured execution differs from verified finality".into());
        }
        let durable = self
            .state_ref
            .kura
            .v2_finality_artifact(artifact.height)
            .map_err(|error| error.to_string())?
            .ok_or("execution finality has not been durably stored")?;
        if durable != *artifact {
            return Err("durable finality differs from execution authority".into());
        }
        let Some(ExecutionOutputPlanState::Sealed(sealed)) = self.execution_output_plan.take()
        else {
            unreachable!("exclusive borrow retains the checked seal")
        };
        self.execution_output_plan = Some(ExecutionOutputPlanState::Authorized(
            AuthorizedExecutionOutputs {
                sealed,
                finality_hash: HashOf::new(artifact).into(),
            },
        ));
        Ok(())
    }

    /// Consume authorized execution around the sole deterministic metadata tail.
    /// Failed or unwound preparation permanently poisons this publication owner.
    pub(in crate::state) fn finalize_authorized_execution_outputs(
        &mut self,
        block: &crate::block::CommittedBlock,
        prepare: impl FnOnce(
            &mut Self,
        ) -> Result<
            Vec<iroha_data_model::events::EventBox>,
            crate::state::MergeLedgerCommitError,
        >,
    ) -> Result<Vec<iroha_data_model::events::EventBox>, crate::state::MergeLedgerCommitError> {
        let invalid = crate::state::MergeLedgerCommitError::ExecutionBatchInvalid;
        let Some(ExecutionOutputPlanState::Authorized(authorized)) = self
            .execution_output_plan
            .replace(ExecutionOutputPlanState::Finalizing)
        else {
            self.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
            return Err(invalid(
                "execution publication lacks exact durable finality authorization".into(),
            ));
        };
        let mut owner = SealOwner {
            state: self,
            finished: false,
        };
        let state = &mut *owner.state;
        let artifact = block
            .verified_v2_finality_artifact()
            .ok_or_else(|| invalid("publication lost verified finality".into()))?;
        if state.native_lane_stage.is_some() {
            state
                .validate_native_output_source(block.as_ref())
                .map_err(invalid)?;
        }
        let wire = block
            .as_ref()
            .encode_wire()
            .map_err(|error| invalid(error.to_string()))?;
        if authorized.finality_hash != Hash::from(HashOf::new(artifact))
            || authorized.sealed.proposal != block.as_ref().hash()
            || state._curr_block != block.as_ref().header()
            || u64::try_from(wire.len()).ok() != Some(authorized.sealed.wire_bytes)
            || Hash::new(&wire) != authorized.sealed.wire_hash
            || state.world.net_state_delta().map_err(invalid)? != authorized.sealed.world_delta
        {
            return Err(invalid(
                "authorized execution changed before metadata preparation".into(),
            ));
        }
        authorized
            .sealed
            .witness_surface
            .as_ref()
            .ok_or_else(|| invalid("publication lost its captured execution surface".into()))?
            .verify(state)
            .map_err(invalid)?;
        let events = prepare(state)?;
        let surface = state
            .prepare_finalized_publication_surface()
            .map_err(invalid)?;
        state.execution_output_plan = Some(ExecutionOutputPlanState::Finalized(
            FinalizedExecutionOutputs {
                authorized,
                surface: Box::new(surface),
                _events_hash: crate::state::world_projection::hash_value(&events)
                    .map_err(invalid)?,
            },
        ));
        owner.finished = true;
        Ok(events)
    }

    /// Validate the retained linear owner immediately before journal publication.
    pub(in crate::state) fn verify_execution_output_publication(&self) -> Result<(), String> {
        match self.execution_output_plan.as_ref() {
            None if self.native_lane_stage.is_none() => Ok(()),
            Some(ExecutionOutputPlanState::Finalized(finalized)) => {
                if finalized.authorized.sealed.proposal != self._curr_block.hash() {
                    return Err("finalized execution belongs to another carrier".into());
                }
                finalized.surface.verify(self)
            }
            _ => Err(
                "execution output owner has not completed finality and publication preparation"
                    .into(),
            ),
        }
    }

    /// Run the complete execution output owner and consume its actual sources.
    /// The caller still owes source/finality and non-output resource admission.
    /// The sealed output owner does not grant commit authority by itself: the
    /// original witness, durable finality and publication surface must join it.
    pub(crate) fn execute_and_seal_ordinary_outputs<E>(
        &mut self,
        block: &mut SignedBlock,
        genesis: Option<&crate::block::AuthenticatedGenesisOutputSource>,
        finalize: impl FnOnce(
            &mut Self,
            &SignedBlock,
            &[RoutingDecision],
        ) -> Result<ExecutionOutputSealMetadata, E>,
    ) -> Result<(), ExecutionOutputSealError<E>> {
        self.reserve_ordinary_execution_outputs(block)?;
        self.execute_ordinary_output_plan(block, genesis)?;
        self.seal_execution_outputs(block, finalize)
    }

    /// Consume completed actual execution exactly once and attach all metadata.
    /// Taking or dropping the capsule never clears the publication guard. The
    /// finalizer cannot supply replacement rows, sources or policy. All its State
    /// effects precede transcript sealing and the sole checked model attachment.
    pub(crate) fn seal_execution_outputs<E>(
        &mut self,
        block: &mut SignedBlock,
        finalize: impl FnOnce(
            &mut Self,
            &SignedBlock,
            &[RoutingDecision],
        ) -> Result<ExecutionOutputSealMetadata, E>,
    ) -> Result<(), ExecutionOutputSealError<E>> {
        let Some(ExecutionOutputPlanState::Retained(retained)) = self
            .execution_output_plan
            .replace(ExecutionOutputPlanState::Sealing)
        else {
            self.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
            return Err("output seal requires one retained actual execution".into());
        };
        let mut owner = SealOwner {
            state: self,
            finished: false,
        };
        let state = &mut *owner.state;
        let result = (|| {
            // Reauthenticate auxiliary proposal bodies before any finalizer work;
            // equal headers and Network roots alone do not bind those bytes.
            block.validate_proposal_commitments()?;
            let input_root = MerkleTree::root_from_typed_leaves(
                block.network_entrypoints().map(TransactionEntrypoint::hash),
            )
            .map(Hash::from);
            if retained.proposal != block.hash()
                || block.header() != state._curr_block
                || retained.input_root != input_root
            {
                return Err("output seal source differs from its actual execution".into());
            }
            if retained.native {
                state.validate_native_output_carrier(block)?;
            } else if state.native_lane_stage.is_some()
                || block
                    .execution_context()
                    .is_some_and(|context| context.native_lane_decisions.is_some())
            {
                return Err("execution output seal cannot substitute native authority".into());
            }
            let sources = retained
                .sources
                .ok_or("output seal requires all actual phases")?;
            if sources.is_native() != retained.native
                || sources.proposal() != block.hash()
                || sources.source_context().network_id != state.network_id
                || sources.source_context().height != state._curr_block.height().get()
                || sources.entries().len() != retained.rows.len()
                || sources.network_routes().len() != block.network_entrypoint_count()
            {
                return Err("output seal lost its complete actual source inventory".into());
            }
            if !state.batch_transfer_outcomes.is_empty() {
                return Err("output seal retains unowned business receipts".into());
            }
            let metadata = finalize(state, block, sources.network_routes())
                .map_err(ExecutionOutputSealError::Finalizer)?;
            if !matches!(
                state.execution_output_plan,
                Some(ExecutionOutputPlanState::Sealing)
            ) {
                return Err("output finalizer invalidated its State owner".into());
            }
            if !state.batch_transfer_outcomes.is_empty() {
                return Err("output finalizer introduced unowned business receipts".into());
            }
            let fragments = u64::try_from(state.committed_fragment_count())
                .map_err(|_| "committed fragment count exceeds u64")?;
            if fragments != metadata.committed_fragment_count {
                return Err("output finalizer did not account for every applied fragment".into());
            }
            let tx_set = iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
                (0..block.network_entrypoint_count()).map(|index| {
                    block
                        .network_entrypoint_at(index)
                        .expect("immutable Network count and source positions agree")
                }),
            )
            .map_err(|error| error.to_string())?;
            state.set_fastpq_tx_set_hash(tx_set.into());
            let pending = state.submit_transfer_transcript_digest_batch();
            state.finalize_owned_fastpq_source_inventory_with_pending(&sources, pending)?;
            let transcripts = state.drain_transfer_transcripts_with_pending(None);
            let envelopes = state.drain_axt_envelopes();
            let policy = state.axt_policy_snapshot();
            let transitions = state.axt_authorization_transitioned().clone();
            let limits = state.frozen_output_capacity()?.policy.limits();
            let measured = retained.rows.iter().try_fold(0_u64, |total, row| {
                let bytes = norito::canonical_frame_len(row).map_err(|error| error.to_string())?;
                total
                    .checked_add(u64::try_from(bytes).map_err(|_| "row length exceeds u64")?)
                    .ok_or_else(|| "retained row bytes overflow".to_owned())
            })?;
            if measured != retained.row_bytes {
                return Err("retained rows differ from their consumed output budget".into());
            }
            // Borrow the actual World journals after every finalizer effect.
            // Encoding failure is a local ownership error before attachment.
            let world_delta = state.world.net_state_delta()?;
            block
                .set_execution_outputs(
                    retained.rows,
                    fragments,
                    transcripts,
                    envelopes,
                    policy,
                    transitions,
                    metadata.lane_finality_statements,
                    &limits,
                )
                .map_err(|error| error.to_string())?;
            let wire = block.encode_wire().map_err(|error| error.to_string())?;
            Ok(SealedExecutionOutputs {
                witness_hash: None,
                witness_surface: None,
                sources,
                world_delta,
                proposal: block.hash(),
                wire_hash: Hash::new(&wire),
                wire_bytes: u64::try_from(wire.len())
                    .map_err(|_| "sealed wire length exceeds u64")?,
            })
        })();
        match result {
            Ok(sealed) => {
                state.execution_output_plan = Some(ExecutionOutputPlanState::Sealed(sealed));
                owner.finished = true;
                Ok(())
            }
            Err(error) => {
                state.execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
                Err(error)
            }
        }
    }

    /// Check the exact attached wire while publication is still gated. Later
    /// signatures or result changes require the eventual final publication owner;
    /// they cannot reuse this earlier attachment's binding.
    pub(crate) fn verify_execution_output_seal(&self, block: &SignedBlock) -> Result<(), String> {
        let Some(ExecutionOutputPlanState::Sealed(sealed)) = self.execution_output_plan.as_ref()
        else {
            return Err("execution outputs do not have a completed seal".into());
        };
        let limits = self.frozen_output_capacity()?.policy.limits();
        block
            .validate_execution_outputs(&limits)
            .map_err(|error| error.to_string())?;
        sealed.verify_wire_binding(block)?;
        if self._curr_block != block.header() {
            return Err("execution output attachment changed after its seal".into());
        }
        if self.world.net_state_delta()? != sealed.world_delta {
            return Err("World values changed after the execution output seal".into());
        }
        if let Some(surface) = sealed.witness_surface.as_ref() {
            surface.verify(self)?;
        }
        self.verified_fastpq_source_inventory_for_capture()?;
        Ok(())
    }
}
