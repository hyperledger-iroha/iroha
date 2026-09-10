// Existing tests kept in their original parent test namespace.
#[test]
fn finalized_query_reconciliation_is_bounded_and_stops_on_empty_progress() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, _reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    let first_query = TestFinalizedQuery::new(vec![
        Some(page(vec![event(1, "storage:event:1", "10")])),
        Some(page(vec![event(2, "storage:event:2", "2")])),
    ]);
    let first_head = first_query.finalized_head().expect("first query head");
    assert!(matches!(
        service.reconcile_finalized_query(&first_query, 0, first_head),
        Err(HedgingBillingServiceError::InvalidQueryBound)
    ));
    assert!(matches!(
        service.reconcile_finalized_query(
            &first_query,
            HEDGING_BILLING_MAX_PAGES_PER_SCAN_V1 + 1,
            first_head,
        ),
        Err(HedgingBillingServiceError::InvalidQueryBound)
    ));
    assert_eq!(first_query.calls.load(Ordering::Relaxed), 0);
    let first = service
        .reconcile_finalized_query(&first_query, 1, first_head)
        .expect("bounded first scan");
    assert_eq!(
        first,
        HedgingBillingReconcileOutcomeV1 {
            pages_applied: 1,
            events_applied: 1,
            next_sequence: 2,
            finalized_cursor: Some(
                page(vec![event(1, "storage:event:1", "10")])
                    .journal_commitment
                    .finalized_cursor,
            ),
        }
    );
    assert_eq!(first_query.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        first_query
            .requested_max_events
            .lock()
            .expect("requested max-events state")
            .as_slice(),
        &[service_policy().max_events_per_page]
    );
    let finality_only_cursor = cursor(12, [0xB2; 32], PERIOD_END + 2);
    let first_commitment = page(vec![event(1, "storage:event:1", "10")]).journal_commitment;
    let finality_only_page = HedgingBillingFinalizedEventPageV1 {
        version: HEDGING_BILLING_FINALIZED_PAGE_VERSION_V1,
        network_id: test_network_id(b"hedging-billing-test-genesis"),
        start_sequence: 2,
        next_sequence: 2,
        journal_commitment: HedgingBillingJournalCommitmentV1 {
            version: HEDGING_BILLING_JOURNAL_COMMITMENT_VERSION_V1,
            network_id: test_network_id(b"hedging-billing-test-genesis"),
            finalized_cursor: finality_only_cursor,
            journal_next_sequence: 2,
            journal_root: first_commitment.journal_root,
        },
        append_proof: vec![0xA5],
        inclusion_proof: vec![0xB6],
        events: Vec::new(),
    };
    let empty_query = TestFinalizedQuery::new(vec![
        Some(finality_only_page.clone()),
        Some(finality_only_page),
    ]);
    let empty_head = empty_query.finalized_head().expect("empty query head");
    let empty = service
        .reconcile_finalized_query(&empty_query, 10, empty_head)
        .expect("finality-only scan");
    assert_eq!(
        empty,
        HedgingBillingReconcileOutcomeV1 {
            pages_applied: 1,
            events_applied: 0,
            next_sequence: 2,
            finalized_cursor: Some(finality_only_cursor),
        }
    );
    assert_eq!(
        empty_query.calls.load(Ordering::Relaxed),
        1,
        "a finality-only page must terminate the scan"
    );
    assert_eq!(
        empty_query
            .positions
            .lock()
            .expect("query-position state")
            .as_slice(),
        &[HedgingBillingQueryPositionV1 {
            next_sequence: 2,
            journal_commitment: Some(first_commitment),
        }]
    );
    assert_eq!(
        service
            .reconcile_finalized_query(&empty_query, 10, empty_head)
            .expect("exact finality-only replay"),
        HedgingBillingReconcileOutcomeV1 {
            pages_applied: 0,
            events_applied: 0,
            next_sequence: 2,
            finalized_cursor: Some(finality_only_cursor),
        }
    );
    assert_eq!(empty_query.calls.load(Ordering::Relaxed), 2);
    let bounded_root = tempfile::tempdir().expect("bounded state root");
    let (bounded_service, ..) = ready_service(bounded_root.path());
    let beyond_head_query =
        TestFinalizedQuery::new(vec![Some(page(vec![event(1, "storage:beyond-head", "1")]))]);
    let query_head = beyond_head_query
        .finalized_head()
        .expect("beyond-head query cursor");
    let earlier_head = HedgingBillingFinalizedCursorV1 {
        height: query_head.height - 1,
        block_hash: [0x31; 32],
        finalized_at_unix: query_head.finalized_at_unix - 1,
    };
    assert_eq!(
        bounded_service
            .reconcile_finalized_query(&beyond_head_query, 1, earlier_head)
            .expect_err("a page beyond the authenticated scan head must fail before ingest"),
        HedgingBillingServiceError::FinalizedForkOrRollback
    );
    assert_eq!(
        bounded_service
            .query_position()
            .expect("unchanged bounded query position")
            .next_sequence,
        1
    );
}
