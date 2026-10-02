//! Exact global-coordinate Merkle reduction in bounded clearing tiles.
//!
//! At most 4096 row streams finalize in place in their original wiping slots.
//! Public prefix clones retain the existing SHA3-384 rate, padding and erasure.
//! Every lower frontier node is captured before its digest slot is overwritten.

use super::*;

const TILE_ROWS: usize = super::super::privacy_outer_hash::MAX_PRIVACY_OUTER_BATCH_FRAMES_V1;
const _: () = assert!(TILE_ROWS.is_power_of_two());

/// Conservative payload for the digest tile, prefix slots and named temporaries.
/// Every possible tile task is charged its own pair of hash states and digest
/// copies even though Rayon executes only a bounded subset simultaneously.
pub(super) fn payload_bound_v1() -> Result<usize, AggregateStarkErrorV1> {
    let stream = core::mem::size_of::<PrivacyOuterLastFieldStreamV1>();
    let prefix = core::mem::size_of::<PrivacyOuterDomainPrefixV1>();
    let digest = core::mem::size_of::<PrivacyOuterDigestV1>();
    let per_slot = stream
        .checked_add(digest)
        .and_then(|n| n.checked_add(2 * prefix))
        .and_then(|n| n.checked_add(3 * digest))
        .and_then(|n| n.checked_add(core::mem::size_of::<Option<(usize, AggregateStarkErrorV1)>>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    TILE_ROWS
        .checked_mul(per_slot)
        .and_then(|n| n.checked_add(usize::BITS as usize * prefix))
        .and_then(|n| n.checked_add(core::mem::size_of::<DigestTileV1>()))
        .and_then(|n| n.checked_add(core::mem::size_of::<Vec<PrivacyOuterDomainPrefixV1>>()))
        .and_then(|n| n.checked_add(32 * core::mem::size_of::<usize>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

struct DigestTileV1(Vec<PrivacyOuterDigestV1>);
impl DigestTileV1 {
    fn new(rows: usize) -> Result<Self, AggregateStarkErrorV1> {
        if rows == 0 || rows > TILE_ROWS || !rows.is_power_of_two() {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(rows)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if values.capacity() > TILE_ROWS {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        values.resize(rows, PrivacyOuterDigestV1::default());
        Ok(Self(values))
    }
    fn clear_v1(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}
impl Drop for DigestTileV1 {
    fn drop(&mut self) {
        self.clear_v1();
        #[cfg(test)]
        tests::observe_clear_v1(&self.0);
    }
}

/// Prefer the earliest public position, independent of worker completion order.
fn first_error_v1(
    left: Option<(usize, AggregateStarkErrorV1)>,
    right: Option<(usize, AggregateStarkErrorV1)>,
) -> Option<(usize, AggregateStarkErrorV1)> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (left, right) => left.or(right),
    }
}

/// Reduce one complete aligned tile and append precisely its subtree root.
pub(super) fn append_tile_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    nodes: &mut [PrivacyOuterDigestV1],
) -> Result<(), AggregateStarkErrorV1> {
    append_tile_with_cut_v1(accumulator, nodes, None)
}

fn append_tile_with_cut_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    nodes: &mut [PrivacyOuterDigestV1],
    mut cut: Option<&mut retained_commitment::RetainedMerkleCutV1>,
) -> Result<(), AggregateStarkErrorV1> {
    let first = accumulator.next_leaf;
    if accumulator.node_prefixes.capacity() > usize::BITS as usize
        || nodes.is_empty()
        || nodes.len() > TILE_ROWS
        || !nodes.len().is_power_of_two()
        || !first.is_multiple_of(nodes.len())
        || first
            .checked_add(nodes.len())
            .is_none_or(|end| end > accumulator.leaf_count)
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let mut width = nodes.len();
    let mut level = 0;
    while width > 1 {
        let global_start = first >> level;
        if level == retained_commitment::CUT_LEVEL_V1 {
            if let Some(cut) = cut.as_deref_mut() {
                cut.capture_v1(global_start, &nodes[..width])?;
            }
        }
        for (offset, &node) in nodes[..width].iter().enumerate() {
            accumulator.capture(level, global_start + offset, node)?;
        }
        let prefix = accumulator
            .node_prefixes
            .get(level)
            .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
        let failure = nodes[..width]
            .par_chunks_exact_mut(2)
            .enumerate()
            .map(|(offset, pair)| {
                let index = (global_start >> 1) + offset;
                match prefix.hash_at_with_counter(
                    index as u64,
                    0,
                    &[pair[0].as_bytes(), pair[1].as_bytes()],
                ) {
                    Some(parent) => {
                        pair[0] = parent;
                        pair[1].zeroize_v1();
                        None
                    }
                    None => Some((index, AggregateStarkErrorV1::InvalidLayout)),
                }
            })
            .reduce(|| None, first_error_v1);
        if let Some((_, error)) = failure {
            return Err(error);
        }
        // Ascending compaction never overwrites a future even-index source.
        for index in 0..width / 2 {
            nodes[index] = nodes[index * 2];
        }
        for value in &mut nodes[width / 2..width] {
            value.zeroize_v1();
        }
        width /= 2;
        level += 1;
    }
    if level == retained_commitment::CUT_LEVEL_V1 {
        if let Some(cut) = cut {
            cut.capture_v1(first >> level, &nodes[..1])?;
        }
    }
    accumulator.append_subtree_v1(level, nodes[0])
}

/// Tree input remains in exact order; all excess and short streams still fail.
pub(super) fn append_leaves_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    mut leaves: impl Iterator<Item = Result<PrivacyOuterDigestV1, AggregateStarkErrorV1>>,
) -> Result<(), AggregateStarkErrorV1> {
    let mut tile = DigestTileV1::new(accumulator.leaf_count.min(TILE_ROWS))?;
    while accumulator.next_leaf < accumulator.leaf_count {
        for target in &mut tile.0 {
            *target = leaves
                .next()
                .ok_or(AggregateStarkErrorV1::InvalidProofShape)??;
        }
        append_tile_v1(accumulator, &mut tile.0)?;
        tile.clear_v1();
    }
    if let Some(trailing) = leaves.next() {
        trailing?;
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    Ok(())
}

/// Parallel finalization wipes original slots; no private state backing is moved.
pub(super) fn finish_rows_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut [PrivacyOuterLastFieldStreamV1],
) -> Result<(), AggregateStarkErrorV1> {
    finish_rows_with_v1(accumulator, streams, |_, stream| {
        stream
            .finalize_in_place_v1()
            .map_err(map_digest_stream_error_v1)
            .map_err(map_transparent_error_v1)
    })
}

fn finish_rows_with_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    finalize: impl Fn(
        usize,
        &mut PrivacyOuterLastFieldStreamV1,
    ) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1>
    + Sync,
) -> Result<(), AggregateStarkErrorV1> {
    finish_rows_with_cut_v1(accumulator, streams, None, finalize)
}

pub(super) fn finish_rows_retaining_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    cut: &mut retained_commitment::RetainedMerkleCutV1,
) -> Result<(), AggregateStarkErrorV1> {
    finish_rows_with_cut_v1(accumulator, streams, Some(cut), |_, stream| {
        stream
            .finalize_in_place_v1()
            .map_err(map_digest_stream_error_v1)
            .map_err(map_transparent_error_v1)
    })
}

fn finish_rows_with_cut_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    mut cut: Option<&mut retained_commitment::RetainedMerkleCutV1>,
    finalize: impl Fn(
        usize,
        &mut PrivacyOuterLastFieldStreamV1,
    ) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1>
    + Sync,
) -> Result<(), AggregateStarkErrorV1> {
    if streams.len() != accumulator.leaf_count || accumulator.next_leaf != 0 {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let rows = accumulator.leaf_count.min(TILE_ROWS);
    let mut tile = DigestTileV1::new(rows)?;
    for batch in streams.chunks_mut(rows) {
        if batch.len() != rows {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let first = accumulator.next_leaf;
        let failure = batch
            .par_iter_mut()
            .zip(tile.0.par_iter_mut())
            .enumerate()
            .map(
                |(index, (stream, target))| match finalize(first + index, stream) {
                    Ok(digest) => {
                        *target = digest;
                        None
                    }
                    Err(error) => Some((index, error)),
                },
            )
            .reduce(|| None, first_error_v1);
        if let Some((_, error)) = failure {
            return Err(error);
        }
        append_tile_with_cut_v1(accumulator, &mut tile.0, cut.as_deref_mut())?;
        tile.clear_v1();
    }
    Ok(())
}

#[cfg(test)]
#[path = "streaming_commitment_tests.rs"]
mod tests;
