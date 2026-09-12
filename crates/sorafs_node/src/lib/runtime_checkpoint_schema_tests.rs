// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<AuxiliaryRuntimeCheckpointV5>(
        "sorafs_node::AuxiliaryRuntimeCheckpointV5",
    );
    crate::schema_identity_test_support::assert_identity::<GcStorageIdentityV1>(
        "sorafs_node::GcStorageIdentityV1",
    );
}

#[test]
fn node_auxiliary_checkpoint_and_gc_identity_advertise_explicit_schema_identity() {
    let (config, _temp) = storage_config_with_temp_dir();
    let node = NodeHandle::new(config.clone());
    let checkpoint = node
        .export_auxiliary_runtime_checkpoint()
        .expect("actual auxiliary checkpoint");
    let bytes = crate::schema_identity_test_support::assert_canonical_frame(
        &checkpoint,
        "sorafs_node::AuxiliaryRuntimeCheckpointV5",
    );
    assert_eq!(
        bytes,
        fs::read(auxiliary_runtime_checkpoint_path(config.data_dir()))
            .expect("persisted checkpoint")
    );
    let identity = GcStorageIdentityV1 {
        total_bytes: 17,
        manifest_count: 2,
        gc_freed_bytes_total: 3,
        gc_evictions_total: 1,
        manifest_set_digest: [0x74; 32],
        chunk_refcounts_digest: [0x75; 32],
    };
    let frame = crate::schema_identity_test_support::assert_canonical_frame(
        &identity,
        "sorafs_node::GcStorageIdentityV1",
    );
    assert_eq!(
        norito::decode_canonical::<GcStorageIdentityV1>(&frame).unwrap(),
        identity
    );
}
