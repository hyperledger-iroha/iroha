    #[test]
    fn kura_init_keeps_blocks_when_commit_manifests_are_missing() {
        let temp_dir = TempDir::new().expect("tempdir");
        let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
        let blocks = {
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
            let blocks = store_dummy_block_arcs(&kura, 2);
            kura.store_commit_manifest(CommitManifest::new(
                1,
                blocks[0].hash(),
                None,
                None,
                Hash::new(b"checkpoint 1"),
                None,
            ))
            .expect("store manifest 1");
            kura.store_wsv_checkpoint(2, blocks[1].hash(), Hash::new(b"stale checkpoint 2"))
                .expect("store stale checkpoint 2");
            blocks
        };

        let (reopened, count) =
            Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen kura");
        assert_eq!(count.0, 2);
        assert_eq!(reopened.blocks_count(), 2);
        assert_eq!(
            reopened.get_durable_block_hash(nonzero!(1_usize)),
            Some(blocks[0].hash())
        );
        assert_eq!(
            reopened.get_durable_block_hash(nonzero!(2_usize)),
            Some(blocks[1].hash())
        );
        assert!(
            reopened
                .commit_manifest(1)
                .expect("read retained manifest")
                .is_some()
        );
        assert!(
            reopened
                .commit_manifest(2)
                .expect("read missing manifest")
                .is_none()
        );
        assert!(
            reopened
                .wsv_checkpoint(2)
                .expect("read retained checkpoint")
                .is_some()
        );
    }

    #[test]
    fn commit_manifest_recovery_accepts_partial_post_commit_sidecar_windows() {
        let temp_dir = TempDir::new().expect("tempdir");
        let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
        let blocks = {
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
            let blocks = store_dummy_block_arcs(&kura, 3);

            // Height 1 models a crash after the block append and before either WSV sidecar.
            // Height 2 models a crash after checkpoint persistence and before manifest write.
            kura.store_wsv_checkpoint(2, blocks[1].hash(), Hash::new(b"checkpoint 2"))
                .expect("store checkpoint 2");
            // Height 3 models checkpoint write failure followed by successful manifest write.
            kura.store_commit_manifest(CommitManifest::new(
                3,
                blocks[2].hash(),
                None,
                None,
                Hash::new(b"checkpoint 3"),
                None,
            ))
            .expect("store manifest 3 without checkpoint");

            blocks
        };

        let (reopened, count) =
            Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen kura");
        assert_eq!(count.0, 3);
        assert_eq!(reopened.blocks_count(), 3);
        for (index, block) in blocks.iter().enumerate() {
            let height = NonZeroUsize::new(index + 1).expect("non-zero height");
            assert_eq!(reopened.get_durable_block_hash(height), Some(block.hash()));
        }
        assert!(
            reopened
                .commit_manifest(1)
                .expect("read missing manifest 1")
                .is_none()
        );
        assert!(
            reopened
                .wsv_checkpoint(1)
                .expect("read missing checkpoint 1")
                .is_none()
        );
        assert!(
            reopened
                .wsv_checkpoint(2)
                .expect("read checkpoint 2")
                .is_some()
        );
        assert!(
            reopened
                .commit_manifest(2)
                .expect("read missing manifest 2")
                .is_none()
        );
        assert!(
            reopened
                .commit_manifest(3)
                .expect("read manifest 3")
                .is_some()
        );
        assert!(
            reopened
                .wsv_checkpoint(3)
                .expect("read missing checkpoint 3")
                .is_none()
        );
    }

    #[test]
    fn prune_to_height_removes_wsv_checkpoints_above_new_tip() {
        let kura = Kura::blank_kura_for_testing();
        let blocks = store_dummy_block_arcs(&kura, 3);
        let retained_hash = Hash::new(b"retained checkpoint");
        let pruned_hash = Hash::new(b"pruned checkpoint");

        kura.store_wsv_checkpoint(2, blocks[1].hash(), retained_hash)
            .expect("store retained checkpoint");
        kura.store_wsv_checkpoint(3, blocks[2].hash(), pruned_hash)
            .expect("store pruned checkpoint");

        kura.prune_to_height(2).expect("prune to height 2");

        let retained = kura
            .wsv_checkpoint(2)
            .expect("read retained checkpoint")
            .expect("retained checkpoint present");
        assert_eq!(retained.state_hash(), retained_hash);
        assert!(
            kura.wsv_checkpoint(3)
                .expect("read pruned checkpoint")
                .is_none()
        );
    }

    #[test]
    fn prune_to_height_removes_commit_manifests_above_new_tip() {
        let kura = Kura::blank_kura_for_testing();
        let blocks = store_dummy_block_arcs(&kura, 3);
        let retained_hash = Hash::new(b"retained manifest checkpoint");
        let pruned_hash = Hash::new(b"pruned manifest checkpoint");

        kura.store_commit_manifest(CommitManifest::new(
            2,
            blocks[1].hash(),
            None,
            None,
            retained_hash,
            None,
        ))
        .expect("store retained manifest");
        kura.store_commit_manifest(CommitManifest::new(
            3,
            blocks[2].hash(),
            None,
            None,
            pruned_hash,
            None,
        ))
        .expect("store pruned manifest");

        kura.prune_to_height(2).expect("prune to height 2");

        let retained = kura
            .commit_manifest(2)
            .expect("read retained manifest")
            .expect("retained manifest present");
        assert_eq!(retained.wsv_checkpoint_hash, retained_hash);
        assert!(
            kura.commit_manifest(3)
                .expect("read pruned manifest")
                .is_none()
        );
    }

    #[test]
    fn replace_top_block_rejects_checkpointed_top_without_mutation() {
        let kura = Kura::blank_kura_for_testing();
        let block = DummyBlocks::new().next();
        let original_hash = block.hash();
        let original_state_hash = Hash::new(b"original checkpoint");
        kura.store_block(Arc::clone(&block)).expect("store block");
        kura.store_wsv_checkpoint(1, original_hash, original_state_hash)
            .expect("store original checkpoint");
        let checkpoint_path = kura.wsv_checkpoint_path(1);
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read checkpoint bytes");
        let durable_wire = {
            let mut store = kura.block_store.lock();
            read_block(&mut store, 0)
                .expect("read durable original")
                .encode_wire()
                .expect("encode durable original")
        };
        kura.replace_top_block(Arc::clone(&block))
            .expect("same-wire retry remains idempotent after checkpoint publication");
        kura.pending_budget_bytes.store(41, Ordering::Release);
        kura.pending_budget_bytes_valid
            .store(true, Ordering::Release);

        let replacement: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into();
        let replacement_hash = replacement.hash();
        assert_ne!(original_hash, replacement_hash);

        assert!(matches!(
            kura.replace_top_block(replacement),
            Err(Error::CommittedBlockReplacementForbidden { height: 1 })
        ));
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(original_hash)
        );
        assert_eq!(
            kura.get_block(nonzero!(1_usize)).as_deref(),
            Some(block.as_ref())
        );
        assert_eq!(
            {
                let mut store = kura.block_store.lock();
                read_block(&mut store, 0)
                    .expect("reread durable original")
                    .encode_wire()
                    .expect("encode durable original after rejection")
            },
            durable_wire
        );
        assert_eq!(
            fs::read(&checkpoint_path).expect("reread checkpoint"),
            checkpoint_bytes
        );
        assert!(!kura.canonical_association_stage_path().exists());
        assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert!(kura.pending_budget_bytes_valid.load(Ordering::Acquire));
        assert_eq!(kura.pending_budget_bytes.load(Ordering::Acquire), 41);
    }

    #[test]
    fn replace_top_block_rejects_manifest_bound_top_without_mutation() {
        let kura = Kura::blank_kura_for_testing();
        let block = DummyBlocks::new().next();
        let original_hash = block.hash();
        let original_state_hash = Hash::new(b"original manifest checkpoint");
        kura.store_block(Arc::clone(&block)).expect("store block");
        kura.store_wsv_checkpoint(1, original_hash, original_state_hash)
            .expect("store original checkpoint");
        let manifest = CommitManifest::new(1, original_hash, None, None, original_state_hash, None);
        kura.store_commit_manifest(manifest.clone())
            .expect("store original commit manifest");
        let checkpoint = kura
            .wsv_checkpoint(1)
            .expect("read bound checkpoint")
            .expect("bound checkpoint exists");
        assert_eq!(
            checkpoint.commit_manifest_hash,
            Some(manifest.encoded_hash())
        );
        let checkpoint_path = kura.wsv_checkpoint_path(1);
        let manifest_path = kura.commit_manifest_path(1);
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read bound checkpoint bytes");
        let manifest_bytes = fs::read(&manifest_path).expect("read manifest bytes");
        let durable_wire = {
            let mut store = kura.block_store.lock();
            read_block(&mut store, 0)
                .expect("read durable original")
                .encode_wire()
                .expect("encode durable original")
        };

        let replacement: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into();
        let replacement_hash = replacement.hash();
        assert_ne!(original_hash, replacement_hash);

        assert!(matches!(
            kura.replace_top_block(replacement),
            Err(Error::CommittedBlockReplacementForbidden { height: 1 })
        ));
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(original_hash)
        );
        assert_eq!(
            kura.get_block(nonzero!(1_usize)).as_deref(),
            Some(block.as_ref())
        );
        assert_eq!(
            {
                let mut store = kura.block_store.lock();
                read_block(&mut store, 0)
                    .expect("reread durable original")
                    .encode_wire()
                    .expect("encode durable original after rejection")
            },
            durable_wire
        );
        assert_eq!(
            fs::read(&checkpoint_path).expect("reread checkpoint"),
            checkpoint_bytes
        );
        assert_eq!(
            fs::read(&manifest_path).expect("reread manifest"),
            manifest_bytes
        );
        assert_eq!(
            kura.commit_manifest(1)
                .expect("reread manifest")
                .expect("manifest remains"),
            manifest
        );
        assert!(!kura.canonical_association_stage_path().exists());
        assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
    }

    #[test]
    fn replace_top_block_replay_metadata_preflight_fails_closed_without_mutation() {
        #[derive(Clone, Copy)]
        enum ReplayMetadataCase {
            ManifestOnly,
            CorruptCheckpoint,
            CorruptManifest,
        }

        for case in [
            ReplayMetadataCase::ManifestOnly,
            ReplayMetadataCase::CorruptCheckpoint,
            ReplayMetadataCase::CorruptManifest,
        ] {
            let kura = Kura::blank_kura_for_testing();
            let block = DummyBlocks::new().next();
            let original_hash = block.hash();
            kura.store_block(Arc::clone(&block)).expect("store block");

            let protected_path = match case {
                ReplayMetadataCase::ManifestOnly => {
                    let manifest = CommitManifest::new(
                        1,
                        original_hash,
                        None,
                        None,
                        Hash::new(b"manifest-only checkpoint hash"),
                        None,
                    );
                    kura.store_commit_manifest(manifest)
                        .expect("store manifest without a WSV checkpoint");
                    assert!(
                        !kura.wsv_checkpoint_path(1).exists(),
                        "manifest-only publication must exercise the manifest preflight branch"
                    );
                    kura.commit_manifest_path(1)
                }
                ReplayMetadataCase::CorruptCheckpoint => {
                    let path = kura.wsv_checkpoint_path(1);
                    fs::create_dir_all(path.parent().expect("checkpoint parent"))
                        .expect("create checkpoint directory");
                    fs::write(&path, b"malformed WSV checkpoint").expect("corrupt checkpoint");
                    path
                }
                ReplayMetadataCase::CorruptManifest => {
                    let path = kura.commit_manifest_path(1);
                    fs::create_dir_all(path.parent().expect("manifest parent"))
                        .expect("create manifest directory");
                    fs::write(&path, b"malformed commit manifest").expect("corrupt manifest");
                    path
                }
            };
            let protected_bytes = fs::read(&protected_path).expect("read protected sidecar");
            kura.pending_budget_bytes.store(73, Ordering::Release);
            kura.pending_budget_bytes_valid
                .store(true, Ordering::Release);

            let replacement: SignedBlock = ValidBlock::new_dummy_and_modify_header(
                checked_keypair().private_key(),
                |header| {
                    header.set_height(nonzero!(1_u64));
                    header.set_prev_block_hash(None);
                    header.set_view_change_index(header.view_change_index().saturating_add(1));
                },
            )
            .into();
            assert_ne!(replacement.hash(), original_hash);

            let error = kura
                .replace_top_block(replacement)
                .expect_err("replay metadata must forbid top replacement");
            match case {
                ReplayMetadataCase::ManifestOnly => assert!(matches!(
                    error,
                    Error::CommittedBlockReplacementForbidden { height: 1 }
                )),
                ReplayMetadataCase::CorruptCheckpoint | ReplayMetadataCase::CorruptManifest => {
                    assert!(matches!(error, Error::NoritoFrame(_)))
                }
            }
            assert_eq!(
                kura.get_durable_block_hash(nonzero!(1_usize)),
                Some(original_hash)
            );
            assert_eq!(
                kura.get_block(nonzero!(1_usize)).as_deref(),
                Some(block.as_ref())
            );
            assert_eq!(
                fs::read(&protected_path).expect("reread protected sidecar"),
                protected_bytes
            );
            assert!(!kura.canonical_association_stage_path().exists());
            assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
            assert!(kura.pending_budget_bytes_valid.load(Ordering::Acquire));
            assert_eq!(kura.pending_budget_bytes.load(Ordering::Acquire), 73);
        }
    }

    #[test]
    fn prune_sidecars_remove_temps_and_fail_closed_on_non_file_suffix() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = new_block_store(&temp_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        store
            .write_da_block_bytes(1, b"retained")
            .expect("write retained sidecar");
        store
            .write_da_block_bytes(3, b"pruned")
            .expect("write pruned sidecar");
        let da_dir = primary_blocks_dir(&temp_dir).join("da_blocks");
        let retained = store.da_block_path(1);
        let pruned = store.da_block_path(3);
        let invalid_height = da_dir.join("not-a-height.norito");
        let temp_artifact = da_dir.join("00000000000000000004.norito.tmp");
        let directory_artifact = da_dir.join("00000000000000000005.norito");
        std::fs::write(&invalid_height, b"operator note").expect("write invalid sidecar name");
        std::fs::write(&temp_artifact, b"partial temp").expect("write temp artifact");
        std::fs::create_dir(&directory_artifact).expect("create directory artifact");

        assert!(matches!(
            store.prune(2),
            Err(Error::PruneIntentConflict(message))
                if message.contains("not removable as a file")
        ));
        std::fs::remove_dir(&directory_artifact).expect("remove blocking directory artifact");
        store.prune(2).expect("retry sidecar prune");

        assert!(
            !retained.exists(),
            "sidecars without a retained canonical index entry must be removed"
        );
        assert!(!pruned.exists(), "above-tip sidecar should be removed");
        assert!(
            invalid_height.exists(),
            "non-height .norito artifacts should be ignored"
        );
        assert!(
            !temp_artifact.exists(),
            "above-tip temporary sidecars must be removed"
        );
    }

    #[test]
    fn fast_init_rewrites_tampered_hash_file() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 3);

        let hash_path = primary_blocks_dir(&temp_dir).join(HASHES_FILE_NAME);
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&hash_path)
                .unwrap();
            // Overwrite the second hash with garbage to simulate tampering.
            file.seek(SeekFrom::Start(SIZE_OF_BLOCK_HASH)).unwrap();
            file.write_all(&[0xAA; Hash::LENGTH]).unwrap();
            file.flush().unwrap();
        }

        let (kura, BlockCount(count)) = Kura::new(
            &Config {
                init_mode: InitMode::Fast,
                store_dir: iroha_config::base::WithOrigin::inline(temp_dir.path().to_path_buf()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
                replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("re-init kura");

        assert_eq!(count, 3);
        let block_hash = kura.get_block_hash(nonzero!(2_usize)).unwrap();
        let block = kura.get_block(nonzero!(2_usize)).unwrap();
        assert_eq!(block_hash, block.hash());
    }

    #[test]
    fn fast_init_prunes_truncated_block_data() {
        let temp_dir = TempDir::new().unwrap();
        populate_store(&temp_dir, 3);

        let data_path = primary_blocks_dir(&temp_dir).join(DATA_FILE_NAME);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&data_path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        file.set_len(len.saturating_sub(4)).unwrap();

        let (kura, BlockCount(count)) = Kura::new(
            &Config {
                init_mode: InitMode::Fast,
                store_dir: iroha_config::base::WithOrigin::inline(temp_dir.path().to_path_buf()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,

                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
                replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            },
            &RuntimeLaneConfig::default(),
        )
        .expect("re-init kura");

        assert_eq!(count, 2);
        assert!(kura.get_block(nonzero!(3_usize)).is_none());

        let mut store = new_block_store(&temp_dir);
        assert_eq!(store.read_index_count().unwrap(), 2);
        assert_eq!(store.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn commit_marker_prunes_excess_entries_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..3 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        store.write_commit_marker(1).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();

        assert_eq!(reopened.read_index_count().unwrap(), 1);
        assert_eq!(reopened.read_hashes_count().unwrap(), 1);
        assert_eq!(reopened.read_durable_index_count().unwrap(), 1);
        let marker = reopened.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 1);

        let last = reopened.read_block_index(0).unwrap();
        let data_len = reopened.data_file_len().unwrap();
        assert_eq!(data_len, last.start + last.length);
    }

    #[test]
    fn commit_marker_truncates_hashes_tail_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let hashes_path = blocks_dir.join(HASHES_FILE_NAME);
        let hashes_file = std::fs::OpenOptions::new()
            .write(true)
            .open(&hashes_path)
            .unwrap();
        hashes_file.set_len(3 * SIZE_OF_BLOCK_HASH).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_index_count().unwrap(), 2);
        assert_eq!(reopened.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn commit_marker_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        store.append_block_to_chain(&blocks.next()).unwrap();
        store.append_block_to_chain(&blocks.next()).unwrap();

        store.write_commit_marker(1).unwrap();
        store.write_commit_marker(2).unwrap();

        let marker = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 2);
        assert!(blocks_dir.join(COUNT_FILE_NAME).exists());
    }

    #[test]
    fn commit_marker_boundary_is_canonical_and_ambient_independent() {
        let temp_dir = TempDir::new().expect("create commit-marker root");
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        let marker = BlockStoreCommitMarker::new(0, None);
        let canonical = norito::encode_canonical(&marker).expect("encode canonical commit marker");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;

        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            store
                .write_commit_marker_value(&marker)
                .expect("write marker under alternate ambient layout");
            assert_ne!(
                norito::to_bytes(&marker).expect("encode ambient marker fixture"),
                canonical,
                "canonical marker encoding must restore the caller's ambient layout"
            );
        }
        let marker_path = store.commit_marker_path();
        assert_eq!(
            std::fs::read(&marker_path).expect("read published commit marker"),
            canonical,
            "durable marker publication must ignore ambient layout"
        );

        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&marker).expect("encode alternate-layout commit marker")
        };
        assert_ne!(alternate, canonical);
        std::fs::write(&marker_path, alternate).expect("replace marker with alternate layout");
        assert!(
            store
                .read_commit_marker()
                .expect("classify alternate-layout marker")
                .is_none(),
            "the durable marker reader must reject alternate layouts"
        );
        assert!(
            !marker_path.exists(),
            "recovery must remove a rejected main marker before reconstruction"
        );
    }

    #[test]
    fn init_rejects_commit_marker_tip_hash_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();
        let mut blocks = DummyBlocks::new();
        store.append_block_to_chain(&blocks.next()).unwrap();
        let mut marker = store.read_commit_marker().unwrap().expect("marker");
        marker.tip_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed([0xA6; 32])));
        let marker_bytes = norito::to_bytes(&marker).expect("encode tampered marker");
        std::fs::write(store.commit_marker_path(), marker_bytes).expect("write tampered marker");
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        assert!(matches!(
            reopened.create_files_if_they_do_not_exist(),
            Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[test]
    fn init_rejects_nonempty_tip_on_empty_commit_marker() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();
        let marker = BlockStoreCommitMarker {
            version: BlockStoreCommitMarker::VERSION,
            count: 0,
            tip_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed([0xA7; 32]))),
        };
        std::fs::write(
            store.commit_marker_path(),
            norito::to_bytes(&marker).expect("encode invalid empty marker"),
        )
        .expect("write invalid empty marker");
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        assert!(matches!(
            reopened.create_files_if_they_do_not_exist(),
            Err(Error::IO(error, _)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[test]
    fn finalized_prefix_preflight_rejects_commit_marker_tip_hash_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();
        let mut blocks = DummyBlocks::new();
        store.append_block_to_chain(&blocks.next()).unwrap();
        let mut marker = store.read_commit_marker().unwrap().expect("marker");
        marker.tip_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed([0xA8; 32])));
        std::fs::write(
            store.commit_marker_path(),
            norito::to_bytes(&marker).expect("encode tampered marker"),
        )
        .expect("write tampered marker");

        assert!(matches!(
            store.preflight_v2_finalized_prefix(1),
            Err(Error::FinalizedV2BlockMutation {
                rewrite_from_height: 1,
                finalized_height: 1,
            })
        ));
    }

    #[test]
    fn commit_marker_corruption_falls_back_to_index_count() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let marker_path = blocks_dir.join(COUNT_FILE_NAME);
        std::fs::write(&marker_path, b"corrupt").unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_durable_index_count().unwrap(), 2);
        let marker = reopened.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 2);
    }

    #[test]
    fn commit_marker_corruption_falls_back_to_data_backed_count() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let first = store.read_block_index(0).unwrap();
        let first_end = first.start + first.length;
        drop(store);

        let data_path = blocks_dir.join(DATA_FILE_NAME);
        let data_file = std::fs::OpenOptions::new()
            .write(true)
            .open(&data_path)
            .unwrap();
        data_file.set_len(first_end).unwrap();

        let marker_path = blocks_dir.join(COUNT_FILE_NAME);
        std::fs::write(&marker_path, b"corrupt").unwrap();

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        assert_eq!(reopened.read_durable_index_count().unwrap(), 1);
        assert_eq!(reopened.read_index_count().unwrap(), 1);
        assert_eq!(reopened.read_hashes_count().unwrap(), 1);

        let last = reopened.read_block_index(0).unwrap();
        let data_len = reopened.data_file_len().unwrap();
        assert_eq!(data_len, last.start + last.length);
    }

    #[test]
    fn index_misalignment_truncates_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let index_path = blocks_dir.join(INDEX_FILE_NAME);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&index_path)
            .unwrap();
        file.write_all(&[0u8; 3]).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        let len = reopened.index_file_len().unwrap();
        assert_eq!(len % BlockIndex::SIZE, 0);
        assert_eq!(reopened.read_index_count().unwrap(), 2);
    }

    #[test]
    fn hashes_misalignment_truncates_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let mut blocks = DummyBlocks::new();
        for _ in 0..2 {
            store.append_block_to_chain(&blocks.next()).unwrap();
        }

        let hashes_path = blocks_dir.join(HASHES_FILE_NAME);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&hashes_path)
            .unwrap();
        file.write_all(&[0u8; 3]).unwrap();
        drop(store);

        let mut reopened = BlockStore::new(&blocks_dir);
        reopened.create_files_if_they_do_not_exist().unwrap();
        let len = reopened.hashes_file_len().unwrap();
        assert_eq!(len % SIZE_OF_BLOCK_HASH, 0);
        assert_eq!(reopened.read_hashes_count().unwrap(), 2);
    }

    #[test]
    fn prune_does_not_advance_commit_marker() {
        let temp_dir = TempDir::new().unwrap();
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store = BlockStore::new(&blocks_dir);
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store.append_block_to_chain(block.as_ref()).unwrap();

        let marker = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker.count, 1);

        store.prune(5).unwrap();

        let marker_after = store.read_commit_marker().unwrap().expect("marker");
        assert_eq!(marker_after.count, 1);
        assert_eq!(store.read_index_count().unwrap(), 1);
    }

    #[test]
    fn batched_fsync_waits_until_interval_elapses() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut store = BlockStore::with_fsync(
            temp_dir.path(),
            FsyncMode::Batched,
            Duration::from_millis(5),
        );
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");

        assert!(
            store.fsync_pending_for_tests(),
            "batched fsync should leave pending work"
        );
        let wait = store.next_fsync_wait().expect("pending fsync deadline");
        assert!(
            wait <= Duration::from_millis(5),
            "expected wait under batching window"
        );

        thread::sleep(Duration::from_millis(6));
        store
            .flush_pending_fsync(false)
            .expect("flush pending fsync succeeds");
        assert!(
            !store.fsync_pending_for_tests(),
            "batched fsync should clear after flush"
        );
    }

    #[test]
    fn fsync_on_flushes_immediately() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut store = BlockStore::with_fsync(temp_dir.path(), FsyncMode::Always, FSYNC_INTERVAL);
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");
        assert!(
            !store.fsync_pending_for_tests(),
            "immediate fsync should clear pending flag"
        );
        assert!(
            store.next_fsync_wait().is_none(),
            "immediate fsync should not schedule a wait"
        );
    }

    #[test]
    fn commit_marker_write_failure_rolls_back_unpublished_append() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store =
            BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, Duration::from_millis(10));
        store.create_files_if_they_do_not_exist().unwrap();

        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block");

        assert!(
            store.commit_marker_pending.is_some(),
            "expected pending commit marker before flush"
        );
        assert!(
            store.fsync_pending_for_tests(),
            "fsync should be pending before flush"
        );

        store
            .fail_next_commit_marker_write
            .store(true, Ordering::Release);

        store
            .flush_pending_fsync(true)
            .expect_err("flush should fail when commit marker temp is a directory");

        assert!(
            store.commit_marker_pending.is_none(),
            "failed marker publication must clear the pending replacement"
        );
        assert!(
            !store.fsync_pending_for_tests(),
            "rolled-back journal work must not be published by a later fsync"
        );
        assert_eq!(store.read_index_count().unwrap(), 0);
        assert_eq!(store.read_hashes_count().unwrap(), 0);
    }

    #[test]
    fn commit_marker_ack_failure_with_new_readback_commits_append() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocks_dir = primary_blocks_dir(&temp_dir);
        let mut store =
            BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, Duration::from_secs(60));
        store.create_files_if_they_do_not_exist().unwrap();
        let block = DummyBlocks::new().next();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append pending block");
        store
            .fail_next_commit_marker_ack_after_persist
            .store(true, Ordering::Release);

        store
            .flush_pending_fsync(true)
            .expect("readable new marker turns acknowledgement failure into committed success");
        assert!(store.commit_marker_pending.is_none());
        assert!(!store.fsync_pending_for_tests());
        assert_eq!(store.read_durable_index_count().unwrap(), 1);
        assert_eq!(
            store
                .read_commit_marker()
                .unwrap()
                .expect("committed marker")
                .tip_hash,
            Some(block.hash())
        );
    }

    #[test]
    fn writer_loop_records_periodic_fsync_failure_without_panic() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.fsync_interval = Duration::from_millis(1);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");

        let block = DummyBlocks::new().next();
        {
            let mut store = kura.block_store.lock();
            store
                .append_block_to_chain(block.as_ref())
                .expect("append block");
            assert!(
                store.fsync_pending_for_tests(),
                "batched append should leave pending fsync work"
            );
        }

        kura.block_store
            .lock()
            .fail_next_commit_marker_write
            .store(true, Ordering::Release);

        let shutdown_signal = ShutdownSignal::new();
        let writer_kura = Arc::clone(&kura);
        let writer = thread::spawn(move || {
            writer_kura.receive_blocks_loop(&shutdown_signal);
        });

        writer.join().expect("writer loop should not panic");
        let fault = kura.writer_fault.lock().clone();
        assert!(
            fault
                .as_deref()
                .is_some_and(|fault| fault.contains("periodic fsync")),
            "writer should record periodic fsync failure, got {fault:?}"
        );
        assert!(
            !kura.block_store.lock().fsync_pending_for_tests(),
            "failed writer fsync must roll back instead of publishing after the caller unwinds"
        );
    }
