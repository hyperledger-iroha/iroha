//! Creator-only flat immutable directory batches and retained recovery.

use super::*;
use crate::private_files::{
    bounded_length, bounded_seek, bounded_write, byte_ceiling, seek_target,
};

#[cfg(any(target_os = "linux", target_os = "android"))]
#[path = "sealed_batch/tool_image.rs"]
mod tool_image;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use tool_image::SealedToolImage;

/// Exact access policy of one immutable batch member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatBatchKind {
    /// Owner read and execute access, with no writable capability after sealing.
    Executable,
    /// Owner read access only.
    ReadOnly,
}

/// One checked name, exact access policy and immutable byte ceiling.
#[derive(Clone, Debug)]
pub struct FlatBatchEntry {
    name: std::ffi::OsString,
    kind: FlatBatchKind,
    maximum: u64,
}

impl FlatBatchEntry {
    /// Describe a direct child; bounds apply during creation and recovery.
    ///
    /// # Errors
    /// Refuses invalid child names or a byte ceiling exceeding native extent.
    pub fn new(name: impl AsRef<OsStr>, kind: FlatBatchKind, maximum: usize) -> io::Result<Self> {
        Ok(Self {
            name: checked_name(name.as_ref())?.to_owned(),
            kind,
            maximum: byte_ceiling(maximum)?,
        })
    }

    /// The exact direct-child name.
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    /// The access policy required at sealing and every subsequent check.
    pub const fn kind(&self) -> FlatBatchKind {
        self.kind
    }

    /// The maximum permitted extent, including sparse data.
    pub const fn maximum(&self) -> u64 {
        self.maximum
    }
}

/// Exclusive creator of a fixed, flat directory and all its original children.
///
/// No directory, file handle or independently owned writer escapes this batch.
/// Sealing consumes every creator before exposing the immutable owner. Failure
/// preserves the claimed namespace for explicit reconciliation; it never cleans
/// up files selected again by pathname.
///
/// ```compile_fail
/// use iroha_fs::PendingFlatBatch;
/// fn escape(batch: PendingFlatBatch<5>) { let file = batch.file(); }
/// ```
#[derive(Debug)]
pub struct PendingFlatBatch<const N: usize> {
    directory: platform::Directory,
    creation: platform::FlatBatchCreation,
    entries: [FlatBatchEntry; N],
    files: Vec<platform::RetainedFile>,
    positions: [u64; N],
}

/// A fixed inventory of immutable originals under their immutable original parent.
///
/// This retains all native readers and their complete ancestry. The names and
/// access policies are authenticated here; callers still authenticate contents.
/// A pathname returned for launching is only an observation: the launch owner
/// must keep this batch alive and revalidate before spawn and after process exit.
///
/// ```compile_fail
/// use iroha_fs::SealedFlatBatch;
/// fn rewrite(mut batch: SealedFlatBatch<5>) { batch.writer(0).unwrap(); }
/// ```
///
/// ```compile_fail
/// use iroha_fs::SealedFlatBatch;
/// fn duplicate(batch: SealedFlatBatch<5>) { batch.clone(); }
/// ```
#[derive(Debug)]
pub struct SealedFlatBatch<const N: usize> {
    directory: platform::Directory,
    entries: [FlatBatchEntry; N],
    files: Vec<platform::FlatBatchFile>,
}

/// Borrowed bounded writer; it cannot outlive or independently seal its batch.
#[derive(Debug)]
pub struct FlatBatchWriter<'a, const N: usize> {
    batch: &'a mut PendingFlatBatch<N>,
    index: usize,
}

/// Borrowed bounded reader with an independent position and no native-handle escape.
#[derive(Debug)]
pub struct FlatBatchReader<'a, const N: usize> {
    batch: &'a SealedFlatBatch<N>,
    index: usize,
    position: u64,
}

/// Observable sealing boundaries for deterministic interruption and custody checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatBatchSealStep {
    /// The indexed original child has received its final access policy and data sync.
    FileSealed(usize),
    /// The original directory has its final policy; the mutation observation is armed.
    DirectorySealed,
    /// The original directory has transferred to its final retained reader.
    ParentTransferred,
    /// An independent reader now bridges the indexed child while its creator is live.
    ChildBridged(usize),
    /// The indexed child has transferred to its final retained reader.
    ChildTransferred(usize),
    /// All creators are consumed; final inventory and custody checks follow.
    Validate,
}

fn check_entries<const N: usize>(entries: &[FlatBatchEntry; N]) -> io::Result<()> {
    if N == 0 {
        return Err(invalid("a flat batch requires at least one entry"));
    }
    for (index, entry) in entries.iter().enumerate() {
        checked_name(&entry.name)?;
        if entries[..index]
            .iter()
            .any(|other| other.name == entry.name)
        {
            return Err(invalid("flat batch names must be distinct"));
        }
    }
    Ok(())
}

fn check_inventory<const N: usize>(
    directory: &platform::Directory,
    entries: &[FlatBatchEntry; N],
) -> io::Result<()> {
    let names = directory.entries(N)?;
    if names.len() != N
        || names
            .iter()
            .any(|name| !entries.iter().any(|entry| &entry.name == name))
    {
        return Err(invalid(
            "flat batch inventory differs from its fixed entries",
        ));
    }
    Ok(())
}

fn create<const N: usize>(
    parent: &platform::Directory,
    name: &OsStr,
    entries: [FlatBatchEntry; N],
) -> io::Result<PendingFlatBatch<N>> {
    check_entries(&entries)?;
    let directory = parent.create_flat_batch_directory(checked_name(name)?)?;
    let creation = directory.capture_flat_batch_creation()?;
    let mut files = Vec::with_capacity(N);
    for entry in &entries {
        files.push(directory.create_retained_private(&entry.name)?);
    }
    check_inventory(&directory, &entries)?;
    creation.revalidate(&directory)?;
    Ok(PendingFlatBatch {
        directory,
        creation,
        entries,
        files,
        positions: [0; N],
    })
}

impl OwnerDirectory {
    /// Exclusively create one private final-name directory and its fixed child inventory.
    ///
    /// # Errors
    /// Refuses empty/duplicate inventories, existing names, unsafe ancestry and native errors.
    pub fn create_flat_batch<const N: usize>(
        &self,
        name: impl AsRef<OsStr>,
        entries: [FlatBatchEntry; N],
    ) -> io::Result<PendingFlatBatch<N>> {
        create(&self.inner, name.as_ref(), entries)
    }
}

impl PrivateDirectory {
    /// Exclusively create a flat immutable batch below this original private parent.
    ///
    /// # Errors
    /// Refuses empty/duplicate inventories, existing names, unsafe ancestry and native errors.
    pub fn create_flat_batch<const N: usize>(
        &self,
        name: impl AsRef<OsStr>,
        entries: [FlatBatchEntry; N],
    ) -> io::Result<PendingFlatBatch<N>> {
        create(&self.inner, name.as_ref(), entries)
    }
}

impl ReaderDirectory {
    /// Recover the exact immutable direct-child directory and all its sealed original files.
    ///
    /// No existing policy is repaired. Admission retains both the directory mutation
    /// observation and every child; no selected parent is reopened as authority.
    ///
    /// # Errors
    /// Refuses a mutable directory, wrong inventory/policies, excessive extents, links,
    /// changed ancestry and native errors.
    pub fn open_sealed_flat_batch<const N: usize>(
        &self,
        name: impl AsRef<OsStr>,
        entries: [FlatBatchEntry; N],
    ) -> io::Result<SealedFlatBatch<N>> {
        check_entries(&entries)?;
        let directory = self
            .inner
            .open_flat_batch_directory(checked_name(name.as_ref())?)?;
        check_inventory(&directory, &entries)?;
        let mut files = Vec::with_capacity(N);
        for entry in &entries {
            files.push(directory.open_flat_batch_file(&entry.name, entry.kind)?);
        }
        let batch = SealedFlatBatch {
            directory,
            entries,
            files,
        };
        batch.revalidate()?;
        Ok(batch)
    }
}

impl<const N: usize> PendingFlatBatch<N> {
    /// Observe the full original directory identity before writing the manifest.
    ///
    /// This machine-local identity survives the policy transition. It is not a
    /// persistent cryptographic identity after every original handle has closed.
    ///
    /// # Errors
    /// Returns changed ancestry, inventory or native metadata errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.creation.revalidate(&self.directory)?;
        self.directory.revalidate()?;
        check_inventory(&self.directory, &self.entries)?;
        self.directory.identity()
    }

    /// Observe an indexed original file identity without exposing its creator handle.
    ///
    /// # Errors
    /// Refuses invalid indices, changed custody or native metadata errors.
    pub fn entry_identity(&self, index: usize) -> io::Result<FileIdentity> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.creation.revalidate(&self.directory)?;
        file.revalidate()?;
        let identity = FileIdentity::of(file.file())?;
        file.revalidate()?;
        self.directory.revalidate()?;
        Ok(identity)
    }

    /// Borrow the indexed creator with its original extent ceiling.
    ///
    /// # Errors
    /// Refuses an index outside the fixed inventory or changed directory custody.
    pub fn writer(&mut self, index: usize) -> io::Result<FlatBatchWriter<'_, N>> {
        self.entries
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.creation.revalidate(&self.directory)?;
        Ok(FlatBatchWriter { batch: self, index })
    }

    /// Seal every original child and the original parent, consuming all writable handles.
    ///
    /// # Errors
    /// Refuses changed custody, excessive extents, altered inventory and native errors.
    pub fn seal(self) -> io::Result<SealedFlatBatch<N>> {
        self.seal_observed(|_| Ok(()))
    }

    /// Seal with fallible observations at each custody-transfer boundary.
    ///
    /// Observers cannot acquire batch capabilities. An error preserves names and may
    /// leave a partially sealed directory, which is never returned as a sealed owner.
    ///
    /// # Errors
    /// Returns observer errors and all errors of [`Self::seal`].
    pub fn seal_observed(
        self,
        mut observe: impl FnMut(FlatBatchSealStep) -> io::Result<()>,
    ) -> io::Result<SealedFlatBatch<N>> {
        self.creation.revalidate(&self.directory)?;
        check_inventory(&self.directory, &self.entries)?;
        for (file, entry) in self.files.iter().zip(&self.entries) {
            file.revalidate()?;
            bounded_length(file.file(), entry.maximum)?;
        }
        let (directory, files) = platform::seal_flat_batch(
            self.directory,
            self.creation,
            self.files,
            &self.entries,
            &mut observe,
        )?;
        let batch = SealedFlatBatch {
            directory,
            entries: self.entries,
            files,
        };
        observe(FlatBatchSealStep::Validate)?;
        batch.revalidate()?;
        Ok(batch)
    }
}

impl<const N: usize> SealedFlatBatch<N> {
    /// Construct a native command for an executable original in this retained batch.
    ///
    /// Linux/Android use the original descriptor through `/proc/self/fd` and capture
    /// this owner inside the command; no mutable ancestor selects the executable.
    /// Windows uses the original pinned pathname. The caller must keep this batch
    /// alive through Windows spawn and must retain it through child exit on every
    /// platform, rechecking custody before spawn and after exit. A release layer
    /// should expose only its own command/child wrappers enforcing that lifetime.
    /// This does not authenticate the executable bytes or its dynamic dependencies.
    ///
    /// # Errors
    /// Refuses invalid indices, read-only entries, changed custody, non-native Linux
    /// image headers, missing descriptor filesystem, and platforms without a proven
    /// original-descriptor execution mechanism.
    pub fn command(self: &std::sync::Arc<Self>, index: usize) -> io::Result<crate::ProcessCommand> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        if entry.kind != FlatBatchKind::Executable {
            return Err(denied(
                "only an executable batch entry may construct a command",
            ));
        }
        self.revalidate()?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let command = crate::ProcessCommand::from_retained_descriptor(
            platform::flat_batch_command(self.files[index].file(), std::sync::Arc::clone(self))?,
        );
        #[cfg(windows)]
        let command = crate::ProcessCommand::new(self.entry_path(index)?)?;
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
        {
            // TODO: qualify descriptor-bound native execution on Darwin and other
            // Unix hosts before enabling this launch projection there. A pathname
            // fallback would reintroduce mutable-ancestor execution authority.
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "original-descriptor execution is not qualified on this Unix host",
            ));
        }
        #[cfg(any(windows, target_os = "linux", target_os = "android"))]
        {
            self.revalidate()?;
            Ok(command)
        }
    }

    /// Recheck the fixed inventory, every original leaf and the immutable parent ancestry.
    ///
    /// # Errors
    /// Refuses any policy, identity, content metadata, extent, namespace or observed mutation.
    pub fn revalidate(&self) -> io::Result<()> {
        self.directory.revalidate()?;
        check_inventory(&self.directory, &self.entries)?;
        for (file, entry) in self.files.iter().zip(&self.entries) {
            file.revalidate()?;
            bounded_length(file.file(), entry.maximum)?;
        }
        self.directory.revalidate()
    }

    /// Observe the directory path while retaining this owner as the actual authority.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Return the full original parent identity after complete batch validation.
    ///
    /// # Errors
    /// Returns custody or native metadata errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        self.directory.identity()
    }

    /// Observe a full original child identity after complete batch validation.
    ///
    /// # Errors
    /// Refuses invalid indices, changed custody or native metadata errors.
    pub fn entry_identity(&self, index: usize) -> io::Result<FileIdentity> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.revalidate()?;
        let identity = FileIdentity::of(file.file())?;
        self.revalidate()?;
        Ok(identity)
    }

    /// Observe an indexed child path under complete batch custody.
    ///
    /// # Errors
    /// Refuses invalid indices and any batch custody error.
    pub fn entry_path(&self, index: usize) -> io::Result<PathBuf> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.revalidate()?;
        Ok(self.path().join(&entry.name))
    }

    /// Capture the indexed original's full snapshot under complete batch custody.
    ///
    /// # Errors
    /// Refuses invalid indices, changed custody and native metadata errors.
    pub fn entry_snapshot(&self, index: usize) -> io::Result<FileSnapshot> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.revalidate()?;
        let snapshot = file.snapshot().map(|inner| FileSnapshot { inner });
        self.revalidate()?;
        snapshot
    }

    /// Borrow one reader with its own cursor without extracting any original capability.
    ///
    /// # Errors
    /// Refuses invalid indices and any batch custody error.
    pub fn reader(&self, index: usize) -> io::Result<FlatBatchReader<'_, N>> {
        self.entries
            .get(index)
            .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
        self.revalidate()?;
        Ok(FlatBatchReader {
            batch: self,
            index,
            position: 0,
        })
    }
}

impl<const N: usize> io::Write for FlatBatchWriter<'_, N> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let file = &mut self.batch.files[self.index];
        file.revalidate()?;
        let result = bounded_write(
            file.file_mut(),
            self.batch.entries[self.index].maximum,
            &mut self.batch.positions[self.index],
            bytes,
        );
        file.revalidate()?;
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let file = &mut self.batch.files[self.index];
        file.revalidate()?;
        let result = io::Write::flush(file.file_mut());
        file.revalidate()?;
        result
    }
}

impl<const N: usize> io::Seek for FlatBatchWriter<'_, N> {
    fn seek(&mut self, from: io::SeekFrom) -> io::Result<u64> {
        let file = &mut self.batch.files[self.index];
        file.revalidate()?;
        let result = bounded_seek(
            file.file_mut(),
            self.batch.entries[self.index].maximum,
            &mut self.batch.positions[self.index],
            from,
        );
        file.revalidate()?;
        result
    }
}

impl<const N: usize> io::Read for FlatBatchReader<'_, N> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.batch.revalidate()?;
        let remaining = self.batch.entries[self.index]
            .maximum
            .saturating_sub(self.position);
        let limit = bytes
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let result = crate::read_at(
            self.batch.files[self.index].file(),
            &mut bytes[..limit],
            self.position,
        );
        self.batch.revalidate()?;
        let read = result?;
        self.position += read as u64;
        Ok(read)
    }
}

impl<const N: usize> io::Seek for FlatBatchReader<'_, N> {
    fn seek(&mut self, from: io::SeekFrom) -> io::Result<u64> {
        self.batch.revalidate()?;
        let maximum = self.batch.entries[self.index].maximum;
        let length = bounded_length(self.batch.files[self.index].file(), maximum)?;
        self.position = seek_target(self.position, length, maximum, from)?;
        self.batch.revalidate()?;
        Ok(self.position)
    }
}

#[cfg(test)]
#[path = "sealed_batch/tests.rs"]
mod tests;
