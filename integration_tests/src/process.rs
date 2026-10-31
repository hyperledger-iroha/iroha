//! Bounded process helpers for integration tests.
use crate::timeouts::read_env_duration;
use iroha_fs::{ProcessCommand as Command, ProcessStdio as Stdio};
use iroha_test_network::{ResolvedAsyncChild, ResolvedChild, ResolvedCommand};
use std::{
    io::{self, Read},
    process::{ExitStatus, Output},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    task::JoinHandle,
};
/// Owned launch request; qualified commands retain their original admission owners.
#[doc(hidden)]
#[derive(Debug)]
pub struct BoundedCommand(ResolvedCommand);
impl From<Command> for BoundedCommand {
    fn from(command: Command) -> Self {
        Self(ResolvedCommand::from_development_command(command))
    }
}
impl From<ResolvedCommand> for BoundedCommand {
    fn from(command: ResolvedCommand) -> Self {
        Self(command)
    }
}
impl BoundedCommand {
    fn prepare(&mut self, capture: bool) {
        self.0.kill_on_drop(true).configure(|options| {
            #[cfg(unix)]
            options.process_group(0);
            options.stdin(Stdio::null());
            if capture {
                options.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
        });
    }
    fn spawn(self) -> io::Result<ResolvedChild> {
        self.0.spawn()
    }
    fn spawn_async(self) -> io::Result<ResolvedAsyncChild> {
        self.0.spawn_async()
    }
}

/// Environment override for ordinary test subprocess timeouts.
pub const PROCESS_TIMEOUT_ENV: &str = "IROHA_TEST_PROCESS_TIMEOUT_MS";
/// Environment override for nested build subprocess timeouts.
pub const BUILD_TIMEOUT_ENV: &str = "IROHA_TEST_BUILD_TIMEOUT_MS";
const PROCESS_TIMEOUT_DEFAULT: Duration = Duration::from_secs(60);
const BUILD_TIMEOUT_DEFAULT: Duration = Duration::from_secs(20 * 60);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROCESS_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
/// Default timeout for ordinary test subprocesses.
#[must_use]
pub fn process_timeout() -> Duration {
    read_env_duration(PROCESS_TIMEOUT_ENV, PROCESS_TIMEOUT_DEFAULT)
}
/// Default timeout for nested build/check subprocesses.
#[must_use]
pub fn build_timeout() -> Duration {
    read_env_duration(BUILD_TIMEOUT_ENV, BUILD_TIMEOUT_DEFAULT)
}
/// Run a command to completion with piped output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub fn output_with_timeout(
    command: impl Into<BoundedCommand>,
    timeout: Duration,
) -> io::Result<Output> {
    let mut command = command.into();
    command.prepare(true);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    #[cfg(unix)]
    let tree = ProcessTree::for_std_child(&child);
    let mut stdout_reader = child.stdout.take().map(read_pipe);
    let mut stderr_reader = child.stderr.take().map(read_pipe);
    let mut stdout = stdout_reader.is_none().then(Vec::new);
    let mut stderr = stderr_reader.is_none().then(Vec::new);
    let mut status = None;
    let started = Instant::now();
    loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if stdout.is_none() {
            stdout = poll_pipe(&mut stdout_reader);
        }
        if stderr.is_none() {
            stderr = poll_pipe(&mut stderr_reader);
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            return Ok(Output {
                status: status.take().expect("status present"),
                stdout: stdout.take().expect("stdout present"),
                stderr: stderr.take().expect("stderr present"),
            });
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            terminate_std_child(
                #[cfg(unix)]
                &tree,
                &mut child,
            );
            let _ = wait_for_exit(&mut child, PROCESS_CLEANUP_TIMEOUT);
            let _ = drain_pipe(&mut stdout_reader, PROCESS_CLEANUP_TIMEOUT);
            let _ = drain_pipe(&mut stderr_reader, PROCESS_CLEANUP_TIMEOUT);
            return Err(command_timeout_error(&command_debug, timeout));
        }
        thread::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed)));
    }
}
/// Run a command to completion with inherited output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub fn status_with_timeout(
    command: impl Into<BoundedCommand>,
    timeout: Duration,
) -> io::Result<ExitStatus> {
    let mut command = command.into();
    command.prepare(false);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    #[cfg(unix)]
    let tree = ProcessTree::for_std_child(&child);
    let Some(status) = wait_for_exit(&mut child, timeout)? else {
        terminate_std_child(
            #[cfg(unix)]
            &tree,
            &mut child,
        );
        let _ = wait_for_exit(&mut child, PROCESS_CLEANUP_TIMEOUT);
        return Err(command_timeout_error(&command_debug, timeout));
    };
    Ok(status)
}
/// Run an owned command asynchronously with piped output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub async fn tokio_output_with_timeout(
    command: impl Into<BoundedCommand> + Send,
    timeout: Duration,
) -> io::Result<Output> {
    let mut command = command.into();
    command.prepare(true);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn_async()?;
    #[cfg(unix)]
    let tree = match ProcessTree::for_tokio_child(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(err);
        }
    };
    let mut stdout_reader = child.stdout.take().map(read_async_pipe);
    let mut stderr_reader = child.stderr.take().map(read_async_pipe);
    let mut stdout = stdout_reader.is_none().then(Vec::new);
    let mut stderr = stderr_reader.is_none().then(Vec::new);
    let mut status = None;
    let started = Instant::now();
    loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if stdout.is_none() {
            stdout = poll_async_pipe(&mut stdout_reader).await;
        }
        if stderr.is_none() {
            stderr = poll_async_pipe(&mut stderr_reader).await;
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            return Ok(Output {
                status: status.take().expect("status present"),
                stdout: stdout.take().expect("stdout present"),
                stderr: stderr.take().expect("stderr present"),
            });
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            terminate_tokio_child(
                #[cfg(unix)]
                &tree,
                &mut child,
            );
            let _ = tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, child.wait()).await;
            let _ = drain_async_pipe(&mut stdout_reader, PROCESS_CLEANUP_TIMEOUT).await;
            let _ = drain_async_pipe(&mut stderr_reader, PROCESS_CLEANUP_TIMEOUT).await;
            return Err(command_timeout_error(&command_debug, timeout));
        }
        tokio::time::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed))).await;
    }
}
/// Run an owned command asynchronously with inherited output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub async fn tokio_status_with_timeout(
    command: impl Into<BoundedCommand> + Send,
    timeout: Duration,
) -> io::Result<ExitStatus> {
    let mut command = command.into();
    command.prepare(false);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn_async()?;
    #[cfg(unix)]
    let tree = match ProcessTree::for_tokio_child(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(err);
        }
    };
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            terminate_tokio_child(
                #[cfg(unix)]
                &tree,
                &mut child,
            );
            let _ = tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, child.wait()).await;
            return Err(command_timeout_error(&command_debug, timeout));
        }
        tokio::time::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed))).await;
    }
}
// Unix process groups retain their existing cleanup policy. Windows children
// already own their creation-time Job in the shared native process owner.
#[cfg(unix)]
struct ProcessTree {
    child_id: u32,
}
#[cfg(unix)]
impl ProcessTree {
    fn for_std_child(child: &ResolvedChild) -> Self {
        Self {
            child_id: child.id(),
        }
    }
    fn for_tokio_child(child: &ResolvedAsyncChild) -> io::Result<Self> {
        let child_id = child
            .id()
            .ok_or_else(|| io::Error::other("spawned child has no process id"))?;
        Ok(Self { child_id })
    }
    fn terminate(&self) -> io::Result<()> {
        terminate_unix_process_group(self.child_id)
    }
}
fn terminate_std_child(#[cfg(unix)] tree: &ProcessTree, child: &mut ResolvedChild) {
    #[cfg(unix)]
    if tree.terminate().is_ok() {
        return;
    }
    let _ = child.kill();
}
fn terminate_tokio_child(#[cfg(unix)] tree: &ProcessTree, child: &mut ResolvedAsyncChild) {
    #[cfg(unix)]
    if tree.terminate().is_ok() {
        return;
    }
    let _ = child.start_kill();
}
#[cfg(unix)]
#[allow(unsafe_code)]
fn terminate_unix_process_group(child_id: u32) -> io::Result<()> {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;
    let process_group = i32::try_from(child_id)
        .map_err(|_| io::Error::other(format!("process id {child_id} does not fit pid_t")))?;
    if unsafe { kill(-process_group, SIGKILL) } == 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(3) {
            Ok(())
        } else {
            Err(err)
        }
    }
}
struct PipeReader {
    receiver: Receiver<Vec<u8>>,
}
fn read_pipe<R>(mut pipe: R) -> PipeReader
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    PipeReader { receiver }
}
fn poll_pipe(reader: &mut Option<PipeReader>) -> Option<Vec<u8>> {
    match reader.as_ref()?.receiver.try_recv() {
        Ok(bytes) => {
            *reader = None;
            Some(bytes)
        }
        Err(TryRecvError::Disconnected) => {
            *reader = None;
            Some(Vec::new())
        }
        Err(TryRecvError::Empty) => None,
    }
}
fn drain_pipe(reader: &mut Option<PipeReader>, timeout: Duration) -> Vec<u8> {
    reader
        .take()
        .and_then(|reader| reader.receiver.recv_timeout(timeout).ok())
        .unwrap_or_default()
}
fn read_async_pipe<R>(mut pipe: R) -> JoinHandle<Vec<u8>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes).await;
        bytes
    })
}
async fn poll_async_pipe(reader: &mut Option<JoinHandle<Vec<u8>>>) -> Option<Vec<u8>> {
    if !reader.as_ref().is_some_and(JoinHandle::is_finished) {
        return None;
    }
    let handle = reader.take()?;
    Some(handle.await.unwrap_or_default())
}
async fn drain_async_pipe(reader: &mut Option<JoinHandle<Vec<u8>>>, timeout: Duration) -> Vec<u8> {
    let Some(mut handle) = reader.take() else {
        return Vec::new();
    };
    tokio::select! {
        result = &mut handle => result.unwrap_or_default(),
        () = tokio::time::sleep(timeout) => {
            handle.abort();
            Vec::new()
        }
    }
}
fn wait_for_exit(child: &mut ResolvedChild, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return Ok(None);
        }
        thread::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed)));
    }
}
fn command_timeout_error(command_debug: &str, timeout: Duration) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!("command {command_debug} timed out after {timeout:?}"),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    const MODE: &str = "IROHA_BOUNDED_PROCESS_FIXTURE_MODE";
    const READY: &str = "IROHA_BOUNDED_PROCESS_FIXTURE_READY";
    const DEADLINE: Duration = Duration::from_secs(1);

    fn fixture_name() -> String {
        let (_, module) = module_path!().split_once("::").unwrap();
        format!("{module}::bounded_process_child_fixture")
    }

    fn fixture_command(mode: &str, ready: &std::path::Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap()).unwrap();
        command
            .args([
                "--exact",
                &fixture_name(),
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(MODE, mode)
            .env(READY, ready);
        command
    }

    #[test]
    #[ignore = "executed as a genuine child by the bounded-process controls"]
    #[allow(
        clippy::zombie_processes,
        reason = "the fixture parent must exit before its descendant so the actual outer supervisor proves cleanup"
    )]
    fn bounded_process_child_fixture() {
        let mode = std::env::var(MODE).expect("fixture invocation mode");
        let ready = std::env::var_os(READY).expect("fixture readiness path");
        if mode == "descendant" {
            // This actual descendant deliberately uses ordinary native inheritance.
            // Its containing original Job/process group must already exist at creation.
            let _child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &fixture_name(),
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(MODE, "leaf")
                .env(READY, ready)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .expect("spawn genuine pipe-holding descendant");
            return;
        }
        assert!(matches!(mode.as_str(), "sleep" | "leaf"));
        std::fs::write(ready, mode.as_bytes()).expect("record actual child execution");
        std::io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(60));
    }

    fn assert_timeout(
        error: &io::Error,
        ready: &std::path::Path,
        expected: &str,
        started: Instant,
    ) {
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            std::fs::read_to_string(ready).unwrap(),
            expected,
            "the intended root/descendant must execute before the deadline"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout cleanup took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn output_with_timeout_fails_fast() {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let started = Instant::now();
        let error = output_with_timeout(fixture_command("sleep", &ready), DEADLINE).unwrap_err();
        assert_timeout(&error, &ready, "sleep", started);
    }

    #[test]
    fn output_with_timeout_terminates_pipe_holding_descendants() {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let started = Instant::now();
        let error =
            output_with_timeout(fixture_command("descendant", &ready), DEADLINE).unwrap_err();
        assert_timeout(&error, &ready, "leaf", started);
    }

    #[tokio::test]
    async fn tokio_output_with_timeout_fails_fast() {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let started = Instant::now();
        let error = tokio_output_with_timeout(fixture_command("sleep", &ready), DEADLINE)
            .await
            .unwrap_err();
        assert_timeout(&error, &ready, "sleep", started);
    }

    #[tokio::test]
    async fn tokio_output_with_timeout_terminates_pipe_holding_descendants() {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let started = Instant::now();
        let error = tokio_output_with_timeout(fixture_command("descendant", &ready), DEADLINE)
            .await
            .unwrap_err();
        assert_timeout(&error, &ready, "leaf", started);
    }
}
