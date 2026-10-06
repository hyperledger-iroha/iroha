//! Real Bootstrap state-effect sigma: hostile rehashed states and native proof.

#[path = "common/bootstrap.rs"]
mod bootstrap;
mod common;

use ff::Field;
use iroha_kagemusha_proof::{
    a_relation::bootstrap::BootstrapPolicy,
    admin_sigma::{BOOTSTRAP_K, BootstrapCircuit},
    witness::core_index,
};
use iroha_pasta::{Fp, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    frontend::{Circuit, configure, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    verifier::accumulate_generator,
};

#[test]
fn bootstrap_policy_rejects_invalid_fixed_artifacts() {
    use iroha_plonk_gadgets::p256::native::{Affine, P};
    assert!(BootstrapPolicy::new([1, 2], [3, 4], Affine::GENERATOR).is_ok());
    assert!(BootstrapPolicy::new([0; 2], [3, 4], Affine::GENERATOR).is_err());
    assert!(BootstrapPolicy::new([1, 2], [0; 2], Affine::GENERATOR).is_err());
    assert!(BootstrapPolicy::new([1, 2], [3, 4], Affine { x: P, y: [0; 4] }).is_err());
}

#[test]
fn bootstrap_initial_state_and_rehashed_invalid_states() {
    let witness = bootstrap::witness();
    let circuit = BootstrapCircuit::new(&witness);
    let instances = circuit.instances();
    assert!(
        check_circuit(&circuit, BOOTSTRAP_K, &instances, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&circuit, BOOTSTRAP_K, Some(&instances)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), BOOTSTRAP_K, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let lanes: Vec<_> = known
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().rposition(|v| *v).map_or(0, |row| row + 1))
        .collect();
    eprintln!(
        "genuine Bootstrap sigma k{BOOTSTRAP_K}: rows={lanes:?}, cells={}",
        known
            .tables
            .advice_assigned()
            .iter()
            .flatten()
            .filter(|v| **v)
            .count()
    );
    assert_eq!(lanes.len(), 5);
    let (cs, _) = configure::<Fp, BootstrapCircuit>(&circuit).unwrap();
    assert_eq!(cs.instance_lengths(), &[1]);
    for index in (core_index::BALANCE..=core_index::BLACKLIST_ISSUED_AT).chain([
        core_index::POLICY_EPOCH,
        core_index::TIME_FLOOR,
        core_index::STATE_NONCE,
    ]) {
        let mut wrong = witness;
        if index == core_index::STATE_NONCE {
            wrong.core[index] = Fp::ZERO;
        } else {
            wrong.core[index] += Fp::ONE;
        }
        bootstrap::rebind(&mut wrong);
        let bad = BootstrapCircuit::new(&wrong);
        assert!(
            !check_circuit(&bad, BOOTSTRAP_K, &bad.instances(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "rehashed core {index}"
        );
    }
    for index in 1..8 {
        let mut wrong = witness;
        wrong.rest[index] += Fp::ONE;
        bootstrap::rebind(&mut wrong);
        let bad = BootstrapCircuit::new(&wrong);
        assert!(
            !check_circuit(&bad, BOOTSTRAP_K, &bad.instances(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "rehashed rest {index}"
        );
    }
    let mut wrong = instances;
    wrong[0][0] += Fp::ONE;
    assert!(
        !check_circuit(&circuit, BOOTSTRAP_K, &wrong, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn native_bootstrap_sigma_proof_and_complete_opening() {
    let circuit = BootstrapCircuit::new(&bootstrap::witness());
    let instances = circuit.instances();
    let params = common::vesta_params(BOOTSTRAP_K);
    let key = keygen_pk_v2(
        &params,
        &circuit,
        &KeygenConfigV2::pipa_r(BootstrapCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        Witness::from_circuit(&key, &circuit, &instances).unwrap(),
        common::recovery(188),
        ProverConfig::default(),
    )
    .unwrap();
    let opening = accumulate_generator(
        &params,
        key.binding(),
        key.vk(),
        &instances,
        &proof.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    opening.decide(&params, MemoryBudget::DEFAULT).unwrap();
    assert_eq!(opening.g(), proof.opening.g());
    eprintln!("genuine Bootstrap sigma: {} proof bytes", proof.proof.len());
    let mut wrong = instances;
    wrong[0][0] += Fp::ONE;
    assert!(
        accumulate_generator(
            &params,
            key.binding(),
            key.vk(),
            &wrong,
            &proof.proof,
            MemoryBudget::DEFAULT
        )
        .is_err()
    );
}

fn load_witness() -> iroha_kagemusha_proof::admin_sigma::LoadWitness {
    use iroha_kagemusha_proof::admin_sigma::{LoadWitness, StateWitness};
    let before = bootstrap::witness();
    let mut after = before;
    after.core[core_index::BALANCE] = Fp::from(100);
    after.core[core_index::NEXT_LOAD] = Fp::ONE;
    after.core[core_index::SEQUENCE] = Fp::ONE;
    after.core[core_index::STATE_NONCE] += Fp::ONE;
    // A must prove the recovery insertion; sigma binds this carried root.
    after.core[core_index::LOAD_REDEEM_ROOT] = Fp::from(99);
    bootstrap::rebind(&mut after);
    let mut statement = after.statement;
    statement[14] = before.lineage[5];
    statement[16] = Fp::from(2);
    statement[17..].fill(Fp::ZERO);
    statement[17] = Fp::from(55);
    statement[19] = Fp::from(100);
    statement[20] = Fp::from(3);
    LoadWitness {
        predecessor: StateWitness::from(&before),
        successor: StateWitness::from(&after),
        statement,
    }
}
fn rebind_load(w: &mut iroha_kagemusha_proof::admin_sigma::LoadWitness) {
    use iroha_kagemusha_proof::witness::{CORE_DOMAIN, REST_DOMAIN};
    use iroha_pasta::poseidon::hash_with_domain;
    for state in [&mut w.predecessor, &mut w.successor] {
        let mut preimage = state.core.to_vec();
        preimage.push(hash_with_domain(REST_DOMAIN, &state.rest));
        state.lineage[5] = hash_with_domain(CORE_DOMAIN, &preimage);
        state.lineage[1..3].copy_from_slice(&state.core[1..3]);
        state.lineage[6..8].copy_from_slice(&state.core[5..7]);
        state.lineage[8] = state.core[7];
        state.lineage[13] = state.core[0]
            + Fp::from(256) * state.core[30]
            + Fp::from(2).pow_vartime([72]) * state.core[21];
    }
    w.statement[3..7].copy_from_slice(&w.successor.core[1..5]);
    for (field, index) in [(7, 7), (8, 0), (9, 10), (10, 12)] {
        w.statement[field] = w.successor.core[index];
    }
    w.statement[14] = w.predecessor.lineage[5];
    w.statement[15] = w.successor.lineage[5];
}
#[test]
fn load_sigma_constrains_value_continuity_and_exact_changes() {
    use iroha_kagemusha_proof::admin_sigma::LoadCircuit;
    let witness = load_witness();
    let circuit = LoadCircuit::new(&witness);
    let instances = circuit.instances();
    assert!(
        check_circuit(&circuit, 12, &instances, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&circuit, 12, Some(&instances)).unwrap();
    let unknown = synthesize(&circuit.without_witnesses(), 12, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let lanes: Vec<_> = known
        .tables
        .advice_assigned()
        .iter()
        .map(|column| column.iter().rposition(|v| *v).map_or(0, |row| row + 1))
        .collect();
    eprintln!(
        "Load sigma k12 rows={lanes:?}, cells={}",
        known
            .tables
            .advice_assigned()
            .iter()
            .flatten()
            .filter(|v| **v)
            .count()
    );
    assert_eq!(lanes.len(), 5);
    for index in 0..33 {
        if [core_index::STATE_NONCE, core_index::LOAD_REDEEM_ROOT].contains(&index) {
            continue;
        }
        let mut wrong = witness;
        wrong.successor.core[index] += Fp::ONE;
        rebind_load(&mut wrong);
        let bad = LoadCircuit::new(&wrong);
        assert!(
            !check_circuit(&bad, 12, &bad.instances(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "rehashed Load core{index}"
        );
    }
    for index in 0..8 {
        let mut wrong = witness;
        wrong.successor.rest[index] += Fp::ONE;
        rebind_load(&mut wrong);
        let bad = LoadCircuit::new(&wrong);
        assert!(
            !check_circuit(&bad, 12, &bad.instances(), CheckMode::Strict)
                .is_ok_and(|r| r.is_satisfied()),
            "rehashed Load rest{index}"
        );
    }
    let mut wrong = witness;
    wrong.statement[18] = Fp::ONE;
    let bad = LoadCircuit::new(&wrong);
    assert!(
        !check_circuit(&bad, 12, &bad.instances(), CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}
#[test]
fn native_load_sigma_proof_and_complete_opening() {
    use iroha_kagemusha_proof::admin_sigma::LoadCircuit;
    let circuit = LoadCircuit::new(&load_witness());
    let instances = circuit.instances();
    let params = common::vesta_params(12);
    let key = keygen_pk_v2(
        &params,
        &circuit,
        &KeygenConfigV2::pipa_r(LoadCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        Witness::from_circuit(&key, &circuit, &instances).unwrap(),
        common::recovery(189),
        ProverConfig::default(),
    )
    .unwrap();
    let opening = accumulate_generator(
        &params,
        key.binding(),
        key.vk(),
        &instances,
        &proof.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    opening.decide(&params, MemoryBudget::DEFAULT).unwrap();
    eprintln!("Load sigma proof {} bytes", proof.proof.len());
    let mut wrong = instances;
    wrong[0][0] += Fp::ONE;
    assert!(
        accumulate_generator(
            &params,
            key.binding(),
            key.vk(),
            &wrong,
            &proof.proof,
            MemoryBudget::DEFAULT
        )
        .is_err()
    );
}
