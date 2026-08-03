//! Cross-conformance gates for the stand-alone Python reference implementation
//! under `reference/`.
//!
//! These `#[ignore]` tests black-box the crate through its public API: each reads
//! an artifact produced by the Python reference prover and asserts that the real
//! Rust verifier accepts it. The fixture generators live in the `vega_mc_zkp`
//! test module (they need crate internals); these gates need only the public
//! verifier, so they live here. Run explicitly with:
//!
//!   cargo test --test reference_conformance -- --ignored --nocapture
//!
//! The required `.bin` inputs are git-ignored; regenerate them with the exporters
//! and the Python tests under `reference/tests/` first.

use std::fs;
use std::path::Path;

use vega_prover::provider::T256HyraxEngine;
use vega_prover::vega_mc_zkp::{VegaMcVerifierKey, VegaMcZkSNARK};

type E = T256HyraxEngine;

// Cross-conformance gate: verify a proof produced by the stand-alone Python
// reference prover (reference/pyvega). Reads the Rust-exported vk.bin and the
// Python-serialized python_proof.bin, deserializes both, and asserts the Rust
// verifier accepts.
#[test]
#[ignore]
fn verify_python_proof() {
    let num_circuits = 2usize;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/fixtures/cubic");

    let vk_bytes = fs::read(dir.join("vk.bin")).unwrap();
    let vk: VegaMcVerifierKey<E> = bincode::deserialize(&vk_bytes).unwrap();

    let proof_bytes = fs::read(dir.join("python_proof.bin")).unwrap();
    let snark: VegaMcZkSNARK<E> = bincode::deserialize(&proof_bytes).unwrap();

    let (pv_step, pv_core) = snark
        .verify(&vk, num_circuits)
        .expect("Rust verifier rejected the Python-produced proof");

    eprintln!(
        "PASS: Rust verifier accepted the Python proof ({} B): public_values_step={:?}, public_values_core={:?}",
        proof_bytes.len(),
        pv_step,
        pv_core,
    );
}

// Fully stand-alone cross-conformance gate: verify a proof produced by the
// Python reference implementation against a verifier key ALSO produced by the
// Python setup (Python key-gen + shapes, zero Rust runtime). Regenerate the
// inputs first with:
//   sage -python reference/tests/test_standalone.py
#[test]
#[ignore]
fn verify_python_standalone() {
    let num_circuits = 2usize;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/fixtures/cubic");

    let vk_bytes = fs::read(dir.join("python_vk.bin")).unwrap();
    let vk: VegaMcVerifierKey<E> = bincode::deserialize(&vk_bytes).unwrap();

    let proof_bytes = fs::read(dir.join("python_standalone_proof.bin")).unwrap();
    let snark: VegaMcZkSNARK<E> = bincode::deserialize(&proof_bytes).unwrap();

    let (pv_step, pv_core) = snark
        .verify(&vk, num_circuits)
        .expect("Rust verifier rejected the stand-alone Python proof");

    eprintln!(
        "PASS: Rust verifier accepted the stand-alone Python vk ({} B) + proof ({} B): public_values_step={:?}, public_values_core={:?}",
        vk_bytes.len(),
        proof_bytes.len(),
        pv_step,
        pv_core,
    );
}
