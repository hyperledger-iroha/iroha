//! Polynomial representation and commit/open operations via IPA.

use crate::{
    IpaScalar,
    backend::IpaBackend,
    errors::Error,
    ipa::{IpaProof, IpaProver, IpaVerifier, commit_vec},
    params::Params,
    transcript::Transcript,
};

/// Dense polynomial over backend scalar field represented by its coefficients in
/// ascending order, i.e., `coeffs[i]` is the coefficient of `x^i`.
#[derive(Clone, Debug)]
pub struct Polynomial<B: IpaBackend> {
    coeffs: Vec<B::Scalar>,
}

impl<B: IpaBackend> Polynomial<B> {
    /// Creates a polynomial from coefficients in ascending order.
    pub fn from_coeffs(coeffs: Vec<B::Scalar>) -> Self {
        Self { coeffs }
    }

    /// Returns the number of coefficients.
    pub fn len(&self) -> usize {
        self.coeffs.len()
    }

    /// Returns true if the polynomial has no coefficients.
    pub fn is_empty(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// Evaluates the polynomial at a point `x`.
    pub fn evaluate(&self, x: B::Scalar) -> B::Scalar {
        // Horner's rule
        let mut acc = B::Scalar::zero();
        for &c in self.coeffs.iter().rev() {
            acc = acc.mul(x).add(c);
        }
        acc
    }

    /// Commits to the coefficient vector using the `g` generators from `params`.
    pub fn commit(&self, params: &Params<B>) -> Result<B::Group, Error> {
        commit_vec::<B>(params.g(), &self.coeffs)
    }

    /// Creates an opening proof at point `z` that the committed polynomial
    /// evaluates to `t` at `z`.
    pub fn open(
        &self,
        params: &Params<B>,
        transcript: &mut Transcript,
        z: B::Scalar,
        p_g: B::Group,
    ) -> Result<(IpaProof<B>, B::Scalar), Error> {
        let n = params.n();
        if self.len() != n {
            return Err(Error::DimensionMismatch {
                expected: n,
                actual: self.len(),
            });
        }
        // Build public vector b = [1, z, z^2, ..., z^{n-1}]
        let mut b = Vec::with_capacity(n);
        let mut pow = B::Scalar::one();
        for _ in 0..n {
            b.push(pow);
            pow = pow.mul(z);
        }
        let t = self.evaluate(z);
        let proof = IpaProver::<B>::prove(params, transcript, &self.coeffs, &b, p_g, t)?;
        Ok((proof, t))
    }

    /// Verifies an opening proof for the committed polynomial at point `z`
    /// with claimed evaluation `t`.
    pub fn verify_open(
        params: &Params<B>,
        transcript: &mut Transcript,
        z: B::Scalar,
        p_g: B::Group,
        t: B::Scalar,
        proof: &IpaProof<B>,
    ) -> Result<(), Error> {
        let n = params.n();
        let mut b = Vec::with_capacity(n);
        let mut pow = B::Scalar::one();
        for _ in 0..n {
            b.push(pow);
            pow = pow.mul(z);
        }
        IpaVerifier::<B>::verify(params, transcript, &b, p_g, t, proof)
    }
}
