//! The node's lane block stores (`specs/sumeragi_lanes.md` §4.5): one [`FileLaneBlockStore`]
//! per lane incarnation under one root next to Kura, shared by the lane instance that appends
//! to it and by the global executor that merges from it (live, and when replaying Kura).

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iroha_data_model::NetworkId;
use iroha_model_base::topology::LaneId;
use iroha_sumeragi::types::Hash32;
use parking_lot::Mutex;

use super::{
    LaneBatch, incarnation_instance,
    merge::{CommittedLaneBlock, LaneBlockSource},
    store::FileLaneBlockStore,
};
use crate::sumeragi::driver::{SharedCrypto, traits::BlockStore as _};

/// The lane block stores of one node.
pub struct LaneStores {
    root: PathBuf,
    network: NetworkId,
    chain_id: String,
    crypto: SharedCrypto,
    stores: Mutex<BTreeMap<(LaneId, [u8; 32]), Arc<FileLaneBlockStore>>>,
}

impl core::fmt::Debug for LaneStores {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LaneStores")
            .field("root", &self.root)
            .field("open", &self.stores.lock().len())
            .finish_non_exhaustive()
    }
}

impl LaneStores {
    /// The stores under `root` of the lanes of `network` / `chain_id`.
    #[must_use]
    pub fn new(root: PathBuf, network: NetworkId, chain_id: String, crypto: SharedCrypto) -> Self {
        Self {
            root,
            network,
            chain_id,
            crypto,
            stores: Mutex::new(BTreeMap::new()),
        }
    }

    /// The root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The instance id of incarnation `incarnation` of `lane`.
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

    /// The store of an incarnation, opened (and created) on first use.
    ///
    /// # Errors
    /// The store cannot be opened.
    pub fn store(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
    ) -> io::Result<Arc<FileLaneBlockStore>> {
        let mut stores = self.stores.lock();
        if let Some(store) = stores.get(&(lane, *incarnation)) {
            return Ok(Arc::clone(store));
        }
        let instance = self.instance(lane, incarnation);
        let store = Arc::new(FileLaneBlockStore::open(
            &self.root,
            &instance,
            Arc::clone(&self.crypto),
        )?);
        stores.insert((lane, *incarnation), Arc::clone(&store));
        Ok(store)
    }

    /// Stop sharing a retired incarnation's store (its frames stay on disk for replay).
    pub fn release(&self, lane: LaneId, incarnation: &[u8; 32]) {
        self.stores.lock().remove(&(lane, *incarnation));
    }

    fn opened(&self, lane: LaneId, incarnation: &[u8; 32]) -> Option<Arc<FileLaneBlockStore>> {
        match self.store(lane, incarnation) {
            Ok(store) => Some(store),
            Err(error) => {
                iroha_logger::warn!(%lane, %error, "lane store cannot be opened");
                None
            }
        }
    }
}

impl LaneBlockSource for LaneStores {
    fn tip(&self, lane: LaneId, incarnation: &[u8; 32]) -> Option<u64> {
        self.opened(lane, incarnation).map(|store| store.height())
    }

    fn block(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
    ) -> Option<CommittedLaneBlock> {
        let entry = self.opened(lane, incarnation)?.entry(height)?;
        Some(CommittedLaneBlock {
            block_hash: entry.commit_qc.block_hash,
            result: entry.commit_qc.result,
            batch: LaneBatch::from_payload(&entry.block.payload).ok(),
        })
    }

    fn wait_for(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
        timeout: Duration,
    ) -> bool {
        self.opened(lane, incarnation)
            .is_some_and(|store| store.wait_for(height, timeout))
    }
}
