//! Vote handling for consensus messages.

use std::{
    collections::btree_map::Entry,
    num::NonZeroUsize,
    sync::{Arc, mpsc},
    time::Instant,
};

use iroha_logger::prelude::*;

use crate::sumeragi::consensus::Phase;

use super::locked_qc::qc_extends_locked_with_lookup;
use super::*;

pub(super) type VoteLogKey = (
    crate::sumeragi::consensus::Phase,
    u64,
    u64,
    u64,
    crate::sumeragi::consensus::ValidatorIndex,
);

fn vote_key(vote: &crate::sumeragi::consensus::Vote) -> VoteLogKey {
    (vote.phase, vote.height, vote.view, vote.epoch, vote.signer)
}

fn vote_duplicate(
    vote_log: &BTreeMap<VoteLogKey, crate::sumeragi::consensus::Vote>,
    vote: &crate::sumeragi::consensus::Vote,
) -> bool {
    vote_log
        .get(&vote_key(vote))
        .is_some_and(|existing| existing.block_hash == vote.block_hash)
}

pub(super) fn record_vote_drop_without_roster(
    vote: &crate::sumeragi::consensus::Vote,
    reason: super::status::VoteValidationDropReason,
) {
    super::status::record_vote_validation_drop(super::status::VoteValidationDropRecord {
        reason,
        height: vote.height,
        view: vote.view,
        epoch: vote.epoch,
        signer_index: vote.signer,
        peer_id: None,
        roster_hash: None,
        roster_len: 0,
        block_hash: vote.block_hash,
    });
}

fn log_vote_validation_drop(
    vote: &crate::sumeragi::consensus::Vote,
    reason: super::status::VoteValidationDropReason,
    peer_id: Option<&PeerId>,
    roster_hash: &HashOf<Vec<PeerId>>,
    roster_len: u32,
    mode_tag: &str,
) {
    debug!(
        reason = reason.as_str(),
        phase = ?vote.phase,
        height = vote.height,
        view = vote.view,
        epoch = vote.epoch,
        signer = vote.signer,
        block_hash = %vote.block_hash,
        peer = ?peer_id,
        roster_len,
        roster_hash = %roster_hash,
        mode_tag,
        "dropping vote during validation"
    );
}

const VOTE_VERIFY_DEFERRED_DISPATCH_MAX: usize = 32;
const VOTE_VERIFY_POP_CACHE_MAX: usize = 64;
const VOTE_VERIFY_TOPOLOGY_CACHE_MAX: usize = 64;

impl Actor {
    pub(super) fn cached_vote_verify_pops(
        &mut self,
        roster: &Vec<PeerId>,
        roster_hash: &HashOf<Vec<PeerId>>,
    ) -> Arc<BTreeMap<PublicKey, Vec<u8>>> {
        if let Some(cached) = self.subsystems.vote_verify.pop_cache.get(roster_hash) {
            return Arc::clone(cached);
        }
        let mut pops = BTreeMap::new();
        let trusted_pops = &self.common_config.trusted_peers.value().pops;
        for peer in roster {
            let pk = peer.public_key();
            if let Some(pop) = self
                .roster_validation_cache
                .pops
                .get(pk)
                .or_else(|| trusted_pops.get(pk))
            {
                pops.insert(pk.clone(), pop.clone());
            }
        }
        let pops = Arc::new(pops);
        if self.subsystems.vote_verify.pop_cache.len() >= VOTE_VERIFY_POP_CACHE_MAX {
            self.subsystems.vote_verify.pop_cache.clear();
        }
        let roster_hash = roster_hash.clone();
        self.subsystems
            .vote_verify
            .pop_cache
            .insert(roster_hash, Arc::clone(&pops));
        pops
    }

    pub(super) fn cached_signature_topology(
        &mut self,
        height: u64,
        view: u64,
        roster_hash: &HashOf<Vec<PeerId>>,
        topology: &super::network_topology::Topology,
        mode_tag: &'static str,
        prf_seed: Option<[u8; 32]>,
    ) -> Arc<super::network_topology::Topology> {
        let key = super::SignatureTopologyCacheKey {
            height,
            view,
            roster_hash: roster_hash.clone(),
            mode_tag,
            prf_seed,
        };
        if let Some(cached) = self
            .subsystems
            .vote_verify
            .signature_topology_cache
            .get(&key)
        {
            return Arc::clone(cached);
        }
        let signature_topology = Arc::new(topology_for_view(
            topology, height, view, mode_tag, prf_seed,
        ));
        if self.subsystems.vote_verify.signature_topology_cache.len()
            >= VOTE_VERIFY_TOPOLOGY_CACHE_MAX
        {
            self.subsystems.vote_verify.signature_topology_cache.clear();
        }
        self.subsystems
            .vote_verify
            .signature_topology_cache
            .insert(key, Arc::clone(&signature_topology));
        signature_topology
    }

    fn build_vote_verify_work(
        &self,
        id: u64,
        key: VoteVerifyKey,
        vote: &crate::sumeragi::consensus::Vote,
        context: &VoteProcessingContext,
    ) -> super::vote_verify::VoteVerifyWork {
        super::vote_verify::VoteVerifyWork {
            id,
            key,
            vote: vote.clone(),
            signature_topology: context.signature_topology.clone(),
            pops: Arc::clone(&context.pops),
            chain_id: self.common_config.chain.clone(),
            mode_tag: context.mode_tag,
        }
    }

    fn try_send_vote_verify_work(
        &mut self,
        mut work: super::vote_verify::VoteVerifyWork,
    ) -> Result<(), super::vote_verify::VoteVerifyWork> {
        let mut disconnected = Vec::new();
        let total = self.subsystems.vote_verify.work_txs.len();
        for _ in 0..total {
            let idx = self.subsystems.vote_verify.next_worker % total;
            self.subsystems.vote_verify.next_worker =
                self.subsystems.vote_verify.next_worker.saturating_add(1);
            let work_tx = &self.subsystems.vote_verify.work_txs[idx];
            match work_tx.try_send(work) {
                Ok(()) => {
                    if !disconnected.is_empty() {
                        disconnected.sort_unstable();
                        disconnected.dedup();
                        for idx in disconnected.into_iter().rev() {
                            if idx < self.subsystems.vote_verify.work_txs.len() {
                                self.subsystems.vote_verify.work_txs.swap_remove(idx);
                            }
                        }
                        if self.subsystems.vote_verify.next_worker
                            >= self.subsystems.vote_verify.work_txs.len()
                        {
                            self.subsystems.vote_verify.next_worker = 0;
                        }
                    }
                    return Ok(());
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
                if idx < self.subsystems.vote_verify.work_txs.len() {
                    self.subsystems.vote_verify.work_txs.swap_remove(idx);
                }
            }
            if self.subsystems.vote_verify.next_worker >= self.subsystems.vote_verify.work_txs.len()
            {
                self.subsystems.vote_verify.next_worker = 0;
            }
        }
        Err(work)
    }

    pub(super) fn dispatch_pending_vote_verifications(&mut self) -> bool {
        if self.subsystems.vote_verify.pending.is_empty()
            || self.subsystems.vote_verify.work_txs.is_empty()
        {
            return false;
        }
        let keys: Vec<_> = self
            .subsystems
            .vote_verify
            .pending
            .keys()
            .cloned()
            .take(VOTE_VERIFY_DEFERRED_DISPATCH_MAX)
            .collect();
        if keys.is_empty() {
            return false;
        }
        let mut progress = false;
        for key in keys {
            let Some(pending) = self.subsystems.vote_verify.pending.remove(&key) else {
                continue;
            };
            if self.subsystems.vote_verify.inflight.contains_key(&key) {
                continue;
            }
            if self.subsystems.vote_verify.work_txs.is_empty() {
                self.subsystems.vote_verify.pending.insert(key, pending);
                break;
            }
            let work = self.build_vote_verify_work(
                pending.id,
                key.clone(),
                &pending.vote,
                &pending.context,
            );
            match self.try_send_vote_verify_work(work) {
                Ok(()) => {
                    self.subsystems.vote_verify.inflight.insert(
                        key,
                        super::VoteVerifyInFlight {
                            id: pending.id,
                            vote: pending.vote,
                            context: pending.context,
                        },
                    );
                    progress = true;
                }
                Err(_) => {
                    self.subsystems.vote_verify.pending.insert(key, pending);
                    break;
                }
            }
        }
        progress
    }

    pub(super) fn consensus_context_for_height(
        &self,
        height: u64,
    ) -> (ConsensusMode, &'static str, Option<[u8; 32]>) {
        let world = self.state.world.view();
        let consensus_mode = super::effective_consensus_mode_for_height_from_world(
            &world,
            height,
            self.config.consensus_mode,
        );
        let mode_tag = match consensus_mode {
            ConsensusMode::Permissioned => super::consensus::PERMISSIONED_TAG,
            ConsensusMode::Npos => super::consensus::NPOS_TAG,
        };
        let prf_seed = Some(super::prf_seed_for_height_from_world(
            &world,
            &self.chain_id,
            height,
        ));
        drop(world);
        (consensus_mode, mode_tag, prf_seed)
    }

    pub(super) fn handle_vote_inbound(&mut self, vote: crate::sumeragi::consensus::Vote) {
        if self.invalid_sig_penalty.is_suppressed(
            InvalidSigKind::Vote,
            vote.signer.into(),
            Instant::now(),
        ) {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                kind = InvalidSigKind::Vote.as_str(),
                "dropping vote from penalized signer"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PenalizedSender,
            );
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::PenalizedSender,
            );
            return;
        }
        let key = super::VoteVerifyKey::from_vote(&vote);
        if self
            .subsystems
            .vote_verify
            .pending_validation
            .contains_key(&key)
            || self.subsystems.vote_verify.pending.contains_key(&key)
            || self.subsystems.vote_verify.inflight.contains_key(&key)
        {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping duplicate vote while validation is queued"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::Duplicate,
            );
            return;
        }
        if vote_duplicate(&self.vote_log, &vote) {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping duplicate vote already recorded"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::Duplicate,
            );
            return;
        }
        let pending_cap = self.config.worker.validation_pending_cap;
        if self.subsystems.vote_verify.pending_validation.len() >= pending_cap {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                pending = self.subsystems.vote_verify.pending_validation.len(),
                cap = pending_cap,
                "dropping vote: pending validation cap reached"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Backpressure,
            );
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::Backpressure,
            );
            return;
        }
        self.subsystems
            .vote_verify
            .pending_validation
            .insert(key, vote);
    }

    pub(super) fn process_pending_vote_validation(&mut self) -> bool {
        if self.subsystems.vote_verify.pending_validation.is_empty() {
            return false;
        }
        let keys: Vec<_> = self
            .subsystems
            .vote_verify
            .pending_validation
            .keys()
            .cloned()
            .take(VOTE_VERIFY_DEFERRED_DISPATCH_MAX)
            .collect();
        if keys.is_empty() {
            return false;
        }
        let mut progress = false;
        for key in keys {
            let Some(vote) = self.subsystems.vote_verify.pending_validation.remove(&key) else {
                continue;
            };
            self.handle_vote(vote);
            progress = true;
        }
        progress
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_vote(&mut self, vote: crate::sumeragi::consensus::Vote) {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let stale_view = self.stale_view(vote.height, vote.view);
        if self.drop_vote_for_height_or_view(&vote, committed_height, stale_view)
            || self.drop_precommit_vote_for_lock(&vote)
        {
            return;
        }
        if self.invalid_sig_penalty.is_suppressed(
            InvalidSigKind::Vote,
            vote.signer.into(),
            Instant::now(),
        ) {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                kind = InvalidSigKind::Vote.as_str(),
                "dropping vote from penalized signer"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PenalizedSender,
            );
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::PenalizedSender,
            );
            return;
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(vote.height);
        let mut topology_peers = if matches!(vote.phase, Phase::NewView) {
            self.roster_for_new_view_with_mode(
                vote.block_hash,
                vote.height,
                vote.view,
                consensus_mode,
            )
        } else {
            self.roster_for_vote_with_mode_observing_sidecar(
                vote.block_hash,
                vote.height,
                vote.view,
                consensus_mode,
                "vote_roster_lookup",
            )
        };
        if topology_peers.is_empty() && matches!(vote.phase, Phase::Commit) {
            let missing_request = self
                .pending
                .missing_block_requests
                .contains_key(&vote.block_hash);
            let allow_fallback = missing_request
                || (vote.height <= committed_height.saturating_add(1)
                    && (self.block_known_locally(vote.block_hash) || self.runtime_da_enabled()));
            if allow_fallback {
                let committed_epoch = self.epoch_for_height(committed_height);
                let epoch_matches = matches!(consensus_mode, ConsensusMode::Permissioned)
                    || vote.epoch == committed_epoch;
                if epoch_matches {
                    let world = self.state.world_view();
                    let commit_topology = self.state.commit_topology_snapshot();
                    let fallback = self.active_topology_with_genesis_fallback_from_world(
                        &world,
                        commit_topology.as_slice(),
                        committed_height,
                        consensus_mode,
                    );
                    if !fallback.is_empty() {
                        topology_peers =
                            super::roster::canonicalize_roster_for_mode(fallback, consensus_mode);
                        debug!(
                            height = vote.height,
                            view = vote.view,
                            signer = vote.signer,
                            block_hash = %vote.block_hash,
                            "using active topology fallback for precommit vote roster"
                        );
                    }
                }
            }
        }
        if topology_peers.is_empty() {
            if matches!(vote.phase, Phase::Prepare | Phase::Commit) {
                self.maybe_request_missing_block_for_unresolved_roster(&vote);
            }
            record_vote_drop_without_roster(
                &vote,
                super::status::VoteValidationDropReason::RosterMissing,
            );
            self.defer_vote_for_roster(vote, "commit topology missing");
            return;
        }
        let roster_hash = iroha_crypto::HashOf::new(&topology_peers);
        let pops = self.cached_vote_verify_pops(&topology_peers, &roster_hash);
        let topology = super::network_topology::Topology::new(topology_peers);
        let signature_topology = self.cached_signature_topology(
            vote.height,
            vote.view,
            &roster_hash,
            &topology,
            mode_tag,
            prf_seed,
        );
        let context = VoteProcessingContext {
            topology,
            signature_topology,
            consensus_mode,
            mode_tag,
            prf_seed,
            stale_view,
            pops,
        };
        let cache_key = super::VoteVerifyCacheKey::from_vote(&vote);
        if self
            .subsystems
            .vote_verify
            .verified_cache
            .contains(&cache_key)
        {
            let chain_id = self.common_config.chain.clone();
            let evidence_context = super::evidence::EvidenceValidationContext {
                topology: &context.topology,
                chain_id: &chain_id,
                mode_tag,
                prf_seed,
            };
            if self.validate_and_record_vote_with_signature_result(
                &vote,
                context.signature_topology.as_ref(),
                &evidence_context,
                mode_tag,
                Some(Ok(())),
            ) {
                self.apply_validated_vote(vote, context);
            }
            return;
        }
        if self.try_dispatch_vote_verification(&vote, &context) {
            return;
        }
        let chain_id = self.common_config.chain.clone();
        let evidence_context = super::evidence::EvidenceValidationContext {
            topology: &context.topology,
            chain_id: &chain_id,
            mode_tag,
            prf_seed,
        };
        if !self.validate_and_record_vote(
            &vote,
            context.signature_topology.as_ref(),
            &evidence_context,
            mode_tag,
        ) {
            return;
        }
        self.apply_validated_vote(vote, context);
    }

    pub(super) fn apply_validated_vote(
        &mut self,
        vote: crate::sumeragi::consensus::Vote,
        context: VoteProcessingContext,
    ) {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let topology_peers = context.topology.as_ref().to_vec();
        self.touch_pending_progress(vote.block_hash, vote.height, vote.view, Instant::now());
        if !matches!(vote.phase, Phase::NewView) {
            // NEW_VIEW votes reference the highest QC block hash; caching would poison rosters for
            // subsequent PREPARE/COMMIT votes on that block.
            self.cache_vote_roster(
                vote.block_hash,
                vote.height,
                vote.view,
                topology_peers.clone(),
            );
        }
        self.prune_vote_caches_horizon(committed_height);
        match vote.phase {
            Phase::Prepare | Phase::Commit => {
                self.try_form_qc_from_votes(
                    vote.phase,
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    vote.epoch,
                    &context.topology,
                );
                if matches!(vote.phase, Phase::Commit) {
                    if let Some(pending) = self.pending.pending_blocks.get(&vote.block_hash) {
                        if pending.aborted {
                            debug!(
                                height = vote.height,
                                view = vote.view,
                                block = %vote.block_hash,
                                "skipping block sync update for aborted pending block"
                            );
                            return;
                        }
                    }
                    let block = match self.pending.pending_blocks.get(&vote.block_hash) {
                        Some(pending) => pending.block.clone(),
                        None => return,
                    };
                    let world = self.state.world_view();
                    let block_time =
                        self.block_time_for_mode_from_world(&world, context.consensus_mode);
                    let cooldown = block_time.max(REBROADCAST_COOLDOWN_FLOOR);
                    if self.block_sync_rebroadcast_log.allow(
                        vote.block_hash,
                        std::time::Instant::now(),
                        cooldown,
                    ) {
                        let mut update = self.block_sync_update_for_precommit_vote(
                            &block,
                            self.state.as_ref(),
                            self.kura.as_ref(),
                            &self.qc_cache,
                            &self.vote_log,
                            &vote,
                        );
                        let commit_votes = update.commit_votes.len();
                        let has_commit_qc = update.commit_qc.is_some();
                        let should_broadcast =
                            match self.pending.pending_blocks.get_mut(&vote.block_hash) {
                                Some(pending) => pending.should_broadcast_block_sync_update(
                                    vote.view,
                                    commit_votes,
                                    has_commit_qc,
                                ),
                                None => false,
                            };
                        if !should_broadcast {
                            iroha_logger::trace!(
                                height = vote.height,
                                view = vote.view,
                                block = %vote.block_hash,
                                signer = vote.signer,
                                commit_votes,
                                has_commit_qc,
                                "skipping block sync update broadcast: no new commit votes"
                            );
                            return;
                        }
                        if self.prepare_block_sync_update_for_broadcast(
                            &mut update,
                            context.consensus_mode,
                        ) {
                            self.broadcast_block_sync_update(update, &topology_peers);
                            iroha_logger::info!(
                                height = vote.height,
                                view = vote.view,
                                block = %vote.block_hash,
                                signer = vote.signer,
                                targets = topology_peers.len(),
                                "sending block sync update with cached precommit votes to commit topology after recording vote"
                            );
                        } else {
                            match self.decide_no_roster_fallback_or_fail_closed(
                                vote.height,
                                vote.view,
                                vote.block_hash,
                                ViewChangeCause::MissingPayload,
                                "precommit_vote_no_roster",
                            ) {
                                super::NoRosterFallbackDecision::AllowFallback => {
                                    self.broadcast_block_created_for_block_sync(
                                        super::message::BlockCreated::from(&block),
                                        &topology_peers,
                                    );
                                    iroha_logger::info!(
                                        height = vote.height,
                                        view = vote.view,
                                        block = %vote.block_hash,
                                        signer = vote.signer,
                                        targets = topology_peers.len(),
                                        "sending BlockCreated payload to commit topology (no verifiable roster yet)"
                                    );
                                }
                                super::NoRosterFallbackDecision::BootstrapPending => {
                                    iroha_logger::debug!(
                                        height = vote.height,
                                        view = vote.view,
                                        block = %vote.block_hash,
                                        "deferring BlockCreated fallback broadcast while no-roster bootstrap is pending"
                                    );
                                }
                                super::NoRosterFallbackDecision::FailClosed => {
                                    iroha_logger::warn!(
                                        height = vote.height,
                                        view = vote.view,
                                        block = %vote.block_hash,
                                        "skipping BlockCreated fallback broadcast after no-roster fail-closed escalation"
                                    );
                                }
                            }
                        }
                    } else {
                        iroha_logger::trace!(
                            height = vote.height,
                            view = vote.view,
                            block = %vote.block_hash,
                            signer = vote.signer,
                            cooldown_ms = cooldown.as_millis(),
                            "skipping block sync update broadcast due to cooldown"
                        );
                    }
                }
            }
            Phase::NewView => {
                let Some(highest) = vote.highest_qc else {
                    debug!(
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        "skipping NEW_VIEW vote missing highest certificate reference"
                    );
                    return;
                };
                let signer_peer = usize::try_from(vote.signer)
                    .ok()
                    .and_then(|idx| context.signature_topology.as_ref().as_ref().get(idx))
                    .cloned();
                let Some(signer_peer) = signer_peer else {
                    warn!(
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        roster_len = context.signature_topology.as_ref().as_ref().len(),
                        "dropping NEW_VIEW vote: signer index out of range"
                    );
                    return;
                };
                self.observe_new_view_highest_qc(highest);
                crate::sumeragi::new_view_stats::note_receipt(vote.height, vote.view, &signer_peer);
                if let Some(local_view) = context.stale_view {
                    debug!(
                        height = vote.height,
                        view = vote.view,
                        local_view,
                        signer = vote.signer,
                        "recorded stale NEW_VIEW vote"
                    );
                    return;
                }
                let count = self.subsystems.propose.new_view_tracker.record(
                    vote.height,
                    vote.view,
                    signer_peer,
                    highest,
                );
                debug!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    count,
                    "recorded NEW_VIEW vote"
                );
                self.try_form_qc_from_votes(
                    vote.phase,
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    vote.epoch,
                    &context.topology,
                );
            }
        }
    }

    fn try_dispatch_vote_verification(
        &mut self,
        vote: &crate::sumeragi::consensus::Vote,
        context: &VoteProcessingContext,
    ) -> bool {
        if self.subsystems.vote_verify.work_txs.is_empty() {
            return false;
        }
        let key = super::VoteVerifyKey::from_vote(vote);
        if self.subsystems.vote_verify.inflight.contains_key(&key)
            || self.subsystems.vote_verify.pending.contains_key(&key)
        {
            debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping duplicate vote while verification is in flight"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
            record_vote_drop_without_roster(
                vote,
                super::status::VoteValidationDropReason::Duplicate,
            );
            return true;
        }
        let id = self.subsystems.vote_verify.next_id();
        let work = self.build_vote_verify_work(id, key.clone(), vote, context);
        if self.try_send_vote_verify_work(work).is_ok() {
            self.subsystems.vote_verify.inflight.insert(
                key,
                super::VoteVerifyInFlight {
                    id,
                    vote: vote.clone(),
                    context: context.clone(),
                },
            );
            return true;
        }
        if self.subsystems.vote_verify.work_txs.is_empty() {
            return false;
        }
        self.subsystems.vote_verify.pending.insert(
            key,
            super::VoteVerifyPending {
                id,
                vote: vote.clone(),
                context: context.clone(),
            },
        );
        true
    }

    pub(super) fn prune_vote_caches_horizon(&mut self, committed_height: u64) {
        let highest_commit = self
            .highest_qc
            .filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
        let anchor_height =
            highest_commit.map_or(committed_height, |qc| qc.height.max(committed_height));
        let active_height = anchor_height.saturating_add(1);
        let min_height = active_height.saturating_sub(super::VOTE_CACHE_HEIGHT_WINDOW);
        let max_height = active_height.saturating_add(super::VOTE_CACHE_HEIGHT_WINDOW);
        let current_view = self.phase_tracker.current_view(active_height);
        let min_view = current_view.map(|view| view.saturating_sub(super::VOTE_CACHE_VIEW_WINDOW));
        let max_view = current_view.map(|view| view.saturating_add(super::VOTE_CACHE_VIEW_WINDOW));
        let should_keep = |height: u64, view: u64| -> bool {
            if height <= committed_height || height < min_height || height > max_height {
                return false;
            }
            if height == active_height {
                if let Some(min_view) = min_view {
                    if view < min_view {
                        return false;
                    }
                }
                if let Some(max_view) = max_view {
                    if view > max_view {
                        return false;
                    }
                }
            }
            true
        };
        self.vote_log
            .retain(|(_, height, view, _, _), _| should_keep(*height, *view));
        self.vote_validation_cache
            .retain(|(_, height, view, _, _), _| should_keep(*height, *view));
        self.qc_cache
            .retain(|(_, _, height, view, _), _| should_keep(*height, *view));
        self.qc_signer_tally
            .retain(|(_, _, height, view, _), _| should_keep(*height, *view));
        self.subsystems
            .qc_verify
            .verified_cache
            .retain(|key| should_keep(key.key.height, key.key.view));
        self.vote_roster_cache
            .retain(|_, entry| should_keep(entry.height, entry.view));
        self.deferred_qcs
            .retain(|(_, _, height, view, _), _| should_keep(*height, *view));
        self.deferred_missing_payload_qcs
            .retain(|(_, _, height, view, _), _| should_keep(*height, *view));
        self.quarantined_block_sync_qcs
            .retain(|(_, _, height, view, _), _| should_keep(*height, *view));
        self.deferred_votes.retain(|_, votes| {
            votes.retain(|(_, height, view, _, _), _| should_keep(*height, *view));
            !votes.is_empty()
        });
        self.subsystems
            .vote_verify
            .pending
            .retain(|key, _| should_keep(key.height, key.view));
        self.subsystems
            .vote_verify
            .pending_validation
            .retain(|key, _| should_keep(key.height, key.view));
        self.subsystems
            .vote_verify
            .signature_topology_cache
            .retain(|key, _| should_keep(key.height, key.view));
        self.subsystems
            .vote_verify
            .verified_cache
            .retain(|key| should_keep(key.key.height, key.key.view));
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn drop_vote_for_height_or_view(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
        committed_height: u64,
        stale_view: Option<u64>,
    ) -> bool {
        if vote.height <= committed_height {
            let matches_committed = vote.phase == Phase::Commit
                && self
                    .kura
                    .get_block_height_by_hash(vote.block_hash)
                    .is_some_and(|height| {
                        u64::try_from(height.get()).is_ok_and(|height| height == vote.height)
                    });
            if matches_committed {
                iroha_logger::debug!(
                    phase = ?vote.phase,
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    committed_height,
                    "accepting commit vote for committed block"
                );
            } else {
                iroha_logger::debug!(
                    phase = ?vote.phase,
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    committed_height,
                    "dropping stale vote below committed height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StaleHeight,
                );
                record_vote_drop_without_roster(
                    vote,
                    super::status::VoteValidationDropReason::StaleHeight,
                );
                return true;
            }
        }
        let expected_epoch = self.epoch_for_height(vote.height);
        if vote.epoch != expected_epoch {
            iroha_logger::debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                expected_epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping vote with mismatched epoch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::EpochMismatch,
            );
            record_vote_drop_without_roster(
                vote,
                super::status::VoteValidationDropReason::EpochMismatch,
            );
            return true;
        }
        if let Some(local_view) = stale_view {
            let da_enabled = self.runtime_da_enabled();
            let missing_request = self
                .pending
                .missing_block_requests
                .contains_key(&vote.block_hash);
            if vote.phase == crate::sumeragi::consensus::Phase::Commit
                && (self.block_known_locally(vote.block_hash) || da_enabled || missing_request)
            {
                iroha_logger::debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    da_enabled,
                    missing_request,
                    "accepting precommit vote for stale view"
                );
            } else if vote.phase == crate::sumeragi::consensus::Phase::NewView {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    signer = vote.signer,
                    "accepting stale NEW_VIEW vote for highest QC processing"
                );
            } else {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    kind = "Vote",
                    "dropping consensus message for stale view"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::StaleView,
                );
                record_vote_drop_without_roster(
                    vote,
                    super::status::VoteValidationDropReason::StaleView,
                );
                return true;
            }
        }
        false
    }

    pub(super) fn request_missing_block_for_highest_qc(
        &mut self,
        highest: crate::sumeragi::consensus::QcHeaderRef,
        source: &'static str,
    ) -> bool {
        self.request_missing_block_for_highest_qc_inner(highest, source, false, true)
    }

    pub(super) fn request_missing_block_for_highest_qc_force(
        &mut self,
        highest: crate::sumeragi::consensus::QcHeaderRef,
        source: &'static str,
    ) -> bool {
        self.request_missing_block_for_highest_qc_inner(highest, source, true, true)
    }

    pub(super) fn request_missing_block_for_highest_qc_force_skip_sidecar_observation(
        &mut self,
        highest: crate::sumeragi::consensus::QcHeaderRef,
        source: &'static str,
    ) -> bool {
        self.sidecar_observation_suppression_depth =
            self.sidecar_observation_suppression_depth.saturating_add(1);
        let requested =
            self.request_missing_block_for_highest_qc_inner(highest, source, true, false);
        self.sidecar_observation_suppression_depth =
            self.sidecar_observation_suppression_depth.saturating_sub(1);
        requested
    }

    fn consensus_missing_block_retry_window(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Duration {
        let quorum_retry = self.quorum_timeout(self.runtime_da_enabled());
        let mut retry_window = super::quorum_reschedule_backoff_from_timeout(quorum_retry);
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height > committed_height.saturating_add(1) {
            retry_window = retry_window.min(self.rebroadcast_cooldown());
        }
        self.missing_block_retry_window_with_rbc_progress(block_hash, height, view, retry_window)
    }

    #[allow(clippy::too_many_lines)]
    fn request_missing_block_for_highest_qc_inner(
        &mut self,
        highest: crate::sumeragi::consensus::QcHeaderRef,
        source: &'static str,
        force_fetch: bool,
        observe_sidecar_mismatch: bool,
    ) -> bool {
        let payload_available =
            self.block_payload_available_for_progress(highest.subject_block_hash);
        let local_known = self
            .local_block_height_view(highest.subject_block_hash)
            .is_some();
        if observe_sidecar_mismatch {
            self.observe_sidecar_mismatch_for_height(
                highest.height,
                highest.subject_block_hash,
                "highest_qc_missing_block",
            );
        }
        if payload_available && !force_fetch && local_known {
            self.clear_missing_block_request(
                &highest.subject_block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return false;
        }
        let (consensus_mode, mode_tag, _) = self.consensus_context_for_height(highest.height);
        let mut roster = self.roster_for_vote_with_mode(
            highest.subject_block_hash,
            highest.height,
            highest.view,
            consensus_mode,
        );
        let mut signers = BTreeSet::new();
        let mut roster_source = "commit topology";
        if roster.is_empty() {
            let expected_epoch = self.epoch_for_height(highest.height);
            if let Some(record) = super::status::precommit_signers_for_round(
                highest.subject_block_hash,
                highest.height,
                highest.view,
                expected_epoch,
            ) {
                if record.mode_tag.as_str() == mode_tag && !record.validator_set.is_empty() {
                    let candidate = super::roster::canonicalize_roster_for_mode(
                        record.validator_set.clone(),
                        consensus_mode,
                    );
                    let mut candidate_signers = record.signers.clone();
                    if candidate != record.validator_set {
                        candidate_signers.clear();
                    }
                    let signers_in_range = candidate_signers.iter().all(|idx| {
                        usize::try_from(*idx)
                            .ok()
                            .is_some_and(|idx| idx < candidate.len())
                    });
                    if !signers_in_range {
                        candidate_signers.clear();
                    }
                    if !candidate.is_empty() {
                        roster = candidate;
                        signers = candidate_signers;
                        roster_source = "precommit signer history";
                    }
                }
            }
        }
        if roster.is_empty() {
            let expected_epoch = self.epoch_for_height(highest.height);
            if let Some(cert) = super::status::commit_qc_history().into_iter().find(|cert| {
                cert.phase == crate::sumeragi::consensus::Phase::Commit
                    && cert.subject_block_hash == highest.subject_block_hash
                    && cert.height == highest.height
                    && cert.view == highest.view
                    && cert.epoch == expected_epoch
                    && cert.mode_tag == mode_tag
                    && !cert.validator_set.is_empty()
            }) {
                roster = super::roster::canonicalize_roster_for_mode(
                    cert.validator_set.clone(),
                    consensus_mode,
                );
                roster_source = "commit QC history";
            }
        }
        if roster.is_empty() {
            if let Some(cert) = super::status::commit_qc_history().into_iter().find(|cert| {
                cert.phase == crate::sumeragi::consensus::Phase::Commit
                    && cert.mode_tag == mode_tag
                    && !cert.validator_set.is_empty()
            }) {
                roster = super::roster::canonicalize_roster_for_mode(
                    cert.validator_set.clone(),
                    consensus_mode,
                );
                roster_source = "latest commit QC history";
            }
        }
        if roster.is_empty() {
            roster = self.effective_commit_topology();
            if !roster.is_empty() {
                signers.clear();
                roster_source = "commit topology fallback";
            }
        }
        if roster.is_empty() {
            let commit_topology = self.state.commit_topology_snapshot();
            if !commit_topology.is_empty() {
                roster =
                    super::roster::canonicalize_roster_for_mode(commit_topology, consensus_mode);
                signers.clear();
                roster_source = "commit topology snapshot";
            }
        }
        if roster.is_empty() {
            let trusted = self.trusted_topology();
            if !trusted.is_empty() {
                roster = trusted;
                signers.clear();
                roster_source = "trusted topology fallback";
            }
        }
        if roster.is_empty() {
            debug!(
                height = highest.height,
                view = highest.view,
                block = %highest.subject_block_hash,
                source,
                "skipping highest QC fetch: no roster available"
            );
            return false;
        }
        if roster_source != "commit topology" {
            debug!(
                height = highest.height,
                view = highest.view,
                block = %highest.subject_block_hash,
                roster_source,
                source,
                "using fallback roster for highest QC fetch"
            );
        }
        let topology = super::network_topology::Topology::new(roster);
        let retry_window = self.consensus_missing_block_retry_window(
            highest.subject_block_hash,
            highest.height,
            highest.view,
        );
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let now = Instant::now();
        let decision = plan_missing_block_fetch(
            &mut self.pending.missing_block_requests,
            highest.subject_block_hash,
            highest.height,
            highest.view,
            crate::sumeragi::consensus::Phase::Commit,
            MissingBlockPriority::Consensus,
            &signers,
            &topology,
            now,
            retry_window,
            None,
            signer_fallback_attempts,
        );
        let dwell = self
            .pending
            .missing_block_requests
            .get(&highest.subject_block_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let dwell_ms = dwell.as_millis();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(
            targets_len,
            dwell.as_millis().try_into().unwrap_or(u64::MAX),
        );
        match &decision {
            MissingBlockFetchDecision::Requested { target_kind, .. } => {
                self.note_missing_block_height_attempt(
                    highest.subject_block_hash,
                    highest.height,
                    highest.view,
                    super::MissingBlockRecoveryStage::HashFetch,
                    Some(*target_kind),
                    now,
                );
            }
            MissingBlockFetchDecision::NoTargets => {
                self.note_missing_block_height_attempt(
                    highest.subject_block_hash,
                    highest.height,
                    highest.view,
                    super::MissingBlockRecoveryStage::HashFetch,
                    None,
                    now,
                );
            }
            MissingBlockFetchDecision::Backoff => {
                self.note_missing_block_height_hash_miss(
                    highest.subject_block_hash,
                    highest.height,
                    highest.view,
                    super::MissingBlockRecoveryStage::HashFetch,
                    now,
                );
            }
        }
        let _ = self.maybe_escalate_missing_block_height_recovery(
            highest.subject_block_hash,
            highest.height,
            highest.view,
            now,
        );
        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    highest.subject_block_hash,
                    highest.height,
                    highest.view,
                    MissingBlockPriority::Consensus,
                    &targets,
                );
                iroha_logger::info!(
                    height = highest.height,
                    view = highest.view,
                    block = ?highest.subject_block_hash,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    source,
                    "requested missing block payload from highest QC"
                );
                true
            }
            MissingBlockFetchDecision::NoTargets => {
                iroha_logger::warn!(
                    height = highest.height,
                    view = highest.view,
                    block = ?highest.subject_block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    targets = targets_len,
                    source,
                    "unable to request missing block payload from highest QC: no peers available"
                );
                false
            }
            MissingBlockFetchDecision::Backoff => {
                iroha_logger::info!(
                    height = highest.height,
                    view = highest.view,
                    block = ?highest.subject_block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    targets = targets_len,
                    source,
                    "skipping missing-block fetch from highest QC during backoff"
                );
                false
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn observe_new_view_highest_qc(
        &mut self,
        highest: crate::sumeragi::consensus::QcHeaderRef,
    ) {
        let should_update = self.highest_qc.is_none_or(|current| {
            let incoming = (highest.height, highest.view);
            let existing = (current.height, current.view);
            let promotes_phase = incoming == existing
                && highest.phase == crate::sumeragi::consensus::Phase::Commit
                && current.phase != crate::sumeragi::consensus::Phase::Commit;
            incoming > existing || promotes_phase
        });
        if should_update {
            self.highest_qc = Some(highest);
            super::status::set_highest_qc(highest.height, highest.view);
            super::status::set_highest_qc_hash(highest.subject_block_hash);
        }
        self.request_missing_block_for_highest_qc(highest, "new_view");
    }

    pub(super) fn drop_precommit_vote_for_lock(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
    ) -> bool {
        if !matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit) {
            return false;
        }
        let Some(lock) = self.locked_qc else {
            return false;
        };
        if vote.height < lock.height {
            iroha_logger::debug!(
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                locked_height = lock.height,
                locked_hash = %lock.subject_block_hash,
                "dropping precommit vote below locked height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return true;
        }
        if !self.block_known_for_lock(lock.subject_block_hash) {
            return false;
        }
        if vote.height == lock.height {
            if vote.block_hash != lock.subject_block_hash {
                iroha_logger::debug!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    "dropping precommit vote that conflicts with locked block"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                record_vote_drop_without_roster(
                    vote,
                    super::status::VoteValidationDropReason::LockedQc,
                );
                return true;
            }
        }
        if self.block_known_locally(vote.block_hash) {
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
                iroha_logger::debug!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    "dropping precommit vote that does not extend locked chain"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                record_vote_drop_without_roster(
                    vote,
                    super::status::VoteValidationDropReason::LockedQc,
                );
                return true;
            }
        }
        false
    }

    #[allow(clippy::too_many_lines)]
    fn validate_and_record_vote(
        &mut self,
        vote: &crate::sumeragi::consensus::Vote,
        signature_topology: &super::network_topology::Topology,
        evidence_context: &super::evidence::EvidenceValidationContext<'_>,
        mode_tag: &str,
    ) -> bool {
        self.validate_and_record_vote_with_signature_result(
            vote,
            signature_topology,
            evidence_context,
            mode_tag,
            None,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn validate_and_record_vote_with_signature_result(
        &mut self,
        vote: &crate::sumeragi::consensus::Vote,
        signature_topology: &super::network_topology::Topology,
        evidence_context: &super::evidence::EvidenceValidationContext<'_>,
        mode_tag: &str,
        signature_result: Option<Result<(), VoteSignatureError>>,
    ) -> bool {
        let (consensus_mode, _, _) = self.consensus_context_for_height(vote.height);
        let roster_len = u32::try_from(signature_topology.as_ref().len()).unwrap_or(u32::MAX);
        let roster_hash = iroha_crypto::HashOf::new(&signature_topology.as_ref().to_vec());
        let canonical_roster = super::roster::canonicalize_roster_for_mode(
            signature_topology.as_ref().to_vec(),
            consensus_mode,
        );
        let membership_hash = iroha_crypto::HashOf::new(&canonical_roster);
        let peer_id = usize::try_from(vote.signer)
            .ok()
            .and_then(|idx| signature_topology.as_ref().get(idx).cloned());
        let record_drop = |reason| {
            super::status::record_vote_validation_drop(super::status::VoteValidationDropRecord {
                reason,
                height: vote.height,
                view: vote.view,
                epoch: vote.epoch,
                signer_index: vote.signer,
                peer_id: peer_id.clone(),
                roster_hash: Some(roster_hash),
                roster_len,
                block_hash: vote.block_hash,
            });
            log_vote_validation_drop(
                vote,
                reason,
                peer_id.as_ref(),
                &roster_hash,
                roster_len,
                mode_tag,
            );
        };
        if vote_duplicate(&self.vote_log, vote) {
            iroha_logger::debug!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping duplicate vote already recorded"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
            record_drop(super::status::VoteValidationDropReason::Duplicate);
            return false;
        }
        let signature_result = signature_result.unwrap_or_else(|| {
            vote_signature_check(
                vote,
                signature_topology,
                &self.common_config.chain,
                mode_tag,
            )
        });
        if signature_result.is_ok() {
            self.subsystems
                .vote_verify
                .verified_cache
                .insert(super::VoteVerifyCacheKey::from_vote(vote));
        }
        match signature_result {
            Ok(()) => {}
            Err(err) => {
                let outcome = self.record_invalid_signature(
                    InvalidSigKind::Vote,
                    vote.height,
                    vote.view,
                    vote.signer.into(),
                );
                let roster_len = signature_topology.as_ref().len();
                if matches!(outcome, InvalidSigOutcome::Logged) {
                    warn!(
                        phase = ?vote.phase,
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        block_hash = %vote.block_hash,
                        peer = ?peer_id,
                        roster_len,
                        roster_hash = %roster_hash,
                        mode_tag,
                        ?err,
                        "dropping vote with invalid signature"
                    );
                } else {
                    debug!(
                        phase = ?vote.phase,
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        block_hash = %vote.block_hash,
                        peer = ?peer_id,
                        roster_len,
                        roster_hash = %roster_hash,
                        mode_tag,
                        ?err,
                        "suppressing repeated invalid vote signature log"
                    );
                }
                let drop_reason = match err {
                    super::VoteSignatureError::SignerIndexOverflow(_) => {
                        super::status::VoteValidationDropReason::SignerIndexOverflow
                    }
                    super::VoteSignatureError::SignerOutOfRange { .. } => {
                        super::status::VoteValidationDropReason::SignerOutOfRange
                    }
                    super::VoteSignatureError::SignatureInvalid => {
                        super::status::VoteValidationDropReason::SignatureInvalid
                    }
                };
                record_drop(drop_reason);
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidSignature,
                );
                return false;
            }
        }
        if vote.phase == Phase::NewView {
            // NEW_VIEW votes sign only the block hash, so highest QC fields must be checked here.
            let Some(highest) = vote.highest_qc else {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    "dropping NEW_VIEW vote missing highest certificate reference"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::MissingHighestQc,
                );
                record_drop(super::status::VoteValidationDropReason::MissingHighestQc);
                return false;
            };
            let expected_epoch = self.epoch_for_height(highest.height);
            if highest.epoch != expected_epoch {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    highest_height = highest.height,
                    highest_epoch = highest.epoch,
                    expected_epoch,
                    "dropping NEW_VIEW vote with mismatched highest QC epoch"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::EpochMismatch,
                );
                record_drop(super::status::VoteValidationDropReason::EpochMismatch);
                return false;
            }
            let valid_phase =
                matches!(highest.phase, Phase::Commit) || matches!(highest.phase, Phase::Prepare);
            if !valid_phase {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    highest_height = highest.height,
                    highest_view = highest.view,
                    phase = ?highest.phase,
                    "dropping NEW_VIEW vote with invalid highest certificate phase"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                record_drop(super::status::VoteValidationDropReason::HighestQcMismatch);
                return false;
            }
            if highest.subject_block_hash != vote.block_hash {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    highest_hash = %highest.subject_block_hash,
                    "dropping NEW_VIEW vote with mismatched highest block hash"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                record_drop(super::status::VoteValidationDropReason::HighestQcMismatch);
                return false;
            }
            let expected_height = highest.height.saturating_add(1);
            if vote.height != expected_height {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    highest_height = highest.height,
                    expected_height,
                    "dropping NEW_VIEW vote with mismatched height"
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::QcVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::HighestQcMismatch,
                );
                record_drop(super::status::VoteValidationDropReason::HighestQcMismatch);
                return false;
            }
            if let Some((local_height, local_view)) =
                self.local_block_height_view(highest.subject_block_hash)
            {
                if local_height != highest.height || local_view != highest.view {
                    warn!(
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        highest_height = highest.height,
                        highest_view = highest.view,
                        local_height,
                        local_view,
                        "dropping NEW_VIEW vote with highest QC that mismatches local block metadata"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::QcVote,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::HighestQcMismatch,
                    );
                    record_drop(super::status::VoteValidationDropReason::HighestQcMismatch);
                    return false;
                }
            }
        }
        let key = vote_key(vote);
        let cache_entry = VoteValidationCacheEntry {
            roster_hash,
            membership_hash,
        };
        if let Some(existing) = self.vote_log.get(&key).cloned() {
            if existing.block_hash == vote.block_hash {
                iroha_logger::debug!(
                    phase = ?vote.phase,
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    "dropping duplicate vote already recorded"
                );
                record_drop(super::status::VoteValidationDropReason::Duplicate);
                return false;
            }
            self.note_double_vote(Some(&existing), vote, evidence_context);
            let cross_phase = match vote.phase {
                crate::sumeragi::consensus::Phase::Prepare => {
                    Some(crate::sumeragi::consensus::Phase::Commit)
                }
                crate::sumeragi::consensus::Phase::Commit => {
                    Some(crate::sumeragi::consensus::Phase::Prepare)
                }
                _ => None,
            };
            if let Some(phase) = cross_phase {
                if let Some(previous) = self
                    .vote_log
                    .get(&(phase, vote.height, vote.view, vote.epoch, vote.signer))
                    .cloned()
                {
                    self.note_double_vote(Some(&previous), vote, evidence_context);
                }
            }
            warn!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                existing_block = %existing.block_hash,
                "dropping conflicting vote for signer"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::QcVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ConflictingVote,
            );
            record_drop(super::status::VoteValidationDropReason::ConflictingVote);
            return false;
        }
        let previous = self.vote_log.insert(key, vote.clone());
        self.vote_validation_cache.insert(key, cache_entry);
        iroha_logger::debug!(
            phase = ?vote.phase,
            height = vote.height,
            view = vote.view,
            epoch = vote.epoch,
            signer = vote.signer,
            block_hash = ?vote.block_hash,
            duplicate = previous.is_some(),
            "recorded vote"
        );
        self.note_double_vote(previous.as_ref(), vote, evidence_context);
        let cross_phase = match vote.phase {
            crate::sumeragi::consensus::Phase::Prepare => {
                Some(crate::sumeragi::consensus::Phase::Commit)
            }
            crate::sumeragi::consensus::Phase::Commit => {
                Some(crate::sumeragi::consensus::Phase::Prepare)
            }
            _ => None,
        };
        if let Some(phase) = cross_phase {
            if let Some(previous) = self
                .vote_log
                .get(&(phase, vote.height, vote.view, vote.epoch, vote.signer))
                .cloned()
            {
                self.note_double_vote(Some(&previous), vote, evidence_context);
            }
        }
        true
    }

    pub(super) fn cache_vote_roster(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        roster: Vec<PeerId>,
    ) {
        if roster.is_empty() {
            return;
        }
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let roster = super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
        let entry = VoteRosterCacheEntry {
            roster,
            height,
            view,
        };
        match self.vote_roster_cache.entry(block_hash) {
            Entry::Vacant(slot) => {
                slot.insert(entry);
            }
            Entry::Occupied(existing) => {
                let cached = existing.get();
                if cached.roster != entry.roster {
                    warn!(
                        block = %block_hash,
                        cached = cached.roster.len(),
                        observed = entry.roster.len(),
                        "vote roster mismatch; keeping cached roster to preserve vote validity"
                    );
                } else if cached.height != entry.height || cached.view != entry.view {
                    warn!(
                        block = %block_hash,
                        cached_height = cached.height,
                        cached_view = cached.view,
                        observed_height = entry.height,
                        observed_view = entry.view,
                        "vote roster height/view mismatch; keeping cached roster to preserve vote validity"
                    );
                }
            }
        }
        self.replay_deferred_votes_for_block(block_hash);
    }

    fn defer_vote_for_roster(
        &mut self,
        vote: crate::sumeragi::consensus::Vote,
        reason: &'static str,
    ) {
        let key = vote_key(&vote);
        let phase = vote.phase;
        let height = vote.height;
        let view = vote.view;
        let signer = vote.signer;
        let block_hash = vote.block_hash;
        let deferred = {
            let entry = self.deferred_votes.entry(block_hash).or_default();
            if entry.contains_key(&key) {
                debug!(
                    phase = ?phase,
                    height,
                    view,
                    signer,
                    block_hash = %block_hash,
                    reason,
                    "deferring duplicate vote while roster is unresolved"
                );
                return;
            }
            entry.insert(key, vote);
            entry.len()
        };
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::QcVote,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::RosterMissing,
        );
        info!(
            phase = ?phase,
            height,
            view,
            signer,
            block_hash = %block_hash,
            deferred,
            reason,
            "deferring vote until commit roster is available"
        );
    }

    fn maybe_request_missing_block_for_unresolved_roster(
        &mut self,
        vote: &crate::sumeragi::consensus::Vote,
    ) {
        if self.block_known_locally(vote.block_hash) {
            return;
        }
        let mut roster = self.effective_commit_topology();
        let mut roster_source = "commit topology";
        if roster.is_empty() {
            roster = self.trusted_topology();
            roster_source = "trusted topology fallback";
        }
        if roster.is_empty() {
            debug!(
                height = vote.height,
                view = vote.view,
                phase = ?vote.phase,
                block_hash = %vote.block_hash,
                "skipping missing-block fetch: no roster available"
            );
            return;
        }
        let topology = super::network_topology::Topology::new(roster);
        let retry_window =
            self.consensus_missing_block_retry_window(vote.block_hash, vote.height, vote.view);
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let now = Instant::now();
        let signers = BTreeSet::new();
        let decision = plan_missing_block_fetch(
            &mut self.pending.missing_block_requests,
            vote.block_hash,
            vote.height,
            vote.view,
            vote.phase,
            MissingBlockPriority::Consensus,
            &signers,
            &topology,
            now,
            retry_window,
            None,
            signer_fallback_attempts,
        );
        let dwell = self
            .pending
            .missing_block_requests
            .get(&vote.block_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let dwell_ms = dwell.as_millis();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(
            targets_len,
            dwell.as_millis().try_into().unwrap_or(u64::MAX),
        );
        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    MissingBlockPriority::Consensus,
                    &targets,
                );
                iroha_logger::info!(
                    height = vote.height,
                    view = vote.view,
                    phase = ?vote.phase,
                    block = ?vote.block_hash,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    roster_source,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    "requested missing block payload after roster fallback"
                );
            }
            MissingBlockFetchDecision::NoTargets => {
                iroha_logger::warn!(
                    height = vote.height,
                    view = vote.view,
                    phase = ?vote.phase,
                    block = ?vote.block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    targets = targets_len,
                    roster_source,
                    "unable to request missing block payload after roster fallback: no peers available"
                );
            }
            MissingBlockFetchDecision::Backoff => {
                iroha_logger::info!(
                    height = vote.height,
                    view = vote.view,
                    phase = ?vote.phase,
                    block = ?vote.block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms,
                    targets = targets_len,
                    roster_source,
                    "skipping missing-block fetch after roster fallback during backoff"
                );
            }
        }
    }

    fn replay_deferred_votes_for_block(&mut self, block_hash: HashOf<BlockHeader>) {
        let Some(deferred) = self.deferred_votes.remove(&block_hash) else {
            return;
        };
        let count = deferred.len();
        if count == 0 {
            return;
        }
        info!(
            block = %block_hash,
            deferred = count,
            "replaying deferred votes after roster resolution"
        );
        for (_, vote) in deferred {
            self.handle_vote(vote);
        }
    }

    pub(super) fn try_replay_deferred_votes(&mut self) -> bool {
        if self.deferred_votes.is_empty() {
            return false;
        }
        let mut resolved = Vec::new();
        let deferred_samples: Vec<_> = self
            .deferred_votes
            .iter()
            .filter_map(|(block_hash, votes)| {
                votes
                    .values()
                    .next()
                    .cloned()
                    .map(|vote| (*block_hash, vote))
            })
            .collect();
        for (block_hash, vote) in deferred_samples {
            let (consensus_mode, _, _) = self.consensus_context_for_height(vote.height);
            let roster = if matches!(vote.phase, Phase::NewView) {
                self.roster_for_new_view_with_mode(
                    block_hash,
                    vote.height,
                    vote.view,
                    consensus_mode,
                )
            } else {
                self.roster_for_vote_with_mode_observing_sidecar(
                    block_hash,
                    vote.height,
                    vote.view,
                    consensus_mode,
                    "deferred_vote_roster_lookup",
                )
            };
            if !roster.is_empty() {
                resolved.push((block_hash, roster));
            }
        }
        let replayed = !resolved.is_empty();
        for (block_hash, roster) in resolved {
            let Some(sample) = self
                .deferred_votes
                .get(&block_hash)
                .and_then(|votes| votes.values().next().map(|vote| (vote.height, vote.view)))
            else {
                continue;
            };
            self.cache_vote_roster(block_hash, sample.0, sample.1, roster);
        }
        replayed
    }

    fn note_double_vote(
        &mut self,
        previous: Option<&crate::sumeragi::consensus::Vote>,
        current: &crate::sumeragi::consensus::Vote,
        evidence_context: &super::evidence::EvidenceValidationContext<'_>,
    ) {
        let Some(previous) = previous else {
            return;
        };
        if previous.block_hash == current.block_hash {
            return;
        }
        if super::evidence::record_double_vote(
            &mut self.evidence_store,
            self.state.as_ref(),
            previous,
            current,
            evidence_context,
        ) {
            iroha_logger::warn!(
                phase = ?current.phase,
                height = current.height,
                view = current.view,
                epoch = current.epoch,
                signer = current.signer,
                "double vote detected; storing evidence"
            );
        }
    }

    pub(super) fn block_sync_update_for_precommit_vote(
        &self,
        block: &SignedBlock,
        state: &State,
        kura: &Kura,
        qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
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
        vote: &crate::sumeragi::consensus::Vote,
    ) -> super::message::BlockSyncUpdate {
        let mut update = super::block_sync_update_with_roster(
            block,
            state,
            kura,
            self.config.consensus_mode,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            &self.roster_validation_cache,
        );
        Self::apply_cached_qcs_to_block_sync_update(
            &mut update,
            qc_cache,
            vote_log,
            vote.block_hash,
            vote.height,
            vote.view,
            vote.epoch,
            state,
            self.config.consensus_mode,
        );
        update
    }

    // Prefer the roster tied to a committed block to keep signatures valid across roster changes.
    #[allow(dead_code)]
    pub(super) fn roster_for_vote(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Vec<PeerId> {
        self.roster_for_vote_with_mode(block_hash, height, view, self.consensus_mode)
    }

    fn pending_chain_hashes_for_parent(
        &self,
        mut parent_hash: HashOf<BlockHeader>,
        mut parent_height: u64,
        committed_height: u64,
    ) -> Option<BTreeMap<u64, HashOf<BlockHeader>>> {
        if parent_height <= committed_height {
            return None;
        }
        let mut hashes = BTreeMap::new();
        loop {
            hashes.insert(parent_height, parent_hash);
            if parent_height <= committed_height.saturating_add(1) {
                break;
            }
            let pending = self.pending.pending_blocks.get(&parent_hash)?;
            if pending.height != parent_height {
                return None;
            }
            parent_hash = pending.block.header().prev_block_hash()?;
            parent_height = parent_height.saturating_sub(1);
        }
        Some(hashes)
    }

    #[allow(clippy::unused_self)]
    pub(super) fn roster_from_commit_qc_history(&self, height: u64) -> Option<Vec<PeerId>> {
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let parent_height = height.checked_sub(1)?;
        let cert = super::status::commit_qc_history()
            .into_iter()
            .filter(|candidate| {
                candidate.height == parent_height
                    && matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit)
            })
            .max_by(|a, b| a.height.cmp(&b.height).then_with(|| a.view.cmp(&b.view)))?;
        if cert.validator_set.is_empty() {
            return None;
        }
        let mut topology = super::network_topology::Topology::new(cert.validator_set.clone());
        topology.block_committed(cert.validator_set.clone(), cert.subject_block_hash);
        let mut roster = topology.as_ref().to_vec();
        if roster.is_empty() {
            return None;
        }
        let world = self.state.world_view();
        roster = super::roster::filter_roster_with_live_consensus_keys_at_height_world(
            &world, roster, height,
        );
        if roster.is_empty() {
            return None;
        }
        roster = super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
        Some(roster)
    }

    pub(super) fn roster_from_commit_qc_history_roll_forward(
        &self,
        height: u64,
        target_parent_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<Vec<PeerId>> {
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let target_parent = height.checked_sub(1)?;
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(0);
        let committed_hash_for_height = |h: u64| {
            let h_usize = usize::try_from(h).ok()?;
            let height = NonZeroUsize::new(h_usize)?;
            self.kura.get_block_hash(height)
        };
        let pending_hashes = target_parent_hash.and_then(|hash| {
            self.pending_chain_hashes_for_parent(hash, target_parent, committed_height)
        });
        let known_hash_for_height = |h: u64| {
            if h == 0 {
                return None;
            }
            if let Some(hashes) = pending_hashes.as_ref() {
                if let Some(hash) = hashes.get(&h) {
                    return Some(*hash);
                }
            }
            if h <= committed_height {
                return committed_hash_for_height(h);
            }
            None
        };
        if let Some(target_hash) = target_parent_hash {
            if let Some(known_hash) = known_hash_for_height(target_parent) {
                if known_hash != target_hash {
                    return None;
                }
            }
        }
        let candidate_matches_known_chain = |candidate: &crate::sumeragi::consensus::Qc| {
            known_hash_for_height(candidate.height)
                .is_none_or(|known_hash| known_hash == candidate.subject_block_hash)
        };
        let mut exact_candidates = Vec::new();
        let mut fallback_candidates = Vec::new();
        // Require an exact parent-height QC only when the parent chain cannot be
        // reconstructed from pending blocks. When the chain is available, older
        // commit QCs can be safely rolled forward through known parent hashes.
        let strict_parent_qc = target_parent_hash.is_some()
            && target_parent > committed_height
            && pending_hashes.is_none();
        for candidate in super::status::commit_qc_history() {
            if !matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit) {
                continue;
            }
            if let Some(target_hash) = target_parent_hash {
                if candidate.height == target_parent && candidate.subject_block_hash == target_hash
                {
                    exact_candidates.push(candidate);
                    continue;
                }
                if strict_parent_qc {
                    continue;
                }
            }
            if candidate.height <= target_parent && candidate_matches_known_chain(&candidate) {
                fallback_candidates.push(candidate);
            }
        }
        exact_candidates.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.view.cmp(&a.view)));
        fallback_candidates
            .sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.view.cmp(&a.view)));
        let mut candidates = exact_candidates;
        if !strict_parent_qc {
            candidates.extend(fallback_candidates.into_iter().filter(|candidate| {
                target_parent_hash.is_none_or(|target_hash| {
                    candidate.height != target_parent || candidate.subject_block_hash != target_hash
                })
            }));
        }
        let hash_for_height = |h: u64| {
            if h == 0 {
                return None;
            }
            if let Some(hashes) = pending_hashes.as_ref() {
                if let Some(hash) = hashes.get(&h) {
                    return Some(*hash);
                }
            }
            if h <= committed_height {
                return committed_hash_for_height(h);
            }
            if h == target_parent {
                return target_parent_hash;
            }
            None
        };
        'candidates: for cert in candidates {
            if cert.validator_set.is_empty() {
                continue;
            }
            let mut roster = cert.validator_set.clone();
            let mut current_height = cert.height;
            let mut current_hash = cert.subject_block_hash;
            while current_height < height {
                let mut topology = super::network_topology::Topology::new(roster.clone());
                topology.block_committed(roster.clone(), current_hash);
                roster = topology.as_ref().to_vec();
                current_height = current_height.saturating_add(1);
                if current_height == height {
                    let world = self.state.world_view();
                    roster = super::roster::filter_roster_with_live_consensus_keys_at_height_world(
                        &world, roster, height,
                    );
                    if roster.is_empty() {
                        continue 'candidates;
                    }
                    roster = super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
                    return Some(roster);
                }
                let Some(next_hash) = hash_for_height(current_height) else {
                    continue 'candidates;
                };
                current_hash = next_hash;
            }
        }
        None
    }

    /// Roster selection for locally emitted votes (NEW_VIEW / precommit) at the active height.
    pub(super) fn roster_for_live_vote_with_mode(
        &self,
        height: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height > committed_height.saturating_add(1) {
            return Vec::new();
        }
        if let Some(pending) = self.pending_activation_roster_for_height(height, consensus_mode) {
            return pending;
        }
        let world = self.state.world_view();
        let commit_topology = self.state.commit_topology_snapshot();
        self.active_topology_with_genesis_fallback_from_world(
            &world,
            commit_topology.as_slice(),
            committed_height,
            consensus_mode,
        )
    }

    /// Canonical roster derivation for a consensus round.
    ///
    /// This path is deterministic for active/future rounds and depends only on
    /// canonical chain/on-chain state for the target height.
    pub(super) fn canonical_round_roster_with_mode(
        &self,
        height: u64,
        _view: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let canonicalize =
            |roster| super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height >= committed_height.saturating_add(1) {
            if let Some(roster) = self.roster_from_commit_qc_history_roll_forward(height, None) {
                return canonicalize(roster);
            }
            if let Some(roster) = self.roster_from_commit_qc_history(height) {
                return canonicalize(roster);
            }
            if height > committed_height.saturating_add(1) {
                return Vec::new();
            }
        }
        if let Some(pending) = self.pending_activation_roster_for_height(height, consensus_mode) {
            return canonicalize(pending);
        }
        if height == committed_height {
            let prev = self.state.prev_commit_topology_snapshot();
            if !prev.is_empty() {
                return canonicalize(prev);
            }
        }
        let world = self.state.world_view();
        let commit_topology = self.state.commit_topology_snapshot();
        canonicalize(self.active_topology_with_genesis_fallback_from_world(
            &world,
            commit_topology.as_slice(),
            committed_height,
            consensus_mode,
        ))
    }

    // Prefer the cached roster once votes are validated to keep signatures stable across roster changes.
    pub(super) fn roster_for_vote_with_mode(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let canonicalize =
            |roster| super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
        if let Some(cached) = self.vote_roster_cache.get(&block_hash) {
            if !cached.roster.is_empty() {
                return canonicalize(cached.roster.clone());
            }
        }
        let allow_sidecar = !self.sidecar_quarantined_for_height(height);
        if let Some(selection) = super::persisted_roster_for_block(
            self.state.as_ref(),
            &self.kura,
            consensus_mode,
            height,
            block_hash,
            Some(view),
            &self.roster_validation_cache,
            None,
            allow_sidecar,
        )
        .or_else(|| {
            super::block_sync_history_roster_for_block(
                consensus_mode,
                block_hash,
                height,
                Some(view),
                &self.chain_id,
                &self.roster_validation_cache,
            )
        }) {
            if !selection.roster.is_empty() {
                return canonicalize(selection.roster);
            }
        }
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height <= committed_height {
            let mut roster_height = height;
            let mut roster_view = None;
            if let Some(block_height) = self.kura.get_block_height_by_hash(block_hash) {
                roster_height = u64::try_from(block_height.get()).unwrap_or(height);
                if let Some(block) = self.kura.get_block(block_height) {
                    roster_view = Some(block.header().view_change_index());
                }
            }
            if roster_view.is_none() && roster_height == height {
                roster_view = Some(view);
            }
            let allow_sidecar = !self.sidecar_quarantined_for_height(roster_height);
            let roster_selection = super::persisted_roster_for_block(
                self.state.as_ref(),
                &self.kura,
                consensus_mode,
                roster_height,
                block_hash,
                roster_view,
                &self.roster_validation_cache,
                None,
                allow_sidecar,
            )
            .or_else(|| {
                super::block_sync_history_roster_for_block(
                    consensus_mode,
                    block_hash,
                    roster_height,
                    roster_view,
                    &self.chain_id,
                    &self.roster_validation_cache,
                )
            });
            if let Some(selection) = roster_selection {
                if !selection.roster.is_empty() {
                    return canonicalize(selection.roster);
                }
            }
            if height == committed_height {
                let prev = self.state.prev_commit_topology_snapshot();
                if !prev.is_empty() {
                    return canonicalize(prev);
                }
                return self.canonical_round_roster_with_mode(height, view, consensus_mode);
            }
            return Vec::new();
        }
        if let Some(parent_hash) =
            self.pending
                .pending_blocks
                .get(&block_hash)
                .and_then(|pending| {
                    (pending.height == height)
                        .then(|| pending.block.header().prev_block_hash())
                        .flatten()
                })
        {
            if let Some(roster) =
                self.roster_from_commit_qc_history_roll_forward(height, Some(parent_hash))
            {
                return canonicalize(roster);
            }
            if height > committed_height.saturating_add(1) {
                return Vec::new();
            }
        }

        self.canonical_round_roster_with_mode(height, view, consensus_mode)
    }

    pub(super) fn roster_for_vote_with_mode_observing_sidecar(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
        reason: &'static str,
    ) -> Vec<PeerId> {
        self.observe_sidecar_mismatch_for_height(height, block_hash, reason);
        self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode)
    }

    fn pending_activation_roster_for_height(
        &self,
        height: u64,
        consensus_mode: ConsensusMode,
    ) -> Option<Vec<PeerId>> {
        let (activate_at, roster) = self.pending_roster_activation.as_ref()?;
        if height < *activate_at {
            return None;
        }
        let filtered = {
            let world = self.state.world_view();
            super::roster::filter_roster_with_live_consensus_keys_at_height_world(
                &world,
                roster.clone(),
                height,
            )
        };
        let canonical = super::roster::canonicalize_roster_for_mode(filtered, consensus_mode);
        (!canonical.is_empty()).then_some(canonical)
    }

    pub(super) fn roster_for_new_view_with_mode(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let committed_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if height <= committed_height {
            return self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        }
        if height == committed_height.saturating_add(1) {
            return self.roster_for_live_vote_with_mode(height, consensus_mode);
        }
        if let Some(roster) =
            self.roster_from_commit_qc_history_roll_forward(height, Some(block_hash))
        {
            return super::roster::canonicalize_roster_for_mode(roster, consensus_mode);
        }
        let roster = self.canonical_round_roster_with_mode(height, view, consensus_mode);
        if !roster.is_empty() {
            return roster;
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, net::SocketAddr, num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock},
        peer::{Peer, PeerId},
        transaction::SignedTransaction,
    };
    use iroha_primitives::unique_vec::UniqueVec;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn sample_vote(block_hash: HashOf<BlockHeader>) -> crate::sumeragi::consensus::Vote {
        crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height: 3,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        }
    }

    fn sample_block(height: u64) -> SignedBlock {
        let header = BlockHeader {
            height: NonZeroU64::new(height).expect("non-zero height"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            creation_time_ms: 0,
            view_change_index: 0,
            confidential_features: None,
        };
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let signature = SignatureOf::from_hash(key_pair.private_key(), header.hash());
        let block_signature = BlockSignature::new(0, signature);
        SignedBlock::presigned(block_signature, header, Vec::<SignedTransaction>::new())
    }

    fn trusted_self() -> (iroha_config::parameters::actual::TrustedPeers, PeerId) {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(key_pair.public_key().clone());
        let address: SocketAddr = "127.0.0.1:7015".parse().expect("socket address parses");
        let peer = Peer::new(address.into(), peer_id.clone());
        let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key()).expect("pop proves");
        let mut pops = BTreeMap::new();
        pops.insert(peer_id.public_key().clone(), pop);
        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: peer,
            others: UniqueVec::new(),
            pops,
        };
        (trusted, peer_id)
    }

    #[test]
    fn vote_duplicate_matches_same_block() {
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
        let vote = sample_vote(hash);
        let mut vote_log = BTreeMap::new();
        vote_log.insert(vote_key(&vote), vote.clone());

        assert!(vote_duplicate(&vote_log, &vote));
    }

    #[test]
    fn vote_duplicate_rejects_different_block() {
        let hash_a = HashOf::from_untyped_unchecked(Hash::prehashed([0xBB; Hash::LENGTH]));
        let hash_b = HashOf::from_untyped_unchecked(Hash::prehashed([0xCC; Hash::LENGTH]));
        let vote = sample_vote(hash_a);
        let mut vote_log = BTreeMap::new();
        vote_log.insert(vote_key(&vote), vote.clone());

        let other = sample_vote(hash_b);

        assert!(!vote_duplicate(&vote_log, &other));
    }

    #[test]
    fn commit_vote_block_sync_update_includes_cached_vote() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let block = sample_block(2);
        let block_hash = block.hash();

        let mut vote_log = BTreeMap::new();
        let vote = sample_vote(block_hash);
        vote_log.insert(vote_key(&vote), vote.clone());
        let qc_cache: BTreeMap<
            (
                crate::sumeragi::consensus::Phase,
                HashOf<BlockHeader>,
                u64,
                u64,
                u64,
            ),
            crate::sumeragi::consensus::Qc,
        > = BTreeMap::new();

        let (trusted, me_id) = trusted_self();
        let roster_cache = {
            let view = state.view();
            super::RosterValidationCache::from_world(view.world(), super::EPOCH_LENGTH_BLOCKS, None)
        };
        let mut update = super::block_sync_update_with_roster(
            &block,
            &state,
            kura.as_ref(),
            ConsensusMode::Permissioned,
            &trusted,
            &me_id,
            &roster_cache,
        );
        Actor::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &qc_cache,
            &vote_log,
            vote.block_hash,
            vote.height,
            vote.view,
            vote.epoch,
            &state,
            ConsensusMode::Permissioned,
        );

        assert_eq!(update.commit_votes.len(), 1);
        assert_eq!(update.commit_votes[0].block_hash, block_hash);
        assert!(update.commit_qc.is_none());
    }
}
