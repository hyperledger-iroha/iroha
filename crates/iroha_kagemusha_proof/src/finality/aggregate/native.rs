//! Bounded untrusted witness generation using the native BLS arithmetic library.

use super::*;
use crate::finality::roster::key_tree_native;
use ark_bls12_381::{G1Affine, G1Projective};
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{PrimeField, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

/// Derive the proposed aggregate key from an exact bounded roster and bitmap.
/// This native sum grants no authority; the complete aggregation source proves it.
/// # Errors
/// Noncanonical committee geometry, keys, signer bitmap, or identity aggregate.
pub fn propose_aggregate_key(keys: &[[u8; 48]], bitmap: &[u8]) -> Result<[u8; 48], Error> {
    let n = keys.len();
    if !(4..=31).contains(&n)
        || !(n - 1).is_multiple_of(3)
        || bitmap.len() != n.div_ceil(8)
        || (n..bitmap.len() * 8).any(|i| (bitmap[i / 8] >> (i % 8)) & 1 != 0)
        || (0..n)
            .filter(|&i| (bitmap[i / 8] >> (i % 8)) & 1 != 0)
            .count()
            != n - (n - 1) / 3
    {
        return Err(Error::Synthesis);
    }
    let mut sum = G1Projective::zero();
    for (i, key) in keys.iter().enumerate() {
        let point =
            G1Affine::deserialize_compressed(key.as_slice()).map_err(|_| Error::Synthesis)?;
        if point.is_zero() {
            return Err(Error::Synthesis);
        }
        if (bitmap[i / 8] >> (i % 8)) & 1 != 0 {
            sum += point;
        }
    }
    let point = sum.into_affine();
    if point.is_zero() {
        return Err(Error::Synthesis);
    }
    let mut bytes = Vec::with_capacity(48);
    point
        .serialize_compressed(&mut bytes)
        .map_err(|_| Error::Synthesis)?;
    bytes.try_into().map_err(|_| Error::Synthesis)
}

fn point_witness(point: G1Affine) -> G1AffineWitness {
    if point.is_zero() {
        G1AffineWitness {
            x: [0; 6],
            y: [0; 6],
            infinity: true,
        }
    } else {
        G1AffineWitness {
            x: point.x.into_bigint().0,
            y: point.y.into_bigint().0,
            infinity: false,
        }
    }
}

/// Prepare the complete 33-leaf ordered sum from original roster and QC bytes.
/// Every output is an untrusted witness; only the fixed program's proofs and
/// genesis-rooted roster authentication grant finality authority.
/// # Errors
/// Bad committee/quorum geometry, noncanonical or invalid keys, spare signer
/// bits, wrong aggregate, or internal witness/frame construction failure.
pub fn prepare_aggregation(
    keys: &[[u8; 48]],
    bitmap: &[u8],
    aggregate_key: [u8; 48],
) -> Result<Vec<AggregateLeafCircuit>, Error> {
    let (roster_root, paths) = key_tree_native(keys)?;
    let n = keys.len();
    let f = (n - 1) / 3;
    if bitmap.len() != n.div_ceil(8) {
        return Err(Error::Synthesis);
    }
    let mut padded = [0u8; 4];
    padded[..bitmap.len()].copy_from_slice(bitmap);
    let selected = |seat: usize| (padded[seat / 8] >> (seat % 8)) & 1 == 1;
    if (n..32).any(selected) || (0..n).filter(|&i| selected(i)).count() != n - f {
        return Err(Error::Synthesis);
    }
    let points = keys
        .iter()
        .map(|key| G1Affine::deserialize_compressed(key.as_slice()).map_err(|_| Error::Synthesis))
        .collect::<Result<Vec<_>, _>>()?;
    if points.iter().any(AffineRepr::is_zero) {
        return Err(Error::Synthesis);
    }
    let context = AggregateContext {
        roster_root,
        members: u8::try_from(n).map_err(|_| Error::Synthesis)?,
        faults: u8::try_from(f).map_err(|_| Error::Synthesis)?,
        bitmap: padded,
        aggregate_key,
    };
    let mut sum = G1Projective::zero();
    let mut leaves = vec![AggregateLeafCircuit::new(
        0,
        context,
        None,
        Some(point_witness(sum.into_affine())),
        None,
    )?];
    for (seat, path) in paths.iter().enumerate().take(31) {
        let before = point_witness(sum.into_affine());
        let point = points
            .get(seat)
            .copied()
            .unwrap_or_else(G1Affine::generator);
        if selected(seat) {
            sum += point;
        }
        leaves.push(AggregateLeafCircuit::new(
            u32::try_from(seat).map_err(|_| Error::Synthesis)? + 1,
            context,
            Some(before),
            Some(point_witness(sum.into_affine())),
            Some(AggregateSeat {
                key: *keys.get(seat).unwrap_or(&[0; 48]),
                path: *path,
                point: point_witness(point),
            }),
        )?);
    }
    let result = sum.into_affine();
    let mut encoded = Vec::with_capacity(48);
    result
        .serialize_compressed(&mut encoded)
        .map_err(|_| Error::Synthesis)?;
    if result.is_zero() || encoded.as_slice() != aggregate_key {
        return Err(Error::Synthesis);
    }
    leaves.push(AggregateLeafCircuit::new(
        32,
        context,
        Some(point_witness(result)),
        None,
        None,
    )?);
    Ok(leaves)
}
