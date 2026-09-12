// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        ("sorafs_node::reputation::runtime::ReputationJournalAbsenceReceiptMaterialV1", <ReputationJournalAbsenceReceiptMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalAbsenceReceiptMaterialV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalProducerCheckpointV1", <ReputationJournalProducerCheckpointV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalProducerCheckpointV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalProducerPolicyDigestMaterialV1", <ReputationJournalProducerPolicyDigestMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalProducerPolicyDigestMaterialV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalSealedCheckpointRecordV1", <ReputationJournalSealedCheckpointRecordV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalSealedCheckpointRecordV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalSourceMaterialV1", <ReputationJournalSourceMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalSourceMaterialV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalSubmitterPolicyDigestMaterialV1", <ReputationJournalSubmitterPolicyDigestMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalSubmitterPolicyDigestMaterialV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationJournalTransactionIdempotencyMaterialV1", <ReputationJournalTransactionIdempotencyMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationJournalTransactionIdempotencyMaterialV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationPublicationCheckpointV1", <ReputationPublicationCheckpointV1 as norito::NoritoSchema>::nominal_name(), <ReputationPublicationCheckpointV1 as norito::NoritoSchema>::frame_name()),
        ("sorafs_node::reputation::runtime::ReputationPublicationPolicyDigestMaterialV1", <ReputationPublicationPolicyDigestMaterialV1 as norito::NoritoSchema>::nominal_name(), <ReputationPublicationPolicyDigestMaterialV1 as norito::NoritoSchema>::frame_name()),
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
fn journal_checkpoint_schema_survives_bounded_encoding_and_sealed_record() {
    let policy = producer_policy();
    let digest = policy.digest().expect("policy digest");
    let checkpoint =
        ReputationJournalProducerCheckpointV1::empty(digest, policy.authority_policy.clone());
    let bytes = assert_declared_persistence_frame(
        &checkpoint,
        "sorafs_node::reputation::runtime::ReputationJournalProducerCheckpointV1",
    );
    let (bounded, encoded) = encode_bounded_journal_checkpoint(
        checkpoint.clone(),
        &policy,
        digest,
        policy.checkpoint_max_bytes,
    )
    .expect("bounded checkpoint");
    assert_eq!(bounded, checkpoint);
    assert_eq!(encoded, bytes);
    assert_eq!(
        decode_journal_checkpoint(&bytes, &policy, digest).unwrap(),
        checkpoint
    );
    let record = ReputationJournalSealedCheckpointRecordV1::new(1, None, bytes.clone()).unwrap();
    assert_declared_persistence_frame(
        &record,
        "sorafs_node::reputation::runtime::ReputationJournalSealedCheckpointRecordV1",
    );
    assert_eq!(record.checkpoint_bytes, bytes);
    let publication = ReputationPublicationCheckpointV1::empty(digest);
    let publication_bytes = assert_declared_persistence_frame(
        &publication,
        "sorafs_node::reputation::runtime::ReputationPublicationCheckpointV1",
    );
    assert!(decode_journal_checkpoint(&publication_bytes, &policy, digest).is_err());
}
