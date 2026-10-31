//! Native Windows slot and initialization-lock ACL custody regressions.

use super::*;

fn fixture() -> (tempfile::TempDir, RootedDirectory, TwoSlotStoreConfigV1) {
    let temporary = tempfile::tempdir().unwrap();
    let root = RootedDirectory::open_root(temporary.path(), true).unwrap();
    let config = TwoSlotStoreConfigV1::try_new("acl-store", [0x31; 32], [0x42; 32], 512).unwrap();
    (temporary, root, config)
}

fn change_everyone_read_grant(path: &Path, grant: bool) {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .arg(if grant { "/grant" } else { "/remove:g" })
        .arg(if grant { "*S-1-1-0:R" } else { "*S-1-1-0" })
        .status()
        .unwrap();
    assert!(status.success(), "native owner ACL mutation must succeed");
}

#[test]
fn windows_two_slot_new_files_have_canonical_private_acl_and_reopen_exactly() {
    let (_temporary, root, config) = fixture();
    root.sync_all().expect("native retained directory sync");
    let mut stages = Vec::new();
    let store = root
        .open_or_create_two_slot_store_v1_with_init_hook(config.clone(), b"first", |stage| {
            stages.push(stage);
            Ok(())
        })
        .unwrap_or_else(|error| panic!("native initialization failed after {stages:?}: {error:?}"));
    for slot in &store.slots {
        assert_eq!(
            platform::private_storage_acl_snapshot(
                &slot.handle,
                &store.directory.display_path.join(&slot.name)
            )
            .unwrap(),
            slot.private_acl,
        );
    }
    let lease = store.try_acquire_bound_operation_lease(&root).unwrap();
    lease.verify().unwrap();
    lease.release().unwrap();
    let first = store.load().unwrap();
    let second = store.compare_and_swap(&first, b"second").unwrap();
    assert_eq!(second.generation(), 2);
    assert_eq!(second.payload(), b"second");
    drop(store);
    let reader = RootedDirectory::open_root(&root.display_path, false).unwrap();
    assert_eq!(
        reader.load_existing_two_slot_store_v1(config).unwrap(),
        second
    );
}

#[test]
fn windows_two_slot_rejects_read_only_acl_grants_on_both_original_slots() {
    for slot_id in 0..2 {
        let (_temporary, root, config) = fixture();
        let store = root
            .open_or_create_two_slot_store_v1(config, b"first")
            .unwrap();
        let before = store.load().unwrap();
        let path = store
            .directory
            .display_path
            .join(&store.slots[slot_id].name);
        change_everyone_read_grant(&path, true);
        let load = store.load();
        let write = store.compare_and_swap(&before, b"second");
        change_everyone_read_grant(&path, false);
        assert_eq!(load.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(write.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        let reader = RootedDirectory::open_root(&root.display_path, false).unwrap();
        assert_eq!(
            reader
                .load_existing_two_slot_store_v1(store.config.clone())
                .unwrap(),
            before,
            "refused CAS cannot change the lineage"
        );
    }
}

#[test]
fn windows_two_slot_init_lock_rechecks_original_acl_for_verify_release_and_reopen() {
    let (_temporary, root, config) = fixture();
    let store = root
        .open_or_create_two_slot_store_v1(config.clone(), b"first")
        .unwrap();
    let lease = store.try_acquire_bound_operation_lease(&root).unwrap();
    let path = root.display_path.join(two_slot_init_lock_name(&config));
    change_everyone_read_grant(&path, true);
    let verify = lease.verify();
    let release = lease.release();
    let reopen = store.try_acquire_bound_operation_lease(&root);
    change_everyone_read_grant(&path, false);
    assert_eq!(verify.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(release.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert!(
        matches!(reopen, Err(TwoSlotTryErrorV1::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied)
    );
    store
        .try_acquire_bound_operation_lease(&root)
        .unwrap()
        .release()
        .unwrap();
}

#[test]
fn windows_two_slot_rejects_existing_inherited_init_lock_without_repair() {
    let (_temporary, root, config) = fixture();
    let path = root.display_path.join(two_slot_init_lock_name(&config));
    fs::File::create(&path).unwrap();
    let before = fs::read(&path).unwrap();
    let error = root
        .open_or_create_two_slot_store_v1(config.clone(), b"first")
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!root.display_path.join(&config.store_name).exists());
}

#[test]
fn windows_two_slot_rechecks_both_acl_snapshots_at_every_cas_boundary() {
    let boundaries = [
        "inactive-zero-trailer-written",
        "inactive-trailer-invalidated",
        "inactive-record-written",
        "inactive-record-synced",
        "inactive-commit-trailer-written",
        "inactive-commit-trailer-synced",
        "successor-readback-verified",
    ];
    for boundary in boundaries {
        for slot_id in 0..2 {
            let (_temporary, root, config) = fixture();
            let store = root
                .open_or_create_two_slot_store_v1(config, b"first")
                .unwrap();
            let before = store.load().unwrap();
            let path = store
                .directory
                .display_path
                .join(&store.slots[slot_id].name);
            let mut mutated = false;
            let result = store.compare_and_swap_with_test_hook(&before, b"second", |step| {
                if step == boundary {
                    change_everyone_read_grant(&path, true);
                    mutated = true;
                }
                Ok(())
            });
            assert!(mutated, "CAS must reach {boundary}");
            change_everyone_read_grant(&path, false);
            assert_eq!(
                result.unwrap_err().kind(),
                io::ErrorKind::PermissionDenied,
                "slot {slot_id} ACL drift at {boundary} cannot return success"
            );
        }
    }
}

#[test]
fn windows_two_slot_rejects_junction_directory_without_symlink_privilege() {
    let (temporary, root, config) = fixture();
    let target = temporary.path().join("outside");
    fs::create_dir(&target).unwrap();
    let linked = temporary.path().join(&config.store_name);
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&linked)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction creation failed: {output:?}"
    );
    let result = root.open_or_create_two_slot_store_v1(config.clone(), b"first");
    let opened = root.open_directory(OsStr::new(&config.store_name));
    fs::remove_dir(&linked).unwrap();
    assert!(
        result.is_err(),
        "junction cannot become the canonical slot directory"
    );
    assert!(
        opened.is_err(),
        "retained directory open cannot follow a junction"
    );
    assert_eq!(
        fs::read_dir(target).unwrap().count(),
        0,
        "external target is untouched"
    );
}
