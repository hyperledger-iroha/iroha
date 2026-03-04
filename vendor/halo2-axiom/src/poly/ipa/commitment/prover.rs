use ff::Field;
use rand_core::RngCore;

use super::ParamsIPA;
use crate::arithmetic::{
    best_multiexp_with_extra, compute_inner_product, eval_polynomial, parallelize, CurveAffine,
};

use crate::poly::commitment::ParamsProver;
use crate::poly::{commitment::Blind, Coeff, Polynomial};
use crate::transcript::{EncodedChallenge, TranscriptWrite};

use group::Curve;
use std::io::{self, ErrorKind};

/// Create a polynomial commitment opening proof for the polynomial defined
/// by the coefficients `px`, the blinding factor `blind` used for the
/// polynomial commitment, and the point `x` that the polynomial is
/// evaluated at.
///
/// This function will panic if the provided polynomial is too large with
/// respect to the polynomial commitment parameters.
///
/// **Important:** This function assumes that the provided `transcript` has
/// already seen the common inputs: the polynomial commitment P, the claimed
/// opening v, and the point x. It's probably also nice for the transcript
/// to have seen the elliptic curve description and the URS, if you want to
/// be rigorous.
pub fn create_proof<
    C: CurveAffine,
    E: EncodedChallenge<C>,
    R: RngCore,
    T: TranscriptWrite<C, E>,
>(
    params: &ParamsIPA<C>,
    mut rng: R,
    transcript: &mut T,
    p_poly: &Polynomial<C::Scalar, Coeff>,
    p_blind: Blind<C::Scalar>,
    x_3: C::Scalar,
) -> io::Result<()> {
    // We're limited to polynomials of degree n - 1.
    assert_eq!(p_poly.len(), params.n as usize);

    // Sample a random polynomial (of same degree) that has a root at x_3, first
    // by setting all coefficients to random values.
    let mut s_poly = (*p_poly).clone();
    for coeff in s_poly.iter_mut() {
        *coeff = C::Scalar::random(&mut rng);
    }
    // Evaluate the random polynomial at x_3
    let s_at_x3 = eval_polynomial(&s_poly[..], x_3);
    // Subtract constant coefficient to get a random polynomial with a root at x_3
    s_poly[0] -= &s_at_x3;
    // And sample a random blind
    let s_poly_blind = Blind(C::Scalar::random(&mut rng));

    // Write a commitment to the random polynomial to the transcript
    let s_poly_commitment = params.commit(&s_poly, s_poly_blind).to_affine();
    transcript.write_point(s_poly_commitment)?;

    // Challenge that will ensure that the prover cannot change P but can only
    // witness a random polynomial commitment that agrees with P at x_3, with high
    // probability.
    let xi = *transcript.squeeze_challenge_scalar::<()>();

    // Challenge that ensures that the prover did not interfere with the U term
    // in their commitments.
    let z = *transcript.squeeze_challenge_scalar::<()>();

    // We'll be opening `P' = P - [v] G_0 + [ξ] S` to ensure it has a root at
    // zero.
    let mut p_prime_poly = s_poly * xi + p_poly;
    let v = eval_polynomial(&p_prime_poly, x_3);
    p_prime_poly[0] -= &v;
    let p_prime_blind = s_poly_blind * Blind(xi) + p_blind;

    // This accumulates the synthetic blinding factor `f` starting
    // with the blinding factor for `P'`.
    let mut f = p_prime_blind.0;

    // Initialize the vector `p_prime` as the coefficients of the polynomial.
    let mut p_prime = p_prime_poly.values;
    assert_eq!(p_prime.len(), params.n as usize);

    // Initialize the vector `b` as the powers of `x_3`. The inner product of
    // `p_prime` and `b` is the evaluation of the polynomial at `x_3`.
    let mut b = Vec::with_capacity(1 << params.k);
    {
        let mut cur = C::Scalar::ONE;
        for _ in 0..(1 << params.k) {
            b.push(cur);
            cur *= &x_3;
        }
    }

    // Initialize the vector `G'` from the URS. We'll be progressively collapsing
    // this vector into smaller and smaller vectors until it is of length 1.
    let mut g_prime = params.g.clone();

    // Perform the inner product argument, round by round.
    for j in 0..params.k {
        let half = 1 << (params.k - j - 1); // half the length of `p_prime`, `b`, `G'`

        // Compute L and R by bundling the evaluation and blinding scalars into a single MSM call,
        // which avoids the redundant preprocessing work from the previous two-step approach.
        let value_l_j = compute_inner_product(&p_prime[half..], &b[0..half]);
        let value_r_j = compute_inner_product(&p_prime[0..half], &b[half..]);
        let l_j_randomness = C::Scalar::random(&mut rng);
        let r_j_randomness = C::Scalar::random(&mut rng);
        let l_j = best_multiexp_with_extra(
            &p_prime[half..],
            &g_prime[0..half],
            &[
                (value_l_j * &z, params.u),
                (l_j_randomness, params.w),
            ],
        )
        .to_affine();
        let r_j = best_multiexp_with_extra(
            &p_prime[0..half],
            &g_prime[half..],
            &[
                (value_r_j * &z, params.u),
                (r_j_randomness, params.w),
            ],
        )
        .to_affine();

        // Feed L and R into the real transcript
        transcript.write_point(l_j)?;
        transcript.write_point(r_j)?;

        let u_j = *transcript.squeeze_challenge_scalar::<()>();
        let u_j_inv = Option::<C::Scalar>::from(u_j.invert()).ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                "Fiat-Shamir challenge produced a non-invertible scalar",
            )
        })?;

        // Collapse `p_prime` and `b` using the parallel helper.
        collapse_round_vectors(&mut p_prime, &mut b, half, u_j, u_j_inv);
        p_prime.truncate(half);
        b.truncate(half);

        // Collapse `G'`
        parallel_generator_collapse(&mut g_prime, u_j);
        g_prime.truncate(half);

        // Update randomness (the synthetic blinding factor at the end)
        f += &(l_j_randomness * &u_j_inv);
        f += &(r_j_randomness * &u_j);
    }

    // We have fully collapsed `p_prime`, `b`, `G'`
    assert_eq!(p_prime.len(), 1);
    let c = p_prime[0];

    transcript.write_scalar(c)?;
    transcript.write_scalar(f)?;

    Ok(())
}

fn collapse_round_vectors<F: Field>(
    p_prime: &mut Vec<F>,
    b: &mut Vec<F>,
    half: usize,
    u_j: F,
    u_j_inv: F,
) {
    if half == 0 {
        return;
    }

    let (p_lo, p_hi_mut) = p_prime.split_at_mut(half);
    let p_hi: &[F] = &*p_hi_mut;
    let u_inv = u_j_inv;
    parallelize(p_lo, |chunk, start| {
        let hi_chunk = &p_hi[start..start + chunk.len()];
        for (offset, value) in chunk.iter_mut().enumerate() {
            *value += hi_chunk[offset] * u_inv;
        }
    });

    let (b_lo, b_hi_mut) = b.split_at_mut(half);
    let b_hi: &[F] = &*b_hi_mut;
    let u = u_j;
    parallelize(b_lo, |chunk, start| {
        let hi_chunk = &b_hi[start..start + chunk.len()];
        for (offset, value) in chunk.iter_mut().enumerate() {
            *value += hi_chunk[offset] * u;
        }
    });
}

fn parallel_generator_collapse<C: CurveAffine>(g: &mut [C], challenge: C::Scalar) {
    let len = g.len() / 2;
    let (g_lo, g_hi) = g.split_at_mut(len);

    parallelize(g_lo, |g_lo, start| {
        let g_hi = &g_hi[start..];
        let mut tmp = Vec::with_capacity(g_lo.len());
        for (g_lo, g_hi) in g_lo.iter().zip(g_hi.iter()) {
            tmp.push(g_lo.to_curve() + &(*g_hi * challenge));
        }
        C::Curve::batch_normalize(&tmp, g_lo);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arithmetic::CurveAffine;
    use crate::poly::commitment::Blind;
    use crate::poly::ipa::commitment::ParamsIPA;
    use crate::poly::{Coeff, Polynomial};
    use crate::transcript::{EncodedChallenge, Transcript, TranscriptWrite};
    use ff::Field;
    use halo2curves::pasta::EqAffine;
    use rand_core::OsRng;
    use std::io::{self, ErrorKind};
    use std::marker::PhantomData;

    #[derive(Clone, Copy)]
    struct DummyChallenge<C: CurveAffine>(C::Scalar);

    impl<C: CurveAffine> EncodedChallenge<C> for DummyChallenge<C> {
        type Input = C::Scalar;

        fn new(input: &Self::Input) -> Self {
            Self(*input)
        }

        fn get_scalar(&self) -> C::Scalar {
            self.0
        }
    }

    struct DummyTranscript<C: CurveAffine> {
        challenges: Vec<C::Scalar>,
        index: usize,
        _marker: PhantomData<C>,
    }

    impl<C: CurveAffine> DummyTranscript<C> {
        fn new(challenges: Vec<C::Scalar>) -> Self {
            Self {
                challenges,
                index: 0,
                _marker: PhantomData,
            }
        }
    }

    impl<C: CurveAffine> Transcript<C, DummyChallenge<C>> for DummyTranscript<C> {
        fn squeeze_challenge(&mut self) -> DummyChallenge<C> {
            let scalar = if self.index < self.challenges.len() {
                let value = self.challenges[self.index];
                self.index += 1;
                value
            } else {
                C::Scalar::ONE
            };
            DummyChallenge(scalar)
        }

        fn common_point(&mut self, _point: C) -> io::Result<()> {
            Ok(())
        }

        fn common_scalar(&mut self, _scalar: C::Scalar) -> io::Result<()> {
            Ok(())
        }
    }

    impl<C: CurveAffine> TranscriptWrite<C, DummyChallenge<C>> for DummyTranscript<C> {
        fn write_point(&mut self, _point: C) -> io::Result<()> {
            Ok(())
        }

        fn write_scalar(&mut self, _scalar: C::Scalar) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn collapse_round_vectors_matches_sequential() {
        type Scalar = <EqAffine as CurveAffine>::ScalarExt;
        let mut rng = OsRng;
        let size = 64;

        let mut p_prime_parallel: Vec<Scalar> =
            (0..size).map(|_| Scalar::random(&mut rng)).collect();
        let mut b_parallel: Vec<Scalar> = (0..size).map(|_| Scalar::random(&mut rng)).collect();

        let mut p_prime_serial = p_prime_parallel.clone();
        let mut b_serial = b_parallel.clone();

        let (u_j, u_j_inv) = loop {
            let candidate = Scalar::random(&mut rng);
            if let Some(inv) = Option::<Scalar>::from(candidate.invert()) {
                break (candidate, inv);
            }
        };

        let half = size / 2;
        super::collapse_round_vectors(&mut p_prime_parallel, &mut b_parallel, half, u_j, u_j_inv);
        p_prime_parallel.truncate(half);
        b_parallel.truncate(half);

        for i in 0..half {
            p_prime_serial[i] = p_prime_serial[i] + (p_prime_serial[i + half] * u_j_inv);
            b_serial[i] = b_serial[i] + (b_serial[i + half] * u_j);
        }
        p_prime_serial.truncate(half);
        b_serial.truncate(half);

        assert_eq!(p_prime_parallel, p_prime_serial);
        assert_eq!(b_parallel, b_serial);
    }

    #[test]
    fn create_proof_fails_on_non_invertible_challenge() {
        const K: u32 = 2;
        type Scalar = <EqAffine as CurveAffine>::ScalarExt;

        let params = ParamsIPA::<EqAffine>::new(K);
        let poly = Polynomial::<Scalar, Coeff> {
            values: vec![Scalar::ZERO; params.n as usize],
            _marker: PhantomData,
        };
        let blind = Blind(Scalar::ZERO);
        let mut transcript = DummyTranscript::<EqAffine>::new(vec![
            Scalar::ONE,  // xi
            Scalar::ONE,  // z
            Scalar::ZERO, // u_0 triggers error
        ]);

        let result = create_proof::<EqAffine, DummyChallenge<EqAffine>, _, DummyTranscript<EqAffine>>(
            &params,
            OsRng,
            &mut transcript,
            &poly,
            blind,
            Scalar::ONE,
        );

        assert!(
            matches!(result, Err(ref err) if err.kind() == ErrorKind::InvalidData),
            "expected InvalidData error, got {result:?}"
        );
    }
}
