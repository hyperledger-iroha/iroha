//! Stable filesystem metadata snapshots for security-sensitive path checks.

#[cfg(unix)]
mod unix {
    use std::{
        fs::{self, File, OpenOptions},
        io,
        ops::Deref,
        os::unix::fs::OpenOptionsExt as _,
        path::Path,
    };

    /// Standard metadata with a retained handle that pins the sampled filesystem object.
    #[derive(Debug)]
    pub(crate) struct SecureMetadata {
        metadata: fs::Metadata,
        _identity_handle: File,
    }

    impl Deref for SecureMetadata {
        type Target = fs::Metadata;

        fn deref(&self) -> &Self::Target {
            &self.metadata
        }
    }

    pub(crate) fn from_file(file: &File) -> io::Result<SecureMetadata> {
        Ok(SecureMetadata {
            metadata: file.metadata()?,
            _identity_handle: file.try_clone()?,
        })
    }

    pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY)
            .open(path)?;
        Ok(SecureMetadata {
            metadata: file.metadata()?,
            _identity_handle: file,
        })
    }
}

#[cfg(unix)]
pub(crate) use unix::{SecureMetadata, from_file, from_path};

#[cfg(not(any(unix, windows)))]
use std::{fs, io, path::Path};

#[cfg(not(any(unix, windows)))]
pub(crate) type SecureMetadata = fs::Metadata;

#[cfg(not(any(unix, windows)))]
pub(crate) fn from_file(file: &fs::File) -> io::Result<SecureMetadata> {
    file.metadata()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
    fs::symlink_metadata(path)
}

#[cfg(unix)]
pub(crate) fn same_file(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
pub(crate) fn same_file(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    left.same_file(right)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn same_file(_left: &SecureMetadata, _right: &SecureMetadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    same_file(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
}

#[cfg(windows)]
pub(crate) fn unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    left.unchanged(right)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn unchanged(_left: &SecureMetadata, _right: &SecureMetadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn is_direct_file(metadata: &SecureMetadata) -> bool {
    !metadata.file_type().is_symlink() && metadata.file_type().is_file()
}

#[cfg(windows)]
pub(crate) fn is_direct_file(metadata: &SecureMetadata) -> bool {
    metadata.is_direct_file()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_direct_file(_metadata: &SecureMetadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn is_direct_directory(metadata: &SecureMetadata) -> bool {
    !metadata.file_type().is_symlink() && metadata.file_type().is_dir()
}

#[cfg(windows)]
pub(crate) fn is_direct_directory(metadata: &SecureMetadata) -> bool {
    metadata.is_direct_directory()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_direct_directory(_metadata: &SecureMetadata) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn number_of_links(metadata: &SecureMetadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt as _;

    Some(metadata.nlink())
}

#[cfg(windows)]
pub(crate) fn number_of_links(metadata: &SecureMetadata) -> Option<u64> {
    Some(u64::from(metadata.number_of_links()))
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn number_of_links(_metadata: &SecureMetadata) -> Option<u64> {
    None
}

#[cfg(windows)]
mod windows {
    #![allow(unsafe_code)]

    use std::{
        ffi::c_void,
        fs::{self, File, OpenOptions},
        io,
        mem::MaybeUninit,
        ops::Deref,
        os::windows::{fs::OpenOptionsExt as _, io::AsRawHandle as _},
        path::Path,
    };

    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_SHARE_READ_DELETE: u32 = 0x0000_0001 | 0x0000_0004;
    const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    const _: () = assert!(std::mem::size_of::<ByHandleFileInformation>() == 52);
    const _: () = assert!(std::mem::align_of::<ByHandleFileInformation>() == 4);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetFileInformationByHandle"]
        fn get_file_information_by_handle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    impl FileInformation {
        const fn is_direct_file(self) -> bool {
            self.file_attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) == 0
        }

        const fn is_direct_directory(self) -> bool {
            self.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                && self.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
        }

        const fn same_file(self, other: Self) -> bool {
            self.volume_serial_number == other.volume_serial_number
                && self.file_index_high == other.file_index_high
                && self.file_index_low == other.file_index_low
        }
    }

    fn information(file: &File) -> io::Result<FileInformation> {
        let mut raw = MaybeUninit::<ByHandleFileInformation>::uninit();
        // SAFETY: `file` owns a valid handle and `raw` points to writable storage with the
        // documented `BY_HANDLE_FILE_INFORMATION` layout.
        let succeeded = unsafe {
            get_file_information_by_handle(file.as_raw_handle().cast(), raw.as_mut_ptr())
        };
        if succeeded == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a successful `GetFileInformationByHandle` call initializes the entire output.
        let raw = unsafe { raw.assume_init() };
        Ok(FileInformation {
            file_attributes: raw.file_attributes,
            creation_time: raw.creation_time,
            last_write_time: raw.last_write_time,
            volume_serial_number: raw.volume_serial_number,
            file_size_high: raw.file_size_high,
            file_size_low: raw.file_size_low,
            number_of_links: raw.number_of_links,
            file_index_high: raw.file_index_high,
            file_index_low: raw.file_index_low,
        })
    }

    /// Standard metadata bound to stable Win32 identity, revision, and link fields.
    #[derive(Debug)]
    pub(crate) struct SecureMetadata {
        metadata: fs::Metadata,
        information: FileInformation,
        // Keep the sampled object alive so its file identifier cannot be recycled before a
        // subsequent snapshot is compared with it. Every pathname handle is opened with delete
        // sharing, so retaining it does not prevent a legitimate atomic replacement.
        _identity_handle: File,
    }

    impl SecureMetadata {
        pub(super) const fn same_file(&self, other: &Self) -> bool {
            self.information.same_file(other.information)
        }

        pub(super) fn unchanged(&self, other: &Self) -> bool {
            self.information == other.information
        }

        pub(super) const fn is_direct_file(&self) -> bool {
            self.information.is_direct_file()
        }

        pub(super) const fn is_direct_directory(&self) -> bool {
            self.information.is_direct_directory()
        }

        pub(super) const fn number_of_links(&self) -> u32 {
            self.information.number_of_links
        }
    }

    impl Deref for SecureMetadata {
        type Target = fs::Metadata;

        fn deref(&self) -> &Self::Target {
            &self.metadata
        }
    }

    fn sample(file: &File) -> io::Result<(fs::Metadata, FileInformation)> {
        let information_before = information(file)?;
        let metadata = file.metadata()?;
        let information_after = information(file)?;
        if information_before != information_after {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file metadata changed while it was sampled",
            ));
        }
        Ok((metadata, information_after))
    }

    pub(crate) fn from_file(file: &File) -> io::Result<SecureMetadata> {
        let (metadata, information) = sample(file)?;
        Ok(SecureMetadata {
            metadata,
            information,
            _identity_handle: file.try_clone()?,
        })
    }

    pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
        let file = OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ_WRITE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let (metadata, information) = sample(&file)?;
        Ok(SecureMetadata {
            metadata,
            information,
            _identity_handle: file,
        })
    }

    pub(crate) fn open_direct_file(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            // Deny write sharing for the complete lifetime of bounded reads. Delete sharing
            // remains enabled so a pathname replacement is observable instead of merely blocked.
            .share_mode(FILE_SHARE_READ_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }
}

#[cfg(windows)]
pub(crate) use windows::{SecureMetadata, from_file, from_path, open_direct_file};

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use std::{fs, io::Write as _};

    use super::*;

    #[test]
    fn snapshots_bind_path_and_handle_to_one_object() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("record.bin");
        fs::write(&path, b"stable").expect("write record");
        let file = fs::File::open(&path).expect("open record");

        let named = from_path(&path).expect("snapshot named record");
        let opened = from_file(&file).expect("snapshot opened record");

        assert!(is_direct_file(&named));
        assert!(same_file(&named, &opened));
        assert!(unchanged(&named, &opened));
        assert_eq!(number_of_links(&opened), Some(1));
        assert!(is_direct_directory(
            &from_path(directory.path()).expect("snapshot directory")
        ));
    }

    #[test]
    fn retained_snapshot_rejects_an_equal_length_replacement() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("record.bin");
        let displaced = directory.path().join("record.displaced.bin");
        fs::write(&path, b"same-length").expect("write original record");
        let original = from_path(&path).expect("snapshot original record");

        fs::rename(&path, &displaced).expect("displace original record");
        fs::write(&path, b"same-length").expect("write replacement record");
        let replacement = from_path(&path).expect("snapshot replacement record");

        assert!(!same_file(&original, &replacement));
        assert!(!unchanged(&original, &replacement));
    }

    #[test]
    fn revision_snapshot_detects_in_place_length_change() {
        let mut file = tempfile::NamedTempFile::new().expect("create temporary file");
        file.write_all(b"before").expect("write initial bytes");
        file.flush().expect("flush initial bytes");
        let before = from_file(file.as_file()).expect("snapshot original revision");

        file.write_all(b"-after").expect("extend temporary file");
        file.flush().expect("flush extended bytes");
        let after = from_file(file.as_file()).expect("snapshot changed revision");

        assert!(same_file(&before, &after));
        assert!(!unchanged(&before, &after));
    }

    #[test]
    fn link_count_snapshot_tracks_hard_links() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("record.bin");
        let link = directory.path().join("record.link.bin");
        fs::write(&path, b"linked").expect("write record");
        let before = from_path(&path).expect("snapshot unlinked record");
        fs::hard_link(&path, &link).expect("create hard link");
        let after = from_path(&path).expect("snapshot linked record");

        assert_eq!(number_of_links(&before), Some(1));
        assert_eq!(number_of_links(&after), Some(2));
        assert!(same_file(&before, &after));
        assert!(!unchanged(&before, &after));
    }
}
