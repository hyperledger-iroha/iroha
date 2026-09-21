//! Bounded authentication of the original snapshot hash-journal prefix.

use std::io::{self, Read, Seek, SeekFrom};

use super::{
    BlockHeader, BlockStore, Error, Hash, HashOf, Result, SIZE_OF_BLOCK_HASH,
    VERIFIED_SNAPSHOT_TAIL_DIGEST_DOMAIN,
};

/// Check the complete byte range before seeking or allocating a hash result.
pub(super) fn checked_block_hash_read_range(
    start_block_height: u64,
    block_count: usize,
    file_len: u64,
) -> Result<(u64, u64)> {
    let range = start_block_height
        .checked_mul(SIZE_OF_BLOCK_HASH)
        .zip(
            u64::try_from(block_count)
                .ok()
                .and_then(|count| count.checked_mul(SIZE_OF_BLOCK_HASH)),
        )
        .filter(|(start, required)| {
            start
                .checked_add(*required)
                .is_some_and(|end| end <= file_len)
        });
    range.ok_or(Error::OutOfBoundsBlockRead {
        start_block_height,
        block_count,
    })
}

/// Hash an already owned snapshot slice without allocating a second inventory.
pub(super) fn verified_snapshot_hash_journal_digest(
    snapshot_hashes: &[HashOf<BlockHeader>],
) -> io::Result<Hash> {
    let snapshot_height = u64::try_from(snapshot_hashes.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Hash::new_from_writer(|writer| {
        writer.write_all(VERIFIED_SNAPSHOT_TAIL_DIGEST_DOMAIN)?;
        writer.write_all(&snapshot_height.to_le_bytes())?;
        for hash in snapshot_hashes {
            writer.write_all(hash.as_ref())?;
        }
        Ok(())
    })
}

impl BlockStore {
    /// Hash only the requested durable prefix using the original cached handle.
    ///
    /// Scratch is a fixed 4 KiB on the ordinary worker stack. Invalid ranges
    /// refuse before seeking; malformed entries and short reads retain the
    /// original journal's I/O error context and never authorize marker repair.
    pub(super) fn verified_snapshot_hash_journal_digest_from_store(
        &mut self,
        snapshot_height: u64,
    ) -> Result<Hash> {
        const HASHES_PER_CHUNK: usize = 128;
        let block_count = usize::try_from(snapshot_height)?;
        let hashes_file = self.ensure_hashes_file()?;
        let file_len = hashes_file.try_io(|file| file.metadata().map(|meta| meta.len()))?;
        let (start, _) = checked_block_hash_read_range(0, block_count, file_len)?;
        hashes_file.try_io(|file| {
            file.seek(SeekFrom::Start(start))?;
            Hash::new_from_writer(|writer| {
                writer.write_all(VERIFIED_SNAPSHOT_TAIL_DIGEST_DOMAIN)?;
                writer.write_all(&snapshot_height.to_le_bytes())?;
                let mut buffer = [0_u8; HASHES_PER_CHUNK * Hash::LENGTH];
                let mut remaining = block_count;
                while remaining > 0 {
                    let count = remaining.min(HASHES_PER_CHUNK);
                    let chunk = &mut buffer[..count * Hash::LENGTH];
                    file.read_exact(chunk)?;
                    for hash in chunk.chunks_exact(Hash::LENGTH) {
                        if hash[Hash::LENGTH - 1] & 1 == 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "block hash journal entry lacks the canonical marker bit",
                            ));
                        }
                    }
                    writer.write_all(chunk)?;
                    remaining -= count;
                }
                Ok(())
            })
        })
    }
}
