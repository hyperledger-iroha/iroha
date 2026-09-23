// Consuming handoff from exact candidate validation to carrier preparation.

/// Exact output of one successful candidate validation and its frozen context.
/// Only the validators below can construct this input. Live preparation and
/// historical replay consume the same source-owned execution; neither accepts
/// an independently supplied State overlay, witness, or context.
pub(crate) struct ValidatedCarrierPreparationInput<'state> {
    valid: ValidBlock,
    state: Box<StateBlock<'state>>,
    context: Arc<iroha_data_model::block::consensus_v2::HeightContext>,
    native: Option<crate::state::NativeExecutionCustody>,
}

/// Historical execution retains its original Native sources until the isolated
/// State has passed finality, wire and checkpoint checks. Field order releases
/// all State writers before Native custody on every rejected replay.
pub(crate) struct ValidatedReplayExecution<'state> {
    pub(crate) valid: ValidBlock,
    pub(crate) state: Box<StateBlock<'state>>,
    pub(crate) native: Option<crate::state::NativeExecutionCustody>,
}

impl<'state> ValidatedCarrierPreparationInput<'state> {
    /// Transfer the entire validated scope into the private preparation owner.
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidBlock,
        Box<StateBlock<'state>>,
        Arc<iroha_data_model::block::consensus_v2::HeightContext>,
        Option<crate::state::NativeExecutionCustody>,
    ) {
        (self.valid, self.state, self.context, self.native)
    }
}

impl ValidBlock {
    /// Validate and consume one exact candidate into its metadata preparation.
    ///
    /// The caller already authenticated the immutable proposal and height
    /// context, as for the underlying candidate validator. No current-height
    /// finality lookup is required before Prepare. Events stay discarded, as in
    /// the production pre-vote caller; this does not grant Apply authority.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_and_prepare_sumeragi_v2_candidate_keep_voting_block<'state>(
        block: SignedBlock,
        topology: &Topology,
        genesis_account: &AccountId,
        time_source: &TimeSource,
        block_cadence: Duration,
        validation_context: SumeragiV2ValidationContext,
        state: &'state State,
        voting_block: &mut Option<VotingBlock>,
    ) -> Result<crate::state::PreparedCarrier<'state>, Error> {
        let Some(context) = validation_context.authenticated_height_context.clone() else {
            return Err((
                Box::new(block),
                Box::new(Self::execution_context_error(
                    "carrier preparation requires the exact authenticated height context",
                )),
            ));
        };
        let context_matches = context.id() == validation_context.context_id
            && context.height == block.header().height().get()
            && context.network_id == *state.network_id_ref()
            && topology
                .as_ref()
                .iter()
                .eq(context.roster.iter().map(|entry| &entry.validator));
        if !context_matches {
            return Err((
                Box::new(block),
                Box::new(Self::execution_context_error(
                    "carrier preparation differs from its frozen validation context",
                )),
            ));
        }
        let (valid, state) = Self::validate_sumeragi_v2_candidate_keep_voting_block(
            block,
            topology,
            genesis_account,
            time_source,
            block_cadence,
            validation_context,
            state,
            voting_block,
        )
        .unpack(|_| {})?;
        crate::state::PreparedCarrier::prepare(ValidatedCarrierPreparationInput {
            valid,
            state,
            context,
            native: None,
        })
        .map_err(|(block, reason)| {
            (
                block,
                Box::new(match reason {
                    crate::state::MergeLedgerCommitError::BlockHashAdmission(error) => {
                        BlockValidationError::BlockHashAdmission(error)
                    }
                    error => BlockValidationError::LocalStorageRecoveryRequired {
                        reason: format!("carrier preparation: {error}"),
                    },
                }),
            )
        })
    }
}

// Exact source-owned Native preparation through the shared global checks.
// This private path has no live admission, vote, or publication entry point.

/// Preserve the producer of a Native preparation failure through service dispatch.
/// Local storage and execution refusals must not become invalid-body markers
/// merely because their diagnostic text crossed this preparation boundary.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeCandidatePreparationError {
    /// The common global validator retains its local/deterministic distinction.
    #[error("Native global preflight: {0}")]
    Preflight(#[source] Box<BlockValidationError>),
    /// Exact source execution retains its typed admission and observation errors.
    #[error(transparent)]
    Execution(#[from] crate::state::MergeLedgerCommitError),
    /// An already executed owner could not complete local metadata preparation.
    /// This is not evidence that the authenticated proposal is invalid.
    #[error("Native carrier preparation requires recovery: {0}")]
    Preparation(#[source] crate::state::MergeLedgerCommitError),
}

impl ValidBlock {
    /// Consume exact Native sources only after the common proposal preflight.
    /// The returned private owner retains global context and original execution;
    /// resource admission, availability custody and publication remain required.
    pub(crate) fn prepare_native_candidate<'state>(
        source: crate::state::PreparedNativeLaneBatchSourceV1<'state>,
        context: crate::sumeragi::v2::VerifiedHeightContext,
        genesis_account: &AccountId,
        time_source: &TimeSource,
        block_cadence: Duration,
    ) -> Result<Option<crate::state::PreparedCarrier<'state>>, NativeCandidatePreparationError>
    {
        let Some(execution) = Self::validate_and_record_native_candidate(
            source, context, genesis_account, time_source, block_cadence,
        )? else {
            return Ok(None);
        };
        crate::state::PreparedCarrier::prepare(execution)
            .map(Some)
            .map_err(|(_, reason)| NativeCandidatePreparationError::Preparation(reason))
    }

    /// Sole Native global preflight and recorded execution, shared by live
    /// preparation and authenticated historical replay before either metadata tail.
    fn validate_and_record_native_candidate<'state>(
        source: crate::state::PreparedNativeLaneBatchSourceV1<'state>,
        context: crate::sumeragi::v2::VerifiedHeightContext,
        genesis_account: &AccountId,
        time_source: &TimeSource,
        block_cadence: Duration,
    ) -> Result<Option<ValidatedCarrierPreparationInput<'state>>, NativeCandidatePreparationError> {
        use crate::state::MergeLedgerCommitError;
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        let Some((state, body, generation)) = source.preparation_input() else {
            return Ok(None);
        };
        let preflight = (|| -> Result<(), BlockValidationError> {
            if !body.is_resultless_proposal() {
                return Err(Self::execution_context_error(
                    "Native preparation requires a resultless proposal",
                ));
            }
            super::native_lane_batch_for_execution(body).map_err(Self::execution_context_error)?;
            body.validate_proposal_commitments()
                .map_err(Self::execution_context_error)?;
            let frozen = context.context();
            if frozen.height != body.header().height().get()
                || frozen.network_id != *state.network_id_ref()
                || frozen
                    .parent_commit_qc
                    .as_ref()
                    .map(|qc| qc.subject.block_hash)
                    .or_else(|| frozen.snapshot_bootstrap.map(|anchor| anchor.snapshot_block_hash))
                    != body.header().prev_block_hash()
            {
                return Err(Self::execution_context_error(
                    "Native global proposal differs from its verified context",
                ));
            }
            crate::sumeragi::v2_body_store::verify_origin_block_signature(
                frozen,
                body,
                &crate::sumeragi::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            )
            .map_err(|error| Self::execution_context_error(error.to_string()))?;
            let wire = body
                .encode_wire()
                .map_err(|error| Self::execution_context_error(error.to_string()))?;
            if u64::try_from(wire.len())
                .ok()
                .is_none_or(|length| length > frozen.da_layout.max_payload_size_bytes)
            {
                return Err(Self::execution_context_error(
                    "Native global proposal exceeds its authenticated DA carrier limit",
                ));
            }
            let topology = Topology::new(frozen.roster.iter().map(|entry| entry.validator.clone()));
            let profile = ConsensusValidationProfile::NativePreparation {
                block_cadence,
                context: SumeragiV2ValidationContext::from_height_context(frozen),
            };
            let static_data = Self::validate_static_state_dependent(
                body,
                &topology,
                genesis_account,
                &state.query_view(),
                false,
                time_source,
                true,
                profile,
            )?;
            // Native has no ordinary external inputs. Its exact admitted inputs
            // and Decisions are validated by the consumed source below. Still
            // execute the common snapshot/commitment check on the actual body.
            let prepared = Self::prepare_external_transactions(body);
            let transactions = state.transactions.view();
            let committed =
                Self::committed_heights_for_prepared_transactions(&prepared, &transactions);
            let carriers = Self::committed_heights_for_entrypoint_carriers(body, &transactions);
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            Self::validate_static_with_snapshot(
                body,
                state.network_id_ref(),
                genesis_account,
                &static_data,
                &committed,
                &carriers,
                &prepared,
                metrics,
            )
        })();
        if generation != state.state_view_generation() {
            return Ok(None);
        }
        preflight.map_err(|error| NativeCandidatePreparationError::Preflight(Box::new(error)))?;
        let body = body.clone();
        let Some(recorded) = source.record_execution(body, context)? else {
            return Ok(None);
        };
        let (block, state, native) = recorded.into_preparation_parts().map_err(|reason| {
            NativeCandidatePreparationError::Preparation(
                MergeLedgerCommitError::ExecutionBatchInvalid(reason),
            )
        })?;
        let context = Arc::new(native.context().context().clone());
        Ok(Some(ValidatedCarrierPreparationInput {
            valid: Self::new_signatures_verified(block),
            state,
            context,
            native: Some(native),
        }))
    }
}
