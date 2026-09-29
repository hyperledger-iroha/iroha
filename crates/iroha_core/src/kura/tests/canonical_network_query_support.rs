/// Default network used only by structural, unauthenticated projection tests.
pub(crate) fn canonical_query_network_id() -> iroha_data_model::NetworkId {
    native_storage_network_id()
}

/// Original native execution and physical custody for canonical query fixtures.
pub(crate) struct CanonicalQueryStore {
    /// Executing State and its authoritative committed hash cut.
    pub(crate) state: Arc<State>,
    /// Actual physical store used by the State query reader.
    pub(crate) kura: Arc<Kura>,
    /// Original native bodies retained independently of query caches.
    pub(crate) blocks: Vec<Arc<SignedBlock>>,
    _chain: crate::sumeragi::test_chain::CertifiedTestChain,
}
impl CanonicalQueryStore {
    /// Retain the chain's original executed blocks and certificates without resigning.
    pub(crate) fn from_chain(chain: crate::sumeragi::test_chain::CertifiedTestChain) -> Self {
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

    /// Extend the same original native chain without altering any retained query prefix.
    pub(crate) fn append_next(&mut self) {
        self._chain.commit(Vec::new());
        self.blocks.push(Arc::clone(
            self._chain.committed(self._chain.height()).block(),
        ));
    }

    /// Open a detached query-permission fixture over the original authenticated history.
    pub(crate) fn reader_state(&self, world: World) -> State {
        let mut state = State::try_new_with_chain_and_network_id(
            crate::state::AllocationBudget::new(
                iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
            ),
            world,
            Arc::clone(&self.kura),
            LiveQueryStore::start_test(),
            self.state.view().chain_id.clone(),
            *self.state.network_id_ref(),
            #[cfg(feature = "telemetry")]
            Default::default(),
        )
        .expect("reader opens original native custody");
        for block in &self.blocks {
            state.push_block_hash_for_testing(block.hash());
        }
        state
    }
    /// Exact expected bytes for the selected complete physical bodies, without reading storage.
    pub(crate) fn wire_bytes(&self, heights: impl IntoIterator<Item = usize>) -> u64 {
        heights
            .into_iter()
            .map(|height| {
                u64::try_from(self.blocks[height - 1].encode_wire().unwrap().len()).unwrap()
            })
            .sum()
    }
    /// Corrupt an actual body byte while preserving its canonical index/header/finality authority.
    pub(crate) fn corrupt_body(&self, height: NonZeroUsize) {
        let mut bytes = self.blocks[height.get() - 1].encode_wire().unwrap();
        let last = bytes.last_mut().expect("canonical wire is nonempty");
        *last ^= 1;
        self.overwrite_body(height, &bytes);
    }
    /// Install same-sized adversarial wire under the original QC and index, without promoting it.
    pub(crate) fn overwrite_body(&self, height: NonZeroUsize, bytes: &[u8]) {
        let index = u64::try_from(height.get() - 1).unwrap();
        let (path, slot) = {
            let mut store = self.kura.block_store.lock();
            (
                store.path_to_blockchain.join(DATA_FILE_NAME),
                store.read_block_index(index).unwrap(),
            )
        };
        assert!(!slot.is_evicted());
        assert_eq!(u64::try_from(bytes.len()).unwrap(), slot.length);
        let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.seek(SeekFrom::Start(slot.start)).unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        // Leave the already authenticated sparse index untouched. The actual
        // canonical reader must reject altered disk bytes even with a warm body.
    }
}
