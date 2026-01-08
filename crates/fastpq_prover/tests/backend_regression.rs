//! Regression fixtures for the FASTPQ stage 2 backend artifacts.

use std::fs;

use fastpq_prover::{OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch};
use norito::core::to_bytes;

fn synthetic_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    for idx in 0..rows {
        let key = format!("asset/xor/account/{idx:08}").into_bytes();
        let pre = (idx as u64).to_le_bytes().to_vec();
        let post = (idx as u64 + 1).to_le_bytes().to_vec();
        let op = match idx % 3 {
            0 => OperationKind::Transfer,
            1 => OperationKind::Mint,
            _ => OperationKind::Burn,
        };
        batch.push(StateTransition::new(key, pre, post, op));
    }
    batch.sort();
    batch.public_inputs.dsid = [0u8; 16];
    batch.public_inputs.slot = 0;
    batch.public_inputs.old_root = [0u8; 32];
    batch.public_inputs.new_root = [0u8; 32];
    batch.public_inputs.perm_root = [0u8; 32];
    batch.public_inputs.tx_set_hash = [0u8; 32];
    batch
}

#[test]
fn stage2_artifact_balanced_1k_matches_fixture() {
    let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
    let batch = synthetic_batch(1_000);
    let proof = prover.prove(&batch).expect("proof");
    let expected = include_bytes!("fixtures/stage2_balanced_1k.bin");
    let encoded = to_bytes(&proof).expect("encode proof");
    if expected.is_empty() {
        println!(
            "generating stage2_balanced_1k.bin ({} bytes)",
            encoded.len()
        );
        fs::write("tests/fixtures/stage2_balanced_1k.bin", &encoded).expect("write fixture");
        panic!("wrote stage2_balanced_1k.bin fixture; re-run tests");
    }
    if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        fs::write("tests/fixtures/stage2_balanced_1k.bin", &encoded).expect("write fixture");
        return;
    }
    assert_eq!(encoded.as_slice(), expected);
}

#[test]
fn stage2_artifact_balanced_5k_matches_fixture() {
    let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
    let batch = synthetic_batch(5_000);
    let proof = prover.prove(&batch).expect("proof");
    let expected = include_bytes!("fixtures/stage2_balanced_5k.bin");
    let encoded = to_bytes(&proof).expect("encode proof");
    if expected.is_empty() {
        println!(
            "generating stage2_balanced_5k.bin ({} bytes)",
            encoded.len()
        );
        fs::write("tests/fixtures/stage2_balanced_5k.bin", &encoded).expect("write fixture");
        panic!("wrote stage2_balanced_5k.bin fixture; re-run tests");
    }
    if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        fs::write("tests/fixtures/stage2_balanced_5k.bin", &encoded).expect("write fixture");
        return;
    }
    assert_eq!(encoded.as_slice(), expected);
}
