// Local encoding and persistence tests exercise the same prepared plan and physical writer
// used by authenticated capture. No detached producer, synthetic finality or logical gate exists.
#[test]
fn provider_prepared_plan_holds_the_actual_writer_and_drop_has_no_effects() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let before = projection(7);
    archive.insert(before.clone()).unwrap();
    let writer = archive.index.try_write().unwrap();
    let plan = archive
        .prepare_insert_locked(&advance_projection(&before, 8), &writer)
        .unwrap();
    assert!(matches!(
        archive.index.try_write(),
        Err(ArchiveIndexLockError::Busy(_))
    ));
    assert_eq!(writer.generation, 1);
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    drop(plan);
    drop(writer);
    let index = archive.read_index().unwrap();
    assert_eq!(index.generation, 1);
    assert_eq!(
        reconstruct_projection(&index, &before.key, bounds()).unwrap(),
        before
    );
}

#[test]
fn provider_admission_rejects_transition_capacity_and_generation_before_writes() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let before = projection(7);
    archive.insert(before.clone()).unwrap();
    let mut substituted = advance_projection(&before, 8);
    substituted.providers[0].expected_owner = Some(account(0x71));
    assert!(matches!(
        archive.insert(substituted),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
            reason: "provider completion authority is noncanonical or differs from registered owner",
        })
    ));
    // Exercise the transition guard with a canonical projection, so shape
    // validation cannot hide an unauthorized mid-history policy replacement.
    let mut substituted = advance_projection(&before, 8);
    substituted.providers[0].expected_authority = Some(completion_authority(0x11, policy(0xC1, 2)));
    substituted.validate(bounds()).unwrap();
    assert!(matches!(
        archive.insert(substituted),
        Err(ProviderIngestFinalizedArchiveErrorV1::AuthoritySubstitution { .. })
    ));
    assert!(matches!(
        archive.insert(advance_projection(&before, 9)),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCoverageGap { .. })
    ));
    archive.write_index().unwrap().generation = u64::MAX;
    assert!(matches!(
        archive.insert(advance_projection(&before, 8)),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    assert_eq!(archive.read_index().unwrap().by_height.len(), 1);

    let directory = physical_tempdir().unwrap();
    let mut small = bounds();
    small.max_archive_entries = NonZeroUsize::new(1).unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), small).unwrap(),
    );
    archive.insert(before.clone()).unwrap();
    assert!(matches!(
        archive.insert(advance_projection(&before, 8)),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert!(archive.index.try_write().is_ok());
}

#[test]
fn provider_prepared_plan_retries_exact_bytes_and_publishes_once_under_original_writer() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let expected = projection(7);
    let mut writer = archive.index.try_write().unwrap();
    let mut plan = archive.prepare_insert_locked(&expected, &writer).unwrap();
    let record = plan.record.as_ref().unwrap();
    let path = record.entry.path.clone();
    let bytes = record.bytes.clone();
    let pointer = record.bytes.as_ptr();
    let expected_total = record.total_bytes;
    fs::create_dir(&path).unwrap();
    for _ in 0..2 {
        assert!(plan.persist(&archive, &mut writer).is_err());
        assert!(writer.by_height.is_empty());
        assert_eq!(writer.total_bytes, 0);
        assert_eq!(writer.generation, 0);
        assert_eq!(plan.record.as_ref().unwrap().bytes.as_ptr(), pointer);
        assert_eq!(plan.record.as_ref().unwrap().bytes, bytes);
    }
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        plan.persist(&archive, &mut writer).unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(writer.total_bytes, expected_total);
    assert_eq!(writer.generation, 1);
    assert_eq!(
        plan.persist(&archive, &mut writer).unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(writer.total_bytes, expected_total);
    assert_eq!(writer.generation, 1);
    drop(writer);
    drop(archive);
    let reopened =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    assert_eq!(
        reconstruct_projection(&reopened.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected
    );
}

#[test]
fn provider_prepared_plan_refuses_directory_substitution_before_publication() {
    let directory = physical_tempdir().unwrap();
    let archive =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    let mut writer = archive.index.try_write().unwrap();
    let mut plan = archive
        .prepare_insert_locked(&projection(7), &writer)
        .unwrap();
    let original_bytes = plan.record.as_ref().unwrap().bytes.as_ptr();
    let original_records = archive.root.join("retained-records");
    fs::rename(&archive.records, &original_records).unwrap();
    fs::create_dir(&archive.records).unwrap();
    assert!(plan.persist(&archive, &mut writer).is_err());
    assert_eq!(plan.record.as_ref().unwrap().bytes.as_ptr(), original_bytes);
    assert!(writer.by_height.is_empty());
    assert_eq!(writer.total_bytes, 0);
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&original_records).unwrap().count(), 0);
    fs::remove_dir(&archive.records).unwrap();
    fs::rename(&original_records, &archive.records).unwrap();
    assert_eq!(
        plan.persist(&archive, &mut writer).unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
}
