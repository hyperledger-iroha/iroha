//! Release-audit artifact bodies assertions and fixture ownership.

use super::*;

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    generated_inventory: &GeneratedPackages,
    artifact_digests: MalformedArtifactBytes,
) {
    let_row! { copied_audit_body_prefix = b"copied-full-bootstrap-release-audit-report-and-archive-body-shared-evidence" };
    let_row! { copied_commitment_markers = [ b"; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; artifact-bundle-digest=".as_slice(), signed.review_fixture.artifact_bundle_digest_hex.as_bytes(), b"; evaluator-artifact-set-digest=".as_slice(), signed.review_fixture.evaluator_artifact_set_digest_hex.as_bytes(), b"; centered-source-chain-digest=".as_slice(), signed.review_fixture.centered_source_chain_digest_hex.as_bytes(), b"; arithmetic-trace-profile-digest=".as_slice(), signed.review_fixture.arithmetic_trace_profile_digest_hex.as_bytes(), b"; arithmetic-air-constraint-system-digest=".as_slice(), signed.review_fixture.arithmetic_air_constraint_system_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; generated-circuit-body-byte-length=".as_slice(), signed.review_fixture.generated_circuit_body_byte_length.as_bytes(), b"; generated-circuit-body-hex=".as_slice(), signed.review_fixture.generated_circuit_body_hex.as_bytes(), b"; coefficient-to-slot-key-artifact-hex=".as_slice(), signed.review_fixture.coefficient_to_slot_key_artifact_hex.as_bytes(), b"; slot-to-coefficient-key-artifact-hex=".as_slice(), signed.review_fixture.slot_to_coefficient_key_artifact_hex.as_bytes(), b"; blind-rotation-key-artifact-hex=".as_slice(), signed.review_fixture.blind_rotation_key_artifact_hex.as_bytes(), b"; extraction-key-artifact-hex=".as_slice(), signed.review_fixture.sample_extraction_key_artifact_hex.as_bytes(), b"; accumulator-artifact-hex=".as_slice(), signed.review_fixture.accumulator_artifact_hex.as_bytes(), b"; proof-public-input-schema-artifact-hex=".as_slice(), signed.review_fixture.proof_public_input_schema_artifact_hex.as_bytes(), b"; arithmetic-air-constraint-system-artifact-hex=".as_slice(), signed.review_fixture.arithmetic_air_constraint_system_artifact_hex.as_bytes(), b"; native-prover-payload-hex=".as_slice(), signed.review_fixture.native_prover_payload_hex.as_bytes(), b"; prover-native-payload-digest=".as_slice(), signed.review_fixture.prover_native_payload_digest_hex.as_bytes(), b"; native-verifier-payload-hex=".as_slice(), signed.review_fixture.native_verifier_payload_hex.as_bytes(), b"; verifier-native-payload-digest=".as_slice(), signed.review_fixture.verifier_native_payload_digest_hex.as_bytes(), b"; prover-key-artifact-hex=".as_slice(), signed.review_fixture.prover_key_artifact_hex.as_bytes(), b"; verifier-key-artifact-hex=".as_slice(), signed.review_fixture.verifier_key_artifact_hex.as_bytes(), b"; native-circuit-fingerprint=".as_slice(), signed.review_fixture.native_circuit_fingerprint_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), b"; prover-key-digest=".as_slice(), signed.review_fixture.prover_key_digest_hex.as_bytes(), b"; verifier-key-digest=".as_slice(), signed.review_fixture.verifier_key_digest_hex.as_bytes(), ] .concat() };
    let_row! { copied_audit_body = [ copied_audit_body_prefix.as_slice(), copied_commitment_markers.as_slice(), ] .concat() };
    let_row! { copied_body_audit_report_bytes = release_report_bytes_v1(&copied_audit_body).expect("canonical copied-body report bytes") };
    let_row! { copied_body_audit_archive_bytes = release_archive_bytes_v1(&copied_audit_body).expect("canonical copied-body archive bytes") };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 648 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &copied_body_audit_report_bytes, &copied_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    mutation_row! { stale_material_for_copied_body_preflight = signed.artifact_fixture.material.clone(); stale_material_for_copied_body_preflight.circuit_id = "stale-bfv-full-bootstrap-circuit".to_owned(); assert_local_diag! { signed.artifact_fixture.diagnostics; 649 => release_package_v1( &signed.artifact_fixture.params, &stale_material_for_copied_body_preflight, &signed.artifact_fixture.artifacts, &copied_body_audit_report_bytes, &copied_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) }; };
    let_row! { whitespace_decorated_copied_audit_body = [ b" \n\t".as_slice(), copied_audit_body.as_slice(), b"\t\n ".as_slice(), ] .concat() };
    let_row! { whitespace_copied_body_audit_archive_bytes = release_archive_bytes_v1(&whitespace_decorated_copied_audit_body) .expect("canonical whitespace-decorated copied-body archive bytes") };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 650 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &copied_body_audit_report_bytes, &whitespace_copied_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let mut internally_whitespace_decorated_copied_audit_body = Vec::new();
    for &byte in copied_audit_body_prefix.as_slice() {
        internally_whitespace_decorated_copied_audit_body.push(byte);
        internally_whitespace_decorated_copied_audit_body.extend_from_slice(b" \n");
    }
    internally_whitespace_decorated_copied_audit_body.extend_from_slice(&copied_commitment_markers);
    let_row! { internally_whitespace_copied_body_audit_archive_bytes = release_archive_bytes_v1(&internally_whitespace_decorated_copied_audit_body) .expect("canonical internally whitespace-decorated copied-body archive bytes") };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 651 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &copied_body_audit_report_bytes, &internally_whitespace_copied_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let mut alnum_decorated_copied_audit_body = Vec::new();
    for &byte in copied_audit_body_prefix.as_slice() {
        alnum_decorated_copied_audit_body.push(byte.to_ascii_uppercase());
        if byte.is_ascii_alphanumeric() {
            alnum_decorated_copied_audit_body.extend_from_slice(b".-");
        }
    }
    alnum_decorated_copied_audit_body.extend_from_slice(&copied_commitment_markers);
    let_row! { alnum_copied_body_audit_archive_bytes = release_archive_bytes_v1(&alnum_decorated_copied_audit_body) .expect("canonical alnum-decorated copied-body archive bytes") };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 652 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &copied_body_audit_report_bytes, &alnum_copied_body_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { signed_package_with_audit_artifacts = |audit_report_bytes: Vec<u8>, audit_evidence_archive_bytes: Vec<u8>| -> BfvFullBootstrapReleaseAuditPackageV1 { let record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, Hash::new(audit_report_bytes.as_slice()), Hash::new(audit_evidence_archive_bytes.as_slice()), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign release audit record with adversarial artifacts"); let manifest = release_manifest_v1(&record).expect("build adversarial release audit manifest"); BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&record) .expect("digest adversarial release audit record"), manifest_digest: release_manifest_digest_v1(&manifest) .expect("digest adversarial release audit manifest"), record, manifest, audit_report_bytes, audit_evidence_archive_bytes, } } };
    let_row! { internally_whitespace_copied_body_package = signed_package_with_audit_artifacts( copied_body_audit_report_bytes.clone(), internally_whitespace_copied_body_audit_archive_bytes, ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 653; (validate_release_package_v1(&internally_whitespace_copied_body_package)), (release_package_digest_v1(&internally_whitespace_copied_body_package)) };
    let_row! { alnum_copied_body_package = signed_package_with_audit_artifacts( copied_body_audit_report_bytes.clone(), alnum_copied_body_audit_archive_bytes, ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 655; (validate_release_package_v1(&alnum_copied_body_package)), (release_package_digest_v1(&alnum_copied_body_package)) };
    let_row! { mut delayed_placeholder_report_body = vec![b'x'; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 32] };
    delayed_placeholder_report_body.extend_from_slice(b" delayed placeholder audit report");
    let_row! { delayed_placeholder_report_package = signed_package_with_audit_artifacts( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, delayed_placeholder_report_body.as_slice(), ] .concat(), signed.review_fixture.audit_evidence_archive_bytes.clone(), ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 657; (validate_release_package_v1(&delayed_placeholder_report_package)), (release_package_digest_v1(&delayed_placeholder_report_package)) };
    let_row! { mut delayed_placeholder_archive_body = vec![b'x'; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 32] };
    delayed_placeholder_archive_body.extend_from_slice(b" delayed pending external audit archive");
    let_row! { delayed_placeholder_archive_package = signed_package_with_audit_artifacts( signed.review_fixture.audit_report_bytes.clone(), [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, delayed_placeholder_archive_body.as_slice(), ] .concat(), ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 659; (validate_release_package_v1(&delayed_placeholder_archive_package)), (release_package_digest_v1(&delayed_placeholder_archive_package)) };
    macro_rules! expect_package_artifact_rejected {
        ($field:ident, $value:expr, $index:literal) => {{
            let mut rejected = generated_inventory.package.clone();
            rejected.$field = $value;
            assert_error_matrix_row! { signed.artifact_fixture.diagnostics; $index;
                (validate_release_package_v1(&rejected)),
                (release_package_digest_v1(&rejected))
            };
        }};
        (report_body: $body:expr, $index:literal) => {
            expect_package_artifact_rejected!(
                audit_report_bytes,
                [BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, $body].concat(),
                $index
            )
        };
        (archive_body: $body:expr, $index:literal) => {
            expect_package_artifact_rejected!(
                audit_evidence_archive_bytes,
                [BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, $body].concat(),
                $index
            )
        };
        (single $field:ident, $value:expr, $index:literal) => {{
            mutation_row! { rejected = generated_inventory.package.clone(); rejected.$field = $value; assert_local_diag! { signed.artifact_fixture.diagnostics; $index => validate_release_package_v1(&rejected) }; };
        }};
        (single report_body: $body:expr, $index:literal) => {
            expect_package_artifact_rejected!(
                single audit_report_bytes,
                [BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, $body].concat(),
                $index
            )
        };
        (single archive_body: $body:expr, $index:literal) => {
            expect_package_artifact_rejected!(
                single audit_evidence_archive_bytes,
                [BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, $body].concat(),
                $index
            )
        };
    }
    expect_package_artifact_rejected!(report_body: b"placeholder external audit report", 661);
    expect_package_artifact_rejected!(
        archive_body: b"\x54\x4f\x44\x4f pending external audit archive",
        663
    );
    expect_package_artifact_rejected!(
        report_body: b"external review draft not for production release report body v1",
        665
    );
    expect_package_artifact_rejected!(
        archive_body: b"external review evidence archive replace before production body v1",
        667
    );
    expect_package_artifact_rejected!(
        report_body: b"external review not-production-ready release report body v1",
        669
    );
    expect_package_artifact_rejected!(
        archive_body: b"external review evidence archive replace_before_production body v1",
        671
    );
    mutation_row! { long_placeholder_report_body = b"placeholder external audit report ".to_vec(); long_placeholder_report_body.resize( BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1, b'x', ); assert_local_diag! { signed.artifact_fixture.diagnostics; 673 => release_report_bytes_v1(&long_placeholder_report_body) }; };
    expect_package_artifact_rejected!(
        single report_body: long_placeholder_report_body.as_slice(),
        674
    );
    mutation_row! { long_placeholder_archive_body = b"\x54\x4f\x44\x4f pending external audit archive ".to_vec(); long_placeholder_archive_body.resize( BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1, b'x', ); assert_local_diag! { signed.artifact_fixture.diagnostics; 675 => release_archive_bytes_v1(&long_placeholder_archive_body) }; };
    expect_package_artifact_rejected!(
        single archive_body: long_placeholder_archive_body.as_slice(),
        676
    );
    mutation_row! { whitespace_padded_placeholder_report_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1]; whitespace_padded_placeholder_report_body .extend_from_slice(b"placeholder external audit report"); assert_local_diag! { signed.artifact_fixture.diagnostics; 677 => release_report_bytes_v1(&whitespace_padded_placeholder_report_body) }; };
    expect_package_artifact_rejected!(
        single report_body: whitespace_padded_placeholder_report_body.as_slice(),
        678
    );
    mutation_row! { whitespace_padded_placeholder_archive_body = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1]; whitespace_padded_placeholder_archive_body .extend_from_slice(b"\x54\x4f\x44\x4f pending external audit archive"); expect_package_artifact_rejected!( single archive_body: whitespace_padded_placeholder_archive_body.as_slice(), 679 ); let_row! { mut binary_prefixed_placeholder_report_body = b"\xffplaceholder external audit report".to_vec() }; binary_prefixed_placeholder_report_body .resize(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES, b'x'); assert_local_diag! { signed.artifact_fixture.diagnostics; 680 => release_report_bytes_v1(&binary_prefixed_placeholder_report_body) }; };
    expect_package_artifact_rejected!(
        single report_body: binary_prefixed_placeholder_report_body.as_slice(),
        681
    );
    mutation_row! { binary_prefixed_placeholder_archive_body = b"\xff\x54\x4f\x44\x4f pending external audit archive".to_vec(); binary_prefixed_placeholder_archive_body.resize( BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES, b'x', ); expect_package_artifact_rejected!( single archive_body: binary_prefixed_placeholder_archive_body.as_slice(), 682 ); let_row! { mut binary_fragmented_placeholder_report_body = b"reviewer metadata\xffoperator your.proof audit report body".to_vec() }; binary_fragmented_placeholder_report_body .resize(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES, b'x'); assert_local_diag! { signed.artifact_fixture.diagnostics; 683 => release_report_bytes_v1(&binary_fragmented_placeholder_report_body) }; };
    expect_package_artifact_rejected!(
        report_body: binary_fragmented_placeholder_report_body.as_slice(),
        684
    );
    mutation_row! { binary_split_placeholder_report_body = b"reviewer metadata\xffoperator your\xffproof audit report body".to_vec(); binary_split_placeholder_report_body .resize(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES, b'x'); assert_local_diag! { signed.artifact_fixture.diagnostics; 686 => release_report_bytes_v1(&binary_split_placeholder_report_body) }; };
    expect_package_artifact_rejected!(
        report_body: binary_split_placeholder_report_body.as_slice(),
        687
    );
    let_row! { mut binary_fragmented_placeholder_archive_body = b"reviewer metadata\xffoperator your.proof evidence archive body".to_vec() };
    binary_fragmented_placeholder_archive_body.resize(
        BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES,
        b'x',
    );
    expect_package_artifact_rejected!(
        archive_body: binary_fragmented_placeholder_archive_body.as_slice(),
        689
    );
    let_row! { mut binary_split_placeholder_archive_body = b"reviewer metadata\xffoperator your\xffproof evidence archive body".to_vec() };
    binary_split_placeholder_archive_body.resize(
        BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES,
        b'x',
    );
    expect_package_artifact_rejected!(
        archive_body: binary_split_placeholder_archive_body.as_slice(),
        691
    );
    let_row! { short_body_report_package = signed_package_with_audit_artifacts( artifact_digests.short_body_audit_report_bytes, signed.review_fixture.audit_evidence_archive_bytes.clone(), ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 693; (validate_release_package_v1(&short_body_report_package)), (release_package_digest_v1(&short_body_report_package)) };
    expect_package_artifact_rejected!(
        audit_evidence_archive_bytes,
        artifact_digests.blank_body_audit_archive_bytes,
        695
    );
    let_row! { whitespace_nested_report_package = signed_package_with_audit_artifacts( artifact_digests.whitespace_nested_audit_report_bytes, signed.review_fixture.audit_evidence_archive_bytes.clone(), ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 697; (validate_release_package_v1(&whitespace_nested_report_package)), (release_package_digest_v1(&whitespace_nested_report_package)) };
    expect_package_artifact_rejected!(
        audit_report_bytes,
        artifact_digests.header_only_audit_report_bytes,
        699
    );
    expect_package_artifact_rejected!(
        audit_evidence_archive_bytes,
        artifact_digests.header_only_audit_archive_bytes,
        701
    );
    expect_package_artifact_rejected!(
        audit_report_bytes,
        artifact_digests.zero_body_audit_report_bytes,
        703
    );
    expect_package_artifact_rejected!(
        audit_evidence_archive_bytes,
        artifact_digests.zero_body_audit_archive_bytes,
        705
    );
    let_row! { copied_body_package = signed_package_with_audit_artifacts( copied_body_audit_report_bytes, copied_body_audit_archive_bytes, ) };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 707; (validate_release_package_v1(&copied_body_package)), (release_package_digest_v1(&copied_body_package)) };
    let unheadered_audit_report_bytes = b"external-review-ok-without-v1-header".to_vec();
    assert_local_diag! { signed.artifact_fixture.diagnostics; 709 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &unheadered_audit_report_bytes, &signed.review_fixture.audit_evidence_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { unheadered_report_record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, Hash::new(&unheadered_audit_report_bytes), signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign unheadered-report release audit record") };
    let_row! { unheadered_report_manifest = release_manifest_v1(&unheadered_report_record) .expect("build unheadered-report release audit manifest") };
    let_row! { unheadered_report_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&unheadered_report_record) .expect("digest unheadered-report record"), manifest_digest: release_manifest_digest_v1(&unheadered_report_manifest) .expect("digest unheadered-report manifest"), record: unheadered_report_record, manifest: unheadered_report_manifest, audit_report_bytes: unheadered_audit_report_bytes, audit_evidence_archive_bytes: signed.review_fixture.audit_evidence_archive_bytes.clone(), } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 710 => validate_release_package_v1(&unheadered_report_package) };
    let unheadered_audit_archive_bytes = b"evidence-bundle-ok-without-v1-header".to_vec();
    assert_local_diag! { signed.artifact_fixture.diagnostics; 711 => release_package_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &signed.review_fixture.audit_report_bytes, &unheadered_audit_archive_bytes, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) };
    let_row! { unheadered_archive_record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, Hash::new(&unheadered_audit_archive_bytes), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign unheadered-archive release audit record") };
    let_row! { unheadered_archive_manifest = release_manifest_v1(&unheadered_archive_record) .expect("build unheadered-archive release audit manifest") };
    let_row! { unheadered_archive_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&unheadered_archive_record) .expect("digest unheadered-archive record"), manifest_digest: release_manifest_digest_v1(&unheadered_archive_manifest) .expect("digest unheadered-archive manifest"), record: unheadered_archive_record, manifest: unheadered_archive_manifest, audit_report_bytes: signed.review_fixture.audit_report_bytes.clone(), audit_evidence_archive_bytes: unheadered_audit_archive_bytes, } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 712 => validate_release_package_v1(&unheadered_archive_package) };
    let zero_audit_report_bytes = vec![0_u8; 32];
    let_row! { zero_report_record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, Hash::new(&zero_audit_report_bytes), signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign zero-report release audit record") };
    let_row! { zero_report_manifest = release_manifest_v1(&zero_report_record).expect("build zero-report release audit manifest") };
    let_row! { zero_report_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&zero_report_record) .expect("digest zero-report record"), manifest_digest: release_manifest_digest_v1(&zero_report_manifest) .expect("digest zero-report manifest"), record: zero_report_record, manifest: zero_report_manifest, audit_report_bytes: zero_audit_report_bytes, audit_evidence_archive_bytes: signed.review_fixture.audit_evidence_archive_bytes.clone(), } };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 713; (validate_release_package_v1(&zero_report_package)), (release_package_digest_v1(&zero_report_package)) };
    let zero_audit_archive_bytes = vec![0_u8; 32];
    let_row! { zero_archive_record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, signed.review_fixture.audit_report_digest, Hash::new(&zero_audit_archive_bytes), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign zero-archive release audit record") };
    let_row! { zero_archive_manifest = release_manifest_v1(&zero_archive_record) .expect("build zero-archive release audit manifest") };
    let_row! { zero_archive_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&zero_archive_record) .expect("digest zero-archive record"), manifest_digest: release_manifest_digest_v1(&zero_archive_manifest) .expect("digest zero-archive manifest"), record: zero_archive_record, manifest: zero_archive_manifest, audit_report_bytes: signed.review_fixture.audit_report_bytes.clone(), audit_evidence_archive_bytes: zero_audit_archive_bytes, } };
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 715; (validate_release_package_v1(&zero_archive_package)), (release_package_digest_v1(&zero_archive_package)) };
    mutation_row! { stale_signoff_version = signed.signoff_authority.signoff.clone(); stale_signoff_version.version += 1; assert_local_diag! { signed.artifact_fixture.diagnostics; 717 => validate_release_signoff_v1(&stale_signoff_version) }; };
    let mut stale_record_version = signed.signoff_codec.record.clone();
    stale_record_version.version += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 718; (validate_bfv_full_bootstrap_release_audit_record_v1(&stale_record_version)), (release_record_digest_v1(&stale_record_version)) };
    let mut stale_record_field_count = signed.signoff_codec.record.clone();
    stale_record_field_count.field_count += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 720; (validate_bfv_full_bootstrap_release_audit_record_v1(&stale_record_field_count)), (release_record_digest_v1(&stale_record_field_count)) };
    mutation_row! { stale_package_version = generated_inventory.package.clone(); stale_package_version.version += 1; assert_local_diag! { signed.artifact_fixture.diagnostics; 722 => validate_release_package_v1(&stale_package_version) }; };
    let mut stale_package_field_count = generated_inventory.package.clone();
    stale_package_field_count.field_count += 1;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 723; (validate_release_package_v1(&stale_package_field_count)), (release_package_digest_v1(&stale_package_field_count)) };
    let mut stale_package_record_digest = generated_inventory.package.clone();
    stale_package_record_digest.record_digest = Hash::new(b"stale-release-audit-record-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 725; (validate_release_package_v1(&stale_package_record_digest)), (release_package_digest_v1(&stale_package_record_digest)) };
    let mut aliased_package_record_manifest_digests = generated_inventory.package.clone();
    aliased_package_record_manifest_digests.record.version += 1;
    aliased_package_record_manifest_digests.manifest_digest =
        aliased_package_record_manifest_digests.record_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 727; (validate_release_package_v1(&aliased_package_record_manifest_digests)), (release_package_digest_v1(&aliased_package_record_manifest_digests)) };
    let mut tampered_report_package = generated_inventory.package.clone();
    tampered_report_package
        .audit_report_bytes
        .extend_from_slice(b":tampered");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 729; (validate_release_package_v1(&tampered_report_package)), (release_package_digest_v1(&tampered_report_package)) };
    let mut tampered_archive_package = generated_inventory.package.clone();
    tampered_archive_package
        .audit_evidence_archive_bytes
        .extend_from_slice(b":tampered");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 731; (validate_release_package_v1(&tampered_archive_package)), (release_package_digest_v1(&tampered_archive_package)) };
}
