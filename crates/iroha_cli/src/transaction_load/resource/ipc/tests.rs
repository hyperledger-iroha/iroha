//! Real-file capture authentication and bounded frame controls; no child process runs.

use super::*;
use std::fs::OpenOptions;
use std::os::unix::{
    fs::{OpenOptionsExt, PermissionsExt, symlink},
    net::UnixListener,
};

fn control() -> Control {
    Control {
        origin: WallInstant::now(),
        lifetime_ns: 10_000_000_000,
        request_deadline_ns: AtomicU64::new(10_000_000_000),
        stop: AtomicBool::new(false),
        exit: AtomicU8::new(0),
    }
}
fn capture(captures: &Captures, request: Request, available: bool) -> (Response, Manifest) {
    let bytes = json::to_vec(
        &norito::json!({"schema":CAPTURE_SCHEMA,"kind":(request.kind.text()),
        "sequence":(request.sequence),"available":available}),
    )
    .unwrap();
    let manifest = Manifest {
        name: request.kind.manifest_name(request.sequence),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(captures.path.join(&manifest.name))
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    captures.directory.sync_all().unwrap();
    (
        Response {
            outcome: if available {
                Outcome::Complete
            } else {
                Outcome::Unavailable
            },
            manifest: Some(manifest.clone()),
        },
        manifest,
    )
}

#[test]
fn bounded_frame_accepts_fragmentation_but_rejects_oversize_and_trailing_lines() {
    let mut frame = Vec::new();
    accept_frame_chunk(&mut frame, b"{\"ok\":").unwrap();
    accept_frame_chunk(&mut frame, b"true}\n").unwrap();
    assert_eq!(frame, b"{\"ok\":true}\n");
    assert!(accept_frame_chunk(&mut frame, b"{}").is_err());
    assert!(accept_frame_chunk(&mut Vec::new(), b"{}\n{}\n").is_err());
    let mut maximum = vec![b' '; MAX_IPC_BYTES - 1];
    accept_frame_chunk(&mut maximum, b"\n").unwrap();
    assert_eq!(maximum.len(), MAX_IPC_BYTES);
    assert!(accept_frame_chunk(&mut vec![b' '; MAX_IPC_BYTES], b"\n").is_err());
}

#[test]
fn actual_manifest_binds_exact_bytes_hash_kind_sequence_and_availability() {
    let dir = tempfile::tempdir().unwrap();
    let captures = Captures::create(&dir.path().canonicalize().unwrap().join("captures")).unwrap();
    let request = Request {
        kind: Kind::Sample,
        sequence: 1,
        timeout_ms: 400,
    };
    let ctl = control();
    let (response, manifest) = capture(&captures, request, true);
    captures
        .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
        .unwrap();
    let mut wrong_hash = manifest.clone();
    wrong_hash.sha256 = "0".repeat(64);
    assert!(
        captures
            .authenticate(request, &response, &wrong_hash, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    let mut wrong_size = manifest.clone();
    wrong_size.bytes += 1;
    assert!(
        captures
            .authenticate(request, &response, &wrong_size, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    assert!(
        captures
            .authenticate(
                Request {
                    sequence: 2,
                    ..request
                },
                &response,
                &manifest,
                &ctl,
                ctl.lifetime_ns
            )
            .is_err()
    );
    assert!(
        captures
            .authenticate(
                request,
                &Response {
                    outcome: Outcome::Unavailable,
                    ..response.clone()
                },
                &manifest,
                &ctl,
                ctl.lifetime_ns
            )
            .is_err()
    );
    let unavailable = Request {
        sequence: 2,
        ..request
    };
    let (response, manifest) = capture(&captures, unavailable, false);
    captures
        .authenticate(unavailable, &response, &manifest, &ctl, ctl.lifetime_ns)
        .unwrap();
}

#[test]
fn manifest_rejects_links_nonregular_files_public_modes_and_replaced_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let captures = Captures::create(&root.join("captures")).unwrap();
    let request = Request {
        kind: Kind::Sample,
        sequence: 1,
        timeout_ms: 400,
    };
    let ctl = control();
    let (response, manifest) = capture(&captures, request, true);
    let path = captures.path.join(&manifest.name);
    std::fs::hard_link(&path, root.join("alias")).unwrap();
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    std::fs::remove_file(root.join("alias")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    captures
        .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
        .unwrap();
    std::fs::rename(&path, root.join("saved")).unwrap();
    symlink(root.join("saved"), &path).unwrap();
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    std::fs::remove_file(&path).unwrap();
    let socket = UnixListener::bind(&path).unwrap();
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
    drop(socket);
    std::fs::remove_file(&path).unwrap();
    std::fs::rename(root.join("saved"), &path).unwrap();
    std::fs::rename(&captures.path, root.join("old-captures")).unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&captures.path)
        .unwrap();
    assert!(captures.check().is_err());
}

#[test]
fn capture_directory_is_no_clobber_and_source_deadlines_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let captures = Captures::create(&root.join("captures")).unwrap();
    assert!(Captures::create(&captures.path).is_err());
    assert!(Captures::create(Path::new("relative-captures")).is_err());
    let ctl = control();
    assert!(ctl.check(ctl.lifetime_ns).is_ok());
    assert!(ctl.check(0).is_err());
    ctl.stop.store(true, Ordering::SeqCst);
    assert!(ctl.check(ctl.lifetime_ns).is_err());
    let request = Request {
        kind: Kind::Sample,
        sequence: 1,
        timeout_ms: 400,
    };
    let (response, manifest) = capture(&captures, request, true);
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
}

#[test]
fn authenticated_manifest_never_accepts_remote_offsets_as_its_identity() {
    let dir = tempfile::tempdir().unwrap();
    let captures = Captures::create(&dir.path().canonicalize().unwrap().join("captures")).unwrap();
    let request = Request {
        kind: Kind::Sample,
        sequence: 1,
        timeout_ms: 400,
    };
    let ctl = control();
    let (response, mut manifest) = capture(&captures, request, true);
    let path = captures.path.join(&manifest.name);
    let mut value: Value = json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("sequence".to_owned(), norito::json!(9));
    value
        .as_object_mut()
        .unwrap()
        .insert("start_offset_ns".to_owned(), norito::json!(0));
    let bytes = json::to_vec(&value).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    manifest.sha256 = format!("{:x}", Sha256::digest(&bytes));
    manifest.bytes = bytes.len() as u64;
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
}

#[test]
fn hash_correct_manifest_with_duplicate_identity_field_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let captures = Captures::create(&dir.path().canonicalize().unwrap().join("captures")).unwrap();
    let request = Request {
        kind: Kind::Sample,
        sequence: 1,
        timeout_ms: 400,
    };
    let ctl = control();
    let (response, mut manifest) = capture(&captures, request, true);
    captures
        .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
        .unwrap();
    let path = captures.path.join(&manifest.name);
    let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
    let duplicate = text.replacen("{", "{\"sequence\":1,", 1).into_bytes();
    std::fs::write(&path, &duplicate).unwrap();
    manifest.bytes = duplicate.len() as u64;
    manifest.sha256 = format!("{:x}", Sha256::digest(&duplicate));
    assert!(
        captures
            .authenticate(request, &response, &manifest, &ctl, ctl.lifetime_ns)
            .is_err()
    );
}
