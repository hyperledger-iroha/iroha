//! In-circuit decider v2 verification, split by field (the DESIGN.md section
//! 9.0 placement agreed with Claude b04a57).
//!
//! **Field side** ([`verify_decider_v2_field`], a circuit over the decider's
//! scalar field `F`). It replays the field transcript bit for bit
//! [`crate::decider_v2::verify_many`] and checks the zero-check rounds, the
//! final term evaluation, the shift reduction and every succinct table in
//! native arithmetic. It outputs `sigma`, the claim values at `sigma` and the
//! field transcript's digest `d_F`. [`opening_scalars`] computes the batched
//! opening's `v* = sum_c alpha^c v_c` and `b' = folded_eq(sigma, u)` from the
//! opening's `alpha` and `u` imported as limbs. Values entering from the
//! commitment side (each accumulator's digest `kgwfacd1`, the helper digest)
//! arrive as S6 limbs. Values leaving (`sigma`, `d_F`, `v*`, `b'`) leave as S6
//! limbs ([`FieldLanes::limbs`]).
//!
//! The transcript absorbs only each member's digest, which commits to its field
//! data. A member's public columns are needed only for the instance copy term
//! `I`: either the field side holds them and computes `I`
//! ([`assign_accumulator_field`]), or `I` is computed where they are hashed,
//! with the copy challenges this side exports, and imported
//! ([`assign_accumulator_field_terms`]; the plan must read public values only
//! through copies).
//!
//! **Commitment side** (a circuit over the commitment curve's base field, on
//! [`FoldLanes`]): [`helper_digest`] computes the copy helpers' digest, and
//! [`opening_transcript`] starts the opening transcript from `d_F`. The
//! batched opening's group part (`circuit::mle`) continues it, and
//! `circuit::verify::accumulator_digest` gives `kgwfacd1`.

use std::collections::HashMap;

use ff::{Field, PrimeField};
use iroha_pasta::{PastaCurve, PastaField, poseidon::PoseidonField};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Word, ecc::AssignedPoint};

use super::{FoldLanes, ScalarCell, field::FieldLanes, transcript as t, verify::AccumulatorCells};
use crate::{
    decider_v2::{
        DeciderV2Plan, DeciderV2Proof, batch_claims,
        eval::Op,
        helper_offsets, mle,
        plan::{Committed, Source, Weight},
    },
    fold::Accumulator,
    relation::FoldShape,
    transcript::{
        TAG_DECIDER_FIELD, TAG_DECIDER_HELPERS, TAG_DECIDER_OPENING, TAG_DECIDER_PERMUTATION,
        TAG_DECIDER_SHIFT, s6_limbs,
    },
};

type W<C> = Word<<C as PastaCurve>::ScalarExt>;

/// One accumulator's field data in a field-side circuit: its digest
/// `kgwfacd1` as limbs (from the commitment side), its public columns or its
/// imported instance copy term, and `theta`, `r`, `beta` and `e` as native
/// words.
#[derive(Clone, Debug)]
pub struct AccumulatorFieldCells<C: PastaCurve> {
    /// `kgwfacd1` as `(lo, hi)` limbs.
    pub digest: [W<C>; 2],
    /// The folded public columns, when this side holds them (empty otherwise).
    pub public: Vec<Vec<W<C>>>,
    /// The instance copy term `I`, when imported instead of the public columns.
    pub copy_terms: Option<W<C>>,
    /// `theta` powers.
    pub theta: Vec<W<C>>,
    /// One `LogUp` challenge per group.
    pub r: Vec<W<C>>,
    /// `pow` challenges.
    pub beta: Vec<W<C>>,
    /// The accumulated error.
    pub e: W<C>,
}

/// The field messages of a decider v2 proof in a field-side circuit.
#[derive(Clone, Debug)]
pub struct DeciderV2FieldProofCells<C: PastaCurve> {
    /// The zero-check round values.
    pub zero_check: Vec<Vec<W<C>>>,
    /// The committed tables at `rho`.
    pub at_rho: Vec<W<C>>,
    /// The shift-reduction round values.
    pub shift: Vec<[W<C>; 2]>,
    /// The committed polynomials at `sigma` (the claim values).
    pub at_sigma: Vec<W<C>>,
}

/// What the field side hands on: `sigma`, the claim values, `d_F` and the copy
/// challenges.
#[derive(Clone, Debug)]
pub struct DeciderV2FieldOutput<C: PastaCurve> {
    /// The copy challenges `gamma` and `eta` (exported to the circuit that
    /// computes an imported instance copy term).
    pub copy_challenges: [W<C>; 2],
    /// The opening point `sigma`.
    pub point: Vec<W<C>>,
    /// The claim values, in claim order.
    pub at_sigma: Vec<W<C>>,
    /// The field transcript's digest `d_F`.
    pub field_digest: W<C>,
}

fn witnesses<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    values: Option<&[C::ScalarExt]>,
    count: usize,
) -> Result<Vec<W<C>>, Error> {
    if values.is_some_and(|v| v.len() != count) {
        return Err(Error::Synthesis);
    }
    (0..count)
        .map(|i| {
            lanes.glue.witness(
                region,
                values.map_or_else(Value::unknown, |v| Value::known(v[i])),
            )
        })
        .collect()
}

/// Assigns an accumulator's field data and its digest limbs (`None`: key
/// generation). The wrap constrains these cells to the values crossing from
/// the commitment side.
///
/// # Errors
/// Layout errors, or an accumulator of another shape.
pub fn assign_accumulator_field<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    domain: &[u8; 32],
    shape: &FoldShape,
    acc: Option<&Accumulator<C>>,
) -> Result<AccumulatorFieldCells<C>, Error> {
    let limbs = acc.map(|a| {
        let [lo, hi] = s6_limbs(&a.digest(domain));
        [C::ScalarExt::from_u128(lo), C::ScalarExt::from_u128(hi)]
    });
    let digest = witnesses::<C>(lanes, region, limbs.as_ref().map(<[_; 2]>::as_slice), 2)?;
    let public = shape
        .instance_lengths
        .iter()
        .enumerate()
        .map(|(c, len)| {
            witnesses::<C>(
                lanes,
                region,
                acc.map(|a| a.instance.public[c].as_slice()),
                *len,
            )
        })
        .collect::<Result<_, _>>()?;
    let theta = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.instance.theta.as_slice()),
        shape.theta_len,
    )?;
    let r = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.instance.r.as_slice()),
        shape.groups.len(),
    )?;
    let beta = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.beta.as_slice()),
        shape.index_bits,
    )?;
    let e = witnesses::<C>(lanes, region, acc.map(|a| core::slice::from_ref(&a.e)), 1)?.remove(0);
    Ok(AccumulatorFieldCells {
        digest: [digest[0].clone(), digest[1].clone()],
        public,
        copy_terms: None,
        theta,
        r,
        beta,
        e,
    })
}

/// Assigns an accumulator's field data without its public columns: the digest
/// limbs, `theta`, `r`, `beta`, `e`, and the instance copy term `I`
/// ([`crate::decider_v2::instance_copy_terms`]), which the circuit hashing the
/// public columns computes with the exported copy challenges (`None`: key
/// generation). The wrap constrains every cell to the values crossing from the
/// commitment side.
///
/// # Errors
/// Layout errors, or an accumulator of another shape.
pub fn assign_accumulator_field_terms<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    domain: &[u8; 32],
    shape: &FoldShape,
    acc: Option<&Accumulator<C>>,
    copy_terms: Option<C::ScalarExt>,
) -> Result<AccumulatorFieldCells<C>, Error> {
    let limbs = acc.map(|a| {
        let [lo, hi] = s6_limbs(&a.digest(domain));
        [C::ScalarExt::from_u128(lo), C::ScalarExt::from_u128(hi)]
    });
    let digest = witnesses::<C>(lanes, region, limbs.as_ref().map(<[_; 2]>::as_slice), 2)?;
    let theta = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.instance.theta.as_slice()),
        shape.theta_len,
    )?;
    let r = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.instance.r.as_slice()),
        shape.groups.len(),
    )?;
    let beta = witnesses::<C>(
        lanes,
        region,
        acc.map(|a| a.beta.as_slice()),
        shape.index_bits,
    )?;
    let e = witnesses::<C>(lanes, region, acc.map(|a| core::slice::from_ref(&a.e)), 1)?.remove(0);
    let terms = witnesses::<C>(
        lanes,
        region,
        copy_terms.as_ref().map(core::slice::from_ref),
        1,
    )?
    .remove(0);
    Ok(AccumulatorFieldCells {
        digest: [digest[0].clone(), digest[1].clone()],
        public: Vec::new(),
        copy_terms: Some(terms),
        theta,
        r,
        beta,
        e,
    })
}

/// The shape of a batch: rounds, round degree, reads, claims.
fn shape<C: PastaCurve>(
    plans: &[&DeciderV2Plan<C>],
) -> Result<(usize, usize, usize, usize), Error> {
    let first = plans.first().ok_or(Error::Synthesis)?;
    if plans.iter().any(|plan| plan.mu != first.mu) {
        return Err(Error::Synthesis);
    }
    Ok((
        first.mu,
        plans.iter().map(|plan| plan.degree).max().unwrap_or(1),
        plans.iter().map(|plan| plan.reads.len()).sum(),
        batch_claims(plans).opened.len(),
    ))
}

/// Assigns the field messages of a decider v2 proof (`None`: key generation).
///
/// # Errors
/// Layout errors, or a proof of another shape.
pub fn assign_decider_v2_field_proof<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    plans: &[&DeciderV2Plan<C>],
    proof: Option<&DeciderV2Proof<C>>,
) -> Result<DeciderV2FieldProofCells<C>, Error> {
    let (mu, degree, reads, committed) = shape(plans)?;
    if let Some(p) = proof {
        if p.zero_check.len() != mu
            || p.zero_check.iter().any(|round| round.len() != degree)
            || p.at_rho.len() != reads
            || p.shift.len() != mu
            || p.at_sigma.len() != committed
        {
            return Err(Error::Synthesis);
        }
    }
    let zero_check = (0..mu)
        .map(|j| {
            witnesses::<C>(
                lanes,
                region,
                proof.map(|p| p.zero_check[j].as_slice()),
                degree,
            )
        })
        .collect::<Result<_, _>>()?;
    let at_rho = witnesses::<C>(lanes, region, proof.map(|p| p.at_rho.as_slice()), reads)?;
    let shift = (0..mu)
        .map(|j| {
            let round = witnesses::<C>(lanes, region, proof.map(|p| p.shift[j].as_slice()), 2)?;
            Ok([round[0].clone(), round[1].clone()])
        })
        .collect::<Result<_, Error>>()?;
    let at_sigma = witnesses::<C>(
        lanes,
        region,
        proof.map(|p| p.at_sigma.as_slice()),
        committed,
    )?;
    Ok(DeciderV2FieldProofCells {
        zero_check,
        at_rho,
        shift,
        at_sigma,
    })
}

/// Native arithmetic helpers over the field lanes.
struct Arith<'l, 'a, C: PastaCurve> {
    lanes: &'l mut FieldLanes<'a, C::ScalarExt>,
    one: W<C>,
    zero: W<C>,
}

impl<C: PastaCurve> Arith<'_, '_, C> {
    fn constant(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        value: C::ScalarExt,
    ) -> Result<W<C>, Error> {
        self.lanes.constant(region, value)
    }
    fn add(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        a: &W<C>,
        b: &W<C>,
    ) -> Result<W<C>, Error> {
        self.lanes.add(region, a, b)
    }
    fn sub(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        a: &W<C>,
        b: &W<C>,
    ) -> Result<W<C>, Error> {
        self.lanes.sub(region, a, b)
    }
    fn mul(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        a: &W<C>,
        b: &W<C>,
    ) -> Result<W<C>, Error> {
        self.lanes.mul(region, a, b)
    }
    fn scale(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        a: &W<C>,
        factor: C::ScalarExt,
    ) -> Result<W<C>, Error> {
        self.lanes.affine(region, a, factor, C::ScalarExt::ZERO)
    }
    fn complement(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        a: &W<C>,
    ) -> Result<W<C>, Error> {
        self.lanes
            .affine(region, a, -C::ScalarExt::ONE, C::ScalarExt::ONE)
    }
    fn inverse(&mut self, region: &mut Region<'_, C::ScalarExt>, a: &W<C>) -> Result<W<C>, Error> {
        let value = a.value().map(|v| v.invert().unwrap_or(C::ScalarExt::ZERO));
        let inverse = self.lanes.glue.witness(region, value)?;
        let product = self.mul(region, a, &inverse)?;
        let one = self.one.clone();
        FieldLanes::assert_equal(region, &product, &one)?;
        Ok(inverse)
    }

    /// The polynomial through `(t, values[t])`, `t = 0..values.len()`, at `x`.
    fn interpolate(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        values: &[W<C>],
        x: &W<C>,
    ) -> Result<W<C>, Error> {
        let m = values.len();
        let mut diffs = Vec::with_capacity(m);
        for j in 0..m {
            diffs.push(self.lanes.affine(
                region,
                x,
                C::ScalarExt::ONE,
                -C::ScalarExt::from(j as u64),
            )?);
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
        let mut total = self.zero.clone();
        for (i, value) in values.iter().enumerate() {
            let mut denominator = C::ScalarExt::ONE;
            for j in 0..m {
                if j != i {
                    denominator *= C::ScalarExt::from(i as u64) - C::ScalarExt::from(j as u64);
                }
            }
            let basis = self.mul(region, &prefix[i], &suffix[i])?;
            let weighted = self.mul(region, value, &basis)?;
            let term = self.scale(
                region,
                &weighted,
                denominator.invert().expect("distinct nodes"),
            )?;
            total = self.add(region, &total, &term)?;
        }
        Ok(total)
    }

    /// `sum_j eq(rho, j) eq(sigma, (j + k) mod 2^mu)` by the carry automaton of
    /// the constant addition (final carry dropped).
    fn shifted_eq(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        rho: &[W<C>],
        sigma: &[W<C>],
        rotation: i32,
    ) -> Result<W<C>, Error> {
        let size = 1i64 << rho.len();
        let offset = i64::from(rotation).rem_euclid(size);
        let mut no_carry = self.one.clone();
        let mut carry = self.zero.clone();
        for (bit, (x, y)) in rho.iter().zip(sigma).enumerate() {
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

    /// `eq(point, row)` for a constant row, given `1 - point`.
    fn eq_row(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        point: &[W<C>],
        complements: &[W<C>],
        row: usize,
    ) -> Result<W<C>, Error> {
        let mut acc = self.one.clone();
        for (j, (x, not_x)) in point.iter().zip(complements).enumerate() {
            let factor = if (row >> j) & 1 == 1 { x } else { not_x };
            acc = self.mul(region, &acc, factor)?;
        }
        Ok(acc)
    }

    /// `sum_{i < count} eq(point, i)`, the indicator of the first `count` rows
    /// ([`mle::prefix_indicator`]), given `1 - point`. Walking down from the
    /// top bit, a one bit of `count` adds `above (1 - x_j)` and keeps
    /// `above x_j`, a zero bit keeps `above (1 - x_j)`. The running pair after
    /// bit `j` depends only on `count >> j`, so `memo` shares it between counts
    /// (`None` stands for the empty sum and the empty product).
    fn prefix(
        &mut self,
        region: &mut Region<'_, C::ScalarExt>,
        (point, complements): (&[W<C>], &[W<C>]),
        count: usize,
        memo: &mut Prefixes<C>,
    ) -> Result<W<C>, Error> {
        if count >= 1 << point.len() {
            return Ok(self.one.clone());
        }
        let (mut sum, mut above): (Option<W<C>>, Option<W<C>>) = (None, None);
        for j in (0..point.len()).rev() {
            if let Some(state) = memo.get(&(j, count >> j)) {
                (sum, above) = state.clone();
                continue;
            }
            let low = match &above {
                Some(a) => self.mul(region, a, &complements[j])?,
                None => complements[j].clone(),
            };
            if (count >> j) & 1 == 1 {
                sum = Some(match &sum {
                    Some(s) => self.add(region, s, &low)?,
                    None => low,
                });
                above = Some(match &above {
                    Some(a) => self.mul(region, a, &point[j])?,
                    None => point[j].clone(),
                });
            } else {
                above = Some(low);
            }
            memo.insert((j, count >> j), (sum.clone(), above.clone()));
        }
        Ok(sum.unwrap_or_else(|| self.zero.clone()))
    }
}

/// Memoized prefix-indicator states: `(bit, count >> bit)` to the running sum
/// and product after that bit.
type Prefixes<C> = HashMap<(usize, usize), (Option<W<C>>, Option<W<C>>)>;

/// Verifies the field part of a decider v2 proof for every member `(plan,
/// accumulator field data)`, given the helper digest as limbs; returns
/// `sigma`, the claim values and `d_F`. The lane is clear afterwards.
///
/// # Errors
/// Layout errors, a proof or batch of another shape, or another transcript
/// live on the lane.
#[allow(clippy::too_many_lines)]
pub fn verify_decider_v2_field<C: PastaCurve>(
    lanes: &mut FieldLanes<'_, C::ScalarExt>,
    region: &mut Region<'_, C::ScalarExt>,
    domain: &[u8; 32],
    members: &[(&DeciderV2Plan<C>, &AccumulatorFieldCells<C>)],
    helper_digest: &[W<C>; 2],
    proof: &DeciderV2FieldProofCells<C>,
) -> Result<DeciderV2FieldOutput<C>, Error> {
    type S<C> = <C as PastaCurve>::ScalarExt;
    let plans: Vec<&DeciderV2Plan<C>> = members.iter().map(|m| m.0).collect();
    let (mu, degree, reads, committed) = shape(&plans)?;
    if proof.zero_check.len() != mu
        || proof.zero_check.iter().any(|round| round.len() != degree)
        || proof.at_rho.len() != reads
        || proof.shift.len() != mu
        || proof.at_sigma.len() != committed
    {
        return Err(Error::Synthesis);
    }
    // A member without public columns brings its copy term, and its relation
    // must read public values only through copies.
    if members.iter().any(|(plan, acc)| {
        acc.copy_terms.is_some() && (plan.reads_public_beyond_copies() || !acc.public.is_empty())
    }) {
        return Err(Error::Synthesis);
    }
    lanes.start(TAG_DECIDER_FIELD, domain)?;
    lanes.absorb_u64(members.len() as u64);
    for (_, acc) in members {
        lanes.absorb(&acc.digest);
    }
    lanes.absorb_u64(TAG_DECIDER_PERMUTATION);
    let gamma = lanes.challenge(region)?;
    let eta = lanes.challenge(region)?;
    lanes.absorb(helper_digest);
    let alpha = lanes.challenge(region)?;
    let kappa = lanes.challenge(region)?;
    let lambda = lanes.challenge(region)?;
    let tau = (0..mu)
        .map(|_| lanes.challenge(region))
        .collect::<Result<Vec<_>, _>>()?;
    let one = lanes.constant(region, S::<C>::ONE)?;
    let zero = lanes.constant(region, S::<C>::ZERO)?;
    let mut ar = Arith::<C> { lanes, one, zero };

    // Powers of alpha and kappa.
    let max_alpha = plans
        .iter()
        .map(|plan| plan.relation.groups.len() + 2)
        .max()
        .unwrap_or(2);
    let mut alpha_powers = vec![ar.one.clone()];
    for _ in 0..max_alpha {
        let next = ar.mul(region, alpha_powers.last().expect("seeded"), &alpha)?;
        alpha_powers.push(next);
    }
    let max_kappa = plans
        .iter()
        .map(|plan| plan.helper_count())
        .max()
        .unwrap_or(0);
    let mut kappa_powers = vec![ar.one.clone()];
    for _ in 0..max_kappa {
        let next = ar.mul(region, kappa_powers.last().expect("seeded"), &kappa)?;
        kappa_powers.push(next);
    }

    // Claimed sum: sum_r lambda^r (e_r - alpha^(G_r+2) I_r).
    let mut claim = ar.zero.clone();
    let mut scale = ar.one.clone();
    let mut scales = Vec::with_capacity(members.len());
    for (plan, acc) in members {
        let instance_terms = match &acc.copy_terms {
            Some(terms) => terms.clone(),
            None => {
                let mut sum = ar.zero.clone();
                for (column, row, id, sigma) in &plan.copies.instance_cells {
                    let value = acc
                        .public
                        .get(*column)
                        .and_then(|c| c.get(*row))
                        .cloned()
                        .unwrap_or_else(|| ar.zero.clone());
                    let base = ar.add(region, &gamma, &value)?;
                    let id_term = ar.scale(region, &eta, S::<C>::from(*id))?;
                    let sigma_term = ar.scale(region, &eta, S::<C>::from(*sigma))?;
                    let left = ar.add(region, &base, &id_term)?;
                    let right = ar.add(region, &base, &sigma_term)?;
                    let left = ar.inverse(region, &left)?;
                    let right = ar.inverse(region, &right)?;
                    let difference = ar.sub(region, &left, &right)?;
                    sum = ar.add(region, &sum, &difference)?;
                }
                sum
            }
        };
        let weighted = ar.mul(
            region,
            &alpha_powers[plan.relation.groups.len() + 2],
            &instance_terms,
        )?;
        let member = ar.sub(region, &acc.e, &weighted)?;
        let scaled = ar.mul(region, &scale, &member)?;
        claim = ar.add(region, &claim, &scaled)?;
        scales.push(scale.clone());
        scale = ar.mul(region, &scale, &lambda)?;
    }

    // Zero-check rounds.
    let mut rho = Vec::with_capacity(mu);
    for round in &proof.zero_check {
        ar.lanes.absorb(round);
        let challenge = ar.lanes.challenge(region)?;
        let s1 = ar.sub(region, &claim, &round[0])?;
        let mut values = vec![round[0].clone(), s1];
        values.extend(round[1..].iter().cloned());
        claim = ar.interpolate(region, &values, &challenge)?;
        rho.push(challenge);
    }

    // The final evaluation of every member's terms at rho. Prefix indicators
    // (usable rows, step keys) share their partial products.
    let complements = rho
        .iter()
        .map(|x| ar.complement(region, x))
        .collect::<Result<Vec<_>, _>>()?;
    let mut prefixes: Prefixes<C> = HashMap::new();
    let mut total = ar.zero.clone();
    let mut read_offset = 0;
    for ((plan, acc), scale) in members.iter().zip(&scales) {
        let relation = &plan.relation;
        let n = relation.n;
        let pb = relation.constraint_bits;
        let mut scalar_values: Vec<W<C>> = acc.theta.clone();
        scalar_values.extend(acc.r.iter().cloned());
        scalar_values.push(gamma.clone());
        scalar_values.push(eta.clone());
        for (column, row) in &plan.cells {
            scalar_values.push(
                acc.public
                    .get(*column)
                    .and_then(|c| c.get(*row))
                    .cloned()
                    .unwrap_or_else(|| ar.zero.clone()),
            );
        }
        let mut pow_lo = vec![ar.one.clone()];
        for b in &acc.beta[..pb] {
            for i in 0..pow_lo.len() {
                let next = ar.mul(region, &pow_lo[i], b)?;
                pow_lo.push(next);
            }
        }
        let mut read_of = vec![None; plan.sources.len()];
        for (i, read) in plan.reads.iter().enumerate() {
            read_of[read.source] = Some(read_offset + i);
        }
        let mut leaves: Vec<Option<W<C>>> = vec![None; plan.sources.len()];
        for (index, source) in plan.sources.iter().enumerate() {
            let value = if let Some(i) = read_of[index] {
                proof.at_rho[i].clone()
            } else {
                match *source {
                    Source::Instance(c, k) => {
                        let mut sum = ar.zero.clone();
                        for (row, cell) in acc.public[c].iter().enumerate() {
                            let weight =
                                ar.eq_row(region, &rho, &complements, mle::rotate(row, -k, n))?;
                            let term = ar.mul(region, cell, &weight)?;
                            sum = ar.add(region, &sum, &term)?;
                        }
                        sum
                    }
                    Source::Steps(..) => {
                        // first + sum delta (1 - P(row)) = (first + sum delta)
                        // - sum delta P(row), two changes per glue row.
                        let steps = plan.steps[index].as_ref().ok_or(Error::Synthesis)?;
                        let constant = steps
                            .changes
                            .iter()
                            .fold(steps.first, |sum, (_, delta)| sum + delta);
                        let mut value = ar.constant(region, constant)?;
                        for pair in steps.changes.chunks(2) {
                            let mut terms = Vec::with_capacity(2);
                            for (row, delta) in pair {
                                terms.push((
                                    -*delta,
                                    ar.prefix(region, (&rho, &complements), *row, &mut prefixes)?,
                                ));
                            }
                            let mut linear: Vec<(S<C>, &W<C>)> = vec![(S::<C>::ONE, &value)];
                            linear.extend(terms.iter().map(|(delta, prefix)| (*delta, prefix)));
                            value = ar.lanes.glue.linear(region, &linear, S::<C>::ZERO)?;
                        }
                        value
                    }
                    Source::Pow => {
                        let mut acc_pow = ar.one.clone();
                        for ((x, not_x), b) in rho.iter().zip(&complements).zip(&acc.beta[pb..]) {
                            let xb = ar.mul(region, x, b)?;
                            let factor = ar.add(region, not_x, &xb)?;
                            acc_pow = ar.mul(region, &acc_pow, &factor)?;
                        }
                        acc_pow
                    }
                    Source::EqTau => {
                        let mut acc_eq = ar.one.clone();
                        for (x, y) in tau.iter().zip(&rho) {
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
                        ar.prefix(region, (&rho, &complements), relation.usable, &mut prefixes)?
                    }
                    Source::Index => {
                        let mut sum = ar.zero.clone();
                        let mut power = S::<C>::ONE;
                        for x in &rho {
                            let term = ar.scale(region, x, power)?;
                            sum = ar.add(region, &sum, &term)?;
                            power = power.double();
                        }
                        sum
                    }
                    Source::Witness(..) | Source::Key(..) | Source::Helper(_) => {
                        return Err(Error::Synthesis);
                    }
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
            let mut stack: Vec<W<C>> = Vec::new();
            for op in term.poly.ops() {
                match op {
                    Op::Const(value) => stack.push(ar.constant(region, *value)?),
                    Op::Table(index) => {
                        stack.push(leaves[*index as usize].clone().ok_or(Error::Synthesis)?)
                    }
                    Op::Scalar(index) => stack.push(scalar_values[*index as usize].clone()),
                    Op::Neg => {
                        let v = stack.pop().ok_or(Error::Synthesis)?;
                        stack.push(ar.scale(region, &v, -S::<C>::ONE)?);
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
    FieldLanes::assert_equal(region, &total, &claim)?;

    // Shift reduction.
    ar.lanes.absorb(&proof.at_rho);
    ar.lanes.absorb_u64(TAG_DECIDER_SHIFT);
    let zeta = ar.lanes.challenge(region)?;
    let mut claim = ar.zero.clone();
    let mut power = ar.one.clone();
    let mut zeta_powers = Vec::with_capacity(reads);
    for value in &proof.at_rho {
        zeta_powers.push(power.clone());
        let term = ar.mul(region, &power, value)?;
        claim = ar.add(region, &claim, &term)?;
        power = ar.mul(region, &power, &zeta)?;
    }
    let mut sigma = Vec::with_capacity(mu);
    for round in &proof.shift {
        ar.lanes.absorb(round);
        let challenge = ar.lanes.challenge(region)?;
        let s1 = ar.sub(region, &claim, &round[0])?;
        let values = [round[0].clone(), s1, round[1].clone()];
        claim = ar.interpolate(region, &values, &challenge)?;
        sigma.push(challenge);
    }
    let mut by_rotation: Vec<(i32, W<C>)> = Vec::new();
    let mut expected = ar.zero.clone();
    let mut read_index = 0;
    let batch = batch_claims(&plans);
    for (plan, claims) in plans.iter().zip(&batch.of) {
        for read in &plan.reads {
            let weight = match by_rotation.iter().position(|(k, _)| *k == read.rotation) {
                Some(slot) => by_rotation[slot].1.clone(),
                None => {
                    let w = ar.shifted_eq(region, &rho, &sigma, read.rotation)?;
                    by_rotation.push((read.rotation, w.clone()));
                    w
                }
            };
            let weighted = ar.mul(region, &zeta_powers[read_index], &weight)?;
            let term = ar.mul(region, &weighted, &proof.at_sigma[claims[read.poly]])?;
            expected = ar.add(region, &expected, &term)?;
            read_index += 1;
        }
    }
    FieldLanes::assert_equal(region, &expected, &claim)?;
    ar.lanes.absorb(&proof.at_sigma);
    let field_digest = ar.lanes.finish(region)?;
    Ok(DeciderV2FieldOutput {
        copy_challenges: [gamma, eta],
        point: sigma,
        at_sigma: proof.at_sigma.clone(),
        field_digest,
    })
}

/// The batched opening's field scalars: `v* = sum_c alpha^c v_c` (Horner) and
/// `b' = folded_eq(sigma, u) = prod_j ((1 - sigma_{k-1-j}) + u_j sigma_{k-1-j})`,
/// from the opening's `alpha` and `u` imported as limbs (a single claim takes
/// no `alpha`).
///
/// # Errors
/// Layout errors, or `u` of another length than `sigma`.
pub fn opening_scalars<F: PastaField + PoseidonField>(
    lanes: &mut FieldLanes<'_, F>,
    region: &mut Region<'_, F>,
    values: &[Word<F>],
    sigma: &[Word<F>],
    alpha: &Word<F>,
    u: &[Word<F>],
) -> Result<(Word<F>, Word<F>), Error> {
    if u.len() != sigma.len() || values.is_empty() {
        return Err(Error::Synthesis);
    }
    let (last, rest) = values.split_last().ok_or(Error::Synthesis)?;
    let mut value = last.clone();
    for v in rest.iter().rev() {
        let scaled = lanes.mul(region, &value, alpha)?;
        value = lanes.add(region, &scaled, v)?;
    }
    let mut folded = lanes.constant(region, F::ONE)?;
    for (x, uj) in sigma.iter().rev().zip(u) {
        let ux = lanes.mul(region, uj, x)?;
        let not_x = lanes.affine(region, x, -F::ONE, F::ONE)?;
        let factor = lanes.add(region, &not_x, &ux)?;
        folded = lanes.mul(region, &folded, &factor)?;
    }
    Ok((value, folded))
}

/// The copy helpers' digest on the commitment side, as
/// [`crate::decider_v2::helper_digest`] (a complete transcript on a clear lane).
///
/// # Errors
/// Layout errors, or another transcript live on the lane.
pub fn helper_digest<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    helpers: &[AssignedPoint<C::Base>],
) -> Result<Word<C::Base>, Error> {
    t::start(lanes, TAG_DECIDER_HELPERS, domain)?;
    t::absorb_u64(lanes, helpers.len() as u64);
    for point in helpers {
        t::absorb_point(lanes, point);
    }
    t::finish(lanes, region)
}

/// Starts the opening transcript on the commitment side from the field
/// transcript's digest `d_F` (imported as a scalar); the lane stays live for
/// the batched opening's group part.
///
/// # Errors
/// Layout errors, or another transcript live on the lane.
pub fn opening_transcript<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    domain: &[u8; 32],
    field_digest: &ScalarCell<C>,
) -> Result<(), Error> {
    t::start(lanes, TAG_DECIDER_OPENING, domain)?;
    t::absorb_scalar(lanes, region, field_digest)
}

/// The commitments of a batch's claims on the commitment side, in claim order,
/// as [`crate::decider_v2::verify_many`] lists them: per member its committed
/// polynomials, except key polynomials an earlier member already opens.
/// Witness columns come from the accumulator's commitments, key polynomials
/// are circuit constants, copy helpers come from `helpers` (all members'
/// helpers in member order, [`DeciderV2Plan::helper_count`] each).
///
/// # Errors
/// Layout errors, or helpers or accumulators of another shape.
pub fn claim_commitments<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    members: &[(&DeciderV2Plan<C>, &AccumulatorCells<C>)],
    helpers: &[AssignedPoint<C::Base>],
) -> Result<Vec<AssignedPoint<C::Base>>, Error> {
    if helpers.len()
        != members
            .iter()
            .map(|(plan, _)| plan.helper_count())
            .sum::<usize>()
    {
        return Err(Error::Synthesis);
    }
    let plans: Vec<&DeciderV2Plan<C>> = members.iter().map(|(plan, _)| *plan).collect();
    let offsets = helper_offsets(&plans);
    let mut commitments = Vec::new();
    for (member, index) in batch_claims(&plans).opened {
        let (plan, acc) = members[member];
        commitments.push(match plan.committed[index] {
            Committed::Witness(c) => acc
                .instance
                .commitments
                .get(c)
                .cloned()
                .ok_or(Error::Synthesis)?,
            Committed::Key(p) => lanes
                .ecc
                .constant_point(region, &C::from(plan.key_commitments[p]))?,
            Committed::Helper(j) => helpers[offsets[member] + j].clone(),
        });
    }
    Ok(commitments)
}
