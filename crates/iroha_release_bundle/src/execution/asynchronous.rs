//! Cancellation-safe Tokio supervision with an independent terminal custody reaper.

use std::{
    io,
    process::{ExitStatus, Output},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt as _},
    process::{ChildStderr, ChildStdin, ChildStdout},
};

use super::{Command, retain_until_exit};
use crate::bundle::Program;
use iroha_fs::ProcessChild as NativeChild;

impl Command {
    /// Launch asynchronously while retaining the complete bundle and admission owner.
    ///
    /// Its native child never escapes this owner. Cancelling a wait or dropping the
    /// Tokio runtime transfers terminal observation to an independent native reaper.
    ///
    /// # Errors
    /// Refuses changed custody or selection, missing runtime support and native spawn errors.
    pub fn spawn_async(self) -> io::Result<AsyncChild> {
        self.validate()?;
        tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
        let child = self.inner.spawn()?;
        // Install the complete admission owner before any fallible pipe conversion.
        // Conversion failure or unwind therefore keeps Program + original child
        // together through termination and the independent terminal reaper.
        let mut owned = AsyncChild {
            stdin: None,
            stdout: None,
            stderr: None,
            inner: Some(child),
            program: self.program,
            kill_on_drop: true,
            terminal_observed: false,
        };
        let original = owned
            .inner
            .as_mut()
            .expect("original installed before conversion");
        owned.stdin = original
            .stdin
            .take()
            .map(ChildStdin::from_std)
            .transpose()?;
        owned.stdout = original
            .stdout
            .take()
            .map(ChildStdout::from_std)
            .transpose()?;
        owned.stderr = original
            .stderr
            .take()
            .map(ChildStderr::from_std)
            .transpose()?;
        owned.program.revalidate()?;
        owned.kill_on_drop = self.kill_on_drop;
        Ok(owned)
    }
}

/// Original asynchronous child, pipe ends, bundle and full independent admission custody.
#[derive(Debug)]
pub struct AsyncChild {
    /// Original input pipe. Take it before waiting if its close is an explicit shutdown action.
    pub stdin: Option<ChildStdin>,
    /// Original stdout pipe, when configured.
    pub stdout: Option<ChildStdout>,
    /// Original stderr pipe, when configured.
    pub stderr: Option<ChildStderr>,
    inner: Option<NativeChild>,
    program: Program,
    kill_on_drop: bool,
    terminal_observed: bool,
}

impl AsyncChild {
    /// Observe the original Windows process handle before whole-Job terminal observation.
    ///
    /// The handle is borrowed from this child, never reopened by PID. This returns
    /// `None` after terminal observation even while the original owner is retained.
    #[cfg(windows)]
    pub fn raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        use std::os::windows::io::AsRawHandle as _;
        (!self.terminal_observed).then(|| {
            self.inner
                .as_ref()
                .expect("original lives until Drop")
                .as_raw_handle()
        })
    }

    /// Identifier of the original child, absent after terminal observation.
    pub fn id(&self) -> Option<u32> {
        (!self.terminal_observed).then(|| self.inner.as_ref().expect("child lives until Drop").id())
    }

    /// Poll actual exit and validate the retained bundle before reporting status.
    ///
    /// # Errors
    /// Returns native polling or changed-custody errors.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.observe()?;
        self.program.revalidate()?;
        Ok(status)
    }

    /// Close still-owned stdin, await original exit, then check the complete bundle.
    ///
    /// Cancellation leaves the original child and bundle in this wrapper.
    ///
    /// # Errors
    /// Returns native wait errors or a final custody failure.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        let status = self
            .inner
            .as_mut()
            .expect("child lives until Drop")
            .wait_async()
            .await?;
        self.terminal_observed = true;
        self.program.revalidate()?;
        Ok(status)
    }

    /// Request termination of this wrapper's own child without abandoning its custody.
    ///
    /// # Errors
    /// Returns native termination errors. Await [`Self::wait`] to observe completion.
    pub fn start_kill(&mut self) -> io::Result<()> {
        self.inner.as_mut().expect("child lives until Drop").kill()
    }

    /// Terminate and reap this original child before final bundle validation.
    ///
    /// # Errors
    /// Returns native termination, wait and custody errors.
    pub async fn kill(&mut self) -> io::Result<()> {
        self.start_kill()?;
        self.wait().await.map(|_| ())
    }

    /// Drain both captured streams concurrently with original process supervision.
    ///
    /// Cancellation drops this wrapper into its independent original-custody reaper.
    ///
    /// # Errors
    /// Returns native stream, wait and final custody errors.
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        let (status, stdout, stderr) =
            tokio::join!(self.wait(), read_pipe(stdout), read_pipe(stderr));
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }

    fn observe(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self
            .inner
            .as_mut()
            .expect("child lives until Drop")
            .try_wait()?;
        self.terminal_observed |= status.is_some();
        Ok(status)
    }
}

async fn read_pipe(pipe: Option<impl AsyncRead + Unpin>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes).await?;
    }
    Ok(bytes)
}

impl Drop for AsyncChild {
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
