// Actual geometry mutation under one retained publication boundary.

fn held_geometry_transition<'input>(
    kura: &'input Kura,
    previous: &'input LaneConfig,
    updated: &'input LaneConfig,
    previous_incarnations: &'input BTreeMap<LaneId, Hash>,
    updated_incarnations: &'input BTreeMap<LaneId, Hash>,
    previous_activations: &'input BTreeMap<LaneId, u64>,
    updated_activations: &'input BTreeMap<LaneId, u64>,
    replaced: &'input BTreeSet<LaneId>,
) -> (
    crate::kura::publication_lease::KuraPublicationLease<'input>,
    guarded_publication::PreparedLaneGeometryTransition<'input>,
) {
    let prune = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required().unwrap();
    let canonical = kura.canonical_chain_lock.lock();
    kura.resolve_canonical_storage_before_mutation().unwrap();
    let pending_canonical_bytes = kura
        .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
        .unwrap();
    let geometry = kura.lane_geometry_lock.lock();
    let previous_bindings = kura
        .geometry_bindings(previous, previous_incarnations, previous_activations)
        .unwrap();
    let updated_bindings = kura
        .geometry_bindings(updated, updated_incarnations, updated_activations)
        .unwrap();
    let journal_was_present = kura
        .validate_path_kind(&kura.lane_geometry_journal_path(), false)
        .unwrap();
    let mut journal = kura.read_lane_geometry_journal().unwrap();
    kura.finish_pending_lane_geometry_gc_locked(&mut journal)
        .unwrap();
    let lease = crate::kura::publication_lease::KuraPublicationLease::from_geometry_guards(
        kura,
        kura.sidecar_lock.lock(),
        geometry,
        canonical,
        prune,
    );
    let prepared = guarded_publication::PreparedLaneGeometryTransition {
        previous,
        updated,
        previous_incarnations,
        updated_incarnations,
        previous_activation_heights: previous_activations,
        updated_activation_heights: updated_activations,
        previous_lineage_root: unscoped_lineage_root(&previous_bindings),
        updated_lineage_root: unscoped_lineage_root(&updated_bindings),
        replaced_lane_ids: replaced,
        certified_retirements: BTreeSet::new(),
        transition_height: Some(9),
        namespace_receipts: None,
        pending_canonical_bytes,
        previous_catalog: geometry_catalog_fingerprint(&previous_bindings),
        updated_catalog: geometry_catalog_fingerprint(&updated_bindings),
        previous_bindings,
        updated_bindings,
        journal_was_present,
        journal,
    };
    (lease, prepared)
}

fn held_geometry_catalog(
    kura: &Kura,
    config: &LaneConfig,
    incarnations: &BTreeMap<LaneId, Hash>,
    activations: &BTreeMap<LaneId, u64>,
) -> guarded_publication::PreparedLaneGeometryCatalog {
    let bindings = kura
        .geometry_bindings(config, incarnations, activations)
        .unwrap();
    guarded_publication::PreparedLaneGeometryCatalog {
        fingerprint: geometry_catalog_fingerprint(&bindings),
        lineage_root: unscoped_lineage_root(&bindings),
        bindings,
        configured_baseline: None,
        journal: kura.read_lane_geometry_journal().unwrap(),
    }
}

fn assert_geometry_fences_owned(kura: &Kura) {
    for lock in [
        &kura.prune_lock,
        &kura.canonical_chain_lock,
        &kura.lane_geometry_lock,
        &kura.sidecar_lock,
    ] {
        assert!(lock.try_lock_or_wait().is_err());
    }
}

#[test]
fn guarded_geometry_apply_and_catalog_retry_keep_one_physical_boundary() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (previous, updated) = initial_and_extended_configs();
    let (previous_incarnations, previous_activations) = initial_geometry();
    let (updated_incarnations, updated_activations) = extended_geometry();
    let kura = open_kura(&root, &previous);
    authenticate_transition_fixture_primary(&kura, &previous, &previous_incarnations);
    let replaced = BTreeSet::new();
    let (lease, prepared) = held_geometry_transition(
        &kura,
        &previous,
        &updated,
        &previous_incarnations,
        &updated_incarnations,
        &previous_activations,
        &updated_activations,
        &replaced,
    );
    assert!(std::ptr::eq(lease.kura_under_publication_guards(), &*kura));
    assert_geometry_fences_owned(&kura);
    lease.apply_prepared_lane_geometry(prepared).unwrap();
    assert_geometry_fences_owned(&kura);
    let files_applied = kura.read_lane_geometry_journal().unwrap();
    assert_eq!(files_applied.records.len(), 1);
    assert_eq!(
        files_applied.records[0].phase,
        LaneGeometryPhase::FilesApplied
    );
    let exact_transition = files_applied.records[0].transition_id;
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let new_binding = &files_applied.records[0].updated_bindings[1];
    kura.require_lane_marker(new_binding).unwrap();
    kura.fail_next_lane_geometry_publication_after_write_for_test();
    let error = lease
        .publish_prepared_lane_geometry_catalog(held_geometry_catalog(
            &kura,
            &updated,
            &updated_incarnations,
            &updated_activations,
        ))
        .expect_err("post-replacement failure restores the original FilesApplied journal");
    assert!(error.to_string().contains("after journal replacement"));
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    assert_geometry_fences_owned(&kura);
    lease
        .publish_prepared_lane_geometry_catalog(held_geometry_catalog(
            &kura,
            &updated,
            &updated_incarnations,
            &updated_activations,
        ))
        .unwrap();
    let published = kura.read_lane_geometry_journal().unwrap();
    assert_eq!(published.records.len(), 1);
    assert_eq!(published.records[0].transition_id, exact_transition);
    assert_eq!(
        published.records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
    assert_geometry_fences_owned(&kura);
    drop(lease);
    drop(
        kura.try_publication_lease()
            .expect("all original fences released"),
    );
    // The production wrapper reaches the same guarded implementation on retry.
    kura.apply_lane_geometry_transition_at_height(
        &previous,
        &updated,
        &previous_incarnations,
        &updated_incarnations,
        &previous_activations,
        &updated_activations,
        &replaced,
        9,
    )
    .unwrap();
    assert_eq!(kura.read_lane_geometry_journal().unwrap(), published);
}

#[test]
fn guarded_geometry_foreign_catalog_refusal_preserves_files_and_releases_on_drop() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (previous, updated) = initial_and_extended_configs();
    let (previous_incarnations, previous_activations) = initial_geometry();
    let (updated_incarnations, updated_activations) = extended_geometry();
    let kura = open_kura(&root, &previous);
    authenticate_transition_fixture_primary(&kura, &previous, &previous_incarnations);
    let replaced = BTreeSet::new();
    let (lease, prepared) = held_geometry_transition(
        &kura,
        &previous,
        &updated,
        &previous_incarnations,
        &updated_incarnations,
        &previous_activations,
        &updated_activations,
        &replaced,
    );
    lease.apply_prepared_lane_geometry(prepared).unwrap();
    let original = fs::read(kura.lane_geometry_journal_path()).unwrap();
    lease
        .publish_prepared_lane_geometry_catalog(held_geometry_catalog(
            &kura,
            &previous,
            &previous_incarnations,
            &previous_activations,
        ))
        .expect_err("predecessor is not the exact uncertain successor");
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        original
    );
    assert_geometry_fences_owned(&kura);
    drop(lease);
    drop(
        kura.try_publication_lease()
            .expect("refusal did not leak guards"),
    );
    kura.mark_lane_geometry_catalog_published(
        &updated,
        &updated_incarnations,
        &updated_activations,
        None,
    )
    .expect("production wrapper can finish the exact retained transition");
}
