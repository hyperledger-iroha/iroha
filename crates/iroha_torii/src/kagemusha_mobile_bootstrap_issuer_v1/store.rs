//! Exclusively owned, bounded server checkpoint storage with explicit creation and recovery.

use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read as _, Write as _},
    path::Path,
    sync::{Mutex, OnceLock},
};

use iroha_data_model::kagemusha::KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1;

use super::Result;

pub(super) const HEAD: &str = "checkpoint.norito";
pub(super) const TEMP: &str = ".checkpoint.norito.tmp";
const LOCK: &str = ".writer.lock";

fn failure() -> String {
    "bootstrap authority durable state is unavailable".to_owned()
}

struct ProcessDirectories {
    process_id: u32,
    identities: Mutex<BTreeSet<(u64, u64)>>,
}
static OPEN_DIRECTORIES: OnceLock<ProcessDirectories> = OnceLock::new();

struct DirectoryLease {
    identity: (u64, u64),
    process_id: u32,
}
impl DirectoryLease {
    fn acquire(directory: &File) -> Result<Self> {
        let process_id = std::process::id();
        let registry = OPEN_DIRECTORIES.get_or_init(|| ProcessDirectories {
            process_id,
            identities: Mutex::new(BTreeSet::new()),
        });
        // Never acquire a mutex inherited from a different process.
        if registry.process_id != process_id {
            return Err(failure());
        }
        let identity = directory_identity(directory)?;
        if !registry
            .identities
            .lock()
            .map_err(|_| failure())?
            .insert(identity)
        {
            return Err("bootstrap authority store already has a live owner".to_owned());
        }
        Ok(Self {
            identity,
            process_id,
        })
    }
}
impl Drop for DirectoryLease {
    fn drop(&mut self) {
        if self.process_id != std::process::id() {
            return;
        }
        if let Some(registry) = OPEN_DIRECTORIES.get()
            && registry.process_id == self.process_id
            && let Ok(mut identities) = registry.identities.lock()
        {
            identities.remove(&self.identity);
        }
    }
}

pub(super) struct HighWaterStore {
    process_id: u32,
    directory: File,
    _writer_lock: File,
    _lease: DirectoryLease,
    poisoned: bool,
    #[cfg(test)]
    pub(super) fail_next_write_stage: u8,
}

impl HighWaterStore {
    pub(super) fn create(path: &Path, initial: &[u8]) -> Result<Self> {
        validate_size(initial)?;
        let directory = open_directory(path, true)?;
        let lease = DirectoryLease::acquire(&directory)?;
        let writer_lock = open_writer_lock(&directory, true)?;
        let mut store = Self {
            process_id: std::process::id(),
            directory,
            _writer_lock: writer_lock,
            _lease: lease,
            poisoned: false,
            #[cfg(test)]
            fail_next_write_stage: 0,
        };
        store.replace(initial)?;
        Ok(store)
    }

    pub(super) fn recover(path: &Path) -> Result<(Self, Vec<u8>)> {
        let directory = open_directory(path, false)?;
        let lease = DirectoryLease::acquire(&directory)?;
        let writer_lock = open_writer_lock(&directory, false)?;
        validate_namespace(&directory)?;
        let archive = read_record(&directory, HEAD)?.ok_or_else(failure)?;
        Ok((
            Self {
                process_id: std::process::id(),
                directory,
                _writer_lock: writer_lock,
                _lease: lease,
                poisoned: false,
                #[cfg(test)]
                fail_next_write_stage: 0,
            },
            archive,
        ))
    }

    fn require_live(&self) -> Result<()> {
        if self.process_id != std::process::id() || self.poisoned {
            return Err(failure());
        }
        Ok(())
    }

    pub(super) fn require_current(&mut self, expected: &[u8]) -> Result<()> {
        self.require_live()?;
        match read_record(&self.directory, HEAD) {
            Ok(Some(actual)) if actual == expected => Ok(()),
            _ => {
                self.poisoned = true;
                Err(failure())
            }
        }
    }

    pub(super) fn finish_recovery(&mut self) -> Result<()> {
        self.require_live()?;
        if read_record(&self.directory, TEMP)?.is_some() {
            // All errors after a possible durable mutation leave this handle poisoned.
            self.poisoned = true;
            remove_temporary(&self.directory)?;
            self.poisoned = false;
        }
        Ok(())
    }

    pub(super) fn replace(&mut self, next: &[u8]) -> Result<()> {
        self.require_live()?;
        validate_size(next)?;
        self.poisoned = true;
        #[cfg(test)]
        let injection = std::mem::take(&mut self.fail_next_write_stage);
        #[cfg(not(test))]
        let injection = 0;
        persist_head(&self.directory, next, injection)?;
        // Only exact, synced, rebound readback clears uncertainty. Failure requires recovery.
        self.poisoned = false;
        Ok(())
    }
}

fn validate_size(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 {
        return Err(failure());
    }
    Ok(())
}

#[cfg(unix)]
fn directory_identity(directory: &File) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = directory.metadata().map_err(|_| failure())?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
fn open_directory(path: &Path, create: bool) -> Result<File> {
    use rustix::fs::{Mode, OFlags};
    use std::{os::unix::fs::MetadataExt as _, path::Component};

    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(failure());
    }
    let components = components
        .map(|part| match part {
            Component::Normal(name) => Ok(name),
            _ => Err(failure()),
        })
        .collect::<Result<Vec<_>>>()?;
    if components.is_empty() {
        return Err(failure());
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut parent =
        File::from(rustix::fs::open("/", flags, Mode::empty()).map_err(|_| failure())?);
    for (index, name) in components.iter().enumerate() {
        let final_component = index + 1 == components.len();
        if final_component && create {
            // Creation never adopts an existing empty directory or a missing recovery head.
            rustix::fs::mkdirat(&parent, *name, Mode::RWXU).map_err(|_| failure())?;
        }
        let before = rustix::fs::statat(&parent, *name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| failure())?;
        let child = File::from(
            rustix::fs::openat(&parent, *name, flags, Mode::empty()).map_err(|_| failure())?,
        );
        if final_component && create {
            rustix::fs::fchmod(&child, Mode::RWXU).map_err(|_| failure())?;
            child.sync_all().map_err(|_| failure())?;
            parent.sync_all().map_err(|_| failure())?;
        }
        let opened = child.metadata().map_err(|_| failure())?;
        let after = rustix::fs::statat(&parent, *name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| failure())?;
        if !opened.is_dir()
            || u64::try_from(before.st_dev).ok() != Some(opened.dev())
            || u64::try_from(before.st_ino).ok() != Some(opened.ino())
            || u64::try_from(after.st_dev).ok() != Some(opened.dev())
            || u64::try_from(after.st_ino).ok() != Some(opened.ino())
            || (final_component
                && (opened.uid() != rustix::process::geteuid().as_raw()
                    || opened.mode() & 0o777 != 0o700))
        {
            return Err(failure());
        }
        parent = child;
    }
    Ok(parent)
}

#[cfg(unix)]
fn open_writer_lock(directory: &File, create: bool) -> Result<File> {
    use rustix::fs::{Mode, OFlags};
    let mut flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    if create {
        flags |= OFlags::CREATE | OFlags::EXCL;
    }
    let file = File::from(
        rustix::fs::openat(directory, LOCK, flags, Mode::RUSR | Mode::WUSR)
            .map_err(|_| failure())?,
    );
    if create {
        rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).map_err(|_| failure())?;
        file.sync_all().map_err(|_| failure())?;
        directory.sync_all().map_err(|_| failure())?;
    }
    validate_file_identity(directory, LOCK, &file, 0)?;
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| failure())?;
    validate_file_identity(directory, LOCK, &file, 0)?;
    Ok(file)
}

#[cfg(unix)]
fn validate_namespace(directory: &File) -> Result<()> {
    for entry in rustix::fs::Dir::read_from(directory).map_err(|_| failure())? {
        let entry = entry.map_err(|_| failure())?;
        match entry.file_name().to_bytes() {
            b"." | b".." | b"checkpoint.norito" | b".writer.lock" | b".checkpoint.norito.tmp" => {}
            _ => return Err(failure()),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_file_identity(directory: &File, name: &str, file: &File, size: u64) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let opened = file.metadata().map_err(|_| failure())?;
    let named = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| failure())?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o777 != 0o600
        || opened.nlink() != 1
        || opened.len() != size
        || rustix::fs::FileType::from_raw_mode(named.st_mode) != rustix::fs::FileType::RegularFile
        || named.st_uid != opened.uid()
        || named.st_mode & 0o777 != 0o600
        || named.st_nlink != 1
        || u64::try_from(named.st_dev).ok() != Some(opened.dev())
        || u64::try_from(named.st_ino).ok() != Some(opened.ino())
        || u64::try_from(named.st_size).ok() != Some(size)
    {
        return Err(failure());
    }
    Ok(())
}

#[cfg(unix)]
fn read_record(directory: &File, name: &str) -> Result<Option<Vec<u8>>> {
    use rustix::fs::{Mode, OFlags};
    use std::os::unix::fs::MetadataExt as _;
    let before = match rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(value) => value,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(failure()),
    };
    if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::RegularFile
        || before.st_nlink != 1
        || u64::try_from(before.st_size)
            .ok()
            .is_none_or(|size| size > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 as u64)
    {
        return Err(failure());
    }
    let mut file = File::from(
        rustix::fs::openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| failure())?,
    );
    let metadata = file.metadata().map_err(|_| failure())?;
    validate_file_identity(directory, name, &file, metadata.len())?;
    if u64::try_from(before.st_dev).ok() != Some(metadata.dev())
        || u64::try_from(before.st_ino).ok() != Some(metadata.ino())
        || u64::try_from(before.st_size).ok() != Some(metadata.len())
    {
        return Err(failure());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(usize::try_from(metadata.len()).map_err(|_| failure())?)
        .map_err(|_| failure())?;
    (&mut file)
        .take(KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure())?;
    validate_file_identity(directory, name, &file, metadata.len())?;
    let after = file.metadata().map_err(|_| failure())?;
    if bytes.len() as u64 != metadata.len()
        || after.mtime() != metadata.mtime()
        || after.mtime_nsec() != metadata.mtime_nsec()
        || after.ctime() != metadata.ctime()
        || after.ctime_nsec() != metadata.ctime_nsec()
    {
        return Err(failure());
    }
    Ok(Some(bytes))
}

#[cfg(unix)]
fn persist_head(directory: &File, bytes: &[u8], injection: u8) -> Result<()> {
    use rustix::fs::{Mode, OFlags};
    let mut file = File::from(
        rustix::fs::openat(
            directory,
            TEMP,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| failure())?,
    );
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).map_err(|_| failure())?;
    file.write_all(bytes).map_err(|_| failure())?;
    file.sync_all().map_err(|_| failure())?;
    validate_file_identity(directory, TEMP, &file, bytes.len() as u64)?;
    if injection == 1 {
        return Err(failure());
    }
    rustix::fs::renameat(directory, TEMP, directory, HEAD).map_err(|_| failure())?;
    if injection == 2 {
        return Err(failure());
    }
    validate_file_identity(directory, HEAD, &file, bytes.len() as u64)?;
    directory.sync_all().map_err(|_| failure())?;
    if injection == 3 {
        return Err(failure());
    }
    if read_record(directory, HEAD)?.as_deref() != Some(bytes) {
        return Err(failure());
    }
    validate_file_identity(directory, HEAD, &file, bytes.len() as u64)
}

#[cfg(unix)]
fn remove_temporary(directory: &File) -> Result<()> {
    rustix::fs::unlinkat(directory, TEMP, rustix::fs::AtFlags::empty()).map_err(|_| failure())?;
    directory.sync_all().map_err(|_| failure())
}

#[cfg(not(unix))]
fn directory_identity(_: &File) -> Result<(u64, u64)> {
    Err(failure())
}
#[cfg(not(unix))]
fn open_directory(_: &Path, _: bool) -> Result<File> {
    Err(failure())
}
#[cfg(not(unix))]
fn open_writer_lock(_: &File, _: bool) -> Result<File> {
    Err(failure())
}
#[cfg(not(unix))]
fn validate_namespace(_: &File) -> Result<()> {
    Err(failure())
}
#[cfg(not(unix))]
fn read_record(_: &File, _: &str) -> Result<Option<Vec<u8>>> {
    Err(failure())
}
#[cfg(not(unix))]
fn persist_head(_: &File, _: &[u8], _: u8) -> Result<()> {
    Err(failure())
}
#[cfg(not(unix))]
fn remove_temporary(_: &File) -> Result<()> {
    Err(failure())
}
