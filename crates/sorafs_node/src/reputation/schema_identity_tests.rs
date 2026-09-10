// Explicit framed roots and actual persistence codec regressions.

#[test]
fn declared_schema_roots_are_distinct_and_explicit() {
    let rows = [
        (
            "sorafs_node::reputation::ReputationFinalizedIdentityV1",
            <ReputationFinalizedIdentityV1 as norito::NoritoSchema>::nominal_name(),
            <ReputationFinalizedIdentityV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::reputation::ReputationIngestCheckpointV1",
            <ReputationIngestCheckpointV1 as norito::NoritoSchema>::nominal_name(),
            <ReputationIngestCheckpointV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::reputation::ReputationIngestPolicyV1",
            <ReputationIngestPolicyV1 as norito::NoritoSchema>::nominal_name(),
            <ReputationIngestPolicyV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::reputation::ReputationSnapshotSeedV1",
            <ReputationSnapshotSeedV1 as norito::NoritoSchema>::nominal_name(),
            <ReputationSnapshotSeedV1 as norito::NoritoSchema>::frame_name(),
        ),
        (
            "sorafs_node::reputation::ReputationUnsignedSigningMaterialV1",
            <ReputationUnsignedSigningMaterialV1 as norito::NoritoSchema>::nominal_name(),
            <ReputationUnsignedSigningMaterialV1 as norito::NoritoSchema>::frame_name(),
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
fn reputation_ingest_checkpoint_schema_survives_policy_bound_recovery() {
    let policy = policy();
    let digest = policy.canonical_digest().expect("policy digest");
    let checkpoint = ReputationIngestCheckpointV1::empty(digest);
    let bytes = assert_declared_persistence_frame(
        &checkpoint,
        "sorafs_node::reputation::ReputationIngestCheckpointV1",
    );
    assert_eq!(
        decode_checkpoint(&bytes, &policy, digest).unwrap(),
        checkpoint
    );
    let policy_bytes = assert_declared_persistence_frame(
        &policy,
        "sorafs_node::reputation::ReputationIngestPolicyV1",
    );
    assert!(decode_checkpoint(&policy_bytes, &policy, digest).is_err());
    assert_declared_persistence_frame(
        &ReputationFinalizedIdentityV1 {
            height: TARGET_HEIGHT,
            block_hash: TARGET_HASH,
        },
        "sorafs_node::reputation::ReputationFinalizedIdentityV1",
    );
}
