//! Proposal-cache and proposal/header mismatch helpers.

use std::{collections::BTreeMap, time::Instant};

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::block::{BlockHeader, BlockPayload, SignedBlock};
use iroha_data_model::transaction::signed::TransactionEntrypoint;
use norito::codec::Encode as _;

use crate::sumeragi::status;
use crate::sumeragi::{consensus::Proposal, message::ProposalHint};

pub(super) struct ProposalCache {
    pub(super) hints: BTreeMap<(u64, u64), ProposalHint>,
    pub(super) proposals: BTreeMap<(u64, u64), Proposal>,
    pub(super) observed_at: BTreeMap<(u64, u64), Instant>,
    limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProposalCacheShape {
    pub(super) hint_count: usize,
    pub(super) proposal_count: usize,
    pub(super) observed_count: usize,
    pub(super) hint_limit_enforced: bool,
    pub(super) proposal_limit_enforced: bool,
    pub(super) observed_only_for_live_entries: bool,
    pub(super) live_entries_have_observed: bool,
}

#[inline]
pub(super) fn evidence_within_horizon(
    current_height: u64,
    horizon: u64,
    subject_height: Option<u64>,
) -> bool {
    if horizon == 0 {
        return true;
    }
    let reference = subject_height.unwrap_or(current_height);
    let lower_bound = current_height.saturating_sub(horizon);
    reference >= lower_bound
}

impl ProposalCache {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            hints: BTreeMap::new(),
            proposals: BTreeMap::new(),
            observed_at: BTreeMap::new(),
            limit,
        }
    }

    pub(super) fn insert_hint(&mut self, hint: ProposalHint) {
        let key = (hint.height, hint.view);
        self.observed_at.entry(key).or_insert_with(Instant::now);
        self.hints.insert(key, hint);
        self.evict_if_needed();
        self.debug_assert_shape();
    }

    pub(super) fn get_hint(&self, height: u64, view: u64) -> Option<&ProposalHint> {
        self.hints.get(&(height, view))
    }

    pub(super) fn get_proposal(&self, height: u64, view: u64) -> Option<&Proposal> {
        self.proposals.get(&(height, view))
    }

    pub(super) fn observed_at(&self, height: u64, view: u64) -> Option<Instant> {
        self.observed_at.get(&(height, view)).copied()
    }

    pub(super) fn pop_hint(&mut self, height: u64, view: u64) -> Option<ProposalHint> {
        let key = (height, view);
        let removed = self.hints.remove(&key);
        self.remove_observed_if_empty(key);
        self.debug_assert_shape();
        removed
    }

    pub(super) fn insert_proposal(&mut self, proposal: Proposal) {
        let key = (proposal.header.height, proposal.header.view);
        self.observed_at.entry(key).or_insert_with(Instant::now);
        self.proposals.insert(key, proposal);
        self.evict_if_needed();
        self.debug_assert_shape();
    }

    pub(super) fn pop_proposal(&mut self, height: u64, view: u64) -> Option<Proposal> {
        let key = (height, view);
        let removed = self.proposals.remove(&key);
        self.remove_observed_if_empty(key);
        self.debug_assert_shape();
        removed
    }

    fn remove_observed_if_empty(&mut self, key: (u64, u64)) {
        if !self.hints.contains_key(&key) && !self.proposals.contains_key(&key) {
            self.observed_at.remove(&key);
        }
    }

    fn evict_if_needed(&mut self) {
        let mut evicted = 0u64;
        while self.hints.len() > self.limit {
            if let Some(first_key) = self.hints.keys().next().copied() {
                self.hints.remove(&first_key);
                evicted = evicted.saturating_add(1);
            } else {
                break;
            }
        }
        while self.proposals.len() > self.limit {
            if let Some(first_key) = self.proposals.keys().next().copied() {
                self.proposals.remove(&first_key);
                evicted = evicted.saturating_add(1);
            } else {
                break;
            }
        }
        self.observed_at
            .retain(|key, _| self.hints.contains_key(key) || self.proposals.contains_key(key));
        if evicted > 0 {
            status::inc_pending_queue_evictions_total(evicted);
        }
        self.debug_assert_shape();
    }

    pub(super) fn prune_height_leq(&mut self, height: u64) {
        self.hints.retain(|(h, _), _| *h > height);
        self.proposals.retain(|(h, _), _| *h > height);
        self.observed_at.retain(|(h, _), _| *h > height);
        self.debug_assert_shape();
    }

    pub(super) fn shape(&self) -> ProposalCacheShape {
        ProposalCacheShape {
            hint_count: self.hints.len(),
            proposal_count: self.proposals.len(),
            observed_count: self.observed_at.len(),
            hint_limit_enforced: self.hints.len() <= self.limit,
            proposal_limit_enforced: self.proposals.len() <= self.limit,
            observed_only_for_live_entries: self
                .observed_at
                .keys()
                .all(|key| self.hints.contains_key(key) || self.proposals.contains_key(key)),
            live_entries_have_observed: self
                .hints
                .keys()
                .chain(self.proposals.keys())
                .all(|key| self.observed_at.contains_key(key)),
        }
    }

    fn debug_assert_shape(&self) {
        let shape = self.shape();
        debug_assert!(shape.hint_limit_enforced);
        debug_assert!(shape.proposal_limit_enforced);
        debug_assert!(shape.observed_only_for_live_entries);
        debug_assert!(shape.live_entries_have_observed);
    }

    #[cfg(test)]
    pub(super) fn hint_count(&self) -> usize {
        self.hints.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProposalMismatch {
    Height {
        proposal: u64,
        block: u64,
    },
    View {
        proposal: u64,
        block: u64,
    },
    Parent {
        expected: HashOf<BlockHeader>,
        observed: HashOf<BlockHeader>,
    },
    TxRoot {
        expected: Hash,
        observed: Hash,
    },
    StateRoot {
        expected: Hash,
        observed: Hash,
    },
    PayloadHash {
        expected: Hash,
        observed: Hash,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProposalMismatchKind {
    None,
    Height,
    View,
    Parent,
    TxRoot,
    StateRoot,
    PayloadHash,
}

impl ProposalMismatch {
    pub(super) const fn kind(&self) -> ProposalMismatchKind {
        match self {
            ProposalMismatch::Height { .. } => ProposalMismatchKind::Height,
            ProposalMismatch::View { .. } => ProposalMismatchKind::View,
            ProposalMismatch::Parent { .. } => ProposalMismatchKind::Parent,
            ProposalMismatch::TxRoot { .. } => ProposalMismatchKind::TxRoot,
            ProposalMismatch::StateRoot { .. } => ProposalMismatchKind::StateRoot,
            ProposalMismatch::PayloadHash { .. } => ProposalMismatchKind::PayloadHash,
        }
    }

    pub(super) fn reason(&self) -> String {
        match self {
            ProposalMismatch::Height { proposal, block } => {
                format!("proposal height {proposal} disagrees with block height {block}")
            }
            ProposalMismatch::View { proposal, block } => {
                format!("proposal view {proposal} disagrees with block view {block}")
            }
            ProposalMismatch::Parent { expected, observed } => format!(
                "proposal parent hash {observed:?} disagrees with block parent {expected:?}"
            ),
            ProposalMismatch::TxRoot { expected, observed } => {
                format!("proposal tx_root {observed:?} disagrees with block tx_root {expected:?}")
            }
            ProposalMismatch::StateRoot { expected, observed } => format!(
                "proposal state_root {observed:?} disagrees with block state_root {expected:?}"
            ),
            ProposalMismatch::PayloadHash { expected, observed } => format!(
                "proposal payload hash {observed:?} disagrees with recomputed hash {expected:?}"
            ),
        }
    }
}

pub(super) fn proposal_mismatch_kind(
    proposal: &Proposal,
    header: &BlockHeader,
    payload_hash: &Hash,
) -> ProposalMismatchKind {
    let block_height = header.height().get();
    let block_view = header.view_change_index();
    if proposal.header.height != block_height {
        return ProposalMismatchKind::Height;
    }
    if proposal.header.view != block_view {
        return ProposalMismatchKind::View;
    }
    let expected_parent = parent_hash_from_header(header);
    if proposal.header.parent_hash != expected_parent {
        return ProposalMismatchKind::Parent;
    }
    let expected_tx_root = tx_root_from_header(header);
    if proposal.header.tx_root != expected_tx_root {
        return ProposalMismatchKind::TxRoot;
    }
    let expected_state_root = state_root_from_header(header);
    if proposal.header.state_root != expected_state_root {
        let zero_hash = Hash::prehashed([0; Hash::LENGTH]);
        if proposal.header.state_root != zero_hash {
            return ProposalMismatchKind::StateRoot;
        }
    }
    if &proposal.payload_hash != payload_hash {
        return ProposalMismatchKind::PayloadHash;
    }
    ProposalMismatchKind::None
}

pub(super) fn detect_proposal_mismatch(
    proposal: &Proposal,
    header: &BlockHeader,
    payload_hash: &Hash,
) -> Option<ProposalMismatch> {
    let block_height = header.height().get();
    let block_view = header.view_change_index();
    let kind = proposal_mismatch_kind(proposal, header, payload_hash);
    let mismatch = match kind {
        ProposalMismatchKind::None => None,
        ProposalMismatchKind::Height => Some(ProposalMismatch::Height {
            proposal: proposal.header.height,
            block: block_height,
        }),
        ProposalMismatchKind::View => Some(ProposalMismatch::View {
            proposal: proposal.header.view,
            block: block_view,
        }),
        ProposalMismatchKind::Parent => {
            let expected_parent = parent_hash_from_header(header);
            Some(ProposalMismatch::Parent {
                expected: expected_parent,
                observed: proposal.header.parent_hash,
            })
        }
        ProposalMismatchKind::TxRoot => {
            let expected_tx_root = tx_root_from_header(header);
            Some(ProposalMismatch::TxRoot {
                expected: expected_tx_root,
                observed: proposal.header.tx_root,
            })
        }
        ProposalMismatchKind::StateRoot => {
            let expected_state_root = state_root_from_header(header);
            Some(ProposalMismatch::StateRoot {
                expected: expected_state_root,
                observed: proposal.header.state_root,
            })
        }
        ProposalMismatchKind::PayloadHash => Some(ProposalMismatch::PayloadHash {
            expected: *payload_hash,
            observed: proposal.payload_hash,
        }),
    };
    debug_assert_eq!(
        mismatch
            .as_ref()
            .map_or(ProposalMismatchKind::None, ProposalMismatch::kind),
        kind
    );
    mismatch
}

/// Canonicalize block payload encoding before hashing to avoid layout drift from Norito’s adaptive
/// encode heuristics. Strip execution results and signatures so the payload hash stays stable across
/// validation and signature collection.
pub(super) fn block_payload_bytes(block: &SignedBlock) -> Vec<u8> {
    let mut header = block.header();
    header.result_merkle_root = None;
    let external_entrypoints: Vec<_> = block.external_entrypoints_cloned().collect();
    let entry_merkle: MerkleTree<TransactionEntrypoint> = external_entrypoints
        .iter()
        .map(|entrypoint| entrypoint.hash())
        .collect();
    header.merkle_root = entry_merkle.root();
    BlockPayload {
        header,
        transactions: block.transactions_vec().clone(),
        external_entrypoints,
        execution_context: block.execution_context().cloned(),
        da_commitments: block.da_commitments().cloned(),
        da_proof_policies: block.da_proof_policies().cloned(),
        da_pin_intents: block.da_pin_intents().cloned(),
        previous_roster_evidence: block.previous_roster_evidence().cloned(),
        npos_consensus_effects: block.npos_consensus_effects().cloned(),
    }
    .encode()
}

fn parent_hash_from_header(header: &BlockHeader) -> HashOf<BlockHeader> {
    header.prev_block_hash().unwrap_or_else(|| {
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
    })
}

fn tx_root_from_header(header: &BlockHeader) -> Hash {
    header
        .merkle_root()
        .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from)
}

fn state_root_from_header(header: &BlockHeader) -> Hash {
    header
        .result_merkle_root()
        .map_or_else(|| Hash::prehashed([0; Hash::LENGTH]), Hash::from)
}
