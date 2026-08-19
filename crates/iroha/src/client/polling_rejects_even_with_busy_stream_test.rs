#[tokio::test]
async fn polling_rejects_even_with_busy_stream() {
    let hash: HashOf<SignedTransaction> =
        HashOf::from_untyped_unchecked(Hash::prehashed([12_u8; Hash::LENGTH]));
    let queued_event = EventBox::Pipeline(PipelineEventBox::Transaction(TransactionEvent {
        hash,
        block_height: None,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        status: TransactionStatus::Queued,
    }));
    let (tx, rx) = mpsc::unbounded_channel::<Result<EventBox, eyre::Report>>();
    let mut events = UnboundedReceiverStream::new(rx);
    for _ in 0..128 {
        let _ = tx.send(Ok(queued_event.clone()));
    }
    let mut checks = 0u8;
    let rejection = TransactionRejectionReason::Validation(ValidationFail::InternalError(
        "rejected".to_string(),
    ));
    let err = listen_for_tx_confirmation_stream_with_status_check(
        &mut events,
        hash,
        Duration::from_secs(1),
        Duration::from_millis(1),
        None,
        || {
            checks = checks.saturating_add(1);
            Ok(Some(super::TxConfirmationStatus::Rejected(Some(
                rejection.clone(),
            ))))
        },
    )
    .await
    .expect_err("expected rejection from status polling");
    assert!(err.to_string().contains("Transaction rejected"));
    assert!(checks > 0);
}
