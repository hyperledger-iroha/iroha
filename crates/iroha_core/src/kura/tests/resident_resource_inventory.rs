// Independent recounts deliberately walk test-owned maps; production sampling never does.
fn recount_transaction_resident_associations(index: &super::TransactionEntrypointIndex) -> u64 {
    let mut count = index.indexed_heights.len()
        + index.incomplete_merge_heights.len()
        + index.incomplete_kaigi_signal_heights.len()
        + index.inventories_by_height.len();
    count += index
        .heights_by_entrypoint
        .values()
        .map(|heights| heights.len())
        .sum::<usize>();
    count += index
        .heights_by_authority
        .values()
        .map(|heights| heights.len())
        .sum::<usize>();
    count += index
        .heights_by_timestamp_ms
        .values()
        .map(|heights| heights.len())
        .sum::<usize>();
    count += index
        .heights_by_result_status
        .values()
        .map(|heights| heights.len())
        .sum::<usize>();
    for inventory in index.inventories_by_height.values() {
        count += inventory.entrypoint_hashes.len()
            + inventory.authorities.len()
            + inventory.timestamps_ms.len()
            + inventory.result_statuses.len()
            + inventory.kaigi_calls.len();
    }
    for by_height in index.kaigi_signal_candidates.values() {
        count += by_height
            .values()
            .map(|locators| locators.len())
            .sum::<usize>();
    }
    u64::try_from(count).expect("bounded fixture association count")
}

#[test]
fn resident_transaction_counts_memberships_through_duplicate_replace_and_truncate() {
    use super::resident_inventory::ResidentOwner;
    let mut generator = DummyBlocks::new();
    let block = generator.next_with_results();
    let distinct_entrypoints = block.entrypoint_hashes().collect::<BTreeSet<_>>().len();
    assert!(distinct_entrypoints > 0);
    let mut index = super::TransactionEntrypointIndex::complete_empty();
    let assert_count = |index: &super::TransactionEntrypointIndex| {
        assert_eq!(
            index.resident_associations().unwrap(),
            recount_transaction_resident_associations(index)
        );
    };
    for height in 1..=3 {
        Kura::insert_transaction_entrypoint_heights(
            &mut index,
            NonZeroUsize::new(height).unwrap(),
            &block,
        );
        assert_count(&index);
    }
    assert_eq!(
        index.heights_by_entrypoint.len(),
        distinct_entrypoints,
        "unchanged outer keys each own three distinct height memberships"
    );
    assert_eq!(
        index.heights_by_entrypoint.values().next().unwrap().len(),
        3
    );
    assert_eq!(index.heights_by_authority.values().next().unwrap().len(), 3);
    assert_eq!(
        index.heights_by_timestamp_ms.values().next().unwrap().len(),
        3
    );
    assert_eq!(
        index
            .heights_by_result_status
            .values()
            .next()
            .unwrap()
            .len(),
        3
    );
    let before_duplicate = index.resident_associations().unwrap();
    Kura::insert_transaction_entrypoint_heights(&mut index, nonzero!(2_usize), &block);
    assert_eq!(index.resident_associations().unwrap(), before_duplicate);
    let call = kaigi_signal_test_call("resident-memberships");
    for phase in [0, 1] {
        insert_kaigi_signal_test_locator(&mut index, &call, kaigi_signal_test_locator(2, phase, 0));
        assert_count(&index);
    }
    let before_locator_duplicate = index.resident_associations().unwrap();
    insert_kaigi_signal_test_locator(&mut index, &call, kaigi_signal_test_locator(2, 1, 0));
    assert_eq!(
        index.resident_associations().unwrap(),
        before_locator_duplicate
    );
    Kura::remove_transaction_entrypoint_height(&mut index, nonzero!(2_usize));
    assert_count(&index);
    assert!(!index.kaigi_signal_candidates.contains_key(&call));
    Kura::remove_transaction_entrypoint_height(&mut index, nonzero!(2_usize));
    assert_count(&index);
    let replacement = generator.next_with_results();
    Kura::insert_transaction_entrypoint_heights(&mut index, nonzero!(2_usize), &replacement);
    assert_count(&index);
    Kura::truncate_transaction_entrypoint_index_to(&mut index, 1);
    assert_count(&index);
    assert_eq!(index.heights_by_entrypoint.len(), distinct_entrypoints);
    Kura::truncate_transaction_entrypoint_index_to(&mut index, 0);
    assert_count(&index);
    assert_eq!(index.resident_associations().unwrap(), 0);
}

#[test]
fn resident_merge_projection_and_real_kaigi_candidate_insertion_match_recount() {
    use super::resident_inventory::ResidentOwner;
    use iroha_data_model::metadata::Metadata;
    use iroha_primitives::json::Json;
    let call = kaigi_signal_test_call("resident-real-signal");
    let mut metadata = Metadata::default();
    metadata.insert(
        "kaigi_signal".parse().unwrap(),
        Json::new(norito::json!({
            "schema": "iroha-demo-kaigi-chain-signal/v1", "callId": (call.to_string())
        })),
    );
    let transaction = TransactionBuilder::new(
        test_network_id(b"kura-operation-index-network"),
        AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "resident signal".to_owned())])
    .with_metadata(metadata)
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entry = merge_entry_with_indexed_entrypoint(TransactionEntrypoint::External(transaction));
    let batch = entry.execution_batch.as_ref().unwrap();
    let mut index = super::TransactionEntrypointIndex::complete_empty();
    let block_hash = batch.application_block_header.hash();
    for height in 1..=2 {
        Kura::insert_merge_execution_index_heights(
            &mut index,
            NonZeroUsize::new(height).unwrap(),
            block_hash,
            batch,
        );
        assert_eq!(
            index.resident_associations().unwrap(),
            recount_transaction_resident_associations(&index)
        );
    }
    assert_eq!(index.kaigi_signal_candidates[&call].len(), 2);
    assert_eq!(
        index.kaigi_signal_candidates[&call][&nonzero!(2_usize)].len(),
        1
    );
    let before = index.resident_associations().unwrap();
    Kura::insert_merge_execution_index_heights(&mut index, nonzero!(2_usize), block_hash, batch);
    assert_eq!(index.resident_associations().unwrap(), before);
    Kura::remove_transaction_entrypoint_height(&mut index, nonzero!(1_usize));
    assert_eq!(
        index.resident_associations().unwrap(),
        recount_transaction_resident_associations(&index)
    );
    Kura::truncate_transaction_entrypoint_index_to(&mut index, 0);
    assert_eq!(index.resident_associations().unwrap(), 0);
}

#[test]
fn resident_canonical_counts_materialized_slots_and_never_certifies_deferred_height() {
    use super::resident_inventory::ResidentOwner;
    let first = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"resident first"));
    let second = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"resident second"));
    let mut dense: BlockData = [(first, None), (second, None), (first, None)]
        .into_iter()
        .collect();
    let reverse = Kura::build_block_height_index(&dense);
    assert_eq!(dense.resident_associations().unwrap(), 3);
    assert_eq!(reverse.resident_associations().unwrap(), 2);
    dense.truncate(1);
    assert_eq!(dense.resident_associations().unwrap(), 1);
    let mut deferred = BlockData::deferred(1_000_000);
    assert_eq!(deferred.resident_associations().unwrap(), 0);
    assert!(!deferred.resident_complete());
    deferred.cache_hash(900_000, first);
    deferred.cache_hash(900_000, first);
    deferred.cache_hash(700_000, second);
    assert_eq!(deferred.resident_associations().unwrap(), 2);
    deferred.truncate(800_000);
    assert_eq!(deferred.resident_associations().unwrap(), 1);
    assert!(!deferred.resident_complete());
    let kura = Kura::blank_kura_for_testing();
    assert!(
        kura.reconcile_resident_resource_inventory().is_err(),
        "uninitialized carrier inventory cannot become a zero baseline"
    );
    assert!(kura.resource_inventory.try_snapshot().is_err());
}

#[test]
fn resident_merge_frames_latest_route_reload_truncate_and_failure_are_exact() {
    use super::resident_inventory::ResidentOwner;
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("merge-resident.log");
    let first = merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x35; 32], [0x36; 32]));
    let second = sample_merge_entry(2);
    let mut log = super::MergeLedgerLog::open_at(&path, 1).unwrap();
    assert!(log.append(&first).unwrap());
    assert_eq!(log.resident_associations().unwrap(), 3);
    assert!(!log.append(&first).unwrap());
    assert_eq!(log.resident_associations().unwrap(), 3);
    assert!(log.append(&second).unwrap());
    assert_eq!(
        log.entries.len(),
        1,
        "payload cache capacity does not count full frame indexes"
    );
    assert_eq!(log.resident_associations().unwrap(), 5);
    drop(log);
    let mut reopened = super::MergeLedgerLog::open_at(&path, 1).unwrap();
    assert_eq!(reopened.resident_associations().unwrap(), 5);
    assert!(reopened.resident_complete());
    reopened.truncate_to_len(1).unwrap();
    assert_eq!(reopened.resident_associations().unwrap(), 3);
    assert_eq!(reopened.latest_execution_entries.len(), 1);
    reopened.fail_next_append = true;
    assert!(reopened.append(&second).is_err());
    assert!(!reopened.resident_complete());
    assert_eq!(reopened.resident_associations().unwrap(), 3);
    drop(reopened);
    let mut repaired = super::MergeLedgerLog::open_at(&path, 1).unwrap();
    assert!(repaired.resident_complete());
    assert_eq!(repaired.resident_associations().unwrap(), 3);
    repaired.truncate_to_len(0).unwrap();
    assert_eq!(repaired.resident_associations().unwrap(), 0);
    assert!(repaired.latest_execution_entries.is_empty());
    assert!(!super::MergeLedgerLog::deferred(1).resident_complete());
}

#[test]
fn resident_carrier_forward_reverse_counts_survive_duplicate_reload_and_removal() {
    use super::resident_inventory::ResidentOwner;
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let _ = store_indexed_reservation_carrier(&kura, 0x42);
    let record = *kura
        .merge_carrier_index
        .lock()
        .by_height
        .values()
        .next()
        .unwrap();
    assert_eq!(
        kura.merge_carrier_index
            .lock()
            .resident_associations()
            .unwrap(),
        2
    );
    let _owner = kura.merge_carrier_lock.lock();
    assert!(!kura.write_merge_carrier_record_unlocked(record).unwrap());
    assert_eq!(
        kura.merge_carrier_index
            .lock()
            .resident_associations()
            .unwrap(),
        2
    );
    *kura.merge_carrier_index.lock() = super::MergeCarrierIndex::default();
    assert!(!kura.merge_carrier_index.lock().resident_complete());
    kura.ensure_merge_carrier_index_initialized_unlocked()
        .unwrap();
    assert_eq!(
        kura.merge_carrier_index
            .lock()
            .resident_associations()
            .unwrap(),
        2
    );
    assert!(kura.merge_carrier_index.lock().resident_complete());
    kura.remove_merge_carrier_record_unlocked(record).unwrap();
    assert_eq!(
        kura.merge_carrier_index
            .lock()
            .resident_associations()
            .unwrap(),
        0
    );
    assert!(kura.merge_carrier_index.lock().resident_complete());
}

#[test]
fn resident_live_kura_publication_matches_real_index_owners_without_partial_snapshot() {
    use super::resource_inventory::{Family, Unavailable};
    let kura = Kura::blank_kura_for_testing();
    {
        let _owner = kura.merge_carrier_lock.lock();
        kura.ensure_merge_carrier_index_initialized_unlocked()
            .unwrap();
    }
    kura.reconcile_resident_resource_inventory().unwrap();
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentCanonical)
            .unwrap()
            .resident_associations,
        0
    );
    assert!(matches!(
        kura.resource_inventory.try_snapshot(),
        Err(Unavailable::Unregistered)
    ));
    let mut generator = DummyBlocks::new();
    let first = generator.next_with_results();
    kura.store_block(Arc::clone(&first)).unwrap();
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentCanonical)
            .unwrap()
            .resident_associations,
        2
    );
    let observed = kura
        .resource_inventory
        .component_usage_for_tests(Family::ResidentTransaction)
        .unwrap()
        .resident_associations;
    assert_eq!(
        observed,
        recount_transaction_resident_associations(&kura.transaction_entrypoint_index.lock())
    );
    kura.set_transaction_entrypoint_index_entry(1, &first, 1, None);
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentTransaction)
            .unwrap()
            .resident_associations,
        observed
    );
    let second = generator.next_with_results();
    kura.store_block(second).unwrap();
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentCanonical)
            .unwrap()
            .resident_associations,
        4
    );
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentTransaction)
            .unwrap()
            .resident_associations,
        recount_transaction_resident_associations(&kura.transaction_entrypoint_index.lock())
    );
    assert!(
        kura.resource_inventory.try_snapshot().is_err(),
        "four resident owners never qualify unregistered physical families"
    );
}
