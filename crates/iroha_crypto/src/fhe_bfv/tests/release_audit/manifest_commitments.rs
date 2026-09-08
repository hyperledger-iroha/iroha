//! Release-audit manifest commitments assertions and fixture ownership.

use super::*;

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    generated_inventory: &GeneratedPackages,
    manifest_codec: &ManifestFrames,
    package_codec: &PackageAuthorities,
) {
    macro_rules! expect_manifest_and_package_rejected {
        ($($field:ident: $value:expr, $index:literal;)+) => {
            $({
                let mut rejected_manifest = manifest_codec.manifest.clone();
                rejected_manifest.$field = $value;
                assert_error_matrix_row! {
                    signed.artifact_fixture.diagnostics;
                    $index;
                    (validate_release_manifest_v1(&rejected_manifest)),
                    (release_manifest_digest_v1(&rejected_manifest))
                };
                let mut rejected_package = generated_inventory.package.clone();
                rejected_package.manifest = rejected_manifest;
                assert_error_matrix_row! {
                    signed.artifact_fixture.diagnostics;
                    $index + 2;
                    (validate_release_package_v1(&rejected_package)),
                    (release_package_digest_v1(&rejected_package))
                };
            })+
        };
    }
    expect_manifest_and_package_rejected! {
        audit_scope: format!(" {BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SCOPE_V1}"), 523;
        version: manifest_codec.manifest.version + 1, 527;
        field_count: manifest_codec.manifest.field_count + 1, 531;
        verdict: BfvFullBootstrapReleaseAuditVerdictV1::Rejected, 535;
        package_version: manifest_codec.manifest.package_version + 1, 539;
        package_field_count: manifest_codec.manifest.package_field_count + 1, 543;
    }
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = manifest_codec.manifest => validate_release_manifest_v1(@);
        547 => case.record_digest = Hash::prehashed([0_u8; Hash::LENGTH]);
        548 => case.generated_circuit_body_digest = Hash::prehashed([0_u8; Hash::LENGTH]);
    }
    let mut stale_manifest_digest_package = generated_inventory.package.clone();
    stale_manifest_digest_package.manifest_digest =
        Hash::new(b"stale-release-audit-manifest-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 549; (validate_release_package_v1(&stale_manifest_digest_package)), (release_package_digest_v1(&stale_manifest_digest_package)) };
    macro_rules! expect_rehashed_manifest_digest_rejected {
        ($($field:ident, $preimage_suffix:literal, $expect_label:literal, $index:literal;)+) => {
            $({
                let mut rejected = generated_inventory.package.clone();
                rejected.manifest.$field = Hash::new(
                    concat!("stale-release-audit-manifest-", $preimage_suffix).as_bytes(),
                );
                rejected.manifest_digest = release_manifest_digest_v1(&rejected.manifest)
                    .expect(concat!("digest stale-", $expect_label, " manifest"));
                assert_local_diag! {
                    signed.artifact_fixture.diagnostics;
                    $index => validate_release_package_v1(&rejected)
                };
            })+
        };
    }
    expect_rehashed_manifest_digest_rejected! {
        record_digest, "record-digest", "record", 551;
        release_audit_evidence_digest, "evidence-digest", "evidence", 552;
    }
    let mut stale_manifest_source_chain_package = generated_inventory.package.clone();
    stale_manifest_source_chain_package
        .manifest
        .centered_scale_round_source_chain_digest =
        Hash::new(b"stale-release-audit-manifest-centered-source-chain-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 553; (validate_release_package_v1(&stale_manifest_source_chain_package)), (release_manifest_digest_v1(&stale_manifest_source_chain_package.manifest)) };
    expect_rehashed_manifest_digest_rejected! {
        audit_report_digest, "report-digest", "report", 555;
        audit_evidence_archive_digest, "evidence-archive-digest", "archive", 556;
        artifact_bundle_digest, "artifact-bundle-digest", "artifact", 557;
        evaluator_artifact_set_digest,
            "evaluator-artifact-set-digest", "evaluator-artifact", 558;
        proof_key_pair_commitment,
            "proof-key-pair-commitment", "proof-key-pair", 559;
        prover_key_digest, "prover-key-digest", "prover-key", 560;
        verifier_key_digest, "verifier-key-digest", "verifier-key", 561;
    }
    let mut stale_manifest_generated_body_package = generated_inventory.package.clone();
    stale_manifest_generated_body_package
        .manifest
        .generated_circuit_body_digest =
        Hash::new(b"stale-release-audit-manifest-generated-circuit-body-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 562; (validate_release_manifest_v1(&stale_manifest_generated_body_package.manifest)), (release_manifest_digest_v1(&stale_manifest_generated_body_package.manifest)), (validate_release_package_v1(&stale_manifest_generated_body_package)) };
    let mut stale_manifest_native_fingerprint = manifest_codec.manifest.clone();
    stale_manifest_native_fingerprint.native_circuit_fingerprint =
        Hash::new(b"stale-release-audit-manifest-native-circuit-fingerprint");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 565; (validate_release_manifest_v1(&stale_manifest_native_fingerprint)), (release_manifest_digest_v1(&stale_manifest_native_fingerprint)) };
    mutation_row! { stale_manifest_native_fingerprint_package = generated_inventory.package.clone(); stale_manifest_native_fingerprint_package.manifest = stale_manifest_native_fingerprint; assert_local_diag! { signed.artifact_fixture.diagnostics; 567 => validate_release_package_v1(&stale_manifest_native_fingerprint_package) }; };
    let mut aliased_manifest_archive_digest = manifest_codec.manifest.clone();
    aliased_manifest_archive_digest.audit_evidence_archive_digest =
        aliased_manifest_archive_digest.proof_key_pair_commitment;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 568; (validate_release_manifest_v1(&aliased_manifest_archive_digest)), (release_manifest_digest_v1(&aliased_manifest_archive_digest)) };
    let mut aliased_manifest_report_evidence_digest = manifest_codec.manifest.clone();
    aliased_manifest_report_evidence_digest.audit_report_digest =
        aliased_manifest_report_evidence_digest.release_audit_evidence_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 570; (validate_release_manifest_v1(&aliased_manifest_report_evidence_digest)), (release_manifest_digest_v1(&aliased_manifest_report_evidence_digest)) };
    let mut aliased_manifest_archive_evidence_digest = manifest_codec.manifest.clone();
    aliased_manifest_archive_evidence_digest.audit_evidence_archive_digest =
        aliased_manifest_archive_evidence_digest.release_audit_evidence_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 572; (validate_release_manifest_v1(&aliased_manifest_archive_evidence_digest)), (release_manifest_digest_v1(&aliased_manifest_archive_evidence_digest)) };
    let_row! { manifest_signed_commitment_aliases = [ ("record digest", &manifest_codec.manifest.record_digest), ( "centered scale-round source-chain digest", &manifest_codec.manifest.centered_scale_round_source_chain_digest, ), ("artifact bundle digest", &manifest_codec.manifest.artifact_bundle_digest), ( "evaluator artifact set digest", &manifest_codec.manifest.evaluator_artifact_set_digest, ), ( "proof-key pair commitment", &manifest_codec.manifest.proof_key_pair_commitment, ), ("prover-key digest", &manifest_codec.manifest.prover_key_digest), ("verifier-key digest", &manifest_codec.manifest.verifier_key_digest), ( "prover native payload digest", &manifest_codec.manifest.prover_native_payload_digest, ), ( "verifier native payload digest", &manifest_codec.manifest.verifier_native_payload_digest, ), ( "native circuit fingerprint", &manifest_codec.manifest.native_circuit_fingerprint, ), ( "generated circuit body digest", &manifest_codec.manifest.generated_circuit_body_digest, ), ] };
    for (label, alias_digest) in manifest_signed_commitment_aliases {
        let mut aliased_report_digest = manifest_codec.manifest.clone();
        aliased_report_digest.audit_report_digest = *alias_digest;
        let expected = format!("audit report digest must be distinct from {label}");
        let_row! { context = format!("release audit manifests must reject report digest aliasing with {label}") };
        assert_call! { assert_error_contains; validate_release_manifest_v1(&aliased_report_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(574, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(574, &context), };
        let_row! { context = format!( "release audit manifest digesting must reject report digest aliasing with {label}" ) };
        assert_call! { assert_error_contains; release_manifest_digest_v1(&aliased_report_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(575, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(575, &context), };
        let mut aliased_archive_digest = manifest_codec.manifest.clone();
        aliased_archive_digest.audit_evidence_archive_digest = *alias_digest;
        let expected = format!("evidence archive digest must be distinct from {label}");
        let_row! { context = format!("release audit manifests must reject archive digest aliasing with {label}") };
        assert_call! { assert_error_contains; validate_release_manifest_v1(&aliased_archive_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(576, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(576, &context), };
        let_row! { context = format!( "release audit manifest digesting must reject archive digest aliasing with {label}" ) };
        assert_call! { assert_error_contains; release_manifest_digest_v1(&aliased_archive_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(577, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(577, &context), };
    }
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = generated_inventory.package => validate_release_package_v1(@);
        578 => {
            case.manifest.reviewer_id = "sora-zk-audit-wg-2026-alt".to_owned();
            case.manifest_digest =
                release_manifest_digest_v1(&case.manifest)
                    .expect("digest stale-reviewer-id manifest");
        };
        579 => {
            case.manifest.reviewer_public_key = package_codec.alternate_reviewer_key_pair.public_key().clone();
            case.manifest_digest =
                release_manifest_digest_v1(&case.manifest)
                    .expect("digest stale-reviewer-key manifest");
        };
    }
    let mut stale_evidence_for_payload_preflight = signed.artifact_fixture.evidence.clone();
    stale_evidence_for_payload_preflight.version += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 580; (release_signoff_payload_v1( &stale_evidence_for_payload_preflight, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &stale_evidence_for_payload_preflight, Hash::prehashed([0_u8; Hash::LENGTH]), signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &stale_evidence_for_payload_preflight, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_report_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &signed.artifact_fixture.evidence, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &signed.artifact_fixture.evidence, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_report_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &signed.artifact_fixture.evidence, signed.evidence_codec.digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (release_signoff_payload_v1( &signed.artifact_fixture.evidence, signed.review_fixture.audit_report_digest, signed.evidence_codec.digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = signed.signoff_authority.signoff => validate_release_signoff_v1(@);
        587 => case.payload.audit_report_digest = case.payload.artifact_bundle_digest;
        588 => case.payload.audit_evidence_archive_digest = case.payload.artifact_bundle_digest;
    }
    let_row! { signoff_signed_commitment_aliases = [ ( "centered scale-round source-chain digest", &signed.signoff_authority.signoff.payload.centered_scale_round_source_chain_digest, ), ( "artifact bundle digest", &signed.signoff_authority.signoff.payload.artifact_bundle_digest, ), ( "evaluator artifact set digest", &signed.signoff_authority.signoff.payload.evaluator_artifact_set_digest, ), ( "proof-key pair commitment", &signed.signoff_authority.signoff.payload.proof_key_pair_commitment, ), ("prover-key digest", &signed.signoff_authority.signoff.payload.prover_key_digest), ("verifier-key digest", &signed.signoff_authority.signoff.payload.verifier_key_digest), ( "prover native payload digest", &signed.signoff_authority.signoff.payload.prover_native_payload_digest, ), ( "verifier native payload digest", &signed.signoff_authority.signoff.payload.verifier_native_payload_digest, ), ( "native circuit fingerprint", &signed.signoff_authority.signoff.payload.native_circuit_fingerprint, ), ( "generated circuit body digest", &signed.signoff_authority.signoff.payload.generated_circuit_body_digest, ), ] };
    for (label, alias_digest) in signoff_signed_commitment_aliases {
        let mut aliased_report_digest = signed.signoff_authority.signoff.clone();
        aliased_report_digest.payload.audit_report_digest = *alias_digest;
        let expected = format!("audit report digest must be distinct from {label}");
        let_row! { context = format!("release audit signoffs must reject report digest aliasing with {label}") };
        assert_call! { assert_error_contains; validate_release_signoff_v1(&aliased_report_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(589, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(589, &context), };
        let mut aliased_archive_digest = signed.signoff_authority.signoff.clone();
        aliased_archive_digest.payload.audit_evidence_archive_digest = *alias_digest;
        let expected = format!("evidence archive digest must be distinct from {label}");
        let_row! { context = format!("release audit signoffs must reject archive digest aliasing with {label}") };
        assert_call! { assert_error_contains; validate_release_signoff_v1(&aliased_archive_digest), signed.artifact_fixture.diagnostics.dynamic_expected_at(590, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(590, &context), };
    }
    let_row! { record_external_digest_aliases = [ ("release evidence digest", &signed.evidence_codec.digest), ( "centered scale-round source-chain digest", &signed.signoff_authority.signoff.payload.centered_scale_round_source_chain_digest, ), ( "artifact bundle digest", &signed.signoff_authority.signoff.payload.artifact_bundle_digest, ), ( "evaluator artifact set digest", &signed.signoff_authority.signoff.payload.evaluator_artifact_set_digest, ), ( "proof-key pair commitment", &signed.signoff_authority.signoff.payload.proof_key_pair_commitment, ), ("prover-key digest", &signed.signoff_authority.signoff.payload.prover_key_digest), ("verifier-key digest", &signed.signoff_authority.signoff.payload.verifier_key_digest), ( "prover native payload digest", &signed.signoff_authority.signoff.payload.prover_native_payload_digest, ), ( "verifier native payload digest", &signed.signoff_authority.signoff.payload.verifier_native_payload_digest, ), ( "native circuit fingerprint", &signed.signoff_authority.signoff.payload.native_circuit_fingerprint, ), ( "generated circuit body digest", &signed.signoff_authority.signoff.payload.generated_circuit_body_digest, ), ] };
    for (label, alias_digest) in record_external_digest_aliases {
        let_row! { expected = if label == "release evidence digest" { "audit report digest must be distinct from release evidence digest".to_owned() } else { format!("audit report digest must be distinct from {label}") } };
        let_row! { context = format!("release audit records must reject report digest aliasing with {label}") };
        assert_call! { assert_error_contains; release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, *alias_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ), signed.artifact_fixture.diagnostics.dynamic_expected_at(591, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(591, &context), };
        let_row! { expected = if label == "release evidence digest" { "evidence archive digest must be distinct from release evidence digest".to_owned() } else { format!("evidence archive digest must be distinct from {label}") } };
        let_row! { context = format!("release audit records must reject archive digest aliasing with {label}") };
        assert_call! { assert_error_contains; release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, *alias_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ), signed.artifact_fixture.diagnostics.dynamic_expected_at(592, &expected), signed.artifact_fixture.diagnostics.dynamic_context_at(592, &context), };
    }
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 593; (release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, Hash::prehashed([0_u8; Hash::LENGTH]), signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &[], &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &[0_u8; 32], &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &[0_u8; 32], "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )) };
}
