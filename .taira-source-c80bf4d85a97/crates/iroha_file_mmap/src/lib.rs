//! Safe read-only memory mapping helper used by persistent storage backends.
//!
//! This crate wraps [`memmap2`] and exposes a copy-on-write read-only mapping
//! constructor that validates invariants required by Iroha. The unsafe call is
//! contained within this crate so that consumers compiled with `-D unsafe-code`
//! can still rely on memory mapping without relaxing their lint settings.

#![allow(unsafe_code)]

use std::{fs::File, io};

use memmap2::{Mmap, MmapOptions};

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
    use std::io::{ErrorKind, Write};

    use tempfile::NamedTempFile;

    use super::*;

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
