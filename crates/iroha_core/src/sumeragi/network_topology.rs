#![allow(unexpected_cfgs, clippy::similar_names)]
//! Structures formalising the peer topology (e.g. which peers have which predefined roles).

use core::convert::TryFrom;

use derive_more::Display;
use indexmap::IndexSet;
use iroha_crypto::HashOf;
#[cfg(test)]
use iroha_crypto::KeyPair;
use iroha_crypto::PublicKey;
use iroha_data_model::{
    block::{BlockHeader, BlockSignature},
    prelude::PeerId,
};

/// The ordering of the peers which defines their roles in the current round of consensus.
///
/// A  |       |              |>|                  |->|
/// B  |       |              | |                  |  V
/// C  | A Set |              ^ V  Rotate A Set    ^  |
/// D  | 2f +1 |              | |                  |  V  Rotate all
/// E  |       |              |<|                  ^  |
/// F             | B Set |                        |  V
/// G             |   f   |                        |<-|
///
/// Above is an illustration of how the various operations work for a f = 2 topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology(
    /// Ordered set of peers
    Vec<PeerId>,
    /// Current view change index. Reset to 0 after every block commit.
    u64,
);

/// Topology with at least one peer
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Deref)]
pub struct NonEmptyTopology<'topology> {
    topology: &'topology Topology,
}

/// Topology which requires consensus (more than one peer)
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Deref)]
pub struct ConsensusTopology<'topology> {
    topology: &'topology Topology,
}

impl AsRef<[PeerId]> for Topology {
    fn as_ref(&self) -> &[PeerId] {
        &self.0
    }
}

impl IntoIterator for Topology {
    type Item = PeerId;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Topology {
    /// Create a new topology.
    pub fn new(peers: impl IntoIterator<Item = PeerId>) -> Self {
        let topology = peers.into_iter().collect::<IndexSet<_>>();

        assert!(
            !topology.is_empty(),
            "Topology must contain at least one peer"
        );

        Topology(topology.into_iter().collect(), 0)
    }

    pub(crate) fn position(&self, peer: &PublicKey) -> Option<usize> {
        self.0.iter().position(|p| p.public_key() == peer)
    }

    #[allow(dead_code)]
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &PeerId> {
        self.0.iter()
    }

    /// True, if the topology contains at least one peer and thus requires consensus
    pub fn is_non_empty(&self) -> Option<NonEmptyTopology<'_>> {
        (!self.0.is_empty()).then_some(NonEmptyTopology { topology: self })
    }

    /// Is consensus required, aka are there more than 1 peer.
    pub fn is_consensus_required(&self) -> Option<ConsensusTopology<'_>> {
        (self.0.len() > 1).then_some(ConsensusTopology { topology: self })
    }

    /// How many faulty peers can this topology tolerate.
    pub fn max_faults(&self) -> usize {
        (self.0.len().saturating_sub(1)) / 3
    }

    /// The required amount of votes to commit a block with this topology.
    pub fn min_votes_for_commit(&self) -> usize {
        commit_quorum_from_len(self.0.len())
    }

    /// The required amount of votes to trigger a view change (f + 1).
    pub fn min_votes_for_view_change(&self) -> usize {
        let required = self.max_faults().saturating_add(1);
        required.min(self.0.len().max(1))
    }

    /// Index of leader
    #[allow(clippy::unused_self)] // In order to be consistent with `proxy_tail_index` method
    pub const fn leader_index(&self) -> usize {
        0
    }

    /// Index of proxy tail
    pub fn proxy_tail_index(&self) -> usize {
        self.min_votes_for_commit().saturating_sub(1)
    }

    /// Deterministic set of collector indices for the current topology.
    ///
    /// Collectors are the peers responsible for aggregating votes and forming a QC.
    /// By default this is a contiguous slice starting at `proxy_tail_index()` and
    /// extending into Set B validators. The leader is excluded except for single-peer
    /// topologies, where index `0` is used so local vote aggregation can progress.
    /// Indices do not wrap around.
    ///
    /// Returns at most `k` indices; fewer if there are not enough peers after
    /// the proxy tail.
    pub fn collector_indices_k(&self, k: usize) -> Vec<usize> {
        if self.0.is_empty() || k == 0 {
            return Vec::new();
        }
        if self.0.len() == 1 {
            return vec![0];
        }
        let start = self.proxy_tail_index();
        let end_exclusive = (start + k).min(self.0.len());
        if end_exclusive <= start {
            return Vec::new();
        }
        (start..end_exclusive).collect()
    }

    /// Effective collector fan-out (k) floor for the current topology.
    ///
    /// Ensures the requested `k` is at least the commit quorum size, bounded by the number
    /// of non-leader peers. Returns `0` for empty topologies or when `k` is `0`.
    /// A single-peer topology uses the local validator as its only collector.
    pub fn collector_fanout_floor(&self, k: usize) -> usize {
        if self.0.is_empty() || k == 0 {
            return 0;
        }
        if self.0.len() == 1 {
            return 1;
        }
        let required = self.min_votes_for_commit();
        let max_collectors = self.0.len().saturating_sub(1);
        k.max(required).min(max_collectors)
    }

    /// Fallback collector indices when PRF selection is unavailable.
    ///
    /// The fallback starts at `proxy_tail_index()` and wraps around to fill at least
    /// the commit quorum size (bounded by the number of non-leader peers). The
    /// leader is excluded except for single-peer topologies. Indices are unique.
    pub fn collector_indices_k_fallback(&self, k: usize) -> Vec<usize> {
        if self.0.is_empty() || k == 0 {
            return Vec::new();
        }
        if self.0.len() == 1 {
            return vec![0];
        }
        let desired = self.collector_fanout_floor(k);
        if desired == 0 {
            return Vec::new();
        }

        let mut indices = Vec::with_capacity(desired);
        let mut idx = self.proxy_tail_index();
        let n = self.0.len();
        for _ in 0..n {
            if idx != self.leader_index() {
                indices.push(idx);
                if indices.len() == desired {
                    break;
                }
            }
            idx = (idx + 1) % n;
        }
        indices
    }

    /// Deterministic topology fan-out indices starting at `proxy_tail_index()`.
    ///
    /// This is a bounded wraparound slice (skipping the leader) that does not force
    /// the quorum-sized floor. Useful for small parallel fan-out paths where only a
    /// few extra targets are desired.
    pub fn topology_fanout_from_tail(&self, count: usize) -> Vec<usize> {
        if self.0.len() <= 1 || count == 0 {
            return Vec::new();
        }
        let max_collectors = self.0.len().saturating_sub(1);
        let desired = count.min(max_collectors);
        if desired == 0 {
            return Vec::new();
        }
        let mut indices = Vec::with_capacity(desired);
        let mut idx = self.proxy_tail_index();
        let n = self.0.len();
        for _ in 0..n {
            if idx != self.leader_index() {
                indices.push(idx);
                if indices.len() == desired {
                    break;
                }
            }
            idx = (idx + 1) % n;
        }
        indices
    }

    /// Deterministic set of collectors (peer ids) for the current topology.
    pub fn collectors_k(&self, k: usize) -> Vec<&PeerId> {
        self.collector_indices_k(k)
            .into_iter()
            .map(|idx| &self.0[idx])
            .collect()
    }

    /// Clamp the configured redundant send fan-out to at least the commit quorum size.
    pub fn redundant_send_r_floor(&self, configured: u8) -> u8 {
        let quorum = self.min_votes_for_commit();
        let quorum_u8 = u8::try_from(quorum).unwrap_or(u8::MAX);
        configured.max(quorum_u8).max(1)
    }

    /// Rotate peers so that the peer at `idx` becomes leader (index 0),
    /// preserving the current `view_change_index` counter.
    pub fn rotate_preserve_view_to_front(&mut self, idx: usize) {
        if self.0.is_empty() {
            return;
        }
        let n = self.0.len();
        let r = idx % n;
        self.0.rotate_left(r);
    }

    /// Deterministically shuffle the topology for a given `height` using the PRF seed.
    ///
    /// Callers should canonicalize the roster before shuffling to keep the
    /// permutation deterministic across nodes.
    pub fn shuffle_prf(&mut self, seed: [u8; 32], height: u64) {
        let n = self.0.len();
        if n <= 1 {
            return;
        }
        let mut slots: Vec<usize> = (0..n).collect();
        let mut shuffled = Vec::with_capacity(n);
        let mut ctr: u64 = 0;
        while !slots.is_empty() {
            let pos = Self::shuffle_prf_slot(seed, height, ctr, slots.len());
            let pick = slots.swap_remove(pos);
            shuffled.push(self.0[pick].clone());
            ctr = ctr.saturating_add(1);
        }
        self.0 = shuffled;
    }

    /// PRF-based leader index for (height, view).
    pub fn leader_index_prf(&self, seed: [u8; 32], height: u64, view: u64) -> usize {
        use iroha_crypto::blake2::{Blake2b512, Digest as _};
        let n = self.0.len();
        if n == 0 {
            return 0;
        }
        let mut hasher = Blake2b512::new();
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &seed);
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &height.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &view.to_be_bytes());
        let digest = iroha_crypto::blake2::Digest::finalize(hasher);
        let mut w = [0u8; 8];
        w.copy_from_slice(&digest[..8]);
        let modulus = u128::try_from(n).expect("topology length fits u128");
        (u128::from(u64::from_be_bytes(w)) % modulus) as usize
    }

    /// `NPoS`-style PRF-based collector selection for a given (height, view).
    ///
    /// Deterministically selects up to `k` distinct collector indices from the validator set,
    /// excluding the leader index for multi-peer rosters. Single-peer rosters always
    /// return index `0` so local vote aggregation remains possible. Selection uses a
    /// Blake2b-512 based PRF seeded by `seed` and keyed by `(height, view, counter)`
    /// to avoid repeats.
    pub fn collector_indices_k_prf(
        &self,
        k: usize,
        seed: [u8; 32],
        height: u64,
        view: u64,
    ) -> Vec<usize> {
        let n = self.0.len();
        if n == 0 || k == 0 {
            return Vec::new();
        }
        if n == 1 {
            return vec![0];
        }
        // Build candidate list excluding the leader index (0).
        let mut candidates: Vec<usize> = (1..n).collect();
        let mut selected: Vec<usize> = Vec::new();
        let mut ctr: u64 = 0;
        while selected.len() < k && !candidates.is_empty() {
            let pos = Self::collector_prf_slot(seed, height, view, ctr, candidates.len());
            let pick = candidates.swap_remove(pos);
            selected.push(pick);
            ctr = ctr.saturating_add(1);
        }
        selected
    }

    fn collector_prf_slot(
        seed: [u8; 32],
        height: u64,
        view: u64,
        ctr: u64,
        modulus: usize,
    ) -> usize {
        use iroha_crypto::blake2::{Blake2b512, Digest as _};

        debug_assert!(modulus > 0);

        let mut hasher = Blake2b512::new();
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &seed);
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &height.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &view.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &ctr.to_be_bytes());
        let digest = iroha_crypto::blake2::Digest::finalize(hasher);
        let mut idx_bytes = [0u8; 8];
        idx_bytes.copy_from_slice(&digest[..8]);
        let r = u64::from_be_bytes(idx_bytes);
        let modulus = u128::try_from(modulus).expect("candidate length fits u128");
        (u128::from(r) % modulus) as usize
    }

    fn shuffle_prf_slot(seed: [u8; 32], height: u64, ctr: u64, modulus: usize) -> usize {
        use iroha_crypto::blake2::{Blake2b512, Digest as _};

        debug_assert!(modulus > 0);

        let mut hasher = Blake2b512::new();
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &seed);
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &height.to_be_bytes());
        iroha_crypto::blake2::digest::Update::update(&mut hasher, &ctr.to_be_bytes());
        let digest = iroha_crypto::blake2::Digest::finalize(hasher);
        let mut idx_bytes = [0u8; 8];
        idx_bytes.copy_from_slice(&digest[..8]);
        let r = u64::from_be_bytes(idx_bytes);
        let modulus = u128::try_from(modulus).expect("candidate length fits u128");
        (u128::from(r) % modulus) as usize
    }

    /// Whether the given peer participates as a collector for the current topology
    /// given the desired `k`.
    pub fn is_collector(&self, peer: &PeerId, k: usize) -> bool {
        self.collector_indices_k(k)
            .into_iter()
            .any(|idx| &self.0[idx] == peer)
    }

    /// Index of leader
    pub fn leader(&self) -> &PeerId {
        &self.0[self.leader_index()]
    }

    /// Index of leader
    pub fn proxy_tail(&self) -> &PeerId {
        &self.0[self.proxy_tail_index()]
    }

    /// Filter signatures by roles in the topology.
    pub fn filter_signatures_by_roles<'a, I: IntoIterator<Item = &'a BlockSignature>>(
        &self,
        roles: &[Role],
        signatures: I,
    ) -> impl Iterator<Item = &'a BlockSignature>
    where
        <I as IntoIterator>::IntoIter: 'a,
    {
        let mut filtered = IndexSet::new();

        for role in roles {
            match (role, self.is_non_empty(), self.is_consensus_required()) {
                (Role::Leader, Some(topology), _) => {
                    filtered.insert(topology.leader_index());
                }
                (Role::ProxyTail, Some(topology), None) => {
                    filtered.insert(topology.proxy_tail_index());
                }
                (Role::ProxyTail, _, Some(topology)) => {
                    filtered.insert(topology.proxy_tail_index());
                }
                (Role::ValidatingPeer, _, Some(topology)) => {
                    filtered.extend(topology.leader_index() + 1..topology.proxy_tail_index());
                }
                (Role::SetBValidator, _, Some(topology)) => {
                    filtered.extend(topology.proxy_tail_index() + 1..topology.0.len());
                }
                _ => {}
            }
        }

        signatures.into_iter().filter(move |signature| {
            usize::try_from(signature.index())
                .ok()
                .is_some_and(|idx| filtered.contains(&idx))
        })
    }

    /// What role does this peer have in the topology.
    pub fn role(&self, peer: &PeerId) -> Role {
        match self.position(peer.public_key()) {
            Some(x) if x == self.leader_index() => Role::Leader,
            Some(x) if x < self.proxy_tail_index() => Role::ValidatingPeer,
            Some(x) if x == self.proxy_tail_index() => Role::ProxyTail,
            Some(_) => Role::SetBValidator,
            None => Role::Undefined,
        }
    }

    /// Add or remove peers from the topology.
    fn update_peer_list(&mut self, new_peers: impl IntoIterator<Item = PeerId>) {
        let (old_peers, new_peers): (IndexSet<_>, IndexSet<_>) = new_peers
            .into_iter()
            .partition(|peer| self.0.contains(peer));
        self.0.retain(|peer| old_peers.contains(peer));
        self.0.extend(new_peers);
    }

    /// Rotate peers n times.
    pub fn nth_rotation(&mut self, n: u64) -> u64 {
        assert!(n >= self.1, "View change index must monotonically increase");

        let rotations = n - self.1;
        let len = self.0.len() as u64;
        if len > 0 {
            let rem = usize::try_from(rotations % len).unwrap_or(0);
            if rem > 0 {
                self.0.rotate_left(rem);
            }
        }

        self.1 = n;
        rotations
    }

    /// Return current view change index of topology
    pub fn view_change_index(&self) -> u64 {
        self.1
    }

    /// Update topology after a block has been committed.
    ///
    /// Membership is refreshed while preserving canonical ordering for subsequent
    /// per-view role derivation.
    pub fn block_committed(
        &mut self,
        new_peers: impl IntoIterator<Item = PeerId>,
        _prev_block_hash: HashOf<BlockHeader>,
    ) {
        self.update_peer_list(new_peers);
        self.1 = 0;
    }

    /// Canonicalize the internal peer ordering without changing the view index.
    pub(crate) fn canonicalize_order(&mut self) {
        if self.0.is_empty() {
            return;
        }
        self.0.sort();
        self.0.dedup();
    }
}

/// Compute the commit quorum size for a topology of the given length.
#[must_use]
pub fn commit_quorum_from_len(len: usize) -> usize {
    if len <= 3 {
        return len;
    }
    len.saturating_mul(2).saturating_add(1) / 3
}

/// Compute the redundant send fan-out (r) for a topology of the given length.
///
/// Uses the standard `2f+1` formula with `f = floor((len - 1) / 3)`, clamped to `u8::MAX`.
#[must_use]
pub fn redundant_send_r_from_len(len: usize) -> u8 {
    let len = len.max(1);
    let max_faults = len.saturating_sub(1) / 3;
    let r = max_faults.saturating_mul(2).saturating_add(1);
    u8::try_from(r).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod prf_collectors_tests {
    use super::*;

    #[test]
    fn topology_new_deduplicates_peers_preserving_order() {
        let peer_a = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let topology = Topology::new(vec![peer_a.clone(), peer_b.clone(), peer_a.clone()]);

        let peers = topology.as_ref();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0], peer_a);
        assert_eq!(peers[1], peer_b);
    }

    #[test]
    fn prf_collectors_are_unique_and_exclude_leader() {
        // Build a 7-peer topology
        let keys: Vec<PeerId> = (0..7)
            .map(|_i| {
                let pk = iroha_crypto::KeyPair::random().public_key().clone();
                // keep unique but deterministic order by index (unused here)
                PeerId::new(pk)
            })
            .collect();
        let topo = Topology::new(keys);
        let seed = [7u8; 32];
        let idxs = topo.collector_indices_k_prf(3, seed, 10, 5);
        assert!(idxs.len() <= 3);
        // No repeats
        let set: std::collections::BTreeSet<_> = idxs.iter().copied().collect();
        assert_eq!(set.len(), idxs.len());
        // Excludes leader 0
        assert!(idxs.iter().all(|&i| i != 0));
    }

    #[test]
    fn prf_leader_is_deterministic_and_varies_with_seed() {
        let peers: Vec<PeerId> = (0..8)
            .map(|_| PeerId::new(iroha_crypto::KeyPair::random().public_key().clone()))
            .collect();
        let topo = Topology::new(peers);
        let seed_a = [1u8; 32];
        let seed_b = [2u8; 32];
        let la1 = topo.leader_index_prf(seed_a, 10, 5);
        let la2 = topo.leader_index_prf(seed_a, 10, 5);
        let lb = topo.leader_index_prf(seed_b, 10, 5);
        assert_eq!(la1, la2);
        // With different seed, high probability to differ; allow equality only if very unlucky
        // Accept difference as a sanity check:
        assert!(la1 != lb || topo.as_ref().len() == 1);
    }
}

/// Historical rotation keyed to the previous block hash.
///
/// This helper produces a `Topology` whose Set A (first `min_votes_for_commit()` peers)
/// is rotated left by `hash(prev_block_hash) mod min_votes_for_commit` positions. The
/// Set B segment order is preserved, and the view change index is set to 0.
///
/// Notes
/// - Rotation depends only on the provided inputs. It is not influenced by any
///   signature set or runtime state and is thus suitable for auditing.
/// - `prev_block_hash` is the hash of the block immediately preceding the round.
///
/// Invariants
/// - Deterministic across nodes and hardware.
/// - Independent of any observed QC signer set for the same height.
/// - Historical behavior; permissioned mode now uses PRF-based ordering.
///
/// Example
/// ```ignore
/// use iroha_core::sumeragi::network_topology::{rotated_for_prev_block_hash, ConsensusTopology};
/// use iroha_data_model::{block::BlockHeader, prelude::PeerId};
/// use iroha_crypto::HashOf;
///
/// // Application provides the current ordered peer list (e.g., from committed state)
/// let peers: Vec<PeerId> = get_current_peers();
/// let prev_hash: HashOf<BlockHeader> = get_prev_block_hash();
///
/// let topo = rotated_for_prev_block_hash(peers, prev_hash);
/// let leader = topo.leader().clone();
/// let proxy_tail = topo
///     .is_consensus_required()
///     .expect("N > 1 required for proxy tail")
///     .proxy_tail()
///     .clone();
/// // Now (leader, proxy_tail) are the expected roles for the next round
/// ```
pub fn rotated_for_prev_block_hash(
    peers: impl IntoIterator<Item = PeerId>,
    prev_block_hash: HashOf<BlockHeader>,
) -> Topology {
    // Canonicalize ordering to keep topology deterministic across nodes and restarts.
    let mut peers: Vec<PeerId> = peers.into_iter().collect();
    peers.sort();
    peers.dedup();

    let mut topology = Topology::new(peers);
    let rotate_at = topology.min_votes_for_commit();
    let k = rotation_offset_for_prev_hash(&prev_block_hash, rotate_at);
    if k > 0 {
        topology.0[..rotate_at].rotate_left(k);
    }
    topology
}

fn rotation_offset_for_prev_hash(prev_block_hash: &HashOf<BlockHeader>, rotate_at: usize) -> usize {
    if rotate_at == 0 {
        return 0;
    }
    let mut head = [0u8; 8];
    head.copy_from_slice(&prev_block_hash.as_ref()[..8]);
    let modulus = u128::try_from(rotate_at).expect("rotate_at fits u128");
    (u128::from(u64::from_be_bytes(head)) % modulus) as usize
}

/// Compute the expected role of each peer for auditing given the previous block hash.
/// Returns a vector of (`PeerId`, `Role`) in the rotated topology order.
pub fn audit_roles_for_prev_block_hash(
    peers: impl IntoIterator<Item = PeerId>,
    prev_block_hash: HashOf<BlockHeader>,
) -> Vec<(PeerId, Role)> {
    let topo = rotated_for_prev_block_hash(peers, prev_block_hash);
    topo.0
        .iter()
        .cloned()
        .map(|pid| {
            let role = topo.role(&pid);
            (pid, role)
        })
        .collect()
}

/// Deterministic PRF-based shuffle for permissioned ordering at a given height.
pub fn shuffled_for_prf_seed(
    peers: impl IntoIterator<Item = PeerId>,
    seed: [u8; 32],
    height: u64,
) -> Topology {
    // Canonicalize ordering to keep topology deterministic across nodes and restarts.
    let mut peers: Vec<PeerId> = peers.into_iter().collect();
    peers.sort();
    peers.dedup();
    let mut topology = Topology::new(peers);
    topology.shuffle_prf(seed, height);
    topology
}

impl<'topology> NonEmptyTopology<'topology> {
    /// Get leader's [`PeerId`].
    pub fn leader(&self) -> &'topology PeerId {
        &self.topology.0[self.topology.leader_index()]
    }
}

impl<'topology> ConsensusTopology<'topology> {
    /// Get proxy tail's peer id.
    pub fn proxy_tail(&self) -> &'topology PeerId {
        &self.topology.0[self.topology.proxy_tail_index()]
    }

    /// Get leader's [`PeerId`]
    pub fn leader(&self) -> &'topology PeerId {
        &self.topology.0[self.topology.leader_index()]
    }

    /// Get Set A validator [`PeerId`]s (excluding leader/proxy tail).
    pub fn validating_peers(&self) -> &'topology [PeerId] {
        &self.0[self.leader_index() + 1..self.proxy_tail_index()]
    }

    /// Get Set B validator [`PeerId`]s (tail segment after proxy tail).
    pub fn set_b_validators(&self) -> &'topology [PeerId] {
        &self.0[self.proxy_tail_index() + 1..]
    }

    /// Get all voting [`PeerId`]s (Set A + Set B).
    pub fn voting_peers(&self) -> &'topology [PeerId] {
        &self.0[..]
    }
}

/// Possible Peer's roles in consensus.
#[derive(Debug, Display, Clone, Copy, PartialOrd, Ord, Eq, PartialEq, Hash)]
pub enum Role {
    /// Leader.
    Leader,
    /// Validating Peer.
    ValidatingPeer,
    /// Set B validator (tail segment; still voting).
    SetBValidator,
    /// Proxy Tail.
    ProxyTail,
    /// Undefined. Not part of the topology
    Undefined,
}

#[cfg(test)]
#[allow(dead_code)]
fn test_peers(n_peers: usize) -> Vec<PeerId> {
    let mut peers: Vec<_> = (0..n_peers)
        .map(|_| PeerId::new(KeyPair::random().into_parts().0))
        .collect();
    peers.sort();
    peers
}

#[cfg(test)]
/// Construct a test `Topology` with `n_peers` randomly generated keys.
pub fn test_topology(n_peers: usize) -> Topology {
    let keys = (0..n_peers).map(|_| KeyPair::random()).collect::<Vec<_>>();
    test_topology_with_keys(&keys)
}

#[cfg(test)]
#[allow(single_use_lifetimes)] // false-positive
/// Construct a test `Topology` from provided key pairs.
pub fn test_topology_with_keys<'a>(keys: impl IntoIterator<Item = &'a KeyPair>) -> Topology {
    let peers = keys
        .into_iter()
        .map(|key| PeerId::new(key.public_key().clone()));
    Topology::new(peers)
}

#[cfg(all(test, feature = "iroha-core-tests"))]
mod tests {
    #![allow(unused_variables, unused_mut)]
    use iroha_crypto::KeyPair;
    use iroha_primitives::unique_vec;

    use super::*;
    use crate::block::ValidBlock;

    fn extract_order(topology: &Topology, initial_topology: &Topology) -> Vec<usize> {
        topology
            .0
            .iter()
            .map(|peer| {
                initial_topology
                    .0
                    .iter()
                    .position(|p| p.public_key() == peer.public_key())
                    .unwrap()
            })
            .collect()
    }

    fn prev_hash_with_seed(seed: u64) -> HashOf<BlockHeader> {
        let mut seed_bytes = [0u8; iroha_crypto::Hash::LENGTH];
        seed_bytes[..8].copy_from_slice(&seed.to_be_bytes());
        HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed(seed_bytes))
    }

    #[test]
    fn rotate_set_a() {
        let mut topology = test_topology(7);
        let initial_topology = topology.clone();
        let rotate_at = topology.min_votes_for_commit().min(topology.0.len());
        if rotate_at > 1 {
            topology.0[..rotate_at].rotate_left(1);
        }
        assert_eq!(
            extract_order(&topology, &initial_topology),
            vec![1, 2, 3, 4, 0, 5, 6]
        )
    }

    #[test]
    fn update_peer_list() {
        let mut topology = test_topology(7);
        let peer0 = topology.0[0].clone();
        let peer2 = topology.0[2].clone();
        let peer5 = topology.0[5].clone();
        let peer7 = test_peers(1).remove(0);
        // New peers will be 0, 2, 5, 7
        let new_peers = unique_vec![peer5.clone(), peer0.clone(), peer2.clone(), peer7.clone()];
        topology.update_peer_list(new_peers);
        assert_eq!(topology.0, vec![peer0, peer2, peer5, peer7])
    }

    #[test]
    fn filter_by_role() {
        let key_pairs = core::iter::repeat_with(KeyPair::random)
            .take(7)
            .collect::<Vec<_>>();
        let topology = test_topology_with_keys(&key_pairs);

        let dummy_block = ValidBlock::new_dummy(key_pairs[0].private_key());
        let dummy_signature = dummy_block
            .as_ref()
            .signatures()
            .next()
            .unwrap()
            .signature()
            .clone();
        let dummy_signatures = (0..key_pairs.len())
            .map(|i| BlockSignature::new(i as u64, dummy_signature.clone()))
            .collect::<Vec<_>>();

        let leader_signatures = topology
            .filter_signatures_by_roles(&[Role::Leader], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(leader_signatures.len(), 1);
        assert_eq!(leader_signatures[0].index(), 0);

        let proxy_tail_signatures = topology
            .filter_signatures_by_roles(&[Role::ProxyTail], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(proxy_tail_signatures.len(), 1);
        assert_eq!(proxy_tail_signatures[0].index(), 4);

        let validating_peers_signatures = topology
            .filter_signatures_by_roles(&[Role::ValidatingPeer], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(validating_peers_signatures.len(), 3);
        assert!(
            validating_peers_signatures
                .iter()
                .map(|s| s.index())
                .eq(1..4)
        );

        let set_b_signatures = topology
            .filter_signatures_by_roles(&[Role::SetBValidator], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(set_b_signatures.len(), 2);
        assert!(set_b_signatures.iter().map(|s| s.index()).eq(5..7));
    }

    #[test]
    fn filter_by_role_ignores_invalid_signature_indices() {
        let key_pairs = core::iter::repeat_with(KeyPair::random)
            .take(3)
            .collect::<Vec<_>>();
        let topology = test_topology_with_keys(key_pairs.iter().take(3));

        let dummy_block = ValidBlock::new_dummy(key_pairs[0].private_key());
        let dummy_signature = dummy_block
            .as_ref()
            .signatures()
            .next()
            .unwrap()
            .signature()
            .clone();
        let dummy_signatures = [
            BlockSignature::new(0, dummy_signature.clone()),
            BlockSignature::new(u64::MAX, dummy_signature.clone()),
        ];

        let leader_signatures = topology
            .filter_signatures_by_roles(&[Role::Leader], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(leader_signatures.len(), 1);
        assert_eq!(leader_signatures[0].index(), 0);
    }

    #[test]
    fn filter_by_role_1() {
        let key_pairs = core::iter::repeat_with(KeyPair::random)
            .take(7)
            .collect::<Vec<_>>();
        let key_pairs_iter = key_pairs.iter().take(1);
        let topology = test_topology_with_keys(key_pairs_iter);

        let dummy_block = ValidBlock::new_dummy(key_pairs[0].private_key());
        let dummy_signature = dummy_block
            .as_ref()
            .signatures()
            .next()
            .unwrap()
            .signature()
            .clone();
        let dummy_signatures = (0..key_pairs.len())
            .map(|i| BlockSignature::new(i as u64, dummy_signature.clone()))
            .collect::<Vec<_>>();

        let leader_signatures = topology
            .filter_signatures_by_roles(&[Role::Leader], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(leader_signatures.len(), 1);
        assert_eq!(leader_signatures[0].index(), 0);

        let proxy_tail_signatures =
            topology.filter_signatures_by_roles(&[Role::ProxyTail], dummy_signatures.iter());
        let proxy_tail_signatures = proxy_tail_signatures.collect::<Vec<_>>();
        assert_eq!(proxy_tail_signatures.len(), 1);
        assert_eq!(proxy_tail_signatures[0].index(), 0);

        let mut validating_peers_signatures =
            topology.filter_signatures_by_roles(&[Role::ValidatingPeer], dummy_signatures.iter());
        assert!(validating_peers_signatures.next().is_none());

        let mut set_b_signatures =
            topology.filter_signatures_by_roles(&[Role::SetBValidator], dummy_signatures.iter());
        assert!(set_b_signatures.next().is_none());
    }

    #[test]
    fn filter_by_role_2() {
        let key_pairs = core::iter::repeat_with(KeyPair::random)
            .take(7)
            .collect::<Vec<_>>();
        let key_pairs_iter = key_pairs.iter().take(2);
        let topology = test_topology_with_keys(key_pairs_iter);

        let dummy_block = ValidBlock::new_dummy(key_pairs[0].private_key());
        let dummy_signature = dummy_block
            .as_ref()
            .signatures()
            .next()
            .unwrap()
            .signature()
            .clone();
        let dummy_signatures = (0..key_pairs.len())
            .map(|i| BlockSignature::new(i as u64, dummy_signature.clone()))
            .collect::<Vec<_>>();

        let leader_signatures = topology
            .filter_signatures_by_roles(&[Role::Leader], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(leader_signatures.len(), 1);
        assert_eq!(leader_signatures[0].index(), 0);

        let proxy_tail_signatures = topology
            .filter_signatures_by_roles(&[Role::ProxyTail], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(proxy_tail_signatures.len(), 1);
        assert_eq!(proxy_tail_signatures[0].index(), 1);

        let mut validating_peers_signatures =
            topology.filter_signatures_by_roles(&[Role::ValidatingPeer], dummy_signatures.iter());
        assert!(validating_peers_signatures.next().is_none());

        let mut set_b_signatures =
            topology.filter_signatures_by_roles(&[Role::SetBValidator], dummy_signatures.iter());
        assert!(set_b_signatures.next().is_none());
    }

    #[test]
    fn filter_by_role_3() {
        let key_pairs = core::iter::repeat_with(KeyPair::random)
            .take(7)
            .collect::<Vec<_>>();
        let key_pairs_iter = key_pairs.iter().take(3);
        let topology = test_topology_with_keys(key_pairs_iter);

        let dummy_block = ValidBlock::new_dummy(key_pairs[0].private_key());
        let dummy_signature = dummy_block
            .as_ref()
            .signatures()
            .next()
            .unwrap()
            .signature()
            .clone();
        let dummy_signatures = (0..key_pairs.len())
            .map(|i| BlockSignature::new(i as u64, dummy_signature.clone()))
            .collect::<Vec<_>>();

        let leader_signatures = topology
            .filter_signatures_by_roles(&[Role::Leader], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(leader_signatures.len(), 1);
        assert_eq!(leader_signatures[0].index(), 0);

        let proxy_tail_signatures = topology
            .filter_signatures_by_roles(&[Role::ProxyTail], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(proxy_tail_signatures.len(), 1);
        assert_eq!(proxy_tail_signatures[0].index(), 2);

        let validating_peers_signatures = topology
            .filter_signatures_by_roles(&[Role::ValidatingPeer], dummy_signatures.iter())
            .collect::<Vec<_>>();
        assert_eq!(validating_peers_signatures.len(), 1);
        assert_eq!(validating_peers_signatures[0].index(), 1);

        let mut set_b_signatures =
            topology.filter_signatures_by_roles(&[Role::SetBValidator], dummy_signatures.iter());
        assert!(set_b_signatures.next().is_none());
    }

    #[test]
    fn proxy_tail() {
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::proxy_tail),
            Some(&peers[4])
        );
    }

    #[test]
    #[should_panic(expected = "Topology must contain at least one peer")]
    fn topology_empty() {
        let _topology = Topology::new(Vec::new());
    }

    #[test]
    fn proxy_tail_1() {
        let topology = test_topology(1);

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::proxy_tail),
            None
        );
    }

    #[test]
    fn proxy_tail_2() {
        let peers = test_peers(2);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::proxy_tail),
            Some(&peers[1])
        );
    }

    #[test]
    fn proxy_tail_3() {
        let peers = test_peers(3);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::proxy_tail),
            Some(&peers[2])
        );
    }

    #[test]
    fn leader() {
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_non_empty()
                .as_ref()
                .map(NonEmptyTopology::leader),
            Some(&peers[0])
        );
    }

    #[test]
    fn leader_1() {
        let peers = test_peers(1);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_non_empty()
                .as_ref()
                .map(NonEmptyTopology::leader),
            Some(&peers[0])
        );
    }

    #[test]
    fn leader_2() {
        let peers = test_peers(2);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_non_empty()
                .as_ref()
                .map(NonEmptyTopology::leader),
            Some(&peers[0])
        );
    }

    #[test]
    fn leader_3() {
        let peers = test_peers(3);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_non_empty()
                .as_ref()
                .map(NonEmptyTopology::leader),
            Some(&peers[0])
        );
    }

    #[test]
    fn validating_peers() {
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            Some(&peers[1..4])
        );
    }

    #[test]
    fn validating_peers_1() {
        let peers = test_peers(1);
        let topology = Topology::new(peers);

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            None
        );
    }

    #[test]
    fn validating_peers_2() {
        let peers = test_peers(2);
        let topology = Topology::new(peers);

        let empty_peer_slice: &[PeerId] = &[];
        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            Some(empty_peer_slice)
        );
    }

    #[test]
    fn validating_peers_3() {
        let peers = test_peers(3);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            Some(&peers[1..2])
        );
    }

    #[test]
    fn quorum_and_collectors_small_topologies() {
        let cases = [
            (1_usize, 1_usize, vec![0]),
            (2, 2, vec![1]),
            (3, 3, vec![2]),
            (4, 3, vec![2, 3]),
            (5, 3, vec![2, 3]),
        ];

        for (len, expected_min, expected_collectors) in cases {
            let mut topology = test_topology(len);
            assert_eq!(topology.min_votes_for_commit(), expected_min);
            assert_eq!(topology.proxy_tail_index(), expected_min - 1);
            assert_eq!(topology.collector_indices_k(2), expected_collectors);

            if let Some(consensus) = topology.is_consensus_required() {
                let validating = consensus.validating_peers();
                let expected_validating_len = expected_min.saturating_sub(2);
                assert_eq!(validating.len(), expected_validating_len);
            }

            let rotate_at = topology.min_votes_for_commit().min(topology.0.len());
            if rotate_at > 1 {
                topology.0[..rotate_at].rotate_left(1);
            }
            assert_eq!(topology.min_votes_for_commit(), expected_min);
            assert_eq!(topology.proxy_tail_index(), expected_min - 1);
            assert_eq!(topology.collector_indices_k(2), expected_collectors);
        }
    }

    #[test]
    fn commit_quorum_helper_matches_topology_rule() {
        let cases = [1_usize, 2, 3, 4, 5, 6, 7, 9, 10, 16];
        for len in cases {
            let topology = test_topology(len);
            assert_eq!(
                topology.min_votes_for_commit(),
                commit_quorum_from_len(len),
                "quorum mismatch for len={len}"
            );
        }
    }

    #[test]
    fn commit_quorum_len6_is_four() {
        let topology = test_topology(6);
        assert_eq!(commit_quorum_from_len(6), 4);
        assert_eq!(topology.min_votes_for_commit(), 4);
    }

    #[test]
    fn redundant_send_r_tracks_2f_plus_1() {
        let cases = [
            (0_usize, 1_u8),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 3),
            (5, 3),
            (6, 3),
            (7, 5),
            (8, 5),
            (9, 5),
            (10, 7),
        ];
        for (len, expected) in cases {
            assert_eq!(
                redundant_send_r_from_len(len),
                expected,
                "redundant_send_r mismatch for len={len}"
            );
        }
    }

    #[test]
    fn redundant_send_r_clamps_to_u8_max() {
        let len = usize::from(u8::MAX).saturating_mul(4);
        assert_eq!(redundant_send_r_from_len(len), u8::MAX);
    }

    #[test]
    fn view_change_quorum_is_f_plus_one() {
        let cases = [1_usize, 2, 3, 4, 5, 6, 7, 10, 16];
        for len in cases {
            let topology = test_topology(len);
            let max_faults = (len.saturating_sub(1)) / 3;
            let expected = max_faults.saturating_add(1);
            assert_eq!(
                topology.min_votes_for_view_change(),
                expected,
                "view-change quorum mismatch for len={len}"
            );
        }
    }

    #[test]
    fn set_b_validators() {
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::set_b_validators),
            Some(&peers[5..])
        );
    }

    #[test]
    fn set_b_validators_1() {
        let peers = test_peers(1);
        let topology = Topology::new(peers);

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            None
        );
    }

    #[test]
    fn set_b_validators_2() {
        let peers = test_peers(2);
        let topology = Topology::new(peers);

        let empty_peer_slice: &[PeerId] = &[];
        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::set_b_validators),
            Some(empty_peer_slice)
        );
    }

    #[test]
    fn set_b_validators_3() {
        let peers = test_peers(3);
        let topology = Topology::new(peers);

        let empty_peer_slice: &[PeerId] = &[];
        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::set_b_validators),
            Some(empty_peer_slice)
        );
    }

    #[test]
    fn voting_peers_span_full_topology() {
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::voting_peers),
            Some(&peers[..])
        );
    }

    #[test]
    fn validating_peers_empty() {
        let peers = test_peers(2);
        let topology = Topology::new(peers);

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::validating_peers),
            Some::<&[_]>(&[]),
        );
    }

    #[test]
    fn set_b_validators_empty() {
        let peers = test_peers(3);
        let topology = Topology::new(peers);

        assert_eq!(
            topology
                .is_consensus_required()
                .as_ref()
                .map(ConsensusTopology::set_b_validators),
            Some::<&[_]>(&[]),
        );
    }

    #[test]
    fn collectors_k_contiguous_from_tail() {
        // N = 7; min_votes = 5 => tail idx = 4; Set B = 5,6
        let peers = test_peers(7);
        let topology = Topology::new(peers);
        // K=1 -> only tail
        assert_eq!(topology.collector_indices_k(1), vec![4]);
        // K=2 -> tail + first Set B
        assert_eq!(topology.collector_indices_k(2), vec![4, 5]);
        // K larger than available peers after tail is truncated
        assert_eq!(topology.collector_indices_k(10), vec![4, 5, 6]);
    }

    #[test]
    fn collectors_k_excludes_leader_and_no_wrap() {
        // N = 4; min_votes = 3 => tail idx = 2; Set B = 3; leader idx = 0
        let peers = test_peers(4);
        let topology = Topology::new(peers);
        // K=2 would be [2,3]; no wrap to 0
        assert_eq!(topology.collector_indices_k(2), vec![2, 3]);
    }

    #[test]
    fn collectors_fallback_wraps_to_quorum() {
        // N = 4; min_votes = 3 => tail idx = 2; skip leader idx = 0.
        let peers = test_peers(4);
        let topology = Topology::new(peers);
        // K=2 should still fill quorum=3 via wraparound: 2,3,1.
        assert_eq!(topology.collector_indices_k_fallback(2), vec![2, 3, 1]);
    }

    #[test]
    fn collectors_fallback_bumps_small_k_to_quorum() {
        // N = 5; min_votes = 3 => tail idx = 2.
        let peers = test_peers(5);
        let topology = Topology::new(peers);
        // K=1 should still fill quorum=3: 2,3,4.
        assert_eq!(topology.collector_indices_k_fallback(1), vec![2, 3, 4]);
    }

    #[test]
    fn collector_fanout_floor_respects_quorum_and_bounds() {
        let peers = test_peers(4);
        let topology = Topology::new(peers);
        // N = 4 => min_votes = 3, max collectors = 3.
        assert_eq!(topology.collector_fanout_floor(1), 3);
        assert_eq!(topology.collector_fanout_floor(2), 3);
        assert_eq!(topology.collector_fanout_floor(3), 3);
        assert_eq!(topology.collector_fanout_floor(5), 3);

        let peers = test_peers(2);
        let topology = Topology::new(peers);
        // N = 2 => min_votes = 2, max collectors = 1.
        assert_eq!(topology.collector_fanout_floor(1), 1);
        assert_eq!(topology.collector_fanout_floor(2), 1);
    }

    #[test]
    fn topology_fanout_from_tail_wraps_and_skips_leader() {
        // N = 4; min_votes = 3 => tail idx = 2; skip leader idx = 0.
        let peers = test_peers(4);
        let topology = Topology::new(peers);
        assert_eq!(topology.topology_fanout_from_tail(2), vec![2, 3]);
        assert_eq!(topology.topology_fanout_from_tail(3), vec![2, 3, 1]);
    }

    #[test]
    fn redundant_send_r_floor_bumps_to_quorum() {
        // N = 5 => min_votes_for_commit = 3.
        let peers = test_peers(5);
        let topology = Topology::new(peers);
        assert_eq!(topology.redundant_send_r_floor(2), 3);
        assert_eq!(topology.redundant_send_r_floor(6), 6);
    }

    #[test]
    fn single_peer_topology_uses_local_collector() {
        let peers = test_peers(1);
        let topology = Topology::new(peers.clone());
        let seed = [0x55; 32];
        assert_eq!(topology.collector_fanout_floor(1), 1);
        assert_eq!(topology.collector_indices_k(1), vec![0]);
        assert_eq!(topology.collector_indices_k_fallback(1), vec![0]);
        assert_eq!(topology.collector_indices_k_prf(1, seed, 42, 3), vec![0]);
        assert!(topology.is_collector(&peers[0], 1));
    }

    #[test]
    fn is_collector_k3_n7_roles() {
        // N = 7; min_votes = 5 => tail idx = 4; Set B = 5,6
        let peers = test_peers(7);
        let topology = Topology::new(peers.clone());
        // K = 3 should pick [4,5,6]
        assert_eq!(topology.collector_indices_k(3), vec![4, 5, 6]);
        // Roles: leader(0) -> false, validators(1..=3) -> false, tail(4) -> true, Set B(5,6) -> true
        let leader = &peers[0];
        assert!(!topology.is_collector(leader, 3));
        for (idx, peer) in peers.iter().enumerate().take(4).skip(1) {
            assert!(
                !topology.is_collector(peer, 3),
                "validator idx {idx} must not be a collector"
            );
        }
        assert!(
            topology.is_collector(&peers[4], 3),
            "tail must be a collector"
        );
        assert!(
            topology.is_collector(&peers[5], 3),
            "set B idx 5 must be a collector"
        );
        assert!(
            topology.is_collector(&peers[6], 3),
            "set B idx 6 must be a collector"
        );
    }

    #[test]
    fn collectors_k_various_for_n7() {
        let peers = test_peers(7);
        let topology = Topology::new(peers);
        // K = 1 -> only tail
        assert_eq!(topology.collector_indices_k(1), vec![4]);
        // K = 2 -> tail + first Set B
        assert_eq!(topology.collector_indices_k(2), vec![4, 5]);
        // K = 3 -> tail + two Set B
        assert_eq!(topology.collector_indices_k(3), vec![4, 5, 6]);
        // K = 10 -> clamped to available [4,5,6]
        assert_eq!(topology.collector_indices_k(10), vec![4, 5, 6]);
    }

    #[test]
    fn collectors_k3_follow_nth_rotation_for_n7() {
        // For N=7, min_votes=5, tail index = 4; after a left rotation by r, the
        // collector peers at indices [4,5,6] correspond to original peers at
        // [(4+r)%7, (5+r)%7, (6+r)%7].
        let peers = test_peers(7);
        let mut topo = Topology::new(peers.clone());

        // No rotation (r=0)
        let idxs = topo.collector_indices_k(3);
        let cs: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        let expect0 = vec![peers[4].clone(), peers[5].clone(), peers[6].clone()];
        assert_eq!(cs, expect0);

        // r = 1
        topo.nth_rotation(1);
        let idxs = topo.collector_indices_k(3);
        let cs: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        let expect1 = vec![peers[5].clone(), peers[6].clone(), peers[0].clone()];
        assert_eq!(cs, expect1);

        // r = 3 (cumulative view index = 3)
        topo.nth_rotation(3);
        let idxs = topo.collector_indices_k(3);
        let cs: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        let expect3 = vec![
            peers[(4 + 3) % 7].clone(),
            peers[(5 + 3) % 7].clone(),
            peers[(6 + 3) % 7].clone(),
        ];
        assert_eq!(cs, expect3);
    }

    #[test]
    fn collectors_k3_follow_block_committed_membership_refresh() {
        // After a commit, collector indices keep the same canonical ordering.
        let peers = test_peers(7);
        let mut topo = Topology::new(peers.clone());
        let idxs_before = topo.collector_indices_k(3);
        let cs_before: Vec<_> = idxs_before.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(
            cs_before,
            vec![peers[4].clone(), peers[5].clone(), peers[6].clone()]
        );

        let prev_hash = prev_hash_with_seed(2);
        topo.block_committed(peers.clone(), prev_hash);
        let idxs_after = topo.collector_indices_k(3);
        let cs_after: Vec<_> = idxs_after.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs_after, cs_before);
    }

    #[test]
    fn block_committed_preserves_membership_order() {
        let mut peers = test_peers(4);
        peers.reverse();
        let mut topo = Topology::new(peers.clone());
        let prev_hash = prev_hash_with_seed(0);

        topo.block_committed(peers.clone(), prev_hash);

        assert_eq!(topo.as_ref(), peers.as_slice());
    }

    #[test]
    fn canonicalize_order_sorts_without_resetting_view() {
        let mut peers = test_peers(4);
        peers.reverse();
        let mut topo = Topology::new(peers.clone());
        topo.nth_rotation(2);
        let view_index = topo.view_change_index();

        topo.canonicalize_order();

        let mut expected = peers;
        expected.sort();
        expected.dedup();
        assert_eq!(topo.as_ref(), expected.as_slice());
        assert_eq!(topo.view_change_index(), view_index);
    }

    #[test]
    fn collectors_k2_follow_nth_rotation_for_n4() {
        // N=4 -> min_votes=3 -> collector indices [2,3]
        let peers = test_peers(4);
        let mut topo = Topology::new(peers.clone());
        // r=0
        let idxs = topo.collector_indices_k(2);
        let cs0: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs0, vec![peers[2].clone(), peers[3].clone()]);
        // r=1
        topo.nth_rotation(1);
        let idxs = topo.collector_indices_k(2);
        let cs1: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs1, vec![peers[3].clone(), peers[0].clone()]);
        // r=3
        topo.nth_rotation(3);
        let idxs = topo.collector_indices_k(2);
        let cs3: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(
            cs3,
            vec![peers[(2 + 3) % 4].clone(), peers[(3 + 3) % 4].clone()]
        );
    }

    #[test]
    fn collectors_k2_follow_block_committed_canonical_order_n4() {
        // N=4 -> min_votes=3 -> canonicalized ordering only.
        let peers = test_peers(4);
        let mut topo = Topology::new(peers.clone());
        let idxs_before = topo.collector_indices_k(2);
        let cs_before: Vec<_> = idxs_before.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs_before, vec![peers[2].clone(), peers[3].clone()]);
        let prev_hash = prev_hash_with_seed(2);
        topo.block_committed(peers.clone(), prev_hash);
        let idxs_after = topo.collector_indices_k(2);
        let cs_after: Vec<_> = idxs_after.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs_after, cs_before);
    }

    #[test]
    fn collectors_k3_follow_nth_rotation_for_n5() {
        // N=5 -> min_votes=3 -> collector indices [2,3,4]
        let peers = test_peers(5);
        let mut topo = Topology::new(peers.clone());
        // r=0
        let idxs = topo.collector_indices_k(3);
        let cs0: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(
            cs0,
            vec![peers[2].clone(), peers[3].clone(), peers[4].clone()]
        );
        // r=2
        topo.nth_rotation(2);
        let idxs = topo.collector_indices_k(3);
        let cs2: Vec<_> = idxs.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(
            cs2,
            vec![peers[4].clone(), peers[0].clone(), peers[1].clone()]
        );
    }

    #[test]
    fn collectors_k3_follow_block_committed_canonical_order_n5() {
        // N=5 -> min_votes=3 -> canonicalized ordering only.
        let peers = test_peers(5);
        let mut topo = Topology::new(peers.clone());
        let idxs_before = topo.collector_indices_k(3);
        let cs_before: Vec<_> = idxs_before.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(
            cs_before,
            vec![peers[2].clone(), peers[3].clone(), peers[4].clone()]
        );
        let prev_hash = prev_hash_with_seed(2);
        topo.block_committed(peers.clone(), prev_hash);
        let idxs_after = topo.collector_indices_k(3);
        let cs_after: Vec<_> = idxs_after.iter().map(|&i| topo.0[i].clone()).collect();
        assert_eq!(cs_after, cs_before);
    }

    #[test]
    fn no_leader_in_collectors_n4() {
        let peers = test_peers(4);
        let mut topo = Topology::new(peers);
        for r in 0_u64..4 {
            topo.nth_rotation(r);
            for k in 1..=4 {
                let idxs = topo.collector_indices_k(k);
                assert!(
                    !idxs.contains(&0),
                    "leader (idx 0) must not be in collectors for k={k} after r={r}"
                );
            }
        }
    }

    #[test]
    fn no_leader_in_collectors_n5() {
        let peers = test_peers(5);
        let mut topo = Topology::new(peers);
        for r in 0_u64..5 {
            topo.nth_rotation(r);
            for k in 1..=5 {
                let idxs = topo.collector_indices_k(k);
                assert!(
                    !idxs.contains(&0),
                    "leader (idx 0) must not be in collectors for k={k} after r={r}"
                );
            }
        }
    }

    #[test]
    fn no_leader_in_collectors_n7() {
        let peers = test_peers(7);
        let mut topo = Topology::new(peers);
        for r in 0_u64..7 {
            topo.nth_rotation(r);
            for k in 1..=7 {
                let idxs = topo.collector_indices_k(k);
                assert!(
                    !idxs.contains(&0),
                    "leader (idx 0) must not be in collectors for k={k} after r={r}"
                );
            }
        }
    }

    #[test]
    fn nth_rotation_handles_large_view_indices() {
        let peers = test_peers(4);
        let mut topo = Topology::new(peers.clone());
        let large_view = u64::MAX - 2;
        topo.nth_rotation(large_view);

        let mut expected = Topology::new(peers);
        let rotations = large_view % expected.as_ref().len() as u64;
        expected.nth_rotation(rotations);

        assert_eq!(topo.as_ref(), expected.as_ref());
        assert_eq!(topo.view_change_index(), large_view);
    }

    #[test]
    fn rotated_for_prev_block_hash_rotates_set_a() {
        let peers = test_peers(7);
        let mut seed_bytes = [0u8; iroha_crypto::Hash::LENGTH];
        seed_bytes[7] = 1;
        let prev_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed(seed_bytes),
        );

        let rotated = rotated_for_prev_block_hash(peers.clone(), prev_hash);
        let mut expected = Topology::new(peers);
        let rotate_at = expected.min_votes_for_commit();
        expected.0[..rotate_at].rotate_left(1);
        assert_eq!(rotated.0, expected.0);
    }

    #[test]
    fn rotated_for_prev_block_hash_is_deterministic_across_nodes() {
        let keys = (0..5).map(|_| KeyPair::random()).collect::<Vec<_>>();
        let peers = keys.iter().map(|k| PeerId::new(k.public_key().clone()));
        let prev_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x11; iroha_crypto::Hash::LENGTH]),
        );
        let a = rotated_for_prev_block_hash(peers.clone(), prev_hash);
        let b = rotated_for_prev_block_hash(peers, prev_hash);
        assert_eq!(a.0, b.0);
        assert_eq!(a.leader(), b.leader());
        assert_eq!(a.proxy_tail(), b.proxy_tail());
    }

    #[test]
    fn rotated_for_prev_block_hash_is_deterministic_regardless_of_input_order() {
        let keys = (0..5).map(|_| KeyPair::random()).collect::<Vec<_>>();
        let peers: Vec<PeerId> = keys
            .iter()
            .map(|k| PeerId::new(k.public_key().clone()))
            .collect();
        let prev_hash = HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x11; iroha_crypto::Hash::LENGTH]),
        );

        let mut sorted = peers.clone();
        sorted.sort();

        let mut reversed = sorted.clone();
        reversed.reverse();

        let a = rotated_for_prev_block_hash(sorted, prev_hash);
        let b = rotated_for_prev_block_hash(reversed, prev_hash);

        assert_eq!(a.0, b.0);
    }

    #[test]
    fn prf_shuffle_is_deterministic_across_nodes() {
        let keys = (0..5).map(|_| KeyPair::random()).collect::<Vec<_>>();
        let peers = keys.iter().map(|k| PeerId::new(k.public_key().clone()));
        let seed = [0x22; 32];
        let a = shuffled_for_prf_seed(peers.clone(), seed, 42);
        let b = shuffled_for_prf_seed(peers, seed, 42);
        assert_eq!(a.0, b.0);
    }

    #[test]
    fn prf_shuffle_is_deterministic_regardless_of_input_order() {
        let keys = (0..5).map(|_| KeyPair::random()).collect::<Vec<_>>();
        let peers: Vec<PeerId> = keys
            .iter()
            .map(|k| PeerId::new(k.public_key().clone()))
            .collect();
        let seed = [0x33; 32];

        let mut sorted = peers.clone();
        sorted.sort();

        let mut reversed = sorted.clone();
        reversed.reverse();

        let a = shuffled_for_prf_seed(sorted, seed, 9);
        let b = shuffled_for_prf_seed(reversed, seed, 9);

        assert_eq!(a.0, b.0);
    }
}
