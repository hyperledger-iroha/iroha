// Repair discovery copies one manifest identifier at a time, with stable exclusive cursors.
#[test]
fn repair_discovery_cursor_visits_each_manifest_once_in_key_order() {
    let directory = tempfile::tempdir().unwrap();
    let backend = StorageBackend::new(temp_config(&directory)).unwrap();
    assert!(backend.next_repair_manifest_id(None).unwrap().is_none());
    let mut expected = Vec::new();
    for seed in 1..=3u8 {
        let payload = vec![seed; 1024];
        let plan = single_file_plan(&payload).unwrap();
        let manifest = test_manifest(&payload, &plan, seed);
        expected.push(
            backend
                .ingest_manifest(&manifest, &plan, &mut payload.as_slice())
                .unwrap(),
        );
    }
    expected.sort();
    let mut observed = Vec::new();
    let mut after = None;
    while let Some(id) = backend.next_repair_manifest_id(after.as_deref()).unwrap() {
        after = Some(id.clone());
        observed.push(id);
    }
    assert_eq!(observed, expected);
    // The cursor need not identify a currently retained object.
    assert_eq!(
        backend.next_repair_manifest_id(Some("0")).unwrap(),
        expected.first().cloned()
    );
    assert!(
        backend
            .next_repair_manifest_id(Some(&"f".repeat(64)))
            .unwrap()
            .is_none()
    );
}
