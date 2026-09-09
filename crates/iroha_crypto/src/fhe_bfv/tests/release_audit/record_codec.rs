//! Release-audit record codec assertions and fixture ownership.

use super::*;

pub(super) struct RecordFrames {
    pub(super) record_bytes: Vec<u8>,
    pub(super) record_digest: Hash,
    pub(super) artifact_bundle_bytes: Vec<u8>,
    pub(super) compressed_artifact_bundle_bytes: Vec<u8>,
    pub(super) compressed_record_bytes: Vec<u8>,
    pub(super) binary_split_placeholder_record_bytes: Vec<u8>,
    pub(super) binary_split_placeholder_artifact_bundle_bytes: Vec<u8>,
}

pub(super) fn check(
    artifact_fixture: &ArtifactFixture,
    evidence_codec: &EvidenceFixtures,
    review_fixture: &ReviewDocuments,
    signoff_authority: &SignoffFixtures,
    signoff_codec: &SignedRecords,
) -> RecordFrames {
    let record_bytes = norito::to_bytes(&signoff_codec.record).expect("encode audit record");
    let_row! { decoded_record = norito::decode_from_bytes::<BfvFullBootstrapReleaseAuditRecordV1>(&record_bytes) .expect("decode audit record") };
    assert_eq_row! { decoded_record, signoff_codec.record, "{}", artifact_fixture.diagnostics.static_context_at(211) };
    let_row! { safely_decoded_record = decode_bfv_full_bootstrap_release_audit_record_bytes_v1(&record_bytes) .expect("safe canonical release audit record decode") };
    assert_eq_row! { safely_decoded_record, signoff_codec.record, "{}", artifact_fixture.diagnostics.static_context_at(212) };
    validate_bfv_full_bootstrap_release_audit_record_for_artifacts_v1(
        &artifact_fixture.params,
        &artifact_fixture.material,
        &artifact_fixture.artifacts,
        &decoded_record,
    )
    .expect("decoded release audit record matches governed artifacts");
    let record_digest =
        release_record_digest_v1(&signoff_codec.record).expect("release audit record digest");
    assert_row! { (record_digest) == (Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_DIGEST_DOMAIN, record_bytes.as_slice(), ])) && (record_digest) == (release_record_digest_v1(&decoded_record) .expect("decoded release audit record digest")) && (record_digest) == (bfv_full_bootstrap_release_audit_record_digest_from_bytes_v1(&record_bytes) .expect("canonical release audit record byte digest")) && (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifacts_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts, &record_bytes, ) .expect("canonical release audit record bytes validate against governed artifacts")) == (signoff_codec.record), "{}", artifact_fixture.diagnostics.group_context(213, 4), };
    let_row! { artifact_bundle_bytes = norito::to_bytes(&artifact_fixture.artifacts).expect("encode release audit source artifacts") };
    let_row! { (decoded_record_artifacts, decoded_record_for_artifact_bytes) = validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_bundle_bytes, &record_bytes, ) .expect("canonical artifact-bundle and record bytes validate together") };
    assert_row! { (decoded_record_artifacts) == (artifact_fixture.artifacts) && (decoded_record_for_artifact_bytes) == (signoff_codec.record), "{}", artifact_fixture.diagnostics.group_context(217, 2), };
    let_row! { ( builder_decoded_record_artifacts, builder_record_for_artifact_bytes, builder_record_digest_for_artifact_bytes, ) = bfv_full_bootstrap_release_audit_record_and_digest_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_bundle_bytes, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), ) .expect("canonical artifact-bundle bytes build release audit record and digest") };
    assert_row! { (builder_decoded_record_artifacts) == (artifact_fixture.artifacts) && (builder_record_for_artifact_bytes) == (signoff_codec.record) && (builder_record_digest_for_artifact_bytes) == (record_digest), "{}", artifact_fixture.diagnostics.group_context(219, 3), };
    let_row! { compressed_artifact_bundle_bytes = norito::to_compressed_bytes(&artifact_fixture.artifacts, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit source artifacts") };
    assert_ne_row! { compressed_artifact_bundle_bytes, artifact_bundle_bytes, "compressed record source artifact-bundle bytes must differ from canonical v1 bytes" };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 222; (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &record_bytes, )), (bfv_full_bootstrap_release_audit_record_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, " sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_record_and_digest_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", &signoff_authority.all_zero_reviewer_private_key, )), (bfv_full_bootstrap_release_audit_record_and_all_digests_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, Hash::prehashed([0_u8; Hash::LENGTH]), review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), )), (bfv_full_bootstrap_release_audit_record_and_digest_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), )) };
    let_row! { compressed_record_bytes = norito::to_compressed_bytes(&signoff_codec.record, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit record") };
    assert_ne_row! { compressed_record_bytes, record_bytes, "compressed release audit record must differ from canonical v1 bytes" };
    let_row! { decoded_compressed_record = decode_trusted_compressed_fixture(&compressed_record_bytes, &signoff_codec.record) .expect("compressed release audit record must decode structurally") };
    assert_eq_row! { decoded_compressed_record, signoff_codec.record, "{}", artifact_fixture.diagnostics.static_context_at(227) };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 228; (decode_bfv_full_bootstrap_release_audit_record_bytes_v1(&compressed_record_bytes)), (bfv_full_bootstrap_release_audit_record_digest_from_bytes_v1( &compressed_record_bytes )), (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifacts_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts, &compressed_record_bytes, )), (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_bundle_bytes, &compressed_record_bytes, )) };
    let_row! { binary_split_placeholder_record_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit record bytes".to_vec() };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 232; (decode_bfv_full_bootstrap_release_audit_record_bytes_v1( &binary_split_placeholder_record_bytes, )), (bfv_full_bootstrap_release_audit_record_digest_from_bytes_v1( &binary_split_placeholder_record_bytes, )), (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifacts_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts, &binary_split_placeholder_record_bytes, )) };
    let_row! { binary_split_placeholder_artifact_bundle_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit record artifact bundle" .to_vec() };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 235; (validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, &record_bytes, )), (bfv_full_bootstrap_release_audit_record_and_digest_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), )) };
    let_row! { stale_record_bytes = norito::to_bytes(&signoff_codec.stale_generated_body_record).expect("encode stale release audit record") };
    assert_local_diag! { artifact_fixture.diagnostics; 237 => validate_bfv_full_bootstrap_release_audit_record_bytes_for_artifacts_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts, &stale_record_bytes, ) };
    assert_ne_row! { record_digest, evidence_codec.digest, "release audit record digest must be domain-separated from evidence digest" };
    RecordFrames {
        record_bytes,
        record_digest,
        artifact_bundle_bytes,
        compressed_artifact_bundle_bytes,
        compressed_record_bytes,
        binary_split_placeholder_record_bytes,
        binary_split_placeholder_artifact_bundle_bytes,
    }
}
