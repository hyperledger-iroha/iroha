//! Actual native journal handles, retained failures and publication boundaries.

use super::*;
use std::{fs, io::Write as _, panic::AssertUnwindSafe, process::Command};

#[test]
fn journal_retirement_keeps_original_until_explicit_close() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"exact original");
    let identity = FileIdentity::of(&original).unwrap();
    let mut disposed = false;
    assert!(owner.require_retired_private_journal(&original).is_err());
    owner
        .retire_private_journal("retiring", &original, &mut disposed)
        .unwrap();
    owner.require_retired_private_journal(&original).unwrap();
    assert!(disposed);
    assert_eq!(FileIdentity::of(&original).unwrap(), identity);
    assert!(
        owner
            .retire_private_journal("retiring", &original, &mut disposed)
            .is_err()
    );
    assert!(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(owner.path().join("retiring"))
            .is_err()
    );
    drop(original);
    owner.sync().unwrap();
    assert!(!owner.path().join("retiring").exists());
}

#[test]
fn journal_retirement_preserves_first_postdisposition_error_and_original() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"exact original");
    let identity = FileIdentity::of(&original).unwrap();
    let mut disposed = false;
    let error = owner
        .retire_private_journal_observed(
            OsStr::new("retiring"),
            &original,
            &mut disposed,
            |after| {
                if after {
                    Err(io::Error::other("first after disposition"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "first after disposition");
    assert!(disposed);
    assert_eq!(FileIdentity::of(&original).unwrap(), identity);
    drop(original);
    owner.sync().unwrap();
    assert!(!owner.path().join("retiring").exists());
}

#[test]
fn journal_retirement_refuses_substitute_without_disposing_original() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"original");
    let mut disposed = false;
    assert!(
        owner
            .retire_private_journal_observed(
                OsStr::new("retiring"),
                &original,
                &mut disposed,
                |after| {
                    assert!(!after);
                    fs::rename(owner.path().join("retiring"), owner.path().join("moved"))?;
                    let replacement = create(&owner, "retiring", b"substitute");
                    drop(replacement);
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!disposed);
    assert_eq!(
        fs::read(owner.path().join("retiring")).unwrap(),
        b"substitute"
    );
    assert_eq!(fs::read(owner.path().join("moved")).unwrap(), b"original");
}

#[test]
fn journal_retirement_unwind_after_disposition_keeps_latch() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"original");
    let identity = FileIdentity::of(&original).unwrap();
    let mut disposed = false;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = owner.retire_private_journal_observed(
            OsStr::new("retiring"),
            &original,
            &mut disposed,
            |after| {
                assert!(!after, "actual post-disposition unwind");
                Ok(())
            },
        );
    }));
    assert!(result.is_err());
    assert!(disposed);
    assert_eq!(FileIdentity::of(&original).unwrap(), identity);
    drop(original);
    owner.sync().unwrap();
    assert!(!owner.path().join("retiring").exists());
}

fn root() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("journal")).unwrap();
    (temporary, owner)
}

fn create(owner: &OwnerDirectory, name: &str, bytes: &[u8]) -> File {
    let mut slot = None;
    owner.create_private_journal_into(name, &mut slot).unwrap();
    let mut file = slot.unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    owner.require_same_lock(name, &file).unwrap();
    file
}

fn grant(path: &Path, permission: &str) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", permission])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn journal_creator_keeps_postcreate_failure_and_rejects_slot_name_reuse() {
    let (_temporary, owner) = root();
    let mut slot = None;
    let mut original_id = None;
    let error = owner
        .create_private_journal_into_observed(OsStr::new("pending"), &mut slot, |mut file| {
            original_id = Some(FileIdentity::of(file)?);
            file.write_all(b"partial")?;
            Err(io::Error::other("first creator failure"))
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "first creator failure");
    let file = slot.as_ref().unwrap();
    assert_eq!(Some(FileIdentity::of(file).unwrap()), original_id);
    owner.require_same_lock("pending", file).unwrap();
    assert_eq!(fs::read(owner.path().join("pending")).unwrap(), b"partial");
    assert!(
        owner
            .create_private_journal_into("other", &mut slot)
            .is_err()
    );
    assert!(!owner.path().join("other").exists());
    let mut collision = None;
    assert_eq!(
        owner
            .create_private_journal_into("pending", &mut collision)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(collision.is_none());
}

#[test]
fn journal_creator_unwind_keeps_original_in_the_callers_slot() {
    let (_temporary, owner) = root();
    let mut slot = None;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ =
            owner.create_private_journal_into_observed(OsStr::new("pending"), &mut slot, |_| {
                panic!("creator callback unwind")
            });
    }));
    assert!(result.is_err());
    owner
        .require_same_lock("pending", slot.as_ref().unwrap())
        .unwrap();
}

#[test]
fn journal_creator_real_custody_failure_keeps_original_and_both_names() {
    let (_temporary, owner) = root();
    let mut slot = None;
    assert!(
        owner
            .create_private_journal_into_observed(OsStr::new("pending"), &mut slot, |_| {
                fs::hard_link(owner.path().join("pending"), owner.path().join("alias"))
            })
            .is_err()
    );
    let original = slot.as_ref().unwrap();
    let id = FileIdentity::of(original).unwrap();
    assert!(FileSnapshot::private_journal(original).is_err());
    for name in ["pending", "alias"] {
        let named = File::open(owner.path().join(name)).unwrap();
        assert_eq!(FileIdentity::of(&named).unwrap(), id);
    }
}

#[test]
fn journal_existing_original_excludes_writers_and_reopens_after_close() {
    let (_temporary, owner) = root();
    assert!(
        owner
            .open_private_journal_optional("missing")
            .unwrap()
            .is_none()
    );
    let writer = create(&owner, "record", b"original");
    let id = FileIdentity::of(&writer).unwrap();
    assert_eq!(
        owner
            .open_private_journal_optional("record")
            .unwrap_err()
            .raw_os_error(),
        Some(32)
    );
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(owner.path().join("record"))
            .is_err()
    );
    assert_eq!(fs::read(owner.path().join("record")).unwrap(), b"original");
    drop(writer);
    let recovered = owner
        .open_private_journal_optional("record")
        .unwrap()
        .unwrap();
    assert_eq!(FileIdentity::of(&recovered).unwrap(), id);
    owner.require_same_lock("record", &recovered).unwrap();
}

#[test]
fn journal_publication_creates_then_replaces_a_retained_predecessor() {
    let (_temporary, owner) = root();
    let first = create(&owner, "pending", b"first");
    let first_id = FileIdentity::of(&first).unwrap();
    let mut published = false;
    owner
        .publish_private_journal_observed("pending", "record", &first, None, &mut published, |_| {
            Ok(())
        })
        .unwrap();
    assert!(published);
    owner.require_same_lock("record", &first).unwrap();
    assert!(!owner.path().join("pending").exists());
    let second = create(&owner, "pending", b"second phase");
    let second_id = FileIdentity::of(&second).unwrap();
    assert_ne!(first_id, second_id);
    let mut next_published = false;
    owner
        .publish_private_journal_observed(
            "pending",
            "record",
            &second,
            Some(&first),
            &mut next_published,
            |_| Ok(()),
        )
        .unwrap();
    assert!(next_published);
    assert_eq!(FileIdentity::of(&first).unwrap(), first_id);
    assert_eq!(FileIdentity::of(&second).unwrap(), second_id);
    second.sync_all().unwrap();
    owner.sync().unwrap();
    owner.require_same_lock("record", &second).unwrap();
    assert_eq!(
        fs::read(owner.path().join("record")).unwrap(),
        b"second phase"
    );
}

#[test]
fn journal_expected_absence_and_predecessor_substitution_refuse_without_publication() {
    let (_temporary, owner) = root();
    let original = create(&owner, "record", b"same bytes");
    let pending = create(&owner, "pending", b"new phase");
    let mut published = false;
    let mut callbacks = 0;
    let error = owner
        .publish_private_journal_observed(
            "pending",
            "record",
            &pending,
            None,
            &mut published,
            |_| {
                callbacks += 1;
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(callbacks, 0);
    assert!(!published);
    fs::rename(owner.path().join("record"), owner.path().join("displaced")).unwrap();
    let replacement = create(&owner, "record", b"same bytes");
    assert!(
        owner
            .publish_private_journal_observed(
                "pending",
                "record",
                &pending,
                Some(&original),
                &mut published,
                |_| Ok(())
            )
            .is_err()
    );
    assert!(!published);
    owner.require_same_lock("record", &replacement).unwrap();
    owner.require_same_lock("pending", &pending).unwrap();
    assert_eq!(
        fs::read(owner.path().join("displaced")).unwrap(),
        b"same bytes"
    );
}

#[test]
fn journal_source_substitution_after_observer_is_detected_without_cleanup() {
    let (_temporary, owner) = root();
    let pending = create(&owner, "pending", b"same bytes");
    let mut substitute = None;
    let mut published = false;
    assert!(
        owner
            .publish_private_journal_observed(
                "pending",
                "record",
                &pending,
                None,
                &mut published,
                |step| {
                    assert_eq!(step, AtomicWriteStep::Publish);
                    fs::rename(owner.path().join("pending"), owner.path().join("displaced"))?;
                    substitute = Some(create(&owner, "pending", b"same bytes"));
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!published);
    assert!(!owner.path().join("record").exists());
    owner
        .require_same_lock("pending", substitute.as_ref().unwrap())
        .unwrap();
    assert_eq!(
        fs::read(owner.path().join("displaced")).unwrap(),
        b"same bytes"
    );
}

#[test]
fn journal_postpublication_error_latches_original_and_never_renames_twice() {
    let (_temporary, owner) = root();
    let pending = create(&owner, "pending", b"complete");
    let id = FileIdentity::of(&pending).unwrap();
    let mut published = false;
    let error = owner
        .publish_private_journal_observed(
            "pending",
            "record",
            &pending,
            None,
            &mut published,
            |step| {
                if step == AtomicWriteStep::Published {
                    Err(io::Error::other("first published failure"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "first published failure");
    assert!(published);
    assert_eq!(FileIdentity::of(&pending).unwrap(), id);
    owner.require_same_lock("record", &pending).unwrap();
    let occupant = create(&owner, "pending", b"unrelated");
    let mut callbacks = 0;
    assert!(
        owner
            .publish_private_journal_observed(
                "pending",
                "record",
                &pending,
                None,
                &mut published,
                |_| {
                    callbacks += 1;
                    Ok(())
                }
            )
            .is_err()
    );
    assert_eq!(callbacks, 0);
    pending.sync_all().unwrap();
    owner.sync().unwrap();
    owner.require_same_lock("record", &pending).unwrap();
    owner.require_same_lock("pending", &occupant).unwrap();
    assert_eq!(fs::read(owner.path().join("record")).unwrap(), b"complete");
}

#[test]
fn journal_postpublication_unwind_keeps_the_typed_latch() {
    let (_temporary, owner) = root();
    let pending = create(&owner, "pending", b"complete");
    let mut published = false;
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = owner.publish_private_journal_observed(
            "pending",
            "record",
            &pending,
            None,
            &mut published,
            |step| {
                assert_ne!(
                    step,
                    AtomicWriteStep::Published,
                    "published observer unwind"
                );
                Ok(())
            },
        );
    }));
    assert!(result.is_err());
    assert!(published);
    owner.require_same_lock("record", &pending).unwrap();
}

#[test]
fn journal_private_acl_hardlink_and_directory_leaves_refuse() {
    let (_temporary, owner) = root();
    let file = create(&owner, "shared", b"shared");
    drop(file);
    fs::hard_link(owner.path().join("shared"), owner.path().join("alias")).unwrap();
    assert!(owner.open_private_journal_optional("shared").is_err());
    fs::create_dir(owner.path().join("directory")).unwrap();
    assert!(owner.open_private_journal_optional("directory").is_err());
    let private = create(&owner, "private", b"private");
    drop(private);
    grant(&owner.path().join("private"), "*S-1-1-0:(R)");
    assert!(owner.open_private_journal_optional("private").is_err());
    assert_eq!(fs::read(owner.path().join("shared")).unwrap(), b"shared");
}

#[test]
fn journal_late_private_acl_refuses_and_preserves_the_first_observer_error() {
    let (_temporary, owner) = root();
    let pending = create(&owner, "pending", b"complete");
    let mut published = false;
    assert!(
        owner
            .publish_private_journal_observed(
                "pending",
                "record",
                &pending,
                None,
                &mut published,
                |_| {
                    grant(&owner.path().join("pending"), "*S-1-1-0:(R)");
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!published);
    assert!(!owner.path().join("record").exists());
    let second = create(&owner, "second", b"second");
    let error = owner
        .publish_private_journal_observed("second", "record", &second, None, &mut published, |_| {
            grant(owner.path(), "*S-1-1-0:(W)");
            Err(io::Error::other("first observer failure"))
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "first observer failure");
    assert!(!published);
    assert!(owner.revalidate().is_err());
}

#[test]
fn journal_retained_parent_excludes_rename_and_reparse_parent_refuses() {
    let (temporary, owner) = root();
    let pending = create(&owner, "pending", b"original");
    let moved = temporary.path().join("renamed");
    let error = fs::rename(owner.path(), &moved).unwrap_err();
    assert!(matches!(error.raw_os_error(), Some(5 | 32)));
    owner.require_same_lock("pending", &pending).unwrap();
    let alias = temporary.path().join("junction");
    let result = Command::new("cmd.exe")
        .current_dir(temporary.path())
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&alias)
        .arg(owner.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(OwnerDirectory::open(&alias).is_err());
    assert_eq!(fs::read(owner.path().join("pending")).unwrap(), b"original");
}

#[test]
fn journal_retired_original_revalidation_refuses_late_private_acl_change() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"exact original");
    let mut disposed = false;
    let error = owner
        .retire_private_journal_observed(
            OsStr::new("retiring"),
            &original,
            &mut disposed,
            |after| {
                if after {
                    Err(io::Error::other("retain delete-pending original"))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "retain delete-pending original");
    assert!(disposed);
    owner.require_retired_private_journal(&original).unwrap();
    // Parent disclosure is independently rejected even though the original is
    // still held and its namespace deletion has already been accepted.
    grant(owner.path(), "*S-1-1-0:(OI)(CI)(F)");
    assert!(owner.require_retired_private_journal(&original).is_err());
    drop(original);
}

#[test]
fn journal_retired_original_refuses_readonly_attribute_drift() {
    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"retained private original");
    let mut disposed = false;
    owner
        .retire_private_journal("retiring", &original, &mut disposed)
        .unwrap();
    assert!(disposed);
    owner.require_retired_private_journal(&original).unwrap();
    let initial = original.metadata().unwrap().permissions();
    let mut readonly = initial.clone();
    readonly.set_readonly(true);
    original.set_permissions(readonly).unwrap();
    let actually_readonly = original.metadata().unwrap().permissions().readonly();
    let refused = owner.require_retired_private_journal(&original);
    // Restore only this owned fixture before evaluating the refusal/closing it.
    original.set_permissions(initial).unwrap();
    assert!(actually_readonly);
    assert!(refused.is_err());
    owner.require_retired_private_journal(&original).unwrap();
    drop(original);
    owner.sync().unwrap();
}

#[test]
#[allow(
    unsafe_code,
    reason = "native fixture must create delete-pending multi-link state refused by the production owner"
)]
fn journal_retired_original_refuses_remaining_hardlink_after_raw_disposition() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FILE_STANDARD_INFO, FileDispositionInfo, FileStandardInfo,
        GetFileInformationByHandleEx, SetFileInformationByHandle,
    };

    let (_temporary, owner) = root();
    let original = create(&owner, "retiring", b"retained linked fixture");
    let alias = owner.path().join("alias");
    fs::hard_link(owner.path().join("retiring"), &alias).unwrap();
    // Deliberately construct a state which the production retirement admission
    // refuses. The raw operation targets only this test-owned original.
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the exact fixture handle owns DELETE and disposition has its declared layout.
    assert_ne!(
        unsafe {
            SetFileInformationByHandle(
                original.as_raw_handle(),
                FileDispositionInfo,
                std::ptr::from_ref(&disposition).cast(),
                u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>()).unwrap(),
            )
        },
        0
    );
    let mut standard = std::mem::MaybeUninit::<FILE_STANDARD_INFO>::zeroed();
    // SAFETY: the retained handle remains valid and the output has the exact native extent.
    assert_ne!(
        unsafe {
            GetFileInformationByHandleEx(
                original.as_raw_handle(),
                FileStandardInfo,
                standard.as_mut_ptr().cast(),
                u32::try_from(std::mem::size_of::<FILE_STANDARD_INFO>()).unwrap(),
            )
        },
        0
    );
    // SAFETY: the successful native query initialized the complete output.
    let standard = unsafe { standard.assume_init() };
    assert!(standard.DeletePending);
    assert_eq!(standard.NumberOfLinks, 1);
    assert!(!standard.Directory);
    assert!(owner.require_retired_private_journal(&original).is_err());
    drop(original);
    owner.sync().unwrap();
    assert_eq!(fs::read(&alias).unwrap(), b"retained linked fixture");
}

fn predecessor_metadata_authority(path: &Path) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_WRITE_ATTRIBUTES, READ_CONTROL,
        WRITE_DAC,
    };
    fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES)
        .share_mode(7)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .unwrap()
}

#[test]
fn journal_replacement_refuses_late_predecessor_acl_and_keeps_first_error() {
    for first_error in [false, true] {
        let (_temporary, owner) = root();
        let old = create(&owner, "record", b"previous");
        let old_identity = FileIdentity::of(&old).unwrap();
        let metadata = predecessor_metadata_authority(&owner.path().join("record"));
        assert_eq!(FileIdentity::of(&metadata).unwrap(), old_identity);
        let pending = create(&owner, "pending", b"next");
        let mut published = false;
        let error = owner
            .publish_private_journal_observed(
                "pending",
                "record",
                &pending,
                Some(&old),
                &mut published,
                |step| {
                    if step == AtomicWriteStep::Published {
                        crate::platform::grant_test_original_everyone(&metadata, 0x8000_0000)?;
                        if first_error {
                            return Err(io::Error::other("first displaced observation"));
                        }
                    }
                    Ok(())
                },
            )
            .unwrap_err();
        assert!(published);
        if first_error {
            assert_eq!(error.to_string(), "first displaced observation");
        }
        assert_eq!(FileIdentity::of(&old).unwrap(), old_identity);
        assert!(owner.require_retired_private_journal(&old).is_err());
        let mut bytes = [0; 8];
        assert_eq!(crate::read_at(&old, &mut bytes, 0).unwrap(), bytes.len());
        assert_eq!(&bytes, b"previous");
        owner.require_same_lock("record", &pending).unwrap();
        assert_eq!(fs::read(owner.path().join("record")).unwrap(), b"next");
    }
}

#[test]
fn journal_replacement_refuses_late_predecessor_readonly_then_revalidates_original() {
    let (_temporary, owner) = root();
    let old = create(&owner, "record", b"previous");
    let old_identity = FileIdentity::of(&old).unwrap();
    let initial = old.metadata().unwrap().permissions();
    let pending = create(&owner, "pending", b"next");
    let mut published = false;
    let result = owner.publish_private_journal_observed(
        "pending",
        "record",
        &pending,
        Some(&old),
        &mut published,
        |step| {
            if step == AtomicWriteStep::Published {
                owner.require_retired_private_journal(&old)?;
                let mut readonly = initial.clone();
                readonly.set_readonly(true);
                old.set_permissions(readonly)?;
            }
            Ok(())
        },
    );
    let readonly = old.metadata().unwrap().permissions().readonly();
    old.set_permissions(initial).unwrap();
    assert!(published);
    assert!(readonly);
    assert!(result.is_err());
    owner.require_retired_private_journal(&old).unwrap();
    assert_eq!(FileIdentity::of(&old).unwrap(), old_identity);
    owner.require_same_lock("record", &pending).unwrap();
    assert_eq!(fs::read(owner.path().join("record")).unwrap(), b"next");
}
