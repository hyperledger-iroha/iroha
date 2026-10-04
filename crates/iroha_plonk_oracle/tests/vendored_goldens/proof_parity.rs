//! Native re-proving of every vendored golden case (oracle builds only).
//!
//! Each case is exported ([`crate::cases::setup`]) and proved by
//! `create_proof_oracle` with the vendored `transcript_repr` injected and the
//! vendored `ChaCha20` seed, inside Rayon pools of 1, 2, 4 and 7 threads. The
//! bytes must not depend on the pool, their SHA-256 must equal the vendored
//! constant, and both the native oracle verifier and the vendored verifier
//! must accept them. On a mismatch the vendored proof is recomputed and the
//! first differing 32-byte message is reported.

use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    prover::{ProverConfig, ProverRandomness, create_proof_oracle},
    verifier::verify_full_oracle,
};
use iroha_plonk_oracle::{
    convert::{CurveBridge, Pallas, Vesta},
    pools::same_on_each_pool,
};

use crate::{
    cases::{CaseId, Family, Setup, cases_for, first_difference, setup},
    golden_proof_bytes::digest_hex,
};

/// One native oracle-mode proof of `setup` with the 32-byte `seed`.
pub fn prove_native<B: CurveBridge>(setup: &Setup<B>, seed: [u8; 32]) -> Vec<u8> {
    create_proof_oracle(
        &setup.params,
        &setup.pk,
        &setup.witness,
        ProverRandomness::fixed_seed_for_tests(seed),
        ProverConfig::default(),
        setup.transcript_repr,
    )
    .expect("native oracle-mode proof")
}

/// Proves `case` in every pool and checks it against the golden constant and
/// both verifiers; returns the proof.
fn check_case<B: CurveBridge>(setup: &Setup<B>, case: &CaseId) -> Vec<u8> {
    let proof = same_on_each_pool(case.name, |_| prove_native(setup, case.seed_bytes()));
    let digest = digest_hex(&proof);
    if digest != case.sha256 {
        let vendored = setup.prove_vendored(case.seed_bytes());
        panic!(
            "{}: native SHA-256 {digest}, golden {}; native {} bytes, vendored {} bytes, \
             first differing message {:?}",
            case.name,
            case.sha256,
            proof.len(),
            vendored.len(),
            first_difference(&proof, &vendored),
        );
    }
    verify_full_oracle(
        &setup.params,
        setup.pk.binding(),
        setup.pk.vk(),
        &setup.native_instances(),
        &proof,
        MemoryBudget::DEFAULT,
        setup.transcript_repr,
    )
    .unwrap_or_else(|error| panic!("{}: native verifier rejects: {error}", case.name));
    setup
        .verify_vendored(&setup.vendored_instances(), &proof)
        .unwrap_or_else(|error| panic!("{}: vendored verifier rejects: {error}", case.name));
    println!(
        "NATIVE_GOLDEN case={} bytes={} sha256={digest} pools=1,2,4,7",
        case.name,
        proof.len()
    );
    proof
}

/// Re-proves every golden case of `family` over curve `B` at every `k`.
fn native_goldens<B: CurveBridge>(family: Family, ks: &[u32]) {
    for &k in ks {
        let setup = setup::<B>(family, k);
        for case in cases_for(family, B::NAME, k) {
            check_case(&setup, &case);
        }
    }
}

#[test]
fn sigma_eq_native_proofs_are_golden() {
    native_goldens::<Vesta>(Family::Sigma, &[6, 9]);
}

#[test]
fn sigma_ep_native_proofs_are_golden() {
    native_goldens::<Pallas>(Family::Sigma, &[6, 9]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_eq_k11_native_proofs_are_golden() {
    native_goldens::<Vesta>(Family::Sigma, &[11]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_ep_k11_native_proofs_are_golden() {
    native_goldens::<Pallas>(Family::Sigma, &[11]);
}

#[test]
fn wide_eq_native_proofs_are_golden() {
    native_goldens::<Vesta>(Family::Wide, &[8, 10]);
}

#[test]
fn wide_ep_native_proofs_are_golden() {
    native_goldens::<Pallas>(Family::Wide, &[8, 10]);
}
