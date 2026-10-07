//! Native re-proving of every vendored golden case (oracle builds only).
//!
//! Each case is exported ([`crate::cases::setup`]) and proved by
//! `create_proof_oracle` with the vendored `transcript_repr` injected and the
//! vendored `ChaCha20` seed, inside Rayon pools of 1, 2, 4 and 7 threads. The
//! bytes must not depend on the pool, their SHA-256 must equal the vendored
//! constant, and both the native oracle verifier and the vendored verifier
//! must accept them. On a mismatch the vendored proof is recomputed and the
//! first differing 32-byte message is reported.
//!
//! The k13 cases additionally recompute the independent vendored proof to
//! exercise native parallel polynomial arithmetic above its size thresholds.

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

/// Check a full proof above all parallel polynomial thresholds, using
/// independently recomputed vendored bytes and both complete verifiers.
fn parallel_arithmetic_matches_vendored<B: CurveBridge>() {
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let parallel = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let setup = parallel.install(|| setup::<B>(Family::Wide, 13));
    let seed = [0xa7; 32];
    let vendored = serial.install(|| setup.prove_vendored(seed));
    for pool in [&serial, &parallel] {
        let native = pool.install(|| prove_native(&setup, seed));
        assert_eq!(
            native,
            vendored,
            "{} k13 polynomial arithmetic changed proof bytes with {} workers",
            B::NAME,
            pool.current_num_threads()
        );
        pool.install(|| {
            verify_full_oracle(
                &setup.params,
                setup.pk.binding(),
                setup.pk.vk(),
                &setup.native_instances(),
                &native,
                MemoryBudget::DEFAULT,
                setup.transcript_repr,
            )
            .expect("native complete verifier");
            setup
                .verify_vendored(&setup.vendored_instances(), &native)
                .expect("vendored complete verifier");
        });
    }
    println!(
        "PARALLEL_ARITHMETIC curve={} k=13 bytes={} sha256={} pools=1,4",
        B::NAME,
        vendored.len(),
        digest_hex(&vendored)
    );
}

#[test]
#[ignore = "k13 parallel arithmetic proof parity; run in release"]
fn wide_eq_k13_parallel_arithmetic_matches_vendored() {
    parallel_arithmetic_matches_vendored::<Vesta>();
}

#[test]
#[ignore = "k13 parallel arithmetic proof parity; run in release"]
fn wide_ep_k13_parallel_arithmetic_matches_vendored() {
    parallel_arithmetic_matches_vendored::<Pallas>();
}
