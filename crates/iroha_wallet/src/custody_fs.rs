//! Bounded no-follow filesystem operations for immutable private wallets.

use eyre::{Result, WrapErr, bail, eyre};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use zeroize::Zeroizing;

/// An owner-private directory retained across every child operation.
pub(crate) struct PrivateDirectory {
    pub(crate) path: PathBuf,
    #[cfg(unix)]
    descriptor: File,
}

/// Resolve the existing platform prefix, retaining only normal missing path components.
pub(crate) fn resolved_target(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("wallet store must be an absolute path without parent traversal");
    }
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    bail!("wallet store ancestor must be a real directory");
                }
                let mut resolved = ancestor.canonicalize()?;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| eyre!("invalid wallet store"))?,
                );
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| eyre!("invalid wallet store"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

impl PrivateDirectory {
    #[cfg(unix)]
    pub(crate) fn ensure_child(&self, name: &str) -> Result<Self> {
        single_name(name)?;
        self.revalidate()?;
        match rustix::fs::mkdirat(
            &self.descriptor,
            name,
            rustix::fs::Mode::from_raw_mode(0o700),
        ) {
            Ok(()) => self.descriptor.sync_all()?,
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
        self.child(name, false)
    }

    #[cfg(not(unix))]
    pub(crate) fn ensure_child(&self, _: &str) -> Result<Self> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(unix)]
    pub(crate) fn open_or_create(path: &Path) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        let mut ancestor = path;
        let mut suffix = Vec::new();
        while !ancestor.try_exists()? {
            suffix.push(
                ancestor
                    .file_name()
                    .ok_or_else(|| eyre!("invalid private directory"))?,
            );
            ancestor = ancestor
                .parent()
                .ok_or_else(|| eyre!("invalid private directory"))?;
        }
        let mut directory = File::from(rustix::fs::open(
            ancestor,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        for component in suffix.iter().rev() {
            match rustix::fs::mkdirat(&directory, *component, Mode::from_raw_mode(0o700)) {
                Ok(()) => directory.sync_all()?,
                Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            directory = File::from(rustix::fs::openat(
                &directory,
                *component,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )?);
            validate_metadata(&directory.metadata()?, true, true)?;
        }
        validate_metadata(&directory.metadata()?, true, true)?;
        let result = Self {
            path: path.to_owned(),
            descriptor: directory,
        };
        result.revalidate()?;
        Ok(result)
    }

    #[cfg(not(unix))]
    pub(crate) fn open_or_create(_: &Path) -> Result<Self> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(unix)]
    pub(crate) fn child(&self, name: &str, create: bool) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        self.revalidate()?;
        if create {
            rustix::fs::mkdirat(&self.descriptor, name, Mode::from_raw_mode(0o700))
                .wrap_err("wallet directory already exists or cannot be created")?;
            self.descriptor.sync_all()?;
        }
        let descriptor = File::from(rustix::fs::openat(
            &self.descriptor,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        validate_metadata(&descriptor.metadata()?, true, true)?;
        self.revalidate()?;
        Ok(Self {
            path: self.path.join(name),
            descriptor,
        })
    }

    #[cfg(not(unix))]
    pub(crate) fn child(&self, _: &str, _: bool) -> Result<Self> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(unix)]
    pub(crate) fn revalidate(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::symlink_metadata(&self.path)?;
        let retained = self.descriptor.metadata()?;
        validate_metadata(&actual, true, true)?;
        if actual.dev() != retained.dev() || actual.ino() != retained.ino() {
            bail!("wallet directory moved or was replaced during the operation");
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub(crate) fn revalidate(&self) -> Result<()> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(unix)]
    pub(crate) fn read(&self, name: &str, maximum: usize) -> Result<Zeroizing<Vec<u8>>> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        self.revalidate()?;
        let file = File::from(rustix::fs::openat(
            &self.descriptor,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let bytes = read_checked(file, maximum, true)?;
        self.revalidate()?;
        Ok(bytes)
    }

    #[cfg(not(unix))]
    pub(crate) fn read(&self, _: &str, _: usize) -> Result<Zeroizing<Vec<u8>>> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(unix)]
    pub(crate) fn write_new(&self, name: &str, bytes: &[u8]) -> Result<()> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        self.revalidate()?;
        let mut file = File::from(rustix::fs::openat(
            &self.descriptor,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )?);
        file.write_all(bytes)?;
        file.sync_all()?;
        validate_metadata(&file.metadata()?, false, true)?;
        self.descriptor.sync_all()?;
        self.revalidate()
    }

    #[cfg(not(unix))]
    pub(crate) fn write_new(&self, _: &str, _: &[u8]) -> Result<()> {
        bail!("private wallet custody requires native Unix descriptor support")
    }

    #[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
    pub(crate) fn publish(&self, temporary: &Self, name: &str) -> Result<()> {
        single_name(name)?;
        if temporary.path.parent() != Some(self.path.as_path()) {
            bail!("pending wallet is outside the retained collection");
        }
        self.revalidate()?;
        temporary.revalidate()?;
        rustix::fs::renameat_with(
            &self.descriptor,
            temporary
                .path
                .file_name()
                .ok_or_else(|| eyre!("invalid pending wallet"))?,
            &self.descriptor,
            name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .wrap_err("wallet name already exists or atomic publication failed")?;
        self.descriptor.sync_all()?;
        self.revalidate()
    }

    #[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
    pub(crate) fn publish(&self, _: &Self, _: &str) -> Result<()> {
        bail!("wallet creation requires atomic no-replace directory publication")
    }

    #[cfg(unix)]
    pub(crate) fn remove_pending(&self, temporary: &Self) -> Result<()> {
        use rustix::fs::AtFlags;
        if temporary.path.parent() != Some(self.path.as_path())
            || !temporary
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".pending-"))
        {
            bail!("cleanup requires this store's unpublished wallet directory");
        }
        self.revalidate()?;
        temporary.revalidate()?;
        for name in ["private.key", "client.toml", "wallet.json"] {
            match rustix::fs::unlinkat(&temporary.descriptor, name, AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(error.into()),
            }
        }
        rustix::fs::unlinkat(
            &self.descriptor,
            temporary
                .path
                .file_name()
                .ok_or_else(|| eyre!("invalid pending wallet"))?,
            AtFlags::REMOVEDIR,
        )?;
        self.descriptor.sync_all()?;
        Ok(())
    }

    #[cfg(not(unix))]
    pub(crate) fn remove_pending(&self, _: &Self) -> Result<()> {
        bail!("private wallet custody requires native Unix descriptor support")
    }
}

fn single_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("private wallet file names must be single normal path components");
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn read_external(
    path: &Path,
    maximum: usize,
    private: bool,
) -> Result<Zeroizing<Vec<u8>>> {
    use rustix::fs::{Mode, OFlags};
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("input file has no parent"))?
        .canonicalize()?;
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("input must name one regular file"))?;
    let directory = File::from(rustix::fs::open(
        &parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    let file = File::from(rustix::fs::openat(
        &directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    read_checked(file, maximum, private)
}

#[cfg(not(unix))]
pub(crate) fn read_external(_: &Path, _: usize, _: bool) -> Result<Zeroizing<Vec<u8>>> {
    bail!("private wallet custody requires native Unix descriptor support")
}

#[cfg(unix)]
fn read_checked(mut file: File, maximum: usize, private: bool) -> Result<Zeroizing<Vec<u8>>> {
    use std::os::unix::fs::MetadataExt;
    let before = file.metadata()?;
    validate_metadata(&before, false, private)?;
    if before.len() > maximum as u64 {
        bail!("wallet input exceeds its size bound");
    }
    let mut bytes = Zeroizing::new(Vec::new());
    std::io::Read::by_ref(&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    validate_metadata(&after, false, private)?;
    if bytes.len() > maximum
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        bail!("wallet input changed or exceeded its size bound while being read");
    }
    Ok(bytes)
}

#[cfg(unix)]
fn validate_metadata(metadata: &fs::Metadata, directory: bool, private: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let mode = metadata.mode() & 0o7777;
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || if directory {
            !metadata.is_dir() || mode != 0o700
        } else {
            !metadata.is_file()
                || metadata.nlink() != 1
                || if private {
                    !matches!(mode, 0o400 | 0o600)
                } else {
                    mode & 0o022 != 0
                }
        }
    {
        bail!(
            "wallet storage must be current-owner private directories (0700) and single-link regular files (0400 or 0600)"
        );
    }
    Ok(())
}
