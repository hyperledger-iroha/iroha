//! Native execution selected by an already retained build-input original.

use std::{io, sync::Arc};

use crate::{ProcessCommand, RetainedBuildInput};

impl RetainedBuildInput {
    /// Construct a command selecting this original build input.
    ///
    /// Linux/Android use the held descriptor and retain this owner in the command
    /// through spawn. The original spelling supplies argv0 only, preserving direct
    /// multicall-tool behavior without using that spelling for executable lookup.
    /// Windows uses its original pathname under the existing native
    /// write/delete-sharing exclusion; the caller must retain this owner through
    /// spawn. On every host the caller must retain its complete tool, configuration,
    /// source and lock owners until the child is terminal, and revalidate them before
    /// launch and after exit. The command alone does not enforce that longer lifetime.
    ///
    /// This accepts the existing initial build-input hardlink policy. It does not
    /// seal mutable tool bytes or authenticate their contents or dynamic dependencies.
    ///
    /// # Errors
    /// Refuses changed custody, a non-native Linux image, unavailable original
    /// descriptor execution, and native command errors. Other Unix hosts remain
    /// unsupported until their original-descriptor launch has been qualified.
    pub fn command(self: &Arc<Self>) -> io::Result<ProcessCommand> {
        self.revalidate()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let command = {
            use std::os::unix::process::CommandExt as _;
            let mut selected =
                crate::platform::flat_batch_command(self.inner.file(), Arc::clone(self))?;
            // Preserve multicall behavior without using this spelling for lookup.
            selected.arg0(&self.path);
            ProcessCommand::from_retained_descriptor(selected)
        };
        #[cfg(windows)]
        let command = ProcessCommand::new(&self.path)?;
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
        {
            // TODO: qualify native descriptor execution on other Unix hosts. A
            // pathname fallback would again let a mutable ancestor select the tool.
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "original-descriptor execution is not qualified on this Unix host",
            ))
        }
        #[cfg(any(windows, target_os = "linux", target_os = "android"))]
        {
            self.revalidate()?;
            Ok(command)
        }
    }
}

#[cfg(all(test, any(windows, target_os = "linux", target_os = "android")))]
#[path = "build_input_command_tests.rs"]
mod tests;
