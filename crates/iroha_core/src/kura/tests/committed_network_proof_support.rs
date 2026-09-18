// Real configured Kura custody for State's committed Network proof tests.
// These fixtures sign exact test wire; they do not claim economic execution.

/// Physical configured store and genuine four-member finality for one proof target.
pub(crate) struct CommittedNetworkProofFixture {
    _root: TempDir,
    /// Store that owns the exact durable bodies and retained finality artifacts.
    pub(crate) kura: Arc<Kura>,
    /// Contiguous genesis and target bodies, retained independently of Kura's cache.
    pub(crate) blocks: Vec<Arc<SignedBlock>>,
    /// Independently selected fixture contexts and exact-wire CommitQCs.
    pub(crate) artifacts: Vec<V2FinalityArtifact>,
}

impl CommittedNetworkProofFixture {
    /// Store a caller-built target after an actual parent and optionally publish its QC chain.
    pub(crate) fn new(
        target: impl FnOnce(&SignedBlock) -> SignedBlock,
        publish_finality: bool,
    ) -> Self {
        let (root, _config, kura) = kura_root_fixture(nonzero!(4_usize));
        establish_dummy_store_primary_anchor(&kura);
        let parent = DummyBlocks::new().next_with_results();
        let target = Arc::new(target(&parent));
        assert_eq!(target.header().height().get(), 2);
        assert_eq!(target.header().prev_block_hash(), Some(parent.hash()));
        let blocks = vec![parent, target];
        for block in &blocks {
            kura.store_block(Arc::clone(block))
                .expect("store exact proof fixture body");
        }
        let artifacts = v2_finality_artifacts_for_chain(&blocks);
        for artifact in &artifacts {
            assert_eq!(artifact.height_context.roster.len(), 4);
            assert_eq!(artifact.commit_qc.signers.len(), 3);
            artifact
                .verify()
                .expect("actual three-of-four BLS and PoP finality");
            if publish_finality {
                let _ = kura
                    .store_v2_finality_artifact(artifact)
                    .expect("publish authentic proof fixture finality");
            }
        }
        Self {
            _root: root,
            kura,
            blocks,
            artifacts,
        }
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
