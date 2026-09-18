// Admitted archive persistence controls; these lower-level fixtures confer no consensus finality.
#[test]
fn prepared_provider_capture_reserves_writer_and_drop_has_no_effects() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let before = projection(7);
    archive.insert(before.clone()).unwrap();
    let prepared = archive
        .prepare_insert(advance_projection(&before, 8))
        .unwrap();
    assert!(
        archive.index.try_write().is_ok(),
        "capture releases the original physical writer"
    );
    let wait = match archive.write_index() {
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { wait }) => wait,
        _ => panic!("the exact logical writer must remain reserved"),
    };
    assert!(!wait.is_released());
    assert_eq!(archive.read_index().unwrap().by_height.len(), 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    drop(prepared);
    assert!(wait.is_released());
    assert!(archive.write_index().is_ok());
    let index = archive.read_index().unwrap();
    assert_eq!(index.by_height.len(), 1);
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
        archive.prepare_insert(substituted),
        Err(ProviderIngestFinalizedArchiveErrorV1::AuthoritySubstitution { .. })
    ));
    assert!(matches!(
        archive.prepare_insert(advance_projection(&before, 9)),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCoverageGap { .. })
    ));
    archive.write_index().unwrap().generation = u64::MAX;
    assert!(matches!(
        archive.prepare_insert(advance_projection(&before, 8)),
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
        archive.prepare_insert(advance_projection(&before, 8)),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. })
    ));
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert!(archive.capture_gate.ensure_unreserved().is_ok());
    assert!(archive.index.try_write().is_ok());
}

#[test]
fn prepared_provider_capture_retries_exact_bytes_and_publishes_once() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let expected = projection(7);
    let mut prepared = archive.prepare_insert(expected.clone()).unwrap();
    let record = prepared.plan.record.as_ref().unwrap();
    let path = record.entry.path.clone();
    let bytes = record.bytes.clone();
    let expected_total = record.total_bytes;
    fs::create_dir(&path).unwrap();
    assert!(
        prepared.persist().is_err(),
        "an obstructed immutable path is a storage failure"
    );
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert_eq!(archive.read_index().unwrap().total_bytes, 0);
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        prepared.persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        prepared.persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    drop(prepared);
    drop(archive);
    let reopened =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    assert_eq!(
        reconstruct_projection(&reopened.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected
    );
}

#[test]
fn prepared_provider_owner_moves_to_worker_and_retains_original_archive() {
    fn assert_static_send_sync<T: Send + Sync + 'static>() {}
    assert_static_send_sync::<PreparedProviderIngestCapture>();
    assert_static_send_sync::<PreparedProviderInsertion>();
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let expected = projection(7);
    let prepared = archive.prepare_insert(expected.clone()).unwrap();
    let original = Arc::downgrade(&archive);
    let wait = archive.capture_gate.ensure_unreserved().unwrap_err();
    assert!(archive.index.try_write().is_ok());
    drop(archive);
    assert!(original.upgrade().is_some());
    std::thread::spawn(move || {
        let mut prepared = prepared;
        assert_eq!(
            prepared.persist().unwrap(),
            ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
        );
    })
    .join()
    .unwrap();
    assert!(wait.is_released());
    assert!(original.upgrade().is_none());
    let reopened =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    assert_eq!(
        reconstruct_projection(&reopened.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected
    );
}

#[test]
fn provider_capture_defers_real_insert_and_retention_without_blocking_readers() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let first = projection(7);
    let second = advance_projection(&first, 8);
    archive.insert(first.clone()).unwrap();
    archive.insert(second.clone()).unwrap();
    let (_, _, proposal) = prepared_compaction_for_test(&archive, second.key.clone());
    let third = advance_projection(&second, 9);
    let mut capture = archive.prepare_insert(third.clone()).unwrap();
    let wait = match archive.insert(third.clone()) {
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { wait }) => wait,
        _ => panic!("ordinary insertion must defer to the retained predecessor"),
    };
    assert!(matches!(
        archive.prepare_insert(third.clone()),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. })
    ));
    let authority = TestRetentionAuthority::new();
    let kura = Kura::blank_kura_for_testing();
    assert!(matches!(
        archive.approve_and_install_kura_authenticated_compaction(
            &proposal,
            &kura,
            &authority.binding(),
            &authority
        ),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. })
    ));
    assert!(authority.latest.lock().unwrap().is_none());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 2);
    assert_eq!(fs::read_dir(&archive.checkpoints).unwrap().count(), 0);
    let before = archive
        .read_provider_page(&second.key, PROVIDER_A, None, 1)
        .unwrap();
    assert!(archive.index.try_write().is_ok());
    assert_eq!(
        capture.persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(
        archive
            .read_provider_page(&second.key, PROVIDER_A, None, 1)
            .unwrap(),
        before
    );
    assert!(
        !wait.is_released(),
        "published capture retains custody for exact retry"
    );
    assert!(matches!(
        archive.write_index(),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. })
    ));
    drop(capture);
    assert!(wait.is_released());
    assert_eq!(
        archive.insert(third).unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
}

#[test]
fn provider_capture_rejects_foreign_owner_and_directory_substitution_before_publication() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let foreign_directory = physical_tempdir().unwrap();
    let foreign =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&foreign_directory), bounds())
            .unwrap();
    let mut prepared = archive.prepare_insert(projection(7)).unwrap();
    assert!(matches!(
        foreign.write_reserved_index(&prepared.reservation),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureOwnerMismatch)
    ));
    assert!(foreign.read_index().unwrap().by_height.is_empty());
    let original_records = archive.root.join("retained-records");
    fs::rename(&archive.records, &original_records).unwrap();
    fs::create_dir(&archive.records).unwrap();
    assert!(prepared.persist().is_err());
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&original_records).unwrap().count(), 0);
    assert!(archive.capture_gate.ensure_unreserved().is_err());
    fs::remove_dir(&archive.records).unwrap();
    fs::rename(&original_records, &archive.records).unwrap();
    assert_eq!(
        prepared.persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
}

#[tokio::test]
async fn provider_capture_wait_tracks_original_release_not_later_reservation() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let projection = projection(7);
    let first = archive.prepare_insert(projection.clone()).unwrap();
    let mut wait = match archive.insert(projection.clone()) {
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { wait }) => wait,
        _ => panic!("capture must retain an exact release event"),
    };
    drop(first);
    let second = archive.prepare_insert(projection).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), wait.wait_for_release())
        .await
        .unwrap();
    assert!(wait.is_released());
    assert!(archive.capture_gate.ensure_unreserved().is_err());
    drop(second);
    assert!(archive.write_index().is_ok());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
}
