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

/// Initial pairs alone need reconstructed original finalizers and serialized
/// scratch. Selected replay keeps the original scalar bound above.
pub(super) fn initial_payload_bound_v1() -> Result<usize, AggregateStarkErrorV1> {
    let scalar = payload_bound_v1()?;
    TILE_ROWS
        .checked_mul(crate::privacy_engines::privacy_outer_hash::paired_stream_workspace_bytes_v1())
        .and_then(|paired| scalar.checked_add(paired))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

pub(super) struct DigestTileV1(pub(super) Vec<PrivacyOuterDigestV1>);
impl DigestTileV1 {
    pub(super) fn new(rows: usize) -> Result<Self, AggregateStarkErrorV1> {
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
    pub(super) fn clear_v1(&mut self) {
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
pub(super) fn first_error_v1(
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
    cut: Option<&mut retained_commitment::RetainedMerkleCutV1>,
) -> Result<(), AggregateStarkErrorV1> {
    let first = accumulator.next_leaf;
    let level = reduce_tile_at_level_v1(accumulator, nodes, first, 0, cut)?;
    accumulator.append_subtree_v1(level, nodes[0])
}

/// Reduce a bounded aligned tile at its original tree level, capturing every
/// lower frontier node before overwriting it. The caller owns ordered append.
pub(super) fn reduce_tile_at_level_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    nodes: &mut [PrivacyOuterDigestV1],
    first: usize,
    initial_level: usize,
    mut cut: Option<&mut retained_commitment::RetainedMerkleCutV1>,
) -> Result<usize, AggregateStarkErrorV1> {
    let span = nodes
        .len()
        .checked_shl(
            u32::try_from(initial_level).map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        )
        .filter(|&span| span != 0)
        .ok_or(AggregateStarkErrorV1::InvalidProofShape)?;
    if accumulator.node_prefixes.capacity() > usize::BITS as usize
        || nodes.is_empty()
        || nodes.len() > TILE_ROWS
        || !nodes.len().is_power_of_two()
        || !first.is_multiple_of(span)
        || first
            .checked_add(span)
            .is_none_or(|end| end > accumulator.leaf_count)
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let mut width = nodes.len();
    let mut level = initial_level;
    while width > 1 {
        let global_start = first >> level;
        if level == retained_commitment::CUT_LEVEL_V1
            && let Some(cut) = cut.as_deref_mut()
        {
            cut.capture_v1(global_start, &nodes[..width])?;
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
                prefix
                    .hash_at_with_counter(
                        index as u64,
                        0,
                        &[pair[0].as_bytes(), pair[1].as_bytes()],
                    )
                    .map_or(
                        Some((index, AggregateStarkErrorV1::InvalidLayout)),
                        |parent| {
                            pair[0] = parent;
                            pair[1].zeroize_v1();
                            None
                        },
                    )
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
    if level == retained_commitment::CUT_LEVEL_V1
        && let Some(cut) = cut
    {
        cut.capture_v1(first >> level, &nodes[..1])?;
    }
    Ok(level)
}

/// Verify all retained subtree roots without serializing their upper Merkle
/// reductions. The existing clearing tile and worker reserve cover this copy.
pub(super) fn append_retained_subtrees_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    roots: &[PrivacyOuterDigestV1],
    level: usize,
) -> Result<(), AggregateStarkErrorV1> {
    if roots.is_empty() || !roots.len().is_power_of_two() {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let mut tile = DigestTileV1::new(roots.len().min(TILE_ROWS))?;
    for chunk in roots.chunks(tile.0.len()) {
        tile.0.copy_from_slice(chunk);
        let first = accumulator.next_leaf;
        let top = reduce_tile_at_level_v1(accumulator, &mut tile.0, first, level, None)?;
        accumulator.append_subtree_v1(top, tile.0[0])?;
        tile.clear_v1();
    }
    Ok(())
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
#[cfg(test)]
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

#[cfg(test)]
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

pub(super) fn finish_owned_rows_retaining_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut RowStreamsV1,
    cut: &mut retained_commitment::RetainedMerkleCutV1,
) -> Result<(), AggregateStarkErrorV1> {
    match streams {
        RowStreamsV1::Scalar(values) => finish_rows_retaining_v1(accumulator, values, cut),
        RowStreamsV1::InitialPairs(values) => {
            finish_pairs_with_cut_v1(accumulator, values, Some(cut))
        }
    }
}

#[cfg(test)]
pub(super) fn finish_owned_rows_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut RowStreamsV1,
) -> Result<(), AggregateStarkErrorV1> {
    match streams {
        RowStreamsV1::Scalar(values) => finish_rows_v1(accumulator, values),
        RowStreamsV1::InitialPairs(values) => finish_pairs_with_cut_v1(accumulator, values, None),
    }
}

fn finish_pairs_with_cut_v1(
    accumulator: &mut StreamingMerkleAccumulatorV1,
    streams: &mut [crate::privacy_engines::privacy_outer_hash::PrivacyOuterLastFieldPairV1],
    mut cut: Option<&mut retained_commitment::RetainedMerkleCutV1>,
) -> Result<(), AggregateStarkErrorV1> {
    if streams.len().checked_mul(2) != Some(accumulator.leaf_count)
        || accumulator.next_leaf != 0
        || accumulator.leaf_count < 2
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let rows = accumulator.leaf_count.min(TILE_ROWS);
    let mut tile = DigestTileV1::new(rows)?;
    for batch in streams.chunks_mut(rows / 2) {
        if batch.len() != rows / 2 {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let first = accumulator.next_leaf;
        let failure = batch
            .par_iter_mut()
            .zip(tile.0.par_chunks_exact_mut(2))
            .enumerate()
            .map(|(index, (stream, target))| {
                let target: &mut [PrivacyOuterDigestV1; 2] = match target.try_into() {
                    Ok(target) => target,
                    Err(_) => {
                        return Some((first + 2 * index, AggregateStarkErrorV1::InternalInvariant));
                    }
                };
                stream.finalize_into_v1(target).err().map(|(lane, error)| {
                    (
                        first + 2 * index + lane,
                        map_transparent_error_v1(map_digest_stream_error_v1(error)),
                    )
                })
            })
            .reduce(|| None, first_error_v1);
        if let Some((_, error)) = failure {
            return Err(error);
        }
        append_tile_with_cut_v1(accumulator, &mut tile.0, cut.as_deref_mut())?;
        tile.clear_v1();
    }
    Ok(())
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
