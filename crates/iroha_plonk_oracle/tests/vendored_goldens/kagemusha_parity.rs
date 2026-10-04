//! Proof-byte parity of the KAGEMUSHA path on the vendored golden circuits
//! (oracle builds only; first part of task T16).
//!
//! Each golden is keyed natively with `KagemushaPoseidonRp57` and the
//! `FoldedGenerator` suffix (the vendored keys are unchanged: the
//! verifying-key bytes do not depend on the transcript) and proved by
//! `create_proof_oracle` with the injected vendored `transcript_repr`
//! (`fe_to_fe` Poseidon absorption, no instance frame) and the `ChaCha20`
//! stream of the golden seed, in Rayon pools of 1, 2, 4 and 7 threads. The
//! bytes must equal the vendored KAGEMUSHA proof
//! ([`crate::kagemusha_vendored::prove_augmented`]: the snark-verifier
//! Poseidon transcript, then the vendored `G'_0`), so the Poseidon
//! transcript, its point absorption and the suffix layout are pinned at
//! proof level, not only by transcript KATs. Both the native oracle
//! verifier and the vendored augmented verifier accept both proofs.
//!
//! TODO(T16, `iroha_core_zk` owner): the `sigma_native_k11` KAGEMUSHA golden
//! itself (`GOLDEN_SHA256 sigma_native_k11/{eq,ep}`, recovery seed `[7; 32]`)
//! lives in `iroha_core_zk`'s private test module; re-proving it natively
//! needs its circuit exported there (`prover_golden_parity_tests.rs` with a
//! dev-dependency on this crate).

use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    cs::{ProofSuffixV1, TranscriptV1},
    keys::ProvingKey,
    prover::{ProverConfig, ProverRandomness, Witness, create_proof_oracle},
    verifier::verify_full_oracle,
};
use iroha_plonk_oracle::{
    convert::{CurveBridge, NativeScalar, Pallas, Vesta},
    export::vendored_keygen_config,
    pools::same_on_each_pool,
};

use crate::{
    cases::{Family, Setup, cases_for, first_difference, setup},
    golden_proof_bytes::digest_hex,
};

/// The native key and witness of a golden's KAGEMUSHA configuration.
pub struct KagemushaKeys<B: CurveBridge> {
    /// The proving key (Poseidon transcript, `FoldedGenerator` suffix).
    pub pk: ProvingKey<B::Native>,
    /// The witness.
    pub witness: Witness<NativeScalar<B>>,
}

/// Keys `setup`'s export for the KAGEMUSHA path.
pub fn kagemusha_keys<B: CurveBridge>(setup: &Setup<B>) -> KagemushaKeys<B> {
    let config = vendored_keygen_config::<B>(
        setup.vendored_pk.get_vk(),
        TranscriptV1::KagemushaPoseidonRp57,
        ProofSuffixV1::FoldedGenerator,
    );
    let pk = setup
        .exported
        .keygen(&setup.params, &config)
        .expect("KAGEMUSHA native key");
    assert_eq!(
        pk.vk().to_bytes(),
        setup.pk.vk().to_bytes(),
        "the VK bytes do not depend on the transcript"
    );
    let witness = setup.exported.witness(&pk).expect("witness");
    KagemushaKeys { pk, witness }
}

/// One native oracle-mode KAGEMUSHA proof with the 32-byte `seed`.
pub fn prove_native_kagemusha<B: CurveBridge>(
    setup: &Setup<B>,
    keys: &KagemushaKeys<B>,
    seed: [u8; 32],
) -> Vec<u8> {
    create_proof_oracle(
        &setup.params,
        &keys.pk,
        &keys.witness,
        ProverRandomness::fixed_seed_for_tests(seed),
        ProverConfig::default(),
        setup.transcript_repr,
    )
    .expect("native KAGEMUSHA proof")
}

/// Proves every golden case of `family` over `B` at every `k` on both
/// paths and requires equal bytes and mutual acceptance.
fn kagemusha_goldens<B: CurveBridge>(family: Family, ks: &[u32]) {
    for &k in ks {
        let setup = setup::<B>(family, k);
        let keys = kagemusha_keys(&setup);
        for case in cases_for(family, B::NAME, k) {
            let label = format!("{} (KAGEMUSHA path)", case.name);
            let seed = case.seed_bytes();
            let native = same_on_each_pool(&label, |_| prove_native_kagemusha(&setup, &keys, seed));
            let vendored = setup.prove_vendored_kagemusha(seed);
            assert!(
                native == vendored,
                "{label}: native {} bytes, vendored {} bytes, first differing message {:?}",
                native.len(),
                vendored.len(),
                first_difference(&native, &vendored),
            );
            verify_full_oracle(
                &setup.params,
                keys.pk.binding(),
                keys.pk.vk(),
                &setup.native_instances(),
                &vendored,
                MemoryBudget::DEFAULT,
                setup.transcript_repr,
            )
            .unwrap_or_else(|error| panic!("{label}: native verifier rejects: {error}"));
            setup
                .verify_vendored_kagemusha(&setup.vendored_instances(), &native)
                .unwrap_or_else(|error| panic!("{label}: vendored verifier rejects: {error}"));
            println!(
                "NATIVE_KAGEMUSHA case={} bytes={} sha256={} pools=1,2,4,7",
                case.name,
                native.len(),
                digest_hex(&native)
            );
        }
    }
}

#[test]
fn sigma_eq_kagemusha_proofs_match_vendored() {
    kagemusha_goldens::<Vesta>(Family::Sigma, &[6, 9]);
}

#[test]
fn sigma_ep_kagemusha_proofs_match_vendored() {
    kagemusha_goldens::<Pallas>(Family::Sigma, &[6, 9]);
}

#[test]
fn wide_eq_kagemusha_proofs_match_vendored() {
    kagemusha_goldens::<Vesta>(Family::Wide, &[8, 10]);
}

#[test]
fn wide_ep_kagemusha_proofs_match_vendored() {
    kagemusha_goldens::<Pallas>(Family::Wide, &[8, 10]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_k11_kagemusha_proofs_match_vendored() {
    kagemusha_goldens::<Vesta>(Family::Sigma, &[11]);
    kagemusha_goldens::<Pallas>(Family::Sigma, &[11]);
}
