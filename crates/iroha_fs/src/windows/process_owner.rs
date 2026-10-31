//! General Windows process ownership with Job membership installed at creation.
//!
//! This owner authenticates no executable, source tree or release. A caller with
//! stronger admission requirements must retain its own authority through successful
//! whole-Job `wait`/`try_wait`, including any cancellation or reaper path. No API
//! reconstructs a process from a PID. Process and primary-thread originals remain
//! held together with the unnamed Job until the root has exited and accounting
//! reports no live Job members. No breakaway policy is enabled.
//!
//! Streams are explicit because arbitrary `std::process::Stdio` values cannot be
//! inspected through stable Rust. Only the three stream duplicates are inherited;
//! selected process, thread and Job handles are refused as non-stream objects.
//! Qualification handles are never automatically converted into stream selections;
//! an explicit caller-selected ordinary file stream remains permitted.
//! Temporary inheritable duplicates can still be inherited by an unrelated launcher
//! that concurrently uses unfiltered native handle inheritance. This owner cannot
//! synchronize with the Rust standard library's private spawn mutex.

#![allow(
    unsafe_code,
    reason = "native creation-time Job and explicit original stream ownership"
)]

use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::windows::{
        io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle},
        process::ExitStatusExt as _,
    },
    path::Path,
    process::{ChildStderr, ChildStdin, ChildStdout, Command as Settings, ExitStatus},
    ptr,
    sync::{Arc, Mutex},
    thread,
};
use windows_sys::Win32::{
    Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CREATE_UNICODE_ENVIRONMENT, CreateProcessW, EXTENDED_STARTUPINFO_PRESENT,
        GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
        WaitForSingleObject,
    },
};

#[path = "process_owner/arguments.rs"]
mod arguments;
#[path = "process_owner/attributes.rs"]
mod attributes;
#[path = "process_owner/environment.rs"]
mod environment;
#[path = "process_owner/job.rs"]
mod job;
#[path = "process_owner/streams.rs"]
mod streams;

use attributes::Attributes;
use job::Job;

/// One explicitly selected standard stream; no opaque standard-library command is accepted.
#[derive(Debug, Default)]
pub enum Stream {
    /// Duplicate the current process's corresponding standard stream.
    #[default]
    Inherit,
    /// Connect the child to the native null device.
    Null,
    /// Return the corresponding parent pipe end on the owned child.
    Piped,
    /// Consume an actual original stream/file handle and duplicate only it for the child.
    Owned(OwnedHandle),
}

impl From<File> for Stream {
    fn from(file: File) -> Self {
        Self::Owned(file.into())
    }
}

/// One native launch request with explicit streams and no raw-command escape.
#[derive(Debug)]
pub struct Command {
    // Used solely for stable argument/environment configuration and read-only accessors.
    // This value is never spawned and cannot receive raw native callbacks/creation flags.
    settings: Settings,
    clear_environment: bool,
    stdin: Stream,
    stdout: Stream,
    stderr: Stream,
    kill_on_drop: bool,
}

impl Command {
    /// Select one absolute executable path without PATH lookup or shell interpretation.
    ///
    /// This does not confer file custody; a qualified caller must keep its independent
    /// original executable and ancestry pinned before, during and after this launch.
    pub fn new(program: impl AsRef<Path>) -> io::Result<Self> {
        arguments::absolute(program.as_ref())?;
        Ok(Self {
            settings: Settings::new(program.as_ref()),
            clear_environment: false,
            stdin: Stream::Inherit,
            stdout: Stream::Inherit,
            stderr: Stream::Inherit,
            kill_on_drop: false,
        })
    }

    /// Append a literal CRT-compatible argument, never interpreted by a shell.
    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.settings.arg(argument);
        self
    }
    /// Observe configured arguments without access to the underlying settings owner.
    pub fn get_args(&self) -> std::process::CommandArgs<'_> {
        self.settings.get_args()
    }
    /// Observe the selected executable spelling without granting launch authority.
    pub fn get_program(&self) -> &OsStr {
        self.settings.get_program()
    }
    /// Observe explicit environment deltas.
    pub fn get_envs(&self) -> std::process::CommandEnvs<'_> {
        self.settings.get_envs()
    }

    /// Set a child environment entry; native keys compare case-insensitively.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.settings.env(key, value);
        self
    }
    /// Remove an inherited or explicitly configured environment entry.
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.settings.env_remove(key);
        self
    }
    /// Use an initially empty environment rather than the native process snapshot.
    pub fn env_clear(&mut self) -> &mut Self {
        self.clear_environment = true;
        self.settings.env_clear();
        self
    }
    /// Set an absolute child working directory without mutating the parent's current directory.
    pub fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.settings.current_dir(directory);
        self
    }
    /// Configure the explicit standard input stream.
    pub fn stdin(&mut self, stream: impl Into<Stream>) -> &mut Self {
        self.stdin = stream.into();
        self
    }
    /// Configure the explicit standard output stream.
    pub fn stdout(&mut self, stream: impl Into<Stream>) -> &mut Self {
        self.stdout = stream.into();
        self
    }
    /// Configure the explicit standard error stream.
    pub fn stderr(&mut self, stream: impl Into<Stream>) -> &mut Self {
        self.stderr = stream.into();
        self
    }
    /// Request termination of the original whole Job if the returned child is dropped live.
    pub fn kill_on_drop(&mut self, enabled: bool) -> &mut Self {
        self.kill_on_drop = enabled;
        self
    }

    /// Create the original process already assigned to its retained unnamed Job.
    ///
    /// All fallible settings/stream/attribute preparation occurs before process creation.
    /// Incompatible parent Job policies, unavailable creation attributes and malformed
    /// inputs are errors; there is no post-spawn assignment fallback.
    pub fn spawn(self) -> io::Result<Child> {
        let Self {
            settings,
            clear_environment,
            stdin,
            stdout,
            stderr,
            kill_on_drop,
        } = self;
        let application = arguments::absolute(Path::new(settings.get_program()))?;
        let mut line = arguments::command_line(settings.get_program(), settings.get_args())?;
        let environment = environment::capture(clear_environment, settings.get_envs())?;
        let directory = settings
            .get_current_dir()
            .map(arguments::absolute)
            .transpose()?;
        let mut input = streams::prepare(stdin, 0)?;
        let mut output = streams::prepare(stdout, 1)?;
        let mut error = streams::prepare(stderr, 2)?;
        let job = Job::new()?;
        let inherited = [
            streams::duplicate(input.child.as_raw_handle(), true)?,
            streams::duplicate(output.child.as_raw_handle(), true)?,
            streams::duplicate(error.child.as_raw_handle(), true)?,
        ];
        let handles = inherited.each_ref().map(AsRawHandle::as_raw_handle);
        let jobs = [job.handle().as_raw_handle()];
        let mut attributes = Attributes::new(&handles, &jobs)?;
        // SAFETY: zeroed reserved fields are valid; required fields are initialized below.
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap();
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = handles[0];
        startup.StartupInfo.hStdOutput = handles[1];
        startup.StartupInfo.hStdError = handles[2];
        startup.lpAttributeList = attributes.raw();
        // SAFETY: process-information output is fully initialized by successful creation.
        let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: all pointers reference retained complete buffers; only three intended
        // stream duplicates are inheritable. Job membership is part of this creation,
        // before any new thread executes, and no existing process is discovered by PID.
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                directory.as_ref().map_or(ptr::null(), Vec::as_ptr),
                &raw const startup.StartupInfo,
                &raw mut information,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // Nothing fallible intervenes between creation and taking both original handles.
        // SAFETY: successful CreateProcessW transfers these two distinct owned originals.
        let originals = Originals {
            process: unsafe { OwnedHandle::from_raw_handle(information.hProcess) },
            _thread: unsafe { OwnedHandle::from_raw_handle(information.hThread) },
            job,
            id: information.dwProcessId,
            root_status: None,
        };
        // These parent server endpoints were explicitly created with OVERLAPPED support,
        // as required by the stable standard-library pipe ownership conversions.
        Ok(Child {
            stdin: input.parent.take().map(ChildStdin::from),
            stdout: output.parent.take().map(ChildStdout::from),
            stderr: error.parent.take().map(ChildStderr::from),
            originals: Some(originals),
            kill_on_drop,
        })
    }
}

#[derive(Debug)]
struct Originals {
    process: OwnedHandle,
    // The initial thread is retained even after its terminal state; never rediscovered by ID.
    _thread: OwnedHandle,
    job: Job,
    id: u32,
    root_status: Option<ExitStatus>,
}

impl Originals {
    fn root_status(&mut self, timeout: u32) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.root_status {
            return Ok(Some(status));
        }
        // SAFETY: wait only on the original process handle, which remains owned here.
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), timeout) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => return Ok(None),
            WAIT_FAILED => return Err(io::Error::last_os_error()),
            _ => return Err(io::Error::other("unexpected original process wait result")),
        }
        let mut status = 0;
        // SAFETY: terminal observation precedes querying this same retained original.
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &raw mut status) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let status = ExitStatus::from_raw(status);
        self.root_status = Some(status);
        Ok(Some(status))
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let Some(status) = self.root_status(0)? else {
            return Ok(None);
        };
        Ok((self.job.active_processes()? == 0).then_some(status))
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self
            .root_status(INFINITE)?
            .ok_or_else(|| io::Error::other("infinite process wait returned nonterminal"))?;
        self.job.wait_empty()?;
        Ok(status)
    }
}

/// Original native process, primary thread, Job and explicit parent stream endpoints.
///
/// A successful wait means the root exited and the entire Job has no live members.
/// Callers must drain piped output themselves or concurrently with waiting.
#[derive(Debug)]
pub struct Child {
    /// Parent's original writer to child standard input, when configured as piped.
    pub stdin: Option<ChildStdin>,
    /// Parent's original reader of child standard output, when configured as piped.
    pub stdout: Option<ChildStdout>,
    /// Parent's original reader of child standard error, when configured as piped.
    pub stderr: Option<ChildStderr>,
    originals: Option<Originals>,
    kill_on_drop: bool,
}

impl Child {
    /// Observe a diagnostic process ID, never an authority for recovering a handle.
    pub fn id(&self) -> u32 {
        self.originals.as_ref().unwrap().id
    }
    /// Observe terminal state only when the root exited and every Job member terminated.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.originals.as_mut().unwrap().try_wait()
    }
    /// Close this owner's stdin and wait for the root and entire original Job to terminate.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        self.originals.as_mut().unwrap().wait()
    }
    /// Request termination through the original unnamed Job, including all its descendants.
    pub fn kill(&mut self) -> io::Result<()> {
        self.originals.as_ref().unwrap().job.terminate(1)
    }
}

impl AsHandle for Child {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.originals.as_ref().unwrap().process.as_handle()
    }
}
impl AsRawHandle for Child {
    fn as_raw_handle(&self) -> RawHandle {
        self.as_handle().as_raw_handle()
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // Close parent endpoints before any independent reaper waits on EOF-driven children.
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        let Some(mut originals) = self.originals.take() else {
            return;
        };
        if matches!(originals.try_wait(), Ok(Some(_))) {
            return;
        }
        if self.kill_on_drop {
            let _ = originals.job.terminate(1);
        }
        // Arc staging retains originals if OS thread creation fails before the closure runs.
        let custody = Arc::new(Mutex::new(Some(originals)));
        let reaper_custody = Arc::clone(&custody);
        let spawned = thread::Builder::new()
            .name("iroha-original-job-reaper".into())
            .spawn(move || {
                let mut slot = reaper_custody
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let Some(mut originals) = slot.take() else {
                    return;
                };
                drop(slot);
                if originals.wait().is_err() {
                    // An observation failure is not proof of exit. Preserve all original
                    // native objects indefinitely instead of releasing unconfirmed custody.
                    std::mem::forget(originals);
                }
            });
        if spawned.is_err() {
            std::mem::forget(custody);
        }
    }
}

#[cfg(test)]
#[path = "process_owner/tests.rs"]
mod tests;
