// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Zero-knowledge circuit implementing the algebraic checks of the Spartan verifier logic.
//!
//! [`VegaVerifierCircuit`] constrains the complete Spartan verification across
//! `outer_len + inner_len + 3` rounds: outer sum-check, inner sum-check, and bridging rounds.
//!
//! Note: This circuit only encodes the algebraic checks of the verifier. It does **not**
//! encode the Fiat-Shamir challenge generation, so no proof composition is performed here.
use crate::traits::{Engine, circuit::MultiRoundCircuit};
use bellpepper_core::{ConstraintSystem, SynthesisError, num::AllocatedNum};
use ff::Field;

/// Evaluates a polynomial using Horner's method within R1CS constraints.
fn eval_poly_horner<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    coeffs: &[AllocatedNum<E::Scalar>],
    x: &AllocatedNum<E::Scalar>,
) -> Result<AllocatedNum<E::Scalar>, SynthesisError> {
    // Start from highest coefficient.
    let mut acc = coeffs.last().unwrap().clone(); // degree >= 1 in practice
    // We iterate from degree-1 down to 0 (excluding last which we already used).
    for (i, c_i) in coeffs.iter().rev().skip(1).enumerate() {
        // new_acc = acc * x + c_i
        let new_acc = AllocatedNum::alloc(cs.namespace(|| format!("new_acc_{i}")), || {
            let a = acc.get_value().unwrap_or(E::Scalar::ZERO);
            let xv = x.get_value().unwrap_or(E::Scalar::ZERO);
            let c = c_i.get_value().unwrap_or(E::Scalar::ZERO);
            Ok(a * xv + c)
        })?;
        cs.enforce(
            || format!("enforce_new_acc_{i}"),
            |lc| lc + acc.get_variable(),
            |lc| lc + x.get_variable(),
            |lc| lc + new_acc.get_variable() - c_i.get_variable(),
        );

        acc = new_acc;
    }
    Ok(acc)
}

/// Allocates a new variable fixed to zero and enforces the constraint `z = 0`.
fn alloc_zero<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
) -> Result<AllocatedNum<E::Scalar>, SynthesisError> {
    // Allocate with value 0
    let z = AllocatedNum::alloc(cs.namespace(|| "zero"), || Ok(E::Scalar::ZERO))?;
    // Enforce z * 1 = 0
    cs.enforce(
        || "z_is_zero",
        |lc| lc + z.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc, // constant 0
    );
    Ok(z)
}

/// Helper function to allocate polynomial coefficients in R1CS constraints.
fn alloc_coeffs<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    coeffs: &[E::Scalar],
) -> Result<Vec<AllocatedNum<E::Scalar>>, SynthesisError> {
    coeffs
        .iter()
        .enumerate()
        .map(|(i, c)| AllocatedNum::alloc(cs.namespace(|| format!("coef_{i}")), || Ok(*c)))
        .collect::<Result<Vec<_>, _>>()
}

/// Enforces that the sum of polynomial coefficients equals the claimed value.
///
/// This constraint verifies that `poly[0] + poly[1] + ... + poly[n-1] + poly[0] = claim`.
/// The doubling of `poly[0]` is intentional and corresponds to the sum-check protocol
/// where the polynomial evaluated at 0 and 1 should sum to the claim.
///
/// # Arguments
/// * `cs` - Constraint system for enforcing the constraint
/// * `poly` - Polynomial coefficients as allocated numbers
/// * `claim` - The claimed sum value
fn enforce_sc_claim<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    poly: &[AllocatedNum<E::Scalar>],
    claim: &AllocatedNum<E::Scalar>,
) -> Result<(), SynthesisError> {
    // Enforce that the sum of coefficients equals the claim
    cs.enforce(
        || "sum_of_coeffs_equals_claim",
        |lc| {
            poly.iter()
                .map(|p| p.get_variable())
                .fold(lc, |lc, p| lc + p)
                + poly[0].get_variable()
        },
        |lc| lc + CS::one(),
        |lc| lc + claim.get_variable(),
    );
    Ok(())
}

/// Enforces the final check of the outer sum-check of Spartan:
/// prev_claim = tau_at_rx * (claim_Az*claim_Bz - claim_Cz)
fn enforce_outer_sc_final_check<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    claim_Az: &AllocatedNum<E::Scalar>,
    claim_Bz: &AllocatedNum<E::Scalar>,
    claim_Cz: &AllocatedNum<E::Scalar>,
    tau_at_rx: &AllocatedNum<E::Scalar>,
    prev_claim: &AllocatedNum<E::Scalar>,
) -> Result<(), SynthesisError> {
    // prod_Az_Bz = claim_Az * claim_Bz
    let prod_Az_Bz = claim_Az.mul(cs.namespace(|| "AzBz"), claim_Bz)?;

    // prev_claim = tau_at_rx * (prod_AzBz - Cz)
    cs.enforce(
        || "prev_claim = tau_at_rx*(prod_AzBz - Cz)",
        |lc| lc + tau_at_rx.get_variable(),
        |lc| lc + prod_Az_Bz.get_variable() - claim_Cz.get_variable(),
        |lc| lc + prev_claim.get_variable(),
    );

    Ok(())
}

/// Computes joint_claim = Az + r*Bz + r^2*Cz and returns joint_claim
fn compute_joint_claim<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    Az: &AllocatedNum<E::Scalar>,
    Bz: &AllocatedNum<E::Scalar>,
    Cz: &AllocatedNum<E::Scalar>,
    r: &AllocatedNum<E::Scalar>,
    r_sq: &AllocatedNum<E::Scalar>,
) -> Result<AllocatedNum<E::Scalar>, SynthesisError> {
    // r_times_Bz = Bz * r
    let r_times_Bz = r.mul(cs.namespace(|| "r_times_Bz"), Bz)?;

    // joint = Az + r_times_claim_Bz + r_sq * claim_Cz
    // We instead check: joint - Az - r_times_claim_Bz = r_sq * claim_Cz
    let joint = AllocatedNum::alloc(cs.namespace(|| "joint"), || {
        let a = Az.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        let t1 = r_times_Bz
            .get_value()
            .ok_or(SynthesisError::AssignmentMissing)?;
        let rsq = r_sq.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        let c = Cz.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        Ok(a + t1 + rsq * c)
    })?;
    cs.enforce(
        || "joint_enf",
        |lc| lc + Cz.get_variable(),
        |lc| lc + r_sq.get_variable(),
        |lc| lc + joint.get_variable() - Az.get_variable() - r_times_Bz.get_variable(),
    );
    Ok(joint)
}

/// Enforces the final check of the inner sum-check of Spartan:
/// eval_z \gets (1-r_y0)*eval_W + r_y0*eval_X
/// quotient = prev_claim / eval_z
/// inputize(quotient)
fn enforce_inner_sc_final_check<E: Engine, CS: ConstraintSystem<E::Scalar>>(
    mut cs: CS,
    r_y0: &AllocatedNum<E::Scalar>,
    eval_W: &AllocatedNum<E::Scalar>,
    eval_X: &AllocatedNum<E::Scalar>,
    prev_claim: &AllocatedNum<E::Scalar>,
) -> Result<(), SynthesisError> {
    // tmp_w = eval_W * (1 - ry0)
    let tmp_w = AllocatedNum::alloc(cs.namespace(|| "tmp_w"), || {
        let ew = eval_W.get_value().unwrap_or(E::Scalar::ZERO);
        let om = E::Scalar::ONE - r_y0.get_value().unwrap_or(E::Scalar::ZERO);
        Ok(ew * om)
    })?;
    cs.enforce(
        || "tmp_w_def",
        |lc| lc + eval_W.get_variable(),
        |lc| lc + CS::one() - r_y0.get_variable(),
        |lc| lc + tmp_w.get_variable(),
    );

    // sum_z_expected = tmp_w + eval_X * r_y0
    // sum_z_expected - tmp_w = eval_X * r_y0
    let sum_z_expected = AllocatedNum::alloc(cs.namespace(|| "sum_z_expected"), || {
        let tw = tmp_w.get_value().unwrap_or(E::Scalar::ZERO);
        let tx = eval_X.get_value().unwrap_or(E::Scalar::ZERO);
        Ok(tw + tx * r_y0.get_value().unwrap_or(E::Scalar::ZERO))
    })?;
    cs.enforce(
        || "sum_z_expected_def",
        |lc| lc + eval_X.get_variable(),
        |lc| lc + r_y0.get_variable(),
        |lc| lc + sum_z_expected.get_variable() - tmp_w.get_variable(),
    );

    // prev_e = (eval_A + r * eval_B + r^2 * eval_C) * sum_z_expected
    // we compute prev_e / sum_z_expected and inputize that
    // The verifier can compute eval_A + r * eval_B + r^2 * eval_C and check equality
    let quotient = AllocatedNum::alloc_input(cs.namespace(|| "quotient"), || {
        let prev_claim_v = prev_claim
            .get_value()
            .ok_or(SynthesisError::AssignmentMissing)?;
        let se_v = sum_z_expected
            .get_value()
            .ok_or(SynthesisError::AssignmentMissing)?;
        Ok(if se_v.is_zero().into() {
            E::Scalar::ZERO
        } else {
            prev_claim_v * se_v.invert().unwrap()
        })
    })?;

    // check that quotient * sum_z_expected = prev_e
    cs.enforce(
        || "check_quotient",
        |lc| lc + quotient.get_variable(),
        |lc| lc + sum_z_expected.get_variable(),
        |lc| lc + prev_claim.get_variable(),
    );

    Ok(())
}

/// Circuit constraining Spartan verifier computation across multiple rounds.
#[derive(Clone, Debug)]
pub struct VegaVerifierCircuit<E: Engine> {
    pub(crate) outer_polys: Vec<[E::Scalar; 4]>,
    pub(crate) claim_Az: E::Scalar,
    pub(crate) claim_Bz: E::Scalar,
    pub(crate) claim_Cz: E::Scalar,
    pub(crate) tau_at_rx: E::Scalar,
    pub(crate) inner_polys: Vec<[E::Scalar; 3]>,
    pub(crate) eval_W: E::Scalar,
    pub(crate) eval_X: E::Scalar,
    pub(crate) mr_commitment_width: usize,
}

impl<E: Engine> VegaVerifierCircuit<E> {
    pub fn default(num_rounds_x: usize, num_rounds_y: usize, mr_commitment_width: usize) -> Self {
        Self {
            outer_polys: vec![[E::Scalar::ZERO; 4]; num_rounds_x],
            claim_Az: E::Scalar::ZERO,
            claim_Bz: E::Scalar::ZERO,
            claim_Cz: E::Scalar::ZERO,
            tau_at_rx: E::Scalar::ZERO,
            inner_polys: vec![[E::Scalar::ZERO; 3]; num_rounds_y],
            eval_W: E::Scalar::ZERO,
            eval_X: E::Scalar::ZERO,
            mr_commitment_width,
        }
    }

    // number of outer sum-check rounds
    fn num_outer_rounds(&self) -> usize {
        self.outer_polys.len()
    }
    // number of inner sum-check rounds
    fn num_inner_rounds(&self) -> usize {
        self.inner_polys.len()
    }

    fn idx_outer_final(&self) -> usize {
        self.num_outer_rounds()
    }
    fn idx_inner_start(&self) -> usize {
        self.idx_outer_final() + 1
    }
    fn idx_inner_final(&self) -> usize {
        self.idx_inner_start() + self.num_inner_rounds()
    }
    fn idx_commit_w(&self) -> usize {
        self.idx_inner_final() + 1
    }
}

impl<E: Engine> MultiRoundCircuit<E> for VegaVerifierCircuit<E> {
    fn num_challenges(&self, round_index: usize) -> Result<usize, SynthesisError> {
        if round_index < self.idx_inner_final() {
            Ok(1)
        } else if round_index == self.idx_inner_final() || round_index == self.idx_commit_w() {
            Ok(0)
        } else {
            Err(SynthesisError::Unsatisfiable)
        }
    }

    fn rounds<CS: ConstraintSystem<E::Scalar>>(
        &self,
        cs: &mut CS,
        round_index: usize,
        prior_round_vars: &[Vec<AllocatedNum<E::Scalar>>],
        prev_challenges: &[Vec<AllocatedNum<E::Scalar>>],
        challenges: Option<&[E::Scalar]>,
    ) -> Result<(Vec<AllocatedNum<E::Scalar>>, Vec<AllocatedNum<E::Scalar>>), SynthesisError> {
        // outer cubic sum-check rounds
        if round_index < self.idx_outer_final() {
            // allocate polynomial sent in this round
            let poly = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("outer_sc_coeffs_round_{round_index}")),
                &self.outer_polys[round_index],
            )?;

            let claim = if round_index == 0 {
                alloc_zero::<E, _>(cs.namespace(|| "initial_claim_zero"))
            } else {
                // allocate challenge r_x[round_index]
                let r_x_idx = AllocatedNum::alloc_input(
                    cs.namespace(|| format!("r_x_{round_index}")),
                    || Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO)),
                )?;

                eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("outer_prev_eval_{round_index}")),
                    &prior_round_vars[round_index - 1],
                    &r_x_idx,
                )
            }?;

            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("outer_sc_claim_check {round_index}")),
                &poly,
                &claim,
            )?;

            Ok((poly, vec![]))
        } else if round_index == self.idx_outer_final() {
            // Compute claim = poly(r) from last round's coefficients and the new challenge r
            let r =
                AllocatedNum::alloc_input(cs.namespace(|| format!("r_x_{round_index}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;
            let claim = eval_poly_horner::<E, _>(
                cs.namespace(|| "outer_final_prev_eval"),
                &prior_round_vars[round_index - 1],
                &r,
            )?;

            let claim_Az = AllocatedNum::alloc(cs.namespace(|| "Az_outer"), || Ok(self.claim_Az))?;
            let claim_Bz = AllocatedNum::alloc(cs.namespace(|| "Bz_outer"), || Ok(self.claim_Bz))?;
            let claim_Cz = AllocatedNum::alloc(cs.namespace(|| "Cz_outer"), || Ok(self.claim_Cz))?;
            let tau_at_rx =
                AllocatedNum::alloc(cs.namespace(|| "tau_at_rx_outer"), || Ok(self.tau_at_rx))?;

            // Final outer equality: claim == tau_at_rx * (claim_Az*claim_Bz - claim_Cz)
            enforce_outer_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_outer_final_check"),
                &claim_Az,
                &claim_Bz,
                &claim_Cz,
                &tau_at_rx,
                &claim,
            )?;

            Ok((
                vec![
                    claim_Az.clone(),
                    claim_Bz.clone(),
                    claim_Cz.clone(),
                    tau_at_rx.clone(),
                ],
                vec![],
            ))
        } else if round_index >= self.idx_inner_start() && round_index < self.idx_inner_final() {
            // Inner quadratic sum-check per-round consistency
            let idx = round_index - self.idx_inner_start();

            // Allocate current round's poly
            let poly = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("inner_round_{idx}")),
                &self.inner_polys[idx],
            )?;

            let r = AllocatedNum::alloc_input(cs.namespace(|| format!("r {round_index}")), || {
                Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
            })?;

            let claim = if idx == 0 {
                let r_sq = r.square(cs.namespace(|| "r_sq"))?;

                let claims_outer = &prior_round_vars[self.idx_outer_final()];
                compute_joint_claim::<E, _>(
                    cs.namespace(|| "compute_inner_joint_claim"),
                    &claims_outer[0], // claim_Az
                    &claims_outer[1], // claim_Bz
                    &claims_outer[2], // claim_Cz
                    &r,
                    &r_sq,
                )
            } else {
                let prev_poly = &prior_round_vars[round_index - 1];
                eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("inner_prev_eval_{idx}")),
                    prev_poly,
                    &r,
                )
            }?;

            // Enforce 2*c0+c1+c2 == claim
            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("inner_sc_claim_check {idx}")),
                &poly,
                &claim,
            )?;

            // Expose this round's coefficients; next round will consume them
            Ok((poly, vec![r]))
        } else if round_index == self.idx_inner_final() {
            let prev_poly = &prior_round_vars[round_index - 1];
            let r_y_idx =
                AllocatedNum::alloc_input(cs.namespace(|| format!("r_y_{round_index}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;
            let claim = eval_poly_horner::<E, _>(
                cs.namespace(|| format!("inner_prev_eval_{round_index}")),
                prev_poly,
                &r_y_idx,
            )?;

            let eval_W = AllocatedNum::alloc(cs.namespace(|| "eval_W"), || Ok(self.eval_W))?;

            let tau_at_rx = prior_round_vars[self.idx_outer_final()][3].clone();
            tau_at_rx.inputize(cs.namespace(|| "inputize_tau_at_rx"))?;

            let eval_X = AllocatedNum::alloc_input(cs.namespace(|| "eval_X"), || Ok(self.eval_X))?;
            let r_y0 = &prev_challenges[self.idx_inner_start() + 1][0];

            enforce_inner_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_inner_final_check"),
                r_y0,
                &eval_W,
                &eval_X,
                &claim,
            )?;

            Ok((vec![eval_W], vec![]))
        } else if round_index == self.idx_commit_w() {
            // Dedicated commit round for eval_W only (padded to commitment width)
            let eval_W =
                AllocatedNum::alloc(cs.namespace(|| "eval_W_dedicated"), || Ok(self.eval_W))?;

            // enforce that eval_W matches the one from prior round
            let eval_W_prev = &prior_round_vars[round_index - 1][0];
            cs.enforce(
                || "eval_W_match",
                |lc| lc + eval_W.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + eval_W_prev.get_variable(),
            );

            // Pad to width
            for j in 0..self.mr_commitment_width - 1 {
                alloc_zero::<E, _>(cs.namespace(|| format!("pad_eval_W_{j}")))?;
            }
            Ok((vec![eval_W], vec![]))
        } else {
            Err(SynthesisError::Unsatisfiable)
        }
    }

    fn num_rounds(&self) -> usize {
        self.idx_commit_w() + 1
    }

    fn commitment_width(&self) -> usize {
        self.mr_commitment_width
    }
}

/// NeutronNova verifier circuit constraining computation across multiple rounds.
#[derive(Clone, Debug)]
pub struct VegaMcVerifierCircuit<E: Engine> {
    // NeutronNova folding scheme verifier state across multiple rounds
    // NIFS cubic sum-check polynomials (4 coeffs per round)
    pub(crate) nifs_polys: Vec<[E::Scalar; 4]>,
    // Accumulated equality value acc_eq = \prod_t eq(r_b_t; rho_t), provided as a public input
    pub(crate) eq_rho_at_rb: E::Scalar,
    pub(crate) t_out_step: E::Scalar,

    // Spartan sum-check proof for step and core circuits
    pub(crate) outer_polys_step: Vec<[E::Scalar; 4]>,
    pub(crate) outer_polys_core: Vec<[E::Scalar; 4]>,

    pub(crate) claim_Az_step: E::Scalar,
    pub(crate) claim_Bz_step: E::Scalar,
    pub(crate) claim_Cz_step: E::Scalar,

    pub(crate) claim_Az_core: E::Scalar,
    pub(crate) claim_Bz_core: E::Scalar,
    pub(crate) claim_Cz_core: E::Scalar,

    // evaluation of tau at rx
    pub(crate) tau_at_rx: E::Scalar,

    pub(crate) inner_polys_step: Vec<[E::Scalar; 3]>,
    pub(crate) inner_polys_core: Vec<[E::Scalar; 3]>,

    pub(crate) eval_W_step: E::Scalar,
    pub(crate) eval_W_core: E::Scalar,
    pub(crate) eval_X_step: E::Scalar,
    pub(crate) eval_X_core: E::Scalar,

    pub(crate) mr_commitment_width: usize,
}

impl<E: Engine> VegaMcVerifierCircuit<E> {
    /// Creates a default instance of the NeutronNova verifier circuit with zeroed fields.
    pub fn default(
        num_rounds_z: usize,
        num_rounds_x: usize,
        num_rounds_y: usize,
        mr_commitment_width: usize,
    ) -> Self {
        Self {
            outer_polys_step: vec![[E::Scalar::ZERO; 4]; num_rounds_x],
            outer_polys_core: vec![[E::Scalar::ZERO; 4]; num_rounds_x],
            claim_Az_step: E::Scalar::ZERO,
            claim_Bz_step: E::Scalar::ZERO,
            claim_Cz_step: E::Scalar::ZERO,
            claim_Az_core: E::Scalar::ZERO,
            claim_Bz_core: E::Scalar::ZERO,
            claim_Cz_core: E::Scalar::ZERO,
            tau_at_rx: E::Scalar::ZERO,
            inner_polys_step: vec![[E::Scalar::ZERO; 3]; num_rounds_y],
            inner_polys_core: vec![[E::Scalar::ZERO; 3]; num_rounds_y],
            eval_W_step: E::Scalar::ZERO,
            eval_W_core: E::Scalar::ZERO,
            eval_X_step: E::Scalar::ZERO,
            eval_X_core: E::Scalar::ZERO,
            t_out_step: E::Scalar::ZERO,
            nifs_polys: vec![[E::Scalar::ZERO; 4]; num_rounds_z],
            eq_rho_at_rb: E::Scalar::ZERO,
            mr_commitment_width,
        }
    }

    // Number of NIFS rounds
    fn num_nifs_rounds(&self) -> usize {
        self.nifs_polys.len()
    }
    // Number of outer sum-check rounds (same for step and core)
    fn num_outer_rounds(&self) -> usize {
        self.outer_polys_step.len()
    }
    // Number of inner sum-check rounds (same for step and core)
    fn num_inner_rounds(&self) -> usize {
        self.inner_polys_step.len()
    }

    fn idx_nifs_final(&self) -> usize {
        self.num_nifs_rounds()
    }

    fn idx_outer_start(&self) -> usize {
        self.idx_nifs_final() + 1
    }

    fn idx_outer_final(&self) -> usize {
        self.idx_outer_start() + self.num_outer_rounds()
    }

    fn idx_inner_start(&self) -> usize {
        self.idx_outer_final() + 1
    }

    fn idx_inner_final(&self) -> usize {
        self.idx_inner_start() + self.num_inner_rounds()
    }

    /// Returns the round index at which the circuit commits only to `eval_W` for the step circuit.
    fn idx_commit_w_step(&self) -> usize {
        self.idx_inner_final() + 1
    }

    /// Returns the round index at which the circuit commits only to `eval_W` for the core circuit.
    fn idx_commit_w_core(&self) -> usize {
        self.idx_commit_w_step() + 1
    }
}

impl<E: Engine> MultiRoundCircuit<E> for VegaMcVerifierCircuit<E> {
    fn num_challenges(&self, round_index: usize) -> Result<usize, SynthesisError> {
        if round_index < self.num_nifs_rounds() {
            Ok(1)
        } else if round_index == self.idx_nifs_final() {
            Ok(0) // no challenge after the NIFS final round and for the first round of outer sum-check
        } else if round_index < self.idx_inner_final() {
            Ok(1)
        } else if round_index == self.idx_inner_final()
            || round_index == self.idx_commit_w_step()
            || round_index == self.idx_commit_w_core()
        {
            Ok(0)
        } else {
            Err(SynthesisError::Unsatisfiable)
        }
    }

    fn rounds<CS: ConstraintSystem<E::Scalar>>(
        &self,
        cs: &mut CS,
        round_index: usize,
        prior_round_vars: &[Vec<AllocatedNum<E::Scalar>>],
        prev_challenges: &[Vec<AllocatedNum<E::Scalar>>],
        challenges: Option<&[E::Scalar]>,
    ) -> Result<(Vec<AllocatedNum<E::Scalar>>, Vec<AllocatedNum<E::Scalar>>), SynthesisError> {
        // NIFS cubic sum-check rounds
        if round_index < self.num_nifs_rounds() {
            // allocate polynomial sent in this round
            let poly = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("nifs_round_{round_index}")),
                &self.nifs_polys[round_index],
            )?;

            let claim = if round_index == 0 {
                alloc_zero::<E, _>(cs.namespace(|| "initial_claim_zero"))
            } else {
                // allocate challenge r_x[round_index]
                let r = AllocatedNum::alloc_input(
                    cs.namespace(|| format!("r_z_{round_index}")),
                    || Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO)),
                )?;

                eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("nifs_prev_eval_{round_index}")),
                    &prior_round_vars[round_index - 1],
                    &r,
                )
            }?;

            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("nifs_sc_claim_check {round_index}")),
                &poly,
                &claim,
            )?;

            Ok((poly, vec![]))
        } else if round_index == self.idx_nifs_final() {
            let r =
                AllocatedNum::alloc_input(cs.namespace(|| format!("r_z_{round_index}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;
            let claim = eval_poly_horner::<E, _>(
                cs.namespace(|| "nifs_final_prev_eval"),
                &prior_round_vars[round_index - 1],
                &r,
            )?;

            let t_out_step =
                AllocatedNum::alloc(cs.namespace(|| "t_out_step"), || Ok(self.t_out_step))?;
            let eq_rho_at_rb =
                AllocatedNum::alloc(cs.namespace(|| "eq_rho_at_rb"), || Ok(self.eq_rho_at_rb))?;

            // rho_acc * t_out_step = claim
            cs.enforce(
                || "nifs_final_eq",
                |lc| lc + eq_rho_at_rb.get_variable(),
                |lc| lc + t_out_step.get_variable(),
                |lc| lc + claim.get_variable(),
            );

            // Expose eq_rho_at_rb so later rounds (inner-final) can inputize it as a public value without re-allocation
            Ok((vec![eq_rho_at_rb.clone(), t_out_step.clone()], vec![]))
        } else if round_index > self.idx_nifs_final() && round_index < self.idx_outer_final() {
            let i = round_index - self.idx_outer_start();

            let poly_step = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("outer_step_{i}")),
                &self.outer_polys_step[i],
            )?;
            let poly_core = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("outer_core_{i}")),
                &self.outer_polys_core[i],
            )?;

            // for core branch, claim = 0 if i == 0 else eval at r of previous round's polynomial
            // for step branch, claim = t_out_step if i == 0 else eval at r of previous round's polynomial
            let (claim_step, claim_core) = if i == 0 {
                let claim_step = prior_round_vars[round_index - 1][1].clone();
                let claim_core = alloc_zero::<E, _>(cs.namespace(|| "initial_claim_core_zero"))?;

                (claim_step, claim_core)
            } else {
                let r = AllocatedNum::alloc_input(cs.namespace(|| format!("r_x_{i}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;

                let claim_step = eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("outer_step_prev_eval_{i}")),
                    &prior_round_vars[round_index - 1][0..4], // cubic polynomial
                    &r,
                )?;

                let claim_core = eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("outer_core_prev_eval_{i}")),
                    &prior_round_vars[round_index - 1][4..8], // cubic polynomial
                    &r,
                )?;

                (claim_step, claim_core)
            };

            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("outer_sc_claim_check_step_{i}")),
                &poly_step,
                &claim_step,
            )?;
            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("outer_sc_claim_check_core_{i}")),
                &poly_core,
                &claim_core,
            )?;

            Ok(([poly_step, poly_core].concat(), vec![]))
        } else if round_index == self.idx_outer_final() {
            let r =
                AllocatedNum::alloc_input(cs.namespace(|| format!("r_x_{round_index}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;
            let claim_step = eval_poly_horner::<E, _>(
                cs.namespace(|| "outer_final_prev_eval"),
                &prior_round_vars[round_index - 1][0..4], // cubic polynomial
                &r,
            )?;
            let claim_core = eval_poly_horner::<E, _>(
                cs.namespace(|| "outer_final_prev_eval_core"),
                &prior_round_vars[round_index - 1][4..8], // cubic polynomial
                &r,
            )?;

            let claim_Az_step =
                AllocatedNum::alloc(cs.namespace(|| "Az_step"), || Ok(self.claim_Az_step))?;
            let claim_Bz_step =
                AllocatedNum::alloc(cs.namespace(|| "Bz_step"), || Ok(self.claim_Bz_step))?;
            let claim_Cz_step =
                AllocatedNum::alloc(cs.namespace(|| "Cz_step"), || Ok(self.claim_Cz_step))?;

            let claim_Az_core =
                AllocatedNum::alloc(cs.namespace(|| "Az_core"), || Ok(self.claim_Az_core))?;
            let claim_Bz_core =
                AllocatedNum::alloc(cs.namespace(|| "Bz_core"), || Ok(self.claim_Bz_core))?;
            let claim_Cz_core =
                AllocatedNum::alloc(cs.namespace(|| "Cz_core"), || Ok(self.claim_Cz_core))?;

            let tau_at_rx =
                AllocatedNum::alloc(cs.namespace(|| "tau_at_rx_outer"), || Ok(self.tau_at_rx))?;

            enforce_outer_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_outer_final_check"),
                &claim_Az_step,
                &claim_Bz_step,
                &claim_Cz_step,
                &tau_at_rx,
                &claim_step,
            )?;

            enforce_outer_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_outer_final_check_core"),
                &claim_Az_core,
                &claim_Bz_core,
                &claim_Cz_core,
                &tau_at_rx,
                &claim_core,
            )?;

            Ok((
                vec![
                    claim_Az_step,
                    claim_Bz_step,
                    claim_Cz_step,
                    claim_Az_core,
                    claim_Bz_core,
                    claim_Cz_core,
                    tau_at_rx.clone(),
                ],
                vec![],
            ))
        } else if round_index >= self.idx_inner_start() && round_index < self.idx_inner_final() {
            // Inner quadratic sum-check rounds
            let idx = round_index - self.idx_inner_start();

            let poly_step = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("inner_step_{idx}")),
                &self.inner_polys_step[idx],
            )?;
            let poly_core = alloc_coeffs::<E, _>(
                cs.namespace(|| format!("inner_core_{idx}")),
                &self.inner_polys_core[idx],
            )?;

            let r = AllocatedNum::alloc_input(cs.namespace(|| format!("r_y[{idx}]")), || {
                Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
            })?;

            let (claim_step, claim_core) = if idx == 0 {
                let r_sq = r.square(cs.namespace(|| "r_sq"))?;

                let claims_outer = &prior_round_vars[self.idx_outer_final()];
                let claim_step = compute_joint_claim::<E, _>(
                    cs.namespace(|| "compute_inner_joint_claim_step"),
                    &claims_outer[0], // claim_Az_step
                    &claims_outer[1], // claim_Bz_step
                    &claims_outer[2], // claim_Cz_step
                    &r,
                    &r_sq,
                )?;
                let claim_core = compute_joint_claim::<E, _>(
                    cs.namespace(|| "compute_inner_joint_claim_core"),
                    &claims_outer[3], // claim_Az_core
                    &claims_outer[4], // claim_Bz_core
                    &claims_outer[5], // claim_Cz_core
                    &r,
                    &r_sq,
                )?;

                (claim_step, claim_core)
            } else {
                let prev_poly_step = &prior_round_vars[round_index - 1][0..3]; // quadratic polynomial
                let prev_poly_core = &prior_round_vars[round_index - 1][3..6]; // quadratic polynomial

                let claim_step = eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("inner_step_prev_eval_{idx}")),
                    prev_poly_step,
                    &r,
                )?;
                let claim_core = eval_poly_horner::<E, _>(
                    cs.namespace(|| format!("inner_core_prev_eval_{idx}")),
                    prev_poly_core,
                    &r,
                )?;

                (claim_step, claim_core)
            };

            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("inner_sc_claim_check_step_{idx}")),
                &poly_step,
                &claim_step,
            )?;
            enforce_sc_claim::<E, _>(
                cs.namespace(|| format!("inner_sc_claim_check_core_{idx}")),
                &poly_core,
                &claim_core,
            )?;
            Ok(([poly_step, poly_core].concat(), vec![r]))
        } else if round_index == self.idx_inner_final() {
            let prev_poly_step = &prior_round_vars[round_index - 1][0..3]; // quadratic polynomial
            let prev_poly_core = &prior_round_vars[round_index - 1][3..6]; // quadratic polynomial
            let r =
                AllocatedNum::alloc_input(cs.namespace(|| format!("r_y_{round_index}")), || {
                    Ok(challenges.map(|c| c[0]).unwrap_or(E::Scalar::ZERO))
                })?;
            let claim_step = eval_poly_horner::<E, _>(
                cs.namespace(|| "inner_step_prev_eval"),
                prev_poly_step,
                &r,
            )?;
            let claim_core = eval_poly_horner::<E, _>(
                cs.namespace(|| "inner_core_prev_eval"),
                prev_poly_core,
                &r,
            )?;

            // tau_at_rx is a public input
            let tau_at_rx = prior_round_vars[self.idx_outer_final()][6].clone();
            tau_at_rx.inputize(cs.namespace(|| "tau_at_rx_inp"))?;

            let eval_X_step =
                AllocatedNum::alloc_input(cs.namespace(|| "eval_X_step"), || Ok(self.eval_X_step))?;
            let eval_X_core =
                AllocatedNum::alloc_input(cs.namespace(|| "eval_X_core"), || Ok(self.eval_X_core))?;

            let eq_rho_at_rb = &prior_round_vars[self.idx_nifs_final()][0];
            eq_rho_at_rb.inputize(cs.namespace(|| "eq_rho_at_rb_inp"))?;

            // allocate eval_W_step and eval_W_core and use it in the next rounds
            let eval_W_step =
                AllocatedNum::alloc(cs.namespace(|| "eval_W_step"), || Ok(self.eval_W_step))?;
            let eval_W_core =
                AllocatedNum::alloc(cs.namespace(|| "eval_W_core"), || Ok(self.eval_W_core))?;

            let r_y0 = &prev_challenges[self.idx_inner_start() + 1][0];
            enforce_inner_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_inner_final_check_step"),
                r_y0,
                &eval_W_step,
                &eval_X_step,
                &claim_step,
            )?;

            enforce_inner_sc_final_check::<E, _>(
                cs.namespace(|| "enforce_inner_final_check_core"),
                r_y0,
                &eval_W_core,
                &eval_X_core,
                &claim_core,
            )?;

            Ok((vec![eval_W_step.clone(), eval_W_core.clone()], vec![]))
        } else if round_index == self.idx_commit_w_step() {
            // Commit round for eval_W_step
            let eval_W_step = AllocatedNum::alloc(cs.namespace(|| "eval_W_step_commit"), || {
                Ok(self.eval_W_step)
            })?;

            // enforce that eval_W matches the one from prior round
            let eval_W_prev = &prior_round_vars[round_index - 1][0];
            cs.enforce(
                || "eval_W_step_match",
                |lc| lc + eval_W_step.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + eval_W_prev.get_variable(),
            );

            // Pad to per-round commitment width
            for j in 0..self.mr_commitment_width - 1 {
                alloc_zero::<E, _>(cs.namespace(|| format!("pad_eval_W_step_{j}")))?;
            }

            Ok((vec![], vec![]))
        } else if round_index == self.idx_commit_w_core() {
            // Commit round for eval_W_core
            let eval_W_core = AllocatedNum::alloc(cs.namespace(|| "eval_W_core_commit"), || {
                Ok(self.eval_W_core)
            })?;

            // enforce that eval_W matches the one from prior round
            let eval_W_prev = &prior_round_vars[round_index - 2][1];
            cs.enforce(
                || "eval_W_core_match",
                |lc| lc + eval_W_core.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + eval_W_prev.get_variable(),
            );

            // Pad to per-round commitment width
            for j in 0..self.mr_commitment_width - 1 {
                alloc_zero::<E, _>(cs.namespace(|| format!("pad_eval_W_core_{j}")))?;
            }
            Ok((vec![], vec![]))
        } else {
            Err(SynthesisError::Unsatisfiable)
        }
    }

    fn num_rounds(&self) -> usize {
        self.idx_commit_w_core() + 1
    }

    fn commitment_width(&self) -> usize {
        self.mr_commitment_width
    }
}
