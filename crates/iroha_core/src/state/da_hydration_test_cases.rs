//! DA hydration atomicity and committed-Kura body authentication regressions.

use super::*;

state_test! { sync block_by_height_reads_committed_kura_body_without_state_view
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let_row! { block = iroha_data_model::block::builder::BlockBuilder::new(header) .build_with_signature(0, keypair.private_key()) };
    let block_hash = block.hash();
    kura.store_block(Arc::new(block))
        .expect("store block in kura");
    assert!(
        state.block_by_height(nonzero!(1_usize)).is_none(),
        "a Kura body outside the committed WSV prefix must stay hidden"
    );
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(block_hash);
        block_hashes.commit_for_tests();
    }
    let_row! { loaded = state .block_by_height(nonzero!(1_usize)) .expect("block should be available") };
    assert_eq!(loaded.hash(), block_hash);
    assert!(state.block_by_height(nonzero!(2_usize)).is_none());
}
state_test! { sync block_by_hash_reads_committed_kura_body_without_state_view
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let_row! { block = iroha_data_model::block::builder::BlockBuilder::new(header) .build_with_signature(0, keypair.private_key()) };
    let block_hash = block.hash();
    kura.store_block(Arc::new(block))
        .expect("store block in kura");
    assert!(state.block_height_by_hash(block_hash).is_none());
    assert!(
        state.block_by_hash(block_hash).is_none(),
        "a Kura body outside the committed WSV prefix must stay hidden"
    );
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(block_hash);
        block_hashes.commit_for_tests();
    }
    assert_eq!(
        state.block_height_by_hash(block_hash),
        Some(nonzero!(1_usize))
    );
    let_row! { loaded = state .block_by_hash(block_hash) .expect("block should be available") };
    assert_eq!(loaded.hash(), block_hash);
    let missing = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new("missing-block"));
    assert!(state.block_by_hash(missing).is_none());
}
state_test! { sync block_query_consumers_reject_kura_body_not_committed_by_wsv
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let_row! { block = iroha_data_model::block::builder::BlockBuilder::new(header) .build_with_signature(0, keypair.private_key()) };
    let actual = block.hash();
    kura.store_block(Arc::new(block))
        .expect("store mismatched Kura body");
    let expected = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA7; 32]));
    assert_ne!(actual, expected);
    {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(expected);
        block_hashes.commit_for_tests();
    }
    assert!(state.block_by_height(nonzero!(1_usize)).is_none());
    assert!(state.block_by_hash(actual).is_none());
    let view = state.view();
    assert!(view.block_by_height(nonzero!(1_usize)).is_none());
    assert_eq!(
        view.block_height_by_hash(expected),
        Some(nonzero!(1_usize)),
        "the WSV journal remains the canonical hash-to-height source"
    );
    assert!(matches!(
        view.canonical_block_by_height(nonzero!(1_usize)),
        Err(CanonicalHistoryError::BlockHashMismatch {
            height: 1,
            expected_hash,
            actual_hash,
        }) if expected_hash == expected && actual_hash == actual
    ));
    assert!(view.latest_block().is_none());
    assert!(matches!(
        view.all_blocks(nonzero!(1_usize)).next(),
        Some(Err(CanonicalHistoryError::BlockHashMismatch { height: 1, .. }))
    ));
}
fn da_index_debug_snapshot(state: &State) -> (String, String, String, String, String) {
    (
        format!("{:?}", state.da_commitments.read()),
        format!("{:?}", state.da_confidential_compute.read()),
        format!("{:?}", state.da_receipt_cursors.read()),
        format!("{:?}", state.da_shard_cursors.read()),
        format!("{:?}", state.da_pin_intents.read()),
    )
}
state_test! { sync failed_da_rewind_on_missing_body_preserves_all_published_indexes
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let first_record = sample_da_commitment_record(LaneId::new(0), 1, 1, 0x31);
    let_row! { first_block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .with_da_commitments(Some(DaCommitmentBundle::new(vec![first_record]))) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    kura.store_block(Arc::new(first_block.clone()))
        .expect("store first block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(first_block.hash());
        hashes.commit_for_tests();
    }
    state
        .ensure_da_indexes_hydrated()
        .expect("hydrate the first committed prefix");
    let sentinel = sample_da_commitment_record(LaneId::new(0), 2, 0, 0x41);
    let_row! { sentinel_location = DaCommitmentLocation { block_height: 77, index_in_bundle: 3 } };
    state
        .da_commitments
        .write()
        .insert(&sentinel, sentinel_location);
    state.da_confidential_compute.write().insert(
        &sentinel,
        sentinel_location,
        &ConfidentialComputePolicy::new(
            ConfidentialComputeMechanism::Encryption,
            NonZeroU32::new(7).expect("non-zero test key version"),
            BTreeSet::new(),
        ),
    );
    state
        .advance_da_receipt_cursors_from_bundle(77, std::slice::from_ref(&sentinel))
        .expect("seed published receipt cursor sentinel");
    state
        .advance_da_shard_cursors_from_bundle(77, std::slice::from_ref(&sentinel))
        .expect("seed published shard cursor sentinel");
    let_row! { mut pin = test_da_pin_intent( *state.network_id_ref(), LaneId::new(0), 2, 1, StorageTicketId::new([0x51; 32]), ManifestDigest::new([0x52; 32]), ) };
    set_test_da_pin_intent_alias(
        &mut pin,
        &ALICE_KEYPAIR,
        Some("atomic-rebuild-sentinel".to_owned()),
    );
    state.da_pin_intents.write().insert(pin, sentinel_location);
    let before = da_index_debug_snapshot(&state);
    let missing_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x61; 32]));
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(missing_hash);
        hashes.commit_for_tests();
    }
    let_row! { error = state .rewind_da_indexes_to_height(2) .expect_err("a non-hash-only committed body must exist") };
    assert!(matches!(
        error,
        DaIndexHydrationError::MissingBlock { height } if height == nonzero!(2_u64)
    ));
    assert_eq!(
        da_index_debug_snapshot(&state),
        before,
        "a failed local rebuild must not publish any of its five partial indexes"
    );
}
state_test! { sync hydrate_da_indexes_rejects_kura_body_hash_mismatch
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let_row! { block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    let actual = block.hash();
    kura.store_block(Arc::new(block)).expect("store mismatched body");
    let expected = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x71; 32]));
    assert_ne!(actual, expected);
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(expected);
        hashes.commit_for_tests();
    }
    let_row! { error = state .ensure_da_indexes_hydrated() .expect_err("Kura body must authenticate against the WSV prefix") };
    assert!(matches!(
        error,
        DaIndexHydrationError::BlockHashMismatch {
            height,
            expected: observed_expected,
            actual: observed_actual,
        } if height == nonzero!(1_u64)
            && observed_expected == expected
            && observed_actual == actual
    ));
}
state_test! { sync hydrate_da_indexes_rejects_cursor_regression
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let_row! { make_record = |sequence: u64| { DaCommitmentRecord::new( LaneId::new(0), 1, sequence, BlobDigest::new([0xAA; 32]), iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xBB; 32]), DaProofScheme::MerkleSha256, Hash::prehashed([0xCC; 32]), None, RetentionClass::default(), StorageTicketId::new([0xEE; 32]), checked_da_ack_signature(0x11), ) } };
    let first_bundle = DaCommitmentBundle::new(vec![make_record(1)]);
    let_row! { first_block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .with_da_commitments(Some(first_bundle)) .sign(keypair.private_key()) .unpack(|_| {}) };
    let signed_first: SignedBlock = first_block.into();
    kura.store_block(Arc::new(signed_first.clone()))
        .expect("store block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(signed_first.hash());
        hashes.commit_for_tests();
    }
    let second_bundle = DaCommitmentBundle::new(vec![make_record(0)]);
    let_row! { second_block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&signed_first)) .with_da_commitments(Some(second_bundle)) .sign(keypair.private_key()) .unpack(|_| {}) };
    let signed_second: SignedBlock = second_block.into();
    kura.store_block(Arc::new(signed_second.clone()))
        .expect("store block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(signed_second.hash());
        hashes.commit_for_tests();
    }
    let_row! { err = state .ensure_da_indexes_hydrated() .expect_err("hydration should fail on regression") };
    match err {
        DaIndexHydrationError::ShardCursor(DaShardCursorError::Regression {
            shard_id,
            lane_id,
            observed_sequence,
            current_sequence,
            ..
        }) => {
            assert_eq!(shard_id, 0);
            assert_eq!(lane_id, LaneId::new(0));
            assert_eq!(observed_sequence, 0);
            assert_eq!(current_sequence, 1);
            let height_str = signed_second.header().height().get().to_string();
            assert!(
                err.to_string().contains(&height_str),
                "expected error message to include height {height_str}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
state_test! { sync hydrate_da_indexes_rejects_receipt_sequence_gap
    let (state, kura) = blank_test_state_with_kura();
    let keypair = crate::state::checked_keypair();
    let_row! { make_record = |sequence, seed| { DaCommitmentRecord::new( LaneId::new(0), 1, sequence, BlobDigest::new([seed; 32]), iroha_data_model::sorafs::pin_registry::ManifestDigest::new([seed.wrapping_add(1); 32]), DaProofScheme::MerkleSha256, Hash::prehashed([seed.wrapping_add(2); 32]), None, RetentionClass::default(), StorageTicketId::new([seed.wrapping_add(3); 32]), checked_da_ack_signature(seed.wrapping_add(4)), ) } };
    let_row! { first_block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .with_da_commitments(Some(DaCommitmentBundle::new(vec![make_record(1, 0x31)]))) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    kura.store_block(Arc::new(first_block.clone()))
        .expect("store first block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(first_block.hash());
        hashes.commit_for_tests();
    }
    let_row! { second_block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&first_block)) .with_da_commitments(Some(DaCommitmentBundle::new(vec![make_record(3, 0x41)]))) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    kura.store_block(Arc::new(second_block.clone()))
        .expect("store second block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(second_block.hash());
        hashes.commit_for_tests();
    }
    let_row! { err = state .ensure_da_indexes_hydrated() .expect_err("hydration should fail on receipt cursor gap") };
    assert!(matches!(
        err,
        DaIndexHydrationError::ReceiptCursor(DaReceiptCursorError::MissingSequence {
            lane,
            epoch: 1,
            expected: 2,
            observed: 3
        }) if lane == LaneId::new(0)
    ));
}
