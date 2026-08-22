//! Fixed `k=16` Eq/Fp and Ep/Fq binding circuits for helper statements.
//!
//! These circuits prove canonical 184-word/27-cell public encoding, exact role
//! and protocol selection, closed operation/Android flags, exact-next `u64`
//! arithmetic without overflow, mandatory digest presence, canonical absence
//! of optional Android digests, and two required digest inequalities. They do
//! not yet prove P-256 ECDSA, Android DER/KeyMint extension parsing, or child
//! IPA verification. Consequently they are named binding circuits and cannot
//! authorize a payment or activate the production backend.

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::PrimeField,
        pasta::{Fp, Fq},
    },
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Expression, Instance,
        Selector,
    },
    poly::Rotation,
};

use super::{
    helper_abi::{
        fixed_helper_word_v1, pack_words_as_field, OfflineCashHelperAbiErrorV1,
        ANDROID_DIGEST_OFFSETS, CURRENT_GUARD_WORD_START, CURRENT_HEAD_WORD_START,
        HELPER_ABI_WORDS, HELPER_ANDROID_PRESENT_WORD, HELPER_FROM_HIGH_WORD, HELPER_FROM_LOW_WORD,
        HELPER_INSTANCE_CELLS, HELPER_INSTANCE_CELLS_MAX, HELPER_OPERATION_WORD,
        HELPER_TO_HIGH_WORD, HELPER_TO_LOW_WORD, HELPER_WORDS_PER_INSTANCE, NEXT_GUARD_WORD_START,
        REQUIRED_DIGEST_OFFSETS, TRANSITION_WORD_START,
    },
    helper_relation::OfflineCashValidatedHelperRelationV1,
    protocol::OfflineCashHalo2CircuitRoleV1,
    OfflineCashHalo2ParityV1,
};

const U32_BITS: usize = 32;
const WORD_ROWS: usize = U32_BITS + 1;
const DIGEST_WORDS: usize = 8;
const PACKED_BITS: u32 = (HELPER_WORDS_PER_INSTANCE * U32_BITS) as u32;

const _: () = assert!(HELPER_INSTANCE_CELLS <= HELPER_INSTANCE_CELLS_MAX);
const _: () = assert!(HELPER_ABI_WORDS * WORD_ROWS + HELPER_INSTANCE_CELLS < (1 << 16));

#[derive(Clone, Debug)]
pub(super) struct OfflineCashHelperBindingConfigV1 {
    word: Column<Advice>,
    bit: Column<Advice>,
    accumulator: Column<Advice>,
    packed: Column<Advice>,
    lanes: [Column<Advice>; DIGEST_WORDS],
    instance: Column<Instance>,
    q_start: Selector,
    q_bit: Selector,
    q_word: Selector,
    q_operation: Selector,
    q_boolean: Selector,
    q_pack: Selector,
    q_exact_next: Selector,
    q_required_digest: Selector,
    q_android_digest: Selector,
    q_difference_start: Selector,
    q_difference_step: Selector,
    q_difference_terminal: Selector,
}

fn configure_helper_v1<F: PrimeField>(
    meta: &mut ConstraintSystem<F>,
) -> OfflineCashHelperBindingConfigV1 {
    assert!(
        F::CAPACITY >= PACKED_BITS,
        "Offline Cash helper 224-bit packing requires sufficient field capacity"
    );
    let word = meta.advice_column();
    let bit = meta.advice_column();
    let accumulator = meta.advice_column();
    let packed = meta.advice_column();
    let lanes: [Column<Advice>; DIGEST_WORDS] = std::array::from_fn(|_| meta.advice_column());
    let constant = meta.fixed_column();
    let instance = meta.instance_column();
    meta.enable_equality(word);
    meta.enable_equality(bit);
    meta.enable_equality(packed);
    for column in lanes {
        meta.enable_equality(column);
    }
    meta.enable_equality(instance);
    meta.enable_constant(constant);

    let q_start = meta.selector();
    meta.create_gate("offline cash helper u32 start", |meta| {
        let enabled = meta.query_selector(q_start);
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * accumulator]
    });

    let q_bit = meta.selector();
    meta.create_gate("offline cash helper u32 bit", |meta| {
        let enabled = meta.query_selector(q_bit);
        let bit = meta.query_advice(bit, Rotation::cur());
        let current = meta.query_advice(accumulator, Rotation::cur());
        let next = meta.query_advice(accumulator, Rotation::next());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![
            enabled.clone() * bit.clone() * (bit.clone() - one),
            enabled * (next - current * two - bit),
        ]
    });

    let q_word = meta.selector();
    meta.create_gate("offline cash helper reconstructed u32", |meta| {
        let enabled = meta.query_selector(q_word);
        let word = meta.query_advice(word, Rotation::cur());
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * (word - accumulator)]
    });

    let q_operation = meta.selector();
    meta.create_gate("offline cash helper closed operation", |meta| {
        let enabled = meta.query_selector(q_operation);
        let word = meta.query_advice(word, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![enabled * (word.clone() - one) * (word - two)]
    });

    let q_boolean = meta.selector();
    meta.create_gate("offline cash helper closed Android flag", |meta| {
        let enabled = meta.query_selector(q_boolean);
        let word = meta.query_advice(word, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        vec![enabled * word.clone() * (word - one)]
    });

    let q_pack = meta.selector();
    meta.create_gate("offline cash helper 7x32 little-endian pack", |meta| {
        let enabled = meta.query_selector(q_pack);
        let packed = meta.query_advice(packed, Rotation::cur());
        let radix = F::from(1_u64 << 32);
        let mut coefficient = F::ONE;
        let mut reconstructed = Expression::Constant(F::ZERO);
        for column in &lanes[..HELPER_WORDS_PER_INSTANCE] {
            reconstructed = reconstructed
                + meta.query_advice(*column, Rotation::cur()) * Expression::Constant(coefficient);
            coefficient *= radix;
        }
        vec![enabled * (packed - reconstructed)]
    });

    let q_exact_next = meta.selector();
    meta.create_gate("offline cash helper exact-next u64", |meta| {
        let enabled = meta.query_selector(q_exact_next);
        let from_low = meta.query_advice(lanes[0], Rotation::cur());
        let from_high = meta.query_advice(lanes[1], Rotation::cur());
        let to_low = meta.query_advice(lanes[2], Rotation::cur());
        let to_high = meta.query_advice(lanes[3], Rotation::cur());
        let carry = meta.query_advice(bit, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let radix = Expression::Constant(F::from(1_u64 << 32));
        vec![
            enabled.clone() * carry.clone() * (carry.clone() - one.clone()),
            enabled.clone() * (from_low + one - to_low - carry.clone() * radix),
            enabled * (from_high + carry - to_high),
        ]
    });

    let q_required_digest = meta.selector();
    meta.create_gate("offline cash helper required digest", |meta| {
        let enabled = meta.query_selector(q_required_digest);
        let inverse = meta.query_advice(accumulator, Rotation::cur());
        let sum = lanes[..DIGEST_WORDS]
            .iter()
            .fold(Expression::Constant(F::ZERO), |sum, column| {
                sum + meta.query_advice(*column, Rotation::cur())
            });
        vec![enabled * (sum * inverse - Expression::Constant(F::ONE))]
    });

    let q_android_digest = meta.selector();
    meta.create_gate("offline cash helper optional Android digest", |meta| {
        let enabled = meta.query_selector(q_android_digest);
        let present = meta.query_advice(bit, Rotation::cur());
        let inverse = meta.query_advice(accumulator, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let sum = lanes[..DIGEST_WORDS]
            .iter()
            .fold(Expression::Constant(F::ZERO), |sum, column| {
                sum + meta.query_advice(*column, Rotation::cur())
            });
        let mut constraints = vec![
            enabled.clone() * present.clone() * (present.clone() - one.clone()),
            enabled.clone() * (sum * inverse - present.clone()),
        ];
        constraints.extend(lanes[..DIGEST_WORDS].iter().map(|column| {
            enabled.clone()
                * (one.clone() - present.clone())
                * meta.query_advice(*column, Rotation::cur())
        }));
        constraints
    });

    let q_difference_start = meta.selector();
    meta.create_gate("offline cash helper digest difference start", |meta| {
        let enabled = meta.query_selector(q_difference_start);
        vec![enabled * meta.query_advice(accumulator, Rotation::cur())]
    });
    let q_difference_step = meta.selector();
    meta.create_gate(
        "offline cash helper digest difference accumulator",
        |meta| {
            let enabled = meta.query_selector(q_difference_step);
            let lhs = meta.query_advice(word, Rotation::cur());
            let rhs = meta.query_advice(bit, Rotation::cur());
            let current = meta.query_advice(accumulator, Rotation::cur());
            let next = meta.query_advice(accumulator, Rotation::next());
            let difference = lhs - rhs;
            vec![enabled * (next - current - difference.clone() * difference)]
        },
    );
    let q_difference_terminal = meta.selector();
    meta.create_gate("offline cash helper distinct digest terminal", |meta| {
        let enabled = meta.query_selector(q_difference_terminal);
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        let inverse = meta.query_advice(packed, Rotation::cur());
        vec![enabled * (accumulator * inverse - Expression::Constant(F::ONE))]
    });

    OfflineCashHelperBindingConfigV1 {
        word,
        bit,
        accumulator,
        packed,
        lanes,
        instance,
        q_start,
        q_bit,
        q_word,
        q_operation,
        q_boolean,
        q_pack,
        q_exact_next,
        q_required_digest,
        q_android_digest,
        q_difference_start,
        q_difference_step,
        q_difference_terminal,
    }
}

fn option_field<F: PrimeField>(value: Option<u64>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| Value::known(F::from(value)))
}

fn option_inverse<F: PrimeField>(value: Option<F>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| {
        Value::known(Option::<F>::from(value.invert()).unwrap_or(F::ZERO))
    })
}

fn synthesize_helper_v1<F: PrimeField>(
    words: Option<&[u32; HELPER_ABI_WORDS]>,
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
    config: OfflineCashHelperBindingConfigV1,
    mut layouter: impl Layouter<F>,
) -> Result<(), PlonkError> {
    let word_cells = layouter.assign_region(
        || "offline cash helper canonical u32 words",
        |mut region| {
            let mut cells = Vec::with_capacity(HELPER_ABI_WORDS);
            for word_index in 0..HELPER_ABI_WORDS {
                let base = word_index * WORD_ROWS;
                config.q_start.enable(&mut region, base)?;
                region.assign_advice(config.accumulator, base, Value::known(F::ZERO));
                let witness_word = words.map(|words| words[word_index]);
                let mut reconstructed = witness_word.map(|_| 0_u64);
                for bit_index in 0..U32_BITS {
                    let row = base + bit_index;
                    config.q_bit.enable(&mut region, row)?;
                    let witness_bit = witness_word
                        .map(|word| u64::from((word >> (U32_BITS - 1 - bit_index)) & 1));
                    region.assign_advice(config.bit, row, option_field::<F>(witness_bit));
                    reconstructed = reconstructed
                        .zip(witness_bit)
                        .map(|(accumulator, bit)| accumulator * 2 + bit);
                    region.assign_advice(
                        config.accumulator,
                        row + 1,
                        option_field::<F>(reconstructed),
                    );
                }
                let word_row = base + U32_BITS;
                config.q_word.enable(&mut region, word_row)?;
                if word_index == HELPER_OPERATION_WORD {
                    config.q_operation.enable(&mut region, word_row)?;
                }
                if word_index == HELPER_ANDROID_PRESENT_WORD {
                    config.q_boolean.enable(&mut region, word_row)?;
                }
                let cell = if let Some(constant) = fixed_helper_word_v1(parity, role, word_index) {
                    region
                        .assign_advice_from_constant(
                            || format!("fixed helper word {word_index}"),
                            config.word,
                            word_row,
                            F::from(u64::from(constant)),
                        )?
                        .cell()
                } else {
                    region
                        .assign_advice(
                            config.word,
                            word_row,
                            option_field::<F>(witness_word.map(u64::from)),
                        )
                        .cell()
                };
                cells.push(cell);
            }
            Ok(cells)
        },
    )?;

    layouter.assign_region(
        || "offline cash helper exact-next",
        |mut region| {
            config.q_exact_next.enable(&mut region, 0)?;
            for (lane, word_index) in [
                HELPER_FROM_LOW_WORD,
                HELPER_FROM_HIGH_WORD,
                HELPER_TO_LOW_WORD,
                HELPER_TO_HIGH_WORD,
            ]
            .into_iter()
            .enumerate()
            {
                let value = words.map(|words| u64::from(words[word_index]));
                let cell = region
                    .assign_advice(config.lanes[lane], 0, option_field::<F>(value))
                    .cell();
                region.constrain_equal(cell, word_cells[word_index]);
            }
            let carry = words.map(|words| u64::from(words[HELPER_FROM_LOW_WORD] == u32::MAX));
            region.assign_advice(config.bit, 0, option_field::<F>(carry));
            Ok(())
        },
    )?;

    layouter.assign_region(
        || "offline cash helper digest presence",
        |mut region| {
            let mut row = 0_usize;
            for offset in REQUIRED_DIGEST_OFFSETS {
                config.q_required_digest.enable(&mut region, row)?;
                let mut sum = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    let word_index = offset + lane;
                    let value = words.map(|words| u64::from(words[word_index]));
                    let cell = region
                        .assign_advice(config.lanes[lane], row, option_field::<F>(value))
                        .cell();
                    region.constrain_equal(cell, word_cells[word_index]);
                    sum = sum.zip(value).map(|(sum, value)| sum + F::from(value));
                }
                region.assign_advice(config.accumulator, row, option_inverse(sum));
                row += 1;
            }
            for offset in ANDROID_DIGEST_OFFSETS {
                config.q_android_digest.enable(&mut region, row)?;
                let present = words.map(|words| u64::from(words[HELPER_ANDROID_PRESENT_WORD]));
                let present_cell = region
                    .assign_advice(config.bit, row, option_field::<F>(present))
                    .cell();
                region.constrain_equal(present_cell, word_cells[HELPER_ANDROID_PRESENT_WORD]);
                let mut sum = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    let word_index = offset + lane;
                    let value = words.map(|words| u64::from(words[word_index]));
                    let cell = region
                        .assign_advice(config.lanes[lane], row, option_field::<F>(value))
                        .cell();
                    region.constrain_equal(cell, word_cells[word_index]);
                    sum = sum.zip(value).map(|(sum, value)| sum + F::from(value));
                }
                region.assign_advice(config.accumulator, row, option_inverse(sum));
                row += 1;
            }
            Ok(())
        },
    )?;

    for (label, lhs_offset, rhs_offset) in [
        (
            "offline cash helper current/next guard inequality",
            CURRENT_GUARD_WORD_START,
            NEXT_GUARD_WORD_START,
        ),
        (
            "offline cash helper current/transition inequality",
            CURRENT_HEAD_WORD_START,
            TRANSITION_WORD_START,
        ),
    ] {
        layouter.assign_region(
            || label,
            |mut region| {
                config.q_difference_start.enable(&mut region, 0)?;
                region.assign_advice(config.accumulator, 0, Value::known(F::ZERO));
                let mut running = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    config.q_difference_step.enable(&mut region, lane)?;
                    let lhs_index = lhs_offset + lane;
                    let rhs_index = rhs_offset + lane;
                    let lhs = words.map(|words| u64::from(words[lhs_index]));
                    let rhs = words.map(|words| u64::from(words[rhs_index]));
                    let lhs_cell = region
                        .assign_advice(config.word, lane, option_field::<F>(lhs))
                        .cell();
                    let rhs_cell = region
                        .assign_advice(config.bit, lane, option_field::<F>(rhs))
                        .cell();
                    region.constrain_equal(lhs_cell, word_cells[lhs_index]);
                    region.constrain_equal(rhs_cell, word_cells[rhs_index]);
                    running = running.zip(lhs.zip(rhs)).map(|(sum, (lhs, rhs))| {
                        let difference = F::from(lhs) - F::from(rhs);
                        sum + difference * difference
                    });
                    region.assign_advice(
                        config.accumulator,
                        lane + 1,
                        running.map_or_else(Value::unknown, Value::known),
                    );
                }
                config
                    .q_difference_terminal
                    .enable(&mut region, DIGEST_WORDS)?;
                region.assign_advice(config.packed, DIGEST_WORDS, option_inverse(running));
                Ok(())
            },
        )?;
    }

    let packed_cells = layouter.assign_region(
        || "offline cash helper canonical public cells",
        |mut region| {
            let mut cells = Vec::with_capacity(HELPER_INSTANCE_CELLS);
            for cell_index in 0..HELPER_INSTANCE_CELLS {
                config.q_pack.enable(&mut region, cell_index)?;
                let start = cell_index * HELPER_WORDS_PER_INSTANCE;
                let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
                for lane in 0..HELPER_WORDS_PER_INSTANCE {
                    let word_index = start + lane;
                    if word_index < end {
                        let value = words.map(|words| u64::from(words[word_index]));
                        let lane_cell = region
                            .assign_advice(config.lanes[lane], cell_index, option_field::<F>(value))
                            .cell();
                        region.constrain_equal(lane_cell, word_cells[word_index]);
                    } else {
                        region.assign_advice_from_constant(
                            || format!("zero helper padding lane {lane}"),
                            config.lanes[lane],
                            cell_index,
                            F::ZERO,
                        )?;
                    }
                }
                let packed = words.map(|words| pack_words_as_field::<F>(&words[start..end]));
                cells.push(
                    region
                        .assign_advice(
                            config.packed,
                            cell_index,
                            packed.map_or_else(Value::unknown, Value::known),
                        )
                        .cell(),
                );
            }
            Ok(cells)
        },
    )?;
    for (row, cell) in packed_cells.into_iter().enumerate() {
        layouter.constrain_instance(cell, config.instance, row);
    }
    Ok(())
}

macro_rules! define_helper_binding_circuit {
    ($name:ident, $field:ty, $parity:expr, $role:expr) => {
        pub(super) struct $name {
            words: Option<[u32; HELPER_ABI_WORDS]>,
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("has_public_words", &self.words.is_some())
                    .field("semantic_p256_verification", &"deferred")
                    .finish()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self { words: None }
            }
        }

        impl $name {
            pub(super) fn new(
                relation: &OfflineCashValidatedHelperRelationV1,
            ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
                let instances = relation.public_instances($parity, $role)?;
                Ok(Self {
                    words: Some(*instances.words()),
                })
            }

            #[cfg(test)]
            pub(super) fn from_words_for_test(words: [u32; HELPER_ABI_WORDS]) -> Self {
                Self { words: Some(words) }
            }
        }

        impl Circuit<$field> for $name {
            type Config = OfflineCashHelperBindingConfigV1;
            type FloorPlanner = SimpleFloorPlanner;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                Self::default()
            }

            fn configure(meta: &mut ConstraintSystem<$field>) -> Self::Config {
                configure_helper_v1(meta)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                synthesize_helper_v1(self.words.as_ref(), $parity, $role, config, layouter)
            }
        }
    };
}

define_helper_binding_circuit!(
    OfflineCashEqGuardUseBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
define_helper_binding_circuit!(
    OfflineCashEpGuardUseBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
define_helper_binding_circuit!(
    OfflineCashEqPlatformBindBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
define_helper_binding_circuit!(
    OfflineCashEpPlatformBindBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
define_helper_binding_circuit!(
    OfflineCashEqAndroidKeyCertBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
define_helper_binding_circuit!(
    OfflineCashEpAndroidKeyCertBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
define_helper_binding_circuit!(
    OfflineCashEqGuardBundleBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardBundle
);
define_helper_binding_circuit!(
    OfflineCashEpGuardBundleBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardBundle
);
