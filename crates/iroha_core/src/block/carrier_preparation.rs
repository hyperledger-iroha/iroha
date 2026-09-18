// Consuming handoff from exact candidate validation to carrier preparation.

/// Exact output of one successful candidate validation and its frozen context.
/// Only the validator below can construct this input; preparation accepts no
/// independently supplied block, State overlay, witness, or context.
pub(crate) struct ValidatedCarrierPreparationInput<'state> {
    valid: ValidBlock,
    state: Box<StateBlock<'state>>,
    context: Arc<iroha_data_model::block::consensus_v2::HeightContext>,
}

impl<'state> ValidatedCarrierPreparationInput<'state> {
    /// Transfer the entire validated scope into the private preparation owner.
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidBlock,
        Box<StateBlock<'state>>,
        Arc<iroha_data_model::block::consensus_v2::HeightContext>,
    ) {
        (self.valid, self.state, self.context)
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
        })
        .map_err(|(block, reason)| {
            (
                block,
                Box::new(Self::execution_context_error(format!(
                    "carrier preparation: {reason}"
                ))),
            )
        })
    }
}
