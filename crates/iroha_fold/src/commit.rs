//! Per-column Pedersen commitments of folding witnesses.
//!
//! Every witness column (length `n`) is committed under the same basis with its own
//! blinding scalar: `C = sum_i w_i B_i + rho W`. Commitments fold linearly, so a
//! folded commitment is `C_acc + gamma (C_new - C_acc)` and its opening is the same
//! combination of the columns and blinds. The basis is fixed per relation: the
//! Lagrange basis of the IPA parameters (for a relaxed-PLONK decider) or their
//! monomial generators (for a multilinear decider).

use ff::Field;
use iroha_pasta::{
    PastaCurve,
    msm::{MemoryBudget, msm_secret},
    params::ParamsIpa,
};

use crate::Error;

/// A commitment basis and its blinding base.
#[derive(Clone, Debug)]
pub struct CommitKey<C: PastaCurve> {
    bases: Vec<C::AffineExt>,
    blind: C::AffineExt,
}

/// A witness: every committed column and its blinding scalar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness<F> {
    /// Columns in relation order: advice, then per `LogUp` group `m`, `h_s`, `g`.
    pub columns: Vec<Vec<F>>,
    /// One blinding scalar per column.
    pub blinds: Vec<F>,
}

impl<C: PastaCurve> CommitKey<C> {
    /// A key over `bases` with blinding base `blind`.
    #[must_use]
    pub fn new(bases: Vec<C::AffineExt>, blind: C::AffineExt) -> Self {
        Self { bases, blind }
    }

    /// The Lagrange basis of `params` with its blinding base `w`.
    #[must_use]
    pub fn lagrange(params: &ParamsIpa<C>) -> Self {
        Self::new(params.g_lagrange().to_vec(), params.w())
    }

    /// The monomial generators of `params` with its blinding base `w`.
    #[must_use]
    pub fn monomial(params: &ParamsIpa<C>) -> Self {
        Self::new(params.g().to_vec(), params.w())
    }

    /// Basis length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bases.len()
    }

    /// Whether the basis is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }

    /// Commits one column with blinding scalar `blind`.
    ///
    /// # Errors
    /// [`Error::Shape`] for a column longer than the basis; [`Error::Msm`] when the
    /// MSM fails.
    pub fn commit(
        &self,
        column: &[C::ScalarExt],
        blind: &C::ScalarExt,
    ) -> Result<C::AffineExt, Error> {
        if column.len() > self.bases.len() {
            return Err(Error::Shape("column longer than the commitment basis"));
        }
        let sum = if column.iter().all(|v| bool::from(v.is_zero())) {
            C::identity()
        } else {
            msm_secret::<C>(column, &self.bases[..column.len()], MemoryBudget::DEFAULT)
                .map_err(|_| Error::Msm)?
        };
        Ok((sum + C::from(self.blind) * blind).to_affine())
    }
}

/// `acc + gamma (new - acc)` for points.
pub(crate) fn fold_point<C: PastaCurve>(
    acc: &C::AffineExt,
    new: &C::AffineExt,
    gamma: &C::ScalarExt,
) -> C::AffineExt {
    let acc = C::from(*acc);
    (acc + (C::from(*new) - acc) * gamma).to_affine()
}

/// `acc + gamma (new - acc)` for scalars.
pub(crate) fn fold_scalar<F: Field>(acc: &F, new: &F, gamma: &F) -> F {
    *acc + *gamma * (*new - *acc)
}
