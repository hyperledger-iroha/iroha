#[test]
fn bounded_status_page_matches_full_sort_across_restart_and_cursor_boundaries() {
    let directory = tempdir().expect("tempdir");
    let configured_policy = policy();
    let runtime = Arc::new(TestCheckpointRuntime::new(0xA4));
    let outbox = open_sealed(&directory, Arc::clone(&runtime)).expect("sealed outbox");
    let authorizations = (1..=12)
        .map(|order| authorization(order, 7))
        .collect::<Vec<_>>();
    for authorization in &authorizations {
        outbox.enqueue(authorization.clone()).expect("enqueue");
    }
    let (musubi, receipt) = musubi_authorization_and_receipt(0x60, 7, 0x36);
    observe_finalized(&outbox, cursor(7));
    outbox.enqueue(musubi.clone()).expect("enqueue Musubi");
    let claim = outbox
        .claim_source(musubi.job_id(), owner(6), 100, cursor(7))
        .expect("claim Musubi source");
    outbox
        .mark_local_stored_verified(&claim, 101, manifest_id(&musubi), Some(receipt.clone()))
        .expect("persist Musubi receipt");
    observe_finalized(&outbox, cursor(8));
    for index in [1, 2, 6, 10] {
        let authorization = &authorizations[index];
        outbox
            .cancel(
                authorization.job_id(),
                cancellation_evidence(
                    authorization,
                    ProviderIngestCancellationReasonV1::OrderExpired,
                    8,
                ),
            )
            .expect("finalized cancellation");
    }
    let expected = {
        let state = outbox.state.lock().expect("outbox state");
        let mut rows = state
            .checkpoint
            .active
            .iter()
            .map(active_status)
            .chain(state.checkpoint.terminal.iter().map(terminal_status))
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.job_id);
        rows
    };
    assert_eq!(expected.len(), 13);
    assert!(expected.iter().any(|row| matches!(
        &row.state,
        ProviderIngestDeliveryStateV1::PendingSource { .. }
    )));
    assert!(
        expected
            .iter()
            .any(|row| matches!(&row.state, ProviderIngestDeliveryStateV1::Cancelled { .. }))
    );
    assert!(expected.iter().any(|row| matches!(
        &row.state,
        ProviderIngestDeliveryStateV1::LocalStored {
            musubi_bundle: Some(retained),
            ..
        } if retained.as_ref() == &receipt
    )));
    drop(outbox);
    let reopened = open_sealed(&directory, runtime).expect("restart from sealed authority");
    for limit in [1, configured_policy.max_status_page_size] {
        let mut after = None;
        loop {
            let page = reopened.statuses_page(after, limit).expect("bounded page");
            let remaining = expected
                .iter()
                .filter(|row| after.is_none_or(|id| row.job_id > id))
                .collect::<Vec<_>>();
            assert_eq!(
                page.rows,
                remaining
                    .iter()
                    .take(limit)
                    .map(|row| (*row).clone())
                    .collect::<Vec<_>>()
            );
            let next = (remaining.len() > limit).then(|| remaining[limit - 1].job_id);
            assert_eq!(page.next_after_job_id, next);
            let Some(cursor) = next else { break };
            after = Some(cursor);
        }
        for after in [
            Some([0; 32]),
            Some(expected[limit - 1].job_id),
            Some([0xFF; 32]),
        ] {
            let page = reopened.statuses_page(after, limit).expect("boundary page");
            let remaining = expected
                .iter()
                .filter(|row| after.is_none_or(|id| row.job_id > id))
                .collect::<Vec<_>>();
            assert_eq!(
                page.rows,
                remaining
                    .iter()
                    .take(limit)
                    .map(|row| (*row).clone())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                page.next_after_job_id,
                (remaining.len() > limit).then(|| remaining[limit - 1].job_id)
            );
        }
    }
}

#[test]
fn bounded_status_page_preserves_stable_duplicate_order_in_corrupt_memory() {
    let outbox = ProviderIngestOutbox::in_memory(policy()).expect("outbox");
    let authorization = authorization(0x98, 7);
    outbox.enqueue(authorization.clone()).expect("enqueue");
    // Persisted checkpoints reject duplicate identities. This deliberately corrupt in-memory
    // snapshot checks that the bounded read retains the former stable-sort behavior.
    {
        let mut state = outbox.state.lock().expect("outbox state");
        let sequence = state.checkpoint.next_sequence;
        state
            .checkpoint
            .terminal
            .push(StoredTerminalProviderIngestV1 {
                sequence,
                authorization: authorization.clone(),
                outcome: StoredProviderIngestTerminalOutcomeV1::Cancelled {
                    reason: ProviderIngestCancellationReasonV1::OrderExpired,
                    observed_finalized_cursor: cursor(8),
                },
            });
    }
    let page = outbox.statuses_page(None, 2).expect("duplicate page");
    assert_eq!(page.rows.len(), 2);
    assert!(matches!(
        &page.rows[0].state,
        ProviderIngestDeliveryStateV1::PendingSource { .. }
    ));
    assert!(matches!(
        &page.rows[1].state,
        ProviderIngestDeliveryStateV1::Cancelled { .. }
    ));
    assert_eq!(page.next_after_job_id, None);
    let single = outbox.statuses_page(None, 1).expect("single duplicate");
    assert_eq!(single.rows.as_slice(), &page.rows[..1]);
    assert_eq!(single.next_after_job_id, Some(authorization.job_id()));
    assert!(
        outbox
            .statuses_page(single.next_after_job_id, 1)
            .expect("cursor excludes duplicate identity")
            .rows
            .is_empty()
    );
}
