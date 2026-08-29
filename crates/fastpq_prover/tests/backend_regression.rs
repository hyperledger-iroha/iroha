//! Optional CPU/GPU parity coverage for FASTPQ V1 raw statements.
#[cfg(feature = "fastpq-gpu")]
use crate::common::v1_fixture_batch;
#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::{ExecutionMode, Prover, PublicInputs};
#[cfg(feature = "fastpq-gpu")]
use norito::core::to_bytes;

#[cfg(feature = "fastpq-gpu")]
#[test]
fn v1_raw_statement_cpu_gpu_parity() {
    if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
        eprintln!("skipping cpu/gpu parity test; gpu backend unavailable");
        return;
    }
    let batch = v1_fixture_batch(1_000, PublicInputs::default());
    let cpu = Prover::canonical_with_execution_mode(
        "fastpq-state-transition-stark-v1",
        ExecutionMode::Cpu,
    )
    .expect("cpu prover");
    let gpu = Prover::canonical_with_execution_mode(
        "fastpq-state-transition-stark-v1",
        ExecutionMode::Gpu,
    )
    .expect("gpu prover");
    let cpu_proof = cpu
        .prove_raw_statement(&batch)
        .expect("raw cpu fixture proof");
    let gpu_proof = gpu
        .prove_raw_statement(&batch)
        .expect("raw gpu fixture proof");
    let cpu_encoded = to_bytes(&cpu_proof).expect("encode cpu proof");
    let gpu_encoded = to_bytes(&gpu_proof).expect("encode gpu proof");
    assert_eq!(cpu_encoded, gpu_encoded, "V1 cpu/gpu proofs must match");
}
