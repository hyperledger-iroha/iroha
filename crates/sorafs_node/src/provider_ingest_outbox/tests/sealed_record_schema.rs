// Sealed record framing and the original byte/digest/revision/lineage rejection test.

#[test]
fn sealed_record_rejects_byte_digest_revision_and_lineage_tamper() {
    let checkpoint_bytes =
        encode_provider_ingest_checkpoint(&ProviderIngestOutboxCheckpointV1::default(), policy())
            .expect("checkpoint bytes");
    let record = ProviderIngestSealedCheckpointRecordV1::new(1, None, None, checkpoint_bytes);
    let mut tampered_bytes = record.clone();
    tampered_bytes.checkpoint_bytes[0] ^= 0x80;
    assert_eq!(
        tampered_bytes.validate(policy().checkpoint_max_bytes),
        Err(ProviderIngestOutboxError::InvalidSealedCheckpoint)
    );
    let mut tampered_digest = record.clone();
    tampered_digest.checkpoint_digest[0] ^= 0x80;
    assert_eq!(
        tampered_digest.validate(policy().checkpoint_max_bytes),
        Err(ProviderIngestOutboxError::InvalidSealedCheckpoint)
    );
    let mut tampered_revision = record.clone();
    tampered_revision.revision[0] ^= 0x80;
    assert_eq!(
        tampered_revision.validate(policy().checkpoint_max_bytes),
        Err(ProviderIngestOutboxError::InvalidSealedCheckpoint)
    );
    let mut tampered_lineage = record;
    tampered_lineage.predecessor_revision = Some([0xA5; 32]);
    assert_eq!(
        tampered_lineage.validate(policy().checkpoint_max_bytes),
        Err(ProviderIngestOutboxError::InvalidSealedCheckpoint)
    );
}

#[test]
fn sealed_checkpoint_schema_distinguishes_inner_state_and_signing_context() {
    use crate::schema_identity_test_support::assert_canonical_frame;
    let authorization = authorization(0x61, 7);
    assert_canonical_frame(
        &authorization,
        "sorafs_node::provider_ingest_outbox::FinalizedProviderIngestAuthorizationV1",
    );
    crate::schema_identity_test_support::assert_identity::<FinalizedProviderIngestMusubiContextV1>(
        "sorafs_node::provider_ingest_outbox::FinalizedProviderIngestMusubiContextV1",
    );
    let outbox = ProviderIngestOutbox::in_memory(policy()).unwrap();
    outbox.enqueue(authorization.clone()).unwrap();
    let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
    let checkpoint_bytes = assert_canonical_frame(
        &checkpoint,
        "sorafs_node::provider_ingest_outbox::ProviderIngestOutboxCheckpointV1",
    );
    assert_eq!(
        encode_provider_ingest_checkpoint(&checkpoint, policy()).unwrap(),
        checkpoint_bytes
    );
    let sealed =
        ProviderIngestSealedCheckpointRecordV1::new(1, None, None, checkpoint_bytes.clone());
    let bytes = assert_canonical_frame(
        &sealed,
        "sorafs_node::provider_ingest_outbox::ProviderIngestSealedCheckpointRecordV1",
    );
    assert_eq!(
        sealed
            .to_canonical_bytes(policy().checkpoint_max_bytes)
            .unwrap(),
        bytes
    );
    assert_eq!(
        ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(
            &bytes,
            policy().checkpoint_max_bytes
        )
        .unwrap(),
        sealed
    );
    let transaction = signed_completion_at(&authorization, 8, cursor(7), 0x41);
    let context = completion_context(&transaction, 8, cursor(7));
    let context_bytes = assert_canonical_frame(
        &context,
        "sorafs_node::provider_ingest_outbox::ProviderIngestCompletionSigningContextV1",
    );
    for foreign in [&checkpoint_bytes, &context_bytes] {
        assert!(matches!(
            norito::decode_canonical::<ProviderIngestSealedCheckpointRecordV1>(foreign),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(
            ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(
                foreign,
                policy().checkpoint_max_bytes
            )
            .is_err()
        );
    }
    assert!(decode_provider_ingest_checkpoint(&bytes, policy()).is_err());
}
