//! Mandatory admission before directory creation over test-owned local descriptors.

use super::*;
use std::fs::OpenOptions;
use std::os::unix::{fs::OpenOptionsExt, net::UnixStream};

fn request(kind: Kind) -> Request {
    Request {
        kind,
        sequence: 0,
        timeout_ms: 2000,
    }
}
fn fixture() -> (
    tempfile::TempDir,
    Pipe<UnixStream, UnixStream, PathBuf>,
    UnixStream,
    UnixStream,
) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap().join("captures");
    let (stdin, input) = UnixStream::pair().unwrap();
    let (stdout, output) = UnixStream::pair().unwrap();
    for pipe in [&stdin, &stdout] {
        pipe.set_nonblocking(true).unwrap();
    }
    for peer in [&input, &output] {
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
    }
    let control = Arc::new(Control {
        origin: WallInstant::now(),
        lifetime_ns: 10_000_000_000,
        request_deadline_ns: AtomicU64::new(10_000_000_000),
        stop: AtomicBool::new(false),
        exit: AtomicU8::new(0),
    });
    (
        root,
        Pipe {
            stdin: Some(stdin),
            stdout,
            control,
            captures: path,
        },
        input,
        output,
    )
}
fn read_request(input: &mut UnixStream, expected: Request) {
    let mut line = Vec::new();
    loop {
        let mut byte = [0_u8];
        input.read_exact(&mut byte).unwrap();
        line.push(byte[0]);
        assert!(line.len() <= MAX_IPC_BYTES);
        if byte[0] == b'\n' {
            break;
        }
    }
    assert_eq!(line, expected.line().unwrap());
}
fn response(request: Request, outcome: &str, manifest: Value) -> Vec<u8> {
    if request.kind == Kind::Admit && manifest == Value::Null {
        let mut value = allocation::tests::value(MAX_FILE_BYTES, MAX_FILE_BYTES);
        let object = value.as_object_mut().unwrap();
        object.insert("sequence".to_owned(), norito::json!(request.sequence));
        object.insert("outcome".to_owned(), norito::json!(outcome));
        if outcome != "complete" {
            object.insert("admission".to_owned(), Value::Null);
        }
        return allocation::tests::line(&value);
    }
    let mut line = json::to_vec(&norito::json!({"schema": RESPONSE_SCHEMA,
        "kind": (request.kind.text()), "sequence": (request.sequence),
        "outcome": outcome, "manifest": manifest}))
    .unwrap();
    line.push(b'\n');
    line
}

#[test]
fn admission_actor_requires_one_ack_before_creation_and_real_preflight() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let watched_path = path.clone();
    let control = Arc::clone(&pending.control);
    let deadline = control.now() + 2_000_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        assert!(!watched_path.exists());
        output
            .write_all(&response(request(Kind::Admit), "complete", Value::Null))
            .unwrap();
        read_request(&mut input, request(Kind::Preflight));
        assert!(watched_path.is_dir());
        assert_eq!(
            std::fs::metadata(&watched_path).unwrap().mode() & 0o7777,
            0o700
        );
        let bytes = json::to_vec(&norito::json!({"schema": CAPTURE_SCHEMA,
            "kind": "preflight", "sequence": 0, "available": true}))
        .unwrap();
        let name = "preflight-0000000000.json";
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(watched_path.join(name))
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        let manifest = norito::json!({"name": name,
            "sha256": (format!("{:x}", Sha256::digest(&bytes))), "bytes": (bytes.len())});
        output
            .write_all(&response(request(Kind::Preflight), "complete", manifest))
            .unwrap();
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    assert!(
        sender
            .send(Work {
                request: request(Kind::Preflight),
                deadline,
                reply,
            })
            .is_ok()
    );
    drop(sender);
    let (admission_reply, mut admission_receive) = tokio::sync::oneshot::channel();
    serve_pipe(
        pending,
        receiver,
        allocation::tests::expected(),
        request(Kind::Admit),
        deadline,
        admission_reply,
    );
    assert!(admission_receive.try_recv().unwrap().is_ok());
    let result = receive.try_recv().unwrap().unwrap();
    assert_eq!(result.outcome, Outcome::Complete);
    assert_eq!(result.manifest.unwrap().name, "preflight-0000000000.json");
    assert!(path.is_dir());
    assert!(control.stop.load(Ordering::Acquire));
    peer.join().unwrap();
}

#[test]
fn admission_failed_unavailable_or_capture_reply_never_creates_directory() {
    for (outcome, manifest) in [
        ("failed", Value::Null),
        ("unavailable", Value::Null),
        (
            "complete",
            norito::json!({"name": "admit-0000000000.json", "sha256": ("a".repeat(64)), "bytes": 1}),
        ),
    ] {
        let (_root, pending, mut input, mut output) = fixture();
        let path = pending.captures.clone();
        let deadline = pending.control.now() + 2_000_000_000;
        let peer = std::thread::spawn(move || {
            read_request(&mut input, request(Kind::Admit));
            output
                .write_all(&response(request(Kind::Admit), outcome, manifest))
                .unwrap();
        });
        assert!(
            pending
                .admit(
                    request(Kind::Admit),
                    deadline,
                    &allocation::tests::expected()
                )
                .is_err()
        );
        assert!(!path.exists());
        peer.join().unwrap();
    }
}

#[test]
fn admission_rejects_wrong_response_identity_before_any_directory_creation() {
    for reply in [
        request(Kind::Preflight),
        Request {
            sequence: 1,
            ..request(Kind::Admit)
        },
    ] {
        let (_root, pending, mut input, mut output) = fixture();
        let path = pending.captures.clone();
        let deadline = pending.control.now() + 2_000_000_000;
        let peer = std::thread::spawn(move || {
            read_request(&mut input, request(Kind::Admit));
            output
                .write_all(&response(reply, "complete", Value::Null))
                .unwrap();
        });
        assert!(
            pending
                .admit(
                    request(Kind::Admit),
                    deadline,
                    &allocation::tests::expected()
                )
                .is_err()
        );
        assert!(!path.exists());
        peer.join().unwrap();
    }
}

#[test]
fn admission_and_preflight_cannot_reset_the_callers_absolute_deadline() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let deadline = pending.control.now() + 2_000_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        output
            .write_all(&response(request(Kind::Admit), "complete", Value::Null))
            .unwrap();
    });
    let (admitted, writers) = pending
        .admit(
            request(Kind::Admit),
            deadline,
            &allocation::tests::expected(),
        )
        .unwrap();
    peer.join().unwrap();
    assert!(!path.exists());
    assert_eq!(writers.journal.max_bytes, MAX_FILE_BYTES);
    let mut admitted = admitted.begin_preflight(deadline).unwrap();
    assert!(path.is_dir());
    // A preflight's positive timeout field cannot override an expired absolute
    // deadline supplied by the caller to the full admission/setup operation.
    let expired = admitted.control.now();
    assert!(
        admitted
            .exchange(request(Kind::Preflight), expired)
            .is_err()
    );
    assert!(std::fs::read_dir(&path).unwrap().next().is_none());
    let (_root, pending, _input, _output) = fixture();
    let path = pending.captures.clone();
    let expired = pending.control.now();
    assert!(
        pending
            .admit(
                request(Kind::Admit),
                expired,
                &allocation::tests::expected()
            )
            .is_err()
    );
    assert!(!path.exists());
}

#[test]
fn admission_actor_rejects_invalid_first_work_without_starting_capture_preflight() {
    for first in [
        request(Kind::Sample),
        request(Kind::Admit),
        Request {
            sequence: 1,
            ..request(Kind::Preflight)
        },
    ] {
        let (_root, pending, mut input, mut output) = fixture();
        let path = pending.captures.clone();
        let control = Arc::clone(&pending.control);
        let deadline = control.now() + 2_000_000_000;
        let peer = std::thread::spawn(move || {
            read_request(&mut input, request(Kind::Admit));
            output
                .write_all(&response(request(Kind::Admit), "complete", Value::Null))
                .unwrap();
            input
        });
        let (sender, receiver) = mpsc::sync_channel(1);
        let (reply, mut receive) = tokio::sync::oneshot::channel();
        assert!(
            sender
                .send(Work {
                    request: first,
                    deadline,
                    reply,
                })
                .is_ok()
        );
        let (admission_reply, mut admission_receive) = tokio::sync::oneshot::channel();
        serve_pipe(
            pending,
            receiver,
            allocation::tests::expected(),
            request(Kind::Admit),
            deadline,
            admission_reply,
        );
        assert!(admission_receive.try_recv().unwrap().is_ok());
        assert!(receive.try_recv().unwrap().is_err());
        assert!(control.stop.load(Ordering::Acquire));
        assert!(!path.exists());
        let mut input = peer.join().unwrap();
        let mut byte = [0_u8];
        assert_eq!(input.read(&mut byte).unwrap(), 0);
    }
}

#[test]
fn admission_actor_does_not_extend_preflight_after_admission_spends_time() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let observed_path = path.clone();
    let control = Arc::clone(&pending.control);
    let peer_control = Arc::clone(&control);
    let deadline = control.now() + 2_000_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        assert!(!observed_path.exists());
        // Spend part of the original request before admission. A wrongly reset
        // second deadline would accept the response deliberately sent below.
        std::thread::sleep(Duration::from_millis(200));
        output
            .write_all(&response(request(Kind::Admit), "complete", Value::Null))
            .unwrap();
        read_request(&mut input, request(Kind::Preflight));
        assert!(observed_path.is_dir());
        let bytes = json::to_vec(&norito::json!({"schema": CAPTURE_SCHEMA,
            "kind": "preflight", "sequence": 0, "available": true}))
        .unwrap();
        let name = "preflight-0000000000.json";
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(observed_path.join(name))
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        let manifest = norito::json!({"name": name,
            "sha256": (format!("{:x}", Sha256::digest(&bytes))), "bytes": (bytes.len())});
        while peer_control.now() < deadline + 50_000_000 {
            std::thread::sleep(TURN);
        }
        // Correct cancellation may already have closed this test-owned pipe.
        let _ = output.write_all(&response(request(Kind::Preflight), "complete", manifest));
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    assert!(
        sender
            .send(Work {
                request: request(Kind::Preflight),
                deadline,
                reply
            })
            .is_ok()
    );
    drop(sender);
    let (admission_reply, mut admission_receive) = tokio::sync::oneshot::channel();
    serve_pipe(
        pending,
        receiver,
        allocation::tests::expected(),
        request(Kind::Admit),
        deadline,
        admission_reply,
    );
    assert!(admission_receive.try_recv().unwrap().is_ok());
    assert!(receive.try_recv().unwrap().is_err());
    assert!(control.stop.load(Ordering::Acquire));
    peer.join().unwrap();
    assert!(path.join("preflight-0000000000.json").is_file());
}

#[test]
fn admission_actor_returns_allocations_before_parent_authorizes_any_output() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let control = Arc::clone(&pending.control);
    let deadline = control.now() + 2_000_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        output
            .write_all(&response(request(Kind::Admit), "complete", Value::Null))
            .unwrap();
        let mut byte = [0_u8];
        assert_eq!(input.read(&mut byte).unwrap(), 0);
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    let actor = std::thread::spawn(move || {
        serve_pipe(
            pending,
            receiver,
            allocation::tests::expected(),
            request(Kind::Admit),
            deadline,
            reply,
        )
    });
    let writers = loop {
        match receive.try_recv() {
            Ok(value) => break value.unwrap(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                assert!(control.now() < deadline);
                std::thread::sleep(TURN);
            }
            Err(_) => panic!("admission owner closed"),
        }
    };
    assert_eq!(writers.journal.max_bytes, MAX_FILE_BYTES);
    assert_eq!(writers.trace.max_bytes, MAX_FILE_BYTES);
    assert!(!path.exists());
    drop(sender);
    actor.join().unwrap();
    peer.join().unwrap();
    assert!(!path.exists());
    assert!(control.stop.load(Ordering::Acquire));
}

#[test]
fn admission_actor_rejects_changed_budget_before_exposing_writers_or_capture_path() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let control = Arc::clone(&pending.control);
    let deadline = control.now() + 2_000_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        let mut value = allocation::tests::value(MAX_FILE_BYTES, MAX_FILE_BYTES);
        value
            .as_object_mut()
            .unwrap()
            .get_mut("admission")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("budget_sha256".to_owned(), norito::json!("b".repeat(64)));
        output.write_all(&allocation::tests::line(&value)).unwrap();
    });
    let (_sender, receiver) = mpsc::sync_channel(1);
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    serve_pipe(
        pending,
        receiver,
        allocation::tests::expected(),
        request(Kind::Admit),
        deadline,
        reply,
    );
    assert!(receive.try_recv().unwrap().is_err());
    assert!(!path.exists());
    assert!(control.stop.load(Ordering::Acquire));
    peer.join().unwrap();
}

#[test]
fn admitted_parent_setup_cannot_extend_deadline_or_create_late_capture_directory() {
    let (_root, pending, mut input, mut output) = fixture();
    let path = pending.captures.clone();
    let control = Arc::clone(&pending.control);
    let deadline = control.now() + 300_000_000;
    let peer = std::thread::spawn(move || {
        read_request(&mut input, request(Kind::Admit));
        output
            .write_all(&response(request(Kind::Admit), "complete", Value::Null))
            .unwrap();
        let mut byte = [0_u8];
        assert_eq!(input.read(&mut byte).unwrap(), 0);
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, mut receive) = tokio::sync::oneshot::channel();
    let actor = std::thread::spawn(move || {
        serve_pipe(
            pending,
            receiver,
            allocation::tests::expected(),
            request(Kind::Admit),
            deadline,
            reply,
        )
    });
    loop {
        match receive.try_recv() {
            Ok(value) => {
                assert!(value.is_ok());
                break;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                assert!(control.now() < deadline);
                std::thread::sleep(TURN);
            }
            Err(_) => panic!("admission owner closed"),
        }
    }
    assert!(!path.exists());
    while control.now() <= deadline {
        std::thread::sleep(TURN);
    }
    actor.join().unwrap();
    peer.join().unwrap();
    let (reply, _receive) = tokio::sync::oneshot::channel();
    assert!(
        sender
            .try_send(Work {
                request: request(Kind::Preflight),
                deadline,
                reply
            })
            .is_err()
    );
    assert!(!path.exists());
    assert!(control.stop.load(Ordering::Acquire));
}
