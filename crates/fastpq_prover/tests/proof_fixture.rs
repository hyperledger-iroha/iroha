//! Golden Stage 2 proof fixture parity; regeneration gated by `FASTPQ_UPDATE_FIXTURES`.

use std::{env, fs, path::PathBuf};

use fastpq_prover::{OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch};
use norito::core::to_bytes;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Construct the regression batch used for Stage 2 fixtures.
fn stage2_fixture_batch(rows: usize) -> TransitionBatch {
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
fn golden_stage2_proof_matches_fixture() {
    let path = fixture_path("stage2_balanced_1k.bin");
    if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = stage2_fixture_batch(1_000);
        let proof = prover.prove(&batch).expect("generate proof for fixture");
        let reencoded = to_bytes(&proof).expect("encode regenerated proof");
        fs::write(&path, &reencoded).expect("write regenerated fixture");
        return;
    }

    let bytes = fs::read(&path).expect("read proof fixture");
    let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
    let batch = stage2_fixture_batch(1_000);
    let proof = prover.prove(&batch).expect("produce proof for comparison");
    let reencoded = to_bytes(&proof).expect("encode proof for comparison");
    assert_eq!(
        reencoded, bytes,
        "golden proof fixture drifted; regenerate with FASTPQ_UPDATE_FIXTURES=1"
    );
}
