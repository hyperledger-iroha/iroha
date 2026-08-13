// This file is part of MIDNIGHT-ZK.
// Copyright (C) 2025 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// You may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use std::marker::PhantomData;
use ff::PrimeField;
use halo2_proofs::{
    circuit::{Chip, Layouter, Region, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Fixed, Selector},
    poly::Rotation,
};
mod compression;
mod gates;
mod message_schedule;
mod spread_table;
pub(crate) mod util;
use compression::*;
use gates::*;
use message_schedule::*;
use spread_table::*;
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedBits, AssignedBlockWord, BlockWord, PaddedByte, Sha256Instructions,
};
#[derive(Clone, Debug)]
struct PackConfig {
    bytes: [Column<Advice>; 4],
    word: Column<Advice>,
    s_pack: Selector,
}
impl PackConfig {
    fn configure<F: PrimeField>(
        meta: &mut ConstraintSystem<F>,
        bytes: [Column<Advice>; 4],
        word: Column<Advice>,
    ) -> Self {
        let s_pack = meta.selector();
        meta.create_gate("pack four big-endian bytes into a SHA-256 word", |meta| {
            let q = meta.query_selector(s_pack);
            let [b0, b1, b2, b3] = bytes.map(|column| meta.query_advice(column, Rotation::cur()));
            let word = meta.query_advice(word, Rotation::cur());
            let two_8 = Expression::Constant(F::from(1 << 8));
            let two_16 = Expression::Constant(F::from(1 << 16));
            let two_24 = Expression::Constant(F::from(1 << 24));
            vec![q * (b0 * two_24 + b1 * two_16 + b2 * two_8 + b3 - word)]
        });
        Self {
            bytes,
            word,
            s_pack,
        }
    }
    fn assign_block<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        block: [PaddedByte<F>; super::BLOCK_BYTE_SIZE],
        block_index: usize,
    ) -> Result<[AssignedBlockWord<F>; super::BLOCK_SIZE], Error> {
        layouter.assign_region(
            || format!("pack canonical SHA-256 block {block_index}"),
            |mut region| {
                let mut words = Vec::with_capacity(super::BLOCK_SIZE);
                for (word_index, bytes) in block.chunks_exact(4).enumerate() {
                    self.s_pack.enable(&mut region, word_index)?;
                    let mut value = Value::known(0_u32);
                    for (byte_index, byte) in bytes.iter().enumerate() {
                        value = value
                            .zip(byte.value())
                            .map(|(word, byte)| (word << 8) | u32::from(byte));
                        match byte {
                            PaddedByte::Source(source) => {
                                let assigned = region.assign_advice(
                                    self.bytes[byte_index],
                                    word_index,
                                    source.value.map(|byte| F::from(u64::from(byte))),
                                );
                                region.constrain_equal(assigned.cell(), source.cell);
                            }
                            PaddedByte::Constant(byte) => {
                                region.assign_advice_from_constant(
                                    || format!("padding byte {byte_index} of word {word_index}"),
                                    self.bytes[byte_index],
                                    word_index,
                                    F::from(u64::from(*byte)),
                                )?;
                            }
                        }
                    }
                    words.push(AssignedBits::<32, F>::assign(
                        &mut region,
                        || format!("packed word {word_index}"),
                        self.word,
                        word_index,
                        value,
                    )?);
                }
                words.try_into().map_err(|_| Error::Synthesis)
            },
        )
    }
}
/// Configuration for a [`Table16Chip`].
#[derive(Clone, Debug)]
pub struct Table16Config {
    lookup: SpreadTableConfig,
    message_schedule: MessageScheduleConfig,
    compression: CompressionConfig,
    pack: PackConfig,
}
#[derive(Clone, Debug)]
struct Table16SharedConfig {
    lookup: SpreadTable,
    constant: Column<Fixed>,
}
/// A chip that implements SHA-256 with a maximum lookup table size of $2^16$.
#[derive(Clone, Debug)]
pub struct Table16Chip<F: PrimeField> {
    config: Table16Config,
    _marker: PhantomData<F>,
}
impl<F: PrimeField> Chip<F> for Table16Chip<F> {
    type Config = Table16Config;
    type Loaded = ();
    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}
impl<F: PrimeField> Table16Chip<F> {
    fn assert_field_size() {
        assert!(
            F::NUM_BITS >= 65,
            "Table16 SHA-256 requires a field modulus larger than 2^64 - 1"
        );
    }
    /// Reconstructs this chip from the given config.
    pub fn construct(config: <Self as Chip<F>>::Config) -> Self {
        Self::assert_field_size();
        Self {
            config,
            _marker: PhantomData,
        }
    }
    fn configure_shared(meta: &mut ConstraintSystem<F>) -> Table16SharedConfig {
        let lookup = SpreadTableChip::<F>::configure_table(meta);
        let constant = meta.fixed_column();
        meta.enable_constant(constant);
        Table16SharedConfig { lookup, constant }
    }
    fn configure_lane(
        meta: &mut ConstraintSystem<F>,
        shared: &Table16SharedConfig,
        advice: [Column<Advice>; 10],
        lookup_tail: Column<Advice>,
    ) -> <Self as Chip<F>>::Config {
        Self::assert_field_size();
        let [
            input_tag,
            input_dense,
            input_spread,
            a_3,
            a_4,
            message_schedule_column,
            a_6,
            a_7,
            a_8,
            a_9,
        ] = advice;
        let extras = [a_3, a_4, a_6, a_7, a_8, a_9];
        let lookup = SpreadTableChip::configure_with_table(
            meta,
            input_tag,
            input_dense,
            input_spread,
            lookup_tail,
            shared.lookup.clone(),
        );
        let lookup_inputs = lookup.input.clone();
        let a_1 = lookup_inputs.dense;
        let a_2 = lookup_inputs.spread;
        for column in [a_1, a_2, a_3, a_4, message_schedule_column, a_6, a_7, a_8] {
            meta.enable_equality(column);
        }
        let _ = shared.constant;
        let compression = CompressionConfig::configure(
            meta,
            lookup_inputs.clone(),
            message_schedule_column,
            extras,
        );
        let message_schedule =
            MessageScheduleConfig::configure(meta, lookup_inputs, message_schedule_column, extras);
        let pack = PackConfig::configure(meta, [a_3, a_4, a_6, a_7], message_schedule_column);
        Table16Config {
            lookup,
            message_schedule,
            compression,
            pack,
        }
    }
    /// Configure several independent Table16 lanes that share exactly one
    /// three-column spread table and one fixed constant column.
    pub(crate) fn configure_lanes<const LANES: usize>(
        meta: &mut ConstraintSystem<F>,
    ) -> [<Self as Chip<F>>::Config; LANES] {
        Self::assert_field_size();
        let shared = Self::configure_shared(meta);
        std::array::from_fn(|_| {
            let advice = std::array::from_fn(|_| meta.advice_column());
            let lookup_tail = meta.advice_column();
            Self::configure_lane(meta, &shared, advice, lookup_tail)
        })
    }
    /// Configures a circuit to include this chip.
    #[cfg(test)]
    pub fn configure(meta: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
        Self::configure_lanes::<1>(meta)
            .into_iter()
            .next()
            .expect("one Table16 lane")
    }
    /// Copy-binds range-checked source bytes into canonical, padded SHA-256
    /// blocks and constrains their big-endian packing into 32-bit words.
    #[cfg(test)]
    pub(crate) fn canonical_blocks(
        &self,
        layouter: &mut impl Layouter<F>,
        input: &[super::AssignedByte<F>],
    ) -> Result<Vec<[AssignedBlockWord<F>; super::BLOCK_SIZE]>, Error> {
        let suffix = super::canonical_padding_suffix(input.len()).ok_or(Error::Synthesis)?;
        let padded_len = input
            .len()
            .checked_add(suffix.len())
            .ok_or(Error::Synthesis)?;
        if padded_len % super::BLOCK_BYTE_SIZE != 0 {
            return Err(Error::Synthesis);
        }
        let mut padded = Vec::new();
        padded
            .try_reserve_exact(padded_len)
            .map_err(|_| Error::Synthesis)?;
        padded.extend(input.iter().cloned().map(PaddedByte::Source));
        padded.extend(suffix.into_iter().map(PaddedByte::Constant));
        debug_assert_eq!(
            padded
                .iter()
                .filter(|byte| matches!(byte, PaddedByte::Source(_)))
                .count(),
            input.len()
        );
        let mut blocks = Vec::with_capacity(padded_len / super::BLOCK_BYTE_SIZE);
        for (block_index, block) in padded.chunks_exact(super::BLOCK_BYTE_SIZE).enumerate() {
            let block: [PaddedByte<F>; super::BLOCK_BYTE_SIZE] =
                block.to_vec().try_into().map_err(|_| Error::Synthesis)?;
            blocks.push(
                self.config
                    .pack
                    .assign_block(layouter, block, block_index)?,
            );
        }
        Ok(blocks)
    }
    pub(crate) fn assign_padded_block(
        &self,
        layouter: &mut impl Layouter<F>,
        block: [PaddedByte<F>; super::BLOCK_BYTE_SIZE],
        block_index: usize,
    ) -> Result<[AssignedBlockWord<F>; super::BLOCK_SIZE], Error> {
        self.config.pack.assign_block(layouter, block, block_index)
    }
    /// Loads the lookup table required by this chip into the circuit.
    pub fn load(config: Table16Config, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        SpreadTableChip::load(config.lookup, layouter)
    }
}
impl<F: PrimeField> Sha256Instructions<F> for Table16Chip<F> {
    type State = State<F>;
    fn initialization_vector(&self, layouter: &mut impl Layouter<F>) -> Result<Self::State, Error> {
        self.config().compression.initialize_with_iv(layouter)
    }
    // Given a chaining state and an input message block, copy-decompose the
    // state, compress the message block, and return the final state.
    // The values of the blockword array are re-assigned to satisfy the satisfy the
    // message schedule constraint and then they are copy constrainted to ensure
    // the newly assigned values are equal to the ones given as input
    //
    // Panics if `input` contains Assign values that do not convert to u32 (i.e. the
    // field element representation should be exactly 4 bytes, the rest being zero).
    fn compress(
        &self,
        layouter: &mut impl Layouter<F>,
        chaining_state: &Self::State,
        input: [AssignedBlockWord<F>; super::BLOCK_SIZE],
    ) -> Result<Self::State, Error> {
        let config = self.config();
        let lookup_inputs = &config.lookup.input;
        // Every block initializes itself. This makes the constraints
        // independent of how a streaming caller chunks update() calls and
        // prevents raw feed-forward A/E words from bypassing decomposition.
        let initialized_state = config
            .compression
            .initialize_with_state(layouter, chaining_state.clone())?;
        // extract the values that need to be input in `process`
        let input_values = input.clone().map(|word| BlockWord(word.value_u32()));
        // the output is well formed due to the constraints in `process`. The w values
        // are therefore rangechecked
        // assign the values for message schedule. Note that at these point the values
        // used are arbitrary and not-connected to the assigned input
        let (w, w_halves) = config.message_schedule.process(layouter, input_values)?;
        // here we make the connection with the input. Specifically, we assert that the
        // first 16 values returned by message schedule that represent the 16
        // 32-bit input words to be absorbed are equal with the assigned input
        // as field elements
        layouter.assign_region(
            || "Assert equality of input",
            |mut region| {
                for (w, input) in w[0..16].iter().zip(input.iter()) {
                    // Since w is already rangechecked, input is also in the appropriate range
                    region.constrain_equal(w.0.cell(), input.cell());
                }
                Ok(())
            },
        )?;
        config
            .compression
            .compress(layouter, initialized_state, w_halves, lookup_inputs)
    }
    fn digest(
        &self,
        layouter: &mut impl Layouter<F>,
        state: &Self::State,
    ) -> Result<[AssignedBlockWord<F>; super::DIGEST_SIZE], Error> {
        // Copy the dense forms of the state variable chunks down to this gate.
        // Reconstruct the 32-bit dense words.
        self.config().compression.digest(layouter, state.clone())
    }
}
/// Common assignment patterns used by Table16 regions.
trait Table16Assignment<F: PrimeField> {
    /// Assign cells for general spread computation used in sigma, ch, ch_neg,
    /// maj gates
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    fn assign_spread_outputs(
        &self,
        region: &mut Region<'_, F>,
        lookup: &SpreadInputs,
        a_3: Column<Advice>,
        row: usize,
        r_0_even: Value<[bool; 16]>,
        r_0_odd: Value<[bool; 16]>,
        r_1_even: Value<[bool; 16]>,
        r_1_odd: Value<[bool; 16]>,
    ) -> Result<
        (
            (AssignedBits<16, F>, AssignedBits<16, F>),
            (AssignedBits<16, F>, AssignedBits<16, F>),
        ),
        Error,
    > {
        // Lookup R_0^{even}, R_0^{odd}, R_1^{even}, R_1^{odd}
        let r_0_even = SpreadVar::with_lookup(
            region,
            lookup,
            row - 1,
            r_0_even.map(SpreadWord::<16, 32>::new),
        )?;
        let r_0_odd =
            SpreadVar::with_lookup(region, lookup, row, r_0_odd.map(SpreadWord::<16, 32>::new))?;
        let r_1_even = SpreadVar::with_lookup(
            region,
            lookup,
            row + 1,
            r_1_even.map(SpreadWord::<16, 32>::new),
        )?;
        let r_1_odd = SpreadVar::with_lookup(
            region,
            lookup,
            row + 2,
            r_1_odd.map(SpreadWord::<16, 32>::new),
        )?;
        // Assign and copy R_1^{odd}
        r_1_odd
            .spread
            .copy_advice(|| "Assign and copy R_1^{odd}", region, a_3, row)?;
        Ok((
            (r_0_even.dense, r_1_even.dense),
            (r_0_odd.dense, r_1_odd.dense),
        ))
    }
    /// Assign outputs of sigma gates
    #[allow(clippy::too_many_arguments)]
    fn assign_sigma_outputs(
        &self,
        region: &mut Region<'_, F>,
        lookup: &SpreadInputs,
        a_3: Column<Advice>,
        row: usize,
        r_0_even: Value<[bool; 16]>,
        r_0_odd: Value<[bool; 16]>,
        r_1_even: Value<[bool; 16]>,
        r_1_odd: Value<[bool; 16]>,
    ) -> Result<(AssignedBits<16, F>, AssignedBits<16, F>), Error> {
        let (even, _odd) = self.assign_spread_outputs(
            region, lookup, a_3, row, r_0_even, r_0_odd, r_1_even, r_1_odd,
        )?;
        Ok(even)
    }
}
#[cfg(test)]
mod constraint_inventory_tests {
    use halo2_proofs::{halo2curves::pasta::Fp, plonk::ConstraintSystem};
    use super::Table16Chip;
    #[test]
    fn five_lane_tail_relation_adds_no_fixed_selector_or_permutation_columns() {
        let mut meta = ConstraintSystem::<Fp>::default();
        let _ = Table16Chip::<Fp>::configure_lanes::<5>(&mut meta);
        assert_eq!(meta.num_advice_columns(), 55);
        assert_eq!(meta.num_fixed_columns(), 4);
        assert_eq!(meta.num_instance_columns(), 0);
        assert_eq!(meta.num_selectors(), 110);
        assert_eq!(meta.permutation().get_columns().len(), 41);
        assert_eq!(meta.lookups().len(), 20);
        assert_eq!(meta.degree(), 9);
    }
}
