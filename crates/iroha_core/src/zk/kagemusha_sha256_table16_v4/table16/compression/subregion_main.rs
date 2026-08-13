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
use halo2_proofs::{circuit::Region, plonk::Error};
use super::{
    super::{RoundWord, RoundWordA, RoundWordE, StateWord},
    CompressionConfig, State,
    compression_util::*,
};
use crate::zk::kagemusha_sha256_table16_v4::{AssignedBits, ROUND_CONSTANTS};
impl CompressionConfig {
    #[allow(clippy::many_single_char_names)]
    #[allow(clippy::too_many_arguments)]
    pub fn assign_round<F: PrimeField>(
        &self,
        region: &mut Region<'_, F>,
        round_idx: MainRoundIdx,
        previous_state: &State<F>,
        state: State<F>,
        schedule_word: &(AssignedBits<16, F>, AssignedBits<16, F>),
        assigned_halves: &mut Vec<AssignedBits<16, F>>,
    ) -> Result<State<F>, Error> {
        let a_3 = self.extras[0];
        let a_4 = self.extras[1];
        let a_7 = self.extras[3];
        let (a, b, c, d, e, f, g, h) = match_state(state);
        // s_upper_sigma_1(E)
        let sigma_1 = self.assign_upper_sigma_1(region, round_idx, e.pieces.clone().unwrap())?;
        // Ch(E, F, G)
        let ch = self.assign_ch(
            region,
            round_idx,
            e.spread_halves.clone().unwrap(),
            f.spread_halves.clone().unwrap(),
        )?;
        let ch_neg = self.assign_ch_neg(
            region,
            round_idx,
            e.spread_halves.clone().unwrap(),
            g.spread_halves.clone().unwrap(),
        )?;
        // s_upper_sigma_0(A)
        let sigma_0 = self.assign_upper_sigma_0(region, round_idx, a.pieces.clone().unwrap())?;
        // Maj(A, B, C)
        let maj = self.assign_maj(
            region,
            round_idx,
            a.spread_halves.clone().unwrap(),
            b.spread_halves.clone().unwrap(),
            c.spread_halves.clone().unwrap(),
        )?;
        // H' = H + Ch(E, F, G) + s_upper_sigma_1(E) + K + W
        let h_prime = self.assign_h_prime(
            region,
            round_idx,
            h,
            ch,
            ch_neg,
            sigma_1,
            ROUND_CONSTANTS[round_idx.as_usize()],
            schedule_word,
            assigned_halves,
        )?;
        // E_new = H' + D
        let e_new_dense = self.assign_e_new(region, round_idx, &d, &h_prime, assigned_halves)?;
        let e_new_val = e_new_dense.value();
        // A_new = H' + Maj(A, B, C) + sigma_0(A)
        let a_new_dense =
            self.assign_a_new(region, round_idx, maj, sigma_0, h_prime, assigned_halves)?;
        let a_new_val = a_new_dense.value();
        if round_idx < 63.into() {
            // Assign and copy A_new
            let a_new_row = get_decompose_a_row((round_idx + 1).into());
            a_new_dense
                .0
                .copy_advice(|| "a_new_lo", region, a_7, a_new_row)?;
            a_new_dense
                .1
                .copy_advice(|| "a_new_hi", region, a_7, a_new_row + 1)?;
            // Assign and copy E_new
            let e_new_row = get_decompose_e_row((round_idx + 1).into());
            e_new_dense
                .0
                .copy_advice(|| "e_new_lo", region, a_7, e_new_row)?;
            e_new_dense
                .1
                .copy_advice(|| "e_new_hi", region, a_7, e_new_row + 1)?;
            // Decompose A into (2, 11, 9, 10)-bit chunks
            let a_new = self.decompose_a(region, (round_idx + 1).into(), &a_new_dense)?;
            // Decompose E into (6, 5, 14, 7)-bit chunks
            let e_new = self.decompose_e(region, (round_idx + 1).into(), &e_new_dense)?;
            Ok(State::new(
                StateWord::A(a_new),
                StateWord::B(RoundWord::new(a.dense_halves, a.spread_halves)),
                StateWord::C(b),
                StateWord::D(c.dense_halves),
                StateWord::E(e_new),
                StateWord::F(RoundWord::new(e.dense_halves, e.spread_halves)),
                StateWord::G(f),
                StateWord::H(g.dense_halves),
            ))
        } else {
            let abcd_row = get_digest_abcd_row();
            let efgh_row = get_digest_efgh_row();
            let a_final =
                self.assign_word_halves_dense(region, abcd_row, a_3, abcd_row, a_4, a_new_val)?;
            // add the equality constraint for the newly assigned halves of A and the
            // previous ones
            region.constrain_equal(a_final.0.cell(), a_new_dense.0.cell());
            region.constrain_equal(a_final.1.cell(), a_new_dense.1.cell());
            let e_final =
                self.assign_word_halves_dense(region, efgh_row, a_3, efgh_row, a_4, e_new_val)?;
            // add the equality constraint for the newly assigned halves of E and the
            // previous ones
            region.constrain_equal(e_final.0.cell(), e_new_dense.0.cell());
            region.constrain_equal(e_final.1.cell(), e_new_dense.1.cell());
            // add the current state with the previous one
            let (prev_a, prev_b, prev_c, prev_d, prev_e, prev_f, prev_g, prev_h) =
                match_state(previous_state.clone());
            // previous A value
            let term1 = prev_a.dense_halves;
            // current A value
            let term2 = a_final;
            let row = get_state_addition_digest_row(StateRow::A);
            let a_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous B value
            let term1 = prev_b.dense_halves;
            // current B value is A
            let term2 = a.dense_halves;
            let row = get_state_addition_digest_row(StateRow::B);
            let b_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous C value
            let term1 = prev_c.dense_halves;
            // current C value is B
            let term2 = b.dense_halves;
            let row = get_state_addition_digest_row(StateRow::C);
            let c_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous D value
            let term1 = prev_d;
            // current D value is C
            let term2 = c.dense_halves.clone();
            let row = get_state_addition_digest_row(StateRow::D);
            let d_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous E value
            let term1 = prev_e.dense_halves;
            // current E value is e_final
            let term2 = e_final.clone();
            let row = get_state_addition_digest_row(StateRow::E);
            let e_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous F value
            let term1 = prev_f.dense_halves;
            // current F value is E
            let term2 = e.dense_halves;
            let row = get_state_addition_digest_row(StateRow::F);
            let f_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous G value
            let term1 = prev_g.dense_halves;
            // current G value is F
            let term2 = f.dense_halves;
            let row = get_state_addition_digest_row(StateRow::G);
            let g_state = self.assign_state_addition(region, row, term1, term2)?;
            // previous h value
            let term1 = prev_h;
            // current H value is G
            let term2 = g.dense_halves;
            let row = get_state_addition_digest_row(StateRow::H);
            let h_state = self.assign_state_addition(region, row, term1, term2)?;
            Ok(State::new(
                StateWord::A(RoundWordA::new_dense(a_state)),
                // here we do not use the correct spread halves to initialize this. This is ok
                // since only the dense halfs are needed for state initialization (the spread ones
                // are computed in it). Same in the following
                // TODO: Change the RoundWord struct to have optional spread parts
                StateWord::B(RoundWord::new(b_state, a.spread_halves)),
                StateWord::C(RoundWord::new(c_state, c.spread_halves)),
                StateWord::D(d_state),
                StateWord::E(RoundWordE::new_dense(e_state)),
                StateWord::F(RoundWord::new(f_state, f.spread_halves)),
                StateWord::G(RoundWord::new(g_state, g.spread_halves)),
                StateWord::H(h_state),
            ))
        }
    }
}
