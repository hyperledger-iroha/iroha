//! Folding a small circuit with every relation feature: selectors, a fixed
//! coefficient, a rotation, a degree-three gate, a tuple lookup into a fixed
//! table, two single-column lookups sharing one table (shared reciprocal cells), a
//! dynamic advice table, copies to an instance cell and to a constant.

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Witness,
    decide::{decide, decide_fresh},
    fold::{fold, fresh_instance, verify_fold},
};
use iroha_pasta::{Ep, Fq};
use iroha_plonk::{
    frontend::synthesize,
    pcs::ipa::PinnedParams,
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

fn fresh(setup: &Setup, seed: u64, rng: &mut ChaCha20Rng) -> (iroha_fold::Instance<Ep>, Witness<Fq>) {
    let toy = Toy { seed };
    let synthesized = synthesize(&toy, K, Some(&toy.instances::<Fq>())).expect("witness");
    let (advice, public) = setup.relation.stage_witness(&synthesized).expect("stage witness");
    fresh_instance(&setup.relation, &setup.key, &DOMAIN, advice, public, rng).expect("fresh instance")
}

#[test]
fn relation_shape() {
    let s = setup();
    assert_eq!(s.relation.n(), 1 << K);
    // Gates are degree 3; shared single-column cells (two range inputs) are
    // degree 1 + 2; the tuple lookup is 1 + 2.
    assert_eq!(s.relation.degree(), 3);
    // Three groups: pairs (tuple), the two range lookups (shared), cubes (dynamic).
    assert_eq!(s.relation.groups(), 3);
    assert_eq!(s.relation.theta_len(), 1);
}

#[test]
fn fresh_instances_decide() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    for seed in 1..4 {
        let (instance, witness) = fresh(&s, seed, &mut rng);
        decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).expect("genuine fresh instance");
    }
}

#[test]
fn folds_decide_and_verifier_matches_prover() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    let (first, mut witness) = fresh(&s, 1, &mut rng);
    let mut acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    decide(&s.relation, &s.key, &acc, &witness).expect("initial accumulator");
    for seed in 2..8 {
        let (instance, fresh_witness) = fresh(&s, seed, &mut rng);
        let (next, next_witness, proof) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &fresh_witness).expect("fold");
        let verified = verify_fold(&s.relation, &DOMAIN, &acc, &instance, &proof).expect("verify");
        assert_eq!(verified, next, "native verifier recomputes the prover's accumulator");
        decide(&s.relation, &s.key, &next, &next_witness).expect("folded accumulator decides");
        acc = next;
        witness = next_witness;
    }
}

/// Folds `bad` into a valid accumulator and returns whether the result decides.
fn folded_decides(s: &Setup, bad: (iroha_fold::Instance<Ep>, Witness<Fq>), rng: &mut ChaCha20Rng) -> bool {
    let (first, witness) = fresh(s, 1, rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    let (instance, fresh_witness) = bad;
    match fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &fresh_witness) {
        Ok((next, next_witness, _)) => decide(&s.relation, &s.key, &next, &next_witness).is_ok(),
        Err(_) => false,
    }
}

#[test]
fn a_false_gate_witness_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let (_, mut witness) = fresh(&s, 2, &mut rng);
    // Break the mul gate on row 4 (column e is outside every lookup) and recommit
    // honestly: the instance stays well formed, the relation is false.
    witness.columns[4][4] += Fq::ONE;
    let toy = Toy { seed: 2 };
    let public = toy.instances::<Fq>();
    let (instance, witness) = fresh_instance(&s.relation, &s.key, &DOMAIN, witness.columns[..6].to_vec(), public, &mut rng).expect("instance");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).is_err());
    assert!(!folded_decides(&s, (instance, witness), &mut rng));
}

#[test]
fn a_false_public_input_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(4);
    let toy = Toy { seed: 3 };
    let synthesized = synthesize(&toy, K, Some(&toy.instances::<Fq>())).expect("witness");
    let (advice, mut public) = s.relation.stage_witness(&synthesized).expect("stage witness");
    public[0][1] += Fq::ONE;
    let bad = fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, &mut rng).expect("instance");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &bad.0, &bad.1).is_err());
    assert!(!folded_decides(&s, bad, &mut rng));
}

#[test]
fn a_false_reciprocal_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    let (mut instance, mut witness) = fresh(&s, 4, &mut rng);
    // Change one reciprocal of the shared range group after its challenges and
    // recommit it: the round-one data, hence the challenges, are unchanged.
    // Columns: six advice, then per group m, h_0, g: pairs 6..=8, the shared range
    // group 9..=11, cubes 12..=14. Column 10 is the range group's h_0.
    assert_eq!(s.relation.columns(), 15);
    let column = 10;
    let row = witness.columns[column].iter().position(|v| !bool::from(v.is_zero())).expect("nonzero cell");
    witness.columns[column][row] += Fq::ONE;
    instance.commitments[column] = s.key.commit(&witness.columns[column], &witness.blinds[column]).expect("commit");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).is_err());
    assert!(!folded_decides(&s, (instance, witness), &mut rng));
}

#[test]
fn tampered_messages_or_challenges_are_rejected() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(6);
    let (first, witness) = fresh(&s, 1, &mut rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    let (instance, fresh_witness) = fresh(&s, 5, &mut rng);
    let (next, next_witness, proof) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &fresh_witness).expect("fold");
    // A changed F or K coefficient changes the verifier's accumulator; the honest
    // witness no longer decides it.
    for which in 0..2 {
        let mut bad = proof.clone();
        if which == 0 { bad.f[0] += Fq::ONE } else { bad.k[0] += Fq::ONE }
        let verified = verify_fold(&s.relation, &DOMAIN, &acc, &instance, &bad).expect("verify");
        assert_ne!(verified, next);
        assert!(decide(&s.relation, &s.key, &verified, &next_witness).is_err());
    }
    // Foreign LogUp challenges are refused before folding.
    let mut foreign = instance.clone();
    foreign.r[0] += Fq::ONE;
    assert!(verify_fold(&s.relation, &DOMAIN, &acc, &foreign, &proof).is_err());
    assert!(fold(&s.relation, &DOMAIN, &acc, &witness, &foreign, &fresh_witness).is_err());
    // Another domain derives other challenges, so the fold does not verify there.
    let other = *b"another relation domain of tests";
    assert!(verify_fold(&s.relation, &other, &acc, &instance, &proof).is_err());
}

/// Fresh instances, a fold and a decider v2 proof are the same bytes whether
/// the engine runs on one thread or on several.
#[test]
fn results_do_not_depend_on_the_thread_count() {
    use iroha_fold::decider_v2::{DeciderV2Plan, prove};
    let synthesized = synthesize(&Toy { seed: 1 }, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&synthesized).expect("relation");
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    let s = Setup { relation, key: CommitKey::monomial(params.params()) };
    let plan = DeciderV2Plan::new(s.relation.clone(), &s.key).expect("plan");
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("pool");
        pool.install(|| {
            let mut rng = ChaCha20Rng::seed_from_u64(77);
            let (first, w1) = fresh(&s, 1, &mut rng);
            let (second, w2) = fresh(&s, 2, &mut rng);
            let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first.clone()).expect("accumulator");
            let (folded, witness, proof) = fold(&s.relation, &DOMAIN, &acc, &w1, &second, &w2).expect("fold");
            let decided = prove(&plan, &DOMAIN, &folded, &witness).expect("decider");
            (first, second, folded, witness, proof, decided.proof, decided.claims)
        })
    };
    assert_eq!(run(1), run(4));
}
