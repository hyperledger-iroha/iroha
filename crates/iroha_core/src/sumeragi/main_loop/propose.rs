//! Proposal assembly and pacemaker-driven propose path.

use iroha_logger::prelude::*;
use iroha_primitives::time::TimeSource;

use super::*;

pub(super) fn resolve_prev_block_for_proposal(
    proposal_height: u64,
    highest_qc: &crate::sumeragi::consensus::QcHeaderRef,
    kura: &Kura,
    pending_blocks: &BTreeMap<HashOf<BlockHeader>, PendingBlock>,
) -> Option<Arc<SignedBlock>> {
    let mut prev_block = proposal_height.checked_sub(1).and_then(|prev| {
        let prev_height_usize = if let Ok(value) = usize::try_from(prev) {
            value
        } else {
            warn!(
                height = proposal_height,
                "previous height exceeds usize; skipping cached block lookup"
            );
            return None;
        };
        NonZeroUsize::new(prev_height_usize).and_then(|nz| kura.get_block(nz))
    });
    if prev_block.is_none() && proposal_height > 1 {
        if let Some(pending_parent) = pending_blocks
            .get(&highest_qc.subject_block_hash)
            .filter(|pending| pending.height + 1 == proposal_height)
        {
            prev_block = Some(pending_parent.block.clone().into());
        }
    }
    prev_block
}

fn precommit_qc_for_view_change(
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    committed_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
    let highest_precommit =
        highest_qc.filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
    match (highest_precommit, committed_qc) {
        (Some(highest), Some(committed)) => {
            if (highest.height, highest.view) >= (committed.height, committed.view) {
                Some(highest)
            } else {
                Some(committed)
            }
        }
        (Some(highest), None) => Some(highest),
        (None, Some(committed)) => Some(committed),
        (None, None) => None,
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ViewConversionError {
    U32Overflow,
    USizeOverflow,
}

fn view_to_usize(view: u64) -> Result<usize, ViewConversionError> {
    let view_u32 = u32::try_from(view).map_err(|_| ViewConversionError::U32Overflow)?;
    usize::try_from(view_u32).map_err(|_| ViewConversionError::USizeOverflow)
}

impl Actor {
    pub(super) fn max_tx_budget(
        queue_len: usize,
        block_param_limit: u64,
        config_cap: Option<NonZeroUsize>,
    ) -> (usize, NonZeroUsize) {
        let param_limit = usize::try_from(block_param_limit).unwrap_or_else(|_| {
            warn!(
                block_param_limit,
                "block max transactions exceeds usize; capping to usize::MAX"
            );
            usize::MAX
        });
        let max_tx_target = config_cap
            .map(NonZeroUsize::get)
            .map_or(param_limit, |cfg| cfg.min(param_limit));
        let max_in_block = NonZeroUsize::new(queue_len.min(max_tx_target).max(1))
            .expect("non-zero by construction");
        (max_tx_target, max_in_block)
    }

    pub(super) fn add_heartbeat_if_empty(
        &self,
        proposal_height: u64,
        time_source: &TimeSource,
        tx_batch: &mut Vec<AcceptedTransaction<'static>>,
        routing_batch: &mut Vec<RoutingDecision>,
    ) -> Result<()> {
        if !tx_batch.is_empty() {
            return Ok(());
        }
        let view = self.state.view();
        let params = view.world().parameters();
        let tx_params = params.transaction();
        let max_clock_drift = params.sumeragi().max_clock_drift();
        let crypto_cfg = view.crypto.clone();
        let heartbeat = crate::tx::build_heartbeat_transaction_with_time_source(
            self.common_config.chain.clone(),
            &self.common_config.key_pair,
            &tx_params,
            proposal_height,
            time_source,
        );
        let accepted = AcceptedTransaction::accept_with_time_source(
            heartbeat,
            &self.common_config.chain,
            max_clock_drift,
            tx_params,
            crypto_cfg.as_ref(),
            time_source,
        )
        .map_err(|err| eyre!("failed to accept heartbeat transaction: {err}"))?;
        let routing = crate::queue::evaluate_policy(&view.nexus.routing_policy, &accepted);
        tx_batch.push(accepted);
        routing_batch.push(routing);
        Ok(())
    }

    pub(super) fn drop_stale_pending_block(
        &mut self,
        pending_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<(usize, usize, usize, usize)> {
        let (tx_count, requeued, failures, duplicate_failures) =
            super::drop_pending_block_and_requeue(
                &mut self.pending.pending_blocks,
                pending_hash,
                self.queue.as_ref(),
                self.state.as_ref(),
            )?;

        self.clean_rbc_sessions_for_block(pending_hash, height);
        self.qc_cache
            .retain(|(_, hash, _, _, _), _| hash != &pending_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _), _| hash != &pending_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(height, view);
        self.subsystems
            .propose
            .proposals_seen
            .remove(&(height, view));

        Some((tx_count, requeued, failures, duplicate_failures))
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    pub(super) fn assemble_and_broadcast_proposal(
        &mut self,
        height: u64,
        view: u64,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        topology: &mut super::network_topology::Topology,
        leader_index: usize,
        local_validator_index: u32,
        _now: Instant,
    ) -> Result<bool> {
        if self.is_observer() {
            return Ok(false);
        }
        super::status::set_leader_index(leader_index as u64);
        let required_for_commit = topology.min_votes_for_commit();
        debug!(
            height,
            view,
            topology_size = topology.as_ref().len(),
            required_for_commit,
            "proposal topology snapshot"
        );
        let proposal_height = height;
        let view_usize = match view_to_usize(view) {
            Ok(value) => value,
            Err(ViewConversionError::U32Overflow) => {
                warn!(
                    height,
                    view, "view exceeds u32::MAX; skipping proposal assembly"
                );
                return Ok(false);
            }
            Err(ViewConversionError::USizeOverflow) => {
                warn!(
                    height,
                    view, "view exceeds usize::MAX; skipping proposal assembly"
                );
                return Ok(false);
            }
        };
        self.init_collector_plan(topology, proposal_height, view);
        let prev_block = resolve_prev_block_for_proposal(
            proposal_height,
            &highest_qc,
            &self.kura,
            &self.pending.pending_blocks,
        );
        if prev_block.is_none() && proposal_height > 1 {
            warn!(
                height = proposal_height,
                view,
                parent_height = proposal_height.saturating_sub(1),
                highest_height = highest_qc.height,
                highest_hash = %highest_qc.subject_block_hash,
                "deferring proposal assembly: parent block not available locally"
            );
            return Ok(false);
        }

        let queue_len = self.queue.tx_len();
        let mut tx_guards = Vec::new();
        let (
            _block_digest,
            conf_features,
            mut transactions,
            mut routing_decisions,
            deferred_transactions,
        ) = {
            let state_view = self.state.view();
            let block_max_param = state_view.world().parameters().block().max_transactions();
            let (max_tx_target, max_in_block) = Self::max_tx_budget(
                queue_len,
                block_max_param.get(),
                self.config.block_max_transactions,
            );
            debug!(
                height,
                view,
                queue_len,
                max_tx_param = block_max_param.get(),
                max_tx_target,
                max_in_block = max_in_block.get(),
                "proposal assembly budget"
            );
            let mut lane_consumption: BTreeMap<LaneId, u64> = BTreeMap::new();
            let mut deferred_accumulator: Vec<AcceptedTransaction<'static>> = Vec::new();
            loop {
                let before_fetch = tx_guards.len();
                self.queue
                    .get_transactions_for_block(&state_view, max_in_block, &mut tx_guards);
                let fetched_new = tx_guards.len() > before_fetch;
                if !fetched_new {
                    break;
                }
                let deferred = self.queue.enforce_lane_teu_limits_with_consumption(
                    &mut tx_guards,
                    &mut lane_consumption,
                );
                if !deferred.is_empty() {
                    deferred_accumulator.extend(deferred);
                }
            }
            let committed_height =
                u64::try_from(state_view.height()).expect("committed height exceeds u64::MAX");
            let next_height = committed_height
                .checked_add(1)
                .expect("block height exceeds u64::MAX");
            let digest = compute_confidential_feature_digest(
                state_view.world(),
                &state_view.zk,
                next_height,
            );
            let transactions: Vec<AcceptedTransaction<'static>> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::clone_accepted)
                .collect();
            let routing_decisions: Vec<RoutingDecision> = tx_guards
                .iter()
                .map(crate::queue::TransactionGuard::routing)
                .collect();
            let conf_features = if digest.is_empty() {
                None
            } else {
                Some(digest)
            };
            (
                digest,
                conf_features,
                transactions,
                routing_decisions,
                deferred_accumulator,
            )
        };

        if transactions.len() > 1 {
            let order = interleave_lane_indices(&routing_decisions);

            if order.iter().enumerate().any(|(idx, &value)| idx != value) {
                fn reorder_vec<T: Clone>(vec: &mut Vec<T>, order: &[usize]) {
                    let original = vec.clone();
                    vec.clear();
                    for &idx in order {
                        vec.push(original[idx].clone());
                    }
                }
                reorder_vec(&mut transactions, &order);
                reorder_vec(&mut routing_decisions, &order);
            }
        }

        for tx in deferred_transactions {
            if let Err(err) = self.queue.push(tx, self.state.view()) {
                warn!(?err.err, "failed to requeue transaction deferred by lane TEU limits");
            }
        }

        let queue_len_after_pop = self.queue.tx_len();
        if empty_block_disfavored(
            transactions.len(),
            queue_len_after_pop,
            self.has_nonempty_pending_at_height(proposal_height),
        ) {
            info!(
                height,
                view,
                queue_len = queue_len_after_pop,
                "deferring empty proposal while transactions are queued"
            );
            return Ok(false);
        }

        let da_enabled = self.runtime_da_enabled();
        let mut overflow_transactions = Vec::new();
        let mut tx_batch;
        let mut routing_batch;
        if da_enabled {
            let mut remaining_budget = self
                .config
                .rbc_chunk_max_bytes
                .max(1)
                .saturating_mul(usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize"));
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            for (tx, routing) in transactions.into_iter().zip(routing_decisions.into_iter()) {
                let encoded_len = tx.as_ref().encode().len();
                if encoded_len > remaining_budget {
                    overflow_transactions.push(tx);
                    continue;
                }
                remaining_budget = remaining_budget.saturating_sub(encoded_len);
                tx_batch.push(tx);
                routing_batch.push(routing);
            }
        } else {
            let mut payload_budget = Some(non_rbc_payload_budget(
                self.config.block_max_payload_bytes,
                self.consensus_frame_cap,
            ));
            tx_batch = Vec::with_capacity(transactions.len());
            routing_batch = Vec::with_capacity(routing_decisions.len());
            for (tx, routing) in transactions.into_iter().zip(routing_decisions.into_iter()) {
                if let Some(budget) = payload_budget {
                    let encoded_len = tx.as_ref().encode().len();
                    if encoded_len > budget {
                        overflow_transactions.push(tx);
                        continue;
                    }
                    payload_budget = Some(budget.saturating_sub(encoded_len));
                }
                tx_batch.push(tx);
                routing_batch.push(routing);
            }
        }

        if tx_batch.len() > 1 {
            if let Some(admin_idx) = tx_batch.iter().position(Self::is_peer_admin_transaction) {
                let admin_tx = tx_batch.remove(admin_idx);
                let admin_route = routing_batch.remove(admin_idx);
                overflow_transactions.append(&mut tx_batch);
                tx_batch.push(admin_tx);
                routing_batch.push(admin_route);
            }
        }

        let original_for_requeue = tx_batch.clone();
        let heartbeat_time_source = TimeSource::new_system();
        let mut removed_for_chunk_cap: Vec<AcceptedTransaction<'static>> = Vec::new();
        let mut removed_for_frame_cap: Vec<AcceptedTransaction<'static>> = Vec::new();

        let assembly_result: Result<()> = (|| {
            self.add_heartbeat_if_empty(
                proposal_height,
                &heartbeat_time_source,
                &mut tx_batch,
                &mut routing_batch,
            )?;
            let (
                signed_block,
                block_created_msg,
                payload_bytes,
                payload_hash,
                proposal,
                proposal_hint,
                transactions_for_plan,
                block_hash,
            ) = loop {
                let nexus = self.state.nexus_snapshot();
                let nexus_enabled = nexus.enabled;
                let lane_config = nexus.lane_config.clone();
                let mut builder =
                    BlockBuilder::new(tx_batch.clone()).chain(view_usize, prev_block.as_deref());

                let receipt_plan = if nexus_enabled {
                    let cursor_snapshot = self.state.da_receipt_cursor_snapshot();
                    let (receipts, _cache_outcome) = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        crate::da::receipts::prune_spool(&da_rbc.spool_dir, &cursor_snapshot);
                        da_rbc
                            .spool_cache
                            .load_receipt_entries(&da_rbc.spool_dir)
                            .map_err(|err| eyre!(err))?
                    };
                    #[cfg(feature = "telemetry")]
                    self.telemetry.note_da_spool_cache(
                        crate::telemetry::DaSpoolCacheKind::Receipts,
                        _cache_outcome.as_telemetry(),
                    );
                    crate::da::receipts::plan_committable_receipts(
                        &lane_config,
                        &cursor_snapshot,
                        &self.subsystems.da_rbc.da.sealed_commitments,
                        receipts,
                    )
                    .map_err(|err| eyre!(err))?
                } else {
                    Vec::new()
                };

                let mut bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_commitment_bundle(&da_rbc.spool_dir) {
                        Ok((value, _cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::Commitments,
                                _cache_outcome.as_telemetry(),
                            );
                            value
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                spool = %da_rbc.spool_dir.display(),
                                "failed to load DA commitments from spool; proceeding without DA bundle"
                            );
                            None
                        }
                    }
                };

                if bundle_opt.is_none() && nexus_enabled && !receipt_plan.is_empty() {
                    return Err(eyre!(
                        "DA receipts are present but no commitment records are available in the spool"
                    ));
                }

                if let Some(bundle) = bundle_opt.as_mut() {
                    // Drop commitments that were already sealed to avoid duplication.
                    let filtered = {
                        let da_rbc = &mut self.subsystems.da_rbc;
                        let mut kept = Vec::with_capacity(bundle.commitments.len());
                        for record in &bundle.commitments {
                            let key =
                                iroha_data_model::da::commitment::DaCommitmentKey::from_record(
                                    record,
                                );
                            if da_rbc.da.sealed_commitments.contains(&key) {
                                continue;
                            }
                            let policy = lane_config.manifest_policy(record.lane_id);
                            let (available, _cache_outcome) =
                                crate::sumeragi::main_loop::manifest_available_for_commitment(
                                    &mut da_rbc.manifest_cache,
                                    &da_rbc.spool_dir,
                                    record,
                                    policy,
                                );
                            #[cfg(feature = "telemetry")]
                            self.telemetry
                                .note_da_manifest_cache(_cache_outcome.as_telemetry());
                            if available {
                                kept.push(record.clone());
                            }
                        }
                        kept
                    };
                    bundle.commitments = filtered;

                    if nexus_enabled {
                        if receipt_plan.is_empty() {
                            bundle.commitments.clear();
                        } else {
                            let filtered = crate::da::receipts::align_commitments_for_receipts(
                                &receipt_plan,
                                &bundle.commitments,
                            )
                            .map_err(|err| eyre!(err))?;
                            bundle.commitments = filtered;
                        }
                    }

                    if bundle.is_empty() {
                        bundle_opt = None;
                    } else {
                        self.validate_da_bundle(bundle)?;
                    }

                    if let Some(bundle) = bundle_opt.as_ref() {
                        let shard_cursor_path = crate::da::DaShardCursorJournal::journal_path(
                            &self.subsystems.da_rbc.spool_dir,
                        );
                        let mut shard_journal = match crate::da::DaShardCursorJournal::load(
                            &lane_config,
                            shard_cursor_path.clone(),
                        ) {
                            Ok(journal) => journal,
                            Err(err) => {
                                warn!(
                                    ?err,
                                    path = %shard_cursor_path.display(),
                                    "failed to load DA shard cursor journal; rebuilding from scratch"
                                );
                                crate::da::DaShardCursorJournal::new(
                                    &lane_config,
                                    shard_cursor_path.clone(),
                                )
                            }
                        };

                        if let Err(err) = shard_journal.record_bundle(proposal_height, bundle) {
                            warn!(
                                ?err,
                                "failed to update shard cursors from DA bundle; leaving journal unchanged"
                            );
                        } else if let Err(err) = shard_journal.persist() {
                            warn!(
                                ?err,
                                path = %shard_cursor_path.display(),
                                "failed to persist DA shard cursor journal"
                            );
                        }
                    }
                }

                if let Some(bundle) = bundle_opt {
                    for record in &bundle.commitments {
                        let key =
                            iroha_data_model::da::commitment::DaCommitmentKey::from_record(record);
                        self.subsystems.da_rbc.da.sealed_commitments.insert(key);
                    }
                    self.subsystems
                        .da_rbc
                        .da
                        .da_bundles
                        .insert(proposal_height, bundle.clone());
                    builder = builder.with_da_commitments(Some(bundle));
                }

                let pin_bundle_opt = {
                    let da_rbc = &mut self.subsystems.da_rbc;
                    match da_rbc.spool_cache.load_pin_bundle(&da_rbc.spool_dir) {
                        Ok((value, _cache_outcome)) => {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_spool_cache(
                                crate::telemetry::DaSpoolCacheKind::PinIntents,
                                _cache_outcome.as_telemetry(),
                            );
                            value
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                spool = %da_rbc.spool_dir.display(),
                                "failed to load DA pin intents from spool; proceeding without pin bundle"
                            );
                            None
                        }
                    }
                };

                if let Some(bundle) = pin_bundle_opt {
                    let view = self.state.view();
                    let world = view.world();
                    let account_exists = |account: &iroha_data_model::account::AccountId| -> bool {
                        world.accounts().get(account).is_some()
                    };
                    let (mut intents, rejected) = crate::da::sanitize_pin_intents(
                        bundle.intents,
                        &lane_config,
                        account_exists,
                    );
                    drop(view);
                    if !rejected.is_empty() {
                        for reason in rejected {
                            #[cfg(feature = "telemetry")]
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Dropped,
                                crate::telemetry::PinIntentSpoolReason::from(&reason),
                            );
                            warn!(
                                height = proposal_height,
                                ?reason,
                                "dropping invalid DA pin intent before sealing bundle"
                            );
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    let dedupe_before = intents.len();
                    intents.retain(|intent| {
                        let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
                        !self.subsystems.da_rbc.da.sealed_pin_intents.contains(&key)
                    });
                    #[cfg(feature = "telemetry")]
                    {
                        let deduped = dedupe_before.saturating_sub(intents.len());
                        for _ in 0..deduped {
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Dropped,
                                crate::telemetry::PinIntentSpoolReason::SealedDuplicate,
                            );
                        }
                    }
                    if !intents.is_empty() {
                        let sanitized_bundle = DaPinIntentBundle::new(intents);
                        for intent in &sanitized_bundle.intents {
                            let key = (intent.lane_id.as_u32(), intent.epoch, intent.sequence);
                            self.subsystems.da_rbc.da.sealed_pin_intents.insert(key);
                        }
                        #[cfg(feature = "telemetry")]
                        for _ in &sanitized_bundle.intents {
                            self.telemetry.note_da_pin_intent_spool(
                                crate::telemetry::PinIntentSpoolResult::Kept,
                                crate::telemetry::PinIntentSpoolReason::Kept,
                            );
                        }
                        self.subsystems
                            .da_rbc
                            .da
                            .da_pin_bundles
                            .insert(proposal_height, sanitized_bundle.clone());
                        builder = builder.with_da_pin_intents(Some(sanitized_bundle));
                    }
                }

                let proof_policy_bundle = crate::da::proof_policy_bundle(&lane_config);
                builder = builder.with_da_proof_policies(Some(proof_policy_bundle));

                let new_block = builder
                    .with_confidential_features(conf_features)
                    .sign(self.common_config.key_pair.private_key())
                    .unpack(|event| self.emit_pipeline_event(event));
                let block_created_msg =
                    BlockMessage::BlockCreated(super::message::BlockCreated::from(&new_block));
                let signed_block: SignedBlock = new_block.into();
                let built_height = signed_block.header().height().get();
                if built_height != proposal_height {
                    debug!(
                        expected = proposal_height,
                        actual = built_height,
                        "constructed block height differs from NEW_VIEW target"
                    );
                }
                let block_hash = signed_block.hash();
                let payload_bytes = block_payload_bytes(&signed_block);
                if da_enabled {
                    let total_chunks =
                        rbc::chunk_count(payload_bytes.len(), self.config.rbc_chunk_max_bytes);
                    if total_chunks > usize::try_from(RBC_MAX_TOTAL_CHUNKS).expect("fits in usize")
                    {
                        if tx_batch.len() <= 1 {
                            warn!(
                                height = proposal_height,
                                view,
                                total_chunks,
                                max_chunks = RBC_MAX_TOTAL_CHUNKS,
                                "block payload exceeds RBC chunk cap; unable to assemble proposal"
                            );
                            return Err(eyre!(
                                "proposal payload requires {total_chunks} chunks, exceeding cap {}",
                                RBC_MAX_TOTAL_CHUNKS
                            ));
                        }
                        if let Some(removed_tx) = tx_batch.pop() {
                            let _ = routing_batch.pop();
                            removed_for_chunk_cap.push(removed_tx);
                            continue;
                        }
                    }
                }

                let frame_len = super::consensus_block_wire_len(
                    self.common_config.peer.id(),
                    &block_created_msg,
                );
                if frame_len > self.consensus_frame_cap {
                    if tx_batch.len() <= 1 {
                        warn!(
                            height = proposal_height,
                            view,
                            frame_len,
                            cap = self.consensus_frame_cap,
                            da_enabled,
                            "BlockCreated frame exceeds consensus cap; unable to assemble proposal"
                        );
                        return Err(eyre!(
                            "proposal frame size {frame_len} exceeds consensus cap {}",
                            self.consensus_frame_cap
                        ));
                    }
                    if let Some(removed_tx) = tx_batch.pop() {
                        let _ = routing_batch.pop();
                        removed_for_frame_cap.push(removed_tx);
                        continue;
                    }
                }
                let payload_hash = Hash::new(&payload_bytes);
                let proposal = Self::build_consensus_proposal(
                    &signed_block,
                    payload_hash,
                    highest_qc,
                    local_validator_index,
                    view,
                );
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_proposal(proposal.clone());

                let proposal_hint = super::message::ProposalHint {
                    block_hash,
                    height: proposal_height,
                    view,
                    highest_qc,
                };
                self.subsystems
                    .propose
                    .proposal_cache
                    .insert_hint(proposal_hint);

                break (
                    signed_block,
                    block_created_msg,
                    payload_bytes,
                    payload_hash,
                    proposal,
                    proposal_hint,
                    tx_batch.clone(),
                    block_hash,
                );
            };

            let proposal_epoch = self.epoch_for_height(proposal_height);
            let mut rbc_plan = self.prepare_rbc_plan(rbc::RbcPlanInputs {
                signed_block: &signed_block,
                transactions: &transactions_for_plan,
                routing: &routing_batch,
                payload: &payload_bytes,
                payload_hash,
                height: proposal_height,
                view,
                epoch: proposal_epoch,
                local_validator_index,
            })?;
            drop(payload_bytes);

            if let Some(plan) = rbc_plan.as_ref() {
                // Install RBC sessions up front so local proposal handling sees the session,
                // but defer RBC network traffic until proposal messages are enqueued.
                self.install_rbc_session_plan(&plan.primary)?;
                if let Some(dup) = plan.duplicate.as_ref() {
                    self.install_rbc_session_plan(dup)?;
                }
                self.publish_rbc_backlog_snapshot();
            }

            // Loop back consensus messages locally so the leader participates immediately.
            self.handle_proposal_hint(proposal_hint)?;
            self.handle_proposal(proposal.clone())?;

            let topology_peers = topology.as_ref();
            let local_peer_id = self.common_config.peer.id().clone();
            for peer in topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessage::ProposalHint(proposal_hint),
                });
            }
            for peer in topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessage::Proposal(proposal.clone()),
                });
            }
            for peer in topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: block_created_msg.clone(),
                });
            }

            if let BlockMessage::BlockCreated(block_msg) = block_created_msg.clone() {
                self.handle_block_created(block_msg)?;
            }

            if let Some(plan) = rbc_plan.take() {
                self.broadcast_rbc_session_plan(plan.primary)?;
                if let Some(dup) = plan.duplicate {
                    self.broadcast_rbc_session_plan(dup)?;
                }
            }

            let relay_envelopes = crate::sumeragi::status::lane_relay_envelopes_snapshot();
            if !relay_envelopes.is_empty() {
                self.subsystems.merge.lane_relay.broadcast(relay_envelopes);
            }

            self.record_phase_sample(PipelinePhase::Propose, proposal_height, view);

            let tx_count = transactions_for_plan.len();
            iroha_logger::info!(
                height = proposal_height,
                view,
                tx_count,
                queue_len,
                leader_index,
                block_hash = %block_hash,
                "assembled proposal"
            );

            Ok(())
        })();

        if let Err(err) = assembly_result {
            drop(tx_guards);
            for tx in original_for_requeue {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                if let Err(push_err) = self.queue.push(tx, self.state.view()) {
                    warn!(?push_err.err, "failed to requeue transaction after assembly failure");
                }
            }
            for tx in overflow_transactions {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                if let Err(err) = self.queue.push(tx, self.state.view()) {
                    warn!(?err.err, "failed to requeue transaction overflowed by RBC budget");
                }
            }
            for tx in removed_for_chunk_cap {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                if let Err(err) = self.queue.push(tx, self.state.view()) {
                    warn!(?err.err, "failed to requeue transaction trimmed by RBC chunk cap");
                }
            }
            for tx in removed_for_frame_cap {
                if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                    continue;
                }
                if let Err(err) = self.queue.push(tx, self.state.view()) {
                    warn!(
                        ?err.err,
                        "failed to requeue transaction trimmed by consensus frame cap"
                    );
                }
            }
            return Err(err);
        }

        drop(tx_guards);
        for tx in overflow_transactions {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            if let Err(err) = self.queue.push(tx, self.state.view()) {
                warn!(?err.err, "failed to requeue transaction overflowed by RBC budget");
            }
        }
        for tx in removed_for_chunk_cap {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            if let Err(err) = self.queue.push(tx, self.state.view()) {
                warn!(?err.err, "failed to requeue transaction trimmed by RBC chunk cap");
            }
        }
        for tx in removed_for_frame_cap {
            if crate::tx::is_heartbeat_transaction(tx.as_ref()) {
                continue;
            }
            if let Err(err) = self.queue.push(tx, self.state.view()) {
                warn!(
                    ?err.err,
                    "failed to requeue transaction trimmed by consensus frame cap"
                );
            }
        }

        Ok(true)
    }

    /// Enforce DA proof/commitment caps before embedding them into a block.
    ///
    /// The current `PoR` proof bundle is tracked by commitments only; we bound proof
    /// openings by the same count until proof summaries are threaded through the
    /// consensus path.
    pub(super) fn validate_da_bundle(&mut self, bundle: &DaCommitmentBundle) -> Result<()> {
        let lane_config = self.state.nexus_snapshot().lane_config.clone();
        validate_da_bundle_caps(
            bundle,
            self.config.da_max_commitments_per_block,
            self.config.da_max_proof_openings_per_block,
        )?;

        for record in &bundle.commitments {
            let policy = lane_config.manifest_policy(record.lane_id);
            let (outcome, _cache_outcome) = {
                let da_rbc = &mut self.subsystems.da_rbc;
                manifest_guard_outcome(
                    &mut da_rbc.manifest_cache,
                    &da_rbc.spool_dir,
                    record,
                    policy,
                )
            };
            #[cfg(feature = "telemetry")]
            self.telemetry
                .note_da_manifest_cache(_cache_outcome.as_telemetry());
            match outcome {
                ManifestGuardOutcome::Pass => {}
                ManifestGuardOutcome::Warn(err) => warn!(
                    ?err,
                    ?policy,
                    lane = record.lane_id.as_u32(),
                    epoch = record.epoch,
                    sequence = record.sequence,
                    "audit-only lane missing DA manifest; sealing commitment with warning"
                ),
                ManifestGuardOutcome::Reject(err) => {
                    return Err(eyre!(
                        "DA manifest guard failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    ));
                }
            }

            crate::da::validate_confidential_compute_record(&lane_config, record).map_err(
                |err| {
                    eyre!(
                        "confidential-compute validation failed for lane {} epoch {} seq {}: {err}",
                        record.lane_id.as_u32(),
                        record.epoch,
                        record.sequence
                    )
                },
            )?;
        }

        crate::da::validate_commitment_bundle(bundle, &lane_config)
            .map_err(|err| eyre!("DA commitment bundle failed validation: {err}"))?;

        Ok(())
    }

    pub(super) fn build_consensus_proposal(
        block: &SignedBlock,
        payload_hash: Hash,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
        proposer: u32,
        view: u64,
    ) -> crate::sumeragi::consensus::Proposal {
        let header = block.header();
        let parent_hash = header.prev_block_hash().unwrap_or_else(|| {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
        });
        let tx_root = header
            .merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let state_root = header
            .result_merkle_root()
            .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from);
        let block_height = header.height().get();

        crate::sumeragi::consensus::Proposal {
            header: crate::sumeragi::consensus::ConsensusBlockHeader {
                parent_hash,
                tx_root,
                state_root,
                proposer,
                height: block_height,
                view,
                epoch: highest_qc.epoch,
                highest_qc,
            },
            payload_hash,
        }
    }

    pub(super) fn should_defer_proposal(&mut self) -> bool {
        self.subsystems.propose.backpressure_gate.should_defer()
    }

    pub(super) fn on_pacemaker_backpressure_deferral(
        &mut self,
        now: Instant,
        state: BackpressureState,
    ) {
        super::status::inc_pacemaker_backpressure_deferrals();
        #[cfg(feature = "telemetry")]
        {
            self.telemetry.inc_pacemaker_backpressure_deferrals();
        }
        debug!(
            ?now,
            tx_queue_depth = state.queued(),
            tx_queue_capacity = state.capacity().get(),
            "Pacemaker deferred proposal assembly due to saturated transaction queue"
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_pacemaker_propose_ready(&mut self, now: Instant) -> bool {
        trace!(?now, "pacemaker evaluating NEW_VIEW gating");
        let prev_attempt = self.subsystems.propose.last_pacemaker_attempt.replace(now);
        let view_snapshot = self.state.view();
        let mut topology_peers = self.effective_commit_topology_from_view(&view_snapshot);
        let pending_queue_len = self.queue.tx_len();
        let active_pending = self.active_pending_blocks_len();
        let view_height = view_snapshot.height();
        let committed_height = view_height as u64;
        drop(view_snapshot);

        // Drop NEW_VIEW entries that point at already-committed heights so the pacemaker
        // cannot re-propose a finalized height after a commit.
        self.subsystems
            .propose
            .new_view_tracker
            .prune(committed_height);

        let empty_child_ctx = if pending_queue_len == 0 {
            self.empty_child_fallback(now)
        } else {
            None
        };
        let da_enabled = self.runtime_da_enabled();
        let committed_qc = self.latest_committed_qc();
        let precommit_qc = precommit_qc_for_view_change(self.highest_qc, committed_qc);
        let tracked_height = active_round_height(self.highest_qc, committed_qc, committed_height);

        if topology_peers.is_empty() {
            if let Some(qc) = precommit_qc.as_ref().or(committed_qc.as_ref()) {
                if let Some(roster) = self.roster_from_commit_qc_history_roll_forward(
                    tracked_height,
                    Some(qc.subject_block_hash),
                ) {
                    topology_peers = roster;
                }
            }
        }

        if topology_peers.is_empty() {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    queue_len = pending_queue_len,
                    height = view_height,
                    "deferring proposal: empty commit topology"
                );
            } else {
                trace!("deferring proposal: empty commit topology");
            }
            return false;
        }

        let mut topology = super::network_topology::Topology::new(topology_peers);
        let mut required = topology.min_votes_for_view_change();
        let local_peer_id = self.common_config.peer.id().clone();
        let local_idx = self.local_validator_index_for_topology(&topology);
        let local_peer = local_idx.map(|_| local_peer_id.clone());

        if da_enabled && pending_queue_len == 0 && empty_child_ctx.is_none() {
            trace!(
                da_enabled,
                "DA enabled and transaction queue is empty; proposing heartbeat"
            );
        }

        let online_peers = self.network.online_peers(|peers| peers.iter().count());
        // `online_peers` counts only remote validators; include the local node if it is part of
        // the commit topology so we do not stall when exactly `required` validators are up.
        let online_total = online_peers + usize::from(local_idx.is_some());
        let mut view_age = self.phase_tracker.view_age(tracked_height, now);
        if view_age.is_none() {
            self.phase_tracker.start_new_round(tracked_height, now);
            view_age = self.phase_tracker.view_age(tracked_height, now);
        }
        let current_view = self.phase_tracker.current_view(tracked_height);
        let bootstrap_view = current_view.is_none_or(|view| view == 0);
        if let Some(view) = current_view {
            // Avoid proposing stale views by pruning NEW_VIEW entries below the local view.
            self.subsystems
                .propose
                .new_view_tracker
                .drop_below_view(tracked_height, view);
        }
        if pending_queue_len > 0
            && self
                .subsystems
                .propose
                .propose_attempt_monitor
                .should_log(now)
        {
            let since_last_attempt = prev_attempt.map(|ts| now.saturating_duration_since(ts));
            let since_last_success = self
                .subsystems
                .propose
                .last_successful_proposal
                .map(|ts| now.saturating_duration_since(ts));
            iroha_logger::info!(
                height = view_height,
                view = current_view,
                view_age_ms = view_age.map(|d| d.as_millis()),
                pending_blocks = self.pending.pending_blocks.len(),
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                online_total,
                since_last_attempt_ms = since_last_attempt.map(|d| d.as_millis()),
                since_last_success_ms = since_last_success.map(|d| d.as_millis()),
                "pacemaker attempt with queued transactions"
            );
        }
        let offline_grace = self.commit_quorum_timeout();
        let offline_grace_expired = view_age.is_some_and(|age| age >= offline_grace);
        if online_total < required && !offline_grace_expired {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    queue_len = pending_queue_len,
                    height = tracked_height,
                    required,
                    online_peers,
                    online_total,
                    grace_ms = offline_grace.as_millis(),
                    "deferring proposal: insufficient online peers for commit quorum (within grace)"
                );
            } else {
                trace!(
                    height = tracked_height,
                    required,
                    online_peers,
                    online_total,
                    grace_ms = offline_grace.as_millis(),
                    "deferring proposal: insufficient online peers for commit quorum (within grace)"
                );
            }
            return false;
        } else if online_total < required {
            warn!(
                height = tracked_height,
                required,
                online_peers,
                online_total,
                grace_ms = offline_grace.as_millis(),
                age_ms = view_age.map(|age| age.as_millis()),
                "proceeding with proposal despite online peer count below quorum after grace"
            );
        }

        if required == 1 && topology.as_ref().len() == 1 {
            if let Some(local_peer) = local_peer.as_ref() {
                if let Some(qc) = precommit_qc {
                    // Seed NEW_VIEW tracker so single-validator networks can progress.
                    if self.highest_qc.is_none_or(|current| {
                        let incoming = (qc.height, qc.view);
                        let existing = (current.height, current.view);
                        incoming > existing
                            || (incoming == existing
                                && current.phase != crate::sumeragi::consensus::Phase::Commit)
                    }) {
                        self.highest_qc = Some(qc);
                    }
                    self.subsystems.propose.new_view_tracker.record(
                        qc.height.saturating_add(1),
                        0,
                        local_peer.clone(),
                        qc,
                    );
                }
            }
        }

        if pending_queue_len > 0 {
            iroha_logger::info!(
                queue_len = pending_queue_len,
                topology_len = topology.as_ref().len(),
                required,
                height = view_height,
                "pacemaker evaluating proposal assembly with queued transactions"
            );
            if active_pending > 0 {
                iroha_logger::debug!(
                    height = view_height,
                    pending = active_pending,
                    "pending block already assembled for current slot; waiting for view-change"
                );
            }
        }

        let new_view_summary: Vec<String> = self
            .subsystems
            .propose
            .new_view_tracker
            .entries
            .iter()
            .map(|((h, v), entry)| format!("{h}:{v}={}", entry.senders.len()))
            .collect();
        debug!(
            height = view_height,
            required,
            local_idx = ?local_idx,
            forced = ?self.subsystems.propose.forced_view_after_timeout,
            new_view_slots = ?new_view_summary,
            "pacemaker NEW_VIEW snapshot before selection"
        );

        let mut candidate = self.subsystems.propose.new_view_tracker.select_with_quorum(
            required,
            local_peer.as_ref(),
            topology.as_ref(),
        );
        if pending_queue_len > 0 {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    let should_override = candidate.as_ref().is_none_or(|selection| {
                        selection.key.0 != forced_height || selection.key.1 < forced_view
                    });
                    if forced_height == qc.height.saturating_add(1) && should_override {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        if candidate.is_none() {
            if let Some((forced_height, forced_view)) =
                self.subsystems.propose.forced_view_after_timeout
            {
                if let Some(qc) = precommit_qc {
                    if forced_height == qc.height.saturating_add(1) {
                        candidate = Some(NewViewSelection {
                            key: (forced_height, forced_view),
                            quorum: required,
                            highest_qc: qc,
                        });
                        self.subsystems.propose.forced_view_after_timeout = None;
                    }
                }
            }
        }

        if let Some(ref ctx) = empty_child_ctx {
            if candidate
                .as_ref()
                .is_none_or(|selection| selection.key.0 <= ctx.highest_qc.height)
            {
                candidate = Some(NewViewSelection {
                    key: (ctx.target_height, ctx.target_view),
                    quorum: required,
                    highest_qc: ctx.highest_qc,
                });
            }
        }

        let Some(selection) = candidate.or_else(|| {
            // Fallback: bootstrap the first view using the latest committed QC when no NEW_VIEW
            // quorum has been observed yet. This prevents the pacemaker from stalling indefinitely
            // at startup before any view changes occur.
            if !bootstrap_view {
                return None;
            }
            let _local_idx = local_idx?;
            let qc = self.latest_committed_qc()?;
            Some(NewViewSelection {
                key: (qc.height.saturating_add(1), 0),
                quorum: required,
                highest_qc: qc,
            })
        }) else {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    queue_len = pending_queue_len,
                    height = view_height,
                    required,
                    local_idx = ?local_idx,
                    new_view_slots = ?new_view_summary,
                    "deferring proposal: awaiting NEW_VIEW quorum"
                );
            } else {
                debug!(
                    required,
                    local_idx = ?local_idx,
                    new_view_slots = ?new_view_summary,
                    "deferring proposal: awaiting NEW_VIEW quorum"
                );
            }
            return false;
        };

        let (height, view_idx) = selection.key;
        let quorum = selection.quorum;
        let mut highest_qc = selection.highest_qc;
        let empty_child_selected = empty_child_ctx
            .as_ref()
            .is_some_and(|ctx| ctx.target_height == height && ctx.target_view == view_idx);

        debug!(
            height,
            view = view_idx,
            quorum,
            local_idx = ?local_idx,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            new_view_slots = ?new_view_summary,
            "selected NEW_VIEW candidate"
        );
        if empty_child_selected {
            if let Some(ctx) = empty_child_ctx.as_ref() {
                iroha_logger::info!(
                    height = ctx.target_height,
                    view = ctx.target_view,
                    parent = %ctx.highest_qc.subject_block_hash,
                    payload = %ctx.parent_payload_hash,
                    "selected empty-child fallback to finalize locked parent"
                );
            }
        }

        let epoch = self.epoch_for_height(height);
        let precommit_votes_at_view = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.height == height
                    && vote.view == view_idx
                    && vote.epoch == epoch
                    && self.block_known_locally(vote.block_hash)
            })
            .count();
        if precommit_votes_at_view > 0 {
            iroha_logger::info!(
                height,
                view = view_idx,
                precommit_votes = precommit_votes_at_view,
                queue_len = pending_queue_len,
                "deferring proposal: precommit votes already observed for this view"
            );
            return false;
        }

        // Avoid rebuilding multiple proposals for the same (height, view) slot. Reassembly in the
        // same view causes double-voting and scatters QC collection; wait for the existing
        // proposal to gather votes or transition via a view change instead.
        if self
            .subsystems
            .propose
            .proposal_cache
            .get_proposal(height, view_idx)
            .is_some()
        {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already cached for this slot; waiting for votes/view change"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already cached for this slot; deferring reassembly"
                );
            }
            return false;
        }

        if self
            .subsystems
            .propose
            .proposals_seen
            .contains(&(height, view_idx))
        {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already observed for this slot; waiting for progress"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "proposal already observed for this slot; deferring reassembly"
                );
            }
            return false;
        }

        if let Some((pending_age, pending_view)) = self
            .pending
            .pending_blocks
            .values()
            .filter(|pending| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|pending| (pending.age(), pending.view))
            .min_by_key(|(age, _)| *age)
        {
            let quorum_timeout = self.quorum_timeout(da_enabled);
            if quorum_timeout != Duration::ZERO && pending_age < quorum_timeout {
                iroha_logger::info!(
                    height,
                    pending_view,
                    target_view = view_idx,
                    age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    queue_len = pending_queue_len,
                    "deferring proposal: pending block still within quorum timeout window"
                );
                return false;
            }
        }

        if let Some((pending_hash, pending_parent)) = self
            .pending
            .pending_blocks
            .iter()
            .find(|(_, pending)| {
                !pending.aborted && pending.height == height && pending.view == view_idx
            })
            .map(|(hash, pending)| (*hash, pending.block.header().prev_block_hash()))
        {
            let view = self.state.view();
            let committed_height = view.height();
            let committed_hash = view.latest_block_hash();
            drop(view);

            if pending_block_stale_for_tip(height, pending_parent, committed_height, committed_hash)
            {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    pending_hash = %pending_hash,
                    pending_parent = ?pending_parent,
                    committed_hash = ?committed_hash,
                    "dropping stale pending proposal that no longer builds on committed chain"
                );
                if let Some((tx_count, requeued, failures, duplicate_failures)) =
                    self.drop_stale_pending_block(pending_hash, height, view_idx)
                {
                    if tx_count > 0 {
                        iroha_logger::info!(
                            height,
                            view = view_idx,
                            tx_count,
                            requeued,
                            failures,
                            duplicate_failures,
                            "requeued transactions from stale pending proposal"
                        );
                    }
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "proposal already pending for this slot; deferring reassembly"
                );
                return false;
            }
        }

        if self.highest_qc.is_none_or(|current| {
            (highest_qc.height, highest_qc.view) > (current.height, current.view)
        }) {
            self.highest_qc = Some(highest_qc);
        }

        if let Err(reason) = ensure_locked_qc_allows(self.locked_qc, highest_qc) {
            match reason {
                LockedQcRejection::HeightRegressed { locked, highest } => {
                    let Some(lock) = self.locked_qc else {
                        return false;
                    };
                    iroha_logger::info!(
                        locked_height = locked,
                        highest_height = highest,
                        height,
                        view = view_idx,
                        queue_len = pending_queue_len,
                        "deferring proposal: highest QC lags locked QC; retaining lock"
                    );
                    highest_qc = lock;
                }
                LockedQcRejection::HashMismatch { .. } => {
                    let locked_hash = self.locked_qc.map(|qc| qc.subject_block_hash);
                    let locked_missing =
                        locked_hash.is_some_and(|hash| !self.block_known_locally(hash));
                    if locked_missing {
                        iroha_logger::warn!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            locked_hash = ?locked_hash,
                            "clearing locked QC that is missing from kura"
                        );
                        self.locked_qc = Some(highest_qc);
                    } else {
                        iroha_logger::info!(
                            ?reason,
                            height,
                            view = view_idx,
                            queue_len = pending_queue_len,
                            "deferring proposal: locked QC prevents proposal"
                        );
                        return false;
                    }
                }
            }
        }

        if !self.highest_qc_extends_locked(highest_qc) {
            if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    info!(
                        height,
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        "resetting locked QC to committed chain to unblock proposal"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
            }
        }

        self.maybe_realign_locked_to_committed_tip();

        if !self.highest_qc_extends_locked(highest_qc) {
            if let Some(lock) = self.locked_qc
                && (highest_qc.height, highest_qc.view) <= (lock.height, lock.view)
            {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = %highest_qc.subject_block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    queue_len = pending_queue_len,
                    "replacing non-extending highest QC with locked QC"
                );
                highest_qc = lock;
            } else if let Some(new_lock) = realign_locked_to_committed_if_extends(
                self.locked_qc,
                self.latest_committed_qc(),
                highest_qc,
                |hash, height| self.parent_hash_for(hash, height),
            ) {
                if self.locked_qc != Some(new_lock) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        locked_height = new_lock.height,
                        locked_hash = %new_lock.subject_block_hash,
                        highest_height = highest_qc.height,
                        highest_hash = %highest_qc.subject_block_hash,
                        queue_len = pending_queue_len,
                        "realigning locked QC to committed chain for proposal assembly"
                    );
                    self.locked_qc = Some(new_lock);
                    super::status::set_locked_qc(
                        new_lock.height,
                        new_lock.view,
                        Some(new_lock.subject_block_hash),
                    );
                }
                if !self.highest_qc_extends_locked(highest_qc) {
                    iroha_logger::info!(
                        height,
                        view = view_idx,
                        highest_height = highest_qc.height,
                        highest_hash = ?highest_qc.subject_block_hash,
                        locked_height = ?self.locked_qc.map(|qc| qc.height),
                        locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                        queue_len = pending_queue_len,
                        "deferring proposal: highest QC does not extend locked chain"
                    );
                    return false;
                }
            } else {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    highest_height = highest_qc.height,
                    highest_hash = ?highest_qc.subject_block_hash,
                    locked_height = ?self.locked_qc.map(|qc| qc.height),
                    locked_hash = ?self.locked_qc.map(|qc| qc.subject_block_hash),
                    queue_len = pending_queue_len,
                    "deferring proposal: highest QC does not extend locked chain"
                );
                return false;
            }
        }

        let proposal_roster = self
            .roster_from_commit_qc_history_roll_forward(height, Some(highest_qc.subject_block_hash))
            .unwrap_or_else(|| self.effective_commit_topology());
        if proposal_roster.is_empty() {
            warn!(
                height,
                view = view_idx,
                "deferring proposal: empty commit topology for selected height"
            );
            return false;
        }
        topology = super::network_topology::Topology::new(proposal_roster);
        required = topology.min_votes_for_view_change();

        let leader_index = match self.leader_index_for(&mut topology, height, view_idx) {
            Ok(idx) => idx,
            Err(err) => {
                warn!(
                    ?err,
                    height,
                    view = view_idx,
                    "failed to compute leader index"
                );
                return false;
            }
        };

        let Some(local_pos) = topology.position(self.common_config.peer.id().public_key()) else {
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    queue_len = pending_queue_len,
                    "deferring proposal: local node not part of validator set"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "local node not part of validator set"
                );
            }
            return false;
        };
        if local_pos != leader_index {
            let leader_peer = topology.iter().next().cloned();
            if pending_queue_len > 0 {
                iroha_logger::info!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    queue_len = pending_queue_len,
                    "deferring proposal: local node is not leader for this round"
                );
            } else {
                trace!(
                    height,
                    view = view_idx,
                    local_idx = local_pos,
                    leader_index,
                    leader = ?leader_peer,
                    "local node is not leader for this round"
                );
            }
            return false;
        }

        let Ok(local_idx_val) = u32::try_from(local_pos) else {
            warn!(local_pos, "local validator index exceeds u32 limits");
            return false;
        };

        debug!(
            height,
            view = view_idx,
            quorum,
            required,
            leader_index,
            "NEW_VIEW quorum satisfied; assembling proposal"
        );

        let leader_peer = topology.iter().next().cloned();

        iroha_logger::info!(
            height,
            view = view_idx,
            leader_index,
            leader = ?leader_peer,
            local_idx = local_pos,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            "starting proposal assembly"
        );

        if empty_child_selected {
            if let Some(ctx) = empty_child_ctx.as_ref() {
                self.record_empty_child_attempt(ctx.parent_payload_hash, now);
            }
        }

        let assembled = match self.assemble_and_broadcast_proposal(
            height,
            view_idx,
            highest_qc,
            &mut topology,
            leader_index,
            local_idx_val,
            now,
        ) {
            Ok(assembled) => assembled,
            Err(err) => {
                warn!(?err, height, view = view_idx, "failed to assemble proposal");
                return false;
            }
        };

        if !assembled {
            return false;
        }

        iroha_logger::info!(
            height,
            view = view_idx,
            local_idx = local_idx_val,
            leader_index,
            quorum,
            highest_height = highest_qc.height,
            highest_view = highest_qc.view,
            highest_hash = %highest_qc.subject_block_hash,
            "proposal assembly succeeded"
        );

        self.subsystems
            .propose
            .new_view_tracker
            .remove(height, view_idx);
        self.subsystems.propose.last_successful_proposal = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{ViewConversionError, view_to_usize};

    #[test]
    fn view_to_usize_rejects_u32_overflow() {
        let view = u64::from(u32::MAX) + 1;
        assert_eq!(view_to_usize(view), Err(ViewConversionError::U32Overflow));
    }

    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    #[test]
    fn view_to_usize_accepts_u32_max() {
        let view = u64::from(u32::MAX);
        assert_eq!(view_to_usize(view), Ok(u32::MAX as usize));
    }
}
