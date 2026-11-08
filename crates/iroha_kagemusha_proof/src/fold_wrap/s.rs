//! The first wrap S, over `Fp` (a Vesta proof that R verifies): the Q side's
//! commitment work, which does not fit in P next to its verification of R.
//!
//! S runs the Q-side decider's commitment side and bound opening over every Q
//! kind. It hashes each accumulator into its digest, computes each member's
//! copy term under Omega's copy challenges and the helpers' digest,
//! recomputes the claimed commitments and checks the opening's group part.
//! Its public column ([`SPublic`]) carries the Q crossing:
//! - S's own values: digests, field terms with their copy terms, `D_H`,
//!   `alpha`, `u` and the claim `G'_0`;
//! - Omega's values (copy challenges, `d_F`, `v*`, `b'`), which S imports.
//!
//! R verifies S and hashes the crossing into `D_SQ`, which reaches Omega
//! through P. Omega hashes its own values and compares, so every crossing
//! value is bound on both sides. R also forwards the member digests (P's
//! `h_out`) and the claim `(G'_0, u)` (P's `acc_P`). S verifies no proof, so
//! it uses the slim lane profile, which R verifies cheaply.

use std::sync::Arc;

use iroha_fold::{
    Accumulator,
    circuit::{
        FoldLanes, ScalarCell,
        decider_v2::{claim_commitments, helper_digest, opening_transcript},
        mle::{
            EndoAlpha, OpeningClaimCells, OpeningKey, assign_batch_opening,
            verify_batch_bound_group,
        },
        transcript as t,
        verify::{accumulator_digest, assign_accumulator},
    },
    decider_v2::DeciderV2Plan,
};
use iroha_pasta::{Ep, EpAffine, Fp, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::mle::BatchOpening,
};
use iroha_plonk_gadgets::Word;
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

use super::{
    K_U32, WrapError, WrapSpec, copy_terms_cells,
    crossing::{AccumulatorTerms, Pair, QCrossing, QMember, SLayout, SPublic},
};

/// S's fixed program: the Q kinds' decider plans, the opening's generators
/// and S's public column.
#[derive(Clone, Debug)]
pub struct SPlan {
    /// The chain's fixed parts.
    pub spec: Arc<WrapSpec>,
    /// Each Q kind's decider plan, in kind order.
    pub q_plans: Vec<Arc<DeciderV2Plan<Ep>>>,
    /// The Q opening's fixed generators.
    pub opening: OpeningKey<Ep>,
    /// S's public column.
    pub layout: SLayout,
}

impl SPlan {
    /// S's program for `spec`.
    ///
    /// # Errors
    /// [`WrapError::Input`] for a spec without Q kinds, or Q relations the
    /// decider does not admit at k16 or that read public values beyond their
    /// copies.
    pub fn new(spec: Arc<WrapSpec>) -> Result<Self, WrapError> {
        if spec.q.is_empty() {
            return Err(WrapError::Input);
        }
        let q_plans = spec
            .q
            .iter()
            .map(|r| {
                DeciderV2Plan::with_step_runs(r.clone(), &spec.key_q, spec.q_step_runs)
                    .map(Arc::new)
            })
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
        let layout = SLayout::new(
            &spec.q.iter().map(|r| r.shape()).collect::<Vec<_>>(),
            K_U32 as usize,
        );
        Ok(Self {
            opening: OpeningKey::new(spec.params_q.params()),
            layout,
            q_plans,
            spec,
        })
    }
}

/// The Q-side decider's witness in S: the accumulators of every Q kind, the
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

fn pair(cell: &ScalarCell<Ep>) -> Pair<Word<Fp>> {
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
    plan: &SPlan,
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

/// Range buses of S's lanes.
pub const S_RANGE_BUSES: usize = 2;

/// The first wrap S.
#[derive(Clone, Debug)]
pub struct SCircuit {
    /// The program.
    pub plan: Arc<SPlan>,
    /// The witness (`None`: key generation).
    pub witness: Option<Arc<QGroupWitness>>,
}

/// S's columns: verifier lanes (S drives them as fold lanes) and one public
/// column.
#[derive(Clone, Debug)]
pub struct SConfig {
    verifier: VerifierConfig<Ep>,
    instance: Column<Instance>,
}

impl Circuit<Fp> for SCircuit {
    type Config = SConfig;
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

    fn configure(meta: &mut ConstraintSystem<Fp>) -> SConfig {
        Self::configure_with_params(meta, 0)
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, rows: usize) -> SConfig {
        // S verifies no proof, so its foreign-field kernel shares the glue
        // ports: fewer columns for R to verify.
        let verifier = VerifierConfig::configure_serialized_foreign(meta, S_RANGE_BUSES)
            .expect("fixed bus count");
        let instance = meta.instance_column(rows);
        meta.enable_equality(instance);
        SConfig { verifier, instance }
    }

    fn synthesize(&self, config: SConfig, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let plan = &*self.plan;
        let witness = self.witness.as_deref();
        let mut chip = VerifierChip::<Ep>::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let rows = layouter.assign_region(
            || "fold wrap S",
            |mut region| {
                let refs = chip.fold_lanes()?;
                let mut lanes = FoldLanes {
                    glue: refs.glue,
                    range: refs.range,
                    ff: refs.ff,
                    ecc: refs.ecc,
                    duplex: refs.hash,
                };
                let (q, claim) = q_commitment_side(&mut lanes, &mut region, plan, witness)?;
                let public = SPublic {
                    q,
                    claim: [
                        claim.folded_generator.x().clone(),
                        claim.folded_generator.y().clone(),
                    ],
                };
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
