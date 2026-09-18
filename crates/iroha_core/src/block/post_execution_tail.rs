// One ordinary sequential/DAG suffix; source-specific authorization stays in callers.
// TODO: this extraction neither moves recorder start before native construction nor
// removes native witness suppression. Native execution/publication remains inactive.

type TailBatchOutcomes = BTreeMap<
    HashOf<TransactionEntrypoint>,
    Vec<iroha_data_model::events::data::prelude::AssetBatchTransferOutcome>,
>;

/// Attach executor-owned receipt rows without clearing already-complete outputs.
/// Validate every row before changing any result. No display-hash inference occurs here.
fn assign_tail_batch_outcomes(
    results: &mut [iroha_data_model::transaction::signed::TransactionResult],
    owners: &BTreeMap<HashOf<TransactionEntrypoint>, usize>,
    outcomes: TailBatchOutcomes,
) -> Result<(), String> {
    let mut assigned = BTreeSet::new();
    for (owner, receipts) in &outcomes {
        let index = owners
            .get(owner)
            .ok_or_else(|| "batch receipt has no exact executed owner".to_owned())?;
        let result = results
            .get(*index)
            .ok_or_else(|| "batch receipt result position is out of range".to_owned())?;
        if !assigned.insert(*index) {
            return Err("batch receipts repeat one result position".into());
        }
        if receipts.is_empty() || !result.batch_transfer_outcomes().is_empty() {
            return Err("batch receipt competes with an existing output or is empty".into());
        }
    }
    for (owner, receipts) in outcomes {
        results[owners[&owner]].set_batch_transfer_outcomes(receipts);
    }
    Ok(())
}

/// Prefix identities are real network execution calls; reveal receipts use their
/// inner call. Canonical outer/inner uniqueness preserves the existing receipt gate.
fn prefix_tail_receipt_owners(
    entries: &[TransactionEntrypoint],
) -> Result<BTreeMap<HashOf<TransactionEntrypoint>, usize>, String> {
    let mut identities = BTreeSet::new();
    for entry in entries {
        if !identities.insert(entry.hash()) {
            return Err("network prefix repeats a canonical receipt identity".into());
        }
    }
    let mut owners = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let call = entry.execution_call_hash();
        if matches!(entry, TransactionEntrypoint::SealedReveal(_)) && !identities.insert(call) {
            return Err("network prefix repeats a sealed receipt alias".into());
        }
        if owners.insert(call, index).is_some() {
            return Err("network prefix repeats an executed receipt owner".into());
        }
    }
    Ok(owners)
}

/// Join only the actual returned Time invocation hashes to the actual returned
/// result positions. Equal display entrypoints are valid and remain separate leaves.
fn join_time_tail_receipts(
    prefix_owners: &BTreeMap<HashOf<TransactionEntrypoint>, usize>,
    entries: &[iroha_data_model::trigger::TimeTriggerEntrypoint],
    display_hashes: &[HashOf<TransactionEntrypoint>],
    results: Vec<TransactionResultInner>,
    calls: &[Hash],
    outcomes: TailBatchOutcomes,
) -> Result<Vec<iroha_data_model::transaction::signed::TransactionResult>, String> {
    if entries.len() != display_hashes.len()
        || entries.len() != results.len()
        || entries.len() != calls.len()
    {
        return Err("Time execution vectors do not have identical positions".into());
    }
    let mut owners = BTreeMap::new();
    for (index, ((entry, display), call)) in
        entries.iter().zip(display_hashes).zip(calls).enumerate()
    {
        if entry.hash_as_entrypoint() != *display {
            return Err("Time display hash differs from its executed entrypoint".into());
        }
        let owner = HashOf::from_untyped_unchecked(*call);
        if prefix_owners.contains_key(&owner) || owners.insert(owner, index).is_some() {
            return Err("Time invocation repeats an already executed receipt owner".into());
        }
    }
    let mut full = results
        .into_iter()
        .map(iroha_data_model::transaction::signed::TransactionResult::from)
        .collect::<Vec<_>>();
    assign_tail_batch_outcomes(&mut full, &owners, outcomes)?;
    Ok(full)
}

impl ValidBlock {
    /// Complete the ordinary post-execution suffix on its original overlay.
    /// Return timers so each caller retains its source-specific finalization timing.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn finalize_ordinary_execution_tail(
        block: &mut SignedBlock,
        state_block: &mut StateBlock<'_>,
        mut hashes: Vec<HashOf<TransactionEntrypoint>>,
        mut results: Vec<iroha_data_model::transaction::signed::TransactionResult>,
        transaction_event_hashes: &[Option<HashOf<SignedTransaction>>],
        routing: &[crate::queue::RoutingDecision],
        advertised_fragments: Option<u64>,
        advertised_axt_policy: Option<&AxtPolicySnapshot>,
        advertised_axt_transitions: Option<&BTreeSet<DataSpaceId>>,
        mut timings: Option<&mut ValidationTimings>,
    ) -> Result<(Option<Instant>, Option<Instant>), BlockValidationError> {
        let to_ms = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        // No parallel native accepting path: eventual source cutover must supply
        // its private full prefix and complete witness/publication authorization.
        if block
            .execution_context()
            .is_some_and(|bundle| bundle.native_lane_decisions.is_some())
        {
            return Err(Self::execution_context_error(
                "native execution is not active in the ordinary tail",
            ));
        }
        let entries = block.external_entrypoints_slice();
        if results.len() != entries.len()
            || routing.len() != entries.len()
            || transaction_event_hashes.len() != entries.len()
            || !entries
                .iter()
                .map(TransactionEntrypoint::hash)
                .eq(hashes.iter().copied())
        {
            return Err(BlockValidationError::MerkleRootMismatch);
        }
        let prefix_owners = prefix_tail_receipt_owners(entries)
            .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
        // Seal prefix receipt ownership before independent pipeline/Time work.
        assign_tail_batch_outcomes(
            &mut results,
            &prefix_owners,
            state_block.drain_batch_transfer_outcomes(),
        )
        .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
        Self::execute_deterministic_pipeline_triggers(
            block,
            state_block,
            transaction_event_hashes,
            &results,
            routing,
        )?;
        let time_start = timings.as_ref().map(|_| Instant::now());
        let (time_entries, mut time_hashes, time_results, time_calls) =
            state_block.execute_time_triggers(&block.header());
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), time_start) {
            timings.execution_tx_time_triggers_ms = to_ms(start.elapsed());
        }
        let pruned = crate::tx::prune_expired_sealed_commitments(state_block);
        if pruned > 0 {
            iroha_logger::debug!(
                count = pruned,
                "pruned expired sealed transaction commitments"
            );
        }
        let mut full_time = join_time_tail_receipts(
            &prefix_owners,
            &time_entries,
            &time_hashes,
            time_results,
            &time_calls,
            state_block.drain_batch_transfer_outcomes(),
        )
        .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
        let finalize_start = timings.as_ref().map(|_| Instant::now());
        let digest_start = timings.as_ref().map(|_| Instant::now());
        let pending = state_block.submit_transfer_transcript_digest_batch();
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), digest_start) {
            timings.execution_tx_finalize_digest_submit_ms = to_ms(start.elapsed());
        }
        hashes.append(&mut time_hashes);
        results.append(&mut full_time);
        let tx_set_start = timings.as_ref().map(|_| Instant::now());
        let time_inputs = time_entries
            .iter()
            .cloned()
            .map(TransactionEntrypoint::Time)
            .collect::<Vec<_>>();
        let tx_set = iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
            entries.iter().chain(time_inputs.iter()),
        )
        .map_err(|error| {
            Self::execution_context_error(format!(
                "FASTPQ canonical transaction-wire commitment failed: {error}"
            ))
        })?
        .into();
        state_block.set_fastpq_tx_set_hash(tx_set);
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), tx_set_start) {
            timings.execution_tx_finalize_tx_set_ms = to_ms(start.elapsed());
        }
        let inventory_start = timings.as_ref().map(|_| Instant::now());
        state_block
            .finalize_fastpq_source_inventory_with_pending(entries, routing, &time_calls, pending)
            .map_err(Self::execution_context_error)?;
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), inventory_start) {
            timings.execution_tx_finalize_dataspaces_ms = to_ms(start.elapsed());
        }
        let transcripts_start = timings.as_ref().map(|_| Instant::now());
        let transcripts = state_block.drain_transfer_transcripts_with_pending(None);
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), transcripts_start) {
            timings.execution_tx_finalize_transcripts_ms = to_ms(start.elapsed());
        }
        let axt_start = timings.as_ref().map(|_| Instant::now());
        let envelopes = state_block.drain_axt_envelopes();
        let fragments =
            Self::validated_committed_fragment_count(state_block, advertised_fragments)?;
        state_block
            .finalize_axt_asset_incarnations()
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to finalize AXT asset incarnations: {error}"
                ))
            })?;
        state_block
            .evaluate_nexus_autoscale(block, fragments)
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to evaluate Nexus autoscale: {error}"
                ))
            })?;
        state_block
            .finalize_axt_policy_transition_ratchets()
            .map_err(|error| {
                Self::execution_context_error(format!(
                    "failed to finalize the AXT policy counter ratchet: {error}"
                ))
            })?;
        let policy = state_block.axt_policy_snapshot();
        Self::validate_advertised_axt_post_state(advertised_axt_policy, &policy)?;
        let transitions = state_block.axt_authorization_transitioned().clone();
        Self::validate_advertised_axt_transitions(
            advertised_axt_transitions,
            &transitions,
            policy.version,
        )?;
        let completions = state_block.world.trigger_completions();
        if let (Some(timings), Some(start)) = (timings.as_deref_mut(), axt_start) {
            timings.execution_tx_finalize_axt_ms = to_ms(start.elapsed());
        }
        let set_results_start = timings.as_ref().map(|_| Instant::now());
        block
            .set_full_transaction_results_with_transcripts(
                time_entries,
                &hashes,
                results,
                fragments,
                transcripts,
                envelopes,
                policy,
            )
            .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
        block
            .set_axt_transitioned_dataspaces(transitions)
            .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
        block.set_trigger_completions(completions);
        Ok((finalize_start, set_results_start))
    }
}

/// Test the real common tail without granting a source-specific ValidBlock or commit.
#[cfg(test)]
pub(crate) fn finish_ordinary_tail_for_test(
    block: &mut SignedBlock,
    state_block: &mut StateBlock<'_>,
    prefix_results: Vec<iroha_data_model::transaction::signed::TransactionResult>,
    routes: &[crate::queue::RoutingDecision],
) -> Result<(), BlockValidationError> {
    let hashes = block
        .external_entrypoints_slice()
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect();
    let events = block
        .external_entrypoints_slice()
        .iter()
        .map(ValidBlock::signed_transaction_from_entrypoint)
        .map(|transaction| transaction.map(SignedTransaction::hash))
        .collect::<Vec<_>>();
    ValidBlock::finalize_ordinary_execution_tail(
        block,
        state_block,
        hashes,
        prefix_results,
        &events,
        routes,
        None,
        None,
        None,
        None,
    )
    .map(|_| ())
}
