//! Vector commitments and verifier-side linear combinations of points.
//!
//! A commitment to `a` of length `m <= n` is `sum_{i<m} a_i B_i + blind * W`,
//! where `B` is `g` (coefficient form, [`commit`]) or `g_lagrange`
//! (evaluation form, [`commit_lagrange`]). The vendored `Blind::default()` is
//! one, so key-generation commitments are `... + W` ([`DEFAULT_BLIND`]); this
//! keeps every commitment of an all-zero column away from the identity.
//!
//! [`Secrecy`] selects the MSM posture: witness-dependent scalars use
//! [`iroha_pasta::msm::msm_secret`] and constant-time scalar multiplication of
//! `W`; public data (fixed columns, instances, verifier combinations) uses the
//! variable-time paths. Both return the same point.

use ff::Field;
use group::prime::PrimeCurveAffine;
use iroha_pasta::{
    PastaCurve, PastaField,
    msm::{FixedBaseTable, MemoryBudget, MsmError, msm_naive, msm_public, msm_secret},
    params::ParamsIpa,
};

/// The key-generation blind (`Blind::default()` in the vendored code).
pub const DEFAULT_BLIND: u64 = 1;

/// Whether the committed scalars are secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Secrecy {
    /// Public scalars (fixed columns, instances, verifier data).
    Public,
    /// Witness-dependent scalars.
    Secret,
}

/// `sum_i scalars[i] * bases[i] + blind * w` with `scalars.len() <= bases.len()`.
fn commit_with_bases<C: PastaCurve>(
    bases: &[C::AffineExt],
    table: Option<&FixedBaseTable<C>>,
    scalars: &[C::ScalarExt],
    blind: &C::ScalarExt,
    w: &C::AffineExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    let bases = bases.get(..scalars.len()).ok_or(MsmError::LengthMismatch(
        iroha_pasta::LengthMismatch {
            left: scalars.len(),
            right: bases.len(),
        },
    ))?;
    let sum = match (table, secrecy) {
        (Some(table), Secrecy::Public) if table.len() == scalars.len() => {
            table.msm_public(scalars, budget)?
        }
        (Some(table), Secrecy::Secret) if table.len() == scalars.len() => {
            table.msm_secret(scalars, budget)?
        }
        (_, Secrecy::Public) => msm_public::<C>(scalars, bases, budget)?,
        (_, Secrecy::Secret) => msm_secret::<C>(scalars, bases, budget)?,
    };
    let blind_term = match secrecy {
        Secrecy::Public => w.to_curve().mul_vartime(blind),
        Secrecy::Secret => w.to_curve() * *blind,
    };
    Ok(sum + blind_term)
}

/// Commits to coefficients: `sum_i coeffs[i] g[i] + blind * W`.
///
/// # Errors
///
/// [`MsmError::LengthMismatch`] when `coeffs` is longer than `g`;
/// [`MsmError::Budget`] when the MSM does not fit `budget`.
pub fn commit<C: PastaCurve>(
    params: &ParamsIpa<C>,
    coeffs: &[C::ScalarExt],
    blind: &C::ScalarExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    commit_with_bases(
        params.g(),
        None,
        coeffs,
        blind,
        &params.w(),
        secrecy,
        budget,
    )
}

/// Commits to evaluations: `sum_i values[i] g_lagrange[i] + blind * W`.
///
/// # Errors
///
/// As [`commit`].
pub fn commit_lagrange<C: PastaCurve>(
    params: &ParamsIpa<C>,
    values: &[C::ScalarExt],
    blind: &C::ScalarExt,
    secrecy: Secrecy,
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    commit_with_bases(
        params.g_lagrange(),
        None,
        values,
        blind,
        &params.w(),
        secrecy,
        budget,
    )
}

/// Commitment-key tables (precomputed window multiples of `g` and
/// `g_lagrange`) that a proving key may own. Results equal [`commit`] and
/// [`commit_lagrange`] bit for bit.
#[derive(Clone, Debug)]
pub struct CommitmentTables<C: PastaCurve> {
    g: Option<FixedBaseTable<C>>,
    g_lagrange: Option<FixedBaseTable<C>>,
}

impl<C: PastaCurve> Default for CommitmentTables<C> {
    fn default() -> Self {
        Self::none()
    }
}

impl<C: PastaCurve> CommitmentTables<C> {
    /// No tables: every commitment uses the variable-base MSM.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            g: None,
            g_lagrange: None,
        }
    }

    /// Builds the tables that fit `budget` (`g_lagrange` first, then `g`,
    /// each charged the whole budget separately); a table that does not fit
    /// is simply absent.
    #[must_use]
    pub fn build(params: &ParamsIpa<C>, budget: MemoryBudget) -> Self {
        Self {
            g_lagrange: FixedBaseTable::new(params.g_lagrange(), budget).ok(),
            g: FixedBaseTable::new(params.g(), budget).ok(),
        }
    }

    /// Whether the `g` and `g_lagrange` tables exist.
    #[must_use]
    pub fn present(&self) -> (bool, bool) {
        (self.g.is_some(), self.g_lagrange.is_some())
    }

    /// [`commit`] through the `g` table when it covers `coeffs`.
    ///
    /// # Errors
    ///
    /// As [`commit`].
    pub fn commit(
        &self,
        params: &ParamsIpa<C>,
        coeffs: &[C::ScalarExt],
        blind: &C::ScalarExt,
        secrecy: Secrecy,
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        commit_with_bases(
            params.g(),
            self.g.as_ref(),
            coeffs,
            blind,
            &params.w(),
            secrecy,
            budget,
        )
    }

    /// [`commit_lagrange`] through the `g_lagrange` table when it covers
    /// `values`.
    ///
    /// # Errors
    ///
    /// As [`commit`].
    pub fn commit_lagrange(
        &self,
        params: &ParamsIpa<C>,
        values: &[C::ScalarExt],
        blind: &C::ScalarExt,
        secrecy: Secrecy,
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        commit_with_bases(
            params.g_lagrange(),
            self.g_lagrange.as_ref(),
            values,
            blind,
            &params.w(),
            secrecy,
            budget,
        )
    }
}

/// A public MSM that never fails for lack of memory: when no plan fits
/// `budget` it falls back to independent complete-formula scalar
/// multiplications (slower, constant memory, the same result).
pub(crate) fn msm_public_or_naive<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> C {
    debug_assert_eq!(scalars.len(), bases.len());
    msm_public::<C>(scalars, bases, budget).unwrap_or_else(|_| msm_naive::<C>(scalars, bases))
}

/// A linear combination of points `sum_i scalars[i] * bases[i]` (public
/// data): the commitment a verifier opens, or the left-hand side it checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Msm<C: PastaCurve> {
    scalars: Vec<C::ScalarExt>,
    bases: Vec<C::AffineExt>,
}

impl<C: PastaCurve> Default for Msm<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: PastaCurve> Msm<C> {
    /// The empty combination (the identity).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scalars: Vec::new(),
            bases: Vec::new(),
        }
    }

    /// The single point `point`.
    #[must_use]
    pub fn from_point(point: C::AffineExt) -> Self {
        Self {
            scalars: vec![C::ScalarExt::ONE],
            bases: vec![point],
        }
    }

    /// Adds `scalar * point`.
    pub fn push(&mut self, scalar: C::ScalarExt, point: C::AffineExt) {
        self.scalars.push(scalar);
        self.bases.push(point);
    }

    /// Multiplies every term by `factor`.
    pub fn scale(&mut self, factor: &C::ScalarExt) {
        for scalar in &mut self.scalars {
            *scalar *= factor;
        }
    }

    /// Adds every term of `other`.
    pub fn add_msm(&mut self, other: &Self) {
        self.scalars.extend_from_slice(&other.scalars);
        self.bases.extend_from_slice(&other.bases);
    }

    /// The number of terms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scalars.len()
    }

    /// Whether there are no terms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scalars.is_empty()
    }

    /// The terms as `(scalar, base)` pairs.
    pub fn terms(&self) -> impl Iterator<Item = (&C::ScalarExt, &C::AffineExt)> {
        self.scalars.iter().zip(self.bases.iter())
    }

    /// Evaluates the combination (never fails for lack of memory).
    #[must_use]
    pub fn evaluate(&self, budget: MemoryBudget) -> C {
        msm_public_or_naive::<C>(&self.scalars, &self.bases, budget)
    }

    /// Whether the combination is the identity.
    #[must_use]
    pub fn is_identity(&self, budget: MemoryBudget) -> bool {
        bool::from(self.evaluate(budget).is_identity())
    }

    /// The evaluated combination in affine form.
    #[must_use]
    pub fn to_affine(&self, budget: MemoryBudget) -> C::AffineExt {
        self.evaluate(budget).to_affine()
    }
}

/// The key-generation blind as a field element.
pub(crate) fn default_blind<F: PastaField>() -> F {
    F::from(DEFAULT_BLIND)
}

#[cfg(test)]
mod tests {
    use group::Curve;
    use iroha_pasta::{Ep, Fq};
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    use super::*;

    fn params() -> ParamsIpa<Ep> {
        ParamsIpa::new(4).expect("k = 4")
    }

    #[test]
    fn commitments_agree_across_postures_and_tables() {
        let params = params();
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        let coeffs: Vec<Fq> = (0..16).map(|_| Fq::random(&mut rng)).collect();
        let blind = Fq::random(&mut rng);
        let budget = MemoryBudget::DEFAULT;
        let expected = msm_naive::<Ep>(&coeffs, params.g()) + params.w() * blind;
        let public = commit(&params, &coeffs, &blind, Secrecy::Public, budget).expect("msm");
        let secret = commit(&params, &coeffs, &blind, Secrecy::Secret, budget).expect("msm");
        assert_eq!(public, expected);
        assert_eq!(secret, expected);
        let tables = CommitmentTables::build(&params, budget);
        assert_eq!(tables.present(), (true, true));
        assert_eq!(
            tables.commit(&params, &coeffs, &blind, Secrecy::Secret, budget),
            Ok(expected)
        );
        let lagrange = msm_naive::<Ep>(&coeffs, params.g_lagrange()) + params.w() * blind;
        assert_eq!(
            commit_lagrange(&params, &coeffs, &blind, Secrecy::Public, budget),
            Ok(lagrange)
        );
        assert_eq!(
            tables.commit_lagrange(&params, &coeffs, &blind, Secrecy::Public, budget),
            Ok(lagrange)
        );
        // Prefix commitments skip the tables and use g[..m].
        let prefix = commit(&params, &coeffs[..5], &blind, Secrecy::Public, budget).expect("msm");
        assert_eq!(
            prefix,
            msm_naive::<Ep>(&coeffs[..5], &params.g()[..5]) + params.w() * blind
        );
        assert_eq!(
            tables.commit(&params, &coeffs[..5], &blind, Secrecy::Public, budget),
            Ok(prefix)
        );
        let too_long = vec![Fq::ONE; 17];
        assert!(commit(&params, &too_long, &blind, Secrecy::Public, budget).is_err());
        // An all-zero column commits to W, never the identity.
        let zero = commit_lagrange(
            &params,
            &[Fq::ZERO; 16],
            &default_blind(),
            Secrecy::Public,
            budget,
        )
        .expect("msm");
        assert_eq!(zero.to_affine(), params.w());
        assert_eq!(CommitmentTables::<Ep>::default().present(), (false, false));
        assert_eq!(
            CommitmentTables::<Ep>::build(&params, MemoryBudget::new(0)).present(),
            (false, false)
        );
    }

    #[test]
    fn msm_combinations_and_fallback() {
        let params = params();
        let g = params.g();
        let mut msm = Msm::<Ep>::from_point(g[0]);
        msm.push(Fq::from(2), g[1]);
        let mut other = Msm::<Ep>::new();
        other.push(-Fq::ONE, g[0]);
        msm.add_msm(&other);
        msm.scale(&Fq::from(3));
        assert_eq!(msm.len(), 3);
        assert!(!msm.is_empty());
        assert_eq!(msm.terms().count(), 3);
        let expected = g[1].to_curve() * Fq::from(6);
        assert_eq!(msm.evaluate(MemoryBudget::DEFAULT), expected);
        assert_eq!(msm.to_affine(MemoryBudget::DEFAULT), expected.to_affine());
        // A zero budget forces the naive fallback with the same result.
        let scalars: Vec<Fq> = (1..=16_u64).map(Fq::from).collect();
        assert_eq!(
            msm_public_or_naive::<Ep>(&scalars, g, MemoryBudget::new(0)),
            msm_naive::<Ep>(&scalars, g)
        );
        let mut zero = Msm::<Ep>::from_point(g[2]);
        zero.push(-Fq::ONE, g[2]);
        assert!(zero.is_identity(MemoryBudget::DEFAULT));
        assert!(Msm::<Ep>::default().is_identity(MemoryBudget::DEFAULT));
    }
}
