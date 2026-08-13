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
use std::convert::TryInto;
use ff::PrimeField;
use halo2_proofs::{
    circuit::{Region, Value},
    plonk::Error,
};
use super::{
    super::{SpreadVar, SpreadWord, Table16Assignment},
    MessageScheduleConfig,
    schedule_util::*,
};
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedBits, BlockWord,
    util::{even_bits, i2lebsp, lebs2ip, odd_bits, spread_bits},
};
// A word in subregion 1
// (3, 4, 11, 14)-bit chunks
#[derive(Debug)]
pub struct Subregion1Word<F: PrimeField> {
    index: usize,
    a: AssignedBits<3, F>,
    b: AssignedBits<4, F>,
    spread_c: AssignedBits<22, F>,
    spread_d: AssignedBits<28, F>,
}
impl<F: PrimeField> Subregion1Word<F> {
    fn spread_a(&self) -> Value<[bool; 6]> {
        self.a.value().map(|v| v.spread())
    }
    fn spread_b(&self) -> Value<[bool; 8]> {
        self.b.value().map(|v| v.spread())
    }
    fn spread_c(&self) -> Value<[bool; 22]> {
        self.spread_c.value().map(|v| v.0)
    }
    fn spread_d(&self) -> Value<[bool; 28]> {
        self.spread_d.value().map(|v| v.0)
    }
    fn xor_lower_sigma_0(&self) -> Value<[bool; 64]> {
        self.spread_a()
            .zip(self.spread_b())
            .zip(self.spread_c())
            .zip(self.spread_d())
            .map(|(((a, b), c), d)| {
                let xor_0 = b
                    .iter()
                    .chain(c.iter())
                    .chain(d.iter())
                    .chain(std::iter::repeat(&false).take(6))
                    .copied()
                    .collect::<Vec<_>>();
                let xor_1 = c
                    .iter()
                    .chain(d.iter())
                    .chain(a.iter())
                    .chain(b.iter())
                    .copied()
                    .collect::<Vec<_>>();
                let xor_2 = d
                    .iter()
                    .chain(a.iter())
                    .chain(b.iter())
                    .chain(c.iter())
                    .copied()
                    .collect::<Vec<_>>();
                let xor_0 = lebs2ip::<64>(&xor_0.try_into().unwrap());
                let xor_1 = lebs2ip::<64>(&xor_1.try_into().unwrap());
                let xor_2 = lebs2ip::<64>(&xor_2.try_into().unwrap());
                i2lebsp(xor_0 + xor_1 + xor_2)
            })
    }
}
pub(crate) type AssignedHalves<F> = (AssignedBits<16, F>, AssignedBits<16, F>);
impl MessageScheduleConfig {
    pub fn assign_subregion1<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        input: &[BlockWord],
    ) -> Result<Vec<AssignedHalves<F>>, Error> {
        assert_eq!(input.len(), SUBREGION_1_LEN);
        Ok(input
            .iter()
            .enumerate()
            .map(|(idx, word)| {
                // s_decompose_1 on W_[1..14]
                let subregion1_word = self
                    .decompose_subregion1_word(
                        region,
                        word.0.map(|word| i2lebsp(word.into())),
                        idx + 1,
                    )
                    .unwrap();
                // lower_sigma_0 on W_[1..14]
                self.lower_sigma_0(region, subregion1_word).unwrap()
            })
            .collect::<Vec<_>>())
    }
    /// Pieces of length [3, 4, 11, 14]
    fn decompose_subregion1_word<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        word: Value<[bool; 32]>,
        index: usize,
    ) -> Result<Subregion1Word<F>, Error> {
        let row = get_word_row(index);
        // Rename these here for ease of matching the gates to the specification.
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let pieces = word.map(|word| {
            vec![
                word[0..3].to_vec(),
                word[3..7].to_vec(),
                word[7..18].to_vec(),
                word[18..32].to_vec(),
            ]
        });
        let pieces = pieces.transpose_vec(4);
        // Assign `a` (3-bit piece)
        let a = AssignedBits::<3, F>::assign_bits(
            region,
            || "word_a",
            a_3,
            row + 1,
            pieces[0].clone(),
        )?;
        // Assign `b` (4-bit piece)
        let b = AssignedBits::<4, F>::assign_bits(
            region,
            || "word_b",
            a_4,
            row + 1,
            pieces[1].clone(),
        )?;
        // Assign `c` (11-bit piece) lookup
        let spread_c = pieces[2].clone().map(SpreadWord::<11, 22>::try_new);
        let spread_c = SpreadVar::with_lookup(region, &self.lookup, row + 1, spread_c)?;
        // Assign `d` (14-bit piece) lookup
        let spread_d = pieces[3].clone().map(SpreadWord::<14, 28>::try_new);
        let spread_d = SpreadVar::with_lookup(region, &self.lookup, row, spread_d)?;
        Ok(Subregion1Word {
            index,
            a,
            b,
            spread_c: spread_c.spread,
            spread_d: spread_d.spread,
        })
    }
    // sigma_0 v1 on a word in W_1 to W_13
    // (3, 4, 11, 14)-bit chunks
    fn lower_sigma_0<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        word: Subregion1Word<F>,
    ) -> Result<(AssignedBits<16, F>, AssignedBits<16, F>), Error> {
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_5 = self.message_schedule;
        let a_6 = self.extras[2];
        let row = get_word_row(word.index) + 3;
        // Assign `a` and copy constraint
        word.a.copy_advice(|| "a", region, a_5, row + 1)?;
        // Witness `spread_a`
        let spread_a = word.a.value().map(|bits| spread_bits(bits.0));
        AssignedBits::<6, F>::assign_bits(region, || "spread_a", a_6, row + 1, spread_a)?;
        // Split `b` (4-bit chunk) into `b_hi` and `b_lo`
        // Assign `b_lo`, `spread_b_lo`
        let b_lo: Value<[bool; 2]> = word.b.value().map(|b| b.0[..2].try_into().unwrap());
        let spread_b_lo = b_lo.map(spread_bits);
        {
            AssignedBits::<2, F>::assign_bits(region, || "b_lo", a_3, row - 1, b_lo)?;
            AssignedBits::<4, F>::assign_bits(region, || "spread_b_lo", a_4, row - 1, spread_b_lo)?;
        };
        // Split `b` (2-bit chunk) into `b_hi` and `b_lo`
        // Assign `b_hi`, `spread_b_hi`
        let b_hi: Value<[bool; 2]> = word.b.value().map(|b| b.0[2..].try_into().unwrap());
        let spread_b_hi = b_hi.map(spread_bits);
        {
            AssignedBits::<2, F>::assign_bits(region, || "b_hi", a_5, row - 1, b_hi)?;
            AssignedBits::<4, F>::assign_bits(region, || "spread_b_hi", a_6, row - 1, spread_b_hi)?;
        };
        // Assign `b` and copy constraint
        word.b.copy_advice(|| "b", region, a_6, row)?;
        // Assign `spread_c` and copy constraint
        word.spread_c.copy_advice(|| "spread_c", region, a_4, row)?;
        // Assign `spread_d` and copy constraint
        word.spread_d.copy_advice(|| "spread_d", region, a_5, row)?;
        // Calculate R_0^{even}, R_0^{odd}, R_1^{even}, R_1^{odd}
        let r = word.xor_lower_sigma_0();
        let r_0: Value<[bool; 32]> = r.map(|r| r[..32].try_into().unwrap());
        let r_0_even = r_0.map(even_bits);
        let r_0_odd = r_0.map(odd_bits);
        let r_1: Value<[bool; 32]> = r.map(|r| r[32..].try_into().unwrap());
        let r_1_even = r_1.map(even_bits);
        let r_1_odd = r_1.map(odd_bits);
        self.assign_sigma_outputs(
            region,
            &self.lookup,
            a_3,
            row,
            r_0_even,
            r_0_odd,
            r_1_even,
            r_1_odd,
        )
    }
}
