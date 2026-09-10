// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::governance::FencedPrivacyStateV1",
            <FencedPrivacyStateV1 as norito::NoritoSchema>::nominal_name(),
            <FencedPrivacyStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagCommittedStateV1",
            <RuntimeDagCommittedStateV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagCommittedStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagKeyTransitionSigningPayloadV1",
            <RuntimeDagKeyTransitionSigningPayloadV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagKeyTransitionSigningPayloadV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagProducerCheckpointV1",
            <RuntimeDagProducerCheckpointV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagProducerCheckpointV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagProducerPublishIntentV1",
            <RuntimeDagProducerPublishIntentV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagProducerPublishIntentV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagProducerStagingStateV1",
            <RuntimeDagProducerStagingStateV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagProducerStagingStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagQualificationArchiveBodyV1",
            <RuntimeDagQualificationArchiveBodyV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagQualificationArchiveBodyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagQualificationArchiveV1",
            <RuntimeDagQualificationArchiveV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagQualificationArchiveV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagQualificationStateV1",
            <RuntimeDagQualificationStateV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagQualificationStateV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagQualificationTransitionBodyV1",
            <RuntimeDagQualificationTransitionBodyV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagQualificationTransitionBodyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::governance::RuntimeDagQualificationTransitionV1",
            <RuntimeDagQualificationTransitionV1 as norito::NoritoSchema>::nominal_name(),
            <RuntimeDagQualificationTransitionV1 as norito::NoritoSchema>::frame_name(),
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
fn governance_retained_state_frames_use_distinct_persistence_roots() {
    let state = FencedPrivacyStateV1 {
        version: GOVERNANCE_FENCED_PRIVACY_STATE_VERSION_V1,
        pending: None,
        publication_cache: None,
        authoritative_head_sync: None,
    };
    let bytes =
        assert_declared_persistence_frame(&state, "sorafs_node::governance::FencedPrivacyStateV1");
    assert_eq!(
        encode_governance_two_slot_value_v1(&state, "schema fixture").unwrap(),
        bytes
    );
    assert_eq!(
        decode_canonical_runtime_dag::<FencedPrivacyStateV1>(&bytes, "schema fixture").unwrap(),
        state
    );
    let committed = RuntimeDagCommittedStateV1 {
        version: 1,
        head_bytes: None,
        index_bytes: None,
    };
    let committed_bytes = assert_declared_persistence_frame(
        &committed,
        "sorafs_node::governance::RuntimeDagCommittedStateV1",
    );
    assert!(
        decode_canonical_runtime_dag::<FencedPrivacyStateV1>(&committed_bytes, "wrong root")
            .is_err()
    );
}
