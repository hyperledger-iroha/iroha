/// Network selected independently by the actual four-validator finality fixture.
pub(crate) fn canonical_query_network_id() -> iroha_data_model::NetworkId {
    test_network_id(b"kura-v2-finality-test")
}

// Physical canonical query fixtures reuse the genuine configured primary and
// exact-wire three-of-four BLS chain. They do not claim economic execution.

/// Own the durable files and authenticated finality for a caller-built fixture chain.
pub(crate) struct CanonicalQueryStore {
    _root: TempDir,
    /// Actual physical store used by the State query reader.
    pub(crate) kura: Arc<Kura>,
    /// Independently retained exact bodies for assertions and corruption controls.
    pub(crate) blocks: Vec<Arc<SignedBlock>>,
}
impl CanonicalQueryStore {
    /// Persist a contiguous canonical chain and bind every exact executed wire by CommitQC.
    pub(crate) fn new(blocks: Vec<Arc<SignedBlock>>) -> Self {
        assert!(!blocks.is_empty());
        let (root, _, kura) = kura_root_fixture(nonzero!(32_usize));
        // State readers must share the exact configured genesis incarnation,
        // established before the first physical body is persisted.
        let _initial_state = State::new_with_chain_and_network_id_for_testing(
            World::default(), Arc::clone(&kura), LiveQueryStore::start_test(),
            ChainId::from("canonical-query"), canonical_query_network_id(),
        );
        for (index, block) in blocks.iter().enumerate() {
            assert_eq!(
                block.header().height().get(),
                u64::try_from(index + 1).unwrap()
            );
            assert_eq!(
                block.header().prev_block_hash(),
                index.checked_sub(1).map(|i| blocks[i].hash())
            );
            block.validate_output_merkle_cache().unwrap();
            kura.store_block(Arc::clone(block))
                .expect("store exact canonical query body");
        }
        let artifacts =
            persist_v2_finality_chain_through(&kura, NonZeroUsize::new(blocks.len()).unwrap());
        for (block, artifact) in blocks.iter().zip(&artifacts) {
            assert_eq!(
                artifact.height_context.network_id,
                canonical_query_network_id()
            );
            assert_eq!(artifact.height_context.roster.len(), 4);
            assert_eq!(artifact.commit_qc.signers.len(), 3);
            artifact.verify().expect("actual BLS and PoP finality");
            let wire = block.encode_wire().unwrap();
            assert_eq!(
                artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_len,
                u64::try_from(wire.len()).unwrap()
            );
            assert_eq!(
                artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_hash,
                Hash::new(&wire)
            );
        }
        Self {
            _root: root,
            kura,
            blocks,
        }
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

/// Publish a structural query fixture through actual storage and independently signed finality.
/// Kura's blank test constructor owns the temporary directory for its whole lifetime.
pub(crate) fn persist_canonical_query_blocks(kura: &Kura, blocks: &[Arc<SignedBlock>]) {
    establish_dummy_store_primary_anchor(kura);
    for (index, block) in blocks.iter().enumerate() {
        assert_eq!(
            block.header().height().get(),
            u64::try_from(index + 1).unwrap()
        );
        assert_eq!(
            block.header().prev_block_hash(),
            index.checked_sub(1).map(|i| blocks[i].hash())
        );
        block.validate_output_merkle_cache().unwrap();
        kura.store_block(Arc::clone(block)).unwrap();
    }
    if !blocks.is_empty() {
        // One explicit finite epoch covers these structural history fixtures,
        // including the original 100-carrier tests. Epoch-transition tests own
        // next-epoch snapshots separately; all contexts below are actually signed.
        const QUERY_EPOCH_END_HEIGHT: u64 = 1024;
        assert!(u64::try_from(blocks.len()).unwrap() < QUERY_EPOCH_END_HEIGHT);
        let keys = v2_finality_fixture_keys();
        let mut artifacts = Vec::with_capacity(blocks.len());
        for block in blocks {
            let artifact = v2_finality_artifact_for_block_with_keys_and_context_policy(
                block,
                artifacts.last(),
                &keys,
                v2_finality_fixture_execution_commitment(),
                None,
                canonical_query_network_id(),
                0,
                QUERY_EPOCH_END_HEIGHT,
                DataAvailabilityLayout {
                    encoding: PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
            );
            let receipt = kura.store_v2_finality_artifact(&artifact).unwrap();
            assert_v2_commit_receipt_matches_artifact(&receipt, &artifact);
            artifacts.push(artifact);
        }
        for (block, artifact) in blocks.iter().zip(artifacts) {
            assert_eq!(
                artifact.height_context.network_id,
                canonical_query_network_id()
            );
            assert_eq!(artifact.height_context.roster.len(), 4);
            assert_eq!(artifact.commit_qc.signers.len(), 3);
            artifact.verify().unwrap();
            let wire = block.encode_wire().unwrap();
            assert_eq!(
                artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_hash,
                Hash::new(&wire)
            );
            assert_eq!(
                artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_len,
                u64::try_from(wire.len()).unwrap()
            );
        }
    }
}
