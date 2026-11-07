//! The pieces of the Fp wrap P (module documentation of [`super`]) and P
//! assembled ([`PCircuit`]).
//!
//! Every function lays out one piece on lanes P already owns and returns the
//! cells of its crossing values; [`PCircuit`] binds each to its row of P's
//! public column ([`super::crossing`]).

use std::sync::Arc;

use ff::Field;
use iroha_fold::{
    Accumulator,
    circuit::{
        FoldLanes, ScalarCell,
        claims::{
            assign_claim_accumulator, assign_claim_accumulator_field, assign_restructure_field,
            claim_accumulator_digest, verify_restructure_field,
        },
        decider_v2::{
            assign_accumulator_field_terms, assign_decider_v2_field_proof, claim_commitments,
            helper_digest, opening_scalars, opening_transcript, verify_decider_v2_field,
        },
        field::FieldLanes,
        ivc::ivc_hash,
        mle::{
            EndoAlpha, OpeningClaimCells, OpeningKey, assign_batch_opening,
            verify_batch_bound_group,
        },
        transcript as t,
        verify::{accumulator_digest, assign_accumulator},
    },
    claims::{ClaimAccumulator, ROUNDS, RestructureProof},
    decider_v2::{DeciderV2Plan, DeciderV2Proof},
};
use iroha_pasta::{Ep, EpAffine, Eq, Fp, Fq};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::mle::BatchOpening,
};
use iroha_plonk_gadgets::{
    GlueChip, Uint, Word,
    bytes::{
        element::LeElement,
        tape::{BytesChip, BytesConfig},
    },
};
use iroha_plonk_recursion::{
    FOLD_WITNESS_BYTES, FoldPurpose, FoldWitness,
    accumulation_circuit::{FoldInputCells, FoldPlan},
    codec::ScalarCells,
    restructure_circuit::{RestructurePlan, RestructureStatementCells},
    verifier::{GeneratorClaimCells, VerificationMode, VerifierChip, VerifierConfig, VerifierPlan},
};

use super::{
    K_U32, WrapError, WrapSpec, copy_terms_cells,
    crossing::{
        AClaimCrossing, AccumulatorTerms, PLayout, PPublic, Pair, PallasClaimCrossing, QCrossing,
        QMember, RPublic, VestaClaimCrossing,
    },
    r::RPlan,
};
use crate::a_relation::LINEAGE_DOMAIN;

/// The Q-side decider's witness in P: the accumulators of every Q kind, the
/// batch's helper commitments and bound opening, and the imported field-side
/// values.
#[derive(Clone, Debug)]
pub struct QGroupWitness {
    /// The decided Q accumulators, in kind order.
    pub accs: Vec<Accumulator<Ep>>,
    /// `proof.helpers`.
    pub helpers: Vec<EpAffine>,
    /// The bound opening.
    pub opening: BatchOpening<Ep>,
    /// `d_F`, `v*`, `b'` (scalars of the Q relations, `Fq`).
    pub imports: [Fq; 3],
    /// The batch's copy challenges `gamma`, `eta` (Omega's field transcript).
    pub copy_challenges: [Fq; 2],
}

/// The A-side decider's field-side witness in P: the A accumulators after
/// R's last A fold, the batch proof's field messages, and the copy terms R
/// computes.
#[derive(Clone, Debug)]
pub struct AFieldWitness {
    /// The decided A accumulators, in kind order.
    pub accs: Vec<Accumulator<Eq>>,
    /// The batch proof.
    pub proof: DeciderV2Proof<Eq>,
    /// Each member's instance copy term `I` (R computes them).
    pub copy_terms: Vec<Fp>,
}

fn known<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// The integer of one of R's rows that holds an `Fp` value, as a native word.
fn recompose_row(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    row: &ScalarCells<Ep>,
) -> Result<Word<Fp>, Error> {
    lanes.recompose(region, row.lo().word(), row.hi().word())
}

/// Constrains `word` to one of R's rows that holds an `Fp` value.
fn bind_row(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    word: &Word<Fp>,
    row: &ScalarCells<Ep>,
) -> Result<(), Error> {
    let value = recompose_row(lanes, region, row)?;
    FieldLanes::assert_equal(region, word, &value)
}

/// The A-side decider's field side over every A kind (native `Fp`), against
/// R's public column. Each member's digest limbs and field terms are R's
/// rows, as are `D_H`, `alpha` and `u`. The copy challenges, `d_F`, `v*` and
/// `b'` it computes are forced onto R's rows (R imported them). Returns the
/// round challenges' words, which P forwards to Omega's `acc_V`.
///
/// # Errors
/// Layout errors or a witness of another shape; a false proof is
/// unsatisfiable.
pub fn a_field_side(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    plan: &PPlan,
    rows: &RPublic<ScalarCells<Ep>>,
    witness: Option<&AFieldWitness>,
) -> Result<Vec<Word<Fp>>, Error> {
    let spec = &*plan.spec;
    let plans: Vec<&DeciderV2Plan<Eq>> = plan.r.a_plans.iter().map(|p| &**p).collect();
    if rows.members.len() != plans.len()
        || witness.is_some_and(|w| w.accs.len() != plans.len() || w.copy_terms.len() != plans.len())
    {
        return Err(Error::Synthesis);
    }
    let mut members = Vec::with_capacity(plans.len());
    for (index, (relation, member)) in spec.a.iter().zip(&rows.members).enumerate() {
        let acc = assign_accumulator_field_terms::<Eq>(
            lanes,
            region,
            &spec.domains.a_fold,
            &relation.shape(),
            witness.map(|w| &w.accs[index]),
            witness.map(|w| w.copy_terms[index]),
        )?;
        FieldLanes::assert_equal(region, &acc.digest[0], member.acc_digest.lo().word())?;
        FieldLanes::assert_equal(region, &acc.digest[1], member.acc_digest.hi().word())?;
        let terms = &member.terms;
        if acc.theta.len() != terms.theta.len()
            || acc.r.len() != terms.r.len()
            || acc.beta.len() != terms.beta.len()
        {
            return Err(Error::Synthesis);
        }
        for (words, rows) in [
            (&acc.theta, &terms.theta),
            (&acc.r, &terms.r),
            (&acc.beta, &terms.beta),
        ] {
            for (word, row) in words.iter().zip(rows) {
                bind_row(lanes, region, word, row)?;
            }
        }
        bind_row(lanes, region, &acc.e, &terms.e)?;
        let copy_terms = acc.copy_terms.as_ref().ok_or(Error::Synthesis)?;
        bind_row(lanes, region, copy_terms, &terms.copy_terms)?;
        members.push(acc);
    }
    let proof = assign_decider_v2_field_proof(lanes, region, &plans, witness.map(|w| &w.proof))?;
    let helper_digest = [
        rows.helper_digest.lo().word().clone(),
        rows.helper_digest.hi().word().clone(),
    ];
    let batch: Vec<_> = plans.iter().copied().zip(members.iter()).collect();
    let output = verify_decider_v2_field(
        lanes,
        region,
        &spec.domains.a_fold,
        &batch,
        &helper_digest,
        &proof,
    )?;
    let alpha = recompose_row(lanes, region, &rows.alpha)?;
    let rounds = rows
        .rounds
        .iter()
        .map(|row| recompose_row(lanes, region, row))
        .collect::<Result<Vec<_>, _>>()?;
    let (value, folded) = opening_scalars(
        lanes,
        region,
        &output.at_sigma,
        &output.point,
        &alpha,
        &rounds,
    )?;
    for (word, row) in output.copy_challenges.iter().zip(&rows.copy_challenges) {
        bind_row(lanes, region, word, row)?;
    }
    bind_row(lanes, region, &output.field_digest, &rows.field_digest)?;
    bind_row(lanes, region, &value, &rows.value)?;
    bind_row(lanes, region, &folded, &rows.folded)?;
    Ok(rounds)
}

fn pair<C: iroha_pasta::PastaCurve>(cell: &ScalarCell<C>) -> Pair<Word<C::Base>> {
    [cell.s6.lo().word().clone(), cell.s6.hi().word().clone()]
}

/// The Q-side decider's commitment side over every Q kind, with its crossing
/// values as cells, and the opening's undecided Pallas claim `(G'_0, u)` for
/// `acc_P`. Every Q kind shares the side's fold domain, which is also the
/// batch decider's.
///
/// # Errors
/// Layout errors or a witness of another shape; a false opening is
/// unsatisfiable.
pub fn q_commitment_side(
    lanes: &mut FoldLanes<'_, Ep>,
    region: &mut Region<'_, Fp>,
    plan: &PPlan,
    witness: Option<&QGroupWitness>,
) -> Result<(QCrossing<Word<Fp>>, OpeningClaimCells<Ep>), Error> {
    let spec = &*plan.spec;
    let plans: Vec<&DeciderV2Plan<Ep>> = plan.q_plans.iter().map(|p| &**p).collect();
    let mu = plans.first().ok_or(Error::Synthesis)?.mu();
    let helper_count: usize = plans.iter().map(|p| p.helper_count()).sum();
    if witness.is_some_and(|w| w.helpers.len() != helper_count || w.accs.len() != plans.len()) {
        return Err(Error::Synthesis);
    }
    let challenge = |index: usize| {
        witness.map_or_else(Value::unknown, |w| Value::known(w.copy_challenges[index]))
    };
    let gamma = lanes.assign_scalar(region, challenge(0))?;
    let eta = lanes.assign_scalar(region, challenge(1))?;
    let mut accs = Vec::with_capacity(plans.len());
    let mut members = Vec::with_capacity(plans.len());
    let domain = &spec.domains.q_fold;
    for (index, (relation, member_plan)) in spec.q.iter().zip(&plans).enumerate() {
        let acc = assign_accumulator(
            lanes,
            region,
            &relation.shape(),
            witness.map(|w| &w.accs[index]),
        )?;
        let acc_digest = accumulator_digest(lanes, region, domain, &acc)?;
        let copy_terms = copy_terms_cells(
            lanes,
            region,
            member_plan,
            &acc.instance.public,
            &gamma,
            &eta,
        )?;
        members.push(QMember {
            acc_digest,
            terms: AccumulatorTerms {
                theta: acc.theta.iter().map(pair).collect(),
                r: acc.r.iter().map(pair).collect(),
                beta: acc.beta.iter().map(pair).collect(),
                e: pair(&acc.e),
                copy_terms: pair(&copy_terms),
            },
        });
        accs.push(acc);
    }
    let mut helpers = Vec::with_capacity(helper_count);
    for index in 0..helper_count {
        let value =
            witness.map_or_else(Value::unknown, |w| Value::known(Ep::from(w.helpers[index])));
        helpers.push(lanes.assign_point(region, value)?);
    }
    let helper = helper_digest(lanes, region, domain, &helpers)?;
    let import =
        |index: usize| witness.map_or_else(Value::unknown, |w| Value::known(w.imports[index]));
    let field_digest = lanes.assign_scalar(region, import(0))?;
    let value = lanes.assign_scalar(region, import(1))?;
    let folded = lanes.assign_scalar(region, import(2))?;
    let opening = assign_batch_opening(lanes, region, mu, witness.map(|w| &w.opening))?;
    let batch: Vec<_> = plans.iter().copied().zip(accs.iter()).collect();
    let commitments = claim_commitments(lanes, region, &batch, &helpers)?;
    opening_transcript(lanes, region, domain, &field_digest)?;
    let group = verify_batch_bound_group(
        lanes,
        region,
        &plan.opening,
        mu,
        &commitments,
        &value,
        &folded,
        &opening,
    )?;
    t::finish(lanes, region)?;
    let EndoAlpha { scalar: alpha, .. } = group.alpha.ok_or(Error::Synthesis)?;
    let crossing = QCrossing {
        members,
        helper_digest: helper,
        copy_challenges: [pair(&gamma), pair(&eta)],
        field_digest: pair(&field_digest),
        value: pair(&value),
        folded: pair(&folded),
        alpha: pair(&alpha),
        rounds: group.claim.challenges.iter().map(pair).collect(),
    };
    Ok((crossing, group.claim))
}

/// The Pallas claim accumulator's group side in P: the accumulator and its
/// digest, the imported sumcheck point and value, and the committed-mask
/// opening of `G*` there.
#[derive(Clone, Debug)]
pub struct PallasClaimWitness {
    /// The claim accumulator A keeps.
    pub acc: ClaimAccumulator<Ep>,
    /// The restructuring sumcheck's `rho` (Omega).
    pub rho: [Fq; ROUNDS],
    /// The claimed `s~(rho)` (Omega).
    pub value: Fq,
}

/// The Pallas claim accumulator's group side: its crossing values and the
/// restructured structured claim for `acc_P`.
///
/// # Errors
/// Layout errors; a false opening is unsatisfiable.
pub fn pallas_claim_side(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    plan: &RestructurePlan<Ep>,
    messages: &[LeElement<Fp>],
    length: &Uint<Fp, 32>,
    witness: Option<&PallasClaimWitness>,
) -> Result<(PallasClaimCrossing<Word<Fp>>, GeneratorClaimCells<Ep>), Error> {
    let (acc, digest, rho, value) = {
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes {
            glue: refs.glue,
            range: refs.range,
            ff: refs.ff,
            ecc: refs.ecc,
            duplex: refs.hash,
        };
        let acc = assign_claim_accumulator(&mut lanes, region, witness.map(|w| &w.acc))?;
        let digest = claim_accumulator_digest(&mut lanes, region, domain, &acc)?;
        let rho = (0..ROUNDS)
            .map(|j| lanes.assign_scalar(region, known(witness.map(|w| &w.rho[j]))))
            .collect::<Result<Vec<_>, _>>()?;
        let value = lanes.assign_scalar(region, known(witness.map(|w| &w.value)))?;
        (acc, digest, rho, value)
    };
    let point = rho
        .iter()
        .map(|c| ScalarCells::from_limbs(&mut chip.uint(), region, c.s6.lo(), c.s6.hi()))
        .collect::<Result<Vec<_>, _>>()?;
    let opened = ScalarCells::from_limbs(&mut chip.uint(), region, value.s6.lo(), value.s6.hi())?;
    let statement = RestructureStatementCells {
        commitment: acc.g.clone(),
        point,
        value: opened,
    };
    let output = chip.verify_restructure(region, plan, &statement, messages, length)?;
    let crossing = PallasClaimCrossing {
        digest,
        u: acc.u.iter().map(pair).collect(),
        beta: acc.beta.iter().map(pair).collect(),
        e: pair(&acc.e),
        rho: rho.iter().map(pair).collect(),
        value: pair(&value),
    };
    Ok((crossing, output.claim))
}

/// The Vesta claim accumulator's field side in P: its field data and the
/// restructuring sumcheck.
#[derive(Clone, Debug)]
pub struct VestaClaimWitness {
    /// The claim accumulator Q keeps.
    pub acc: ClaimAccumulator<Eq>,
    /// The restructuring sumcheck's messages.
    pub proof: RestructureProof<Fp>,
}

/// The Vesta claim accumulator's restructuring sumcheck (native `Fp`) with
/// its crossing values as cells: the digest limbs (Omega's), the field data,
/// and the exported `rho` and `v`.
///
/// # Errors
/// Layout errors; a false sumcheck is unsatisfiable.
pub fn vesta_claim_side(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    witness: Option<&VestaClaimWitness>,
) -> Result<VestaClaimCrossing<Word<Fp>>, Error> {
    let acc = assign_claim_accumulator_field::<Eq>(lanes, region, domain, witness.map(|w| &w.acc))?;
    let proof = assign_restructure_field::<Eq>(lanes, region, witness.map(|w| &w.proof))?;
    let (rho, value) = verify_restructure_field(lanes, region, domain, &acc, &proof)?;
    Ok(VestaClaimCrossing {
        digest: acc.digest,
        u: acc.u,
        beta: acc.beta,
        e: acc.e,
        rho,
        value,
    })
}

/// P's fixed program.
#[derive(Clone, Debug)]
pub struct PPlan {
    /// The chain's fixed parts.
    pub spec: Arc<WrapSpec>,
    /// Each Q kind's decider plan, in kind order.
    pub q_plans: Vec<Arc<DeciderV2Plan<Ep>>>,
    /// R's program (the A kinds' plans and R's public column).
    pub r: Arc<RPlan>,
    /// R's verifier program.
    pub r_verifier: VerifierPlan<Ep>,
    /// R's key, a circuit constant.
    pub r_key: VerifyingKey<Ep>,
    /// The Q opening's fixed generators.
    pub opening: OpeningKey<Ep>,
    /// The Pallas claim restructuring's pinned points and context.
    pub restructure: RestructurePlan<Ep>,
    /// `acc_P`'s fold: the Q decider's claim, the restructured claim, R's.
    pub fold: FoldPlan<Ep>,
    /// P's public column.
    pub layout: PLayout,
}

impl PPlan {
    /// P's program for `spec`, over R's program and key.
    ///
    /// # Errors
    /// [`WrapError::Input`] for a spec without Q kinds, a boundary or
    /// lineage position out of range, Q relations the decider does not admit
    /// at k16 or that read public values beyond their copies, or a key of
    /// another descriptor.
    pub fn new(
        r: Arc<RPlan>,
        binding: DescriptorBinding,
        key: VerifyingKey<Ep>,
    ) -> Result<Self, WrapError> {
        let spec = Arc::clone(&r.spec);
        if spec.q.is_empty()
            || spec
                .boundary
                .iter()
                .chain(&spec.lineage)
                .any(|&i| i >= spec.a_state)
            || key.descriptor_digest() != binding.digest()
        {
            return Err(WrapError::Input);
        }
        let q_plans = spec
            .q
            .iter()
            .map(|r| DeciderV2Plan::new(r.clone(), &spec.key_q).map(Arc::new))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| WrapError::Input)?;
        // Every member's public columns stay with its digest: the field side
        // imports the copy term, so no relation may read public values beyond
        // its copies.
        if q_plans
            .iter()
            .any(|p| u32::try_from(p.mu()) != Ok(K_U32) || p.reads_public_beyond_copies())
        {
            return Err(WrapError::Input);
        }
        let layout = PLayout::new(
            &spec.q.iter().map(|r| r.shape()).collect::<Vec<_>>(),
            K_U32 as usize,
        );
        Ok(Self {
            opening: OpeningKey::new(spec.params_q.params()),
            restructure: RestructurePlan::new(&spec.params_q).map_err(|_| WrapError::Input)?,
            fold: FoldPlan::new(&spec.params_q, vec![K_U32; 3], FoldPurpose::FoldWrapPallas)
                .map_err(|_| WrapError::Input)?,
            r_verifier: VerifierPlan::new(binding, spec.params_q.clone())
                .map_err(|_| WrapError::Input)?,
            r_key: key,
            layout,
            q_plans,
            r,
            spec,
        })
    }
}

/// Every input of P besides its plan; private to the wallet.
pub struct PWitness {
    /// `A_n`'s step index.
    pub step: u64,
    /// `z_A`, the A state after `A_n`.
    pub state: Vec<Fp>,
    /// R's public column.
    pub r_public: Vec<Fq>,
    /// R's proof.
    pub r_proof: Vec<u8>,
    /// The Q-side decider's commitment side.
    pub q: QGroupWitness,
    /// The A-side decider's field side.
    pub a: AFieldWitness,
    /// The Pallas claim accumulator's group side.
    pub pallas: PallasClaimWitness,
    /// The Vesta claim accumulator's field side.
    pub vesta: VestaClaimWitness,
    /// The Pallas claim accumulator's committed-mask opening.
    pub restructure: FoldWitness<Ep>,
    /// `acc_P`'s fold witness.
    pub fold: FoldWitness<Ep>,
}

impl core::fmt::Debug for PWitness {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PWitness { private: [REDACTED] }")
    }
}

/// Range buses of P's verifier lanes.
pub const P_RANGE_BUSES: usize = 2;

/// The Fp wrap P.
#[derive(Clone, Debug)]
pub struct PCircuit {
    /// The program.
    pub plan: Arc<PPlan>,
    /// The witness (`None`: key generation).
    pub witness: Option<Arc<PWitness>>,
}

/// P's columns: the shared verifier lanes, a byte tape and one public column.
#[derive(Clone, Debug)]
pub struct PConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    instance: Column<Instance>,
}

impl Circuit<Fp> for PCircuit {
    type Config = PConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;

    fn without_witnesses(&self) -> Self {
        Self {
            plan: Arc::clone(&self.plan),
            witness: None,
        }
    }

    fn params(&self) -> usize {
        self.plan.layout.rows()
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> PConfig {
        Self::configure_with_params(meta, 0)
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, rows: usize) -> PConfig {
        // P verifies R, so its foreign-field kernel keeps its own columns;
        // range checks spread over a bank of buses.
        let verifier =
            VerifierConfig::configure_banked(meta, P_RANGE_BUSES).expect("fixed bus count");
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let instance = meta.instance_column(rows);
        meta.enable_equality(instance);
        PConfig {
            verifier,
            bytes,
            instance,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn synthesize(&self, config: PConfig, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let plan = &*self.plan;
        let spec = &*plan.spec;
        let witness = self.witness.as_deref();
        let r_layout = &plan.r.layout;
        let r_proof_length = plan.r_verifier.proof_length();
        if witness.is_some_and(|w| {
            w.state.len() != spec.a_state
                || w.r_public.len() != r_layout.rows()
                || w.r_proof.len() != r_proof_length
        }) {
            return Err(Error::Synthesis);
        }
        let restructure_bytes = witness.map(|w| w.restructure.to_private_bytes());
        let fold_bytes = witness.map(|w| w.fold.to_private_bytes());
        let mut chip = VerifierChip::<Ep>::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let rows = layouter.assign_region(
            || "fold wrap P",
            |mut region| {
                let region = &mut region;
                let tape = super::byte_elements(
                    &mut bytes,
                    &mut chip,
                    region,
                    &[
                        (witness.map(|w| w.r_proof.as_slice()), r_proof_length),
                        (restructure_bytes.as_deref(), FOLD_WITNESS_BYTES),
                        (fold_bytes.as_deref(), FOLD_WITNESS_BYTES),
                    ],
                )?;
                let (proof_messages, rest) = tape.split_at(r_proof_length / 32);
                let (restructure_messages, fold_messages) = rest.split_at(FOLD_WITNESS_BYTES / 32);
                let r_length = chip
                    .uint()
                    .assign::<32>(region, Value::known(r_proof_length as u128))?;
                let restructure_length = super::witness_length(&mut chip, region)?;
                let fold_length = super::witness_length(&mut chip, region)?;
                // R's public column as canonical Fq scalars, and R verified hard.
                let r_rows = (0..r_layout.rows())
                    .map(|i| {
                        ScalarCells::<Ep>::assign_value(
                            &mut chip.uint(),
                            region,
                            known(witness.map(|w| &w.r_public[i])),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let key = chip.constant_key(region, &plan.r_verifier, &plan.r_key)?;
                let verified = chip.verify(
                    region,
                    &plan.r_verifier,
                    &key,
                    core::slice::from_ref(&r_rows),
                    proof_messages,
                    &r_length,
                    VerificationMode::Hard,
                )?;
                let r_public = r_layout.parse(&r_rows)?;
                // Field sides (native Fp): the A-side decider against R's
                // values, and the Vesta claims, whose digest is R's (Omega's,
                // forwarded).
                let (a_public, rounds, vesta) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FieldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        duplex: refs.hash,
                    };
                    let rounds =
                        a_field_side(&mut lanes, region, plan, &r_public, witness.map(|w| &w.a))?;
                    let vesta = vesta_claim_side(
                        &mut lanes,
                        region,
                        &spec.domains.vesta_claims,
                        witness.map(|w| &w.vesta),
                    )?;
                    FieldLanes::assert_equal(
                        region,
                        &vesta.digest[0],
                        r_public.claim_digest.lo().word(),
                    )?;
                    FieldLanes::assert_equal(
                        region,
                        &vesta.digest[1],
                        r_public.claim_digest.hi().word(),
                    )?;
                    let a_public = r_public
                        .a_public
                        .iter()
                        .map(|row| recompose_row(&mut lanes, region, row))
                        .collect::<Result<Vec<_>, _>>()?;
                    (a_public, rounds, vesta)
                };
                // The Q-side decider's commitment side and opening (Pallas).
                let (q, q_claim) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FoldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        ff: refs.ff,
                        ecc: refs.ecc,
                        duplex: refs.hash,
                    };
                    q_commitment_side(&mut lanes, region, plan, witness.map(|w| &w.q))?
                };
                // The Pallas claims' group side.
                let (pallas, restructured) = pallas_claim_side(
                    &mut chip,
                    region,
                    &spec.domains.pallas_claims,
                    &plan.restructure,
                    restructure_messages,
                    &restructure_length,
                    witness.map(|w| &w.pallas),
                )?;
                // acc_P = fold(Q decider claim, restructured claim, R's claim).
                let g = chip.constrain_generator(
                    region,
                    q_claim.folded_generator.x(),
                    q_claim.folded_generator.y(),
                )?;
                let challenges = q_claim
                    .challenges
                    .iter()
                    .map(|u| {
                        ScalarCells::from_limbs(&mut chip.uint(), region, u.s6.lo(), u.s6.hi())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let q_input = FoldInputCells::from_normalized(
                    &mut chip,
                    region,
                    K_U32,
                    g,
                    challenges.try_into().map_err(|_| Error::Synthesis)?,
                )?;
                let restructured = FoldInputCells::from_claim(&mut chip, region, &restructured)?;
                let r_input = FoldInputCells::from_claim(&mut chip, region, &verified.claim)?;
                let acc_p = chip.verify_fold(
                    region,
                    &plan.fold,
                    &[q_input, restructured, r_input],
                    fold_messages,
                    &fold_length,
                    VerificationMode::Hard,
                )?;
                // The frame: z_A at an operation boundary, and h_out_n against
                // A_n's public column (from R).
                let state = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FoldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        ff: refs.ff,
                        ecc: refs.ecc,
                        duplex: refs.hash,
                    };
                    let state = (0..spec.a_state)
                        .map(|i| {
                            lanes
                                .glue
                                .witness(region, known(witness.map(|w| &w.state[i])))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    // An Omega only at an operation boundary: the step context
                    // and the program counter are zero.
                    for &index in &spec.boundary {
                        lanes
                            .glue
                            .enforce_constant(region, &state[index], Fp::ZERO)?;
                    }
                    let step = lanes
                        .glue
                        .witness(region, known(witness.map(|w| Fp::from(w.step)).as_ref()))?;
                    let base = lanes.glue.constant(region, spec.base_a)?;
                    let mut digests: Vec<Word<Fp>> =
                        q.members.iter().map(|m| m.acc_digest.clone()).collect();
                    digests.push(pallas.digest.clone());
                    let h_out = ivc_hash(
                        &mut lanes,
                        region,
                        &spec.domains.a_ivc,
                        &step,
                        &base,
                        &state,
                        &digests,
                    )?;
                    let h_out_n = a_public.get(1).ok_or(Error::Synthesis)?;
                    GlueChip::assert_equal(region, &h_out.word, h_out_n)?;
                    state
                };
                // D_A over the lineage fields of z_A and acc_P.
                let mut words: Vec<Word<Fp>> = spec
                    .lineage
                    .iter()
                    .map(|&i| state.get(i).cloned().ok_or(Error::Synthesis))
                    .collect::<Result<_, _>>()?;
                words.extend([acc_p.claim.g().x().clone(), acc_p.claim.g().y().clone()]);
                for scalar in acc_p.claim.challenges() {
                    words.extend([scalar.lo().word().clone(), scalar.hi().word().clone()]);
                }
                let d_a = chip.hash_words(region, LINEAGE_DOMAIN, &words)?;
                let limbs =
                    |row: &ScalarCells<Ep>| [row.lo().word().clone(), row.hi().word().clone()];
                let public = PPublic {
                    d_a,
                    q,
                    a_claim: AClaimCrossing {
                        g: [limbs(&r_public.claim[0]), limbs(&r_public.claim[1])],
                        u: rounds,
                    },
                    pallas,
                    vesta,
                };
                Ok(public.rows())
            },
        )?;
        for (row, word) in rows.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}
