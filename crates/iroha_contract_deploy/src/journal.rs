//! Owner-only, locked, descriptor-relative immutable deployment evidence.
use eyre::{Result, WrapErr as _, eyre};
use norito::json::{JsonDeserialize, JsonSerialize};
use std::{
    fs::File,
    io::{Read as _, Write as _},
    path::Path,
};

const MAX_RECORD_BYTES: u64 = 128 * 1024 * 1024;

pub(super) struct Journal {
    directory: File,
    _lock: File,
}

#[cfg(unix)]
impl Journal {
    pub(super) fn open(path: &Path, create: bool) -> Result<Self> {
        use rustix::fs::{FlockOperation, Mode, OFlags};
        use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
        if create {
            match std::fs::DirBuilder::new().mode(0o700).create(path) {
                Ok(()) => {
                    File::open(
                        path.parent()
                            .filter(|parent| !parent.as_os_str().is_empty())
                            .unwrap_or(Path::new(".")),
                    )?
                    .sync_all()?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let directory = File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let metadata = directory.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o777 != 0o700
        {
            return Err(eyre!(
                "deployment journal must be an owner-only directory with mode 0700"
            ));
        }
        let lock = File::from(rustix::fs::openat(
            &directory,
            "lock",
            OFlags::RDWR
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC
                | if create {
                    OFlags::CREATE
                } else {
                    OFlags::empty()
                },
            Mode::RUSR | Mode::WUSR,
        )?);
        validate_file(&lock)?;
        rustix::fs::flock(&lock, FlockOperation::NonBlockingLockExclusive)
            .wrap_err("deployment journal is already in use")?;
        lock.sync_all()?;
        directory.sync_all()?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }

    pub(super) fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.open_record(name)?.is_some())
    }

    pub(super) fn require_unattempted(&self) -> Result<()> {
        for entry in rustix::fs::Dir::read_from(&self.directory)? {
            let entry = entry?;
            if !matches!(
                entry.file_name().to_bytes(),
                b"." | b".." | b"lock" | b"plan.json" | b"cancelled.json"
            ) {
                return Err(eyre!(
                    "only a fully unattempted deployment may be cancelled; retained execution or unknown evidence blocks cancellation"
                ));
            }
        }
        Ok(())
    }

    fn open_record(&self, name: &str) -> Result<Option<File>> {
        use rustix::fs::{Mode, OFlags};
        validate_name(name)?;
        match rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => {
                let file = File::from(fd);
                validate_file(&file)?;
                Ok(Some(file))
            }
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn read_bytes(&self, name: &str) -> Result<Vec<u8>> {
        use std::os::unix::fs::MetadataExt as _;
        let file = self
            .open_record(name)?
            .ok_or_else(|| eyre!("deployment journal has no {name}"))?;
        let before = file.metadata()?;
        let mut bytes = Vec::new();
        (&file).take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
        let after = file.metadata()?;
        validate_file(&file)?;
        if bytes.len() as u64 != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(eyre!(
                "deployment journal record changed during its bounded read"
            ));
        }
        Ok(bytes)
    }

    pub(super) fn read<T: JsonDeserialize>(&self, name: &str) -> Result<T> {
        norito::json::from_slice(&self.read_bytes(name)?)
            .wrap_err("decode closed deployment journal record")
    }

    pub(super) fn put_exact<T: JsonSerialize>(&self, name: &str, record: &T) -> Result<()> {
        use rustix::fs::{Mode, OFlags};
        validate_name(name)?;
        let bytes = norito::json::to_vec(record)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(eyre!("deployment journal record exceeds fixed byte bound"));
        }
        if self.exists(name)? {
            if self.read_bytes(name)? != bytes {
                return Err(eyre!(
                    "immutable deployment journal record {name} disagrees with this operation"
                ));
            }
            return Ok(());
        }
        // Create-exclusive retains an incomplete record after a crash rather than permitting an
        // ambiguous replacement. Dispatch only occurs after both file and directory fsync succeed.
        let mut file = File::from(rustix::fs::openat(
            &self.directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        validate_file(&file)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        self.directory.sync_all()?;
        Ok(())
    }
}

#[cfg(not(unix))]
impl Journal {
    pub(super) fn open(_path: &Path, _create: bool) -> Result<Self> {
        Err(eyre!(
            "durable deployment journals require a qualified Unix filesystem"
        ))
    }
    pub(super) fn exists(&self, _name: &str) -> Result<bool> {
        Err(eyre!("unsupported deployment journal filesystem"))
    }
    pub(super) fn require_unattempted(&self) -> Result<()> {
        Err(eyre!("unsupported deployment journal filesystem"))
    }
    pub(super) fn read<T: JsonDeserialize>(&self, _name: &str) -> Result<T> {
        Err(eyre!("unsupported deployment journal filesystem"))
    }
    pub(super) fn put_exact<T: JsonSerialize>(&self, _name: &str, _record: &T) -> Result<()> {
        Err(eyre!("unsupported deployment journal filesystem"))
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 80
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
        || name == "."
        || name == ".."
    {
        return Err(eyre!("invalid fixed deployment journal record name"));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_file(file: &File) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() > MAX_RECORD_BYTES
    {
        return Err(eyre!(
            "deployment journal files must be bounded, regular, single-link, owner-only files with mode 0600"
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "journal_tests.rs"]
mod tests;
