//! Native immutable flat batches with continuous original-handle reader handoff.

use super::*;
use crate::{FlatBatchEntry, FlatBatchKind, FlatBatchSealStep};
use directory_move::SecuritySnapshot;
use private_files::InventoryChanges;
use windows_sys::Win32::Foundation::GENERIC_EXECUTE;

/// Original parent security, captured before child creation or observer callbacks.
#[derive(Debug)]
pub struct FlatBatchCreation {
    security: SecuritySnapshot,
    parent_security: InventoryChanges,
}

impl FlatBatchCreation {
    pub(crate) fn revalidate(&self, directory: &Directory) -> io::Result<()> {
        self.parent_security
            .require_unchanged()
            .map_err(|_| io::Error::other("batch creator parent security notification changed"))?;
        directory.revalidate()?;
        if self.security != SecuritySnapshot::of(&directory.current().file)? {
            return Err(io::Error::other(
                "batch creator parent security descriptor changed",
            ));
        }
        self.parent_security.require_unchanged()
    }
}

/// Exact immutable directory custody shared by every retained descendant.
#[derive(Debug)]
pub(super) struct DirectorySeal {
    before: Snapshot,
    security: SecuritySnapshot,
    changes: InventoryChanges,
    parent_security: InventoryChanges,
}

impl DirectorySeal {
    fn admit(file: &File, path: &Path) -> io::Result<Self> {
        let changes = InventoryChanges::begin(path)?;
        let parent_security = InventoryChanges::begin_security(path.parent().ok_or_else(changed)?)?;
        private_files::validate_executable(file)?;
        let seal = Self {
            before: snapshot(file, true, true)?,
            security: SecuritySnapshot::of(file)?,
            changes,
            parent_security,
        };
        seal.revalidate(file)?;
        Ok(seal)
    }

    pub(super) fn revalidate(&self, file: &File) -> io::Result<()> {
        self.changes.require_unchanged()?;
        self.parent_security.require_unchanged()?;
        private_files::validate_executable(file)?;
        if self.before != snapshot(file, true, true)?
            || self.security != SecuritySnapshot::of(file)?
        {
            return Err(changed());
        }
        self.changes.require_unchanged()?;
        self.parent_security.require_unchanged()
    }
}

impl Directory {
    pub(crate) fn capture_flat_batch_creation(&self) -> io::Result<FlatBatchCreation> {
        let parent_security =
            InventoryChanges::begin_security(self.path().parent().ok_or_else(changed)?)?;
        let creation = FlatBatchCreation {
            security: SecuritySnapshot::of(&self.current().file)?,
            parent_security,
        };
        creation.revalidate(self)?;
        Ok(creation)
    }

    pub(crate) fn create_flat_batch_directory(&self, name: &OsStr) -> io::Result<Self> {
        self.revalidate()?;
        if self.links.len() >= 128 {
            return Err(invalid("private directory depth bound exceeded"));
        }
        let path = self.path().join(name);
        create_directory(&path)?;
        // Acquire all creator rights at the first admitted handle. No later owner
        // gains WRITE_DAC by reopening a path selected after admission.
        let file = open_file(
            &path,
            FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES | WRITE_DAC,
            FILE_SHARE_READ,
            OPEN_EXISTING,
            true,
            false,
        )?;
        snapshot(&file, true, true)?;
        validate_current_owner(&file)?;
        sync_directory_metadata(&file)?;
        let mut links = self.links.clone();
        links.push(Arc::new(Link {
            path,
            file,
            private: true,
            sealed: None,
        }));
        let result = Self { links };
        self.sync()?;
        result.revalidate()?;
        Ok(result)
    }

    pub(crate) fn open_flat_batch_directory(&self, name: &OsStr) -> io::Result<Self> {
        self.revalidate()?;
        if self.links.len() >= 128 {
            return Err(invalid("private directory depth bound exceeded"));
        }
        let path = self.path().join(name);
        let file = open_file(
            &path,
            GENERIC_READ | GENERIC_EXECUTE,
            FILE_SHARE_READ,
            OPEN_EXISTING,
            true,
            false,
        )?;
        let sealed = Some(Arc::new(DirectorySeal::admit(&file, &path)?));
        let mut links = self.links.clone();
        links.push(Arc::new(Link {
            path,
            file,
            private: true,
            sealed,
        }));
        let result = Self { links };
        result.revalidate()?;
        Ok(result)
    }

    pub(crate) fn open_flat_batch_file(
        &self,
        name: &OsStr,
        kind: FlatBatchKind,
    ) -> io::Result<FlatBatchFile> {
        self.revalidate()?;
        let file = open_file(
            &self.path().join(name),
            access(kind),
            FILE_SHARE_READ,
            OPEN_EXISTING,
            false,
            false,
        )?;
        let before = snapshot(&file, true, false)?;
        validate_kind(&file, kind)?;
        let reader = retained(self.clone(), name.to_owned(), file, before, kind);
        reader.revalidate()?;
        FlatBatchFile::admit(reader)
    }
}

fn access(kind: FlatBatchKind) -> u32 {
    GENERIC_READ
        | if kind == FlatBatchKind::Executable {
            GENERIC_EXECUTE
        } else {
            0
        }
}

fn validate_kind(file: &File, kind: FlatBatchKind) -> io::Result<()> {
    match kind {
        FlatBatchKind::Executable => private_files::validate_executable(file),
        FlatBatchKind::ReadOnly => private_files::validate_read_only(file),
    }
}

fn retained(
    directory: Directory,
    name: std::ffi::OsString,
    file: File,
    before: Snapshot,
    kind: FlatBatchKind,
) -> RetainedFile {
    RetainedFile {
        directory,
        name,
        file,
        before,
        links: LinkPolicy::Single,
        private: true,
        writable: false,
        read_only: kind == FlatBatchKind::ReadOnly,
        executable: kind == FlatBatchKind::Executable,
        publishable: false,
        ancestry: AncestryCheck::EachCall,
    }
}

pub fn seal_flat_batch<const N: usize>(
    mut directory: Directory,
    creation: FlatBatchCreation,
    files: Vec<RetainedFile>,
    entries: &[FlatBatchEntry; N],
    observe: &mut impl FnMut(FlatBatchSealStep) -> io::Result<()>,
) -> io::Result<(Directory, Vec<FlatBatchFile>)> {
    let mut originals = Vec::with_capacity(N);
    for (index, (file, entry)) in files.into_iter().zip(entries).enumerate() {
        creation.revalidate(&directory)?;
        let sealed = match entry.kind() {
            FlatBatchKind::Executable => file.seal_executable()?,
            FlatBatchKind::ReadOnly => file.seal_read_only()?,
        };
        sealed.revalidate()?;
        let security = SecuritySnapshot::of(&sealed.file)?;
        observe(FlatBatchSealStep::FileSealed(index))?;
        creation.revalidate(&directory)?;
        sealed.revalidate()?;
        validate_original(&sealed.file, sealed.before, &security, entry.kind())?;
        let RetainedFile {
            file, name, before, ..
        } = sealed;
        originals.push((file, name, before, security));
    }
    creation.revalidate(&directory)?;
    let current = directory.links.pop().ok_or_else(changed)?;
    let mut link =
        Arc::try_unwrap(current).map_err(|_| denied("batch parent has an escaped owner"))?;
    private_files::set_sealed_acl(&link.file, true)?;
    sync_directory_metadata(&link.file)?;
    if !creation
        .security
        .same_principals_after_dacl_seal(&SecuritySnapshot::of(&link.file)?)
    {
        return Err(io::Error::other(
            "batch parent principals or descriptor control changed during sealing",
        ));
    }
    // The old watch was checked before this deliberate DACL transition. Its own
    // authored event is never cleared or reused as evidence of an unchanged root.
    drop(creation);
    link.sealed = Some(Arc::new(DirectorySeal::admit(&link.file, &link.path)?));
    observe(FlatBatchSealStep::DirectorySealed)?;
    link.sealed
        .as_ref()
        .ok_or_else(changed)?
        .revalidate(&link.file)?;
    // WRITE_DAC/WRITE_ATTRIBUTES are metadata authority, so the final read/execute
    // parent can coexist with this original creator under read-only sharing.
    let parent_reader = open_file(
        &link.path,
        GENERIC_READ | GENERIC_EXECUTE,
        FILE_SHARE_READ,
        OPEN_EXISTING,
        true,
        false,
    )?;
    link.sealed
        .as_ref()
        .ok_or_else(changed)?
        .revalidate(&parent_reader)?;
    drop(std::mem::replace(&mut link.file, parent_reader));
    directory.links.push(Arc::new(link));
    observe(FlatBatchSealStep::ParentTransferred)?;
    directory.revalidate()?;

    let mut readers = Vec::with_capacity(N);
    for (index, ((creator, name, before, security), entry)) in
        originals.into_iter().zip(entries).enumerate()
    {
        // A reduced DuplicateHandle still carries the creator FileObject's original
        // sharing state. An independently opened reader is the necessary bridge.
        let bridge = open_file(
            &directory.path().join(&name),
            access(entry.kind()),
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        )?;
        validate_original(&bridge, before, &security, entry.kind())?;
        validate_original(&creator, before, &security, entry.kind())?;
        observe(FlatBatchSealStep::ChildBridged(index))?;
        validate_original(&bridge, before, &security, entry.kind())?;
        validate_original(&creator, before, &security, entry.kind())?;
        directory.revalidate()?;
        drop(creator);
        let file = open_file(
            &directory.path().join(&name),
            access(entry.kind()),
            FILE_SHARE_READ,
            OPEN_EXISTING,
            false,
            false,
        )?;
        validate_original(&file, before, &security, entry.kind())?;
        validate_original(&bridge, before, &security, entry.kind())?;
        drop(bridge);
        let reader = retained(directory.clone(), name, file, before, entry.kind());
        observe(FlatBatchSealStep::ChildTransferred(index))?;
        reader.revalidate()?;
        readers.push(FlatBatchFile {
            inner: reader,
            security,
        });
    }
    directory.revalidate()?;
    Ok((directory, readers))
}

fn validate_original(
    file: &File,
    before: Snapshot,
    security: &SecuritySnapshot,
    kind: FlatBatchKind,
) -> io::Result<()> {
    validate_kind(file, kind)?;
    if before != snapshot(file, true, false)? || *security != SecuritySnapshot::of(file)? {
        return Err(changed());
    }
    Ok(())
}

/// One immutable leaf, including the native security fields outside file metadata.
#[derive(Debug)]
pub struct FlatBatchFile {
    inner: RetainedFile,
    security: SecuritySnapshot,
}

impl FlatBatchFile {
    fn admit(inner: RetainedFile) -> io::Result<Self> {
        let security = SecuritySnapshot::of(inner.file())?;
        let file = Self { inner, security };
        file.revalidate()?;
        Ok(file)
    }

    pub(crate) fn file(&self) -> &File {
        self.inner.file()
    }

    pub(crate) fn snapshot(&self) -> io::Result<FileSnapshot> {
        self.revalidate()?;
        self.inner.snapshot()
    }

    pub(crate) fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()?;
        if self.security != SecuritySnapshot::of(self.file())? {
            return Err(changed());
        }
        self.inner.revalidate()
    }
}
