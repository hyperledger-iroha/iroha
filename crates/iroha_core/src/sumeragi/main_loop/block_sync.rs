//! Block sync and missing-block request handlers.

use iroha_logger::prelude::*;

use super::*;

impl Actor {
    fn send_fetch_pending_block_response(&mut self, peer: PeerId, mut msg: BlockMessage) {
        if let BlockMessage::BlockSyncUpdate(update) = &mut msg {
            let block_hash = update.block.hash();
            let height = update.block.header().height().get();
            let view = update.block.header().view_change_index();
            let expected_epoch = self.epoch_for_height(height);
            Self::apply_cached_qcs_to_block_sync_update(
                update,
                &self.qc_cache,
                &self.vote_log,
                block_hash,
                height,
                view,
                expected_epoch,
                self.state.as_ref(),
                self.config.consensus_mode,
            );
            if !self.trim_block_sync_update_for_frame_cap(update) {
                let fallback = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: update.block.clone(),
                });
                let fallback_len =
                    super::consensus_block_wire_len(self.common_config.peer.id(), &fallback);
                if fallback_len > self.consensus_payload_frame_cap {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        cap = self.consensus_payload_frame_cap,
                        fallback_len,
                        "dropping oversized block sync response; BlockCreated still exceeds cap"
                    );
                    return;
                }
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    cap = self.consensus_payload_frame_cap,
                    fallback_len,
                    "block sync response exceeds frame cap; sending BlockCreated instead"
                );
                self.schedule_background(BackgroundRequest::Post {
                    peer,
                    msg: fallback,
                });
                return;
            }
        }
        self.schedule_background(BackgroundRequest::Post { peer, msg });
    }

    fn send_fetch_pending_block_rbc_init(&mut self, peer: PeerId, block: &SignedBlock) {
        if !self.runtime_da_enabled() {
            return;
        }
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let key = Self::session_key(&block_hash, height, view);
        let init = self
            .rebuild_rbc_init(key)
            .or_else(|| self.rebuild_rbc_init_from_block(block, key));
        let Some(init) = init else {
            return;
        };
        // Send RBC INIT alongside missing-block responses so peers can process READY/DELIVER.
        self.send_fetch_pending_block_response(peer, BlockMessage::RbcInit(init));
    }

    pub(super) fn should_drop_future_block_sync_update(
        &self,
        block_hash: &HashOf<BlockHeader>,
        parent_hash: Option<HashOf<BlockHeader>>,
        height: u64,
        view: u64,
        requested_missing_block: bool,
    ) -> bool {
        if requested_missing_block || self.block_known_locally(*block_hash) {
            return false;
        }
        if parent_hash.is_some_and(|hash| self.block_payload_available_locally(hash)) {
            return false;
        }
        self.should_drop_future_consensus_message(height, view, "BlockSyncUpdate")
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let super::message::BlockSyncUpdate {
            block,
            commit_votes,
            commit_qc: incoming_qc,
            validator_checkpoint,
            stake_snapshot,
        } = update;
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let parent_hash = block.header().prev_block_hash();
        let mut requested_missing_block = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        if self.should_drop_future_block_sync_update(
            &block_hash,
            parent_hash,
            block_height,
            block_view,
            requested_missing_block,
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            if let Some(parent_hash) = parent_hash {
                let local_height = {
                    let view = self.state.view();
                    u64::try_from(view.height()).unwrap_or(u64::MAX)
                };
                let expected_height = local_height.saturating_add(1);
                let commit_topology = self.effective_commit_topology();
                let expected_usize = usize::try_from(expected_height).ok();
                let actual_usize = usize::try_from(block_height).ok();
                self.request_missing_parent(
                    block_hash,
                    block_height,
                    block_view,
                    parent_hash,
                    &commit_topology,
                    None,
                    expected_usize,
                    actual_usize,
                    "block_sync_future_window",
                );
                if block_height > expected_height.saturating_add(1) {
                    self.request_missing_parents_for_gap(
                        &commit_topology,
                        None,
                        "block_sync_future_gap",
                    );
                }
            }
            return Ok(());
        }
        if let Some(local_view) = self.stale_view(block_height, block_view) {
            let da_enabled = self.runtime_da_enabled();
            if !da_enabled && !requested_missing_block {
                debug!(
                    height = block_height,
                    view = block_view,
                    local_view,
                    kind = "BlockSyncUpdate",
                    "dropping consensus message for stale view"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StaleView,
                );
                return Ok(());
            }
            debug!(
                height = block_height,
                view = block_view,
                local_view,
                da_enabled,
                missing_request = requested_missing_block,
                "accepting BlockSyncUpdate for stale view"
            );
        }
        if let Ok(height_usize) = usize::try_from(block_height)
            && let Some(nz_height) = NonZeroUsize::new(height_usize)
            && let Some(committed) = self.kura.get_block(nz_height)
        {
            let committed_hash = committed.hash();
            if committed_hash != block_hash {
                info!(
                    committed_height = height_usize,
                    committed_hash = %committed_hash,
                    incoming_hash = %block_hash,
                    "dropping block sync update that conflicts with committed block"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::CommitConflict,
                );
                self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
                return Ok(());
            }
        }
        let block_known = self.kura.get_block_height_by_hash(block_hash).is_some();
        let has_roster_hint = incoming_qc.is_some()
            || validator_checkpoint.is_some()
            || stake_snapshot.is_some()
            || !commit_votes.is_empty();
        if block_known && !has_roster_hint {
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        let (consensus_mode, mode_tag, prf_seed, local_height) = {
            let view = self.state.view();
            let consensus_mode = super::effective_consensus_mode_for_height(
                &view,
                block_height,
                self.config.consensus_mode,
            );
            let mode_tag = match consensus_mode {
                ConsensusMode::Permissioned => PERMISSIONED_TAG,
                ConsensusMode::Npos => NPOS_TAG,
            };
            let prf_seed = Some(super::prf_seed_for_height(&view, block_height));
            let local_height = u64::try_from(view.height()).unwrap_or(u64::MAX);
            (consensus_mode, mode_tag, prf_seed, local_height)
        };
        if self.runtime_da_enabled()
            && !requested_missing_block
            && !self.block_payload_available_locally(block_hash)
            && block_height <= local_height.saturating_add(1)
        {
            requested_missing_block = true;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        let has_commit_votes = !commit_votes.is_empty();
        let mut commit_votes = Some(commit_votes);
        let mut process_commit_votes = |actor: &mut Actor| {
            let Some(commit_votes) = commit_votes.take() else {
                return;
            };
            let mut dropped_votes = 0usize;
            for vote in commit_votes {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.block_hash != block_hash
                    || vote.height != block_height
                    || vote.view != block_view
                    || vote.epoch != expected_epoch
                {
                    dropped_votes = dropped_votes.saturating_add(1);
                    continue;
                }
                actor.handle_vote(vote);
            }
            if dropped_votes > 0 {
                debug!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    dropped_votes,
                    "dropping mismatched commit votes from block sync update"
                );
            }
        };
        let persisted_roster = persisted_roster_for_block(
            self.state.as_ref(),
            &self.kura,
            consensus_mode,
            block_height,
            block_hash,
            Some(block_view),
            &self.roster_validation_cache,
        );
        let cert_hint = incoming_qc.as_ref();
        let checkpoint_hint = validator_checkpoint.as_ref();
        // Allow NPoS bootstrap to accept next-height block sync updates without roster artifacts.
        let allow_uncertified = match consensus_mode {
            ConsensusMode::Permissioned => {
                block_height == local_height.saturating_add(1) || requested_missing_block
            }
            ConsensusMode::Npos => {
                block_height == local_height.saturating_add(1) || requested_missing_block
            }
        };
        let Some(selection) = select_block_sync_roster(
            &block,
            block_hash,
            block_height,
            persisted_roster,
            cert_hint,
            checkpoint_hint,
            stake_snapshot.as_ref(),
            self.state.as_ref(),
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            consensus_mode,
            mode_tag,
            allow_uncertified,
            &self.roster_validation_cache,
        ) else {
            if block_known
                && cert_hint.is_none()
                && checkpoint_hint.is_none()
                && stake_snapshot.is_none()
                && has_commit_votes
            {
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "processing commit votes without roster hints for known block"
                );
                process_commit_votes(self);
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
                return Ok(());
            }
            let roster_snapshot = self
                .state
                .commit_roster_snapshot_for_block(block_height, block_hash)
                .is_some();
            warn!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                cert_hint = cert_hint.is_some(),
                checkpoint_hint = checkpoint_hint.is_some(),
                requested_missing_block,
                roster_snapshot,
                "dropping block sync update: no verifiable roster available"
            );
            super::status::inc_block_sync_drop_invalid_signatures();
            super::status::inc_block_sync_roster_drop_missing();
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterMissing,
            );
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.note_block_sync_roster_drop("missing");
            }
            if block_known {
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
            }
            return Ok(());
        };
        super::status::inc_block_sync_roster_source(selection.source.as_str());
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.note_block_sync_roster_source(selection.source.as_str());
        }
        info!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            source = selection.source.as_str(),
            "block sync roster selected"
        );
        self.cache_vote_roster(
            block_hash,
            block_height,
            block_view,
            selection.roster.clone(),
        );
        let topology = super::network_topology::Topology::new(selection.roster.clone());
        if let Some(checkpoint) = selection.checkpoint.clone() {
            super::status::record_validator_checkpoint(checkpoint);
        }
        // Persist commit rosters only once the block is known locally.
        let commit_roster_record = selection.commit_qc.as_ref().map(|cert| {
            let checkpoint = selection.checkpoint.clone().unwrap_or_else(|| {
                ValidatorSetCheckpoint::new(
                    cert.height,
                    cert.view,
                    cert.subject_block_hash,
                    cert.parent_state_root,
                    cert.post_state_root,
                    cert.validator_set.clone(),
                    cert.aggregate.signers_bitmap.clone(),
                    cert.aggregate.bls_aggregate_signature.clone(),
                    cert.validator_set_hash_version,
                    None,
                )
            });
            (cert.clone(), checkpoint, selection.stake_snapshot.clone())
        });
        if block_known {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            process_commit_votes(self);
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        let had_incoming_qc = incoming_qc.is_some();
        let block_signers_result = {
            let state_view = self.state.view();
            validated_block_signers(&block, &topology, &state_view, mode_tag, prf_seed)
        };
        let block_signers = match block_signers_result {
            Ok(signers) => signers,
            Err(err) => {
                let local_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
                let parent_missing = block
                    .header()
                    .prev_block_hash()
                    .is_some_and(|hash| !self.block_known_locally(hash));
                let ahead = block_height > local_height.saturating_add(1);
                let defer_signatures = matches!(
                    err,
                    crate::block::SignatureVerificationError::UnknownSignature
                        | crate::block::SignatureVerificationError::UnknownSignatory
                );
                if parent_missing && ahead && defer_signatures {
                    let expected_height = local_height.saturating_add(1);
                    let expected_usize = usize::try_from(expected_height).ok();
                    let actual_usize = usize::try_from(block_height).ok();
                    if let Some(parent_hash) = block.header().prev_block_hash() {
                        let commit_topology = self.effective_commit_topology();
                        self.request_missing_parent(
                            block_hash,
                            block_height,
                            block_view,
                            parent_hash,
                            &commit_topology,
                            Some(&selection.roster),
                            expected_usize,
                            actual_usize,
                            "block_sync_signatures",
                        );
                        if block_height > expected_height.saturating_add(1) {
                            self.request_missing_parents_for_gap(
                                &commit_topology,
                                Some(&selection.roster),
                                "block_sync_gap",
                            );
                        }
                    }
                    info!(
                        ?err,
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        local_height,
                        "deferring block sync update due to signature mismatch while behind"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockSyncUpdate,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::SignatureMismatchDeferred,
                    );
                    let created = super::message::BlockCreated { block };
                    let _ = self.handle_block_created(created, sender.clone());
                    return Ok(());
                }
                super::status::inc_block_sync_drop_invalid_signatures();
                warn!(
                    ?err,
                    hash = ?block_hash,
                    height = block_height,
                    view = block_view,
                    "dropping block sync update with invalid or insufficient signatures"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidSignature,
                );
                return Ok(());
            }
        };
        let commit_quorum = topology.min_votes_for_commit().max(1);
        let mut candidate_qc = {
            let state_view = self.state.view();
            incoming_qc
                .or_else(|| selection.commit_qc.clone())
                .or_else(|| {
                    crate::block_sync::BlockSynchronizer::block_sync_qc_for(
                        &state_view,
                        self.config.consensus_mode,
                        &block,
                    )
                })
        };
        candidate_qc = candidate_qc.and_then(|qc| {
            if qc.height != block_height {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    qc_height = qc.height,
                    "dropping block sync QC with mismatched height"
                );
                return None;
            }
            if qc.subject_block_hash != block_hash {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    qc_hash = %qc.subject_block_hash,
                    "dropping block sync QC with mismatched block hash"
                );
                return None;
            }
            if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                warn!(
                    height = block_height,
                    view = block_view,
                    hash = %block_hash,
                    phase = ?qc.phase,
                    "dropping block sync QC with non-precommit phase"
                );
                return None;
            }
            Some(qc)
        });
        let original_candidate_qc = candidate_qc.clone();

        let commit_cert_present = selection.commit_qc.is_some();
        let checkpoint_present = selection.checkpoint.is_some();
        let candidate_qc_present = candidate_qc.is_some();
        let candidate_qc_signers = candidate_qc.as_ref().map(qc_signer_count);
        let block_signer_count = block_signers.len();
        let signature_quorum_met = match consensus_mode {
            ConsensusMode::Permissioned => block_signer_count >= commit_quorum,
            ConsensusMode::Npos => {
                let signature_topology = super::topology_for_view(
                    &topology,
                    block_height,
                    block_view,
                    mode_tag,
                    prf_seed,
                );
                let mut signer_peers = BTreeSet::new();
                for signer in &block_signers {
                    let Ok(idx) = usize::try_from(*signer) else {
                        continue;
                    };
                    let Some(peer) = signature_topology.as_ref().get(idx) else {
                        continue;
                    };
                    signer_peers.insert(peer.clone());
                }
                if let Some(snapshot) = selection.stake_snapshot.as_ref() {
                    super::stake_snapshot::stake_quorum_reached_for_snapshot(
                        snapshot,
                        &selection.roster,
                        &signer_peers,
                    )
                    .unwrap_or(false)
                } else {
                    false
                }
            }
        };
        let local_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        let validated_qc = candidate_qc.as_ref().and_then(|qc| {
            let state_view = self.state.view();
            let world = state_view.world();
            match validate_block_sync_qc(
                qc,
                &topology,
                world,
                &block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
            ) {
                Ok(_) => Some(qc.clone()),
                Err(err) => {
                    record_qc_validation_error(self.telemetry_handle(), &err);
                    if had_incoming_qc {
                        super::status::inc_block_sync_qc_replaced();
                    }
                    warn!(
                        ?err,
                        reason = qc_validation_reason(&err),
                        hash = ?block_hash,
                        height = block_height,
                        view = block_view,
                        block_signers = block_signer_count,
                        candidate_qc_signers,
                        had_incoming_qc,
                        "dropping block sync QC after validation failure"
                    );
                    None
                }
            }
        });

        let derive_valid_qc = || {
            cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                block_height,
                block_view,
                expected_epoch,
            )
            .and_then(|qc| {
                let state_view = self.state.view();
                let world = state_view.world();
                validate_block_sync_qc(
                    &qc,
                    &topology,
                    world,
                    &block_signers,
                    block_view,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    mode_tag,
                    prf_seed,
                )
                .ok()
                .map(|_| qc)
            })
        };

        let (mut incoming_qc, incoming_qc_validated) = match (candidate_qc.take(), validated_qc) {
            (None, None) => {
                let derived = derive_valid_qc();
                let derived_validated = derived.is_some();
                (derived, derived_validated)
            }
            (_, Some(qc)) => (Some(qc), true),
            (Some(_), None) if had_incoming_qc => {
                let derived = derive_valid_qc();
                let derived_validated = derived.is_some();
                (derived, derived_validated)
            }
            (Some(_), None) => (None, false),
        };
        if incoming_qc.is_none() && had_incoming_qc {
            if let Some(qc) = original_candidate_qc {
                let aggregate_ok = super::qc_aggregate_consistent(
                    &qc,
                    &topology,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    mode_tag,
                );
                if aggregate_ok {
                    let stake_quorum_ok = match consensus_mode {
                        ConsensusMode::Permissioned => true,
                        ConsensusMode::Npos => {
                            let mut ok = false;
                            if let Some(snapshot) = stake_snapshot.as_ref() {
                                let roster_len = topology.as_ref().len();
                                match super::qc_signer_indices(&qc, roster_len, roster_len) {
                                    Ok(parsed) => match super::signer_peers_for_topology(
                                        &parsed.voting,
                                        &topology,
                                    ) {
                                        Ok(signer_peers) => {
                                            ok = super::stake_snapshot::stake_quorum_reached_for_snapshot(
                                                snapshot,
                                                topology.as_ref(),
                                                &signer_peers,
                                            )
                                            .unwrap_or(false);
                                        }
                                        Err(_) => {
                                            warn!(
                                                height = block_height,
                                                view = block_view,
                                                block = %block_hash,
                                                "dropping block sync QC: signer mapping failed"
                                            );
                                        }
                                    },
                                    Err(_) => {
                                        warn!(
                                            height = block_height,
                                            view = block_view,
                                            block = %block_hash,
                                            "dropping block sync QC: invalid signer bitmap"
                                        );
                                    }
                                }
                            } else {
                                warn!(
                                    height = block_height,
                                    view = block_view,
                                    block = %block_hash,
                                    "dropping block sync QC: missing stake snapshot"
                                );
                            }
                            ok
                        }
                    };
                    if stake_quorum_ok {
                        let qc_signers = qc_signer_count(&qc);
                        info!(
                            hash = %block_hash,
                            height = block_height,
                            view = block_view,
                            qc_signers,
                            "accepting block sync QC validated from aggregate signature despite local validation failure"
                        );
                        incoming_qc = Some(qc);
                    }
                }
            }
        }
        let qc_evidence_present = incoming_qc.is_some();
        if !block_sync_quorum_available(
            block_signer_count,
            commit_quorum,
            signature_quorum_met,
            qc_evidence_present,
            commit_cert_present,
            checkpoint_present,
            requested_missing_block,
            block_height,
            local_height,
        ) {
            super::status::inc_block_sync_drop_invalid_signatures();
            warn!(
                hash = ?block_hash,
                height = block_height,
                view = block_view,
                block_signers = block_signer_count,
                signatures = block.signatures().count(),
                commit_quorum,
                candidate_qc_present,
                candidate_qc_signers,
                qc_evidence_present,
                incoming_qc_validated,
                missing_request = requested_missing_block,
                local_height,
                "dropping block sync update missing commit-role quorum"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::QuorumMissing,
            );
            return Ok(());
        } else if requested_missing_block
            && block_signer_count < commit_quorum
            && !qc_evidence_present
        {
            info!(
                hash = ?block_hash,
                height = block_height,
                view = block_view,
                signatures = block_signer_count,
                commit_quorum,
                "applying block sync update below commit quorum to satisfy missing-block request"
            );
        }
        let incoming_qc_signers = incoming_qc.as_ref().map(qc_signer_count);
        let allow_nonextending_qc = selection.commit_qc.is_some()
            || incoming_qc.as_ref().is_some_and(|cert| {
                let inputs = self.roster_validation_cache.inputs_for_roster(
                    &cert.validator_set,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                );
                super::validate_commit_qc_roster(
                    cert,
                    block_hash,
                    block_height,
                    Some(block_view),
                    consensus_mode,
                    expected_epoch,
                    &self.common_config.chain,
                    mode_tag,
                    false,
                    &inputs,
                )
                .is_ok()
            })
            || incoming_qc_validated;
        if !qc_evidence_present
            && !commit_cert_present
            && !checkpoint_present
            && block_signer_count < commit_quorum
            && !requested_missing_block
        {
            let now = Instant::now();
            let cooldown = self.rebroadcast_cooldown();
            if self.block_sync_fetch_log.allow(block_hash, now, cooldown) {
                let targets = Self::build_fetch_targets(&block_signers, &topology);
                if targets.is_empty() {
                    debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "skipping pending-block fetch: no viable targets"
                    );
                } else {
                    let request = super::message::FetchPendingBlock {
                        requester: self.common_config.peer.id.clone(),
                        block_hash,
                    };
                    let msg = BlockMessage::FetchPendingBlock(request);
                    for peer in targets {
                        self.schedule_background(BackgroundRequest::Post {
                            peer,
                            msg: msg.clone(),
                        });
                    }
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        block_signers = block_signer_count,
                        commit_quorum,
                        "requesting pending block to recover missing QC"
                    );
                }
            } else {
                trace!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    cooldown_ms = cooldown.as_millis(),
                    "skipping pending-block fetch due to cooldown"
                );
            }
        }
        info!(
            hash = ?block_hash,
            height = block_height,
            block_signers = block_signer_count,
            candidate_qc_present,
            candidate_qc_signers,
            incoming_qc_signers,
            "applying block sync update"
        );

        let created = super::message::BlockCreated { block };
        let creation_result = self.handle_block_created(created, sender.clone());
        let block_known_after_creation = self.block_known_locally(block_hash);
        let creation_ok = creation_result.is_ok();
        let ready_for_qc = block_sync_ready_for_qc(block_known_after_creation, &creation_result);
        if block_known_after_creation {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
        }
        if !ready_for_qc {
            if let Err(err) = &creation_result {
                warn!(
                    ?err,
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: failed to apply block payload"
                );
            } else {
                warn!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: block not accepted locally"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadUnapplied,
            );
        }
        process_commit_votes(self);

        let qc_to_apply = if ready_for_qc {
            incoming_qc.take()
        } else {
            None
        };

        block_sync_apply_qc_after_block(
            creation_result,
            block_known_after_creation,
            qc_to_apply,
            |qc| {
                if topology.as_ref().is_empty() {
                    warn!(
                        height = block_height,
                        view = block_view,
                        "dropping block sync QC: empty commit topology"
                    );
                    return Ok(());
                }
                if qc.subject_block_hash != block_hash {
                    warn!(
                        incoming_hash = %block_hash,
                        qc_hash = %qc.subject_block_hash,
                        "ignoring block sync QC that does not match block hash"
                    );
                    return Ok(());
                }
                if qc.height != block_height {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        qc_height = qc.height,
                        "ignoring block sync QC that does not match block height"
                    );
                    return Ok(());
                }
                let expected_epoch = self.epoch_for_height(block_height);
                if qc.epoch != expected_epoch {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        expected_epoch,
                        qc_epoch = qc.epoch,
                        "ignoring block sync QC with mismatched epoch"
                    );
                    return Ok(());
                }
                if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                    warn!(
                        incoming_hash = %block_hash,
                        phase = ?qc.phase,
                        "ignoring block sync QC with non-precommit phase"
                    );
                    return Ok(());
                }
                let qc_signers = qc_signer_count(&qc);
                let qc_ref = Self::qc_to_header_ref(&qc);
                let extends_locked = qc_extends_locked_if_present(
                    self.locked_qc,
                    qc_ref,
                    |hash, height| self.parent_hash_for(hash, height),
                    |hash| self.block_known_for_lock(hash),
                );
                let tally_result = {
                    let state_view = self.state.view();
                    let world = state_view.world();
                    tally_qc_against_block_signers(
                        &qc,
                        &topology,
                        world,
                        &block_signers,
                        block_view,
                        &self.roster_validation_cache.pops,
                        &self.common_config.chain,
                        consensus_mode,
                        stake_snapshot.as_ref(),
                        mode_tag,
                        prf_seed,
                    )
                };
                match tally_result {
                    Ok(tally) => {
                        crate::sumeragi::status::record_precommit_signers(
                            crate::sumeragi::status::PrecommitSignerRecord {
                                block_hash,
                                height: qc.height,
                                view: qc.view,
                                epoch: qc.epoch,
                                parent_state_root: qc.parent_state_root,
                                post_state_root: qc.post_state_root,
                                signers: tally.voting_signers.clone(),
                                bls_aggregate_signature: qc
                                    .aggregate
                                    .bls_aggregate_signature
                                    .clone(),
                                roster_len: topology.as_ref().len(),
                                mode_tag: mode_tag.to_string(),
                                validator_set: topology.as_ref().to_vec(),
                                stake_snapshot: stake_snapshot.clone(),
                            },
                        );
                        self.note_validated_qc_tally(&qc, tally.clone());
                        let block_known_for_lock = self.block_known_for_lock(block_hash);
                        if !self.process_precommit_qc(
                            &qc,
                            block_known_for_lock,
                            allow_nonextending_qc,
                        ) {
                            info!(
                                incoming_hash = %block_hash,
                                height = block_height,
                                view = block_view,
                                "dropping block sync QC that conflicts with locked chain"
                            );
                            return Ok(());
                        }
                        super::status::record_commit_qc(qc.clone());
                        self.qc_cache.insert(
                            (
                                qc.phase,
                                qc.subject_block_hash,
                                qc.height,
                                qc.view,
                                qc.epoch,
                            ),
                            qc.clone(),
                        );
                        #[cfg(feature = "telemetry")]
                        if let Some(telemetry) = self.telemetry_handle() {
                            telemetry.note_qc_signer_counts(
                                "precommit",
                                tally.present_signers,
                                tally.voting_signers.len(),
                            );
                        }
                        debug!(
                            incoming_hash = %block_hash,
                            signers = tally.voting_signers.len(),
                            qc_signers,
                            "applied block sync QC after validation"
                        );
                        if extends_locked {
                            if block_known_for_lock {
                                self.apply_commit_qc(
                                    &qc,
                                    topology.as_ref(),
                                    block_hash,
                                    block_height,
                                    block_view,
                                );
                                self.process_commit_candidates();
                            } else {
                                debug!(
                                    incoming_hash = %block_hash,
                                    height = block_height,
                                    view = block_view,
                                    "deferring commit apply for block sync QC until block is validated"
                                );
                            }
                        } else {
                            debug!(
                                incoming_hash = %block_hash,
                                height = block_height,
                                view = block_view,
                                "skipping commit apply for non-extending block sync QC"
                            );
                        }
                    }
                    Err(err) => {
                        record_qc_validation_error(self.telemetry_handle(), &err);
                        warn!(
                            ?err,
                            reason = qc_validation_reason(&err),
                            incoming_hash = %block_hash,
                            height = block_height,
                            view = block_view,
                            qc_signers,
                            block_signers = block_signer_count,
                            "dropping block sync QC after validation failure"
                        );
                    }
                }
                Ok(())
            },
        )?;

        if creation_ok && !block_known_after_creation {
            if let Some(qc) = incoming_qc.take() {
                // Cache the QC so we can reuse it once the block becomes available locally.
                self.cache_block_sync_qc_for_unknown_block(
                    qc,
                    block_hash,
                    block_height,
                    block_view,
                    &topology,
                    &block_signers,
                    allow_nonextending_qc,
                    consensus_mode,
                    stake_snapshot.clone(),
                    mode_tag,
                    prf_seed,
                );
            }
        }

        Ok(())
    }

    /// Cache a validated precommit QC from block sync when the block payload is not ready yet.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn cache_block_sync_qc_for_unknown_block(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        topology: &super::network_topology::Topology,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        allow_nonextending_qc: bool,
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
        mode_tag: &str,
        prf_seed: Option<[u8; 32]>,
    ) {
        if topology.as_ref().is_empty() {
            warn!(
                height = block_height,
                view = block_view,
                "dropping cached block sync QC: empty commit topology"
            );
            return;
        }
        if qc.subject_block_hash != block_hash {
            warn!(
                incoming_hash = %block_hash,
                qc_hash = %qc.subject_block_hash,
                "ignoring cached block sync QC that does not match block hash"
            );
            return;
        }
        if qc.height != block_height {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                qc_height = qc.height,
                "ignoring cached block sync QC that does not match block height"
            );
            return;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        if qc.epoch != expected_epoch {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                expected_epoch,
                qc_epoch = qc.epoch,
                "ignoring cached block sync QC with mismatched epoch"
            );
            return;
        }
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            warn!(
                incoming_hash = %block_hash,
                phase = ?qc.phase,
                "ignoring cached block sync QC with non-precommit phase"
            );
            return;
        }
        let qc_signers = qc_signer_count(&qc);
        let tally_result = {
            let state_view = self.state.view();
            let world = state_view.world();
            tally_qc_against_block_signers(
                &qc,
                topology,
                world,
                block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
            )
        };
        match tally_result {
            Ok(tally) => {
                crate::sumeragi::status::record_precommit_signers(
                    crate::sumeragi::status::PrecommitSignerRecord {
                        block_hash,
                        height: qc.height,
                        view: qc.view,
                        epoch: qc.epoch,
                        parent_state_root: qc.parent_state_root,
                        post_state_root: qc.post_state_root,
                        signers: tally.voting_signers.clone(),
                        bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                        roster_len: topology.as_ref().len(),
                        mode_tag: mode_tag.to_string(),
                        validator_set: topology.as_ref().to_vec(),
                        stake_snapshot: stake_snapshot.clone(),
                    },
                );
                self.note_validated_qc_tally(&qc, tally.clone());
                if !self.process_precommit_qc(&qc, false, allow_nonextending_qc) {
                    info!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        "dropping cached block sync QC that conflicts with locked chain"
                    );
                    return;
                }
                super::status::record_commit_qc(qc.clone());
                self.qc_cache.insert(
                    (
                        qc.phase,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        qc.epoch,
                    ),
                    qc,
                );
                debug!(
                    incoming_hash = %block_hash,
                    signers = tally.voting_signers.len(),
                    qc_signers,
                    "cached block sync QC before block payload is ready"
                );
            }
            Err(err) => {
                record_qc_validation_error(self.telemetry_handle(), &err);
                warn!(
                    ?err,
                    reason = qc_validation_reason(&err),
                    incoming_hash = %block_hash,
                    height = block_height,
                    view = block_view,
                    qc_signers,
                    block_signers = block_signers.len(),
                    "dropping cached block sync QC after validation failure"
                );
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_fetch_pending_block(
        &mut self,
        request: super::message::FetchPendingBlock,
    ) -> Result<()> {
        let block_hash = request.block_hash;
        let peer = request.requester;
        let mut responded = false;

        let inflight_response = if let Some(inflight) = self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .filter(|inflight| inflight.block_hash == block_hash)
        {
            if matches!(
                inflight.pending.validation_status,
                ValidationStatus::Invalid
            ) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid inflight pending block"
                );
                None
            } else {
                let block = inflight.pending.block.clone();
                let block_hash = block.hash();
                let block_height = block.header().height().get();
                let block_view = block.header().view_change_index();
                let update = super::block_sync_update_with_roster(
                    &block,
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.config.consensus_mode,
                    self.common_config.trusted_peers.value(),
                    self.common_config.peer.id(),
                    &self.roster_validation_cache,
                );
                let mut update = update;
                let expected_epoch = self.epoch_for_height(block_height);
                Self::apply_cached_qcs_to_block_sync_update(
                    &mut update,
                    &self.qc_cache,
                    &self.vote_log,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                    self.state.as_ref(),
                    self.config.consensus_mode,
                );
                let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
                let has_roster = super::block_sync_update_has_roster(&update, consensus_mode);
                let has_cached_qc = update.commit_qc.is_some() || !update.commit_votes.is_empty();
                let msg = if !has_roster && !has_cached_qc {
                    BlockMessage::BlockCreated(super::message::BlockCreated::from(&block))
                } else {
                    BlockMessage::BlockSyncUpdate(update)
                };
                Some((block, msg))
            }
        } else {
            None
        };
        if let Some((block, msg)) = inflight_response {
            self.send_fetch_pending_block_response(peer.clone(), msg);
            self.send_fetch_pending_block_rbc_init(peer.clone(), &block);
            return Ok(());
        }

        let pending_response = if let Some(pending) = self.pending.pending_blocks.get(&block_hash) {
            if matches!(pending.validation_status, ValidationStatus::Invalid) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid pending block"
                );
                None
            } else {
                let block = pending.block.clone();
                let block_hash = block.hash();
                let block_height = block.header().height().get();
                let block_view = block.header().view_change_index();
                let update = super::block_sync_update_with_roster(
                    &block,
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.config.consensus_mode,
                    self.common_config.trusted_peers.value(),
                    self.common_config.peer.id(),
                    &self.roster_validation_cache,
                );
                let mut update = update;
                let expected_epoch = self.epoch_for_height(block_height);
                Self::apply_cached_qcs_to_block_sync_update(
                    &mut update,
                    &self.qc_cache,
                    &self.vote_log,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                    self.state.as_ref(),
                    self.config.consensus_mode,
                );
                let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
                let has_roster = super::block_sync_update_has_roster(&update, consensus_mode);
                let has_cached_qc = update.commit_qc.is_some() || !update.commit_votes.is_empty();
                let msg = if !has_roster && !has_cached_qc {
                    BlockMessage::BlockCreated(super::message::BlockCreated::from(&block))
                } else {
                    BlockMessage::BlockSyncUpdate(update)
                };
                Some((block, msg))
            }
        } else {
            None
        };
        if let Some((block, msg)) = pending_response {
            self.send_fetch_pending_block_response(peer.clone(), msg);
            self.send_fetch_pending_block_rbc_init(peer.clone(), &block);
            return Ok(());
        }

        if let Some(height) = self.kura.get_block_height_by_hash(block_hash) {
            if let Some(block) = self.kura.get_block(height) {
                let block = block.as_ref();
                let block_hash = block.hash();
                let block_height = block.header().height().get();
                let block_view = block.header().view_change_index();
                let update = super::block_sync_update_with_roster(
                    block,
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.config.consensus_mode,
                    self.common_config.trusted_peers.value(),
                    self.common_config.peer.id(),
                    &self.roster_validation_cache,
                );
                let mut update = update;
                let expected_epoch = self.epoch_for_height(block_height);
                Self::apply_cached_qcs_to_block_sync_update(
                    &mut update,
                    &self.qc_cache,
                    &self.vote_log,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                    self.state.as_ref(),
                    self.config.consensus_mode,
                );
                let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
                let has_roster = super::block_sync_update_has_roster(&update, consensus_mode);
                let has_cached_qc = update.commit_qc.is_some() || !update.commit_votes.is_empty();
                let msg = if !has_roster && !has_cached_qc {
                    BlockMessage::BlockCreated(super::message::BlockCreated::from(block))
                } else {
                    BlockMessage::BlockSyncUpdate(update)
                };
                self.send_fetch_pending_block_response(peer.clone(), msg);
                self.send_fetch_pending_block_rbc_init(peer.clone(), block);
                responded = true;
            }
        }

        if !responded {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::FetchPendingBlock,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::NotFound,
            );
        }
        Ok(())
    }
}
