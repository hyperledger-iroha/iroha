//! In-circuit verification of a bound batched multilinear opening, bit for bit
//! [`iroha_plonk::pcs::mle::verify_batch_bound`], continuing the caller's
//! transcript on the duplex lane.
//!
//! The caller (the decider v2 verifier) has already bound the point `sigma` and
//! every claim `(C_c, v_c)` in its transcript. This circuit absorbs the tag,
//! `k` and the claim count, squeezes the 128-bit batching coefficients
//! (`alpha_0 = 1`), absorbs `rho*`, squeezes `z`, absorbs each round's `L_j`,
//! `R_j` before squeezing `u_j`, absorbs `G'_0` and `c`, and constrains the
//! complete group equation
//!
//! ```text
//! sum_c alpha_c C_c - rho* W + (v* - c b') z U + sum_j (u_j^-1 L_j + u_j R_j) - c G'_0 = 0,
//! v* = sum_c alpha_c v_c,   b' = prod_j (1 + sigma_{k-1-j} (u_j - 1)).
//! ```
//!
//! The group operations on `C` are native; `alpha_c` multiply as 128-bit
//! scalars, the other scalars are foreign FF values exported to canonical
//! limbs. Zero `z` or `u_j` make the circuit unsatisfiable (their inverses),
//! as the native verifier refuses them. The result is the undecided claim
//! `G'_0 = <s(u), g>` with its challenges; the lane stays live for the caller
//! to finish its transcript.

use ff::Field;
use iroha_pasta::{PastaCurve, params::ParamsIpa};
use iroha_plonk::{
    frontend::{Error, Region, Value},
    pcs::mle::{BatchOpening, TAG_BATCH_OPENING},
};
use iroha_plonk_gadgets::{
    ecc::{AssignedPoint, EccChip, ScalarLimbs},
    ff::FfValue,
};

use super::{FoldLanes, ScalarCell, transcript as t};

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
    let (first, rest) = claims.split_first().ok_or(Error::Synthesis)?;
    if opening.rounds.len() != k {
        return Err(Error::Synthesis);
    }

    // The transcript, exactly as the native bound verifier.
    t::absorb_u64(lanes, TAG_BATCH_OPENING);
    t::absorb_u64(lanes, k as u64);
    t::absorb_u64(lanes, claims.len() as u64);
    let mut alphas = Vec::with_capacity(rest.len());
    for _ in rest {
        alphas.push(t::challenge_128(lanes, region)?);
    }
    t::absorb_scalar(lanes, region, &opening.blind)?;
    let z = t::challenge(lanes, region)?;
    let mut challenges = Vec::with_capacity(k);
    for (left, right) in &opening.rounds {
        t::absorb_point(lanes, left);
        t::absorb_point(lanes, right);
        challenges.push(t::challenge(lanes, region)?);
    }
    t::absorb_point(lanes, &opening.folded_generator);
    t::absorb_scalar(lanes, region, &opening.scalar)?;

    // The field part: v*, b' and the U coefficient. z has an inverse.
    let _nonzero = lanes.ff.inverse(region, &z.ff)?;
    let mut value = first.1.ff.clone();
    for ((_, claim_value), (_, alpha)) in rest.iter().zip(&alphas) {
        let term = lanes.mul(region, &alpha.ff, &claim_value.ff)?;
        value = lanes.add(region, &value, &term)?;
    }
    let one = lanes.constant(region, C::ScalarExt::ONE)?;
    let mut folded = one.clone();
    for (challenge, coordinate) in challenges.iter().zip(point.iter().rev()) {
        let shifted = lanes.sub(region, &challenge.ff, &one)?;
        let product = lanes.mul(region, &coordinate.ff, &shifted)?;
        let factor = lanes.add(region, &one, &product)?;
        folded = lanes.mul(region, &folded, &factor)?;
    }
    let opened = lanes.mul(region, &opening.scalar.ff, &folded)?;
    let difference = lanes.sub(region, &value, &opened)?;
    let auxiliary_coefficient = lanes.mul(region, &difference, &z.ff)?;

    // The group part, with complete additions.
    let mut equation = first.0.clone();
    if !rest.is_empty() {
        let zero = lanes.uint().constant::<127>(region, 0)?;
        for ((commitment, _), (alpha_lo, _)) in rest.iter().zip(&alphas) {
            let (scaled, _) = lanes.ecc.mul(
                region,
                lanes.range,
                ScalarLimbs::new(alpha_lo, &zero),
                commitment,
            )?;
            equation = lanes.ecc.add(region, &equation, &scaled)?;
        }
    }
    let w = lanes.ecc.constant_point(region, &C::from(key.w))?;
    let zero = lanes.constant(region, C::ScalarExt::ZERO)?;
    let negative_blind = lanes.sub(region, &zero, &opening.blind.ff)?;
    let term = scale(lanes, region, &w, negative_blind)?;
    equation = lanes.ecc.add(region, &equation, &term)?;
    let u = lanes.ecc.constant_point(region, &C::from(key.u))?;
    let term = scale(lanes, region, &u, auxiliary_coefficient)?;
    equation = lanes.ecc.add(region, &equation, &term)?;
    for ((left, right), challenge) in opening.rounds.iter().zip(&challenges) {
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
        .enforce_constant(region, identity.word(), C::Base::ONE)?;
    Ok(OpeningClaimCells {
        folded_generator: opening.folded_generator.clone(),
        challenges,
    })
}
