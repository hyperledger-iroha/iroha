// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! Inner Product Argument (IPA) implementation
use crate::{
    errors::VegaError,
    provider::traits::{DlogGroup, DlogGroupExt},
    traits::{
        Engine,
        transcript::{TranscriptEngineTrait, TranscriptReprTrait},
    },
};
use core::fmt::Debug;
use ff::Field;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// computes the inner product of two vectors.
pub(crate) fn inner_product<T: Field + Send + Sync>(a: &[T], b: &[T]) -> T {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .fold(T::ZERO, |acc, (ai, bi)| acc + *ai * *bi)
}

/// An inner product instance consists of a commitment to a vector `a` and another vector `b`
/// and the claim that c = <a, b>.
pub struct InnerProductInstance<E: Engine> {
    comm_a_vec: E::GE,
    b_vec: Vec<E::Scalar>,
    comm_c: E::GE,
}

/// Holds witness for the inner product instance.
pub struct InnerProductWitness<E: Engine> {
    a_vec: Vec<E::Scalar>,
    r_a: E::Scalar, // blind for the commitment to a_vec
    r_c: E::Scalar, // blind for the commitment to c
}

impl<E: Engine> InnerProductInstance<E>
where
    E::GE: DlogGroup,
{
    /// Creates a new inner product instance
    pub fn new(comm_a_vec: &E::GE, b_vec: &[E::Scalar], comm_c: &E::GE) -> Self {
        InnerProductInstance {
            comm_a_vec: *comm_a_vec,
            b_vec: b_vec.to_vec(),
            comm_c: *comm_c,
        }
    }
}

impl<E: Engine> TranscriptReprTrait<E::GE> for InnerProductInstance<E>
where
    E::GE: DlogGroup,
{
    fn to_transcript_bytes(&self) -> Vec<u8> {
        // we do not need to include self.b_vec as in our context it is produced from the transcript
        [
            self.comm_a_vec.to_transcript_bytes(),
            self.comm_c.to_transcript_bytes(),
        ]
        .concat()
    }
}

impl<E: Engine> InnerProductWitness<E> {
    /// Creates a new inner product witness
    pub fn new(a_vec: &[E::Scalar], r_a: &E::Scalar, r_c: &E::Scalar) -> Self {
        InnerProductWitness {
            a_vec: a_vec.to_vec(),
            r_a: *r_a,
            r_c: *r_c,
        }
    }
}

// Instance: C_a, C_c, b_vec
// Witness: a_vec, r_a, c, r_c
// Sat if: C_x = Com(x, r_x), C_c = Com(c, r_c), and y = <a_vec, b_vec>
//
// P: samples d_vec, r_\beta, r_\delta, and sends:
// \delta \gets Com(d_vec, r_delta)
// \beta \gets Com(<b_vec, d_vec>, r_beta)
//
// V: sends a challenge r
//
// P: sends
// z_vec \gets r * a_vec + d_vec
// z_\delta \gets r * r_a + r_\delta
// z_\beta \gets r * r_c + r_\beta
//
// V: checks
// r * Comm_a + delta =? Com(z_vec, z_\delta)
// r * Comm_c + beta =? Com(<z_vec, b_vec>, z_\beta)
//
/// An inner product argument using a linear-sized argument
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct InnerProductArgumentLinear<E: Engine>
where
    E::GE: DlogGroupExt,
{
    delta: E::GE,
    beta: E::GE,
    z_vec: Vec<E::Scalar>,
    z_delta: E::Scalar,
    z_beta: E::Scalar,
}

impl<E: Engine> InnerProductArgumentLinear<E>
where
    E::GE: DlogGroupExt,
{
    fn protocol_name() -> &'static [u8] {
        b"inner product argument (linear)"
    }

    /// Proves the inner product argument
    pub fn prove(
        ck: &[<E::GE as DlogGroup>::AffineGroupElement],
        h: &E::GE,
        ck_c: &<E::GE as DlogGroup>::AffineGroupElement,
        h_c: &E::GE,
        U: &InnerProductInstance<E>,
        W: &InnerProductWitness<E>,
        transcript: &mut E::TE,
    ) -> Result<Self, VegaError> {
        transcript.dom_sep(Self::protocol_name());

        // absorb the instance in the transcript
        transcript.absorb(b"U", U);

        // produce randomness for the proofs using fast CSPRNG
        let mut rng = crate::iroha_rng::ScopedRng;
        let d_vec = (0..U.b_vec.len())
            .map(|_| E::Scalar::random(&mut rng))
            .collect::<Vec<E::Scalar>>();
        let r_delta = E::Scalar::random(&mut rng);
        let r_beta = E::Scalar::random(&mut rng);

        let delta =
            E::GE::vartime_multiscalar_mul(&d_vec, &ck[0..d_vec.len()], true)? + *h * r_delta;
        let beta = E::GE::group(ck_c) * inner_product(&U.b_vec, &d_vec) + *h_c * r_beta;

        transcript.absorb(b"delta", &delta);
        transcript.absorb(b"beta", &beta);

        let r = transcript.squeeze(b"r")?;

        let z_vec = (0..d_vec.len())
            .into_par_iter()
            .map(|i| r * W.a_vec[i] + d_vec[i])
            .collect::<Vec<E::Scalar>>();

        let z_delta = r * W.r_a + r_delta;
        let z_beta = r * W.r_c + r_beta;

        Ok(Self {
            delta,
            z_vec,
            z_delta,
            beta,
            z_beta,
        })
    }

    /// Verifies the inner product argument
    pub fn verify(
        &self,
        ck: &[<E::GE as DlogGroup>::AffineGroupElement],
        h: &E::GE,
        ck_c: &<E::GE as DlogGroup>::AffineGroupElement,
        h_c: &E::GE,
        n: usize,
        U: &InnerProductInstance<E>,
        transcript: &mut E::TE,
    ) -> Result<(), VegaError> {
        transcript.dom_sep(Self::protocol_name());

        // absorb the instance in the transcript
        transcript.absorb(b"U", U);

        transcript.absorb(b"delta", &self.delta);
        transcript.absorb(b"beta", &self.beta);

        let r = transcript.squeeze(b"r")?;

        if self.z_vec.len() != n || ck.len() < self.z_vec.len() {
            return Err(VegaError::InvalidInputLength {
                reason: format!(
                    "Inner product argument verify: Expected {} elements in z_vec, got {}",
                    n,
                    self.z_vec.len()
                ),
            });
        }

        if U.comm_a_vec * r + self.delta
            != E::GE::vartime_multiscalar_mul(&self.z_vec, &ck[0..self.z_vec.len()], true)?
                + *h * self.z_delta
        {
            return Err(VegaError::InvalidPCS {
                reason: "Inner product argument verify: First equation failed".to_string(),
            });
        }

        if U.comm_c * r + self.beta
            != E::GE::group(ck_c) * inner_product(&self.z_vec, &U.b_vec) + *h_c * self.z_beta
        {
            return Err(VegaError::InvalidPCS {
                reason: "Inner product argument verify: Second equation failed".to_string(),
            });
        }

        Ok(())
    }
}
