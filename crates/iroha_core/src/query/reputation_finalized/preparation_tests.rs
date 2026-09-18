// These archive-local projection fixtures exercise persistence, not consensus finality.
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
        prepared.persist().is_err(),
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
        prepared.persist().unwrap(),
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(archive.read_index().unwrap().total_bytes, expected_total);
    assert_eq!(archive.read_index().unwrap().policy_count, 1);
    assert_eq!(archive.read_index().unwrap().anchor_count, 1);
    assert_eq!(archive.read_index().unwrap().generation, 1);
    assert_eq!(
        prepared.persist().unwrap(),
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
        foreign.write_reserved_index(&prepared.reservation),
        Err(ReputationFinalizedArchiveError::CaptureOwnerMismatch)
    ));
    assert!(archive.write_reserved_index(&prepared.reservation).is_ok());
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
