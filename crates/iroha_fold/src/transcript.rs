//! RP57 Poseidon transcripts of the folding protocols.
//!
//! A transcript runs over the scalar field `F` of the commitment curve, the field
//! of the folded relation, so an in-circuit verifier on that field runs it
//! natively. It starts with a role tag and a 32-byte domain (scheme, relation,
//! operation kind and side, chosen by the caller). Curve points are absorbed as
//! their affine coordinates, each split into two 128-bit limbs, so the encoding is
//! injective on both Pasta curves (`(0, 0)` is the identity and is not on either
//! curve). Scalars are absorbed as field elements.

use iroha_pasta::{PastaAffine, PastaField, poseidon::{PoseidonField, Sponge}};

/// Role tag of a fresh instance's round-one transcript (`theta`, `r`).
pub const TAG_INSTANCE: u64 = u64::from_le_bytes(*b"kgwfins1");
/// Role tag of an accumulator built from a fresh instance (`beta`).
pub const TAG_ACCUMULATOR: u64 = u64::from_le_bytes(*b"kgwfacc1");
/// Role tag of one fold (`delta`, `alpha`, `gamma`).
pub const TAG_FOLD: u64 = u64::from_le_bytes(*b"kgwfold1");
/// Role tag of one claim-accumulator fold.
pub const TAG_CLAIM_FOLD: u64 = u64::from_le_bytes(*b"kgwfclm1");
/// Role tag of a claim accumulator built from a claim.
pub const TAG_CLAIM_ACCUMULATOR: u64 = u64::from_le_bytes(*b"kgwfcla1");

/// A Poseidon transcript over `F`.
#[derive(Clone, Debug)]
pub struct Transcript<F: PoseidonField> {
    sponge: Sponge<F>,
}

fn limbs<B: PastaField>(value: &B) -> [u128; 2] {
    let l = value.to_canonical_limbs();
    [
        u128::from(l[0]) | (u128::from(l[1]) << 64),
        u128::from(l[2]) | (u128::from(l[3]) << 64),
    ]
}

impl<F: PoseidonField> Transcript<F> {
    /// A transcript for role `tag` under `domain`.
    #[must_use]
    pub fn new(tag: u64, domain: &[u8; 32]) -> Self {
        let mut sponge = Sponge::new();
        sponge.update(&[F::from(tag)]);
        for chunk in domain.as_chunks::<16>().0 {
            sponge.update(&[F::from_u128(u128::from_le_bytes(*chunk))]);
        }
        Self { sponge }
    }

    /// Absorbs a count or length.
    pub fn absorb_u64(&mut self, value: u64) {
        self.sponge.update(&[F::from(value)]);
    }

    /// Absorbs field elements.
    pub fn absorb_scalars(&mut self, values: &[F]) {
        self.sponge.update(values);
    }

    /// Absorbs a curve point by its two coordinates, two 128-bit limbs each.
    pub fn absorb_point<A: PastaAffine>(&mut self, point: &A) {
        for coordinate in [point.x(), point.y()] {
            for limb in limbs(&coordinate) {
                self.sponge.update(&[F::from_u128(limb)]);
            }
        }
    }

    /// A full-width challenge.
    pub fn challenge(&mut self) -> F {
        self.sponge.squeeze()
    }

    /// A challenge below `2^128` (used as a scalar on commitments).
    pub fn challenge_128(&mut self) -> F {
        let value = self.sponge.squeeze();
        F::from_u128(limbs(&value)[0])
    }
}
