//! Commands and supervised children retaining bundle and admission custody until process exit.

use std::{
    ffi::{OsStr, OsString},
    io::{self, Read},
    path::Path,
    process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus, Output},
    sync::{Arc, Mutex},
    thread,
};

use crate::bundle::{Program, invalid};
use iroha_fs::{
    ProcessChild as NativeChild, ProcessCommand as NativeCommand, ProcessStdio as Stdio,
};

#[cfg(feature = "tokio")]
mod asynchronous;
#[cfg(feature = "tokio")]
pub use asynchronous::AsyncChild;

/// Configurable native command retaining its authenticated executable and complete bundle.
#[derive(Debug)]
pub struct Command {
    program: Program,
    inner: NativeCommand,
    expected_program: OsString,
    kill_on_drop: bool,
}

/// Borrowed process settings with no spawning, native-command or program-selection access.
///
/// ```compile_fail
/// use iroha_release_bundle::execution::CommandOptions;
/// fn escape(options: &mut CommandOptions<'_>) { let child = options.spawn(); }
/// ```
///
/// ```compile_fail
/// use iroha_release_bundle::execution::CommandOptions;
/// fn replace(options: &mut CommandOptions<'_>) {
///     *options = std::process::Command::new("another-program");
/// }
/// ```
#[derive(Debug)]
pub struct CommandOptions<'a> {
    inner: &'a mut NativeCommand,
}

impl<'a> CommandOptions<'a> {
    /// Borrow configuration of a command whose process lifetime its caller already owns.
    ///
    /// This facade grants no program-selection or spawning access and authenticates no
    /// release artifact. It lets callers configure development and retained commands
    /// through the same restricted interface without extracting a retained command.
    pub fn new(command: &'a mut NativeCommand) -> Self {
        Self { inner: command }
    }

    /// Append one literal argument.
    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(argument);
        self
    }

    /// Append literal arguments without shell interpretation.
    pub fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(arguments);
        self
    }

    /// Observe literal configured arguments without granting program or launch access.
    pub fn get_args(&self) -> std::process::CommandArgs<'_> {
        self.inner.get_args()
    }

    /// Select the child's Unix process group without granting raw command access.
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        self.inner.process_group(group);
        self
    }

    /// Retain original Unix inputs and install only absent fixed child descriptor slots.
    ///
    /// # Errors
    /// Refuses invalid maps or source duplication failure; an occupied destination
    /// is reported as a native launch failure before any input slot is installed.
    #[cfg(unix)]
    pub fn inherit_descriptors(
        &mut self,
        mappings: &[(std::os::fd::BorrowedFd<'_>, std::os::fd::RawFd)],
    ) -> io::Result<()> {
        self.inner.inherit_descriptors(mappings)
    }

    /// Set one child environment entry.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    /// Set child environment entries.
    pub fn envs<I, K, V>(&mut self, entries: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(entries);
        self
    }

    /// Remove one inherited or explicitly configured child environment entry.
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    /// Clear the inherited child environment.
    pub fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self
    }

    /// Observe explicitly configured environment entries without exposing launch authority.
    pub fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.inner.get_envs()
    }

    /// Select the child's working directory without changing its original executable.
    pub fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.inner.current_dir(directory);
        self
    }

    /// Select the original child input stream.
    pub fn stdin(&mut self, input: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(input);
        self
    }

    /// Select the original child output stream.
    pub fn stdout(&mut self, output: impl Into<Stdio>) -> &mut Self {
        self.inner.stdout(output);
        self
    }

    /// Select the original child error stream.
    pub fn stderr(&mut self, output: impl Into<Stdio>) -> &mut Self {
        self.inner.stderr(output);
        self
    }
}

impl Program {
    /// Prepare a command selecting this exact retained executable on the native platform.
    ///
    /// Linux uses the original descriptor's executable projection. Windows keeps the
    /// immutable parent and all five readers under its native sharing exclusion.
    ///
    /// # Errors
    /// Refuses changed custody, a nonexecutable image or an unsupported native launch path.
    pub fn command(&self) -> io::Result<Command> {
        self.revalidate()?;
        let command = self.bundle.batch.command(self.kind.index())?;
        let expected_program = command.get_program().to_owned();
        Ok(Command {
            program: self.clone(),
            inner: command,
            expected_program,
            kill_on_drop: false,
        })
    }
}

impl Command {
    /// Observe configured arguments without exposing the native command or executable owner.
    pub fn get_args(&self) -> std::process::CommandArgs<'_> {
        self.inner.get_args()
    }

    /// Configure arguments, environment, working directory and streams.
    ///
    /// The settings facade cannot select a different program, spawn a bare child or
    /// transfer the native command away from its retained owner.
    pub fn configure(&mut self, configure: impl FnOnce(&mut CommandOptions<'_>)) -> &mut Self {
        configure(&mut CommandOptions::new(&mut self.inner));
        self
    }

    /// Request termination of this command's own child when its supervisor is dropped.
    ///
    /// Reaping and original custody continue after the drop, including on cancellation.
    pub fn kill_on_drop(&mut self, enabled: bool) -> &mut Self {
        self.kill_on_drop = enabled;
        self
    }

    fn validate(&self) -> io::Result<()> {
        if self.inner.get_program() != self.expected_program {
            return Err(invalid(
                "release command no longer selects its retained original",
            ));
        }
        self.program.revalidate()
    }

    /// Launch with the complete bundle and admission owner retained until this child exits.
    ///
    /// # Errors
    /// Refuses changed selection or custody and native process-creation failures.
    pub fn spawn(self) -> io::Result<Child> {
        self.validate()?;
        let mut child = self.inner.spawn()?;
        let mut owned = Child {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            inner: Some(child),
            program: self.program,
            kill_on_drop: self.kill_on_drop,
        };
        if let Err(error) = owned.program.revalidate() {
            // The process already exists. Keep it in the owning wrapper and terminate
            // this failed launch through its reaper rather than dropping a bare child.
            owned.kill_on_drop = true;
            return Err(error);
        }
        Ok(owned)
    }

    /// Run with closed stdin and captured stdout/stderr, retaining custody through exit.
    ///
    /// # Errors
    /// Returns launch, stream, wait and final custody errors.
    pub fn output(mut self) -> io::Result<Output> {
        self.inner
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.spawn()?.wait_with_output()
    }

    /// Run to completion with the configured streams and recheck the original bundle.
    ///
    /// # Errors
    /// Returns launch, wait and final custody errors.
    pub fn status(self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }
}

/// One native child and the complete bundle and admission owner authorizing its launch.
///
/// Dropping an unexited child transfers the child and complete original program to
/// a reaper. A completed wait checks custody before exposing the exit status.
#[derive(Debug)]
pub struct Child {
    /// Original input pipe, when configured. Take it before waiting for independently
    /// controlled stdin-EOF shutdown; otherwise `wait` closes it.
    pub stdin: Option<ChildStdin>,
    /// Original stdout pipe, when configured.
    pub stdout: Option<ChildStdout>,
    /// Original stderr pipe, when configured.
    pub stderr: Option<ChildStderr>,
    inner: Option<NativeChild>,
    program: Program,
    kill_on_drop: bool,
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for Child {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        std::os::windows::io::AsRawHandle::as_raw_handle(
            self.inner.as_ref().expect("child lives until Drop"),
        )
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsHandle for Child {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        std::os::windows::io::AsHandle::as_handle(
            self.inner.as_ref().expect("child lives until Drop"),
        )
    }
}

impl Child {
    /// Native process identifier of this original child.
    pub fn id(&self) -> u32 {
        self.inner.as_ref().expect("child lives until Drop").id()
    }

    /// Poll the original child and validate its retained bundle before returning status.
    ///
    /// # Errors
    /// Returns native wait errors or any changed original custody.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self
            .inner
            .as_mut()
            .expect("child lives until Drop")
            .try_wait()?;
        self.program.revalidate()?;
        Ok(status)
    }

    /// Close any still-owned input pipe and reap this child before final custody validation.
    ///
    /// # Errors
    /// Returns wait errors or changed original custody, keeping the child owned on error.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        let status = self
            .inner
            .as_mut()
            .expect("child lives until Drop")
            .wait()?;
        self.program.revalidate()?;
        Ok(status)
    }

    /// Terminate this wrapper's own original child; custody remains until it is reaped.
    ///
    /// # Errors
    /// Returns native termination errors. Call [`Self::wait`] to observe completion.
    pub fn kill(&mut self) -> io::Result<()> {
        self.inner.as_mut().expect("child lives until Drop").kill()
    }

    /// Drain both output pipes concurrently and retain the bundle until child exit.
    ///
    /// # Errors
    /// Returns stream, reaper, native wait and final custody errors.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        let stdout = match thread::Builder::new()
            .name("iroha-release-stdout".into())
            .spawn(move || read_pipe(stdout))
        {
            Ok(reader) => reader,
            Err(error) => {
                // This child belongs exclusively to this command. A resource failure
                // must not abandon a live producer blocked on an undrained pipe.
                let _ = self.kill();
                return Err(error);
            }
        };
        let stderr = read_pipe(stderr);
        let status = self.wait();
        let stdout = stdout
            .join()
            .map_err(|_| io::Error::other("release stdout reader panicked"))?;
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }
}

fn read_pipe(pipe: Option<impl Read>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

trait Reap {
    fn wait_terminal(&mut self) -> io::Result<()>;
}

impl Reap for NativeChild {
    fn wait_terminal(&mut self) -> io::Result<()> {
        self.wait().map(|_| ())
    }
}

fn reap(mut child: impl Reap, program: Program) {
    match child.wait_terminal() {
        Ok(()) => {
            // An abandoned supervisor cannot receive an error, but the retained owner
            // remains live through the terminal observation and final custody check.
            let _ = program.revalidate();
        }
        Err(_) => {
            // Without a terminal observation releasing these originals would abandon
            // live execution custody. Preserve the child, bundle AND admission owner
            // together for the process lifetime instead.
            std::mem::forget((child, program));
        }
    }
}

fn retain_until_exit(child: impl Reap + Send + 'static, program: Program) {
    let job = Arc::new(Mutex::new(Some((child, program))));
    let worker = Arc::clone(&job);
    let spawned = thread::Builder::new()
        .name("iroha-release-reaper".into())
        .spawn(move || {
            if let Some((child, program)) = worker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                reap(child, program);
            }
        });
    if spawned.is_err() {
        // Thread exhaustion cannot discard a process owner. A synchronous wait could
        // deadlock on an externally owned input pipe which closes only after Drop returns.
        // Preserve the child and complete Program (including admission custody) for
        // this process lifetime on this exceptional path.
        if let Some((child, program)) = job
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            std::mem::forget((child, program));
        }
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        let Some(mut child) = self.inner.take() else {
            return;
        };
        if matches!(child.try_wait(), Ok(Some(_))) {
            let _ = self.program.revalidate();
            return;
        }
        if self.kill_on_drop {
            let _ = child.kill();
        }
        retain_until_exit(child, self.program.clone());
    }
}
