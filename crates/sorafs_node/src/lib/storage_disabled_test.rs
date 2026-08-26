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

#[test]
fn emergency_disabled_handle_does_not_open_durable_runtime_trees() {
    let temp = tempfile::tempdir().expect("temporary root");
    let emergency_root = temp.path().join("emergency-fast");
    let handle = NodeHandle::try_new_emergency_disabled(emergency_root.clone())
        .expect("disabled process-local handle");

    assert!(!handle.is_enabled());
    assert_eq!(handle.config().runtime_retention().state_entry_limit(), 1);
    assert_eq!(handle.config().runtime_retention().event_history_limit(), 1);
    assert!(
        !emergency_root.exists(),
        "Fast startup must not create PoTR or transaction-forwarder trees"
    );
}
