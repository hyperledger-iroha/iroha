//! The second wrap R, over `Fq` (a Pallas proof that P verifies): S's
//! verification and the A side's commitment work, which do not fit in Omega
//! next to its verification of P.
//!
//! R verifies S hard under S's pinned key and hashes S's Q crossing into
//! `D_SQ` (domain [`Q_CROSSING_DOMAIN`]). It folds `A_n` into its kind's
//! accumulator, or starts it when `Q_{n-1}` kept none of that kind. It checks
//! `A_n`'s passthrough as `Q_{n-1}`'s `h_out` over every kept A digest and the
//! Vesta claim digest. It then runs the A-side decider's commitment side and
//! bound opening over every A kind after the fold. Its public column
//! ([`RPublic`]) carries the A side's crossing to P:
//! - R's own values: digests, field terms with their copy terms, `D_H`,
//!   `alpha`, `u` and the claim `G'_0`;
//! - P's values (copy challenges, `d_F`, `v*`, `b'`), which P forces when it
//!   verifies R;
//! - `A_n`'s public columns, for P's `h_out` check;
//! - the Vesta claim digest, which Omega forces through P;
//! - what R forwards from S: the Q member digests (P's `h_out`), the Q
//!   opening claim (P's `acc_P`), `D_SQ` (Omega's, through P), and R's claim
//!   from verifying S (Omega's `acc_V`, through P).

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    Accumulator, Instance,
    circuit::{
        FoldLanes, ScalarCell,
        decider_v2::{claim_commitments, helper_digest, opening_transcript},
        ivc::ivc_hash,
        mle::{EndoAlpha, OpeningKey, assign_batch_opening, verify_batch_bound_group},
        transcript as t,
        verify::{
            InstanceCells, accumulator_digest, assign_accumulator, assign_proof,
            verify_fold_or_start,
        },
    },
    decider_v2::DeciderV2Plan,
    fold::FoldProof,
    ivc::placeholder_accumulator,
};
use iroha_pasta::{Eq, EqAffine, Fp, Fq};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::mle::BatchOpening,
};
use iroha_plonk_gadgets::{
    GlueChip, Word, WordHasher,
    bytes::tape::{BytesChip, BytesConfig},
};
use iroha_plonk_recursion::{
    codec::ScalarCells,
    verifier::{GeneratorClaimCells, VerificationMode, VerifierChip, VerifierConfig, VerifierPlan},
};

use super::{
    K_U32, WrapError, WrapSpec, copy_terms_cells,
    crossing::{
        AccumulatorTerms, ClaimRows, DataShape, Pair, Q_CROSSING_DOMAIN, RLayout, RMember, RPublic,
        SPublic,
    },
    s::SPlan,
};

/// R's fixed program: S's program, verifier program and pinned key, the A
/// kinds' decider plans, the opening's generators and R's public column.
#[derive(Clone, Debug)]
pub struct RPlan {
    /// The chain's fixed parts.
    pub spec: Arc<WrapSpec>,
    /// S's program (the Q kinds' plans and S's public column).
    pub s: Arc<SPlan>,
    /// S's verifier program.
    pub s_verifier: VerifierPlan<Eq>,
    /// S's key, a circuit constant.
    pub s_key: VerifyingKey<Eq>,
    /// Each A kind's decider plan, in kind order.
    pub a_plans: Vec<Arc<DeciderV2Plan<Eq>>>,
    /// The A opening's fixed generators.
    pub opening: OpeningKey<Eq>,
    /// R's public column.
    pub layout: RLayout,
}

impl RPlan {
    /// R's program over S's program and key.
    ///
    /// # Errors
    /// [`WrapError::Input`] for a last kind out of range, A relations the
    /// decider does not admit at k16 or that read public values beyond their
    /// copies, or a key of another descriptor.
    pub fn new(
        s: Arc<SPlan>,
        binding: DescriptorBinding,
        key: VerifyingKey<Eq>,
    ) -> Result<Self, WrapError> {
        let spec = Arc::clone(&s.spec);
        if spec.last >= spec.a.len() || key.descriptor_digest() != binding.digest() {
            return Err(WrapError::Input);
        }
        let a_plans = spec
            .a
            .iter()
            .map(|r| {
                DeciderV2Plan::with_step_runs(r.clone(), &spec.key_a, spec.a_step_runs)
                    .map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| WrapError::Input)?;
        if a_plans
            .iter()
            .any(|p| u32::try_from(p.mu()) != Ok(K_U32) || p.reads_public_beyond_copies())
        {
            return Err(WrapError::Input);
        }
        let layout = RLayout {
            a_public: spec.a[spec.last].shape().instance_lengths.iter().sum(),
            a_data: spec.a.iter().map(|r| DataShape::of(&r.shape())).collect(),
            q_kinds: spec.q.len(),
            rounds: K_U32 as usize,
        };
        Ok(Self {
            opening: OpeningKey::new(spec.params_a.params()),
            s_verifier: VerifierPlan::new(binding, spec.params_a.clone())
                .map_err(|_| WrapError::Input)?,
            s_key: key,
            layout,
            a_plans,
            s,
            spec,
        })
    }
}

/// Every input of R besides its plan; private to the wallet.
#[derive(Clone, Debug)]
pub struct RWitness {
    /// S's public column.
    pub s_public: Vec<Fp>,
    /// S's proof.
    pub s_proof: Vec<u8>,
    /// `Q_{n-1}`'s step index.
    pub step: u64,
    /// `z_Q`, the Q state after `Q_{n-1}`.
    pub state: Vec<Fq>,
    /// The A accumulators `Q_{n-1}` keeps, in kind order; only the last
    /// kind's may be absent (`A_n` then starts it).
    pub accs: Vec<Option<Accumulator<Eq>>>,
    /// `A_n`'s instance.
    pub fresh: Instance<Eq>,
    /// The fold's messages (zero when `A_n` starts its accumulator).
    pub fold: FoldProof<Fp>,
    /// The batch proof's helper commitments.
    pub helpers: Vec<EqAffine>,
    /// The batch's bound opening.
    pub opening: BatchOpening<Eq>,
    /// The batch's copy challenges `gamma`, `eta` (P's field transcript).
    pub copy_challenges: [Fp; 2],
    /// `d_F`, `v*`, `b'` (P's field side).
    pub imports: [Fp; 3],
    /// `kgwfcld1` of the Vesta claim accumulator (Omega's).
    pub claim_digest: Fq,
}

/// The second wrap R.
#[derive(Clone, Debug)]
pub struct RCircuit {
    /// The program.
    pub plan: Arc<RPlan>,
    /// The witness (`None`: key generation).
    pub witness: Option<Arc<RWitness>>,
}

fn known<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

/// The native word of an `Fp` scalar's integer (below `p < q`).
fn word_of(
    glue: &mut GlueChip<Fq>,
    region: &mut Region<'_, Fq>,
    cell: &ScalarCell<Eq>,
) -> Result<Word<Fq>, Error> {
    let two128 = Fq::from_u128(1 << 64).square();
    glue.linear(
        region,
        &[
            (Fq::ONE, cell.s6.lo().word()),
            (two128, cell.s6.hi().word()),
        ],
        Fq::ZERO,
    )
}

/// The integer of one of S's limb rows (below `2^128`: S range-checked its
/// limbs, and the high limb of the row's canonical scalar is zero).
fn limb(
    glue: &mut GlueChip<Fq>,
    region: &mut Region<'_, Fq>,
    row: &ScalarCells<Eq>,
) -> Result<Word<Fq>, Error> {
    glue.enforce_constant(region, row.hi().word(), Fq::ZERO)?;
    Ok(row.lo().word().clone())
}

/// The `Fq` value of S's canonical limb pair.
fn pair_word(
    glue: &mut GlueChip<Fq>,
    region: &mut Region<'_, Fq>,
    pair: &Pair<ScalarCells<Eq>>,
) -> Result<Word<Fq>, Error> {
    let lo = limb(glue, region, &pair[0])?;
    let hi = limb(glue, region, &pair[1])?;
    let two128 = Fq::from_u128(1 << 64).square();
    glue.linear(region, &[(Fq::ONE, &lo), (two128, &hi)], Fq::ZERO)
}

/// The native word of one of S's `Fp` rows (the same integer, `p < q`).
fn native(row: &ScalarCells<Eq>) -> Result<Word<Fq>, Error> {
    row.native_word().cloned().ok_or(Error::Synthesis)
}

/// What R forwards from S, as cells of R's public column.
#[derive(Clone, Debug)]
pub struct Forwarded {
    /// The Q member digests.
    pub q_digests: Vec<Word<Fq>>,
    /// The Q opening claim `(G'_0, u)`.
    pub q_claim: ClaimRows<Word<Fq>>,
    /// `D_SQ`.
    pub q_crossing_digest: Word<Fq>,
    /// R's claim from verifying S.
    pub s_claim: ClaimRows<Word<Fq>>,
}

/// `D_SQ` over S's Q crossing (each digest as its canonical limbs, each `Fq`
/// value recomposed from its limb rows), the Q member digests and opening
/// claim, and R's claim `claim` from verifying S, from S's verified rows.
///
/// # Errors
/// Layout errors or a claim of another length.
pub fn forward(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    s: &SPublic<ScalarCells<Eq>>,
    claim: &GeneratorClaimCells<Eq>,
) -> Result<Forwarded, Error> {
    let words = s.q.to_words(
        |row| [row.lo().word().clone(), row.hi().word().clone()],
        |pair| pair_word(lanes.glue, region, pair),
    )?;
    let q_crossing_digest = lanes
        .duplex
        .hash_words(region, Q_CROSSING_DOMAIN, &words.words())?;
    if claim.challenges().len() != K_U32 as usize {
        return Err(Error::Synthesis);
    }
    Ok(Forwarded {
        q_digests: s
            .q
            .members
            .iter()
            .map(|member| native(&member.acc_digest))
            .collect::<Result<_, _>>()?,
        q_claim: ClaimRows {
            g: [native(&s.claim[0])?, native(&s.claim[1])?],
            u: words.rounds,
        },
        q_crossing_digest,
        s_claim: ClaimRows {
            g: [claim.g().x().clone(), claim.g().y().clone()],
            u: claim
                .challenges()
                .iter()
                .map(native)
                .collect::<Result<_, _>>()?,
        },
    })
}

/// R's A side: the last A fold or start, the passthrough, and the A-side
/// decider's commitment side and opening, with every value of R's public
/// column as a cell (`forwarded` carries what R hands on from S).
///
/// # Errors
/// Layout errors or a witness of another shape; a false fold, passthrough or
/// opening is unsatisfiable.
#[allow(clippy::too_many_lines)]
pub fn r_side(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    plan: &RPlan,
    witness: Option<&RWitness>,
    forwarded: Forwarded,
) -> Result<RPublic<Word<Fq>>, Error> {
    let spec: &WrapSpec = &plan.spec;
    let plans: Vec<&DeciderV2Plan<Eq>> = plan.a_plans.iter().map(|p| &**p).collect();
    let domain = &spec.domains.a_fold;
    let last = spec.last;
    let last_shape = spec.a[last].shape();
    let helper_count: usize = plans.iter().map(|p| p.helper_count()).sum();
    if witness.is_some_and(|w| {
        w.state.len() != spec.q_state
            || w.accs.len() != spec.a.len()
            || w.accs
                .iter()
                .enumerate()
                .any(|(k, acc)| acc.is_none() && k != last)
            || w.fresh.commitments.len() != last_shape.columns
            || w.fresh
                .public
                .iter()
                .map(Vec::len)
                .ne(last_shape.instance_lengths.iter().copied())
            || w.helpers.len() != helper_count
    }) {
        return Err(Error::Synthesis);
    }
    // The kept accumulators and their digests; an absent last kind is a
    // placeholder that the start ignores.
    let mut kept = Vec::with_capacity(spec.a.len());
    let mut kept_digests = Vec::with_capacity(spec.a.len());
    for (kind, relation) in spec.a.iter().enumerate() {
        let value = witness.map(|w| {
            w.accs[kind]
                .clone()
                .unwrap_or_else(|| placeholder_accumulator::<Eq>(relation))
        });
        let acc = assign_accumulator(lanes, region, &relation.shape(), value.as_ref())?;
        kept_digests.push(accumulator_digest(lanes, region, domain, &acc)?);
        kept.push(acc);
    }
    let start = lanes.glue.boolean(
        region,
        known(witness.map(|w| w.accs[last].is_none()).as_ref()),
    )?;
    // A_n folds into (or starts) its kind's accumulator.
    let mut commitments = Vec::with_capacity(last_shape.columns);
    for column in 0..last_shape.columns {
        let point = witness.map(|w| Eq::from(w.fresh.commitments[column]));
        commitments.push(lanes.assign_point(region, known(point.as_ref()))?);
    }
    let mut columns = Vec::with_capacity(last_shape.instance_lengths.len());
    for (column, length) in last_shape.instance_lengths.iter().enumerate() {
        let mut cells = Vec::with_capacity(*length);
        for row in 0..*length {
            let value = witness.map(|w| w.fresh.public[column][row]);
            cells.push(lanes.assign_scalar(region, known(value.as_ref()))?);
        }
        columns.push(cells);
    }
    let fresh = InstanceCells {
        commitments,
        public: columns,
    };
    let proof = assign_proof(lanes, region, &last_shape, witness.map(|w| &w.fold))?;
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
    // absent kind) and the Vesta claim accumulator's kgwfcld1 (Omega's).
    let claim_digest = lanes
        .glue
        .witness(region, known(witness.map(|w| &w.claim_digest)))?;
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
    let passthrough = fresh
        .public
        .first()
        .and_then(|c| c.first())
        .ok_or(Error::Synthesis)?;
    GlueChip::assert_equal(region, passthrough.s6.lo().word(), hash.lo.word())?;
    GlueChip::assert_equal(region, passthrough.s6.hi().word(), hash.hi.word())?;
    // The decided accumulators: the kept ones with the last kind folded,
    // each with its digest, field terms and copy term under P's challenges.
    let mut decided = kept;
    decided[last] = folded;
    let challenge = |index: usize| witness.map(|w| w.copy_challenges[index]);
    let gamma = lanes.assign_scalar(region, known(challenge(0).as_ref()))?;
    let eta = lanes.assign_scalar(region, known(challenge(1).as_ref()))?;
    let mut members = Vec::with_capacity(decided.len());
    for (kind, (acc, member_plan)) in decided.iter().zip(&plans).enumerate() {
        let acc_digest = if kind == last {
            accumulator_digest(lanes, region, domain, acc)?
        } else {
            kept_digests[kind].clone()
        };
        let copy_terms = copy_terms_cells(
            lanes,
            region,
            member_plan,
            &acc.instance.public,
            &gamma,
            &eta,
        )?;
        let words = |lanes: &mut FoldLanes<'_, Eq>,
                     region: &mut Region<'_, Fq>,
                     cells: &[ScalarCell<Eq>]|
         -> Result<Vec<Word<Fq>>, Error> {
            cells
                .iter()
                .map(|cell| word_of(lanes.glue, region, cell))
                .collect()
        };
        let terms = AccumulatorTerms {
            theta: words(lanes, region, &acc.theta)?,
            r: words(lanes, region, &acc.r)?,
            beta: words(lanes, region, &acc.beta)?,
            e: word_of(lanes.glue, region, &acc.e)?,
            copy_terms: word_of(lanes.glue, region, &copy_terms)?,
        };
        members.push(RMember { acc_digest, terms });
    }
    // The A-side decider's commitment side and its opening.
    let mut helpers = Vec::with_capacity(helper_count);
    for index in 0..helper_count {
        let point = witness.map(|w| Eq::from(w.helpers[index]));
        helpers.push(lanes.assign_point(region, known(point.as_ref()))?);
    }
    let helper = helper_digest(lanes, region, domain, &helpers)?;
    let import = |index: usize| witness.map(|w| w.imports[index]);
    let field_digest = lanes.assign_scalar(region, known(import(0).as_ref()))?;
    let value = lanes.assign_scalar(region, known(import(1).as_ref()))?;
    let folded_eq = lanes.assign_scalar(region, known(import(2).as_ref()))?;
    let mu = plans.first().ok_or(Error::Synthesis)?.mu();
    let opening = assign_batch_opening(lanes, region, mu, witness.map(|w| &w.opening))?;
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
    let a_public = fresh
        .public
        .iter()
        .flatten()
        .map(|cell| word_of(lanes.glue, region, cell))
        .collect::<Result<Vec<_>, _>>()?;
    let rounds = group
        .claim
        .challenges
        .iter()
        .map(|cell| word_of(lanes.glue, region, cell))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RPublic {
        a_public,
        members,
        helper_digest: helper,
        copy_challenges: [
            word_of(lanes.glue, region, &gamma)?,
            word_of(lanes.glue, region, &eta)?,
        ],
        field_digest: word_of(lanes.glue, region, &field_digest)?,
        value: word_of(lanes.glue, region, &value)?,
        folded: word_of(lanes.glue, region, &folded_eq)?,
        alpha: word_of(lanes.glue, region, &alpha)?,
        rounds,
        claim: [
            group.claim.folded_generator.x().clone(),
            group.claim.folded_generator.y().clone(),
        ],
        claim_digest,
        q_digests: forwarded.q_digests,
        q_claim: forwarded.q_claim,
        q_crossing_digest: forwarded.q_crossing_digest,
        s_claim: forwarded.s_claim,
    })
}

/// Range buses of R's lanes.
pub const R_RANGE_BUSES: usize = 4;

/// R's columns: verifier lanes (R drives them as fold lanes), a byte tape for
/// S's proof and one public column.
#[derive(Clone, Debug)]
pub struct RConfig {
    verifier: VerifierConfig<Eq>,
    bytes: BytesConfig,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fq> for RCircuit {
    type Config = RConfig;
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

    fn configure(meta: &mut ConstraintSystem<Fq>) -> RConfig {
        Self::configure_with_params(meta, 0)
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, rows: usize) -> RConfig {
        // R keeps the slim profile although it verifies S: its foreign-field
        // kernel shares the glue ports, so P verifies fewer columns. Range
        // checks spread over a bank of buses.
        let verifier = VerifierConfig::configure_serialized_foreign(meta, R_RANGE_BUSES)
            .expect("fixed bus count");
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let instance = meta.instance_column(rows);
        meta.enable_equality(instance);
        RConfig {
            verifier,
            bytes,
            instance,
        }
    }

    fn synthesize(&self, config: RConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let plan = &*self.plan;
        let witness = self.witness.as_deref();
        let s_layout = &plan.s.layout;
        let s_proof_length = plan.s_verifier.proof_length();
        if witness.is_some_and(|w| {
            w.s_public.len() != s_layout.rows() || w.s_proof.len() != s_proof_length
        }) {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::<Eq>::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let rows = layouter.assign_region(
            || "fold wrap R",
            |mut region| {
                let region = &mut region;
                let proof = super::byte_elements(
                    &mut bytes,
                    &mut chip,
                    region,
                    &[(witness.map(|w| w.s_proof.as_slice()), s_proof_length)],
                )?;
                let s_length = chip
                    .uint()
                    .assign::<32>(region, Value::known(s_proof_length as u128))?;
                // S's public column as canonical Fp scalars, and S verified hard.
                let s_rows = (0..s_layout.rows())
                    .map(|i| {
                        ScalarCells::<Eq>::assign_value(
                            &mut chip.uint(),
                            region,
                            known(witness.map(|w| &w.s_public[i])),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let key = chip.constant_key(region, &plan.s_verifier, &plan.s_key)?;
                let verified = chip.verify(
                    region,
                    &plan.s_verifier,
                    &key,
                    core::slice::from_ref(&s_rows),
                    &proof,
                    &s_length,
                    VerificationMode::Hard,
                )?;
                let s_public = s_layout.parse(&s_rows)?;
                let refs = chip.fold_lanes()?;
                let mut lanes = FoldLanes {
                    glue: refs.glue,
                    range: refs.range,
                    ff: refs.ff,
                    ecc: refs.ecc,
                    duplex: refs.hash,
                };
                let forwarded = forward(&mut lanes, region, &s_public, &verified.claim)?;
                let public = r_side(&mut lanes, region, plan, witness, forwarded)?;
                Ok(public.rows())
            },
        )?;
        if rows.len() != plan.layout.rows() {
            return Err(Error::Synthesis);
        }
        for (row, word) in rows.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}
