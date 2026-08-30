#[rustfmt::skip]
mod compact_finality_app_contracts {
use super::*;

fn schema_fields<'a>(schema: &'a Map, key: &str, context: &str) -> &'a Vec<Value> {
    schema.get(key).and_then(Value::as_array).unwrap_or_else(|| panic!("{context}.{key}"))
}

fn assert_array_bounds(schema: &Map, minimum: u64, maximum: u64, unique: Option<bool>) {
    assert_eq!(schema.get("minItems").and_then(Value::as_u64), Some(minimum));
    assert_eq!(schema.get("maxItems").and_then(Value::as_u64), Some(maximum));
    if let Some(unique) = unique {
        assert_eq!(schema.get("uniqueItems").and_then(Value::as_bool), Some(unique));
    }
}

fn assert_item_ref(schema: &Map, expected: &str, context: &str) {
    assert_eq!(schema.get("items").and_then(Value::as_object).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some(expected), "{context} item reference");
}

fn object_field_set(object: &Map) -> BTreeSet<&str> {
    object.keys().map(String::as_str).collect()
}

fn schema_string_field_set<'a>(schema: &'a Map, key: &str, context: &str) -> BTreeSet<&'a str> {
    schema_fields(schema, key, context).iter().map(|value| value.as_str().unwrap_or_else(|| panic!("{context}.{key} contains a non-string value"))).collect()
}

fn assert_exact_closed_required_schema_fields(schemas: &Map, name: &str, expected: &[&str]) {
    let schema = contract_schema(schemas, name);
    assert_eq!(schema.get("additionalProperties").and_then(Value::as_bool), Some(false), "{name} must reject unknown compatibility fields");
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(object_field_set(schema.get("properties").and_then(Value::as_object).unwrap_or_else(|| panic!("{name}.properties"))), expected, "{name} property surface");
    assert_eq!(schema_string_field_set(schema, "required", name), expected, "{name} required-field surface");
}

fn asset_field_set(inventory: &str) -> BTreeSet<&'static str> {
    contract_strings(inventory).into_iter().collect()
}

#[test]
fn sumeragi_v2_da_schema_requires_reed_solomon16_without_plain_compatibility() {
    let schemas = openapi_schemas();
    let encoding = contract_schema(&schemas, "SumeragiV2PayloadEncoding");
    let allowed = contract_property(&schemas, "SumeragiV2PayloadEncoding", "encoding").get("enum").and_then(Value::as_array).expect("payload encoding enum");
    assert_eq!(allowed, &[Value::from("reed_solomon16")]);
    assert!(!allowed.iter().any(|value| value.as_str() == Some("plain")), "retired Plain encoding");
    let layout = contract_schema(&schemas, "SumeragiV2DataAvailabilityLayout");
    assert!(layout.get("oneOf").is_none() && layout.get("anyOf").is_none(), "RS16 layout compatibility union");
    assert_required_inventory(layout, "sumeragi.da.required", "Sumeragi v2 DA layout");
    assert_eq!(contract_property(&schemas, "SumeragiV2DataAvailabilityLayout", "encoding").get("$ref").and_then(Value::as_str), Some("#/components/schemas/SumeragiV2PayloadEncoding"));
    for field in ["data_shards", "parity_shards"] {
        assert_eq!(contract_property(&schemas, "SumeragiV2DataAvailabilityLayout", field).get("minimum").and_then(Value::as_u64), Some(1), "positive {field}");
    }
    let serialized = norito::json::to_string(&Value::Object(layout.clone())).expect("serialize DA layout");
    assert!(!serialized.contains("\"plain\""), "retired Plain layout branch");
    assert!(encoding.contains_key("properties"));
}

#[test]
fn inrou_guest_image_schema_requires_one_concrete_published_artifact() {
    let schemas = openapi_schemas();
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouGuestImageV1", &["initrd_image_path", "kernel_image_path", "published_artifact", "rootfs_image_path"]);
    let published_artifact = contract_property(&schemas, "SoraInrouGuestImageV1", "published_artifact");
    assert_eq!(published_artifact.get("$ref").and_then(Value::as_str), Some("#/components/schemas/SoraPublishedInrouGuestImageArtifactV1"));
    assert!(published_artifact.get("anyOf").is_none() && published_artifact.get("oneOf").is_none() && published_artifact.get("nullable").is_none(), "admitted Inrou V1 artifacts must not expose a missing/null compatibility branch");
    assert_exact_closed_required_schema_fields(&schemas, "SoraPublishedInrouGuestImageArtifactV1", &["content_cid", "manifest_digest_hex"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouManifestV1", &["guest_images", "schema_version"]);
    for retired_schema in ["SoraArtifactDistributionPolicyV1", "SoraArtifactDistributionTargetV1", "SoraInrouGuestOsV1"] {
        assert!(!schemas.contains_key(retired_schema), "retired schema `{retired_schema}` must be absent");
    }
    for (schema_name, retired_fields) in [("SoraCapabilityPolicyV1", &["allow_wallet_signing"][..]), ("SoraInrouHostCapabilityRecordV1", &["geography_tags", "observed_latency_ms"][..]), ("SoraInrouReplicaPlacementV1", &["selected_geography_tag", "selection_latency_ms"][..])] {
        let properties = contract_schema(&schemas, schema_name).get("properties").and_then(Value::as_object).unwrap_or_else(|| panic!("{schema_name} properties"));
        for retired_field in retired_fields {
            assert!(!properties.contains_key(*retired_field), "{schema_name} must not expose retired field `{retired_field}`");
        }
    }
}

#[test]
fn inrou_first_release_openapi_matches_block_clock_and_exact_admission() {
    let schemas = openapi_schemas();

    assert_exact_closed_required_schema_fields(&schemas, "SoraHttpServiceEconomicsV1", &["schema_version", "quota_class", "deployment_deposit", "prepaid_runtime_balance", "lease_duration_blocks", "runtime_price_per_block", "storage_price_per_gib_block", "egress_price_per_mib"]);
    let economics = contract_schema(&schemas, "SoraHttpServiceEconomicsV1");
    let economics_json = norito::json::to_string(&Value::Object(economics.clone())).expect("serialize economics");
    for retired in ["lease_duration_sequences", "runtime_price_per_sequence", "storage_price_per_gib_sequence"] {
        assert!(!economics_json.contains(retired), "retired sequence-clock economics field `{retired}`");
    }

    let guest_images = contract_property(&schemas, "SoraInrouManifestV1", "guest_images");
    assert_eq!(guest_images.get("minProperties").and_then(Value::as_u64), Some(1));
    assert_eq!(guest_images.get("maxProperties").and_then(Value::as_u64), Some(2));
    assert_eq!(guest_images.get("additionalProperties").and_then(Value::as_object).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/SoraInrouGuestImageV1"));
    let admitted_guest_isas = guest_images.get("propertyNames").and_then(Value::as_object).and_then(|names| names.get("enum")).and_then(Value::as_array).expect("Inrou guest-image property-name enum").iter().map(|value| value.as_str().expect("guest ISA name")).collect::<BTreeSet<_>>();
    assert_eq!(admitted_guest_isas, BTreeSet::from(["aarch64", "x86_64"]));

    assert_eq!(contract_property(&schemas, "SoraInrouReplicaPlacementV1", "lease_started_height").get("minimum").and_then(Value::as_u64), Some(1));
    for schema_name in [
        "SoraInrouPlacementTargetV1",
        "SoraInrouReplicaPlacementV1",
    ] {
        for field in ["validator_account_id", "peer_id"] {
            assert_eq!(
                contract_property(&schemas, schema_name, field)
                    .get("minLength")
                    .and_then(Value::as_u64),
                Some(1),
                "{schema_name}.{field} must reject an empty identity",
            );
        }
    }

    let exact_resource_bounds = [
        ("cpu_millis", u64::from(iroha_data_model::soracloud::SORA_INROU_MIN_CPU_MILLIS_V1), Some(u64::from(iroha_data_model::soracloud::SORA_INROU_CPU_MILLIS_ALIGNMENT_V1))),
        ("memory_bytes", iroha_data_model::soracloud::SORA_INROU_MIN_MEMORY_BYTES_V1, Some(iroha_data_model::soracloud::SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1)),
        ("ephemeral_storage_bytes", iroha_data_model::soracloud::SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1, Some(iroha_data_model::soracloud::SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1)),
        ("max_open_files_per_process", u64::from(iroha_data_model::soracloud::SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1), None),
    ];
    for (field, minimum, multiple_of) in exact_resource_bounds {
        let property = contract_property(&schemas, "SoraResourceLimitsV1", field);
        assert_eq!(property.get("minimum").and_then(Value::as_u64), Some(minimum), "SoraResourceLimitsV1.{field} minimum");
        assert_eq!(property.get("multipleOf").and_then(Value::as_u64), multiple_of, "SoraResourceLimitsV1.{field} enforcement unit");
    }

    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouHostCapabilityRecordV1", &["schema_version", "validator_account_id", "peer_id", "supported_guest_isas", "trusted_guest_artifact", "max_hosted_replica_capacity", "max_cpu_millis", "max_memory_bytes", "max_storage_bytes", "advertised_at_ms", "heartbeat_expires_at_ms"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouPlacementTargetV1", &["validator_account_id", "peer_id"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouReplicaPlacementV1", &["replica_slot", "economic_clock", "lease_started_height", "placement_incarnation", "host_availability", "validator_account_id", "peer_id", "selected_guest_isa"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeInrouPlan", &["selected_guest_isa", "kernel_image_path", "rootfs_image_path", "initrd_image_path", "root_volume_name"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeLeaseVolumePlan", &["volume_name", "kind", "storage_class", "mount_path", "max_total_bytes", "lease_started_height", "lease_expires_height", "authoritative_generation", "local_materialization_dir"]);
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeReplicaPlan", &["replica_slot", "lease_started_height", "placement_incarnation", "host_availability", "validator_account_id", "peer_id", "materialization_dir", "health_status", "listen_base_url", "pid", "last_error"]);
    assert_exact_closed_required_schema_fields(
        &schemas,
        "SoracloudPublicServiceDiscoveryV1",
        &["schema_version", "service_name", "service_version", "execution_plane", "runtime", "route_host", "path_prefix", "base_url", "healthcheck_path", "healthcheck_url", "service_manifest_hash", "container_manifest_hash", "deployment_bundle_hash", "document_hash", "content_cid", "public_discovery_url", "public_discovery_cid_host_url", "manifest_digest_hex"],
    );

    let runtime_service = contract_schema(&schemas, "SoracloudRuntimeServicePlan").get("properties").and_then(Value::as_object).expect("SoracloudRuntimeServicePlan properties");
    assert!(runtime_service.contains_key("lease_expires_height"));
    for retired in ["lease_expires_sequence", "supports_private_secret_payload_reads"] {
        assert!(!runtime_service.contains_key(retired), "runtime service plan retained `{retired}`");
    }

    let control_plane_service = contract_schema(&schemas, "ControlPlaneServiceSnapshot");
    let control_plane_properties = control_plane_service.get("properties").and_then(Value::as_object).expect("ControlPlaneServiceSnapshot properties");
    let control_plane_required = schema_string_field_set(control_plane_service, "required", "ControlPlaneServiceSnapshot");
    assert!(control_plane_properties.contains_key("service_lease"));
    assert!(control_plane_required.contains("service_lease"));
    for retired in [
        "quota_class",
        "service_lease_status",
        "lease_expires_height",
        "lease_expires_sequence",
        "prepaid_runtime_balance",
        "remaining_runtime_balance",
    ] {
        assert!(
            !control_plane_properties.contains_key(retired),
            "ControlPlaneServiceSnapshot retained flattened lease field `{retired}`",
        );
        assert!(!control_plane_required.contains(retired));
    }
}

#[test]
fn bridge_finality_v2_schemas_are_exact_closed_and_bounded() {
    let schemas = openapi_schemas();
    let max_validators = u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound");
    assert_schema_shapes(
        &schemas,
        &[
            SchemaShape { name: "BridgeFinalityProof", required: "bridge.proof.required", optional: None },
            SchemaShape { name: "BridgeFinalityAttestationV1", required: "bridge.attestation.required", optional: None },
            SchemaShape { name: "SumeragiV2FinalityArtifact", required: "finality.artifact.required", optional: None },
            SchemaShape {
                name: "SumeragiV2HeightContext",
                required: "height.context.required",
                optional: None,
            },
            SchemaShape { name: "SumeragiV2ValidatorPower", required: "validator.power.required", optional: None },
            SchemaShape { name: "SumeragiV2DualQuorum", required: "dual.quorum.required", optional: None },
            SchemaShape { name: "SumeragiV2BlockSubject", required: "block.subject.required", optional: None },
            SchemaShape { name: "SumeragiV2MergeCarrierCommitment", required: "merge.carrier.required", optional: None },
            SchemaShape { name: "SumeragiV2ExecutionCommitment", required: "execution.required", optional: None },
            SchemaShape { name: "SumeragiV2QuorumCertificate", required: "qc.required", optional: None },
            SchemaShape { name: "SumeragiV2CommitQuorumCertificate", required: "qc.required", optional: None },
            SchemaShape { name: "SumeragiV2SnapshotBootstrapAnchor", required: "snapshot.bootstrap.required", optional: None },
            SchemaShape { name: "SumeragiV2FinalizedNextEpochSnapshot", required: "next.epoch.required", optional: None },
            SchemaShape { name: "BridgeCommitment", required: "bridge.commitment.required", optional: None },
            SchemaShape { name: "BridgeFinalityBundle", required: "bridge.bundle.required", optional: None },
            SchemaShape { name: "BlockHeader", required: "block.header.required", optional: None },
        ],
    );
    for field in contract_strings("height.context.nullable") {
        let _ = nullable_property_ref(&schemas, "SumeragiV2HeightContext", field);
    }
    for field in contract_strings("block.subject.nullable") {
        let _ = nullable_property_ref(&schemas, "SumeragiV2BlockSubject", field);
    }
    for field in contract_strings("execution.nullable") {
        let _ = nullable_property_ref(&schemas, "SumeragiV2ExecutionCommitment", field);
    }
    for field in contract_strings("block.header.nullable") {
        let _ = nullable_property_ref(&schemas, "BlockHeader", field);
    }
    assert_eq!(contract_property(&schemas, "BridgeFinalityProof", "version").get("enum").and_then(Value::as_array), Some(&vec![Value::from(2_u64)]));
    for (owner, field) in [("SumeragiV2FinalityArtifact", "format_version"), ("SumeragiV2FinalityArtifact", "protocol_version"), ("SumeragiV2HeightContext", "protocol_version")] {
        assert_eq!(contract_property(&schemas, owner, field).get("enum").and_then(Value::as_array), Some(&vec![Value::from(4_u64)]), "{owner}.{field} version");
    }
    let roster = contract_property(&schemas, "SumeragiV2HeightContext", "roster");
    assert_array_bounds(roster, 4, max_validators, Some(true));
    assert_item_ref(roster, "#/components/schemas/SumeragiV2ValidatorPower", "height-context roster");
    assert_eq!(contract_property(&schemas, "SumeragiV2ValidatorPower", "power").get("enum").and_then(Value::as_array), Some(&vec![Value::from(1_u64)]));
    assert_property_refs(
        &schemas,
        &[
            PropertyRefContract {
                owner: "SumeragiV2ValidatorPower",
                property: "validator",
                expected: "#/components/schemas/SumeragiV2BlsValidatorId",
            },
            PropertyRefContract { owner: "SumeragiV2MergeCarrierCommitment", property: "entry_hash", expected: "#/components/schemas/Hash" },
            PropertyRefContract {
                owner: "SumeragiV2FinalityArtifact",
                property: "commit_qc",
                expected: "#/components/schemas/SumeragiV2CommitQuorumCertificate",
            },
            PropertyRefContract {
                owner: "SumeragiV2QuorumCertificate",
                property: "execution_commitment",
                expected: "#/components/schemas/SumeragiV2ExecutionCommitment",
            },
            PropertyRefContract {
                owner: "SumeragiV2CommitQuorumCertificate",
                property: "execution_commitment",
                expected: "#/components/schemas/SumeragiV2ExecutionCommitment",
            },
        ],
    );
    assert_eq!(contract_schema(&schemas, "SumeragiV2BlsValidatorId").get("pattern").and_then(Value::as_str), Some("^ea0130[0-9A-F]{96}$"));
    for owner in ["SumeragiV2FinalityArtifact", "SumeragiV2FinalizedNextEpochSnapshot"] {
        let pops = contract_property(&schemas, owner, "validator_set_pops");
        assert_array_bounds(pops, 4, max_validators, None);
        assert_item_ref(pops, "#/components/schemas/SumeragiV2BlsProof", owner);
    }
    let proof = contract_schema(&schemas, "SumeragiV2BlsProof");
    assert_array_bounds(proof, 96, 96, None);
    let byte = proof.get("items").and_then(Value::as_object).expect("BLS proof byte");
    assert_eq!(byte.get("minimum").and_then(Value::as_u64), Some(0));
    assert_eq!(byte.get("maximum").and_then(Value::as_u64), Some(255));
    for owner in ["SumeragiV2QuorumCertificate", "SumeragiV2CommitQuorumCertificate"] {
        assert_array_bounds(contract_property(&schemas, owner, "signers"), 1, max_validators, Some(true));
    }
    assert_array_bounds(contract_property(&schemas, "SumeragiV2FinalizedNextEpochSnapshot", "roster"), 4, max_validators, None);
    assert_eq!(contract_property(&schemas, "SumeragiV2DualQuorum", "min_signers").get("maximum").and_then(Value::as_u64), Some(max_validators));
    assert_eq!(contract_property(&schemas, "SumeragiV2MergeCarrierCommitment", "version").get("const").and_then(Value::as_u64), Some(1));
    assert_eq!(contract_property(&schemas, "SumeragiV2ExecutionCommitment", "merge_carrier").get("oneOf").and_then(Value::as_array).map(Vec::len), Some(2));
    let wire_len = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "executed_block_wire_len");
    assert_eq!(wire_len.get("format").and_then(Value::as_str), Some("uint64"));
    assert_eq!(wire_len.get("minimum").and_then(Value::as_u64), Some(1));
    let topups = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "topup_anchor_count");
    assert_eq!(topups.get("minimum").and_then(Value::as_u64), Some(0));
    assert_eq!(topups.get("maximum").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK)));
    let execution = contract_schema(&schemas, "SumeragiV2ExecutionCommitment");
    assert_eq!(execution.get("oneOf").and_then(Value::as_array).map(Vec::len), Some(2));
    assert_eq!(contract_property(&schemas, "SumeragiV2ExecutionCommitment", "native_amx_application_manifest_version").get("const").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION)));
    let manifest_count = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "native_amx_application_manifest_count");
    assert_eq!(manifest_count.get("minimum").and_then(Value::as_u64), Some(0));
    assert_eq!(manifest_count.get("maximum").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES)));
    assert_exact_closed_required_schema_fields(
        &schemas,
        "SumeragiV2LaneFinalityManifestCommitment",
        &["root", "leaf_count"],
    );
    assert_eq!(property_ref(&schemas, "SumeragiV2LaneFinalityManifestCommitment", "root"), "#/components/schemas/Hash");
    let lane_leaf_count = contract_property(&schemas, "SumeragiV2LaneFinalityManifestCommitment", "leaf_count");
    assert_eq!(lane_leaf_count.get("minimum").and_then(Value::as_u64), Some(1));
    assert_eq!(lane_leaf_count.get("maximum").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::MAX_LANE_FINALITY_STATEMENTS_PER_BLOCK)));
    assert_eq!(execution.get("allOf").and_then(Value::as_array).and_then(|all| all.first()).and_then(Value::as_object).and_then(|contract| contract.get("oneOf")).and_then(Value::as_array).map(Vec::len), Some(2));
    assert_eq!(contract_property(&schemas, "SumeragiV2CommitPhase", "phase").get("enum").and_then(Value::as_array), Some(&vec![Value::from("commit")]));
    assert_array_bounds(contract_schema(&schemas, "SumeragiV2HeightContextId"), 1, 1, None);
    let mut components = Map::new();
    for name in contract_strings("bridge.components") {
        components.insert(name.to_owned(), schemas.get(name).unwrap_or_else(|| panic!("missing {name}")).clone());
    }
    let serialized = norito::json::to_string(&Value::Object(components)).expect("serialize bridge schemas");
    for retired in contract_strings("bridge.retired") {
        assert!(!serialized.contains(&format!("\"{retired}\"")), "retired bridge field `{retired}`");
    }
}

#[test]
fn bridge_finality_schema_matches_norito_json_and_decoder_rejects_v1_fields() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let proof = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof).expect("decode SCCP v2 fixture");
    let value = norito::json::to_value(&proof).expect("serialize finality proof");
    let proof_object = value.as_object().expect("proof object");
    assert_eq!(object_field_set(proof_object), asset_field_set("bridge.proof.required"));
    let header = proof_object.get("block_header").and_then(Value::as_object).expect("block header");
    for required in contract_strings("fixture.header.required") {
        assert!(header.contains_key(required), "serialized header omitted {required}");
    }
    assert!(
        header
            .get("npos_effects_hash")
            .and_then(Value::as_str)
            .is_some(),
        "the exact SCCP V1 fixture must carry the active NPoS effects commitment"
    );
    assert!(
        header
            .get("execution_context_hash")
            .is_some_and(Value::is_null),
        "the exact SCCP V1 fixture must carry the required nullable execution-context slot"
    );
    let expected_root = hex::encode_upper(fixture.bundle.commitment_root);
    assert_eq!(header.get("sccp_commitment_root").and_then(Value::as_str), Some(expected_root.as_str()));
    let document = generate_spec();
    let schemas = component_schemas(&document);
    let bytes32 = contract_schema(schemas, "SumeragiV2Bytes32");
    assert_eq!(bytes32.get("type").and_then(Value::as_str), Some("string"));
    assert_eq!(bytes32.get("minLength").and_then(Value::as_u64), Some(64));
    assert_eq!(bytes32.get("maxLength").and_then(Value::as_u64), Some(64));
    assert_eq!(bytes32.get("pattern").and_then(Value::as_str), Some("^[0-9A-F]{64}$"));
    let fixed = contract_schema(schemas, "SumeragiV2Fixed32ByteArray");
    assert_eq!(fixed.get("type").and_then(Value::as_str), Some("array"));
    assert_array_bounds(fixed, 32, 32, None);
    let artifact = proof_object.get("finality_artifact").and_then(Value::as_object).expect("artifact object");
    assert_eq!(object_field_set(artifact), asset_field_set("fixture.artifact.fields"));
    assert_eq!(artifact.get("format_version").and_then(Value::as_u64), Some(4));
    assert_eq!(artifact.get("protocol_version").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)));
    let context = artifact.get("height_context").and_then(Value::as_object).expect("height context");
    assert_eq!(context.get("protocol_version").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)));
    for nullable in contract_strings("height.context.nullable") {
        assert!(
            context.get(nullable).is_some_and(Value::is_null),
            "exact SCCP V1 fixture must carry null height-context slot `{nullable}`"
        );
    }
    let mode = context.get("mode").and_then(Value::as_object).expect("consensus mode");
    assert_eq!(mode.get("mode").and_then(Value::as_str), Some("npos"));
    assert_eq!(mode.get("details"), Some(&Value::Null));
    let encoding = context.get("da_layout").and_then(Value::as_object).and_then(|layout| layout.get("encoding")).and_then(Value::as_object).expect("payload encoding");
    assert_eq!(encoding.get("encoding").and_then(Value::as_str), Some("reed_solomon16"));
    assert_eq!(encoding.get("details"), Some(&Value::Null));
    let roster = context.get("roster").and_then(Value::as_array).expect("powered roster");
    assert_eq!(roster.len(), 4);
    for validator in roster {
        let validator = validator.as_object().expect("validator-power object");
        let key = validator.get("validator").and_then(Value::as_str).expect("validator id");
        assert_eq!(key.len(), 102);
        assert!(key.starts_with("ea0130"));
        assert!(key[6..].bytes().all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)));
        assert!(validator.get("power").and_then(Value::as_u64).is_some());
    }
    let quorum = context.get("quorum").and_then(Value::as_object).expect("dual quorum");
    assert_eq!(quorum.len(), 2);
    assert!(quorum.get("min_signers").and_then(Value::as_u64).is_some());
    assert!(quorum.get("total_power").and_then(Value::as_u64).is_some());
    let subject = artifact.get("subject").and_then(Value::as_object).expect("block subject");
    assert!(subject.get("parent_block_hash").is_some_and(Value::is_null));
    for field in ["block_hash", "payload_hash"] {
        let hash = subject.get(field).and_then(Value::as_str).expect("canonical hash");
        assert!(hash.starts_with("hash:") && hash.len() == 74);
    }
    let qc = artifact.get("commit_qc").and_then(Value::as_object).expect("commit QC");
    let phase = qc.get("phase").and_then(Value::as_object).expect("commit phase");
    assert_eq!(phase.get("phase").and_then(Value::as_str), Some("commit"));
    assert_eq!(phase.get("details"), Some(&Value::Null));
    assert!(qc.get("round").and_then(Value::as_object).and_then(|round| round.get("context_id")).and_then(Value::as_array).is_some_and(|id| id.len() == 1));
    assert_eq!(qc.get("proposal_round"), qc.get("round"));
    let execution = qc.get("execution_commitment").and_then(Value::as_object).expect("execution commitment");
    assert_eq!(object_field_set(execution), asset_field_set("fixture.execution.fields"));
    for root in ["parent_state_root", "post_state_root", "ordinary_writes_root", "native_amx_application_manifest_root"] {
        assert!(execution.get(root).and_then(Value::as_str).is_some_and(|hash| hash.starts_with("hash:") && hash.len() == 74), "canonical {root}");
    }
    for nullable in contract_strings("execution.nullable") {
        assert_eq!(
            execution.get(nullable),
            Some(&Value::Null),
            "exact SCCP V1 fixture must carry null execution slot `{nullable}`"
        );
    }
    assert!(execution.get("executed_block_wire_len").and_then(Value::as_u64).is_some_and(|length| length > 0));
    assert_eq!(execution.get("topup_anchor_count").and_then(Value::as_u64), Some(0));
    assert_eq!(execution.get("native_amx_application_manifest_version").and_then(Value::as_u64), Some(u64::from(iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION)));
    assert_eq!(execution.get("native_amx_application_manifest_count").and_then(Value::as_u64), Some(0));
    let empty_root = norito::json::to_value(&iroha_data_model::block::consensus_v2::native_amx_application_manifest_empty_root()).expect("serialize empty root");
    assert_eq!(execution.get("native_amx_application_manifest_root"), Some(&empty_root));
    assert!(qc.get("aggregate_signature").and_then(Value::as_array).is_some_and(|signature| signature.len() == 96));
    let pops = artifact.get("validator_set_pops").and_then(Value::as_array).expect("validator PoPs");
    assert_eq!(pops.len(), roster.len());
    assert!(pops.iter().all(|pop| pop.as_array().is_some_and(|bytes| { bytes.len() == 96 && bytes.iter().all(|byte| byte.as_u64().is_some_and(|byte| byte <= 255)) })));
    let mut missing = value.clone();
    let removed = missing.as_object_mut().and_then(|proof| proof.get_mut("finality_artifact")).and_then(Value::as_object_mut).and_then(|artifact| artifact.get_mut("commit_qc")).and_then(Value::as_object_mut).expect("mutable CommitQC").remove("execution_commitment");
    assert!(removed.is_some());
    assert!(norito::json::from_value::<iroha_data_model::bridge::BridgeFinalityProof>(missing).is_err(), "decoder accepted missing execution commitment");
    for retired in contract_strings("fixture.retired") {
        let mut hostile = value.clone();
        hostile.as_object_mut().expect("proof object").insert(retired.to_owned(), Value::Null);
        assert!(norito::json::from_value::<iroha_data_model::bridge::BridgeFinalityProof>(hostile).is_err(), "decoder accepted retired {retired}");
    }
}

#[test]
fn ledger_state_endpoints_expose_one_closed_authenticated_v2_schema() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    assert_schema_shapes(schemas, &[SchemaShape { name: "StateFinalityResponse", required: "ledger.state_finality.required", optional: None }]);
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract { owner: "StateFinalityResponse", property: "block_hash", expected: "#/components/schemas/Hash" },
            PropertyRefContract { owner: "StateFinalityResponse", property: "state_root", expected: "#/components/schemas/Hash" },
            PropertyRefContract { owner: "StateFinalityResponse", property: "block_header", expected: "#/components/schemas/BlockHeader" },
            PropertyRefContract {
                owner: "StateFinalityResponse",
                property: "finality_artifact",
                expected: "#/components/schemas/SumeragiV2FinalityArtifact",
            },
        ],
    );
    let schema = contract_schema(schemas, "StateFinalityResponse");
    let properties = schema.get("properties").and_then(Value::as_object).expect("state finality properties");
    for retired in contract_strings("ledger.state_finality.retired") {
        assert!(!properties.contains_key(retired), "first-release state finality schema retained `{retired}`");
    }
    for retired_schema in ["StateRootResponse", "StateProofResponse"] {
        assert!(!schemas.contains_key(retired_schema), "retired dual response schema `{retired_schema}` remains public");
    }
    assert_eq!(contract_property(schemas, "StateFinalityResponse", "height").get("minimum").and_then(Value::as_u64), Some(1));
    for path in ["/v1/ledger/state/{height}", "/v1/ledger/state-proof/{height}"] {
        let operation = openapi_operation(&document, path, "get");
        let description = operation.get("description").and_then(Value::as_str).expect("ledger state endpoint description");
        assert!(description.contains("Sumeragi-v2"));
        assert!(description.contains("fails closed"));
        assert_eq!(operation_response_schema_ref(operation, "200", path), "#/components/schemas/StateFinalityResponse");
        let content = response_content(operation, "200", path);
        assert_eq!(content.keys().map(String::as_str).collect::<BTreeSet<_>>(), BTreeSet::from(["application/json", "application/x-norito"]));
        let norito = content.get("application/x-norito").and_then(|media| media.get("schema")).and_then(Value::as_object).expect("Norito response schema");
        assert_eq!(norito.get("type").and_then(Value::as_str), Some("string"));
        assert_eq!(norito.get("format").and_then(Value::as_str), Some("binary"));
    }
    let paths = document.get("paths").and_then(Value::as_object).expect("OpenAPI paths");
    for retired_path in contract_strings("ledger.state_finality.retired_paths") {
        assert!(!paths.contains_key(retired_path), "retired legacy finality path remains public: {retired_path}");
    }
    for retired_schema in contract_strings("ledger.state_finality.retired_schemas") {
        assert!(!schemas.contains_key(retired_schema), "retired legacy finality schema remains public: {retired_schema}");
    }
}

#[test]
fn bridge_finality_operations_describe_durable_v2_evidence() {
    let document = generate_spec();
    for contract in [
        OperationResponseContract {
            path: "/v1/bridge/finality/{height}",
            method: "get",
            status: "200",
            schema_ref: "#/components/schemas/BridgeFinalityProof",
        },
        OperationResponseContract {
            path: "/v1/bridge/finality/attestation/{height}",
            method: "get",
            status: "200",
            schema_ref: "#/components/schemas/BridgeFinalityAttestationV1",
        },
        OperationResponseContract {
            path: "/v1/bridge/finality/bundle/{height}",
            method: "get",
            status: "200",
            schema_ref: "#/components/schemas/BridgeFinalityBundle",
        },
    ] {
        let operation = openapi_operation(&document, contract.path, contract.method);
        let description = operation.get("description").and_then(Value::as_str).expect("bridge description");
        assert!(description.contains("Sumeragi-v2") && description.contains("durable"));
        assert!(!description.contains("validator set signatures") && !description.contains("block header and commit certificate"));
        assert_eq!(operation_response_schema_ref(operation, "200", contract.path), contract.schema_ref);
        let norito = response_content(operation, "200", contract.path).get("application/x-norito").and_then(|media| media.get("schema")).and_then(Value::as_object).expect("Norito response schema");
        assert_eq!(norito.get("type").and_then(Value::as_str), Some("string"));
        assert_eq!(norito.get("format").and_then(Value::as_str), Some("binary"));
    }
    let path = "/v1/bridge/finality/attestation/{height}";
    let operation = openapi_operation(&document, path, "get");
    let challenge = operation_parameter(operation, "X-Iroha-Finality-Challenge", path);
    assert_eq!(challenge.get("in").and_then(Value::as_str), Some("header"));
    assert_eq!(challenge.get("required").and_then(Value::as_bool), Some(true));
    for status in ["200", "400", "404", "406", "503"] {
        let headers = operation_responses(operation, path).get(status).and_then(|response| response.get("headers")).and_then(Value::as_object).unwrap_or_else(|| panic!("{status} headers"));
        let constant = |name| headers.get(name).and_then(|header| header.get("schema")).and_then(|schema| schema.get("const")).and_then(Value::as_str);
        assert_eq!(constant("Cache-Control"), Some("no-store"));
        assert_eq!(constant("Vary"), Some("X-Iroha-Finality-Challenge, Accept"));
    }
}

#[test]
fn generated_spec_documents_read_only_nexus_lifecycle_status() {
    let document = generate_spec();
    let paths = document.get("paths").and_then(Value::as_object).expect("paths");
    let path = paths.get("/v1/nexus/lifecycle").and_then(Value::as_object).expect("lifecycle path");
    let operation = path.get("get").and_then(Value::as_object).expect("lifecycle GET");
    assert_eq!(operation_response_schema_ref(operation, "200", "/v1/nexus/lifecycle"), "#/components/schemas/NexusLaneLifecycleStatusV1");
    let content = response_content(operation, "200", "lifecycle");
    assert!(content.contains_key("application/json") && content.contains_key("application/x-norito"));
    assert!(!path.contains_key("post"), "lifecycle resource is read-only");
    let schemas = component_schemas(&document);
    let schema = contract_schema(schemas, "NexusLaneLifecycleStatusV1");
    assert_eq!(schema.get("additionalProperties"), Some(&Value::Bool(false)));
    assert_required_inventory(schema, "lifecycle.required", "lifecycle status");
    let properties = schema.get("properties").and_then(Value::as_object).expect("lifecycle properties");
    assert_eq!(object_field_set(properties), ["catalog_hash", "incarnation_root", "incarnations", "lane_count", "lanes", "version"].into_iter().collect(), "lifecycle schema must expose exactly the current V1 fields");
    assert_eq!(properties.get("lanes").and_then(|property| property.get("type")).and_then(Value::as_str), Some("array"));
    assert!(!properties.contains_key("nexus_enabled"), "the current-only lifecycle schema must not retain the removed enablement switch");
    for name in ["catalog_hash", "incarnations", "incarnation_root"] {
        assert!(properties.contains_key(name));
    }
    assert_eq!(properties.get("incarnations").and_then(|property| property.get("items")).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/NexusLaneLifecycleIncarnationEntry"));
}

#[test]
fn generated_spec_documents_exact_authoritative_sumeragi_v2_status() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    let max_validators = u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound");
    for (path, expected, label) in [("/v1/sumeragi/status", "#/components/schemas/SumeragiStatusResponse", "authoritative status"), ("/v1/sumeragi/diagnostics", "#/components/schemas/SumeragiDiagnosticsResponse", "operator diagnostics")] {
        let actual = document.get("paths").and_then(Value::as_object).and_then(|paths| paths.get(path)).and_then(Value::as_object).and_then(|item| item.get("get")).and_then(Value::as_object).map(|operation| operation_response_schema_ref(operation, "200", path));
        assert_eq!(actual, catalog_openapi_route_enabled(CatalogHttpMethod::Get, path).then_some(expected), "{label} catalog projection");
    }
    let status = contract_schema(schemas, "SumeragiStatusResponse");
    assert_eq!(status.get("additionalProperties").and_then(Value::as_bool), Some(false));
    let properties = status.get("properties").and_then(Value::as_object).expect("status properties");
    assert_eq!(properties.get("protocol_version").and_then(|schema| schema.get("minimum")).and_then(Value::as_u64), Some(4));
    for present in contract_strings("status.present") {
        assert!(properties.contains_key(present), "status missing {present}");
    }
    for absent in contract_strings("status.absent") {
        assert!(!properties.contains_key(absent), "status retained {absent}");
    }
    for field in ["node_fingerprint", "build_fingerprint", "config_fingerprint"] {
        assert_eq!(properties.get(field).and_then(|schema| schema.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/Hash"));
    }
    assert_eq!(properties.get("height_context_id").and_then(|schema| schema.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/SumeragiV2HeightContextId"));
    for contract in [
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "phase", expected: "#/components/schemas/SumeragiV2StatusPhase" },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "body_state",
            expected: "#/components/schemas/SumeragiV2BodyState",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "locked_prepare_qc",
            expected: "#/components/schemas/SumeragiV2QuorumCertificateRef",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "highest_prepare_qc",
            expected: "#/components/schemas/SumeragiV2QuorumCertificateRef",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "last_timeout_certificate",
            expected: "#/components/schemas/SumeragiV2TimeoutCertificateRef",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "last_committed_subject",
            expected: "#/components/schemas/SumeragiV2BlockSubject",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "height_context",
            expected: "#/components/schemas/SumeragiV2HeightContextStatus",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "last_commit_qc",
            expected: "#/components/schemas/SumeragiV2CommitQcStatus",
        },
        PropertyRefContract {
            owner: "SumeragiStatusResponse",
            property: "liveness",
            expected: "#/components/schemas/SumeragiV2LivenessStatus",
        },
    ] {
        assert_eq!(contract_property(schemas, contract.owner, contract.property).get("$ref").and_then(Value::as_str), Some(contract.expected));
    }
    let required = status.get("required").and_then(Value::as_array).expect("status required");
    for field in ["restart_required", "height_context", "liveness"] {
        assert!(required.iter().any(|value| value.as_str() == Some(field)));
    }
    assert!(!required.iter().any(|value| value.as_str() == Some("last_commit_qc")));
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "SumeragiV2HeightContextStatus",
                property: "mode",
                expected: "#/components/schemas/SumeragiV2ConsensusMode",
            },
            PropertyRefContract {
                owner: "SumeragiV2HeightContextStatus",
                property: "quorum",
                expected: "#/components/schemas/SumeragiV2DualQuorum",
            },
            PropertyRefContract {
                owner: "SumeragiV2CommitQcStatus",
                property: "certificate",
                expected: "#/components/schemas/SumeragiV2QuorumCertificateRef",
            },
        ],
    );
    assert_eq!(contract_property(schemas, "SumeragiV2HeightContextStatus", "validator_count").get("maximum").and_then(Value::as_u64), Some(max_validators));
    for field in ["validator_count", "signer_count", "min_signers"] {
        assert_eq!(contract_property(schemas, "SumeragiV2CommitQcStatus", field).get("maximum").and_then(Value::as_u64), Some(max_validators));
    }
    let diagnostics = contract_schema(schemas, "SumeragiDiagnosticsResponse");
    assert_eq!(diagnostics.get("additionalProperties").and_then(Value::as_bool), Some(false));
    let diagnostics = diagnostics.get("properties").and_then(Value::as_object).expect("diagnostics properties");
    assert!(diagnostics.contains_key("npos"));
    for (field, expected) in [
        ("lane_commitments", "#/components/schemas/SumeragiLaneCommitment"),
        ("dataspace_commitments", "#/components/schemas/SumeragiDataspaceCommitment"),
        ("lane_payload_ownerships", "#/components/schemas/SumeragiLanePayloadOwnership"),
        ("committed_lane_blocks", "#/components/schemas/SumeragiCommittedLaneBlock"),
        ("lane_block_sessions", "#/components/schemas/SumeragiLaneBlockSessionStatus"),
        ("lane_governance", "#/components/schemas/SumeragiLaneGovernance"),
    ] {
        assert_eq!(diagnostics.get(field).and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some(expected));
    }
    for field in ["height", "view", "phase", "leader", "locked_prepare_qc"] {
        assert!(!diagnostics.contains_key(field));
    }
    let settlement = contract_schema(schemas, "LaneSettlementCommitment");
    let settlement_properties = settlement.get("properties").and_then(Value::as_object).expect("settlement properties");
    assert_eq!(settlement_properties.get("native_amx_receipts").and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/NativeAmxReceipt"));
    assert_eq!(settlement_properties.get("native_amx_receipts").and_then(|schema| schema.get("maxItems")).and_then(Value::as_u64), Some(4_096));
    assert_eq!(settlement_properties.get("nexus_fee_receipts").and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/NexusFeeReceipt"));
    assert_eq!(settlement.get("additionalProperties").and_then(Value::as_bool), Some(false));
    assert!(schema_fields(settlement, "required", "settlement").iter().any(|field| field.as_str() == Some("lane_incarnation")));
    let receipt = contract_schema(schemas, "NativeAmxReceipt");
    assert_required_inventory(receipt, "native.receipt.required", "native AMX receipt");
    let legs = contract_property(schemas, "NativeAmxReceipt", "legs");
    assert_item_ref(legs, "#/components/schemas/NativeAmxLegRecord", "native AMX legs");
    assert_array_bounds(legs, 1, 255, Some(true));
    let leg_properties = contract_schema(schemas, "NativeAmxLegRecord").get("properties").and_then(Value::as_object).expect("leg properties");
    assert!(!leg_properties.contains_key("lane_incarnation"));
    assert_eq!(component_required(schemas, "NativeAmxLegRecord"), contract_strings("native.leg.required"));
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "NativeAmxLegRecord",
                property: "participant_proposal",
                expected: "#/components/schemas/NativeAmxParticipantLaneBlockProposal",
            },
            PropertyRefContract {
                owner: "NativeAmxLegRecord",
                property: "participant_settlement",
                expected: "#/components/schemas/NativeAmxParticipantSettlementCommitment",
            },
            PropertyRefContract { owner: "NativeAmxLegRecord", property: "participant_settlement_hash", expected: "#/components/schemas/Hash" },
            PropertyRefContract { owner: "NativeAmxLegRecord", property: "prepare_qc", expected: "#/components/schemas/NativeAmxAttestationQc" },
            PropertyRefContract { owner: "NativeAmxLegRecord", property: "commit_qc", expected: "#/components/schemas/NativeAmxAttestationQc" },
            PropertyRefContract { owner: "NativeAmxAttestationQc", property: "body", expected: "#/components/schemas/NativeAmxAttestationBody" },
        ],
    );
    let proposal = contract_schema(schemas, "NativeAmxParticipantLaneBlockProposal");
    assert_eq!(proposal.get("additionalProperties").and_then(Value::as_bool), Some(false));
    assert_eq!(schema_fields(proposal, "required", "native AMX participant proposal").iter().filter_map(Value::as_str).collect::<BTreeSet<_>>(), contract_strings("native.proposal.required").into_iter().collect::<BTreeSet<_>>());
    assert_eq!(contract_property(schemas, "NativeAmxParticipantLaneBlockProposal", "payload_block_hint").get("type").and_then(Value::as_str), Some("null"));
    let proposal_description = proposal.get("description").and_then(Value::as_str).expect("native AMX participant proposal description");
    assert!(proposal_description.contains("requires payload_block_hint to be present as null"));
    let participant = contract_schema(schemas, "NativeAmxParticipantSettlementCommitment");
    assert_eq!(participant.get("additionalProperties").and_then(Value::as_bool), Some(false));
    for field in ["total_local_amount", "total_xor_due", "total_xor_after_haircut", "total_xor_variance"] {
        assert_eq!(contract_property(schemas, "NativeAmxParticipantSettlementCommitment", field).get("const").and_then(Value::as_str), Some("0"));
    }
    for field in ["nexus_fee_receipts", "native_amx_receipts"] {
        assert_eq!(contract_property(schemas, "NativeAmxParticipantSettlementCommitment", field).get("maxItems").and_then(Value::as_u64), Some(0));
    }
    let participant_receipts = contract_property(schemas, "NativeAmxParticipantSettlementCommitment", "receipts");
    assert_array_bounds(participant_receipts, 1, 4_096, Some(true));
    assert_item_ref(participant_receipts, "#/components/schemas/NativeAmxParticipantSettlementReceipt", "participant receipts");
    let qc = contract_schema(schemas, "NativeAmxAttestationQc").get("properties").and_then(Value::as_object).expect("native AMX QC properties");
    for (field, minimum, maximum, unique, item) in [("validator_set", 1, 128, Some(true), "#/components/schemas/SumeragiV2BlsValidatorId"), ("validator_set_pops", 1, 128, None, "#/components/schemas/SumeragiV2BlsProof")] {
        let array = qc.get(field).and_then(Value::as_object).unwrap_or_else(|| panic!("{field} schema"));
        assert_array_bounds(array, minimum, maximum, unique);
        assert_item_ref(array, item, field);
    }
    assert_array_bounds(qc.get("signers_bitmap").and_then(Value::as_object).expect("signers bitmap"), 1, 16, None);
    assert_eq!(qc.get("bls_aggregate_signature").and_then(|schema| schema.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/SumeragiV2BlsProof"));
    for field in ["accepted_candidate_indices", "accepted_transaction_hashes"] {
        assert_array_bounds(contract_property(schemas, "NativeAmxParticipantLaneBlockDescriptor", field), 1, 4_096, Some(true));
    }
    let body = contract_schema(schemas, "NativeAmxAttestationBody");
    assert_required_inventory(body, "native.body.required", "native AMX body");
    let body_required = schema_fields(body, "required", "native AMX body");
    assert!(!body_required.iter().any(|field| field.as_str() == Some("coordinator_lane_block_height")));
    for field in ["participant_validator_count", "participant_min_quorum"] {
        assert_eq!(contract_property(schemas, "NativeAmxAttestationBody", field).get("maximum").and_then(Value::as_u64), Some(128));
    }
    assert_eq!(contract_property(schemas, "NativeAmxAttestationBody", "phase").get("$ref").and_then(Value::as_str), Some("#/components/schemas/NativeAmxPhase"));
    let phase = contract_schema(schemas, "NativeAmxPhase").get("properties").and_then(Value::as_object).expect("native phase properties");
    let phase_values = phase.get("phase").and_then(|tag| tag.get("enum")).and_then(Value::as_array).expect("phase enum");
    for value in ["prepare", "commit"] {
        assert!(phase_values.iter().any(|entry| entry.as_str() == Some(value)));
    }
    assert_eq!(phase.get("detail").and_then(|detail| detail.get("type")).and_then(Value::as_str), Some("null"));
    for (name, tag, expected) in [("LaneLiquidityProfile", "profile", &["Tier1", "Tier2", "Tier3"][..]), ("LaneVolatilityClass", "bucket", &["Stable", "Elevated", "Dislocated"])] {
        let values = contract_property(schemas, name, tag).get("enum").and_then(Value::as_array).expect("tag enum");
        for expected in expected {
            assert!(values.iter().any(|value| value.as_str() == Some(*expected)));
        }
    }
    assert_property_refs(
        schemas,
        &[
            PropertyRefContract {
                owner: "LaneSwapMetadata",
                property: "liquidity_profile",
                expected: "#/components/schemas/LaneLiquidityProfile",
            },
            PropertyRefContract {
                owner: "LaneSwapMetadata",
                property: "volatility_class",
                expected: "#/components/schemas/LaneVolatilityClass",
            },
        ],
    );
    for field in ["total_local_amount", "total_xor_due", "total_xor_after_haircut", "total_xor_variance"] {
        assert_eq!(settlement_properties.get(field).and_then(|property| property.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/Quantity"));
    }
    for retired in ["total_local_micro", "total_xor_due_micro", "total_xor_after_haircut_micro", "total_xor_variance_micro"] {
        assert!(!settlement_properties.contains_key(retired));
    }
    let receipt_properties = contract_schema(schemas, "LaneSettlementReceipt").get("properties").and_then(Value::as_object).expect("settlement receipt properties");
    assert_eq!(receipt_properties.get("source_id").and_then(|schema| schema.get("pattern")).and_then(Value::as_str), Some("^[0-9A-F]{64}$"));
    for (field, retired) in [("local_amount", "local_amount_micro"), ("xor_due", "xor_due_micro"), ("xor_after_haircut", "xor_after_haircut_micro"), ("xor_variance", "xor_variance_micro")] {
        assert_eq!(receipt_properties.get(field).and_then(|schema| schema.get("$ref")).and_then(Value::as_str), Some("#/components/schemas/Quantity"));
        assert!(!receipt_properties.contains_key(retired));
    }
    for (owner, field) in [("NexusFeeReceipt", "lane_id"), ("NativeAmxAttestationBody", "coordinator_lane_id"), ("NativeAmxAttestationBody", "participant_lane_id"), ("NativeAmxLegRecord", "lane_id"), ("NativeAmxReceipt", "lane_id"), ("LaneSettlementCommitment", "lane_id"), ("LaneRelayEnvelope", "lane_id")] {
        assert_eq!(contract_property(schemas, owner, field).get("maximum").and_then(Value::as_u64), Some(u64::from(u32::MAX)), "{owner}.{field} uint32 bound");
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one cohesive exact Soracloud priority-contract inventory")]
fn generated_spec_documents_exact_soracloud_priority_contracts() {
    let document = canonical_document();
    let paths = document.get("paths").and_then(Value::as_object).expect("paths");
    assert_eq!(paths.keys().filter(|path| path.starts_with("/v1/soracloud/")).count(), 51);
    for retired_path in [
        "/v1/soracloud/agent/autonomy/run",
        "/v1/soracloud/agent/autonomy/run/finalize",
        "/v1/soracloud/model-host/advertise",
        "/v1/soracloud/model-host/heartbeat",
        "/v1/soracloud/model-host/withdraw",
        "/v1/soracloud/model-host/status",
    ] {
        assert!(!paths.contains_key(retired_path), "retired generated-HF inference ingress `{retired_path}` must not be published");
    }
    for path in ["/v1/soracloud/deploy", "/v1/soracloud/upgrade"] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_request_schema_ref(operation, path), "#/components/schemas/SignedBundleRequest");
        assert_eq!(operation_response_schema_ref(operation, "200", path), "#/components/schemas/SoracloudMutationDraftResponse");
    }
    let schemas = component_schemas(&document);
    for retired_schema in ["AgentAutonomyRunPayload", "SignedAgentAutonomyRunRequest", "AgentAutonomyFinalizeRequest"] {
        assert!(!schemas.contains_key(retired_schema), "retired generated-HF inference schema `{retired_schema}` must not be published");
    }
    assert_strict_object_schema(schemas, "SignedBundleRequest", &["bundle", "initial_service_configs", "initial_service_secrets", "provenance"], &[]);
    let status_operation = openapi_operation(&document, "/v1/soracloud/status", "get");
    assert_eq!(operation_response_schema_ref(status_operation, "200", "/v1/soracloud/status"), "#/components/schemas/SoracloudStatusV1");
    let status = status_operation.get("description").and_then(Value::as_str).expect("Soracloud status description");
    for phrase in ["configured_lane_count", "declared_lane_count", "active lane ids/count", "autoscale-capacity lane ids/count"] {
        assert!(status.contains(phrase));
    }
    let join = openapi_operation(&document, "/v1/soracloud/hf/lease/join", "post");
    assert_eq!(operation_request_schema_ref(join, "/v1/soracloud/hf/lease/join"), "#/components/schemas/SignedHfSharedLeaseJoinRequest");
    assert_eq!(operation_response_schema_ref(join, "200", "/v1/soracloud/hf/lease/join"), "#/components/schemas/SoracloudMutationDraftResponse");
    let headers = operation_parameters(join, "HF shared-lease join");
    for name in contract_strings("hf.headers") {
        assert!(headers.iter().any(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name)), "HF shared-lease join missing {name}");
    }
    assert_strict_object_schema(schemas, "SignedHfSharedLeaseJoinRequest", &["payload", "provenance"], &[]);
    assert_strict_object_schema(schemas, "HfSharedLeaseJoinPayload", &["repo_id", "revision", "service_name", "apartment_name", "storage_class", "lease_term_ms", "lease_asset_definition_id", "base_fee"], &[]);
    let apartment_name = contract_property(schemas, "HfSharedLeaseJoinPayload", "apartment_name").get("anyOf").and_then(Value::as_array).expect("HF apartment_name nullable union");
    assert_eq!(apartment_name.len(), 2);
    assert_eq!(apartment_name[0].get("type").and_then(Value::as_str), Some("string"));
    assert_eq!(apartment_name[1].get("type").and_then(Value::as_str), Some("null"));
    assert_strict_object_schema(schemas, "SoracloudMutationDraftResponse", &["ok", "authority", "signed_by", "tx_instructions"], &[]);
    assert_item_ref(contract_property(schemas, "SoracloudMutationDraftResponse", "tx_instructions"), "#/components/schemas/SoracloudTxInstruction", "Soracloud mutation draft instructions");
    assert_eq!(contract_property(schemas, "SoracloudMutationDraftResponse", "tx_instructions").get("minItems").and_then(Value::as_u64), Some(1), "Soracloud mutation drafts must contain at least one transaction instruction",);
    assert_eq!(contract_property(schemas, "SoracloudTxInstruction", "payload_hex").get("pattern").and_then(Value::as_str), Some("^(?:[0-9a-f]{2})+$"));
    let wallet_spend = openapi_operation(&document, "/v1/soracloud/agent/wallet/spend", "post");
    assert_eq!(operation_request_schema_ref(wallet_spend, "agent wallet spend"), "#/components/schemas/SignedAgentWalletSpendRequest");
    assert_strict_object_schema(schemas, "AgentWalletSpendPayload", &["apartment_name", "request_id", "asset_definition", "amount"], &[]);
    assert_strict_object_schema(schemas, "SoracloudStatusV1", &["schema_version", "service_health", "routing", "hosted_http_topology", "resource_pressure", "failed_admissions", "runtime_manager", "control_plane"], &[]);
    let topology = contract_property(schemas, "SoracloudStatusV1", "hosted_http_topology");
    assert_eq!(topology.get("additionalProperties").and_then(Value::as_bool), Some(false));
    let topology_fields = BTreeSet::from(["active_capability_adverts", "hosted_replica_count", "placed_host_count", "unavailable_replica_count"]);
    assert_eq!(object_field_set(topology.get("properties").and_then(Value::as_object).expect("Soracloud hosted HTTP topology properties")), topology_fields);
    assert_eq!(schema_fields(topology, "required", "Soracloud hosted HTTP topology").iter().map(|field| field.as_str().expect("topology field")).collect::<BTreeSet<_>>(), topology_fields);
    for field in topology_fields {
        let property = topology.get("properties").and_then(Value::as_object).and_then(|properties| properties.get(field)).and_then(Value::as_object).unwrap_or_else(|| panic!("hosted topology {field}"));
        assert_eq!(property.get("type").and_then(Value::as_str), Some("integer"));
        assert_eq!(property.get("format").and_then(Value::as_str), Some("uint64"));
        assert_eq!(property.get("minimum").and_then(Value::as_u64), Some(0));
    }
    let routing = contract_property(schemas, "SoracloudStatusV1", "routing");
    assert_eq!(routing.get("additionalProperties").and_then(Value::as_bool), Some(false));
    let routing_properties = routing.get("properties").and_then(Value::as_object).expect("Soracloud status routing properties");
    let routing_fields = BTreeSet::from(["configured_lane_count", "declared_lane_count", "active_lane_count", "active_lane_ids", "autoscale_capacity_lane_count", "autoscale_capacity_lane_ids", "dataspace_count", "routing_rules", "default_lane_id", "default_dataspace_id"]);
    assert_eq!(object_field_set(routing_properties), routing_fields);
    assert_eq!(schema_fields(routing, "required", "Soracloud status routing").iter().map(|field| field.as_str().expect("routing field")).collect::<BTreeSet<_>>(), routing_fields);
    assert_eq!(property_ref(schemas, "SoracloudStatusV1", "runtime_manager"), "#/components/schemas/SoracloudStatusRuntimeManagerV1");
    assert_exact_closed_required_schema_fields(schemas, "ControlPlaneAuditEvent",
        &["sequence", "action", "service_name", "from_version", "to_version", "service_manifest_hash", "container_manifest_hash", "process_generation", "config_generation", "secret_generation", "config_snapshot_hash", "secret_snapshot_hash", "binding_name", "state_key", "config_mutations", "secret_mutations", "governance_tx_hash", "rollout_state", "policy_name", "policy_snapshot_hash", "jurisdiction_tag", "consent_evidence_hash", "break_glass", "break_glass_reason", "lease_usage", "service_lease_commitment", "lease_reporting_epoch_rollover", "signed_by"]);
    assert_eq!(nullable_property_ref(schemas, "ControlPlaneAuditEvent", "lease_reporting_epoch_rollover"), "#/components/schemas/SoraServiceLeaseReportingEpochRolloverV1");
    assert_exact_closed_required_schema_fields(schemas, "SoraServiceLeaseReportingEpochRolloverV1", &["schema_version", "economic_clock", "lease_started_height", "previous_reporting_epoch", "new_reporting_epoch", "reporter_account_id", "active_service_version", "replica_slot", "placement_incarnation", "finalized_checkpoint_count", "settled_egress_bytes_delta", "settled_egress_bytes"]);
    assert_eq!(property_ref(schemas, "SoraServiceLeaseReportingEpochRolloverV1", "economic_clock"), "#/components/schemas/SoraServiceLeaseClockV1");
    assert_eq!(property_ref(schemas, "SoraServiceLeaseReportingEpochRolloverV1", "placement_incarnation"), "#/components/schemas/Hash");
    let action_variants = contract_schema(schemas, "SoracloudAction").get("oneOf").and_then(Value::as_array).expect("SoracloudAction variants").iter()
        .map(|variant| variant.get("properties").and_then(Value::as_object).and_then(|properties| properties.get("action")).and_then(Value::as_object).and_then(|action| action.get("const")).and_then(Value::as_str).expect("SoracloudAction const")).collect::<BTreeSet<_>>();
    assert_eq!(action_variants, BTreeSet::from(["CiphertextQuery", "ConfigMutation", "DecryptionRequest", "Deploy", "FheJobRun", "FhePolicyRegister", "FhePolicyRevoke", "FhePolicyRotate", "LeaseUsage", "LeaseReportingEpochRollover", "Rollback", "Rollout", "SecretMutation", "StateMutation", "Upgrade"]));

    for (path, method, request, response) in [("/v1/soracloud/agent/autonomy/allow", "post", Some("#/components/schemas/SignedAgentArtifactAllowRequest"), "#/components/schemas/SoracloudMutationDraftResponse"), ("/v1/soracloud/agent/autonomy/status", "get", None, "#/components/schemas/AgentAutonomyStatusResponse")] {
        let operation = openapi_operation(&document, path, method);
        if let Some(request) = request {
            assert_eq!(operation_request_schema_ref(operation, path), request);
        }
        assert_eq!(operation_response_schema_ref(operation, "200", path), response);
    }
    assert_strict_object_schema(schemas, "SignedAgentArtifactAllowRequest", &["payload", "provenance"], &[]);
    assert_strict_object_schema(schemas, "AgentArtifactAllowPayload", &["apartment_name", "artifact_hash", "provenance_hash"], &[]);
    assert_strict_object_schema(
        schemas,
        "AgentAutonomyStatusResponse",
        &[
            "apartment_name",
            "sequence",
            "status",
            "lease_expires_height",
            "lease_remaining_blocks",
            "manifest_hash",
            "revoked_policy_capability_count",
            "budget_ceiling_units",
            "budget_remaining_units",
            "allowlist_count",
            "run_count",
            "process_generation",
            "process_started_sequence",
            "last_active_sequence",
            "last_checkpoint_sequence",
            "checkpoint_count",
            "persistent_state_total_bytes",
            "persistent_state_key_count",
            "allowlist",
            "recent_runs",
        ],
        &[],
    );
}

#[test]
fn generated_spec_documents_app_query_page_metadata() {
    let document = generate_spec();
    assert_operation_response_contracts(
        &document,
        &[
            OperationResponseContract { path: "/v1/accounts", method: "get", status: "200", schema_ref: "#/components/schemas/AccountListResponse" },
            OperationResponseContract {
                path: "/v1/accounts/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/AccountQueryResponse",
            },
            OperationResponseContract { path: "/v1/domains", method: "get", status: "200", schema_ref: "#/components/schemas/DomainListResponse" },
            OperationResponseContract {
                path: "/v1/domains/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/DomainQueryResponse",
            },
            OperationResponseContract {
                path: "/v1/accounts/{account_id}/assets",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/AccountAssetListResponse",
            },
            OperationResponseContract {
                path: "/v1/accounts/{account_id}/assets/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/AccountAssetQueryResponse",
            },
            OperationResponseContract {
                path: "/v1/assets/definitions",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/AssetDefinitionListResponse",
            },
            OperationResponseContract {
                path: "/v1/assets/definitions/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/AssetDefinitionQueryResponse",
            },
            OperationResponseContract {
                path: "/v1/assets/{definition_id}/holders",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/AssetHolderListResponse",
            },
            OperationResponseContract {
                path: "/v1/assets/{definition_id}/holders/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/AssetHolderQueryResponse",
            },
            OperationResponseContract { path: "/v1/nfts", method: "get", status: "200", schema_ref: "#/components/schemas/NftListResponse" },
            OperationResponseContract { path: "/v1/nfts/query", method: "post", status: "200", schema_ref: "#/components/schemas/NftQueryResponse" },
            OperationResponseContract { path: "/v1/rwas", method: "get", status: "200", schema_ref: "#/components/schemas/RwaListResponse" },
            OperationResponseContract { path: "/v1/rwas/query", method: "post", status: "200", schema_ref: "#/components/schemas/RwaQueryResponse" },
            OperationResponseContract {
                path: "/v1/repo/agreements",
                method: "get",
                status: "200",
                schema_ref: "#/components/schemas/RepoAgreementListResponse",
            },
            OperationResponseContract {
                path: "/v1/repo/agreements/query",
                method: "post",
                status: "200",
                schema_ref: "#/components/schemas/RepoAgreementListResponse",
            },
        ],
    );
    let schemas = component_schemas(&document);
    let metadata = contract_schema(schemas, "AppPageMetadata");
    assert_required_inventory(metadata, "app.page.required", "app page metadata");
    let required = schema_fields(metadata, "required", "app page metadata");
    assert!(!required.iter().any(|field| field.as_str() == Some("total")));
    let properties = metadata.get("properties").and_then(Value::as_object).expect("metadata properties");
    for field in contract_strings("app.page.properties") {
        assert!(properties.contains_key(field), "metadata missing {field}");
    }
    let repo = contract_schema(schemas, "RepoAgreement");
    let repo_required = schema_fields(repo, "required", "repo agreement");
    let repo_properties = repo.get("properties").and_then(Value::as_object).expect("repo properties");
    for field in contract_strings("repo.agreement.fields") {
        assert!(repo_required.iter().any(|value| value.as_str() == Some(field)));
        assert!(repo_properties.contains_key(field));
    }
    let query = contract_schema(schemas, "RepoAgreementsQueryRequest").get("properties").and_then(Value::as_object).expect("repo query properties");
    for field in contract_strings("repo.query.fields") {
        assert!(query.contains_key(field));
    }
}

#[test]
fn alias_openapi_documents_optional_public_and_exact_restricted_auth() {
    let document = generate_spec();
    for path in ["/v1/aliases/resolve-index", "/v1/aliases/by-account"] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_alias_auth_required_response(operation, path);
        assert!(operation_responses(operation, path).contains_key("403"));
    }
    let lookup = openapi_operation(&document, "/v1/aliases/by-account", "post").get("description").and_then(Value::as_str).expect("alias description");
    assert!(lookup.contains("Canonical authentication is optional") && lookup.contains("required for restricted data"));
    let resolve = openapi_operation(&document, "/v1/aliases/resolve", "post");
    assert_eq!(operation_header_requirements(resolve), canonical_account_header_requirements(false));
    assert_alias_auth_required_response(resolve, "/v1/aliases/resolve");
    assert!(resolve.get("description").and_then(Value::as_str).is_some_and(|text| text.contains("Public dataspaces may be read unsigned")));
    assert!(operation_responses(resolve, "alias resolve").contains_key("403"));
    for path in ["/v1/aliases/setup/plan", "/v1/aliases/lease/renew/plan", "/v1/aliases/auto-renew/plan"] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_alias_auth_required_response(operation, path);
        assert!(operation_responses(operation, path).contains_key("409"));
    }
    for (path, reject, forbidden) in [("/v1/retail/recipients/lookup", "recipient_lookup_signature_required", true), ("/v1/retail/recipients/route", "recipient_lookup_signature_required", true), ("/v1/fee-sponsor-programs/by-id", "fee_sponsor_program_signature_required", false), ("/v1/fees/quote", "fee_quote_signature_required", false)] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, reject);
        assert_eq!(operation_responses(operation, path).contains_key("403"), forbidden);
    }
}

#[test]
fn protected_contract_identity_openapi_is_signed_and_exact() {
    let document = generate_spec();
    for (path, method, response_schema, reject, statuses) in [
        ("/v1/contracts/aliases/resolve", "post", "#/components/schemas/ContractAliasResolveResponse", "alias_auth_required", &["200", "400", "401", "404", "429", "500"][..]),
        ("/v1/gov/contracts/{contract_address}", "get", "#/components/schemas/GovernedContractResponse", "contract_code_auth_required", &["200", "400", "401", "404", "429", "500"]),
        ("/v1/contracts/code-bytes/{code_hash}", "get", "#/components/schemas/JsonValue", "contract_code_auth_required", &["200", "400", "401", "404", "429"]),
    ] {
        let operation = openapi_operation(&document, path, method);
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, reject);
        assert_eq!(operation_response_schema_ref(operation, "200", path), response_schema);
        assert_eq!(operation_responses(operation, path).keys().map(String::as_str).collect::<BTreeSet<_>>(), statuses.iter().copied().collect::<BTreeSet<_>>());
    }
    assert_eq!(operation_request_schema_ref(openapi_operation(&document, "/v1/contracts/aliases/resolve", "post"), "/v1/contracts/aliases/resolve"), "#/components/schemas/ContractAliasResolveRequest");
    let schemas = component_schemas(&document);
    assert_schema_shapes(
        schemas,
        &[
            SchemaShape { name: "ContractAliasResolveRequest", required: "contract.alias.request.required", optional: None },
            SchemaShape {
                name: "ContractAliasBinding",
                required: "contract.alias.binding.required",
                optional: Some("contract.alias.binding.optional"),
            },
            SchemaShape { name: "ContractAliasResolveResponse", required: "contract.alias.response.required", optional: None },
        ],
    );
    let variants = contract_schema(schemas, "GovernedContractResponse").get("oneOf").and_then(Value::as_array).expect("governed variants");
    assert_eq!(variants.len(), 3);
    for (variant, inventory) in variants.iter().zip(["governed.found.fields", "governed.inactive.fields", "governed.missing.fields"]) {
        let variant = variant.as_object().expect("governed variant");
        assert_eq!(variant.get("additionalProperties"), Some(&Value::Bool(false)));
        let expected = asset_field_set(inventory);
        assert_eq!(schema_fields(variant, "required", "governed variant").iter().filter_map(Value::as_str).collect::<BTreeSet<_>>(), expected);
        assert_eq!(variant.get("properties").and_then(Value::as_object).map(object_field_set), Some(expected));
    }
    for (schema, expected) in [
        (
            "GovernedContractLifecycleV1",
            [
                "version",
                "origin",
                "origin_account",
                "origin_proposal_content_id_hex",
                "origin_governance_attempt_id_hex",
                "owner",
                "pending_owner",
                "parliament_delegated",
                "active_code_hash_hex",
                "revision",
                "emergency_hold",
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ),
        (
            "GovernedContractEmergencyHoldV1",
            [
                "incident_digest_hex",
                "proposal_content_id_hex",
                "governance_attempt_id_hex",
                "reason",
                "imposed_at_height",
                "expires_at_height",
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ),
    ] {
        let schema = contract_schema(schemas, schema);
        assert_eq!(schema.get("additionalProperties"), Some(&Value::Bool(false)));
        assert_eq!(
            schema_fields(schema, "required", "governed lifecycle component")
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>(),
            expected
        );
    }
}

#[test]
fn multisig_read_auth_contract_is_path_specific() {
    let document = generate_spec();
    for path in ["/v1/multisig/spec", "/v1/multisig/proposals/query", "/v1/multisig/proposals/resolve"] {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, "multisig_read_auth_required");
        let description = operation.get("description").and_then(Value::as_str).expect("multisig read description");
        for phrase in ["Both canonical `multisig_account_id`", "`multisig_account_alias` selectors require", "body fields never establish signer identity"] {
            assert!(description.contains(phrase));
        }
    }
    for path in ["/v1/multisig/propose", "/v1/multisig/approve", "/v1/multisig/cancel", "/v1/contracts/call/multisig/propose", "/v1/contracts/call/multisig/approve"] {
        assert!(operation_header_requirements(openapi_operation(&document, path, "post")).is_empty(), "POST {path} body-authenticated write contract");
    }
}

}
