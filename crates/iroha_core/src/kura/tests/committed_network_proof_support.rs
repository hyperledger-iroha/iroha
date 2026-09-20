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
        Self::build(target, publish_finality, false)
    }

    /// Inject malformed stored wire below live admission, then sign its exact bytes.
    /// This models hostile persisted state without requiring the live writer to accept it.
    pub(crate) fn with_malformed_target(
        target: impl FnOnce(&SignedBlock) -> SignedBlock,
    ) -> Self {
        Self::build(target, true, true)
    }

    fn build(
        target: impl FnOnce(&SignedBlock) -> SignedBlock,
        publish_finality: bool,
        malformed_target: bool,
    ) -> Self {
        let (root, _config, kura) = kura_root_fixture(nonzero!(4_usize));
        establish_dummy_store_primary_anchor(&kura);
        let parent = DummyBlocks::new().next_with_results();
        let target = Arc::new(target(&parent));
        assert_eq!(target.header().height().get(), 2);
        assert_eq!(target.header().prev_block_hash(), Some(parent.hash()));
        let blocks = vec![parent, target];
        for (index, block) in blocks.iter().enumerate() {
            if malformed_target && index == 1 {
                assert!(
                    kura.store_block(Arc::clone(block)).is_err(),
                    "live admission must reject the malformed output structure"
                );
                {
                    let mut store = kura.block_store.lock();
                    store.append_block_to_chain(block)
                        .expect("inject malformed canonical wire into the physical fixture");
                    store.flush_pending_fsync(true)
                        .expect("retain the malformed fixture's exact durable slot");
                }
                kura.block_data.lock().push((block.hash(), Some(Arc::clone(block))));
                kura.block_height_index.lock().insert(block.hash(), nonzero!(2_usize));
                continue;
            }
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
                if malformed_target && artifact.height == 2 {
                    assert!(
                        kura.store_v2_finality_artifact(artifact).is_err(),
                        "live finality publication must reject malformed execution outputs"
                    );
                    // A cryptographically valid QC can accompany hostile disk
                    // contents. Inject the exact retained evidence below live
                    // publication so the proof reader must validate the body.
                    let target = &blocks[1];
                    let (wire_len, wire_hash) = Kura::canonical_block_wire_identity(target)
                        .expect("malformed fixture still has exact canonical wire");
                    let retained = KuraRetainedBlockRecord::new(
                        target.header(),
                        Kura::canonical_proposal_wire_hash(target)
                            .expect("malformed fixture has exact proposal wire"),
                        wire_len,
                        wire_hash,
                        None,
                        Vec::new(),
                    );
                    let retained_path = kura.retained_block_record_path(2);
                    fs::create_dir_all(retained_path.parent().unwrap()).unwrap();
                    fs::write(retained_path, retained.encode())
                        .expect("inject exact hostile retained-wire metadata");
                    let finality_path = kura.v2_finality_artifact_path(2);
                    fs::create_dir_all(finality_path.parent().unwrap()).unwrap();
                    fs::write(
                        finality_path,
                        KuraV2FinalityRecord::new(target.header(), artifact.clone()).encode(),
                    )
                    .expect("inject genuine exact-wire finality for hostile persisted body");
                } else {
                    let _ = kura
                        .store_v2_finality_artifact(artifact)
                        .expect("publish authentic proof fixture finality");
                }
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
