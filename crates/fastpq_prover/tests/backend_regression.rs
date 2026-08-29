//! Regression fixtures for FASTPQ V1 backend artifacts captured from transition batches.
use crate::common::{fixture_update_requested, v1_fixture_batch};
use fastpq_prover::{ExecutionMode, Prover, PublicInputs};
use iroha_crypto::Hash;
use norito::core::to_bytes;
use std::{fs, path::PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn v1_artifacts_match_fixtures() {
    let update = fixture_update_requested();
    let prover = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
        .expect("prover");
    let fixtures: [(&str, usize, &[u8]); 2] = [
        (
            "v1_balanced_1k.bin",
            1_000,
            include_bytes!("fixtures/v1_balanced_1k.bin"),
        ),
        (
            "v1_balanced_5k.bin",
            5_000,
            include_bytes!("fixtures/v1_balanced_5k.bin"),
        ),
    ];
    for (name, rows, expected) in fixtures {
        let batch = v1_fixture_batch(rows, PublicInputs::default());
        let proof = prover
            .prove_raw_statement(&batch)
            .expect("raw fixture proof");
        let encoded = to_bytes(&proof).expect("encode proof");
        assert!(!expected.is_empty(), "{name} must not be empty");
        if update {
            fs::write(fixture_path(name), &encoded).expect("write fixture");
        } else {
            assert_fixture_bytes(name, &encoded, expected);
        }
    }
}
#[cfg(feature = "fastpq-gpu")]
#[test]
fn v1_artifact_balanced_cpu_gpu_parity() {
    if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
        eprintln!("skipping cpu/gpu parity test; gpu backend unavailable");
        return;
    }
    let expected = include_bytes!("fixtures/v1_balanced_1k.bin");
    let batch = v1_fixture_batch(1_000, PublicInputs::default());
    let cpu = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
        .expect("cpu prover");
    let gpu = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Gpu)
        .expect("gpu prover");
    let cpu_proof = cpu
        .prove_raw_statement(&batch)
        .expect("raw cpu fixture proof");
    let gpu_proof = gpu
        .prove_raw_statement(&batch)
        .expect("raw gpu fixture proof");
    let cpu_encoded = to_bytes(&cpu_proof).expect("encode cpu proof");
    let gpu_encoded = to_bytes(&gpu_proof).expect("encode gpu proof");
    assert_eq!(
        cpu_encoded.as_slice(),
        expected,
        "cpu proof should match canonical V1 fixture"
    );
    assert_eq!(
        gpu_encoded.as_slice(),
        expected,
        "gpu proof should match canonical V1 fixture"
    );
    assert_eq!(cpu_encoded, gpu_encoded, "V1 cpu/gpu proofs must match");
}
fn assert_fixture_bytes(name: &str, actual: &[u8], expected: &[u8]) {
    assert!(
        actual == expected,
        "{name} fixture mismatch: actual_len={} expected_len={} actual_hash={} expected_hash={}",
        actual.len(),
        expected.len(),
        Hash::new(actual),
        Hash::new(expected)
    );
}
