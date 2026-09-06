// Fail-closed Sora profile discovery regression.

#[test]
fn apply_sora_profile_leaves_discovery_disabled_without_admission() {
    let mut root = minimal_root();
    assert!(root.torii.sorafs_discovery.admission.is_none());
    assert!(!root.torii.sorafs_discovery.discovery_enabled);
    root.apply_sora_profile();
    assert!(
        !root.torii.sorafs_storage.enabled,
        "Sora profile must not manufacture an embedded storage-provider role"
    );
    assert!(
        !root.torii.sorafs_discovery.discovery_enabled,
        "discovery must remain fail-closed without an admission trust policy"
    );
}

#[test]
fn apply_sora_profile_preserves_explicit_storage_provider_role() {
    let mut root = minimal_root();
    root.torii.sorafs_storage.enabled = true;

    root.apply_sora_profile();

    assert!(
        root.torii.sorafs_storage.enabled,
        "profile geometry must not override an explicit storage-provider choice"
    );
}
