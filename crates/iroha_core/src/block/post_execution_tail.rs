// The sole ordinary metadata finalizer runs inside State's consuming output seal.
// It cannot supply output rows, receipt owners or callback completion projections.

impl ValidBlock {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn finalize_owned_execution_metadata(
        block: &SignedBlock,
        state: &mut StateBlock<'_>,
        routes: &[crate::queue::RoutingDecision],
        advertised_fragments: Option<u64>,
        advertised_policy: Option<&AxtPolicySnapshot>,
        advertised_transitions: Option<&BTreeSet<DataSpaceId>>,
    ) -> Result<crate::state::ExecutionOutputSealMetadata, BlockValidationError> {
        Self::finalize_common_execution_metadata(
            block,
            state,
            routes,
            advertised_policy,
            advertised_transitions,
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
        state
            .stage_ordinary_lane_frontiers(block)
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "ordinary lane application frontier is invalid: {error}"
                ))
            })?;
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

    /// Shared deterministic metadata runs after actual Network/Pipeline/Time
    /// execution and before the one output seal captures its final World delta.
    fn finalize_common_execution_metadata(
        block: &SignedBlock,
        state: &mut StateBlock<'_>,
        routes: &[crate::queue::RoutingDecision],
        advertised_policy: Option<&AxtPolicySnapshot>,
        advertised_transitions: Option<&BTreeSet<DataSpaceId>>,
    ) -> Result<(), BlockValidationError> {
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
        Ok(())
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
}

include!("native_execution_metadata.rs");
