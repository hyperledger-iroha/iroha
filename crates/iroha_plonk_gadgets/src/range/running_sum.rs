//! Running-sum range checks against a `2^b`-row lookup table (port of the
//! M8 `U128Circuit` decomposition).
//!
//! # Relation
//!
//! A check that `v < 2^bits` with `b`-bit limbs uses `L = ceil(bits / b)`
//! limbs and the running sum `z_i = floor(v / 2^(b i))` in one
//! equality-enabled advice column, one row per `z_i`:
//!
//! - rows `0 .. L-1` (`q_step`): `z_i - 2^b z_{i+1}` (limb `i`) is in the
//!   table, i.e. in `[0, 2^b)`;
//! - row `L-1` (`q_limb`): `z_{L-1}` (the top limb) is in the table;
//! - when the top limb has `t = bits - (L-1) b < b` bits, one more row
//!   (`q_limb` and `q_shift`) holds `z_{L-1} 2^(b-t)`, which must also be in
//!   the table, so `z_{L-1} < 2^t`.
//!
//! Then `v = z_0 = sum_i limb_i 2^(b i) < 2^(b(L-1)) 2^t = 2^bits` as an
//! integer: every partial sum stays below `2^bits <= 2^252 < p`, so nothing
//! wraps. An out-of-range witness has no satisfying assignment; the chip
//! still lays out `floor(v / 2^(b i))`, which a table lookup rejects.
//!
//! The lookup input is `q_step (z - 2^b z_next) + q_limb z` (degree 2, so the
//! lookup needs degree 5) and the shift gate is
//! `q_shift (z - z_prev shift)` (degree 3) with the factor `2^(b-t)` in a
//! fixed column, so one chip checks any width. The column is queried at
//! rotations `-1, 0, 1`, inside the default blinding budget.
//!
//! A check takes `L + [t < b]` rows: 10 for 128 bits at `b = 15`, 16 at
//! `b = 9` (M8 inventory).

use iroha_pasta::PastaField;
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Fixed, Rotation, Selector, TableColumn},
    frontend::{Error, Layouter, Region, Value},
};

use crate::cells::{RowCursor, Word, assign_word, copy_word};

/// The widest value a range check accepts (`2^252 < p` for both fields).
pub const MAX_RANGE_BITS: usize = 252;

/// The limb width `b` of a running-sum chip, `1 <= b <= 24`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LimbBits(usize);

impl LimbBits {
    /// The widest supported limb (a `2^24`-row table).
    pub const MAX: usize = 24;

    /// `bits`, when it is in `1..=24`.
    #[must_use]
    pub const fn new(bits: usize) -> Option<Self> {
        if bits >= 1 && bits <= Self::MAX {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// The widest limb whose table fits a circuit of `2^k` rows: `k - 1`
    /// (the M8 choice), when that is a valid width.
    #[must_use]
    pub const fn for_k(k: u32) -> Option<Self> {
        if k < 2 {
            return None;
        }
        Self::new((k - 1) as usize)
    }

    /// The width.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    /// The table size `2^b`.
    #[must_use]
    pub const fn table_rows(self) -> usize {
        1 << self.0
    }
}

/// The shape of one range check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeShape {
    /// The checked width.
    pub bits: usize,
    /// The limb width `b`.
    pub limb_bits: usize,
    /// The number of limbs `L = ceil(bits / b)`.
    pub limbs: usize,
    /// The width `t` of the top limb (`1..=b`).
    pub top_bits: usize,
    /// Rows: `L`, plus one shifted top-limb row when `t < b`.
    pub rows: usize,
}

impl RangeShape {
    /// The shape of a `bits`-bit check with `limb_bits` limbs, for
    /// `1 <= bits <= 252`.
    #[must_use]
    pub const fn new(bits: usize, limb_bits: LimbBits) -> Option<Self> {
        if bits == 0 || bits > MAX_RANGE_BITS {
            return None;
        }
        let b = limb_bits.get();
        let limbs = bits.div_ceil(b);
        let top_bits = bits - (limbs - 1) * b;
        let rows = if top_bits < b { limbs + 1 } else { limbs };
        Some(Self {
            bits,
            limb_bits: b,
            limbs,
            top_bits,
            rows,
        })
    }

    /// Whether the check has a shifted top-limb row.
    #[must_use]
    pub const fn shifted(&self) -> bool {
        self.top_bits < self.limb_bits
    }
}

/// Columns, table and selectors of the running-sum chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunningSumConfig {
    z: Column<Advice>,
    table: TableColumn,
    shift: Column<Fixed>,
    q_step: Selector,
    q_limb: Selector,
    q_shift: Selector,
    limb_bits: LimbBits,
}

impl RunningSumConfig {
    /// Configures the chip on `z` (made equality-enabled) with a new table
    /// column of `2^b` rows.
    pub fn configure<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        z: Column<Advice>,
        limb_bits: LimbBits,
    ) -> Self {
        meta.enable_equality(z);
        let table = meta.lookup_table_column();
        let shift = meta.fixed_column();
        let q_step = meta.complex_selector();
        let q_limb = meta.complex_selector();
        let q_shift = meta.selector();
        let radix = F::from(1_u64 << limb_bits.get());
        meta.lookup("running-sum limb", |cells| {
            let step = cells.query_selector(q_step);
            let limb = cells.query_selector(q_limb);
            let cur = cells.query_advice(z, Rotation::cur());
            let next = cells.query_advice(z, Rotation::next());
            vec![(step * (cur.clone() - next * radix) + limb * cur, table)]
        });
        meta.create_gate("running-sum shifted top limb", |cells| {
            let q = cells.query_selector(q_shift);
            let cur = cells.query_advice(z, Rotation::cur());
            let prev = cells.query_advice(z, Rotation::prev());
            let factor = cells.query_fixed(shift, Rotation::cur());
            vec![("z - z_prev shift", q * (cur - prev * factor))]
        });
        Self {
            z,
            table,
            shift,
            q_step,
            q_limb,
            q_shift,
            limb_bits,
        }
    }

    /// The running-sum column.
    #[must_use]
    pub const fn column(&self) -> Column<Advice> {
        self.z
    }

    /// The limb width.
    #[must_use]
    pub const fn limb_bits(&self) -> LimbBits {
        self.limb_bits
    }
}

/// `limbs >> shift` for a little-endian 256-bit integer.
fn shift_right(limbs: [u64; 4], shift: usize) -> [u64; 4] {
    let words = shift / 64;
    let bits = shift % 64;
    let mut out = [0_u64; 4];
    for (index, out) in out.iter_mut().enumerate() {
        let source = index + words;
        let Some(low) = limbs.get(source) else {
            continue;
        };
        let mut value = low >> bits;
        if bits > 0
            && let Some(high) = limbs.get(source + 1)
        {
            value |= high << (64 - bits);
        }
        *out = value;
    }
    out
}

/// The running-sum column entries for `value`, honest or not:
/// `z_i = floor(v / 2^(b i))` for `i < L`, then the shifted top limb.
#[must_use]
pub fn running_sum_witness<F: PastaField>(value: &F, shape: RangeShape) -> Vec<F> {
    let limbs = value.to_canonical_limbs();
    let mut entries: Vec<F> = (0..shape.limbs)
        .map(|limb| F::from_raw_reduced(shift_right(limbs, shape.limb_bits * limb)))
        .collect();
    if shape.shifted()
        && let Some(top) = entries.last().copied()
    {
        entries.push(top * F::from(1_u64 << (shape.limb_bits - shape.top_bits)));
    }
    entries
}

/// Native reference: the limbs of `value` when `value < 2^bits`, or `None`.
#[must_use]
pub fn limbs_native<F: PastaField>(value: &F, shape: RangeShape) -> Option<Vec<u64>> {
    let width = usize::try_from(value.bit_length_vartime()).ok()?;
    if width > shape.bits {
        return None;
    }
    let limbs = value.to_canonical_limbs();
    let mask = (1_u64 << shape.limb_bits) - 1;
    Some(
        (0..shape.limbs)
            .map(|limb| shift_right(limbs, shape.limb_bits * limb)[0] & mask)
            .collect(),
    )
}

/// The running-sum range-check chip.
#[derive(Clone, Debug)]
pub struct RunningSumChip<F: PastaField> {
    config: RunningSumConfig,
    rows: RowCursor,
    _marker: core::marker::PhantomData<F>,
}

impl<F: PastaField> RunningSumChip<F> {
    /// A chip whose first row is 0.
    #[must_use]
    pub const fn new(config: RunningSumConfig) -> Self {
        Self {
            config,
            rows: RowCursor::starting_at(0),
            _marker: core::marker::PhantomData,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &RunningSumConfig {
        &self.config
    }

    /// The first row not used yet.
    #[must_use]
    pub const fn next_row(&self) -> usize {
        self.rows.next_row()
    }

    /// The shape of a `bits`-bit check.
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] for a width outside `1..=252`.
    pub fn shape(&self, bits: usize) -> Result<RangeShape, Error> {
        RangeShape::new(bits, self.config.limb_bits).ok_or(Error::Synthesis)
    }

    /// Loads the limb table `0 .. 2^b`.
    ///
    /// # Errors
    ///
    /// [`Error`] when the table does not fit the usable rows.
    pub fn load_table(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        let table = self.config.table;
        let rows = self.config.limb_bits.table_rows();
        layouter.assign_table(
            || "running-sum limb table",
            |mut cells| {
                for row in 0..rows {
                    let value = u64::try_from(row).map_err(|_| Error::BoundsFailure)?;
                    cells.assign_cell(|| "limb", table, row, || Value::known(F::from(value)))?;
                }
                Ok(())
            },
        )
    }

    /// Range-checks a copy of `word` to `bits` bits and returns the copy
    /// (`z_0`).
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] for a width outside `1..=252`, and [`Error`]
    /// from the layout.
    pub fn range_check(
        &mut self,
        region: &mut Region<'_, F>,
        word: &Word<F>,
        bits: usize,
    ) -> Result<Word<F>, Error> {
        let shape = self.shape(bits)?;
        let start = self.rows.take(shape.rows)?;
        let z0 = copy_word(region, word, self.config.z, start)?;
        self.decompose(region, start, word.value(), shape)?;
        Ok(z0)
    }

    /// Assigns `value` as `z_0` of a new `bits`-bit check and returns it.
    ///
    /// # Errors
    ///
    /// As [`Self::range_check`].
    pub fn witness_range_checked(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<F>,
        bits: usize,
    ) -> Result<Word<F>, Error> {
        let shape = self.shape(bits)?;
        let start = self.rows.take(shape.rows)?;
        let z0 = assign_word(region, self.config.z, start, value)?;
        self.decompose(region, start, value, shape)?;
        Ok(z0)
    }

    /// Lays out rows `1..` of a check whose `z_0` is at `start`.
    fn decompose(
        &self,
        region: &mut Region<'_, F>,
        start: usize,
        value: Value<F>,
        shape: RangeShape,
    ) -> Result<(), Error> {
        let entries = value.map(|value| running_sum_witness(&value, shape));
        let entries = entries.transpose_vec(shape.rows)?;
        for (offset, entry) in entries.into_iter().enumerate().skip(1) {
            let row = start.checked_add(offset).ok_or(Error::BoundsFailure)?;
            assign_word(region, self.config.z, row, entry)?;
        }
        for limb in 0..shape.limbs - 1 {
            self.config.q_step.enable(region, start + limb)?;
        }
        let top = start + shape.limbs - 1;
        region.enable_selector(String::new, &self.config.q_limb, top)?;
        if shape.shifted() {
            let shifted = top + 1;
            region.enable_selector(String::new, &self.config.q_limb, shifted)?;
            self.config.q_shift.enable(region, shifted)?;
            let factor = F::from(1_u64 << (shape.limb_bits - shape.top_bits));
            region.assign_fixed(self.config.shift, shifted, factor)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ff::{Field, PrimeField};
    use iroha_pasta::{Fp, Fq};

    use super::*;

    fn bits(b: usize) -> LimbBits {
        LimbBits::new(b).expect("valid limb width")
    }

    #[test]
    fn limb_bits_bounds() {
        assert_eq!(LimbBits::new(0), None);
        assert_eq!(LimbBits::new(25), None);
        assert_eq!(LimbBits::new(9).map(LimbBits::get), Some(9));
        assert_eq!(LimbBits::for_k(10), LimbBits::new(9));
        assert_eq!(LimbBits::for_k(1), None);
        assert_eq!(bits(9).table_rows(), 512);
    }

    #[test]
    fn shapes_match_the_m8_inventory() {
        // M8: rows per 128-bit check at lookup bits 15 / 12 / 10 / 8.
        for (b, rows) in [(15, 10), (12, 12), (10, 14), (8, 16), (9, 16)] {
            assert_eq!(
                RangeShape::new(128, bits(b)).map(|s| s.rows),
                Some(rows),
                "b = {b}"
            );
        }
        let shape = RangeShape::new(64, bits(8)).expect("shape");
        assert_eq!((shape.limbs, shape.top_bits, shape.rows), (8, 8, 8));
        assert!(!shape.shifted());
        assert_eq!(RangeShape::new(0, bits(8)), None);
        assert_eq!(RangeShape::new(253, bits(8)), None);
        assert_eq!(RangeShape::new(3, bits(8)).map(|s| s.rows), Some(2));
    }

    #[test]
    fn shift_right_matches_integer_shifts() {
        let limbs = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210, 7, 1 << 60];
        assert_eq!(shift_right(limbs, 0), limbs);
        assert_eq!(shift_right(limbs, 64), [limbs[1], 7, 1 << 60, 0]);
        assert_eq!(shift_right(limbs, 256), [0; 4]);
        let low = u128::from(limbs[0]) | (u128::from(limbs[1]) << 64);
        let shifted = shift_right(limbs, 4);
        assert_eq!(
            u128::from(shifted[0]) | (u128::from(shifted[1]) << 64),
            (low >> 4) | (7 << 124)
        );
    }

    #[test]
    fn witness_and_native_limbs() {
        let shape = RangeShape::new(10, bits(4)).expect("shape");
        assert_eq!((shape.limbs, shape.top_bits, shape.rows), (3, 2, 4));
        let value = Fp::from(0b10_1101_0110);
        let entries = running_sum_witness(&value, shape);
        assert_eq!(
            entries,
            vec![
                value,
                Fp::from(0b10_1101),
                Fp::from(0b10),
                Fp::from(0b10 << 2)
            ]
        );
        assert_eq!(
            limbs_native(&value, shape),
            Some(vec![0b0110, 0b1101, 0b10])
        );
        assert_eq!(limbs_native(&Fp::from(1 << 10), shape), None);
        assert_eq!(limbs_native(&-Fq::ONE, shape), None);
        let wide = RangeShape::new(128, bits(9)).expect("shape");
        let max = Fq::from_u128(u128::MAX);
        let limbs = limbs_native(&max, wide).expect("in range");
        assert_eq!(limbs.len(), 15);
        assert_eq!(limbs[14], 0b11);
    }
}
