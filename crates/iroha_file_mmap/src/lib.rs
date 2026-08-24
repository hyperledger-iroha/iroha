//! Safe operating-system file primitives used by persistent storage backends.
//!
//! This crate wraps [`memmap2`] for validated copy-on-write read-only mappings
//! and exposes a stable Windows by-handle metadata query. Their narrowly scoped
//! operating-system calls are contained here so consumers compiled with
//! `-D unsafe-code` do not need to relax their lint settings.
#![allow(unsafe_code)]
use memmap2::{Mmap, MmapOptions};
use std::{fs::File, io};

#[cfg(windows)]
mod windows {
    use std::{ffi::c_void, fs::File, io, mem::MaybeUninit, os::windows::io::AsRawHandle as _};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
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

    /// Stable Win32 identity, revision, and link metadata sampled from one open handle.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct WindowsFileInformation {
        file_attributes: u32,
        creation_time: u64,
        last_write_time: u64,
        volume_serial_number: u32,
        file_size: u64,
        number_of_links: u32,
        file_index: u64,
    }

    impl WindowsFileInformation {
        /// Win32 file attribute bits.
        pub const fn file_attributes(self) -> u32 {
            self.file_attributes
        }

        /// Creation time in 100-nanosecond intervals since the Windows epoch.
        pub const fn creation_time(self) -> u64 {
            self.creation_time
        }

        /// Last-write time in 100-nanosecond intervals since the Windows epoch.
        pub const fn last_write_time(self) -> u64 {
            self.last_write_time
        }

        /// Serial number of the volume containing the file.
        pub const fn volume_serial_number(self) -> u32 {
            self.volume_serial_number
        }

        /// File length in bytes.
        pub const fn file_size(self) -> u64 {
            self.file_size
        }

        /// Number of filesystem links to the file.
        pub const fn number_of_links(self) -> u32 {
            self.number_of_links
        }

        /// File identity within its volume.
        pub const fn file_index(self) -> u64 {
            self.file_index
        }
    }

    const fn combine(high: u32, low: u32) -> u64 {
        (high as u64) << 32 | low as u64
    }

    /// Sample stable Win32 metadata from an already-open file or directory handle.
    ///
    /// # Errors
    ///
    /// Returns the operating-system error reported by `GetFileInformationByHandle`.
    pub fn windows_file_information(file: &File) -> io::Result<WindowsFileInformation> {
        let mut raw = MaybeUninit::<ByHandleFileInformation>::uninit();
        // SAFETY: `file` owns a valid handle and `raw` points to writable storage
        // with the exact documented BY_HANDLE_FILE_INFORMATION layout.
        let succeeded = unsafe {
            get_file_information_by_handle(file.as_raw_handle().cast(), raw.as_mut_ptr())
        };
        if succeeded == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a successful GetFileInformationByHandle call initializes the
        // complete output structure.
        let raw = unsafe { raw.assume_init() };
        Ok(WindowsFileInformation {
            file_attributes: raw.file_attributes,
            creation_time: combine(raw.creation_time.high, raw.creation_time.low),
            last_write_time: combine(raw.last_write_time.high, raw.last_write_time.low),
            volume_serial_number: raw.volume_serial_number,
            file_size: combine(raw.file_size_high, raw.file_size_low),
            number_of_links: raw.number_of_links,
            file_index: combine(raw.file_index_high, raw.file_index_low),
        })
    }
}

#[cfg(windows)]
pub use windows::{WindowsFileInformation, windows_file_information};
/// Read-only memory mapped view over a file.
#[derive(Debug)]
pub struct ReadOnlyMmap {
    mmap: Mmap,
}
impl ReadOnlyMmap {
    /// Create a copy-on-write read-only mapping covering the first `len` bytes of `file`.
    ///
    /// Returns [`io::ErrorKind::UnexpectedEof`] if `len` exceeds the current length of `file`
    /// as measured both before and immediately after the mapping is created.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the underlying operating system call fails while
    /// establishing the mapping.
    pub fn copy_read_only(file: &File, len: usize) -> io::Result<Self> {
        let file_len = file.metadata()?.len();
        Self::copy_read_only_with_file_len(file, len, file_len)
    }
    /// Create a copy-on-write read-only mapping using a pre-fetched file length.
    ///
    /// This avoids re-querying metadata before the mapping is created, which is useful when
    /// the caller has already fetched the value (e.g., to size caches or mirrors).
    ///
    /// Returns [`io::ErrorKind::UnexpectedEof`] if `len` exceeds the current length of `file`
    /// as measured both before and immediately after the mapping is created.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the underlying operating system call fails while
    /// establishing the mapping.
    pub fn copy_read_only_with_file_len(
        file: &File,
        len: usize,
        file_len: u64,
    ) -> io::Result<Self> {
        let len_u64 = len as u64;
        if len_u64 > file_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("requested mapping length {len} exceeds file length {file_len}"),
            ));
        }
        let mut options = MmapOptions::new();
        options.len(len);
        // Safety: we request a read-only copy-on-write mapping. Callers guarantee that
        // the file is not truncated while the mapping is live, and Iroha invalidates
        // mirrors before resizing files, so the mapped region remains valid.
        let mmap = unsafe { options.map_copy_read_only(file) }?;
        let post_map_len = file.metadata()?.len();
        if len_u64 > post_map_len {
            drop(mmap);
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "requested mapping length {len} exceeds file length {post_map_len} after mapping"
                ),
            ));
        }
        Ok(Self { mmap })
    }
    /// Total number of bytes visible through the mapping.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mmap.len()
    }
    /// Reports whether the mapping exposes any bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
    /// Returns the mapped bytes as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.mmap.as_ref()
    }
}
impl core::ops::Deref for ReadOnlyMmap {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.mmap.as_ref()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{ErrorKind, Write};
    use tempfile::NamedTempFile;

    #[cfg(windows)]
    #[test]
    fn windows_information_is_stable_for_one_open_file() {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(b"metadata").expect("write temp file");
        file.flush().expect("flush temp file");

        let before = windows_file_information(file.as_file()).expect("sample file information");
        let after = windows_file_information(file.as_file()).expect("resample file information");
        assert_eq!(before, after);
        assert_eq!(before.file_size(), 8);
        assert_eq!(before.number_of_links(), 1);
    }
    #[test]
    fn mapping_reports_length_and_non_empty_state() {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(b"iroha").expect("write file contents");
        let mmap = ReadOnlyMmap::copy_read_only(file.as_file(), 5).expect("create mapping");
        assert_eq!(mmap.len(), 5);
        assert!(!mmap.is_empty());
        assert_eq!(mmap.as_slice(), b"iroha");
    }
    #[test]
    fn mapping_fails_when_len_exceeds_file_size() {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(b"iroha").expect("write file contents");
        let file_len = file.as_file().metadata().expect("metadata").len();
        let err = ReadOnlyMmap::copy_read_only(file.as_file(), 6)
            .expect_err("mapping longer than file must fail");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
        let err = ReadOnlyMmap::copy_read_only_with_file_len(file.as_file(), 6, file_len)
            .expect_err("mapping longer than file must fail with cached length");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }
    #[test]
    fn mapping_detects_truncate_race() {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(b"iroha").expect("write file contents");
        let file_len = file.as_file().metadata().expect("metadata").len();
        let truncated_len = file_len.checked_sub(1).expect("non-zero file length");
        file.as_file_mut()
            .set_len(truncated_len)
            .expect("truncate file");
        let len_usize =
            usize::try_from(file_len).expect("file length from metadata fits into usize");
        let err = ReadOnlyMmap::copy_read_only_with_file_len(file.as_file(), len_usize, file_len)
            .expect_err("mapping should detect truncate race");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }
}
