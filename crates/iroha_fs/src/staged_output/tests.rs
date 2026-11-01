//! Actual Windows original staging, replay and publication failure boundaries.

use std::{fs, process::Command};

use super::*;

fn fixture() -> (tempfile::TempDir, OwnerDirectory) {
    let temporary = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(temporary.path().join("outputs")).unwrap();
    (temporary, owner)
}

fn grant(path: &Path, rights: &str) {
    let status = Command::new("icacls")
        .arg(path)
        .arg("/grant")
        .arg(format!("*S-1-1-0:{rights}"))
        .status()
        .unwrap();
    assert!(status.success(), "actual native ACL mutation must succeed");
}

#[test]
fn native_stage_replays_exact_original_and_reanchors_mutable_cursor() {
    let (_temporary, owner) = fixture();
    let mut stage = owner.stage_output("payload").unwrap();
    stage.write_all(b"abcdef").unwrap();
    stage.flush().unwrap();
    let before = stage.inner.identity().unwrap();
    {
        let mut first = stage.reader().unwrap();
        let mut second = stage.reader().unwrap();
        let mut bytes = [0; 2];
        first.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        second.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        first.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"cd");
    }
    stage.write_all(b"gh").unwrap();
    assert_eq!(stage.stream_position().unwrap(), 8);
    stage.seek(SeekFrom::Start(1)).unwrap();
    stage.write_all(b"Z").unwrap();
    stage.rewind().unwrap();
    let mut bytes = Vec::new();
    stage.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"aZcdefgh");
    assert_eq!(stage.inner.identity().unwrap(), before);
    let mut replay = Vec::new();
    stage.reader().unwrap().read_to_end(&mut replay).unwrap();
    assert_eq!(replay, bytes);
}

#[test]
fn native_stage_keeps_private_file_and_ancestry_after_parent_owner_drops() {
    let (temporary, owner) = fixture();
    let root = owner.path().to_owned();
    grant(&root, "(R)");
    let mut stage = owner.stage_output("payload").unwrap();
    let path = stage.path().to_owned();
    drop(owner);
    stage.write_all(b"retained").unwrap();
    stage.revalidate().unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::remove_file(&path).is_err());
    assert!(fs::rename(&path, root.join("substitute")).is_err());
    assert!(fs::rename(&root, temporary.path().join("moved")).is_err());
    drop(stage);
    assert!(!path.exists());
    fs::rename(root, temporary.path().join("moved")).unwrap();
}

#[test]
fn native_stage_creation_collisions_are_bounded_and_never_adopt_existing_bytes() {
    let (_temporary, owner) = fixture();
    let occupied = owner.path().join("occupied");
    fs::write(&occupied, b"original").unwrap();
    let mut attempts = 0;
    let error = owner
        .stage_output_with_names(OsStr::new("payload"), || {
            attempts += 1;
            "occupied".into()
        })
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(attempts, CREATE_ATTEMPTS);
    assert_eq!(fs::read(&occupied).unwrap(), b"original");
    assert_eq!(fs::read_dir(owner.path()).unwrap().count(), 1);
    for name in ["", ".", "..", "nested/file", "nested\\file", "C:\\output"] {
        assert!(owner.stage_output(name).is_err());
    }
}

#[test]
fn native_stage_postcreate_error_and_unwind_dispose_only_the_created_original() {
    let (_temporary, owner) = fixture();
    let occupied = owner.path().join("occupied");
    fs::write(&occupied, b"keep").unwrap();
    let error = owner
        .inner
        .create_output_stage_observed(OsStr::new("refused"), |_| {
            Err(io::Error::other("first creation observation"))
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "first creation observation");
    assert!(!owner.path().join("refused").exists());
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner
            .inner
            .create_output_stage_observed(OsStr::new("unwound"), |_| {
                panic!("after actual create-new")
            });
    }));
    assert!(unwind.is_err());
    assert!(!owner.path().join("unwound").exists());
    assert_eq!(fs::read(occupied).unwrap(), b"keep");
}

#[test]
fn native_stage_publication_keeps_old_reader_and_private_complete_replacement() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    fs::write(&destination, b"previous").unwrap();
    grant(&destination, "(R)");
    let mut old = fs::File::open(&destination).unwrap();
    let mut stage = owner.stage_output("payload").unwrap();
    let temporary = stage.path().to_owned();
    stage.write_all(b"complete new payload").unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"previous");
    stage.publish().unwrap();
    assert!(!temporary.exists());
    assert_eq!(
        crate::read_private(&destination, 64).unwrap().as_slice(),
        b"complete new payload"
    );
    let mut old_bytes = Vec::new();
    old.read_to_end(&mut old_bytes).unwrap();
    assert_eq!(old_bytes, b"previous");
}

#[test]
fn native_stage_refuses_hardlink_unsafe_acl_writer_and_directory_destinations() {
    let (_temporary, owner) = fixture();
    for kind in ["hardlink", "unsafe", "writer", "directory"] {
        let destination = owner.path().join(kind);
        if kind == "directory" {
            fs::create_dir(&destination).unwrap();
        } else {
            fs::write(&destination, b"preserved").unwrap();
        }
        if kind == "hardlink" {
            fs::hard_link(&destination, owner.path().join("alias")).unwrap();
        }
        if kind == "unsafe" {
            grant(&destination, "(W)");
        }
        let writer = (kind == "writer").then(|| {
            fs::OpenOptions::new()
                .write(true)
                .open(&destination)
                .unwrap()
        });
        let mut stage = owner.stage_output(kind).unwrap();
        let temporary = stage.path().to_owned();
        stage.write_all(b"rejected replacement").unwrap();
        assert!(stage.publish().is_err());
        drop(writer);
        assert!(!temporary.exists());
        if kind == "directory" {
            assert!(destination.is_dir());
        } else {
            assert_eq!(fs::read(destination).unwrap(), b"preserved");
        }
    }
}

#[test]
fn native_stage_refuses_junction_parent_and_destination_without_outside_mutation() {
    let (temporary, owner) = fixture();
    let outside = temporary.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), b"outside").unwrap();
    let junction = owner.path().join("junction");
    assert!(
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .unwrap()
            .success()
    );
    assert!(OwnerDirectory::open(&junction).is_err());
    let mut stage = owner.stage_output("junction").unwrap();
    let path = stage.path().to_owned();
    stage.write_all(b"refused").unwrap();
    assert!(stage.publish().is_err());
    assert!(!path.exists());
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"outside");
    fs::remove_dir(junction).unwrap();
}

#[test]
fn native_stage_before_publication_error_preserves_destination_and_cleans_stage() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    fs::write(&destination, b"old").unwrap();
    let mut stage = owner.stage_output("payload").unwrap();
    let path = stage.path().to_owned();
    stage.write_all(b"verified").unwrap();
    let error = stage
        .publish_observed(|step| {
            if step == AtomicWriteStep::Publish {
                Err(io::Error::other("refuse before rename"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "refuse before rename");
    assert!(!stage.published);
    drop(stage);
    assert!(!path.exists());
    assert_eq!(fs::read(destination).unwrap(), b"old");
}

#[test]
fn native_stage_detects_predecessor_substitution_and_new_occupied_absence() {
    let (_temporary, owner) = fixture();
    for initially_present in [false, true] {
        let name = if initially_present {
            "present"
        } else {
            "absent"
        };
        let destination = owner.path().join(name);
        let moved = owner.path().join("prior");
        if initially_present {
            fs::write(&destination, b"old").unwrap();
        }
        let mut stage = owner.stage_output(name).unwrap();
        let path = stage.path().to_owned();
        stage.write_all(b"new").unwrap();
        assert!(
            stage
                .publish_observed(|step| {
                    if step == AtomicWriteStep::Publish {
                        if initially_present {
                            fs::rename(&destination, &moved).unwrap();
                        }
                        fs::write(&destination, b"competing").unwrap();
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(!stage.published);
        drop(stage);
        assert!(!path.exists());
        assert_eq!(fs::read(destination).unwrap(), b"competing");
        if initially_present {
            assert_eq!(fs::read(&moved).unwrap(), b"old");
        }
    }
}

#[test]
fn native_stage_postpublication_failure_latches_visibility_and_preserves_first_error() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    fs::write(&destination, b"old").unwrap();
    let mut stage = owner.stage_output("payload").unwrap();
    let path = stage.path().to_owned();
    stage.write_all(b"complete").unwrap();
    let error = stage
        .publish_observed(|step| {
            if step == AtomicWriteStep::Published {
                grant(&destination, "(R)");
                Err(io::Error::other("first postpublication error"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "first postpublication error");
    assert!(stage.published);
    assert!(
        stage
            .publish_observed(|_| panic!("must not rename again"))
            .is_err()
    );
    drop(stage);
    assert!(!path.exists());
    assert_eq!(fs::read(destination).unwrap(), b"complete");
}

#[test]
fn native_stage_postpublication_unwind_never_discards_visible_destination() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    let mut stage = owner.stage_output("payload").unwrap();
    let path = stage.path().to_owned();
    stage.write_all(b"complete").unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = stage.publish_observed(|step| {
            assert!(
                step != AtomicWriteStep::Published,
                "after actual native rename"
            );
            Ok(())
        });
    }));
    assert!(unwind.is_err());
    assert!(stage.published);
    drop(stage);
    assert!(!path.exists());
    assert_eq!(fs::read(destination).unwrap(), b"complete");
}

#[test]
fn native_stage_replay_refuses_late_private_acl_and_drop_never_touches_destination() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    fs::write(&destination, b"prior").unwrap();
    let mut stage = owner.stage_output("payload").unwrap();
    stage.write_all(b"not admitted").unwrap();
    let path = stage.path().to_owned();
    {
        let mut replay = stage.reader().unwrap();
        grant(&path, "(R)");
        assert!(replay.read(&mut [0; 1]).is_err());
    }
    assert!(stage.write_all(b"more").is_err());
    drop(stage);
    assert!(!path.exists());
    assert_eq!(fs::read(destination).unwrap(), b"prior");
}

#[test]
fn native_stage_replay_rejects_actual_extent_change_against_initial_snapshot() {
    let (_temporary, owner) = fixture();
    let mut stage = owner.stage_output("payload").unwrap();
    stage.write_all(b"original extent").unwrap();
    let identity = stage.inner.identity().unwrap();
    {
        let mut replay = stage.reader().unwrap();
        // The test reaches the private owner solely to mutate the real original.
        // Public callers receive neither this handle nor concurrent writer authority.
        stage.inner.file().set_len(1).unwrap();
        assert_eq!(stage.inner.identity().unwrap(), identity);
        assert!(replay.read(&mut [0; 8]).is_err());
    }
    let path = stage.path().to_owned();
    drop(stage);
    assert!(!path.exists());
    assert!(!owner.path().join("payload").exists());
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
fn native_stage_refuses_late_displaced_public_acl_and_preserves_first_error() {
    for first_error in [false, true] {
        let (_temporary, owner) = fixture();
        let destination = owner.path().join("payload");
        fs::write(&destination, b"previous").unwrap();
        grant(&destination, "(R)");
        let metadata = predecessor_metadata_authority(&destination);
        let mut old = fs::File::open(&destination).unwrap();
        let old_identity = crate::FileIdentity::of(&old).unwrap();
        assert_eq!(crate::FileIdentity::of(&metadata).unwrap(), old_identity);
        let mut stage = owner.stage_output("payload").unwrap();
        stage.write_all(b"next").unwrap();
        let temporary = stage.path().to_owned();
        let error = stage
            .publish_observed(|step| {
                if step == AtomicWriteStep::Published {
                    crate::platform::grant_test_original_everyone(&metadata, 0x4000_0000)?;
                    if first_error {
                        return Err(io::Error::other("first displaced output observation"));
                    }
                }
                Ok(())
            })
            .unwrap_err();
        assert!(stage.published);
        if first_error {
            assert_eq!(error.to_string(), "first displaced output observation");
        }
        assert_eq!(crate::FileIdentity::of(&old).unwrap(), old_identity);
        let mut bytes = Vec::new();
        old.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"previous");
        stage.revalidate().unwrap();
        drop(stage);
        assert!(!temporary.exists());
        assert_eq!(
            crate::read_private(&destination, 64).unwrap().as_slice(),
            b"next"
        );
    }
}

#[test]
fn native_stage_refuses_late_displaced_readonly_without_deleting_replacement() {
    let (_temporary, owner) = fixture();
    let destination = owner.path().join("payload");
    fs::write(&destination, b"previous").unwrap();
    grant(&destination, "(R)");
    let metadata = predecessor_metadata_authority(&destination);
    let old_identity = crate::FileIdentity::of(&metadata).unwrap();
    let initial = metadata.metadata().unwrap().permissions();
    let mut stage = owner.stage_output("payload").unwrap();
    stage.write_all(b"next").unwrap();
    let result = stage.publish_observed(|step| {
        if step == AtomicWriteStep::Published {
            let mut readonly = initial.clone();
            readonly.set_readonly(true);
            metadata.set_permissions(readonly)?;
        }
        Ok(())
    });
    let readonly = metadata.metadata().unwrap().permissions().readonly();
    metadata.set_permissions(initial).unwrap();
    assert!(stage.published);
    assert!(readonly);
    assert!(result.is_err());
    assert_eq!(crate::FileIdentity::of(&metadata).unwrap(), old_identity);
    stage.revalidate().unwrap();
    drop(stage);
    assert_eq!(
        crate::read_private(&destination, 64).unwrap().as_slice(),
        b"next"
    );
}
