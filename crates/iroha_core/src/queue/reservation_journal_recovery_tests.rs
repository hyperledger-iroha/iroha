// Reservation-journal release, recovery, and checked-transition regression tests.
//
// Included by `queue::reservation_journal::tests` to preserve exact libtest names.
#[test]
fn ordered_release_rejects_conflicts_partial_completion_and_aba_reuse() {
    let records = vec![record(1, 1), record(2, 1)];
    let barrier = release_barrier(&records, 1);
    let completion = release_completion(&records, 1);
    let mut live = records.clone();
    let mut committed = Vec::new();
    let mut plan_tombstoned = Vec::new();
    let mut barriers = Vec::new();
    let mut completed = Vec::new();
    assert!(
        apply_frame(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV6::CompleteRelease(completion.clone()),
        )
        .is_err(),
        "completion must require its exact prepared barrier"
    );
    assert_eq!(live, records);
    apply_frame(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut barriers,
        &mut completed,
        LaneQueueReservationJournalFrameV6::PrepareRelease(barrier.clone()),
    )
    .expect("prepare exact release");
    let mut conflicting_barrier = barrier.clone();
    conflicting_barrier.retirement_hash = Hash::new(b"conflicting-retirement");
    assert!(
        apply_frame(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV6::PrepareRelease(conflicting_barrier),
        )
        .is_err(),
        "overlapping release identities must fail closed"
    );
    let mut wrong_records = completion.clone();
    wrong_records.ordered_records[0].enqueue_timestamp_ms = wrong_records.ordered_records[0]
        .enqueue_timestamp_ms
        .saturating_add(1);
    assert!(
        apply_frame(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV6::CompleteRelease(wrong_records),
        )
        .is_err(),
        "completion must match exact live records, including FIFO timestamps"
    );
    assert_eq!(live, records);
    assert_eq!(barriers, vec![barrier.clone()]);
    assert!(completed.is_empty());
    apply_frame(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut barriers,
        &mut completed,
        LaneQueueReservationJournalFrameV6::CompleteRelease(completion.clone()),
    )
    .expect("complete exact release");
    assert!(live.is_empty());
    assert!(barriers.is_empty());
    assert_eq!(completed, vec![completion.clone()]);
    let recreated = record(1, 2);
    assert!(
        apply_frame(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![recreated]),
        )
        .is_err(),
        "completed release must block same-hash ABA reservation reuse"
    );
    let mut stale_forget = barrier.clone();
    stale_forget.retirement_hash = Hash::new(b"stale-forget-retirement");
    apply_frame(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut barriers,
        &mut completed,
        LaneQueueReservationJournalFrameV6::ForgetRelease(stale_forget),
    )
    .expect("stale full identity is a harmless no-op");
    assert_eq!(completed, vec![completion]);
    apply_frame(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut barriers,
        &mut completed,
        LaneQueueReservationJournalFrameV6::ForgetRelease(barrier),
    )
    .expect("forget exact completion");
    assert!(completed.is_empty());
}
#[test]
fn snapshot_rejects_completed_release_overlapping_live_ownership() {
    let record = record(1, 1);
    let completion = release_completion(core::slice::from_ref(&record), 1);
    let mut live = Vec::new();
    let mut committed = Vec::new();
    let mut plan_tombstoned = Vec::new();
    let mut barriers = Vec::new();
    let mut completed = Vec::new();
    assert!(
        apply_frame(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: vec![record],
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: vec![completion],
            },
        )
        .is_err()
    );
    assert!(live.is_empty(), "invalid snapshot must apply atomically");
    assert!(committed.is_empty());
    assert!(barriers.is_empty());
    assert!(completed.is_empty());
}
#[test]
fn exact_tombstone_does_not_remove_readmitted_hash_with_new_plan() {
    let old = record(1, 1);
    let mut replacement = old.clone();
    replacement.key.routing_plan_digest = Hash::new(b"replacement-plan");
    replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal");
    let mut records = vec![replacement.clone()];
    let mut committed = Vec::new();
    apply_unprotected_frame(
        &mut records,
        &mut committed,
        LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![old.key]),
    )
    .expect("stale release is idempotent");
    assert_eq!(records, vec![replacement]);
}
#[test]
fn put_rejects_same_hash_reuse_behind_commit_barrier() {
    let old = record(1, 1);
    let mut live = vec![old.clone()];
    let mut committed = Vec::new();
    apply_unprotected_frame(
        &mut live,
        &mut committed,
        LaneQueueReservationJournalFrameV6::Commit(old.key),
    )
    .expect("commit exact live reservation");
    assert!(live.is_empty());
    let mut replacement = old;
    replacement.key.routing_plan_digest = Hash::new(b"replacement-plan-after-commit");
    replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal-after-commit");
    assert!(
        apply_unprotected_frame(
            &mut live,
            &mut committed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![replacement]),
        )
        .is_err(),
        "commit cleanup must block all same-hash reservation identities"
    );
}
#[test]
fn compaction_is_replay_equivalent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reservations.norito");
    let first = record(1, 1);
    let second = record(2, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(vec![first.clone(), second.clone()])
        .expect("put records");
    journal.release(first.key).expect("release first");
    assert!(
        journal
            .compact_if_needed(core::slice::from_ref(&second), &[], &[], &[], &[])
            .expect("compact")
    );
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("reopen compacted journal");
    assert_eq!(replay.records(), &[second]);
}
#[test]
fn compaction_preserves_valid_owner_state_larger_than_one_execution_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("large-owner-snapshot.norito");
    let owner_count = MAX_MERGE_EXECUTION_ENTRYPOINTS + 1;
    let records = (0..owner_count).map(indexed_record).collect::<Vec<_>>();
    let limits = LaneQueueReservationJournalLimits::new(
        minimum_bootstrap_frame_bytes().expect("bootstrap size"),
        u64::from(u32::MAX),
        u64::MAX,
        owner_count,
    );
    let (mut journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("create large bounded journal");
    assert!(replay.records().is_empty());
    journal
        .put_batch(records[..MAX_MERGE_EXECUTION_ENTRYPOINTS].to_vec())
        .expect("persist maximum canonical execution group");
    journal
        .put_batch(records[MAX_MERGE_EXECUTION_ENTRYPOINTS..].to_vec())
        .expect("persist the next bounded owner batch");
    assert!(
        journal
            .compact_if_needed(&records, &[], &[], &[], &[])
            .expect("compact owner state above one execution group")
    );
    drop(journal);
    let (_journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("reopen compacted owner state above one execution group");
    assert_eq!(replay.records(), records.as_slice());
}
#[test]
fn compaction_failure_after_rename_is_recovered_on_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ambiguous-compaction.norito");
    let record = record(1, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(vec![record.clone()])
        .expect("put live reservation");
    journal.inject_next_compaction_fault(
        ReservationJournalCompactionFault::AfterRenameBeforeParentSync,
    );
    assert!(
        journal
            .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[], &[])
            .is_err(),
        "a post-rename durability ambiguity must fail closed"
    );
    assert!(journal.durability_ambiguous());
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("reopen renamed journal");
    assert_eq!(replay.records(), &[record]);
}
#[test]
fn post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("post-sync-append-publication.norito");
    let record = indexed_record(0);
    let (mut journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("open journal");
    assert!(replay.records().is_empty());
    let memory_before = journal.replay_state.clone();
    journal
        .inject_next_append_fault(ReservationJournalAppendFault::AfterSyncBeforeReplayPublication);
    let error = journal
        .put_batch(vec![record.clone()])
        .expect_err("checked publication failure after sync must fail closed");
    assert!(
        error.to_string().contains("different exact pre-state"),
        "unexpected checked publication rejection: {error}"
    );
    assert!(journal.durability_ambiguous());
    assert_eq!(
        journal.replay_state, memory_before,
        "disk-ahead failure must leave the predecessor memory state unpublished"
    );
    assert_eq!(
        journal.known_len,
        fs::metadata(&path).expect("durable journal metadata").len(),
        "the complete synchronized frame must remain visible for restart repair"
    );
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay disk-ahead frame");
    assert_eq!(replay.records(), &[record]);
}
#[test]
fn post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("post-sync-compaction-publication.norito");
    let record = indexed_record(0);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(vec![record.clone()])
        .expect("persist compaction fixture");
    let memory_before = journal.replay_state.clone();
    journal.inject_next_compaction_fault(
        ReservationJournalCompactionFault::AfterSyncBeforeReplayPublication,
    );
    let error = journal
        .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[], &[])
        .expect_err("checked compaction publication failure must fail closed");
    assert!(
        error.to_string().contains("different exact pre-state"),
        "unexpected checked compaction rejection: {error}"
    );
    assert!(journal.durability_ambiguous());
    assert_eq!(
        journal.replay_state, memory_before,
        "durable replacement must not partially publish candidate memory state"
    );
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("replay durable replacement");
    assert_eq!(replay.records(), &[record]);
}
#[test]
fn compaction_preserves_prepared_and_completed_release_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("release-compaction.norito");
    let prepared_records = vec![record(1, 1), record(2, 1)];
    let completed_records = vec![record(3, 1), record(4, 1)];
    let prepared = release_barrier(&prepared_records, 1);
    let completed = release_completion(&completed_records, 2);
    let mut all_records = prepared_records.clone();
    all_records.extend(completed_records);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(all_records)
        .expect("persist all reservation ownership");
    journal
        .prepare_release(prepared.clone())
        .expect("prepare first release");
    journal
        .prepare_release(completed.barrier.clone())
        .expect("prepare second release");
    journal
        .complete_release(completed.clone())
        .expect("complete second release");
    assert!(
        journal
            .compact_if_needed(
                &prepared_records,
                &[],
                &[],
                core::slice::from_ref(&prepared),
                core::slice::from_ref(&completed),
            )
            .expect("compact all V6 release state")
    );
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("replay compacted V6 snapshot");
    assert_eq!(replay.records(), prepared_records.as_slice());
    assert!(replay.committed().is_empty());
    assert_eq!(replay.release_barriers(), &[prepared]);
    assert_eq!(replay.completed_releases(), &[completed]);
}
#[test]
fn commit_barrier_survives_restart_until_exact_forget_is_durable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("commit-barrier.norito");
    let record = record(9, 1);
    {
        let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
        journal
            .put_batch(vec![record.clone()])
            .expect("put reservation");
        journal.commit(record.key).expect("commit reservation");
    }
    let (mut journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("replay commit barrier");
    assert!(replay.records().is_empty());
    assert_eq!(replay.committed(), &[record.key]);
    assert!(replay.plan_tombstoned().is_empty());
    journal
        .plan_tombstoned(record.key)
        .expect("mark exact durable QueuePlan tombstone");
    journal
        .forget_commit(record.key)
        .expect("forget after independent queue-plan durability");
    journal
        .compact_if_needed(&[], &[], &[], &[], &[])
        .expect("compact forgotten barrier");
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("reopen forgotten barrier");
    assert!(replay.records().is_empty());
    assert!(replay.committed().is_empty());
}
#[test]
fn plan_tombstoned_marker_is_exact_required_and_compaction_stable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("plan-tombstoned-compaction.norito");
    let record = record(10, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    assert!(
        journal.plan_tombstoned(record.key).is_err(),
        "a marker without its exact commit barrier must fail closed"
    );
    journal
        .put_batch(vec![record.clone()])
        .expect("persist live owner");
    journal.commit(record.key).expect("persist commit barrier");
    assert!(
        journal.forget_commit(record.key).is_err(),
        "ForgetCommit must not consume an unmarked barrier"
    );
    journal
        .plan_tombstoned(record.key)
        .expect("persist exact marker");
    assert!(
        journal
            .compact_if_needed(
                &[],
                core::slice::from_ref(&record.key),
                core::slice::from_ref(&record.key),
                &[],
                &[],
            )
            .expect("compact marked barrier")
    );
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, 1).expect("reopen compacted marker");
    assert_eq!(replay.committed(), &[record.key]);
    assert_eq!(replay.plan_tombstoned(), &[record.key]);
}
#[test]
fn newly_created_journal_survives_immediate_close_and_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("newly-created.norito");
    let record = record(11, 1);
    {
        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
        assert!(replay.records().is_empty());
        assert!(
            fs::symlink_metadata(&path)
                .expect("journal metadata")
                .is_file()
        );
        journal
            .put_batch(vec![record.clone()])
            .expect("power-loss durability boundary");
    }
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("reopen journal");
    assert_eq!(replay.records(), &[record]);
}
#[test]
fn journal_exclusive_owner_lock_blocks_a_second_runtime() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("exclusive-owner.norito");
    let record = record(31, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open first owner");
    journal
        .put_batch(vec![record.clone()])
        .expect("persist lock-transfer fixture");
    assert!(
        journal
            .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[], &[])
            .expect("compact while retaining exclusive ownership"),
        "fixture must exercise lock transfer to the replacement inode"
    );
    let error = LaneQueueReservationJournal::open(&path, u64::MAX)
        .err()
        .expect("a second owner must fail while the first lock is retained");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    drop(journal);
    assert!(
        LaneQueueReservationJournal::open(&path, u64::MAX).is_ok(),
        "dropping the exact owner must release its OS lock"
    );
}
#[cfg(unix)]
#[test]
fn cached_revision_rejects_an_unlocked_same_length_external_rewrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("external-rewrite.norito");
    let first = record(1, 1);
    let second = record(2, 1);
    let third = record(3, 1);
    let (mut journal, _) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("open journal");
    journal
        .put_batch(vec![first.clone()])
        .expect("persist first owner");
    let mut alternate = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    alternate.extend_from_slice(
        &encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![second]))
            .expect("encode alternate same-length owner"),
    );
    assert_eq!(
        u64::try_from(alternate.len()).expect("fixture length fits u64"),
        fs::metadata(&path).expect("journal metadata").len()
    );
    fs::write(&path, alternate).expect("simulate an unlocked same-inode external writer");
    let error = journal
        .put_batch(vec![third])
        .expect_err("cached metadata revision must reject the external rewrite");
    assert!(
        error
            .to_string()
            .contains("metadata changed outside its durable owner"),
        "unexpected rejection: {error}"
    );
    assert!(journal.durability_ambiguous());
}
#[test]
fn journal_rejects_non_regular_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("journal-directory");
    fs::create_dir(&path).expect("create path directory");
    assert!(
        LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
        "a directory must never be opened or truncated as a journal"
    );
}
#[cfg(unix)]
#[test]
fn journal_rejects_symlink_path_and_symlink_parent() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target");
    File::create(&target).expect("create target");
    let path_link = dir.path().join("journal-link");
    symlink(&target, &path_link).expect("create journal symlink");
    assert!(LaneQueueReservationJournal::open(&path_link, u64::MAX).is_err());
    let real_parent = dir.path().join("real-parent");
    fs::create_dir(&real_parent).expect("create real parent");
    let linked_parent = dir.path().join("linked-parent");
    symlink(&real_parent, &linked_parent).expect("create parent symlink");
    assert!(
        LaneQueueReservationJournal::open(linked_parent.join("journal"), u64::MAX).is_err(),
        "journal creation must not follow a symlink parent"
    );
}
#[test]
fn compaction_rejects_preexisting_regular_temp_collision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("collision.norito");
    let record = record(12, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(vec![record.clone()])
        .expect("write live record");
    let tmp = path.with_extension("reservation-compact.tmp");
    fs::write(&tmp, b"sentinel").expect("create colliding temp");
    assert!(
        journal
            .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[], &[])
            .is_err(),
        "compaction must never truncate a predictable preexisting temp path"
    );
    assert_eq!(
        journal.path,
        fs::canonicalize(&path).expect("canonical journal path")
    );
    assert_eq!(
        fs::read(&tmp).expect("read colliding temp"),
        b"sentinel",
        "rejected compaction must not alter the preexisting temp file"
    );
}
#[cfg(unix)]
#[test]
fn compaction_rejects_symlink_temp_collision_without_touching_target() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("symlink-collision.norito");
    let target = dir.path().join("do-not-truncate");
    fs::write(&target, b"sentinel").expect("write target sentinel");
    let record = record(13, 1);
    let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
    journal
        .put_batch(vec![record.clone()])
        .expect("write live record");
    let tmp = path.with_extension("reservation-compact.tmp");
    symlink(&target, &tmp).expect("create malicious temp symlink");
    assert!(
        journal
            .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[], &[])
            .is_err()
    );
    assert_eq!(fs::read(&target).expect("read sentinel"), b"sentinel");
}
#[test]
fn initial_bootstrap_recovers_every_recognizable_staged_prefix() {
    let expected = encode_frame(&bootstrap_frame()).expect("encode canonical V6 bootstrap");
    for written in 0..expected.len() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join(format!("bootstrap-prefix-{written}.norito"));
        fs::write(&path, &expected[..written]).expect("write interrupted bootstrap prefix");
        let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("recover canonical bootstrap prefix");
        assert!(replay.records().is_empty());
        assert_eq!(
            fs::read(&path).expect("read repaired bootstrap"),
            expected,
            "bootstrap prefix {written} must be replaced by the exact durable V6 marker"
        );
    }
}
#[test]
fn full_length_torn_terminal_header_is_repaired_without_parsing_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("full-header-tear.norito");
    let first = record(21, 1);
    {
        let (mut journal, _) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
        journal
            .put_batch(vec![first.clone()])
            .expect("persist preceding frame");
    }
    let durable_len = path.metadata().expect("journal metadata").len();
    let torn_header = vec![0xA5; usize::try_from(FRAME_HEADER_BYTES).expect("header fits usize")];
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open journal append")
        .write_all(&torn_header)
        .expect("write full-length torn header");
    let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
        .expect("repair full-length staged header");
    assert_eq!(replay.records(), &[first]);
    assert_eq!(
        path.metadata().expect("repaired metadata").len(),
        durable_len
    );
}
#[test]
fn complete_indeterminate_frame_is_synced_before_two_restart_adoption() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("two-crash-adoption.norito");
    let first = record(22, 1);
    let second = record(23, 1);
    {
        let (mut journal, _) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
        journal
            .put_batch(vec![first.clone()])
            .expect("persist first record");
    }
    let second_frame = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
        second.clone(),
    ]))
    .expect("encode indeterminate complete frame");
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open raw append")
        .write_all(&second_frame)
        .expect("materialize complete pre-sync frame");
    {
        let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("first restart adopts and synchronizes complete frame");
        assert_eq!(replay.records(), &[first.clone(), second.clone()]);
    }
    let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
        .expect("second restart retains adopted frame");
    assert_eq!(replay.records(), &[first, second]);
}
#[test]
fn authenticated_truncated_compaction_temp_is_discarded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("truncated-temp.norito");
    let first = record(24, 1);
    {
        let (mut journal, _) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
        journal
            .put_batch(vec![first.clone()])
            .expect("persist record");
    }
    let snapshot = canonical_snapshot(core::slice::from_ref(&first), &[], &[], &[], &[])
        .expect("build canonical snapshot");
    let compacted =
        encode_compacted_journal(snapshot.as_ref()).expect("encode canonical compaction");
    let tmp = path.with_extension("reservation-compact.tmp");
    fs::write(&tmp, &compacted[..compacted.len() / 2])
        .expect("write authenticated compaction prefix");
    let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
        .expect("reconcile interrupted compaction");
    assert_eq!(replay.records(), &[first]);
    assert!(
        !tmp.exists(),
        "authenticated prefix must be durably removed"
    );
}
#[test]
fn corrupt_or_oversized_compaction_temp_fails_closed_and_is_retained() {
    for oversized in [false, true] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(if oversized {
            "oversized-temp.norito"
        } else {
            "corrupt-temp.norito"
        });
        let first = record(25, 1);
        {
            let (mut journal, _) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            journal
                .put_batch(vec![first.clone()])
                .expect("persist record");
        }
        let snapshot = canonical_snapshot(core::slice::from_ref(&first), &[], &[], &[], &[])
            .expect("build canonical snapshot");
        let mut compacted =
            encode_compacted_journal(snapshot.as_ref()).expect("encode canonical compaction");
        if oversized {
            compacted.push(0);
        } else {
            compacted[0] ^= 0x80;
        }
        let tmp = path.with_extension("reservation-compact.tmp");
        fs::write(&tmp, &compacted).expect("write invalid compaction temp");
        let canonical = fs::read(&path).expect("read canonical before rejection");
        assert!(
            LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
            "invalid compaction temp must fail closed"
        );
        assert_eq!(
            fs::read(&tmp).expect("retain invalid temp evidence"),
            compacted
        );
        assert_eq!(
            fs::read(&path).expect("retain canonical evidence"),
            canonical
        );
    }
}
#[test]
fn compaction_temp_cannot_recreate_missing_canonical_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing-canonical.norito");
    let tmp = path.with_extension("reservation-compact.tmp");
    let compacted =
        encode_compacted_journal(None).expect("encode an otherwise valid empty compaction");
    fs::write(&tmp, &compacted).expect("write orphan compaction temp");
    assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
    assert!(
        !path.exists(),
        "startup must not synthesize a canonical owner"
    );
    assert_eq!(fs::read(&tmp).expect("retain orphan evidence"), compacted);
}
#[test]
fn portable_atomic_replacement_replaces_existing_destination() {
    let dir = tempfile::tempdir().expect("tempdir");
    let destination = dir.path().join("destination");
    let temporary = dir.path().join("temporary");
    fs::write(&destination, b"old").expect("write old destination");
    fs::write(&temporary, b"new").expect("write replacement");
    persist_atomic_replacement(&temporary, &destination).expect("replace destination");
    assert_eq!(fs::read(&destination).expect("read replacement"), b"new");
    assert!(!temporary.exists());
}
#[cfg(unix)]
#[test]
fn journal_rejects_existing_and_new_hardlinks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("hardlink-journal.norito");
    let alias = dir.path().join("hardlink-alias.norito");
    let first = record(26, 1);
    let mut journal = LaneQueueReservationJournal::open(&path, u64::MAX)
        .expect("create journal")
        .0;
    fs::hard_link(&path, &alias).expect("create unexpected hardlink");
    assert!(
        journal.put_batch(vec![first]).is_err(),
        "cached append handle must reject a link-count change"
    );
    assert!(journal.durability_ambiguous());
    drop(journal);
    assert!(
        LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
        "startup must reject a multiply linked journal"
    );
    fs::remove_file(&alias).expect("remove hardlink alias");
    assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_ok());
}
#[test]
fn journal_requires_preexisting_durable_parent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing_parent = dir.path().join("missing").join("nested");
    let path = missing_parent.join("reservations.norito");
    assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
    assert!(
        !missing_parent.exists(),
        "journal open must not create an ancestor chain it cannot durably link"
    );
}
#[test]
fn runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("checked-commit-owner.norito");
    let record = indexed_record(0);
    let (mut journal, _) =
        LaneQueueReservationJournal::open(&path, 1).expect("open checked journal");
    let before_len = journal.known_len;
    let error = journal
        .commit(record.key)
        .expect_err("runtime Absent-to-Committed must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        journal.known_len, before_len,
        "rejected commit must not append any frame"
    );
    assert!(
        !journal.poisoned,
        "semantic rejection before storage must leave the journal usable"
    );
    journal
        .put_batch(vec![record.clone()])
        .expect("install exact live owner");
    journal
        .commit(record.key)
        .expect("exact Live-to-Committed remains valid");
    let mut recovered = IndexedReservationReplayState::default();
    recovered
        .transition(
            &LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: vec![record.key],
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: Vec::new(),
            },
            8,
        )
        .expect("snapshot recovery has a distinct checked reconstruction action");
    assert_eq!(recovered.replay().committed(), &[record.key]);
    let invalid_replay_path = dir.path().join("absent-commit-replay.norito");
    let mut invalid_replay = encode_frame(&bootstrap_frame()).expect("encode exact V6 bootstrap");
    invalid_replay.extend(
        encode_frame(&LaneQueueReservationJournalFrameV6::Commit(record.key))
            .expect("encode structurally valid absent-owner commit"),
    );
    fs::write(&invalid_replay_path, &invalid_replay).expect("write exact invalid replay prefix");
    let replay_error = LaneQueueReservationJournal::open(&invalid_replay_path, 1)
        .err()
        .expect("startup replay must reject Absent-to-Committed");
    assert_eq!(replay_error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        fs::read(&invalid_replay_path).expect("retain rejected replay evidence"),
        invalid_replay,
        "semantic replay rejection must not rewrite complete retained evidence"
    );
}
#[test]
fn prepared_checked_transition_is_bound_to_frame_and_state_generation() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let first_frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]);
    let second_frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![second.clone()]);
    let mut wrong_frame_state = IndexedReservationReplayState::default();
    let wrong_frame_authorization = wrong_frame_state
        .prepare_checked_transition(&first_frame, 8)
        .expect("prepare exact first frame");
    assert!(
        wrong_frame_state
            .apply_checked_transition(&second_frame, 8, wrong_frame_authorization)
            .is_err(),
        "one frame's move-only authorization must not apply another frame"
    );
    assert!(wrong_frame_state.replay().records().is_empty());
    let mut wrong_bound_state = IndexedReservationReplayState::default();
    let wrong_bound_authorization = wrong_bound_state
        .prepare_checked_transition(&first_frame, 8)
        .expect("prepare transition with exact ownership bound");
    assert!(
        wrong_bound_state
            .apply_checked_transition(&first_frame, 9, wrong_bound_authorization)
            .is_err(),
        "authorization must not cross configured ownership bounds"
    );
    assert!(wrong_bound_state.replay().records().is_empty());
    let mut stale_state = IndexedReservationReplayState::default();
    let stale_authorization = stale_state
        .prepare_checked_transition(&first_frame, 8)
        .expect("prepare transition at generation zero");
    stale_state
        .transition(&second_frame, 8)
        .expect("advance retained-prefix generation");
    let before_stale_attempt = stale_state.clone();
    assert!(
        stale_state
            .apply_checked_transition(&first_frame, 8, stale_authorization)
            .is_err(),
        "authorization must become stale after another state operation"
    );
    assert_eq!(stale_state, before_stale_attempt);
    assert_eq!(stale_state.replay().records(), &[second]);
}
#[test]
fn prepared_checked_transition_rejects_same_generation_cross_state_substitution() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let absent = indexed_record(2);
    let common_frame = LaneQueueReservationJournalFrameV6::ForgetCommit(absent.key);
    let mut identical_left = IndexedReservationReplayState::default();
    identical_left
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
            8,
        )
        .expect("seed the exact left state");
    let mut identical_right = identical_left.clone();
    assert_eq!(identical_left, identical_right);
    assert_eq!(
        identical_left.transition_generation,
        identical_right.transition_generation
    );
    assert_eq!(
        identical_left.checked_state_identity,
        identical_right.checked_state_identity
    );
    let identical_left_authorization = identical_left
        .prepare_checked_transition(&common_frame, 8)
        .expect("prepare on one of two logically identical state instances");
    let identical_right_before = identical_right.clone();
    let error = identical_right
        .apply_checked_transition(&common_frame, 8, identical_left_authorization)
        .expect_err("authorization must not cross an independently mutable clone");
    assert!(
        error.to_string().contains("different exact state instance"),
        "unexpected state-instance rejection: {error}"
    );
    assert_eq!(identical_right, identical_right_before);
    let mut left = IndexedReservationReplayState::default();
    let mut right = IndexedReservationReplayState::default();
    left.transition(
        &LaneQueueReservationJournalFrameV6::PutBatch(vec![first]),
        8,
    )
    .expect("advance left checked state");
    right
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![second]),
            8,
        )
        .expect("advance right checked state");
    assert_eq!(left.transition_generation, right.transition_generation);
    assert_ne!(
        left.checked_state_identity, right.checked_state_identity,
        "divergent canonical frames at one generation need distinct state identities"
    );
    let left_authorization = left
        .prepare_checked_transition(&common_frame, 8)
        .expect("prepare a stuttering transition on the left state");
    let right_before = right.clone();
    let error = right
        .apply_checked_transition(&common_frame, 8, left_authorization)
        .expect_err("same-generation authorization must not cross divergent states");
    assert!(
        error.to_string().contains("different exact state instance"),
        "unexpected cross-state rejection: {error}"
    );
    assert_eq!(right, right_before);
}
#[test]
fn prepared_checked_transition_binds_exact_ordered_owner_token_coverage() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let third = indexed_record(2);
    let fourth = indexed_record(3);
    let frame =
        LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone(), second.clone(), third]);
    let mut missing_state = IndexedReservationReplayState::default();
    let mut missing = missing_state
        .prepare_checked_transition(&frame, 8)
        .expect("prepare exact three-owner transition");
    let _missing_token = missing
        .owner_transitions
        .pop()
        .expect("three-owner transition has a final token");
    let missing_before = missing_state.clone();
    assert!(
        missing_state
            .apply_checked_transition(&frame, 8, missing)
            .is_err(),
        "missing owner evidence must fail closed"
    );
    assert_eq!(missing_state, missing_before);
    let mut reordered_state = IndexedReservationReplayState::default();
    let mut reordered = reordered_state
        .prepare_checked_transition(&frame, 8)
        .expect("prepare exact ordered transition");
    reordered.owner_transitions.reverse();
    let reordered_before = reordered_state.clone();
    assert!(
        reordered_state
            .apply_checked_transition(&frame, 8, reordered)
            .is_err(),
        "reordered owner evidence must fail closed"
    );
    assert_eq!(reordered_state, reordered_before);
    let mut altered_state = IndexedReservationReplayState::default();
    let mut altered = altered_state
        .prepare_checked_transition(&frame, 8)
        .expect("prepare original owner transition");
    let alternate_frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![first, second, fourth]);
    let mut alternate_state = IndexedReservationReplayState::default();
    let mut alternate = alternate_state
        .prepare_checked_transition(&alternate_frame, 8)
        .expect("prepare alternate owner transition");
    let replacement = alternate
        .owner_transitions
        .pop()
        .expect("alternate transition has a final token");
    let _replaced_token = altered
        .owner_transitions
        .pop()
        .expect("original transition has a final token");
    altered.owner_transitions.push(replacement);
    let altered_before = altered_state.clone();
    assert!(
        altered_state
            .apply_checked_transition(&frame, 8, altered)
            .is_err(),
        "substituted owner evidence must fail closed"
    );
    assert_eq!(altered_state, altered_before);
}
#[test]
fn checked_transition_result_identity_and_candidate_application_are_atomic() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]);
    let mut wrong_result_state = IndexedReservationReplayState::default();
    let mut wrong_result = wrong_result_state
        .prepare_checked_transition(&frame, 8)
        .expect("prepare exact resulting identity");
    wrong_result.resulting_state_identity =
        Hash::new(b"altered checked reservation result identity");
    let wrong_result_before = wrong_result_state.clone();
    assert!(
        wrong_result_state
            .apply_checked_transition(&frame, 8, wrong_result)
            .is_err(),
        "altered resulting identity must fail closed"
    );
    assert_eq!(wrong_result_state, wrong_result_before);
    let absent_frame = LaneQueueReservationJournalFrameV6::ForgetCommit(second.key);
    let mut shape_state = IndexedReservationReplayState::default();
    let shape_authorization = shape_state
        .prepare_checked_transition(&absent_frame, 8)
        .expect("prepare an absent no-op before shape drift");
    shape_state.next_order = 1;
    let shape_before = shape_state.clone();
    let error = shape_state
        .apply_checked_transition(&absent_frame, 8, shape_authorization)
        .expect_err("next-order drift must invalidate the exact pre-state witness");
    assert!(
        error
            .to_string()
            .contains("different exact pre-state shape"),
        "unexpected shape-drift rejection: {error}"
    );
    assert_eq!(shape_state, shape_before);
    let mut owner_state = IndexedReservationReplayState::default();
    owner_state.ownership.insert(
        second.key.entrypoint_hash,
        DurableReservationOwnership::Live(second.key),
    );
    let owner_authorization = owner_state
        .prepare_checked_transition(&absent_frame, 8)
        .expect("prepare against one exact owner projection");
    owner_state.ownership.insert(
        second.key.entrypoint_hash,
        DurableReservationOwnership::Committed(second.key),
    );
    let owner_before = owner_state.clone();
    let error = owner_state
        .apply_checked_transition(&absent_frame, 8, owner_authorization)
        .expect_err("same-shape owner substitution must invalidate checked evidence");
    assert!(
        error
            .to_string()
            .contains("owner evidence no longer matches the exact pre-state"),
        "unexpected owner-evidence rejection: {error}"
    );
    assert_eq!(owner_state, owner_before);
    let mut candidate_state = IndexedReservationReplayState::default();
    let candidate_authorization = candidate_state
        .prepare_checked_transition(&frame, 8)
        .expect("prepare before injecting a semantic pre-state failure");
    candidate_state
        .fifo_ordinals
        .insert(first.fifo_order.ordinal, second.key.entrypoint_hash);
    let candidate_before = candidate_state.clone();
    assert!(
        candidate_state
            .apply_checked_transition(&frame, 8, candidate_authorization)
            .is_err(),
        "checked application must reject the injected index conflict before mutation"
    );
    assert_eq!(
        candidate_state, candidate_before,
        "failed semantic revalidation must not partially mutate the published state"
    );
    let removal_frame = LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![first.key]);
    let exact_lane = (first.key.lane_id, first.key.lane_incarnation);
    let mut lane_key_state = IndexedReservationReplayState::default();
    lane_key_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
            8,
        )
        .expect("seed exact live reservation before lane-key substitution");
    let lane_key_authorization = lane_key_state
        .prepare_checked_transition(&removal_frame, 8)
        .expect("prepare release before same-shape lane-key substitution");
    let indexed_hashes = lane_key_state
        .live_by_lane_incarnation
        .remove(&exact_lane)
        .expect("seeded reservation has an exact lane-incarnation index");
    assert!(
        lane_key_state
            .live_by_lane_incarnation
            .insert(
                (LaneId::new(99), first.key.lane_incarnation),
                indexed_hashes
            )
            .is_none()
    );
    let lane_key_before = lane_key_state.clone();
    let error = lane_key_state
        .apply_checked_transition(&removal_frame, 8, lane_key_authorization)
        .expect_err("same-shape lane-key substitution must fail before removal");
    assert!(
        error.to_string().contains("lane-incarnation index"),
        "unexpected lane-key substitution rejection: {error}"
    );
    assert_eq!(
        lane_key_state, lane_key_before,
        "lane-key corruption must not permit partial removal"
    );
    let mut lane_member_state = IndexedReservationReplayState::default();
    lane_member_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
            8,
        )
        .expect("seed exact live reservation before lane-member substitution");
    let lane_member_authorization = lane_member_state
        .prepare_checked_transition(&removal_frame, 8)
        .expect("prepare release before same-shape lane-member substitution");
    let lane_hashes = lane_member_state
        .live_by_lane_incarnation
        .get_mut(&exact_lane)
        .expect("seeded reservation has an exact lane-incarnation set");
    assert!(lane_hashes.remove(&first.key.entrypoint_hash));
    assert!(lane_hashes.insert(second.key.entrypoint_hash));
    let lane_member_before = lane_member_state.clone();
    let error = lane_member_state
        .apply_checked_transition(&removal_frame, 8, lane_member_authorization)
        .expect_err("same-shape lane-member substitution must fail before removal");
    assert!(
        error.to_string().contains("lane-incarnation index"),
        "unexpected lane-member substitution rejection: {error}"
    );
    assert_eq!(
        lane_member_state, lane_member_before,
        "lane-member corruption must not permit partial removal"
    );
    let mut fifo_member_state = IndexedReservationReplayState::default();
    fifo_member_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
            8,
        )
        .expect("seed exact live reservation before FIFO substitution");
    let fifo_member_authorization = fifo_member_state
        .prepare_checked_transition(&removal_frame, 8)
        .expect("prepare release before same-shape FIFO substitution");
    assert_eq!(
        fifo_member_state
            .fifo_ordinals
            .insert(first.fifo_order.ordinal, second.key.entrypoint_hash),
        Some(first.key.entrypoint_hash)
    );
    let fifo_member_before = fifo_member_state.clone();
    let error = fifo_member_state
        .apply_checked_transition(&removal_frame, 8, fifo_member_authorization)
        .expect_err("same-shape FIFO substitution must fail before removal");
    assert!(
        error.to_string().contains("FIFO index"),
        "unexpected FIFO substitution rejection: {error}"
    );
    assert_eq!(
        fifo_member_state, fifo_member_before,
        "FIFO corruption must not permit partial removal"
    );
    let multi_removal_frame =
        LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![first.key, second.key]);
    let mut later_target_state = IndexedReservationReplayState::default();
    later_target_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone(), second.clone()]),
            8,
        )
        .expect("seed two exact live reservations before later-target corruption");
    let later_target_authorization = later_target_state
        .prepare_checked_transition(&multi_removal_frame, 8)
        .expect("prepare two-owner release before corrupting its later target");
    assert_eq!(
        later_target_state
            .fifo_ordinals
            .insert(second.fifo_order.ordinal, first.key.entrypoint_hash),
        Some(second.key.entrypoint_hash)
    );
    let later_target_before = later_target_state.clone();
    let error = later_target_state
        .apply_checked_transition(&multi_removal_frame, 8, later_target_authorization)
        .expect_err("a corrupt later target must fail before the first removal");
    assert!(
        error.to_string().contains("FIFO index"),
        "unexpected later-target rejection: {error}"
    );
    assert_eq!(
        later_target_state, later_target_before,
        "all removals must be preflighted before the first indexed mutation"
    );
    let mut commit_state = IndexedReservationReplayState::default();
    commit_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
            8,
        )
        .expect("seed an exact live reservation before commit corruption");
    let commit_frame = LaneQueueReservationJournalFrameV6::Commit(first.key);
    let commit_authorization = commit_state
        .prepare_checked_transition(&commit_frame, 8)
        .expect("prepare commit before same-shape FIFO corruption");
    assert_eq!(
        commit_state
            .fifo_ordinals
            .insert(first.fifo_order.ordinal, second.key.entrypoint_hash),
        Some(first.key.entrypoint_hash)
    );
    let commit_before = commit_state.clone();
    let error = commit_state
        .apply_checked_transition(&commit_frame, 8, commit_authorization)
        .expect_err("commit must reject secondary-index corruption before removal");
    assert!(
        error.to_string().contains("FIFO index"),
        "unexpected commit-index rejection: {error}"
    );
    assert_eq!(
        commit_state, commit_before,
        "failed commit preflight must preserve live and committed ownership"
    );
    let release = release_barrier(&[first.clone(), second.clone()], 91);
    let release_digest = release.digest();
    let completion = release_completion(&[first.clone(), second.clone()], 91);
    let completion_frame = LaneQueueReservationJournalFrameV6::CompleteRelease(completion);
    let mut completion_state = IndexedReservationReplayState::default();
    completion_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone(), second.clone()]),
            8,
        )
        .expect("seed two exact live reservations before ordered completion");
    completion_state
        .transition(
            &LaneQueueReservationJournalFrameV6::PrepareRelease(release),
            8,
        )
        .expect("prepare the exact two-record release barrier");
    let completion_authorization = completion_state
        .prepare_checked_transition(&completion_frame, 8)
        .expect("prepare completion before corrupting its later target");
    assert_eq!(
        completion_state
            .fifo_ordinals
            .insert(second.fifo_order.ordinal, first.key.entrypoint_hash),
        Some(second.key.entrypoint_hash)
    );
    let completion_before = completion_state.clone();
    let error = completion_state
        .apply_checked_transition(&completion_frame, 8, completion_authorization)
        .expect_err("completion must preflight every member before removing its barrier");
    assert!(
        error.to_string().contains("FIFO index"),
        "unexpected completion-index rejection: {error}"
    );
    assert!(
        completion_state
            .release_barriers
            .contains_key(&release_digest),
        "failed completion preflight must preserve its prepared release barrier"
    );
    assert_eq!(
        completion_state, completion_before,
        "failed completion preflight must preserve the barrier and every live record"
    );
}
#[test]
fn checked_transition_generation_overflow_is_rejected_without_mutation() {
    let first = indexed_record(0);
    let frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![first]);
    let mut state = IndexedReservationReplayState::default();
    state.transition_generation = u64::MAX;
    let before = state.clone();
    let error = state
        .prepare_checked_transition(&frame, 8)
        .err()
        .expect("generation overflow must fail before authorization");
    assert!(
        error.to_string().contains("generation overflow"),
        "unexpected generation overflow rejection: {error}"
    );
    assert_eq!(state, before);
}
#[test]
fn indexed_replay_matches_reference_vector_transitions_and_ordering() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let third = indexed_record(2);
    let fourth = indexed_record(3);
    let release = release_barrier(&[first.clone(), second.clone()], 71);
    let completion = release_completion(&[first.clone(), second.clone()], 71);
    let mut stale_forget = release.clone();
    stale_forget.retirement_hash = Hash::new(b"indexed-differential-stale-forget");
    let frames = vec![
        LaneQueueReservationJournalFrameV6::PutBatch(vec![
            first.clone(),
            second.clone(),
            first.clone(),
        ]),
        LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone()]),
        LaneQueueReservationJournalFrameV6::PrepareRelease(release.clone()),
        LaneQueueReservationJournalFrameV6::PrepareRelease(release.clone()),
        LaneQueueReservationJournalFrameV6::CompleteRelease(completion.clone()),
        LaneQueueReservationJournalFrameV6::CompleteRelease(completion),
        LaneQueueReservationJournalFrameV6::PrepareRelease(release.clone()),
        LaneQueueReservationJournalFrameV6::ForgetRelease(stale_forget),
        LaneQueueReservationJournalFrameV6::ForgetRelease(release),
        LaneQueueReservationJournalFrameV6::PutBatch(vec![third.clone(), fourth.clone()]),
        LaneQueueReservationJournalFrameV6::Commit(third.key),
        LaneQueueReservationJournalFrameV6::PlanTombstoned(third.key),
        LaneQueueReservationJournalFrameV6::ForgetCommit(third.key),
        LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![fourth.key]),
    ];
    let mut indexed = IndexedReservationReplayState::default();
    let mut live = Vec::new();
    let mut committed = Vec::new();
    let mut plan_tombstoned = Vec::new();
    let mut prepared = Vec::new();
    let mut completed = Vec::new();
    for frame in frames {
        apply_frame_with_ownership_limit(
            &mut live,
            &mut committed,
            &mut plan_tombstoned,
            &mut prepared,
            &mut completed,
            frame.clone(),
            32,
        )
        .expect("reference transition");
        indexed.transition(&frame, 32).expect("indexed transition");
        let replay = indexed.replay();
        assert_eq!(replay.records(), live);
        assert_eq!(replay.committed(), committed);
        assert_eq!(replay.plan_tombstoned(), plan_tombstoned);
        assert_eq!(replay.release_barriers(), prepared);
        assert_eq!(replay.completed_releases(), completed);
        assert_eq!(
            indexed.ownership,
            durable_ownership_from_replay(&replay, 32).expect("reference ownership")
        );
    }
}
#[test]
fn indexed_transition_rejections_are_atomic_and_match_reference_replay() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let release = release_barrier(core::slice::from_ref(&first), 72);
    let mut indexed = IndexedReservationReplayState::default();
    indexed
        .transition(
            &LaneQueueReservationJournalFrameV6::PutBatch(vec![first.clone(), second.clone()]),
            8,
        )
        .expect("seed indexed state");
    indexed
        .transition(
            &LaneQueueReservationJournalFrameV6::PrepareRelease(release.clone()),
            8,
        )
        .expect("seed prepared release");
    let replay = indexed.replay();
    let mut conflicting_release = release.clone();
    conflicting_release.retirement_hash = Hash::new(b"indexed-adversarial-conflicting-release");
    let invalid_frames = vec![
        LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![first.key]),
        LaneQueueReservationJournalFrameV6::Commit(first.key),
        LaneQueueReservationJournalFrameV6::PrepareRelease(conflicting_release),
        LaneQueueReservationJournalFrameV6::ForgetRelease(release.clone()),
        LaneQueueReservationJournalFrameV6::CompleteRelease({
            let mut wrong = release_completion(core::slice::from_ref(&first), 72);
            wrong.ordered_records[0].enqueue_timestamp_ms = wrong.ordered_records[0]
                .enqueue_timestamp_ms
                .saturating_add(1);
            wrong
        }),
        LaneQueueReservationJournalFrameV6::PutBatch({
            let mut collision = indexed_record(2);
            collision.fifo_order.ordinal = second.fifo_order.ordinal;
            vec![collision]
        }),
    ];
    for frame in invalid_frames {
        let before = indexed.clone();
        let mut live = replay.records.clone();
        let mut committed = replay.committed.clone();
        let mut plan_tombstoned = replay.plan_tombstoned.clone();
        let mut prepared = replay.release_barriers.clone();
        let mut completed = replay.completed_releases.clone();
        assert!(
            apply_frame_with_ownership_limit(
                &mut live,
                &mut committed,
                &mut plan_tombstoned,
                &mut prepared,
                &mut completed,
                frame.clone(),
                8,
            )
            .is_err(),
            "reference replay must reject adversarial frame {frame:?}"
        );
        assert!(
            indexed.transition(&frame, 8).is_err(),
            "indexed replay must reject adversarial frame {frame:?}"
        );
        assert_eq!(
            indexed, before,
            "indexed rejection must not partially mutate any index"
        );
    }
}
#[test]
fn runtime_semantic_preflight_rejects_invalid_frames_before_durable_append() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("semantic-runtime-preflight.norito");
    let first = indexed_record(0);
    let barrier = release_barrier(core::slice::from_ref(&first), 73);
    let (mut journal, _) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
    journal
        .put_batch(vec![first.clone()])
        .expect("persist live reservation");
    journal
        .prepare_release(barrier.clone())
        .expect("persist prepared release");
    let durable_len = fs::metadata(&path).expect("journal metadata").len();
    let mut wrong_completion = release_completion(core::slice::from_ref(&first), 73);
    wrong_completion.ordered_records[0].enqueue_timestamp_ms = wrong_completion.ordered_records[0]
        .enqueue_timestamp_ms
        .saturating_add(1);
    let mut conflicting_barrier = barrier.clone();
    conflicting_barrier.retirement_hash = Hash::new(b"runtime-preflight-conflicting-release");
    assert!(journal.release(first.key).is_err());
    assert!(journal.prepare_release(conflicting_barrier).is_err());
    assert!(journal.forget_release(barrier).is_err());
    assert!(journal.complete_release(wrong_completion).is_err());
    assert_eq!(
        fs::metadata(&path).expect("journal metadata").len(),
        durable_len,
        "semantic rejections must precede all journal writes"
    );
    assert!(
        !journal.durability_ambiguous(),
        "deterministic semantic rejection must leave the journal usable"
    );
}
#[test]
fn production_replay_handles_many_singleton_frames_with_exact_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("many-singleton-frames.norito");
    let records = (0..MAX_MERGE_EXECUTION_ENTRYPOINTS)
        .map(indexed_record)
        .collect::<Vec<_>>();
    let mut bytes = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    for record in &records {
        bytes.extend_from_slice(
            &encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
                record.clone(),
            ]))
            .expect("encode singleton reservation"),
        );
    }
    fs::write(&path, &bytes).expect("write singleton-frame journal");
    let file_len = u64::try_from(bytes.len()).expect("journal length fits u64");
    let limits = LaneQueueReservationJournalLimits::new(
        file_len,
        u64::from(u32::MAX),
        file_len,
        records.len(),
    );
    let (_journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("indexed production replay");
    assert_eq!(replay.records(), records);
    assert!(replay.committed().is_empty());
    assert!(replay.release_barriers().is_empty());
    assert!(replay.completed_releases().is_empty());
}
#[cfg(windows)]
#[test]
fn journal_rejects_reparse_point_file_when_platform_allows_fixture() {
    use std::os::windows::fs::symlink_file;
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target");
    File::create(&target).expect("create target");
    let path = dir.path().join("journal-reparse");
    match symlink_file(&target, &path) {
        Ok(()) => {
            let metadata =
                secure_file_metadata::from_path(&path).expect("reparse metadata");
            assert!(journal_file_is_reparse_point(&metadata));
            assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
        Err(error) => panic!("create reparse fixture: {error}"),
    }
}
