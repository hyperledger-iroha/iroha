//! The pieces of the Fp wrap P (module documentation of [`super`]).
//!
//! Every function lays out one piece on lanes P already owns and returns the
//! cells of its crossing values; the caller binds each to its instance row.

use ff::PrimeField;
use iroha_fold::{
    Accumulator,
    circuit::{
        FoldLanes, ScalarCell,
        decider_v2::{
            assign_accumulator_field, assign_decider_v2_field_proof, claim_commitments,
            helper_digest, opening_scalars, opening_transcript, verify_decider_v2_field,
        },
        field::FieldLanes,
        mle::{
            EndoAlpha, OpeningClaimCells, OpeningKey, assign_batch_opening,
            verify_batch_bound_group,
        },
        transcript as t,
        verify::{accumulator_digest, assign_accumulator},
    },
    decider_v2::{DeciderV2Plan, DeciderV2Proof},
    relation::FoldShape,
};
use iroha_pasta::{Ep, EpAffine, Eq, Fp, Fq};
use iroha_plonk::{
    frontend::{Error, Region, Value},
    pcs::mle::BatchOpening,
};
use iroha_plonk_gadgets::{Word, ff::CanonicalS6};

/// Canonical S6 limbs `(lo, hi)` of a value crossing between the wraps.
pub type Limbs = [Word<Fp>; 2];

/// The A-side decider's crossing values in P.
#[derive(Clone, Debug)]
pub struct AFieldCrossing {
    /// `kgwfacd1` of the A accumulator (Omega computes it; imported).
    pub acc_digest: Limbs,
    /// `D_H` of the A decider's helpers (Omega computes it; imported).
    pub helper_digest: Limbs,
    /// The A opening's batching challenge (Omega's `T_G`; imported).
    pub alpha: Limbs,
    /// The A opening's round challenges (Omega's `T_G`; imported).
    pub rounds: Vec<Limbs>,
    /// The field transcript's digest `d_F` (exported to Omega's `T_G`).
    pub field_digest: Limbs,
    /// `v* = sum_c alpha^c v_c` (exported).
    pub value: Limbs,
    /// `b' = folded_eq(sigma, u)` (exported).
    pub folded: Limbs,
}

/// The imported values' witnesses, as native integers (`< 2^255`).
#[derive(Clone, Debug)]
pub struct AFieldImports {
    /// `kgwfacd1` limbs.
    pub acc_digest: [u128; 2],
    /// `D_H` limbs.
    pub helper_digest: [u128; 2],
    /// `alpha` limbs.
    pub alpha: [u128; 2],
    /// `u_j` limbs.
    pub rounds: Vec<[u128; 2]>,
}

fn limbs_value(value: Option<[u128; 2]>, index: usize) -> Value<Fp> {
    value.map_or_else(Value::unknown, |v| Value::known(Fp::from_u128(v[index])))
}

/// Assigns imported limbs and recomposes them into a native word.
fn import(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    value: Option<[u128; 2]>,
) -> Result<(Limbs, Word<Fp>), Error> {
    let lo = lanes.glue.witness(region, limbs_value(value, 0))?;
    let hi = lanes.glue.witness(region, limbs_value(value, 1))?;
    let word = lanes.recompose(region, &lo, &hi)?;
    Ok(([lo, hi], word))
}

/// Exports a native word as its canonical limbs.
fn export(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    word: &Word<Fp>,
) -> Result<Limbs, Error> {
    let limbs = lanes.limbs(region, word)?;
    Ok([limbs.lo().word().clone(), limbs.hi().word().clone()])
}

/// The A-side decider's field side (native `Fp`): the accumulator's field
/// data bound to its imported digest, the field transcript to `d_F`, and the
/// opening's `v*` and `b'` for the imported `alpha` and `u` (`IVC.md`
/// section 11). Omega's commitment side computes the imports and consumes the
/// exports; P's instance carries both.
///
/// # Errors
/// Layout errors; a false decider proof is unsatisfiable.
#[allow(clippy::too_many_arguments)]
pub fn a_decider_field_side(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    plan: &DeciderV2Plan<Eq>,
    shape: &FoldShape,
    acc: Option<&Accumulator<Eq>>,
    proof: Option<&DeciderV2Proof<Eq>>,
    imports: Option<&AFieldImports>,
    rounds: usize,
) -> Result<AFieldCrossing, Error> {
    let acc_cells = assign_accumulator_field::<Eq>(lanes, region, domain, shape, acc)?;
    let proof_cells = assign_decider_v2_field_proof(lanes, region, &[plan], proof)?;
    // The accumulator digest limbs the field side absorbs are the imported ones.
    let (acc_digest, _) = import(lanes, region, imports.map(|i| i.acc_digest))?;
    for (imported, assigned) in acc_digest.iter().zip(&acc_cells.digest) {
        region.constrain_equal(imported.cell(), assigned.cell())?;
    }
    let (helper_digest, _) = import(lanes, region, imports.map(|i| i.helper_digest))?;
    let output = verify_decider_v2_field(
        lanes,
        region,
        domain,
        &[(plan, &acc_cells)],
        &helper_digest,
        &proof_cells,
    )?;
    let (alpha, alpha_word) = import(lanes, region, imports.map(|i| i.alpha))?;
    let mut round_limbs = Vec::with_capacity(rounds);
    let mut round_words = Vec::with_capacity(rounds);
    for j in 0..rounds {
        let (limbs, word) = import(lanes, region, imports.map(|i| i.rounds[j]))?;
        round_limbs.push(limbs);
        round_words.push(word);
    }
    let (value, folded) = opening_scalars(
        lanes,
        region,
        &output.at_sigma,
        &output.point,
        &alpha_word,
        &round_words,
    )?;
    let field_digest = export(lanes, region, &output.field_digest)?;
    let value = export(lanes, region, &value)?;
    let folded = export(lanes, region, &folded)?;
    Ok(AFieldCrossing {
        acc_digest,
        helper_digest,
        alpha,
        rounds: round_limbs,
        field_digest,
        value,
        folded,
    })
}

/// The Q-side decider's crossing values and claim in P.
#[derive(Clone, Debug)]
pub struct QGroupCrossing {
    /// `kgwfacd1` of the Q accumulator (exported to Omega's field side).
    pub acc_digest: Limbs,
    /// `D_H` of the Q decider's helpers (exported).
    pub helper_digest: Limbs,
    /// The field transcript's digest `d_F` (Omega's field side; imported).
    pub field_digest: Limbs,
    /// `v*` (Omega's field side; imported).
    pub value: Limbs,
    /// `b'` (Omega's field side; imported).
    pub folded: Limbs,
    /// The opening's batching challenge (exported).
    pub alpha: Limbs,
    /// The opening's round challenges (exported).
    pub rounds: Vec<Limbs>,
    /// The opening's undecided Pallas claim `(G'_0, u)` for `acc_P`.
    pub claim: OpeningClaimCells<Ep>,
}

/// The Q-side decider's witness in P: the accumulator, the decider's helper
/// commitments and its bound opening, and the imported field-side values.
#[derive(Clone, Debug)]
pub struct QGroupWitness {
    /// The decided Q accumulator.
    pub acc: Accumulator<Ep>,
    /// `proof.helpers`.
    pub helpers: Vec<EpAffine>,
    /// The bound opening.
    pub opening: BatchOpening<Ep>,
    /// `d_F`, `v*`, `b'` (scalars of the Q relation, `Fq`).
    pub imports: [Fq; 3],
}

fn scalar_limbs(
    lanes: &mut FoldLanes<'_, Ep>,
    region: &mut Region<'_, Fp>,
    value: Value<Fq>,
) -> Result<(ScalarCell<Ep>, Limbs), Error> {
    let cell = lanes.assign_scalar(region, value)?;
    let limbs = [cell.s6.lo().word().clone(), cell.s6.hi().word().clone()];
    Ok((cell, limbs))
}

fn word_limbs(
    lanes: &mut FoldLanes<'_, Ep>,
    region: &mut Region<'_, Fp>,
    word: &Word<Fp>,
) -> Result<Limbs, Error> {
    let limbs = CanonicalS6::from_native_word(&mut lanes.uint(), region, word)?;
    Ok([limbs.lo().word().clone(), limbs.hi().word().clone()])
}

/// The Q-side decider's commitment side (Pallas, native in `Fp`): the
/// accumulator digest and helper digest the field side absorbs, the opening
/// transcript `T_G` seeded with the imported `d_F`, the claim commitments in
/// the decider's claim order, and the bound opening's group side with the
/// imported `v*` and `b'` (`IVC.md` section 11). The lane is cleared after.
///
/// # Errors
/// Layout errors; a false opening is unsatisfiable.
pub fn q_decider_commitment_side(
    lanes: &mut FoldLanes<'_, Ep>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    plan: &DeciderV2Plan<Ep>,
    shape: &FoldShape,
    key: &OpeningKey<Ep>,
    witness: Option<&QGroupWitness>,
) -> Result<QGroupCrossing, Error> {
    let acc = assign_accumulator(lanes, region, shape, witness.map(|w| &w.acc))?;
    let acc_digest = accumulator_digest(lanes, region, domain, &acc)?;
    let mut helpers = Vec::with_capacity(plan.helper_count());
    for index in 0..plan.helper_count() {
        let value =
            witness.map_or_else(Value::unknown, |w| Value::known(Ep::from(w.helpers[index])));
        helpers.push(lanes.assign_point(region, value)?);
    }
    let helper = helper_digest(lanes, region, domain, &helpers)?;
    let import =
        |index: usize| witness.map_or_else(Value::unknown, |w| Value::known(w.imports[index]));
    let (field_digest, field_digest_limbs) = scalar_limbs(lanes, region, import(0))?;
    let (value, value_limbs) = scalar_limbs(lanes, region, import(1))?;
    let (folded, folded_limbs) = scalar_limbs(lanes, region, import(2))?;
    let opening = assign_batch_opening(lanes, region, plan.mu(), witness.map(|w| &w.opening))?;
    let commitments = claim_commitments(lanes, region, &[(plan, &acc)], &helpers)?;
    opening_transcript(lanes, region, domain, &field_digest)?;
    let group = verify_batch_bound_group(
        lanes,
        region,
        key,
        plan.mu(),
        &commitments,
        &value,
        &folded,
        &opening,
    )?;
    t::finish(lanes, region)?;
    let EndoAlpha { scalar: alpha, .. } = group.alpha.ok_or(Error::Synthesis)?;
    let rounds = group
        .claim
        .challenges
        .iter()
        .map(|u| [u.s6.lo().word().clone(), u.s6.hi().word().clone()])
        .collect();
    Ok(QGroupCrossing {
        acc_digest: word_limbs(lanes, region, &acc_digest)?,
        helper_digest: word_limbs(lanes, region, &helper)?,
        field_digest: field_digest_limbs,
        value: value_limbs,
        folded: folded_limbs,
        alpha: [alpha.s6.lo().word().clone(), alpha.s6.hi().word().clone()],
        rounds,
        claim: group.claim,
    })
}
