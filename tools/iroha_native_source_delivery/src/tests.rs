//! Inert file-relation controls for retained promotion and interrupted boundaries.

use super::*;
use norito::json;
use std::fs;

const OLD: &[u8] = b"original native artifact";
const FRESH: &[u8] = b"authenticated fresh native bytes";
const NAME: &str = "_crypto.pyd";
const STAGE: &str = ".fresh-native-test";

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn fixture(old: bool) -> (tempfile::TempDir, Value) {
    let temporary = tempfile::tempdir().unwrap();
    let root = OwnerDirectory::open(temporary.path()).unwrap();
    let source = root.create_child("source").unwrap();
    if old {
        source
            .write_atomic(NAME, OLD, iroha_fs::PublishMode::CreateNew)
            .unwrap();
    }
    let request = json!({"op": "begin", "source": (source.path().to_str().unwrap()),
        "backup": (root.path().join("backup").to_str().unwrap()), "name": NAME,
        "old_sha256": (if old { Value::String(hash(OLD)) } else { Value::Null })});
    (temporary, request)
}

fn stage(session: &mut Session) {
    session
        .request(&json!({"op": "intent", "json": "{\"intent\":true}"}))
        .unwrap();
    session.request(&json!({"op": "stage", "name": STAGE, "length": (FRESH.len()), "sha256": (hash(FRESH))})).unwrap();
    session
        .request(&json!({"op": "chunk", "hex": (hex::encode(FRESH))}))
        .unwrap();
    session.request(&json!({"op": "seal"})).unwrap();
}

fn finish(session: &mut Session) {
    session.request(&json!({"op": "backup"})).unwrap();
    session.request(&json!({"op": "activate"})).unwrap();
    session
        .request(&json!({"op": "receipt", "json": "{\"receipt\":true}"}))
        .unwrap();
    session.request(&json!({"op": "complete"})).unwrap();
}

#[test]
fn sealed_control_readback_rejects_content_changed_before_sealing() {
    let (_temporary, request) = fixture(false);
    let session = Session::begin(&request).unwrap();
    let mut writer = session
        .backup
        .create_retained_private("control.json", MAX_CONTROL)
        .unwrap();
    writer.write_all(b"altered").unwrap();
    // Model a mutation before sealing establishes its immutable baseline. Equal
    // length and valid retained metadata do not authenticate authored bytes.
    let mut sealed = writer.seal_read_only().unwrap();
    assert!(verify_control_bytes(&mut sealed, b"written").is_err());
    assert!(verify_control_bytes(&mut sealed, b"alter").is_err());
    assert!(verify_control_bytes(&mut sealed, b"altered trailing").is_err());
    verify_control_bytes(&mut sealed, b"altered").unwrap();
    assert!(session.backup.path().join("control.json").is_file());
}

#[test]
fn original_identity_and_fresh_bytes_survive_complete_transaction() {
    for old in [true, false] {
        let (_temporary, request) = fixture(old);
        let mut session = Session::begin(&request).unwrap();
        let original_id = session.old.as_ref().map(|file| file.identity().unwrap());
        stage(&mut session);
        let fresh_id = session.fresh.as_ref().unwrap().identity().unwrap();
        finish(&mut session);
        assert_eq!(session.phase, Phase::Complete);
        assert_eq!(
            session.active.as_ref().unwrap().identity().unwrap(),
            fresh_id
        );
        assert_eq!(
            session
                .preserved
                .as_ref()
                .map(|file| file.identity().unwrap()),
            original_id
        );
        assert_eq!(fs::read(session.source.path().join(NAME)).unwrap(), FRESH);
        if old {
            assert_eq!(fs::read(session.backup.path().join(NAME)).unwrap(), OLD);
        }
        assert!(!session.backup.path().join(STAGE).exists());
        assert!(session.backup.path().join("intent.json").is_file());
        assert!(session.backup.path().join("delivery.json").is_file());
    }
}

#[test]
fn request_inventory_order_and_digest_are_enforced() {
    let (_temporary, request) = fixture(true);
    let mut session = Session::begin(&request).unwrap();
    assert!(session.request(&json!({"op": "activate"})).is_err());
    assert!(
        session
            .request(&json!({"op": "intent", "json": "{}", "extra": true}))
            .is_err()
    );
    assert!(
        session
            .request(&json!({"op": "intent", "json": ""}))
            .is_err()
    );
    session
        .request(&json!({"op": "intent", "json": "{}"}))
        .unwrap();
    for (name, length, checksum) in [
        ("../escape", 1, hash(FRESH)),
        (STAGE, 0, hash(FRESH)),
        (STAGE, MAX_NATIVE + 1, hash(FRESH)),
        (STAGE, 1, "Z".repeat(64)),
    ] {
        assert!(
            session
                .request(
                    &json!({"op": "stage", "name": name, "length": length, "sha256": checksum})
                )
                .is_err()
        );
    }
    session
        .request(&json!({"op": "stage", "name": STAGE, "length": 1, "sha256": (hash(b"a"))}))
        .unwrap();
    assert!(
        session
            .request(&json!({"op": "chunk", "hex": "zz"}))
            .is_err()
    );
    assert!(
        session
            .request(&json!({"op": "chunk", "hex": "6162"}))
            .is_err()
    );
    assert!(session.request(&json!({"op": "seal"})).is_err());
    session
        .request(&json!({"op": "chunk", "hex": "62"}))
        .unwrap();
    assert!(session.request(&json!({"op": "seal"})).is_err());
    assert_eq!(fs::read(session.source.path().join(NAME)).unwrap(), OLD);
}

#[test]
fn original_mismatch_and_nonempty_backup_are_refused() {
    let (temporary, mut request) = fixture(true);
    request
        .as_object_mut()
        .unwrap()
        .insert("old_sha256".into(), Value::String(hash(b"other")));
    assert!(Session::begin(&request).is_err());
    assert_eq!(
        fs::read(temporary.path().join("source").join(NAME)).unwrap(),
        OLD
    );
    fs::write(temporary.path().join("backup/foreign"), b"keep").unwrap();
    request
        .as_object_mut()
        .unwrap()
        .insert("old_sha256".into(), Value::String(hash(OLD)));
    assert!(Session::begin(&request).is_err());
    assert_eq!(
        fs::read(temporary.path().join("backup/foreign")).unwrap(),
        b"keep"
    );
}

#[test]
fn racing_activation_never_replaces_or_cleans_up_competing_file() {
    let (_temporary, request) = fixture(true);
    let mut session = Session::begin(&request).unwrap();
    stage(&mut session);
    session.request(&json!({"op": "backup"})).unwrap();
    fs::write(session.source.path().join(NAME), b"competing original").unwrap();
    assert!(session.request(&json!({"op": "activate"})).is_err());
    assert_eq!(
        fs::read(session.source.path().join(NAME)).unwrap(),
        b"competing original"
    );
    assert_eq!(fs::read(session.backup.path().join(NAME)).unwrap(), OLD);
    assert_eq!(fs::read(session.backup.path().join(STAGE)).unwrap(), FRESH);
    assert!(!session.backup.path().join("delivery.json").exists());
}

#[test]
fn interruption_preserves_intent_original_and_fresh_objects() {
    for boundary in 0..3 {
        let (temporary, request) = fixture(true);
        let mut session = Session::begin(&request).unwrap();
        stage(&mut session);
        if boundary >= 1 {
            session.request(&json!({"op": "backup"})).unwrap();
        }
        if boundary >= 2 {
            session.request(&json!({"op": "activate"})).unwrap();
        }
        drop(session);
        let source = temporary.path().join("source");
        let backup = temporary.path().join("backup");
        assert_eq!(
            fs::read(if boundary == 0 {
                source.join(NAME)
            } else {
                backup.join(NAME)
            })
            .unwrap(),
            OLD
        );
        assert_eq!(
            fs::read(if boundary == 2 {
                source.join(NAME)
            } else {
                backup.join(STAGE)
            })
            .unwrap(),
            FRESH
        );
        assert!(backup.join("intent.json").is_file());
        assert!(!backup.join("delivery.json").exists());
        if boundary == 1 {
            assert!(!source.join(NAME).exists());
        }
    }
}

#[test]
fn framed_transport_rejects_partial_or_excessive_requests_and_eof() {
    for bytes in [
        b"{}".to_vec(),
        vec![b' '; usize::try_from(MAX_REQUEST).unwrap() + 1],
        b"invalid\n".to_vec(),
    ] {
        assert!(run(io::Cursor::new(bytes), Vec::new()).is_err());
    }
    let (temporary, request) = fixture(true);
    let mut input = json::to_vec(&request).unwrap();
    input.push(b'\n');
    let mut output = Vec::new();
    assert!(run(io::Cursor::new(input), &mut output).is_err());
    assert_eq!(output, b"{}\n");
    assert_eq!(
        fs::read(temporary.path().join("source").join(NAME)).unwrap(),
        OLD
    );
}

#[cfg(windows)]
#[test]
fn native_session_blocks_original_rewrite_and_parent_substitution() {
    let (temporary, request) = fixture(true);
    let mut session = Session::begin(&request).unwrap();
    assert!(fs::write(session.source.path().join(NAME), b"changed").is_err());
    assert!(fs::rename(session.source.path(), temporary.path().join("replacement")).is_err());
    stage(&mut session);
    assert!(fs::write(session.backup.path().join(STAGE), b"changed").is_err());
    finish(&mut session);
}
