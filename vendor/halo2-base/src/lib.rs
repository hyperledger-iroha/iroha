//! Base library to build Halo2 circuits.
#![allow(incomplete_features)]
#![deny(clippy::perf)]
#![allow(clippy::too_many_arguments)]
#![warn(clippy::default_numeric_fallback)]
#![warn(missing_docs)]

use std::{
    fmt,
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

use getset::CopyGetters;
use itertools::Itertools;
// Different memory allocator options:
#[cfg(feature = "jemallocator")]
use jemallocator::Jemalloc;
#[cfg(feature = "jemallocator")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

// mimalloc is fastest on Mac M2
#[cfg(feature = "mimalloc")]
use mimalloc::MiMalloc;
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(all(feature = "halo2-pse", feature = "halo2-axiom"))]
compile_error!("Cannot enable both \"halo2-pse\" and \"halo2-axiom\" features.");
#[cfg(not(any(feature = "halo2-pse", feature = "halo2-axiom")))]
compile_error!("Must enable exactly one of \"halo2-pse\" or \"halo2-axiom\".");
#[cfg(all(feature = "halo2-pse", feature = "cuda"))]
compile_error!("The \"cuda\" feature is only supported with \"halo2-axiom\".");

// use gates::flex_gate::MAX_PHASE;
#[cfg(feature = "halo2-pse")]
pub use halo2_proofs;
#[cfg(all(feature = "halo2-axiom", not(feature = "cuda")))]
pub use halo2_proofs_axiom as halo2_proofs;
#[cfg(all(feature = "halo2-axiom", feature = "cuda"))]
pub use halo2_proofs_axiom_gpu as halo2_proofs;

use halo2_proofs::halo2curves::ff;
use halo2_proofs::plonk::Assigned;
use utils::ScalarField;
use virtual_region::copy_constraints::SharedCopyConstraintManager;

/// Module that contains the main API for creating and working with circuits.
/// `gates` is misleading because we currently only use one custom gate throughout.
pub mod gates;
/// Module for the Poseidon hash function.
pub mod poseidon;
/// Module for SafeType which enforce value range and realted functions.
pub mod safe_types;
/// Utility functions for converting between different types of field elements.
pub mod utils;
pub mod virtual_region;

/// Constant representing whether the Layouter calls `synthesize` once just to get region shape.
#[cfg(feature = "halo2-axiom")]
pub const SKIP_FIRST_PASS: bool = false;
/// Constant representing whether the Layouter calls `synthesize` once just to get region shape.
#[cfg(feature = "halo2-pse")]
pub const SKIP_FIRST_PASS: bool = true;

/// Canonical type identifier for raw cells assigned outside halo2-base virtual regions.
pub const EXTERNAL_CELL_TYPE_ID: &str = "halo2-base:External Raw Halo2 Cell";
/// Canonical type identifier for first-phase halo2-base virtual regions.
pub const FIRST_PHASE_CELL_TYPE_ID: &str = "halo2-base:SinglePhaseCoreManager:FirstPhase";
/// Canonical type identifier for second-phase halo2-base virtual regions.
pub const SECOND_PHASE_CELL_TYPE_ID: &str = "halo2-base:SinglePhaseCoreManager:SecondPhase";
/// Canonical type identifier for third-phase halo2-base virtual regions.
pub const THIRD_PHASE_CELL_TYPE_ID: &str = "halo2-base:SinglePhaseCoreManager:ThirdPhase";

/// Convenience Enum which abstracts the scenarios under a value is added to an advice column.
#[derive(Clone, Copy, Debug)]
pub enum QuantumCell<F: ScalarField> {
    /// An [AssignedValue] already existing in the advice column (e.g., a witness value that was already assigned in a previous cell in the column).
    /// * Assigns a new cell into the advice column with value equal to the value of a.
    /// * Imposes an equality constraint between the new cell and the cell of a so the Verifier guarantees that these two cells are always equal.
    Existing(AssignedValue<F>),
    // This is a guard for witness values assigned after pkey generation. We do not use `Value` api anymore.
    /// A non-existing witness [ScalarField] value (e.g. private input) to add to an advice column.
    Witness(F),
    /// A non-existing witness [ScalarField] marked as a fraction for optimization in batch inversion later.
    WitnessFraction(Assigned<F>),
    /// A known constant value added as a witness value to the advice column and added to the "Fixed" column during circuit creation time.
    /// * Visible to both the Prover and the Verifier.
    /// * Imposes an equality constraint between the two corresponding cells in the advice and fixed columns.
    Constant(F),
}

impl<F: ScalarField> From<AssignedValue<F>> for QuantumCell<F> {
    /// Converts an [`AssignedValue<F>`] into a [`QuantumCell<F>`] of enum variant `Existing`.
    fn from(a: AssignedValue<F>) -> Self {
        Self::Existing(a)
    }
}

impl<F: ScalarField> QuantumCell<F> {
    /// Returns an immutable reference to the underlying [ScalarField] value of a [`QuantumCell<F>`].
    ///
    /// Panics if the [`QuantumCell<F>`] is of type `WitnessFraction`.
    pub fn value(&self) -> &F {
        match self {
            Self::Existing(a) => a.value(),
            Self::Witness(a) => a,
            Self::WitnessFraction(_) => {
                panic!("Trying to get value of a fraction before batch inversion")
            }
            Self::Constant(a) => a,
        }
    }
}

/// Unique tag for a context across all virtual regions.
/// In the form `(type_id, context_id)`, where `type_id` is one of the four
/// canonical halo2-base cell identifiers and `context_id` is local to that
/// virtual region.
pub type ContextTag = (&'static str, usize);

/// Pointer to the position of a cell at `offset` in an advice column within a [Context] of `context_id`.
///
/// The packed word orders its fields as `(type_id, context_id, offset)`, so its
/// derived equality and ordering exactly match the former tuple-shaped storage.
/// Its low bit is always set, allowing `Option<ContextCell>` to share the same
/// eight-byte layout.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContextCell(NonZeroU64);

impl ContextCell {
    const CONTEXT_BITS: u32 = 29;
    const OFFSET_BITS: u32 = 32;
    const OFFSET_SHIFT: u32 = 1;
    const CONTEXT_SHIFT: u32 = Self::OFFSET_SHIFT + Self::OFFSET_BITS;
    const TYPE_SHIFT: u32 = Self::CONTEXT_SHIFT + Self::CONTEXT_BITS;
    const CONTEXT_MASK: u64 = (1_u64 << Self::CONTEXT_BITS) - 1;
    const OFFSET_MASK: u64 = u32::MAX as u64;

    /// Creates a new [ContextCell] with the given `type_id`, `context_id`, and `offset`.
    ///
    /// `type_id` must be one of the four canonical halo2-base cell identifiers.
    /// Context identifiers use 29 bits and offsets use 32 bits; exceeding either
    /// range is rejected instead of aliasing a different virtual cell.
    pub fn new(type_id: &'static str, context_id: usize, offset: usize) -> Self {
        let type_code = match type_id {
            EXTERNAL_CELL_TYPE_ID => 0_u64,
            FIRST_PHASE_CELL_TYPE_ID => 1,
            SECOND_PHASE_CELL_TYPE_ID => 2,
            THIRD_PHASE_CELL_TYPE_ID => 3,
            _ => panic!("unsupported ContextCell type_id: {type_id}"),
        };
        assert!(
            context_id <= Self::CONTEXT_MASK as usize,
            "ContextCell context_id exceeds the compact 29-bit range"
        );
        let offset = u32::try_from(offset)
            .expect("ContextCell offset exceeds the compact u32 range") as u64;
        let packed = (type_code << Self::TYPE_SHIFT)
            | ((context_id as u64) << Self::CONTEXT_SHIFT)
            | (offset << Self::OFFSET_SHIFT)
            | 1;
        Self(NonZeroU64::new(packed).expect("ContextCell niche bit must be set"))
    }

    /// Returns the canonical identifier of the virtual region that owns this cell.
    pub const fn type_id(&self) -> &'static str {
        match self.0.get() >> Self::TYPE_SHIFT {
            0 => EXTERNAL_CELL_TYPE_ID,
            1 => FIRST_PHASE_CELL_TYPE_ID,
            2 => SECOND_PHASE_CELL_TYPE_ID,
            3 => THIRD_PHASE_CELL_TYPE_ID,
            _ => panic!("invalid packed ContextCell type code"),
        }
    }

    /// Returns the identifier of the [Context] that this cell belongs to.
    pub const fn context_id(&self) -> usize {
        ((self.0.get() >> Self::CONTEXT_SHIFT) & Self::CONTEXT_MASK) as usize
    }

    /// Returns the relative offset of the cell within this [Context] advice column.
    pub const fn offset(&self) -> usize {
        ((self.0.get() >> Self::OFFSET_SHIFT) & Self::OFFSET_MASK) as usize
    }
}

impl fmt::Debug for ContextCell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextCell")
            .field("type_id", &self.type_id())
            .field("context_id", &self.context_id())
            .field("offset", &self.offset())
            .finish()
    }
}

impl Hash for ContextCell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Keep the former derived `usize` hash sequence so compact storage does
        // not perturb hash-table behavior for any representable cell.
        self.type_id().hash(state);
        self.context_id().hash(state);
        self.offset().hash(state);
    }
}

#[cfg(test)]
mod context_cell_tests {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash as _, Hasher as _},
        mem::{align_of, size_of},
    };

    use super::{
        AssignedValue, ContextCell, EXTERNAL_CELL_TYPE_ID, FIRST_PHASE_CELL_TYPE_ID,
        SECOND_PHASE_CELL_TYPE_ID, THIRD_PHASE_CELL_TYPE_ID,
    };
    use crate::halo2_proofs::halo2curves::bn256::Fr;

    fn context_cell_hash(cell: ContextCell) -> u64 {
        let mut hasher = DefaultHasher::new();
        cell.hash(&mut hasher);
        hasher.finish()
    }

    fn legacy_hash(type_id: &'static str, context_id: usize, offset: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        type_id.hash(&mut hasher);
        context_id.hash(&mut hasher);
        offset.hash(&mut hasher);
        hasher.finish()
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn context_cell_has_compact_64_bit_layout() {
        assert_eq!(size_of::<ContextCell>(), 8);
        assert_eq!(align_of::<ContextCell>(), 8);
        assert_eq!(size_of::<Option<ContextCell>>(), 8);
        assert_eq!(size_of::<(ContextCell, ContextCell)>(), 16);
        assert_eq!(size_of::<AssignedValue<Fr>>(), 80);
    }

    #[test]
    fn context_cell_preserves_legacy_order_and_hash_semantics() {
        let fields = [
            (EXTERNAL_CELL_TYPE_ID, 0, 0),
            (EXTERNAL_CELL_TYPE_ID, 0, 9),
            (EXTERNAL_CELL_TYPE_ID, 4, 0),
            (FIRST_PHASE_CELL_TYPE_ID, 0, 0),
            (SECOND_PHASE_CELL_TYPE_ID, 0, 0),
            (THIRD_PHASE_CELL_TYPE_ID, 0, 0),
        ];
        for left in fields {
            let left_cell = ContextCell::new(left.0, left.1, left.2);
            assert_eq!(left_cell.type_id(), left.0);
            assert_eq!(left_cell.context_id(), left.1);
            assert_eq!(left_cell.offset(), left.2);
            assert_eq!(
                format!("{left_cell:?}"),
                format!(
                    "ContextCell {{ type_id: {:?}, context_id: {}, offset: {} }}",
                    left.0, left.1, left.2
                )
            );
            assert_eq!(
                context_cell_hash(left_cell),
                legacy_hash(left.0, left.1, left.2)
            );
            for right in fields {
                let right_cell = ContextCell::new(right.0, right.1, right.2);
                assert_eq!(left_cell.cmp(&right_cell), left.cmp(&right));
                assert_eq!(left_cell == right_cell, left == right);
            }
        }
    }

    #[test]
    fn context_cell_accepts_compact_maximum_and_returns_usize() {
        let maximum_context = (1_usize << 29) - 1;
        let maximum_offset = usize::try_from(u32::MAX).expect("u32 fits supported usize targets");
        let cell = ContextCell::new(
            THIRD_PHASE_CELL_TYPE_ID,
            maximum_context,
            maximum_offset,
        );
        assert_eq!(cell.type_id(), THIRD_PHASE_CELL_TYPE_ID);
        assert_eq!(cell.context_id(), maximum_context);
        assert_eq!(cell.offset(), maximum_offset);
    }

    #[test]
    #[should_panic(expected = "unsupported ContextCell type_id")]
    fn context_cell_rejects_unknown_type_id() {
        ContextCell::new("halo2-base:test:unknown", 0, 0);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "ContextCell context_id exceeds the compact 29-bit range")]
    fn context_cell_rejects_context_id_overflow() {
        ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 1_usize << 29, 0);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "ContextCell offset exceeds the compact u32 range")]
    fn context_cell_rejects_offset_overflow() {
        ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, u32::MAX as usize + 1);
    }
}

/// Pointer containing cell value and location within [Context].
///
/// Note: Performs a copy of the value, should only be used when you are about to assign the value again elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct AssignedValue<F: crate::ff::Field> {
    /// Value of the cell.
    pub value: Assigned<F>, // we don't use reference to avoid issues with lifetimes (you can't safely borrow from vector and push to it at the same time).
    // only needed during vkey, pkey gen to fetch the actual cell from the relevant context
    /// [ContextCell] pointer to the cell the value is assigned to within an advice column of a [Context].
    pub cell: Option<ContextCell>,
}

impl<F: ScalarField> AssignedValue<F> {
    /// Returns an immutable reference to the underlying value of an [`AssignedValue<F>`].
    ///
    /// Panics if the witness value is of type [Assigned::Rational] or [Assigned::Zero].
    pub fn value(&self) -> &F {
        match &self.value {
            Assigned::Trivial(a) => a,
            _ => unreachable!(), // if trying to fetch an un-evaluated fraction, you will have to do something manual
        }
    }

    /// Debug helper function for writing negative tests. This will change the **witness** value in `ctx` corresponding to `self.offset`.
    /// This assumes that `ctx` is the context that `self` lies in.
    pub fn debug_prank(&self, ctx: &mut Context<F>, prank_value: F) {
        ctx.replace_advice_with_trivial(self.cell.unwrap().offset(), prank_value);
    }
}

impl<F: ScalarField> AsRef<AssignedValue<F>> for AssignedValue<F> {
    fn as_ref(&self) -> &AssignedValue<F> {
        self
    }
}

const COMPACT_ADVICE_ZERO_BITS_PER_BYTE: usize = u8::BITS as usize;

fn compact_advice_position(index: usize) -> u32 {
    u32::try_from(index).expect("CompactAdvice position exceeds the compact u32 range")
}

fn compact_advice_zero_mask_len(advice_len: usize) -> usize {
    advice_len
        .checked_add(COMPACT_ADVICE_ZERO_BITS_PER_BYTE - 1)
        .expect("CompactAdvice zero-mask length overflow")
        / COMPACT_ADVICE_ZERO_BITS_PER_BYTE
}

fn compact_advice_zero_mask_location(index: usize) -> (usize, u8) {
    let byte = index / COMPACT_ADVICE_ZERO_BITS_PER_BYTE;
    let bit = 1_u8 << (index % COMPACT_ADVICE_ZERO_BITS_PER_BYTE);
    (byte, bit)
}

/// Memory-dense storage for the exact [`Assigned`] values in a [`Context`].
///
/// Numerators and trivial values share the dense field vector. `Zero` variants
/// are marked in a packed bit mask, while sorted sparse positions pair exactly
/// with rational denominators; an unmarked position is `Trivial`. Random
/// reconstruction is O(log R), where R is the number of rational values, and
/// sequential assignment uses a merge iterator. Rational values stay rational
/// until physical assignment so Halo2 retains its batch-inversion behavior
/// (including the specified `x / 0 -> 0` semantics).
#[derive(Clone)]
struct CompactAdvice<F: ScalarField> {
    numerators: Vec<F>,
    zero_mask: Vec<u8>,
    rational_positions: Vec<u32>,
    denominators: Vec<F>,
}

impl<F: ScalarField> Default for CompactAdvice<F> {
    fn default() -> Self {
        Self {
            numerators: Vec::new(),
            zero_mask: Vec::new(),
            rational_positions: Vec::new(),
            denominators: Vec::new(),
        }
    }
}

impl<F: ScalarField> fmt::Debug for CompactAdvice<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<F: ScalarField> CompactAdvice<F> {
    fn len(&self) -> usize {
        debug_assert_eq!(
            self.zero_mask.len(),
            compact_advice_zero_mask_len(self.numerators.len())
        );
        debug_assert_eq!(self.rational_positions.len(), self.denominators.len());
        self.numerators.len()
    }

    fn push(&mut self, value: Assigned<F>) {
        let index = self.numerators.len();
        let position = compact_advice_position(index);
        let new_len = index
            .checked_add(1)
            .expect("CompactAdvice advice length overflow");
        let required_mask_len = compact_advice_zero_mask_len(new_len);
        let (numerator, is_zero, denominator) = match value {
            Assigned::Zero => (F::ZERO, true, None),
            Assigned::Trivial(value) => (value, false, None),
            Assigned::Rational(numerator, denominator) => {
                (numerator, false, Some(denominator))
            }
        };

        // Complete every fallible allocation before changing logical lengths.
        self.numerators.reserve(1);
        if required_mask_len > self.zero_mask.len() {
            self.zero_mask.reserve(1);
        }
        if denominator.is_some() {
            self.rational_positions.reserve(1);
            self.denominators.reserve(1);
        }

        self.numerators.push(numerator);
        if required_mask_len > self.zero_mask.len() {
            self.zero_mask.push(0);
        }
        if is_zero {
            self.set_zero(index, true);
        }
        if let Some(denominator) = denominator {
            debug_assert!(
                self.rational_positions
                    .last()
                    .map_or(true, |last| *last < position),
                "CompactAdvice rational positions must be strictly increasing"
            );
            self.rational_positions.push(position);
            self.denominators.push(denominator);
        }
    }

    fn is_zero(&self, index: usize) -> bool {
        let (byte, bit) = compact_advice_zero_mask_location(index);
        let mask = *self
            .zero_mask
            .get(byte)
            .expect("CompactAdvice zero mask does not cover its advice position");
        mask & bit != 0
    }

    fn set_zero(&mut self, index: usize, is_zero: bool) {
        let (byte, bit) = compact_advice_zero_mask_location(index);
        let mask = self
            .zero_mask
            .get_mut(byte)
            .expect("CompactAdvice zero mask does not cover its advice position");
        if is_zero {
            *mask |= bit;
        } else {
            *mask &= !bit;
        }
    }

    fn get(&self, index: usize) -> Option<Assigned<F>> {
        let numerator = *self.numerators.get(index)?;
        let position = compact_advice_position(index);
        let is_zero = self.is_zero(index);
        if is_zero {
            debug_assert!(
                self.rational_positions.binary_search(&position).is_err(),
                "CompactAdvice position cannot be both Zero and Rational"
            );
            return Some(Assigned::Zero);
        }
        let rational_index = match self.rational_positions.last().copied() {
            None => return Some(Assigned::Trivial(numerator)),
            Some(last_position) if position > last_position => {
                return Some(Assigned::Trivial(numerator));
            }
            Some(last_position) if position == last_position => {
                self.rational_positions.len() - 1
            }
            Some(_) => match self.rational_positions.binary_search(&position) {
                Ok(index) => index,
                Err(_) => return Some(Assigned::Trivial(numerator)),
            },
        };
        Some(Assigned::Rational(
            numerator,
            *self
                .denominators
                .get(rational_index)
                .expect("CompactAdvice rational positions and denominators differ"),
        ))
    }

    fn last(&self) -> Option<Assigned<F>> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    fn iter(&self) -> CompactAdviceIter<'_, F> {
        CompactAdviceIter {
            advice: self,
            position: 0,
            rational_index: 0,
        }
    }

    fn rational_len(&self) -> usize {
        debug_assert_eq!(self.rational_positions.len(), self.denominators.len());
        self.rational_positions.len()
    }

    fn denominator_slots_len(&self) -> usize {
        self.denominators.len()
    }

    fn zero_mask_bytes_len(&self) -> usize {
        self.zero_mask.len()
    }

    fn rational_position_slots_len(&self) -> usize {
        self.rational_positions.len()
    }

    fn capacities(&self) -> [usize; 4] {
        [
            self.numerators.capacity(),
            self.zero_mask.capacity(),
            self.rational_positions.capacity(),
            self.denominators.capacity(),
        ]
    }

    fn replace_with_trivial(&mut self, index: usize, value: F) {
        assert!(
            index < self.numerators.len(),
            "advice replacement offset is out of bounds"
        );
        let position = compact_advice_position(index);
        if let Ok(rational_index) = self.rational_positions.binary_search(&position) {
            let old_len = self.denominators.len();
            debug_assert_eq!(old_len, self.rational_positions.len());
            self.denominators
                .copy_within(rational_index + 1..old_len, rational_index);
            self.denominators[old_len - 1] = F::ZERO;
            let _ = self.denominators.pop();
            self.rational_positions.remove(rational_index);
        }
        self.numerators[index] = value;
        self.set_zero(index, false);
    }

    fn wipe(&mut self) {
        self.numerators.fill(F::ZERO);
        self.zero_mask.fill(0);
        self.rational_positions.fill(0);
        self.denominators.fill(F::ZERO);
        self.rational_positions.clear();
        self.denominators.clear();
    }
}

/// Sequential exact decoder for [`CompactAdvice`].
struct CompactAdviceIter<'a, F: ScalarField> {
    advice: &'a CompactAdvice<F>,
    position: usize,
    rational_index: usize,
}

impl<F: ScalarField> Iterator for CompactAdviceIter<'_, F> {
    type Item = Assigned<F>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(&numerator) = self.advice.numerators.get(self.position) else {
            assert_eq!(
                self.rational_index,
                self.advice.rational_positions.len(),
                "CompactAdvice contains a rational position beyond its advice"
            );
            return None;
        };
        let position = compact_advice_position(self.position);
        let rational_here = match self
            .advice
            .rational_positions
            .get(self.rational_index)
            .copied()
        {
            Some(rational_position) if rational_position < position => {
                panic!("CompactAdvice rational positions are not strictly increasing")
            }
            Some(rational_position) if rational_position == position => true,
            _ => false,
        };
        let is_zero = self.advice.is_zero(self.position);
        assert!(
            !(is_zero && rational_here),
            "CompactAdvice position cannot be both Zero and Rational"
        );
        self.position += 1;
        Some(if is_zero {
            Assigned::Zero
        } else if rational_here {
            let denominator = *self
                .advice
                .denominators
                .get(self.rational_index)
                .expect("CompactAdvice rational positions and denominators differ");
            self.rational_index += 1;
            Assigned::Rational(numerator, denominator)
        } else {
            Assigned::Trivial(numerator)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.advice.numerators.len() - self.position;
        (remaining, Some(remaining))
    }
}

impl<F: ScalarField> ExactSizeIterator for CompactAdviceIter<'_, F> {}
impl<F: ScalarField> std::iter::FusedIterator for CompactAdviceIter<'_, F> {}

#[cfg(test)]
mod compact_advice_tests {
    use super::{
        compact_advice_position, compact_advice_zero_mask_len, AssignedValue, CompactAdvice,
        Context, FIRST_PHASE_CELL_TYPE_ID,
    };
    use crate::ff::Field as _;
    use crate::gates::flex_gate::threads::SinglePhaseCoreManager;
    use crate::halo2_proofs::{halo2curves::bn256::Fr, plonk::Assigned};
    use crate::virtual_region::copy_constraints::SharedCopyConstraintManager;

    fn assert_exact(actual: Assigned<Fr>, expected: Assigned<Fr>) {
        match (actual, expected) {
            (Assigned::Zero, Assigned::Zero) => {}
            (Assigned::Trivial(actual), Assigned::Trivial(expected)) => {
                assert_eq!(actual, expected);
            }
            (
                Assigned::Rational(actual_numerator, actual_denominator),
                Assigned::Rational(expected_numerator, expected_denominator),
            ) => {
                assert_eq!(actual_numerator, expected_numerator);
                assert_eq!(actual_denominator, expected_denominator);
            }
            (actual, expected) => panic!("Assigned variant changed: {actual:?} != {expected:?}"),
        }
    }

    fn fixture() -> CompactAdvice<Fr> {
        let mut advice = CompactAdvice::default();
        for value in fixture_values() {
            advice.push(value);
        }
        advice
    }

    fn fixture_values() -> Vec<Assigned<Fr>> {
        vec![
            Assigned::Zero,
            Assigned::Trivial(Fr::from(3)),
            Assigned::Rational(Fr::from(5), Fr::from(7)),
            Assigned::Rational(Fr::from(11), Fr::ZERO),
            Assigned::Trivial(Fr::from(13)),
            Assigned::Trivial(Fr::from(17)),
            Assigned::Rational(Fr::from(19), Fr::from(23)),
            Assigned::Zero,
            Assigned::Zero,
            Assigned::Rational(Fr::from(29), Fr::from(31)),
            Assigned::Trivial(Fr::from(37)),
        ]
    }

    fn assert_sequence(advice: &CompactAdvice<Fr>, expected: &[Assigned<Fr>]) {
        assert_eq!(advice.len(), expected.len());
        assert_eq!(advice.iter().len(), expected.len());
        for (index, expected) in expected.iter().copied().enumerate() {
            assert_exact(
                advice.get(index).expect("reference advice position exists"),
                expected,
            );
        }
        assert!(advice.get(expected.len()).is_none());
        for (actual, expected) in advice.iter().zip(expected.iter().copied()) {
            assert_exact(actual, expected);
        }
    }

    #[test]
    fn compact_advice_round_trips_exact_assigned_variants_and_order() {
        let advice = fixture();
        let expected = fixture_values();
        assert_sequence(&advice, &expected);
        assert_exact(
            advice.last().expect("fixture has a last value"),
            *expected.last().expect("fixture is not empty"),
        );
        assert_eq!(
            format!("{advice:?}"),
            format!("{expected:?}"),
            "Context Debug must retain the old decoded Vec<Assigned> surface"
        );
        assert_eq!(advice.rational_positions, [2, 3, 6, 9]);
        assert_eq!(advice.rational_len(), 4);
        assert_eq!(advice.denominator_slots_len(), 4);
        assert_eq!(advice.zero_mask_bytes_len(), 2);
    }

    #[test]
    fn compact_advice_zero_mask_crosses_byte_boundaries() {
        let mut advice = CompactAdvice::default();
        let expected = (0..25)
            .map(|index| match index {
                0 | 7 | 8 | 15 | 16 | 23 | 24 => Assigned::Zero,
                6 => Assigned::Rational(Fr::from(41), Fr::from(43)),
                9 => Assigned::Rational(Fr::from(47), Fr::ZERO),
                17 => Assigned::Rational(Fr::from(53), Fr::from(59)),
                _ => Assigned::Trivial(Fr::from(index as u64 + 1)),
            })
            .collect::<Vec<_>>();
        assert_eq!(advice.zero_mask_bytes_len(), 0);
        for value in expected.iter().copied() {
            advice.push(value);
        }

        assert_sequence(&advice, &expected);
        assert_eq!(advice.zero_mask, [0x81, 0x81, 0x81, 0x01]);
        assert_eq!(
            advice.zero_mask_bytes_len(),
            compact_advice_zero_mask_len(expected.len())
        );
        assert_eq!(advice.rational_positions, [6, 9, 17]);
        assert_exact(
            advice.last().expect("boundary fixture has a last value"),
            Assigned::Zero,
        );
    }

    #[test]
    fn compact_advice_replace_clone_wipe_and_reuse_preserve_invariants() {
        let original = fixture();
        let mut clone = original.clone();
        let mut expected = fixture_values();

        clone.replace_with_trivial(2, Fr::from(13));
        expected[2] = Assigned::Trivial(Fr::from(13));
        clone.replace_with_trivial(7, Fr::from(41));
        expected[7] = Assigned::Trivial(Fr::from(41));
        clone.replace_with_trivial(3, Fr::from(43));
        expected[3] = Assigned::Trivial(Fr::from(43));
        clone.replace_with_trivial(1, Fr::from(47));
        expected[1] = Assigned::Trivial(Fr::from(47));
        clone.replace_with_trivial(1, Fr::from(53));
        expected[1] = Assigned::Trivial(Fr::from(53));
        clone.push(Assigned::Rational(Fr::from(59), Fr::from(61)));
        expected.push(Assigned::Rational(Fr::from(59), Fr::from(61)));

        assert_sequence(&clone, &expected);
        assert_eq!(clone.rational_positions, [6, 9, 11]);
        assert_eq!(
            clone.denominators,
            [Fr::from(23), Fr::from(31), Fr::from(61)]
        );
        assert_eq!(clone.rational_len(), 3);
        assert_sequence(&original, &fixture_values());

        let capacities = clone.capacities();

        clone.wipe();
        assert_eq!(clone.capacities(), capacities);
        assert!(clone.rational_positions.is_empty());
        assert!(clone.denominators.is_empty());
        assert_eq!(clone.rational_len(), 0);
        assert_eq!(clone.denominator_slots_len(), 0);
        assert!(clone.zero_mask.iter().all(|byte| *byte == 0));
        assert_eq!(clone.len(), expected.len());
        for value in clone.iter() {
            assert_exact(value, Assigned::Trivial(Fr::ZERO));
        }

        let old_len = clone.len();
        let appended = [
            Assigned::Zero,
            Assigned::Trivial(Fr::from(67)),
            Assigned::Trivial(Fr::from(71)),
            Assigned::Trivial(Fr::from(73)),
            Assigned::Trivial(Fr::from(79)),
            Assigned::Rational(Fr::from(83), Fr::ZERO),
        ];
        for value in appended {
            clone.push(value);
        }
        assert_exact(
            clone.get(old_len).expect("reused partial mask byte"),
            Assigned::Zero,
        );
        assert_exact(
            clone.last().expect("rational appended after wipe"),
            Assigned::Rational(Fr::from(83), Fr::ZERO),
        );
        assert_eq!(clone.rational_positions, [17]);
        assert_eq!(clone.denominators, [Fr::ZERO]);
        assert_eq!(clone.zero_mask_bytes_len(), 3);
    }

    #[test]
    fn context_debug_prank_and_wipe_use_compact_storage() {
        let copy_manager = SharedCopyConstraintManager::<Fr>::default();
        let mut context = Context::new(true, 0, FIRST_PHASE_CELL_TYPE_ID, 0, copy_manager);
        context.assign_cell(crate::QuantumCell::WitnessFraction(Assigned::Rational(
            Fr::from(17),
            Fr::from(19),
        )));
        let assigned: AssignedValue<Fr> = context.last().expect("assigned rational");
        assert_exact(
            assigned.value,
            Assigned::Rational(Fr::from(17), Fr::from(19)),
        );

        assigned.debug_prank(&mut context, Fr::from(23));
        assert_exact(
            context.get(0).value,
            Assigned::Trivial(Fr::from(23)),
        );
        context.wipe_advice();
        assert_eq!(context.advice_len(), 1);
        assert_exact(context.get(0).value, Assigned::Trivial(Fr::ZERO));
    }

    #[test]
    fn manager_clone_and_clear_keep_compact_advice_independent() {
        let copy_manager = SharedCopyConstraintManager::<Fr>::default();
        let mut manager = SinglePhaseCoreManager::new(true, copy_manager);
        manager
            .main()
            .assign_cell(crate::QuantumCell::WitnessFraction(Assigned::Rational(
                Fr::from(29),
                Fr::from(31),
            )));
        let clone = manager.clone();

        manager.clear();
        assert_eq!(manager.thread_count(), 0);
        assert_eq!(clone.threads[0].advice_len(), 1);
        assert_exact(
            clone.threads[0].get(0).value,
            Assigned::Rational(Fr::from(29), Fr::from(31)),
        );
    }

    #[test]
    fn compact_advice_positions_accept_the_full_u32_range() {
        assert_eq!(compact_advice_position(0), 0);
        assert_eq!(compact_advice_position(u32::MAX as usize), u32::MAX);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "CompactAdvice position exceeds the compact u32 range")]
    fn compact_advice_rejects_position_overflow() {
        compact_advice_position(u32::MAX as usize + 1);
    }

    #[test]
    #[should_panic(expected = "advice replacement offset is out of bounds")]
    fn compact_advice_rejects_out_of_bounds_replacement() {
        fixture().replace_with_trivial(11, Fr::ZERO);
    }
}

/// Represents a single thread of an execution trace.
/// * We keep the naming [Context] for historical reasons.
///
/// [Context] is CPU thread-local.
#[derive(Clone, Debug, CopyGetters)]
pub struct Context<F: ScalarField> {
    /// Flag to determine whether only witness generation or proving and verification key generation is being performed.
    /// * If witness gen is performed many operations can be skipped for optimization.
    #[getset(get_copy = "pub")]
    witness_gen_only: bool,
    /// The challenge phase that this [Context] will map to.
    #[getset(get_copy = "pub")]
    phase: usize,
    /// Canonical identifier for the external-cell region or one of the three challenge phases.
    /// Closed static tags keep packed cell identity stable across builds and dependencies.
    #[getset(get_copy = "pub")]
    type_id: &'static str,
    /// Identifier to reference cells from this [Context].
    context_id: usize,

    /// Single column of advice cells in exact, memory-dense [`Assigned`] storage.
    advice: CompactAdvice<F>,

    /// Slight optimization: since zero is so commonly used, keep a reference to the zero cell.
    zero_cell: Option<AssignedValue<F>>,

    // ========================================
    // General principle: we don't need to optimize anything specific to `witness_gen_only == false` because it is only done during keygen
    // If `witness_gen_only == false`:
    /// [Vec] representing the selector column of this [Context] accompanying each `advice` column
    /// * Assumed to have the same length as `advice`
    pub selector: Vec<bool>,

    /// Global shared thread-safe manager for all copy (equality) constraints between virtual advice, constants, and raw external Halo2 cells.
    pub copy_manager: SharedCopyConstraintManager<F>,
}

impl<F: ScalarField> Context<F> {
    /// Creates a new [Context] with the given `context_id` and witness generation enabled/disabled by the `witness_gen_only` flag.
    /// * `witness_gen_only`: flag to determine whether public key generation or only witness generation is being performed.
    /// * `context_id`: identifier to reference advice cells from this [Context] later.
    ///
    /// `type_id` must be one of the four canonical halo2-base cell identifiers.
    pub fn new(
        witness_gen_only: bool,
        phase: usize,
        type_id: &'static str,
        context_id: usize,
        copy_manager: SharedCopyConstraintManager<F>,
    ) -> Self {
        // Validate the closed tag and compact context range even when the
        // context never receives a cell.
        let _ = ContextCell::new(type_id, context_id, 0);
        Self {
            witness_gen_only,
            phase,
            type_id,
            context_id,
            advice: CompactAdvice::default(),
            selector: Vec::new(),
            zero_cell: None,
            copy_manager,
        }
    }

    /// The context id, this can be used as a tag when CPU multi-threading
    pub fn id(&self) -> usize {
        self.context_id
    }

    /// A unique tag that should identify this context across all virtual regions and phases.
    pub fn tag(&self) -> ContextTag {
        (self.type_id, self.context_id)
    }

    /// Returns the number of virtual advice cells in this context.
    pub fn advice_len(&self) -> usize {
        self.advice.len()
    }

    /// Returns the number of advice values retained as exact rational variants.
    pub fn rational_advice_len(&self) -> usize {
        self.advice.rational_len()
    }

    /// Returns the number of exact rational-denominator slots.
    pub fn advice_denominator_slots_len(&self) -> usize {
        self.advice.denominator_slots_len()
    }

    /// Returns the number of bytes currently used by the packed `Zero` mask.
    pub fn advice_zero_mask_bytes_len(&self) -> usize {
        self.advice.zero_mask_bytes_len()
    }

    /// Returns the number of sorted sparse rational-position slots.
    pub fn advice_rational_position_slots_len(&self) -> usize {
        self.advice.rational_position_slots_len()
    }

    /// Returns capacities for numerator entries, zero-mask bytes, rational
    /// positions, and rational denominators, in that order.
    pub fn advice_storage_capacities(&self) -> [usize; 4] {
        self.advice.capacities()
    }

    /// Iterates the exact [`Assigned`] values without exposing their compact backing storage.
    pub(crate) fn advice_values(&self) -> impl ExactSizeIterator<Item = Assigned<F>> + '_ {
        self.advice.iter()
    }

    /// Replaces one virtual advice value with an exact [`Assigned::Trivial`] value.
    ///
    /// This is intended for negative-test mutation helpers. If the old value
    /// was rational, its sparse position is removed and its no-longer-referenced
    /// denominator is zeroed before removal.
    pub fn replace_advice_with_trivial(&mut self, offset: usize, value: F) {
        self.advice.replace_with_trivial(offset, value);
    }

    /// Zeroes all retained advice material before the context is cleared.
    pub fn wipe_advice(&mut self) {
        self.advice.wipe();
    }

    fn latest_cell(&self) -> ContextCell {
        ContextCell::new(self.type_id, self.context_id, self.advice.len() - 1)
    }

    /// Virtually assigns the `input` within the current [Context], with different handling depending on the [QuantumCell] variant.
    pub fn assign_cell(&mut self, input: impl Into<QuantumCell<F>>) {
        // Determine the type of the cell and push it to the relevant vector
        match input.into() {
            QuantumCell::Existing(acell) => {
                self.advice.push(acell.value);
                // If witness generation is not performed, enforce equality constraints between the existing cell and the new cell
                if !self.witness_gen_only {
                    let new_cell = self.latest_cell();
                    self.copy_manager
                        .lock()
                        .unwrap()
                        .advice_equalities
                        .push((new_cell, acell.cell.unwrap()));
                }
            }
            QuantumCell::Witness(val) => {
                self.advice.push(Assigned::Trivial(val));
            }
            QuantumCell::WitnessFraction(val) => {
                self.advice.push(val);
            }
            QuantumCell::Constant(c) => {
                self.advice.push(Assigned::Trivial(c));
                // If witness generation is not performed, enforce equality constraints between the existing cell and the new cell
                if !self.witness_gen_only {
                    let new_cell = self.latest_cell();
                    self.copy_manager.lock().unwrap().constant_equalities.push((c, new_cell));
                }
            }
        }
    }

    /// Returns the [AssignedValue] of the last cell in the `advice` column of [Context] or [None] if `advice` is empty
    pub fn last(&self) -> Option<AssignedValue<F>> {
        self.advice.last().map(|v| {
            // Keep the virtual location during witness generation as well.
            // Composite circuits may need to copy-constrain a raw Halo2 chip
            // to a cell assigned by this virtual region after both regions
            // have been materialized. Constraint collection remains guarded
            // by `witness_gen_only`; retaining the pointer does not add any
            // virtual equality work.
            AssignedValue { value: v, cell: Some(self.latest_cell()) }
        })
    }

    /// Returns the [AssignedValue] of the cell at the given `offset` in the `advice` column of [Context]
    /// * `offset`: the offset of the cell to be fetched
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    /// * Assumes `offset` is a valid index in `advice`;
    ///     * `0` <= `offset` < `advice.len()` (or `advice.len() + offset >= 0` if `offset` is negative)
    pub fn get(&self, offset: isize) -> AssignedValue<F> {
        let offset = if offset < 0 {
            self.advice.len().wrapping_add_signed(offset)
        } else {
            offset as usize
        };
        assert!(offset < self.advice.len());
        let cell = ContextCell::new(self.type_id, self.context_id, offset);
        AssignedValue {
            value: self
                .advice
                .get(offset)
                .expect("validated advice offset must exist"),
            cell: Some(cell),
        }
    }

    /// Creates an equality constraint between two `advice` cells.
    /// * `a`: the first `advice` cell to be constrained equal
    /// * `b`: the second `advice` cell to be constrained equal
    /// * Assumes both cells are `advice` cells
    pub fn constrain_equal(&mut self, a: &AssignedValue<F>, b: &AssignedValue<F>) {
        if !self.witness_gen_only {
            self.copy_manager
                .lock()
                .unwrap()
                .advice_equalities
                .push((a.cell.unwrap(), b.cell.unwrap()));
        }
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset`.
    ///
    /// * `inputs`: Iterator that specifies the cells to be assigned
    /// * `gate_offsets`: specifies relative offset from current position to enable selector for the gate (e.g., `0` is `inputs[0]`).
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last previously assigned cell)
    pub fn assign_region<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
    ) where
        Q: Into<QuantumCell<F>>,
    {
        if self.witness_gen_only {
            for input in inputs {
                self.assign_cell(input);
            }
        } else {
            let row_offset = self.advice.len();
            // note: row_offset may not equal self.selector.len() at this point if we previously used `load_constant` or `load_witness`
            for input in inputs {
                self.assign_cell(input);
            }
            self.selector.resize(self.advice.len(), false);
            for offset in gate_offsets {
                *self
                    .selector
                    .get_mut(row_offset.checked_add_signed(offset).expect("Invalid gate offset"))
                    .expect("Invalid selector offset") = true;
            }
        }
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset` and returns the last assigned cell.
    ///
    /// Assumes `gate_offsets` is the same length as `inputs`
    ///
    /// Returns the last assigned cell
    /// * `inputs`: Iterator that specifies the cells to be assigned
    /// * `gate_offsets`: specifies indices to enable selector for the gate; assume `gate_offsets` is sorted in increasing order
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    pub fn assign_region_last<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
    ) -> AssignedValue<F>
    where
        Q: Into<QuantumCell<F>>,
    {
        self.assign_region(inputs, gate_offsets);
        self.last().unwrap()
    }

    /// Pushes multiple advice cells to the `advice` column of [Context] and enables them by enabling the corresponding selector specified in `gate_offset`.
    ///
    /// Allows for the specification of equality constraints between cells at `equality_offsets` within the `advice` column and external advice cells specified in `external_equality` (e.g, Fixed column).
    /// * `gate_offsets`: specifies indices to enable selector for the gate;
    ///     * `offset` may be negative indexing from the end of the column (e.g., `-1` is the last cell)
    /// * `equality_offsets`: specifies pairs of indices to constrain equality
    /// * `external_equality`: specifies an existing cell to constrain equality with the cell at a certain index
    pub fn assign_region_smart<Q>(
        &mut self,
        inputs: impl IntoIterator<Item = Q>,
        gate_offsets: impl IntoIterator<Item = isize>,
        equality_offsets: impl IntoIterator<Item = (isize, isize)>,
        external_equality: impl IntoIterator<Item = (Option<ContextCell>, isize)>,
    ) where
        Q: Into<QuantumCell<F>>,
    {
        let row_offset = self.advice.len();
        self.assign_region(inputs, gate_offsets);

        // note: row_offset may not equal self.selector.len() at this point if we previously used `load_constant` or `load_witness`
        // If not in witness generation mode, add equality constraints.
        if !self.witness_gen_only {
            // Add equality constraints between cells in the advice column.
            for (offset1, offset2) in equality_offsets {
                self.copy_manager.lock().unwrap().advice_equalities.push((
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset1),
                    ),
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset2),
                    ),
                ));
            }
            // Add equality constraints between cells in the advice column and external cells (Fixed column).
            for (cell, offset) in external_equality {
                self.copy_manager.lock().unwrap().advice_equalities.push((
                    cell.unwrap(),
                    ContextCell::new(
                        self.type_id,
                        self.context_id,
                        row_offset.wrapping_add_signed(offset),
                    ),
                ));
            }
        }
    }

    /// Assigns a region of witness cells in an iterator and returns a [Vec] of assigned cells.
    /// * `witnesses`: Iterator that specifies the cells to be assigned
    pub fn assign_witnesses(
        &mut self,
        witnesses: impl IntoIterator<Item = F>,
    ) -> Vec<AssignedValue<F>> {
        let row_offset = self.advice.len();
        self.assign_region(witnesses.into_iter().map(QuantumCell::Witness), []);
        (row_offset..self.advice.len())
            .map(|offset| {
                let cell = ContextCell::new(self.type_id, self.context_id, offset);
                AssignedValue {
                    value: self
                        .advice
                        .get(offset)
                        .expect("newly assigned advice offset must exist"),
                    cell: Some(cell),
                }
            })
            .collect()
    }

    /// Assigns a witness value and returns the corresponding assigned cell.
    /// * `witness`: the witness value to be assigned
    pub fn load_witness(&mut self, witness: F) -> AssignedValue<F> {
        self.assign_cell(QuantumCell::Witness(witness));
        if !self.witness_gen_only {
            self.selector.resize(self.advice.len(), false);
        }
        self.last().unwrap()
    }

    /// Assigns a constant value and returns the corresponding assigned cell.
    /// * `c`: the constant value to be assigned
    pub fn load_constant(&mut self, c: F) -> AssignedValue<F> {
        self.assign_cell(QuantumCell::Constant(c));
        if !self.witness_gen_only {
            self.selector.resize(self.advice.len(), false);
        }
        self.last().unwrap()
    }

    /// Assigns a list of constant values and returns the corresponding assigned cells.
    /// * `c`: the list of constant values to be assigned
    pub fn load_constants(&mut self, c: &[F]) -> Vec<AssignedValue<F>> {
        c.iter().map(|v| self.load_constant(*v)).collect_vec()
    }

    /// Assigns the 0 value to a new cell or returns a previously assigned zero cell from `zero_cell`.
    pub fn load_zero(&mut self) -> AssignedValue<F> {
        if let Some(zcell) = &self.zero_cell {
            return *zcell;
        }
        let zero_cell = self.load_constant(F::ZERO);
        self.zero_cell = Some(zero_cell);
        zero_cell
    }

    /// Helper function for debugging using `MockProver`. This adds a constraint that always fails.
    /// The `MockProver` will print out the row, column where it fails, so it serves as a debugging "break point"
    /// so you can add to your code to search for where the actual constraint failure occurs.
    pub fn debug_assert_false(&mut self) {
        use rand_chacha::rand_core::OsRng;
        let rand1 = self.load_witness(F::random(OsRng));
        let rand2 = self.load_witness(F::random(OsRng));
        self.constrain_equal(&rand1, &rand2);
    }
}
