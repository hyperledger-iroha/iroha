//! Byte-exact transcript helpers shared by the parked RNS-native Bulletproof relations.
//!
//! Each relation passes its own challenge and codec domain, so Fiat-Shamir transcripts and codec
//! digests remain domain-separated per relation. Only the identical hashing, cursor and
//! scalar/point absorption bodies live here.
use crate::{
    generalized_bulletproof::GeneralizedBulletproofErrorV1,
    vega::{VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, sponge::Keccak256},
};

const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const SCALAR_BYTES_V1: usize = 32;
const MAX_CHALLENGE_ATTEMPTS_V1: u8 = 128;

/// Keccak-256 of `bytes`.
pub(super) fn hash_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(bytes);
    hash.finalize()
}

/// Relation codec digest `Keccak256(domain || [version] || bytes)`.
pub(super) fn codec_digest_v1(domain: &[u8], version: u8, bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[version]);
    hash.update(bytes);
    hash.finalize()
}

/// Derive the next nonzero challenge under the relation's challenge `domain`.
///
/// The accepted challenge, its ordinal and rejection attempt are absorbed into `state`, and
/// `ordinal` advances by one.
pub(super) fn derive_challenge_v1(
    domain: &[u8],
    state: &mut Vec<u8>,
    ordinal: &mut u32,
) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
    for attempt in 0_u8..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut input = Vec::with_capacity(domain.len() + state.len() + 6);
        input.extend_from_slice(domain);
        input.extend_from_slice(state);
        input.extend_from_slice(&ordinal.to_be_bytes());
        input.push(attempt);
        let mut low = input.clone();
        low.push(0);
        input.push(1);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&hash_v1(&low));
        wide[32..].copy_from_slice(&hash_v1(&input));
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            state.push(2);
            state.extend_from_slice(&ordinal.to_be_bytes());
            state.push(attempt);
            state.extend_from_slice(&challenge.to_le_bytes());
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            return Ok(challenge);
        }
    }
    Err(GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)
}

/// Borrow the next `count` proof bytes at `*cursor` and advance the cursor.
pub(super) fn take_v1<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    count: usize,
) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
    let end = cursor
        .checked_add(count)
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
            actual: bytes.len(),
            expected: end,
        })?;
    *cursor = end;
    Ok(value)
}

/// Decode one canonical scalar from `taken` and absorb it into `state` under tag 0.
pub(super) fn absorb_scalar_v1(
    state: &mut Vec<u8>,
    taken: &[u8],
) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
    let encoded: [u8; SCALAR_BYTES_V1] = taken
        .try_into()
        .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
    let scalar = Scalar::from_le_bytes_exact(encoded)
        .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
    state.push(0);
    state.extend_from_slice(&encoded);
    Ok(scalar)
}

/// Decode one non-identity point from `taken` and absorb it into `state` under tag 1.
pub(super) fn absorb_point_v1(
    state: &mut Vec<u8>,
    taken: &[u8],
) -> Result<Point, GeneralizedBulletproofErrorV1> {
    let encoded: [u8; POINT_BYTES_V1] = taken
        .try_into()
        .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
    let point = Point::from_non_identity_wire_bytes_exact(&encoded)
        .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
    state.push(1);
    state.extend_from_slice(&encoded);
    Ok(point)
}
