//! Exact Eq/Fp and Ep/Fq `StateLeaf` relation circuits.
//!
//! These fixed-shape circuits range-constrain every semantic `u32`, enforce the
//! exact header/parity/protocol/operation contract, bind the 93 semantic words to 14
//! canonical 224-bit cells in one public-instance column, enforce exact u128
//! send/receive conservation, constrain the three canonical private heads, and
//! close deterministic opening/transition/semantic hashing for `ReceiveFold`,
//! and constrain the deterministic seed, both branch openings, canonical
//! context, and exact Norito transition/semantic hashing for `SendSplit`.
//! The separate final `State` wrapper recursively authenticates this leaf and
//! owns the 136-word reciprocal-audit tail, avoiding a transcript fixed point.

use std::sync::Mutex;

use halo2_proofs::{
    circuit::{Layouter, V1, Value},
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
    OfflineCashHalo2ParityV1,
    state_abi::{
        OfflineCashStateAbiErrorV1, OfflineCashStateLeafPublicInstancesV1,
        STATE_INSTANCE_CELLS_MAX, STATE_LEAF_ABI_WORDS, STATE_LEAF_INSTANCE_CELLS,
        STATE_OPERATION_WORD, STATE_WORDS_PER_INSTANCE, fixed_state_word_v1, pack_words_as_field,
    },
    state_relation::{
        OfflineCashStatePrivateWitnessV1,
        circuit::{
            OfflineCashStateRelationConfigV1, configure_relation_v1, synthesize_relation_v1,
        },
    },
};

const U32_BITS: usize = 32;
const WORD_ROWS: usize = U32_BITS + 1;
const PACKED_BITS: u32 = (STATE_WORDS_PER_INSTANCE * U32_BITS) as u32;

const _: () = assert!(STATE_LEAF_INSTANCE_CELLS <= STATE_INSTANCE_CELLS_MAX);
const _: () = assert!(STATE_LEAF_ABI_WORDS * WORD_ROWS + STATE_LEAF_INSTANCE_CELLS < (1 << 16));

#[derive(Clone, Debug)]
pub(super) struct OfflineCashStateCircuitConfigV1 {
    word: Column<Advice>,
    bit: Column<Advice>,
    accumulator: Column<Advice>,
    packed: Column<Advice>,
    pack_lanes: [Column<Advice>; STATE_WORDS_PER_INSTANCE],
    instance: Column<Instance>,
    q_start: Selector,
    q_bit: Selector,
    q_word: Selector,
    q_operation: Selector,
    q_pack: Selector,
    relation: OfflineCashStateRelationConfigV1,
}

fn configure_state_v1<F: PrimeField>(
    meta: &mut ConstraintSystem<F>,
) -> OfflineCashStateCircuitConfigV1 {
    assert!(
        F::CAPACITY >= PACKED_BITS,
        "Offline Cash STATE 224-bit packing requires sufficient field capacity"
    );
    let word = meta.advice_column();
    let bit = meta.advice_column();
    let accumulator = meta.advice_column();
    let packed = meta.advice_column();
    let pack_lanes = std::array::from_fn(|_| meta.advice_column());
    let constant = meta.fixed_column();
    let instance = meta.instance_column();
    for column in pack_lanes {
        meta.enable_equality(column);
    }
    meta.enable_equality(word);
    meta.enable_equality(packed);
    meta.enable_equality(instance);
    meta.enable_constant(constant);

    let q_start = meta.selector();
    meta.create_gate("offline cash STATE u32 start", |meta| {
        let enabled = meta.query_selector(q_start);
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * accumulator]
    });

    let q_bit = meta.selector();
    meta.create_gate("offline cash STATE u32 bit", |meta| {
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
    meta.create_gate("offline cash STATE reconstructed u32", |meta| {
        let enabled = meta.query_selector(q_word);
        let word = meta.query_advice(word, Rotation::cur());
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * (word - accumulator)]
    });

    let q_operation = meta.selector();
    meta.create_gate("offline cash STATE closed operation", |meta| {
        let enabled = meta.query_selector(q_operation);
        let word = meta.query_advice(word, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![enabled * (word.clone() - one) * (word - two)]
    });

    let q_pack = meta.selector();
    meta.create_gate("offline cash STATE 7x32 little-endian pack", |meta| {
        let enabled = meta.query_selector(q_pack);
        let packed = meta.query_advice(packed, Rotation::cur());
        let radix = F::from(1_u64 << 32);
        let mut coefficient = F::ONE;
        let mut reconstructed = Expression::Constant(F::ZERO);
        for column in pack_lanes {
            reconstructed = reconstructed
                + meta.query_advice(column, Rotation::cur()) * Expression::Constant(coefficient);
            coefficient *= radix;
        }
        vec![enabled * (packed - reconstructed)]
    });

    OfflineCashStateCircuitConfigV1 {
        word,
        bit,
        accumulator,
        packed,
        pack_lanes,
        instance,
        q_start,
        q_bit,
        q_word,
        q_operation,
        q_pack,
        relation: configure_relation_v1(meta),
    }
}

fn option_field<F: PrimeField>(value: Option<u64>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| Value::known(F::from(value)))
}

fn synthesize_state_v1<F: PrimeField>(
    words: Option<&[u32; STATE_LEAF_ABI_WORDS]>,
    private_witness: Option<&OfflineCashStatePrivateWitnessV1>,
    parity: OfflineCashHalo2ParityV1,
    config: OfflineCashStateCircuitConfigV1,
    mut layouter: impl Layouter<F>,
) -> Result<(), PlonkError> {
    let word_cells = layouter.assign_region(
        || "offline cash STATE canonical u32 words",
        |mut region| {
            let mut cells = Vec::with_capacity(STATE_LEAF_ABI_WORDS);
            for word_index in 0..STATE_LEAF_ABI_WORDS {
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
                if word_index == STATE_OPERATION_WORD {
                    config.q_operation.enable(&mut region, word_row)?;
                }
                let cell = if let Some(constant) = fixed_state_word_v1(parity, word_index) {
                    region
                        .assign_advice_from_constant(
                            || format!("fixed STATE word {word_index}"),
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

    synthesize_relation_v1(
        words,
        private_witness,
        &config.relation,
        &word_cells,
        &mut layouter,
    )?;

    let packed_cells = layouter.assign_region(
        || "offline cash STATE canonical 224-bit public cells",
        |mut region| {
            let mut cells = Vec::with_capacity(STATE_LEAF_INSTANCE_CELLS);
            for cell_index in 0..STATE_LEAF_INSTANCE_CELLS {
                config.q_pack.enable(&mut region, cell_index)?;
                let start = cell_index * STATE_WORDS_PER_INSTANCE;
                let end = start
                    .saturating_add(STATE_WORDS_PER_INSTANCE)
                    .min(STATE_LEAF_ABI_WORDS);
                for lane in 0..STATE_WORDS_PER_INSTANCE {
                    let word_index = start + lane;
                    if word_index < end {
                        let value = words.map(|words| u64::from(words[word_index]));
                        let lane_cell = region
                            .assign_advice(
                                config.pack_lanes[lane],
                                cell_index,
                                option_field::<F>(value),
                            )
                            .cell();
                        region.constrain_equal(lane_cell, word_cells[word_index]);
                    } else {
                        region.assign_advice_from_constant(
                            || format!("zero padding lane {lane}"),
                            config.pack_lanes[lane],
                            cell_index,
                            F::ZERO,
                        )?;
                    }
                }
                let packed_value = words.map(|words| pack_words_as_field::<F>(&words[start..end]));
                let packed_cell = region
                    .assign_advice(
                        config.packed,
                        cell_index,
                        packed_value.map_or_else(Value::unknown, Value::known),
                    )
                    .cell();
                cells.push(packed_cell);
            }
            Ok(cells)
        },
    )?;
    for (row, cell) in packed_cells.into_iter().enumerate() {
        layouter.constrain_instance(cell, config.instance, row);
    }
    Ok(())
}

/// Exact Eq/Fp public-binding circuit for the `StateLeaf` role.
pub(super) struct OfflineCashEqStateLeafCircuitV1 {
    words: Option<[u32; STATE_LEAF_ABI_WORDS]>,
    private_witness: Mutex<Option<OfflineCashStatePrivateWitnessV1>>,
}

impl core::fmt::Debug for OfflineCashEqStateLeafCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEqStateLeafCircuitV1")
            .field("has_public_words", &self.words.is_some())
            .field("private_witness", &"[REDACTED]")
            .finish()
    }
}

impl Default for OfflineCashEqStateLeafCircuitV1 {
    fn default() -> Self {
        Self {
            words: None,
            private_witness: Mutex::new(None),
        }
    }
}

impl OfflineCashEqStateLeafCircuitV1 {
    pub(super) fn new(
        instances: OfflineCashStateLeafPublicInstancesV1,
        private_witness: OfflineCashStatePrivateWitnessV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        if instances.parity() != OfflineCashHalo2ParityV1::Eq {
            return Err(OfflineCashStateAbiErrorV1::ParityMismatch);
        }
        private_witness.validate_against_leaf(&instances)?;
        Ok(Self {
            words: Some(*instances.words()),
            private_witness: Mutex::new(Some(private_witness)),
        })
    }

    #[cfg(test)]
    pub(super) fn from_words_for_test(
        words: [u32; STATE_LEAF_ABI_WORDS],
        private_witness: OfflineCashStatePrivateWitnessV1,
    ) -> Self {
        Self {
            words: Some(words),
            private_witness: Mutex::new(Some(private_witness)),
        }
    }

    #[cfg(test)]
    pub(super) fn has_witness_for_test(&self) -> bool {
        self.words.is_some()
            && self
                .private_witness
                .lock()
                .is_ok_and(|witness| witness.is_some())
    }
}

impl Circuit<Fp> for OfflineCashEqStateLeafCircuitV1 {
    type Config = OfflineCashStateCircuitConfigV1;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        configure_state_v1(meta)
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<Fp>,
    ) -> Result<(), PlonkError> {
        // V1 is two-pass. Never take the move-only real witness while it is
        // only measuring region shapes.
        Self::default().synthesize(config, layouter)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<Fp>,
    ) -> Result<(), PlonkError> {
        let private_witness = self
            .private_witness
            .lock()
            .map_err(|_| PlonkError::Synthesis)?
            .take();
        synthesize_state_v1(
            self.words.as_ref(),
            private_witness.as_ref(),
            OfflineCashHalo2ParityV1::Eq,
            config,
            layouter,
        )
    }
}

/// Exact Ep/Fq public-binding circuit for the `StateLeaf` role.
pub(super) struct OfflineCashEpStateLeafCircuitV1 {
    words: Option<[u32; STATE_LEAF_ABI_WORDS]>,
    private_witness: Mutex<Option<OfflineCashStatePrivateWitnessV1>>,
}

impl core::fmt::Debug for OfflineCashEpStateLeafCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEpStateLeafCircuitV1")
            .field("has_public_words", &self.words.is_some())
            .field("private_witness", &"[REDACTED]")
            .finish()
    }
}

impl Default for OfflineCashEpStateLeafCircuitV1 {
    fn default() -> Self {
        Self {
            words: None,
            private_witness: Mutex::new(None),
        }
    }
}

impl OfflineCashEpStateLeafCircuitV1 {
    pub(super) fn new(
        instances: OfflineCashStateLeafPublicInstancesV1,
        private_witness: OfflineCashStatePrivateWitnessV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        if instances.parity() != OfflineCashHalo2ParityV1::Ep {
            return Err(OfflineCashStateAbiErrorV1::ParityMismatch);
        }
        private_witness.validate_against_leaf(&instances)?;
        Ok(Self {
            words: Some(*instances.words()),
            private_witness: Mutex::new(Some(private_witness)),
        })
    }

    #[cfg(test)]
    pub(super) fn from_words_for_test(
        words: [u32; STATE_LEAF_ABI_WORDS],
        private_witness: OfflineCashStatePrivateWitnessV1,
    ) -> Self {
        Self {
            words: Some(words),
            private_witness: Mutex::new(Some(private_witness)),
        }
    }

    #[cfg(test)]
    pub(super) fn has_witness_for_test(&self) -> bool {
        self.words.is_some()
            && self
                .private_witness
                .lock()
                .is_ok_and(|witness| witness.is_some())
    }
}

impl Circuit<Fq> for OfflineCashEpStateLeafCircuitV1 {
    type Config = OfflineCashStateCircuitConfigV1;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
        configure_state_v1(meta)
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<Fq>,
    ) -> Result<(), PlonkError> {
        // V1 is two-pass. Never take the move-only real witness while it is
        // only measuring region shapes.
        Self::default().synthesize(config, layouter)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<Fq>,
    ) -> Result<(), PlonkError> {
        let private_witness = self
            .private_witness
            .lock()
            .map_err(|_| PlonkError::Synthesis)?
            .take();
        synthesize_state_v1(
            self.words.as_ref(),
            private_witness.as_ref(),
            OfflineCashHalo2ParityV1::Ep,
            config,
            layouter,
        )
    }
}
