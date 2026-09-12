//! Release-audit report commitments assertions and fixture ownership.

use super::*;

pub(super) struct ReportCommitments {
    pub(super) proof_profile_field_count_fragment: String,
    pub(super) stale_proof_profile_field_count_fragment: String,
    pub(super) proof_profile_public_opening_material_version_fragment: String,
    pub(super) stale_proof_profile_public_opening_material_version_fragment: String,
    pub(super) proof_profile_public_opening_material_field_count_fragment: String,
    pub(super) proof_profile_n_log2_fragment: String,
    pub(super) proof_profile_blowup_log2_fragment: String,
    pub(super) stale_proof_profile_blowup_log2_fragment: String,
    pub(super) proof_profile_fold_arity_fragment: String,
    pub(super) stale_proof_profile_fold_arity_fragment: String,
    pub(super) proof_profile_merkle_arity_fragment: String,
    pub(super) stale_proof_profile_merkle_arity_fragment: String,
    pub(super) proof_profile_requires_base_fragment: String,
    pub(super) stale_proof_profile_requires_base_fragment: &'static str,
    pub(super) proof_profile_rejects_suffixed_fragment: String,
    pub(super) stale_proof_profile_rejects_suffixed_fragment: &'static str,
    pub(super) proof_profile_requires_verifier_trace_digest_fragment: String,
    pub(super) stale_proof_profile_requires_verifier_trace_digest_fragment: &'static str,
    pub(super) proof_profile_validates_public_opening_fragment: String,
    pub(super) stale_proof_profile_validates_public_opening_fragment: &'static str,
    pub(super) proof_profile_validates_merkle_path_roots_fragment: String,
    pub(super) stale_proof_profile_validates_merkle_path_roots_fragment: &'static str,
    pub(super) proof_profile_air_constraint_domain_fragment: String,
    pub(super) proof_profile_public_opening_domain_fragment: String,
    pub(super) stale_proof_profile_public_opening_domain_fragment: &'static str,
    pub(super) proof_profile_separates_release_prover_fragment: String,
    pub(super) stale_proof_profile_separates_release_prover_fragment: &'static str,
    pub(super) arithmetic_trace_profile_digest_fragment: String,
    pub(super) arithmetic_air_constraint_system_digest_fragment: String,
}

pub(super) fn check(signed: &SignedAuditInputs<'_>) -> ReportCommitments {
    let arithmetic_air_constraint_system_digest_hex = &signed
        .review_fixture
        .arithmetic_air_constraint_system_digest_hex;
    let arithmetic_trace_profile_digest_hex =
        &signed.review_fixture.arithmetic_trace_profile_digest_hex;
    let proof_profile_blowup_log2 = &signed.review_fixture.proof_profile_blowup_log2;
    let proof_profile_field_count = &signed.review_fixture.proof_profile_field_count;
    let proof_profile_fold_arity = &signed.review_fixture.proof_profile_fold_arity;
    let proof_profile_merkle_arity = &signed.review_fixture.proof_profile_merkle_arity;
    let proof_profile_n_log2 = &signed.review_fixture.proof_profile_n_log2;
    let proof_profile_public_opening_material_field_count = &signed
        .review_fixture
        .proof_profile_public_opening_material_field_count;
    let proof_profile_public_opening_material_version = &signed
        .review_fixture
        .proof_profile_public_opening_material_version;
    let proof_profile_queries = &signed.review_fixture.proof_profile_queries;
    let proof_profile_rejects_suffixed_transcript_label_aliases = &signed
        .review_fixture
        .proof_profile_rejects_suffixed_transcript_label_aliases;
    let proof_profile_requires_canonical_base_transcript_label = &signed
        .review_fixture
        .proof_profile_requires_canonical_base_transcript_label;
    let proof_profile_requires_verifier_owned_trace_material_digest = &signed
        .review_fixture
        .proof_profile_requires_verifier_owned_trace_material_digest;
    let proof_profile_validates_fri_query_chain = &signed
        .review_fixture
        .proof_profile_validates_fri_query_chain;
    let proof_profile_validates_merkle_path_roots = &signed
        .review_fixture
        .proof_profile_validates_merkle_path_roots;
    let proof_profile_validates_transcript_public_opening_material = &signed
        .review_fixture
        .proof_profile_validates_transcript_public_opening_material;

    let_row! { unsigned_report_without_release_evidence_digest = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; reviewed generated circuit and proof-key material", ] .concat() };
    let_row! { signed_report_without_release_evidence_digest_record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, Hash::new(&unsigned_report_without_release_evidence_digest), signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("build signed record for report missing release evidence digest") };
    let_row! { signed_report_without_release_evidence_digest_manifest = release_manifest_v1(&signed_report_without_release_evidence_digest_record) .expect("build manifest for report missing release evidence digest") };
    let_row! { signed_report_without_release_evidence_digest_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1( &signed_report_without_release_evidence_digest_record, ) .expect("digest record for report missing release evidence digest"), manifest_digest: release_manifest_digest_v1( &signed_report_without_release_evidence_digest_manifest, ) .expect("digest manifest for report missing release evidence digest"), record: signed_report_without_release_evidence_digest_record, manifest: signed_report_without_release_evidence_digest_manifest, audit_report_bytes: unsigned_report_without_release_evidence_digest, audit_evidence_archive_bytes: signed.review_fixture.audit_evidence_archive_bytes.clone(), } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 299 => validate_release_package_v1(&signed_report_without_release_evidence_digest_package) };
    let_row! { signed_package_with_report_bytes = |audit_report_bytes: Vec<u8>, context: &str| -> BfvFullBootstrapReleaseAuditPackageV1 { let report_commitment = Hash::new(&audit_report_bytes); let record = release_record_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, report_commitment, signed.review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect(context); let manifest = release_manifest_v1(&record).expect("build manifest for signed report package"); BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record_digest: release_record_digest_v1(&record) .expect("digest record for signed report package"), manifest_digest: release_manifest_digest_v1(&manifest) .expect("digest manifest for signed report package"), record, manifest, audit_report_bytes, audit_evidence_archive_bytes: signed.review_fixture.audit_evidence_archive_bytes.clone(), } } };

    let_row! { stale_proof_profile_field_count = signed.artifact_fixture.evidence .proof_profile .field_count .saturating_add(1) .to_string() };
    let_row! { stale_proof_profile_public_opening_material_version = signed.artifact_fixture.evidence .proof_profile .public_opening_material_version .saturating_add(1) .to_string() };
    let_row! { stale_proof_profile_public_opening_material_field_count = signed.artifact_fixture.evidence .proof_profile .public_opening_material_field_count .saturating_add(1) .to_string() };
    let_row! { proof_profile_field_count_fragment = format!("; proof-profile-field-count={proof_profile_field_count}") };
    let_row! { stale_proof_profile_field_count_fragment = format!("; proof-profile-field-count={stale_proof_profile_field_count}") };
    let_row! { proof_profile_public_opening_material_version_fragment = format!( "; proof-profile-public-opening-material-version={proof_profile_public_opening_material_version}" ) };
    let_row! { stale_proof_profile_public_opening_material_version_fragment = format!( "; proof-profile-public-opening-material-version={stale_proof_profile_public_opening_material_version}" ) };
    let_row! { proof_profile_public_opening_material_field_count_fragment = format!( "; proof-profile-public-opening-material-field-count={proof_profile_public_opening_material_field_count}" ) };
    let_row! { stale_proof_profile_public_opening_material_field_count_fragment = format!( "; proof-profile-public-opening-material-field-count={stale_proof_profile_public_opening_material_field_count}" ) };
    let stale_proof_profile_trace_n_log2 = signed
        .artifact_fixture
        .evidence
        .proof_profile
        .n_log2
        .saturating_add(1);
    let_row! { stale_proof_profile_trace_n_log2_fragment = format!("; proof-profile-n-log2={stale_proof_profile_trace_n_log2}") };
    let stale_proof_profile_queries = signed
        .artifact_fixture
        .evidence
        .proof_profile
        .queries
        .saturating_add(1);
    let proof_profile_queries_fragment = format!("; proof-profile-queries={proof_profile_queries}");
    let_row! { stale_proof_profile_queries_fragment = format!("; proof-profile-queries={stale_proof_profile_queries}") };
    let proof_profile_n_log2_fragment = format!("; proof-profile-n-log2={proof_profile_n_log2}");
    let_row! { proof_profile_blowup_log2_fragment = format!("; proof-profile-blowup-log2={proof_profile_blowup_log2}") };
    let stale_proof_profile_blowup_log2 = signed
        .artifact_fixture
        .evidence
        .proof_profile
        .blowup_log2
        .saturating_add(1);
    let_row! { stale_proof_profile_blowup_log2_fragment = format!("; proof-profile-blowup-log2={stale_proof_profile_blowup_log2}") };
    let_row! { proof_profile_fold_arity_fragment = format!("; proof-profile-fold-arity={proof_profile_fold_arity}") };
    let stale_proof_profile_fold_arity = signed
        .artifact_fixture
        .evidence
        .proof_profile
        .fold_arity
        .saturating_add(1);
    let_row! { stale_proof_profile_fold_arity_fragment = format!("; proof-profile-fold-arity={stale_proof_profile_fold_arity}") };
    let_row! { proof_profile_merkle_arity_fragment = format!("; proof-profile-merkle-arity={proof_profile_merkle_arity}") };
    let stale_proof_profile_merkle_arity = signed
        .artifact_fixture
        .evidence
        .proof_profile
        .merkle_arity
        .saturating_add(1);
    let_row! { stale_proof_profile_merkle_arity_fragment = format!("; proof-profile-merkle-arity={stale_proof_profile_merkle_arity}") };
    let_row! { proof_profile_requires_base_fragment = format!( "; proof-profile-requires-canonical-base-transcript-label={proof_profile_requires_canonical_base_transcript_label}" ) };
    let_row! { stale_proof_profile_requires_base_fragment = "; proof-profile-requires-canonical-base-transcript-label=false" };
    let_row! { proof_profile_rejects_suffixed_fragment = format!( "; proof-profile-rejects-suffixed-transcript-label-aliases={proof_profile_rejects_suffixed_transcript_label_aliases}" ) };
    let_row! { stale_proof_profile_rejects_suffixed_fragment = "; proof-profile-rejects-suffixed-transcript-label-aliases=false" };
    let_row! { proof_profile_requires_verifier_trace_digest_fragment = format!( "; proof-profile-requires-verifier-owned-trace-material-digest={proof_profile_requires_verifier_owned_trace_material_digest}" ) };
    let_row! { stale_proof_profile_requires_verifier_trace_digest_fragment = "; proof-profile-requires-verifier-owned-trace-material-digest=false" };
    let_row! { proof_profile_validates_public_opening_fragment = format!( "; proof-profile-validates-transcript-public-opening-material={proof_profile_validates_transcript_public_opening_material}" ) };
    let_row! { stale_proof_profile_validates_public_opening_fragment = "; proof-profile-validates-transcript-public-opening-material=false" };
    let_row! { proof_profile_validates_merkle_path_roots_fragment = format!( "; proof-profile-validates-merkle-path-roots={proof_profile_validates_merkle_path_roots}" ) };
    let_row! { stale_proof_profile_validates_merkle_path_roots_fragment = "; proof-profile-validates-merkle-path-roots=false" };
    let_row! { proof_profile_validates_fri_query_chain_fragment = format!( "; proof-profile-validates-fri-query-chain={proof_profile_validates_fri_query_chain}" ) };
    let_row! { stale_proof_profile_validates_fri_query_chain_fragment = "; proof-profile-validates-fri-query-chain=false" };
    let_row! { proof_profile_prover_input_domain = std::str::from_utf8(&signed.artifact_fixture.evidence.proof_profile.prover_input_material_digest_domain) .expect("proof-profile prover input digest domain is UTF-8") };
    let_row! { proof_profile_prover_input_domain_fragment = format!( "; proof-profile-prover-input-material-digest-domain={proof_profile_prover_input_domain}" ) };
    let stale_proof_profile_prover_input_domain_fragment = "; proof-profile-prover-input-material-digest-domain=wrong-full-bootstrap-prover-input-domain";
    let_row! { proof_profile_air_constraint_domain = std::str::from_utf8( &signed.artifact_fixture.evidence .proof_profile .arithmetic_air_constraint_system_digest_domain, ) .expect("proof-profile AIR constraint-system digest domain is UTF-8") };
    let_row! { proof_profile_air_constraint_domain_fragment = format!( "; proof-profile-arithmetic-air-constraint-system-digest-domain={proof_profile_air_constraint_domain}" ) };
    let_row! { proof_profile_public_opening_domain = std::str::from_utf8(&signed.artifact_fixture.evidence.proof_profile.public_opening_material_digest_domain) .expect("proof-profile public opening material digest domain is UTF-8") };
    let_row! { proof_profile_public_opening_domain_fragment = format!( "; proof-profile-public-opening-material-digest-domain={proof_profile_public_opening_domain}" ) };
    let stale_proof_profile_public_opening_domain_fragment = "; proof-profile-public-opening-material-digest-domain=wrong-full-bootstrap-public-opening-domain";
    let_row! { proof_profile_proof_key_pair_domain = std::str::from_utf8(&signed.artifact_fixture.evidence.proof_profile.proof_key_pair_commitment_domain) .expect("proof-profile proof-key pair commitment domain is UTF-8") };
    let_row! { proof_profile_proof_key_pair_domain_fragment = format!( "; proof-profile-proof-key-pair-commitment-domain={proof_profile_proof_key_pair_domain}" ) };
    let stale_proof_profile_proof_key_pair_domain_fragment = "; proof-profile-proof-key-pair-commitment-domain=wrong-full-bootstrap-proof-key-pair-domain";
    let_row! { proof_profile_separates_release_prover_fragment = format!( "; proof-profile-separates-release-prover-material-domains={}", signed.artifact_fixture.evidence .proof_profile .separates_release_prover_material_domains ) };
    let_row! { stale_proof_profile_separates_release_prover_fragment = "; proof-profile-separates-release-prover-material-domains=false" };
    let_row! { arithmetic_trace_profile_digest_fragment = format!("; arithmetic-trace-profile-digest={arithmetic_trace_profile_digest_hex}") };
    let_row! { arithmetic_air_constraint_system_digest_fragment = format!( "; arithmetic-air-constraint-system-digest={arithmetic_air_constraint_system_digest_hex}" ) };
    let_row! { signed_report_without_proof_profile_field_count_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics, &signed.review_fixture.audit_report_bytes, &proof_profile_field_count_fragment, ""), "build signed report missing proof-profile field count", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 301 => validate_release_package_v1(&signed_report_without_proof_profile_field_count_package) };
    let_row! { signed_report_with_stale_proof_profile_field_count_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_field_count_fragment, &stale_proof_profile_field_count_fragment, ), "build signed report with stale proof-profile field count", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 302 => validate_release_package_v1(&signed_report_with_stale_proof_profile_field_count_package) };
    let_row! { signed_report_without_public_opening_material_version_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_public_opening_material_version_fragment, "", ), "build signed report missing proof-profile public-opening material version", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 303 => validate_release_package_v1(&signed_report_without_public_opening_material_version_package) };
    let_row! { signed_report_with_stale_public_opening_material_field_count_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_public_opening_material_field_count_fragment, &stale_proof_profile_public_opening_material_field_count_fragment, ), "build signed report with stale proof-profile public-opening material field count", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 304 => validate_release_package_v1( &signed_report_with_stale_public_opening_material_field_count_package, ) };
    let_row! { signed_report_with_stale_trace_geometry_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_n_log2_fragment, &stale_proof_profile_trace_n_log2_fragment, ), "build signed report with stale proof-profile native trace geometry", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 305 => validate_release_package_v1(&signed_report_with_stale_trace_geometry_package) };
    let_row! { signed_report_with_stale_queries_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_queries_fragment, &stale_proof_profile_queries_fragment, ), "build signed report with stale proof-profile FRI query count", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 306 => validate_release_package_v1(&signed_report_with_stale_queries_package) };
    let_row! { signed_report_with_downgraded_base_label_obligation_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_requires_base_fragment, stale_proof_profile_requires_base_fragment, ), "build signed report with downgraded canonical base transcript-label obligation", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 307 => validate_release_package_v1(&signed_report_with_downgraded_base_label_obligation_package) };
    let_row! { signed_report_with_downgraded_suffixed_label_obligation_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_rejects_suffixed_fragment, stale_proof_profile_rejects_suffixed_fragment, ), "build signed report with downgraded suffixed transcript-label obligation", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 308 => validate_release_package_v1( &signed_report_with_downgraded_suffixed_label_obligation_package, ) };
    let_row! { signed_report_with_downgraded_verifier_trace_digest_obligation_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_requires_verifier_trace_digest_fragment, stale_proof_profile_requires_verifier_trace_digest_fragment, ), "build signed report with downgraded verifier-owned trace-material digest obligation", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 309 => validate_release_package_v1( &signed_report_with_downgraded_verifier_trace_digest_obligation_package, ) };
    let_row! { signed_report_with_downgraded_public_opening_obligation_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_validates_public_opening_fragment, stale_proof_profile_validates_public_opening_fragment, ), "build signed report with downgraded public-opening material obligation", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 310 => validate_release_package_v1( &signed_report_with_downgraded_public_opening_obligation_package, ) };
    let_row! { signed_report_with_downgraded_fri_query_chain_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_validates_fri_query_chain_fragment, stale_proof_profile_validates_fri_query_chain_fragment, ), "build signed report with downgraded FRI query-chain validation obligation", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 311 => validate_release_package_v1(&signed_report_with_downgraded_fri_query_chain_package) };
    let_row! { signed_report_without_prover_input_domain_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_prover_input_domain_fragment, "", ), "build signed report missing proof-profile prover input digest domain", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 312 => validate_release_package_v1(&signed_report_without_prover_input_domain_package) };
    let_row! { signed_report_with_stale_prover_input_domain_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_prover_input_domain_fragment, stale_proof_profile_prover_input_domain_fragment, ), "build signed report with stale proof-profile prover input digest domain", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 313 => validate_release_package_v1(&signed_report_with_stale_prover_input_domain_package) };
    let_row! { signed_report_with_stale_public_opening_domain_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_public_opening_domain_fragment, stale_proof_profile_public_opening_domain_fragment, ), "build signed report with stale proof-profile public opening material digest domain", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 314 => validate_release_package_v1(&signed_report_with_stale_public_opening_domain_package) };
    let_row! { signed_report_with_stale_proof_key_pair_domain_package = signed_package_with_report_bytes( replace_ascii_once(&signed.artifact_fixture.diagnostics,  &signed.review_fixture.audit_report_bytes, &proof_profile_proof_key_pair_domain_fragment, stale_proof_profile_proof_key_pair_domain_fragment, ), "build signed report with stale proof-profile proof-key pair commitment domain", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 315 => validate_release_package_v1(&signed_report_with_stale_proof_key_pair_domain_package) };
    let uppercase_release_evidence_digest_hex = signed
        .review_fixture
        .release_evidence_digest_hex
        .to_ascii_uppercase();
    let_row! { signed_report_with_uppercase_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), uppercase_release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with uppercase release evidence digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 316 => validate_release_package_v1(&signed_report_with_uppercase_release_evidence_digest_package) };
    let_row! { signed_report_with_uppercase_release_evidence_label_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; RELEASE-EVIDENCE-DIGEST=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with uppercase release evidence digest label", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 317 => validate_release_package_v1(&signed_report_with_uppercase_release_evidence_label_package) };
    let release_evidence_digest_bytes: [u8; Hash::LENGTH] = signed.evidence_codec.digest.into();
    let_row! { signed_report_with_raw_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), release_evidence_digest_bytes.as_slice(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with raw release evidence digest bytes", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 318 => validate_release_package_v1(&signed_report_with_raw_release_evidence_digest_package) };
    let_row! { signed_report_with_colon_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest: ".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with colon-separated release evidence digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 319 => validate_release_package_v1(&signed_report_with_colon_release_evidence_digest_package) };
    let_row! { signed_report_without_generated_body_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report missing generated circuit body digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 320 => validate_release_package_v1(&signed_report_without_generated_body_digest_package) };
    let_row! { stale_report_proof_key_pair_commitment_hex = hex::encode(<[u8; Hash::LENGTH]>::from( Hash::new(b"stale-release-audit-report-proof-key-pair-commitment"), )) };
    let_row! { signed_report_with_stale_proof_key_pair_commitment_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), stale_report_proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with stale proof-key pair commitment", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 321 => validate_release_package_v1(&signed_report_with_stale_proof_key_pair_commitment_package) };
    let_row! { signed_report_with_relabelled_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; reviewer signoff checked; copied-release-evidence-hash=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with relabelled release evidence digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 322 => validate_release_package_v1(&signed_report_with_relabelled_release_evidence_digest_package) };
    let_row! { signed_report_with_prefixed_release_evidence_label_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; copied-release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with prefixed release evidence label", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 323 => validate_release_package_v1(&signed_report_with_prefixed_release_evidence_label_package) };
    let_row! { signed_report_with_spaced_prefixed_release_evidence_label_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; copied release evidence digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with spaced prefixed release evidence label", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 324 => validate_release_package_v1( &signed_report_with_spaced_prefixed_release_evidence_label_package, ) };
    let_row! { signed_report_with_cross_field_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=; copied-release-evidence-hash=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with cross-field release evidence digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 325 => validate_release_package_v1( &signed_report_with_cross_field_release_evidence_digest_package, ) };
    let_row! { signed_report_with_crlf_cross_field_release_evidence_digest_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1\r\n release evidence digest=\r\n copied release evidence hash=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with CRLF cross-field release evidence digest", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 326 => validate_release_package_v1( &signed_report_with_crlf_cross_field_release_evidence_digest_package, ) };
    let_row! { signed_report_with_prefixed_release_evidence_value_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=0".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with prefixed release evidence value", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 327 => validate_release_package_v1(&signed_report_with_prefixed_release_evidence_value_package) };
    let_row! { signed_report_with_suffixed_release_evidence_value_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"0".as_slice(), ] .concat(), "build signed report with suffixed release evidence value", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 328 => validate_release_package_v1(&signed_report_with_suffixed_release_evidence_value_package) };
    let_row! { signed_report_with_punctuation_suffixed_release_evidence_value_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b".copied".as_slice(), ] .concat(), "build signed report with punctuation-suffixed release evidence value", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 329 => validate_release_package_v1( &signed_report_with_punctuation_suffixed_release_evidence_value_package, ) };
    let_row! { conflicting_release_evidence_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( Hash::new(b"stale-release-audit-report-release-evidence-digest"), )) };
    let_row! { signed_report_with_conflicting_duplicate_release_evidence_label_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; release evidence digest=".as_slice(), conflicting_release_evidence_digest_hex.as_bytes(), ] .concat(), "build signed report with conflicting duplicate release evidence label", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 330 => validate_release_package_v1( &signed_report_with_conflicting_duplicate_release_evidence_label_package, ) };
    let_row! { signed_report_with_same_value_duplicate_release_evidence_label_package = signed_package_with_report_bytes( [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; release evidence digest=".as_slice(), signed.review_fixture.release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), signed.review_fixture.generated_circuit_body_digest_hex.as_bytes(), b"; proof key pair commitment=".as_slice(), signed.review_fixture.proof_key_pair_commitment_hex.as_bytes(), ] .concat(), "build signed report with same-value duplicate release evidence label", ) };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 331 => validate_release_package_v1( &signed_report_with_same_value_duplicate_release_evidence_label_package, ) };
    ReportCommitments {
        proof_profile_field_count_fragment,
        stale_proof_profile_field_count_fragment,
        proof_profile_public_opening_material_version_fragment,
        stale_proof_profile_public_opening_material_version_fragment,
        proof_profile_public_opening_material_field_count_fragment,
        proof_profile_n_log2_fragment,
        proof_profile_blowup_log2_fragment,
        stale_proof_profile_blowup_log2_fragment,
        proof_profile_fold_arity_fragment,
        stale_proof_profile_fold_arity_fragment,
        proof_profile_merkle_arity_fragment,
        stale_proof_profile_merkle_arity_fragment,
        proof_profile_requires_base_fragment,
        stale_proof_profile_requires_base_fragment,
        proof_profile_rejects_suffixed_fragment,
        stale_proof_profile_rejects_suffixed_fragment,
        proof_profile_requires_verifier_trace_digest_fragment,
        stale_proof_profile_requires_verifier_trace_digest_fragment,
        proof_profile_validates_public_opening_fragment,
        stale_proof_profile_validates_public_opening_fragment,
        proof_profile_validates_merkle_path_roots_fragment,
        stale_proof_profile_validates_merkle_path_roots_fragment,
        proof_profile_air_constraint_domain_fragment,
        proof_profile_public_opening_domain_fragment,
        stale_proof_profile_public_opening_domain_fragment,
        proof_profile_separates_release_prover_fragment,
        stale_proof_profile_separates_release_prover_fragment,
        arithmetic_trace_profile_digest_fragment,
        arithmetic_air_constraint_system_digest_fragment,
    }
}
