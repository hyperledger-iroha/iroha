//! One inner-product argument opening many monomial-basis column commitments
//! at one point.
//!
//! For claims `(C_c, v_c)` that columns `a_c` (each `2^k` values, committed as
//! `C_c = <a_c, g> + rho_c W`) take the values `v_c = a_c(sigma)`:
//!
//! 1. absorb [`TAG_BATCH_OPENING`], `k`, the claim count, `sigma` and every
//!    `(C_c, v_c)`;
//! 2. draw one 128-bit challenge limb `lo` (none for a single claim), take
//!    its endoscalar `alpha = (2^64 + 2 B1 + 1) + zeta (2^64 + 2 B2 + 1)` for
//!    `lo = B1 + 2^64 B2` ([`super::endoscalar`]) and batch with its powers
//!    `alpha_c = alpha^c`, so the batch opens `a* = sum_c alpha_c a_c`
//!    against `C* = sum_c alpha_c C_c` to `v* = sum_c alpha_c v_c`. Distinct
//!    limbs give distinct endoscalars, so `alpha` is uniform on a set of
//!    `2^128`, and a false claim survives only if `alpha` is a root of the
//!    nonzero error polynomial `sum_c alpha^c e_c` of degree at most `m - 1`:
//!    probability at most `(m - 1) / 2^128`, next to the 128-bit fold
//!    challenges. A circuit evaluates both sums by Horner's rule, the group
//!    one with a 64-iteration endoscalar chain per step;
//! 3. reveal and absorb the combined blind `rho* = sum_c alpha_c rho_c`, so
//!    the verifier removes the blinds with one term `-rho* W`;
//! 4. squeeze `z`, then run `k` rounds of the BGH19 argument with the public
//!    vector `b = eq(sigma, .)`, top index bit first as in
//!    [`crate::pcs::ipa::prover`]: `L_j = <a_hi, G_lo> + <a_hi, b_lo> z U` and
//!    `R_j = <a_lo, G_hi> + <a_lo, b_hi> z U`, both absorbed, then `u_j`,
//!    `a_lo += u_j^-1 a_hi`, `b_lo += u_j b_hi`, `G_lo += u_j G_hi`;
//! 5. absorb the folded generator `G'_0` and the folded scalar `c = a'_0`.
//!
//! The verifier checks, with complete curve arithmetic,
//!
//! ```text
//! C* - rho* W + (v* - c b') z U + sum_j (u_j^-1 L_j + u_j R_j) - c G'_0 = 0,
//! b' = prod_j ((1 - sigma_{k-1-j}) + u_j sigma_{k-1-j})   (folded_eq)
//! ```
//!
//! and returns the claim `G'_0 = <s(u), g[0..2^k)>`. As with
//! [`crate::pcs::ipa::verifier::PendingOpening::accumulate`], that is not an
//! acceptance: a prover can solve the equation for `G'_0` after every
//! challenge, so only deciding the returned [`GeneratorClaim`] (or folding it
//! into an accumulator that is decided) makes the opening sound.
//!
//! [`open_batch_bound`] and [`verify_batch_bound`] skip step 1's `sigma` and
//! `(C_c, v_c)` for a caller whose transcript has already bound them (a
//! decider absorbs its accumulator digest and its evaluations, takes `sigma`
//! from its own challenges and its key commitments are fixed): they absorb only
//! the tag, `k` and the claim count before the batching coefficients. Without
//! that binding a prover picks the values after the coefficients and opens two
//! false claims whose combination is true (see the tests).
//!
//! The argument is not hiding: `L_j`, `R_j`, `c` and `rho*` reveal
//! combinations of the columns and blinds. Deciders are private witnesses of
//! the hiding wraps (DESIGN.md section 10); the column commitments themselves
//! stay blinded. Zero challenges (`z`, `u_j`; probability `2^-254`) are
//! refused rather than remapped, so an in-circuit verifier only needs the
//! inverses it computes anyway (plus one for `z`).

use ff::{BatchInvert, Field};
use group::CurveAffine;
use iroha_pasta::{
    CancellationToken, PastaCurve, PastaField,
    fold::fold_generators_cancellable,
    msm::{MemoryBudget, SharedMemoryBudget, msm_secret_cancellable},
    params::ParamsIpa,
};

use super::{MleTranscript, TAG_BATCH_OPENING, endoscalar, eq_table, folded_eq};
use crate::{
    pcs::ipa::{GeneratorClaim, IpaError, commit::Msm},
    secret::SecretPolynomial,
};

/// What an opening absorbs before its batching coefficients.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binding {
    /// The tag, `k`, the count, the point and every commitment and value.
    Absorb,
    /// The tag, `k` and the count: the caller has bound the rest.
    Bound,
}

/// A claim that the column committed as `commitment` takes `value` at the
/// batch's point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MleClaim<C: PastaCurve> {
    /// `<a, g[0..2^k)> + rho W`.
    pub commitment: C::AffineExt,
    /// `a(point)`.
    pub value: C::ScalarExt,
}

/// The opening proof of one batch (see the module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchOpening<C: PastaCurve> {
    /// The combined blind `rho* = sum_c alpha_c rho_c`.
    pub blind: C::ScalarExt,
    /// `(L_j, R_j)` for each round, top index bit first.
    pub rounds: Vec<(C::AffineExt, C::AffineExt)>,
    /// The claimed folded generator `G'_0 = <s(u), g[0..2^k)>`.
    pub folded_generator: C::AffineExt,
    /// The folded scalar `c`.
    pub scalar: C::ScalarExt,
}

/// Opens `columns` (each `2^k` values for `k = point.len()`, committed with
/// `blinds` on the first `2^k` monomial generators) at `point` as `claims`,
/// in `transcript`, and returns the proof and its generator claim.
///
/// # Errors
///
/// [`IpaError::LengthMismatch`] for an empty batch, unequal claim, column and
/// blind counts, or a column that is not `2^k` long;
/// [`IpaError::ParamsTooSmall`] when `k` exceeds the parameters;
/// [`IpaError::OpeningFailed`] when the claimed values are not the columns'
/// values at `point`, or for a zero `z`; [`IpaError::ZeroChallenge`] for a
/// zero round challenge; [`IpaError::Msm`] when an MSM does not fit `budget`.
pub fn open_batch<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    columns: &[&[C::ScalarExt]],
    blinds: &[C::ScalarExt],
    budget: MemoryBudget,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    open_batch_cancellable(
        params, transcript, point, claims, columns, blinds, budget, None,
    )
}

/// [`open_batch`] with cooperative cancellation. On cancellation, discard the
/// transcript: it is not a resumable prefix.
///
/// # Errors
///
/// As [`open_batch`], or [`IpaError::Cancelled`].
#[allow(clippy::too_many_arguments)]
pub fn open_batch_cancellable<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    columns: &[&[C::ScalarExt]],
    blinds: &[C::ScalarExt],
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    open(
        Binding::Absorb,
        params,
        transcript,
        point,
        claims,
        columns,
        blinds,
        budget,
        cancellation,
    )
}

/// [`open_batch`] for claims that `transcript` has already bound: it must
/// have absorbed `point` and every claim's commitment and value, through data
/// from which the verifier rebuilds the same claim list in the same order.
/// The opening then absorbs only [`TAG_BATCH_OPENING`], `k` and the claim
/// count before the batching coefficients.
///
/// # Errors
///
/// As [`open_batch`].
pub fn open_batch_bound<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    columns: &[&[C::ScalarExt]],
    blinds: &[C::ScalarExt],
    budget: MemoryBudget,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    open(
        Binding::Bound,
        params,
        transcript,
        point,
        claims,
        columns,
        blinds,
        budget,
        None,
    )
}

/// [`open_batch_bound`] with cooperative cancellation. On cancellation,
/// discard the transcript: it is not a resumable prefix.
///
/// # Errors
///
/// As [`open_batch`], or [`IpaError::Cancelled`].
#[allow(clippy::too_many_arguments)]
pub fn open_batch_bound_cancellable<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    columns: &[&[C::ScalarExt]],
    blinds: &[C::ScalarExt],
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    open(
        Binding::Bound,
        params,
        transcript,
        point,
        claims,
        columns,
        blinds,
        budget,
        cancellation,
    )
}

// The single-letter names follow the BGH19 notation, as in the IPA prover.
#[allow(clippy::too_many_arguments, clippy::many_single_char_names)]
fn open<C, T>(
    binding: Binding,
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    columns: &[&[C::ScalarExt]],
    blinds: &[C::ScalarExt],
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    CancellationToken::checkpoint(cancellation)?;
    let k = check_shape(params, point, claims.len())?;
    let n = 1_usize << k;
    for count in [columns.len(), blinds.len()] {
        if count != claims.len() {
            return Err(IpaError::LengthMismatch {
                expected: claims.len(),
                actual: count,
            });
        }
    }
    if let Some(column) = columns.iter().find(|column| column.len() != n) {
        return Err(IpaError::LengthMismatch {
            expected: n,
            actual: column.len(),
        });
    }

    let alphas = absorb_statement(transcript, binding, k, point, claims);
    let mut a = SecretPolynomial::new(vec![C::ScalarExt::ZERO; n]);
    let mut blind = C::ScalarExt::ZERO;
    let mut value = C::ScalarExt::ZERO;
    for (((column, column_blind), claim), alpha) in
        columns.iter().zip(blinds).zip(claims).zip(&alphas)
    {
        for (acc, entry) in a.iter_mut().zip(column.iter()) {
            *acc += *entry * alpha;
        }
        blind += *column_blind * alpha;
        value += claim.value * alpha;
    }
    CancellationToken::checkpoint(cancellation)?;
    let mut b = eq_table(point);
    let opened = a
        .iter()
        .zip(&b)
        .fold(C::ScalarExt::ZERO, |acc, (a, b)| acc + *a * b);
    if opened != value {
        return Err(IpaError::OpeningFailed);
    }
    transcript.absorb_scalars(&[blind]);
    let z = transcript.challenge();
    if bool::from(z.is_zero()) {
        return Err(IpaError::OpeningFailed);
    }

    let u_base = params.u().to_curve();
    let mut g = params.g()[..n].to_vec();
    let mut rounds = Vec::with_capacity(k);
    let mut challenges = Vec::with_capacity(k);
    for round in 0..k {
        CancellationToken::checkpoint(cancellation)?;
        let half = n >> (round + 1);
        let (a_lo, a_hi) = a.split_at(half);
        let (b_lo, b_hi) = b.split_at(half);
        let cross_l = inner_product(a_hi, b_lo);
        let cross_r = inner_product(a_lo, b_hi);
        let l = (msm_secret_cancellable::<C>(
            a_hi,
            &g[..half],
            budget,
            &SharedMemoryBudget::process_default(),
            cancellation,
        )? + u_base * (cross_l * z))
            .to_affine();
        let r = (msm_secret_cancellable::<C>(
            a_lo,
            &g[half..],
            budget,
            &SharedMemoryBudget::process_default(),
            cancellation,
        )? + u_base * (cross_r * z))
            .to_affine();
        transcript.absorb_point(&l);
        transcript.absorb_point(&r);
        rounds.push((l, r));

        let u = transcript.challenge();
        let u_inv =
            Option::<C::ScalarExt>::from(u.invert()).ok_or(IpaError::ZeroChallenge { round })?;
        challenges.push(u);
        let (a_lo, a_hi) = a.split_at_mut(half);
        for (lo, hi) in a_lo.iter_mut().zip(a_hi.iter()) {
            *lo += *hi * u_inv;
        }
        a.truncate(half);
        let (b_lo, b_hi) = b.split_at_mut(half);
        for (lo, hi) in b_lo.iter_mut().zip(b_hi.iter()) {
            *lo += *hi * u;
        }
        b.truncate(half);
        fold_generators_cancellable::<C>(&mut g, &u, cancellation)?;
        g.truncate(half);
    }

    CancellationToken::checkpoint(cancellation)?;
    let opening = BatchOpening {
        blind,
        rounds,
        folded_generator: g[0],
        scalar: a[0],
    };
    transcript.absorb_point(&opening.folded_generator);
    transcript.absorb_scalars(&[opening.scalar]);
    let claim = GeneratorClaim::new(k as u32, opening.folded_generator, challenges)?;
    Ok((opening, claim))
}

/// Verifies `opening` of `claims` at `point` in `transcript` and returns the
/// generator claim `G'_0 = <s(u), g[0..2^k)>` it rests on. This is not an
/// acceptance until that claim is decided (see the module documentation).
///
/// # Errors
///
/// [`IpaError::LengthMismatch`] for an empty batch or a round count other
/// than `k`; [`IpaError::ParamsTooSmall`] when `k` exceeds the parameters;
/// [`IpaError::OpeningFailed`] when the opening equation fails or `z` is zero;
/// [`IpaError::ZeroChallenge`] for a zero round challenge.
pub fn verify_batch<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    opening: &BatchOpening<C>,
    budget: MemoryBudget,
) -> Result<GeneratorClaim<C>, IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    verify(
        Binding::Absorb,
        params,
        transcript,
        point,
        claims,
        opening,
        budget,
    )
}

/// Verifies an [`open_batch_bound`] opening. `transcript` must have bound
/// `point` and every claim exactly as the prover's did (see
/// [`open_batch_bound`]); otherwise a verified opening proves nothing about
/// the individual claims.
///
/// # Errors
///
/// As [`verify_batch`].
pub fn verify_batch_bound<C, T>(
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    opening: &BatchOpening<C>,
    budget: MemoryBudget,
) -> Result<GeneratorClaim<C>, IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    verify(
        Binding::Bound,
        params,
        transcript,
        point,
        claims,
        opening,
        budget,
    )
}

// The single-letter names follow the BGH19 notation, as in the IPA verifier.
#[allow(clippy::many_single_char_names)]
fn verify<C, T>(
    binding: Binding,
    params: &ParamsIpa<C>,
    transcript: &mut T,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    opening: &BatchOpening<C>,
    budget: MemoryBudget,
) -> Result<GeneratorClaim<C>, IpaError>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    let k = check_shape(params, point, claims.len())?;
    if opening.rounds.len() != k {
        return Err(IpaError::LengthMismatch {
            expected: k,
            actual: opening.rounds.len(),
        });
    }
    let alphas = absorb_statement(transcript, binding, k, point, claims);
    transcript.absorb_scalars(&[opening.blind]);
    let z = transcript.challenge();
    if bool::from(z.is_zero()) {
        return Err(IpaError::OpeningFailed);
    }
    let mut challenges = Vec::with_capacity(k);
    for (round, (l, r)) in opening.rounds.iter().enumerate() {
        transcript.absorb_point(l);
        transcript.absorb_point(r);
        let u = transcript.challenge();
        if bool::from(u.is_zero()) {
            return Err(IpaError::ZeroChallenge { round });
        }
        challenges.push(u);
    }
    transcript.absorb_point(&opening.folded_generator);
    transcript.absorb_scalars(&[opening.scalar]);

    let value = claims
        .iter()
        .zip(&alphas)
        .fold(C::ScalarExt::ZERO, |acc, (claim, alpha)| {
            acc + claim.value * alpha
        });
    let b = folded_eq(point, &challenges).ok_or(IpaError::OpeningFailed)?;
    let mut inverses = challenges.clone();
    inverses.iter_mut().batch_invert();

    let mut msm = Msm::<C>::new();
    for (claim, alpha) in claims.iter().zip(&alphas) {
        msm.push(*alpha, claim.commitment);
    }
    msm.push(-opening.blind, params.w());
    msm.push((value - opening.scalar * b) * z, params.u());
    for ((l, r), (u, u_inv)) in opening.rounds.iter().zip(challenges.iter().zip(&inverses)) {
        msm.push(*u_inv, *l);
        msm.push(*u, *r);
    }
    msm.push(-opening.scalar, opening.folded_generator);
    if !msm.is_identity(budget) {
        return Err(IpaError::OpeningFailed);
    }
    GeneratorClaim::new(k as u32, opening.folded_generator, challenges)
}

/// `k = point.len()` for a non-empty batch within the parameters.
fn check_shape<C: PastaCurve>(
    params: &ParamsIpa<C>,
    point: &[C::ScalarExt],
    claims: usize,
) -> Result<usize, IpaError> {
    if claims == 0 {
        return Err(IpaError::LengthMismatch {
            expected: 1,
            actual: 0,
        });
    }
    let available = params.k();
    match u32::try_from(point.len()) {
        Ok(needed) if needed <= available => Ok(point.len()),
        _ => Err(IpaError::ParamsTooSmall {
            needed: u32::try_from(point.len()).unwrap_or(u32::MAX),
            available,
        }),
    }
}

/// Absorbs the statement (all of it, or only the tag, `k` and the count for a
/// caller that bound the rest) and returns the batching coefficients: the
/// powers of one 128-bit challenge (none drawn for a single claim).
fn absorb_statement<C, T>(
    transcript: &mut T,
    binding: Binding,
    k: usize,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
) -> Vec<C::ScalarExt>
where
    C: PastaCurve,
    T: MleTranscript<C> + ?Sized,
{
    transcript.absorb_u64(TAG_BATCH_OPENING);
    transcript.absorb_u64(k as u64);
    transcript.absorb_u64(claims.len() as u64);
    if binding == Binding::Absorb {
        transcript.absorb_scalars(point);
        for claim in claims {
            transcript.absorb_point(&claim.commitment);
            transcript.absorb_scalars(&[claim.value]);
        }
    }
    let alpha = if claims.len() > 1 {
        let limbs = transcript.challenge_128().to_canonical_limbs();
        endoscalar::<C::ScalarExt>(u128::from(limbs[0]) | (u128::from(limbs[1]) << 64))
    } else {
        C::ScalarExt::ONE
    };
    let mut alphas = Vec::with_capacity(claims.len());
    let mut power = C::ScalarExt::ONE;
    for _ in claims {
        alphas.push(power);
        power *= alpha;
    }
    alphas
}

fn inner_product<F: Field>(a: &[F], b: &[F]) -> F {
    a.iter().zip(b).fold(F::ZERO, |acc, (a, b)| acc + *a * b)
}
