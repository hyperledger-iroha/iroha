//! Fixed immutable inventory, interruption, original identity and native launch controls.

use super::*;
use std::{io::Write as _, process::Command};

fn entries() -> [FlatBatchEntry; 2] {
    [
        FlatBatchEntry::new("program", FlatBatchKind::Executable, 16).unwrap(),
        FlatBatchEntry::new("manifest", FlatBatchKind::ReadOnly, 8).unwrap(),
    ]
}

fn pending(parent: &OwnerDirectory, name: &str) -> PendingFlatBatch<2> {
    let mut batch = parent.create_flat_batch(name, entries()).unwrap();
    batch
        .writer(0)
        .unwrap()
        .write_all(b"program bytes")
        .unwrap();
    batch.writer(1).unwrap().write_all(b"manifest").unwrap();
    batch
}

#[cfg(windows)]
#[test]
fn creator_parent_security_excursions_are_not_adopted_by_sealing() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let mut before_observers = pending(&owner, "before");
    let path = owner.path().join("before");
    set_owner_access(&path, "RX");
    set_owner_access(&path, "F");
    assert!(before_observers.writer(0).is_err());
    assert!(before_observers.identity().is_err());
    assert!(before_observers.entry_identity(0).is_err());
    assert!(before_observers.seal().is_err());
    for (name, restore) in [("persistent", false), ("restored", true)] {
        let batch = pending(&owner, name);
        let path = owner.path().join(name);
        let mut observed = false;
        let result = batch.seal_observed(|step| {
            if step == FlatBatchSealStep::FileSealed(0) {
                observed = true;
                set_owner_access(&path, "RX");
                if restore {
                    set_owner_access(&path, "F");
                }
            }
            Ok(())
        });
        assert!(observed);
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(path.join("program")).unwrap(),
            b"program bytes"
        );
    }
}

#[cfg(windows)]
#[test]
fn immutable_parent_and_every_child_refuse_write_delete_and_rename() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "batch").seal().unwrap();
    let identities: Vec<_> = (0..2)
        .map(|index| batch.entry_identity(index).unwrap())
        .collect();
    for (index, expected) in identities.iter().enumerate() {
        let path = batch.entry_path(index).unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        assert!(std::fs::rename(&path, owner.path().join("moved-child")).is_err());
        assert_eq!(&batch.entry_identity(index).unwrap(), expected);
    }
    assert!(std::fs::write(batch.path().join("extra"), b"unexpected").is_err());
    assert!(std::fs::rename(batch.path(), owner.path().join("moved-batch")).is_err());
    assert!(std::fs::rename(owner.path(), temporary.path().join("moved-owner")).is_err());
    batch.revalidate().unwrap();
}

#[cfg(windows)]
#[test]
fn parent_security_watch_refuses_root_only_acl_restoration() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "batch").seal().unwrap();
    let path = batch.path().to_owned();
    let children: Vec<_> = (0..2)
        .map(|index| batch.entry_snapshot(index).unwrap())
        .collect();
    // The watched root's own ACL is not a child event of its inventory watch.
    // A separate original-parent security watch observes this complete excursion.
    set_owner_access(&path, "F");
    set_owner_access(&path, "RX");
    assert!(batch.revalidate().is_err());
    let fresh = ReaderDirectory::open(owner.path())
        .unwrap()
        .open_sealed_flat_batch("batch", entries())
        .unwrap();
    for (index, original) in children.iter().enumerate() {
        assert_eq!(&fresh.entry_snapshot(index).unwrap(), original);
    }
    assert!(batch.revalidate().is_err());
}

#[test]
fn command_refuses_read_only_entries_invalid_indices_and_unsupported_images() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = std::sync::Arc::new(pending(&owner, "batch").seal().unwrap());
    assert_eq!(
        batch.command(1).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(batch.command(2).is_err());
    #[cfg(any(target_os = "linux", target_os = "android"))]
    assert_eq!(
        batch.command(0).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    assert_eq!(
        batch.command(0).unwrap_err().kind(),
        io::ErrorKind::Unsupported
    );
    batch.revalidate().unwrap();
}

#[test]
fn original_identities_bytes_positions_and_recovered_inventory_stay_bound() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let pending = pending(&owner, "batch");
    let original_directory = pending.identity().unwrap();
    let original_files: Vec<_> = (0..2)
        .map(|index| pending.entry_identity(index).unwrap())
        .collect();
    assert!(pending.entry_identity(2).is_err());
    let batch = pending.seal().unwrap();
    assert_eq!(batch.identity().unwrap(), original_directory);
    assert_eq!(batch.path(), owner.path().join("batch"));
    for (index, expected) in [b"program bytes".as_slice(), b"manifest"]
        .iter()
        .enumerate()
    {
        assert_eq!(batch.entry_identity(index).unwrap(), original_files[index]);
        let before = batch.entry_snapshot(index).unwrap();
        let mut reader = batch.reader(index).unwrap();
        let mut other = batch.reader(index).unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(&bytes, expected);
        let mut first = [0; 1];
        other.read_exact(&mut first).unwrap();
        assert_eq!(first[0], expected[0]);
        reader.seek(SeekFrom::Start(2)).unwrap();
        assert_eq!(reader.seek(SeekFrom::Current(-1)).unwrap(), 1);
        assert_eq!(
            reader.seek(SeekFrom::End(-1)).unwrap(),
            expected.len() as u64 - 1
        );
        assert!(reader.seek(SeekFrom::Start(17)).is_err());
        assert_eq!(batch.entry_snapshot(index).unwrap(), before);
    }
    assert!(batch.reader(2).is_err());
    assert!(batch.entry_path(2).is_err());
    assert!(batch.entry_snapshot(2).is_err());
    assert!(batch.entry_identity(2).is_err());
    let reader = ReaderDirectory::open(owner.path()).unwrap();
    let recovered = reader.open_sealed_flat_batch("batch", entries()).unwrap();
    assert_eq!(recovered.identity().unwrap(), original_directory);
    for (index, expected) in original_files.iter().enumerate() {
        assert_eq!(&recovered.entry_identity(index).unwrap(), expected);
    }
    assert_eq!(
        recovered.entry_snapshot(0).unwrap(),
        batch.entry_snapshot(0).unwrap()
    );
    drop(reader);
    drop(owner);
    recovered.revalidate().unwrap();
    batch.revalidate().unwrap();
}

#[test]
fn invalid_specs_and_existing_name_refuse_before_namespace_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    for name in ["", ".", "..", "a/b", "../program"] {
        assert!(FlatBatchEntry::new(name, FlatBatchKind::Executable, 1).is_err());
    }
    assert!(owner.create_flat_batch("empty", []).is_err());
    let one = FlatBatchEntry::new("same", FlatBatchKind::ReadOnly, 1).unwrap();
    assert!(
        owner
            .create_flat_batch("duplicate", [one.clone(), one])
            .is_err()
    );
    assert!(!owner.path().join("empty").exists());
    assert!(!owner.path().join("duplicate").exists());
    let batch = pending(&owner, "batch");
    assert!(owner.create_flat_batch("batch", entries()).is_err());
    batch.seal().unwrap().revalidate().unwrap();
}

#[test]
fn original_bound_includes_sparse_writes_and_flush() {
    let temporary = tempfile::tempdir().unwrap();
    let private = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let mut batch = private
        .create_flat_batch(
            "batch",
            [FlatBatchEntry::new("manifest", FlatBatchKind::ReadOnly, 4).unwrap()],
        )
        .unwrap();
    assert!(batch.writer(1).is_err());
    let mut writer = batch.writer(0).unwrap();
    assert!(writer.write_all(b"12345").is_err());
    assert!(writer.seek(SeekFrom::Start(5)).is_err());
    writer.seek(SeekFrom::Start(3)).unwrap();
    writer.write_all(b"x").unwrap();
    writer.flush().unwrap();
    assert!(writer.write_all(b"z").is_err());
    let batch = batch.seal().unwrap();
    let mut bytes = Vec::new();
    batch.reader(0).unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"\0\0\0x");
}

#[test]
fn every_observed_interruption_preserves_original_names_and_bytes() {
    let steps = [
        FlatBatchSealStep::FileSealed(0),
        FlatBatchSealStep::FileSealed(1),
        FlatBatchSealStep::DirectorySealed,
        FlatBatchSealStep::ParentTransferred,
        FlatBatchSealStep::ChildBridged(0),
        FlatBatchSealStep::ChildTransferred(0),
        FlatBatchSealStep::ChildBridged(1),
        FlatBatchSealStep::ChildTransferred(1),
        FlatBatchSealStep::Validate,
    ];
    for stop in steps {
        let temporary = tempfile::tempdir().unwrap();
        let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
        let pending = pending(&owner, "batch");
        let path = pending.directory.path().to_owned();
        let identity = FileMetadata::symlink_metadata(path.join("program"))
            .unwrap()
            .identity();
        let error = pending
            .seal_observed(|step| {
                if step == stop {
                    Err(io::Error::other("injected batch interruption"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "injected batch interruption");
        assert_eq!(
            std::fs::read(path.join("program")).unwrap(),
            b"program bytes"
        );
        assert_eq!(std::fs::read(path.join("manifest")).unwrap(), b"manifest");
        assert_eq!(
            FileMetadata::symlink_metadata(path.join("program"))
                .unwrap()
                .identity(),
            identity
        );
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 2);
    }
}

#[test]
fn recovery_refuses_changed_kind_ceiling_inventory_and_mutable_parent() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let reader = ReaderDirectory::open(owner.path()).unwrap();
    let incomplete = pending(&owner, "incomplete");
    assert!(
        reader
            .open_sealed_flat_batch("incomplete", entries())
            .is_err()
    );
    drop(incomplete);
    let batch = pending(&owner, "batch").seal().unwrap();
    let mut wrong = entries();
    wrong[0] = FlatBatchEntry::new("program", FlatBatchKind::ReadOnly, 16).unwrap();
    assert!(reader.open_sealed_flat_batch("batch", wrong).is_err());
    let mut short = entries();
    short[0] = FlatBatchEntry::new("program", FlatBatchKind::Executable, 3).unwrap();
    assert!(reader.open_sealed_flat_batch("batch", short).is_err());
    let wrong_inventory = [FlatBatchEntry::new("program", FlatBatchKind::Executable, 16).unwrap()];
    assert!(
        reader
            .open_sealed_flat_batch("batch", wrong_inventory)
            .is_err()
    );
    batch.revalidate().unwrap();
}

#[test]
fn extra_file_and_same_original_hardlink_refuse_sealing_without_cleanup() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "extra");
    let path = batch.directory.path().to_owned();
    std::fs::write(path.join("unexpected"), b"extra").unwrap();
    assert!(batch.seal().is_err());
    assert_eq!(std::fs::read(path.join("unexpected")).unwrap(), b"extra");
    let batch = pending(&owner, "linked");
    let path = batch.directory.path().to_owned();
    // The creator's private writable policy permits this adversarial NTFS/Unix
    // alias. Sealing must reject the actual shared identity without deleting it.
    let alias = owner.path().join("alias");
    std::fs::hard_link(path.join("program"), &alias).unwrap();
    assert!(batch.seal().is_err());
    assert_eq!(std::fs::read(alias).unwrap(), b"program bytes");
    assert_eq!(
        std::fs::read(path.join("program")).unwrap(),
        b"program bytes"
    );
}

#[cfg(unix)]
#[test]
fn parent_namespace_replacement_and_restored_mutation_remain_rejected() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "batch").seal().unwrap();
    let path = batch.path().to_owned();
    std::fs::rename(&path, owner.path().join("moved")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(batch.revalidate().is_err());
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(owner.path().join("moved"), &path).unwrap();
    assert!(batch.revalidate().is_err());
    let fresh = ReaderDirectory::open(owner.path())
        .unwrap()
        .open_sealed_flat_batch("batch", entries())
        .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(path.join("transient"), b"changed").unwrap();
    std::fs::remove_file(path.join("transient")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
    assert!(fresh.revalidate().is_err());
}

#[test]
fn actual_native_process_launches_with_all_five_originals_live() {
    fn require_send_sync<T: Send + Sync>() {}
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("probe.rs");
    std::fs::write(&source, b"//! Immutable batch execution fixture.\nfn main() { print!(\"immutable-flat-batch\"); std::process::exit(19); }\n").unwrap();
    let compiled = temporary
        .path()
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            "--crate-type=bin",
            "--crate-name=flat_batch_probe",
            "-C",
            "debuginfo=0",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&compiled)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let executable = std::fs::read(&compiled).unwrap();
    let entries: [FlatBatchEntry; 5] = std::array::from_fn(|index| {
        if index == 4 {
            FlatBatchEntry::new("manifest", FlatBatchKind::ReadOnly, 8).unwrap()
        } else {
            FlatBatchEntry::new(
                format!("program{index}{}", if cfg!(windows) { ".exe" } else { "" }),
                FlatBatchKind::Executable,
                executable.len(),
            )
            .unwrap()
        }
    });
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let mut pending = owner.create_flat_batch("batch", entries.clone()).unwrap();
    for index in 0..4 {
        pending
            .writer(index)
            .unwrap()
            .write_all(&executable)
            .unwrap();
    }
    pending.writer(4).unwrap().write_all(b"manifest").unwrap();
    let batch = std::sync::Arc::new(pending.seal().unwrap());
    let snapshots: Vec<_> = (0..5)
        .map(|index| batch.entry_snapshot(index).unwrap())
        .collect();
    for index in 0..4 {
        #[cfg(any(windows, target_os = "linux", target_os = "android"))]
        let result = batch.command(index).unwrap().output().unwrap();
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
        let result = Command::new(batch.entry_path(index).unwrap())
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(19));
        assert_eq!(result.stdout, b"immutable-flat-batch");
        assert!(result.stderr.is_empty());
        batch.revalidate().unwrap();
    }
    let recovered = ReaderDirectory::open(owner.path())
        .unwrap()
        .open_sealed_flat_batch("batch", entries)
        .unwrap();
    let result = Command::new(recovered.entry_path(0).unwrap())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(19));
    for (index, snapshot) in snapshots.iter().enumerate() {
        assert_eq!(&batch.entry_snapshot(index).unwrap(), snapshot);
        assert_eq!(&recovered.entry_snapshot(index).unwrap(), snapshot);
    }
    let mut bytes = Vec::new();
    recovered
        .reader(0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, executable);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let command = batch.command(0).unwrap();
        let retained_command = batch.command(0).unwrap();
        let named_parent = owner.path().to_owned();
        let moved_parent = temporary.path().join("moved-owner");
        std::fs::rename(&named_parent, &moved_parent).unwrap();
        std::fs::create_dir(&named_parent).unwrap();
        assert!(batch.revalidate().is_err());
        // This command still selects the exact original even when a mutable
        // observational ancestor path no longer names its retained owner.
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(19));
        assert_eq!(result.stdout, b"immutable-flat-batch");
        std::fs::remove_dir(&named_parent).unwrap();
        std::fs::rename(&moved_parent, &named_parent).unwrap();
        batch.revalidate().unwrap();
        let weak = std::sync::Arc::downgrade(&batch);
        drop(batch);
        assert!(weak.upgrade().is_some());
        assert_eq!(retained_command.output().unwrap().status.code(), Some(19));
        assert!(weak.upgrade().is_none());
    }
    require_send_sync::<SealedFlatBatch<5>>();
}

#[cfg(windows)]
fn set_owner_access(path: &Path, access: &str) {
    let identity = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .unwrap();
    assert!(identity.status.success());
    let identity = String::from_utf8_lossy(&identity.stdout);
    let sid = identity
        .rsplit(',')
        .next()
        .unwrap()
        .trim()
        .trim_matches('"');
    assert!(
        sid.starts_with("S-1-")
            && sid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
    );
    let result = Command::new("icacls")
        .arg(path)
        .arg("/grant:r")
        .arg(format!("*{sid}:{access}"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(windows)]
#[test]
fn native_watch_remains_latched_after_exact_acl_and_namespace_restoration() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "batch").seal().unwrap();
    let path = batch.path().to_owned();
    assert!(std::fs::rename(&path, owner.path().join("moved")).is_err());
    batch.revalidate().unwrap();
    set_owner_access(&path, "F");
    std::fs::write(path.join("transient"), b"mutated").unwrap();
    std::fs::remove_file(path.join("transient")).unwrap();
    set_owner_access(&path, "RX");
    assert!(batch.revalidate().is_err());
    assert!(batch.revalidate().is_err());
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 2);
    let fresh = ReaderDirectory::open(owner.path())
        .unwrap()
        .open_sealed_flat_batch("batch", entries())
        .unwrap();
    fresh.revalidate().unwrap();
}

#[cfg(windows)]
#[test]
fn foreign_grant_refuses_recovery_without_repair() {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("owner")).unwrap();
    let batch = pending(&owner, "batch").seal().unwrap();
    let path = batch.path().to_owned();
    drop(batch);
    let result = Command::new("icacls")
        .arg(path.join("program"))
        .arg("/grant")
        .arg("*S-1-1-0:(R)")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reader = ReaderDirectory::open(owner.path()).unwrap();
    assert!(reader.open_sealed_flat_batch("batch", entries()).is_err());
    assert_eq!(
        std::fs::read(path.join("program")).unwrap(),
        b"program bytes"
    );
    let original = File::open(path.join("program")).unwrap();
    assert!(FileSnapshot::private_journal(&original).is_err());
}
