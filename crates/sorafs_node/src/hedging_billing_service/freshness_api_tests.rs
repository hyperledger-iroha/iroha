#[test]
fn acknowledgement_precommit_fence_preserves_checkpoint_for_head_race() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, reference, _verifier, _publisher, ack_authority) =
        ready_service(root.path());
    let first = page(vec![event(1, "storage:event:ack-fence", "10")]);
    service
        .ingest_finalized_page(&first)
        .expect("committed source page");
    let period = service
        .finalize_next_period(&period_close(&reference, first.journal_commitment))
        .expect("committed period");
    let statement_id = period.statement_ids[0];
    service
        .sign_next_statement(&TestSigner::valid())
        .expect("sign")
        .expect("signed statement");
    let receipt = service
        .publish_next_statement()
        .expect("publish")
        .expect("publication receipt");
    let anchor_before = service.api_projection_anchor().expect("published anchor");
    let request = BillingStatementAcknowledgementRequestV1 {
        expected_checkpoint_fingerprint: anchor_before.checkpoint_fingerprint,
        statement_id,
        owner_account_id: primary_account_bytes(),
        request_nonce: [0x93; 32],
        authentication_proof: vec![0xAC],
    };
    let fence_calls = AtomicUsize::new(0);
    let mut advancing_head_fence = || {
        if fence_calls.fetch_add(1, Ordering::Relaxed) == 0 {
            Ok(())
        } else {
            Err(HedgingBillingServiceError::External(
                HedgingBillingExternalError::Unavailable,
            ))
        }
    };
    assert_eq!(
        service
            .api_acknowledge_statement_with_precommit_fence(
                &request,
                receipt.published_at_unix + 1,
                &mut advancing_head_fence,
            )
            .expect_err("head change before commit must fail closed"),
        HedgingBillingRuntimeApiErrorV1::Unavailable
    );
    assert_eq!(fence_calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        service
            .api_projection_anchor()
            .expect("anchor after rejected commit"),
        anchor_before,
        "a failed immediate pre-commit fence must not mutate the local checkpoint"
    );
    let still_published = service
        .api_published_statement(&BillingPublishedStatementRequestV1 {
            owner_account_id: request.owner_account_id.clone(),
            statement_id,
            expected_checkpoint_fingerprint: anchor_before.checkpoint_fingerprint,
        })
        .expect("unchanged published statement");
    assert!(still_published.acknowledgement.is_none());
    assert!(
        ack_authority
            .lookup(statement_id)
            .expect("authoritative acknowledgement lookup")
            .is_some(),
        "the external durable acknowledgement remains available for reconciliation"
    );
    let mut stable_head_fence = || Ok(());
    let reconciled = service
        .api_acknowledge_statement_with_precommit_fence(
            &request,
            receipt.published_at_unix + 2,
            &mut stable_head_fence,
        )
        .expect("stable retry reconciles the durable acknowledgement");
    assert_eq!(reconciled.acknowledgement.statement_id, statement_id);
    assert_ne!(reconciled.anchor, anchor_before);
}
#[test]
fn exposure_and_intent_pages_include_below_threshold_periods_without_auto_execution() {
    let root = tempfile::tempdir().expect("state root");
    let (service, _feed_policy, reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    let mut second = event(2, "storage:event:below-threshold:2", "1");
    second.account_id = account_bytes(0x92);
    let first = page(vec![
        event(1, "storage:event:below-threshold:1", "1"),
        second,
    ]);
    service
        .ingest_finalized_page(&first)
        .expect("committed source page");
    let period = service
        .finalize_next_period(&period_close(&reference, first.journal_commitment))
        .expect("committed below-threshold period");
    assert!(period.hedge_intent.is_none());
    let anchor = service.api_projection_anchor().expect("projection anchor");
    let request = HedgingBillingProjectionPageRequestV1 {
        expected_checkpoint_fingerprint: anchor.checkpoint_fingerprint,
        after: None,
        limit: 1,
    };
    let exposure = service
        .api_exposure_page(&request)
        .expect("below-threshold exposure page");
    assert_eq!(exposure.items.len(), 1);
    assert_eq!(exposure.items[0].statement_count, 2);
    assert_eq!(exposure.items[0].xor_exposure, xor("2"));
    assert!(!exposure.items[0].hedge_threshold_reached);
    assert!(exposure.items[0].hedge_intent_id.is_none());
    assert!(!exposure.items[0].automatic_execution);
    let after = service
        .api_exposure_page(&HedgingBillingProjectionPageRequestV1 {
            after: Some(exposure.items[0].cursor),
            ..request
        })
        .expect("exclusive exposure cursor");
    assert!(after.items.is_empty());
    assert_eq!(
        service
            .api_exposure_page(&HedgingBillingProjectionPageRequestV1 {
                after: Some([0xCC; 32]),
                ..request
            })
            .expect_err("unknown exposure cursor"),
        HedgingBillingRuntimeApiErrorV1::InvalidRequest
    );
    let intents = service
        .api_hedge_intent_page(&request)
        .expect("empty hedge-intent page");
    assert!(intents.items.is_empty());
    assert!(!intents.automatic_execution_enabled);
    assert!(matches!(
        service.pending_statement_delivery_projections(0),
        Err(HedgingBillingServiceError::InvalidQueryBound)
    ));
    assert!(matches!(
        service
            .pending_statement_delivery_projections(HEDGING_BILLING_MAX_DELIVERY_WORK_ITEMS_V1 + 1),
        Err(HedgingBillingServiceError::InvalidQueryBound)
    ));
    let work = service
        .pending_statement_delivery_projections(1)
        .expect("bounded delivery work scan");
    assert_eq!(work.len(), 1);
    assert_eq!(
        work[0].status,
        BillingStatementDeliveryStatusV1::ReadyForSigning
    );
    let rotated = service
        .pending_statement_delivery_projections_rotated(1, 1)
        .expect("rotated bounded delivery work scan");
    assert_eq!(rotated.len(), 1);
    assert_ne!(
        work[0].statement_id, rotated[0].statement_id,
        "the scan cursor must rotate within a persistent stage backlog"
    );
    let selected_id = rotated[0].statement_id;
    let signed = service
        .sign_statement(selected_id, &TestSigner::valid())
        .expect("sign the exact fair-scan selection");
    assert_eq!(
        signed.governed_statement.statement.statement_id,
        selected_id
    );
    let delivery_states = service
        .statement_delivery_projections()
        .expect("inspect exact delivery states");
    let selected = delivery_states
        .iter()
        .find(|projection| projection.statement_id == selected_id)
        .expect("selected statement projection");
    assert_eq!(
        selected.status,
        BillingStatementDeliveryStatusV1::ReadyForPublication
    );
    let original = delivery_states
        .iter()
        .find(|projection| projection.statement_id == work[0].statement_id)
        .expect("original first statement projection");
    assert_eq!(
        original.status,
        BillingStatementDeliveryStatusV1::ReadyForSigning
    );
    let receipt = service
        .publish_statement(selected_id)
        .expect("publish the exact fair-scan selection");
    assert_eq!(receipt.statement_id, selected_id);
    let after_publication = service
        .statement_delivery_projections()
        .expect("inspect states after targeted publication");
    let original = after_publication
        .iter()
        .find(|projection| projection.statement_id == work[0].statement_id)
        .expect("original first statement after publication");
    assert_eq!(
        original.status,
        BillingStatementDeliveryStatusV1::ReadyForSigning,
        "targeted publication must not disturb an unrelated ready statement"
    );
}
#[test]
fn exact_network_domain_roundtrips_and_rejects_cross_genesis_replay() {
    let policy = service_policy();
    let policy_bytes = policy.canonical_bytes().expect("canonical policy bytes");
    assert_eq!(
        HedgingBillingServicePolicyV1::from_canonical_bytes(&policy_bytes)
            .expect("decode canonical policy"),
        policy
    );
    let mut trailing = policy_bytes;
    trailing.push(0);
    assert!(matches!(
        HedgingBillingServicePolicyV1::from_canonical_bytes(&trailing),
        Err(HedgingBillingServiceError::InvalidPolicy)
    ));
    let event = event(1, "storage:network-domain:1", "1");
    let page = page(vec![event.clone()]);
    let page_bytes = norito::to_bytes(&page).expect("canonical page bytes");
    let decoded: HedgingBillingFinalizedEventPageV1 =
        norito::decode_from_bytes(&page_bytes).expect("decode canonical page");
    assert_eq!(decoded, page);
    let foreign_network = test_network_id(b"hedging-billing-foreign-genesis");
    assert_ne!(policy.network_id, foreign_network);
    let mut foreign_page = page.clone();
    foreign_page.network_id = foreign_network;
    foreign_page.journal_commitment.network_id = foreign_network;
    assert!(matches!(
        foreign_page.validate(&policy),
        Err(HedgingBillingServiceError::InvalidFinalizedPage)
    ));
    assert_ne!(
        source_receipt(policy.network_id, &event).expect("local source receipt"),
        source_receipt(foreign_network, &event).expect("foreign source receipt")
    );
    assert_ne!(
        event_replay_digest(policy.network_id, &event).expect("local replay identity"),
        event_replay_digest(foreign_network, &event).expect("foreign replay identity")
    );
    let checkpoint = HedgingBillingCheckpointV1::empty(&policy).expect("empty checkpoint");
    let checkpoint_bytes = encode_checkpoint(&checkpoint, &policy, &feed_policy())
        .expect("canonical checkpoint bytes");
    let mut foreign_policy = policy;
    foreign_policy.network_id = foreign_network;
    assert!(matches!(
        decode_checkpoint(&checkpoint_bytes, &foreign_policy, &feed_policy()),
        Err(HedgingBillingServiceError::InvalidCheckpoint)
    ));
}
