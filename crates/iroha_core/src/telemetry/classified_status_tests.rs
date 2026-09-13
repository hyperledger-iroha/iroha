// Native actor/State/Kura controls. The included parent owns the real unit fixture.
use iroha_torii_shared::status::BuildStatus;

async fn pause_actor(sut: &SystemUnderTest) -> oneshot::Sender<()> {
    let (entered, ready) = oneshot::channel();
    let (resume, wait) = oneshot::channel();
    sut.telemetry
        .actor
        .send(Message::TestBarrier {
            entered,
            resume: wait,
        })
        .await
        .unwrap_or_else(|_| panic!("live actor"));
    ready.await.expect("actor entered barrier");
    resume
}

fn request(sut: &SystemUnderTest) -> oneshot::Receiver<Result<OwnedStatus, StatusSnapshotError>> {
    let (reply, response) = oneshot::channel();
    sut.telemetry
        .actor
        .try_send(Message::Status {
            build: BuildStatus::default(),
            deadline: tokio::time::Instant::now() + METRICS_SYNC_TIMEOUT,
            reply,
        })
        .unwrap_or_else(|_| panic!("status mailbox admission"));
    response
}

fn replace_tip(sut: &SystemUnderTest, hash: HashOf<BlockHeader>) {
    let mut journal = sut.state.block_hashes.block_and_revert();
    journal.push_for_tests(hash);
    journal.commit_for_tests();
}

#[tokio::test(start_paused = true)]
async fn queued_status_captures_the_publication_after_its_admission_barrier() {
    let sut = SystemUnderTest::new();
    let resume = pause_actor(&sut).await;
    let pre_await_height = sut.state.committed_height();
    let response = request(&sut);
    let block = sut.commit_block(sut.create_block());
    let external = u64::try_from(block.as_ref().external_transactions().len()).unwrap();
    resume.send(()).unwrap();
    let (status, height) = response.await.unwrap().unwrap().into_parts();
    assert_eq!(pre_await_height, 0);
    assert_eq!(height, 1);
    assert_eq!(status.blocks, height);
    assert_eq!(status.blocks_non_empty, 1);
    assert_eq!(status.txs_approved + status.txs_rejected, external);
    assert!(status.nexus.is_some());
}

#[tokio::test(start_paused = true)]
async fn owned_status_bytes_remain_immutable_after_later_classification() {
    let sut = SystemUnderTest::new();
    sut.commit_block(sut.create_block());
    let (first, first_height) = sut
        .telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap()
        .into_parts();
    let original = norito::to_bytes(&first).unwrap();
    sut.mock_time_handle.advance(Duration::from_millis(10));
    sut.commit_block(sut.create_block());
    let (second, second_height) = sut
        .telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap()
        .into_parts();
    assert_eq!((first_height, second_height), (1, 2));
    assert_eq!((first.blocks, second.blocks), (1, 2));
    assert_eq!(norito::to_bytes(&first).unwrap(), original);
}

#[tokio::test(start_paused = true)]
async fn missing_applied_kura_block_cannot_publish_any_classified_counter() {
    let sut = SystemUnderTest::new();
    let mut journal = sut.state.block_hashes.block();
    journal.push_for_tests(HashOf::from_untyped_unchecked(Hash::new(
        b"missing status body",
    )));
    journal.commit_for_tests();
    let result = sut.telemetry.status_snapshot(&BuildStatus::default()).await;
    assert!(matches!(result, Err(StatusSnapshotError::MissingBlock)));
    assert_eq!(sut.telemetry.metrics.block_height.get(), 0);
    assert_eq!(sut.telemetry.metrics.block_height_non_empty.get(), 0);
    assert_eq!(
        sut.telemetry
            .metrics
            .txs
            .with_label_values(&["total"])
            .get(),
        0
    );
}

#[tokio::test(start_paused = true)]
async fn substituted_kura_sequence_retires_staged_counters_and_retries_exactly_once() {
    let sut = SystemUnderTest::new();
    let block = sut.commit_block(sut.create_block());
    let actual = block.as_ref().header().hash();
    let other = HashOf::from_untyped_unchecked(Hash::new(b"substituted status journal"));
    replace_tip(&sut, other);
    let result = sut.telemetry.status_snapshot(&BuildStatus::default()).await;
    assert!(matches!(result, Err(StatusSnapshotError::JournalMismatch)));
    assert_eq!(sut.telemetry.metrics.block_height.get(), 0);
    assert_eq!(
        sut.telemetry
            .metrics
            .txs
            .with_label_values(&["total"])
            .get(),
        0
    );
    assert_eq!(sut.telemetry.metrics.last_block_committed_at_ms.get(), 0);
    replace_tip(&sut, actual);
    let (status, height) = sut
        .telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap()
        .into_parts();
    assert_eq!(height, 1);
    assert_eq!(status.blocks_non_empty, 1);
    let total = sut
        .telemetry
        .metrics
        .txs
        .with_label_values(&["total"])
        .get();
    sut.telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap();
    assert_eq!(
        sut.telemetry
            .metrics
            .txs
            .with_label_values(&["total"])
            .get(),
        total
    );
    replace_tip(&sut, other);
    assert!(matches!(
        sut.telemetry.status_snapshot(&BuildStatus::default()).await,
        Err(StatusSnapshotError::CheckpointChanged)
    ));
    assert_eq!(sut.telemetry.metrics.block_height.get(), 1);
}

#[tokio::test(start_paused = true)]
async fn expired_active_status_finishes_its_finite_target_and_keeps_chunk_progress() {
    let sut = SystemUnderTest::new();
    for _ in 0..65 {
        sut.mock_time_handle.advance(Duration::from_millis(1));
        sut.commit_block(sut.create_block());
    }
    let (entered, ready) = oneshot::channel();
    let (resume, wait) = oneshot::channel();
    sut.telemetry
        .actor
        .send(Message::TestChunkBarrier {
            entered,
            resume: wait,
        })
        .await
        .unwrap_or_else(|_| panic!("live actor"));
    let response = request(&sut);
    assert_eq!(ready.await.unwrap(), 64);
    assert_eq!(sut.telemetry.metrics.block_height.get(), 64);
    // A later publication must not extend this active request's fixed target.
    sut.mock_time_handle.advance(Duration::from_millis(1));
    let extra = sut.commit_block(sut.create_block());
    let extra_count = u64::try_from(extra.as_ref().external_transactions().len()).unwrap();
    assert_eq!(sut.state.telemetry_status_target().unwrap().height, 66);
    tokio::time::advance(METRICS_SYNC_TIMEOUT).await;
    resume.send(()).unwrap();
    assert!(matches!(
        response.await.unwrap(),
        Err(StatusSnapshotError::DeadlineElapsed)
    ));
    assert_eq!(sut.telemetry.metrics.block_height.get(), 65);
    let before = sut
        .telemetry
        .metrics
        .txs
        .with_label_values(&["total"])
        .get();
    let (status, height) = sut
        .telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap()
        .into_parts();
    assert_eq!(height, 66);
    assert_eq!(status.blocks, 66);
    assert_eq!(
        sut.telemetry
            .metrics
            .txs
            .with_label_values(&["total"])
            .get(),
        before + extra_count
    );
    sut.telemetry
        .status_snapshot(&BuildStatus::default())
        .await
        .unwrap();
    assert_eq!(
        sut.telemetry
            .metrics
            .txs
            .with_label_values(&["total"])
            .get(),
        before + extra_count
    );
}

#[tokio::test(start_paused = true)]
async fn dropping_http_waiter_does_not_drop_active_actor_work() {
    let sut = SystemUnderTest::new();
    sut.commit_block(sut.create_block());
    let (entered, ready) = oneshot::channel();
    let (resume, wait) = oneshot::channel();
    sut.telemetry
        .actor
        .send(Message::TestChunkBarrier {
            entered,
            resume: wait,
        })
        .await
        .unwrap_or_else(|_| panic!("live actor"));
    let response = request(&sut);
    assert_eq!(ready.await.unwrap(), 1);
    drop(response);
    let (entered, mut next) = oneshot::channel();
    let (release_next, wait_next) = oneshot::channel();
    sut.telemetry
        .actor
        .send(Message::TestBarrier {
            entered,
            resume: wait_next,
        })
        .await
        .unwrap_or_else(|_| panic!("live actor"));
    assert!(matches!(
        next.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    resume.send(()).unwrap();
    next.await.unwrap();
    assert_eq!(sut.telemetry.metrics.block_height.get(), 1);
    release_next.send(()).unwrap();
    assert_eq!(
        sut.telemetry
            .status_snapshot(&BuildStatus::default())
            .await
            .unwrap()
            .into_parts()
            .1,
        1
    );
}

#[tokio::test(start_paused = true)]
async fn status_queue_wait_uses_original_500ms_deadline_and_does_not_start_expired_work() {
    let sut = SystemUnderTest::new();
    let resume = pause_actor(&sut).await;
    let start = tokio::time::Instant::now();
    let error = sut.telemetry.status_snapshot(&BuildStatus::default()).await;
    assert!(matches!(error, Err(StatusSnapshotError::DeadlineElapsed)));
    assert_eq!(tokio::time::Instant::now() - start, METRICS_SYNC_TIMEOUT);
    resume.send(()).unwrap();
    let resume = pause_actor(&sut).await;
    assert_eq!(sut.telemetry.metrics.block_height.get(), 0);
    resume.send(()).unwrap();
}

#[tokio::test(start_paused = true)]
async fn status_mailbox_full_closed_and_disabled_are_explicit() {
    let sut = SystemUnderTest::new();
    let resume = pause_actor(&sut).await;
    for _ in 0..CHANNEL_CAPACITY {
        sut.telemetry
            .actor
            .try_send(Message::Sync { reply: None })
            .unwrap_or_else(|_| panic!("exact bounded mailbox"));
    }
    assert!(matches!(
        sut.telemetry.status_snapshot(&BuildStatus::default()).await,
        Err(StatusSnapshotError::MailboxUnavailable)
    ));
    resume.send(()).unwrap();
    let closed = Telemetry::new(Arc::new(Metrics::default()), true);
    assert!(matches!(
        closed.status_snapshot(&BuildStatus::default()).await,
        Err(StatusSnapshotError::MailboxUnavailable)
    ));
    let disabled = Telemetry::new(Arc::new(Metrics::default()), false);
    assert!(matches!(
        disabled.status_snapshot(&BuildStatus::default()).await,
        Err(StatusSnapshotError::Disabled)
    ));
}

#[tokio::test(start_paused = true)]
async fn checked_metrics_sync_and_status_refuse_substituted_counter_height() {
    let sut = SystemUnderTest::new();
    sut.telemetry.metrics.block_height.inc();
    assert!(matches!(
        sut.telemetry.status_snapshot(&BuildStatus::default()).await,
        Err(StatusSnapshotError::CounterMismatch)
    ));
    assert!(sut.telemetry.metrics_fresh_checked().await.is_err());
}
