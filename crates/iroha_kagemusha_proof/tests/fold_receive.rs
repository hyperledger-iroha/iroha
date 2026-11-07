//! The folded Receive A1 step (KAGEMUSHA-Fold, `fold_lineage`): the IVC frame,
//! the Receive's step context and stage A1's tasks (consumed and credit
//! effects, the own step relation) on genuine Receive transitions.
//!
//! Parity: on the accept and both burn fixtures of today's map component test
//! and on the same mutations, the folded step is satisfied exactly when
//! today's A1 task owners, laid out directly, accept. Beyond them the step binds
//! the A state (predecessor lineage, an empty context, the first sub-step),
//! and its instance and the Q accumulator it keeps decide. Heavy: k16
//! circuits; run optimized.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[allow(dead_code)]
mod common;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;
#[path = "common/receive_objects.rs"]
mod receive_objects;
#[path = "common/send_objects.rs"]
#[allow(dead_code)]
mod send_objects;

use std::sync::Arc;

use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    PrefixMode, RelationShape, SigmaRelation,
    a_relation::{
        AProofPlan, LineagePublicCells, QProofPlan,
        fold::receive::{ReceiveFoldPlan, ReceiveFoldSource, ReceiveFoldStep, ReceiveStepWitness},
        native::receive::{IncomingWitness, Transition},
        own::OwnPolicy,
        receive::{MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES, ReceiveStagePlan},
        schedule::compiled::OperationSchedule,
        transition::TransitionInputs,
    },
    admin_sigma::StateWitness,
    SigmaProver,
    a_relation::schedule::sigma_selector,
    fold_lineage::{
        A_CONTEXT, A_DIGESTS, A_PC, AKind, BASE, CLAIM_Q, FOLD_A, FOLD_Q, Q_CLAIM_SLOT, Q_DIGESTS, QFrameStep, QKind,
        sigma::{QSigmaStep, QSigmaStepWitness, fold_selected},
        signature::QSignatureStep,
        start_passthrough,
    },
    operation_relation::{
        map_effects::{CONSUMED_DOMAIN, InsertCells, MapEffectsChip, MapState, MapTransition, ReceiveMapWitness},
        state::StateCells,
        statement::StatementCells,
    },
    q_sigma::{
        QSigmaPlan, SigmaClass, SigmaSlotWitness,
        native::{IncomingMode, IncomingSigma},
    },
    q_signature::{QSignatureCircuit, QSignaturePlan, SignatureWitness},
    tree::IndexedInsert,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, poseidon::hash_with_domain};
use iroha_plonk::{
    DescriptorBinding,
    check::{CheckMode, check_circuit},
    cs::{
        CircuitDescriptorV1, CircuitDescriptorV2, Column, ConstraintSystem, CurveV1, DescriptorConfig, Instance,
        InstanceModeV1, InstanceType, ProofSuffixV1, TranscriptV1, TranscriptV2,
    },
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_vk_with_binding_v2},
    pcs::ipa::PinnedParams,
    transcript::decode_point,
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig, UintChip,
    imt::{LeafCells, OpeningCells, PathCells},
    p256::native::{self as p256, Affine, HALF_N, ORDER, verify_prehashed},
    sha256::native::sha256_of_digest,
    statement::STATEMENT_DOMAIN,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig, VerifierPlan},
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

const K: u32 = 16;

/// A circuit whose only content is public columns of the given lengths: its key
/// is a descriptor of a Q leaf or Omega program for the plan's schema.
#[derive(Clone)]
struct Export(Vec<usize>);

impl Circuit<Fq> for Export {
    type Config = (GlueConfig, Vec<Column<Instance>>);
    type Params = Vec<usize>;
    type FloorPlanner = SimpleFloorPlanner;
    fn params(&self) -> Vec<usize> {
        self.0.clone()
    }
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
        Self::configure_with_params(meta, vec![1])
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, lengths: Vec<usize>) -> Self::Config {
        let columns = core::array::from_fn(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let glue = GlueConfig::configure(meta, columns, fixed);
        let public = lengths
            .into_iter()
            .map(|n| {
                let column = meta.instance_column(n);
                meta.enable_equality(column);
                column
            })
            .collect();
        (glue, public)
    }
    fn synthesize(&self, (config, public): Self::Config, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config);
        let words = layouter.assign_region(
            || "descriptor export",
            |mut region| (0..self.0.iter().sum::<usize>()).map(|_| glue.witness(&mut region, Value::unknown())).collect::<Result<Vec<_>, _>>(),
        )?;
        let mut offset = 0;
        for (column, len) in public.into_iter().zip(&self.0) {
            for row in 0..*len {
                layouter.constrain_instance(words[offset + row].cell(), column, row)?;
            }
            offset += len;
        }
        Ok(())
    }
}

fn export_plan(params: &PinnedParams<Ep>, lengths: Vec<usize>, types: Vec<InstanceType>) -> (VerifierPlan<Ep>, iroha_plonk::VerifyingKey<Ep>) {
    let (binding, key) =
        keygen_vk_with_binding_v2(params, &Export(lengths), &KeygenConfigV2::pipa_r(types)).expect("descriptor key");
    (VerifierPlan::new(binding, params.clone()).expect("verifier plan"), key)
}

/// The incoming-only sigma plan over a k12 Vesta descriptor (only its schema is
/// used: A1 neither verifies sigma nor reads Q0).
fn sigma_plan() -> QSigmaPlan {
    let mut cs = ConstraintSystem::<Fp>::default();
    cs.instance_column(1);
    let layout = CircuitDescriptorV1::from_constraint_system(
        &cs.finalize(&[], false).expect("finalize"),
        DescriptorConfig {
            curve: CurveV1::Vesta,
            k: 12,
            transcript: TranscriptV1::KagemushaPoseidonRp57,
            instance_mode: InstanceModeV1::Direct,
            proof_suffix: ProofSuffixV1::FoldedGenerator,
        },
    )
    .expect("layout");
    let descriptor =
        CircuitDescriptorV2::from_layout(layout, TranscriptV2::KagemushaPoseidonRp57Base, vec![InstanceType::Bounded])
            .expect("descriptor");
    let verifier = VerifierPlan::new(
        DescriptorBinding::new_v2(descriptor).expect("binding"),
        PinnedParams::<Eq>::derive(12).expect("params"),
    )
    .expect("sigma verifier");
    let incoming = SigmaClass::new(verifier, vec![(2, Fq::from(12)), (3, Fq::from(13))]).expect("class");
    QSigmaPlan::incoming_only(incoming, &PinnedParams::<Eq>::derive(16).expect("params")).expect("sigma plan")
}

/// Today's Receive schedule over descriptor keys, pinned for folding.
fn plan() -> Arc<ReceiveFoldPlan> {
    let params = PinnedParams::<Ep>::derive(16).expect("params");
    let policy = OwnPolicy::new([31, 32], bootstrap_objects::key(23)).expect("policy");
    let sigma = sigma_plan();
    let (verifier, key) = export_plan(&params, sigma.instance_lengths().to_vec(), QSigmaPlan::instance_types().to_vec());
    let q0 = QProofPlan::new(verifier, key).expect("Q0 plan");
    let schema = ReceiveStagePlan::signature_schema_for(Variant::Receive, policy).expect("schema");
    let (verifier, key) = export_plan(&params, vec![schema.instance_length()], QSignaturePlan::instance_types().to_vec());
    let q1 = QProofPlan::new(verifier, key).expect("Q1 plan");
    let (omega, _) = export_plan(
        &params,
        vec![1, 2, 16],
        vec![InstanceType::Bounded, InstanceType::Field, InstanceType::Bounded],
    );
    let operation = AProofPlan::inline_with_incoming(Variant::Receive, sigma, vec![q0, q1], Some(omega), &params)
        .expect("operation");
    let specs = ReceiveStagePlan::context_specs(Variant::Receive, MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES).expect("specs");
    let context = OperationSchedule::for_variant(Variant::Receive).bind(operation, specs).expect("schedule");
    let stage = ReceiveStagePlan::new(context, policy).expect("stage plan");
    Arc::new(ReceiveFoldPlan::new(stage).expect("fold plan"))
}

/// The genuine Receive transition of today's map component test.
fn transition(valid: bool, insert: bool) -> (send_objects::SendFixture, receive_objects::ReceiveFixture) {
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    let send = send_objects::from_load(&loaded.successor);
    let (receiver, _, _) = receive_objects::receiver();
    let receive = receive_objects::from_send(&send, &StateWitness::from(&receiver), Fp::from(401), valid, insert);
    (send, receive)
}

fn trivial_claims() -> (FoldInput<Ep>, AccumulatorT<Eq>) {
    let p = decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).expect("point");
    let v = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).expect("point");
    (FoldInput::from_normalized(p, 16, [Fq::ONE; 16]).expect("claim"), AccumulatorT::new(v, [Fp::ONE; 16]).expect("claim"))
}

/// Incoming proposals for the accept or burn branch over the payer's lineage
/// fields: all four results and every mode Accept, or a failed result with
/// every mode Trivial.
fn incoming(valid: bool, public: [Fp; 18]) -> IncomingWitness {
    let (p, v) = trivial_claims();
    IncomingWitness {
        public,
        public_valid: true,
        pallas: AccumulatorT::new(*p.g(), *p.challenges()).expect("claim"),
        vesta: v.clone(),
        opening: p.clone(),
        results: if valid { [true; 4] } else { [false, true, true, true] },
        modes: [if valid { IncomingMode::Accept } else { IncomingMode::Trivial }; 4],
        pallas_corrections: [*p.g(); 2],
        vesta_correction: *v.g(),
    }
}

fn source(plan: &ReceiveFoldPlan, valid: bool, insert: bool) -> ReceiveFoldSource {
    let (send, fixture) = transition(valid, insert);
    let context = plan.stage().context();
    let commitments = context
        .object_specs()
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let i = u64::try_from(i).expect("small");
            if i == 3 {
                [fixture.payment, Fp::from(u64::from(spec.capacity)), Fp::from(203)]
            } else {
                [Fp::from(100 + i), Fp::from(u64::from(spec.capacity)), Fp::from(200 + i)]
            }
        })
        .collect();
    let q = core::array::from_fn(|index| {
        context
            .operation()
            .q(index)
            .expect("Q plan")
            .verifier()
            .binding()
            .descriptor()
            .instance_lengths
            .iter()
            .map(|n| vec![Fq::ZERO; *n as usize])
            .collect()
    });
    ReceiveFoldSource::new(
        plan,
        Transition {
            before: fixture.before,
            after: fixture.after,
            statement: fixture.statement,
            consumed: fixture.consumed,
            credit: fixture.credit,
            insert: fixture.insert,
        },
        send.statement,
        fixture.step,
        commitments,
        q,
        incoming(valid, send.before.lineage),
    )
    .expect("genuine source")
}

/// The fold relations: the A1 step's and the frame Q step's (whose start
/// passthrough fixes the A state the test segment starts from).
struct Relations {
    a1: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
    key_q: CommitKey<Ep>,
}

fn a1_step(plan: &Arc<ReceiveFoldPlan>, shape: FoldShape, witness: Option<ReceiveStepWitness>) -> ReceiveFoldStep {
    ReceiveFoldStep::new(plan.clone(), AKind::Receive1, shape, witness).expect("A1 step")
}

fn relations(plan: &Arc<ReceiveFoldPlan>) -> Relations {
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![2],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_relation = |shape: FoldShape| {
        FoldedRelation::<Fp>::from_synthesized(&synthesize(&a1_step(plan, shape, None), K, None).expect("A1 layout"))
            .expect("A1 relation")
    };
    let first = a_relation(placeholder);
    let q = q_relation(first.shape(), Fq::ZERO);
    let a1 = a_relation(q.shape());
    assert_eq!(a1.shape(), first.shape(), "an A step's shape does not depend on the shape it folds");
    eprintln!("FOLD_RECEIVE A1 relation {:?}", a1.summary());
    eprintln!("FOLD_RECEIVE Qframe relation {:?}", q.summary());
    Relations {
        a1,
        key_a: CommitKey::lagrange(PinnedParams::<Eq>::derive(K).expect("params").params()),
        key_q: CommitKey::lagrange(PinnedParams::<Ep>::derive(K).expect("params").params()),
    }
}

fn q_relation(a_shape: FoldShape, start: Fq) -> FoldedRelation<Fq> {
    let step = QFrameStep { shape: a_shape, start, witness: None };
    FoldedRelation::<Fq>::from_synthesized(&synthesize(&step, K, None).expect("Q layout")).expect("Q relation")
}

/// The segment `Q frame (step 1), A1 (step 2)` from the A state `base`: the A1
/// circuit and its public column, plus the Q accumulator A keeps after it.
struct Segment {
    circuit: ReceiveFoldStep,
    public: Vec<Vec<Fp>>,
    kept: iroha_fold::ivc::Kept<Ep>,
    q: FoldedRelation<Fq>,
}

fn build_segment(rel: &Relations, plan: &Arc<ReceiveFoldPlan>, source: &ReceiveFoldSource, base: &[Fp]) -> Segment {
    let mut rng = ChaCha20Rng::seed_from_u64(29);
    let start = start_passthrough(base);
    let q = q_relation(rel.a1.shape(), start);
    // The segment's first step has no predecessor (its slot is unused).
    let q_chain = QKind::Frame.chain(AKind::Receive5);
    let prepared =
        prepare_step(&q_chain, 1, &rel.a1, None, None, vec![], vec![Fq::ZERO; Q_DIGESTS], start).expect("prepare Q");
    let public = vec![step_public(&q_chain, 1, Fq::from(BASE), prepared.passthrough, &[], &prepared.digests, &[])];
    let step = QFrameStep { shape: rel.a1.shape(), start, witness: Some(prepared.witness) };
    assert!(check_circuit(&step, K, &public, CheckMode::Strict).expect("Q synthesis").is_satisfied());
    let (advice, public) = q.stage_witness(&synthesize(&step, K, Some(&public)).expect("Q witness")).expect("stage");
    let fresh = fresh_instance(&q, &rel.key_q, &FOLD_Q, advice, public, &mut rng).expect("Q instance");

    let a_chain = AKind::Receive1.chain();
    let prepared = prepare_step(&a_chain, 2, &q, None, Some(&fresh), base.to_vec(), vec![Fp::ZERO; A_DIGESTS], Fp::ZERO)
        .expect("prepare A1");
    // A refused state still lays out a consistent frame: the body must refuse it.
    let next = plan.next_state(AKind::Receive1, source, base).unwrap_or_else(|_| {
        let mut next = base.to_vec();
        next[A_CONTEXT] = plan.context_digest(source).expect("digest");
        next[A_PC] = Fp::ONE;
        next
    });
    let public = vec![step_public(&a_chain, 2, Fp::from(BASE), prepared.passthrough, &next, &prepared.digests, &[])];
    let witness = ReceiveStepWitness { source: Arc::new(source.clone()), frame: prepared.witness };
    Segment {
        circuit: a1_step(plan, q.shape(), Some(witness)),
        public,
        kept: prepared.kept.expect("A starts the Q accumulator"),
        q,
    }
}

fn satisfied(circuit: &impl Circuit<Fp, Params = ()>, public: &[Vec<Fp>]) -> bool {
    let report = check_circuit(circuit, K, public, CheckMode::Strict).expect("synthesis");
    if !report.is_satisfied() && std::env::var_os("FOLD_RECEIVE_FAILURES").is_some() {
        eprintln!("FOLD_RECEIVE failures: {:?}", &report.failures()[..report.failures().len().min(6)]);
    }
    report.is_satisfied()
}

/// Today's stage A1 tasks laid out directly on today's owners (the map effects
/// chip and the inline step relation), without plan, context or frame.
#[derive(Clone)]
struct TodayA1 {
    fixture: receive_objects::ReceiveFixture,
    statement: [Fp; 26],
    payment: Fp,
    verdict: bool,
}

#[derive(Clone, Debug)]
struct TodayConfig {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}

impl TodayA1 {
    fn insertion(uint: &mut UintChip<'_, Fp>, region: &mut Region<'_, Fp>, witness: &IndexedInsert<Fp>) -> Result<InsertCells, Error> {
        let leaf = witness.leaf;
        let words = uint
            .glue()
            .witnesses(region, &[leaf.key, leaf.value, leaf.next_key].map(Value::known))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let mut paths = Vec::new();
        for (index, siblings) in [(witness.leaf_slot, witness.leaf_siblings), (witness.slot, witness.slot_siblings)] {
            let index = uint.glue().witness(region, Value::known(Fp::from(u64::from(index))))?;
            let siblings = uint.glue().witnesses(region, &siblings.map(Value::known))?.try_into().map_err(|_| Error::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| Error::Synthesis)?;
        Ok(InsertCells { low: OpeningCells { leaf: LeafCells::from_words(words), path: low }, slot })
    }

    fn state(chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, witness: &StateWitness) -> Result<(StateCells, LineagePublicCells), Error> {
        let glue = |chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, values: &[Fp]| {
            chip.uint().glue().witnesses(region, &values.iter().map(|v| Value::known(*v)).collect::<Vec<_>>())
        };
        let core = glue(chip, region, &witness.core)?.try_into().map_err(|_| Error::Synthesis)?;
        let rest = glue(chip, region, &witness.rest)?.try_into().map_err(|_| Error::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = glue(chip, region, &witness.lineage)?.try_into().map_err(|_| Error::Synthesis)?;
        Ok((state, LineagePublicCells::constrain(&mut chip.uint(), region, &public)?))
    }
}

impl Circuit<Fp> for TodayA1 {
    type Config = TodayConfig;
    type Params = ();
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> TodayConfig {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 4).expect("verifier");
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        TodayConfig { verifier, public }
    }
    fn synthesize(&self, config: TodayConfig, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let native = self.fixture.step.evaluate(SigmaRelation::RECEIVE);
        let digest = layouter.assign_region(
            || "today's A1 tasks",
            |mut region| {
                let (before, pred) = Self::state(&mut chip, &mut region, &self.fixture.before)?;
                let (after, next) = Self::state(&mut chip, &mut region, &self.fixture.after)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &self.statement.map(Value::known))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(&mut chip, &mut region, Variant::Receive, &fields)?;
                let consumed = Self::insertion(&mut chip.uint(), &mut region, &self.fixture.consumed)?;
                let credit = Self::insertion(&mut chip.uint(), &mut region, &self.fixture.credit)?;
                let inserted_key = statement.fields()[17].clone();
                let inserted_value = chip.hash_words(
                    &mut region,
                    CONSUMED_DOMAIN,
                    &[inserted_key.clone(), statement.fields()[20].clone(), statement.fields()[9].clone()],
                )?;
                let insert = chip.uint().glue().boolean(&mut region, Value::known(self.fixture.insert))?;
                let payment_digest = chip.uint().glue().witness(&mut region, Value::known(self.payment))?;
                let verdict = chip.uint().glue().boolean(&mut region, Value::known(self.verdict))?;
                let witness = ReceiveMapWitness { consumed, credit, inserted_key, inserted_value, insert, payment_digest };
                let transition = MapTransition {
                    statement: &statement,
                    predecessor: MapState { state: &before, lineage: &pred },
                    successor: MapState { state: &after, lineage: &next },
                };
                let lanes = chip.operation_lanes()?;
                let mut maps = MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash);
                maps.receive_consumed(&mut region, &transition, &witness, &verdict)?;
                maps.receive_credit(&mut region, &transition, &witness, &verdict)?;
                TransitionInputs {
                    shape: RelationShape::new(SigmaRelation::RECEIVE, PrefixMode::Absorbed),
                    witness: Some((&self.fixture.step, &native)),
                }
                .constrain(&mut chip, &mut region, &statement)?;
                Ok(statement.digest().clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
    }
}

impl TodayA1 {
    fn accepts(&self) -> bool {
        let public = vec![vec![hash_with_domain(STATEMENT_DOMAIN, &self.statement)]];
        satisfied(self, &public)
    }
}

/// One mutation of the map component test, applied to both layouts.
#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Verdict,
    Burned,
    CreditRoot,
    CreditRoute,
    ConsumedRoute,
    Payment,
    CreditId,
}

const MUTATIONS: [Mutation; 8] = [
    Mutation::None,
    Mutation::Verdict,
    Mutation::Burned,
    Mutation::CreditRoot,
    Mutation::CreditRoute,
    Mutation::ConsumedRoute,
    Mutation::Payment,
    Mutation::CreditId,
];

fn mutate(mutation: Mutation, valid: bool, source: &mut ReceiveFoldSource, today: &mut TodayA1) {
    match mutation {
        Mutation::None => {}
        Mutation::Verdict => {
            source.incoming = incoming(!valid, source.incoming.public);
            today.verdict = !valid;
        }
        Mutation::Burned => {
            source.transition.after.lineage[14] += Fp::ONE;
            today.fixture.after.lineage[14] += Fp::ONE;
        }
        Mutation::CreditRoot => {
            source.transition.after.lineage[16] += Fp::ONE;
            today.fixture.after.lineage[16] += Fp::ONE;
        }
        Mutation::CreditRoute => {
            source.transition.credit.leaf_siblings[31] += Fp::ONE;
            today.fixture.credit.leaf_siblings[31] += Fp::ONE;
        }
        Mutation::ConsumedRoute => {
            source.transition.consumed.leaf_siblings[0] += Fp::ONE;
            today.fixture.consumed.leaf_siblings[0] += Fp::ONE;
        }
        Mutation::Payment => {
            source.commitments[3][0] += Fp::ONE;
            today.payment += Fp::ONE;
        }
        Mutation::CreditId => {
            source.transition.statement[17] += Fp::ONE;
            today.statement[17] += Fp::ONE;
        }
    }
}

#[test]
#[ignore = "k16 folded Receive A1 steps on genuine transitions; run optimized"]
fn folded_a1_accepts_exactly_what_todays_a1_owners_accept() {
    let plan = plan();
    let rel = relations(&plan);
    for (valid, insert) in [(true, true), (false, true), (false, false)] {
        let genuine = source(&plan, valid, insert);
        let (_, fixture) = transition(valid, insert);
        let today = TodayA1 { statement: fixture.statement, payment: fixture.payment, verdict: valid, fixture };
        let base = genuine.initial_state();
        for mutation in MUTATIONS {
            let (mut source, mut reference) = (genuine.clone(), today.clone());
            mutate(mutation, valid, &mut source, &mut reference);
            let started = std::time::Instant::now();
            let segment = build_segment(&rel, &plan, &source, &base);
            let folded = satisfied(&segment.circuit, &segment.public);
            let accepted = reference.accepts();
            eprintln!(
                "FOLD_RECEIVE_A1 valid={valid} insert={insert} mutation={mutation:?} folded={folded} today={accepted} {:.1}s",
                started.elapsed().as_secs_f64()
            );
            assert_eq!(folded, accepted, "valid={valid} insert={insert} mutation={mutation:?}");
            if matches!(mutation, Mutation::None) {
                assert!(folded, "the genuine transition is accepted");
            }
        }
    }
}

#[test]
#[ignore = "k16 folded Receive A1 step; run optimized"]
fn folded_a1_binds_the_a_state_and_decides() {
    let plan = plan();
    let rel = relations(&plan);
    let source = source(&plan, true, true);
    let base = source.initial_state();
    let segment = build_segment(&rel, &plan, &source, &base);
    assert!(satisfied(&segment.circuit, &segment.public));
    let synthesized = synthesize(&segment.circuit, K, Some(&segment.public)).expect("A1 witness");
    let rows = synthesized.tables.advice_assigned().iter().map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1)).max();
    eprintln!("FOLD_RECEIVE_A1 rows={rows:?}");
    // Key generation lays out exactly the same cells without a witness.
    let unknown = synthesize(&segment.circuit.without_witnesses(), K, None).expect("A1 layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(synthesized.tables.permutation(), unknown.tables.permutation());
    assert_eq!(synthesized.tables.advice_assigned(), unknown.tables.advice_assigned());
    // The A1 instance decides fresh, and so does the Q accumulator it started.
    let mut rng = ChaCha20Rng::seed_from_u64(31);
    let (advice, public) = rel.a1.stage_witness(&synthesized).expect("stage");
    let (instance, witness) =
        fresh_instance(&rel.a1, &rel.key_a, &FOLD_A, advice, public, &mut rng).expect("A1 instance");
    decide_fresh(&rel.a1, &rel.key_a, &FOLD_A, &instance, &witness).expect("A1 decides");
    let (acc, acc_witness) = &segment.kept;
    decide(&segment.q, &rel.key_q, acc, acc_witness).expect("the Q accumulator decides");
    // A1 opens the Receive: the A state must carry the predecessor's lineage, no
    // context and no sub-step done; any other state lays out a consistent
    // frame that the body refuses.
    for (case, position, delta) in [("lineage", 6, Fp::ONE), ("context", A_CONTEXT, Fp::from(5)), ("pc", A_PC, Fp::ONE)] {
        let mut wrong = base.clone();
        wrong[position] += delta;
        let segment = build_segment(&rel, &plan, &source, &wrong);
        assert!(!satisfied(&segment.circuit, &segment.public), "A1 accepted another {case}");
    }
}

/// The decider profile of the folded A1 relation (key polynomials and opened
/// commitments per accumulator), for the compression wraps' capacity budget.
#[test]
#[ignore = "k16 relation and decider plan; run optimized"]
fn folded_a1_decider_profile() {
    let plan = plan();
    let rel = relations(&plan);
    let key = CommitKey::<Eq>::monomial(PinnedParams::<Eq>::derive(K).expect("params").params());
    let decider = iroha_fold::decider_v2::DeciderV2Plan::new(rel.a1.clone(), &key).expect("decider plan");
    let (sources, keys, copy_columns, terms) = decider.sizes();
    let (multiplications, absorbed, (witness, keyed, helpers)) = decider.verifier_profile();
    let runs = decider.key_runs();
    let few = runs.iter().filter(|(count, _)| *count <= 16).count();
    let distinct = {
        let mut points: Vec<_> = decider.key_commitments().iter().map(|p| format!("{p:?}")).collect();
        points.sort_unstable();
        points.dedup();
        points.len()
    };
    eprintln!(
        "FOLD_RECEIVE_A1_DECIDER sources={sources} key_polys={keys} distinct_key_commitments={distinct} \
         keys_with_at_most_16_runs={few} copy_columns={copy_columns} terms={terms} \
         final_multiplications={multiplications} absorbed={absorbed} opened=(witness {witness}, key {keyed}, helper {helpers})"
    );
}

/// A genuine incoming Send sigma (k12) and the incoming-only plan over its
/// verifier; `valid = false` corrupts the first commitment encoding.
fn sigma_source(valid: bool) -> (QSigmaPlan, SigmaSlotWitness) {
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    let send = send_objects::from_load(&loaded.successor);
    let prover = SigmaProver::<Eq>::keygen_with_params(
        common::pinned_shape(common::folded(SigmaRelation::SEND), (12, 1)),
        common::vesta_params(12),
    )
    .expect("sigma key");
    let proof = prover.prove(&send.step, common::fresh_randomness()).expect("sigma proof");
    let mut bytes = proof.bytes;
    if !valid {
        bytes[..32].fill(0xff);
    }
    let verifier = prover.verifier();
    let plan = QSigmaPlan::incoming_only(
        SigmaClass::from_verifiers(&[(sigma_selector(3, 0).expect("selector"), &verifier)]).expect("class"),
        &common::vesta_params(16),
    )
    .expect("sigma plan");
    let witness = SigmaSlotWitness {
        key: prover.proving_key().vk().clone(),
        statement: proof.public.instance()[0],
        length: bytes.len().try_into().expect("length"),
        proof: bytes,
    };
    (plan, witness)
}

#[test]
#[ignore = "k16 folded Receive Q sigma steps on genuine Send sigmas; run optimized"]
fn folded_q_sigma_step_verifies_sigma_and_folds_its_claim() {
    let plan = plan();
    let rel = relations(&plan);
    let vparams = common::vesta_params(16);
    for (valid, mode) in [(true, IncomingMode::Accept), (false, IncomingMode::Trivial)] {
        let (sigma_plan, sigma) = sigma_source(valid);
        let folded = sigma_plan
            .prepare_folded(IncomingSigma { sigma, mode }, &vparams, &FoldConfig::default())
            .expect("prepared incoming slot");
        // One step without predecessor: the sigma business and the claim start
        // (the chain test folds A1 here).
        let chain = QKind::Sigma.chain(AKind::Receive1);
        let start = Fq::from(7);
        let prepared = prepare_step(&chain, 1, &rel.a1, None, None, vec![], vec![Fq::ZERO; Q_DIGESTS], start)
            .expect("prepare");
        let (claims, s, claim_proof) = fold_selected(None, &folded.selected).expect("claim start");
        let mut digests = prepared.digests.clone();
        digests[Q_CLAIM_SLOT] = claims.digest(&CLAIM_Q);
        let mut public = vec![step_public(&chain, 1, Fq::from(BASE), prepared.passthrough, &[], &digests, &[])];
        public.extend(folded.instances.clone());
        let step = QSigmaStep {
            leaf: folded.circuit.clone(),
            shape: rel.a1.shape(),
            start,
            witness: Some(QSigmaStepWitness { frame: prepared.witness, selected: folded.selected.clone(), claims: None, claim_proof }),
        };
        let satisfied = |public: &[Vec<Fq>]| {
            let report = check_circuit(&step, K, public, CheckMode::Strict).expect("synthesis");
            if !report.is_satisfied() && std::env::var_os("FOLD_RECEIVE_FAILURES").is_some() {
                eprintln!("FOLD_RECEIVE failures: {:?}", &report.failures()[..report.failures().len().min(6)]);
            }
            report.is_satisfied()
        };
        assert!(satisfied(&public), "valid={valid} mode={mode:?}");
        // The soft verdict and the mode are Q0's exports; the claim
        // accumulator decides.
        assert_eq!(public[4][0], Fq::from(u64::from(valid)));
        iroha_fold::claims::decide_claims(vparams.params().g(), &claims, &s).expect("the selected claim decides");
        for (column, row) in [(4, 0), (4, 1), (3, 0), (1, 0)] {
            let mut changed = public.clone();
            changed[column][row] += Fq::ONE;
            assert!(!satisfied(&changed), "valid={valid} column {column} row {row}");
        }
        let mut changed = public.clone();
        changed[0][1] += Fq::ONE;
        assert!(!satisfied(&changed), "another claim digest in h_out");
        if valid {
            let synthesized = synthesize(&step, K, Some(&public)).expect("witness");
            let rows = synthesized.tables.advice_assigned().iter().map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1)).max();
            let unknown = synthesize(&step.without_witnesses(), K, None).expect("layout");
            assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
            assert_eq!(synthesized.tables.permutation(), unknown.tables.permutation());
            let relation = FoldedRelation::<Fq>::from_synthesized(&unknown).expect("Q sigma relation");
            let key = CommitKey::<Ep>::monomial(PinnedParams::<Ep>::derive(K).expect("params").params());
            let decider = iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
            let (_, keys, copy_columns, _) = decider.sizes();
            eprintln!(
                "FOLD_RECEIVE_QSIGMA rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
                relation.summary()
            );
        }
    }
}

/// A genuine `SHA256withECDSA` signature over the 32-byte encoding of `digest`.
fn p256_signature(digest: u64, secret: u64, nonce: u64) -> SignatureWitness {
    let digest = Fp::from(digest);
    let hash = sha256_of_digest(&digest);
    let message =
        core::array::from_fn(|i| u64::from_be_bytes(hash[(3 - i) * 8..(4 - i) * 8].try_into().expect("eight bytes")));
    let secret = [secret, 0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321, 0x0123_4567];
    let nonce = [nonce, 0x0bad_cafe_dead_beef, 0x1357_9bdf_2468_ace0, 0x0765_4321];
    let key = p256::mul(&Affine::GENERATOR, &secret).expect("key");
    let r = ORDER.reduce_once(&p256::mul(&Affine::GENERATOR, &nonce).expect("nonce point").x);
    let mut s = ORDER.mul(&ORDER.inverse(&nonce), &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)));
    if p256::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(verify_prehashed(&message, &r, &s, &key));
    SignatureWitness { digest, key: [key.x, key.y], signature: [r, s] }
}

#[test]
#[ignore = "k16 folded Receive Q signature steps with genuine P-256 slots; run optimized"]
fn folded_q_signature_step_keeps_the_own_slot_hard_and_incoming_slots_soft() {
    let plan = plan();
    let rel = relations(&plan);
    let policy = OwnPolicy::new([31, 32], bootstrap_objects::key(23)).expect("policy");
    let schema = ReceiveStagePlan::signature_schema_for(Variant::Receive, policy).expect("schema");
    let own = 0x5eed_0001;
    let genuine = vec![p256_signature(0x1111, own, 0xa1), p256_signature(0x2222, 0x5eed_1000, 0xa2), p256_signature(0x3333, own, 0xa3)];
    let chain = QKind::Signature.chain(AKind::Receive3);
    let start = Fq::from(9);
    let step = |witnesses: Vec<SignatureWitness>, verdicts: Option<Vec<Fq>>| {
        let leaf = QSignatureCircuit::new(schema.clone(), witnesses.clone()).expect("leaf");
        let prepared =
            prepare_step(&chain, 1, &rel.a1, None, None, vec![], vec![Fq::ZERO; Q_DIGESTS], start).expect("prepare");
        let mut public = vec![step_public(&chain, 1, Fq::from(BASE), prepared.passthrough, &[], &prepared.digests, &[])];
        let mut column = match schema.native_instances(&witnesses) {
            Ok([column]) => column,
            Err(_) => vec![Fq::ZERO; schema.instance_length()],
        };
        if let Some(verdicts) = verdicts {
            for (slot, verdict) in verdicts.into_iter().enumerate() {
                column[(slot + 1) * iroha_kagemusha_proof::q_signature::SLOT_WORDS - 1] = verdict;
            }
        }
        public.push(column);
        (QSignatureStep { leaf, shape: rel.a1.shape(), start, frame: Some(prepared.witness) }, public)
    };
    let satisfied = |circuit: &QSignatureStep, public: &[Vec<Fq>]| {
        check_circuit(circuit, K, public, CheckMode::Strict).expect("synthesis").is_satisfied()
    };
    let (circuit, public) = step(genuine.clone(), None);
    assert!(satisfied(&circuit, &public), "genuine merged slots");
    // A malformed incoming slot is satisfiable only with its false soft verdict.
    let mut malformed = genuine.clone();
    malformed[1].signature[1][0] ^= 1;
    let (circuit, public) = step(malformed.clone(), None);
    assert_eq!(public[1][2 * iroha_kagemusha_proof::q_signature::SLOT_WORDS - 1], Fq::ZERO);
    assert!(satisfied(&circuit, &public), "soft incoming slot");
    let (circuit, public) = step(malformed, Some(vec![Fq::ONE; 3]));
    assert!(!satisfied(&circuit, &public), "a malformed incoming slot claimed valid");
    // No verdict satisfies a malformed own receipt.
    let mut forged = genuine.clone();
    forged[0].signature[1][0] ^= 1;
    for verdict in [Fq::ZERO, Fq::ONE] {
        let (circuit, public) = step(forged.clone(), Some(vec![verdict, Fq::ONE, Fq::ONE]));
        assert!(!satisfied(&circuit, &public), "a malformed own receipt with verdict {verdict:?}");
    }
    let (circuit, public) = step(genuine, None);
    let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
    let rows = synthesized.tables.advice_assigned().iter().map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1)).max();
    let unknown = synthesize(&circuit.without_witnesses(), K, None).expect("layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(synthesized.tables.permutation(), unknown.tables.permutation());
    let relation = FoldedRelation::<Fq>::from_synthesized(&unknown).expect("Q signature relation");
    let key = CommitKey::<Ep>::monomial(PinnedParams::<Ep>::derive(K).expect("params").params());
    let decider = iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
    let (_, keys, copy_columns, _) = decider.sizes();
    eprintln!("FOLD_RECEIVE_QSIGNATURE rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}", relation.summary());
}
