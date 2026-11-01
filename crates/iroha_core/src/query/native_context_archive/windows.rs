//! Original Windows archive directories, immutable readers and no-replace publication.

use super::*;
use iroha_fs::{
    FileIdentity, FileMoveStep, FileSnapshot, OwnerDirectory, ReaderDirectory, RetainedFile,
};

pub(crate) enum ArchiveDirectory {
    Owner(OwnerDirectory),
    Reader(ReaderDirectory),
}

impl ArchiveDirectory {
    pub(crate) fn from_owner(owner: OwnerDirectory) -> Self {
        Self::Owner(owner)
    }

    pub(crate) fn identity(&self) -> io::Result<FileIdentity> {
        match self {
            Self::Owner(owner) => owner.identity(),
            Self::Reader(reader) => reader.identity(),
        }
    }

    fn revalidate(&self) -> io::Result<()> {
        match self {
            Self::Owner(owner) => owner.revalidate(),
            Self::Reader(reader) => reader.revalidate(),
        }
    }

    fn owner(&self) -> io::Result<&OwnerDirectory> {
        match self {
            Self::Owner(owner) => Ok(owner),
            Self::Reader(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only archive has no publication authority",
            )),
        }
    }
}

/// A selected original file, whose private metadata and ancestry survive refused polls.
pub(super) struct ArchiveFile {
    original: RetainedFile,
    snapshot: FileSnapshot,
}

impl ArchiveFile {
    #[cfg(test)]
    pub(super) fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        self.original.identity()
    }

    pub(super) fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if FileSnapshot::of(self.original.file(), true)? != self.snapshot {
            return Err(io::Error::other("native archive original metadata changed"));
        }
        self.original.revalidate()
    }

    pub(super) fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.revalidate()?;
        let metadata = self.original.file().metadata()?;
        self.revalidate()?;
        Ok(metadata)
    }
}

impl Read for ArchiveFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.revalidate()?;
        let result = self.original.file_mut().read(bytes);
        self.revalidate()?;
        result
    }
}

impl Seek for ArchiveFile {
    fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
        self.revalidate()?;
        let result = self.original.file_mut().seek(position);
        self.revalidate()?;
        result
    }
}

pub(super) fn open_directory(
    store_root: &Path,
    create: bool,
) -> io::Result<(ArchiveDirectory, ArchiveDirectory)> {
    let root = if create {
        ArchiveDirectory::Owner(OwnerDirectory::open(store_root)?)
    } else {
        let reader = ReaderDirectory::open(store_root)?;
        reader.require_current_owner()?;
        ArchiveDirectory::Reader(reader)
    };
    let directory = open_archive_directory_named(&root, "native-contexts", create)?;
    Ok((root, directory))
}

pub(super) fn open_archive_directory_named(
    root: &ArchiveDirectory,
    namespace: &str,
    create: bool,
) -> io::Result<ArchiveDirectory> {
    root.revalidate()?;
    let directory = match root {
        ArchiveDirectory::Owner(owner) => ArchiveDirectory::Owner(if create {
            owner.ensure_child(namespace)?
        } else {
            owner.open_child(namespace)?
        }),
        ArchiveDirectory::Reader(reader) if !create => {
            let child = reader.open_child(namespace)?;
            child.require_current_owner()?;
            ArchiveDirectory::Reader(child)
        }
        ArchiveDirectory::Reader(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only archive cannot create a namespace",
            ));
        }
    };
    directory.revalidate()?;
    root.revalidate()?;
    Ok(directory)
}

pub(super) fn verify_archive_directory(
    root: &ArchiveDirectory,
    namespace: &str,
    original: &ArchiveDirectory,
) -> io::Result<()> {
    original.revalidate()?;
    let named = open_archive_directory_named(root, namespace, false)?;
    if named.identity()? != original.identity()? {
        return Err(io::Error::other(
            "native context directory identity changed",
        ));
    }
    original.revalidate()?;
    root.revalidate()
}

pub(super) fn open_named_record(
    directory: &ArchiveDirectory,
    name: &RecordName,
) -> io::Result<ArchiveFile> {
    directory.revalidate()?;
    let original = match directory {
        ArchiveDirectory::Owner(owner) => owner
            .open_retained_regular_optional(name.as_str())?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?,
        ArchiveDirectory::Reader(reader) => reader.open_retained_private(name.as_str())?,
    };
    let snapshot = FileSnapshot::of(original.file(), true)?;
    let file = ArchiveFile { original, snapshot };
    file.revalidate()?;
    directory.revalidate()?;
    Ok(file)
}

pub(super) fn publish_named_record(
    directory: &ArchiveDirectory,
    target: &RecordName,
    staged: &RecordName,
    bytes: &[u8],
) -> io::Result<()> {
    let owner = directory.owner()?;
    match open_named_record(directory, target) {
        Ok(mut file) => {
            verify_stream(&mut file, bytes)?;
            owner.sync()?;
            remove_matching_stage(directory, staged, bytes)?;
            file.revalidate()?;
            directory.revalidate()?;
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // A stage carries no source authority. Recovery discards only the canonical owner's
    // exact private single-link stage and authors a fresh original from retained bytes.
    // No recovered file is converted into a creator or mutable publication capability.
    owner.remove_private(staged.as_str())?;
    let mut pending = owner.create_retained_private(staged.as_str(), bytes.len().max(1))?;
    pending.write_all(bytes)?;
    let source = pending.finish_private()?;
    #[cfg(test)]
    process_crash::observe(target, process_crash::Boundary::StageSynced);
    let mut published = false;
    let moved = source.move_no_replace_observed(owner, target.as_str(), |step| {
        if step == FileMoveStep::Published {
            published = true;
        }
        Ok(())
    });
    let mut moved = match moved {
        Ok(moved) => moved,
        Err(error) if !published && error.kind() == io::ErrorKind::AlreadyExists => {
            let mut file = open_named_record(directory, target)?;
            verify_stream(&mut file, bytes)?;
            owner.sync()?;
            remove_matching_stage(directory, staged, bytes)?;
            file.revalidate()?;
            directory.revalidate()?;
            return Ok(());
        }
        // In particular, do not erase either name following uncertain publication.
        Err(error) => return Err(error),
    };
    moved.rewind()?;
    verify_stream(&mut moved, bytes)?;
    moved.require_source_absent()?;
    directory.revalidate()?;
    #[cfg(test)]
    process_crash::observe(target, process_crash::Boundary::CanonicalSynced);
    // Native no-replace rename has already removed the staging name. This boundary
    // retains the same complete canonical bytes and original parent/leaf authority.
    moved.require_source_absent()?;
    #[cfg(test)]
    process_crash::observe(target, process_crash::Boundary::Complete);
    Ok(())
}

fn remove_matching_stage(
    directory: &ArchiveDirectory,
    name: &RecordName,
    expected: &[u8],
) -> io::Result<()> {
    let mut original = match open_named_record(directory, name) {
        Ok(original) => original,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    verify_stream(&mut original, expected)?;
    original.revalidate()?;
    let snapshot = original.snapshot;
    // Transfer only the exact observed private snapshot to the canonical DELETE opener.
    // The first reader excludes DELETE sharing; a replacement during transfer refuses.
    drop(original);
    if !directory
        .owner()?
        .remove_private_if_snapshot(name.as_str(), &snapshot)?
    {
        return Err(io::Error::other(
            "matching native stage disappeared before removal",
        ));
    }
    directory.revalidate()
}
