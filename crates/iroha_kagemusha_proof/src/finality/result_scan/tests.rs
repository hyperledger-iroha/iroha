//! Full native result fixture, fixed-layout checks, and source substitution tests.
use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

pub(super) fn fixture_bytes(name: &str) -> Vec<u8> {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let encoded = fixture
        .get(name)
        .and_then(norito::json::Value::as_str)
        .unwrap();
    (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
        .collect()
}
fn oracle(frame: &[u8]) -> [u8; 32] {
    let mut input = RESULT_TAG.to_vec();
    input.extend_from_slice(frame);
    *iroha_crypto::Hash::new(input).as_ref()
}
fn valid(leaf: &ResultScanCircuit) -> bool {
    let instances = leaf.instances().unwrap();
    let compiled = synthesize(leaf, 16, Some(&instances)).expect("fixed k16 scan layout");
    let rows = compiled
        .tables
        .advice_assigned()
        .iter()
        .filter_map(|column| column.iter().rposition(|assigned| *assigned))
        .max()
        .map_or(0, |last| last + 1);
    let report =
        iroha_plonk::check::check(&compiled.cs, &compiled.tables, CheckMode::Strict).unwrap();
    eprintln!(
        "result scan {:?} {:?}: {rows} rows, valid={}",
        leaf.plan,
        &leaf.endpoints()[2..4],
        report.is_satisfied()
    );
    if !report.is_satisfied() {
        for failure in report.failures().iter().take(3) {
            eprintln!("{failure}");
        }
    }
    report.is_satisfied()
}
fn linked(leaves: &[ResultScanCircuit]) {
    assert_eq!(
        leaves.first().unwrap().endpoints()[4],
        leaves[0].context.boundary_digest(false)
    );
    let last = leaves.last().unwrap();
    assert_eq!(last.endpoints()[5], last.context.boundary_digest(true));
    assert_eq!(last.endpoints()[3], Fp::from(u64::from(RESULT_SCAN_LEAVES)));
    for pair in leaves.windows(2) {
        let left = pair[0].endpoints();
        let right = pair[1].endpoints();
        assert_eq!(left[..2], right[..2]);
        assert_eq!(left[3], right[2]);
        assert_eq!(left[5], right[4]);
    }
}

#[test]
fn completed_proposal_matches_native_hash_and_does_not_accept_an_expected_digest() {
    for length in [
        0,
        128 - RESULT_TAG.len(),
        129 - RESULT_TAG.len(),
        MAX_RESULT_BYTES as usize,
    ] {
        let frame = vec![0x5a; length];
        let actual = oracle(&frame);
        let mut wrong = actual;
        wrong[0] ^= 1;
        let leaves = prepare_result_scan(&frame, wrong).unwrap();
        assert!(leaves[0].proposed_digest().is_err());
        let finish = leaves.last().unwrap();
        assert_eq!(finish.proposed_digest().unwrap(), actual);
        assert_ne!(finish.proposed_digest().unwrap(), finish.context.expected);
    }
    assert!(
        ResultScanCircuit::for_source(ResultScanPlan::Finish)
            .unwrap()
            .proposed_digest()
            .is_err()
    );
}

#[test]
fn complete_native_result_scan_is_contiguous_and_strict_at_k16() {
    let frame = fixture_bytes("result_preimage_hex");
    let expected: [u8; 32] = fixture_bytes("result_hash_hex").try_into().unwrap();
    assert_eq!(oracle(&frame), expected);
    let leaves = prepare_result_scan(&frame, expected).unwrap();
    assert_eq!(leaves.len(), RESULT_SCAN_LEAVES as usize);
    linked(&leaves);
    for leaf in &leaves {
        assert!(valid(leaf));
    }
}

#[test]
fn three_scan_sources_have_witness_independent_original_layouts() {
    let frame: Vec<_> = (0..300).map(|i| u8::try_from(i % 256).unwrap()).collect();
    let leaves = prepare_result_scan(&frame, oracle(&frame)).unwrap();
    for index in [0, 1, 3, 4, leaves.len() - 2, leaves.len() - 1] {
        let leaf = &leaves[index];
        let public = leaf.instances().unwrap();
        let known = synthesize(leaf, 16, Some(&public)).unwrap();
        let unknown =
            synthesize(&ResultScanCircuit::for_source(leaf.plan).unwrap(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.selectors(), unknown.tables.selectors());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
    }
    let first = synthesize(&leaves[1], 16, None).unwrap();
    let final_block = synthesize(&leaves[leaves.len() - 2], 16, None).unwrap();
    assert_eq!(first.tables.fixed(), final_block.tables.fixed());
    assert_eq!(first.tables.selectors(), final_block.tables.selectors());
    assert_eq!(first.tables.permutation(), final_block.tables.permutation());
}

#[test]
fn source_states_tape_expected_digest_and_completion_cannot_be_substituted() {
    let frame: Vec<_> = (0..200)
        .map(|i| u8::try_from((i * 7) % 256).unwrap())
        .collect();
    let leaves = prepare_result_scan(&frame, oracle(&frame)).unwrap();
    let mut iv = leaves[0].clone();
    iv.after.words[7] ^= 1;
    assert!(
        !valid(&iv),
        "the full IV, including non-digest words, is fixed"
    );
    let mut output = leaves[1].clone();
    output.after.words[7] ^= 1;
    assert!(!valid(&output), "the full output state is constrained");
    let mut input = leaves[2].clone();
    input.before.words[6] ^= 1;
    assert!(
        !valid(&input),
        "a substituted continuation must not retain the output"
    );
    let mut cursor = leaves[1].clone();
    cursor.before.processed = 1;
    assert!(
        !valid(&cursor),
        "an unaligned or skipped byte cannot start a block"
    );
    let mut other_frame = frame.clone();
    other_frame[0] ^= 1;
    let mut tape = leaves[1].clone();
    tape.tape = Arc::new(ResultTapeWitness::from_frame(&Value::known(other_frame)).unwrap());
    assert!(
        !valid(&tape),
        "original tape membership binds every absorbed byte"
    );
    let mut incomplete = leaves.last().unwrap().clone();
    incomplete.before.processed -= 1;
    assert!(
        !valid(&incomplete),
        "Finish cannot release an incomplete hash"
    );
    let mut expected = leaves.last().unwrap().clone();
    expected.context.expected[0] ^= 1;
    assert!(
        !valid(&expected),
        "expected R is checked in circuit, not by the builder"
    );
    let mut overrun = leaves[1].clone();
    overrun.before = leaves.last().unwrap().before;
    assert!(
        !valid(&overrun),
        "a completed stream cannot replace an active compression slot"
    );
    let mut skipped = leaves[1].clone();
    skipped.cursor = 2;
    assert!(!valid(&skipped), "the slot determines exact byte progress");
    let mut padding = leaves[3].clone();
    assert!(valid(&padding), "padding preserves the completed stream");
    padding.after.words[7] ^= 1;
    assert!(!valid(&padding), "padding cannot alter any chaining word");
    let mut padding = leaves[3].clone();
    padding.before.processed -= 1;
    assert!(!valid(&padding), "padding cannot hide unfinished bytes");
    let mut cursor = leaves[3].clone();
    cursor.cursor = RESULT_SCAN_BLOCKS + 1;
    assert!(!valid(&cursor), "padding cannot occupy the Finish slot");
    let mut oversized = leaves[0].clone();
    oversized.context.frame_len = MAX_RESULT_BYTES + 1;
    assert!(!valid(&oversized), "native and circuit bounds agree");
    let mut public = leaves[0].instances().unwrap();
    public[0][0] += Fp::ONE;
    assert!(
        !check_circuit(&leaves[0], 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn exact_full_blocks_and_maximum_frame_have_bounded_native_witnesses() {
    for length in [
        0,
        128 - RESULT_TAG.len(),
        129 - RESULT_TAG.len(),
        MAX_RESULT_BYTES as usize,
    ] {
        let frame = vec![0x5a; length];
        let leaves = prepare_result_scan(&frame, oracle(&frame)).unwrap();
        linked(&leaves);
        assert_eq!(leaves.len(), RESULT_SCAN_LEAVES as usize);
        let real_blocks = (length + RESULT_TAG.len()).div_ceil(128);
        for leaf in &leaves[real_blocks + 1..leaves.len() - 1] {
            assert_eq!(leaf.before.words, leaf.after.words);
            assert_eq!(leaf.before.processed, leaf.after.processed);
        }
        let last = leaves.last().unwrap();
        let mut actual: Vec<_> = last.before.words[..4]
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        actual[31] |= 1;
        assert_eq!(actual, oracle(&frame));
        assert!(
            Arc::ptr_eq(&leaves[0].tape, &last.tape),
            "all leaves share one bounded tape"
        );
        assert!(valid(&leaves[0]));
        assert!(valid(&leaves[real_blocks]));
        if real_blocks < RESULT_SCAN_BLOCKS as usize {
            assert!(valid(&leaves[real_blocks + 1]));
        }
        assert!(valid(&leaves[leaves.len() - 2]));
        assert!(valid(last));
    }
    assert!(prepare_result_scan(&vec![0; MAX_RESULT_BYTES as usize + 1], [0; 32]).is_err());
}
