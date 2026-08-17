// Fail-closed Sora profile discovery regression.

#[test]
fn apply_sora_profile_leaves_discovery_disabled_without_admission() {
    let mut root = minimal_root();
    assert!(root.torii.sorafs_discovery.admission.is_none());
    assert!(!root.torii.sorafs_discovery.discovery_enabled);
    root.apply_sora_profile();
    assert!(root.nexus.enabled, "Sora profile must still enable Nexus");
    assert!(
        root.torii.sorafs_storage.enabled,
        "Sora profile must still enable SoraFS storage"
    );
    assert!(
        !root.torii.sorafs_discovery.discovery_enabled,
        "discovery must remain fail-closed without an admission trust policy"
    );
}
