//! Unix retained-file custody borrowing an existing directory and caller-owned basename.

use super::*;

/// A bounded Unix writer borrowing its original directory authority and basename.
///
/// The descriptor and metadata live inline; opening this capability does not clone the
/// directory's retained lineage or allocate an owned basename. Both borrowed inputs must
/// outlive this capability. Native ACL/path scratch and the existing directory's allocations
/// remain the caller's admission responsibility. This is not a complete I/O memory budget.
///
/// ```compile_fail
/// use std::{ffi::OsStr, path::Path};
/// use iroha_fs::PrivateDirectory;
/// fn outlive_directory(path: &Path, name: &OsStr) {
///     let file = {
///         let directory = PrivateDirectory::open_exact(path).unwrap();
///         directory.create_borrowed_private(name, 64).unwrap()
///     };
///     drop(file);
/// }
/// ```
///
/// ```compile_fail
/// use std::ffi::OsString;
/// use iroha_fs::PrivateDirectory;
/// fn outlive_name(directory: &PrivateDirectory) {
///     let file = {
///         let name = OsString::from("receipt");
///         directory.create_borrowed_private(&name, 64).unwrap()
///     };
///     drop(file);
/// }
/// ```
///
/// ```compile_fail
/// use std::{ffi::OsStr, io::Write};
/// use iroha_fs::PrivateDirectory;
/// fn use_consumed_writer(directory: &PrivateDirectory) {
///     let mut file = directory.create_borrowed_private(OsStr::new("receipt"), 64).unwrap();
///     let sealed = file.seal_read_only().unwrap();
///     file.write_all(b"replacement").unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct BorrowedPendingPrivateFile<'a> {
    inner: platform::RetainedFile<&'a platform::Directory, &'a OsStr>,
    maximum: u64,
    position: u64,
}

/// A bounded Unix immutable reader borrowing its directory and current basename.
///
/// Uses the same strict access, identity, ancestry and publication checks as
/// [`SealedPrivateFile`]. No writable descriptor, clone or raw-handle accessor is exposed.
/// Reads require stable snapshots for content authentication. Reopening never restores
/// exclusive creation authority, and each created file may be published only once.
///
/// ```compile_fail
/// use std::io::Write;
/// use iroha_fs::BorrowedSealedPrivateFile;
/// fn rewrite(mut file: BorrowedSealedPrivateFile<'_>) {
///     file.write_all(b"replacement").unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use iroha_fs::BorrowedSealedPrivateFile;
/// fn duplicate(file: BorrowedSealedPrivateFile<'_>) {
///     let writable = file.file().try_clone().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct BorrowedSealedPrivateFile<'a> {
    inner: platform::RetainedFile<&'a platform::Directory, &'a OsStr>,
    maximum: u64,
    position: u64,
}

impl PrivateDirectory {
    /// Exclusively create a bounded private Unix writer borrowing this directory and name.
    ///
    /// The new file and its parent are synchronized before success. Existing names are never
    /// replaced or hardened. A refusal may leave an incomplete file for later reconciliation.
    /// Retained Rust directory/name copies are avoided; existing lineage and native I/O
    /// resources still require caller admission.
    ///
    /// # Errors
    /// Refuses invalid names, existing files, unsafe custody and native creation failures.
    pub fn create_borrowed_private<'a>(
        &'a self,
        name: &'a OsStr,
        maximum: usize,
    ) -> io::Result<BorrowedPendingPrivateFile<'a>> {
        let maximum = byte_ceiling(maximum)?;
        Ok(BorrowedPendingPrivateFile {
            inner: self.inner.create_borrowed_private(checked_name(name)?)?,
            maximum,
            position: 0,
        })
    }

    /// Reopen one bounded, strictly owner-read-only Unix file with borrowed authority.
    ///
    /// Requires exact mode `0400`; incomplete writable files are refused without repair.
    /// This borrows both directory lineage and basename without retaining Rust-owned copies.
    ///
    /// # Errors
    /// Refuses invalid names, excessive length, unsafe custody and native open failures.
    pub fn open_borrowed_read_only<'a>(
        &'a self,
        name: &'a OsStr,
        maximum: usize,
    ) -> io::Result<BorrowedSealedPrivateFile<'a>> {
        let maximum = byte_ceiling(maximum)?;
        let file = BorrowedSealedPrivateFile {
            inner: self.inner.open_borrowed_read_only(checked_name(name)?)?,
            maximum,
            position: 0,
        };
        file.len()?;
        Ok(file)
    }
}

impl<'a> BorrowedPendingPrivateFile<'a> {
    /// Consume writable custody into exact mode `0400` and synchronized immutable contents.
    ///
    /// The same descriptor, borrowed authority and original extent bound survive sealing.
    /// Any failure leaves the claimed name for reconciliation.
    ///
    /// # Errors
    /// Refuses changed custody, excessive extent, permission and synchronization failures.
    pub fn seal_read_only(self) -> io::Result<BorrowedSealedPrivateFile<'a>> {
        let file = BorrowedSealedPrivateFile {
            inner: self.inner.seal_read_only()?,
            maximum: self.maximum,
            position: self.position,
        };
        file.len()?;
        Ok(file)
    }
}

impl<'a> BorrowedSealedPrivateFile<'a> {
    /// Read the retained native object's kernel identity.
    ///
    /// # Errors
    /// Refuses changed identity or native metadata failure.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.inner.identity()
    }

    /// Capture exact metadata after checking the file, strict access and borrowed ancestry.
    ///
    /// # Errors
    /// Refuses changed content or custody and native metadata failures.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        Ok(FileSnapshot {
            inner: self.inner.snapshot()?,
        })
    }

    /// Recheck strict access, original content metadata and the borrowed namespace.
    ///
    /// # Errors
    /// Refuses changed content or custody and native metadata failures.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }

    /// Observe current length within the original byte ceiling.
    ///
    /// # Errors
    /// Refuses excessive extent or native metadata failure.
    pub fn len(&self) -> io::Result<u64> {
        bounded_length(self.inner.file(), self.maximum)
    }

    /// Whether the current bounded extent is empty.
    ///
    /// # Errors
    /// Refuses excessive extent or native metadata failure.
    pub fn is_empty(&self) -> io::Result<bool> {
        self.len().map(|length| length == 0)
    }

    /// Consume exclusive creation authority to publish under an absent borrowed sibling name.
    ///
    /// Keeps the exact native object and borrows the new name without cloning it. No replacement
    /// is allowed. An error can follow durable publication; callers must reconcile evidence.
    ///
    /// # Errors
    /// Refuses invalid names, existing destinations, reopened/already-published capabilities,
    /// changed custody and native publication or synchronization failures.
    pub fn publish_new_name(self, name: &'a OsStr) -> io::Result<Self> {
        Ok(Self {
            inner: self.inner.publish_with_name(checked_name(name)?)?,
            maximum: self.maximum,
            position: self.position,
        })
    }
}

impl io::Write for BorrowedPendingPrivateFile<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        bounded_write(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            bytes,
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(self.inner.file_mut())
    }
}

impl io::Read for BorrowedSealedPrivateFile<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        bounded_read(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            bytes,
        )
    }
}

impl io::Seek for BorrowedPendingPrivateFile<'_> {
    fn seek(&mut self, from: io::SeekFrom) -> io::Result<u64> {
        bounded_seek(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            from,
        )
    }
}

impl io::Seek for BorrowedSealedPrivateFile<'_> {
    fn seek(&mut self, from: io::SeekFrom) -> io::Result<u64> {
        bounded_seek(
            self.inner.file_mut(),
            self.maximum,
            &mut self.position,
            from,
        )
    }
}

// TODO: Admit inherited directory/native scratch through the original caller resource owner,
// and implement an equivalent Windows borrowed-custody seam before claiming portable budgets.

#[cfg(test)]
#[path = "borrowed_tests.rs"]
mod tests;
