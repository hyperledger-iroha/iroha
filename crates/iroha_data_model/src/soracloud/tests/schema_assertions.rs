//! Assert canonical Soracloud proof, execution and release-audit schema contracts.

use norito::json::Value;

pub(super) fn assert_soracloud_proof_key_commitment_domains(
    schema_value: &Value,
    pointer: &str,
    context: &str,
    material_domain: &str,
    pair_domain: &str,
) {
    let domains = schema_value
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-key commitment domains at `{pointer}`")
        });
    for (field, expected) in [("material", material_domain), ("pair", pair_domain)] {
        assert_eq!(
            domains.get(field).and_then(Value::as_str),
            Some(expected),
            "{context} proof-key commitment-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        domains
            .get("separates_material_and_pair")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise material/pair commitment-domain separation"
    );
    assert_ne!(
        domains.get("material").and_then(Value::as_str),
        domains.get("pair").and_then(Value::as_str),
        "{context} proof-key material and pair commitment domains must be distinct"
    );
}

pub(super) fn assert_soracloud_artifact_digest_domains(
    schema_value: &Value,
    pointer: &str,
    context: &str,
    circuit_material_domain: &str,
    evaluator_artifact_set_domain: &str,
    circuit_artifact_bundle_domain: &str,
) {
    let domains = schema_value
        .pointer(pointer)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} must carry artifact digest domains at `{pointer}`"));
    for (field, expected) in [
        ("circuit_material", circuit_material_domain),
        ("evaluator_artifact_set", evaluator_artifact_set_domain),
        ("circuit_artifact_bundle", circuit_artifact_bundle_domain),
    ] {
        assert_eq!(
            domains.get(field).and_then(Value::as_str),
            Some(expected),
            "{context} artifact digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        domains
            .get("separates_material_set_and_bundle")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise artifact digest-domain separation"
    );
    assert_ne!(
        domains.get("circuit_material").and_then(Value::as_str),
        domains
            .get("evaluator_artifact_set")
            .and_then(Value::as_str),
        "{context} circuit-material and evaluator-artifact-set domains must be distinct"
    );
    assert_ne!(
        domains
            .get("evaluator_artifact_set")
            .and_then(Value::as_str),
        domains
            .get("circuit_artifact_bundle")
            .and_then(Value::as_str),
        "{context} evaluator-artifact-set and circuit-artifact-bundle domains must be distinct"
    );
}

pub(super) fn assert_schema_object<'a>(
    schema_value: &'a Value,
    pointer: &str,
    context: &str,
) -> &'a Value {
    schema_value
        .pointer(pointer)
        .filter(|value| value.as_object().is_some())
        .unwrap_or_else(|| panic!("{context} must carry object at `{pointer}`"))
}

pub(super) fn assert_schema_string_field(
    section: &Value,
    field: &str,
    expected: &str,
    context: &str,
) {
    assert_eq!(
        section.get(field).and_then(Value::as_str),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}

pub(super) fn assert_schema_u64_field(section: &Value, field: &str, expected: u64, context: &str) {
    assert_eq!(
        section.get(field).and_then(Value::as_u64),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}

pub(super) fn assert_schema_bool_field(
    section: &Value,
    field: &str,
    expected: bool,
    context: &str,
) {
    assert_eq!(
        section.get(field).and_then(Value::as_bool),
        Some(expected),
        "{context} schema field `{field}` drifted"
    );
}

#[allow(clippy::too_many_lines)]
pub(super) fn assert_soracloud_execution_schema_sections(schema_value: &Value, context: &str) {
    let witness_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_DOMAIN,
    )
    .expect("execution witness digest domain is valid UTF-8");
    let proof_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution proof input material digest domain is valid UTF-8");
    let prover_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution prover input material digest domain is valid UTF-8");
    let air_evaluation_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR evaluation material digest domain is valid UTF-8");
    let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
    let trace_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic trace material digest domain is valid UTF-8");
    let air_constraint_system_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
    let composition_challenge_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_COMPOSITION_CHALLENGE_DOMAIN,
    )
    .expect("arithmetic AIR composition-challenge domain is valid UTF-8");
    let witness = assert_schema_object(
        schema_value,
        "/execution_witness_layout",
        "execution witness layout",
    );
    assert_schema_string_field(witness, "digest_domain", witness_digest_domain, context);
    assert_schema_u64_field(
        witness,
        "material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            witness,
            "material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_WITNESS_DIGEST_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
        witness,
        "trace_field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PREFIX_TRACE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_u64_field(
        witness,
        "trace_bounds_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PREFIX_TRACE_BOUNDS_FIELD_COUNT_V1,
        ),
        context,
    );
    for field in [
        "binds_galois_key_set_digest",
        "binds_trace",
        "binds_trace_bounds",
    ] {
        assert_schema_bool_field(witness, field, true, context);
    }
    let trace_profile = assert_schema_object(
        schema_value,
        "/arithmetic_trace_profile",
        "arithmetic trace profile",
    );
    for (field, expected) in [
        (
            "version",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PROFILE_VERSION_V1,
        ),
        (
            "field_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PROFILE_FIELD_COUNT_V1,
        ),
        (
            "material_version",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_VERSION_V1,
        ),
        (
            "material_field_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_FIELD_COUNT_V1,
        ),
        (
            "row_width",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1,
        ),
        (
            "private_row_count",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_COUNT_V1,
        ),
    ] {
        assert_schema_u64_field(trace_profile, field, u64::from(expected), context);
    }
    assert_schema_u64_field(
        trace_profile,
        "private_row_kind",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_KIND_V1,
        context,
    );
    assert_schema_u64_field(
        trace_profile,
        "public_row_kind",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_ROW_KIND_V1,
        context,
    );
    assert_schema_bool_field(
            trace_profile,
            "forbids_unmasked_private_row_openings",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_FORBIDS_UNMASKED_PRIVATE_ROW_OPENINGS_V1,
            context,
        );
    assert_schema_bool_field(
        trace_profile,
        "forbids_duplicate_openings",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_FORBIDS_DUPLICATE_OPENINGS_V1,
        context,
    );
    let air_contract = assert_schema_object(
        schema_value,
        "/arithmetic_air_contract",
        "arithmetic AIR contract",
    );
    assert_schema_u64_field(
            air_contract,
            "version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            air_contract,
            "field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_string_field(
        air_contract,
        "composition_challenge_domain",
        composition_challenge_domain,
        context,
    );
    assert_schema_u64_field(
            air_contract,
            "composition_challenge_digest_bytes",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_COMPOSITION_CHALLENGE_DIGEST_BYTES_V1,
            ),
            context,
        );
    for field in [
        "binds_constraint_system_digest",
        "enforces_goldilocks_field_canonicality",
        "enforces_row_kind_partition",
        "enforces_active_rows_match_witness_material",
        "enforces_full_bootstrap_arithmetic_constraints",
        "enforces_public_padding_rows",
        "enforces_statement_hash_nonzero",
        "enforces_trace_output_matches_claim",
        "enforces_trace_bound_matches_claim",
        "enforces_no_unmasked_private_row_openings",
        "enforces_duplicate_free_openings",
        "derives_opening_schedule_from_statement_hash",
        "derives_opening_schedule_from_trace_material_digest",
        "bounds_opening_schedule_rejection_sampling",
        "validates_transcript_public_padding_openings",
        "binds_composition_challenges_to_statement_hash",
        "binds_composition_challenges_to_trace_material_digest",
        "binds_composition_challenges_to_row_index",
        "binds_composition_challenges_to_column_index",
        "maps_zero_composition_challenge_to_one",
    ] {
        assert_schema_bool_field(air_contract, field, true, context);
    }
    let native_air_envelope =
        assert_schema_object(schema_value, "/native_air_envelope", "native AIR envelope");
    for field in [
        "validates_stark_parameter_profile",
        "binds_domain_tag_to_statement_hash",
        "validates_circuit_id",
        "validates_trace_width",
        "validates_query_opening_count",
        "requires_public_padding_context",
        "requires_verifier_owned_trace_material_digest",
        "rejects_auxiliary_composition_value_commitments",
        "binds_public_digest_to_statement_hash",
        "validates_merkle_path_shape",
        "validates_merkle_path_roots",
        "validates_fri_query_chain",
        "binds_first_fri_values_to_opened_air_values",
        "binds_fri_queries_to_air_commitment_roots",
        "binds_trace_root_to_governed_arithmetic_trace",
        "binds_composition_root_to_governed_air_evaluation",
        "binds_opened_rows_to_governed_arithmetic_trace",
        "binds_opened_composition_values_to_governed_air_evaluation",
        "validates_public_padding_openings",
        "requires_zero_public_padding_composition_values",
        "requires_canonical_base_transcript_label",
        "rejects_suffixed_transcript_label_aliases",
        "rejects_blank_native_envelope_bytes",
        "rejects_placeholder_native_envelope_text",
    ] {
        assert_schema_bool_field(native_air_envelope, field, true, context);
    }
    let artifact_bundle = assert_schema_object(schema_value, "/artifact_bundle", "artifact bundle");
    assert_schema_u64_field(
            artifact_bundle,
            "artifact_digest_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_MATERIAL_ARTIFACT_DIGEST_COUNT_V1,
            ),
            context,
        );
    for field in [
        "binds_arithmetic_air_constraint_system_artifact",
        "validates_arithmetic_air_constraint_system_material",
    ] {
        assert_schema_bool_field(artifact_bundle, field, true, context);
    }
    let release_prover_input = assert_schema_object(
        schema_value,
        "/release_prover_input",
        "release prover input",
    );
    assert_schema_u64_field(
        release_prover_input,
        "proof_input_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        release_prover_input,
        "proof_input_material_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_string_field(
        release_prover_input,
        "proof_input_material_digest_domain",
        proof_input_material_digest_domain,
        context,
    );
    let release_prover_digest_domains = release_prover_input
        .get("release_prover_digest_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{context} must carry release-prover digest domains"));
    for (field, expected) in [
        ("proof_input_material", proof_input_material_digest_domain),
        ("prover_input_material", prover_input_material_digest_domain),
        (
            "air_evaluation_material",
            air_evaluation_material_digest_domain,
        ),
        (
            "public_opening_material",
            public_opening_material_digest_domain,
        ),
        ("arithmetic_trace_material", trace_material_digest_domain),
        (
            "arithmetic_air_constraint_system",
            air_constraint_system_digest_domain,
        ),
    ] {
        assert_eq!(
            release_prover_digest_domains
                .get(field)
                .and_then(Value::as_str),
            Some(expected),
            "{context} release-prover digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        release_prover_digest_domains
            .get("separates_release_prover_material_domains")
            .and_then(Value::as_bool),
        Some(true),
        "{context} must advertise release-prover material digest-domain separation"
    );
    assert_ne!(
        release_prover_digest_domains
            .get("proof_input_material")
            .and_then(Value::as_str),
        release_prover_digest_domains
            .get("prover_input_material")
            .and_then(Value::as_str),
        "{context} proof-input and prover-input material domains must be distinct"
    );
    assert_ne!(
        release_prover_digest_domains
            .get("prover_input_material")
            .and_then(Value::as_str),
        release_prover_digest_domains
            .get("arithmetic_trace_material")
            .and_then(Value::as_str),
        "{context} prover-input and trace-material domains must be distinct"
    );
    assert_schema_u64_field(
        release_prover_input,
        "prover_input_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            release_prover_input,
            "prover_input_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
        release_prover_input,
        "air_evaluation_material_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            release_prover_input,
            "air_evaluation_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            release_prover_input,
            "public_opening_material_version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            release_prover_input,
            "public_opening_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    for field in [
        "hashes_proof_input_material",
        "binds_release_prover_arithmetic_air_constraint_system_digest",
        "binds_release_prover_arithmetic_air_constraint_system_artifact_digest",
        "binds_release_prover_arithmetic_air_evaluation_material_digest",
        "binds_arithmetic_air_evaluation_trace_material_digest",
        "requires_zero_arithmetic_air_composition_values",
        "binds_arithmetic_trace_material_digest",
        "binds_trace_proof_input_consistency",
        "binds_generated_proof_key_pair",
        "binds_release_prover_verifier_key",
        "validates_artifact_bound_prover_input",
        "rejects_stale_galois_key_set_replay",
        "rejects_stale_proof_key_artifacts",
        "derives_opening_schedule_from_statement_hash",
        "derives_opening_schedule_from_trace_material_digest",
        "bounds_opening_schedule_rejection_sampling",
        "validates_transcript_public_padding_openings",
        "validates_transcript_public_opening_material",
        "requires_verifier_owned_trace_material_digest",
        "requires_canonical_base_transcript_label",
        "rejects_suffixed_transcript_label_aliases",
    ] {
        assert_schema_bool_field(release_prover_input, field, true, context);
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn assert_soracloud_release_audit_schema_sections(schema_value: &Value, context: &str) {
    let evidence_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_DIGEST_DOMAIN,
    )
    .expect("release-audit evidence digest domain is valid UTF-8");
    let record_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_DIGEST_DOMAIN,
    )
    .expect("release-audit record digest domain is valid UTF-8");
    let manifest_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_DIGEST_DOMAIN,
    )
    .expect("release-audit manifest digest domain is valid UTF-8");
    let package_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_DIGEST_DOMAIN,
    )
    .expect("release-audit package digest domain is valid UTF-8");
    let circuit_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap circuit-material digest domain is valid UTF-8");
    let evaluator_artifact_set_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EVALUATOR_ARTIFACT_SET_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap evaluator-artifact-set digest domain is valid UTF-8");
    let circuit_artifact_bundle_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_CIRCUIT_ARTIFACT_BUNDLE_DIGEST_DOMAIN,
    )
    .expect("full-bootstrap circuit-artifact-bundle digest domain is valid UTF-8");
    let proof_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution proof input material digest domain is valid UTF-8");
    let prover_input_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("execution prover input material digest domain is valid UTF-8");
    let air_evaluation_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR evaluation material digest domain is valid UTF-8");
    let public_opening_material_digest_domain = std::str::from_utf8(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_DIGEST_DOMAIN,
        )
        .expect("arithmetic trace public-opening material digest domain is valid UTF-8");
    let trace_material_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN,
    )
    .expect("arithmetic trace material digest domain is valid UTF-8");
    let air_constraint_system_digest_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN,
    )
    .expect("arithmetic AIR constraint-system digest domain is valid UTF-8");
    let proof_key_material_commitment_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_COMMITMENT_DOMAIN,
    )
    .expect("proof-key material commitment domain is valid UTF-8");
    let proof_key_pair_commitment_domain = std::str::from_utf8(
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_PROOF_KEY_PAIR_COMMITMENT_DOMAIN,
    )
    .expect("proof-key pair commitment domain is valid UTF-8");
    let evidence = assert_schema_object(
        schema_value,
        "/release_audit_evidence",
        "release-audit evidence",
    );
    assert_schema_u64_field(
        evidence,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        evidence,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(evidence, "digest_domain", evidence_digest_domain, context);
    assert_schema_u64_field(
        evidence,
        "proof_profile_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PROOF_PROFILE_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_u64_field(
            evidence,
            "proof_profile_public_opening_material_version",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_VERSION_V1,
            ),
            context,
        );
    assert_schema_u64_field(
            evidence,
            "proof_profile_public_opening_material_field_count",
            u64::from(
                iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PUBLIC_OPENING_MATERIAL_FIELD_COUNT_V1,
            ),
            context,
        );
    let proof_profile_release_prover_digest_domains = evidence
        .get("proof_profile_release_prover_digest_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-profile release-prover digest domains")
        });
    for (field, expected) in [
        ("proof_input_material", proof_input_material_digest_domain),
        ("prover_input_material", prover_input_material_digest_domain),
        (
            "air_evaluation_material",
            air_evaluation_material_digest_domain,
        ),
        (
            "public_opening_material",
            public_opening_material_digest_domain,
        ),
        ("arithmetic_trace_material", trace_material_digest_domain),
        (
            "arithmetic_air_constraint_system",
            air_constraint_system_digest_domain,
        ),
    ] {
        assert_eq!(
            proof_profile_release_prover_digest_domains
                .get(field)
                .and_then(Value::as_str),
            Some(expected),
            "{context} proof-profile release-prover digest-domain field `{field}` drifted"
        );
    }
    assert_eq!(
        proof_profile_release_prover_digest_domains
            .get("separates_release_prover_material_domains")
            .and_then(Value::as_bool),
        Some(true),
        "{context} proof-profile must advertise release-prover material digest-domain separation"
    );
    assert_schema_bool_field(
        evidence,
        "proof_profile_validates_transcript_public_opening_material",
        true,
        context,
    );
    assert_schema_bool_field(
        evidence,
        "proof_profile_requires_verifier_owned_trace_material_digest",
        true,
        context,
    );
    let proof_profile_proof_key_commitment_domains = evidence
        .get("proof_profile_proof_key_commitment_domains")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!("{context} must carry proof-profile proof-key commitment domains")
        });
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("material")
            .and_then(Value::as_str),
        Some(proof_key_material_commitment_domain),
        "{context} proof-profile proof-key material commitment domain drifted"
    );
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("pair")
            .and_then(Value::as_str),
        Some(proof_key_pair_commitment_domain),
        "{context} proof-profile proof-key pair commitment domain drifted"
    );
    assert_eq!(
        proof_profile_proof_key_commitment_domains
            .get("separates_material_and_pair")
            .and_then(Value::as_bool),
        Some(true),
        "{context} proof-profile must advertise proof-key material/pair commitment-domain separation"
    );
    assert_schema_u64_field(
        evidence,
        "key_evidence_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_KEY_EVIDENCE_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_soracloud_artifact_digest_domains(
        schema_value,
        "/release_audit_evidence/artifact_digest_domains",
        context,
        circuit_material_digest_domain,
        evaluator_artifact_set_digest_domain,
        circuit_artifact_bundle_digest_domain,
    );
    let signoff = assert_schema_object(
        schema_value,
        "/release_audit_signoff",
        "release-audit signoff",
    );
    assert_schema_u64_field(
        signoff,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_FIELD_COUNT_V1),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "payload_version",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_PAYLOAD_VERSION_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "payload_field_count",
        u64::from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SIGNOFF_PAYLOAD_FIELD_COUNT_V1,
        ),
        context,
    );
    assert_schema_u64_field(
        signoff,
        "reviewer_id_max_bytes",
        u64::try_from(
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REVIEWER_ID_MAX_BYTES,
        )
        .expect("release-audit reviewer id max bytes fits u64"),
        context,
    );
    for field in [
        "binds_release_audit_evidence_digest",
        "binds_generated_circuit_body_digest",
        "binds_centered_scale_round_source_chain_digest",
        "binds_prover_native_payload_digest",
        "binds_verifier_native_payload_digest",
        "binds_external_audit_report_digest",
        "binds_evidence_archive_digest",
    ] {
        assert_schema_bool_field(signoff, field, true, context);
    }
    let record = assert_schema_object(
        schema_value,
        "/release_audit_record",
        "release-audit record",
    );
    assert_schema_u64_field(
        record,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        record,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_RECORD_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(record, "digest_domain", record_digest_domain, context);
    let manifest = assert_schema_object(
        schema_value,
        "/release_audit_manifest",
        "release-audit manifest",
    );
    assert_schema_u64_field(
        manifest,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        manifest,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_MANIFEST_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(manifest, "digest_domain", manifest_digest_domain, context);
    assert_schema_string_field(
        manifest,
        "scope",
        iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_SCOPE_V1,
        context,
    );
    for field in [
        "binds_release_audit_record_digest",
        "binds_release_audit_evidence_digest",
        "binds_centered_scale_round_source_chain_digest",
        "binds_artifact_bundle_digest",
        "binds_evaluator_artifact_set_digest",
        "binds_proof_key_pair_commitment",
        "binds_prover_native_payload_digest",
        "binds_verifier_native_payload_digest",
        "binds_native_circuit_fingerprint",
        "binds_generated_circuit_body_digest",
        "binds_external_audit_report_digest",
        "binds_evidence_archive_digest",
    ] {
        assert_schema_bool_field(manifest, field, true, context);
    }
    let package = assert_schema_object(
        schema_value,
        "/release_audit_package",
        "release-audit package",
    );
    assert_schema_u64_field(
        package,
        "version",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_VERSION_V1),
        context,
    );
    assert_schema_u64_field(
        package,
        "field_count",
        u64::from(iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_PACKAGE_FIELD_COUNT_V1),
        context,
    );
    assert_schema_string_field(package, "digest_domain", package_digest_domain, context);
    for (field, expected) in [
        (
            "audit_report_max_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_MAX_BYTES,
        ),
        (
            "audit_archive_max_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_MAX_BYTES,
        ),
        (
            "audit_report_body_min_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_BODY_MIN_BYTES,
        ),
        (
            "audit_archive_body_min_bytes",
            iroha_crypto::fhe_bfv::BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_BODY_MIN_BYTES,
        ),
    ] {
        assert_schema_u64_field(
            package,
            field,
            u64::try_from(expected).expect("release-audit byte bound fits u64"),
            context,
        );
    }
    for field in [
        "requires_evidence_archive_body_prover_native_payload_digest",
        "requires_evidence_archive_body_verifier_native_payload_digest",
    ] {
        assert_schema_bool_field(package, field, true, context);
    }
}
