//! Bounded process helpers for integration tests.

use std::{
    future::Future,
    io::{self, Read},
    pin::Pin,
    process::{Command, ExitStatus, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::timeouts::read_env_duration;

/// Environment override for ordinary test subprocess timeouts.
pub const PROCESS_TIMEOUT_ENV: &str = "IROHA_TEST_PROCESS_TIMEOUT_MS";
/// Environment override for nested build subprocess timeouts.
pub const BUILD_TIMEOUT_ENV: &str = "IROHA_TEST_BUILD_TIMEOUT_MS";

const PROCESS_TIMEOUT_DEFAULT: Duration = Duration::from_secs(60);
const BUILD_TIMEOUT_DEFAULT: Duration = Duration::from_secs(20 * 60);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

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

/// Extension methods that run std commands with the default integration-test timeout.
pub trait CommandTimeoutExt {
    /// Run a command to completion with piped output and the default timeout.
    ///
    /// # Errors
    ///
    /// Returns process errors or a timeout error.
    fn bounded_output(&mut self) -> io::Result<Output>;

    /// Run a command to completion with inherited output and the default timeout.
    ///
    /// # Errors
    ///
    /// Returns process errors or a timeout error.
    fn bounded_status(&mut self) -> io::Result<ExitStatus>;
}

impl CommandTimeoutExt for Command {
    fn bounded_output(&mut self) -> io::Result<Output> {
        output_with_timeout(self, process_timeout())
    }

    fn bounded_status(&mut self) -> io::Result<ExitStatus> {
        status_with_timeout(self, process_timeout())
    }
}

/// Extension methods that run Tokio commands with the default integration-test timeout.
pub trait TokioCommandTimeoutExt {
    /// Run a command to completion with piped output and the default timeout.
    fn bounded_output(&mut self) -> Pin<Box<dyn Future<Output = io::Result<Output>> + '_>>;

    /// Run a command to completion with inherited output and the default timeout.
    fn bounded_status(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + '_>>;
}

impl TokioCommandTimeoutExt for tokio::process::Command {
    fn bounded_output(&mut self) -> Pin<Box<dyn Future<Output = io::Result<Output>> + '_>> {
        Box::pin(tokio_output_with_timeout(self, process_timeout()))
    }

    fn bounded_status(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + '_>> {
        Box::pin(tokio_status_with_timeout(self, process_timeout()))
    }
}

/// Run a command to completion with piped output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub fn output_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().map(read_pipe);
    let stderr = child.stderr.take().map(read_pipe);
    let Some(status) = wait_for_exit(&mut child, timeout)? else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = join_pipe(stdout);
        let _ = join_pipe(stderr);
        return Err(command_timeout_error(&command_debug, timeout));
    };

    Ok(Output {
        status,
        stdout: join_pipe(stdout),
        stderr: join_pipe(stderr),
    })
}

/// Run a command to completion with inherited output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub fn status_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<ExitStatus> {
    command.stdin(Stdio::null());
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    let Some(status) = wait_for_exit(&mut child, timeout)? else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(command_timeout_error(&command_debug, timeout));
    };
    Ok(status)
}

/// Run a Tokio command to completion with piped output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub async fn tokio_output_with_timeout(
    command: &mut tokio::process::Command,
    timeout: Duration,
) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let command_debug = format!("{command:?}");
    tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| command_timeout_error(&command_debug, timeout))?
}

/// Run a Tokio command to completion with inherited output and a timeout.
///
/// # Errors
///
/// Returns process spawn/wait errors or [`io::ErrorKind::TimedOut`] when the
/// process does not exit before `timeout`.
pub async fn tokio_status_with_timeout(
    command: &mut tokio::process::Command,
    timeout: Duration,
) -> io::Result<ExitStatus> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    let command_debug = format!("{command:?}");
    tokio::time::timeout(timeout, command.status())
        .await
        .map_err(|_| command_timeout_error(&command_debug, timeout))?
}

fn read_pipe<R>(mut pipe: R) -> thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

fn join_pipe(handle: Option<thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

fn wait_for_exit(
    child: &mut std::process::Child,
    timeout: Duration,
) -> io::Result<Option<ExitStatus>> {
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

    fn sleeping_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "ping -n 2 127.0.0.1 > nul"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 1"]);
            command
        }
    }

    #[test]
    fn output_with_timeout_fails_fast() {
        let err =
            output_with_timeout(&mut sleeping_command(), Duration::from_millis(20)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn tokio_output_with_timeout_fails_fast() {
        let mut command = if cfg!(windows) {
            let mut command = tokio::process::Command::new("cmd");
            command.args(["/C", "ping -n 2 127.0.0.1 > nul"]);
            command
        } else {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "sleep 1"]);
            command
        };
        let err = tokio_output_with_timeout(&mut command, Duration::from_millis(20))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }
}
