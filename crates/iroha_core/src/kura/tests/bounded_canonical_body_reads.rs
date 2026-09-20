// Exact finalized-wire query reads reuse actual durable Kura and four-key finality fixtures.
fn bounded_read_executed_blocks(kura: &Kura) -> Vec<Arc<SignedBlock>> {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    establish_dummy_store_primary_anchor(kura);
    let mut generator = DummyBlocks::new();
    let mut blocks = (0..4)
        .map(|_| generator.next_with_results())
        .collect::<Vec<_>>();
    let mut rejected = blocks[1].as_ref().clone();
    let mut outputs = rejected.execution_outputs().to_vec();
    let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
        panic!("fixture has a Network row")
    };
    row.result = iroha_data_model::transaction::TransactionResult::new(Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("alpha".into()),
        ),
    ));
    install_network_index_test_outputs(&mut rejected, outputs);
    blocks[1] = Arc::new(rejected);
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store executed fixture");
    }
    blocks
}

#[test]
fn bounded_canonical_body_read_requires_exact_precharged_hash_and_length() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(4_usize));
    let blocks = bounded_read_executed_blocks(&kura);
    let height = nonzero!(2_usize);
    let hash = blocks[1].hash();
    let wire_len = u64::try_from(blocks[1].encode_wire().unwrap().len()).unwrap();
    assert!(matches!(
        kura.read_block_body_with_wire_bound(height, hash, wire_len),
        Err(Error::MissingV2FinalityArtifact { height: 2 })
    ));
    finalize_chain_through_for_eviction(&kura, height);
    assert_eq!(
        kura.read_block_body_with_wire_bound(height, hash, wire_len)
            .expect("exact finalized body")
            .as_deref(),
        Some(blocks[1].as_ref())
    );
    for (expected_hash, charged_len) in [
        (blocks[2].hash(), wire_len),
        (hash, wire_len - 1),
        (hash, wire_len + 1),
        (hash, 0),
    ] {
        assert!(matches!(
            kura.read_block_body_with_wire_bound(height, expected_hash, charged_len),
            Err(Error::CanonicalBlockWireMismatch { height: 2 })
        ));
    }
    assert_eq!(
        kura.read_block_body_with_wire_bound(height, hash, wire_len)
            .unwrap()
            .as_deref(),
        Some(blocks[1].as_ref()),
        "refused reservations do not mutate the authentic body"
    );
}

#[test]
fn bounded_canonical_body_read_rejects_same_header_self_consistent_output_substitution() {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(4_usize));
    let blocks = bounded_read_executed_blocks(&kura);
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let canonical = &blocks[1];
    assert_eq!(kura.get_block(height).as_deref(), Some(canonical.as_ref()));
    let mut substituted = canonical.as_ref().clone();
    let mut outputs = substituted.execution_outputs().to_vec();
    let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
        unreachable!()
    };
    row.result = iroha_data_model::transaction::TransactionResult::new(Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("bravo".into()),
        ),
    ));
    install_network_index_test_outputs(&mut substituted, outputs);
    substituted.validate_output_merkle_cache().unwrap();
    assert_eq!(substituted.header(), canonical.header());
    assert_eq!(substituted.hash(), canonical.hash());
    assert_ne!(
        substituted.output_merkle_commitment(),
        canonical.output_merkle_commitment()
    );
    let wire = substituted.encode_wire().unwrap();
    let canonical_wire = canonical.encode_wire().unwrap();
    assert_eq!(wire.len(), canonical_wire.len());
    let (path, slot) = {
        let mut store = kura.block_store.lock();
        (
            store.path_to_blockchain.join(DATA_FILE_NAME),
            store.read_block_index(1).unwrap(),
        )
    };
    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(slot.start)).unwrap();
    file.write_all(&wire).unwrap();
    file.sync_all().unwrap();
    assert!(matches!(
        kura.read_block_body_with_wire_bound(
            height,
            canonical.hash(),
            u64::try_from(canonical_wire.len()).unwrap()
        ),
        Err(Error::CanonicalBlockWireMismatch { height: 2 })
    ));
    assert!(
        kura.block_data.lock().cached_body(1).is_some(),
        "warm decoded state must not override exact durable execution evidence"
    );
}

#[test]
fn bounded_canonical_body_read_refuses_oversize_evicted_replica_before_decode() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = bounded_read_executed_blocks(&kura);
    let height = nonzero!(2_usize);
    let canonical = &blocks[1];
    let wire_len = u64::try_from(canonical.encode_wire().unwrap().len()).unwrap();
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(kura.evict_block_bodies(payload_len).unwrap() >= payload_len);
    let path = {
        let mut store = kura.block_store.lock();
        assert!(store.read_block_index(1).unwrap().is_evicted());
        store.da_block_path(2)
    };
    assert_eq!(
        kura.read_block_body_with_wire_bound(height, canonical.hash(), wire_len)
            .unwrap()
            .as_deref(),
        Some(canonical.as_ref())
    );
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&[0]).unwrap();
    file.sync_all().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), wire_len + 1);
    assert!(
        kura.read_block_body_with_wire_bound(height, canonical.hash(), wire_len)
            .is_err()
    );
    assert_eq!(
        fs::metadata(path).unwrap().len(),
        wire_len + 1,
        "query reads refuse occupied corruption without repairing it"
    );
}

#[test]
fn lazy_inline_body_cannot_promote_missing_finality_or_substituted_outputs() {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(4_usize));
    let blocks = bounded_read_executed_blocks(&kura);
    let height = nonzero!(2_usize);
    kura.mark_transaction_entrypoint_index_incomplete(2, 4);
    kura.block_data.lock()[1].1 = None;
    assert_eq!(kura.get_block(height).as_deref(), Some(blocks[1].as_ref()));
    assert!(
        kura.transaction_entrypoint_index
            .lock()
            .incomplete_heights
            .contains(&height)
    );
    assert!(kura.block_data.lock().cached_body(1).is_none());
    finalize_chain_through_for_eviction(&kura, height);
    assert_eq!(kura.get_block(height).as_deref(), Some(blocks[1].as_ref()));
    assert!(
        kura.transaction_entrypoint_index
            .lock()
            .indexed_heights
            .contains(&height)
    );
    assert!(kura.block_data.lock().cached_body(1).is_some());

    let mut substituted = blocks[1].as_ref().clone();
    let mut outputs = substituted.execution_outputs().to_vec();
    let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
        unreachable!()
    };
    row.result = iroha_data_model::transaction::TransactionResult::new(Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("bravo".into()),
        ),
    ));
    install_network_index_test_outputs(&mut substituted, outputs);
    assert_eq!(substituted.header(), blocks[1].header());
    let wire = substituted.encode_wire().unwrap();
    assert_eq!(wire.len(), blocks[1].encode_wire().unwrap().len());
    let (path, slot) = {
        let mut store = kura.block_store.lock();
        (
            store.path_to_blockchain.join(DATA_FILE_NAME),
            store.read_block_index(1).unwrap(),
        )
    };
    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(slot.start)).unwrap();
    file.write_all(&wire).unwrap();
    file.sync_all().unwrap();
    kura.mark_transaction_entrypoint_index_incomplete(2, 4);
    kura.block_data.lock()[1].1 = None;
    // Force the tampered bytes through a cold read. The portable data mirror
    // otherwise legitimately retains the previously authenticated original wire.
    kura.block_store.lock().invalidate_data_mmap();
    // Historical get_block is not the query authority. Even a decodable value
    // returned there cannot restore membership or enter the canonical body cache.
    let loaded = kura
        .get_block(height)
        .expect("decode substituted historical body");
    assert_eq!(loaded.encode_wire().unwrap(), wire);
    let index = kura.transaction_entrypoint_index.lock();
    assert!(index.incomplete_heights.contains(&height));
    assert!(!index.indexed_heights.contains(&height));
    assert!(!index.inventories_by_height.contains_key(&height));
    drop(index);
    assert!(kura.block_data.lock().cached_body(1).is_none());
}

#[test]
fn executed_history_denial_precedes_cold_body_decode_and_projection() {
    use iroha_data_model::query::error::QueryExecutionFail;
    for corrupt in [false, true] {
        let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(4_usize));
        let blocks = bounded_read_executed_blocks(&kura);
        let height = nonzero!(2_usize);
        finalize_chain_through_for_eviction(&kura, height);
        let hashes = blocks.iter().map(|block| block.hash()).collect::<Vec<_>>();
        let wire_len = u64::try_from(blocks[1].encode_wire().unwrap().len()).unwrap();
        let (path, slot) = {
            let mut store = kura.block_store.lock();
            (
                store.path_to_blockchain.join(DATA_FILE_NAME),
                store.read_block_index(1).unwrap(),
            )
        };
        if corrupt {
            let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(slot.start)).unwrap();
            file.write_all(&vec![0; usize::try_from(slot.length).unwrap()])
                .unwrap();
            file.sync_all().unwrap();
        }
        kura.mark_transaction_entrypoint_index_incomplete(2, 4);
        kura.block_data.lock()[1].1 = None;
        let index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
        let bytes_before = fs::read(&path).unwrap();
        let body_bytes_before = kura.canonical_body_bytes_read_for_test();
        let charged = std::cell::Cell::new(0);
        let denied = crate::state::CanonicalHistorySource::read_executed_for_testing(
            &kura,
            &hashes,
            height,
            |length| {
                charged.set(charged.get() + 1);
                assert_eq!(
                    length, wire_len,
                    "admission uses exact signed execution length"
                );
                Err(QueryExecutionFail::GasBudgetExceeded)
            },
        );
        assert!(matches!(denied, Err(QueryExecutionFail::GasBudgetExceeded)));
        assert_eq!(charged.get(), 1);
        assert_eq!(
            kura.canonical_body_bytes_read_for_test(),
            body_bytes_before,
            "denied admission performs neither inline body read path"
        );
        assert!(kura.block_data.lock().cached_body(1).is_none());
        assert_eq!(
            format!("{:?}", *kura.transaction_entrypoint_index.lock()),
            index_before
        );
        assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert_eq!(fs::read(&path).unwrap(), bytes_before);
        let admitted = crate::state::CanonicalHistorySource::read_executed_for_testing(
            &kura,
            &hashes,
            height,
            |length| {
                charged.set(charged.get() + 1);
                assert_eq!(length, wire_len);
                Ok(())
            },
        );
        assert_eq!(charged.get(), 2);
        assert_eq!(
            kura.canonical_body_bytes_read_for_test(),
            body_bytes_before + wire_len,
            "only the admitted invocation reads the exact signed body length"
        );
        if corrupt {
            let expected = format!(
                "canonical executed body at height 2 failed storage authentication: {}",
                Error::CanonicalBlockWireMismatch { height: 2 },
            );
            assert!(
                matches!(admitted,
                Err(QueryExecutionFail::Conversion(message)) if message == expected),
                "admitted occupied corruption fails its exact signed-wire check"
            );
        } else {
            assert_eq!(admitted.unwrap().as_ref(), blocks[1].as_ref());
        }
        assert!(
            kura.block_data.lock().cached_body(1).is_none(),
            "the exact query body reader has no cache publication side effect"
        );
        assert_eq!(
            format!("{:?}", *kura.transaction_entrypoint_index.lock()),
            index_before
        );
        assert_eq!(fs::read(&path).unwrap(), bytes_before);
    }
}
