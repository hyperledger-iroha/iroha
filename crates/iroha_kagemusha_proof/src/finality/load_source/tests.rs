//! Native receipt/event/path parity, strict source boundaries and fixed layouts.

use super::*;
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn fixture(name: &str) -> Vec<u8> {
    let json: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/ordinary_load_receipt_v1.json"
    ))
    .unwrap();
    let hex = json
        .get(name)
        .and_then(norito::json::Value::as_str)
        .unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn sample() -> Vec<LoadSourceCircuit> {
    let frame = fixture("result_preimage_hex");
    let receipt = fixture("receipt_transcript_hex").try_into().unwrap();
    let event: [u8; 32] = fixture("event_box_hash_hex").try_into().unwrap();
    let tree = [HashOf::<u8>::from_untyped_unchecked(Hash::prehashed(event))]
        .into_iter()
        .collect::<MerkleTree<u8>>();
    let root = *tree.root().unwrap().as_ref();
    assert_eq!(root, frame[220..252]);
    let leaves = prepare_load_source(&frame, &receipt, root, 1, 0, &[[0; 32]; 32]).unwrap();
    assert_eq!(
        leaves[0].context.receipt.digest.to_repr().as_slice(),
        fixture("receipt_digest_hex")
    );
    leaves
}
fn valid(leaf: &LoadSourceCircuit) -> bool {
    let instances = leaf.instances().unwrap();
    let compiled = synthesize(leaf, 16, Some(&instances)).expect("fixed k16 Load source layout");
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
        "Load source {:?}/{}: {rows} rows, valid={}",
        leaf.plan,
        leaf.cursor,
        report.is_satisfied()
    );
    if !report.is_satisfied() {
        for failure in report.failures().iter().take(3) {
            eprintln!("{failure}");
        }
    }
    report.is_satisfied()
}
fn linked(leaves: &[LoadSourceCircuit]) {
    assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
    assert_eq!(
        leaves[0].endpoints()[4],
        leaves[0].context.boundary_digest(false)
    );
    let last = leaves.last().unwrap();
    assert_eq!(last.endpoints()[5], last.context.boundary_digest(true));
    assert_eq!(last.endpoints()[3], Fp::from(u64::from(PROGRAM_LENGTH)));
    for pair in leaves.windows(2) {
        let (a, b) = (pair[0].endpoints(), pair[1].endpoints());
        assert_eq!(a[..2], b[..2]);
        assert_eq!(a[3], b[2]);
        assert_eq!(a[5], b[4]);
    }
}

#[test]
fn genuine_native_receipt_event_and_result_close_all_35_stages_at_k16() {
    let leaves = sample();
    linked(&leaves);
    for leaf in &leaves {
        assert!(valid(leaf));
    }
}

#[test]
fn every_source_class_and_path_cursor_has_one_original_layout() {
    let leaves = sample();
    for index in [0, 1, 33, 34] {
        let leaf = &leaves[index];
        let instances = leaf.instances().unwrap();
        let known = synthesize(leaf, 16, Some(&instances)).unwrap();
        let unknown =
            synthesize(&LoadSourceCircuit::for_source(leaf.plan).unwrap(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.selectors(), unknown.tables.selectors());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
    }
    let first = synthesize(&leaves[1], 16, None).unwrap();
    let last = synthesize(&leaves[32], 16, None).unwrap();
    assert_eq!(first.tables.fixed(), last.tables.fixed());
    assert_eq!(first.tables.selectors(), last.tables.selectors());
    assert_eq!(first.tables.permutation(), last.tables.permutation());
}

#[test]
fn live_counted_paths_match_native_ragged_tree_and_reject_padding() {
    let receipt: [u8; LoadReceiptCells::BYTES] =
        fixture("receipt_transcript_hex").try_into().unwrap();
    let event: [u8; 32] = fixture("event_box_hash_hex").try_into().unwrap();
    for (count, index) in [(2, 1), (3, 2), (5, 4)] {
        let leaves = (0..count)
            .map(|i| {
                HashOf::<u8>::from_untyped_unchecked(if i == index {
                    Hash::prehashed(event)
                } else {
                    Hash::new([u8::try_from(i).unwrap()])
                })
            })
            .collect::<Vec<_>>();
        let tree = leaves.iter().copied().collect::<MerkleTree<u8>>();
        let proof = tree.get_proof(index).unwrap();
        let mut siblings = [[0; 32]; 32];
        for (dst, src) in siblings.iter_mut().zip(proof.audit_path()) {
            *dst = src.map_or([0; 32], |hash| *hash.as_ref());
        }
        // These synthetic prefix bytes test extraction/inclusion only. They are
        // deliberately not presented as a genuinely certified native result.
        let mut frame = fixture("result_preimage_hex");
        frame[220..252].copy_from_slice(tree.root().unwrap().as_ref());
        frame[253..261].copy_from_slice(&u64::from(count).to_le_bytes());
        let path = prepare_load_source(
            &frame,
            &receipt,
            *tree.root().unwrap().as_ref(),
            u64::from(count),
            index,
            &siblings,
        )
        .unwrap();
        linked(&path);
        assert!(valid(&path[0]));
        for stage in &path[1..=proof.audit_path().len()] {
            assert!(valid(stage));
        }
        assert!(valid(&path[33]));
        assert!(valid(&path[34]));
        let mut wrong = path[1].clone();
        wrong.sibling[0] ^= 1;
        assert!(!valid(&wrong));
        let mut wrong = path[proof.audit_path().len() + 1].clone();
        wrong.sibling[31] = 1;
        assert!(!valid(&wrong));
    }
}

#[test]
fn receipt_terms_path_states_prefix_and_boundaries_cannot_be_substituted() {
    let leaves = sample();
    for count in [0, (1_u64 << 32) + 1] {
        let mut wrong = leaves[0].clone();
        wrong.context.event_count = count;
        assert!(!valid(&wrong), "count must fit the native bounded path");
    }
    let mut wrong = leaves[0].clone();
    wrong.context.event_index = 1;
    assert!(
        !valid(&wrong),
        "index must be strictly below the exact count"
    );
    let mut wrong = leaves[0].clone();
    wrong.context.receipt.amount += 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[0].clone();
    wrong.context.receipt.digest += Fp::ONE;
    assert!(!valid(&wrong));
    let mut wrong = leaves[0].clone();
    wrong.receipt[250] ^= 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[0].clone();
    wrong.after.digest[0] ^= 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[1].clone();
    wrong.before.digest[0] ^= 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[1].clone();
    wrong.after.width = 2;
    assert!(!valid(&wrong));
    let mut wrong = leaves[1].clone();
    wrong.cursor = 33;
    assert!(!valid(&wrong));
    let mut wrong = leaves[1].clone();
    wrong.sibling[0] = 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[33].clone();
    wrong.context.receipt.height += 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[33].clone();
    wrong.context.event_count = 2;
    assert!(!valid(&wrong));
    let mut wrong = leaves[33].clone();
    wrong.context.event_root[0] ^= 1;
    assert!(!valid(&wrong));
    let mut wrong = leaves[33].clone();
    wrong.context.result_frame_len += 1;
    assert!(!valid(&wrong));
    let mut frame = fixture("result_preimage_hex");
    frame[41] ^= 1;
    let mut wrong = leaves[33].clone();
    wrong.tape = Arc::new(ResultTapeWitness::from_frame(&Value::known(frame)).unwrap());
    assert!(!valid(&wrong));
    let mut wrong = leaves[34].clone();
    wrong.before.width = 2;
    assert!(!valid(&wrong));
    let mut wrong = leaves[34].clone();
    wrong.cursor = 33;
    assert!(!valid(&wrong));
    let mut instances = leaves[1].instances().unwrap();
    instances[0][4] += Fp::ONE;
    assert!(
        !check_circuit(&leaves[1], 16, &instances, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn witness_only_single_block_hash_matches_independent_native_hash() {
    use crate::finality::result_scan::marked_hash_one_block;
    for len in [0, 45, 51, 87, 128] {
        let bytes = (0..len)
            .map(|i| u8::try_from((i * 17) % 256).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            marked_hash_one_block(&bytes).unwrap(),
            *Hash::new(&bytes).as_ref()
        );
    }
    assert!(marked_hash_one_block(&[0; 129]).is_err());
}
