fn storage_temp_data_dir(temp_dir: &TempDir) -> PathBuf {
    temp_dir
        .path()
        .canonicalize()
        .expect("canonical storage temp dir")
        .join("storage")
}
#[test]
fn sorafs_storage_temp_data_dir_uses_canonical_parent() {
    let temp_dir = tempdir().expect("storage temp dir");
    let data_dir = storage_temp_data_dir(&temp_dir);
    assert_eq!(
        data_dir.parent().expect("storage path parent"),
        temp_dir.path().canonicalize().expect("canonical temp dir")
    );
}

fn isolate_discovery_persistence(config: &mut actual_cfg::Torii, directory: &TempDir) {
    let root = storage_temp_data_dir(directory);
    config.data_dir = root.join("torii");
    config.sorafs_storage.data_dir = config.data_dir.join("sorafs");
    config.sorafs_discovery.replay_checkpoint_path =
        config.data_dir.join("provider-advert-replay.to");
    config.da_ingest.replay_cache_store_dir = config.data_dir.join("da_replay");
    config.da_ingest.manifest_store_dir = config.data_dir.join("da_manifests");
    config.sorafs_gc.state_dir = Some(config.sorafs_storage.data_dir.join("gc"));
    config.sorafs_por.state_dir = config.sorafs_storage.data_dir.join("por");
    config.sorafs_por.drand.state_path = config
        .sorafs_por
        .state_dir
        .join(iroha_config::parameters::defaults::sorafs::por::DRAND_STATE_FILE);
    config.sorafs_por.vrf_state_path = config
        .sorafs_por
        .state_dir
        .join(iroha_config::parameters::defaults::sorafs::por::VRF_STATE_FILE);
    if config.iso_bridge.store_dir.is_some() {
        config.iso_bridge.store_dir = Some(config.data_dir.join("iso_bridge"));
    }
    if config.iso_bridge.audit_export_dir.is_some() {
        config.iso_bridge.audit_export_dir = Some(config.data_dir.join("iso_audit"));
    }
}

#[test]
fn discovery_persistence_paths_are_canonical_isolated_and_preserve_service_flags() {
    let directories = [tempdir().unwrap(), tempdir().unwrap()];
    let mut paths = Vec::new();
    for directory in &directories {
        let mut config = iroha_torii::test_utils::mk_minimal_root_cfg().torii;
        config.iso_bridge.store_dir = Some(PathBuf::from("old-iso"));
        config.iso_bridge.audit_export_dir = Some(PathBuf::from("old-audit"));
        let enabled = (
            config.sorafs_storage.enabled,
            config.sorafs_gc.enabled,
            config.sorafs_por.enabled,
        );
        isolate_discovery_persistence(&mut config, directory);
        assert_eq!(
            enabled,
            (
                config.sorafs_storage.enabled,
                config.sorafs_gc.enabled,
                config.sorafs_por.enabled
            )
        );
        let canonical_root = directory.path().canonicalize().unwrap();
        let owned = vec![
            config.data_dir,
            config.sorafs_storage.data_dir,
            config.sorafs_discovery.replay_checkpoint_path,
            config.da_ingest.replay_cache_store_dir,
            config.da_ingest.manifest_store_dir,
            config.sorafs_gc.state_dir.unwrap(),
            config.sorafs_por.state_dir,
            config.sorafs_por.drand.state_path,
            config.sorafs_por.vrf_state_path,
            config.iso_bridge.store_dir.unwrap(),
            config.iso_bridge.audit_export_dir.unwrap(),
        ];
        for path in &owned {
            assert!(path.is_absolute());
            assert!(path.starts_with(&canonical_root), "{}", path.display());
        }
        paths.push(owned);
    }
    assert!(paths[0].iter().all(|left| !paths[1].contains(left)));
}
