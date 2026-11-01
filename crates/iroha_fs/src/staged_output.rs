//! Private native output staging with borrowed replay and explicit publication.

use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crate::{AtomicWriteStep, OwnerDirectory, checked_name, invalid, platform, read_at};

const CREATE_ATTEMPTS: usize = 128;

/// One private Windows staging original, retained until discard or publication.
///
/// The final destination is bound at creation. Reads and writes never reopen a
/// pathname. Drop removes only this unpublished original by handle; it never
/// removes the destination after native publication, including on a later error.
/// Existing safe public-read destination files may be replaced. Their ACLs are
/// not repaired; the replacement, like every newly created stage, is private.
#[derive(Debug)]
pub struct StagedOutputFile {
    inner: platform::RetainedFile,
    temporary: PathBuf,
    destination: PathBuf,
    destination_name: OsString,
    position: u64,
    published: bool,
}

/// A bounded replay of the same staging original, borrowed without writer authority.
///
/// The initial snapshot and independent offset remain fixed for this replay.
/// Borrowing prevents mutable access to the owner until the reader is released.
#[derive(Debug)]
pub struct StagedOutputReader<'a> {
    original: &'a StagedOutputFile,
    snapshot: platform::FileSnapshot,
    length: u64,
    position: u64,
}

impl OwnerDirectory {
    /// Create a private same-parent stage for one eventual regular output name.
    ///
    /// The directory may retain safe public-read permissions. Creation tries at
    /// most 128 fresh names, retrying only actual create-new collisions. The final
    /// destination is not opened or changed until explicit publication.
    ///
    /// # Errors
    /// Refuses invalid names, unsafe custody, exhausted names or native errors.
    pub fn stage_output(&self, destination: impl AsRef<OsStr>) -> io::Result<StagedOutputFile> {
        self.stage_output_with_names(destination.as_ref(), || {
            format!("output-{}", crate::temporary_name()).into()
        })
    }

    fn stage_output_with_names(
        &self,
        destination: &OsStr,
        mut next_name: impl FnMut() -> OsString,
    ) -> io::Result<StagedOutputFile> {
        let destination_name = checked_name(destination)?.to_owned();
        let destination = self.path().join(&destination_name);
        self.revalidate()?;
        for _ in 0..CREATE_ATTEMPTS {
            let name = next_name();
            let name = checked_name(&name)?;
            if name == destination_name.as_os_str() {
                continue;
            }
            let temporary = self.path().join(name);
            if let Some(inner) = self.inner.create_output_stage(name)? {
                return Ok(StagedOutputFile {
                    inner,
                    temporary,
                    destination,
                    destination_name,
                    position: 0,
                    published: false,
                });
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "private output staging names exhausted",
        ))
    }
}

impl StagedOutputFile {
    /// Return the staging spelling for diagnostics; this grants no pathname authority.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.temporary
    }

    /// Return the exact final spelling bound through the retained parent at creation.
    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    /// Recheck the original, its private custody, selected name and all ancestors.
    /// # Errors
    /// Refuses changed custody or native observation errors.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.require_current_owner()
    }

    /// Flush staging bytes and metadata through the original handle.
    /// # Errors
    /// Preserves the first sync error and refuses success after custody changes.
    pub fn sync_all(&self) -> io::Result<()> {
        self.revalidate()?;
        let result = self.inner.file().sync_all();
        let custody = self.revalidate();
        result.and(custody)
    }

    /// Borrow an independent replay beginning at offset zero without reopening a path.
    ///
    /// Callers must flush their own outer buffers before requesting replay. Native
    /// positioned reads may move the physical cursor; later mutable operations
    /// always restore this owner's separate logical cursor first.
    /// # Errors
    /// Refuses unsafe or changed staging custody.
    pub fn reader(&self) -> io::Result<StagedOutputReader<'_>> {
        self.revalidate()?;
        let snapshot = self.inner.snapshot()?;
        Ok(StagedOutputReader {
            original: self,
            snapshot,
            length: snapshot.local_parts().2,
            position: 0,
        })
    }

    /// Sync and atomically publish this complete original at its bound destination.
    ///
    /// This is one-file publication, not a transaction across multiple outputs.
    /// Before the native rename, failure discards only the unpublished stage.
    /// After rename, failure leaves the complete destination for reconciliation;
    /// neither Drop nor retry on a consumed owner removes or republishes it.
    /// Existing safe public-read files are admitted without changing their ACLs
    /// before replacement. The new destination retains the private staging ACL.
    /// # Errors
    /// Refuses unsafe destinations, sharing conflicts, changed custody or I/O errors.
    pub fn publish(mut self) -> io::Result<()> {
        self.publish_observed(|_| Ok(()))
    }

    fn publish_observed(
        &mut self,
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.published {
            return Err(invalid("output stage has already been published"));
        }
        let result = (|| {
            observe(AtomicWriteStep::SyncFile)?;
            self.sync_all()?;
            self.inner.publish_output_stage(
                &self.destination_name,
                &mut self.published,
                &mut observe,
            )
        })();
        let custody = self.revalidate();
        result.and(custody)
    }

    fn positioned<T>(
        &mut self,
        operation: impl FnOnce(&mut std::fs::File) -> io::Result<(T, u64)>,
    ) -> io::Result<T> {
        self.revalidate()?;
        let result = self
            .inner
            .file_mut()
            .seek(SeekFrom::Start(self.position))
            .and_then(|_| operation(self.inner.file_mut()));
        if let Ok((_, position)) = &result {
            self.position = *position;
        }
        let custody = self.revalidate();
        result.and_then(|(value, _)| custody.map(|()| value))
    }
}

impl Read for StagedOutputFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let position = self.position;
        self.positioned(|file| {
            let count = file.read(bytes)?;
            let next = position
                .checked_add(count as u64)
                .ok_or_else(|| invalid("output read position overflow"))?;
            Ok((count, next))
        })
    }
}

impl Write for StagedOutputFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let position = self.position;
        self.positioned(|file| {
            let count = file.write(bytes)?;
            let next = position
                .checked_add(count as u64)
                .ok_or_else(|| invalid("output write position overflow"))?;
            Ok((count, next))
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.revalidate()?;
        let result = self.inner.file_mut().flush();
        let custody = self.revalidate();
        result.and(custody)
    }
}

impl Seek for StagedOutputFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.positioned(|file| file.seek(position).map(|position| (position, position)))
    }
}

impl Read for StagedOutputReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let check = || {
            self.original.revalidate()?;
            if self.original.inner.snapshot()? != self.snapshot {
                return Err(crate::changed());
            }
            Ok(())
        };
        check()?;
        let limit = usize::try_from((self.length - self.position).min(bytes.len() as u64))
            .map_err(|_| invalid("output replay length overflow"))?;
        let result = if limit == 0 {
            Ok(0)
        } else {
            read_at(
                self.original.inner.file(),
                &mut bytes[..limit],
                self.position,
            )
            .and_then(|count| {
                if count == 0 {
                    Err(io::ErrorKind::UnexpectedEof.into())
                } else {
                    Ok(count)
                }
            })
        };
        let custody = check();
        let count = result.and_then(|count| custody.map(|()| count))?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Drop for StagedOutputFile {
    fn drop(&mut self) {
        if !self.published {
            // This can fail on genuine native I/O/ACL changes. Never compensate
            // with a pathname deletion or touch the separately bound destination.
            let _ = self.inner.discard_output_stage();
        }
    }
}

#[cfg(test)]
#[path = "staged_output/tests.rs"]
mod tests;
