// Admitted archive persistence controls; these lower-level fixtures confer no consensus finality.
#[test]
fn provider_candidate_reservation_freezes_exact_cut_without_holding_index() {
    use std::{
        future::Future as _,
        pin::Pin,
        task::{Context, Waker},
    };

    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let kura = Kura::blank_kura_for_testing();
    let first = projection(7);
    archive.insert(first.clone()).unwrap();
    let successor = advance_projection(&first, 8);
    let before_bytes = archive.read_index().unwrap().total_bytes;

    let reader = archive.read_index().unwrap();
    let wait = match archive.try_reserve_candidate(successor.key, &kura) {
        Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { wait }) => wait,
        _ => panic!("preexecution reservation must not wait on a physical archive reader"),
    };
    archive.capture_gate.ensure_unreserved().unwrap();
    let mut released = wait.wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut released).poll(&mut context).is_pending());
    drop(reader);
    assert!(Pin::new(&mut released).poll(&mut context).is_ready());

    let candidate = archive.try_reserve_candidate(successor.key, &kura).unwrap();
    assert_eq!(candidate.key, successor.key);
    assert!(Arc::ptr_eq(&candidate.kura, &kura));
    let wait = archive.capture_gate.ensure_unreserved().unwrap_err();
    assert!(matches!(
        archive.try_reserve_candidate(successor.key, &kura),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. })
    ));
    assert!(matches!(
        archive.insert(successor),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. })
    ));
    // Committed readers and their exact predecessor remain available.
    let index = archive.read_index().unwrap();
    assert_eq!(
        reconstruct_projection(&index, &first.key, bounds()).unwrap(),
        first
    );
    assert_eq!(index.generation, 1);
    assert_eq!(index.total_bytes, before_bytes);
    drop(index);
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 1);
    assert!(!wait.is_released());
    let candidate = match candidate.into_prepared() {
        Err(original) => original,
        Ok(_) => panic!("reservation alone cannot manufacture an admitted plan"),
    };
    assert!(!wait.is_released());
    drop(candidate);
    assert!(wait.is_released());
    archive.capture_gate.ensure_unreserved().unwrap();
}

#[test]
fn provider_candidate_reservation_rejects_fork_and_gap_before_claim() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let kura = Kura::blank_kura_for_testing();
    let first = projection(7);
    archive.insert(first.clone()).unwrap();
    for altered_key in [
        ProviderIngestFinalizedArchiveKeyV1 {
            block_hash: [0xD1; 32],
            ..first.key
        },
        ProviderIngestFinalizedArchiveKeyV1 {
            finalized_at_unix_ms: first.key.finalized_at_unix_ms + 1,
            ..first.key
        },
    ] {
        assert!(matches!(
            archive.try_reserve_candidate(altered_key, &kura),
            Err(ProviderIngestFinalizedArchiveErrorV1::FinalizedFork { height: 7, .. })
        ));
        archive.capture_gate.ensure_unreserved().unwrap();
    }
    assert!(matches!(
        archive.try_reserve_candidate(key(9), &kura),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveCoverageGap {
            missing_height: 8,
            observed_height: 9,
            ..
        })
    ));
    archive.capture_gate.ensure_unreserved().unwrap();
    // Exact replay and the sole successor both retain the real predecessor.
    drop(archive.try_reserve_candidate(first.key, &kura).unwrap());
    drop(archive.try_reserve_candidate(key(8), &kura).unwrap());
    assert_eq!(archive.read_index().unwrap().generation, 1);
}

#[test]
fn provider_candidate_plan_refusal_retains_original_projection_and_reservation() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let kura = Kura::blank_kura_for_testing();
    let expected = projection(7);
    let mut candidate = archive.try_reserve_candidate(expected.key, &kura).unwrap();
    // This archive-local fixture supplies projection data only. Original State
    // capture is exercised by the joint carrier tests; all admission, storage
    // checks, predecessor custody and encoding below use the production path.
    candidate.capture_attempted = true;
    candidate.projection = Some(expected.clone());
    let original_projection = candidate.projection.as_ref().unwrap().providers.as_ptr();
    let reservation = archive.capture_gate.ensure_unreserved().unwrap_err();
    let reader = archive.read_index().unwrap();
    assert!(matches!(
        candidate.try_prepare(),
        Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. })
    ));
    drop(reader);
    assert!(candidate.plan.is_none());
    let mut candidate = match candidate.into_prepared() {
        Err(original) => original,
        Ok(_) => panic!("refused preparation must return the original candidate owner"),
    };
    assert_eq!(
        candidate.projection.as_ref().unwrap().providers.as_ptr(),
        original_projection
    );
    assert!(!reservation.is_released());

    let held_records = directory.path().join("original-records");
    fs::rename(&archive.records, &held_records).unwrap();
    fs::create_dir(&archive.records).unwrap();
    assert!(candidate.try_prepare().is_err());
    assert!(candidate.plan.is_none());
    assert_eq!(candidate.projection.as_ref().unwrap(), &expected);
    assert_eq!(
        candidate.projection.as_ref().unwrap().providers.as_ptr(),
        original_projection
    );
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert!(!reservation.is_released());
    fs::remove_dir(&archive.records).unwrap();
    fs::rename(&held_records, &archive.records).unwrap();

    candidate.try_prepare().unwrap();
    let original_bytes = candidate
        .plan
        .as_ref()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .bytes
        .as_ptr();
    candidate.try_prepare().unwrap();
    assert_eq!(
        candidate
            .plan
            .as_ref()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr(),
        original_bytes
    );
    let mut prepared = match candidate.into_prepared() {
        Ok(prepared) => prepared,
        Err(_) => panic!("successful planning must hand off its original reservation"),
    };
    assert_eq!(
        prepared
            .insertion
            .plan
            .record
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr(),
        original_bytes
    );
    assert!(!reservation.is_released());
    assert_eq!(archive.read_index().unwrap().generation, 0);
    // Lower-level archive persistence does not confer consensus finality.
    assert_eq!(
        prepared.insertion.try_persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(
        reconstruct_projection(&archive.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected
    );
    assert!(!reservation.is_released());
    drop(prepared);
    assert!(reservation.is_released());
}

#[test]
fn provider_candidate_failed_original_capture_is_never_retried() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let kura = Kura::blank_kura_for_testing();
    let state = Box::new(crate::state::State::new_for_testing(
        crate::state::World::default(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
    ));
    let mut candidate = archive.try_reserve_candidate(key(1), &kura).unwrap();
    let reservation = archive.capture_gate.ensure_unreserved().unwrap_err();
    let view = state.view();
    assert!(matches!(
        candidate.capture_original(&view),
        Err(ProviderIngestFinalizedArchiveErrorV1::FinalityAuthentication { .. })
    ));
    assert!(candidate.capture_attempted);
    assert!(candidate.projection.is_none());
    assert!(matches!(
        candidate.capture_original(&view),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
            reason: "candidate archive capture was already attempted",
        })
    ));
    assert!(candidate.try_prepare().is_err());
    assert!(!reservation.is_released());
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    drop(candidate);
    assert!(reservation.is_released());
}

#[test]
fn provider_candidate_reservation_poison_is_storage_refusal_without_claim() {
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    let kura = Kura::blank_kura_for_testing();
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _writer = archive.index.write().unwrap();
            panic!("poison candidate archive index");
        }))
        .is_err()
    );
    assert!(matches!(
        archive.try_reserve_candidate(key(1), &kura),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveLockPoisoned)
    ));
    archive.capture_gate.ensure_unreserved().unwrap();
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
}

fn provider_capture_with_durable_finality() -> (
    tempfile::TempDir,
    Arc<ProviderIngestFinalizedArchiveV1>,
    PreparedProviderIngestCapture,
    ProviderIngestFinalizedProjectionV1,
    KuraV2CommitReceipt,
) {
    let (kura, block, finality, receipt) = crate::kura::tests::carrier_checkpoint_receipt_fixture();
    let directory = physical_tempdir().unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap(),
    );
    // This archive-local projection tests durable authentication and custody,
    // not execution or permission to publish State.
    let mut expected = projection(block.header().height().get());
    expected.key = ProviderIngestFinalizedArchiveKeyV1::try_new(
        finality.height_context.network_id,
        block.header().height().get(),
        *block.hash().as_ref(),
        block.header().creation_time_ms,
    )
    .unwrap();
    let capture = PreparedProviderIngestCapture {
        insertion: archive.prepare_insert(expected.clone()).unwrap(),
        kura: Arc::clone(&kura),
    };
    (directory, archive, capture, expected, receipt)
}

#[test]
fn prepared_provider_capture_authenticates_under_held_kura_lease_and_retains_retry() {
    let (_directory, archive, mut capture, expected, receipt) =
        provider_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let record = capture.insertion.plan.record.as_ref().unwrap();
    let path = record.entry.path.clone();
    let bytes = record.bytes.clone();
    let original_buffer = record.bytes.as_ptr();
    let expected_total = record.total_bytes;
    let wait = archive.capture_gate.ensure_unreserved().unwrap_err();
    let foreign = Kura::blank_kura_for_testing();
    let foreign_lease = foreign.try_publication_lease().unwrap();
    assert!(matches!(
        capture.publish_under_publication_lease(&foreign_lease, &receipt),
        Err(ProviderIngestFinalizedArchiveErrorV1::FinalityAuthentication { .. })
    ));
    drop(foreign_lease);
    let lease = kura.try_publication_lease().unwrap();
    assert!(kura.try_publication_lease().is_err());
    let network = capture.insertion.plan.key.network_id;
    capture.insertion.plan.key.network_id = test_network_id(0xFA);
    assert!(matches!(
        capture.publish_under_publication_lease(&lease, &receipt),
        Err(ProviderIngestFinalizedArchiveErrorV1::FinalityAuthentication { .. })
    ));
    capture.insertion.plan.key.network_id = network;
    capture.insertion.plan.key.finalized_at_unix_ms += 1;
    assert!(matches!(
        capture.publish_under_publication_lease(&lease, &receipt),
        Err(ProviderIngestFinalizedArchiveErrorV1::FinalityAuthentication { .. })
    ));
    capture.insertion.plan.key.finalized_at_unix_ms -= 1;
    capture
        .reauthenticate_under_publication_lease(&lease, &receipt)
        .unwrap();
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert!(!wait.is_released());
    fs::create_dir(&path).unwrap();
    assert!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .is_err()
    );
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert_eq!(archive.read_index().unwrap().total_bytes, 0);
    assert_eq!(
        capture
            .insertion
            .plan
            .record
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr(),
        original_buffer
    );
    assert!(!wait.is_released());
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert!(!wait.is_released());
    drop(lease);
    let lease = kura.try_publication_lease().unwrap();
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    drop(lease);
    drop(capture);
    assert!(wait.is_released());
    assert_eq!(
        reconstruct_projection(&archive.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected
    );
}

#[test]
fn prepared_provider_capture_index_busy_releases_kura_and_retries_original_owner() {
    use std::{
        future::Future as _,
        pin::Pin,
        task::{Context, Waker},
    };

    let (_directory, archive, mut capture, expected, receipt) =
        provider_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let original_buffer = capture
        .insertion
        .plan
        .record
        .as_ref()
        .unwrap()
        .bytes
        .as_ptr();
    let original_bytes = capture
        .insertion
        .plan
        .record
        .as_ref()
        .unwrap()
        .bytes
        .clone();
    let reservation = archive.capture_gate.ensure_unreserved().unwrap_err();
    std::thread::scope(|scope| {
        let (held, acquired) = std::sync::mpsc::channel();
        let (progress, progressed) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let reader_archive = Arc::clone(&archive);
        let reader_kura = Arc::clone(&kura);
        let height = receipt.height();
        let lease = kura.try_publication_lease().unwrap();
        let reader = scope.spawn(move || {
            let index = reader_archive.read_index().unwrap();
            held.send(()).unwrap();
            // This is the real index -> Kura edge in archive qualification.
            reader_kura
                .v2_finality_artifact_with_receipt(height)
                .unwrap()
                .unwrap();
            progress.send(()).unwrap();
            released.recv().unwrap();
            drop(index);
        });
        acquired.recv().unwrap();
        let wait = match capture.publish_under_publication_lease(&lease, &receipt) {
            Err(ProviderIngestFinalizedArchiveErrorV1::IndexBusy { wait }) => wait,
            other => panic!("held archive reader must return its actual release wait: {other:?}"),
        };
        assert_eq!(
            capture
                .insertion
                .plan
                .record
                .as_ref()
                .unwrap()
                .bytes
                .as_ptr(),
            original_buffer
        );
        assert_eq!(
            capture.insertion.plan.record.as_ref().unwrap().bytes,
            original_bytes
        );
        assert!(!reservation.is_released());
        assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
        // Release every outer fence before waiting for the blocked reader.
        drop(lease);
        progressed.recv().unwrap();
        let mut wait = wait.wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        release.send(()).unwrap();
        reader.join().unwrap();
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    });
    assert!(!reservation.is_released());
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert_eq!(archive.read_index().unwrap().total_bytes, 0);
    let lease = kura.try_publication_lease().unwrap();
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().generation, 1);
    drop(lease);
    drop(capture);
    assert!(reservation.is_released());
    assert_eq!(
        reconstruct_projection(&archive.read_index().unwrap(), &expected.key, bounds()).unwrap(),
        expected,
    );
}

#[test]
fn prepared_provider_capture_index_poison_is_storage_failure_not_busy() {
    let (_directory, archive, mut capture, _expected, receipt) =
        provider_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let original_buffer = capture
        .insertion
        .plan
        .record
        .as_ref()
        .unwrap()
        .bytes
        .as_ptr();
    let reservation = archive.capture_gate.ensure_unreserved().unwrap_err();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _writer = archive.index.write().unwrap();
            panic!("poison original archive index writer");
        }))
        .is_err()
    );
    let lease = kura.try_publication_lease().unwrap();
    assert!(matches!(
        capture.publish_under_publication_lease(&lease, &receipt),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveLockPoisoned)
    ));
    assert!(matches!(
        archive.read_index(),
        Err(ProviderIngestFinalizedArchiveErrorV1::ArchiveLockPoisoned)
    ));
    assert_eq!(
        capture
            .insertion
            .plan
            .record
            .as_ref()
            .unwrap()
            .bytes
            .as_ptr(),
        original_buffer
    );
    assert!(!reservation.is_released());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    drop(lease);
    drop(capture);
    assert!(reservation.is_released());
}

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
        prepared.try_persist().is_err(),
        "an obstructed immutable path is a storage failure"
    );
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert_eq!(archive.read_index().unwrap().total_bytes, 0);
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        prepared.try_persist().unwrap(),
        ProviderIngestFinalizedArchiveInsertOutcomeV1::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        prepared.try_persist().unwrap(),
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
            prepared.try_persist().unwrap(),
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
        capture.try_persist().unwrap(),
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
        foreign.try_write_reserved_index(&prepared.reservation),
        Err(ProviderIngestFinalizedArchiveErrorV1::CaptureOwnerMismatch)
    ));
    assert!(foreign.read_index().unwrap().by_height.is_empty());
    let original_records = archive.root.join("retained-records");
    fs::rename(&archive.records, &original_records).unwrap();
    fs::create_dir(&archive.records).unwrap();
    assert!(prepared.try_persist().is_err());
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert_eq!(fs::read_dir(&archive.records).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&original_records).unwrap().count(), 0);
    assert!(archive.capture_gate.ensure_unreserved().is_err());
    fs::remove_dir(&archive.records).unwrap();
    fs::rename(&original_records, &archive.records).unwrap();
    assert_eq!(
        prepared.try_persist().unwrap(),
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
