//! One node-local lease for the lane signing and scheduling engine.
//!
//! The runner owns this slot for its entire process lifetime. A completed
//! height releases its lease only after the old adapter's durable rollover
//! consumes that adapter, so a Native owner cannot coexist with its signer.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use iroha_data_model::NetworkId;
use iroha_model_base::peer::PeerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneEngineKind {
    Legacy,
    Native,
}

struct LaneEngineSlot {
    network_id: NetworkId,
    local_peer: PeerId,
    occupied: AtomicBool,
}

/// A single Sumeragi worker's exclusive lane-engine source.
pub(crate) struct LaneEngineOwner {
    slot: Arc<LaneEngineSlot>,
}

impl LaneEngineOwner {
    /// Bind the slot to the authenticated process network and local peer.
    pub(crate) fn new(network_id: NetworkId, local_peer: PeerId) -> Self {
        Self {
            slot: Arc::new(LaneEngineSlot {
                network_id,
                local_peer,
                occupied: AtomicBool::new(false),
            }),
        }
    }

    fn claim(&self, kind: LaneEngineKind) -> Result<LaneEngineLease, String> {
        self.slot
            .occupied
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                "another lane signer or scheduler still owns this Sumeragi worker".to_owned()
            })?;
        Ok(LaneEngineLease {
            slot: Arc::clone(&self.slot),
            kind,
        })
    }

    /// Claim the current production adapter through its exact durable rollover.
    pub(crate) fn claim_legacy(&self) -> Result<LaneEngineLease, String> {
        self.claim(LaneEngineKind::Legacy)
    }

    /// Claim the one process-lived Native replacement after legacy retirement.
    pub(crate) fn claim_native(&self) -> Result<LaneEngineLease, String> {
        self.claim(LaneEngineKind::Native)
    }
}

/// Move-only proof that one node-local engine excludes the other.
pub(crate) struct LaneEngineLease {
    slot: Arc<LaneEngineSlot>,
    kind: LaneEngineKind,
}

impl LaneEngineLease {
    /// Confirm exact authority before any signer, worker or storage opens.
    pub(crate) fn matches_legacy(&self, network_id: NetworkId, local_peer: &PeerId) -> bool {
        self.matches(LaneEngineKind::Legacy, network_id, local_peer)
    }

    /// Confirm exact authority before a process-lived Native owner opens.
    pub(crate) fn matches_native(&self, network_id: NetworkId, local_peer: &PeerId) -> bool {
        self.matches(LaneEngineKind::Native, network_id, local_peer)
    }

    fn matches(&self, kind: LaneEngineKind, network_id: NetworkId, local_peer: &PeerId) -> bool {
        self.kind == kind
            && self.slot.network_id == network_id
            && &self.slot.local_peer == local_peer
            && self.slot.occupied.load(Ordering::Acquire)
    }
}

impl Drop for LaneEngineLease {
    fn drop(&mut self) {
        let was_occupied = self.slot.occupied.swap(false, Ordering::AcqRel);
        debug_assert!(
            was_occupied,
            "one exclusive lane-engine lease must own its slot"
        );
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    use super::*;

    fn peer() -> PeerId {
        PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        )
    }

    fn network(byte: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([byte; 32])))
    }

    #[test]
    fn node_local_lease_excludes_both_signers_until_rollover_releases_it() {
        let network_id = network(7);
        let local = peer();
        let owner = LaneEngineOwner::new(network_id, local.clone());
        let independent = LaneEngineOwner::new(network_id, peer());
        let legacy = owner
            .claim_legacy()
            .expect("first engine claims this worker");
        assert!(legacy.matches_legacy(network_id, &local));
        assert!(!legacy.matches_native(network_id, &local));
        assert!(!legacy.matches_legacy(network(8), &local));
        assert!(!legacy.matches_legacy(network_id, &peer()));
        assert!(owner.claim_legacy().is_err());
        assert!(owner.claim_native().is_err());
        assert!(
            independent.claim_native().is_ok(),
            "another node has its own slot"
        );
        drop(legacy);
        let native = owner
            .claim_native()
            .expect("retired height released the slot");
        assert!(native.matches_native(network_id, &local));
        assert!(owner.claim_legacy().is_err());
        drop(native);
        assert!(
            owner.claim_legacy().is_ok(),
            "an explicit native retirement releases it"
        );
    }
}
