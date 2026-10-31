//! Retained Windows direct outputs with custody checks around every operation.

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Write},
};

use crate::{OwnerDirectory, checked_name, platform};

/// A mutable Windows regular output retaining its exact file and parent ancestry.
///
/// Existing current-owner files may retain safe public-read permissions. New files
/// are private. Opening intentionally truncates the admitted original; later I/O
/// failures may leave partial output. This is not atomic publication or recovery.
/// No raw file or publication authority is exposed by this capability.
#[derive(Debug)]
pub struct RetainedOutputFile {
    inner: platform::RetainedFile,
}

impl OwnerDirectory {
    /// Open one native regular output and truncate its admitted original.
    ///
    /// A single create-new attempt creates private files. Only an actual existing
    /// name is opened without truncation, checked for current ownership, safe ACLs,
    /// a single link, type, identity and retained ancestry, and then truncated.
    /// Existing permissions are never repaired. The returned owner excludes other
    /// writers and deletion while it remains live, including after this directory drops.
    ///
    /// # Errors
    /// Refuses invalid names, unsafe originals or ancestors, live writers and native I/O.
    /// An error after truncation may leave an empty or partial original output.
    pub fn open_truncating_output(
        &self,
        name: impl AsRef<OsStr>,
    ) -> io::Result<RetainedOutputFile> {
        Ok(RetainedOutputFile {
            inner: self.inner.open_output(checked_name(name.as_ref())?)?,
        })
    }
}

impl RetainedOutputFile {
    /// Recheck current ownership, native file custody, selected name and all ancestors.
    /// # Errors
    /// Refuses changed custody or a native observation error.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.require_current_owner()
    }

    /// Flush the completed original file's data and metadata to native storage.
    /// # Errors
    /// Refuses changed custody or native synchronization errors. No rollback is implied.
    pub fn sync_all(&self) -> io::Result<()> {
        self.revalidate()?;
        let result = self.inner.file().sync_all();
        let custody = self.revalidate();
        result.and(custody)
    }

    fn observed<T>(&mut self, operation: impl FnOnce(&mut File) -> io::Result<T>) -> io::Result<T> {
        self.revalidate()?;
        let result = operation(self.inner.file_mut());
        // Always close ordinary I/O through custody. Preserve the original I/O
        // error if both fail, but do not return successful bytes on custody loss.
        let custody = self.revalidate();
        result.and_then(|value| custody.map(|()| value))
    }
}

impl Write for RetainedOutputFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.observed(|file| file.write(bytes))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.observed(Write::flush)
    }
}

#[cfg(test)]
#[path = "output_file/tests.rs"]
mod tests;
