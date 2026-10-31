// Native private geometry creators preserve the reserved recovery protocol.

fn native_private_geometry_fixture(path: &Path, bytes: &[u8]) -> GeometryFileIdentity {
    let owner = iroha_fs::OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let name = path.file_name().unwrap();
    let mut original = None;
    owner
        .create_private_journal_into(name, &mut original)
        .unwrap();
    let mut file = original.take().unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    owner.require_same_lock(name, &file).unwrap();
    let metadata = secure_file_metadata::from_file(&file).unwrap();
    checked_geometry_file_identity(&metadata, path).unwrap()
}

fn native_private_geometry_identity(path: &Path) -> GeometryFileIdentity {
    let owner = iroha_fs::OwnerDirectory::open(path.parent().unwrap()).unwrap();
    let file = owner
        .open_private_journal_optional(path.file_name().unwrap())
        .unwrap()
        .unwrap();
    iroha_fs::FileSnapshot::private_journal(&file).unwrap();
    owner
        .require_same_lock(path.file_name().unwrap(), &file)
        .unwrap();
    checked_geometry_file_identity(&secure_file_metadata::from_file(&file).unwrap(), path).unwrap()
}

#[test]
fn native_configured_catalog_private_baseline_preserves_hard_link_recovery() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("private-baseline");
    let hash = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, hash).unwrap();
    let path = root.join(JOURNAL_FILE_NAME);
    let identity = native_private_geometry_identity(&path);
    let bytes = fs::read(&path).unwrap();
    fs::hard_link(&path, root.join(JOURNAL_TEMP_FILE_NAME)).unwrap();
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, hash).unwrap();
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert_eq!(native_private_geometry_identity(&path), identity);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &RuntimeLaneConfig::from_catalog(&configured),
        &configured,
    )
    .unwrap();
    assert_eq!(native_private_geometry_identity(&path), identity);
}

#[test]
fn native_configured_catalog_refuses_inherited_temp_without_promotion_or_repair() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    fs::create_dir(&root).unwrap();
    let configured = configured_primary_catalog("inherited-temp");
    let hash = LaneLifecycleParameterV1::catalog_hash(&configured);
    let bytes = LaneGeometryJournal {
        configured_catalog_hash: Some(hash),
        ..LaneGeometryJournal::default()
    }
    .encode();
    let path = root.join(JOURNAL_TEMP_FILE_NAME);
    fs::write(&path, &bytes).unwrap();
    let owner = iroha_fs::OwnerDirectory::open(&root).unwrap();
    assert!(
        owner
            .open_private_journal_optional(JOURNAL_TEMP_FILE_NAME)
            .is_err()
    );
    let before = native_observation_tree(&root);
    assert!(Kura::establish_or_verify_configured_lane_catalog_baseline(&root, hash).is_err());
    assert_eq!(native_observation_tree(&root), before);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!root.join(JOURNAL_FILE_NAME).exists());
    assert!(
        owner
            .open_private_journal_optional(JOURNAL_TEMP_FILE_NAME)
            .is_err()
    );
}

#[test]
fn native_configured_catalog_refuses_unrelated_hard_link_without_cleanup() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("unrelated-link");
    let hash = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, hash).unwrap();
    let path = root.join(JOURNAL_FILE_NAME);
    let bytes = fs::read(&path).unwrap();
    let outside = temp.path().join("unrelated-journal");
    fs::hard_link(&path, &outside).unwrap();
    let before = native_observation_tree(&root);
    assert!(Kura::establish_or_verify_configured_lane_catalog_baseline(&root, hash).is_err());
    assert_eq!(native_observation_tree(&root), before);
    assert_eq!(fs::read(&outside).unwrap(), bytes);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn native_atomic_geometry_private_create_replace_and_exact_temp_recovery() {
    let kura = Kura::blank_kura_for_testing();
    let path = kura.store_root.join("native-geometry-control.norito");
    let temp = kura.store_root.join("native-geometry-control.norito.tmp");
    kura.atomic_write_geometry_file(&path, &temp, b"first")
        .unwrap();
    let first = native_private_geometry_identity(&path);
    kura.atomic_write_geometry_file(&path, &temp, b"second value")
        .unwrap();
    assert_ne!(native_private_geometry_identity(&path), first);
    assert_eq!(fs::read(&path).unwrap(), b"second value");
    let recovered = native_private_geometry_fixture(&temp, b"recovered value");
    kura.atomic_write_geometry_file(&path, &temp, b"recovered value")
        .unwrap();
    assert_eq!(native_private_geometry_identity(&path), recovered);
    assert_eq!(fs::read(&path).unwrap(), b"recovered value");
    assert!(!temp.exists());
}

#[test]
fn native_atomic_geometry_refuses_different_temp_without_rewriting_either_original() {
    let kura = Kura::blank_kura_for_testing();
    let path = kura.store_root.join("native-geometry-collision.norito");
    let temp = kura.store_root.join("native-geometry-collision.norito.tmp");
    kura.atomic_write_geometry_file(&path, &temp, b"authoritative")
        .unwrap();
    let current = native_private_geometry_identity(&path);
    let staged = native_private_geometry_fixture(&temp, b"uncommitted");
    for _ in 0..2 {
        assert!(
            kura.atomic_write_geometry_file(&path, &temp, b"replacement")
                .is_err()
        );
        assert_eq!(native_private_geometry_identity(&path), current);
        assert_eq!(native_private_geometry_identity(&temp), staged);
        assert_eq!(fs::read(&path).unwrap(), b"authoritative");
        assert_eq!(fs::read(&temp).unwrap(), b"uncommitted");
    }
    kura.atomic_write_geometry_file(&path, &temp, b"uncommitted")
        .unwrap();
    assert_eq!(native_private_geometry_identity(&path), staged);
}

#[test]
fn native_atomic_geometry_refuses_inherited_files_and_live_writer_without_repair() {
    let kura = Kura::blank_kura_for_testing();
    let path = kura.store_root.join("native-geometry-custody.norito");
    let temp = kura.store_root.join("native-geometry-custody.norito.tmp");
    let owner = iroha_fs::OwnerDirectory::open(&kura.store_root).unwrap();
    fs::write(&temp, b"same intended bytes").unwrap();
    assert!(
        owner
            .open_private_journal_optional(temp.file_name().unwrap())
            .is_err()
    );
    assert!(
        kura.atomic_write_geometry_file(&path, &temp, b"same intended bytes")
            .is_err()
    );
    assert!(!path.exists());
    assert_eq!(fs::read(&temp).unwrap(), b"same intended bytes");
    assert!(
        owner
            .open_private_journal_optional(temp.file_name().unwrap())
            .is_err()
    );
    fs::remove_file(&temp).unwrap();
    fs::write(&path, b"inherited predecessor").unwrap();
    assert!(
        kura.atomic_write_geometry_file(&path, &temp, b"replacement")
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"inherited predecessor");
    assert!(!temp.exists());
    assert!(
        owner
            .open_private_journal_optional(path.file_name().unwrap())
            .is_err()
    );
    fs::remove_file(&path).unwrap();
    kura.atomic_write_geometry_file(&path, &temp, b"private original")
        .unwrap();
    let held = owner
        .open_private_journal_optional(path.file_name().unwrap())
        .unwrap()
        .unwrap();
    let before = iroha_fs::FileIdentity::of(&held).unwrap();
    assert!(
        kura.atomic_write_geometry_file(&path, &temp, b"replacement")
            .is_err()
    );
    owner
        .require_same_lock(path.file_name().unwrap(), &held)
        .unwrap();
    assert_eq!(iroha_fs::FileIdentity::of(&held).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), b"private original");
    assert!(!temp.exists());
    drop(held);
    kura.atomic_write_geometry_file(&path, &temp, b"replacement")
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
}
