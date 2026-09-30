//! Canonical policy, private PRF transcript and encrypted envelope controls.

use super::*;

#[test]
fn backend_names_identify_the_evaluator_and_reject_retired_hash_tags() {
    for (backend, name) in [
        (RamLfeBackend::HkdfSha3_512PrfV1, "hkdf-sha3-512-prf-v1"),
        (RamLfeBackend::BfvAffineV1, "bfv-affine-v1"),
        (RamLfeBackend::BfvProgrammedV1, "bfv-programmed-v1"),
    ] {
        assert_eq!(backend.as_str(), name);
        let bytes = norito::encode_canonical(&backend).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RamLfeBackend>(&bytes).unwrap(),
            backend
        );
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&backend).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(
                norito::json::from_str::<RamLfeBackend>(&json).unwrap(),
                backend
            );
        }
    }
    #[cfg(feature = "json")]
    for retired in ["bfv-affine-sha3-256-v1", "bfv-programmed-sha3-256-v1"] {
        assert!(norito::json::from_str::<RamLfeBackend>(&format!("\"{retired}\"")).is_err());
    }
}

// Independent owned reference shapes share the explicitly declared protocol
// identity. They do not make arbitrary reference types schema-transparent.
#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::HkdfRequestInputV1",
    frame = "iroha_crypto::ram_lfe::HkdfRequestInputV1"
)]
struct OwnedHkdfRequest {
    policy_hash: Hash,
    public_parameters: Vec<u8>,
    associated_data: Vec<u8>,
    normalized_input: Vec<u8>,
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::PolicyCommitmentInputV1",
    frame = "iroha_crypto::ram_lfe::PolicyCommitmentInputV1"
)]
struct OwnedPolicyCommitment {
    backend: RamLfeBackend,
    public_parameters: Vec<u8>,
    secret_commitment: [u8; 32],
}

#[test]
fn policy_and_prf_ignore_ambient_layout_and_keep_borrowed_frame_identity() {
    let secret = b"canonical-prf-secret";
    let commitment = policy_commitment(secret, b"phone#retail".to_vec()).unwrap();
    let request = ClientRequest {
        normalized_input: b"+15551234567".to_vec(),
        associated_data: b"phone#retail".to_vec(),
    };
    let expected = evaluate_commitment(secret, &commitment, &request).unwrap();
    let secret_commitment = policy_secret::commit(commitment.backend, secret).unwrap();
    let owned_policy = norito::encode_canonical(&OwnedPolicyCommitment {
        backend: commitment.backend,
        public_parameters: commitment.public_parameters.clone(),
        secret_commitment,
    })
    .unwrap();
    let borrowed_policy = norito::encode_canonical(&PolicyCommitmentInputV1 {
        backend: commitment.backend,
        public_parameters: &commitment.public_parameters,
        secret_commitment,
    })
    .unwrap();
    assert_eq!(borrowed_policy, owned_policy);
    assert_eq!(
        commitment.policy_hash,
        Hash::new_from_chunks(&[POLICY_DOMAIN, &owned_policy])
    );

    let owned_frame = norito::encode_canonical(&OwnedHkdfRequest {
        policy_hash: commitment.policy_hash,
        public_parameters: commitment.public_parameters.clone(),
        associated_data: request.associated_data.clone(),
        normalized_input: request.normalized_input.clone(),
    })
    .unwrap();
    // Exercise the external HKDF concatenation contract independently of the
    // production borrowed multi-part path; every field and domain stays ordered.
    let salt = [HKDF_SALT_DOMAIN, commitment.policy_hash.as_ref()].concat();
    let hkdf = Hkdf::<Sha3_512>::new(Some(&salt), secret);
    let mut opaque_material = [0_u8; Hash::LENGTH];
    hkdf.expand(
        &[HKDF_OPAQUE_INFO_DOMAIN, &owned_frame].concat(),
        &mut opaque_material,
    )
    .unwrap();
    let opaque = Hash::new_from_chunks(&[OPAQUE_HASH_DOMAIN, &opaque_material]);
    let mut receipt_material = [0_u8; Hash::LENGTH];
    hkdf.expand(
        &[HKDF_RECEIPT_INFO_DOMAIN, &owned_frame, opaque.as_ref()].concat(),
        &mut receipt_material,
    )
    .unwrap();
    let receipt = Hash::new_from_chunks(&[RECEIPT_HASH_DOMAIN, &receipt_material, opaque.as_ref()]);
    assert_eq!(expected.opaque_id, opaque);
    assert_eq!(expected.receipt_hash, receipt);
    assert_eq!(expected.output, request.normalized_input);
    for flags in [0, norito::core::default_encode_flags()] {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            policy_commitment(secret, commitment.public_parameters.clone()).unwrap(),
            commitment
        );
        assert_eq!(
            evaluate_commitment(secret, &commitment, &request).unwrap(),
            expected
        );
        let borrowed =
            hkdf_request_transcript(commitment.policy_hash, &commitment, &request).unwrap();
        assert_eq!(&*borrowed, &owned_frame);
    }
    for altered in [
        ClientRequest {
            associated_data: b"phone#other".to_vec(),
            ..request.clone()
        },
        ClientRequest {
            normalized_input: b"+15551234568".to_vec(),
            ..request
        },
    ] {
        let changed = evaluate_commitment(secret, &commitment, &altered).unwrap();
        assert_ne!(changed.opaque_id, expected.opaque_id);
        assert_ne!(changed.receipt_hash, expected.receipt_hash);
    }
}

#[test]
fn bfv_policy_normalizes_explicit_layout_before_committing() {
    let secret = b"canonical-bfv-policy";
    let (encryption, _, relinearization_key) = derive_identifier_key_material_from_seed(
        &crate::ram_lfe_bfv_parameters_v1(),
        63,
        secret,
        b"phone#retail",
    )
    .unwrap();
    let program = default_bfv_programmed_hidden_program();
    let public = try_bfv_programmed_public_parameters(
        encryption.clone(),
        BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        },
    )
    .unwrap();
    let canonical_affine = norito::encode_canonical(&encryption).unwrap();
    let canonical_programmed = norito::encode_canonical(&public).unwrap();
    let affine = bfv_affine_policy_commitment(secret, &canonical_affine).unwrap();
    let programmed =
        bfv_programmed_policy_commitment_with_program(secret, &canonical_programmed, &program)
            .unwrap();
    for flags in [0, norito::core::default_encode_flags()] {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let other_affine = norito::to_bytes(&encryption).unwrap();
        let other_programmed = norito::to_bytes(&public).unwrap();
        if flags != norito::core::default_encode_flags() {
            assert_ne!(other_affine, canonical_affine);
            assert_ne!(other_programmed, canonical_programmed);
        }
        let actual_affine = bfv_affine_policy_commitment(secret, &other_affine).unwrap();
        assert_eq!(actual_affine, affine);
        assert_eq!(actual_affine.public_parameters, canonical_affine);
        let actual_programmed =
            bfv_programmed_policy_commitment_with_program(secret, &other_programmed, &program)
                .unwrap();
        assert_eq!(actual_programmed, programmed);
        assert_eq!(actual_programmed.public_parameters, canonical_programmed);
    }
    assert!(bfv_affine_policy_commitment(secret, b"invalid parameters").is_err());
}

#[test]
fn registered_ciphertext_envelope_has_fixed_canonical_length() {
    let (encryption, _, _) = derive_identifier_key_material_from_seed(
        &crate::ram_lfe_bfv_parameters_v1(),
        63,
        b"canonical-envelope",
        b"phone#retail",
    )
    .unwrap();
    let ciphertext =
        crate::encrypt_identifier_from_seed(&encryption, b"+15551234567", b"canonical-ciphertext")
            .unwrap();
    assert_eq!(ciphertext.slots.len(), 64);
    let canonical = norito::encode_canonical(&ciphertext).unwrap();
    assert_eq!(canonical.len(), 75_187);
    for flags in [0, norito::core::default_encode_flags()] {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            norito::canonical_frame_len(&ciphertext).unwrap(),
            canonical.len()
        );
        assert_eq!(norito::encode_canonical(&ciphertext).unwrap(), canonical);
    }
}
