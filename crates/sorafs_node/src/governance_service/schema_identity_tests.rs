// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::governance_service::MirrorIndexStorePayloadV1",
            <MirrorIndexStorePayloadV1 as norito::NoritoSchema>::nominal_name(),
            <MirrorIndexStorePayloadV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_service::PublishIntentBodyV1",
            <PublishIntentBodyV1 as norito::NoritoSchema>::nominal_name(),
            <PublishIntentBodyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_service::RequestAuthReplayStateV1",
            <RequestAuthReplayStateV1 as norito::NoritoSchema>::nominal_name(),
            <RequestAuthReplayStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_service::SignedBlockPrefixArchiveV1",
            <SignedBlockPrefixArchiveV1 as norito::NoritoSchema>::nominal_name(),
            <SignedBlockPrefixArchiveV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance_service::CheckpointBodyV1",
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
fn mirror_state_schema_survives_actual_bounded_persistence_codec() {
    let payload = MirrorIndexStorePayloadV1::empty();
    let bytes = assert_declared_persistence_frame(
        &payload,
        "sorafs_node::governance_service::MirrorIndexStorePayloadV1",
    );
    assert_eq!(encode_mirror_index_store_payload(&payload).unwrap(), bytes);
    assert_eq!(decode_mirror_index_store_payload(&bytes).unwrap(), payload);
    let replay = RequestAuthReplayStateV1 {
        version: 1,
        entries: Vec::new(),
    };
    let replay_bytes = assert_declared_persistence_frame(
        &replay,
        "sorafs_node::governance_service::RequestAuthReplayStateV1",
    );
    assert!(decode_mirror_index_store_payload(&replay_bytes).is_err());
}
