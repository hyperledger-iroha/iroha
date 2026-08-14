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
use std::{convert::TryInto, ops::Range};
use ff::PrimeField;
use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};
use super::{super::DIGEST_SIZE, SpreadInputs, SpreadVar, SpreadWord, Table16Assignment};
use crate::zk::kagemusha_sha256_table16_v4::AssignedWord;
mod compression_gates;
mod compression_util;
mod subregion_digest;
mod subregion_initial;
mod subregion_main;
use compression_gates::CompressionGate;
use compression_util::match_state;
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedBits, ROUNDS,
    util::{i2lebsp, lebs2ip},
};
pub trait UpperSigmaVar<
    const A_LEN: usize,
    const B_LEN: usize,
    const C_LEN: usize,
    const D_LEN: usize,
>
{
    fn spread_a(&self) -> Value<[bool; A_LEN]>;
    fn spread_b(&self) -> Value<[bool; B_LEN]>;
    fn spread_c(&self) -> Value<[bool; C_LEN]>;
    fn spread_d(&self) -> Value<[bool; D_LEN]>;
    fn xor_upper_sigma(&self) -> Value<[bool; 64]> {
        self.spread_a()
            .zip(self.spread_b())
            .zip(self.spread_c())
            .zip(self.spread_d())
            .map(|(((a, b), c), d)| {
                let xor_0 = b
                    .iter()
                    .chain(c.iter())
                    .chain(d.iter())
                    .chain(a.iter())
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
/// A variable that represents the `[A,B,C,D]` words of the SHA-256 internal
/// state.
///
/// The structure of this variable is influenced by the following factors:
/// - In `Σ_0(A)` we need `A` to be split into pieces `(a,b,c,d)` of lengths
///   `(2,11,9,10)` bits respectively (counting from the little end), as well as
///   their spread forms.
/// - `Maj(A,B,C)` requires having the bits of each input in spread form. For
///   `A` we can reuse the pieces from `Σ_0(A)`. Since `B` and `C` are assigned
///   from `A` and `B` respectively in each round, we therefore also have the
///   same pieces in earlier rows. We align the columns to make it efficient to
///   copy-constrain these forms where they are needed.
#[derive(Clone, Debug)]
pub struct AbcdVar<F: PrimeField> {
    a: SpreadVar<2, 4, F>,
    b: SpreadVar<11, 22, F>,
    c_lo: SpreadVar<3, 6, F>,
    c_mid: SpreadVar<3, 6, F>,
    c_hi: SpreadVar<3, 6, F>,
    d: SpreadVar<10, 20, F>,
}
impl<F: PrimeField> AbcdVar<F> {
    fn a_range() -> Range<usize> {
        0..2
    }
    fn b_range() -> Range<usize> {
        2..13
    }
    fn c_lo_range() -> Range<usize> {
        13..16
    }
    fn c_mid_range() -> Range<usize> {
        16..19
    }
    fn c_hi_range() -> Range<usize> {
        19..22
    }
    fn d_range() -> Range<usize> {
        22..32
    }
    fn pieces(val: u32) -> Vec<Vec<bool>> {
        let val: [bool; 32] = i2lebsp(val.into());
        vec![
            val[Self::a_range()].to_vec(),
            val[Self::b_range()].to_vec(),
            val[Self::c_lo_range()].to_vec(),
            val[Self::c_mid_range()].to_vec(),
            val[Self::c_hi_range()].to_vec(),
            val[Self::d_range()].to_vec(),
        ]
    }
}
impl<F: PrimeField> UpperSigmaVar<4, 22, 18, 20> for AbcdVar<F> {
    fn spread_a(&self) -> Value<[bool; 4]> {
        self.a.spread.value().map(|v| v.0)
    }
    fn spread_b(&self) -> Value<[bool; 22]> {
        self.b.spread.value().map(|v| v.0)
    }
    fn spread_c(&self) -> Value<[bool; 18]> {
        self.c_lo
            .spread
            .value()
            .zip(self.c_mid.spread.value())
            .zip(self.c_hi.spread.value())
            .map(|((c_lo, c_mid), c_hi)| {
                c_lo.iter()
                    .chain(c_mid.iter())
                    .chain(c_hi.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
    }
    fn spread_d(&self) -> Value<[bool; 20]> {
        self.d.spread.value().map(|v| v.0)
    }
}
/// A variable that represents the `[E,F,G,H]` words of the SHA-256 internal
/// state.
///
/// The structure of this variable is influenced by the following factors:
/// - In `Σ_1(E)` we need `E` to be split into pieces `(a,b,c,d)` of lengths
///   `(6,5,14,7)` bits respectively (counting from the little end), as well as
///   their spread forms.
/// - `Ch(E,F,G)` requires having the bits of each input in spread form. For `E`
///   we can reuse the pieces from `Σ_1(E)`. Since `F` and `G` are assigned from
///   `E` and `F` respectively in each round, we therefore also have the same
///   pieces in earlier rows. We align the columns to make it efficient to
///   copy-constrain these forms where they are needed.
#[derive(Clone, Debug)]
pub struct EfghVar<F: PrimeField> {
    a_lo: SpreadVar<3, 6, F>,
    a_hi: SpreadVar<3, 6, F>,
    b_lo: SpreadVar<2, 4, F>,
    b_hi: SpreadVar<3, 6, F>,
    c: SpreadVar<14, 28, F>,
    d: SpreadVar<7, 14, F>,
}
impl<F: PrimeField> EfghVar<F> {
    fn a_lo_range() -> Range<usize> {
        0..3
    }
    fn a_hi_range() -> Range<usize> {
        3..6
    }
    fn b_lo_range() -> Range<usize> {
        6..8
    }
    fn b_hi_range() -> Range<usize> {
        8..11
    }
    fn c_range() -> Range<usize> {
        11..25
    }
    fn d_range() -> Range<usize> {
        25..32
    }
    fn pieces(val: u32) -> Vec<Vec<bool>> {
        let val: [bool; 32] = i2lebsp(val.into());
        vec![
            val[Self::a_lo_range()].to_vec(),
            val[Self::a_hi_range()].to_vec(),
            val[Self::b_lo_range()].to_vec(),
            val[Self::b_hi_range()].to_vec(),
            val[Self::c_range()].to_vec(),
            val[Self::d_range()].to_vec(),
        ]
    }
}
impl<F: PrimeField> UpperSigmaVar<12, 10, 28, 14> for EfghVar<F> {
    fn spread_a(&self) -> Value<[bool; 12]> {
        self.a_lo
            .spread
            .value()
            .zip(self.a_hi.spread.value())
            .map(|(a_lo, a_hi)| {
                a_lo.iter()
                    .chain(a_hi.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
    }
    fn spread_b(&self) -> Value<[bool; 10]> {
        self.b_lo
            .spread
            .value()
            .zip(self.b_hi.spread.value())
            .map(|(b_lo, b_hi)| {
                b_lo.iter()
                    .chain(b_hi.iter())
                    .copied()
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
    }
    fn spread_c(&self) -> Value<[bool; 28]> {
        self.c.spread.value().map(|v| v.0)
    }
    fn spread_d(&self) -> Value<[bool; 14]> {
        self.d.spread.value().map(|v| v.0)
    }
}
#[derive(Clone, Debug)]
pub struct RoundWordDense<F: PrimeField>(AssignedBits<16, F>, AssignedBits<16, F>);
impl<F: PrimeField> From<(AssignedBits<16, F>, AssignedBits<16, F>)> for RoundWordDense<F> {
    fn from(halves: (AssignedBits<16, F>, AssignedBits<16, F>)) -> Self {
        Self(halves.0, halves.1)
    }
}
impl<F: PrimeField> RoundWordDense<F> {
    pub fn value(&self) -> Value<u32> {
        self.0
            .value_u16()
            .zip(self.1.value_u16())
            .map(|(lo, hi)| lo as u32 + (1 << 16) * hi as u32)
    }
}
#[derive(Clone, Debug)]
pub struct RoundWordSpread<F: PrimeField>(AssignedBits<32, F>, AssignedBits<32, F>);
impl<F: PrimeField> From<(AssignedBits<32, F>, AssignedBits<32, F>)> for RoundWordSpread<F> {
    fn from(halves: (AssignedBits<32, F>, AssignedBits<32, F>)) -> Self {
        Self(halves.0, halves.1)
    }
}
impl<F: PrimeField> RoundWordSpread<F> {
    pub fn value(&self) -> Value<u64> {
        self.0
            .value_u32()
            .zip(self.1.value_u32())
            .map(|(lo, hi)| lo as u64 + (1 << 32) * hi as u64)
    }
}
#[derive(Clone, Debug)]
pub struct RoundWordA<F: PrimeField> {
    pieces: Option<AbcdVar<F>>,
    dense_halves: RoundWordDense<F>,
    spread_halves: Option<RoundWordSpread<F>>,
}
impl<F: PrimeField> RoundWordA<F> {
    pub fn new(
        pieces: AbcdVar<F>,
        dense_halves: RoundWordDense<F>,
        spread_halves: RoundWordSpread<F>,
    ) -> Self {
        RoundWordA {
            pieces: Some(pieces),
            dense_halves,
            spread_halves: Some(spread_halves),
        }
    }
    pub fn new_dense(dense_halves: RoundWordDense<F>) -> Self {
        RoundWordA {
            pieces: None,
            dense_halves,
            spread_halves: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct RoundWordE<F: PrimeField> {
    pieces: Option<EfghVar<F>>,
    dense_halves: RoundWordDense<F>,
    spread_halves: Option<RoundWordSpread<F>>,
}
impl<F: PrimeField> RoundWordE<F> {
    pub fn new(
        pieces: EfghVar<F>,
        dense_halves: RoundWordDense<F>,
        spread_halves: RoundWordSpread<F>,
    ) -> Self {
        RoundWordE {
            pieces: Some(pieces),
            dense_halves,
            spread_halves: Some(spread_halves),
        }
    }
    pub fn new_dense(dense_halves: RoundWordDense<F>) -> Self {
        RoundWordE {
            pieces: None,
            dense_halves,
            spread_halves: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct RoundWord<F: PrimeField> {
    dense_halves: RoundWordDense<F>,
    spread_halves: Option<RoundWordSpread<F>>,
}
impl<F: PrimeField> RoundWord<F> {
    pub fn new(dense_halves: RoundWordDense<F>, spread_halves: Option<RoundWordSpread<F>>) -> Self {
        RoundWord {
            dense_halves,
            spread_halves,
        }
    }
}
/// The internal state for SHA-256.
#[derive(Clone, Debug)]
pub struct State<F: PrimeField> {
    a: Option<StateWord<F>>,
    b: Option<StateWord<F>>,
    c: Option<StateWord<F>>,
    d: Option<StateWord<F>>,
    e: Option<StateWord<F>>,
    f: Option<StateWord<F>>,
    g: Option<StateWord<F>>,
    h: Option<StateWord<F>>,
}
impl<F: PrimeField> State<F> {
    #[allow(clippy::many_single_char_names)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        a: StateWord<F>,
        b: StateWord<F>,
        c: StateWord<F>,
        d: StateWord<F>,
        e: StateWord<F>,
        f: StateWord<F>,
        g: StateWord<F>,
        h: StateWord<F>,
    ) -> Self {
        State {
            a: Some(a),
            b: Some(b),
            c: Some(c),
            d: Some(d),
            e: Some(e),
            f: Some(f),
            g: Some(g),
            h: Some(h),
        }
    }
    pub fn empty_state() -> Self {
        State {
            a: None,
            b: None,
            c: None,
            d: None,
            e: None,
            f: None,
            g: None,
            h: None,
        }
    }
}
#[derive(Clone, Debug)]
pub enum StateWord<F: PrimeField> {
    A(RoundWordA<F>),
    B(RoundWord<F>),
    C(RoundWord<F>),
    D(RoundWordDense<F>),
    E(RoundWordE<F>),
    F(RoundWord<F>),
    G(RoundWord<F>),
    H(RoundWordDense<F>),
}
#[derive(Clone, Debug)]
pub(super) struct CompressionConfig {
    lookup: SpreadInputs,
    message_schedule: Column<Advice>,
    extras: [Column<Advice>; 6],
    s_ch: Selector,
    s_ch_neg: Selector,
    s_maj: Selector,
    s_h_prime: Selector,
    s_a_new: Selector,
    s_e_new: Selector,
    s_upper_sigma_0: Selector,
    s_upper_sigma_1: Selector,
    s_add_halves: Selector,
    // Decomposition gate for AbcdVar
    s_decompose_abcd: Selector,
    // Decomposition gate for EfghVar
    s_decompose_efgh: Selector,
    s_digest: Selector,
}
impl<F: PrimeField> Table16Assignment<F> for CompressionConfig {}
impl CompressionConfig {
    pub(super) fn configure<F: PrimeField>(
        meta: &mut ConstraintSystem<F>,
        lookup: SpreadInputs,
        message_schedule: Column<Advice>,
        extras: [Column<Advice>; 6],
    ) -> Self {
        let s_ch = meta.selector();
        let s_ch_neg = meta.selector();
        let s_maj = meta.selector();
        let s_h_prime = meta.selector();
        let s_a_new = meta.selector();
        let s_e_new = meta.selector();
        let s_upper_sigma_0 = meta.selector();
        let s_upper_sigma_1 = meta.selector();
        let s_add_halves = meta.selector();
        // Decomposition gate for AbcdVar
        let s_decompose_abcd = meta.selector();
        // Decomposition gate for EfghVar
        let s_decompose_efgh = meta.selector();
        let s_digest = meta.selector();
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
        // Decompose `A,B,C,D` words into (2, 11, 9, 10)-bit chunks.
        // `c` is split into (3, 3, 3)-bit c_lo, c_mid, c_hi.
        meta.create_gate("decompose ABCD", |meta| {
            let s_decompose_abcd = meta.query_selector(s_decompose_abcd);
            let a = meta.query_advice(a_3, Rotation::next()); // 2-bit chunk
            let spread_a = meta.query_advice(a_4, Rotation::next());
            let b = meta.query_advice(a_1, Rotation::cur()); // 11-bit chunk
            let spread_b = meta.query_advice(a_2, Rotation::cur());
            let tag_b = meta.query_advice(a_0, Rotation::cur());
            let c_lo = meta.query_advice(a_3, Rotation::cur()); // 3-bit chunk
            let spread_c_lo = meta.query_advice(a_4, Rotation::cur());
            let c_mid = meta.query_advice(a_5, Rotation::cur()); // 3-bit chunk
            let spread_c_mid = meta.query_advice(a_6, Rotation::cur());
            let c_hi = meta.query_advice(a_5, Rotation::next()); // 3-bit chunk
            let spread_c_hi = meta.query_advice(a_6, Rotation::next());
            let d = meta.query_advice(a_1, Rotation::next()); // 7-bit chunk
            let spread_d = meta.query_advice(a_2, Rotation::next());
            let tag_d = meta.query_advice(a_0, Rotation::next());
            let word_lo = meta.query_advice(a_7, Rotation::cur());
            let spread_word_lo = meta.query_advice(a_8, Rotation::cur());
            let word_hi = meta.query_advice(a_7, Rotation::next());
            let spread_word_hi = meta.query_advice(a_8, Rotation::next());
            CompressionGate::s_decompose_abcd(
                s_decompose_abcd,
                a,
                spread_a,
                b,
                spread_b,
                tag_b,
                c_lo,
                spread_c_lo,
                c_mid,
                spread_c_mid,
                c_hi,
                spread_c_hi,
                d,
                spread_d,
                tag_d,
                word_lo,
                spread_word_lo,
                word_hi,
                spread_word_hi,
            )
        });
        // Decompose `E,F,G,H` words into (6, 5, 14, 7)-bit chunks.
        // `a` is split into (3, 3)-bit a_lo, a_hi
        // `b` is split into (2, 3)-bit b_lo, b_hi
        meta.create_gate("Decompose EFGH", |meta| {
            let s_decompose_efgh = meta.query_selector(s_decompose_efgh);
            let a_lo = meta.query_advice(a_3, Rotation::next()); // 3-bit chunk
            let spread_a_lo = meta.query_advice(a_4, Rotation::next());
            let a_hi = meta.query_advice(a_5, Rotation::next()); // 3-bit chunk
            let spread_a_hi = meta.query_advice(a_6, Rotation::next());
            let b_lo = meta.query_advice(a_3, Rotation::cur()); // 2-bit chunk
            let spread_b_lo = meta.query_advice(a_4, Rotation::cur());
            let b_hi = meta.query_advice(a_5, Rotation::cur()); // 3-bit chunk
            let spread_b_hi = meta.query_advice(a_6, Rotation::cur());
            let c = meta.query_advice(a_1, Rotation::next()); // 14-bit chunk
            let spread_c = meta.query_advice(a_2, Rotation::next());
            let tag_c = meta.query_advice(a_0, Rotation::next());
            let d = meta.query_advice(a_1, Rotation::cur()); // 7-bit chunk
            let spread_d = meta.query_advice(a_2, Rotation::cur());
            let tag_d = meta.query_advice(a_0, Rotation::cur());
            let word_lo = meta.query_advice(a_7, Rotation::cur());
            let spread_word_lo = meta.query_advice(a_8, Rotation::cur());
            let word_hi = meta.query_advice(a_7, Rotation::next());
            let spread_word_hi = meta.query_advice(a_8, Rotation::next());
            CompressionGate::s_decompose_efgh(
                s_decompose_efgh,
                a_lo,
                spread_a_lo,
                a_hi,
                spread_a_hi,
                b_lo,
                spread_b_lo,
                b_hi,
                spread_b_hi,
                c,
                spread_c,
                tag_c,
                d,
                spread_d,
                tag_d,
                word_lo,
                spread_word_lo,
                word_hi,
                spread_word_hi,
            )
        });
        // s_upper_sigma_0 on abcd words
        // (2, 11, 9, 10)-bit chunks
        meta.create_gate("s_upper_sigma_0", |meta| {
            let s_upper_sigma_0 = meta.query_selector(s_upper_sigma_0);
            let spread_r0_even = meta.query_advice(a_2, Rotation::prev());
            let spread_r0_odd = meta.query_advice(a_2, Rotation::cur());
            let spread_r1_even = meta.query_advice(a_2, Rotation::next());
            let spread_r1_odd = meta.query_advice(a_3, Rotation::cur());
            let spread_a = meta.query_advice(a_3, Rotation::next());
            let spread_b = meta.query_advice(a_5, Rotation::cur());
            let spread_c_lo = meta.query_advice(a_3, Rotation::prev());
            let spread_c_mid = meta.query_advice(a_4, Rotation::prev());
            let spread_c_hi = meta.query_advice(a_4, Rotation::next());
            let spread_d = meta.query_advice(a_4, Rotation::cur());
            CompressionGate::s_upper_sigma_0(
                s_upper_sigma_0,
                spread_r0_even,
                spread_r0_odd,
                spread_r1_even,
                spread_r1_odd,
                spread_a,
                spread_b,
                spread_c_lo,
                spread_c_mid,
                spread_c_hi,
                spread_d,
            )
        });
        // s_upper_sigma_1 on efgh words
        // (6, 5, 14, 7)-bit chunks
        meta.create_gate("s_upper_sigma_1", |meta| {
            let s_upper_sigma_1 = meta.query_selector(s_upper_sigma_1);
            let spread_r0_even = meta.query_advice(a_2, Rotation::prev());
            let spread_r0_odd = meta.query_advice(a_2, Rotation::cur());
            let spread_r1_even = meta.query_advice(a_2, Rotation::next());
            let spread_r1_odd = meta.query_advice(a_3, Rotation::cur());
            let spread_a_lo = meta.query_advice(a_3, Rotation::next());
            let spread_a_hi = meta.query_advice(a_4, Rotation::next());
            let spread_b_lo = meta.query_advice(a_3, Rotation::prev());
            let spread_b_hi = meta.query_advice(a_4, Rotation::prev());
            let spread_c = meta.query_advice(a_5, Rotation::cur());
            let spread_d = meta.query_advice(a_4, Rotation::cur());
            CompressionGate::s_upper_sigma_1(
                s_upper_sigma_1,
                spread_r0_even,
                spread_r0_odd,
                spread_r1_even,
                spread_r1_odd,
                spread_a_lo,
                spread_a_hi,
                spread_b_lo,
                spread_b_hi,
                spread_c,
                spread_d,
            )
        });
        // s_ch on efgh words
        // First part of choice gate on (E, F, G), E ∧ F
        meta.create_gate("s_ch", |meta| {
            let s_ch = meta.query_selector(s_ch);
            let spread_p0_even = meta.query_advice(a_2, Rotation::prev());
            let spread_p0_odd = meta.query_advice(a_2, Rotation::cur());
            let spread_p1_even = meta.query_advice(a_2, Rotation::next());
            let spread_p1_odd = meta.query_advice(a_3, Rotation::cur());
            let spread_e_lo = meta.query_advice(a_3, Rotation::prev());
            let spread_e_hi = meta.query_advice(a_4, Rotation::prev());
            let spread_f_lo = meta.query_advice(a_3, Rotation::next());
            let spread_f_hi = meta.query_advice(a_4, Rotation::next());
            CompressionGate::s_ch(
                s_ch,
                spread_p0_even,
                spread_p0_odd,
                spread_p1_even,
                spread_p1_odd,
                spread_e_lo,
                spread_e_hi,
                spread_f_lo,
                spread_f_hi,
            )
        });
        // s_ch_neg on efgh words
        // Second part of Choice gate on (E, F, G), ¬E ∧ G
        meta.create_gate("s_ch_neg", |meta| {
            let s_ch_neg = meta.query_selector(s_ch_neg);
            let spread_q0_even = meta.query_advice(a_2, Rotation::prev());
            let spread_q0_odd = meta.query_advice(a_2, Rotation::cur());
            let spread_q1_even = meta.query_advice(a_2, Rotation::next());
            let spread_q1_odd = meta.query_advice(a_3, Rotation::cur());
            let spread_e_lo = meta.query_advice(a_5, Rotation::prev());
            let spread_e_hi = meta.query_advice(a_5, Rotation::cur());
            let spread_e_neg_lo = meta.query_advice(a_3, Rotation::prev());
            let spread_e_neg_hi = meta.query_advice(a_4, Rotation::prev());
            let spread_g_lo = meta.query_advice(a_3, Rotation::next());
            let spread_g_hi = meta.query_advice(a_4, Rotation::next());
            CompressionGate::s_ch_neg(
                s_ch_neg,
                spread_q0_even,
                spread_q0_odd,
                spread_q1_even,
                spread_q1_odd,
                spread_e_lo,
                spread_e_hi,
                spread_e_neg_lo,
                spread_e_neg_hi,
                spread_g_lo,
                spread_g_hi,
            )
        });
        // s_maj on abcd words
        meta.create_gate("s_maj", |meta| {
            let s_maj = meta.query_selector(s_maj);
            let spread_m0_even = meta.query_advice(a_2, Rotation::prev());
            let spread_m0_odd = meta.query_advice(a_2, Rotation::cur());
            let spread_m1_even = meta.query_advice(a_2, Rotation::next());
            let spread_m1_odd = meta.query_advice(a_3, Rotation::cur());
            let spread_a_lo = meta.query_advice(a_4, Rotation::prev());
            let spread_a_hi = meta.query_advice(a_5, Rotation::prev());
            let spread_b_lo = meta.query_advice(a_4, Rotation::cur());
            let spread_b_hi = meta.query_advice(a_5, Rotation::cur());
            let spread_c_lo = meta.query_advice(a_4, Rotation::next());
            let spread_c_hi = meta.query_advice(a_5, Rotation::next());
            CompressionGate::s_maj(
                s_maj,
                spread_m0_even,
                spread_m0_odd,
                spread_m1_even,
                spread_m1_odd,
                spread_a_lo,
                spread_a_hi,
                spread_b_lo,
                spread_b_hi,
                spread_c_lo,
                spread_c_hi,
            )
        });
        // s_h_prime to compute H' = H + Ch(E, F, G) + s_upper_sigma_1(E) + K + W
        meta.create_gate("s_h_prime", |meta| {
            let s_h_prime = meta.query_selector(s_h_prime);
            let h_prime_lo = meta.query_advice(a_7, Rotation::next());
            let h_prime_hi = meta.query_advice(a_8, Rotation::next());
            let h_prime_carry = meta.query_advice(a_9, Rotation::next());
            let sigma_e_lo = meta.query_advice(a_4, Rotation::cur());
            let sigma_e_hi = meta.query_advice(a_5, Rotation::cur());
            let ch_lo = meta.query_advice(a_1, Rotation::cur());
            let ch_hi = meta.query_advice(a_6, Rotation::next());
            let ch_neg_lo = meta.query_advice(a_5, Rotation::prev());
            let ch_neg_hi = meta.query_advice(a_5, Rotation::next());
            let h_lo = meta.query_advice(a_7, Rotation::prev());
            let h_hi = meta.query_advice(a_7, Rotation::cur());
            let k_lo = meta.query_advice(a_6, Rotation::prev());
            let k_hi = meta.query_advice(a_6, Rotation::cur());
            let w_lo = meta.query_advice(a_8, Rotation::prev());
            let w_hi = meta.query_advice(a_8, Rotation::cur());
            CompressionGate::s_h_prime(
                s_h_prime,
                h_prime_lo,
                h_prime_hi,
                h_prime_carry,
                sigma_e_lo,
                sigma_e_hi,
                ch_lo,
                ch_hi,
                ch_neg_lo,
                ch_neg_hi,
                h_lo,
                h_hi,
                k_lo,
                k_hi,
                w_lo,
                w_hi,
            )
        });
        // s_add_halves
        meta.create_gate("s_add_halves", |meta| {
            let s_add_halves = meta.query_selector(s_add_halves);
            let st_new_lo = meta.query_advice(a_7, Rotation::cur());
            let st_new_hi = meta.query_advice(a_8, Rotation::cur());
            let st_new_carry = meta.query_advice(a_9, Rotation::cur());
            let term1_lo = meta.query_advice(a_3, Rotation::cur());
            let term1_hi = meta.query_advice(a_4, Rotation::cur());
            let term2_lo = meta.query_advice(a_5, Rotation::cur());
            let term2_hi = meta.query_advice(a_6, Rotation::cur());
            CompressionGate::s_add_halves(
                s_add_halves,
                st_new_lo,
                st_new_hi,
                st_new_carry,
                term1_lo,
                term1_hi,
                term2_lo,
                term2_hi,
            )
        });
        // s_a_new
        meta.create_gate("s_a_new", |meta| {
            let s_a_new = meta.query_selector(s_a_new);
            let a_new_lo = meta.query_advice(a_8, Rotation::cur());
            let a_new_hi = meta.query_advice(a_8, Rotation::next());
            let a_new_carry = meta.query_advice(a_9, Rotation::cur());
            let sigma_a_lo = meta.query_advice(a_6, Rotation::cur());
            let sigma_a_hi = meta.query_advice(a_6, Rotation::next());
            let maj_abc_lo = meta.query_advice(a_1, Rotation::cur());
            let maj_abc_hi = meta.query_advice(a_3, Rotation::prev());
            let h_prime_lo = meta.query_advice(a_7, Rotation::prev());
            let h_prime_hi = meta.query_advice(a_8, Rotation::prev());
            CompressionGate::s_a_new(
                s_a_new,
                a_new_lo,
                a_new_hi,
                a_new_carry,
                sigma_a_lo,
                sigma_a_hi,
                maj_abc_lo,
                maj_abc_hi,
                h_prime_lo,
                h_prime_hi,
            )
        });
        // s_e_new
        meta.create_gate("s_e_new", |meta| {
            let s_e_new = meta.query_selector(s_e_new);
            let e_new_lo = meta.query_advice(a_8, Rotation::cur());
            let e_new_hi = meta.query_advice(a_8, Rotation::next());
            let e_new_carry = meta.query_advice(a_9, Rotation::next());
            let d_lo = meta.query_advice(a_7, Rotation::cur());
            let d_hi = meta.query_advice(a_7, Rotation::next());
            let h_prime_lo = meta.query_advice(a_7, Rotation::prev());
            let h_prime_hi = meta.query_advice(a_8, Rotation::prev());
            CompressionGate::s_e_new(
                s_e_new,
                e_new_lo,
                e_new_hi,
                e_new_carry,
                d_lo,
                d_hi,
                h_prime_lo,
                h_prime_hi,
            )
        });
        // s_digest for final round
        meta.create_gate("s_digest", |meta| {
            let s_digest = meta.query_selector(s_digest);
            let lo_0 = meta.query_advice(a_3, Rotation::cur());
            let hi_0 = meta.query_advice(a_4, Rotation::cur());
            let word_0 = meta.query_advice(a_5, Rotation::cur());
            let lo_1 = meta.query_advice(a_6, Rotation::cur());
            let hi_1 = meta.query_advice(a_7, Rotation::cur());
            let word_1 = meta.query_advice(a_8, Rotation::cur());
            let lo_2 = meta.query_advice(a_3, Rotation::next());
            let hi_2 = meta.query_advice(a_4, Rotation::next());
            let word_2 = meta.query_advice(a_5, Rotation::next());
            let lo_3 = meta.query_advice(a_6, Rotation::next());
            let hi_3 = meta.query_advice(a_7, Rotation::next());
            let word_3 = meta.query_advice(a_8, Rotation::next());
            CompressionGate::s_digest(
                s_digest, lo_0, hi_0, word_0, lo_1, hi_1, word_1, lo_2, hi_2, word_2, lo_3, hi_3,
                word_3,
            )
        });
        CompressionConfig {
            lookup,
            message_schedule,
            extras,
            s_ch,
            s_ch_neg,
            s_maj,
            s_h_prime,
            s_a_new,
            s_e_new,
            s_upper_sigma_0,
            s_upper_sigma_1,
            s_add_halves,
            s_decompose_abcd,
            s_decompose_efgh,
            s_digest,
        }
    }
    /// Initialize compression with a constant Initialization Vector of 32-byte
    /// words. Returns an initialized state.
    pub(super) fn initialize_with_iv<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
    ) -> Result<State<F>, Error> {
        let mut new_state = State::empty_state();
        layouter.assign_region(
            || "initialize_with_iv",
            |mut region| {
                new_state = self.initialize_iv(&mut region)?;
                Ok(())
            },
        )?;
        Ok(new_state)
    }
    /// Initialize compression with some initialized state. This could be a
    /// state output from a previous compression round.
    pub(super) fn initialize_with_state<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        init_state: State<F>,
    ) -> Result<State<F>, Error> {
        let mut new_state = State::empty_state();
        layouter.assign_region(
            || "initialize_with_state",
            |mut region| {
                new_state = self.initialize_state(&mut region, init_state.clone())?;
                Ok(())
            },
        )?;
        Ok(new_state)
    }
    /// Given an initialized state and a message schedule, perform 64
    /// compression rounds.
    pub(super) fn compress<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        initialized_state: State<F>,
        w_halves: [(AssignedBits<16, F>, AssignedBits<16, F>); ROUNDS],
        lookup_inputs: &SpreadInputs,
    ) -> Result<State<F>, Error> {
        let mut state = State::empty_state();
        let mut assigned_halves = layouter.assign_region(
            || "compress",
            |mut region| {
                let mut assigned_halves = Vec::new();
                state = initialized_state.clone();
                for (idx, w_halves) in w_halves.iter().enumerate() {
                    state = self.assign_round(
                        &mut region,
                        idx.into(),
                        &initialized_state,
                        state.clone(),
                        w_halves,
                        &mut assigned_halves,
                    )?;
                }
                Ok(assigned_halves)
            },
        )?;
        // The feed-forward outputs are part of the reusable State API, so make
        // all sixteen halves canonical here rather than relying on a future
        // consumer to range-check them.
        let (a, b, c, d, e, f, g, h) = match_state(state.clone());
        assigned_halves.extend([
            a.dense_halves.0,
            a.dense_halves.1,
            b.dense_halves.0,
            b.dense_halves.1,
            c.dense_halves.0,
            c.dense_halves.1,
            d.0,
            d.1,
            e.dense_halves.0,
            e.dense_halves.1,
            f.dense_halves.0,
            f.dense_halves.1,
            g.dense_halves.0,
            g.dense_halves.1,
            h.0,
            h.1,
        ]);
        debug_assert_eq!(assigned_halves.len(), 148);
        // Range-check 128 H' halves, the four final-round raw A/E halves, and
        // the sixteen canonical feed-forward output halves.
        layouter.assign_region(
            || "16-bit range check for the assigned halves from compress",
            |mut region| {
                for (offset, assigned_half) in assigned_halves.iter().enumerate() {
                    let dense_val = assigned_half.value().map(|bits| bits.0);
                    let spread_var = SpreadVar::with_lookup(
                        &mut region,
                        lookup_inputs,
                        offset,
                        dense_val.map(SpreadWord::<16, 32>::new),
                    )?;
                    // equality constraint for the newly assigned half and the previous one
                    region.constrain_equal(assigned_half.cell(), spread_var.dense.cell());
                }
                Ok(())
            },
        )?;
        Ok(state)
    }
    /// After the final round, convert the state into the final digest.
    pub(super) fn digest<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        state: State<F>,
    ) -> Result<[AssignedWord<F>; DIGEST_SIZE], Error> {
        let digest = layouter.assign_region(
            || "digest",
            |mut region| self.assign_digest(&mut region, state.clone()),
        )?;
        Ok(digest)
    }
}
#[cfg(test)]
mod isolated_tests {
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::Fp,
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use super::super::{Table16Chip, Table16Config};
    #[derive(Clone, Debug, Default)]
    struct DecomposeAbcdCircuit;
    impl Circuit<Fp> for DecomposeAbcdCircuit {
        type Config = Table16Config;
        type FloorPlanner = V1;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            self.clone()
        }
        fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
            Table16Chip::<Fp>::configure(meta)
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fp>,
        ) -> Result<(), Error> {
            Table16Chip::<Fp>::load(config.clone(), &mut layouter)?;
            layouter.assign_region(
                || "isolated IV A decomposition",
                |mut region| {
                    config.compression.decompose_iv_a(&mut region)?;
                    Ok(())
                },
            )
        }
    }
    #[test]
    fn decompose_abcd_assignment_matches_gate() {
        MockProver::run(17, &DecomposeAbcdCircuit, vec![])
            .expect("isolated decompose synthesis")
            .assert_satisfied();
    }
}
#[cfg(test)]
mod tests {
    use ff::PrimeField;
    use halo2_proofs::halo2curves::pasta::pallas;
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use sha2::Digest;
    use super::super::{super::BLOCK_SIZE, Table16Chip, Table16Config, msg_schedule_test_input};
    use crate::zk::kagemusha_sha256_table16_v4::{BlockWord, IV, ROUND_CONSTANTS};
    fn host_compress(initial: [u32; 8], round_constants: [u32; 64]) -> ([u32; 8], [u32; 8]) {
        let mut schedule = [0_u32; 64];
        schedule[0] = 0x6162_6380;
        schedule[15] = 24;
        for index in 16..64 {
            let sigma0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let sigma1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(sigma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(sigma1);
        }
        let mut working = initial;
        for round in 0..64 {
            let [a, b, c, d, e, f, g, h] = working;
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choose)
                .wrapping_add(round_constants[round])
                .wrapping_add(schedule[round]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = sigma0.wrapping_add(majority);
            working = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        (
            std::array::from_fn(|index| initial[index].wrapping_add(working[index])),
            working,
        )
    }
    #[derive(Clone)]
    struct BadCompressionCircuit {
        expected_words: Vec<(usize, u64)>,
    }
    impl<F: PrimeField> Circuit<F> for BadCompressionCircuit {
        type Config = Table16Config;
        type FloorPlanner = V1;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            self.clone()
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
            let (_, halves) = config
                .message_schedule
                .process(&mut layouter, msg_schedule_test_input())?;
            let state = config.compression.initialize_with_iv(&mut layouter)?;
            let state = config.compression.compress(
                &mut layouter,
                state,
                halves,
                &config.compression.lookup,
            )?;
            let digest = config.compression.digest(&mut layouter, state)?;
            layouter.assign_region(
                || "bind an adversarial compression result",
                |mut region| {
                    for (index, expected) in &self.expected_words {
                        region.constrain_constant(digest[*index].cell(), F::from(*expected))?;
                    }
                    Ok(())
                },
            )
        }
    }
    fn assert_bad_compression_words_rejected(expected_words: Vec<(usize, u64)>) {
        let prover =
            MockProver::<pallas::Base>::run(17, &BadCompressionCircuit { expected_words }, vec![])
                .expect("adversarial compression synthesis");
        assert!(prover.verify().is_err());
    }
    #[test]
    fn compress() {
        struct MyCircuit;
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
                Table16Chip::load(config.clone(), &mut layouter)?;
                // Test vector: "abc"
                let input: [BlockWord; BLOCK_SIZE] = msg_schedule_test_input();
                let (_, w_halves) = config.message_schedule.process(&mut layouter, input)?;
                let compression = config.compression.clone();
                let lookup_inputs = &compression.lookup;
                let initial_state = compression.initialize_with_iv(&mut layouter)?;
                let state = config.compression.compress(
                    &mut layouter,
                    initial_state,
                    w_halves,
                    lookup_inputs,
                )?;
                let hash_output = sha2::Sha256::digest("abc");
                let expected_result: Vec<u32> = hash_output
                    .chunks(4)
                    .map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()))
                    .collect();
                let digest = config.compression.digest(&mut layouter, state)?;
                for (idx, word) in digest.iter().enumerate() {
                    word.value().assert_if_known(|digest_word| {
                        u32::from(*digest_word) == expected_result[idx]
                    });
                }
                Ok(())
            }
        }
        let circuit = MyCircuit {};
        let prover = match MockProver::<pallas::Base>::run(17, &circuit, vec![]) {
            Ok(prover) => prover,
            Err(e) => panic!("{:?}", e),
        };
        assert_eq!(prover.verify(), Ok(()));
    }
    #[test]
    fn mutated_iv_is_rejected() {
        let mut mutated = IV;
        mutated[0] ^= 1;
        let (digest, _) = host_compress(mutated, ROUND_CONSTANTS);
        assert_ne!(digest, host_compress(IV, ROUND_CONSTANTS).0);
        assert_bad_compression_words_rejected(vec![(0, u64::from(digest[0]))]);
    }
    #[test]
    fn mutated_round_constant_is_rejected() {
        let mut mutated = ROUND_CONSTANTS;
        mutated[0] ^= 1;
        let (digest, _) = host_compress(IV, mutated);
        assert_ne!(digest, host_compress(IV, ROUND_CONSTANTS).0);
        assert_bad_compression_words_rejected(vec![(0, u64::from(digest[0]))]);
    }
    #[test]
    fn omitted_feed_forward_is_rejected() {
        let (digest, working) = host_compress(IV, ROUND_CONSTANTS);
        assert_ne!(working[0], digest[0]);
        assert_ne!(working[4], digest[4]);
        assert_bad_compression_words_rejected(vec![
            (0, u64::from(working[0])),
            (4, u64::from(working[4])),
        ]);
    }
    #[test]
    fn unreduced_final_word_carry_is_rejected() {
        let digest = host_compress(IV, ROUND_CONSTANTS).0;
        assert_bad_compression_words_rejected(vec![
            (0, u64::from(digest[0]) + (1_u64 << 32)),
            (4, u64::from(digest[4]) + (1_u64 << 32)),
        ]);
    }
}
