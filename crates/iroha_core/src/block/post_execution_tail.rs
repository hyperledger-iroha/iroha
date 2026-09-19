// The common metadata finalizer runs inside State's consuming output seal.
// It cannot supply output rows, receipt owners or callback completion projections.

impl ValidBlock {
    /// Project only actual settlement contributions. Native execution also
    /// retains empty per-source commitments to prove absence of fee receipts;
    /// those do not require a cross-lane settlement statement or manifest.
    fn nonempty_native_lane_settlements(
        commitments: &[LaneBlockCommitment],
    ) -> Result<Vec<LaneBlockCommitment>, BlockValidationError> {
        let mut nonempty = Vec::new();
        for commitment in commitments {
            if commitment.tx_count != 0 {
                nonempty.push(commitment.clone());
            } else if commitment.total_local_amount != Quantity::zero()
                || commitment.total_xor_due != Quantity::zero()
                || commitment.total_xor_after_haircut != Quantity::zero()
                || commitment.total_xor_variance != Quantity::zero()
                || commitment.swap_metadata.is_some()
                || !commitment.receipts.is_empty()
                || !commitment.nexus_fee_receipts.is_empty()
                || !commitment.native_amx_receipts.is_empty()
            {
                return Err(Self::execution_context_error(
                    "native settlement evidence has no contributing source transaction",
                ));
            }
        }
        Ok(nonempty)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn finalize_owned_execution_metadata(
        block: &SignedBlock,
        state: &mut StateBlock<'_>,
        routes: &[crate::queue::RoutingDecision],
        advertised_fragments: Option<u64>,
        advertised_policy: Option<&AxtPolicySnapshot>,
        advertised_transitions: Option<&BTreeSet<DataSpaceId>>,
    ) -> Result<crate::state::ExecutionOutputSealMetadata, BlockValidationError> {
        if routes.len() != block.network_entrypoint_count() {
            return Err(Self::execution_context_error(
                "settlement lost its complete frozen Network routes",
            ));
        }
        let pruned = crate::tx::prune_expired_sealed_commitments(state);
        if pruned != 0 {
            iroha_logger::debug!(
                count = pruned,
                "pruned expired sealed transaction commitments"
            );
        }
        let fragments = u64::try_from(state.committed_fragment_count())
            .map_err(|_| Self::execution_context_error("committed fragments exceed u64"))?;
        state.finalize_axt_asset_incarnations().map_err(|error| {
            Self::execution_context_error(format!(
                "failed to finalize AXT asset incarnations: {error}"
            ))
        })?;
        state
            .evaluate_nexus_autoscale(block, fragments)
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to evaluate Nexus autoscale: {error}"
                ))
            })?;
        state
            .finalize_axt_policy_transition_ratchets()
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to finalize the AXT policy counter ratchet: {error}"
                ))
            })?;
        let policy = state.axt_policy_snapshot();
        Self::validate_advertised_axt_post_state(advertised_policy, &policy)?;
        Self::validate_advertised_axt_transitions(
            advertised_transitions,
            state.axt_authorization_transitioned(),
            policy.version,
        )?;
        let mut summaries: BTreeMap<LaneId, LaneSummary> = BTreeMap::new();
        let mut routed = Vec::new();
        for (input, route) in block.network_entrypoints().zip(routes) {
            let summary = summaries.entry(route.lane_id).or_default();
            summary.tx_vertices = summary.tx_vertices.saturating_add(1);
            if let Some(signed) = Self::signed_transaction_from_entrypoint(input) {
                let metadata = AcceptedTransaction::prepare_signed_metadata(signed);
                let bytes = u64::try_from(metadata.encoded_len).map_err(|_| {
                    Self::execution_context_error("routed signed transaction length exceeds u64")
                })?;
                summary.rbc_bytes_total =
                    summary.rbc_bytes_total.checked_add(bytes).ok_or_else(|| {
                        Self::execution_context_error("routed lane byte count overflow")
                    })?;
                routed.push((signed.hash(), *route));
            }
        }
        let lane_finality_statements =
            Self::finalize_lane_settlement_evidence(block, state, &routed, &summaries)?;
        // These are real World writes and must precede the seal's net-delta cut.
        if block
            .execution_context()
            .is_none_or(|bundle| bundle.native_lane_decisions.is_none())
        {
            state
                .stage_ordinary_lane_frontiers(block)
                .map_err(|error| {
                    Self::execution_context_error(format!(
                        "ordinary lane application frontier is invalid: {error}"
                    ))
                })?;
        }
        let height = usize::try_from(block.header().height().get())
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .ok_or_else(|| {
                Self::execution_context_error("carrier height exceeds host membership width")
            })?;
        let membership = crate::tx::canonical_carrier_membership_hashes(
            state,
            block.external_entrypoints_slice(),
        );
        state
            .stage_canonical_carrier_membership(membership, height)
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to stage canonical carrier membership: {error}"
                ))
            })?;
        state
            .resolve_queue_plan_pending_obligations_from_block(block)
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "QueuePlan pending application obligation could not be resolved: {error}"
                ))
            })?;
        let committed_fragment_count =
            Self::validated_committed_fragment_count(state, advertised_fragments)?;
        Ok(crate::state::ExecutionOutputSealMetadata {
            committed_fragment_count,
            lane_finality_statements,
        })
    }

    /// Qualify the canonical native owner, including pristine controls, the
    /// actual complete common output tail and the same recorder through capture.
    /// This grants neither global admission nor publication authority.
    #[cfg(test)]
    pub(crate) fn execute_native_block_and_capture_for_test<'state>(
        block: &mut SignedBlock,
        state: &'state State,
        context: &consensus_v2::HeightContext,
    ) -> Result<Box<StateBlock<'state>>, BlockValidationError> {
        native_lane_batch_for_execution(block).map_err(Self::execution_context_error)?;
        let _guard = crate::sumeragi::witness::exec_witness_guard();
        crate::sumeragi::witness::start_block();
        let mut overlay = Self::state_block_for_execution(
            block,
            state,
            false,
            Some(context.mode),
            Some(context),
            None,
        )?;
        Self::validate_staged_execution_controls(block, &overlay)?;
        Self::execute_and_record_canonical_outputs(
            block,
            &mut overlay,
            None,
            SccpRootValidation::Enforce,
            None,
        )?;
        validate_axt_envelopes(block, &mut overlay)?;
        overlay
            .finalize_lane_consensus_contexts(block, Some(context))
            .and_then(|()| overlay.capture_exec_witness())
            .map_err(Self::execution_context_error)?;
        Ok(overlay)
    }

    /// Reexecute a fixture through the actual whole producer and metadata finalizer.
    /// The explicit genesis key is fixture trust input; ordinary source/finality
    /// admission remains the caller's responsibility. No supplied rows enter here.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn execute_block_outputs_for_test(
        block: &mut SignedBlock,
        state: &mut StateBlock<'_>,
        genesis_account: Option<&AccountId>,
    ) -> Result<(), BlockValidationError> {
        let genesis = genesis_account
            .map(|account| authenticate_genesis_block_intents(block, account))
            .transpose()?;
        Self::validate_staged_execution_controls(block, state)?;
        let _guard = crate::sumeragi::witness::exec_witness_guard();
        Self::execute_and_record_canonical_outputs(
            block,
            state,
            None,
            SccpRootValidation::Enforce,
            genesis.as_ref(),
        )
    }

    /// Execute a fixture and retain its actual witness under the same recorder owner.
    /// Genesis binds the supplied context to the resulting staged policy before
    /// lane-context finalization. This grants no publication or finality authority.
    #[cfg(test)]
    pub(crate) fn execute_block_outputs_and_capture_for_test(
        block: &mut SignedBlock,
        state: &mut StateBlock<'_>,
        genesis_account: Option<&AccountId>,
        context: &mut consensus_v2::HeightContext,
    ) -> Result<(), BlockValidationError> {
        let genesis = genesis_account
            .map(|account| authenticate_genesis_block_intents(block, account))
            .transpose()?;
        Self::validate_staged_execution_controls(block, state)?;
        let _guard = crate::sumeragi::witness::exec_witness_guard();
        Self::execute_and_record_canonical_outputs(
            block,
            state,
            None,
            SccpRootValidation::Enforce,
            genesis.as_ref(),
        )?;
        if genesis.is_some() {
            context.nexus_amx_context_hash =
                crate::sumeragi::staged_genesis_nexus_amx_context_hash(state);
            context.execution_policy_hash =
                crate::sumeragi::staged_genesis_execution_policy_hash(state)
                    .map_err(|error| Self::execution_context_error(error.to_string()))?;
        }
        state
            .finalize_lane_consensus_contexts(block, Some(context))
            .and_then(|()| state.capture_exec_witness())
            .map_err(Self::execution_context_error)
    }
}
