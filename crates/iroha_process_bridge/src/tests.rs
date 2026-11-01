//! Actual compiled-child controls at the C ABI boundary.
use super::*;
use std::{
    fs,
    io::Write as _,
    path::Path,
    time::{Duration, Instant},
};

#[test]
#[ignore = "subprocess entry explicitly selected by parent controls"]
fn original_child() {
    let Ok(mode) = std::env::var("IROHA_BRIDGE_TEST_MODE") else {
        return;
    };
    let release = std::env::var("IROHA_BRIDGE_TEST_RELEASE").unwrap();
    if mode == "leaf" {
        while !Path::new(&release).exists() {
            std::thread::sleep(Duration::from_millis(5));
        }
        return;
    }
    if mode == "root" {
        let mut leaf = std::process::Command::new(std::env::current_exe().unwrap());
        leaf.args([
            "--ignored",
            "--exact",
            "tests::original_child",
            "--nocapture",
        ])
        .env("IROHA_BRIDGE_TEST_MODE", "leaf")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit());
        let _leaf = leaf.spawn().unwrap();
        std::io::stdout()
            .write_all(b"original-root-exit\n")
            .unwrap();
        std::process::exit(0);
    }
    if mode == "wait" {
        println!("original-child-running");
        while !Path::new(&release).exists() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn launch(program: &Path, directory: &Path, mode: &str, release: &Path) -> (i32, u64) {
    let mut environment = BTreeMap::from([
        ("IROHA_BRIDGE_TEST_MODE".to_owned(), mode.to_owned()),
        (
            "IROHA_BRIDGE_TEST_RELEASE".to_owned(),
            release.to_str().unwrap().to_owned(),
        ),
    ]);
    if let Some(value) = std::env::var_os("SystemRoot") {
        environment.insert("SystemRoot".into(), value.into_string().unwrap());
    }
    let request = norito::json!({"program":(program.to_str().unwrap()),"directory":(directory.to_str().unwrap()),
        "arguments":["--ignored","--exact","tests::original_child","--nocapture"],"environment":environment});
    let bytes = norito::json::to_vec(&request).unwrap();
    let mut token = 0;
    // SAFETY: both request and aligned independent output live through this call.
    let result = unsafe { iroha_process_spawn_v1(bytes.as_ptr(), bytes.len(), &raw mut token) };
    (result, token)
}

#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn terminal(token: u64) -> i64 {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let mut status = 12345;
        // SAFETY: this aligned output remains writable for the call.
        let observed = unsafe { iroha_process_try_wait_v1(token, &raw mut status) };
        if observed == OK {
            return status;
        }
        assert_eq!(observed, PENDING);
        assert!(Instant::now() < until, "original tree did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn output(token: u64) -> Vec<u8> {
    let mut result = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let mut count = 0;
        // SAFETY: independent writable buffer/count satisfy the complete ABI contract.
        let status = unsafe {
            iroha_process_read_v1(token, buffer.as_mut_ptr(), buffer.len(), &raw mut count)
        };
        if status == EOF {
            assert_eq!(count, 0);
            return result;
        }
        assert_eq!(status, OK);
        result.extend_from_slice(&buffer[..count]);
    }
}

#[test]
#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn close_refuses_live_original_and_invalid_reads_do_not_release_it() {
    let directory = tempfile::tempdir().unwrap();
    let release = directory.path().join("release");
    let (status, token) = launch(
        &std::env::current_exe().unwrap(),
        directory.path(),
        "wait",
        &release,
    );
    assert_eq!(status, OK);
    assert_ne!(token, 0);
    assert_eq!(iroha_process_close_v1(token), PENDING);
    let mut count = 0;
    // SAFETY: null data is explicitly refused before it is dereferenced.
    assert_eq!(
        unsafe { iroha_process_read_v1(token, std::ptr::null_mut(), 1, &raw mut count) },
        INVALID
    );
    assert_eq!(iroha_process_close_v1(token), PENDING);
    fs::write(&release, b"owned release").unwrap();
    assert!(
        String::from_utf8(output(token))
            .unwrap()
            .contains("original-child-running")
    );
    assert_eq!(terminal(token), 0);
    assert_eq!(iroha_process_close_v1(token), OK);
    assert_eq!(iroha_process_close_v1(token), UNKNOWN);
}

#[test]
#[allow(
    unsafe_code,
    reason = "exercise the documented C ABI against two independently owned fixture processes"
)]
fn explicit_termination_affects_only_the_selected_original_session() {
    let directory = tempfile::tempdir().unwrap();
    let releases = [
        directory.path().join("first"),
        directory.path().join("second"),
    ];
    let mut launches = [None, None];
    let result = catch_unwind(AssertUnwindSafe(|| {
        for (index, release) in releases.iter().enumerate() {
            let observed = launch(
                &std::env::current_exe().unwrap(),
                directory.path(),
                "wait",
                release,
            );
            launches[index] = Some(observed);
            assert_eq!(observed.0, OK);
        }
        let tokens = launches.map(|value| value.unwrap().1);
        assert_eq!(iroha_process_kill_v1(u64::MAX), UNKNOWN);
        assert_eq!(iroha_process_kill_v1(tokens[0]), OK);
        assert_ne!(terminal(tokens[0]), 0);
        let mut untouched = 12345;
        // SAFETY: this independent aligned status output lives throughout the call.
        assert_eq!(
            unsafe { iroha_process_try_wait_v1(tokens[1], &raw mut untouched) },
            PENDING
        );
        assert_eq!(untouched, 12345);
        assert_eq!(iroha_process_close_v1(tokens[1]), PENDING);
    }));
    // Release these fixture processes through their own protocol even if an
    // assertion failed; this does not terminate an unrelated process or compiler.
    for release in &releases {
        fs::write(release, b"owned fixture release").unwrap();
    }
    for (index, observed) in launches.into_iter().enumerate() {
        if let Some((status, token)) = observed {
            if status == OK {
                let bytes = output(token);
                let _status = terminal(token);
                if index == 1 {
                    let other = String::from_utf8(bytes).unwrap();
                    assert!(other.contains("original-child-running"), "{other}");
                }
            }
            if token != 0 {
                assert_eq!(iroha_process_close_v1(token), OK);
            }
        }
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[cfg(windows)]
#[test]
#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn root_terminal_cannot_release_source_custody_while_original_job_descendant_lives() {
    use std::os::windows::io::AsRawHandle as _;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WaitForSingleObject(handle: std::os::windows::io::RawHandle, milliseconds: u32) -> u32;
    }
    let directory = tempfile::tempdir().unwrap();
    let release = directory.path().join("release");
    let (status, token) = launch(
        &std::env::current_exe().unwrap(),
        directory.path(),
        "root",
        &release,
    );
    assert_eq!(status, OK);
    // A descendant may inherit an additional stdout writer. Read the bounded
    // readiness marker, never require pipe EOF before releasing that descendant.
    let mut marker = Vec::new();
    let mut buffer = [0_u8; 256];
    while !marker
        .windows(b"original-root-exit\n".len())
        .any(|part| part == b"original-root-exit\n")
    {
        let mut count = 0;
        // SAFETY: independent writable bounded regions live through this call.
        assert_eq!(
            unsafe {
                iroha_process_read_v1(token, buffer.as_mut_ptr(), buffer.len(), &raw mut count)
            },
            OK
        );
        marker.extend_from_slice(&buffer[..count]);
        assert!(marker.len() <= 4096, "original marker exceeded its bound");
    }
    let session = original(token).unwrap();
    {
        let retained = session.child.lock().unwrap();
        let child = retained.as_ref().unwrap();
        // SAFETY: the same retained owner keeps its original process HANDLE live.
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle(), 10_000) },
            0
        );
    }
    let mut exit = 12345;
    // SAFETY: aligned independent output is writable for the call.
    assert_eq!(
        unsafe { iroha_process_try_wait_v1(token, &raw mut exit) },
        PENDING
    );
    assert_eq!(exit, 12345);
    assert_eq!(iroha_process_close_v1(token), PENDING);
    fs::write(&release, b"owned descendant release").unwrap();
    assert_eq!(terminal(token), 0);
    let _remaining_original_stdout = output(token);
    assert_eq!(iroha_process_close_v1(token), OK);
}

#[test]
fn explicit_spawn_refusal_still_returns_a_closable_owned_token() {
    let directory = tempfile::tempdir().unwrap();
    let (status, token) = launch(
        &directory.path().join("no-such-original"),
        directory.path(),
        "wait",
        &directory.path().join("release"),
    );
    assert_eq!(status, IO);
    assert_ne!(token, 0);
    assert_eq!(iroha_process_close_v1(token), OK);
}

#[test]
fn unavailable_custody_slot_refuses_before_native_creation() {
    for slot in 0..3 {
        let session = Session {
            child: Mutex::new(None),
            stdout: Mutex::new(None),
            refused: Mutex::new(false),
        };
        let poisoned = catch_unwind(AssertUnwindSafe(|| match slot {
            0 => {
                let _held = session.child.lock().unwrap();
                panic!("injected child custody poison");
            }
            1 => {
                let _held = session.stdout.lock().unwrap();
                panic!("injected stream custody poison");
            }
            _ => {
                let _held = session.refused.lock().unwrap();
                panic!("injected refusal custody poison");
            }
        }));
        assert!(poisoned.is_err());
        let invoked = std::cell::Cell::new(false);
        assert_eq!(
            start_original(&session, || {
                invoked.set(true);
                ProcessCommand::new(std::env::current_exe().unwrap())?.spawn()
            }),
            Err(PANIC)
        );
        assert!(
            !invoked.get(),
            "native creation ran without destination custody"
        );
        let child = session
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(child.is_none());
    }
}

#[test]
#[allow(
    unsafe_code,
    reason = "exercise the actual documented C ABI with live bounded buffers and original handles"
)]
fn malformed_and_excessive_requests_never_allocate_process_owners() {
    let mut token = 99;
    // SAFETY: request/output are valid; the over-limit extent is refused before reading.
    assert_eq!(
        unsafe { iroha_process_spawn_v1(b"{}".as_ptr(), MAX_REQUEST + 1, &raw mut token) },
        INVALID
    );
    assert_eq!(token, 0);
    // SAFETY: the two-byte actual request and independent output remain valid.
    assert_eq!(
        unsafe { iroha_process_spawn_v1(b"{}".as_ptr(), 2, &raw mut token) },
        INVALID
    );
    assert_eq!(token, 0);
    assert_eq!(iroha_process_close_v1(u64::MAX), UNKNOWN);
}
