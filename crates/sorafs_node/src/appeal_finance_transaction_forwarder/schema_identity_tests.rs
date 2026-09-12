// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceSealedCheckpointRecordV1",
            <AppealFinanceSealedCheckpointRecordV1 as norito::NoritoSchema>::nominal_name(),
            <AppealFinanceSealedCheckpointRecordV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::appeal_finance_transaction_forwarder::AuthenticatedCheckpointV1",
            <AuthenticatedCheckpointV1 as norito::NoritoSchema>::nominal_name(),
            <AuthenticatedCheckpointV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::appeal_finance_transaction_forwarder::DrawdownIdentityMaterialV1",
            <DrawdownIdentityMaterialV1 as norito::NoritoSchema>::nominal_name(),
            <DrawdownIdentityMaterialV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::appeal_finance_transaction_forwarder::PreparedOperationMaterialV1",
            <PreparedOperationMaterialV1 as norito::NoritoSchema>::nominal_name(),
            <PreparedOperationMaterialV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::appeal_finance_transaction_forwarder::CheckpointBodyV1",
            <CheckpointBodyV1 as norito::NoritoSchema>::nominal_name(),
            <CheckpointBodyV1 as norito::NoritoSchema>::frame_name(),
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
fn authenticated_checkpoint_roots_bind_the_complete_persisted_frame() {
    let body = CheckpointBodyV1::default();
    let body_frame = assert_declared_persistence_frame(
        &body,
        "sorafs_node::appeal_finance_transaction_forwarder::CheckpointBodyV1",
    );
    let checkpoint = AuthenticatedCheckpointV1 {
        version: 1,
        checkpoint_sequence: 1,
        predecessor_checkpoint_digest: None,
        provider_handle: "schema-test".to_owned(),
        public_key: [1; 32],
        provider_revision: 1,
        provider_policy_digest: [2; 32],
        body_digest: checkpoint_body_digest(&body).expect("body digest"),
        body,
        checkpoint_digest: [3; 32],
        signature: [4; 64],
    };
    let bytes = assert_declared_persistence_frame(
        &checkpoint,
        "sorafs_node::appeal_finance_transaction_forwarder::AuthenticatedCheckpointV1",
    );
    assert!(norito::decode_canonical::<AuthenticatedCheckpointV1>(&body_frame).is_err());
    let record = AppealFinanceSealedCheckpointRecordV1::new(1, [3; 32], bytes.clone());
    assert_declared_persistence_frame(
        &record,
        "sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceSealedCheckpointRecordV1",
    );
    assert_eq!(record.checkpoint_bytes, bytes);
    let mut changed = record.clone();
    changed.checkpoint_bytes[0] ^= 1;
    assert_ne!(sealed_checkpoint_record_revision(&changed), record.revision);
}
