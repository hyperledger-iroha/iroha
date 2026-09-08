//! Final V1 raw-transcript fixture with six-lane digests and exact 32-byte Fp4 values.
use fastpq_prover::{
    Error, ExecutionMode, Proof, Prover, PublicInputs, VerifyLimits, verify_raw_statement,
    verify_raw_statement_with_limits,
};
use norito::core::to_bytes;
use std::{fs, path::Path};
const FIXTURE_NAME: &str = "v1_raw_transcript_64.bin";
// This mixed raw fixture opens all 136 queries. Keep its finite diagnostic
// budget explicit even when the default resource profile admits its size; this is
// neither a state-transition admission test nor an AXT payload-size exception.
const RAW_FIXTURE_MAX_PROOF_BYTES: usize = 2 * 1024 * 1024;
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
    let expected = fs::read(&path).expect("read canonical proof fixture");
    assert!(
        !expected.is_empty(),
        "fixture {FIXTURE_NAME} is empty; set FASTPQ_UPDATE_FIXTURES=1 and re-run tests"
    );
    let proof: Proof = norito::decode_from_bytes(&expected).expect("decode proof");
    assert!(matches!(
        verify_raw_statement_with_limits(
            &batch,
            &proof,
            VerifyLimits {
                max_proof_bytes: 512 * 1024,
                ..VerifyLimits::default()
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    verify_raw_statement(&batch, &proof).expect("raw fixture fits derived default resources");
    let limits = VerifyLimits {
        max_proof_bytes: RAW_FIXTURE_MAX_PROOF_BYTES,
        ..VerifyLimits::default()
    };
    verify_raw_statement_with_limits(&batch, &proof, limits).expect("raw fixture proof verifies");
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
        expected.as_slice(),
        "regenerated proof diverged from fixture"
    );
}
