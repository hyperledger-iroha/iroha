//! Non-ZK Spartan proof for relaxed R1CS instances.
//!
//! Proves that a relaxed R1CS instance `(comm_W, comm_E, u, X)` is satisfiable:
//!   Az * Bz = u*Cz + E,  where z = (W, u, X)
//!
//! Uses direct polynomial opening (RLC of Hyrax row commitments) instead of IPA,
//! sending only `width` field elements + 1 blind per opening. With width=32 this is
//! 33 scalars (~1 KB) per polynomial, far cheaper than IPA for both prover and verifier.
#![allow(non_snake_case)]
use crate::{
    CommitmentKey,
    errors::VegaError,
    math::Math,
    polys::{eq::EqPolynomial, multilinear::MultilinearPolynomial},
    r1cs::{R1CSShape, RelaxedR1CSInstance, RelaxedR1CSWitness, SparseMatrix},
    sumcheck::SumcheckProof,
    traits::{Engine, pcs::PCSEngineTrait, transcript::TranscriptEngineTrait},
};
use ff::Field;
use serde::{Deserialize, Serialize};

/// Compute sum_i rx[i] * M[i][j] for all j (transpose-multiply with eq weights).
fn bind_matrix_row_vars<E: Engine>(
    M: &SparseMatrix<E::Scalar>,
    rx: &[E::Scalar],
    num_cols: usize,
) -> Vec<E::Scalar> {
    let mut evals = vec![E::Scalar::ZERO; num_cols];
    for (row_idx, ptrs) in M.indptr.windows(2).enumerate() {
        let row_weight = rx[row_idx];
        if row_weight == E::Scalar::ZERO {
            continue;
        }
        for idx in ptrs[0]..ptrs[1] {
            let col = M.indices[idx];
            let val = M.data[idx];
            evals[col] += row_weight * val;
        }
    }
    evals
}

/// Compute rx^T * M * ry = sum_i sum_j rx[i] * M[i][j] * ry[j].
fn evaluate_matrix_with_tables<E: Engine>(
    M: &SparseMatrix<E::Scalar>,
    T_x: &[E::Scalar],
    T_y: &[E::Scalar],
) -> E::Scalar {
    M.indptr
        .windows(2)
        .enumerate()
        .map(|(row_idx, ptrs)| {
            let tx = T_x[row_idx];
            if tx == E::Scalar::ZERO {
                return E::Scalar::ZERO;
            }
            let row_sum: E::Scalar = (ptrs[0]..ptrs[1])
                .map(|idx| {
                    let col = M.indices[idx];
                    let val = M.data[idx];
                    T_y[col] * val
                })
                .sum();
            tx * row_sum
        })
        .sum()
}

/// A Spartan proof for a relaxed R1CS instance (non-ZK).
///
/// Instead of IPA proofs, uses direct openings: the prover sends the RLC'd vector
/// (of length `width`) and the combined blind for each polynomial.
///
/// # Soundness note
///
/// This crate-internal proof does NOT absorb `comm_W`/`comm_E` into its transcript --
/// only `(u, X)`. It is sound only when composed after a step (e.g., NIFS) that has
/// already bound the commitments to the transcript, and is intentionally not exposed
/// as a standalone proof system.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub(crate) struct RelaxedR1CSSpartanProof<E: Engine> {
    pub(crate) sc_proof_outer: SumcheckProof<E>,
    pub(crate) claims_outer: (E::Scalar, E::Scalar, E::Scalar),
    pub(crate) sc_proof_inner: SumcheckProof<E>,
    // Direct opening for W at r_y[1..]
    pub(crate) v_W: Vec<E::Scalar>,
    pub(crate) blind_W: E::Scalar,
    // Direct opening for E at r_x
    pub(crate) v_E: Vec<E::Scalar>,
    pub(crate) blind_E: E::Scalar,
}

impl<E: Engine> RelaxedR1CSSpartanProof<E> {
    /// Prove that a relaxed R1CS instance is satisfiable.
    ///
    /// The relation is: Az * Bz = u*Cz + E, where z = (W, u, X).
    /// Takes `u` and `X` separately to avoid computing folded commitments on the prover side.
    pub(crate) fn prove(
        S: &R1CSShape<E>,
        ck: &CommitmentKey<E>,
        u: &E::Scalar,
        X: &[E::Scalar],
        W: &RelaxedR1CSWitness<E>,
        transcript: &mut E::TE,
    ) -> Result<Self, VegaError> {
        transcript.absorb(b"u_relaxed", u);
        transcript.absorb(b"X_relaxed", &X);

        let num_cons = S.num_cons;
        let num_vars = S.num_vars;
        let num_rounds_x = num_cons.log_2();
        // Match standard Spartan: num_rounds_y = log2(num_vars_padded) + 1
        // z is padded to 2 * num_vars_padded so W fits entirely in the first half
        let num_vars_padded = num_vars.next_power_of_two();
        let num_rounds_y = num_vars_padded.log_2() + 1;
        let z_len = num_vars_padded * 2;

        // z = (W, u, X)
        let z_unpadded = [W.W.clone(), vec![*u], X.to_vec()].concat();

        let (Az, Bz, Cz) = S.multiply_vec(&z_unpadded)?;

        // Outer sumcheck: sum_x eq(tau,x) * (Az(x)*Bz(x) - u*Cz(x) - E(x)) = 0
        let tau = (0..num_rounds_x)
            .map(|_| transcript.squeeze(b"t"))
            .collect::<Result<Vec<_>, VegaError>>()?;

        // Third polynomial for cubic sumcheck: u*Cz + E
        let uCz_plus_E: Vec<E::Scalar> = Cz
            .iter()
            .zip(W.E.iter())
            .map(|(cz_i, e_i)| *u * cz_i + e_i)
            .collect();

        let mut poly_Az = MultilinearPolynomial::new(Az);
        let mut poly_Bz = MultilinearPolynomial::new(Bz);
        let mut poly_uCzE = MultilinearPolynomial::new(uCz_plus_E);

        let (sc_proof_outer, r_x, claims_outer) = SumcheckProof::prove_cubic_with_three_inputs(
            &E::Scalar::ZERO,
            tau,
            &mut poly_Az,
            &mut poly_Bz,
            &mut poly_uCzE,
            transcript,
        )?;

        let (claim_Az, claim_Bz, claim_uCzE) = (claims_outer[0], claims_outer[1], claims_outer[2]);

        transcript.absorb(
            b"claims_outer",
            &[claim_Az, claim_Bz, claim_uCzE].as_slice(),
        );

        // Inner sumcheck setup
        let r = transcript.squeeze(b"r")?;
        let r_sq = r * r;

        let evals_rx = EqPolynomial::evals_from_points(&r_x);

        // claim_E = MLE(E)(r_x) = <E, eq(r_x)>
        let claim_E: E::Scalar =
            W.E.iter()
                .zip(evals_rx.iter())
                .map(|(e_i, eq_i)| *e_i * eq_i)
                .fold(E::Scalar::ZERO, |acc, v| acc + v);

        let claim_inner_joint = claim_Az + r * claim_Bz + r_sq * (claim_uCzE - claim_E);

        // Bind row variables for A, B, C -- using original num_cols
        let num_cols = num_vars + 1 + S.num_io;
        let evals_A = bind_matrix_row_vars::<E>(&S.A, &evals_rx, num_cols);
        let evals_B = bind_matrix_row_vars::<E>(&S.B, &evals_rx, num_cols);
        let evals_C = bind_matrix_row_vars::<E>(&S.C, &evals_rx, num_cols);

        let mut poly_ABC: Vec<E::Scalar> = evals_A
            .iter()
            .zip(evals_B.iter())
            .zip(evals_C.iter())
            .map(|((a, b), c)| *a + r * b + r_sq * *u * c)
            .collect();
        poly_ABC.resize(z_len, E::Scalar::ZERO);

        let mut poly_z = z_unpadded;
        poly_z.resize(z_len, E::Scalar::ZERO);

        let (sc_proof_inner, r_y, _claims_inner) = SumcheckProof::prove_quad(
            &claim_inner_joint,
            num_rounds_y,
            &mut MultilinearPolynomial::new(poly_ABC),
            &mut MultilinearPolynomial::new(poly_z),
            transcript,
        )?;

        // Direct openings using RLC of Hyrax rows
        let (v_W, blind_W) = E::PCS::prove_direct(ck, &W.W, &W.r_W, &r_y[1..])?;
        let (v_E, blind_E) = E::PCS::prove_direct(ck, &W.E, &W.r_E, &r_x)?;

        // Absorb direct opening vectors into transcript
        transcript.absorb(b"v_W", &v_W.as_slice());
        transcript.absorb(b"v_E", &v_E.as_slice());

        Ok(Self {
            sc_proof_outer,
            claims_outer: (claim_Az, claim_Bz, claim_uCzE),
            sc_proof_inner,
            v_W,
            blind_W,
            v_E,
            blind_E,
        })
    }

    /// Verify a relaxed R1CS Spartan proof.
    ///
    /// The verifier provides the full folded instance (with commitments) from NIFS::verify.
    pub(crate) fn verify(
        &self,
        S: &R1CSShape<E>,
        vk_ee: &<E::PCS as PCSEngineTrait<E>>::VerifierKey,
        U: &RelaxedR1CSInstance<E>,
        transcript: &mut E::TE,
    ) -> Result<(), VegaError> {
        // Must match prover: absorb only (u, X) -- not the commitments.
        transcript.absorb(b"u_relaxed", &U.u);
        transcript.absorb(b"X_relaxed", &U.X.as_slice());

        let num_cons = S.num_cons;
        let num_vars = S.num_vars;
        let num_rounds_x = num_cons.log_2();
        let num_vars_padded = num_vars.next_power_of_two();
        let num_rounds_y = num_vars_padded.log_2() + 1;

        // Outer sumcheck
        let tau = (0..num_rounds_x)
            .map(|_| transcript.squeeze(b"t"))
            .collect::<Result<EqPolynomial<_>, VegaError>>()?;

        let (claim_outer_final, r_x) =
            self.sc_proof_outer
                .verify(E::Scalar::ZERO, num_rounds_x, 3, transcript)?;

        let (claim_Az, claim_Bz, claim_uCzE) = self.claims_outer;
        let taus_bound_rx = tau.evaluate(&r_x);
        let claim_outer_expected = taus_bound_rx * (claim_Az * claim_Bz - claim_uCzE);
        if claim_outer_final != claim_outer_expected {
            return Err(VegaError::InvalidSumcheckProof);
        }

        transcript.absorb(
            b"claims_outer",
            &[claim_Az, claim_Bz, claim_uCzE].as_slice(),
        );

        // Inner sumcheck
        let r = transcript.squeeze(b"r")?;
        let r_sq = r * r;

        // Verify direct opening of E at r_x -> gives us eval_E
        let eval_E = E::PCS::verify_direct(vk_ee, &U.comm_E, &self.v_E, &self.blind_E, &r_x)?;

        let claim_inner_joint = claim_Az + r * claim_Bz + r_sq * (claim_uCzE - eval_E);

        let (claim_inner_final, r_y) =
            self.sc_proof_inner
                .verify(claim_inner_joint, num_rounds_y, 2, transcript)?;

        // Verify direct opening of W at r_y[1..] -> gives us eval_W
        let eval_W = E::PCS::verify_direct(vk_ee, &U.comm_W, &self.v_W, &self.blind_W, &r_y[1..])?;

        // Matrix evaluations at (r_x, r_y) -- compute T_y first since we also use it for eval_Z
        let T_x = EqPolynomial::evals_from_points(&r_x);
        let T_y = EqPolynomial::evals_from_points(&r_y);

        // eval_Z = sum z[i] * T_y[i]
        // W occupies positions 0..num_vars (all in first half), u at num_vars, X at num_vars+1..
        // For i < num_vars_padded: T_y[i] = (1-r_y[0]) * eq(i, r_y[1..])
        // So W's contribution = (1-r_y[0]) * eval_W.
        // u and X may be in first or second half -- use T_y[i] directly for correctness.
        let eval_Z = {
            let mut sum = (E::Scalar::ONE - r_y[0]) * eval_W;
            sum += U.u * T_y[num_vars];
            for (j, x_j) in U.X.iter().enumerate() {
                sum += *x_j * T_y[num_vars + 1 + j];
            }
            sum
        };
        let eval_A = evaluate_matrix_with_tables::<E>(&S.A, &T_x, &T_y);
        let eval_B = evaluate_matrix_with_tables::<E>(&S.B, &T_x, &T_y);
        let eval_C = evaluate_matrix_with_tables::<E>(&S.C, &T_x, &T_y);

        let eval_ABC = eval_A + r * eval_B + r_sq * U.u * eval_C;

        let claim_inner_expected = eval_ABC * eval_Z;
        if claim_inner_final != claim_inner_expected {
            return Err(VegaError::InvalidSumcheckProof);
        }

        // Absorb direct opening vectors into transcript (must match prover)
        transcript.absorb(b"v_W", &self.v_W.as_slice());
        transcript.absorb(b"v_E", &self.v_E.as_slice());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ff::Field;
    use tracing_subscriber::EnvFilter;

    use super::*;

    #[test]
    fn test_relaxed_spartan_proof() {
        crate::iroha_rng::with_deterministic_test_seed(0x40, || {
            let _ = tracing_subscriber::fmt()
                .with_target(false)
                .with_env_filter(EnvFilter::from_default_env())
                .try_init();

            type E = crate::provider::PallasHyraxEngine;
            test_relaxed_spartan_with::<E>();
        });
    }

    fn test_relaxed_spartan_with<E: Engine>() {
        let num_cons = 4;
        let num_vars = 4;
        let num_io = 0;

        let A = SparseMatrix {
            indices: vec![0, 1, 2, 3],
            indptr: vec![0, 1, 2, 3, 4],
            data: vec![E::Scalar::ONE; 4],
            cols: num_vars + 1 + num_io,
        };
        let B = A.clone();
        let C = A.clone();

        let shape = R1CSShape::<E>::new(num_cons, num_vars, num_io, A, B, C).unwrap();

        let width = 32;
        let (ck, vk_ee) = E::PCS::setup(b"test", shape.num_vars.max(shape.num_cons), width);

        let (U, W) = shape.sample_random_instance_witness(&ck).unwrap();
        shape.is_sat_relaxed(&ck, &U, &W).unwrap();

        // Prove
        let mut transcript_p = E::TE::new(b"test_relaxed_spartan");
        let proof =
            RelaxedR1CSSpartanProof::prove(&shape, &ck, &U.u, &U.X, &W, &mut transcript_p).unwrap();

        // Verify
        let mut transcript_v = E::TE::new(b"test_relaxed_spartan");
        proof.verify(&shape, &vk_ee, &U, &mut transcript_v).unwrap();
    }
}
