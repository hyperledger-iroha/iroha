//! Release-audit manifest codec assertions and fixture ownership.

use super::*;

pub(super) struct ManifestFrames {
    pub(super) manifest: BfvFullBootstrapReleaseAuditManifestV1,
    pub(super) manifest_digest: Hash,
}

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    generated_inventory: &GeneratedPackages,
) -> ManifestFrames {
    assert_eq_row! { generated_inventory.package.record, signed.signoff_codec.record, "{}", signed.artifact_fixture.diagnostics.static_context_at(418) };
    let manifest =
        release_manifest_v1(&signed.signoff_codec.record).expect("build release audit manifest");
    assert_row! { (generated_inventory.package.manifest) == (manifest) && (manifest.generated_circuit_body_digest) == (bfv_native_stark_digest_binding_hash_v1(signed.signoff_codec.record.evidence.prover_key.generated_circuit_body_digest)) && (manifest.generated_circuit_body_digest) == (bfv_native_stark_digest_binding_hash_v1(signed.signoff_codec.record.evidence.verifier_key.generated_circuit_body_digest)) && (manifest.prover_native_payload_digest) == (bfv_native_stark_digest_binding_hash_v1(signed.signoff_codec.record.evidence.prover_key.native_payload_digest)) && (manifest.verifier_native_payload_digest) == (bfv_native_stark_digest_binding_hash_v1(signed.signoff_codec.record.evidence.verifier_key.native_payload_digest)) && (manifest.centered_scale_round_source_chain_digest) == (signed.signoff_codec.record.evidence.centered_scale_round_source_chain_digest), "{}", signed.artifact_fixture.diagnostics.group_context(419, 6), };
    validate_release_manifest_v1(&manifest).expect("release audit manifest validates");
    validate_bfv_full_bootstrap_release_audit_manifest_for_record_v1(
        &manifest,
        &signed.signoff_codec.record,
    )
    .expect("release audit manifest matches signed record");
    mutation_row! { placeholder_circuit_id_manifest = manifest.clone(); placeholder_circuit_id_manifest.circuit_id = "\x54\x4f\x44\x4f_full-bootstrap_release_manifest_circuit".to_owned(); assert_local_diag! { signed.artifact_fixture.diagnostics; 425 => validate_release_manifest_v1(&placeholder_circuit_id_manifest) }; };
    let manifest_bytes = norito::to_bytes(&manifest).expect("encode audit manifest");
    let_row! { decoded_manifest = norito::decode_from_bytes::<BfvFullBootstrapReleaseAuditManifestV1>(&manifest_bytes) .expect("decode audit manifest") };
    assert_eq_row! { decoded_manifest, manifest, "{}", signed.artifact_fixture.diagnostics.static_context_at(426) };
    let_row! { safely_decoded_manifest = decode_bfv_full_bootstrap_release_audit_manifest_bytes_v1(&manifest_bytes) .expect("safe canonical release audit manifest decode") };
    assert_eq_row! { safely_decoded_manifest, manifest, "{}", signed.artifact_fixture.diagnostics.static_context_at(427) };
    validate_bfv_full_bootstrap_release_audit_manifest_for_record_v1(
        &decoded_manifest,
        &signed.signoff_codec.record,
    )
    .expect("decoded release audit manifest matches signed record");
    let_row! { (decoded_manifest_bytes, decoded_manifest_record_bytes) = validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &manifest_bytes, &signed.record_codec.record_bytes, ) .expect("canonical release audit manifest and record bytes validate together") };
    assert_row! { (decoded_manifest_bytes) == (manifest) && (decoded_manifest_record_bytes) == (signed.signoff_codec.record), "{}", signed.artifact_fixture.diagnostics.group_context(428, 2), };
    let_row! { (trusted_manifest_bytes, trusted_manifest_record_bytes) = validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_trusted_reviewer_v1( &manifest_bytes, &signed.record_codec.record_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.public_key(), ) .expect("canonical release audit manifest/record bytes validate against trusted reviewer") };
    assert_row! { (trusted_manifest_bytes) == (manifest) && (trusted_manifest_record_bytes) == (signed.signoff_codec.record), "{}", signed.artifact_fixture.diagnostics.group_context(430, 2), };
    mutation_row! { stale_native_payload_manifest = manifest.clone(); stale_native_payload_manifest.verifier_native_payload_digest = Hash::new(b"stale-release-audit-manifest-verifier-native-payload-digest"); assert_local_diag! { signed.artifact_fixture.diagnostics; 432 => validate_bfv_full_bootstrap_release_audit_manifest_for_record_v1( &stale_native_payload_manifest, &signed.signoff_codec.record, ) }; };
    let_row! { manifest_reviewer_key_rejections = [ ( "empty", crate::PublicKey(crate::PublicKeyCompact::new(crate::Algorithm::Ed25519, &[])), "payload must not be empty", ), ( "all-zero", crate::PublicKey(crate::PublicKeyCompact::new( crate::Algorithm::Ed25519, &[0_u8; 32], )), "all zero", ), ( "non-Ed25519", signed.signoff_authority.secp256k1_reviewer_key_pair.public_key().clone(), "Ed25519", ), ] };
    for (label, reviewer_public_key, expected_message) in manifest_reviewer_key_rejections {
        let mut malformed_manifest = manifest.clone();
        malformed_manifest.reviewer_public_key = reviewer_public_key;
        let_row! { context = format!("release audit manifests must reject {label} reviewer public-key payloads") };
        assert_call! { assert_error_contains; validate_release_manifest_v1(&malformed_manifest), signed.artifact_fixture.diagnostics.dynamic_expected_at(433, expected_message), signed.artifact_fixture.diagnostics.dynamic_context_at(433, &context), };
        let_row! { context = format!( "release audit manifest digesting must reject {label} reviewer public-key payloads" ) };
        assert_call! { assert_error_contains; release_manifest_digest_v1(&malformed_manifest), signed.artifact_fixture.diagnostics.dynamic_expected_at(434, expected_message), signed.artifact_fixture.diagnostics.dynamic_context_at(434, &context), };
    }
    mutation_row! { placeholder_manifest_commitment = manifest.clone(); placeholder_manifest_commitment.prover_key_digest = Hash::new(b"placeholder full-bootstrap prover-key commitment"); assert_local_diag! { signed.artifact_fixture.diagnostics; 435 => validate_release_manifest_v1(&placeholder_manifest_commitment) }; };
    let_row! { manifest_placeholder_digest_setters: [ManifestDigestSetter; 11] = [ ("record digest", |manifest, digest| { manifest.record_digest = digest; }), ("release evidence digest", |manifest, digest| { manifest.release_audit_evidence_digest = digest; }), ("artifact bundle digest", |manifest, digest| { manifest.artifact_bundle_digest = digest; }), ("evaluator artifact set digest", |manifest, digest| { manifest.evaluator_artifact_set_digest = digest; }), ("proof-key pair commitment", |manifest, digest| { manifest.proof_key_pair_commitment = digest; }), ("prover-key digest", |manifest, digest| { manifest.prover_key_digest = digest; }), ("verifier-key digest", |manifest, digest| { manifest.verifier_key_digest = digest; }), ("native circuit fingerprint", |manifest, digest| { manifest.native_circuit_fingerprint = digest; }), ("generated circuit body digest", |manifest, digest| { manifest.generated_circuit_body_digest = digest; }), ("audit report digest", |manifest, digest| { manifest.audit_report_digest = digest; }), ("evidence archive digest", |manifest, digest| { manifest.audit_evidence_archive_digest = digest; }), ] };
    for (label, set_digest) in manifest_placeholder_digest_setters {
        for placeholder_preimage in BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_PREIMAGES {
            let mut placeholder_manifest = manifest.clone();
            set_digest(&mut placeholder_manifest, Hash::new(placeholder_preimage));
            let_row! { context = format!("release audit manifests must reject placeholder {label} commitments") };
            assert_call! { assert_error_contains; validate_release_manifest_v1(&placeholder_manifest), signed.artifact_fixture.diagnostics.static_expected_at(436), signed.artifact_fixture.diagnostics.dynamic_context_at(436, &context), };
            let_row! { context = format!( "release audit manifest digesting must reject placeholder {label} commitments" ) };
            assert_call! { assert_error_contains; release_manifest_digest_v1(&placeholder_manifest), signed.artifact_fixture.diagnostics.static_expected_at(437), signed.artifact_fixture.diagnostics.dynamic_context_at(437, &context), };
        }
        let mut delayed_placeholder_manifest = manifest.clone();
        set_digest(
            &mut delayed_placeholder_manifest,
            signed.evidence_codec.delayed_material_placeholder_digest,
        );
        let_row! { context = format!("release audit manifests must reject delayed placeholder {label} commitments") };
        assert_call! { assert_error_contains; validate_release_manifest_v1(&delayed_placeholder_manifest), signed.artifact_fixture.diagnostics.static_expected_at(438), signed.artifact_fixture.diagnostics.dynamic_context_at(438, &context), };
        let_row! { context = format!( "release audit manifest digesting must reject delayed placeholder {label} commitments" ) };
        assert_call! { assert_error_contains; release_manifest_digest_v1(&delayed_placeholder_manifest), signed.artifact_fixture.diagnostics.static_expected_at(439), signed.artifact_fixture.diagnostics.dynamic_context_at(439, &context), };
    }
    let_row! { manifest_digest = release_manifest_digest_v1(&manifest).expect("release audit manifest digest") };
    assert_row! { (manifest_digest) == (Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_DIGEST_DOMAIN, manifest_bytes.as_slice(), ])) && (generated_inventory.package.manifest_digest) == (manifest_digest) && (manifest_digest) == (release_manifest_digest_v1(&decoded_manifest) .expect("decoded release audit manifest digest")) && (manifest_digest) == (bfv_full_bootstrap_release_audit_manifest_digest_from_bytes_v1( &manifest_bytes ) .expect("canonical release audit manifest byte digest")), "{}", signed.artifact_fixture.diagnostics.group_context(440, 4), };
    let_row! { ( builder_record_from_record_bytes, builder_manifest_from_record_bytes, builder_manifest_digest_from_record_bytes, ) = bfv_full_bootstrap_release_audit_manifest_and_digest_for_record_bytes_v1(&signed.record_codec.record_bytes) .expect("canonical record bytes build release audit manifest and digest") };
    assert_row! { (builder_record_from_record_bytes) == (signed.signoff_codec.record) && (builder_manifest_from_record_bytes) == (manifest) && (builder_manifest_digest_from_record_bytes) == (manifest_digest), "{}", signed.artifact_fixture.diagnostics.group_context(444, 3), };
    let_row! { ( builder_manifest_artifacts_from_artifact_bytes, builder_manifest_record_from_artifact_bytes, builder_manifest_from_artifact_bytes, builder_manifest_digest_from_artifact_bytes, ) = bfv_full_bootstrap_release_audit_manifest_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("canonical artifact-bundle bytes build release audit manifest and digest") };
    assert_row! { (builder_manifest_artifacts_from_artifact_bytes) == (signed.artifact_fixture.artifacts) && (builder_manifest_record_from_artifact_bytes) == (signed.signoff_codec.record) && (builder_manifest_from_artifact_bytes) == (manifest) && (builder_manifest_digest_from_artifact_bytes) == (manifest_digest), "{}", signed.artifact_fixture.diagnostics.group_context(447, 4), };
    let_row! { ( builder_manifest_digests_artifacts_from_artifact_bytes, builder_manifest_digests_record_from_artifact_bytes, builder_manifest_from_artifact_bytes_with_digests, builder_manifest_artifact_bundle_digest_from_artifact_bytes, builder_manifest_record_digest_from_artifact_bytes, builder_manifest_digest_from_artifact_bytes_with_digests, ) = bfv_full_bootstrap_release_audit_manifest_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("canonical artifact-bundle bytes build release audit manifest and all digests") };
    assert_row! { (builder_manifest_digests_artifacts_from_artifact_bytes) == (signed.artifact_fixture.artifacts) && (builder_manifest_digests_record_from_artifact_bytes) == (signed.signoff_codec.record) && (builder_manifest_from_artifact_bytes_with_digests) == (manifest) && (builder_manifest_artifact_bundle_digest_from_artifact_bytes) == (generated_inventory.package.record.evidence.artifact_bundle_digest) && (builder_manifest_record_digest_from_artifact_bytes) == (generated_inventory.package.record_digest) && (builder_manifest_digest_from_artifact_bytes_with_digests) == (manifest_digest), "{}", signed.artifact_fixture.diagnostics.group_context(451, 6), };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 457; (bfv_full_bootstrap_release_audit_manifest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, " sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_manifest_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", &signed.signoff_authority.all_zero_reviewer_private_key, )), (bfv_full_bootstrap_release_audit_manifest_and_all_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, Hash::prehashed([0_u8; Hash::LENGTH]), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_manifest_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_manifest_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.compressed_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_manifest_and_digest_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_manifest_and_digests_for_artifact_bundle_bytes_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.record_codec.binary_split_placeholder_artifact_bundle_bytes, signed.review_fixture.audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )) };
    let_row! { compressed_manifest_bytes = norito::to_compressed_bytes(&manifest, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit manifest") };
    assert_ne_row! { compressed_manifest_bytes, manifest_bytes, "compressed release audit manifest must differ from canonical v1 bytes" };
    let_row! { decoded_compressed_manifest = decode_trusted_compressed_fixture(&compressed_manifest_bytes, &manifest) .expect("compressed release audit manifest must decode structurally") };
    assert_eq_row! { decoded_compressed_manifest, manifest, "{}", signed.artifact_fixture.diagnostics.static_context_at(464) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 465; (decode_bfv_full_bootstrap_release_audit_manifest_bytes_v1(&compressed_manifest_bytes)), (bfv_full_bootstrap_release_audit_manifest_digest_from_bytes_v1( &compressed_manifest_bytes, )), (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &compressed_manifest_bytes, &signed.record_codec.record_bytes, )), (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &manifest_bytes, &signed.record_codec.compressed_record_bytes, )), (bfv_full_bootstrap_release_audit_manifest_and_digest_for_record_bytes_v1( &signed.record_codec.compressed_record_bytes, )) };
    let_row! { binary_split_placeholder_manifest_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit manifest bytes".to_vec() };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 470; (decode_bfv_full_bootstrap_release_audit_manifest_bytes_v1( &binary_split_placeholder_manifest_bytes, )), (bfv_full_bootstrap_release_audit_manifest_digest_from_bytes_v1( &binary_split_placeholder_manifest_bytes, )), (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &binary_split_placeholder_manifest_bytes, &signed.record_codec.record_bytes, )), (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &manifest_bytes, &signed.record_codec.binary_split_placeholder_record_bytes, )), (bfv_full_bootstrap_release_audit_manifest_and_digest_for_record_bytes_v1( &signed.record_codec.binary_split_placeholder_record_bytes, )) };
    let_row! { stale_native_payload_manifest_bytes = norito::to_bytes(&stale_native_payload_manifest) .expect("encode stale native-payload release audit manifest") };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 475; (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_v1( &stale_native_payload_manifest_bytes, &signed.record_codec.record_bytes, )), (validate_bfv_full_bootstrap_release_audit_manifest_bytes_for_record_bytes_trusted_reviewer_v1( &manifest_bytes, &signed.record_codec.record_bytes, "sora-zk-audit-wg-2027", signed.review_fixture.reviewer_key_pair.public_key(), )) };
    assert_ne_row! { manifest_digest, signed.record_codec.record_digest, "release audit manifest digest must be domain-separated from record digest" };
    validate_release_package_v1(&generated_inventory.package)
        .expect("release audit package validates");
    validate_release_package_for_artifacts_v1(
        &signed.artifact_fixture.params,
        &signed.artifact_fixture.material,
        &signed.artifact_fixture.artifacts,
        &generated_inventory.package,
    )
    .expect("release audit package matches governed artifacts");
    ManifestFrames {
        manifest,
        manifest_digest,
    }
}
