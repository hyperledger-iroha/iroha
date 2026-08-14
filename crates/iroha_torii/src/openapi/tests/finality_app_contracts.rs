#[test]
fn sumeragi_v2_da_schema_requires_reed_solomon16_without_plain_compatibility() {
    let schemas = openapi_schemas();
    let encoding = schemas
        .get("SumeragiV2PayloadEncoding")
        .and_then(Value::as_object)
        .expect("Sumeragi v2 payload encoding schema");
    let allowed_encodings = encoding
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("encoding"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("enum"))
        .and_then(Value::as_array)
        .expect("Sumeragi v2 payload encoding enum");
    assert_eq!(allowed_encodings, &[Value::from("reed_solomon16")]);
    assert!(
        !allowed_encodings
            .iter()
            .any(|encoding| encoding.as_str() == Some("plain")),
        "retired Plain payload encoding must not be advertised"
    );
    let layout = schemas
        .get("SumeragiV2DataAvailabilityLayout")
        .and_then(Value::as_object)
        .expect("Sumeragi v2 data-availability layout schema");
    assert!(
        layout.get("oneOf").is_none() && layout.get("anyOf").is_none(),
        "the RS16-only layout must not retain a compatibility union"
    );
    let required = layout
        .get("required")
        .and_then(Value::as_array)
        .expect("Sumeragi v2 data-availability required fields");
    for field in ["encoding", "data_shards", "parity_shards"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "Sumeragi v2 data-availability layout must require {field}"
        );
    }
    let properties = layout
        .get("properties")
        .and_then(Value::as_object)
        .expect("Sumeragi v2 data-availability properties");
    assert_eq!(
        properties
            .get("encoding")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2PayloadEncoding")
    );
    for field in ["data_shards", "parity_shards"] {
        assert_eq!(
            properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("minimum"))
                .and_then(Value::as_u64),
            Some(1),
            "RS16 requires a positive {field} value"
        );
    }
    let serialized_layout = norito::json::to_string(&Value::Object(layout.clone()))
        .expect("serialize Sumeragi v2 data-availability layout schema");
    assert!(
        !serialized_layout.contains("\"plain\""),
        "retired Plain layout branch must not be advertised"
    );
}
#[test]
fn bridge_finality_v2_schemas_are_exact_closed_and_bounded() {
    fn schema<'a>(schemas: &'a Map, name: &str) -> &'a Map {
        schemas
            .get(name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing object schema {name}"))
    }
    fn string_set(value: Option<&Value>, label: &str) -> Vec<String> {
        let mut values = value
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{label} must be an array"))
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("{label} entries must be strings"))
                    .to_owned()
            })
            .collect::<Vec<_>>();
        values.sort_unstable();
        values
    }
    fn assert_closed_shape(schemas: &Map, name: &str, required: &[&str], properties: &[&str]) {
        let schema = schema(schemas, name);
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false)),
            "{name} must reject unknown fields"
        );
        let mut expected_required = required
            .iter()
            .map(|field| (*field).to_owned())
            .collect::<Vec<_>>();
        expected_required.sort_unstable();
        assert_eq!(
            string_set(schema.get("required"), &format!("{name}.required")),
            expected_required,
            "{name} required-field drift"
        );
        let mut actual_properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name}.properties must be an object"))
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        actual_properties.sort_unstable();
        let mut expected_properties = properties
            .iter()
            .map(|field| (*field).to_owned())
            .collect::<Vec<_>>();
        expected_properties.sort_unstable();
        assert_eq!(
            actual_properties, expected_properties,
            "{name} property-set drift"
        );
    }
    fn property<'a>(schemas: &'a Map, schema_name: &str, field: &str) -> &'a Map {
        schema(schemas, schema_name)
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing {schema_name}.{field} schema"))
    }
    fn assert_ref(schemas: &Map, schema_name: &str, field: &str, expected: &str) {
        assert_eq!(
            property(schemas, schema_name, field)
                .get("$ref")
                .and_then(Value::as_str),
            Some(expected),
            "{schema_name}.{field} reference drift"
        );
    }
    let schemas = openapi_schemas();
    let max_validators =
        u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT)
            .expect("Sumeragi validator bound must fit in u64");
    assert_closed_shape(
        &schemas,
        "BridgeFinalityProof",
        &["version", "block_header", "finality_artifact"],
        &["version", "block_header", "finality_artifact"],
    );
    assert_closed_shape(
        &schemas,
        "BridgeFinalityAttestationV1",
        &["body", "signature"],
        &["body", "signature"],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2FinalityArtifact",
        &[
            "format_version",
            "protocol_version",
            "height",
            "height_context",
            "subject",
            "block_hash",
            "commit_qc",
            "validator_set_pops",
        ],
        &[
            "format_version",
            "protocol_version",
            "height",
            "height_context",
            "subject",
            "block_hash",
            "commit_qc",
            "validator_set_pops",
        ],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2HeightContext",
        &[
            "network_id",
            "protocol_version",
            "height",
            "epoch",
            "epoch_end_height",
            "mode",
            "roster",
            "quorum",
            "nexus_amx_context_hash",
            "execution_policy_hash",
            "da_layout",
            "leader_seed",
        ],
        &[
            "network_id",
            "protocol_version",
            "height",
            "epoch",
            "epoch_end_height",
            "next_epoch_snapshot",
            "mode",
            "parent_commit_qc",
            "snapshot_bootstrap",
            "roster",
            "quorum",
            "nexus_amx_context_hash",
            "execution_policy_hash",
            "da_layout",
            "leader_seed",
        ],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2ValidatorPower",
        &["validator", "power"],
        &["validator", "power"],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2DualQuorum",
        &["min_signers", "total_power"],
        &["min_signers", "total_power"],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2BlockSubject",
        &["block_hash", "payload_hash"],
        &["parent_block_hash", "block_hash", "payload_hash"],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2MergeCarrierCommitment",
        &["version", "entry_hash"],
        &["version", "entry_hash"],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2ExecutionCommitment",
        &[
            "parent_state_root",
            "post_state_root",
            "ordinary_writes_root",
            "topup_anchor_count",
            "native_amx_application_manifest_version",
            "native_amx_application_manifest_root",
            "native_amx_application_manifest_count",
            "merge_carrier",
            "executed_block_wire_len",
            "executed_block_wire_hash",
        ],
        &[
            "parent_state_root",
            "post_state_root",
            "ordinary_writes_root",
            "topup_anchor_root",
            "topup_anchor_count",
            "native_amx_application_manifest_version",
            "native_amx_application_manifest_root",
            "native_amx_application_manifest_count",
            "merge_carrier",
            "executed_block_wire_len",
            "executed_block_wire_hash",
        ],
    );
    for certificate in [
        "SumeragiV2QuorumCertificate",
        "SumeragiV2CommitQuorumCertificate",
    ] {
        assert_closed_shape(
            &schemas,
            certificate,
            &[
                "round",
                "proposal_round",
                "phase",
                "subject",
                "execution_commitment",
                "signers",
                "aggregate_signature",
            ],
            &[
                "round",
                "proposal_round",
                "phase",
                "subject",
                "execution_commitment",
                "signers",
                "aggregate_signature",
            ],
        );
    }
    assert_closed_shape(
        &schemas,
        "SumeragiV2SnapshotBootstrapAnchor",
        &[
            "snapshot_height",
            "snapshot_block_hash",
            "snapshot_block_creation_time_ms",
            "snapshot_state_hash",
        ],
        &[
            "snapshot_height",
            "snapshot_block_hash",
            "snapshot_block_creation_time_ms",
            "snapshot_state_hash",
        ],
    );
    assert_closed_shape(
        &schemas,
        "SumeragiV2FinalizedNextEpochSnapshot",
        &[
            "epoch",
            "epoch_end_height",
            "mode",
            "roster",
            "validator_set_pops",
            "quorum",
            "leader_seed",
        ],
        &[
            "epoch",
            "epoch_end_height",
            "mode",
            "roster",
            "validator_set_pops",
            "quorum",
            "leader_seed",
        ],
    );
    assert_closed_shape(
        &schemas,
        "BridgeCommitment",
        &[
            "network_id",
            "height_context_id",
            "block_height",
            "block_hash",
        ],
        &[
            "network_id",
            "height_context_id",
            "block_height",
            "block_hash",
        ],
    );
    assert_closed_shape(
        &schemas,
        "BridgeFinalityBundle",
        &["commitment", "finality_proof"],
        &["commitment", "finality_proof"],
    );
    assert_closed_shape(
        &schemas,
        "BlockHeader",
        &[
            "height",
            "prev_block_hash",
            "merkle_root",
            "result_merkle_root",
            "da_proof_policies_hash",
            "da_commitments_hash",
            "da_pin_intents_hash",
            "prev_roster_evidence_hash",
            "sccp_commitment_root",
            "creation_time_ms",
            "view_change_index",
            "confidential_features",
        ],
        &[
            "height",
            "prev_block_hash",
            "merkle_root",
            "result_merkle_root",
            "da_proof_policies_hash",
            "da_commitments_hash",
            "da_pin_intents_hash",
            "prev_roster_evidence_hash",
            "npos_effects_hash",
            "sccp_commitment_root",
            "creation_time_ms",
            "view_change_index",
            "confidential_features",
            "execution_context_hash",
        ],
    );
    assert_eq!(
        property(&schemas, "BridgeFinalityProof", "version")
            .get("enum")
            .and_then(Value::as_array),
        Some(&vec![Value::from(2_u64)])
    );
    for (schema_name, field, version) in [
        ("SumeragiV2FinalityArtifact", "format_version", 4_u64),
        ("SumeragiV2FinalityArtifact", "protocol_version", 4_u64),
        ("SumeragiV2HeightContext", "protocol_version", 4_u64),
    ] {
        assert_eq!(
            property(&schemas, schema_name, field)
                .get("enum")
                .and_then(Value::as_array),
            Some(&vec![Value::from(version)]),
            "{schema_name}.{field} must be version-exact"
        );
    }
    let roster = property(&schemas, "SumeragiV2HeightContext", "roster");
    assert_eq!(roster.get("minItems").and_then(Value::as_u64), Some(4));
    assert_eq!(
        roster.get("maxItems").and_then(Value::as_u64),
        Some(max_validators)
    );
    assert_eq!(
        roster.get("uniqueItems").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        roster
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2ValidatorPower")
    );
    assert_eq!(
        property(&schemas, "SumeragiV2ValidatorPower", "power")
            .get("enum")
            .and_then(Value::as_array),
        Some(&vec![Value::from(1_u64)])
    );
    assert_ref(
        &schemas,
        "SumeragiV2ValidatorPower",
        "validator",
        "#/components/schemas/SumeragiV2BlsValidatorId",
    );
    assert_eq!(
        schema(&schemas, "SumeragiV2BlsValidatorId")
            .get("pattern")
            .and_then(Value::as_str),
        Some("^ea0130[0-9A-F]{96}$")
    );
    for owner in [
        "SumeragiV2FinalityArtifact",
        "SumeragiV2FinalizedNextEpochSnapshot",
    ] {
        let pops = property(&schemas, owner, "validator_set_pops");
        assert_eq!(pops.get("minItems").and_then(Value::as_u64), Some(4));
        assert_eq!(
            pops.get("maxItems").and_then(Value::as_u64),
            Some(max_validators)
        );
        assert_eq!(
            pops.get("items")
                .and_then(Value::as_object)
                .and_then(|items| items.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/SumeragiV2BlsProof")
        );
    }
    let bls_proof = schema(&schemas, "SumeragiV2BlsProof");
    assert_eq!(bls_proof.get("minItems").and_then(Value::as_u64), Some(96));
    assert_eq!(bls_proof.get("maxItems").and_then(Value::as_u64), Some(96));
    let bls_byte = bls_proof
        .get("items")
        .and_then(Value::as_object)
        .expect("BLS proof byte schema");
    assert_eq!(bls_byte.get("minimum").and_then(Value::as_u64), Some(0));
    assert_eq!(bls_byte.get("maximum").and_then(Value::as_u64), Some(255));
    for certificate in [
        "SumeragiV2QuorumCertificate",
        "SumeragiV2CommitQuorumCertificate",
    ] {
        let signers = property(&schemas, certificate, "signers");
        assert_eq!(signers.get("minItems").and_then(Value::as_u64), Some(1));
        assert_eq!(
            signers.get("maxItems").and_then(Value::as_u64),
            Some(max_validators)
        );
        assert_eq!(
            signers.get("uniqueItems").and_then(Value::as_bool),
            Some(true)
        );
    }
    assert_eq!(
        property(&schemas, "SumeragiV2FinalizedNextEpochSnapshot", "roster")
            .get("minItems")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        property(&schemas, "SumeragiV2FinalizedNextEpochSnapshot", "roster")
            .get("maxItems")
            .and_then(Value::as_u64),
        Some(max_validators)
    );
    assert_eq!(
        property(&schemas, "SumeragiV2DualQuorum", "min_signers")
            .get("maximum")
            .and_then(Value::as_u64),
        Some(max_validators)
    );
    assert_ref(
        &schemas,
        "SumeragiV2MergeCarrierCommitment",
        "entry_hash",
        "#/components/schemas/Hash",
    );
    assert_eq!(
        property(&schemas, "SumeragiV2MergeCarrierCommitment", "version")
            .get("const")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        property(&schemas, "SumeragiV2ExecutionCommitment", "merge_carrier")
            .get("oneOf")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "merge_carrier must admit exactly null or the V1 tagged commitment"
    );
    let executed_wire_len = property(
        &schemas,
        "SumeragiV2ExecutionCommitment",
        "executed_block_wire_len",
    );
    assert_eq!(
        executed_wire_len.get("format").and_then(Value::as_str),
        Some("uint64")
    );
    assert_eq!(
        executed_wire_len.get("minimum").and_then(Value::as_u64),
        Some(1),
        "executed_block_wire_len must exclude zero"
    );
    assert_ref(
        &schemas,
        "SumeragiV2FinalityArtifact",
        "commit_qc",
        "#/components/schemas/SumeragiV2CommitQuorumCertificate",
    );
    for certificate in [
        "SumeragiV2QuorumCertificate",
        "SumeragiV2CommitQuorumCertificate",
    ] {
        assert_ref(
            &schemas,
            certificate,
            "execution_commitment",
            "#/components/schemas/SumeragiV2ExecutionCommitment",
        );
    }
    let topup_count = property(
        &schemas,
        "SumeragiV2ExecutionCommitment",
        "topup_anchor_count",
    );
    assert_eq!(topup_count.get("minimum").and_then(Value::as_u64), Some(0));
    assert_eq!(
        topup_count.get("maximum").and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK,
        ))
    );
    assert_eq!(
        schema(&schemas, "SumeragiV2ExecutionCommitment")
            .get("oneOf")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "top-up root/count presence must be encoded as an exact two-branch contract"
    );
    let native_manifest_version = property(
        &schemas,
        "SumeragiV2ExecutionCommitment",
        "native_amx_application_manifest_version",
    );
    assert_eq!(
        native_manifest_version.get("const").and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        ))
    );
    let native_manifest_count = property(
        &schemas,
        "SumeragiV2ExecutionCommitment",
        "native_amx_application_manifest_count",
    );
    assert_eq!(
        native_manifest_count.get("minimum").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        native_manifest_count.get("maximum").and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES,
        ))
    );
    assert_eq!(
        schema(&schemas, "SumeragiV2ExecutionCommitment")
            .get("allOf")
            .and_then(Value::as_array)
            .and_then(|all_of| all_of.first())
            .and_then(Value::as_object)
            .and_then(|manifest_contract| manifest_contract.get("oneOf"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "Native AMX manifest count/root emptiness must be encoded as an exact two-branch contract"
    );
    assert_eq!(
        property(&schemas, "SumeragiV2CommitPhase", "phase")
            .get("enum")
            .and_then(Value::as_array),
        Some(&vec![Value::from("commit")])
    );
    let context_id = schema(&schemas, "SumeragiV2HeightContextId");
    assert_eq!(context_id.get("minItems").and_then(Value::as_u64), Some(1));
    assert_eq!(context_id.get("maxItems").and_then(Value::as_u64), Some(1));
    let mut bridge_components = Map::new();
    for name in [
        "BridgeFinalityProof",
        "BridgeFinalityAttestationBodyV1",
        "BridgeFinalityAttestationV1",
        "BridgeCommitment",
        "BridgeFinalityBundle",
        "SumeragiV2FinalityArtifact",
        "SumeragiV2FinalizedNextEpochSnapshot",
        "SumeragiV2HeightContext",
        "SumeragiV2ValidatorPower",
        "SumeragiV2DualQuorum",
        "SumeragiV2BlockSubject",
        "SumeragiV2MergeCarrierCommitment",
        "SumeragiV2ExecutionCommitment",
        "SumeragiV2QuorumCertificate",
        "SumeragiV2CommitQuorumCertificate",
    ] {
        bridge_components.insert(
            name.to_owned(),
            schemas
                .get(name)
                .unwrap_or_else(|| panic!("missing bridge component {name}"))
                .clone(),
        );
    }
    let serialized = norito::json::to_string(&Value::Object(bridge_components))
        .expect("serialize bridge finality schemas");
    for retired in [
        "authority_set",
        "next_authority_set",
        "justification",
        "signatures",
        "validator_set_hash",
        "validator_set_hash_version",
        "validator_set",
        "subject_block_hash",
        "mode_tag",
        "highest_qc",
        "aggregate",
        "signers_bitmap",
        "bls_aggregate_signature",
        "mmr_root",
        "mmr_leaf_index",
        "mmr_peaks",
    ] {
        assert!(
            !serialized.contains(&format!("\"{retired}\"")),
            "retired v1 bridge-finality field `{retired}` reappeared"
        );
    }
}
#[test]
fn bridge_finality_schema_matches_norito_json_and_decoder_rejects_v1_fields() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let proof = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("decode exact SCCP v2 finality fixture");
    let value = norito::json::to_value(&proof).expect("serialize exact finality proof");
    let proof_object = value.as_object().expect("proof JSON object");
    let mut proof_fields = proof_object.keys().map(String::as_str).collect::<Vec<_>>();
    proof_fields.sort_unstable();
    assert_eq!(
        proof_fields,
        ["block_header", "finality_artifact", "version"]
    );
    let header = proof_object
        .get("block_header")
        .and_then(Value::as_object)
        .expect("block header JSON object");
    for required in [
        "height",
        "prev_block_hash",
        "merkle_root",
        "result_merkle_root",
        "da_proof_policies_hash",
        "da_commitments_hash",
        "da_pin_intents_hash",
        "prev_roster_evidence_hash",
        "sccp_commitment_root",
        "creation_time_ms",
        "view_change_index",
        "confidential_features",
    ] {
        assert!(
            header.contains_key(required),
            "serialized block header omitted required JSON field {required}"
        );
    }
    assert!(!header.contains_key("npos_effects_hash"));
    assert!(!header.contains_key("execution_context_hash"));
    let expected_commitment_root = hex::encode_upper(fixture.bundle.commitment_root);
    assert_eq!(
        header.get("sccp_commitment_root").and_then(Value::as_str),
        Some(expected_commitment_root.as_str())
    );
    let document = generate_spec();
    let schemas = component_schemas(&document);
    let bytes32 = schemas
        .get("SumeragiV2Bytes32")
        .and_then(Value::as_object)
        .expect("Sumeragi v2 bytes32 schema");
    assert_eq!(bytes32.get("type").and_then(Value::as_str), Some("string"));
    assert_eq!(bytes32.get("minLength").and_then(Value::as_u64), Some(64));
    assert_eq!(bytes32.get("maxLength").and_then(Value::as_u64), Some(64));
    assert_eq!(
        bytes32.get("pattern").and_then(Value::as_str),
        Some("^[0-9A-F]{64}$")
    );
    let fixed_bytes32 = schemas
        .get("SumeragiV2Fixed32ByteArray")
        .and_then(Value::as_object)
        .expect("Sumeragi v2 fixed bytes32 adapter schema");
    assert_eq!(
        fixed_bytes32.get("type").and_then(Value::as_str),
        Some("array")
    );
    assert_eq!(
        fixed_bytes32.get("minItems").and_then(Value::as_u64),
        Some(32)
    );
    assert_eq!(
        fixed_bytes32.get("maxItems").and_then(Value::as_u64),
        Some(32)
    );
    let artifact = proof_object
        .get("finality_artifact")
        .and_then(Value::as_object)
        .expect("v2 artifact JSON object");
    let mut artifact_fields = artifact.keys().map(String::as_str).collect::<Vec<_>>();
    artifact_fields.sort_unstable();
    assert_eq!(
        artifact_fields,
        [
            "block_hash",
            "commit_qc",
            "format_version",
            "height",
            "height_context",
            "protocol_version",
            "subject",
            "validator_set_pops",
        ]
    );
    assert_eq!(
        artifact.get("format_version").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        artifact.get("protocol_version").and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::PROTOCOL_VERSION
        ))
    );
    let context = artifact
        .get("height_context")
        .and_then(Value::as_object)
        .expect("height context JSON object");
    assert_eq!(
        context.get("protocol_version").and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::PROTOCOL_VERSION
        ))
    );
    assert!(!context.contains_key("next_epoch_snapshot"));
    assert!(!context.contains_key("parent_commit_qc"));
    assert!(!context.contains_key("snapshot_bootstrap"));
    let mode = context
        .get("mode")
        .and_then(Value::as_object)
        .expect("adjacently tagged consensus mode");
    assert_eq!(mode.get("mode").and_then(Value::as_str), Some("npos"));
    assert_eq!(mode.get("details"), Some(&Value::Null));
    let encoding = context
        .get("da_layout")
        .and_then(Value::as_object)
        .and_then(|layout| layout.get("encoding"))
        .and_then(Value::as_object)
        .expect("adjacently tagged payload encoding");
    assert_eq!(
        encoding.get("encoding").and_then(Value::as_str),
        Some("reed_solomon16")
    );
    assert_eq!(encoding.get("details"), Some(&Value::Null));
    let roster = context
        .get("roster")
        .and_then(Value::as_array)
        .expect("powered roster array");
    assert_eq!(roster.len(), 4);
    for validator in roster {
        let validator = validator.as_object().expect("validator-power object");
        let public_key = validator
            .get("validator")
            .and_then(Value::as_str)
            .expect("validator id string");
        assert_eq!(public_key.len(), 102);
        assert!(public_key.starts_with("ea0130"));
        assert!(
            public_key[6..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        );
        assert!(validator.get("power").and_then(Value::as_u64).is_some());
    }
    let quorum = context
        .get("quorum")
        .and_then(Value::as_object)
        .expect("dual quorum object");
    assert_eq!(quorum.len(), 2);
    assert!(quorum.get("min_signers").and_then(Value::as_u64).is_some());
    assert!(quorum.get("total_power").and_then(Value::as_u64).is_some());
    let subject = artifact
        .get("subject")
        .and_then(Value::as_object)
        .expect("block subject object");
    assert!(!subject.contains_key("parent_block_hash"));
    for hash_field in ["block_hash", "payload_hash"] {
        let hash = subject
            .get(hash_field)
            .and_then(Value::as_str)
            .expect("canonical hash literal");
        assert!(hash.starts_with("hash:"));
        assert_eq!(hash.len(), 74);
    }
    let commit_qc = artifact
        .get("commit_qc")
        .and_then(Value::as_object)
        .expect("commit QC object");
    let phase = commit_qc
        .get("phase")
        .and_then(Value::as_object)
        .expect("adjacently tagged commit phase");
    assert_eq!(phase.get("phase").and_then(Value::as_str), Some("commit"));
    assert_eq!(phase.get("details"), Some(&Value::Null));
    assert!(
        commit_qc
            .get("round")
            .and_then(Value::as_object)
            .and_then(|round| round.get("context_id"))
            .and_then(Value::as_array)
            .is_some_and(|context_id| context_id.len() == 1)
    );
    assert_eq!(commit_qc.get("proposal_round"), commit_qc.get("round"));
    let execution_commitment = commit_qc
        .get("execution_commitment")
        .and_then(Value::as_object)
        .expect("mandatory execution commitment object");
    let mut execution_fields = execution_commitment
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    execution_fields.sort_unstable();
    assert_eq!(
        execution_fields,
        [
            "executed_block_wire_hash",
            "executed_block_wire_len",
            "native_amx_application_manifest_count",
            "native_amx_application_manifest_root",
            "native_amx_application_manifest_version",
            "merge_carrier",
            "ordinary_writes_root",
            "parent_state_root",
            "post_state_root",
            "topup_anchor_count",
        ],
        "zero-top-up commitment must omit only its optional top-up root"
    );
    for root in [
        "parent_state_root",
        "post_state_root",
        "ordinary_writes_root",
        "native_amx_application_manifest_root",
    ] {
        assert!(
            execution_commitment
                .get(root)
                .and_then(Value::as_str)
                .is_some_and(|hash| hash.starts_with("hash:") && hash.len() == 74),
            "execution commitment omitted canonical {root}"
        );
    }
    assert_eq!(
        execution_commitment.get("merge_carrier"),
        Some(&Value::Null),
        "ordinary finality commitment must serialize an explicit null merge carrier"
    );
    assert!(
        execution_commitment
            .get("executed_block_wire_len")
            .and_then(Value::as_u64)
            .is_some_and(|len| len > 0),
        "execution commitment must serialize the exact non-zero block wire length"
    );
    assert_eq!(
        execution_commitment
            .get("topup_anchor_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        execution_commitment
            .get("native_amx_application_manifest_version")
            .and_then(Value::as_u64),
        Some(u64::from(
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        ))
    );
    assert_eq!(
        execution_commitment
            .get("native_amx_application_manifest_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    let expected_native_manifest_empty_root = norito::json::to_value(
        &iroha_data_model::block::consensus_v2::native_amx_application_manifest_empty_root(),
    )
    .expect("serialize canonical Native AMX application manifest empty root");
    assert_eq!(
        execution_commitment.get("native_amx_application_manifest_root"),
        Some(&expected_native_manifest_empty_root)
    );
    assert!(
        commit_qc
            .get("aggregate_signature")
            .and_then(Value::as_array)
            .is_some_and(|signature| signature.len() == 96)
    );
    let pops = artifact
        .get("validator_set_pops")
        .and_then(Value::as_array)
        .expect("validator PoP array");
    assert_eq!(pops.len(), roster.len());
    assert!(pops.iter().all(|pop| {
        pop.as_array().is_some_and(|bytes| {
            bytes.len() == 96
                && bytes
                    .iter()
                    .all(|byte| byte.as_u64().is_some_and(|byte| byte <= 255))
        })
    }));
    let mut missing_execution = value.clone();
    let removed = missing_execution
        .as_object_mut()
        .and_then(|proof| proof.get_mut("finality_artifact"))
        .and_then(Value::as_object_mut)
        .and_then(|artifact| artifact.get_mut("commit_qc"))
        .and_then(Value::as_object_mut)
        .expect("mutable commit QC object")
        .remove("execution_commitment");
    assert!(
        removed.is_some(),
        "fixture commit QC must carry execution commitment"
    );
    assert!(
        norito::json::from_value::<iroha_data_model::bridge::BridgeFinalityProof>(
            missing_execution,
        )
        .is_err(),
        "BridgeFinalityProof JSON decoder accepted a CommitQC without its execution commitment"
    );
    for retired in [
        "height",
        "chain_id",
        "block_hash",
        "commit_qc",
        "validator_set_pops",
        "authority_set",
        "next_authority_set",
        "justification",
        "signatures",
        "validator_set_hash",
        "validator_set_hash_version",
        "validator_set",
        "subject_block_hash",
        "parent_state_root",
        "post_state_root",
        "mode_tag",
        "highest_qc",
        "aggregate",
        "signers_bitmap",
        "bls_aggregate_signature",
    ] {
        let mut hostile = value.clone();
        hostile
            .as_object_mut()
            .expect("proof JSON object")
            .insert(retired.to_owned(), Value::Null);
        assert!(
            norito::json::from_value::<iroha_data_model::bridge::BridgeFinalityProof>(hostile)
                .is_err(),
            "BridgeFinalityProof JSON decoder accepted retired v1 field {retired}"
        );
    }
}
#[test]
fn bridge_finality_operations_describe_durable_v2_evidence() {
    let paths = generate_spec()
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths")
        .clone();
    for (path, response_schema) in [
        (
            "/v1/bridge/finality/{height}",
            "#/components/schemas/BridgeFinalityProof",
        ),
        (
            "/v1/bridge/finality/attestation/{height}",
            "#/components/schemas/BridgeFinalityAttestationV1",
        ),
        (
            "/v1/bridge/finality/bundle/{height}",
            "#/components/schemas/BridgeFinalityBundle",
        ),
    ] {
        let operation = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing GET operation for {path}"));
        let description = operation
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing GET description for {path}"));
        assert!(description.contains("Sumeragi-v2"));
        assert!(description.contains("durable"));
        assert!(!description.contains("validator set signatures"));
        assert!(!description.contains("block header and commit certificate"));
        let content = operation
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing successful response content for {path}"));
        assert_eq!(
            content
                .get("application/json")
                .and_then(Value::as_object)
                .and_then(|media| media.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(response_schema),
            "{path} JSON response schema"
        );
        let norito = content
            .get("application/x-norito")
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing Norito response schema for {path}"));
        assert_eq!(norito.get("type").and_then(Value::as_str), Some("string"));
        assert_eq!(norito.get("format").and_then(Value::as_str), Some("binary"));
    }
    let attestation = paths
        .get("/v1/bridge/finality/attestation/{height}")
        .and_then(Value::as_object)
        .and_then(|path| path.get("get"))
        .and_then(Value::as_object)
        .expect("attestation GET");
    let challenge = attestation
        .get("parameters")
        .and_then(Value::as_array)
        .and_then(|parameters| {
            parameters.iter().find(|parameter| {
                parameter
                    .as_object()
                    .and_then(|parameter| parameter.get("name"))
                    .and_then(Value::as_str)
                    == Some("X-Iroha-Finality-Challenge")
            })
        })
        .and_then(Value::as_object)
        .expect("attestation challenge parameter");
    assert_eq!(challenge.get("in").and_then(Value::as_str), Some("header"));
    assert_eq!(
        challenge.get("required").and_then(Value::as_bool),
        Some(true)
    );
    let responses = attestation
        .get("responses")
        .and_then(Value::as_object)
        .expect("attestation responses");
    for status in ["200", "400", "404", "406", "503"] {
        let headers = responses
            .get(status)
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("attestation {status} response headers"));
        assert_eq!(
            headers
                .get("Cache-Control")
                .and_then(Value::as_object)
                .and_then(|header| header.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some("no-store"),
            "attestation {status} Cache-Control"
        );
        assert_eq!(
            headers
                .get("Vary")
                .and_then(Value::as_object)
                .and_then(|header| header.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some("X-Iroha-Finality-Challenge, Accept"),
            "attestation {status} Vary"
        );
    }
}
#[test]
fn generated_spec_documents_read_only_nexus_lifecycle_status() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    let path = paths
        .get("/v1/nexus/lifecycle")
        .and_then(Value::as_object)
        .expect("Nexus lifecycle path");
    let get = path
        .get("get")
        .and_then(Value::as_object)
        .expect("Nexus lifecycle GET operation");
    let responses = get
        .get("responses")
        .and_then(Value::as_object)
        .expect("Nexus lifecycle GET responses");
    let status_schema_ref = responses
        .get("200")
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    assert_eq!(
        status_schema_ref,
        Some("#/components/schemas/NexusLaneLifecycleStatusV1")
    );
    let content = responses
        .get("200")
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .expect("lifecycle status response content");
    assert!(content.contains_key("application/json"));
    assert!(content.contains_key("application/x-norito"));
    assert!(
        !path.contains_key("post"),
        "the first-release lifecycle resource is read-only"
    );
    let schema = doc
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .and_then(|schemas| schemas.get("NexusLaneLifecycleStatusV1"))
        .and_then(Value::as_object)
        .expect("NexusLaneLifecycleStatusV1 schema");
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&Value::Bool(false))
    );
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("required lifecycle response fields");
    for field in [
        "version",
        "nexus_enabled",
        "lane_count",
        "lanes",
        "catalog_hash",
        "incarnations",
        "incarnation_root",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "Nexus lifecycle status should require {field}"
        );
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("lifecycle response properties");
    assert_eq!(
        properties
            .get("lanes")
            .and_then(Value::as_object)
            .and_then(|property| property.get("type"))
            .and_then(Value::as_str),
        Some("array")
    );
    assert!(properties.contains_key("catalog_hash"));
    assert!(properties.contains_key("incarnations"));
    assert!(properties.contains_key("incarnation_root"));
    assert_eq!(
        properties
            .get("incarnations")
            .and_then(Value::as_object)
            .and_then(|property| property.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NexusLaneLifecycleIncarnationEntry")
    );
}
#[test]
fn generated_spec_documents_exact_authoritative_sumeragi_v2_status() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    let status_schema_ref = paths
        .get("/v1/sumeragi/status")
        .and_then(Value::as_object)
        .and_then(|path| path.get("get"))
        .and_then(Value::as_object)
        .and_then(|operation| operation.get("responses"))
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("200"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    let status_enabled =
        catalog_openapi_route_enabled(CatalogHttpMethod::Get, "/v1/sumeragi/status");
    assert_eq!(
        status_schema_ref,
        status_enabled.then_some("#/components/schemas/SumeragiStatusResponse"),
        "authoritative status presence and schema must follow the enabled catalog OpenAPI projection"
    );
    let schemas = doc
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("components schemas");
    let max_validators =
        u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT)
            .expect("Sumeragi validator bound must fit in u64");
    let status_schema = schemas
        .get("SumeragiStatusResponse")
        .and_then(Value::as_object)
        .expect("status response schema");
    assert_eq!(
        status_schema
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    let status_properties = status_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("status response properties");
    assert_eq!(
        status_properties
            .get("protocol_version")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("minimum"))
            .and_then(Value::as_u64),
        Some(4)
    );
    assert!(status_properties.contains_key("height_context_id"));
    assert!(status_properties.contains_key("restart_required"));
    assert!(status_properties.contains_key("pending_persistence_id"));
    assert!(status_properties.contains_key("last_committed_subject"));
    assert!(status_properties.contains_key("height_context"));
    assert!(status_properties.contains_key("last_commit_qc"));
    assert!(status_properties.contains_key("liveness"));
    assert!(!status_properties.contains_key("lane_settlement_commitments"));
    assert!(!status_properties.contains_key("lane_relay_envelopes"));
    assert!(!status_properties.contains_key("rbc_status"));
    assert!(!status_properties.contains_key("missing_qc_total"));
    for field in [
        "node_fingerprint",
        "build_fingerprint",
        "config_fingerprint",
    ] {
        assert_eq!(
            status_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/Hash")
        );
    }
    let height_context = status_properties
        .get("height_context_id")
        .and_then(Value::as_object)
        .expect("height context id schema");
    assert_eq!(
        height_context.get("$ref").and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2HeightContextId")
    );
    for (field, schema_ref) in [
        ("phase", "#/components/schemas/SumeragiV2StatusPhase"),
        ("body_state", "#/components/schemas/SumeragiV2BodyState"),
        (
            "locked_prepare_qc",
            "#/components/schemas/SumeragiV2QuorumCertificateRef",
        ),
        (
            "highest_prepare_qc",
            "#/components/schemas/SumeragiV2QuorumCertificateRef",
        ),
        (
            "last_timeout_certificate",
            "#/components/schemas/SumeragiV2TimeoutCertificateRef",
        ),
        (
            "last_committed_subject",
            "#/components/schemas/SumeragiV2BlockSubject",
        ),
        (
            "height_context",
            "#/components/schemas/SumeragiV2HeightContextStatus",
        ),
        (
            "last_commit_qc",
            "#/components/schemas/SumeragiV2CommitQcStatus",
        ),
        ("liveness", "#/components/schemas/SumeragiV2LivenessStatus"),
    ] {
        assert_eq!(
            status_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(schema_ref),
            "status field {field} must retain its exact consensus type"
        );
    }
    let required = status_schema
        .get("required")
        .and_then(Value::as_array)
        .expect("status required fields");
    for field in ["restart_required", "height_context", "liveness"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "status must require {field}"
        );
    }
    assert!(
        !required
            .iter()
            .any(|value| value.as_str() == Some("last_commit_qc")),
        "last CommitQC must remain optional before an authenticated summary exists"
    );
    let context_schema = schemas
        .get("SumeragiV2HeightContextStatus")
        .and_then(Value::as_object)
        .expect("height context status schema");
    let context_properties = context_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("height context status properties");
    assert_eq!(
        context_properties
            .get("mode")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2ConsensusMode")
    );
    assert_eq!(
        context_properties
            .get("quorum")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2DualQuorum")
    );
    assert_eq!(
        context_properties
            .get("validator_count")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("maximum"))
            .and_then(Value::as_u64),
        Some(max_validators)
    );
    let commit_schema = schemas
        .get("SumeragiV2CommitQcStatus")
        .and_then(Value::as_object)
        .expect("CommitQC status schema");
    let commit_properties = commit_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("CommitQC status properties");
    assert_eq!(
        commit_properties
            .get("certificate")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2QuorumCertificateRef")
    );
    for field in ["validator_count", "signer_count", "min_signers"] {
        assert_eq!(
            commit_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(max_validators),
            "CommitQC status {field} must match the reducer validator bound"
        );
    }
    let diagnostics_schema_ref = paths
        .get("/v1/sumeragi/diagnostics")
        .and_then(Value::as_object)
        .and_then(|path| path.get("get"))
        .and_then(Value::as_object)
        .and_then(|operation| operation.get("responses"))
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("200"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    let diagnostics_enabled =
        catalog_openapi_route_enabled(CatalogHttpMethod::Get, "/v1/sumeragi/diagnostics");
    assert_eq!(
        diagnostics_schema_ref,
        diagnostics_enabled.then_some("#/components/schemas/SumeragiDiagnosticsResponse"),
        "operator diagnostics presence and schema must follow the enabled catalog OpenAPI projection"
    );
    let diagnostics_schema = schemas
        .get("SumeragiDiagnosticsResponse")
        .and_then(Value::as_object)
        .expect("diagnostics response schema");
    assert_eq!(
        diagnostics_schema
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    let diagnostics_properties = diagnostics_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("diagnostics response properties");
    assert!(diagnostics_properties.contains_key("npos"));
    for (field, schema_ref) in [
        (
            "lane_commitments",
            "#/components/schemas/SumeragiLaneCommitment",
        ),
        (
            "dataspace_commitments",
            "#/components/schemas/SumeragiDataspaceCommitment",
        ),
        (
            "lane_payload_ownerships",
            "#/components/schemas/SumeragiLanePayloadOwnership",
        ),
        (
            "committed_lane_blocks",
            "#/components/schemas/SumeragiCommittedLaneBlock",
        ),
        (
            "lane_block_sessions",
            "#/components/schemas/SumeragiLaneBlockSessionStatus",
        ),
        (
            "lane_governance",
            "#/components/schemas/SumeragiLaneGovernance",
        ),
    ] {
        assert_eq!(
            diagnostics_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("items"))
                .and_then(Value::as_object)
                .and_then(|items| items.get("$ref"))
                .and_then(Value::as_str),
            Some(schema_ref),
            "diagnostics field {field} must retain its exact wire type"
        );
    }
    for canonical_field in ["height", "view", "phase", "leader", "locked_prepare_qc"] {
        assert!(
            !diagnostics_properties.contains_key(canonical_field),
            "diagnostics must not duplicate canonical field {canonical_field}"
        );
    }
    let commitment_properties = schemas
        .get("LaneSettlementCommitment")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("lane settlement commitment properties");
    assert_eq!(
        commitment_properties
            .get("native_amx_receipts")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NativeAmxReceipt")
    );
    assert_eq!(
        commitment_properties
            .get("native_amx_receipts")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("maxItems"))
            .and_then(Value::as_u64),
        Some(4_096)
    );
    assert_eq!(
        commitment_properties
            .get("nexus_fee_receipts")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NexusFeeReceipt")
    );
    assert_eq!(
        schemas
            .get("LaneSettlementCommitment")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("additionalProperties"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        schemas
            .get("LaneSettlementCommitment")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("required"))
            .and_then(Value::as_array)
            .is_some_and(|required| required
                .iter()
                .any(|field| field.as_str() == Some("lane_incarnation")))
    );
    let receipt_properties = schemas
        .get("NativeAmxReceipt")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("native AMX receipt properties");
    let receipt_required = schemas
        .get("NativeAmxReceipt")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .expect("native AMX receipt required fields");
    for field in [
        "network_id",
        "lane_incarnation",
        "authority_context_height",
        "lane_block_height",
        "lane_block_view",
        "coordinator_proposal_hash",
    ] {
        assert!(
            receipt_required
                .iter()
                .any(|required| required.as_str() == Some(field)),
            "native AMX receipt must require {field}"
        );
    }
    assert_eq!(
        receipt_properties
            .get("legs")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("items"))
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NativeAmxLegRecord")
    );
    let legs_schema = receipt_properties
        .get("legs")
        .and_then(Value::as_object)
        .expect("native AMX legs schema");
    assert_eq!(legs_schema.get("minItems").and_then(Value::as_u64), Some(1));
    assert_eq!(
        legs_schema.get("maxItems").and_then(Value::as_u64),
        Some(255)
    );
    assert_eq!(
        legs_schema.get("uniqueItems").and_then(Value::as_bool),
        Some(true)
    );
    let leg_properties = schemas
        .get("NativeAmxLegRecord")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("native AMX leg properties");
    assert!(
        !leg_properties.contains_key("lane_incarnation"),
        "native AMX v2 legs bind participant context through both QCs, not a duplicated top-level incarnation"
    );
    assert_eq!(
        component_required(schemas, "NativeAmxLegRecord"),
        [
            "lane_id",
            "dataspace_id",
            "participant_proposal",
            "participant_settlement",
            "participant_settlement_hash",
            "prepare_qc",
            "commit_qc",
        ]
    );
    for (field, schema_ref) in [
        (
            "participant_proposal",
            "#/components/schemas/NativeAmxParticipantLaneBlockProposal",
        ),
        (
            "participant_settlement",
            "#/components/schemas/NativeAmxParticipantSettlementCommitment",
        ),
        ("participant_settlement_hash", "#/components/schemas/Hash"),
    ] {
        assert_eq!(
            leg_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(schema_ref),
            "{field} should reference its exact participant-finality schema"
        );
    }
    let participant_settlement = schemas
        .get("NativeAmxParticipantSettlementCommitment")
        .and_then(Value::as_object)
        .expect("Native AMX participant settlement schema");
    assert_eq!(
        participant_settlement
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false)
    );
    let participant_settlement_properties = participant_settlement
        .get("properties")
        .and_then(Value::as_object)
        .expect("Native AMX participant settlement properties");
    for field in [
        "total_local_amount",
        "total_xor_due",
        "total_xor_after_haircut",
        "total_xor_variance",
    ] {
        assert_eq!(
            participant_settlement_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some("0"),
            "participant settlement {field} must be zero"
        );
    }
    for field in ["nexus_fee_receipts", "native_amx_receipts"] {
        assert_eq!(
            participant_settlement_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maxItems"))
                .and_then(Value::as_u64),
            Some(0),
            "participant settlement {field} must be empty"
        );
    }
    let participant_receipts = participant_settlement_properties
        .get("receipts")
        .and_then(Value::as_object)
        .expect("Native AMX participant receipts schema");
    assert_eq!(
        participant_receipts.get("minItems").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        participant_receipts.get("maxItems").and_then(Value::as_u64),
        Some(4_096)
    );
    assert_eq!(
        participant_receipts
            .get("uniqueItems")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        participant_receipts
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NativeAmxParticipantSettlementReceipt")
    );
    for qc_field in ["prepare_qc", "commit_qc"] {
        assert_eq!(
            leg_properties
                .get(qc_field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/NativeAmxAttestationQc"),
            "{qc_field} should reference the QC schema"
        );
    }
    let qc_properties = schemas
        .get("NativeAmxAttestationQc")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("native AMX QC properties");
    assert_eq!(
        qc_properties
            .get("body")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NativeAmxAttestationBody")
    );
    let validator_set = qc_properties
        .get("validator_set")
        .and_then(Value::as_object)
        .expect("native AMX validator set schema");
    assert_eq!(
        validator_set.get("minItems").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        validator_set.get("maxItems").and_then(Value::as_u64),
        Some(128)
    );
    assert_eq!(
        validator_set.get("uniqueItems").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        validator_set
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2BlsValidatorId")
    );
    let validator_set_pops = qc_properties
        .get("validator_set_pops")
        .and_then(Value::as_object)
        .expect("native AMX validator proof-of-possession schema");
    assert_eq!(
        validator_set_pops.get("minItems").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        validator_set_pops.get("maxItems").and_then(Value::as_u64),
        Some(128)
    );
    assert_eq!(
        validator_set_pops
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2BlsProof")
    );
    assert_eq!(
        qc_properties
            .get("signers_bitmap")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("minItems"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        qc_properties
            .get("signers_bitmap")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("maxItems"))
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        qc_properties
            .get("bls_aggregate_signature")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiV2BlsProof")
    );
    let descriptor_properties = schemas
        .get("NativeAmxParticipantLaneBlockDescriptor")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("Native AMX participant descriptor properties");
    for field in ["accepted_candidate_indices", "accepted_transaction_hashes"] {
        let accepted = descriptor_properties
            .get(field)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("Native AMX descriptor {field} schema"));
        assert_eq!(accepted.get("minItems").and_then(Value::as_u64), Some(1));
        assert_eq!(
            accepted.get("maxItems").and_then(Value::as_u64),
            Some(4_096)
        );
        assert_eq!(
            accepted.get("uniqueItems").and_then(Value::as_bool),
            Some(true)
        );
    }
    let body_schema = schemas
        .get("NativeAmxAttestationBody")
        .and_then(Value::as_object)
        .expect("native AMX body schema");
    let body_required = body_schema
        .get("required")
        .and_then(Value::as_array)
        .expect("native AMX body required fields");
    for field in [
        "round",
        "epoch",
        "network_id",
        "coordinator_lane_incarnation",
        "participant_lane_incarnation",
        "participant_previous_block_height",
        "participant_previous_block_descriptor_hash",
        "participant_lane_block_height",
        "participant_lane_block_view",
        "participant_proposal_hash",
        "participant_settlement_commitment",
        "participant_validator_set_hash",
        "participant_validator_count",
        "participant_min_quorum",
        "authority_context_height",
        "coordinator_lane_block_view",
        "coordinator_proposal_hash",
        "planned_coordinator_block_height",
    ] {
        assert!(
            body_required
                .iter()
                .any(|required| required.as_str() == Some(field)),
            "native AMX body must require {field}"
        );
    }
    assert!(
        !body_required
            .iter()
            .any(|required| required.as_str() == Some("coordinator_lane_block_height"))
    );
    let body_properties = body_schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("native AMX body properties");
    for field in ["participant_validator_count", "participant_min_quorum"] {
        assert_eq!(
            body_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum"))
                .and_then(Value::as_u64),
            Some(128),
            "native AMX body must cap {field}"
        );
    }
    let body_phase = body_properties
        .get("phase")
        .and_then(Value::as_object)
        .expect("native AMX phase schema");
    assert_eq!(
        body_phase.get("$ref").and_then(Value::as_str),
        Some("#/components/schemas/NativeAmxPhase")
    );
    let native_phase = schemas
        .get("NativeAmxPhase")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("native AMX tagged phase properties");
    let phase_values = native_phase
        .get("phase")
        .and_then(Value::as_object)
        .and_then(|phase| phase.get("enum"))
        .and_then(Value::as_array)
        .expect("native AMX phase enum");
    assert!(
        phase_values
            .iter()
            .any(|value| value.as_str() == Some("prepare"))
    );
    assert!(
        phase_values
            .iter()
            .any(|value| value.as_str() == Some("commit"))
    );
    assert_eq!(
        native_phase
            .get("detail")
            .and_then(Value::as_object)
            .and_then(|detail| detail.get("type"))
            .and_then(Value::as_str),
        Some("null")
    );
    for (schema_name, tag, values) in [
        (
            "LaneLiquidityProfile",
            "profile",
            &["Tier1", "Tier2", "Tier3"][..],
        ),
        (
            "LaneVolatilityClass",
            "bucket",
            &["Stable", "Elevated", "Dislocated"][..],
        ),
    ] {
        let properties = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{schema_name} tagged enum properties"));
        let actual = properties
            .get(tag)
            .and_then(Value::as_object)
            .and_then(|tag| tag.get("enum"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{schema_name}.{tag} enum"));
        for expected in values {
            assert!(
                actual.iter().any(|value| value.as_str() == Some(*expected)),
                "{schema_name}.{tag} must include {expected}"
            );
        }
    }
    assert_eq!(
        schemas
            .get("LaneSwapMetadata")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("liquidity_profile"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/LaneLiquidityProfile")
    );
    assert_eq!(
        schemas
            .get("LaneSwapMetadata")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("volatility_class"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/LaneVolatilityClass")
    );
    for field in [
        "total_local_amount",
        "total_xor_due",
        "total_xor_after_haircut",
        "total_xor_variance",
    ] {
        let property = commitment_properties
            .get(field)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("lane settlement {field} schema"));
        assert_eq!(
            property.get("$ref").and_then(Value::as_str),
            Some("#/components/schemas/Quantity")
        );
    }
    for retired in [
        "total_local_micro",
        "total_xor_due_micro",
        "total_xor_after_haircut_micro",
        "total_xor_variance_micro",
    ] {
        assert!(
            !commitment_properties.contains_key(retired),
            "retired fixed-unit field leaked into the settlement schema: {retired}"
        );
    }
    let settlement_receipt_properties = schemas
        .get("LaneSettlementReceipt")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("lane settlement receipt properties");
    assert_eq!(
        settlement_receipt_properties
            .get("source_id")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("pattern"))
            .and_then(Value::as_str),
        Some("^[0-9A-F]{64}$")
    );
    for field in [
        "local_amount",
        "xor_due",
        "xor_after_haircut",
        "xor_variance",
    ] {
        assert_eq!(
            settlement_receipt_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/Quantity"),
            "lane settlement receipt {field} must use the canonical Quantity schema"
        );
    }
    for retired in [
        "local_amount_micro",
        "xor_due_micro",
        "xor_after_haircut_micro",
        "xor_variance_micro",
    ] {
        assert!(
            !settlement_receipt_properties.contains_key(retired),
            "retired fixed-unit field leaked into the settlement receipt schema: {retired}"
        );
    }
    for (schema_name, field_name) in [
        ("NexusFeeReceipt", "lane_id"),
        ("NativeAmxAttestationBody", "coordinator_lane_id"),
        ("NativeAmxAttestationBody", "participant_lane_id"),
        ("NativeAmxLegRecord", "lane_id"),
        ("NativeAmxReceipt", "lane_id"),
        ("LaneSettlementCommitment", "lane_id"),
        ("LaneRelayEnvelope", "lane_id"),
    ] {
        let maximum = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field_name))
            .and_then(Value::as_object)
            .and_then(|property| property.get("maximum"))
            .and_then(Value::as_u64);
        assert_eq!(
            maximum,
            Some(u64::from(u32::MAX)),
            "{schema_name}.{field_name} must retain an unsigned uint32 maximum"
        );
    }
}
#[test]
fn generated_spec_documents_soracloud_private_uploaded_model_routes() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    assert!(paths.contains_key("/v1/soracloud/status"));
    assert!(paths.contains_key("/v1/soracloud/hf/deploy"));
    assert!(paths.contains_key("/v1/soracloud/model/upload/private/execute"));
    assert!(paths.contains_key("/v1/soracloud/model/upload/private/receipts"));
    let status_description = paths
        .get("/v1/soracloud/status")
        .and_then(Value::as_object)
        .and_then(|path| path.get("get"))
        .and_then(Value::as_object)
        .and_then(|get| get.get("description"))
        .and_then(Value::as_str)
        .expect("Soracloud status description");
    assert!(status_description.contains("configured_lane_count"));
    assert!(status_description.contains("declared_lane_count"));
    assert!(status_description.contains("active lane ids/count"));
    assert!(status_description.contains("autoscale-capacity lane ids/count"));
    let hf_deploy = paths
        .get("/v1/soracloud/hf/deploy")
        .and_then(Value::as_object)
        .and_then(|path| path.get("post"))
        .and_then(Value::as_object)
        .expect("Soracloud HF deploy POST operation");
    assert_eq!(
        hf_deploy
            .get(SORACLOUD_HF_DEPLOY_CONTRACT_EXTENSION)
            .and_then(Value::as_str),
        Some(SORACLOUD_HF_DEPLOY_CONTRACT_V1)
    );
    assert_eq!(
        hf_deploy
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SoracloudHfDeployDraftV1")
    );
    let documented_headers = hf_deploy
        .get("parameters")
        .and_then(Value::as_array)
        .expect("Soracloud HF deploy canonical request headers");
    for header in [
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ] {
        assert!(
            documented_headers.iter().any(|parameter| {
                parameter
                    .as_object()
                    .and_then(|parameter| parameter.get("name"))
                    .and_then(Value::as_str)
                    == Some(header)
            }),
            "Soracloud HF deploy header missing {header}"
        );
    }
    let schemas = doc
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("schema section");
    let hf_draft = schemas
        .get("SoracloudHfDeployDraftV1")
        .and_then(Value::as_object)
        .expect("Soracloud HF deploy draft schema");
    assert_eq!(
        hf_draft
            .get(SORACLOUD_HF_DEPLOY_CONTRACT_EXTENSION)
            .and_then(Value::as_str),
        Some(SORACLOUD_HF_DEPLOY_CONTRACT_V1)
    );
    let tx_instructions = hf_draft
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("tx_instructions"))
        .and_then(Value::as_object)
        .expect("Soracloud HF deploy draft tx_instructions schema");
    assert_eq!(
        tx_instructions.get("minItems").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        tx_instructions.get("maxItems").and_then(Value::as_u64),
        Some(3)
    );
    let payload_hex = schemas
        .get("SoracloudTxInstruction")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("payload_hex"))
        .and_then(Value::as_object)
        .expect("Soracloud transaction instruction payload_hex schema");
    assert_eq!(
        payload_hex.get("pattern").and_then(Value::as_str),
        Some("^(?:[0-9a-f]{2})+$")
    );
    let response = schemas
        .get("PrivateUploadedModelReceiptListResponse")
        .and_then(Value::as_object)
        .expect("receipt-list schema");
    let properties = response
        .get("properties")
        .and_then(Value::as_object)
        .expect("receipt-list properties");
    for key in [
        "total",
        "returned_items",
        "remaining_items",
        "has_more",
        "count_mode",
        "continue_cursor",
    ] {
        assert!(properties.contains_key(key), "metadata field missing {key}");
    }
}
#[test]
fn generated_spec_documents_app_query_page_metadata() {
    let doc = generate_spec();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths section");
    for (path, method, expected_ref) in [
        (
            "/v1/accounts",
            "get",
            "#/components/schemas/AccountListResponse",
        ),
        (
            "/v1/accounts/query",
            "post",
            "#/components/schemas/AccountQueryResponse",
        ),
        (
            "/v1/domains",
            "get",
            "#/components/schemas/DomainListResponse",
        ),
        (
            "/v1/domains/query",
            "post",
            "#/components/schemas/DomainQueryResponse",
        ),
        (
            "/v1/accounts/{account_id}/assets",
            "get",
            "#/components/schemas/AccountAssetListResponse",
        ),
        (
            "/v1/accounts/{account_id}/assets/query",
            "post",
            "#/components/schemas/AccountAssetQueryResponse",
        ),
        (
            "/v1/assets/definitions",
            "get",
            "#/components/schemas/AssetDefinitionListResponse",
        ),
        (
            "/v1/assets/definitions/query",
            "post",
            "#/components/schemas/AssetDefinitionQueryResponse",
        ),
        (
            "/v1/assets/{definition_id}/holders",
            "get",
            "#/components/schemas/AssetHolderListResponse",
        ),
        (
            "/v1/assets/{definition_id}/holders/query",
            "post",
            "#/components/schemas/AssetHolderQueryResponse",
        ),
        ("/v1/nfts", "get", "#/components/schemas/NftListResponse"),
        (
            "/v1/nfts/query",
            "post",
            "#/components/schemas/NftQueryResponse",
        ),
        ("/v1/rwas", "get", "#/components/schemas/RwaListResponse"),
        (
            "/v1/rwas/query",
            "post",
            "#/components/schemas/RwaQueryResponse",
        ),
        (
            "/v1/repo/agreements",
            "get",
            "#/components/schemas/RepoAgreementListResponse",
        ),
        (
            "/v1/repo/agreements/query",
            "post",
            "#/components/schemas/RepoAgreementListResponse",
        ),
    ] {
        let schema_ref = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|path_item| path_item.get(method))
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("responses"))
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .expect("path response schema ref");
        assert_eq!(schema_ref, expected_ref, "{method} {path}");
    }
    let schemas = doc
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("schema section");
    let metadata = schemas
        .get("AppPageMetadata")
        .and_then(Value::as_object)
        .expect("app page metadata schema");
    let required = metadata
        .get("required")
        .and_then(Value::as_array)
        .expect("metadata required fields");
    for field in ["items", "has_more", "count_mode"] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "metadata required fields should include {field}"
        );
    }
    assert!(
        !required.iter().any(|value| value.as_str() == Some("total")),
        "bounded count responses must not require total"
    );
    let properties = metadata
        .get("properties")
        .and_then(Value::as_object)
        .expect("metadata properties");
    for field in [
        "total",
        "has_more",
        "count_mode",
        "indexed_height",
        "indexed_block_hash",
        "query_source",
    ] {
        assert!(
            properties.contains_key(field),
            "metadata properties should include {field}"
        );
    }
    let repo_agreement = schemas
        .get("RepoAgreement")
        .and_then(Value::as_object)
        .expect("repo agreement schema");
    let repo_required = repo_agreement
        .get("required")
        .and_then(Value::as_array)
        .expect("repo agreement required fields");
    let repo_properties = repo_agreement
        .get("properties")
        .and_then(Value::as_object)
        .expect("repo agreement properties");
    for field in [
        "cash_source",
        "collateral_custody_asset",
        "settlement_timestamp_ms",
        "status",
    ] {
        assert!(
            repo_required
                .iter()
                .any(|value| value.as_str() == Some(field)),
            "repo agreement should require {field}"
        );
        assert!(
            repo_properties.contains_key(field),
            "repo agreement should document {field}"
        );
    }
    let repo_request_properties = schemas
        .get("RepoAgreementsQueryRequest")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("repo query request properties");
    for field in ["filter", "select", "aggregate", "fetch_size", "count_mode"] {
        assert!(
            repo_request_properties.contains_key(field),
            "repo query request should document {field}"
        );
    }
}
#[test]
fn alias_openapi_documents_optional_public_and_exact_restricted_auth() {
    let document = generate_spec();
    let canonical_headers = vec![
        ("X-Iroha-Account".to_owned(), false),
        ("X-Iroha-Signature".to_owned(), false),
        ("X-Iroha-Timestamp-Ms".to_owned(), false),
        ("X-Iroha-Nonce".to_owned(), false),
        ("X-Iroha-Witness".to_owned(), false),
    ];
    for path in ["/v1/aliases/resolve-index", "/v1/aliases/by-account"] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(
            operation_header_requirements(operation),
            canonical_headers,
            "POST {path} canonical authentication headers"
        );
        assert_alias_auth_required_response(operation, path);
        assert!(
            operation
                .get("responses")
                .and_then(Value::as_object)
                .is_some_and(|responses| responses.contains_key("403")),
            "POST {path} must distinguish authorization failure from missing authentication"
        );
    }
    let lookup_description = openapi_operation(&document, "/v1/aliases/by-account", "post")
        .get("description")
        .and_then(Value::as_str)
        .expect("alias by-account description");
    assert!(lookup_description.contains("Canonical authentication is optional"));
    assert!(lookup_description.contains("required for restricted data"));
    let exact_resolve = openapi_operation(&document, "/v1/aliases/resolve", "post");
    assert_eq!(
        operation_header_requirements(exact_resolve),
        canonical_headers,
        "POST /v1/aliases/resolve canonical authentication headers"
    );
    assert_alias_auth_required_response(exact_resolve, "/v1/aliases/resolve");
    assert!(
        exact_resolve
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(
                |description| description.contains("Public dataspaces may be read unsigned")
            )
    );
    let setup_plan = openapi_operation(&document, "/v1/aliases/setup/plan", "post");
    assert_eq!(
        operation_header_requirements(setup_plan),
        canonical_headers,
        "POST /v1/aliases/setup/plan canonical authentication headers"
    );
    assert_alias_auth_required_response(setup_plan, "/v1/aliases/setup/plan");
    assert!(
        setup_plan
            .get("responses")
            .and_then(Value::as_object)
            .is_some_and(|responses| responses.contains_key("409")),
        "planner must document structured drift conflicts"
    );
    for path in [
        "/v1/aliases/lease/renew/plan",
        "/v1/aliases/auto-renew/plan",
    ] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(
            operation_header_requirements(operation),
            canonical_headers,
            "POST {path} canonical authentication headers"
        );
        assert_alias_auth_required_response(operation, path);
        assert!(
            operation
                .get("responses")
                .and_then(Value::as_object)
                .is_some_and(|responses| responses.contains_key("409")),
            "POST {path} must document live CAS/owner/quote conflicts"
        );
    }
    assert!(
        exact_resolve
            .get("responses")
            .and_then(Value::as_object)
            .is_some_and(|responses| responses.contains_key("403")),
        "the exact alias resolve route must distinguish permission failure from missing authentication"
    );
    for (path, reject_code, requires_authorization_response) in [
        (
            "/v1/retail/recipients/lookup",
            "recipient_lookup_signature_required",
            true,
        ),
        (
            "/v1/retail/recipients/route",
            "recipient_lookup_signature_required",
            true,
        ),
        (
            "/v1/fee-sponsor-programs/by-id",
            "fee_sponsor_program_signature_required",
            false,
        ),
        ("/v1/fees/quote", "fee_quote_signature_required", false),
    ] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(
            operation_header_requirements(operation),
            canonical_headers,
            "POST {path} canonical authentication headers"
        );
        assert_canonical_auth_required_response(operation, path, reject_code);
        assert_eq!(
            operation
                .get("responses")
                .and_then(Value::as_object)
                .is_some_and(|responses| responses.contains_key("403")),
            requires_authorization_response,
            "POST {path} authorization response contract"
        );
    }
}
#[test]
fn protected_contract_identity_openapi_is_signed_and_exact() {
    let document = generate_spec();
    let canonical_headers = vec![
        ("X-Iroha-Account".to_owned(), false),
        ("X-Iroha-Signature".to_owned(), false),
        ("X-Iroha-Timestamp-Ms".to_owned(), false),
        ("X-Iroha-Nonce".to_owned(), false),
        ("X-Iroha-Witness".to_owned(), false),
    ];
    for (path, method, response_schema, reject_code, expected_statuses) in [
        (
            "/v1/contracts/aliases/resolve",
            "post",
            "#/components/schemas/ContractAliasResolveResponse",
            "alias_auth_required",
            &["200", "400", "401", "404", "429", "500"][..],
        ),
        (
            "/v1/gov/contracts/{contract_address}",
            "get",
            "#/components/schemas/GovernedContractResponse",
            "contract_code_auth_required",
            &["200", "400", "401", "404", "429", "500"][..],
        ),
        (
            "/v1/contracts/code-bytes/{code_hash}",
            "get",
            "#/components/schemas/JsonValue",
            "contract_code_auth_required",
            &["200", "400", "401", "404", "429"][..],
        ),
    ] {
        let operation = openapi_operation(&document, path, method);
        assert_eq!(
            operation_header_requirements(operation),
            canonical_headers,
            "{method} {path} canonical authentication headers"
        );
        assert_canonical_auth_required_response(operation, path, reject_code);
        assert_eq!(
            operation_response_schema_ref(operation, "200", path),
            response_schema,
            "{method} {path} success schema"
        );
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("operation responses");
        assert_eq!(
            responses
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected_statuses.iter().copied().collect(),
            "{method} {path} must document the exact fail-closed response surface"
        );
    }
    assert_eq!(
        operation_request_schema_ref(
            openapi_operation(&document, "/v1/contracts/aliases/resolve", "post"),
            "/v1/contracts/aliases/resolve",
        ),
        "#/components/schemas/ContractAliasResolveRequest"
    );
    let schemas = document
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("schema section");
    for (name, required_fields) in [
        ("ContractAliasResolveRequest", vec!["contract_alias"]),
        (
            "ContractAliasBinding",
            vec!["alias", "status", "bound_at_ms"],
        ),
        (
            "ContractAliasResolveResponse",
            vec![
                "contract_alias",
                "contract_address",
                "contract_subject_account",
                "dataspace",
                "contract_alias_binding",
                "source",
            ],
        ),
    ] {
        let schema = schemas
            .get(name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{name} schema"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required fields")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            required,
            required_fields.into_iter().collect(),
            "{name} required fields"
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("schema properties");
        assert_eq!(
            properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            required
                .iter()
                .copied()
                .chain(match name {
                    "ContractAliasBinding" => {
                        ["lease_expiry_ms", "grace_until_ms"]
                            .as_slice()
                            .iter()
                            .copied()
                    }
                    _ => [].as_slice().iter().copied(),
                })
                .collect(),
            "{name} must not advertise undeclared compatibility fields"
        );
    }
    let governed = schemas
        .get("GovernedContractResponse")
        .and_then(Value::as_object)
        .expect("governed contract schema");
    let variants = governed
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("governed response variants");
    assert_eq!(variants.len(), 2);
    for (variant, expected) in variants.iter().zip([
        [
            "found",
            "contract_address",
            "contract_subject_account",
            "dataspace",
            "code_hash_hex",
            "abi_hash_hex",
            "public_entrypoints",
        ]
        .as_slice(),
        ["found", "contract_address", "dataspace"].as_slice(),
    ]) {
        let variant = variant.as_object().expect("governed response variant");
        assert_eq!(
            variant.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let required = variant
            .get("required")
            .and_then(Value::as_array)
            .expect("variant required fields")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        let properties = variant
            .get("properties")
            .and_then(Value::as_object)
            .expect("variant properties")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(required, expected);
        assert_eq!(properties, expected);
    }
}
#[test]
fn multisig_read_auth_contract_is_path_specific() {
    let document = generate_spec();
    let canonical_headers = vec![
        ("X-Iroha-Account".to_owned(), false),
        ("X-Iroha-Signature".to_owned(), false),
        ("X-Iroha-Timestamp-Ms".to_owned(), false),
        ("X-Iroha-Nonce".to_owned(), false),
        ("X-Iroha-Witness".to_owned(), false),
    ];
    for path in [
        "/v1/multisig/spec",
        "/v1/multisig/proposals/query",
        "/v1/multisig/proposals/resolve",
    ] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(
            operation_header_requirements(operation),
            canonical_headers,
            "POST {path} conditional canonical authentication headers"
        );
        assert_canonical_auth_required_response(operation, path, "multisig_read_auth_required");
        let description = operation
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("POST {path} description"));
        assert!(description.contains("Both canonical `multisig_account_id`"));
        assert!(description.contains("`multisig_account_alias` selectors require"));
        assert!(description.contains("body fields never establish signer identity"));
    }
    for path in [
        "/v1/multisig/propose",
        "/v1/multisig/approve",
        "/v1/multisig/cancel",
        "/v1/contracts/call/multisig/propose",
        "/v1/contracts/call/multisig/approve",
    ] {
        let operation = openapi_operation(&document, path, "post");
        assert!(
            operation_header_requirements(operation).is_empty(),
            "POST {path} must retain its body-authenticated write contract"
        );
    }
}
