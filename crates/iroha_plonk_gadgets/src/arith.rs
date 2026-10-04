//! The glue arithmetic chip: the standard PLONK gate plus boolean, select
//! and is-zero gates on four equality-enabled advice columns.
//!
//! # Layout
//!
//! One operation is one row of the advice columns `a, b, c, d`. Operands
//! arrive by copy constraint, outputs are new cells, and every assigned cell
//! is pinned by the row's gate or by a copy.
//!
//! | gate | polynomial | degree |
//! | --- | --- | --- |
//! | standard | `q_m a b + q_a a + q_b b + q_c c + q_d d + q_k` | 3 |
//! | boolean | `s_bool (a^2 - a)` | 3 |
//! | select | `s_select (a (b - c) + c - d)` | 3 |
//! | is-zero | `s_is_zero (a b + c - 1)`, `s_is_zero a c`, `s_is_zero b c` | 3 |
//!
//! The coefficients `q_*` are fixed columns, so the standard gate needs no
//! selector: a row whose coefficients are all zero (including every
//! blinding row) is unconstrained by it. The is-zero gate also pins the
//! inverse witness to zero when the input is zero, so no cell of the chip is
//! ever free.
//!
//! Constants are pinned either by the standard gate (`a + q_k = 0`) or
//! through the constants column ([`GlueChip::assert_constant`]).

use iroha_pasta::PastaField;
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Expression, Fixed, Rotation, Selector},
    frontend::{Error, Region, Value},
};

use crate::cells::{Bit, RowCursor, Word, assign_word, copy_word};

/// Advice columns of the glue chip.
pub const GLUE_WIDTH: usize = 4;

/// Columns and selectors of the glue chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlueConfig {
    advice: [Column<Advice>; GLUE_WIDTH],
    /// `q_m, q_a, q_b, q_c, q_d, q_k`.
    coefficients: [Column<Fixed>; 6],
    s_bool: Selector,
    s_select: Selector,
    s_is_zero: Selector,
}

impl GlueConfig {
    /// Configures the glue gates on `advice` (made equality-enabled) and
    /// enables `constants` as a constants column.
    pub fn configure<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; GLUE_WIDTH],
        constants: Column<Fixed>,
    ) -> Self {
        for column in advice {
            meta.enable_equality(column);
        }
        meta.enable_constant(constants);
        let coefficients = core::array::from_fn(|_| meta.fixed_column());
        let s_bool = meta.selector();
        let s_select = meta.selector();
        let s_is_zero = meta.selector();
        meta.create_gate("glue standard", |cells| {
            let [a, b, c, d] = advice.map(|column| cells.query_advice(column, Rotation::cur()));
            let [qm, qa, qb, qc, qd, qk] =
                coefficients.map(|column| cells.query_fixed(column, Rotation::cur()));
            vec![(
                "q_m a b + q_a a + q_b b + q_c c + q_d d + q_k",
                qm * a.clone() * b.clone() + qa * a + qb * b + qc * c + qd * d + qk,
            )]
        });
        meta.create_gate("glue boolean", |cells| {
            let s = cells.query_selector(s_bool);
            let a = cells.query_advice(advice[0], Rotation::cur());
            vec![("a^2 - a", s * (a.clone() * a.clone() - a))]
        });
        meta.create_gate("glue select", |cells| {
            let enabled = cells.query_selector(s_select);
            let [bit, x, y, out] = advice.map(|column| cells.query_advice(column, Rotation::cur()));
            vec![(
                "a (b - c) + c - d",
                enabled * (bit * (x - y.clone()) + y - out),
            )]
        });
        meta.create_gate("glue is-zero", |cells| {
            let s = cells.query_selector(s_is_zero);
            let [a, b, c, _] = advice.map(|column| cells.query_advice(column, Rotation::cur()));
            vec![
                (
                    "x inv + bit - 1",
                    s.clone() * (a.clone() * b.clone() + c.clone() - Expression::Constant(F::ONE)),
                ),
                ("x bit", s.clone() * (a * c.clone())),
                ("inv bit", s * (b * c)),
            ]
        });
        Self {
            advice,
            coefficients,
            s_bool,
            s_select,
            s_is_zero,
        }
    }

    /// The advice columns `a, b, c, d`.
    #[must_use]
    pub const fn advice(&self) -> [Column<Advice>; GLUE_WIDTH] {
        self.advice
    }
}

/// The coefficients of one standard-gate row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Coefficients<F> {
    pub(crate) m: F,
    pub(crate) a: F,
    pub(crate) b: F,
    pub(crate) c: F,
    pub(crate) d: F,
    pub(crate) k: F,
}

impl<F: PastaField> Coefficients<F> {
    /// All zero: the standard gate is off.
    pub(crate) const fn zero() -> Self {
        Self {
            m: F::ZERO,
            a: F::ZERO,
            b: F::ZERO,
            c: F::ZERO,
            d: F::ZERO,
            k: F::ZERO,
        }
    }
}

/// What one cell of a row holds.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Slot<'w, F: PastaField> {
    /// Left unassigned.
    Empty,
    /// A copy of an existing word.
    Copy(&'w Word<F>),
    /// A new witness value.
    Value(Value<F>),
}

/// The glue chip: one operation per row, allocated from its own cursor.
#[derive(Clone, Debug)]
pub struct GlueChip<F: PastaField> {
    config: GlueConfig,
    rows: RowCursor,
    _marker: core::marker::PhantomData<F>,
}

impl<F: PastaField> GlueChip<F> {
    /// A chip whose first row is 0.
    #[must_use]
    pub const fn new(config: GlueConfig) -> Self {
        Self::starting_at(config, 0)
    }

    /// A chip whose first row is `row` (to share the columns with other
    /// users above it).
    #[must_use]
    pub const fn starting_at(config: GlueConfig, row: usize) -> Self {
        Self {
            config,
            rows: RowCursor::starting_at(row),
            _marker: core::marker::PhantomData,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &GlueConfig {
        &self.config
    }

    /// The first row not used yet.
    #[must_use]
    pub const fn next_row(&self) -> usize {
        self.rows.next_row()
    }

    /// Lays out one row: assigns the slots, the nonzero coefficients and the
    /// selector, and returns the cells it assigned.
    pub(crate) fn row(
        &mut self,
        region: &mut Region<'_, F>,
        coefficients: Coefficients<F>,
        slots: [Slot<'_, F>; GLUE_WIDTH],
        selector: Option<Selector>,
    ) -> Result<[Option<Word<F>>; GLUE_WIDTH], Error> {
        let row = self.rows.take(1)?;
        let mut words = [None, None, None, None];
        for ((slot, column), word) in slots.iter().zip(self.config.advice).zip(&mut words) {
            *word = match slot {
                Slot::Empty => None,
                Slot::Copy(source) => Some(copy_word(region, source, column, row)?),
                Slot::Value(value) => Some(assign_word(region, column, row, *value)?),
            };
        }
        let Coefficients { m, a, b, c, d, k } = coefficients;
        for (value, column) in [m, a, b, c, d, k].into_iter().zip(self.config.coefficients) {
            if !bool::from(value.is_zero()) {
                region.assign_fixed(column, row, value)?;
            }
        }
        if let Some(selector) = selector {
            selector.enable(region, row)?;
        }
        Ok(words)
    }

    /// Lays out a row and returns the cell in slot `index`.
    fn row_output(
        &mut self,
        region: &mut Region<'_, F>,
        coefficients: Coefficients<F>,
        slots: [Slot<'_, F>; GLUE_WIDTH],
        selector: Option<Selector>,
        index: usize,
    ) -> Result<Word<F>, Error> {
        let mut words = self.row(region, coefficients, slots, selector)?;
        words
            .get_mut(index)
            .and_then(Option::take)
            .ok_or(Error::Synthesis)
    }

    /// Free witnesses, four per row. They are pinned only by the copies and
    /// gates that later use them.
    ///
    /// # Errors
    ///
    /// [`Error`] when a row is out of range or a value is unknown while
    /// proving.
    pub fn witnesses(
        &mut self,
        region: &mut Region<'_, F>,
        values: &[Value<F>],
    ) -> Result<Vec<Word<F>>, Error> {
        let mut out = Vec::with_capacity(values.len());
        for chunk in values.chunks(GLUE_WIDTH) {
            let mut slots = [Slot::Empty; GLUE_WIDTH];
            for (slot, value) in slots.iter_mut().zip(chunk) {
                *slot = Slot::Value(*value);
            }
            let words = self.row(region, Coefficients::zero(), slots, None)?;
            out.extend(words.into_iter().flatten());
        }
        Ok(out)
    }

    /// One free witness (see [`Self::witnesses`]).
    ///
    /// # Errors
    ///
    /// As [`Self::witnesses`].
    pub fn witness(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<F>,
    ) -> Result<Word<F>, Error> {
        self.witnesses(region, &[value])?
            .pop()
            .ok_or(Error::Synthesis)
    }

    /// A cell pinned to `constant` by the standard gate (`a - constant = 0`).
    ///
    /// # Errors
    ///
    /// [`Error`] when the row is out of range.
    pub fn constant(&mut self, region: &mut Region<'_, F>, constant: F) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            a: F::ONE,
            k: -constant,
            ..Coefficients::zero()
        };
        let slots = [
            Slot::Value(Value::known(constant)),
            Slot::Empty,
            Slot::Empty,
            Slot::Empty,
        ];
        self.row_output(region, coefficients, slots, None, 0)
    }

    /// `x + y`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn add(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        y: &Word<F>,
    ) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            a: F::ONE,
            b: F::ONE,
            c: -F::ONE,
            ..Coefficients::zero()
        };
        let out = x.value() + y.value();
        let slots = [Slot::Copy(x), Slot::Copy(y), Slot::Value(out), Slot::Empty];
        self.row_output(region, coefficients, slots, None, 2)
    }

    /// `x - y`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn sub(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        y: &Word<F>,
    ) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            a: F::ONE,
            b: -F::ONE,
            c: -F::ONE,
            ..Coefficients::zero()
        };
        let out = x.value() - y.value();
        let slots = [Slot::Copy(x), Slot::Copy(y), Slot::Value(out), Slot::Empty];
        self.row_output(region, coefficients, slots, None, 2)
    }

    /// `x * y`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn mul(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        y: &Word<F>,
    ) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            m: F::ONE,
            c: -F::ONE,
            ..Coefficients::zero()
        };
        let out = x.value() * y.value();
        let slots = [Slot::Copy(x), Slot::Copy(y), Slot::Value(out), Slot::Empty];
        self.row_output(region, coefficients, slots, None, 2)
    }

    /// `x * y + z`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn mul_add(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        y: &Word<F>,
        z: &Word<F>,
    ) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            m: F::ONE,
            c: F::ONE,
            d: -F::ONE,
            ..Coefficients::zero()
        };
        let out = x.value() * y.value() + z.value();
        let slots = [
            Slot::Copy(x),
            Slot::Copy(y),
            Slot::Copy(z),
            Slot::Value(out),
        ];
        self.row_output(region, coefficients, slots, None, 3)
    }

    /// `x + constant`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn add_constant(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        constant: F,
    ) -> Result<Word<F>, Error> {
        let coefficients = Coefficients {
            a: F::ONE,
            b: -F::ONE,
            k: constant,
            ..Coefficients::zero()
        };
        let out = x.value().map(|x| x + constant);
        let slots = [Slot::Copy(x), Slot::Value(out), Slot::Empty, Slot::Empty];
        self.row_output(region, coefficients, slots, None, 1)
    }

    /// `sum_i coefficient_i * word_i + constant` over at most three terms.
    ///
    /// # Errors
    ///
    /// [`Error::Synthesis`] for more than three terms, and [`Error`] from the
    /// layout.
    pub fn linear(
        &mut self,
        region: &mut Region<'_, F>,
        terms: &[(F, &Word<F>)],
        constant: F,
    ) -> Result<Word<F>, Error> {
        if terms.len() >= GLUE_WIDTH {
            return Err(Error::Synthesis);
        }
        let mut coefficients = Coefficients {
            d: -F::ONE,
            k: constant,
            ..Coefficients::zero()
        };
        let mut slots = [Slot::Empty; GLUE_WIDTH];
        let mut out = Value::known(constant);
        for (index, (coefficient, word)) in terms.iter().enumerate() {
            match index {
                0 => coefficients.a = *coefficient,
                1 => coefficients.b = *coefficient,
                _ => coefficients.c = *coefficient,
            }
            slots[index] = Slot::Copy(word);
            out = out + word.value().map(|value| value * coefficient);
        }
        slots[GLUE_WIDTH - 1] = Slot::Value(out);
        self.row_output(region, coefficients, slots, None, GLUE_WIDTH - 1)
    }

    /// Constrains `x = y` (a copy constraint; no row, so no chip state).
    ///
    /// # Errors
    ///
    /// [`Error`] from the copy.
    pub fn assert_equal(region: &mut Region<'_, F>, x: &Word<F>, y: &Word<F>) -> Result<(), Error> {
        region.constrain_equal(x.cell(), y.cell())
    }

    /// Constrains `x = constant` through the constants column (no row, so no
    /// chip state).
    ///
    /// # Errors
    ///
    /// [`Error`] from the copy.
    pub fn assert_constant(
        region: &mut Region<'_, F>,
        x: &Word<F>,
        constant: F,
    ) -> Result<(), Error> {
        region.constrain_constant(x.cell(), constant)
    }

    /// Constrains `x != 0` with an inverse witness (`x inv - 1 = 0`).
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assert_nonzero(&mut self, region: &mut Region<'_, F>, x: &Word<F>) -> Result<(), Error> {
        let coefficients = Coefficients {
            m: F::ONE,
            k: -F::ONE,
            ..Coefficients::zero()
        };
        let inverse = x.value().map(|x| x.invert().unwrap_or(F::ZERO));
        let slots = [
            Slot::Copy(x),
            Slot::Value(inverse),
            Slot::Empty,
            Slot::Empty,
        ];
        self.row(region, coefficients, slots, None).map(|_| ())
    }

    /// A new boolean witness.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn boolean(
        &mut self,
        region: &mut Region<'_, F>,
        value: Value<bool>,
    ) -> Result<Bit<F>, Error> {
        let value = value.map(|bit| if bit { F::ONE } else { F::ZERO });
        let slots = [Slot::Value(value), Slot::Empty, Slot::Empty, Slot::Empty];
        let selector = Some(self.config.s_bool);
        self.row_output(region, Coefficients::zero(), slots, selector, 0)
            .map(Bit::new)
    }

    /// Constrains `x` to `{0, 1}` and returns it as a [`Bit`].
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn assert_bool(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
    ) -> Result<Bit<F>, Error> {
        let slots = [Slot::Copy(x), Slot::Empty, Slot::Empty, Slot::Empty];
        let selector = Some(self.config.s_bool);
        self.row_output(region, Coefficients::zero(), slots, selector, 0)
            .map(Bit::new)
    }

    /// `bit ? x : y`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn select(
        &mut self,
        region: &mut Region<'_, F>,
        bit: &Bit<F>,
        x: &Word<F>,
        y: &Word<F>,
    ) -> Result<Word<F>, Error> {
        let out = bit
            .word()
            .value()
            .zip(x.value())
            .zip(y.value())
            .map(|((bit, x), y)| bit * (x - y) + y);
        let slots = [
            Slot::Copy(bit.word()),
            Slot::Copy(x),
            Slot::Copy(y),
            Slot::Value(out),
        ];
        let selector = Some(self.config.s_select);
        self.row_output(region, Coefficients::zero(), slots, selector, 3)
    }

    /// `[x = 0]`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn is_zero(&mut self, region: &mut Region<'_, F>, x: &Word<F>) -> Result<Bit<F>, Error> {
        // Constant time in the (secret) witness: `CtOption::unwrap_or`
        // selects without branching, and `bit = 1 - x inv`.
        let inverse = x.value().map(|x| x.invert().unwrap_or(F::ZERO));
        let bit = x
            .value()
            .zip(inverse)
            .map(|(x, inverse)| F::ONE - x * inverse);
        let slots = [
            Slot::Copy(x),
            Slot::Value(inverse),
            Slot::Value(bit),
            Slot::Empty,
        ];
        let selector = Some(self.config.s_is_zero);
        self.row_output(region, Coefficients::zero(), slots, selector, 2)
            .map(Bit::new)
    }

    /// `[x = y]` (a subtraction row and an is-zero row).
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn is_equal(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Word<F>,
        y: &Word<F>,
    ) -> Result<Bit<F>, Error> {
        let difference = self.sub(region, x, y)?;
        self.is_zero(region, &difference)
    }

    /// `1 - bit`.
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn not(&mut self, region: &mut Region<'_, F>, bit: &Bit<F>) -> Result<Bit<F>, Error> {
        let coefficients = Coefficients {
            a: F::ONE,
            b: F::ONE,
            k: -F::ONE,
            ..Coefficients::zero()
        };
        let out = bit.word().value().map(|bit| F::ONE - bit);
        let slots = [
            Slot::Copy(bit.word()),
            Slot::Value(out),
            Slot::Empty,
            Slot::Empty,
        ];
        self.row_output(region, coefficients, slots, None, 1)
            .map(Bit::new)
    }

    /// `x AND y` (a product of bits is a bit).
    ///
    /// # Errors
    ///
    /// [`Error`] from the layout.
    pub fn and(
        &mut self,
        region: &mut Region<'_, F>,
        x: &Bit<F>,
        y: &Bit<F>,
    ) -> Result<Bit<F>, Error> {
        self.mul(region, x.word(), y.word()).map(Bit::new)
    }
}

/// Native reference of [`GlueChip::select`].
#[must_use]
pub fn select_native<F: PastaField>(bit: bool, x: F, y: F) -> F {
    if bit { x } else { y }
}

/// Native reference of [`GlueChip::is_zero`].
#[must_use]
pub fn is_zero_native<F: PastaField>(x: &F) -> bool {
    bool::from(x.is_zero())
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use iroha_pasta::{Fp, Fq};

    use super::*;

    #[test]
    fn native_references() {
        assert_eq!(
            select_native(true, Fp::from(3u64), Fp::from(4u64)),
            Fp::from(3u64)
        );
        assert_eq!(
            select_native(false, Fq::from(3u64), Fq::from(4u64)),
            Fq::from(4u64)
        );
        assert!(is_zero_native(&Fp::ZERO));
        assert!(!is_zero_native(&Fq::ONE));
        let zero = Coefficients::<Fp>::zero();
        assert_eq!(zero.m, Fp::ZERO);
        assert_eq!(zero.k, Fp::ZERO);
    }
}
