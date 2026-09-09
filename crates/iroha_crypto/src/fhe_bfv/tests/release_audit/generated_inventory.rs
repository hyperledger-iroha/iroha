//! Release-audit generated inventory assertions and fixture ownership.

use super::*;

pub(super) struct GeneratedPackages {
    pub(super) package: BfvFullBootstrapReleaseAuditPackageV1,
    pub(super) generated_audit_report_bytes: Vec<u8>,
    pub(super) generated_audit_evidence_archive_bytes: Vec<u8>,
}

pub(super) fn check(signed: &SignedAuditInputs<'_>) -> GeneratedPackages {
    let_row! { package = release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build release audit package") };
    let_row! { (external_review_package, external_review_package_digest) = bfv_full_bootstrap_release_audit_external_review_package_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build externally reviewed release audit package and digest") };
    assert_eq_row! { external_review_package, package, "{}", signed.artifact_fixture.diagnostics.static_context_at(238) };
    validate_bfv_full_bootstrap_release_audit_package_for_artifacts_and_trusted_reviewer_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &external_review_package,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect(
        "signed fixture package binds its governed artifacts and trusted reviewer structurally",
    );
    assert!(
        matches!(
            validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &external_review_package,
        external_review_package_digest,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    ),
            Err(BfvError::ProductionQualificationUnavailable(
                BfvProductionQualificationBlockerV1::MissingRegisteredHeOrgLatticeNoiseAndQromEvidence,
            )),
        ),
        "a structurally valid signed package cannot supply missing registered BFV production evidence",
    );
    let_row! { generic_reviewer_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), signed.review_fixture.proof_profile_fields_fragment.as_bytes(), ] .concat() };
    let_row! { generic_reviewer_archive_body = [ b"external-review-evidence-archive: BFV full-bootstrap prover verifier evidence v1; artifact-bundle-digest=".as_slice(), signed.review_fixture.artifact_bundle_digest_hex.as_bytes(), b"; evaluator-artifact-set-digest=".as_slice(), signed.review_fixture.evaluator_artifact_set_digest_hex.as_bytes(), b"; centered-source-chain-digest=".as_slice(), signed.review_fixture.centered_source_chain_digest_hex.as_bytes(), b"; arithmetic-trace-profile-digest=".as_slice(), signed.review_fixture.arithmetic_trace_profile_digest_hex.as_bytes(), b"; arithmetic-air-constraint-system-digest=".as_slice(), signed.review_fixture.arithmetic_air_constraint_system_digest_hex.as_bytes(), signed.review_fixture.proof_profile_fields_fragment.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; generated-circuit-body-byte-length=".as_slice(), signed.review_fixture.generated_circuit_body_byte_length.as_bytes(), b"; generated-circuit-body-hex=".as_slice(), signed.review_fixture.generated_circuit_body_hex.as_bytes(), b"; coefficient-to-slot-key-artifact-hex=".as_slice(), signed.review_fixture.coefficient_to_slot_key_artifact_hex.as_bytes(), b"; slot-to-coefficient-key-artifact-hex=".as_slice(), signed.review_fixture.slot_to_coefficient_key_artifact_hex.as_bytes(), b"; blind-rotation-key-artifact-hex=".as_slice(), signed.review_fixture.blind_rotation_key_artifact_hex.as_bytes(), b"; extraction-key-artifact-hex=".as_slice(), signed.review_fixture.sample_extraction_key_artifact_hex.as_bytes(), b"; accumulator-artifact-hex=".as_slice(), signed.review_fixture.accumulator_artifact_hex.as_bytes(), b"; proof-public-input-schema-artifact-hex=".as_slice(), signed.review_fixture.proof_public_input_schema_artifact_hex.as_bytes(), b"; arithmetic-air-constraint-system-artifact-hex=".as_slice(), signed.review_fixture.arithmetic_air_constraint_system_artifact_hex.as_bytes(), b"; native-prover-payload-hex=".as_slice(), signed.review_fixture.native_prover_payload_hex.as_bytes(), b"; prover-native-payload-digest=".as_slice(), signed.review_fixture.prover_native_payload_digest_hex.as_bytes(), b"; native-verifier-payload-hex=".as_slice(), signed.review_fixture.native_verifier_payload_hex.as_bytes(), b"; verifier-native-payload-digest=".as_slice(), signed.review_fixture.verifier_native_payload_digest_hex.as_bytes(), b"; prover-key-artifact-hex=".as_slice(), signed.review_fixture.prover_key_artifact_hex.as_bytes(), b"; verifier-key-artifact-hex=".as_slice(), signed.review_fixture.verifier_key_artifact_hex.as_bytes(), b"; native-circuit-fingerprint=".as_slice(), signed.review_fixture.native_circuit_fingerprint_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), b"; prover-key-digest=".as_slice(), signed.review_fixture.prover_key_digest_hex.as_bytes(), b"; verifier-key-digest=".as_slice(), signed.review_fixture.verifier_key_digest_hex.as_bytes(), ] .concat() };
    let_row! { generic_reviewer_archive_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, generic_reviewer_archive_body.as_slice(), ] .concat() };
    let_row! { generic_reviewer_package = release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generic_reviewer_report_bytes, &generic_reviewer_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build structurally valid package with generic external-review statements") };
    let_row! { generic_reviewer_package_digest = release_package_digest_v1(&generic_reviewer_package) .expect("generic reviewer package digest") };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 239; (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generic_reviewer_package, generic_reviewer_package_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (bfv_full_bootstrap_release_audit_external_review_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generic_reviewer_report_bytes, &generic_reviewer_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )) };
    let_row! { (generated_audit_report_bytes, generated_audit_evidence_archive_bytes) = bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, ) .expect("build canonical release audit report/archive bytes from governed artifacts") };
    let_row! { ( decoded_report_archive_artifacts, generated_report_bytes_from_artifact_bundle_bytes, generated_archive_bytes_from_artifact_bundle_bytes, ) = bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifact_bundle_bytes_decoded_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.artifact_bundle_bytes, ) .expect("build canonical release audit report/archive bytes from artifact bytes") };
    assert_row! { (decoded_report_archive_artifacts) == (signed.artifact_fixture.artifacts) && (generated_report_bytes_from_artifact_bundle_bytes) == (generated_audit_report_bytes) && (generated_archive_bytes_from_artifact_bundle_bytes) == (generated_audit_evidence_archive_bytes), "{}", signed.artifact_fixture.diagnostics.group_context(241, 3), };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 244; (bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifact_bundle_bytes_decoded_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifact_bundle_bytes_decoded_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, )) };
    assert_row! { generated_audit_report_bytes.starts_with(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1) };
    assert_row! { generated_audit_evidence_archive_bytes .starts_with(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1) };
    let_row! { generated_report_body = bfv_full_bootstrap_release_audit_artifact_body_v1( "generated BFV full-bootstrap release audit report", &generated_audit_report_bytes, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, ) .expect("generated report body validates") };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, &[b"proof-profile-field-count".as_slice()], &[signed.review_fixture.proof_profile_field_count.as_bytes()], ), "generated report must advertise the release-audit proof-profile field count" };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, &[b"proof-profile-requires-canonical-base-transcript-label".as_slice()], &[b"true".as_slice()], ), "generated report must advertise canonical base transcript-label enforcement" };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, &[b"proof-profile-rejects-suffixed-transcript-label-aliases".as_slice()], &[b"true".as_slice()], ), "generated report must advertise suffixed transcript-label alias rejection" };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_REQUIRES_VERIFIER_TRACE_DIGEST_LABEL_ALIASES, &[b"true".as_slice()], ), "generated report must advertise verifier-owned trace material digest input" };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_VALIDATES_PUBLIC_OPENING_MATERIAL_LABEL_ALIASES, &[b"true".as_slice()], ), "generated report must advertise typed public-opening material validation" };
    let proof_profile_domains = [
        (
            "proof input material digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_PROOF_INPUT_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .proof_input_material_digest_domain
                .as_slice(),
        ),
        (
            "prover input material digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_PROVER_INPUT_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .prover_input_material_digest_domain
                .as_slice(),
        ),
        (
            "AIR evaluation material digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_AIR_EVALUATION_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .air_evaluation_material_digest_domain
                .as_slice(),
        ),
        (
            "public opening material digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_PUBLIC_OPENING_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .public_opening_material_digest_domain
                .as_slice(),
        ),
        (
            "arithmetic trace material digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_TRACE_MATERIAL_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .arithmetic_trace_material_digest_domain
                .as_slice(),
        ),
        (
            "arithmetic AIR constraint-system digest domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_AIR_CONSTRAINT_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .arithmetic_air_constraint_system_digest_domain
                .as_slice(),
        ),
        (
            "proof-key material commitment domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_PROOF_KEY_MATERIAL_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .proof_key_material_commitment_domain
                .as_slice(),
        ),
        (
            "proof-key pair commitment domain",
            BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_PROOF_KEY_PAIR_DOMAIN_LABEL_ALIASES,
            signed
                .artifact_fixture
                .evidence
                .proof_profile
                .proof_key_pair_commitment_domain
                .as_slice(),
        ),
    ];
    for &(label, aliases, value) in &proof_profile_domains {
        assert_row! { release_body_contains_labelled_value_v1(generated_report_body, aliases, &[value],), "generated report must advertise proof-profile {label}" };
    }
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_SEPARATES_RELEASE_PROVER_DOMAINS_LABEL_ALIASES, &[b"true".as_slice()], ), "generated report must advertise release-prover digest-domain separation" };
    assert_row! { release_body_contains_labelled_value_v1( generated_report_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_SEPARATES_PROOF_KEY_COMMITMENTS_LABEL_ALIASES, &[b"true".as_slice()], ), "generated report must advertise proof-key commitment-domain separation" };
    let_row! { generated_archive_body = bfv_full_bootstrap_release_audit_artifact_body_v1( "generated BFV full-bootstrap release audit archive", &generated_audit_evidence_archive_bytes, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, ) .expect("generated archive body validates") };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, &[b"proof-profile-field-count".as_slice()], &[signed.review_fixture.proof_profile_field_count.as_bytes()], ), "generated archive must advertise the release-audit proof-profile field count" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, &[b"proof-profile-requires-canonical-base-transcript-label".as_slice()], &[b"true".as_slice()], ), "generated archive must advertise canonical base transcript-label enforcement" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, &[b"proof-profile-rejects-suffixed-transcript-label-aliases".as_slice()], &[b"true".as_slice()], ), "generated archive must advertise suffixed transcript-label alias rejection" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_REQUIRES_VERIFIER_TRACE_DIGEST_LABEL_ALIASES, &[b"true".as_slice()], ), "generated archive must advertise verifier-owned trace material digest input" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_VALIDATES_PUBLIC_OPENING_MATERIAL_LABEL_ALIASES, &[b"true".as_slice()], ), "generated archive must advertise typed public-opening material validation" };
    for &(label, aliases, value) in &proof_profile_domains {
        assert_row! { release_body_contains_labelled_value_v1(generated_archive_body, aliases, &[value],), "generated archive must advertise proof-profile {label}" };
    }
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_SEPARATES_RELEASE_PROVER_DOMAINS_LABEL_ALIASES, &[b"true".as_slice()], ), "generated archive must advertise release-prover digest-domain separation" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_SEPARATES_PROOF_KEY_COMMITMENTS_LABEL_ALIASES, &[b"true".as_slice()], ), "generated archive must advertise proof-key commitment-domain separation" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_TRACE_PROFILE_LABEL_ALIASES, &[signed.review_fixture.arithmetic_trace_profile_digest_hex.as_bytes()], ), "generated archive must carry the typed arithmetic trace-profile digest" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_AIR_CONSTRAINT_LABEL_ALIASES, &[signed.review_fixture.arithmetic_air_constraint_system_digest_hex.as_bytes()], ), "generated archive must carry the typed arithmetic AIR constraint-system digest" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_GENERATED_BODY_HEX_LABEL_ALIASES, &[signed.review_fixture.generated_circuit_body_hex.as_bytes()], ), "generated archive must carry the exact canonical generated-circuit body bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_COEFFICIENT_TO_SLOT_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.coefficient_to_slot_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed coefficient-to-slot key artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_SLOT_TO_COEFFICIENT_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.slot_to_coefficient_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed slot-to-coefficient key artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BLIND_ROTATION_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.blind_rotation_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed blind-rotation key artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_SAMPLE_EXTRACTION_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.sample_extraction_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed sample-extraction key artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_ACCUMULATOR_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.accumulator_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed accumulator artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_PROOF_SCHEMA_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.proof_public_input_schema_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed proof public-input schema artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_ARITHMETIC_AIR_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.arithmetic_air_constraint_system_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed arithmetic AIR artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_NATIVE_PROVER_PAYLOAD_HEX_LABEL_ALIASES, &[signed.review_fixture.native_prover_payload_hex.as_bytes()], ), "generated archive must carry the exact canonical native prover payload bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_NATIVE_VERIFIER_PAYLOAD_HEX_LABEL_ALIASES, &[signed.review_fixture.native_verifier_payload_hex.as_bytes()], ), "generated archive must carry the exact canonical native verifier payload bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_PROVER_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.prover_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed prover-key artifact bytes" };
    assert_row! { release_body_contains_labelled_value_v1( generated_archive_body, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_VERIFIER_KEY_ARTIFACT_HEX_LABEL_ALIASES, &[signed.review_fixture.verifier_key_artifact_hex.as_bytes()], ), "generated archive must carry the exact governed verifier-key artifact bytes" };
    validate_bfv_full_bootstrap_release_audit_archive_binds_governed_artifacts_v1(
        &generated_audit_evidence_archive_bytes,
        &signed.artifact_fixture.artifacts,
    )
    .expect("generated archive must bind the exact governed proof-key artifact bytes");
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = signed.artifact_fixture.artifacts =>
            validate_bfv_full_bootstrap_release_audit_archive_binds_governed_artifacts_v1(
            &generated_audit_evidence_archive_bytes,
            @,
        );
        246 => case.prover_key[0] ^= 0x01;
        247 => case.proof_public_input_schema[0] ^= 0x01;
        248 => case.arithmetic_air_constraint_system[0] ^= 0x01;
        249 => case.accumulator[0] ^= 0x01;
    }
    let_row! { generated_package = release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_audit_report_bytes, &generated_audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build release audit package from canonical generated report/archive bytes") };
    validate_release_package_for_artifacts_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &generated_package,
    )
    .expect("canonical generated release audit report/archive bytes match governed artifacts");
    assert_local_diag! { signed.artifact_fixture.diagnostics; 250 => bfv_full_bootstrap_release_audit_external_review_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_audit_report_bytes, &generated_audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { canonical_package = bfv_full_bootstrap_release_audit_package_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build release audit package from governed artifacts") };
    assert_row! { (canonical_package.audit_report_bytes) == (generated_audit_report_bytes) && (canonical_package.audit_evidence_archive_bytes) == (generated_audit_evidence_archive_bytes), "{}", signed.artifact_fixture.diagnostics.group_context(251, 2), };
    validate_release_package_for_artifacts_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &canonical_package,
    )
    .expect("one-shot generated package matches governed artifacts");
    let_row! { canonical_package_digest = release_package_digest_v1(&canonical_package).expect("one-shot package digest validates") };
    let_row! { (paired_package, paired_package_digest) = bfv_full_bootstrap_release_audit_package_and_digest_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build release audit package and digest from governed artifacts") };
    assert_row! { (paired_package) == (canonical_package) && (paired_package_digest) == (canonical_package_digest), "{}", signed.artifact_fixture.diagnostics.group_context(253, 2), };
    let_row! { ( decoded_package_artifacts_from_bytes, package_from_artifact_bundle_bytes, package_digest_from_artifact_bundle_bytes, ) = bfv_full_bootstrap_release_audit_package_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build deterministic release audit package and digest from artifact bytes") };
    assert_row! { (decoded_package_artifacts_from_bytes) == (signed.artifact_fixture.artifacts) && (package_from_artifact_bundle_bytes) == (canonical_package) && (package_digest_from_artifact_bundle_bytes) == (canonical_package_digest), "{}", signed.artifact_fixture.diagnostics.group_context(255, 3), };
    let_row! { ( decoded_package_artifacts_with_digests, package_from_artifact_bundle_bytes_with_digests, admitted_artifact_bundle_digest_from_package_builder, package_digest_from_artifact_bundle_bytes_with_digests, ) = bfv_full_bootstrap_release_audit_package_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build deterministic release audit package and digests from artifact bytes") };
    assert_row! { (decoded_package_artifacts_with_digests) == (signed.artifact_fixture.artifacts) && (package_from_artifact_bundle_bytes_with_digests) == (canonical_package) && (admitted_artifact_bundle_digest_from_package_builder) == (signed.artifact_fixture.evidence.artifact_bundle_digest) && (package_digest_from_artifact_bundle_bytes_with_digests) == (canonical_package_digest), "{}", signed.artifact_fixture.diagnostics.group_context(258, 4), };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 262; (bfv_full_bootstrap_release_audit_package_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_private_key, )), (bfv_full_bootstrap_release_audit_package_and_all_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_all_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_package_and_all_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &canonical_package, canonical_package_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    GeneratedPackages {
        package,
        generated_audit_report_bytes,
        generated_audit_evidence_archive_bytes,
    }
}
