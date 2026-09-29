//! Retained, bounded file reads for the authenticated body-restoration worker.
//!
//! Disk bytes remain untrusted. Completion returns their original charged backing for
//! canonical decoding; only the protocol restoration job can produce `AvailableBody`.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

use mv::allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};

/// Failure to progress an original file read, without losing its source or partial backing.
#[derive(Debug)]
pub(super) enum ArtifactReadError {
    /// The filesystem refused progress or the source changed during the read.
    Io(io::Error),
    /// The exact production pool refused the physical byte allocation.
    Allocation(ChargedBufferError),
    /// The pending allocation belongs to a different pool.
    ForeignBudget,
}

impl std::fmt::Display for ArtifactReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Allocation(error) => error.fmt(f),
            Self::ForeignBudget => f.write_str("artifact read belongs to another allocation pool"),
        }
    }
}

impl std::error::Error for ArtifactReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::ForeignBudget => None,
        }
    }
}

/// One open source and, after admission, one non-growing charged destination.
pub(super) struct ArtifactRead {
    path: PathBuf,
    file: File,
    length: usize,
    bytes: Option<ChargedBuffer<u8>>,
}

impl ArtifactRead {
    /// Open a regular bounded artifact. Only a missing path is `None`; malformed files,
    /// permission errors, and allocation failures must never masquerade as absence.
    pub(super) fn open(path: &Path, max_bytes: usize) -> io::Result<Option<Self>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
            options.custom_flags(i32::from_ne_bytes(flags.bits().to_ne_bytes()));
        }
        #[cfg(not(unix))]
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(invalid("artifact is not a regular file"));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
            _ => {}
        }
        let file = match options.open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        let length =
            usize::try_from(metadata.len()).map_err(|_| invalid("artifact length overflow"))?;
        if !metadata.is_file() || length > max_bytes {
            return Err(invalid(
                "artifact type or length exceeds the storage boundary",
            ));
        }
        Ok(Some(Self {
            path: path.into(),
            file,
            length,
            bytes: None,
        }))
    }

    /// Complete into the same original byte allocation. Every refusal returns this exact job;
    /// retries continue at its current file offset, with its already-filled prefix intact.
    #[allow(
        clippy::result_large_err,
        reason = "return the original read owner without allocating"
    )]
    pub(super) fn complete(
        mut self,
        budget: &AllocationBudget,
    ) -> Result<ChargedBuffer<u8>, (Self, ArtifactReadError)> {
        if let Err(error) = self.fill(budget) {
            return Err((self, error));
        }
        Ok(self.bytes.take().expect("completed original read backing"))
    }

    fn fill(&mut self, budget: &AllocationBudget) -> Result<(), ArtifactReadError> {
        if let Some(bytes) = &self.bytes {
            if !bytes.belongs_to(budget) {
                return Err(ArtifactReadError::ForeignBudget);
            }
        } else {
            self.bytes = Some(
                ChargedBuffer::new(self.length, budget).map_err(ArtifactReadError::Allocation)?,
            );
        }
        let bytes = self.bytes.as_mut().expect("original allocation retained");
        let mut scratch = [0u8; 8192];
        while bytes.as_slice().len() < self.length {
            let wanted = (self.length - bytes.as_slice().len()).min(scratch.len());
            let count = match self.file.read(&mut scratch[..wanted]) {
                Ok(0) => {
                    return Err(ArtifactReadError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "artifact truncated during read",
                    )));
                }
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(ArtifactReadError::Io(error)),
            };
            bytes
                .append(&scratch[..count])
                .map_err(ArtifactReadError::Io)?;
        }
        let mut trailing = [0];
        loop {
            match self.file.read(&mut trailing) {
                Ok(0) => break,
                Ok(_) => return Err(ArtifactReadError::Io(invalid("artifact grew during read"))),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(ArtifactReadError::Io(error)),
            }
        }
        let opened = self.file.metadata().map_err(ArtifactReadError::Io)?;
        let current = fs::symlink_metadata(&self.path).map_err(ArtifactReadError::Io)?;
        if opened.len() != self.length as u64 || !current.file_type().is_file() {
            return Err(ArtifactReadError::Io(invalid(
                "artifact source changed during read",
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if opened.dev() != current.dev() || opened.ino() != current.ino() {
                return Err(ArtifactReadError::Io(invalid(
                    "artifact path no longer names the original source",
                )));
            }
        }
        Ok(())
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_refusal_keeps_the_open_source_for_exact_retry() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("artifact");
        fs::write(&path, b"original").unwrap();
        let budget = AllocationBudget::new(8);
        let occupied = ChargedBuffer::<u8>::new(1, &budget).unwrap();
        let read = ArtifactRead::open(&path, 8).unwrap().unwrap();
        let (read, error) = read.complete(&budget).err().unwrap();
        assert!(matches!(error, ArtifactReadError::Allocation(_)));
        assert!(read.bytes.is_none());
        drop(occupied);
        let bytes = read.complete(&budget).ok().unwrap();
        assert_eq!(bytes.as_slice(), b"original");
        assert!(bytes.belongs_to(&budget));
    }

    #[test]
    fn partial_read_retains_exact_pointer_and_rejects_a_foreign_pool() {
        use std::io::{Seek as _, SeekFrom, Write as _};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("artifact");
        fs::write(&path, b"original").unwrap();
        let read = ArtifactRead::open(&path, 8).unwrap().unwrap();
        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(4).unwrap();
        let budget = AllocationBudget::new(8);
        let (read, error) = read.complete(&budget).err().unwrap();
        assert!(
            matches!(error, ArtifactReadError::Io(ref error) if error.kind() == io::ErrorKind::UnexpectedEof)
        );
        let pointer = read.bytes.as_ref().unwrap().as_slice().as_ptr();
        assert_eq!(read.bytes.as_ref().unwrap().as_slice(), b"orig");
        let (read, error) = read.complete(&AllocationBudget::new(8)).err().unwrap();
        assert!(matches!(error, ArtifactReadError::ForeignBudget));
        assert_eq!(read.bytes.as_ref().unwrap().as_slice().as_ptr(), pointer);
        file.seek(SeekFrom::Start(4)).unwrap();
        file.write_all(b"inal").unwrap();
        let bytes = read.complete(&budget).ok().unwrap();
        assert_eq!(bytes.as_slice(), b"original");
        assert_eq!(bytes.as_slice().as_ptr(), pointer);
    }

    #[test]
    fn absence_length_and_source_replacement_are_distinct() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("artifact");
        assert!(ArtifactRead::open(&path, 8).unwrap().is_none());
        fs::write(&path, b"too large").unwrap();
        assert!(ArtifactRead::open(&path, 8).is_err());
        fs::write(&path, b"original").unwrap();
        let read = ArtifactRead::open(&path, 8).unwrap().unwrap();
        let replacement = root.path().join("replacement");
        fs::write(&replacement, b"original").unwrap();
        fs::rename(&replacement, &path).unwrap();
        #[cfg(unix)]
        assert!(matches!(
            read.complete(&AllocationBudget::new(8)),
            Err((_, ArtifactReadError::Io(_)))
        ));
        #[cfg(not(unix))]
        drop(read);
    }
}
