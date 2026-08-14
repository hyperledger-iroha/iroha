// Reservation-journal codec, bound, crash, and replay regression tests.
#[test]
fn frame_decoder_rejects_an_advertised_alternate_layout() {
    let frame = LaneQueueReservationJournalFrameV6::PutBatch(vec![record(18, 3)]);
    let canonical = norito::encode_canonical(&frame).expect("encode canonical frame payload");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&frame).expect("encode alternate-layout frame payload")
    };
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes::<LaneQueueReservationJournalFrameV6>(&alternate)
            .expect("ordinary Norito accepts the advertised alternate layout"),
        frame
    );
    let limits =
        LaneQueueReservationJournalLimits::new(u64::MAX, u64::from(u32::MAX), u64::MAX, usize::MAX);
    let error = decode_frame(&alternate, limits)
        .expect_err("durable frame decoding must reject alternate layouts");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "lane reservation journal payload is not canonically encoded"
    );
}
#[test]
fn configured_frame_limit_rejects_valid_oversized_payload_before_replay() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bounded-frame.norito");
    let bootstrap = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    let operation = LaneQueueReservationJournalFrameV6::PutBatch(vec![record(1, 1)]);
    let operation_payload =
        norito::encode_canonical(&operation).expect("encode canonical operation payload");
    let operation_frame = encode_frame(&operation).expect("encode operation frame");
    let configured_payload_limit =
        u64::try_from(operation_payload.len() - 1).expect("payload length fits u64");
    let bootstrap_payload_len = u64::try_from(
        norito::encode_canonical(&bootstrap_frame())
            .expect("canonical bootstrap payload")
            .len(),
    )
    .expect("bootstrap payload fits u64");
    assert!(configured_payload_limit >= bootstrap_payload_len);
    let mut bytes = bootstrap;
    bytes.extend_from_slice(&operation_frame);
    fs::write(&path, &bytes).expect("write exact journal");
    let file_len = u64::try_from(bytes.len()).expect("journal length fits u64");
    let limits =
        LaneQueueReservationJournalLimits::new(file_len, configured_payload_limit, file_len, 8);
    let error = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .err()
        .expect("configured payload limit must reject the frame");
    assert!(
        error.to_string().contains("configured payload limit"),
        "unexpected error: {error}"
    );
}
#[test]
fn configured_file_limit_rejects_oversized_startup_journal_before_scan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bounded-file.norito");
    drop(
        LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("create journal")
            .0,
    );
    let admitted_len = fs::metadata(&path).expect("journal metadata").len();
    OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open journal")
        .write_all(&[0xA5])
        .expect("extend journal beyond configured limit");
    let limits =
        LaneQueueReservationJournalLimits::new(admitted_len, u64::from(u32::MAX), admitted_len, 8);
    let error = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .err()
        .expect("oversized file must fail before replay");
    assert!(
        error.to_string().contains("exceeds configured limit"),
        "unexpected error: {error}"
    );
}
#[test]
fn replay_rejects_more_distinct_owners_than_configured_queue_capacity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bounded-owners.norito");
    let first = LaneQueueReservationJournalFrameV6::PutBatch(vec![record(2, 1)]);
    let second = LaneQueueReservationJournalFrameV6::PutBatch(vec![record(3, 1)]);
    let shrink = LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![record(3, 1).key]);
    let mut bytes = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    bytes.extend_from_slice(&encode_frame(&first).expect("encode first owner"));
    bytes.extend_from_slice(&encode_frame(&second).expect("encode second owner"));
    bytes.extend_from_slice(&encode_frame(&shrink).expect("encode later shrink"));
    fs::write(&path, &bytes).expect("write exact journal");
    let file_len = u64::try_from(bytes.len()).expect("journal length fits u64");
    let max_payload = [
        norito::encode_canonical(&bootstrap_frame()).expect("encode bootstrap payload"),
        norito::encode_canonical(&first).expect("encode first payload"),
        norito::encode_canonical(&second).expect("encode second payload"),
        norito::encode_canonical(&shrink).expect("encode shrink payload"),
    ]
    .into_iter()
    .map(|payload| u64::try_from(payload.len()).expect("payload length fits u64"))
    .max()
    .expect("payload set is non-empty");
    let limits = LaneQueueReservationJournalLimits::new(file_len, max_payload, file_len, 1);
    let error = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .err()
        .expect("second distinct owner must exceed the configured capacity");
    assert!(
        error.to_string().contains("above configured limit 1"),
        "unexpected error: {error}"
    );
    assert!(
        error.to_string().contains("owns 2 transactions"),
        "a later release must not excuse an over-limit replay prefix: {error}"
    );
}
#[test]
fn invalid_runtime_limits_fail_before_creating_a_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (name, limits) in [
        (
            "zero-frame",
            LaneQueueReservationJournalLimits::new(1, 0, 1, 1),
        ),
        (
            "zero-owner",
            LaneQueueReservationJournalLimits::new(1, 1, 1, 0),
        ),
        (
            "threshold-over-file",
            LaneQueueReservationJournalLimits::new(2, 1, 1, 1),
        ),
    ] {
        let path = dir.path().join(format!("{name}.norito"));
        assert!(
            LaneQueueReservationJournal::open_with_limits(&path, limits).is_err(),
            "{name} limits must fail closed"
        );
        assert!(
            !path.exists(),
            "{name} limits must be rejected before storage creation"
        );
    }
}
#[test]
fn every_execution_group_rejects_4097_members_before_mutating_replay_state() {
    let oversized = (0..=MAX_MERGE_EXECUTION_ENTRYPOINTS)
        .map(indexed_record)
        .collect::<Vec<_>>();
    let sentinel = indexed_record(MAX_MERGE_EXECUTION_ENTRYPOINTS + 1);
    let oversized_barrier = release_barrier(&oversized, 1);
    let oversized_completion = release_completion(&oversized, 2);
    let mut oversized_completion_barrier =
        release_completion(core::slice::from_ref(&oversized[0]), 3);
    oversized_completion_barrier.barrier = oversized_barrier.clone();
    for (label, frame) in [
        (
            "put",
            LaneQueueReservationJournalFrameV6::PutBatch(oversized.clone()),
        ),
        (
            "release",
            LaneQueueReservationJournalFrameV6::ReleaseBatch(
                oversized.iter().map(|record| record.key).collect(),
            ),
        ),
        (
            "prepare-release",
            LaneQueueReservationJournalFrameV6::PrepareRelease(oversized_barrier.clone()),
        ),
        (
            "forget-release",
            LaneQueueReservationJournalFrameV6::ForgetRelease(oversized_barrier),
        ),
        (
            "snapshot-prepare-release",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: vec![release_barrier(&oversized, 4)],
                completed_releases: Vec::new(),
            },
        ),
        (
            "snapshot-complete-release",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: vec![oversized_completion.clone()],
            },
        ),
        (
            "complete-release-records",
            LaneQueueReservationJournalFrameV6::CompleteRelease(oversized_completion),
        ),
        (
            "complete-release-barrier",
            LaneQueueReservationJournalFrameV6::CompleteRelease(oversized_completion_barrier),
        ),
    ] {
        let mut records = vec![sentinel.clone()];
        let mut committed = Vec::new();
        let mut plan_tombstoned = Vec::new();
        let mut release_barriers = Vec::new();
        let mut completed_releases = Vec::new();
        let error = apply_frame(
            &mut records,
            &mut committed,
            &mut plan_tombstoned,
            &mut release_barriers,
            &mut completed_releases,
            frame,
        )
        .expect_err("4,097-member frame must fail closed");
        assert!(
            error.to_string().contains("exceeds canonical limit 4096"),
            "{label}: unexpected rejection: {error}"
        );
        assert_eq!(
            records,
            vec![sentinel.clone()],
            "{label}: cardinality admission must precede replay mutation"
        );
        assert!(committed.is_empty());
        assert!(release_barriers.is_empty());
        assert!(completed_releases.is_empty());
    }
}
#[test]
fn every_snapshot_top_level_vector_obeys_configured_owner_capacity() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let first_barrier = release_barrier(core::slice::from_ref(&first), 1);
    let second_barrier = release_barrier(core::slice::from_ref(&second), 2);
    let first_completion = release_completion(core::slice::from_ref(&first), 3);
    let second_completion = release_completion(core::slice::from_ref(&second), 4);
    let frames = [
        (
            "live",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: vec![first.clone(), second.clone()],
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: Vec::new(),
            },
        ),
        (
            "committed",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: vec![first.key, second.key],
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: Vec::new(),
            },
        ),
        (
            "prepared",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: vec![first_barrier, second_barrier],
                completed_releases: Vec::new(),
            },
        ),
        (
            "completed",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: Vec::new(),
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: vec![first_completion, second_completion],
            },
        ),
    ];
    for (label, frame) in frames {
        let error = validate_frame_cardinality(&frame, 1)
            .expect_err("two snapshot entries must exceed configured capacity one");
        assert!(
            error
                .to_string()
                .contains("exceeds configured ownership limit 1"),
            "{label}: unexpected error: {error}"
        );
    }
}
#[test]
fn runtime_owner_limit_rejects_before_write_and_remains_reopenable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("runtime-owner-limit.norito");
    let limits = LaneQueueReservationJournalLimits::new(u64::MAX, u64::from(u32::MAX), u64::MAX, 1);
    let first = indexed_record(0);
    let second = indexed_record(1);
    let (mut journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("create bounded journal");
    assert!(replay.records().is_empty());
    journal
        .put_batch(vec![first.clone()])
        .expect("first owner fits configured capacity");
    let durable_len = fs::metadata(&path).expect("journal metadata").len();
    let error = journal
        .put_batch(vec![second])
        .expect_err("second distinct owner must fail before append");
    assert!(error.to_string().contains("above configured limit 1"));
    assert!(!journal.durability_ambiguous());
    assert_eq!(
        fs::metadata(&path).expect("journal metadata").len(),
        durable_len,
        "owner admission rejection must not extend the durable file"
    );
    drop(journal);
    let (_journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("the admitted prefix must remain restartable");
    assert_eq!(replay.records(), &[first]);
}
#[test]
fn stale_forget_release_does_not_undercount_completed_ownership() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stale-forget-owner-limit.norito");
    let limits = LaneQueueReservationJournalLimits::new(u64::MAX, u64::from(u32::MAX), u64::MAX, 1);
    let first = indexed_record(0);
    let second = indexed_record(1);
    let completion = release_completion(core::slice::from_ref(&first), 5);
    let mut stale = completion.barrier.clone();
    stale.retirement_hash = Hash::new(b"stale-forget-owner-limit");
    let (mut journal, _, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("create bounded journal");
    journal.put_batch(vec![first]).expect("persist first owner");
    journal
        .prepare_release(completion.barrier.clone())
        .expect("prepare exact release");
    journal
        .complete_release(completion.clone())
        .expect("complete exact release");
    journal
        .forget_release(stale)
        .expect("stale full barrier identity is a durable no-op");
    let durable_len = fs::metadata(&path).expect("journal metadata").len();
    let error = journal
        .put_batch(vec![second])
        .expect_err("completed ownership must still consume the configured slot");
    assert!(error.to_string().contains("above configured limit 1"));
    assert_eq!(
        fs::metadata(&path).expect("journal metadata").len(),
        durable_len,
        "an under-capacity rejection must precede durable append"
    );
    drop(journal);
    let (_journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("the exact completed owner remains replayable");
    assert_eq!(replay.completed_releases(), &[completion]);
}
#[test]
fn replay_rejects_same_length_valid_content_mutation_on_retained_append_handle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("same-length-mutation.norito");
    let mut original = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    original.extend_from_slice(
        &encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![record(
            1, 1,
        )]))
        .expect("encode original operation"),
    );
    let mut alternate = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
    alternate.extend_from_slice(
        &encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![record(
            2, 1,
        )]))
        .expect("encode alternate operation"),
    );
    assert_eq!(
        original.len(),
        alternate.len(),
        "fixture journals must have the same exact length"
    );
    fs::write(&path, &original).expect("write original journal");
    let len = u64::try_from(original.len()).expect("fixture length fits u64");
    let limits = LaneQueueReservationJournalLimits::new(len, u64::from(u32::MAX), len, 8);
    let mut file = open_regular_append(&path).expect("open retained append handle");
    let identity = verify_open_regular_path(&path, &file).expect("file identity");
    let parent = open_regular_parent(&path).expect("open parent");
    let parent_identity = verify_open_regular_parent(&path, &parent).expect("parent identity");
    let error = replay_open_file_after_initial_hash(
        &path,
        &mut file,
        identity,
        len,
        &parent,
        parent_identity,
        limits,
        || {
            fs::write(&path, &alternate)?;
            Ok(())
        },
    )
    .expect_err("a valid same-length in-place replacement must fail closed");
    assert!(
        error
            .to_string()
            .contains("content or metadata changed during replay"),
        "unexpected error: {error}"
    );
}
#[test]
fn ownership_limit_is_checked_before_applying_the_exceeding_prefix() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let mut records = vec![first.clone()];
    let mut committed = Vec::new();
    let mut plan_tombstoned = Vec::new();
    let mut release_barriers = Vec::new();
    let mut completed_releases = Vec::new();
    let error = apply_frame_with_ownership_limit(
        &mut records,
        &mut committed,
        &mut plan_tombstoned,
        &mut release_barriers,
        &mut completed_releases,
        LaneQueueReservationJournalFrameV6::PutBatch(vec![second]),
        1,
    )
    .expect_err("second owner must be rejected before replay mutation");
    assert!(error.to_string().contains("above configured limit 1"));
    assert_eq!(records, vec![first]);
    assert!(committed.is_empty());
    assert!(release_barriers.is_empty());
    assert!(completed_releases.is_empty());
}
#[test]
fn ownership_union_counts_tombstones_and_completed_releases_exactly_once() {
    let first = indexed_record(0);
    let second = indexed_record(1);
    let mut live = vec![first.clone()];
    let mut committed = Vec::new();
    let mut plan_tombstoned = Vec::new();
    let mut prepared = Vec::new();
    let mut completed = Vec::new();
    let error = apply_frame_with_ownership_limit(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut prepared,
        &mut completed,
        LaneQueueReservationJournalFrameV6::Commit(second.key),
        1,
    )
    .expect_err("a runtime commit cannot create a missing owner");
    assert!(
        error
            .to_string()
            .contains("requires an exact live reservation")
    );
    assert_eq!(live, vec![first.clone()]);
    assert!(committed.is_empty());
    let prepared_for_first = release_barrier(core::slice::from_ref(&first), 4);
    apply_frame_with_ownership_limit(
        &mut live,
        &mut committed,
        &mut plan_tombstoned,
        &mut prepared,
        &mut completed,
        LaneQueueReservationJournalFrameV6::PrepareRelease(prepared_for_first.clone()),
        1,
    )
    .expect("a prepared view of the same live owner must not be double-counted");
    assert_eq!(live, vec![first.clone()]);
    assert_eq!(prepared, vec![prepared_for_first]);
    let sentinel = indexed_record(2);
    let mut current_live = vec![sentinel.clone()];
    let second_completion = release_completion(core::slice::from_ref(&second), 5);
    let error = apply_frame_with_ownership_limit(
        &mut current_live,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        LaneQueueReservationJournalFrameV6::Snapshot {
            live: vec![first],
            committed: Vec::new(),
            plan_tombstoned: Vec::new(),
            release_barriers: Vec::new(),
            completed_releases: vec![second_completion],
        },
        1,
    )
    .expect_err("snapshot live and completed ownership must share one configured union");
    assert!(error.to_string().contains("above configured limit 1"));
    assert_eq!(
        current_live,
        vec![sentinel],
        "an over-limit snapshot union must be rejected before replacing current replay state"
    );
}
#[test]
fn deterministic_file_budget_exhaustion_does_not_poison_or_extend_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bounded-append.norito");
    let bootstrap_bytes = u64::try_from(
        encode_frame(&bootstrap_frame())
            .expect("encode bootstrap")
            .len(),
    )
    .expect("bootstrap length fits u64");
    let limits = LaneQueueReservationJournalLimits::new(
        bootstrap_bytes,
        u64::from(u32::MAX),
        bootstrap_bytes,
        8,
    );
    let (mut journal, replay, _seal) = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .expect("create exactly bootstrap-sized journal");
    assert!(replay.records().is_empty());
    for _ in 0..2 {
        let error = journal
            .put_batch(vec![record(1, 1)])
            .expect_err("configured file exhaustion must reject before writing");
        assert!(
            error.to_string().contains("exceeds configured limit"),
            "unexpected append rejection: {error}"
        );
        assert!(
            !journal.durability_ambiguous(),
            "a deterministic pre-write bound must not poison the journal"
        );
        assert_eq!(
            fs::metadata(&path).expect("journal metadata").len(),
            bootstrap_bytes,
            "rejected append must leave the durable prefix unchanged"
        );
    }
}
#[test]
fn decoder_allocation_ceiling_tracks_configured_frame_budget() {
    let configured = 128_u64 * 1024 * 1024;
    let budget = frame_decode_allocation_budget(
        usize::try_from(configured).expect("fixture fits usize"),
        configured,
    )
    .expect("bounded allocation budget");
    assert_eq!(
        budget,
        usize::try_from(configured)
            .expect("fixture fits usize")
            .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES)
    );
    assert!(
        budget
            < usize::try_from(configured)
                .expect("fixture fits usize")
                .saturating_mul(FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT),
        "near-limit hostile frames must not receive the full calibrated multiplier"
    );
}
fn apply_unprotected_frame(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    frame: LaneQueueReservationJournalFrameV6,
) -> io::Result<()> {
    apply_frame(
        records,
        committed,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        frame,
    )
}
#[test]
fn crash_at_every_operation_frame_write_boundary_is_prefix_atomic() {
    let first = record(1, 1);
    let second = record(2, 1);
    let barrier = release_barrier(core::slice::from_ref(&first), 1);
    let completion = release_completion(core::slice::from_ref(&first), 1);
    let first_frame = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
        first.clone(),
    ]))
    .expect("encode first frame");
    let bootstrap = encode_frame(&bootstrap_frame()).expect("encode V6 bootstrap");
    let cases = [
        (
            "put",
            LaneQueueReservationJournalFrameV6::PutBatch(vec![second]),
        ),
        (
            "release",
            LaneQueueReservationJournalFrameV6::ReleaseBatch(vec![first.key]),
        ),
        (
            "commit",
            LaneQueueReservationJournalFrameV6::Commit(first.key),
        ),
        (
            "forget-commit",
            LaneQueueReservationJournalFrameV6::ForgetCommit(first.key),
        ),
        (
            "snapshot",
            LaneQueueReservationJournalFrameV6::Snapshot {
                live: Vec::new(),
                committed: vec![first.key],
                plan_tombstoned: Vec::new(),
                release_barriers: Vec::new(),
                completed_releases: Vec::new(),
            },
        ),
        (
            "prepare-release",
            LaneQueueReservationJournalFrameV6::PrepareRelease(barrier.clone()),
        ),
        (
            "complete-release",
            LaneQueueReservationJournalFrameV6::CompleteRelease(completion),
        ),
        (
            "forget-release",
            LaneQueueReservationJournalFrameV6::ForgetRelease(barrier),
        ),
    ];
    for (label, operation) in cases {
        let operation_frame = encode_frame(&operation).expect("encode operation frame");
        for written in 0..operation_frame.len() {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("reservations.norito");
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&path)
                .expect("open raw journal");
            file.write_all(&bootstrap).expect("write V6 bootstrap");
            file.write_all(&first_frame).expect("write first frame");
            file.write_all(&operation_frame[..written])
                .expect("write partial operation frame");
            drop(file);
            let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
                .expect("repair truncated boundary");
            assert_eq!(
                replay.records(),
                &[first.clone()],
                "{label} boundary {written} must expose only the preceding durable frame"
            );
            assert!(replay.committed().is_empty());
            assert!(replay.release_barriers().is_empty());
            assert!(replay.completed_releases().is_empty());
        }
    }
}
#[test]
fn corrupt_complete_suffix_fails_closed_without_truncation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reservations.norito");
    let first = record(1, 1);
    let second = record(2, 1);
    let third = record(3, 1);
    let first_frame = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
        first.clone(),
    ]))
    .expect("encode first");
    let mut corrupt = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![second]))
        .expect("encode second");
    let third_frame = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![third]))
        .expect("encode third");
    let bootstrap = encode_frame(&bootstrap_frame()).expect("encode V6 bootstrap");
    let corrupt_index = corrupt.len() - 1;
    corrupt[corrupt_index] ^= 0x80;
    let mut file = File::create(&path).expect("create journal");
    file.write_all(&bootstrap).expect("write V6 bootstrap");
    file.write_all(&first_frame).expect("write first");
    file.write_all(&corrupt).expect("write corrupt second");
    file.write_all(&third_frame).expect("write trailing third");
    drop(file);
    let corrupt_len = path.metadata().expect("metadata").len();
    assert!(
        LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
        "a fully written frame with a bad commit/checksum is corruption, not a torn write"
    );
    assert_eq!(
        path.metadata().expect("metadata after rejection").len(),
        corrupt_len,
        "fail-closed recovery must retain corrupt evidence for operator repair"
    );
}
#[test]
fn legacy_and_unknown_frame_magic_are_rejected_without_rewrite() {
    for (label, magic) in [
        ("v1", *b"IRQRJNL1"),
        ("v2", *b"IRQRJNL2"),
        ("v3", *b"IRQRJNL3"),
        ("v5", *b"IRQRJNL5"),
        ("unknown", *b"IRQRJNL9"),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("{label}.norito"));
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        fs::write(&path, &bytes).expect("write legacy or unknown header");
        assert!(
            LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
            "{label} journal magic must fail closed"
        );
        assert_eq!(
            fs::read(&path).expect("retain rejected bytes"),
            bytes,
            "{label} evidence must not be rewritten as a V6 journal"
        );
    }
}
#[test]
fn complete_v5_envelope_is_hard_rejected_without_decode_or_rewrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("retired-v5-envelope.norito");
    let mut bytes = encode_frame(&bootstrap_frame()).expect("encode complete V6 fixture");
    bytes[..RESERVATION_JOURNAL_FRAME_MAGIC.len()].copy_from_slice(b"IRQRJNL5");
    let version_start = RESERVATION_JOURNAL_FRAME_MAGIC.len();
    let version_end = version_start + 2;
    bytes[version_start..version_end].copy_from_slice(&5_u16.to_le_bytes());
    fs::write(&path, &bytes).expect("write complete retired V5 envelope");
    let error = LaneQueueReservationJournal::open(&path, u64::MAX)
        .err()
        .expect("V5 envelope must fail before payload decoding");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("only bootstrapped V6 is supported"),
        "unexpected V5 envelope rejection: {error}"
    );
    assert_eq!(
        fs::read(&path).expect("retain rejected V5 envelope"),
        bytes,
        "V5 evidence must never be migrated, truncated, or rewritten"
    );
}
#[test]
fn complete_v3_frames_are_rejected_without_repair_or_rewrite() {
    let mut legacy_record = record(1, 1);
    legacy_record.version = 3;
    legacy_record.fifo_order.version = 3;
    let frames = [
        encode_v3_fixture_frame(&LaneQueueReservationJournalFrameV3Fixture::PutBatch(vec![
            legacy_record.clone(),
        ])),
        encode_v3_fixture_frame(&LaneQueueReservationJournalFrameV3Fixture::Release(
            legacy_record.key,
        )),
    ];
    for (index, bytes) in frames.into_iter().enumerate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("v3-frame-{index}.norito"));
        fs::write(&path, &bytes).expect("write complete V3 frame fixture");
        let original_len = path.metadata().expect("V3 metadata").len();
        let error = LaneQueueReservationJournal::open(&path, u64::MAX)
            .err()
            .expect("a complete V3 frame must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("frame magic mismatch"),
            "unexpected V3 rejection: {error}"
        );
        assert_eq!(
            path.metadata().expect("metadata after rejection").len(),
            original_len,
            "complete V3 evidence must not be truncated"
        );
        assert_eq!(
            fs::read(&path).expect("retain complete V3 evidence"),
            bytes,
            "complete V3 evidence must not be rewritten as V6"
        );
    }
}
#[test]
fn complete_v4_bootstrap_is_rejected_without_repair_or_rewrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("v4-bootstrap.norito");
    let bytes = encode_v4_bootstrap_fixture();
    fs::write(&path, &bytes).expect("write complete V4 bootstrap fixture");
    let error = LaneQueueReservationJournal::open(&path, u64::MAX)
        .err()
        .expect("a complete V4 bootstrap must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("frame magic mismatch"),
        "unexpected V4 rejection: {error}"
    );
    assert_eq!(
        fs::read(&path).expect("retain complete V4 evidence"),
        bytes,
        "complete V4 evidence must not be rewritten as V6"
    );
}
#[test]
fn v6_envelope_rejects_unsupported_record_versions_without_rewrite() {
    for unsupported_version in [3, 4, 6] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join(format!("v6-envelope-v{unsupported_version}-record.norito"));
        let mut unsupported = record(1, 1);
        unsupported.version = unsupported_version;
        unsupported.fifo_order.version = unsupported_version;
        let bytes = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
            unsupported,
        ]))
        .expect("encode V6 envelope around unsupported record");
        let mut journal_bytes = encode_frame(&bootstrap_frame()).expect("encode V6 bootstrap");
        journal_bytes.extend_from_slice(&bytes);
        fs::write(&path, &journal_bytes).expect("write version-mismatched frame");
        let error = LaneQueueReservationJournal::open(&path, u64::MAX)
            .err()
            .expect("unsupported record inside a V6 envelope must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("retain version-mismatched evidence"),
            journal_bytes,
            "version-mismatched evidence must not be rewritten"
        );
    }
}
#[test]
fn v6_envelope_rejects_mismatched_reservation_identity_without_rewrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("v6-mismatched-reservation-identity.norito");
    let mut mismatched = record(1, 1);
    mismatched.key.signed_transaction_hash =
        typed_hash::<SignedTransaction>(b"mismatched-signed-transaction");
    let frame = encode_frame(&LaneQueueReservationJournalFrameV6::PutBatch(vec![
        mismatched,
    ]))
    .expect("encode mismatched reservation identity");
    let mut journal_bytes = encode_frame(&bootstrap_frame()).expect("encode V6 bootstrap");
    journal_bytes.extend_from_slice(&frame);
    fs::write(&path, &journal_bytes).expect("write mismatched reservation identity");
    let error = LaneQueueReservationJournal::open(&path, u64::MAX)
        .err()
        .expect("mismatched reservation identity must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("compatibility transaction hash does not match its entrypoint"),
        "unexpected mismatched-identity rejection: {error}",
    );
    assert_eq!(
        fs::read(&path).expect("retain mismatched reservation evidence"),
        journal_bytes,
        "mismatched reservation evidence must not be rewritten",
    );
}
#[test]
fn v6_release_batch_replay_is_atomic_idempotent_and_exact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("v6-release-batch.norito");
    let first = record(1, 1);
    let second = record(2, 1);
    let third = record(3, 1);
    let released = vec![first.key, third.key];
    {
        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create V6 journal");
        assert!(replay.records().is_empty());
        journal
            .put_batch(vec![first.clone(), second.clone(), third])
            .expect("persist V6 reservation batch");
        journal
            .release_batch(released.clone())
            .expect("atomically release two exact reservations");
    }
    let (mut journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay V6 release batch");
    assert_eq!(
        replay.records(),
        core::slice::from_ref(&second),
        "one V6 ReleaseBatch frame must remove every exact member"
    );
    let mut replacement = first;
    replacement.key.routing_plan_digest = Hash::new(b"replacement-plan");
    replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal");
    journal
        .put_batch(vec![replacement.clone()])
        .expect("re-admit same hash under a distinct exact owner");
    journal
        .release_batch(released.clone())
        .expect("replay stale exact release batch");
    journal
        .release_batch(released)
        .expect("repeat stale exact release batch idempotently");
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay exact V6 history");
    assert_eq!(
        replay.records(),
        &[second, replacement],
        "a repeated V6 batch must not remove a later non-identical reservation"
    );
    assert!(replay.committed().is_empty());
    assert!(replay.release_barriers().is_empty());
    assert!(replay.completed_releases().is_empty());
}
#[test]
fn duplicate_exact_replay_is_idempotent_but_conflicting_owner_is_rejected() {
    let exact = record(1, 1);
    let mut records = Vec::new();
    let mut committed = Vec::new();
    apply_unprotected_frame(
        &mut records,
        &mut committed,
        LaneQueueReservationJournalFrameV6::PutBatch(vec![exact.clone(), exact.clone()]),
    )
    .expect("duplicate exact record");
    assert_eq!(records, vec![exact.clone()]);
    let mut conflicting = exact;
    conflicting.key.reservation_owner_hash = Hash::new(b"conflicting-owner");
    assert!(
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![conflicting]),
        )
        .is_err()
    );
    let mut conflicting_plan = records[0].clone();
    conflicting_plan.key.routing_plan_digest = Hash::new(b"conflicting-plan");
    assert!(
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![conflicting_plan]),
        )
        .is_err()
    );
    let mut conflicting_fifo_order = record(3, 1);
    conflicting_fifo_order.fifo_order = records[0].fifo_order;
    assert!(
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![conflicting_fifo_order]),
        )
        .is_err(),
        "one durable FIFO ordinal cannot identify two transaction hashes"
    );
    let mut participant = record(2, 1);
    participant.key.coordinator_leg.role = RouteLegRole::Participant;
    assert!(
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV6::PutBatch(vec![participant]),
        )
        .is_err(),
        "participant legs must never become full-transaction reservations"
    );
}
#[test]
fn reservation_record_rejects_mismatched_primary_hashes_atomically() {
    let existing = record(1, 1);
    let mut records = vec![existing.clone()];
    let mut committed = vec![existing.key];
    let mut mismatched = record(2, 1);
    mismatched.key.signed_transaction_hash =
        typed_hash::<SignedTransaction>(b"mismatched-signed-transaction");
    let records_before = records.clone();
    let committed_before = committed.clone();
    let error = apply_unprotected_frame(
        &mut records,
        &mut committed,
        LaneQueueReservationJournalFrameV6::PutBatch(vec![mismatched]),
    )
    .expect_err("malformed reservation identity must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        records, records_before,
        "failed validation must not mutate live reservations",
    );
    assert_eq!(
        committed, committed_before,
        "failed validation must not mutate commit barriers",
    );
}
#[test]
fn ordered_release_survives_every_restart_phase_and_exact_retries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ordered-release.norito");
    let records = vec![record(1, 1), record(2, 1)];
    let barrier = release_barrier(&records, 1);
    let completion = release_completion(&records, 1);
    {
        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
        assert!(replay.records().is_empty());
        journal
            .put_batch(records.clone())
            .expect("persist exact reservation batch");
        journal
            .prepare_release(barrier.clone())
            .expect("prepare ordered release");
        journal
            .prepare_release(barrier.clone())
            .expect("repeat exact prepare");
    }
    let (mut journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay prepared release");
    assert_eq!(replay.records(), records.as_slice());
    assert_eq!(replay.release_barriers(), &[barrier.clone()]);
    assert!(replay.completed_releases().is_empty());
    journal
        .complete_release(completion.clone())
        .expect("complete ordered release");
    journal
        .complete_release(completion.clone())
        .expect("repeat exact completion");
    journal
        .prepare_release(barrier.clone())
        .expect("retry prepare after completion");
    drop(journal);
    let (mut journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay completed release");
    assert!(replay.records().is_empty());
    assert!(replay.release_barriers().is_empty());
    assert_eq!(replay.completed_releases(), &[completion]);
    journal
        .forget_release(barrier.clone())
        .expect("forget exact completion");
    journal
        .forget_release(barrier)
        .expect("repeat exact forget");
    drop(journal);
    let (_journal, replay) =
        LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay forgotten release");
    assert!(replay.records().is_empty());
    assert!(replay.release_barriers().is_empty());
    assert!(replay.completed_releases().is_empty());
}
