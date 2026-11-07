//! Capacity of the Fp wrap's heaviest part: the multilinear decider of a real
//! Q-step accumulator (genuine Receive signature leaf plus fold frame, folded
//! twice in the toy chain) together with its bound opening, in one k16 circuit
//! over `Fp`. The circuit must reach the native transcript digest and `G'_0`;
//! the test prints how far each lane got.

mod common;

use std::sync::Arc;

use common::{
    qstep::{QStep, leaf},
    step::{BASE, DOMAIN_A, DOMAIN_Q, IVC_A, K, Side, Step, business},
};
use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        decider_v2::{assign_decider_v2_proof, verify_decider_v2},
        mle::{OpeningKey, assign_batch_opening, verify_batch_bound},
        transcript as circuit_transcript,
        verify::assign_accumulator,
    },
    decider_v2::{self, DeciderV2Plan, DeciderV2Proof},
    fold::fresh_instance,
    ivc::{self, Fresh, Kept, embed, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, synthesize},
    pcs::{ipa::PinnedParams, mle::BatchOpening},
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// The Q and A step relations of the toy chain, committed under the monomial
/// generators (v2 deciders).
struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
    params_q: PinnedParams<Ep>,
}

fn relations() -> Relations {
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![3],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_relation = |shape: FoldShape| {
        let layout = synthesize(
            &Step::<Ep> {
                side: Side::A,
                shape,
                witness: None,
            },
            K,
            None,
        )
        .expect("A layout");
        FoldedRelation::<Fp>::from_synthesized(&layout).expect("A relation")
    };
    let first = a_relation(placeholder).shape();
    let q_step = QStep {
        leaf: leaf(0).without_witnesses(),
        shape: first.clone(),
        witness: None,
    };
    let layout = synthesize(&q_step, K, None).expect("Q layout");
    let q = FoldedRelation::<Fq>::from_synthesized(&layout).expect("Q relation");
    let a = a_relation(q.shape());
    assert_eq!(a.shape(), first);
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    Relations {
        key_q: CommitKey::monomial(params_q.params()),
        key_a: CommitKey::monomial(params_a.params()),
        q,
        a,
        params_q,
    }
}

/// Runs Q1, A2, Q3, A4 and returns the Q accumulator A keeps (Q1 and Q3).
fn real_q_accumulator(rel: &Relations) -> Kept<Ep> {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let q_chain = Side::Q.chain();
    let a_chain = Side::A.chain();
    let (mut q_state, mut a_state) = (Fq::from(BASE), Fp::from(BASE));
    let mut q_kept: Option<Kept<Eq>> = None;
    let mut a_kept: Option<Kept<Ep>> = None;
    let mut last_a: Option<Fresh<Eq>> = None;
    let mut last_q: Option<Fresh<Ep>> = None;
    let start = embed(&ivc::ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &[Fp::from(BASE)],
        Fp::ZERO,
    ));
    for j in 1..=4_u64 {
        if j % 2 == 1 {
            let prepared = prepare_step(
                &q_chain,
                j,
                &rel.a,
                q_kept.as_ref(),
                last_a.as_ref(),
                vec![q_state],
                start,
            )
            .expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            let leaf = leaf(j);
            let leaf_public =
                leaf.instances(&[true, true, true]).expect("leaf instances")[0].clone();
            let frame = step_public(
                &q_chain,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[next],
                prepared.digest,
                &[Fq::from(j)],
            );
            let circuit = QStep {
                leaf,
                shape: rel.a.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized =
                synthesize(&circuit, K, Some(&[frame, leaf_public])).expect("witness");
            let (advice, public) = rel.q.stage_witness(&synthesized).expect("stage witness");
            last_q = Some(
                fresh_instance(&rel.q, &rel.key_q, &DOMAIN_Q, advice, public, &mut rng)
                    .expect("fresh"),
            );
            q_kept = prepared.kept;
            q_state = next;
        } else {
            let prepared = prepare_step(
                &a_chain,
                j,
                &rel.q,
                a_kept.as_ref(),
                last_q.as_ref(),
                vec![a_state],
                Fp::ZERO,
            )
            .expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            let public = vec![step_public(
                &a_chain,
                j,
                Fp::from(BASE),
                prepared.passthrough,
                &[next],
                prepared.digest,
                &[Fp::from(j)],
            )];
            let circuit = Step::<Ep> {
                side: Side::A,
                shape: rel.q.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.a.stage_witness(&synthesized).expect("stage witness");
            last_a = Some(
                fresh_instance(&rel.a, &rel.key_a, &DOMAIN_A, advice, public, &mut rng)
                    .expect("fresh"),
            );
            a_kept = prepared.kept;
            a_state = next;
        }
    }
    a_kept.expect("A keeps the Q accumulator after A4")
}

thread_local! {
    static LANES: std::cell::Cell<[usize; 4]> = const { std::cell::Cell::new([0; 4]) };
    static DECIDER_LANES: std::cell::Cell<[usize; 4]> = const { std::cell::Cell::new([0; 4]) };
}

/// The Q decider and its opening over one accumulator, exposing the finished
/// transcript digest and `G'_0`.
#[derive(Clone)]
struct WrapPart {
    plan: Arc<DeciderV2Plan<Ep>>,
    k: usize,
    shape: FoldShape,
    key: OpeningKey<Ep>,
    acc: Option<Accumulator<Ep>>,
    proof: Option<DeciderV2Proof<Ep>>,
    opening: Option<BatchOpening<Ep>>,
}

#[derive(Clone, Debug)]
struct WrapConfig {
    fold: FoldVerifierConfig<Ep>,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fp> for WrapPart {
    type Config = WrapConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            plan: Arc::clone(&self.plan),
            k: self.k,
            shape: self.shape.clone(),
            key: self.key.clone(),
            acc: None,
            proof: None,
            opening: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> WrapConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        WrapConfig { fold, instance }
    }

    fn synthesize(&self, config: WrapConfig, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let (digest, generator) = layouter.assign_region(
            || "Q decider and opening",
            |mut region| {
                let mut lanes = chips.lanes();
                let acc =
                    assign_accumulator(&mut lanes, &mut region, &self.shape, self.acc.as_ref())?;
                let proof = assign_decider_v2_proof(
                    &mut lanes,
                    &mut region,
                    &[&self.plan],
                    self.proof.as_ref(),
                )?;
                let claims = verify_decider_v2(
                    &mut lanes,
                    &mut region,
                    &DOMAIN_Q,
                    &[(&self.plan, &acc)],
                    &proof,
                )?;
                DECIDER_LANES.with(|cell| {
                    cell.set([
                        lanes.ff.next_row(),
                        lanes.ecc.next_row(),
                        lanes.glue.next_row(),
                        lanes.range.next_row(),
                    ]);
                });
                let opening =
                    assign_batch_opening(&mut lanes, &mut region, self.k, self.opening.as_ref())?;
                let generator = verify_batch_bound(
                    &mut lanes,
                    &mut region,
                    &self.key,
                    &claims.point,
                    &claims.claims,
                    &opening,
                )?
                .folded_generator;
                let digest = circuit_transcript::finish(&mut lanes, &mut region)?;
                LANES.with(|cell| {
                    cell.set([
                        lanes.ff.next_row(),
                        lanes.ecc.next_row(),
                        lanes.glue.next_row(),
                        lanes.range.next_row(),
                    ]);
                });
                Ok((digest, generator))
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.instance, 0)?;
        layouter.constrain_instance(generator.x().cell(), config.instance, 1)?;
        layouter.constrain_instance(generator.y().cell(), config.instance, 2)
    }
}

#[test]
#[ignore = "real Q-step accumulator and a k16 decider circuit; run optimized"]
fn the_q_decider_and_its_opening_fit_one_k16_circuit() {
    let rel = relations();
    let (acc, witness) = real_q_accumulator(&rel);
    let plan = DeciderV2Plan::new(rel.q.clone(), &rel.key_q).expect("v2 plan");
    let (sources, keys, helpers, terms) = plan.sizes();
    println!(
        "Q v2 plan: {sources} tables, {keys} key polynomials, {helpers} copy helpers, {terms} terms, degree {}",
        plan.degree()
    );
    let mut proved = decider_v2::prove(&plan, &DOMAIN_Q, &acc, &witness).expect("v2 prove");
    let (opening, _) = decider_v2::open(rel.params_q.params(), &mut proved).expect("v2 open");
    let mut verified =
        decider_v2::verify(&plan, &DOMAIN_Q, &acc, &proved.proof).expect("v2 verify");
    let claim = decider_v2::verify_opening(rel.params_q.params(), &mut verified, &opening)
        .expect("opening verifies");
    claim
        .decide(&rel.params_q, iroha_pasta::msm::MemoryBudget::DEFAULT)
        .expect("claim decides");
    let k = opening.rounds.len();
    println!("Q opening: {} claims, k = {k}", verified.claims.len());
    let public = vec![verified.transcript.digest(), claim.g().x(), claim.g().y()];
    let circuit = WrapPart {
        plan: Arc::new(plan),
        k,
        shape: rel.q.shape(),
        key: OpeningKey::new(rel.params_q.params()),
        acc: Some(acc),
        proof: Some(proved.proof),
        opening: Some(opening),
    };
    let report =
        check_circuit(&circuit, 16, &[public], CheckMode::Strict).expect("synthesis at k16");
    let [ff, ecc, glue, range] = LANES.with(std::cell::Cell::get);
    println!("lanes at k16 (65,536 rows): FF {ff}, ECC {ecc}, glue {glue}, range {range}");
    let [ff, ecc, glue, range] = DECIDER_LANES.with(std::cell::Cell::get);
    println!(
        "  of which the decider before its opening: FF {ff}, ECC {ecc}, glue {glue}, range {range}"
    );
    assert!(
        report.is_satisfied(),
        "the Q decider and its opening reach the native digest and G'_0"
    );
}
