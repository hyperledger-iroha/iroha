//! Deterministic collector routing utilities.
//!
//! This module centralises the logic used by Sumeragi to choose and
//! advance collector targets across retries. The helpers are public so
//! higher-level crates and integration tests can exercise fairness and
//! backoff behaviour without constructing a full consensus actor.

use iroha_config::parameters::actual::ConsensusMode;
use iroha_data_model::prelude::PeerId;

use super::network_topology::Topology;

/// Ordered collector targets with retry/backoff tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorPlan {
    targets: Vec<PeerId>,
    sent: usize,
    gossip_triggered: bool,
}

impl CollectorPlan {
    /// Create a new plan from a set of collector peer IDs.
    pub fn new(targets: Vec<PeerId>) -> Self {
        Self {
            targets,
            sent: 0,
            gossip_triggered: false,
        }
    }

    /// Return a view of the underlying targets in their planned order.
    pub fn targets(&self) -> &[PeerId] {
        &self.targets
    }

    /// Peek at the next collector without advancing the plan.
    pub fn peek(&self) -> Option<&PeerId> {
        self.targets.get(self.sent)
    }

    /// Number of collectors that have been consumed so far.
    pub fn sent_count(&self) -> usize {
        self.sent
    }

    /// Whether all planned collectors have already been used.
    pub fn exhausted(&self) -> bool {
        self.sent >= self.targets.len()
    }

    /// Mark the gossip fallback as triggered. Returns `true` on the
    /// first call and `false` afterwards so callers can ensure the
    /// fallback path executes at most once per block.
    pub fn trigger_gossip(&mut self) -> bool {
        if self.gossip_triggered {
            false
        } else {
            self.gossip_triggered = true;
            true
        }
    }

    /// Check whether the gossip fallback was already triggered.
    pub fn gossip_triggered(&self) -> bool {
        self.gossip_triggered
    }
}

impl Default for CollectorPlan {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Iterator for CollectorPlan {
    type Item = PeerId;

    /// Pop the next collector target if any remain.
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(peer) = self.targets.get(self.sent) {
            self.sent += 1;
            Some(peer.clone())
        } else {
            None
        }
    }
}

/// Compute the deterministic collector order for a `(height, view)` pair.
///
/// * In permissioned mode we use PRF-based selection keyed by `(seed, height, view)`
///   to randomize collector ordering per height/view.
/// * In `NPoS` mode we reuse the PRF-based selection from `Topology` to
///   derive a pseudo-random but fully deterministic ordering.
///
/// `seed` must be provided for PRF-based selection; callers may pass `None`
/// to fall back to the quorum-sized wraparound slice starting at `proxy_tail_index()`.
pub fn deterministic_collectors(
    topology: &Topology,
    mode: ConsensusMode,
    k: usize,
    seed: Option<[u8; 32]>,
    height: u64,
    view: u64,
) -> Vec<PeerId> {
    let effective_k = topology.collector_fanout_floor(k);
    if effective_k == 0 {
        return Vec::new();
    }
    match mode {
        ConsensusMode::Permissioned => {
            if let Some(seed) = seed {
                let idxs = topology.collector_indices_k_prf(effective_k, seed, height, view);
                return idxs
                    .into_iter()
                    .map(|idx| topology.as_ref()[idx].clone())
                    .collect();
            }
            topology
                .collector_indices_k_fallback(effective_k)
                .into_iter()
                .map(|idx| topology.as_ref()[idx].clone())
                .collect()
        }
        ConsensusMode::Npos => {
            let idxs = seed
                .map(|seed| topology.collector_indices_k_prf(effective_k, seed, height, view))
                .unwrap_or_else(|| topology.collector_indices_k_fallback(effective_k));
            idxs.into_iter()
                .map(|idx| topology.as_ref()[idx].clone())
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;

    use super::*;

    #[test]
    fn plan_advances_and_marks_gossip_once() {
        let peers: Vec<PeerId> = (0..3)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let mut plan = CollectorPlan::new(peers.clone());

        assert_eq!(plan.sent_count(), 0);
        assert_eq!(plan.next(), Some(peers[0].clone()));
        assert_eq!(plan.sent_count(), 1);
        assert_eq!(plan.peek(), Some(&peers[1]));
        assert_eq!(plan.next(), Some(peers[1].clone()));
        assert_eq!(plan.next(), Some(peers[2].clone()));
        assert!(plan.exhausted());
        assert!(plan.next().is_none());
        assert!(plan.trigger_gossip());
        assert!(!plan.trigger_gossip());
        assert!(plan.gossip_triggered());
    }

    #[test]
    fn permissioned_collectors_use_prf_seed() {
        let peers: Vec<PeerId> = (0..5)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let seed = [0x11; 32];
        let plan =
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, Some(seed), 2, 0);
        let effective_k = topology.collector_fanout_floor(2);
        let expected_idxs = topology.collector_indices_k_prf(effective_k, seed, 2, 0);
        let expected: Vec<_> = expected_idxs
            .into_iter()
            .map(|idx| peers[idx].clone())
            .collect();
        assert_eq!(plan, expected);
    }

    #[test]
    fn npos_collectors_depend_on_seed_and_are_deterministic() {
        let peers: Vec<PeerId> = (0..6)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let seed = [0x42; 32];
        let plan1 = deterministic_collectors(&topology, ConsensusMode::Npos, 3, Some(seed), 5, 2);
        let plan2 = deterministic_collectors(&topology, ConsensusMode::Npos, 3, Some(seed), 5, 2);
        assert_eq!(plan1, plan2);
        assert_eq!(plan1.len(), topology.collector_fanout_floor(3));
    }

    #[test]
    fn fallback_collectors_wrap_and_fill_quorum() {
        let peers: Vec<PeerId> = (0..4)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let expected_idxs = topology.collector_indices_k_fallback(2);
        let expected: Vec<_> = expected_idxs
            .into_iter()
            .map(|idx| peers[idx].clone())
            .collect();

        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 1, 0),
            expected
        );
        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Npos, 2, None, 1, 0),
            expected
        );
    }

    #[test]
    fn single_peer_topology_keeps_local_collector() {
        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let topology = Topology::new(vec![peer.clone()]);
        let seed = [0xAB; 32];

        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 1, Some(seed), 7, 0),
            vec![peer.clone()]
        );
        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 1, None, 7, 0),
            vec![peer.clone()]
        );
        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Npos, 1, Some(seed), 7, 0),
            vec![peer.clone()]
        );
        assert_eq!(
            deterministic_collectors(&topology, ConsensusMode::Npos, 1, None, 7, 0),
            vec![peer]
        );
    }
}
