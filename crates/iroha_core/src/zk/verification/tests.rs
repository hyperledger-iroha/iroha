//! Relation-confusion and fail-closed coverage for the public verifier API.

use super::*;
use crate::zk::{
    IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID, ZK_BACKEND_HALO2_IPA, ZK_BACKEND_STARK_FRI_V1,
};

fn policy() -> ZkVerifyGuardrails {
    ZkVerifyGuardrails {
        halo2_enabled: true,
        halo2_max_envelope_bytes: 1024 * 1024,
        halo2_max_proof_bytes: 1024 * 1024,
        stark_enabled: true,
        stark_max_envelope_bytes: 1024 * 1024,
        stark_max_proof_bytes: 1024 * 1024,
    }
}

fn envelope(backend: BackendTag, circuit: &str) -> Vec<u8> {
    norito::encode_canonical(&OpenVerifyEnvelope {
        backend,
        circuit_id: circuit.to_owned(),
        vk_hash: [1; 32],
        public_inputs: vec![1],
        proof_bytes: vec![1],
        aux: Vec::new(),
    })
    .expect("canonical test envelope")
}

#[test]
fn admission_rejects_policy_and_size_before_decoding() {
    let key = VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
    let proof = ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0; 2]);
    let mut limits = policy();
    limits.halo2_enabled = false;
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, limits),
        Err(ProofVerificationError::BackendDisabled)
    );
    limits.halo2_enabled = true;
    limits.halo2_max_envelope_bytes = 1;
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, limits),
        Err(ProofVerificationError::EnvelopeTooLarge {
            actual: 2,
            maximum: 1
        })
    );
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, policy()),
        Err(ProofVerificationError::MalformedEnvelope)
    );
    let unsupported = ProofBox::new("not-a-proof-backend".to_owned(), proof.bytes);
    assert_eq!(
        verify_for_relation(
            ProofRelation::IvmReplayBinding,
            &unsupported,
            &key,
            policy()
        ),
        Err(ProofVerificationError::UnsupportedBackend)
    );
}

#[test]
fn every_admitted_halo2_circuit_has_one_explicit_relation() {
    use ProofRelation::*;
    let relations = [
        IvmReplayBinding,
        KaigiAuthorization,
        KaigiUsage,
        ConfidentialTransfer,
        ConfidentialFullUnshield,
        ConfidentialChangeUnshield,
    ];
    assert_eq!(
        crate::zk::HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1.len(),
        relations.len()
    );
    for (circuit, expected) in crate::zk::HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1
        .iter()
        .zip(relations)
    {
        assert_eq!(
            compiled_relation(ZK_BACKEND_HALO2_IPA, circuit),
            Some(expected)
        );
        assert_eq!(compiled_relation(ZK_BACKEND_STARK_FRI_V1, circuit), None);
    }
    assert_eq!(
        compiled_relation(ZK_BACKEND_HALO2_IPA, "halo2/pasta/ipa/unreviewed"),
        None
    );
}

#[test]
fn public_binding_cannot_satisfy_a_confidential_requirement() {
    let backend = ZK_BACKEND_STARK_FRI_V1;
    let circuit = format!("{backend}:public-binding-demo");
    let proof = ProofBox::new(backend.to_owned(), envelope(BackendTag::Stark, &circuit));
    let key = VerifyingKeyBox::new(backend.to_owned(), Vec::new());
    assert_eq!(
        verify_for_relation(ProofRelation::ConfidentialTransfer, &proof, &key, policy()),
        Err(ProofVerificationError::RelationMismatch {
            expected: ProofRelation::ConfidentialTransfer,
            actual: ProofRelation::PublicInputBinding,
        })
    );
    assert_eq!(
        verify_for_relation(ProofRelation::PublicInputBinding, &proof, &key, policy()),
        Err(ProofVerificationError::VerifyingKeyMismatch)
    );
    let mut decoded: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).unwrap();
    decoded.vk_hash = crate::zk::hash_vk(&key);
    let proof = ProofBox::new(
        backend.to_owned(),
        norito::encode_canonical(&decoded).unwrap(),
    );
    // Matching a label and key digest is insufficient: native verification must succeed.
    assert_eq!(
        verify_for_relation(ProofRelation::PublicInputBinding, &proof, &key, policy()),
        Err(ProofVerificationError::InvalidProof)
    );
    for reserved in [
        iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V1_CIRCUIT_ID,
        "zk-ace-other",
        crate::zk::GOVERNANCE_BALLOT_CIRCUIT_ID_V1,
        crate::zk::GOVERNANCE_TALLY_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
    ] {
        let circuit = format!("{backend}:{reserved}");
        // Dedicated protocols must not fall through to the public binding AIR.
        assert_eq!(compiled_relation(backend, &circuit), None, "{circuit}");
    }
}

#[test]
fn backend_confusion_and_unknown_circuits_reject_before_crypto() {
    let key = VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
    let wrong_backend = ProofBox::new(
        ZK_BACKEND_HALO2_IPA.to_owned(),
        envelope(
            BackendTag::Stark,
            IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID,
        ),
    );
    assert_eq!(
        verify_for_relation(
            ProofRelation::IvmReplayBinding,
            &wrong_backend,
            &key,
            policy()
        ),
        Err(ProofVerificationError::MalformedEnvelope)
    );
    let unknown = ProofBox::new(
        ZK_BACKEND_HALO2_IPA.to_owned(),
        envelope(BackendTag::Halo2IpaPasta, "halo2/pasta/ipa/unreviewed"),
    );
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &unknown, &key, policy()),
        Err(ProofVerificationError::UnsupportedRelation)
    );
    let oversized = ProofBox::new(
        ZK_BACKEND_HALO2_IPA.to_owned(),
        envelope(
            BackendTag::Halo2IpaPasta,
            IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID,
        ),
    );
    let mut limits = policy();
    limits.halo2_max_proof_bytes = 0;
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &oversized, &key, limits),
        Err(ProofVerificationError::ProofTooLarge {
            actual: 1,
            maximum: 0
        })
    );
}

#[cfg(feature = "zk-halo2-ipa")]
#[test]
fn native_proof_verifies_only_as_replay_binding_and_rejects_tampering() {
    use crate::zk::{
        halo2_ipa_ivm_replay_binding_vk_box, prove_halo2_ipa_ivm_replay_binding_envelope,
    };
    use iroha_crypto::Hash;

    let key = halo2_ipa_ivm_replay_binding_vk_box().expect("canonical key");
    let proof = prove_halo2_ipa_ivm_replay_binding_envelope(
        IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID,
        &key,
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
        None,
    )
    .expect("binding proof");
    let verified = verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, policy())
        .expect("native proof verifies");
    assert_eq!(verified.relation(), ProofRelation::IvmReplayBinding);
    let _ = verified.elapsed();
    let record = crate::zk::halo2_ipa_ivm_replay_binding_vk_record("core", 1)
        .expect("canonical registry record");
    assert_eq!(
        record.circuit_id,
        IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID
    );
    crate::zk::validate_and_prepare_verifying_key_record_v1(
        &iroha_data_model::proof::VerifyingKeyId::new(key.backend.clone(), "replay-binding"),
        &record,
    )
    .expect("builder emits an admissible complete registry record");
    for circuit_id in [
        "ivm-replay-binding-v1",
        "halo2/pasta/ivm-replay-binding-v1",
        "halo2/ipa:ivm-replay-binding-v1",
        "halo2/pasta/ipa/ivm-execution-v1",
    ] {
        let mut relabelled: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).unwrap();
        relabelled.circuit_id = circuit_id.to_owned();
        let relabelled = ProofBox::new(
            proof.backend.clone(),
            norito::encode_canonical(&relabelled).unwrap(),
        );
        assert_eq!(
            verify_for_relation(ProofRelation::IvmReplayBinding, &relabelled, &key, policy()),
            Err(ProofVerificationError::UnsupportedRelation),
            "accepted {circuit_id}"
        );
    }
    let wrong_key = VerifyingKeyBox::new(key.backend.clone(), vec![1, 2, 3]);
    assert_eq!(
        verify_for_relation(
            ProofRelation::IvmReplayBinding,
            &proof,
            &wrong_key,
            policy()
        ),
        Err(ProofVerificationError::VerifyingKeyMismatch)
    );
    assert_eq!(
        verify_for_relation(ProofRelation::ConfidentialTransfer, &proof, &key, policy()),
        Err(ProofVerificationError::RelationMismatch {
            expected: ProofRelation::ConfidentialTransfer,
            actual: ProofRelation::IvmReplayBinding,
        })
    );
    // These supported-relation controls replace the retired RAM-LFE generic
    // verifier's metadata and resource assertions. Start with an actual valid
    // proof so a malformed native fixture cannot make every negative pass.
    let original: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).expect("envelope");
    let metadata_cases: [(&str, fn(&mut OpenVerifyEnvelope), ProofVerificationError); 5] = [
        (
            "backend tag",
            |e| e.backend = BackendTag::Stark,
            ProofVerificationError::MalformedEnvelope,
        ),
        (
            "auxiliary bytes",
            |e| e.aux = b"unbound-metadata".to_vec(),
            ProofVerificationError::InvalidProof,
        ),
        (
            "zero key hash",
            |e| e.vk_hash = [0; 32],
            ProofVerificationError::VerifyingKeyMismatch,
        ),
        (
            "wrong key hash",
            |e| e.vk_hash = [0xa5; 32],
            ProofVerificationError::VerifyingKeyMismatch,
        ),
        (
            "schema drift",
            |e| e.public_inputs.extend_from_slice(b":drift"),
            ProofVerificationError::InvalidProof,
        ),
    ];
    for (label, mutate, expected) in metadata_cases {
        let mut changed = original.clone();
        mutate(&mut changed);
        let changed = ProofBox::new(
            proof.backend.clone(),
            norito::encode_canonical(&changed).expect("mutated envelope"),
        );
        assert_eq!(
            verify_for_relation(ProofRelation::IvmReplayBinding, &changed, &key, policy()),
            Err(expected),
            "{label}"
        );
    }
    for backend in [
        "halo2/ipa:debug",
        "halo2/ipa:trusted-setup",
        "halo2/ipa:production-ready",
    ] {
        let changed = ProofBox::new(backend.to_owned(), proof.bytes.clone());
        assert_eq!(
            verify_for_relation(ProofRelation::IvmReplayBinding, &changed, &key, policy()),
            Err(ProofVerificationError::UnsupportedBackend),
            "{backend}"
        );
    }
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&original).expect("alternate layout")
    };
    assert_ne!(alternate, proof.bytes);
    assert_eq!(
        verify_for_relation(
            ProofRelation::IvmReplayBinding,
            &ProofBox::new(proof.backend.clone(), alternate),
            &key,
            policy(),
        ),
        Err(ProofVerificationError::MalformedEnvelope)
    );
    let mut disabled = policy();
    disabled.halo2_enabled = false;
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, disabled),
        Err(ProofVerificationError::BackendDisabled)
    );
    let mut envelope_limited = policy();
    envelope_limited.halo2_max_envelope_bytes = proof.bytes.len() - 1;
    assert_eq!(
        verify_for_relation(
            ProofRelation::IvmReplayBinding,
            &proof,
            &key,
            envelope_limited
        ),
        Err(ProofVerificationError::EnvelopeTooLarge {
            actual: proof.bytes.len(),
            maximum: proof.bytes.len() - 1,
        })
    );
    let mut proof_limited = policy();
    proof_limited.halo2_max_proof_bytes = original.proof_bytes.len() - 1;
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &proof, &key, proof_limited),
        Err(ProofVerificationError::ProofTooLarge {
            actual: original.proof_bytes.len(),
            maximum: original.proof_bytes.len() - 1,
        })
    );
    let mut decoded: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes).expect("envelope");
    *decoded.proof_bytes.last_mut().expect("native proof bytes") ^= 1;
    let tampered = ProofBox::new(
        proof.backend,
        norito::encode_canonical(&decoded).expect("encode tampered envelope"),
    );
    assert_eq!(
        verify_for_relation(ProofRelation::IvmReplayBinding, &tampered, &key, policy()),
        Err(ProofVerificationError::InvalidProof)
    );
}
