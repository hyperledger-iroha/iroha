    #[test]
    fn fhe_execution_policy_validate_rejects_output_overflow() {
        let mut policy = sample_fhe_execution_policy();
        policy.max_output_ciphertexts = NonZeroU16::new(16).expect("nonzero");
        let error = policy
            .validate()
            .expect_err("output ciphertext count above input count must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_output_ciphertexts",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_unsupported_rounding_mode() {
        let mut policy = sample_fhe_execution_policy();
        policy.rounding_mode = FheDeterministicRoundingModeV1::Floor;
        let error = policy
            .validate()
            .expect_err("unsupported rounding mode must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rounding_mode",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_exact_evaluator_budget_overflow() {
        let evaluator_budget = BfvEvaluationBudget::exact_evaluator_v1();

        let mut depth_overflow = sample_fhe_execution_policy();
        depth_overflow.max_multiplication_depth =
            NonZeroU16::new(evaluator_budget.max_multiplicative_depth + 1).expect("nonzero");
        let error = depth_overflow
            .validate()
            .expect_err("policy depth above exact evaluator budget must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_multiplication_depth",
                ..
            }
        ));

        let mut bootstrap_overflow = sample_fhe_execution_policy();
        bootstrap_overflow.max_bootstrap_count = evaluator_budget.max_bootstrap_refresh_rounds + 1;
        let error = bootstrap_overflow
            .validate()
            .expect_err("policy bootstrap count above exact evaluator budget must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_bootstrap_count",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_public_key_proof_statement_sentinel() {
        let mut policy = sample_fhe_execution_policy();
        policy.public_key_proof_statement_digest = Some(zero_prehash_statement_hash());
        let error = policy
            .validate()
            .expect_err("public-key proof statement placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key_proof_statement_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));
    }

    #[test]
    fn fhe_execution_policy_validate_requires_public_key_proof_statement_digest() {
        let mut policy = sample_fhe_execution_policy();
        policy.public_key_proof_statement_digest = None;
        let error = policy
            .validate()
            .expect_err("production FHE execution policies must bind public-key proof statements");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key_proof_statement_digest",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_incomplete_release_audit_context() {
        let mut non_full_bootstrap_policy = sample_fhe_execution_policy();
        non_full_bootstrap_policy.full_bootstrap_release_audit_package_digest =
            Some(sample_hash(94));
        let error = non_full_bootstrap_policy
            .validate()
            .expect_err("release-audit fields must require full-bootstrap material policy");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));

        let reviewer_keypair = sample_ed25519_keypair(0x50);
        let (package, package_digest) =
            sample_full_bootstrap_release_audit_package_and_digest(&reviewer_keypair);
        let mut full_bootstrap_policy = sample_fhe_execution_policy();
        full_bootstrap_policy.bootstrap_key_zero_refresh_proof_statement_digest = None;
        full_bootstrap_policy.full_bootstrap_material_proof_statement_digest =
            Some(sample_hash(93));

        let mut package_only_policy = full_bootstrap_policy.clone();
        package_only_policy.full_bootstrap_release_audit_package = Some(package.clone());
        let error = package_only_policy
            .validate()
            .expect_err("release-audit package alone must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package_digest",
                ..
            }
        ));
        assert!(
            error
                .to_string()
                .contains("requires release audit package digest")
        );

        let mut digest_only_policy = full_bootstrap_policy.clone();
        digest_only_policy.full_bootstrap_release_audit_package_digest = Some(package_digest);
        let error = digest_only_policy
            .validate()
            .expect_err("release-audit package digest alone must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));
        assert!(error.to_string().contains("requires release audit package"));

        let mut reviewer_id_only_policy = full_bootstrap_policy.clone();
        reviewer_id_only_policy.full_bootstrap_release_audit_trusted_reviewer_id =
            Some(SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID.to_string());
        let error = reviewer_id_only_policy
            .validate()
            .expect_err("release-audit trusted reviewer id alone must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));
        assert!(error.to_string().contains("requires release audit package"));

        let mut reviewer_key_only_policy = full_bootstrap_policy;
        reviewer_key_only_policy.full_bootstrap_release_audit_trusted_reviewer_public_key =
            Some(reviewer_keypair.public_key().clone());
        let error = reviewer_key_only_policy
            .validate()
            .expect_err("release-audit trusted reviewer public key alone must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));
        assert!(error.to_string().contains("requires release audit package"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_execution_policy_validate_binds_release_audit_trusted_reviewer() {
        let reviewer_keypair = sample_ed25519_keypair(0x51);
        let (package, package_digest) =
            sample_full_bootstrap_release_audit_package_and_digest(&reviewer_keypair);

        let mut policy = sample_fhe_execution_policy();
        policy.bootstrap_key_zero_refresh_proof_statement_digest = None;
        policy.full_bootstrap_material_proof_statement_digest = Some(sample_hash(93));
        policy.full_bootstrap_release_audit_package = Some(package.clone());
        policy.full_bootstrap_release_audit_package_digest = Some(package_digest);
        policy.full_bootstrap_release_audit_trusted_reviewer_id =
            Some(SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID.to_string());
        policy.full_bootstrap_release_audit_trusted_reviewer_public_key =
            Some(reviewer_keypair.public_key().clone());
        policy
            .validate()
            .expect("policy should accept matching trusted release-audit reviewer");

        let mut report_tampered_package = package.clone();
        report_tampered_package
            .audit_report_bytes
            .extend_from_slice(b"; policy report byte tamper after signing");
        let mut report_tampered_policy = policy.clone();
        report_tampered_policy.full_bootstrap_release_audit_package = Some(report_tampered_package);
        let error = report_tampered_policy
            .validate()
            .expect_err("policy must reject post-signature audit report byte mutations");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "full_bootstrap_release_audit_package");
        assert!(
            reason.contains("release audit report bytes digest mismatch"),
            "unexpected reason: {reason}"
        );

        let mut archive_tampered_package = package.clone();
        archive_tampered_package
            .audit_evidence_archive_bytes
            .extend_from_slice(b"; policy archive byte tamper after signing");
        let mut archive_tampered_policy = policy.clone();
        archive_tampered_policy.full_bootstrap_release_audit_package =
            Some(archive_tampered_package);
        let error = archive_tampered_policy
            .validate()
            .expect_err("policy must reject post-signature audit evidence archive byte mutations");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "full_bootstrap_release_audit_package");
        assert!(
            reason.contains("release audit evidence archive bytes digest mismatch"),
            "unexpected reason: {reason}"
        );

        let (generic_marker_package, generic_marker_package_digest) =
            sample_structural_full_bootstrap_release_audit_package_and_digest_with_marker_statements(
                &reviewer_keypair,
                b"external-review-approved: independent BFV full-bootstrap release audit report v1",
                b"external-review-evidence-archive: independent BFV full-bootstrap prover verifier evidence v1",
            );
        let mut generic_marker_policy = policy.clone();
        generic_marker_policy.full_bootstrap_release_audit_package = Some(generic_marker_package);
        generic_marker_policy.full_bootstrap_release_audit_package_digest =
            Some(generic_marker_package_digest);
        let error = generic_marker_policy
            .validate()
            .expect_err("policy must reject generic external-review marker statements");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "full_bootstrap_release_audit_package");
        assert!(
            reason.contains("signed reviewer id"),
            "unexpected reason: {reason}"
        );

        let machine_generated_params = ram_lfe_bfv_parameters_v1();
        let machine_generated_artifacts = sample_full_bootstrap_circuit_artifacts();
        let machine_generated_material = sample_full_bootstrap_material(&machine_generated_params);
        let (machine_generated_package, machine_generated_package_digest) =
            iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_package_and_digest_for_artifacts_v1(
                &machine_generated_params,
                &machine_generated_material,
                &machine_generated_artifacts,
                SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID,
                reviewer_keypair.private_key(),
            )
            .expect("machine-generated package remains structurally valid for deterministic evidence");
        let mut machine_generated_policy = policy.clone();
        machine_generated_policy.full_bootstrap_release_audit_package =
            Some(machine_generated_package);
        machine_generated_policy.full_bootstrap_release_audit_package_digest =
            Some(machine_generated_package_digest);
        let error = machine_generated_policy
            .validate()
            .expect_err("policy must reject machine-generated release-audit bodies");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));
        assert!(error.to_string().contains("machine-generated"));

        let mut stale_digest_policy = policy.clone();
        stale_digest_policy.full_bootstrap_release_audit_package_digest = Some(sample_hash(95));
        let error = stale_digest_policy
            .validate()
            .expect_err("policy must reject a release-audit package digest mismatch");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("does not match"));

        let mut zero_digest_policy = policy.clone();
        zero_digest_policy.full_bootstrap_release_audit_package_digest =
            Some(Hash::prehashed([0_u8; Hash::LENGTH]));
        let error = zero_digest_policy
            .validate()
            .expect_err("policy must reject a zero caller-pinned release-audit package digest");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "full_bootstrap_release_audit_package_digest");
        assert!(
            reason.contains("zero prehash sentinel"),
            "unexpected reason: {reason}"
        );

        for (label, placeholder_digest) in [
            ("direct", Hash::new(b"not production ready")),
            (
                "delayed",
                Hash::new(b"governed material digest before placeholder: template"),
            ),
            (
                "leading-whitespace delayed",
                Hash::new(
                    [
                        b" \n\t".as_slice(),
                        b"governed material digest before placeholder: ".as_slice(),
                        b"template".as_slice(),
                    ]
                    .concat(),
                ),
            ),
            (
                "binary-framed",
                Hash::new([b"\xff".as_slice(), b"mock"].concat()),
            ),
            (
                "binary-framed delayed",
                Hash::new(
                    [
                        b"governed material digest before placeholder: ".as_slice(),
                        b"\xff".as_slice(),
                        b"template".as_slice(),
                    ]
                    .concat(),
                ),
            ),
            (
                "leading-whitespace binary-framed delayed",
                Hash::new(
                    [
                        b" \n\t".as_slice(),
                        b"governed material digest before placeholder: ".as_slice(),
                        b"\xff".as_slice(),
                        b"template".as_slice(),
                    ]
                    .concat(),
                ),
            ),
            ("mock", Hash::new(b"mock")),
            (
                "delayed fixture",
                Hash::new(b"full-bootstrap material before placeholder: fixture"),
            ),
            (
                "separator-spelled native proof payload",
                Hash::new(b"p-l-a-c-e-h-o-l-d-e-r BFV full-bootstrap native proof key payload"),
            ),
            (
                "punctuated pending native proof payload",
                Hash::new(b"p.e.n.d.i.n.g BFV full-bootstrap native proof key payload"),
            ),
            (
                "underscore sample native proof payload",
                Hash::new(b"s_a_m_p_l_e BFV full-bootstrap native proof key payload"),
            ),
            (
                "delayed underscore template native proof payload",
                Hash::new(
                    [
                        b"governed material digest before placeholder: ".as_slice(),
                        b"t_e_m_p_l_a_t_e BFV full-bootstrap native proof key payload".as_slice(),
                    ]
                    .concat(),
                ),
            ),
        ] {
            let mut placeholder_digest_policy = policy.clone();
            placeholder_digest_policy.full_bootstrap_release_audit_package_digest =
                Some(placeholder_digest);
            let error = match placeholder_digest_policy.validate() {
                Ok(()) => panic!(
                    "policy must reject {label} placeholder caller-pinned release-audit package digest"
                ),
                Err(error) => error,
            };
            let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
                panic!("unexpected error: {error}");
            };
            assert_eq!(field, "full_bootstrap_release_audit_package_digest");
            assert!(
                reason.contains("placeholder full-bootstrap digest"),
                "unexpected reason: {reason}"
            );
        }

        let mut record_digest_alias_policy = policy.clone();
        record_digest_alias_policy.full_bootstrap_release_audit_package_digest =
            Some(package.record_digest);
        let error = record_digest_alias_policy
            .validate()
            .expect_err("policy must reject a package record digest as the pinned package digest");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("record digest"));

        let mut manifest_digest_alias_policy = policy.clone();
        manifest_digest_alias_policy.full_bootstrap_release_audit_package_digest =
            Some(package.manifest_digest);
        let error = manifest_digest_alias_policy.validate().expect_err(
            "policy must reject a package manifest digest as the pinned package digest",
        );
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("manifest digest"));

        let mut blank_reviewer_policy = policy.clone();
        blank_reviewer_policy.full_bootstrap_release_audit_trusted_reviewer_id =
            Some(" \t".to_string());
        let error = blank_reviewer_policy
            .validate()
            .expect_err("policy must reject a blank release-audit trusted reviewer id");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                field: "full_bootstrap_release_audit_trusted_reviewer_id",
                ..
            }
        ));

        let mut placeholder_reviewer_policy = policy.clone();
        placeholder_reviewer_policy.full_bootstrap_release_audit_trusted_reviewer_id =
            Some("not-production-ready-reviewer".to_string());
        let error = placeholder_reviewer_policy
            .validate()
            .expect_err("policy must reject placeholder release-audit trusted reviewer ids");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "full_bootstrap_release_audit_trusted_reviewer_id");
        assert!(
            reason.contains("placeholder"),
            "unexpected reason: {reason}"
        );

        let non_ed25519_reviewer_keypair = sample_bls_keypair(0x53);
        let mut non_ed25519_reviewer_policy = policy.clone();
        non_ed25519_reviewer_policy.full_bootstrap_release_audit_trusted_reviewer_public_key =
            Some(non_ed25519_reviewer_keypair.public_key().clone());
        let error = non_ed25519_reviewer_policy
            .validate()
            .expect_err("policy must reject non-Ed25519 release-audit trusted reviewer keys");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(
            field,
            "full_bootstrap_release_audit_trusted_reviewer_public_key"
        );
        assert!(reason.contains("Ed25519"), "unexpected reason: {reason}");

        let wrong_reviewer_keypair = sample_ed25519_keypair(0x52);
        let mut wrong_reviewer_policy = policy;
        wrong_reviewer_policy.full_bootstrap_release_audit_trusted_reviewer_public_key =
            Some(wrong_reviewer_keypair.public_key().clone());
        let error = wrong_reviewer_policy
            .validate()
            .expect_err("policy must reject a release-audit package signed by another reviewer");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_release_audit_package",
                ..
            }
        ));
        assert!(error.to_string().contains("reviewer"));
    }

    #[test]
    fn fhe_execution_policy_validate_requires_bootstrap_key_proof_statement_digest() {
        let mut missing_digest = sample_fhe_execution_policy();
        missing_digest.bootstrap_key_zero_refresh_proof_statement_digest = None;
        let error = missing_digest
            .validate()
            .expect_err("bootstrap-capable policies must bind bootstrap proof statement digest");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                ..
            }
        ));

        missing_digest.max_bootstrap_count = 0;
        missing_digest
            .validate()
            .expect("policies without bootstrap budget need no bootstrap proof statement");

        let mut full_bootstrap_statement = sample_fhe_execution_policy();
        full_bootstrap_statement.bootstrap_key_zero_refresh_proof_statement_digest = None;
        full_bootstrap_statement.full_bootstrap_material_proof_statement_digest =
            Some(sample_hash(93));
        full_bootstrap_statement.validate().expect(
            "full-bootstrap material proof statement can satisfy bootstrap-capable policy binding",
        );

        let mut ambiguous_statement = sample_fhe_execution_policy();
        ambiguous_statement.full_bootstrap_material_proof_statement_digest = Some(sample_hash(93));
        let error = ambiguous_statement
            .validate()
            .expect_err("policies must not bind both bootstrap statement classes");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_material_proof_statement_digest",
                ..
            }
        ));

        let mut stale_digest = sample_fhe_execution_policy();
        stale_digest.max_bootstrap_count = 0;
        let error = stale_digest
            .validate()
            .expect_err("policies without bootstrap budget must reject stale proof statements");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                ..
            }
        ));

        let mut stale_full_digest = full_bootstrap_statement;
        stale_full_digest.max_bootstrap_count = 0;
        let error = stale_full_digest.validate().expect_err(
            "policies without bootstrap budget must reject stale full-bootstrap proof statements",
        );
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_material_proof_statement_digest",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_zero_prehash_statement_sentinels() {
        let zero_statement = zero_prehash_statement_hash();

        let mut zero_refresh_statement = sample_fhe_execution_policy();
        zero_refresh_statement.bootstrap_key_zero_refresh_proof_statement_digest =
            Some(zero_statement);
        let error = zero_refresh_statement
            .validate()
            .expect_err("zero-refresh statement placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));

        let mut full_bootstrap_statement = sample_fhe_execution_policy();
        full_bootstrap_statement.bootstrap_key_zero_refresh_proof_statement_digest = None;
        full_bootstrap_statement.full_bootstrap_material_proof_statement_digest =
            Some(zero_statement);
        let error = full_bootstrap_statement
            .validate()
            .expect_err("full-bootstrap material statement placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_material_proof_statement_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));
    }

    #[test]
    fn fhe_execution_policy_validate_rejects_zero_prehash_key_digest_sentinels() {
        let zero_digest = zero_prehash_statement_hash();

        let mut evaluation_key_digest = sample_fhe_execution_policy();
        evaluation_key_digest.evaluation_key_digest = zero_digest;
        let error = evaluation_key_digest
            .validate()
            .expect_err("evaluation-key digest placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "evaluation_key_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));

        let mut refresh_transcript_digest = sample_fhe_execution_policy();
        refresh_transcript_digest.evaluation_key_refresh_transcript_digest = zero_digest;
        let error = refresh_transcript_digest
            .validate()
            .expect_err("refresh-transcript digest placeholder must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "evaluation_key_refresh_transcript_digest",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));
    }

    #[test]
    fn bfv_refresh_transcript_derives_public_key_proof_statement_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_, public_key, _) = iroha_crypto::fhe_bfv::keygen_from_seed(
            &params,
            b"soracloud-public-key-proof-statement-keygen",
        )
        .expect("keygen");
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: public_key.clone(),
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };

        let derived = transcript
            .public_key_proof_statement_digest(&params)
            .expect("derive exact public-key proof statement");
        let expected =
            iroha_crypto::fhe_bfv::bfv_public_key_proof_statement_digest(&params, &public_key)
                .expect("crypto public-key proof statement");
        assert_eq!(derived, expected);

        let bounded = transcript
            .public_key_proof_statement_digest_with_mode(
                &params,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("derive bounded public-key proof statement");
        let expected_bounded =
            iroha_crypto::fhe_bfv::bfv_bounded_noise_public_key_proof_statement_digest(
                &params,
                &public_key,
            )
            .expect("crypto bounded public-key proof statement");
        assert_eq!(bounded, expected_bounded);
        assert_ne!(
            derived, bounded,
            "exact and bounded public-key proof statements must stay domain-separated"
        );

        let mut malformed = transcript;
        let degree = usize::from(params.polynomial_degree);
        malformed.public_key = BfvPublicKey {
            b: vec![0; degree - 1],
            a: vec![0; degree],
        };
        let error = malformed
            .public_key_proof_statement_digest(&params)
            .expect_err("malformed transcript public keys must fail statement derivation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key",
                ..
            }
        ));

        let all_zero_key = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: BfvPublicKey {
                b: vec![0; degree],
                a: vec![0; degree],
            },
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };
        let error = all_zero_key
            .public_key_proof_statement_digest(&params)
            .expect_err("inert all-zero transcript public keys must fail statement derivation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key_proof_statement_digest",
                ..
            }
        ));
        assert!(
            error.to_string().contains("all zero"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bfv_refresh_transcript_derives_ciphertext_proof_statement_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_, public_key, _) = iroha_crypto::fhe_bfv::keygen_from_seed(
            &params,
            b"soracloud-ciphertext-proof-statement-keygen",
        )
        .expect("keygen");
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: public_key.clone(),
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };
        let ciphertext = iroha_crypto::fhe_bfv::encrypt_from_seed(
            &params,
            &public_key,
            &[7, 42],
            b"soracloud-ciphertext-proof-statement-encrypt",
        )
        .expect("encrypt ciphertext statement sample");
        let declared_bound =
            iroha_crypto::fhe_bfv::bfv_fresh_bounded_noise_ciphertext_bound(&params)
                .expect("fresh bounded-noise bound");

        let exact = transcript
            .ciphertext_proof_statement_digest(&params, &ciphertext, declared_bound)
            .expect("derive exact ciphertext proof statement");
        let expected_exact =
            iroha_crypto::fhe_bfv::bfv_ciphertext_exact_residual_proof_statement_digest(
                &params,
                &public_key,
                &ciphertext,
                declared_bound,
            )
            .expect("crypto exact ciphertext proof statement");
        assert_eq!(exact, expected_exact);

        let bounded = transcript
            .ciphertext_proof_statement_digest_with_mode(
                &params,
                &ciphertext,
                declared_bound,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("derive bounded ciphertext proof statement");
        let expected_bounded =
            iroha_crypto::fhe_bfv::bfv_bounded_noise_ciphertext_proof_statement_digest(
                &params,
                &public_key,
                &ciphertext,
                declared_bound,
            )
            .expect("crypto bounded ciphertext proof statement");
        assert_eq!(bounded, expected_bounded);
        assert_ne!(
            exact, bounded,
            "exact and bounded ciphertext proof statements must stay domain-separated"
        );

        let degree = usize::from(params.polynomial_degree);
        let all_zero = BfvCiphertext {
            c0: vec![0; degree],
            c1: vec![0; degree],
        };
        let error = transcript
            .ciphertext_proof_statement_digest(&params, &all_zero, 0)
            .expect_err("all-zero ciphertexts must fail statement derivation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_proof_statement_digest",
                ..
            }
        ));
        assert!(
            error.to_string().contains("all-zero ciphertext"),
            "unexpected error: {error}"
        );

        let all_zero_key_transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: BfvPublicKey {
                b: vec![0; degree],
                a: vec![0; degree],
            },
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };
        let error = all_zero_key_transcript
            .ciphertext_proof_statement_digest(&params, &ciphertext, declared_bound)
            .expect_err("inert all-zero transcript public keys must fail ciphertext statements");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_proof_statement_digest",
                ..
            }
        ));
        assert!(
            error.to_string().contains("all zero"),
            "unexpected error: {error}"
        );

        let error = transcript
            .ciphertext_proof_statement_digest(&params, &ciphertext, u128::MAX)
            .expect_err("impossible exact residual bounds must fail statement derivation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "ciphertext_proof_statement_digest",
                ..
            }
        ));
        assert!(
            error
                .to_string()
                .contains("exceeds centered residual capacity"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bfv_refresh_transcript_derives_bootstrap_key_proof_statement_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_secret_key, public_key, relinearization_key) =
            iroha_crypto::fhe_bfv::keygen_from_seed(
                &params,
                b"soracloud-bootstrap-proof-statement-keygen",
            )
            .expect("keygen");
        let bootstrap_seed = b"soracloud-bootstrap-proof-statement-bootstrap";
        let bootstrap_key = iroha_crypto::fhe_bfv::bootstrap_key_with_max_refresh_rounds_from_seed(
            &params,
            &public_key,
            "soracloud-bootstrap-proof",
            2,
            bootstrap_seed,
        )
        .expect("bootstrap key");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(bootstrap_key.clone()),
        };
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: public_key.clone(),
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: Some(BfvBootstrapRefreshTranscriptV1 {
                key_id: "soracloud-bootstrap-proof".to_string(),
                max_refresh_rounds: 2,
                seed: bootstrap_seed.to_vec(),
            }),
        };

        let derived = transcript
            .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::ExactLift,
            )
            .expect("derive bootstrap proof statement")
            .expect("bootstrap key is present");
        let expected = evaluation_keys
            .bootstrap_key_zero_refresh_proof_statement_digest_for_transcript(
                &params,
                &public_key,
                &[],
                Some(BfvBootstrapKeyTranscriptSeed {
                    key_id: "soracloud-bootstrap-proof",
                    max_refresh_rounds: 2,
                    seed: bootstrap_seed,
                }),
            )
            .expect("crypto transcript-bound bootstrap proof statement digest")
            .expect("bootstrap key is present");
        assert_eq!(derived, expected);
        let raw_key_only =
            iroha_crypto::fhe_bfv::bootstrap_key_zero_refresh_proof_statement_digest(
                &params,
                &public_key,
                &bootstrap_key,
            )
            .expect("raw bootstrap-key statement digest");
        assert_ne!(
            derived, raw_key_only,
            "Soracloud policies must bind the transcript inventory, not only bootstrap key bytes"
        );

        let (_bounded_secret_key, bounded_public_key, bounded_relinearization_key) =
            iroha_crypto::fhe_bfv::keygen_bounded_noise_with_relinearization_from_seed(
                &params,
                b"soracloud-bootstrap-proof-statement-bounded-keygen",
            )
            .expect("bounded keygen");
        let bounded_bootstrap_seed = b"soracloud-bootstrap-proof-statement-bounded-bootstrap";
        let bounded_bootstrap_key =
            iroha_crypto::fhe_bfv::bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed(
                &params,
                &bounded_public_key,
                "soracloud-bounded-bootstrap-proof",
                2,
                bounded_bootstrap_seed,
            )
            .expect("bounded bootstrap key");
        let bounded_evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key: bounded_relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(bounded_bootstrap_key),
        };
        let bounded_transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: bounded_public_key,
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: Some(BfvBootstrapRefreshTranscriptV1 {
                key_id: "soracloud-bounded-bootstrap-proof".to_string(),
                max_refresh_rounds: 2,
                seed: bounded_bootstrap_seed.to_vec(),
            }),
        };
        let bounded = bounded_transcript
            .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                &params,
                &bounded_evaluation_keys,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("derive bounded bootstrap proof statement")
            .expect("bootstrap key is present");
        assert_ne!(
            derived, bounded,
            "exact and bounded bootstrap proof statements must stay domain-separated"
        );

        let mut wrong_seed_transcript = transcript.clone();
        wrong_seed_transcript
            .bootstrap_transcript
            .as_mut()
            .expect("bootstrap transcript")
            .seed = b"soracloud-bootstrap-proof-statement-wrong-bootstrap".to_vec();
        let error = wrong_seed_transcript
            .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::ExactLift,
            )
            .expect_err("bootstrap transcript seed drift must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                ..
            }
        ));

        let mut drifted_evaluation_keys = evaluation_keys.clone();
        drifted_evaluation_keys.relinearization_key.entries[0].b[0] =
            drifted_evaluation_keys.relinearization_key.entries[0].b[0].wrapping_add(1)
                % params.ciphertext_modulus;
        let drifted_evaluation_key_digest = transcript
            .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                &params,
                &drifted_evaluation_keys,
                BfvRefreshTranscriptModeV1::ExactLift,
            )
            .expect("shape-valid evaluation-key drift still derives a statement")
            .expect("bootstrap key is present");
        assert_ne!(
            derived, drifted_evaluation_key_digest,
            "statement digest must bind the evaluation-key bundle digest"
        );

        let mut drifted_transcript = transcript;
        drifted_transcript
            .bootstrap_transcript
            .as_mut()
            .expect("bootstrap transcript")
            .key_id
            .push_str("-drift");
        let error = drifted_transcript
            .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::ExactLift,
            )
            .expect_err("bootstrap transcript metadata drift must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript",
                ..
            }
        ));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "digest binding test keeps adversarial cases inline"
    )]
    fn bfv_refresh_transcript_derives_full_bootstrap_material_proof_statement_digest() {
        let params = ram_lfe_bfv_parameters_v1();
        let (_, public_key, relinearization_key) = iroha_crypto::fhe_bfv::keygen_from_seed(
            &params,
            b"soracloud-full-bootstrap-proof-keygen",
        )
        .expect("keygen");
        let refresh_key = iroha_crypto::fhe_bfv::bootstrap_key_with_max_refresh_rounds_from_seed(
            &params,
            &public_key,
            "soracloud-full-bootstrap-proof",
            1,
            b"soracloud-full-bootstrap-proof-refresh",
        )
        .expect("bootstrap key");
        let full_bootstrap_key = iroha_crypto::fhe_bfv::full_bootstrap_key_from_material_v1(
            &params,
            &public_key,
            "soracloud-full-bootstrap-proof",
            sample_full_bootstrap_material(&params),
        )
        .expect("full-bootstrap key");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(full_bootstrap_key.clone()),
        };
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: public_key.clone(),
            rotation_transcripts: Vec::new(),
            bootstrap_transcript: None,
        };

        let derived = transcript
            .full_bootstrap_material_proof_statement_digest_for_evaluation_keys(
                &params,
                &evaluation_keys,
            )
            .expect("derive full-bootstrap proof statement")
            .expect("full-bootstrap statement is present");
        let expected = evaluation_keys
            .full_bootstrap_material_proof_statement_digest(&params, &public_key)
            .expect("crypto full-bootstrap proof statement")
            .expect("full-bootstrap statement is present");
        assert_eq!(derived, expected);
        assert_eq!(
            transcript
                .digest_for_evaluation_keys(&params, &evaluation_keys)
                .expect("derive full-bootstrap refresh transcript digest"),
            evaluation_keys
                .refresh_transcript_digest(&params, &public_key, &[], None)
                .expect("crypto full-bootstrap refresh transcript digest")
        );
        let (_, other_public_key, _) = iroha_crypto::fhe_bfv::keygen_from_seed(
            &params,
            b"soracloud-full-bootstrap-proof-other-public-key",
        )
        .expect("other keygen");
        let wrong_public_key_transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key: other_public_key,
            ..transcript.clone()
        };
        let error = wrong_public_key_transcript
            .digest_for_evaluation_keys(&params, &evaluation_keys)
            .expect_err("full-bootstrap refresh transcript digest must reject wrong public keys");
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "refresh_transcript");
        assert!(
            reason.contains("public-key digest"),
            "unexpected reason: {reason}"
        );
        let error = wrong_public_key_transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &evaluation_keys,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect_err(
                "bounded full-bootstrap refresh transcript digest must reject wrong public keys",
            );
        let SoracloudManifestError::InvalidField { field, reason, .. } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(field, "refresh_transcript");
        assert!(
            reason.contains("public-key digest"),
            "unexpected reason: {reason}"
        );

        let refresh_only_keys = BfvEvaluationKeyBundle {
            relinearization_key: relinearization_key.clone(),
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(refresh_key),
        };
        assert_eq!(
            transcript
                .full_bootstrap_material_proof_statement_digest_for_evaluation_keys(
                    &params,
                    &refresh_only_keys,
                )
                .expect("refresh-only full-bootstrap statement lookup validates"),
            None,
            "refresh-only bootstrap keys must not derive a full-bootstrap statement"
        );

        let mut drifted_material_keys = evaluation_keys.clone();
        drifted_material_keys
            .bootstrap_key
            .as_mut()
            .expect("bootstrap key")
            .full_bootstrap_material
            .as_mut()
            .expect("full-bootstrap material")
            .accumulator_digest = Hash::new(b"soracloud-full-bootstrap-proof-drifted-material");
        let drifted = transcript
            .full_bootstrap_material_proof_statement_digest_for_evaluation_keys(
                &params,
                &drifted_material_keys,
            )
            .expect("derive drifted full-bootstrap proof statement")
            .expect("full-bootstrap statement is present");
        assert_ne!(
            derived, drifted,
            "statement digest must bind the full-bootstrap material digest"
        );

        let mut drifted_proof_commitment_keys = evaluation_keys.clone();
        drifted_proof_commitment_keys
            .bootstrap_key
            .as_mut()
            .expect("bootstrap key")
            .full_bootstrap_material
            .as_mut()
            .expect("full-bootstrap material")
            .verifier_key_material_commitment =
            Hash::new(b"soracloud-full-bootstrap-proof-drifted-verifier-commitment");
        let drifted_proof_commitment = transcript
            .full_bootstrap_material_proof_statement_digest_for_evaluation_keys(
                &params,
                &drifted_proof_commitment_keys,
            )
            .expect("derive drifted proof commitment full-bootstrap proof statement")
            .expect("full-bootstrap statement is present");
        assert_ne!(
            derived, drifted_proof_commitment,
            "statement digest must bind the full-bootstrap proof-key material commitments"
        );

        let mut missing_material_key = full_bootstrap_key;
        missing_material_key.full_bootstrap_material = None;
        let missing_material_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: Some(missing_material_key),
        };
        let error = transcript
            .full_bootstrap_material_proof_statement_digest_for_evaluation_keys(
                &params,
                &missing_material_keys,
            )
            .expect_err("full-bootstrap mode without material must fail");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "full_bootstrap_material_proof_statement_digest",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_caps_match_crypto_key_caps() {
        assert_eq!(
            BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES, BFV_DETERMINISTIC_SEED_MAX_BYTES,
            "Soracloud transcript admission must share the BFV deterministic seed cap"
        );
        assert_eq!(
            BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES, BFV_BOOTSTRAP_KEY_ID_MAX_BYTES,
            "Soracloud transcript admission must share the BFV bootstrap key-id cap"
        );
        assert_eq!(
            BFV_REFRESH_TRANSCRIPT_MAX_ROTATION_TRANSCRIPTS, BFV_EVALUATION_KEY_MAX_ROTATION_KEYS,
            "Soracloud transcript admission must share the BFV rotation-key cap"
        );
        assert_eq!(
            BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS,
            BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS,
            "Soracloud transcript admission must share the BFV bootstrap round cap"
        );
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_empty_seeds() {
        let mut empty_rotation_seed = sample_bfv_refresh_transcript();
        empty_rotation_seed
            .rotation_transcripts
            .push(BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: Vec::new(),
            });
        let error = empty_rotation_seed
            .validate_seed_bounds()
            .expect_err("empty rotation transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                field: "rotation_transcripts.seed",
                ..
            }
        ));

        let mut empty_bootstrap_seed = sample_bfv_refresh_transcript();
        empty_bootstrap_seed.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-test-key".to_string(),
            max_refresh_rounds: 1,
            seed: Vec::new(),
        });
        let error = empty_bootstrap_seed
            .validate_seed_bounds()
            .expect_err("empty bootstrap transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                field: "bootstrap_transcript.seed",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_all_zero_seeds() {
        let mut all_zero_rotation_seed = sample_bfv_refresh_transcript();
        all_zero_rotation_seed
            .rotation_transcripts
            .push(BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: vec![0; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES],
            });
        let error = all_zero_rotation_seed
            .validate_seed_bounds()
            .expect_err("all-zero rotation transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_transcripts.seed",
                ..
            }
        ));

        let mut all_zero_bootstrap_seed = sample_bfv_refresh_transcript();
        all_zero_bootstrap_seed.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-test-key".to_string(),
            max_refresh_rounds: 1,
            seed: vec![0; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES],
        });
        let error = all_zero_bootstrap_seed
            .validate_seed_bounds()
            .expect_err("all-zero bootstrap transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.seed",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_unbounded_rotation_inventory() {
        let mut transcript = sample_bfv_refresh_transcript();
        transcript.rotation_transcripts = (0..=BFV_REFRESH_TRANSCRIPT_MAX_ROTATION_TRANSCRIPTS)
            .map(|index| BfvRotationRefreshTranscriptV1 {
                rotation_steps: u32::try_from(index + 1).expect("index fits u32"),
                seed: vec![0xA5],
            })
            .collect();
        let error = transcript
            .validate_seed_bounds()
            .expect_err("too many rotation transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_transcripts",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_malformed_rotation_steps() {
        let mut zero_rotation_steps = sample_bfv_refresh_transcript();
        zero_rotation_steps
            .rotation_transcripts
            .push(BfvRotationRefreshTranscriptV1 {
                rotation_steps: 0,
                seed: vec![0xA5],
            });
        let error = zero_rotation_steps
            .validate_seed_bounds()
            .expect_err("zero rotation transcript steps must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_transcripts.rotation_steps",
                ..
            }
        ));

        let mut duplicate_rotation_steps = sample_bfv_refresh_transcript();
        duplicate_rotation_steps.rotation_transcripts = vec![
            BfvRotationRefreshTranscriptV1 {
                rotation_steps: 7,
                seed: vec![0xA5],
            },
            BfvRotationRefreshTranscriptV1 {
                rotation_steps: 7,
                seed: vec![0x5A],
            },
        ];
        let error = duplicate_rotation_steps
            .validate_seed_bounds()
            .expect_err("duplicate rotation transcript steps must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_transcripts.rotation_steps",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_oversized_seeds() {
        let mut oversized_rotation_seed = sample_bfv_refresh_transcript();
        oversized_rotation_seed
            .rotation_transcripts
            .push(BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: vec![0xA5; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES + 1],
            });
        let error = oversized_rotation_seed
            .validate_seed_bounds()
            .expect_err("oversized rotation transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_transcripts.seed",
                ..
            }
        ));

        let mut oversized_bootstrap_seed = sample_bfv_refresh_transcript();
        oversized_bootstrap_seed.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-test-key".to_string(),
            max_refresh_rounds: 1,
            seed: vec![0xA5; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES + 1],
        });
        let error = oversized_bootstrap_seed
            .validate_seed_bounds()
            .expect_err("oversized bootstrap transcript seeds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.seed",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_malformed_bootstrap_key_ids() {
        let mut empty_key_id = sample_bfv_refresh_transcript();
        empty_key_id.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: String::new(),
            max_refresh_rounds: 1,
            seed: vec![0xA5],
        });
        let error = empty_key_id
            .validate_seed_bounds()
            .expect_err("empty bootstrap transcript key ids must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                field: "bootstrap_transcript.key_id",
                ..
            }
        ));

        let mut oversized_key_id = sample_bfv_refresh_transcript();
        oversized_key_id.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "k".repeat(BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES + 1),
            max_refresh_rounds: 1,
            seed: vec![0xA5],
        });
        let error = oversized_key_id
            .validate_seed_bounds()
            .expect_err("oversized bootstrap transcript key ids must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.key_id",
                ..
            }
        ));

        let mut padded_key_id = sample_bfv_refresh_transcript();
        padded_key_id.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: " bootstrap-test-key ".to_string(),
            max_refresh_rounds: 1,
            seed: vec![0xA5],
        });
        let error = padded_key_id
            .validate_seed_bounds()
            .expect_err("padded bootstrap transcript key ids must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.key_id",
                ..
            }
        ));

        let mut control_byte_key_id = sample_bfv_refresh_transcript();
        control_byte_key_id.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap\nkey".to_string(),
            max_refresh_rounds: 1,
            seed: vec![0xA5],
        });
        let error = control_byte_key_id
            .validate_seed_bounds()
            .expect_err("non-printable bootstrap transcript key ids must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.key_id",
                ..
            }
        ));

        let mut delimiter_key_id = sample_bfv_refresh_transcript();
        delimiter_key_id.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap/key".to_string(),
            max_refresh_rounds: 1,
            seed: vec![0xA5],
        });
        let error = delimiter_key_id
            .validate_seed_bounds()
            .expect_err("delimiter bootstrap transcript key ids must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.key_id",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_validate_rejects_malformed_bootstrap_round_counts() {
        let mut zero_rounds = sample_bfv_refresh_transcript();
        zero_rounds.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-test-key".to_string(),
            max_refresh_rounds: 0,
            seed: vec![0xA5],
        });
        let error = zero_rounds
            .validate_seed_bounds()
            .expect_err("zero bootstrap transcript rounds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.max_refresh_rounds",
                ..
            }
        ));

        let mut over_budget_rounds = sample_bfv_refresh_transcript();
        over_budget_rounds.bootstrap_transcript = Some(BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-test-key".to_string(),
            max_refresh_rounds: BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS
                .checked_add(1)
                .expect("test budget fits u16"),
            seed: vec![0xA5],
        });
        let error = over_budget_rounds
            .validate_seed_bounds()
            .expect_err("over-budget bootstrap transcript rounds must fail admission preflight");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_transcript.max_refresh_rounds",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_digest_rejects_public_key_shape_before_bundle_shape() {
        let params = iroha_crypto::fhe_bfv::BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 16_777_216,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let mut transcript = sample_bfv_refresh_transcript();
        transcript.public_key = BfvPublicKey {
            b: vec![0; 7],
            a: vec![0; 8],
        };
        let error = transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &sample_bfv_evaluation_key_bundle(),
                BfvRefreshTranscriptModeV1::ExactLift,
            )
            .expect_err("malformed transcript public keys must fail before bundle validation");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key",
                ..
            }
        ));
    }

    #[test]
    fn bfv_refresh_transcript_digest_uses_policy_mode() {
        let params = iroha_crypto::fhe_bfv::BfvParameters {
            polynomial_degree: 8,
            ciphertext_modulus: 4_294_967_296,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let (secret_key, public_key, relinearization_key) =
            iroha_crypto::fhe_bfv::keygen_bounded_noise_with_relinearization_from_seed(
                &params,
                b"soracloud-bounded-refresh-mode-keygen",
            )
            .expect("bounded-noise keygen");
        let rotation_seed = b"soracloud-bounded-refresh-mode-rotation".to_vec();
        let rotation_key = iroha_crypto::fhe_bfv::rotation_key_bounded_noise_from_seed(
            &params,
            &public_key,
            1,
            &rotation_seed,
        )
        .expect("bounded-noise rotation key");
        let bootstrap_seed = b"soracloud-bounded-refresh-mode-bootstrap".to_vec();
        let bootstrap_key =
            iroha_crypto::fhe_bfv::bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed(
                &params,
                &public_key,
                "soracloud-bounded-bootstrap",
                2,
                &bootstrap_seed,
            )
            .expect("bounded-noise bootstrap key");
        let bundle = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: vec![rotation_key],
            galois_keys: Vec::new(),
            bootstrap_key: Some(bootstrap_key),
        };
        bundle
            .validate_bounded_noise_zero_refreshes(&params, &secret_key)
            .expect("bounded-noise refresh masks decrypt to zero");
        let transcript = BfvEvaluationKeyRefreshTranscriptV1 {
            public_key,
            rotation_transcripts: vec![BfvRotationRefreshTranscriptV1 {
                rotation_steps: 1,
                seed: rotation_seed,
            }],
            bootstrap_transcript: Some(BfvBootstrapRefreshTranscriptV1 {
                key_id: "soracloud-bounded-bootstrap".to_string(),
                max_refresh_rounds: 2,
                seed: bootstrap_seed,
            }),
        };

        let bounded_digest = transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &bundle,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("bounded-noise transcript digest");
        let repeated_bounded_digest = transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &bundle,
                BfvRefreshTranscriptModeV1::BoundedNoise,
            )
            .expect("repeat bounded-noise transcript digest");
        assert_eq!(bounded_digest, repeated_bounded_digest);

        let err = transcript
            .digest_for_evaluation_keys(&params, &bundle)
            .expect_err("exact transcript digest must reject bounded-noise refresh masks");
        assert!(matches!(
            err,
            SoracloudManifestError::InvalidField {
                field: "refresh_transcript",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_for_param_set_rejects_withdrawn_param_set() {
        let mut param_set = sample_fhe_param_set();
        param_set.lifecycle = FheParamLifecycleV1::Withdrawn;
        let policy = sample_fhe_execution_policy();
        let error = policy
            .validate_for_param_set(&param_set)
            .expect_err("withdrawn parameter sets must reject new execution policy admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set.lifecycle",
                ..
            }
        ));
    }

    #[test]
    fn fhe_execution_policy_validate_for_param_set_rejects_adversarial_linkage() {
        let policy = sample_fhe_execution_policy();

        let mut wrong_param_name = sample_fhe_param_set();
        wrong_param_name.param_set = "fhe_bfv_other".parse().expect("valid name");
        let error = policy
            .validate_for_param_set(&wrong_param_name)
            .expect_err("policy must not bind to a different parameter set name");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set",
                ..
            }
        ));

        let mut wrong_version = sample_fhe_param_set();
        wrong_version.version = NonZeroU32::new(3).expect("nonzero");
        let error = policy
            .validate_for_param_set(&wrong_version)
            .expect_err("policy must not bind to a different parameter set version");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set_version",
                ..
            }
        ));

        let mut shallow_param_set = sample_fhe_param_set();
        shallow_param_set.max_multiplicative_depth = NonZeroU16::new(1).expect("nonzero");
        let error = policy
            .validate_for_param_set(&shallow_param_set)
            .expect_err("policy depth cannot exceed the parameter-set budget");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "max_multiplication_depth",
                ..
            }
        ));

        let mut proposed_param_set = sample_fhe_param_set();
        proposed_param_set.lifecycle = FheParamLifecycleV1::Proposed;
        proposed_param_set.deprecation_height = None;
        proposed_param_set.withdraw_height = None;
        let error = policy
            .validate_for_param_set(&proposed_param_set)
            .expect_err("proposed parameter sets are not executable");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set.lifecycle",
                ..
            }
        ));
    }

    #[test]
    fn fhe_governance_bundle_validate_accepts_consistent_payload() {
        let bundle = FheGovernanceBundleV1 {
            schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
            param_set: sample_fhe_param_set(),
            execution_policy: sample_fhe_execution_policy(),
        };
        assert!(
            bundle.validate_for_admission().is_ok(),
            "consistent FHE governance bundle must pass validation"
        );
    }

    #[test]
    fn fhe_governance_bundle_validate_requires_public_key_proof_statement_digest() {
        let mut policy = sample_fhe_execution_policy();
        policy.public_key_proof_statement_digest = None;
        let bundle = FheGovernanceBundleV1 {
            schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
            param_set: sample_fhe_param_set(),
            execution_policy: policy,
        };
        let error = bundle
            .validate_for_admission()
            .expect_err("production governance bundles must bind public-key proof statements");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "public_key_proof_statement_digest",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_rejects_duplicate_input_keys() {
        let mut job = sample_fhe_job_spec();
        job.inputs.push(FheJobInputRefV1 {
            state_key: "/state/health/patient-1".to_string(),
            payload_bytes: NonZeroU64::new(64).expect("nonzero"),
            commitment: sample_hash(123),
        });
        let error = job
            .validate()
            .expect_err("duplicate input state keys must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "inputs.state_key",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_rejects_zero_prehash_input_commitment_sentinel() {
        let mut job = sample_fhe_job_spec();
        job.inputs[0].commitment = zero_prehash_statement_hash();
        let error = job
            .validate()
            .expect_err("input commitment placeholder must fail job admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "inputs.commitment",
                ..
            }
        ));
        assert!(error.to_string().contains("zero prehash sentinel"));
    }

    #[test]
    fn fhe_job_spec_balanced_multiply_depth_matches_tree_shape() {
        assert!(bfv_balanced_multiplication_depth(0).is_err());
        assert_eq!(
            bfv_balanced_multiplication_depth(1).expect("single input"),
            0
        );
        assert_eq!(bfv_balanced_multiplication_depth(2).expect("two inputs"), 1);
        assert_eq!(
            bfv_balanced_multiplication_depth(3).expect("three inputs"),
            2
        );
        assert_eq!(
            bfv_balanced_multiplication_depth(4).expect("four inputs"),
            2
        );
        assert_eq!(
            bfv_balanced_multiplication_depth(5).expect("five inputs"),
            3
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fhe_job_spec_validate_rejects_adversarial_operation_shapes() {
        assert_fhe_job_invalid_field(
            "add jobs require at least two inputs",
            |job| {
                job.inputs.pop();
            },
            "inputs",
        );
        assert_fhe_job_invalid_field(
            "add jobs cannot smuggle multiplication depth",
            |job| job.requested_multiplication_depth = 1,
            "requested_multiplication_depth",
        );
        assert_fhe_job_invalid_field(
            "add depth metadata fails before input arity",
            |job| {
                job.inputs.pop();
                job.requested_multiplication_depth = 1;
            },
            "requested_multiplication_depth",
        );
        assert_fhe_job_invalid_field(
            "multiply jobs require non-zero depth",
            |job| job.operation = FheJobOperationV1::Multiply,
            "requested_multiplication_depth",
        );
        assert_fhe_job_invalid_field(
            "multiply zero-depth metadata fails before input arity",
            |job| {
                job.operation = FheJobOperationV1::Multiply;
                job.inputs.pop();
            },
            "requested_multiplication_depth",
        );
        assert_fhe_job_invalid_field(
            "multiply jobs must declare balanced depth",
            |job| {
                job.operation = FheJobOperationV1::Multiply;
                job.requested_multiplication_depth = 1;
                job.inputs.push(FheJobInputRefV1 {
                    state_key: "/state/health/patient-3".to_string(),
                    payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                    commitment: sample_hash(123),
                });
            },
            "requested_multiplication_depth",
        );
        assert_fhe_job_invalid_field(
            "multiply jobs cannot request rotations",
            |job| {
                job.operation = FheJobOperationV1::Multiply;
                job.requested_multiplication_depth = 1;
                job.rotation_steps = 1;
            },
            "operation",
        );
        assert_fhe_job_invalid_field(
            "rotate jobs require exactly one input",
            |job| {
                job.operation = FheJobOperationV1::RotateLeft;
                job.rotation_steps = 1;
            },
            "inputs",
        );
        assert_fhe_job_invalid_field(
            "rotate zero-step metadata fails before input shape",
            |job| {
                job.operation = FheJobOperationV1::RotateLeft;
            },
            "rotation_steps",
        );
        assert_fhe_job_invalid_field(
            "rotate jobs require non-zero rotation steps",
            |job| {
                job.operation = FheJobOperationV1::RotateLeft;
                job.inputs.truncate(1);
            },
            "rotation_steps",
        );
        assert_fhe_job_invalid_field(
            "bootstrap jobs require exactly one input",
            |job| {
                job.operation = FheJobOperationV1::Bootstrap;
                job.bootstrap_count = 1;
            },
            "inputs",
        );
        assert_fhe_job_invalid_field(
            "bootstrap jobs require non-zero bootstrap count",
            |job| {
                job.operation = FheJobOperationV1::Bootstrap;
                job.inputs.truncate(1);
            },
            "bootstrap_count",
        );
        assert_fhe_job_invalid_field(
            "zero-count bootstrap jobs fail before input shape",
            |job| {
                job.operation = FheJobOperationV1::Bootstrap;
                job.bootstrap_count = 0;
            },
            "bootstrap_count",
        );
        assert_fhe_job_invalid_field(
            "bootstrap depth metadata fails before input shape",
            |job| {
                job.operation = FheJobOperationV1::Bootstrap;
                job.bootstrap_count = 1;
                job.requested_multiplication_depth = 1;
            },
            "operation",
        );
    }

    #[test]
    fn fhe_job_spec_validate_rejects_adversarial_state_keys() {
        let mut relative_input = sample_fhe_job_spec();
        relative_input.inputs[0].state_key = "state/health/patient-1".to_string();
        let error = relative_input
            .validate()
            .expect_err("input state keys must be absolute");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "inputs.state_key",
                ..
            }
        ));

        let mut empty_input = sample_fhe_job_spec();
        empty_input.inputs[0].state_key = "   ".to_string();
        let error = empty_input
            .validate()
            .expect_err("blank input state keys must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::EmptyField {
                field: "inputs.state_key",
                ..
            }
        ));

        let mut relative_output = sample_fhe_job_spec();
        relative_output.output_state_key = "state/health/result-1".to_string();
        let error = relative_output
            .validate()
            .expect_err("output state keys must be absolute");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "output_state_key",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_rejects_policy_mismatch() {
        let mut job = sample_fhe_job_spec();
        job.policy_name = "fhe_policy_other".parse().expect("valid name");
        let error = job
            .validate_for_execution(&sample_fhe_execution_policy(), &sample_fhe_param_set())
            .expect_err("job with policy mismatch must fail execution admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "policy_name",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_rejects_adversarial_budget_abuse() {
        let policy = sample_fhe_execution_policy();
        let param_set = sample_fhe_param_set();

        let mut too_many_inputs = sample_fhe_job_spec();
        for input_index in 3..=9 {
            too_many_inputs.inputs.push(FheJobInputRefV1 {
                state_key: format!("/state/health/patient-{input_index}"),
                payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
                commitment: sample_hash(u8::try_from(120 + input_index).expect("fits")),
            });
        }
        let error = too_many_inputs
            .validate_for_execution(&policy, &param_set)
            .expect_err("jobs above policy input count must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "inputs",
                ..
            }
        ));

        let mut depth_overflow = sample_fhe_job_spec();
        depth_overflow.operation = FheJobOperationV1::Multiply;
        depth_overflow.requested_multiplication_depth = 3;
        let error = depth_overflow
            .validate_for_execution(&policy, &param_set)
            .expect_err("jobs above policy multiplication depth must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "requested_multiplication_depth",
                ..
            }
        ));

        let mut rotation_overflow = sample_fhe_job_spec();
        rotation_overflow.operation = FheJobOperationV1::RotateLeft;
        rotation_overflow.inputs.truncate(1);
        rotation_overflow.rotation_steps = 129;
        let error = rotation_overflow
            .validate_for_execution(&policy, &param_set)
            .expect_err("jobs above policy rotation budget must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "rotation_steps",
                ..
            }
        ));

        let mut bootstrap_overflow = sample_fhe_job_spec();
        bootstrap_overflow.operation = FheJobOperationV1::Bootstrap;
        bootstrap_overflow.inputs.truncate(1);
        bootstrap_overflow.bootstrap_count = 2;
        let error = bootstrap_overflow
            .validate_for_execution(&policy, &param_set)
            .expect_err("jobs above policy bootstrap budget must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "bootstrap_count",
                ..
            }
        ));

        let mut input_payload_overflow = sample_fhe_job_spec();
        input_payload_overflow.inputs[0].payload_bytes = NonZeroU64::new(131_073).expect("nonzero");
        let error = input_payload_overflow
            .validate_for_execution(&policy, &param_set)
            .expect_err("inputs above ciphertext size policy must fail admission");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "inputs.payload_bytes",
                ..
            }
        ));

        let mut tight_policy = sample_fhe_execution_policy();
        tight_policy.max_ciphertext_bytes = NonZeroU64::new(2_048).expect("nonzero");
        tight_policy.max_plaintext_bytes = NonZeroU64::new(1_024).expect("nonzero");
        let output_error = sample_fhe_job_spec()
            .validate_for_execution(&tight_policy, &param_set)
            .expect_err("deterministic output above ciphertext size policy must fail admission");
        assert!(matches!(
            output_error,
            SoracloudManifestError::InvalidField {
                field: "output_state_key",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_rejects_output_size_overflow() {
        let mut policy = sample_fhe_execution_policy();
        policy.max_ciphertext_bytes = NonZeroU64::new(u64::MAX).expect("nonzero");
        policy.max_plaintext_bytes = NonZeroU64::new(u64::MAX).expect("nonzero");

        let mut job = sample_fhe_job_spec();
        job.inputs[0].payload_bytes = NonZeroU64::new(u64::MAX).expect("nonzero");

        let projection_error = job
            .try_deterministic_output_payload_bytes()
            .expect_err("projected FHE output overflow must be reported");
        assert!(matches!(
            projection_error,
            SoracloudManifestError::InvalidField {
                field: "output_state_key",
                ..
            }
        ));
        assert_eq!(
            job.deterministic_output_payload_bytes(),
            u64::MAX,
            "infallible output projection must remain conservative"
        );

        let admission_error = job
            .validate_for_execution(&policy, &sample_fhe_param_set())
            .expect_err("overflowed FHE output projection must fail execution admission");
        assert!(matches!(
            admission_error,
            SoracloudManifestError::InvalidField {
                field: "output_state_key",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_rejects_adversarial_parameter_claims() {
        let policy = sample_fhe_execution_policy();
        let param_set = sample_fhe_param_set();

        let mut wrong_param_name = sample_fhe_job_spec();
        wrong_param_name.param_set = "fhe_bfv_other".parse().expect("valid name");
        let error = wrong_param_name
            .validate_for_execution(&policy, &param_set)
            .expect_err("job parameter name must match the execution policy");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set",
                ..
            }
        ));

        let mut wrong_param_version = sample_fhe_job_spec();
        wrong_param_version.param_set_version = NonZeroU32::new(3).expect("nonzero");
        let error = wrong_param_version
            .validate_for_execution(&policy, &param_set)
            .expect_err("job parameter version must match the execution policy");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "param_set_version",
                ..
            }
        ));
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_accepts_consistent_job() {
        let job = sample_fhe_job_spec();
        let policy = sample_fhe_execution_policy();
        let param_set = sample_fhe_param_set();
        assert!(
            job.validate_for_execution(&policy, &param_set).is_ok(),
            "consistent FHE job should pass execution admission checks"
        );
        assert!(
            job.deterministic_output_payload_bytes() > 0,
            "deterministic output size must be non-zero"
        );
    }

    #[test]
    fn fhe_job_spec_validate_for_execution_accepts_multi_input_add_and_multiply() {
        let policy = sample_fhe_execution_policy();
        let param_set = sample_fhe_param_set();
        let mut job = sample_fhe_job_spec();
        job.inputs.extend([
            FheJobInputRefV1 {
                state_key: "/state/health/patient-3".to_string(),
                payload_bytes: NonZeroU64::new(4_096).expect("nonzero"),
                commitment: sample_hash(123),
            },
            FheJobInputRefV1 {
                state_key: "/state/health/patient-4".to_string(),
                payload_bytes: NonZeroU64::new(1_024).expect("nonzero"),
                commitment: sample_hash(124),
            },
        ]);

        job.validate_for_execution(&policy, &param_set)
            .expect("four-input add job must be admitted within policy");
        assert_eq!(
            job.deterministic_output_payload_bytes(),
            4_096 + 16,
            "multi-input add output projection uses the largest input plus add overhead"
        );

        job.operation = FheJobOperationV1::Multiply;
        job.requested_multiplication_depth = 2;
        job.validate_for_execution(&policy, &param_set)
            .expect("four-input multiply job must be admitted within depth and input policy");
        assert_eq!(
            job.deterministic_output_payload_bytes(),
            4_096 + 128,
            "multiply output projection binds requested depth, not input count"
        );
    }

    #[test]
    fn fhe_job_spec_output_commitment_binds_multi_input_order() {
        let mut job = sample_fhe_job_spec();
        job.inputs.push(FheJobInputRefV1 {
            state_key: "/state/health/patient-3".to_string(),
            payload_bytes: NonZeroU64::new(2_048).expect("nonzero"),
            commitment: sample_hash(123),
        });
        let canonical = job.deterministic_output_commitment();

        job.inputs.swap(0, 2);
        let reordered = job.deterministic_output_commitment();

        assert_ne!(
            canonical, reordered,
            "multi-input FHE output commitments must bind input order"
        );
    }

    #[test]
    fn decryption_authority_policy_validate_rejects_unsorted_approvers() {
        let mut policy = sample_decryption_authority_policy();
        policy.approver_ids.swap(0, 1);
        let error = policy
            .validate()
            .expect_err("unsorted approver list must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "approver_ids",
                ..
            }
        ));
    }

    #[test]
    fn decryption_request_validate_for_policy_rejects_ttl_overflow() {
        let policy = sample_decryption_authority_policy();
        let mut request = sample_decryption_request();
        request.requested_ttl_blocks = NonZeroU32::new(2_000).expect("nonzero");
        let error = request
            .validate_for_policy(&policy)
            .expect_err("ttl overflow must be rejected");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "requested_ttl_blocks",
                ..
            }
        ));
    }

    #[test]
    fn decryption_request_validate_for_policy_rejects_break_glass_when_disabled() {
        let policy = sample_decryption_authority_policy();
        let mut request = sample_decryption_request();
        request.break_glass = true;
        request.break_glass_reason = Some("emergency access".to_string());
        let error = request
            .validate_for_policy(&policy)
            .expect_err("break-glass should fail when policy disallows it");
        assert!(matches!(
            error,
            SoracloudManifestError::InvalidField {
                field: "break_glass",
                ..
            }
        ));
    }

