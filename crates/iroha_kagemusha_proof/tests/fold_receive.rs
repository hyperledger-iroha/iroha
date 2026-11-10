//! The folded Receive steps and chain (KAGEMUSHA-Fold, `fold_lineage`).
//!
//! Step tests on genuine fixtures: A1 parity with today's A1 owners (on the
//! accept and both burn fixtures of today's map component test and on the same
//! mutations, the folded step is satisfied exactly when today's owners, laid
//! out directly, accept), the Q Sigma and Q Signature leaves, and the A3, A4
//! and A5 bodies, and the segment rebase. The chain test rebases a segment on
//! the receiver's last Omega, folds one genuine Receive through it (ten
//! half-steps) and decides everything the chain leaves to the compression
//! (`support/fold_receive_chain.rs`). Heavy: k16 circuits; run optimized.

#[path = "support/fold_receive_chain.rs"]
mod fold_receive_chain;

use std::sync::Arc;

use ff::Field;
use fold_receive_chain::{
    FullReceive, HalfStep, K, ReceiveKeys, ReceiveRelations, SCHEDULE, active_commitment,
    bootstrap_objects, common, decide_chain, exact_commitment, framed, full_receive,
    full_receive_with, load_objects, p256_signature, payment_transcript, plan, receipt,
    receive_keys, receive_objects, receive_relations, run_chain, send_objects, signed_digest,
    trivial_claims, undecidable_claims,
};
use iroha_fold::{
    CommitKey, FoldedRelation,
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    PrefixMode, RelationShape, SigmaProver, SigmaRelation,
    a_relation::schedule::sigma_selector,
    a_relation::{
        LineagePublicCells,
        fold::{
            rebase::{RebaseFoldStep, RebaseSource, RebaseStepWitness},
            receive::{
                ReceiveFoldPlan, ReceiveFoldSource, ReceiveFoldStep, ReceiveStepWitness, pc_before,
            },
        },
        native::receive::{IncomingWitness, Transition},
        own::OwnPolicy,
        receive::ReceiveStagePlan,
        transition::TransitionInputs,
    },
    admin_sigma::StateWitness,
    fold_lineage::{
        A_CLAIM_SLOT, A_CONTEXT, A_DIGESTS, A_PC, A_STEP_RUNS, AKind, BASE, CLAIM_A, CLAIM_Q,
        ClaimFolds, FOLD_A, FOLD_Q, Q_CLAIM_SLOT, Q_DIGESTS, QFrameStep, QFrameStepWitness, QKind,
        claim_of, fold_claims, segment_base, segment_start,
        sigma::{QSigmaStep, QSigmaStepWitness, fold_selected},
        signature::QSignatureStep,
        start_passthrough, trivial_export, trivial_vesta,
    },
    operation_relation::{
        map_effects::{
            CONSUMED_DOMAIN, InsertCells, MapEffectsChip, MapState, MapTransition,
            ReceiveMapWitness,
        },
        state::StateCells,
        statement::StatementCells,
    },
    q_sigma::{
        QSigmaPlan, SigmaClass, SigmaSlotWitness,
        native::{IncomingMode, IncomingSigma},
    },
    q_signature::{QSignatureCircuit, SignatureWitness},
    tree::IndexedInsert,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, poseidon::hash_with_domain};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{
    UintChip,
    bytes::p_bytes_native,
    imt::{LeafCells, OpeningCells, PathCells},
    statement::STATEMENT_DOMAIN,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig},
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

/// The genuine Receive transition of today's map component test.
fn transition(
    valid: bool,
    insert: bool,
) -> (send_objects::SendFixture, receive_objects::ReceiveFixture) {
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    let send = send_objects::from_load(&loaded.successor);
    let (receiver, _, _) = receive_objects::receiver();
    let receive = receive_objects::from_send(
        &send,
        &StateWitness::from(&receiver),
        Fp::from(401),
        valid,
        insert,
    );
    (send, receive)
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
        results: if valid {
            [true; 4]
        } else {
            [false, true, true, true]
        },
        modes: [if valid {
            IncomingMode::Accept
        } else {
            IncomingMode::Trivial
        }; 4],
        pallas_corrections: [*p.g(); 2],
        vesta_correction: *v.g(),
    }
}

/// The originals of the folded steps' tests: a genuine own transition, the raw
/// Omega and sigma tapes with their exact commitments and combined digest, and
/// placeholder tapes for the objects the A3 and A4 owners decode.
fn source(plan: &ReceiveFoldPlan, valid: bool, insert: bool) -> ReceiveFoldSource {
    let (send, fixture) = transition(valid, insert);
    let context = plan.stage().context();
    let specs = context.object_specs();
    let omega: Vec<u8> = (0..200u32).map(|i| (i * 7 + 3) as u8).collect();
    let sigma: Vec<u8> = (0..100u32).map(|i| (i * 11 + 5) as u8).collect();
    let objects: [Vec<u8>; 9] = core::array::from_fn(|i| match i {
        4 => omega.clone(),
        5 => sigma.clone(),
        _ => vec![0; specs[i].capacity as usize],
    });
    let proof_digest = p_bytes_native(
        u64::from_le_bytes(*b"kgwprf_1"),
        &[framed(&omega), framed(&sigma)].concat(),
    );
    let sigma_digest = p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &framed(&sigma));
    let commitments = specs
        .iter()
        .enumerate()
        .map(|(i, spec)| match i {
            3 => [
                fixture.payment,
                Fp::from(u64::from(spec.capacity)),
                Fp::from(203),
            ],
            4 => active_commitment(*spec, proof_digest, &omega),
            5 => active_commitment(*spec, sigma_digest, &sigma),
            _ => {
                let i = u64::try_from(i).expect("small");
                [
                    Fp::from(100 + i),
                    Fp::from(u64::from(spec.capacity)),
                    Fp::from(200 + i),
                ]
            }
        })
        .collect();
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
        objects,
        commitments,
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

fn a1_step(
    plan: &Arc<ReceiveFoldPlan>,
    shape: FoldShape,
    witness: Option<ReceiveStepWitness>,
) -> ReceiveFoldStep {
    a_step(plan, AKind::Receive1, shape, witness)
}

fn a_step(
    plan: &Arc<ReceiveFoldPlan>,
    kind: AKind,
    shape: FoldShape,
    witness: Option<ReceiveStepWitness>,
) -> ReceiveFoldStep {
    ReceiveFoldStep::new(plan.clone(), kind, shape, witness).expect("A step")
}

/// The A state the circuit outputs from `base` (also for a refused base, so
/// that the frame stays consistent and only the body refuses it).
fn circuit_next(
    plan: &ReceiveFoldPlan,
    source: &ReceiveFoldSource,
    kind: AKind,
    base: &[Fp],
) -> Vec<Fp> {
    let mut next = if kind == AKind::Receive5 {
        source.transition.after.lineage.to_vec()
    } else {
        base[..18].to_vec()
    };
    if kind == AKind::Receive5 {
        next.extend([Fp::ZERO, Fp::ZERO]);
    } else {
        next.extend([
            plan.context_digest(source).expect("digest"),
            Fp::from(pc_before(kind) + 1),
        ]);
    }
    next
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
        FoldedRelation::<Fp>::from_synthesized(
            &synthesize(&a1_step(plan, shape, None), K, None).expect("A1 layout"),
        )
        .expect("A1 relation")
    };
    let first = a_relation(placeholder);
    let q = q_relation(first.shape());
    let a1 = a_relation(q.shape());
    assert_eq!(
        a1.shape(),
        first.shape(),
        "an A step's shape does not depend on the shape it folds"
    );
    eprintln!("FOLD_RECEIVE A1 relation {:?}", a1.summary());
    eprintln!("FOLD_RECEIVE Qframe relation {:?}", q.summary());
    Relations {
        a1,
        key_a: CommitKey::lagrange(PinnedParams::<Eq>::derive(K).expect("params").params()),
        key_q: CommitKey::lagrange(PinnedParams::<Ep>::derive(K).expect("params").params()),
    }
}

fn q_relation(a_shape: FoldShape) -> FoldedRelation<Fq> {
    let step = QFrameStep {
        shape: a_shape,
        witness: None,
    };
    FoldedRelation::<Fq>::from_synthesized(&synthesize(&step, K, None).expect("Q layout"))
        .expect("Q relation")
}

/// The segment `Q frame (step 1), A1 (step 2)` from the A state `base`: the A1
/// circuit and its public column, plus the Q accumulator A keeps after it.
struct Segment {
    circuit: ReceiveFoldStep,
    public: Vec<Vec<Fp>>,
    kept: iroha_fold::ivc::Kept<Ep>,
    q: FoldedRelation<Fq>,
}

/// The staged public columns of a Q step opening a segment, opened at the A
/// state `base` instead. Every segment opens at the segment base, which only a
/// Rebase leaves; a step test opens its own segment at the state its A step
/// requires by replacing the Q instance's passthrough (A's base hash over
/// `base`). That instance then no longer satisfies its Q relation, so the step
/// tests decide only A's side; the chain test decides both.
fn opened_at(base: &[Fp], mut public: Vec<Vec<Fq>>) -> Vec<Vec<Fq>> {
    public[0][0] = start_passthrough(base);
    public
}

fn build_segment(
    rel: &Relations,
    plan: &Arc<ReceiveFoldPlan>,
    source: &ReceiveFoldSource,
    base: &[Fp],
) -> Segment {
    build_segment_of(rel, plan, source, AKind::Receive1, base)
}

fn build_segment_of(
    rel: &Relations,
    plan: &Arc<ReceiveFoldPlan>,
    source: &ReceiveFoldSource,
    kind: AKind,
    base: &[Fp],
) -> Segment {
    let mut rng = ChaCha20Rng::seed_from_u64(29);
    let q = q_relation(rel.a1.shape());
    // The segment's first step has no predecessor (its slot is unused); its
    // placeholder carries the trivial export, whose claim starts Q's claims.
    let q_chain = QKind::Frame.chain(AKind::Receive5);
    let mut prepared = prepare_step(
        &q_chain,
        1,
        &rel.a1,
        None,
        None,
        vec![],
        vec![Fq::ZERO; Q_DIGESTS],
        segment_start(),
    )
    .expect("prepare Q");
    prepared.witness.fresh.public[1] = trivial_export();
    let selected = trivial_vesta();
    let (claims, _, folds) =
        fold_claims(&CLAIM_Q, None, &[claim_of(&selected)]).expect("Q claim fold");
    let mut digests = prepared.digests.clone();
    digests[Q_CLAIM_SLOT] = claims.digest(&CLAIM_Q);
    let public = vec![step_public(
        &q_chain,
        1,
        Fq::from(BASE),
        prepared.passthrough,
        &[],
        &digests,
        &[],
    )];
    let witness = QFrameStepWitness {
        frame: prepared.witness,
        selected,
        claims: folds,
    };
    let step = QFrameStep {
        shape: rel.a1.shape(),
        witness: Some(witness),
    };
    assert!(
        check_circuit(&step, K, &public, CheckMode::Strict)
            .expect("Q synthesis")
            .is_satisfied()
    );
    let (advice, public) = q
        .stage_witness(&synthesize(&step, K, Some(&public)).expect("Q witness"))
        .expect("stage");
    let fresh = fresh_instance(
        &q,
        &rel.key_q,
        &FOLD_Q,
        advice,
        opened_at(base, public),
        &mut rng,
    )
    .expect("Q instance");

    let a_chain = kind.chain();
    let prepared = prepare_step(
        &a_chain,
        2,
        &q,
        None,
        Some(&fresh),
        base.to_vec(),
        vec![Fp::ZERO; A_DIGESTS],
        Fp::ZERO,
    )
    .expect("prepare A step");
    // A refused state still lays out a consistent frame: the body must refuse it.
    let next = plan
        .next_state(kind, source, base)
        .unwrap_or_else(|_| circuit_next(plan, source, kind, base));
    let (public, claims) = a_public(
        plan,
        kind,
        source,
        prepared.passthrough,
        &next,
        &prepared.digests,
    );
    let witness = ReceiveStepWitness {
        source: Arc::new(source.clone()),
        frame: prepared.witness,
        claims,
    };
    Segment {
        circuit: a_step(plan, kind, q.shape(), Some(witness)),
        public,
        kept: prepared.kept.expect("A starts the Q accumulator"),
        q,
    }
}

/// The public columns of an A step of `kind` at step 2 (the frame column over
/// its digest list, then its export column) and its claim folds: A5 folds the
/// incoming Pallas claims into A's claim accumulator, started here.
fn a_public(
    plan: &ReceiveFoldPlan,
    kind: AKind,
    source: &ReceiveFoldSource,
    passthrough: Fp,
    next: &[Fp],
    digests: &[Fp],
) -> (Vec<Vec<Fp>>, Option<ClaimFolds<Ep>>) {
    let mut digests = digests.to_vec();
    let claims = (kind == AKind::Receive5).then(|| {
        let claims = plan
            .incoming_pallas_claims(source)
            .expect("incoming Pallas claims")
            .map(|c| claim_of(&c));
        let (acc, _, folds) = fold_claims(&CLAIM_A, None, &claims).expect("A claim folds");
        digests[A_CLAIM_SLOT] = acc.digest(&CLAIM_A);
        folds
    });
    let frame = step_public(
        &kind.chain(),
        2,
        Fp::from(BASE),
        passthrough,
        next,
        &digests,
        &[],
    );
    (vec![frame, plan.export(kind, source)], claims)
}

fn satisfied(circuit: &impl Circuit<Fp, Params = ()>, public: &[Vec<Fp>]) -> bool {
    let report = check_circuit(circuit, K, public, CheckMode::Strict).expect("synthesis");
    if !report.is_satisfied() && std::env::var_os("FOLD_RECEIVE_FAILURES").is_some() {
        eprintln!(
            "FOLD_RECEIVE failures: {:?}",
            &report.failures()[..report.failures().len().min(6)]
        );
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
    fn insertion(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        witness: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, Error> {
        let leaf = witness.leaf;
        let words = uint
            .glue()
            .witnesses(
                region,
                &[leaf.key, leaf.value, leaf.next_key].map(Value::known),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let mut paths = Vec::new();
        for (index, siblings) in [
            (witness.leaf_slot, witness.leaf_siblings),
            (witness.slot, witness.slot_siblings),
        ] {
            let index = uint
                .glue()
                .witness(region, Value::known(Fp::from(u64::from(index))))?;
            let siblings = uint
                .glue()
                .witnesses(region, &siblings.map(Value::known))?
                .try_into()
                .map_err(|_| Error::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| Error::Synthesis)?;
        Ok(InsertCells {
            low: OpeningCells {
                leaf: LeafCells::from_words(words),
                path: low,
            },
            slot,
        })
    }

    fn state(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        witness: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), Error> {
        let glue = |chip: &mut VerifierChip<Ep>, region: &mut Region<'_, Fp>, values: &[Fp]| {
            chip.uint().glue().witnesses(
                region,
                &values.iter().map(|v| Value::known(*v)).collect::<Vec<_>>(),
            )
        };
        let core = glue(chip, region, &witness.core)?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let rest = glue(chip, region, &witness.rest)?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = glue(chip, region, &witness.lineage)?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        Ok((
            state,
            LineagePublicCells::constrain(&mut chip.uint(), region, &public)?,
        ))
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
        let verifier =
            VerifierConfig::configure_serialized_foreign_tagged(meta, 4).expect("verifier");
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        TodayConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: TodayConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
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
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Receive,
                    &fields,
                )?;
                let consumed =
                    Self::insertion(&mut chip.uint(), &mut region, &self.fixture.consumed)?;
                let credit = Self::insertion(&mut chip.uint(), &mut region, &self.fixture.credit)?;
                let inserted_key = statement.fields()[17].clone();
                let inserted_value = chip.hash_words(
                    &mut region,
                    CONSUMED_DOMAIN,
                    &[
                        inserted_key.clone(),
                        statement.fields()[20].clone(),
                        statement.fields()[9].clone(),
                    ],
                )?;
                let insert = chip
                    .uint()
                    .glue()
                    .boolean(&mut region, Value::known(self.fixture.insert))?;
                let payment_digest = chip
                    .uint()
                    .glue()
                    .witness(&mut region, Value::known(self.payment))?;
                let verdict = chip
                    .uint()
                    .glue()
                    .boolean(&mut region, Value::known(self.verdict))?;
                let witness = ReceiveMapWitness {
                    consumed,
                    credit,
                    inserted_key,
                    inserted_value,
                    insert,
                    payment_digest,
                };
                let transition = MapTransition {
                    statement: &statement,
                    predecessor: MapState {
                        state: &before,
                        lineage: &pred,
                    },
                    successor: MapState {
                        state: &after,
                        lineage: &next,
                    },
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
        let today = TodayA1 {
            statement: fixture.statement,
            payment: fixture.payment,
            verdict: valid,
            fixture,
        };
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
            assert_eq!(
                folded, accepted,
                "valid={valid} insert={insert} mutation={mutation:?}"
            );
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
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    eprintln!("FOLD_RECEIVE_A1 rows={rows:?}");
    // Key generation lays out exactly the same cells without a witness.
    let unknown = synthesize(&segment.circuit.without_witnesses(), K, None).expect("A1 layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        synthesized.tables.permutation(),
        unknown.tables.permutation()
    );
    assert_eq!(
        synthesized.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    // The A1 instance decides fresh, and so does the Q accumulator it started.
    let mut rng = ChaCha20Rng::seed_from_u64(31);
    let (advice, public) = rel.a1.stage_witness(&synthesized).expect("stage");
    let (instance, witness) =
        fresh_instance(&rel.a1, &rel.key_a, &FOLD_A, advice, public, &mut rng)
            .expect("A1 instance");
    decide_fresh(&rel.a1, &rel.key_a, &FOLD_A, &instance, &witness).expect("A1 decides");
    // At the segment base the Frame instance opening the segment is genuine and
    // the Q accumulator A1 starts decides, but A1 refuses that state: only a
    // Rebase follows a segment's first instance.
    let at_base = build_segment(&rel, &plan, &source, &segment_base());
    let (acc, acc_witness) = &at_base.kept;
    decide(&at_base.q, &rel.key_q, acc, acc_witness).expect("the Q accumulator decides");
    assert!(
        !satisfied(&at_base.circuit, &at_base.public),
        "A1 accepted the segment base"
    );
    // A1 opens the Receive: the A state must carry the predecessor's lineage, no
    // context and no sub-step done; any other state lays out a consistent
    // frame that the body refuses.
    for (case, position, delta) in [
        ("lineage", 6, Fp::ONE),
        ("context", A_CONTEXT, Fp::from(5)),
        ("pc", A_PC, Fp::ONE),
    ] {
        let mut wrong = base.clone();
        wrong[position] += delta;
        let segment = build_segment(&rel, &plan, &source, &wrong);
        assert!(
            !satisfied(&segment.circuit, &segment.public),
            "A1 accepted another {case}"
        );
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
    let decider =
        iroha_fold::decider_v2::DeciderV2Plan::new(rel.a1.clone(), &key).expect("decider plan");
    let (sources, keys, copy_columns, terms) = decider.sizes();
    let (multiplications, absorbed, (witness, keyed, helpers)) = decider.verifier_profile();
    let runs = decider.key_runs();
    let few = runs.iter().filter(|(count, _)| *count <= 16).count();
    let distinct = {
        let mut points: Vec<_> = decider
            .key_commitments()
            .iter()
            .map(|p| format!("{p:?}"))
            .collect();
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
    let proof = prover
        .prove(&send.step, common::fresh_randomness())
        .expect("sigma proof");
    let mut bytes = proof.bytes;
    if !valid {
        bytes[..32].fill(0xff);
    }
    let verifier = prover.verifier();
    let plan = QSigmaPlan::incoming_only(
        SigmaClass::from_verifiers(&[(sigma_selector(3, 0).expect("selector"), &verifier)])
            .expect("class"),
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
            .prepare_folded(
                IncomingSigma { sigma, mode },
                &vparams,
                &FoldConfig::default(),
            )
            .expect("prepared incoming slot");
        // One step without predecessor: the sigma business and the claim start
        // (the chain test folds A1 here). The placeholder A1 instance carries
        // A1's export of a Receive with trivial transported claims.
        let chain = QKind::Sigma.chain(AKind::Receive1);
        let start = segment_start();
        let mut prepared = prepare_step(
            &chain,
            1,
            &rel.a1,
            None,
            None,
            vec![],
            vec![Fq::ZERO; Q_DIGESTS],
            start,
        )
        .expect("prepare");
        let receive = source(&plan, true, true);
        prepared.witness.fresh.public[1] = plan.export(AKind::Receive1, &receive);
        let incoming = plan.incoming_vesta_claim(&receive).expect("incoming acc_V");
        let (claims, s, folds) =
            fold_selected(None, &folded.selected, &incoming).expect("claim folds");
        let mut digests = prepared.digests.clone();
        digests[Q_CLAIM_SLOT] = claims.digest(&CLAIM_Q);
        let mut public = vec![step_public(
            &chain,
            1,
            Fq::from(BASE),
            prepared.passthrough,
            &[],
            &digests,
            &[],
        )];
        public.extend(folded.instances.clone());
        let step = QSigmaStep {
            leaf: folded.circuit.clone(),
            shape: rel.a1.shape(),
            witness: Some(QSigmaStepWitness {
                frame: prepared.witness,
                selected: folded.selected.clone(),
                incoming: incoming.clone(),
                claims: folds,
            }),
        };
        let satisfied = |public: &[Vec<Fq>]| {
            let report = check_circuit(&step, K, public, CheckMode::Strict).expect("synthesis");
            if !report.is_satisfied() && std::env::var_os("FOLD_RECEIVE_FAILURES").is_some() {
                eprintln!(
                    "FOLD_RECEIVE failures: {:?}",
                    &report.failures()[..report.failures().len().min(6)]
                );
            }
            report.is_satisfied()
        };
        assert!(satisfied(&public), "valid={valid} mode={mode:?}");
        // The soft verdict and the mode are Q0's exports; the claim
        // accumulator decides.
        assert_eq!(public[4][0], Fq::from(u64::from(valid)));
        iroha_fold::claims::decide_claims(vparams.params().g(), &claims, &s)
            .expect("sigma's and the incoming claim decide");
        for (column, row) in [(4, 0), (4, 1), (3, 0), (1, 0)] {
            let mut changed = public.clone();
            changed[column][row] += Fq::ONE;
            assert!(
                !satisfied(&changed),
                "valid={valid} column {column} row {row}"
            );
        }
        let mut changed = public.clone();
        changed[0][1] += Fq::ONE;
        assert!(!satisfied(&changed), "another claim digest in h_out");
        // Folding a claim other than the selection of A1's export (its
        // generator with other challenges) is refused, even with a consistent
        // claim digest.
        let other_claim =
            FoldInput::from_normalized(*incoming.g(), 16, [Fp::from(5); 16]).expect("claim");
        let (other, _, other_folds) =
            fold_selected(None, &folded.selected, &other_claim).expect("claim folds");
        let mut other_digests = digests.clone();
        other_digests[Q_CLAIM_SLOT] = other.digest(&CLAIM_Q);
        let mut other_public = public.clone();
        other_public[0] = step_public(
            &chain,
            1,
            Fq::from(BASE),
            prepared.passthrough,
            &[],
            &other_digests,
            &[],
        );
        let mut other_step = step.clone();
        if let Some(witness) = other_step.witness.as_mut() {
            witness.incoming = other_claim;
            witness.claims = other_folds;
        }
        assert!(
            !check_circuit(&other_step, K, &other_public, CheckMode::Strict)
                .expect("synthesis")
                .is_satisfied(),
            "valid={valid}: Q Sigma folded a claim other than A1's export"
        );
        if valid {
            let synthesized = synthesize(&step, K, Some(&public)).expect("witness");
            let rows = synthesized
                .tables
                .advice_assigned()
                .iter()
                .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
                .max();
            let unknown = synthesize(&step.without_witnesses(), K, None).expect("layout");
            assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
            assert_eq!(
                synthesized.tables.permutation(),
                unknown.tables.permutation()
            );
            let relation =
                FoldedRelation::<Fq>::from_synthesized(&unknown).expect("Q sigma relation");
            let key =
                CommitKey::<Ep>::monomial(PinnedParams::<Ep>::derive(K).expect("params").params());
            let decider = iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key)
                .expect("decider plan");
            let (_, keys, copy_columns, _) = decider.sizes();
            eprintln!(
                "FOLD_RECEIVE_QSIGMA rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
                relation.summary()
            );
        }
    }
}

#[test]
#[ignore = "k16 folded Receive Q signature steps with genuine P-256 slots; run optimized"]
fn folded_q_signature_step_keeps_the_own_slot_hard_and_incoming_slots_soft() {
    let plan = plan();
    let rel = relations(&plan);
    let policy = OwnPolicy::new([31, 32], bootstrap_objects::key(23)).expect("policy");
    let schema = ReceiveStagePlan::signature_schema_for(Variant::Receive, policy).expect("schema");
    let own = 0x5eed_0001;
    let genuine = vec![
        p256_signature(0x1111, own, 0xa1),
        p256_signature(0x2222, 0x5eed_1000, 0xa2),
        p256_signature(0x3333, own, 0xa3),
    ];
    let chain = QKind::Signature.chain(AKind::Receive3);
    let start = segment_start();
    let step = |witnesses: Vec<SignatureWitness>, verdicts: Option<Vec<Fq>>| {
        let leaf = QSignatureCircuit::new(schema.clone(), witnesses.clone()).expect("leaf");
        let prepared = prepare_step(
            &chain,
            1,
            &rel.a1,
            None,
            None,
            vec![],
            vec![Fq::ZERO; Q_DIGESTS],
            start,
        )
        .expect("prepare");
        let mut public = vec![step_public(
            &chain,
            1,
            Fq::from(BASE),
            prepared.passthrough,
            &[],
            &prepared.digests,
            &[],
        )];
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
        (
            QSignatureStep {
                leaf,
                shape: rel.a1.shape(),
                frame: Some(prepared.witness),
            },
            public,
        )
    };
    let satisfied = |circuit: &QSignatureStep, public: &[Vec<Fq>]| {
        check_circuit(circuit, K, public, CheckMode::Strict)
            .expect("synthesis")
            .is_satisfied()
    };
    let (circuit, public) = step(genuine.clone(), None);
    assert!(satisfied(&circuit, &public), "genuine merged slots");
    // A malformed incoming slot is satisfiable only with its false soft verdict.
    let mut malformed = genuine.clone();
    malformed[1].signature[1][0] ^= 1;
    let (circuit, public) = step(malformed.clone(), None);
    assert_eq!(
        public[1][2 * iroha_kagemusha_proof::q_signature::SLOT_WORDS - 1],
        Fq::ZERO
    );
    assert!(satisfied(&circuit, &public), "soft incoming slot");
    let (circuit, public) = step(malformed, Some(vec![Fq::ONE; 3]));
    assert!(
        !satisfied(&circuit, &public),
        "a malformed incoming slot claimed valid"
    );
    // No verdict satisfies a malformed own receipt.
    let mut forged = genuine.clone();
    forged[0].signature[1][0] ^= 1;
    for verdict in [Fq::ZERO, Fq::ONE] {
        let (circuit, public) = step(forged.clone(), Some(vec![verdict, Fq::ONE, Fq::ONE]));
        assert!(
            !satisfied(&circuit, &public),
            "a malformed own receipt with verdict {verdict:?}"
        );
    }
    let (circuit, public) = step(genuine, None);
    let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    let unknown = synthesize(&circuit.without_witnesses(), K, None).expect("layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        synthesized.tables.permutation(),
        unknown.tables.permutation()
    );
    let relation = FoldedRelation::<Fq>::from_synthesized(&unknown).expect("Q signature relation");
    let key = CommitKey::<Ep>::monomial(PinnedParams::<Ep>::derive(K).expect("params").params());
    let decider =
        iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
    let (_, keys, copy_columns, _) = decider.sizes();
    eprintln!(
        "FOLD_RECEIVE_QSIGNATURE rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
        relation.summary()
    );
}

#[test]
#[ignore = "k16 folded Receive A5 steps on genuine transitions; run optimized"]
fn folded_a5_closes_the_receive_with_todays_terminal_owners() {
    let plan = plan();
    let rel = relations(&plan);
    for (valid, insert) in [(true, true), (false, true), (false, false)] {
        let genuine = source(&plan, valid, insert);
        let base = plan
            .state_before(AKind::Receive5, &genuine)
            .expect("A5 state");
        let segment = build_segment_of(&rel, &plan, &genuine, AKind::Receive5, &base);
        assert!(
            satisfied(&segment.circuit, &segment.public),
            "valid={valid} insert={insert}"
        );
        // A5 writes the successor's lineage and closes the context.
        let next = plan
            .next_state(AKind::Receive5, &genuine, &base)
            .expect("next");
        assert_eq!(next[..18], genuine.transition.after.lineage);
        assert_eq!(next[A_CONTEXT..], [Fp::ZERO, Fp::ZERO]);
        let cases: [(&str, Box<dyn Fn(&mut ReceiveFoldSource, &mut Vec<Fp>)>); 5] = [
            // Burned total belongs to A5's terminal effects (A1 let it pass).
            (
                "burned total",
                Box::new(|s: &mut ReceiveFoldSource, _: &mut Vec<Fp>| {
                    s.transition.after.lineage[14] += Fp::ONE
                }),
            ),
            (
                "consumed route",
                Box::new(|s: &mut ReceiveFoldSource, _: &mut Vec<Fp>| {
                    s.transition.consumed.leaf_siblings[0] += Fp::ONE
                }),
            ),
            (
                "Omega tape",
                Box::new(|s: &mut ReceiveFoldSource, _: &mut Vec<Fp>| s.objects[4][17] ^= 1),
            ),
            (
                "program counter",
                Box::new(|_: &mut ReceiveFoldSource, b: &mut Vec<Fp>| b[A_PC] = Fp::from(2)),
            ),
            (
                "context",
                Box::new(|_: &mut ReceiveFoldSource, b: &mut Vec<Fp>| b[A_CONTEXT] += Fp::ONE),
            ),
        ];
        for (case, mutate) in cases {
            let (mut source, mut base) = (genuine.clone(), base.clone());
            mutate(&mut source, &mut base);
            let segment = build_segment_of(&rel, &plan, &source, AKind::Receive5, &base);
            assert!(
                !satisfied(&segment.circuit, &segment.public),
                "valid={valid} insert={insert}: A5 accepted another {case}"
            );
        }
    }
    let genuine = source(&plan, true, true);
    let base = plan
        .state_before(AKind::Receive5, &genuine)
        .expect("A5 state");
    let segment = build_segment_of(&rel, &plan, &genuine, AKind::Receive5, &base);
    let synthesized = synthesize(&segment.circuit, K, Some(&segment.public)).expect("A5 witness");
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    let unknown = synthesize(&segment.circuit.without_witnesses(), K, None).expect("A5 layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        synthesized.tables.permutation(),
        unknown.tables.permutation()
    );
    let relation = FoldedRelation::<Fp>::from_synthesized(&unknown).expect("A5 relation");
    assert_eq!(
        relation.shape(),
        rel.a1.shape(),
        "every A kind shares one fold shape"
    );
    let key = CommitKey::<Eq>::monomial(PinnedParams::<Eq>::derive(K).expect("params").params());
    let decider =
        iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
    let (_, keys, copy_columns, _) = decider.sizes();
    eprintln!(
        "FOLD_RECEIVE_A5 rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
        relation.summary()
    );
}

/// A genuine signed-object Receive: the payer's Send receipt and Payment over
/// the actual Request and payer credential, the own receipt under the
/// receiver's payment key, the quoted current credential and its
/// certificate, Q1's merged signatures over them, and the transition bound to
/// that Payment.
fn signed_source(
    plan: &ReceiveFoldPlan,
    valid: bool,
    insert: bool,
) -> (ReceiveFoldSource, Vec<SignatureWitness>) {
    use iroha_kagemusha_proof::operation_relation::objects::ObjectKind;
    let (payer, _, _) = bootstrap_objects::enrollment();
    let (loaded, _, _) = load_objects::funded(&payer);
    let send = send_objects::from_load(&loaded.successor);
    let (receiver, certificate, credential) =
        bootstrap_objects::enrollment_for(bootstrap_objects::Identity::Receiver);
    let before = StateWitness::from(&receiver);
    let context = plan.stage().context();
    let specs = context.object_specs();
    let omega: Vec<u8> = (0..200u32).map(|i| (i * 7 + 3) as u8).collect();
    let sigma: Vec<u8> = (0..100u32).map(|i| (i * 11 + 5) as u8).collect();
    let proof_digest = p_bytes_native(
        u64::from_le_bytes(*b"kgwprf_1"),
        &[framed(&omega), framed(&sigma)].concat(),
    );
    let send_receipt = receipt(
        &send.statement,
        &send.before.lineage[6..8],
        proof_digest,
        Fp::ZERO,
        29,
        71,
    );
    let request = send.objects[1].clone();
    let payer_credential = send.objects[0].clone();
    let payment = payment_transcript(
        &request,
        &payer_credential,
        &send.statement,
        proof_digest,
        send_receipt.digest(),
    );
    let payment_digest = p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &payment);
    let fixture = receive_objects::from_send(&send, &before, payment_digest, valid, insert);
    let own_proof = p_bytes_native(
        u64::from_le_bytes(*b"kgwstep1"),
        &framed(b"retained own sigma"),
    );
    let own_receipt = receipt(
        &fixture.statement,
        &before.lineage[6..8],
        own_proof,
        payment_digest,
        43,
        73,
    );
    let request_signature = {
        let signed = bootstrap_objects::sign(
            ObjectKind::Request,
            request[..ObjectKind::Request.body_len()].to_vec(),
            43,
            59,
        );
        assert_eq!(signed.bytes, request);
        signed.signature
    };
    let signatures = vec![
        own_receipt.signature,
        send_receipt.signature,
        request_signature,
    ];
    let objects: [Vec<u8>; 9] = [
        request,
        payer_credential,
        send_receipt.bytes,
        payment,
        omega,
        sigma,
        own_receipt.bytes,
        credential.bytes,
        certificate.bytes,
    ];
    let kinds = [
        Some(ObjectKind::Request),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Receipt),
        None,
        None,
        None,
        Some(ObjectKind::Receipt),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
    ];
    let sigma_digest = p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &framed(&objects[5]));
    let commitments = specs
        .iter()
        .enumerate()
        .map(|(i, spec)| match (i, kinds[i]) {
            (_, Some(kind)) => {
                exact_commitment(*spec, signed_digest(kind, &objects[i]), &objects[i])
            }
            (3, None) => exact_commitment(*spec, payment_digest, &objects[i]),
            (4, None) => active_commitment(*spec, proof_digest, &objects[i]),
            (5, None) => active_commitment(*spec, sigma_digest, &objects[i]),
            _ => unreachable!("nine fixed slots"),
        })
        .collect();
    let source = ReceiveFoldSource::new(
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
        objects,
        commitments,
        incoming(valid, send.before.lineage),
    )
    .expect("genuine signed source");
    (source, signatures)
}

/// The segment `Q signature (step 1), A4 (step 2)` from the A state `base`:
/// the Q step carries Q1's leaf over `signatures`.
fn signature_segment(
    rel: &Relations,
    plan: &Arc<ReceiveFoldPlan>,
    source: &ReceiveFoldSource,
    signatures: &[SignatureWitness],
    base: &[Fp],
) -> Segment {
    let mut rng = ChaCha20Rng::seed_from_u64(37);
    let schema = plan
        .stage()
        .signature_schema(ReceiveStagePlan::SIGNATURE_Q)
        .expect("schema")
        .clone();
    let leaf = QSignatureCircuit::new(schema.clone(), signatures.to_vec()).expect("leaf");
    let q_chain = QKind::Signature.chain(AKind::Receive3);
    let prepared = prepare_step(
        &q_chain,
        1,
        &rel.a1,
        None,
        None,
        vec![],
        vec![Fq::ZERO; Q_DIGESTS],
        segment_start(),
    )
    .expect("prepare Q");
    // The Q step exports its leaf's own column; the context may claim another.
    let [column] = schema.native_instances(signatures).expect("Q1 column");
    let mut public = vec![step_public(
        &q_chain,
        1,
        Fq::from(BASE),
        prepared.passthrough,
        &[],
        &prepared.digests,
        &[],
    )];
    public.push(column);
    let step = QSignatureStep {
        leaf,
        shape: rel.a1.shape(),
        frame: Some(prepared.witness),
    };
    let q = FoldedRelation::<Fq>::from_synthesized(
        &synthesize(&step.without_witnesses(), K, None).expect("Q layout"),
    )
    .expect("Q signature relation");
    assert!(
        check_circuit(&step, K, &public, CheckMode::Strict)
            .expect("Q synthesis")
            .is_satisfied()
    );
    let (advice, public) = q
        .stage_witness(&synthesize(&step, K, Some(&public)).expect("Q witness"))
        .expect("stage");
    let fresh = fresh_instance(
        &q,
        &rel.key_q,
        &FOLD_Q,
        advice,
        opened_at(base, public),
        &mut rng,
    )
    .expect("Q instance");
    let kind = AKind::Receive4;
    let a_chain = kind.chain();
    let prepared = prepare_step(
        &a_chain,
        2,
        &q,
        None,
        Some(&fresh),
        base.to_vec(),
        vec![Fp::ZERO; A_DIGESTS],
        Fp::ZERO,
    )
    .expect("prepare A4");
    let next = plan
        .next_state(kind, source, base)
        .unwrap_or_else(|_| circuit_next(plan, source, kind, base));
    let (public, claims) = a_public(
        plan,
        kind,
        source,
        prepared.passthrough,
        &next,
        &prepared.digests,
    );
    let witness = ReceiveStepWitness {
        source: Arc::new(source.clone()),
        frame: prepared.witness,
        claims,
    };
    Segment {
        circuit: a_step(plan, kind, q.shape(), Some(witness)),
        public,
        kept: prepared.kept.expect("A starts U_Q"),
        q,
    }
}

#[test]
#[ignore = "k16 folded Receive A4 steps on genuine signed objects; run optimized"]
fn folded_a4_authorizes_the_own_receipt_and_checks_incoming_signatures() {
    let plan = plan();
    let rel = relations(&plan);
    for (valid, insert) in [(true, true), (false, true)] {
        let (genuine, signatures) = signed_source(&plan, valid, insert);
        let base = plan
            .state_before(AKind::Receive4, &genuine)
            .expect("A4 state");
        let segment = signature_segment(&rel, &plan, &genuine, &signatures, &base);
        assert!(
            satisfied(&segment.circuit, &segment.public),
            "valid={valid} insert={insert}"
        );
        // Another own receipt byte, Request byte or Send receipt byte breaks a
        // signed digest the context or Q1 holds, and another program counter
        // is refused.
        for (case, index, offset) in [
            ("own receipt", 6, 40),
            ("Request", 0, 10),
            ("Send receipt", 2, 40),
        ] {
            let mut source = genuine.clone();
            source.objects[index][offset] ^= 1;
            let segment = signature_segment(&rel, &plan, &source, &signatures, &base);
            assert!(
                !satisfied(&segment.circuit, &segment.public),
                "valid={valid}: A4 accepted another {case}"
            );
        }
        // Signatures over other messages: the Q step exports them, and A4's
        // owners refuse slots that do not sign the context's objects.
        let mut other = signatures.clone();
        other[0] = p256_signature(0x7777, 43, 0x99);
        let segment = signature_segment(&rel, &plan, &genuine, &other, &base);
        assert!(
            !satisfied(&segment.circuit, &segment.public),
            "valid={valid}: A4 accepted a Q1 that signs other messages"
        );
        let mut wrong = base.clone();
        wrong[A_PC] = Fp::ONE;
        let segment = signature_segment(&rel, &plan, &genuine, &signatures, &wrong);
        assert!(
            !satisfied(&segment.circuit, &segment.public),
            "valid={valid}: A4 accepted another program counter"
        );
    }
    let (genuine, signatures) = signed_source(&plan, true, true);
    let base = plan
        .state_before(AKind::Receive4, &genuine)
        .expect("A4 state");
    let segment = signature_segment(&rel, &plan, &genuine, &signatures, &base);
    let synthesized = synthesize(&segment.circuit, K, Some(&segment.public)).expect("A4 witness");
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    let unknown = synthesize(&segment.circuit.without_witnesses(), K, None).expect("A4 layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        synthesized.tables.permutation(),
        unknown.tables.permutation()
    );
    let relation = FoldedRelation::<Fp>::from_synthesized(&unknown).expect("A4 relation");
    assert_eq!(
        relation.shape(),
        rel.a1.shape(),
        "every A kind shares one fold shape"
    );
    let key = CommitKey::<Eq>::monomial(PinnedParams::<Eq>::derive(K).expect("params").params());
    let decider =
        iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
    let (_, keys, copy_columns, _) = decider.sizes();
    eprintln!(
        "FOLD_RECEIVE_A4 rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
        relation.summary()
    );
}

/// The segment `Q sigma (step 1), A3 (step 2)` from the A state `base`: the Q
/// step verifies the real sigma and starts Q's claim accumulator with sigma's
/// claim and the incoming `acc_V` (its placeholder A1 instance carries A1's
/// export of `source`).
fn sigma_segment(
    rel: &Relations,
    keys: &ReceiveKeys,
    full: &FullReceive,
    source: &ReceiveFoldSource,
    base: &[Fp],
) -> Segment {
    let mut rng = ChaCha20Rng::seed_from_u64(43);
    let plan = &keys.plan;
    let q_chain = QKind::Sigma.chain(AKind::Receive1);
    let mut prepared = prepare_step(
        &q_chain,
        1,
        &rel.a1,
        None,
        None,
        vec![],
        vec![Fq::ZERO; Q_DIGESTS],
        segment_start(),
    )
    .expect("prepare Q");
    prepared.witness.fresh.public[1] = plan.export(AKind::Receive1, source);
    let incoming = plan.incoming_vesta_claim(source).expect("incoming acc_V");
    let (claims, _, folds) =
        fold_selected(None, &full.sigma.selected, &incoming).expect("claim folds");
    let mut digests = prepared.digests.clone();
    digests[Q_CLAIM_SLOT] = claims.digest(&CLAIM_Q);
    let mut public = vec![step_public(
        &q_chain,
        1,
        Fq::from(BASE),
        prepared.passthrough,
        &[],
        &digests,
        &[],
    )];
    public.extend(full.sigma.instances.clone());
    let witness = QSigmaStepWitness {
        frame: prepared.witness,
        selected: full.sigma.selected.clone(),
        incoming,
        claims: folds,
    };
    let step = QSigmaStep {
        leaf: full.sigma.circuit.clone(),
        shape: rel.a1.shape(),
        witness: Some(witness),
    };
    let q = FoldedRelation::<Fq>::from_synthesized(
        &synthesize(&step.without_witnesses(), K, None).expect("Q layout"),
    )
    .expect("Q sigma relation");
    assert!(
        check_circuit(&step, K, &public, CheckMode::Strict)
            .expect("Q synthesis")
            .is_satisfied()
    );
    let (advice, public) = q
        .stage_witness(&synthesize(&step, K, Some(&public)).expect("Q witness"))
        .expect("stage");
    let fresh = fresh_instance(
        &q,
        &rel.key_q,
        &FOLD_Q,
        advice,
        opened_at(base, public),
        &mut rng,
    )
    .expect("Q instance");
    let kind = AKind::Receive3;
    let prepared = prepare_step(
        &kind.chain(),
        2,
        &q,
        None,
        Some(&fresh),
        base.to_vec(),
        vec![Fp::ZERO; A_DIGESTS],
        Fp::ZERO,
    )
    .expect("prepare A3");
    let next = plan
        .next_state(kind, source, base)
        .unwrap_or_else(|_| circuit_next(plan, source, kind, base));
    let (public, claims) = a_public(
        plan,
        kind,
        source,
        prepared.passthrough,
        &next,
        &prepared.digests,
    );
    let witness = ReceiveStepWitness {
        source: Arc::new(source.clone()),
        frame: prepared.witness,
        claims,
    };
    Segment {
        circuit: a_step(plan, kind, q.shape(), Some(witness)),
        public,
        kept: prepared.kept.expect("A starts U_Q"),
        q,
    }
}

#[test]
#[ignore = "k16 folded Receive A3 steps on a genuine incoming Omega and sigma; run optimized"]
fn folded_a3_verifies_the_incoming_omega_and_the_payment_objects() {
    let keys = receive_keys();
    let full = full_receive(&keys, None);
    let rel = relations(&keys.plan);
    let genuine = full.source.clone();
    let base = keys
        .plan
        .state_before(AKind::Receive3, &genuine)
        .expect("A3 state");
    let segment = sigma_segment(&rel, &keys, &full, &genuine, &base);
    assert!(
        satisfied(&segment.circuit, &segment.public),
        "genuine incoming Omega, sigma and objects"
    );
    // Another Omega proof byte, Payment byte or sigma byte breaks the soft
    // verdicts, the Payment semantics or the sigma binding to Q0's columns,
    // and another program counter is refused.
    for (case, index, offset) in [
        ("Omega proof", 4, 330),
        ("Payment", 3, 40),
        ("sigma", 5, 40),
    ] {
        let mut source = genuine.clone();
        source.objects[index][offset] ^= 1;
        let segment = sigma_segment(&rel, &keys, &full, &source, &base);
        assert!(
            !satisfied(&segment.circuit, &segment.public),
            "A3 accepted another {case}"
        );
    }
    let mut wrong = base.clone();
    wrong[A_PC] = Fp::from(2);
    let segment = sigma_segment(&rel, &keys, &full, &genuine, &wrong);
    assert!(
        !satisfied(&segment.circuit, &segment.public),
        "A3 accepted another program counter"
    );
    let synthesized = synthesize(&segment.circuit, K, Some(&segment.public)).expect("A3 witness");
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    let unknown = synthesize(&segment.circuit.without_witnesses(), K, None).expect("A3 layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    let relation = FoldedRelation::<Fp>::from_synthesized(&unknown).expect("A3 relation");
    assert_eq!(
        relation.shape(),
        rel.a1.shape(),
        "every A kind shares one fold shape"
    );
    let key = CommitKey::<Eq>::monomial(PinnedParams::<Eq>::derive(K).expect("params").params());
    let decider =
        iroha_fold::decider_v2::DeciderV2Plan::new(relation.clone(), &key).expect("decider plan");
    let (_, keys, copy_columns, _) = decider.sizes();
    eprintln!(
        "FOLD_RECEIVE_A3 rows={rows:?} relation={:?} decider_keys={keys} copy_columns={copy_columns}",
        relation.summary()
    );
}

/// The genuine segment: a Rebase on the receiver's last Omega, then one Receive
/// (ten half-steps), each checked strictly, committed and folded; everything
/// the chain leaves decides. Transported claims that do not decide (the
/// incoming Omega's or the predecessor's) pass every step and are refused only
/// by their curve's claim decider: `acc_P` by A's at the end of the chain,
/// `acc_V` by Q's after the step that folds it.
#[test]
#[ignore = "k16 folded segment chain (ten half-steps, decider v2); run optimized"]
fn a_rebased_segment_folds_a_receive_and_everything_it_leaves_decides() {
    let keys = receive_keys();
    let full = full_receive(&keys, None);
    let rel = receive_relations(&keys, &full);
    for (kind, relation) in AKind::ALL.iter().zip(&rel.a) {
        eprintln!("FOLD_RECEIVE_CHAIN A {kind:?} {:?}", relation.summary());
    }
    for (kind, relation) in QKind::ALL.iter().zip(&rel.q) {
        eprintln!("FOLD_RECEIVE_CHAIN Q {kind:?} {:?}", relation.summary());
    }
    let started = std::time::Instant::now();
    let chain = run_chain(&keys, &rel, &full, SCHEDULE.len());
    eprintln!(
        "FOLD_RECEIVE_CHAIN ten half-steps (checked, committed, folded) {:.1}s",
        started.elapsed().as_secs_f64()
    );
    // The Receive closes: the successor's lineage, no context, no sub-step.
    let mut expected = full.source.transition.after.lineage.to_vec();
    expected.extend([Fp::ZERO, Fp::ZERO]);
    assert_eq!(chain.a_state, expected);
    assert_eq!(chain.last, Some(HalfStep::A(AKind::Receive5)));
    // A keeps every Q kind and its claims; Q keeps A1, A3, A4, the Rebase and
    // its claims (A5 is the next fold's).
    assert!(chain.a_kept.iter().all(Option::is_some));
    let kept = |kind: AKind| chain.q_kept[kind as usize].is_some();
    assert!(
        [
            AKind::Receive1,
            AKind::Receive3,
            AKind::Receive4,
            AKind::Rebase
        ]
        .into_iter()
        .all(kept)
            && !kept(AKind::Receive5)
    );
    let (claims_a, _) = chain.claims_a.as_ref().expect("A's claims");
    let (claims_q, _) = chain.claims_q.as_ref().expect("Q's claims");
    assert_eq!(chain.a_digests[A_CLAIM_SLOT], claims_a.digest(&CLAIM_A));
    assert_eq!(chain.q_digests[Q_CLAIM_SLOT], claims_q.digest(&CLAIM_Q));
    let started = std::time::Instant::now();
    let decision = decide_chain(&rel, &chain);
    eprintln!(
        "FOLD_RECEIVE_CHAIN decision {decision:?} {:.1}s",
        started.elapsed().as_secs_f64()
    );
    assert!(decision.all(), "every accumulator decides: {decision:?}");

    let (bad_p, bad_v) = undecidable_claims();
    let (p, v) = trivial_claims();
    let trivial_p = AccumulatorT::new(*p.g(), *p.challenges()).expect("claim");
    // A bad transported acc_P, incoming or the predecessor's: every step holds
    // and A's claim accumulator refuses.
    for bad in [
        full_receive(&keys, Some((bad_p.clone(), v.clone()))),
        full_receive_with(&keys, None, Some((bad_p, v))),
    ] {
        let chain = run_chain(&keys, &rel, &bad, SCHEDULE.len());
        let decision = decide_chain(&rel, &chain);
        assert!(
            decision.q.is_ok() && decision.a.is_ok() && decision.claims_q.is_ok(),
            "{decision:?}"
        );
        assert!(
            decision.claims_a.is_err(),
            "a bad acc_P decided: {decision:?}"
        );
    }
    // A bad transported acc_V: Q's claim accumulator refuses after the step
    // that folds it (the Frame folding the Rebase for the predecessor's, Q
    // Sigma for the incoming one).
    for (bad, steps) in [
        (
            full_receive_with(&keys, None, Some((trivial_p.clone(), bad_v.clone()))),
            3,
        ),
        (full_receive(&keys, Some((trivial_p, bad_v))), 5),
    ] {
        let chain = run_chain(&keys, &rel, &bad, steps);
        let (acc, s) = chain.claims_q.as_ref().expect("Q's claims");
        assert!(
            iroha_fold::claims::decide_claims(rel.params_a.params().g(), acc, s).is_err(),
            "a bad acc_V decided"
        );
    }
}

/// The Frame instance opening a segment at the A state `base` under the chain's
/// relations: genuine at the segment base, its passthrough replaced otherwise
/// (see [`opened_at`]).
fn opening_frame(rel: &ReceiveRelations, base: &[Fp]) -> iroha_fold::ivc::Fresh<Ep> {
    let mut rng = ChaCha20Rng::seed_from_u64(47);
    let q_chain = QKind::Frame.chain(AKind::Receive5);
    let a = &rel.a[AKind::Receive5 as usize];
    let mut prepared = prepare_step(
        &q_chain,
        1,
        a,
        None,
        None,
        vec![],
        vec![Fq::ZERO; Q_DIGESTS],
        segment_start(),
    )
    .expect("prepare Q");
    prepared.witness.fresh.public[1] = trivial_export();
    let selected = trivial_vesta();
    let (claims, _, folds) =
        fold_claims(&CLAIM_Q, None, &[claim_of(&selected)]).expect("Q claim fold");
    let mut digests = prepared.digests.clone();
    digests[Q_CLAIM_SLOT] = claims.digest(&CLAIM_Q);
    let public = vec![step_public(
        &q_chain,
        1,
        Fq::from(BASE),
        prepared.passthrough,
        &[],
        &digests,
        &[],
    )];
    let witness = QFrameStepWitness {
        frame: prepared.witness,
        selected,
        claims: folds,
    };
    let step = QFrameStep {
        shape: rel.a_shape.clone(),
        witness: Some(witness),
    };
    assert!(
        check_circuit(&step, K, &public, CheckMode::Strict)
            .expect("Q synthesis")
            .is_satisfied()
    );
    let relation = &rel.q[QKind::Frame as usize];
    let (advice, public) = relation
        .stage_witness(&synthesize(&step, K, Some(&public)).expect("Q witness"))
        .expect("stage");
    fresh_instance(
        relation,
        &rel.key_q,
        &FOLD_Q,
        advice,
        opened_at(base, public),
        &mut rng,
    )
    .expect("Q instance")
}

/// The Rebase opens a segment only at the segment base, and only on an Omega
/// it hard-verifies: another lineage, proof or transported `acc_V` than the
/// Omega's, or a state other than the segment base, is refused. A1 refuses the
/// segment base. Prints the Rebase's rows and the decider batches with it.
#[test]
#[ignore = "k16 folded segment rebase; run optimized"]
fn the_rebase_opens_a_segment_only_at_the_base_and_on_its_last_omega() {
    let keys = receive_keys();
    let full = full_receive(&keys, None);
    let rel = receive_relations(&keys, &full);
    let frame = QKind::Frame as usize;
    let side = AKind::Rebase.chain();
    let at_base = opening_frame(&rel, &segment_base());
    // The Rebase at step 2 on `source` from the A state `state` (the Frame
    // instance opening the segment opens it at `state`, so that only the
    // Rebase's own rules refuse a state other than the segment base).
    let rebase = |source: &RebaseSource, state: &[Fp]| -> (RebaseFoldStep, Vec<Vec<Fp>>) {
        let last_q = if state == segment_base() {
            at_base.clone()
        } else {
            opening_frame(&rel, state)
        };
        let prepared = prepare_step(
            &side,
            2,
            &rel.q[frame],
            None,
            Some(&last_q),
            state.to_vec(),
            vec![Fp::ZERO; A_DIGESTS],
            Fp::ZERO,
        )
        .expect("prepare Rebase");
        let claims = source.pallas_claims().map(|c| claim_of(&c));
        let (acc, _, folds) = fold_claims(&CLAIM_A, None, &claims).expect("A claim folds");
        let mut digests = prepared.digests.clone();
        digests[A_CLAIM_SLOT] = acc.digest(&CLAIM_A);
        let public = vec![
            step_public(
                &side,
                2,
                Fp::from(BASE),
                prepared.passthrough,
                &source.next_state(),
                &digests,
                &[],
            ),
            source.export(),
        ];
        let witness = RebaseStepWitness {
            source: Arc::new(source.clone()),
            frame: prepared.witness,
            claims: folds,
        };
        (
            RebaseFoldStep::new(keys.plan.clone(), rel.q[frame].shape(), Some(witness)),
            public,
        )
    };
    let genuine = &full.rebase;
    let (step, public) = rebase(genuine, &segment_base());
    assert!(satisfied(&step, &public), "the genuine rebase");
    let synthesized = synthesize(&step, K, Some(&public)).expect("Rebase witness");
    let rows = synthesized
        .tables
        .advice_assigned()
        .iter()
        .map(|c| c.iter().rposition(|v| *v).map_or(0, |r| r + 1))
        .max();
    let unknown = synthesize(&step.without_witnesses(), K, None).expect("Rebase layout");
    assert_eq!(synthesized.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        synthesized.tables.permutation(),
        unknown.tables.permutation()
    );
    assert_eq!(
        synthesized.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let mut other = genuine.clone();
    other.lineage[6] += Fp::ONE;
    let (step, public) = rebase(&other, &segment_base());
    assert!(
        !satisfied(&step, &public),
        "a lineage the Omega does not prove"
    );
    let mut other = genuine.clone();
    other.proof[100] ^= 1;
    let (step, public) = rebase(&other, &segment_base());
    assert!(!satisfied(&step, &public), "a tampered Omega proof");
    let mut other = genuine.clone();
    other.vesta = undecidable_claims().1;
    let (step, public) = rebase(&other, &segment_base());
    assert!(
        !satisfied(&step, &public),
        "an acc_V other than the Omega's"
    );
    let (step, public) = rebase(genuine, &genuine.next_state());
    assert!(!satisfied(&step, &public), "a rebase mid-segment");
    // A1 refuses the segment base.
    let prepared = prepare_step(
        &AKind::Receive1.chain(),
        2,
        &rel.q[frame],
        None,
        Some(&at_base),
        segment_base(),
        vec![Fp::ZERO; A_DIGESTS],
        Fp::ZERO,
    )
    .expect("prepare A1");
    let mut next = vec![Fp::ZERO; 18];
    next.extend([
        keys.plan.context_digest(&full.source).expect("digest"),
        Fp::ONE,
    ]);
    let public = vec![
        step_public(
            &AKind::Receive1.chain(),
            2,
            Fp::from(BASE),
            prepared.passthrough,
            &next,
            &prepared.digests,
            &[],
        ),
        keys.plan.export(AKind::Receive1, &full.source),
    ];
    let witness = ReceiveStepWitness {
        source: Arc::new(full.source.clone()),
        frame: prepared.witness,
        claims: None,
    };
    let a1 = ReceiveFoldStep::new(
        keys.plan.clone(),
        AKind::Receive1,
        rel.q[frame].shape(),
        Some(witness),
    )
    .expect("A1 step");
    assert!(!satisfied(&a1, &public), "A1 accepted the segment base");
    // The decider batches the wraps decide, with the Rebase and the Frame
    // that folds export claims.
    let a_plans: Vec<_> = rel
        .a
        .iter()
        .map(|r| {
            iroha_fold::decider_v2::DeciderV2Plan::with_step_runs(
                r.clone(),
                &rel.key_a,
                A_STEP_RUNS,
            )
            .expect("plan")
        })
        .collect();
    let q_plans: Vec<_> = rel
        .q
        .iter()
        .map(|r| iroha_fold::decider_v2::DeciderV2Plan::new(r.clone(), &rel.key_q).expect("plan"))
        .collect();
    let receive: Vec<_> = a_plans[..4].iter().collect();
    let all: Vec<_> = a_plans.iter().collect();
    let q: Vec<_> = q_plans.iter().collect();
    let rebase_plan = &a_plans[AKind::Rebase as usize];
    eprintln!(
        "FOLD_RECEIVE_REBASE rows={rows:?} relation={:?} opened={:?} A batch (claims, absorbed, degree) without Rebase {:?} with {:?}; Q batch {:?}; Frame opened {:?}",
        rel.a[AKind::Rebase as usize].summary(),
        rebase_plan.verifier_profile().2,
        iroha_fold::decider_v2::DeciderV2Plan::batch_profile(&receive),
        iroha_fold::decider_v2::DeciderV2Plan::batch_profile(&all),
        iroha_fold::decider_v2::DeciderV2Plan::batch_profile(&q),
        q_plans[frame].verifier_profile().2,
    );
}
