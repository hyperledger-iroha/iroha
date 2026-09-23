//! Guarded inner IPA using the original protocol owner, RNG and transcript.
//!
//! Incoming P becomes b after its last use; S becomes P'. Both full physical columns
//! stay initialized until their guards erase them. A reusable half-column plus two
//! scalars guards the concatenation used by the ordinary MSM dispatch. Its affine
//! bases and the owned G' bank are included in the checked capacity bound. Inherited
//! keys/parameters, backend scalar representations and caches, generator-collapse
//! worker scratch, allocator overhead and compiler/register copies remain outside
//! this component bound. TODO: integrate the authenticated Core constructors and
//! qualify complete-process memory, erasure and latency before product admission.

use super::*;
use crate::{
    arithmetic::{best_multiexp, compute_inner_product},
    poly::ipa::commitment::{collapse_round_vectors, parallel_generator_collapse},
};

struct Scalars<F: StoredAssignmentFieldV1> {
    s_blind: SecretLookupBlindV1<F>,
    evaluation: SecretLookupBlindV1<F>,
    left: SecretLookupBlindV1<F>,
    right: SecretLookupBlindV1<F>,
    l_random: SecretLookupBlindV1<F>,
    r_random: SecretLookupBlindV1<F>,
}
impl<F: StoredAssignmentFieldV1> Scalars<F> {
    fn new() -> Self {
        Self {
            s_blind: SecretLookupBlindV1(Blind(F::ZERO)),
            evaluation: SecretLookupBlindV1(Blind(F::ZERO)),
            left: SecretLookupBlindV1(Blind(F::ZERO)),
            right: SecretLookupBlindV1(Blind(F::ZERO)),
            l_random: SecretLookupBlindV1(Blind(F::ZERO)),
            r_random: SecretLookupBlindV1(Blind(F::ZERO)),
        }
    }
    fn clear_round(&mut self) {
        clear(std::slice::from_mut(&mut self.left.0.0));
        clear(std::slice::from_mut(&mut self.right.0.0));
        clear(std::slice::from_mut(&mut self.l_random.0.0));
        clear(std::slice::from_mut(&mut self.r_random.0.0));
    }
}

/// Reusable guarded equivalent of the ordinary extra-term MSM concatenation.
struct MsmScratch<C: CurveAffine>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    scalars: Fields<C::Scalar>,
    bases: Vec<C>,
}
impl<C: CurveAffine> MsmScratch<C>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    fn new(half: usize) -> Result<Self, StoredLookupErrorV1> {
        let count = add(half, 2)?;
        let scalars = Fields::new(count)?;
        let mut bases = reserve(count)?;
        bases.resize(count, C::identity());
        Ok(Self { scalars, bases })
    }

    #[allow(clippy::too_many_arguments)]
    fn msm(
        &mut self,
        coefficients: &[C::Scalar],
        bases: &[C],
        value: C::Scalar,
        random: C::Scalar,
        z: C::Scalar,
        u: C,
        w: C,
    ) -> Result<C::Curve, StoredLookupErrorV1> {
        let count = add(coefficients.len(), 2)?;
        if coefficients.len() != bases.len()
            || count > self.scalars.0.len()
            || count > self.bases.len()
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let n = coefficients.len();
        self.scalars.0[..n].copy_from_slice(coefficients);
        self.bases[..n].copy_from_slice(bases);
        self.scalars.0[n] = value * z;
        self.scalars.0[n + 1] = random;
        self.bases[n] = u;
        self.bases[n + 1] = w;
        // Same joined scalar/base order and backend dispatch as best_multiexp_with_extra.
        // The containing guard clears all initialized slots if this dispatch unwinds.
        let result = best_multiexp(&self.scalars.0[..count], &self.bases[..count]);
        clear(&mut self.scalars.0);
        Ok(result)
    }
}

struct Workspace<C: CurveAffine>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    s: Column<C::Scalar>,
    g: Vec<C>,
    msm: MsmScratch<C>,
    scalars: Scalars<C::Scalar>,
}
impl<C: CurveAffine> Workspace<C>
where
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    fn new(domain: &EvaluationDomain<C::Scalar>, g: &[C]) -> Result<Self, StoredLookupErrorV1> {
        let s = Column::new(domain, g.len())?;
        let mut owned_g = reserve(g.len())?;
        owned_g.extend_from_slice(g);
        Ok(Self {
            s,
            g: owned_g,
            msm: MsmScratch::new(g.len() / 2)?,
            scalars: Scalars::new(),
        })
    }
    fn actual_payload(&self, p: usize) -> Result<usize, StoredLookupErrorV1> {
        payload::<C>(
            p,
            self.s.0.values.capacity(),
            self.msm.scalars.0.capacity(),
            self.g.capacity(),
            self.msm.bases.capacity(),
        )
    }
}

fn payload<C: CurveAffine>(
    p: usize,
    s: usize,
    msm_scalars: usize,
    g: usize,
    msm_bases: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    add(
        add(
            mul(
                add(add(p, s)?, msm_scalars)?,
                std::mem::size_of::<C::Scalar>(),
            )?,
            mul(add(g, msm_bases)?, std::mem::size_of::<C>())?,
        )?,
        add(
            std::mem::size_of::<Workspace<C>>(),
            add(
                std::mem::size_of::<Column<C::Scalar>>(),
                std::mem::size_of::<SecretLookupBlindV1<C::Scalar>>(),
            )?,
        )?,
    )
}

/// Erase removed initialized halves without truncating either guarded physical column.
fn collapse<F: StoredAssignmentFieldV1>(
    p: &mut [F],
    b: &mut [F],
    live: &mut usize,
    u: F,
) -> Result<F, StoredLookupErrorV1> {
    if p.len() != b.len() || *live < 2 || *live > p.len() || !live.is_power_of_two() {
        return Err(StoredLookupErrorV1::Context);
    }
    let inverse = Option::<F>::from(u.invert()).ok_or(StoredLookupErrorV1::Context)?;
    let half = *live / 2;
    collapse_round_vectors(&mut p[..*live], &mut b[..*live], half, u, inverse);
    clear(&mut p[half..*live]);
    clear(&mut b[half..*live]);
    *live = half;
    Ok(inverse)
}

/// Closed completed continuation; no detached proof, key, RNG or transcript accessor.
#[allow(dead_code)]
pub(crate) struct CompletedStoredIpaProofV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const Q: bool,
    const M: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    inner: ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
}

/// Checked logical payload minimum, with the incoming P allocation's actual capacity.
pub(in crate::plonk::prover::stored) fn scratch_bytes<C, P, R, T, E, const Q: bool, const M: u64>(
    owner: &PreparedStoredIpaOpeningV1<'_, '_, C, P, R, T, E, Q, M>,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    let n = owner.inner.validate()?.n;
    let params = owner.inner.inner.inner.inner.params;
    if n == 0 || owner.p.0.len() != n || 1_usize.checked_shl(params.k) != Some(n) {
        return Err(StoredLookupErrorV1::Context);
    }
    let extra = add(n / 2, 2)?;
    payload::<C>(owner.p.0.values.capacity(), n, extra, n, extra)
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    PreparedStoredIpaOpeningV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Consume the original P frontier through the unchanged inner-IPA event sequence.
    /// Every full field sample and external protocol effect is bracketed by owner checks.
    pub(crate) fn finish_guarded_ipa(
        mut self,
        scratch_limit_bytes: usize,
    ) -> Result<
        CompletedStoredIpaProofV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        if scratch_bytes(&self)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let params = self.inner.inner.inner.inner.params;
        let n = self.p.0.len();
        let mut work = Workspace::new(&self.inner.inner.inner.inner.pk.vk.domain, &params.g)?;
        if work.actual_payload(self.p.0.values.capacity())? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        self.inner.validate()?;
        for scalar in &mut work.s.0.values {
            self.inner.validate()?;
            *scalar = C::Scalar::random(&mut self.inner.inner.inner.inner.rng);
            self.inner.validate()?;
        }
        evaluate(&work.s.0.values, *self.x3, &mut work.scalars.evaluation.0.0);
        work.s.0.values[0] -= work.scalars.evaluation.0.0;
        clear(std::slice::from_mut(&mut work.scalars.evaluation.0.0));
        self.inner.validate()?;
        work.scalars.s_blind.0 = Blind(C::Scalar::random(&mut self.inner.inner.inner.inner.rng));
        self.inner.validate()?;
        let commitment = params.commit(&work.s.0, work.scalars.s_blind.0).to_affine();
        self.inner.validate()?;
        self.inner
            .inner
            .inner
            .inner
            .transcript
            .write_point(commitment)
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.inner.validate()?;
        let xi = *self
            .inner
            .inner
            .inner
            .inner
            .transcript
            .squeeze_challenge_scalar::<()>();
        self.inner.validate()?;
        let z = *self
            .inner
            .inner
            .inner
            .inner
            .transcript
            .squeeze_challenge_scalar::<()>();
        self.inner.validate()?;
        fold(&mut work.s.0.values, xi, &self.p.0.values)?;
        evaluate(&work.s.0.values, *self.x3, &mut work.scalars.evaluation.0.0);
        work.s.0.values[0] -= work.scalars.evaluation.0.0;
        clear(std::slice::from_mut(&mut work.scalars.evaluation.0.0));
        self.blind.0.0 = work.scalars.s_blind.0.0 * xi + self.blind.0.0;
        clear(std::slice::from_mut(&mut work.scalars.s_blind.0.0));
        // P has had its final use. Reuse that same fully initialized guarded allocation for b.
        clear(&mut self.p.0.values);
        self.inner.validate()?;
        let mut power = C::Scalar::ONE;
        for scalar in &mut self.p.0.values {
            *scalar = power;
            power *= *self.x3;
        }
        self.inner.validate()?;
        let mut live = n;
        for _ in 0..params.k {
            let half = live / 2;
            work.scalars.left.0.0 =
                compute_inner_product(&work.s.0.values[half..live], &self.p.0.values[..half]);
            work.scalars.right.0.0 =
                compute_inner_product(&work.s.0.values[..half], &self.p.0.values[half..live]);
            self.inner.validate()?;
            work.scalars.l_random.0.0 = C::Scalar::random(&mut self.inner.inner.inner.inner.rng);
            self.inner.validate()?;
            work.scalars.r_random.0.0 = C::Scalar::random(&mut self.inner.inner.inner.inner.rng);
            self.inner.validate()?;
            let l = work
                .msm
                .msm(
                    &work.s.0.values[half..live],
                    &work.g[..half],
                    work.scalars.left.0.0,
                    work.scalars.l_random.0.0,
                    z,
                    params.u,
                    params.w,
                )?
                .to_affine();
            self.inner.validate()?;
            let r = work
                .msm
                .msm(
                    &work.s.0.values[..half],
                    &work.g[half..live],
                    work.scalars.right.0.0,
                    work.scalars.r_random.0.0,
                    z,
                    params.u,
                    params.w,
                )?
                .to_affine();
            self.inner.validate()?;
            self.inner
                .inner
                .inner
                .inner
                .transcript
                .write_point(l)
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            self.inner.validate()?;
            self.inner
                .inner
                .inner
                .inner
                .transcript
                .write_point(r)
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            self.inner.validate()?;
            let u = *self
                .inner
                .inner
                .inner
                .inner
                .transcript
                .squeeze_challenge_scalar::<()>();
            self.inner.validate()?;
            let inverse = collapse(&mut work.s.0.values, &mut self.p.0.values, &mut live, u)?;
            self.inner.validate()?;
            parallel_generator_collapse(&mut work.g[..2 * half], u);
            self.inner.validate()?;
            work.g.truncate(half);
            self.blind.0.0 += work.scalars.l_random.0.0 * inverse;
            self.blind.0.0 += work.scalars.r_random.0.0 * u;
            work.scalars.clear_round();
            self.inner.validate()?;
        }
        if live != 1 {
            return Err(StoredLookupErrorV1::Context);
        }
        self.inner.validate()?;
        self.inner
            .inner
            .inner
            .inner
            .transcript
            .write_scalar(work.s.0.values[0])
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.inner.validate()?;
        self.inner
            .inner
            .inner
            .inner
            .transcript
            .write_scalar(self.blind.0.0)
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.inner.validate()?;
        drop(work);
        self.inner.validate()?;
        let Self {
            inner, p, blind, ..
        } = self;
        drop(p);
        drop(blind);
        inner.validate()?;
        Ok(CompletedStoredIpaProofV1 { inner })
    }
}

#[cfg(test)]
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    CompletedStoredIpaProofV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    pub(in crate::plonk::prover::stored) fn observed_inner(
        &self,
    ) -> &ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M> {
        &self.inner
    }
}

#[cfg(test)]
#[path = "inner_ipa_tests.rs"]
mod tests;
