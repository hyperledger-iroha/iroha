#[test]
fn sealed_checkpoint_qualification_timeout_is_typed_and_bounded() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x61));
    runtime.block_operation(TestCheckpointOperation::Qualification);
    let started = Instant::now();
    assert!(matches!(
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)),
        Err(ProviderIngestOutboxError::CheckpointProviderTimeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        runtime.operation_calls(TestCheckpointOperation::Qualification),
        1
    );
    let second_started = Instant::now();
    assert!(matches!(
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)),
        Err(ProviderIngestOutboxError::CheckpointBusy)
    ));
    assert!(second_started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        runtime.operation_calls(TestCheckpointOperation::Qualification),
        1,
        "a hung provider boundary must retain the writer lease and reject another worker"
    );
    runtime.release_blocked_operation();
    let reopened = reopen_sealed_after_worker_release(&directory, runtime)
        .expect("reopen after the timed-out worker exits");
    assert_eq!(
        reopened.finalized_cursor_high_water().expect("sealed head"),
        None
    );
}
#[test]
fn sealed_checkpoint_load_timeout_is_typed_and_bounded() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x62));
    runtime.block_operation(TestCheckpointOperation::LoadLatest);
    let started = Instant::now();
    assert!(matches!(
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)),
        Err(ProviderIngestOutboxError::CheckpointProviderTimeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        runtime.operation_calls(TestCheckpointOperation::LoadLatest),
        1
    );
    assert!(runtime.latest().is_none());
    runtime.release_blocked_operation();
    let reopened = reopen_sealed_after_worker_release(&directory, runtime)
        .expect("reopen after timed-out load");
    assert_eq!(
        reopened.finalized_cursor_high_water().expect("sealed head"),
        None
    );
}
#[test]
fn sealed_checkpoint_cas_timeout_does_not_block_shutdown_and_reopens() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x63));
    runtime.block_operation(TestCheckpointOperation::CompareAndSwap);
    let started = Instant::now();
    assert!(matches!(
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)),
        Err(ProviderIngestOutboxError::CheckpointProviderTimeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        runtime.operation_calls(TestCheckpointOperation::CompareAndSwap),
        1
    );
    runtime.release_blocked_operation();
    wait_for_checkpoint_sequence(&runtime, 1);
    let reopened = reopen_sealed_after_worker_release(&directory, runtime)
        .expect("reopen after timed-out CAS");
    assert_eq!(
        reopened.finalized_cursor_high_water().expect("sealed head"),
        None
    );
}
#[test]
fn sealed_checkpoint_readback_timeout_is_sticky_and_recoverable_on_reopen() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x64));
    let outbox =
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    runtime.block_load_after_next_cas();
    assert_eq!(
        outbox.observe_finalized_snapshot(cursor(11), finalized_block_time_ms(cursor(11))),
        Err(ProviderIngestOutboxError::CheckpointProviderTimeout)
    );
    assert!(
        outbox
            .state
            .lock()
            .expect("outbox state")
            .durability_failure
            .is_none(),
        "the sticky worker timeout must not be replaced by durability poison"
    );
    let load_calls_after_timeout = runtime.operation_calls(TestCheckpointOperation::LoadLatest);
    assert_eq!(
        outbox.aggregate_counts(),
        Err(ProviderIngestOutboxError::CheckpointProviderTimeout)
    );
    assert_eq!(
        runtime.operation_calls(TestCheckpointOperation::LoadLatest),
        load_calls_after_timeout,
        "a timed-out worker must reject later requests without spawning or queuing work"
    );
    let shutdown_started = Instant::now();
    drop(outbox);
    assert!(shutdown_started.elapsed() < Duration::from_secs(5));
    runtime.release_blocked_operation();
    wait_for_checkpoint_sequence(&runtime, 2);
    let reopened = reopen_sealed_after_worker_release(&directory, runtime)
        .expect("reopen from sealed successor");
    assert_eq!(
        reopened.finalized_cursor_high_water().expect("sealed head"),
        Some(cursor(11))
    );
}
#[test]
fn sealed_checkpoint_commit_then_worker_panic_is_ambiguous_and_recoverable() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x66));
    let outbox =
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    runtime.set_next_cas_behavior(TestCheckpointCasBehavior::CommitThenPanic);
    assert_eq!(
        outbox.observe_finalized_snapshot(cursor(12), finalized_block_time_ms(cursor(12))),
        Err(ProviderIngestOutboxError::CheckpointAuthorityAmbiguous)
    );
    assert_eq!(
        runtime
            .latest()
            .expect("committed authoritative successor")
            .checkpoint_sequence,
        2
    );
    assert!(
        outbox
            .state
            .lock()
            .expect("outbox state")
            .durability_failure
            .is_some()
    );
    drop(outbox);
    let reopened = reopen_sealed_after_worker_release(&directory, runtime)
        .expect("reopen after committed CAS response loss");
    assert_eq!(
        reopened.finalized_cursor_high_water().expect("sealed head"),
        Some(cursor(12))
    );
}
#[test]
fn bounded_checkpoint_admission_serializes_healthy_concurrent_reads() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x65));
    let outbox =
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    let baseline_load_calls = runtime.operation_calls(TestCheckpointOperation::LoadLatest);
    runtime.block_operation(TestCheckpointOperation::LoadLatest);
    let first_outbox = outbox.clone();
    let first = std::thread::spawn(move || first_outbox.aggregate_counts());
    wait_for_operation_calls(
        &runtime,
        TestCheckpointOperation::LoadLatest,
        baseline_load_calls + 1,
    );
    let second_outbox = outbox.clone();
    let second = std::thread::spawn(move || second_outbox.aggregate_counts());
    let third_outbox = outbox.clone();
    let third = std::thread::spawn(move || third_outbox.aggregate_counts());
    std::thread::sleep(Duration::from_millis(10));
    runtime.release_blocked_operation();
    for operation in [first, second, third] {
        operation
            .join()
            .expect("checkpoint caller thread")
            .expect("healthy concurrent checkpoint read");
    }
    outbox
        .aggregate_counts()
        .expect("checkpoint worker remains qualified after bounded contention");
}
#[test]
fn authoritative_head_read_is_serialized_with_local_checkpoint_persistence() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x68));
    let outbox =
        open_sealed_with_deadline(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    let (reader_loaded, reader_loaded_rx) = std::sync::mpsc::sync_channel(0);
    let (release_reader, release_reader_rx) = std::sync::mpsc::sync_channel(0);
    let reader_outbox = outbox.clone();
    let reader = std::thread::spawn(move || {
        let state = reader_outbox.lock_state_after_authoritative_load(|| {
            reader_loaded.send(()).expect("signal authoritative read");
            release_reader_rx
                .recv()
                .expect("release authoritative reader");
        })?;
        Ok::<_, ProviderIngestOutboxError>(state.aggregate_counts)
    });
    reader_loaded_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("reader observed authoritative predecessor");
    let (writer_lock_attempted, writer_lock_attempted_rx) = std::sync::mpsc::sync_channel(0);
    let writer_outbox = outbox.clone();
    let writer = std::thread::spawn(move || match writer_outbox.state.try_lock() {
        Ok(mut state) => {
            writer_lock_attempted
                .send(true)
                .expect("signal early writer lock");
            let mut candidate = state.checkpoint.clone();
            candidate.finalized_cursor_high_water = Some(cursor(13));
            candidate.finalized_block_time_ms_high_water =
                Some(finalized_block_time_ms(cursor(13)));
            writer_outbox.persist_candidate(&mut state, candidate)
        }
        Err(std::sync::TryLockError::WouldBlock) => {
            writer_lock_attempted
                .send(false)
                .expect("signal serialized writer lock");
            writer_outbox
                .observe_finalized_snapshot(cursor(13), finalized_block_time_ms(cursor(13)))
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            Err(ProviderIngestOutboxError::StateUnavailable)
        }
    });
    let writer_acquired_before_reader_release = writer_lock_attempted_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("writer attempted local state lock");
    let (reader_result, writer_result) = if writer_acquired_before_reader_release {
        let writer_result = writer.join().expect("writer thread");
        release_reader.send(()).expect("release stale reader");
        let reader_result = reader.join().expect("reader thread");
        (reader_result, writer_result)
    } else {
        release_reader.send(()).expect("release serialized reader");
        let reader_result = reader.join().expect("reader thread");
        let writer_result = writer.join().expect("writer thread");
        (reader_result, writer_result)
    };
    assert!(
        !writer_acquired_before_reader_release,
        "a local persist acquired state after an authoritative read but before comparison"
    );
    reader_result.expect("authoritative read remains consistent with local state");
    writer_result.expect("serialized local persistence advances the sealed head");
    assert_eq!(
        runtime
            .latest()
            .expect("authoritative successor")
            .checkpoint_sequence,
        2
    );
    assert_eq!(
        outbox
            .finalized_cursor_high_water()
            .expect("advanced finalized cursor"),
        Some(cursor(13))
    );
}
#[test]
fn expired_checkpoint_admission_is_busy_without_poisoning_the_worker() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x67));
    let outbox = open_sealed_with_deadline(&directory, runtime).expect("sealed outbox");
    let worker = &outbox
        .checkpoint_authority
        .as_ref()
        .expect("checkpoint authority")
        .worker;
    let expired = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .expect("past instant");
    assert!(matches!(
        worker.acquire_call(expired),
        Err(ProviderIngestOutboxError::CheckpointProviderBusy)
    ));
    assert!(!worker.timed_out.load(Ordering::Acquire));
    outbox
        .aggregate_counts()
        .expect("expired admission must not poison later checkpoint reads");
}
#[test]
fn sealed_checkpoint_restart_uses_external_authority_and_exact_cache() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x71));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("open sealed outbox");
    observe_finalized(&outbox, cursor(3));
    let sealed = runtime.latest().expect("sealed checkpoint");
    assert_eq!(sealed.checkpoint_sequence, 2);
    assert_eq!(
        fs::read(checkpoint_path(&directory)).expect("read local cache"),
        sealed
            .to_canonical_bytes(policy().checkpoint_max_bytes)
            .expect("canonical sealed record")
    );
    drop(outbox);
    let reopened = open_sealed(&directory, runtime).expect("restart from sealed authority");
    assert_eq!(
        reopened
            .finalized_cursor_high_water()
            .expect("read finalized cursor"),
        Some(cursor(3))
    );
}
#[test]
fn sealed_checkpoint_restart_repairs_only_an_exact_immediate_predecessor_cache() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x79));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    let predecessor_cache = fs::read(checkpoint_path(&directory)).expect("read predecessor cache");
    observe_finalized(&outbox, cursor(9));
    let sealed = runtime.latest().expect("sealed successor");
    fs::write(checkpoint_path(&directory), predecessor_cache)
        .expect("simulate crash before local cache replacement");
    drop(outbox);
    let reopened = open_sealed(&directory, runtime).expect("recover exact successor");
    assert_eq!(
        reopened
            .finalized_cursor_high_water()
            .expect("recovered finalized cursor"),
        Some(cursor(9))
    );
    assert_eq!(
        fs::read(checkpoint_path(&directory)).expect("read repaired cache"),
        sealed
            .to_canonical_bytes(policy().checkpoint_max_bytes)
            .expect("canonical successor")
    );
}
#[test]
fn sealed_checkpoint_two_writer_conflict_fails_closed() {
    let first_directory = tempdir().expect("first checkpoint directory");
    let second_directory = tempdir().expect("second checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x72));
    let first = open_sealed(&first_directory, Arc::clone(&runtime)).expect("first sealed writer");
    let second =
        open_sealed(&second_directory, Arc::clone(&runtime)).expect("second sealed writer");
    observe_finalized(&first, cursor(4));
    assert_eq!(
        second.observe_finalized_snapshot(cursor(5), finalized_block_time_ms(cursor(5))),
        Err(ProviderIngestOutboxError::CheckpointFork)
    );
    assert_eq!(
        second.finalized_cursor_high_water(),
        Err(ProviderIngestOutboxError::CheckpointFork)
    );
}
#[test]
fn ambiguous_sealed_commit_succeeds_only_after_exact_authoritative_readback() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x73));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    runtime.set_next_cas_behavior(TestCheckpointCasBehavior::CommitAmbiguous);
    observe_finalized(&outbox, cursor(6));
    assert_eq!(
        runtime
            .latest()
            .expect("committed ambiguous record")
            .checkpoint_sequence,
        2
    );
}
#[test]
fn unchanged_predecessor_is_an_explicit_safe_retry_for_every_cas_outcome() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x74));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    runtime.set_next_cas_behavior(TestCheckpointCasBehavior::UnchangedAmbiguous);
    assert_eq!(
        outbox.observe_finalized_snapshot(cursor(7), finalized_block_time_ms(cursor(7))),
        Err(ProviderIngestOutboxError::CheckpointCasUnchanged)
    );
    runtime.set_next_cas_behavior(TestCheckpointCasBehavior::UnchangedOk);
    assert_eq!(
        outbox.observe_finalized_snapshot(cursor(7), finalized_block_time_ms(cursor(7))),
        Err(ProviderIngestOutboxError::CheckpointCasUnchanged)
    );
    assert_eq!(
        outbox
            .finalized_cursor_high_water()
            .expect("unchanged predecessor remains readable"),
        None
    );
    observe_finalized(&outbox, cursor(7));
}
#[test]
fn sealed_checkpoint_rollback_and_same_sequence_fork_fail_startup() {
    let directory = tempdir().expect("checkpoint directory");
    let runtime = Arc::new(TestCheckpointRuntime::new(0x75));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    let genesis = runtime.latest().expect("genesis record");
    observe_finalized(&outbox, cursor(8));
    let committed = runtime.latest().expect("successor record");
    drop(outbox);
    runtime.replace_latest(Some(genesis));
    assert!(matches!(
        open_sealed(&directory, Arc::clone(&runtime)),
        Err(ProviderIngestOutboxError::CheckpointRollback)
    ));
    let mut forked_checkpoint =
        decode_provider_ingest_checkpoint(&committed.checkpoint_bytes, policy())
            .expect("decode committed checkpoint");
    forked_checkpoint.next_sequence = forked_checkpoint
        .next_sequence
        .checked_add(1)
        .expect("advance fixture sequence");
    let forked = ProviderIngestSealedCheckpointRecordV1::new(
        committed.checkpoint_sequence,
        committed.predecessor_revision,
        committed.predecessor_checkpoint_digest,
        encode_provider_ingest_checkpoint(&forked_checkpoint, policy())
            .expect("encode forked checkpoint"),
    );
    runtime.replace_latest(Some(forked));
    assert!(matches!(
        open_sealed(&directory, runtime),
        Err(ProviderIngestOutboxError::CheckpointFork)
    ));
}
