//! The Fq wrap Omega (module documentation of [`super`]).
//!
//! Omega verifies P hard under P's pinned key, reads P's public column as
//! canonical `Fp` scalars and binds every crossing value to its own side:
//! values P computed enter Omega's arithmetic from P's rows, and values Omega
//! computes are constrained to P's rows (canonical limbs, exactly). The Q
//! crossing with S is bound through `D_SQ`: Omega hashes its field side's
//! values and the ones it imports from S, and compares with the `D_SQ` R
//! computed from S's verified rows (P forwards it). Its own public columns
//! are today's `[D_A]`, `acc_V.G`, `acc_V.u`.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    Accumulator,
    circuit::{
        FoldLanes, ScalarCell,
        claims::{
            assign_claim_accumulator, assign_claim_accumulator_field, assign_restructure_field,
            claim_accumulator_digest, verify_restructure_field,
        },
        decider_v2::{
            assign_accumulator_field_terms, assign_decider_v2_field_proof, opening_scalars,
            verify_decider_v2_field,
        },
        field::FieldLanes,
    },
    claims::{ClaimAccumulator, RestructureProof},
    decider_v2::{DeciderV2Plan, DeciderV2Proof},
    transcript::s6_limbs,
};
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::{Column, ConstraintSystem, Instance as InstanceColumn, InstanceType},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, Word, WordHasher,
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
    K_U32, WrapError,
    crossing::{AccumulatorTerms, Pair, PallasClaimCrossing, Q_CROSSING_DOMAIN, QCrossingWords},
    p::PPlan,
    s::SPlan,
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

/// Omega's fixed program: P's program and pinned key, the Vesta claim
/// restructuring's points and `acc_V`'s fold.
#[derive(Clone, Debug)]
pub struct OmegaPlan {
    /// P's program (shapes, domains, crossing layout).
    pub p: Arc<PPlan>,
    /// P's verifier program.
    pub verifier: VerifierPlan<Eq>,
    /// P's key, a circuit constant.
    pub key: VerifyingKey<Eq>,
    /// The Vesta claim restructuring's pinned points and context.
    pub restructure: RestructurePlan<Eq>,
    /// `acc_V`'s fold: P's claim, R's claim for S, the A decider's claim, the
    /// restructured claim.
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
            restructure: RestructurePlan::new(&params).map_err(|_| WrapError::Input)?,
            fold: FoldPlan::new(&params, vec![K_U32; 4], FoldPurpose::FoldWrapVesta)
                .map_err(|_| WrapError::Input)?,
            verifier: VerifierPlan::new(binding, params).map_err(|_| WrapError::Input)?,
            key,
            p,
        })
    }
}

/// The Q-side decider's field side in Omega: every Q kind's field data, the
/// batch proof, and the values S computes that the field side imports.
#[derive(Clone, Debug)]
pub struct QFieldWitness {
    /// The decided Q accumulators, in kind order.
    pub accs: Vec<Accumulator<Ep>>,
    /// Each member's instance copy term `I` (S computes them).
    pub copy_terms: Vec<Fq>,
    /// The batch proof.
    pub proof: DeciderV2Proof<Ep>,
    /// `D_H` of the batch's helpers (S).
    pub helper_digest: Fp,
    /// The opening's batching challenge (S).
    pub alpha: Fq,
    /// The opening's round challenges (S).
    pub rounds: Vec<Fq>,
}

/// The Pallas claims' restructuring sumcheck in Omega.
#[derive(Clone, Debug)]
pub struct PallasFieldWitness {
    /// The claim accumulator A keeps.
    pub acc: ClaimAccumulator<Ep>,
    /// The sumcheck's messages.
    pub proof: RestructureProof<Fq>,
}

/// Every input of Omega besides its plan; private to the wallet.
pub struct OmegaWitness {
    /// P's public column.
    pub p_public: Vec<Fp>,
    /// P's proof.
    pub p_proof: Vec<u8>,
    /// The Q-side decider's field side.
    pub q: QFieldWitness,
    /// The Pallas claims' sumcheck.
    pub pallas: PallasFieldWitness,
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

/// Words assigned from the witness (`None`: key generation).
fn witness_words(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    values: Option<&[Fq]>,
    count: usize,
) -> Result<Vec<Word<Fq>>, Error> {
    if values.is_some_and(|v| v.len() != count) {
        return Err(Error::Synthesis);
    }
    (0..count)
        .map(|i| lanes.glue.witness(region, known(values.map(|v| &v[i]))))
        .collect()
}

/// The Q-side decider's field side over every Q kind (native `Fq`): each
/// member's field data, the batch proof, the field transcript, and `v*`,
/// `b'` from S's opening challenges. Its values and the ones it imports from
/// S (the members' digests and field terms, `D_H`, `alpha`, `u`) must hash to
/// `D_SQ`, R's digest of S's rows (P's pair `q_crossing_digest`).
///
/// # Errors
/// Layout errors or a witness of another shape; a false proof or another
/// crossing is unsatisfiable.
pub fn q_field_side(
    lanes: &mut FieldLanes<'_, Fq>,
    region: &mut Region<'_, Fq>,
    plan: &SPlan,
    q_crossing_digest: &Pair<ScalarCells<Eq>>,
    witness: Option<&QFieldWitness>,
) -> Result<(), Error> {
    let spec = &*plan.spec;
    let domain = &spec.domains.q_fold;
    let plans: Vec<&DeciderV2Plan<Ep>> = plan.q_plans.iter().map(|p| &**p).collect();
    if witness.is_some_and(|w| w.accs.len() != plans.len() || w.copy_terms.len() != plans.len()) {
        return Err(Error::Synthesis);
    }
    let mut members = Vec::with_capacity(plans.len());
    let mut crossing = Vec::with_capacity(plans.len());
    for (index, relation) in spec.q.iter().enumerate() {
        let acc = assign_accumulator_field_terms::<Ep>(
            lanes,
            region,
            domain,
            &relation.shape(),
            witness.map(|w| &w.accs[index]),
            witness.map(|w| w.copy_terms[index]),
        )?;
        let terms = AccumulatorTerms {
            theta: acc.theta.clone(),
            r: acc.r.clone(),
            beta: acc.beta.clone(),
            e: acc.e.clone(),
            copy_terms: acc.copy_terms.clone().ok_or(Error::Synthesis)?,
        };
        crossing.push((acc.digest.clone(), terms));
        members.push(acc);
    }
    let proof = assign_decider_v2_field_proof(lanes, region, &plans, witness.map(|w| &w.proof))?;
    let helper_limbs = witness.map(|w| s6_limbs(&w.helper_digest).map(Fq::from_u128));
    let helper = witness_words(
        lanes,
        region,
        helper_limbs.as_ref().map(|l| l.as_slice()),
        2,
    )?;
    let alpha = witness_words(
        lanes,
        region,
        witness.map(|w| core::slice::from_ref(&w.alpha)),
        1,
    )?;
    let rounds = witness_words(
        lanes,
        region,
        witness.map(|w| w.rounds.as_slice()),
        plan.layout.rounds,
    )?;
    let helper = [helper[0].clone(), helper[1].clone()];
    let batch: Vec<_> = plans.iter().copied().zip(members.iter()).collect();
    let output = verify_decider_v2_field(lanes, region, domain, &batch, &helper, &proof)?;
    let (value, folded) = opening_scalars(
        lanes,
        region,
        &output.at_sigma,
        &output.point,
        &alpha[0],
        &rounds,
    )?;
    let [gamma, eta] = &output.copy_challenges;
    let words = QCrossingWords {
        members: crossing,
        helper_digest: helper,
        copy_challenges: [gamma.clone(), eta.clone()],
        field_digest: output.field_digest.clone(),
        value,
        folded,
        alpha: alpha[0].clone(),
        rounds,
    };
    let digest = lanes
        .duplex
        .hash_words(region, Q_CROSSING_DOMAIN, &words.words())?;
    let expected = pair_word(lanes, region, q_crossing_digest)?;
    FieldLanes::assert_equal(region, &digest, &expected)
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
                        &plan.p.r.s,
                        &public.q_crossing_digest,
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
                // The Vesta claim accumulator: its digest is the one R hashed
                // into the passthrough (P forwards it), its field data P's.
                let (claims, a_claim_g, s_claim_g) = {
                    let refs = chip.fold_lanes()?;
                    let mut lanes = FoldLanes {
                        glue: refs.glue,
                        range: refs.range,
                        ff: refs.ff,
                        ecc: refs.ecc,
                        duplex: refs.hash,
                    };
                    let claims =
                        assign_claim_accumulator(&mut lanes, region, witness.map(|w| &w.vesta))?;
                    let claim_digest = claim_accumulator_digest(
                        &mut lanes,
                        region,
                        &plan.p.spec.domains.vesta_claims,
                        &claims,
                    )?;
                    bind_canonical(&mut lanes, region, &claim_digest, &public.vesta.digest)?;
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
                    let mut field = FieldLanes {
                        glue: lanes.glue,
                        range: lanes.range,
                        duplex: lanes.duplex,
                    };
                    let mut point = |g: &[Pair<ScalarCells<Eq>>; 2]| {
                        Ok::<_, Error>([
                            pair_word(&mut field, region, &g[0])?,
                            pair_word(&mut field, region, &g[1])?,
                        ])
                    };
                    let a_claim_g = point(&public.a_claim.g)?;
                    let s_claim_g = point(&public.s_claim.g)?;
                    (claims, a_claim_g, s_claim_g)
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
                // acc_V = fold(P's claim, R's claim for S, the A decider's
                // claim (both from R, through P), the restructured claim).
                let p_input = FoldInputCells::from_claim(&mut chip, region, &verified.claim)?;
                let mut forwarded = |g: &[Word<Fq>; 2], u: &[ScalarCells<Eq>]| {
                    let g = chip.constrain_generator(region, &g[0], &g[1])?;
                    FoldInputCells::from_normalized(
                        &mut chip,
                        region,
                        K_U32,
                        g,
                        u.to_vec().try_into().map_err(|_| Error::Synthesis)?,
                    )
                };
                let s_input = forwarded(&s_claim_g, &public.s_claim.u)?;
                let a_input = forwarded(&a_claim_g, &public.a_claim.u)?;
                let r_input = FoldInputCells::from_claim(&mut chip, region, &restructured.claim)?;
                let acc_v = chip.verify_fold(
                    region,
                    &plan.fold,
                    &[p_input, s_input, a_input, r_input],
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
