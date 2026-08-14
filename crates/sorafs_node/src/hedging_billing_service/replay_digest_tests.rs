#[test]
fn cross_peer_old_page_replay_is_exact_and_equivocation_safe() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, _reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    let first = page(vec![event(1, "storage:event:1", "10")]);
    let second = page(vec![event(2, "storage:event:2", "2")]);
    service
        .ingest_finalized_page(&first)
        .expect("ingest first peer page");
    service
        .ingest_finalized_page(&second)
        .expect("ingest second peer page");
    assert_eq!(
        service
            .ingest_finalized_page(&first)
            .expect("old exact cross-peer replay"),
        HedgingBillingIngestOutcomeV1::Replay { next_sequence: 3 }
    );
    let forged = page(vec![event(1, "storage:event:1", "11")]);
    assert!(matches!(
        service.ingest_finalized_page(&forged),
        Err(HedgingBillingServiceError::FinalizedEventEquivocation)
    ));
}
#[test]
fn event_replay_digest_binds_network_and_exact_event() {
    let first_network = test_network_id(b"billing-network-a");
    let second_network = test_network_id(b"billing-network-b");
    let first_event = event(1, "storage:event:1", "10");
    let mut changed_event = first_event.clone();
    changed_event.quantity_units = 2;
    let first_digest = event_replay_digest(first_network, &first_event).expect("digest");
    eprintln!(
        "billing replay compatibility digest: {}",
        hex::encode(first_digest)
    );
    assert_ne!(
        first_digest,
        event_replay_digest(second_network, &first_event).expect("network-bound digest")
    );
    assert_ne!(
        first_digest,
        event_replay_digest(first_network, &changed_event).expect("event-bound digest")
    );
}
