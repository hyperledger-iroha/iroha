//! In-circuit pieces of the two-curve IVC (output `IVC.md` section 4): the
//! truncated IVC hash, the passthrough check against a public input of the
//! other side's instance, and foreign bounded values as native words.
//!
//! A public input of the other side's instance is a foreign scalar here
//! ([`ScalarCell`], canonical S6 limbs). A truncated hash below `2^250` and a
//! bounded bridge value below `2^128` are the same integers in both fields, so
//! comparing them is comparing limbs, and recomposing them is exact.

use ff::{Field, PrimeField};
use iroha_pasta::PastaCurve;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, U128, Uint, Word, ff::CanonicalS6};

use super::{FoldVerifierChips, ScalarCell, transcript as t};
use crate::ivc::{HASH_BITS, TAG_IVC};

/// Bits of the high limb of a truncated hash.
const HIGH_BITS: usize = HASH_BITS as usize - 128;

/// A truncated IVC hash: its native word and its two limbs.
#[derive(Clone, Debug)]
pub struct HashCells<F: iroha_pasta::PastaField> {
    /// `lo + 2^128 hi`, below `2^250`.
    pub word: Word<F>,
    /// The low 128 bits.
    pub lo: U128<F>,
    /// Bits 128 to 249.
    pub hi: Uint<F, HIGH_BITS>,
}

fn two_pow<F: PrimeField>(bits: u32) -> F {
    F::from(2u64).pow_vartime([u64::from(bits)])
}

/// The IVC hash `H(kgwfivc1, domain, step, base, state, digest)`, truncated to
/// 250 bits, as [`crate::ivc::ivc_hash`].
///
/// # Errors
/// Layout errors, or another transcript live on the lane.
pub fn ivc_hash<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    step: &Word<C::Base>,
    base: &Word<C::Base>,
    state: &[Word<C::Base>],
    digest: &Word<C::Base>,
) -> Result<HashCells<C::Base>, Error> {
    t::start(chips, TAG_IVC, domain)?;
    t::absorb_word(chips, step);
    t::absorb_word(chips, base);
    for word in state {
        t::absorb_word(chips, word);
    }
    t::absorb_word(chips, digest);
    let word = t::finish(chips, region)?;
    let mut uint = chips.uint();
    let canonical = CanonicalS6::from_native_word(&mut uint, region, &word)?;
    let high = canonical.hi().value();
    let hi = uint.assign::<HIGH_BITS>(region, high.map(|h| h & ((1 << HIGH_BITS) - 1)))?;
    let top = uint.assign::<5>(region, high.map(|h| h >> HIGH_BITS))?;
    let recomposed = uint.glue().linear(
        region,
        &[(C::Base::ONE, hi.word()), (two_pow(HIGH_BITS as u32), top.word())],
        C::Base::ZERO,
    )?;
    GlueChip::assert_equal(region, &recomposed, canonical.hi().word())?;
    let lo = canonical.lo().clone();
    let word = uint
        .glue()
        .linear(region, &[(C::Base::ONE, lo.word()), (two_pow(128), hi.word())], C::Base::ZERO)?;
    Ok(HashCells { word, lo, hi })
}

/// Constrains `a = b`, or nothing where `skip` is one.
///
/// # Errors
/// Layout errors.
pub fn assert_equal_unless<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    a: &Word<C::Base>,
    b: &Word<C::Base>,
    skip: Option<&Bit<C::Base>>,
) -> Result<(), Error> {
    match skip {
        None => GlueChip::assert_equal(region, a, b),
        Some(bit) => {
            let difference = chips.glue.sub(region, a, b)?;
            let keep = chips.glue.not(region, bit)?;
            let product = chips.glue.mul(region, keep.word(), &difference)?;
            chips.glue.enforce_constant(region, &product, C::Base::ZERO)
        }
    }
}

/// Constrains the foreign scalar `value` (a public input of the other side's
/// instance) to the integer of `hash`, unless `skip` is one.
///
/// # Errors
/// Layout errors.
pub fn assert_hash_unless<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    value: &ScalarCell<C>,
    hash: &HashCells<C::Base>,
    skip: Option<&Bit<C::Base>>,
) -> Result<(), Error> {
    let (lo, hi) = (value.s6.lo().word().clone(), value.s6.hi().word().clone());
    assert_equal_unless(chips, region, &lo, hash.lo.word(), skip)?;
    assert_equal_unless(chips, region, &hi, hash.hi.word(), skip)
}

/// The native word of a foreign scalar below `2^250` (a passthrough hash).
///
/// # Errors
/// Layout errors; a larger integer is unsatisfiable.
pub fn foreign_hash_word<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    value: &ScalarCell<C>,
) -> Result<Word<C::Base>, Error> {
    let hi = chips.uint().range_check::<HIGH_BITS>(region, value.s6.hi().word())?;
    chips.glue.linear(
        region,
        &[(C::Base::ONE, value.s6.lo().word()), (two_pow(128), hi.word())],
        C::Base::ZERO,
    )
}

/// The native word of a foreign scalar below `2^128` (a bridge value).
///
/// # Errors
/// Layout errors; a larger integer is unsatisfiable.
pub fn foreign_bounded_word<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    value: &ScalarCell<C>,
) -> Result<Word<C::Base>, Error> {
    chips.glue.enforce_constant(region, value.s6.hi().word(), C::Base::ZERO)?;
    Ok(value.s6.lo().word().clone())
}
