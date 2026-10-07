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
fn retired_and_unsupported_backends_fail_before_native_key_admission() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();

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
    let retired = ProofBox::new("halo2/ipa".to_owned(), vec![1, 2, 3, 4]);
    assert_eq!(
        transaction.preverify_proof(&retired, None, 0, None, None, true),
        PreverifyResult::UnsupportedBackend,
        "retired generic Halo2 never enters native key admission"
    );
}
#[test]
fn stark_fri_profile_labels_require_enveloped_state_preverify_metadata() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0);
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
fn native_pipa_r_profile_labels_require_the_canonical_backend() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();

    for backend in [
        "halo2/ipa:ivm-replay-binding-v1",
        "pipa-r/pasta:kaigi-usage-v1",
    ] {
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
        let vk_commitment = crate::zk::hash_vk(&vk);
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::NativePipaRPasta,
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
            PreverifyResult::UnsupportedBackend,
            "circuit identity must not be embedded in the canonical backend tag"
        );
    }
}

#[test]
fn native_backend_key_and_envelope_refusals_preserve_original_dedup_and_retry() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let native = crate::zk::native_pipa_r::NativeRelationV1::KaigiUsage;
    for (backend, tag, circuit_id, schema) in [
        (
            crate::zk::ZK_BACKEND_NATIVE_PIPA_R,
            BackendTag::NativePipaRPasta,
            native.circuit_id().to_owned(),
            crate::zk::native_pipa_r::public_schema(native).to_vec(),
        ),
        (
            crate::zk::ZK_BACKEND_STARK_FRI_V1,
            BackendTag::Stark,
            format!(
                "{}:state-preverify-test",
                crate::zk::ZK_BACKEND_STARK_FRI_V1
            ),
            vec![0x55; 32],
        ),
    ] {
        // This is lightweight envelope admission, not a cryptographic proof fixture.
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
        let commitment = crate::zk::hash_vk(&vk);
        let envelope = OpenVerifyEnvelope {
            backend: tag,
            circuit_id,
            vk_hash: commitment,
            public_inputs: schema,
            proof_bytes: vec![0xAA, 0xBB, 0xCC],
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            backend.to_owned(),
            norito::encode_canonical(&envelope).expect("encode current native OpenVerifyEnvelope"),
        );
        let source = proof.bytes.as_ptr();
        let original_bytes = proof.bytes.clone();
        let mut mismatch = commitment;
        mismatch[0] ^= 1;
        for (actual, expected, active, refusal) in [
            (None, None, true, PreverifyResult::VerifyingKeyMissing),
            (
                Some(commitment),
                Some(commitment),
                false,
                PreverifyResult::VerifyingKeyInactive,
            ),
            (
                Some(mismatch),
                Some(commitment),
                true,
                PreverifyResult::VerifyingKeyMismatch,
            ),
        ] {
            assert_eq!(
                transaction.preverify_proof(&proof, Some(&vk), 0, actual, expected, active),
                refusal,
                "{backend} retains original key admission before dedup",
            );
            let mut observed = transaction.zk_dedup.clone();
            assert!(observed.check_and_insert_with_commitment(&proof, Some(commitment)));
            assert_eq!(proof.bytes.as_ptr(), source);
            assert_eq!(proof.bytes, original_bytes);
        }
        let other_backend = if tag == BackendTag::NativePipaRPasta {
            crate::zk::ZK_BACKEND_STARK_FRI_V1
        } else {
            crate::zk::ZK_BACKEND_NATIVE_PIPA_R
        };
        let foreign_vk = VerifyingKeyBox::new(other_backend.to_owned(), vk.bytes.clone());
        assert_eq!(
            transaction.preverify_proof(
                &proof,
                Some(&foreign_vk),
                0,
                Some(commitment),
                Some(commitment),
                true
            ),
            PreverifyResult::VerifyingKeyMismatch,
            "foreign key backend cannot replace the original native key",
        );
        let malformed = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
        assert_eq!(
            transaction.preverify_proof(
                &malformed,
                Some(&vk),
                0,
                Some(commitment),
                Some(commitment),
                true
            ),
            PreverifyResult::MalformedProof,
            "supported backends retain canonical envelope admission",
        );
        let mut observed = transaction.zk_dedup.clone();
        assert!(observed.check_and_insert_with_commitment(&malformed, Some(commitment)));
        assert!(observed.check_and_insert_with_commitment(&proof, Some(commitment)));
        assert_eq!(
            transaction.preverify_proof(
                &proof,
                Some(&vk),
                0,
                Some(commitment),
                Some(commitment),
                true
            ),
            PreverifyResult::Accepted,
            "same original native proof retries after key and envelope refusals",
        );
        assert_eq!(
            transaction.preverify_proof(
                &proof,
                Some(&vk),
                0,
                Some(commitment),
                Some(commitment),
                true
            ),
            PreverifyResult::Duplicate,
            "only accepted native admission inserts the original dedup identity",
        );
        assert_eq!(proof.bytes.as_ptr(), source);
        assert_eq!(proof.bytes, original_bytes);
    }
}
