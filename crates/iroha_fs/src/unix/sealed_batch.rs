//! Original Unix directory and child ownership across a closed immutable batch seal.

use super::*;
use crate::{FlatBatchEntry, FlatBatchKind, FlatBatchSealStep};
use std::os::unix::fs::PermissionsExt as _;

pub type FlatBatchFile = RetainedFile;

/// Original directory principals admitted before any child writer is created.
#[derive(Clone, Copy, Debug)]
pub struct FlatBatchCreation {
    owner: u32,
    group: u32,
}

impl FlatBatchCreation {
    pub(crate) fn revalidate(self, directory: &Directory) -> io::Result<()> {
        directory.revalidate()?;
        let metadata = directory.current().file.metadata()?;
        if metadata.uid() != self.owner
            || metadata.gid() != self.group
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(changed());
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(
    unsafe_code,
    reason = "pre_exec captures the original owner using only an allocation-free no-op after fork"
)]
pub fn flat_batch_command<T: Send + Sync + 'static>(
    file: &File,
    owner: Arc<T>,
) -> io::Result<std::process::Command> {
    use std::os::unix::process::CommandExt as _;
    let path = flat_batch_descriptor_path(file)?;
    let mut command = std::process::Command::new(path);
    // SAFETY: no allocation, locks, I/O or destructor runs in this callback. The
    // captured Arc keeps every original alive in the parent Command and child until
    // exec consumes the CLOEXEC descriptor. ELF execution needs no descriptor leak.
    unsafe {
        command.pre_exec(move || {
            std::hint::black_box(&owner);
            Ok(())
        });
    }
    Ok(command)
}

/// Check the native image and the exact kernel descriptor projection.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn flat_batch_descriptor_path(file: &File) -> io::Result<PathBuf> {
    use std::os::fd::AsRawFd as _;
    let mut magic = [0; 4];
    if crate::read_at(file, &mut magic, 0)? != 4 || magic != *b"\x7fELF" {
        return Err(invalid(
            "descriptor launch requires a native ELF executable",
        ));
    }
    let path = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
    // The kernel descriptor projection must resolve to this exact retained object;
    // a missing/unusable proc filesystem is an error, never a pathname fallback.
    let projected = File::open(&path)?;
    if identity(&projected)? != identity(file)? {
        return Err(changed());
    }
    Ok(path)
}

/// Immutable original-directory metadata carried by every retained descendant link.
#[derive(Debug)]
pub(super) struct DirectorySeal {
    before: fs::Metadata,
}

impl DirectorySeal {
    fn admit(file: &File) -> io::Result<Self> {
        validate_sealed_directory(file)?;
        Ok(Self {
            before: file.metadata()?,
        })
    }

    pub(super) fn revalidate(&self, file: &File) -> io::Result<()> {
        validate_sealed_directory(file)?;
        if !unchanged(&self.before, &file.metadata()?) {
            return Err(changed());
        }
        Ok(())
    }
}

fn validate_sealed_directory(file: &File) -> io::Result<()> {
    validate_directory(file, false)?;
    let metadata = file.metadata()?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o7777 != 0o500 {
        return Err(denied(
            "immutable batch directory requires exact owner mode 0500",
        ));
    }
    #[cfg(target_vendor = "apple")]
    apple_acl::validate(file, true)?;
    Ok(())
}

impl Directory {
    pub(crate) fn capture_flat_batch_creation(&self) -> io::Result<FlatBatchCreation> {
        let metadata = self.current().file.metadata()?;
        let creation = FlatBatchCreation {
            owner: metadata.uid(),
            group: metadata.gid(),
        };
        creation.revalidate(self)?;
        Ok(creation)
    }

    pub(crate) fn create_flat_batch_directory(&self, name: &OsStr) -> io::Result<Self> {
        self.child(name, true, true)
    }

    pub(crate) fn open_flat_batch_directory(&self, name: &OsStr) -> io::Result<Self> {
        let mut directory = self.child_reader(name)?;
        let current = directory.links.pop().ok_or_else(changed)?;
        let mut link = Arc::try_unwrap(current).map_err(|_| changed())?;
        link.sealed = Some(Arc::new(DirectorySeal::admit(&link.file)?));
        directory.links.push(Arc::new(link));
        directory.revalidate()?;
        Ok(directory)
    }

    pub(crate) fn open_flat_batch_file(
        &self,
        name: &OsStr,
        kind: FlatBatchKind,
    ) -> io::Result<RetainedFile> {
        match kind {
            FlatBatchKind::Executable => self.open_retained_executable(name),
            FlatBatchKind::ReadOnly => self.open_retained_read_only(name),
        }
    }
}

pub fn seal_flat_batch<const N: usize>(
    mut directory: Directory,
    creation: FlatBatchCreation,
    files: Vec<RetainedFile>,
    entries: &[FlatBatchEntry; N],
    observe: &mut impl FnMut(FlatBatchSealStep) -> io::Result<()>,
) -> io::Result<(Directory, Vec<RetainedFile>)> {
    let mut originals = Vec::with_capacity(N);
    for (index, (file, entry)) in files.into_iter().zip(entries).enumerate() {
        creation.revalidate(&directory)?;
        let sealed = match entry.kind() {
            FlatBatchKind::Executable => file.seal_executable()?,
            FlatBatchKind::ReadOnly => file.seal_read_only()?,
        };
        sealed.revalidate()?;
        observe(FlatBatchSealStep::FileSealed(index))?;
        creation.revalidate(&directory)?;
        sealed.revalidate()?;
        // The batch alone retains these raw creators. Dropping each internal ancestry
        // copy makes the parent transition exclusive without weakening generic guards.
        let RetainedFile {
            file, name, before, ..
        } = sealed;
        originals.push((file, name, before));
    }
    creation.revalidate(&directory)?;
    let current = directory.links.pop().ok_or_else(changed)?;
    let mut link =
        Arc::try_unwrap(current).map_err(|_| denied("batch parent has an escaped owner"))?;
    link.file
        .set_permissions(fs::Permissions::from_mode(0o500))?;
    link.file.sync_all()?;
    let sealed_metadata = link.file.metadata()?;
    if sealed_metadata.uid() != creation.owner || sealed_metadata.gid() != creation.group {
        return Err(changed());
    }
    link.private = false;
    link.sealed = Some(Arc::new(DirectorySeal::admit(&link.file)?));
    directory.links.push(Arc::new(link));
    observe(FlatBatchSealStep::DirectorySealed)?;
    directory.revalidate()?;
    observe(FlatBatchSealStep::ParentTransferred)?;
    directory.revalidate()?;

    let mut readers = Vec::with_capacity(N);
    for (index, ((creator, name, before), entry)) in originals.into_iter().zip(entries).enumerate()
    {
        // A new readonly open is necessary even on Unix: retaining the writable
        // creator file description would make native execution fail with ETXTBSY.
        let file = File::from(rustix::fs::openat(
            &directory.current().file,
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if !unchanged(&before, &file.metadata()?) || !unchanged(&before, &creator.metadata()?) {
            return Err(changed());
        }
        match entry.kind() {
            FlatBatchKind::Executable => private_files::validate_executable(&file)?,
            FlatBatchKind::ReadOnly => private_files::validate_read_only(&file)?,
        }
        observe(FlatBatchSealStep::ChildBridged(index))?;
        if !unchanged(&before, &file.metadata()?) || !unchanged(&before, &creator.metadata()?) {
            return Err(changed());
        }
        directory.revalidate()?;
        drop(creator);
        let reader = RetainedFile {
            directory: directory.clone(),
            name,
            file,
            before,
            links: LinkPolicy::Single,
            private: entry.kind() == FlatBatchKind::ReadOnly,
            writable: false,
            read_only: entry.kind() == FlatBatchKind::ReadOnly,
            executable: entry.kind() == FlatBatchKind::Executable,
            publication: PublicationAuthority::None,
            ancestry: AncestryCheck::EachCall,
        };
        observe(FlatBatchSealStep::ChildTransferred(index))?;
        reader.revalidate()?;
        readers.push(reader);
    }
    directory.revalidate()?;
    Ok((directory, readers))
}
