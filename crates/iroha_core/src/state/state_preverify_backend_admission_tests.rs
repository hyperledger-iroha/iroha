//! State preverification backend-admission regression tests.
use super::*;
use crate::{kura::Kura, zk::PreverifyResult};
use iroha_data_model::{
    block::BlockHeader,
    proof::{ProofBox, VerifyingKeyBox},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use std::{num::NonZeroU64, sync::Arc};
#[test]
fn unsupported_halo2_looking_backends_fail_backend_admission_before_curve_policy() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction.zk.halo2.curve = iroha_config::parameters::actual::ZkCurve::Bn254;
    for backend in [
        "halo2/bn254",
        "halo2/bn254/vote",
        "halo2/kzg",
        "halo2/debug",
        "halo2/mock",
        "halo2/unknown-native-v1",
        "halo2/ipa:production-ready",
        "halo2/ipa:claimed-mainnet",
    ] {
        let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
        assert_eq!(
            transaction.preverify_proof(&proof, None, 0, None, None, true),
            PreverifyResult::UnsupportedBackend,
            "case {backend}"
        );
    }
    let admitted = ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![1, 2, 3, 4]);
    assert_eq!(
        transaction.preverify_proof(&admitted, None, 0, None, None, true),
        PreverifyResult::CurveNotAllowed,
        "admitted Halo2/Pasta backends must still honor curve policy"
    );
}
#[test]
fn stark_fri_profile_labels_require_enveloped_state_preverify_metadata() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    for backend in [crate::zk::ZK_BACKEND_STARK_FRI_V1] {
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
        let vk_commitment = crate::zk::hash_vk(&vk);
        let raw = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
        assert_eq!(
            transaction.preverify_proof(
                &raw,
                Some(&vk),
                0,
                Some(vk_commitment),
                Some(vk_commitment),
                true,
            ),
            PreverifyResult::MalformedProof,
            "state preverify must require OpenVerifyEnvelope metadata for {backend}"
        );
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: format!("{backend}:state-preverify-test"),
            vk_hash: vk_commitment,
            public_inputs: vec![0x55; 32],
            proof_bytes: vec![0xAA, 0xBB, 0xCC],
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
        );
        assert_eq!(
            transaction.preverify_proof(
                &proof,
                Some(&vk),
                0,
                Some(vk_commitment),
                Some(vk_commitment),
                true,
            ),
            PreverifyResult::Accepted,
            "malformed raw payload for {backend} must not poison state preverify dedup"
        );
    }
}
#[test]
fn halo2_ipa_profile_labels_use_family_curve_segment() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction.zk.halo2.curve = iroha_config::parameters::actual::ZkCurve::Pallas;
    let backend = "halo2/ipa:ivm-execution-v1";
    let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
    let vk_commitment = crate::zk::hash_vk(&vk);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: backend.to_owned(),
        vk_hash: vk_commitment,
        public_inputs: vec![0x55; 32],
        proof_bytes: vec![0xAA, 0xBB, 0xCC],
        aux: Vec::new(),
    };
    let proof = ProofBox::new(
        backend.to_owned(),
        norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
    );
    assert_eq!(
        transaction.preverify_proof(
            &proof,
            None,
            0,
            Some(vk_commitment),
            Some(vk_commitment),
            true,
        ),
        PreverifyResult::Accepted,
        "Halo2 IPA profile labels must be checked as IPA/Pasta labels before metadata preverify"
    );
}
