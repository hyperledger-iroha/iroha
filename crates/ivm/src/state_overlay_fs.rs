//! Handle-rooted filesystem operations for the development state overlay.

use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::File,
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("handle-rooted durable state overlays are not implemented for this Unix target");

/// One overlay leaf rooted at a retained ancestor directory capability.
#[derive(Clone, Debug)]
pub(super) struct RetainedOverlayTarget {
    location: Arc<Mutex<RetainedLocation>>,
    publication: Arc<Mutex<()>>,
    leaf: OsString,
    poisoned: Arc<AtomicBool>,
    #[cfg(test)]
    fail_next_publication_sync: Arc<AtomicBool>,
}

#[derive(Debug)]
struct RetainedLocation {
    directory: File,
    remaining: VecDeque<OsString>,
}

impl RetainedOverlayTarget {
    pub(super) fn from_path(path: &Path) -> io::Result<Self> {
        let leaf = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "durable state overlay path must name a file",
            )
        })?;
        platform::validate_component(leaf)?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let (directory, remaining) = platform::retain_parent(parent_path)?;
        let metadata = directory.metadata()?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay retained ancestor must be a direct directory",
            ));
        }
        if remaining.is_empty() {
            platform::validate_private_parent(&directory)?;
        }
        Ok(Self {
            location: Arc::new(Mutex::new(RetainedLocation {
                directory,
                remaining: remaining.into(),
            })),
            publication: Arc::new(Mutex::new(())),
            leaf: leaf.to_os_string(),
            poisoned: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            fail_next_publication_sync: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(super) fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire)
    }

    pub(super) fn poison(&self) {
        self.poisoned.store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn fail_next_publication_sync(&self) {
        self.fail_next_publication_sync
            .store(true, Ordering::Release);
    }

    fn resolve_parent(&self, create: bool) -> io::Result<Option<File>> {
        let mut location = self.location.lock().map_err(|_| {
            io::Error::other("durable state overlay retained parent lock is poisoned")
        })?;
        while let Some(name) = location.remaining.front().cloned() {
            platform::validate_lookup_parent(&location.directory)?;
            let (next, created) = match platform::open_directory(&location.directory, &name)? {
                Some(directory) => (directory, false),
                None if !create => return Ok(None),
                None => match platform::create_directory(&location.directory, &name) {
                    Ok(directory) => (directory, true),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        let directory = platform::open_directory(&location.directory, &name)?
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::NotFound,
                                    "durable state overlay ancestor disappeared while it was created",
                                )
                            })?;
                        (directory, false)
                    }
                    Err(error) => return Err(error),
                },
            };
            if !next.metadata()?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable state overlay ancestor must be a direct directory",
                ));
            }
            platform::validate_private_parent(&next)?;
            platform::sync_parent_entry(&location.directory, &next, created)?;
            location.directory = next;
            location.remaining.pop_front();
        }
        platform::validate_private_parent(&location.directory)?;
        location.directory.try_clone().map(Some)
    }

    fn temporary_name() -> OsString {
        let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
        OsString::from(format!(".ivm-state-tmp-{}-{nonce}", std::process::id()))
    }
}

#[derive(Debug)]
pub(super) enum AtomicWriteError {
    BeforePublication(io::Error),
    AfterPublication(io::Error),
    Poisoned(io::Error),
}

impl From<io::Error> for AtomicWriteError {
    fn from(error: io::Error) -> Self {
        Self::BeforePublication(error)
    }
}

/// Atomically publish one bounded overlay image below its retained parent.
pub(super) fn atomic_write(
    target: &RetainedOverlayTarget,
    bytes: &[u8],
) -> Result<(), AtomicWriteError> {
    let _publication_guard = target.publication.lock().map_err(|_| {
        target.poison();
        AtomicWriteError::Poisoned(io::Error::other(
            "durable state overlay publication lock is poisoned",
        ))
    })?;
    if target.is_poisoned() {
        return Err(AtomicWriteError::Poisoned(io::Error::other(
            "durable state overlay persistence target is poisoned",
        )));
    }
    let parent = target.resolve_parent(true)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "durable state overlay parent could not be created",
        )
    })?;
    let (temporary_name, mut temporary_file) = (0..128)
        .find_map(|_| {
            let name = RetainedOverlayTarget::temporary_name();
            match platform::create_file(&parent, &name) {
                Ok(file) => Some(Ok((name, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not reserve a durable state overlay temporary file",
            )
        })?;
    let initial = match regular_file_snapshot(&temporary_file) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            platform::remove_new_file(&parent, &temporary_file, &temporary_name);
            return Err(error.into());
        }
    };
    if !initial.is_single_link() || initial.len() != 0 {
        platform::remove_open_file(&parent, &temporary_file, &temporary_name, initial);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay temporary is not a new single-link regular file",
        )
        .into());
    }
    let mut published = false;
    let result: io::Result<()> = (|| {
        temporary_file.write_all(bytes)?;
        temporary_file.sync_all()?;
        let written = regular_file_snapshot(&temporary_file)?;
        let expected_len = u64::try_from(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay length cannot be represented",
            )
        })?;
        if !written.is_single_link() || written.len() != expected_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay temporary write was incomplete",
            ));
        }
        platform::verify_temporary_binding(&parent, &temporary_name, &temporary_file, written)?;
        validate_destination(&parent, target)?;
        platform::replace_open_file(&parent, &temporary_file, &temporary_name, &target.leaf)?;
        published = true;
        #[cfg(test)]
        if target
            .fail_next_publication_sync
            .swap(false, Ordering::AcqRel)
        {
            return Err(io::Error::other(
                "injected durable state overlay publication sync failure",
            ));
        }
        platform::sync_publication(&parent, &temporary_file)?;
        Ok(())
    })();
    if result.is_err() && !published {
        let snapshot = regular_file_snapshot(&temporary_file).ok();
        platform::remove_open_file(
            &parent,
            &temporary_file,
            &temporary_name,
            snapshot.unwrap_or(initial),
        );
    }
    result.map_err(|error| {
        if published {
            target.poison();
            AtomicWriteError::AfterPublication(error)
        } else {
            AtomicWriteError::BeforePublication(error)
        }
    })
}

fn validate_destination(parent: &File, target: &RetainedOverlayTarget) -> io::Result<()> {
    let Some(file) = platform::open_file(parent, &target.leaf)? else {
        return Ok(());
    };
    if !regular_file_snapshot(&file)?.is_single_link() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay destination must be a direct single-link regular file",
        ));
    }
    Ok(())
}

/// Read one overlay image through its retained parent capability.
pub(super) fn read_bounded(
    target: &RetainedOverlayTarget,
    maximum: usize,
) -> io::Result<Option<Vec<u8>>> {
    let Some(parent) = target.resolve_parent(false)? else {
        return Ok(None);
    };
    let Some(mut file) = platform::open_file(&parent, &target.leaf)? else {
        return Ok(None);
    };
    let before = regular_file_snapshot(&file)?;
    if !before.is_single_link() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay must be a direct single-link regular file",
        ));
    }
    let maximum_u64 = u64::try_from(maximum).expect("fixed state overlay limit fits u64");
    if before.len() > maximum_u64 {
        return Err(file_too_large());
    }
    let capacity = usize::try_from(before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay length cannot be addressed",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum_u64.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(file_too_large());
    }
    let after = regular_file_snapshot(&file)?;
    let Some(named_file) = platform::open_file(&parent, &target.leaf)? else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay binding disappeared while it was being read",
        ));
    };
    let named = regular_file_snapshot(&named_file)?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay byte count cannot be represented",
        )
    })?;
    if !after.is_single_link()
        || !named.is_single_link()
        || !before.same_identity(after)
        || !after.same_identity(named)
        || !before.same_revision(after)
        || !after.same_revision(named)
        || after.len() != observed
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay changed while it was being read",
        ));
    }
    Ok(Some(bytes))
}

fn file_too_large() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "durable state overlay exceeds the first-release file limit",
    )
}

#[cfg(all(test, windows))]
pub(super) fn windows_test_same_identity(first: &Path, second: &Path) -> io::Result<bool> {
    let first = File::open(first)?;
    let second = File::open(second)?;
    Ok(regular_file_snapshot(&first)?.same_identity(regular_file_snapshot(&second)?))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
#[derive(Clone, Copy)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
impl FileSnapshot {
    const fn is_single_link(self) -> bool {
        self.links == 1
    }

    const fn len(self) -> u64 {
        self.length
    }

    const fn same_identity(self, other: Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }

    const fn same_revision(self, other: Self) -> bool {
        self.same_identity(other)
            && self.length == other.length
            && self.modified_seconds == other.modified_seconds
            && self.modified_nanoseconds == other.modified_nanoseconds
            && self.changed_seconds == other.changed_seconds
            && self.changed_nanoseconds == other.changed_nanoseconds
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn regular_file_snapshot(file: &File) -> io::Result<FileSnapshot> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != platform::effective_uid()
        || metadata.mode() & 0o022 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay object must be a direct owner-controlled regular file",
        ));
    }
    Ok(FileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct FileSnapshot {
    attributes: u32,
    creation_time: u64,
    last_write_time: u64,
    volume_serial_number: u32,
    length: u64,
    file_index: u64,
    links: u32,
}

#[cfg(windows)]
impl FileSnapshot {
    const fn is_single_link(self) -> bool {
        self.attributes
            & (platform::FILE_ATTRIBUTE_DIRECTORY | platform::FILE_ATTRIBUTE_REPARSE_POINT)
            == 0
            && self.links == 1
    }

    const fn len(self) -> u64 {
        self.length
    }

    const fn same_identity(self, other: Self) -> bool {
        self.volume_serial_number == other.volume_serial_number
            && self.file_index == other.file_index
    }

    const fn same_revision(self, other: Self) -> bool {
        self.same_identity(other)
            && self.attributes == other.attributes
            && self.creation_time == other.creation_time
            && self.last_write_time == other.last_write_time
            && self.length == other.length
    }
}

#[cfg(windows)]
fn regular_file_snapshot(file: &File) -> io::Result<FileSnapshot> {
    platform::file_snapshot(file)
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
#[derive(Clone, Copy)]
struct FileSnapshot;

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
impl FileSnapshot {
    const fn is_single_link(self) -> bool {
        false
    }

    const fn len(self) -> u64 {
        0
    }

    const fn same_identity(self, _other: Self) -> bool {
        false
    }

    const fn same_revision(self, _other: Self) -> bool {
        false
    }
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
fn regular_file_snapshot(_file: &File) -> io::Result<FileSnapshot> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "handle-rooted durable state overlays require Unix or Windows",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
#[allow(unsafe_code)]
mod platform {
    use super::{FileSnapshot, PathBuf};
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    use std::fs;
    use std::{
        ffi::{CString, OsStr, OsString},
        fs::File,
        io,
        os::{
            fd::{AsRawFd as _, FromRawFd as _, RawFd},
            raw::{c_char, c_int, c_uint},
            unix::ffi::OsStrExt as _,
        },
        path::{Component, Path},
    };

    const O_READ_ONLY: c_int = 0;
    const O_WRITE_ONLY: c_int = 1;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_CREATE: c_int = 0x200;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_EXCLUSIVE: c_int = 0x800;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_NO_FOLLOW: c_int = 0x100;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_DIRECTORY: c_int = 0x0010_0000;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_CLOSE_ON_EXEC: c_int = 0x0100_0000;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    const O_NONBLOCK: c_int = 0x4;

    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    const O_CREATE: c_int = 0x200;
    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    const O_EXCLUSIVE: c_int = 0x800;
    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    const O_NO_FOLLOW: c_int = 0x100;
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    const O_DIRECTORY: c_int = 0x0002_0000;
    #[cfg(target_os = "netbsd")]
    const O_DIRECTORY: c_int = 0x0020_0000;
    #[cfg(target_os = "dragonfly")]
    const O_DIRECTORY: c_int = 0x0800_0000;
    #[cfg(target_os = "freebsd")]
    const O_CLOSE_ON_EXEC: c_int = 0x0010_0000;
    #[cfg(target_os = "netbsd")]
    const O_CLOSE_ON_EXEC: c_int = 0x0040_0000;
    #[cfg(target_os = "openbsd")]
    const O_CLOSE_ON_EXEC: c_int = 0x0001_0000;
    #[cfg(target_os = "dragonfly")]
    const O_CLOSE_ON_EXEC: c_int = 0x0002_0000;
    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    const O_NONBLOCK: c_int = 0x4;

    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    const O_DIRECTORY: c_int = 0x4000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    const O_NO_FOLLOW: c_int = 0x8000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    const O_DIRECTORY: c_int = 0x0001_0000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    const O_NO_FOLLOW: c_int = 0x0002_0000;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_CREATE: c_int = 0x40;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_EXCLUSIVE: c_int = 0x80;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        not(any(target_arch = "sparc", target_arch = "sparc64"))
    ))]
    const O_CLOSE_ON_EXEC: c_int = 0x0008_0000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "sparc", target_arch = "sparc64")
    ))]
    const O_CLOSE_ON_EXEC: c_int = 0x0040_0000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "mips", target_arch = "mips64")
    ))]
    const O_NONBLOCK: c_int = 0x80;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        any(target_arch = "sparc", target_arch = "sparc64")
    ))]
    const O_NONBLOCK: c_int = 0x4000;
    #[cfg(all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "mips",
            target_arch = "mips64",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    ))]
    const O_NONBLOCK: c_int = 0x800;

    unsafe extern "C" {
        fn geteuid() -> c_uint;
        fn openat(directory: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
        fn mkdirat(directory: c_int, path: *const c_char, mode: c_uint) -> c_int;
        fn renameat(
            source_directory: c_int,
            source: *const c_char,
            destination_directory: c_int,
            destination: *const c_char,
        ) -> c_int;
        fn unlinkat(directory: c_int, path: *const c_char, flags: c_int) -> c_int;
    }

    pub(super) fn validate_component(name: &OsStr) -> io::Result<()> {
        let bytes = name.as_bytes();
        if bytes.is_empty()
            || bytes == b"."
            || bytes == b".."
            || bytes.contains(&0)
            || bytes.contains(&b'/')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "durable state overlay path component is empty, relative, or contains NUL",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_private_parent(parent: &File) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = parent.metadata()?;
        if !metadata.is_dir() || metadata.uid() != effective_uid() || metadata.mode() & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "durable state overlay final parent must be owner-controlled and not group/world writable",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_lookup_parent(parent: &File) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = parent.metadata()?;
        let trusted_owner = matches!(metadata.uid(), 0) || metadata.uid() == effective_uid();
        let protected_namespace = metadata.mode() & 0o022 == 0 || metadata.mode() & 0o1000 != 0;
        if !metadata.is_dir() || !trusted_owner || !protected_namespace {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "durable state overlay lookup parent must have a trusted owner and be private or sticky",
            ));
        }
        Ok(())
    }

    pub(super) fn effective_uid() -> u32 {
        // SAFETY: `geteuid` takes no arguments and has no preconditions.
        unsafe { geteuid() }
    }

    fn c_name(name: &OsStr) -> io::Result<CString> {
        validate_component(name)?;
        CString::new(name.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "durable state overlay path component contains NUL",
            )
        })
    }

    fn file_from_fd(fd: RawFd) -> io::Result<File> {
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every successful call in this module returns a fresh owned fd.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn open_directory_exact(parent: &File, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: the retained parent and NUL-terminated component outlive the call.
        file_from_fd(unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_ONLY | O_DIRECTORY | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0,
            )
        })
    }

    pub(super) fn open_directory(parent: &File, name: &OsStr) -> io::Result<Option<File>> {
        match open_directory_exact(parent, name) {
            Ok(directory) => Ok(Some(directory)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn create_directory(parent: &File, name: &OsStr) -> io::Result<File> {
        let encoded_name = c_name(name)?;
        // SAFETY: the retained parent and component are valid for the call.
        if unsafe { mkdirat(parent.as_raw_fd(), encoded_name.as_ptr(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
        open_directory_exact(parent, name)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn normalize_root_owned_alias(path: &Path) -> io::Result<PathBuf> {
        use std::os::unix::fs::MetadataExt as _;
        if !path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let mut components = path.components();
        if !matches!(components.next(), Some(Component::RootDir)) {
            return Ok(path.to_path_buf());
        }
        let Some(Component::Normal(first)) = components.next() else {
            return Ok(path.to_path_buf());
        };
        if !matches!(first.to_str(), Some("var" | "tmp" | "etc")) {
            return Ok(path.to_path_buf());
        }
        let alias = Path::new("/").join(first);
        let alias_metadata = fs::symlink_metadata(&alias)?;
        if !alias_metadata.file_type().is_symlink() {
            return Ok(path.to_path_buf());
        }
        let root_metadata = fs::metadata("/")?;
        if alias_metadata.uid() != 0
            || root_metadata.uid() != 0
            || root_metadata.mode() & 0o022 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Apple durable state overlay root alias is not root-owned",
            ));
        }
        let mut normalized = fs::canonicalize(&alias)?;
        for component in components {
            normalized.push(component.as_os_str());
        }
        Ok(normalized)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    fn normalize_root_owned_alias(path: &Path) -> io::Result<PathBuf> {
        Ok(path.to_path_buf())
    }

    pub(super) fn retain_parent(path: &Path) -> io::Result<(File, Vec<OsString>)> {
        let path = normalize_root_owned_alias(path)?;
        let mut names = Vec::new();
        for component in path.components() {
            match component {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => {
                    validate_component(name)?;
                    names.push(name.to_os_string());
                }
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "durable state overlay parent must not contain relative components",
                    ));
                }
            }
        }
        let mut directory = if path.is_absolute() {
            File::open("/")?
        } else {
            File::open(".")?
        };
        for (index, name) in names.iter().enumerate() {
            validate_lookup_parent(&directory)?;
            directory = match open_directory(&directory, name)? {
                Some(child) => child,
                None => return Ok((directory, names[index..].to_vec())),
            };
            if !directory.metadata()?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable state overlay ancestor must be a direct directory",
                ));
            }
        }
        Ok((directory, Vec::new()))
    }

    pub(super) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: O_EXCL and O_NOFOLLOW bind a fresh direct child to this parent.
        file_from_fd(unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_WRITE_ONLY | O_CREATE | O_EXCLUSIVE | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0o600,
            )
        })
    }

    pub(super) fn open_file(parent: &File, name: &OsStr) -> io::Result<Option<File>> {
        let name = c_name(name)?;
        // SAFETY: the retained parent and component remain valid for the call.
        let result = file_from_fd(unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_ONLY | O_NO_FOLLOW | O_CLOSE_ON_EXEC | O_NONBLOCK,
                0,
            )
        });
        match result {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn verify_temporary_binding(
        parent: &File,
        name: &OsStr,
        file: &File,
        expected: FileSnapshot,
    ) -> io::Result<()> {
        let Some(named) = open_file(parent, name)? else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay temporary binding disappeared",
            ));
        };
        let actual = super::regular_file_snapshot(&named)?;
        let opened = super::regular_file_snapshot(file)?;
        if !expected.same_revision(opened)
            || !opened.same_identity(actual)
            || !actual.is_single_link()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay temporary binding changed",
            ));
        }
        Ok(())
    }

    pub(super) fn replace_open_file(
        parent: &File,
        _file: &File,
        temporary_name: &OsStr,
        destination_name: &OsStr,
    ) -> io::Result<()> {
        let temporary_name = c_name(temporary_name)?;
        let destination_name = c_name(destination_name)?;
        // SAFETY: both components are resolved by the same retained parent fd.
        if unsafe {
            renameat(
                parent.as_raw_fd(),
                temporary_name.as_ptr(),
                parent.as_raw_fd(),
                destination_name.as_ptr(),
            )
        } == 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn sync_publication(parent: &File, _file: &File) -> io::Result<()> {
        parent.sync_all()
    }

    pub(super) fn sync_parent_entry(
        parent: &File,
        _child: &File,
        _created: bool,
    ) -> io::Result<()> {
        // Sync every lazily traversed entry. This also retries the barrier if a
        // prior attempt created the directory but its parent sync failed.
        parent.sync_all()
    }

    pub(super) fn remove_open_file(
        parent: &File,
        file: &File,
        name: &OsStr,
        expected: FileSnapshot,
    ) {
        let Ok(Some(named)) = open_file(parent, name) else {
            return;
        };
        let Ok(actual) = super::regular_file_snapshot(&named) else {
            return;
        };
        let Ok(opened) = super::regular_file_snapshot(file) else {
            return;
        };
        if !expected.same_identity(opened) || !opened.same_identity(actual) {
            return;
        }
        let Ok(name) = c_name(name) else {
            return;
        };
        // SAFETY: the direct binding was matched to the retained temporary.
        let _ = unsafe { unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) };
    }

    pub(super) fn remove_new_file(parent: &File, _file: &File, name: &OsStr) {
        let Ok(name) = c_name(name) else {
            return;
        };
        // SAFETY: the name was just reserved with O_EXCL in an owner-controlled,
        // mode-bit-private parent and no untrusted operation intervened.
        let _ = unsafe { unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) };
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    use super::{FileSnapshot, PathBuf};
    use std::{
        ffi::{OsStr, OsString, c_void},
        fs::{File, OpenOptions},
        io,
        mem::{MaybeUninit, offset_of, size_of},
        os::windows::{
            ffi::OsStrExt as _,
            fs::OpenOptionsExt as _,
            io::{AsRawHandle as _, FromRawHandle as _, RawHandle},
        },
        path::{Component, Path},
        ptr,
    };

    type Handle = *mut c_void;
    type NtStatus = i32;
    pub(super) const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
    pub(super) const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
    const FILE_WRITE_THROUGH: u32 = 0x0000_0002;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
    const FILE_OPEN_FOR_BACKUP_INTENT: u32 = 0x0000_4000;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const OBJ_CASE_INSENSITIVE: u32 = 0x0000_0040;
    const FILE_RENAME_INFO_CLASS: u32 = 3;
    const FILE_DISPOSITION_INFO_CLASS: u32 = 4;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: Handle,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut c_void,
        security_quality_of_service: *mut c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        status_or_pointer: isize,
        information: usize,
    }

    #[repr(C)]
    struct FileRenameInfo {
        replace_if_exists: u8,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
    }

    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: u8,
    }

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        _last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        #[link_name = "NtCreateFile"]
        fn nt_create_file(
            file_handle: *mut Handle,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut c_void,
            ea_length: u32,
        ) -> NtStatus;
        #[link_name = "RtlNtStatusToDosError"]
        fn rtl_nt_status_to_dos_error(status: NtStatus) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetFileInformationByHandle"]
        fn get_file_information_by_handle(
            file: Handle,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        #[link_name = "SetFileInformationByHandle"]
        fn set_file_information_by_handle(
            file: Handle,
            information_class: u32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
    }

    pub(super) fn validate_component(name: &OsStr) -> io::Result<()> {
        let wide = name.encode_wide().collect::<Vec<_>>();
        if wide.is_empty()
            || name == OsStr::new(".")
            || name == OsStr::new("..")
            || wide.contains(&0)
            || wide.iter().any(|unit| matches!(*unit, 0x2f | 0x5c | 0x3a))
            || matches!(wide.last(), Some(unit) if *unit == b'.' as u16 || *unit == b' ' as u16)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "durable state overlay Windows component is empty, relative, aliased, or contains a separator",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_private_parent(parent: &File) -> io::Result<()> {
        if is_direct_directory(parent)? {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows overlay parent is a reparse point",
            ))
        }
    }

    pub(super) fn validate_lookup_parent(parent: &File) -> io::Result<()> {
        validate_private_parent(parent)
    }

    fn nt_error(status: NtStatus) -> io::Error {
        // SAFETY: conversion is a pure ntdll status mapping.
        let code = unsafe { rtl_nt_status_to_dos_error(status) };
        io::Error::from_raw_os_error(i32::try_from(code).unwrap_or(i32::MAX))
    }

    fn nt_open_relative(
        parent: &File,
        name: &OsStr,
        access: u32,
        share_access: u32,
        disposition: u32,
        options: u32,
    ) -> io::Result<File> {
        validate_component(name)?;
        let mut name_wide = name.encode_wide().collect::<Vec<_>>();
        let byte_len = name_wide
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u16::try_from(length).ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows overlay component is too long",
                )
            })?;
        let mut unicode = UnicodeString {
            length: byte_len,
            maximum_length: byte_len,
            buffer: name_wide.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: u32::try_from(size_of::<ObjectAttributes>())
                .expect("OBJECT_ATTRIBUTES fits u32"),
            root_directory: parent.as_raw_handle(),
            object_name: &mut unicode,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: ptr::null_mut(),
            security_quality_of_service: ptr::null_mut(),
        };
        let mut status_block = IoStatusBlock {
            status_or_pointer: 0,
            information: 0,
        };
        let mut handle = ptr::null_mut();
        // SAFETY: all ABI structures and pointers remain initialized for the call.
        let status = unsafe {
            nt_create_file(
                &mut handle,
                access,
                &mut attributes,
                &mut status_block,
                ptr::null_mut(),
                FILE_ATTRIBUTE_NORMAL,
                share_access,
                disposition,
                options | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT,
                ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            return Err(nt_error(status));
        }
        if handle.is_null() {
            return Err(io::Error::other(
                "NtCreateFile returned a null overlay handle",
            ));
        }
        // SAFETY: NtCreateFile returned a fresh owned handle.
        Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
    }

    fn open_directory_exact(parent: &File, name: &OsStr) -> io::Result<File> {
        nt_open_relative(
            parent,
            name,
            MAXIMUM_ALLOWED | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_DIRECTORY_FILE | FILE_OPEN_FOR_BACKUP_INTENT,
        )
    }

    pub(super) fn open_directory(parent: &File, name: &OsStr) -> io::Result<Option<File>> {
        match open_directory_exact(parent, name) {
            Ok(directory) if is_direct_directory(&directory)? => Ok(Some(directory)),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows overlay ancestor is a reparse point",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn create_directory(parent: &File, name: &OsStr) -> io::Result<File> {
        nt_open_relative(
            parent,
            name,
            MAXIMUM_ALLOWED | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_OPEN_FOR_BACKUP_INTENT | FILE_WRITE_THROUGH,
        )
    }

    fn is_direct_directory(file: &File) -> io::Result<bool> {
        let snapshot = file_snapshot(file)?;
        Ok(snapshot.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
            && snapshot.attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0)
    }

    fn open_anchor(path: &Path) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options
            .access_mode(MAXIMUM_ALLOWED | SYNCHRONIZE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        options.open(path)
    }

    pub(super) fn retain_parent(path: &Path) -> io::Result<(File, Vec<OsString>)> {
        let components = path.components().collect::<Vec<_>>();
        let first_normal = components
            .iter()
            .position(|component| matches!(component, Component::Normal(_)))
            .unwrap_or(components.len());
        let (mut directory, start) = if path.is_absolute() {
            let mut volume_root = PathBuf::new();
            for component in &components[..first_normal] {
                match component {
                    Component::Prefix(_) | Component::RootDir => {
                        volume_root.push(component.as_os_str())
                    }
                    Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "malformed Windows overlay volume root",
                        ));
                    }
                }
            }
            (open_anchor(&volume_root)?, first_normal)
        } else {
            (open_anchor(Path::new("."))?, 0)
        };
        if !is_direct_directory(&directory)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows overlay anchor is a reparse point",
            ));
        }
        let mut names = Vec::new();
        for component in &components[start..] {
            let name = match component {
                Component::CurDir => continue,
                Component::Normal(name) => name,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Windows overlay parent contains a relative component",
                    ));
                }
            };
            validate_component(name)?;
            names.push(name.to_os_string());
        }
        for (index, name) in names.iter().enumerate() {
            directory = match open_directory(&directory, name)? {
                Some(child) => child,
                None => return Ok((directory, names[index..].to_vec())),
            };
            if !is_direct_directory(&directory)? {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows overlay ancestor is a reparse point",
                ));
            }
        }
        Ok((directory, Vec::new()))
    }

    pub(super) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
        // Keep the temporary exclusive until its exact handle has been renamed
        // and flushed, preventing concurrent content mutation.
        nt_open_relative(
            parent,
            name,
            GENERIC_WRITE | DELETE_ACCESS | SYNCHRONIZE,
            0,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE | FILE_WRITE_THROUGH,
        )
    }

    pub(super) fn open_file(parent: &File, name: &OsStr) -> io::Result<Option<File>> {
        // Deny concurrent writers for the lifetime of each read handle while
        // still permitting handle-relative replacement via delete sharing.
        match nt_open_relative(
            parent,
            name,
            GENERIC_READ | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        ) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn file_snapshot(file: &File) -> io::Result<FileSnapshot> {
        let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
        // SAFETY: the live handle and exact writable ABI structure are valid.
        if unsafe { get_file_information_by_handle(file.as_raw_handle(), information.as_mut_ptr()) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetFileInformationByHandle initialized the structure.
        let information = unsafe { information.assume_init() };
        let combine = |high: u32, low: u32| u64::from(high) << 32 | u64::from(low);
        Ok(FileSnapshot {
            attributes: information.file_attributes,
            creation_time: combine(
                information.creation_time.high,
                information.creation_time.low,
            ),
            last_write_time: combine(
                information.last_write_time.high,
                information.last_write_time.low,
            ),
            volume_serial_number: information.volume_serial_number,
            length: combine(information.file_size_high, information.file_size_low),
            file_index: combine(information.file_index_high, information.file_index_low),
            links: information.number_of_links,
        })
    }

    pub(super) fn verify_temporary_binding(
        _parent: &File,
        _name: &OsStr,
        file: &File,
        expected: FileSnapshot,
    ) -> io::Result<()> {
        let actual = file_snapshot(file)?;
        if !expected.same_revision(actual) || !actual.is_single_link() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows overlay temporary changed",
            ));
        }
        Ok(())
    }

    pub(super) fn replace_open_file(
        parent: &File,
        file: &File,
        _temporary_name: &OsStr,
        destination_name: &OsStr,
    ) -> io::Result<()> {
        validate_component(destination_name)?;
        let target = destination_name.encode_wide().collect::<Vec<_>>();
        let target_bytes = target
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u32::try_from(length).ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows overlay target is too long",
                )
            })?;
        let offset = offset_of!(FileRenameInfo, file_name);
        let total = offset
            .checked_add(usize::try_from(target_bytes).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows overlay rename buffer is too large",
                )
            })?;
        let mut storage = vec![0usize; total.div_ceil(size_of::<usize>())];
        let info = storage.as_mut_ptr().cast::<FileRenameInfo>();
        // SAFETY: storage has the exact alignment and computed byte length.
        unsafe {
            (*info).replace_if_exists = 1;
            (*info).root_directory = parent.as_raw_handle();
            (*info).file_name_length = target_bytes;
            ptr::copy_nonoverlapping(
                target.as_ptr(),
                storage.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
                target.len(),
            );
        }
        // SAFETY: the retained handles and initialized buffer outlive the call.
        if unsafe {
            set_file_information_by_handle(
                file.as_raw_handle(),
                FILE_RENAME_INFO_CLASS,
                info.cast(),
                u32::try_from(total).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Windows overlay rename buffer exceeds u32",
                    )
                })?,
            )
        } != 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn sync_publication(_parent: &File, file: &File) -> io::Result<()> {
        // Flush the renamed write-through handle: Windows does not document
        // `FlushFileBuffers` for directory handles, while write-through file
        // handles synchronously flush rename metadata.
        file.sync_all()
    }

    pub(super) fn sync_parent_entry(
        _parent: &File,
        _child: &File,
        _created: bool,
    ) -> io::Result<()> {
        // Newly created directory handles use FILE_WRITE_THROUGH. Windows does
        // not document FlushFileBuffers as a directory-handle operation.
        Ok(())
    }

    pub(super) fn remove_open_file(
        _parent: &File,
        file: &File,
        _name: &OsStr,
        expected: FileSnapshot,
    ) {
        let Ok(actual) = file_snapshot(file) else {
            return;
        };
        if !expected.same_identity(actual) {
            return;
        }
        let disposition = FileDispositionInfo { delete_file: 1 };
        // SAFETY: disposition targets this exact retained temporary handle.
        let _ = unsafe {
            set_file_information_by_handle(
                file.as_raw_handle(),
                FILE_DISPOSITION_INFO_CLASS,
                ptr::from_ref(&disposition).cast(),
                u32::try_from(size_of::<FileDispositionInfo>())
                    .expect("FILE_DISPOSITION_INFO fits u32"),
            )
        };
    }

    pub(super) fn remove_new_file(_parent: &File, file: &File, _name: &OsStr) {
        let disposition = FileDispositionInfo { delete_file: 1 };
        // SAFETY: disposition targets the exact exclusively created handle.
        let _ = unsafe {
            set_file_information_by_handle(
                file.as_raw_handle(),
                FILE_DISPOSITION_INFO_CLASS,
                ptr::from_ref(&disposition).cast(),
                u32::try_from(size_of::<FileDispositionInfo>())
                    .expect("FILE_DISPOSITION_INFO fits u32"),
            )
        };
    }
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
mod platform {
    use super::FileSnapshot;
    use std::{
        ffi::{OsStr, OsString},
        fs::File,
        io,
        path::Path,
    };

    pub(super) fn validate_component(_name: &OsStr) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn validate_private_parent(_parent: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn validate_lookup_parent(_parent: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn retain_parent(_path: &Path) -> io::Result<(File, Vec<OsString>)> {
        Err(unsupported())
    }

    pub(super) fn open_directory(_parent: &File, _name: &OsStr) -> io::Result<Option<File>> {
        Err(unsupported())
    }

    pub(super) fn create_directory(_parent: &File, _name: &OsStr) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn create_file(_parent: &File, _name: &OsStr) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn open_file(_parent: &File, _name: &OsStr) -> io::Result<Option<File>> {
        Err(unsupported())
    }

    pub(super) fn verify_temporary_binding(
        _parent: &File,
        _name: &OsStr,
        _file: &File,
        _expected: FileSnapshot,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn replace_open_file(
        _parent: &File,
        _file: &File,
        _temporary_name: &OsStr,
        _destination_name: &OsStr,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn sync_publication(_parent: &File, _file: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn sync_parent_entry(
        _parent: &File,
        _child: &File,
        _created: bool,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_open_file(
        _parent: &File,
        _file: &File,
        _name: &OsStr,
        _expected: FileSnapshot,
    ) {
    }

    pub(super) fn remove_new_file(_parent: &File, _file: &File, _name: &OsStr) {}

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "handle-rooted durable state overlays require Unix or Windows",
        )
    }
}
