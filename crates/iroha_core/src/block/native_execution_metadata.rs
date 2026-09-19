// Native metadata consumes actual already-executed settlement and source custody.
// It neither translates Native Decisions into old AMX receipts nor reruns economics.

impl ValidBlock {
    /// Attach the common output result to this exact completed Native execution.
    ///
    /// The State stage must bind every supplied source and settlement hash to its
    /// original producer. This consumes no new inputs and does not reset/drain the
    /// witness recorder: its owner must retain the guard through lane-context
    /// finalization and checked witness capture. This is not block validity, State
    /// publication, or Native Apply authority; the live Native gate stays closed.
    pub(crate) fn seal_native_execution_outputs(
        block: &mut SignedBlock,
        state: &mut StateBlock<'_>,
        executions: &[crate::state::PreexecutedLaneDecisionGroupV1],
    ) -> Result<(), BlockValidationError> {
        state
            .verify_native_execution_metadata(block, executions)
            .map_err(Self::execution_context_error)?;
        let advertised_fragments = block.committed_fragment_count();
        let advertised_policy = block.axt_policy_snapshot().cloned();
        let advertised_transitions = block.axt_transitioned_dataspaces().cloned();
        if block.has_results() {
            block
                .validate_output_merkle_cache()
                .map_err(|error| Self::execution_context_error(error.to_string()))?;
            advertised_policy
                .as_ref()
                .ok_or_else(|| {
                    Self::execution_context_error("Native result is missing its AXT policy")
                })?
                .validate()
                .map_err(|error| Self::execution_context_error(error.to_string()))?;
        }
        state
            .seal_execution_outputs(block, |state, source, routes| {
                // The common seal rejoined the completed write cut before this
                // finalizer. Validate retained settlement/source custody before
                // any allowed finalizer writes move beyond that prefix.
                state
                    .verify_native_execution_metadata(source, executions)
                    .map_err(Self::execution_context_error)?;
                Self::finalize_common_execution_metadata(
                    source,
                    state,
                    routes,
                    advertised_policy.as_ref(),
                    advertised_transitions.as_ref(),
                )?;
                let lane_finality_statements =
                    Self::native_execution_finality_statements(source, state, executions, routes)?;
                let height = usize::try_from(source.header().height().get())
                    .ok()
                    .and_then(std::num::NonZeroUsize::new)
                    .ok_or_else(|| {
                        Self::execution_context_error("Native carrier membership height overflows")
                    })?;
                // Native economics already staged the exact group membership,
                // authenticated signed aliases, obligations and all-route tips.
                // An empty ordinary set means there are no external inputs; it
                // supplies no Native/old-merge permission of its own.
                state
                    .stage_canonical_carrier_membership(
                        std::iter::empty::<HashOf<TransactionEntrypoint>>(),
                        height,
                    )
                    .map_err(|error| Self::execution_context_error(error.to_string()))?;
                let committed_fragment_count =
                    Self::validated_committed_fragment_count(state, advertised_fragments)?;
                Ok(crate::state::ExecutionOutputSealMetadata {
                    committed_fragment_count,
                    lane_finality_statements,
                })
            })
            .map_err(|error| match error {
                crate::state::ExecutionOutputSealError::Owner(reason) => {
                    Self::execution_context_error(reason)
                }
                crate::state::ExecutionOutputSealError::Finalizer(error) => error,
            })?;
        state
            .verify_execution_output_seal(block)
            .map_err(Self::execution_context_error)
    }

    /// Validate whether an actual native commitment has receipt-backed relay work.
    /// Empty per-source commitments prove absence of fees, regardless of input count.
    fn native_settlement_requires_relay(
        commitment: &LaneBlockCommitment,
    ) -> Result<bool, BlockValidationError> {
        let has_receipts = !commitment.receipts.is_empty()
            || !commitment.nexus_fee_receipts.is_empty()
            || !commitment.native_amx_receipts.is_empty();
        if !has_receipts {
            if !commitment.total_local_amount.is_zero()
                || !commitment.total_xor_due.is_zero()
                || !commitment.total_xor_after_haircut.is_zero()
                || !commitment.total_xor_variance.is_zero()
                || commitment.swap_metadata.is_some()
            {
                return Err(Self::execution_context_error(
                    "Native settlement has unbound economic totals without receipts",
                ));
            }
            return Ok(false);
        }
        if commitment.tx_count == 0 {
            return Err(Self::execution_context_error(
                "native settlement evidence has no contributing source transaction",
            ));
        }
        Ok(true)
    }

    /// Project actual economic relay effects without draining or applying them again.
    ///
    /// The Native group source/output authenticates receipt-free input execution.
    /// Such a group needs no old relay statement, even when its atomic-group
    /// count is one. A real receipt-bearing commitment must satisfy the existing
    /// complete envelope integrity and nonzero policy-root contract unchanged.
    fn native_execution_finality_statements(
        block: &SignedBlock,
        state: &StateBlock<'_>,
        executions: &[crate::state::PreexecutedLaneDecisionGroupV1],
        routes: &[crate::queue::RoutingDecision],
    ) -> Result<Vec<iroha_data_model::nexus::LaneFinalityStatement>, BlockValidationError> {
        if executions.len() != routes.len() || executions.len() != block.network_entrypoint_count()
        {
            return Err(Self::execution_context_error(
                "Native settlement lost its complete ordered Network sources",
            ));
        }
        let policy = state.axt_policy_snapshot();
        let mut statements = Vec::new();
        for (execution, route) in executions.iter().zip(routes) {
            let source = &execution.source;
            let plan = source
                .payload
                .input
                .routing_plan()
                .map_err(Self::execution_context_error)?;
            if plan.coordinator_route() != *route {
                return Err(Self::execution_context_error(
                    "Native settlement route differs from its executed coordinator",
                ));
            }
            let (slot_index, slot) = source
                .payload
                .descriptor
                .slots
                .iter()
                .enumerate()
                .find(|(_, slot)| slot.route == *route)
                .ok_or_else(|| {
                    Self::execution_context_error("Native settlement has no exact coordinator slot")
                })?;
            let decision = source.decisions.get(slot_index).ok_or_else(|| {
                Self::execution_context_error("Native settlement has no exact coordinator Decision")
            })?;
            let commitment = &execution.settlement_commitment;
            if (
                commitment.lane_id,
                commitment.dataspace_id,
                commitment.lane_incarnation,
                commitment.block_height,
            ) != (
                slot.route.lane_id,
                slot.route.dataspace_id,
                slot.lane_incarnation,
                slot.lane_height,
            ) || iroha_data_model::nexus::compute_settlement_hash(commitment)
                .map_err(|error| Self::execution_context_error(error.to_string()))?
                != execution.settlement_hash
            {
                return Err(Self::execution_context_error(
                    "Native settlement differs from its executed coordinate or exact commitment",
                ));
            }
            if !Self::native_settlement_requires_relay(commitment)? {
                continue;
            }
            let descriptor_hash = source
                .payload
                .descriptor
                .canonical_hash()
                .map_err(Self::execution_context_error)?;
            // Attribute the actual signed RS16 body length for this coordinator;
            // do not invent an ordinary LanePayloadOwnership or participant QC.
            let mut envelope = LaneRelayEnvelope::new(
                block.header(),
                block.header().da_commitments_hash(),
                commitment.clone(),
                decision.manifest.byte_len,
            )
            .map_err(|error| Self::execution_context_error(error.to_string()))?
            .with_lane_block_descriptor_hash(Some(descriptor_hash));
            envelope.manifest_root = policy
                .entries
                .iter()
                .find(|entry| entry.dsid == commitment.dataspace_id)
                .map(|entry| entry.policy.manifest_root);
            let statement = envelope.lane_finality_statement().map_err(|error| {
                Self::execution_context_error(format!(
                    "Native economic relay effect is incomplete: {error}"
                ))
            })?;
            statements.push(statement);
        }
        statements.sort_unstable_by_key(|statement| {
            (
                statement.lane_id,
                statement.dataspace_id,
                statement.lane_incarnation,
                statement.block_height,
            )
        });
        Ok(statements)
    }
}
