//! Release-audit evidence binding assertions and fixture ownership.

use super::*;

pub(super) fn check(
    signed: &SignedAuditInputs<'_>,
    generated_inventory: &GeneratedPackages,
    package_codec: &PackageAuthorities,
) {
    mutation_row! { tampered_report_signoff = signed.signoff_authority.signoff.clone(); tampered_report_signoff.payload.audit_report_digest = Hash::new(b"tampered-bfv-release-audit-report-digest"); assert_local_diag! { signed.artifact_fixture.diagnostics; 733 => validate_release_signoff_v1(&tampered_report_signoff) }; };
    let_row! { wrong_signer_signature = SignatureOf::try_new(package_codec.alternate_reviewer_key_pair.private_key(), &signed.signoff_authority.signoff.payload) .expect("checked wrong-reviewer release audit fixture signature") };
    let_row! { wrong_signer_signoff = BfvFullBootstrapReleaseAuditSignoffV1 { signature: wrong_signer_signature, ..signed.signoff_authority.signoff.clone() } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 734 => validate_release_signoff_v1(&wrong_signer_signoff) };
    let_row! { (alternate_secret_key, _public_key, _relinearization_key) = keygen_from_seed(&signed.artifact_fixture.params, b"bfv-full-bootstrap-release-audit-drift-keygen") .expect("alternate sample-extraction keygen") };
    mutation_row! { drifted_artifacts = signed.artifact_fixture.artifacts.clone(); drifted_artifacts.sample_extraction_key = sample_full_bootstrap_sample_extraction_switch_key_artifact_payload( &signed.artifact_fixture.params, &alternate_secret_key, ); let_row! { drifted_evaluator_artifact_set_digest = full_evaluator_artifact_set_digest_v1( &signed.artifact_fixture.params, 1, &drifted_artifacts.coefficient_to_slot_key, &drifted_artifacts.slot_to_coefficient_key, &drifted_artifacts.blind_rotation_key, &drifted_artifacts.sample_extraction_key, &drifted_artifacts.accumulator, &drifted_artifacts.proof_public_input_schema, &drifted_artifacts.arithmetic_air_constraint_system, ) .expect("derive drifted evaluator artifact set digest") }; let_row! { (drifted_prover_key, drifted_verifier_key) = sample_full_bootstrap_proof_key_artifact_payloads( &signed.artifact_fixture.params, Hash::new(&drifted_artifacts.proof_public_input_schema), drifted_evaluator_artifact_set_digest, ) }; drifted_artifacts.prover_key = drifted_prover_key; drifted_artifacts.verifier_key = drifted_verifier_key; let_row! { drifted_material = sample_full_bootstrap_circuit_material_for_artifacts(&signed.artifact_fixture.params, &drifted_artifacts) }; let_row! { drifted_evidence = release_evidence_v1(&signed.artifact_fixture.params, &drifted_material, &drifted_artifacts) .expect("derive drifted full-bootstrap release audit evidence") }; let_row! { drifted_signoff = sign_bfv_full_bootstrap_release_audit_signoff_v1( &drifted_evidence, Hash::new(b"drifted-bfv-full-bootstrap-release-audit-report-v1"), Hash::new(b"drifted-bfv-full-bootstrap-release-audit-archive-v1"), "sora-zk-audit-wg-2026", signed.review_fixture.reviewer_key_pair.private_key(), ) .expect("sign drifted release audit evidence") }; assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 735; (validate_bfv_full_bootstrap_release_audit_signoff_for_evidence_v1( &drifted_signoff, &signed.artifact_fixture.evidence, )), (validate_bfv_full_bootstrap_release_audit_signoff_for_artifacts_v1( &signed.artifact_fixture.params, &drifted_material, &drifted_artifacts, &signed.signoff_authority.signoff, )) }; let_row! { drifted_record = BfvFullBootstrapReleaseAuditRecordV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_FIELD_COUNT_V1, evidence: drifted_evidence.clone(), signoff: drifted_signoff.clone(), } }; validate_bfv_full_bootstrap_release_audit_record_v1(&drifted_record) .expect("drifted release audit record is internally consistent"); assert_local_diag! { signed.artifact_fixture.diagnostics; 737 => validate_bfv_full_bootstrap_release_audit_record_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &signed.artifact_fixture.artifacts, &drifted_record, ) }; };
    let_row! { mismatched_record = BfvFullBootstrapReleaseAuditRecordV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_FIELD_COUNT_V1, evidence: signed.artifact_fixture.evidence.clone(), signoff: drifted_signoff.clone(), } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 738 => validate_bfv_full_bootstrap_release_audit_record_v1(&mismatched_record) };
    let_row! { mismatched_package = BfvFullBootstrapReleaseAuditPackageV1 { version: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1, field_count: BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1, record: mismatched_record, record_digest: generated_inventory.package.record_digest, manifest: generated_inventory.package.manifest.clone(), manifest_digest: generated_inventory.package.manifest_digest, audit_report_bytes: signed.review_fixture.audit_report_bytes.clone(), audit_evidence_archive_bytes: signed.review_fixture.audit_evidence_archive_bytes.clone(), } };
    assert_local_diag! { signed.artifact_fixture.diagnostics; 739 => validate_release_package_v1(&mismatched_package) };
    assert_ne_row! { signed.artifact_fixture.evidence.sample_extraction_key_digest, drifted_evidence.sample_extraction_key_digest, "release audit evidence must bind sample-extraction artifact drift" };
    assert_ne_row! { signed.evidence_codec.digest, release_evidence_digest_v1(&drifted_evidence) .expect("drifted release audit evidence digest"), "release audit evidence digest must change when valid generated artifacts change" };
    let mut stale_material = signed.artifact_fixture.material.clone();
    stale_material.prover_key_digest = Hash::new(b"stale-release-audit-prover-key-digest");
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 740; (release_evidence_v1(&signed.artifact_fixture.params, &stale_material, &signed.artifact_fixture.artifacts)), (validate_bfv_full_bootstrap_release_audit_signoff_for_artifacts_v1( &signed.artifact_fixture.params, &stale_material, &signed.artifact_fixture.artifacts, &signed.signoff_authority.signoff, )), (validate_bfv_full_bootstrap_release_audit_record_for_artifacts_v1( &signed.artifact_fixture.params, &stale_material, &signed.artifact_fixture.artifacts, &signed.signoff_codec.record, )), (validate_release_package_for_artifacts_v1( &signed.artifact_fixture.params, &stale_material, &signed.artifact_fixture.artifacts, &generated_inventory.package )) };
    let_row! { governed_material_profile_drift_setters: [CircuitMaterialDigestSetter; 3] = [ ("RNS modulus-chain digest", |material, digest| { material.rns_modulus_chain_digest = digest; }), ( "key-switch decomposition-chain digest", |material, digest| { material.key_switch_decomposition_chain_digest = digest; }, ), ( "centered scale-round source-chain digest", |material, digest| { material.centered_scale_round_source_chain_digest = digest; }, ), ] };
    for (label, set_digest) in governed_material_profile_drift_setters {
        let mut stale_profile_material = signed.artifact_fixture.material.clone();
        set_digest(
            &mut stale_profile_material,
            Hash::new(format!("profile-drift-release-audit-material-{label}-v1").as_bytes()),
        );
        assert_call! { assert_error_contains; release_evidence_v1(&signed.artifact_fixture.params, &stale_profile_material, &signed.artifact_fixture.artifacts), signed.artifact_fixture.diagnostics.static_expected_at(744), signed.artifact_fixture.diagnostics.dynamic_context_at( 744, &format!("release audit evidence must reject governed material {label} drift"), ), };
        assert_call! { assert_error_contains; validate_bfv_full_bootstrap_release_audit_signoff_for_artifacts_v1( &signed.artifact_fixture.params, &stale_profile_material, &signed.artifact_fixture.artifacts, &signed.signoff_authority.signoff, ), signed.artifact_fixture.diagnostics.static_expected_at(745), signed.artifact_fixture.diagnostics.dynamic_context_at(745, &format!( "release audit signoff artifact validation must reject governed material {label} drift" )) };
        assert_call! { assert_error_contains; validate_bfv_full_bootstrap_release_audit_record_for_artifacts_v1( &signed.artifact_fixture.params, &stale_profile_material, &signed.artifact_fixture.artifacts, &signed.signoff_codec.record, ), signed.artifact_fixture.diagnostics.static_expected_at(746), signed.artifact_fixture.diagnostics.dynamic_context_at(746, &format!( "release audit record artifact validation must reject governed material {label} drift" )) };
        assert_call! { assert_error_contains; validate_release_package_for_artifacts_v1( &signed.artifact_fixture.params, &stale_profile_material, &signed.artifact_fixture.artifacts, &generated_inventory.package, ), signed.artifact_fixture.diagnostics.static_expected_at(747), signed.artifact_fixture.diagnostics.dynamic_context_at(747, &format!( "release audit package artifact validation must reject governed material {label} drift" )) };
    }
    let mut stale_artifacts = signed.artifact_fixture.artifacts.clone();
    stale_artifacts.accumulator = b"stale-release-audit-accumulator-artifact".to_vec();
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 748; (release_evidence_v1(&signed.artifact_fixture.params, &signed.artifact_fixture.material, &stale_artifacts)), (validate_bfv_full_bootstrap_release_audit_signoff_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &stale_artifacts, &signed.signoff_authority.signoff, )), (validate_bfv_full_bootstrap_release_audit_record_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &stale_artifacts, &signed.signoff_codec.record, )), (validate_release_package_for_artifacts_v1( &signed.artifact_fixture.params, &signed.artifact_fixture.material, &stale_artifacts, &generated_inventory.package )) };
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = signed.artifact_fixture.evidence => release_evidence_digest_v1(@);
        752 => case.proof_profile.queries = case.proof_profile.queries.saturating_add(1);
    }
    macro_rules! expect_proof_profile_field_rejected {
        ($($field:ident: $value:expr => $index:literal;)+) => {
            $({
                let mut rejected = signed.artifact_fixture.evidence.clone();
                rejected.proof_profile.$field = $value;
                assert_local_diag! {
                    signed.artifact_fixture.diagnostics;
                    $index => validate_release_evidence_v1(&rejected)
                };
            })+
        };
    }
    expect_proof_profile_field_rejected! {
        proof_system: "stark/fri/sha256".to_owned() => 753;
        blowup_log2: signed.artifact_fixture.evidence.proof_profile.n_log2 + 1 => 754;
        public_input_hash_bytes: 0 => 755;
        air_evaluation_material_field_count:
            signed.artifact_fixture.evidence.proof_profile.air_evaluation_material_field_count.saturating_add(1) => 756;
        prover_input_material_digest_domain:
            b"wrong-full-bootstrap-release-audit-prover-input-domain".to_vec() => 757;
        air_evaluation_material_digest_domain:
            b"replace_before_production".to_vec() => 758;
        separates_release_prover_material_domains: false => 759;
        proof_key_pair_commitment_domain:
            b"wrong-full-bootstrap-proof-key-pair-commitment-domain".to_vec() => 760;
        separates_proof_key_material_and_pair_commitments: false => 761;
        supports_bounded_noise: false => 762;
    }
    macro_rules! expect_policy_profile_downgrades_rejected {
        ($index:literal; $($field:ident => $label:literal),+ $(,)?) => {
            $({
                let mut downgraded = signed.artifact_fixture.evidence.clone();
                downgraded.proof_profile.$field = false;
                let context = format!(
                    "release audit evidence validation must reject proof profile {} downgrades",
                    $label,
                );
                assert_error_contains(
                    validate_release_evidence_v1(&downgraded),
                    signed.artifact_fixture.diagnostics.static_expected_at($index),
                    signed.artifact_fixture.diagnostics.dynamic_context_at($index, &context),
                );
            })+
        };
    }
    expect_policy_profile_downgrades_rejected! {
        763;
        validates_artifact_bound_prover_input => "artifact-bound prover input validation",
        rejects_stale_galois_key_set_replay => "stale Galois-key set replay rejection",
        rejects_stale_proof_key_artifacts => "stale proof-key artifact replay rejection",
    }
    expect_policy_profile_downgrades_rejected! {
        764;
        derives_opening_schedule_from_statement_hash => "statement-hash-derived opening schedule",
        derives_opening_schedule_from_trace_material_digest =>
            "trace-material-derived opening schedule",
        bounds_opening_schedule_rejection_sampling =>
            "bounded opening-schedule rejection sampling",
        validates_transcript_public_padding_openings =>
            "transcript public-padding opening replay",
        requires_verifier_owned_trace_material_digest => "verifier-owned trace material digest",
        requires_canonical_base_transcript_label =>
            "canonical base transcript-label enforcement",
        rejects_suffixed_transcript_label_aliases =>
            "suffixed transcript-label alias rejection",
    }
    expect_policy_profile_downgrades_rejected! {
        765;
        validates_merkle_path_shape => "Merkle path shape validation",
        validates_merkle_path_roots => "Merkle path root validation",
        validates_fri_query_chain => "FRI query-chain validation",
        binds_first_fri_values_to_opened_air_values => "first-FRI/opened-AIR replay binding",
        binds_fri_queries_to_air_commitment_roots => "AIR-root FRI query binding",
    }
    expect_policy_profile_downgrades_rejected! {
        766;
        validates_air_evaluation_material_digest => "AIR evaluation material digest validation",
        validates_air_evaluation_trace_material_digest =>
            "trace-bound AIR evaluation material validation",
        requires_zero_air_composition_values => "zero AIR composition validation",
    }
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = signed.artifact_fixture.evidence => validate_release_evidence_v1(@);
        767 => case.artifact_bundle_digest = Hash::prehashed([0_u8; Hash::LENGTH]);
        768 => case.max_bootstrap_depth = 0;
        769 => case.prover_key.field_count += 1;
        770 => case.prover_key.centered_scale_round_source_chain_digest =
            Hash::new(b"stale-release-audit-prover-key-source-chain");
    }
    mutation_row! { placeholder_key_digest_evidence = signed.artifact_fixture.evidence.clone(); placeholder_key_digest_evidence.prover_key.key_digest = Hash::new(b"placeholder full-bootstrap prover-key commitment"); assert_local_diag! { signed.artifact_fixture.diagnostics; 771 => validate_release_key_evidence_shape_v1( "BFV full-bootstrap release audit prover key", BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, &placeholder_key_digest_evidence.prover_key, ) }; };
    mutation_row! { placeholder_key_material_commitment_evidence = signed.artifact_fixture.evidence.clone(); placeholder_key_material_commitment_evidence .verifier_key .key_material_commitment = Hash::new(b"placeholder full-bootstrap verifier-key commitment"); assert_local_diag! { signed.artifact_fixture.diagnostics; 772 => validate_release_key_evidence_shape_v1( "BFV full-bootstrap release audit verifier key", BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, &placeholder_key_material_commitment_evidence.verifier_key, ) }; };
    assert_local_diag_clone_mutations! {
        signed.artifact_fixture.diagnostics;
        case = signed.artifact_fixture.evidence => validate_release_evidence_v1(@);
        773 => case.prover_key.native_circuit_fingerprint =
            Hash::new(b"placeholder BFV full-bootstrap native proof key payload");
        774 => case.prover_key.native_payload_kind =
            BFV_FULL_BOOTSTRAP_NATIVE_VERIFIER_PAYLOAD_KIND_V1.to_owned();
        775 => case.verifier_key.native_circuit_fingerprint =
            Hash::new(b"stale-release-audit-native-fingerprint");
    }
    mutation_row! { stale_shared_fingerprint_evidence = signed.artifact_fixture.evidence.clone(); let_row! { stale_native_circuit_fingerprint = Hash::new(b"stale-shared-release-audit-native-fingerprint") }; stale_shared_fingerprint_evidence .prover_key .native_circuit_fingerprint = stale_native_circuit_fingerprint; stale_shared_fingerprint_evidence .verifier_key .native_circuit_fingerprint = stale_native_circuit_fingerprint; assert_local_diag! { signed.artifact_fixture.diagnostics; 776 => validate_release_evidence_v1(&stale_shared_fingerprint_evidence) }; };
    let mut duplicate_payload_digest_evidence = signed.artifact_fixture.evidence.clone();
    duplicate_payload_digest_evidence
        .verifier_key
        .native_payload_digest = duplicate_payload_digest_evidence
        .prover_key
        .native_payload_digest;
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 777; (validate_release_evidence_v1(&duplicate_payload_digest_evidence)), (release_evidence_digest_v1(&duplicate_payload_digest_evidence)) };
    let mut aliased_payload_digest_evidence = signed.artifact_fixture.evidence.clone();
    aliased_payload_digest_evidence
        .prover_key
        .native_payload_digest = native_payload_digest_for_test(
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        aliased_payload_digest_evidence
            .proof_key_pair_commitment
            .as_ref(),
    );
    assert_error_matrix_row! { signed.artifact_fixture.diagnostics; 779; (validate_release_evidence_v1(&aliased_payload_digest_evidence)), (release_evidence_digest_v1(&aliased_payload_digest_evidence)) };
    macro_rules! expect_payload_digest_rejected {
        ($($case:ident.$role:ident = $digest:expr => $index:literal;)+) => {$(
            let mut $case = signed.artifact_fixture.evidence.clone();
            $case.$role.native_payload_digest = $digest;
            assert_local_diag! {
                signed.artifact_fixture.diagnostics; $index => validate_release_evidence_v1(&$case)
            };
        )+};
    }
    expect_payload_digest_rejected! {
        empty_payload_digest_evidence.prover_key = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, []) => 781;
        zero_payload_digest_evidence.verifier_key = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, [0_u8; 64]) => 782;
    }
    for len in [2_usize, 4, 8, 16, 128] {
        let mut short_zero_payload_digest_evidence = signed.artifact_fixture.evidence.clone();
        short_zero_payload_digest_evidence
            .prover_key
            .native_payload_digest = native_payload_digest_for_test(
            BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            vec![0_u8; len],
        );
        assert_call! { assert_error_contains; validate_release_evidence_v1( &short_zero_payload_digest_evidence, ), signed.artifact_fixture.diagnostics.static_expected_at(783), signed.artifact_fixture.diagnostics.dynamic_context_at(783, &format!( "release audit evidence validation must reject {len}-byte all-zero native-payload digest sentinels" )) };
    }
    expect_payload_digest_rejected! {
        placeholder_payload_digest_evidence.prover_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, b"PLACEHOLDER BFV FULL-BOOTSTRAP NATIVE PROVER PAYLOAD") => 784;
        proof_key_placeholder_payload_digest_evidence.verifier_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, b"PLACEHOLDER BFV FULL-BOOTSTRAP NATIVE PROOF KEY PAYLOAD") => 785;
    }
    for sentinel in [
        b"sample".as_slice(),
        b"template".as_slice(),
        b"example".as_slice(),
        b"mock".as_slice(),
        b"fixture".as_slice(),
    ] {
        mutation_row! { sentinel_payload_digest_evidence = signed.artifact_fixture.evidence.clone(); sentinel_payload_digest_evidence .prover_key .native_payload_digest = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, sentinel); assert_local_diag! { signed.artifact_fixture.diagnostics; 786 => validate_release_evidence_v1(&sentinel_payload_digest_evidence) }; };
        let_row! { mut whitespace_prefixed_sentinel_payload = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] };
        whitespace_prefixed_sentinel_payload.extend_from_slice(sentinel);
        mutation_row! { whitespace_prefixed_sentinel_payload_digest_evidence = signed.artifact_fixture.evidence.clone(); whitespace_prefixed_sentinel_payload_digest_evidence .verifier_key .native_payload_digest = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, &whitespace_prefixed_sentinel_payload); assert_local_diag! { signed.artifact_fixture.diagnostics; 787 => validate_release_evidence_v1(&whitespace_prefixed_sentinel_payload_digest_evidence) }; };
    }
    expect_payload_digest_rejected! {
        draft_payload_digest_evidence.prover_key = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, b"draft") => 788;
        not_for_production_payload_digest_evidence.verifier_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, b"not for production") => 789;
        spaced_replace_payload_digest_evidence.prover_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, b"replace before production") => 790;
        underscore_replace_payload_digest_evidence.verifier_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, b"replace_before_production") => 791;
        not_ready_payload_digest_evidence.prover_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, b"not-production-ready") => 792;
        whitespace_prefixed_placeholder_payload_digest_evidence.verifier_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, b" \n\tPENDING BFV FULL-BOOTSTRAP NATIVE PROOF KEY PAYLOAD") => 793;
    }
    let_row! { mut padded_placeholder_native_payload = b"PLACEHOLDER BFV FULL-BOOTSTRAP NATIVE PROVER PAYLOAD ".to_vec() };
    padded_placeholder_native_payload.resize(
        BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1,
        b'x',
    );
    expect_payload_digest_rejected! {
        padded_placeholder_payload_digest_evidence.prover_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, &padded_placeholder_native_payload) => 794;
    }
    let_row! { mut delayed_placeholder_native_payload = vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1] };
    delayed_placeholder_native_payload
        .extend_from_slice(b"\x54\x4f\x44\x4f pending BFV full-bootstrap native proof key payload");
    mutation_row! { delayed_placeholder_payload_digest_evidence = signed.artifact_fixture.evidence.clone(); delayed_placeholder_payload_digest_evidence .verifier_key .native_payload_digest = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, &delayed_placeholder_native_payload); assert_local_diag! { signed.artifact_fixture.diagnostics; 795 => release_evidence_digest_v1(&delayed_placeholder_payload_digest_evidence) }; };
    let_row! { leading_whitespace_delayed_placeholder_native_payload_digest = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, [b" \n\t".as_slice(), vec![b' '; BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PLACEHOLDER_BODY_TEST_PADDING_BYTES + 1].as_slice(), b"TODO pending BFV full-bootstrap native proof key payload"].concat()) };
    expect_payload_digest_rejected! {
        leading_whitespace_delayed_placeholder_payload_digest_evidence.prover_key =
            leading_whitespace_delayed_placeholder_native_payload_digest => 796;
    }
    let mut binary_decorated_placeholder_native_payload = Vec::from([0xff]);
    binary_decorated_placeholder_native_payload
        .extend_from_slice(b"PLACEHOLDER BFV FULL-BOOTSTRAP NATIVE VERIFIER PAYLOAD");
    expect_payload_digest_rejected! {
        binary_decorated_payload_digest_evidence.verifier_key =
            native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, &binary_decorated_placeholder_native_payload) => 797;
    }
    mutation_row! { role_swapped_evidence = signed.artifact_fixture.evidence.clone(); role_swapped_evidence.verifier_key.key_role = BfvFullBootstrapCircuitArtifactRoleV1::ProverKey; assert_local_diag! { signed.artifact_fixture.diagnostics; 798 => release_evidence_digest_v1(&role_swapped_evidence) }; };
}
