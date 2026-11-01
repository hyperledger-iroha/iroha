//! Native telemetry checkpoint custody, crash recovery and exact-error controls.

use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use super::*;
use crate::integrity::{ChainState, encode_state_snapshot};
use iroha_config::parameters::actual::TelemetryIntegrity;
use iroha_fs::PublishMode;
use norito::json::{Map, Value};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "iroha-telemetry-native-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("private").join("state.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config() -> TelemetryIntegrity {
    TelemetryIntegrity {
        enabled: true,
        state_dir: None,
        signing_key: Some([7; 32]),
        signing_key_id: Some("native-key".into()),
    }
}
fn chain(path: &Path) -> ChainState {
    ChainState::new_with_state_path(config(), Some(path.to_path_buf()), "native", b"native-sink")
        .unwrap()
}
fn payload(seq: u64) -> Vec<u8> {
    encode_state_snapshot([5; 32], seq, [0; 32], None).unwrap()
}
fn message(text: &str) -> Map {
    let mut map = Map::new();
    map.insert("msg".into(), Value::from(text));
    map
}

#[tokio::test]
async fn pending_record_restart_preserves_signed_bytes_and_commit_sequence() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut first = chain(&path);
    first.stage_record(message("first"), false).await.unwrap();
    let exact = first.pending_record().unwrap().to_vec();
    assert!(first.pending_is_durable());
    drop(first);
    let mut resumed = chain(&path);
    assert_eq!(resumed.pending_record(), Some(exact.as_slice()));
    assert!(!resumed.pending_output_is_confirmed());
    assert!(matches!(
        resumed.commit_pending().await,
        Err(IntegrityError::OutputNotConfirmed)
    ));
    resumed.confirm_pending_output().unwrap();
    resumed.commit_pending().await.unwrap();
    drop(resumed);
    let mut next = chain(&path);
    next.stage_record(message("second"), false).await.unwrap();
    let value: Value = norito::json::from_slice(next.pending_record().unwrap()).unwrap();
    let fields = value
        .as_object()
        .unwrap()
        .get("chain")
        .unwrap()
        .as_object()
        .unwrap();
    assert_eq!(fields.get("seq").and_then(Value::as_u64), Some(2));
    assert_eq!(
        fields.get("key_id").and_then(Value::as_str),
        Some("native-key")
    );
}

#[test]
fn one_original_lock_excludes_readers_removal_and_other_exporters() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let first = StateStore::open(&path).unwrap();
    let id = FileIdentity::of(&first.lock).unwrap();
    assert!(matches!(
        StateStore::open(&path),
        Err(IntegrityError::StateAlreadyInUse(_))
    ));
    let named = first.directory.path().join(&first.lock_name);
    assert!(File::open(&named).is_err());
    assert!(fs::remove_file(&named).is_err());
    first.revalidate().unwrap();
    drop(first);
    let resumed = StateStore::open(&path).unwrap();
    assert_eq!(FileIdentity::of(&resumed.lock).unwrap(), id);
}

#[test]
fn prepublication_failure_keeps_originals_and_exact_bytes_for_retry() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    store.persist(payload(1)).unwrap();
    let previous = fs::read(&path).unwrap();
    let error = store
        .persist_observed(payload(2), |step| {
            if step == AtomicWriteStep::WriteTail {
                Err("injected partial write".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error, "injected partial write");
    assert_eq!(fs::read(&path).unwrap(), previous);
    let id = store
        .pending
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .original_identity
        .unwrap();
    assert!(
        store
            .persist(payload(3))
            .unwrap_err()
            .contains("exact original bytes")
    );
    store
        .persist_observed(payload(2), |step| {
            if step == AtomicWriteStep::WriteHead {
                let named =
                    FileMetadata::symlink_metadata(store.directory.path().join(&store.temporary))
                        .unwrap();
                assert_eq!(named.identity(), id);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(store.load(&path, Ok).unwrap().unwrap().seq, 2);
    assert!(store.pending.lock().unwrap().is_none());
}

#[test]
fn postpublication_retry_retains_displaced_original_and_never_republishes() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    store.persist(payload(1)).unwrap();
    let error = store
        .persist_observed(payload(2), |step| {
            if step == AtomicWriteStep::Published {
                Err("injected after publication".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error, "injected after publication");
    {
        let slot = store.pending.lock().unwrap();
        let pending = slot.as_ref().unwrap();
        assert!(pending.published);
        store.check_publication(pending).unwrap();
        assert_eq!(
            store
                .read_original(&store.name, pending.original.as_ref().unwrap())
                .unwrap(),
            payload(2)
        );
        store
            .owner
            .require_retired_private_journal(pending.predecessor.as_ref().unwrap())
            .unwrap();
    }
    store
        .persist_observed(payload(2), |step| {
            assert_ne!(step, AtomicWriteStep::Publish);
            assert_ne!(step, AtomicWriteStep::Published);
            Ok(())
        })
        .unwrap();
    assert_eq!(store.load(&path, Ok).unwrap().unwrap().seq, 2);
}

#[test]
fn abandoned_private_temporary_is_bounded_and_reused_without_path_deletion() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    store
        .directory
        .write_atomic(&store.temporary, b"partial", PublishMode::CreateNew)
        .unwrap();
    let id = FileMetadata::symlink_metadata(store.directory.path().join(&store.temporary))
        .unwrap()
        .identity();
    store.persist(payload(1)).unwrap();
    assert_eq!(
        FileMetadata::symlink_metadata(&path).unwrap().identity(),
        id
    );
    assert!(!store.directory.path().join(&store.temporary).exists());
    store
        .directory
        .write_atomic(
            &store.temporary,
            &vec![b'x'; usize::try_from(STATE_FILE_MAX_BYTES).unwrap() + 1],
            PublishMode::CreateNew,
        )
        .unwrap();
    assert!(store.persist(payload(2)).unwrap_err().contains("exceeds"));
    assert_eq!(fs::read(&path).unwrap(), payload(1));
}

#[test]
fn bounded_native_state_uses_the_common_strict_schema_and_sink_checks() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    store
        .directory
        .write_atomic(
            &store.name,
            &vec![b' '; usize::try_from(STATE_FILE_MAX_BYTES).unwrap() + 1],
            PublishMode::CreateNew,
        )
        .unwrap();
    assert!(store.load(&path, Ok).unwrap_err().contains("exceeding"));
    store
        .directory
        .write_atomic(
            &store.name,
            br#"{"version":1,"seq":1}"#,
            PublishMode::Replace,
        )
        .unwrap();
    assert!(store.load(&path, Ok).unwrap_err().contains("exactly"));
    store.persist(payload(1)).unwrap();
    drop(store);
    assert!(matches!(
        ChainState::new_with_state_path(config(), Some(path), "other", b"sink"),
        Err(IntegrityError::LoadState(_))
    ));
}

#[test]
fn inherited_acl_and_hardlinked_state_or_lock_are_refused_without_repair() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    fs::write(&path, b"inherited").unwrap();
    assert!(store.load(&path, Ok).is_err());
    assert!(store.persist(payload(1)).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"inherited");
    drop(store);
    fs::remove_file(&path).unwrap();
    let store = StateStore::open(&path).unwrap();
    store.persist(payload(1)).unwrap();
    fs::hard_link(&path, fixture.0.join("state-alias")).unwrap();
    assert!(store.load(&path, Ok).is_err());
    assert!(store.persist(payload(2)).is_err());
    assert_eq!(fs::read(&path).unwrap(), payload(1));
    let lock = store.directory.path().join(&store.lock_name);
    drop(store);
    fs::hard_link(&lock, fixture.0.join("lock-alias")).unwrap();
    assert!(matches!(
        StateStore::open(&path),
        Err(IntegrityError::StateCustody(_))
    ));
}

#[test]
fn unsafe_roots_and_changed_configured_path_never_gain_new_authority() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("public")).unwrap();
    assert!(StateStore::open(&fixture.0.join("public/state.json")).is_err());
    assert!(StateStore::open(Path::new(r"\\server\share\state.json")).is_err());
    assert!(StateStore::open(Path::new(r"\\.\C:\state.json")).is_err());
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    assert!(
        store
            .require_path(&fixture.0.join("different/state.json"))
            .is_err()
    );
    assert!(!fixture.0.join("different").exists());
    assert!(StateStore::open(&store.directory.path().join("state.lock")).is_err());
}

#[test]
fn junction_state_leaf_is_refused_without_symlink_privilege_skips() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    let target = fixture.0.join("foreign");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel"), b"unchanged").unwrap();
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&path)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(store.load(&path, Ok).is_err());
    assert!(store.persist(payload(1)).is_err());
    assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"unchanged");
    fs::remove_dir(&path).unwrap();
}

#[test]
fn eager_final_custody_preserves_initiating_error_and_detects_lock_drift() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    let permissions = store.lock.metadata().unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    let error = store
        .persist_observed(payload(1), |step| {
            if step == AtomicWriteStep::WriteTail {
                store.lock.set_permissions(readonly.clone()).unwrap();
                return Err("initiating write refusal".into());
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error, "initiating write refusal");
    assert!(store.revalidate().is_err());
    store.lock.set_permissions(permissions).unwrap();
    // Explicit timestamp drift remains a refusal after permissions are restored.
    store
        .lock
        .set_times(
            fs::FileTimes::new().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(12345)),
        )
        .unwrap();
    assert!(store.revalidate().is_err());
    assert_eq!(
        finish::<()>(Err("first".into()), Err("later".into())).unwrap_err(),
        "first"
    );
    assert_eq!(
        finish::<()>(Ok(()), Err("later".into())).unwrap_err(),
        "later"
    );
}

#[test]
fn occupied_absence_and_replaced_stage_preserve_foreign_bytes() {
    for substitute_stage in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.path();
        let store = StateStore::open(&path).unwrap();
        let foreign = payload(77);
        let result = store.persist_observed(payload(1), |step| {
            if substitute_stage && step == AtomicWriteStep::WriteTail {
                fs::rename(
                    store.directory.path().join(&store.temporary),
                    fixture.0.join("held-original"),
                )
                .unwrap();
                store
                    .directory
                    .write_atomic(&store.temporary, &foreign, PublishMode::CreateNew)
                    .unwrap();
            } else if !substitute_stage && step == AtomicWriteStep::Publish {
                store
                    .directory
                    .write_atomic(&store.name, &foreign, PublishMode::CreateNew)
                    .unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        let named = if substitute_stage {
            &store.temporary
        } else {
            &store.name
        };
        assert_eq!(
            fs::read(store.directory.path().join(named)).unwrap(),
            foreign
        );
        assert!(!store.pending.lock().unwrap().as_ref().unwrap().published);
        if substitute_stage {
            assert!(!path.exists());
        }
    }
}

#[test]
fn parent_acl_drift_is_observed_on_first_error_exit() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let store = StateStore::open(&path).unwrap();
    let error = store
        .persist_observed(payload(1), |step| {
            if step == AtomicWriteStep::WriteTail {
                let output = std::process::Command::new("icacls.exe")
                    .arg(store.directory.path())
                    .args(["/grant", "*S-1-1-0:(RX)"])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                return Err("initiating ACL observation error".into());
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error, "initiating ACL observation error");
    assert!(store.revalidate().is_err());
    let output = std::process::Command::new("icacls.exe")
        .arg(store.directory.path())
        .args(["/remove:g", "*S-1-1-0"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    store.revalidate().unwrap();
    store.persist(payload(1)).unwrap();
}

#[tokio::test]
async fn pending_hash_and_signature_validation_run_before_original_release() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut first = chain(&path);
    first.stage_record(message("signed"), false).await.unwrap();
    drop(first);
    let directory = PrivateDirectory::open(path.parent().unwrap()).unwrap();
    let original = directory
        .read("state.json", usize::try_from(STATE_FILE_MAX_BYTES).unwrap())
        .unwrap();
    for field in ["hash", "signature"] {
        let mut state: Value = norito::json::from_slice(&original).unwrap();
        let snapshot = state.as_object_mut().unwrap();
        let mut pending: Value =
            norito::json::from_str(snapshot.get("pending_record").unwrap().as_str().unwrap())
                .unwrap();
        pending
            .as_object_mut()
            .unwrap()
            .get_mut("chain")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), Value::from(hex::encode([0_u8; 32])));
        snapshot.insert(
            "pending_record".into(),
            Value::from(norito::json::to_json(&pending).unwrap()),
        );
        directory
            .write_atomic(
                "state.json",
                &norito::json::to_vec(&state).unwrap(),
                PublishMode::Replace,
            )
            .unwrap();
        assert!(matches!(
            ChainState::new_with_state_path(config(), Some(path.clone()), "native", b"native-sink"),
            Err(IntegrityError::LoadState(_))
        ));
    }
}
