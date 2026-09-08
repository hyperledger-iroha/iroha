//! Release-audit artifact digests assertions and fixture ownership.

use super::*;

pub(super) struct MalformedArtifactBytes {
    pub(super) header_only_audit_report_bytes: Vec<u8>,
    pub(super) header_only_audit_archive_bytes: Vec<u8>,
    pub(super) zero_body_audit_report_bytes: Vec<u8>,
    pub(super) zero_body_audit_archive_bytes: Vec<u8>,
    pub(super) short_body_audit_report_bytes: Vec<u8>,
    pub(super) blank_body_audit_archive_bytes: Vec<u8>,
    pub(super) whitespace_nested_audit_report_bytes: Vec<u8>,
}

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    manifest_codec: &ManifestFrames,
) -> MalformedArtifactBytes {
    let expect_report_digest_rejected = |digest, index| {
        mutation_row! { rejected = signed.signoff_authority.signoff.clone(); rejected.payload.audit_report_digest = digest; assert_local_diag! { signed.artifact_fixture.diagnostics; index => validate_release_signoff_v1(&rejected) }; };
    };
    let expect_archive_digest_rejected = |digest, index| {
        mutation_row! { rejected = manifest_codec.manifest.clone(); rejected.audit_evidence_archive_digest = digest; assert_local_diag! { signed.artifact_fixture.diagnostics; index => validate_release_manifest_v1(&rejected) }; };
    };
    let header_only_audit_report_bytes = BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1.to_vec();
    let header_only_audit_report_digest = Hash::new(&header_only_audit_report_bytes);
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 597; (release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &header_only_audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, header_only_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )) };
    expect_report_digest_rejected(header_only_audit_report_digest, 599);
    let_row! { nested_header_audit_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, ] .concat() };
    let nested_header_audit_report_digest = Hash::new(&nested_header_audit_report_bytes);
    assert_local_diag! { signed.artifact_fixture.diagnostics; 600 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, nested_header_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(nested_header_audit_report_digest, 601);
    let_row! { whitespace_nested_header_audit_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b" \n\t".as_slice(), BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, ] .concat() };
    let_row! { whitespace_nested_header_audit_report_digest = Hash::new(&whitespace_nested_header_audit_report_bytes) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 602 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, whitespace_nested_header_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(whitespace_nested_header_audit_report_digest, 603);
    let_row! { known_zero_body_audit_report_body = [0_u8; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES] };
    let_row! { known_zero_body_audit_report_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, known_zero_body_audit_report_body.as_slice(), ]) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 604 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, known_zero_body_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(known_zero_body_audit_report_digest, 605);
    let_row! { padded_zero_body_audit_report_body = vec![0_u8; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] };
    let_row! { padded_zero_body_audit_report_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, padded_zero_body_audit_report_body.as_slice(), ]) };
    expect_report_digest_rejected(padded_zero_body_audit_report_digest, 606);
    let_row! { known_blank_body_audit_report_body = [b'\n'; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES] };
    let_row! { known_blank_body_audit_report_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, known_blank_body_audit_report_body.as_slice(), ]) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 607 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, known_blank_body_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(known_blank_body_audit_report_digest, 608);
    let_row! { placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"placeholder external audit report", ] .concat(), ) };
    expect_report_digest_rejected(placeholder_audit_report_digest, 609);
    let_row! { draft_audit_report_digest = Hash::new([BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"draft"].concat()) };
    expect_report_digest_rejected(draft_audit_report_digest, 610);
    let_row! { not_ready_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"not-production-ready".as_slice(), ] .concat(), ) };
    expect_report_digest_rejected(not_ready_audit_report_digest, 611);
    mutation_row! { long_placeholder_audit_report_body = b"placeholder external audit report ".to_vec(); long_placeholder_audit_report_body.resize( BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1, b'x', ); let_row! { long_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, long_placeholder_audit_report_body.as_slice(), ] .concat(), ) }; expect_report_digest_rejected(long_placeholder_audit_report_digest, 612); let_row! { mut whitespace_placeholder_audit_report_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] }; whitespace_placeholder_audit_report_body .extend_from_slice(b"placeholder external audit report"); let_row! { whitespace_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, whitespace_placeholder_audit_report_body.as_slice(), ] .concat(), ) }; expect_report_digest_rejected(whitespace_placeholder_audit_report_digest, 613); let_row! { delayed_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_DIGEST_DELAY_PREFIXES[0], b"placeholder external audit report", ] .concat(), ) }; assert_local_diag! { signed.artifact_fixture.diagnostics; 614 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, delayed_placeholder_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) }; };
    expect_report_digest_rejected(delayed_placeholder_audit_report_digest, 615);
    let_row! { binary_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"\xffplaceholder external audit report", ] .concat(), ) };
    expect_report_digest_rejected(binary_placeholder_audit_report_digest, 616);
    let_row! { uppercase_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"PLACEHOLDER EXTERNAL AUDIT REPORT", ] .concat(), ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 617 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, uppercase_placeholder_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(uppercase_placeholder_audit_report_digest, 618);
    let_row! { whitespace_uppercase_placeholder_audit_report_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b" \n\tPLACEHOLDER EXTERNAL AUDIT REPORT", ] .concat(), ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 619 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, whitespace_uppercase_placeholder_audit_report_digest, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_report_digest_rejected(whitespace_uppercase_placeholder_audit_report_digest, 620);
    let_row! { header_only_audit_archive_bytes = BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1.to_vec() };
    let header_only_audit_archive_digest = Hash::new(&header_only_audit_archive_bytes);
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 621; (release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &header_only_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )), (release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, header_only_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), )) };
    expect_archive_digest_rejected(header_only_audit_archive_digest, 623);
    let_row! { nested_header_audit_archive_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, ] .concat() };
    let nested_header_audit_archive_digest = Hash::new(&nested_header_audit_archive_bytes);
    assert_local_diag! { signed.artifact_fixture.diagnostics; 624 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, nested_header_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(nested_header_audit_archive_digest, 625);
    let_row! { whitespace_nested_header_audit_archive_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b" \n\t".as_slice(), BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, ] .concat() };
    let_row! { whitespace_nested_header_audit_archive_digest = Hash::new(&whitespace_nested_header_audit_archive_bytes) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 626 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, whitespace_nested_header_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(whitespace_nested_header_audit_archive_digest, 627);
    let_row! { known_zero_body_audit_archive_body = [0_u8; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES] };
    let_row! { known_zero_body_audit_archive_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, known_zero_body_audit_archive_body.as_slice(), ]) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 628 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, known_zero_body_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(known_zero_body_audit_archive_digest, 629);
    let_row! { known_blank_body_audit_archive_body = [b'\t'; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES] };
    let_row! { known_blank_body_audit_archive_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, known_blank_body_audit_archive_body.as_slice(), ]) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 630 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, known_blank_body_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(known_blank_body_audit_archive_digest, 631);
    let_row! { padded_blank_body_audit_archive_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] };
    let_row! { padded_blank_body_audit_archive_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, padded_blank_body_audit_archive_body.as_slice(), ]) };
    expect_archive_digest_rejected(padded_blank_body_audit_archive_digest, 632);
    let_row! { placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b"\x54\x4f\x44\x4f pending external audit archive", ] .concat(), ) };
    expect_archive_digest_rejected(placeholder_audit_archive_digest, 633);
    mutation_row! { long_placeholder_audit_archive_body = b"\x54\x4f\x44\x4f pending external audit archive ".to_vec(); long_placeholder_audit_archive_body.resize( BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1, b'x', ); let_row! { long_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, long_placeholder_audit_archive_body.as_slice(), ] .concat(), ) }; expect_archive_digest_rejected(long_placeholder_audit_archive_digest, 634); let_row! { mut whitespace_placeholder_audit_archive_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] }; whitespace_placeholder_audit_archive_body .extend_from_slice(b"\x54\x4f\x44\x4f pending external audit archive"); let_row! { whitespace_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, whitespace_placeholder_audit_archive_body.as_slice(), ] .concat(), ) }; expect_archive_digest_rejected(whitespace_placeholder_audit_archive_digest, 635); let_row! { delayed_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_DIGEST_DELAY_PREFIXES[1], b"\x54\x4f\x44\x4f pending external audit archive", ] .concat(), ) }; assert_local_diag! { signed.artifact_fixture.diagnostics; 636 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, delayed_placeholder_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) }; };
    expect_archive_digest_rejected(delayed_placeholder_audit_archive_digest, 637);
    let_row! { binary_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b"\xff\x54\x4f\x44\x4f pending external audit archive", ] .concat(), ) };
    expect_archive_digest_rejected(binary_placeholder_audit_archive_digest, 638);
    let_row! { uppercase_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b"PENDING BFV FULL-BOOTSTRAP AUDIT ARCHIVE", ] .concat(), ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 639 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, uppercase_placeholder_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(uppercase_placeholder_audit_archive_digest, 640);
    let_row! { whitespace_uppercase_placeholder_audit_archive_digest = Hash::new( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b"\r\nPENDING BFV FULL-BOOTSTRAP AUDIT ARCHIVE", ] .concat(), ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 641 => release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, whitespace_uppercase_placeholder_audit_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    expect_archive_digest_rejected(whitespace_uppercase_placeholder_audit_archive_digest, 642);
    mutation_row! { zero_body_audit_report_bytes = BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1.to_vec(); zero_body_audit_report_bytes.extend_from_slice(&[0_u8; 32]); assert_local_diag! { signed.artifact_fixture.diagnostics; 643 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &zero_body_audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) }; };
    mutation_row! { zero_body_audit_archive_bytes = BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1.to_vec(); zero_body_audit_archive_bytes.extend_from_slice(&[0_u8; 32]); assert_local_diag! { signed.artifact_fixture.diagnostics; 644 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &zero_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) }; };
    let_row! { short_body_audit_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"reviewed".as_slice(), ] .concat() };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 645 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &short_body_audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { mut blank_body_audit_archive_bytes = BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1.to_vec() };
    let blank_archive_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES];
    blank_body_audit_archive_bytes.extend_from_slice(&blank_archive_body);
    assert_local_diag! { signed.artifact_fixture.diagnostics; 646 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &blank_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { whitespace_nested_audit_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b" \n\t".as_slice(), BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, b"nested archive artifact header smuggled into the audit report body", ] .concat() };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 647 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &whitespace_nested_audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    MalformedArtifactBytes {
        header_only_audit_report_bytes,
        header_only_audit_archive_bytes,
        zero_body_audit_report_bytes,
        zero_body_audit_archive_bytes,
        short_body_audit_report_bytes,
        blank_body_audit_archive_bytes,
        whitespace_nested_audit_report_bytes,
    }
}
