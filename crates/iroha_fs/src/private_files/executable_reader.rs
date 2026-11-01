//! Bounded recovery of an existing native executable under its original retained ancestry.

use super::*;

/// An opaque bounded reader retaining one strictly sealed executable and its ancestry.
///
/// Admission requires a single-link current-owner file with exact Unix `0500` or a
/// protected Windows current-user-only read-and-execute DACL. Every operation checks
/// the original identity, metadata, access policy and named ancestry. This capability
/// cannot write, clone, expose a descriptor, publish or move the file.
///
/// The enclosing directory retains ordinary safe reader policy; it is not sealed.
/// This type does not authenticate executable contents or bind a pathname-based spawn
/// to an authenticated release bundle. A caller loading code must retain this reader
/// throughout loading and check its original snapshot afterward.
///
/// ```compile_fail
/// use std::io::Write;
/// use iroha_fs::SealedExecutable;
/// fn rewrite(mut executable: SealedExecutable) {
///     executable.write_all(b"replacement").unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use iroha_fs::SealedExecutable;
/// fn expose(executable: SealedExecutable) {
///     let descriptor = executable.file();
/// }
/// ```
///
/// ```compile_fail
/// use iroha_fs::SealedExecutable;
/// fn duplicate(executable: SealedExecutable) {
///     let duplicate = executable.clone();
/// }
/// ```
///
/// ```compile_fail
/// use iroha_fs::SealedExecutable;
/// fn acquire_publication(executable: SealedExecutable) {
///     executable.into_movable().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct SealedExecutable {
    inner: platform::RetainedFile,
    maximum: u64,
    position: u64,
}

impl ReaderDirectory {
    /// Recover one bounded, strictly sealed executable under these original ancestors.
    ///
    /// Opens only read authority. Writable files and ordinary read-only receipts are
    /// rejected without changing their access policy. The returned reader retains this
    /// directory's original ancestry independently of this wrapper's lifetime.
    ///
    /// # Errors
    /// Refuses invalid names, excessive length, unsafe executable access, links, changed
    /// custody and native errors. Missing executables remain errors.
    pub fn open_sealed_executable(
        &self,
        name: impl AsRef<OsStr>,
        maximum: usize,
    ) -> io::Result<SealedExecutable> {
        self.inner.revalidate()?;
        let result = (|| {
            let maximum = byte_ceiling(maximum)?;
            let file = SealedExecutable {
                inner: self
                    .inner
                    .open_retained_executable(checked_name(name.as_ref())?)?,
                maximum,
                position: 0,
            };
            file.revalidate()?;
            Ok(file)
        })();
        self.inner.revalidate()?;
        result
    }
}

impl SealedExecutable {
    /// Return the full original native identity after checking executable custody.
    ///
    /// # Errors
    /// Refuses changed metadata, access, ancestry, excessive length or native errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        let result = self.inner.identity();
        self.revalidate()?;
        result
    }

    /// Capture original identity and metadata under the strict executable policy.
    ///
    /// # Errors
    /// Refuses changed metadata, access, ancestry, excessive length or native errors.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        self.revalidate()?;
        let result = self.inner.snapshot().map(|inner| FileSnapshot { inner });
        self.revalidate()?;
        result
    }

    /// Check the original executable, bounded extent and every retained ancestor.
    ///
    /// # Errors
    /// Refuses changed metadata, access, ancestry, excessive length or native errors.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()?;
        bounded_length(self.inner.file(), self.maximum)?;
        self.inner.revalidate()
    }

    /// Return the original bounded length under unchanged executable custody.
    ///
    /// # Errors
    /// Refuses changed metadata, access, ancestry, excessive length or native errors.
    pub fn len(&self) -> io::Result<u64> {
        self.revalidate()?;
        let result = bounded_length(self.inner.file(), self.maximum);
        self.revalidate()?;
        result
    }

    /// Whether the original executable extent is empty.
    ///
    /// # Errors
    /// Returns the same custody, extent and native errors as [`Self::len`].
    pub fn is_empty(&self) -> io::Result<bool> {
        self.len().map(|length| length == 0)
    }
}

impl Read for SealedExecutable {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.revalidate()?;
        let result = bounded_read(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            bytes,
        );
        self.revalidate()?;
        result
    }
}

impl Seek for SealedExecutable {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.revalidate()?;
        let result = bounded_seek(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            from,
        );
        self.revalidate()?;
        result
    }
}

#[cfg(test)]
mod tests;
