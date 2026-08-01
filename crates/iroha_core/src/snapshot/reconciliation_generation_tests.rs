
    #[self::test]
    async fn ordinary_snapshot_hash_reconcile_rejects_ahead_suffix_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let kura_store_dir = tmp_root.path().join("kura");
        let lane_config = LaneConfig::default();
        let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&kura_config, &lane_config).expect("kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("canonical"));
        let canonical_hash = block.hash();
        store_block_and_mark_state_height(&mut state, &kura, block);
        let extra_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));

        let hashes = vec![canonical_hash, extra_hash];
        let error = reconcile_snapshot_hash_height_with_kura(&hashes, 1, &kura, false, None)
            .expect_err("ordinary signed snapshots cannot invent a hash-only suffix");
        assert!(matches!(error, TryReadError::MismatchedHeight { .. }));

        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.block_hash_at_height(nonzero!(2_usize)), None);
        assert!(
            kura.get_block(nonzero!(2_usize)).is_none(),
            "rejected snapshot must not invent a block body"
        );
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);

        drop(state);
        drop(kura);
        let (reopened, BlockCount(reopened_count)) =
            Kura::new(&kura_config, &lane_config).expect("reopen kura");
        assert_eq!(
            reopened_count, 1,
            "cold restart must not discover a rejected hash-only suffix"
        );
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            reopened.get_block(nonzero!(1_usize)).is_some(),
            "rejected recovery must preserve retained block bodies"
        );
        assert!(
            reopened.get_block(nonzero!(2_usize)).is_none(),
            "rejected suffix must remain absent after restart"
        );
    }

    #[self::test]
    async fn snapshot_hash_reconcile_rejects_forged_prefix_before_extending_suffix() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("canonical"));
        store_block_and_mark_state_height(&mut state, &kura, block);
        let canonical_hash = kura
            .block_hash_at_height(nonzero!(1_usize))
            .expect("canonical Kura prefix hash");
        let forged_prefix =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x91; 32]));
        let attacker_suffix =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x92; 32]));

        SNAPSHOT_HASH_RECONCILIATION_PASSES.with(|passes| passes.set(0));
        let error = reconcile_snapshot_hash_height_with_kura(
            &[forged_prefix, attacker_suffix],
            1,
            &kura,
            false,
            None,
        )
        .expect_err("a divergent retained prefix must reject before suffix extension");

        SNAPSHOT_HASH_RECONCILIATION_PASSES.with(|passes| {
            assert_eq!(
                passes.get(),
                1,
                "forged prefix rejection must complete one fail-before-mutation pass"
            );
        });
        assert!(matches!(
            error,
            TryReadError::MismatchedHash { height: 1, .. }
        ));
        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            kura.block_hash_at_height(nonzero!(1_usize)),
            Some(canonical_hash)
        );
        assert!(
            kura.block_hash_at_height(nonzero!(2_usize)).is_none(),
            "rejected snapshot must not persist its attacker-controlled suffix"
        );
    }

    #[self::test]
    async fn ordinary_signed_snapshot_rejects_kura_tail_loss_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("snapshot");
        let source_kura_store_dir = tmp_root.path().join("source-kura");
        let tail_loss_kura_store_dir = tmp_root.path().join("tail-loss-kura");
        let lane_config = LaneConfig::default();
        let source_kura_config =
            kura_config_for_snapshot_test(&source_kura_store_dir, nonzero!(1_usize));
        let tail_loss_kura_config =
            kura_config_for_snapshot_test(&tail_loss_kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&source_kura_config, &lane_config).expect("source Kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[Arc::clone(&block1), block2],
        );

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot write");
        let pointer_before =
            std::fs::read(snapshot_store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();

        let (tail_loss_kura, BlockCount(initial_height)) =
            Kura::new(&tail_loss_kura_config, &lane_config).expect("tail-loss Kura init");
        assert_eq!(initial_height, 0);
        tail_loss_kura
            .store_block(Arc::clone(&block1))
            .expect("persist retained prefix block");
        let prefix_hash = block1.hash();

        let error = match try_read_snapshot(
            &snapshot_store_dir,
            &tail_loss_kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("ordinary signed snapshot must not repair a lost Kura suffix"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::MismatchedHeight {
                snapshot_height: 2,
                kura_height: 1
            }
        ));
        assert_eq!(tail_loss_kura.blocks_count(), 1);
        assert_eq!(tail_loss_kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            tail_loss_kura.block_hash_at_height(nonzero!(1_usize)),
            Some(prefix_hash)
        );
        assert_eq!(tail_loss_kura.block_hash_at_height(nonzero!(2_usize)), None);
        assert_eq!(
            std::fs::read(snapshot_store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "rejected recovery must not replace the selected snapshot generation"
        );

        drop(tail_loss_kura);
        let (reopened, BlockCount(reopened_count)) =
            Kura::new(&tail_loss_kura_config, &lane_config).expect("cold reopen tail-loss Kura");
        assert_eq!(reopened_count, 1);
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            reopened.block_hash_at_height(nonzero!(1_usize)),
            Some(prefix_hash)
        );
        assert_eq!(reopened.block_hash_at_height(nonzero!(2_usize)), None);
    }

    #[self::test]
    async fn snapshot_read_validates_hashes_without_historical_block_body() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("snapshot");
        let kura_store_dir = tmp_root.path().join("kura");
        let lane_config = LaneConfig::default();
        let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&kura_config, &lane_config).expect("kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));
        let expected_snapshot = canonical_state_snapshot_bytes_for_tests(&state);
        let expected_chain_id = state.chain_id.clone();
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[
                Arc::clone(&block1),
                Arc::clone(&block2),
                Arc::clone(&block3),
            ],
        );

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot write");
        drop(state);
        drop(kura);

        let (kura, block_count) = Kura::new(&kura_config, &lane_config).expect("kura reopen");
        let historical_height = nonzero!(2_usize);
        let payload_len = kura
            .advertise_required_replicas_for_bench(historical_height)
            .expect("historical payload length");
        let freed = kura
            .evict_block_bodies_for_bench(payload_len)
            .expect("evict historical block body");
        assert!(freed >= payload_len);
        let historical_sidecar_path = lane_config
            .primary()
            .blocks_dir(&kura_store_dir)
            .join("da_blocks")
            .join(format!("{:020}.norito", historical_height.get()));
        assert!(
            historical_sidecar_path.is_file(),
            "expected evicted block sidecar at {}",
            historical_sidecar_path.display()
        );
        std::fs::remove_file(&historical_sidecar_path).expect("remove historical sidecar");
        assert!(
            kura.block_hash_at_height(historical_height).is_some(),
            "hash journal must still contain the historical block"
        );
        assert!(
            kura.get_block(historical_height).is_none(),
            "test fixture must make the historical block body unavailable"
        );

        let snapshot_state = try_read_snapshot(
            &snapshot_store_dir,
            &kura,
            LiveQueryStore::start_test,
            block_count,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &expected_chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read should validate historical hashes without block bodies");

        assert_eq!(
            canonical_state_snapshot_bytes_for_tests(&snapshot_state),
            expected_snapshot
        );
    }

    #[self::test]
    async fn snapshot_hash_reconcile_rejects_non_latest_mismatch() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x44; 32]));

        let err = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("non-latest hash mismatch must reject snapshot");
        assert!(matches!(
            err,
            TryReadError::MismatchedHash { height: 2, .. }
        ));
        assert_eq!(state.committed_height(), 3);
    }

    #[self::test]
    async fn snapshot_hash_reconcile_rejects_latest_mismatch_without_mutation() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x55; 32]));

        let state_height_before = state.committed_height();
        let state_hash_before = state.latest_block_hash_fast();
        let kura_height_before = kura.exact_durable_blocks_count().unwrap();
        let error = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("latest hash mismatch must reject instead of trusting snapshot undo state");
        assert!(matches!(
            error,
            TryReadError::MismatchedHash { height: 2, .. }
        ));

        assert_eq!(state.committed_height(), state_height_before);
        assert_eq!(
            state.latest_block_hash_fast(),
            state_hash_before,
            "latest mismatch rejection must leave the snapshot WSV untouched"
        );
        assert_eq!(
            kura.exact_durable_blocks_count().unwrap(),
            kura_height_before,
            "latest mismatch rejection must not prune Kura"
        );
        assert_eq!(state.latest_block_hash_fast(), Some(block2.hash()));
    }

    #[self::test]
    async fn audited_snapshot_hash_reconcile_rejects_every_divergent_existing_hash() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x66; 32]));
        snapshot_hashes[2] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x77; 32]));

        let err = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("audited bootstrap cannot replace any existing Kura hash");
        assert!(matches!(
            err,
            TryReadError::MismatchedHash { height: 2, .. }
        ));
        assert_eq!(state.committed_height(), 3);
    }

    #[self::test]
    async fn snapshot_read_succeeds_without_selector_bootstrap() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let expected_chain_id = state.chain_id.clone();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let snapshot_state = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read");
        assert_eq!(snapshot_state.chain_id, expected_chain_id);
    }

    #[self::test]
    async fn snapshot_generation_shape_is_exact_and_idempotent() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        assert_canonical_snapshot_generation(&store_dir);
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generation_before = current_generation_name(&store_dir);

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("repeating the exact snapshot must be idempotent");

        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_eq!(current_generation_name(&store_dir), generation_before);
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            1,
            "idempotence must not create another immutable generation"
        );
        assert_canonical_snapshot_generation(&store_dir);
    }

    #[self::test]
    async fn snapshot_reader_rejects_every_noncanonical_current_pointer() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
        let canonical = std::fs::read(&pointer_path).unwrap();
        let canonical_text = std::str::from_utf8(&canonical).unwrap();
        let digest = canonical_text.trim_end_matches('\n');
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        for malformed in [
            digest.as_bytes().to_vec(),
            format!("{}\n", digest.to_ascii_uppercase()).into_bytes(),
            b"../foreign\n".to_vec(),
            vec![0xff, b'\n'],
        ] {
            std::fs::write(&pointer_path, malformed).unwrap();
            let error =
                bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
                    .err()
                    .expect("noncanonical current pointer must fail closed");
            assert!(matches!(
                error,
                TryReadError::SnapshotGenerationInvalid { .. }
            ));
        }

        let oversized = format!("{digest}\n\n").into_bytes();
        assert!(
            u64::try_from(oversized.len()).unwrap() > SNAPSHOT_CURRENT_MAX_BYTES,
            "oversized fixture must exercise the pre-parse pointer bound"
        );
        std::fs::write(&pointer_path, oversized).unwrap();
        let error = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .err()
            .expect("oversized current pointer must fail before parsing");
        match error {
            TryReadError::IO(error, path) => {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert_eq!(path, pointer_path);
            }
            other => panic!("unexpected oversized-pointer rejection: {other:?}"),
        }
    }

    #[self::test]
    async fn bound_generation_rejects_pointer_and_artifact_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        let bound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("bind canonical generation");
        let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
        let pointer_bytes = std::fs::read(&pointer_path).unwrap();
        std::fs::remove_file(&pointer_path).unwrap();
        std::fs::write(&pointer_path, &pointer_bytes).unwrap();
        assert!(
            bound.verify_selection_unchanged().is_err(),
            "same-byte pointer substitution must invalidate the bound generation"
        );

        let rebound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("rebind substituted pointer");
        let payload_path = current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME);
        let payload_bytes = std::fs::read(&payload_path).unwrap();
        std::fs::remove_file(&payload_path).unwrap();
        std::fs::write(&payload_path, payload_bytes).unwrap();
        assert!(
            rebound.verify_generation_unchanged().is_err(),
            "same-byte artifact substitution must invalidate the bound generation"
        );
    }

    #[self::test]
    async fn bound_generation_rejects_same_byte_directory_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        let bound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("bind canonical generation");
        let generation_dir = current_generation_dir(&store_dir);
        let displaced_dir = generation_dir.with_extension("displaced");
        std::fs::rename(&generation_dir, &displaced_dir).unwrap();
        std::fs::create_dir(&generation_dir).unwrap();
        for name in [
            SNAPSHOT_FILE_NAME,
            SNAPSHOT_DIGEST_FILE_NAME,
            SNAPSHOT_SIGNATURE_FILE_NAME,
            SNAPSHOT_MERKLE_FILE_NAME,
        ] {
            std::fs::copy(displaced_dir.join(name), generation_dir.join(name)).unwrap();
        }

        assert!(
            bound.verify_generation_unchanged().is_err(),
            "same-byte generation-directory substitution must invalidate every binding"
        );
    }

    #[self::test]
    async fn current_pointer_never_selects_a_partial_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        std::fs::create_dir_all(&generations_dir).unwrap();
        let digest = hex::encode(Sha256::digest(b"partial generation"));
        std::fs::write(
            store_dir.join(SNAPSHOT_CURRENT_FILE_NAME),
            format!("{digest}\n"),
        )
        .unwrap();
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");

        assert!(
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).is_err(),
            "a pointer to a missing generation must fail closed"
        );
        let generation_dir = generations_dir.join(&digest);
        std::fs::create_dir(&generation_dir).unwrap();
        std::fs::write(
            generation_dir.join(SNAPSHOT_FILE_NAME),
            b"partial generation",
        )
        .unwrap();
        assert!(
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).is_err(),
            "a pointer to a partially written generation must fail closed"
        );
    }

    #[self::test]
    async fn conflicting_immutable_generation_cannot_publish_current_pointer() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let payload = exact_snapshot_payload_bytes(&state);
        let digest = hex::encode(Sha256::digest(&payload));
        let conflicting_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME).join(&digest);
        std::fs::create_dir_all(&conflicting_dir).unwrap();
        let conflicting_payload = b"attacker-preplanted-generation";
        std::fs::write(
            conflicting_dir.join(SNAPSHOT_FILE_NAME),
            conflicting_payload,
        )
        .unwrap();

        let error = try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect_err("a conflicting digest-named generation must fail closed");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert!(!store_dir.join(SNAPSHOT_CURRENT_FILE_NAME).exists());
        assert_eq!(
            std::fs::read(conflicting_dir.join(SNAPSHOT_FILE_NAME)).unwrap(),
            conflicting_payload
        );
    }

    #[self::test]
    async fn snapshot_write_reuses_the_exact_immutable_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("initial snapshot write");
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot idempotent publication");
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_canonical_snapshot_generation(&store_dir);
    }

    #[self::test]
    async fn snapshot_writer_enforces_the_reader_payload_limit_before_publication() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let payload_len = exact_snapshot_payload_bytes(&state).len();
        let exact_limit = NonZeroUsize::new(payload_len).expect("snapshot payload is non-empty");

        try_write_snapshot_with_limit(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE, exact_limit)
            .expect("payload exactly at the configured bound must publish");
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generations_before = std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
            .unwrap()
            .count();

        let smaller_limit = NonZeroUsize::new(payload_len - 1).expect("fixture is larger than one");
        let error = try_write_snapshot_with_limit(
            &state,
            &store_dir,
            &key_pair,
            TEST_CHUNK_SIZE,
            smaller_limit,
        )
        .expect_err("payload one byte over the configured bound must reject");
        assert!(matches!(
            error,
            TryWriteError::PayloadTooLarge { actual, maximum }
                if actual == payload_len && maximum == smaller_limit
        ));
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "oversize rejection must not replace the authoritative pointer"
        );
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            generations_before,
            "oversize rejection must not leave a generation or staging orphan"
        );
    }

    #[self::test]
    async fn snapshot_generation_gc_retains_current_and_previous_only() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let first = b"first complete generation";
        let second = b"second complete generation";
        let third = b"third complete generation";
        let first_name = hex::encode(Sha256::digest(first));
        let second_name = hex::encode(Sha256::digest(second));
        let third_name = hex::encode(Sha256::digest(third));

        write_snapshot_bundle_from_bytes(&store_dir, first, &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, second, &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, third, &key_pair);

        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(!generations_dir.join(first_name).exists());
        assert!(generations_dir.join(&second_name).is_dir());
        assert!(generations_dir.join(&third_name).is_dir());
        assert_eq!(current_generation_name(&store_dir), third_name);

        write_snapshot_bundle_from_bytes(&store_dir, third, &key_pair);
        assert!(
            generations_dir.join(second_name).is_dir(),
            "idempotent publication must preserve the prior rollback generation"
        );
        assert_eq!(
            std::fs::read_dir(generations_dir).unwrap().count(),
            2,
            "repeated writes must keep storage bounded"
        );
    }

    #[self::test]
    async fn idempotent_gc_fails_closed_when_rollback_chronology_is_ambiguous() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let current_bytes = b"current generation";
        write_snapshot_bundle_from_bytes(&store_dir, current_bytes, &key_pair);
        let current_name = current_generation_name(&store_dir);
        let (_, first_extra) =
            publish_test_snapshot_generation(&store_dir, b"first extra generation", &key_pair);
        let first_extra_name = first_extra.name.clone();
        let (_, second_extra) =
            publish_test_snapshot_generation(&store_dir, b"second extra generation", &key_pair);
        let second_extra_name = second_extra.name.clone();
        let (store_identity, current) =
            publish_test_snapshot_generation(&store_dir, current_bytes, &key_pair);

        let error = publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &current,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect_err("GC must not invent chronology for multiple authenticated extras");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert_eq!(current_generation_name(&store_dir), current_name);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        for name in [current_name, first_extra_name, second_extra_name] {
            assert!(
                generations_dir.join(name).is_dir(),
                "ambiguous GC must preserve every authenticated generation"
            );
        }
    }

    #[self::test]
    async fn generation_gc_entry_limit_is_enforced_while_enumerating() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let current_bytes = b"bounded GC old current generation";
        let next_bytes = b"bounded GC new current generation";
        write_snapshot_bundle_from_bytes(&store_dir, current_bytes, &key_pair);
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        for index in 0..SNAPSHOT_GENERATION_GC_MAX_ENTRIES - 1 {
            std::fs::write(generations_dir.join(format!("unknown-{index:04}")), b"keep").unwrap();
        }
        let (store_identity, next) =
            publish_test_snapshot_generation(&store_dir, next_bytes, &key_pair);

        let error = publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &next,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect_err("MAX+1 entries must stop bounded GC");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_eq!(
            std::fs::read_dir(generations_dir).unwrap().count(),
            SNAPSHOT_GENERATION_GC_MAX_ENTRIES + 1
        );
    }

    #[self::test]
    async fn post_pointer_gc_failures_report_durable_publication_success() {
        for failure_stage in [1, 2] {
            let tmp_root = tempdir().unwrap();
            let store_dir = tmp_root
                .path()
                .join(format!("snapshot-stage-{failure_stage}"));
            let key_pair = checked_random_snapshot_keypair();
            write_snapshot_bundle_from_bytes(&store_dir, b"generation one", &key_pair);
            write_snapshot_bundle_from_bytes(&store_dir, b"generation two", &key_pair);
            let (store_identity, next) =
                publish_test_snapshot_generation(&store_dir, b"generation three", &key_pair);
            let next_name = next.name.clone();
            SNAPSHOT_GC_FAILURE_STAGE.with(|stage| stage.set(failure_stage));

            publish_snapshot_current_pointer(
                &store_dir,
                store_identity,
                &next,
                defaults::snapshot::MAX_PAYLOAD_BYTES,
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
            )
            .expect("a durable pointer is success even when later maintenance fails");

            assert_eq!(current_generation_name(&store_dir), next_name);
            bind_current_snapshot_generation(
                &store_dir,
                u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap(),
                TEST_CHUNK_SIZE,
            )
            .expect("post-maintenance-error current generation remains complete and readable");
        }
    }

    #[self::test]
    async fn post_pointer_gc_rejects_same_path_generation_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot-stage-substitution");
        let key_pair = checked_random_snapshot_keypair();
        let stale_name = hex::encode(Sha256::digest(b"generation one"));
        write_snapshot_bundle_from_bytes(&store_dir, b"generation one", &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, b"generation two", &key_pair);
        let (store_identity, next) =
            publish_test_snapshot_generation(&store_dir, b"generation three", &key_pair);
        let next_name = next.name.clone();
        SNAPSHOT_GC_FAILURE_STAGE.with(|stage| stage.set(3));

        publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &next,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect("a durable pointer remains successful when GC rejects a substitution");

        assert_eq!(current_generation_name(&store_dir), next_name);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(
            generations_dir.join(&stale_name).is_dir(),
            "the replacement at the captured path must survive"
        );
        assert!(
            generations_dir
                .join(&stale_name)
                .with_extension("gc-displaced")
                .is_dir(),
            "the injected displaced tree must remain available for diagnosis"
        );
        bind_current_snapshot_generation(
            &store_dir,
            u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap(),
            TEST_CHUNK_SIZE,
        )
        .expect("substitution rejection cannot damage the published generation");
    }

    #[self::test]
    async fn snapshot_generation_gc_cleans_safe_orphans_but_preserves_malicious_trees() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, b"canonical generation", &key_pair);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);

        let safe_orphan = generations_dir.join(".snapshot-generation-orphan");
        std::fs::create_dir(&safe_orphan).unwrap();
        std::fs::write(safe_orphan.join(SNAPSHOT_FILE_NAME), b"partial").unwrap();
        let unknown_tree = generations_dir.join("operator-owned");
        std::fs::create_dir(&unknown_tree).unwrap();
        std::fs::write(unknown_tree.join("sentinel"), b"keep").unwrap();
        let invalid_digest_name = hex::encode(Sha256::digest(b"claimed payload"));
        let invalid_digest_tree = generations_dir.join(invalid_digest_name);
        std::fs::create_dir(&invalid_digest_tree).unwrap();
        std::fs::write(invalid_digest_tree.join(SNAPSHOT_FILE_NAME), b"conflict").unwrap();

        let payload_limit = u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap();
        bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("orphan and unknown trees cannot affect current selection");
        write_snapshot_bundle_from_bytes(&store_dir, b"canonical generation", &key_pair);

        assert!(
            !safe_orphan.exists(),
            "safe staging orphan should be reclaimed"
        );
        assert!(unknown_tree.join("sentinel").is_file());
        assert!(
            invalid_digest_tree.join(SNAPSHOT_FILE_NAME).is_file(),
            "invalid digest-named trees are conflicts, never GC repair targets"
        );
    }

    #[self::test]
    async fn concurrent_same_payload_snapshot_writers_publish_one_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = Arc::new(tmp_root.path().join("snapshot"));
        let state = Arc::new(state_factory());
        let key_pair = checked_random_snapshot_keypair();
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for _ in 0..2 {
            let store_dir = Arc::clone(&store_dir);
            let state = Arc::clone(&state);
            let key_pair = key_pair.clone();
            let barrier = Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                try_write_snapshot(&state, store_dir.as_path(), &key_pair, TEST_CHUNK_SIZE)
                    .map_err(|error| error.to_string())
            }));
        }
        barrier.wait();
        for writer in writers {
            writer.join().expect("snapshot writer thread").unwrap();
        }

        assert_canonical_snapshot_generation(&store_dir);
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            1
        );
    }

    #[self::test]
    async fn pointer_switch_does_not_invalidate_a_pinned_immutable_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, b"selected generation", &key_pair);
        let payload_limit = u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap();
        let selected =
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).unwrap();

        write_snapshot_bundle_from_bytes(&store_dir, b"new current generation", &key_pair);
        assert!(selected.verify_selection_unchanged().is_err());
        selected
            .verify_generation_unchanged()
            .expect("post-mutation validation pins the selected immutable generation only");
    }

    #[self::test]
    async fn cannot_find_snapshot_on_read_is_not_found() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let chain_id = ChainId::from(TEST_CHAIN_ID);

        let Err(error) = try_read_snapshot(
            store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::NotFound));
    }

    #[self::test]
    async fn cannot_parse_snapshot_on_read_is_error() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        std::fs::create_dir(&store_dir).unwrap();
        let key_pair = checked_random_snapshot_keypair();
        let chain_id = ChainId::from(TEST_CHAIN_ID);
        let corrupted = [1, 4, 1, 2, 3, 4, 1, 4];
        write_snapshot_bundle_from_bytes(&store_dir, &corrupted, &key_pair);

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert_eq!(format!("{error}"), "Error (de)serializing state snapshot");
    }

    #[self::test]
    async fn checksum_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        // Corrupt the digest without touching the snapshot bytes.
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_DIGEST_FILE_NAME),
            "deadbeef",
        )
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[self::test]
    async fn chain_id_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let expected_chain_id = ChainId::from("other-chain");

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &expected_chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::ChainIdMismatch { .. }));
    }

    #[self::test]
    async fn snapshot_write_rejects_state_ahead_of_kura() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));

        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push(hash);
            block_hashes.commit_for_tests();
        }

        let Err(error) = try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE) else {
            panic!("snapshot write should reject state ahead of Kura");
        };

        assert!(matches!(
            error,
            TryWriteError::StateAheadOfKura {
                state_height: 1,
                kura_height: 0,
            }
        ));
    }

    #[self::test]
    async fn missing_checksum_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(current_generation_artifact(
            &store_dir,
            SNAPSHOT_DIGEST_FILE_NAME,
        ))
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[self::test]
    async fn missing_merkle_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(current_generation_artifact(
            &store_dir,
            SNAPSHOT_MERKLE_FILE_NAME,
        ))
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[self::test]
    async fn merkle_root_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        metadata.root_hex = hex::encode([0xAA; Hash::LENGTH]);
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMismatch { .. }));
    }

    #[self::test]
    async fn merkle_leaf_count_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        assert!(
            metadata.leaf_hashes_hex.pop().is_some(),
            "expected at least one Merkle leaf"
        );
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMetadataMalformed(_)));
    }

    #[self::test]
    async fn merkle_chunk_size_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        metadata.chunk_size_bytes = u64::try_from(TEST_CHUNK_SIZE.get() * 2).expect("fits in u64");
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::MerkleChunkSizeMismatch { .. }
        ));
    }

    #[self::test]
    async fn merkle_metadata_rejects_numeric_string_fields() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut value: norito::json::Value =
            json::from_slice(&std::fs::read(&merkle_path).expect("read merkle"))
                .expect("parse merkle json");
        let map = value.as_object_mut().expect("metadata object");
        map.insert(
            "chunk_size_bytes".to_owned(),
            norito::json::Value::String(TEST_CHUNK_SIZE.get().to_string()),
        );
        let snapshot_len =
            std::fs::metadata(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot metadata")
                .len();
        map.insert(
            "total_len_bytes".to_owned(),
            norito::json::Value::String(snapshot_len.to_string()),
        );
        let mut merkle_file = File::create(&merkle_path).expect("create merkle file");
        json::to_writer(&mut merkle_file, &value).expect("write merkle json");

        let error = SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX)
            .expect_err("numeric-string Merkle fields are not canonical first-release JSON");
        assert!(matches!(error, SnapshotMerkleError::Parse(_)));
    }

    #[self::test]
    async fn merkle_chunk_proof_verifies() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let metadata = SnapshotMerkleMetadata::from_path(
            &current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME),
            u64::MAX,
        )
        .expect("metadata");
        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let chunk = &snapshot_bytes[..snapshot_bytes.len().min(TEST_CHUNK_SIZE.get())];
        metadata
            .verify_chunk(0, chunk)
            .expect("chunk proof should verify");

        let mut corrupted = chunk.to_vec();
        if corrupted.is_empty() {
            corrupted.push(1);
        } else {
            corrupted[0] ^= 0xFF;
        }
        let Err(err) = metadata.verify_chunk(0, &corrupted) else {
            panic!("corrupted chunk should fail verification");
        };
        assert!(matches!(err, SnapshotMerkleError::ProofInvalid { .. }));
    }

    #[self::test]
    async fn can_read_multiple_blocks() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        store_complete_snapshot_commit_evidence_for_blocks(&state, &kura, &[block1, block2]);

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        )
        .unwrap();

        assert_eq!(state.view().height(), 2);
    }

    #[self::test]
    async fn finalized_snapshot_tip_rejects_replacement_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let canonical_tip = block2.hash();
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[Arc::clone(&block1), block2],
        );

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();

        // Once the complete commit tuple authorizes a snapshot, the terminal block is final and
        // cannot be replaced by a same-height soft-fork candidate.
        let replacement = signed_block_after_transaction(
            accepted_log_transaction("soft-fork replacement"),
            Some(block1.as_ref()),
        );
        let replacement_hash = replacement.hash();
        assert_ne!(replacement_hash, canonical_tip);
        let error = kura
            .replace_top_block(replacement)
            .expect_err("checkpointed snapshot tip must reject replacement");
        assert!(matches!(
            error,
            crate::kura::Error::CommittedBlockReplacementForbidden { height: 2 }
        ));
        assert_eq!(
            kura.block_hash_at_height(nonzero!(2_usize)),
            Some(canonical_tip)
        );
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 2);
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "rejected block replacement must not change the selected snapshot generation"
        );

        let restored = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            <_>::default(),
        )
        .unwrap();

        assert_eq!(restored.view().height(), 2);
        assert_eq!(restored.latest_block_hash_fast(), Some(canonical_tip));
    }
