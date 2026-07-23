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

use ff::PrimeField;
use halo2_proofs::{
    circuit::Layouter,
    plonk::{Advice, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

use super::{super::BLOCK_SIZE, SpreadInputs, Table16Assignment};

mod schedule_gates;
mod schedule_util;
mod subregion1;
mod subregion2;
mod subregion3;

use schedule_gates::ScheduleGate;
#[cfg(test)]
pub(crate) use schedule_util::msg_schedule_test_input;
use schedule_util::*;

use crate::zk::kagemusha_sha256_table16_v4::{AssignedBits, BlockWord, ROUNDS};

#[derive(Clone, Debug)]
pub(crate) struct MessageWord<F: PrimeField>(pub(super) AssignedBits<32, F>);

impl<F: PrimeField> std::ops::Deref for MessageWord<F> {
    type Target = AssignedBits<32, F>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub(super) struct MessageScheduleConfig {
    lookup: SpreadInputs,
    message_schedule: Column<Advice>,
    extras: [Column<Advice>; 6],

    /// Construct a word using reduce_4.
    s_word: Selector,
    /// Decomposition gate for W_0, W_62, W_63.
    s_decompose_0: Selector,
    /// Decomposition gate for W_[1..14]
    s_decompose_1: Selector,
    /// Decomposition gate for W_[14..49]
    s_decompose_2: Selector,
    /// Decomposition gate for W_[49..62]
    s_decompose_3: Selector,
    /// sigma_0 gate for W_[1..14]
    s_lower_sigma_0: Selector,
    /// sigma_1 gate for W_[49..62]
    s_lower_sigma_1: Selector,
    /// sigma_0_v2 gate for W_[14..49]
    s_lower_sigma_0_v2: Selector,
    /// sigma_1_v2 gate for W_[14..49]
    s_lower_sigma_1_v2: Selector,
}

impl<F: PrimeField> Table16Assignment<F> for MessageScheduleConfig {}

impl MessageScheduleConfig {
    /// Configures the message schedule.
    ///
    /// `message_schedule` is the column into which the message schedule will be
    /// placed. The caller must create appropriate permutations in order to
    /// load schedule words into the compression rounds.
    ///
    /// `extras` contains columns that the message schedule will only use for
    /// internal gates, and will not place any constraints on (such as
    /// lookup constraints) outside itself.
    #[allow(clippy::many_single_char_names)]
    pub(super) fn configure<F: PrimeField>(
        meta: &mut ConstraintSystem<F>,
        lookup: SpreadInputs,
        message_schedule: Column<Advice>,
        extras: [Column<Advice>; 6],
    ) -> Self {
        // Create fixed columns for the selectors we will require.
        let s_word = meta.selector();
        let s_decompose_0 = meta.selector();
        let s_decompose_1 = meta.selector();
        let s_decompose_2 = meta.selector();
        let s_decompose_3 = meta.selector();
        let s_lower_sigma_0 = meta.selector();
        let s_lower_sigma_1 = meta.selector();
        let s_lower_sigma_0_v2 = meta.selector();
        let s_lower_sigma_1_v2 = meta.selector();

        // Rename these here for ease of matching the gates to the specification.
        let a_0 = lookup.tag;
        let a_1 = lookup.dense;
        let a_2 = lookup.spread;
        let a_3 = extras[0];
        let a_4 = extras[1];
        let a_5 = message_schedule;
        let a_6 = extras[2];
        let a_7 = extras[3];
        let a_8 = extras[4];
        let a_9 = extras[5];

        // s_word for W_[16..64]
        meta.create_gate("s_word for W_[16..64]", |meta| {
            let s_word = meta.query_selector(s_word);

            let sigma_0_lo = meta.query_advice(a_6, Rotation::prev());
            let sigma_0_hi = meta.query_advice(a_6, Rotation::cur());

            let sigma_1_lo = meta.query_advice(a_7, Rotation::prev());
            let sigma_1_hi = meta.query_advice(a_7, Rotation::cur());

            let w_minus_7_lo = meta.query_advice(a_8, Rotation::prev());
            let w_minus_7_hi = meta.query_advice(a_8, Rotation::cur());

            let w_minus_16_lo = meta.query_advice(a_3, Rotation::prev());
            let w_minus_16_hi = meta.query_advice(a_4, Rotation::prev());

            let word = meta.query_advice(a_5, Rotation::cur());
            let carry = meta.query_advice(a_9, Rotation::cur());

            ScheduleGate::s_word(
                s_word,
                sigma_0_lo,
                sigma_0_hi,
                sigma_1_lo,
                sigma_1_hi,
                w_minus_7_lo,
                w_minus_7_hi,
                w_minus_16_lo,
                w_minus_16_hi,
                word,
                carry,
            )
        });

        // s_decompose_0 for all words
        meta.create_gate("s_decompose_0", |meta| {
            let s_decompose_0 = meta.query_selector(s_decompose_0);
            let lo = meta.query_advice(a_3, Rotation::cur());
            let hi = meta.query_advice(a_4, Rotation::cur());
            let word = meta.query_advice(a_5, Rotation::cur());

            ScheduleGate::s_decompose_0(s_decompose_0, lo, hi, word)
        });

        // s_decompose_1 for W_[1..14]
        // (3, 4, 11, 14)-bit chunks
        meta.create_gate("s_decompose_1", |meta| {
            let s_decompose_1 = meta.query_selector(s_decompose_1);
            let a = meta.query_advice(a_3, Rotation::next()); // 3-bit chunk
            let b = meta.query_advice(a_4, Rotation::next()); // 4-bit chunk
            let c = meta.query_advice(a_1, Rotation::next()); // 11-bit chunk
            let tag_c = meta.query_advice(a_0, Rotation::next());
            let d = meta.query_advice(a_1, Rotation::cur()); // 14-bit chunk
            let tag_d = meta.query_advice(a_0, Rotation::cur());
            let word = meta.query_advice(a_5, Rotation::cur());

            ScheduleGate::s_decompose_1(s_decompose_1, a, b, c, tag_c, d, tag_d, word)
        });

        // s_decompose_2 for W_[14..49]
        // (3, 4, 3, 7, 1, 1, 13)-bit chunks
        meta.create_gate("s_decompose_2", |meta| {
            let s_decompose_2 = meta.query_selector(s_decompose_2);
            let a = meta.query_advice(a_3, Rotation::prev()); // 3-bit chunk
            let b = meta.query_advice(a_1, Rotation::next()); // 4-bit chunk
            let tag_b = meta.query_advice(a_0, Rotation::next());
            let c = meta.query_advice(a_4, Rotation::prev()); // 3-bit chunk
            let d = meta.query_advice(a_1, Rotation::cur()); // 7-bit chunk
            let tag_d = meta.query_advice(a_0, Rotation::cur());
            let e = meta.query_advice(a_3, Rotation::next()); // 1-bit chunk
            let f = meta.query_advice(a_4, Rotation::next()); // 1-bit chunk
            let g = meta.query_advice(a_1, Rotation::prev()); // 13-bit chunk
            let tag_g = meta.query_advice(a_0, Rotation::prev());
            let word = meta.query_advice(a_5, Rotation::cur());

            ScheduleGate::s_decompose_2(
                s_decompose_2,
                a,
                b,
                tag_b,
                c,
                d,
                tag_d,
                e,
                f,
                g,
                tag_g,
                word,
            )
        });

        // s_decompose_3 for W_49 to W_61
        // (10, 7, 2, 13)-bit chunks
        meta.create_gate("s_decompose_3", |meta| {
            let s_decompose_3 = meta.query_selector(s_decompose_3);
            let a = meta.query_advice(a_1, Rotation::next()); // 10-bit chunk
            let tag_a = meta.query_advice(a_0, Rotation::next());
            let b = meta.query_advice(a_4, Rotation::next()); // 7-bit chunk
            let c = meta.query_advice(a_3, Rotation::next()); // 2-bit chunk
            let d = meta.query_advice(a_1, Rotation::cur()); // 13-bit chunk
            let tag_d = meta.query_advice(a_0, Rotation::cur());
            let word = meta.query_advice(a_5, Rotation::cur());

            ScheduleGate::s_decompose_3(s_decompose_3, a, tag_a, b, c, d, tag_d, word)
        });

        // sigma_0 v1 on W_[1..14]
        // (3, 4, 11, 14)-bit chunks
        meta.create_gate("sigma_0 v1", |meta| {
            ScheduleGate::s_lower_sigma_0(
                meta.query_selector(s_lower_sigma_0),
                meta.query_advice(a_2, Rotation::prev()), // spread_r0_even
                meta.query_advice(a_2, Rotation::cur()),  // spread_r0_odd
                meta.query_advice(a_2, Rotation::next()), // spread_r1_even
                meta.query_advice(a_3, Rotation::cur()),  // spread_r1_odd
                meta.query_advice(a_5, Rotation::next()), // a
                meta.query_advice(a_6, Rotation::next()), // spread_a
                meta.query_advice(a_6, Rotation::cur()),  // b
                meta.query_advice(a_3, Rotation::prev()), // b_lo
                meta.query_advice(a_4, Rotation::prev()), // spread_b_lo
                meta.query_advice(a_5, Rotation::prev()), // b_hi
                meta.query_advice(a_6, Rotation::prev()), // spread_b_hi
                meta.query_advice(a_4, Rotation::cur()),  // spread_c
                meta.query_advice(a_5, Rotation::cur()),  // spread_d
            )
        });

        // sigma_0 v2 on W_[14..49]
        // (3, 4, 3, 7, 1, 1, 13)-bit chunks
        meta.create_gate("sigma_0 v2", |meta| {
            ScheduleGate::s_lower_sigma_0_v2(
                meta.query_selector(s_lower_sigma_0_v2),
                meta.query_advice(a_2, Rotation::prev()), // spread_r0_even
                meta.query_advice(a_2, Rotation::cur()),  // spread_r0_odd
                meta.query_advice(a_2, Rotation::next()), // spread_r1_even
                meta.query_advice(a_3, Rotation::cur()),  // spread_r1_odd
                meta.query_advice(a_3, Rotation::next()), // a
                meta.query_advice(a_4, Rotation::next()), // spread_a
                meta.query_advice(a_6, Rotation::cur()),  // b
                meta.query_advice(a_3, Rotation::prev()), // b_lo
                meta.query_advice(a_4, Rotation::prev()), // spread_b_lo
                meta.query_advice(a_5, Rotation::prev()), // b_hi
                meta.query_advice(a_6, Rotation::prev()), // spread_b_hi
                meta.query_advice(a_5, Rotation::next()), // c
                meta.query_advice(a_6, Rotation::next()), // spread_c
                meta.query_advice(a_4, Rotation::cur()),  // spread_d
                meta.query_advice(a_7, Rotation::cur()),  // spread_e
                meta.query_advice(a_7, Rotation::next()), // spread_f
                meta.query_advice(a_5, Rotation::cur()),  // spread_g
            )
        });

        // sigma_1 v2 on W_14 to W_48
        // (3, 4, 3, 7, 1, 1, 13)-bit chunks
        meta.create_gate("sigma_1 v2", |meta| {
            ScheduleGate::s_lower_sigma_1_v2(
                meta.query_selector(s_lower_sigma_1_v2),
                meta.query_advice(a_2, Rotation::prev()), // spread_r0_even
                meta.query_advice(a_2, Rotation::cur()),  // spread_r0_odd
                meta.query_advice(a_2, Rotation::next()), // spread_r1_even
                meta.query_advice(a_3, Rotation::cur()),  // spread_r1_odd
                meta.query_advice(a_3, Rotation::next()), // a
                meta.query_advice(a_4, Rotation::next()), // spread_a
                meta.query_advice(a_6, Rotation::cur()),  // b
                meta.query_advice(a_3, Rotation::prev()), // b_lo
                meta.query_advice(a_4, Rotation::prev()), // spread_b_lo
                meta.query_advice(a_5, Rotation::prev()), // b_hi
                meta.query_advice(a_6, Rotation::prev()), // spread_b_hi
                meta.query_advice(a_5, Rotation::next()), // c
                meta.query_advice(a_6, Rotation::next()), // spread_c
                meta.query_advice(a_4, Rotation::cur()),  // spread_d
                meta.query_advice(a_7, Rotation::cur()),  // spread_e
                meta.query_advice(a_7, Rotation::next()), // spread_f
                meta.query_advice(a_5, Rotation::cur()),  // spread_g
            )
        });

        // sigma_1 v1 on W_49 to W_61
        // (10, 7, 2, 13)-bit chunks
        meta.create_gate("sigma_1 v1", |meta| {
            ScheduleGate::s_lower_sigma_1(
                meta.query_selector(s_lower_sigma_1),
                meta.query_advice(a_2, Rotation::prev()), // spread_r0_even
                meta.query_advice(a_2, Rotation::cur()),  // spread_r0_odd
                meta.query_advice(a_2, Rotation::next()), // spread_r1_even
                meta.query_advice(a_3, Rotation::cur()),  // spread_r1_odd
                meta.query_advice(a_4, Rotation::cur()),  // spread_a
                meta.query_advice(a_6, Rotation::cur()),  // b
                meta.query_advice(a_3, Rotation::prev()), // b_lo
                meta.query_advice(a_4, Rotation::prev()), // spread_b_lo
                meta.query_advice(a_5, Rotation::prev()), // b_mid
                meta.query_advice(a_6, Rotation::prev()), // spread_b_mid
                meta.query_advice(a_5, Rotation::next()), // b_hi
                meta.query_advice(a_6, Rotation::next()), // spread_b_hi
                meta.query_advice(a_3, Rotation::next()), // c
                meta.query_advice(a_4, Rotation::next()), // spread_c
                meta.query_advice(a_5, Rotation::cur()),  // spread_d
            )
        });

        MessageScheduleConfig {
            lookup,
            message_schedule,
            extras,
            s_word,
            s_decompose_0,
            s_decompose_1,
            s_decompose_2,
            s_decompose_3,
            s_lower_sigma_0,
            s_lower_sigma_1,
            s_lower_sigma_0_v2,
            s_lower_sigma_1_v2,
        }
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn process<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        input: [BlockWord; BLOCK_SIZE],
    ) -> Result<
        (
            [MessageWord<F>; ROUNDS],
            [(AssignedBits<16, F>, AssignedBits<16, F>); ROUNDS],
        ),
        Error,
    > {
        let mut w = Vec::<MessageWord<F>>::with_capacity(ROUNDS);
        let mut w_halves = Vec::<(AssignedBits<16, F>, AssignedBits<16, F>)>::with_capacity(ROUNDS);

        layouter.assign_region(
            || "process message block",
            |mut region| {
                w = Vec::<MessageWord<F>>::with_capacity(ROUNDS);
                w_halves = Vec::<(AssignedBits<16, F>, AssignedBits<16, F>)>::with_capacity(ROUNDS);

                // Assign all fixed columns
                for index in 1..14 {
                    let row = get_word_row(index);
                    self.s_decompose_1.enable(&mut region, row)?;
                    self.s_lower_sigma_0.enable(&mut region, row + 3)?;
                }

                for index in 14..49 {
                    let row = get_word_row(index);
                    self.s_decompose_2.enable(&mut region, row)?;
                    self.s_lower_sigma_0_v2.enable(&mut region, row + 3)?;
                    self.s_lower_sigma_1_v2
                        .enable(&mut region, row + SIGMA_0_V2_ROWS + 3)?;

                    let new_word_idx = index + 2;
                    self.s_word
                        .enable(&mut region, get_word_row(new_word_idx - 16) + 1)?;
                }

                for index in 49..62 {
                    let row = get_word_row(index);
                    self.s_decompose_3.enable(&mut region, row)?;
                    self.s_lower_sigma_1.enable(&mut region, row + 3)?;

                    let new_word_idx = index + 2;
                    self.s_word
                        .enable(&mut region, get_word_row(new_word_idx - 16) + 1)?;
                }

                for index in 0..64 {
                    let row = get_word_row(index);
                    self.s_decompose_0.enable(&mut region, row)?;
                }

                // Assign W[0..16]
                for (i, word) in input.iter().enumerate() {
                    let (word, halves) = self.assign_word_and_halves(&mut region, word.0, i)?;
                    w.push(MessageWord(word));
                    w_halves.push(halves);
                }

                // Returns the output of sigma_0 on W_[1..14]
                let lower_sigma_0_output = self.assign_subregion1(&mut region, &input[1..14])?;

                // sigma_0_v2 and sigma_1_v2 on W_[14..49]
                // Returns the output of sigma_0_v2 on W_[36..49], to be used in subregion3
                let lower_sigma_0_v2_output = self.assign_subregion2(
                    &mut region,
                    lower_sigma_0_output,
                    &mut w,
                    &mut w_halves,
                )?;

                // sigma_1 v1 on W[49..62]
                self.assign_subregion3(
                    &mut region,
                    lower_sigma_0_v2_output,
                    &mut w,
                    &mut w_halves,
                )?;

                Ok(())
            },
        )?;

        // w_halves need not to be range checked as 16-bit long, since only the sum of
        // the pair will be used in the following compression process.
        Ok((w.try_into().unwrap(), w_halves.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use ff::PrimeField;
    use halo2_proofs::halo2curves::pasta::pallas;
    use halo2_proofs::{
        circuit::{Layouter, V1, Value},
        dev::MockProver,
        plonk::{Circuit, ConstraintSystem, Error},
    };

    use super::{
        super::{super::BLOCK_SIZE, SpreadTableChip, Table16Chip, Table16Config},
        schedule_util::*,
    };
    use crate::zk::kagemusha_sha256_table16_v4::{BlockWord, util::lebs2ip};

    #[test]
    fn message_schedule() {
        struct MyCircuit {}

        impl<F: PrimeField> Circuit<F> for MyCircuit {
            type Config = Table16Config;
            type FloorPlanner = V1;
            type Params = ();

            fn without_witnesses(&self) -> Self {
                MyCircuit {}
            }

            fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
                Table16Chip::configure(meta)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<F>,
            ) -> Result<(), Error> {
                // Load lookup table
                SpreadTableChip::load(config.lookup.clone(), &mut layouter)?;

                // Provide input
                // Test vector: "abc"
                let inputs: [BlockWord; BLOCK_SIZE] = msg_schedule_test_input();

                // Run message_scheduler to get W_[0..64]
                let (w, _) = config.message_schedule.process(&mut layouter, inputs)?;
                for (word, test_word) in w.iter().zip(MSG_SCHEDULE_TEST_OUTPUT.iter()) {
                    word.value().assert_if_known(|bits| {
                        let word: u32 = lebs2ip(bits) as u32;
                        word == *test_word
                    });
                }
                Ok(())
            }
        }

        let circuit: MyCircuit = MyCircuit {};

        let prover = match MockProver::<pallas::Base>::run(17, &circuit, vec![]) {
            Ok(prover) => prover,
            Err(e) => panic!("{:?}", e),
        };
        assert_eq!(prover.verify(), Ok(()));
    }

    #[test]
    fn unreduced_message_schedule_carry_is_rejected() {
        struct CarryCircuit;

        impl<F: PrimeField> Circuit<F> for CarryCircuit {
            type Config = Table16Config;
            type FloorPlanner = V1;
            type Params = ();

            fn without_witnesses(&self) -> Self {
                Self
            }

            fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
                Table16Chip::configure(meta)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<F>,
            ) -> Result<(), Error> {
                Table16Chip::<F>::load(config.clone(), &mut layouter)?;
                let inputs = [BlockWord(Value::known(u32::MAX)); BLOCK_SIZE];
                let (words, _) = config.message_schedule.process(&mut layouter, inputs)?;
                let sigma0 = u32::MAX.rotate_right(7) ^ u32::MAX.rotate_right(18) ^ (u32::MAX >> 3);
                let sigma1 =
                    u32::MAX.rotate_right(17) ^ u32::MAX.rotate_right(19) ^ (u32::MAX >> 10);
                let unreduced = u64::from(u32::MAX)
                    + u64::from(sigma0)
                    + u64::from(u32::MAX)
                    + u64::from(sigma1);
                assert!(unreduced > u64::from(u32::MAX));
                layouter.assign_region(
                    || "reject an unreduced W16 carry",
                    |mut region| {
                        region.constrain_constant(words[16].cell(), F::from(unreduced))?;
                        Ok(())
                    },
                )
            }
        }

        let prover = MockProver::<pallas::Base>::run(17, &CarryCircuit, vec![])
            .expect("carry test synthesis");
        assert!(prover.verify().is_err());
    }
}
