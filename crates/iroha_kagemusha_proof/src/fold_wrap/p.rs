//! The pieces of the Fp wrap P (module documentation of [`super`]) and P
//! assembled ([`PCircuit`]).
//!
//! Every function lays out one piece on lanes P already owns and returns the
//! cells of its crossing values; [`PCircuit`] binds each to its row of P's
//! public column ([`super::crossing`]).

use std::sync::Arc;

use ff::PrimeField;
use iroha_fold::{
    Accumulator,
    circuit::{
        FoldLanes, ScalarCell,
        claims::{
            assign_claim_accumulator, assign_claim_accumulator_field, assign_restructure_field,
            claim_accumulator_digest, verify_restructure_field,
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
        verify::{accumulator_digest, assign_accumulator},
    },
    claims::{ClaimAccumulator, ROUNDS, RestructureProof},
    decider_v2::{DeciderV2Plan, DeciderV2Proof},
    relation::FoldShape,
    transcript::s6_limbs,
};
use iroha_pasta::{Ep, EpAffine, Eq, Fp, Fq};
use iroha_plonk::{
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
    verifier::{GeneratorClaimCells, VerificationMode, VerifierChip, VerifierConfig},
};

use super::{
    K_U32, WrapError, WrapSpec,
    crossing::{
        ACrossing, AccumulatorData, PLayout, PPublic, Pair, PallasClaimCrossing, QCrossing,
        VestaClaimCrossing,
    },
};
use crate::a_relation::LINEAGE_DOMAIN;

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

/// The A-side decider's field-side witness in P: `U_A'` (the accumulator after
/// Omega's last A fold), the decider proof's field messages, and the values
/// Omega's commitment side computes (`D_H`, the opening's `alpha` and `u`).
#[derive(Clone, Debug)]
pub struct AFieldWitness {
    /// `U_A'`.
    pub acc: Accumulator<Eq>,
    /// The decider proof.
    pub proof: DeciderV2Proof<Eq>,
    /// `D_H` of the proof's helpers (a Vesta base-field value).
    pub helper_digest: Fq,
    /// The opening's batching challenge (the endoscalar).
    pub alpha: Fp,
    /// The opening's round challenges.
    pub rounds: Vec<Fp>,
}

fn known<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// The A-side decider's field side with its crossing values as cells: the
/// field data and the digest limbs `assign_accumulator_field` witnesses, the
/// imported `D_H`, `alpha`, `u`, and the exported `d_F`, `v*`, `b'`.
///
/// # Errors
/// Layout errors or a witness of another shape; a false proof is
/// unsatisfiable.
pub fn a_field_side(
    lanes: &mut FieldLanes<'_, Fp>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    plan: &DeciderV2Plan<Eq>,
    shape: &FoldShape,
    witness: Option<&AFieldWitness>,
) -> Result<ACrossing<Word<Fp>>, Error> {
    if witness.is_some_and(|w| w.rounds.len() != plan.mu()) {
        return Err(Error::Synthesis);
    }
    let acc =
        assign_accumulator_field::<Eq>(lanes, region, domain, shape, witness.map(|w| &w.acc))?;
    let proof = assign_decider_v2_field_proof(lanes, region, &[plan], witness.map(|w| &w.proof))?;
    let limbs = witness.map(|w| s6_limbs(&w.helper_digest).map(Fp::from_u128));
    let helper_digest = [
        lanes
            .glue
            .witness(region, known(limbs.as_ref().map(|l| &l[0])))?,
        lanes
            .glue
            .witness(region, known(limbs.as_ref().map(|l| &l[1])))?,
    ];
    let output = verify_decider_v2_field(
        lanes,
        region,
        domain,
        &[(plan, &acc)],
        &helper_digest,
        &proof,
    )?;
    let alpha = lanes
        .glue
        .witness(region, known(witness.map(|w| &w.alpha)))?;
    let rounds = (0..plan.mu())
        .map(|j| {
            lanes
                .glue
                .witness(region, known(witness.map(|w| &w.rounds[j])))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (value, folded) = opening_scalars(
        lanes,
        region,
        &output.at_sigma,
        &output.point,
        &alpha,
        &rounds,
    )?;
    Ok(ACrossing {
        acc_digest: acc.digest.clone(),
        helper_digest,
        data: AccumulatorData {
            public: acc.public.concat(),
            theta: acc.theta,
            r: acc.r,
            beta: acc.beta,
            e: acc.e,
        },
        field_digest: output.field_digest,
        value,
        folded,
        alpha,
        rounds,
    })
}

fn pair<C: iroha_pasta::PastaCurve>(cell: &ScalarCell<C>) -> Pair<Word<C::Base>> {
    [cell.s6.lo().word().clone(), cell.s6.hi().word().clone()]
}

/// The Q-side decider's commitment side with its crossing values as cells,
/// and the opening's undecided Pallas claim `(G'_0, u)` for `acc_P`.
///
/// # Errors
/// Layout errors or a witness of another shape; a false opening is
/// unsatisfiable.
pub fn q_commitment_side(
    lanes: &mut FoldLanes<'_, Ep>,
    region: &mut Region<'_, Fp>,
    domain: &[u8; 32],
    plan: &DeciderV2Plan<Ep>,
    shape: &FoldShape,
    key: &OpeningKey<Ep>,
    witness: Option<&QGroupWitness>,
) -> Result<(QCrossing<Word<Fp>>, OpeningClaimCells<Ep>), Error> {
    if witness.is_some_and(|w| w.helpers.len() != plan.helper_count()) {
        return Err(Error::Synthesis);
    }
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
    let field_digest = lanes.assign_scalar(region, import(0))?;
    let value = lanes.assign_scalar(region, import(1))?;
    let folded = lanes.assign_scalar(region, import(2))?;
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
    let data = AccumulatorData {
        public: acc.instance.public.iter().flatten().map(pair).collect(),
        theta: acc.theta.iter().map(pair).collect(),
        r: acc.r.iter().map(pair).collect(),
        beta: acc.beta.iter().map(pair).collect(),
        e: pair(&acc.e),
    };
    let crossing = QCrossing {
        acc_digest,
        helper_digest: helper,
        data,
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
    /// The Q relation's decider plan.
    pub q_plan: Arc<DeciderV2Plan<Ep>>,
    /// The last A relation's decider plan.
    pub a_plan: Arc<DeciderV2Plan<Eq>>,
    /// The Q opening's fixed generators.
    pub opening: OpeningKey<Ep>,
    /// The Pallas claim restructuring's pinned points and context.
    pub restructure: RestructurePlan<Ep>,
    /// `acc_P`'s fold: the Q decider's claim, then the restructured claim.
    pub fold: FoldPlan<Ep>,
    /// P's public column.
    pub layout: PLayout,
}

impl PPlan {
    /// P's program for `spec`.
    ///
    /// # Errors
    /// [`WrapError::Input`] for relations the decider or the k16 recursion
    /// do not admit.
    pub fn new(spec: Arc<WrapSpec>) -> Result<Self, WrapError> {
        let q_plan =
            DeciderV2Plan::new(spec.q.clone(), &spec.key_q).map_err(|_| WrapError::Input)?;
        let a_plan =
            DeciderV2Plan::new(spec.a.clone(), &spec.key_a).map_err(|_| WrapError::Input)?;
        if q_plan.mu() != a_plan.mu() || u32::try_from(q_plan.mu()) != Ok(K_U32) {
            return Err(WrapError::Input);
        }
        let layout = PLayout::new(&spec.a.shape(), &spec.q.shape(), q_plan.mu());
        Ok(Self {
            opening: OpeningKey::new(spec.params_q.params()),
            restructure: RestructurePlan::new(&spec.params_q).map_err(|_| WrapError::Input)?,
            fold: FoldPlan::new(&spec.params_q, vec![K_U32; 2], FoldPurpose::FoldWrapPallas)
                .map_err(|_| WrapError::Input)?,
            layout,
            q_plan: Arc::new(q_plan),
            a_plan: Arc::new(a_plan),
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
    /// `A_n`'s public columns, concatenated.
    pub a_public: Vec<Fp>,
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
pub const P_RANGE_BUSES: usize = 1;
/// P's byte tape: the Pallas restructuring witness, then `acc_P`'s fold
/// witness.
pub const P_TAPE_BYTES: usize = 2 * FOLD_WITNESS_BYTES;

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
        // The byte tape shares the curve lane's first rows (all tape bytes
        // are laid out before any curve operation), and the foreign-field
        // kernel shares the glue ports, so Omega verifies fewer columns.
        let (verifier, bytes) =
            VerifierConfig::configure_serialized_with_byte_tape(meta, P_RANGE_BUSES, P_TAPE_BYTES)
                .expect("fixed bus count");
        let instance = meta.instance_column(rows);
        meta.enable_equality(instance);
        PConfig {
            verifier,
            bytes,
            instance,
        }
    }

    fn synthesize(&self, config: PConfig, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let plan = &*self.plan;
        let spec = &*plan.spec;
        let witness = self.witness.as_deref();
        if witness.is_some_and(|w| {
            w.state.len() != spec.a_state || w.a_public.len() != plan.layout.a_public
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
                        (restructure_bytes.as_deref(), FOLD_WITNESS_BYTES),
                        (fold_bytes.as_deref(), FOLD_WITNESS_BYTES),
                    ],
                )?;
                let (restructure_messages, fold_messages) = tape.split_at(FOLD_WITNESS_BYTES / 32);
                let restructure_length = super::witness_length(&mut chip, region)?;
                let fold_length = super::witness_length(&mut chip, region)?;
                // The A-side decider's and the Vesta claims' field sides (native Fp).
                let (a, vesta) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FieldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        duplex: refs.hash,
                    };
                    let a = a_field_side(
                        &mut lanes,
                        region,
                        &spec.domains.a_fold,
                        &plan.a_plan,
                        &spec.a.shape(),
                        witness.map(|w| &w.a),
                    )?;
                    let vesta = vesta_claim_side(
                        &mut lanes,
                        region,
                        &spec.domains.vesta_claims,
                        witness.map(|w| &w.vesta),
                    )?;
                    (a, vesta)
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
                    q_commitment_side(
                        &mut lanes,
                        region,
                        &spec.domains.q_fold,
                        &plan.q_plan,
                        &spec.q.shape(),
                        &plan.opening,
                        witness.map(|w| &w.q),
                    )?
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
                // acc_P = fold(Q decider claim, restructured claim).
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
                let acc_p = chip.verify_fold(
                    region,
                    &plan.fold,
                    &[q_input, restructured],
                    fold_messages,
                    &fold_length,
                    VerificationMode::Hard,
                )?;
                // The frame: A_n's public column, z_A, and h_out_n.
                let (a_public, state) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FoldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        ff: refs.ff,
                        ecc: refs.ecc,
                        duplex: refs.hash,
                    };
                    let a_public = (0..plan.layout.a_public)
                        .map(|i| {
                            lanes
                                .glue
                                .witness(region, known(witness.map(|w| &w.a_public[i])))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let state = (0..spec.a_state)
                        .map(|i| {
                            lanes
                                .glue
                                .witness(region, known(witness.map(|w| &w.state[i])))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let step = lanes
                        .glue
                        .witness(region, known(witness.map(|w| Fp::from(w.step)).as_ref()))?;
                    let base = lanes.glue.constant(region, spec.base_a)?;
                    let h_out = ivc_hash(
                        &mut lanes,
                        region,
                        &spec.domains.a_ivc,
                        &step,
                        &base,
                        &state,
                        &[q.acc_digest.clone(), pallas.digest.clone()],
                    )?;
                    let h_out_n = a_public.get(1).ok_or(Error::Synthesis)?;
                    GlueChip::assert_equal(region, &h_out.word, h_out_n)?;
                    (a_public, state)
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
                let public = PPublic {
                    a_public,
                    d_a,
                    q,
                    a,
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
