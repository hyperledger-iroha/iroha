//! The fold transcripts on the duplex lane, bit for bit
//! [`crate::transcript::Transcript`].
//!
//! The lane holds one transcript at a time: [`start`] requires a clear lane and
//! every transcript ends with a clearing squeeze ([`finish`],
//! [`challenge_last`] or [`challenge_128_last`]). Squeezes map to scalars like
//! `iroha_plonk_recursion::codec::map_challenge`: the squeezed word is
//! canonically decomposed, then `Fp` words keep their integer in `Fq` and `Fq`
//! words subtract `p` when at least `p`.

use ff::PrimeField;
use iroha_pasta::{Fp, PastaCurve};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{U128, Word, ecc::AssignedPoint, ff::CanonicalS6};

use super::{FoldLanes, ScalarCell, scalar_modulus};

/// Starts a transcript for role `tag` under `domain` on a clear lane.
///
/// # Errors
/// [`Error::Synthesis`] when another transcript is live on the lane.
pub fn start<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    tag: u64,
    domain: &[u8; 32],
) -> Result<(), Error> {
    if !chips.duplex.is_clear() {
        return Err(Error::Synthesis);
    }
    chips.duplex.absorb_constant(C::Base::from(tag));
    for chunk in domain.as_chunks::<16>().0 {
        chips
            .duplex
            .absorb_constant(C::Base::from_u128(u128::from_le_bytes(*chunk)));
    }
    Ok(())
}

/// Absorbs a count or length (a circuit constant).
pub fn absorb_u64<C: PastaCurve>(chips: &mut FoldLanes<'_, C>, value: u64) {
    chips.duplex.absorb_constant(C::Base::from(value));
}

/// Absorbs a base-field word (a digest).
pub fn absorb_word<C: PastaCurve>(chips: &mut FoldLanes<'_, C>, word: &Word<C::Base>) {
    chips.duplex.absorb(word);
}

/// Absorbs a scalar of `C`.
///
/// # Errors
/// Layout errors.
pub fn absorb_scalar<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    scalar: &ScalarCell<C>,
) -> Result<(), Error> {
    let words = chips.scalar_words(region, scalar)?;
    chips.duplex.absorb_words(&words);
    Ok(())
}

/// Absorbs scalars of `C`.
///
/// # Errors
/// Layout errors.
pub fn absorb_scalars<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    scalars: &[ScalarCell<C>],
) -> Result<(), Error> {
    for scalar in scalars {
        absorb_scalar(chips, region, scalar)?;
    }
    Ok(())
}

/// Absorbs a point of `C` by its coordinate cells.
pub fn absorb_point<C: PastaCurve>(chips: &mut FoldLanes<'_, C>, point: &AssignedPoint<C::Base>) {
    chips.duplex.absorb(point.x());
    chips.duplex.absorb(point.y());
}

fn squeeze<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    last: bool,
) -> Result<Word<C::Base>, Error> {
    if last {
        chips.duplex.squeeze_and_clear(region)
    } else {
        chips.duplex.squeeze(region)
    }
}

/// Ends the transcript with a digest squeeze.
///
/// # Errors
/// Layout errors.
pub fn finish<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
) -> Result<Word<C::Base>, Error> {
    squeeze(chips, region, true)
}

/// The canonical S6 decomposition of a squeezed word.
fn decompose<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    word: &Word<C::Base>,
) -> Result<CanonicalS6<C::Base>, Error> {
    let mut uint = chips.uint();
    CanonicalS6::from_native_word(&mut uint, region, word)
}

fn map<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    word: &Word<C::Base>,
) -> Result<ScalarCell<C>, Error> {
    let canonical = decompose(chips, region, word)?;
    let mut uint = chips.uint();
    let scalar = if <C::Base as PrimeField>::MODULUS == <Fp as PrimeField>::MODULUS {
        // w < p < q: the canonical Fp integer is a canonical Fq integer.
        canonical.with_modulus(&mut uint, region, scalar_modulus::<C>())?
    } else {
        canonical.reduce_fq_to_fp(&mut uint, region)?
    };
    chips.import(region, scalar)
}

fn map_128<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    word: &Word<C::Base>,
) -> Result<(U128<C::Base>, ScalarCell<C>), Error> {
    let canonical = decompose(chips, region, word)?;
    let lo = canonical.lo().clone();
    let mut uint = chips.uint();
    let zero = uint.constant::<127>(region, 0)?;
    let scalar = CanonicalS6::from_limbs(&mut uint, region, scalar_modulus::<C>(), &lo, &zero)?;
    Ok((lo, chips.import(region, scalar)?))
}

/// A full-width challenge; the transcript continues.
///
/// # Errors
/// Layout errors.
pub fn challenge<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
) -> Result<ScalarCell<C>, Error> {
    let word = squeeze(chips, region, false)?;
    map(chips, region, &word)
}

/// A full-width challenge that ends the transcript.
///
/// # Errors
/// Layout errors.
pub fn challenge_last<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
) -> Result<ScalarCell<C>, Error> {
    let word = squeeze(chips, region, true)?;
    map(chips, region, &word)
}

/// A 128-bit challenge; the transcript continues. Its low limb (for the group
/// multiplication) and the scalar, as [`crate::transcript::Transcript::challenge_128`].
///
/// # Errors
/// Layout errors.
pub fn challenge_128<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
) -> Result<(U128<C::Base>, ScalarCell<C>), Error> {
    let word = squeeze(chips, region, false)?;
    map_128(chips, region, &word)
}

/// A 128-bit challenge that ends the transcript: its low limb (for the group
/// multiplication) and the scalar.
///
/// # Errors
/// Layout errors.
pub fn challenge_128_last<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
) -> Result<(U128<C::Base>, ScalarCell<C>), Error> {
    let word = squeeze(chips, region, true)?;
    map_128(chips, region, &word)
}
