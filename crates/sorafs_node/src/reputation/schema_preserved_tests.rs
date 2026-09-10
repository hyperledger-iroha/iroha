// Existing tests kept in their original parent test namespace.
#[test]
fn exact_replay_is_idempotent_and_byte_identical() {
    let root = TempDir::new().expect("state root");
    let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
    let provider_id = provider(1);
    let batch = proof_only_batch(
        TARGET_HASH,
        vec![proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11)],
    );
    assert_eq!(
        service
            .ingest_finalized_batch(batch.clone())
            .expect("apply event"),
        ReputationIngestOutcomeV1::Applied { events: 1 }
    );
    let before = service
        .canonical_checkpoint_bytes()
        .expect("canonical checkpoint");
    assert_eq!(
        service
            .ingest_finalized_batch(batch)
            .expect("accept exact replay"),
        ReputationIngestOutcomeV1::ExactReplay
    );
    assert_eq!(
        service
            .canonical_checkpoint_bytes()
            .expect("canonical replay checkpoint"),
        before
    );
    assert_eq!(service.metrics().exact_replays, 1);
}
#[test]
fn unified_journal_replay_is_byte_identical_and_advances_all_semantic_sources() {
    let root = TempDir::new().expect("journal state root");
    let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
    let provider_id = provider(12);
    let batch = journal_only_batch(vec![journal_page(vec![
        journal_event(
            1,
            0,
            por_journal_entry(provider_id, 0x31, FINALIZED_AT_MS - 1_000),
        ),
        journal_event(
            2,
            1,
            token_journal_entry(provider_id, 0x32, FINALIZED_AT_MS - 500),
        ),
    ])]);
    assert_eq!(
        service
            .ingest_finalized_batch(batch.clone())
            .expect("apply journal"),
        ReputationIngestOutcomeV1::Applied { events: 2 }
    );
    let before = service
        .canonical_checkpoint_bytes()
        .expect("journal checkpoint");
    {
        let state = service.state.lock().expect("journal state");
        for source in [
            ReputationSourceV1::Por,
            ReputationSourceV1::Dispute,
            ReputationSourceV1::Token,
        ] {
            let progress = state.checkpoint.progress(source);
            assert_eq!(progress.last_event.expect("journal cursor").sequence, 2);
            assert_eq!(
                progress.observed_through,
                Some(ReputationFinalizedIdentityV1 {
                    height: TARGET_HEIGHT,
                    block_hash: TARGET_HASH,
                })
            );
        }
        let accumulator = state.checkpoint.providers.first().expect("provider");
        assert_eq!(accumulator.por_total, 1);
        assert_eq!(accumulator.por_successes, 1);
        assert_eq!(accumulator.token_observations, 1);
        assert_eq!(accumulator.token_violations, 0);
    }
    let cursors = service
        .committed_feed_cursors()
        .expect("public committed-feed cursors");
    assert_eq!(cursors.len(), ALL_COMMITTED_FEEDS.len());
    let journal_cursor = cursors
        .iter()
        .find(|cursor| cursor.feed == ReputationCommittedFeedV1::Journal)
        .expect("unified journal cursor");
    assert_eq!(journal_cursor.after.expect("journal after").sequence, 2);
    assert_eq!(
        journal_cursor.observed_through,
        Some(ReputationFinalizedIdentityV1 {
            height: TARGET_HEIGHT,
            block_hash: TARGET_HASH,
        })
    );
    assert_eq!(
        service
            .ingest_finalized_batch(batch)
            .expect("exact journal replay"),
        ReputationIngestOutcomeV1::ExactReplay
    );
    assert_eq!(
        service
            .canonical_checkpoint_bytes()
            .expect("replayed journal checkpoint"),
        before
    );
    assert_eq!(service.metrics().exact_replays, 2);
}
