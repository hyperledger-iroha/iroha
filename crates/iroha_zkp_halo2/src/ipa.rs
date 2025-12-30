//! Inner-Product Argument (IPA) for polynomial commitments.
//!
//! This module implements a standard IPA proof for inner products over a prime
//! field with multiplicative group commitments. It follows the Bootle et al.
//! style L/R reduction with transcript-derived challenges.

use core::marker::PhantomData;

use crate::{
    IpaGroup, IpaScalar,
    backend::{IpaBackend, product},
    errors::Error,
    params::Params,
    transcript::Transcript,
};

/// Computes the inner product <a, b> in the prime field.
fn inner_product<B: IpaBackend>(a: &[B::Scalar], b: &[B::Scalar]) -> B::Scalar {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = B::Scalar::default();
    for (ai, bi) in a.iter().zip(b.iter()) {
        acc = acc.add(ai.mul(*bi));
    }
    acc
}

/// Commits to a vector `v` using the provided generator vector `g`.
pub(crate) fn commit_vec<B: IpaBackend>(
    g: &[B::Group],
    v: &[B::Scalar],
) -> Result<B::Group, Error> {
    if g.len() != v.len() {
        return Err(Error::DimensionMismatch {
            expected: g.len(),
            actual: v.len(),
        });
    }
    let prods = g.iter().zip(v.iter()).map(|(gi, vi)| gi.pow(*vi));
    Ok(product(prods))
}

/// An IPA proof for <a, b> binding into L/R rounds and final scalars.
#[derive(Clone, Debug)]
pub struct IpaProof<B: IpaBackend> {
    /// L commitments, one per round.
    pub l_vec: Vec<B::Group>,
    /// R commitments, one per round.
    pub r_vec: Vec<B::Group>,
    /// Final scalar after reductions for `a`.
    pub a_final: B::Scalar,
    /// Final scalar after reductions for `b`.
    pub b_final: B::Scalar,
}

impl<B: IpaBackend> IpaProof<B> {
    /// Number of rounds in the proof.
    pub fn rounds(&self) -> usize {
        self.l_vec.len()
    }
}

/// Prover for the IPA opening.
pub struct IpaProver<B: IpaBackend>(PhantomData<B>);

impl<B: IpaBackend> IpaProver<B> {
    /// Creates an IPA proof that a committed vector `a` has inner product `t`
    /// with the public vector `b` under parameters `params` and transcript.
    pub fn prove(
        params: &Params<B>,
        transcript: &mut Transcript,
        a: &[B::Scalar],
        b: &[B::Scalar],
        p_g: B::Group,
        t: B::Scalar,
    ) -> Result<IpaProof<B>, Error> {
        let n = params.n();
        if a.len() != n || b.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: a.len().max(b.len()),
            });
        }
        // Bind public inputs
        transcript.absorb("ipa.n", &(n as u64).to_le_bytes());
        // Construct Q = g^a · h^b · u^{<a,b>} where g^a is provided as `p_g`.
        let hb = commit_vec::<B>(params.h(), b)?;
        let ut = params.u().pow(t);
        let mut q = p_g.mul(hb).mul(ut);

        let mut a_vec = a.to_vec();
        let mut b_vec = b.to_vec();
        let mut g_vec = params.g().to_vec();
        let mut h_vec = params.h().to_vec();

        let mut l_vec = Vec::new();
        let mut r_vec = Vec::new();

        while a_vec.len() > 1 {
            let m = a_vec.len();
            debug_assert_eq!(m & (m - 1), 0, "vector length must stay power-of-two");
            let half = m / 2;

            let (a_l, a_r) = a_vec.split_at(half);
            let (b_l, b_r) = b_vec.split_at(half);
            let (g_l, g_r) = g_vec.split_at(half);
            let (h_l, h_r) = h_vec.split_at(half);

            let c_l = inner_product::<B>(a_l, b_r);
            let c_r = inner_product::<B>(a_r, b_l);

            // L = g_R^{a_L} · h_L^{b_R} · u^{c_l}
            let l = commit_vec::<B>(g_r, a_l)?
                .mul(commit_vec::<B>(h_l, b_r)?)
                .mul(params.u().pow(c_l));
            // R = g_L^{a_R} · h_R^{b_L} · u^{c_r}
            let r = commit_vec::<B>(g_l, a_r)?
                .mul(commit_vec::<B>(h_r, b_l)?)
                .mul(params.u().pow(c_r));

            // Absorb and derive challenge
            let mut lr_bytes = Vec::with_capacity(64);
            lr_bytes.extend_from_slice(&l.to_bytes());
            lr_bytes.extend_from_slice(&r.to_bytes());
            transcript.absorb("ipa.round", &lr_bytes);
            let x = transcript.challenge_scalar::<B::Scalar>("ipa.x");
            let x_inv = x.inv()?;

            // Fold vectors: a' = a_L*x + a_R*x^{-1}
            //               b' = b_L*x^{-1} + b_R*x
            let mut a_new = Vec::with_capacity(half);
            let mut b_new = Vec::with_capacity(half);
            for i in 0..half {
                a_new.push(a_l[i].mul(x).add(a_r[i].mul(x_inv)));
                b_new.push(b_l[i].mul(x_inv).add(b_r[i].mul(x)));
            }

            // Fold generators: g' = g_L^{x^{-1}} || g_R^{x}
            //                  h' = h_L^{x}     || h_R^{x^{-1}}
            let mut g_tmp = Vec::with_capacity(half * 2);
            let mut h_tmp = Vec::with_capacity(half * 2);
            for i in 0..half {
                g_tmp.push(g_l[i].pow(x_inv));
                g_tmp.push(g_r[i].pow(x));
            }
            let g_new = g_tmp
                .chunks_exact(2)
                .map(|pair| pair[0].mul(pair[1]))
                .collect::<Vec<_>>();

            for i in 0..half {
                h_tmp.push(h_l[i].pow(x));
                h_tmp.push(h_r[i].pow(x_inv));
            }
            let h_new = h_tmp
                .chunks_exact(2)
                .map(|pair| pair[0].mul(pair[1]))
                .collect::<Vec<_>>();

            // Update Q: Q' = L^{x^2} · Q · R^{x^{-2}}
            let x2 = x.mul(x);
            let x2_inv = x_inv.mul(x_inv);
            q = l.pow(x2).mul(q).mul(r.pow(x2_inv));

            l_vec.push(l);
            r_vec.push(r);

            a_vec = a_new;
            b_vec = b_new;
            g_vec = g_new;
            h_vec = h_new;
        }

        debug_assert_eq!(a_vec.len(), 1);
        debug_assert_eq!(b_vec.len(), 1);
        let a_final = a_vec[0];
        let b_final = b_vec[0];

        Ok(IpaProof {
            l_vec,
            r_vec,
            a_final,
            b_final,
        })
    }
}

/// Verifier for the IPA opening.
pub struct IpaVerifier<B: IpaBackend>(PhantomData<B>);

impl<B: IpaBackend> IpaVerifier<B> {
    /// Verifies an IPA proof that commitment `p_g` to `a` satisfies
    /// <a, b> == `t` for public vector `b`.
    pub fn verify(
        params: &Params<B>,
        transcript: &mut Transcript,
        b: &[B::Scalar],
        p_g: B::Group,
        t: B::Scalar,
        proof: &IpaProof<B>,
    ) -> Result<(), Error> {
        let n = params.n();
        if b.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: b.len(),
            });
        }

        transcript.absorb("ipa.n", &(n as u64).to_le_bytes());

        let hb = commit_vec::<B>(params.h(), b)?;
        let ut = params.u().pow(t);
        let mut q = p_g.mul(hb).mul(ut);

        let mut g_vec = params.g().to_vec();
        let mut h_vec = params.h().to_vec();

        for (&l, &r) in proof.l_vec.iter().zip(proof.r_vec.iter()) {
            let mut lr_bytes = Vec::with_capacity(64);
            lr_bytes.extend_from_slice(&l.to_bytes());
            lr_bytes.extend_from_slice(&r.to_bytes());
            transcript.absorb("ipa.round", &lr_bytes);
            let x = transcript.challenge_scalar::<B::Scalar>("ipa.x");
            let x_inv = x.inv()?;

            let x2 = x.mul(x);
            let x2_inv = x_inv.mul(x_inv);
            q = l.pow(x2).mul(q).mul(r.pow(x2_inv));

            let m = g_vec.len();
            let half = m / 2;
            let (g_l, g_r) = g_vec.split_at(half);
            let (h_l, h_r) = h_vec.split_at(half);

            let mut g_new = Vec::with_capacity(half);
            for i in 0..half {
                let gi = g_l[i].pow(x_inv).mul(g_r[i].pow(x));
                g_new.push(gi);
            }
            let mut h_new = Vec::with_capacity(half);
            for i in 0..half {
                let hi = h_l[i].pow(x).mul(h_r[i].pow(x_inv));
                h_new.push(hi);
            }
            g_vec = g_new;
            h_vec = h_new;
        }

        debug_assert_eq!(g_vec.len(), 1);
        debug_assert_eq!(h_vec.len(), 1);
        let g_final = g_vec[0];
        let h_final = h_vec[0];

        let a = proof.a_final;
        let b_fin = proof.b_final;
        let term = g_final
            .pow(a)
            .mul(h_final.pow(b_fin))
            .mul(params.u().pow(a.mul(b_fin)));

        if q == term {
            Ok(())
        } else {
            Err(Error::VerificationFailed)
        }
    }
}
