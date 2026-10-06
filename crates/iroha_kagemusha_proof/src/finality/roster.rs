//! Ordered internal commitment to the 31 possible normal-validator keys.
//!
//! This Poseidon tree is a proof-composition commitment, never a new trust root.
//! A schedule source must construct it from the exact authenticated result's
//! ordered roster, with zero keys in inactive positions and the 32nd leaf.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{GlueChip, Word};
use iroha_plonk_recursion::verifier::VerifierChip;

/// Domain of an indexed normal-validator compressed-key leaf.
pub const KEY_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrkyl1");
/// Domain of a depth-tagged internal ordered roster node.
pub const KEY_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrkyn1");

/// Exact internal key leaf, binding its fixed seat and all 48 original bytes.
/// # Errors
/// Seat outside the 32-leaf internal tree or circuit layout errors.
pub fn key_leaf_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    seat: u8,
    key: &[Word<Fp>; 48],
) -> Result<Word<Fp>, Error> {
    if seat >= 32 {
        return Err(Error::Synthesis);
    }
    let mut words = vec![
        chip.uint()
            .glue()
            .constant(region, Fp::from(u64::from(seat)))?,
    ];
    for chunk in key.chunks_exact(16) {
        let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
        for byte in chunk.iter().rev() {
            chip.uint().range_check::<8>(region, byte)?;
            packed = chip.uint().glue().linear(
                region,
                &[(Fp::from(256), &packed), (Fp::ONE, byte)],
                Fp::ZERO,
            )?;
        }
        words.push(packed);
    }
    chip.hash_words(region, KEY_LEAF_DOMAIN, &words)
}

/// Exact depth-tagged parent, where depth zero is directly above the leaves.
/// # Errors
/// Depth outside the five-level tree or circuit layout errors.
pub fn key_node_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    depth: u8,
    left: &Word<Fp>,
    right: &Word<Fp>,
) -> Result<Word<Fp>, Error> {
    if depth >= 5 {
        return Err(Error::Synthesis);
    }
    let depth = chip
        .uint()
        .glue()
        .constant(region, Fp::from(u64::from(depth)))?;
    chip.hash_words(
        region,
        KEY_NODE_DOMAIN,
        &[depth, left.clone(), right.clone()],
    )
}

/// Bind a fixed seat's exact original key bytes to an authenticated roster root.
/// The enclosing source separately authenticates the root and active member count.
/// # Errors
/// Invalid seat/layout; a substituted key, position or path is unsatisfiable.
pub fn verify_key_path(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    seat: u8,
    key: &[Word<Fp>; 48],
    siblings: &[Word<Fp>; 5],
    root: &Word<Fp>,
) -> Result<(), Error> {
    let mut current = key_leaf_cells(chip, region, seat, key)?;
    for (depth, sibling) in siblings.iter().enumerate() {
        let (left, right) = if (seat >> depth) & 1 == 0 {
            (&current, sibling)
        } else {
            (sibling, &current)
        };
        current = key_node_cells(chip, region, depth as u8, left, right)?;
    }
    GlueChip::assert_equal(region, &current, root)
}

/// Native encoding of an indexed compressed key for witness construction.
/// This function never validates the key or authenticates its roster.
/// # Errors
/// Seat outside the 32-leaf internal tree.
pub fn key_leaf_native(seat: u8, key: &[u8; 48]) -> Result<Fp, Error> {
    if seat >= 32 {
        return Err(Error::Synthesis);
    }
    let mut words = vec![Fp::from(u64::from(seat))];
    for chunk in key.chunks_exact(16) {
        let mut packed = Fp::ZERO;
        for byte in chunk.iter().rev() {
            packed = packed * Fp::from(256) + Fp::from(u64::from(*byte));
        }
        words.push(packed);
    }
    Ok(hash_with_domain(KEY_LEAF_DOMAIN, &words))
}

/// Native root and per-seat paths for a complete ordered active roster.
/// Inactive positions, including the 32nd leaf, are uniquely zero padded.
/// No signature validation, proof of possession or roster authority is implied.
/// # Errors
/// Committee size differs from the required `3f+1`, `1<=f<=10`.
pub fn key_tree_native(keys: &[[u8; 48]]) -> Result<(Fp, Vec<[Fp; 5]>), Error> {
    if !(4..=31).contains(&keys.len()) || !(keys.len() - 1).is_multiple_of(3) {
        return Err(Error::Synthesis);
    }
    let mut layer = (0..32)
        .map(|index| key_leaf_native(index as u8, keys.get(index).unwrap_or(&[0; 48])))
        .collect::<Result<Vec<_>, _>>()?;
    let mut paths = vec![[Fp::ZERO; 5]; 31];
    for depth in 0..5 {
        for (seat, path) in paths.iter_mut().enumerate() {
            path[depth] = layer[(seat >> depth) ^ 1];
        }
        layer = layer
            .chunks_exact(2)
            .map(|pair| {
                hash_with_domain(KEY_NODE_DOMAIN, &[Fp::from(depth as u64), pair[0], pair[1]])
            })
            .collect();
    }
    Ok((layer[0], paths))
}

#[cfg(test)]
mod tests;
