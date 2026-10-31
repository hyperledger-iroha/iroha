//! Process selection which retains authenticated release ownership through child exit.

use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::{self, ExitStatus, Output},
};

use iroha_fs::{ProcessChild, ProcessCommand, ProcessStdio as Stdio};
use iroha_release_bundle::{bundle::Program as ReleaseProgram, execution};

/// A development selection or an authenticated retained release executable.
///
/// A diagnostic path cannot substitute for this owner's command construction.
///
/// ```compile_fail
/// use iroha_test_network::ResolvedProgram;
/// fn launch(program: ResolvedProgram) { std::process::Command::new(program); }
/// ```
#[derive(Clone, Debug)]
pub struct ResolvedProgram {
    inner: ProgramKind,
}

#[derive(Clone, Debug)]
enum ProgramKind {
    Development(PathBuf),
    Release(ReleaseProgram),
}

impl ResolvedProgram {
    // Only the resolver may choose a development path after excluding a release contract.
    pub(crate) fn development(path: PathBuf) -> Self {
        Self {
            inner: ProgramKind::Development(path),
        }
    }

    pub(crate) fn retained(program: ReleaseProgram) -> Self {
        Self {
            inner: ProgramKind::Release(program),
        }
    }

    /// Whether this selection retains an authenticated release program and its custody.
    pub const fn is_release(&self) -> bool {
        matches!(&self.inner, ProgramKind::Release(_))
    }

    /// Revalidate all retained release originals; development selections make no such claim.
    ///
    /// # Errors
    /// Returns any bundle or independently attached admission-owner custody failure.
    pub fn revalidate(&self) -> io::Result<()> {
        match &self.inner {
            ProgramKind::Development(_) => Ok(()),
            ProgramKind::Release(program) => program.revalidate(),
        }
    }

    /// Observe the selected name for diagnostics, validating qualified custody first.
    ///
    /// # Errors
    /// Returns any release-bundle custody failure.
    pub fn diagnostic_path(&self) -> io::Result<PathBuf> {
        match &self.inner {
            ProgramKind::Development(path) => Ok(path.clone()),
            ProgramKind::Release(program) => program.diagnostic_path(),
        }
    }

    /// Construct one command which retains the complete qualified bundle when selected.
    ///
    /// # Errors
    /// Returns changed custody or unsupported qualified execution errors.
    pub fn command(&self) -> io::Result<ResolvedCommand> {
        let inner = match &self.inner {
            ProgramKind::Development(path) => CommandKind::Development(ProcessCommand::new(path)?),
            ProgramKind::Release(program) => CommandKind::Release(program.command()?),
        };
        Ok(ResolvedCommand {
            inner,
            kill_on_drop: false,
        })
    }
}

/// One launch request with configuration-only access to either command implementation.
#[derive(Debug)]
pub struct ResolvedCommand {
    inner: CommandKind,
    kill_on_drop: bool,
}

#[derive(Debug)]
enum CommandKind {
    Development(ProcessCommand),
    Release(execution::Command),
}

impl ResolvedCommand {
    /// Adopt an explicit general process request, making no release qualification claim.
    #[doc(hidden)]
    pub fn from_development_command(command: ProcessCommand) -> Self {
        Self {
            inner: CommandKind::Development(command),
            kill_on_drop: false,
        }
    }

    /// Configure arguments, environment, streams and working directory without launch escape.
    pub fn configure(
        &mut self,
        configure: impl FnOnce(&mut execution::CommandOptions<'_>),
    ) -> &mut Self {
        match &mut self.inner {
            CommandKind::Development(command) => {
                configure(&mut execution::CommandOptions::new(command))
            }
            CommandKind::Release(command) => {
                command.configure(configure);
            }
        }
        self
    }

    /// Append one literal argument.
    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.configure(|options| {
            options.arg(argument);
        })
    }

    /// Append literal arguments without shell interpretation.
    pub fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.configure(|options| {
            options.args(arguments);
        })
    }

    /// Observe the literal configured arguments without exposing launch authority.
    pub fn get_args(&self) -> process::CommandArgs<'_> {
        match &self.inner {
            CommandKind::Development(command) => command.get_args(),
            CommandKind::Release(command) => command.get_args(),
        }
    }

    /// Select the native child's Unix process group through restricted settings.
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        self.configure(|options| {
            options.process_group(group);
        })
    }

    /// Keep original inputs alive for fixed Unix child descriptors without command escape.
    ///
    /// # Errors
    /// Returns invalid descriptor-map or source duplication failures. Occupied child
    /// destinations are refused by the native launch before any slot is installed.
    #[cfg(unix)]
    pub fn inherit_descriptors(
        &mut self,
        mappings: &[(std::os::fd::BorrowedFd<'_>, std::os::fd::RawFd)],
    ) -> io::Result<&mut Self> {
        let mut result = Ok(());
        self.configure(|options| result = options.inherit_descriptors(mappings));
        result?;
        Ok(self)
    }

    /// Set one child environment entry.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.configure(|options| {
            options.env(key, value);
        })
    }

    /// Set child environment entries.
    pub fn envs<I, K, V>(&mut self, entries: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.configure(|options| {
            options.envs(entries);
        })
    }

    /// Remove an inherited or explicitly configured environment entry.
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.configure(|options| {
            options.env_remove(key);
        })
    }

    /// Clear the inherited child environment.
    pub fn env_clear(&mut self) -> &mut Self {
        self.configure(|options| {
            options.env_clear();
        })
    }

    /// Select the child's working directory without changing its executable.
    pub fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.configure(|options| {
            options.current_dir(directory);
        })
    }

    /// Set the child's input stream.
    pub fn stdin(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.configure(|options| {
            options.stdin(stream);
        })
    }

    /// Set the child's output stream.
    pub fn stdout(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.configure(|options| {
            options.stdout(stream);
        })
    }

    /// Set the child's error stream.
    pub fn stderr(&mut self, stream: impl Into<Stdio>) -> &mut Self {
        self.configure(|options| {
            options.stderr(stream);
        })
    }

    /// Request termination of this command's original child when its supervisor drops.
    pub fn kill_on_drop(&mut self, enabled: bool) -> &mut Self {
        self.kill_on_drop = enabled;
        match &mut self.inner {
            CommandKind::Development(command) => {
                command.kill_on_drop(enabled);
            }
            CommandKind::Release(command) => {
                command.kill_on_drop(enabled);
            }
        }
        self
    }

    /// Launch one synchronous child, preserving qualified ownership through its supervision.
    ///
    /// # Errors
    /// Returns native launch errors or qualified custody failures.
    pub fn spawn(self) -> io::Result<ResolvedChild> {
        match self.inner {
            CommandKind::Development(command) => {
                let mut child = ResolvedChild::from_development_child(command.spawn()?);
                child.kill_on_drop = self.kill_on_drop;
                Ok(child)
            }
            CommandKind::Release(command) => {
                let mut child = command.spawn()?;
                Ok(ResolvedChild {
                    stdin: child.stdin.take(),
                    stdout: child.stdout.take(),
                    stderr: child.stderr.take(),
                    inner: Some(ChildKind::Release(child)),
                    kill_on_drop: self.kill_on_drop,
                })
            }
        }
    }

    /// Run synchronously with closed stdin and captured stdout/stderr.
    ///
    /// # Errors
    /// Returns launch, stream, wait and qualified custody failures.
    pub fn output(mut self) -> io::Result<Output> {
        self.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.spawn()?.wait_with_output()
    }

    /// Run synchronously with the configured streams until terminal observation.
    ///
    /// # Errors
    /// Returns launch, wait and qualified custody failures.
    pub fn status(self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }

    /// Launch an asynchronous child without exposing the underlying native command.
    ///
    /// # Errors
    /// Returns launch or qualified custody failures; requires an active Tokio runtime.
    pub fn spawn_async(self) -> io::Result<ResolvedAsyncChild> {
        match self.inner {
            CommandKind::Development(command) => {
                tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
                ResolvedAsyncChild::from_development_child(command.spawn()?)
            }
            CommandKind::Release(command) => {
                let mut child = command.spawn_async()?;
                Ok(ResolvedAsyncChild {
                    stdin: child.stdin.take(),
                    stdout: child.stdout.take(),
                    stderr: child.stderr.take(),
                    inner: AsyncChildKind::Release(Box::new(child)),
                    conversion_incomplete: false,
                })
            }
        }
    }

    /// Run asynchronously with closed stdin and captured streams.
    ///
    /// # Errors
    /// Returns launch, stream, wait and qualified custody failures.
    pub async fn output_async(mut self) -> io::Result<Output> {
        self.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.spawn_async()?.wait_with_output().await
    }

    /// Run asynchronously with the configured streams until actual exit.
    ///
    /// # Errors
    /// Returns launch, wait and qualified custody failures.
    pub async fn status_async(self) -> io::Result<ExitStatus> {
        self.spawn_async()?.wait().await
    }
}

/// Synchronous child with explicit pipe ownership and private process supervision.
#[derive(Debug)]
pub struct ResolvedChild {
    /// Original stdin pipe; take it to control an independent EOF shutdown boundary.
    pub stdin: Option<process::ChildStdin>,
    /// Original stdout pipe, when configured.
    pub stdout: Option<process::ChildStdout>,
    /// Original stderr pipe, when configured.
    pub stderr: Option<process::ChildStderr>,
    inner: Option<ChildKind>,
    kill_on_drop: bool,
}

#[derive(Debug)]
enum ChildKind {
    Development(ProcessChild),
    Release(execution::Child),
}

impl ResolvedChild {
    /// Adopt an already-owned development child without making any release qualification.
    ///
    /// This consumes the original child and pipes; it never reopens a process by PID.
    #[doc(hidden)]
    pub fn from_development_child(mut child: ProcessChild) -> Self {
        Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: Some(ChildKind::Development(child)),
            kill_on_drop: false,
        }
    }

    /// Identifier of this original child.
    pub fn id(&self) -> u32 {
        match self
            .inner
            .as_ref()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => child.id(),
            ChildKind::Release(child) => child.id(),
        }
    }

    /// Observe exit without blocking, retaining ownership on every error.
    ///
    /// # Errors
    /// Returns native wait or qualified custody failures.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self
            .inner
            .as_mut()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => child.try_wait(),
            ChildKind::Release(child) => child.try_wait(),
        }
    }

    /// Close still-owned stdin and wait for actual exit.
    ///
    /// # Errors
    /// Returns native wait or qualified custody failures.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        match self
            .inner
            .as_mut()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => child.wait(),
            ChildKind::Release(child) => child.wait(),
        }
    }

    /// Request termination of this original child without releasing qualified custody.
    ///
    /// # Errors
    /// Returns the native termination error; call `wait` to observe completion.
    pub fn kill(&mut self) -> io::Result<()> {
        match self
            .inner
            .as_mut()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => child.kill(),
            ChildKind::Release(child) => child.kill(),
        }
    }

    /// Drain captured streams and wait while retaining the original qualified process owner.
    ///
    /// # Errors
    /// Returns stream, wait and qualified custody failures.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        match self.inner.take().expect("original child lives until Drop") {
            ChildKind::Development(mut child) => {
                child.stdin = self.stdin.take();
                child.stdout = self.stdout.take();
                child.stderr = self.stderr.take();
                child.wait_with_output()
            }
            ChildKind::Release(mut child) => {
                child.stdin = self.stdin.take();
                child.stdout = self.stdout.take();
                child.stderr = self.stderr.take();
                child.wait_with_output()
            }
        }
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for ResolvedChild {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        match self
            .inner
            .as_ref()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => {
                std::os::windows::io::AsRawHandle::as_raw_handle(child)
            }
            ChildKind::Release(child) => std::os::windows::io::AsRawHandle::as_raw_handle(child),
        }
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsHandle for ResolvedChild {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        match self
            .inner
            .as_ref()
            .expect("original child lives until Drop")
        {
            ChildKind::Development(child) => std::os::windows::io::AsHandle::as_handle(child),
            ChildKind::Release(child) => std::os::windows::io::AsHandle::as_handle(child),
        }
    }
}

impl Drop for ResolvedChild {
    fn drop(&mut self) {
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        if let Some(ChildKind::Development(child)) = self.inner.as_mut()
            && self.kill_on_drop
        {
            let _ = child.kill();
        }
        // Both branches retain their original process until terminal observation.
        // Only the release branch carries the independent qualified Program owner.
        drop(self.inner.take());
    }
}

/// Asynchronous child with original streams and private qualified process ownership.
#[derive(Debug)]
pub struct ResolvedAsyncChild {
    /// Original stdin pipe; take it before waiting to retain explicit EOF control.
    pub stdin: Option<tokio::process::ChildStdin>,
    /// Original stdout pipe, when configured.
    pub stdout: Option<tokio::process::ChildStdout>,
    /// Original stderr pipe, when configured.
    pub stderr: Option<tokio::process::ChildStderr>,
    // Rust drops the preceding still-owned pipes before dropping this process owner.
    inner: AsyncChildKind,
    conversion_incomplete: bool,
}

#[derive(Debug)]
enum AsyncChildKind {
    Development {
        child: ProcessChild,
        terminal_observed: bool,
    },
    Release(Box<execution::AsyncChild>),
}

impl ResolvedAsyncChild {
    /// Adopt an explicit original process and convert only its original parent pipes.
    ///
    /// # Errors
    /// Returns actual Tokio pipe registration errors, retaining and reaping the original.
    #[doc(hidden)]
    pub fn from_development_child(child: ProcessChild) -> io::Result<Self> {
        let mut owned = Self {
            stdin: None,
            stdout: None,
            stderr: None,
            inner: AsyncChildKind::Development {
                child,
                terminal_observed: false,
            },
            conversion_incomplete: true,
        };
        let AsyncChildKind::Development { child, .. } = &mut owned.inner else {
            unreachable!()
        };
        owned.stdin = child
            .stdin
            .take()
            .map(tokio::process::ChildStdin::from_std)
            .transpose()?;
        owned.stdout = child
            .stdout
            .take()
            .map(tokio::process::ChildStdout::from_std)
            .transpose()?;
        owned.stderr = child
            .stderr
            .take()
            .map(tokio::process::ChildStderr::from_std)
            .transpose()?;
        owned.conversion_incomplete = false;
        Ok(owned)
    }

    /// Observe the original Windows process handle until whole-process terminal observation.
    #[cfg(windows)]
    pub fn raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        match &self.inner {
            AsyncChildKind::Development {
                child,
                terminal_observed,
            } => {
                use std::os::windows::io::AsRawHandle as _;
                (!terminal_observed).then(|| child.as_raw_handle())
            }
            AsyncChildKind::Release(child) => child.raw_handle(),
        }
    }

    /// Identifier of this original child, absent after terminal observation.
    pub fn id(&self) -> Option<u32> {
        match &self.inner {
            AsyncChildKind::Development {
                child,
                terminal_observed,
            } => (!terminal_observed).then(|| child.id()),
            AsyncChildKind::Release(child) => child.id(),
        }
    }

    /// Poll actual exit without releasing original ownership on errors.
    ///
    /// # Errors
    /// Returns native polling or qualified custody failures.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match &mut self.inner {
            AsyncChildKind::Development {
                child,
                terminal_observed,
            } => {
                let status = child.try_wait()?;
                *terminal_observed |= status.is_some();
                Ok(status)
            }
            AsyncChildKind::Release(child) => child.try_wait(),
        }
    }

    /// Close still-owned stdin and wait; cancellation leaves the child and owner here.
    ///
    /// # Errors
    /// Returns native wait or qualified custody failures.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        match &mut self.inner {
            AsyncChildKind::Development {
                child,
                terminal_observed,
            } => {
                let status = child.wait_async().await?;
                *terminal_observed = true;
                Ok(status)
            }
            AsyncChildKind::Release(child) => child.wait().await,
        }
    }

    /// Request termination of this original child while retaining custody until exit.
    ///
    /// # Errors
    /// Returns native termination errors.
    pub fn start_kill(&mut self) -> io::Result<()> {
        match &mut self.inner {
            AsyncChildKind::Development { child, .. } => child.kill(),
            AsyncChildKind::Release(child) => child.start_kill(),
        }
    }

    /// Request termination and observe completion before releasing qualified ownership.
    ///
    /// # Errors
    /// Returns termination, wait or qualified custody failures.
    pub async fn kill(&mut self) -> io::Result<()> {
        self.start_kill()?;
        self.wait().await.map(|_| ())
    }

    /// Drain streams and wait; cancellation transfers retained execution to its native reaper.
    ///
    /// # Errors
    /// Returns stream, wait or qualified custody failures.
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        use tokio::io::AsyncReadExt as _;
        drop(self.stdin.take());
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        let read_stdout = async move {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = stdout {
                pipe.read_to_end(&mut bytes).await?;
            }
            io::Result::Ok(bytes)
        };
        let read_stderr = async move {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = stderr {
                pipe.read_to_end(&mut bytes).await?;
            }
            io::Result::Ok(bytes)
        };
        let (status, stdout, stderr) = tokio::join!(self.wait(), read_stdout, read_stderr);
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }
}

impl Drop for ResolvedAsyncChild {
    fn drop(&mut self) {
        if self.conversion_incomplete
            && let AsyncChildKind::Development { child, .. } = &mut self.inner
        {
            let _ = child.kill();
        }
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        // Field destruction transfers the original native child into its independent
        // reaper. Qualified children retain their complete Program in that same pair.
    }
}

#[cfg(test)]
#[path = "resolved_program_tests.rs"]
pub(crate) mod tests;
