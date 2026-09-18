// Exact prepared journal phase ownership, independent of finality authorization.

fn unpersisted_create_journal(kura: &Kura) -> LaneGeometryJournal {
    let (previous, updated) = initial_and_extended_configs();
    let (previous_incarnations, previous_activations) = initial_geometry();
    let (updated_incarnations, updated_activations) = extended_geometry();
    let previous_bindings = kura
        .geometry_bindings(&previous, &previous_incarnations, &previous_activations)
        .unwrap();
    let updated_bindings = kura
        .geometry_bindings(&updated, &updated_incarnations, &updated_activations)
        .unwrap();
    let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
    let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
    let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
    let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
    let transition_id = geometry_transition_id(
        0,
        9,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
    );
    let operations = kura
        .build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            &BTreeSet::new(),
        )
        .unwrap();
    let mut journal = kura.read_lane_geometry_journal().unwrap();
    assert!(journal.records.is_empty());
    journal.records.push(LaneGeometryIntent {
        transition_id,
        transition_sequence: 0,
        transition_height: 9,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
        previous_bindings,
        updated_bindings,
        phase: LaneGeometryPhase::Intent,
        operations,
    });
    journal
}

#[test]
fn prepared_geometry_journal_has_exact_phases_and_drop_writes_nothing() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_geometry().0);
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let journal = unpersisted_create_journal(&kura);
    let mut prepared =
        PreparedGeometryJournalTransition::prepare(&kura, journal.clone(), 0).unwrap();
    let retained = prepared.retained_allocation_bytes().unwrap();
    assert!(retained <= MAX_GEOMETRY_JOURNAL_BYTES);
    let phases = [
        LaneGeometryPhase::Intent,
        LaneGeometryPhase::FilesApplied,
        LaneGeometryPhase::CatalogPublished,
        LaneGeometryPhase::RolledBack,
    ];
    // Exercise every direction, including resetting a different phase patch
    // before applying the next one to the sole retained encoding buffer.
    for from in phases {
        prepared.bytes(from);
        for phase in phases {
            let mut expected = journal.clone();
            expected.records[0].phase = phase;
            assert_eq!(prepared.bytes(phase), expected.encode());
            assert_eq!(
                decode_exact::<LaneGeometryJournal>(prepared.bytes(phase)).unwrap(),
                expected
            );
            assert_eq!(prepared.retained_allocation_bytes(), Some(retained));
            prepared.bytes(from);
        }
    }
    assert_eq!(prepared.operations(), journal.records[0].operations);
    drop(prepared);
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    assert!(
        !geometry_fixture_blocks(
            &kura,
            extended.entry(LaneId::new(1)).unwrap(),
            &extended_geometry().0,
            &extended_geometry().1
        )
        .exists()
    );
}

#[test]
fn prepared_geometry_journal_rejects_identity_and_capacity_before_writes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_geometry().0);
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let journal = unpersisted_create_journal(&kura);
    assert!(
        PreparedGeometryJournalTransition::prepare_phases(&root, journal.clone(), 0, 1).is_err()
    );
    assert!(PreparedGeometryJournalTransition::prepare(&kura, journal.clone(), 1).is_err());
    let prepared = PreparedGeometryJournalTransition::prepare(&kura, journal.clone(), 0).unwrap();
    let aggregate = prepared.retained_allocation_bytes().unwrap();
    assert!(aggregate > u64::try_from(journal.encode().len()).unwrap());
    assert!(
        PreparedGeometryJournalTransition::prepare_phases(
            &root,
            journal.clone(),
            0,
            aggregate - 1,
        )
        .is_err(),
        "the journal alone fitting cannot omit retained operations and phase changes"
    );
    let mut forged = journal;
    forged.records[0].transition_height += 1;
    assert!(PreparedGeometryJournalTransition::prepare(&kura, forged, 0).is_err());
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
}

#[test]
fn prepared_geometry_journal_retry_uses_retained_bytes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let journal = unpersisted_create_journal(&kura);
    let mut prepared = PreparedGeometryJournalTransition::prepare(&kura, journal, 0).unwrap();
    let temporary = root.join(JOURNAL_TEMP_FILE_NAME);
    fs::create_dir(&temporary).unwrap();
    assert!(prepared.persist(&kura, LaneGeometryPhase::Intent).is_err());
    fs::remove_dir(&temporary).unwrap();
    prepared.persist(&kura, LaneGeometryPhase::Intent).unwrap();
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        prepared.bytes(LaneGeometryPhase::Intent)
    );
    prepared.persist(&kura, LaneGeometryPhase::Intent).unwrap();
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        prepared.bytes(LaneGeometryPhase::Intent)
    );
}

#[test]
fn prepared_geometry_non_tail_retry_preserves_rolled_back_successor_history() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let third = RuntimeLaneConfig::from_catalog(
        &LaneCatalog::new(
            nonzero!(3_u32),
            vec![
                ModelLaneConfig::default(),
                ModelLaneConfig {
                    id: LaneId::new(1),
                    alias: "elastic-one".to_owned(),
                    ..ModelLaneConfig::default()
                },
                ModelLaneConfig {
                    id: LaneId::new(2),
                    alias: "elastic-two".to_owned(),
                    ..ModelLaneConfig::default()
                },
            ],
        )
        .unwrap(),
    );
    let mut third_incarnations = extended_incarnations.clone();
    third_incarnations.insert(LaneId::new(2), Hash::prehashed([0x33; Hash::LENGTH]));
    let mut third_activations = extended_activations.clone();
    third_activations.insert(LaneId::new(2), 10);
    let configs = [&initial, &extended, &third];
    let incarnations = [
        &initial_incarnations,
        &extended_incarnations,
        &third_incarnations,
    ];
    let activations = [
        &initial_activations,
        &extended_activations,
        &third_activations,
    ];
    let kura = open_kura(&root, &initial);
    for index in 0..2 {
        kura.apply_lane_geometry_transition_at_height(
            configs[index],
            configs[index + 1],
            incarnations[index],
            incarnations[index + 1],
            activations[index],
            activations[index + 1],
            &BTreeSet::new(),
            9 + index as u64,
        )
        .unwrap();
        kura.mark_lane_geometry_catalog_published(
            configs[index + 1],
            incarnations[index + 1],
            activations[index + 1],
            None,
        )
        .unwrap();
    }
    // An earlier record cannot become uncertain while a later one is published.
    // The production retry must first reconcile the actual durable history.
    let published = kura.read_lane_geometry_journal().unwrap();
    assert!(PreparedGeometryJournalTransition::prepare(&kura, published, 0).is_err());
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .unwrap();
    let rolled_back = kura.read_lane_geometry_journal().unwrap();
    assert_eq!(rolled_back.records.len(), 2);
    assert!(
        rolled_back
            .records
            .iter()
            .all(|record| record.phase == LaneGeometryPhase::RolledBack)
    );
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let mut prepared =
        PreparedGeometryJournalTransition::prepare(&kura, rolled_back.clone(), 0).unwrap();
    for phase in [
        LaneGeometryPhase::Intent,
        LaneGeometryPhase::FilesApplied,
        LaneGeometryPhase::CatalogPublished,
        LaneGeometryPhase::RolledBack,
    ] {
        let decoded = decode_exact::<LaneGeometryJournal>(prepared.bytes(phase)).unwrap();
        validate_lane_geometry_journal_structure(&root, &decoded).unwrap();
        assert_eq!(decoded.records[0].phase, phase);
        assert_eq!(decoded.records[1], rolled_back.records[1]);
    }
    drop(prepared);
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    for index in 0..2 {
        // These exact-height calls exercise the production existing-index branch,
        // including the newly prepared non-tail phase bytes on the first retry.
        kura.apply_lane_geometry_transition_at_height(
            configs[index],
            configs[index + 1],
            incarnations[index],
            incarnations[index + 1],
            activations[index],
            activations[index + 1],
            &BTreeSet::new(),
            9 + index as u64,
        )
        .unwrap();
        let replayed = kura.read_lane_geometry_journal().unwrap();
        assert_eq!(
            replayed.records[index].phase,
            LaneGeometryPhase::FilesApplied
        );
        if index == 0 {
            assert_eq!(replayed.records[1], rolled_back.records[1]);
        }
        kura.mark_lane_geometry_catalog_published(
            configs[index + 1],
            incarnations[index + 1],
            activations[index + 1],
            None,
        )
        .unwrap();
    }
    let replayed = kura.read_lane_geometry_journal().unwrap();
    assert_eq!(replayed.records.len(), 2);
    assert!(
        replayed
            .records
            .iter()
            .all(|record| record.phase == LaneGeometryPhase::CatalogPublished)
    );
}
