// Default storage-class coverage for declaration metadata without an override.
#[test]
fn storage_class_metadata_defaults_when_missing() {
    let metadata = Metadata::default();
    let provider = ProviderId::new([0x11; 32]);
    let class =
        super::storage_class_from_declaration_metadata(provider, &metadata, StorageClass::Warm)
            .expect("fallback must succeed");
    assert_eq!(class, StorageClass::Warm);
}
