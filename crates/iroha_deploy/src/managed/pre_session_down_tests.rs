//! Native ownership and authenticated cleanup regressions for pre-session startup.

use super::*;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

thread_local! {
    static BEFORE_WAIT: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    static BEFORE_REQUEST: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    static OWNER_WAITS: RefCell<Option<Vec<Instant>>> = const { RefCell::new(None) };
}

// Record the deadline of every owner wait that follows a validated terminal reply.
pub(super) fn owner_wait(stopped: bool, deadline: Instant) {
    if stopped {
        OWNER_WAITS.with(|waits| {
            if let Some(waits) = waits.borrow_mut().as_mut() {
                waits.push(deadline);
            }
        });
    }
}

pub(super) fn before_wait() {
    let hook = BEFORE_WAIT.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

pub(super) fn before_request() {
    let hook = BEFORE_REQUEST.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

struct ClearHook;

impl Drop for ClearHook {
    fn drop(&mut self) {
        let _ = BEFORE_WAIT.with(|slot| slot.borrow_mut().take());
        let _ = BEFORE_REQUEST.with(|slot| slot.borrow_mut().take());
    }
}

// Serve the one control connection a stop sends, until it arrives or the stop has returned
// (one final accept follows that signal). The worker keeps no wall-clock window of its own:
// the stop's admission spends only the stop's deadline. `None` means no request was sent.
fn accept_until_returned(
    listener: &transport::Listener,
    returned: &AtomicBool,
) -> Option<iroha_managed_ipc::transport::Connection> {
    loop {
        let finished = returned.load(Ordering::SeqCst);
        if let Some(connection) = listener.accept().unwrap() {
            return Some(connection);
        }
        if finished {
            return None;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

// Record when the stop sends its admitted request, on the stop's own thread.
fn record_request_time() -> Arc<std::sync::Mutex<Option<Instant>>> {
    let sent = Arc::new(std::sync::Mutex::new(None));
    let record = Arc::clone(&sent);
    BEFORE_REQUEST.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            *record.lock().unwrap() = Some(Instant::now());
        }));
    });
    sent
}

fn starting(context: ManagedContext) -> ManagedStatus {
    ManagedStatus {
        context,
        phase: ManagedPhase::Starting,
        running_peers: 0,
        failure: None,
        validators: Vec::new(),
    }
}

#[test]
fn pre_session_down_waits_for_owned_failure_and_preserves_retained_generation() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) =
        crate::managed::tests::fixture(&temporary.path().join("managed"), "local");
    directory
        .write_atomic(
            STATUS,
            &encode(&starting(prepared.context.clone())).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let original = generation::read(&directory).unwrap().prepared.context;
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    assert!(
        directory
            .read_optional(WORKER, MAX_METADATA)
            .unwrap()
            .is_none()
    );
    assert!(matches!(store.status("local"), Err(Error::Busy(_))));
    let failed = ManagedStatus {
        context: prepared.context,
        phase: ManagedPhase::Failed,
        running_peers: 0,
        failure: Some("startup program admission failed before control publication".into()),
        validators: Vec::new(),
    };
    let expected = failed.clone();
    let owned_directory = directory.retain().unwrap();
    let (observed, owner) = mpsc::channel();
    let worker = thread::spawn(move || {
        // The signal comes only after the real optional native session read returned None
        // while the original runtime lock remained held. No success or clock is forged.
        owner.recv_timeout(Duration::from_secs(10)).unwrap();
        owned_directory
            .write_atomic(STATUS, &encode(&failed).unwrap(), PublishMode::Replace)
            .unwrap();
        owned_directory.revalidate().unwrap();
        drop(runtime);
    });
    BEFORE_WAIT.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || observed.send(()).unwrap()));
    });
    let _hook = ClearHook;
    let result = store.down("local");
    worker.join().unwrap();
    assert_eq!(result.unwrap(), expected);
    assert_eq!(
        generation::read(&directory).unwrap().prepared.context,
        original
    );
    assert!(
        directory
            .path()
            .join(generation::DIRECTORY)
            .join(MANIFEST)
            .is_file()
    );
    assert!(
        directory
            .read_optional(WORKER, MAX_METADATA)
            .unwrap()
            .is_none()
    );
    assert!(!runtime_owned(&directory).unwrap());
    assert_eq!(store.down("local").unwrap(), expected);
}

#[test]
fn down_does_not_interpret_its_own_operation_lock_as_pending_spawn() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) =
        crate::managed::tests::fixture(&temporary.path().join("managed"), "local");
    directory
        .write_atomic(
            STATUS,
            &encode(&starting(prepared.context.clone())).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    // A separate foreground holder still receives the original Starting observation.
    let foreground = acquire(&directory, "operation.lock", "local").unwrap();
    assert_eq!(store.status("local").unwrap().phase, ManagedPhase::Starting);
    drop(foreground);
    // Down owns both original native gates itself, so no future worker can enter after
    // its companion inherited spawn handoff. A saved Starting record is now a failure.
    let status = store.down("local").unwrap();
    assert_eq!(status.context, prepared.context);
    assert_eq!(status.phase, ManagedPhase::Failed);
    assert_eq!(status.running_peers, 0);
    assert!(status.failure.unwrap().contains("unexpectedly"));
    assert_eq!(
        generation::read(&directory).unwrap().prepared.context,
        prepared.context
    );
}

#[test]
fn pre_session_down_refuses_present_malformed_session_without_clearing_ownership() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) =
        crate::managed::tests::fixture(&temporary.path().join("managed"), "local");
    let status = starting(prepared.context.clone());
    let status_bytes = encode(&status).unwrap();
    directory
        .write_atomic(STATUS, &status_bytes, PublishMode::CreateNew)
        .unwrap();
    directory
        .write_atomic(WORKER, b"malformed", PublishMode::CreateNew)
        .unwrap();
    let _runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let expected = exchange(&directory, ControlAction::Down)
        .unwrap_err()
        .to_string();
    assert_eq!(store.down("local").unwrap_err().to_string(), expected);
    assert!(runtime_owned(&directory).unwrap());
    assert_eq!(
        &*directory.read(STATUS, MAX_METADATA).unwrap(),
        &status_bytes
    );
    assert_eq!(
        generation::read(&directory).unwrap().prepared.context,
        prepared.context
    );
}

#[cfg(any(unix, windows))]
#[test]
fn down_closes_actual_owner_exit_after_session_admission_and_refuses_changed_session() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) =
        crate::managed::tests::fixture(&temporary.path().join("managed"), "local");
    let token = "e".repeat(64);
    let worker_bytes = encode(&WorkerRecord { token }).unwrap();
    directory
        .write_atomic(WORKER, &worker_bytes, PublishMode::CreateNew)
        .unwrap();
    let failed = ManagedStatus {
        context: prepared.context.clone(),
        phase: ManagedPhase::Failed,
        running_peers: 0,
        failure: Some("startup failed after session publication".into()),
        validators: Vec::new(),
    };
    directory
        .write_atomic(
            STATUS,
            &encode(&starting(prepared.context.clone())).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let owned = directory.retain().unwrap();
    let expected = failed.clone();
    BEFORE_REQUEST.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            // Exact worker/session native admission is complete. This genuine owner publishes
            // its terminal failure then releases the actual endpoint and runtime gate.
            owned
                .write_atomic(STATUS, &encode(&failed).unwrap(), PublishMode::Replace)
                .unwrap();
            drop(listener);
            drop(runtime);
        }));
    });
    let _hook = ClearHook;
    assert_eq!(store.down("local").unwrap(), expected);
    assert_eq!(
        directory.read(WORKER, MAX_METADATA).unwrap().as_slice(),
        worker_bytes
    );
    assert!(!runtime_owned(&directory).unwrap());

    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    BEFORE_REQUEST.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || drop(listener)));
    });
    // Retained Failed0 by itself is insufficient while an actual native owner still exists.
    assert!(store.down("local").is_err());
    assert!(runtime_owned(&directory).unwrap());
    drop(runtime);
    assert_eq!(store.down("local").unwrap(), expected);

    #[cfg(unix)]
    for replacement in [
        worker_bytes.clone(),
        encode(&WorkerRecord {
            token: "a".repeat(64),
        })
        .unwrap(),
    ] {
        let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
        let listener = transport::Listener::bind(&directory).unwrap();
        let owned = directory.retain().unwrap();
        BEFORE_REQUEST.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                // Even identical bytes from a new native object cannot replace the admitted
                // session, and a different token is never a cleanup authority fallback.
                owned
                    .write_atomic(WORKER, &replacement, PublishMode::Replace)
                    .unwrap();
                drop(listener);
                drop(runtime);
            }));
        });
        assert!(store.down("local").is_err());
        assert!(!runtime_owned(&directory).unwrap());
        assert_eq!(
            decode::<ManagedStatus>(&directory.read(STATUS, MAX_METADATA).unwrap()).unwrap(),
            expected
        );
        directory
            .write_atomic(WORKER, &worker_bytes, PublishMode::Replace)
            .unwrap();
    }
    assert_eq!(
        generation::read(&directory).unwrap().prepared.context,
        prepared.context
    );
}

#[cfg(any(unix, windows))]
#[test]
fn down_requires_original_context_and_terminal_zero_peer_authenticated_reply() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, prepared) =
        crate::managed::tests::fixture(&temporary.path().join("managed"), "local");
    let status = starting(prepared.context.clone());
    let status_bytes = encode(&status).unwrap();
    directory
        .write_atomic(STATUS, &status_bytes, PublishMode::CreateNew)
        .unwrap();
    let token = "f".repeat(64);
    directory
        .write_atomic(
            WORKER,
            &encode(&WorkerRecord {
                token: token.clone(),
            })
            .unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let mut foreign = status.clone();
    foreign.context.network_id.push('x');
    foreign.phase = ManagedPhase::Stopped;
    let mut ready = status.clone();
    ready.phase = ManagedPhase::Ready;
    let mut live = status;
    live.phase = ManagedPhase::Failed;
    live.running_peers = 1;
    for reply in [foreign, ready, live] {
        let listener = transport::Listener::bind(&directory).unwrap();
        let token = token.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(mut connection) = listener.accept().unwrap() {
                    let request = connection.receive().unwrap();
                    assert_eq!(request.token, token);
                    assert_eq!(request.action, ControlAction::Down);
                    connection.reply(&reply).unwrap();
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
        });
        let result = store.down("local");
        worker.join().unwrap();
        assert!(matches!(result, Err(Error::Invalid(_))), "{result:?}");
        assert!(runtime_owned(&directory).unwrap());
        assert_eq!(
            &*directory.read(STATUS, MAX_METADATA).unwrap(),
            &status_bytes
        );
    }
    drop(runtime);
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let terminal = ManagedStatus {
        context: prepared.context.clone(),
        phase: ManagedPhase::Stopped,
        running_peers: 0,
        failure: None,
        validators: Vec::new(),
    };
    let expected = terminal.clone();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(mut connection) = listener.accept().unwrap() {
                let request = connection.receive().unwrap();
                assert_eq!(request.token, token);
                assert_eq!(request.action, ControlAction::Down);
                connection.reply(&terminal).unwrap();
                drop(runtime);
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
    });
    let result = store.down("local");
    worker.join().unwrap();
    assert_eq!(result.unwrap(), expected);
    assert!(!runtime_owned(&directory).unwrap());
    assert_eq!(
        generation::read(&directory).unwrap().prepared.context,
        prepared.context
    );
}

fn deadline_session(
    root: &std::path::Path,
) -> (ManagedStore, PrivateDirectory, ManagedStatus, String) {
    let (store, directory, prepared) = crate::managed::tests::fixture(root, "local");
    let terminal = ManagedStatus {
        context: prepared.context.clone(),
        phase: ManagedPhase::Stopped,
        running_peers: 0,
        failure: None,
        validators: Vec::new(),
    };
    directory
        .write_atomic(
            STATUS,
            &encode(&starting(prepared.context)).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let token = "a".repeat(64);
    directory
        .write_atomic(
            WORKER,
            &encode(&WorkerRecord {
                token: token.clone(),
            })
            .unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    (store, directory, terminal, token)
}

#[test]
fn down_waits_for_delayed_terminal_reply_and_original_owner_release() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, terminal, token) = deadline_session(&temporary.path().join("managed"));
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let expected = terminal.clone();
    let owned = directory.retain().unwrap();
    let returned = Arc::new(AtomicBool::new(false));
    let worker = {
        let returned = Arc::clone(&returned);
        thread::spawn(move || {
            let Some(mut connection) = accept_until_returned(&listener, &returned) else {
                return false;
            };
            let request = connection.receive().unwrap();
            assert_eq!(request.token, token);
            assert_eq!(request.action, ControlAction::Down);
            thread::sleep(Duration::from_secs(11));
            owned
                .write_atomic(STATUS, &encode(&terminal).unwrap(), PublishMode::Replace)
                .unwrap();
            connection.reply(&terminal).unwrap();
            thread::sleep(Duration::from_millis(100));
            drop(runtime);
            true
        })
    };
    // The production stop budget: a delayed terminal reply and the owner's later release are
    // observed, however long this host takes to admit the stop first.
    let started = Instant::now();
    let result = store.down("local");
    let elapsed = started.elapsed();
    returned.store(true, Ordering::SeqCst);
    assert!(worker.join().unwrap(), "the stop sent its Down request");
    assert_eq!(result.unwrap(), expected);
    assert!(elapsed >= Duration::from_millis(11_100));
    assert!(!runtime_owned(&directory).unwrap());
}

#[test]
fn late_terminal_reply_cannot_extend_down_observation_or_prove_cleanup() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, terminal, _) = deadline_session(&temporary.path().join("managed"));
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let returned = Arc::new(AtomicBool::new(false));
    let (release, held) = mpsc::channel();
    let worker = {
        let returned = Arc::clone(&returned);
        thread::spawn(move || {
            let Some(mut connection) = accept_until_returned(&listener, &returned) else {
                return false;
            };
            assert_eq!(connection.receive().unwrap().action, ControlAction::Down);
            // The terminal reply is withheld until the stop has returned.
            held.recv().unwrap();
            let _ = connection.reply(&terminal);
            drop(runtime);
            true
        })
    };
    // Admit the stop first: the genuine 150 ms deadline below covers the observation phase
    // that production runs after admission under the same deadline.
    let admission = store.admit_down("local", None).unwrap();
    let sent = record_request_time();
    let _hook = ClearHook;
    let started = Instant::now();
    let deadline = started + Duration::from_millis(150);
    let result = observe_down(admission, deadline);
    let elapsed = started.elapsed();
    let still_owned = runtime_owned(&directory).unwrap();
    returned.store(true, Ordering::SeqCst);
    let _ = release.send(());
    assert!(worker.join().unwrap(), "the stop sent its Down request");
    let sent = sent.lock().unwrap().expect("admitted request time");
    assert!(sent < deadline, "Down was sent inside the deadline");
    assert!(result.is_err(), "deadline cannot create terminal evidence");
    assert!(
        still_owned,
        "timeout must preserve original runtime ownership"
    );
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn terminal_reply_does_not_restart_owner_observation_deadline() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, terminal, _) = deadline_session(&temporary.path().join("managed"));
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let returned = Arc::new(AtomicBool::new(false));
    let (release, held) = mpsc::channel();
    let worker = {
        let returned = Arc::clone(&returned);
        thread::spawn(move || {
            let Some(mut connection) = accept_until_returned(&listener, &returned) else {
                return false;
            };
            assert_eq!(connection.receive().unwrap().action, ControlAction::Down);
            connection.reply(&terminal).unwrap();
            // The original owner keeps the runtime until the stop has returned.
            held.recv().unwrap();
            drop(runtime);
            true
        })
    };
    let admission = store.admit_down("local", None).unwrap();
    let _hook = ClearHook;
    OWNER_WAITS.with(|waits| *waits.borrow_mut() = Some(Vec::new()));
    let started = Instant::now();
    let deadline = started + Duration::from_millis(150);
    let result = observe_down(admission, deadline);
    let elapsed = started.elapsed();
    let owner_waits = OWNER_WAITS.with(|waits| waits.borrow_mut().take()).unwrap();
    let still_owned = runtime_owned(&directory).unwrap();
    returned.store(true, Ordering::SeqCst);
    let _ = release.send(());
    assert!(worker.join().unwrap(), "the stop sent its Down request");
    assert!(matches!(result, Err(Error::Busy(_))));
    assert!(still_owned);
    // The validated terminal reply moved the stop to owner observation under its original
    // deadline; no wait after it used a restarted one.
    assert!(
        !owner_waits.is_empty(),
        "terminal reply entered owner observation"
    );
    assert!(owner_waits.iter().all(|wait| *wait == deadline));
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn down_deadline_elapsed_during_admission_sends_no_request() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let (store, directory, _, _) = deadline_session(&temporary.path().join("managed"));
    let runtime = acquire(&directory, "runtime.lock", "local").unwrap();
    let listener = transport::Listener::bind(&directory).unwrap();
    let sent = record_request_time();
    let _hook = ClearHook;
    // The stop takes its deadline before admission: one that elapses there leaves no budget
    // for session admission or the Down request.
    let result = store.down_until("local", Instant::now(), None);
    assert!(matches!(result, Err(Error::Busy(_))));
    assert!(
        sent.lock().unwrap().is_none(),
        "no request after the deadline"
    );
    assert!(listener.accept().unwrap().is_none());
    assert!(runtime_owned(&directory).unwrap());
    drop(runtime);
}
