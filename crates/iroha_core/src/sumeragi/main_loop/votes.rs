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
        let topology_peers = self.roster_for_vote_with_mode(
            vote.block_hash,
            vote.height,
            vote.view,
            consensus_mode,
        );
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
        let signature_topology = topology_for_view(&topology, vote.height, vote.view, mode_tag, prf_seed);
        if !self.validate_and_record_vote(&vote, &signature_topology, &evidence_context, mode_tag)
        {
            return;
        }
        if matches!(vote.phase, Phase::Prevote | Phase::Precommit) {
            self.try_form_qc_from_votes(
                vote.phase,
                vote.block_hash,
                vote.height,
                vote.view,
                vote.epoch,
                topology,
            );
            if matches!(vote.phase, Phase::Precommit) {
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
                    if self
                        .block_sync_rebroadcast_log
                        .allow(vote.block_hash, std::time::Instant::now(), cooldown)
                    {
                        let update = self.block_sync_update_for_precommit_vote(
                            &pending.block,
                            self.state.as_ref(),
                            self.kura.as_ref(),
                            &self.qc_cache,
                            &self.vote_log,
                            &vote,
                        );
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
        if let Some(local_view) = self.stale_view(vote.height, vote.view) {
            let da_enabled = self.runtime_da_enabled();
            let missing_request = self
                .pending
                .missing_block_requests
                .contains_key(&vote.block_hash);
            if vote.phase == crate::sumeragi::consensus::Phase::Precommit
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
        if !matches!(vote.phase, crate::sumeragi::consensus::Phase::Precommit) {
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
                phase: crate::sumeragi::consensus::Phase::Precommit,
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
            crate::sumeragi::consensus::Phase::Prevote => {
                Some(crate::sumeragi::consensus::Phase::Precommit)
            }
            crate::sumeragi::consensus::Phase::Precommit => {
                Some(crate::sumeragi::consensus::Phase::Prevote)
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
        );
        update
    }

    // Prefer the roster tied to a committed block to keep signatures valid across roster changes.
    pub(super) fn roster_for_vote(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Vec<PeerId> {
        self.roster_for_vote_with_mode(block_hash, height, view, self.consensus_mode)
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
        let block_view = self
            .kura
            .get_block_height_by_hash(block_hash)
            .and_then(|height| self.kura.get_block(height))
            .map_or(view, |block| u64::from(block.header().view_change_index()));
        if height <= committed_height {
            if let Some(selection) = super::persisted_roster_for_block(
                self.state.as_ref(),
                &self.kura,
                consensus_mode,
                height,
                block_hash,
                Some(block_view),
            )
            .or_else(|| {
                super::block_sync_history_roster_for_block(
                    self.state.as_ref(),
                    consensus_mode,
                    block_hash,
                    height,
                    Some(block_view),
                )
            }) {
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
            return active;
        }
        if let Some(selection) = super::persisted_roster_for_block(
            self.state.as_ref(),
            &self.kura,
            consensus_mode,
            height,
            block_hash,
            Some(block_view),
        )
        .or_else(|| {
            super::block_sync_history_roster_for_block(
                self.state.as_ref(),
                consensus_mode,
                block_hash,
                height,
                Some(block_view),
            )
        }) {
            if !selection.roster.is_empty() {
                return selection.roster;
            }
        }
        active
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_available_vote(
        &mut self,
        vote: crate::sumeragi::consensus::AvailableVote,
    ) {
        let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if vote.height <= committed_height {
            iroha_logger::debug!(
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block = %vote.block_hash,
                committed_height,
                "dropping stale availability vote below committed height"
            );
            return;
        }
        if let Some(local_view) = self.stale_view(vote.height, vote.view) {
            let da_enabled = self.runtime_da_enabled();
            if self.block_known_locally(vote.block_hash) || da_enabled {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    signer = vote.signer,
                    block = %vote.block_hash,
                    da_enabled,
                    "accepting availability vote for stale view"
                );
            } else {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    kind = "AvailableVote",
                    "dropping consensus message for stale view"
                );
                return;
            }
        }
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(vote.height);
        let topology_peers = self.roster_for_vote_with_mode(
            vote.block_hash,
            vote.height,
            vote.view,
            consensus_mode,
        );
        if topology_peers.is_empty() {
            warn!(
                height = vote.height,
                view = vote.view,
                signer = vote.signer,
                block = %vote.block_hash,
                "dropping availability vote: empty commit topology"
            );
            return;
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let signature_topology =
            topology_for_view(&topology, vote.height, vote.view, mode_tag, prf_seed);
        let local_idx = self.local_validator_index_for_topology(&signature_topology);
        let as_vote = crate::sumeragi::consensus::Vote {
            phase: crate::sumeragi::consensus::Phase::Available,
            block_hash: vote.block_hash,
            height: vote.height,
            view: vote.view,
            epoch: vote.epoch,
            signer: vote.signer,
            bls_sig: vote.bls_sig.clone(),
            signature: vote.signature.clone(),
        };
        match vote_signature_check(
            &as_vote,
            &signature_topology,
            &self.common_config.chain,
            mode_tag,
        ) {
            Ok(()) => {}
            Err(err) => {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    block = %vote.block_hash,
                    roster_len = signature_topology.as_ref().len(),
                    ?err,
                    "dropping availability vote with invalid signature"
                );
                return;
            }
        }
        let key = vote_key(&as_vote);
        if vote_duplicate(&self.vote_log, &as_vote) {
            iroha_logger::debug!(
                height = vote.height,
                view = vote.view,
                epoch = vote.epoch,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping duplicate availability vote already recorded"
            );
            return;
        }
        let duplicate = self.vote_log.insert(key, as_vote).is_some();
        iroha_logger::info!(
            height = vote.height,
            view = vote.view,
            epoch = vote.epoch,
            signer = vote.signer,
            block_hash = ?vote.block_hash,
            duplicate,
            "recorded availability vote"
        );

        if local_idx != Some(vote.signer) {
            let gossip_targets =
                self.new_view_gossip_targets(signature_topology.as_ref(), Some(vote.signer));
            if !gossip_targets.is_empty() {
                let msg = BlockMessage::AvailabilityVote(vote.clone());
                for peer in gossip_targets {
                    self.schedule_background(BackgroundRequest::Post {
                        peer,
                        msg: msg.clone(),
                    });
                }
            }
        }

        self.try_form_qc_from_votes(
            crate::sumeragi::consensus::Phase::Available,
            vote.block_hash,
            vote.height,
            vote.view,
            vote.epoch,
            topology,
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_exec_vote(&mut self, vote: crate::sumeragi::consensus::ExecVote) {
        if let Some(local_view) = self.stale_view(vote.height, vote.view) {
            let missing_request = self
                .pending
                .missing_block_requests
                .contains_key(&vote.block_hash);
            if self.block_known_locally(vote.block_hash) || missing_request {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    signer = vote.signer,
                    block = %vote.block_hash,
                    missing_request,
                    "accepting exec vote for stale view"
                );
            } else {
                debug!(
                    height = vote.height,
                    view = vote.view,
                    local_view,
                    kind = "ExecVote",
                    "dropping consensus message for stale view"
                );
                return;
            }
        }
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(vote.height);
        let topology_peers = self.roster_for_vote_with_mode(
            vote.block_hash,
            vote.height,
            vote.view,
            consensus_mode,
        );
        if topology_peers.is_empty() {
            warn!(
                height = vote.height,
                view = vote.view,
                signer = vote.signer,
                block_hash = %vote.block_hash,
                "dropping exec vote: empty commit topology"
            );
            return;
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let evidence_context = super::evidence::EvidenceValidationContext {
            topology: &topology,
            chain_id: &self.common_config.chain,
            mode_tag,
            prf_seed,
        };
        let signature_topology =
            topology_for_view(&topology, vote.height, vote.view, mode_tag, prf_seed);
        match super::exec_vote_signature_check(
            &vote,
            &signature_topology,
            &self.common_config.chain,
            mode_tag,
        ) {
            Ok(()) => {}
            Err(err) => {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    roster_len = signature_topology.as_ref().len(),
                    ?err,
                    "dropping exec vote with invalid signature"
                );
                return;
            }
        }
        let key = (vote.height, vote.view, vote.epoch, vote.signer);
        if let Some(existing) = self.exec_vote_log.get(&key) {
            if existing.block_hash == vote.block_hash
                && existing.parent_state_root == vote.parent_state_root
                && existing.post_state_root == vote.post_state_root
            {
                iroha_logger::debug!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = ?vote.block_hash,
                    "dropping duplicate exec vote already recorded"
                );
                return;
            }
            if existing.block_hash == vote.block_hash
                && super::evidence::record_double_exec_vote(
                    &mut self.evidence_store,
                    self.state.as_ref(),
                    existing,
                    &vote,
                    &evidence_context,
                )
            {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    "double execution vote detected; storing evidence"
                );
            } else {
                warn!(
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = ?vote.block_hash,
                    existing_block = ?existing.block_hash,
                    "dropping conflicting exec vote for signer"
                );
            }
            return;
        }
        let duplicate = self.exec_vote_log.insert(key, vote.clone()).is_some();
        iroha_logger::debug!(
            height = vote.height,
            view = vote.view,
            epoch = vote.epoch,
            signer = vote.signer,
            block_hash = ?vote.block_hash,
            duplicate,
            "recorded exec vote"
        );
        self.try_form_exec_qc_from_votes(
            vote.block_hash,
            vote.height,
            vote.view,
            vote.epoch,
            topology,
        );
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
            phase: crate::sumeragi::consensus::Phase::Precommit,
            block_hash,
            height: 3,
            view: 0,
            epoch: 0,
            signer: 0,
            bls_sig: Vec::new(),
            signature: Vec::new(),
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
    fn precommit_vote_block_sync_update_includes_cached_vote() {
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
        );

        assert_eq!(update.precommit_votes.len(), 1);
        assert_eq!(update.precommit_votes[0].block_hash, block_hash);
        assert!(update.qc.is_none());
        assert!(update.availability_qc.is_none());
    }
}
