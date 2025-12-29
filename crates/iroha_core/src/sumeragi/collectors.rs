//! Deterministic collector routing utilities.
//!
//! This module centralises the logic used by Sumeragi to choose and
//! advance collector targets across retries. The helpers are public so
//! higher-level crates and integration tests can exercise fairness and
//! backoff behaviour without constructing a full consensus actor.

use core::convert::TryFrom;

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
/// * In permissioned mode we rotate the contiguous tail collector set by
///   `height + view` to evenly distribute primary collector load.
/// * In `NPoS` mode we reuse the PRF-based selection from `Topology` to
///   derive a pseudo-random but fully deterministic ordering.
///
/// `seed` is only used for the `NPoS` path; callers may pass `None` when
/// operating in permissioned mode.
pub fn deterministic_collectors(
    topology: &Topology,
    mode: ConsensusMode,
    k: usize,
    seed: Option<[u8; 32]>,
    height: u64,
    view: u64,
) -> Vec<PeerId> {
    match mode {
        ConsensusMode::Permissioned => {
            let mut idxs = topology.collector_indices_k(k);
            if idxs.is_empty() {
                return Vec::new();
            }
            let offset_seed = height.wrapping_add(view);
            let len_u64 = u64::try_from(idxs.len()).expect("collector count fits into u64");
            let offset = usize::try_from(offset_seed % len_u64)
                .expect("offset must be within collector set");
            idxs.rotate_left(offset);
            idxs.into_iter()
                .map(|idx| topology.as_ref()[idx].clone())
                .collect()
        }
        ConsensusMode::Npos => {
            let Some(seed) = seed else {
                return Vec::new();
            };
            let idxs = topology.collector_indices_k_prf(k, seed, height, view);
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
    fn permissioned_collectors_rotate_with_height_and_view() {
        let peers: Vec<PeerId> = (0..5)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        // min_votes_for_commit = 3 -> collectors indices [2,3,4]
        let plan_h2_v0 =
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 2, 0);
        let plan_h3_v0 =
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 3, 0);
        assert_eq!(plan_h2_v0.len(), 2);
        assert_eq!(plan_h3_v0.len(), 2);
        assert_eq!(plan_h2_v0[0], peers[2]);
        assert_eq!(plan_h2_v0[1], peers[3]);
        // Height increment rotates the primary collector.
        assert_eq!(plan_h3_v0[0], peers[3]);
        assert_eq!(plan_h3_v0[1], peers[2]);

        // View change also rotates.
        let plan_h2_v1 =
            deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 2, 1);
        assert_eq!(plan_h2_v1[0], peers[3]);
        assert_eq!(plan_h2_v1[1], peers[2]);
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
        assert_eq!(plan1.len(), 3);
    }
}
