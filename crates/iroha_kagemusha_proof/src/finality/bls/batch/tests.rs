//! Complete trace coverage, fixed cursor binding and all-class capacity checks.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn batches() -> Vec<BlsBatchCircuit> {
    let context = super::super::tests::context();
    prepare_bls_batches(context.message, context.public_key, context.signature).unwrap()
}
fn accepts(source: &BlsBatchCircuit) -> bool {
    source.instances().is_ok_and(|public| {
        check_circuit(source, 16, &public, CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
    })
}

#[test]
fn paired_trace_preserves_every_original_operation_and_boundary() {
    let context = super::super::tests::context();
    let original =
        prepare_bls_witness(context.message, context.public_key, context.signature).unwrap();
    let paired = batches();
    assert_eq!(paired.len(), usize::try_from(BlsBatchPlan::LENGTH).unwrap());
    assert_eq!(2 * paired.len(), original.len());
    for (position, (pair, original)) in paired.iter().zip(original.chunks_exact(2)).enumerate() {
        assert_eq!(pair.plan.ordinal(), u32::try_from(position).unwrap());
        for (left, right) in pair.leaves.iter().zip(original) {
            assert_eq!(left.plan, right.plan);
            assert_eq!(left.context.digest(), right.context.digest());
            assert_eq!(left.before.tag(), right.before.tag());
            assert_eq!(left.before.words(), right.before.words());
            assert_eq!(left.after.tag(), right.after.tag());
            assert_eq!(left.after.words(), right.after.words());
        }
        let first = original[0].endpoints();
        let last = original[1].endpoints();
        assert_eq!(
            pair.endpoints(),
            [first[0], first[1], first[2], last[3], first[4], last[5]]
        );
    }
    let first = paired.first().unwrap().endpoints();
    let last = paired.last().unwrap().endpoints();
    assert_eq!(first[2], Fp::ZERO);
    assert_eq!(last[3], Fp::from(u64::from(BlsLeafPlan::LENGTH)));
    assert_eq!(first[4], boundary_digest_native(first[1], false));
    assert_eq!(last[5], boundary_digest_native(last[1], true));
    for pair in paired.windows(2) {
        let a = pair[0].endpoints();
        let b = pair[1].endpoints();
        assert_eq!(a[..2], b[..2]);
        assert_eq!(a[3], b[2]);
        assert_eq!(a[5], b[4]);
    }
    assert!(BlsBatchPlan::at(BlsBatchPlan::LENGTH).is_none());
    assert!(BlsBatchPlan::at(u32::MAX).is_none());
}

#[test]
fn cursor_pair_binding_rejects_drop_reorder_and_foreign_context() {
    let paired = batches();
    for position in [0, 1, 107, paired.len() - 1] {
        let source = &paired[position];
        assert!(accepts(source));
        let mut reversed = source.clone();
        reversed.leaves.swap(0, 1);
        assert!(!accepts(&reversed));
        let mut dropped = source.clone();
        dropped.leaves[1] = dropped.leaves[0].clone();
        assert!(!accepts(&dropped));
        let mut relabeled = source.clone();
        relabeled.plan = paired[(position + 1) % paired.len()].plan;
        assert!(!accepts(&relabeled));
        let mut foreign = source.clone();
        foreign.leaves[1].context.message[37] ^= 1;
        assert!(!accepts(&foreign));
        assert!(BlsBatchCircuit::new(source.plan, reversed.leaves).is_err());
        for endpoint in 0..6 {
            let mut public = source.instances().unwrap();
            public[0][endpoint] += Fp::ONE;
            assert!(
                !check_circuit(source, 16, &public, CheckMode::Strict)
                    .unwrap()
                    .is_satisfied()
            );
        }
    }
    let mut changed = paired[1].clone();
    changed.leaves[1].before = paired[2].leaves[1].before.clone();
    assert!(!accepts(&changed));
    let mut context = super::super::tests::context();
    context.message[37] ^= 1;
    let invalid =
        prepare_bls_batches(context.message, context.public_key, context.signature).unwrap();
    assert!(!accepts(invalid.last().unwrap()));
}

#[test]
fn known_unknown_and_imported_pair_layouts_match_across_phase_boundaries() {
    let paired = batches();
    let mut selected = vec![0, 107, paired.len() - 1];
    for (position, pair) in paired.iter().enumerate() {
        if pair.plan.needs_sha()
            || pair.plan.leaves[0].before_tag() != pair.plan.leaves[1].after_tag()
        {
            selected.push(position);
        }
    }
    selected.sort_unstable();
    selected.dedup();
    for position in selected {
        let source = &paired[position];
        assert!(accepts(source), "pair {position}");
        let known = synthesize(source, 16, Some(&source.instances().unwrap())).unwrap();
        for unknown in [
            source.without_witnesses(),
            BlsBatchCircuit::for_source(source.plan).unwrap(),
        ] {
            let actual = synthesize(&unknown, 16, None).unwrap();
            assert_eq!(known.cs, actual.cs);
            assert_eq!(known.tables.fixed(), actual.tables.fixed());
            assert_eq!(known.tables.selectors(), actual.tables.selectors());
            assert_eq!(known.tables.permutation(), actual.tables.permutation());
            assert_eq!(
                known.tables.advice_assigned(),
                actual.tables.advice_assigned()
            );
        }
    }
}

#[test]
#[ignore = "exact unknown-source capacity of all 542 fixed BLS pairs; no proofs"]
fn every_fixed_bls_pair_source_fits_k16() {
    let mut failures = 0;
    let mut maximum = (0, 0);
    for ordinal in 0..BlsBatchPlan::LENGTH {
        let source = BlsBatchCircuit::for_source(BlsBatchPlan::at(ordinal).unwrap()).unwrap();
        match synthesize(&source, 16, None) {
            Ok(layout) => {
                let rows = layout
                    .tables
                    .advice_assigned()
                    .iter()
                    .filter_map(|column| column.iter().rposition(|assigned| *assigned))
                    .map(|last| last + 1)
                    .max()
                    .unwrap_or(0);
                if rows > maximum.1 {
                    maximum = (ordinal, rows);
                }
            }
            Err(error) => {
                failures += 1;
                eprintln!("BLS_PAIR_CAPACITY_FAILURE ordinal={ordinal} error={error}");
            }
        }
    }
    eprintln!(
        "BLS_PAIR_CAPACITY classes={} failures={failures} largest_class={} maximum_advice_rows={} semantic_steps={} proof=false original_import=false",
        BlsBatchPlan::LENGTH,
        maximum.0,
        maximum.1,
        BlsLeafPlan::LENGTH
    );
    assert_eq!(failures, 0);
}

#[test]
#[ignore = "strict known-witness constraints for every one of the 542 original BLS pairs"]
fn strict_complete_native_bls_pairs_at_k16() {
    let paired = batches();
    for (ordinal, source) in paired.iter().enumerate() {
        assert!(accepts(source), "unsatisfied original BLS pair {ordinal}");
        if ordinal % 32 == 0 || ordinal + 1 == paired.len() {
            eprintln!(
                "BLS_PAIR_COMPLETE_PROGRESS checked={} total={} semantic_end={} actual_proof=false",
                ordinal + 1,
                paired.len(),
                2 * (ordinal + 1)
            );
        }
    }
}
