//! A Q step with genuine business: the Receive signature leaf
//! (`QSignatureCircuit`: own receipt hard, incoming receipt and Request soft,
//! real `SHA256withECDSA` P-256 signatures) composed with a fold frame that
//! folds the previous A instance. The chain `Q1 A2 Q3 A4` (A steps are the toy A
//! steps, folding these Q instances) folds and decides. Heavy: run optimized.

use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{self, Fresh, FrameWitness, Kept, embed, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::q_signature::{
    QSignatureCircuit, QSignatureConfig, QSignaturePlan, SignatureKey, SignatureSlot, SignatureWitness,
};
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, synthesize},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::{
    p256::{
        VerifyMode,
        native::{self, Affine, HALF_N, ORDER, verify_prehashed},
    },
    sha256::native::sha256_of_digest,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::step::{BASE, DOMAIN_A, DOMAIN_Q, IVC_A, K, Side, Step, business};

/// A genuine `SHA256withECDSA` signature over the 32-byte encoding of `digest`.
fn signature(digest: u64, secret: u64, nonce: u64) -> SignatureWitness {
    let digest = Fp::from(digest);
    let hash = sha256_of_digest(&digest);
    let message = core::array::from_fn(|i| u64::from_be_bytes(hash[(3 - i) * 8..(4 - i) * 8].try_into().unwrap()));
    let secret = [secret, 0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321, 0x0123_4567];
    let nonce = [nonce, 0x0bad_cafe_dead_beef, 0x1357_9bdf_2468_ace0, 0x0765_4321];
    let key = native::mul(&Affine::GENERATOR, &secret).unwrap();
    let r = ORDER.reduce_once(&native::mul(&Affine::GENERATOR, &nonce).unwrap().x);
    let mut s = ORDER.mul(&ORDER.inverse(&nonce), &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)));
    if native::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(verify_prehashed(&message, &r, &s, &key));
    SignatureWitness { digest, key: [key.x, key.y], signature: [r, s] }
}

/// Receive `i`: the wallet's own key signs its receipt and its Request; a payer
/// key signs the incoming receipt.
fn leaf(i: u64) -> QSignatureCircuit {
    let slot = |mode| SignatureSlot { mode, key: SignatureKey::Variable };
    let plan = QSignaturePlan::new(vec![slot(VerifyMode::Hard), slot(VerifyMode::Soft), slot(VerifyMode::Soft)]).unwrap();
    let own = 0x5eed_0001;
    let witnesses = vec![
        signature(0x1111_0000 + i, own, 0xa100 + i),
        signature(0x2222_0000 + i, 0x5eed_1000 + i, 0xa200 + i),
        signature(0x3333_0000 + i, own, 0xa300 + i),
    ];
    QSignatureCircuit::new(plan, witnesses).unwrap()
}

/// The signature leaf beside a Q fold frame; the frame column is instance column
/// 0, the leaf's public column instance column 1.
#[derive(Clone)]
struct QStep {
    leaf: QSignatureCircuit,
    shape: FoldShape,
    witness: Option<FrameWitness<Eq>>,
}

#[derive(Clone, Debug)]
struct QStepConfig {
    frame: Column<InstanceColumn>,
    fold: FoldVerifierConfig<Eq>,
    leaf: QSignatureConfig,
}

impl Circuit<Fq> for QStep {
    type Config = QStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = <QSignatureCircuit as Circuit<Fq>>::Params;

    fn params(&self) -> Self::Params {
        self.leaf.params()
    }

    fn without_witnesses(&self) -> Self {
        Self { leaf: self.leaf.without_witnesses(), shape: self.shape.clone(), witness: None }
    }

    fn configure(_meta: &mut ConstraintSystem<Fq>) -> QStepConfig {
        unreachable!("configured with the leaf's parameters")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, params: Self::Params) -> QStepConfig {
        let frame = meta.instance_column(3);
        meta.enable_equality(frame);
        let fold = FoldVerifierConfig::configure(meta);
        let leaf = QSignatureCircuit::configure_with_params(meta, params);
        QStepConfig { frame, fold, leaf }
    }

    fn synthesize(&self, config: QStepConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "fold frame",
            |mut region| {
                let chain = Side::Q.chain();
                let frame = fold_frame(&mut chips, &mut region, &chain, &self.shape, Fq::from(BASE), 1, self.witness.as_ref())?;
                let later = chips.glue.not(&mut region, &frame.first)?;
                assert_equal_unless(&mut chips, &mut region, &frame.state[0], &frame.base, Some(&later))?;
                let next = chips.glue.linear(&mut region, &[(Fq::from(2), &frame.state[0])], Fq::ONE)?;
                frame_outputs(&mut chips, &mut region, &chain, &frame, &[next], &[frame.step.clone()])
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        self.leaf.synthesize(config.leaf, layouter.namespace(|| "signature leaf"))
    }
}

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
    let mut q_kept: Option<Kept<Eq>> = None; // A instances, kept by Q
    let mut a_kept: Option<Kept<Ep>> = None; // Q instances, kept by A
    let mut last_a: Option<Fresh<Eq>> = None;
    let mut last_q: Option<Fresh<Ep>> = None;
    let start = embed(&ivc::ivc_hash::<Fp>(&IVC_A, 0, Fp::from(BASE), &[Fp::from(BASE)], Fp::ZERO));
    for j in 1..=4u64 {
        let started = std::time::Instant::now();
        if j % 2 == 1 {
            let prepared = prepare_step(&q_chain, j, &rel.a, q_kept.as_ref(), last_a.as_ref(), vec![q_state], start).expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            let leaf = leaf(j);
            let leaf_public = leaf.instances(&[true, true, true]).expect("leaf instances")[0].clone();
            let frame = step_public(&q_chain, j, Fq::from(BASE), prepared.passthrough, &[next], prepared.digest, &[Fq::from(j)]);
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
            let prepared = prepare_step(&a_chain, j, &rel.q, a_kept.as_ref(), last_q.as_ref(), vec![a_state], Fp::ZERO).expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            let public = vec![step_public(&a_chain, j, Fp::from(BASE), prepared.passthrough, &[next], prepared.digest, &[Fp::from(j)])];
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
