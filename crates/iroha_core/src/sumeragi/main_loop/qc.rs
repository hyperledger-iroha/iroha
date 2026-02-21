//! Vote/QC accumulation, validation, and processing helpers.

use iroha_crypto::Hash;
use iroha_logger::prelude::*;

use super::locked_qc::qc_extends_locked_with_lookup;
use super::*;

#[derive(Debug)]
struct QcSignerSnapshot {
    signers: BTreeSet<ValidatorIndex>,
    voting_signers: usize,
    total_signers: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct QcSignerFilterStats {
    raw_votes: usize,
    invalid_signature: usize,
    roster_mismatch: usize,
    higher_view_filtered: usize,
}

impl QcSignerFilterStats {
    fn filtered_total(self) -> usize {
        self.invalid_signature + self.roster_mismatch + self.higher_view_filtered
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommittedQcDecision {
    Continue,
    RecordOnly,
    Drop,
}

/// For small committees, worker handoff can dominate the aggregate verification cost.
/// Keep verification inline to avoid unnecessary queueing latency on the hot path.
const QC_VERIFY_INLINE_ROSTER_MAX: usize = 8;

pub(super) fn select_commit_root_signers(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
) -> (BTreeSet<crate::sumeragi::consensus::ValidatorIndex>, usize) {
    if signers.is_empty() {
        return (BTreeSet::new(), 0);
    }
    let mut groups: BTreeMap<(Hash, Hash), BTreeSet<crate::sumeragi::consensus::ValidatorIndex>> =
        BTreeMap::new();
    for signer in signers {
        let key = (
            crate::sumeragi::consensus::Phase::Commit,
            height,
            view,
            epoch,
            *signer,
        );
        let Some(vote) = vote_log.get(&key) else {
            continue;
        };
        if vote.block_hash != block_hash {
            continue;
        }
        groups
            .entry((vote.parent_state_root, vote.post_state_root))
            .or_default()
            .insert(*signer);
    }
    let group_count = groups.len();
    let mut selected: Option<(
        &(Hash, Hash),
        &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    )> = None;
    for (root, group) in &groups {
        let replace = match selected {
            None => true,
            Some((best_root, best_group)) => {
                let size_cmp = group.len().cmp(&best_group.len());
                size_cmp.is_gt() || (size_cmp.is_eq() && root < best_root)
            }
        };
        if replace {
            selected = Some((root, group));
        }
    }
    let filtered = selected.map(|(_, group)| group.clone()).unwrap_or_default();
    (filtered, group_count)
}

impl Actor {
    pub(super) fn missing_block_retry_window_with_rbc_progress(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        base_retry_window: Duration,
    ) -> Duration {
        if !self.runtime_da_enabled() || base_retry_window == Duration::ZERO {
            return base_retry_window;
        }
        let key = Self::session_key(&block_hash, height, view);
        let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) else {
            return base_retry_window;
        };
        if session.is_invalid() || session.delivered {
            return base_retry_window;
        }
        let chunks_progressed = session.total_chunks() > 0 && session.received_chunks() > 0;
        let ready_progressed = !session.ready_signatures.is_empty();
        let pending_payload = self.subsystems.da_rbc.rbc.pending.contains_key(&key);
        if !chunks_progressed && !ready_progressed && !pending_payload {
            return base_retry_window;
        }
        let availability_timeout = self.availability_timeout(
            self.quorum_timeout(self.runtime_da_enabled()),
            self.runtime_da_enabled(),
        );
        let widened = super::saturating_mul_duration(base_retry_window, 2);
        widened.min(availability_timeout.max(base_retry_window))
    }

    fn adaptive_retry_window_for_missing_block_request(
        &self,
        block_hash: HashOf<BlockHeader>,
        stats: &super::MissingBlockRequest,
    ) -> Duration {
        let mut retry_window = if stats.retry_window == Duration::ZERO {
            self.rebroadcast_cooldown()
        } else {
            stats.retry_window
        };

        // After the first aggressive pull, relax to normal control-plane cadence to
        // avoid repeatedly flooding fetch requests while the payload is still in-flight.
        if matches!(stats.priority, super::MissingBlockPriority::Consensus)
            && matches!(stats.phase, crate::sumeragi::consensus::Phase::Commit)
            && stats.attempts > 0
            && stats.retry_window <= super::REBROADCAST_COOLDOWN_FLOOR
        {
            retry_window = retry_window.max(self.rebroadcast_cooldown());
        }

        let widened = self.missing_block_retry_window_with_rbc_progress(
            block_hash,
            stats.height,
            stats.view,
            retry_window,
        );
        retry_window.max(widened)
    }

    fn missing_block_retry_window_with_backoff(
        &self,
        base_retry_window: Duration,
        attempts: u32,
    ) -> Duration {
        if base_retry_window == Duration::ZERO {
            return base_retry_window;
        }

        let multiplier = self.recovery_missing_block_retry_backoff_multiplier();
        let cap = self
            .recovery_missing_block_retry_backoff_cap()
            .max(base_retry_window);
        if attempts == 0 || multiplier <= 1 {
            return base_retry_window.min(cap);
        }

        super::saturating_mul_duration(base_retry_window, multiplier).min(cap)
    }

    fn maybe_validate_pending_for_commit_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        commit_topology: &[PeerId],
    ) -> bool {
        if commit_topology.is_empty() {
            return false;
        }
        let hash = qc.subject_block_hash;
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|pending| pending == hash)
        {
            return false;
        }
        if self.subsystems.validation.inflight.contains_key(&hash) {
            return false;
        }
        let needs_validation = match self.pending.pending_blocks.get(&hash) {
            Some(pending)
                if !pending.aborted && pending.validation_status == ValidationStatus::Pending =>
            {
                let state_height = self.state.committed_height();
                let tip_hash = self.state.latest_block_hash_fast();
                pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    state_height,
                    tip_hash,
                )
            }
            None => return false,
            Some(_) => false,
        };
        if !needs_validation {
            return false;
        }
        let outcome = self.validate_pending_block_for_voting_inline(hash, commit_topology);
        match outcome {
            ValidationGateOutcome::Valid => true,
            ValidationGateOutcome::Deferred => false,
            ValidationGateOutcome::Invalid {
                hash,
                height,
                view,
                evidence,
                reason,
                reason_label,
            } => {
                self.handle_validation_reject(hash, height, view, evidence, reason, reason_label);
                false
            }
        }
    }

    fn defer_qc_for_roster(&mut self, qc: crate::sumeragi::consensus::Qc, reason: &'static str) {
        let key = Self::qc_tally_key(&qc);
        let now = Instant::now();
        if self.deferred_qcs.contains_key(&key) {
            if let Some(entry) = self.deferred_qc_roster_state.get_mut(&key) {
                entry.last_attempt = now;
                entry.reason = reason;
            }
            debug!(
                phase = ?qc.phase,
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                reason,
                "deferring duplicate QC while roster is unresolved"
            );
            return;
        }
        let phase = qc.phase;
        let height = qc.height;
        let view = qc.view;
        let block_hash = qc.subject_block_hash;
        self.deferred_qcs.insert(key, qc);
        self.deferred_qc_roster_state.insert(
            key,
            DeferredRosterQcEntry {
                first_seen: now,
                last_attempt: now,
                attempts: 0,
                escalated_fetch: false,
                reason,
            },
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::RosterMissing,
        );
        info!(
            phase = ?phase,
            height,
            view,
            block = %block_hash,
            deferred = self.deferred_qcs.len(),
            reason,
            "deferring QC until commit roster is available"
        );
    }

    fn defer_qc_for_missing_payload(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        reason: &'static str,
    ) {
        let key = Self::qc_tally_key(qc);
        let now = Instant::now();
        if let Some(existing) = self.deferred_missing_payload_qcs.get_mut(&key) {
            existing.qc = qc.clone();
            existing.last_attempt = now;
            debug!(
                phase = ?qc.phase,
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                reason,
                "refreshing deferred QC while payload remains unavailable"
            );
            return;
        }

        if self.deferred_missing_payload_qcs.len() >= DEFERRED_MISSING_PAYLOAD_QC_CAP {
            let oldest = self
                .deferred_missing_payload_qcs
                .iter()
                .min_by_key(|(key, entry)| (entry.first_seen, **key))
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.deferred_missing_payload_qcs.remove(&oldest);
                super::status::inc_qc_deferred_expired();
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = self.telemetry_handle() {
                    telemetry.inc_qc_deferred_expired();
                }
            }
        }

        self.deferred_missing_payload_qcs.insert(
            key,
            DeferredQcEntry {
                qc: qc.clone(),
                first_seen: now,
                last_attempt: now,
                attempts: 0,
                escalated_fetch: false,
                reason,
            },
        );
        super::status::inc_qc_deferred_missing_payload();
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_qc_deferred_missing_payload();
        }
        info!(
            phase = ?qc.phase,
            height = qc.height,
            view = qc.view,
            block = %qc.subject_block_hash,
            deferred = self.deferred_missing_payload_qcs.len(),
            reason,
            "deferring QC until block payload is available locally"
        );
    }

    fn force_targeted_missing_payload_fetch_for_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        reason: &'static str,
    ) {
        let (consensus_mode, _, _) = self.consensus_context_for_height(qc.height);
        let mut roster = if matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
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
        if roster.is_empty() {
            roster = self.effective_commit_topology();
        }
        if roster.is_empty() {
            return;
        }
        let topology = super::network_topology::Topology::new(roster);
        let signer_set =
            super::qc_signer_indices(qc, topology.as_ref().len(), topology.as_ref().len())
                .map(|parsed| parsed.voting.into_iter().collect::<BTreeSet<_>>())
                .unwrap_or_default();
        let targets = Self::build_fetch_targets(&signer_set, &topology);
        if targets.is_empty() {
            return;
        }
        self.request_missing_block(
            qc.subject_block_hash,
            qc.height,
            qc.view,
            super::MissingBlockPriority::Consensus,
            &targets,
        );
        info!(
            phase = ?qc.phase,
            height = qc.height,
            view = qc.view,
            block = %qc.subject_block_hash,
            targets = targets.len(),
            reason,
            "forcing targeted missing-block fetch for deferred QC"
        );
    }

    pub(super) fn try_replay_deferred_qcs(&mut self) -> bool {
        if self.deferred_qcs.is_empty() {
            self.deferred_qc_roster_state.clear();
            return false;
        }
        let now = Instant::now();

        enum ReplayAction {
            Replay {
                key: QcVoteKey,
                block_hash: HashOf<BlockHeader>,
                roster: Vec<PeerId>,
            },
            Escalate {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
            },
            Expire {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
            },
        }

        let mut actions = Vec::new();
        for key in self
            .deferred_qcs
            .keys()
            .cloned()
            .take(DEFERRED_MISSING_PAYLOAD_QC_PER_TICK)
        {
            let Some(qc) = self.deferred_qcs.get(&key) else {
                continue;
            };
            let (consensus_mode, _, _) = self.consensus_context_for_height(qc.height);
            let roster = if matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
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
            if !roster.is_empty() {
                actions.push(ReplayAction::Replay {
                    key,
                    block_hash: qc.subject_block_hash,
                    roster,
                });
                continue;
            }
            let Some(state) = self.deferred_qc_roster_state.get(&key) else {
                continue;
            };
            let age = now.saturating_duration_since(state.first_seen);
            if age < self.recovery_deferred_qc_ttl() {
                continue;
            }
            if state.escalated_fetch || state.attempts >= DEFERRED_MISSING_PAYLOAD_QC_MAX_ATTEMPTS {
                actions.push(ReplayAction::Expire {
                    key,
                    qc: qc.clone(),
                    reason: state.reason,
                });
            } else {
                actions.push(ReplayAction::Escalate {
                    key,
                    qc: qc.clone(),
                    reason: state.reason,
                });
            }
        }
        if actions.is_empty() {
            return false;
        }

        let mut progress = false;
        for action in actions {
            match action {
                ReplayAction::Replay {
                    key,
                    block_hash,
                    roster,
                } => {
                    self.cache_vote_roster(block_hash, key.2, key.3, roster);
                    self.deferred_qc_roster_state.remove(&key);
                    if let Some(qc) = self.deferred_qcs.remove(&key) {
                        if let Err(err) = self.handle_qc(qc) {
                            warn!(?err, "failed to replay deferred QC");
                        } else {
                            progress = true;
                        }
                    }
                }
                ReplayAction::Escalate { key, qc, reason } => {
                    if let Some(state) = self.deferred_qc_roster_state.get_mut(&key) {
                        state.escalated_fetch = true;
                        state.attempts = state.attempts.saturating_add(1);
                        state.first_seen = now;
                        state.last_attempt = now;
                    }
                    self.force_targeted_missing_payload_fetch_for_qc(&qc, reason);
                    progress = true;
                }
                ReplayAction::Expire { key, qc, reason } => {
                    self.deferred_qcs.remove(&key);
                    self.deferred_qc_roster_state.remove(&key);
                    self.force_targeted_missing_payload_fetch_for_qc(&qc, reason);
                    super::status::inc_qc_deferred_expired();
                    #[cfg(feature = "telemetry")]
                    if let Some(telemetry) = self.telemetry_handle() {
                        telemetry.inc_qc_deferred_expired();
                    }
                    let current_view = self.phase_tracker.current_view(qc.height).unwrap_or(0);
                    self.trigger_view_change_with_cause(
                        qc.height,
                        current_view,
                        super::ViewChangeCause::MissingQc,
                    );
                    progress = true;
                }
            }
        }

        progress
    }

    pub(super) fn try_replay_deferred_missing_payload_qcs(&mut self, now: Instant) -> bool {
        if self.deferred_missing_payload_qcs.is_empty() {
            return false;
        }

        enum ReplayAction {
            Replay {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
            },
            Escalate {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
            },
            Expire {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
            },
        }

        let mut actions = Vec::new();
        for key in self
            .deferred_missing_payload_qcs
            .keys()
            .cloned()
            .take(DEFERRED_MISSING_PAYLOAD_QC_PER_TICK)
        {
            let Some(entry) = self.deferred_missing_payload_qcs.get(&key) else {
                continue;
            };
            if self.block_known_locally(entry.qc.subject_block_hash) {
                actions.push(ReplayAction::Replay {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                });
                continue;
            }
            let age = now.saturating_duration_since(entry.first_seen);
            if age < self.recovery_deferred_qc_ttl() {
                continue;
            }
            if entry.escalated_fetch || entry.attempts >= DEFERRED_MISSING_PAYLOAD_QC_MAX_ATTEMPTS {
                actions.push(ReplayAction::Expire {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                });
            } else {
                actions.push(ReplayAction::Escalate {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                });
            }
        }

        if actions.is_empty() {
            return false;
        }

        let mut progress = false;
        for action in actions {
            match action {
                ReplayAction::Replay { key, qc, reason } => {
                    self.deferred_missing_payload_qcs.remove(&key);
                    match self.handle_qc(qc.clone()) {
                        Ok(()) => {
                            super::status::inc_qc_deferred_resolved();
                            #[cfg(feature = "telemetry")]
                            if let Some(telemetry) = self.telemetry_handle() {
                                telemetry.inc_qc_deferred_resolved();
                            }
                            progress = true;
                        }
                        Err(err) => {
                            warn!(?err, reason, "failed to replay deferred missing-payload QC");
                            super::status::inc_qc_deferred_expired();
                            #[cfg(feature = "telemetry")]
                            if let Some(telemetry) = self.telemetry_handle() {
                                telemetry.inc_qc_deferred_expired();
                            }
                            let current_view =
                                self.phase_tracker.current_view(qc.height).unwrap_or(0);
                            self.trigger_view_change_with_cause(
                                qc.height,
                                current_view,
                                super::ViewChangeCause::MissingPayload,
                            );
                            progress = true;
                        }
                    }
                }
                ReplayAction::Escalate { key, qc, reason } => {
                    if let Some(entry) = self.deferred_missing_payload_qcs.get_mut(&key) {
                        entry.escalated_fetch = true;
                        entry.attempts = entry.attempts.saturating_add(1);
                        entry.first_seen = now;
                        entry.last_attempt = now;
                    }
                    self.force_targeted_missing_payload_fetch_for_qc(&qc, reason);
                    progress = true;
                }
                ReplayAction::Expire { key, qc, reason } => {
                    self.deferred_missing_payload_qcs.remove(&key);
                    self.force_targeted_missing_payload_fetch_for_qc(&qc, reason);
                    super::status::inc_qc_deferred_expired();
                    #[cfg(feature = "telemetry")]
                    if let Some(telemetry) = self.telemetry_handle() {
                        telemetry.inc_qc_deferred_expired();
                    }
                    let current_view = self.phase_tracker.current_view(qc.height).unwrap_or(0);
                    self.trigger_view_change_with_cause(
                        qc.height,
                        current_view,
                        super::ViewChangeCause::MissingPayload,
                    );
                    progress = true;
                }
            }
        }

        progress
    }

    pub(super) fn request_missing_block(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        priority: super::MissingBlockPriority,
        targets: &[PeerId],
    ) {
        send_missing_block_request(
            &self.network,
            &self.common_config.peer.id,
            block_hash,
            height,
            view,
            priority,
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
        self.qc_signers_for_votes_with_stats(
            phase,
            block_hash,
            height,
            view,
            epoch,
            signature_topology,
        )
        .0
    }

    fn qc_signers_for_votes_with_stats(
        &self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signature_topology: &super::network_topology::Topology,
    ) -> (BTreeSet<ValidatorIndex>, QcSignerFilterStats) {
        let chain_id = &self.common_config.chain;
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let roster_hash = HashOf::new(&signature_topology.as_ref().to_vec());
        let canonical_roster = super::roster::canonicalize_roster_for_mode(
            signature_topology.as_ref().to_vec(),
            consensus_mode,
        );
        let membership_hash = HashOf::new(&canonical_roster);
        let canonical_topology = super::network_topology::Topology::new(canonical_roster);
        let mut topology_by_view: BTreeMap<u64, super::network_topology::Topology> =
            BTreeMap::new();
        let canonical_signer_for_view = |signer: ValidatorIndex,
                                         view_topology: &super::network_topology::Topology|
         -> Option<ValidatorIndex> {
            let mut view_signers = BTreeSet::new();
            view_signers.insert(signer);
            let canonical = super::normalize_signer_indices_to_canonical(
                &view_signers,
                view_topology,
                &canonical_topology,
            );
            if canonical.len() != 1 {
                return None;
            }
            canonical.into_iter().next()
        };
        let mut highest_view_by_signer: BTreeMap<ValidatorIndex, u64> = BTreeMap::new();
        for stored in self.vote_log.values() {
            if stored.phase != phase || stored.height != height || stored.epoch != epoch {
                continue;
            }
            let key = (
                stored.phase,
                stored.height,
                stored.view,
                stored.epoch,
                stored.signer,
            );
            let view_topology = topology_by_view.entry(stored.view).or_insert_with(|| {
                topology_for_view(&canonical_topology, height, stored.view, mode_tag, prf_seed)
            });
            let expected_roster_hash = HashOf::new(&view_topology.as_ref().to_vec());
            let expected_membership_hash =
                HashOf::new(&super::roster::canonicalize_roster_for_mode(
                    view_topology.as_ref().to_vec(),
                    consensus_mode,
                ));
            let cache_matches = self.vote_validation_cache.get(&key).is_some_and(|entry| {
                entry.membership_hash == expected_membership_hash
                    && entry.roster_hash == expected_roster_hash
            });
            if !cache_matches && !vote_signature_valid(stored, view_topology, chain_id, mode_tag) {
                continue;
            }
            let Some(canonical_signer) = canonical_signer_for_view(stored.signer, view_topology)
            else {
                continue;
            };
            let entry = highest_view_by_signer
                .entry(canonical_signer)
                .or_insert(stored.view);
            if stored.view > *entry {
                *entry = stored.view;
            }
        }
        let mut stats = QcSignerFilterStats::default();
        let mut signers = BTreeSet::new();
        for vote in self.vote_log.values().filter(|stored| {
            stored.phase == phase
                && stored.block_hash == block_hash
                && stored.height == height
                && stored.view == view
                && stored.epoch == epoch
        }) {
            stats.raw_votes = stats.raw_votes.saturating_add(1);
            let key = (vote.phase, vote.height, vote.view, vote.epoch, vote.signer);
            if self
                .vote_validation_cache
                .get(&key)
                .is_some_and(|entry| entry.membership_hash != membership_hash)
            {
                stats.roster_mismatch = stats.roster_mismatch.saturating_add(1);
                continue;
            }
            let cache_matches = self.vote_validation_cache.get(&key).is_some_and(|entry| {
                entry.membership_hash == membership_hash && entry.roster_hash == roster_hash
            });
            if !cache_matches && !vote_signature_valid(vote, signature_topology, chain_id, mode_tag)
            {
                stats.invalid_signature = stats.invalid_signature.saturating_add(1);
                continue;
            }
            let Some(canonical_signer) = canonical_signer_for_view(vote.signer, signature_topology)
            else {
                stats.invalid_signature = stats.invalid_signature.saturating_add(1);
                continue;
            };
            if highest_view_by_signer
                .get(&canonical_signer)
                .copied()
                .unwrap_or(vote.view)
                != view
            {
                stats.higher_view_filtered = stats.higher_view_filtered.saturating_add(1);
                continue;
            }
            signers.insert(vote.signer);
        }
        (signers, stats)
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
        self.defer_qc_if_block_missing_with_quorum_hint(
            phase, block_hash, height, view, signers, topology, /*commit_quorum_met*/ false,
        )
    }

    pub(super) fn defer_qc_if_block_missing_with_quorum_hint(
        &mut self,
        phase: crate::sumeragi::consensus::Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        signers: &BTreeSet<ValidatorIndex>,
        topology: &super::network_topology::Topology,
        commit_quorum_met: bool,
    ) -> bool {
        let base_retry_window = self.rebroadcast_cooldown();
        let commit_quorum = topology.min_votes_for_commit().max(1);
        let voting_signers = voting_signer_count(signers, topology.as_ref().len());
        let near_commit_quorum = voting_signers > 0
            && voting_signers < commit_quorum
            && voting_signers.saturating_add(1) >= commit_quorum;
        let aggressive_qc_fetch = matches!(phase, crate::sumeragi::consensus::Phase::Commit)
            && (commit_quorum_met || voting_signers >= commit_quorum || near_commit_quorum);
        let existing_attempts = self
            .pending
            .missing_block_requests
            .get(&block_hash)
            .map_or(0, |stats| stats.attempts);
        let aggressive_retry_floor = aggressive_qc_fetch && existing_attempts == 0;
        let retry_window = if aggressive_retry_floor {
            super::REBROADCAST_COOLDOWN_FLOOR
        } else {
            self.missing_block_retry_window_with_rbc_progress(
                block_hash,
                height,
                view,
                base_retry_window,
            )
        };
        if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
            if aggressive_retry_floor && stats.attempts == 0 {
                if stats.retry_window == Duration::ZERO || retry_window < stats.retry_window {
                    stats.retry_window = retry_window;
                }
            } else if retry_window > stats.retry_window {
                stats.retry_window = retry_window;
            }
        }
        let defer_view_change =
            self.should_defer_missing_block_view_change(&block_hash, height, view);
        // Retry missing payload fetches on the rebroadcast cadence while gating escalation with
        // the configured deferred-QC TTL.
        let view_change_window = if defer_view_change {
            None
        } else {
            Some(self.recovery_deferred_qc_ttl())
        };
        let peer_id = self.common_config.peer.id.clone();
        let network = self.network.clone();
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let telemetry = self.telemetry_handle();
        let now = Instant::now();
        let fetch_mode = if aggressive_qc_fetch {
            if aggressive_retry_floor {
                crate::sumeragi::status::inc_qc_missing_payload_aggressive_fetch();
            }
            super::MissingBlockFetchMode::AggressiveTopology
        } else {
            super::MissingBlockFetchMode::Default
        };
        let deferred = super::defer_qc_for_missing_block_with_mode(
            self.block_payload_available_locally(block_hash),
            retry_window,
            view_change_window,
            now,
            block_hash,
            height,
            view,
            phase,
            signers,
            topology,
            self.recovery_signer_fallback_attempts(),
            &mut requests,
            telemetry,
            fetch_mode,
            move |targets| {
                send_missing_block_request(
                    &network,
                    &peer_id,
                    block_hash,
                    height,
                    view,
                    super::MissingBlockPriority::Consensus,
                    targets,
                );
            },
        );
        self.pending.missing_block_requests = requests;
        if defer_view_change {
            self.clear_missing_block_view_change(&block_hash);
        }
        if deferred {
            let view_change_state =
                self.pending
                    .missing_block_requests
                    .get(&block_hash)
                    .map(|stats| {
                        let view_change_due = stats
                            .view_change_window
                            .filter(|window| *window != Duration::ZERO)
                            .is_some_and(|window| {
                                !stats.view_change_triggered_in_view()
                                    && now.saturating_duration_since(stats.first_seen) >= window
                            });
                        (view_change_due, stats.first_seen, stats.attempts)
                    });
            let should_defer = view_change_state.is_some_and(|(due, _, _)| due)
                && self.should_defer_missing_block_view_change(&block_hash, height, view);
            if should_defer {
                let queue_depths = super::status::worker_queue_depth_snapshot();
                if let Some((_due, first_seen, attempts)) = view_change_state {
                    debug!(
                        height,
                        view,
                        dwell_ms = now.saturating_duration_since(first_seen).as_millis(),
                        attempts,
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        "missing block dwell exceeded view-change window; deferring view change while payload backlog is unresolved"
                    );
                }
            } else if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
                if stats.mark_view_change_if_due(now) {
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

    #[allow(clippy::too_many_lines)]
    pub(super) fn retry_missing_block_requests(
        &mut self,
        now: Instant,
        tick_deadline: Option<Instant>,
    ) -> bool {
        self.prune_missing_block_recovery_state(now);
        if self.pending.missing_block_requests.is_empty() {
            return false;
        }

        let mut progress = false;
        let mut active_roster_permissioned: Option<Vec<PeerId>> = None;
        let mut active_roster_npos: Option<Vec<PeerId>> = None;
        let pending_keys: Vec<_> = self
            .pending
            .missing_block_requests
            .keys()
            .copied()
            .collect();
        for block_hash in pending_keys {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            if self.block_payload_available_locally(block_hash) {
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
                progress = true;
                continue;
            }

            let stats_snapshot = match self.pending.missing_block_requests.get(&block_hash) {
                Some(stats) => stats.clone(),
                None => continue,
            };
            let expected_epoch = self.epoch_for_height(stats_snapshot.height);
            let mut signers = BTreeSet::new();
            let mut roster: Option<Vec<PeerId>> = None;
            if let Some(qc) = super::cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                stats_snapshot.height,
                stats_snapshot.view,
                expected_epoch,
            )
            .or_else(|| {
                super::cached_qc_for(
                    &self.qc_cache,
                    stats_snapshot.phase,
                    block_hash,
                    stats_snapshot.height,
                    stats_snapshot.view,
                    expected_epoch,
                )
            }) {
                if qc.validator_set.is_empty() {
                    warn!(
                        height = stats_snapshot.height,
                        view = stats_snapshot.view,
                        phase = ?stats_snapshot.phase,
                        block = ?block_hash,
                        "skipping QC roster for missing-block retry: empty validator set"
                    );
                } else {
                    let roster_len = qc.validator_set.len();
                    match super::qc_signer_indices(&qc, roster_len, roster_len) {
                        Ok(parsed) => {
                            signers = parsed.voting;
                        }
                        Err(err) => {
                            warn!(
                                height = stats_snapshot.height,
                                view = stats_snapshot.view,
                                phase = ?stats_snapshot.phase,
                                block = ?block_hash,
                                ?err,
                                "failed to parse QC signer bitmap for missing-block retry"
                            );
                        }
                    }
                    roster = Some(qc.validator_set.clone());
                }
            }
            if roster.is_none() {
                if let Some(snapshot) = self
                    .state
                    .commit_roster_snapshot_for_block(stats_snapshot.height, block_hash)
                {
                    if !snapshot.commit_qc.validator_set.is_empty() {
                        roster = Some(snapshot.commit_qc.validator_set.clone());
                    }
                }
            }
            if roster.is_none() {
                let (consensus_mode, _, _) =
                    self.consensus_context_for_height(stats_snapshot.height);
                let cache = match consensus_mode {
                    ConsensusMode::Permissioned => &mut active_roster_permissioned,
                    ConsensusMode::Npos => &mut active_roster_npos,
                };
                if cache.is_none() {
                    let world = self.state.world_view();
                    let commit_topology = self.state.commit_topology_snapshot();
                    let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                    *cache = Some(self.active_topology_with_genesis_fallback_from_world(
                        &world,
                        commit_topology.as_slice(),
                        height,
                        consensus_mode,
                    ));
                }
                if let Some(active) = cache.as_ref().filter(|active| !active.is_empty()) {
                    roster = Some(active.clone());
                }
            }
            let topology = roster
                .filter(|roster| !roster.is_empty())
                .map(super::network_topology::Topology::new);

            let defer_view_change = self.should_defer_missing_block_view_change(
                &block_hash,
                stats_snapshot.height,
                stats_snapshot.view,
            );
            let retry_window =
                self.adaptive_retry_window_for_missing_block_request(block_hash, &stats_snapshot);
            if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash)
                && retry_window != stats.retry_window
            {
                stats.retry_window = retry_window;
            }
            let effective_retry_window =
                self.missing_block_retry_window_with_backoff(retry_window, stats_snapshot.attempts);
            let mut view_change_window = self
                .pending
                .missing_block_requests
                .get(&block_hash)
                .map_or(stats_snapshot.view_change_window, |stats| {
                    stats.view_change_window
                });
            if defer_view_change {
                view_change_window = None;
            } else if view_change_window.is_none()
                && matches!(
                    stats_snapshot.priority,
                    super::MissingBlockPriority::Consensus
                )
            {
                view_change_window = Some(self.recovery_deferred_qc_ttl());
            }
            let decision = if let Some(ref topology) = topology {
                let since_last_request =
                    now.saturating_duration_since(stats_snapshot.last_requested);
                if since_last_request < effective_retry_window {
                    MissingBlockFetchDecision::Backoff
                } else {
                    let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
                    super::plan_missing_block_fetch(
                        &mut self.pending.missing_block_requests,
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        stats_snapshot.phase,
                        stats_snapshot.priority,
                        &signers,
                        topology,
                        now,
                        retry_window,
                        view_change_window,
                        signer_fallback_attempts,
                    )
                }
            } else {
                let since_last_request =
                    now.saturating_duration_since(stats_snapshot.last_requested);
                if since_last_request < effective_retry_window {
                    MissingBlockFetchDecision::Backoff
                } else {
                    let retry_due = super::touch_missing_block_request(
                        &mut self.pending.missing_block_requests,
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        stats_snapshot.phase,
                        stats_snapshot.priority,
                        now,
                        retry_window,
                        view_change_window,
                    );
                    if retry_due {
                        MissingBlockFetchDecision::NoTargets
                    } else {
                        MissingBlockFetchDecision::Backoff
                    }
                }
            };
            if defer_view_change {
                self.clear_missing_block_view_change(&block_hash);
            }

            let (dwell, since_last_request, attempts) =
                self.pending.missing_block_requests.get(&block_hash).map_or(
                    (Duration::ZERO, Duration::ZERO, 0),
                    |stats: &MissingBlockRequest| {
                        (
                            now.saturating_duration_since(stats.first_seen),
                            now.saturating_duration_since(stats.last_requested),
                            stats.attempts,
                        )
                    },
                );
            let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
            let since_last_ms = since_last_request
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);
            let retry_window_ms = retry_window.as_millis();
            let effective_retry_window_ms = effective_retry_window.as_millis();
            let targets_len = match &decision {
                MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
                _ => 0,
            };

            self.note_missing_block_fetch_metrics(
                &decision,
                effective_retry_window,
                targets_len,
                dwell,
            );
            super::status::record_missing_block_fetch(targets_len, dwell_ms);

            match &decision {
                MissingBlockFetchDecision::Requested {
                    targets,
                    target_kind,
                } => {
                    self.request_missing_block(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        stats_snapshot.priority,
                        &targets,
                    );
                    iroha_logger::info!(
                        height = stats_snapshot.height,
                        view = stats_snapshot.view,
                        phase = ?stats_snapshot.phase,
                        block = ?block_hash,
                        targets = ?targets,
                        target_kind = target_kind.label(),
                        retry_window_ms = effective_retry_window_ms,
                        base_retry_window_ms = retry_window_ms,
                        dwell_ms,
                        since_last_ms,
                        attempts,
                        "retrying missing block fetch"
                    );
                    progress = true;
                }
                MissingBlockFetchDecision::NoTargets => {
                    iroha_logger::warn!(
                        height = stats_snapshot.height,
                        view = stats_snapshot.view,
                        phase = ?stats_snapshot.phase,
                        block = ?block_hash,
                        retry_window_ms = effective_retry_window_ms,
                        base_retry_window_ms = retry_window_ms,
                        dwell_ms,
                        since_last_ms,
                        attempts,
                        "unable to retry missing block fetch: no peers available"
                    );
                }
                MissingBlockFetchDecision::Backoff => {
                    iroha_logger::debug!(
                        height = stats_snapshot.height,
                        view = stats_snapshot.view,
                        phase = ?stats_snapshot.phase,
                        block = ?block_hash,
                        retry_window_ms = effective_retry_window_ms,
                        base_retry_window_ms = retry_window_ms,
                        dwell_ms,
                        since_last_ms,
                        attempts,
                        "missing-block retry still in backoff"
                    );
                }
            }
            match &decision {
                MissingBlockFetchDecision::Requested { target_kind, .. } => {
                    self.note_missing_block_height_attempt(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        super::MissingBlockRecoveryStage::HashFetch,
                        Some(*target_kind),
                        now,
                    );
                }
                MissingBlockFetchDecision::NoTargets => {
                    self.note_missing_block_height_attempt(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        super::MissingBlockRecoveryStage::HashFetch,
                        None,
                        now,
                    );
                }
                MissingBlockFetchDecision::Backoff => {
                    self.note_missing_block_height_hash_miss(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        super::MissingBlockRecoveryStage::HashFetch,
                        now,
                    );
                }
            }
            if self.maybe_escalate_missing_block_height_recovery(
                block_hash,
                stats_snapshot.height,
                stats_snapshot.view,
                now,
            ) {
                progress = true;
                continue;
            }

            let view_change_due = self
                .pending
                .missing_block_requests
                .get(&block_hash)
                .and_then(|stats| {
                    stats
                        .view_change_window
                        .filter(|window| *window != Duration::ZERO)
                        .map(|window| {
                            (
                                !stats.view_change_triggered_in_view()
                                    && now.saturating_duration_since(stats.first_seen) >= window,
                                stats.height,
                                stats.view,
                            )
                        })
                })
                .filter(|(due, _, _)| *due)
                .map(|(_, height, view)| (height, view));
            if let Some((height, view)) = view_change_due {
                if self.should_defer_missing_block_view_change(&block_hash, height, view) {
                    debug!(
                        height,
                        view,
                        dwell_ms,
                        attempts,
                        "missing block dwell exceeded view-change window; deferring view change while RBC availability is unresolved"
                    );
                } else {
                    if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
                        stats.view_change_triggered_view = Some(view);
                    }
                    warn!(
                        height,
                        view,
                        dwell_ms,
                        since_last_ms,
                        attempts,
                        "missing block dwell exceeded view-change window; forcing view change"
                    );
                    self.trigger_view_change_with_cause(
                        height,
                        view,
                        ViewChangeCause::MissingPayload,
                    );
                    progress = true;
                }
            }
        }

        self.update_missing_block_gauges();
        progress
    }

    pub(super) fn should_defer_missing_block_view_change(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        let now = Instant::now();
        let (base_window, dwell) = self
            .pending
            .missing_block_requests
            .get(block_hash)
            .filter(|stats| stats.height == height && stats.view == view)
            .map_or((self.recovery_deferred_qc_ttl(), Duration::ZERO), |stats| {
                let base = stats
                    .view_change_window
                    .unwrap_or(self.recovery_deferred_qc_ttl());
                (base, now.saturating_duration_since(stats.first_seen))
            });
        let within_backlog_extension =
            |actor: &Self| dwell < actor.backlog_extended_view_change_timeout(base_window, true);
        if self.queue_drop_backpressure_active(now, self.payload_rebroadcast_cooldown()) {
            return within_backlog_extension(self);
        }
        if self.queue_block_backpressure_active(now, self.payload_rebroadcast_cooldown()) {
            return within_backlog_extension(self);
        }
        let queue_depths = super::status::worker_queue_depth_snapshot();
        if queue_depths.block_payload_rx > 0 || queue_depths.rbc_chunk_rx > 0 {
            return within_backlog_extension(self);
        }
        if self.has_unresolved_rbc_backlog() {
            return within_backlog_extension(self);
        }
        if !self.runtime_da_enabled() {
            return false;
        }
        let lower = height.saturating_sub(1);
        let upper = height.saturating_add(1);
        let pending_block_near_height =
            self.pending.pending_blocks.values().any(|pending| {
                !pending.aborted && pending.height >= lower && pending.height <= upper
            });
        if pending_block_near_height {
            return within_backlog_extension(self);
        }
        let inflight_pending_near_height =
            self.subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| {
                    !inflight.pending.aborted
                        && inflight.pending.height >= lower
                        && inflight.pending.height <= upper
                });
        if inflight_pending_near_height {
            return within_backlog_extension(self);
        }
        let ready_deferral_near_height = self
            .subsystems
            .da_rbc
            .rbc
            .ready_deferral
            .keys()
            .any(|key| key.1 >= lower && key.1 <= upper);
        if ready_deferral_near_height {
            return within_backlog_extension(self);
        }
        let deliver_deferral_near_height = self
            .subsystems
            .da_rbc
            .rbc
            .deliver_deferral
            .keys()
            .any(|key| key.1 >= lower && key.1 <= upper);
        if deliver_deferral_near_height {
            return within_backlog_extension(self);
        }
        let outbound_backlog_near_height =
            self.subsystems
                .da_rbc
                .rbc
                .outbound_chunks
                .iter()
                .any(|(key, outbound)| {
                    key.1 >= lower && key.1 <= upper && outbound.cursor < outbound.chunks.len()
                });
        if outbound_backlog_near_height {
            return within_backlog_extension(self);
        }
        let rbc_backlog_near_height =
            self.subsystems
                .da_rbc
                .rbc
                .sessions
                .iter()
                .any(|(key, session)| {
                    key.1 >= lower && key.1 <= upper && !session.is_invalid() && !session.delivered
                });
        if rbc_backlog_near_height {
            return within_backlog_extension(self);
        }
        let pending_backlog_near_height = self
            .subsystems
            .da_rbc
            .rbc
            .pending
            .keys()
            .any(|key| key.1 >= lower && key.1 <= upper);
        if pending_backlog_near_height {
            return within_backlog_extension(self);
        }
        let key = (*block_hash, height, view);
        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) {
            if !session.is_invalid() && !session.delivered {
                return within_backlog_extension(self);
            }
        }
        self.subsystems.da_rbc.rbc.pending.contains_key(&key) && within_backlog_extension(self)
    }

    pub(super) fn clear_missing_block_view_change(&mut self, block_hash: &HashOf<BlockHeader>) {
        if let Some(stats) = self.pending.missing_block_requests.get_mut(block_hash) {
            if stats.view_change_window.is_some() || stats.view_change_triggered_view.is_some() {
                stats.view_change_window = None;
                stats.view_change_triggered_view = None;
            }
        }
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
        if allowed {
            self.pending.pending_fetch_requests.remove(block_hash);
        }
        if !allowed {
            debug!(
                ?block_hash,
                reason = reason.as_str(),
                "skipping missing-block request clear; block payload not known locally"
            );
        }
        let now = Instant::now();
        if let Some(stats) = stats {
            let dwell = stats.first_seen.elapsed();
            debug!(
                ?block_hash,
                reason = reason.as_str(),
                dwell_ms = dwell.as_millis(),
                "cleared missing-block request"
            );
            self.note_missing_block_height_recovery_success(*block_hash, stats.height, now);
            self.clear_sidecar_mismatch_for_height(stats.height);
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.observe_missing_block_dwell(dwell);
            }
        }
        if allowed {
            self.note_missing_block_dependency_event(now);
            self.prune_missing_block_recovery_state(now);
        }
        self.update_missing_block_gauges();
    }

    #[allow(clippy::unused_self)]
    pub(super) fn build_qc_from_signers(
        &self,
        ctx: QcBuildContext,
        signers: &BTreeSet<ValidatorIndex>,
        canonical_topology: &super::network_topology::Topology,
        aggregate_signature: Vec<u8>,
        roots: Option<(Hash, Hash)>,
    ) -> crate::sumeragi::consensus::Qc {
        let signers_bitmap = build_signers_bitmap(signers, canonical_topology.as_ref().len());
        debug_assert!(
            !aggregate_signature.is_empty(),
            "QC aggregate signature must be non-empty"
        );
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let (parent_state_root, post_state_root) =
            if ctx.phase == crate::sumeragi::consensus::Phase::Commit {
                roots.unwrap_or_else(|| {
                    warn!(
                        height = ctx.height,
                        view = ctx.view,
                        block = %ctx.block_hash,
                        "missing execution roots while assembling commit QC"
                    );
                    (zero_root, zero_root)
                })
            } else {
                (zero_root, zero_root)
            };

        let validator_set = canonical_topology.as_ref().to_vec();
        crate::sumeragi::consensus::Qc {
            phase: ctx.phase,
            subject_block_hash: ctx.block_hash,
            parent_state_root,
            post_state_root,
            height: ctx.height,
            view: ctx.view,
            epoch: ctx.epoch,
            mode_tag: ctx.mode_tag,
            highest_qc: ctx.highest_qc,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: crate::sumeragi::consensus::QcAggregate {
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
        topology: &super::network_topology::Topology,
    ) {
        // Observers still need locally aggregated QCs for block sync, but must not broadcast them.
        let allow_broadcast = !self.is_observer();
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let canonical_roster = if matches!(phase, crate::sumeragi::consensus::Phase::NewView) {
            self.roster_for_new_view_with_mode(block_hash, height, view, consensus_mode)
        } else {
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode)
        };
        let canonical_topology = if matches!(consensus_mode, ConsensusMode::Permissioned)
            && !topology.as_ref().is_empty()
        {
            let provided = super::roster::canonicalize_roster_for_mode(
                topology.as_ref().to_vec(),
                consensus_mode,
            );
            super::network_topology::Topology::new(provided)
        } else if canonical_roster.is_empty() {
            topology.clone()
        } else {
            super::network_topology::Topology::new(canonical_roster)
        };
        let signature_topology_source =
            if matches!(consensus_mode, ConsensusMode::Npos) && !topology.as_ref().is_empty() {
                topology
            } else {
                &canonical_topology
            };
        let signature_topology =
            topology_for_view(signature_topology_source, height, view, mode_tag, prf_seed);
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
            if existing.phase == phase
                && (matches!(consensus_mode, ConsensusMode::Npos) || existing_signers >= required)
            {
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
        if phase == crate::sumeragi::consensus::Phase::Commit {
            // Avoid aggregating a lower-view commit QC once a higher NEW_VIEW quorum exists,
            // to prevent divergent commits during view changes.
            if let Some(higher_view) = self
                .subsystems
                .propose
                .new_view_tracker
                .highest_quorum_view_for_height(height, required, topology.as_ref())
            {
                if higher_view > view {
                    iroha_logger::info!(
                        height,
                        view,
                        higher_view,
                        block = ?block_hash,
                        "skipping commit QC aggregation: higher NEW_VIEW quorum observed"
                    );
                    return;
                }
            }
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
        let quorum_met = match consensus_mode {
            ConsensusMode::Permissioned => snapshot.voting_signers >= required,
            ConsensusMode::Npos => {
                let signer_peers =
                    match signer_peers_for_topology(&snapshot.signers, &signature_topology) {
                        Ok(peers) => peers,
                        Err(err) => {
                            warn!(
                                ?err,
                                height,
                                view,
                                phase = ?phase,
                                block = ?block_hash,
                                "skipping QC aggregation: failed to map signers to peers"
                            );
                            return;
                        }
                    };
                let world = self.state.world_view();
                let stake_roster = if !canonical_topology.as_ref().is_empty() {
                    canonical_topology.as_ref()
                } else if !topology.as_ref().is_empty() {
                    topology.as_ref()
                } else {
                    signature_topology.as_ref()
                };
                if stake_roster.is_empty() {
                    warn!(
                        height,
                        view,
                        phase = ?phase,
                        block = ?block_hash,
                        "skipping QC aggregation: stake roster unavailable"
                    );
                    return;
                }
                match super::stake_snapshot::stake_quorum_reached_for_world(
                    &world,
                    stake_roster,
                    &signer_peers,
                ) {
                    Ok(result) => result,
                    Err(err) => {
                        warn!(
                            ?err,
                            height,
                            view,
                            phase = ?phase,
                            block = ?block_hash,
                            "skipping QC aggregation: stake quorum check failed"
                        );
                        return;
                    }
                }
            }
        };
        let block_known = self.block_known_locally(block_hash);
        let deferred = self.qc_missing_block_defer(
            phase,
            block_hash,
            height,
            view,
            &snapshot.signers,
            &signature_topology,
            consensus_mode,
            quorum_met,
            required,
            block_known,
            snapshot.voting_signers,
            snapshot.total_signers,
        );
        if deferred {
            return;
        }

        if !quorum_met {
            let now = Instant::now();
            let insufficient_kind = match consensus_mode {
                ConsensusMode::Permissioned => QcInsufficientKind::PermissionedVotes,
                ConsensusMode::Npos => QcInsufficientKind::NposStake,
            };
            let suppressed_since_last = self.qc_insufficient_warning_log.allow(
                insufficient_kind,
                phase,
                block_hash,
                height,
                view,
                now,
                QC_INSUFFICIENT_WARN_COOLDOWN,
            );
            if let Some(suppressed_since_last) = suppressed_since_last {
                self.hotspot_log_summary.record_qc_insufficient_warn();
                if suppressed_since_last > 0 {
                    self.hotspot_log_summary
                        .record_qc_insufficient_suppressed(suppressed_since_last);
                }
                match consensus_mode {
                    ConsensusMode::Permissioned => {
                        iroha_logger::info!(
                            height,
                            view,
                            phase = ?phase,
                            block = ?block_hash,
                            voting_signers = snapshot.voting_signers,
                            total_signers = snapshot.total_signers,
                            required,
                            suppressed_since_last,
                            "not enough votes collected for QC"
                        );
                    }
                    ConsensusMode::Npos => {
                        iroha_logger::info!(
                            height,
                            view,
                            phase = ?phase,
                            block = ?block_hash,
                            voting_signers = snapshot.voting_signers,
                            total_signers = snapshot.total_signers,
                            suppressed_since_last,
                            "not enough stake collected for QC"
                        );
                    }
                }
            } else {
                self.hotspot_log_summary
                    .record_qc_insufficient_suppressed(1);
            }
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
        let highest_qc = if phase == crate::sumeragi::consensus::Phase::NewView {
            super::select_new_view_highest_qc_from_votes(
                &self.vote_log,
                &snapshot.signers,
                height,
                view,
                epoch,
            )
        } else {
            None
        };
        if phase == crate::sumeragi::consensus::Phase::NewView && highest_qc.is_none() {
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
            &canonical_topology,
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

        let roots = if phase == crate::sumeragi::consensus::Phase::Commit {
            snapshot.signers.iter().find_map(|signer| {
                let key = (phase, height, view, epoch, *signer);
                self.vote_log.get(&key).and_then(|vote| {
                    if vote.block_hash == block_hash {
                        Some((vote.parent_state_root, vote.post_state_root))
                    } else {
                        None
                    }
                })
            })
        } else {
            None
        };

        let qc = self.build_qc_from_signers(
            QcBuildContext {
                phase,
                block_hash,
                height,
                view,
                epoch,
                mode_tag: mode_tag.to_string(),
                highest_qc,
            },
            &canonical_signers,
            &canonical_topology,
            aggregate_signature,
            roots,
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
        if let Err(err) = self.handle_qc_with_aggregate(qc.clone(), Some(true)) {
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
        if allow_broadcast {
            let msg = Arc::new(BlockMessage::Qc(qc));
            let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
            let topology_peers = topology.as_ref();
            let local_peer_id = self.common_config.peer.id().clone();
            let topology_set: BTreeSet<_> = topology_peers.iter().cloned().collect();
            for peer in topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                });
            }
            let trusted = self.common_config.trusted_peers.value();
            let mut extra_targets: Vec<PeerId> = trusted
                .others
                .iter()
                .map(|peer| peer.id().clone())
                .filter(|peer| !topology_set.contains(peer))
                .collect();
            if !extra_targets.is_empty() {
                let online: BTreeSet<_> = self
                    .network
                    .online_peers(|set| set.iter().map(|peer| peer.id().clone()).collect());
                extra_targets.retain(|peer| online.contains(peer));
                for peer in extra_targets {
                    if peer == local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer,
                        msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
                    });
                }
            }
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
        let (valid_signers, stats) = self.qc_signers_for_votes_with_stats(
            phase,
            block_hash,
            height,
            view,
            epoch,
            signature_topology,
        );
        let raw_votes = stats.raw_votes;
        let mut signers = valid_signers.clone();
        let mut root_groups = 0;
        if phase == crate::sumeragi::consensus::Phase::Commit && !signers.is_empty() {
            let (filtered, groups) = select_commit_root_signers(
                &self.vote_log,
                block_hash,
                height,
                view,
                epoch,
                &signers,
            );
            root_groups = groups;
            signers = filtered;
        }
        if valid_signers.is_empty() && raw_votes > 0 {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                raw_votes,
                filtered_invalid_signature = stats.invalid_signature,
                filtered_roster_mismatch = stats.roster_mismatch,
                filtered_higher_view = stats.higher_view_filtered,
                filtered_total = stats.filtered_total(),
                required,
                topology_len = signature_topology.as_ref().len(),
                "votes observed but no eligible signers collected for QC"
            );
        } else if raw_votes > 0 && valid_signers.len() != raw_votes {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                raw_votes,
                valid_signers = valid_signers.len(),
                filtered_invalid_signature = stats.invalid_signature,
                filtered_roster_mismatch = stats.roster_mismatch,
                filtered_higher_view = stats.higher_view_filtered,
                filtered_total = stats.filtered_total(),
                required,
                topology_len = signature_topology.as_ref().len(),
                "votes filtered during QC tally; QC may stall"
            );
        }
        if phase == crate::sumeragi::consensus::Phase::Commit
            && root_groups > 1
            && signers.len() < valid_signers.len()
        {
            warn!(
                height,
                view,
                block = ?block_hash,
                root_groups,
                selected_signers = signers.len(),
                valid_signers = valid_signers.len(),
                required,
                "commit votes split across execution roots; QC tally may stall"
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
        consensus_mode: ConsensusMode,
        quorum_met: bool,
        required: usize,
        block_known: bool,
        voting_signers: usize,
        total_signers: usize,
    ) -> bool {
        if !block_known && quorum_met {
            crate::sumeragi::status::inc_qc_quorum_without_qc();
            match consensus_mode {
                ConsensusMode::Permissioned => {
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
                ConsensusMode::Npos => {
                    warn!(
                        height,
                        view,
                        phase = ?phase,
                        block = ?block_hash,
                        voting_signers,
                        total_signers,
                        "stake quorum observed but block payload missing; deferring QC aggregation"
                    );
                }
            }
        }

        if block_known {
            false
        } else {
            if quorum_met {
                self.defer_qc_if_block_missing_with_quorum_hint(
                    phase,
                    block_hash,
                    height,
                    view,
                    signers,
                    signature_topology,
                    /*commit_quorum_met*/ true,
                )
            } else {
                self.defer_qc_if_block_missing(
                    phase,
                    block_hash,
                    height,
                    view,
                    signers,
                    signature_topology,
                )
            }
        }
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
        if !self.block_known_for_lock(lock.subject_block_hash) {
            return true;
        }
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

    pub(super) fn rebuild_qcs_from_cached_votes(&mut self, commit_topology: &[PeerId]) {
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
        if vote_log.is_empty() {
            return;
        }
        rebuild_qc_candidates_with(
            vote_log.iter(),
            1,
            block_known,
            qc_present,
            |key, signer_count| {
                let (phase, block_hash, height, view, epoch) = key;
                let (consensus_mode, _, _) = self.consensus_context_for_height(height);
                let mut commit_roster =
                    self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
                if commit_roster.is_empty() && !commit_topology.is_empty() {
                    commit_roster = commit_topology.to_vec();
                }
                if commit_roster.is_empty() {
                    return;
                }
                let topology = super::network_topology::Topology::new(commit_roster);
                let required = match consensus_mode {
                    ConsensusMode::Permissioned => topology.min_votes_for_commit(),
                    ConsensusMode::Npos => 1,
                };
                if matches!(consensus_mode, ConsensusMode::Permissioned) && signer_count < required
                {
                    return;
                }
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
                self.try_form_qc_from_votes(phase, block_hash, height, view, epoch, &topology);
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

    pub(super) fn block_is_empty(&self, hash: HashOf<BlockHeader>) -> Option<bool> {
        if let Some(height) = self.kura.get_block_height_by_hash(hash) {
            if let Some(block) = self.kura.get_block(height) {
                if !block.is_empty() {
                    return Some(false);
                }
                if self.state.time_triggers_due_for_block_fast(&block.header()) {
                    return Some(false);
                }
                return Some(true);
            }
        }
        self.pending.pending_blocks.get(&hash).map(|pending| {
            if !pending.block.is_empty() {
                return false;
            }
            if self
                .state
                .time_triggers_due_for_block_fast(&pending.block.header())
            {
                return false;
            }
            true
        })
    }

    fn drop_empty_block_state(&mut self, qc: &crate::sumeragi::consensus::Qc) {
        let block_hash = qc.subject_block_hash;
        let block_height = qc.height;
        let view = qc.view;

        let _ = super::drop_pending_block_and_requeue(
            &mut self.pending.pending_blocks,
            block_hash,
            self.queue.as_ref(),
            self.state.as_ref(),
        );
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::Obsolete);
        self.clean_rbc_sessions_for_block(block_hash, block_height);
        self.qc_cache
            .retain(|(_, hash, _, _, _), _| *hash != block_hash);
        self.qc_signer_tally
            .retain(|(_, hash, _, _, _), _| *hash != block_hash);
        self.subsystems
            .propose
            .proposal_cache
            .pop_proposal(block_height, view);
        self.subsystems
            .propose
            .proposal_cache
            .pop_hint(block_height, view);

        let mut drop_keys = Vec::new();
        for (key, vote) in &self.vote_log {
            if vote.block_hash == block_hash && vote.height == block_height {
                drop_keys.push(*key);
            }
        }
        if !drop_keys.is_empty() {
            for key in drop_keys {
                self.vote_log.remove(&key);
                self.vote_validation_cache.remove(&key);
            }
            self.qc_signer_tally
                .retain(|(phase, hash, height, _, _), _| {
                    *phase != crate::sumeragi::consensus::Phase::Commit
                        || *hash != block_hash
                        || *height != block_height
                });
            self.qc_cache.retain(|(phase, hash, height, _, _), _| {
                *phase != crate::sumeragi::consensus::Phase::Commit
                    || *hash != block_hash
                    || *height != block_height
            });
        }
        self.vote_roster_cache.remove(&block_hash);
        self.block_signer_cache.remove_block(&block_hash);
    }

    pub(super) fn has_nonempty_pending_at_height(&self, height: u64) -> bool {
        self.pending.pending_blocks.values().any(|pending| {
            pending.height == height && !pending.aborted && !pending.block.is_empty()
        })
    }

    pub(super) fn drop_missing_lock_if_unknown(&mut self, qc: &crate::sumeragi::consensus::Qc) {
        if let Some(lock) = self.locked_qc {
            if !self.block_known_locally(lock.subject_block_hash) {
                let locked_hash = lock.subject_block_hash;
                info!(
                    locked_height = lock.height,
                    locked_view = lock.view,
                    locked_hash = %locked_hash,
                    incoming_height = qc.height,
                    incoming_view = qc.view,
                    "locked QC payload missing locally; requesting payload before processing incoming QC"
                );
                let now = Instant::now();
                let retry_window = self.rebroadcast_cooldown();
                let view_change_window = Some(self.recovery_deferred_qc_ttl());
                let (consensus_mode, _mode_tag, _prf_seed) =
                    self.consensus_context_for_height(lock.height);
                let mut roster = self.roster_for_vote_with_mode(
                    locked_hash,
                    lock.height,
                    lock.view,
                    consensus_mode,
                );
                if roster.is_empty() {
                    roster = self.effective_commit_topology();
                }
                if !roster.is_empty() {
                    let topology = super::network_topology::Topology::new(roster);
                    let signers = BTreeSet::new();
                    let peer_id = self.common_config.peer.id.clone();
                    let network = self.network.clone();
                    let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
                    let _ = super::defer_qc_for_missing_block(
                        false,
                        retry_window,
                        view_change_window,
                        now,
                        locked_hash,
                        lock.height,
                        lock.view,
                        lock.phase,
                        &signers,
                        &topology,
                        self.recovery_signer_fallback_attempts(),
                        &mut requests,
                        self.telemetry_handle(),
                        move |targets| {
                            send_missing_block_request(
                                &network,
                                &peer_id,
                                locked_hash,
                                lock.height,
                                lock.view,
                                super::MissingBlockPriority::Consensus,
                                targets,
                            )
                        },
                    );
                    self.pending.missing_block_requests = requests;
                }
                return;
            }
        }
    }

    fn qc_for_committed_height(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        committed_height: u64,
        consensus_mode: ConsensusMode,
        mode_tag: &'static str,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> CommittedQcDecision {
        if qc.height > committed_height {
            return CommittedQcDecision::Continue;
        }
        if let Ok(height_usize) = usize::try_from(qc.height)
            && let Some(nz_height) = NonZeroUsize::new(height_usize)
            && let Some(committed) = self.kura.get_block(nz_height)
        {
            let committed_hash = committed.hash();
            if committed_hash != qc.subject_block_hash {
                if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                    info!(
                        height = qc.height,
                        view = qc.view,
                        committed_height,
                        committed_hash = %committed_hash,
                        incoming_hash = %qc.subject_block_hash,
                        phase = ?qc.phase,
                        "dropping QC for committed height with divergent hash (non-commit)"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::CommitConflict,
                    );
                    return CommittedQcDecision::Drop;
                }
                let expected_epoch = self.epoch_for_height(qc.height);
                let inputs = self.roster_validation_cache.inputs_for_roster(
                    &qc.validator_set,
                    consensus_mode,
                    stake_snapshot,
                );
                let allow_genesis_stub = qc.height == 1 && qc.view == 0;
                if let Err(err) = super::validate_commit_qc_roster_cached(
                    &self.roster_validation_cache,
                    qc,
                    qc.subject_block_hash,
                    qc.height,
                    Some(qc.view),
                    consensus_mode,
                    expected_epoch,
                    &self.common_config.chain,
                    mode_tag,
                    allow_genesis_stub,
                    &inputs,
                ) {
                    info!(
                        height = qc.height,
                        view = qc.view,
                        committed_height,
                        committed_hash = %committed_hash,
                        incoming_hash = %qc.subject_block_hash,
                        ?err,
                        "dropping QC for committed height with divergent hash (invalid commit QC)"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::CommitConflict,
                    );
                    return CommittedQcDecision::Drop;
                }
                info!(
                    height = qc.height,
                    view = qc.view,
                    committed_height,
                    committed_hash = %committed_hash,
                    incoming_hash = %qc.subject_block_hash,
                    "rejecting conflicting commit QC at committed height; enforcing finality"
                );
                #[cfg(feature = "telemetry")]
                {
                    self.telemetry.inc_commit_conflict_detected();
                }
                let evidence = crate::sumeragi::consensus::Evidence {
                    kind: crate::sumeragi::consensus::EvidenceKind::InvalidQc,
                    payload: crate::sumeragi::consensus::EvidencePayload::InvalidQc {
                        certificate: qc.clone(),
                        reason: "commit_conflict_finality".to_owned(),
                    },
                };
                if let Err(err) = self.record_and_broadcast_evidence(evidence) {
                    warn!(
                        ?err,
                        height = qc.height,
                        view = qc.view,
                        "failed to record commit-conflict evidence"
                    );
                }
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::CommitConflict,
                );
                return CommittedQcDecision::Drop;
            }
            return CommittedQcDecision::RecordOnly;
        }
        info!(
            height = qc.height,
            view = qc.view,
            committed_height,
            hash = %qc.subject_block_hash,
            "dropping QC for already committed height with unknown block"
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::Committed,
        );
        CommittedQcDecision::Drop
    }

    pub(super) fn should_drop_qc_on_empty_block(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
    ) -> bool {
        if !block_known {
            return false;
        }
        if let Some(true) = self.block_is_empty(qc.subject_block_hash) {
            let queue_len = self.queue.queued_len();
            let pending_nonempty = self.has_nonempty_pending_at_height(qc.height);
            info!(
                phase = ?qc.phase,
                height = qc.height,
                view = qc.view,
                queue_len,
                pending_nonempty,
                hash = %qc.subject_block_hash,
                "dropping QC for empty block; empty blocks are disallowed"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            self.drop_empty_block_state(qc);
            return true;
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

    #[allow(clippy::too_many_lines)]
    pub(super) fn process_precommit_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
        allow_nonextending: bool,
    ) -> bool {
        self.record_phase_sample(PipelinePhase::CollectCommit, qc.height, qc.view);
        let qc_ref = Self::qc_to_header_ref(qc);
        if let Some(lock) = self.locked_qc
            && !self.block_known_for_lock(lock.subject_block_hash)
            && (qc.height != lock.height || qc.subject_block_hash != lock.subject_block_hash)
        {
            // Keep lock/highest stable while local lock payload is missing; apply once recovered.
            self.drop_missing_lock_if_unknown(qc);
            info!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                locked_height = lock.height,
                locked_hash = %lock.subject_block_hash,
                "deferring precommit QC lock/highest update until locked payload is available"
            );
            return true;
        }
        if let Some(lock) = self.locked_qc {
            if self.block_known_for_lock(lock.subject_block_hash) {
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
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
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
                |hash| self.block_known_for_lock(hash),
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
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
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
                "precommit QC arrived before block validation; cached without updating locks/highest"
            );
        }
        true
    }

    pub(super) fn process_new_view_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        signers: &[ValidatorIndex],
        topology: &super::network_topology::Topology,
    ) {
        let Some(highest) = qc.highest_qc else {
            warn!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                "ignoring NEW_VIEW certificate missing highest certificate reference"
            );
            return;
        };
        let valid_phase = matches!(highest.phase, crate::sumeragi::consensus::Phase::Commit)
            || matches!(highest.phase, crate::sumeragi::consensus::Phase::Prepare);
        if !valid_phase {
            warn!(
                height = qc.height,
                view = qc.view,
                highest_height = highest.height,
                highest_view = highest.view,
                phase = ?highest.phase,
                "ignoring NEW_VIEW certificate with invalid highest certificate phase"
            );
            return;
        }
        let expected_height = highest.height.saturating_add(1);
        if qc.height != expected_height {
            warn!(
                height = qc.height,
                view = qc.view,
                highest_height = highest.height,
                expected_height,
                "ignoring NEW_VIEW certificate with mismatched height"
            );
            return;
        }
        let roster = topology.as_ref();
        for signer in signers {
            let Some(peer) = usize::try_from(*signer)
                .ok()
                .and_then(|idx| roster.get(idx))
            else {
                warn!(
                    signer = ?signer,
                    roster_len = roster.len(),
                    "skipping NEW_VIEW signer outside topology"
                );
                continue;
            };
            self.subsystems.propose.new_view_tracker.record(
                qc.height,
                qc.view,
                peer.clone(),
                highest,
            );
        }
    }

    pub(super) fn rehydrate_pending_from_kura_for_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        if self
            .pending
            .pending_blocks
            .contains_key(&qc.subject_block_hash)
        {
            return true;
        }
        if self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .is_some_and(|inflight| inflight.block_hash == qc.subject_block_hash)
        {
            return true;
        }
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|hash| hash == qc.subject_block_hash)
        {
            return true;
        }
        let Some(height_nz) = self.kura.get_block_height_by_hash(qc.subject_block_hash) else {
            return false;
        };
        let Some(block) = self.kura.get_block(height_nz) else {
            return false;
        };
        let block_height = u64::try_from(height_nz.get()).unwrap_or(u64::MAX);
        if block_height != qc.height {
            warn!(
                qc_height = qc.height,
                block_height,
                block = %qc.subject_block_hash,
                "skipping pending rehydration: kura height mismatch"
            );
            return false;
        }
        let block_view = block.header().view_change_index();
        if block_view != qc.view {
            warn!(
                qc_height = qc.height,
                qc_view = qc.view,
                block_view,
                block = %qc.subject_block_hash,
                "skipping pending rehydration: kura view mismatch"
            );
            return false;
        }
        let payload_bytes = super::proposals::block_payload_bytes(&block);
        let payload_hash = iroha_crypto::Hash::new(&payload_bytes);
        let mut pending =
            PendingBlock::new(block.as_ref().clone(), payload_hash, qc.height, qc.view);
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            pending.commit_qc_seen = true;
            pending.commit_qc_epoch = Some(qc.epoch);
        }
        pending.mark_kura_persisted();
        self.pending
            .pending_blocks
            .insert(qc.subject_block_hash, pending);
        self.clear_missing_block_request(
            &qc.subject_block_hash,
            MissingBlockClearReason::PayloadAvailable,
        );
        true
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
            self.vote_validation_cache.remove(&key);
        }
        for (hash, _) in &drop_blocks {
            self.vote_roster_cache.remove(hash);
            self.block_signer_cache.remove_block(hash);
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

    pub(super) fn handle_qc(&mut self, qc: crate::sumeragi::consensus::Qc) -> Result<()> {
        self.handle_qc_with_aggregate(qc, None)
    }

    #[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
    pub(super) fn handle_qc_with_aggregate(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        aggregate_ok: Option<bool>,
    ) -> Result<()> {
        let mut aggregate_ok = aggregate_ok;
        // Prepare certificates are view-scoped; commit/new-view certificates can safely arrive
        // after a local view change and still unlock progress, so don't drop them as stale.
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Prepare)
            && self.drop_stale_view(qc.height, qc.view, "QC")
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return Ok(());
        }
        let expected_epoch = self.epoch_for_height(qc.height);
        if qc.epoch != expected_epoch {
            warn!(
                height = qc.height,
                view = qc.view,
                epoch = qc.epoch,
                expected_epoch,
                phase = ?qc.phase,
                block = %qc.subject_block_hash,
                "dropping QC with mismatched epoch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            return Ok(());
        }
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::NewView) {
            let Some(highest) = qc.highest_qc else {
                warn!(
                    height = qc.height,
                    view = qc.view,
                    block = %qc.subject_block_hash,
                    "dropping NEW_VIEW QC missing highest certificate reference"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::MissingHighestQc,
                );
                return Ok(());
            };
            let expected_highest_epoch = self.epoch_for_height(highest.height);
            if highest.epoch != expected_highest_epoch {
                warn!(
                    height = qc.height,
                    view = qc.view,
                    highest_height = highest.height,
                    highest_epoch = highest.epoch,
                    expected_highest_epoch,
                    "dropping NEW_VIEW QC with mismatched highest epoch"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::EpochMismatch,
                );
                return Ok(());
            }
            if let Some((local_height, local_view)) =
                self.local_block_height_view(highest.subject_block_hash)
            {
                if local_height != highest.height || local_view != highest.view {
                    warn!(
                        height = qc.height,
                        view = qc.view,
                        highest_height = highest.height,
                        highest_view = highest.view,
                        local_height,
                        local_view,
                        "dropping NEW_VIEW QC with highest certificate that mismatches local block metadata"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::HighestQcMismatch,
                    );
                    return Ok(());
                }
            }
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
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
            self.defer_qc_for_roster(qc, "commit topology missing");
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let topology_len = topology.as_ref().len();
        if aggregate_ok.is_none()
            && self
                .subsystems
                .qc_verify
                .verified_cache
                .contains(&super::QcVerifyCacheKey::from_qc(&qc))
        {
            aggregate_ok = Some(true);
        }
        if aggregate_ok.is_none()
            && topology_len > QC_VERIFY_INLINE_ROSTER_MAX
            && !self.subsystems.qc_verify.work_txs.is_empty()
        {
            if let Some(inputs) = super::qc_aggregate_inputs(
                &qc,
                &topology,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                mode_tag,
            ) {
                let key = super::QcVerifyKey::from_qc(&qc);
                if self.subsystems.qc_verify.inflight.contains_key(&key) {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        "dropping duplicate QC while aggregate verification is in flight"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::Duplicate,
                    );
                    return Ok(());
                }
                let id = self.subsystems.qc_verify.next_id();
                let mut work = super::qc_verify::QcVerifyWork {
                    id,
                    key: key.clone(),
                    inputs,
                };
                let mut dispatched = false;
                let mut disconnected = Vec::new();
                let total = self.subsystems.qc_verify.work_txs.len();
                for _ in 0..total {
                    let idx = self.subsystems.qc_verify.next_worker % total;
                    self.subsystems.qc_verify.next_worker =
                        self.subsystems.qc_verify.next_worker.saturating_add(1);
                    let work_tx = &self.subsystems.qc_verify.work_txs[idx];
                    match work_tx.try_send(work) {
                        Ok(()) => {
                            dispatched = true;
                            break;
                        }
                        Err(mpsc::TrySendError::Full(returned)) => {
                            work = returned;
                        }
                        Err(mpsc::TrySendError::Disconnected(returned)) => {
                            work = returned;
                            disconnected.push(idx);
                        }
                    }
                }
                if !disconnected.is_empty() {
                    disconnected.sort_unstable();
                    disconnected.dedup();
                    for idx in disconnected.into_iter().rev() {
                        if idx < self.subsystems.qc_verify.work_txs.len() {
                            self.subsystems.qc_verify.work_txs.swap_remove(idx);
                        }
                    }
                    if self.subsystems.qc_verify.next_worker
                        >= self.subsystems.qc_verify.work_txs.len()
                    {
                        self.subsystems.qc_verify.next_worker = 0;
                    }
                }
                if dispatched {
                    self.subsystems.qc_verify.inflight.insert(
                        key,
                        super::QcVerifyInFlight {
                            id,
                            target: super::QcVerifyTarget::Consensus(qc.clone()),
                        },
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::AggregateVerifyDeferred,
                    );
                    return Ok(());
                }
                if self.subsystems.qc_verify.work_txs.is_empty() {
                    warn!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        "QC verify workers unavailable; running aggregate verification inline"
                    );
                    self.subsystems.qc_verify.result_rx = None;
                    self.subsystems.qc_verify.inflight.clear();
                } else {
                    warn!(
                        height = qc.height,
                        view = qc.view,
                        phase = ?qc.phase,
                        block = %qc.subject_block_hash,
                        "QC verify worker queue full; running aggregate verification inline"
                    );
                }
            }
        } else if aggregate_ok.is_none() && topology_len <= QC_VERIFY_INLINE_ROSTER_MAX {
            debug!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                block = %qc.subject_block_hash,
                roster_len = topology_len,
                "verifying QC aggregate inline for small commit roster"
            );
        }
        let (stake_snapshot, validation, evidence) = {
            let world = self.state.world_view();
            let stake_snapshot = match consensus_mode {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => CommitStakeSnapshot::from_roster(&world, topology.as_ref()),
            };
            let (validation, evidence) = validate_qc_with_evidence(
                &self.vote_log,
                &qc,
                &topology,
                &world,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                aggregate_ok,
            );
            (stake_snapshot, validation, evidence)
        };
        let validation = match validation {
            Ok(outcome) => {
                if aggregate_ok != Some(false) {
                    self.subsystems
                        .qc_verify
                        .verified_cache
                        .insert(super::QcVerifyCacheKey::from_qc(&qc));
                }
                outcome
            }
            Err(err) => {
                if let Some(outcome) = self.recover_qc_from_aggregate(
                    &qc,
                    &topology,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    aggregate_ok,
                    &err,
                ) {
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
                    let handling_reason = match err {
                        QcValidationError::MissingVotes { .. }
                        | QcValidationError::InsufficientSigners { .. }
                        | QcValidationError::StakeSnapshotUnavailable
                        | QcValidationError::StakeQuorumMissing => {
                            super::status::ConsensusMessageReason::QuorumMissing
                        }
                        QcValidationError::InvalidSignature { .. } => {
                            super::status::ConsensusMessageReason::InvalidSignature
                        }
                        QcValidationError::HighestQcMismatch => {
                            super::status::ConsensusMessageReason::HighestQcMismatch
                        }
                        QcValidationError::ModeTagMismatch => {
                            super::status::ConsensusMessageReason::ModeMismatch
                        }
                        QcValidationError::ValidatorSetMismatch => {
                            super::status::ConsensusMessageReason::RosterHashMismatch
                        }
                        _ => super::status::ConsensusMessageReason::InvalidPayload,
                    };
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        handling_reason,
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
        debug_assert!(
            missing_votes == 0 || matches!(consensus_mode, ConsensusMode::Npos),
            "QC validation should fail when votes are missing in permissioned mode"
        );
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            let signer_set: BTreeSet<_> = signer_indices.iter().copied().collect();
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash: qc.subject_block_hash,
                    height: qc.height,
                    view: qc.view,
                    epoch: qc.epoch,
                    parent_state_root: qc.parent_state_root,
                    post_state_root: qc.post_state_root,
                    signers: signer_set,
                    bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    validator_set: topology.as_ref().to_vec(),
                    stake_snapshot: stake_snapshot.clone(),
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
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            if let Some(pending) = self.pending.pending_blocks.get_mut(&qc.subject_block_hash) {
                if pending.aborted
                    && !matches!(pending.validation_status, ValidationStatus::Invalid)
                {
                    let block = pending.block.clone();
                    let payload_hash = pending.payload_hash;
                    let height = pending.height;
                    let view = pending.view;
                    pending.revive_after_abort(block, payload_hash, height, view);
                    pending.commit_qc_seen = true;
                    pending.commit_qc_epoch = Some(qc.epoch);
                    info!(
                        height = qc.height,
                        view = qc.view,
                        block = %qc.subject_block_hash,
                        "revived aborted pending block after commit QC"
                    );
                }
            }
        }
        let committed_height = self.state.committed_height();
        let committed_height_u64 = u64::try_from(committed_height).unwrap_or(u64::MAX);
        self.drop_missing_lock_if_unknown(&qc);
        match self.qc_for_committed_height(
            &qc,
            committed_height_u64,
            consensus_mode,
            mode_tag,
            stake_snapshot.as_ref(),
        ) {
            CommittedQcDecision::Continue => {}
            CommittedQcDecision::RecordOnly => {
                if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
                    let stake_snapshot = self
                        .state
                        .commit_roster_snapshot_for_block(qc.height, qc.subject_block_hash)
                        .and_then(|snapshot| snapshot.stake_snapshot);
                    let checkpoint = ValidatorSetCheckpoint::new(
                        qc.height,
                        qc.view,
                        qc.subject_block_hash,
                        qc.parent_state_root,
                        qc.post_state_root,
                        qc.validator_set.clone(),
                        qc.aggregate.signers_bitmap.clone(),
                        qc.aggregate.bls_aggregate_signature.clone(),
                        qc.validator_set_hash_version,
                        None,
                    );
                    self.state
                        .record_commit_roster(&qc, &checkpoint, stake_snapshot);
                    super::status::record_commit_qc(qc.clone());
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
                return Ok(());
            }
            CommittedQcDecision::Drop => return Ok(()),
        }

        let block_known_locally = self.block_known_locally(qc.subject_block_hash);
        let qc_key = Self::qc_tally_key(&qc);
        if block_known_locally {
            self.deferred_missing_payload_qcs.remove(&qc_key);
        }
        let mut block_known_for_lock = self.block_known_for_lock(qc.subject_block_hash);
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            iroha_logger::debug!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                signers = signer_indices.len(),
                block_known = block_known_locally,
                block_validated = block_known_for_lock,
                pending = self.pending.pending_blocks.contains_key(&qc.subject_block_hash),
                "processing precommit QC"
            );
        }
        if self.should_drop_qc_on_empty_block(&qc, block_known_locally) {
            return Ok(());
        }

        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            // Persist roster artifacts on QC arrival so block sync can validate missing payloads.
            let checkpoint = ValidatorSetCheckpoint::new(
                qc.height,
                qc.view,
                qc.subject_block_hash,
                qc.parent_state_root,
                qc.post_state_root,
                qc.validator_set.clone(),
                qc.aggregate.signers_bitmap.clone(),
                qc.aggregate.bls_aggregate_signature.clone(),
                qc.validator_set_hash_version,
                None,
            );
            self.state
                .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone());
        }

        if !block_known_locally {
            self.defer_qc_for_missing_payload(&qc, "payload_missing");
            info!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                hash = %qc.subject_block_hash,
                "received QC for unknown block; caching without updating locks/highest"
            );
            let view_change_window = self.recovery_deferred_qc_ttl();
            let base_retry_window = self.rebroadcast_cooldown();
            let aggressive_qc_fetch = matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit);
            let existing_attempts = self
                .pending
                .missing_block_requests
                .get(&qc.subject_block_hash)
                .map_or(0, |stats| stats.attempts);
            let aggressive_retry_floor = aggressive_qc_fetch && existing_attempts == 0;
            let retry_window = if aggressive_retry_floor {
                crate::sumeragi::status::inc_qc_missing_payload_aggressive_fetch();
                super::REBROADCAST_COOLDOWN_FLOOR
            } else {
                self.missing_block_retry_window_with_rbc_progress(
                    qc.subject_block_hash,
                    qc.height,
                    qc.view,
                    base_retry_window,
                )
            };
            let now = Instant::now();
            if let Some(stats) = self
                .pending
                .missing_block_requests
                .get_mut(&qc.subject_block_hash)
            {
                if aggressive_retry_floor && stats.attempts == 0 {
                    if stats.retry_window == Duration::ZERO || retry_window < stats.retry_window {
                        stats.retry_window = retry_window;
                    }
                } else if retry_window > stats.retry_window {
                    stats.retry_window = retry_window;
                }
            }
            let signer_set: BTreeSet<_> = signer_indices.iter().copied().collect();
            let fetch_mode = if aggressive_qc_fetch {
                super::MissingBlockFetchMode::AggressiveTopology
            } else {
                super::MissingBlockFetchMode::Default
            };
            let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
            let decision = plan_missing_block_fetch_with_mode(
                &mut self.pending.missing_block_requests,
                qc.subject_block_hash,
                qc.height,
                qc.view,
                qc.phase,
                super::MissingBlockPriority::Consensus,
                &signer_set,
                &topology,
                now,
                retry_window,
                Some(view_change_window),
                signer_fallback_attempts,
                fetch_mode,
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
                    self.request_missing_block(
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                        super::MissingBlockPriority::Consensus,
                        &targets,
                    );
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
        } else if !block_known_for_lock {
            info!(
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                hash = %qc.subject_block_hash,
                "received QC for unvalidated block; caching without updating locks/highest"
            );
        }

        let accepted = match qc.phase {
            crate::sumeragi::consensus::Phase::Prepare => {
                self.process_prevote_qc(&qc, block_known_locally);
                true
            }
            crate::sumeragi::consensus::Phase::Commit => {
                self.process_precommit_qc(&qc, block_known_for_lock, false)
            }
            crate::sumeragi::consensus::Phase::NewView => {
                self.process_new_view_qc(&qc, &signer_indices, &topology);
                true
            }
        };
        if !accepted {
            return Ok(());
        }
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            super::status::record_commit_qc(qc.clone());
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
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
            && block_known_locally
            && !block_known_for_lock
        {
            if self.maybe_validate_pending_for_commit_qc(&qc, &commit_topology) {
                block_known_for_lock = self.block_known_for_lock(qc.subject_block_hash);
            }
        }
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) && block_known_locally {
            if block_known_for_lock {
                let commit_ready = self
                    .pending
                    .pending_blocks
                    .get(&qc.subject_block_hash)
                    .and_then(|pending| {
                        let state_height = self.state.committed_height();
                        let tip_hash = self.state.latest_block_hash_fast();
                        let parent = pending.block.header().prev_block_hash();
                        super::pending_extends_tip(pending.height, parent, state_height, tip_hash)
                            .then_some(())
                    })
                    .is_some();
                if commit_ready {
                    let _ = self.rehydrate_pending_from_kura_for_qc(&qc);
                    self.apply_commit_qc(
                        &qc,
                        &commit_topology,
                        qc.subject_block_hash,
                        qc.height,
                        qc.view,
                    );
                } else if let Some(pending) =
                    self.pending.pending_blocks.get_mut(&qc.subject_block_hash)
                {
                    pending.commit_qc_seen = true;
                    pending.commit_qc_epoch = Some(qc.epoch);
                    info!(
                        height = qc.height,
                        view = qc.view,
                        block = %qc.subject_block_hash,
                        "deferring commit QC application until block extends committed tip"
                    );
                }
            } else if let Some(pending) =
                self.pending.pending_blocks.get_mut(&qc.subject_block_hash)
            {
                pending.commit_qc_seen = true;
                pending.commit_qc_epoch = Some(qc.epoch);
                info!(
                    height = qc.height,
                    view = qc.view,
                    block = %qc.subject_block_hash,
                    "deferring commit QC application until block is validated"
                );
            }
        }
        if !block_known_for_lock {
            if let Some(lock) = self.locked_qc {
                // Keep status in sync if we cleared an unknown lock earlier.
                super::status::set_locked_qc(lock.height, lock.view, Some(lock.subject_block_hash));
            }
        }
        self.request_commit_pipeline();
        Ok(())
    }

    pub(super) fn recover_qc_from_aggregate(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        topology: &super::network_topology::Topology,
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
        aggregate_ok: Option<bool>,
        err: &QcValidationError,
    ) -> Option<QcValidationOutcome> {
        let QcValidationError::MissingVotes { .. } = err else {
            return None;
        };
        if qc.phase == crate::sumeragi::consensus::Phase::NewView {
            let highest = super::validate_new_view_qc_highest(qc).ok()?;
            let expected_highest_epoch = self.epoch_for_height(highest.height);
            if highest.epoch != expected_highest_epoch {
                return None;
            }
        }
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let _signature_topology =
            super::topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
        let aggregate_ok = match aggregate_ok {
            Some(value) => value,
            None => qc_aggregate_consistent(
                qc,
                topology,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                mode_tag,
            ),
        };
        if !aggregate_ok {
            return None;
        }
        let roster_len = topology.as_ref().len();
        let parsed_signers = qc_signer_indices(qc, roster_len, roster_len).ok()?;
        if matches!(consensus_mode, ConsensusMode::Npos) {
            let snapshot = stake_snapshot?;
            let signer_peers = signer_peers_for_topology(&parsed_signers.voting, topology).ok()?;
            match stake_quorum_reached_for_snapshot(snapshot, topology.as_ref(), &signer_peers) {
                Ok(true) => {}
                _ => return None,
            }
        }
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

    pub(super) fn handle_exec_witness(&self, witness: crate::sumeragi::consensus::ExecWitnessMsg) {
        if self.drop_stale_view(witness.height, witness.view, "ExecWitness") {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ExecWitness,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleView,
            );
            return;
        }
        if self
            .events_sender
            .send(EventBox::Pipeline(PipelineEventBox::Witness(witness)))
            .is_err()
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::ExecWitness,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EnqueueFailed,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::select_commit_root_signers;
    use crate::sumeragi::consensus::{Phase, ValidatorIndex, Vote};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
    use std::collections::{BTreeMap, BTreeSet};

    fn vote_with_roots(
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signer: ValidatorIndex,
        parent_state_root: Hash,
        post_state_root: Hash,
    ) -> Vote {
        Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            highest_qc: None,
            signer,
            bls_sig: vec![1],
        }
    }

    #[test]
    fn commit_root_signers_pick_majority_group() {
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
        let height = 8;
        let view = 3;
        let epoch = 0;
        let primary_parent_root = Hash::prehashed([0x11; Hash::LENGTH]);
        let primary_post_root = Hash::prehashed([0x12; Hash::LENGTH]);
        let secondary_parent_root = Hash::prehashed([0x21; Hash::LENGTH]);
        let secondary_post_root = Hash::prehashed([0x22; Hash::LENGTH]);

        let mut vote_log = BTreeMap::new();
        vote_log.insert(
            (Phase::Commit, height, view, epoch, 0),
            vote_with_roots(
                block_hash,
                height,
                view,
                epoch,
                0,
                primary_parent_root,
                primary_post_root,
            ),
        );
        vote_log.insert(
            (Phase::Commit, height, view, epoch, 1),
            vote_with_roots(
                block_hash,
                height,
                view,
                epoch,
                1,
                primary_parent_root,
                primary_post_root,
            ),
        );
        vote_log.insert(
            (Phase::Commit, height, view, epoch, 2),
            vote_with_roots(
                block_hash,
                height,
                view,
                epoch,
                2,
                secondary_parent_root,
                secondary_post_root,
            ),
        );

        let mut signers = BTreeSet::new();
        signers.insert(0);
        signers.insert(1);
        signers.insert(2);

        let (filtered, groups) =
            select_commit_root_signers(&vote_log, block_hash, height, view, epoch, &signers);
        assert_eq!(groups, 2);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&0));
        assert!(filtered.contains(&1));
    }

    #[test]
    fn commit_root_signers_tiebreaks_by_root() {
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xB1; Hash::LENGTH]));
        let height = 5;
        let view = 1;
        let epoch = 0;
        let primary_parent_root = Hash::prehashed([0x01; Hash::LENGTH]);
        let primary_post_root = Hash::prehashed([0x02; Hash::LENGTH]);
        let secondary_parent_root = Hash::prehashed([0x02; Hash::LENGTH]);
        let secondary_post_root = Hash::prehashed([0x03; Hash::LENGTH]);

        let mut vote_log = BTreeMap::new();
        vote_log.insert(
            (Phase::Commit, height, view, epoch, 1),
            vote_with_roots(
                block_hash,
                height,
                view,
                epoch,
                1,
                primary_parent_root,
                primary_post_root,
            ),
        );
        vote_log.insert(
            (Phase::Commit, height, view, epoch, 2),
            vote_with_roots(
                block_hash,
                height,
                view,
                epoch,
                2,
                secondary_parent_root,
                secondary_post_root,
            ),
        );

        let mut signers = BTreeSet::new();
        signers.insert(1);
        signers.insert(2);

        let (filtered, groups) =
            select_commit_root_signers(&vote_log, block_hash, height, view, epoch, &signers);
        assert_eq!(groups, 2);
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains(&1));
    }
}
