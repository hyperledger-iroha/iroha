//! The relaxed-PLONK decider (v1) on the toy relation: its circuit is satisfied
//! exactly for accumulators the native decider accepts, and a PIPA-R proof of it
//! (the profile the in-circuit verifier admits) verifies.

use std::sync::Arc;

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Witness,
    decide::decide,
    decider::{Decider, DeciderPlan, decider_instances, decider_witness},
    fold::{fold, fresh_instance},
};
use iroha_pasta::{Ep, Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness,
    check::{CheckMode, check_circuit},
    cs::InstanceType,
    frontend::synthesize,
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    prove_circuit, prove_with_committed_advice,
    transcript::encode_point,
    verify_full,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::{K, Toy};

const DOMAIN: [u8; 32] = *b"iroha_fold toy relation domain!!";

struct Setup {
    relation: FoldedRelation<Fq>,
    key: CommitKey<Ep>,
}

fn setup() -> Setup {
    let synthesized = synthesize(&Toy { seed: 1 }, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&synthesized).expect("relation");
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    Setup { relation, key: CommitKey::lagrange(params.params()) }
}

fn fresh(s: &Setup, seed: u64, rng: &mut ChaCha20Rng) -> (iroha_fold::Instance<Ep>, Witness<Fq>) {
    let toy = Toy { seed };
    let synthesized = synthesize(&toy, K, Some(&toy.instances::<Fq>())).expect("witness");
    let (advice, public) = s.relation.stage_witness(&synthesized).expect("stage witness");
    fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, rng).expect("fresh instance")
}

/// An accumulator of three genuine instances.
fn accumulator(s: &Setup) -> (Accumulator<Ep>, Witness<Fq>) {
    let mut rng = ChaCha20Rng::seed_from_u64(31);
    let (first, mut witness) = fresh(s, 1, &mut rng);
    let mut acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    for seed in 2..4 {
        let (instance, w) = fresh(s, seed, &mut rng);
        let (next, next_w, _) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &w).expect("fold");
        acc = next;
        witness = next_w;
    }
    decide(&s.relation, &s.key, &acc, &witness).expect("genuine accumulator");
    (acc, witness)
}

fn satisfied(plan: &Arc<DeciderPlan<Fq>>, acc: &Accumulator<Ep>, witness: &Witness<Fq>, instances: &[Vec<Fq>]) -> bool {
    let circuit = Decider::new(Arc::clone(plan), decider_witness(plan, acc, witness));
    check_circuit(&circuit, K, instances, CheckMode::Strict).expect("synthesis").is_satisfied()
}

#[test]
fn the_decider_circuit_accepts_exactly_deciding_accumulators() {
    let s = setup();
    let plan = Arc::new(DeciderPlan::new(s.relation.clone()).expect("plan"));
    let (acc, witness) = accumulator(&s);
    let instances = decider_instances(&plan, &acc);
    let circuit = Decider::new(Arc::clone(&plan), decider_witness(&plan, &acc, &witness));
    let report = check_circuit(&circuit, K, &instances, CheckMode::Strict).expect("synthesis");
    assert!(report.is_satisfied(), "{report}");

    // Another accumulated error.
    let mut wrong = instances.clone();
    let e = wrong.last_mut().and_then(|s| s.last_mut()).expect("e");
    *e += Fq::ONE;
    assert!(!satisfied(&plan, &acc, &witness, &wrong));
    // Another witness cell (column a, row 5): a gate no longer holds as accumulated.
    let mut bad = witness.clone();
    bad.columns[0][5] += Fq::ONE;
    assert!(!satisfied(&plan, &acc, &bad, &instances));
    // Another folded public input.
    let mut wrong = instances.clone();
    wrong[0][0] += Fq::ONE;
    assert!(!satisfied(&plan, &acc, &witness, &wrong));
}

#[test]
fn an_accumulator_that_folded_a_false_instance_has_no_satisfying_decider() {
    let s = setup();
    let plan = Arc::new(DeciderPlan::new(s.relation.clone()).expect("plan"));
    let (acc, witness) = accumulator(&s);
    // A false gate witness (column e, row 4), recommitted honestly.
    let mut rng = ChaCha20Rng::seed_from_u64(32);
    let (_, mut w) = fresh(&s, 5, &mut rng);
    w.columns[4][4] += Fq::ONE;
    let public = Toy { seed: 5 }.instances::<Fq>();
    let (instance, w) =
        fresh_instance(&s.relation, &s.key, &DOMAIN, w.columns[..6].to_vec(), public, &mut rng).expect("instance");
    let (next, next_w, _) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &w).expect("fold");
    assert!(decide(&s.relation, &s.key, &next, &next_w).is_err());
    assert!(!satisfied(&plan, &next, &next_w, &decider_instances(&plan, &next)));
}

#[test]
fn a_pipa_r_proof_of_the_decider_verifies() {
    let s = setup();
    let plan = Arc::new(DeciderPlan::new(s.relation.clone()).expect("plan"));
    let (acc, witness) = accumulator(&s);
    let instances = decider_instances(&plan, &acc);
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    let config = KeygenConfigV2::pipa_r(vec![InstanceType::Field; instances.len()]);
    let pk = keygen_pk_v2(&params, &Decider::layout(Arc::clone(&plan)), &config).expect("proving key");
    let circuit = Decider::new(Arc::clone(&plan), decider_witness(&plan, &acc, &witness));
    let proof = prove_circuit(&params, &pk, &circuit, &instances, ProverRandomness::hedged(), ProverConfig::default())
        .expect("proof");
    assert_eq!(verify_full(&params, pk.binding(), pk.vk(), &instances, &proof, MemoryBudget::DEFAULT), Ok(()));
    let mut wrong = instances;
    let e = wrong.last_mut().and_then(|s| s.last_mut()).expect("e");
    *e += Fq::ONE;
    assert!(verify_full(&params, pk.binding(), pk.vk(), &wrong, &proof, MemoryBudget::DEFAULT).is_err());
}

#[test]
fn a_committed_advice_decider_proof_carries_the_accumulator_commitments() {
    let s = setup();
    let plan = Arc::new(DeciderPlan::new(s.relation.clone()).expect("plan"));
    let (acc, witness) = accumulator(&s);
    let instances = decider_instances(&plan, &acc);
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    let config = KeygenConfigV2::pipa_r(vec![InstanceType::Field; instances.len()]);
    let pk = keygen_pk_v2(&params, &Decider::layout(Arc::clone(&plan)), &config).expect("proving key");
    // The decider's advice columns as synthesized, with the accumulator's witness
    // columns (decider advice columns 0..W) given verbatim with their blinds.
    let circuit = Decider::new(Arc::clone(&plan), decider_witness(&plan, &acc, &witness));
    let synthesized = synthesize(&circuit, K, Some(&instances)).expect("witness");
    let mut advice = synthesized.tables.advice().expect("advice").to_vec();
    let mut committed = vec![None; advice.len()];
    for (c, column) in witness.columns.iter().enumerate() {
        advice[c].clone_from(column);
        committed[c] = Some(witness.blinds[c]);
    }
    let output = prove_with_committed_advice(
        &params,
        &pk,
        advice,
        committed,
        instances.clone(),
        ProverRandomness::hedged(),
        ProverConfig::default(),
    )
    .expect("committed-advice proof");
    assert_eq!(verify_full(&params, pk.binding(), pk.vk(), &instances, &output.proof, MemoryBudget::DEFAULT), Ok(()));
    // Proof messages 0..W are the advice commitments, in column order.
    for (c, commitment) in acc.instance.commitments.iter().enumerate() {
        assert_eq!(&output.proof[32 * c..32 * (c + 1)], &encode_point::<Ep>(commitment)[..], "column {c}");
    }
}
