//! Performance regression test for the FASTPQ production prover.
//!
//! This is intentionally `#[ignore]` because the prover is CPU-heavy in debug
//! builds. Run the regression with
//! `cargo test -p fastpq_prover --test perf_production --release -- --ignored`
//! to obtain realistic timings.

use std::{env, time::Instant};

use fastpq_prover::{
    OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch, verify,
};
use norito::core::to_bytes;

const PARAMETER_SET: &str = "fastpq-lane-balanced";
const DEFAULT_ROW_COUNT: usize = 20_000;
const DEFAULT_EXPECTED_MS: f64 = 1_000.0;
const DEFAULT_MAX_KIB: f64 = 512.0;

fn synthetic_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new(PARAMETER_SET, PublicInputs::default());
    for i in 0..rows {
        let asset_id = format!("asset/{:08x}", i % 64);
        let key = format!("{asset_id}/balance/{i:08x}").into_bytes();

        let delta = ((i % 7) + 1) as u64;
        let (pre_value, post_value, operation) = match i % 16 {
            0 => {
                let base = 50_000u64 + i as u64;
                (
                    base.to_le_bytes().to_vec(),
                    (base + delta).to_le_bytes().to_vec(),
                    OperationKind::Mint,
                )
            }
            1 => {
                let base = 60_000u64 + i as u64 + delta;
                (
                    base.to_le_bytes().to_vec(),
                    (base - delta).to_le_bytes().to_vec(),
                    OperationKind::Burn,
                )
            }
            _ => {
                let base = 70_000u64 + i as u64;
                (
                    base.to_le_bytes().to_vec(),
                    (base + (delta % 3 + 1)).to_le_bytes().to_vec(),
                    OperationKind::Transfer,
                )
            }
        };

        batch.push(StateTransition::new(key, pre_value, post_value, operation));
    }

    batch.sort();
    let mut dsid = [0u8; 16];
    for (idx, byte) in dsid.iter_mut().enumerate() {
        *byte = (idx as u8).wrapping_mul(11);
    }
    batch.public_inputs.dsid = dsid;
    batch.public_inputs.slot = 1_726_128_000_000_000;
    batch.public_inputs.old_root = [0x11; 32];
    batch.public_inputs.new_root = [0x22; 32];
    batch.public_inputs.perm_root = [0x33; 32];
    batch.public_inputs.tx_set_hash = [0x44; 32];

    batch
}

fn configured_row_count() -> usize {
    env::var("FASTPQ_PROOF_ROWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&rows| rows > 0)
        .unwrap_or(DEFAULT_ROW_COUNT)
}

fn configured_expected_ms() -> f64 {
    env::var("FASTPQ_EXPECTED_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&ms| ms > 0.0)
        .unwrap_or(DEFAULT_EXPECTED_MS)
}

fn configured_max_kib() -> f64 {
    env::var("FASTPQ_EXPECTED_KIB")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&kib| kib > 0.0)
        .unwrap_or(DEFAULT_MAX_KIB)
}

#[test]
#[ignore = "expensive production perf regression"]
fn production_prover_handles_large_batch_quickly() {
    let row_count = configured_row_count();
    let batch = synthetic_batch(row_count);
    let prover = Prover::canonical(PARAMETER_SET).expect("prover");

    let started = Instant::now();
    let proof = prover.prove(&batch).expect("prove synthetic batch");
    let elapsed = started.elapsed();

    let proof_bytes = to_bytes(&proof).expect("serialize proof");
    verify(&batch, &proof).expect("proof verifies");

    #[allow(clippy::cast_precision_loss)]
    let proof_kib = proof_bytes.len() as f64 / 1024.0;
    let duration_ms = elapsed.as_secs_f64() * 1000.0;
    eprintln!(
        "fastpq_production_prover: rows={row_count} duration_ms={duration_ms:.2} proof_kib={proof_kib:.1}"
    );

    if !cfg!(debug_assertions) {
        let expected_ms = configured_expected_ms();
        let max_kib = configured_max_kib();

        let elapsed_ms = duration_ms;
        assert!(
            elapsed_ms <= expected_ms,
            "placeholder backend took {elapsed_ms:.2} ms for {row_count} rows (threshold {expected_ms:.2} ms)"
        );

        assert!(
            proof_kib <= max_kib,
            "placeholder proof length {proof_kib:.1} KiB exceeds budget {max_kib:.1} KiB"
        );
    }
}
