// Original native execution and configured custody for State's finalized proof tests.

/// Own one genuine native chain without rebuilding or recertifying its results.
pub(crate) struct CommittedNetworkProofFixture {
    /// Exact production State which executed every retained block.
    pub(crate) state: Arc<State>,
    /// Physical store and execution journal published by the original native driver.
    pub(crate) kura: Arc<Kura>,
    /// Original immutable committed bodies, retained independently of Kura's cache.
    pub(crate) blocks: Vec<Arc<SignedBlock>>,
    _chain: crate::sumeragi::test_chain::CertifiedTestChain,
}

impl CommittedNetworkProofFixture {
    /// Retain the actual executed and certified chain; the proof target is height two.
    pub(crate) fn from_chain(chain: crate::sumeragi::test_chain::CertifiedTestChain) -> Self {
        assert!(chain.height() >= 2);
        let blocks = (1..=chain.height())
            .map(|height| Arc::clone(chain.committed(height).block()))
            .collect();
        Self {
            state: Arc::clone(chain.state()),
            kura: Arc::clone(chain.kura()),
            blocks,
            _chain: chain,
        }
    }

    /// A real nonempty native successor for historical-header component tests.
    pub(crate) fn ordinary() -> Self {
        use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
        chain.commit(Vec::new());
        Self::from_chain(chain)
    }

    /// Inject hostile body bytes while retaining the original authenticated journal.
    /// This grants no new certificate or execution authority to the replacement.
    pub(crate) fn replace_target_wire(&self, bytes: &[u8]) {
        let mut store = self.kura.block_store.lock();
        let slot = store.read_block_index(1).unwrap();
        store.write_block_data(slot.start, bytes).unwrap();
        store
            .write_block_index(1, slot.start, u64::try_from(bytes.len()).unwrap())
            .unwrap();
        store.flush_pending_fsync(true).unwrap();
        drop(store);
        self.make_target_cold();
    }

    /// Target whose exact wire the retained fixture certificate binds.
    pub(crate) fn target(&self) -> &SignedBlock {
        self.blocks[1].as_ref()
    }

    /// Remove derived target custody so a subsequent query must exercise durable reads.
    pub(crate) fn make_target_cold(&self) {
        self.kura.mark_transaction_entrypoint_index_incomplete(2, 2);
        self.kura.block_data.lock()[1].1 = None;
    }

    /// Whether a query unexpectedly published a target body cache entry.
    pub(crate) fn target_cached(&self) -> bool {
        self.kura.block_data.lock().cached_body(1).is_some()
    }

    /// Exact derived-index image for no-promotion assertions.
    pub(crate) fn index_image(&self) -> String {
        format!("{:?}", *self.kura.transaction_entrypoint_index.lock())
    }

    /// Read the physical target slot, independently of canonical body decoding.
    pub(crate) fn target_disk_bytes(&self) -> Vec<u8> {
        let (path, slot) = {
            let mut store = self.kura.block_store.lock();
            (
                store.path_to_blockchain.join(DATA_FILE_NAME),
                store.read_block_index(1).unwrap(),
            )
        };
        let mut file = fs::File::open(path).unwrap();
        file.seek(SeekFrom::Start(slot.start)).unwrap();
        let mut bytes = vec![0; usize::try_from(slot.length).unwrap()];
        file.read_exact(&mut bytes).unwrap();
        bytes
    }

    /// Alter only target body bytes, preserving index, header journal, marker and finality.
    pub(crate) fn overwrite_target_wire(&self, bytes: &[u8]) {
        let (path, slot) = {
            let mut store = self.kura.block_store.lock();
            (
                store.path_to_blockchain.join(DATA_FILE_NAME),
                store.read_block_index(1).unwrap(),
            )
        };
        assert_eq!(u64::try_from(bytes.len()).unwrap(), slot.length);
        let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.seek(SeekFrom::Start(slot.start)).unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        self.make_target_cold();
    }
}
