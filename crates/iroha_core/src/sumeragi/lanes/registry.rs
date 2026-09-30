//! Retained lane store ownership shared by execution, global merge and historical replay.
//!
//! An opening owns its exclusive disk lock and original funded recovery work until it becomes
//! a ready store. Storage and authority errors remain errors; only an absent committed height
//! is absent. Runtime supplies independently authenticated historical authority.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iroha_allocation::AllocationBudget;
use iroha_data_model::NetworkId;
use iroha_model_base::topology::LaneId;
use iroha_sumeragi::{crypto::AttestationVerifier, types::Hash32};
use parking_lot::Mutex;

use super::{
    LaneBatch, incarnation_instance,
    merge::{CommittedLaneBlock, LaneBlockSource},
    store::{FileLaneBlockStore, LaneStoreOpen},
};
use crate::sumeragi::{
    availability_schedule::AvailabilitySchedule,
    driver::{SharedCrypto, traits::BlockStore as _},
};

/// Independently pinned authority owners for one native lane incarnation.
/// Returned by the node's authenticated State/archive provider, never derived from the artifact
/// being read. The application verifier is additionally pinned to the signed-genesis network.
pub struct LaneStoreAuthority {
    /// Immutable historical schedule for this exact instance.
    pub schedule: Arc<dyn AvailabilitySchedule>,
    /// Full native application verifier pinned to this instance and authenticated network.
    pub verifier: Arc<dyn AttestationVerifier + Send + Sync>,
}

/// Resolve original authenticated schedule and attestation authority for a lane incarnation.
/// Production implementations use signed genesis and certified activation/archive state. They
/// must check the independent requested instance against that authority before returning it.
pub trait LaneStoreAuthorities: Send + Sync {
    /// Return authority pinned to `lane`, `incarnation` and the independently computed `instance`.
    /// `None` means that authority is unresolved; the registry reports `WouldBlock` for it.
    ///
    /// # Errors
    /// Authentication failure, corrupt historical state or I/O.
    fn authority(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        instance: Hash32,
    ) -> io::Result<Option<LaneStoreAuthority>>;
}

enum StoreSlot {
    Opening(LaneStoreOpen),
    Ready(Arc<FileLaneBlockStore>),
}

/// The exclusive lane block store owners of one node.
pub struct LaneStores {
    root: PathBuf,
    network: NetworkId,
    chain_id: String,
    crypto: SharedCrypto,
    budget: AllocationBudget,
    authorities: Arc<dyn LaneStoreAuthorities>,
    stores: Mutex<BTreeMap<(LaneId, [u8; 32]), StoreSlot>>,
}

impl core::fmt::Debug for LaneStores {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LaneStores")
            .field("root", &self.root)
            .field("owned", &self.stores.lock().len())
            .finish_non_exhaustive()
    }
}

impl LaneStores {
    /// Bind the node's lane storage root, network, original State pool and authority owners.
    #[must_use]
    pub fn new(
        root: PathBuf,
        network: NetworkId,
        chain_id: String,
        crypto: SharedCrypto,
        budget: AllocationBudget,
        authorities: Arc<dyn LaneStoreAuthorities>,
    ) -> Self {
        Self {
            root,
            network,
            chain_id,
            crypto,
            budget,
            authorities,
            stores: Mutex::new(BTreeMap::new()),
        }
    }

    /// The root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The independently computed instance id of this exact lane incarnation.
    #[must_use]
    pub fn instance(&self, lane: LaneId, incarnation: &[u8; 32]) -> Hash32 {
        incarnation_instance(
            &*self.crypto,
            &self.network,
            &self.chain_id,
            lane,
            incarnation,
        )
    }

    /// Recover one incarnation or return its already recovered shared owner.
    /// Every failed recovery is retained in `Opening`, including its original exclusive lock,
    /// validated prefix and funded read/decode/restoration owners. The caller schedules retries;
    /// this method does not spin, replace a pending owner or expose an unvalidated tip.
    ///
    /// # Errors
    /// Authentication, corruption, I/O or original-resource refusal. `WouldBlock` is retryable;
    /// callers must not reinterpret any other error as a missing or empty store.
    pub fn store(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
    ) -> io::Result<Arc<FileLaneBlockStore>> {
        let key = (lane, *incarnation);
        let mut stores = self.stores.lock();
        if let Some(StoreSlot::Ready(store)) = stores.get(&key) {
            return Ok(Arc::clone(store));
        }
        let opening = match stores.remove(&key) {
            Some(StoreSlot::Opening(opening)) => opening,
            Some(StoreSlot::Ready(_)) => unreachable!("ready owner returned while lock held"),
            None => {
                let instance = self.instance(lane, incarnation);
                let authority = self
                    .authorities
                    .authority(lane, incarnation, instance)?
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "historical lane authority is unresolved",
                        )
                    })?;
                if authority.schedule.instance() != instance {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "historical lane schedule belongs to another incarnation",
                    ));
                }
                FileLaneBlockStore::begin_open(
                    &self.root,
                    &instance,
                    Arc::clone(&self.crypto),
                    self.budget.clone(),
                    authority.schedule,
                    authority.verifier,
                )?
            }
        };
        match opening.complete() {
            Ok(store) => {
                let store = Arc::new(store);
                stores.insert(key, StoreSlot::Ready(Arc::clone(&store)));
                Ok(store)
            }
            Err((opening, error)) => {
                stores.insert(key, StoreSlot::Opening(opening));
                Err(error)
            }
        }
    }

    /// Explicitly relinquish a retired incarnation's owner, including pending recovery.
    /// Frames remain on disk for replay. Outstanding ready-store Arcs retain their exclusive lock.
    pub fn release(&self, lane: LaneId, incarnation: &[u8; 32]) {
        self.stores.lock().remove(&(lane, *incarnation));
    }
}

impl LaneBlockSource for LaneStores {
    fn tip(&self, lane: LaneId, incarnation: &[u8; 32]) -> io::Result<Option<u64>> {
        self.store(lane, incarnation)
            .map(|store| Some(store.height()))
    }

    fn block(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
    ) -> io::Result<Option<CommittedLaneBlock>> {
        let store = self.store(lane, incarnation)?;
        let Some((body, qc)) = store.committed_body(height)? else {
            return Ok(None);
        };
        Ok(Some(CommittedLaneBlock {
            block_hash: qc.block_hash,
            result: qc.result,
            // A completely authenticated Byzantine payload may not be a lane batch. That is
            // distinct from corruption of its stored signed artifact, which returns Err above.
            batch: LaneBatch::from_payload(body.payload().as_slice()).ok(),
        }))
    }

    fn wait_for(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
        timeout: Duration,
    ) -> io::Result<bool> {
        Ok(self.store(lane, incarnation)?.wait_for(height, timeout))
    }
}

#[cfg(test)]
#[path = "registry/tests.rs"]
mod tests;
