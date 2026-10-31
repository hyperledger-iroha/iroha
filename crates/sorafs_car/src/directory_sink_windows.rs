//! Bounded native Windows chunk staging with explicit directory publication outcomes.

use super::*;
#[cfg(test)]
use iroha_fs::DirectoryMoveStep;
use iroha_fs::{PrivateDirectory, RetainedFile};
use std::{ffi::OsString, fs, path::Component};

#[derive(Debug)]
pub(super) struct NativeSink {
    parent: Option<PrivateDirectory>,
    staging: Option<PrivateDirectory>,
    destination: OsString,
    expected: Vec<ExpectedSinkChunk>,
    records: Vec<PersistedChunkRecord>,
    expected_bytes: u64,
    total_bytes: u64,
    pending: [Option<RetainedFile>; DIRECTORY_SYNC_BATCH_CHUNKS],
    pending_len: usize,
    poisoned: bool,
    #[cfg(test)]
    fault: Option<Fault>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    PrepareSync,
    Flush(usize),
    Move(DirectoryMoveStep),
}

impl NativeSink {
    pub(super) fn new() -> Self {
        Self {
            parent: None,
            staging: None,
            destination: OsString::new(),
            expected: Vec::new(),
            records: Vec::new(),
            expected_bytes: 0,
            total_bytes: 0,
            pending: std::array::from_fn(|_| None),
            pending_len: 0,
            poisoned: false,
            #[cfg(test)]
            fault: None,
        }
    }

    fn healthy(&self) -> Result<(), ChunkStoreError> {
        if self.poisoned {
            return Err(ChunkStoreError::Io(io::Error::other(
                "native chunk sink cannot publish after a write or durability failure",
            )));
        }
        Ok(())
    }

    fn staging(&self) -> Result<&PrivateDirectory, ChunkStoreError> {
        self.staging.as_ref().ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("native chunk sink was not prepared"))
        })
    }

    fn require_absent(parent: &PrivateDirectory, name: &std::ffi::OsStr) -> io::Result<()> {
        match parent.custody_entry_kind(name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => parent.revalidate(),
            Err(error) => Err(error),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "native chunk destination must remain absent",
            )),
        }
    }

    pub(super) fn prepare(
        &mut self,
        root: &Path,
        plan: &CarBuildPlan,
        heap_limit: usize,
    ) -> Result<(), ChunkStoreError> {
        self.healthy()?;
        if self.staging.is_some() {
            return Err(ChunkStoreError::Io(io::Error::other(
                "native chunk sink is already prepared",
            )));
        }
        plan.validate_for_ingest_with_limit(heap_limit)?;
        let Some(Component::Normal(name)) = root.components().next_back() else {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native chunk destination must end in a normal component",
            )));
        };
        let parent_path = root
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = PrivateDirectory::open(parent_path).map_err(ChunkStoreError::Io)?;
        Self::require_absent(&parent, name).map_err(ChunkStoreError::Io)?;
        self.records.clear();
        self.expected.clear();
        for (context, result) in [
            (
                "native directory sink records",
                self.records.try_reserve_exact(plan.chunks.len()),
            ),
            (
                "native directory sink expected chunks",
                self.expected.try_reserve_exact(plan.chunks.len()),
            ),
        ] {
            result.map_err(|_| ChunkStoreError::AllocationFailed {
                context,
                requested: plan.chunks.len(),
            })?;
        }
        self.expected
            .extend(plan.chunks.iter().map(|chunk| ExpectedSinkChunk {
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
            }));
        let mut staging = None;
        for _ in 0..ATOMIC_PATH_RETRIES {
            let nonce: [u8; 16] = rand::random();
            match parent.create_child(format!(".sorafs-chunks.{}.partial", hex::encode(nonce))) {
                Ok(directory) => {
                    staging = Some(directory);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(ChunkStoreError::Io(error)),
            }
        }
        self.staging = Some(staging.ok_or_else(|| {
            ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "cannot allocate native chunk staging directory",
            ))
        })?);
        self.parent = Some(parent);
        self.destination = name.to_owned();
        self.expected_bytes = plan.content_length;
        self.total_bytes = 0;
        let result = (|| {
            #[cfg(test)]
            if self.fault == Some(Fault::PrepareSync) {
                return Err(ChunkStoreError::Io(io::Error::other(
                    "injected native staging-parent sync failure",
                )));
            }
            self.parent
                .as_ref()
                .expect("retained parent")
                .sync()
                .map_err(ChunkStoreError::Io)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    pub(super) fn write_chunk(
        &mut self,
        index: usize,
        chunk: &CarChunk,
        data: &[u8],
    ) -> Result<(), ChunkStoreError> {
        self.healthy()?;
        if index != self.records.len() || index >= self.expected.len() {
            return Err(ChunkStoreError::SinkChunkOrder {
                expected: self.records.len(),
                actual: index,
            });
        }
        let expected = self.expected[index];
        if (expected.offset, expected.length, expected.digest)
            != (chunk.offset, chunk.length, chunk.digest)
        {
            return Err(ChunkStoreError::SinkChunkMetadataMismatch { chunk_index: index });
        }
        if data.len() != expected.length as usize {
            return Err(ChunkStoreError::SinkChunkLengthMismatch {
                chunk_index: index,
                expected: expected.length as usize,
                actual: data.len(),
            });
        }
        if blake3::hash(data).as_bytes() != &expected.digest {
            return Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: index });
        }
        let file_name = format!("chunk_{index:05}.bin");
        let result = (|| {
            let staging = self.staging()?;
            let mut file = staging
                .create_custody_writer(&file_name)
                .map_err(ChunkStoreError::Io)?;
            file.file_mut()
                .write_all(data)
                .map_err(ChunkStoreError::Io)?;
            file.revalidate().map_err(ChunkStoreError::Io)?;
            staging.revalidate().map_err(ChunkStoreError::Io)?;
            Ok(file)
        })();
        let file = match result {
            Ok(file) => file,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.pending[self.pending_len] = Some(file);
        self.pending_len += 1;
        self.records.push(PersistedChunkRecord {
            file_name,
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.digest,
        });
        self.total_bytes = self
            .total_bytes
            .checked_add(data.len() as u64)
            .ok_or(ChunkStoreError::PayloadLengthTooLarge)?;
        if self.pending_len == DIRECTORY_SYNC_BATCH_CHUNKS {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), ChunkStoreError> {
        self.healthy()?;
        let result = (|| {
            self.staging()?.revalidate().map_err(ChunkStoreError::Io)?;
            for (index, file) in self.pending[..self.pending_len].iter().enumerate() {
                let file = file.as_ref().expect("retained original chunk writer");
                file.revalidate().map_err(ChunkStoreError::Io)?;
                #[cfg(test)]
                if self.fault == Some(Fault::Flush(index)) {
                    return Err(ChunkStoreError::Io(io::Error::other(
                        "injected original chunk flush failure",
                    )));
                }
                #[cfg(not(test))]
                let _ = index;
                file.file().sync_all().map_err(ChunkStoreError::Io)?;
                file.revalidate().map_err(ChunkStoreError::Io)?;
            }
            self.staging()?.revalidate().map_err(ChunkStoreError::Io)
        })();
        if result.is_err() {
            self.poisoned = true;
            return result;
        }
        for file in &mut self.pending[..self.pending_len] {
            *file = None;
        }
        self.pending_len = 0;
        Ok(())
    }

    fn validate_contents(&self) -> Result<(), ChunkStoreError> {
        let staging = self.staging()?;
        // Native ancestry handles exclude rename/deletion of this original root.
        // Join its named path before and after the bounded iterator; never allocate
        // a plan-sized collection of names or follow a child during counting.
        staging.revalidate().map_err(ChunkStoreError::Io)?;
        let mut count = 0_usize;
        for entry in fs::read_dir(staging.path()).map_err(ChunkStoreError::Io)? {
            entry.map_err(ChunkStoreError::Io)?;
            count = count
                .checked_add(1)
                .ok_or(ChunkStoreError::PayloadLengthTooLarge)?;
            if count > self.expected.len() {
                break;
            }
        }
        staging.revalidate().map_err(ChunkStoreError::Io)?;
        if count != self.expected.len() {
            return Err(ChunkStoreError::SinkIncomplete {
                expected_chunks: self.expected.len(),
                actual_chunks: count,
                expected_bytes: self.expected_bytes,
                actual_bytes: self.total_bytes,
            });
        }
        let mut buffer = [0_u8; 64 * 1024];
        for (index, expected) in self.expected.iter().enumerate() {
            let mut original = staging
                .open_retained_private(format!("chunk_{index:05}.bin"))
                .map_err(ChunkStoreError::Io)?;
            let before = original.snapshot().map_err(ChunkStoreError::Io)?;
            let length = original
                .file()
                .metadata()
                .map_err(ChunkStoreError::Io)?
                .len();
            if length != u64::from(expected.length) {
                return Err(ChunkStoreError::SinkChunkLengthMismatch {
                    chunk_index: index,
                    expected: expected.length as usize,
                    actual: usize::try_from(length).unwrap_or(usize::MAX),
                });
            }
            let mut remaining = expected.length as usize;
            let mut digest = Hasher::new();
            while remaining != 0 {
                let count = remaining.min(buffer.len());
                original
                    .file_mut()
                    .read_exact(&mut buffer[..count])
                    .map_err(ChunkStoreError::Io)?;
                digest.update(&buffer[..count]);
                remaining -= count;
            }
            let mut extra = [0_u8; 1];
            if original
                .file_mut()
                .read(&mut extra)
                .map_err(ChunkStoreError::Io)?
                != 0
                || original.snapshot().map_err(ChunkStoreError::Io)? != before
            {
                return Err(ChunkStoreError::Io(io::Error::other(
                    "native staged chunk changed during readback",
                )));
            }
            if digest.finalize().as_bytes() != &expected.digest {
                return Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: index });
            }
        }
        staging.revalidate().map_err(ChunkStoreError::Io)
    }

    pub(super) fn finish(mut self) -> Result<DirectoryChunkSinkOutput, ChunkStoreError> {
        self.healthy()?;
        if self.records.len() != self.expected.len() || self.total_bytes != self.expected_bytes {
            return Err(ChunkStoreError::SinkIncomplete {
                expected_chunks: self.expected.len(),
                actual_chunks: self.records.len(),
                expected_bytes: self.expected_bytes,
                actual_bytes: self.total_bytes,
            });
        }
        self.flush()?;
        self.validate_contents()?;
        self.staging()?.sync().map_err(ChunkStoreError::Io)?;
        let parent = self.parent.as_ref().expect("prepared native parent");
        Self::require_absent(parent, &self.destination).map_err(ChunkStoreError::Io)?;
        let staging = self.staging.take().expect("prepared original staging");
        // The native move owns the original through publication. Any move failure
        // consumes that authority; no pathname cleanup is attempted afterward.
        let result = staging.move_no_replace_observed(parent, &self.destination, |step| {
            #[cfg(test)]
            if self.fault == Some(Fault::Move(step)) {
                return Err(io::Error::other("injected native move boundary failure"));
            }
            #[cfg(not(test))]
            let _ = step;
            Ok(())
        });
        let publication = match result {
            Ok(original) => {
                if original.revalidate().is_ok() {
                    DirectoryPublicationStatus::Durable
                } else {
                    DirectoryPublicationStatus::PublishedButDurabilityUncertain
                }
            }
            Err(error) if error.publication_may_be_visible() => {
                DirectoryPublicationStatus::PublishedButDurabilityUncertain
            }
            Err(error) => {
                return Err(ChunkStoreError::Io(io::Error::new(
                    error.io_error().kind(),
                    error,
                )));
            }
        };
        Ok(DirectoryChunkSinkOutput {
            records: std::mem::take(&mut self.records),
            total_bytes: self.total_bytes,
            publication,
        })
    }
}

impl Drop for NativeSink {
    fn drop(&mut self) {
        // Release every child before touching only our still-retained staging root.
        for file in &mut self.pending {
            *file = None;
        }
        if let Some(staging) = self.staging.take()
            && staging.clear_contents_preserving(&[]).is_ok()
        {
            let _ = staging.remove_empty();
        }
    }
}

#[cfg(test)]
mod tests;
