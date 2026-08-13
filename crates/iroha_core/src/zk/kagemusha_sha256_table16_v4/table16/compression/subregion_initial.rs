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
    circuit::{Region, Value},
    plonk::Error,
};
use super::{
    super::{RoundWord, StateWord},
    CompressionConfig, RoundWordDense, State,
    compression_util::*,
};
use crate::zk::kagemusha_sha256_table16_v4::IV;
impl CompressionConfig {
    #[allow(clippy::many_single_char_names)]
    pub fn initialize_iv<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
    ) -> Result<State<F>, Error> {
        let a_7 = self.extras[3];
        // Decompose E into (6, 5, 14, 7)-bit chunks
        let e = self.decompose_iv_e(region)?;
        // Decompose F, G
        let f = self.decompose_iv_f(region)?;
        let g = self.decompose_iv_g(region)?;
        // Assign H
        let h = IV[7];
        let h_row = get_h_row(RoundIdx::Init);
        let assigned_h =
            self.assign_word_halves_dense_fixed(region, h_row, a_7, h_row + 1, a_7, h)?;
        // Decompose A into (2, 11, 9, 10)-bit chunks
        let a = self.decompose_iv_a(region)?;
        // Decompose B, C
        let b = self.decompose_iv_b(region)?;
        let c = self.decompose_iv_c(region)?;
        // Assign D
        let d = IV[3];
        let d_row = get_d_row(RoundIdx::Init);
        let assigned_d =
            self.assign_word_halves_dense_fixed(region, d_row, a_7, d_row + 1, a_7, d)?;
        Ok(State::new(
            StateWord::A(a),
            StateWord::B(b),
            StateWord::C(c),
            StateWord::D(assigned_d),
            StateWord::E(e),
            StateWord::F(f),
            StateWord::G(g),
            StateWord::H(assigned_h),
        ))
    }
    #[allow(clippy::many_single_char_names)]
    pub fn initialize_state<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        state: State<F>,
    ) -> Result<State<F>, Error> {
        let a_7 = self.extras[3];
        let (a, b, c, d, e, f, g, h) = match_state(state);
        // Decompose E into (6, 5, 14, 7)-bit chunks
        let e = self.decompose_e(region, RoundIdx::Init, &e.dense_halves)?;
        // Decompose F, G
        let f = self.decompose_f(region, InitialRound, &f.dense_halves)?;
        let g = self.decompose_g(region, InitialRound, &g.dense_halves)?;
        // Assign H
        let h_value = h.value();
        let h_row = get_h_row(RoundIdx::Init);
        let assigned_h =
            self.assign_word_halves_dense(region, h_row, a_7, h_row + 1, a_7, h_value)?;
        region.constrain_equal(h.0.cell(), assigned_h.0.cell());
        region.constrain_equal(h.1.cell(), assigned_h.1.cell());
        // Decompose A into (2, 11, 9, 10)-bit chunks
        let a = self.decompose_a(region, RoundIdx::Init, &a.dense_halves)?;
        // Decompose B, C
        let b = self.decompose_b(region, InitialRound, &b.dense_halves)?;
        let c = self.decompose_c(region, InitialRound, &c.dense_halves)?;
        // Assign D
        let d_value = d.value();
        let d_row = get_d_row(RoundIdx::Init);
        let assigned_d =
            self.assign_word_halves_dense(region, d_row, a_7, d_row + 1, a_7, d_value)?;
        region.constrain_equal(d.0.cell(), assigned_d.0.cell());
        region.constrain_equal(d.1.cell(), assigned_d.1.cell());
        Ok(State::new(
            StateWord::A(a),
            StateWord::B(b),
            StateWord::C(c),
            StateWord::D(d),
            StateWord::E(e),
            StateWord::F(f),
            StateWord::G(g),
            StateWord::H(h),
        ))
    }
    fn decompose_iv_b<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_b_row(InitialRound);
        let b = IV[1];
        let (dense_halves, spread_halves) = self.assign_word_halves_fixed(region, row, b)?;
        self.decompose_abcd(region, row, Value::known(b))?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_b<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        round_idx: InitialRound,
        assigned_b: &RoundWordDense<F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_b_row(round_idx);
        let b_value = assigned_b.value();
        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, b_value)?;
        region.constrain_equal(dense_halves.0.cell(), assigned_b.0.cell());
        region.constrain_equal(dense_halves.1.cell(), assigned_b.1.cell());
        self.decompose_abcd(region, row, b_value)?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_iv_c<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_c_row(InitialRound);
        let c = IV[2];
        let (dense_halves, spread_halves) = self.assign_word_halves_fixed(region, row, c)?;
        self.decompose_abcd(region, row, Value::known(c))?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_c<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        round_idx: InitialRound,
        assigned_c: &RoundWordDense<F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_c_row(round_idx);
        let c_value = assigned_c.value();
        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, c_value)?;
        region.constrain_equal(dense_halves.0.cell(), assigned_c.0.cell());
        region.constrain_equal(dense_halves.1.cell(), assigned_c.1.cell());
        self.decompose_abcd(region, row, c_value)?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_iv_f<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_f_row(InitialRound);
        let f = IV[5];
        let (dense_halves, spread_halves) = self.assign_word_halves_fixed(region, row, f)?;
        self.decompose_efgh(region, row, Value::known(f))?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_f<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        round_idx: InitialRound,
        assigned_f: &RoundWordDense<F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_f_row(round_idx);
        let f_value = assigned_f.value();
        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, f_value)?;
        region.constrain_equal(dense_halves.0.cell(), assigned_f.0.cell());
        region.constrain_equal(dense_halves.1.cell(), assigned_f.1.cell());
        self.decompose_efgh(region, row, f_value)?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_iv_g<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_g_row(InitialRound);
        let g = IV[6];
        let (dense_halves, spread_halves) = self.assign_word_halves_fixed(region, row, g)?;
        self.decompose_efgh(region, row, Value::known(g))?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
    fn decompose_g<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        round_idx: InitialRound,
        assigned_g: &RoundWordDense<F>,
    ) -> Result<RoundWord<F>, Error> {
        let row = get_decompose_g_row(round_idx);
        let g_value = assigned_g.value();
        let (dense_halves, spread_halves) = self.assign_word_halves(region, row, g_value)?;
        region.constrain_equal(dense_halves.0.cell(), assigned_g.0.cell());
        region.constrain_equal(dense_halves.1.cell(), assigned_g.1.cell());
        self.decompose_efgh(region, row, g_value)?;
        Ok(RoundWord::new(dense_halves, Some(spread_halves)))
    }
}
