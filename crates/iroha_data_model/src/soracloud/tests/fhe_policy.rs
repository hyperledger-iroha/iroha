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
    non_full_bootstrap_policy.full_bootstrap_release_audit_package_digest = Some(sample_hash(94));
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
    archive_tampered_policy.full_bootstrap_release_audit_package = Some(archive_tampered_package);
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
    machine_generated_policy.full_bootstrap_release_audit_package = Some(machine_generated_package);
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
    let error = manifest_digest_alias_policy
        .validate()
        .expect_err("policy must reject a package manifest digest as the pinned package digest");
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
fn fhe_execution_policy_validate_requires_exactly_one_bootstrap_mode() {
    let mut missing_mode = sample_fhe_execution_policy();
    missing_mode.bootstrap_key_zero_refresh_proof_statement_digest = None;
    let error = missing_mode
        .validate()
        .expect_err("bootstrap-capable policies must select a governed bootstrap mode");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "bootstrap_key_zero_refresh_proof_statement_digest",
            ..
        }
    ));
    missing_mode.max_bootstrap_count = 0;
    missing_mode
        .validate()
        .expect("policies without bootstrap budget need no bootstrap mode");
    let reviewer_keypair = sample_ed25519_keypair(0x5A);
    let (package, package_digest) =
        sample_full_bootstrap_release_audit_package_and_digest(&reviewer_keypair);
    let mut full_bootstrap = sample_fhe_execution_policy();
    full_bootstrap.bootstrap_key_zero_refresh_proof_statement_digest = None;
    full_bootstrap.full_bootstrap_release_audit_package = Some(package);
    full_bootstrap.full_bootstrap_release_audit_package_digest = Some(package_digest);
    full_bootstrap.full_bootstrap_release_audit_trusted_reviewer_id =
        Some(SAMPLE_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID.to_string());
    full_bootstrap.full_bootstrap_release_audit_trusted_reviewer_public_key =
        Some(reviewer_keypair.public_key().clone());
    full_bootstrap
        .validate()
        .expect("governed release-audited material is the full-bootstrap mode");
    let mut ambiguous = full_bootstrap.clone();
    ambiguous.bootstrap_key_zero_refresh_proof_statement_digest = Some(sample_hash(92));
    let error = ambiguous
        .validate()
        .expect_err("policies must not select both bootstrap modes");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "full_bootstrap_release_audit_package",
            ..
        }
    ));
    full_bootstrap.max_bootstrap_count = 0;
    let error = full_bootstrap
        .validate()
        .expect_err("policies without bootstrap budget must reject governed material");
    assert!(matches!(
        error,
        SoracloudManifestError::InvalidField {
            field: "full_bootstrap_release_audit_package",
            ..
        }
    ));
}
#[test]
fn fhe_execution_policy_validate_rejects_zero_prehash_statement_sentinels() {
    let zero_statement = zero_prehash_statement_hash();
    let mut zero_refresh_statement = sample_fhe_execution_policy();
    zero_refresh_statement.bootstrap_key_zero_refresh_proof_statement_digest = Some(zero_statement);
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
    let declared_bound = iroha_crypto::fhe_bfv::bfv_fresh_bounded_noise_ciphertext_bound(&params)
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
    let (_secret_key, public_key, relinearization_key) = iroha_crypto::fhe_bfv::keygen_from_seed(
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
    let raw_key_only = iroha_crypto::fhe_bfv::bootstrap_key_zero_refresh_proof_statement_digest(
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
        BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS, BFV_BOOTSTRAP_KEY_MAX_REFRESH_ROUNDS,
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
#[allow(clippy::too_many_lines)]
fn sample_governed_fhe_material_for_lifecycle(
    version: NonZeroU32,
) -> SoracloudFheGovernedMaterialV1 {
    let params = ram_lfe_bfv_parameters_v1();
    let (_secret_key, public_key, relinearization_key) =
        keygen_from_seed(&params, b"soracloud-governed-material-lifecycle")
            .expect("deterministic governed-material key generation");
    let evaluation_keys = BfvEvaluationKeyBundle {
        relinearization_key,
        rotation_keys: Vec::new(),
        galois_keys: Vec::new(),
        bootstrap_key: None,
    };
    let evaluation_key_refresh_transcript = BfvEvaluationKeyRefreshTranscriptV1 {
        public_key,
        rotation_transcripts: Vec::new(),
        bootstrap_transcript: None,
    };
    let evaluation_key_digest = evaluation_keys
        .digest(&params)
        .expect("governed evaluation-key digest");
    let evaluation_key_refresh_transcript_digest = evaluation_key_refresh_transcript
        .digest_for_evaluation_keys_with_mode(
            &params,
            &evaluation_keys,
            BfvRefreshTranscriptModeV1::ExactLift,
        )
        .expect("governed refresh-transcript digest");
    let public_key_proof_statement_digest = evaluation_key_refresh_transcript
        .public_key_proof_statement_digest_with_mode(&params, BfvRefreshTranscriptModeV1::ExactLift)
        .expect("governed public-key proof statement");
    let param_set = FheParamSetV1 {
        schema_version: FHE_PARAM_SET_VERSION_V1,
        param_set: sample_name("bfv_governed_v1"),
        version: NonZeroU32::new(1).expect("nonzero"),
        backend: REGISTERED_SORACLOUD_BFV_BACKEND_V1.to_string(),
        scheme: FheSchemeV1::Bfv,
        ciphertext_modulus_bits: vec![
            NonZeroU16::new(53).expect("nonzero"),
            NonZeroU16::new(52).expect("nonzero"),
        ],
        plaintext_modulus_bits: NonZeroU16::new(9).expect("nonzero"),
        polynomial_modulus_degree: NonZeroU32::new(u32::from(params.polynomial_degree))
            .expect("nonzero"),
        slot_count: NonZeroU32::new(u32::from(params.polynomial_degree)).expect("nonzero"),
        security_level_bits: NonZeroU16::new(128).expect("nonzero"),
        max_multiplicative_depth: NonZeroU16::new(1).expect("nonzero"),
        lifecycle: FheParamLifecycleV1::Active,
        activation_height: Some(1),
        withdraw_height: None,
        parameter_digest: iroha_crypto::fhe_bfv::registered_bfv_parameter_digest(&params)
            .expect("registered parameter digest"),
        rns_modulus_chain_digest: iroha_crypto::fhe_bfv::registered_bfv_rns_modulus_chain_digest(
            &params,
        )
        .expect("registered RNS digest"),
        key_switch_decomposition_chain_digest:
            iroha_crypto::fhe_bfv::registered_bfv_key_switch_decomposition_chain_digest(&params)
                .expect("registered decomposition digest"),
    };
    let policy_name = sample_name("governed_analytics");
    let policy = FheExecutionPolicyV1 {
        schema_version: FHE_EXECUTION_POLICY_VERSION_V1,
        policy_name: policy_name.clone(),
        param_set: param_set.param_set.clone(),
        param_set_version: param_set.version,
        evaluation_key_digest,
        evaluation_key_refresh_transcript_digest,
        refresh_transcript_mode: BfvRefreshTranscriptModeV1::ExactLift,
        public_key_proof_statement_digest: Some(public_key_proof_statement_digest),
        bootstrap_key_zero_refresh_proof_statement_digest: None,
        full_bootstrap_release_audit_package: None,
        full_bootstrap_release_audit_package_digest: None,
        full_bootstrap_release_audit_trusted_reviewer_id: None,
        full_bootstrap_release_audit_trusted_reviewer_public_key: None,
        max_ciphertext_bytes: NonZeroU64::new(131_072).expect("nonzero"),
        max_plaintext_bytes: NonZeroU64::new(512).expect("nonzero"),
        max_input_ciphertexts: NonZeroU16::new(4).expect("nonzero"),
        max_output_ciphertexts: NonZeroU16::new(1).expect("nonzero"),
        max_multiplication_depth: NonZeroU16::new(1).expect("nonzero"),
        max_rotation_count: NonZeroU32::new(1).expect("nonzero"),
        max_bootstrap_count: 0,
        rounding_mode: FheDeterministicRoundingModeV1::NearestTiesToEven,
    };
    let mut material = SoracloudFheGovernedMaterialV1 {
        schema_version: SORACLOUD_FHE_GOVERNED_MATERIAL_VERSION_V1,
        service_name: sample_name("governed_service"),
        policy_name,
        version,
        governance_bundle: FheGovernanceBundleV1 {
            schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
            param_set,
            execution_policy: policy,
        },
        evaluation_keys,
        evaluation_key_refresh_transcript,
        full_bootstrap_circuit_artifacts: None,
        material_digest: Hash::new(b"pending governed material digest"),
    };
    material.material_digest = material
        .computed_material_digest()
        .expect("compute governed material digest");
    material
}
#[test]
fn fhe_param_set_norito_rejects_retired_deprecation_layout() {
    #[derive(Clone, Copy, Encode)]
    enum RetiredFheParamLifecycleV1 {
        Proposed,
        Active,
        Deprecated,
        Withdrawn,
    }
    #[derive(Encode)]
    struct RetiredFheParamSetV1 {
        schema_version: u16,
        param_set: Name,
        version: NonZeroU32,
        backend: String,
        scheme: FheSchemeV1,
        ciphertext_modulus_bits: Vec<NonZeroU16>,
        plaintext_modulus_bits: NonZeroU16,
        polynomial_modulus_degree: NonZeroU32,
        slot_count: NonZeroU32,
        security_level_bits: NonZeroU16,
        max_multiplicative_depth: NonZeroU16,
        lifecycle: RetiredFheParamLifecycleV1,
        activation_height: Option<u64>,
        deprecation_height: Option<u64>,
        withdraw_height: Option<u64>,
        parameter_digest: Hash,
        rns_modulus_chain_digest: Hash,
        key_switch_decomposition_chain_digest: Hash,
    }

    for (label, lifecycle, activation_height, deprecation_height, withdraw_height) in [
        (
            "proposed",
            RetiredFheParamLifecycleV1::Proposed,
            None,
            None,
            None,
        ),
        (
            "active",
            RetiredFheParamLifecycleV1::Active,
            Some(10_000),
            None,
            Some(40_000),
        ),
        (
            "deprecated",
            RetiredFheParamLifecycleV1::Deprecated,
            Some(10_000),
            Some(20_000),
            Some(40_000),
        ),
        (
            "withdrawn",
            RetiredFheParamLifecycleV1::Withdrawn,
            Some(10_000),
            Some(20_000),
            Some(40_000),
        ),
    ] {
        let canonical = sample_fhe_param_set();
        let retired = RetiredFheParamSetV1 {
            schema_version: canonical.schema_version,
            param_set: canonical.param_set,
            version: canonical.version,
            backend: canonical.backend,
            scheme: canonical.scheme,
            ciphertext_modulus_bits: canonical.ciphertext_modulus_bits,
            plaintext_modulus_bits: canonical.plaintext_modulus_bits,
            polynomial_modulus_degree: canonical.polynomial_modulus_degree,
            slot_count: canonical.slot_count,
            security_level_bits: canonical.security_level_bits,
            max_multiplicative_depth: canonical.max_multiplicative_depth,
            lifecycle,
            activation_height,
            deprecation_height,
            withdraw_height,
            parameter_digest: canonical.parameter_digest,
            rns_modulus_chain_digest: canonical.rns_modulus_chain_digest,
            key_switch_decomposition_chain_digest: canonical.key_switch_decomposition_chain_digest,
        };
        let bytes = retired.encode();
        assert!(
            FheParamSetV1::decode_all(&mut bytes.as_slice()).is_err(),
            "first-release FHE parameter sets must reject the retired {label} deprecation layout"
        );
    }
}
#[cfg(feature = "json")]
#[test]
#[allow(clippy::too_many_lines)]
fn fhe_canonical_model_v1_json_is_closed_and_requires_explicit_keys() {
    use iroha_crypto::fhe_bfv::{
        BfvBootstrapKey, BfvBootstrapKeyMode, BfvCiphertext,
        BfvFullBootstrapCircuitArtifactBundleV1, BfvFullBootstrapCircuitArtifactRoleV1,
        BfvFullBootstrapCircuitMaterialV1, BfvFullBootstrapReleaseAuditEvidenceV1,
        BfvFullBootstrapReleaseAuditKeyEvidenceV1, BfvFullBootstrapReleaseAuditManifestV1,
        BfvFullBootstrapReleaseAuditPackageV1, BfvFullBootstrapReleaseAuditProofProfileV1,
        BfvFullBootstrapReleaseAuditRecordV1, BfvFullBootstrapReleaseAuditSignoffPayloadV1,
        BfvFullBootstrapReleaseAuditSignoffV1, BfvFullBootstrapReleaseAuditVerdictV1, BfvGaloisKey,
        BfvRelinearizationKey, BfvRelinearizationKeyEntry, BfvRotationKey,
    };

    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value =
                norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong error: {error:?}",
                $label
            );
        }};
    }
    macro_rules! assert_required_fields {
        (
            $value:expr,
            $ty:ty,
            [$($field:literal),+ $(,)?],
            [$($nullable:literal),* $(,)?],
            $label:literal
        ) => {{
            let canonical =
                norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            norito::json::from_value::<$ty>(canonical.clone())
                .expect(concat!("canonical ", $label, " must decode"));
            $(
                assert!(
                    canonical.get($field).is_some(),
                    "canonical {} must emit `{}`",
                    $label,
                    $field
                );
                let mut missing = canonical.clone();
                assert!(
                    missing
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .remove($field)
                        .is_some()
                );
                norito::json::from_value::<$ty>(missing).expect_err(concat!(
                    $label,
                    " must reject an omitted canonical key"
                ));
            )+
            $(
                assert!(
                    matches!(canonical.get($nullable), Some(norito::json::Value::Null)),
                    "canonical {} must emit `{}` as null",
                    $label,
                    $nullable
                );
            )*
        }};
    }

    let mut param_set = sample_fhe_param_set();
    param_set.ciphertext_modulus_bits.clear();
    param_set.activation_height = None;
    param_set.withdraw_height = None;
    let mut refresh_transcript = sample_bfv_refresh_transcript();
    refresh_transcript.rotation_transcripts.clear();
    refresh_transcript.bootstrap_transcript = None;
    let mut execution_policy = sample_fhe_execution_policy();
    execution_policy.public_key_proof_statement_digest = None;
    execution_policy.bootstrap_key_zero_refresh_proof_statement_digest = None;
    execution_policy.full_bootstrap_release_audit_package = None;
    execution_policy.full_bootstrap_release_audit_package_digest = None;
    execution_policy.full_bootstrap_release_audit_trusted_reviewer_id = None;
    execution_policy.full_bootstrap_release_audit_trusted_reviewer_public_key = None;
    let governance_bundle = FheGovernanceBundleV1 {
        schema_version: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
        param_set: param_set.clone(),
        execution_policy: execution_policy.clone(),
    };
    let governed_material =
        sample_governed_fhe_material_for_lifecycle(NonZeroU32::new(1).expect("nonzero"));
    let policy_reference = governed_material.policy_reference();
    let permission_scope = SoracloudFheGovernancePermissionScopeV1 {
        schema_version: SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
        service_name: governed_material.service_name.clone(),
        policy_name: governed_material.policy_name.clone(),
    };
    let version_state = SoracloudFhePolicyVersionStateV1 {
        material: governed_material.clone(),
        admitted_by_transaction_hash: sample_hash(221),
        lifecycle: SoracloudFhePolicyVersionLifecycleV1::Active,
        deactivated_by_transaction_hash: None,
    };
    let policy_record = SoracloudFhePolicyRecordV1 {
        schema_version: SORACLOUD_FHE_POLICY_RECORD_VERSION_V1,
        service_name: governed_material.service_name.clone(),
        policy_name: governed_material.policy_name.clone(),
        active_version: None,
        versions: BTreeMap::new(),
    };
    let mut input_admission_proof = sample_fhe_input_admission_proof();
    input_admission_proof.public_key = None;
    input_admission_proof
        .ciphertext_proof_statement_digests
        .clear();
    let public_key_proof = sample_fhe_public_key_proof();
    let bootstrap_key_proof = sample_fhe_bootstrap_key_proof();
    let full_bootstrap_execution_proof = sample_fhe_full_bootstrap_execution_proof();
    let mut secret = sample_secret_envelope();
    secret.aad_digest = None;
    let mut ciphertext_state = sample_ciphertext_state_record();
    ciphertext_state.metadata.policy_tag = None;
    ciphertext_state.metadata.tags.clear();
    let ciphertext_metadata = ciphertext_state.metadata.clone();
    let mut job_spec = sample_fhe_job_spec();
    job_spec.inputs.clear();
    let job_input = FheJobInputRefV1 {
        state_key: "/state/private/input".to_owned(),
        payload_bytes: NonZeroU64::new(1).expect("nonzero"),
        commitment: sample_hash(222),
    };
    let mut decryption_policy = sample_decryption_authority_policy();
    decryption_policy.approver_ids.clear();
    let mut decryption_request = sample_decryption_request();
    decryption_request.consent_evidence_hash = None;
    decryption_request.break_glass_reason = None;
    let query_spec = sample_ciphertext_query_spec();
    let mut query_response = sample_ciphertext_query_response();
    let mut query_item = query_response.results[0].clone();
    let inclusion_proof = query_item.proof.clone().expect("sample inclusion proof");
    query_item.state_key = None;
    query_item.proof = None;
    query_response.results.clear();
    let container = sample_container();
    let mut service = sample_service(vec![sample_binding("private_state")]);
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    let deployment_bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };

    let mut retired_param_set =
        norito::json::to_value(&param_set).expect("serialize FHE parameter set");
    retired_param_set
        .as_object_mut()
        .expect("FHE parameter-set JSON object")
        .insert("deprecation_height".to_owned(), norito::json!(20_000));
    norito::json::from_value::<FheParamSetV1>(retired_param_set)
        .expect_err("retired FHE deprecation_height must be rejected");

    let mut retired_lifecycle = norito::json::to_value(&FheParamLifecycleV1::Active)
        .expect("serialize FHE parameter lifecycle");
    *retired_lifecycle
        .get_mut("lifecycle")
        .expect("FHE lifecycle discriminator") = norito::json!("Deprecated");
    norito::json::from_value::<FheParamLifecycleV1>(retired_lifecycle)
        .expect_err("retired FHE Deprecated lifecycle must be rejected");

    assert_unknown_rejected!(FheSchemeV1::Bfv, FheSchemeV1, "FHE scheme");
    assert_unknown_rejected!(
        FheParamLifecycleV1::Active,
        FheParamLifecycleV1,
        "FHE parameter lifecycle"
    );
    assert_unknown_rejected!(param_set, FheParamSetV1, "FHE parameter set");
    assert_unknown_rejected!(
        FheDeterministicRoundingModeV1::NearestTiesToEven,
        FheDeterministicRoundingModeV1,
        "FHE rounding mode"
    );
    assert_unknown_rejected!(
        BfvRefreshTranscriptModeV1::ExactLift,
        BfvRefreshTranscriptModeV1,
        "BFV refresh transcript mode"
    );
    assert_unknown_rejected!(
        BfvCiphertextBoundModeV1::ExactResidualMultiple,
        BfvCiphertextBoundModeV1,
        "BFV ciphertext bound mode"
    );
    assert_unknown_rejected!(
        BfvRotationRefreshTranscriptV1 {
            rotation_steps: 1,
            seed: Vec::new(),
        },
        BfvRotationRefreshTranscriptV1,
        "BFV rotation refresh transcript"
    );
    assert_unknown_rejected!(
        BfvBootstrapRefreshTranscriptV1 {
            key_id: "bootstrap-v1".to_owned(),
            max_refresh_rounds: 1,
            seed: Vec::new(),
        },
        BfvBootstrapRefreshTranscriptV1,
        "BFV bootstrap refresh transcript"
    );
    assert_unknown_rejected!(
        refresh_transcript,
        BfvEvaluationKeyRefreshTranscriptV1,
        "BFV evaluation-key refresh transcript"
    );
    assert_unknown_rejected!(
        execution_policy,
        FheExecutionPolicyV1,
        "FHE execution policy"
    );
    assert_unknown_rejected!(
        governance_bundle,
        FheGovernanceBundleV1,
        "FHE governance bundle"
    );
    assert_unknown_rejected!(
        policy_reference,
        SoracloudFhePolicyReferenceV1,
        "FHE policy reference"
    );
    assert_unknown_rejected!(
        permission_scope,
        SoracloudFheGovernancePermissionScopeV1,
        "FHE governance permission scope"
    );
    assert_unknown_rejected!(
        governed_material,
        SoracloudFheGovernedMaterialV1,
        "governed FHE material"
    );
    assert_unknown_rejected!(
        SoracloudFhePolicyVersionLifecycleV1::Active,
        SoracloudFhePolicyVersionLifecycleV1,
        "FHE policy version lifecycle"
    );
    assert_unknown_rejected!(
        version_state,
        SoracloudFhePolicyVersionStateV1,
        "FHE policy version state"
    );
    assert_unknown_rejected!(
        policy_record,
        SoracloudFhePolicyRecordV1,
        "FHE policy record"
    );
    assert_unknown_rejected!(
        input_admission_proof,
        SoracloudFheInputAdmissionProofV1,
        "FHE input admission proof"
    );
    assert_unknown_rejected!(
        public_key_proof,
        SoracloudFhePublicKeyProofV1,
        "FHE public-key proof"
    );
    assert_unknown_rejected!(
        bootstrap_key_proof,
        SoracloudFheBootstrapKeyProofV1,
        "FHE bootstrap-key proof"
    );
    assert_unknown_rejected!(
        full_bootstrap_execution_proof,
        SoracloudFheFullBootstrapExecutionProofV1,
        "FHE full-bootstrap execution proof"
    );
    assert_unknown_rejected!(
        SecretEnvelopeEncryptionV1::FheCiphertext,
        SecretEnvelopeEncryptionV1,
        "secret-envelope encryption mode"
    );
    assert_unknown_rejected!(secret, SecretEnvelopeV1, "secret envelope");
    assert_unknown_rejected!(
        ciphertext_metadata,
        CiphertextStateMetadataV1,
        "ciphertext state metadata"
    );
    assert_unknown_rejected!(
        ciphertext_state,
        CiphertextStateRecordV1,
        "ciphertext state record"
    );
    assert_unknown_rejected!(
        FheJobOperationV1::Add,
        FheJobOperationV1,
        "FHE job operation"
    );
    assert_unknown_rejected!(job_input, FheJobInputRefV1, "FHE job input");
    assert_unknown_rejected!(job_spec, FheJobSpecV1, "FHE job spec");
    assert_unknown_rejected!(
        DecryptionAuthorityModeV1::ThresholdService,
        DecryptionAuthorityModeV1,
        "decryption authority mode"
    );
    assert_unknown_rejected!(
        decryption_policy,
        DecryptionAuthorityPolicyV1,
        "decryption authority policy"
    );
    assert_unknown_rejected!(
        decryption_request,
        DecryptionRequestV1,
        "decryption request"
    );
    assert_unknown_rejected!(
        CiphertextQueryMetadataLevelV1::Minimal,
        CiphertextQueryMetadataLevelV1,
        "ciphertext query metadata level"
    );
    assert_unknown_rejected!(query_spec, CiphertextQuerySpecV1, "ciphertext query spec");
    assert_unknown_rejected!(
        inclusion_proof,
        CiphertextInclusionProofV1,
        "ciphertext inclusion proof"
    );
    assert_unknown_rejected!(
        query_item,
        CiphertextQueryResultItemV1,
        "ciphertext query item"
    );
    assert_unknown_rejected!(
        query_response,
        CiphertextQueryResponseV1,
        "ciphertext query response"
    );
    assert_unknown_rejected!(
        deployment_bundle,
        SoraDeploymentBundleV1,
        "Sora deployment bundle"
    );

    assert_required_fields!(
        param_set,
        FheParamSetV1,
        [
            "ciphertext_modulus_bits",
            "activation_height",
            "withdraw_height",
        ],
        ["activation_height", "withdraw_height"],
        "FHE parameter set"
    );
    assert_required_fields!(
        refresh_transcript,
        BfvEvaluationKeyRefreshTranscriptV1,
        ["rotation_transcripts", "bootstrap_transcript"],
        ["bootstrap_transcript"],
        "BFV evaluation-key refresh transcript"
    );
    assert_required_fields!(
        execution_policy,
        FheExecutionPolicyV1,
        [
            "refresh_transcript_mode",
            "public_key_proof_statement_digest",
            "bootstrap_key_zero_refresh_proof_statement_digest",
            "full_bootstrap_release_audit_package",
            "full_bootstrap_release_audit_package_digest",
            "full_bootstrap_release_audit_trusted_reviewer_id",
            "full_bootstrap_release_audit_trusted_reviewer_public_key",
        ],
        [
            "public_key_proof_statement_digest",
            "bootstrap_key_zero_refresh_proof_statement_digest",
            "full_bootstrap_release_audit_package",
            "full_bootstrap_release_audit_package_digest",
            "full_bootstrap_release_audit_trusted_reviewer_id",
            "full_bootstrap_release_audit_trusted_reviewer_public_key",
        ],
        "FHE execution policy"
    );
    assert_required_fields!(
        governed_material,
        SoracloudFheGovernedMaterialV1,
        ["full_bootstrap_circuit_artifacts"],
        ["full_bootstrap_circuit_artifacts"],
        "governed FHE material"
    );
    assert_required_fields!(
        version_state,
        SoracloudFhePolicyVersionStateV1,
        ["deactivated_by_transaction_hash"],
        ["deactivated_by_transaction_hash"],
        "FHE policy version state"
    );
    assert_required_fields!(
        policy_record,
        SoracloudFhePolicyRecordV1,
        ["active_version", "versions"],
        ["active_version"],
        "FHE policy record"
    );
    assert_required_fields!(
        input_admission_proof,
        SoracloudFheInputAdmissionProofV1,
        [
            "public_key",
            "ciphertext_proof_statement_digests",
            "bound_mode",
        ],
        ["public_key"],
        "FHE input admission proof"
    );
    assert_required_fields!(
        secret,
        SecretEnvelopeV1,
        ["aad_digest"],
        ["aad_digest"],
        "secret envelope"
    );
    assert_required_fields!(
        ciphertext_metadata,
        CiphertextStateMetadataV1,
        ["policy_tag", "tags"],
        ["policy_tag"],
        "ciphertext state metadata"
    );
    assert_required_fields!(job_spec, FheJobSpecV1, ["inputs"], [], "FHE job spec");
    assert_required_fields!(
        decryption_policy,
        DecryptionAuthorityPolicyV1,
        ["approver_ids"],
        [],
        "decryption authority policy"
    );
    assert_required_fields!(
        decryption_request,
        DecryptionRequestV1,
        ["consent_evidence_hash", "break_glass_reason"],
        ["consent_evidence_hash", "break_glass_reason"],
        "decryption request"
    );
    assert_required_fields!(
        query_item,
        CiphertextQueryResultItemV1,
        ["state_key", "proof"],
        ["state_key", "proof"],
        "ciphertext query item"
    );
    assert_required_fields!(
        query_response,
        CiphertextQueryResponseV1,
        ["results"],
        [],
        "ciphertext query response"
    );

    let entry = BfvRelinearizationKeyEntry {
        b: Vec::new(),
        a: Vec::new(),
    };
    let relinearization_key = BfvRelinearizationKey {
        entries: vec![entry.clone()],
    };
    let ciphertext = BfvCiphertext {
        c0: Vec::new(),
        c1: Vec::new(),
    };
    let rotation_key = BfvRotationKey {
        rotation_steps: 1,
        zero_refresh: ciphertext.clone(),
    };
    let galois_key = BfvGaloisKey {
        automorphism_power: 3,
        entries: vec![entry.clone()],
    };
    let bootstrap_key = BfvBootstrapKey {
        key_id: "bootstrap-v1".to_owned(),
        max_refresh_rounds: 0,
        public_key_digest: None,
        zero_refresh: ciphertext.clone(),
        round_refreshes: Vec::new(),
        mode: BfvBootstrapKeyMode::RefreshOnlyV1,
        full_bootstrap_material: None,
    };
    let evaluation_keys = BfvEvaluationKeyBundle {
        relinearization_key: relinearization_key.clone(),
        rotation_keys: vec![rotation_key.clone()],
        galois_keys: vec![galois_key.clone()],
        bootstrap_key: None,
    };
    let params = ram_lfe_bfv_parameters_v1();
    let full_bootstrap_material = sample_full_bootstrap_material(&params);
    let full_bootstrap_artifacts = sample_full_bootstrap_circuit_artifacts();
    let reviewer_keypair = sample_ed25519_keypair(0x64);
    let (release_package, _) =
        sample_full_bootstrap_release_audit_package_and_digest(&reviewer_keypair);
    let release_record = release_package.record.clone();
    let release_evidence = release_record.evidence.clone();
    let release_profile = release_evidence.proof_profile.clone();
    let release_key_evidence = release_evidence.prover_key.clone();
    let release_signoff = release_record.signoff.clone();
    let release_signoff_payload = release_signoff.payload.clone();
    let release_manifest = release_package.manifest.clone();
    let release_verdict = release_manifest.verdict;

    assert_unknown_rejected!(
        entry,
        BfvRelinearizationKeyEntry,
        "BFV relinearization entry"
    );
    assert_unknown_rejected!(
        relinearization_key,
        BfvRelinearizationKey,
        "BFV relinearization key"
    );
    assert_unknown_rejected!(galois_key, BfvGaloisKey, "BFV Galois key");
    assert_unknown_rejected!(rotation_key, BfvRotationKey, "BFV rotation key");
    assert_unknown_rejected!(
        BfvBootstrapKeyMode::RefreshOnlyV1,
        BfvBootstrapKeyMode,
        "BFV bootstrap-key mode"
    );
    assert_unknown_rejected!(
        full_bootstrap_material,
        BfvFullBootstrapCircuitMaterialV1,
        "BFV full-bootstrap material"
    );
    assert_unknown_rejected!(
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        BfvFullBootstrapCircuitArtifactRoleV1,
        "BFV full-bootstrap artifact role"
    );
    assert_unknown_rejected!(ciphertext, BfvCiphertext, "BFV ciphertext");
    assert_unknown_rejected!(
        full_bootstrap_artifacts,
        BfvFullBootstrapCircuitArtifactBundleV1,
        "BFV full-bootstrap artifacts"
    );
    assert_unknown_rejected!(
        release_profile,
        BfvFullBootstrapReleaseAuditProofProfileV1,
        "BFV release-audit proof profile"
    );
    assert_unknown_rejected!(
        release_key_evidence,
        BfvFullBootstrapReleaseAuditKeyEvidenceV1,
        "BFV release-audit key evidence"
    );
    assert_unknown_rejected!(
        release_evidence,
        BfvFullBootstrapReleaseAuditEvidenceV1,
        "BFV release-audit evidence"
    );
    assert_unknown_rejected!(
        release_signoff_payload,
        BfvFullBootstrapReleaseAuditSignoffPayloadV1,
        "BFV release-audit signoff payload"
    );
    assert_unknown_rejected!(
        release_signoff,
        BfvFullBootstrapReleaseAuditSignoffV1,
        "BFV release-audit signoff"
    );
    assert_unknown_rejected!(
        release_record,
        BfvFullBootstrapReleaseAuditRecordV1,
        "BFV release-audit record"
    );
    assert_unknown_rejected!(
        release_verdict,
        BfvFullBootstrapReleaseAuditVerdictV1,
        "BFV release-audit verdict"
    );
    assert_unknown_rejected!(
        release_manifest,
        BfvFullBootstrapReleaseAuditManifestV1,
        "BFV release-audit manifest"
    );
    assert_unknown_rejected!(
        release_package,
        BfvFullBootstrapReleaseAuditPackageV1,
        "BFV release-audit package"
    );
    assert_unknown_rejected!(bootstrap_key, BfvBootstrapKey, "BFV bootstrap key");
    assert_unknown_rejected!(
        evaluation_keys,
        BfvEvaluationKeyBundle,
        "BFV evaluation-key bundle"
    );
    assert_required_fields!(
        bootstrap_key,
        BfvBootstrapKey,
        [
            "public_key_digest",
            "round_refreshes",
            "mode",
            "full_bootstrap_material",
        ],
        ["public_key_digest", "full_bootstrap_material"],
        "BFV bootstrap key"
    );
    assert_required_fields!(
        evaluation_keys,
        BfvEvaluationKeyBundle,
        ["rotation_keys", "galois_keys", "bootstrap_key"],
        ["bootstrap_key"],
        "BFV evaluation-key bundle"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn governed_fhe_material_and_policy_history_enforce_exact_monotonic_lifecycle() {
    let material_v1 =
        sample_governed_fhe_material_for_lifecycle(NonZeroU32::new(1).expect("nonzero"));
    material_v1
        .validate()
        .expect("canonical governed material must validate");
    let reference_v1 = material_v1.policy_reference();
    reference_v1
        .validate()
        .expect("canonical governed material reference must validate");
    assert_eq!(reference_v1.material_digest, material_v1.material_digest);
    let scope = SoracloudFheGovernancePermissionScopeV1 {
        schema_version: SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
        service_name: material_v1.service_name.clone(),
        policy_name: material_v1.policy_name.clone(),
    };
    scope.validate().expect("canonical permission scope");
    let mut wrong_scope_version = scope;
    wrong_scope_version.schema_version = 0;
    assert!(matches!(
        wrong_scope_version
            .validate()
            .expect_err("permission scope version drift must fail"),
        SoracloudManifestError::UnsupportedVersion { .. }
    ));
    let admitted_v1 = Hash::new(b"governed material admission v1");
    let mut record = SoracloudFhePolicyRecordV1 {
        schema_version: SORACLOUD_FHE_POLICY_RECORD_VERSION_V1,
        service_name: material_v1.service_name.clone(),
        policy_name: material_v1.policy_name.clone(),
        active_version: Some(material_v1.version),
        versions: BTreeMap::from([(
            material_v1.version,
            SoracloudFhePolicyVersionStateV1 {
                material: material_v1.clone(),
                admitted_by_transaction_hash: admitted_v1,
                lifecycle: SoracloudFhePolicyVersionLifecycleV1::Active,
                deactivated_by_transaction_hash: None,
            },
        )]),
    };
    record.validate().expect("first active policy version");
    let material_v2 =
        sample_governed_fhe_material_for_lifecycle(NonZeroU32::new(2).expect("nonzero"));
    let rotated_by = Hash::new(b"governed material rotation v2");
    let old = record
        .versions
        .get_mut(&material_v1.version)
        .expect("version one");
    old.lifecycle = SoracloudFhePolicyVersionLifecycleV1::Superseded;
    old.deactivated_by_transaction_hash = Some(rotated_by);
    record.versions.insert(
        material_v2.version,
        SoracloudFhePolicyVersionStateV1 {
            material: material_v2.clone(),
            admitted_by_transaction_hash: rotated_by,
            lifecycle: SoracloudFhePolicyVersionLifecycleV1::Active,
            deactivated_by_transaction_hash: None,
        },
    );
    record.active_version = Some(material_v2.version);
    record.validate().expect("exact next policy version");
    let mut skipped_version = record.clone();
    let active = skipped_version
        .versions
        .remove(&material_v2.version)
        .expect("active version two");
    let version_three = NonZeroU32::new(3).expect("nonzero");
    skipped_version.versions.insert(version_three, active);
    skipped_version.active_version = Some(version_three);
    assert!(matches!(
        skipped_version
            .validate()
            .expect_err("version gaps must fail"),
        SoracloudManifestError::InvalidField {
            field: "versions",
            ..
        }
    ));
    let revoked_by = Hash::new(b"governed material revocation v2");
    let active = record
        .versions
        .get_mut(&material_v2.version)
        .expect("version two");
    active.lifecycle = SoracloudFhePolicyVersionLifecycleV1::Revoked;
    active.deactivated_by_transaction_hash = Some(revoked_by);
    record.active_version = None;
    record
        .validate()
        .expect("permanently revoked policy history");
    let mut digest_tamper = material_v2;
    digest_tamper.material_digest = Hash::new(b"attacker-selected governed material digest");
    assert!(matches!(
        digest_tamper
            .validate()
            .expect_err("material digest substitution must fail"),
        SoracloudManifestError::InvalidField {
            field: "material_digest",
            ..
        }
    ));
}
