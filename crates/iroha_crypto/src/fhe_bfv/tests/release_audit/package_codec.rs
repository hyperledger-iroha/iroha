//! Release-audit package codec assertions and fixture ownership.

use super::*;

pub(super) struct PackageAuthorities {
    pub(super) alternate_reviewer_key_pair: crate::KeyPair,
}

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    generated_inventory: &GeneratedPackages,
    manifest_codec: &ManifestFrames,
) -> PackageAuthorities {
    let package_bytes =
        norito::to_bytes(&generated_inventory.package).expect("encode audit package");
    let_row! { decoded_package = norito::decode_from_bytes::<BfvFullBootstrapReleaseAuditPackageV1>(&package_bytes) .expect("decode audit package") };
    assert_eq_row! { decoded_package, generated_inventory.package, "{}", signed.artifact_fixture.diagnostics.static_context_at(477) };
    let_row! { safely_decoded_package = decode_bfv_full_bootstrap_release_audit_package_bytes_v1(&package_bytes) .expect("safe canonical release audit package decode") };
    assert_eq_row! { safely_decoded_package, generated_inventory.package, "{}", signed.artifact_fixture.diagnostics.static_context_at(478) };
    validate_release_package_for_artifacts_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &decoded_package,
    )
    .expect("decoded release audit package matches governed artifacts");
    let package_digest = release_package_digest_v1(&generated_inventory.package)
        .expect("release audit package digest");
    assert_row! { (package_digest) == (Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_DIGEST_DOMAIN, package_bytes.as_slice(), ])) && (package_digest) == (release_package_digest_v1(&decoded_package) .expect("decoded release audit package digest")) && (package_digest) == (bfv_full_bootstrap_release_audit_package_digest_from_bytes_v1(&package_bytes) .expect("canonical release audit package byte digest")), "{}", signed.artifact_fixture.diagnostics.group_context(479, 3), };
    let_row! { compressed_package_bytes = norito::to_compressed_bytes(&generated_inventory.package, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit package") };
    assert_ne_row! { compressed_package_bytes, package_bytes, "compressed release audit package must differ from canonical v1 bytes" };
    let_row! { decoded_compressed_package = decode_trusted_compressed_fixture(&compressed_package_bytes, &generated_inventory.package) .expect("compressed release audit package must decode structurally") };
    assert_eq_row! { decoded_compressed_package, generated_inventory.package, "{}", signed.artifact_fixture.diagnostics.static_context_at(482) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 483; (decode_bfv_full_bootstrap_release_audit_package_bytes_v1(&compressed_package_bytes)), (bfv_full_bootstrap_release_audit_package_digest_from_bytes_v1( &compressed_package_bytes )) };
    let_row! { binary_split_placeholder_package_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit package bytes".to_vec() };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 485; (decode_bfv_full_bootstrap_release_audit_package_bytes_v1( &binary_split_placeholder_package_bytes, )), (bfv_full_bootstrap_release_audit_package_digest_from_bytes_v1( &binary_split_placeholder_package_bytes, )) };
    assert_ne_row! { package_digest, signed.record_codec.record_digest, "release audit package digest must be domain-separated from record digest" };
    assert_ne_row! { package_digest, manifest_codec.manifest_digest, "release audit package digest must be domain-separated from manifest digest" };
    for placeholder_preimage in BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_PREIMAGES {
        let placeholder_digest = Hash::new(placeholder_preimage);
        let mut placeholder_record_digest_package = generated_inventory.package.clone();
        placeholder_record_digest_package.record_digest = placeholder_digest;
        assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 487; (validate_release_package_v1(&placeholder_record_digest_package)), (release_package_digest_v1(&placeholder_record_digest_package)) };
        let mut placeholder_manifest_digest_package = generated_inventory.package.clone();
        placeholder_manifest_digest_package.manifest_digest = placeholder_digest;
        assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 489; (validate_release_package_v1(&placeholder_manifest_digest_package)), (release_package_digest_v1(&placeholder_manifest_digest_package)) };
    }
    let placeholder_digest = Hash::new(BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_PREIMAGES[0]);
    let mut stale_record_with_placeholder_record_digest = generated_inventory.package.clone();
    stale_record_with_placeholder_record_digest.record.version += 1;
    stale_record_with_placeholder_record_digest.record_digest = placeholder_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 491; (validate_release_package_v1(&stale_record_with_placeholder_record_digest)), (release_package_digest_v1(&stale_record_with_placeholder_record_digest)) };
    let mut stale_manifest_with_placeholder_manifest_digest = generated_inventory.package.clone();
    stale_manifest_with_placeholder_manifest_digest
        .manifest
        .version += 1;
    stale_manifest_with_placeholder_manifest_digest.manifest_digest = placeholder_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 493; (validate_release_package_v1(&stale_manifest_with_placeholder_manifest_digest)), (release_package_digest_v1(&stale_manifest_with_placeholder_manifest_digest)) };
    let mut delayed_placeholder_record_digest_package = generated_inventory.package.clone();
    delayed_placeholder_record_digest_package.record_digest =
        signed.evidence_codec.delayed_material_placeholder_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 495; (validate_release_package_v1(&delayed_placeholder_record_digest_package)), (release_package_digest_v1(&delayed_placeholder_record_digest_package)) };
    let mut delayed_placeholder_manifest_digest_package = generated_inventory.package.clone();
    delayed_placeholder_manifest_digest_package.manifest_digest =
        signed.evidence_codec.delayed_material_placeholder_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 497; (validate_release_package_v1(&delayed_placeholder_manifest_digest_package)), (release_package_digest_v1(&delayed_placeholder_manifest_digest_package)) };
    validate_bfv_full_bootstrap_release_audit_signoff_trusted_reviewer_v1(
        &signed.signoff_authority.signoff,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect("signoff matches trusted reviewer");
    validate_bfv_full_bootstrap_release_audit_record_trusted_reviewer_v1(
        &signed.signoff_codec.record,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect("record matches trusted reviewer");
    validate_bfv_full_bootstrap_release_audit_manifest_trusted_reviewer_v1(
        &manifest_codec.manifest,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect("manifest matches trusted reviewer");
    validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1(
        &generated_inventory.package,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect("package matches trusted reviewer");
    let mut stale_signoff_for_trusted_reviewer_preflight = signed.signoff_authority.signoff.clone();
    stale_signoff_for_trusted_reviewer_preflight.version += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 499; (validate_bfv_full_bootstrap_release_audit_signoff_trusted_reviewer_v1( &stale_signoff_for_trusted_reviewer_preflight, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_signoff_trusted_reviewer_v1( &stale_signoff_for_trusted_reviewer_preflight, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )) };
    let mut stale_record_for_trusted_reviewer_preflight = signed.signoff_codec.record.clone();
    stale_record_for_trusted_reviewer_preflight.version += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 501; (validate_bfv_full_bootstrap_release_audit_record_trusted_reviewer_v1( &stale_record_for_trusted_reviewer_preflight, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_record_trusted_reviewer_v1( &stale_record_for_trusted_reviewer_preflight, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )) };
    let mut stale_manifest_for_trusted_reviewer_preflight = manifest_codec.manifest.clone();
    stale_manifest_for_trusted_reviewer_preflight.field_count += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 503; (validate_bfv_full_bootstrap_release_audit_manifest_trusted_reviewer_v1( &stale_manifest_for_trusted_reviewer_preflight, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_manifest_trusted_reviewer_v1( &stale_manifest_for_trusted_reviewer_preflight, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )) };
    let mut stale_package_for_direct_trusted_reviewer_preflight =
        generated_inventory.package.clone();
    stale_package_for_direct_trusted_reviewer_preflight.record_digest =
        Hash::new(b"stale-release-audit-package-record-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 505; (validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1( &stale_package_for_direct_trusted_reviewer_preflight, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1( &stale_package_for_direct_trusted_reviewer_preflight, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )) };
    validate_bfv_full_bootstrap_release_audit_package_for_artifacts_and_trusted_reviewer_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &generated_inventory.package,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    )
    .expect("package matches governed artifacts and trusted reviewer");
    let mut stale_package_for_trusted_reviewer_preflight = generated_inventory.package.clone();
    stale_package_for_trusted_reviewer_preflight.record_digest =
        Hash::new(b"stale-release-audit-package-record-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 507; (validate_bfv_full_bootstrap_release_audit_package_for_artifacts_and_trusted_reviewer_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_for_trusted_reviewer_preflight, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_package_for_artifacts_and_trusted_reviewer_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_for_trusted_reviewer_preflight, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )) };
    assert!(
        matches!(
            validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &generated_inventory.package,
        package_digest,
        "sora-zk-audit-wg-2026",
        signed.review_fixture.reviewer_key_pair.public_key(),
    ),
            Err(BfvError::ProductionQualificationUnavailable(
                BfvProductionQualificationBlockerV1::MissingRegisteredHeOrgLatticeNoiseAndQromEvidence,
            )),
        ),
        "a structurally valid signed package cannot supply missing registered BFV production evidence",
    );
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 509; (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_for_trusted_reviewer_preflight, package_digest, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_public_key, )), (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, Hash::prehashed([0_u8; Hash::LENGTH]), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    for placeholder_preimage in BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_PREIMAGES {
        assert_local_diag! { signed.artifact_fixture.diagnostics; 511 => validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, Hash::new(placeholder_preimage), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ) };
    }
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 512; (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, signed.evidence_codec.delayed_material_placeholder_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, Hash::new(b"stale-release-audit-package-digest"), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, signed.record_codec.record_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    let mut stale_package_with_record_digest_alias = generated_inventory.package.clone();
    stale_package_with_record_digest_alias.manifest_digest =
        Hash::new(b"stale-release-audit-package-manifest-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 515; (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_with_record_digest_alias, signed.record_codec.record_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, manifest_codec.manifest_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    mutation_row! { stale_package_with_manifest_digest_alias = generated_inventory.package.clone(); stale_package_with_manifest_digest_alias.record_digest = Hash::new(b"stale-release-audit-package-record-digest"); assert_local_diag! { signed.artifact_fixture.diagnostics; 517 => validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_with_manifest_digest_alias, manifest_codec.manifest_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ) }; };
    let_row! { signed_commitment_package_digest_aliases = [ ( &generated_inventory.package.record.signoff.payload.release_audit_evidence_digest, "signed release evidence digest", ), ( &generated_inventory.package .record .signoff .payload .centered_scale_round_source_chain_digest, "signed centered scale-round source-chain digest", ), ( &generated_inventory.package.record.signoff.payload.artifact_bundle_digest, "signed artifact bundle digest", ), ( &generated_inventory.package.record.signoff.payload.evaluator_artifact_set_digest, "signed evaluator artifact set digest", ), ( &generated_inventory.package.record.signoff.payload.proof_key_pair_commitment, "signed proof-key pair commitment", ), ( &generated_inventory.package.record.signoff.payload.prover_key_digest, "signed prover-key digest", ), ( &generated_inventory.package.record.signoff.payload.verifier_key_digest, "signed verifier-key digest", ), ( &generated_inventory.package.record.signoff.payload.prover_native_payload_digest, "signed prover native payload digest", ), ( &generated_inventory.package .record .signoff .payload .verifier_native_payload_digest, "signed verifier native payload digest", ), ( &generated_inventory.package.record.signoff.payload.native_circuit_fingerprint, "signed native circuit fingerprint", ), ( &generated_inventory.package.record.signoff.payload.generated_circuit_body_digest, "signed generated circuit body digest", ), ( &generated_inventory.package.record.signoff.payload.audit_report_digest, "signed audit report digest", ), ( &generated_inventory.package.record.signoff.payload.audit_evidence_archive_digest, "signed evidence archive digest", ), ] };
    for (digest, expected) in signed_commitment_package_digest_aliases {
        assert_call! { assert_error_contains; validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &generated_inventory.package, *digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ), signed.artifact_fixture.diagnostics.dynamic_expected_at(518, expected), signed.artifact_fixture.diagnostics.static_context_at(518), };
    }
    mutation_row! { stale_package_with_signed_commitment_alias = generated_inventory.package.clone(); stale_package_with_signed_commitment_alias.manifest_digest = Hash::new(b"stale-release-audit-package-manifest-digest-behind-signed-alias"); assert_local_diag! { signed.artifact_fixture.diagnostics; 519 => validate_release_package_for_artifacts_trusted_reviewer_and_digest_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &stale_package_with_signed_commitment_alias, generated_inventory.package.record.signoff.payload.release_audit_evidence_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ) }; };
    let_row! { alternate_reviewer_key_pair = crate::KeyPair::try_from_seed(vec![0xA8; 32], crate::Algorithm::Ed25519) .expect("fixture seed derives alternate reviewer Ed25519 keypair") };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 520; (validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1( &generated_inventory.package, "sora-zk-audit-wg-2026-alt", signed.review_fixture.reviewer_key_pair.public_key(), )), (validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1( &generated_inventory.package, "sora-zk-audit-wg-2026", alternate_reviewer_key_pair.public_key(), )) };
    let_row! { untrusted_reviewer_package = release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", alternate_reviewer_key_pair.private_key(), ) .expect("alternate reviewer can produce internally valid package") };
    validate_release_package_v1(&untrusted_reviewer_package)
        .expect("alternate reviewer package is self-consistent");
    assert_local_diag! { signed.artifact_fixture.diagnostics; 522 => validate_bfv_full_bootstrap_release_audit_package_for_artifacts_and_trusted_reviewer_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &untrusted_reviewer_package, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ) };
    PackageAuthorities {
        alternate_reviewer_key_pair,
    }
}
