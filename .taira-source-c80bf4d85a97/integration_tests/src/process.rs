//! Bounded process helpers for integration tests.

use std::{
    future::Future,
    io::{self, Read},
    pin::Pin,
    process::{Command, ExitStatus, Output, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use crate::timeouts::read_env_duration;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    task::JoinHandle,
};

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
    configure_std_command(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    let tree = match ProcessTree::for_std_child(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
    };
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
            terminate_std_child(&tree, &mut child);
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
pub fn status_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<ExitStatus> {
    configure_std_command(command);
    command.stdin(Stdio::null());
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
    let tree = match ProcessTree::for_std_child(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
    };
    let Some(status) = wait_for_exit(&mut child, timeout)? else {
        terminate_std_child(&tree, &mut child);
        let _ = wait_for_exit(&mut child, PROCESS_CLEANUP_TIMEOUT);
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
    configure_tokio_command(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
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
            terminate_tokio_child(&tree, &mut child);
            let _ = tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, child.wait()).await;
            let _ = drain_async_pipe(&mut stdout_reader, PROCESS_CLEANUP_TIMEOUT).await;
            let _ = drain_async_pipe(&mut stderr_reader, PROCESS_CLEANUP_TIMEOUT).await;
            return Err(command_timeout_error(&command_debug, timeout));
        }
        tokio::time::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed))).await;
    }
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
    configure_tokio_command(command);
    command.stdin(Stdio::null()).kill_on_drop(true);
    let command_debug = format!("{command:?}");
    let mut child = command.spawn()?;
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
            terminate_tokio_child(&tree, &mut child);
            let _ = tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, child.wait()).await;
            return Err(command_timeout_error(&command_debug, timeout));
        }
        tokio::time::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed))).await;
    }
}

fn configure_std_command(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        command.process_group(0);
    }
}

fn configure_tokio_command(command: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
}

struct ProcessTree {
    #[cfg(not(windows))]
    child_id: u32,
    #[cfg(windows)]
    job: windows_process::Job,
}

impl ProcessTree {
    fn for_std_child(child: &std::process::Child) -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                job: windows_process::Job::new(child)?,
            })
        }
        #[cfg(not(windows))]
        {
            let child_id = child.id();
            Ok(Self { child_id })
        }
    }

    fn for_tokio_child(child: &tokio::process::Child) -> io::Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                job: windows_process::Job::new(child)?,
            })
        }
        #[cfg(not(windows))]
        {
            let child_id = child
                .id()
                .ok_or_else(|| io::Error::other("spawned Tokio child has no process id"))?;
            Ok(Self { child_id })
        }
    }

    fn terminate(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            terminate_unix_process_group(self.child_id)
        }
        #[cfg(windows)]
        {
            self.job.terminate()
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "process-tree termination is unsupported on this platform",
            ))
        }
    }
}

fn terminate_std_child(tree: &ProcessTree, child: &mut std::process::Child) {
    if tree.terminate().is_err() {
        let _ = child.kill();
    }
}

fn terminate_tokio_child(tree: &ProcessTree, child: &mut tokio::process::Child) {
    if tree.terminate().is_err() {
        let _ = child.start_kill();
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn terminate_unix_process_group(child_id: u32) -> io::Result<()> {
    let process_group = i32::try_from(child_id)
        .map_err(|_| io::Error::other(format!("process id {child_id} does not fit pid_t")))?;
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;

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

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_process {
    use std::{
        ffi::c_void,
        io,
        mem::size_of,
        os::windows::io::{AsRawHandle, RawHandle},
        ptr,
    };

    type Handle = *mut c_void;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    impl IoCounters {
        const fn empty() -> Self {
            Self {
                read_operation_count: 0,
                write_operation_count: 0,
                other_operation_count: 0,
                read_transfer_count: 0,
                write_transfer_count: 0,
                other_transfer_count: 0,
            }
        }
    }

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    impl JobObjectBasicLimitInformation {
        const fn kill_on_close() -> Self {
            Self {
                per_process_user_time_limit: 0,
                per_job_user_time_limit: 0,
                limit_flags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                minimum_working_set_size: 0,
                maximum_working_set_size: 0,
                active_process_limit: 0,
                affinity: 0,
                priority_class: 0,
                scheduling_class: 0,
            }
        }
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    impl JobObjectExtendedLimitInformation {
        const fn kill_on_close() -> Self {
            Self {
                basic_limit_information: JobObjectBasicLimitInformation::kill_on_close(),
                io_info: IoCounters::empty(),
                process_memory_limit: 0,
                job_memory_limit: 0,
                peak_process_memory_used: 0,
                peak_job_memory_used: 0,
            }
        }
    }

    unsafe extern "system" {
        fn CreateJobObjectW(job_attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            job_object_information_class: i32,
            job_object_information: *mut c_void,
            job_object_information_length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn CloseHandle(object: Handle) -> i32;
    }

    pub(super) struct Job {
        handle: Handle,
    }

    impl Job {
        pub(super) fn new<T>(child: &T) -> io::Result<Self>
        where
            T: AsRawHandle,
        {
            let handle = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }

            let job = Self { handle };
            let mut info = JobObjectExtendedLimitInformation::kill_on_close();
            let info_len = u32::try_from(size_of::<JobObjectExtendedLimitInformation>())
                .expect("job object limit information size fits u32");
            let ok = unsafe {
                SetInformationJobObject(
                    job.handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    (&mut info as *mut JobObjectExtendedLimitInformation).cast(),
                    info_len,
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }

            let process = child.as_raw_handle();
            let ok = unsafe { AssignProcessToJobObject(job.handle, raw_handle_to_handle(process)) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(job)
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            if unsafe { TerminateJobObject(self.handle, 1) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(self.handle) };
        }
    }

    fn raw_handle_to_handle(handle: RawHandle) -> Handle {
        handle.cast()
    }
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

    fn pipe_holding_descendant_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args([
                "/C",
                "start /B \"\" cmd /C \"ping -n 6 127.0.0.1\" & echo ready",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 5 & printf ready"]);
            command
        }
    }

    fn tokio_pipe_holding_descendant_command() -> tokio::process::Command {
        if cfg!(windows) {
            let mut command = tokio::process::Command::new("cmd");
            command.args([
                "/C",
                "start /B \"\" cmd /C \"ping -n 6 127.0.0.1\" & echo ready",
            ]);
            command
        } else {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "sleep 5 & printf ready"]);
            command
        }
    }

    #[test]
    fn output_with_timeout_fails_fast() {
        let err =
            output_with_timeout(&mut sleeping_command(), Duration::from_millis(20)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn output_with_timeout_terminates_pipe_holding_descendants() {
        let started = Instant::now();
        let err = output_with_timeout(
            &mut pipe_holding_descendant_command(),
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout cleanup took {:?}",
            started.elapsed()
        );
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

    #[tokio::test]
    async fn tokio_output_with_timeout_terminates_pipe_holding_descendants() {
        let started = Instant::now();
        let err = tokio_output_with_timeout(
            &mut tokio_pipe_holding_descendant_command(),
            Duration::from_millis(50),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout cleanup took {:?}",
            started.elapsed()
        );
    }
}
