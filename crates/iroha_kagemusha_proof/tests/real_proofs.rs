//! The M7 relation checks as real PIPA-v1 proofs (KAGEMUSHA step format on
//! Vesta, at the shape within the 3.5 KB budget):
//!
//! - an honest witness is proved and verified; a wrong public output is
//!   rejected; a fixed recovery stream reproduces the proof bytes and a
//!   different one changes them;
//! - a mutated witness (overdraft, newer policy epoch, early accepted time,
//!   `u128` overflow) is refused by [`SigmaProver::prove`] with its
//!   violation, and the engine itself refuses it (a limb lookup input is
//!   missing from its table);
//! - a forger who zeroes the failing range checks so every lookup passes
//!   gets a proof from the engine, and the verifier rejects it (the zeroed
//!   check breaks the copy from the checked value);
//! - the honest proof of the same seed does not verify for the mutated
//!   statement.
//!
//! Every byte of a proof, flipped, is rejected; a proof on Pallas verifies
//! too. All tests here prove for real and are ignored in debug builds.

mod common;

use common::{
    CHECK_SEED, RELATION_CASES, budget_shape, case_label, forged_witness, recovery,
    relation_shapes, vesta_prover,
};
use ff::Field;
use iroha_kagemusha_proof::{
    Mutation, PrefixMode, ProofFormat, RelationShape, SigmaError, SigmaProver, StateLayout,
    StepRelation, Violation, sample_witness,
};
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{ProverConfig, ProverError, create_proof, prove_circuit};
use rayon::prelude::*;

#[test]
#[ignore = "24 cases with real proofs; run in release"]
fn m7_relation_checks_as_real_proofs() {
    let mut cases = 0;
    for relation in relation_shapes() {
        let shape = budget_shape(relation);
        let prover = vesta_prover(shape);
        let verifier = prover.verifier();
        let honest = sample_witness::<Fp>(CHECK_SEED, relation.step, Mutation::None);
        let honest_proof = prover.prove(&honest, recovery(1)).expect("honest proof");
        for (step, mutation, expected) in RELATION_CASES {
            if step != relation.step {
                continue;
            }
            let label = case_label(relation, mutation);
            let witness = sample_witness::<Fp>(CHECK_SEED, step, mutation);
            if expected {
                let proof = prover.prove(&witness, recovery(2)).expect("proof");
                assert_eq!(
                    proof.bytes.len(),
                    shape
                        .proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP)
                        .expect("length")
                );
                assert_eq!(
                    verifier.verify(&proof.public, &proof.bytes),
                    Ok(()),
                    "{label}"
                );
                let mut wrong = proof.public;
                wrong.statement += Fp::ONE;
                assert!(verifier.verify(&wrong, &proof.bytes).is_err(), "{label}");
                if let Some(request) = &mut wrong.request {
                    wrong.statement = proof.public.statement;
                    *request += Fp::ONE;
                    assert!(verifier.verify(&wrong, &proof.bytes).is_err(), "{label}");
                }
                assert_eq!(
                    prover.prove(&witness, recovery(2)).expect("proof").bytes,
                    proof.bytes,
                    "{label}: a fixed stream reproduces the proof"
                );
                assert_ne!(honest_proof.bytes, proof.bytes, "{label}: fresh blinds");
                println!(
                    "M12_REAL case={} k={} lanes={} mutation={mutation:?} accepted=true bytes={}",
                    relation.label(),
                    shape.k,
                    shape.params.lanes(),
                    proof.bytes.len()
                );
            } else {
                // The library refuses the witness with its violation.
                match prover.prove(&witness, recovery(2)) {
                    Err(SigmaError::RelationViolated(violations)) => {
                        assert!(!violations.is_empty(), "{label}");
                    }
                    other => panic!("{label}: {other:?}"),
                }
                // The engine refuses it too: a limb is missing from its table.
                let circuit = prover.circuit(&witness).expect("circuit");
                let claimed = witness.evaluate(relation.layout).public();
                let engine = prove_circuit(
                    prover.params(),
                    prover.proving_key(),
                    &circuit,
                    &[claimed.instance()],
                    recovery(3),
                    ProverConfig::default(),
                );
                assert!(
                    matches!(engine, Err(ProverError::LookupInputMissing { lookup: 0 })),
                    "{label}: {engine:?}"
                );
                // A forger zeroes the failing range checks: the engine proves,
                // the verifier rejects.
                let (forged, instance, failures) = forged_witness(&prover, &witness);
                let forged_proof = create_proof(
                    prover.params(),
                    prover.proving_key(),
                    &forged,
                    recovery(4),
                    ProverConfig::default(),
                )
                .expect("the engine proves the forged witness");
                let verdict = verifier.verify(&claimed, &forged_proof);
                assert!(verdict.is_err(), "{label}: forged proof accepted");
                assert_eq!(claimed.instance(), instance);
                // The honest proof does not verify for the mutated statement.
                assert!(
                    verifier.verify(&claimed, &honest_proof.bytes).is_err(),
                    "{label}"
                );
                println!(
                    "M12_REAL case={} k={} lanes={} mutation={mutation:?} accepted=false \
                     prove=refused engine=LookupInputMissing forged={verdict:?} \
                     checker_failures={}",
                    relation.label(),
                    shape.k,
                    shape.params.lanes(),
                    failures.len()
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 24);
}

/// Every byte of a proof, flipped in its lowest and highest bit, is rejected.
fn every_byte_is_bound(relation: RelationShape) -> usize {
    let shape = budget_shape(relation);
    let prover = vesta_prover(shape);
    let verifier = prover.verifier();
    let witness = sample_witness::<Fp>(CHECK_SEED, relation.step, Mutation::None);
    let proof = prover.prove(&witness, recovery(9)).expect("proof");
    assert_eq!(verifier.verify(&proof.public, &proof.bytes), Ok(()));
    let accepted: Vec<(usize, u8)> = (0..proof.bytes.len())
        .into_par_iter()
        .flat_map_iter(|index| [(index, 0x01_u8), (index, 0x80_u8)])
        .filter(|(index, mask)| {
            let mut tampered = proof.bytes.clone();
            tampered[*index] ^= mask;
            verifier.verify(&proof.public, &tampered).is_ok()
        })
        .collect();
    assert!(
        accepted.is_empty(),
        "{}: accepted {accepted:?}",
        relation.label()
    );
    // Truncated and extended proofs are rejected too.
    let mut longer = proof.bytes.clone();
    longer.push(0);
    assert!(verifier.verify(&proof.public, &longer).is_err());
    assert!(
        verifier
            .verify(&proof.public, &proof.bytes[..proof.bytes.len() - 1])
            .is_err()
    );
    proof.bytes.len()
}

#[test]
#[ignore = "two flips per proof byte, each a full verification; run in release"]
fn every_proof_byte_is_bound() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let relation = RelationShape::new(step, StateLayout::TwoLevel, PrefixMode::Folded);
        let bytes = every_byte_is_bound(relation);
        println!(
            "M12_TAMPER_BYTES case={} bytes={bytes} flips={} accepted=0",
            relation.label(),
            2 * bytes
        );
    }
}

#[test]
#[ignore = "real proofs on Pallas; run in release"]
fn pallas_step_proofs_verify() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let relation = RelationShape::new(step, StateLayout::TwoLevel, PrefixMode::Folded);
        let shape = budget_shape(relation);
        let prover =
            SigmaProver::<Ep>::keygen(shape, ProofFormat::KAGEMUSHA_STEP).expect("Pallas keys");
        let witness = sample_witness::<Fq>(CHECK_SEED, step, Mutation::None);
        let proof = prover.prove(&witness, recovery(5)).expect("Pallas proof");
        assert_eq!(
            prover.verifier().verify(&proof.public, &proof.bytes),
            Ok(())
        );
        assert_eq!(
            proof.bytes.len(),
            shape
                .proof_length::<Ep>(ProofFormat::KAGEMUSHA_STEP)
                .expect("length")
        );
        let mutated = sample_witness::<Fq>(
            CHECK_SEED,
            step,
            match step {
                StepRelation::Send => Mutation::Overdraft,
                StepRelation::Receive => Mutation::Overflow,
            },
        );
        assert!(matches!(
            prover.prove(&mutated, recovery(5)),
            Err(SigmaError::RelationViolated(violations))
                if violations.contains(&Violation::Overdraft)
                    || violations.contains(&Violation::BalanceOverflow)
        ));
    }
}

/// A witness of the other step is refused before proving.
#[test]
#[ignore = "key generation; run in release"]
fn witnesses_of_the_other_step_are_refused() {
    let relation = RelationShape::new(
        StepRelation::Receive,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let prover = vesta_prover(budget_shape(relation));
    let send = sample_witness::<Fp>(1, StepRelation::Send, Mutation::None);
    assert_eq!(
        prover.prove(&send, recovery(1)).map(|proof| proof.bytes),
        Err(SigmaError::WrongRelation {
            expected: StepRelation::Receive,
            found: StepRelation::Send
        })
    );
    let receive = sample_witness::<Fp>(1, StepRelation::Receive, Mutation::None);
    let proof = prover.prove(&receive, recovery(1)).expect("proof");
    let mut with_request = proof.public;
    with_request.request = Some(Fp::ZERO);
    assert_eq!(
        prover.verifier().verify(&with_request, &proof.bytes),
        Err(SigmaError::PublicShape)
    );
}
