//! Explicit native process requests and original terminal ownership without release policy.

use std::{
    ffi::OsStr,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus, Output},
    sync::{Arc, Mutex},
    thread,
};

use crate::ProcessStdio;

#[path = "process/wake.rs"]
mod wake;

#[cfg(windows)]
use crate::windows::process_owner::{Child as NativeChild, Command as NativeCommand};
#[cfg(unix)]
use std::process::{Child as NativeChild, Command as NativeCommand};

/// An explicit one-shot native launch request, without release admission or raw-command escape.
///
/// Unix preserves native descriptor and process-group execution. Windows installs its
/// original Job during process creation; no post-spawn assignment fallback exists.
#[derive(Debug)]
pub struct ProcessCommand {
    inner: NativeCommand,
    kill_on_drop: bool,
    current_directory: Option<CurrentDirectory>,
}

#[derive(Debug)]
enum CurrentDirectory {
    Named(PathBuf),
    Retained(Arc<crate::ReaderDirectory>),
}

impl ProcessCommand {
    /// Select one absolute executable spelling, without implicit PATH or shell resolution.
    ///
    /// # Errors
    /// Refuses relative executable names and invalid native path settings.
    pub fn new(program: impl AsRef<Path>) -> io::Result<Self> {
        if !program.as_ref().is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "process executable must be absolute",
            ));
        }
        #[cfg(unix)]
        let inner = NativeCommand::new(program.as_ref());
        #[cfg(windows)]
        let inner = NativeCommand::new(program)?;
        Ok(Self {
            inner,
            kill_on_drop: false,
            current_directory: None,
        })
    }

    #[cfg(unix)]
    pub(crate) fn from_retained_descriptor(inner: NativeCommand) -> Self {
        Self {
            inner,
            kill_on_drop: false,
            current_directory: None,
        }
    }

    /// Append one literal argument.
    pub fn arg(&mut self, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(value);
        self
    }
    /// Append literal arguments without shell interpretation.
    pub fn args<I, S>(&mut self, values: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for value in values {
            self.inner.arg(value);
        }
        self
    }
    /// Observe the selected executable spelling without obtaining an unowned command.
    pub fn get_program(&self) -> &OsStr {
        self.inner.get_program()
    }
    /// Observe configured literal arguments.
    pub fn get_args(&self) -> std::process::CommandArgs<'_> {
        self.inner.get_args()
    }
    /// Bind one explicit nested-tool environment value to a retained native input.
    ///
    /// Linux/Android inherit only a fresh duplicate of this selected original into
    /// this child. The parent duplicate stays CLOEXEC; only the child copy becomes
    /// inheritable, and the value names its own `/proc/self/fd` projection. Descendant
    /// launchers must preserve that descriptor. Their argv0 is the descriptor path,
    /// so multicall tools must be tested in this mode before accepting a build.
    /// Windows uses the original pathname under native sharing exclusion.
    ///
    /// Apply this after environment clearing/defaults and do not replace the binding.
    /// Callers retain the tool and their complete admission owners through terminal
    /// observation. This grants no source, immutable-byte or toolchain qualification.
    ///
    /// # Errors
    /// Refuses invalid environment names, changed input custody, unsupported native
    /// descriptor execution, and descriptor or native command errors.
    pub fn env_retained_build_input(
        &mut self,
        key: impl AsRef<OsStr>,
        input: &Arc<crate::RetainedBuildInput>,
    ) -> io::Result<&mut Self> {
        let key = key.as_ref();
        if key.is_empty()
            || key
                .as_encoded_bytes()
                .iter()
                .any(|byte| matches!(byte, 0 | b'='))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid retained tool environment name",
            ));
        }
        input.revalidate()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        crate::platform::build_input_environment(
            &mut self.inner,
            key,
            input.inner.file(),
            Arc::clone(input),
        )?;
        #[cfg(windows)]
        self.inner.env(key, &input.path);
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "original-descriptor tool environment is not qualified on this Unix host",
            ))
        }
        #[cfg(any(windows, target_os = "linux", target_os = "android"))]
        {
            input.revalidate()?;
            Ok(self)
        }
    }

    /// Bind an anonymous kernel-sealed tool image to one child environment value.
    ///
    /// Linux/Android inherit only a fresh child descriptor, retaining the image and
    /// its source batch. Descendants must preserve that descriptor and value; their
    /// descriptor argv0 does not select a mutable pathname or a multicall name.
    /// The caller retains complete release admission through original terminal exit.
    ///
    /// # Errors
    /// Refuses invalid environment names, changed custody or native descriptor errors.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn env_sealed_tool_image<const N: usize>(
        &mut self,
        key: impl AsRef<OsStr>,
        image: &Arc<crate::SealedToolImage<N>>,
    ) -> io::Result<&mut Self> {
        let key = key.as_ref();
        if key.is_empty()
            || key
                .as_encoded_bytes()
                .iter()
                .any(|byte| matches!(byte, 0 | b'='))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sealed tool environment name",
            ));
        }
        image.bind_environment(&mut self.inner, key)?;
        Ok(self)
    }

    /// Observe explicit environment settings.
    pub fn get_envs(&self) -> std::process::CommandEnvs<'_> {
        self.inner.get_envs()
    }
    /// Observe the configured working directory.
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.current_directory
            .as_ref()
            .map(|directory| match directory {
                CurrentDirectory::Named(path) => path.as_path(),
                CurrentDirectory::Retained(owner) => owner.path(),
            })
    }
    /// Set one child environment entry.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }
    /// Set child environment entries.
    pub fn envs<I, K, V>(&mut self, values: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, value) in values {
            self.inner.env(key, value);
        }
        self
    }
    /// Remove an inherited or explicitly configured environment entry.
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }
    /// Start the child with an empty environment.
    pub fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self
    }
    /// Select the child's working directory without changing the current process directory.
    /// Replaces any earlier named or retained directory setting.
    pub fn current_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.current_directory = Some(CurrentDirectory::Named(path.as_ref().to_owned()));
        self
    }
    /// Select this original directory, retaining its ancestry through child completion.
    ///
    /// Unix changes only the forked child's directory through the retained descriptor.
    /// Windows uses its original name while retained ancestor handles exclude replacement.
    /// Replaces any earlier directory setting. This authenticates directory custody at
    /// configuration and spawn, not its contents or executable; enclosing admission owners
    /// must still revalidate their complete inputs after terminal observation.
    ///
    /// # Errors
    /// Refuses changed directory custody. Native descriptor or launch errors are returned
    /// by spawn; unsupported Unix descriptor-directory operations remain errors.
    pub fn current_dir_retained(
        &mut self,
        directory: &Arc<crate::ReaderDirectory>,
    ) -> io::Result<&mut Self> {
        directory.revalidate()?;
        self.current_directory = Some(CurrentDirectory::Retained(Arc::clone(directory)));
        Ok(self)
    }
    /// Select an explicit original input stream.
    pub fn stdin(&mut self, stream: impl Into<ProcessStdio>) -> &mut Self {
        self.inner.stdin(native_stream(stream.into()));
        self
    }
    /// Select an explicit original output stream.
    pub fn stdout(&mut self, stream: impl Into<ProcessStdio>) -> &mut Self {
        self.inner.stdout(native_stream(stream.into()));
        self
    }
    /// Select an explicit original error stream.
    pub fn stderr(&mut self, stream: impl Into<ProcessStdio>) -> &mut Self {
        self.inner.stderr(native_stream(stream.into()));
        self
    }
    /// Request original-child termination on Drop; Windows terminates the whole original Job.
    pub fn kill_on_drop(&mut self, enabled: bool) -> &mut Self {
        self.kill_on_drop = enabled;
        self
    }

    /// Select the native Unix process group without exposing arbitrary pre-exec callbacks.
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        use std::os::unix::process::CommandExt as _;
        self.inner.process_group(group);
        self
    }
    /// Retain original Unix inputs for absent fixed child descriptor slots.
    ///
    /// # Errors
    /// Refuses invalid maps or failed source duplication; occupied destinations fail at spawn.
    #[cfg(unix)]
    pub fn inherit_descriptors(
        &mut self,
        mappings: &[(std::os::fd::BorrowedFd<'_>, std::os::fd::RawFd)],
    ) -> io::Result<()> {
        crate::inherit_descriptors(&mut self.inner, mappings)
    }

    /// Consume the request and retain the original process until terminal observation.
    ///
    /// # Errors
    /// Returns invalid settings, stream preparation or native creation errors.
    pub fn spawn(self) -> io::Result<ProcessChild> {
        let Self {
            mut inner,
            kill_on_drop,
            current_directory,
        } = self;
        let current_directory = prepare_current_directory(&mut inner, current_directory)?;
        // The common original owner applies the requested drop policy. Its nested
        // Windows owner remains a custody fallback, without a second termination policy.
        #[cfg(windows)]
        inner.kill_on_drop(false);
        let mut child = inner.spawn()?;
        Ok(ProcessChild {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: Some(child),
            kill_on_drop,
            current_directory,
        })
    }
    /// Run with closed stdin and captured output/error streams.
    ///
    /// # Errors
    /// Returns creation, stream or terminal-observation errors.
    pub fn output(mut self) -> io::Result<Output> {
        self.stdin(ProcessStdio::Null)
            .stdout(ProcessStdio::Piped)
            .stderr(ProcessStdio::Piped);
        self.spawn()?.wait_with_output()
    }
    /// Run with configured streams until terminal observation.
    ///
    /// # Errors
    /// Returns native creation or terminal-observation errors.
    pub fn status(self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }
}

fn prepare_current_directory(
    command: &mut NativeCommand,
    directory: Option<CurrentDirectory>,
) -> io::Result<Option<Arc<crate::ReaderDirectory>>> {
    match directory {
        None => Ok(None),
        Some(CurrentDirectory::Named(path)) => {
            command.current_dir(path);
            Ok(None)
        }
        Some(CurrentDirectory::Retained(owner)) => {
            owner.revalidate()?;
            #[cfg(unix)]
            owner.inner.bind_current_directory(command)?;
            #[cfg(windows)]
            command.current_dir(owner.path());
            owner.revalidate()?;
            Ok(Some(owner))
        }
    }
}

#[cfg(unix)]
fn native_stream(stream: ProcessStdio) -> std::process::Stdio {
    stream.into_standard()
}
#[cfg(windows)]
fn native_stream(stream: ProcessStdio) -> crate::windows::process_owner::Stream {
    use crate::windows::process_owner::Stream;
    match stream {
        ProcessStdio::Inherit => Stream::Inherit,
        ProcessStdio::Null => Stream::Null,
        ProcessStdio::Piped => Stream::Piped,
        ProcessStdio::File(file) => Stream::Owned(file.into()),
    }
}

/// Original process and explicit pipes, retained independently of any async runtime.
///
/// Windows completion includes both the original root and every original Job member.
/// This owner grants no executable qualification; stronger callers must retain their
/// admission owner together with this child through every reaper and wait error.
#[derive(Debug)]
pub struct ProcessChild {
    /// Original parent input pipe; take it to keep independent EOF control.
    pub stdin: Option<ChildStdin>,
    /// Original parent output pipe.
    pub stdout: Option<ChildStdout>,
    /// Original parent error pipe.
    pub stderr: Option<ChildStderr>,
    inner: Option<NativeChild>,
    kill_on_drop: bool,
    current_directory: Option<Arc<crate::ReaderDirectory>>,
}

impl ProcessChild {
    /// Diagnostic identifier of this owned original, never a process-recovery capability.
    pub fn id(&self) -> u32 {
        self.inner.as_ref().expect("original lives until Drop").id()
    }
    /// Poll terminal observation, retaining originals on error.
    ///
    /// # Errors
    /// Returns the original native observation error.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.inner
            .as_mut()
            .expect("original lives until Drop")
            .try_wait()
    }
    /// Close still-owned stdin and wait for terminal observation.
    ///
    /// # Errors
    /// Returns the original observation error without releasing its ownership.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        self.inner
            .as_mut()
            .expect("original lives until Drop")
            .wait()
    }

    /// Close still-owned stdin and await terminal observation without an async runtime requirement.
    ///
    /// A polling wake worker holds no process or admission authority. Cancelling this
    /// future stops that worker and leaves this child available for polling or termination.
    /// Windows terminal observation still requires every original Job member to exit.
    ///
    /// # Errors
    /// Returns native observation or wake-thread creation errors without releasing the original.
    pub async fn wait_async(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        if let Some(status) = self.try_wait()? {
            return Ok(status);
        }
        let mut wake = wake::PollWake::new()?;
        loop {
            wake.tick().await;
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
        }
    }
    /// Terminate the original child, or whole original Job on Windows; wait separately.
    ///
    /// # Errors
    /// Returns the native termination error.
    pub fn kill(&mut self) -> io::Result<()> {
        self.inner
            .as_mut()
            .expect("original lives until Drop")
            .kill()
    }
    /// Drain both original output streams and wait, retaining the process through errors.
    ///
    /// # Errors
    /// Returns stream, reader-thread or native observation errors.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        let stdout = match thread::Builder::new()
            .name("iroha-process-stdout".into())
            .spawn(move || read_pipe(stdout))
        {
            Ok(reader) => reader,
            Err(error) => {
                let _ = self.kill();
                return Err(error);
            }
        };
        let stderr = read_pipe(stderr);
        let status = self.wait();
        let stdout = stdout
            .join()
            .map_err(|_| io::Error::other("original stdout reader panicked"))?;
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsHandle for ProcessChild {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        std::os::windows::io::AsHandle::as_handle(
            self.inner.as_ref().expect("original lives until Drop"),
        )
    }
}
#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for ProcessChild {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        std::os::windows::io::AsRawHandle::as_raw_handle(
            self.inner.as_ref().expect("original lives until Drop"),
        )
    }
}

fn read_pipe(pipe: Option<impl Read>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

impl Drop for ProcessChild {
    fn drop(&mut self) {
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        let Some(mut child) = self.inner.take() else {
            return;
        };
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        if self.kill_on_drop {
            let _ = child.kill();
        }
        let staged = Arc::new(Mutex::new(Some((child, self.current_directory.take()))));
        let worker = Arc::clone(&staged);
        let result = thread::Builder::new()
            .name("iroha-process-reaper".into())
            .spawn(move || {
                if let Some((mut child, current_directory)) = worker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                    && child.wait().is_err()
                {
                    std::mem::forget((child, current_directory));
                }
            });
        if result.is_err() {
            std::mem::forget(staged);
        }
    }
}

#[cfg(test)]
#[path = "process/tests.rs"]
mod tests;
