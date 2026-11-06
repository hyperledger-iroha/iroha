//! Retained, bounded payload input for publishing an already prepared native CAR plan.
//!
//! Acquisition authenticates local file custody and exact length, not payload commitments.
//! Callers must verify the plan, manifest and complete stream before publication. The original
//! descriptor and its ancestors remain retained across every verification and upload pass.

use crate::CarBuildPlan;
use iroha_fs::{FileSnapshot, RetainedFile};
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// Maximum on-disk payload accepted by prepared publication; never an in-memory allocation.
pub const MAX_STREAMING_INPUT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// A reviewed plan and the exact retained file containing its flattened payload.
pub struct PreparedCarInput {
    /// Original chunk and file geometry, preserved without rechunking or repacking.
    pub plan: CarBuildPlan,
    /// Seekable payload backed by one retained native file descriptor.
    pub payload: NativePayloadReader,
}

impl PreparedCarInput {
    /// Retain a bounded regular payload file with exactly the supplied plan's length.
    ///
    /// The native plan and existing metadata heap limits are checked before opening the source.
    /// Content is not read here: the caller must reproduce the native payload/CAR/PoR commitments
    /// against its independently authenticated manifest, for example with
    /// `payload_verifier::verify_payload_reader` when the `manifest` feature is enabled.
    ///
    /// # Errors
    /// Rejects invalid or oversized plans, length mismatches, links, unsafe custody and I/O errors.
    pub fn open(path: &Path, plan: CarBuildPlan) -> io::Result<Self> {
        if plan.content_length > MAX_STREAMING_INPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prepared payload exceeds 4 GiB limit",
            ));
        }
        plan.validate_for_ingest()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let file = RetainedFile::open_regular(path)?;
        let snapshot = file.snapshot()?;
        if snapshot.local_parts().2 != plan.content_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prepared payload length differs from the exact native plan",
            ));
        }
        let payload = NativePayloadReader {
            file,
            snapshot,
            length: plan.content_length,
            position: 0,
        };
        payload.revalidate()?;
        Ok(Self { plan, payload })
    }
}

/// Bounded seekable payload view that rejects changes to its retained file or namespace.
pub struct NativePayloadReader {
    file: RetainedFile,
    snapshot: FileSnapshot,
    length: u64,
    position: u64,
}

impl NativePayloadReader {
    /// Revalidate retained custody, identity, length and modification metadata.
    ///
    /// This is a local stability check, not a replacement for cryptographic content validation.
    ///
    /// # Errors
    /// Rejects mutation, replacement, namespace/custody changes and native I/O errors.
    pub fn revalidate(&self) -> io::Result<()> {
        if self.file.snapshot()? != self.snapshot {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prepared payload changed after acquisition",
            ));
        }
        Ok(())
    }
}

impl Read for NativePayloadReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.revalidate()?;
        let count = usize::try_from((self.length - self.position).min(bytes.len() as u64))
            .map_err(io::Error::other)?;
        if count == 0 {
            return Ok(0);
        }
        // Positional I/O leaves the retained descriptor's cursor out of the reader's authority.
        iroha_fs::read_exact_at(self.file.file(), &mut bytes[..count], self.position)?;
        self.revalidate()?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for NativePayloadReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.length) + i128::from(offset),
        };
        if !(0..=i128::from(self.length)).contains(&next) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prepared payload seek outside exact input",
            ));
        }
        self.revalidate()?;
        self.position = next as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests;
