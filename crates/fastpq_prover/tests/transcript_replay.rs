//! Canonical V1 raw-transcript regression fixture.
use fastpq_prover::{ExecutionMode, Proof, Prover, PublicInputs, verify_raw_statement};
use norito::core::to_bytes;
use std::{fs, path::Path};
const FIXTURE_NAME: &str = "v1_raw_transcript_64.bin";
mod common;
use common::{fixture_update_requested, v1_fixture_batch};
fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(FIXTURE_NAME)
}
#[test]
fn v1_raw_transcript_64_fixture_verifies() {
    let mut public_inputs = PublicInputs::default();
    public_inputs.dsid = [0x11; 16];
    public_inputs.slot = 42;
    public_inputs.perm_root = [0xCC; 32];
    public_inputs.tx_set_hash = [0xDD; 32];
    let batch = v1_fixture_batch(64, public_inputs);
    let path = fixture_path();
    if fixture_update_requested() {
        let prover = Prover::canonical_with_execution_mode(
            "fastpq-state-transition-stark-v1",
            ExecutionMode::Cpu,
        )
        .expect("prover");
        let proof = prover
            .prove_raw_statement(&batch)
            .expect("raw fixture proof");
        let encoded = to_bytes(&proof).expect("encode proof");
        fs::write(&path, &encoded).expect("write fixture");
        return;
    }
    let expected = include_bytes!("fixtures/v1_raw_transcript_64.bin");
    assert!(
        !expected.is_empty(),
        "fixture {FIXTURE_NAME} is empty; set FASTPQ_UPDATE_FIXTURES=1 and re-run tests"
    );
    let proof: Proof = norito::decode_from_bytes(expected).expect("decode proof");
    verify_raw_statement(&batch, &proof).expect("raw fixture proof verifies");
    let prover = Prover::canonical_with_execution_mode(
        "fastpq-state-transition-stark-v1",
        ExecutionMode::Cpu,
    )
    .expect("prover");
    let regenerated = prover
        .prove_raw_statement(&batch)
        .expect("regenerate raw fixture proof");
    let encoded = to_bytes(&regenerated).expect("encode regenerated proof");
    assert_eq!(
        encoded.as_slice(),
        expected,
        "regenerated proof diverged from fixture"
    );
}
