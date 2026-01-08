//! Stage 4 transcript replay regression fixtures.

#[cfg(feature = "fastpq-gpu")]
use std::{convert::TryFrom, env, fs, path::Path};

#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::{
    OperationKind, Proof, Prover, PublicInputs, StateTransition, TransitionBatch, verify,
};
#[cfg(feature = "fastpq-gpu")]
use norito::core::to_bytes;

#[cfg(feature = "fastpq-gpu")]
const FIXTURE_NAME: &str = "stage4_balanced_preview.bin";

#[cfg(feature = "fastpq-gpu")]
fn annotate_batch(batch: &mut TransitionBatch) {
    batch.public_inputs.dsid = [0x11; 16];
    batch.public_inputs.slot = 42;
    batch.public_inputs.old_root = [0xAA; 32];
    batch.public_inputs.new_root = [0xBB; 32];
    batch.public_inputs.perm_root = [0xCC; 32];
    batch.public_inputs.tx_set_hash = [0xDD; 32];
}

#[cfg(feature = "fastpq-gpu")]
fn preview_fixture_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    for idx in 0..rows {
        let key = format!("asset/xor/account/{idx:04}").into_bytes();
        let idx_u64 = u64::try_from(idx).expect("sample row index fits u64");
        let pre = idx_u64.to_le_bytes().to_vec();
        let post = idx_u64.wrapping_add(1).to_le_bytes().to_vec();
        let op = match idx % 3 {
            0 => OperationKind::Transfer,
            1 => OperationKind::Mint,
            _ => OperationKind::Burn,
        };
        batch.push(StateTransition::new(key, pre, post, op));
    }
    batch.sort();
    annotate_batch(&mut batch);
    batch
}

#[cfg(feature = "fastpq-gpu")]
fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(FIXTURE_NAME)
}

#[cfg(feature = "fastpq-gpu")]
#[test]
fn stage4_preview_fixture_replays_transcript() {
    let batch = preview_fixture_batch(64);
    let path = fixture_path();

    if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
        let proof = prover.prove(&batch).expect("proof");
        let encoded = to_bytes(&proof).expect("encode proof");
        fs::write(&path, &encoded).expect("write fixture");
        panic!("updated {FIXTURE_NAME}; re-run tests without FASTPQ_UPDATE_FIXTURES to validate");
    }

    let expected = include_bytes!("fixtures/stage4_balanced_preview.bin");
    assert!(
        !expected.is_empty(),
        "fixture {FIXTURE_NAME} is empty; set FASTPQ_UPDATE_FIXTURES=1 and re-run tests"
    );

    let proof: Proof = norito::decode_from_bytes(expected).expect("decode proof");
    verify(&batch, &proof).expect("fixture proof verifies");

    let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
    let regenerated = prover.prove(&batch).expect("regenerate proof");
    let encoded = to_bytes(&regenerated).expect("encode regenerated proof");
    assert_eq!(
        encoded.as_slice(),
        expected,
        "regenerated proof diverged from fixture"
    );
}
