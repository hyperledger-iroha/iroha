//! Four-row placement of the unchanged fused CRT residuals.
//!
//! Four equality-enabled ports hold, in order, three left limbs, three right
//! limbs, three result limbs, three quotient limbs and four offset carries.
//! The direct profile reads sixteen roots at rotations -1..2. The compact
//! staged profile binds four low convolutions and the native product in five
//! spare columns, then checks the same residuals using only -1, 0, 1. Its
//! streaming unsigned dot gate accumulates those same five quantities from a
//! constrained zero state. Every result/quotient/carry root is copied from an independent exact range
//! certificate; no integer bound or admission rule changes with this layout.

use super::{
    CARRIES, CARRY_OFFSET_BITS, ForeignModulus, FusedTerms, FusedWitness, LIMB_BITS, LIMBS, Mode,
    TOP_LIMB_BITS, fused_constraints, serialized::ranged,
};
use crate::{
    GlueChip, Word,
    cells::copy_word,
    phase::{Enable, PhaseColumns},
    range::RunningSumChip,
};
use iroha_pasta::PastaField;
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Expression, Rotation},
    frontend::{Error, Region, Value},
};

/// Fixed modulus and four-row CRT gates on the caller's existing Glue ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RotatedFfConfig {
    pub(super) columns: [Column<Advice>; 4],
    pub(super) modulus: ForeignModulus,
    mul: Enable,
    div: Enable,
    staged: Option<Staged>,
}

/// Five polynomial products carried to the following CRT-check row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Staged {
    sums: [Column<Advice>; 5],
    phases: PhaseColumns,
}
impl Staged {
    fn finish(self) -> Enable {
        self.phases.enable(1, Some((2, 4, 4)))
    }
    fn dot(self) -> Enable {
        self.phases.enable(1, Some((1, 3, 3)))
    }
    fn zero(self) -> Enable {
        self.phases.enable(1, Some((2, 3, 4)))
    }
    fn carry(self) -> Enable {
        self.phases.enable(1, Some((3, 3, 3)))
    }
}

impl RotatedFfConfig {
    /// Uses two ordinary selectors for the fixed modulus. Kernel rows and
    /// ordinary Glue rows must share one reservation cursor.
    pub fn configure<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        columns: [Column<Advice>; 4],
        modulus: ForeignModulus,
    ) -> Self {
        let mul = meta.selector().into();
        let div = meta.selector().into();
        Self::configure_on(meta, columns, modulus, mul, div, None)
    }
    /// Reuses ECC phase payload five codes3/4. The shared ECC configuration
    /// must use the same four-code domain for guard/split enables.
    pub fn configure_phased<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        columns: [Column<Advice>; 4],
        modulus: ForeignModulus,
        phases: PhaseColumns,
    ) -> Self {
        Self::configure_on(
            meta,
            columns,
            modulus,
            phases.enable(1, Some((5, 3, 4))),
            phases.enable(1, Some((5, 4, 4))),
            None,
        )
    }
    /// Places the same sixteen roots on four rows, staging the four low
    /// convolutions and native product in five spare columns. Every query
    /// uses rotations -1, 0 or 1. The spare columns need no copy argument.
    /// All nine columns must be distinct and belong to the same row owner.
    pub fn configure_staged_phased<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        columns: [Column<Advice>; 4],
        sums: [Column<Advice>; 5],
        modulus: ForeignModulus,
        phases: PhaseColumns,
    ) -> Self {
        assert!(
            columns
                .iter()
                .chain(&sums)
                .enumerate()
                .all(|(i, a)| { columns.iter().chain(&sums).skip(i + 1).all(|b| a != b) }),
            "CRT ports and temporary columns must be distinct"
        );
        Self::configure_on(
            meta,
            columns,
            modulus,
            phases.enable(1, Some((5, 3, 4))),
            phases.enable(1, Some((5, 4, 4))),
            Some(Staged { sums, phases }),
        )
    }
    fn configure_on<F: PastaField>(
        meta: &mut ConstraintSystem<F>,
        columns: [Column<Advice>; 4],
        modulus: ForeignModulus,
        mul: Enable,
        div: Enable,
        staged: Option<Staged>,
    ) -> Self {
        for column in columns {
            meta.enable_equality(column);
        }
        if let Some(staged) = staged {
            configure_staged(meta, columns, modulus, mul, div, staged);
            return Self {
                columns,
                modulus,
                mul,
                div,
                staged: Some(staged),
            };
        }
        for (name, selector, division) in [
            ("four-row CRT multiplication", mul, false),
            ("four-row CRT division", div, true),
        ] {
            meta.create_gate(name, |cells| {
                let values: [_; 16] = core::array::from_fn(|i| {
                    cells.query_advice(
                        columns[i % 4],
                        Rotation(i32::try_from(i / 4).expect("four rows") - 1),
                    )
                });
                let left = core::array::from_fn(|i| values[i].clone());
                let right = core::array::from_fn(|i| values[3 + i].clone());
                let result = core::array::from_fn(|i| values[6 + i].clone());
                let quotient = core::array::from_fn(|i| values[9 + i].clone());
                let carries = core::array::from_fn(|i| {
                    values[12 + i].clone() - super::constant::<F>(1 << CARRY_OFFSET_BITS)
                });
                let (p, r, s, padding) = if division {
                    (
                        right,
                        result,
                        left,
                        modulus
                            .division_padding()
                            .expect("supported modulus padding")
                            .1,
                    )
                } else {
                    (left, right, result, [0; LIMBS])
                };
                fused_constraints(
                    &selector.query(cells),
                    &FusedTerms {
                        p,
                        r,
                        s,
                        q: quotient,
                        u: carries,
                    },
                    modulus,
                    padding,
                )
            });
        }
        Self {
            columns,
            modulus,
            mul,
            div,
            staged,
        }
    }
    pub(super) fn constrain<F: PastaField>(
        &self,
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        mode: Mode,
        operands: (&[Word<F>; LIMBS], &[Word<F>; LIMBS]),
        witness: Value<FusedWitness<F>>,
    ) -> Result<[Word<F>; LIMBS], Error> {
        if glue.config().advice() != self.columns {
            return Err(Error::Synthesis);
        }
        let result = ranged(
            range,
            region,
            witness.map(|w| w.c),
            [LIMB_BITS, LIMB_BITS, TOP_LIMB_BITS],
        )?;
        let quotient = ranged(range, region, witness.map(|w| w.q), [LIMB_BITS; LIMBS])?;
        let carries = ranged(
            range,
            region,
            witness.map(|w| w.u),
            [CARRY_OFFSET_BITS + 1; CARRIES],
        )?;
        let start = glue.reserve_shared_rows(4)?;
        match mode {
            Mode::Mul => self.mul,
            Mode::Div => self.div,
        }
        .enable(region, start + 1)?;
        let ordered = if self.staged.is_some() && matches!(mode, Mode::Div) {
            (operands.1, &result, operands.0)
        } else {
            (operands.0, operands.1, &result)
        };
        for (i, word) in ordered
            .0
            .iter()
            .chain(ordered.1)
            .chain(ordered.2)
            .chain(&quotient)
            .chain(&carries)
            .enumerate()
        {
            copy_word(region, word, self.columns[i % 4], start + i / 4)?;
        }
        if let Some(staged) = self.staged {
            staged.finish().enable(region, start + 2)?;
            let padding = if matches!(mode, Mode::Div) {
                self.modulus.division_padding().ok_or(Error::Synthesis)?.1
            } else {
                [0; LIMBS]
            };
            let values =
                ordered
                    .0
                    .iter()
                    .chain(ordered.1)
                    .fold(Value::known(Vec::new()), |values, word| {
                        values.zip(word.value()).map(|(mut values, value)| {
                            values.push(value);
                            values
                        })
                    });
            let values = values.map(|values| product_values(&values[..3], &values[3..], padding));
            for (i, column) in staged.sums.into_iter().enumerate() {
                region.assign_advice(column, start + 2, values.map(|v| v[i]))?;
            }
        }
        Ok(result)
    }

    pub(super) fn dot<F: PastaField>(
        &self,
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        pairs: &[(&super::FfValue<F>, &super::FfValue<F>)],
    ) -> Result<Option<super::FfValue<F>>, Error> {
        if self.staged.is_none() {
            return Ok(None);
        }
        if glue.config().advice() != self.columns || super::dot::admitted(pairs)? != self.modulus {
            return Err(Error::Synthesis);
        }
        let values = pairs
            .iter()
            .fold(Value::known(Vec::new()), |values, (a, b)| {
                values
                    .zip(a.limb_values())
                    .zip(b.limb_values())
                    .map(|((mut values, a), b)| {
                        values.push((a, b));
                        values
                    })
            });
        let witness = values.map(|values| super::dot::witness::<F>(self.modulus, &values));
        self.constrain_dot(glue, range, region, pairs, witness)
    }

    pub(super) fn constrain_dot<F: PastaField>(
        &self,
        glue: &mut GlueChip<F>,
        range: &mut RunningSumChip<F>,
        region: &mut Region<'_, F>,
        pairs: &[(&super::FfValue<F>, &super::FfValue<F>)],
        witness: Value<FusedWitness<F>>,
    ) -> Result<Option<super::FfValue<F>>, Error> {
        let Some(staged) = self.staged else {
            return Ok(None);
        };
        if glue.config().advice() != self.columns || super::dot::admitted(pairs)? != self.modulus {
            return Err(Error::Synthesis);
        }
        let result = ranged(
            range,
            region,
            witness.map(|w| w.c),
            [LIMB_BITS, LIMB_BITS, TOP_LIMB_BITS],
        )?;
        let quotient = ranged(range, region, witness.map(|w| w.q), [LIMB_BITS; LIMBS])?;
        let carries = ranged(
            range,
            region,
            witness.map(|w| w.u),
            [CARRY_OFFSET_BITS + 1; CARRIES],
        )?;
        let start = glue.reserve_shared_rows(2 * pairs.len() + 3)?;
        staged.zero().enable(region, start + 1)?;
        let mut sums = Value::known([F::ZERO; 5]);
        for (i, (left, right)) in pairs.iter().enumerate() {
            let row = start + 2 * i;
            for (j, word) in left.limbs.iter().chain(&right.limbs).enumerate() {
                copy_word(region, word, self.columns[j % 4], row + j / 4)?;
            }
            staged.dot().enable(region, row + 1)?;
            for (j, column) in staged.sums.iter().enumerate() {
                region.assign_advice(*column, row + 1, sums.map(|s| s[j]))?;
            }
            let values = left.limbs.iter().chain(&right.limbs).fold(
                Value::known(Vec::new()),
                |values, word| {
                    values.zip(word.value()).map(|(mut values, word)| {
                        values.push(word);
                        values
                    })
                },
            );
            sums = sums.zip(values).map(|(sums, values)| {
                let products = product_values(&values[..3], &values[3..], [0; LIMBS]);
                core::array::from_fn(|j| sums[j] + products[j])
            });
            for (j, column) in staged.sums.iter().enumerate() {
                region.assign_advice(*column, row + 2, sums.map(|s| s[j]))?;
            }
            staged.carry().enable(region, row + 2)?;
        }
        let finish = start + 2 * pairs.len() + 1;
        for (j, column) in staged.sums.iter().enumerate() {
            region.assign_advice(*column, finish, sums.map(|s| s[j]))?;
        }
        staged.finish().enable(region, finish)?;
        for (i, word) in result.iter().chain(&quotient).chain(&carries).enumerate() {
            let index = i + 6;
            copy_word(
                region,
                word,
                self.columns[index % 4],
                finish - 2 + index / 4,
            )?;
        }
        Ok(Some(super::FfValue::from_parts(
            result,
            super::PROPER_BOUNDS,
            self.modulus,
            super::Form::Proper,
        )))
    }
}

fn product_values<F: PastaField>(left: &[F], right: &[F], padding: [u128; LIMBS]) -> [F; 5] {
    let radix = F::from_u128(1 << LIMB_BITS);
    let native = |limbs: &[F]| {
        limbs
            .iter()
            .rev()
            .fold(F::ZERO, |sum, limb| sum * radix + limb)
    };
    core::array::from_fn(|column| {
        if column == 4 {
            native(left) * native(right) + native(&padding.map(F::from_u128))
        } else {
            let mut value = padding.get(column).map_or(F::ZERO, |x| F::from_u128(*x));
            for (i, a) in left.iter().enumerate() {
                if let Some(j) = column.checked_sub(i).filter(|j| *j < LIMBS) {
                    value += *a * right[j];
                }
            }
            value
        }
    })
}

fn configure_staged<F: PastaField>(
    meta: &mut ConstraintSystem<F>,
    ports: [Column<Advice>; 4],
    modulus: ForeignModulus,
    mul: Enable,
    div: Enable,
    staged: Staged,
) {
    meta.create_gate("unsigned dot starts at zero", |cells| {
        let enabled = staged.zero().query(cells);
        staged
            .sums
            .map(|column| enabled.clone() * cells.query_advice(column, Rotation::cur()))
    });
    meta.create_gate("unsigned dot carries five sums", |cells| {
        let enabled = staged.carry().query(cells);
        staged.sums.map(|column| {
            enabled.clone()
                * (cells.query_advice(column, Rotation::next())
                    - cells.query_advice(column, Rotation::cur()))
        })
    });
    meta.create_gate("unsigned dot product step", |cells| {
        let roots: [_; 6] = core::array::from_fn(|i| {
            cells.query_advice(
                ports[i % 4],
                Rotation(i32::try_from(i / 4).expect("two rows") - 1),
            )
        });
        let enabled = staged.dot().query(cells);
        staged
            .sums
            .iter()
            .enumerate()
            .map(|(column, advice)| {
                let product = if column == 4 {
                    super::recompose(&roots[..3]) * super::recompose(&roots[3..])
                } else {
                    let mut value = Expression::Constant(F::ZERO);
                    for i in 0..LIMBS {
                        if let Some(j) = column.checked_sub(i).filter(|j| *j < LIMBS) {
                            value = value + roots[i].clone() * roots[3 + j].clone();
                        }
                    }
                    value
                };
                enabled.clone()
                    * (cells.query_advice(*advice, Rotation::next())
                        - cells.query_advice(*advice, Rotation::cur())
                        - product)
            })
            .collect::<Vec<_>>()
    });
    for (enable, padding) in [
        (mul, [0; LIMBS]),
        (
            div,
            modulus.division_padding().expect("supported modulus").1,
        ),
    ] {
        meta.create_gate("staged CRT products", |cells| {
            let roots: [_; 6] = core::array::from_fn(|i| {
                cells.query_advice(
                    ports[i % 4],
                    Rotation(i32::try_from(i / 4).expect("two rows") - 1),
                )
            });
            let q = enable.query(cells);
            let products: [_; 5] = core::array::from_fn(|column| {
                if column == 4 {
                    super::recompose(&roots[..3]) * super::recompose(&roots[3..])
                        + Expression::Constant(super::from_limbs(&padding).to_field::<F>())
                } else {
                    let mut value = Expression::Constant(
                        padding.get(column).map_or(F::ZERO, |x| F::from_u128(*x)),
                    );
                    for i in 0..LIMBS {
                        if let Some(j) = column.checked_sub(i).filter(|j| *j < LIMBS) {
                            value = value + roots[i].clone() * roots[3 + j].clone();
                        }
                    }
                    value
                }
            });
            staged
                .sums
                .iter()
                .zip(products)
                .map(|(column, expected)| {
                    q.clone() * (cells.query_advice(*column, Rotation::next()) - expected)
                })
                .collect::<Vec<_>>()
        });
    }
    meta.create_gate("staged CRT residuals", |cells| {
        let roots: [_; 10] = core::array::from_fn(|i| {
            let index = i + 6;
            cells.query_advice(
                ports[index % 4],
                Rotation(i32::try_from(index / 4).expect("four rows") - 2),
            )
        });
        let products = staged
            .sums
            .map(|column| cells.query_advice(column, Rotation::cur()));
        let carry = core::array::from_fn::<_, CARRIES, _>(|i| {
            roots[6 + i].clone() - super::constant::<F>(1 << CARRY_OFFSET_BITS)
        });
        let radix = F::from_u128(1 << LIMB_BITS);
        let m = modulus.limbs();
        let q = staged.finish().query(cells);
        let mut residuals = Vec::new();
        for column in 0..CARRIES {
            let mut value = products[column].clone() - carry[column].clone() * radix;
            if column < LIMBS {
                value = value - roots[column].clone();
            }
            if column > 0 {
                value = value + carry[column - 1].clone();
            }
            for i in 0..LIMBS {
                if let Some(j) = column.checked_sub(i).filter(|j| *j < LIMBS) {
                    value = value - roots[3 + i].clone() * F::from_u128(m[j]);
                }
            }
            residuals.push(q.clone() * value);
        }
        residuals.push(
            q * (products[4].clone()
                - super::recompose(&roots[..3])
                - super::recompose(&roots[3..6]) * modulus.nat().to_field::<F>()),
        );
        residuals
    });
}
