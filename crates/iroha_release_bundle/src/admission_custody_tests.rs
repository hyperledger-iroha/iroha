//! Full admission-owner lifetime and refusal boundaries around actual native children.

use super::*;
use crate::bundle::{AdmissionCustody, Program};
use iroha_fs::ProcessStdio as Stdio;
use std::{
    io::{BufRead as _, BufReader},
    sync::{
        Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug)]
struct GuardState {
    valid: AtomicBool,
    calls: AtomicUsize,
    fail_at: AtomicUsize,
    child_ready: Mutex<Option<PathBuf>>,
    observed_running: AtomicBool,
}

impl Default for GuardState {
    fn default() -> Self {
        Self {
            valid: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
            fail_at: AtomicUsize::new(usize::MAX),
            child_ready: Mutex::new(None),
            observed_running: AtomicBool::new(false),
        }
    }
}

#[derive(Debug)]
struct Guard(Arc<GuardState>);

impl AdmissionCustody for Guard {
    fn revalidate(&self) -> io::Result<()> {
        let count = self.0.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.0.fail_at.load(Ordering::SeqCst) == count {
            if let Some(path) = self.0.child_ready.lock().unwrap().as_ref() {
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    if fs::read(path).is_ok_and(|bytes| bytes == b"ready\n") {
                        self.0.observed_running.store(true, Ordering::SeqCst);
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "native child did not report ready"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            self.0.valid.store(false, Ordering::SeqCst);
        }
        if !self.0.valid.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "test admission original changed",
            ));
        }
        Ok(())
    }
}

fn selected() -> (Fixture, Program, Arc<GuardState>, Weak<Guard>) {
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "guarded", fixture.inputs()).unwrap();
    let state = Arc::new(GuardState::default());
    let guard = Arc::new(Guard(Arc::clone(&state)));
    let weak = Arc::downgrade(&guard);
    let program = bundle
        .program(BinaryKind::Irohad)
        .with_admission_custody(guard)
        .unwrap();
    drop(bundle);
    (fixture, program, state, weak)
}

fn await_guard_drop(weak: &Weak<Guard>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while weak.upgrade().is_some() {
        assert!(Instant::now() < deadline, "admission owner was not reaped");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn input_command(program: &Program) -> crate::execution::Command {
    let mut command = program.command().unwrap();
    command.configure(|options| {
        options
            .arg("input")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
    });
    command
}

#[test]
fn cached_selection_and_configured_command_keep_full_admission_owner() {
    let (_fixture, program, state, weak) = selected();
    let cached = program.clone();
    drop(program);
    assert!(weak.upgrade().is_some());
    cached.diagnostic_path().unwrap();
    let command = cached.command().unwrap();
    drop(cached);
    assert!(weak.upgrade().is_some());
    let calls = state.calls.load(Ordering::SeqCst);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(output.stdout, b"release-original");
    assert!(state.calls.load(Ordering::SeqCst) > calls);
    assert!(weak.upgrade().is_none());
}

#[test]
fn normal_wait_keeps_guard_until_the_terminal_child_wrapper_drops() {
    let (_fixture, program, state, weak) = selected();
    let mut child = input_command(&program).spawn().unwrap();
    drop(program);
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    assert!(child.try_wait().unwrap().is_none());
    let calls = state.calls.load(Ordering::SeqCst);
    drop(input);
    assert_eq!(child.wait().unwrap().code(), Some(19));
    assert!(state.calls.load(Ordering::SeqCst) > calls);
    assert!(weak.upgrade().is_some());
    drop(child);
    assert!(weak.upgrade().is_none());
}

#[test]
fn live_drop_keeps_full_guard_until_external_stdin_closes_and_child_exits() {
    let (_fixture, program, _state, weak) = selected();
    let mut child = input_command(&program).spawn().unwrap();
    drop(program);
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    drop(child);
    assert!(
        weak.upgrade().is_some(),
        "live native reaper lost full guard"
    );
    drop(input);
    await_guard_drop(&weak);
}

#[test]
fn guard_drift_before_spawn_refuses_launch_and_diagnostic_use() {
    let (fixture, program, state, weak) = selected();
    let marker = fixture.root.path().join("pre-spawn-output");
    let mut command = program.command().unwrap();
    command.configure(|options| {
        options.stdout(fs::File::create(&marker).unwrap());
    });
    state.valid.store(false, Ordering::SeqCst);
    assert_eq!(
        program.diagnostic_path().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    drop(program);
    assert_eq!(
        command.spawn().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(fs::read(marker).unwrap(), b"");
    assert!(weak.upgrade().is_none());
}

#[test]
fn post_spawn_guard_failure_reaps_the_actually_running_native_child() {
    let (fixture, program, state, weak) = selected();
    let ready = fixture.root.path().join("post-spawn-ready");
    let mut command = input_command(&program);
    command.configure(|options| {
        options.stdout(fs::File::create(&ready).unwrap());
    });
    *state.child_ready.lock().unwrap() = Some(ready);
    state
        .fail_at
        .store(state.calls.load(Ordering::SeqCst) + 2, Ordering::SeqCst);
    drop(program);
    assert_eq!(
        command.spawn().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(state.observed_running.load(Ordering::SeqCst));
    await_guard_drop(&weak);
}

#[test]
fn terminal_guard_drift_is_reported_after_actual_exit_without_early_release() {
    let (_fixture, program, state, weak) = selected();
    let mut child = input_command(&program).spawn().unwrap();
    drop(program);
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    state.valid.store(false, Ordering::SeqCst);
    drop(input);
    assert_eq!(
        child.wait().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(weak.upgrade().is_some());
    drop(child);
    assert!(weak.upgrade().is_none());
}

#[test]
fn a_second_admission_guard_cannot_replace_the_original_owner() {
    let (_fixture, program, state, weak) = selected();
    let replacement = Arc::new(Guard(Arc::new(GuardState::default())));
    let replacement_weak = Arc::downgrade(&replacement);
    assert_eq!(
        program
            .clone()
            .with_admission_custody(replacement)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(replacement_weak.upgrade().is_none());
    assert!(weak.upgrade().is_some());
    state.valid.store(false, Ordering::SeqCst);
    assert_eq!(
        program.command().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    drop(program);
    assert!(weak.upgrade().is_none());
}

#[test]
fn initial_invalid_guard_is_refused_without_downgrading_the_bundle() {
    let fixture = Fixture::new();
    let bundle = publish(&fixture.owner, "invalid-guard", fixture.inputs()).unwrap();
    let state = Arc::new(GuardState::default());
    state.valid.store(false, Ordering::SeqCst);
    let guard = Arc::new(Guard(state));
    let weak = Arc::downgrade(&guard);
    assert_eq!(
        bundle
            .program(BinaryKind::Irohad)
            .with_admission_custody(guard)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(weak.upgrade().is_none());
    bundle.revalidate().unwrap();
}

#[cfg(unix)]
#[test]
fn restricted_options_preserve_literal_arguments_and_actual_child_process_group() {
    let (_fixture, program, _state, _weak) = selected();
    let mut command = input_command(&program);
    command.configure(|options| {
        options.arg("literal argument").process_group(0);
        assert_eq!(
            options.get_args().collect::<Vec<_>>(),
            ["input", "literal argument"]
        );
    });
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["input", "literal argument"]
    );
    let mut child = command.spawn().unwrap();
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    let output = std::process::Command::new("ps")
        .args(["-o", "pgid=", "-p", &child.id().to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap(),
        child.id()
    );
    drop(input);
    assert_eq!(child.wait().unwrap().code(), Some(19));
}

#[cfg(unix)]
#[test]
fn a_permissive_guard_cannot_bypass_the_original_bundle_policy() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_fixture, program, state, _weak) = selected();
    let calls = state.calls.load(Ordering::SeqCst);
    fs::set_permissions(program.bundle.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(program.revalidate().is_err());
    assert_eq!(state.calls.load(Ordering::SeqCst), calls);
}

#[cfg(feature = "tokio")]
#[test]
fn cancelled_wait_and_destroyed_runtime_keep_the_full_guard_until_eof() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let (_fixture, program, _state, weak) = selected();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (mut child, input, _output) = runtime.block_on(async {
        let mut child = input_command(&program).spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).await.unwrap();
        assert_eq!(line, "ready\n");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), child.wait())
                .await
                .is_err()
        );
        (child, input, output)
    });
    drop(program);
    assert!(child.try_wait().unwrap().is_none());
    drop(runtime);
    drop(child);
    assert!(weak.upgrade().is_some());
    drop(input);
    await_guard_drop(&weak);
}

#[cfg(feature = "tokio")]
#[test]
fn asynchronous_terminal_guard_drift_keeps_full_owner_through_actual_wait() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let (_fixture, program, state, weak) = selected();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut child = input_command(&program).spawn_async().unwrap();
        drop(program);
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).await.unwrap();
        assert_eq!(line, "ready\n");
        state.valid.store(false, Ordering::SeqCst);
        drop(input);
        assert_eq!(
            child.wait().await.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(weak.upgrade().is_some());
        drop(child);
        assert!(weak.upgrade().is_none());
    });
}

#[cfg(feature = "tokio")]
#[test]
fn asynchronous_guard_refusal_before_spawn_never_releases_a_live_child() {
    let (_fixture, program, state, weak) = selected();
    let command = input_command(&program);
    state.valid.store(false, Ordering::SeqCst);
    drop(program);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert_eq!(
            command.spawn_async().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    });
    assert!(weak.upgrade().is_none());
}

#[cfg(feature = "tokio")]
#[test]
fn asynchronous_post_spawn_guard_failure_reaps_actual_child_after_runtime_drop() {
    let (fixture, program, state, weak) = selected();
    let ready = fixture.root.path().join("async-post-spawn-ready");
    let mut command = input_command(&program);
    command.configure(|options| {
        options.stdout(fs::File::create(&ready).unwrap());
    });
    *state.child_ready.lock().unwrap() = Some(ready);
    state
        .fail_at
        .store(state.calls.load(Ordering::SeqCst) + 2, Ordering::SeqCst);
    drop(program);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert_eq!(
            command.spawn_async().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    });
    assert!(state.observed_running.load(Ordering::SeqCst));
    drop(runtime);
    await_guard_drop(&weak);
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "test observes the PID through the child's borrowed original native handle, never reopening by PID"
)]
fn original_process_id(handle: std::os::windows::io::RawHandle) -> u32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetProcessId(process: std::os::windows::io::RawHandle) -> u32;
    }
    // SAFETY: each caller keeps the owning child or an independently duplicated
    // owned process handle alive throughout this observation.
    let pid = unsafe { GetProcessId(handle) };
    assert_ne!(pid, 0, "{}", io::Error::last_os_error());
    pid
}

#[cfg(windows)]
#[test]
fn synchronous_native_handle_is_the_original_process_before_and_after_wait() {
    use std::os::windows::io::{AsHandle as _, AsRawHandle as _};
    let (_fixture, program, _state, weak) = selected();
    let mut child = input_command(&program).spawn().unwrap();
    drop(program);
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    let id = child.id();
    assert_eq!(original_process_id(child.as_raw_handle()), id);
    let independent = child.as_handle().try_clone_to_owned().unwrap();
    assert_eq!(original_process_id(independent.as_raw_handle()), id);
    drop(input);
    assert_eq!(child.wait().unwrap().code(), Some(19));
    assert_eq!(original_process_id(child.as_raw_handle()), id);
    drop(child);
    assert!(weak.upgrade().is_none());
    assert_eq!(original_process_id(independent.as_raw_handle()), id);
}

#[cfg(all(windows, feature = "tokio"))]
#[test]
fn asynchronous_native_handle_is_original_and_absent_after_tokio_reaps() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let (_fixture, program, _state, weak) = selected();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut child = input_command(&program).spawn_async().unwrap();
        drop(program);
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).await.unwrap();
        assert_eq!(line, "ready\n");
        assert_eq!(
            original_process_id(child.raw_handle().unwrap()),
            child.id().unwrap()
        );
        drop(input);
        assert_eq!(child.wait().await.unwrap().code(), Some(19));
        assert!(child.raw_handle().is_none());
        drop(child);
        assert!(weak.upgrade().is_none());
    });
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "observe only the original child process handle, without process recovery"
)]
fn require_original_root_terminal(handle: std::os::windows::io::RawHandle) {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WaitForSingleObject(handle: std::os::windows::io::RawHandle, milliseconds: u32) -> u32;
    }
    // SAFETY: the caller retains its original child while this test observes root exit.
    assert_eq!(unsafe { WaitForSingleObject(handle, 10_000) }, 0);
}

#[cfg(windows)]
#[test]
fn exited_root_and_live_descendant_keep_full_admission_through_drop_reaper() {
    use std::io::Read as _;
    use std::os::windows::io::AsRawHandle as _;
    let (_fixture, program, _state, weak) = selected();
    let mut command = program.command().unwrap();
    command.configure(|options| {
        options
            .arg("root-exit")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
    });
    let mut child = command.spawn().unwrap();
    drop(program);
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    require_original_root_terminal(child.as_raw_handle());
    assert!(child.try_wait().unwrap().is_none());
    drop(child);
    assert!(weak.upgrade().is_some());
    drop(input);
    await_guard_drop(&weak);
    line.clear();
    output.read_to_string(&mut line).unwrap();
    assert!(line.is_empty());
}

#[cfg(all(windows, feature = "tokio"))]
#[test]
fn exited_root_async_cancellation_and_runtime_drop_keep_descendant_admission() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let (_fixture, program, _state, weak) = selected();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (child, input, output) = runtime.block_on(async {
        let mut command = program.command().unwrap();
        command.configure(|options| {
            options
                .arg("root-exit")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
        });
        let mut child = command.spawn_async().unwrap();
        drop(program);
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).await.unwrap();
        assert_eq!(line, "ready\n");
        require_original_root_terminal(child.raw_handle().unwrap());
        assert!(
            tokio::time::timeout(Duration::from_millis(30), child.wait())
                .await
                .is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        (child, input, output)
    });
    drop(runtime);
    drop(child);
    assert!(weak.upgrade().is_some());
    drop(input);
    await_guard_drop(&weak);
    drop(output);
}

#[cfg(all(unix, feature = "tokio"))]
#[test]
fn actual_pipe_conversion_unwind_keeps_full_child_admission_until_reaped() {
    let (_fixture, program, _state, weak) = selected();
    let command = input_command(&program);
    drop(program);
    // An active runtime without an I/O driver exercises Tokio's actual original-pipe
    // registration failure. It must not release Program ahead of the spawned child.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let result = runtime.block_on(async {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| command.spawn_async()))
    });
    assert!(result.is_err());
    drop(runtime);
    await_guard_drop(&weak);
}

#[cfg(feature = "tokio")]
#[test]
fn io_only_runtime_wait_needs_no_timer_driver_and_keeps_full_admission() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let (_fixture, program, _state, weak) = selected();
    let mut builder = tokio::runtime::Builder::new_current_thread();
    // Windows process pipes require only Tokio's blocking adapter. Unix needs
    // its I/O driver, and neither host needs the timer driver for original wait.
    #[cfg(unix)]
    builder.enable_io();
    let runtime = builder.build().unwrap();
    runtime.block_on(async {
        let mut child = input_command(&program).spawn_async().unwrap();
        drop(program);
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).await.unwrap();
        assert_eq!(line, "ready\n");
        assert!(child.try_wait().unwrap().is_none());
        assert_eq!(child.wait().await.unwrap().code(), Some(19));
        assert!(weak.upgrade().is_some());
        assert!(child.id().is_none());
        drop(child);
        assert!(weak.upgrade().is_none());
    });
}
