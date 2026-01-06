//! Proposal-cache and proposal/header mismatch helpers.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{BlockHeader, BlockPayload, SignedBlock};
use norito::codec::Encode as _;

use crate::sumeragi::{consensus::Proposal, message::ProposalHint};

pub(super) struct ProposalCache {
    pub(super) hints: BTreeMap<(u64, u64), ProposalHint>,
    pub(super) proposals: BTreeMap<(u64, u64), Proposal>,
    limit: usize,
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
            limit,
        }
    }

    pub(super) fn insert_hint(&mut self, hint: ProposalHint) {
        let key = (hint.height, hint.view);
        self.hints.insert(key, hint);
        self.evict_if_needed();
    }

    pub(super) fn get_hint(&self, height: u64, view: u64) -> Option<&ProposalHint> {
        self.hints.get(&(height, view))
    }

    pub(super) fn get_proposal(&self, height: u64, view: u64) -> Option<&Proposal> {
        self.proposals.get(&(height, view))
    }

    pub(super) fn pop_hint(&mut self, height: u64, view: u64) -> Option<ProposalHint> {
        self.hints.remove(&(height, view))
    }

    pub(super) fn insert_proposal(&mut self, proposal: Proposal) {
        let key = (proposal.header.height, proposal.header.view);
        self.proposals.insert(key, proposal);
        self.evict_if_needed();
    }

    pub(super) fn pop_proposal(&mut self, height: u64, view: u64) -> Option<Proposal> {
        self.proposals.remove(&(height, view))
    }

    fn evict_if_needed(&mut self) {
        while self.hints.len() > self.limit {
            if let Some(first_key) = self.hints.keys().next().copied() {
                self.hints.remove(&first_key);
            } else {
                break;
            }
        }
        while self.proposals.len() > self.limit {
            if let Some(first_key) = self.proposals.keys().next().copied() {
                self.proposals.remove(&first_key);
            } else {
                break;
            }
        }
    }

    pub(super) fn prune_height_leq(&mut self, height: u64) {
        self.hints.retain(|(h, _), _| *h > height);
        self.proposals.retain(|(h, _), _| *h > height);
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

impl ProposalMismatch {
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

pub(super) fn detect_proposal_mismatch(
    proposal: &Proposal,
    header: &BlockHeader,
    payload_hash: &Hash,
) -> Option<ProposalMismatch> {
    let block_height = header.height().get();
    let block_view = u64::from(header.view_change_index());
    if proposal.header.height != block_height {
        return Some(ProposalMismatch::Height {
            proposal: proposal.header.height,
            block: block_height,
        });
    }
    if proposal.header.view != block_view {
        return Some(ProposalMismatch::View {
            proposal: proposal.header.view,
            block: block_view,
        });
    }
    let expected_parent = parent_hash_from_header(header);
    if proposal.header.parent_hash != expected_parent {
        return Some(ProposalMismatch::Parent {
            expected: expected_parent,
            observed: proposal.header.parent_hash,
        });
    }
    let expected_tx_root = tx_root_from_header(header);
    if proposal.header.tx_root != expected_tx_root {
        return Some(ProposalMismatch::TxRoot {
            expected: expected_tx_root,
            observed: proposal.header.tx_root,
        });
    }
    let expected_state_root = state_root_from_header(header);
    if proposal.header.state_root != expected_state_root {
        return Some(ProposalMismatch::StateRoot {
            expected: expected_state_root,
            observed: proposal.header.state_root,
        });
    }
    if &proposal.payload_hash != payload_hash {
        return Some(ProposalMismatch::PayloadHash {
            expected: *payload_hash,
            observed: proposal.payload_hash,
        });
    }
    None
}

/// Canonicalize block payload encoding before hashing to avoid layout drift from Norito’s adaptive
/// encode heuristics. Strip execution results and signatures so the payload hash stays stable across
/// validation and signature collection.
pub(super) fn block_payload_bytes(block: &SignedBlock) -> Vec<u8> {
    let mut header = block.header();
    header.result_merkle_root = None;
    BlockPayload {
        header,
        transactions: block.transactions_vec().clone(),
        da_commitments: block.da_commitments().cloned(),
        da_proof_policies: block.da_proof_policies().cloned(),
        da_pin_intents: block.da_pin_intents().cloned(),
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
