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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommittedQcDecision {
    Continue,
    RecordOnly,
    Drop,
}

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
    fn defer_qc_for_roster(&mut self, qc: crate::sumeragi::consensus::Qc, reason: &'static str) {
        let key = Self::qc_tally_key(&qc);
        if self.deferred_qcs.contains_key(&key) {
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

    pub(super) fn try_replay_deferred_qcs(&mut self) -> bool {
        if self.deferred_qcs.is_empty() {
            return false;
        }
        let mut to_replay = Vec::new();
        for (key, qc) in &self.deferred_qcs {
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
                to_replay.push((*key, qc.subject_block_hash, roster));
            }
        }
        if to_replay.is_empty() {
            return false;
        }
        for (key, block_hash, roster) in to_replay {
            self.cache_vote_roster(block_hash, key.2, key.3, roster);
            if let Some(qc) = self.deferred_qcs.remove(&key) {
                if let Err(err) = self.handle_qc(qc) {
                    warn!(?err, "failed to replay deferred QC");
                }
            }
        }
        true
    }

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
        let retry_window = self.rebroadcast_cooldown();
        // Retry missing payload fetches on the rebroadcast cadence while keeping
        // view-change gating tied to the quorum timeout to avoid premature churn under DA.
        let view_change_window = Some(self.quorum_timeout(da_enabled));
        let peer_id = self.common_config.peer.id.clone();
        let network = self.network.clone();
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let telemetry = self.telemetry_handle();
        let now = Instant::now();
        let deferred = defer_qc_for_missing_block(
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
            self.config.missing_block_signer_fallback_attempts,
            &mut requests,
            telemetry,
            move |targets| send_missing_block_request(&network, &peer_id, block_hash, targets),
        );
        self.pending.missing_block_requests = requests;
        if deferred {
            if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
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
    pub(super) fn retry_missing_block_requests(&mut self, now: Instant) -> bool {
        if self.pending.missing_block_requests.is_empty() {
            return false;
        }

        let mut progress = false;
        let pending_keys: Vec<_> = self
            .pending
            .missing_block_requests
            .keys()
            .copied()
            .collect();
        for block_hash in pending_keys {
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
                let (consensus_mode, _, _) =
                    self.consensus_context_for_height(stats_snapshot.height);
                let commit_topology = if matches!(
                    stats_snapshot.phase,
                    crate::sumeragi::consensus::Phase::NewView
                ) {
                    self.roster_for_new_view_with_mode(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        consensus_mode,
                    )
                } else {
                    self.roster_for_vote_with_mode(
                        block_hash,
                        stats_snapshot.height,
                        stats_snapshot.view,
                        consensus_mode,
                    )
                };
                if commit_topology.is_empty() {
                    let active = self.effective_commit_topology();
                    if !active.is_empty() {
                        roster = Some(active);
                    }
                } else {
                    roster = Some(commit_topology);
                }
            }
            let topology = roster
                .filter(|roster| !roster.is_empty())
                .map(super::network_topology::Topology::new);

            let retry_window = self
                .pending
                .missing_block_requests
                .get(&block_hash)
                .map_or(stats_snapshot.retry_window, |stats| stats.retry_window);
            let view_change_window = self
                .pending
                .missing_block_requests
                .get(&block_hash)
                .map_or(stats_snapshot.view_change_window, |stats| {
                    stats.view_change_window
                });
            let decision = if let Some(ref topology) = topology {
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
                    self.config.missing_block_signer_fallback_attempts,
                )
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
            };

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
            let targets_len = match &decision {
                MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
                _ => 0,
            };

            self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
            super::status::record_missing_block_fetch(targets_len, dwell_ms);

            match decision {
                MissingBlockFetchDecision::Requested {
                    targets,
                    target_kind,
                } => {
                    self.request_missing_block(block_hash, &targets);
                    iroha_logger::info!(
                        height = stats_snapshot.height,
                        view = stats_snapshot.view,
                        phase = ?stats_snapshot.phase,
                        block = ?block_hash,
                        targets = ?targets,
                        target_kind = target_kind.label(),
                        retry_window_ms,
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
                        retry_window_ms,
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
                        retry_window_ms,
                        dwell_ms,
                        since_last_ms,
                        attempts,
                        "missing-block retry still in backoff"
                    );
                }
            }

            let mut view_change = None;
            if let Some(stats) = self.pending.missing_block_requests.get_mut(&block_hash) {
                if stats.mark_view_change_if_due(now) {
                    view_change = Some((stats.height, stats.view, stats.attempts));
                }
            }
            if let Some((height, view, attempts)) = view_change {
                warn!(
                    height,
                    view,
                    dwell_ms,
                    since_last_ms,
                    attempts,
                    "missing block dwell exceeded view-change window; forcing view change"
                );
                self.trigger_view_change_with_cause(height, view, ViewChangeCause::MissingPayload);
                progress = true;
            }
        }

        self.update_missing_block_gauges();
        progress
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
        if self.is_observer() {
            return;
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology = topology_for_view(topology, height, view, mode_tag, prf_seed);
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
                let state_view = self.state.view();
                match super::stake_snapshot::stake_quorum_reached_for_peers(
                    &state_view,
                    topology.as_ref(),
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
                        "not enough stake collected for QC"
                    );
                }
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
            let mut selected: Option<crate::sumeragi::consensus::QcRef> = None;
            for signer in &snapshot.signers {
                let Some(vote) = self.vote_log.get(&(phase, height, view, epoch, *signer)) else {
                    continue;
                };
                let Some(candidate) = vote.highest_qc else {
                    continue;
                };
                if candidate.phase != crate::sumeragi::consensus::Phase::Commit {
                    continue;
                }
                selected = Some(selected.map_or(candidate, |current| {
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
                }));
            }
            selected
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
            topology,
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
            topology,
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
        let msg = BlockMessage::Qc(qc);
        let topology_peers = topology.as_ref();
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in topology_peers {
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
        let valid_signers =
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
                required,
                topology_len = signature_topology.as_ref().len(),
                "votes observed but no valid signers collected for QC"
            );
        } else if raw_votes > 0 && valid_signers.len() != raw_votes {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                raw_votes,
                valid_signers = valid_signers.len(),
                required,
                topology_len = signature_topology.as_ref().len(),
                "some votes failed signature/topology validation; QC tally may stall"
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
                return Some(block.is_empty());
            }
        }
        self.pending
            .pending_blocks
            .get(&hash)
            .map(|pending| pending.block.is_empty())
    }

    pub(super) fn has_nonempty_pending_at_height(&self, height: u64) -> bool {
        self.pending.pending_blocks.values().any(|pending| {
            pending.height == height && !pending.aborted && !pending.block.is_empty()
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

    fn qc_for_committed_height(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        committed_height: u64,
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
                info!(
                    height = qc.height,
                    view = qc.view,
                    committed_height,
                    committed_hash = %committed_hash,
                    incoming_hash = %qc.subject_block_hash,
                    "dropping QC for already committed height with divergent hash"
                );
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
        &self,
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

    pub(super) fn process_precommit_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_known: bool,
        allow_nonextending: bool,
    ) -> bool {
        self.record_phase_sample(PipelinePhase::CollectCommit, qc.height, qc.view);
        let qc_ref = Self::qc_to_header_ref(qc);
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
        let expected_height = if highest.phase == crate::sumeragi::consensus::Phase::Commit {
            highest.height.saturating_add(1)
        } else {
            highest.height
        };
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
        }
        for (hash, _) in &drop_blocks {
            self.vote_roster_cache.remove(hash);
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

    #[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
    pub(super) fn handle_qc(&mut self, qc: crate::sumeragi::consensus::Qc) -> Result<()> {
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
        let (stake_snapshot, validation, evidence) = {
            let state_view = self.state.view();
            let world = state_view.world();
            let stake_snapshot = match consensus_mode {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => CommitStakeSnapshot::from_roster(world, topology.as_ref()),
            };
            let (validation, evidence) = validate_qc_with_evidence(
                &self.vote_log,
                &qc,
                &topology,
                world,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
            );
            (stake_snapshot, validation, evidence)
        };
        let validation = match validation {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Some(outcome) = self.recover_qc_from_aggregate(
                    &qc,
                    &topology,
                    consensus_mode,
                    stake_snapshot.as_ref(),
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
        let committed_height = self.state.view().height();
        let committed_height_u64 = u64::try_from(committed_height).unwrap_or(u64::MAX);
        self.drop_missing_lock_if_unknown(&qc);
        match self.qc_for_committed_height(&qc, committed_height_u64) {
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
                }
                return Ok(());
            }
            CommittedQcDecision::Drop => return Ok(()),
        }

        let block_known_locally = self.block_known_locally(qc.subject_block_hash);
        let block_known_for_lock = self.block_known_for_lock(qc.subject_block_hash);
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

        if !block_known_locally {
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
                qc.view,
                qc.phase,
                super::MissingBlockPriority::Consensus,
                &signer_set,
                &topology,
                now,
                retry_window,
                Some(retry_window),
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
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) && block_known_locally {
            if block_known_for_lock {
                let commit_ready = self
                    .pending
                    .pending_blocks
                    .get(&qc.subject_block_hash)
                    .and_then(|pending| {
                        let (state_height, tip_hash) = {
                            let view = self.state.view();
                            (view.height(), view.latest_block_hash())
                        };
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
        self.process_commit_candidates();
        Ok(())
    }

    pub(super) fn recover_qc_from_aggregate(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
        topology: &super::network_topology::Topology,
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
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
        let aggregate_ok = qc_aggregate_consistent(
            qc,
            topology,
            &self.roster_validation_cache.pops,
            &self.common_config.chain,
            mode_tag,
        );
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
