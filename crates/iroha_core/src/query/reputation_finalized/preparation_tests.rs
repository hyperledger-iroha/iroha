// Persistence refusals and retries over the active physical index and insertion path.

#[test]
fn prepared_reputation_insertion_retries_partial_io_without_reallocating_or_double_accounting() {
    let directory = tempdir().unwrap();
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    let mut prepared = archive.prepare_insert(projection.clone()).unwrap();
    let state = prepared.state.as_ref().unwrap();
    let path = state.anchor_path.clone();
    let bytes = state.anchor_bytes.clone();
    let original_buffer = state.anchor_bytes.as_ptr();
    let expected_total = state.total_bytes;
    let policy_bytes = bounded_bytes_len(&state.policies[0].bytes);
    assert!(prepared.index.by_height.is_empty());
    assert_eq!(prepared.index.generation, 0);
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    fs::create_dir(&path).unwrap();
    for _ in 0..2 {
        assert!(
            prepared.persist().is_err(),
            "anchor obstruction follows policy publication"
        );
        assert!(prepared.index.by_height.is_empty());
        assert_eq!(prepared.index.policy_count, 1);
        assert_eq!(prepared.index.policies.len(), 1);
        assert_eq!(prepared.index.total_bytes, policy_bytes);
        assert_eq!(prepared.index.generation, 0);
        assert_eq!(
            prepared.state.as_ref().unwrap().anchor_bytes.as_ptr(),
            original_buffer
        );
        assert_eq!(prepared.state.as_ref().unwrap().anchor_bytes, bytes);
        assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 1);
    }
    fs::remove_dir(&path).unwrap();
    assert_eq!(
        prepared.persist().unwrap(),
        ReputationFinalizedArchiveInsertOutcome::Inserted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(prepared.index.total_bytes, expected_total);
    assert_eq!(prepared.index.policy_count, 1);
    assert_eq!(prepared.index.anchor_count, 1);
    assert_eq!(prepared.index.generation, 1);
    assert_eq!(
        prepared.persist().unwrap(),
        ReputationFinalizedArchiveInsertOutcome::ExactReplay
    );
    assert_eq!(prepared.index.total_bytes, expected_total);
    assert_eq!(prepared.index.policy_count, 1);
    assert_eq!(prepared.index.anchor_count, 1);
    assert_eq!(prepared.index.generation, 1);
    drop(prepared);
    assert_eq!(
        archive.get_exact(&projection.key).unwrap(),
        Some(projection.clone())
    );
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
    assert_eq!(index.generation, 1);
}

#[test]
fn prepared_reputation_insertion_owns_actual_writer_until_drop_without_side_effects() {
    use std::{
        future::Future as _,
        pin::Pin,
        task::{Context, Waker},
    };
    let directory = tempdir().unwrap();
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    let prepared = archive.prepare_insert(projection.clone()).unwrap();
    let wait = match archive.index.try_write() {
        Err(ArchiveIndexLockError::Busy(wait)) => wait,
        _ => panic!("prepared insertion must retain the actual writer"),
    };
    let mut released = wait.wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut released).poll(&mut context).is_pending());
    assert!(prepared.index.by_height.is_empty());
    assert!(prepared.index.policies.is_empty());
    assert_eq!(fs::read_dir(&archive.anchors).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&archive.policies).unwrap().count(), 0);
    drop(prepared);
    assert!(Pin::new(&mut released).poll(&mut context).is_ready());
    assert!(archive.index.try_write().is_ok());
    assert!(archive.is_empty().unwrap());
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
fn reputation_admission_rejects_fork_gap_capacity_and_generation_before_writes() {
    let directory = tempdir().unwrap();
    let archive = open_archive(&directory, bounds());
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
