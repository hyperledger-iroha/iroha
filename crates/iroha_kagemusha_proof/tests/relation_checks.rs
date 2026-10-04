//! The M7 relation checks (`m7_relation_checks`, 24 cases) in the strict
//! constraint checker: both steps, both state layouts and both Poseidon
//! prefix modes (the analogue of M7's two Poseidon backends: two different
//! circuits computing identical digests). Each honest witness is accepted;
//! an overdraft, a newer Request policy epoch, an accepted time below the
//! floor and a `u128` overflow are rejected, and every rejection is a limb
//! lookup of a range check (the relation's checked arithmetic), never a
//! digest or copy mismatch.
//!
//! The per-cell tamper suite (`iroha_plonk_gadgets::tamper`) runs on every
//! honest case in release (ignored here): no assigned advice cell of the
//! composed relation is free.

mod common;

use common::{
    CHECK_SEED, RELATION_CASES, case_label, check_witness, check_witness_of, relation_shapes,
    smallest_shape,
};
use iroha_kagemusha_proof::{
    Mutation, PrefixMode, RelationShape, SigmaCircuit, StateLayout, StepRelation, Violation,
    sample_witness,
};
use iroha_pasta::{Fp, Fq, poseidon::PoseidonField};
use iroha_plonk::check::CheckFailure;
use iroha_plonk_gadgets::tamper::undetected_tampers;

/// The violation the native reference must report for a mutation.
const fn expected_violation(mutation: Mutation) -> Option<Violation> {
    match mutation {
        Mutation::None => None,
        Mutation::Overdraft => Some(Violation::Overdraft),
        Mutation::Overflow => Some(Violation::BalanceOverflow),
        Mutation::StaleEpoch => Some(Violation::PolicyEpochNewer),
        Mutation::EarlyTime => Some(Violation::AcceptedTimeBelowFloor),
    }
}

/// Runs the 24 cases on field `F` and returns how many were checked.
fn relation_checks<F: PoseidonField>() -> usize {
    let mut cases = 0;
    for relation in relation_shapes() {
        let shape = smallest_shape(relation);
        for (step, mutation, expected) in RELATION_CASES {
            if step != relation.step {
                continue;
            }
            let witness = sample_witness::<F>(CHECK_SEED, step, mutation);
            let native = witness.evaluate(relation.layout);
            assert_eq!(
                native.is_honest(),
                expected,
                "{}",
                case_label(relation, mutation)
            );
            if let Some(violation) = expected_violation(mutation) {
                assert!(native.violations.contains(&violation));
            }
            let report = check_witness(&shape, &witness);
            println!(
                "M12_CHECK case={} k={} lanes={} mutation={mutation:?} accepted={} expected={expected} failures={}",
                relation.label(),
                shape.k,
                shape.params.lanes(),
                report.is_satisfied(),
                report.failures().len()
            );
            assert_eq!(
                report.is_satisfied(),
                expected,
                "{}: {:?}",
                case_label(relation, mutation),
                report.failures()
            );
            for failure in report.failures() {
                assert!(
                    matches!(failure, CheckFailure::LookupInputMissing { lookup: 0, .. }),
                    "{}: {failure}",
                    case_label(relation, mutation)
                );
            }
            cases += 1;
        }
    }
    cases
}

#[test]
fn m7_relation_checks_in_the_constraint_checker() {
    assert_eq!(relation_checks::<Fp>(), 24);
}

#[test]
#[ignore = "the 24 cases on the Pallas scalar field; run in release"]
fn m7_relation_checks_on_the_other_field() {
    assert_eq!(relation_checks::<Fq>(), 24);
}

/// A witness edit of [`every_relation_rule_is_enforced`].
type Edit = fn(&mut iroha_kagemusha_proof::StepWitness<Fp>);

/// One tamper sweep result: the case, its assigned cells and the free ones.
type TamperResult = (String, usize, Vec<(usize, usize)>);

/// Every relation rule, broken alone, is rejected by the checker (beyond
/// the four M7 mutations).
#[test]
fn every_relation_rule_is_enforced() {
    let relation = RelationShape::new(
        StepRelation::Send,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let shape = smallest_shape(relation);
    let honest = check_witness_of(StepRelation::Send, Mutation::None);
    let edits: [(&str, Violation, Edit); 6] = [
        ("lifecycle", Violation::LifecycleNotActive, |w| {
            w.predecessor.core.lifecycle = 2;
        }),
        ("sequence", Violation::SequenceOverflow, |w| {
            w.predecessor.core.sequence = u128::MAX;
        }),
        ("ordinal", Violation::SendOrdinalOverflow, |w| {
            w.predecessor.core.next_send = u128::MAX;
        }),
        ("amount", Violation::ZeroAmount, |w| {
            if let iroha_kagemusha_proof::StepInputs::Send(send) = &mut w.inputs {
                send.amount = 0;
            }
        }),
        ("request time", Violation::AcceptedTimeBelowRequest, |w| {
            if let iroha_kagemusha_proof::StepInputs::Send(send) = &mut w.inputs {
                send.request_time = send.accepted_lower + 1;
            }
        }),
        ("window", Violation::AcceptedWindowInverted, |w| {
            if let iroha_kagemusha_proof::StepInputs::Send(send) = &mut w.inputs {
                send.accepted_upper = send.accepted_lower - 1;
            }
        }),
    ];
    for (name, violation, edit) in edits {
        let mut witness = honest.clone();
        edit(&mut witness);
        assert_eq!(
            witness.evaluate(relation.layout).violations,
            vec![violation],
            "{name}"
        );
        let report = check_witness(&shape, &witness);
        assert!(!report.is_satisfied(), "{name} accepted");
    }
    // A lifecycle that is not Active fails its constant copy, not a lookup.
    let mut inactive = honest.clone();
    inactive.predecessor.core.lifecycle = 2;
    assert!(
        check_witness(&shape, &inactive)
            .failures()
            .iter()
            .any(|failure| matches!(failure, CheckFailure::CopyMismatch { .. }))
    );
    // A zero amount fails the nonzero gate.
    let receive = RelationShape::new(
        StepRelation::Receive,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let mut zero = check_witness_of(StepRelation::Receive, Mutation::None);
    if let iroha_kagemusha_proof::StepInputs::Receive(inputs) = &mut zero.inputs {
        inputs.amount = 0;
    }
    let report = check_witness(&smallest_shape(receive), &zero);
    assert!(
        report
            .failures()
            .iter()
            .any(|failure| matches!(failure, CheckFailure::ConstraintNotSatisfied { .. })),
        "{report:?}"
    );
}

/// A wrong public output is rejected by the instance copy.
#[test]
fn wrong_public_outputs_are_rejected() {
    use ff::Field;
    use iroha_plonk::check::{CheckMode, check_circuit};
    let relation = RelationShape::new(
        StepRelation::Send,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let shape = smallest_shape(relation);
    let witness = check_witness_of(StepRelation::Send, Mutation::None);
    let public = witness.evaluate(relation.layout).public().instance();
    let circuit = SigmaCircuit::new(shape.params, witness);
    for index in 0..public.len() {
        let mut wrong = public.clone();
        wrong[index] += Fp::ONE;
        let report =
            check_circuit(&circuit, shape.k, &[wrong], CheckMode::Strict).expect("synthesis");
        assert!(!report.is_satisfied(), "output {index}");
    }
}

/// The per-cell tamper suite on every honest case: each assigned advice
/// cell, shifted alone, makes the strict checker fail.
#[test]
#[ignore = "one synthesis and check per assigned cell (about 5-13k per case); run in release"]
fn honest_cases_have_no_free_cells() {
    use rayon::prelude::*;
    let honest: Vec<RelationShape> = relation_shapes();
    let results: Vec<TamperResult> = honest
        .par_iter()
        .map(|relation| {
            let shape = smallest_shape(*relation);
            let witness = sample_witness::<Fp>(CHECK_SEED, relation.step, Mutation::None);
            let public = witness.evaluate(relation.layout).public().instance();
            let circuit = SigmaCircuit::new(shape.params, witness);
            let cells = iroha_plonk_gadgets::tamper::assigned_advice_cells(
                &circuit,
                shape.k,
                core::slice::from_ref(&public),
            )
            .expect("cells");
            let undetected =
                undetected_tampers(&circuit, shape.k, &[public]).expect("tamper sweep");
            (relation.label(), cells.len(), undetected)
        })
        .collect();
    for (label, cells, undetected) in results {
        println!(
            "M12_TAMPER case={label} cells={cells} undetected={}",
            undetected.len()
        );
        assert!(undetected.is_empty(), "{label}: free cells {undetected:?}");
    }
}
