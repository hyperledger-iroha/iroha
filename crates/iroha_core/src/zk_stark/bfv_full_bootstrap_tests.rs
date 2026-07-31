// BFV full-bootstrap fixtures and STARK rejection profiles.
//
// Included lexically by `zk_stark::tests` through `zk_stark/tests.rs`.

    fn sample_bfv_full_bootstrap_linear_transform_artifact_payload(
        params: &iroha_crypto::BfvParameters,
        role: iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1,
    ) -> Vec<u8> {
        let transform = iroha_crypto::BfvFullBootstrapLinearTransformV1 {
            input_slot_count: params.polynomial_degree,
            output_slot_count: params.polynomial_degree,
            diagonals: vec![iroha_crypto::BfvFullBootstrapLinearTransformDiagonalV1 {
                rotation_steps: 0,
                plaintext: iroha_crypto::encode_packed_plaintext_slots(
                    params,
                    &vec![1; usize::from(params.polynomial_degree)],
                )
                .expect("encode identity packed-slot mask"),
            }],
        };
        iroha_crypto::encode_bfv_full_bootstrap_linear_transform_artifact_v1(
            params, 1, role, &transform,
        )
        .expect("encode full-bootstrap linear transform artifact")
    }

    fn sample_bfv_full_bootstrap_artifacts_for_secret(
        params: &iroha_crypto::BfvParameters,
        secret_key: &iroha_crypto::BfvSecretKey,
    ) -> iroha_crypto::BfvFullBootstrapCircuitArtifactBundleV1 {
        let accumulator = iroha_crypto::BfvFullBootstrapAccumulatorV1 {
            slot_count: params.polynomial_degree,
            test_vector: iroha_crypto::encode_packed_plaintext_slots(
                params,
                &vec![1; usize::from(params.polynomial_degree)],
            )
            .expect("encode full-bootstrap accumulator"),
        };
        let accumulator = iroha_crypto::encode_bfv_full_bootstrap_accumulator_artifact_v1(
            params,
            1,
            &accumulator,
        )
        .expect("encode accumulator artifact");
        let accumulator_digest = iroha_crypto::Hash::new(&accumulator);
        let proof_public_input_schema =
            iroha_crypto::encode_bfv_full_bootstrap_proof_public_input_schema_artifact_v1(
                params,
                1,
                &iroha_crypto::bfv_full_bootstrap_proof_public_input_schema_v1(),
            )
            .expect("encode proof public-input schema artifact");
        let proof_public_input_schema_digest = iroha_crypto::Hash::new(&proof_public_input_schema);
        let arithmetic_air_constraint_system =
            iroha_crypto::encode_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_v1(
                params,
                1,
                &iroha_crypto::bfv_full_bootstrap_arithmetic_air_constraint_system_material_v1(),
            )
            .expect("encode arithmetic AIR artifact");
        let coefficient_to_slot_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
            params,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::CoefficientToSlotKey,
        );
        let slot_to_coefficient_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
            params,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::SlotToCoefficientKey,
        );
        let blind_rotation_key =
            iroha_crypto::bfv_full_bootstrap_blind_rotation_key_for_packed_left_rotation_v1(
                params,
                accumulator_digest,
                1,
            )
            .expect("build blind-rotation key");
        let blind_rotation_key =
            iroha_crypto::encode_bfv_full_bootstrap_blind_rotation_artifact_v1(
                params,
                1,
                &blind_rotation_key,
            )
            .expect("encode blind-rotation artifact");
        let sample_extraction = iroha_crypto::BfvFullBootstrapSampleExtractionV1 {
            source_slot_count: params.polynomial_degree,
            source_ciphertext_component_count: 2,
            extracted_coefficient_index: 0,
            output_ciphertext_component_count: 2,
        };
        let sample_extraction_key =
            iroha_crypto::bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
                params,
                secret_key,
                sample_extraction,
                b"zk-stark-bfv-full-bootstrap-sample-switch",
            )
            .expect("build sample-extraction switch key");
        let sample_extraction_key =
            iroha_crypto::encode_bfv_full_bootstrap_sample_extraction_switch_key_artifact_v1(
                params,
                1,
                &sample_extraction_key,
            )
            .expect("encode sample-extraction switch key artifact");
        let evaluator_artifact_set_digest =
            iroha_crypto::bfv_full_bootstrap_evaluator_artifact_set_digest_v1(
                params,
                1,
                &coefficient_to_slot_key,
                &slot_to_coefficient_key,
                &blind_rotation_key,
                &sample_extraction_key,
                &accumulator,
                &proof_public_input_schema,
                &arithmetic_air_constraint_system,
            )
            .expect("derive evaluator artifact-set digest");
        let prover_key_material =
            iroha_crypto::encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1(
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            )
            .expect("encode native prover material");
        let verifier_key_material =
            iroha_crypto::encode_bfv_full_bootstrap_native_stark_fri_verifier_key_material_v1(
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            )
            .expect("encode native verifier material");
        let (prover_key, verifier_key) =
            iroha_crypto::bfv_full_bootstrap_proof_key_pair_from_key_material_v1(
                params,
                1,
                proof_public_input_schema_digest,
                evaluator_artifact_set_digest,
                &prover_key_material,
                &verifier_key_material,
            )
            .expect("build native proof-key pair");
        let prover_key = iroha_crypto::encode_bfv_full_bootstrap_proof_key_artifact_v1(
            params,
            1,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            &prover_key,
        )
        .expect("encode prover-key artifact");
        let verifier_key = iroha_crypto::encode_bfv_full_bootstrap_proof_key_artifact_v1(
            params,
            1,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
            &verifier_key,
        )
        .expect("encode verifier-key artifact");
        iroha_crypto::BfvFullBootstrapCircuitArtifactBundleV1 {
            coefficient_to_slot_key,
            slot_to_coefficient_key,
            blind_rotation_key,
            sample_extraction_key,
            accumulator,
            proof_public_input_schema,
            arithmetic_air_constraint_system,
            prover_key,
            verifier_key,
        }
    }

    fn bfv_full_bootstrap_stark_test_prover_input_material()
    -> iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1 {
        bfv_full_bootstrap_stark_test_prover_input_material_for_slot(0)
    }

    fn bfv_full_bootstrap_stark_test_release_audit_package_and_digest(
        params: &iroha_crypto::BfvParameters,
        material: &iroha_crypto::BfvFullBootstrapCircuitMaterialV1,
        artifacts: &iroha_crypto::BfvFullBootstrapCircuitArtifactBundleV1,
        reviewer_key_pair: &iroha_crypto::KeyPair,
    ) -> (
        iroha_crypto::BfvFullBootstrapReleaseAuditPackageV1,
        iroha_crypto::Hash,
    ) {
        let (generated_report_bytes, generated_archive_bytes) =
            iroha_crypto::bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifacts_v1(
                params, material, artifacts,
            )
            .expect("sample full-bootstrap release audit generated report/archive bytes");
        let generated_report_body = generated_report_bytes
            .strip_prefix(iroha_crypto::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1)
            .expect("generated report bytes carry canonical header");
        let generated_archive_body = generated_archive_bytes
            .strip_prefix(iroha_crypto::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1)
            .expect("generated archive bytes carry canonical header");
        let report_suffix = generated_report_body
            .strip_prefix(b"machine-generated BFV full-bootstrap release audit report inventory v1")
            .expect("generated report body carries deterministic inventory prefix");
        let archive_suffix = generated_archive_body
            .strip_prefix(
                b"machine-generated BFV full-bootstrap release evidence archive inventory v1",
            )
            .expect("generated archive body carries deterministic inventory prefix");
        let report_body = [
            b"external-review-approved: reviewer-id=sora-zk-audit-wg-2026 independent BFV full-bootstrap release audit report v1"
                .as_slice(),
            report_suffix,
        ]
        .concat();
        let archive_body = [
            b"external-review-evidence-archive: reviewer-id=sora-zk-audit-wg-2026 independent BFV full-bootstrap prover verifier evidence v1"
                .as_slice(),
            archive_suffix,
        ]
        .concat();
        let report_bytes =
            iroha_crypto::bfv_full_bootstrap_release_audit_report_bytes_v1(&report_body)
                .expect("sample external-review report bytes");
        let archive_bytes =
            iroha_crypto::bfv_full_bootstrap_release_audit_archive_bytes_v1(&archive_body)
                .expect("sample external-review archive bytes");
        iroha_crypto::bfv_full_bootstrap_release_audit_external_review_package_and_digest_v1(
            params,
            material,
            artifacts,
            &report_bytes,
            &archive_bytes,
            "sora-zk-audit-wg-2026",
            reviewer_key_pair.private_key(),
        )
        .expect("sample external-review full-bootstrap release audit package and digest")
    }

    fn bfv_full_bootstrap_stark_test_prover_input_material_for_slot(
        slot_index: u32,
    ) -> iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1 {
        let params = iroha_crypto::ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, _relinearization_key) =
            iroha_crypto::keygen_from_seed(&params, b"zk-stark-bfv-full-bootstrap-keygen")
                .expect("BFV keygen");
        let artifacts = sample_bfv_full_bootstrap_artifacts_for_secret(&params, &secret_key);
        let material = iroha_crypto::bfv_full_bootstrap_circuit_material_from_artifacts_v1(
            &params, 1, &artifacts,
        )
        .expect("derive governed full-bootstrap material");
        let blind_rotation = iroha_crypto::decode_bfv_full_bootstrap_blind_rotation_artifact_v1(
            &params,
            &material,
            &artifacts.blind_rotation_key,
        )
        .expect("decode blind-rotation artifact");
        let bootstrap_key = iroha_crypto::full_bootstrap_key_from_material_v1(
            &params,
            &public_key,
            "zk-stark-bfv-full-bootstrap-refresh-key",
            material.clone(),
        )
        .expect("full-bootstrap key");
        let plaintext = iroha_crypto::encode_packed_plaintext_slots(
            &params,
            &(0..usize::from(params.polynomial_degree))
                .map(|slot| u64::try_from((slot * 13 + 11) % 257).expect("slot fits"))
                .collect::<Vec<_>>(),
        )
        .expect("encode packed BFV plaintext");
        let input = iroha_crypto::encrypt_from_seed(
            &params,
            &public_key,
            &plaintext,
            b"zk-stark-bfv-full-bootstrap-input",
        )
        .expect("encrypt BFV input");
        let galois_keys = blind_rotation
            .steps
            .iter()
            .map(|step| {
                iroha_crypto::galois_key_from_seed(
                    &params,
                    &secret_key,
                    step.automorphism_power,
                    b"zk-stark-bfv-full-bootstrap-galois",
                )
                .expect("Galois key")
            })
            .collect::<Vec<_>>();
        let reviewer_key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![0xC3; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("fixture seed derives release reviewer keypair");
        let (release_audit_package, release_audit_package_digest) =
            bfv_full_bootstrap_stark_test_release_audit_package_and_digest(
                &params,
                &material,
                &artifacts,
                &reviewer_key_pair,
            );
        let output =
            iroha_crypto::full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                &input,
                &release_audit_package,
                release_audit_package_digest,
                "sora-zk-audit-wg-2026",
                reviewer_key_pair.public_key(),
            )
            .expect("release-audited artifact-aware full-bootstrap output");
        let input_bound = iroha_crypto::bfv_encrypted_zero_refresh_residual_multiple_bound(&params)
            .expect("input residual bound");
        let output_bound =
            iroha_crypto::bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                input_bound,
                &release_audit_package,
                release_audit_package_digest,
                "sora-zk-audit-wg-2026",
                reviewer_key_pair.public_key(),
            )
            .expect("release-audited artifact-aware full-bootstrap output bound");
        let claim = iroha_crypto::bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            slot_index,
            input,
            output,
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple,
            input_bound,
            output_bound,
        )
        .expect("derive execution proof claim");
        let witness_material =
            iroha_crypto::bfv_full_bootstrap_execution_witness_digest_material_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                &claim,
            )
            .expect("derive execution witness material");
        let proof_input = iroha_crypto::bfv_full_bootstrap_execution_proof_input_material_v1(
            &public_key,
            &witness_material,
        )
        .expect("build execution proof input material");
        let prover_key = iroha_crypto::decode_bfv_full_bootstrap_proof_key_artifact_v1(
            &params,
            &material,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            &artifacts.prover_key,
        )
        .expect("decode prover key artifact");
        let verifier_key = iroha_crypto::decode_bfv_full_bootstrap_proof_key_artifact_v1(
            &params,
            &material,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
            &artifacts.verifier_key,
        )
        .expect("decode verifier key artifact");
        iroha_crypto::bfv_full_bootstrap_execution_prover_input_material_v1(
            &proof_input,
            &prover_key,
            &verifier_key,
        )
        .expect("build BFV execution prover input material")
    }

    #[test]
    fn bfv_full_bootstrap_air_transcript_label_is_canonical_base_label() {
        let base = iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1;
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            base
        ));

        for label in [
            format!("{base}:0"),
            format!("{base}:1"),
            format!("{base}:00"),
            format!("{base}:01"),
            format!("{base}:0001"),
            format!(
                "{}:{}",
                base,
                BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS - 1
            ),
            format!(
                "{}:{}",
                base, BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS
            ),
            format!("{base}:+1"),
            format!("{base}:1 "),
            format!("{base}:1:2"),
            format!("{base}-1"),
        ] {
            assert!(
                !bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(&label),
                "noncanonical BFV STARK transcript label must be rejected: {label:?}"
            );
        }
    }

    #[test]
    fn bfv_full_bootstrap_air_prover_binds_statement_and_public_openings() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let env: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            &env.transcript_label
        ));
        let compressed_bytes =
            norito::to_compressed_bytes(&env, Some(norito::CompressionConfig::default()))
                .expect("encode compressed BFV STARK envelope");
        assert_ne!(
            compressed_bytes, bytes,
            "compressed BFV STARK envelope bytes must differ from canonical v1 bytes"
        );
        let compressed_env: StarkVerifyEnvelopeV1 = norito::decode_from_bytes(&compressed_bytes)
            .expect("compressed BFV STARK envelope must remain structurally decodable");
        assert_eq!(
            norito::to_bytes(&compressed_env).expect("re-encode compressed BFV STARK envelope"),
            bytes,
            "compressed BFV STARK envelope must decode to the same typed proof as canonical bytes"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&compressed_bytes, &material),
            "artifact-bound BFV verifier must reject noncanonical compressed proof framing"
        );
        let expected_params =
            bfv_full_bootstrap_stark_air_params_v1(material.proof_input_material.statement_hash);
        assert!(bfv_full_bootstrap_stark_air_params_match_v1(
            &env.params,
            &expected_params
        ));
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;
        assert!(
            verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must accept a generated native AIR envelope without private trace rows"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &compressed_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject noncanonical compressed proof framing"
        );
        let expected_base_indices = bfv_full_bootstrap_expected_base_indices_v1(
            material.proof_input_material.statement_hash,
            material.arithmetic_trace_material_digest,
            usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1),
            material.arithmetic_trace_material.rows.len(),
        )
        .expect("derive BFV explicit public-padding base indices");
        let suffixed_label_bytes =
            prove_stark_fri_reserved_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
                expected_params.clone(),
                format!(
                    "{}:1",
                    iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
                ),
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1.to_owned(),
                public_digest,
                material.arithmetic_trace_material.rows.clone(),
                material
                    .arithmetic_air_evaluation_material
                    .composition_values
                    .clone(),
                &expected_base_indices,
            )
            .expect("build structurally valid suffixed-label explicit BFV AIR envelope");
        assert!(
            verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
                &suffixed_label_bytes,
                &StarkVerifierLimits::default(),
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                &public_digest,
                &material.arithmetic_trace_material.rows,
                &material
                    .arithmetic_air_evaluation_material
                    .composition_values,
                &expected_base_indices,
            ),
            "suffixed-label envelope must remain a valid explicit-schedule AIR proof"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &suffixed_label_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject suffixed-label alternate proof encodings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&suffixed_label_bytes, &material),
            "artifact-bound BFV verifier must reject suffixed-label alternate proof encodings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]),
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject zero statement hashes before envelope replay"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]),
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject zero trace-material digests before envelope replay"
        );
        let placeholder_statement_hash =
            iroha_crypto::Hash::new(b"pending BFV full-bootstrap execution witness digest");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                placeholder_statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject placeholder statement hashes before envelope replay"
        );
        let placeholder_trace_material_digest =
            iroha_crypto::Hash::new(b"pending BFV full-bootstrap execution witness digest");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                placeholder_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject placeholder trace-material digests before envelope replay"
        );
        let delayed_placeholder_statement_preimage = [
            b" \n\t".as_slice(),
            b"full-bootstrap material before placeholder: ".as_slice(),
            b"pending BFV full-bootstrap execution witness digest".as_slice(),
        ]
        .concat();
        let delayed_placeholder_statement_hash =
            iroha_crypto::Hash::new(&delayed_placeholder_statement_preimage);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                delayed_placeholder_statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject leading-whitespace delayed placeholder statement hashes before envelope replay"
        );
        let separator_spelled_statement_preimages = [
            b"p-e-n-d-i-n-g BFV full-bootstrap execution witness digest".as_slice(),
            b"p.e.n.d.i.n.g BFV full-bootstrap execution witness digest".as_slice(),
            b"p_e_n_d_i_n_g BFV full-bootstrap execution witness digest".as_slice(),
        ];
        let separator_spelled_statement_hashes =
            separator_spelled_statement_preimages.map(iroha_crypto::Hash::new);
        for separator_spelled_statement_hash in separator_spelled_statement_hashes.iter().copied() {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    separator_spelled_statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject separator-spelled placeholder statement hashes before envelope replay"
            );
        }
        let delayed_separator_spelled_statement_hashes =
            separator_spelled_statement_preimages.map(|preimage| {
                let delayed_preimage = [
                    b" \n\t".as_slice(),
                    b"full-bootstrap material before placeholder: ".as_slice(),
                    preimage,
                ]
                .concat();
                iroha_crypto::Hash::new(&delayed_preimage)
            });
        for delayed_separator_spelled_statement_hash in
            delayed_separator_spelled_statement_hashes.iter().copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    delayed_separator_spelled_statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject delayed separator-spelled placeholder statement hashes before envelope replay"
            );
        }
        let delayed_placeholder_trace_material_digest =
            iroha_crypto::Hash::new(&delayed_placeholder_statement_preimage);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                delayed_placeholder_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject leading-whitespace delayed placeholder trace-material digests before envelope replay"
        );
        let separator_spelled_trace_material_preimages = separator_spelled_statement_preimages;
        let separator_spelled_trace_material_digests =
            separator_spelled_trace_material_preimages.map(iroha_crypto::Hash::new);
        for separator_spelled_trace_material_digest in
            separator_spelled_trace_material_digests.iter().copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    material.proof_input_material.statement_hash,
                    separator_spelled_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject separator-spelled placeholder trace-material digests before envelope replay"
            );
        }
        let delayed_separator_spelled_trace_material_digests =
            separator_spelled_trace_material_preimages.map(|preimage| {
                let delayed_preimage = [
                    b" \n\t".as_slice(),
                    b"full-bootstrap material before placeholder: ".as_slice(),
                    preimage,
                ]
                .concat();
                iroha_crypto::Hash::new(&delayed_preimage)
            });
        for delayed_separator_spelled_trace_material_digest in
            delayed_separator_spelled_trace_material_digests
                .iter()
                .copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    material.proof_input_material.statement_hash,
                    delayed_separator_spelled_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject delayed separator-spelled placeholder trace-material digests before envelope replay"
            );
        }
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                u32::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_COUNT_V1),
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject out-of-range public slot headers before envelope replay"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                iroha_crypto::Hash::new(b"stale BFV full-bootstrap public statement hash"),
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must bind the statement hash"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index.saturating_add(1),
                witness.bound_mode,
            ),
            "public-padding BFV verifier must bind the public slot index"
        );
        let alternate_bound_mode = match witness.bound_mode {
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple => {
                iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::BoundedNoise
            }
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::BoundedNoise => {
                iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple
            }
        };
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                alternate_bound_mode,
            ),
            "public-padding BFV verifier must bind the public bound mode"
        );
        let air = env.proof.air.as_ref().expect("BFV AIR section");
        let domain_size = 1_usize << usize::from(env.params.n_log2);
        let public_padding_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &material.proof_input_material.statement_hash,
            trace_material_digest: &material.arithmetic_trace_material_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        assert!(
            stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                public_padding_context
            ),
            "private BFV public-padding context must accept the canonical STARK parameter profile"
        );
        let mut stale_public_padding_params = env.params.clone();
        stale_public_padding_params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap context statement"),
        )
        .domain_tag;
        assert!(
            !stark_air_context_matches_statement(
                &stale_public_padding_params,
                air,
                domain_size,
                public_padding_context,
            ),
            "private BFV public-padding context must reject statement-bound domain-tag drift"
        );
        let mut drifted_public_padding_params = env.params.clone();
        drifted_public_padding_params.hash_fn = STARK_HASH_POSEIDON2_V1;
        assert!(
            !stark_air_context_matches_statement(
                &drifted_public_padding_params,
                air,
                domain_size,
                public_padding_context,
            ),
            "private BFV public-padding context must reject canonical parameter-profile drift"
        );
        let zero_statement_hash = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
        let zero_public_digest = [0_u8; iroha_crypto::Hash::LENGTH];
        let zero_statement_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &zero_statement_hash,
            trace_material_digest: &material.arithmetic_trace_material_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        let mut zero_digest_air = air.clone();
        zero_digest_air.public_digest = zero_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &zero_digest_air,
                domain_size,
                zero_statement_context,
            ),
            "private BFV public-padding context must reject zero statement hashes even when the AIR digest matches"
        );
        let zero_trace_digest = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
        let zero_trace_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &material.proof_input_material.statement_hash,
            trace_material_digest: &zero_trace_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        assert!(
            !stark_air_context_matches_statement(&env.params, air, domain_size, zero_trace_context,),
            "private BFV public-padding context must reject zero trace-material digests even when the AIR digest matches"
        );
        let first_public_opening = air.openings.first().expect("BFV AIR public opening");
        assert!(
            stark_air_composition_value_for_context(
                zero_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &zero_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a zero statement hash"
        );
        let placeholder_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
            placeholder_statement_hash.into();
        let placeholder_statement_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &placeholder_statement_hash,
                trace_material_digest: &material.arithmetic_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        let mut placeholder_digest_air = air.clone();
        placeholder_digest_air.public_digest = placeholder_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &placeholder_digest_air,
                domain_size,
                placeholder_statement_context,
            ),
            "private BFV public-padding context must reject placeholder statement hashes even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                placeholder_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &placeholder_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a placeholder statement hash"
        );
        let placeholder_trace_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &material.proof_input_material.statement_hash,
                trace_material_digest: &placeholder_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                placeholder_trace_context,
            ),
            "private BFV public-padding context must reject placeholder trace-material digests even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                placeholder_trace_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a placeholder trace-material digest"
        );
        let delayed_placeholder_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
            delayed_placeholder_statement_hash.into();
        let delayed_placeholder_statement_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &delayed_placeholder_statement_hash,
                trace_material_digest: &material.arithmetic_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        let mut delayed_placeholder_digest_air = air.clone();
        delayed_placeholder_digest_air.public_digest = delayed_placeholder_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &delayed_placeholder_digest_air,
                domain_size,
                delayed_placeholder_statement_context,
            ),
            "private BFV public-padding context must reject leading-whitespace delayed placeholder statement hashes even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                delayed_placeholder_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &delayed_placeholder_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a leading-whitespace delayed placeholder statement hash"
        );
        for separator_spelled_statement_hash in separator_spelled_statement_hashes.iter().copied() {
            let separator_spelled_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
                separator_spelled_statement_hash.into();
            let separator_spelled_statement_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &separator_spelled_statement_hash,
                    trace_material_digest: &material.arithmetic_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            let mut separator_spelled_digest_air = air.clone();
            separator_spelled_digest_air.public_digest = separator_spelled_public_digest;
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    &separator_spelled_digest_air,
                    domain_size,
                    separator_spelled_statement_context,
                ),
                "private BFV public-padding context must reject separator-spelled placeholder statement hashes even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    separator_spelled_statement_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &separator_spelled_public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a separator-spelled placeholder statement hash"
            );
        }
        for delayed_separator_spelled_statement_hash in
            delayed_separator_spelled_statement_hashes.iter().copied()
        {
            let delayed_separator_spelled_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
                delayed_separator_spelled_statement_hash.into();
            let delayed_separator_spelled_statement_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &delayed_separator_spelled_statement_hash,
                    trace_material_digest: &material.arithmetic_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            let mut delayed_separator_spelled_digest_air = air.clone();
            delayed_separator_spelled_digest_air.public_digest =
                delayed_separator_spelled_public_digest;
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    &delayed_separator_spelled_digest_air,
                    domain_size,
                    delayed_separator_spelled_statement_context,
                ),
                "private BFV public-padding context must reject delayed separator-spelled placeholder statement hashes even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    delayed_separator_spelled_statement_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &delayed_separator_spelled_public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a delayed separator-spelled placeholder statement hash"
            );
        }
        let delayed_placeholder_trace_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &material.proof_input_material.statement_hash,
                trace_material_digest: &delayed_placeholder_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                delayed_placeholder_trace_context,
            ),
            "private BFV public-padding context must reject leading-whitespace delayed placeholder trace-material digests even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                delayed_placeholder_trace_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a leading-whitespace delayed placeholder trace-material digest"
        );
        for separator_spelled_trace_material_digest in
            separator_spelled_trace_material_digests.iter().copied()
        {
            let separator_spelled_trace_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &material.proof_input_material.statement_hash,
                    trace_material_digest: &separator_spelled_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    air,
                    domain_size,
                    separator_spelled_trace_context,
                ),
                "private BFV public-padding context must reject separator-spelled placeholder trace-material digests even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    separator_spelled_trace_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a separator-spelled placeholder trace-material digest"
            );
        }
        for delayed_separator_spelled_trace_material_digest in
            delayed_separator_spelled_trace_material_digests
                .iter()
                .copied()
        {
            let delayed_separator_spelled_trace_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &material.proof_input_material.statement_hash,
                    trace_material_digest: &delayed_separator_spelled_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    air,
                    domain_size,
                    delayed_separator_spelled_trace_context,
                ),
                "private BFV public-padding context must reject delayed separator-spelled placeholder trace-material digests even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    delayed_separator_spelled_trace_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a delayed separator-spelled placeholder trace-material digest"
            );
        }
        let opening_indices = air
            .openings
            .iter()
            .map(|opening| opening.index)
            .collect::<Vec<_>>();
        let opened_rows = air
            .openings
            .iter()
            .map(|opening| opening.row.clone())
            .collect::<Vec<_>>();
        let opened_next_rows = air
            .openings
            .iter()
            .map(|opening| opening.next_row.clone())
            .collect::<Vec<_>>();
        iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_transcript_public_padding_openings_v1(
            &opening_indices,
            &opened_rows,
            &opened_next_rows,
            material.proof_input_material.statement_hash,
            material.arithmetic_trace_material_digest,
            material.proof_input_material.witness_material.slot_index,
            material.proof_input_material.witness_material.bound_mode,
        )
        .expect("BFV transcript-bound public opening set");
        for opening in &air.openings {
            iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_public_padding_opening_v1(
                opening.index,
                &opening.row,
                &opening.next_row,
                material.proof_input_material.statement_hash,
                material.proof_input_material.witness_material.slot_index,
                material.proof_input_material.witness_material.bound_mode,
            )
            .expect("BFV sampled opening is a canonical public padding row");
        }

        let mut duplicate_opening_env = env.clone();
        {
            let duplicate_air = duplicate_opening_env
                .proof
                .air
                .as_mut()
                .expect("BFV AIR section");
            duplicate_air.openings[1] = duplicate_air.openings[0].clone();
        }
        let duplicate_opening_bytes =
            norito::to_bytes(&duplicate_opening_env).expect("encode duplicate BFV AIR opening");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &duplicate_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject duplicated sampled public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&duplicate_opening_bytes, &material),
            "artifact-bound BFV verifier must reject duplicated sampled public openings"
        );

        let mut reordered_opening_env = env.clone();
        reordered_opening_env
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings
            .swap(0, 1);
        let reordered_opening_bytes =
            norito::to_bytes(&reordered_opening_env).expect("encode reordered BFV AIR openings");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &reordered_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject reordered public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&reordered_opening_bytes, &material),
            "artifact-bound BFV verifier must reject reordered public openings"
        );

        let mut truncated_opening_env = env.clone();
        truncated_opening_env
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings
            .pop();
        let truncated_opening_bytes =
            norito::to_bytes(&truncated_opening_env).expect("encode truncated BFV AIR openings");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &truncated_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject truncated sampled public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&truncated_opening_bytes, &material),
            "artifact-bound BFV verifier must reject truncated sampled public openings"
        );

        let generic_bfv_err = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            expected_params.clone(),
            "IROHA-TEST-BFV-GENERIC-AIR-REJECTED".to_owned(),
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1.to_owned(),
            public_digest,
            material.arithmetic_trace_material.rows.clone(),
            material
                .arithmetic_air_evaluation_material
                .composition_values
                .clone(),
        )
        .expect_err("generic explicit AIR builder must reject BFV circuit ids");
        assert!(
            generic_bfv_err.contains("BFV full-bootstrap"),
            "generic BFV AIR rejection must name the reserved family: {generic_bfv_err}"
        );

        let mut stale_domain = env.clone();
        stale_domain.params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap statement hash"),
        )
        .domain_tag;
        let stale_domain_bytes = norito::to_bytes(&stale_domain).expect("encode stale domain");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &stale_domain_bytes,
            &material
        ));

        let mut stale_material = material.clone();
        stale_material.proof_input_material.statement_hash =
            iroha_crypto::Hash::new(b"stale BFV full-bootstrap statement hash");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes,
            &stale_material
        ));

        let mut tampered_opening = env;
        tampered_opening
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings[0]
            .row[9] = 1;
        let tampered_opening_bytes =
            norito::to_bytes(&tampered_opening).expect("encode tampered opening");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &tampered_opening_bytes,
            &material
        ));
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &tampered_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject drifted public row openings"
        );
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_auxiliary_generic_composition_sidecars() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));

        let mut sidecar_envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        assert!(sidecar_envelope.proof.commits.comp_root.is_none());
        assert!(sidecar_envelope.proof.comp_values.is_none());
        attach_valid_auxiliary_composition_values(&mut sidecar_envelope);
        assert!(sidecar_envelope.proof.commits.comp_root.is_some());
        assert_eq!(
            sidecar_envelope.proof.comp_values.as_ref().map(Vec::len),
            Some(sidecar_envelope.proof.queries.len())
        );

        let mut comp_root_only = sidecar_envelope.clone();
        comp_root_only.proof.comp_values = None;
        let mut comp_values_only = sidecar_envelope.clone();
        comp_values_only.proof.commits.comp_root = None;
        let mut truncated_values = sidecar_envelope.clone();
        truncated_values
            .proof
            .comp_values
            .as_mut()
            .expect("composition values")
            .pop();

        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;
        for (case, envelope) in [
            ("paired auxiliary sidecars", sidecar_envelope),
            ("comp_root without comp_values", comp_root_only),
            ("comp_values without comp_root", comp_values_only),
            ("truncated comp_values", truncated_values),
        ] {
            let auxiliary_bytes =
                norito::to_bytes(&envelope).expect("encode auxiliary BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &auxiliary_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&auxiliary_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &auxiliary_bytes,
                    material.proof_input_material.statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "BFV public-padding verifier must reject {case}"
            );
        }
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_malformed_proof_and_air_bindings() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode malformed BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &malformed_bytes,
                    material.proof_input_material.statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "BFV public-padding verifier must reject {case}"
            );
        };

        let mut bad_proof_version = envelope.clone();
        bad_proof_version.proof.version = 2;
        assert_rejected("non-v1 proof version", &bad_proof_version);

        let mut bad_commit_version = envelope.clone();
        bad_commit_version.proof.commits.version = 2;
        assert_rejected("non-v1 commitment version", &bad_commit_version);

        let mut missing_air = envelope.clone();
        missing_air.proof.air = None;
        assert_rejected("missing AIR section", &missing_air);

        let mut foreign_air = envelope.clone();
        foreign_air
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .circuit_id = "stark/fri/sha256-goldilocks:foreign-bfv-air".to_owned();
        assert_rejected("foreign AIR circuit id", &foreign_air);

        let mut drifted_composition_root = envelope.clone();
        drifted_composition_root
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .composition_root[0] ^= 0x01;
        assert_rejected("AIR composition-root drift", &drifted_composition_root);

        let mut drifted_trace_root = envelope.clone();
        drifted_trace_root
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .trace_root[0] ^= 0x01;
        assert_rejected("AIR trace-root drift", &drifted_trace_root);

        let mut drifted_fri_root = envelope.clone();
        drifted_fri_root.proof.commits.roots[0][0] ^= 0x01;
        assert_rejected("FRI composition-root drift", &drifted_fri_root);

        let mut missing_query = envelope.clone();
        missing_query.proof.queries.pop();
        assert_rejected("missing FRI query chain", &missing_query);

        let mut reordered_query = envelope.clone();
        assert!(
            reordered_query.proof.queries.len() > 1,
            "BFV AIR test envelope has multiple FRI query chains"
        );
        reordered_query.proof.queries.swap(0, 1);
        assert_rejected("reordered FRI query chains", &reordered_query);

        let mut missing_opening = envelope;
        missing_opening
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings
            .pop();
        assert_rejected("missing AIR opening", &missing_opening);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_opening_path_and_sample_drift() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode malformed BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &malformed_bytes,
                    material.proof_input_material.statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "BFV public-padding verifier must reject {case}"
            );
        };

        let mut wrong_opening_index = envelope.clone();
        let opening_index = wrong_opening_index
            .proof
            .air
            .as_ref()
            .expect("AIR section")
            .openings[0]
            .index;
        wrong_opening_index
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .index = opening_index.wrapping_add(1);
        assert_rejected("opening index drift", &wrong_opening_index);

        let mut swapped_row_paths = envelope.clone();
        let opening = &mut swapped_row_paths
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0];
        core::mem::swap(&mut opening.row_path, &mut opening.next_row_path);
        assert_rejected("swapped row and next-row paths", &swapped_row_paths);

        let mut tampered_row_path = envelope.clone();
        tampered_row_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .row_path
            .siblings
            .first_mut()
            .expect("row path sibling")[0] ^= 0x01;
        assert_rejected("row Merkle path drift", &tampered_row_path);

        let mut tampered_next_row_path = envelope.clone();
        tampered_next_row_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .next_row_path
            .siblings
            .first_mut()
            .expect("next-row path sibling")[0] ^= 0x01;
        assert_rejected("next-row Merkle path drift", &tampered_next_row_path);

        let mut tampered_composition_path = envelope.clone();
        tampered_composition_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .composition_path
            .siblings
            .first_mut()
            .expect("composition path sibling")[0] ^= 0x01;
        assert_rejected(
            "composition-value Merkle path drift",
            &tampered_composition_path,
        );

        let mut tampered_composition_value = envelope.clone();
        tampered_composition_value
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .composition_value ^= 0x01;
        assert_rejected(
            "opened composition-value drift",
            &tampered_composition_value,
        );

        let mut tampered_fri_base_value = envelope.clone();
        tampered_fri_base_value.proof.queries[0][0].y0 ^= 0x01;
        assert_rejected("FRI base value drift", &tampered_fri_base_value);

        let mut duplicated_opening = envelope;
        let air = duplicated_opening.proof.air.as_mut().expect("AIR section");
        assert!(air.openings.len() > 1, "BFV AIR test envelope has queries");
        air.openings[1] = air.openings[0].clone();
        assert_rejected("duplicated AIR opening", &duplicated_opening);
    }

    #[test]
    fn bfv_full_bootstrap_air_verifier_limits_are_enforced() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        assert!(
            verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &StarkVerifierLimits::default(),
                &material,
            )
        );

        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let air = envelope.proof.air.as_ref().expect("BFV AIR section");
        let witness = &material.proof_input_material.witness_material;

        let mut tight_envelope_bytes = StarkVerifierLimits::default();
        tight_envelope_bytes.max_envelope_bytes = bytes.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_envelope_bytes,
                &material,
            ),
            "BFV native AIR verifier must honor envelope byte limits"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
                &bytes,
                &tight_envelope_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "BFV public-padding verifier must honor envelope byte limits"
        );

        let mut tight_transcript_label = StarkVerifierLimits::default();
        tight_transcript_label.max_transcript_label_len =
            envelope.transcript_label.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_transcript_label,
                &material,
            ),
            "BFV native AIR verifier must honor transcript-label limits"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
                &bytes,
                &tight_transcript_label,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "BFV public-padding verifier must honor transcript-label limits"
        );

        let mut tight_queries = StarkVerifierLimits::default();
        tight_queries.max_queries = envelope.proof.queries.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_queries,
                &material,
            ),
            "BFV native AIR verifier must honor query-count limits"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
                &bytes,
                &tight_queries,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "BFV public-padding verifier must honor query-count limits"
        );

        let mut tight_air_width = StarkVerifierLimits::default();
        tight_air_width.max_air_width = usize::from(air.trace_width).saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_air_width,
                &material,
            ),
            "BFV native AIR verifier must honor AIR width limits"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
                &bytes,
                &tight_air_width,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "BFV public-padding verifier must honor AIR width limits"
        );

        let mut tight_merkle_depth = StarkVerifierLimits::default();
        tight_merkle_depth.max_merkle_depth = usize::from(envelope.params.n_log2).saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_merkle_depth,
                &material,
            ),
            "BFV native AIR verifier must honor Merkle-depth limits"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
                &bytes,
                &tight_merkle_depth,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "BFV public-padding verifier must honor Merkle-depth limits"
        );
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_parameter_profile_drift() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode parameter-drift BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &malformed_bytes,
                    material.proof_input_material.statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "BFV public-padding verifier must reject {case}"
            );
        };

        let mut bad_version = envelope.clone();
        bad_version.params.version = 2;
        assert_rejected("STARK parameter version drift", &bad_version);

        let mut bad_domain_depth = envelope.clone();
        bad_domain_depth.params.n_log2 = bad_domain_depth.params.n_log2.saturating_add(1);
        assert_rejected("STARK domain-depth drift", &bad_domain_depth);

        let mut bad_blowup = envelope.clone();
        bad_blowup.params.blowup_log2 = bad_blowup.params.blowup_log2.saturating_add(1);
        assert_rejected("STARK blowup drift", &bad_blowup);

        let mut bad_fold_arity = envelope.clone();
        bad_fold_arity.params.fold_arity = 4;
        assert_rejected("STARK fold-arity drift", &bad_fold_arity);

        let mut bad_merkle_arity = envelope.clone();
        bad_merkle_arity.params.merkle_arity = 4;
        assert_rejected("STARK Merkle-arity drift", &bad_merkle_arity);

        let mut bad_hash_selector = envelope.clone();
        bad_hash_selector.params.hash_fn = STARK_HASH_POSEIDON2_V1;
        assert_rejected("STARK hash-selector drift", &bad_hash_selector);

        let mut bad_query_count = envelope.clone();
        bad_query_count.params.queries = bad_query_count.params.queries.saturating_sub(1);
        assert_rejected("STARK query-count header drift", &bad_query_count);

        let mut stale_domain = envelope;
        stale_domain.params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap parameter profile"),
        )
        .domain_tag;
        assert_rejected("statement-bound domain-tag drift", &stale_domain);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_stale_prover_input_material() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));

        let assert_rejected_material =
            |case: &str, material: iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1| {
                assert!(
                    iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(
                        &material
                    )
                    .is_err(),
                    "mutated BFV prover input material must fail validation: {case}"
                );
                assert!(
                    !verify_stark_fri_bfv_full_bootstrap_air_envelope(&bytes, &material),
                    "BFV native AIR verifier must reject {case}"
                );
                assert!(
                    prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material).is_err(),
                    "BFV native AIR prover must reject {case}"
                );
            };

        let mut stale_version = material.clone();
        stale_version.version = stale_version.version.saturating_add(1);
        assert_rejected_material("stale prover input material version", stale_version);

        let mut stale_field_count = material.clone();
        stale_field_count.field_count = stale_field_count.field_count.saturating_add(1);
        assert_rejected_material("stale prover input material field count", stale_field_count);

        let mut stale_proof_input_version = material.clone();
        stale_proof_input_version.proof_input_material.version = stale_proof_input_version
            .proof_input_material
            .version
            .saturating_add(1);
        assert_rejected_material(
            "stale proof-input material version",
            stale_proof_input_version,
        );

        let mut stale_trace_digest = material.clone();
        stale_trace_digest.arithmetic_trace_material_digest =
            iroha_crypto::Hash::new(b"stale BFV arithmetic trace material digest");
        assert_rejected_material("stale arithmetic trace material digest", stale_trace_digest);

        let mut stale_air_evaluation_digest = material.clone();
        stale_air_evaluation_digest.arithmetic_air_evaluation_material_digest =
            iroha_crypto::Hash::new(b"stale BFV arithmetic AIR evaluation material digest");
        assert_rejected_material(
            "stale arithmetic AIR evaluation material digest",
            stale_air_evaluation_digest,
        );

        let mut drifted_trace_rows = material.clone();
        drifted_trace_rows.arithmetic_trace_material.rows[0][0] ^= 0x01;
        assert_rejected_material("drifted arithmetic trace rows", drifted_trace_rows);

        let mut drifted_composition_values = material.clone();
        drifted_composition_values
            .arithmetic_air_evaluation_material
            .composition_values[0] ^= 0x01;
        assert_rejected_material(
            "drifted arithmetic AIR composition values",
            drifted_composition_values,
        );

        let mut stale_public_opening_digest = material.clone();
        stale_public_opening_digest.public_opening_material_digest =
            iroha_crypto::Hash::new(b"stale BFV public opening material digest");
        assert_rejected_material(
            "stale public opening material digest",
            stale_public_opening_digest,
        );

        let alternate_material = bfv_full_bootstrap_stark_test_prover_input_material_for_slot(1);
        let mut retargeted_public_opening_material = material.clone();
        retargeted_public_opening_material.public_opening_material =
            alternate_material.public_opening_material.clone();
        retargeted_public_opening_material.public_opening_material_digest =
            alternate_material.public_opening_material_digest;
        assert_rejected_material(
            "retargeted public opening material",
            retargeted_public_opening_material,
        );

        let mut swapped_proof_keys = material;
        core::mem::swap(
            &mut swapped_proof_keys.prover_key,
            &mut swapped_proof_keys.verifier_key,
        );
        assert_rejected_material("swapped BFV native proof keys", swapped_proof_keys);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_valid_cross_statement_material_replay() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material_for_slot(0);
        let alternate_material = bfv_full_bootstrap_stark_test_prover_input_material_for_slot(1);
        iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(
            &alternate_material,
        )
        .expect("alternate BFV prover material is internally valid");
        assert_ne!(
            material.proof_input_material.statement_hash,
            alternate_material.proof_input_material.statement_hash,
            "alternate slot must bind a distinct BFV statement hash"
        );
        assert_ne!(
            material.arithmetic_trace_material_digest,
            alternate_material.arithmetic_trace_material_digest,
            "alternate slot must bind distinct trace material"
        );
        assert_ne!(
            material.arithmetic_air_evaluation_material_digest,
            alternate_material.arithmetic_air_evaluation_material_digest,
            "alternate slot must bind distinct AIR evaluation material"
        );

        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let alternate_bytes =
            prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&alternate_material)
                .expect("alternate BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &alternate_bytes,
            &alternate_material,
        ));
        assert_ne!(
            bytes, alternate_bytes,
            "statement-specific BFV native AIR envelopes must differ"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&bytes, &alternate_material),
            "valid BFV native AIR envelope must not replay against another valid statement package"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&alternate_bytes, &material),
            "alternate BFV native AIR envelope must not replay against the original statement package"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_unsupported_fold_arity() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 4,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:invalid".to_owned(),
        };
        let err = synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned())
            .expect_err("unsupported fold_arity must fail");
        assert!(
            err.contains("fold_arity"),
            "error should mention fold_arity, got: {err}"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_blowup_larger_than_domain() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 5,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:invalid-blowup-domain".to_owned(),
        };
        let err = synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned())
            .expect_err("blowup_log2 larger than n_log2 must fail");
        assert!(
            err.contains("blowup factor"),
            "error should mention blowup factor, got: {err}"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_tampered_domain_tag() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:tamper".to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-STARK".to_owned(),
            "stark/fri/sha256-goldilocks:tamper".to_owned(),
            [0x33; 32],
        )
        .expect("ok");
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode synthesized envelope");
        envelope.params.domain_tag.push_str(":mutated");
        let tampered = norito::to_bytes(&envelope).expect("encode mutated envelope");
        assert!(!verify_stark_fri_envelope(&tampered));
    }

    #[test]
    fn synthesized_envelope_rejects_malformed_payload() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:malformed".to_owned(),
        };
        let mut bytes =
            synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned()).expect("ok");
        bytes.truncate(bytes.len().saturating_sub(1));
        assert!(!verify_stark_fri_envelope(&bytes));
    }
