// Production startup recovery retains the canonical journal while discarding
// only an exact authenticated interrupted-compaction prefix.

#[test]
fn startup_reconciles_empty_chunk_boundary_and_complete_compaction_prefixes() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("bounded-compaction-recovery.norito");
    let limits = LaneQueueReservationJournalLimits::new(64 * 1024, 64 * 1024, 128 * 1024, 32);
    let records: Vec<_> = (0..32).map(indexed_record).collect();
    {
        let (mut journal, _, _) =
            LaneQueueReservationJournal::open_with_limits(&path, limits).expect("create journal");
        journal.put_batch(records.clone()).expect("persist owners");
    }
    let canonical = fs::read(&path).expect("retain canonical bytes");
    let snapshot =
        canonical_snapshot(&records, &[], &[], &[], &[]).expect("canonical owner snapshot");
    let expected =
        encode_compacted_journal(snapshot.as_ref()).expect("encode interrupted compaction");
    assert!(expected.len() > 4097, "fixture must cross scratch boundary");
    let temp = path.with_extension("reservation-compact.tmp");
    for length in [0, 1, 4095, 4096, 4097, expected.len()] {
        fs::write(&temp, &expected[..length]).expect("write interrupted prefix");
        let (journal, replay, _) = LaneQueueReservationJournal::open_with_limits(&path, limits)
            .expect("recover exact prefix");
        assert_eq!(replay.records(), records);
        assert!(replay.committed().is_empty());
        assert!(replay.release_barriers().is_empty());
        assert!(replay.completed_releases().is_empty());
        assert!(!temp.exists(), "only authenticated temp is removed");
        assert_eq!(fs::read(&path).expect("canonical still present"), canonical);
        drop(journal);
    }
}

#[test]
fn startup_preserves_a_hardlinked_authentic_compaction_temp() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("hardlinked-compaction.norito");
    let limits = LaneQueueReservationJournalLimits::new(64 * 1024, 64 * 1024, 128 * 1024, 32);
    {
        let (journal, _, _) =
            LaneQueueReservationJournal::open_with_limits(&path, limits).expect("create journal");
        drop(journal);
    }
    let canonical = fs::read(&path).expect("retain canonical bytes");
    let expected = encode_compacted_journal(None).expect("empty compacted journal");
    let temp = path.with_extension("reservation-compact.tmp");
    let alias = directory.path().join("other-owner.tmp");
    fs::write(&temp, &expected).expect("write authentic bytes");
    fs::hard_link(&temp, &alias).expect("add another physical owner");
    let error = LaneQueueReservationJournal::open_with_limits(&path, limits)
        .err()
        .expect("an authentic prefix with another link must still reject");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("single-link"));
    assert_eq!(fs::read(&path).expect("canonical retained"), canonical);
    assert_eq!(fs::read(&temp).expect("temp retained"), expected);
    assert_eq!(fs::read(&alias).expect("alias retained"), expected);
}
