//! Existing private Windows originals retained through readback and durability barriers.

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

use crate::{FileSnapshot, OwnerDirectory, checked_name, platform};

/// An existing private original with native flush access and no mutation API.
///
/// The original excludes other writers and deletion, retaining its selected name and
/// ancestors until this owner drops. Reads, seeks and flushes recheck the exact captured
/// snapshot. No creation, truncation, publication or raw descriptor is exposed.
#[derive(Debug)]
pub struct PrivateDurabilityFile {
    inner: platform::RetainedFile,
}

impl OwnerDirectory {
    /// Retain an existing private file for bounded readback and native synchronization.
    ///
    /// Write access is required by Windows flushing; this capability exposes no writes.
    /// Missing files remain absent and existing bytes and permissions are never repaired.
    ///
    /// # Errors
    /// Refuses unsafe private custody, indirect or shared files, other writers and native errors.
    pub fn open_private_durability(
        &self,
        name: impl AsRef<OsStr>,
    ) -> io::Result<PrivateDurabilityFile> {
        Ok(PrivateDurabilityFile {
            inner: self
                .inner
                .open_private_durability(checked_name(name.as_ref())?)?,
        })
    }
}

impl PrivateDurabilityFile {
    /// Recheck the original snapshot, private custody, selected name and all ancestors.
    /// # Errors
    /// Refuses any changed original, custody or namespace and native observation failures.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }

    /// Join this exact original to an independently retained directory and leaf name.
    /// # Errors
    /// Refuses a different parent or leaf, changed custody and native errors.
    pub fn require_name(&self, parent: &OwnerDirectory, name: impl AsRef<OsStr>) -> io::Result<()> {
        self.inner
            .require_durability_name(&parent.inner, checked_name(name.as_ref())?)
    }

    /// Capture the revalidated original's exact native snapshot.
    /// # Errors
    /// Refuses changed custody, contents metadata or native errors.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        Ok(FileSnapshot {
            inner: self.inner.snapshot()?,
        })
    }

    /// Flush this existing original without releasing its custody.
    /// # Errors
    /// Returns the original flush error first, while still checking final custody.
    pub fn sync_all(&self) -> io::Result<()> {
        self.revalidate()?;
        let result = self.inner.file().sync_all();
        let final_check = self.revalidate();
        result.and(final_check)
    }

    fn observed<T>(&mut self, operation: impl FnOnce(&mut File) -> io::Result<T>) -> io::Result<T> {
        self.revalidate()?;
        let result = operation(self.inner.file_mut());
        let final_check = self.revalidate();
        result.and_then(|value| final_check.map(|()| value))
    }
}

impl Read for PrivateDurabilityFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.observed(|file| file.read(buffer))
    }
}

impl Seek for PrivateDurabilityFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.observed(|file| file.seek(position))
    }
}

#[cfg(test)]
#[path = "private_durability/tests.rs"]
mod tests;
