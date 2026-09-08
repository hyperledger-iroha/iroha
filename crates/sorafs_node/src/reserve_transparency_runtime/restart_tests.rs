// Restart fixtures use private real directories; production rejects linked ancestors.

#[test]
fn restart_resumes_after_last_durable_event_without_duplication() {
    let temp = tempfile::tempdir().expect("temporary scanner root");
    let config = scanner_config(
        temp.path()
            .canonicalize()
            .expect("canonical temporary root")
            .join("scanner"),
    );
    let (query, projection, sink) = test_dependencies();
    let mut first = scanner(
        &config,
        Arc::clone(&query),
        Arc::clone(&projection),
        Arc::clone(&sink),
    );
    let first_outcome = first.tick().expect("first bounded page");
    assert_eq!(first_outcome.events, 1);
    assert!(!first_outcome.caught_up);
    drop(first);
    let mut restarted = scanner(
        &config,
        Arc::clone(&query),
        Arc::clone(&projection),
        Arc::clone(&sink),
    );
    let second_outcome = restarted.tick().expect("resume after checkpoint cursor");
    assert_eq!(second_outcome.events, 1);
    assert!(second_outcome.caught_up);
    drop(restarted);
    let mut caught_up = scanner(&config, query, projection, Arc::clone(&sink));
    let replay_outcome = caught_up.tick().expect("restart at caught-up cursor");
    assert_eq!(replay_outcome.events, 0);
    assert!(replay_outcome.caught_up);
    assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
    assert_eq!(sink.entries.lock().expect("test sink lock").len(), 2);
}
#[test]
fn durable_source_replay_before_cursor_write_is_idempotent() {
    let temp = tempfile::tempdir().expect("temporary scanner root");
    let config = scanner_config(
        temp.path()
            .canonicalize()
            .expect("canonical temporary root")
            .join("scanner"),
    );
    let (query, projection, sink) = test_dependencies();
    let replay_entry = reserve_finalized_event_source_entry(&query.events[0])
        .expect("derive pre-crash source entry");
    sink.record_source_entry(replay_entry)
        .expect("simulate durable source write before crash");
    let mut scanner = scanner(&config, query, projection, Arc::clone(&sink));
    let outcome = scanner.tick().expect("replay exact source after restart");
    assert_eq!(outcome.events, 1);
    assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
    assert_eq!(sink.entries.lock().expect("test sink lock").len(), 1);
}
#[test]
fn restart_fails_closed_when_persisted_anchor_left_committed_chain() {
    let temp = tempfile::tempdir().expect("temporary scanner root");
    let config = scanner_config(
        temp.path()
            .canonicalize()
            .expect("canonical temporary root")
            .join("scanner"),
    );
    let (query, projection, sink) = test_dependencies();
    let mut first = scanner(
        &config,
        Arc::clone(&query),
        Arc::clone(&projection),
        Arc::clone(&sink),
    );
    first.tick().expect("persist initial exact cursor");
    drop(first);
    projection.replace_hash(3, [0xF3; 32]);
    let mut restarted = scanner(&config, query, projection, sink);
    assert_eq!(
        restarted.tick(),
        Err(ReserveTransparencyScannerErrorV1::ForkOrReorg)
    );
}
