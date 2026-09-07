// Fixed V1 reputation hashes, byte admission, and durable checkpoint boundaries.

/// Every admitted V1 flag combination, including combined packed sequences and structs.
pub(super) fn supported_layouts() -> [u8; 10] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}

/// Encode an oracle independently of the fixed-frame convenience helpers under test.
pub(super) fn canonical_test_frame<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
    let _layout = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::to_bytes(value).expect("encode independent fixed-layout fixture")
}

#[test]
fn reserve_projection_and_hashes_are_canonical_across_caller_layouts() {
    let page = reserve_provider_page(provider(4));
    let target = ReputationFinalizedIdentityV1 {
        height: TARGET_HEIGHT,
        block_hash: TARGET_HASH,
    };
    let target_bytes = canonical_test_frame(&target);
    let account_bytes = canonical_test_frame(&page.accounts[0]);
    let page_bytes = canonical_test_frame(&page);
    let mut projection_hasher = blake3::Hasher::new();
    projection_hasher.update(b"sorafs-reputation-reserve-projection-v1");
    for bytes in [&target_bytes, &account_bytes] {
        projection_hasher.update(&u64::try_from(bytes.len()).unwrap().to_le_bytes());
        projection_hasher.update(bytes);
    }
    let expected_projection = *projection_hasher.finalize().as_bytes();
    let mut page_hasher = blake3::Hasher::new();
    page_hasher.update(b"reputation-test-page-v1");
    page_hasher.update(&u64::try_from(page_bytes.len()).unwrap().to_le_bytes());
    page_hasher.update(&page_bytes);
    let expected_page_hash = *page_hasher.finalize().as_bytes();
    let mut saw_alternate_frame = false;
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        saw_alternate_frame |= norito::to_bytes(&page).unwrap() != page_bytes;
        let mut accounted_bytes = 0;
        let (stages, digest) = prepare_reserve_projection(
            std::slice::from_ref(&page),
            target,
            FINALIZED_AT_MS,
            &policy(),
            &mut accounted_bytes,
        )
        .expect("validate canonical committed reserve projection");
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].provider_id, provider(4));
        assert_eq!(
            stages[0].stage,
            reserve_stage(ReserveLifecycleStage::Active)
        );
        assert_eq!(digest, expected_projection, "caller layout {flags:#04x}");
        assert_eq!(accounted_bytes, page_bytes.len());
        assert_eq!(
            hash_canonical(b"reputation-test-page-v1", &page).unwrap(),
            expected_page_hash
        );
        let mut changed = page.clone();
        changed.accounts[0].revision += 1;
        assert_ne!(
            prepare_reserve_projection(&[changed], target, FINALIZED_AT_MS, &policy(), &mut 0,)
                .expect("validate distinct committed account revision")
                .1,
            expected_projection,
            "the digest must authenticate account bytes beyond the lifecycle stage"
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(saw_alternate_frame, "exercise a distinct enclosing layout");
}

#[test]
fn page_and_batch_byte_limits_are_canonical_across_caller_layouts() {
    let page = reserve_provider_page(provider(4));
    let page_bytes = canonical_test_frame(&page).len();
    let target = ReputationFinalizedIdentityV1 {
        height: TARGET_HEIGHT,
        block_hash: TARGET_HASH,
    };
    let checkpoint = ReputationIngestCheckpointV1::empty([0xA8; 32]);
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        for (max_page_bytes, max_batch_bytes, accepted_pages) in [
            (page_bytes, page_bytes * 2, 2),
            (page_bytes - 1, page_bytes * 2, 0),
            (page_bytes, page_bytes * 2 - 1, 1),
        ] {
            let mut bounded_policy = policy();
            bounded_policy.checkpoint_max_bytes = u64::try_from(max_batch_bytes).unwrap();
            let mut context =
                PrepareContext::new(&checkpoint, &bounded_policy, target, FINALIZED_AT_MS);
            let mut standalone_bytes = 0;
            for accepted in 0..accepted_pages {
                context
                    .accept_encoded_page(&page, max_page_bytes)
                    .expect("exact canonical page and batch ceiling admits the frame");
                validate_encoded_page(
                    &page,
                    max_page_bytes,
                    &mut standalone_bytes,
                    bounded_policy.checkpoint_max_bytes,
                )
                .expect("reserve accounting agrees with event-page accounting");
                assert_eq!(context.encoded_page_bytes, (accepted + 1) * page_bytes);
                assert_eq!(standalone_bytes, context.encoded_page_bytes);
            }
            assert_eq!(
                context.accept_encoded_page(&page, max_page_bytes),
                Err(ReputationIngestError::CapacityExceeded)
            );
            assert_eq!(
                validate_encoded_page(
                    &page,
                    max_page_bytes,
                    &mut standalone_bytes,
                    bounded_policy.checkpoint_max_bytes,
                ),
                Err(ReputationIngestError::CapacityExceeded)
            );
        }
        let unbounded_policy = policy();
        let mut context =
            PrepareContext::new(&checkpoint, &unbounded_policy, target, FINALIZED_AT_MS);
        context.encoded_page_bytes = usize::MAX;
        let mut standalone_bytes = usize::MAX;
        assert_eq!(
            context.accept_encoded_page(&page, page_bytes),
            Err(ReputationIngestError::CapacityExceeded)
        );
        assert_eq!(
            validate_encoded_page(&page, page_bytes, &mut standalone_bytes, u64::MAX),
            Err(ReputationIngestError::CapacityExceeded)
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}

#[test]
fn replicas_produce_identical_checkpoint_and_unsigned_material_bytes() {
    let left_root = TempDir::new().expect("left root");
    let left = ReputationIngestService::open(left_root.path(), policy()).expect("open left");
    let batch = all_sources_batch(provider(7));
    left.ingest_finalized_batch(batch.clone())
        .expect("ingest left");
    let expected_checkpoint = left.state.lock().unwrap().checkpoint.clone();
    let expected_reserve_digest = expected_checkpoint
        .progress(ReputationSourceV1::Reserve)
        .reserve_projection_digest
        .expect("completed reserve source retains its committed projection digest");
    let expected_bytes = canonical_test_frame(&expected_checkpoint);
    assert_eq!(left.canonical_checkpoint_bytes().unwrap(), expected_bytes);
    let left_material = left
        .unsigned_signing_material()
        .expect("left unsigned material");
    let expected_material_bytes = canonical_test_frame(&left_material);
    let mut saw_alternate_checkpoint = false;
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        let right_root = TempDir::new().expect("right root");
        let right = ReputationIngestService::open(right_root.path(), policy())
            .expect("open replica under caller layout");
        right
            .ingest_finalized_batch(batch.clone())
            .expect("ingest right");
        assert_eq!(right.canonical_checkpoint_bytes().unwrap(), expected_bytes);
        assert_eq!(
            fs::read(
                right_root
                    .path()
                    .join(REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1)
            )
            .expect("read durably committed bytes"),
            expected_bytes
        );
        let right_checkpoint = right.state.lock().unwrap().checkpoint.clone();
        assert_eq!(right_checkpoint, expected_checkpoint);
        assert_eq!(
            right_checkpoint
                .progress(ReputationSourceV1::Reserve)
                .reserve_projection_digest,
            Some(expected_reserve_digest)
        );
        let right_material = right
            .unsigned_signing_material()
            .expect("right unsigned material");
        assert_eq!(left_material, right_material);
        assert_eq!(
            norito::encode_canonical(&right_material).unwrap(),
            expected_material_bytes
        );
        let mut exact_policy = policy();
        exact_policy.checkpoint_max_bytes = u64::try_from(expected_bytes.len()).unwrap();
        ensure_checkpoint_size(&right_checkpoint, &exact_policy).expect("exact ceiling fits");
        exact_policy.checkpoint_max_bytes -= 1;
        assert_eq!(
            ensure_checkpoint_size(&right_checkpoint, &exact_policy),
            Err(ReputationIngestError::CheckpointTooLarge)
        );
        let alternate_bytes = norito::to_bytes(&right_checkpoint).unwrap();
        if alternate_bytes != expected_bytes {
            saw_alternate_checkpoint = true;
            assert_eq!(
                decode_checkpoint(
                    &alternate_bytes,
                    &right.policy,
                    right_checkpoint.policy_digest
                ),
                Err(ReputationIngestError::InvalidCheckpoint),
                "matching ambient flags cannot admit an alternate checkpoint frame"
            );
        }
        drop(right);
        let restored = ReputationIngestService::open(right_root.path(), policy())
            .expect("reload fixed canonical checkpoint under caller layout");
        assert_eq!(
            restored.canonical_checkpoint_bytes().unwrap(),
            expected_bytes
        );
        assert_eq!(restored.unsigned_signing_material().unwrap(), left_material);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(saw_alternate_checkpoint);
}
