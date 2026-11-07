//! The multilinear decider of a folded accumulator (v2; output `IVC.md`
//! section 11).
//!
//! An accumulator `(phi, beta, e)` with witness `w` decides when `w` opens every
//! commitment, satisfies every copy and `LogUp` sum, and
//! `e = sum_rows P(row) Phi(row)` (as [`crate::decide::decide`]). The v2 decider
//! reduces all of it to evaluation claims of committed multilinear polynomials
//! at one point `sigma`, which one batched IPA opening proves:
//!
//! 1. *Copies* (a `LogUp` permutation argument): from the accumulator's digest,
//!    challenges `gamma` and `eta`; for every column with copy cells the prover
//!    commits `z_j = 1 / (gamma + v + eta id) - 1 / (gamma + v + eta sigma)`.
//! 2. *Zero-check*: one sumcheck over the `mu` row bits of
//!    `S = P Phi + sum_g alpha^{1+g} L_g + alpha^{G+1} eq(tau, .) sum_j kappa^j C_j
//!    + alpha^{G+2} sum_j z_j`, with claimed sum `e - alpha^{G+2} I`. `P` is the
//!    `pow` extension of `beta`'s row bits; `Phi` sums the weighted gate,
//!    reciprocal-cell and table constraints, each gate's guard read from a key
//!    polynomial of its guard values; `L_g = sum_s a_s h_s - u g` is the `LogUp`
//!    sum over exactly the constrained cells; `C_j` is the copy row constraint of
//!    column `j`; `I` sums the public copy terms of instance cells.
//! 3. *Shift reduction*: the zero-check ends at `rho` with claimed values of every
//!    committed table at its rotations; one degree-two sumcheck reduces them to
//!    unshifted values at `sigma` through the weights `shifted_eq(rho, sigma, k)`.
//! 4. *Opening*: every committed polynomial (accumulator witness columns, key
//!    polynomials, copy helpers) at `sigma`, proved by one batched IPA opening
//!    (`iroha_plonk::pcs::mle`, [`open`] and [`verify_opening`]) that continues
//!    the same transcript and yields a generator claim. The decision is complete
//!    once that claim is decided or accumulated into a claim accumulator.
//!
//! [`prove_many`] and [`verify_many`] decide several accumulators of one field
//! (any relations with the same domain size and commitment key) at once. They
//! share the copy challenges, one zero-check over `sum_r lambda^r S_r`, one
//! shift reduction and one opening.
//!
//! Two transcripts (the DESIGN.md section 9.0 split): the *field transcript*
//! runs natively over the decider's scalar field ([`FieldTranscript`]) and
//! carries every field message, so that its circuit (over that field) checks
//! the sumchecks natively. Values computed on the commitment side enter it as
//! S6 limbs: each accumulator's digest `kgwfacd1`, which binds its commitments,
//! and [`helper_digest`] of the copy helpers. Its final digest `d_F` seeds the
//! *opening transcript* over the commitment curve's base field, where the
//! batched opening runs. Deciders are not zero-knowledge.

pub(crate) mod eval;
pub(crate) mod mle;
pub(crate) mod plan;
pub(crate) mod sumcheck;
#[cfg(test)]
mod tests;

use ff::Field;
use iroha_pasta::{PastaCurve, msm::MemoryBudget, params::ParamsIpa};
use rayon::prelude::*;
use iroha_plonk::pcs::{
    ipa::GeneratorClaim,
    mle::{self as pcs_mle, BatchOpening, MleClaim, open_batch_bound, verify_batch_bound},
};

pub use plan::DeciderV2Plan;
use plan::{Committed, CopyColumn, Source, Support, Weight};

use crate::{
    Error,
    commit::Witness,
    fold::{Accumulator, pow_vector},
    logup::batch_invert,
    transcript::{
        FieldTranscript, TAG_DECIDER_FIELD, TAG_DECIDER_HELPERS, TAG_DECIDER_OPENING, TAG_DECIDER_PERMUTATION,
        TAG_DECIDER_SHIFT, Transcript,
    },
};

/// The decider's messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeciderV2Proof<C: PastaCurve> {
    /// One copy helper commitment per column with copy cells.
    pub helpers: Vec<C::AffineExt>,
    /// The zero-check round polynomials at `0, 2, ..., d`.
    pub zero_check: Vec<Vec<C::ScalarExt>>,
    /// The committed tables at `rho`, in plan order.
    pub at_rho: Vec<C::ScalarExt>,
    /// The shift-reduction round polynomials at `0, 2`.
    pub shift: Vec<[C::ScalarExt; 2]>,
    /// The committed polynomials at `sigma`, in plan order.
    pub at_sigma: Vec<C::ScalarExt>,
}

/// The prover's output: the proof and what the batched opening needs.
#[derive(Clone, Debug)]
pub struct Proved<C: PastaCurve> {
    /// The decider's messages.
    pub proof: DeciderV2Proof<C>,
    /// The opening point `sigma`.
    pub point: Vec<C::ScalarExt>,
    /// One claim per committed polynomial.
    pub claims: Vec<MleClaim<C>>,
    /// The committed polynomials, in claim order.
    pub polys: Vec<Vec<C::ScalarExt>>,
    /// Their blinding scalars.
    pub blinds: Vec<C::ScalarExt>,
    /// The field transcript's final digest `d_F`, which seeds the opening.
    pub field_digest: C::ScalarExt,
    /// The opening transcript (over the commitment base field, seeded with
    /// `d_F`), for the batched opening to continue.
    pub transcript: Transcript<C>,
}

/// The verifier's output: the claims the batched opening must prove.
#[derive(Clone, Debug)]
pub struct Verified<C: PastaCurve> {
    /// The opening point `sigma`.
    pub point: Vec<C::ScalarExt>,
    /// One claim per committed polynomial.
    pub claims: Vec<MleClaim<C>>,
    /// The field transcript's final digest `d_F`, which seeds the opening.
    pub field_digest: C::ScalarExt,
    /// The opening transcript (over the commitment base field, seeded with
    /// `d_F`), for the batched opening to continue.
    pub transcript: Transcript<C>,
}

/// Challenges drawn after the copy helpers are bound.
struct Challenges<F> {
    alpha: F,
    kappa: F,
    lambda: F,
    tau: Vec<F>,
}

fn check_accumulator<C: PastaCurve>(plan: &DeciderV2Plan<C>, acc: &Accumulator<C>) -> Result<(), Error> {
    let relation = &plan.relation;
    if acc.beta.len() != relation.index_bits()
        || acc.instance.commitments.len() != relation.columns()
        || acc.instance.theta.len() != relation.theta_len
        || acc.instance.r.len() != relation.groups.len()
        || acc.instance.public.iter().map(Vec::len).ne(relation.instance_lengths.iter().copied())
    {
        return Err(Error::Shape("accumulator"));
    }
    Ok(())
}

/// The scalars: `theta`, `r`, `gamma`, `eta`, then the instance cells.
fn scalars<C: PastaCurve>(plan: &DeciderV2Plan<C>, acc: &Accumulator<C>, gamma: C::ScalarExt, eta: C::ScalarExt) -> Vec<C::ScalarExt> {
    let mut out = acc.instance.theta.clone();
    out.extend_from_slice(&acc.instance.r);
    out.push(gamma);
    out.push(eta);
    for (column, row) in &plan.cells {
        out.push(public_cell::<C>(&acc.instance.public, *column, *row));
    }
    out
}

fn public_cell<C: PastaCurve>(public: &[Vec<C::ScalarExt>], column: usize, row: usize) -> C::ScalarExt {
    public.get(column).and_then(|c| c.get(row)).copied().unwrap_or(<C::ScalarExt as Field>::ZERO)
}

/// The term coefficients of one accumulator, scaled by its batch weight.
fn weights<C: PastaCurve>(
    plan: &DeciderV2Plan<C>,
    acc: &Accumulator<C>,
    challenges: &Challenges<C::ScalarExt>,
    scale: C::ScalarExt,
) -> Vec<C::ScalarExt> {
    let pow_lo = pow_vector(&acc.beta[..plan.relation.constraint_bits]);
    let power = |base: C::ScalarExt, k: usize| base.pow_vartime([k as u64]);
    plan.terms
        .iter()
        .map(|term| {
            scale
                * match term.weight {
                    Weight::Index(c) => pow_lo[c],
                    Weight::Alpha(k) => power(challenges.alpha, k),
                    Weight::AlphaKappa(k, j) => power(challenges.alpha, k) * power(challenges.kappa, j),
                }
        })
        .collect()
}

/// The public copy terms of the instance cells.
fn instance_copy_terms<C: PastaCurve>(plan: &DeciderV2Plan<C>, acc: &Accumulator<C>, gamma: C::ScalarExt, eta: C::ScalarExt) -> Result<C::ScalarExt, Error> {
    let mut total = <C::ScalarExt as Field>::ZERO;
    for (column, row, id, sigma) in &plan.copies.instance_cells {
        let value = public_cell::<C>(&acc.instance.public, *column, *row);
        let left = gamma + value + eta * C::ScalarExt::from(*id);
        let right = gamma + value + eta * C::ScalarExt::from(*sigma);
        let (left, right): (Option<C::ScalarExt>, Option<C::ScalarExt>) = (left.invert().into(), right.invert().into());
        let (Some(left), Some(right)) = (left, right) else {
            return Err(Error::Rejected("copy denominator"));
        };
        total += left - right;
    }
    Ok(total)
}

/// The shared start of a decider's field transcript: the batch size, then per
/// member its digest (`kgwfacd1`, as limbs) and its field data natively
/// (public columns, `theta`, `r`, `beta`, `e`), then the copy challenges
/// `gamma` and `eta`.
fn start<'a, C: PastaCurve + 'a>(
    domain: &[u8; 32],
    accs: impl ExactSizeIterator<Item = &'a Accumulator<C>>,
) -> (FieldTranscript<C::ScalarExt>, C::ScalarExt, C::ScalarExt) {
    let mut transcript = FieldTranscript::<C::ScalarExt>::new(TAG_DECIDER_FIELD, domain);
    transcript.absorb_u64(accs.len() as u64);
    for acc in accs {
        transcript.absorb_foreign(&acc.digest(domain));
        transcript.absorb_u64(acc.instance.public.len() as u64);
        for column in &acc.instance.public {
            transcript.absorb_u64(column.len() as u64);
            transcript.absorb(column);
        }
        transcript.absorb(&acc.instance.theta);
        transcript.absorb(&acc.instance.r);
        transcript.absorb(&acc.beta);
        transcript.absorb(&[acc.e]);
    }
    transcript.absorb_u64(TAG_DECIDER_PERMUTATION);
    let gamma = transcript.challenge();
    let eta = transcript.challenge();
    (transcript, gamma, eta)
}

/// The digest of every member's copy-helper commitments, in member order, over
/// the commitment base field (the field transcript absorbs it as limbs).
#[must_use]
pub fn helper_digest<C: PastaCurve>(domain: &[u8; 32], helpers: &[C::AffineExt]) -> C::Base {
    let mut transcript = Transcript::<C>::new(TAG_DECIDER_HELPERS, domain);
    transcript.absorb_u64(helpers.len() as u64);
    for point in helpers {
        transcript.absorb_point(point);
    }
    transcript.digest()
}

/// The opening transcript, seeded with the field transcript's digest `d_F`.
fn opening_transcript<C: PastaCurve>(domain: &[u8; 32], field_digest: C::ScalarExt) -> Transcript<C> {
    let mut transcript = Transcript::<C>::new(TAG_DECIDER_OPENING, domain);
    transcript.absorb_scalars(&[field_digest]);
    transcript
}

fn draw<C: PastaCurve>(transcript: &mut FieldTranscript<C::ScalarExt>, mu: usize) -> Challenges<C::ScalarExt> {
    let alpha = transcript.challenge();
    let kappa = transcript.challenge();
    let lambda = transcript.challenge();
    let tau = (0..mu).map(|_| transcript.challenge()).collect();
    Challenges { alpha, kappa, lambda, tau }
}

/// The common row-bit count and the round degree of a batch.
fn batch_shape<'a, C: PastaCurve + 'a>(plans: impl Iterator<Item = &'a DeciderV2Plan<C>>) -> Result<(usize, usize), Error> {
    let mut shape: Option<(usize, usize)> = None;
    for plan in plans {
        shape = match shape {
            None => Some((plan.mu, plan.degree)),
            Some((mu, _)) if mu != plan.mu => return Err(Error::Shape("decider batch domains")),
            Some((mu, degree)) => Some((mu, degree.max(plan.degree))),
        };
    }
    shape.ok_or(Error::Shape("empty decider batch"))
}

/// The copy helper columns.
fn copy_helpers<C: PastaCurve>(plan: &DeciderV2Plan<C>, witness: &Witness<C::ScalarExt>, gamma: C::ScalarExt, eta: C::ScalarExt) -> Result<Vec<Vec<C::ScalarExt>>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let n = plan.relation.n;
    let mut helpers = Vec::with_capacity(plan.copies.columns.len());
    for (j, column) in plan.copies.columns.iter().enumerate() {
        let values = match column {
            CopyColumn::Witness(c) => &witness.columns[*c],
            CopyColumn::Fixed(c) => &plan.relation.fixed[*c],
        };
        let sigma = &plan.key_polys[plan.copies.sigma[j]];
        let rows = &plan.copies.rows[j];
        let mut denominators = Vec::with_capacity(2 * rows.len());
        for row in rows {
            let i = *row as usize;
            let id = S::<C>::from((j * n + i) as u64);
            denominators.push(gamma + values[i] + eta * id);
            denominators.push(gamma + values[i] + eta * sigma[i]);
        }
        if denominators.iter().any(|d| bool::from(d.is_zero())) {
            return Err(Error::Shape("copy denominator is zero"));
        }
        batch_invert(&mut denominators);
        let mut helper = vec![S::<C>::ZERO; n];
        for (k, row) in rows.iter().enumerate() {
            helper[*row as usize] = denominators[2 * k] - denominators[2 * k + 1];
        }
        helpers.push(helper);
    }
    Ok(helpers)
}

/// The pairs a term is evaluated on in the current round.
enum Pairs {
    List(Vec<u32>),
    Prefix(usize),
}

impl Pairs {
    fn of(support: &Support) -> Self {
        match support {
            Support::Rows(rows) => {
                let mut pairs: Vec<u32> = rows.iter().map(|r| r >> 1).collect();
                pairs.sort_unstable();
                pairs.dedup();
                Self::List(pairs)
            }
            Support::Prefix(count) => Self::Prefix(count.div_ceil(2)),
        }
    }

    fn halve(&mut self) {
        match self {
            Self::List(pairs) => {
                for p in pairs.iter_mut() {
                    *p >>= 1;
                }
                pairs.dedup();
            }
            Self::Prefix(count) => *count = count.div_ceil(2),
        }
    }
}

fn committed_poly<'a, C: PastaCurve>(
    plan: &'a DeciderV2Plan<C>,
    witness: &'a Witness<C::ScalarExt>,
    helpers: &'a [Vec<C::ScalarExt>],
    poly: Committed,
) -> &'a [C::ScalarExt] {
    match poly {
        Committed::Witness(c) => &witness.columns[c],
        Committed::Key(p) => &plan.key_polys[p],
        Committed::Helper(j) => &helpers[j],
    }
}

/// One accumulator of a batch, on the prover's side.
struct Lane<'a, C: PastaCurve> {
    plan: &'a DeciderV2Plan<C>,
    acc: &'a Accumulator<C>,
    witness: &'a Witness<C::ScalarExt>,
    helpers: Vec<Vec<C::ScalarExt>>,
    helper_commitments: Vec<C::AffineExt>,
}

/// The zero-check tables of one lane.
fn lane_tables<C: PastaCurve>(lane: &Lane<'_, C>, tau: &[C::ScalarExt]) -> Vec<Vec<C::ScalarExt>> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let (plan, acc, witness) = (lane.plan, lane.acc, lane.witness);
    let relation = &plan.relation;
    let n = relation.n;
    let pb = relation.constraint_bits;
    plan.sources
        .par_iter()
        .map(|source| match *source {
            Source::Witness(c, k) => mle::rotated(&witness.columns[c], k),
            Source::Key(p, k) => mle::rotated(&plan.key_polys[p], k),
            Source::Instance(c, k) => {
                let mut column = acc.instance.public[c].clone();
                column.resize(n, S::<C>::ZERO);
                mle::rotated(&column, k)
            }
            Source::Helper(j) => lane.helpers[j].clone(),
            Source::Pow => pow_vector(&acc.beta[pb..]),
            Source::EqTau => pcs_mle::eq_table(tau),
            Source::Usable => (0..n).map(|i| if i < relation.usable { S::<C>::ONE } else { S::<C>::ZERO }).collect(),
            Source::Index => (0..n).map(|i| S::<C>::from(i as u64)).collect(),
        })
        .collect()
}

/// Proves that `acc` with `witness` decides, down to opening claims at one point.
///
/// # Errors
/// As [`prove_many`].
pub fn prove<C: PastaCurve>(
    plan: &DeciderV2Plan<C>,
    domain: &[u8; 32],
    acc: &Accumulator<C>,
    witness: &Witness<C::ScalarExt>,
) -> Result<Proved<C>, Error> {
    prove_many(domain, &[(plan, acc, witness)])
}

/// Proves that every accumulator of `inputs` decides, down to opening claims at
/// one point. The accumulators share a field, a domain size and a commitment
/// key, and may belong to different relations. They share one copy phase, one
/// zero-check over `sum_r lambda^r S_r`, one shift reduction and one opening.
///
/// # Errors
/// [`Error::Shape`] for inputs of another shape, an empty batch, unequal domains
/// or a zero copy denominator (negligible); [`Error::Msm`].
#[allow(clippy::too_many_lines)]
pub fn prove_many<C: PastaCurve>(
    domain: &[u8; 32],
    inputs: &[(&DeciderV2Plan<C>, &Accumulator<C>, &Witness<C::ScalarExt>)],
) -> Result<Proved<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let (mu, m) = batch_shape(inputs.iter().map(|input| input.0))?;
    let n = 1usize << mu;
    for &(plan, acc, witness) in inputs {
        check_accumulator(plan, acc)?;
        if witness.columns.len() != plan.relation.columns() || witness.columns.iter().any(|c| c.len() != n) {
            return Err(Error::Shape("witness"));
        }
    }
    let (mut transcript, gamma, eta) = start(domain, inputs.iter().map(|input| input.1));
    let mut lanes = Vec::with_capacity(inputs.len());
    for &(plan, acc, witness) in inputs {
        let helpers = copy_helpers(plan, witness, gamma, eta)?;
        let slices: Vec<&[S<C>]> = helpers.iter().map(Vec::as_slice).collect();
        let helper_commitments = plan.key.commit_many(&slices, &vec![S::<C>::ZERO; helpers.len()])?;
        lanes.push(Lane { plan, acc, witness, helpers, helper_commitments });
    }
    let helpers: Vec<C::AffineExt> = lanes.iter().flat_map(|lane| lane.helper_commitments.iter().copied()).collect();
    transcript.absorb_foreign(&helper_digest::<C>(domain, &helpers));
    let challenges = draw::<C>(&mut transcript, mu);

    // Zero-check over every lane.
    let mut scale = S::<C>::ONE;
    let mut states = Vec::with_capacity(lanes.len());
    for lane in &lanes {
        let scalars = scalars(lane.plan, lane.acc, gamma, eta);
        let weights = weights(lane.plan, lane.acc, &challenges, scale);
        let tables = lane_tables(lane, &challenges.tau);
        let pairs: Vec<Pairs> = lane.plan.terms.iter().map(|t| Pairs::of(&t.support)).collect();
        states.push((scalars, weights, tables, pairs));
        scale *= challenges.lambda;
    }
    // Every (lane, term) is one parallel work item of a round.
    let work: Vec<(usize, usize)> = lanes
        .iter()
        .enumerate()
        .flat_map(|(li, lane)| (0..lane.plan.terms.len()).map(move |ti| (li, ti)))
        .collect();
    let mut zero_check = Vec::with_capacity(mu);
    let mut rho = Vec::with_capacity(mu);
    for _ in 0..mu {
        let sums = work
            .par_iter()
            .map_init(
                || Vec::with_capacity(32),
                |stack, &(li, ti)| {
                    let (scalars, weights, tables, pairs) = &states[li];
                    let term = &lanes[li].plan.terms[ti];
                    let half = tables[0].len() / 2;
                    let mut line = [S::<C>::ZERO; eval::LINE];
                    let mut add = |p: usize| {
                        let v = term.poly.eval_pair(tables, p, m, scalars, stack);
                        for (a, b) in line.iter_mut().zip(v).take(m) {
                            *a += b;
                        }
                    };
                    match &pairs[ti] {
                        Pairs::List(list) => list.iter().for_each(|p| add(*p as usize)),
                        Pairs::Prefix(count) => (0..(*count).min(half)).for_each(&mut add),
                    }
                    let weight = weights[ti];
                    line.iter().take(m).map(|v| weight * v).collect::<Vec<_>>()
                },
            )
            .reduce(
                || vec![S::<C>::ZERO; m],
                |mut a, b| {
                    for (x, y) in a.iter_mut().zip(b) {
                        *x += y;
                    }
                    a
                },
            );
        transcript.absorb(&sums);
        let challenge = transcript.challenge();
        states.par_iter_mut().for_each(|(_, _, tables, pairs)| {
            tables.par_iter_mut().for_each(|table| mle::fold(table, challenge));
            pairs.iter_mut().for_each(Pairs::halve);
        });
        zero_check.push(sums);
        rho.push(challenge);
    }
    let mut at_rho = Vec::new();
    for (lane, (_, _, tables, _)) in lanes.iter().zip(&states) {
        at_rho.extend(lane.plan.reads.iter().map(|read| tables[read.source][0]));
    }
    drop(states);

    // Shift reduction over every lane's reads.
    transcript.absorb(&at_rho);
    transcript.absorb_u64(TAG_DECIDER_SHIFT);
    let zeta = transcript.challenge();
    let mut power = S::<C>::ONE;
    let mut combined: Vec<(i32, Vec<S<C>>)> = Vec::new();
    for lane in &lanes {
        for read in &lane.plan.reads {
            let poly = committed_poly(lane.plan, lane.witness, &lane.helpers, lane.plan.committed[read.poly]);
            let slot = match combined.iter().position(|(k, _)| *k == read.rotation) {
                Some(slot) => slot,
                None => {
                    combined.push((read.rotation, vec![S::<C>::ZERO; n]));
                    combined.len() - 1
                }
            };
            combined[slot].1.par_iter_mut().zip(poly).for_each(|(q, v)| *q += power * v);
            power *= zeta;
        }
    }
    let mut shift_lanes: Vec<(Vec<S<C>>, Vec<S<C>>)> = Vec::with_capacity(combined.len());
    for (k, q) in combined {
        shift_lanes.push((pcs_mle::shifted_eq_table(&rho, k).ok_or(Error::Shape("shift weights"))?, q));
    }
    let mut shift = Vec::with_capacity(mu);
    let mut sigma = Vec::with_capacity(mu);
    let two = S::<C>::from(2);
    for _ in 0..mu {
        let half = shift_lanes[0].0.len() / 2;
        let (s0, s2) = shift_lanes
            .par_iter()
            .map(|(e, q)| {
                (0..half)
                    .into_par_iter()
                    .map(|p| {
                        let (e0, e1, q0, q1) = (e[2 * p], e[2 * p + 1], q[2 * p], q[2 * p + 1]);
                        (e0 * q0, (two * e1 - e0) * (two * q1 - q0))
                    })
                    .reduce(|| (S::<C>::ZERO, S::<C>::ZERO), |a, b| (a.0 + b.0, a.1 + b.1))
            })
            .reduce(|| (S::<C>::ZERO, S::<C>::ZERO), |a, b| (a.0 + b.0, a.1 + b.1));
        let sent = [s0, s2];
        transcript.absorb(&sent);
        let challenge = transcript.challenge();
        shift_lanes.par_iter_mut().for_each(|(e, q)| {
            mle::fold(e, challenge);
            mle::fold(q, challenge);
        });
        shift.push(sent);
        sigma.push(challenge);
    }

    // Every lane's committed polynomials at sigma.
    let mut polys = Vec::new();
    let mut commitments = Vec::new();
    let mut blinds = Vec::new();
    for lane in &lanes {
        for poly in &lane.plan.committed {
            polys.push(committed_poly(lane.plan, lane.witness, &lane.helpers, *poly).to_vec());
            let (commitment, blind) = match *poly {
                Committed::Witness(c) => (lane.acc.instance.commitments[c], lane.witness.blinds[c]),
                Committed::Key(p) => (lane.plan.key_commitments[p], S::<C>::ZERO),
                Committed::Helper(j) => (lane.helper_commitments[j], S::<C>::ZERO),
            };
            commitments.push(commitment);
            blinds.push(blind);
        }
    }
    let at_sigma = polys
        .par_iter()
        .map(|poly| pcs_mle::evaluate(poly, &sigma).ok_or(Error::Shape("evaluation")))
        .collect::<Result<Vec<S<C>>, _>>()?;
    transcript.absorb(&at_sigma);
    let field_digest = transcript.challenge();
    let claims = commitments.into_iter().zip(&at_sigma).map(|(commitment, value)| MleClaim { commitment, value: *value }).collect();
    let proof = DeciderV2Proof { helpers, zero_check, at_rho, shift, at_sigma };
    let transcript = opening_transcript::<C>(domain, field_digest);
    Ok(Proved { proof, point: sigma, claims, polys, blinds, field_digest, transcript })
}

/// The `eq(point, row)` weight of one hypercube row.
fn eq_row<F: Field>(point: &[F], row: usize) -> F {
    point.iter().enumerate().fold(F::ONE, |acc, (j, x)| if (row >> j) & 1 == 1 { acc * x } else { acc * (F::ONE - x) })
}

/// Verifies a decider proof for `acc` down to opening claims at one point.
///
/// # Errors
/// As [`verify_many`].
pub fn verify<C: PastaCurve>(
    plan: &DeciderV2Plan<C>,
    domain: &[u8; 32],
    acc: &Accumulator<C>,
    proof: &DeciderV2Proof<C>,
) -> Result<Verified<C>, Error> {
    verify_many(domain, &[(plan, acc)], proof)
}

/// Verifies a batched decider proof for every accumulator of `inputs` down to
/// opening claims at one point; the batched opening of those claims completes
/// the decision.
///
/// # Errors
/// [`Error::Shape`] for a proof, batch or accumulator of another shape,
/// [`Error::Rejected`] naming the failing check.
#[allow(clippy::too_many_lines)]
pub fn verify_many<C: PastaCurve>(
    domain: &[u8; 32],
    inputs: &[(&DeciderV2Plan<C>, &Accumulator<C>)],
    proof: &DeciderV2Proof<C>,
) -> Result<Verified<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let (mu, m) = batch_shape(inputs.iter().map(|input| input.0))?;
    let n = 1usize << mu;
    for &(plan, acc) in inputs {
        check_accumulator(plan, acc)?;
    }
    let helper_count: usize = inputs.iter().map(|(plan, _)| plan.copies.columns.len()).sum();
    let read_count: usize = inputs.iter().map(|(plan, _)| plan.reads.len()).sum();
    let committed_count: usize = inputs.iter().map(|(plan, _)| plan.committed.len()).sum();
    if proof.helpers.len() != helper_count
        || proof.zero_check.len() != mu
        || proof.zero_check.iter().any(|round| round.len() != m)
        || proof.at_rho.len() != read_count
        || proof.shift.len() != mu
        || proof.at_sigma.len() != committed_count
    {
        return Err(Error::Shape("decider proof"));
    }
    let (mut transcript, gamma, eta) = start(domain, inputs.iter().map(|input| input.1));
    transcript.absorb_foreign(&helper_digest::<C>(domain, &proof.helpers));
    let challenges = draw::<C>(&mut transcript, mu);

    // Zero-check.
    let mut claim = S::<C>::ZERO;
    let mut scale = S::<C>::ONE;
    for &(plan, acc) in inputs {
        let groups = plan.relation.groups.len();
        let terms = instance_copy_terms(plan, acc, gamma, eta)?;
        claim += scale * (acc.e - challenges.alpha.pow_vartime([(groups + 2) as u64]) * terms);
        scale *= challenges.lambda;
    }
    let mut rho = Vec::with_capacity(mu);
    for round in &proof.zero_check {
        transcript.absorb(round);
        let challenge = transcript.challenge();
        claim = sumcheck::next_claim(claim, round, challenge);
        rho.push(challenge);
    }
    let mut total = S::<C>::ZERO;
    let mut scale = S::<C>::ONE;
    let mut read_offset = 0;
    for &(plan, acc) in inputs {
        let relation = &plan.relation;
        let pb = relation.constraint_bits;
        let scalars = scalars(plan, acc, gamma, eta);
        let weights = weights(plan, acc, &challenges, scale);
        let mut read_of = vec![None; plan.sources.len()];
        for (i, read) in plan.reads.iter().enumerate() {
            read_of[read.source] = Some(read_offset + i);
        }
        let value = |index: usize| -> S<C> {
            if let Some(i) = read_of[index] {
                return proof.at_rho[i];
            }
            match plan.sources[index] {
                Source::Instance(c, k) => acc.instance.public[c]
                    .iter()
                    .enumerate()
                    .fold(S::<C>::ZERO, |sum, (row, x)| sum + *x * eq_row(&rho, mle::rotate(row, -k, n))),
                Source::Pow => mle::pow(&acc.beta[pb..], &rho),
                Source::EqTau => mle::eq(&challenges.tau, &rho),
                Source::Usable => mle::prefix_indicator(relation.usable, &rho),
                Source::Index => mle::index(0, &rho),
                Source::Witness(..) | Source::Key(..) | Source::Helper(_) => unreachable!("committed tables are read"),
            }
        };
        total += plan
            .terms
            .iter()
            .zip(&weights)
            .fold(S::<C>::ZERO, |sum, (term, weight)| sum + *weight * term.poly.eval(&value, &scalars));
        read_offset += plan.reads.len();
        scale *= challenges.lambda;
    }
    if total != claim {
        return Err(Error::Rejected("zero-check"));
    }

    // Shift reduction.
    transcript.absorb(&proof.at_rho);
    transcript.absorb_u64(TAG_DECIDER_SHIFT);
    let zeta = transcript.challenge();
    let mut claim = S::<C>::ZERO;
    let mut power = S::<C>::ONE;
    for v in &proof.at_rho {
        claim += power * v;
        power *= zeta;
    }
    let mut sigma = Vec::with_capacity(mu);
    for round in &proof.shift {
        transcript.absorb(round);
        let challenge = transcript.challenge();
        claim = sumcheck::next_claim(claim, round, challenge);
        sigma.push(challenge);
    }
    let mut expected = S::<C>::ZERO;
    let mut power = S::<C>::ONE;
    let mut by_rotation: Vec<(i32, S<C>)> = Vec::new();
    let mut committed_offset = 0;
    for &(plan, _) in inputs {
        for read in &plan.reads {
            let weight = match by_rotation.iter().position(|(k, _)| *k == read.rotation) {
                Some(slot) => by_rotation[slot].1,
                None => {
                    let w = pcs_mle::shifted_eq(&rho, &sigma, read.rotation).ok_or(Error::Shape("shift weights"))?;
                    by_rotation.push((read.rotation, w));
                    w
                }
            };
            expected += power * weight * proof.at_sigma[committed_offset + read.poly];
            power *= zeta;
        }
        committed_offset += plan.committed.len();
    }
    if expected != claim {
        return Err(Error::Rejected("shift reduction"));
    }
    transcript.absorb(&proof.at_sigma);
    let field_digest = transcript.challenge();
    let mut claims = Vec::with_capacity(committed_count);
    let (mut committed_offset, mut helper_offset) = (0, 0);
    for &(plan, acc) in inputs {
        for (i, poly) in plan.committed.iter().enumerate() {
            let commitment = match *poly {
                Committed::Witness(c) => acc.instance.commitments[c],
                Committed::Key(p) => plan.key_commitments[p],
                Committed::Helper(j) => proof.helpers[helper_offset + j],
            };
            claims.push(MleClaim { commitment, value: proof.at_sigma[committed_offset + i] });
        }
        committed_offset += plan.committed.len();
        helper_offset += plan.copies.columns.len();
    }
    let transcript = opening_transcript::<C>(domain, field_digest);
    Ok(Verified { point: sigma, claims, field_digest, transcript })
}

/// Opens the decider's claims with one batched IPA opening, continuing the
/// prover's transcript. Returns the opening and its generator claim.
///
/// The opening is `pcs::mle`'s bound variant: it does not re-absorb the claims,
/// because the decider transcript has already bound every one of them. Each
/// accumulator column is bound by the absorbed accumulator digest, each key
/// commitment is fixed by the plan, each helper is absorbed, every value is in
/// the absorbed `at_sigma`, and `sigma` is squeezed from the same transcript.
///
/// # Errors
/// [`Error::Rejected`] when the opening fails (claims not the polynomials'
/// values, a zero challenge, an MSM over budget).
pub fn open<C: PastaCurve>(
    params: &ParamsIpa<C>,
    proved: &mut Proved<C>,
) -> Result<(BatchOpening<C>, GeneratorClaim<C>), Error> {
    let columns: Vec<&[C::ScalarExt]> = proved.polys.iter().map(Vec::as_slice).collect();
    open_batch_bound(params, &mut proved.transcript, &proved.point, &proved.claims, &columns, &proved.blinds, MemoryBudget::DEFAULT)
        .map_err(|_| Error::Rejected("batched opening"))
}

/// Verifies the batched opening of a verified decider's claims, continuing its
/// transcript. Returns the generator claim, which still has to be decided or
/// folded into a claim accumulator.
///
/// # Errors
/// [`Error::Rejected`] when the opening equation fails.
pub fn verify_opening<C: PastaCurve>(
    params: &ParamsIpa<C>,
    verified: &mut Verified<C>,
    opening: &BatchOpening<C>,
) -> Result<GeneratorClaim<C>, Error> {
    verify_batch_bound(params, &mut verified.transcript, &verified.point, &verified.claims, opening, MemoryBudget::DEFAULT)
        .map_err(|_| Error::Rejected("batched opening"))
}
