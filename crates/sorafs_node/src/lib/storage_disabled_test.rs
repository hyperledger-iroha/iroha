// Disabled-backend coverage for the public node storage handle.
#[test]
fn node_handle_storage_methods_error_when_disabled() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let payload = b"disabled storage payload";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    let mut reader = &payload[..];
    let err = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect_err("storage disabled");
    assert!(matches!(err, NodeStorageError::Disabled));
    assert!(matches!(
        handle.with_admitted_payload_read_lease(&[0xA5; 32], |_| ()),
        Err(AdmittedPayloadReadLeaseErrorV1::Disabled)
    ));
}
