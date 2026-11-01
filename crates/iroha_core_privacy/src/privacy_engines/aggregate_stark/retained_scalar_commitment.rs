//! Small clearing cuts for composition and FRI commitments, with exact global
//! frontier coordinates and complete retained-root validation before publication.

use super::*;

pub(crate) const CUT_LEVEL_V1: usize = 12;
const CUT_ROWS_V1: usize = 1 << CUT_LEVEL_V1;
const _: () =
    assert!(CUT_ROWS_V1 == super::super::privacy_outer_hash::MAX_PRIVACY_OUTER_BATCH_FRAMES_V1);

/// Private original roots of aligned 4096-leaf subtrees.
pub(crate) struct RetainedScalarCutV1 {
    rows: usize,
    roots: Vec<PrivacyOuterDigestV1>,
    root: PrivacyOuterDigestV1,
}

impl RetainedScalarCutV1 {
    fn new_v1(rows: usize) -> Result<Self, AggregateStarkErrorV1> {
        if rows < CUT_ROWS_V1 || !rows.is_power_of_two() {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let count = rows / CUT_ROWS_V1;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(count)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if roots.capacity() != count {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        Ok(Self {
            rows,
            roots,
            root: PrivacyOuterDigestV1::default(),
        })
    }

    /// Allocation payload; callers separately charge their inline option slots.
    pub(crate) fn allocation_bound_v1(rows: usize) -> Result<usize, AggregateStarkErrorV1> {
        if rows == 0 || !rows.is_power_of_two() {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        (rows / CUT_ROWS_V1)
            .checked_mul(core::mem::size_of::<PrivacyOuterDigestV1>())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    pub(crate) fn allocated_payload_bytes_v1(&self) -> Result<usize, AggregateStarkErrorV1> {
        if self.roots.len() != self.rows / CUT_ROWS_V1 || self.roots.capacity() != self.roots.len()
        {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        Self::allocation_bound_v1(self.rows)
    }
}

impl Drop for RetainedScalarCutV1 {
    fn drop(&mut self) {
        for root in &mut self.roots {
            root.zeroize_v1();
        }
        self.root.zeroize_v1();
        #[cfg(test)]
        tests::observe_clear_v1(&self.roots, self.root);
    }
}

/// Complete capacities and inline owners retained by composition and FRI.
pub(crate) fn oracle_payload_bound_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let rounds = layout.fri_rounds(parameters)?;
    let slot = core::mem::size_of::<Option<RetainedScalarCutV1>>();
    let composition = RetainedScalarCutV1::allocation_bound_v1(layout.common_lde_size())?;
    let mut per_lane = slot
        .checked_add(core::mem::size_of::<AggregateRetainedStreamingFriLaneV1>())
        .and_then(|n| {
            rounds
                .checked_mul(slot)
                .and_then(|slots| n.checked_add(slots))
        })
        .and_then(|n| n.checked_add(composition))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let mut rows = layout.common_lde_size();
    for _ in 0..rounds {
        per_lane = per_lane
            .checked_add(RetainedScalarCutV1::allocation_bound_v1(
                parameters.fri_commitment_layout.leaf_count(rows)?,
            )?)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        rows /= 2;
    }
    per_lane
        .checked_mul(parameters.security_lanes)
        .and_then(|n| n.checked_add(2 * core::mem::size_of::<Vec<usize>>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Charge actual allocator capacities, including collection and owner headers.
#[allow(clippy::ptr_arg)]
pub(crate) fn oracle_payload_bytes_v1(
    composition: &Vec<Option<RetainedScalarCutV1>>,
    fri: &Vec<AggregateRetainedStreamingFriLaneV1>,
) -> Result<usize, AggregateStarkErrorV1> {
    let mut bytes = composition
        .capacity()
        .checked_mul(core::mem::size_of::<Option<RetainedScalarCutV1>>())
        .and_then(|n| {
            fri.capacity()
                .checked_mul(core::mem::size_of::<AggregateRetainedStreamingFriLaneV1>())
                .and_then(|extra| n.checked_add(extra))
        })
        .and_then(|n| n.checked_add(2 * core::mem::size_of::<Vec<usize>>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    for cut in composition.iter().flatten() {
        bytes = bytes
            .checked_add(cut.allocated_payload_bytes_v1()?)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    }
    for lane in fri {
        bytes = bytes
            .checked_add(lane.cut_payload_bytes_v1()?)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    }
    Ok(bytes)
}

fn fill_v1(
    tile: &mut streaming_commitment::DigestTileV1,
    first: usize,
    leaf: &(impl Fn(usize) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1> + Sync),
) -> Result<(), AggregateStarkErrorV1> {
    let failure = tile
        .0
        .par_iter_mut()
        .enumerate()
        .map(|(offset, target)| {
            leaf(first + offset).map_or_else(
                |error| Some((first + offset, error)),
                |digest| {
                    *target = digest;
                    None
                },
            )
        })
        .reduce(|| None, streaming_commitment::first_error_v1);
    failure.map_or(Ok(()), |(_, error)| Err(error))
}

/// Hash bounded leaf tiles once, optionally retaining their original roots.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_v1(
    context: TransparentStarkDigestContextV1,
    node_role: &'static [u8],
    rows: usize,
    opening_indices: &[usize],
    retain: bool,
    leaf: impl Fn(usize) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1> + Sync,
) -> Result<(StreamingMerkleCommitmentV1, Option<RetainedScalarCutV1>), AggregateStarkErrorV1> {
    let mut acc = StreamingMerkleAccumulatorV1::new(context, node_role, rows, opening_indices)?;
    let mut cut = if retain && rows >= CUT_ROWS_V1 {
        Some(RetainedScalarCutV1::new_v1(rows)?)
    } else {
        None
    };
    let mut tile = streaming_commitment::DigestTileV1::new(rows.min(CUT_ROWS_V1))?;
    while acc.next_leaf < rows {
        let first = acc.next_leaf;
        fill_v1(&mut tile, first, &leaf)?;
        let level =
            streaming_commitment::reduce_tile_at_level_v1(&mut acc, &mut tile.0, first, 0, None)?;
        if let Some(cut) = cut.as_mut() {
            if level != CUT_LEVEL_V1 || cut.roots.len() != first / CUT_ROWS_V1 {
                return Err(AggregateStarkErrorV1::InvalidProofShape);
            }
            cut.roots.push(tile.0[0]);
        }
        acc.append_subtree_v1(level, tile.0[0])?;
        tile.clear_v1();
    }
    let commitment = acc.finish()?;
    if let Some(cut) = cut.as_mut() {
        cut.allocated_payload_bytes_v1()?;
        cut.root = commitment.root;
    }
    Ok((commitment, cut))
}

/// Rehash selected original subtrees, then validate every retained upper node.
/// Empty openings still validate the full cut root. Small trees replay fully.
#[allow(clippy::too_many_arguments)]
pub(super) fn open_v1(
    context: TransparentStarkDigestContextV1,
    node_role: &'static [u8],
    rows: usize,
    opening_indices: &[usize],
    expected: PrivacyOuterDigestV1,
    cut: Option<&RetainedScalarCutV1>,
    leaf: impl Fn(usize) -> Result<PrivacyOuterDigestV1, AggregateStarkErrorV1> + Sync,
) -> Result<StreamingMerkleCommitmentV1, AggregateStarkErrorV1> {
    let Some(cut) = cut else {
        if rows >= CUT_ROWS_V1 {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let commitment = commit_v1(context, node_role, rows, opening_indices, false, leaf)?.0;
        return if commitment.root == expected {
            Ok(commitment)
        } else {
            Err(AggregateStarkErrorV1::InvalidProofShape)
        };
    };
    if cut.rows != rows || cut.root != expected {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    cut.allocated_payload_bytes_v1()?;
    let mut acc = StreamingMerkleAccumulatorV1::new(context, node_role, rows, opening_indices)?;
    let mut tile = streaming_commitment::DigestTileV1::new(CUT_ROWS_V1)?;
    let mut previous = None;
    for &index in opening_indices {
        let block = index / CUT_ROWS_V1;
        if previous == Some(block) {
            continue;
        }
        previous = Some(block);
        let first = block * CUT_ROWS_V1;
        fill_v1(&mut tile, first, &leaf)?;
        streaming_commitment::reduce_tile_at_level_v1(&mut acc, &mut tile.0, first, 0, None)?;
        if tile.0[0] != cut.roots[block] {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        tile.clear_v1();
    }
    // Release the lower tile before the upper reduction borrows the same reserve.
    drop(tile);
    streaming_commitment::append_retained_subtrees_v1(&mut acc, &cut.roots, CUT_LEVEL_V1)?;
    if acc.pending.last().copied().flatten() != Some(expected) {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    acc.finish()
}

#[cfg(test)]
#[path = "retained_scalar_commitment_tests.rs"]
mod tests;
