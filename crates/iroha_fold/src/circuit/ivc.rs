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
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Bit, GlueChip, U128, Uint, Word, ff::CanonicalS6};

use super::{
    FoldLanes, ScalarCell, transcript as t,
    verify::{accumulator_digest, assign_accumulator, assign_instance, assign_proof, verify_fold_or_start},
};
use crate::{
    ivc::{ChainSide, FrameWitness, HASH_BITS, TAG_IVC},
    relation::FoldShape,
};

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
    chips: &mut FoldLanes<'_, C>,
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
    chips: &mut FoldLanes<'_, C>,
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
    chips: &mut FoldLanes<'_, C>,
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
    chips: &mut FoldLanes<'_, C>,
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
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    value: &ScalarCell<C>,
) -> Result<Word<C::Base>, Error> {
    chips.glue.enforce_constant(region, value.s6.hi().word(), C::Base::ZERO)?;
    Ok(value.s6.lo().word().clone())
}

/// The constrained fold frame of one step: the step index, the base, whether
/// this is the side's first instance, the side's previous state, the other
/// side's bridge, the passthrough to output and the kept accumulator's digest.
#[derive(Clone, Debug)]
pub struct Frame<F: iroha_pasta::PastaField> {
    /// The step index.
    pub step: Word<F>,
    /// The segment base (a circuit constant).
    pub base: Word<F>,
    /// One on the side's first instance; the business checks its base rule there.
    pub first: Bit<F>,
    /// The side's previous state, authenticated by the passthrough check.
    pub state: Vec<Word<F>>,
    /// The other side's bridge values as native words.
    pub bridge: Vec<Word<F>>,
    /// The passthrough this step outputs.
    pub passthrough: Word<F>,
    /// The kept accumulator's digest after the step.
    pub digest: Word<F>,
}

/// The fold frame of a step of `side` (`IVC.md` section 4): checks that the
/// other side's previous instance passes through this side's previous hash
/// (`H(step - 2, base, state, stored)`), that the kept accumulator is the stored
/// one, folds the instance into it (or starts it), and reads the instance's
/// `h_out` (the passthrough to output) and bridge. The other side's frame
/// column is its relation's first instance column, `[passthrough, h_out,
/// bridge...]`. On the first instance of the side that starts the chain there is
/// no predecessor: the checks are skipped, the digest is zero and the
/// passthrough is `start_passthrough`, a circuit constant: the other side's base
/// hash `H(0, base, base_state, 0)`. It is never a witness, so a mid-chain
/// "restart" of this side forces the other side back to its first instance with
/// its base state and an absent accumulator (`IVC.md` section 5); no folded
/// instance can be dropped while its effects stay in a state.
///
/// # Errors
/// Layout errors, or a witness of another shape.
#[allow(clippy::too_many_arguments)]
pub fn fold_frame<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    side: &ChainSide,
    shape: &FoldShape,
    base: C::Base,
    start_passthrough: C::Base,
    state_len: usize,
    witness: Option<&FrameWitness<C>>,
) -> Result<Frame<C::Base>, Error> {
    if witness.is_some_and(|w| w.state.len() != state_len) || shape.instance_lengths.first().is_none_or(|len| *len < 2) {
        return Err(Error::Synthesis);
    }
    let known = |f: &dyn Fn(&FrameWitness<C>) -> C::Base| witness.map_or_else(Value::unknown, |w| Value::known(f(w)));
    let step = chips.glue.witness(region, known(&|w| C::Base::from(w.step)))?;
    let base = chips.glue.constant(region, base)?;
    let mut state = Vec::with_capacity(state_len);
    for i in 0..state_len {
        state.push(chips.glue.witness(region, known(&|w| w.state[i]))?);
    }
    let stored = chips.glue.witness(region, known(&|w| w.stored))?;
    let acc = assign_accumulator(chips, region, shape, witness.map(|w| &w.acc))?;
    let fresh = assign_instance(chips, region, shape, witness.map(|w| &w.fresh))?;
    let proof = assign_proof(chips, region, shape, witness.map(|w| &w.proof))?;

    let shifted = chips.glue.add_constant(region, &step, -C::Base::from(side.first_step))?;
    let first = chips.glue.is_zero(region, &shifted)?;
    let absent = chips.glue.is_zero(region, &stored)?;
    let no_predecessor = (side.first_step == 1).then(|| first.clone());

    let previous = chips.glue.add_constant(region, &step, -C::Base::from(2))?;
    let expected = ivc_hash(chips, region, &side.ivc_domain, &previous, &base, &state, &stored)?;
    let frame_column = &fresh.public[0];
    assert_hash_unless(chips, region, &frame_column[0], &expected, no_predecessor.as_ref())?;
    let digest = accumulator_digest(chips, region, &side.fold_domain, &acc)?;
    assert_equal_unless(chips, region, &digest, &stored, Some(&absent))?;
    let folded = verify_fold_or_start(chips, region, shape, &side.fold_domain, &acc, &fresh, &proof, &absent)?;
    let new_digest = accumulator_digest(chips, region, &side.fold_domain, &folded)?;
    let from_fresh = foreign_hash_word(chips, region, &frame_column[1])?;
    let mut bridge = Vec::with_capacity(frame_column.len() - 2);
    for value in &frame_column[2..] {
        bridge.push(foreign_bounded_word(chips, region, value)?);
    }
    let (digest, passthrough) = match &no_predecessor {
        Some(bit) => {
            let zero = chips.glue.constant(region, <C::Base as Field>::ZERO)?;
            let start = chips.glue.constant(region, start_passthrough)?;
            (chips.glue.select(region, bit, &zero, &new_digest)?, chips.glue.select(region, bit, &start, &from_fresh)?)
        }
        None => (new_digest, from_fresh),
    };
    Ok(Frame { step, base, first, state, bridge, passthrough, digest })
}

/// The frame column a step outputs: `[passthrough, h_out, bridge...]` with
/// `h_out = H(step, base, next, digest)`, as [`crate::ivc::step_public`].
///
/// # Errors
/// Layout errors.
pub fn frame_outputs<C: PastaCurve>(
    chips: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    side: &ChainSide,
    frame: &Frame<C::Base>,
    next: &[Word<C::Base>],
    bridge: &[Word<C::Base>],
) -> Result<Vec<Word<C::Base>>, Error> {
    let h = ivc_hash(chips, region, &side.ivc_domain, &frame.step, &frame.base, next, &frame.digest)?;
    let mut out = vec![frame.passthrough.clone(), h.word];
    out.extend(bridge.iter().cloned());
    Ok(out)
}
