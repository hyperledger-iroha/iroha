//! Checked ranges for canonical block hash-journal reads.

use super::{Error, Result, SIZE_OF_BLOCK_HASH};

/// Refuse offset, byte-count and end overflow before allocation or seeking.
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
