//! Regression fixtures for the FASTPQ stage 2 backend artifacts.

use fastpq_prover::{OperationKind, Prover, StateTransition, TransitionBatch};
use norito::core::to_bytes;
use std::fs;

fn synthetic_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced");
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
    batch.metadata.insert("dsid".into(), vec![0u8; 16]);
    batch
        .metadata
        .insert("slot".into(), 0u64.to_le_bytes().to_vec());
    batch.metadata.insert("old_root".into(), vec![0u8; 32]);
    batch.metadata.insert("new_root".into(), vec![0u8; 32]);
    batch.metadata.insert("perm_root".into(), vec![0u8; 32]);
    batch.metadata.insert("tx_set_hash".into(), vec![0u8; 32]);
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
