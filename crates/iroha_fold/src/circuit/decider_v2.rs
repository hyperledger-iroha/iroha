//! In-circuit verification of a decider v2 proof up to its opening, bit for
//! bit [`crate::decider_v2::verify_many`], over the base field of the
//! commitment curve: the transcript and the group values are native, the
//! decider's scalars are foreign ([`ScalarCell`]).
//!
//! The decider transcript stays live after [`verify_decider_v2`] has absorbed
//! the claim values: the batched opening (`circuit::mle`) continues it and
//! finishes it. Every accumulator digest is therefore computed before the
//! decider transcript starts.

use ff::Field;
use iroha_pasta::PastaCurve;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{ecc::AssignedPoint, ff::FfValue};

use super::{
    FoldLanes, ScalarCell, transcript as t,
    verify::{AccumulatorCells, accumulator_digest},
};
use crate::{
    decider_v2::{
        DeciderV2Plan, DeciderV2Proof,
        eval::Op,
        mle,
        plan::{Committed, Source, Weight},
    },
    transcript::{TAG_DECIDER, TAG_DECIDER_PERMUTATION, TAG_DECIDER_SHIFT},
};

/// A decider v2 proof in circuit.
#[derive(Clone, Debug)]
pub struct DeciderV2ProofCells<C: PastaCurve> {
    /// The copy helper commitments.
    pub helpers: Vec<AssignedPoint<C::Base>>,
    /// The zero-check round values.
    pub zero_check: Vec<Vec<ScalarCell<C>>>,
    /// The committed tables at `rho`.
    pub at_rho: Vec<ScalarCell<C>>,
    /// The shift-reduction round values.
    pub shift: Vec<[ScalarCell<C>; 2]>,
    /// The committed polynomials at `sigma`.
    pub at_sigma: Vec<ScalarCell<C>>,
}

/// What the batched opening must prove: the point `sigma` and every claim
/// (commitment, value) in claim order.
#[derive(Clone, Debug)]
pub struct DeciderV2Claims<C: PastaCurve> {
    /// The opening point `sigma`.
    pub point: Vec<ScalarCell<C>>,
    /// The commitments and claimed values.
    pub claims: Vec<(AssignedPoint<C::Base>, ScalarCell<C>)>,
}

fn value_of<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

fn scalars<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    values: Option<&[C::ScalarExt]>,
    count: usize,
) -> Result<Vec<ScalarCell<C>>, Error> {
    if values.is_some_and(|v| v.len() != count) {
        return Err(Error::Synthesis);
    }
    (0..count).map(|i| lanes.assign_scalar(region, value_of(values.map(|v| &v[i])))).collect()
}

/// The shape of a batch: helper count, rounds, round degree, reads, claims.
fn shape<C: PastaCurve>(plans: &[&DeciderV2Plan<C>]) -> Result<(usize, usize, usize, usize, usize), Error> {
    let first = plans.first().ok_or(Error::Synthesis)?;
    if plans.iter().any(|plan| plan.mu != first.mu) {
        return Err(Error::Synthesis);
    }
    Ok((
        plans.iter().map(|plan| plan.copies.columns.len()).sum(),
        first.mu,
        plans.iter().map(|plan| plan.degree).max().unwrap_or(1),
        plans.iter().map(|plan| plan.reads.len()).sum(),
        plans.iter().map(|plan| plan.committed.len()).sum(),
    ))
}

/// Assigns a decider v2 proof for the batch of `plans` (`None`: key generation).
///
/// # Errors
/// Layout errors, or a proof of another shape.
pub fn assign_decider_v2_proof<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    plans: &[&DeciderV2Plan<C>],
    proof: Option<&DeciderV2Proof<C>>,
) -> Result<DeciderV2ProofCells<C>, Error> {
    let (helper_count, mu, degree, reads, committed) = shape(plans)?;
    if let Some(p) = proof {
        if p.helpers.len() != helper_count
            || p.zero_check.len() != mu
            || p.zero_check.iter().any(|round| round.len() != degree)
            || p.at_rho.len() != reads
            || p.shift.len() != mu
            || p.at_sigma.len() != committed
        {
            return Err(Error::Synthesis);
        }
    }
    let helpers = (0..helper_count)
        .map(|i| lanes.assign_point(region, value_of(proof.map(|p| C::from(p.helpers[i])).as_ref())))
        .collect::<Result<_, _>>()?;
    let zero_check = (0..mu)
        .map(|j| scalars(lanes, region, proof.map(|p| p.zero_check[j].as_slice()), degree))
        .collect::<Result<_, _>>()?;
    let at_rho = scalars(lanes, region, proof.map(|p| p.at_rho.as_slice()), reads)?;
    let shift = (0..mu)
        .map(|j| {
            let round = scalars(lanes, region, proof.map(|p| p.shift[j].as_slice()), 2)?;
            Ok([round[0].clone(), round[1].clone()])
        })
        .collect::<Result<_, Error>>()?;
    let at_sigma = scalars(lanes, region, proof.map(|p| p.at_sigma.as_slice()), committed)?;
    Ok(DeciderV2ProofCells { helpers, zero_check, at_rho, shift, at_sigma })
}

type Ff<C> = FfValue<<C as PastaCurve>::Base>;

/// Native-field helpers over the FF lane.
struct Arith<'l, 'a, C: PastaCurve> {
    lanes: &'l mut FoldLanes<'a, C>,
    one: Ff<C>,
}

impl<C: PastaCurve> Arith<'_, '_, C> {
    fn constant(&mut self, region: &mut Region<'_, C::Base>, value: C::ScalarExt) -> Result<Ff<C>, Error> {
        self.lanes.constant(region, value)
    }
    fn add(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>, b: &Ff<C>) -> Result<Ff<C>, Error> {
        self.lanes.add(region, a, b)
    }
    fn sub(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>, b: &Ff<C>) -> Result<Ff<C>, Error> {
        self.lanes.sub(region, a, b)
    }
    fn mul(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>, b: &Ff<C>) -> Result<Ff<C>, Error> {
        self.lanes.mul(region, a, b)
    }
    fn scale(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>, factor: C::ScalarExt) -> Result<Ff<C>, Error> {
        let factor = self.constant(region, factor)?;
        self.mul(region, a, &factor)
    }
    fn complement(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>) -> Result<Ff<C>, Error> {
        let one = self.one.clone();
        self.sub(region, &one, a)
    }
    fn inverse(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>) -> Result<Ff<C>, Error> {
        self.lanes.ff.inverse(region, a)
    }
    /// Constrains `a = b` as scalars (equal canonical limbs).
    fn assert_equal(&mut self, region: &mut Region<'_, C::Base>, a: &Ff<C>, b: &Ff<C>) -> Result<(), Error> {
        let a = self.lanes.export(region, a.clone())?;
        let b = self.lanes.export(region, b.clone())?;
        region.constrain_equal(a.s6.lo().word().cell(), b.s6.lo().word().cell())?;
        region.constrain_equal(a.s6.hi().word().cell(), b.s6.hi().word().cell())?;
        Ok(())
    }

    /// The polynomial through `(t, values[t])` for `t = 0..values.len()`, at `x`.
    fn interpolate(&mut self, region: &mut Region<'_, C::Base>, values: &[Ff<C>], x: &Ff<C>) -> Result<Ff<C>, Error> {
        let m = values.len();
        let mut diffs = Vec::with_capacity(m);
        for j in 0..m {
            let node = self.constant(region, C::ScalarExt::from(j as u64))?;
            diffs.push(self.sub(region, x, &node)?);
        }
        let mut prefix = vec![self.one.clone()];
        for d in diffs.iter().take(m - 1) {
            let next = self.mul(region, prefix.last().expect("seeded"), d)?;
            prefix.push(next);
        }
        let mut suffix = vec![self.one.clone(); m];
        for i in (0..m - 1).rev() {
            suffix[i] = self.mul(region, &suffix[i + 1], &diffs[i + 1])?;
        }
        let mut total = self.constant(region, C::ScalarExt::ZERO)?;
        for (i, value) in values.iter().enumerate() {
            let mut denominator = C::ScalarExt::ONE;
            for j in 0..m {
                if j != i {
                    denominator *= C::ScalarExt::from(i as u64) - C::ScalarExt::from(j as u64);
                }
            }
            let basis = self.mul(region, &prefix[i], &suffix[i])?;
            let weighted = self.mul(region, value, &basis)?;
            let term = self.scale(region, &weighted, denominator.invert().expect("distinct nodes"))?;
            total = self.add(region, &total, &term)?;
        }
        Ok(total)
    }

    /// `sum_j eq(rho, j) eq(sigma, (j + k) mod 2^mu)` by the carry automaton of
    /// the constant addition: state (carry 0, carry 1), one 2x2 step per bit,
    /// the final carry dropped.
    fn shifted_eq(
        &mut self,
        region: &mut Region<'_, C::Base>,
        rho: &[Ff<C>],
        sigma: &[Ff<C>],
        rotation: i32,
    ) -> Result<Ff<C>, Error> {
        let size = 1i64 << rho.len();
        let offset = i64::from(rotation).rem_euclid(size);
        let mut no_carry = self.one.clone();
        let mut carry = self.constant(region, C::ScalarExt::ZERO)?;
        for (bit, (x, y)) in rho.iter().zip(sigma).enumerate() {
            // Weights of (j bit, output bit): (0, 0) = (1 - x)(1 - y), (0, 1) =
            // (1 - x) y, (1, 0) = x (1 - y), (1, 1) = x y.
            let ones = self.mul(region, x, y)?;
            let zero_one = self.sub(region, y, &ones)?;
            let one_zero = self.sub(region, x, &ones)?;
            let not_x = self.complement(region, x)?;
            let zeros = self.sub(region, &not_x, &zero_one)?;
            let equal = self.add(region, &zeros, &ones)?;
            if (offset >> bit) & 1 == 0 {
                let stay = self.mul(region, &no_carry, &equal)?;
                let settle = self.mul(region, &carry, &zero_one)?;
                let keep = self.mul(region, &carry, &one_zero)?;
                no_carry = self.add(region, &stay, &settle)?;
                carry = keep;
            } else {
                let settle = self.mul(region, &no_carry, &zero_one)?;
                let start = self.mul(region, &no_carry, &one_zero)?;
                let keep = self.mul(region, &carry, &equal)?;
                carry = self.add(region, &start, &keep)?;
                no_carry = settle;
            }
        }
        self.add(region, &no_carry, &carry)
    }

    /// `eq(point, row)` for a constant row.
    fn eq_row(&mut self, region: &mut Region<'_, C::Base>, point: &[Ff<C>], row: usize) -> Result<Ff<C>, Error> {
        let mut acc = self.one.clone();
        for (j, x) in point.iter().enumerate() {
            let factor = if (row >> j) & 1 == 1 { x.clone() } else { self.complement(region, x)? };
            acc = self.mul(region, &acc, &factor)?;
        }
        Ok(acc)
    }
}

/// Verifies a decider v2 proof for every member `(plan, accumulator)`, up to
/// the opening; returns the point and the claims, with the decider transcript
/// still live for the batched opening.
///
/// # Errors
/// Layout errors, a proof or batch of another shape, or another transcript
/// live on the lane.
#[allow(clippy::too_many_lines)]
pub fn verify_decider_v2<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    members: &[(&DeciderV2Plan<C>, &AccumulatorCells<C>)],
    proof: &DeciderV2ProofCells<C>,
) -> Result<DeciderV2Claims<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let plans: Vec<&DeciderV2Plan<C>> = members.iter().map(|m| m.0).collect();
    let (helper_count, mu, degree, reads, committed) = shape(&plans)?;
    if proof.helpers.len() != helper_count
        || proof.zero_check.len() != mu
        || proof.zero_check.iter().any(|round| round.len() != degree)
        || proof.at_rho.len() != reads
        || proof.shift.len() != mu
        || proof.at_sigma.len() != committed
    {
        return Err(Error::Synthesis);
    }
    let digests = members
        .iter()
        .map(|(_, acc)| accumulator_digest(lanes, region, domain, acc))
        .collect::<Result<Vec<_>, _>>()?;
    t::start(lanes, TAG_DECIDER, domain)?;
    t::absorb_u64(lanes, members.len() as u64);
    for digest in &digests {
        t::absorb_word(lanes, digest);
    }
    t::absorb_u64(lanes, TAG_DECIDER_PERMUTATION);
    let gamma = t::challenge(lanes, region)?;
    let eta = t::challenge(lanes, region)?;
    for helper in &proof.helpers {
        t::absorb_point(lanes, helper);
    }
    let alpha = t::challenge(lanes, region)?;
    let kappa = t::challenge(lanes, region)?;
    let lambda = t::challenge(lanes, region)?;
    let tau = (0..mu).map(|_| t::challenge(lanes, region)).collect::<Result<Vec<_>, _>>()?;
    let one = lanes.constant(region, S::<C>::ONE)?;
    let mut ar = Arith { lanes, one };
    let zero = ar.constant(region, S::<C>::ZERO)?;

    // Powers of alpha and kappa.
    let max_alpha = plans.iter().map(|plan| plan.relation.groups.len() + 2).max().unwrap_or(2);
    let mut alpha_powers = vec![ar.one.clone()];
    for _ in 0..max_alpha {
        let next = ar.mul(region, alpha_powers.last().expect("seeded"), &alpha.ff)?;
        alpha_powers.push(next);
    }
    let max_kappa = plans.iter().map(|plan| plan.copies.columns.len()).max().unwrap_or(0);
    let mut kappa_powers = vec![ar.one.clone()];
    for _ in 0..max_kappa {
        let next = ar.mul(region, kappa_powers.last().expect("seeded"), &kappa.ff)?;
        kappa_powers.push(next);
    }

    // Claimed sum: sum_r lambda^r (e_r - alpha^(G_r+2) I_r).
    let mut claim = zero.clone();
    let mut scale = ar.one.clone();
    let mut scales = Vec::with_capacity(members.len());
    for (plan, acc) in members {
        let mut instance_terms = zero.clone();
        for (column, row, id, sigma) in &plan.copies.instance_cells {
            let value = acc.instance.public.get(*column).and_then(|c| c.get(*row)).map_or_else(|| zero.clone(), |cell| cell.ff.clone());
            let base = ar.add(region, &gamma.ff, &value)?;
            let id_term = ar.scale(region, &eta.ff, S::<C>::from(*id))?;
            let sigma_term = ar.scale(region, &eta.ff, S::<C>::from(*sigma))?;
            let left = ar.add(region, &base, &id_term)?;
            let right = ar.add(region, &base, &sigma_term)?;
            let left = ar.inverse(region, &left)?;
            let right = ar.inverse(region, &right)?;
            let difference = ar.sub(region, &left, &right)?;
            instance_terms = ar.add(region, &instance_terms, &difference)?;
        }
        let weighted = ar.mul(region, &alpha_powers[plan.relation.groups.len() + 2], &instance_terms)?;
        let member = ar.sub(region, &acc.e.ff, &weighted)?;
        let scaled = ar.mul(region, &scale, &member)?;
        claim = ar.add(region, &claim, &scaled)?;
        scales.push(scale.clone());
        scale = ar.mul(region, &scale, &lambda.ff)?;
    }

    // Zero-check rounds.
    let mut rho = Vec::with_capacity(mu);
    for round in &proof.zero_check {
        t::absorb_scalars(ar.lanes, region, round)?;
        let challenge = t::challenge(ar.lanes, region)?;
        let s1 = ar.sub(region, &claim, &round[0].ff)?;
        let mut values = vec![round[0].ff.clone(), s1];
        values.extend(round[1..].iter().map(|c| c.ff.clone()));
        claim = ar.interpolate(region, &values, &challenge.ff)?;
        rho.push(challenge);
    }
    let rho_ff: Vec<Ff<C>> = rho.iter().map(|c| c.ff.clone()).collect();
    let tau_ff: Vec<Ff<C>> = tau.iter().map(|c| c.ff.clone()).collect();

    // The final evaluation of every member's terms at rho.
    let mut total = zero.clone();
    let mut read_offset = 0;
    for ((plan, acc), scale) in members.iter().zip(&scales) {
        let relation = &plan.relation;
        let n = relation.n;
        let pb = relation.constraint_bits;
        let mut scalar_values: Vec<Ff<C>> = acc.theta.iter().map(|c| c.ff.clone()).collect();
        scalar_values.extend(acc.r.iter().map(|c| c.ff.clone()));
        scalar_values.push(gamma.ff.clone());
        scalar_values.push(eta.ff.clone());
        for (column, row) in &plan.cells {
            scalar_values.push(acc.instance.public.get(*column).and_then(|c| c.get(*row)).map_or_else(|| zero.clone(), |cell| cell.ff.clone()));
        }
        // pow over the constraint bits of beta.
        let mut pow_lo = vec![ar.one.clone()];
        for b in &acc.beta[..pb] {
            for i in 0..pow_lo.len() {
                let next = ar.mul(region, &pow_lo[i], &b.ff)?;
                pow_lo.push(next);
            }
        }
        let mut read_of = vec![None; plan.sources.len()];
        for (i, read) in plan.reads.iter().enumerate() {
            read_of[read.source] = Some(read_offset + i);
        }
        let mut leaves: Vec<Option<Ff<C>>> = vec![None; plan.sources.len()];
        for (index, source) in plan.sources.iter().enumerate() {
            let value = if let Some(i) = read_of[index] {
                proof.at_rho[i].ff.clone()
            } else {
                match *source {
                    Source::Instance(c, k) => {
                        let mut sum = zero.clone();
                        for (row, cell) in acc.instance.public[c].iter().enumerate() {
                            let weight = ar.eq_row(region, &rho_ff, mle::rotate(row, -k, n))?;
                            let term = ar.mul(region, &cell.ff, &weight)?;
                            sum = ar.add(region, &sum, &term)?;
                        }
                        sum
                    }
                    Source::Pow => {
                        let mut acc_pow = ar.one.clone();
                        for (x, b) in rho_ff.iter().zip(&acc.beta[pb..]) {
                            let xb = ar.mul(region, x, &b.ff)?;
                            let not_x = ar.complement(region, x)?;
                            let factor = ar.add(region, &not_x, &xb)?;
                            acc_pow = ar.mul(region, &acc_pow, &factor)?;
                        }
                        acc_pow
                    }
                    Source::EqTau => {
                        let mut acc_eq = ar.one.clone();
                        for (x, y) in tau_ff.iter().zip(&rho_ff) {
                            let xy = ar.mul(region, x, y)?;
                            let not_x = ar.complement(region, x)?;
                            let not_y_part = ar.sub(region, &not_x, y)?;
                            let twice = ar.add(region, &xy, &xy)?;
                            let factor = ar.add(region, &not_y_part, &twice)?;
                            acc_eq = ar.mul(region, &acc_eq, &factor)?;
                        }
                        acc_eq
                    }
                    Source::Usable => {
                        let usable = relation.usable;
                        if usable >= n {
                            ar.one.clone()
                        } else {
                            let mut sum = zero.clone();
                            let mut above = ar.one.clone();
                            for j in (0..mu).rev() {
                                let not_x = ar.complement(region, &rho_ff[j])?;
                                if (usable >> j) & 1 == 1 {
                                    let term = ar.mul(region, &above, &not_x)?;
                                    sum = ar.add(region, &sum, &term)?;
                                    above = ar.mul(region, &above, &rho_ff[j])?;
                                } else {
                                    above = ar.mul(region, &above, &not_x)?;
                                }
                            }
                            sum
                        }
                    }
                    Source::Index => {
                        let mut sum = zero.clone();
                        let mut power = S::<C>::ONE;
                        for x in &rho_ff {
                            let term = ar.scale(region, x, power)?;
                            sum = ar.add(region, &sum, &term)?;
                            power = power.double();
                        }
                        sum
                    }
                    Source::Witness(..) | Source::Key(..) | Source::Helper(_) => return Err(Error::Synthesis),
                }
            };
            leaves[index] = Some(value);
        }
        for term in &plan.terms {
            let weight = match term.weight {
                Weight::Index(c) => pow_lo.get(c).cloned().ok_or(Error::Synthesis)?,
                Weight::Alpha(k) => alpha_powers[k].clone(),
                Weight::AlphaKappa(k, j) => ar.mul(region, &alpha_powers[k], &kappa_powers[j])?,
            };
            let mut stack: Vec<Ff<C>> = Vec::new();
            for op in term.poly.ops() {
                match op {
                    Op::Const(value) => stack.push(ar.constant(region, *value)?),
                    Op::Table(index) => stack.push(leaves[*index as usize].clone().ok_or(Error::Synthesis)?),
                    Op::Scalar(index) => stack.push(scalar_values[*index as usize].clone()),
                    Op::Neg => {
                        let v = stack.pop().ok_or(Error::Synthesis)?;
                        stack.push(ar.sub(region, &zero, &v)?);
                    }
                    Op::Scale(factor) => {
                        let v = stack.pop().ok_or(Error::Synthesis)?;
                        stack.push(ar.scale(region, &v, *factor)?);
                    }
                    Op::Add => {
                        let b = stack.pop().ok_or(Error::Synthesis)?;
                        let a = stack.pop().ok_or(Error::Synthesis)?;
                        stack.push(ar.add(region, &a, &b)?);
                    }
                    Op::Mul => {
                        let b = stack.pop().ok_or(Error::Synthesis)?;
                        let a = stack.pop().ok_or(Error::Synthesis)?;
                        stack.push(ar.mul(region, &a, &b)?);
                    }
                }
            }
            let value = stack.pop().ok_or(Error::Synthesis)?;
            let weighted = ar.mul(region, &weight, &value)?;
            let scaled = ar.mul(region, scale, &weighted)?;
            total = ar.add(region, &total, &scaled)?;
        }
        read_offset += plan.reads.len();
    }
    ar.assert_equal(region, &total, &claim)?;

    // Shift reduction.
    t::absorb_scalars(ar.lanes, region, &proof.at_rho)?;
    t::absorb_u64(ar.lanes, TAG_DECIDER_SHIFT);
    let zeta = t::challenge(ar.lanes, region)?;
    let mut claim = zero.clone();
    let mut power = ar.one.clone();
    let mut zeta_powers = Vec::with_capacity(reads);
    for value in &proof.at_rho {
        zeta_powers.push(power.clone());
        let term = ar.mul(region, &power, &value.ff)?;
        claim = ar.add(region, &claim, &term)?;
        power = ar.mul(region, &power, &zeta.ff)?;
    }
    let mut sigma = Vec::with_capacity(mu);
    for round in &proof.shift {
        t::absorb_scalars(ar.lanes, region, round)?;
        let challenge = t::challenge(ar.lanes, region)?;
        let s1 = ar.sub(region, &claim, &round[0].ff)?;
        let values = [round[0].ff.clone(), s1, round[1].ff.clone()];
        claim = ar.interpolate(region, &values, &challenge.ff)?;
        sigma.push(challenge);
    }
    let sigma_ff: Vec<Ff<C>> = sigma.iter().map(|c| c.ff.clone()).collect();
    let mut by_rotation: Vec<(i32, Ff<C>)> = Vec::new();
    let mut expected = zero.clone();
    let (mut read_index, mut committed_offset) = (0, 0);
    for plan in &plans {
        for read in &plan.reads {
            let weight = match by_rotation.iter().position(|(k, _)| *k == read.rotation) {
                Some(slot) => by_rotation[slot].1.clone(),
                None => {
                    let w = ar.shifted_eq(region, &rho_ff, &sigma_ff, read.rotation)?;
                    by_rotation.push((read.rotation, w.clone()));
                    w
                }
            };
            let weighted = ar.mul(region, &zeta_powers[read_index], &weight)?;
            let term = ar.mul(region, &weighted, &proof.at_sigma[committed_offset + read.poly].ff)?;
            expected = ar.add(region, &expected, &term)?;
            read_index += 1;
        }
        committed_offset += plan.committed.len();
    }
    ar.assert_equal(region, &expected, &claim)?;
    t::absorb_scalars(ar.lanes, region, &proof.at_sigma)?;

    // The claims, in claim order.
    let mut claims = Vec::with_capacity(committed);
    let (mut committed_offset, mut helper_offset) = (0, 0);
    for (plan, acc) in members {
        for (i, poly) in plan.committed.iter().enumerate() {
            let commitment = match *poly {
                Committed::Witness(c) => acc.instance.commitments[c].clone(),
                Committed::Key(p) => ar.lanes.ecc.constant_point(region, &C::from(plan.key_commitments[p]))?,
                Committed::Helper(j) => proof.helpers[helper_offset + j].clone(),
            };
            claims.push((commitment, proof.at_sigma[committed_offset + i].clone()));
        }
        committed_offset += plan.committed.len();
        helper_offset += plan.copies.columns.len();
    }
    Ok(DeciderV2Claims { point: sigma, claims })
}
