//! RP57 Poseidon transcripts of the folding protocols.
//!
//! The transcript of instances committed on a curve `C` runs over `C::Base`,
//! the field of the circuit that folds them (output `IVC.md` section 3): an A
//! relation (over `Fp`, committed on Vesta) is folded by a Q circuit over `Fq`,
//! and a Q relation (over `Fq`, committed on Pallas) by an A circuit over `Fp`.
//! The encodings are those of the PIPA-R base-field transcript, so the circuit
//! absorbs every value natively:
//!
//! - a point as its affine coordinates `(x, y)`, the identity as `(0, 0)`
//!   (no point of either curve has `x = 0`);
//! - a Pallas-relation scalar (`Fq`, in an `Fp` transcript) as its two S6 limbs
//!   `lo < 2^128` and `hi < 2^127`; a Vesta-relation scalar (`Fp`, in an `Fq`
//!   transcript) as the one `Fq` element with the same integer;
//! - a full challenge maps the squeezed word `w` to the scalar field with
//!   [`PastaField::from_raw_reduced`] (`Fp` to `Fq` keeps the integer since
//!   `p < q`; `Fq` to `Fp` subtracts `p` when `w >= p`), as
//!   `iroha_plonk_recursion::codec::map_challenge` does; a 128-bit challenge is
//!   the low 128 bits of `w`.
//!
//! Every transcript starts with a role tag and a 32-byte domain (scheme,
//! relation, side and kind, chosen by the caller), each 16-byte half absorbed as
//! one word.

use core::marker::PhantomData;

use ff::PrimeField;
use iroha_pasta::{Fp, PastaAffine, PastaCurve, PastaField, poseidon::Sponge};

/// Role tag of a fresh instance's transcript (`theta`, `r`, its digest).
pub const TAG_INSTANCE: u64 = u64::from_le_bytes(*b"kgwfins1");
/// Role tag of an accumulator digest.
pub const TAG_ACCUMULATOR_DIGEST: u64 = u64::from_le_bytes(*b"kgwfacd1");
/// Role tag of an accumulator started from a fresh instance (`beta`).
pub const TAG_ACCUMULATOR: u64 = u64::from_le_bytes(*b"kgwfacc1");
/// Role tag of one fold (`delta`, `alpha`, `gamma`).
pub const TAG_FOLD: u64 = u64::from_le_bytes(*b"kgwfold1");
/// Role tag of a claim accumulator started from a claim (`beta`).
pub const TAG_CLAIM_ACCUMULATOR: u64 = u64::from_le_bytes(*b"kgwfcla1");
/// Role tag of one claim-accumulator fold.
pub const TAG_CLAIM_FOLD: u64 = u64::from_le_bytes(*b"kgwfclm1");
/// Role tag of a claim-accumulator digest.
pub const TAG_CLAIM_DIGEST: u64 = u64::from_le_bytes(*b"kgwfcld1");

/// Whether a scalar of `C` is absorbed as one base-field word (Vesta: `Fp < Fq`)
/// rather than as two S6 limbs (Pallas: `Fq > Fp`).
#[must_use]
pub fn scalar_is_one_word<C: PastaCurve>() -> bool {
    <C::ScalarExt as PrimeField>::MODULUS == <Fp as PrimeField>::MODULUS
}

/// The low and high S6 limbs of a canonical field element.
#[must_use]
pub fn s6_limbs<B: PastaField>(value: &B) -> [u128; 2] {
    let l = value.to_canonical_limbs();
    [
        u128::from(l[0]) | (u128::from(l[1]) << 64),
        u128::from(l[2]) | (u128::from(l[3]) << 64),
    ]
}

/// The base-field words that absorb the scalar `value` of `C`.
#[must_use]
pub fn scalar_words<C: PastaCurve>(value: &C::ScalarExt) -> Vec<C::Base> {
    if scalar_is_one_word::<C>() {
        vec![C::Base::from_raw_reduced(value.to_canonical_limbs())]
    } else {
        s6_limbs(value).iter().map(|limb| C::Base::from_u128(*limb)).collect()
    }
}

/// The scalar of the squeezed word `word` (full-width challenge map).
#[must_use]
pub fn map_challenge<C: PastaCurve>(word: &C::Base) -> C::ScalarExt {
    C::ScalarExt::from_raw_reduced(word.to_canonical_limbs())
}

/// The 128-bit challenge of the squeezed word `word`.
#[must_use]
pub fn map_challenge_128<C: PastaCurve>(word: &C::Base) -> C::ScalarExt {
    C::ScalarExt::from_u128(s6_limbs(word)[0])
}

/// A base-field Poseidon transcript over instances committed on `C`.
#[derive(Clone, Debug)]
pub struct Transcript<C: PastaCurve> {
    sponge: Sponge<C::Base>,
    marker: PhantomData<C>,
}

impl<C: PastaCurve> Transcript<C> {
    /// A transcript for role `tag` under `domain`.
    #[must_use]
    pub fn new(tag: u64, domain: &[u8; 32]) -> Self {
        let mut sponge = Sponge::new();
        sponge.update(&[C::Base::from(tag)]);
        for chunk in domain.as_chunks::<16>().0 {
            sponge.update(&[C::Base::from_u128(u128::from_le_bytes(*chunk))]);
        }
        Self { sponge, marker: PhantomData }
    }

    /// Absorbs a count or length.
    pub fn absorb_u64(&mut self, value: u64) {
        self.sponge.update(&[C::Base::from(value)]);
    }

    /// Absorbs base-field words (digests).
    pub fn absorb_base(&mut self, values: &[C::Base]) {
        self.sponge.update(values);
    }

    /// Absorbs scalars of `C`.
    pub fn absorb_scalars(&mut self, values: &[C::ScalarExt]) {
        for value in values {
            self.sponge.update(&scalar_words::<C>(value));
        }
    }

    /// Absorbs a point of `C` by its coordinates (`(0, 0)` for the identity).
    pub fn absorb_point(&mut self, point: &C::AffineExt) {
        self.sponge.update(&[point.x(), point.y()]);
    }

    /// Squeezes one base-field word (a digest).
    pub fn digest(&mut self) -> C::Base {
        self.sponge.squeeze()
    }

    /// A full-width challenge.
    pub fn challenge(&mut self) -> C::ScalarExt {
        map_challenge::<C>(&self.sponge.squeeze())
    }

    /// A challenge below `2^128` (used as a scalar on commitments).
    pub fn challenge_128(&mut self) -> C::ScalarExt {
        map_challenge_128::<C>(&self.sponge.squeeze())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;
    use iroha_pasta::{Ep, Eq, Fq};

    #[test]
    fn scalar_encodings_follow_the_pipa_r_base_transcript() {
        assert!(!scalar_is_one_word::<Ep>());
        assert!(scalar_is_one_word::<Eq>());
        let s = -Fq::ONE;
        let [lo, hi] = s6_limbs(&s);
        assert!(hi < 1 << 127);
        assert_eq!(scalar_words::<Ep>(&s), vec![Fp::from_u128(lo), Fp::from_u128(hi)]);
        let t = -Fp::ONE;
        assert_eq!(scalar_words::<Eq>(&t), vec![Fq::from_raw_reduced(t.to_canonical_limbs())]);
    }

    #[test]
    fn challenge_maps_keep_or_reduce_the_integer() {
        // Fp -> Fq keeps every integer.
        let w = -Fp::ONE;
        assert_eq!(map_challenge::<Ep>(&w).to_repr(), w.to_repr());
        assert_eq!(map_challenge_128::<Ep>(&w), Fq::from_u128(s6_limbs(&w)[0]));
        // Fq -> Fp subtracts p exactly when w >= p.
        let w = -Fq::ONE;
        assert_eq!(map_challenge::<Eq>(&w), Fp::from_raw_reduced(w.to_canonical_limbs()));
        assert_eq!(map_challenge::<Eq>(&Fq::from(7u64)), Fp::from(7u64));
    }
}
