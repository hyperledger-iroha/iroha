//! Shared fixed-row carrier binding with injective Mersenne encoding.
//!
//! This internal machine evaluates the complete ordered remainder and ternary quotient-pack
//! polynomial at two constrained challenges. Its capacity, carrier count and bounded pack owner
//! are supplied by each protocol. It confers no authority without authentic proof instance
//! commitments, transcript-bound challenges, and reciprocal consumer verification.

use crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1;
use ff::PrimeField;
use halo2_base::{
    AssignedValue,
    gates::circuit::{BaseConfig, MaybeRangeConfig},
};
use halo2_proofs::{
    circuit::{Cell, Layouter, Value},
    plonk::{
        Advice, Column, ConstraintSystem, Error as PlonkError, Expression, Fixed, TableColumn,
    },
    poly::Rotation,
};
use p256::elliptic_curve::bigint::{Encoding as _, NonZero, U256};

pub(super) const CARRIER_RLC_CHALLENGE_BITS_V1: usize = 125;
pub(super) const CARRIER_RLC_MODULUS_V1: u128 = (1_u128 << 127) - 1;
/// Quotients of a `u128` by the 127-bit RLC modulus are ternary digits.
/// Packing eighty of them is canonical because `3^80 - 1 < 2^127 - 1`.
pub(super) const CARRIER_RLC_QUOTIENT_RADIX_V1: u128 = 3;
pub(super) const CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1: usize = 80;
// The common-prime carrier binding used to run through BaseCircuitBuilder. At the real claim
// width, its generic vertical gates occupied another ~4 million advice cells and materialized one
// selector plus one permutation column for almost every 65,527-row slice. This fixed machine keeps
// the same canonical two-challenge polynomial, but schedules all divisions in one narrow region.
// Only BUS participates in the permutation argument; the remaining columns are local state.
pub(super) const CARRIER_RLC_BUS: usize = 0;
pub(super) const CARRIER_RLC_VALUE: usize = 1;
pub(super) const CARRIER_RLC_COEFFICIENT: usize = 2;
pub(super) const CARRIER_RLC_QUOTIENT_BIT_0: usize = 3;
pub(super) const CARRIER_RLC_QUOTIENT_BIT_1: usize = 4;
pub(super) const CARRIER_RLC_RAW_REMAINDER: usize = 5;
pub(super) const CARRIER_RLC_REMAINDER_INVERSE: usize = 6;
pub(super) const CARRIER_RLC_CHALLENGE_A: usize = 7;
pub(super) const CARRIER_RLC_CHALLENGE_B: usize = 8;
pub(super) const CARRIER_RLC_ACCUMULATOR_A: usize = 9;
pub(super) const CARRIER_RLC_ACCUMULATOR_B: usize = 10;
pub(super) const CARRIER_RLC_QUOTIENT_PACK: usize = 11;
pub(super) const CARRIER_RLC_DIVISION_QUOTIENT: usize = 12;
pub(super) const CARRIER_RLC_DIVISION_REMAINDER: usize = 13;
pub(super) const CARRIER_RLC_RANGE_START: usize = 14;
pub(super) const CARRIER_RLC_RANGE_LIMBS: usize = 18;
pub(super) const CARRIER_RLC_SCALED_FIRST_TOP: usize =
    CARRIER_RLC_RANGE_START + CARRIER_RLC_RANGE_LIMBS;
pub(super) const CARRIER_RLC_SCALED_SECOND_TOP: usize = CARRIER_RLC_SCALED_FIRST_TOP + 1;
pub(super) const CARRIER_RLC_COLUMNS: usize = CARRIER_RLC_SCALED_SECOND_TOP + 1;
// Logical witness records remain unchanged for the host oracle and mutation fixtures.
// Each record is assigned into two physical rows with one shared ten-column range bank.
pub(super) const CARRIER_RLC_ROWS_PER_LOGICAL_ROW: usize = 2;
pub(super) const CARRIER_RLC_PHYSICAL_STATE_COLUMNS: usize = 4;
pub(super) const CARRIER_RLC_PHYSICAL_RANGE_COLUMNS: usize = 10;
pub(super) const CARRIER_RLC_PHYSICAL_COLUMNS: usize =
    CARRIER_RLC_PHYSICAL_STATE_COLUMNS + CARRIER_RLC_PHYSICAL_RANGE_COLUMNS;
// (logical value, physical column, half-row). Only the head BUS has external copies.
pub(super) const CARRIER_RLC_STATE_LAYOUT: [(usize, usize, usize); 8] = [
    (CARRIER_RLC_BUS, 0, 0),
    (CARRIER_RLC_CHALLENGE_A, 1, 0),
    (CARRIER_RLC_CHALLENGE_B, 2, 0),
    (CARRIER_RLC_ACCUMULATOR_A, 3, 0),
    (CARRIER_RLC_REMAINDER_INVERSE, 0, 1),
    (CARRIER_RLC_COEFFICIENT, 1, 1),
    (CARRIER_RLC_ACCUMULATOR_B, 2, 1),
    (CARRIER_RLC_QUOTIENT_PACK, 3, 1),
];
pub(super) const CARRIER_RLC_RADIX_BITS: usize = 15;
pub(super) const CARRIER_RLC_RADIX: u128 = 1_u128 << CARRIER_RLC_RADIX_BITS;

#[derive(Clone, Debug)]
pub(super) struct KagemushaCarrierRlcConfigV1 {
    pub(super) advice: [Column<Advice>; CARRIER_RLC_PHYSICAL_COLUMNS],
    pub(super) range_table: TableColumn,
    pub(super) owns_range_table: bool,
    pub(super) mode_bit_0: Column<Fixed>,
    pub(super) mode_bit_1: Column<Fixed>,
    pub(super) payload: Column<Fixed>,
}

impl KagemushaCarrierRlcConfigV1 {
    #[cfg(test)]
    pub(super) fn configure<F: KagemushaPoseidonFieldV1>(meta: &mut ConstraintSystem<F>) -> Self {
        Self::configure_with_base(meta, None)
    }

    pub(super) fn configure_with_base<F: KagemushaPoseidonFieldV1>(
        meta: &mut ConstraintSystem<F>,
        base: Option<&BaseConfig<F>>,
    ) -> Self {
        let advice = std::array::from_fn(|_| meta.advice_column());
        meta.enable_equality(advice[CARRIER_RLC_BUS]);
        // Base synthesis assigns its table before this machine. Share only an actually
        // configured table with the exact same 15-bit contents, and assign it exactly once.
        // Base can omit its range table when no Base lookup advice is configured, or use a
        // different range width; those configurations retain this machine's owned table.
        let shared_range_table = base.and_then(|base| match &base.base {
            MaybeRangeConfig::WithRange(range) if range.lookup_bits() == CARRIER_RLC_RADIX_BITS => {
                Some(range.lookup)
            }
            _ => None,
        });
        let owns_range_table = shared_range_table.is_none();
        let range_table = shared_range_table.unwrap_or_else(|| meta.lookup_table_column());
        // Two fixed mode bits select inactive/boundary/preprocess/evaluate rows. The third fixed
        // column is a mode-local payload: a boundary subtype, the preprocess ternary power, or an
        // evaluate-side/load/store opcode. Unassigned rows decode as inactive (0, 0, 0).
        let mode_bit_0 = meta.fixed_column();
        let mode_bit_1 = meta.fixed_column();
        let payload = meta.fixed_column();

        meta.create_gate("Kagemusha claim carrier RLC state machine", |meta| {
            let mut current: [Expression<F>; CARRIER_RLC_COLUMNS] =
                std::array::from_fn(|_| Expression::Constant(F::ZERO));
            let mut next: [Expression<F>; CARRIER_RLC_COLUMNS] =
                std::array::from_fn(|_| Expression::Constant(F::ZERO));
            for (logical, physical, half) in CARRIER_RLC_STATE_LAYOUT {
                current[logical] = meta.query_advice(advice[physical], Rotation(half as i32));
                if logical != CARRIER_RLC_BUS && logical != CARRIER_RLC_REMAINDER_INVERSE {
                    next[logical] = meta.query_advice(
                        advice[physical],
                        Rotation((half + CARRIER_RLC_ROWS_PER_LOGICAL_ROW) as i32),
                    );
                }
            }
            for half in 0..CARRIER_RLC_ROWS_PER_LOGICAL_ROW {
                for limb in 0..9 {
                    current[CARRIER_RLC_RANGE_START + half * 9 + limb] = meta.query_advice(
                        advice[CARRIER_RLC_PHYSICAL_STATE_COLUMNS + limb],
                        Rotation(half as i32),
                    );
                }
                current[CARRIER_RLC_SCALED_FIRST_TOP + half] = meta.query_advice(
                    advice[CARRIER_RLC_PHYSICAL_COLUMNS - 1],
                    Rotation(half as i32),
                );
            }
            let one = Expression::Constant(F::ONE);
            let zero = Expression::Constant(F::ZERO);
            let two = Expression::Constant(F::from(2));
            let three = Expression::Constant(F::from(3));
            let inverse_two = Expression::Constant(F::from(2).invert().unwrap());
            let inverse_six = Expression::Constant(F::from(6).invert().unwrap());
            let bit_0 = meta.query_fixed(mode_bit_0, Rotation::cur());
            let bit_1 = meta.query_fixed(mode_bit_1, Rotation::cur());
            let power = meta.query_fixed(payload, Rotation::cur());

            let boundary = (one.clone() - bit_0.clone()) * bit_1.clone();
            let preprocess = bit_0.clone() * (one.clone() - bit_1.clone());
            let evaluate = bit_0 * bit_1;
            let payload_minus_one = power.clone() - one.clone();
            let payload_minus_two = power.clone() - two.clone();
            let payload_minus_three = power.clone() - three;
            let payload_plus_one = power.clone() + one.clone();

            // Boundary payload 0/1/2/3 selects start-A/start-B/end-A/end-B. Cubic Lagrange
            // indicators multiplied by the quadratic boundary mode keep every gated relation at
            // degree six or less.
            let start_a = boundary.clone()
                * (zero.clone()
                    - payload_minus_one.clone()
                        * payload_minus_two.clone()
                        * payload_minus_three.clone()
                        * inverse_six.clone());
            let start_b = boundary.clone()
                * power.clone()
                * payload_minus_two.clone()
                * payload_minus_three.clone()
                * inverse_two.clone();
            let end_a = boundary.clone()
                * (zero.clone()
                    - power.clone()
                        * payload_minus_one.clone()
                        * payload_minus_three
                        * inverse_two.clone());
            let end_b = boundary
                * power.clone()
                * payload_minus_one.clone()
                * payload_minus_two.clone()
                * inverse_six.clone();

            // Evaluation payload 0/1 is side A (normal/load); 2/-1 is side B
            // (normal/store). The two exceptional cubic indicators recover load/store directly.
            let evaluate_b_side = power.clone() * payload_minus_one.clone() * inverse_two.clone();
            let evaluate_a = evaluate.clone() * (one.clone() - evaluate_b_side.clone());
            let evaluate_b = evaluate.clone() * evaluate_b_side;
            let load_pack = evaluate.clone()
                * (zero.clone()
                    - power.clone() * payload_minus_two.clone() * payload_plus_one * inverse_two);
            let store_pack = evaluate.clone()
                * (zero - power.clone() * payload_minus_one * payload_minus_two * inverse_six);
            let modulus = Expression::Constant(F::from_u128(CARRIER_RLC_MODULUS_V1));
            let transition = start_a.clone()
                + start_b.clone()
                + preprocess.clone()
                + evaluate_a.clone()
                + evaluate_b.clone()
                + end_a.clone();
            let idle = start_a.clone() + start_b.clone() + end_a.clone();

            let compose = |limbs: std::ops::Range<usize>| {
                limbs
                    .enumerate()
                    .fold(Expression::Constant(F::ZERO), |sum, (position, index)| {
                        sum + current[CARRIER_RLC_RANGE_START + index].clone()
                            * Expression::Constant(F::from_u128(
                                1_u128 << (CARRIER_RLC_RADIX_BITS * position),
                            ))
                    })
            };
            let first_range = compose(0..9);
            let second_range = compose(9..18);
            let first_top = current[CARRIER_RLC_RANGE_START + 8].clone();
            let second_top = current[CARRIER_RLC_RANGE_START + 17].clone();
            // On preprocess rows V and R are the independently range-constrained integers.
            // q=(V-R)/M is ternary, and V=q*M+R cannot wrap in either Pasta field:
            // V<2^128, R<M and q in {0,1,2} imply both integer sides are below 2^129.
            let quotient = (first_range.clone() - second_range.clone())
                * Expression::Constant(
                    F::from_u128(CARRIER_RLC_MODULUS_V1)
                        .invert()
                        .expect("claim RLC modulus is nonzero"),
                );
            vec![
                start_a.clone()
                    * (current[CARRIER_RLC_CHALLENGE_A].clone() - current[CARRIER_RLC_BUS].clone()),
                start_b.clone()
                    * (current[CARRIER_RLC_CHALLENGE_B].clone() - current[CARRIER_RLC_BUS].clone()),
                start_a.clone() * current[CARRIER_RLC_ACCUMULATOR_A].clone(),
                start_a.clone() * current[CARRIER_RLC_ACCUMULATOR_B].clone(),
                start_a.clone() * current[CARRIER_RLC_QUOTIENT_PACK].clone(),
                end_a.clone()
                    * (current[CARRIER_RLC_ACCUMULATOR_A].clone()
                        - current[CARRIER_RLC_BUS].clone()),
                end_b.clone()
                    * (current[CARRIER_RLC_ACCUMULATOR_B].clone()
                        - current[CARRIER_RLC_BUS].clone()),
                transition.clone()
                    * (next[CARRIER_RLC_CHALLENGE_A].clone()
                        - current[CARRIER_RLC_CHALLENGE_A].clone()),
                transition
                    * (next[CARRIER_RLC_CHALLENGE_B].clone()
                        - current[CARRIER_RLC_CHALLENGE_B].clone()),
                idle.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_A].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_A].clone()),
                idle.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_B].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_B].clone()),
                idle * (next[CARRIER_RLC_QUOTIENT_PACK].clone()
                    - current[CARRIER_RLC_QUOTIENT_PACK].clone()),
                preprocess.clone() * (current[CARRIER_RLC_BUS].clone() - first_range.clone()),
                preprocess.clone()
                    * quotient.clone()
                    * (quotient.clone() - one.clone())
                    * (quotient.clone() - two),
                preprocess.clone()
                    * ((second_range.clone() - modulus.clone())
                        * current[CARRIER_RLC_REMAINDER_INVERSE].clone()
                        - one.clone()),
                preprocess.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_A].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_A].clone()),
                preprocess.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_B].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_B].clone()),
                preprocess.clone() * (next[CARRIER_RLC_COEFFICIENT].clone() - second_range.clone()),
                preprocess.clone()
                    * (next[CARRIER_RLC_QUOTIENT_PACK].clone()
                        - current[CARRIER_RLC_QUOTIENT_PACK].clone()
                        - quotient * power),
                evaluate.clone()
                    * ((second_range.clone() - modulus.clone())
                        * current[CARRIER_RLC_REMAINDER_INVERSE].clone()
                        - one),
                preprocess.clone()
                    * (current[CARRIER_RLC_SCALED_FIRST_TOP].clone()
                        - first_top.clone() * Expression::Constant(F::from(128))),
                evaluate.clone()
                    * (current[CARRIER_RLC_SCALED_FIRST_TOP].clone()
                        - first_top * Expression::Constant(F::from(512))),
                (preprocess + evaluate.clone())
                    * (current[CARRIER_RLC_SCALED_SECOND_TOP].clone()
                        - second_top * Expression::Constant(F::from(256))),
                evaluate_a.clone()
                    * (current[CARRIER_RLC_ACCUMULATOR_A].clone()
                        * current[CARRIER_RLC_CHALLENGE_A].clone()
                        + current[CARRIER_RLC_COEFFICIENT].clone()
                        - first_range.clone() * modulus.clone()
                        - second_range.clone()),
                evaluate_b.clone()
                    * (current[CARRIER_RLC_ACCUMULATOR_B].clone()
                        * current[CARRIER_RLC_CHALLENGE_B].clone()
                        + current[CARRIER_RLC_COEFFICIENT].clone()
                        - first_range.clone() * modulus
                        - second_range.clone()),
                evaluate_a.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_A].clone() - second_range.clone()),
                evaluate_a.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_B].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_B].clone()),
                evaluate_a.clone()
                    * (next[CARRIER_RLC_COEFFICIENT].clone()
                        - current[CARRIER_RLC_COEFFICIENT].clone()),
                evaluate_a
                    * (next[CARRIER_RLC_QUOTIENT_PACK].clone()
                        - current[CARRIER_RLC_QUOTIENT_PACK].clone()),
                evaluate_b.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_A].clone()
                        - current[CARRIER_RLC_ACCUMULATOR_A].clone()),
                evaluate_b.clone()
                    * (next[CARRIER_RLC_ACCUMULATOR_B].clone() - second_range.clone()),
                evaluate_b.clone()
                    * (next[CARRIER_RLC_QUOTIENT_PACK].clone()
                        - current[CARRIER_RLC_QUOTIENT_PACK].clone())
                    + store_pack.clone() * current[CARRIER_RLC_QUOTIENT_PACK].clone(),
                store_pack
                    * (current[CARRIER_RLC_BUS].clone()
                        - current[CARRIER_RLC_QUOTIENT_PACK].clone()),
                load_pack
                    * (current[CARRIER_RLC_BUS].clone() - current[CARRIER_RLC_COEFFICIENT].clone()),
            ]
        });

        // Each physical column is independently range-checked on both row halves.
        // The last column is the scaled high limb. These remain unconditional linear
        // lookups, so a tuple lookup cannot accidentally weaken either integer bound.
        for position in 0..CARRIER_RLC_PHYSICAL_RANGE_COLUMNS {
            meta.lookup("Kagemusha claim carrier RLC range limb", |meta| {
                let cell = meta.query_advice(
                    advice[CARRIER_RLC_PHYSICAL_STATE_COLUMNS + position],
                    Rotation::cur(),
                );
                vec![(cell, range_table)]
            });
        }

        Self {
            advice,
            range_table,
            owns_range_table,
            mode_bit_0,
            mode_bit_1,
            payload,
        }
    }

    pub(super) fn load_range_table<F: KagemushaPoseidonFieldV1>(
        &self,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        if !self.owns_range_table {
            return Ok(());
        }
        layouter.assign_table(
            || "Kagemusha claim carrier RLC range",
            |mut table| {
                for value in 0..CARRIER_RLC_RADIX as usize {
                    table.assign_cell(
                        || "claim carrier RLC range value",
                        self.range_table,
                        value,
                        || Value::known(F::from(value as u64)),
                    )?;
                }
                Ok(())
            },
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CarrierRlcRowModeV1 {
    StartA,
    StartB,
    Preprocess,
    EvaluateA,
    EvaluateB,
    EndA,
    EndB,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum CarrierRlcBusBindingV1<F: PrimeField> {
    Virtual(AssignedValue<F>),
    PackStore { carrier: usize, pack: usize },
    PackLoad { carrier: usize, pack: usize },
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) struct CarrierRlcRawRowV1<F: PrimeField> {
    pub(super) values: [F; CARRIER_RLC_COLUMNS],
    pub(super) mode: CarrierRlcRowModeV1,
    pub(super) store_pack: bool,
    pub(super) load_pack: bool,
    pub(super) ternary_power: F,
    pub(super) binding: Option<CarrierRlcBusBindingV1<F>>,
    #[cfg(test)]
    pub(super) physical_mutation: Option<CarrierRlcPhysicalMutationV1>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum CarrierRlcPhysicalMutationV1 {
    Advice { half: usize, column: usize },
    Fixed { half: usize, column: usize },
}

#[cfg(test)]
pub(super) fn carrier_rlc_physical_values_v1<F: PrimeField>(
    row: &CarrierRlcRawRowV1<F>,
) -> [[F; CARRIER_RLC_PHYSICAL_COLUMNS]; CARRIER_RLC_ROWS_PER_LOGICAL_ROW] {
    let mut values = [[F::ZERO; CARRIER_RLC_PHYSICAL_COLUMNS]; CARRIER_RLC_ROWS_PER_LOGICAL_ROW];
    for (logical, physical, half) in CARRIER_RLC_STATE_LAYOUT {
        values[half][physical] = row.values[logical];
    }
    for (half, values) in values.iter_mut().enumerate() {
        values[CARRIER_RLC_PHYSICAL_STATE_COLUMNS..CARRIER_RLC_PHYSICAL_COLUMNS - 1]
            .copy_from_slice(
                &row.values
                    [CARRIER_RLC_RANGE_START + 9 * half..CARRIER_RLC_RANGE_START + 9 * (half + 1)],
            );
        values[CARRIER_RLC_PHYSICAL_COLUMNS - 1] = row.values[CARRIER_RLC_SCALED_FIRST_TOP + half];
    }
    values
}

#[cfg(test)]
pub(super) fn carrier_rlc_fixed_encoding_v1<F: PrimeField>(
    row: &CarrierRlcRawRowV1<F>,
) -> Result<[F; 3], String> {
    if row.store_pack && !matches!(row.mode, CarrierRlcRowModeV1::EvaluateB) {
        return Err("claim RLC store opcode is not on an evaluation-B row".to_owned());
    }
    if row.load_pack && !matches!(row.mode, CarrierRlcRowModeV1::EvaluateA) {
        return Err("claim RLC load opcode is not on an evaluation-A row".to_owned());
    }
    if !matches!(row.mode, CarrierRlcRowModeV1::Preprocess) && row.ternary_power != F::ZERO {
        return Err("claim RLC ternary power is not on a preprocess row".to_owned());
    }

    let encoding = match row.mode {
        CarrierRlcRowModeV1::StartA => [F::ZERO, F::ONE, F::ZERO],
        CarrierRlcRowModeV1::StartB => [F::ZERO, F::ONE, F::ONE],
        CarrierRlcRowModeV1::Preprocess => {
            if row.ternary_power == F::ZERO {
                return Err("claim RLC preprocess ternary power is zero".to_owned());
            }
            [F::ONE, F::ZERO, row.ternary_power]
        }
        CarrierRlcRowModeV1::EvaluateA => {
            [F::ONE, F::ONE, if row.load_pack { F::ONE } else { F::ZERO }]
        }
        CarrierRlcRowModeV1::EvaluateB => [
            F::ONE,
            F::ONE,
            if row.store_pack {
                F::ZERO - F::ONE
            } else {
                F::from(2)
            },
        ],
        CarrierRlcRowModeV1::EndA => [F::ZERO, F::ONE, F::from(2)],
        CarrierRlcRowModeV1::EndB => [F::ZERO, F::ONE, F::from(3)],
    };
    Ok(encoding)
}

#[derive(Clone, Copy)]
pub(super) struct CarrierRlcStateV1 {
    pub(super) challenge_a: u128,
    pub(super) challenge_b: u128,
    pub(super) accumulator_a: u128,
    pub(super) accumulator_b: u128,
    pub(super) quotient_pack: u128,
    pub(super) coefficient: u128,
}

#[derive(Clone, Debug)]
pub(super) struct CarrierRlcCarrierV1<F: PrimeField> {
    pub(super) values: Vec<AssignedValue<F>>,
    pub(super) expected_a: AssignedValue<F>,
    pub(super) expected_b: AssignedValue<F>,
}

#[derive(Clone, Debug)]
pub(super) struct KagemushaCarrierRlcMachineV1<
    F: KagemushaPoseidonFieldV1,
    const CAPACITY: usize,
    const CARRIERS: usize,
    const PACKS: usize,
> {
    pub(super) challenge_a: AssignedValue<F>,
    pub(super) challenge_b: AssignedValue<F>,
    pub(super) carriers: [CarrierRlcCarrierV1<F>; CARRIERS],
    pub(super) use_unknown: bool,
}

impl<F: KagemushaPoseidonFieldV1, const CAPACITY: usize, const CARRIERS: usize, const PACKS: usize>
    KagemushaCarrierRlcMachineV1<F, CAPACITY, CARRIERS, PACKS>
{
    pub(super) fn unknown(&self) -> Self {
        let mut unknown = self.clone();
        unknown.use_unknown = true;
        unknown
    }

    pub(super) fn required_rows(&self) -> Result<usize, String> {
        self.required_rows_with_capacity(CAPACITY)
    }

    pub(super) fn required_rows_with_capacity(
        &self,
        fixed_capacity: usize,
    ) -> Result<usize, String> {
        if CAPACITY == 0
            || CARRIERS == 0
            || PACKS != CAPACITY.div_ceil(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1)
            || fixed_capacity == 0
            || fixed_capacity > CAPACITY
        {
            return Err(
                "carrier RLC fixed capacity, carrier count or pack owner is invalid".to_owned(),
            );
        }
        let fixed_packs = fixed_capacity.div_ceil(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
        self.carriers.iter().try_fold(0_usize, |total, carrier| {
            if carrier.values.len() != fixed_capacity {
                return Err(
                    "mint-hash claim RLC carrier does not fill its fixed schedule".to_owned(),
                );
            }
            total
                .checked_add(
                    fixed_capacity
                        .checked_mul(3)
                        .and_then(|rows| {
                            fixed_packs
                                .checked_mul(2)
                                .and_then(|packs| rows.checked_add(packs))
                        })
                        .and_then(|rows| rows.checked_add(4))
                        .and_then(|rows| rows.checked_mul(CARRIER_RLC_ROWS_PER_LOGICAL_ROW))
                        .ok_or_else(|| "carrier RLC row count overflowed".to_owned())?,
                )
                .ok_or_else(|| "mint-hash claim RLC row count overflowed".to_owned())
        })
    }

    pub(super) fn validate_capacity(&self, usable_rows: usize) -> Result<(), String> {
        let required = self.required_rows()?;
        if required > usable_rows {
            return Err(format!(
                "mint-hash claim RLC requires {required} rows, exceeding {usable_rows}"
            ));
        }
        Ok(())
    }

    pub(super) fn synthesize(
        &self,
        config: &KagemushaCarrierRlcConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        usable_rows: usize,
    ) -> Result<(), PlonkError> {
        self.validate_capacity(usable_rows)
            .map_err(|_| PlonkError::Synthesis)?;
        config.load_range_table(layouter)?;
        streaming::synthesize_with_capacity(
            self,
            config,
            layouter,
            copy_manager,
            witness_gen_only,
            CAPACITY,
        )
    }

    // Frozen vector oracle retained for existing mutation tests and streaming regressions.
    #[cfg(test)]
    pub(super) fn synthesize_rows(
        &self,
        config: &KagemushaCarrierRlcConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        rows: &[CarrierRlcRawRowV1<F>],
    ) -> Result<(), PlonkError> {
        let physical_cells = if witness_gen_only {
            None
        } else {
            Some(copy_manager.lock().map_err(|_| PlonkError::Synthesis)?)
        };
        layouter.assign_region(
            || "Kagemusha claim carrier fixed-row RLC",
            |mut region| {
                let mut pack_stores = std::collections::BTreeMap::<(usize, usize), Cell>::new();
                let mut pack_loads = std::collections::BTreeMap::<(usize, usize), Cell>::new();
                for (logical_row, row) in rows.iter().enumerate() {
                    let physical_start = logical_row
                        .checked_mul(CARRIER_RLC_ROWS_PER_LOGICAL_ROW)
                        .ok_or(PlonkError::Synthesis)?;
                    let head_fixed =
                        carrier_rlc_fixed_encoding_v1(row).map_err(|_| PlonkError::Synthesis)?;
                    let physical_values = carrier_rlc_physical_values_v1(row);
                    let mut bus = None;
                    for (half, values) in physical_values.iter().enumerate() {
                        let row_index = physical_start + half;
                        let fixed = if half == 0 { head_fixed } else { [F::ZERO; 3] };
                        for (position, column) in [config.mode_bit_0, config.mode_bit_1, config.payload]
                            .into_iter()
                            .enumerate()
                        {
                            let value = fixed[position];
                            #[cfg(test)]
                            let value = if matches!(row.physical_mutation, Some(CarrierRlcPhysicalMutationV1::Fixed { half: target_half, column: target_column }) if target_half == half && target_column == position) {
                                value + F::ONE
                            } else {
                                value
                            };
                            region.assign_fixed(column, row_index, value);
                        }
                        for (column_index, column) in config.advice.iter().copied().enumerate() {
                            let value = values[column_index];
                            #[cfg(test)]
                            let value = if matches!(row.physical_mutation, Some(CarrierRlcPhysicalMutationV1::Advice { half: target_half, column: target_column }) if target_half == half && target_column == column_index) {
                                value + F::ONE
                            } else {
                                value
                            };
                            let value = if self.use_unknown {
                                Value::unknown()
                            } else {
                                Value::known(value)
                            };
                            let assigned = region.assign_advice_discarding_value(column, row_index, value);
                            if half == 0 && column_index == 0 {
                                bus = Some(assigned);
                            }
                        }
                    }
                    let bus = bus.expect("claim RLC always assigns its bus column");
                    if let Some(binding) = row.binding {
                        match binding {
                            CarrierRlcBusBindingV1::Virtual(virtual_value) => {
                                if let Some(physical_cells) = &physical_cells {
                                    let virtual_cell =
                                        virtual_value.cell.ok_or(PlonkError::Synthesis)?;
                                    let physical = physical_cells
                                        .assigned_advices
                                        .resolve(&virtual_cell)
                                        .ok_or(PlonkError::Synthesis)?;
                                    region.constrain_equal(bus, physical);
                                }
                            }
                            CarrierRlcBusBindingV1::PackStore { carrier, pack } => {
                                if pack_stores.insert((carrier, pack), bus).is_some() {
                                    return Err(PlonkError::Synthesis);
                                }
                            }
                            CarrierRlcBusBindingV1::PackLoad { carrier, pack } => {
                                if pack_loads.insert((carrier, pack), bus).is_some() {
                                    return Err(PlonkError::Synthesis);
                                }
                            }
                        }
                    }
                }
                if pack_stores.len() != pack_loads.len() {
                    return Err(PlonkError::Synthesis);
                }
                for (key, stored) in pack_stores {
                    let loaded = pack_loads.remove(&key).ok_or(PlonkError::Synthesis)?;
                    region.constrain_equal(stored, loaded);
                }
                if !pack_loads.is_empty() {
                    return Err(PlonkError::Synthesis);
                }
                Ok(())
            },
        )
    }

    // Frozen vector oracle retained for existing mutation tests and streaming regressions.
    #[cfg(test)]
    pub(super) fn build_rows(&self) -> Result<Vec<CarrierRlcRawRowV1<F>>, String> {
        self.build_rows_with_capacity(CAPACITY)
    }

    // Frozen vector oracle retained for existing mutation tests and streaming regressions.
    #[cfg(test)]
    pub(super) fn build_rows_with_capacity(
        &self,
        fixed_capacity: usize,
    ) -> Result<Vec<CarrierRlcRawRowV1<F>>, String> {
        // Key generation must depend only on the fixed schedule. `without_witnesses` retains the
        // virtual cell identities needed by the equality bridge, but all arithmetic rows are
        // built from a canonical dummy witness and assigned as unknown values.
        let challenge_a = if self.use_unknown {
            1
        } else {
            assigned_u128_cell_v1(self.challenge_a, "claim RLC challenge A")?
        };
        let challenge_b = if self.use_unknown {
            1
        } else {
            assigned_u128_cell_v1(self.challenge_b, "claim RLC challenge B")?
        };
        if challenge_a == 0
            || challenge_b == 0
            || challenge_a > (1_u128 << CARRIER_RLC_CHALLENGE_BITS_V1)
            || challenge_b > (1_u128 << CARRIER_RLC_CHALLENGE_BITS_V1)
        {
            return Err("mint-hash claim RLC challenge is outside its canonical range".to_owned());
        }
        let physical_rows = self.required_rows_with_capacity(fixed_capacity)?;
        let logical_rows = physical_rows / CARRIER_RLC_ROWS_PER_LOGICAL_ROW;
        let mut rows = Vec::with_capacity(logical_rows);
        for (carrier_index, carrier) in self.carriers.iter().enumerate() {
            self.build_carrier_rows(
                &mut rows,
                carrier_index,
                carrier,
                challenge_a,
                challenge_b,
                fixed_capacity,
            )?;
        }
        if rows.len() != logical_rows {
            return Err("mint-hash claim RLC row schedule drifted".to_owned());
        }
        Ok(rows)
    }

    // Frozen vector oracle retained for existing mutation tests and streaming regressions.
    #[cfg(test)]
    pub(super) fn build_carrier_rows(
        &self,
        rows: &mut Vec<CarrierRlcRawRowV1<F>>,
        carrier_index: usize,
        carrier: &CarrierRlcCarrierV1<F>,
        challenge_a: u128,
        challenge_b: u128,
        fixed_capacity: usize,
    ) -> Result<(), String> {
        let mut state = CarrierRlcStateV1 {
            challenge_a,
            challenge_b,
            accumulator_a: 0,
            accumulator_b: 0,
            quotient_pack: 0,
            coefficient: 0,
        };
        let mut start_a = carrier_rlc_state_row_v1(
            state,
            CarrierRlcRowModeV1::StartA,
            Some(CarrierRlcBusBindingV1::Virtual(self.challenge_a)),
        );
        // The local challenge state and its equality bus must contain the same value. The bus
        // copies the original Base challenge cell; leaving it at the generic row's zero value
        // makes every nonzero challenge fail both the boundary gate and the permutation check.
        start_a.values[CARRIER_RLC_BUS] = F::from_u128(challenge_a);
        rows.push(start_a);
        let mut start_b = carrier_rlc_state_row_v1(
            state,
            CarrierRlcRowModeV1::StartB,
            Some(CarrierRlcBusBindingV1::Virtual(self.challenge_b)),
        );
        start_b.values[CARRIER_RLC_BUS] = F::from_u128(challenge_b);
        rows.push(start_b);

        let mut packs = Vec::with_capacity(
            carrier
                .values
                .len()
                .div_ceil(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1),
        );
        let mut ternary_power = 1_u128;
        for (value_index, assigned) in carrier.values.iter().copied().enumerate() {
            let value = if self.use_unknown {
                0
            } else {
                assigned_u128_cell_v1(assigned, "claim RLC carrier value")?
            };
            let quotient = value / CARRIER_RLC_MODULUS_V1;
            let remainder = value % CARRIER_RLC_MODULUS_V1;
            if quotient >= CARRIER_RLC_QUOTIENT_RADIX_V1 {
                return Err("mint-hash claim RLC quotient is not ternary".to_owned());
            }
            let mut preprocess = carrier_rlc_state_row_v1(
                state,
                CarrierRlcRowModeV1::Preprocess,
                Some(CarrierRlcBusBindingV1::Virtual(assigned)),
            );
            preprocess.values[CARRIER_RLC_BUS] = F::from_u128(value);
            preprocess.values[CARRIER_RLC_VALUE] = F::from_u128(value);
            preprocess.values[CARRIER_RLC_QUOTIENT_BIT_0] = F::from_u128(quotient & 1);
            preprocess.values[CARRIER_RLC_QUOTIENT_BIT_1] = F::from_u128(quotient >> 1);
            preprocess.values[CARRIER_RLC_RAW_REMAINDER] = F::from_u128(remainder);
            preprocess.values[CARRIER_RLC_REMAINDER_INVERSE] =
                carrier_rlc_non_modulus_inverse_v1::<F>(remainder)?;
            preprocess.ternary_power = F::from_u128(ternary_power);
            carrier_rlc_set_range_limbs_v1(&mut preprocess, value, remainder);
            rows.push(preprocess);
            state.quotient_pack = state
                .quotient_pack
                .checked_add(
                    quotient
                        .checked_mul(ternary_power)
                        .ok_or_else(|| "claim RLC quotient pack overflowed".to_owned())?,
                )
                .ok_or_else(|| "claim RLC quotient pack overflowed".to_owned())?;
            state.coefficient = remainder;
            carrier_rlc_push_evaluation_rows_v1(rows, &mut state, None)?;

            let pack_end = (value_index + 1) % CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1 == 0
                || value_index + 1 == carrier.values.len();
            if pack_end {
                let pack_index = packs.len();
                packs.push(state.quotient_pack);
                let row = rows
                    .last_mut()
                    .expect("an evaluation-B row precedes every pack boundary");
                row.store_pack = true;
                row.values[CARRIER_RLC_BUS] = F::from_u128(state.quotient_pack);
                row.binding = Some(CarrierRlcBusBindingV1::PackStore {
                    carrier: carrier_index,
                    pack: pack_index,
                });
                state.quotient_pack = 0;
                ternary_power = 1;
            } else {
                ternary_power = ternary_power
                    .checked_mul(CARRIER_RLC_QUOTIENT_RADIX_V1)
                    .ok_or_else(|| "claim RLC ternary power overflowed".to_owned())?;
            }
        }

        for (pack_index, pack) in packs.iter().copied().enumerate() {
            state.coefficient = pack;
            carrier_rlc_push_evaluation_rows_v1(
                rows,
                &mut state,
                Some((carrier_index, pack_index)),
            )?;
        }
        let fixed_pack_count = fixed_capacity.div_ceil(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
        if packs.len() != fixed_pack_count {
            return Err("claim RLC fixed quotient-pack schedule drifted".to_owned());
        }

        let expected_a = if self.use_unknown {
            state.accumulator_a
        } else {
            assigned_u128_cell_v1(carrier.expected_a, "claim RLC expected A")?
        };
        let expected_b = if self.use_unknown {
            state.accumulator_b
        } else {
            assigned_u128_cell_v1(carrier.expected_b, "claim RLC expected B")?
        };
        if expected_a >= CARRIER_RLC_MODULUS_V1 || expected_b >= CARRIER_RLC_MODULUS_V1 {
            return Err("mint-hash claim RLC expected result is not canonical".to_owned());
        }
        let mut end_a = carrier_rlc_state_row_v1(
            state,
            CarrierRlcRowModeV1::EndA,
            Some(CarrierRlcBusBindingV1::Virtual(carrier.expected_a)),
        );
        end_a.values[CARRIER_RLC_BUS] = F::from_u128(expected_a);
        rows.push(end_a);
        let mut end_b = carrier_rlc_state_row_v1(
            state,
            CarrierRlcRowModeV1::EndB,
            Some(CarrierRlcBusBindingV1::Virtual(carrier.expected_b)),
        );
        end_b.values[CARRIER_RLC_BUS] = F::from_u128(expected_b);
        rows.push(end_b);
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn carrier_rlc_state_row_v1<F: KagemushaPoseidonFieldV1>(
    state: CarrierRlcStateV1,
    mode: CarrierRlcRowModeV1,
    binding: Option<CarrierRlcBusBindingV1<F>>,
) -> CarrierRlcRawRowV1<F> {
    let mut values = [F::ZERO; CARRIER_RLC_COLUMNS];
    values[CARRIER_RLC_CHALLENGE_A] = F::from_u128(state.challenge_a);
    values[CARRIER_RLC_CHALLENGE_B] = F::from_u128(state.challenge_b);
    values[CARRIER_RLC_ACCUMULATOR_A] = F::from_u128(state.accumulator_a);
    values[CARRIER_RLC_ACCUMULATOR_B] = F::from_u128(state.accumulator_b);
    values[CARRIER_RLC_QUOTIENT_PACK] = F::from_u128(state.quotient_pack);
    values[CARRIER_RLC_COEFFICIENT] = F::from_u128(state.coefficient);
    CarrierRlcRawRowV1 {
        values,
        mode,
        store_pack: false,
        load_pack: false,
        ternary_power: F::ZERO,
        binding,
        #[cfg(test)]
        physical_mutation: None,
    }
}

// Frozen vector evaluation order; production uses the guarded streaming emitter.
#[cfg(test)]
pub(super) fn carrier_rlc_push_evaluation_rows_v1<F: KagemushaPoseidonFieldV1>(
    rows: &mut Vec<CarrierRlcRawRowV1<F>>,
    state: &mut CarrierRlcStateV1,
    pack_load: Option<(usize, usize)>,
) -> Result<(), String> {
    let (quotient_a, remainder_a) =
        carrier_rlc_native_step_v1(state.accumulator_a, state.challenge_a, state.coefficient)?;
    let binding =
        pack_load.map(|(carrier, pack)| CarrierRlcBusBindingV1::PackLoad { carrier, pack });
    let mut evaluate_a = carrier_rlc_state_row_v1(*state, CarrierRlcRowModeV1::EvaluateA, binding);
    evaluate_a.values[CARRIER_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_a);
    evaluate_a.values[CARRIER_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_a);
    evaluate_a.values[CARRIER_RLC_REMAINDER_INVERSE] =
        carrier_rlc_non_modulus_inverse_v1::<F>(remainder_a)?;
    if pack_load.is_some() {
        evaluate_a.load_pack = true;
        evaluate_a.values[CARRIER_RLC_BUS] = F::from_u128(state.coefficient);
    }
    carrier_rlc_set_range_limbs_v1(&mut evaluate_a, quotient_a, remainder_a);
    rows.push(evaluate_a);
    state.accumulator_a = remainder_a;

    let (quotient_b, remainder_b) =
        carrier_rlc_native_step_v1(state.accumulator_b, state.challenge_b, state.coefficient)?;
    let mut evaluate_b = carrier_rlc_state_row_v1(*state, CarrierRlcRowModeV1::EvaluateB, None);
    evaluate_b.values[CARRIER_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_b);
    evaluate_b.values[CARRIER_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_b);
    evaluate_b.values[CARRIER_RLC_REMAINDER_INVERSE] =
        carrier_rlc_non_modulus_inverse_v1::<F>(remainder_b)?;
    carrier_rlc_set_range_limbs_v1(&mut evaluate_b, quotient_b, remainder_b);
    rows.push(evaluate_b);
    state.accumulator_b = remainder_b;
    Ok(())
}

pub(super) fn carrier_rlc_native_step_v1(
    accumulator: u128,
    challenge: u128,
    coefficient: u128,
) -> Result<(u128, u128), String> {
    let modulus = U256::from_u128(CARRIER_RLC_MODULUS_V1);
    let divisor = Option::<NonZero<U256>>::from(NonZero::new(modulus))
        .expect("fixed claim RLC modulus is nonzero");
    let numerator = U256::from_u128(accumulator)
        .wrapping_mul(&U256::from_u128(challenge))
        .wrapping_add(&U256::from_u128(coefficient));
    let (quotient, remainder) = numerator.div_rem(&divisor);
    let to_u128 = |value: U256| {
        let bytes: [u8; 32] = value.to_le_bytes();
        if bytes[16..].iter().any(|byte| *byte != 0) {
            return Err("claim RLC division output exceeds u128".to_owned());
        }
        Ok(u128::from_le_bytes(
            bytes[..16]
                .try_into()
                .expect("U256 low half has sixteen bytes"),
        ))
    };
    let quotient = to_u128(quotient)?;
    let remainder = to_u128(remainder)?;
    if quotient >= (1_u128 << 126) || remainder >= CARRIER_RLC_MODULUS_V1 {
        return Err("claim RLC division output exceeds its proven bound".to_owned());
    }
    Ok((quotient, remainder))
}

pub(super) fn carrier_rlc_non_modulus_inverse_v1<F: KagemushaPoseidonFieldV1>(
    remainder: u128,
) -> Result<F, String> {
    Option::<F>::from((F::from_u128(remainder) - F::from_u128(CARRIER_RLC_MODULUS_V1)).invert())
        .ok_or_else(|| "claim RLC remainder is not canonical".to_owned())
}

#[cfg(test)]
pub(super) fn carrier_rlc_set_range_limbs_v1<F: KagemushaPoseidonFieldV1>(
    row: &mut CarrierRlcRawRowV1<F>,
    first: u128,
    second: u128,
) {
    let first_top_bits = match row.mode {
        CarrierRlcRowModeV1::Preprocess => 8,
        CarrierRlcRowModeV1::EvaluateA | CarrierRlcRowModeV1::EvaluateB => 6,
        _ => unreachable!("range limbs only occur on claim RLC arithmetic rows"),
    };
    row.values[CARRIER_RLC_SCALED_FIRST_TOP] =
        F::from_u128((first >> 120) << (CARRIER_RLC_RADIX_BITS - first_top_bits));
    row.values[CARRIER_RLC_SCALED_SECOND_TOP] =
        F::from_u128((second >> 120) << (CARRIER_RLC_RADIX_BITS - 7));
    for (half, mut value) in [first, second].into_iter().enumerate() {
        for limb in 0..9 {
            row.values[CARRIER_RLC_RANGE_START + half * 9 + limb] =
                F::from_u128(value & (CARRIER_RLC_RADIX - 1));
            value >>= CARRIER_RLC_RADIX_BITS;
        }
        debug_assert_eq!(value, 0);
    }
}

pub(super) fn assigned_u128_cell_v1<F: halo2_base::utils::ScalarField>(
    cell: AssignedValue<F>,
    label: &str,
) -> Result<u128, String> {
    use halo2_base::utils::fe_to_biguint;

    let integer = fe_to_biguint(cell.value());
    if integer.bits() > 128 {
        return Err(format!("{label} value exceeds u128"));
    }
    let digits = integer.to_u64_digits();
    Ok(u128::from(digits.first().copied().unwrap_or(0))
        | (u128::from(digits.get(1).copied().unwrap_or(0)) << 64))
}

pub(super) mod streaming;

#[cfg(test)]
mod capacity_tests {
    use super::*;
    use halo2_proofs::{
        halo2curves::pasta::{Fp, Fq},
        plonk::Assigned,
    };

    // This is a scalar/cell schedule oracle; it creates no monetary proof or accepting verifier.
    fn one_carrier<F: KagemushaPoseidonFieldV1, const CAPACITY: usize, const PACKS: usize>()
    -> KagemushaCarrierRlcMachineV1<F, CAPACITY, 1, PACKS> {
        let assign = |value| AssignedValue {
            value: Assigned::Trivial(F::from_u128(value)),
            cell: None,
        };
        let values = (0..CAPACITY)
            .map(|index| {
                [
                    0,
                    1,
                    CARRIER_RLC_MODULUS_V1 - 1,
                    CARRIER_RLC_MODULUS_V1,
                    2 * CARRIER_RLC_MODULUS_V1,
                    u128::MAX,
                ][index % 6]
            })
            .collect::<Vec<_>>();
        let endpoint = |challenge| {
            let mut coefficients = values
                .iter()
                .map(|value| value % CARRIER_RLC_MODULUS_V1)
                .collect::<Vec<_>>();
            for chunk in values.chunks(CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
                let mut packed = 0_u128;
                let mut power = 1_u128;
                for (index, value) in chunk.iter().enumerate() {
                    packed += (value / CARRIER_RLC_MODULUS_V1) * power;
                    if index + 1 != chunk.len() {
                        power *= 3;
                    }
                }
                coefficients.push(packed);
            }
            coefficients
                .into_iter()
                .fold(0, |accumulator, coefficient| {
                    carrier_rlc_native_step_v1(accumulator, challenge, coefficient)
                        .unwrap()
                        .1
                })
        };
        KagemushaCarrierRlcMachineV1 {
            challenge_a: assign(2),
            challenge_b: assign(3),
            carriers: [CarrierRlcCarrierV1 {
                expected_a: assign(endpoint(2)),
                expected_b: assign(endpoint(3)),
                values: values.into_iter().map(assign).collect(),
            }],
            use_unknown: false,
        }
    }

    fn check_complete_credential_lane<F: KagemushaPoseidonFieldV1>() {
        let machine = one_carrier::<F, 8162, 103>();
        assert_eq!(machine.required_rows().unwrap(), 49_392);
        machine.validate_capacity(65_527).unwrap();
        let expected = machine.build_rows().unwrap();
        let mut stores = 0;
        let mut loads = 0;
        streaming::reset_cleanup_counts();
        let emitted = streaming::emit_rows_with_capacity(&machine, 8162, |index, row| {
            assert_eq!(row.values, expected[index].values);
            assert_eq!(
                row.fixed_encoding().unwrap(),
                carrier_rlc_fixed_encoding_v1(&expected[index]).unwrap()
            );
            assert_eq!(
                row.physical_values(),
                carrier_rlc_physical_values_v1(&expected[index])
            );
            stores += usize::from(row.store_pack);
            loads += usize::from(row.load_pack);
            Ok(())
        })
        .unwrap();
        assert_eq!(emitted, 24_696);
        assert_eq!((stores, loads), (103, 103));
        let counts = streaming::cleanup_counts();
        assert_eq!(counts[0], emitted);
        assert_eq!(counts[1], 1);
        assert_eq!(counts[2], 1);
        assert_eq!(counts[4], 0);
        assert_eq!(counts[5], 1);
    }

    #[test]
    fn complete_credential_lane_retains_every_value_and_all_103_packs_in_both_fields() {
        check_complete_credential_lane::<Fp>();
        check_complete_credential_lane::<Fq>();
    }

    #[test]
    fn complete_credential_lane_refuses_short_pack_owner_and_short_carrier() {
        let short_owner = one_carrier::<Fp, 8162, 52>();
        assert!(short_owner.required_rows().is_err());
        let mut short_carrier = one_carrier::<Fp, 8162, 103>();
        short_carrier.carriers[0].values.pop();
        assert!(short_carrier.required_rows().is_err());
        assert!(streaming::emit_rows_with_capacity(&short_carrier, 8162, |_, _| Ok(())).is_err());
        let full = one_carrier::<Fp, 8162, 103>();
        assert!(full.validate_capacity(49_391).is_err());
        assert!(streaming::emit_rows_with_capacity(&full, 8163, |_, _| Ok(())).is_err());
    }
}
