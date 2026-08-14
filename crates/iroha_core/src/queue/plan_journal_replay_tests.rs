// Queue-plan replay, compaction, and storage-identity regression tests.
//
// Included by `queue::journal::tests` to preserve exact libtest names.
#[test]
fn prepared_replay_rejects_same_length_content_tamper_before_callback() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tampered-latest-offset.norito");
    let first = record("tampered-offset-first");
    let second = record("tampered-offset-second");
    let replacement = with_single_route(first.clone(), 47, 53);
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
    for record in [&first, &second, &replacement] {
        journal
            .put_deferred_flush(record.clone())
            .expect("append tamper fixture");
    }
    journal.sync_all_with_parent().expect("sync tamper fixture");
    let verified_replay = journal
        .prepare_replay()
        .expect("prepare owned replay snapshot");
    let callback_replay = journal
        .prepare_replay()
        .expect("prepare callback replay snapshot");
    let replacement_position = u64::try_from(
        raw_bootstrap_frame().len()
            + raw_frame(&QueuePlanJournalFrameV4::Put(first)).len()
            + raw_frame(&QueuePlanJournalFrameV4::Put(second)).len(),
    )
    .expect("replacement position fits u64");
    let payload_position = replacement_position
        .checked_add(FRAME_HEADER_BYTES)
        .expect("payload position fits u64");
    let mut tamper = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open fixture for in-place tamper");
    tamper
        .seek(SeekFrom::Start(payload_position))
        .expect("seek payload byte");
    let mut byte = [0_u8; 1];
    tamper.read_exact(&mut byte).expect("read payload byte");
    byte[0] ^= 0x01;
    tamper
        .seek(SeekFrom::Start(payload_position))
        .expect("rewind payload byte");
    tamper.write_all(&byte).expect("tamper payload byte");
    tamper.sync_all().expect("publish in-place tamper");
    let error = verified_replay
        .into_verified_records()
        .expect_err("tampered latest frame must return no owned replay");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("snapshot content changed"),
        "unexpected owned-replay tamper error: {error}",
    );
    let mut callbacks = 0_usize;
    let error = callback_replay
        .for_each_record(|_record| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("tampered latest frame must fail content-bound replay");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("snapshot content changed"),
        "unexpected tamper error: {error}"
    );
    assert_eq!(callbacks, 0, "tampered owner must not reach the callback");
}
#[test]
fn materialized_replay_rejects_wrong_record_identity_before_callback() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("wrong-indexed-put.norito");
    let first = record("wrong-index-first");
    let second = record("wrong-index-second");
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
    journal
        .put_deferred_flush(first.clone())
        .expect("append first");
    journal
        .put_deferred_flush(second.clone())
        .expect("append second");
    journal.sync_all_with_parent().expect("sync fixture");
    let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
    replay
        .live_positions
        .get_mut(&first.entrypoint_hash)
        .expect("first live index")
        .record = second;
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_record| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("wrong materialized Put identity must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("materialized live frame identity"),
        "unexpected materialized-identity error: {error}"
    );
    assert_eq!(
        callbacks, 0,
        "wrong materialized Put must not reach callback"
    );
}
#[test]
fn materialized_replay_rejects_later_record_corruption_before_any_callback() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("wrong-later-indexed-put.norito");
    let first = record("wrong-later-index-first");
    let second = record("wrong-later-index-second");
    let second_key = second.entrypoint_hash;
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
    journal
        .put_deferred_flush(first.clone())
        .expect("append first");
    journal.put_deferred_flush(second).expect("append second");
    journal.sync_all_with_parent().expect("sync fixture");
    let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
    replay
        .live_positions
        .get_mut(&second_key)
        .expect("later live index")
        .record = first;
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_record| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("wrong later materialized Put identity must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("materialized live frame identity"),
        "unexpected later materialized-identity error: {error}",
    );
    assert_eq!(
        callbacks, 0,
        "a valid earlier record must remain private when a later record is corrupt",
    );
}
#[test]
fn materialized_replay_rejects_same_identity_and_plan_with_changed_claim() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("changed-indexed-claim.norito");
    let original = record("changed-indexed-claim");
    let key = original.entrypoint_hash;
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
    journal
        .put_deferred_flush(original)
        .expect("append original claim");
    journal.sync_all_with_parent().expect("sync original claim");
    let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
    let materialized = &mut replay
        .live_positions
        .get_mut(&key)
        .expect("materialized claim")
        .record;
    materialized.enqueue_timestamp_ms = materialized.enqueue_timestamp_ms.saturating_add(1);
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("changed materialized claim must fail replay");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("or claim changed"),
        "unexpected claim mutation error: {error}"
    );
    assert_eq!(callbacks, 0, "changed claim must not reach the callback");
}
#[test]
fn prepared_replay_rejects_valid_historical_remove_rewrite_before_callback() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("changed-historical-remove.norito");
    let first = record("historical-remove-first");
    let second = record("historical-remove-second");
    let first_put = raw_frame(&QueuePlanJournalFrameV4::Put(first.clone()));
    let second_put = raw_frame(&QueuePlanJournalFrameV4::Put(second.clone()));
    let original_remove = raw_frame(&QueuePlanJournalFrameV4::Remove {
        entrypoint_hash: first.entrypoint_hash,
        plan_digest: first.plan_digest(),
        claim_digest: first.claim_digest().expect("hash first claim"),
    });
    let changed_remove = raw_frame(&QueuePlanJournalFrameV4::Remove {
        entrypoint_hash: second.entrypoint_hash,
        plan_digest: second.plan_digest(),
        claim_digest: second.claim_digest().expect("hash second claim"),
    });
    assert_eq!(
        original_remove.len(),
        changed_remove.len(),
        "fixed-size exact tombstone rewrite must preserve frame length"
    );
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
    journal
        .put_deferred_flush(first.clone())
        .expect("append first owner");
    journal
        .put_deferred_flush(second.clone())
        .expect("append second owner");
    journal
        .remove_many_deferred_flush([(
            first.entrypoint_hash,
            first.plan_digest(),
            first.claim_digest().expect("hash first claim"),
        )])
        .expect("append original historical Remove");
    journal
        .sync_all_with_parent()
        .expect("sync historical Remove fixture");
    let replay = journal.prepare_replay().expect("prepare replay snapshot");
    let remove_position = u64::try_from(
        raw_bootstrap_frame()
            .len()
            .checked_add(first_put.len())
            .and_then(|bytes| bytes.checked_add(second_put.len()))
            .expect("historical Remove position"),
    )
    .expect("historical Remove position fits u64");
    let mut tamper = OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open historical Remove");
    tamper
        .seek(SeekFrom::Start(remove_position))
        .expect("seek historical Remove");
    tamper
        .write_all(&changed_remove)
        .expect("rewrite valid historical Remove");
    tamper.sync_all().expect("publish valid Remove rewrite");
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("historical semantic rewrite must invalidate prepared replay");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("snapshot content changed"),
        "unexpected historical rewrite error: {error}"
    );
    assert_eq!(
        callbacks, 0,
        "historical semantic rewrite must fail before any callback"
    );
}
#[test]
fn compaction_preserves_live_fifo_order_and_uses_v4_frames() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("compact.norito");
    let first = record("compact-first");
    let second = record("compact-second");
    let third = record("compact-third");
    let fourth = record("compact-fourth");
    let compact_limits = QueuePlanJournalLimits::new(
        u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
        TEST_MAX_BYTES,
        TEST_MAX_BYTES,
        64,
    );
    let mut journal =
        QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
    journal
        .put_deferred_flush(first.clone())
        .expect("put first");
    journal
        .put_deferred_flush(second.clone())
        .expect("put second");
    journal
        .put_deferred_flush(third.clone())
        .expect("put third");
    journal
        .remove_many_deferred_flush([(
            second.entrypoint_hash,
            second.plan_digest(),
            second.claim_digest().expect("hash second claim"),
        )])
        .expect("remove second");
    journal.compact_if_needed().expect("compact");
    journal
        .replace_strict_durable(fourth.clone())
        .expect("append through rebound post-compaction handle");
    assert_eq!(
        journal.replay().expect("replay"),
        vec![first.clone(), third.clone(), fourth.clone()]
    );
    assert_eq!(
        read_frames(&path, compact_limits).expect("read compacted frames"),
        vec![
            QueuePlanJournalFrameV4::Put(first),
            QueuePlanJournalFrameV4::Put(third),
            QueuePlanJournalFrameV4::Put(fourth),
        ]
    );
    assert!(
        fs::read(&path)
            .expect("read compacted journal bytes")
            .starts_with(&raw_bootstrap_frame()),
        "compaction must retain the exact durable V4 bootstrap as the first frame"
    );
    assert!(!path.with_extension("tmp").exists());
}
#[test]
fn compaction_failure_after_temp_creation_is_reconciled_on_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("compact-failure.norito");
    let compact_limits = QueuePlanJournalLimits::new(
        u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
        TEST_MAX_BYTES,
        TEST_MAX_BYTES,
        64,
    );
    let mut journal =
        QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
    let expected = record("compact-failure");
    journal
        .put_deferred_flush(expected.clone())
        .expect("append compaction fixture");
    journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate);
    let error = journal
        .compact_if_needed()
        .expect_err("post-create compaction failure must propagate");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(journal.is_poisoned());
    assert!(path.with_extension("tmp").is_file());
    drop(journal);
    assert_eq!(
        QueuePlanJournal::open_with_limits(&path, compact_limits, true)
            .expect("restart reconciles recognized empty compaction temp")
            .replay()
            .expect("replay authoritative canonical journal"),
        vec![expected]
    );
    assert!(
        !path.with_extension("tmp").exists(),
        "reconciled unpromoted temp must be durably removed"
    );
}
#[test]
fn compaction_recovery_validates_atomic_remove_batch_prefix() {
    let first = record("compact-remove-batch-first");
    let second = record("compact-remove-batch-second");
    let exact_removals = vec![
        QueuePlanJournalRemovalV4 {
            entrypoint_hash: first.entrypoint_hash.clone(),
            plan_digest: first.plan_digest(),
            claim_digest: first.claim_digest().expect("hash first claim"),
        },
        QueuePlanJournalRemovalV4 {
            entrypoint_hash: second.entrypoint_hash.clone(),
            plan_digest: second.plan_digest(),
            claim_digest: second.claim_digest().expect("hash second claim"),
        },
    ];
    let mut canonical = raw_bootstrap_frame();
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first.clone())));
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::RemoveBatch(
        exact_removals.clone(),
    )));
    let valid_dir = tempfile::tempdir().expect("valid tempdir");
    let valid_path = valid_dir.path().join("compact-remove-batch-valid.norito");
    fs::write(&valid_path, &canonical).expect("write valid canonical");
    fs::write(valid_path.with_extension("tmp"), raw_bootstrap_frame())
        .expect("write valid compaction prefix");
    let journal = QueuePlanJournal::open_with_limits(&valid_path, limits(2), true)
        .expect("valid atomic batch must reconcile compaction recovery");
    assert!(
        journal
            .replay()
            .expect("replay valid atomic batch")
            .is_empty()
    );
    assert!(!valid_path.with_extension("tmp").exists());
    let absent = record("compact-remove-batch-absent");
    let mut invalid_removals = exact_removals;
    invalid_removals[1] = QueuePlanJournalRemovalV4 {
        entrypoint_hash: absent.entrypoint_hash.clone(),
        plan_digest: absent.plan_digest(),
        claim_digest: absent.claim_digest().expect("hash absent claim"),
    };
    let mut invalid_canonical = raw_bootstrap_frame();
    invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first)));
    invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second)));
    invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::RemoveBatch(
        invalid_removals,
    )));
    let invalid_dir = tempfile::tempdir().expect("invalid tempdir");
    let invalid_path = invalid_dir
        .path()
        .join("compact-remove-batch-invalid.norito");
    fs::write(&invalid_path, &invalid_canonical).expect("write invalid canonical");
    fs::write(invalid_path.with_extension("tmp"), raw_bootstrap_frame())
        .expect("write invalid compaction prefix");
    let error = QueuePlanJournal::open_with_limits(&invalid_path, limits(2), true)
        .err()
        .expect("compaction recovery must reject a partially matching atomic batch");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("compaction recovery RemoveBatch does not match"),
        "unexpected recovery error: {error}"
    );
    assert_eq!(
        fs::read(&invalid_path).expect("retain invalid canonical"),
        invalid_canonical
    );
    assert!(invalid_path.with_extension("tmp").is_file());
}
#[test]
fn recognized_compaction_prefixes_are_reconciled_against_canonical_state() {
    let first = record("compact-prefix-first");
    let second = record("compact-prefix-second");
    let replacement = with_single_route(first.clone(), 71, 73);
    let mut canonical = raw_bootstrap_frame();
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first)));
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(
        replacement.clone(),
    )));
    let mut compacted = raw_bootstrap_frame();
    compacted.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(
        replacement.clone(),
    )));
    let second_position = compacted.len();
    compacted.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
    let header = usize::try_from(FRAME_HEADER_BYTES).expect("frame header");
    let bootstrap_len = raw_bootstrap_frame().len();
    let mut cuts = vec![
        0,
        1,
        header.saturating_sub(1),
        header,
        bootstrap_len.saturating_sub(1),
        bootstrap_len,
        bootstrap_len.saturating_add(1),
        second_position.saturating_sub(1),
        second_position,
        second_position.saturating_add(1),
        compacted.len().saturating_sub(1),
        compacted.len(),
    ];
    cuts.sort_unstable();
    cuts.dedup();
    for cut in cuts {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("compact-prefix-{cut}.norito"));
        fs::write(&path, &canonical).expect("write canonical history");
        fs::write(path.with_extension("tmp"), &compacted[..cut])
            .expect("write recognized compaction prefix");
        let journal = QueuePlanJournal::open_with_limits(&path, limits(2), true)
            .unwrap_or_else(|error| panic!("reconcile compaction prefix at cut {cut}: {error}"));
        assert_eq!(
            journal.replay().expect("replay canonical after recovery"),
            vec![replacement.clone(), second.clone()],
            "cut={cut}"
        );
        drop(journal);
        assert_eq!(
            fs::read(&path).expect("retain canonical history"),
            canonical,
            "unpromoted compaction recovery must not replace the authoritative canonical, cut={cut}"
        );
        assert!(!path.with_extension("tmp").exists(), "cut={cut}");
    }
}
#[test]
fn staged_compaction_temp_tears_are_durably_discarded_against_canonical_state() {
    let expected_record = record("compact-staged-tear");
    let bootstrap = raw_bootstrap_frame();
    let put = raw_frame(&QueuePlanJournalFrameV4::Put(expected_record.clone()));
    let mut canonical = bootstrap.clone();
    canonical.extend_from_slice(&put);
    let header = usize::try_from(FRAME_HEADER_BYTES).expect("header");
    let bootstrap_commit = bootstrap
        .len()
        .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
        .expect("bootstrap commit");
    let mut bootstrap_body = bootstrap.clone();
    bootstrap_body[header] ^= 0x80;
    bootstrap_body[bootstrap_commit..].fill(0);
    let put_commit = put
        .len()
        .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
        .expect("Put commit");
    let put_checksum = put_commit.checked_sub(Hash::LENGTH).expect("Put checksum");
    let mut put_body = put.clone();
    put_body[header] ^= 0x80;
    put_body[put_commit..].fill(0);
    let mut full_put_body = bootstrap.clone();
    full_put_body.extend_from_slice(&put_body);
    let mut put_checksum_tear = put.clone();
    put_checksum_tear[put_checksum] ^= 0x80;
    put_checksum_tear[put_commit..].fill(0);
    let mut full_put_checksum = bootstrap.clone();
    full_put_checksum.extend_from_slice(&put_checksum_tear);
    let mut put_commit_tear = put;
    put_commit_tear[put_commit..].fill(0);
    let mut full_put_commit = bootstrap;
    full_put_commit.extend_from_slice(&put_commit_tear);
    let cases = [
        ("bootstrap-header", vec![0xA5_u8; header]),
        ("bootstrap-body", bootstrap_body),
        ("put-body", full_put_body),
        ("put-checksum", full_put_checksum),
        ("put-commit", full_put_commit),
    ];
    for (case, temporary_bytes) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("compact-staged-{case}.norito"));
        let temporary = path.with_extension("tmp");
        fs::write(&path, &canonical).expect("write canonical history");
        fs::write(&temporary, temporary_bytes).expect("write staged compaction tear");
        let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true)
            .unwrap_or_else(|error| panic!("reconcile staged {case} tear: {error}"));
        assert_eq!(
            journal.replay().expect("replay canonical"),
            vec![expected_record.clone()],
            "case={case}"
        );
        drop(journal);
        assert_eq!(
            fs::read(&path).expect("retain canonical bytes"),
            canonical,
            "case={case}"
        );
        assert!(!temporary.exists(), "case={case}");
    }
}
#[test]
fn committed_corrupt_compaction_temp_is_retained_and_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("compact-unexpected-v4.norito");
    let canonical_record = record("compact-expected");
    let mut canonical = raw_bootstrap_frame();
    canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(canonical_record)));
    let mut unexpected = canonical.clone();
    let put_payload_offset = raw_bootstrap_frame()
        .len()
        .checked_add(usize::try_from(FRAME_HEADER_BYTES).expect("header"))
        .expect("Put payload offset");
    unexpected[put_payload_offset] ^= 0x80;
    fs::write(&path, &canonical).expect("write canonical");
    fs::write(path.with_extension("tmp"), &unexpected).expect("write unexpected V4 temp");
    let error = QueuePlanJournal::open_with_limits(&path, limits(1), true)
        .err()
        .expect("unrelated canonical V4 temp must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("committed compaction frame differs from deterministic output"),
        "unexpected recovery error: {error}"
    );
    assert_eq!(fs::read(&path).expect("retain canonical"), canonical);
    assert_eq!(
        fs::read(path.with_extension("tmp")).expect("retain unexpected temp"),
        unexpected
    );
}
#[test]
fn orphaned_compaction_prefixes_cannot_recreate_a_missing_canonical_path() {
    let first = record("compact-orphaned-first");
    let second = record("compact-orphaned-second");
    let bootstrap = raw_bootstrap_frame();
    let first_put = raw_frame(&QueuePlanJournalFrameV4::Put(first));
    let second_put = raw_frame(&QueuePlanJournalFrameV4::Put(second));
    let mut first_of_two = bootstrap.clone();
    first_of_two.extend_from_slice(&first_put);
    let mut apparently_complete = first_of_two.clone();
    apparently_complete.extend_from_slice(&second_put);
    let partial_bootstrap = bootstrap[..bootstrap.len() - 1].to_vec();
    for (case, orphaned) in [
        ("partial-bootstrap", partial_bootstrap),
        ("bootstrap-only", bootstrap),
        ("first-of-two", first_of_two),
        ("apparently-complete", apparently_complete),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("compact-orphaned-{case}.norito"));
        fs::write(path.with_extension("tmp"), &orphaned).expect("write orphaned compaction prefix");
        let error = QueuePlanJournal::open_with_limits(&path, limits(2), true)
            .err()
            .expect("orphaned replacement cannot prove completeness");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case={case}");
        assert!(!path.exists(), "case={case}");
        assert_eq!(
            fs::read(path.with_extension("tmp")).expect("retain orphaned evidence"),
            orphaned,
            "case={case}"
        );
    }
}
#[cfg(unix)]
#[test]
fn compaction_recovery_rejects_temp_path_identity_swap_without_unlinking_replacement() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("compact-temp-swap.norito");
    let temporary = path.with_extension("tmp");
    let displaced = path.with_extension("tmp.displaced");
    let expected = record("compact-temp-swap");
    let mut journal = open(&path).expect("create canonical");
    journal
        .put_deferred_flush(expected)
        .expect("append canonical owner");
    journal
        .sync_all_with_parent()
        .expect("sync canonical owner");
    drop(journal);
    fs::write(&temporary, raw_bootstrap_frame()).expect("write recognized temp prefix");
    let pending = open_pending_compaction_temp(&temporary, limits(1))
        .expect("open pending temp")
        .expect("pending temp exists");
    fs::rename(&temporary, &displaced).expect("displace verified temp pathname");
    let replacement = b"must-not-be-unlinked".to_vec();
    fs::write(&temporary, &replacement).expect("install distinct temp pathname");
    let error = reconcile_pending_compaction_temp(&path, limits(1), pending)
        .expect_err("temp identity swap must fail before unlink");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        fs::read(&temporary).expect("retain replacement pathname"),
        replacement
    );
    assert_eq!(
        fs::read(&displaced).expect("retain originally verified temp"),
        raw_bootstrap_frame()
    );
}
#[test]
fn compaction_rename_then_parent_failure_recovers_replacement_on_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("compact-post-rename.norito");
    let compact_limits = QueuePlanJournalLimits::new(
        u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
        TEST_MAX_BYTES,
        TEST_MAX_BYTES,
        64,
    );
    let first = record("compact-post-rename-first");
    let removed = record("compact-post-rename-removed");
    let mut journal =
        QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
    journal
        .put_deferred_flush(first.clone())
        .expect("append retained record");
    journal
        .put_deferred_flush(removed.clone())
        .expect("append removed record");
    journal
        .remove_many_deferred_flush([(
            removed.entrypoint_hash,
            removed.plan_digest(),
            removed.claim_digest().expect("hash removed claim"),
        )])
        .expect("append tombstone");
    journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterRename);
    journal
        .compact_if_needed()
        .expect_err("post-rename parent failure must propagate");
    assert!(journal.is_poisoned());
    assert!(
        !path.with_extension("tmp").exists(),
        "rename must consume the replacement before the injected parent failure"
    );
    drop(journal);
    assert_eq!(
        QueuePlanJournal::open_with_limits(&path, compact_limits, true)
            .expect("restart validates renamed replacement")
            .replay()
            .expect("replay replacement"),
        vec![first]
    );
}
#[cfg(unix)]
#[test]
fn cached_append_handle_rejects_atomic_path_replacement_without_split_brain() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bound.norito");
    let displaced = dir.path().join("bound.displaced");
    let original = record("bound-original");
    let stale_append = record("bound-stale-append");
    let fresh_append = record("bound-fresh-append");
    let mut stale = open(&path).expect("open original journal");
    stale
        .replace_strict_durable(original.clone())
        .expect("seed original journal");
    fs::rename(&path, &displaced).expect("atomically displace journal pathname");
    fs::write(&path, []).expect("install distinct journal pathname");
    let displaced_before = fs::read(&displaced).expect("read displaced journal");
    let mut fresh = open(&path).expect("open replacement pathname concurrently");
    let error = stale
        .replace_strict_durable(stale_append)
        .expect_err("stale append handle must reject replaced pathname");
    assert!(error.is_indeterminate());
    assert!(error.journal_faulted());
    assert!(stale.is_poisoned());
    assert_eq!(
        fs::read(&displaced).expect("read displaced journal after rejection"),
        displaced_before,
        "the stale inode must receive no acknowledged append"
    );
    assert!(
        fresh.replay().expect("replay fresh journal").is_empty(),
        "the newly bound journal must not inherit stale-inode bytes"
    );
    fresh
        .replace_strict_durable(fresh_append.clone())
        .expect("fresh bound handle remains writable");
    assert_eq!(
        fresh.replay().expect("replay fresh append"),
        vec![fresh_append]
    );
    drop(stale);
    assert_eq!(
        open(&displaced)
            .expect("open displaced journal directly")
            .replay()
            .expect("replay displaced original"),
        vec![original]
    );
}
#[test]
fn second_same_inode_handle_rejects_unobserved_length_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("two-handles.norito");
    let first_record = record("two-handles-first");
    let mut first = open(&path).expect("open first handle");
    let mut stale_second = open(&path).expect("open second handle at same length");
    first
        .replace_strict_durable(first_record.clone())
        .expect("first handle appends");
    let error = stale_second
        .replace_strict_durable(record("two-handles-rejected"))
        .expect_err("second handle must not append across an unobserved length change");
    assert!(error.is_indeterminate());
    assert!(stale_second.is_poisoned());
    assert_eq!(
        first.replay().expect("replay first handle"),
        vec![first_record]
    );
}
#[cfg(unix)]
#[test]
fn cached_append_handle_rejects_post_open_hardlink_count_drift() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("hardlink-drift.norito");
    let alias = dir.path().join("hardlink-drift.alias");
    let original = record("hardlink-drift-original");
    let mut journal = open(&path).expect("open journal");
    journal
        .replace_strict_durable(original.clone())
        .expect("seed journal");
    let original_bytes = fs::read(&path).expect("read original bytes");
    fs::hard_link(&path, &alias).expect("add second filesystem link");
    let error = journal
        .replace_strict_durable(record("hardlink-drift-rejected"))
        .expect_err("link-count drift must fail closed before append");
    assert!(error.is_indeterminate());
    assert!(journal.is_poisoned());
    assert_eq!(
        fs::read(&path).expect("read rejected journal"),
        original_bytes
    );
    drop(journal);
    fs::remove_file(&alias).expect("remove adversarial hardlink");
    assert_eq!(
        open(&path)
            .expect("reopen single-link journal")
            .replay()
            .expect("replay original"),
        vec![original]
    );
}
#[cfg(unix)]
#[test]
fn cached_parent_handle_rejects_directory_replacement_before_sync() {
    let dir = tempfile::tempdir().expect("tempdir");
    let live_parent = dir.path().join("live");
    let displaced_parent = dir.path().join("live.displaced");
    fs::create_dir(&live_parent).expect("create live parent");
    let path = live_parent.join("queue.norito");
    let original = record("parent-original");
    let mut journal = open(&path).expect("open parent-bound journal");
    journal
        .replace_strict_durable(original.clone())
        .expect("seed parent-bound journal");
    fs::rename(&live_parent, &displaced_parent).expect("displace parent directory");
    fs::create_dir(&live_parent).expect("install distinct parent directory");
    fs::write(&path, []).expect("install distinct journal in replacement parent");
    let error = journal
        .sync_data_verified()
        .expect_err("cached parent identity drift must reject synchronization");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(journal.is_poisoned());
    drop(journal);
    assert_eq!(
        open(&displaced_parent.join("queue.norito"))
            .expect("open original journal through displaced parent")
            .replay()
            .expect("replay original parent-bound journal"),
        vec![original]
    );
    assert!(
        open(&path)
            .expect("open replacement-parent journal")
            .replay()
            .expect("replay replacement-parent journal")
            .is_empty()
    );
}
#[cfg(unix)]
#[test]
fn prepared_replay_rejects_path_replacement_before_streaming() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replay-bound.norito");
    let displaced = dir.path().join("replay-bound.displaced");
    let expected = record("replay-bound");
    let mut journal = open(&path).expect("open journal");
    journal
        .replace_strict_durable(expected)
        .expect("seed replay snapshot");
    let replay = journal.prepare_replay().expect("prepare bound replay");
    fs::rename(&path, &displaced).expect("displace replay pathname");
    fs::write(&path, []).expect("install replacement replay pathname");
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("prepared replay must reject a different path identity");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        callbacks, 0,
        "path replacement must fail before any callback",
    );
}
#[test]
fn prepared_replay_rejects_snapshot_length_extension_before_streaming() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replay-length-bound.norito");
    let expected = record("replay-length-bound");
    let appended = record("replay-length-extension");
    let mut journal = open(&path).expect("open journal");
    journal
        .replace_strict_durable(expected)
        .expect("seed replay snapshot");
    let replay = journal.prepare_replay().expect("prepare bound replay");
    let mut concurrent = OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open concurrent append handle");
    concurrent
        .write_all(&raw_frame(&QueuePlanJournalFrameV4::Put(appended)))
        .expect("extend replay snapshot");
    concurrent.sync_all().expect("publish extension");
    let mut callbacks = 0_usize;
    let error = replay
        .for_each_record(|_| {
            callbacks = callbacks.saturating_add(1);
            Ok(())
        })
        .expect_err("prepared replay must reject a changed snapshot length");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(callbacks, 0, "length drift must fail before streaming");
}
#[test]
fn nested_parent_creation_is_restart_idempotent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join("new")
        .join("nested")
        .join("journal")
        .join("queue.norito");
    let expected = record("nested-parent-durability");
    let mut journal = open(&path).expect("create nested durable journal parent");
    journal
        .replace_strict_durable(expected.clone())
        .expect("persist nested-parent owner");
    drop(journal);
    assert!(path.parent().expect("journal parent").is_dir());
    assert_eq!(
        open(&path)
            .expect("restart through existing nested parent")
            .replay()
            .expect("replay nested-parent owner"),
        vec![expected]
    );
}
#[cfg(unix)]
#[test]
fn symlinked_or_hardlinked_journal_and_untrusted_compaction_temp_are_rejected() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target.norito");
    fs::write(&target, []).expect("target");
    let linked = dir.path().join("linked.norito");
    symlink(&target, &linked).expect("symlink");
    assert!(open(&linked).is_err());
    let real_parent = dir.path().join("real-parent");
    fs::create_dir(&real_parent).expect("create real parent");
    let indirect_parent = dir.path().join("indirect-parent");
    symlink(&real_parent, &indirect_parent).expect("symlink parent component");
    let indirect_path = indirect_parent.join("nested").join("queue.norito");
    assert_eq!(
        open(&indirect_path)
            .err()
            .expect("indirect parent component must fail")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        !real_parent.join("nested").exists(),
        "parent-chain rejection must precede directory creation"
    );
    let hardlinked = dir.path().join("hardlinked.norito");
    fs::hard_link(&target, &hardlinked).expect("hardlink");
    let hardlink_error = open(&hardlinked).err().expect("hardlink must fail closed");
    assert_eq!(hardlink_error.kind(), io::ErrorKind::InvalidData);
    assert!(
        hardlink_error
            .to_string()
            .contains("exactly one filesystem link")
    );
    let path = dir.path().join("stale-temp.norito");
    fs::write(&path, []).expect("journal");
    fs::write(path.with_extension("tmp"), b"stale").expect("temp");
    assert_eq!(
        open(&path).err().expect("stale temp").kind(),
        io::ErrorKind::InvalidData
    );
    let symlink_temp_path = dir.path().join("symlink-temp.norito");
    let symlink_temp = symlink_temp_path.with_extension("tmp");
    symlink(&target, &symlink_temp).expect("temp symlink");
    assert_eq!(
        open(&symlink_temp_path).err().expect("symlink temp").kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        !symlink_temp_path.exists(),
        "temp rejection must occur before creating a new journal"
    );
    let hardlink_temp_path = dir.path().join("hardlink-temp.norito");
    let hardlink_temp = hardlink_temp_path.with_extension("tmp");
    fs::hard_link(&target, &hardlink_temp).expect("hardlinked temp");
    assert_eq!(
        open(&hardlink_temp_path)
            .err()
            .expect("hardlinked temp")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        !hardlink_temp_path.exists(),
        "hardlink temp rejection must occur before creating a new journal"
    );
    let oversized_temp_path = dir.path().join("oversized-temp.norito");
    let oversized_temp = oversized_temp_path.with_extension("tmp");
    let oversized = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&oversized_temp)
        .expect("create oversized temp");
    oversized
        .set_len(TEST_MAX_BYTES + 1)
        .expect("extend oversized temp");
    drop(oversized);
    assert_eq!(
        open(&oversized_temp_path)
            .err()
            .expect("oversized temp")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert!(
        !oversized_temp_path.exists(),
        "oversized temp rejection must occur before creating a new journal"
    );
}
