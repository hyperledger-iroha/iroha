// Pipeline-status cache admission and pruning regressions.
#[test]
fn pipeline_status_merge_prefers_committed_success_over_cached_rejection() {
    let now = Instant::now();
    let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
    let mut entry = PipelineStatusEntry::at_time(
        PipelineStatusKind::Rejected,
        None,
        Some(pipeline_rejection_summary(&rejection)),
        now,
    );
    entry.merge_from_event(PipelineStatusEntry::at_time(
        PipelineStatusKind::Committed,
        NonZeroU64::new(7),
        None,
        now + Duration::from_secs(1),
    ));
    assert_eq!(entry.kind, PipelineStatusKind::Committed);
    assert_eq!(entry.block_height, NonZeroU64::new(7));
    assert!(entry.rejection.is_none());
    entry.merge_from_event(PipelineStatusEntry::at_time(
        PipelineStatusKind::Applied,
        NonZeroU64::new(7),
        None,
        now + Duration::from_secs(2),
    ));
    assert_eq!(entry.kind, PipelineStatusKind::Applied);
    assert_eq!(entry.block_height, NonZeroU64::new(7));
    assert!(entry.rejection.is_none());
}
#[test]
fn pipeline_status_cache_records_transaction_event() {
    let cache = PipelineStatusCache::new();
    let (block, _) = make_signed_block(1, None);
    let tx_hash = block.external_transactions().next().expect("tx").hash();
    let height = NonZeroU64::new(2).expect("height");
    let event = TransactionEvent {
        hash: tx_hash,
        block_height: Some(height),
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(1),
        status: TransactionStatus::Approved,
    };
    cache.record_transaction_event(&event);
    let stored = cache.lookup(&tx_hash).expect("entry");
    assert_eq!(stored.kind, PipelineStatusKind::Approved);
    assert_eq!(stored.block_height, Some(height));
    assert!(stored.rejection.is_none());
}
#[tokio::test]
async fn pipeline_status_cache_records_block_event() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    let tx = block.external_transactions().next().expect("tx");
    let tx_hash = tx.hash();
    store_block(&app, block);
    let event = BlockEvent {
        header,
        status: BlockStatus::Applied,
    };
    app.pipeline_status_cache
        .record_block_event(&event, &app.kura);
    let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
    assert_eq!(stored.kind, PipelineStatusKind::Applied);
    let height = NonZeroU64::new(1).expect("height");
    assert_eq!(stored.block_height, Some(height));
}
#[tokio::test]
async fn pipeline_status_cache_refreshes_pending_block() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    let tx_hash = block.external_transactions().next().expect("tx").hash();
    let event = BlockEvent {
        header,
        status: BlockStatus::Committed,
    };
    app.pipeline_status_cache
        .record_block_event(&event, &app.kura);
    assert!(app.pipeline_status_cache.lookup(&tx_hash).is_none());
    store_block(&app, block);
    app.pipeline_status_cache.refresh_pending_blocks(&app.kura);
    let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
    assert_eq!(stored.kind, PipelineStatusKind::Committed);
}
#[test]
fn pipeline_status_cache_prunes_stale_entries() {
    let cache = PipelineStatusCache::with_limits(10, Duration::from_secs(1));
    let (block, _) = make_signed_block(1, None);
    let tx_hash = block.external_transactions().next().expect("tx").hash();
    let now = Instant::now();
    let stale = now
        .checked_sub(Duration::from_secs(5))
        .expect("time subtraction");
    cache.record_entry(
        tx_hash,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
    );
    cache.prune(now);
    assert!(cache.lookup(&tx_hash).is_none());
}
#[test]
fn pipeline_status_cache_eviction_respects_capacity() {
    let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(60));
    let (block_a, _) = make_signed_block(1, None);
    let (block_b, _) = make_signed_block(2, None);
    let hash_a = block_a.external_transactions().next().expect("tx").hash();
    let hash_b = block_b.external_transactions().next().expect("tx").hash();
    let now = Instant::now();
    let stale = now
        .checked_sub(Duration::from_secs(5))
        .expect("time subtraction");
    cache.record_entry(
        hash_a,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
    );
    cache.record_entry(
        hash_b,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
    );
    cache.prune(now);
    assert!(cache.lookup(&hash_a).is_none());
    assert!(cache.lookup(&hash_b).is_some());
}
#[test]
fn pipeline_status_cache_live_counts_track_entries_and_pending_blocks() {
    let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(60));
    let (block_a, _) = make_signed_block(1, None);
    let (block_b, _) = make_signed_block(2, None);
    let hash_a = block_a.external_transactions().next().expect("tx").hash();
    let hash_b = block_b.external_transactions().next().expect("tx").hash();
    let height_a = NonZeroU64::new(1).expect("height");
    let now = Instant::now();
    cache.record_entry(
        hash_a,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
    );
    cache.record_entry(
        hash_a,
        PipelineStatusEntry::at_time(PipelineStatusKind::Approved, None, None, now),
    );
    assert_eq!(cache.entry_count.load(Ordering::Relaxed), 1);
    assert_eq!(cache.entry_order.lock().len(), 1);
    cache.record_entry(
        hash_b,
        PipelineStatusEntry::at_time(
            PipelineStatusKind::Queued,
            None,
            None,
            now + Duration::from_secs(1),
        ),
    );
    cache.prune(now + Duration::from_secs(1));
    assert_eq!(
        cache.entry_count.load(Ordering::Relaxed),
        cache.entries.len()
    );
    assert!(cache.lookup(&hash_a).is_none());
    assert!(cache.lookup(&hash_b).is_some());
    cache.record_pending_block(
        height_a,
        PendingBlockStatus {
            kind: PipelineStatusKind::Committed,
            block_hash: block_a.header().hash(),
            observed_at: now,
        },
    );
    cache.record_pending_block(
        height_a,
        PendingBlockStatus {
            kind: PipelineStatusKind::Applied,
            block_hash: block_b.header().hash(),
            observed_at: now + Duration::from_secs(1),
        },
    );
    assert_eq!(cache.pending_count.load(Ordering::Relaxed), 1);
    assert_eq!(cache.pending_order.lock().len(), 1);
    assert!(cache.remove_pending_by_height(&height_a));
    assert_eq!(cache.pending_count.load(Ordering::Relaxed), 0);
}
#[test]
fn pipeline_status_cache_updates_do_not_accumulate_markers_or_extend_retention() {
    let cache = PipelineStatusCache::with_limits(10, Duration::from_secs(1));
    let (block, _) = make_signed_block(1, None);
    let tx_hash = block.external_transactions().next().expect("tx").hash();
    let now = Instant::now();
    let stale = now
        .checked_sub(Duration::from_secs(5))
        .expect("time subtraction");
    cache.record_entry(
        tx_hash,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
    );
    cache.record_entry(
        tx_hash,
        PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
    );
    assert_eq!(cache.entry_order.lock().len(), 1);
    cache.prune(now);
    assert!(cache.lookup(&tx_hash).is_none());
}
#[test]
fn pipeline_status_cache_pending_blocks_prune_by_ttl_and_capacity() {
    let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(1));
    let (block_a, _) = make_signed_block(1, None);
    let (block_b, _) = make_signed_block(2, None);
    let height_a = NonZeroU64::new(1).expect("height");
    let height_b = NonZeroU64::new(2).expect("height");
    let now = Instant::now();
    let stale = now
        .checked_sub(Duration::from_secs(5))
        .expect("time subtraction");
    cache.record_pending_block(
        height_a,
        PendingBlockStatus {
            kind: PipelineStatusKind::Committed,
            block_hash: block_a.header().hash(),
            observed_at: stale,
        },
    );
    cache.record_pending_block(
        height_b,
        PendingBlockStatus {
            kind: PipelineStatusKind::Applied,
            block_hash: block_b.header().hash(),
            observed_at: now,
        },
    );
    cache.prune(now);
    assert!(cache.pending_blocks.get(&height_a).is_none());
    assert!(cache.pending_blocks.get(&height_b).is_some());
}
