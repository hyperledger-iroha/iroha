// Portable checkpoint restore exercises complete State against original certified history.

pub(crate) struct TransferredCheckpointFixture {
    pub(crate) directory: tempfile::TempDir,
    pub(crate) source: crate::sumeragi::test_chain::CertifiedTestChain,
    pub(crate) restored: Arc<State>,
    pub(crate) kura: Arc<Kura>,
    pub(crate) kura_config: KuraConfig,
    pub(crate) snapshot_config: Config,
    pub(crate) key: KeyPair,
    pub(crate) original_snapshot: Vec<u8>,
}

pub(crate) fn transferred_checkpoint_fixture() -> TransferredCheckpointFixture {
    transferred_checkpoint_fixture_with_da(false)
}

fn transferred_checkpoint_fixture_with_da(include_da: bool) -> TransferredCheckpointFixture {
    let directory = tempdir().unwrap();
    let snapshot_store = directory.path().join("source-snapshots");
    let mut source = native_snapshot_chain();
    if include_da {
        use iroha_data_model::da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord},
            prelude::{BlobDigest, DaProofScheme, RetentionClass, StorageTicketId},
        };
        let acknowledgement = Signature::new(
            checked_random_snapshot_keypair().private_key(),
            b"portable DA fixture acknowledgement",
        );
        let bundle = DaCommitmentBundle::new(vec![DaCommitmentRecord::new(
            LaneId::SINGLE,
            1,
            1,
            BlobDigest::new([0x81; 32]),
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x82; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x83; 32]),
            None,
            RetentionClass::default(),
            StorageTicketId::new([0x84; 32]),
            acknowledgement,
        )]);
        source.commit_with_proposal(
            None,
            Vec::new(),
            crate::sumeragi::test_chain::Signers::Quorum,
            Default::default(),
            |proposal| proposal.set_da_commitments(Some(bundle)),
        );
    } else {
        source.commit(Vec::new());
    }
    let key = checked_random_snapshot_keypair();
    try_write_snapshot(source.state(), &snapshot_store, &key, TEST_CHUNK_SIZE).unwrap();
    let original_snapshot = canonical_state_snapshot_bytes_for_tests(source.state());
    let name = std::fs::read_to_string(snapshot_store.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
    let committed_ms = source.committed(2).block_time_ms();
    checkpoint_store::keep(
        &snapshot_store,
        &snapshot_store
            .join(SNAPSHOT_GENERATIONS_DIR_NAME)
            .join(name.trim_end()),
        2,
        committed_ms,
        defaults::snapshot::MAX_PAYLOAD_BYTES,
        TEST_CHUNK_SIZE,
    )
    .unwrap();
    source.commit(Vec::new());
    let mut snapshot_config = Config {
        mode: Mode::ReadWrite,
        create_every_ms: defaults::snapshot::CREATE_EVERY.into(),
        store_dir: WithOrigin::inline(snapshot_store),
        merkle_chunk_size_bytes: TEST_CHUNK_SIZE,
        max_payload_bytes: defaults::snapshot::MAX_PAYLOAD_BYTES,
        max_read_buffer_bytes: defaults::snapshot::MAX_READ_BUFFER_BYTES,
        resources: SnapshotResourcePolicy::default(),
        verification_public_key: Some(key.public_key().clone()),
        signing_private_key: None,
        upgrade_checkpoint: iroha_config::parameters::user::SnapshotUpgradeCheckpoint::default(),
    };
    let output = directory.path().join("transfer");
    let mut kura_config = kura_config_for_snapshot_test(&output.join("kura"), nonzero!(1_usize));
    let manifest = transfer::export_checkpoint(
        Arc::clone(source.state()),
        &snapshot_config,
        &kura_config,
        key.public_key(),
        2,
        &output,
    )
    .expect("copy exact retained State and acquire genuine signed prefix rows");
    assert_eq!(manifest.height, 2);
    assert_eq!(manifest.block_hash, source.committed(2).block_hash());
    assert_eq!(
        manifest.published_state_root,
        source
            .committed(3)
            .commitment()
            .execution
            .parent_world_state_root
    );
    let decoded: transfer::CheckpointTransferV1 =
        norito::json::from_slice(&std::fs::read(output.join("checkpoint-transfer.json")).unwrap())
            .unwrap();
    assert_eq!(decoded.block_hash, manifest.block_hash);
    assert_eq!(decoded.snapshot_verification_key, *key.public_key());
    assert!(
        transfer::export_checkpoint(
            Arc::clone(source.state()),
            &snapshot_config,
            &kura_config,
            key.public_key(),
            2,
            &output,
        )
        .is_err(),
        "an occupied transfer cannot be overwritten"
    );
    kura_config.store_dir = WithOrigin::inline(output.join("kura"));
    snapshot_config.store_dir = WithOrigin::inline(output.join("snapshots"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&kura_config, &LaneConfig::default())
            .unwrap();
    let mut restored = strict_snapshot_read_for_custody_test(
        &output.join("snapshots"),
        source.state(),
        &kura,
        &key,
        &snapshot_read_budget_for_testing(),
        &|_| Ok(()),
    )
    .expect("restore H using portable ancestry and actual H+1");
    authenticate_native_snapshot(source.state(), &mut restored);
    assert_eq!(restored.committed_height(), 2);
    assert_eq!(
        canonical_state_snapshot_bytes_for_tests(&restored),
        original_snapshot
    );
    TransferredCheckpointFixture {
        directory,
        source,
        restored: Arc::from(restored),
        kura,
        kura_config,
        snapshot_config,
        key,
        original_snapshot,
    }
}

#[test]
fn portable_transfer_export_retains_original_busy_reader_and_retries() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = transferred_checkpoint_fixture();
            let source = fixture.source.state();
            let writer = source.world.block();
            let error = match crate::sumeragi::checkpoint_prefix::CheckpointPrefixExport::new(
                Arc::clone(source),
                2,
            ) {
                Err(error) => error,
                Ok(_) => panic!("checkpoint export bypassed its original World writer"),
            };
            let crate::execution_attempt::ExecutionAttemptError::Rejected(
                crate::sumeragi::checkpoint_prefix::Error::Storage(error),
            ) = error
            else {
                panic!("physical read refusal lost its original carrier")
            };
            let crate::kura::Error::IO(error, _) = *error else {
                panic!("original read refusal was reclassified")
            };
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            assert!(matches!(
                error
                    .get_ref()
                    .and_then(|error| error.downcast_ref::<crate::state::StateViewError>()),
                Some(crate::state::StateViewError::Busy(_))
            ));
            drop(writer);
            crate::sumeragi::checkpoint_prefix::CheckpointPrefixExport::new(Arc::clone(source), 2)
                .expect("release permits a fresh original-source read");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn portable_transfer_authenticates_complete_state_without_omitted_bodies() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = transferred_checkpoint_fixture();
            assert_eq!(fixture.kura.checkpoint_prefix_height(), Some(2));
            assert!(
                fixture
                    .kura
                    .native_frame_read(2, fixture.source.committed(2).block_hash())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 3);
            assert_eq!(
                fixture.restored.latest_block_hash_fast(),
                Some(fixture.source.committed(2).block_hash())
            );
            let rewritten = fixture.directory.path().join("rewritten-snapshot");
            try_write_snapshot(&fixture.restored, &rewritten, &fixture.key, TEST_CHUNK_SIZE)
                .expect("the exact restored portable tip may retain its original snapshot bytes");
            drop(fixture.restored);
            drop(fixture.kura);
            // Reopen both stores and authenticate again; no successful first restore is authority.
            let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(
                &fixture.kura_config,
                &LaneConfig::default(),
            )
            .unwrap();
            let mut restored = strict_snapshot_read_for_custody_test(
                &fixture.snapshot_config.store_dir.resolve_relative_path(),
                fixture.source.state(),
                &reopened,
                &fixture.key,
                &snapshot_read_budget_for_testing(),
                &|_| Ok(()),
            )
            .unwrap();
            authenticate_native_snapshot(fixture.source.state(), &mut restored);
            assert_eq!(
                canonical_state_snapshot_bytes_for_tests(&restored),
                fixture.original_snapshot
            );
            let mut rewritten_state = strict_snapshot_read_for_custody_test(
                &rewritten,
                fixture.source.state(),
                &reopened,
                &fixture.key,
                &snapshot_read_budget_for_testing(),
                &|_| Ok(()),
            )
            .unwrap();
            authenticate_native_snapshot(fixture.source.state(), &mut rewritten_state);
            assert_eq!(
                canonical_state_snapshot_bytes_for_tests(&rewritten_state),
                fixture.original_snapshot
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}

#[test]
fn portable_transfer_rejects_tampered_checkpoint_and_unavailable_selection() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = transferred_checkpoint_fixture();
            let missing_output = fixture.directory.path().join("missing-checkpoint");
            assert!(
                transfer::export_checkpoint(
                    Arc::clone(fixture.source.state()),
                    &fixture.snapshot_config,
                    &fixture.kura_config,
                    fixture.key.public_key(),
                    3,
                    &missing_output,
                )
                .is_err()
            );
            assert!(!missing_output.exists());
            let foreign_output = fixture.directory.path().join("foreign-signer");
            assert!(
                transfer::export_checkpoint(
                    Arc::clone(fixture.source.state()),
                    &fixture.snapshot_config,
                    &fixture.kura_config,
                    checked_random_snapshot_keypair().public_key(),
                    2,
                    &foreign_output,
                )
                .is_err()
            );
            assert!(!foreign_output.exists());
            let entries = checkpoint_store::entries(
                &fixture.snapshot_config.store_dir.resolve_relative_path(),
            )
            .unwrap();
            let payload = entries[0].2.join(SNAPSHOT_FILE_NAME);
            let original = std::fs::read(&payload).unwrap();
            let mut changed = original.clone();
            let index = changed.len() / 2;
            changed[index] ^= 1;
            std::fs::write(&payload, &changed).unwrap();
            assert!(
                strict_snapshot_read_for_custody_test(
                    &fixture.snapshot_config.store_dir.resolve_relative_path(),
                    fixture.source.state(),
                    &fixture.kura,
                    &fixture.key,
                    &snapshot_read_budget_for_testing(),
                    &|_| Ok(()),
                )
                .is_err(),
                "valid portable finality cannot authorize changed State bytes"
            );
            std::fs::write(&payload, original).unwrap();
            let mut retry = strict_snapshot_read_for_custody_test(
                &fixture.snapshot_config.store_dir.resolve_relative_path(),
                fixture.source.state(),
                &fixture.kura,
                &fixture.key,
                &snapshot_read_budget_for_testing(),
                &|_| Ok(()),
            )
            .unwrap();
            authenticate_native_snapshot(fixture.source.state(), &mut retry);
            assert_eq!(
                canonical_state_snapshot_bytes_for_tests(&retry),
                fixture.original_snapshot
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}

#[test]
fn portable_transfer_rebuilds_original_da_indexes_without_historical_bodies() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let fixture = transferred_checkpoint_fixture_with_da(true);
            let digest = iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x82; 32]);
            let original = fixture
                .source
                .state()
                .find_da_commitment_by_manifest(&digest)
                .unwrap();
            assert!(
                original.is_some(),
                "fixture must carry an actual visible DA commitment"
            );
            assert_eq!(
                fixture
                    .restored
                    .find_da_commitment_by_manifest(&digest)
                    .unwrap(),
                original
            );
            assert_eq!(
                format!("{:?}", fixture.restored.da_receipt_cursors().unwrap()),
                format!("{:?}", fixture.source.state().da_receipt_cursors().unwrap())
            );
            assert_eq!(
                format!("{:?}", fixture.restored.da_shard_cursor_index().unwrap()),
                format!(
                    "{:?}",
                    fixture.source.state().da_shard_cursor_index().unwrap()
                )
            );
            assert!(
                fixture
                    .kura
                    .native_frame_read(2, fixture.source.committed(2).block_hash())
                    .unwrap()
                    .is_none()
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}
