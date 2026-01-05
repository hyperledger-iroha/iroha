//! Vote/QC accumulation, validation, and processing helpers.

use iroha_logger::prelude::*;

use super::*;

#[derive(Debug)]
struct QcSignerSnapshot {
    signers: BTreeSet<ValidatorIndex>,
    voting_signers: usize,
    total_signers: usize,
}

impl Actor {
    pub(super) fn request_missing_block(
        &self,
        block_hash: HashOf<BlockHeader>,
        targets: &[PeerId],
    ) {
        send_missing_block_request(
            &self.network,
            &self.common_config.peer.id,
            block_hash,
            targets,
        );
    }

    pub(super) fn qc_signers_for_votes(
        &self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signature_topology: &super::network_topology::Topology,
    ) -> BTreeSet<ValidatorIndex> {
        let chain_id = &self.common_config.chain;
        let (_, mode_tag, _) = self.consensus_context_for_height(height);
        self.vote_log
            .values()
            .filter(|stored| {
                stored.phase == phase
                    && stored.block_hash == block_hash
                    && stored.height == height
                    && stored.view == view
                    && stored.epoch == epoch
            })
            .filter(|vote| vote_signature_valid(vote, signature_topology, chain_id, mode_tag))
            .map(|vote| vote.signer)
            .collect()
    }

    pub(super) fn defer_qc_if_block_missing(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        signers: &BTreeSet<ValidatorIndex>,
        topology: &super::network_topology::Topology,
    ) -> bool {
        let da_enabled = self.runtime_da_enabled();
        let retry_window = self.quorum_timeout(da_enabled);
        let peer_id = self.common_config.peer.id.clone();
        let network = self.network.clone();
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let telemetry = self.telemetry_handle();
        let now = Instant::now();
        let deferred = defer_qc_for_missing_block(
            self.block_payload_available_locally(block_hash),
            retry_window,
            now,
            block_hash,
            height,
            view,
            phase,
            signers,
            topology,
            self.config.missing_block_signer_fallback_attempts,
            &mut requests,
            telemetry,
            move |targets| send_missing_block_request(&network, &peer_id, block_hash, targets),
        );
        self.pending.missing_block_requests = requests;
        if deferred {
            if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
                if stats.mark_view_change_if_due(now, retry_window) {
                    let dwell_ms = now.saturating_duration_since(stats.first_seen).as_millis();
                    let since_last_ms = now
                        .saturating_duration_since(stats.last_requested)
                        .as_millis();
                    warn!(
                        height,
                        view,
                        dwell_ms,
                        since_last_ms,
                        attempts = stats.attempts,
                        "missing block dwell exceeded retry window; forcing view change"
                    );
                    self.trigger_view_change_with_cause(
                        height,
                        view,
                        ViewChangeCause::MissingPayload,
                    );
                }
            }
        }
        deferred
    }

    pub(super) fn build_fetch_targets(
        signers: &BTreeSet<ValidatorIndex>,
        topology: &super::network_topology::Topology,
    ) -> Vec<PeerId> {
        let mut targets: Vec<_> = signers
            .iter()
            .filter_map(|signer| usize::try_from(*signer).ok())
            .filter_map(|idx| topology.as_ref().get(idx).cloned())
            .collect();
        if targets.is_empty() {
            targets = topology.as_ref().to_vec();
        }
        targets
    }

    #[cfg(feature = "telemetry")]
    pub(super) fn update_missing_block_gauges(&self) {
        if let Some(telemetry) = self.telemetry_handle() {
            let oldest_ms = self
                .pending
                .missing_block_requests
                .values()
                .filter_map(|stats| stats.first_seen.elapsed().as_millis().try_into().ok())
                .min()
                .unwrap_or(0);
            telemetry
                .set_missing_block_inflight(self.pending.missing_block_requests.len(), oldest_ms);
        }
    }

    #[cfg(not(feature = "telemetry"))]
    pub(super) fn update_missing_block_gauges(&self) {}

    #[cfg(feature = "telemetry")]
    pub(super) fn note_missing_block_fetch_metrics(
        &self,
        decision: &MissingBlockFetchDecision,
        retry_window: Duration,
        targets: usize,
        dwell: Duration,
    ) {
        if let Some(telemetry) = self.telemetry_handle() {
            let outcome = match decision {
                MissingBlockFetchDecision::Requested { .. } => MissingBlockFetchOutcome::Requested,
                MissingBlockFetchDecision::Backoff => MissingBlockFetchOutcome::Backoff,
                MissingBlockFetchDecision::NoTargets => MissingBlockFetchOutcome::NoTargets,
            };
            let target_kind = match decision {
                MissingBlockFetchDecision::Requested { target_kind, .. } => Some(*target_kind),
                _ => None,
            };
            telemetry.note_missing_block_fetch(outcome, targets, dwell, target_kind);
            telemetry.set_missing_block_retry_window_ms(
                retry_window.as_millis().try_into().unwrap_or(u64::MAX),
            );
            self.update_missing_block_gauges();
        }
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn note_missing_block_fetch_metrics(
        &self,
        _decision: &MissingBlockFetchDecision,
        _retry_window: Duration,
        _targets: usize,
        _dwell: Duration,
    ) {
    }

    pub(super) fn clear_missing_block_request(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
        reason: MissingBlockClearReason,
    ) {
        let block_known_locally = self.block_payload_available_locally(*block_hash);
        let allowed = missing_block_clear_allowed(block_known_locally, reason);
        let stats = allowed
            .then(|| {
                clear_missing_block_request(&mut self.pending.missing_block_requests, block_hash)
            })
            .flatten();
        if !allowed {
            debug!(
                ?block_hash,
                reason = reason.as_str(),
                "skipping missing-block request clear; block payload not known locally"
            );
        }
        if let Some(stats) = stats {
            let dwell = stats.first_seen.elapsed();
            debug!(
                ?block_hash,
                reason = reason.as_str(),
                dwell_ms = dwell.as_millis(),
                "cleared missing-block request"
            );
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.observe_missing_block_dwell(dwell);
            }
        }
        self.update_missing_block_gauges();
    }

    pub(super) fn build_qc_from_signers(
        &self,
        ctx: QcBuildContext,
        signers: &BTreeSet<ValidatorIndex>,
        canonical_topology: &super::network_topology::Topology,
        aggregate_signature: Vec<u8>,
    ) -> crate::sumeragi::consensus::Qc {
        let signers_bitmap = build_signers_bitmap(signers, canonical_topology.as_ref().len());
        debug_assert!(
            !aggregate_signature.is_empty(),
            "QC aggregate signature must be non-empty"
        );

        let validator_set = canonical_topology.as_ref().to_vec();
        crate::sumeragi::consensus::Qc {
            phase: ctx.phase,
            subject_block_hash: ctx.block_hash,
            height: ctx.height,
            view: ctx.view,
            epoch: ctx.epoch,
            mode_tag: ctx.mode_tag,
            highest_cert: ctx.highest_cert,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: crate::sumeragi::consensus::CommitAggregate {
                signers_bitmap,
                bls_aggregate_signature: aggregate_signature,
            },
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn try_form_qc_from_votes(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        topology: super::network_topology::Topology,
    ) {
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology =
            topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let required = signature_topology.min_votes_for_commit();
        let voting_len = signature_topology.as_ref().len();
        if let Some(existing) = self.qc_cache.get(&(phase, block_hash, height, view, epoch)) {
            let existing_signers = self
                .qc_signer_tally
                .get(&(phase, block_hash, height, view, epoch))
                .map_or_else(
                    || qc_voting_signer_count(existing, voting_len),
                    QcSignerTally::voting_len,
                );
            if existing.phase == phase && existing_signers >= required {
                return;
            }
        }
        if self
            .pending
            .pending_blocks
            .get(&block_hash)
            .is_some_and(|pending| pending.aborted)
        {
            iroha_logger::debug!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                "skipping QC aggregation for aborted pending block"
            );
            return;
        }

        let snapshot = self.qc_signer_snapshot(
            phase,
            block_hash,
            height,
            view,
            epoch,
            &signature_topology,
            required,
        );
        let block_known = self.block_known_locally(block_hash);
        let deferred = self.qc_missing_block_defer(
            phase,
            block_hash,
            height,
            view,
            &snapshot.signers,
            &signature_topology,
            required,
            block_known,
            snapshot.voting_signers,
            snapshot.total_signers,
        );
        if deferred {
            return;
        }

        if snapshot.voting_signers < required {
            iroha_logger::info!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                voting_signers = snapshot.voting_signers,
                total_signers = snapshot.total_signers,
                required,
                "not enough votes collected for QC"
            );
            return;
        }
        if !self.precommit_qc_extends_locked(phase, block_hash, height, view, epoch) {
            return;
        }

        let aggregate_signature = match super::aggregate_vote_signatures(
            &self.vote_log,
            phase,
            block_hash,
            height,
            view,
            epoch,
            &snapshot.signers,
        ) {
            Ok(signature) => signature,
            Err(err) => {
                warn!(
                    height,
                    view,
                    phase = ?phase,
                    block = ?block_hash,
                    ?err,
                    "failed to aggregate QC signatures from votes"
                );
                return;
            }
        };
        let highest_cert = if phase == crate::sumeragi::consensus::Phase::NewView {
            let mut selected: Option<crate::sumeragi::consensus::CommitCertificateRef> = None;
            for signer in &snapshot.signers {
                let Some(vote) = self.vote_log.get(&(phase, height, view, epoch, *signer)) else {
                    continue;
                };
                let Some(candidate) = vote.highest_cert else {
                    continue;
                };
                if candidate.phase != crate::sumeragi::consensus::Phase::Commit {
                    continue;
                }
                selected = Some(match selected {
                    None => candidate,
                    Some(current) => {
                        let incoming = (candidate.height, candidate.view);
                        let existing = (current.height, current.view);
                        let promotes_phase = incoming == existing
                            && candidate.phase == crate::sumeragi::consensus::Phase::Commit
                            && current.phase != crate::sumeragi::consensus::Phase::Commit;
                        if incoming > existing || promotes_phase {
                            candidate
                        } else {
                            current
                        }
                    }
                });
            }
            selected
        } else {
            None
        };
        if phase == crate::sumeragi::consensus::Phase::NewView && highest_cert.is_none() {
            warn!(
                height,
                view,
                block = ?block_hash,
                "skipping NEW_VIEW certificate: highest certificate missing in votes"
            );
            return;
        }

        let canonical_signers = super::normalize_signer_indices_to_canonical(
            &snapshot.signers,
            &signature_topology,
            &topology,
        );
        if canonical_signers.len() != snapshot.signers.len() {
            warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                signers = snapshot.signers.len(),
                canonical = canonical_signers.len(),
                "skipping QC: signer mapping to canonical roster incomplete"
            );
            return;
        }

        let qc = self.build_qc_from_signers(
            QcBuildContext {
                phase,
                block_hash,
                height,
                view,
                epoch,
                mode_tag: mode_tag.to_string(),
                highest_cert,
            },
            &canonical_signers,
            &topology,
            aggregate_signature,
        );

        iroha_logger::info!(
            height,
            view,
            phase = ?phase,
            block = ?block_hash,
            voting_signers = snapshot.voting_signers,
            total_signers = snapshot.total_signers,
            required,
            "aggregated QC from votes"
        );
        if let Err(err) = self.handle_qc(qc.clone()) {
            warn!(
                ?err,
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                "failed to handle aggregated QC"
            );
            return;
        }
        let msg = BlockMessage::CommitCertificate(qc);
        let topology_peers = self.effective_commit_topology();
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: msg.clone(),
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn qc_signer_snapshot(
        &self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signature_topology: &super::network_topology::Topology,
        required: usize,
    ) -> QcSignerSnapshot {
        let signers =
            self.qc_signers_for_votes(phase, block_hash, height, view, epoch, signature_topology);
        let raw_votes = self
            .vote_log
            .values()
            .filter(|stored| {
                stored.phase == phase
                    && stored.block_hash == block_hash
                    && stored.height == height
                    && stored.view == view
                    && stored.epoch == epoch
            })
            .count();
        if signers.is_empty() && raw_votes > 0 {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                raw_votes,
                required,
                topology_len = signature_topology.as_ref().len(),
                "votes observed but no valid signers collected for QC"
            );
        } else if raw_votes > 0 && signers.len() != raw_votes {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                raw_votes,
                valid_signers = signers.len(),
                required,
                topology_len = signature_topology.as_ref().len(),
                "some votes failed signature/topology validation; QC tally may stall"
            );
        }
        let voting_signers = voting_signer_count(&signers, signature_topology.as_ref().len());
        let total_signers = signers.len();
        QcSignerSnapshot {
            signers,
            voting_signers,
            total_signers,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn qc_missing_block_defer(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        signers: &BTreeSet<ValidatorIndex>,
        signature_topology: &super::network_topology::Topology,
        required: usize,
        block_known: bool,
        voting_signers: usize,
        total_signers: usize,
    ) -> bool {
        if !block_known && voting_signers >= required {
            crate::sumeragi::status::inc_qc_quorum_without_qc();
            warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                voting_signers,
                total_signers,
                required,
                "quorum of votes observed but block payload missing; deferring QC aggregation"
            );
        }

        let deferred = if block_known {
            false
        } else {
            self.defer_qc_if_block_missing(
                phase,
                block_hash,
                height,
                view,
                signers,
                signature_topology,
            )
        };
        deferred
    }

    fn precommit_qc_extends_locked(
        &self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> bool {
        if !matches!(phase, crate::sumeragi::consensus::Phase::Commit) {
            return true;
        }
        let Some(lock) = self.locked_qc else {
            return true;
        };
        let candidate = crate::sumeragi::consensus::QcHeaderRef {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            height,
            view,
            epoch,
        };
        let extends_locked =
            qc_extends_locked_with_lookup(lock, candidate, |hash, lookup_height| {
                self.parent_hash_for(hash, lookup_height)
            });
        if !extends_locked {
            warn!(
                height,
                view,
                block = ?block_hash,
                locked_height = lock.height,
                locked_hash = %lock.subject_block_hash,
                "skipping precommit QC aggregation: block does not extend locked chain"
            );
        }
        extends_locked
    }

    pub(super) fn try_form_exec_qc_from_votes(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        topology: super::network_topology::Topology,
    ) {
        if self.execution_qc_cache.contains_key(&block_hash) {
            return;
        }
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology =
            topology_for_view(&topology, height, view, mode_tag, prf_seed);
        let required = signature_topology.min_votes_for_commit();
        let voting_len = signature_topology.as_ref().len();
        if required == 0 {
            return;
        }
        let groups =
            super::exec_vote_groups_for_block(&self.exec_vote_log, block_hash, height, view, epoch);
        let mut best: Option<((Hash, Hash), BTreeSet<ValidatorIndex>, usize)> = None;
        for (roots, signers) in groups {
            let voting_signers = voting_signer_count(&signers, voting_len);
            if voting_signers < required {
                continue;
            }
            let better = match best.as_ref() {
                None => true,
                Some((best_roots, _best_signers, best_voting)) => {
                    voting_signers > *best_voting
                        || (voting_signers == *best_voting && roots < *best_roots)
                }
            };
            if better {
                best = Some((roots, signers, voting_signers));
            }
        }
        let Some(((parent_state_root, post_state_root), signers, _)) = best else {
            return;
        };
        let aggregate_signature = match super::aggregate_exec_vote_signatures(
            &self.exec_vote_log,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            &signers,
        ) {
            Ok(signature) => signature,
            Err(err) => {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    ?err,
                    "failed to aggregate ExecutionQC signatures from exec votes"
                );
                return;
            }
        };
        let signers_bitmap = build_signers_bitmap(&signers, signature_topology.as_ref().len());
        let qc = crate::sumeragi::consensus::ExecutionQC {
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            aggregate: crate::sumeragi::consensus::CommitAggregate {
                signers_bitmap,
                bls_aggregate_signature: aggregate_signature,
            },
        };
        if let Err(err) = self.handle_execution_qc(qc.clone()) {
            warn!(
                ?err,
                height,
                view,
                block = %block_hash,
                "failed to persist ExecutionQC after aggregation"
            );
            return;
        }
        let msg = BlockMessage::ExecutionQC(qc);
        let topology_peers = self.effective_commit_topology();
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: msg.clone(),
            });
        }
    }

    pub(super) fn rebuild_qcs_from_cached_votes(&mut self, commit_topology: &[PeerId]) {
        let topology = super::network_topology::Topology::new(commit_topology.to_vec());
        let required = topology.min_votes_for_commit();
        if required == 0 {
            return;
        }

        // Avoid simultaneous immutable + mutable borrows of `self` by snapshotting read-only state.
        let existing_qcs: BTreeSet<QcVoteKey> = self.qc_cache.keys().copied().collect();
        let qc_present = move |key: &QcVoteKey| existing_qcs.contains(key);
        let pending_hashes = self
            .pending
            .pending_blocks
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let kura = Arc::clone(&self.kura);
        let block_known = move |hash| {
            pending_hashes.contains(&hash) || kura.get_block_height_by_hash(hash).is_some()
        };
        let vote_log: Vec<_> = self.vote_log.values().cloned().collect();
        rebuild_qc_candidates_with(
            vote_log.iter(),
            required,
            block_known,
            qc_present,
            |key, signer_count| {
                let (phase, block_hash, height, view, epoch) = key;
                let cached_before = self.qc_cache.contains_key(&key);
                super::status::inc_qc_rebuild_attempts();
                iroha_logger::info!(
                    phase = ?phase,
                    height,
                    view,
                    epoch,
                    block = %block_hash,
                    signers = signer_count,
                    "rebuilding QC from cached votes"
                );
                self.try_form_qc_from_votes(
                    phase,
                    block_hash,
                    height,
                    view,
                    epoch,
                    topology.clone(),
                );
                if let Some(qc) = self.qc_cache.get(&(phase, block_hash, height, view, epoch)) {
                    if qc.phase == phase {
                        if !cached_before {
                            super::status::inc_qc_rebuild_successes();
                        }
                        iroha_logger::info!(
                            phase = ?phase,
                            height,
                            view,
                            epoch,
                            block = %block_hash,
                            "rebuilt QC cached locally"
                        );
                    }
                }
            },
        );
    }

    pub(super) fn qc_to_header_ref(
        qc: &crate::sumeragi::consensus::Qc,
    ) -> crate::sumeragi::consensus::QcHeaderRef {
        crate::sumeragi::consensus::QcHeaderRef {
            phase: qc.phase,
            subject_block_hash: qc.subject_block_hash,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
        }
    }

    pub(super) fn block_tx_count(&self, hash: HashOf<BlockHeader>) -> Option<usize> {
        if let Some(height) = self.kura.get_block_height_by_hash(hash) {
            if let Some(block) = self.kura.get_block(height) {
                return Some(block.transactions_vec().len());
            }
        }
        self.pending
            .pending_blocks
            .get(&hash)
            .map(|pending| pending.block.transactions_vec().len())
    }

    pub(super) fn has_nonempty_pending_at_height(&self, height: u64) -> bool {
        self.pending.pending_blocks.values().any(|pending| {
            pending.height == height
                && !pending.aborted
                && !pending.block.transactions_vec().is_empty()
        })
    }

    pub(super) fn drop_missing_lock_if_unknown(&mut self, qc: &crate::sumeragi::consensus::Qc) {
        if let Some(lock) = self.locked_qc {
            if !self.block_known_locally(lock.subject_block_hash) {
                info!(
                    locked_height = lock.height,
                    locked_view = lock.view,
                    locked_hash = %lock.subject_block_hash,
                    incoming_height = qc.height,
                    incoming_view = qc.view,
                    "clearing locked QC that is missing locally before processing incoming QC"
                );
                self.locked_qc = None;
                super::status::set_locked_qc(0, 0, None);
            }
        }
    }

    pub(super) fn qc_for_committed_height(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        committed_height: u64,
    ) -> bool {
        if qc.height > committed_height {
            return false;
        }
        if let Ok(height_usize) = usize::try_from(qc.height)
            && let Some(nz_height) = NonZeroUsize::new(height_usize)
            && let Some(committed) = self.kura.get_block(nz_height)
        {
            let committed_hash = committed.hash();
            if committed_hash != qc.subject_block_hash {
                info!(
                    height = qc.height,
                    view = qc.view,
                    committed_height,
                    committed_hash = %committed_hash,
                    incoming_hash = %qc.subject_block_hash,
                    "dropping QC for already committed height with divergent hash"
                );
                return true;
            }
        } else {
            info!(
                height = qc.height,
                view = qc.view,
                committed_height,
                hash = %qc.subject_block_hash,
                "dropping QC for already committed height with unknown block"
            );
            return true;
        }
        true
    }

    pub(super) fn should_skip_precommit_on_empty_block(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
    ) -> bool {
        if block_known && qc.phase == crate::sumeragi::consensus::Phase::Commit {
            let queue_len = self.queue.tx_len();
            let pending_nonempty = self.has_nonempty_pending_at_height(qc.height);
            if let Some(tx_count) = self.block_tx_count(qc.subject_block_hash) {
                if empty_block_disfavored(tx_count, queue_len, pending_nonempty) {
                    info!(
                        height = qc.height,
                        view = qc.view,
                        queue_len,
                        pending_nonempty,
                        hash = %qc.subject_block_hash,
                        "processing precommit QC for empty block despite queued transactions to stay in sync"
                    );
                }
            }
        }
        false
    }

    pub(super) fn process_prevote_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
    ) {
        self.record_phase_sample(PipelinePhase::CollectPrepare, qc.height, qc.view);
        if block_known {
            let qc_ref = Self::qc_to_header_ref(qc);
            let should_update = self.highest_qc.is_none_or(|current| {
                (qc_ref.height, qc_ref.view) > (current.height, current.view)
            });
            if should_update {
                super::status::set_highest_qc(qc.height, qc.view);
                super::status::set_highest_qc_hash(qc.subject_block_hash);
                self.highest_qc = Some(qc_ref);
            } else {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    highest_height = self.highest_qc.map(|qc| qc.height),
                    highest_view = self.highest_qc.map(|qc| qc.view),
                    "skipping highest QC update for stale prevote QC"
                );
            }
        } else {
            debug!(
                height = qc.height,
                view = qc.view,
                hash = %qc.subject_block_hash,
                "deferring highest QC update until block arrives"
            );
        }
    }

    pub(super) fn process_precommit_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
        allow_nonextending: bool,
    ) -> bool {
        self.record_phase_sample(PipelinePhase::CollectCommit, qc.height, qc.view);
        let qc_ref = Self::qc_to_header_ref(qc);
        if let Some(lock) = self.locked_qc {
            if self.block_known_locally(lock.subject_block_hash) {
                let conflicts_locked = qc.height < lock.height
                    || (qc.height == lock.height
                        && qc.subject_block_hash != lock.subject_block_hash);
                if conflicts_locked {
                    info!(
                        height = qc.height,
                        view = qc.view,
                        locked_height = lock.height,
                        locked_hash = %lock.subject_block_hash,
                        incoming_hash = %qc.subject_block_hash,
                        "ignoring precommit QC that conflicts with locked chain"
                    );
                    return false;
                }
            }
        }
        if block_known {
            let extends_locked = qc_extends_locked_if_present(
                self.locked_qc,
                qc_ref,
                |hash, height| self.parent_hash_for(hash, height),
                |hash| self.block_known_locally(hash),
            );
            if !extends_locked {
                if !allow_nonextending {
                    info!(
                        height = qc.height,
                        view = qc.view,
                        locked_height = ?self.locked_qc.as_ref().map(|lock| lock.height),
                        locked_hash = ?self
                            .locked_qc
                            .as_ref()
                            .map(|lock| lock.subject_block_hash),
                        incoming_hash = %qc.subject_block_hash,
                        "precommit QC does not extend locked chain; dropping QC"
                    );
                    return false;
                }
                info!(
                    height = qc.height,
                    view = qc.view,
                    locked_height = ?self.locked_qc.as_ref().map(|lock| lock.height),
                    locked_hash = ?self
                        .locked_qc
                        .as_ref()
                        .map(|lock| lock.subject_block_hash),
                    incoming_hash = %qc.subject_block_hash,
                    "accepting non-extending precommit QC from block sync to realign locked chain"
                );
            }
            let should_update = self.highest_qc.is_none_or(|current| {
                let incoming = (qc_ref.height, qc_ref.view);
                let existing = (current.height, current.view);
                incoming > existing
                    || (incoming == existing
                        && current.phase != crate::sumeragi::consensus::Phase::Commit)
            });
            if should_update {
                super::status::set_highest_qc(qc.height, qc.view);
                super::status::set_highest_qc_hash(qc.subject_block_hash);
                self.highest_qc = Some(qc_ref);
            } else {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    highest_height = self.highest_qc.map(|qc| qc.height),
                    highest_view = self.highest_qc.map(|qc| qc.view),
                    "skipping highest QC update for stale precommit QC"
                );
            }
            let should_update = self
                .locked_qc
                .is_none_or(|lock| (qc.height, qc.view) > (lock.height, lock.view));
            if should_update {
                super::status::set_locked_qc(qc.height, qc.view, Some(qc.subject_block_hash));
                self.locked_qc = Some(qc_ref);
                self.prune_precommit_votes_conflicting_with_lock(qc_ref);
            }
        } else {
            info!(
                height = qc.height,
                view = qc.view,
                hash = %qc.subject_block_hash,
                "precommit QC arrived before block; cached without updating locks/highest"
            );
        }
        true
    }

    pub(super) fn process_new_view_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        signers: &[ValidatorIndex],
    ) {
        let Some(highest) = qc.highest_cert else {
            warn!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "ignoring NEW_VIEW certificate missing highest certificate reference"
            );
            return;
        };
        if highest.phase != crate::sumeragi::consensus::Phase::Commit {
            warn!(
                height = qc.height,
                view = qc.view,
                highest_height = highest.height,
                highest_view = highest.view,
                phase = ?highest.phase,
                "ignoring NEW_VIEW certificate with non-commit highest certificate"
            );
            return;
        }
        if qc.height != highest.height.saturating_add(1) {
            warn!(
                height = qc.height,
                view = qc.view,
                highest_height = highest.height,
                "ignoring NEW_VIEW certificate with mismatched height"
            );
            return;
        }
        for signer in signers {
            self.subsystems
                .propose
                .new_view_tracker
                .record(qc.height, qc.view, *signer, highest);
        }
    }

    pub(super) fn prune_precommit_votes_conflicting_with_lock(
        &mut self,
        lock: crate::sumeragi::consensus::QcHeaderRef,
    ) {
        let mut drop_keys = Vec::new();
        let mut drop_blocks: BTreeSet<(HashOf<BlockHeader>, u64)> = BTreeSet::new();
        for (key, vote) in &self.vote_log {
            if vote.phase != crate::sumeragi::consensus::Phase::Commit {
                continue;
            }
            if !self.block_known_locally(vote.block_hash) {
                continue;
            }
            let candidate = crate::sumeragi::consensus::QcHeaderRef {
                phase: crate::sumeragi::consensus::Phase::Commit,
                subject_block_hash: vote.block_hash,
                height: vote.height,
                view: vote.view,
                epoch: vote.epoch,
            };
            let extends_locked = qc_extends_locked_with_lookup(lock, candidate, |hash, height| {
                self.parent_hash_for(hash, height)
            });
            if !extends_locked {
                drop_keys.push(*key);
                drop_blocks.insert((vote.block_hash, vote.height));
            }
        }

        if drop_keys.is_empty() {
            return;
        }

        for key in drop_keys {
            self.vote_log.remove(&key);
        }
        self.qc_signer_tally
            .retain(|(phase, hash, height, _, _), _| {
                *phase != crate::sumeragi::consensus::Phase::Commit
                    || !drop_blocks.contains(&(*hash, *height))
            });
        self.qc_cache.retain(|(phase, hash, height, _, _), _| {
            *phase != crate::sumeragi::consensus::Phase::Commit
                || !drop_blocks.contains(&(*hash, *height))
        });

        iroha_logger::debug!(
            locked_height = lock.height,
            locked_hash = %lock.subject_block_hash,
            dropped = drop_blocks.len(),
            "pruned precommit votes that do not extend locked chain"
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_qc(&mut self, qc: crate::sumeragi::consensus::Qc) -> Result<()> {
        // Prepare certificates are view-scoped; commit/new-view certificates can safely arrive
        // after a local view change and still unlock progress, so don't drop them as stale.
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Prepare)
            && self.drop_stale_view(qc.height, qc.view, "QC")
        {
            return Ok(());
        }
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(qc.height);
        let commit_topology = if matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
            self.roster_for_new_view_with_mode(
                qc.subject_block_hash,
                qc.height,
                qc.view,
                consensus_mode,
            )
        } else {
            self.roster_for_vote_with_mode(
                qc.subject_block_hash,
                qc.height,
                qc.view,
                consensus_mode,
            )
        };
        if commit_topology.is_empty() {
            debug!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                "dropping QC: empty commit topology"
            );
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let (validation, evidence) = validate_qc_with_evidence(
            &self.vote_log,
            &qc,
            &topology,
            &self.common_config.chain,
            mode_tag,
            prf_seed,
        );
        let validation = match validation {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Some(outcome) = self.recover_qc_from_aggregate(&qc, &topology, &err) {
                    outcome
                } else {
                    record_qc_validation_error(self.telemetry_handle(), &err);
                    if let Some(evidence) = evidence {
                        let _ = self.handle_evidence(evidence);
                    }
                    warn!(
                        ?err,
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        "rejecting QC without valid signatures"
                    );
                    return Ok(());
                }
            }
        };
        let QcValidationOutcome {
            signers: signer_indices,
            missing_votes,
            present_signers,
        } = validation;
        debug_assert_eq!(
            missing_votes, 0,
            "QC validation should fail when votes are missing"
        );
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            let signer_set: BTreeSet<_> = signer_indices.iter().copied().collect();
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash: qc.subject_block_hash,
                    height: qc.height,
                    view: qc.view,
                    epoch: qc.epoch,
                    signers: signer_set,
                    bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    validator_set: topology.as_ref().to_vec(),
                },
            );
        }
        self.note_validated_qc_tally(
            &qc,
            QcSignerTally {
                voting_signers: signer_indices.iter().copied().collect(),
                present_signers,
            },
        );
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            let phase_label = match qc.phase {
                crate::sumeragi::consensus::Phase::Prepare => "prepare",
                crate::sumeragi::consensus::Phase::Commit => "commit",
                crate::sumeragi::consensus::Phase::NewView => "new_view",
            };
            telemetry.note_qc_signer_counts(phase_label, present_signers, signer_indices.len());
        }
        let committed_height = self.state.view().height();
        let committed_height_u64 = u64::try_from(committed_height).unwrap_or(u64::MAX);
        self.drop_missing_lock_if_unknown(&qc);
        if self.qc_for_committed_height(&qc, committed_height_u64) {
            return Ok(());
        }

        let block_known = self.block_known_locally(qc.subject_block_hash);
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            iroha_logger::debug!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                signers = signer_indices.len(),
                block_known,
                pending = self.pending.pending_blocks.contains_key(&qc.subject_block_hash),
                "processing precommit QC"
            );
        }
        if self.should_skip_precommit_on_empty_block(&qc, block_known) {
            return Ok(());
        }

        if !block_known {
            info!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                hash = %qc.subject_block_hash,
                "received QC for unknown block; caching without updating locks/highest"
            );
            let da_enabled = self.runtime_da_enabled();
            let retry_window = self.quorum_timeout(da_enabled);
            let now = Instant::now();
            let signer_set: BTreeSet<_> = signer_indices.iter().copied().collect();
            let decision = plan_missing_block_fetch(
                &mut self.pending.missing_block_requests,
                qc.subject_block_hash,
                qc.height,
                &signer_set,
                &topology,
                now,
                retry_window,
                self.config.missing_block_signer_fallback_attempts,
            );
            let dwell = self
                .pending
                .missing_block_requests
                .get(&qc.subject_block_hash)
                .map(|stats| now.saturating_duration_since(stats.first_seen))
                .unwrap_or_default();
            let dwell_ms = dwell.as_millis();
            let targets_len = match &decision {
                MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
                _ => 0,
            };
            let dwell_ms_u64 = dwell_ms.try_into().unwrap_or(u64::MAX);
            self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
            match decision {
                MissingBlockFetchDecision::Requested {
                    targets,
                    target_kind,
                } => {
                    self.request_missing_block(qc.subject_block_hash, &targets);
                    iroha_logger::info!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = ?qc.subject_block_hash,
                        targets = ?targets,
                        target_kind = target_kind.label(),
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms,
                        "requested missing block payload after QC arrival"
                    );
                }
                MissingBlockFetchDecision::NoTargets => {
                    iroha_logger::warn!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = ?qc.subject_block_hash,
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms,
                        targets = targets_len,
                        "unable to request missing block payload: no peers available"
                    );
                }
                MissingBlockFetchDecision::Backoff => {
                    iroha_logger::info!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = ?qc.subject_block_hash,
                        retry_window_ms = retry_window.as_millis(),
                        dwell_ms,
                        targets = targets_len,
                        "skipping missing-block fetch during retry backoff window"
                    );
                }
            }
            super::status::record_missing_block_fetch(targets_len, dwell_ms_u64);
        }

        let accepted = match qc.phase {
            crate::sumeragi::consensus::Phase::Prepare => {
                self.process_prevote_qc(&qc, block_known);
                true
            }
            crate::sumeragi::consensus::Phase::Commit => {
                self.process_precommit_qc(&qc, block_known, false)
            }
            crate::sumeragi::consensus::Phase::NewView => {
                self.process_new_view_qc(&qc, &signer_indices);
                true
            }
        };
        if !accepted {
            return Ok(());
        }
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            super::status::record_commit_certificate(qc.clone());
        }
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
        iroha_logger::info!(
            height = qc.height,
            view = qc.view,
            epoch = qc.epoch,
            phase = ?qc.phase,
            block = %qc.subject_block_hash,
            cache_len = self.qc_cache.len(),
            "cached validated QC"
        );
        if !block_known {
            if let Some(lock) = self.locked_qc {
            // Keep status in sync if we cleared an unknown lock earlier.
                super::status::set_locked_qc(lock.height, lock.view, Some(lock.subject_block_hash));
            }
        }
        self.process_commit_candidates();
        Ok(())
    }

    pub(super) fn recover_qc_from_aggregate(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        topology: &super::network_topology::Topology,
        err: &QcValidationError,
    ) -> Option<QcValidationOutcome> {
        let QcValidationError::MissingVotes { .. } = err else {
            return None;
        };
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let _signature_topology =
            super::topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
        if !qc_aggregate_consistent(qc, topology, &self.common_config.chain, mode_tag) {
            return None;
        }
        let roster_len = topology.as_ref().len();
        let parsed_signers = qc_signer_indices(qc, roster_len, roster_len).ok()?;
        info!(
            height = qc.height,
            view = qc.view,
            phase = ?qc.phase,
            block = %qc.subject_block_hash,
            voting_signers = parsed_signers.voting.len(),
            present_signers = parsed_signers.present.len(),
            roster_len,
            "accepting QC validated from aggregate signature despite missing local votes"
        );
        Some(QcValidationOutcome {
            signers: parsed_signers.voting.into_iter().collect(),
            missing_votes: 0,
            present_signers: parsed_signers.present.len(),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_execution_qc(
        &mut self,
        qc: crate::sumeragi::consensus::ExecutionQC,
    ) -> Result<()> {
        let require_exec_qc = self.config.require_execution_qc
            || matches!(
                self.config.proof_policy,
                ProofPolicy::ExecQcOnly | ProofPolicy::Hybrid
            )
            || self.config.require_wsv_exec_qc;
        if let Some(local_view) = self.stale_view(qc.height, qc.view) {
            let block_known = self.block_known_locally(qc.subject_block_hash);
            let missing_request = self
                .pending
                .missing_block_requests
                .contains_key(&qc.subject_block_hash);
            if block_known || missing_request || require_exec_qc {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    local_view,
                    block = %qc.subject_block_hash,
                    block_known,
                    missing_request,
                    require_exec_qc,
                    "accepting ExecutionQC for stale view"
                );
            } else {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    local_view,
                    kind = "ExecutionQC",
                    "dropping consensus message for stale view"
                );
                return Ok(());
            }
        }
        self.record_phase_sample(PipelinePhase::CollectExec, qc.height, qc.view);
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(qc.height);
        let topology_peers = self.roster_for_vote_with_mode(
            qc.subject_block_hash,
            qc.height,
            qc.view,
            consensus_mode,
        );
        if topology_peers.is_empty() {
            debug!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "dropping ExecutionQC: empty commit topology"
            );
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        if let Err(err) = super::validate_execution_qc(
            &qc,
            &topology,
            &self.common_config.chain,
            mode_tag,
            prf_seed,
        ) {
            warn!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                ?err,
                "dropping invalid ExecutionQC"
            );
            return Ok(());
        }
        let topology_len = topology.as_ref().len();
        let subject = qc.subject_block_hash;
        let parsed_signers = match super::parse_signers_bitmap(
            &qc.aggregate.signers_bitmap,
            topology_len,
            topology_len,
        ) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    height = qc.height,
                    view = qc.view,
                    block = %subject,
                    ?err,
                    "dropping ExecutionQC: invalid signer bitmap"
                );
                return Ok(());
            }
        };
        self.defer_qc_if_block_missing(
            crate::sumeragi::consensus::Phase::Commit,
            subject,
            qc.height,
            qc.view,
            &parsed_signers.voting,
            &topology,
        );
        if self.config.require_wsv_exec_qc {
            self.persist_execution_qc(&qc)?;
        }
        self.execution_qc_cache.insert(subject, qc);
        Ok(())
    }

    pub(super) fn handle_exec_witness(&self, witness: crate::sumeragi::consensus::ExecWitnessMsg) {
        if self.drop_stale_view(witness.height, witness.view, "ExecWitness") {
            return;
        }
        let _ = self
            .events_sender
            .send(EventBox::Pipeline(PipelineEventBox::Witness(witness)));
    }
}
