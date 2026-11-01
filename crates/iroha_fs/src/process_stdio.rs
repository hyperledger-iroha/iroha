//! Explicit native process streams with original ownership and no opaque command conversion.

use std::{fs::File, process::Stdio};

/// One explicitly chosen process stream; arbitrary opaque `Stdio` cannot be imported.
#[derive(Debug, Default)]
pub enum ProcessStdio {
    /// Use the corresponding standard stream of the parent process.
    #[default]
    Inherit,
    /// Open the native null device for this stream's direction.
    Null,
    /// Create a parent/child pipe under the selected native process owner.
    Piped,
    /// Consume the selected original stream file, preserving its native ownership.
    File(File),
}

impl ProcessStdio {
    /// Select the corresponding inherited standard stream.
    pub const fn inherit() -> Self {
        Self::Inherit
    }
    /// Select the native null device.
    pub const fn null() -> Self {
        Self::Null
    }
    /// Request an original parent/child pipe pair.
    pub const fn piped() -> Self {
        Self::Piped
    }

    /// Convert explicit streams for a Unix command or a separately owned ordinary command.
    ///
    /// This conversion conveys stream ownership only. There is intentionally no inverse
    /// conversion from opaque standard-library `Stdio` into the explicit owner.
    pub fn into_standard(self) -> Stdio {
        match self {
            Self::Inherit => Stdio::inherit(),
            Self::Null => Stdio::null(),
            Self::Piped => Stdio::piped(),
            Self::File(file) => Stdio::from(file),
        }
    }
}

impl From<File> for ProcessStdio {
    fn from(file: File) -> Self {
        Self::File(file)
    }
}

#[cfg(unix)]
impl From<std::os::fd::OwnedFd> for ProcessStdio {
    fn from(file: std::os::fd::OwnedFd) -> Self {
        Self::File(file.into())
    }
}

#[cfg(windows)]
impl From<std::os::windows::io::OwnedHandle> for ProcessStdio {
    fn from(file: std::os::windows::io::OwnedHandle) -> Self {
        Self::File(file.into())
    }
}

// Native Windows conversion must admit GetFileType (disk/pipe/character only)
// before the selected handle becomes an inheritable stream duplicate. A generic
// OwnedHandle's type alone cannot establish that it denotes a stream object.
