//! Ordered recursive claim fold for KAGEMUSHA mint-hash shards.
//!
//! A one-block Table8 shard proves only one SHA-256 compression. It is never monetary authority
//! on its own. This module gives those leaves an order- and completeness-preserving state machine
//! and a sound bridge into the sole `k = 16` monetary history. The bridge prepends four constant
//! zero IPA challenges to a `k = 12` leaf accumulator. With the release-authenticated generator
//! prefix check, its 4,096 coefficients are therefore the first 4,096 coefficients of the
//! 65,536-generator monetary basis and every remaining coefficient is zero.
//!
//! Each recursive step folds both the predecessor claim proof and one lifted leaf proof into one
//! fixed 544-byte accumulator. Public claim state is constant-size regardless of leaf count. A
//! terminal claim is valid only after the exact typed-plan stage and job totals have been reached
//! and the ordered terminal-digest root equals the plan commitment.

use ff::{Field as _, PrimeField};
#[cfg(test)]
use halo2_base::QuantumCell::Witness;
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    arithmetic::best_multiexp,
    circuit::{Cell, Layouter, V1, Value},
    halo2curves::{
        CurveAffine,
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Expression, Fixed,
        TableColumn,
    },
    poly::{
        Rotation,
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use p256::elliptic_curve::bigint::{Encoding as _, NonZero, U256};
use snark_verifier::{
    loader::{ScalarLoader as _, native::NativeLoader},
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredAccumulator, DeferredLoader, DeferredScalar, KagemushaNativeDeferredBatchV1,
        bind_accumulator_limbs, constrain_reciprocal_native_batch_v1, deferred_field_chips_v1,
        deferred_loader_v1, derive_mint_hash_claim_native_deferred_batch_v1,
        kagemusha_protocol_structure_digest_v1, load_and_constrain_parent_protocol_v1,
        load_native_accumulator, select_accumulator_v1, verify_fold_with_transcript_binding_v1,
        verify_ordinary_proof_with_transcript_binding_at_k_v1,
        verify_two_carrier_hybrid_ordinary_proof_and_stream_v1,
    },
    mint_hash_shard::{
        KAGEMUSHA_MINT_HASH_SHARD_K_V1, KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1,
        KagemushaMintHashShardStatementV1, public_instance as shard_public,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{
        KagemushaPoseidonChipV1, KagemushaPoseidonFieldV1, decode, digest_limbs, encode, from_u128,
        hash,
    },
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256_table8::{BLOCK_SIZE, DIGEST_SIZE, IV},
};

const PLAN_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhpln1");
const MESSAGE_SEED_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhmsg0");
const MESSAGE_STEP_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhmsg1");
const TERMINAL_SEED_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhseed");
const TERMINAL_STEP_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhjob1");
// Proof-chain revision two commits the final authenticated transcript squeezes instead of
// re-encoding every proof object as constrained bytes.
const PROOF_CHAIN_SEED_DOMAIN_V2: u64 = u64::from_le_bytes(*b"kgmhpc20");
const PROOF_CHAIN_STEP_DOMAIN_V2: u64 = u64::from_le_bytes(*b"kgmhpc21");
const CLAIM_PARENT_EQUATION_TAG_V1: u32 = 11;
const CLAIM_SHARD_EQUATION_TAG_V1: u32 = 12;
const CLAIM_BATCH_TRANSCRIPT_BINDING_COUNT_V1: usize = 4;
// Version 2 uses canonical Mersenne residues plus packed ternary quotients.
const CLAIM_CARRIER_RLC_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmrlc_2");
const CLAIM_CARRIER_RLC_VERSION_V1: u64 = 2;
const CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1: usize = 125;
const CLAIM_CARRIER_RLC_MODULUS_V1: u128 = (1_u128 << 127) - 1;
/// Quotients of a `u128` by the 127-bit RLC modulus are ternary digits.
/// Packing eighty of them is canonical because `3^80 - 1 < 2^127 - 1`.
const CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1: u128 = 3;
const CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1: usize = 80;
const MINIMUM_UNUSABLE_ROWS: usize = 9;
const KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1: usize = 2;
const SHARD_TO_HISTORY_ZERO_ROUNDS_V1: usize =
    (KAGEMUSHA_RECURSION_IPA_K_V1 - KAGEMUSHA_MINT_HASH_SHARD_K_V1) as usize;

// The common-prime carrier binding used to run through BaseCircuitBuilder. At the real claim
// width, its generic vertical gates occupied another ~4 million advice cells and materialized one
// selector plus one permutation column for almost every 65,527-row slice. This fixed machine keeps
// the exact same two-challenge V2 polynomial, but schedules all divisions in one narrow region.
// Only BUS participates in the permutation argument; the remaining columns are local state.
const CLAIM_RLC_BUS: usize = 0;
const CLAIM_RLC_VALUE: usize = 1;
const CLAIM_RLC_COEFFICIENT: usize = 2;
const CLAIM_RLC_QUOTIENT_BIT_0: usize = 3;
const CLAIM_RLC_QUOTIENT_BIT_1: usize = 4;
const CLAIM_RLC_RAW_REMAINDER: usize = 5;
const CLAIM_RLC_REMAINDER_INVERSE: usize = 6;
const CLAIM_RLC_CHALLENGE_A: usize = 7;
const CLAIM_RLC_CHALLENGE_B: usize = 8;
const CLAIM_RLC_ACCUMULATOR_A: usize = 9;
const CLAIM_RLC_ACCUMULATOR_B: usize = 10;
const CLAIM_RLC_QUOTIENT_PACK: usize = 11;
const CLAIM_RLC_DIVISION_QUOTIENT: usize = 12;
const CLAIM_RLC_DIVISION_REMAINDER: usize = 13;
const CLAIM_RLC_RANGE_START: usize = 14;
const CLAIM_RLC_RANGE_LIMBS: usize = 18;
const CLAIM_RLC_SCALED_FIRST_TOP: usize = CLAIM_RLC_RANGE_START + CLAIM_RLC_RANGE_LIMBS;
const CLAIM_RLC_SCALED_SECOND_TOP: usize = CLAIM_RLC_SCALED_FIRST_TOP + 1;
const CLAIM_RLC_COLUMNS: usize = CLAIM_RLC_SCALED_SECOND_TOP + 1;
const CLAIM_RLC_RADIX_BITS: usize = 15;
const CLAIM_RLC_RADIX: u128 = 1_u128 << CLAIM_RLC_RADIX_BITS;

#[derive(Clone, Debug)]
struct KagemushaClaimCarrierRlcConfigV1 {
    advice: [Column<Advice>; CLAIM_RLC_COLUMNS],
    range_table: TableColumn,
    mode_bit_0: Column<Fixed>,
    mode_bit_1: Column<Fixed>,
    payload: Column<Fixed>,
}

impl KagemushaClaimCarrierRlcConfigV1 {
    fn configure<F: KagemushaPoseidonFieldV1>(meta: &mut ConstraintSystem<F>) -> Self {
        let advice = std::array::from_fn(|_| meta.advice_column());
        meta.enable_equality(advice[CLAIM_RLC_BUS]);
        // This machine owns its lookup table. BaseCircuitBuilder deliberately removes its range
        // table when an exact witness happens to require no Base-managed lookup advice, so
        // borrowing that optional table made the custom circuit shape depend on witness traffic.
        let range_table = meta.lookup_table_column();
        // Two fixed mode bits select inactive/boundary/preprocess/evaluate rows. The third fixed
        // column is a mode-local payload: a boundary subtype, the preprocess ternary power, or an
        // evaluate-side/load/store opcode. Unassigned rows decode as inactive (0, 0, 0).
        let mode_bit_0 = meta.fixed_column();
        let mode_bit_1 = meta.fixed_column();
        let payload = meta.fixed_column();

        meta.create_gate("Kagemusha claim carrier RLC state machine", |meta| {
            let current: [Expression<F>; CLAIM_RLC_COLUMNS] =
                std::array::from_fn(|index| meta.query_advice(advice[index], Rotation::cur()));
            let next: [Expression<F>; CLAIM_RLC_COLUMNS] =
                std::array::from_fn(|index| meta.query_advice(advice[index], Rotation::next()));
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
            let modulus = Expression::Constant(F::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1));
            let transition = start_a.clone()
                + start_b.clone()
                + preprocess.clone()
                + evaluate_a.clone()
                + evaluate_b.clone()
                + end_a.clone();
            let idle = start_a.clone() + start_b.clone() + end_a.clone();
            let quotient_bit_0 = current[CLAIM_RLC_QUOTIENT_BIT_0].clone();
            let quotient_bit_1 = current[CLAIM_RLC_QUOTIENT_BIT_1].clone();
            let quotient = quotient_bit_0.clone() + two * quotient_bit_1.clone();

            let compose = |limbs: std::ops::Range<usize>| {
                limbs
                    .enumerate()
                    .fold(Expression::Constant(F::ZERO), |sum, (position, index)| {
                        sum + current[CLAIM_RLC_RANGE_START + index].clone()
                            * Expression::Constant(F::from_u128(
                                1_u128 << (CLAIM_RLC_RADIX_BITS * position),
                            ))
                    })
            };
            let first_range = compose(0..9);
            let second_range = compose(9..18);
            let first_top = current[CLAIM_RLC_RANGE_START + 8].clone();
            let second_top = current[CLAIM_RLC_RANGE_START + 17].clone();
            vec![
                start_a.clone()
                    * (current[CLAIM_RLC_CHALLENGE_A].clone() - current[CLAIM_RLC_BUS].clone()),
                start_b.clone()
                    * (current[CLAIM_RLC_CHALLENGE_B].clone() - current[CLAIM_RLC_BUS].clone()),
                start_a.clone() * current[CLAIM_RLC_ACCUMULATOR_A].clone(),
                start_a.clone() * current[CLAIM_RLC_ACCUMULATOR_B].clone(),
                start_a.clone() * current[CLAIM_RLC_QUOTIENT_PACK].clone(),
                end_a.clone()
                    * (current[CLAIM_RLC_ACCUMULATOR_A].clone() - current[CLAIM_RLC_BUS].clone()),
                end_b.clone()
                    * (current[CLAIM_RLC_ACCUMULATOR_B].clone() - current[CLAIM_RLC_BUS].clone()),
                transition.clone()
                    * (next[CLAIM_RLC_CHALLENGE_A].clone()
                        - current[CLAIM_RLC_CHALLENGE_A].clone()),
                transition
                    * (next[CLAIM_RLC_CHALLENGE_B].clone()
                        - current[CLAIM_RLC_CHALLENGE_B].clone()),
                idle.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_A].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_A].clone()),
                idle.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_B].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_B].clone()),
                idle * (next[CLAIM_RLC_QUOTIENT_PACK].clone()
                    - current[CLAIM_RLC_QUOTIENT_PACK].clone()),
                preprocess.clone()
                    * (current[CLAIM_RLC_BUS].clone() - current[CLAIM_RLC_VALUE].clone()),
                preprocess.clone()
                    * (current[CLAIM_RLC_VALUE].clone()
                        - quotient.clone() * modulus.clone()
                        - current[CLAIM_RLC_RAW_REMAINDER].clone()),
                preprocess.clone()
                    * quotient_bit_0.clone()
                    * (quotient_bit_0.clone() - one.clone()),
                preprocess.clone()
                    * quotient_bit_1.clone()
                    * (quotient_bit_1.clone() - one.clone()),
                preprocess.clone() * quotient_bit_0 * quotient_bit_1,
                preprocess.clone()
                    * ((current[CLAIM_RLC_RAW_REMAINDER].clone() - modulus.clone())
                        * current[CLAIM_RLC_REMAINDER_INVERSE].clone()
                        - one.clone()),
                preprocess.clone() * (current[CLAIM_RLC_VALUE].clone() - first_range.clone()),
                preprocess.clone()
                    * (current[CLAIM_RLC_RAW_REMAINDER].clone() - second_range.clone()),
                preprocess.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_A].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_A].clone()),
                preprocess.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_B].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_B].clone()),
                preprocess.clone()
                    * (next[CLAIM_RLC_COEFFICIENT].clone()
                        - current[CLAIM_RLC_RAW_REMAINDER].clone()),
                preprocess.clone()
                    * (next[CLAIM_RLC_QUOTIENT_PACK].clone()
                        - current[CLAIM_RLC_QUOTIENT_PACK].clone()
                        - quotient * power),
                evaluate.clone() * (current[CLAIM_RLC_DIVISION_QUOTIENT].clone() - first_range),
                evaluate.clone() * (current[CLAIM_RLC_DIVISION_REMAINDER].clone() - second_range),
                evaluate.clone()
                    * ((current[CLAIM_RLC_DIVISION_REMAINDER].clone() - modulus.clone())
                        * current[CLAIM_RLC_REMAINDER_INVERSE].clone()
                        - one),
                preprocess.clone()
                    * (current[CLAIM_RLC_SCALED_FIRST_TOP].clone()
                        - first_top.clone() * Expression::Constant(F::from(128))),
                evaluate.clone()
                    * (current[CLAIM_RLC_SCALED_FIRST_TOP].clone()
                        - first_top * Expression::Constant(F::from(512))),
                (preprocess + evaluate.clone())
                    * (current[CLAIM_RLC_SCALED_SECOND_TOP].clone()
                        - second_top * Expression::Constant(F::from(256))),
                evaluate_a.clone()
                    * (current[CLAIM_RLC_ACCUMULATOR_A].clone()
                        * current[CLAIM_RLC_CHALLENGE_A].clone()
                        + current[CLAIM_RLC_COEFFICIENT].clone()
                        - current[CLAIM_RLC_DIVISION_QUOTIENT].clone() * modulus.clone()
                        - current[CLAIM_RLC_DIVISION_REMAINDER].clone()),
                evaluate_b.clone()
                    * (current[CLAIM_RLC_ACCUMULATOR_B].clone()
                        * current[CLAIM_RLC_CHALLENGE_B].clone()
                        + current[CLAIM_RLC_COEFFICIENT].clone()
                        - current[CLAIM_RLC_DIVISION_QUOTIENT].clone() * modulus
                        - current[CLAIM_RLC_DIVISION_REMAINDER].clone()),
                evaluate_a.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_A].clone()
                        - current[CLAIM_RLC_DIVISION_REMAINDER].clone()),
                evaluate_a.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_B].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_B].clone()),
                evaluate_a.clone()
                    * (next[CLAIM_RLC_COEFFICIENT].clone()
                        - current[CLAIM_RLC_COEFFICIENT].clone()),
                evaluate_a
                    * (next[CLAIM_RLC_QUOTIENT_PACK].clone()
                        - current[CLAIM_RLC_QUOTIENT_PACK].clone()),
                evaluate_b.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_A].clone()
                        - current[CLAIM_RLC_ACCUMULATOR_A].clone()),
                evaluate_b.clone()
                    * (next[CLAIM_RLC_ACCUMULATOR_B].clone()
                        - current[CLAIM_RLC_DIVISION_REMAINDER].clone()),
                evaluate_b.clone()
                    * (next[CLAIM_RLC_QUOTIENT_PACK].clone()
                        - current[CLAIM_RLC_QUOTIENT_PACK].clone())
                    + store_pack.clone() * current[CLAIM_RLC_QUOTIENT_PACK].clone(),
                store_pack
                    * (current[CLAIM_RLC_BUS].clone() - current[CLAIM_RLC_QUOTIENT_PACK].clone()),
                load_pack
                    * (current[CLAIM_RLC_BUS].clone() - current[CLAIM_RLC_COEFFICIENT].clone()),
            ]
        });

        // Halo2 tuple lookups do not independently range-check tuple coordinates. Register one
        // unconditional lookup argument per limb, exactly as `RangeConfig` does for independent
        // lookup advice. Keeping the input linear preserves the existing degree-four lookup floor.
        for position in 0..CLAIM_RLC_RANGE_LIMBS {
            meta.lookup("Kagemusha claim carrier RLC range limb", |meta| {
                let cell =
                    meta.query_advice(advice[CLAIM_RLC_RANGE_START + position], Rotation::cur());
                vec![(cell, range_table)]
            });
        }
        // Mirror `RangeChip::_range_check` for each partial high limb. Dedicated scaled advice
        // keeps these two lookup inputs linear; the gate binds them to the mode-specific scale.
        for position in [CLAIM_RLC_SCALED_FIRST_TOP, CLAIM_RLC_SCALED_SECOND_TOP] {
            meta.lookup("Kagemusha claim carrier RLC partial range limb", |meta| {
                let cell = meta.query_advice(advice[position], Rotation::cur());
                vec![(cell, range_table)]
            });
        }

        Self {
            advice,
            range_table,
            mode_bit_0,
            mode_bit_1,
            payload,
        }
    }

    fn load_range_table<F: KagemushaPoseidonFieldV1>(
        &self,
        layouter: &mut impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        layouter.assign_table(
            || "Kagemusha claim carrier RLC range",
            |mut table| {
                for value in 0..CLAIM_RLC_RADIX as usize {
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
enum ClaimRlcRowModeV1 {
    StartA,
    StartB,
    Preprocess,
    EvaluateA,
    EvaluateB,
    EndA,
    EndB,
}

#[derive(Clone, Copy, Debug)]
enum ClaimRlcBusBindingV1<F: PrimeField> {
    Virtual(AssignedValue<F>),
    PackStore { carrier: usize, pack: usize },
    PackLoad { carrier: usize, pack: usize },
}

#[derive(Clone, Debug)]
struct ClaimRlcRawRowV1<F: PrimeField> {
    values: [F; CLAIM_RLC_COLUMNS],
    mode: ClaimRlcRowModeV1,
    store_pack: bool,
    load_pack: bool,
    ternary_power: F,
    binding: Option<ClaimRlcBusBindingV1<F>>,
}

fn claim_rlc_fixed_encoding_v1<F: PrimeField>(row: &ClaimRlcRawRowV1<F>) -> Result<[F; 3], String> {
    if row.store_pack && !matches!(row.mode, ClaimRlcRowModeV1::EvaluateB) {
        return Err("claim RLC store opcode is not on an evaluation-B row".to_owned());
    }
    if row.load_pack && !matches!(row.mode, ClaimRlcRowModeV1::EvaluateA) {
        return Err("claim RLC load opcode is not on an evaluation-A row".to_owned());
    }
    if !matches!(row.mode, ClaimRlcRowModeV1::Preprocess) && row.ternary_power != F::ZERO {
        return Err("claim RLC ternary power is not on a preprocess row".to_owned());
    }

    let encoding = match row.mode {
        ClaimRlcRowModeV1::StartA => [F::ZERO, F::ONE, F::ZERO],
        ClaimRlcRowModeV1::StartB => [F::ZERO, F::ONE, F::ONE],
        ClaimRlcRowModeV1::Preprocess => {
            if row.ternary_power == F::ZERO {
                return Err("claim RLC preprocess ternary power is zero".to_owned());
            }
            [F::ONE, F::ZERO, row.ternary_power]
        }
        ClaimRlcRowModeV1::EvaluateA => {
            [F::ONE, F::ONE, if row.load_pack { F::ONE } else { F::ZERO }]
        }
        ClaimRlcRowModeV1::EvaluateB => [
            F::ONE,
            F::ONE,
            if row.store_pack {
                F::ZERO - F::ONE
            } else {
                F::from(2)
            },
        ],
        ClaimRlcRowModeV1::EndA => [F::ZERO, F::ONE, F::from(2)],
        ClaimRlcRowModeV1::EndB => [F::ZERO, F::ONE, F::from(3)],
    };
    Ok(encoding)
}

#[derive(Clone, Copy)]
struct ClaimRlcStateV1 {
    challenge_a: u128,
    challenge_b: u128,
    accumulator_a: u128,
    accumulator_b: u128,
    quotient_pack: u128,
    coefficient: u128,
}

#[derive(Clone, Debug)]
struct ClaimRlcCarrierV1<F: PrimeField> {
    values: Vec<AssignedValue<F>>,
    expected_a: AssignedValue<F>,
    expected_b: AssignedValue<F>,
}

#[derive(Clone, Debug)]
struct KagemushaClaimCarrierRlcMachineV1<F: KagemushaPoseidonFieldV1> {
    challenge_a: AssignedValue<F>,
    challenge_b: AssignedValue<F>,
    carriers: [ClaimRlcCarrierV1<F>; 2],
    use_unknown: bool,
}

impl<F: KagemushaPoseidonFieldV1> KagemushaClaimCarrierRlcMachineV1<F> {
    fn unknown(&self) -> Self {
        let mut unknown = self.clone();
        unknown.use_unknown = true;
        unknown
    }

    fn required_rows(&self) -> Result<usize, String> {
        self.required_rows_with_capacity(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1)
    }

    fn required_rows_with_capacity(&self, fixed_capacity: usize) -> Result<usize, String> {
        if fixed_capacity == 0 {
            return Err("mint-hash claim RLC fixed capacity is zero".to_owned());
        }
        let fixed_packs = fixed_capacity.div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
        self.carriers.iter().try_fold(0_usize, |total, carrier| {
            if carrier.values.len() != fixed_capacity {
                return Err(
                    "mint-hash claim RLC carrier does not fill its fixed schedule".to_owned(),
                );
            }
            total
                .checked_add(4 + 3 * fixed_capacity + 2 * fixed_packs)
                .ok_or_else(|| "mint-hash claim RLC row count overflowed".to_owned())
        })
    }

    fn validate_capacity(&self, usable_rows: usize) -> Result<(), String> {
        let required = self.required_rows()?;
        if required > usable_rows {
            return Err(format!(
                "mint-hash claim RLC requires {required} rows, exceeding {usable_rows}"
            ));
        }
        Ok(())
    }

    fn synthesize(
        &self,
        config: &KagemushaClaimCarrierRlcConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        usable_rows: usize,
    ) -> Result<(), PlonkError> {
        self.validate_capacity(usable_rows)
            .map_err(|_| PlonkError::Synthesis)?;
        config.load_range_table(layouter)?;
        let rows = self.build_rows().map_err(|_| PlonkError::Synthesis)?;
        self.synthesize_rows(config, layouter, copy_manager, witness_gen_only, &rows)
    }

    fn synthesize_rows(
        &self,
        config: &KagemushaClaimCarrierRlcConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
        rows: &[ClaimRlcRawRowV1<F>],
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
                for (row_index, row) in rows.iter().enumerate() {
                    let [mode_bit_0, mode_bit_1, payload] =
                        claim_rlc_fixed_encoding_v1(row).map_err(|_| PlonkError::Synthesis)?;
                    region.assign_fixed(config.mode_bit_0, row_index, mode_bit_0);
                    region.assign_fixed(config.mode_bit_1, row_index, mode_bit_1);
                    region.assign_fixed(config.payload, row_index, payload);

                    let mut bus = None;
                    for (column_index, column) in config.advice.iter().copied().enumerate() {
                        let value = if self.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(row.values[column_index])
                        };
                        let assigned = region.assign_advice(column, row_index, value).cell();
                        if column_index == CLAIM_RLC_BUS {
                            bus = Some(assigned);
                        }
                    }
                    let bus = bus.expect("claim RLC always assigns its bus column");
                    if let Some(binding) = row.binding {
                        match binding {
                            ClaimRlcBusBindingV1::Virtual(virtual_value) => {
                                if let Some(physical_cells) = &physical_cells {
                                    let virtual_cell =
                                        virtual_value.cell.ok_or(PlonkError::Synthesis)?;
                                    let physical = *physical_cells
                                        .assigned_advices
                                        .get(&virtual_cell)
                                        .ok_or(PlonkError::Synthesis)?;
                                    region.constrain_equal(bus, physical);
                                }
                            }
                            ClaimRlcBusBindingV1::PackStore { carrier, pack } => {
                                if pack_stores.insert((carrier, pack), bus).is_some() {
                                    return Err(PlonkError::Synthesis);
                                }
                            }
                            ClaimRlcBusBindingV1::PackLoad { carrier, pack } => {
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

    fn build_rows(&self) -> Result<Vec<ClaimRlcRawRowV1<F>>, String> {
        self.build_rows_with_capacity(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1)
    }

    fn build_rows_with_capacity(
        &self,
        fixed_capacity: usize,
    ) -> Result<Vec<ClaimRlcRawRowV1<F>>, String> {
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
            || challenge_a > (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1)
            || challenge_b > (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1)
        {
            return Err("mint-hash claim RLC challenge is outside its canonical range".to_owned());
        }
        let mut rows = Vec::with_capacity(self.required_rows_with_capacity(fixed_capacity)?);
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
        if rows.len() != self.required_rows_with_capacity(fixed_capacity)? {
            return Err("mint-hash claim RLC row schedule drifted".to_owned());
        }
        Ok(rows)
    }

    fn build_carrier_rows(
        &self,
        rows: &mut Vec<ClaimRlcRawRowV1<F>>,
        carrier_index: usize,
        carrier: &ClaimRlcCarrierV1<F>,
        challenge_a: u128,
        challenge_b: u128,
        fixed_capacity: usize,
    ) -> Result<(), String> {
        let mut state = ClaimRlcStateV1 {
            challenge_a,
            challenge_b,
            accumulator_a: 0,
            accumulator_b: 0,
            quotient_pack: 0,
            coefficient: 0,
        };
        rows.push(claim_rlc_state_row_v1(
            state,
            ClaimRlcRowModeV1::StartA,
            Some(ClaimRlcBusBindingV1::Virtual(self.challenge_a)),
        ));
        rows.push(claim_rlc_state_row_v1(
            state,
            ClaimRlcRowModeV1::StartB,
            Some(ClaimRlcBusBindingV1::Virtual(self.challenge_b)),
        ));

        let mut packs = Vec::with_capacity(
            carrier
                .values
                .len()
                .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1),
        );
        let mut ternary_power = 1_u128;
        for (value_index, assigned) in carrier.values.iter().copied().enumerate() {
            let value = if self.use_unknown {
                0
            } else {
                assigned_u128_cell_v1(assigned, "claim RLC carrier value")?
            };
            let quotient = value / CLAIM_CARRIER_RLC_MODULUS_V1;
            let remainder = value % CLAIM_CARRIER_RLC_MODULUS_V1;
            if quotient >= CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1 {
                return Err("mint-hash claim RLC quotient is not ternary".to_owned());
            }
            let mut preprocess = claim_rlc_state_row_v1(
                state,
                ClaimRlcRowModeV1::Preprocess,
                Some(ClaimRlcBusBindingV1::Virtual(assigned)),
            );
            preprocess.values[CLAIM_RLC_BUS] = F::from_u128(value);
            preprocess.values[CLAIM_RLC_VALUE] = F::from_u128(value);
            preprocess.values[CLAIM_RLC_QUOTIENT_BIT_0] = F::from_u128(quotient & 1);
            preprocess.values[CLAIM_RLC_QUOTIENT_BIT_1] = F::from_u128(quotient >> 1);
            preprocess.values[CLAIM_RLC_RAW_REMAINDER] = F::from_u128(remainder);
            preprocess.values[CLAIM_RLC_REMAINDER_INVERSE] =
                claim_rlc_non_modulus_inverse_v1::<F>(remainder)?;
            preprocess.ternary_power = F::from_u128(ternary_power);
            claim_rlc_set_range_limbs_v1(&mut preprocess, value, remainder);
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
            claim_rlc_push_evaluation_rows_v1(rows, &mut state, None)?;

            let pack_end = (value_index + 1) % CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1 == 0
                || value_index + 1 == carrier.values.len();
            if pack_end {
                let pack_index = packs.len();
                packs.push(state.quotient_pack);
                let row = rows
                    .last_mut()
                    .expect("an evaluation-B row precedes every pack boundary");
                row.store_pack = true;
                row.values[CLAIM_RLC_BUS] = F::from_u128(state.quotient_pack);
                row.binding = Some(ClaimRlcBusBindingV1::PackStore {
                    carrier: carrier_index,
                    pack: pack_index,
                });
                state.quotient_pack = 0;
                ternary_power = 1;
            } else {
                ternary_power = ternary_power
                    .checked_mul(CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1)
                    .ok_or_else(|| "claim RLC ternary power overflowed".to_owned())?;
            }
        }

        for (pack_index, pack) in packs.iter().copied().enumerate() {
            state.coefficient = pack;
            claim_rlc_push_evaluation_rows_v1(rows, &mut state, Some((carrier_index, pack_index)))?;
        }
        let fixed_pack_count =
            fixed_capacity.div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
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
        if expected_a >= CLAIM_CARRIER_RLC_MODULUS_V1 || expected_b >= CLAIM_CARRIER_RLC_MODULUS_V1
        {
            return Err("mint-hash claim RLC expected result is not canonical".to_owned());
        }
        let mut end_a = claim_rlc_state_row_v1(
            state,
            ClaimRlcRowModeV1::EndA,
            Some(ClaimRlcBusBindingV1::Virtual(carrier.expected_a)),
        );
        end_a.values[CLAIM_RLC_BUS] = F::from_u128(expected_a);
        rows.push(end_a);
        let mut end_b = claim_rlc_state_row_v1(
            state,
            ClaimRlcRowModeV1::EndB,
            Some(ClaimRlcBusBindingV1::Virtual(carrier.expected_b)),
        );
        end_b.values[CLAIM_RLC_BUS] = F::from_u128(expected_b);
        rows.push(end_b);
        Ok(())
    }
}

fn claim_rlc_state_row_v1<F: KagemushaPoseidonFieldV1>(
    state: ClaimRlcStateV1,
    mode: ClaimRlcRowModeV1,
    binding: Option<ClaimRlcBusBindingV1<F>>,
) -> ClaimRlcRawRowV1<F> {
    let mut values = [F::ZERO; CLAIM_RLC_COLUMNS];
    values[CLAIM_RLC_CHALLENGE_A] = F::from_u128(state.challenge_a);
    values[CLAIM_RLC_CHALLENGE_B] = F::from_u128(state.challenge_b);
    values[CLAIM_RLC_ACCUMULATOR_A] = F::from_u128(state.accumulator_a);
    values[CLAIM_RLC_ACCUMULATOR_B] = F::from_u128(state.accumulator_b);
    values[CLAIM_RLC_QUOTIENT_PACK] = F::from_u128(state.quotient_pack);
    values[CLAIM_RLC_COEFFICIENT] = F::from_u128(state.coefficient);
    ClaimRlcRawRowV1 {
        values,
        mode,
        store_pack: false,
        load_pack: false,
        ternary_power: F::ZERO,
        binding,
    }
}

fn claim_rlc_push_evaluation_rows_v1<F: KagemushaPoseidonFieldV1>(
    rows: &mut Vec<ClaimRlcRawRowV1<F>>,
    state: &mut ClaimRlcStateV1,
    pack_load: Option<(usize, usize)>,
) -> Result<(), String> {
    let (quotient_a, remainder_a) =
        claim_rlc_native_step_v1(state.accumulator_a, state.challenge_a, state.coefficient)?;
    let binding = pack_load.map(|(carrier, pack)| ClaimRlcBusBindingV1::PackLoad { carrier, pack });
    let mut evaluate_a = claim_rlc_state_row_v1(*state, ClaimRlcRowModeV1::EvaluateA, binding);
    evaluate_a.values[CLAIM_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_a);
    evaluate_a.values[CLAIM_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_a);
    evaluate_a.values[CLAIM_RLC_REMAINDER_INVERSE] =
        claim_rlc_non_modulus_inverse_v1::<F>(remainder_a)?;
    if pack_load.is_some() {
        evaluate_a.load_pack = true;
        evaluate_a.values[CLAIM_RLC_BUS] = F::from_u128(state.coefficient);
    }
    claim_rlc_set_range_limbs_v1(&mut evaluate_a, quotient_a, remainder_a);
    rows.push(evaluate_a);
    state.accumulator_a = remainder_a;

    let (quotient_b, remainder_b) =
        claim_rlc_native_step_v1(state.accumulator_b, state.challenge_b, state.coefficient)?;
    let mut evaluate_b = claim_rlc_state_row_v1(*state, ClaimRlcRowModeV1::EvaluateB, None);
    evaluate_b.values[CLAIM_RLC_DIVISION_QUOTIENT] = F::from_u128(quotient_b);
    evaluate_b.values[CLAIM_RLC_DIVISION_REMAINDER] = F::from_u128(remainder_b);
    evaluate_b.values[CLAIM_RLC_REMAINDER_INVERSE] =
        claim_rlc_non_modulus_inverse_v1::<F>(remainder_b)?;
    claim_rlc_set_range_limbs_v1(&mut evaluate_b, quotient_b, remainder_b);
    rows.push(evaluate_b);
    state.accumulator_b = remainder_b;
    Ok(())
}

fn claim_rlc_native_step_v1(
    accumulator: u128,
    challenge: u128,
    coefficient: u128,
) -> Result<(u128, u128), String> {
    let modulus = U256::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1);
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
    if quotient >= (1_u128 << 126) || remainder >= CLAIM_CARRIER_RLC_MODULUS_V1 {
        return Err("claim RLC division output exceeds its proven bound".to_owned());
    }
    Ok((quotient, remainder))
}

fn claim_rlc_non_modulus_inverse_v1<F: KagemushaPoseidonFieldV1>(
    remainder: u128,
) -> Result<F, String> {
    Option::<F>::from(
        (F::from_u128(remainder) - F::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1)).invert(),
    )
    .ok_or_else(|| "claim RLC remainder is not canonical".to_owned())
}

fn claim_rlc_set_range_limbs_v1<F: KagemushaPoseidonFieldV1>(
    row: &mut ClaimRlcRawRowV1<F>,
    first: u128,
    second: u128,
) {
    let first_top_bits = match row.mode {
        ClaimRlcRowModeV1::Preprocess => 8,
        ClaimRlcRowModeV1::EvaluateA | ClaimRlcRowModeV1::EvaluateB => 6,
        _ => unreachable!("range limbs only occur on claim RLC arithmetic rows"),
    };
    row.values[CLAIM_RLC_SCALED_FIRST_TOP] =
        F::from_u128((first >> 120) << (CLAIM_RLC_RADIX_BITS - first_top_bits));
    row.values[CLAIM_RLC_SCALED_SECOND_TOP] =
        F::from_u128((second >> 120) << (CLAIM_RLC_RADIX_BITS - 7));
    for (half, mut value) in [first, second].into_iter().enumerate() {
        for limb in 0..9 {
            row.values[CLAIM_RLC_RANGE_START + half * 9 + limb] =
                F::from_u128(value & (CLAIM_RLC_RADIX - 1));
            value >>= CLAIM_RLC_RADIX_BITS;
        }
        debug_assert_eq!(value, 0);
    }
}

/// Typed, parity-specific plan commitment consumed by every shard and claim step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimPlanV1 {
    pub(crate) parity: KagemushaPastaParityV1,
    pub(crate) release_id: DigestV1,
    pub(crate) total_stages: u64,
    pub(crate) total_jobs: u32,
    /// Expected ordered commitment to every canonically padded message block.
    pub(crate) expected_message_root: DigestV1,
    pub(crate) expected_terminal_root: DigestV1,
    pub(crate) plan_binding: DigestV1,
}

impl KagemushaMintHashClaimPlanV1 {
    /// Derive the complete plan commitment from every exact ordered compression leaf.
    ///
    /// `plan_binding` is deliberately ignored while deriving the commitment: callers first build
    /// provisional leaves, derive this plan, then rebuild the same leaves with the returned
    /// binding. Every position, padded block word and terminal state is committed here.
    pub(crate) fn from_leaves<F: KagemushaPoseidonFieldV1>(
        release_id: DigestV1,
        leaves: &[KagemushaMintHashShardStatementV1],
    ) -> Result<Self, String> {
        if release_id == [0; 32] || leaves.is_empty() {
            return Err("mint hash claim plan is empty or release-unbound".to_owned());
        }
        let total_stages = u64::try_from(leaves.len())
            .map_err(|_| "mint hash claim stage count exceeds u64".to_owned())?;
        let parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        let mut jobs = Vec::new();
        // The seed commits the final job count. Derive it first from exact terminal positions.
        let total_jobs = leaves
            .last()
            .and_then(|leaf| leaf.job_index.checked_add(1))
            .ok_or_else(|| "mint hash claim terminal job index overflowed".to_owned())?;
        let mut expected_message_root =
            message_seed_native::<F>(release_id, total_stages, total_jobs);
        let mut expected_stage = 0_u64;
        let mut expected_job = 0_u32;
        let mut expected_block = 0_u32;
        let mut active_blocks = 0_u32;
        let mut chaining_state = IV;
        for leaf in leaves {
            leaf.validate_shape()?;
            if leaf.parity != parity
                || leaf.release_id != release_id
                || leaf.stage_index != expected_stage
                || leaf.job_index != expected_job
                || leaf.block_index != expected_block
                || leaf.initial_state != chaining_state
                || (expected_block == 0 && active_blocks != 0)
                || (expected_block != 0 && active_blocks != leaf.job_block_count)
            {
                return Err("mint hash plan leaves are not one exact ordered job stream".to_owned());
            }
            expected_message_root = message_step_native::<F>(expected_message_root, leaf);
            expected_stage = expected_stage
                .checked_add(1)
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())?;
            if leaf.is_final_block() {
                jobs.push((leaf.job_block_count, leaf.output_state));
                expected_job = expected_job
                    .checked_add(1)
                    .ok_or_else(|| "mint hash claim job count overflowed u32".to_owned())?;
                expected_block = 0;
                active_blocks = 0;
                chaining_state = IV;
            } else {
                expected_block = expected_block
                    .checked_add(1)
                    .ok_or_else(|| "mint hash claim block index overflowed u32".to_owned())?;
                active_blocks = leaf.job_block_count;
                chaining_state = leaf.output_state;
            }
        }
        if expected_stage != total_stages
            || expected_job != total_jobs
            || expected_block != 0
            || active_blocks != 0
            || chaining_state != IV
        {
            return Err("mint hash claim leaves end inside a job".to_owned());
        }
        let total_jobs = u32::try_from(jobs.len())
            .map_err(|_| "mint hash claim job count exceeds u32".to_owned())?;
        let counted_stages = jobs.iter().try_fold(0_u64, |total, (blocks, _)| {
            if *blocks == 0 {
                return Err("mint hash claim contains a zero-block job".to_owned());
            }
            total
                .checked_add(u64::from(*blocks))
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())
        })?;
        if counted_stages != total_stages {
            return Err("mint hash claim job block counts do not sum to total stages".to_owned());
        }
        let mut root = terminal_seed_native::<F>(release_id, total_stages, total_jobs);
        for (job_index, (block_count, terminal)) in jobs.iter().enumerate() {
            root = terminal_step_native::<F>(
                root,
                u32::try_from(job_index).expect("bounded terminal index"),
                *block_count,
                *terminal,
            );
        }
        let expected_terminal_root = encode(root);
        let expected_message_root = encode(expected_message_root);
        let plan_binding = encode(plan_binding_native::<F>(
            release_id,
            total_stages,
            total_jobs,
            decode::<F>(expected_message_root).expect("fresh message root is canonical"),
            root,
        ));
        Ok(Self {
            parity,
            release_id,
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root,
            plan_binding,
        })
    }

    /// Rebuild a plan from an independently constrained message root and exact job terminals.
    ///
    /// The monetary consumer uses this form after it has committed its own canonical padded
    /// message words in-circuit. A host-provided message root alone is not monetary authority.
    pub(crate) fn from_job_terminals_and_message_root<F: KagemushaPoseidonFieldV1>(
        release_id: DigestV1,
        total_stages: u64,
        jobs: &[(u32, [u32; DIGEST_SIZE])],
        expected_message_root: DigestV1,
    ) -> Result<Self, String> {
        if release_id == [0; 32]
            || total_stages == 0
            || jobs.is_empty()
            || decode::<F>(expected_message_root).is_none()
        {
            return Err("mint hash claim plan is empty or message/release-unbound".to_owned());
        }
        let total_jobs = u32::try_from(jobs.len())
            .map_err(|_| "mint hash claim job count exceeds u32".to_owned())?;
        let counted_stages = jobs.iter().try_fold(0_u64, |total, (blocks, _)| {
            if *blocks == 0 {
                return Err("mint hash claim contains a zero-block job".to_owned());
            }
            total
                .checked_add(u64::from(*blocks))
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())
        })?;
        if counted_stages != total_stages {
            return Err("mint hash claim job block counts do not sum to total stages".to_owned());
        }
        let parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        let mut terminal = terminal_seed_native::<F>(release_id, total_stages, total_jobs);
        for (job_index, (block_count, output)) in jobs.iter().enumerate() {
            terminal = terminal_step_native::<F>(
                terminal,
                u32::try_from(job_index).expect("bounded terminal index"),
                *block_count,
                *output,
            );
        }
        let message = decode::<F>(expected_message_root).expect("checked canonical message root");
        Ok(Self {
            parity,
            release_id,
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root: encode(terminal),
            plan_binding: encode(plan_binding_native::<F>(
                release_id,
                total_stages,
                total_jobs,
                message,
                terminal,
            )),
        })
    }

    fn validate<F: KagemushaPoseidonFieldV1>(&self) -> Result<(), String> {
        let expected_parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        if self.parity != expected_parity
            || self.release_id == [0; 32]
            || self.total_stages == 0
            || self.total_jobs == 0
            || u64::from(self.total_jobs) > self.total_stages
        {
            return Err("mint hash claim plan shape is invalid".to_owned());
        }
        let message = decode::<F>(self.expected_message_root)
            .ok_or_else(|| "mint hash message root is not a canonical scalar".to_owned())?;
        let terminal = decode::<F>(self.expected_terminal_root)
            .ok_or_else(|| "mint hash terminal root is not a canonical scalar".to_owned())?;
        let expected = encode(plan_binding_native::<F>(
            self.release_id,
            self.total_stages,
            self.total_jobs,
            message,
            terminal,
        ));
        if self.plan_binding != expected {
            return Err("mint hash typed-plan binding does not match its totals/root".to_owned());
        }
        Ok(())
    }
}

/// Constant-size public progress claimed after consuming one or more shard proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimStateV1 {
    pub(crate) plan: KagemushaMintHashClaimPlanV1,
    /// Number of leaves consumed; also the exact next global stage index.
    pub(crate) next_stage: u64,
    /// Exact next job index.
    pub(crate) next_job: u32,
    /// Exact next block within `next_job`, or zero at a job boundary.
    pub(crate) next_block: u32,
    /// Fixed block count of the active job, or zero at a job boundary.
    pub(crate) active_job_blocks: u32,
    /// SHA chaining state for the active job; exactly IV at a job boundary.
    pub(crate) chaining_state: [u32; DIGEST_SIZE],
    /// Ordered field-native fold of every completed job digest.
    pub(crate) terminal_root: DigestV1,
    /// Ordered field-native fold of every exact padded message block consumed so far.
    pub(crate) message_root: DigestV1,
    /// True only for the exact final stage, job boundary, and terminal root.
    pub(crate) complete: bool,
}

impl KagemushaMintHashClaimStateV1 {
    /// Apply exactly one valid shard statement to the prior claim, or establish the first claim.
    pub(crate) fn apply<F: KagemushaPoseidonFieldV1>(
        plan: KagemushaMintHashClaimPlanV1,
        previous: Option<Self>,
        leaf: &KagemushaMintHashShardStatementV1,
    ) -> Result<Self, String> {
        plan.validate::<F>()?;
        if leaf.parity != plan.parity
            || leaf.release_id != plan.release_id
            || leaf.plan_binding != plan.plan_binding
            || leaf.stage_index >= plan.total_stages
            || leaf.job_index >= plan.total_jobs
            || leaf.job_block_count == 0
            || leaf.block_index >= leaf.job_block_count
        {
            return Err("mint hash leaf is outside its authenticated typed plan".to_owned());
        }
        let (
            prior_stage,
            prior_job,
            prior_block,
            prior_blocks,
            prior_state,
            prior_terminal_root,
            prior_message_root,
        ) = if let Some(previous) = previous {
            previous.validate::<F>()?;
            if previous.plan != plan || previous.complete {
                return Err("mint hash predecessor is from another or completed plan".to_owned());
            }
            (
                previous.next_stage,
                previous.next_job,
                previous.next_block,
                previous.active_job_blocks,
                previous.chaining_state,
                decode::<F>(previous.terminal_root).ok_or_else(|| {
                    "mint hash predecessor root is not a canonical scalar".to_owned()
                })?,
                decode::<F>(previous.message_root).ok_or_else(|| {
                    "mint hash predecessor message root is not a canonical scalar".to_owned()
                })?,
            )
        } else {
            (
                0,
                0,
                0,
                0,
                IV,
                terminal_seed_native::<F>(plan.release_id, plan.total_stages, plan.total_jobs),
                message_seed_native::<F>(plan.release_id, plan.total_stages, plan.total_jobs),
            )
        };
        if leaf.stage_index != prior_stage
            || leaf.job_index != prior_job
            || leaf.block_index != prior_block
            || leaf.initial_state != prior_state
            || (prior_block == 0 && prior_blocks != 0)
            || (prior_block != 0 && prior_blocks != leaf.job_block_count)
        {
            return Err(
                "mint hash leaf is omitted, reordered, duplicated, or mis-chained".to_owned(),
            );
        }
        let final_block = leaf.block_index + 1 == leaf.job_block_count;
        let (next_job, next_block, active_job_blocks, chaining_state, terminal_root) =
            if final_block {
                (
                    leaf.job_index
                        .checked_add(1)
                        .ok_or_else(|| "mint hash job index overflowed".to_owned())?,
                    0,
                    0,
                    IV,
                    terminal_step_native::<F>(
                        prior_terminal_root,
                        leaf.job_index,
                        leaf.job_block_count,
                        leaf.output_state,
                    ),
                )
            } else {
                (
                    leaf.job_index,
                    leaf.block_index
                        .checked_add(1)
                        .ok_or_else(|| "mint hash block index overflowed".to_owned())?,
                    leaf.job_block_count,
                    leaf.output_state,
                    prior_terminal_root,
                )
            };
        let message_root = message_step_native::<F>(prior_message_root, leaf);
        let next_stage = leaf
            .stage_index
            .checked_add(1)
            .ok_or_else(|| "mint hash stage index overflowed".to_owned())?;
        let expected_root = decode::<F>(plan.expected_terminal_root)
            .ok_or_else(|| "mint hash expected root is not canonical".to_owned())?;
        let expected_message_root = decode::<F>(plan.expected_message_root)
            .ok_or_else(|| "mint hash expected message root is not canonical".to_owned())?;
        let complete = next_stage == plan.total_stages
            && next_job == plan.total_jobs
            && next_block == 0
            && terminal_root == expected_root
            && message_root == expected_message_root;
        if next_stage == plan.total_stages && !complete {
            return Err("mint hash terminal stage does not complete the typed plan".to_owned());
        }
        let state = Self {
            plan,
            next_stage,
            next_job,
            next_block,
            active_job_blocks,
            chaining_state,
            terminal_root: encode(terminal_root),
            message_root: encode(message_root),
            complete,
        };
        state.validate::<F>()?;
        Ok(state)
    }

    fn validate<F: KagemushaPoseidonFieldV1>(&self) -> Result<(), String> {
        self.plan.validate::<F>()?;
        let root = decode::<F>(self.terminal_root)
            .ok_or_else(|| "mint hash claim root is not canonical".to_owned())?;
        let message_root = decode::<F>(self.message_root)
            .ok_or_else(|| "mint hash claim message root is not canonical".to_owned())?;
        let at_boundary = self.next_block == 0;
        if self.next_stage == 0
            || self.next_stage > self.plan.total_stages
            || self.next_job > self.plan.total_jobs
            || (at_boundary && (self.active_job_blocks != 0 || self.chaining_state != IV))
            || (!at_boundary && self.active_job_blocks <= self.next_block)
        {
            return Err("mint hash claim cursor/state shape is invalid".to_owned());
        }
        let exact_complete = self.next_stage == self.plan.total_stages
            && self.next_job == self.plan.total_jobs
            && at_boundary
            && root
                == decode::<F>(self.plan.expected_terminal_root).ok_or_else(|| {
                    "mint hash expected terminal root is not canonical".to_owned()
                })?
            && message_root
                == decode::<F>(self.plan.expected_message_root)
                    .ok_or_else(|| "mint hash expected message root is not canonical".to_owned())?;
        if self.complete != exact_complete {
            return Err(
                "mint hash completeness bit is not derived from exact terminal state".to_owned(),
            );
        }
        Ok(())
    }
}

/// The paired claim state. Each parity proves its own plan, roots, and SHA chaining state while
/// carrying the other component as cross-audited public data. Only the fixed job/block cursor and
/// completeness bit are common because parity-tagged signature challenges intentionally produce
/// different SHA transcripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimPairStateV1 {
    pub(crate) eq: KagemushaMintHashClaimStateV1,
    pub(crate) ep: KagemushaMintHashClaimStateV1,
}

impl KagemushaMintHashClaimPairStateV1 {
    fn validate(&self) -> Result<(), String> {
        self.eq.validate::<Fp>()?;
        self.ep.validate::<Fq>()?;
        if self.eq.plan.parity != KagemushaPastaParityV1::Eq
            || self.ep.plan.parity != KagemushaPastaParityV1::Ep
            || self.eq.plan.release_id != self.ep.plan.release_id
            || self.eq.plan.total_stages != self.ep.plan.total_stages
            || self.eq.plan.total_jobs != self.ep.plan.total_jobs
            || self.eq.next_stage != self.ep.next_stage
            || self.eq.next_job != self.ep.next_job
            || self.eq.next_block != self.ep.next_block
            || self.eq.active_job_blocks != self.ep.active_job_blocks
            || self.eq.complete != self.ep.complete
        {
            return Err("mint hash paired claim state does not share one fixed cursor".to_owned());
        }
        Ok(())
    }
}

/// Release-pinned recursive protocol identities and per-step paired audit/proof bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimMetadataV1 {
    pub(crate) eq_claim_protocol: DigestV1,
    pub(crate) ep_claim_protocol: DigestV1,
    pub(crate) eq_shard_protocol: DigestV1,
    pub(crate) ep_shard_protocol: DigestV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
    /// Field-native chain root over the exact recursively consumed Eq ordinary proof bytes.
    pub(crate) eq_proof_chain_root: DigestV1,
    /// Field-native chain root over the exact recursively consumed Ep ordinary proof bytes.
    pub(crate) ep_proof_chain_root: DigestV1,
}

impl KagemushaMintHashClaimMetadataV1 {
    fn validate(&self) -> Result<(), String> {
        let identities = [
            self.eq_claim_protocol,
            self.ep_claim_protocol,
            self.eq_shard_protocol,
            self.ep_shard_protocol,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
            self.eq_proof_chain_root,
            self.ep_proof_chain_root,
        ];
        if identities.contains(&[0; 32])
            || self.eq_claim_protocol == self.ep_claim_protocol
            || self.eq_shard_protocol == self.ep_shard_protocol
        {
            return Err("mint hash claim metadata is absent or parity-aliased".to_owned());
        }
        Ok(())
    }
}

/// Fixed public instance layout of the paired claim-fold carrier.
pub(crate) mod public_instance {
    pub(crate) const VERSION: usize = 0;
    pub(crate) const PARITY: usize = 1;
    pub(crate) const COMPLETE: usize = 2;
    pub(crate) const RELEASE_LO: usize = 3;
    pub(crate) const EQ_PLAN_LO: usize = 5;
    pub(crate) const EP_PLAN_LO: usize = 7;
    pub(crate) const TOTAL_STAGES: usize = 9;
    pub(crate) const TOTAL_JOBS: usize = 10;
    pub(crate) const NEXT_STAGE: usize = 11;
    pub(crate) const NEXT_JOB: usize = 12;
    pub(crate) const NEXT_BLOCK: usize = 13;
    pub(crate) const ACTIVE_JOB_BLOCKS: usize = 14;
    /// First word of the Eq queue's active SHA chaining state.
    pub(crate) const EQ_CHAINING_STATE: usize = 15;
    /// First word of the Ep queue's active SHA chaining state.
    pub(crate) const EP_CHAINING_STATE: usize = 23;
    pub(crate) const EQ_MESSAGE_ROOT_LO: usize = 31;
    pub(crate) const EP_MESSAGE_ROOT_LO: usize = 33;
    pub(crate) const EQ_TERMINAL_ROOT_LO: usize = 35;
    pub(crate) const EP_TERMINAL_ROOT_LO: usize = 37;
    pub(crate) const EQ_EXPECTED_MESSAGE_ROOT_LO: usize = 39;
    pub(crate) const EP_EXPECTED_MESSAGE_ROOT_LO: usize = 41;
    pub(crate) const EQ_EXPECTED_ROOT_LO: usize = 43;
    pub(crate) const EP_EXPECTED_ROOT_LO: usize = 45;
    pub(crate) const EQ_CLAIM_PROTOCOL_LO: usize = 47;
    pub(crate) const EP_CLAIM_PROTOCOL_LO: usize = 49;
    pub(crate) const EQ_SHARD_PROTOCOL_LO: usize = 51;
    pub(crate) const EP_SHARD_PROTOCOL_LO: usize = 53;
    pub(crate) const EQ_AUDIT_LO: usize = 55;
    pub(crate) const EP_AUDIT_LO: usize = 57;
    pub(crate) const EQ_PROOF_CHAIN_LO: usize = 59;
    pub(crate) const EP_PROOF_CHAIN_LO: usize = 61;
    pub(crate) const HISTORY_START: usize = 63;
    /// Eq proof commitment to the Eq carrier instance column.
    pub(crate) const EQ_PROOF_EQ_CARRIER_COMMITMENT_LO: usize =
        super::KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1;
    /// Eq proof commitment to the Ep carrier instance column.
    pub(crate) const EQ_PROOF_EP_CARRIER_COMMITMENT_LO: usize =
        EQ_PROOF_EQ_CARRIER_COMMITMENT_LO + 2;
    /// Ep proof commitment to the Eq carrier instance column.
    pub(crate) const EP_PROOF_EQ_CARRIER_COMMITMENT_LO: usize =
        EQ_PROOF_EP_CARRIER_COMMITMENT_LO + 2;
    /// Ep proof commitment to the Ep carrier instance column.
    pub(crate) const EP_PROOF_EP_CARRIER_COMMITMENT_LO: usize =
        EP_PROOF_EQ_CARRIER_COMMITMENT_LO + 2;
    /// Low-125-bit-plus-one Poseidon challenge derived by the Eq proof.
    pub(crate) const CARRIER_RLC_EQ_CHALLENGE: usize = EP_PROOF_EP_CARRIER_COMMITMENT_LO + 2;
    /// Low-125-bit-plus-one Poseidon challenge derived by the Ep proof.
    pub(crate) const CARRIER_RLC_EP_CHALLENGE: usize = CARRIER_RLC_EQ_CHALLENGE + 1;
    pub(crate) const EQ_CARRIER_AT_EQ_CHALLENGE: usize = CARRIER_RLC_EP_CHALLENGE + 1;
    pub(crate) const EQ_CARRIER_AT_EP_CHALLENGE: usize = EQ_CARRIER_AT_EQ_CHALLENGE + 1;
    pub(crate) const EP_CARRIER_AT_EQ_CHALLENGE: usize = EQ_CARRIER_AT_EP_CHALLENGE + 1;
    pub(crate) const EP_CARRIER_AT_EP_CHALLENGE: usize = EP_CARRIER_AT_EQ_CHALLENGE + 1;
    pub(crate) const CARRIER_BINDING_END: usize = EP_CARRIER_AT_EP_CHALLENGE + 1;
}

/// Exact constant public cell count, including one 544-byte k=16 history.
pub(crate) const KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1: usize =
    public_instance::HISTORY_START + 34;
/// Internal hybrid semantic column. The first 97 values are the stable external
/// claim ABI; the final fourteen authenticate both carrier columns in both
/// proofs and bind their common-prime equality checks.
pub(super) const KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1: usize =
    public_instance::CARRIER_BINDING_END;
pub(crate) const KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1: usize =
    KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        - KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1;
/// Cross-parity public values absorbed after the deferred equation inventory.
const KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1: usize = 58;
/// Fixed proof-internal carrier capacity for the complete two-lane deferred audit.
///
/// Each k=16 dense lane authenticates at most 504 sources. Every source contributes four
/// canonical `u128` cells and the cross-parity binding contributes another 58 cells, so 4,090 is
/// the exact no-truncation capacity for all 1,008 sources accepted by the configured machine.
pub(super) const KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1: usize = 4_090;
const KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1: usize =
    (KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        - KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1)
        / 4;

/// Equality-bind one terminal claim public column to the exact ordinary SHA queue built by the
/// monetary relation.
///
/// Recursive verification of the claim proof is deliberately a caller responsibility because the
/// caller owns the carried-history fold.  This gadget supplies the other half of that bridge: it
/// reconstructs the message, terminal, and plan roots from assigned SHA message/output cells and
/// pins every release/protocol/cursor/completeness cell.  A host-computed digest can therefore
/// never substitute for the canonical circuit bytes.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn constrain_complete_claim_against_sha_jobs_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    jobs: &crate::zk::pasta_sha256::PastaSha256JobsV1<F>,
    claim: &[AssignedValue<F>],
    parity: KagemushaPastaParityV1,
    expected_release: [AssignedValue<F>; 2],
    expected_eq_claim_protocol: [AssignedValue<F>; 2],
    expected_ep_claim_protocol: [AssignedValue<F>; 2],
    expected_eq_shard_protocol: [AssignedValue<F>; 2],
    expected_ep_shard_protocol: [AssignedValue<F>; 2],
) -> Result<(), String> {
    use crate::zk::{
        pasta_sha256::PastaSha256ByteV1,
        pasta_sha256_table8::{BLOCK_BYTE_SIZE, canonical_padding_suffix},
    };

    if claim.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("terminal mint hash claim public column has wrong shape".to_owned());
    }
    let claimed_jobs = jobs.claim_jobs()?;
    if claimed_jobs.is_empty() {
        return Err("terminal mint hash claim cannot authorize an empty SHA queue".to_owned());
    }
    let total_jobs = u32::try_from(claimed_jobs.len())
        .map_err(|_| "terminal mint hash claim job count exceeds u32".to_owned())?;
    let total_stages = claimed_jobs.iter().try_fold(0_u64, |total, job| {
        let suffix = canonical_padding_suffix(job.message.len())
            .ok_or_else(|| "terminal mint hash claim message length is not encodable".to_owned())?;
        let padded = job
            .message
            .len()
            .checked_add(suffix.len())
            .ok_or_else(|| "terminal mint hash claim padded length overflowed".to_owned())?;
        let blocks = u64::try_from(padded / BLOCK_BYTE_SIZE)
            .map_err(|_| "terminal mint hash claim block count exceeds u64".to_owned())?;
        total
            .checked_add(blocks)
            .ok_or_else(|| "terminal mint hash claim stage count overflowed".to_owned())
    })?;
    if total_stages == 0 {
        return Err("terminal mint hash claim contains no compression blocks".to_owned());
    }

    let gate = range.gate();
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let stages = ctx.load_constant(F::from(total_stages));
    let job_count = ctx.load_constant(F::from(u64::from(total_jobs)));
    let message_seed = poseidon.hash(
        ctx,
        range,
        MESSAGE_SEED_DOMAIN_V1,
        &[expected_release[0], expected_release[1], stages, job_count],
    );
    let terminal_seed = poseidon.hash(
        ctx,
        range,
        TERMINAL_SEED_DOMAIN_V1,
        &[expected_release[0], expected_release[1], stages, job_count],
    );
    let mut message_root = message_seed;
    let mut terminal_root = terminal_seed;
    let mut stage_index = 0_u64;
    for (job_index, job) in claimed_jobs.iter().enumerate() {
        let suffix = canonical_padding_suffix(job.message.len())
            .ok_or_else(|| "terminal mint hash claim message length is not encodable".to_owned())?;
        let padded = job
            .message
            .iter()
            .copied()
            .chain(suffix.into_iter().map(PastaSha256ByteV1::constant))
            .collect::<Vec<_>>();
        if padded.is_empty() || padded.len() % BLOCK_BYTE_SIZE != 0 {
            return Err("terminal mint hash claim padding is not block aligned".to_owned());
        }
        let blocks = u32::try_from(padded.len() / BLOCK_BYTE_SIZE)
            .map_err(|_| "terminal mint hash claim job block count exceeds u32".to_owned())?;
        for (block_index, block) in padded.chunks_exact(BLOCK_BYTE_SIZE).enumerate() {
            let words = block
                .chunks_exact(4)
                .map(|bytes| {
                    gate.inner_product(
                        ctx,
                        bytes.iter().copied().map(PastaSha256ByteV1::quantum_cell),
                        [
                            Constant(F::from(1_u64 << 24)),
                            Constant(F::from(1_u64 << 16)),
                            Constant(F::from(1_u64 << 8)),
                            Constant(F::ONE),
                        ],
                    )
                })
                .collect::<Vec<_>>();
            if words.len() != BLOCK_SIZE {
                return Err("terminal mint hash claim block word shape drifted".to_owned());
            }
            let mut inputs = Vec::with_capacity(5 + BLOCK_SIZE);
            inputs.extend([
                message_root,
                ctx.load_constant(F::from(stage_index)),
                ctx.load_constant(F::from(u64::try_from(job_index).map_err(|_| {
                    "terminal mint hash claim job index exceeds u64".to_owned()
                })?)),
                ctx.load_constant(F::from(u64::try_from(block_index).map_err(|_| {
                    "terminal mint hash claim block index exceeds u64".to_owned()
                })?)),
                ctx.load_constant(F::from(u64::from(blocks))),
            ]);
            inputs.extend(words);
            message_root = poseidon.hash(ctx, range, MESSAGE_STEP_DOMAIN_V1, &inputs);
            stage_index = stage_index
                .checked_add(1)
                .ok_or_else(|| "terminal mint hash claim stage index overflowed".to_owned())?;
        }
        let mut terminal_inputs = Vec::with_capacity(3 + DIGEST_SIZE);
        terminal_inputs.extend([
            terminal_root,
            ctx.load_constant(F::from(u64::try_from(job_index).map_err(|_| {
                "terminal mint hash claim job index exceeds u64".to_owned()
            })?)),
            ctx.load_constant(F::from(u64::from(blocks))),
        ]);
        for word in job.output_words.iter().copied() {
            range.range_check(ctx, word, 32);
            terminal_inputs.push(word);
        }
        terminal_root = poseidon.hash(ctx, range, TERMINAL_STEP_DOMAIN_V1, &terminal_inputs);
    }
    if stage_index != total_stages {
        return Err("terminal mint hash claim stage inventory drifted".to_owned());
    }
    let plan = poseidon.hash(
        ctx,
        range,
        PLAN_DOMAIN_V1,
        &[
            expected_release[0],
            expected_release[1],
            stages,
            job_count,
            message_root,
            terminal_root,
        ],
    );

    let expected_parity = match parity {
        KagemushaPastaParityV1::Eq => F::ZERO,
        KagemushaPastaParityV1::Ep => F::ONE,
    };
    for (actual, expected) in [
        (claim[public_instance::VERSION], F::ONE),
        (claim[public_instance::PARITY], expected_parity),
        (claim[public_instance::COMPLETE], F::ONE),
        (claim[public_instance::TOTAL_STAGES], F::from(total_stages)),
        (
            claim[public_instance::TOTAL_JOBS],
            F::from(u64::from(total_jobs)),
        ),
        (claim[public_instance::NEXT_STAGE], F::from(total_stages)),
        (
            claim[public_instance::NEXT_JOB],
            F::from(u64::from(total_jobs)),
        ),
        (claim[public_instance::NEXT_BLOCK], F::ZERO),
        (claim[public_instance::ACTIVE_JOB_BLOCKS], F::ZERO),
    ] {
        gate.assert_is_const(ctx, &actual, &expected);
    }
    for (actual, expected) in claim[public_instance::RELEASE_LO..public_instance::RELEASE_LO + 2]
        .iter()
        .copied()
        .zip(expected_release)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    for offset in [
        public_instance::EQ_CHAINING_STATE,
        public_instance::EP_CHAINING_STATE,
    ] {
        for (actual, expected) in claim[offset..offset + DIGEST_SIZE]
            .iter()
            .copied()
            .zip(IV.map(|word| F::from(u64::from(word))))
        {
            gate.assert_is_const(ctx, &actual, &expected);
        }
    }
    for (offset, expected) in [
        (
            public_instance::EQ_CLAIM_PROTOCOL_LO,
            expected_eq_claim_protocol,
        ),
        (
            public_instance::EP_CLAIM_PROTOCOL_LO,
            expected_ep_claim_protocol,
        ),
        (
            public_instance::EQ_SHARD_PROTOCOL_LO,
            expected_eq_shard_protocol,
        ),
        (
            public_instance::EP_SHARD_PROTOCOL_LO,
            expected_ep_shard_protocol,
        ),
    ] {
        for (actual, expected) in claim[offset..offset + 2].iter().copied().zip(expected) {
            ctx.constrain_equal(&actual, &expected);
        }
    }

    let plan_limbs = scalar_digest_limbs_v1(ctx, gate, plan);
    let message_limbs = scalar_digest_limbs_v1(ctx, gate, message_root);
    let terminal_limbs = scalar_digest_limbs_v1(ctx, gate, terminal_root);
    let own_plan_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let own_message_offsets = match parity {
        KagemushaPastaParityV1::Eq => [
            public_instance::EQ_MESSAGE_ROOT_LO,
            public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO,
        ],
        KagemushaPastaParityV1::Ep => [
            public_instance::EP_MESSAGE_ROOT_LO,
            public_instance::EP_EXPECTED_MESSAGE_ROOT_LO,
        ],
    };
    let own_terminal_offsets = match parity {
        KagemushaPastaParityV1::Eq => [
            public_instance::EQ_TERMINAL_ROOT_LO,
            public_instance::EQ_EXPECTED_ROOT_LO,
        ],
        KagemushaPastaParityV1::Ep => [
            public_instance::EP_TERMINAL_ROOT_LO,
            public_instance::EP_EXPECTED_ROOT_LO,
        ],
    };
    for (actual, expected) in claim[own_plan_offset..own_plan_offset + 2]
        .iter()
        .copied()
        .zip(plan_limbs)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    for offset in own_message_offsets {
        for (actual, expected) in claim[offset..offset + 2].iter().copied().zip(message_limbs) {
            ctx.constrain_equal(&actual, &expected);
        }
    }
    for offset in own_terminal_offsets {
        for (actual, expected) in claim[offset..offset + 2]
            .iter()
            .copied()
            .zip(terminal_limbs)
        {
            ctx.constrain_equal(&actual, &expected);
        }
    }
    Ok(())
}

/// One parity's private recursive witnesses.
#[derive(Clone, Copy)]
pub(crate) struct KagemushaMintHashClaimParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(crate) parent_protocol: &'a PlonkProtocol<C>,
    pub(crate) parent_instances: &'a [Vec<C::ScalarExt>],
    pub(crate) parent_proof: &'a [u8],
    pub(crate) parent_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(crate) parent_fold_proof: &'a [u8],
    pub(crate) shard_protocol: &'a PlonkProtocol<C>,
    pub(crate) shard_proof: &'a [u8],
    pub(crate) leaf_fold_proof: &'a [u8],
    pub(crate) successor_history: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Complete mutually audited claim-fold witness.
#[derive(Clone)]
pub(crate) struct KagemushaMintHashClaimPairWitnessV1<'a> {
    pub(crate) previous: Option<KagemushaMintHashClaimPairStateV1>,
    pub(crate) previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    pub(crate) successor: KagemushaMintHashClaimPairStateV1,
    pub(crate) metadata: KagemushaMintHashClaimMetadataV1,
    pub(crate) eq_leaf: KagemushaMintHashShardStatementV1,
    pub(crate) ep_leaf: KagemushaMintHashShardStatementV1,
    pub(crate) eq: KagemushaMintHashClaimParityWitnessV1<'a, EqAffine>,
    pub(crate) ep: KagemushaMintHashClaimParityWitnessV1<'a, EpAffine>,
}

/// Base and reciprocal dense-MSM configuration of the narrow k=16 claim fold.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaMintHashClaimConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    carrier_rlc: KagemushaClaimCarrierRlcConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp half of the paired claim fold.
#[derive(Clone)]
pub(crate) struct KagemushaMintHashClaimEqCircuitV1 {
    pub(crate) builder: BaseCircuitBuilder<Fp>,
    carrier_rlc: KagemushaClaimCarrierRlcMachineV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of the paired claim fold.
#[derive(Clone)]
pub(crate) struct KagemushaMintHashClaimEpCircuitV1 {
    pub(crate) builder: BaseCircuitBuilder<Fq>,
    carrier_rlc: KagemushaClaimCarrierRlcMachineV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_claim_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintHashClaimConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    carrier_rlc: self.carrier_rlc.unknown(),
                    dense_jobs: self.dense_jobs.unknown(),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaMintHashClaimConfigV1 {
                    base,
                    carrier_rlc: KagemushaClaimCarrierRlcConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure_with_lanes::<$opposite>(
                        meta,
                        KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1,
                    ),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )?;
                let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
                self.carrier_rlc.synthesize(
                    &config.carrier_rlc,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable_rows,
                )?;
                self.dense_jobs.synthesize(
                    &config.dense,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable_rows,
                )
            }
        }
    };
}

impl_claim_circuit!(
    KagemushaMintHashClaimEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq mint hash claim"
);
impl_claim_circuit!(
    KagemushaMintHashClaimEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep mint hash claim"
);

struct ClaimScalarHalfV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    builder: BaseCircuitBuilder<C::ScalarExt>,
    output: KagemushaNativeDeferredBatchV1<C>,
    common_cells: Vec<AssignedValue<C::ScalarExt>>,
}

/// Compact native deferred-audit witnesses discovered before either exact claim circuit is built.
///
/// The scalar builders used to derive these values are dropped before the reciprocal parity is
/// started. The retained outputs contain only the canonical points, coefficients, selectors,
/// tags, bound public values, and audit digest needed to rebuild either exact circuit later.
pub(crate) struct KagemushaMintHashClaimDeferredAuditsV1 {
    eq: KagemushaNativeDeferredBatchV1<EqAffine>,
    ep: KagemushaNativeDeferredBatchV1<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
    eq_carrier: Vec<u128>,
    ep_carrier: Vec<u128>,
    carrier_binding: ClaimCarrierBindingV1,
}

#[derive(Clone, Copy)]
struct ClaimCarrierCommitmentsV1 {
    eq_proof_eq_carrier: EqAffine,
    eq_proof_ep_carrier: EqAffine,
    ep_proof_eq_carrier: EpAffine,
    ep_proof_ep_carrier: EpAffine,
}

#[derive(Clone, Copy)]
struct ClaimCarrierBindingV1 {
    commitments: ClaimCarrierCommitmentsV1,
    eq_challenge: u128,
    ep_challenge: u128,
    eq_at_eq_challenge: u128,
    eq_at_ep_challenge: u128,
    ep_at_eq_challenge: u128,
    ep_at_ep_challenge: u128,
}

fn placeholder_claim_carrier_binding_v1() -> ClaimCarrierBindingV1 {
    ClaimCarrierBindingV1 {
        commitments: ClaimCarrierCommitmentsV1 {
            eq_proof_eq_carrier: EqAffine::generator(),
            eq_proof_ep_carrier: EqAffine::generator(),
            ep_proof_eq_carrier: EpAffine::generator(),
            ep_proof_ep_carrier: EpAffine::generator(),
        },
        eq_challenge: 1,
        ep_challenge: 1,
        eq_at_eq_challenge: 0,
        eq_at_ep_challenge: 0,
        ep_at_eq_challenge: 0,
        ep_at_ep_challenge: 0,
    }
}

impl KagemushaMintHashClaimDeferredAuditsV1 {
    #[must_use]
    pub(crate) const fn eq_digest(&self) -> DigestV1 {
        self.eq_digest
    }

    #[must_use]
    pub(crate) const fn ep_digest(&self) -> DigestV1 {
        self.ep_digest
    }

    #[must_use]
    pub(super) fn eq_inner_instances(&self, external: &[Fp]) -> Result<Vec<Vec<Fp>>, String> {
        claim_hybrid_instances_v1(
            external,
            &self.eq_carrier,
            &self.ep_carrier,
            self.carrier_binding,
        )
    }

    pub(super) fn ep_inner_instances(&self, external: &[Fq]) -> Result<Vec<Vec<Fq>>, String> {
        claim_hybrid_instances_v1(
            external,
            &self.eq_carrier,
            &self.ep_carrier,
            self.carrier_binding,
        )
    }

    pub(super) fn validate_release_inventory_v1(&self) -> Result<(), String> {
        let eq_sources = self.eq.batch.source_count();
        let ep_sources = self.ep.batch.source_count();
        let eq_carrier = self
            .eq
            .carrier_cells_v1()
            .map_err(|error| format!("Eq mint-hash claim carrier shape is invalid: {error:?}"))?;
        let ep_carrier = self
            .ep
            .carrier_cells_v1()
            .map_err(|error| format!("Ep mint-hash claim carrier shape is invalid: {error:?}"))?;
        if eq_sources > KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1
            || ep_sources > KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1
            || eq_carrier.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
            || ep_carrier.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        {
            return Err(format!(
                "mint-hash claim exceeds compact capacity: Eq S={eq_sources}/L={}, Ep S={ep_sources}/L={}, maximum S={}/L={}",
                eq_carrier.len(),
                ep_carrier.len(),
                KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1,
                KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            ));
        }
        Ok(())
    }
}

fn validate_claim_carrier_active_len_v1<C>(
    output: &KagemushaNativeDeferredBatchV1<C>,
    active_len: usize,
) -> Result<usize, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if output.bound_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
        || output.bound_u128_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
    {
        return Err("mint-hash active carrier has the wrong bound-value inventory".to_owned());
    }
    let expected = output
        .batch
        .source_count()
        .checked_mul(4)
        .and_then(|source_cells| source_cells.checked_add(output.bound_values.len()))
        .ok_or_else(|| "mint-hash active carrier length overflowed".to_owned())?;
    if active_len != expected
        || active_len == 0
        || active_len > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(format!(
            "mint-hash active carrier length is invalid: S={}, expected L={expected}, actual L={active_len}, capacity L={}",
            output.batch.source_count(),
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ));
    }
    Ok(active_len)
}

fn native_claim_carrier_u128_values_v1<C>(
    output: &KagemushaNativeDeferredBatchV1<C>,
) -> Result<Vec<u128>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let cells = output
        .carrier_cells_v1()
        .map_err(|error| format!("mint-hash claim carrier shape is invalid: {error:?}"))?;
    validate_claim_carrier_active_len_v1(output, cells.len())?;
    cells
        .into_iter()
        .map(|cell| assigned_u128_cell_v1(cell, "mint-hash claim carrier"))
        .collect()
}

fn padded_claim_carrier_u128_values_v1<C>(
    output: &KagemushaNativeDeferredBatchV1<C>,
) -> Result<Vec<u128>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let mut values = native_claim_carrier_u128_values_v1(output)?;
    if values.is_empty() || values.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 {
        return Err("mint-hash claim carrier has the wrong shape".to_owned());
    }
    values.resize(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1, 0);
    Ok(values)
}

fn assigned_u128_cell_v1<F: halo2_base::utils::ScalarField>(
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

fn canonical_claim_carrier_commitment_v1<C>(
    parameters: &ParamsIPA<C>,
    values: &[u128],
) -> Result<C, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 {
        return Err("mint-hash claim padded carrier has the wrong shape".to_owned());
    }
    let bases = parameters
        .get_g_lagrange()
        .get(..values.len())
        .ok_or_else(|| "mint-hash claim carrier exceeds the IPA domain".to_owned())?;
    let scalars = values
        .iter()
        .copied()
        .map(C::ScalarExt::from_u128)
        .collect::<Vec<_>>();
    let commitment =
        (best_multiexp::<C>(&scalars, bases) + parameters.get_blind_base().to_curve()).to_affine();
    if bool::from(commitment.is_identity()) {
        return Err("mint-hash claim carrier commitment is the identity".to_owned());
    }
    Ok(commitment)
}

fn point_u128_limbs_v1<C: CurveAffine>(point: C) -> [u128; 2] {
    let bytes = point.to_bytes();
    let bytes = bytes.as_ref();
    std::array::from_fn(|half| {
        u128::from_le_bytes(
            bytes[half * 16..(half + 1) * 16]
                .try_into()
                .expect("Pasta compressed point half has sixteen bytes"),
        )
    })
}

fn claim_carrier_commitment_limbs_v1(commitments: ClaimCarrierCommitmentsV1) -> [u128; 8] {
    let [eq_eq_0, eq_eq_1] = point_u128_limbs_v1(commitments.eq_proof_eq_carrier);
    let [eq_ep_0, eq_ep_1] = point_u128_limbs_v1(commitments.eq_proof_ep_carrier);
    let [ep_eq_0, ep_eq_1] = point_u128_limbs_v1(commitments.ep_proof_eq_carrier);
    let [ep_ep_0, ep_ep_1] = point_u128_limbs_v1(commitments.ep_proof_ep_carrier);
    [
        eq_eq_0, eq_eq_1, eq_ep_0, eq_ep_1, ep_eq_0, ep_eq_1, ep_ep_0, ep_ep_1,
    ]
}

fn claim_carrier_binding_values_v1(binding: ClaimCarrierBindingV1) -> [u128; 14] {
    let commitments = claim_carrier_commitment_limbs_v1(binding.commitments);
    [
        commitments[0],
        commitments[1],
        commitments[2],
        commitments[3],
        commitments[4],
        commitments[5],
        commitments[6],
        commitments[7],
        binding.eq_challenge,
        binding.ep_challenge,
        binding.eq_at_eq_challenge,
        binding.eq_at_ep_challenge,
        binding.ep_at_eq_challenge,
        binding.ep_at_ep_challenge,
    ]
}

fn native_claim_carrier_challenge_v1<F: KagemushaPoseidonFieldV1>(
    commitments: ClaimCarrierCommitmentsV1,
    parity: KagemushaPastaParityV1,
) -> u128 {
    let mut inputs = Vec::with_capacity(13);
    inputs.extend([
        F::from(CLAIM_CARRIER_RLC_VERSION_V1),
        F::from(match parity {
            KagemushaPastaParityV1::Eq => 1,
            KagemushaPastaParityV1::Ep => 2,
        }),
        F::from(u64::from(KAGEMUSHA_RECURSION_IPA_K_V1)),
        F::from(
            u64::try_from(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1)
                .expect("fixed carrier length fits u64"),
        ),
        F::from(2),
    ]);
    inputs.extend(
        claim_carrier_commitment_limbs_v1(commitments)
            .into_iter()
            .map(F::from_u128),
    );
    let digest = encode(hash::<F>(CLAIM_CARRIER_RLC_DOMAIN_V1, &inputs));
    let low = u128::from_le_bytes(
        digest[..16]
            .try_into()
            .expect("Pasta scalar low half has sixteen bytes"),
    );
    (low & ((1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1) - 1)) + 1
}

fn native_claim_carrier_rlc_v1(values: &[u128], challenge: u128) -> Result<u128, String> {
    if values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || challenge == 0
        || challenge > (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1)
    {
        return Err("mint-hash carrier RLC input shape is invalid".to_owned());
    }
    let modulus = U256::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1);
    let divisor = Option::<NonZero<U256>>::from(NonZero::new(modulus))
        .expect("fixed Mersenne RLC modulus is nonzero");
    let challenge = U256::from_u128(challenge);
    let mut accumulator = U256::ZERO;
    for coefficient in native_claim_carrier_coefficients_v1(values)? {
        let product = accumulator.wrapping_mul(&challenge);
        let numerator = product.wrapping_add(&U256::from_u128(coefficient));
        accumulator = numerator.div_rem(&divisor).1;
    }
    let bytes: [u8; 32] = accumulator.to_le_bytes();
    if bytes[16..].iter().any(|byte| *byte != 0) {
        return Err("mint-hash carrier RLC result exceeds u128".to_owned());
    }
    Ok(u128::from_le_bytes(
        bytes[..16]
            .try_into()
            .expect("RLC low half has sixteen bytes"),
    ))
}

/// Injectively encode a fixed `u128` carrier as coefficients below the common modulus.
///
/// A raw value is uniquely `(remainder, quotient)` for division by `M = 2^127 - 1`,
/// with the quotient in `{0, 1, 2}`. All remainders come first, followed by base-three
/// packs of eighty quotients. This cuts the polynomial from 8,180 to 4,142 coefficients
/// without introducing the `0 == M` alias that direct reduction would permit.
fn native_claim_carrier_coefficients_v1(values: &[u128]) -> Result<Vec<u128>, String> {
    if values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 {
        return Err("mint-hash carrier coefficient input shape is invalid".to_owned());
    }
    let mut coefficients = Vec::with_capacity(
        values.len()
            + values
                .len()
                .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1),
    );
    let mut quotients = Vec::with_capacity(values.len());
    for value in values.iter().copied() {
        coefficients.push(value % CLAIM_CARRIER_RLC_MODULUS_V1);
        let quotient = value / CLAIM_CARRIER_RLC_MODULUS_V1;
        if quotient >= CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1 {
            return Err("mint-hash carrier quotient is not a ternary digit".to_owned());
        }
        quotients.push(quotient);
    }
    for chunk in quotients.chunks(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
        let mut packed = 0_u128;
        let mut power = 1_u128;
        for (index, quotient) in chunk.iter().copied().enumerate() {
            packed = packed
                .checked_add(
                    quotient
                        .checked_mul(power)
                        .ok_or_else(|| "mint-hash quotient pack overflowed".to_owned())?,
                )
                .ok_or_else(|| "mint-hash quotient pack overflowed".to_owned())?;
            if index + 1 != chunk.len() {
                power = power
                    .checked_mul(CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1)
                    .ok_or_else(|| "mint-hash quotient radix overflowed".to_owned())?;
            }
        }
        if packed >= CLAIM_CARRIER_RLC_MODULUS_V1 {
            return Err("mint-hash quotient pack exceeds the RLC modulus".to_owned());
        }
        coefficients.push(packed);
    }
    Ok(coefficients)
}

fn derive_claim_carrier_binding_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_carrier: &[u128],
    ep_carrier: &[u128],
) -> Result<ClaimCarrierBindingV1, String> {
    let commitments = ClaimCarrierCommitmentsV1 {
        eq_proof_eq_carrier: canonical_claim_carrier_commitment_v1(eq_parameters, eq_carrier)?,
        eq_proof_ep_carrier: canonical_claim_carrier_commitment_v1(eq_parameters, ep_carrier)?,
        ep_proof_eq_carrier: canonical_claim_carrier_commitment_v1(ep_parameters, eq_carrier)?,
        ep_proof_ep_carrier: canonical_claim_carrier_commitment_v1(ep_parameters, ep_carrier)?,
    };
    let eq_challenge =
        native_claim_carrier_challenge_v1::<Fp>(commitments, KagemushaPastaParityV1::Eq);
    let ep_challenge =
        native_claim_carrier_challenge_v1::<Fq>(commitments, KagemushaPastaParityV1::Ep);
    Ok(ClaimCarrierBindingV1 {
        commitments,
        eq_challenge,
        ep_challenge,
        eq_at_eq_challenge: native_claim_carrier_rlc_v1(eq_carrier, eq_challenge)?,
        eq_at_ep_challenge: native_claim_carrier_rlc_v1(eq_carrier, ep_challenge)?,
        ep_at_eq_challenge: native_claim_carrier_rlc_v1(ep_carrier, eq_challenge)?,
        ep_at_ep_challenge: native_claim_carrier_rlc_v1(ep_carrier, ep_challenge)?,
    })
}

fn append_inner_carrier_binding_v1<F: KagemushaPoseidonFieldV1>(
    semantic: &mut Vec<F>,
    binding: ClaimCarrierBindingV1,
) -> Result<(), String> {
    if semantic.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint-hash claim semantic prefix has the wrong shape".to_owned());
    }
    semantic.extend(claim_carrier_binding_values_v1(binding).map(F::from_u128));
    if semantic.len() != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1 {
        return Err("mint-hash claim inner semantic layout drifted".to_owned());
    }
    Ok(())
}

/// Return the proof-internal three-column instance layout while retaining the
/// stable 97-value semantic prefix for external consumers.
fn claim_hybrid_instances_v1<F: KagemushaPoseidonFieldV1>(
    external_semantic: &[F],
    eq_carrier: &[u128],
    ep_carrier: &[u128],
    binding: ClaimCarrierBindingV1,
) -> Result<Vec<Vec<F>>, String> {
    let mut semantic = external_semantic.to_vec();
    append_inner_carrier_binding_v1(&mut semantic, binding)?;
    if eq_carrier.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || ep_carrier.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("mint-hash claim hybrid carriers have the wrong shape".to_owned());
    }
    Ok(vec![
        semantic,
        eq_carrier.iter().copied().map(F::from_u128).collect(),
        ep_carrier.iter().copied().map(F::from_u128).collect(),
    ])
}

pub(crate) fn canonical_claim_carrier_binding_tail_v1<F: KagemushaPoseidonFieldV1>(
    instances: &[Vec<F>],
) -> Result<[u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1], String> {
    use halo2_base::utils::fe_to_biguint;

    if instances.len() != 3
        || instances[0].len() != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || instances[1].len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || instances[2].len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(
            "mint-hash claim hybrid public shape is not exactly [111, 4090, 4090]".to_owned(),
        );
    }
    instances[0]
        [KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1..public_instance::CARRIER_BINDING_END]
        .iter()
        .map(|value| {
            let integer = fe_to_biguint(value);
            if integer.bits() > 128 {
                return Err("mint-hash claim carrier binding value exceeds u128".to_owned());
            }
            let digits = integer.to_u64_digits();
            Ok(u128::from(digits.first().copied().unwrap_or(0))
                | (u128::from(digits.get(1).copied().unwrap_or(0)) << 64))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "mint-hash claim carrier binding tail shape drifted".to_owned())
}

fn constrain_claim_carrier_challenge_v1<C>(
    loader: &DeferredLoader<'_, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parity: KagemushaPastaParityV1,
) -> Result<AssignedValue<C::ScalarExt>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let commitment_cells = public
        .get(
            public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO
                ..public_instance::CARRIER_RLC_EQ_CHALLENGE,
        )
        .ok_or_else(|| "mint-hash carrier commitment binding is truncated".to_owned())?;
    if commitment_cells.len() != 8 {
        return Err("mint-hash carrier commitment binding shape drifted".to_owned());
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let mut inputs = Vec::with_capacity(13);
    inputs.extend([
        ctx.main()
            .load_constant(C::ScalarExt::from(CLAIM_CARRIER_RLC_VERSION_V1)),
        ctx.main().load_constant(C::ScalarExt::from(match parity {
            KagemushaPastaParityV1::Eq => 1,
            KagemushaPastaParityV1::Ep => 2,
        })),
        ctx.main()
            .load_constant(C::ScalarExt::from(u64::from(KAGEMUSHA_RECURSION_IPA_K_V1))),
        ctx.main().load_constant(C::ScalarExt::from(
            u64::try_from(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1)
                .expect("fixed carrier length fits u64"),
        )),
        ctx.main().load_constant(C::ScalarExt::from(2)),
    ]);
    inputs.extend_from_slice(commitment_cells);
    let poseidon = KagemushaPoseidonChipV1::new(ctx.main(), range);
    let digest = poseidon.hash(ctx.main(), range, CLAIM_CARRIER_RLC_DOMAIN_V1, &inputs);
    let low_limb = chip.assigned_scalar_u128_limbs(&mut ctx, digest)[0];
    let (_, low_125) = range.div_mod(
        ctx.main(),
        Existing(low_limb),
        1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1,
        128,
    );
    let challenge = range
        .gate()
        .add(ctx.main(), Existing(low_125), Constant(C::ScalarExt::ONE));
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::CARRIER_RLC_EQ_CHALLENGE,
        KagemushaPastaParityV1::Ep => public_instance::CARRIER_RLC_EP_CHALLENGE,
    };
    ctx.main()
        .constrain_equal(&challenge, &public[expected_offset]);
    Ok(challenge)
}

/// Divide by `M = 2^127 - 1` with the narrow bounds proved by the carrier gadget.
///
/// The generic range-chip division proves two general strict inequalities. Here a 127-bit
/// remainder is already below `M` except for the single value `M`, which is rejected directly.
/// A 126-bit quotient used by a Horner step recomposes below `2^253`; a 127-bit quotient used by
/// modular exponentiation recomposes below `2^254`. Both are strictly below both Pasta field
/// moduli, so the field equality is the exact integer division relation without wrap.
#[cfg(test)]
fn constrain_claim_carrier_division_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    value: AssignedValue<F>,
    quotient_bits: usize,
    forbidden_quotient: Option<u128>,
) -> (AssignedValue<F>, AssignedValue<F>) {
    use halo2_base::utils::{biguint_to_fe, fe_to_biguint};

    assert!(quotient_bits <= 127);
    let modulus = der_parser::num_bigint::BigUint::from(CLAIM_CARRIER_RLC_MODULUS_V1);
    let integer = fe_to_biguint(value.value());
    let quotient = &integer / &modulus;
    let remainder = integer % &modulus;
    ctx.assign_region(
        [
            Witness(biguint_to_fe(&remainder)),
            Constant(F::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1)),
            Witness(biguint_to_fe(&quotient)),
            Existing(value),
        ],
        [0],
    );
    let remainder = ctx.get(-4);
    let quotient = ctx.get(-2);
    range.range_check(ctx, quotient, quotient_bits);
    if let Some(forbidden) = forbidden_quotient {
        let is_forbidden =
            range
                .gate()
                .is_equal(ctx, Existing(quotient), Constant(F::from_u128(forbidden)));
        range.gate().assert_is_const(ctx, &is_forbidden, &F::ZERO);
    }
    range.range_check(ctx, remainder, 127);
    let is_modulus = range.gate().is_equal(
        ctx,
        Existing(remainder),
        Constant(F::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1)),
    );
    range.gate().assert_is_const(ctx, &is_modulus, &F::ZERO);
    (quotient, remainder)
}

#[cfg(test)]
struct AssignedClaimCarrierCoefficientsV1<F: KagemushaPoseidonFieldV1> {
    active_remainders: Vec<AssignedValue<F>>,
    remainder_zero_tail: usize,
    active_quotient_packs: Vec<AssignedValue<F>>,
    quotient_pack_zero_tail: usize,
}

#[cfg(test)]
fn canonical_claim_carrier_coefficients_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    values: &[AssignedValue<F>],
) -> Result<AssignedClaimCarrierCoefficientsV1<F>, String> {
    if values.is_empty() || values.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 {
        return Err("mint-hash active carrier prefix has the wrong length".to_owned());
    }
    let mut active_remainders = Vec::with_capacity(values.len());
    let mut quotients = Vec::with_capacity(values.len());
    for value in values.iter().copied() {
        range.range_check(ctx, value, 128);
        let (quotient, remainder) =
            constrain_claim_carrier_division_v1(ctx, range, value, 2, Some(3));
        active_remainders.push(remainder);
        quotients.push(quotient);
    }
    let active_quotient_pack_count = values
        .len()
        .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
    let mut active_quotient_packs = Vec::with_capacity(active_quotient_pack_count);
    for chunk in quotients.chunks(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
        let mut power = 1_u128;
        let powers = (0..chunk.len())
            .map(|index| {
                let coefficient = Constant(F::from_u128(power));
                if index + 1 != chunk.len() {
                    power = power
                        .checked_mul(CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1)
                        .expect("fixed ternary quotient pack fits u128");
                }
                coefficient
            })
            .collect::<Vec<_>>();
        let packed = range
            .gate()
            .inner_product(ctx, chunk.iter().copied().map(Existing), powers);
        active_quotient_packs.push(packed);
    }
    let fixed_quotient_pack_count = KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
    Ok(AssignedClaimCarrierCoefficientsV1 {
        active_remainders,
        remainder_zero_tail: KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 - values.len(),
        active_quotient_packs,
        quotient_pack_zero_tail: fixed_quotient_pack_count - active_quotient_pack_count,
    })
}

#[cfg(test)]
fn constrain_claim_carrier_modular_product_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
) -> AssignedValue<F> {
    let product = range.gate().mul(ctx, Existing(left), Existing(right));
    constrain_claim_carrier_division_v1(ctx, range, product, 127, None).1
}

#[cfg(test)]
fn constrain_claim_carrier_modular_power_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    base: AssignedValue<F>,
    mut exponent: usize,
) -> AssignedValue<F> {
    debug_assert!(exponent > 0);
    let mut result: Option<AssignedValue<F>> = None;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = Some(match result {
                Some(result) => {
                    constrain_claim_carrier_modular_product_v1(ctx, range, result, factor)
                }
                None => factor,
            });
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = constrain_claim_carrier_modular_product_v1(ctx, range, factor, factor);
        }
    }
    result.expect("positive fixed exponent has at least one set bit")
}

#[cfg(test)]
fn advance_claim_carrier_zero_run_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    accumulator: AssignedValue<F>,
    challenge: AssignedValue<F>,
    zero_count: usize,
) -> AssignedValue<F> {
    if zero_count == 0 {
        return accumulator;
    }
    let power = constrain_claim_carrier_modular_power_v1(ctx, range, challenge, zero_count);
    constrain_claim_carrier_modular_product_v1(ctx, range, accumulator, power)
}

#[cfg(test)]
fn assigned_claim_carrier_rlc_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    coefficients: &AssignedClaimCarrierCoefficientsV1<F>,
    challenge: AssignedValue<F>,
) -> AssignedValue<F> {
    let step = |ctx: &mut halo2_base::Context<F>,
                accumulator: AssignedValue<F>,
                coefficient: AssignedValue<F>| {
        let numerator = range.gate().mul_add(
            ctx,
            Existing(accumulator),
            Existing(challenge),
            Existing(coefficient),
        );
        constrain_claim_carrier_division_v1(ctx, range, numerator, 126, None).1
    };
    let mut accumulator = ctx.load_zero();
    for coefficient in coefficients.active_remainders.iter().copied() {
        accumulator = step(ctx, accumulator, coefficient);
    }
    accumulator = advance_claim_carrier_zero_run_v1(
        ctx,
        range,
        accumulator,
        challenge,
        coefficients.remainder_zero_tail,
    );
    for coefficient in coefficients.active_quotient_packs.iter().copied() {
        accumulator = step(ctx, accumulator, coefficient);
    }
    advance_claim_carrier_zero_run_v1(
        ctx,
        range,
        accumulator,
        challenge,
        coefficients.quotient_pack_zero_tail,
    )
}

fn constrain_claim_carrier_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &[AssignedValue<F>],
    eq_carrier: &[AssignedValue<F>],
    ep_carrier: &[AssignedValue<F>],
) -> Result<KagemushaClaimCarrierRlcMachineV1<F>, String> {
    if eq_carrier.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || ep_carrier.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("mint-hash claim RLC carriers do not fill the fixed schedule".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let eq_challenge = public[public_instance::CARRIER_RLC_EQ_CHALLENGE];
    let ep_challenge = public[public_instance::CARRIER_RLC_EP_CHALLENGE];
    let upper_exclusive = ctx.load_constant(F::from_u128(
        (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1) + 1,
    ));
    for challenge in [eq_challenge, ep_challenge] {
        range.range_check(ctx, challenge, CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1 + 1);
        let is_zero = range.gate().is_zero(ctx, challenge);
        range.gate().assert_is_const(ctx, &is_zero, &F::ZERO);
        let within_canonical_range = range.is_less_than(
            ctx,
            challenge,
            upper_exclusive,
            CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1 + 1,
        );
        range
            .gate()
            .assert_is_const(ctx, &within_canonical_range, &F::ONE);
    }
    let machine = KagemushaClaimCarrierRlcMachineV1 {
        challenge_a: eq_challenge,
        challenge_b: ep_challenge,
        carriers: [
            ClaimRlcCarrierV1 {
                values: eq_carrier.to_vec(),
                expected_a: public[public_instance::EQ_CARRIER_AT_EQ_CHALLENGE],
                expected_b: public[public_instance::EQ_CARRIER_AT_EP_CHALLENGE],
            },
            ClaimRlcCarrierV1 {
                values: ep_carrier.to_vec(),
                expected_a: public[public_instance::EP_CARRIER_AT_EQ_CHALLENGE],
                expected_b: public[public_instance::EP_CARRIER_AT_EP_CHALLENGE],
            },
        ],
        use_unknown: false,
    };
    machine.required_rows()?;
    Ok(machine)
}

fn pad_assigned_claim_carriers_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    mut carriers: [Vec<AssignedValue<F>>; 2],
) -> Result<[Vec<AssignedValue<F>>; 2], String> {
    if carriers.iter().any(|carrier| {
        carrier.is_empty() || carrier.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    }) {
        return Err("mint-hash assigned carrier pair has the wrong shape".to_owned());
    }
    let padding_zero = builder.main(0).load_constant(F::ZERO);
    for carrier in &mut carriers {
        carrier.resize(
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            padding_zero,
        );
    }
    Ok(carriers)
}

fn validate_claim_pair_witness_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaMintHashClaimPairWitnessV1<'_>,
) -> Result<(), String> {
    validate_mint_hash_shard_basis_prefix_v1(eq_carrier_params, eq_shard_params)?;
    validate_mint_hash_shard_basis_prefix_v1(ep_carrier_params, ep_shard_params)?;
    witness.successor.validate()?;
    witness.metadata.validate()?;
    if witness.previous.is_some() != witness.previous_metadata.is_some() {
        return Err("mint hash predecessor state/metadata presence differs".to_owned());
    }
    if witness.previous.is_none() {
        let expected_eq = super::initial_kagemusha_eq_accumulator_v1(eq_carrier_params)
            .map_err(|error| format!("failed to derive Eq mint hash seed history: {error}"))?;
        let actual_eq = super::KagemushaEqAccumulatorV1::from_native(witness.eq.parent_history)
            .map_err(|error| format!("invalid Eq mint hash seed history: {error}"))?;
        let expected_ep = super::initial_kagemusha_ep_accumulator_v1(ep_carrier_params)
            .map_err(|error| format!("failed to derive Ep mint hash seed history: {error}"))?;
        let actual_ep = super::KagemushaEpAccumulatorV1::from_native(witness.ep.parent_history)
            .map_err(|error| format!("invalid Ep mint hash seed history: {error}"))?;
        if actual_eq != expected_eq || actual_ep != expected_ep {
            return Err("mint hash bootstrap history is not the canonical decided seed".to_owned());
        }
    }
    if let Some(previous_metadata) = witness.previous_metadata {
        previous_metadata.validate()?;
        if previous_metadata.eq_claim_protocol != witness.metadata.eq_claim_protocol
            || previous_metadata.ep_claim_protocol != witness.metadata.ep_claim_protocol
            || previous_metadata.eq_shard_protocol != witness.metadata.eq_shard_protocol
            || previous_metadata.ep_shard_protocol != witness.metadata.ep_shard_protocol
        {
            return Err("mint hash predecessor uses another recursive verifier suite".to_owned());
        }
    }
    validate_paired_leaf_v1(&witness.eq_leaf, &witness.ep_leaf)?;
    let expected_eq = KagemushaMintHashClaimStateV1::apply::<Fp>(
        witness.successor.eq.plan,
        witness.previous.map(|state| state.eq),
        &witness.eq_leaf,
    )?;
    let expected_ep = KagemushaMintHashClaimStateV1::apply::<Fq>(
        witness.successor.ep.plan,
        witness.previous.map(|state| state.ep),
        &witness.ep_leaf,
    )?;
    if witness.successor.eq != expected_eq || witness.successor.ep != expected_ep {
        return Err("mint hash successor is not the exact paired leaf transition".to_owned());
    }
    Ok(())
}

/// Build one mutually audited recursive claim step.
///
/// The returned circuits are not independently authoritative. Their ordinary proof openings and
/// the returned carried histories must still be terminally decided by the mint-authority caller.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_kagemusha_mint_hash_claim_pair_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintHashClaimEqCircuitV1,
        KagemushaMintHashClaimEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    let audits = derive_kagemusha_mint_hash_claim_deferred_audits_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        witness.clone(),
    )?;
    let (eq, _) = build_kagemusha_mint_hash_claim_eq_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        witness.clone(),
        &audits,
    )?;
    let (ep, _) = build_kagemusha_mint_hash_claim_ep_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        witness,
        &audits,
    )?;
    Ok((eq, ep, audits.eq_digest, audits.ep_digest))
}

/// Derive both native deferred-audit witnesses without retaining either scalar circuit graph.
///
/// This is the discovery pass used when the audit digests are not known until the verifier
/// equations have been emitted. Eq is completely built, compacted, and dropped before Ep starts.
/// Callers then bind the returned digests into metadata and use the parity-specific builders below
/// for key generation and proving.
#[allow(clippy::too_many_lines)]
pub(crate) fn derive_kagemusha_mint_hash_claim_deferred_audits_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
) -> Result<KagemushaMintHashClaimDeferredAuditsV1, String> {
    validate_claim_pair_witness_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        &witness,
    )?;
    let eq_carrier_svk = super::composite::eq_succinct_vk(eq_carrier_params);
    let ep_carrier_svk = super::composite::ep_succinct_vk(ep_carrier_params);
    let eq_shard_svk = super::composite::eq_succinct_vk(eq_shard_params);
    let ep_shard_svk = super::composite::ep_succinct_vk(ep_shard_params);
    let KagemushaMintHashClaimPairWitnessV1 {
        previous,
        previous_metadata,
        successor,
        metadata,
        eq_leaf,
        ep_leaf,
        eq,
        ep,
    } = witness;

    let ClaimScalarHalfV1 {
        builder: eq_builder,
        output: eq_output,
        common_cells: _,
    } = build_claim_scalar_half_v1::<EqAffine>(
        &eq_carrier_svk,
        &eq_shard_svk,
        KagemushaPastaParityV1::Eq,
        previous.map(|state| state.eq),
        previous_metadata,
        &successor,
        metadata,
        &eq_leaf,
        eq,
        None,
    )?;
    let eq_digest = assigned_digest_bytes_v1(&eq_output.challenge_limbs)?;
    drop(eq_builder);
    halo2_proofs::release_allocator_slack();

    let ClaimScalarHalfV1 {
        builder: ep_builder,
        output: ep_output,
        common_cells: _,
    } = build_claim_scalar_half_v1::<EpAffine>(
        &ep_carrier_svk,
        &ep_shard_svk,
        KagemushaPastaParityV1::Ep,
        previous.map(|state| state.ep),
        previous_metadata,
        &successor,
        metadata,
        &ep_leaf,
        ep,
        None,
    )?;
    let ep_digest = assigned_digest_bytes_v1(&ep_output.challenge_limbs)?;
    drop(ep_builder);
    halo2_proofs::release_allocator_slack();

    if eq_output.bound_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
        || ep_output.bound_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
        || eq_output.batch.source_count() != eq_output.source_commitments.len()
        || ep_output.batch.source_count() != ep_output.source_commitments.len()
    {
        return Err("mint-hash claim compact source/bound inventory drifted".to_owned());
    }
    let eq_carrier = padded_claim_carrier_u128_values_v1(&eq_output)?;
    let ep_carrier = padded_claim_carrier_u128_values_v1(&ep_output)?;
    let carrier_binding = derive_claim_carrier_binding_v1(
        eq_carrier_params,
        ep_carrier_params,
        &eq_carrier,
        &ep_carrier,
    )?;
    Ok(KagemushaMintHashClaimDeferredAuditsV1 {
        eq: eq_output,
        ep: ep_output,
        eq_digest,
        ep_digest,
        eq_carrier,
        ep_carrier,
        carrier_binding,
    })
}

/// Build the exact Eq claim circuit from compact, independently derived reciprocal audits.
///
/// The returned public values are recomputed from the same witness used by the scalar graph so a
/// release-construction caller can compare them across key, stability, and proving passes.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_kagemusha_mint_hash_claim_eq_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
    audits: &KagemushaMintHashClaimDeferredAuditsV1,
) -> Result<(KagemushaMintHashClaimEqCircuitV1, Vec<Vec<Fp>>), String> {
    validate_claim_pair_witness_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        &witness,
    )?;
    if witness.metadata.eq_deferred_audit != audits.eq_digest
        || witness.metadata.ep_deferred_audit != audits.ep_digest
    {
        return Err("mint hash claim metadata does not bind the derived audit pair".to_owned());
    }
    let mut semantic_instances = claim_public_values_v1::<Fp>(
        KagemushaPastaParityV1::Eq,
        &witness.successor,
        witness.metadata,
        witness.eq.successor_history,
    )?;
    append_inner_carrier_binding_v1(&mut semantic_instances, audits.carrier_binding)?;
    let eq_carrier_svk = super::composite::eq_succinct_vk(eq_carrier_params);
    let eq_shard_svk = super::composite::eq_succinct_vk(eq_shard_params);
    let ClaimScalarHalfV1 {
        builder: mut eq_builder,
        output: eq_output,
        common_cells: eq_common,
    } = build_claim_scalar_half_v1::<EqAffine>(
        &eq_carrier_svk,
        &eq_shard_svk,
        KagemushaPastaParityV1::Eq,
        witness.previous.map(|state| state.eq),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.eq_leaf,
        witness.eq,
        Some(audits.carrier_binding),
    )?;
    bind_own_audit_v1(&mut eq_builder, public_instance::EQ_AUDIT_LO, &eq_output)?;
    let expected_ep = public_digest_cells_v1(
        &eq_builder,
        public_instance::EP_AUDIT_LO,
        "Eq claim Ep audit",
    )?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    let ep_carrier = constrain_reciprocal_native_batch_v1::<EpAffine>(
        &mut eq_builder,
        &audits.ep,
        &expected_ep,
        &eq_common,
        &mut dense_jobs,
        KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1,
    )?;
    let eq_carrier = eq_output
        .carrier_cells_v1()
        .map_err(|error| format!("Eq mint-hash carrier shape is invalid: {error:?}"))?;
    validate_claim_carrier_active_len_v1(&eq_output, eq_carrier.len())?;
    validate_claim_carrier_active_len_v1(&audits.ep, ep_carrier.len())?;
    let assigned_semantic = eq_builder
        .assigned_instances
        .first()
        .cloned()
        .ok_or_else(|| "Eq mint-hash semantic instance column is absent".to_owned())?;
    let [eq_carrier, ep_carrier] =
        pad_assigned_claim_carriers_v1(&mut eq_builder, [eq_carrier, ep_carrier])?;
    let carrier_rlc = constrain_claim_carrier_binding_v1(
        &mut eq_builder,
        &assigned_semantic,
        &eq_carrier,
        &ep_carrier,
    )?;
    eq_builder.assigned_instances.push(eq_carrier);
    eq_builder.assigned_instances.push(ep_carrier);
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    dense_jobs.validate_capacity_with_lanes(
        (1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS,
        KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1,
    )?;
    carrier_rlc
        .validate_capacity((1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS)?;
    if assigned_digest_bytes_v1(&eq_output.challenge_limbs)? != audits.eq_digest
        || padded_claim_carrier_u128_values_v1(&eq_output)? != audits.eq_carrier
    {
        return Err("Eq mint-hash deferred audit changed after exact public rebinding".to_owned());
    }
    let public_instances = claim_hybrid_instances_v1(
        &semantic_instances[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1],
        &audits.eq_carrier,
        &audits.ep_carrier,
        audits.carrier_binding,
    )?;
    Ok((
        KagemushaMintHashClaimEqCircuitV1 {
            builder: eq_builder,
            carrier_rlc,
            dense_jobs,
        },
        public_instances,
    ))
}

/// Build the exact Ep claim circuit from compact, independently derived reciprocal audits.
///
/// No Eq circuit graph is constructed or retained by this operation.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_kagemusha_mint_hash_claim_ep_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
    audits: &KagemushaMintHashClaimDeferredAuditsV1,
) -> Result<(KagemushaMintHashClaimEpCircuitV1, Vec<Vec<Fq>>), String> {
    validate_claim_pair_witness_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        &witness,
    )?;
    if witness.metadata.eq_deferred_audit != audits.eq_digest
        || witness.metadata.ep_deferred_audit != audits.ep_digest
    {
        return Err("mint hash claim metadata does not bind the derived audit pair".to_owned());
    }
    let mut semantic_instances = claim_public_values_v1::<Fq>(
        KagemushaPastaParityV1::Ep,
        &witness.successor,
        witness.metadata,
        witness.ep.successor_history,
    )?;
    append_inner_carrier_binding_v1(&mut semantic_instances, audits.carrier_binding)?;
    let ep_carrier_svk = super::composite::ep_succinct_vk(ep_carrier_params);
    let ep_shard_svk = super::composite::ep_succinct_vk(ep_shard_params);
    let ClaimScalarHalfV1 {
        builder: mut ep_builder,
        output: ep_output,
        common_cells: ep_common,
    } = build_claim_scalar_half_v1::<EpAffine>(
        &ep_carrier_svk,
        &ep_shard_svk,
        KagemushaPastaParityV1::Ep,
        witness.previous.map(|state| state.ep),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.ep_leaf,
        witness.ep,
        Some(audits.carrier_binding),
    )?;
    bind_own_audit_v1(&mut ep_builder, public_instance::EP_AUDIT_LO, &ep_output)?;
    let expected_eq = public_digest_cells_v1(
        &ep_builder,
        public_instance::EQ_AUDIT_LO,
        "Ep claim Eq audit",
    )?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    let eq_carrier = constrain_reciprocal_native_batch_v1::<EqAffine>(
        &mut ep_builder,
        &audits.eq,
        &expected_eq,
        &ep_common,
        &mut dense_jobs,
        KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1,
    )?;
    let ep_carrier = ep_output
        .carrier_cells_v1()
        .map_err(|error| format!("Ep mint-hash carrier shape is invalid: {error:?}"))?;
    validate_claim_carrier_active_len_v1(&audits.eq, eq_carrier.len())?;
    validate_claim_carrier_active_len_v1(&ep_output, ep_carrier.len())?;
    let assigned_semantic = ep_builder
        .assigned_instances
        .first()
        .cloned()
        .ok_or_else(|| "Ep mint-hash semantic instance column is absent".to_owned())?;
    let [eq_carrier, ep_carrier] =
        pad_assigned_claim_carriers_v1(&mut ep_builder, [eq_carrier, ep_carrier])?;
    let carrier_rlc = constrain_claim_carrier_binding_v1(
        &mut ep_builder,
        &assigned_semantic,
        &eq_carrier,
        &ep_carrier,
    )?;
    ep_builder.assigned_instances.push(eq_carrier);
    ep_builder.assigned_instances.push(ep_carrier);
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    dense_jobs.validate_capacity_with_lanes(
        (1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS,
        KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1,
    )?;
    carrier_rlc
        .validate_capacity((1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS)?;
    if assigned_digest_bytes_v1(&ep_output.challenge_limbs)? != audits.ep_digest
        || padded_claim_carrier_u128_values_v1(&ep_output)? != audits.ep_carrier
    {
        return Err("Ep mint-hash deferred audit changed after exact public rebinding".to_owned());
    }
    let public_instances = claim_hybrid_instances_v1(
        &semantic_instances[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1],
        &audits.eq_carrier,
        &audits.ep_carrier,
        audits.carrier_binding,
    )?;
    Ok((
        KagemushaMintHashClaimEpCircuitV1 {
            builder: ep_builder,
            carrier_rlc,
            dense_jobs,
        },
        public_instances,
    ))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_claim_scalar_half_v1<C>(
    carrier_svk: &IpaSuccinctVerifyingKey<C>,
    shard_svk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    previous: Option<KagemushaMintHashClaimStateV1>,
    previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    successor: &KagemushaMintHashClaimPairStateV1,
    metadata: KagemushaMintHashClaimMetadataV1,
    leaf: &KagemushaMintHashShardStatementV1,
    witness: KagemushaMintHashClaimParityWitnessV1<'_, C>,
    carrier_binding: Option<ClaimCarrierBindingV1>,
) -> Result<ClaimScalarHalfV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
{
    macro_rules! profile_cells {
        ($label:literal, $builder:expr) => {
            if std::env::var_os("IROHA_KAGEMUSHA_PROFILE_CELLS").is_some() {
                let statistics = $builder.statistics();
                eprintln!(
                    "KAGEMUSHA_CELLS parity={parity:?} stage={} gate={:?} lookup={:?}",
                    $label,
                    statistics.gate.total_advice_per_phase,
                    statistics.total_lookup_advice_per_phase,
                );
            }
        };
    }
    macro_rules! profile_loader_cells {
        ($label:literal, $loader:expr, $builder:expr) => {
            if std::env::var_os("IROHA_KAGEMUSHA_PROFILE_CELLS").is_some() {
                let gate = $loader.ctx_mut().total_advice();
                let lookup = $builder.statistics().total_lookup_advice_per_phase;
                eprintln!(
                    "KAGEMUSHA_CELLS parity={parity:?} stage={} gate=[{gate}] lookup={lookup:?}",
                    $label,
                );
            }
        };
    }
    if witness.parent_protocol.num_instance.len() != 3
        || witness.parent_protocol.num_instance[0]
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.parent_protocol.num_instance[1]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.parent_protocol.num_instance[2]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.parent_instances.len() != 3
        || witness.parent_instances[0].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.parent_instances[1].len() != witness.parent_protocol.num_instance[1]
        || witness.parent_instances[2].len() != witness.parent_protocol.num_instance[2]
        || witness.shard_protocol.num_instance
            != [KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]
    {
        return Err("mint hash claim parent or shard public ABI is not fixed".to_owned());
    }
    let state = match parity {
        KagemushaPastaParityV1::Eq => successor.eq,
        KagemushaPastaParityV1::Ep => successor.ep,
    };
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits((KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
        .use_instance_columns(3);
    let mut public_values = claim_public_values_v1::<C::ScalarExt>(
        parity,
        successor,
        metadata,
        witness.successor_history,
    )?;
    append_inner_carrier_binding_v1(
        &mut public_values,
        carrier_binding.unwrap_or_else(placeholder_claim_carrier_binding_v1),
    )?;
    let public = public_values
        .into_iter()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    if public.len() != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim public instance shape drifted".to_owned());
    }
    range_check_claim_public_v1(
        &mut builder,
        &public[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1],
    )?;
    let range = builder.range_chip();
    for value in &public[KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1..] {
        range.range_check(builder.main(0), *value, 128);
    }
    builder.assigned_instances = vec![public.clone()];
    profile_cells!("public", builder);

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    if carrier_binding.is_some() {
        constrain_claim_carrier_challenge_v1(&loader, &public, parity)?;
    }
    let parent_enabled = {
        let mut ctx = loader.ctx_mut();
        let first = loader.ecc_chip().range().gate().is_equal(
            ctx.main(),
            public[public_instance::NEXT_STAGE],
            Constant(C::ScalarExt::ONE),
        );
        loader
            .ecc_chip()
            .range()
            .gate()
            .not(ctx.main(), Existing(first))
    };

    let expected_claim_protocol = public_digest_cells_from_slice_v1(
        &public,
        match parity {
            KagemushaPastaParityV1::Eq => public_instance::EQ_CLAIM_PROTOCOL_LO,
            KagemushaPastaParityV1::Ep => public_instance::EP_CLAIM_PROTOCOL_LO,
        },
        "claim protocol",
    )?;
    let claim_structure = kagemusha_protocol_structure_digest_v1(witness.parent_protocol, parity)?;
    let loaded_parent = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.parent_protocol,
        parity,
        claim_structure,
        &expected_claim_protocol,
    )
    .map_err(|error| format!("failed to bind mint hash claim protocol: {error:?}"))?;
    profile_loader_cells!("parent_protocol", loader, builder);
    let parent_semantic = witness.parent_instances[0]
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    let parent_assigned = verify_two_carrier_hybrid_ordinary_proof_and_stream_v1(
        &loader,
        carrier_svk,
        &loaded_parent.protocol,
        &parent_semantic,
        match parity {
            KagemushaPastaParityV1::Eq => [
                [
                    public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    public_instance::EQ_PROOF_EP_CARRIER_COMMITMENT_LO,
                    public_instance::EQ_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
            KagemushaPastaParityV1::Ep => [
                [
                    public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    public_instance::EP_PROOF_EP_CARRIER_COMMITMENT_LO,
                    public_instance::EP_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
        },
        witness.parent_proof,
    )
    .map_err(|error| format!("failed to verify mint hash claim predecessor: {error:?}"))?;
    profile_loader_cells!("parent_proof", loader, builder);
    let parent_accumulator = parent_assigned.accumulator;
    let parent_transcript_binding = parent_assigned.transcript_binding;
    drop(parent_assigned.loaded_stream);
    let parent_column = &parent_semantic[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1];
    let parent_history = load_native_accumulator(&loader, witness.parent_history)
        .map_err(|error| format!("failed to load mint hash claim history: {error:?}"))?;
    let parent_history_cells = parent_column
        .get(public_instance::HISTORY_START..)
        .ok_or_else(|| "mint hash claim predecessor history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &parent_history, &parent_history_cells)
        .map_err(|error| format!("failed to bind mint hash predecessor history: {error:?}"))?;
    let parent_fold = verify_fold_with_transcript_binding_v1(
        &loader,
        carrier_svk,
        &[parent_accumulator, parent_history.clone()],
        witness.parent_fold_proof,
    )
    .map_err(|error| format!("failed to fold mint hash claim predecessor: {error:?}"))?;
    profile_loader_cells!("parent_fold", loader, builder);
    let prior_history = select_accumulator_v1(
        &loader,
        &parent_fold.accumulator,
        &parent_history,
        parent_enabled,
    )
    .map_err(|error| format!("failed to select mint hash predecessor history: {error:?}"))?;
    let parent_equations = loader.ecc_chip().equation_count();

    let expected_shard_protocol = public_digest_cells_from_slice_v1(
        &public,
        match parity {
            KagemushaPastaParityV1::Eq => public_instance::EQ_SHARD_PROTOCOL_LO,
            KagemushaPastaParityV1::Ep => public_instance::EP_SHARD_PROTOCOL_LO,
        },
        "shard protocol",
    )?;
    let shard_structure = kagemusha_protocol_structure_digest_v1(witness.shard_protocol, parity)?;
    let loaded_shard = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.shard_protocol,
        parity,
        shard_structure,
        &expected_shard_protocol,
    )
    .map_err(|error| format!("failed to bind mint hash shard protocol: {error:?}"))?;
    profile_loader_cells!("shard_protocol", loader, builder);
    let shard_values = shard_public_values_v1::<C::ScalarExt>(leaf)?;
    let shard_instances = vec![
        shard_values
            .iter()
            .copied()
            .map(|value| loader.assign_scalar(value))
            .collect::<Vec<_>>(),
    ];
    let (shard_accumulator, shard_transcript_binding) =
        verify_ordinary_proof_with_transcript_binding_at_k_v1(
            &loader,
            shard_svk,
            &loaded_shard.protocol,
            &shard_instances,
            witness.shard_proof,
            KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize,
        )
        .map_err(|error| format!("failed to verify mint hash shard proof: {error:?}"))?;
    profile_loader_cells!("shard_proof", loader, builder);
    bind_shard_instances_v1(&loader, &public, &shard_instances[0], parity)?;
    constrain_claim_parent_and_leaf_cursor_v1(
        &loader,
        &public,
        parent_column,
        &shard_instances[0],
        parent_enabled,
        previous,
        state,
    )?;
    profile_loader_cells!("cursor", loader, builder);
    let lifted = lift_mint_hash_shard_accumulator_v1(&loader, shard_accumulator)?;
    let successor_history = verify_fold_with_transcript_binding_v1(
        &loader,
        carrier_svk,
        &[lifted, prior_history],
        witness.leaf_fold_proof,
    )
    .map_err(|error| format!("failed to fold lifted mint hash shard: {error:?}"))?;
    profile_loader_cells!("leaf_fold", loader, builder);
    bind_accumulator_limbs(
        &loader,
        &successor_history.accumulator,
        public
            .get(public_instance::HISTORY_START..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1)
            .ok_or_else(|| "mint hash successor history is absent".to_owned())?,
    )
    .map_err(|error| format!("failed to bind mint hash successor history: {error:?}"))?;

    constrain_proof_chain_root_v1(
        &loader,
        &public,
        parent_column,
        parity,
        parent_enabled,
        previous_metadata,
        metadata,
        parent_transcript_binding,
        shard_transcript_binding,
    )?;
    profile_loader_cells!("proof_chain", loader, builder);
    let equation_count = loader.ecc_chip().equation_count();
    if parent_equations == 0 || equation_count <= parent_equations {
        return Err("mint hash claim verifier emitted an incomplete equation audit".to_owned());
    }
    let common_cells = common_public_cells_v1(&public);
    let mut tags = vec![CLAIM_PARENT_EQUATION_TAG_V1; parent_equations];
    tags.resize(equation_count, CLAIM_SHARD_EQUATION_TAG_V1);
    let mut assigned_selectors = vec![parent_enabled; parent_equations];
    assigned_selectors.extend(
        (parent_equations..equation_count)
            .map(|_| loader.ctx_mut().main().load_constant(C::ScalarExt::ONE)),
    );
    let verifier_input_binding = mint_hash_claim_batch_input_binding_v1(
        &[
            parent_transcript_binding,
            parent_fold.transcript_binding,
            shard_transcript_binding,
            successor_history.transcript_binding,
        ],
        parent_enabled,
    )?;
    let output = derive_mint_hash_claim_native_deferred_batch_v1(
        &mut builder,
        loader,
        tags,
        assigned_selectors,
        &verifier_input_binding,
        &common_cells,
    )
    .map_err(|error| format!("failed to finalize mint hash claim audit: {error:?}"))?;
    profile_cells!("deferred_batch", builder);
    let carrier = output
        .carrier_cells_v1()
        .map_err(|error| format!("failed to build mint hash claim carrier: {error:?}"))?;
    if output.batch.source_count() > KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1
        || carrier.len() > KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(format!(
            "mint-hash claim compact carrier exceeds capacity: S={}, L={}, maximum S={}/L={}",
            output.batch.source_count(),
            carrier.len(),
            KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ));
    }
    Ok(ClaimScalarHalfV1 {
        builder,
        output,
        common_cells,
    })
}

fn validate_paired_leaf_v1(
    eq: &KagemushaMintHashShardStatementV1,
    ep: &KagemushaMintHashShardStatementV1,
) -> Result<(), String> {
    if eq.parity != KagemushaPastaParityV1::Eq
        || ep.parity != KagemushaPastaParityV1::Ep
        || eq.release_id != ep.release_id
        || eq.stage_index != ep.stage_index
        || eq.job_index != ep.job_index
        || eq.block_index != ep.block_index
        || eq.job_block_count != ep.job_block_count
    {
        return Err("mint hash shard pair does not share one fixed job/block position".to_owned());
    }
    Ok(())
}

pub(crate) fn claim_public_values_v1<F: KagemushaPoseidonFieldV1>(
    parity: KagemushaPastaParityV1,
    state: &KagemushaMintHashClaimPairStateV1,
    metadata: KagemushaMintHashClaimMetadataV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    state.validate()?;
    metadata.validate()?;
    let mut values = Vec::with_capacity(KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1);
    values.extend([
        F::ONE,
        F::from(match parity {
            KagemushaPastaParityV1::Eq => 0,
            KagemushaPastaParityV1::Ep => 1,
        }),
        F::from(u64::from(state.eq.complete)),
    ]);
    values.extend(digest_limbs::<F>(state.eq.plan.release_id));
    values.extend(digest_limbs::<F>(state.eq.plan.plan_binding));
    values.extend(digest_limbs::<F>(state.ep.plan.plan_binding));
    values.extend([
        F::from(state.eq.plan.total_stages),
        F::from(u64::from(state.eq.plan.total_jobs)),
        F::from(state.eq.next_stage),
        F::from(u64::from(state.eq.next_job)),
        F::from(u64::from(state.eq.next_block)),
        F::from(u64::from(state.eq.active_job_blocks)),
    ]);
    values.extend(state.eq.chaining_state.map(|word| F::from(u64::from(word))));
    values.extend(state.ep.chaining_state.map(|word| F::from(u64::from(word))));
    for digest in [
        state.eq.message_root,
        state.ep.message_root,
        state.eq.terminal_root,
        state.ep.terminal_root,
        state.eq.plan.expected_message_root,
        state.ep.plan.expected_message_root,
        state.eq.plan.expected_terminal_root,
        state.ep.plan.expected_terminal_root,
        metadata.eq_claim_protocol,
        metadata.ep_claim_protocol,
        metadata.eq_shard_protocol,
        metadata.ep_shard_protocol,
        metadata.eq_deferred_audit,
        metadata.ep_deferred_audit,
        metadata.eq_proof_chain_root,
        metadata.ep_proof_chain_root,
    ] {
        values.extend(digest_limbs::<F>(digest));
    }
    let history_limbs = history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history limb has sixteen bytes"),
        ))
    });
    values.extend(history_limbs);
    if values.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim public value count drifted".to_owned());
    }
    Ok(values)
}

fn shard_public_values_v1<F: KagemushaPoseidonFieldV1>(
    leaf: &KagemushaMintHashShardStatementV1,
) -> Result<Vec<F>, String> {
    let expected_parity = if F::IS_EQ_PARITY {
        KagemushaPastaParityV1::Eq
    } else {
        KagemushaPastaParityV1::Ep
    };
    if leaf.parity != expected_parity
        || leaf.release_id == [0; 32]
        || leaf.plan_binding == [0; 32]
        || leaf.job_block_count == 0
        || leaf.block_index >= leaf.job_block_count
        || (leaf.block_index == 0 && leaf.initial_state != IV)
    {
        return Err("mint hash shard public statement shape is invalid".to_owned());
    }
    let mut values = Vec::with_capacity(KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1);
    values.extend([
        F::ONE,
        F::from(match leaf.parity {
            KagemushaPastaParityV1::Eq => 0,
            KagemushaPastaParityV1::Ep => 1,
        }),
    ]);
    values.extend(digest_limbs::<F>(leaf.release_id));
    values.extend(digest_limbs::<F>(leaf.plan_binding));
    values.extend([
        F::from(leaf.stage_index),
        F::from(u64::from(leaf.job_index)),
        F::from(u64::from(leaf.block_index)),
        F::from(u64::from(leaf.job_block_count)),
    ]);
    values.extend(leaf.initial_state.map(|word| F::from(u64::from(word))));
    values.extend(leaf.block_words.map(|word| F::from(u64::from(word))));
    values.extend(leaf.output_state.map(|word| F::from(u64::from(word))));
    values.push(F::from(u64::from(
        leaf.block_index + 1 == leaf.job_block_count,
    )));
    if values.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash shard public value count drifted".to_owned());
    }
    Ok(values)
}

fn range_check_claim_public_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &[AssignedValue<F>],
) -> Result<(), String> {
    if public.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim range-check shape mismatch".to_owned());
    }
    let range = builder.range_chip();
    for index in [
        public_instance::VERSION,
        public_instance::PARITY,
        public_instance::COMPLETE,
    ] {
        range.range_check(builder.main(0), public[index], 1);
    }
    for index in public_instance::RELEASE_LO..public_instance::TOTAL_STAGES {
        range.range_check(builder.main(0), public[index], 128);
    }
    for (index, bits) in [
        (public_instance::TOTAL_STAGES, 64),
        (public_instance::TOTAL_JOBS, 32),
        (public_instance::NEXT_STAGE, 64),
        (public_instance::NEXT_JOB, 32),
        (public_instance::NEXT_BLOCK, 32),
        (public_instance::ACTIVE_JOB_BLOCKS, 32),
    ] {
        range.range_check(builder.main(0), public[index], bits);
    }
    for index in public_instance::EQ_CHAINING_STATE..public_instance::EQ_MESSAGE_ROOT_LO {
        range.range_check(builder.main(0), public[index], 32);
    }
    for index in
        public_instance::EQ_MESSAGE_ROOT_LO..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
    {
        range.range_check(builder.main(0), public[index], 128);
    }
    Ok(())
}

fn public_digest_cells_from_slice_v1<F: halo2_base::utils::ScalarField>(
    public: &[AssignedValue<F>],
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    public
        .get(offset..offset + 2)
        .ok_or_else(|| format!("mint hash {label} digest is absent"))?
        .try_into()
        .map_err(|_| format!("mint hash {label} digest shape drifted"))
}

fn public_digest_cells_v1<F: halo2_base::utils::ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    let public = builder
        .assigned_instances
        .first()
        .ok_or_else(|| "mint hash claim public column is absent".to_owned())?;
    public_digest_cells_from_slice_v1(public, offset, label)
}

fn bind_own_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    offset: usize,
    output: &KagemushaNativeDeferredBatchV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let expected = public_digest_cells_v1(builder, offset, "own audit")?;
    for (actual, expected) in output.challenge_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok(())
}

fn assigned_digest_bytes_v1<F: halo2_base::utils::ScalarField>(
    limbs: &[AssignedValue<F>; 2],
) -> Result<DigestV1, String> {
    use halo2_base::utils::fe_to_biguint;

    let mut digest = [0_u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = fe_to_biguint(limb.value()).to_bytes_le();
        if bytes.len() > 16 {
            return Err("mint hash claim audit limb exceeds u128".to_owned());
        }
        digest[index * 16..index * 16 + bytes.len()].copy_from_slice(&bytes);
    }
    if digest == [0; 32] {
        return Err("mint hash claim audit digest is zero".to_owned());
    }
    Ok(digest)
}

/// Fixed-order MintHashClaim transcript binding used before deferred batching.
///
/// The ordinary transcript squeezes bind their protocol transcript initial
/// states, instance commitments, and proof objects. The common public slice
/// separately includes both protocol identities and both canonical proof-chain
/// roots and is absorbed through the batch helper's `bound_values`. That slice
/// intentionally excludes the current audit limbs and current carrier
/// commitments, which are outputs of this batch and would otherwise create a
/// Fiat-Shamir fixed point.
fn mint_hash_claim_batch_input_binding_v1<T: Copy>(
    transcript_bindings: &[T],
    parent_enabled: T,
) -> Result<Vec<T>, String> {
    if transcript_bindings.len() != CLAIM_BATCH_TRANSCRIPT_BINDING_COUNT_V1 {
        return Err("mint hash claim compact batch input shape drifted".to_owned());
    }
    let mut binding = Vec::with_capacity(transcript_bindings.len() + 1);
    binding.extend_from_slice(transcript_bindings);
    binding.push(parent_enabled);
    Ok(binding)
}

fn common_public_cells_v1<F: halo2_base::utils::ScalarField>(
    public: &[AssignedValue<F>],
) -> Vec<AssignedValue<F>> {
    (0..public_instance::HISTORY_START)
        .filter(|index| {
            *index != public_instance::PARITY
                && !(*index >= public_instance::EQ_AUDIT_LO
                    && *index < public_instance::EQ_PROOF_CHAIN_LO)
        })
        .map(|index| public[index])
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn constrain_claim_parent_and_leaf_cursor_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parent: &[DeferredScalar<'chip, C>],
    shard: &[DeferredScalar<'chip, C>],
    parent_enabled: AssignedValue<C::ScalarExt>,
    previous: Option<KagemushaMintHashClaimStateV1>,
    state: KagemushaMintHashClaimStateV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if parent.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
        || shard.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("mint hash claim predecessor public column is truncated".to_owned());
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let gate = range.gate();
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    let leaf_stage = *shard[shard_public::STAGE].assigned();
    let leaf_job = *shard[shard_public::JOB].assigned();
    let leaf_block = *shard[shard_public::BLOCK].assigned();
    let leaf_blocks = *shard[shard_public::JOB_BLOCKS].assigned();
    let chaining_state_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_CHAINING_STATE,
        KagemushaPastaParityV1::Ep => public_instance::EP_CHAINING_STATE,
    };

    let stage_in_range =
        range.is_less_than(ctx, leaf_stage, public[public_instance::TOTAL_STAGES], 64);
    gate.assert_is_const(ctx, &stage_in_range, &C::ScalarExt::ONE);
    let job_in_range = range.is_less_than(ctx, leaf_job, public[public_instance::TOTAL_JOBS], 32);
    gate.assert_is_const(ctx, &job_in_range, &C::ScalarExt::ONE);

    for (parent_offset, expected) in [
        (public_instance::VERSION, public[public_instance::VERSION]),
        (public_instance::PARITY, public[public_instance::PARITY]),
        (
            public_instance::RELEASE_LO,
            public[public_instance::RELEASE_LO],
        ),
        (
            public_instance::RELEASE_LO + 1,
            public[public_instance::RELEASE_LO + 1],
        ),
        (
            public_instance::EQ_PLAN_LO,
            public[public_instance::EQ_PLAN_LO],
        ),
        (
            public_instance::EQ_PLAN_LO + 1,
            public[public_instance::EQ_PLAN_LO + 1],
        ),
        (
            public_instance::EP_PLAN_LO,
            public[public_instance::EP_PLAN_LO],
        ),
        (
            public_instance::EP_PLAN_LO + 1,
            public[public_instance::EP_PLAN_LO + 1],
        ),
        (
            public_instance::TOTAL_STAGES,
            public[public_instance::TOTAL_STAGES],
        ),
        (
            public_instance::TOTAL_JOBS,
            public[public_instance::TOTAL_JOBS],
        ),
        (
            public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO,
            public[public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO],
        ),
        (
            public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO + 1,
            public[public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO + 1],
        ),
        (
            public_instance::EP_EXPECTED_MESSAGE_ROOT_LO,
            public[public_instance::EP_EXPECTED_MESSAGE_ROOT_LO],
        ),
        (
            public_instance::EP_EXPECTED_MESSAGE_ROOT_LO + 1,
            public[public_instance::EP_EXPECTED_MESSAGE_ROOT_LO + 1],
        ),
        (
            public_instance::EQ_EXPECTED_ROOT_LO,
            public[public_instance::EQ_EXPECTED_ROOT_LO],
        ),
        (
            public_instance::EQ_EXPECTED_ROOT_LO + 1,
            public[public_instance::EQ_EXPECTED_ROOT_LO + 1],
        ),
        (
            public_instance::EP_EXPECTED_ROOT_LO,
            public[public_instance::EP_EXPECTED_ROOT_LO],
        ),
        (
            public_instance::EP_EXPECTED_ROOT_LO + 1,
            public[public_instance::EP_EXPECTED_ROOT_LO + 1],
        ),
        (
            public_instance::EQ_CLAIM_PROTOCOL_LO,
            public[public_instance::EQ_CLAIM_PROTOCOL_LO],
        ),
        (
            public_instance::EQ_CLAIM_PROTOCOL_LO + 1,
            public[public_instance::EQ_CLAIM_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EP_CLAIM_PROTOCOL_LO,
            public[public_instance::EP_CLAIM_PROTOCOL_LO],
        ),
        (
            public_instance::EP_CLAIM_PROTOCOL_LO + 1,
            public[public_instance::EP_CLAIM_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EQ_SHARD_PROTOCOL_LO,
            public[public_instance::EQ_SHARD_PROTOCOL_LO],
        ),
        (
            public_instance::EQ_SHARD_PROTOCOL_LO + 1,
            public[public_instance::EQ_SHARD_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EP_SHARD_PROTOCOL_LO,
            public[public_instance::EP_SHARD_PROTOCOL_LO],
        ),
        (
            public_instance::EP_SHARD_PROTOCOL_LO + 1,
            public[public_instance::EP_SHARD_PROTOCOL_LO + 1],
        ),
    ] {
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[parent_offset].assigned(),
            expected,
            parent_enabled,
        );
    }
    let zero = ctx.load_zero();
    constrain_equal_if_v1(
        ctx,
        gate,
        *parent[public_instance::COMPLETE].assigned(),
        zero,
        parent_enabled,
    );
    for (offset, expected) in [
        (public_instance::NEXT_STAGE, leaf_stage),
        (public_instance::NEXT_JOB, leaf_job),
        (public_instance::NEXT_BLOCK, leaf_block),
    ] {
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[offset].assigned(),
            expected,
            parent_enabled,
        );
    }
    let first_block = gate.is_zero(ctx, leaf_block);
    let expected_active = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(leaf_blocks),
        first_block,
    );
    constrain_equal_if_v1(
        ctx,
        gate,
        *parent[public_instance::ACTIVE_JOB_BLOCKS].assigned(),
        expected_active,
        parent_enabled,
    );
    for index in 0..DIGEST_SIZE {
        let expected = *shard[shard_public::INITIAL_STATE + index].assigned();
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[chaining_state_offset + index].assigned(),
            expected,
            parent_enabled,
        );
    }

    let next_stage = gate.add(ctx, Existing(leaf_stage), Constant(C::ScalarExt::ONE));
    ctx.constrain_equal(&next_stage, &public[public_instance::NEXT_STAGE]);
    let final_block = *shard[shard_public::FINAL_BLOCK].assigned();
    let next_job = gate.add(ctx, Existing(leaf_job), Existing(final_block));
    ctx.constrain_equal(&next_job, &public[public_instance::NEXT_JOB]);
    let block_plus_one = gate.add(ctx, Existing(leaf_block), Constant(C::ScalarExt::ONE));
    let next_block = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(block_plus_one),
        final_block,
    );
    ctx.constrain_equal(&next_block, &public[public_instance::NEXT_BLOCK]);
    let next_active = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(leaf_blocks),
        final_block,
    );
    ctx.constrain_equal(&next_active, &public[public_instance::ACTIVE_JOB_BLOCKS]);
    for index in 0..DIGEST_SIZE {
        let output = *shard[shard_public::OUTPUT_STATE + index].assigned();
        let expected = gate.select(
            ctx,
            Constant(C::ScalarExt::from(u64::from(IV[index]))),
            Existing(output),
            final_block,
        );
        ctx.constrain_equal(&expected, &public[chaining_state_offset + index]);
    }

    let release =
        public_digest_cells_from_slice_v1(public, public_instance::RELEASE_LO, "release")?;
    let plan_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let terminal_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_TERMINAL_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_TERMINAL_ROOT_LO,
    };
    let message_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_MESSAGE_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_MESSAGE_ROOT_LO,
    };
    let expected_message_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_EXPECTED_MESSAGE_ROOT_LO,
    };
    let expected_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_EXPECTED_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_EXPECTED_ROOT_LO,
    };
    let plan_limbs = public_digest_cells_from_slice_v1(public, plan_offset, "plan")?;
    let expected_limbs =
        public_digest_cells_from_slice_v1(public, expected_offset, "expected root")?;
    let expected_message_limbs = public_digest_cells_from_slice_v1(
        public,
        expected_message_offset,
        "expected message root",
    )?;
    let expected_message_root = assign_encoded_scalar_v1(
        ctx,
        range,
        state.plan.expected_message_root,
        expected_message_limbs,
    )?;
    let expected_root = assign_encoded_scalar_v1(
        ctx,
        range,
        state.plan.expected_terminal_root,
        expected_limbs,
    )?;
    let plan_binding = assign_encoded_scalar_v1(ctx, range, state.plan.plan_binding, plan_limbs)?;
    constrain_plan_binding_v1(
        ctx,
        range,
        release,
        public[public_instance::TOTAL_STAGES],
        public[public_instance::TOTAL_JOBS],
        expected_message_root,
        expected_root,
        plan_binding,
    );

    // Both branches are always assigned so bootstrap and continuation compile to one circuit/VK.
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let seed_root = poseidon.hash(
        ctx,
        range,
        TERMINAL_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            public[public_instance::TOTAL_STAGES],
            public[public_instance::TOTAL_JOBS],
        ],
    );
    let seed_message_root = poseidon.hash(
        ctx,
        range,
        MESSAGE_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            public[public_instance::TOTAL_STAGES],
            public[public_instance::TOTAL_JOBS],
        ],
    );
    let parent_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(terminal_offset..terminal_offset + 2)
        .ok_or_else(|| "mint hash parent terminal root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash parent terminal root shape drifted".to_owned())?;
    let previous_digest = previous
        .map(|claim| claim.terminal_root)
        .unwrap_or(state.terminal_root);
    let (assigned_parent_root, assigned_parent_limbs) =
        assign_scalar_digest_v1(ctx, range, previous_digest)?;
    for (actual, expected) in assigned_parent_limbs.into_iter().zip(parent_limbs) {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior_root = gate.select(
        ctx,
        Existing(assigned_parent_root),
        Existing(seed_root),
        parent_enabled,
    );
    let parent_message_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(message_offset..message_offset + 2)
        .ok_or_else(|| "mint hash parent message root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash parent message root shape drifted".to_owned())?;
    let previous_message_digest = previous
        .map(|claim| claim.message_root)
        .unwrap_or(state.message_root);
    let (assigned_parent_message_root, assigned_parent_message_limbs) =
        assign_scalar_digest_v1(ctx, range, previous_message_digest)?;
    for (actual, expected) in assigned_parent_message_limbs
        .into_iter()
        .zip(parent_message_limbs)
    {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior_message_root = gate.select(
        ctx,
        Existing(assigned_parent_message_root),
        Existing(seed_message_root),
        parent_enabled,
    );
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let mut terminal_inputs = Vec::with_capacity(3 + DIGEST_SIZE);
    terminal_inputs.push(prior_root);
    terminal_inputs.push(leaf_job);
    terminal_inputs.push(leaf_blocks);
    terminal_inputs.extend(
        shard[shard_public::OUTPUT_STATE..shard_public::OUTPUT_STATE + DIGEST_SIZE]
            .iter()
            .map(|value| *value.assigned()),
    );
    let advanced_root = poseidon.hash(ctx, range, TERMINAL_STEP_DOMAIN_V1, &terminal_inputs);
    let current_root = gate.select(
        ctx,
        Existing(advanced_root),
        Existing(prior_root),
        final_block,
    );
    let current_limbs =
        public_digest_cells_from_slice_v1(public, terminal_offset, "terminal root")?;
    let expected_current =
        assign_encoded_scalar_v1(ctx, range, state.terminal_root, current_limbs)?;
    ctx.constrain_equal(&current_root, &expected_current);

    let mut message_inputs = Vec::with_capacity(5 + BLOCK_SIZE);
    message_inputs.extend([
        prior_message_root,
        leaf_stage,
        leaf_job,
        leaf_block,
        leaf_blocks,
    ]);
    message_inputs.extend(
        shard[shard_public::BLOCK_WORDS..shard_public::BLOCK_WORDS + BLOCK_SIZE]
            .iter()
            .map(|value| *value.assigned()),
    );
    let current_message_root = poseidon.hash(ctx, range, MESSAGE_STEP_DOMAIN_V1, &message_inputs);
    let current_message_limbs =
        public_digest_cells_from_slice_v1(public, message_offset, "message root")?;
    let expected_current_message =
        assign_encoded_scalar_v1(ctx, range, state.message_root, current_message_limbs)?;
    ctx.constrain_equal(&current_message_root, &expected_current_message);

    let stage_complete = gate.is_equal(
        ctx,
        public[public_instance::NEXT_STAGE],
        public[public_instance::TOTAL_STAGES],
    );
    let job_complete = gate.is_equal(
        ctx,
        public[public_instance::NEXT_JOB],
        public[public_instance::TOTAL_JOBS],
    );
    let block_boundary = gate.is_zero(ctx, public[public_instance::NEXT_BLOCK]);
    let root_complete = gate.is_equal(ctx, current_root, expected_root);
    let message_complete = gate.is_equal(ctx, current_message_root, expected_message_root);
    let complete = [
        job_complete,
        block_boundary,
        root_complete,
        message_complete,
    ]
    .into_iter()
    .fold(stage_complete, |value, condition| {
        gate.mul(ctx, Existing(value), Existing(condition))
    });
    ctx.constrain_equal(&complete, &public[public_instance::COMPLETE]);
    let not_complete = gate.not(ctx, Existing(complete));
    let terminal_mismatch = gate.mul(ctx, Existing(stage_complete), Existing(not_complete));
    gate.assert_is_const(ctx, &terminal_mismatch, &C::ScalarExt::ZERO);
    Ok(())
}

fn bind_shard_instances_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    shard: &[DeferredScalar<'chip, C>],
    parity: KagemushaPastaParityV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if shard.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash shard recursive public column is truncated".to_owned());
    }
    let plan_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    let shard_version = *shard[shard_public::VERSION].assigned();
    let shard_parity = *shard[shard_public::PARITY].assigned();
    ctx.constrain_equal(&shard_version, &public[public_instance::VERSION]);
    ctx.constrain_equal(&shard_parity, &public[public_instance::PARITY]);
    for (leaf, expected) in shard[shard_public::RELEASE_LO..shard_public::RELEASE_LO + 2]
        .iter()
        .zip(&public[public_instance::RELEASE_LO..public_instance::RELEASE_LO + 2])
    {
        let leaf = *leaf.assigned();
        ctx.constrain_equal(&leaf, expected);
    }
    for (leaf, expected) in shard[shard_public::PLAN_LO..shard_public::PLAN_LO + 2]
        .iter()
        .zip(&public[plan_offset..plan_offset + 2])
    {
        let leaf = *leaf.assigned();
        ctx.constrain_equal(&leaf, expected);
    }
    Ok(())
}

fn constrain_proof_chain_root_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parent: &[DeferredScalar<'chip, C>],
    parity: KagemushaPastaParityV1,
    parent_enabled: AssignedValue<C::ScalarExt>,
    previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    metadata: KagemushaMintHashClaimMetadataV1,
    parent_transcript_binding: AssignedValue<C::ScalarExt>,
    shard_transcript_binding: AssignedValue<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let proof_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PROOF_CHAIN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PROOF_CHAIN_LO,
    };
    let plan_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let chip = loader.ecc_chip();
    let range = chip.range();
    let gate = range.gate();
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    // As with the state root, assign both branches unconditionally so there is one circuit/VK.
    let release = &public[public_instance::RELEASE_LO..public_instance::RELEASE_LO + 2];
    let plan = &public[plan_offset..plan_offset + 2];
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let seed = poseidon.hash(
        ctx,
        range,
        PROOF_CHAIN_SEED_DOMAIN_V2,
        &[release[0], release[1], plan[0], plan[1]],
    );
    let digest = previous_metadata
        .map(|previous| match parity {
            KagemushaPastaParityV1::Eq => previous.eq_proof_chain_root,
            KagemushaPastaParityV1::Ep => previous.ep_proof_chain_root,
        })
        .unwrap_or(match parity {
            KagemushaPastaParityV1::Eq => metadata.eq_proof_chain_root,
            KagemushaPastaParityV1::Ep => metadata.ep_proof_chain_root,
        });
    let (assigned_parent, assigned_parent_limbs) = assign_scalar_digest_v1(ctx, range, digest)?;
    let parent_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(proof_offset..proof_offset + 2)
        .ok_or_else(|| "mint hash predecessor proof-chain root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash predecessor proof-chain root shape drifted".to_owned())?;
    for (actual, expected) in assigned_parent_limbs.into_iter().zip(parent_limbs) {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior = gate.select(
        ctx,
        Existing(assigned_parent),
        Existing(seed),
        parent_enabled,
    );
    // The bootstrap proof is a shape-only placeholder whose verifier equations are disabled.
    // Bind it to one canonical zero instead of giving arbitrary dummy bytes a chain identity.
    let zero = ctx.load_constant(C::ScalarExt::ZERO);
    let selected_parent_transcript = gate.select(
        ctx,
        Existing(parent_transcript_binding),
        Existing(zero),
        parent_enabled,
    );
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let root = poseidon.hash(
        ctx,
        range,
        PROOF_CHAIN_STEP_DOMAIN_V2,
        &[
            prior,
            public[public_instance::NEXT_STAGE],
            selected_parent_transcript,
            shard_transcript_binding,
        ],
    );
    let expected_limbs = public_digest_cells_from_slice_v1(public, proof_offset, "proof chain")?;
    let expected_digest = match parity {
        KagemushaPastaParityV1::Eq => metadata.eq_proof_chain_root,
        KagemushaPastaParityV1::Ep => metadata.ep_proof_chain_root,
    };
    let expected = assign_encoded_scalar_v1(ctx, range, expected_digest, expected_limbs)?;
    ctx.constrain_equal(&root, &expected);
    Ok(())
}

fn assign_encoded_scalar_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: DigestV1,
    expected_limbs: [AssignedValue<F>; 2],
) -> Result<AssignedValue<F>, String> {
    let (assigned, limbs) = assign_scalar_digest_v1(ctx, range, digest)?;
    for (actual, expected) in limbs.into_iter().zip(expected_limbs) {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(assigned)
}

fn assign_scalar_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: DigestV1,
) -> Result<(AssignedValue<F>, [AssignedValue<F>; 2]), String> {
    let scalar = decode::<F>(digest)
        .ok_or_else(|| "mint hash field-native digest is not canonical".to_owned())?;
    let assigned = ctx.load_witness(scalar);
    let limbs = scalar_digest_limbs_v1(ctx, range.gate(), assigned);
    Ok((assigned, limbs))
}

fn scalar_digest_limbs_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    scalar: AssignedValue<F>,
) -> [AssignedValue<F>; 2] {
    let bits = gate.num_to_bits(ctx, scalar, F::NUM_BITS as usize);
    std::array::from_fn(|limb| {
        let start = limb * 128;
        let end = (start + 128).min(bits.len());
        gate.inner_product(
            ctx,
            bits[start..end].iter().copied(),
            (0..end - start).map(|bit| Constant(from_u128::<F>(1_u128 << bit))),
        )
    })
}

fn constrain_equal_if_v1<F: halo2_base::utils::ScalarField>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    actual: AssignedValue<F>,
    expected: AssignedValue<F>,
    enabled: AssignedValue<F>,
) {
    let difference = gate.sub(ctx, Existing(actual), Existing(expected));
    let selected = gate.mul(ctx, Existing(difference), Existing(enabled));
    gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn terminal_seed_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        TERMINAL_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
        ],
    )
}

fn message_seed_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        MESSAGE_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
        ],
    )
}

fn message_step_native<F: KagemushaPoseidonFieldV1>(
    prior: F,
    leaf: &KagemushaMintHashShardStatementV1,
) -> F {
    let mut inputs = Vec::with_capacity(5 + BLOCK_SIZE);
    inputs.extend([
        prior,
        F::from(leaf.stage_index),
        F::from(u64::from(leaf.job_index)),
        F::from(u64::from(leaf.block_index)),
        F::from(u64::from(leaf.job_block_count)),
    ]);
    inputs.extend(leaf.block_words.map(|word| F::from(u64::from(word))));
    hash(MESSAGE_STEP_DOMAIN_V1, &inputs)
}

fn terminal_step_native<F: KagemushaPoseidonFieldV1>(
    prior: F,
    job_index: u32,
    job_block_count: u32,
    output_state: [u32; DIGEST_SIZE],
) -> F {
    let mut inputs = Vec::with_capacity(3 + DIGEST_SIZE);
    inputs.push(prior);
    inputs.push(F::from(u64::from(job_index)));
    inputs.push(F::from(u64::from(job_block_count)));
    inputs.extend(output_state.map(|word| F::from(u64::from(word))));
    hash(TERMINAL_STEP_DOMAIN_V1, &inputs)
}

fn plan_binding_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
    expected_message_root: F,
    expected_terminal_root: F,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        PLAN_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
            expected_message_root,
            expected_terminal_root,
        ],
    )
}

/// Derive the public proof-chain root from the final authenticated proof transcripts this step.
pub(crate) fn mint_hash_proof_chain_root_v1<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    plan_binding: DigestV1,
    next_stage: u64,
    previous_root: Option<DigestV1>,
    parent_transcript_binding: F,
    shard_transcript_binding: F,
) -> Result<DigestV1, String> {
    if release_id == [0; 32] || plan_binding == [0; 32] || next_stage == 0 {
        return Err("mint hash proof-chain binding is missing its release/plan/stage".to_owned());
    }
    if (next_stage == 1) != previous_root.is_none() {
        return Err(
            "mint hash proof-chain predecessor presence does not match the stage".to_owned(),
        );
    }
    let prior = if let Some(previous) = previous_root {
        decode::<F>(previous)
            .ok_or_else(|| "mint hash prior proof-chain root is noncanonical".to_owned())?
    } else {
        let release = digest_limbs::<F>(release_id);
        let plan = digest_limbs::<F>(plan_binding);
        hash(
            PROOF_CHAIN_SEED_DOMAIN_V2,
            &[release[0], release[1], plan[0], plan[1]],
        )
    };
    // A first-step parent proof is only a circuit-shape placeholder. Match the in-circuit
    // stage selector by assigning that placeholder the unique zero identity.
    let selected_parent_transcript = if next_stage == 1 {
        F::ZERO
    } else {
        parent_transcript_binding
    };
    Ok(encode(hash(
        PROOF_CHAIN_STEP_DOMAIN_V2,
        &[
            prior,
            F::from(next_stage),
            selected_parent_transcript,
            shard_transcript_binding,
        ],
    )))
}

/// Require the helper basis to be the exact prefix of the authenticated monetary basis.
pub(crate) fn validate_mint_hash_shard_basis_prefix_v1<C>(
    carrier: &ParamsIPA<C>,
    shard: &ParamsIPA<C>,
) -> Result<(), String>
where
    C: CurveAffine + PartialEq,
{
    if carrier.k() != KAGEMUSHA_RECURSION_IPA_K_V1 || shard.k() != KAGEMUSHA_MINT_HASH_SHARD_K_V1 {
        return Err("mint hash shard/carrier IPA domains are not k=12/k=16".to_owned());
    }
    let expected = 1_usize << KAGEMUSHA_MINT_HASH_SHARD_K_V1;
    if shard.get_g().len() != expected
        || carrier.get_g().len() < expected
        || shard.get_g() != &carrier.get_g()[..expected]
    {
        return Err("mint hash shard generator basis is not the carrier prefix".to_owned());
    }
    Ok(())
}

/// Lift a `k = 12` opening claim into the exact first 4,096 slots of the `k = 16` history.
fn lift_mint_hash_shard_accumulator_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    shard: DeferredAccumulator<'chip, C>,
) -> Result<DeferredAccumulator<'chip, C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if shard.xi.len() != KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize {
        return Err("mint hash shard accumulator has the wrong round count".to_owned());
    }
    let mut xi = Vec::with_capacity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize);
    xi.extend((0..SHARD_TO_HISTORY_ZERO_ROUNDS_V1).map(|_| loader.load_const(&C::ScalarExt::ZERO)));
    xi.extend(shard.xi);
    if xi.len() != KAGEMUSHA_RECURSION_IPA_K_V1 as usize {
        return Err("mint hash shard accumulator lift shape drifted".to_owned());
    }
    Ok(IpaAccumulator::new(xi, shard.u))
}

/// Circuit-side typed-plan binding used by each paired recursive step.
fn constrain_plan_binding_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    release: [AssignedValue<F>; 2],
    total_stages: AssignedValue<F>,
    total_jobs: AssignedValue<F>,
    expected_message_root: AssignedValue<F>,
    expected_terminal_root: AssignedValue<F>,
    expected_plan_binding: AssignedValue<F>,
) {
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let actual = poseidon.hash(
        ctx,
        range,
        PLAN_DOMAIN_V1,
        &[
            release[0],
            release[1],
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root,
        ],
    );
    ctx.constrain_equal(&actual, &expected_plan_binding);
}

const _: () = {
    assert!(KAGEMUSHA_MINT_HASH_SHARD_K_V1 < KAGEMUSHA_RECURSION_IPA_K_V1);
    assert!(SHARD_TO_HISTORY_ZERO_ROUNDS_V1 == 4);
};

#[cfg(test)]
mod tests {
    use super::super::deferred_parent::accumulator_limb_count;
    use super::*;
    use ff::Field;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::{
            group::{Curve as _, Group as _},
            pasta::{Eq, EqAffine, Fp, Fq},
        },
    };

    const CLAIM_RLC_TEST_K: usize = 9;
    const CLAIM_RLC_TEST_CAPACITY: usize = 4;

    #[derive(Clone, Debug)]
    struct ClaimRlcTestConfig<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        carrier_rlc: KagemushaClaimCarrierRlcConfigV1,
    }

    #[derive(Clone)]
    struct ClaimRlcTestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        machine: KagemushaClaimCarrierRlcMachineV1<F>,
        tamper_padding: bool,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for ClaimRlcTestCircuit<F> {
        type Config = ClaimRlcTestConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                machine: self.machine.unknown(),
                tamper_padding: self.tamper_padding,
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            ClaimRlcTestConfig {
                base,
                carrier_rlc: KagemushaClaimCarrierRlcConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("claim RLC test uses Base parameters")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "claim RLC test Base"),
            )?;
            layouter.assign_table(
                || "claim RLC compact test range",
                |mut table| {
                    // This fixed sparse table covers the deliberately distinct limbs generated
                    // by the boundary witnesses below. A tuple lookup that incorrectly reused
                    // one table row for all limbs would reject the positive circuit.
                    for (row, value) in [
                        0_u64, 1, 2, 3, 4, 7, 8, 9, 18, 27, 83, 127, 255, 16_256, 32_640, 32_766,
                        32_767,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        table.assign_cell(
                            || "compact range value",
                            config.carrier_rlc.range_table,
                            row,
                            || Value::known(F::from(value)),
                        )?;
                    }
                    Ok(())
                },
            )?;
            let mut rows = self
                .machine
                .build_rows_with_capacity(CLAIM_RLC_TEST_CAPACITY)
                .map_err(|_| PlonkError::Synthesis)?;
            if self.tamper_padding {
                let padding = rows
                    .iter_mut()
                    .find(|row| {
                        matches!(row.mode, ClaimRlcRowModeV1::Preprocess)
                            && row.values[CLAIM_RLC_VALUE] == F::ZERO
                    })
                    .ok_or(PlonkError::Synthesis)?;
                padding.values[CLAIM_RLC_VALUE] = F::ONE;
            }
            self.machine.synthesize_rows(
                &config.carrier_rlc,
                &mut layouter,
                &self.builder.core().copy_manager,
                self.builder.witness_gen_only(),
                &rows,
            )
        }
    }

    fn claim_rlc_test_value_v1(values: &[u128], challenge: u128) -> u128 {
        let mut padded = values.to_vec();
        padded.resize(CLAIM_RLC_TEST_CAPACITY, 0);
        let mut coefficients = padded
            .iter()
            .map(|value| value % CLAIM_CARRIER_RLC_MODULUS_V1)
            .collect::<Vec<_>>();
        for chunk in padded.chunks(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
            let mut power = 1_u128;
            let mut pack = 0_u128;
            for value in chunk {
                pack += (value / CLAIM_CARRIER_RLC_MODULUS_V1) * power;
                power *= CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1;
            }
            coefficients.push(pack);
        }
        let mut accumulator = 0;
        for coefficient in coefficients {
            accumulator = claim_rlc_native_step_v1(accumulator, challenge, coefficient)
                .expect("small test RLC step")
                .1;
        }
        accumulator
    }

    fn claim_rlc_test_circuit_v1<F: KagemushaPoseidonFieldV1>(
        tamper_expected: bool,
        tamper_padding: bool,
    ) -> ClaimRlcTestCircuit<F> {
        let mut builder = BaseCircuitBuilder::<F>::new(false).use_k(CLAIM_RLC_TEST_K);
        let challenge_a_value = 2_u128;
        let challenge_b_value = 3_u128;
        let carrier_values = [
            vec![
                CLAIM_CARRIER_RLC_MODULUS_V1,
                2 * CLAIM_CARRIER_RLC_MODULUS_V1,
                0,
                0,
            ],
            vec![u128::MAX, 0, 0, 0],
        ];
        let challenge_a = builder
            .main(0)
            .load_witness(F::from_u128(challenge_a_value));
        let challenge_b = builder
            .main(0)
            .load_witness(F::from_u128(challenge_b_value));
        let carriers = carrier_values.map(|values| ClaimRlcCarrierV1 {
            expected_a: builder.main(0).load_witness(F::from_u128(
                claim_rlc_test_value_v1(&values, challenge_a_value) + u128::from(tamper_expected),
            )),
            expected_b: builder
                .main(0)
                .load_witness(F::from_u128(claim_rlc_test_value_v1(
                    &values,
                    challenge_b_value,
                ))),
            values: values
                .into_iter()
                .map(|value| builder.main(0).load_witness(F::from_u128(value)))
                .collect(),
        });
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        ClaimRlcTestCircuit {
            builder,
            machine: KagemushaClaimCarrierRlcMachineV1 {
                challenge_a,
                challenge_b,
                carriers,
                use_unknown: false,
            },
            tamper_padding,
        }
    }

    fn padded_coefficients<F: Field>(xi: &[F]) -> Vec<F> {
        let mut coefficients = vec![F::ZERO; 1 << xi.len()];
        coefficients[0] = F::ONE;
        for (len, challenge) in xi.iter().rev().enumerate().map(|(i, xi)| (1 << i, xi)) {
            let (left, right) = coefficients.split_at_mut(len);
            right[..len].copy_from_slice(left);
            for coefficient in &mut right[..len] {
                *coefficient *= challenge;
            }
        }
        coefficients
    }

    fn statements() -> (
        KagemushaMintHashClaimPlanV1,
        Vec<KagemushaMintHashShardStatementV1>,
    ) {
        use super::super::mint_hash_shard::KagemushaMintHashPlanV1;

        let release = [0x41; 32];
        let placeholder = [0x77; 32];
        let messages = vec![b"first mint job".to_vec(), vec![0x5a; 130]];
        let provisional = KagemushaMintHashPlanV1::from_messages(
            release,
            KagemushaPastaParityV1::Eq,
            placeholder,
            messages.clone(),
        )
        .unwrap();
        let plan =
            KagemushaMintHashClaimPlanV1::from_leaves::<Fp>(release, provisional.leaves()).unwrap();
        let exact = KagemushaMintHashPlanV1::from_messages(
            release,
            KagemushaPastaParityV1::Eq,
            plan.plan_binding,
            messages,
        )
        .unwrap();
        (plan, exact.leaves().to_vec())
    }

    fn parity_plan<F: KagemushaPoseidonFieldV1>(
        release: DigestV1,
        parity: KagemushaPastaParityV1,
        message: Vec<u8>,
    ) -> (
        KagemushaMintHashClaimPlanV1,
        Vec<KagemushaMintHashShardStatementV1>,
    ) {
        use super::super::mint_hash_shard::KagemushaMintHashPlanV1;

        let provisional = KagemushaMintHashPlanV1::from_messages(
            release,
            parity,
            [0x77; 32],
            vec![message.clone()],
        )
        .unwrap();
        let plan =
            KagemushaMintHashClaimPlanV1::from_leaves::<F>(release, provisional.leaves()).unwrap();
        let exact = KagemushaMintHashPlanV1::from_messages(
            release,
            parity,
            plan.plan_binding,
            vec![message],
        )
        .unwrap();
        (plan, exact.leaves().to_vec())
    }

    #[test]
    fn paired_claim_preserves_distinct_parity_sha_states_at_one_shared_cursor() {
        let release = [0x41; 32];
        let (eq_plan, eq_leaves) =
            parity_plan::<Fp>(release, KagemushaPastaParityV1::Eq, vec![0x11; 130]);
        let (ep_plan, ep_leaves) =
            parity_plan::<Fq>(release, KagemushaPastaParityV1::Ep, vec![0x22; 130]);
        let eq_leaf = &eq_leaves[0];
        let ep_leaf = &ep_leaves[0];
        assert_ne!(eq_leaf.block_words, ep_leaf.block_words);
        assert_ne!(eq_leaf.output_state, ep_leaf.output_state);
        validate_paired_leaf_v1(eq_leaf, ep_leaf).unwrap();

        let eq = KagemushaMintHashClaimStateV1::apply::<Fp>(eq_plan, None, eq_leaf).unwrap();
        let ep = KagemushaMintHashClaimStateV1::apply::<Fq>(ep_plan, None, ep_leaf).unwrap();
        assert_eq!(eq.next_stage, ep.next_stage);
        assert_eq!(eq.next_job, ep.next_job);
        assert_eq!(eq.next_block, ep.next_block);
        assert_eq!(eq.active_job_blocks, ep.active_job_blocks);
        assert_ne!(eq.chaining_state, ep.chaining_state);
        KagemushaMintHashClaimPairStateV1 { eq, ep }
            .validate()
            .unwrap();

        let mut wrong_shape = ep_leaf.clone();
        wrong_shape.job_block_count += 1;
        assert!(validate_paired_leaf_v1(eq_leaf, &wrong_shape).is_err());
    }

    #[test]
    fn claim_public_shape_has_one_chaining_state_per_parity() {
        assert_eq!(public_instance::EQ_CHAINING_STATE, 15);
        assert_eq!(public_instance::EP_CHAINING_STATE, 23);
        assert_eq!(public_instance::EQ_MESSAGE_ROOT_LO, 31);
        assert_eq!(public_instance::HISTORY_START, 63);
        assert_eq!(KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1, 97);
        assert_eq!(
            KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 - public_instance::HISTORY_START,
            accumulator_limb_count(),
            "the external history slice must exclude internal carrier commitments",
        );
    }

    #[test]
    fn compact_claim_carrier_has_fixed_injective_binding_capacity() {
        assert_eq!(KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1, 58);
        assert_eq!(KAGEMUSHA_MINT_HASH_CLAIM_DENSE_LANES_V1, 2);
        assert_eq!(KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1, 1_008);
        assert_eq!(
            4 * KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1
                + KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        );
        assert_eq!(
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
                + KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1,
        );
        assert_eq!(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1, 14);
        assert_eq!(
            CLAIM_CARRIER_RLC_DOMAIN_V1,
            u64::from_le_bytes(*b"kgmrlc_2")
        );
        assert_eq!(CLAIM_CARRIER_RLC_VERSION_V1, 2);
        assert_eq!(CLAIM_CARRIER_RLC_MODULUS_V1, (1_u128 << 127) - 1);
        assert_eq!(CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1, 3);
        assert_eq!(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1, 80);
        assert!(
            CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1
                .pow(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1 as u32)
                - 1
                < CLAIM_CARRIER_RLC_MODULUS_V1
        );
        assert_eq!(
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
                + KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
                    .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1),
            4_142,
        );
        let fixed_packs = KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
            .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
        assert_eq!(
            2 * (4 + 3 * KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 + 2 * fixed_packs),
            24_756,
            "the two-carrier custom machine must retain its fully padded row schedule",
        );
    }

    #[test]
    fn carrier_rlc_configuration_has_three_fixed_mode_columns() {
        let mut meta = ConstraintSystem::<Fp>::default();
        let config = KagemushaClaimCarrierRlcConfigV1::configure(&mut meta);
        assert_eq!(config.advice.len(), CLAIM_RLC_COLUMNS);
        assert_eq!(meta.num_advice_columns(), CLAIM_RLC_COLUMNS);
        assert_eq!(
            meta.num_fixed_columns(),
            4,
            "three mode/payload columns plus the owned range table",
        );
        assert_eq!(meta.num_selectors(), 0);
        assert_eq!(meta.permutation().get_columns().len(), 1);
        assert_eq!(meta.degree(), 6);
    }

    #[test]
    fn carrier_rlc_fixed_mode_payload_encoding_is_injective() {
        let state = ClaimRlcStateV1 {
            challenge_a: 1,
            challenge_b: 1,
            accumulator_a: 0,
            accumulator_b: 0,
            quotient_pack: 0,
            coefficient: 0,
        };
        let negative_one = Fp::ZERO - Fp::ONE;
        let cases = [
            (
                ClaimRlcRowModeV1::StartA,
                false,
                false,
                Fp::ZERO,
                [Fp::ZERO, Fp::ONE, Fp::ZERO],
            ),
            (
                ClaimRlcRowModeV1::StartB,
                false,
                false,
                Fp::ZERO,
                [Fp::ZERO, Fp::ONE, Fp::ONE],
            ),
            (
                ClaimRlcRowModeV1::Preprocess,
                false,
                false,
                Fp::from(9),
                [Fp::ONE, Fp::ZERO, Fp::from(9)],
            ),
            (
                ClaimRlcRowModeV1::EvaluateA,
                false,
                false,
                Fp::ZERO,
                [Fp::ONE, Fp::ONE, Fp::ZERO],
            ),
            (
                ClaimRlcRowModeV1::EvaluateA,
                false,
                true,
                Fp::ZERO,
                [Fp::ONE, Fp::ONE, Fp::ONE],
            ),
            (
                ClaimRlcRowModeV1::EvaluateB,
                false,
                false,
                Fp::ZERO,
                [Fp::ONE, Fp::ONE, Fp::from(2)],
            ),
            (
                ClaimRlcRowModeV1::EvaluateB,
                true,
                false,
                Fp::ZERO,
                [Fp::ONE, Fp::ONE, negative_one],
            ),
            (
                ClaimRlcRowModeV1::EndA,
                false,
                false,
                Fp::ZERO,
                [Fp::ZERO, Fp::ONE, Fp::from(2)],
            ),
            (
                ClaimRlcRowModeV1::EndB,
                false,
                false,
                Fp::ZERO,
                [Fp::ZERO, Fp::ONE, Fp::from(3)],
            ),
        ];
        let mut encodings = Vec::with_capacity(cases.len());
        for (mode, store_pack, load_pack, ternary_power, expected) in cases {
            let mut row = claim_rlc_state_row_v1::<Fp>(state, mode, None);
            row.store_pack = store_pack;
            row.load_pack = load_pack;
            row.ternary_power = ternary_power;
            let encoding = claim_rlc_fixed_encoding_v1(&row).expect("valid fixed mode encoding");
            assert_eq!(encoding, expected);
            encodings.push(encoding);
        }
        for (index, encoding) in encodings.iter().enumerate() {
            assert!(
                !encodings[..index].contains(encoding),
                "fixed mode/payload encodings must not alias",
            );
        }

        let mut invalid = claim_rlc_state_row_v1::<Fp>(state, ClaimRlcRowModeV1::EvaluateA, None);
        invalid.store_pack = true;
        assert!(claim_rlc_fixed_encoding_v1(&invalid).is_err());
        let invalid = claim_rlc_state_row_v1::<Fp>(state, ClaimRlcRowModeV1::Preprocess, None);
        assert!(claim_rlc_fixed_encoding_v1(&invalid).is_err());
    }

    #[test]
    fn carrier_rlc_canonical_encoding_is_injective_at_u128_boundaries() {
        let zero = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        let zero_coefficients = native_claim_carrier_coefficients_v1(&zero).unwrap();
        let boundaries = [
            0,
            CLAIM_CARRIER_RLC_MODULUS_V1 - 1,
            CLAIM_CARRIER_RLC_MODULUS_V1,
            CLAIM_CARRIER_RLC_MODULUS_V1 + 1,
            2 * CLAIM_CARRIER_RLC_MODULUS_V1 - 1,
            2 * CLAIM_CARRIER_RLC_MODULUS_V1,
            u128::MAX,
        ];
        for &position in &[0, 79, 80, 159, 4_089] {
            for &value in &boundaries {
                let mut carrier = zero.clone();
                carrier[position] = value;
                let coefficients = native_claim_carrier_coefficients_v1(&carrier).unwrap();
                assert_eq!(coefficients.len(), 4_142);
                let quotient_pack = coefficients
                    [KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 + position / 80];
                let ternary_power = CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1.pow((position % 80) as u32);
                let quotient = (quotient_pack / ternary_power) % 3;
                assert_eq!(coefficients[position], value % CLAIM_CARRIER_RLC_MODULUS_V1);
                assert_eq!(quotient, value / CLAIM_CARRIER_RLC_MODULUS_V1);
                assert_eq!(
                    coefficients[position] + quotient * CLAIM_CARRIER_RLC_MODULUS_V1,
                    value,
                );
                if value == 0 {
                    assert_eq!(coefficients, zero_coefficients);
                } else {
                    assert_ne!(coefficients, zero_coefficients);
                }
            }
        }

        let mut modulus = zero;
        modulus[KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 - 1] =
            CLAIM_CARRIER_RLC_MODULUS_V1;
        assert_ne!(
            native_claim_carrier_rlc_v1(
                &vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1],
                1,
            )
            .unwrap(),
            native_claim_carrier_rlc_v1(&modulus, 1).unwrap(),
            "canonical quotient packing must distinguish 0 from the valid u128 value M",
        );
    }

    #[test]
    fn carrier_rlc_active_prefix_matches_fixed_host_polynomial_in_both_pasta_fields() {
        fn assert_field<F: KagemushaPoseidonFieldV1>() {
            const TEST_K: usize = 13;

            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(TEST_K)
                .use_lookup_bits(TEST_K - 1);
            for (active_len, trailing_active_zero) in
                [(79, false), (80, false), (81, false), (81, true)]
            {
                let mut carrier = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
                for (index, value) in carrier[..active_len].iter_mut().enumerate() {
                    *value = match index % 4 {
                        0 => CLAIM_CARRIER_RLC_MODULUS_V1,
                        1 => 2 * CLAIM_CARRIER_RLC_MODULUS_V1,
                        2 => u128::MAX,
                        _ => CLAIM_CARRIER_RLC_MODULUS_V1 + index as u128 + 1,
                    };
                }
                carrier[active_len - 1] = if trailing_active_zero {
                    0
                } else {
                    CLAIM_CARRIER_RLC_MODULUS_V1 + 17
                };
                let expected_coefficients = native_claim_carrier_coefficients_v1(&carrier).unwrap();
                let challenge = 1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1;
                let expected_rlc_value = native_claim_carrier_rlc_v1(&carrier, challenge).unwrap();
                let assigned = carrier[..active_len]
                    .iter()
                    .copied()
                    .map(|value| builder.main(0).load_witness(F::from_u128(value)))
                    .collect::<Vec<_>>();
                let range = builder.range_chip();
                let coefficients =
                    canonical_claim_carrier_coefficients_v1(builder.main(0), &range, &assigned)
                        .unwrap();
                let active_remainders = coefficients
                    .active_remainders
                    .iter()
                    .copied()
                    .map(|value| assigned_u128_cell_v1(value, "test carrier remainder").unwrap())
                    .collect::<Vec<_>>();
                let active_quotient_packs = coefficients
                    .active_quotient_packs
                    .iter()
                    .copied()
                    .map(|value| {
                        assigned_u128_cell_v1(value, "test carrier quotient pack").unwrap()
                    })
                    .collect::<Vec<_>>();
                let quotient_pack_count =
                    active_len.div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1);
                assert_eq!(
                    active_remainders,
                    expected_coefficients[..active_len].to_vec()
                );
                assert_eq!(
                    active_quotient_packs,
                    expected_coefficients[KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
                        ..KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
                            + quotient_pack_count]
                        .to_vec()
                );
                assert_eq!(
                    coefficients.remainder_zero_tail,
                    KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 - active_len
                );
                assert_eq!(
                    coefficients.quotient_pack_zero_tail,
                    KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
                        .div_ceil(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1)
                        - quotient_pack_count
                );
                let challenge = builder.main(0).load_constant(F::from_u128(challenge));
                let actual_rlc = assigned_claim_carrier_rlc_v1(
                    builder.main(0),
                    &range,
                    &coefficients,
                    challenge,
                );
                assert_eq!(
                    assigned_u128_cell_v1(actual_rlc, "test carrier RLC").unwrap(),
                    expected_rlc_value
                );
                let expected_rlc = builder
                    .main(0)
                    .load_constant(F::from_u128(expected_rlc_value));
                builder.main(0).constrain_equal(&actual_rlc, &expected_rlc);
                let padding_zero = builder.main(0).load_constant(F::ZERO);
                let mut fixed_values = assigned.clone();
                fixed_values.resize(
                    KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
                    padding_zero,
                );
                let machine = KagemushaClaimCarrierRlcMachineV1 {
                    challenge_a: challenge,
                    challenge_b: challenge,
                    carriers: std::array::from_fn(|_| ClaimRlcCarrierV1 {
                        values: fixed_values.clone(),
                        expected_a: expected_rlc,
                        expected_b: expected_rlc,
                    }),
                    use_unknown: false,
                };
                let rows = machine.build_rows().expect("fixed-machine RLC rows");
                assert_eq!(machine.required_rows().unwrap(), 24_756);
                assert_eq!(rows.len(), machine.required_rows().unwrap());
                let terminal_rows = rows
                    .iter()
                    .filter(|row| {
                        matches!(row.mode, ClaimRlcRowModeV1::EndA | ClaimRlcRowModeV1::EndB)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(terminal_rows.len(), 4);
                for row in terminal_rows {
                    let accumulator = match row.mode {
                        ClaimRlcRowModeV1::EndA => row.values[CLAIM_RLC_ACCUMULATOR_A],
                        ClaimRlcRowModeV1::EndB => row.values[CLAIM_RLC_ACCUMULATOR_B],
                        _ => unreachable!(),
                    };
                    assert_eq!(accumulator, F::from_u128(expected_rlc_value));
                    assert_eq!(row.values[CLAIM_RLC_BUS], accumulator);
                }
            }
            builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
            MockProver::run(TEST_K as u32, &builder, vec![])
                .expect("active-prefix carrier RLC mock prover")
                .assert_satisfied();
        }

        assert_field::<Fp>();
        assert_field::<Fq>();
    }

    #[test]
    fn carrier_rlc_fixed_machine_accepts_distinct_limbs_in_both_pasta_fields() {
        MockProver::run(
            CLAIM_RLC_TEST_K as u32,
            &claim_rlc_test_circuit_v1::<Fp>(false, false),
            vec![],
        )
        .expect("Fp fixed-machine carrier RLC mock prover")
        .assert_satisfied();
        MockProver::run(
            CLAIM_RLC_TEST_K as u32,
            &claim_rlc_test_circuit_v1::<Fq>(false, false),
            vec![],
        )
        .expect("Fq fixed-machine carrier RLC mock prover")
        .assert_satisfied();
    }

    #[test]
    fn carrier_rlc_fixed_machine_rejects_result_and_padding_tampering() {
        assert!(
            MockProver::run(
                CLAIM_RLC_TEST_K as u32,
                &claim_rlc_test_circuit_v1::<Fp>(true, false),
                vec![],
            )
            .expect("tampered-result carrier RLC mock prover")
            .verify()
            .is_err(),
            "the equality bus must reject a forged public RLC result"
        );
        assert!(
            MockProver::run(
                CLAIM_RLC_TEST_K as u32,
                &claim_rlc_test_circuit_v1::<Fq>(false, true),
                vec![],
            )
            .expect("tampered-padding carrier RLC mock prover")
            .verify()
            .is_err(),
            "the fixed zero-coefficient schedule must reject nonzero padding"
        );
    }

    #[test]
    fn carrier_public_padding_reuses_one_zero_and_rejects_nonzero_tail() {
        const TEST_K: usize = 13;

        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_instance_columns(2);
        let eq_active = [11_u64, 12, 13]
            .map(|value| builder.main(0).load_witness(Fp::from(value)))
            .to_vec();
        let ep_active = [21_u64, 22]
            .map(|value| builder.main(0).load_witness(Fp::from(value)))
            .to_vec();
        let [eq_carrier, ep_carrier] =
            pad_assigned_claim_carriers_v1(&mut builder, [eq_active, ep_active]).unwrap();
        let padding_cell = eq_carrier[3].cell;
        assert!(
            eq_carrier[3..]
                .iter()
                .all(|value| value.cell == padding_cell)
        );
        assert!(
            ep_carrier[2..]
                .iter()
                .all(|value| value.cell == padding_cell)
        );
        builder.assigned_instances = vec![eq_carrier, ep_carrier];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));

        let mut eq_public = vec![Fp::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        eq_public[..3].copy_from_slice(&[Fp::from(11), Fp::from(12), Fp::from(13)]);
        let mut ep_public = vec![Fp::ZERO; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        ep_public[..2].copy_from_slice(&[Fp::from(21), Fp::from(22)]);
        MockProver::run(
            TEST_K as u32,
            &builder,
            vec![eq_public.clone(), ep_public.clone()],
        )
        .expect("zero-padded carrier mock prover")
        .assert_satisfied();

        eq_public[80] = Fp::ONE;
        assert!(
            MockProver::run(TEST_K as u32, &builder, vec![eq_public, ep_public])
                .expect("nonzero-padded carrier mock prover")
                .verify()
                .is_err(),
            "a nonzero public value cannot replace constrained carrier padding"
        );
    }

    #[test]
    fn carrier_rlc_direct_division_bounds_do_not_wrap_either_pasta_field() {
        use der_parser::num_bigint::BigUint;
        use halo2_base::utils::{biguint_to_fe, fe_to_biguint, modulus};

        fn assert_field_bounds<F: KagemushaPoseidonFieldV1>() {
            let one = BigUint::from(1_u8);
            let modulus_m = BigUint::from(CLAIM_CARRIER_RLC_MODULUS_V1);
            let maximum_remainder = &modulus_m - &one;
            let maximum_challenge = &one << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1;
            let maximum_honest_numerator =
                &maximum_remainder * maximum_challenge + &maximum_remainder;
            let maximum_constrained_recomposition =
                ((&one << 126_usize) - &one) * &modulus_m + &maximum_remainder;
            let maximum_modular_product = &maximum_remainder * &maximum_remainder;
            let maximum_product_recomposition =
                ((&one << 127_usize) - &one) * &modulus_m + &maximum_remainder;
            assert!(maximum_honest_numerator.bits() <= 253);
            assert!(maximum_constrained_recomposition.bits() <= 253);
            assert!(maximum_constrained_recomposition < modulus::<F>());
            assert!(maximum_modular_product.bits() <= 254);
            assert!(maximum_modular_product < modulus::<F>());
            assert!(maximum_product_recomposition.bits() <= 254);
            assert!(maximum_product_recomposition < modulus::<F>());

            let expected_quotient = &maximum_honest_numerator / &modulus_m;
            let expected_remainder = &maximum_honest_numerator % &modulus_m;
            assert!(expected_quotient.bits() <= 126);
            assert!(expected_remainder < modulus_m);
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
                .use_lookup_bits((KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize);
            let value = builder
                .main(0)
                .load_witness(biguint_to_fe(&maximum_honest_numerator));
            let range = builder.range_chip();
            let (quotient, remainder) =
                constrain_claim_carrier_division_v1(builder.main(0), &range, value, 126, None);
            assert_eq!(fe_to_biguint(quotient.value()), expected_quotient);
            assert_eq!(fe_to_biguint(remainder.value()), expected_remainder);

            let expected_product_quotient = &maximum_modular_product / &modulus_m;
            let expected_product_remainder = &maximum_modular_product % &modulus_m;
            assert!(expected_product_quotient.bits() <= 127);
            let product = builder
                .main(0)
                .load_witness(biguint_to_fe(&maximum_modular_product));
            let (product_quotient, product_remainder) =
                constrain_claim_carrier_division_v1(builder.main(0), &range, product, 127, None);
            assert_eq!(
                fe_to_biguint(product_quotient.value()),
                expected_product_quotient
            );
            assert_eq!(
                fe_to_biguint(product_remainder.value()),
                expected_product_remainder
            );
        }

        assert_field_bounds::<Fp>();
        assert_field_bounds::<Fq>();
    }

    #[test]
    fn carrier_rlc_binds_position_and_both_challenges() {
        let mut carrier = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        carrier[0] = 11;
        carrier[1] = u128::MAX;
        let mut reordered = carrier.clone();
        reordered.swap(0, 1);
        let first = native_claim_carrier_rlc_v1(&carrier, 7).unwrap();
        let second = native_claim_carrier_rlc_v1(&carrier, 13).unwrap();
        assert_ne!(first, native_claim_carrier_rlc_v1(&reordered, 7).unwrap());
        assert_ne!(first, second);
        assert!(first < CLAIM_CARRIER_RLC_MODULUS_V1);
        assert!(second < CLAIM_CARRIER_RLC_MODULUS_V1);
    }

    #[test]
    fn zero_prefix_lift_preserves_exact_k12_coefficients_and_zeros_the_rest() {
        let xi = (1_u64..=u64::from(KAGEMUSHA_MINT_HASH_SHARD_K_V1))
            .map(Fp::from)
            .collect::<Vec<_>>();
        let small = padded_coefficients(&xi);
        let lifted_xi = [vec![Fp::ZERO; SHARD_TO_HISTORY_ZERO_ROUNDS_V1], xi].concat();
        let lifted = padded_coefficients(&lifted_xi);
        assert_eq!(&lifted[..small.len()], small);
        assert!(
            lifted[small.len()..]
                .iter()
                .all(|value| bool::from(value.is_zero()))
        );
    }

    #[test]
    fn generated_k12_basis_is_the_exact_k16_prefix_and_lift_decides() {
        let carrier = ParamsIPA::<EqAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
        let shard = ParamsIPA::<EqAffine>::new(KAGEMUSHA_MINT_HASH_SHARD_K_V1);
        validate_mint_hash_shard_basis_prefix_v1(&carrier, &shard).unwrap();

        let xi = (1_u64..=u64::from(KAGEMUSHA_MINT_HASH_SHARD_K_V1))
            .map(Fp::from)
            .collect::<Vec<_>>();
        let coefficients = padded_coefficients(&xi);
        let point = shard
            .get_g()
            .iter()
            .zip(coefficients)
            .fold(Eq::identity(), |sum, (base, scalar)| sum + *base * scalar)
            .to_affine();
        let lifted = IpaAccumulator::<EqAffine, NativeLoader>::new(
            [vec![Fp::ZERO; SHARD_TO_HISTORY_ZERO_ROUNDS_V1], xi].concat(),
            point,
        );
        let encoded = super::super::KagemushaEqAccumulatorV1::from_native(&lifted).unwrap();
        super::super::decide_kagemusha_eq_accumulator_v1(&carrier, &encoded).unwrap();
    }

    #[test]
    fn ordered_claim_rejects_missing_reordered_duplicated_and_substituted_leaves() {
        let (plan, leaves) = statements();
        let first = KagemushaMintHashClaimStateV1::apply::<Fp>(plan, None, &leaves[0]).unwrap();
        assert!(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &leaves[0]).is_err());
        assert!(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &leaves[2]).is_err());

        let mut substituted = leaves[1].clone();
        substituted.initial_state[0] ^= 1;
        assert!(
            KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &substituted).is_err()
        );
    }

    #[test]
    fn exact_plan_completes_and_no_count_admission_cap_exists() {
        let (plan, leaves) = statements();
        let mut state = None;
        for leaf in &leaves {
            state = Some(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, state, leaf).unwrap());
        }
        let state = state.unwrap();
        assert!(state.complete);
        assert_eq!(state.next_stage, plan.total_stages);
        assert_eq!(state.next_job, plan.total_jobs);

        // The transition API accepts the arithmetic protocol range directly; there is no
        // hop/ancestry/proof-depth maximum or count-based admission constant.
        let huge = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            u64::from(u32::MAX),
            &[(u32::MAX, [0x1234_5678; DIGEST_SIZE])],
            encode(Fp::from(71)),
        )
        .unwrap();
        assert_eq!(huge.total_stages, u64::from(u32::MAX));
    }

    #[test]
    fn typed_plan_binding_rejects_count_and_terminal_substitution() {
        let (plan, _) = statements();
        let mut count = plan;
        count.total_stages += 1;
        assert!(count.validate::<Fp>().is_err());
        let mut terminal = plan;
        terminal.expected_terminal_root[0] ^= 1;
        assert!(terminal.validate::<Fp>().is_err());
        let mut message = plan;
        message.expected_message_root[0] ^= 1;
        assert!(message.validate::<Fp>().is_err());

        let terminals = [[0x1111_1111; DIGEST_SIZE], [0x2222_2222; DIGEST_SIZE]];
        let message_root = encode(Fp::from(73));
        let original = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            4,
            &[(1, terminals[0]), (3, terminals[1])],
            message_root,
        )
        .unwrap();
        let substituted = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            4,
            &[(2, terminals[0]), (2, terminals[1])],
            message_root,
        )
        .unwrap();
        assert_ne!(original.plan_binding, substituted.plan_binding);
    }

    #[test]
    fn proof_chain_v2_binds_ordered_transcripts_and_zeroes_bootstrap_parent() {
        let release = [0x31; 32];
        let plan = encode::<Fp>(Fp::from(41));
        let first =
            mint_hash_proof_chain_root_v1::<Fp>(release, plan, 1, None, Fp::ZERO, Fp::from(51))
                .unwrap();
        let bootstrap_parent_is_ignored = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            1,
            None,
            Fp::from(999),
            Fp::from(51),
        )
        .unwrap();
        let ordered = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            Fp::from(61),
            Fp::from(62),
        )
        .unwrap();
        let reordered = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            Fp::from(62),
            Fp::from(61),
        )
        .unwrap();
        let duplicated = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            Fp::from(61),
            Fp::from(61),
        )
        .unwrap();
        let substituted = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            Fp::from(61),
            Fp::from(63),
        )
        .unwrap();
        assert_eq!(first, bootstrap_parent_is_ignored);
        assert!(
            mint_hash_proof_chain_root_v1::<Fp>(
                release,
                plan,
                2,
                None,
                Fp::from(61),
                Fp::from(62),
            )
            .is_err()
        );
        assert!(
            mint_hash_proof_chain_root_v1::<Fp>(
                release,
                plan,
                1,
                Some(first),
                Fp::from(61),
                Fp::from(62),
            )
            .is_err()
        );
        assert_ne!(ordered, reordered);
        assert_ne!(ordered, duplicated);
        assert_ne!(ordered, substituted);
    }

    #[test]
    fn compact_batch_binding_covers_every_proof_transcript_and_selector() {
        let transcripts = [0x4001_u64, 0x4002, 0x4003, 0x4004];
        let baseline = mint_hash_claim_batch_input_binding_v1(&transcripts, 1_u64).unwrap();
        for index in 0..CLAIM_BATCH_TRANSCRIPT_BINDING_COUNT_V1 {
            let mut changed_transcripts = transcripts;
            changed_transcripts[index] ^= 1;
            assert_ne!(
                baseline,
                mint_hash_claim_batch_input_binding_v1(&changed_transcripts, 1).unwrap()
            );
        }
        assert_ne!(
            baseline,
            mint_hash_claim_batch_input_binding_v1(&transcripts, 0).unwrap()
        );
    }

    #[test]
    fn compact_batch_binding_rejects_transcript_shape_drift() {
        let transcripts = [0_u64; CLAIM_BATCH_TRANSCRIPT_BINDING_COUNT_V1];

        assert!(
            mint_hash_claim_batch_input_binding_v1(&transcripts[..transcripts.len() - 1], 1)
                .is_err()
        );
        let mut extended = transcripts.to_vec();
        extended.push(0);
        assert!(mint_hash_claim_batch_input_binding_v1(&extended, 1).is_err());
    }
}
