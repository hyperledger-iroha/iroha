// These archive-local projection fixtures exercise persistence, not consensus finality.
#[test]
fn reputation_candidate_reserves_before_execution_and_observes_actual_reader_release() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = sample_projection(7, [0x71; 32]);
    let kura = Kura::blank_kura_for_testing();
    for timestamp in [0, u64::MAX] {
        assert!(matches!(
            archive.try_reserve_candidate(projection.key.clone(), timestamp, &kura),
            Err(ReputationFinalizedArchiveError::FinalityAuthentication { .. })
        ));
        assert!(archive.capture_gate.ensure_unreserved().is_ok());
    }
    let reader = archive.read_index().unwrap();
    let error = archive
        .try_reserve_candidate(
            projection.key.clone(),
            projection.finalized_at_unix_ms,
            &kura,
        )
        .unwrap_err();
    let ReputationFinalizedArchiveError::IndexBusy { wait } = error else {
        panic!("actual held reader must refuse preexecution reservation");
    };
    let mut released = Box::pin(wait.wait_for_release());
    assert!(matches!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(reader);
    assert!(matches!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
    let owner = archive
        .try_reserve_candidate(
            projection.key.clone(),
            projection.finalized_at_unix_ms,
            &kura,
        )
        .unwrap();
    let wait = archive.capture_gate.ensure_unreserved().unwrap_err();
    assert!(!wait.is_released());
    assert!(
        archive.index.try_write().is_ok(),
        "reservation retains no physical guard"
    );
    assert!(matches!(
        archive.insert(projection),
        Err(ReputationFinalizedArchiveError::CaptureReserved { .. })
    ));
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(owner);
    assert!(wait.is_released());
}

#[test]
fn reputation_candidate_keeps_original_successor_through_plan_refusals() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<ReputationCandidateCapture>();
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = projection_with_all_feeds(7, [0x71; 32]);
    let kura = Kura::blank_kura_for_testing();
    let mut owner = archive
        .try_reserve_candidate(
            projection.key.clone(),
            projection.finalized_at_unix_ms,
            &kura,
        )
        .unwrap();
    let history = vec![projection.authority_policy.clone()];
    let next_state = reconstruction_state_from_full_successor(None, &projection, &history).unwrap();
    // Archive-local material only: successful original-State capture is exercised
    // by the carrier-journals tests, and this fixture grants no finality.
    owner.capture_attempted = true;
    owner.captured = Some(CapturedReputationCandidate {
        next_state,
        authority_policy_history: history,
    });
    let original = owner
        .captured
        .as_ref()
        .unwrap()
        .next_state
        .journal_events
        .retained_suffix
        .as_ptr();
    let reservation = archive.capture_gate.ensure_unreserved().unwrap_err();
    for _ in 0..2 {
        archive.with_index_reader_for_test(|| {
            assert!(matches!(
                owner.try_prepare(),
                Err(ReputationFinalizedArchiveError::IndexBusy { .. })
            ));
        });
        assert_eq!(
            owner
                .captured
                .as_ref()
                .unwrap()
                .next_state
                .journal_events
                .retained_suffix
                .as_ptr(),
            original
        );
        assert!(!reservation.is_released());
        assert!(owner.plan.is_none());
    }
    owner.try_prepare().unwrap();
    let prepared = owner.plan.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(
        prepared.next_state.journal_events.retained_suffix.as_ptr(),
        original
    );
    let bytes = prepared.anchor_bytes.as_ptr();
    owner.try_prepare().unwrap();
    assert_eq!(
        owner
            .plan
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .anchor_bytes
            .as_ptr(),
        bytes
    );
    let prepared = owner.into_prepared().unwrap();
    assert_eq!(
        prepared
            .insertion
            .state
            .as_ref()
            .unwrap()
            .anchor_bytes
            .as_ptr(),
        bytes
    );
    assert!(!reservation.is_released());
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(prepared);
    assert!(reservation.is_released());
}

#[test]
fn reputation_candidate_retains_original_predecessor_and_rejects_fork_before_capture() {
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = projection_with_all_feeds(7, [0x71; 32]);
    archive.insert(projection.clone()).unwrap();
    let kura = Kura::blank_kura_for_testing();
    let mut fork = projection.key.clone();
    fork.block_hash = [0x72; 32];
    assert!(matches!(
        archive.try_reserve_candidate(fork, projection.finalized_at_unix_ms, &kura),
        Err(ReputationFinalizedArchiveError::FinalizedFork { .. })
    ));
    assert!(archive.capture_gate.ensure_unreserved().is_ok());
    let successor =
        ReputationFinalizedArchiveKeyV1::try_new(projection.key.network_id, 8, [0x81; 32]).unwrap();
    let owner = archive
        .try_reserve_candidate(successor, projection.finalized_at_unix_ms + 1, &kura)
        .unwrap();
    let previous = owner.previous.as_ref().unwrap();
    assert_eq!(previous.key, projection.key);
    assert_eq!(previous.full_projection().unwrap(), projection);
    assert!(matches!(
        archive.write_index(),
        Err(ReputationFinalizedArchiveError::CaptureReserved { .. })
    ));
    assert_eq!(owner.generation, archive.read_index().unwrap().generation);
}

fn reputation_capture_with_durable_finality() -> (
    tempfile::TempDir,
    Arc<ReputationFinalizedArchive>,
    PreparedReputationCapture,
    ReputationFinalizedProjectionV1,
    KuraV2CommitReceipt,
) {
    let (kura, block, finality, receipt) = crate::kura::tests::carrier_checkpoint_receipt_fixture();
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let mut expected = sample_projection(block.header().height().get(), *block.hash().as_ref());
    expected.key = ReputationFinalizedArchiveKeyV1::try_new(
        finality.height_context.network_id,
        block.header().height().get(),
        *block.hash().as_ref(),
    )
    .unwrap();
    expected.finalized_at_unix_ms = block.header().creation_time_ms;
    let capture = PreparedReputationCapture {
        insertion: archive
            .prepare_insert(expected.clone())
            .unwrap()
            .detach(Arc::clone(&archive))
            .unwrap(),
        kura: Arc::clone(&kura),
    };
    (directory, archive, capture, expected, receipt)
}

#[test]
fn prepared_reputation_capture_authenticates_under_held_kura_lease_and_retains_retry() {
    let (_directory, archive, mut capture, expected, receipt) =
        reputation_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let prepared = capture.insertion.state.as_ref().unwrap();
    let path = prepared.anchor_path.clone();
    let bytes = prepared.anchor_bytes.clone();
    let original_buffer = prepared.anchor_bytes.as_ptr();
    let expected_total = prepared.total_bytes;
    let policy_bytes = bounded_bytes_len(&prepared.policies[0].bytes);
    let wait = archive.capture_gate.ensure_unreserved().unwrap_err();
    let foreign = Kura::blank_kura_for_testing();
    let foreign_lease = foreign.try_publication_lease().unwrap();
    assert!(matches!(
        capture.publish_under_publication_lease(&foreign_lease, &receipt),
        Err(ReputationFinalizedArchiveError::FinalityAuthentication { .. })
    ));
    drop(foreign_lease);
    let lease = kura.try_publication_lease().unwrap();
    assert!(kura.try_publication_lease().is_err());
    let network = capture.insertion.key.network_id;
    capture.insertion.key.network_id = network_id(0xFA);
    assert!(matches!(
        capture.publish_under_publication_lease(&lease, &receipt),
        Err(ReputationFinalizedArchiveError::FinalityAuthentication { .. })
    ));
    capture.insertion.key.network_id = network;
    capture.insertion.finalized_at_unix_ms += 1;
    assert!(matches!(
        capture.publish_under_publication_lease(&lease, &receipt),
        Err(ReputationFinalizedArchiveError::FinalityAuthentication { .. })
    ));
    capture.insertion.finalized_at_unix_ms -= 1;
    capture
        .reauthenticate_under_publication_lease(&lease, &receipt)
        .unwrap();
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert!(!wait.is_released());
    // The actual policy write succeeds before the blocked anchor. Retry must
    // retain that progress and never charge the policy a second time.
    fs::create_dir(&path).unwrap();
    assert!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .is_err()
    );
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert_eq!(archive.read_index().unwrap().total_bytes, policy_bytes);
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(
        capture
            .insertion
            .state
            .as_ref()
            .unwrap()
            .anchor_bytes
            .as_ptr(),
        original_buffer
    );
    assert!(!wait.is_released());
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ReputationFinalizedArchiveInsertOutcome::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert!(!wait.is_released());
    drop(lease);
    let lease = kura.try_publication_lease().unwrap();
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ReputationFinalizedArchiveInsertOutcome::ExactReplay
    );
    drop(lease);
    drop(capture);
    assert!(wait.is_released());
    assert_eq!(archive.get_exact(&expected.key).unwrap(), Some(expected));
}

#[test]
fn prepared_reputation_capture_index_busy_releases_kura_and_retries_original_owner() {
    use std::{
        future::Future as _,
        pin::Pin,
        task::{Context, Waker},
    };

    let (_directory, archive, mut capture, expected, receipt) =
        reputation_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let original_buffer = capture
        .insertion
        .state
        .as_ref()
        .unwrap()
        .anchor_bytes
        .as_ptr();
    let original_bytes = capture
        .insertion
        .state
        .as_ref()
        .unwrap()
        .anchor_bytes
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
            let (_, receipt) = reader_kura
                .v2_finality_artifact_with_receipt(height)
                .unwrap()
                .unwrap();
            assert_eq!(receipt.height(), height);
            progress.send(()).unwrap();
            released.recv().unwrap();
            drop(index);
        });
        acquired.recv().unwrap();
        let wait = match capture.publish_under_publication_lease(&lease, &receipt) {
            Err(ReputationFinalizedArchiveError::IndexBusy { wait }) => wait,
            other => panic!("held archive reader must return its actual release wait: {other:?}"),
        };
        assert_eq!(
            capture
                .insertion
                .state
                .as_ref()
                .unwrap()
                .anchor_bytes
                .as_ptr(),
            original_buffer
        );
        assert_eq!(
            capture.insertion.state.as_ref().unwrap().anchor_bytes,
            original_bytes
        );
        assert!(!reservation.is_released());
        assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
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
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(
        capture
            .publish_under_publication_lease(&lease, &receipt)
            .unwrap(),
        ReputationFinalizedArchiveInsertOutcome::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().generation, 1);
    drop(lease);
    drop(capture);
    assert!(reservation.is_released());
    assert_eq!(archive.get_exact(&expected.key).unwrap(), Some(expected));
}

#[test]
fn prepared_reputation_capture_index_poison_is_storage_failure_not_busy() {
    let (_directory, archive, mut capture, _expected, receipt) =
        reputation_capture_with_durable_finality();
    let kura = Arc::clone(&capture.kura);
    let original_buffer = capture
        .insertion
        .state
        .as_ref()
        .unwrap()
        .anchor_bytes
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
        Err(ReputationFinalizedArchiveError::InvalidStorage {
            reason: "archive index lock is poisoned",
            ..
        })
    ));
    assert!(matches!(
        archive.read_index(),
        Err(ReputationFinalizedArchiveError::InvalidStorage {
            reason: "archive index lock is poisoned",
            ..
        })
    ));
    assert_eq!(
        capture
            .insertion
            .state
            .as_ref()
            .unwrap()
            .anchor_bytes
            .as_ptr(),
        original_buffer
    );
    assert!(!reservation.is_released());
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(lease);
    drop(capture);
    assert!(reservation.is_released());
}

#[test]
fn prepared_reputation_capture_reserves_writer_and_drop_has_no_effects() {
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = sample_projection(7, [0x71; 32]);
    let prepared = archive
        .prepare_insert(projection)
        .unwrap()
        .detach(Arc::clone(&archive))
        .unwrap();
    assert!(
        archive.index.try_write().is_ok(),
        "logical custody releases the original physical writer"
    );
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert!(archive.read_index().unwrap().policies.is_empty());
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(prepared);
    assert!(archive.index.try_write().is_ok());
    assert!(archive.is_empty().unwrap());
}

#[test]
fn reputation_admission_rejects_fork_gap_capacity_and_generation_before_writes() {
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let first = sample_projection(7, [0x71; 32]);
    archive.insert(first.clone()).unwrap();
    assert!(matches!(
        archive.prepare_insert(sample_projection(7, [0x72; 32])),
        Err(ReputationFinalizedArchiveError::FinalizedFork { .. })
    ));
    let gap = ReputationReconstructionStateV1::from_projection(sample_projection(9, [0x91; 32]));
    assert!(matches!(
        archive.prepare_captured_state(gap, vec![first.authority_policy.clone()]),
        Err(ReputationFinalizedArchiveError::ArchiveCoverageGap { .. })
    ));
    archive.write_index().unwrap().generation = u64::MAX;
    assert!(matches!(
        archive.prepare_insert(sample_projection(8, [0x81; 32])),
        Err(ReputationFinalizedArchiveError::RetentionRequired { .. })
    ));
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 1);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 1);

    let directory = tempdir().unwrap();
    let archive = open_archive(
        &directory,
        ReputationFinalizedArchiveBounds::try_new(1 << 20, 1, 16 << 20).unwrap(),
    );
    archive.insert(first).unwrap();
    assert!(matches!(
        archive.prepare_insert(sample_projection(8, [0x81; 32])),
        Err(ReputationFinalizedArchiveError::RetentionRequired { .. })
    ));
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 1);
}

#[test]
fn prepared_reputation_capture_retries_after_policy_write_without_double_accounting() {
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = sample_projection(7, [0x71; 32]);
    let mut prepared = archive
        .prepare_insert(projection.clone())
        .unwrap()
        .detach(Arc::clone(&archive))
        .unwrap();
    let state = prepared.state.as_ref().unwrap();
    let path = state.anchor_path.clone();
    let bytes = state.anchor_bytes.clone();
    let expected_total = state.total_bytes;
    let policy_bytes = bounded_bytes_len(&state.policies[0].bytes);
    fs::create_dir(&path).unwrap();
    assert!(
        prepared.try_persist().is_err(),
        "anchor obstruction occurs after the policy write"
    );
    assert!(archive.read_index().unwrap().by_height.is_empty());
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(archive.read_index().unwrap().policies.len(), 1);
    assert_eq!(archive.read_index().unwrap().total_bytes, policy_bytes);
    assert_eq!(archive.read_index().unwrap().generation, 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 1);
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        prepared.try_persist().unwrap(),
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(archive.read_index().unwrap().anchor_count, 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        prepared.try_persist().unwrap(),
        ReputationFinalizedArchiveInsertOutcome::ExactReplay
    );
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(archive.read_index().unwrap().anchor_count, 1);
    drop(prepared);
    drop(archive);
    let reopened = open_archive(&directory, bounds());
    let (restored, _) = reopened
        .latest_at_or_before_with_policy_history(&projection.key.network_id, projection.key.height)
        .unwrap()
        .unwrap();
    assert_eq!(restored, projection);
    let index = reopened.read_index().unwrap();
    assert_eq!(index.total_bytes, expected_total);
    assert_eq!(index.policy_count, 1);
    assert_eq!(index.anchor_count, 1);
}

#[tokio::test]
async fn detached_reputation_owner_defers_writers_with_exact_release_and_keeps_readers_available() {
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let projection = sample_projection(7, [0x71; 32]);
    let prepared = archive
        .prepare_insert(projection.clone())
        .unwrap()
        .detach(Arc::clone(&archive))
        .unwrap();
    assert!(archive.index.try_write().is_ok());
    assert!(archive.is_empty().unwrap());
    assert!(archive.get_exact(&projection.key).unwrap().is_none());
    let mut wait = match archive.insert(projection.clone()) {
        Err(ReputationFinalizedArchiveError::CaptureReserved { wait }) => wait,
        _ => panic!("competing insertion must preserve the original capacity owner"),
    };
    assert!(matches!(
        archive.write_index(),
        Err(ReputationFinalizedArchiveError::CaptureReserved { .. })
    ));
    assert!(!wait.is_released());
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(prepared);
    tokio::time::timeout(std::time::Duration::from_secs(1), wait.wait_for_release())
        .await
        .unwrap();
    assert!(wait.is_released());
    assert_eq!(
        archive.insert(projection.clone()).unwrap(),
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(
        archive.get_exact(&projection.key).unwrap(),
        Some(projection)
    );
}

#[test]
fn detached_reputation_custody_rejects_another_archive_even_with_equal_projection() {
    let directory = tempdir().unwrap();
    let foreign_directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let foreign = Arc::new(open_archive(&foreign_directory, bounds()));
    let projection = sample_projection(7, [0x71; 32]);
    assert!(matches!(
        archive
            .prepare_insert(projection.clone())
            .unwrap()
            .detach(Arc::clone(&foreign)),
        Err(ReputationFinalizedArchiveError::CaptureOwnerMismatch)
    ));
    assert!(archive.write_index().is_ok());
    assert!(foreign.write_index().is_ok());
    let prepared = archive
        .prepare_insert(projection)
        .unwrap()
        .detach(Arc::clone(&archive))
        .unwrap();
    assert!(matches!(
        foreign.try_write_reserved_index(&prepared.reservation),
        Err(ReputationFinalizedArchiveError::CaptureOwnerMismatch)
    ));
    assert!(
        archive
            .try_write_reserved_index(&prepared.reservation)
            .is_ok()
    );
    assert!(archive.is_empty().unwrap());
    assert!(foreign.is_empty().unwrap());
}

#[test]
fn detached_reputation_insertion_retains_real_archive_ownership_across_worker_handoff() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<OwnedReputationInsertion>();
    assert_static_send::<PreparedReputationCapture>();
    let directory = tempdir().unwrap();
    let archive = Arc::new(open_archive(&directory, bounds()));
    let weak = Arc::downgrade(&archive);
    let projection = sample_projection(7, [0x71; 32]);
    let prepared = archive
        .prepare_insert(projection.clone())
        .unwrap()
        .detach(Arc::clone(&archive))
        .unwrap();
    drop(archive);
    assert!(weak.upgrade().is_some());
    assert!(
        ReputationFinalizedArchive::try_open_unsealed_for_test(archive_root(&directory), bounds())
            .is_err(),
        "retained custody includes the real exclusive filesystem owner"
    );
    let prepared = std::thread::spawn(move || prepared).join().unwrap();
    assert_eq!(prepared.key, projection.key);
    assert!(prepared.archive.is_empty().unwrap());
    drop(prepared);
    assert!(weak.upgrade().is_none());
    let reopened = open_archive(&directory, bounds());
    assert!(reopened.is_empty().unwrap());
}
