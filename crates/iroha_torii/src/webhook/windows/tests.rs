//! Native webhook startup, recovery and custody controls using real private originals.

use super::super::{
    RegisteredWebhook, RegistryInner, WEBHOOK_REGISTRY_MAX_BYTES, WebhookEntry, init_persistence,
    lock_registry, persist_registry,
};
use super::*;
use std::{fs, num::NonZeroUsize, process::Command};

struct Fixture {
    _override: crate::data_dir::OverrideGuard,
    _temporary: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::Builder::new()
            .prefix("iroha-webhook-native-")
            .tempdir_in(std::env::temp_dir())
            .expect("local native temporary root");
        let root = temporary.path().join("torii");
        let override_guard = crate::data_dir::OverrideGuard::new(&root);
        *lock_unpoisoned(cursor()) = None;
        *lock_registry() = RegistryInner::default();
        Self {
            _override: override_guard,
            _temporary: temporary,
            root,
        }
    }

    fn directory(&self) -> WebhookDirectory {
        open_webhook_directory_chain(&self.root, true, false)
            .unwrap()
            .unwrap()
    }

    fn queue(&self) -> WebhookDirectory {
        open_webhook_queue_directory(true).unwrap().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        *lock_unpoisoned(cursor()) = None;
        *lock_registry() = RegistryInner::default();
    }
}

fn write(directory: &WebhookDirectory, name: &str, bytes: &[u8]) {
    write_private_webhook_file_atomic(
        directory,
        &directory.path.join(name),
        bytes,
        1024,
        WebhookPublication::CreateNew,
    )
    .unwrap();
}

fn broad_read(path: &Path) {
    assert!(
        Command::new("icacls.exe")
            .arg(path)
            .args(["/grant", "*S-1-1-0:R"])
            .status()
            .expect("actual native ACL operation")
            .success()
    );
}

#[test]
fn configured_startup_persists_private_registry_and_reloads_generation_and_high_water() {
    let fixture = Fixture::new();
    // This is the exact persistence operation invoked before Torii's listener bind.
    init_persistence().unwrap();
    let directory = fixture.directory();
    let original_root = directory.native.identity().unwrap();
    let mut registry = RegistryInner {
        next_id: 41,
        ..RegistryInner::default()
    };
    registry.items.insert(
        7,
        RegisteredWebhook {
            entry: WebhookEntry {
                id: 7,
                url: "https://example.com/hook".into(),
                active: true,
                secret: Some("runtime-only-test-secret".into()),
                filter: None,
            },
            generation: [0x39; 16],
        },
    );
    persist_registry(&registry).unwrap();
    let record = read_private_webhook_file_bounded(
        &directory,
        &directory.path.join("webhooks.json"),
        WEBHOOK_REGISTRY_MAX_BYTES,
    )
    .unwrap()
    .unwrap();
    let value: norito::json::Value = norito::json::from_slice(&record.bytes).unwrap();
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .get("next_id")
            .and_then(norito::json::Value::as_u64),
        Some(41)
    );
    *lock_registry() = RegistryInner::default();
    init_persistence().unwrap();
    let reloaded = lock_registry();
    assert_eq!(reloaded.next_id, 41);
    assert_eq!(reloaded.items.len(), 1);
    assert_eq!(reloaded.items[&7].generation, [0x39; 16]);
    assert_eq!(
        reloaded.items[&7].entry.secret.as_deref(),
        Some("runtime-only-test-secret")
    );
    assert_eq!(directory.native.identity().unwrap(), original_root);
    fixture.queue().native_revalidate().unwrap();
}

#[test]
fn bounded_private_read_and_create_collision_preserve_original_bytes() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    write(&directory, "record.json", b"original");
    let path = directory.path.join("record.json");
    assert!(read_private_webhook_file_bounded(&directory, &path, 7).is_err());
    let original = read_private_webhook_file_bounded(&directory, &path, 8)
        .unwrap()
        .unwrap();
    assert_eq!(original.bytes, b"original");
    assert_eq!(
        write_private_webhook_file_atomic(
            &directory,
            &path,
            b"replace",
            8,
            WebhookPublication::CreateNew
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(
        write_private_webhook_file_atomic(
            &directory,
            &directory.path.join("oversize"),
            b"123",
            2,
            WebhookPublication::CreateNew
        )
        .is_err()
    );
    let after = read_private_webhook_file_bounded(&directory, &path, 8)
        .unwrap()
        .unwrap();
    assert_eq!(after.identity, original.identity);
    assert_eq!(after.bytes, original.bytes);
    assert_eq!(
        directory.native.entries(4).unwrap(),
        vec![OsString::from("record.json")]
    );
}

#[test]
fn queue_replacement_and_exact_unlink_refuse_a_same_byte_substitute() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    let path = directory.path.join("delivery.json");
    write(&directory, "delivery.json", b"first");
    let first = read_private_webhook_file_bounded(&directory, &path, 16)
        .unwrap()
        .unwrap();
    write_private_webhook_file_atomic(
        &directory,
        &path,
        b"second",
        16,
        WebhookPublication::ReplaceIdentity(first.identity),
    )
    .unwrap();
    let second = read_private_webhook_file_bounded(&directory, &path, 16)
        .unwrap()
        .unwrap();
    assert_eq!(second.bytes, b"second");
    assert_ne!(second.identity, first.identity);
    assert!(unlink_private_webhook_entry(&directory, &path, Some(first.identity), true).is_err());
    // Same bytes do not confer the earlier object's deletion or replacement authority.
    write_private_webhook_file_atomic(
        &directory,
        &path,
        b"second",
        16,
        WebhookPublication::Replace,
    )
    .unwrap();
    assert!(
        write_private_webhook_file_atomic(
            &directory,
            &path,
            b"third",
            16,
            WebhookPublication::ReplaceIdentity(second.identity)
        )
        .is_err()
    );
    assert!(unlink_private_webhook_entry(&directory, &path, Some(second.identity), true).is_err());
    let current = read_private_webhook_file_bounded(&directory, &path, 16)
        .unwrap()
        .unwrap();
    assert_eq!(current.bytes, b"second");
    unlink_private_webhook_entry(&directory, &path, Some(current.identity), true).unwrap();
    assert!(
        read_private_webhook_file_bounded(&directory, &path, 16)
            .unwrap()
            .is_none()
    );
}

#[test]
fn hardlinks_and_broad_private_acl_are_refused_without_repair_or_deletion() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    write(&directory, "record.json", b"secret");
    let path = directory.path.join("record.json");
    let alias = directory.path.join("alias.json");
    fs::hard_link(&path, &alias).unwrap();
    assert!(read_private_webhook_file_bounded(&directory, &path, 16).is_err());
    assert!(unlink_private_webhook_entry(&directory, &path, None, true).is_err());
    assert_eq!(fs::read(&alias).unwrap(), b"secret");
    fs::remove_file(&alias).unwrap();
    broad_read(&path);
    assert!(read_private_webhook_file_bounded(&directory, &path, 16).is_err());
    assert!(
        write_private_webhook_file_atomic(
            &directory,
            &path,
            b"new",
            16,
            WebhookPublication::Replace
        )
        .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), b"secret");
}

#[test]
fn queue_acl_drift_is_detected_by_the_retained_private_original() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    broad_read(&directory.path);
    assert!(directory.native_revalidate().is_err());
    assert!(open_webhook_queue_directory(false).is_err());
    assert!(
        write_private_webhook_file_atomic(
            &directory,
            &directory.path.join("new.json"),
            b"x",
            1,
            WebhookPublication::CreateNew
        )
        .is_err()
    );
    assert!(!directory.path.join("new.json").exists());
}

#[test]
fn junction_parent_and_escaped_member_are_refused_without_privilege_skip() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    write(&directory, "record.json", b"safe");
    let alias = fixture._temporary.path().join("alias");
    assert!(
        Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&alias)
            .arg(&directory.path)
            .status()
            .unwrap()
            .success()
    );
    let rejected = open_webhook_directory_chain(&alias, false, false).is_err();
    fs::remove_dir(&alias).unwrap();
    assert!(
        rejected,
        "the caller's junction spelling must not be canonicalized away"
    );
    assert!(
        read_private_webhook_file_bounded(
            &directory,
            &directory.path.join("nested/../record.json"),
            8
        )
        .is_err()
    );
    assert_eq!(
        read_private_webhook_file_bounded(&directory, &directory.path.join("record.json"), 8)
            .unwrap()
            .unwrap()
            .bytes,
        b"safe"
    );
}

#[test]
fn before_publication_failure_recovers_only_owned_private_temporaries() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    write(&directory, "delivery.json", b"old");
    let path = directory.path.join("delivery.json");
    let error = write_observed(
        &directory,
        &path,
        b"new",
        16,
        WebhookPublication::Replace,
        |step| {
            if step == AtomicWriteStep::WriteTail {
                Err(io::Error::other("first write refusal"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "first write refusal");
    assert_eq!(
        read_private_webhook_file_bounded(&directory, &path, 16)
            .unwrap()
            .unwrap()
            .bytes,
        b"old"
    );
    assert_eq!(directory.native.entries(4).unwrap().len(), 2);
    init_persistence().unwrap();
    assert_eq!(
        directory.native.entries(4).unwrap(),
        vec![OsString::from("delivery.json")]
    );
    write_private_webhook_file_atomic(
        &directory,
        &path,
        b"recovered",
        16,
        WebhookPublication::Replace,
    )
    .unwrap();
    assert_eq!(
        read_private_webhook_file_bounded(&directory, &path, 16)
            .unwrap()
            .unwrap()
            .bytes,
        b"recovered"
    );
}

#[test]
fn postpublication_failure_keeps_complete_new_bytes_for_restart_reconciliation() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    write(&directory, "delivery.json", b"old");
    let path = directory.path.join("delivery.json");
    let error = write_observed(
        &directory,
        &path,
        b"complete-new",
        16,
        WebhookPublication::Replace,
        |step| {
            if step == AtomicWriteStep::Published {
                Err(io::Error::other("postpublication refusal"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "postpublication refusal");
    init_persistence().unwrap();
    let actual = read_private_webhook_file_bounded(&directory, &path, 16)
        .unwrap()
        .unwrap();
    assert_eq!(actual.bytes, b"complete-new");
    assert_eq!(
        directory.native.entries(4).unwrap(),
        vec![OsString::from("delivery.json")]
    );
    unlink_private_webhook_entry(&directory, &path, Some(actual.identity), true).unwrap();
}

#[test]
fn recovery_refuses_hardlinked_temporary_without_removing_either_name() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    let name = ".webhook-00112233445566778899aabbccddeeff.tmp";
    write(&directory, name, b"partial");
    fs::hard_link(directory.path.join(name), directory.path.join("retained")).unwrap();
    assert!(cleanup_webhook_temporary_files(&directory, 8).is_err());
    assert_eq!(fs::read(directory.path.join(name)).unwrap(), b"partial");
    assert_eq!(
        fs::read(directory.path.join("retained")).unwrap(),
        b"partial"
    );
}

#[test]
fn native_queue_scan_preserves_batch_capacity_and_inventory_refusal() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    for i in 0..130 {
        write(&directory, &format!("{i:03}.json"), b"{}");
    }
    let policy = WebhookPolicy {
        queue_capacity: NonZeroUsize::new(129).unwrap(),
        ..WebhookPolicy::default()
    };
    let first = discover_queue_batch(policy).unwrap();
    assert_eq!(first.paths.len(), 128);
    assert!(first.overflow_paths.is_empty());
    assert!(!first.sweep_complete);
    let second = discover_queue_batch(policy).unwrap();
    assert_eq!(second.paths.len(), 1);
    assert_eq!(second.overflow_paths.len(), 1);
    assert!(second.sweep_complete);
    assert_eq!(queue_depth_bounded_in(&directory, 129, 130).unwrap(), 129);
    assert!(queue_depth_bounded_in(&directory, 129, 129).is_err());
    assert!(cleanup_webhook_temporary_files(&directory, 129).is_err());
    assert_eq!(directory.native.entries(130).unwrap().len(), 130);
}

#[test]
fn first_publication_error_survives_later_private_custody_refusal() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    let path = directory.path.join("delivery.json");
    let error = write_observed(
        &directory,
        &path,
        b"new",
        16,
        WebhookPublication::CreateNew,
        |step| {
            if step == AtomicWriteStep::WriteHead {
                broad_read(&directory.path);
                Err(io::Error::other("initiating refusal"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "initiating refusal");
    assert!(directory.native_revalidate().is_err());
    assert!(!path.exists());
}

#[test]
fn public_project_directory_is_allowed_but_existing_public_queue_is_not_repaired() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    broad_read(&directory.path);
    directory.native_revalidate().unwrap();
    let queue = directory.native.create_child("queue").unwrap();
    broad_read(queue.path());
    assert!(init_persistence().is_err());
    assert!(PrivateDirectory::open(queue.path()).is_err());
    assert!(queue.entries(1).unwrap().is_empty());
    assert!(!directory.path.join("webhooks.json").exists());
}

#[test]
fn requested_removal_barrier_retries_after_the_original_name_is_absent() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    write(&directory, "delivery.json", b"old");
    let path = directory.path.join("delivery.json");
    let mut barriers = 0;
    for _ in 0..2 {
        let error = unlink_observed(&directory, &path, None, true, |step| {
            assert_eq!(step, AtomicWriteStep::SyncDirectory);
            barriers += 1;
            Err(io::Error::other("late removal barrier refusal"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "late removal barrier refusal");
        assert!(!path.exists());
        directory.native_revalidate().unwrap();
    }
    assert_eq!(barriers, 2);
    unlink_observed(&directory, &path, None, true, |step| {
        assert_eq!(step, AtomicWriteStep::SyncDirectory);
        barriers += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(barriers, 3);
    assert!(directory.native.entries(1).unwrap().is_empty());
}

#[test]
fn empty_recovery_retries_the_failed_final_directory_barrier() {
    let fixture = Fixture::new();
    let directory = fixture.queue();
    write(
        &directory,
        ".webhook-00112233445566778899aabbccddeeff.tmp",
        b"partial",
    );
    let mut barriers = 0;
    for _ in 0..2 {
        let error = cleanup_observed(&directory, 8, |step| {
            assert_eq!(step, AtomicWriteStep::SyncDirectory);
            barriers += 1;
            Err(io::Error::other("late recovery barrier refusal"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "late recovery barrier refusal");
        assert!(directory.native.entries(1).unwrap().is_empty());
    }
    assert_eq!(barriers, 2);
    assert_eq!(
        cleanup_observed(&directory, 8, |step| {
            assert_eq!(step, AtomicWriteStep::SyncDirectory);
            barriers += 1;
            Ok(())
        })
        .unwrap(),
        0
    );
    assert_eq!(barriers, 3);
}

#[test]
fn successful_observer_cannot_publish_after_private_queue_acl_drift() {
    for boundary in [
        AtomicWriteStep::CreateTemp,
        AtomicWriteStep::WriteHead,
        AtomicWriteStep::WriteTail,
        AtomicWriteStep::SyncFile,
        AtomicWriteStep::Publish,
    ] {
        let fixture = Fixture::new();
        let directory = fixture.queue();
        write(&directory, "delivery.json", b"original");
        let path = directory.path.join("delivery.json");
        let result = write_observed(
            &directory,
            &path,
            b"replacement",
            16,
            WebhookPublication::Replace,
            |step| {
                if step == boundary {
                    broad_read(&directory.path);
                }
                Ok(())
            },
        );
        assert!(result.is_err(), "private ACL drift at {boundary:?}");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(directory.native_revalidate().is_err());
    }
}
