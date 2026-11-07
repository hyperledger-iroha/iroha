//! In-circuit verification of a bound batched multilinear opening, bit for bit
//! [`iroha_plonk::pcs::mle::verify_batch_bound`], continuing the caller's
//! transcript on the duplex lane.
//!
//! The caller (the decider v2 verifier) has already bound the point `sigma` and
//! every claim `(C_c, v_c)` in its transcript. This circuit absorbs the tag,
//! `k` and the claim count, squeezes the 128-bit batching challenge limb
//! (two or more claims) whose endoscalar is `alpha` (`alpha_c = alpha^c`,
//! [`iroha_plonk::pcs::mle::endoscalar`]), absorbs `rho*`, squeezes `z`,
//! absorbs each round's `L_j`,
//! `R_j` before squeezing `u_j`, absorbs `G'_0` and `c`, and constrains the
//! complete group equation
//!
//! ```text
//! sum_c alpha_c C_c - rho* W + (v* - c b') z U + sum_j (u_j^-1 L_j + u_j R_j) - c G'_0 = 0,
//! v* = sum_c alpha_c v_c,   b' = prod_j (1 + sigma_{k-1-j} (u_j - 1)).
//! ```
//!
//! The group operations on `C` are native: `C*` is one Horner chain of
//! 64-iteration endoscalar multiplications on the limb's digit words
//! ([`EccChip::horner_endo`]) and `v*` the same Horner rule in FF arithmetic
//! on `alpha`'s scalar; the other scalars are foreign FF values exported to
//! canonical limbs. Zero `z` or `u_j` make the circuit unsatisfiable (their inverses),
//! as the native verifier refuses them. The result is the undecided claim
//! `G'_0 = <s(u), g>` with its challenges; the lane stays live for the caller
//! to finish its transcript.

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use iroha_pasta::{PastaCurve, params::ParamsIpa};
use iroha_plonk::{
    frontend::{Error, Region, Value},
    pcs::mle::{BatchOpening, TAG_BATCH_OPENING},
};
use iroha_plonk_gadgets::{
    U128,
    ecc::{AssignedPoint, EccChip, ScalarLimbs},
    ff::{CanonicalS6, FfValue},
};

use super::{FoldLanes, ScalarCell, scalar_modulus, transcript as t};

/// The fixed generators an opening uses besides the column commitments.
#[derive(Clone, Debug)]
pub struct OpeningKey<C: PastaCurve> {
    w: C::AffineExt,
    u: C::AffineExt,
}

impl<C: PastaCurve> OpeningKey<C> {
    /// The blinding generator `W` and the inner-product generator `U` of `params`.
    #[must_use]
    pub fn new(params: &ParamsIpa<C>) -> Self {
        Self {
            w: params.w(),
            u: params.u(),
        }
    }
}

/// A batch opening in circuit.
#[derive(Clone, Debug)]
pub struct BatchOpeningCells<C: PastaCurve> {
    /// The combined blind `rho*`.
    pub blind: ScalarCell<C>,
    /// `(L_j, R_j)` for each round.
    pub rounds: Vec<(AssignedPoint<C::Base>, AssignedPoint<C::Base>)>,
    /// The claimed folded generator `G'_0`.
    pub folded_generator: AssignedPoint<C::Base>,
    /// The folded scalar `c`.
    pub scalar: ScalarCell<C>,
}

/// The claim an opening rests on: `G'_0 = <s(u), g[0..2^k)>`.
#[derive(Clone, Debug)]
pub struct OpeningClaimCells<C: PastaCurve> {
    /// `G'_0`.
    pub folded_generator: AssignedPoint<C::Base>,
    /// `u_0..u_{k-1}`, top index bit first.
    pub challenges: Vec<ScalarCell<C>>,
}

fn value_of<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// Assigns an opening of `k` rounds; unknown values without one.
///
/// # Errors
/// [`Error::Synthesis`] for an opening of another round count; layout errors.
pub fn assign_batch_opening<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    k: usize,
    opening: Option<&BatchOpening<C>>,
) -> Result<BatchOpeningCells<C>, Error> {
    if opening.is_some_and(|opening| opening.rounds.len() != k) {
        return Err(Error::Synthesis);
    }
    let point = |value: Option<&C::AffineExt>| value_of(value.map(|p| C::from(*p)).as_ref());
    let blind = lanes.assign_scalar(region, value_of(opening.map(|o| &o.blind)))?;
    let mut rounds = Vec::with_capacity(k);
    for round in 0..k {
        let left = lanes.assign_point(region, point(opening.map(|o| &o.rounds[round].0)))?;
        let right = lanes.assign_point(region, point(opening.map(|o| &o.rounds[round].1)))?;
        rounds.push((left, right));
    }
    let folded_generator =
        lanes.assign_point(region, point(opening.map(|o| &o.folded_generator)))?;
    let scalar = lanes.assign_scalar(region, value_of(opening.map(|o| &o.scalar)))?;
    Ok(BatchOpeningCells {
        blind,
        rounds,
        folded_generator,
        scalar,
    })
}

/// `[s] P` for a foreign FF value `s`, through its canonical limbs.
fn scale<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    point: &AssignedPoint<C::Base>,
    scalar: FfValue<C::Base>,
) -> Result<AssignedPoint<C::Base>, Error> {
    let scalar = lanes.export(region, scalar)?;
    scale_cell(lanes, region, point, &scalar)
}

fn scale_cell<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    point: &AssignedPoint<C::Base>,
    scalar: &ScalarCell<C>,
) -> Result<AssignedPoint<C::Base>, Error> {
    let (scaled, _) = lanes.ecc.mul(
        region,
        lanes.range,
        ScalarLimbs::new(scalar.s6.lo(), scalar.s6.hi()),
        point,
    )?;
    Ok(scaled)
}

/// Verifies `opening` of `claims` (commitment, value) at `point` for a caller
/// whose live transcript already bound them, and returns the undecided claim.
///
/// # Errors
/// [`Error::Synthesis`] for an empty batch or a round count other than
/// `point.len()`; layout errors. A false opening is unsatisfiable.
pub fn verify_batch_bound<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    key: &OpeningKey<C>,
    point: &[ScalarCell<C>],
    claims: &[(AssignedPoint<C::Base>, ScalarCell<C>)],
    opening: &BatchOpeningCells<C>,
) -> Result<OpeningClaimCells<C>, Error> {
    let k = point.len();
    let (last, init) = claims.split_last().ok_or(Error::Synthesis)?;
    let challenges = opening_transcript(lanes, region, k, claims.len(), opening)?;
    // v* = sum_c alpha^c v_c by Horner's rule, the last claim first.
    let mut value = last.1.ff.clone();
    if let Some(alpha) = &challenges.alpha {
        for (_, claim_value) in init.iter().rev() {
            let scaled = lanes.mul(region, &value, &alpha.scalar.ff)?;
            value = lanes.add(region, &scaled, &claim_value.ff)?;
        }
    }
    // b' = prod_j (1 + sigma_{k-1-j} (u_j - 1)).
    let one = lanes.constant(region, C::ScalarExt::ONE)?;
    let mut folded = one.clone();
    for (challenge, coordinate) in challenges.rounds.iter().zip(point.iter().rev()) {
        let shifted = lanes.sub(region, &challenge.ff, &one)?;
        let product = lanes.mul(region, &coordinate.ff, &shifted)?;
        let factor = lanes.add(region, &one, &product)?;
        folded = lanes.mul(region, &folded, &factor)?;
    }
    let commitments: Vec<AssignedPoint<C::Base>> = claims
        .iter()
        .map(|(commitment, _)| commitment.clone())
        .collect();
    group_equation(
        lanes,
        region,
        key,
        &commitments,
        &challenges,
        &value,
        &folded,
        opening,
    )?;
    Ok(OpeningClaimCells {
        folded_generator: opening.folded_generator.clone(),
        challenges: challenges.rounds,
    })
}

/// The group side of a split decider opening and what it hands back to the
/// field side.
#[derive(Clone, Debug)]
pub struct GroupOpeningCells<C: PastaCurve> {
    /// The undecided claim `(G'_0, u)`; its `u_j` go to the field side.
    pub claim: OpeningClaimCells<C>,
    /// The batching challenge (its digit words and its scalar, whose S6 limbs
    /// go to the field side's `v*`); `None` for a single claim.
    pub alpha: Option<EndoAlpha<C>>,
}

/// The batching challenge as an endoscalar: the digit words of the squeezed
/// 128-bit limb `lo = B1 + 2^64 B2` and its scalar
/// `alpha = (2^64 + 2 B1 + 1) + zeta (2^64 + 2 B2 + 1)`.
#[derive(Clone, Debug)]
pub struct EndoAlpha<C: PastaCurve> {
    /// `[B1, B2]`, below `2^64` (bounded by every chain that copies them).
    pub digits: [U128<C::Base>; 2],
    /// `alpha`, canonical.
    pub scalar: ScalarCell<C>,
}

/// The group side of [`verify_batch_bound`] for a decider whose field part
/// runs natively in the other circuit: the same opening transcript and group
/// equation, with `v* = sum_c alpha^c v_c` and `b' = folded_eq(sigma, u)`
/// computed by that field side (which binds the claim values and `sigma` in
/// its own transcript) and imported here as `value` and `folded`. It returns
/// `alpha` and the round challenges for the field side, which must use exactly
/// these values (through the public instances that join the two circuits).
///
/// # Errors
/// [`Error::Synthesis`] for no commitments or a round count other than `k`;
/// layout errors. A false opening is unsatisfiable.
#[allow(clippy::too_many_arguments)]
pub fn verify_batch_bound_group<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    key: &OpeningKey<C>,
    k: usize,
    commitments: &[AssignedPoint<C::Base>],
    value: &ScalarCell<C>,
    folded: &ScalarCell<C>,
    opening: &BatchOpeningCells<C>,
) -> Result<GroupOpeningCells<C>, Error> {
    if commitments.is_empty() {
        return Err(Error::Synthesis);
    }
    let challenges = opening_transcript(lanes, region, k, commitments.len(), opening)?;
    group_equation(
        lanes,
        region,
        key,
        commitments,
        &challenges,
        &value.ff,
        &folded.ff,
        opening,
    )?;
    Ok(GroupOpeningCells {
        claim: OpeningClaimCells {
            folded_generator: opening.folded_generator.clone(),
            challenges: challenges.rounds,
        },
        alpha: challenges.alpha,
    })
}

/// The opening's challenges, squeezed exactly as the native bound verifier.
struct OpeningChallenges<C: PastaCurve> {
    alpha: Option<EndoAlpha<C>>,
    z: ScalarCell<C>,
    rounds: Vec<ScalarCell<C>>,
}

/// Absorbs the tag, `k` and the count, squeezes `alpha` (two or more claims),
/// absorbs `rho*`, squeezes `z`, absorbs each round's `L_j`, `R_j` before
/// squeezing `u_j`, then absorbs `G'_0` and `c`.
fn opening_transcript<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    k: usize,
    claims: usize,
    opening: &BatchOpeningCells<C>,
) -> Result<OpeningChallenges<C>, Error> {
    if opening.rounds.len() != k {
        return Err(Error::Synthesis);
    }
    t::absorb_u64(lanes, TAG_BATCH_OPENING);
    t::absorb_u64(lanes, k as u64);
    t::absorb_u64(lanes, claims as u64);
    let alpha = if claims > 1 {
        let (lo, _) = t::challenge_128(lanes, region)?;
        Some(endo_alpha(lanes, region, &lo)?)
    } else {
        None
    };
    t::absorb_scalar(lanes, region, &opening.blind)?;
    let z = t::challenge(lanes, region)?;
    let mut rounds = Vec::with_capacity(k);
    for (left, right) in &opening.rounds {
        t::absorb_point(lanes, left);
        t::absorb_point(lanes, right);
        rounds.push(t::challenge(lanes, region)?);
    }
    t::absorb_point(lanes, &opening.folded_generator);
    t::absorb_scalar(lanes, region, &opening.scalar)?;
    Ok(OpeningChallenges { alpha, z, rounds })
}

/// Splits the squeezed limb `lo` into its digit words (`lo = B1 + 2^64 B2`;
/// the endoscalar chains copy and so bound them below `2^64`) and computes
/// `alpha = (2^64 + 1)(1 + zeta) + 2 B1 + 2 zeta B2` in FF arithmetic.
fn endo_alpha<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    lo: &U128<C::Base>,
) -> Result<EndoAlpha<C>, Error> {
    let value = lo.value();
    let low = lanes
        .uint()
        .assign::<128>(region, value.map(|lo| lo & u128::from(u64::MAX)))?;
    let high = lanes
        .uint()
        .assign::<128>(region, value.map(|lo| lo >> 64))?;
    let combined = lanes.glue.linear(
        region,
        &[
            (C::Base::ONE, low.word()),
            (C::Base::from_u128(1 << 64), high.word()),
        ],
        C::Base::ZERO,
    )?;
    region.constrain_equal(combined.cell(), lo.word().cell())?;
    let zero = lanes.uint().constant::<127>(region, 0)?;
    let mut import = |lanes: &mut FoldLanes<'_, C>, digit: &U128<C::Base>| {
        let s6 = CanonicalS6::from_limbs(
            &mut lanes.uint(),
            region,
            scalar_modulus::<C>(),
            digit,
            &zero,
        )?;
        lanes.import(region, s6).map(|cell| cell.ff)
    };
    let low_ff = import(lanes, &low)?;
    let high_ff = import(lanes, &high)?;
    let zeta = <C::ScalarExt as WithSmallOrderMulGroup<3>>::ZETA;
    let two_64 = C::ScalarExt::from(1_u64 << 32).square();
    let offset = lanes.constant(
        region,
        (two_64 + C::ScalarExt::ONE) * (C::ScalarExt::ONE + zeta),
    )?;
    let two = lanes.constant(region, C::ScalarExt::from(2))?;
    let two_zeta = lanes.constant(region, zeta.double())?;
    let low_term = lanes.mul(region, &low_ff, &two)?;
    let high_term = lanes.mul(region, &high_ff, &two_zeta)?;
    let partial = lanes.add(region, &offset, &low_term)?;
    let alpha = lanes.add(region, &partial, &high_term)?;
    Ok(EndoAlpha {
        digits: [low, high],
        scalar: lanes.export(region, alpha)?,
    })
}

/// Constrains the opening equation with complete additions,
///
/// ```text
/// sum_c alpha^c C_c - rho* W + (v* - c b') z U + sum_j (u_j^-1 L_j + u_j R_j) - c G'_0 = O,
/// ```
///
/// with `z` and every `u_j` invertible.
#[allow(clippy::too_many_arguments)]
fn group_equation<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    key: &OpeningKey<C>,
    commitments: &[AssignedPoint<C::Base>],
    challenges: &OpeningChallenges<C>,
    value: &FfValue<C::Base>,
    folded: &FfValue<C::Base>,
    opening: &BatchOpeningCells<C>,
) -> Result<(), Error> {
    let _nonzero = lanes.ff.inverse(region, &challenges.z.ff)?;
    let opened = lanes.mul(region, &opening.scalar.ff, folded)?;
    let difference = lanes.sub(region, value, &opened)?;
    let auxiliary_coefficient = lanes.mul(region, &difference, &challenges.z.ff)?;
    // C* = sum_c alpha^c C_c as one Horner chain of endoscalar
    // multiplications on alpha's digit words.
    let mut equation = match (&challenges.alpha, commitments) {
        (Some(alpha), _) => lanes.ecc.horner_endo(
            region,
            [alpha.digits[0].word(), alpha.digits[1].word()],
            commitments,
        )?,
        (None, [only]) => only.clone(),
        (None, _) => return Err(Error::Synthesis),
    };
    let w = lanes.ecc.constant_point(region, &C::from(key.w))?;
    let zero = lanes.constant(region, C::ScalarExt::ZERO)?;
    let negative_blind = lanes.sub(region, &zero, &opening.blind.ff)?;
    let term = scale(lanes, region, &w, negative_blind)?;
    equation = lanes.ecc.add(region, &equation, &term)?;
    let u = lanes.ecc.constant_point(region, &C::from(key.u))?;
    let term = scale(lanes, region, &u, auxiliary_coefficient)?;
    equation = lanes.ecc.add(region, &equation, &term)?;
    for ((left, right), challenge) in opening.rounds.iter().zip(&challenges.rounds) {
        let inverse = lanes.ff.inverse(region, &challenge.ff)?;
        let term = scale(lanes, region, left, inverse)?;
        equation = lanes.ecc.add(region, &equation, &term)?;
        let term = scale_cell(lanes, region, right, challenge)?;
        equation = lanes.ecc.add(region, &equation, &term)?;
    }
    let negative_scalar = lanes.sub(region, &zero, &opening.scalar.ff)?;
    let term = scale(lanes, region, &opening.folded_generator, negative_scalar)?;
    equation = lanes.ecc.add(region, &equation, &term)?;
    let identity = EccChip::<C>::is_identity(lanes.glue, region, &equation)?;
    lanes
        .glue
        .enforce_constant(region, identity.word(), C::Base::ONE)
}
