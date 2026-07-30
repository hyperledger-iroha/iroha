    #[test]
    fn fhe_input_admission_proof_validate_rejects_public_input_shape_replay() {
        let sample = sample_fhe_input_admission_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");
        let statement = <[u8; Hash::LENGTH]>::from(sample.statement_hash);
        let other_statement = <[u8; Hash::LENGTH]>::from(sample_hash(21));

        let assert_rejected = |label: &str, public_inputs: Vec<Vec<[u8; Hash::LENGTH]>>| {
            let mut proof = sample.clone();
            let mut replay_envelope = envelope.clone();
            let mut replay_open = open_proof.clone();
            replay_open.public_inputs = public_inputs;
            replay_envelope.proof_bytes =
                norito::to_bytes(&replay_open).expect("encode replay-shaped STARK wrapper");
            replace_fhe_input_admission_open_verify_envelope(&mut proof, &replay_envelope);
            let err = proof.validate().expect_err(label);
            assert!(matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ));
        };

        assert_rejected(
            "extra STARK public-input row must be rejected",
            vec![vec![statement], vec![other_statement]],
        );
        assert_rejected(
            "extra STARK public-input column must be rejected",
            vec![vec![statement, other_statement]],
        );
        assert_rejected(
            "duplicate STARK public-input statement must be rejected",
            vec![vec![statement], vec![statement]],
        );
    }

    #[test]
    fn fhe_input_admission_open_verify_bounds_match_published_caps() {
        let bounds = soracloud_fhe_input_admission_open_verify_bounds();
        assert_eq!(
            bounds.max_circuit_id_bytes,
            SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1.len()
        );
        assert_eq!(
            bounds.max_public_input_bytes,
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1.len()
        );
        assert_eq!(
            bounds.max_proof_bytes,
            SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES
        );
        assert_eq!(bounds.max_aux_bytes, 0);
        assert!(!bounds.allow_aux);
        assert!(bounds.require_nonzero_vk_hash);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_input_admission_proof_validate_rejects_oversized_proof_payloads() {
        let sample = sample_fhe_input_admission_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut oversized_outer = sample.clone();
        oversized_outer.proof.proof.bytes =
            vec![0xA5; SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES + 1];
        oversized_outer.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(
            &oversized_outer.proof.proof.bytes,
        )));
        let err = oversized_outer
            .validate()
            .expect_err("oversized OpenVerify envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("OpenVerifyEnvelope length"),
            "unexpected error: {err}"
        );

        let mut oversized_circuit = sample.clone();
        let mut oversized_circuit_envelope = envelope.clone();
        oversized_circuit_envelope.circuit_id =
            format!("{SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1}_x");
        replace_fhe_input_admission_open_verify_envelope(
            &mut oversized_circuit,
            &oversized_circuit_envelope,
        );
        let err = oversized_circuit
            .validate()
            .expect_err("oversized OpenVerify circuit id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("circuit id length"),
            "unexpected error: {err}"
        );

        let mut oversized_schema = sample.clone();
        let mut oversized_schema_envelope = envelope.clone();
        oversized_schema_envelope.public_inputs =
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1.to_vec();
        oversized_schema_envelope.public_inputs.push(b'x');
        replace_fhe_input_admission_open_verify_envelope(
            &mut oversized_schema,
            &oversized_schema_envelope,
        );
        let err = oversized_schema
            .validate()
            .expect_err("oversized OpenVerify public-input schema must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("public inputs length"),
            "unexpected error: {err}"
        );

        let mut oversized_wrapper = sample.clone();
        let mut oversized_wrapper_envelope = envelope.clone();
        oversized_wrapper_envelope.proof_bytes =
            vec![0xA5; SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES + 1];
        replace_fhe_input_admission_open_verify_envelope(
            &mut oversized_wrapper,
            &oversized_wrapper_envelope,
        );
        let err = oversized_wrapper
            .validate()
            .expect_err("oversized STARK wrapper bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("proof bytes length"),
            "unexpected error: {err}"
        );

        let mut oversized_native = sample;
        let mut oversized_native_envelope = envelope;
        let mut oversized_native_open = open_proof;
        oversized_native_open.envelope_bytes =
            vec![0xA5; SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES + 1];
        oversized_native_envelope.proof_bytes =
            norito::to_bytes(&oversized_native_open).expect("encode oversized STARK wrapper");
        replace_fhe_input_admission_open_verify_envelope(
            &mut oversized_native,
            &oversized_native_envelope,
        );
        let err = oversized_native
            .validate()
            .expect_err("oversized native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_input_admission_proof_validate_requires_canonical_vk_ref_name() {
        let mut admission = sample_fhe_input_admission_proof();
        admission.proof.vk_ref.name = "soracloud_fhe_input_admission_alias_v1".to_string();

        let err = admission
            .validate()
            .expect_err("non-canonical FHE verifier id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));
    }

    #[test]
    fn fhe_input_admission_proof_validate_rejects_backend_mismatch() {
        let mut admission = sample_fhe_input_admission_proof();
        admission.proof.proof.backend = "stark/fri/other".into();

        let err = admission
            .validate()
            .expect_err("mismatched proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut wrong_stark_profile = sample_fhe_input_admission_proof();
        wrong_stark_profile.proof.backend = "stark/fri/poseidon2-goldilocks".into();
        wrong_stark_profile.proof.proof.backend = wrong_stark_profile.proof.backend.clone();
        wrong_stark_profile.proof.vk_ref.backend = wrong_stark_profile.proof.backend.clone();
        let err = wrong_stark_profile
            .validate()
            .expect_err("alternate STARK/FRI profile must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
        assert!(
            err.to_string().contains("canonical BFV STARK/FRI backend"),
            "unexpected error: {err}"
        );

        let mut unsupported = sample_fhe_input_admission_proof();
        unsupported.proof.backend = "stark/fri/debug-proof".into();
        unsupported.proof.proof.backend = unsupported.proof.backend.clone();
        unsupported.proof.vk_ref.backend = unsupported.proof.backend.clone();
        let err = unsupported
            .validate()
            .expect_err("unsupported FHE admission backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
    }

    #[test]
    fn fhe_public_key_proof_validate_accepts_canonical_envelope() {
        let proof = sample_fhe_public_key_proof();
        proof
            .validate()
            .expect("canonical public-key proof envelope must validate");
        assert_eq!(
            soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1(),
            <[u8; 32]>::from(Hash::new(
                SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ))
        );
    }

    #[test]
    fn fhe_public_key_proof_validate_requires_vk_commitment_and_matching_envelope_hash() {
        let mut proof = sample_fhe_public_key_proof();
        proof.proof.vk_commitment = None;

        let err = proof
            .validate()
            .expect_err("missing vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        proof.proof.vk_commitment = Some([0x4A; 32]);
        proof.proof.envelope_hash = None;
        let err = proof
            .validate()
            .expect_err("missing envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));

        proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
        proof
            .validate()
            .expect("matching envelope hash must be accepted");

        let mut forged_commitment = proof.clone();
        forged_commitment.proof.vk_commitment = Some([0xA4; 32]);
        let err = forged_commitment
            .validate()
            .expect_err("forged vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut forged_hash = proof.proof.envelope_hash.expect("matching hash");
        forged_hash[0] ^= 0x01;
        proof.proof.envelope_hash = Some(forged_hash);
        let err = proof
            .validate()
            .expect_err("forged envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField { field: "proof", .. }
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_public_key_proof_validate_rejects_open_verify_envelope_drift() {
        let sample = sample_fhe_public_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut malformed = sample.clone();
        malformed.proof.proof.bytes = vec![0xAA];
        malformed.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&malformed.proof.proof.bytes)));
        let err = malformed
            .validate()
            .expect_err("malformed OpenVerify bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_backend = sample.clone();
        let mut wrong_backend_envelope = envelope.clone();
        wrong_backend_envelope.backend = BackendTag::Halo2IpaPasta;
        replace_fhe_public_key_open_verify_envelope(&mut wrong_backend, &wrong_backend_envelope);
        let err = wrong_backend
            .validate()
            .expect_err("OpenVerify backend drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_circuit = sample.clone();
        let mut wrong_circuit_envelope = envelope.clone();
        wrong_circuit_envelope.circuit_id = "soracloud_fhe_public_key_v2".to_string();
        replace_fhe_public_key_open_verify_envelope(&mut wrong_circuit, &wrong_circuit_envelope);
        let err = wrong_circuit
            .validate()
            .expect_err("OpenVerify circuit id drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string()
                .contains("OpenVerifyEnvelope circuit id must be canonical v1"),
            "unexpected error: {err}"
        );

        let mut wrong_wrapper_version = sample.clone();
        let mut wrong_wrapper_version_envelope = envelope.clone();
        let mut version_drift = open_proof.clone();
        version_drift.version = 2;
        wrong_wrapper_version_envelope.proof_bytes =
            norito::to_bytes(&version_drift).expect("encode version-drifted STARK wrapper");
        replace_fhe_public_key_open_verify_envelope(
            &mut wrong_wrapper_version,
            &wrong_wrapper_version_envelope,
        );
        let err = wrong_wrapper_version
            .validate()
            .expect_err("STARK wrapper version drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_statement = sample.clone();
        let mut wrong_statement_envelope = envelope.clone();
        let mut statement_drift = open_proof.clone();
        statement_drift.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(sample_hash(99))]];
        wrong_statement_envelope.proof_bytes =
            norito::to_bytes(&statement_drift).expect("encode statement-drifted STARK wrapper");
        replace_fhe_public_key_open_verify_envelope(
            &mut wrong_statement,
            &wrong_statement_envelope,
        );
        let err = wrong_statement
            .validate()
            .expect_err("STARK wrapper statement drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_schema = sample.clone();
        let mut wrong_schema_envelope = envelope.clone();
        wrong_schema_envelope.public_inputs = b"soracloud:fhe-public-key:public-inputs:v2".to_vec();
        replace_fhe_public_key_open_verify_envelope(&mut wrong_schema, &wrong_schema_envelope);
        let err = wrong_schema
            .validate()
            .expect_err("OpenVerify public-input schema drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut empty_native = sample.clone();
        let mut empty_native_envelope = envelope.clone();
        let mut empty_native_open = open_proof.clone();
        empty_native_open.envelope_bytes.clear();
        empty_native_envelope.proof_bytes =
            norito::to_bytes(&empty_native_open).expect("encode empty-native STARK wrapper");
        replace_fhe_public_key_open_verify_envelope(&mut empty_native, &empty_native_envelope);
        let err = empty_native
            .validate()
            .expect_err("empty native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut all_zero_native = sample;
        let mut all_zero_native_envelope = envelope;
        let mut all_zero_native_open = open_proof;
        all_zero_native_open.envelope_bytes = vec![0; 32];
        all_zero_native_envelope.proof_bytes =
            norito::to_bytes(&all_zero_native_open).expect("encode all-zero STARK wrapper");
        replace_fhe_public_key_open_verify_envelope(
            &mut all_zero_native,
            &all_zero_native_envelope,
        );
        let err = all_zero_native
            .validate()
            .expect_err("all-zero native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all-zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_public_key_proof_validate_rejects_public_input_shape_replay() {
        let sample = sample_fhe_public_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");
        let statement = <[u8; Hash::LENGTH]>::from(sample.statement_hash);
        let other_statement = <[u8; Hash::LENGTH]>::from(sample_hash(15));

        let assert_rejected = |label: &str, public_inputs: Vec<Vec<[u8; Hash::LENGTH]>>| {
            let mut proof = sample.clone();
            let mut replay_envelope = envelope.clone();
            let mut replay_open = open_proof.clone();
            replay_open.public_inputs = public_inputs;
            replay_envelope.proof_bytes =
                norito::to_bytes(&replay_open).expect("encode replay-shaped STARK wrapper");
            replace_fhe_public_key_open_verify_envelope(&mut proof, &replay_envelope);
            let err = proof.validate().expect_err(label);
            assert!(matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ));
        };

        assert_rejected(
            "extra STARK public-input row must be rejected",
            vec![vec![statement], vec![other_statement]],
        );
        assert_rejected(
            "extra STARK public-input column must be rejected",
            vec![vec![statement, other_statement]],
        );
        assert_rejected(
            "duplicate STARK public-input statement must be rejected",
            vec![vec![statement], vec![statement]],
        );
    }

    #[test]
    fn fhe_public_key_proof_open_verify_bounds_match_published_caps() {
        let bounds = soracloud_fhe_public_key_proof_open_verify_bounds();
        assert_eq!(
            bounds.max_circuit_id_bytes,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1.len()
        );
        assert_eq!(
            bounds.max_public_input_bytes,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len()
        );
        assert_eq!(
            bounds.max_proof_bytes,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES
        );
        assert_eq!(bounds.max_aux_bytes, 0);
        assert!(!bounds.allow_aux);
        assert!(bounds.require_nonzero_vk_hash);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_public_key_proof_validate_rejects_oversized_proof_payloads() {
        let sample = sample_fhe_public_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut oversized_outer = sample.clone();
        oversized_outer.proof.proof.bytes =
            vec![0xAA; SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES + 1];
        oversized_outer.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(
            &oversized_outer.proof.proof.bytes,
        )));
        let err = oversized_outer
            .validate()
            .expect_err("oversized OpenVerify envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("OpenVerifyEnvelope length"),
            "unexpected error: {err}"
        );

        let mut oversized_circuit = sample.clone();
        let mut oversized_circuit_envelope = envelope.clone();
        oversized_circuit_envelope.circuit_id =
            format!("{SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1}_x");
        replace_fhe_public_key_open_verify_envelope(
            &mut oversized_circuit,
            &oversized_circuit_envelope,
        );
        let err = oversized_circuit
            .validate()
            .expect_err("oversized OpenVerify circuit id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("circuit id length"),
            "unexpected error: {err}"
        );

        let mut oversized_schema = sample.clone();
        let mut oversized_schema_envelope = envelope.clone();
        oversized_schema_envelope.public_inputs =
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec();
        oversized_schema_envelope.public_inputs.push(b'x');
        replace_fhe_public_key_open_verify_envelope(
            &mut oversized_schema,
            &oversized_schema_envelope,
        );
        let err = oversized_schema
            .validate()
            .expect_err("oversized OpenVerify public-input schema must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("public inputs length"),
            "unexpected error: {err}"
        );

        let mut oversized_wrapper = sample.clone();
        let mut oversized_wrapper_envelope = envelope.clone();
        oversized_wrapper_envelope.proof_bytes =
            vec![0xAA; SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES + 1];
        replace_fhe_public_key_open_verify_envelope(
            &mut oversized_wrapper,
            &oversized_wrapper_envelope,
        );
        let err = oversized_wrapper
            .validate()
            .expect_err("oversized STARK wrapper bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("proof bytes length"),
            "unexpected error: {err}"
        );

        let mut oversized_native = sample;
        let mut oversized_native_envelope = envelope;
        let mut oversized_native_open = open_proof;
        oversized_native_open.envelope_bytes =
            vec![0xAA; SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 1];
        oversized_native_envelope.proof_bytes =
            norito::to_bytes(&oversized_native_open).expect("encode oversized STARK wrapper");
        replace_fhe_public_key_open_verify_envelope(
            &mut oversized_native,
            &oversized_native_envelope,
        );
        let err = oversized_native
            .validate()
            .expect_err("oversized native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_public_key_proof_validate_rejects_attachment_metadata_drift() {
        let mut proof_backend_mismatch = sample_fhe_public_key_proof();
        proof_backend_mismatch.proof.proof.backend = "stark/fri/other".into();
        let err = proof_backend_mismatch
            .validate()
            .expect_err("mismatched proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut vk_backend_mismatch = sample_fhe_public_key_proof();
        vk_backend_mismatch.proof.vk_ref.backend = "stark/fri/other".into();
        let err = vk_backend_mismatch
            .validate()
            .expect_err("mismatched verifier backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.backend",
                ..
            }
        ));

        let mut wrong_vk_ref = sample_fhe_public_key_proof();
        wrong_vk_ref.proof.vk_ref.name = "soracloud_fhe_public_key_alias_v1".to_string();
        let err = wrong_vk_ref
            .validate()
            .expect_err("non-canonical public-key verifier id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));

        let mut wrong_stark_profile = sample_fhe_public_key_proof();
        wrong_stark_profile.proof.backend = "stark/fri/poseidon2-goldilocks".into();
        wrong_stark_profile.proof.proof.backend = wrong_stark_profile.proof.backend.clone();
        wrong_stark_profile.proof.vk_ref.backend = wrong_stark_profile.proof.backend.clone();
        let err = wrong_stark_profile
            .validate()
            .expect_err("alternate STARK/FRI profile must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
        assert!(
            err.to_string().contains("canonical BFV STARK/FRI backend"),
            "unexpected error: {err}"
        );

        let mut unsupported = sample_fhe_public_key_proof();
        unsupported.proof.backend = "stark/fri/debug-proof".into();
        unsupported.proof.proof.backend = unsupported.proof.backend.clone();
        unsupported.proof.vk_ref.backend = unsupported.proof.backend.clone();
        let err = unsupported
            .validate()
            .expect_err("unsupported public-key proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
    }

    #[test]
    fn fhe_bootstrap_key_proof_validate_accepts_canonical_envelope() {
        let proof = sample_fhe_bootstrap_key_proof();
        proof
            .validate()
            .expect("canonical bootstrap-key proof envelope must validate");
        assert_eq!(
            soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1(),
            <[u8; 32]>::from(Hash::new(
                SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ))
        );
    }

    #[test]
    fn fhe_bootstrap_key_proof_validate_requires_vk_commitment_and_matching_envelope_hash() {
        let mut proof = sample_fhe_bootstrap_key_proof();
        proof.proof.vk_commitment = None;

        let err = proof
            .validate()
            .expect_err("missing vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        proof.proof.vk_commitment = Some([0x52; 32]);
        proof.proof.envelope_hash = None;
        let err = proof
            .validate()
            .expect_err("missing envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));

        proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
        proof
            .validate()
            .expect("matching envelope hash must be accepted");

        let mut forged_commitment = proof.clone();
        forged_commitment.proof.vk_commitment = Some([0x25; 32]);
        let err = forged_commitment
            .validate()
            .expect_err("forged vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut forged_hash = proof.proof.envelope_hash.expect("matching hash");
        forged_hash[0] ^= 0x01;
        proof.proof.envelope_hash = Some(forged_hash);
        let err = proof
            .validate()
            .expect_err("forged envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField { field: "proof", .. }
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_bootstrap_key_proof_validate_rejects_open_verify_envelope_drift() {
        let sample = sample_fhe_bootstrap_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut malformed = sample.clone();
        malformed.proof.proof.bytes = vec![0xB5];
        malformed.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&malformed.proof.proof.bytes)));
        let err = malformed
            .validate()
            .expect_err("malformed OpenVerify bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_backend = sample.clone();
        let mut wrong_backend_envelope = envelope.clone();
        wrong_backend_envelope.backend = BackendTag::Halo2IpaPasta;
        replace_fhe_bootstrap_key_open_verify_envelope(&mut wrong_backend, &wrong_backend_envelope);
        let err = wrong_backend
            .validate()
            .expect_err("OpenVerify backend drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_circuit = sample.clone();
        let mut wrong_circuit_envelope = envelope.clone();
        wrong_circuit_envelope.circuit_id = "soracloud_fhe_bootstrap_key_proof_v2".to_string();
        replace_fhe_bootstrap_key_open_verify_envelope(&mut wrong_circuit, &wrong_circuit_envelope);
        let err = wrong_circuit
            .validate()
            .expect_err("OpenVerify circuit id drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string()
                .contains("OpenVerifyEnvelope circuit id must be canonical v1"),
            "unexpected error: {err}"
        );

        let mut wrong_wrapper_version = sample.clone();
        let mut wrong_wrapper_version_envelope = envelope.clone();
        let mut version_drift = open_proof.clone();
        version_drift.version = 2;
        wrong_wrapper_version_envelope.proof_bytes =
            norito::to_bytes(&version_drift).expect("encode version-drifted STARK wrapper");
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut wrong_wrapper_version,
            &wrong_wrapper_version_envelope,
        );
        let err = wrong_wrapper_version
            .validate()
            .expect_err("STARK wrapper version drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_statement = sample.clone();
        let mut wrong_statement_envelope = envelope.clone();
        let mut statement_drift = open_proof.clone();
        statement_drift.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(sample_hash(99))]];
        wrong_statement_envelope.proof_bytes =
            norito::to_bytes(&statement_drift).expect("encode statement-drifted STARK wrapper");
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut wrong_statement,
            &wrong_statement_envelope,
        );
        let err = wrong_statement
            .validate()
            .expect_err("STARK wrapper statement drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_schema = sample.clone();
        let mut wrong_schema_envelope = envelope.clone();
        wrong_schema_envelope.public_inputs =
            b"soracloud:fhe-bootstrap-key:public-inputs:v2".to_vec();
        replace_fhe_bootstrap_key_open_verify_envelope(&mut wrong_schema, &wrong_schema_envelope);
        let err = wrong_schema
            .validate()
            .expect_err("OpenVerify public-input schema drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut empty_native = sample.clone();
        let mut empty_native_envelope = envelope.clone();
        let mut empty_native_open = open_proof.clone();
        empty_native_open.envelope_bytes.clear();
        empty_native_envelope.proof_bytes =
            norito::to_bytes(&empty_native_open).expect("encode empty-native STARK wrapper");
        replace_fhe_bootstrap_key_open_verify_envelope(&mut empty_native, &empty_native_envelope);
        let err = empty_native
            .validate()
            .expect_err("empty native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut all_zero_native = sample;
        let mut all_zero_native_envelope = envelope;
        let mut all_zero_native_open = open_proof;
        all_zero_native_open.envelope_bytes = vec![0; 32];
        all_zero_native_envelope.proof_bytes =
            norito::to_bytes(&all_zero_native_open).expect("encode all-zero STARK wrapper");
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut all_zero_native,
            &all_zero_native_envelope,
        );
        let err = all_zero_native
            .validate()
            .expect_err("all-zero native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all-zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_bootstrap_key_proof_validate_rejects_public_input_shape_replay() {
        let sample = sample_fhe_bootstrap_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");
        let statement = <[u8; Hash::LENGTH]>::from(sample.statement_hash);
        let other_statement = <[u8; Hash::LENGTH]>::from(sample_hash(21));

        let assert_rejected = |label: &str, public_inputs: Vec<Vec<[u8; Hash::LENGTH]>>| {
            let mut proof = sample.clone();
            let mut replay_envelope = envelope.clone();
            let mut replay_open = open_proof.clone();
            replay_open.public_inputs = public_inputs;
            replay_envelope.proof_bytes =
                norito::to_bytes(&replay_open).expect("encode replay-shaped STARK wrapper");
            replace_fhe_bootstrap_key_open_verify_envelope(&mut proof, &replay_envelope);
            let err = proof.validate().expect_err(label);
            assert!(matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ));
        };

        assert_rejected(
            "extra STARK public-input row must be rejected",
            vec![vec![statement], vec![other_statement]],
        );
        assert_rejected(
            "extra STARK public-input column must be rejected",
            vec![vec![statement, other_statement]],
        );
        assert_rejected(
            "duplicate STARK public-input statement must be rejected",
            vec![vec![statement], vec![statement]],
        );
    }

    #[test]
    fn fhe_bootstrap_key_proof_open_verify_bounds_match_published_caps() {
        let bounds = soracloud_fhe_bootstrap_key_proof_open_verify_bounds();
        assert_eq!(
            bounds.max_circuit_id_bytes,
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1.len()
        );
        assert_eq!(
            bounds.max_public_input_bytes,
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len()
        );
        assert_eq!(
            bounds.max_proof_bytes,
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES
        );
        assert_eq!(bounds.max_aux_bytes, 0);
        assert!(!bounds.allow_aux);
        assert!(bounds.require_nonzero_vk_hash);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_bootstrap_key_proof_validate_rejects_oversized_proof_payloads() {
        let sample = sample_fhe_bootstrap_key_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut oversized_outer = sample.clone();
        oversized_outer.proof.proof.bytes =
            vec![0xB5; SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES + 1];
        oversized_outer.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(
            &oversized_outer.proof.proof.bytes,
        )));
        let err = oversized_outer
            .validate()
            .expect_err("oversized OpenVerify envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("OpenVerifyEnvelope length"),
            "unexpected error: {err}"
        );

        let mut oversized_circuit = sample.clone();
        let mut oversized_circuit_envelope = envelope.clone();
        oversized_circuit_envelope.circuit_id =
            format!("{SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1}_x");
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut oversized_circuit,
            &oversized_circuit_envelope,
        );
        let err = oversized_circuit
            .validate()
            .expect_err("oversized OpenVerify circuit id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("circuit id length"),
            "unexpected error: {err}"
        );

        let mut oversized_schema = sample.clone();
        let mut oversized_schema_envelope = envelope.clone();
        oversized_schema_envelope.public_inputs =
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec();
        oversized_schema_envelope.public_inputs.push(b'x');
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut oversized_schema,
            &oversized_schema_envelope,
        );
        let err = oversized_schema
            .validate()
            .expect_err("oversized OpenVerify public-input schema must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("public inputs length"),
            "unexpected error: {err}"
        );

        let mut oversized_wrapper = sample.clone();
        let mut oversized_wrapper_envelope = envelope.clone();
        oversized_wrapper_envelope.proof_bytes =
            vec![0xB5; SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES + 1];
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut oversized_wrapper,
            &oversized_wrapper_envelope,
        );
        let err = oversized_wrapper
            .validate()
            .expect_err("oversized STARK wrapper bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("proof bytes length"),
            "unexpected error: {err}"
        );

        let mut oversized_native = sample;
        let mut oversized_native_envelope = envelope;
        let mut oversized_native_open = open_proof;
        oversized_native_open.envelope_bytes =
            vec![0xB5; SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 1];
        oversized_native_envelope.proof_bytes =
            norito::to_bytes(&oversized_native_open).expect("encode oversized STARK wrapper");
        replace_fhe_bootstrap_key_open_verify_envelope(
            &mut oversized_native,
            &oversized_native_envelope,
        );
        let err = oversized_native
            .validate()
            .expect_err("oversized native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_bootstrap_key_proof_validate_requires_canonical_vk_ref_name() {
        let mut proof = sample_fhe_bootstrap_key_proof();
        proof.proof.vk_ref.name = "soracloud_fhe_bootstrap_key_alias_v1".to_string();

        let err = proof
            .validate()
            .expect_err("non-canonical bootstrap-key verifier id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));
    }

    #[test]
    fn fhe_bootstrap_key_proof_validate_rejects_attachment_metadata_drift() {
        let mut proof_backend_mismatch = sample_fhe_bootstrap_key_proof();
        proof_backend_mismatch.proof.proof.backend = "stark/fri/other".into();
        let err = proof_backend_mismatch
            .validate()
            .expect_err("mismatched proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut vk_backend_mismatch = sample_fhe_bootstrap_key_proof();
        vk_backend_mismatch.proof.vk_ref.backend = "stark/fri/other".into();
        let err = vk_backend_mismatch
            .validate()
            .expect_err("mismatched verifier-key backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.backend",
                ..
            }
        ));

        let mut wrong_stark_profile = sample_fhe_bootstrap_key_proof();
        wrong_stark_profile.proof.backend = "stark/fri/poseidon2-goldilocks".into();
        wrong_stark_profile.proof.proof.backend = wrong_stark_profile.proof.backend.clone();
        wrong_stark_profile.proof.vk_ref.backend = wrong_stark_profile.proof.backend.clone();
        let err = wrong_stark_profile
            .validate()
            .expect_err("alternate STARK/FRI profile must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
        assert!(
            err.to_string().contains("canonical BFV STARK/FRI backend"),
            "unexpected error: {err}"
        );

        let mut unsupported = sample_fhe_bootstrap_key_proof();
        unsupported.proof.backend = "stark/fri/debug-proof".into();
        unsupported.proof.proof.backend = unsupported.proof.backend.clone();
        unsupported.proof.vk_ref.backend = unsupported.proof.backend.clone();
        let err = unsupported
            .validate()
            .expect_err("unsupported bootstrap-key proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));

        let mut empty_backend = sample_fhe_bootstrap_key_proof();
        empty_backend.proof.backend = " \t ".into();
        let err = empty_backend
            .validate()
            .expect_err("empty bootstrap-key proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::EmptyField {
                field: "proof.backend",
                ..
            }
        ));
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_validate_accepts_canonical_envelope() {
        let proof = sample_fhe_full_bootstrap_material_proof();
        proof
            .validate()
            .expect("canonical full-bootstrap material proof envelope must validate");
        assert_eq!(
            soracloud_fhe_full_bootstrap_material_proof_public_inputs_schema_hash_v1(),
            <[u8; 32]>::from(Hash::new(
                SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ))
        );
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_validate_requires_vk_commitment_and_matching_envelope_hash()
     {
        let mut proof = sample_fhe_full_bootstrap_material_proof();
        proof.proof.vk_commitment = None;

        let err = proof
            .validate()
            .expect_err("missing vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        proof.proof.vk_commitment = Some([0x62; 32]);
        proof.proof.envelope_hash = None;
        let err = proof
            .validate()
            .expect_err("missing envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));

        proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
        proof
            .validate()
            .expect("matching envelope hash must be accepted");

        let mut forged_commitment = proof.clone();
        forged_commitment.proof.vk_commitment = Some([0x26; 32]);
        let err = forged_commitment
            .validate()
            .expect_err("forged vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut forged_hash = proof.proof.envelope_hash.expect("matching hash");
        forged_hash[0] ^= 0x01;
        proof.proof.envelope_hash = Some(forged_hash);
        let err = proof
            .validate()
            .expect_err("forged envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField { field: "proof", .. }
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_full_bootstrap_material_proof_validate_rejects_open_verify_envelope_drift() {
        let sample = sample_fhe_full_bootstrap_material_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut malformed = sample.clone();
        malformed.proof.proof.bytes = vec![0xC5];
        malformed.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&malformed.proof.proof.bytes)));
        let err = malformed
            .validate()
            .expect_err("malformed OpenVerify bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_backend = sample.clone();
        let mut wrong_backend_envelope = envelope.clone();
        wrong_backend_envelope.backend = BackendTag::Halo2IpaPasta;
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut wrong_backend,
            &wrong_backend_envelope,
        );
        let err = wrong_backend
            .validate()
            .expect_err("OpenVerify backend drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_circuit = sample.clone();
        let mut wrong_circuit_envelope = envelope.clone();
        wrong_circuit_envelope.circuit_id = "soracloud_fhe_full_bootstrap_material_v2".to_string();
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut wrong_circuit,
            &wrong_circuit_envelope,
        );
        let err = wrong_circuit
            .validate()
            .expect_err("OpenVerify circuit id drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string()
                .contains("OpenVerifyEnvelope circuit id must be canonical v1"),
            "unexpected error: {err}"
        );

        let mut wrong_wrapper_version = sample.clone();
        let mut wrong_wrapper_version_envelope = envelope.clone();
        let mut version_drift = open_proof.clone();
        version_drift.version = 2;
        wrong_wrapper_version_envelope.proof_bytes =
            norito::to_bytes(&version_drift).expect("encode version-drifted STARK wrapper");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut wrong_wrapper_version,
            &wrong_wrapper_version_envelope,
        );
        let err = wrong_wrapper_version
            .validate()
            .expect_err("STARK wrapper version drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_statement = sample.clone();
        let mut wrong_statement_envelope = envelope.clone();
        let mut statement_drift = open_proof.clone();
        statement_drift.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(sample_hash(99))]];
        wrong_statement_envelope.proof_bytes =
            norito::to_bytes(&statement_drift).expect("encode statement-drifted STARK wrapper");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut wrong_statement,
            &wrong_statement_envelope,
        );
        let err = wrong_statement
            .validate()
            .expect_err("STARK wrapper statement drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_schema = sample.clone();
        let mut wrong_schema_envelope = envelope.clone();
        wrong_schema_envelope.public_inputs =
            b"soracloud:fhe-full-bootstrap-material:public-inputs:v2".to_vec();
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut wrong_schema,
            &wrong_schema_envelope,
        );
        let err = wrong_schema
            .validate()
            .expect_err("OpenVerify public-input schema drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut empty_native = sample.clone();
        let mut empty_native_envelope = envelope.clone();
        let mut empty_native_open = open_proof.clone();
        empty_native_open.envelope_bytes.clear();
        empty_native_envelope.proof_bytes =
            norito::to_bytes(&empty_native_open).expect("encode empty-native STARK wrapper");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut empty_native,
            &empty_native_envelope,
        );
        let err = empty_native
            .validate()
            .expect_err("empty native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut all_zero_native = sample;
        let mut all_zero_native_envelope = envelope;
        let mut all_zero_native_open = open_proof;
        all_zero_native_open.envelope_bytes = vec![0; 32];
        all_zero_native_envelope.proof_bytes =
            norito::to_bytes(&all_zero_native_open).expect("encode all-zero STARK wrapper");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut all_zero_native,
            &all_zero_native_envelope,
        );
        let err = all_zero_native
            .validate()
            .expect_err("all-zero native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all-zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_validate_rejects_public_input_shape_replay() {
        let sample = sample_fhe_full_bootstrap_material_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");
        let statement = <[u8; Hash::LENGTH]>::from(sample.statement_hash);
        let other_statement = <[u8; Hash::LENGTH]>::from(sample_hash(21));

        let assert_rejected = |label: &str, public_inputs: Vec<Vec<[u8; Hash::LENGTH]>>| {
            let mut proof = sample.clone();
            let mut replay_envelope = envelope.clone();
            let mut replay_open = open_proof.clone();
            replay_open.public_inputs = public_inputs;
            replay_envelope.proof_bytes =
                norito::to_bytes(&replay_open).expect("encode replay-shaped STARK wrapper");
            replace_fhe_full_bootstrap_material_open_verify_envelope(&mut proof, &replay_envelope);
            let err = proof.validate().expect_err(label);
            assert!(matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ));
        };

        assert_rejected(
            "extra STARK public-input row must be rejected",
            vec![vec![statement], vec![other_statement]],
        );
        assert_rejected(
            "extra STARK public-input column must be rejected",
            vec![vec![statement, other_statement]],
        );
        assert_rejected(
            "duplicate STARK public-input statement must be rejected",
            vec![vec![statement], vec![statement]],
        );
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_open_verify_bounds_match_published_caps() {
        let bounds = soracloud_fhe_full_bootstrap_material_proof_open_verify_bounds();
        assert_eq!(
            bounds.max_circuit_id_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_CIRCUIT_ID_V1.len()
        );
        assert_eq!(
            bounds.max_public_input_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len()
        );
        assert_eq!(
            bounds.max_proof_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_MAX_STARK_WRAPPER_BYTES
        );
        assert_eq!(bounds.max_aux_bytes, 0);
        assert!(!bounds.allow_aux);
        assert!(bounds.require_nonzero_vk_hash);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_full_bootstrap_material_proof_validate_rejects_oversized_proof_payloads() {
        let sample = sample_fhe_full_bootstrap_material_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut oversized_outer = sample.clone();
        oversized_outer.proof.proof.bytes =
            vec![0xC5; SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_MAX_OPEN_VERIFY_BYTES + 1];
        oversized_outer.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(
            &oversized_outer.proof.proof.bytes,
        )));
        let err = oversized_outer
            .validate()
            .expect_err("oversized OpenVerify envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("OpenVerifyEnvelope length"),
            "unexpected error: {err}"
        );

        let mut oversized_circuit = sample.clone();
        let mut oversized_circuit_envelope = envelope.clone();
        oversized_circuit_envelope.circuit_id =
            format!("{SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_CIRCUIT_ID_V1}_x");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut oversized_circuit,
            &oversized_circuit_envelope,
        );
        let err = oversized_circuit
            .validate()
            .expect_err("oversized OpenVerify circuit id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("circuit id length"),
            "unexpected error: {err}"
        );

        let mut oversized_schema = sample.clone();
        let mut oversized_schema_envelope = envelope.clone();
        oversized_schema_envelope.public_inputs =
            SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec();
        oversized_schema_envelope.public_inputs.push(b'x');
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut oversized_schema,
            &oversized_schema_envelope,
        );
        let err = oversized_schema
            .validate()
            .expect_err("oversized OpenVerify public-input schema must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("public inputs length"),
            "unexpected error: {err}"
        );

        let mut oversized_wrapper = sample.clone();
        let mut oversized_wrapper_envelope = envelope.clone();
        oversized_wrapper_envelope.proof_bytes =
            vec![0xC5; SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_MAX_STARK_WRAPPER_BYTES + 1];
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut oversized_wrapper,
            &oversized_wrapper_envelope,
        );
        let err = oversized_wrapper
            .validate()
            .expect_err("oversized STARK wrapper bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("proof bytes length"),
            "unexpected error: {err}"
        );

        let mut oversized_native = sample;
        let mut oversized_native_envelope = envelope;
        let mut oversized_native_open = open_proof;
        oversized_native_open.envelope_bytes =
            vec![0xC5; SORACLOUD_FHE_FULL_BOOTSTRAP_MATERIAL_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 1];
        oversized_native_envelope.proof_bytes =
            norito::to_bytes(&oversized_native_open).expect("encode oversized STARK wrapper");
        replace_fhe_full_bootstrap_material_open_verify_envelope(
            &mut oversized_native,
            &oversized_native_envelope,
        );
        let err = oversized_native
            .validate()
            .expect_err("oversized native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_validate_requires_canonical_vk_ref_name() {
        let mut proof = sample_fhe_full_bootstrap_material_proof();
        proof.proof.vk_ref.name = "soracloud_fhe_full_bootstrap_material_alias_v1".to_string();

        let err = proof
            .validate()
            .expect_err("non-canonical full-bootstrap material verifier id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));
    }

    #[test]
    fn fhe_full_bootstrap_material_proof_validate_rejects_attachment_metadata_drift() {
        let mut proof_backend_mismatch = sample_fhe_full_bootstrap_material_proof();
        proof_backend_mismatch.proof.proof.backend = "stark/fri/other".into();
        let err = proof_backend_mismatch
            .validate()
            .expect_err("mismatched proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut vk_backend_mismatch = sample_fhe_full_bootstrap_material_proof();
        vk_backend_mismatch.proof.vk_ref.backend = "stark/fri/other".into();
        let err = vk_backend_mismatch
            .validate()
            .expect_err("mismatched verifier-key backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.backend",
                ..
            }
        ));

        let mut wrong_stark_profile = sample_fhe_full_bootstrap_material_proof();
        wrong_stark_profile.proof.backend = "stark/fri/poseidon2-goldilocks".into();
        wrong_stark_profile.proof.proof.backend = wrong_stark_profile.proof.backend.clone();
        wrong_stark_profile.proof.vk_ref.backend = wrong_stark_profile.proof.backend.clone();
        let err = wrong_stark_profile
            .validate()
            .expect_err("non-canonical full-bootstrap material STARK profile must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
        assert!(
            err.to_string().contains("canonical BFV full-bootstrap"),
            "unexpected error: {err}"
        );

        let mut unsupported = sample_fhe_full_bootstrap_material_proof();
        unsupported.proof.backend = "stark/fri/debug-proof".into();
        unsupported.proof.proof.backend = unsupported.proof.backend.clone();
        unsupported.proof.vk_ref.backend = unsupported.proof.backend.clone();
        let err = unsupported
            .validate()
            .expect_err("unsupported full-bootstrap material proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));

        let mut empty_backend = sample_fhe_full_bootstrap_material_proof();
        empty_backend.proof.backend = " \t ".into();
        let err = empty_backend
            .validate()
            .expect_err("empty full-bootstrap material proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::EmptyField {
                field: "proof.backend",
                ..
            }
        ));
    }

    #[test]
    fn fhe_full_bootstrap_execution_proof_validate_accepts_canonical_envelope() {
        let proof = sample_fhe_full_bootstrap_execution_proof();
        proof
            .validate()
            .expect("canonical full-bootstrap execution proof validates");

        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&proof.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        assert_eq!(
            envelope.circuit_id,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            envelope.public_inputs,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1
        );
        assert_eq!(
            soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1(),
            <[u8; 32]>::from(Hash::new(
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1
            ))
        );
    }

    #[test]
    fn fhe_full_bootstrap_execution_proof_validate_requires_vk_commitment_and_matching_envelope_hash()
     {
        let mut proof = sample_fhe_full_bootstrap_execution_proof();
        proof.proof.vk_commitment = None;

        let err = proof
            .validate()
            .expect_err("missing vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        proof.proof.vk_commitment = Some([0x63; 32]);
        proof.proof.envelope_hash = None;
        let err = proof
            .validate()
            .expect_err("missing envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.envelope_hash",
                ..
            }
        ));

        proof.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(&proof.proof.proof.bytes)));
        proof
            .validate()
            .expect("matching envelope hash must be accepted");

        let mut forged_commitment = proof.clone();
        forged_commitment.proof.vk_commitment = Some([0x27; 32]);
        let err = forged_commitment
            .validate()
            .expect_err("forged vk_commitment must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut forged_hash = proof.proof.envelope_hash.expect("matching hash");
        forged_hash[0] ^= 0x01;
        proof.proof.envelope_hash = Some(forged_hash);
        let err = proof
            .validate()
            .expect_err("forged envelope hash must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField { field: "proof", .. }
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_full_bootstrap_execution_proof_validate_rejects_open_verify_envelope_drift() {
        let sample = sample_fhe_full_bootstrap_execution_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut malformed = sample.clone();
        malformed.proof.proof.bytes = vec![0xD5];
        malformed.proof.envelope_hash =
            Some(<[u8; 32]>::from(Hash::new(&malformed.proof.proof.bytes)));
        let err = malformed
            .validate()
            .expect_err("malformed OpenVerify bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_backend = sample.clone();
        let mut wrong_backend_envelope = envelope.clone();
        wrong_backend_envelope.backend = BackendTag::Halo2IpaPasta;
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_backend,
            &wrong_backend_envelope,
        );
        let err = wrong_backend
            .validate()
            .expect_err("OpenVerify backend drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_circuit = sample.clone();
        let mut wrong_circuit_envelope = envelope.clone();
        wrong_circuit_envelope.circuit_id = "soracloud_fhe_full_bootstrap_execution_v2".to_string();
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_circuit,
            &wrong_circuit_envelope,
        );
        let err = wrong_circuit
            .validate()
            .expect_err("OpenVerify circuit id drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_schema = sample.clone();
        let mut wrong_schema_envelope = envelope.clone();
        wrong_schema_envelope.public_inputs =
            b"soracloud:fhe-full-bootstrap-execution:public-inputs:v2".to_vec();
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_schema,
            &wrong_schema_envelope,
        );
        let err = wrong_schema
            .validate()
            .expect_err("OpenVerify public-input schema drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_vk_hash = sample.clone();
        let mut wrong_vk_hash_envelope = envelope.clone();
        wrong_vk_hash_envelope.vk_hash = [0xA4; 32];
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_vk_hash,
            &wrong_vk_hash_envelope,
        );
        let err = wrong_vk_hash
            .validate()
            .expect_err("OpenVerify verifier-key commitment drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_commitment",
                ..
            }
        ));

        let mut wrong_wrapper_version = sample.clone();
        let mut wrong_wrapper_version_envelope = envelope.clone();
        let mut version_drift = open_proof.clone();
        version_drift.version = 2;
        wrong_wrapper_version_envelope.proof_bytes =
            norito::to_bytes(&version_drift).expect("encode version-drifted STARK wrapper");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_wrapper_version,
            &wrong_wrapper_version_envelope,
        );
        let err = wrong_wrapper_version
            .validate()
            .expect_err("STARK wrapper version drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut wrong_statement = sample.clone();
        let mut wrong_statement_envelope = envelope.clone();
        let mut statement_drift = open_proof.clone();
        statement_drift.public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(sample_hash(99))]];
        wrong_statement_envelope.proof_bytes =
            norito::to_bytes(&statement_drift).expect("encode statement-drifted STARK wrapper");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut wrong_statement,
            &wrong_statement_envelope,
        );
        let err = wrong_statement
            .validate()
            .expect_err("STARK wrapper statement drift must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut empty_native = sample.clone();
        let mut empty_native_envelope = envelope.clone();
        let mut empty_native_open = open_proof.clone();
        empty_native_open.envelope_bytes.clear();
        empty_native_envelope.proof_bytes =
            norito::to_bytes(&empty_native_open).expect("encode empty-native STARK wrapper");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut empty_native,
            &empty_native_envelope,
        );
        let err = empty_native
            .validate()
            .expect_err("empty native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut all_zero_native = sample;
        let mut all_zero_native_envelope = envelope;
        let mut all_zero_native_open = open_proof;
        all_zero_native_open.envelope_bytes = vec![0; 32];
        all_zero_native_envelope.proof_bytes =
            norito::to_bytes(&all_zero_native_open).expect("encode all-zero STARK wrapper");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut all_zero_native,
            &all_zero_native_envelope,
        );
        let err = all_zero_native
            .validate()
            .expect_err("all-zero native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("all-zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_full_bootstrap_execution_proof_validate_rejects_public_input_shape_replay() {
        let sample = sample_fhe_full_bootstrap_execution_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");
        let statement = <[u8; Hash::LENGTH]>::from(sample.statement_hash);
        let other_statement = <[u8; Hash::LENGTH]>::from(sample_hash(21));

        let assert_rejected = |label: &str, public_inputs: Vec<Vec<[u8; Hash::LENGTH]>>| {
            let mut proof = sample.clone();
            let mut replay_envelope = envelope.clone();
            let mut replay_open = open_proof.clone();
            replay_open.public_inputs = public_inputs;
            replay_envelope.proof_bytes =
                norito::to_bytes(&replay_open).expect("encode replay-shaped STARK wrapper");
            replace_fhe_full_bootstrap_execution_open_verify_envelope(&mut proof, &replay_envelope);
            let err = proof.validate().expect_err(label);
            assert!(matches!(
                err,
                SoracloudManifestError::InvalidField {
                    field: "proof.proof.bytes",
                    ..
                }
            ));
        };

        assert_rejected(
            "extra STARK public-input row must be rejected",
            vec![vec![statement], vec![other_statement]],
        );
        assert_rejected(
            "extra STARK public-input column must be rejected",
            vec![vec![statement, other_statement]],
        );
        assert_rejected(
            "duplicate STARK public-input statement must be rejected",
            vec![vec![statement], vec![statement]],
        );
    }

    #[test]
    fn fhe_full_bootstrap_execution_proof_open_verify_bounds_match_published_caps() {
        let bounds = soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds();
        assert_eq!(
            bounds.max_circuit_id_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1.len()
        );
        assert_eq!(
            bounds.max_public_input_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len()
        );
        assert_eq!(
            bounds.max_proof_bytes,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES
        );
        assert_eq!(bounds.max_aux_bytes, 0);
        assert!(!bounds.allow_aux);
        assert!(bounds.require_nonzero_vk_hash);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_full_bootstrap_execution_proof_validate_rejects_oversized_proof_payloads() {
        let sample = sample_fhe_full_bootstrap_execution_proof();
        let envelope = norito::decode_from_bytes::<OpenVerifyEnvelope>(&sample.proof.proof.bytes)
            .expect("decode sample OpenVerifyEnvelope");
        let open_proof = norito::decode_from_bytes::<StarkFriOpenProofV1>(&envelope.proof_bytes)
            .expect("decode sample STARK public-input wrapper");

        let mut oversized_outer = sample.clone();
        oversized_outer.proof.proof.bytes =
            vec![0xD5; SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES + 1];
        oversized_outer.proof.envelope_hash = Some(<[u8; 32]>::from(Hash::new(
            &oversized_outer.proof.proof.bytes,
        )));
        let err = oversized_outer
            .validate()
            .expect_err("oversized OpenVerify envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));

        let mut oversized_circuit = sample.clone();
        let mut oversized_circuit_envelope = envelope.clone();
        oversized_circuit_envelope.circuit_id =
            format!("{SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1}_x");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut oversized_circuit,
            &oversized_circuit_envelope,
        );
        let err = oversized_circuit
            .validate()
            .expect_err("oversized OpenVerify circuit id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("circuit id length"),
            "unexpected error: {err}"
        );

        let mut oversized_schema = sample.clone();
        let mut oversized_schema_envelope = envelope.clone();
        oversized_schema_envelope.public_inputs =
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1.to_vec();
        oversized_schema_envelope.public_inputs.push(b'x');
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut oversized_schema,
            &oversized_schema_envelope,
        );
        let err = oversized_schema
            .validate()
            .expect_err("oversized OpenVerify public-input schema must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("public inputs length"),
            "unexpected error: {err}"
        );

        let mut oversized_wrapper = sample.clone();
        let mut oversized_wrapper_envelope = envelope.clone();
        oversized_wrapper_envelope.proof_bytes =
            vec![0xD5; SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES + 1];
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut oversized_wrapper,
            &oversized_wrapper_envelope,
        );
        let err = oversized_wrapper
            .validate()
            .expect_err("oversized STARK wrapper bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("proof bytes length"),
            "unexpected error: {err}"
        );

        let mut oversized_native = sample;
        let mut oversized_native_envelope = envelope;
        let mut oversized_native_open = open_proof;
        oversized_native_open.envelope_bytes =
            vec![0xD5; SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES + 1];
        oversized_native_envelope.proof_bytes =
            norito::to_bytes(&oversized_native_open).expect("encode oversized STARK wrapper");
        replace_fhe_full_bootstrap_execution_open_verify_envelope(
            &mut oversized_native,
            &oversized_native_envelope,
        );
        let err = oversized_native
            .validate()
            .expect_err("oversized native STARK envelope bytes must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.bytes",
                ..
            }
        ));
        assert!(
            err.to_string().contains("native envelope bytes length"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn fhe_full_bootstrap_execution_proof_validate_rejects_attachment_metadata_drift() {
        let mut proof_backend_mismatch = sample_fhe_full_bootstrap_execution_proof();
        proof_backend_mismatch.proof.proof.backend = "stark/fri/other".into();
        let err = proof_backend_mismatch
            .validate()
            .expect_err("mismatched proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.proof.backend",
                ..
            }
        ));

        let mut vk_backend_mismatch = sample_fhe_full_bootstrap_execution_proof();
        vk_backend_mismatch.proof.vk_ref.backend = "stark/fri/other".into();
        let err = vk_backend_mismatch
            .validate()
            .expect_err("mismatched verifier-key backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.backend",
                ..
            }
        ));

        let mut wrong_vk_ref = sample_fhe_full_bootstrap_execution_proof();
        wrong_vk_ref.proof.vk_ref.name =
            "soracloud_fhe_full_bootstrap_execution_alias_v1".to_string();
        let err = wrong_vk_ref
            .validate()
            .expect_err("non-canonical full-bootstrap execution verifier id must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.vk_ref.name",
                ..
            }
        ));

        let mut wrong_stark_profile = sample_fhe_full_bootstrap_execution_proof();
        wrong_stark_profile.proof.backend = "stark/fri/poseidon2-goldilocks".into();
        wrong_stark_profile.proof.proof.backend = wrong_stark_profile.proof.backend.clone();
        wrong_stark_profile.proof.vk_ref.backend = wrong_stark_profile.proof.backend.clone();
        let err = wrong_stark_profile
            .validate()
            .expect_err("non-canonical full-bootstrap execution STARK profile must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
        assert!(
            err.to_string().contains("canonical BFV full-bootstrap"),
            "unexpected error: {err}"
        );

        let mut unsupported = sample_fhe_full_bootstrap_execution_proof();
        unsupported.proof.backend = "stark/fri/debug-proof".into();
        unsupported.proof.proof.backend = unsupported.proof.backend.clone();
        unsupported.proof.vk_ref.backend = unsupported.proof.backend.clone();
        let err = unsupported
            .validate()
            .expect_err("unsupported full-bootstrap execution proof backend must be rejected");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "proof.backend",
                ..
            }
        ));
    }

    #[test]
    fn rollout_provenance_payload_encodes_canonical_tuple() {
        let governance_tx_hash = sample_hash(1);
        let encoded = encode_rollout_provenance_payload(
            "web_portal",
            "rollout-42",
            true,
            Some(50),
            governance_tx_hash,
        )
        .expect("encode payload");
        let expected = norito::to_bytes(&(
            "web_portal",
            "rollout-42",
            true,
            Some(50u8),
            governance_tx_hash,
        ))
        .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_deploy_provenance_payload_encodes_canonical_tuple() {
        let manifest = sample_agent_apartment_manifest();
        let encoded =
            encode_agent_deploy_provenance_payload(manifest.clone(), 10_000, Some(500_000))
                .expect("encode payload");
        let expected =
            norito::to_bytes(&(manifest, 10_000u64, Some(500_000u64))).expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_lease_renew_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_lease_renew_provenance_payload("agent-apartment", 20_000)
            .expect("encode payload");
        let expected = norito::to_bytes(&("agent-apartment", 20_000u64)).expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_restart_provenance_payload_encodes_canonical_tuple() {
        let encoded =
            encode_agent_restart_provenance_payload("agent-apartment", "apply patched policy")
                .expect("encode payload");
        let expected =
            norito::to_bytes(&("agent-apartment", "apply patched policy")).expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_policy_revoke_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_policy_revoke_provenance_payload(
            "agent-apartment",
            "wallet.spend",
            Some("limit exceeded"),
        )
        .expect("encode payload");
        let expected =
            norito::to_bytes(&("agent-apartment", "wallet.spend", Some("limit exceeded")))
                .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_spend_provenance_payload_encodes_canonical_tuple() {
        let amount = xor_quantity_from_nanos(1_250_000);
        let encoded = encode_agent_wallet_spend_provenance_payload(
            "agent-apartment",
            "61CtjvNd9T3THAR65GsMVHr82Bjc",
            &amount,
        )
        .expect("encode payload");
        let expected =
            norito::to_bytes(&("agent-apartment", "61CtjvNd9T3THAR65GsMVHr82Bjc", amount))
                .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_wallet_approve_provenance_payload_encodes_canonical_tuple() {
        let encoded =
            encode_agent_wallet_approve_provenance_payload("agent-apartment", "spend-req-9")
                .expect("encode payload");
        let expected = norito::to_bytes(&("agent-apartment", "spend-req-9")).expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_send_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_message_send_provenance_payload(
            "apartment-a",
            "apartment-b",
            "ops",
            "{\"ping\":true}",
        )
        .expect("encode payload");
        let expected = norito::to_bytes(&("apartment-a", "apartment-b", "ops", "{\"ping\":true}"))
            .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_message_ack_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_message_ack_provenance_payload("agent-apartment", "msg-1")
            .expect("encode payload");
        let expected = norito::to_bytes(&("agent-apartment", "msg-1")).expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_artifact_allow_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_artifact_allow_provenance_payload(
            "agent-apartment",
            "QmArtifactHash",
            Some("QmProvenanceHash"),
        )
        .expect("encode payload");
        let expected = norito::to_bytes(&(
            "agent-apartment",
            "QmArtifactHash",
            Some("QmProvenanceHash"),
        ))
        .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn agent_autonomy_run_provenance_payload_encodes_canonical_tuple() {
        let encoded = encode_agent_autonomy_run_provenance_payload(
            "agent-apartment",
            "QmArtifactHash",
            Some("QmProvenanceHash"),
            42_000,
            "nightly-retrain",
            Some("{\"inputs\":[\"alpha\",\"beta\"]}"),
        )
        .expect("encode payload");
        let expected = norito::to_bytes(&(
            "agent-apartment",
            "QmArtifactHash",
            Some("QmProvenanceHash"),
            42_000u64,
            "nightly-retrain",
            canonical_agent_workflow_input_json_for_payload(Some(
                "{\"inputs\":[\"alpha\",\"beta\"]}",
            ))
            .as_deref(),
        ))
        .expect("encode tuple");
        assert_eq!(encoded, expected);
    }

