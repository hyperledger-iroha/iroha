state_test! { sync rewind_da_indexes_truncates_to_requested_height
    let (mut state, kura) = blank_test_state_with_kura();
    let catalog = LaneCatalog::new(nonzero!(1_u32), vec![LaneConfig::default()]).expect("catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            lane_catalog: catalog,
            lane_config: lane_config.clone(),
            ..Default::default()
        })
        .expect("apply Nexus catalog before rewind");
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
    let second_bundle = DaCommitmentBundle::new(vec![make_record(2)]);
    let_row! { second_block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&signed_first)) .with_da_commitments(Some(second_bundle)) .sign(keypair.private_key()) .unpack(|_| {}) };
    let signed_second: SignedBlock = second_block.into();
    kura.store_block(Arc::new(signed_second.clone()))
        .expect("store block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(signed_second.hash());
        hashes.commit_for_tests();
    }
    state
        .ensure_da_indexes_hydrated()
        .expect("hydration should succeed");
    {
        let cursors = state.da_shard_cursor_index();
        let_row! { cursor = cursors .get(lane_config.shard_id(LaneId::new(0)), LaneId::new(0)) .expect("cursor present at tip") };
        assert_eq!(cursor.sequence, 2);
        assert_eq!(cursor.last_block_height, 2);
    }
    state
        .rewind_da_indexes_to_height(1)
        .expect("rewind should succeed");
    {
        let cursors = state.da_shard_cursor_index();
        let_row! { cursor = cursors .get(lane_config.shard_id(LaneId::new(0)), LaneId::new(0)) .expect("cursor retained after rewind") };
        assert_eq!(cursor.sequence, 1);
        assert_eq!(cursor.last_block_height, 1);
    }
    assert!(
        state.da_commitments().bundle_at(2).is_none(),
        "reverted height should be dropped from commitment index"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn block_and_revert_rewinds_da_indexes() {
    let (mut state, kura) = blank_test_state_with_kura();
    let lane_count = nonzero!(1_u32);
    let catalog = LaneCatalog::new(lane_count, vec![LaneConfig::default()]).expect("lane catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    state
        .set_nexus(iroha_config::parameters::actual::Nexus {
            lane_catalog: catalog,
            lane_config,
            ..Default::default()
        })
        .expect("apply Nexus catalog for telemetry test");
    let keypair = crate::state::checked_keypair();
    let_row! { make_record = |sequence: u64| { DaCommitmentRecord::new( LaneId::new(0), 1, sequence, BlobDigest::new([0xAA; 32]), iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xBB; 32]), DaProofScheme::MerkleSha256, Hash::prehashed([0xCC; 32]), None, RetentionClass::default(), StorageTicketId::new([0xEE; 32]), checked_da_ack_signature(0x11), ) } };
    let first_bundle = DaCommitmentBundle::new(vec![make_record(1)]);
    let_row! { first_block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .with_da_commitments(Some(first_bundle.clone())) .sign(keypair.private_key()) .unpack(|_| {}) };
    let signed_first: SignedBlock = first_block.into();
    kura.store_block(Arc::new(signed_first.clone()))
        .expect("store first block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(signed_first.hash());
        hashes.commit_for_tests();
    }
    let_row! { pin_intent = iroha_data_model::da::pin_intent::DaPinIntent { lane_id: LaneId::new(0), epoch: 1, sequence: 2, storage_ticket: StorageTicketId::new([0xFF; 32]), manifest_hash: iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xAA; 32]), alias: Some("rewind-alias".to_string()), authorization: crate::da::signed_test_ingest_authorization( *state.network_id_ref(), &keypair, LaneId::new(0), 1, 2, 1, ), } };
    let_row! { pin_bundle = iroha_data_model::da::pin_intent::DaPinIntentBundle::new(vec![pin_intent.clone()]) };
    let second_bundle = DaCommitmentBundle::new(vec![make_record(2)]);
    let_row! { second_block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&signed_first)) .with_da_commitments(Some(second_bundle.clone())) .with_da_pin_intents(Some(pin_bundle)) .sign(keypair.private_key()) .unpack(|_| {}) };
    let signed_second: SignedBlock = second_block.into();
    kura.store_block(Arc::new(signed_second.clone()))
        .expect("store second block");
    {
        let mut hashes = state.block_hashes.block();
        hashes.push(signed_second.hash());
        hashes.commit_for_tests();
    }
    state
        .ensure_da_indexes_hydrated()
        .expect("initial hydration should succeed");
    {
        let cursors = state.da_shard_cursor_index();
        let cursor = cursors
            .get(0, LaneId::new(0))
            .expect("cursor present at height 2");
        assert_eq!(cursor.sequence, 2);
        assert_eq!(cursor.last_block_height, 2);
    }
    {
        let pins = state.da_pin_intents();
        assert!(
            pins.get_by_alias("rewind-alias").is_some(),
            "pin intent should be present before rewind"
        );
    }
    let rollback = state.block_and_revert(signed_second.header());
    drop(rollback);
    {
        let cursors = state.da_shard_cursor_index();
        let cursor = cursors
            .get(0, LaneId::new(0))
            .expect("cursor present after rewind");
        assert_eq!(cursor.sequence, 1);
        assert_eq!(cursor.last_block_height, 1);
    }
    let pins = state.da_pin_intents();
    assert!(
        pins.get_by_alias("rewind-alias").is_none(),
        "pin intent from reverted block should be dropped"
    );
    let commitments = state.da_commitments.read();
    assert!(
        commitments.bundle_at(2).is_none(),
        "tail commitment bundle should be removed on rewind"
    );
}
