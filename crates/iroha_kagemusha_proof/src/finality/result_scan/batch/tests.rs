//! Exact complete-stream grouping, full register binding and source layout checks.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn oracle(frame: &[u8]) -> [u8; 32] {
    let mut input = RESULT_TAG.to_vec();
    input.extend_from_slice(frame);
    *iroha_crypto::Hash::new(input).as_ref()
}
fn frame(length: usize) -> Vec<u8> {
    (0..length)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect()
}
fn batches(length: usize) -> Vec<ResultScanBatchCircuit> {
    let frame = frame(length);
    prepare_result_batches(&frame, oracle(&frame)).unwrap()
}
fn accepts(source: &ResultScanBatchCircuit) -> bool {
    source.instances().is_ok_and(|public| {
        check_circuit(source, 16, &public, CheckMode::Strict)
            .is_ok_and(|report| report.is_satisfied())
    })
}

#[test]
fn paired_scan_preserves_start_finish_every_register_and_all_padding() {
    for length in [
        0,
        1,
        127 - RESULT_TAG.len(),
        128 - RESULT_TAG.len(),
        129 - RESULT_TAG.len(),
        127,
        128,
        129,
        300,
        65_535,
        65_536,
    ] {
        let frame = frame(length);
        let expected = oracle(&frame);
        let original = prepare_result_scan(&frame, expected).unwrap();
        let paired = prepare_result_batches(&frame, expected).unwrap();
        assert_eq!(paired.len(), 258);
        let mut cursor = 0;
        for (ordinal, pair) in paired.iter().enumerate() {
            assert_eq!(
                pair.plan,
                ResultScanBatchPlan::at(u32::try_from(ordinal).unwrap()).unwrap()
            );
            let first = original[cursor].endpoints();
            for leaf in &pair.leaves {
                let actual = &original[cursor];
                assert_eq!(leaf.plan, actual.plan);
                assert_eq!(leaf.cursor, actual.cursor);
                assert_eq!(leaf.before.words, actual.before.words);
                assert_eq!(leaf.before.processed, actual.before.processed);
                assert_eq!(leaf.after.words, actual.after.words);
                assert_eq!(leaf.after.processed, actual.after.processed);
                assert_eq!(leaf.context.digest(), actual.context.digest());
                cursor += 1;
            }
            let last = original[cursor - 1].endpoints();
            assert_eq!(
                pair.endpoints(),
                [first[0], first[1], first[2], last[3], first[4], last[5]]
            );
        }
        assert_eq!(cursor, 515);
        assert_eq!(
            paired[0].endpoints()[4],
            paired[0].context().boundary_digest(false)
        );
        assert_eq!(
            paired.last().unwrap().endpoints()[5],
            paired[0].context().boundary_digest(true)
        );
        assert_eq!(paired.last().unwrap().proposed_digest().unwrap(), expected);
        assert!(paired[0].proposed_digest().is_err());
        for pair in paired.windows(2) {
            let a = pair[0].endpoints();
            let b = pair[1].endpoints();
            assert_eq!(a[..2], b[..2]);
            assert_eq!(a[3], b[2]);
            assert_eq!(a[5], b[4]);
        }
        let active_blocks = (length + RESULT_TAG.len()).div_ceil(128);
        let mut selected = vec![
            0,
            1,
            active_blocks / 2,
            (active_blocks / 2 + 1).min(256),
            257,
        ];
        selected.sort_unstable();
        selected.dedup();
        for index in selected {
            assert!(accepts(&paired[index]), "length{length} pair{index}");
        }
    }
    assert!(ResultScanBatchPlan::at(RESULT_BATCH_LENGTH).is_none());
    assert!(ResultScanBatchPlan::at(u32::MAX).is_none());
    assert!(
        ResultScanBatchCircuit::for_source(ResultScanBatchPlan::Finish)
            .unwrap()
            .proposed_digest()
            .is_err()
    );
}

#[test]
fn all_three_pair_classes_fit_and_preserve_original_unknown_layouts() {
    let paired = batches(300);
    for indices in [vec![0], vec![1, 2, 256], vec![257]] {
        let source = &paired[indices[0]];
        assert!(accepts(source));
        let known = synthesize(source, 16, Some(&source.instances().unwrap())).unwrap();
        let rows = known
            .tables
            .advice_assigned()
            .iter()
            .filter_map(|column| column.iter().rposition(|v| *v))
            .map(|last| last + 1)
            .max()
            .unwrap();
        for unknown in [
            source.without_witnesses(),
            ResultScanBatchCircuit::for_source(source.plan).unwrap(),
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
        for index in indices.into_iter().skip(1) {
            let actual = synthesize(
                &paired[index],
                16,
                Some(&paired[index].instances().unwrap()),
            )
            .unwrap();
            assert_eq!(known.cs, actual.cs);
            assert_eq!(known.tables.fixed(), actual.tables.fixed());
            assert_eq!(known.tables.selectors(), actual.tables.selectors());
            assert_eq!(known.tables.permutation(), actual.tables.permutation());
            assert_eq!(
                known.tables.advice_assigned(),
                actual.tables.advice_assigned()
            );
        }
        eprintln!(
            "RESULT_PAIR_SOURCE plan={:?} max_rows={rows} hard_k16=true source_proof=false",
            source.plan
        );
    }
}

#[test]
fn pair_binding_rejects_all_intermediate_words_progress_tape_and_operation_changes() {
    let paired = batches(300);
    for index in [0, 1] {
        let source = &paired[index];
        assert!(accepts(source));
        for register in 0..8 {
            let mut changed = source.clone();
            changed.leaves[1].before.words[register] ^= 1;
            assert!(
                !accepts(&changed),
                "unbound intermediate register{register}"
            );
        }
        let mut progress = source.clone();
        progress.leaves[1].before.processed += 1;
        assert!(!accepts(&progress));
        let mut reordered = source.clone();
        reordered.leaves.reverse();
        assert!(!accepts(&reordered));
        let mut dropped = source.clone();
        dropped.leaves.pop();
        assert!(!accepts(&dropped));
        let mut skipped = source.clone();
        skipped.leaves[1].cursor += 1;
        assert!(!accepts(&skipped));
        let mut relabelled = source.clone();
        relabelled.plan = ResultScanBatchPlan::Finish;
        assert!(!accepts(&relabelled));
        let mut foreign = source.clone();
        foreign.leaves[1].context.expected[0] ^= 1;
        assert!(!accepts(&foreign));
        let mut changed_frame = frame(300);
        changed_frame[0] ^= 1;
        let mut tape = source.clone();
        tape.leaves[1].tape =
            Arc::new(ResultTapeWitness::from_frame(&Value::known(changed_frame)).unwrap());
        assert!(!accepts(&tape));
        let mut output = source.clone();
        output.leaves[1].after.words[7] ^= 1;
        assert!(!accepts(&output));
        assert!(ResultScanBatchCircuit::new(source.plan, reordered.leaves).is_err());
    }
    let mut padding = paired[256].clone();
    assert!(accepts(&padding));
    padding.leaves[1].after.words[7] ^= 1;
    assert!(!accepts(&padding));
    let mut finish = paired[257].clone();
    finish.leaves[0].before.processed -= 1;
    assert!(!accepts(&finish));
    let frame = frame(300);
    let mut wrong = oracle(&frame);
    wrong[0] ^= 1;
    let invalid = prepare_result_batches(&frame, wrong).unwrap();
    assert_eq!(
        invalid.last().unwrap().proposed_digest().unwrap(),
        oracle(&frame)
    );
    assert!(!accepts(invalid.last().unwrap()));
}

#[test]
#[ignore = "strict constraints for every paired step of the native fixture and maximum result"]
fn strict_complete_result_pairs_at_k16() {
    let original = super::super::tests::fixture_bytes("result_preimage_hex");
    for frame in [original, frame(65_536)] {
        let paired = prepare_result_batches(&frame, oracle(&frame)).unwrap();
        for (ordinal, source) in paired.iter().enumerate() {
            assert!(
                accepts(source),
                "length{} result pair{ordinal}",
                frame.len()
            );
            if ordinal % 32 == 0 || ordinal + 1 == paired.len() {
                eprintln!(
                    "RESULT_PAIR_COMPLETE_PROGRESS bytes={} checked={} total={} actual_proof=false",
                    frame.len(),
                    ordinal + 1,
                    paired.len()
                );
            }
        }
    }
}
