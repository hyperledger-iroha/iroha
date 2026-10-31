//! Real anonymous-original sealing, binding and refusal controls.

use super::*;
use crate::{FlatBatchEntry, PrivateDirectory};
use std::path::{Path, PathBuf};
use std::{fs, os::fd::AsRawFd as _, os::unix::fs::FileExt as _};

fn batch(root: &Path, kind: FlatBatchKind, bytes: &[u8]) -> Arc<SealedFlatBatch<1>> {
    let parent = PrivateDirectory::open_or_create(root.join("private")).unwrap();
    let mut pending = parent
        .create_flat_batch(
            "batch",
            [FlatBatchEntry::new("tool", kind, bytes.len()).unwrap()],
        )
        .unwrap();
    pending.writer(0).unwrap().write_all(bytes).unwrap();
    Arc::new(pending.seal().unwrap())
}

fn native_batch(root: &Path) -> Arc<SealedFlatBatch<1>> {
    batch(
        root,
        FlatBatchKind::Executable,
        &fs::read(std::env::current_exe().unwrap()).unwrap(),
    )
}

#[test]
fn sealed_tool_binding_refuses_invalid_names_indices_and_read_only_entries() {
    let temporary = tempfile::tempdir().unwrap();
    let readonly = batch(temporary.path(), FlatBatchKind::ReadOnly, b"data");
    assert!(readonly.sealed_tool_image(1).is_err());
    assert!(readonly.sealed_tool_image(0).is_err());
    readonly.revalidate().unwrap();
    let native = tempfile::tempdir().unwrap();
    let source = native_batch(native.path());
    let image = source.sealed_tool_image(0).unwrap();
    let mut command = crate::ProcessCommand::new(std::env::current_exe().unwrap()).unwrap();
    for name in ["", "A=B", "A\0B"] {
        assert!(command.env_sealed_tool_image(name, &image).is_err());
    }
    assert_eq!(command.get_envs().count(), 0);
    image.revalidate().unwrap();
}

#[test]
fn sealed_tool_binding_holds_exact_duplicate_cloexec_until_command_release() {
    let temporary = tempfile::tempdir().unwrap();
    let source = native_batch(temporary.path());
    let image = source.sealed_tool_image(0).unwrap();
    let identity = image.identity().unwrap();
    let weak_source = Arc::downgrade(&source);
    let weak_image = Arc::downgrade(&image);
    let mut command = image.command().unwrap();
    command.env_sealed_tool_image("TOOL", &image).unwrap();
    let path = command
        .get_envs()
        .find(|(key, _)| *key == "TOOL")
        .and_then(|(_, value)| value)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let descriptor = path.strip_prefix("/proc/self/fd/").unwrap();
    let information = fs::read_to_string(format!("/proc/self/fdinfo/{descriptor}")).unwrap();
    let flags = information
        .lines()
        .find_map(|line| line.strip_prefix("flags:\t"))
        .unwrap();
    assert_ne!(u32::from_str_radix(flags, 8).unwrap() & 0o2_000_000, 0);
    assert_eq!(
        FileIdentity::of(&File::open(&path).unwrap()).unwrap(),
        identity
    );
    assert_ne!(identity, source.entry_identity(0).unwrap());
    drop(source);
    drop(image);
    assert!(weak_source.upgrade().is_some());
    assert!(weak_image.upgrade().is_some());
    drop(command);
    assert!(weak_source.upgrade().is_none());
    assert!(weak_image.upgrade().is_none());
    assert!(!Path::new(&path).exists());
}

#[test]
fn executable_memory_original_refuses_writes_extents_mode_and_rename_before_launch() {
    let temporary = tempfile::tempdir().unwrap();
    let source = native_batch(temporary.path());
    let image = source.sealed_tool_image(0).unwrap();
    assert_eq!(
        rustix::fs::fcntl_get_seals(&image.file).unwrap(),
        required_seals()
    );
    assert_eq!(crate::FileMetadata::of(&image.file).unwrap().links(), 0);
    let path = PathBuf::from(format!("/proc/self/fd/{}", image.file.as_raw_fd()));
    let name = fs::read_link(&path).unwrap();
    assert!(name.to_string_lossy().contains("memfd:iroha-retained-tool"));
    assert_ne!(name.file_stem().unwrap(), "cargo");
    assert!(fs::rename(&path, temporary.path().join("cargo")).is_err());
    assert!(fs::hard_link(&path, temporary.path().join("cargo")).is_err());
    assert!(
        rustix::fs::linkat(
            &image.file,
            "",
            rustix::fs::CWD,
            temporary.path().join("cargo"),
            rustix::fs::AtFlags::EMPTY_PATH
        )
        .is_err()
    );
    assert!(!temporary.path().join("cargo").exists());
    assert_eq!(
        image.file.set_len(0).unwrap_err().raw_os_error(),
        Some(rustix::io::Errno::PERM.raw_os_error())
    );
    assert_eq!(
        image
            .file
            .set_len(image.before.length + 1)
            .unwrap_err()
            .raw_os_error(),
        Some(rustix::io::Errno::PERM.raw_os_error())
    );
    assert_eq!(
        rustix::fs::fchmod(&image.file, Mode::RUSR).unwrap_err(),
        rustix::io::Errno::PERM
    );
    assert_eq!(
        rustix::fs::fcntl_add_seals(&image.file, SealFlags::WRITE).unwrap_err(),
        rustix::io::Errno::PERM
    );
    let mut actual = vec![0; usize::try_from(image.before.length).unwrap()];
    crate::read_exact_at(&image.file, &mut actual, 0).unwrap();
    let mut expected = Vec::new();
    source
        .reader(0)
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    assert_eq!(actual, expected);
    image.revalidate().unwrap();

    // shmem may update mtime/ctime before write_begin refuses a sealed pwrite.
    // Keep the other refusal checks on their original, unmodified snapshot.
    let write_image = source.sealed_tool_image(0).unwrap();
    let before_write = snapshot(&write_image.file).unwrap();
    assert_eq!(before_write, write_image.before);
    assert_eq!(
        write_image
            .file
            .write_at(b"changed", 0)
            .unwrap_err()
            .raw_os_error(),
        Some(rustix::io::Errno::PERM.raw_os_error())
    );
    let after_write = snapshot(&write_image.file).unwrap();
    eprintln!("denied sealed pwrite: before={before_write:?}; after={after_write:?}");
    // No identity, extent, permission, ownership or seal change is permitted.
    assert_eq!(
        Snapshot {
            modified: before_write.modified,
            changed: before_write.changed,
            ..after_write
        },
        before_write
    );
    crate::read_exact_at(&write_image.file, &mut actual, 0).unwrap();
    assert_eq!(actual, expected);
    if after_write == before_write {
        write_image.revalidate().unwrap();
        drop(write_image.command().unwrap());
    } else {
        let error = write_image.revalidate().unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("sealed executable memory original changed")
        );
        assert!(write_image.command().is_err());
    }
    source.revalidate().unwrap();

    // Independently require metadata-drift refusal without depending on whether
    // this kernel's denied pwrite happened within the same timestamp quantum.
    let changed_image = source.sealed_tool_image(0).unwrap();
    let before_change = snapshot(&changed_image.file).unwrap();
    let modified = changed_image
        .file
        .metadata()
        .unwrap()
        .modified()
        .unwrap()
        .checked_add(std::time::Duration::from_secs(1))
        .unwrap();
    changed_image
        .file
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let after_change = snapshot(&changed_image.file).unwrap();
    eprintln!("explicit sealed timestamp drift: before={before_change:?}; after={after_change:?}");
    assert_ne!(after_change.modified, before_change.modified);
    assert_eq!(
        Snapshot {
            modified: before_change.modified,
            changed: before_change.changed,
            ..after_change
        },
        before_change
    );
    crate::read_exact_at(&changed_image.file, &mut actual, 0).unwrap();
    assert_eq!(actual, expected);
    let error = changed_image.revalidate().unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        error
            .to_string()
            .contains("sealed executable memory original changed")
    );
    assert!(changed_image.command().is_err());
    source.revalidate().unwrap();
}

#[test]
fn executable_memory_copy_refuses_non_elf_and_preserves_first_error_and_source() {
    let temporary = tempfile::tempdir().unwrap();
    let source = batch(temporary.path(), FlatBatchKind::Executable, b"not an elf");
    assert!(
        source
            .sealed_tool_image(0)
            .unwrap_err()
            .to_string()
            .contains("ELF")
    );
    source.revalidate().unwrap();
    for selected in [Step::Copied, Step::Sealed, Step::Compared] {
        let native = tempfile::tempdir().unwrap();
        let source = native_batch(native.path());
        let before = source.entry_snapshot(0).unwrap();
        let error = SealedToolImage::copy_observed(&source, 0, |step| {
            if step == selected {
                Err(io::Error::other("first memory image error"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "first memory image error");
        assert_eq!(source.entry_snapshot(0).unwrap(), before);
        source.revalidate().unwrap();
    }
}

#[test]
fn executable_memory_failure_still_observes_changed_source_without_replacing_first_error() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = tempfile::tempdir().unwrap();
    let source = native_batch(temporary.path());
    let path = source.entry_path(0).unwrap();
    let error = SealedToolImage::copy_observed(&source, 0, |step| {
        if step == Step::Compared {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
            Err(io::Error::other("first comparison error"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "first comparison error");
    assert!(source.revalidate().is_err());
}

#[test]
fn executable_memory_policy_and_missing_seals_refuse_before_binding() {
    let temporary = tempfile::tempdir().unwrap();
    let source = native_batch(temporary.path());
    let admitted = source.sealed_tool_image(0).unwrap();
    let mut file = File::from(
        rustix::fs::memfd_create(
            "unsealed-control",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
        )
        .unwrap(),
    );
    file.write_all(b"\x7fELFunsealed").unwrap();
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::XUSR).unwrap();
    let unsealed = Arc::new(SealedToolImage {
        source: Arc::clone(&source),
        file,
        before: admitted.before,
    });
    assert!(
        unsealed
            .revalidate()
            .unwrap_err()
            .to_string()
            .contains("kernel seal")
    );
    assert!(unsealed.command().is_err());
    let mut command = crate::ProcessCommand::new(std::env::current_exe().unwrap()).unwrap();
    assert!(command.env_sealed_tool_image("TOOL", &unsealed).is_err());
    assert_eq!(command.get_envs().count(), 0);

    let noexec = File::from(
        rustix::fs::memfd_create(
            "noexec-control",
            MemfdFlags::CLOEXEC | MemfdFlags::NOEXEC_SEAL,
        )
        .unwrap(),
    );
    assert_eq!(
        rustix::fs::fchmod(&noexec, Mode::RUSR | Mode::XUSR).unwrap_err(),
        rustix::io::Errno::PERM
    );
    assert!(snapshot(&noexec).is_err());
    admitted.revalidate().unwrap();
}
