//! A Q step with genuine business: the Receive signature leaf
//! (`QSignatureCircuit`: own receipt hard, incoming receipt and Request soft,
//! real `SHA256withECDSA` P-256 signatures) composed with a fold frame that
//! folds the previous A instance. The chain `Q1 A2 Q3 A4` (A steps are the toy A
//! steps, folding these Q instances) folds and decides. Heavy: run optimized.

use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{Fresh, Kept, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::{Circuit, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::{
    qstep::{QStep, leaf},
    step::{BASE, DIGESTS, DOMAIN_A, DOMAIN_Q, K, Side, Step, business, business_digests},
};

struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
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
        let layout = synthesize(&Step::<Ep> { side: Side::A, shape, witness: None }, K, None).expect("A layout");
        FoldedRelation::<Fp>::from_synthesized(&layout).expect("A relation")
    };
    let first = a_relation(placeholder).shape();
    let q_step = QStep { leaf: leaf(0).without_witnesses(), shape: first.clone(), witness: None };
    let layout = synthesize(&q_step, K, None).expect("Q layout");
    let q = FoldedRelation::<Fq>::from_synthesized(&layout).expect("Q relation");
    let a = a_relation(q.shape());
    assert_eq!(a.shape(), first);
    eprintln!("Q step relation: {:?}", q.summary());
    eprintln!("A step relation: {:?}", a.summary());
    Relations {
        key_q: CommitKey::lagrange(PinnedParams::<Ep>::derive(K).expect("params").params()),
        key_a: CommitKey::lagrange(PinnedParams::<Eq>::derive(K).expect("params").params()),
        q,
        a,
    }
}

#[test]
#[ignore = "genuine P-256 leaves in k16 IVC steps; run optimized"]
fn q_steps_with_genuine_signature_leaves_fold_and_decide() {
    let rel = relations();
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let q_chain = Side::Q.chain();
    let a_chain = Side::A.chain();
    let (mut q_state, mut a_state) = (Fq::from(BASE), Fp::from(BASE));
    let (mut q_digests, mut a_digests) = (vec![Fq::ZERO; DIGESTS], vec![Fp::ZERO; DIGESTS]);
    let mut q_kept: Option<Kept<Eq>> = None; // A instances, kept by Q
    let mut a_kept: Option<Kept<Ep>> = None; // Q instances, kept by A
    let mut last_a: Option<Fresh<Eq>> = None;
    let mut last_q: Option<Fresh<Ep>> = None;
    let start = common::step::start_passthrough();
    for j in 1..=4u64 {
        let started = std::time::Instant::now();
        if j % 2 == 1 {
            let prepared =
                prepare_step(&q_chain, j, &rel.a, q_kept.as_ref(), last_a.as_ref(), vec![q_state], q_digests.clone(), start)
                    .expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            q_digests = business_digests(Side::Q, j == 1, prepared.digests.clone());
            let leaf = leaf(j);
            let leaf_public = leaf.instances(&[true, true, true]).expect("leaf instances")[0].clone();
            let frame = step_public(&q_chain, j, Fq::from(BASE), prepared.passthrough, &[next], &q_digests, &[Fq::from(j)]);
            let public = vec![frame, leaf_public];
            let circuit = QStep { leaf, shape: rel.a.shape(), witness: Some(prepared.witness) };
            let report = check_circuit(&circuit, K, &public, CheckMode::Strict).expect("synthesis");
            assert!(report.is_satisfied(), "Q step {j}: {report}");
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.q.stage_witness(&synthesized).expect("stage witness");
            last_q = Some(fresh_instance(&rel.q, &rel.key_q, &DOMAIN_Q, advice, public, &mut rng).expect("fresh"));
            q_kept = prepared.kept;
            q_state = next;
        } else {
            let prepared =
                prepare_step(&a_chain, j, &rel.q, a_kept.as_ref(), last_q.as_ref(), vec![a_state], a_digests.clone(), Fp::ZERO)
                    .expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            a_digests = business_digests(Side::A, j == 2, prepared.digests.clone());
            let public = vec![step_public(&a_chain, j, Fp::from(BASE), prepared.passthrough, &[next], &a_digests, &[Fp::from(j)])];
            let circuit = Step::<Ep> { side: Side::A, shape: rel.q.shape(), witness: Some(prepared.witness) };
            let report = check_circuit(&circuit, K, &public, CheckMode::Strict).expect("synthesis");
            assert!(report.is_satisfied(), "A step {j}: {report}");
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.a.stage_witness(&synthesized).expect("stage witness");
            last_a = Some(fresh_instance(&rel.a, &rel.key_a, &DOMAIN_A, advice, public, &mut rng).expect("fresh"));
            a_kept = prepared.kept;
            a_state = next;
        }
        eprintln!("step {j}: {:.1} s", started.elapsed().as_secs_f64());
    }
    let (acc_a, w_a) = q_kept.as_ref().expect("Q keeps A instances");
    decide(&rel.a, &rel.key_a, acc_a, w_a).expect("A instances decide");
    let (acc_q, w_q) = a_kept.as_ref().expect("A keeps Q instances");
    decide(&rel.q, &rel.key_q, acc_q, w_q).expect("Q steps with signature leaves decide");
    let (last, last_w) = last_a.as_ref().expect("last A");
    decide_fresh(&rel.a, &rel.key_a, &DOMAIN_A, last, last_w).expect("last A decides fresh");
    assert_eq!(a_state, Fp::from(BASE + 1 + 9));
}
