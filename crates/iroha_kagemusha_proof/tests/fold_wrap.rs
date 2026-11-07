//! The compression wraps end to end on a two-kind toy chain.
//!
//! Each side has two relation kinds, so P and Omega decide batches. The A
//! state carries two words that must be zero at an operation boundary. The
//! schedule repeats `Q_a, A_a, Q_b, A_b` (`Q_a` folds `A_b`, `Q_b` folds
//! `A_a`, `A_a` folds `Q_a`, `A_b` folds `Q_b`), and each side keeps its claim
//! accumulator from iroha_fold's fixtures.
//!
//! A chain of four ends with `A_b` while `Q_b` kept no `A_b`, so Omega starts
//! that accumulator. A chain of eight ends with `A_b` folding into the kept
//! one. For both chains:
//! - the native producer's P and Omega are satisfied;
//! - a changed crossing row, public value or boundary word is rejected;
//! - both proofs are made, Omega verifies natively, and its opening, `acc_V`
//!   and `acc_P` decide.
//!
//! The test prints each wrap's rows and each stage's time.

#[path = "../../iroha_fold/tests/common/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
mod common;

use std::{sync::Arc, time::Instant};

use common::step::{CLAIM_A, CLAIM_Q, claim_a, claim_q};
use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    fold::fresh_instance,
    ivc::{ChainSide, FrameWitness, Fresh, Kept, embed, ivc_hash, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    a_relation::LINEAGE_FIELDS,
    fold_wrap::{
        Domains, WrapSpec,
        native::{self, FinalChain, WrapTapes},
        p::{PCircuit, PWitness},
    },
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, synthesize},
    pcs::ipa::PinnedParams,
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::Word;
use iroha_plonk_recursion::{
    FoldConfig, FoldInput, FoldTape, FoldWitness, verifier::diagnostic_take_usage,
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

const K: u32 = 16;
const BASE: u64 = 0x5eed;
const FOLD_Q: [u8; 32] = *b"fold wrap toy: Q kinds' domain..";
const FOLD_A: [u8; 32] = *b"fold wrap toy: A kinds' domain..";
const IVC_Q: [u8; 32] = *b"fold wrap toy: Q side IVC domain";
const IVC_A: [u8; 32] = *b"fold wrap toy: A side IVC domain";
/// Kinds per side; a side's digest list is its kept accumulators, then its
/// claim accumulator.
const KINDS: usize = 2;
const DIGESTS: usize = KINDS + 1;
const CLAIM: usize = KINDS;
/// The A state: a value, then two words zero at an operation boundary.
const A_STATE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Q,
    A,
}

/// One relation kind: its side and its index in that side's kind list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kind {
    side: Side,
    index: usize,
}

const QA: Kind = Kind {
    side: Side::Q,
    index: 0,
};
const QB: Kind = Kind {
    side: Side::Q,
    index: 1,
};
const AA: Kind = Kind {
    side: Side::A,
    index: 0,
};
const AB: Kind = Kind {
    side: Side::A,
    index: 1,
};
const SCHEDULE: [Kind; 4] = [QA, AA, QB, AB];

impl Kind {
    /// The other side's kind this kind folds (its slot in this side's list).
    const fn folds(self) -> usize {
        match (self.side, self.index) {
            (Side::Q, 0) => 1,
            (Side::Q, _) => 0,
            (Side::A, index) => index,
        }
    }

    const fn chain(self) -> ChainSide {
        let slot = self.folds();
        let (ivc_domain, fold_domain, first_step) = match self.side {
            Side::Q => (IVC_Q, FOLD_A, 1),
            Side::A => (IVC_A, FOLD_Q, 2),
        };
        ChainSide {
            ivc_domain,
            fold_domain,
            first_step,
            slot,
            digests: DIGESTS,
            choices: 1 << slot,
        }
    }

    const fn state_len(self) -> usize {
        match self.side {
            Side::Q => 0,
            Side::A => A_STATE,
        }
    }
}

fn a_base() -> Vec<Fp> {
    vec![Fp::from(BASE), Fp::ZERO, Fp::ZERO]
}

/// The passthrough of the chain's first instance: A's base hash.
fn start_passthrough<F: PastaField>() -> F {
    embed(&ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &a_base(),
        &[Fp::ZERO; DIGESTS],
    ))
}

/// A side's claim-accumulator digest over the side's field.
fn claim_digest<F: PastaField>(side: Side) -> F {
    match side {
        Side::Q => embed(&claim_q().0.digest(&CLAIM_Q)),
        Side::A => embed(&claim_a().0.digest(&CLAIM_A)),
    }
}

/// A toy step of one kind: the fold frame, the claim accumulator started on
/// the side's first instance, a toy business (A adds the square of Q's
/// bridge and keeps its boundary words), and the frame outputs.
struct KindStep<C: PastaCurve> {
    kind: Kind,
    shape: FoldShape,
    witness: Option<FrameWitness<C>>,
}

impl<C: PastaCurve> Circuit<C::Base> for KindStep<C> {
    type Config = (FoldVerifierConfig<C>, Column<InstanceColumn>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            kind: self.kind,
            shape: self.shape.clone(),
            witness: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        (fold, instance)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.0);
        chips.load_tables(&mut layouter)?;
        let outputs =
            layouter.assign_region(|| "step", |mut region| self.layout(&mut chips, &mut region))?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.1, row)?;
        }
        Ok(())
    }
}

impl<C: PastaCurve> KindStep<C> {
    fn layout(
        &self,
        chips: &mut FoldVerifierChips<C>,
        region: &mut Region<'_, C::Base>,
    ) -> Result<Vec<Word<C::Base>>, Error> {
        let chain = self.kind.chain();
        let frame = fold_frame(
            &mut chips.lanes(),
            region,
            &chain,
            &self.shape,
            C::Base::from(BASE),
            start_passthrough(),
            self.kind.state_len(),
            self.witness.as_ref(),
        )?;
        let started = chips.glue.constant(region, claim_digest(self.kind.side))?;
        let mut digests = frame.digests.clone();
        digests[CLAIM] =
            chips
                .glue
                .select(region, &frame.first, &started, &frame.digests[CLAIM])?;
        let next = match self.kind.side {
            Side::Q => Vec::new(),
            Side::A => {
                // The side's first instance starts from [base, 0, 0].
                let later = chips.glue.not(region, &frame.first)?;
                let zero = chips.glue.constant(region, <C::Base as Field>::ZERO)?;
                assert_equal_unless(
                    &mut chips.lanes(),
                    region,
                    &frame.state[0],
                    &frame.base,
                    Some(&later),
                )?;
                for word in &frame.state[1..] {
                    assert_equal_unless(&mut chips.lanes(), region, word, &zero, Some(&later))?;
                }
                let square = chips.glue.mul(region, &frame.bridge[0], &frame.bridge[0])?;
                let value = chips.glue.add(region, &frame.state[0], &square)?;
                vec![value, frame.state[1].clone(), frame.state[2].clone()]
            }
        };
        frame_outputs(
            &mut chips.lanes(),
            region,
            &chain,
            &frame,
            &next,
            &digests,
            &[frame.step.clone()],
        )
    }
}

/// The toy relations of both sides, under the monomial generators.
struct Relations {
    q: Vec<FoldedRelation<Fq>>,
    a: Vec<FoldedRelation<Fp>>,
    key_q: CommitKey<Ep>,
    key_a: CommitKey<Eq>,
    params_q: PinnedParams<Ep>,
    params_a: PinnedParams<Eq>,
}

fn relation_q(kind: Kind, shape: FoldShape) -> FoldedRelation<Fq> {
    let step = KindStep::<Eq> {
        kind,
        shape,
        witness: None,
    };
    FoldedRelation::from_synthesized(&synthesize(&step, K, None).expect("Q layout"))
        .expect("Q relation")
}

fn relation_a(kind: Kind, shape: FoldShape) -> FoldedRelation<Fp> {
    let step = KindStep::<Ep> {
        kind,
        shape,
        witness: None,
    };
    FoldedRelation::from_synthesized(&synthesize(&step, K, None).expect("A layout"))
        .expect("A relation")
}

/// Each kind's circuit embeds the shape of the kind it folds; the shapes
/// form a cycle, so iterate until they repeat.
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
    let mut q_shapes = vec![placeholder.clone(); KINDS];
    let mut a_shapes = vec![placeholder; KINDS];
    let mut found = None;
    for _ in 0..5 {
        let q: Vec<_> = [QA, QB]
            .iter()
            .map(|k| relation_q(*k, a_shapes[k.folds()].clone()))
            .collect();
        let a: Vec<_> = [AA, AB]
            .iter()
            .map(|k| relation_a(*k, q_shapes[k.folds()].clone()))
            .collect();
        let (next_q, next_a): (Vec<_>, Vec<_>) = (
            q.iter().map(FoldedRelation::shape).collect(),
            a.iter().map(FoldedRelation::shape).collect(),
        );
        let stable = next_q == q_shapes && next_a == a_shapes;
        q_shapes = next_q;
        a_shapes = next_a;
        if stable {
            found = Some((q, a));
            break;
        }
    }
    let (q, a) = found.expect("the kinds' shapes reach a fixed point");
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    Relations {
        key_q: CommitKey::monomial(params_q.params()),
        key_a: CommitKey::monomial(params_a.params()),
        q,
        a,
        params_q,
        params_a,
    }
}

/// Runs `steps` half-steps of the schedule (ending with `A_b`).
fn chain(rel: &Relations, steps: u64) -> FinalChain {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let mut q_kept: Vec<Option<Kept<Eq>>> = vec![None; KINDS];
    let mut a_kept: Vec<Option<Kept<Ep>>> = vec![None; KINDS];
    let (mut q_digests, mut a_digests) = (vec![Fq::ZERO; DIGESTS], vec![Fp::ZERO; DIGESTS]);
    let mut a_state = a_base();
    let (mut last_a, mut last_q): (Option<Fresh<Eq>>, Option<Fresh<Ep>>) = (None, None);
    for j in 1..=steps {
        let kind = SCHEDULE[usize::try_from((j - 1) % 4).expect("small")];
        let chain = kind.chain();
        let slot = kind.folds();
        match kind.side {
            Side::Q => {
                let prepared = prepare_step(
                    &chain,
                    j,
                    &rel.a[slot],
                    q_kept[slot].as_ref(),
                    last_a.as_ref(),
                    Vec::new(),
                    q_digests.clone(),
                    start_passthrough(),
                )
                .expect("prepare");
                q_digests.clone_from(&prepared.digests);
                if j == chain.first_step {
                    q_digests[CLAIM] = claim_digest(Side::Q);
                }
                let public = step_public(
                    &chain,
                    j,
                    Fq::from(BASE),
                    prepared.passthrough,
                    &[],
                    &q_digests,
                    &[Fq::from(j)],
                );
                let circuit = KindStep::<Eq> {
                    kind,
                    shape: rel.a[slot].shape(),
                    witness: Some(prepared.witness),
                };
                let synthesized = synthesize(&circuit, K, Some(&[public])).expect("witness");
                let relation = &rel.q[kind.index];
                let (advice, public) = relation.stage_witness(&synthesized).expect("stage");
                last_q = Some(
                    fresh_instance(relation, &rel.key_q, &FOLD_Q, advice, public, &mut rng)
                        .expect("fresh"),
                );
                q_kept[slot] = prepared.kept;
            }
            Side::A => {
                let prepared = prepare_step(
                    &chain,
                    j,
                    &rel.q[slot],
                    a_kept[slot].as_ref(),
                    last_q.as_ref(),
                    a_state.clone(),
                    a_digests.clone(),
                    Fp::ZERO,
                )
                .expect("prepare");
                a_digests.clone_from(&prepared.digests);
                if j == chain.first_step {
                    a_digests[CLAIM] = claim_digest(Side::A);
                }
                let bridge = prepared.bridge[0];
                let next = vec![a_state[0] + bridge.square(), a_state[1], a_state[2]];
                let public = vec![step_public(
                    &chain,
                    j,
                    Fp::from(BASE),
                    prepared.passthrough,
                    &next,
                    &a_digests,
                    &[Fp::from(j)],
                )];
                let circuit = KindStep::<Ep> {
                    kind,
                    shape: rel.q[slot].shape(),
                    witness: Some(prepared.witness),
                };
                let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
                let relation = &rel.a[kind.index];
                let (advice, public) = relation.stage_witness(&synthesized).expect("stage");
                last_a = Some(
                    fresh_instance(relation, &rel.key_a, &FOLD_A, advice, public, &mut rng)
                        .expect("fresh"),
                );
                a_kept[slot] = prepared.kept;
                a_state = next;
            }
        }
    }
    FinalChain {
        step: steps,
        a_state,
        q_state: Vec::new(),
        u_q: a_kept
            .into_iter()
            .map(|kept| kept.expect("A keeps every Q kind"))
            .collect(),
        u_a: q_kept,
        a_n: last_a.expect("A_n"),
        pallas_claims: claim_a().clone(),
        vesta_claims: claim_q().clone(),
    }
}

/// A deterministic test tape (SplitMix64 words; production samples OS coins).
fn tape<C: PastaCurve>(seed: u64) -> FoldTape<C> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    FoldTape::<C>::try_fill::<core::convert::Infallible>(|coin| {
        *coin = C::ScalarExt::from_raw_reduced([next(), next(), next(), next() >> 2]);
        Ok(())
    })
    .expect("infallible")
}

fn tapes(seed: u64) -> WrapTapes {
    WrapTapes {
        pallas_restructure: tape(seed),
        pallas_fold: tape(seed + 1),
        vesta_restructure: tape(seed + 2),
        vesta_fold: tape(seed + 3),
    }
}

fn usage(label: &str) {
    for entry in diagnostic_take_usage() {
        println!(
            "{label} rows: sponge {}, glue {}, ECC {}, range {}",
            entry.sponge, entry.arithmetic, entry.curve, entry.range
        );
    }
}

fn copy_fold(witness: &FoldWitness<Ep>) -> FoldWitness<Ep> {
    FoldWitness::from_private_bytes(&witness.to_private_bytes()).expect("canonical witness")
}

/// P with one boundary word of `z_A` set to one and `A_n`'s `h_out` recomputed
/// over that state, so only the boundary rule can reject it.
fn off_boundary(keys: &native::WrapKeys, prepared: &native::Prepared) -> (PCircuit, Vec<Fp>) {
    let spec = &keys.p_plan.spec;
    let original: &PWitness = &prepared.p_witness;
    let mut state = original.state.clone();
    state[spec.boundary[0]] = Fp::ONE;
    let layout = &keys.p_plan.layout;
    let parsed = layout.parse(&prepared.p_public).expect("layout");
    let mut digests: Vec<Fp> = parsed.q.members.iter().map(|m| m.acc_digest).collect();
    digests.push(parsed.pallas.digest);
    let h_out = ivc_hash::<Fp>(&IVC_A, original.step, Fp::from(BASE), &state, &digests);
    let mut a_public = original.a_public.clone();
    a_public[1] = h_out;
    let mut public = prepared.p_public.clone();
    public[1] = h_out;
    let witness = PWitness {
        step: original.step,
        state,
        a_public,
        q: original.q.clone(),
        a: original.a.clone(),
        pallas: original.pallas.clone(),
        vesta: original.vesta.clone(),
        restructure: copy_fold(&original.restructure),
        fold: copy_fold(&original.fold),
    };
    (
        PCircuit {
            plan: Arc::clone(&keys.p_plan),
            witness: Some(Arc::new(witness)),
        },
        public,
    )
}

/// Wraps one chain: P and Omega checked (with mutations when `mutate`),
/// proved, Omega verified natively and every carried accumulator decided.
fn wrap(keys: &native::WrapKeys, chain: &FinalChain, seed: u64, mutate: bool) {
    let spec = &keys.p_plan.spec;
    let config = FoldConfig::default();
    let _ = diagnostic_take_usage();
    let now = Instant::now();
    let prepared = native::prepare(keys, chain, tapes(seed), &config).expect("native wraps");
    println!(
        "native deciders, folds and restructurings: {:.1?}",
        now.elapsed()
    );
    let p = native::p_circuit(keys, &prepared);
    let report = check_circuit(&p, K, &[prepared.p_public.clone()], CheckMode::Strict).expect("P");
    usage("P");
    assert!(report.is_satisfied(), "P: {:?}", report.failures().first());
    if mutate {
        let layout = &keys.p_plan.layout;
        let parsed = layout.parse(&prepared.p_public).expect("layout");
        let row_of = |value: &Fp| {
            prepared
                .p_public
                .iter()
                .position(|v| v == value)
                .expect("row")
        };
        for (label, row) in [
            ("h_out_n", 1),
            ("D_A", layout.a_public),
            ("a Q member digest", row_of(&parsed.q.members[1].acc_digest)),
            ("an A member's e", row_of(&parsed.a.members[1].terms.e)),
            ("the last row", layout.rows() - 1),
        ] {
            let mut changed = prepared.p_public.clone();
            changed[row] += Fp::ONE;
            let report = check_circuit(&p, K, &[changed], CheckMode::Strict).expect("P");
            let _ = diagnostic_take_usage();
            assert!(!report.is_satisfied(), "P accepts another {label}");
        }
        let (circuit, public) = off_boundary(keys, &prepared);
        let report = check_circuit(&circuit, K, &[public], CheckMode::Strict).expect("P");
        let _ = diagnostic_take_usage();
        assert!(!report.is_satisfied(), "P accepts a state off the boundary");
    }
    let now = Instant::now();
    let (p_proof, p_opening) = native::prove_p(keys, &prepared).expect("P proof");
    println!("P proof ({} bytes): {:.1?}", p_proof.len(), now.elapsed());
    let acc_p = prepared.acc_p.clone();
    let (omega, instances, acc_v) =
        native::omega_circuit(keys, prepared, p_proof, &p_opening, &config).expect("Omega");
    let _ = diagnostic_take_usage();
    let report = check_circuit(&omega, K, &instances, CheckMode::Strict).expect("Omega");
    usage("Omega");
    assert!(
        report.is_satisfied(),
        "Omega: {:?}",
        report.failures().first()
    );
    if mutate {
        for (column, row) in [(0, 0), (1, 0), (2, 15)] {
            let mut changed = instances.clone();
            changed[column][row] += Fq::ONE;
            let report = check_circuit(&omega, K, &changed, CheckMode::Strict).expect("Omega");
            let _ = diagnostic_take_usage();
            assert!(
                !report.is_satisfied(),
                "Omega accepts another column {column} row {row}"
            );
        }
    }
    let now = Instant::now();
    let columns = Witness::from_circuit(&keys.omega_key, &omega, &instances).expect("witness");
    let output = create_proof_owned_with_claim(
        &spec.params_q,
        &keys.omega_key,
        columns,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .expect("Omega proof");
    println!(
        "Omega proof ({} bytes): {:.1?}",
        output.proof.len(),
        now.elapsed()
    );
    let budget = MemoryBudget::DEFAULT;
    let claim = accumulate_generator(
        &spec.params_q,
        keys.omega_key.binding(),
        keys.omega_key.vk(),
        &instances,
        &output.proof,
        budget,
    )
    .expect("Omega verifies");
    FoldInput::from_opening(*claim.g(), claim.challenges())
        .expect("claim")
        .decide(&spec.params_q, budget)
        .expect("Omega's opening decides");
    acc_v.decide(&spec.params_a, budget).expect("acc_V decides");
    acc_p.decide(&spec.params_q, budget).expect("acc_P decides");
}

#[test]
#[ignore = "two-kind toy chains, two k16 keygens, checks and proofs; run optimized"]
fn the_wraps_finish_two_kind_chains_into_omegas_that_decide() {
    let started = Instant::now();
    let rel = relations();
    println!("relations: {:.1?}", started.elapsed());
    let spec = Arc::new(WrapSpec {
        q: rel.q.clone(),
        a: rel.a.clone(),
        last: AB.index,
        key_q: rel.key_q.clone(),
        key_a: rel.key_a.clone(),
        params_q: rel.params_q.clone(),
        params_a: rel.params_a.clone(),
        domains: Domains {
            q_fold: FOLD_Q,
            a_fold: FOLD_A,
            a_ivc: IVC_A,
            q_ivc: IVC_Q,
            pallas_claims: CLAIM_A,
            vesta_claims: CLAIM_Q,
        },
        base_a: Fp::from(BASE),
        base_q: Fq::from(BASE),
        a_state: A_STATE,
        q_state: 0,
        lineage: [0; LINEAGE_FIELDS],
        boundary: vec![1, 2],
    });
    let now = Instant::now();
    let keys = native::keygen(Arc::clone(&spec)).expect("keys");
    println!(
        "keygen P and Omega: {:.1?}; P has {} public rows",
        now.elapsed(),
        keys.p_plan.layout.rows()
    );
    // Four half-steps: Q_b kept no A_b, so Omega starts it from A_4.
    let short = chain(&rel, 4);
    assert!(short.u_a[AB.index].is_none() && short.u_a[AA.index].is_some());
    wrap(&keys, &short, 1, true);
    // Eight half-steps: A_8 folds into the A_b accumulator Q_7 kept.
    let long = chain(&rel, 8);
    assert!(long.u_a.iter().all(Option::is_some));
    wrap(&keys, &long, 11, false);
    println!("total: {:.1?}", started.elapsed());
}
