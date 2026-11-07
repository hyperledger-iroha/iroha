//! The pieces of the Fp wrap P on the toy two-curve chain (iroha_fold's
//! test fixtures: Q steps with the genuine Receive signature leaf, toy A
//! steps): each piece in P's own lanes reaches exactly the native decider's
//! crossing values, and the test reports the rows it takes.

#[path = "../../iroha_fold/tests/common/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
mod common;

use common::{
    qstep::{QStep, leaf},
    step::{BASE, DIGESTS, DOMAIN_A, DOMAIN_Q, IVC_A, K, Side, Step, business, business_digests},
};
use ff::{Field, PrimeField};
use iroha_fold::circuit::{FoldLanes, mle::OpeningKey};
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation,
    circuit::field::FieldLanes,
    decider_v2::{self, DeciderV2Plan, DeciderV2Proof},
    fold::fresh_instance,
    ivc::{self, Fresh, Kept, embed, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::fold_wrap::p::{
    AFieldImports, QGroupWitness, a_decider_field_side, q_decider_commitment_side,
};
use iroha_pasta::PastaAffine;
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaField};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, synthesize},
    pcs::{ipa::PinnedParams, mle},
};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};
use std::sync::Arc;

/// The toy chain's relations under the monomial generators (v2 deciders).
struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
    params_q: PinnedParams<Ep>,
    params_a: PinnedParams<Eq>,
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
    let q =
        FoldedRelation::<Fq>::from_synthesized(&synthesize(&q_step, K, None).expect("Q layout"))
            .expect("Q relation");
    let a = a_relation(q.shape());
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

/// Runs Q1, A2, Q3, A4: the A accumulator Q keeps (A2) and the Q accumulator A
/// keeps (Q1, Q3).
fn chain(rel: &Relations) -> (Kept<Eq>, Kept<Ep>) {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let (q_chain, a_chain) = (Side::Q.chain(), Side::A.chain());
    let (mut q_state, mut a_state) = (Fq::from(BASE), Fp::from(BASE));
    let (mut q_digests, mut a_digests) = (vec![Fq::ZERO; DIGESTS], vec![Fp::ZERO; DIGESTS]);
    let (mut q_kept, mut a_kept): (Option<Kept<Eq>>, Option<Kept<Ep>>) = (None, None);
    let (mut last_a, mut last_q): (Option<Fresh<Eq>>, Option<Fresh<Ep>>) = (None, None);
    let start = embed(&ivc::ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &[Fp::from(BASE)],
        &[Fp::ZERO; DIGESTS],
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
                q_digests.clone(),
                start,
            )
            .expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            q_digests = business_digests(Side::Q, j == 1, prepared.digests.clone());
            let leaf = leaf(j);
            let leaf_public =
                leaf.instances(&[true, true, true]).expect("leaf instances")[0].clone();
            let frame = step_public(
                &q_chain,
                j,
                Fq::from(BASE),
                prepared.passthrough,
                &[next],
                &q_digests,
                &[Fq::from(j)],
            );
            let circuit = QStep {
                leaf,
                shape: rel.a.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized =
                synthesize(&circuit, K, Some(&[frame, leaf_public])).expect("witness");
            let (advice, public) = rel.q.stage_witness(&synthesized).expect("stage");
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
                a_digests.clone(),
                Fp::ZERO,
            )
            .expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            a_digests = business_digests(Side::A, j == 2, prepared.digests.clone());
            let public = vec![step_public(
                &a_chain,
                j,
                Fp::from(BASE),
                prepared.passthrough,
                &[next],
                &a_digests,
                &[Fp::from(j)],
            )];
            let circuit = Step::<Ep> {
                side: Side::A,
                shape: rel.q.shape(),
                witness: Some(prepared.witness),
            };
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.a.stage_witness(&synthesized).expect("stage");
            last_a = Some(
                fresh_instance(&rel.a, &rel.key_a, &DOMAIN_A, advice, public, &mut rng)
                    .expect("fresh"),
            );
            a_kept = prepared.kept;
            a_state = next;
        }
    }
    (
        q_kept.expect("Q keeps the A accumulator"),
        a_kept.expect("A keeps the Q accumulator"),
    )
}

/// Canonical S6 limbs of a field element's integer.
fn limbs<F: PastaField>(value: &F) -> [u128; 2] {
    let l = value.to_canonical_limbs();
    [
        u128::from(l[0]) | (u128::from(l[1]) << 64),
        u128::from(l[2]) | (u128::from(l[3]) << 64),
    ]
}

/// The A-side decider natively: its proof, the imports the field side needs
/// (Omega's values) and the exports it must reach.
struct ANative {
    plan: Arc<DeciderV2Plan<Eq>>,
    shape: FoldShape,
    acc: Accumulator<Eq>,
    proof: DeciderV2Proof<Eq>,
    imports: AFieldImports,
    rounds: usize,
    /// `[d_F, v*, b']`.
    exports: [[u128; 2]; 3],
}

fn a_native(rel: &Relations, kept: &Kept<Eq>) -> ANative {
    let (acc, witness) = kept;
    let plan = DeciderV2Plan::new(rel.a.clone(), &rel.key_a).expect("A plan");
    let mut proved = decider_v2::prove(&plan, &DOMAIN_A, acc, witness).expect("A prove");
    let (opening, _) = decider_v2::open(rel.params_a.params(), &mut proved).expect("A open");
    let mut verified = decider_v2::verify(&plan, &DOMAIN_A, acc, &proved.proof).expect("A verify");
    // alpha: the T_G schedule's one 128-bit squeeze after tag, k and count.
    let mut replay = verified.transcript.clone();
    replay.absorb_u64(mle::TAG_BATCH_OPENING);
    replay.absorb_u64(opening.rounds.len() as u64);
    replay.absorb_u64(verified.claims.len() as u64);
    let raw = limbs(&replay.challenge_128());
    let alpha = mle::endoscalar::<Fp>(raw[0]);
    let claim = decider_v2::verify_opening(rel.params_a.params(), &mut verified, &opening)
        .expect("A opening");
    claim
        .decide(&rel.params_a, iroha_pasta::msm::MemoryBudget::DEFAULT)
        .expect("A claim decides");
    let value = verified
        .claims
        .iter()
        .rev()
        .fold(Fp::ZERO, |acc, c| acc * alpha + c.value);
    let folded = mle::folded_eq(&verified.point, claim.challenges()).expect("k");
    ANative {
        shape: rel.a.shape(),
        imports: AFieldImports {
            acc_digest: limbs(&acc.digest(&DOMAIN_A)),
            helper_digest: limbs(&decider_v2::helper_digest::<Eq>(
                &DOMAIN_A,
                &proved.proof.helpers,
            )),
            alpha: limbs(&alpha),
            rounds: claim.challenges().iter().map(limbs).collect(),
        },
        rounds: opening.rounds.len(),
        exports: [limbs(&verified.field_digest), limbs(&value), limbs(&folded)],
        acc: acc.clone(),
        proof: proved.proof,
        plan: Arc::new(plan),
    }
}

thread_local! {
    static LANES: std::cell::Cell<[usize; 4]> = const { std::cell::Cell::new([0; 4]) };
}

/// P's lanes running the A-side decider's field side; every crossing value is
/// an instance row (imports, then `d_F`, `v*`, `b'`).
#[derive(Clone)]
struct AFieldCircuit {
    native: Arc<ANative>,
    known: bool,
}

#[derive(Clone, Debug)]
struct AFieldConfig {
    verifier: VerifierConfig<Ep>,
    instance: Column<Instance>,
}

impl Circuit<Fp> for AFieldCircuit {
    type Config = AFieldConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            native: Arc::clone(&self.native),
            known: false,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> AFieldConfig {
        let verifier = VerifierConfig::configure(meta);
        let instance = meta.instance_column(2 * (3 + 16 + 3));
        meta.enable_equality(instance);
        AFieldConfig { verifier, instance }
    }

    fn synthesize(
        &self,
        config: AFieldConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::<Ep>::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let native = &self.native;
        let known = self.known;
        let cells = layouter.assign_region(
            || "A decider field side",
            |mut region| {
                let refs = chip.fold_lanes()?;
                let mut lanes = FieldLanes {
                    glue: refs.glue,
                    range: refs.range,
                    duplex: refs.hash,
                };
                let crossing = a_decider_field_side(
                    &mut lanes,
                    &mut region,
                    &DOMAIN_A,
                    &native.plan,
                    &native.shape,
                    known.then_some(&native.acc),
                    known.then_some(&native.proof),
                    known.then_some(&native.imports),
                    native.rounds,
                )?;
                let refs = chip.fold_lanes()?;
                LANES.with(|cell| {
                    cell.set([
                        refs.ff.next_row(),
                        refs.ecc.next_row(),
                        refs.glue.next_row(),
                        refs.range.next_row(),
                    ])
                });
                let mut cells = Vec::new();
                for limbs in [
                    &crossing.acc_digest,
                    &crossing.helper_digest,
                    &crossing.alpha,
                ]
                .into_iter()
                .chain(crossing.rounds.iter())
                .chain([&crossing.field_digest, &crossing.value, &crossing.folded])
                {
                    cells.extend(limbs.iter().map(|word| word.cell()));
                }
                Ok(cells)
            },
        )?;
        for (row, cell) in cells.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, row)?;
        }
        Ok(())
    }
}

fn instance(native: &ANative) -> Vec<Fp> {
    let imports = &native.imports;
    [imports.acc_digest, imports.helper_digest, imports.alpha]
        .into_iter()
        .chain(imports.rounds.iter().copied())
        .chain(native.exports)
        .flat_map(|[lo, hi]| [Fp::from_u128(lo), Fp::from_u128(hi)])
        .collect()
}

#[test]
#[ignore = "toy chain with genuine P-256 leaves and a k16 circuit; run optimized"]
fn the_a_decider_field_side_runs_on_p_lanes_as_the_native_decider() {
    let rel = relations();
    let (a_kept, _) = chain(&rel);
    let native = Arc::new(a_native(&rel, &a_kept));
    assert_eq!(native.rounds, 16);
    let public = instance(&native);
    let satisfied = |public: &[Fp]| {
        let circuit = AFieldCircuit {
            native: Arc::clone(&native),
            known: true,
        };
        check_circuit(&circuit, 16, &[public.to_vec()], CheckMode::Strict)
            .expect("synthesis")
            .is_satisfied()
    };
    assert!(satisfied(&public), "native crossing values");
    let [ff, ecc, glue, range] = LANES.with(std::cell::Cell::get);
    println!(
        "A decider field side in P's lanes (FF, ECC, glue, range): {ff}, {ecc}, {glue}, {range}"
    );
    // Another exported v*, another imported alpha: unsatisfiable.
    let rows = public.len();
    for row in [rows - 4, 4] {
        let mut changed = public.clone();
        changed[row] += Fp::ONE;
        assert!(!satisfied(&changed), "row {row}");
    }
}

/// The Q-side decider natively: the commitment side's witness and the
/// crossing values it must reach (`acc_digest`, `D_H`, `d_F`, `v*`, `b'`,
/// `alpha`, `u`) plus the claim point.
struct QNative {
    plan: Arc<DeciderV2Plan<Ep>>,
    shape: FoldShape,
    key: OpeningKey<Ep>,
    witness: QGroupWitness,
    public: Vec<Fp>,
}

fn q_native(rel: &Relations, kept: &Kept<Ep>) -> QNative {
    let (acc, witness) = kept;
    let plan = DeciderV2Plan::new(rel.q.clone(), &rel.key_q).expect("Q plan");
    let mut proved = decider_v2::prove(&plan, &DOMAIN_Q, acc, witness).expect("Q prove");
    let (opening, _) = decider_v2::open(rel.params_q.params(), &mut proved).expect("Q open");
    let mut verified = decider_v2::verify(&plan, &DOMAIN_Q, acc, &proved.proof).expect("Q verify");
    let mut replay = verified.transcript.clone();
    replay.absorb_u64(mle::TAG_BATCH_OPENING);
    replay.absorb_u64(opening.rounds.len() as u64);
    replay.absorb_u64(verified.claims.len() as u64);
    let alpha = mle::endoscalar::<Fq>(limbs(&replay.challenge_128())[0]);
    let claim = decider_v2::verify_opening(rel.params_q.params(), &mut verified, &opening)
        .expect("Q opening");
    claim
        .decide(&rel.params_q, iroha_pasta::msm::MemoryBudget::DEFAULT)
        .expect("Q claim decides");
    let value = verified
        .claims
        .iter()
        .rev()
        .fold(Fq::ZERO, |acc, c| acc * alpha + c.value);
    let folded = mle::folded_eq(&verified.point, claim.challenges()).expect("k");
    let mut public: Vec<[u128; 2]> = vec![
        limbs(&acc.digest(&DOMAIN_Q)),
        limbs(&decider_v2::helper_digest::<Ep>(
            &DOMAIN_Q,
            &proved.proof.helpers,
        )),
        limbs(&verified.field_digest),
        limbs(&value),
        limbs(&folded),
        limbs(&alpha),
    ];
    public.extend(claim.challenges().iter().map(limbs));
    let mut public: Vec<Fp> = public
        .into_iter()
        .flat_map(|[lo, hi]| [Fp::from_u128(lo), Fp::from_u128(hi)])
        .collect();
    public.extend([claim.g().x(), claim.g().y()]);
    QNative {
        shape: rel.q.shape(),
        key: OpeningKey::new(rel.params_q.params()),
        witness: QGroupWitness {
            acc: acc.clone(),
            helpers: proved.proof.helpers.clone(),
            opening,
            imports: [verified.field_digest, value, folded],
        },
        public,
        plan: Arc::new(plan),
    }
}

/// P's lanes running the Q-side decider's commitment side and its opening.
#[derive(Clone)]
struct QGroupCircuit {
    native: Arc<QNative>,
    known: bool,
}

impl Circuit<Fp> for QGroupCircuit {
    type Config = AFieldConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            native: Arc::clone(&self.native),
            known: false,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> AFieldConfig {
        let verifier = VerifierConfig::configure(meta);
        let instance = meta.instance_column(2 * (6 + 16) + 2);
        meta.enable_equality(instance);
        AFieldConfig { verifier, instance }
    }

    fn synthesize(
        &self,
        config: AFieldConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::<Ep>::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let native = &self.native;
        let known = self.known;
        let cells = layouter.assign_region(
            || "Q decider commitment side",
            |mut region| {
                let refs = chip.fold_lanes()?;
                let mut lanes = FoldLanes {
                    glue: refs.glue,
                    range: refs.range,
                    ff: refs.ff,
                    ecc: refs.ecc,
                    duplex: refs.hash,
                };
                let crossing = q_decider_commitment_side(
                    &mut lanes,
                    &mut region,
                    &DOMAIN_Q,
                    &native.plan,
                    &native.shape,
                    &native.key,
                    known.then_some(&native.witness),
                )?;
                let refs = chip.fold_lanes()?;
                LANES.with(|cell| {
                    cell.set([
                        refs.ff.next_row(),
                        refs.ecc.next_row(),
                        refs.glue.next_row(),
                        refs.range.next_row(),
                    ])
                });
                let mut cells = Vec::new();
                for limbs in [
                    &crossing.acc_digest,
                    &crossing.helper_digest,
                    &crossing.field_digest,
                    &crossing.value,
                    &crossing.folded,
                    &crossing.alpha,
                ]
                .into_iter()
                .chain(crossing.rounds.iter())
                {
                    cells.extend(limbs.iter().map(|word| word.cell()));
                }
                cells.extend([
                    crossing.claim.folded_generator.x().cell(),
                    crossing.claim.folded_generator.y().cell(),
                ]);
                Ok(cells)
            },
        )?;
        for (row, cell) in cells.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, row)?;
        }
        Ok(())
    }
}

#[test]
#[ignore = "toy chain with genuine P-256 leaves and a k16 circuit; run optimized"]
fn the_q_decider_commitment_side_runs_on_p_lanes_as_the_native_decider() {
    let rel = relations();
    let (_, q_kept) = chain(&rel);
    let native = Arc::new(q_native(&rel, &q_kept));
    let satisfied = |public: &[Fp]| {
        let circuit = QGroupCircuit {
            native: Arc::clone(&native),
            known: true,
        };
        check_circuit(&circuit, 16, &[public.to_vec()], CheckMode::Strict)
            .expect("synthesis")
            .is_satisfied()
    };
    assert!(
        satisfied(&native.public),
        "native crossing values and claim"
    );
    let [ff, ecc, glue, range] = LANES.with(std::cell::Cell::get);
    println!(
        "Q decider commitment side in P's lanes (FF, ECC, glue, range): {ff}, {ecc}, {glue}, {range}"
    );
    // Another imported v* (row 6), another exported alpha (row 10), another claim point.
    let last = native.public.len() - 1;
    for row in [6, 10, last] {
        let mut changed = native.public.clone();
        changed[row] += Fp::ONE;
        assert!(!satisfied(&changed), "row {row}");
    }
}
