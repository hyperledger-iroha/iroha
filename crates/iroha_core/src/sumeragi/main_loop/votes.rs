//! Vote handling for consensus messages.

use iroha_logger::prelude::*;

use crate::sumeragi::consensus::Phase;

use super::*;

type VoteLogKey = (
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

impl Actor {
    pub(super) fn consensus_context_for_height(
        &self,
        height: u64,
    ) -> (ConsensusMode, &'static str, Option<[u8; 32]>) {
        let view = self.state.view();
        let consensus_mode =
            super::effective_consensus_mode_for_height(&view, height, self.config.consensus_mode);
        let mode_tag = match consensus_mode {
            ConsensusMode::Permissioned => super::consensus::PERMISSIONED_TAG,
            ConsensusMode::Npos => super::consensus::NPOS_TAG,
        };
        let prf_seed = if matches!(consensus_mode, ConsensusMode::Npos) {
            Some(super::npos_seed_for_height(&view, height))
        } else {
            None
        };
        (consensus_mode, mode_tag, prf_seed)
    }

    pub(super) fn handle_vote(&mut self, vote: crate::sumeragi::consensus::Vote) {
        let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if self.drop_vote_for_height_or_view(&vote, committed_height)
            || self.drop_precommit_vote_for_lock(&vote)
        {
            return;
        }
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(vote.height);
        let topology_peers = if matches!(vote.phase, Phase::NewView) {
            self.roster_for_new_view_with_mode(
                vote.block_hash,
                vote.height,
                vote.view,
                consensus_mode,
            )
        } else {
            self.roster_for_vote_with_mode(vote.block_hash, vote.height, vote.view, consensus_mode)
        };
        if topology_peers.is_empty() {
            warn!(
                phase = ?vote.phase,
                height = vote.height,
                view = vote.view,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping vote: empty commit topology"
            );
            return;
        }
        let topology = super::network_topology::Topology::new(topology_peers.clone());
        let chain_id = self.common_config.chain.clone();
        let evidence_context = super::evidence::EvidenceValidationContext {
            topology: &topology,
            chain_id: &chain_id,
            mode_tag,
            prf_seed,
        };
        let signature_topology =
            topology_for_view(&topology, vote.height, vote.view, mode_tag, prf_seed);
        if !self.validate_and_record_vote(&vote, &signature_topology, &evidence_context, mode_tag) {
            return;
        }
        match vote.phase {
            Phase::Prepare | Phase::Commit => {
                self.try_form_qc_from_votes(
                    vote.phase,
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    vote.epoch,
                    topology,
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
                        let block_time = {
                            let state_view = self.state.view();
                            state_view.world.parameters().sumeragi().block_time()
                        };
                        let cooldown = block_time.max(REBROADCAST_COOLDOWN_FLOOR);
                        if self.block_sync_rebroadcast_log.allow(
                            vote.block_hash,
                            std::time::Instant::now(),
                            cooldown,
                        ) {
                            let update = self.block_sync_update_for_precommit_vote(
                                &pending.block,
                                self.state.as_ref(),
                                self.kura.as_ref(),
                                &self.qc_cache,
                                &self.vote_log,
                                &vote,
                            );
                            if super::block_sync_update_has_roster(&update, consensus_mode) {
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
                                self.broadcast_block_created_for_block_sync(
                                    super::message::BlockCreated::from(&pending.block),
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
            }
            Phase::NewView => {
                if let Some(highest) = vote.highest_qc {
                    if highest.phase != Phase::Commit {
                        debug!(
                            height = vote.height,
                            view = vote.view,
                            signer = vote.signer,
                            highest_height = highest.height,
                            highest_view = highest.view,
                            phase = ?highest.phase,
                            "ignoring NEW_VIEW highest certificate with non-commit phase"
                        );
                    } else {
                        let count = self.subsystems.propose.new_view_tracker.record(
                            vote.height,
                            vote.view,
                            vote.signer,
                            highest,
                        );
                        crate::sumeragi::new_view_stats::note_receipt(
                            vote.height,
                            vote.view,
                            vote.signer,
                        );
                        debug!(
                            height = vote.height,
                            view = vote.view,
                            signer = vote.signer,
                            count,
                            "recorded NEW_VIEW vote"
                        );
                    }
                } else {
                    debug!(
                        height = vote.height,
                        view = vote.view,
                        signer = vote.signer,
                        "NEW_VIEW vote missing highest certificate reference"
                    );
                }
                self.try_form_qc_from_votes(
                    vote.phase,
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    vote.epoch,
                    topology,
                );
            }
        }
    }

    fn drop_vote_for_height_or_view(
        &self,
        vote: &crate::sumeragi::consensus::Vote,
        committed_height: u64,
    ) -> bool {
        if vote.height <= committed_height {
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
            return true;
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
            return true;
        }
        if let Some(local_view) = self.stale_view(vote.height, vote.view) {
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
            } else {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    kind = "Vote",
                    "dropping consensus message for stale view"
                );
                return true;
            }
        }
        false
    }

    fn drop_precommit_vote_for_lock(&self, vote: &crate::sumeragi::consensus::Vote) -> bool {
        if !matches!(vote.phase, crate::sumeragi::consensus::Phase::Commit) {
            return false;
        }
        let Some(lock) = self.locked_qc else {
            return false;
        };
        if !self.block_known_locally(lock.subject_block_hash) {
            return false;
        }
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
            return true;
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
            let extends_locked =
                super::qc_extends_locked_with_lookup(lock, candidate, |hash, height| {
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
                return true;
            }
        }
        false
    }

    fn validate_and_record_vote(
        &mut self,
        vote: &crate::sumeragi::consensus::Vote,
        signature_topology: &super::network_topology::Topology,
        evidence_context: &super::evidence::EvidenceValidationContext<'_>,
        mode_tag: &str,
    ) -> bool {
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
            return false;
        }
        match vote_signature_check(
            vote,
            signature_topology,
            &self.common_config.chain,
            mode_tag,
        ) {
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
                        roster_len,
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
                        roster_len,
                        ?err,
                        "suppressing repeated invalid vote signature log"
                    );
                }
                return false;
            }
        }
        let key = vote_key(vote);
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
            return false;
        }
        let previous = self.vote_log.insert(key, vote.clone());
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

    fn roster_from_commit_qc_history(&self, height: u64) -> Option<Vec<PeerId>> {
        let parent_height = height.checked_sub(1)?;
        let cert = super::status::commit_qc_history()
            .into_iter()
            .find(|candidate| {
                candidate.height == parent_height
                    && matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit)
            })?;
        if cert.validator_set.is_empty() {
            return None;
        }
        let mut topology = super::network_topology::Topology::new(cert.validator_set.clone());
        topology.block_committed(cert.validator_set.clone(), cert.subject_block_hash);
        let roster = topology.as_ref().to_vec();
        if roster.is_empty() {
            None
        } else {
            Some(roster)
        }
    }

    pub(super) fn roster_from_commit_qc_history_roll_forward(
        &self,
        height: u64,
        target_parent_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<Vec<PeerId>> {
        let target_parent = height.checked_sub(1)?;
        let cert = target_parent_hash.and_then(|target_hash| {
            super::status::commit_qc_history()
                .into_iter()
                .find(|candidate| {
                    candidate.height == target_parent
                        && candidate.subject_block_hash == target_hash
                        && matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit)
                })
        });
        let cert = cert.or_else(|| {
            super::status::commit_qc_history()
                .into_iter()
                .find(|candidate| {
                    candidate.height <= target_parent
                        && matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit)
                })
        })?;
        if cert.validator_set.is_empty() {
            return None;
        }

        let view = self.state.view();
        let committed_height = u64::try_from(view.height()).unwrap_or(0);
        let hashes = view.block_hashes();
        let pending_hashes = target_parent_hash.and_then(|hash| {
            self.pending_chain_hashes_for_parent(hash, target_parent, committed_height)
        });
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
                let idx = usize::try_from(h.saturating_sub(1)).ok()?;
                return hashes.get(idx).copied();
            }
            if h == target_parent {
                return target_parent_hash;
            }
            None
        };

        let mut roster = cert.validator_set.clone();
        let mut current_height = cert.height;
        let mut current_hash = cert.subject_block_hash;
        while current_height < height {
            let mut topology = super::network_topology::Topology::new(roster.clone());
            topology.block_committed(roster.clone(), current_hash);
            roster = topology.as_ref().to_vec();
            current_height = current_height.saturating_add(1);
            if current_height == height {
                return (!roster.is_empty()).then_some(roster);
            }
            current_hash = hash_for_height(current_height)?;
        }
        None
    }

    // Prefer the roster tied to a committed block to keep signatures valid across roster changes.
    pub(super) fn roster_for_vote_with_mode(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        let mut roster_height = height;
        let mut roster_view = None;
        if let Some(block_height) = self.kura.get_block_height_by_hash(block_hash) {
            roster_height = u64::try_from(block_height.get()).unwrap_or(height);
            if let Some(block) = self.kura.get_block(block_height) {
                roster_view = Some(u64::from(block.header().view_change_index()));
            }
        }
        if roster_view.is_none() && roster_height == height {
            roster_view = Some(view);
        }
        let roster_selection = || {
            super::persisted_roster_for_block(
                self.state.as_ref(),
                &self.kura,
                consensus_mode,
                roster_height,
                block_hash,
                roster_view,
            )
            .or_else(|| {
                super::block_sync_history_roster_for_block(
                    self.state.as_ref(),
                    consensus_mode,
                    block_hash,
                    roster_height,
                    roster_view,
                )
            })
        };
        if height <= committed_height {
            if let Some(selection) = roster_selection() {
                if !selection.roster.is_empty() {
                    return selection.roster;
                }
            }
            if height == committed_height {
                let view = self.state.view();
                let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
                if !prev.is_empty() {
                    return prev;
                }
            }
        }
        let active = self.effective_commit_topology();
        if height <= committed_height.saturating_add(1) && !active.is_empty() {
            if let Some(selection) = roster_selection() {
                if !selection.roster.is_empty() {
                    return selection.roster;
                }
            }
            return active;
        }
        if let Some(selection) = roster_selection() {
            if !selection.roster.is_empty() {
                return selection.roster;
            }
        }
        if height > committed_height.saturating_add(1) {
            let parent_hash = self
                .pending
                .pending_blocks
                .get(&block_hash)
                .and_then(|pending| pending.block.header().prev_block_hash());
            if let Some(roster) =
                self.roster_from_commit_qc_history_roll_forward(height, parent_hash)
            {
                return roster;
            }
            if let Some(roster) = self.roster_from_commit_qc_history(height) {
                return roster;
            }
            return Vec::new();
        }
        active
    }

    pub(super) fn roster_for_new_view_with_mode(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        consensus_mode: ConsensusMode,
    ) -> Vec<PeerId> {
        let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if height <= committed_height {
            return self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        }
        if let Some(roster) =
            self.roster_from_commit_qc_history_roll_forward(height, Some(block_hash))
        {
            return roster;
        }
        let active = self.effective_commit_topology();
        if height > committed_height.saturating_add(1) {
            return Vec::new();
        }
        if !active.is_empty() {
            return active;
        }
        self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode)
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
        let mut update = super::block_sync_update_with_roster(
            &block,
            &state,
            kura.as_ref(),
            ConsensusMode::Permissioned,
            &trusted,
            &me_id,
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
