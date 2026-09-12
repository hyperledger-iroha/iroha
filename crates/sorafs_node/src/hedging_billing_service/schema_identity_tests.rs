// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::hedging_billing_service::BillingEventReplayPreimageV1",
            <BillingEventReplayPreimageV1 as norito::NoritoSchema>::nominal_name(),
            <BillingEventReplayPreimageV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::BillingSourceReceiptPreimageV1",
            <BillingSourceReceiptPreimageV1 as norito::NoritoSchema>::nominal_name(),
            <BillingSourceReceiptPreimageV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::BillingStatementAcknowledgementV1",
            <BillingStatementAcknowledgementV1 as norito::NoritoSchema>::nominal_name(),
            <BillingStatementAcknowledgementV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::BillingStatementPublicationReceiptV1",
            <BillingStatementPublicationReceiptV1 as norito::NoritoSchema>::nominal_name(),
            <BillingStatementPublicationReceiptV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::BillingStatementSignaturePreimageV1",
            <BillingStatementSignaturePreimageV1 as norito::NoritoSchema>::nominal_name(),
            <BillingStatementSignaturePreimageV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::CompactedEconomicArchiveV1",
            <CompactedEconomicArchiveV1 as norito::NoritoSchema>::nominal_name(),
            <CompactedEconomicArchiveV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::CompactedSourceArchiveV1",
            <CompactedSourceArchiveV1 as norito::NoritoSchema>::nominal_name(),
            <CompactedSourceArchiveV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::GovernedHedgeExecutionPolicyV1",
            <GovernedHedgeExecutionPolicyV1 as norito::NoritoSchema>::nominal_name(),
            <GovernedHedgeExecutionPolicyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgeExecutionAuthorizationV1",
            <HedgeExecutionAuthorizationV1 as norito::NoritoSchema>::nominal_name(),
            <HedgeExecutionAuthorizationV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgeExecutionSubmissionReceiptV1",
            <HedgeExecutionSubmissionReceiptV1 as norito::NoritoSchema>::nominal_name(),
            <HedgeExecutionSubmissionReceiptV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgeIntentV1",
            <HedgeIntentV1 as norito::NoritoSchema>::nominal_name(),
            <HedgeIntentV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingCheckpointV1",
            <HedgingBillingCheckpointV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingCheckpointV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingEpochTransitionV1",
            <HedgingBillingEpochTransitionV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingEpochTransitionV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessRecordV1",
            <HedgingBillingEpochWitnessRecordV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingEpochWitnessRecordV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingFinalizedEventPageV1",
            <HedgingBillingFinalizedEventPageV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingFinalizedEventPageV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingPeriodClosePreimageV1",
            <HedgingBillingPeriodClosePreimageV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingPeriodClosePreimageV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::HedgingBillingServicePolicyV1",
            <HedgingBillingServicePolicyV1 as norito::NoritoSchema>::nominal_name(),
            <HedgingBillingServicePolicyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::RetainedEpochBaseStateV1",
            <RetainedEpochBaseStateV1 as norito::NoritoSchema>::nominal_name(),
            <RetainedEpochBaseStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::hedging_billing_service::SignedGovernedBillingStatementV1",
            <SignedGovernedBillingStatementV1 as norito::NoritoSchema>::nominal_name(),
            <SignedGovernedBillingStatementV1 as norito::NoritoSchema>::frame_name(),
        ),
    ];
    let mut identities = std::collections::BTreeSet::new();
    for (expected, nominal, frame) in rows {
        assert_eq!(nominal, expected);
        assert_eq!(frame, expected);
        assert!(
            identities.insert(frame),
            "different roots must remain distinct"
        );
    }
}

fn assert_declared_persistence_frame<T>(value: &T, expected_root: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize + std::fmt::Debug + PartialEq,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("canonical typed frame");
    let view = norito::core::from_bytes_view(&frame).expect("validated frame");
    assert_eq!(
        view.schema(),
        norito::core::schema_hash_for_name(expected_root)
    );
    assert_eq!(
        norito::canonical_frame_len(value).expect("exact frame length"),
        frame.len()
    );
    assert_eq!(
        &norito::decode_canonical::<T>(&frame).expect("typed recovery"),
        value
    );
    let mut substituted = frame.clone();
    // The canonical header puts its 16-byte schema after magic and two version bytes.
    substituted[6..22].copy_from_slice(&norito::core::schema_hash_for_name(
        "sorafs_node::different.persistence.root",
    ));
    assert!(matches!(
        norito::decode_canonical::<T>(&substituted),
        Err(norito::Error::SchemaMismatch)
    ));
    let mut suffixed = frame.clone();
    suffixed.push(0);
    assert!(norito::decode_canonical::<T>(&suffixed).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    frame
}

#[test]
fn settled_billing_checkpoint_schema_survives_actual_recovery() {
    let root = tempfile::tempdir().expect("state root");
    let (service, feed_policy, reference, _verifier, _publisher, _ack_authority) =
        ready_service(root.path());
    settle_first_period(&service, &reference);
    let policy = service_policy();
    let checkpoint = service.state.lock().expect("state").checkpoint.clone();
    let bytes = assert_declared_persistence_frame(
        &checkpoint,
        "sorafs_node::hedging_billing_service::HedgingBillingCheckpointV1",
    );
    assert_eq!(
        encode_checkpoint(&checkpoint, &policy, &feed_policy).unwrap(),
        bytes
    );
    assert_eq!(
        decode_checkpoint(&bytes, &policy, &feed_policy).unwrap(),
        checkpoint
    );
    let policy_bytes = assert_declared_persistence_frame(
        &policy,
        "sorafs_node::hedging_billing_service::HedgingBillingServicePolicyV1",
    );
    assert!(decode_checkpoint(&policy_bytes, &policy, &feed_policy).is_err());
    let stored = checkpoint.statements.first().expect("settled statement");
    assert_declared_persistence_frame(
        stored.signed_statement.as_ref().expect("signed statement"),
        "sorafs_node::hedging_billing_service::SignedGovernedBillingStatementV1",
    );
    assert_declared_persistence_frame(
        stored
            .publication_receipt
            .as_ref()
            .expect("publication receipt"),
        "sorafs_node::hedging_billing_service::BillingStatementPublicationReceiptV1",
    );
}
