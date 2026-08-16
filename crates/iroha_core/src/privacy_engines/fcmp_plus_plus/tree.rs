//! Compact append-only FCMP++ Selene/Helios curve tree.
//!
//! Leaves contain up to 38 complete O/I/C tuples.  Their six Wei25519
//! coordinates are hashed to Selene.  Each following layer alternates between
//! 18 Selene-x children hashed to Helios and 38 Helios-x children hashed to
//! Selene.  The frontier stores only the active leaf and the mixed-radix
//! completed siblings, so persisted state is bounded independently of output set size.
use super::{
    FCMP_LAYER_ONE_LEN_V1, FCMP_LAYER_TWO_LEN_V1, FcmpNativeErrorV1, FcmpOutputTupleV1,
    FcmpTreeRootV1,
    field::{
        HeliosPoint, SelenePoint, decode_field25519_scalar, decode_helioselene_scalar,
        edwards_to_wei25519, encode_field25519_scalar, encode_helioselene_scalar, hash_helios,
        hash_selene,
    },
};
use std::collections::BTreeSet;
/// Canonical bounded state needed to append to an FCMP++ output-set tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FcmpFrontierPartsV1 {
    /// Number of output tuples already inserted.
    pub tree_size: u64,
    /// Non-empty active first-layer branch, in insertion order.
    pub active_outputs: Vec<FcmpOutputTupleV1>,
    /// Completed left siblings for each parent layer.
    ///
    /// Level zero contains canonical Helioselene-field encodings of completed
    /// Selene-child x coordinates.  Level one contains Field25519 encodings of
    /// completed Helios-child x coordinates, and the field alternates thereafter.
    pub levels: Vec<Vec<[u8; 32]>>,
    /// Root recomputed from all compact parts.
    pub root: FcmpTreeRootV1,
}
#[derive(Clone, Copy)]
enum ActiveSubtree {
    Selene(SelenePoint),
    Helios(HeliosPoint),
}
fn leaf_hash(outputs: &[FcmpOutputTupleV1]) -> Result<SelenePoint, FcmpNativeErrorV1> {
    if outputs.is_empty() || outputs.len() > FCMP_LAYER_ONE_LEN_V1 {
        return Err(FcmpNativeErrorV1::BranchWidth);
    }
    let mut coordinates = Vec::with_capacity(6 * outputs.len());
    for output in outputs {
        let (output_key, linking_tag_generator, amount_commitment) = output.components();
        for point in [output_key, linking_tag_generator, amount_commitment] {
            let (x, y) = edwards_to_wei25519(point)?;
            coordinates.push(x);
            coordinates.push(y);
        }
    }
    hash_selene(&coordinates)
}
fn expected_shape(tree_size: u64) -> Result<(usize, Vec<usize>), FcmpNativeErrorV1> {
    if tree_size == 0 {
        return Err(FcmpNativeErrorV1::EmptyOutputSet);
    }
    let active_len = usize::try_from(((tree_size - 1) % FCMP_LAYER_ONE_LEN_V1 as u64) + 1)
        .expect("first-layer width fits usize");
    let mut completed = (tree_size - 1) / FCMP_LAYER_ONE_LEN_V1 as u64;
    let mut digits = Vec::new();
    let mut level = 0_u8;
    while completed != 0 {
        if level >= super::FCMP_MAX_TREE_LAYERS_V1 - 1 {
            return Err(FcmpNativeErrorV1::TreeFull);
        }
        let width = if level % 2 == 0 {
            FCMP_LAYER_TWO_LEN_V1
        } else {
            FCMP_LAYER_ONE_LEN_V1
        };
        digits.push(
            usize::try_from(completed % width as u64).expect("compiled branch width fits usize"),
        );
        completed /= width as u64;
        level += 1;
    }
    Ok((active_len, digits))
}
fn validate_shape(parts: &FcmpFrontierPartsV1) -> Result<(), FcmpNativeErrorV1> {
    let (active_len, digits) = expected_shape(parts.tree_size)?;
    if parts.active_outputs.len() != active_len
        || parts.levels.len() != digits.len()
        || parts
            .levels
            .iter()
            .zip(digits)
            .any(|(level, expected)| level.len() != expected)
    {
        return Err(FcmpNativeErrorV1::FrontierShape);
    }
    let mut active_ids = BTreeSet::new();
    for output in &parts.active_outputs {
        // Re-run point validation across the persistence boundary.
        FcmpOutputTupleV1::new(
            output.components().0,
            output.components().1,
            output.components().2,
        )?;
        if !active_ids.insert(output.output_id()) {
            return Err(FcmpNativeErrorV1::DuplicateOutput);
        }
    }
    Ok(())
}
fn reconstruct_root(
    active_outputs: &[FcmpOutputTupleV1],
    levels: &[Vec<[u8; 32]>],
) -> Result<FcmpTreeRootV1, FcmpNativeErrorV1> {
    let mut active = ActiveSubtree::Selene(leaf_hash(active_outputs)?);
    for (level_index, completed_siblings) in levels.iter().enumerate() {
        active = if level_index % 2 == 0 {
            let ActiveSubtree::Selene(child) = active else {
                return Err(FcmpNativeErrorV1::FrontierShape);
            };
            let mut children = completed_siblings
                .iter()
                .copied()
                .map(decode_helioselene_scalar)
                .collect::<Result<Vec<_>, _>>()?;
            children.push(child.x().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?);
            if children.len() > FCMP_LAYER_TWO_LEN_V1 {
                return Err(FcmpNativeErrorV1::FrontierShape);
            }
            ActiveSubtree::Helios(hash_helios(&children)?)
        } else {
            let ActiveSubtree::Helios(child) = active else {
                return Err(FcmpNativeErrorV1::FrontierShape);
            };
            let mut children = completed_siblings
                .iter()
                .copied()
                .map(decode_field25519_scalar)
                .collect::<Result<Vec<_>, _>>()?;
            children.push(child.x().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?);
            if children.len() > FCMP_LAYER_ONE_LEN_V1 {
                return Err(FcmpNativeErrorV1::FrontierShape);
            }
            ActiveSubtree::Selene(hash_selene(&children)?)
        };
    }
    let layers = u8::try_from(levels.len() + 1).map_err(|_| FcmpNativeErrorV1::TreeFull)?;
    match active {
        ActiveSubtree::Selene(point) => FcmpTreeRootV1::from_selene(layers, point),
        ActiveSubtree::Helios(point) => FcmpTreeRootV1::from_helios(layers, point),
    }
}
fn completed_child_scalar(
    active: ActiveSubtree,
    level_index: usize,
) -> Result<[u8; 32], FcmpNativeErrorV1> {
    match (level_index % 2, active) {
        (0, ActiveSubtree::Selene(point)) => Ok(encode_helioselene_scalar(
            point.x().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
        )),
        (1, ActiveSubtree::Helios(point)) => Ok(encode_field25519_scalar(
            point.x().ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
        )),
        _ => Err(FcmpNativeErrorV1::FrontierShape),
    }
}
fn hash_full_parent(
    level_index: usize,
    children: &[[u8; 32]],
) -> Result<ActiveSubtree, FcmpNativeErrorV1> {
    if level_index % 2 == 0 {
        if children.len() != FCMP_LAYER_TWO_LEN_V1 {
            return Err(FcmpNativeErrorV1::FrontierShape);
        }
        let children = children
            .iter()
            .copied()
            .map(decode_helioselene_scalar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveSubtree::Helios(hash_helios(&children)?))
    } else {
        if children.len() != FCMP_LAYER_ONE_LEN_V1 {
            return Err(FcmpNativeErrorV1::FrontierShape);
        }
        let children = children
            .iter()
            .copied()
            .map(decode_field25519_scalar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveSubtree::Selene(hash_selene(&children)?))
    }
}
fn carry_completed_leaf(
    levels: &mut Vec<Vec<[u8; 32]>>,
    leaf: SelenePoint,
) -> Result<(), FcmpNativeErrorV1> {
    let mut active = ActiveSubtree::Selene(leaf);
    for level_index in 0..usize::from(super::FCMP_MAX_TREE_LAYERS_V1 - 1) {
        if levels.len() == level_index {
            levels.push(Vec::new());
        }
        let width = if level_index % 2 == 0 {
            FCMP_LAYER_TWO_LEN_V1
        } else {
            FCMP_LAYER_ONE_LEN_V1
        };
        let scalar = completed_child_scalar(active, level_index)?;
        let level = &mut levels[level_index];
        if level.len() + 1 < width {
            level.push(scalar);
            return Ok(());
        }
        if level.len() + 1 != width {
            return Err(FcmpNativeErrorV1::FrontierShape);
        }
        let mut children = std::mem::take(level);
        children.push(scalar);
        active = hash_full_parent(level_index, &children)?;
    }
    Err(FcmpNativeErrorV1::TreeFull)
}
fn append_one(
    active_outputs: &mut Vec<FcmpOutputTupleV1>,
    levels: &mut Vec<Vec<[u8; 32]>>,
    output: FcmpOutputTupleV1,
) -> Result<(), FcmpNativeErrorV1> {
    if active_outputs.len() == FCMP_LAYER_ONE_LEN_V1 {
        let completed_leaf = leaf_hash(active_outputs)?;
        carry_completed_leaf(levels, completed_leaf)?;
        active_outputs.clear();
    }
    active_outputs.push(output);
    Ok(())
}
/// Build canonical compact frontier parts from a complete ordered genesis output set.
pub fn build_fcmp_frontier_v1(
    outputs: &[FcmpOutputTupleV1],
) -> Result<FcmpFrontierPartsV1, FcmpNativeErrorV1> {
    if outputs.is_empty() {
        return Err(FcmpNativeErrorV1::EmptyOutputSet);
    }
    let mut ids = BTreeSet::new();
    if outputs.iter().any(|output| !ids.insert(output.output_id())) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    let mut active_outputs = Vec::with_capacity(FCMP_LAYER_ONE_LEN_V1);
    let mut levels = Vec::new();
    for output in outputs {
        append_one(&mut active_outputs, &mut levels, *output)?;
    }
    let tree_size = u64::try_from(outputs.len()).map_err(|_| FcmpNativeErrorV1::TreeFull)?;
    let root = reconstruct_root(&active_outputs, &levels)?;
    let parts = FcmpFrontierPartsV1 {
        tree_size,
        active_outputs,
        levels,
        root,
    };
    validate_fcmp_frontier_v1(&parts)?;
    Ok(parts)
}
/// Reconstruct and authenticate persisted FCMP++ compact frontier parts.
pub fn validate_fcmp_frontier_v1(parts: &FcmpFrontierPartsV1) -> Result<(), FcmpNativeErrorV1> {
    validate_shape(parts)?;
    let reconstructed = reconstruct_root(&parts.active_outputs, &parts.levels)?;
    if reconstructed != parts.root {
        return Err(FcmpNativeErrorV1::RootMismatch);
    }
    Ok(())
}
/// Append a non-empty ordered output batch and return the validator-derived successor frontier.
///
/// The compact frontier can only detect duplicates in the active leaf and the
/// new batch.  Ledger integration must additionally check each `output_id`
/// against the protocol-scoped durable output registry before calling this function.
pub fn append_fcmp_outputs_v1(
    current: &FcmpFrontierPartsV1,
    outputs: &[FcmpOutputTupleV1],
) -> Result<FcmpFrontierPartsV1, FcmpNativeErrorV1> {
    validate_fcmp_frontier_v1(current)?;
    if outputs.is_empty() {
        return Err(FcmpNativeErrorV1::EmptyOutputSet);
    }
    let mut ids = current
        .active_outputs
        .iter()
        .map(|output| output.output_id())
        .collect::<BTreeSet<_>>();
    if outputs.iter().any(|output| !ids.insert(output.output_id())) {
        return Err(FcmpNativeErrorV1::DuplicateOutput);
    }
    let mut next = current.clone();
    for output in outputs {
        append_one(&mut next.active_outputs, &mut next.levels, *output)?;
        next.tree_size = next
            .tree_size
            .checked_add(1)
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
    }
    next.root = reconstruct_root(&next.active_outputs, &next.levels)?;
    validate_fcmp_frontier_v1(&next)?;
    Ok(next)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::output_from_multiples;
    fn vector(encoded: &str) -> [u8; 32] {
        assert_eq!(encoded.len(), 64);
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&encoded[(2 * index)..(2 * index) + 2], 16)
                .expect("pinned reference vector is hexadecimal");
        }
        bytes
    }
    fn outputs(count: usize) -> Vec<FcmpOutputTupleV1> {
        (0..count)
            .map(|index| {
                let base = u64::try_from(index)
                    .expect("test index fits u64")
                    .checked_mul(3)
                    .expect("test multiple fits u64")
                    + 1;
                output_from_multiples(base, base + 1, base + 2)
            })
            .collect()
    }
    #[test]
    fn frontier_uses_exact_mixed_radix_boundaries() {
        let at_leaf = build_fcmp_frontier_v1(&outputs(38)).expect("one full leaf");
        assert_eq!(at_leaf.active_outputs.len(), 38);
        assert!(at_leaf.levels.is_empty());
        assert_eq!(at_leaf.root.layers(), 1);
        let next_output = output_from_multiples(4_000, 4_001, 4_002);
        let second_leaf =
            append_fcmp_outputs_v1(&at_leaf, &[next_output]).expect("first output in second leaf");
        assert_eq!(second_leaf.active_outputs.len(), 1);
        let mut completed_leaf_x = at_leaf.root.point();
        completed_leaf_x[31] &= 0x7f;
        assert_eq!(second_leaf.levels, vec![vec![completed_leaf_x]]);
        assert_eq!(second_leaf.root.layers(), 2);
        let full_second_layer =
            build_fcmp_frontier_v1(&outputs(38 * 18)).expect("one full Helios layer");
        assert_eq!(full_second_layer.root.layers(), 2);
        assert_eq!(full_second_layer.levels.len(), 1);
        assert_eq!(full_second_layer.levels[0].len(), 17);
        let third_layer = append_fcmp_outputs_v1(
            &full_second_layer,
            &[output_from_multiples(10_001, 10_002, 10_003)],
        )
        .expect("first output beyond full Helios layer");
        assert_eq!(third_layer.root.layers(), 3);
        assert_eq!(third_layer.levels.len(), 2);
        assert!(third_layer.levels[0].is_empty());
        assert_eq!(third_layer.levels[1].len(), 1);
        let mut reordered_layers = third_layer;
        reordered_layers.levels.swap(0, 1);
        assert_eq!(
            validate_fcmp_frontier_v1(&reordered_layers),
            Err(FcmpNativeErrorV1::FrontierShape)
        );
    }
    #[test]
    fn root_is_order_sensitive_and_append_matches_complete_build() {
        let all = outputs(45);
        let complete = build_fcmp_frontier_v1(&all).expect("complete build");
        let first = build_fcmp_frontier_v1(&all[..41]).expect("prefix");
        let appended = append_fcmp_outputs_v1(&first, &all[41..]).expect("append");
        assert_eq!(appended, complete);
        let mut reordered = all;
        reordered.swap(0, 1);
        assert_ne!(
            build_fcmp_frontier_v1(&reordered).expect("reordered").root,
            complete.root
        );
    }
    #[test]
    fn ordered_tree_roots_match_upstream_vectors_across_curve_layers() {
        // Generated directly with monero-fcmp-plus-plus 0.1.0 at 15ef711.
        assert_eq!(
            build_fcmp_frontier_v1(&outputs(1))
                .expect("one-output tree")
                .root
                .point(),
            vector("b391b38ca41c3b31460fa64e77c574944675fcbc968a831c741b2bcccd9c4de5")
        );
        assert_eq!(
            build_fcmp_frontier_v1(&outputs(38))
                .expect("one full leaf")
                .root
                .point(),
            vector("118890dbba8bfc4fb82e61f263868049dc8050554c613bd4a78c7060a3113a63")
        );
        let root_39 = build_fcmp_frontier_v1(&outputs(39))
            .expect("first even-layer root")
            .root;
        assert_eq!(root_39.layers(), 2);
        assert_eq!(
            root_39.point(),
            vector("6e10ad9d95a60598eba3978197231f0111b5364d60bac26b0264f53959e4296b")
        );
    }
    #[test]
    fn persisted_frontier_rejects_shape_scalar_and_root_malleation() {
        let valid = build_fcmp_frontier_v1(&outputs(45)).expect("valid frontier");
        let mut wrong_size = valid.clone();
        wrong_size.tree_size += 1;
        assert_eq!(
            validate_fcmp_frontier_v1(&wrong_size),
            Err(FcmpNativeErrorV1::FrontierShape)
        );
        let mut wrong_digit = valid.clone();
        wrong_digit.levels[0].push([1; 32]);
        assert_eq!(
            validate_fcmp_frontier_v1(&wrong_digit),
            Err(FcmpNativeErrorV1::FrontierShape)
        );
        let mut noncanonical_scalar = valid.clone();
        noncanonical_scalar.levels[0][0] = [u8::MAX; 32];
        assert_eq!(
            validate_fcmp_frontier_v1(&noncanonical_scalar),
            Err(FcmpNativeErrorV1::ScalarEncoding)
        );
        let mut wrong_root = valid.clone();
        wrong_root.root = build_fcmp_frontier_v1(&outputs(1))
            .expect("other root")
            .root;
        assert_eq!(
            validate_fcmp_frontier_v1(&wrong_root),
            Err(FcmpNativeErrorV1::RootMismatch)
        );
    }
    #[test]
    fn empty_and_duplicate_sets_fail_closed() {
        assert_eq!(
            build_fcmp_frontier_v1(&[]),
            Err(FcmpNativeErrorV1::EmptyOutputSet)
        );
        let output = output_from_multiples(1, 2, 3);
        assert_eq!(
            build_fcmp_frontier_v1(&[output, output]),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        );
        let current = build_fcmp_frontier_v1(&[output]).expect("frontier");
        assert_eq!(
            append_fcmp_outputs_v1(&current, &[output]),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        );
    }
}
