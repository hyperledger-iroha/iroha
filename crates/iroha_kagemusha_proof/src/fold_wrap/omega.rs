//! The Fq wrap Omega (module documentation of [`super`]).
//!
//! Omega verifies P hard under P's pinned key, reads P's public column as
//! canonical `Fp` scalars and binds every crossing value to its own side:
//! values P computed enter Omega's arithmetic from P's rows, and values Omega
//! computes are constrained to P's rows (canonical limbs, exactly). Its own
//! public columns are today's `[D_A]`, `acc_V.G`, `acc_V.u`.

use std::sync::Arc;

use ff::Field;
use iroha_fold::{
    Accumulator, Instance,
    circuit::{
        FoldLanes, ScalarCell,
        claims::{
            ClaimAccumulatorCells, assign_claim_accumulator, assign_claim_accumulator_field,
            assign_restructure_field, claim_accumulator_digest, verify_restructure_field,
        },
        decider_v2::{
            assign_accumulator_field, assign_decider_v2_field_proof, claim_commitments,
            helper_digest, opening_scalars, opening_transcript, verify_decider_v2_field,
        },
        field::FieldLanes,
        ivc::ivc_hash,
        mle::{
            EndoAlpha, OpeningClaimCells, OpeningKey, assign_batch_opening,
            verify_batch_bound_group,
        },
        transcript as t,
        verify::{
            InstanceCells, accumulator_digest, assign_accumulator, assign_proof,
            verify_fold_or_start,
        },
    },
    claims::{ClaimAccumulator, RestructureProof},
    decider_v2::{DeciderV2Plan, DeciderV2Proof},
    fold::FoldProof,
    ivc::placeholder_accumulator,
};
use iroha_pasta::{Ep, Eq, EqAffine, Fp, Fq};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::{Column, ConstraintSystem, Instance as InstanceColumn, InstanceType},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::mle::BatchOpening,
};
use iroha_plonk_gadgets::{
    GlueChip, Word,
    bytes::tape::{BytesChip, BytesConfig},
    ff::CanonicalS6,
};
use iroha_plonk_recursion::{
    FOLD_WITNESS_BYTES, FoldPurpose, FoldWitness, K,
    accumulation_circuit::{FoldInputCells, FoldPlan},
    codec::ScalarCells,
    restructure_circuit::{RestructurePlan, RestructureStatementCells},
    verifier::{VerificationMode, VerifierChip, VerifierConfig, VerifierPlan},
};

use super::{
    K_U32, WrapError, WrapSpec,
    crossing::{PPublic, Pair, PallasClaimCrossing, QCrossing},
    p::PPlan,
};

/// Omega's public column types (today's Omega): `D_A`, `acc_V.G`, `acc_V.u`.
pub const INSTANCE_TYPES: [InstanceType; 3] = [
    InstanceType::Bounded,
    InstanceType::Field,
    InstanceType::Bounded,
];
/// Omega's public column lengths.
pub const INSTANCE_LENGTHS: [usize; 3] = [1, 2, K];
/// Range buses of Omega's verifier lanes.
pub const OMEGA_RANGE_BUSES: usize = 2;

/// Omega's fixed program: P's program and pinned key, the A opening's
/// generators, the Vesta claim restructuring's points and `acc_V`'s fold.
#[derive(Clone, Debug)]
pub struct OmegaPlan {
    /// P's program (shapes, domains, crossing layout).
    pub p: Arc<PPlan>,
    /// P's verifier program.
    pub verifier: VerifierPlan<Eq>,
    /// P's key, a circuit constant.
    pub key: VerifyingKey<Eq>,
    /// The A opening's fixed generators.
    pub opening: OpeningKey<Eq>,
    /// The Vesta claim restructuring's pinned points and context.
    pub restructure: RestructurePlan<Eq>,
    /// `acc_V`'s fold: P's claim, the A decider's claim, the restructured claim.
    pub fold: FoldPlan<Eq>,
}

impl OmegaPlan {
    /// Omega's program over P's program and key.
    ///
    /// # Errors
    /// [`WrapError::Input`] for a key of another descriptor or parameters
    /// below k16.
    pub fn new(
        p: Arc<PPlan>,
        binding: DescriptorBinding,
        key: VerifyingKey<Eq>,
    ) -> Result<Self, WrapError> {
        let params = p.spec.params_a.clone();
        if key.descriptor_digest() != binding.digest() {
            return Err(WrapError::Input);
        }
        Ok(Self {
            opening: OpeningKey::new(params.params()),
            restructure: RestructurePlan::new(&params).map_err(|_| WrapError::Input)?,
            fold: FoldPlan::new(&params, vec![K_U32; 3], FoldPurpose::FoldWrapVesta)
                .map_err(|_| WrapError::Input)?,
            verifier: VerifierPlan::new(binding, params).map_err(|_| WrapError::Input)?,
            key,
            p,
        })
    }
}

/// The Q-side decider's field side in Omega: every Q kind's field data and
/// the batch proof.
#[derive(Clone, Debug)]
pub struct QFieldWitness {
    /// The decided Q accumulators, in kind order.
    pub accs: Vec<Accumulator<Ep>>,
    /// The batch proof.
    pub proof: DeciderV2Proof<Ep>,
}

/// The Pallas claims' restructuring sumcheck in Omega.
#[derive(Clone, Debug)]
pub struct PallasFieldWitness {
    /// The claim accumulator A keeps.
    pub acc: ClaimAccumulator<Ep>,
    /// The sumcheck's messages.
    pub proof: RestructureProof<Fq>,
}

/// The last A fold and the A-side decider's commitment side in Omega.
#[derive(Clone, Debug)]
pub struct AGroupWitness {
    /// The A accumulators `Q_{n-1}` keeps, in kind order; only the last
    /// kind's may be absent (`A_n` then starts it).
    pub accs: Vec<Option<Accumulator<Eq>>>,
    /// `A_n`'s instance (its public columns are P's rows).
    pub fresh: Instance<Eq>,
    /// The fold's messages (zero when `A_n` starts its accumulator).
    pub fold: FoldProof<Fp>,
    /// The decider proof's helper commitments.
    pub helpers: Vec<EqAffine>,
    /// The decider's bound opening.
    pub opening: BatchOpening<Eq>,
}

/// Every input of Omega besides its plan; private to the wallet.
pub struct OmegaWitness {
    /// `Q_{n-1}`'s step index.
    pub step: u64,
    /// `z_Q`, the Q state after `Q_{n-1}`.
    pub state: Vec<Fq>,
    /// P's public column.
    pub p_public: Vec<Fp>,
    /// P's proof.
    pub p_proof: Vec<u8>,
    /// The Q-side decider's field side.
    pub q: QFieldWitness,
    /// The Pallas claims' sumcheck.
    pub pallas: PallasFieldWitness,
    /// The last A fold and the A-side decider's commitment side.
    pub a: AGroupWitness,
    /// The claim accumulator Q keeps.
    pub vesta: ClaimAccumulator<Eq>,
    /// The Vesta claim accumulator's committed-mask opening.
    pub restructure: FoldWitness<Eq>,
    /// `acc_V`'s fold witness.
    pub fold: FoldWitness<Eq>,
}

impl core::fmt::Debug for OmegaWitness {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("OmegaWitness { private: [REDACTED] }")
    }
}

/// The Fq wrap Omega.
#[derive(Clone, Debug)]
pub struct OmegaCircuit {
    /// The program.
    pub plan: Arc<OmegaPlan>,
    /// The witness (`None`: key generation).
    pub witness: Option<Arc<OmegaWitness>>,
}

/// Omega's columns: the shared verifier lanes, a byte tape and today's three
/// public columns.
#[derive(Clone, Debug)]
pub struct OmegaConfig {
    verifier: VerifierConfig<Eq>,
    bytes: BytesConfig,
    public: [Column<InstanceColumn>; 3],
}

fn known<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// The integer of a row below `2^128` (a limb P range-checked or Omega
/// forces): its low limb, with the high limb zero.
fn limb(
    glue: &mut GlueChip<Fq>,
    region: &mut Region<'_, Fq>,
    row: &ScalarCells<Eq>,
) -> Result<Word<Fq>, Error> {
    glue.enforce_constant(region, row.hi().word(), Fq::ZERO)?;
    Ok(row.lo().word().clone())
}

/// The `Fq` value of a pair of canonical limbs P computed.
fn pair_word(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    pair: &Pair<ScalarCells<Eq>>,
) -> Result<Word<Fq>, Error> {
    let lo = limb(lanes.glue, region, &pair[0])?;
    let hi = limb(lanes.glue, region, &pair[1])?;
    lanes.recompose(region, &lo, &hi)
}

/// Constrains `word` to the value of P's canonical pair.
fn bind_pair(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    word: &Word<Fq>,
    pair: &Pair<ScalarCells<Eq>>,
) -> Result<(), Error> {
    let value = pair_word(lanes, region, pair)?;
    FieldLanes::assert_equal(region, word, &value)
}

/// Constrains a field side's digest limbs to P's `Fp` row.
fn bind_digest_limbs(
    region: &mut Region<'_, Fq>,
    limbs: &[Word<Fq>; 2],
    row: &ScalarCells<Eq>,
) -> Result<(), Error> {
    GlueChip::assert_equal(region, &limbs[0], row.lo().word())?;
    GlueChip::assert_equal(region, &limbs[1], row.hi().word())
}

/// Constrains a foreign `Fp` scalar Omega computed to P's row: both are
/// canonical, so the limbs agree.
fn bind_scalar(
    region: &mut Region<'_, Fq>,
    cell: &ScalarCell<Eq>,
    row: &ScalarCells<Eq>,
) -> Result<(), Error> {
    GlueChip::assert_equal(region, cell.s6.lo().word(), row.lo().word())?;
    GlueChip::assert_equal(region, cell.s6.hi().word(), row.hi().word())
}

/// Constrains P's pair to the canonical limbs of the `Fq` value `word` Omega
/// computed (P imported the pair unchecked; this fixes it exactly).
fn bind_canonical(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    word: &Word<Fq>,
    pair: &Pair<ScalarCells<Eq>>,
) -> Result<(), Error> {
    let limbs = CanonicalS6::from_native_word(&mut lanes.uint(), region, word)?;
    let lo = limb(lanes.glue, region, &pair[0])?;
    let hi = limb(lanes.glue, region, &pair[1])?;
    GlueChip::assert_equal(region, limbs.lo().word(), &lo)?;
    GlueChip::assert_equal(region, limbs.hi().word(), &hi)
}

/// A P row as a foreign `Fp` scalar on Omega's fold lanes.
fn import_row(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    row: &ScalarCells<Eq>,
) -> Result<ScalarCell<Eq>, Error> {
    let s6 = CanonicalS6::from_limbs(
        &mut lanes.uint(),
        region,
        CanonicalS6::<Fq>::field_modulus::<Fp>(),
        row.lo(),
        row.hi(),
    )?;
    lanes.import(region, s6)
}

/// The Q-side decider's field side over every Q kind (native `Fq`): each
/// member's field data and digest from P's rows, the field transcript, and
/// `v*`, `b'`, `d_F` constrained to P's rows.
///
/// # Errors
/// Layout errors or a witness of another shape; a false proof is
/// unsatisfiable.
pub fn q_field_side(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    plan: &PPlan,
    crossing: &QCrossing<ScalarCells<Eq>>,
    witness: Option<&QFieldWitness>,
) -> Result<(), Error> {
    let spec = &*plan.spec;
    let domain = &spec.domains.q_fold;
    let plans: Vec<&DeciderV2Plan<Ep>> = plan.q_plans.iter().map(|p| &**p).collect();
    if crossing.members.len() != plans.len() || witness.is_some_and(|w| w.accs.len() != plans.len())
    {
        return Err(Error::Synthesis);
    }
    let mut members = Vec::with_capacity(plans.len());
    for (index, (relation, member)) in spec.q.iter().zip(&crossing.members).enumerate() {
        let acc = assign_accumulator_field::<Ep>(
            lanes,
            region,
            domain,
            &relation.shape(),
            witness.map(|w| &w.accs[index]),
        )?;
        bind_digest_limbs(region, &acc.digest, &member.acc_digest)?;
        let data = &member.data;
        let public: Vec<&Word<Fq>> = acc.public.iter().flatten().collect();
        if public.len() != data.public.len()
            || acc.theta.len() != data.theta.len()
            || acc.r.len() != data.r.len()
            || acc.beta.len() != data.beta.len()
        {
            return Err(Error::Synthesis);
        }
        for (word, pair) in public.into_iter().zip(&data.public) {
            bind_pair(lanes, region, word, pair)?;
        }
        for (words, pairs) in [
            (&acc.theta, &data.theta),
            (&acc.r, &data.r),
            (&acc.beta, &data.beta),
        ] {
            for (word, pair) in words.iter().zip(pairs) {
                bind_pair(lanes, region, word, pair)?;
            }
        }
        bind_pair(lanes, region, &acc.e, &data.e)?;
        members.push(acc);
    }
    let proof = assign_decider_v2_field_proof(lanes, region, &plans, witness.map(|w| &w.proof))?;
    let helper = [
        crossing.helper_digest.lo().word().clone(),
        crossing.helper_digest.hi().word().clone(),
    ];
    let batch: Vec<_> = plans.iter().copied().zip(members.iter()).collect();
    let output = verify_decider_v2_field(lanes, region, domain, &batch, &helper, &proof)?;
    let alpha = pair_word(lanes, region, &crossing.alpha)?;
    let rounds = crossing
        .rounds
        .iter()
        .map(|pair| pair_word(lanes, region, pair))
        .collect::<Result<Vec<_>, _>>()?;
    let (value, folded) = opening_scalars(
        lanes,
        region,
        &output.at_sigma,
        &output.point,
        &alpha,
        &rounds,
    )?;
    bind_pair(lanes, region, &output.field_digest, &crossing.field_digest)?;
    bind_pair(lanes, region, &value, &crossing.value)?;
    bind_pair(lanes, region, &folded, &crossing.folded)
}

/// The Pallas claims' restructuring sumcheck (native `Fq`): the claim
/// accumulator's field data and digest from P's rows, `rho` and `v`
/// constrained to P's rows.
///
/// # Errors
/// Layout errors; a false sumcheck is unsatisfiable.
pub fn pallas_field_side(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    domain: &[u8; 32],
    crossing: &PallasClaimCrossing<ScalarCells<Eq>>,
    witness: Option<&PallasFieldWitness>,
) -> Result<(), Error> {
    let acc = assign_claim_accumulator_field::<Ep>(lanes, region, domain, witness.map(|w| &w.acc))?;
    bind_digest_limbs(region, &acc.digest, &crossing.digest)?;
    for (words, pairs) in [(&acc.u, &crossing.u), (&acc.beta, &crossing.beta)] {
        if words.len() != pairs.len() {
            return Err(Error::Synthesis);
        }
        for (word, pair) in words.iter().zip(pairs) {
            bind_pair(lanes, region, word, pair)?;
        }
    }
    bind_pair(lanes, region, &acc.e, &crossing.e)?;
    let proof = assign_restructure_field::<Ep>(lanes, region, witness.map(|w| &w.proof))?;
    let (rho, value) = verify_restructure_field(lanes, region, domain, &acc, &proof)?;
    for (word, pair) in rho.iter().zip(&crossing.rho) {
        bind_pair(lanes, region, word, pair)?;
    }
    bind_pair(lanes, region, &value, &crossing.value)
}

/// The last A fold: `A_n` (public columns from P's rows) folds into its
/// kind's accumulator, or starts it when `Q_{n-1}` kept none of that kind.
/// `A_n`'s passthrough is recomputed as `Q_{n-1}`'s `h_out` over every kept A
/// digest (zero for the absent kind) and the Vesta claim accumulator's. The
/// A-side decider's commitment side and opening then run over every A kind
/// after the fold. Returns the opening's undecided Vesta claim and the claim
/// accumulator.
///
/// # Errors
/// Layout errors or a witness of another shape; a false fold or opening is
/// unsatisfiable.
#[allow(clippy::too_many_lines)]
pub fn a_commitment_side(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    plan: &OmegaPlan,
    public: &PPublic<ScalarCells<Eq>>,
    witness: Option<&OmegaWitness>,
) -> Result<(OpeningClaimCells<Eq>, ClaimAccumulatorCells<Eq>), Error> {
    let spec: &WrapSpec = &plan.p.spec;
    let plans: Vec<&DeciderV2Plan<Eq>> = plan.p.a_plans.iter().map(|p| &**p).collect();
    let domain = &spec.domains.a_fold;
    let last = spec.last;
    let last_shape = spec.a[last].shape();
    let helper_count: usize = plans.iter().map(|p| p.helper_count()).sum();
    if public.a.members.len() != spec.a.len()
        || witness.is_some_and(|w| {
            w.state.len() != spec.q_state
                || w.a.accs.len() != spec.a.len()
                || w.a
                    .accs
                    .iter()
                    .enumerate()
                    .any(|(k, acc)| acc.is_none() && k != last)
                || w.a.fresh.commitments.len() != last_shape.columns
                || w.a.helpers.len() != helper_count
        })
    {
        return Err(Error::Synthesis);
    }
    // The kept accumulators and their digests; an absent last kind is a
    // placeholder that the start ignores.
    let mut kept = Vec::with_capacity(spec.a.len());
    let mut kept_digests = Vec::with_capacity(spec.a.len());
    for (kind, relation) in spec.a.iter().enumerate() {
        let value = witness.map(|w| {
            w.a.accs[kind]
                .clone()
                .unwrap_or_else(|| placeholder_accumulator::<Eq>(relation))
        });
        let acc = assign_accumulator(lanes, region, &relation.shape(), value.as_ref())?;
        kept_digests.push(accumulator_digest(lanes, region, domain, &acc)?);
        kept.push(acc);
    }
    let start = lanes.glue.boolean(
        region,
        known(witness.map(|w| w.a.accs[last].is_none()).as_ref()),
    )?;
    // A_n folds into (or starts) its kind's accumulator.
    let mut commitments = Vec::with_capacity(last_shape.columns);
    for column in 0..last_shape.columns {
        let point = witness.map(|w| Eq::from(w.a.fresh.commitments[column]));
        commitments.push(lanes.assign_point(region, known(point.as_ref()))?);
    }
    let mut rows = public.a_public.iter();
    let mut columns = Vec::with_capacity(last_shape.instance_lengths.len());
    for length in &last_shape.instance_lengths {
        let mut column = Vec::with_capacity(*length);
        for _ in 0..*length {
            let row = rows.next().ok_or(Error::Synthesis)?;
            column.push(import_row(lanes, region, row)?);
        }
        columns.push(column);
    }
    let fresh = InstanceCells {
        commitments,
        public: columns,
    };
    let proof = assign_proof(lanes, region, &last_shape, witness.map(|w| &w.a.fold))?;
    let folded = verify_fold_or_start(
        lanes,
        region,
        &last_shape,
        domain,
        &kept[last],
        &fresh,
        &proof,
        &start,
    )?;
    // A_n's passthrough is Q_{n-1}'s h_out over the kept digests (zero for an
    // absent kind) and the Vesta claim accumulator's kgwfcld1.
    let claims = assign_claim_accumulator(lanes, region, witness.map(|w| &w.vesta))?;
    let claim_digest =
        claim_accumulator_digest(lanes, region, &spec.domains.vesta_claims, &claims)?;
    let zero = lanes.glue.constant(region, Fq::ZERO)?;
    let mut digests = kept_digests.clone();
    digests[last] = lanes
        .glue
        .select(region, &start, &zero, &kept_digests[last])?;
    digests.push(claim_digest.clone());
    let state = (0..spec.q_state)
        .map(|i| {
            lanes
                .glue
                .witness(region, known(witness.map(|w| &w.state[i])))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let step = lanes
        .glue
        .witness(region, known(witness.map(|w| Fq::from(w.step)).as_ref()))?;
    let base = lanes.glue.constant(region, spec.base_q)?;
    let hash = ivc_hash(
        lanes,
        region,
        &spec.domains.q_ivc,
        &step,
        &base,
        &state,
        &digests,
    )?;
    let passthrough = public.a_public.first().ok_or(Error::Synthesis)?;
    GlueChip::assert_equal(region, passthrough.lo().word(), hash.lo.word())?;
    GlueChip::assert_equal(region, passthrough.hi().word(), hash.hi.word())?;
    // The Vesta claim accumulator's crossing values.
    bind_canonical(lanes, region, &claim_digest, &public.vesta.digest)?;
    for (cells, rows) in [
        (&claims.u, &public.vesta.u),
        (&claims.beta, &public.vesta.beta),
    ] {
        if cells.len() != rows.len() {
            return Err(Error::Synthesis);
        }
        for (cell, row) in cells.iter().zip(rows) {
            bind_scalar(region, cell, row)?;
        }
    }
    bind_scalar(region, &claims.e, &public.vesta.e)?;
    // The decided accumulators: the kept ones with the last kind folded;
    // their digests and field data cross to P.
    let mut decided = kept;
    decided[last] = folded;
    for (kind, (acc, member)) in decided.iter().zip(&public.a.members).enumerate() {
        let digest = if kind == last {
            accumulator_digest(lanes, region, domain, acc)?
        } else {
            kept_digests[kind].clone()
        };
        bind_canonical(lanes, region, &digest, &member.acc_digest)?;
        let data = &member.data;
        let acc_public: Vec<&ScalarCell<Eq>> = acc.instance.public.iter().flatten().collect();
        if acc_public.len() != data.public.len()
            || acc.theta.len() != data.theta.len()
            || acc.r.len() != data.r.len()
            || acc.beta.len() != data.beta.len()
        {
            return Err(Error::Synthesis);
        }
        for (cell, row) in acc_public.into_iter().zip(&data.public) {
            bind_scalar(region, cell, row)?;
        }
        for (cells, rows) in [
            (&acc.theta, &data.theta),
            (&acc.r, &data.r),
            (&acc.beta, &data.beta),
        ] {
            for (cell, row) in cells.iter().zip(rows) {
                bind_scalar(region, cell, row)?;
            }
        }
        bind_scalar(region, &acc.e, &data.e)?;
    }
    // The A-side decider's commitment side and its opening.
    let mut helpers = Vec::with_capacity(helper_count);
    for index in 0..helper_count {
        let point = witness.map(|w| Eq::from(w.a.helpers[index]));
        helpers.push(lanes.assign_point(region, known(point.as_ref()))?);
    }
    let helper = helper_digest(lanes, region, domain, &helpers)?;
    bind_canonical(lanes, region, &helper, &public.a.helper_digest)?;
    let field_digest = import_row(lanes, region, &public.a.field_digest)?;
    let value = import_row(lanes, region, &public.a.value)?;
    let folded_eq = import_row(lanes, region, &public.a.folded)?;
    let mu = plans.first().ok_or(Error::Synthesis)?.mu();
    let opening = assign_batch_opening(lanes, region, mu, witness.map(|w| &w.a.opening))?;
    let batch: Vec<_> = plans.iter().copied().zip(decided.iter()).collect();
    let opened = claim_commitments(lanes, region, &batch, &helpers)?;
    opening_transcript(lanes, region, domain, &field_digest)?;
    let group = verify_batch_bound_group(
        lanes,
        region,
        &plan.opening,
        mu,
        &opened,
        &value,
        &folded_eq,
        &opening,
    )?;
    t::finish(lanes, region)?;
    let EndoAlpha { scalar: alpha, .. } = group.alpha.ok_or(Error::Synthesis)?;
    bind_scalar(region, &alpha, &public.a.alpha)?;
    if group.claim.challenges.len() != public.a.rounds.len() {
        return Err(Error::Synthesis);
    }
    for (cell, row) in group.claim.challenges.iter().zip(&public.a.rounds) {
        bind_scalar(region, cell, row)?;
    }
    Ok((group.claim, claims))
}

impl Circuit<Fq> for OmegaCircuit {
    type Config = OmegaConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            plan: Arc::clone(&self.plan),
            witness: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> OmegaConfig {
        // Range checks spread over a bank of buses sharing one table.
        let verifier =
            VerifierConfig::configure_banked(meta, OMEGA_RANGE_BUSES).expect("fixed bus count");
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let public = INSTANCE_LENGTHS.map(|length| {
            let column = meta.instance_column(length);
            meta.enable_equality(column);
            column
        });
        OmegaConfig {
            verifier,
            bytes,
            public,
        }
    }

    fn synthesize(
        &self,
        config: OmegaConfig,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        let plan = &*self.plan;
        let layout = &plan.p.layout;
        let witness = self.witness.as_deref();
        let proof_length = plan.verifier.proof_length();
        if witness.is_some_and(|w| w.p_public.len() != layout.rows()) {
            return Err(Error::Synthesis);
        }
        let restructure_bytes = witness.map(|w| w.restructure.to_private_bytes());
        let fold_bytes = witness.map(|w| w.fold.to_private_bytes());
        let mut chip = VerifierChip::<Eq>::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let (d_a, g, u) = layouter.assign_region(
            || "fold wrap Omega",
            |mut region| {
                let region = &mut region;
                let tape = super::byte_elements(
                    &mut bytes,
                    &mut chip,
                    region,
                    &[
                        (witness.map(|w| w.p_proof.as_slice()), proof_length),
                        (restructure_bytes.as_deref(), FOLD_WITNESS_BYTES),
                        (fold_bytes.as_deref(), FOLD_WITNESS_BYTES),
                    ],
                )?;
                let (proof_messages, rest) = tape.split_at(proof_length / 32);
                let (restructure_messages, fold_messages) = rest.split_at(FOLD_WITNESS_BYTES / 32);
                let p_length = chip
                    .uint()
                    .assign::<32>(region, Value::known(proof_length as u128))?;
                let restructure_length = super::witness_length(&mut chip, region)?;
                let fold_length = super::witness_length(&mut chip, region)?;
                // P's public column as canonical Fp scalars, and P verified hard.
                let rows = (0..layout.rows())
                    .map(|i| {
                        ScalarCells::<Eq>::assign_value(
                            &mut chip.uint(),
                            region,
                            known(witness.map(|w| &w.p_public[i])),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let key = chip.constant_key(region, &plan.verifier, &plan.key)?;
                let verified = chip.verify(
                    region,
                    &plan.verifier,
                    &key,
                    core::slice::from_ref(&rows),
                    proof_messages,
                    &p_length,
                    VerificationMode::Hard,
                )?;
                let public = layout.parse(&rows)?;
                // Field sides (native Fq).
                {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FieldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        duplex: refs.hash,
                    };
                    q_field_side(
                        &mut lanes,
                        region,
                        &plan.p,
                        &public.q,
                        witness.map(|w| &w.q),
                    )?;
                    pallas_field_side(
                        &mut lanes,
                        region,
                        &plan.p.spec.domains.pallas_claims,
                        &public.pallas,
                        witness.map(|w| &w.pallas),
                    )?;
                }
                // The last A fold, the passthrough, the A-side decider's
                // commitment side and the Vesta claim accumulator.
                let (a_claim, claims) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FoldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        ff: refs.ff,
                        ecc: refs.ecc,
                        duplex: refs.hash,
                    };
                    a_commitment_side(&mut lanes, region, plan, &public, witness)?
                };
                // The Vesta claims' group side at P's rho and v.
                let statement = RestructureStatementCells {
                    commitment: claims.g.clone(),
                    point: public.vesta.rho.clone(),
                    value: public.vesta.value.clone(),
                };
                let restructured = chip.verify_restructure(
                    region,
                    &plan.restructure,
                    &statement,
                    restructure_messages,
                    &restructure_length,
                )?;
                // acc_V = fold(P's claim, the A decider's claim, the restructured claim).
                let p_input = FoldInputCells::from_claim(&mut chip, region, &verified.claim)?;
                let g = chip.constrain_generator(
                    region,
                    a_claim.folded_generator.x(),
                    a_claim.folded_generator.y(),
                )?;
                let a_input = FoldInputCells::from_normalized(
                    &mut chip,
                    region,
                    K_U32,
                    g,
                    public
                        .a
                        .rounds
                        .clone()
                        .try_into()
                        .map_err(|_| Error::Synthesis)?,
                )?;
                let r_input = FoldInputCells::from_claim(&mut chip, region, &restructured.claim)?;
                let acc_v = chip.verify_fold(
                    region,
                    &plan.fold,
                    &[p_input, a_input, r_input],
                    fold_messages,
                    &fold_length,
                    VerificationMode::Hard,
                )?;
                let d_a = public.d_a.native_word().cloned().ok_or(Error::Synthesis)?;
                let g = [acc_v.claim.g().x().clone(), acc_v.claim.g().y().clone()];
                let u = acc_v
                    .claim
                    .challenges()
                    .iter()
                    .map(|c| c.native_word().cloned().ok_or(Error::Synthesis))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((d_a, g, u))
            },
        )?;
        layouter.constrain_instance(d_a.cell(), config.public[0], 0)?;
        for (row, word) in g.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public[1], row)?;
        }
        for (row, word) in u.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public[2], row)?;
        }
        Ok(())
    }
}
