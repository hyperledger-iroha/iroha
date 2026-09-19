// Complete retirement observation keeps maintenance and durability explicit.

fn retirement_observation_fixture(root: &Path) -> (Arc<Kura>, LaneRetirementIdentity) {
    let (initial, extended) = retirement_test_configs();
    let (incarnations, activations) = retirement_test_geometry();
    let initial_incarnations = BTreeMap::from([(LaneId::SINGLE, incarnations[&LaneId::SINGLE])]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        root,
        &initial,
        &extended,
        &initial_incarnations,
        &incarnations,
        &initial_activations,
        &activations,
    );
    let entry = kura
        .lane_storage_entry(LaneId::new(1))
        .expect("retiring instance");
    let retiring = LaneRetirementIdentity {
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: entry.incarnation,
    };
    (kura, retiring)
}

#[test]
fn retirement_observation_preserves_complete_applied_census_without_sync() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, retiring) = retirement_observation_fixture(&root);
    let work = install_merge_applied_retirement_work(&kura, retiring.lane_incarnation);
    // Finish existing local recovery first. Observation must never perform this phase.
    kura.first_release_lane_retirement_admissible_for_test(
        retiring.lane_id,
        retiring.dataspace_id,
        retiring.lane_incarnation,
    )
    .unwrap();
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let before = native_observation_tree(&root);
    super::super::fail_next_indexed_sidecar_dir_sync_for_tests();
    let result = kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::new());
    // Consume/reset even on observation failure, so a failed test cannot leak its fault.
    let untouched =
        super::super::FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.replace(false));
    let census = result.expect("complete authenticated retirement read");
    assert!(
        untouched,
        "pure observation must not synchronize any progress namespace"
    );
    assert_eq!(census.routes.len(), 2);
    assert_eq!(census.retiring, BTreeSet::from([retiring]));
    assert!(census.certified_retirements.is_empty());
    let key = (
        retiring.lane_id,
        work.certified.proposal.descriptor.lane_block_height,
    );
    assert_eq!(census.certified[&key], work.certified);
    assert!(census.inputs.contains_key(&key));
    assert!(census.receipts.contains_key(&key));
    assert!(census.work_items_seen >= 3);
    drop(census);
    assert_eq!(native_observation_tree(&root), before);
}

#[test]
fn retirement_observation_refuses_each_unfinished_pair_without_repair() {
    for data_file in [
        LANE_ARTIFACTS_DATA_FILE,
        LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
        LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
        CERTIFIED_LANE_BLOCKS_DATA_FILE,
        AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
        CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE,
        LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
    ] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (kura, retiring) = retirement_observation_fixture(&root);
        let entry = kura.lane_storage_entry(retiring.lane_id).unwrap();
        let artifacts = Kura::lane_artifact_dir(&entry.blocks_dir(&root));
        fs::create_dir_all(&artifacts).unwrap();
        let temporary = artifacts.join(data_file).with_extension("norito.tmp");
        fs::write(&temporary, b"unpublished progress rewrite").unwrap();
        let before = native_observation_tree(&root);
        {
            let _prune = kura.prune_lock.lock();
            let _canonical = kura.canonical_chain_lock.lock();
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            let error = kura
                .observe_lane_retirement_locked(&[retiring], &BTreeSet::new())
                .err()
                .expect("observation must defer to maintenance");
            assert!(
                matches!(error, Error::IO(ref cause, _) if cause.kind() == ErrorKind::WouldBlock)
            );
        }
        assert_eq!(native_observation_tree(&root), before);
        assert!(temporary.exists());
        kura.first_release_lane_retirement_admissible_for_test(
            retiring.lane_id,
            retiring.dataspace_id,
            retiring.lane_incarnation,
        )
        .expect("explicit maintenance still recovers the unpublished rewrite");
        assert!(!temporary.exists());
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::new())
            .expect("the repaired exact namespace can be observed");
    }
}

#[test]
fn retirement_observation_rejects_foreign_certified_owner_before_effects() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, retiring) = retirement_observation_fixture(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let before = native_observation_tree(&root);
    let foreign = LaneRetirementIdentity {
        lane_incarnation: Hash::new(b"foreign retirement incarnation"),
        ..retiring
    };
    assert!(
        kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::from([foreign]))
            .is_err()
    );
    assert_eq!(native_observation_tree(&root), before);
}

#[test]
fn retirement_observation_preserves_failed_merge_append_until_explicit_maintenance() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, retiring) = retirement_observation_fixture(&root);
    let work = install_merge_applied_retirement_work(&kura, retiring.lane_incarnation);
    kura.first_release_lane_retirement_admissible_for_test(
        retiring.lane_id,
        retiring.dataspace_id,
        retiring.lane_incarnation,
    )
    .expect("complete initial maintenance");
    let census = {
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::new())
            .expect("observe authentic complete retirement before failed append")
    };
    let key = (
        retiring.lane_id,
        work.certified.proposal.descriptor.lane_block_height,
    );
    let receipt = &census.receipts[&key];
    let payload = &census.autonomous[&key].0.executable_payload;
    let path = kura.active_merge_path.lock().clone();
    let original = fs::read(&path).unwrap();
    let mut next = work.entry.clone();
    next.epoch_id += 1;
    next.execution_batch = None;
    kura.fail_next_merge_append_after_for_test(
        crate::kura::MergeLedgerAppendFailurePoint::AfterLength,
    );
    crate::kura::tests::FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES.with(|flag| flag.set(true));
    let appended = kura.append_merge_entry_for_test(&next);
    let unused_fault =
        crate::kura::tests::FAIL_MERGE_TAIL_RECOVERY_FOR_RESOURCES.with(|flag| flag.replace(false));
    assert!(appended.is_err());
    assert!(
        !unused_fault,
        "the real failed append must attempt its owned tail recovery"
    );
    let failed = fs::read(&path).unwrap();
    assert_eq!(failed.len(), original.len() + 4);
    let before = native_observation_tree(&root);
    {
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        assert!(!RetirementScanEffects::Observe.receipt_matches_merge_log(&kura, receipt));
        assert!(
            !RetirementScanEffects::Observe.autonomous_receipt_applies(&kura, receipt, payload)
        );
        assert!(
            kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::new())
                .is_err()
        );
        assert_eq!(
            kura.merge_log.lock().append_recovery_offset,
            Some(original.len() as u64)
        );
        assert_eq!(fs::read(&path).unwrap(), failed);
    }
    assert_eq!(native_observation_tree(&root), before);
    kura.first_release_lane_retirement_admissible_for_test(
        retiring.lane_id,
        retiring.dataspace_id,
        retiring.lane_incarnation,
    )
    .expect("explicit maintenance retains failed-append repair");
    assert_eq!(kura.merge_log.lock().append_recovery_offset, None);
    assert_eq!(fs::read(&path).unwrap(), original);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    assert!(RetirementScanEffects::Observe.receipt_matches_merge_log(&kura, receipt));
    assert!(RetirementScanEffects::Observe.autonomous_receipt_applies(&kura, receipt, payload));
    kura.observe_lane_retirement_locked(&[retiring], &BTreeSet::new())
        .expect("same authenticated retirement is observable after explicit repair");
}
