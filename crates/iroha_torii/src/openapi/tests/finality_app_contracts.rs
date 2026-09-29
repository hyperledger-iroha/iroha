#[rustfmt::skip]
mod compact_finality_app_contracts {
use super::*;
fn schema_fields<'a>(schema: &'a Map, key: &str, context: &str) -> &'a Vec<Value> {
    contract_array(schema.get(key), &format!("{context}.{key}"))
}
fn assert_array_bounds(schema: &Map, minimum: u64, maximum: u64, unique: Option<bool>) {
    scalar_contracts! { schema.get("minItems") => Unsigned(minimum); schema.get("maxItems") => Unsigned(maximum); }
    if let Some(unique) = unique {
        scalar_contracts! { schema.get("uniqueItems") => Flag(unique); }
    }
}
fn assert_item_ref(schema: &Map, expected: &str) {
    scalar_contracts! { schema.get("items").and_then(Value::as_object).and_then(|items| items.get("$ref")) => Text(expected); }
}
fn object_field_set(object: &Map) -> BTreeSet<&str> {
    object.keys().map(String::as_str).collect()
}
fn schema_string_field_set<'a>(schema: &'a Map, key: &str, context: &str) -> BTreeSet<&'a str> {
    schema_fields(schema, key, context).iter().map(|value| value.as_str().unwrap_or_else(|| panic!("{context}.{key} contains a non-string value"))).collect()
}
fn assert_exact_closed_required_schema_fields(schemas: &Map, name: &str, expected: &[&str]) {
    let schema = contract_schema(schemas, name);
    scalar_contracts! { schema.get("additionalProperties") => Flag(false); }
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(object_field_set(schema.get("properties").and_then(Value::as_object).unwrap_or_else(|| panic!("{name}.properties"))), expected, "{name} property surface");
    assert_eq!(schema_string_field_set(schema, "required", name), expected, "{name} required-field surface");
}
fn asset_field_set(inventory: &str) -> BTreeSet<&'static str> {
    contract_strings(inventory).into_iter().collect()
}
#[test]
fn inrou_guest_image_schema_requires_one_concrete_published_artifact() {
    let schemas = openapi_schemas();
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouGuestImageV1", &contract_words("initrd_image_path kernel_image_path published_artifact rootfs_image_path"));
    let published_artifact = contract_property(&schemas, "SoraInrouGuestImageV1", "published_artifact");
    scalar_contracts! { published_artifact.get("$ref") => Text("#/components/schemas/SoraPublishedInrouGuestImageArtifactV1"); }
    assert!(published_artifact.get("anyOf").is_none() && published_artifact.get("oneOf").is_none() && published_artifact.get("nullable").is_none(), "admitted Inrou V1 artifacts must not expose a missing/null compatibility branch");
    assert_exact_closed_required_schema_fields(&schemas, "SoraPublishedInrouGuestImageArtifactV1", &contract_words("content_cid manifest_digest_hex"));
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouManifestV1", &contract_words("guest_images schema_version"));
    member_contracts! { &schemas; Absent => contract_words("SoraArtifactDistributionPolicyV1 SoraArtifactDistributionTargetV1 SoraInrouGuestOsV1"); }
    for (schema_name, retired_fields) in contract_rows! { "SoraCapabilityPolicyV1", &contract_words("allow_wallet_signing")[..]; "SoraInrouHostCapabilityRecordV1", &contract_words("geography_tags observed_latency_ms")[..]; "SoraInrouReplicaPlacementV1", &contract_words("selected_geography_tag selection_latency_ms")[..]; } {
        let properties = contract_object(contract_schema(&schemas, schema_name).get("properties"), &format!("{schema_name} properties"));
        member_contracts! { properties; Absent => retired_fields; }
    }
}
#[test]
fn inrou_first_release_openapi_matches_block_clock_and_exact_admission() {
    let schemas = openapi_schemas();
    assert_exact_closed_required_schema_fields(&schemas, "SoraHttpServiceEconomicsV1", &contract_words(concat!("schema_version quota_class deployment_deposit prepaid_runtime_balance ", "lease_duration_blocks runtime_price_per_block storage_price_per_gib_block ", "egress_price_per_mib")));
    let economics = contract_schema(&schemas, "SoraHttpServiceEconomicsV1");
    let economics_json = norito::json::to_string(&Value::Object(economics.clone())).expect("serialize economics");
    for retired in contract_words("lease_duration_sequences runtime_price_per_sequence storage_price_per_gib_sequence") {
        assert!(!economics_json.contains(retired), "retired sequence-clock economics field `{retired}`");
    }
    let guest_images = contract_property(&schemas, "SoraInrouManifestV1", "guest_images");
    scalar_contracts! { guest_images.get("minProperties") => Unsigned(1); guest_images.get("maxProperties") => Unsigned(2); guest_images.get("additionalProperties").and_then(Value::as_object).and_then(|items| items.get("$ref")) => Text("#/components/schemas/SoraInrouGuestImageV1"); }
    let admitted_guest_isas = contract_array(guest_images.get("propertyNames").and_then(Value::as_object).and_then(|names| names.get("enum")), "Inrou guest-image property-name enum").iter().map(|value| value.as_str().expect("guest ISA name")).collect::<BTreeSet<_>>();
    set_contracts! { admitted_guest_isas => contract_word_set("aarch64 x86_64"); }
    scalar_contracts! { contract_property(&schemas, "SoraInrouReplicaPlacementV1", "lease_started_height").get("minimum") => Unsigned(1); }
    for schema_name in contract_words("SoraInrouPlacementTargetV1 SoraInrouReplicaPlacementV1") {
        for field in contract_words("validator_account_id peer_id") {
            scalar_contracts! { contract_property(&schemas, schema_name, field) .get("minLength") => Unsigned(1); }
        }
    }
    let exact_resource_bounds = contract_rows! {
        "cpu_millis", u64::from(iroha_data_model::soracloud::SORA_INROU_MIN_CPU_MILLIS_V1), Some(u64::from(iroha_data_model::soracloud::SORA_INROU_CPU_MILLIS_ALIGNMENT_V1));
        "memory_bytes", iroha_data_model::soracloud::SORA_INROU_MIN_MEMORY_BYTES_V1, Some(iroha_data_model::soracloud::SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1);
        "ephemeral_storage_bytes", iroha_data_model::soracloud::SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1, Some(iroha_data_model::soracloud::SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1);
        "max_open_files_per_process", u64::from(iroha_data_model::soracloud::SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1), None;
    };
    for (field, minimum, multiple_of) in exact_resource_bounds {
        let property = contract_property(&schemas, "SoraResourceLimitsV1", field);
        scalar_contracts! { property.get("minimum") => Unsigned(minimum); }
        assert_eq!(property.get("multipleOf").and_then(Value::as_u64), multiple_of, "SoraResourceLimitsV1.{field} enforcement unit");
    }
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouHostCapabilityRecordV1", &contract_words(concat!("schema_version validator_account_id peer_id supported_guest_isas ", "trusted_guest_artifact max_hosted_replica_capacity max_cpu_millis ", "max_memory_bytes max_storage_bytes advertised_at_ms heartbeat_expires_at_ms")));
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouPlacementTargetV1", &contract_words("validator_account_id peer_id"));
    assert_exact_closed_required_schema_fields(&schemas, "SoraInrouReplicaPlacementV1", &contract_words(concat!("replica_slot economic_clock lease_started_height placement_incarnation ", "host_availability validator_account_id peer_id selected_guest_isa")));
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeInrouPlan", &contract_words(concat!("selected_guest_isa kernel_image_path rootfs_image_path initrd_image_path ", "root_volume_name")));
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeLeaseVolumePlan", &contract_words(concat!("volume_name kind storage_class mount_path max_total_bytes lease_started_height ", "lease_expires_height authoritative_generation local_materialization_dir")));
    assert_exact_closed_required_schema_fields(&schemas, "SoracloudRuntimeReplicaPlan", &contract_words(concat!("replica_slot lease_started_height placement_incarnation host_availability ", "validator_account_id peer_id materialization_dir health_status listen_base_url ", "pid last_error")));
    assert_exact_closed_required_schema_fields(
        &schemas,
        "SoracloudPublicServiceDiscoveryV1",
        &contract_words(concat!("schema_version service_name service_version execution_plane runtime route_host ", "path_prefix base_url healthcheck_path healthcheck_url service_manifest_hash ", "container_manifest_hash deployment_bundle_hash document_hash content_cid ", "public_discovery_url public_discovery_cid_host_url manifest_digest_hex")),
    );
    let runtime_service = contract_object(contract_schema(&schemas, "SoracloudRuntimeServicePlan").get("properties"), "SoracloudRuntimeServicePlan properties");
    member_contracts! { runtime_service; Present => ["lease_expires_height"]; Absent => contract_words("lease_expires_sequence supports_private_secret_payload_reads"); }
    let control_plane_service = contract_schema(&schemas, "ControlPlaneServiceSnapshot");
    let control_plane_properties = contract_object(control_plane_service.get("properties"), "ControlPlaneServiceSnapshot properties");
    let control_plane_required = schema_string_field_set(control_plane_service, "required", "ControlPlaneServiceSnapshot");
    member_contracts! { control_plane_properties; Present => ["service_lease"]; }
    assert!(control_plane_required.contains("service_lease"));
    for retired in contract_words("quota_class service_lease_status lease_expires_height lease_expires_sequence prepaid_runtime_balance remaining_runtime_balance") {
        member_contracts! { control_plane_properties; Absent => [retired]; }
        assert!(!control_plane_required.contains(retired));
    }
}
#[test]
fn bridge_finality_operations_describe_current_durable_evidence() {
    let document = generate_spec();
    for contract in response_rows! {
        "/v1/bridge/finality/{height}", "get", "200", "#/components/schemas/SumeragiFinalityProof";
        "/v1/bridge/finality/attestation/{height}", "get", "200", "#/components/schemas/SumeragiFinalityAttestation";
        "/v1/bridge/finality/attestation/latest", "get", "200", "#/components/schemas/SumeragiFinalityAttestation";
        "/v1/bridge/finality/bundle/{height}", "get", "200", "#/components/schemas/SumeragiFinalityBundle";
    } {
        let operation = openapi_operation(&document, contract.path, contract.method);
        let description = contract_text(operation.get("description"), "bridge description");
        assert!(description.contains("current") && description.contains("durable"));
        assert!(!description.contains("Sumeragi-v2") && !description.contains("height-context id"));
        text_contracts! { operation_response_schema_ref(operation, "200", contract.path) => contract.schema_ref; }
        let norito = contract_object(response_content(operation, "200").get("application/x-norito").and_then(|media| media.get("schema")), "Norito response schema");
        scalar_contracts! { norito.get("type") => Text("string"); norito.get("format") => Text("binary"); }
    }
    for path in ["/v1/bridge/finality/attestation/{height}", "/v1/bridge/finality/attestation/latest"] {
        let operation = openapi_operation(&document, path, "get");
        let challenge = operation_parameter(operation, "X-Iroha-Finality-Challenge");
        scalar_contracts! { challenge.get("in") => Text("header"); challenge.get("required") => Flag(true); }
        for status in contract_words("200 400 404 406 409 503") {
            let headers = contract_object(operation_responses(operation).get(status).and_then(|response| response.get("headers")), &format!("{status} headers"));
            let constant = |name| headers.get(name).and_then(|header| header.get("schema")).and_then(|schema| schema.get("const")).and_then(Value::as_str);
            assert_eq!(constant("Cache-Control"), Some("no-store"));
            assert_eq!(constant("Vary"), Some("X-Iroha-Finality-Challenge, Accept"));
            assert_eq!(constant("X-Content-Type-Options"), Some("nosniff"));
        }
    }
    let latest = openapi_operation(&document, "/v1/bridge/finality/attestation/latest", "get");
    let parameters = contract_array(latest.get("parameters"), "latest attestation parameters");
    assert!(parameters.iter().all(|parameter| parameter.get("in").and_then(Value::as_str) != Some("path")), "latest selects the durable tip without a height parameter");
    let height = openapi_operation(&document, "/v1/bridge/finality/attestation/{height}", "get");
    assert_eq!(operation_responses(latest).keys().collect::<BTreeSet<_>>(), operation_responses(height).keys().collect::<BTreeSet<_>>(), "both selectors share one response contract");
}
#[test]
fn signed_status_documents_actual_driver_fields() {
    use iroha_data_model::sumeragi::{SumeragiStatus, SumeragiFootprint, SumeragiHaltReason};
    let status = SumeragiStatus {
        instance: [7; 32], height: 2, view: 0, stage: 0, leader: None, proxy_tail: None,
        high_qc_view: None, level: 0, start_level: 0, t_retx_ms: 500,
        committed_height: 1, applied_height: 1, awaiting: false, signer: None,
        unanchored: false, abstaining: true, halted: Some(SumeragiHaltReason::DriverAnomaly),
        footprint: SumeragiFootprint::default(),
    };
    let native = norito::json::to_value(&status).expect("current status JSON");
    let schemas = openapi_schemas();
    let schema = contract_schema(&schemas, "SumeragiStatusResponse");
    let properties = contract_object(schema.get("properties"), "current status properties");
    assert_eq!(object_field_set(native.as_object().expect("current status object")), object_field_set(properties));
    member_contracts! { properties; Absent => contract_strings("status.absent"); }
    assert_eq!(native.get("instance").and_then(Value::as_str), Some("07".repeat(32).as_str()));
    let footprint = native.get("footprint").and_then(Value::as_object).unwrap();
    assert_eq!(object_field_set(footprint), object_field_set(contract_object(contract_schema(&schemas, "SumeragiFootprint").get("properties"), "footprint properties")));
}

#[test]
fn current_finality_schemas_match_portable_wire_bounds() {
    let schemas = openapi_schemas();
    let header = contract_schema(&schemas, "BlockHeader");
    assert!(schema_string_field_set(header, "required", "current block header").contains("global_beacon_pulse_hash"));
    let pulse_hash = contract_property(&schemas, "BlockHeader", "global_beacon_pulse_hash");
    assert!(pulse_hash.get("anyOf").and_then(Value::as_array).is_some_and(|alternatives|
        alternatives.iter().any(|value| value.get("$ref").and_then(Value::as_str) == Some("#/components/schemas/Hash"))
        && alternatives.iter().any(|value| value.get("type").and_then(Value::as_str) == Some("null"))));
    let proof = contract_schema(&schemas, "SumeragiFinalityProof");
    scalar_contracts! { proof.get("additionalProperties") => Flag(false); }
    assert_eq!(schema_string_field_set(proof, "required", "proof"), contract_words("block_header block_wire committee").into_iter().collect());
    scalar_contracts! { contract_property(&schemas, "SumeragiFinalityProof", "block_wire").get("maxItems") => Unsigned(iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES as u64); }
    scalar_contracts! { contract_property(&schemas, "SumeragiFinalityProof", "committee").get("maxItems") => Unsigned(iroha_sumeragi::types::MAX_COMMITTEE_SIZE as u64); }
    let body = contract_schema(&schemas, "SumeragiFinalityAttestationBody");
    assert_eq!(schema_string_field_set(body, "required", "attestation body"), contract_words("challenge network_id node_id node_fingerprint build_fingerprint config_fingerprint genesis_block_hash genesis_finality_proof status finality_proof").into_iter().collect());
    property_refs!(&schemas;
        "SumeragiFinalityAttestation", "body", "#/components/schemas/SumeragiFinalityAttestationBody";
        "SumeragiFinalityAttestationBody", "status", "#/components/schemas/SumeragiStatusResponse";
        "SumeragiFinalityAttestationBody", "finality_proof", "#/components/schemas/SumeragiFinalityProof";
    );
    for retired in ["BridgeFinalityAttestationBodyV1", "BridgeFinalityAttestationV1", "BridgeFinalityBundle"] {
        assert!(!schemas.contains_key(retired), "retired HTTP schema {retired}");
    }
}
#[test]
fn generated_spec_documents_exact_current_sumeragi_status() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    for (path, expected, label) in contract_rows! { "/v1/sumeragi/status", "#/components/schemas/SumeragiStatusResponse", "authoritative status"; "/v1/sumeragi/diagnostics", "#/components/schemas/SumeragiDiagnosticsResponse", "operator diagnostics"; } {
        let actual = document.get("paths").and_then(Value::as_object).and_then(|paths| paths.get(path)).and_then(Value::as_object).and_then(|item| item.get("get")).and_then(Value::as_object).map(|operation| operation_response_schema_ref(operation, "200", path));
        assert_eq!(actual, catalog_openapi_route_enabled(CatalogHttpMethod::Get, path).then_some(expected), "{label} catalog projection");
    }
    assert_exact_closed_required_schema_fields(schemas, "SumeragiStatusResponse", &contract_strings("status.required"));
    let properties = contract_object(contract_schema(schemas, "SumeragiStatusResponse").get("properties"), "status properties");
    member_contracts! { properties; Absent => contract_strings("status.absent"); }
    scalar_contracts! { properties.get("footprint").and_then(|schema| schema.get("$ref")) => Text("#/components/schemas/SumeragiFootprint"); }
    let diagnostics = contract_schema(schemas, "SumeragiDiagnosticsResponse");
    scalar_contracts! { diagnostics.get("additionalProperties") => Flag(false); }
    let diagnostics = contract_object(diagnostics.get("properties"), "diagnostics properties");
    member_contracts! { diagnostics; Present => ["npos"]; }
    for (field, expected) in contract_rows! {
        "lane_commitments", "#/components/schemas/SumeragiLaneCommitment";
        "dataspace_commitments", "#/components/schemas/SumeragiDataspaceCommitment";
        "lane_payload_ownerships", "#/components/schemas/SumeragiLanePayloadOwnership";
        "committed_lane_blocks", "#/components/schemas/SumeragiCommittedLaneBlock";
        "lane_block_sessions", "#/components/schemas/SumeragiLaneBlockSessionStatus";
        "lane_governance", "#/components/schemas/SumeragiLaneGovernance";
    } {
        scalar_contracts! { diagnostics.get(field).and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")) => Text(expected); }
    }
    member_contracts! { diagnostics; Absent => contract_words("height view phase leader locked_prepare_qc"); }
    let settlement = contract_schema(schemas, "LaneSettlementCommitment");
    let settlement_properties = contract_object(settlement.get("properties"), "settlement properties");
    scalar_contracts! {
        settlement_properties.get("native_amx_receipts").and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")) => Text("#/components/schemas/NativeAmxReceipt");
        settlement_properties.get("native_amx_receipts").and_then(|schema| schema.get("maxItems")) => Unsigned(4_096);
        settlement_properties.get("nexus_fee_receipts").and_then(|schema| schema.get("items")).and_then(|items| items.get("$ref")) => Text("#/components/schemas/NexusFeeReceipt");
        settlement.get("additionalProperties") => Flag(false);
    }
    assert!(schema_fields(settlement, "required", "settlement").iter().any(|field| field.as_str() == Some("lane_incarnation")));
    let receipt = contract_schema(schemas, "NativeAmxReceipt");
    assert_required_inventory(receipt, "native.receipt.required");
    let legs = contract_property(schemas, "NativeAmxReceipt", "legs");
    assert_item_ref(legs, "#/components/schemas/NativeAmxLegRecord");
    assert_array_bounds(legs, 1, 255, Some(true));
    let leg_properties = contract_object(contract_schema(schemas, "NativeAmxLegRecord").get("properties"), "leg properties");
    member_contracts! { leg_properties; Absent => ["lane_incarnation"]; }
    assert_eq!(component_required(schemas, "NativeAmxLegRecord"), contract_strings("native.leg.required"));
    property_refs!(schemas;
        "NativeAmxLegRecord", "participant_proposal", "#/components/schemas/NativeAmxParticipantLaneBlockProposal";
        "NativeAmxLegRecord", "participant_settlement", "#/components/schemas/NativeAmxParticipantSettlement";
        "NativeAmxLegRecord", "participant_settlement_hash", "#/components/schemas/Hash";
        "NativeAmxLegRecord", "prepare_qc", "#/components/schemas/NativeAmxAttestationQc";
        "NativeAmxLegRecord", "commit_qc", "#/components/schemas/NativeAmxAttestationQc";
        "NativeAmxAttestationQc", "body", "#/components/schemas/NativeAmxAttestationBody";
    );
    let proposal = contract_schema(schemas, "NativeAmxParticipantLaneBlockProposal");
    scalar_contracts! { proposal.get("additionalProperties") => Flag(false); }
    set_contracts! { schema_fields(proposal, "required", "native AMX participant proposal").iter().filter_map(Value::as_str).collect::<BTreeSet<_>>() => contract_strings("native.proposal.required").into_iter().collect::<BTreeSet<_>>(); }
    scalar_contracts! { contract_property(schemas, "NativeAmxParticipantLaneBlockProposal", "payload_block_hint").get("type") => Text("null"); }
    let proposal_description = contract_text(proposal.get("description"), "native AMX participant proposal description");
    assert!(proposal_description.contains("requires payload_block_hint to be present as null"));
    let participant = contract_schema(schemas, "NativeAmxParticipantSettlement");
    scalar_contracts! { participant.get("additionalProperties") => Flag(false); }
    let expected_fields = contract_words(concat!("lane_id dataspace_id lane_incarnation participant_lane_block_height ", "authority_context_height previous_native_settlement_hash source_ids")).into_iter().collect::<BTreeSet<_>>();
    set_contracts! { schema_fields(participant, "required", "native participant settlement").iter().filter_map(Value::as_str).collect::<BTreeSet<_>>() => expected_fields; participant.get("properties").and_then(Value::as_object).expect("participant properties").keys().map(String::as_str).collect::<BTreeSet<_>>() => expected_fields; }
    for (field, minimum, maximum) in contract_rows! { "lane_id", 0, u64::from(u32::MAX); "dataspace_id", 0, u64::MAX; "participant_lane_block_height", 1, u64::MAX; "authority_context_height", 1, u64::MAX; } {
        let property = contract_property(schemas, "NativeAmxParticipantSettlement", field);
        scalar_contracts! { property.get("minimum") => Unsigned(minimum); property.get("maximum") => Unsigned(maximum); }
    }
    let sources = contract_property(schemas, "NativeAmxParticipantSettlement", "source_ids");
    assert_array_bounds(sources, 1, 4_096, Some(true));
    scalar_contracts! { sources.get("items").and_then(|item| item.get("pattern")) => Text("^(?!0{64}$)[0-9A-F]{64}$"); }
    let incarnation = contract_property(schemas, "NativeAmxParticipantSettlement", "lane_incarnation");
    let alternatives = contract_array(incarnation.get("allOf"), "nonzero canonical incarnation hash");
    scalar_contracts! { alternatives[0].get("$ref") => Text("#/components/schemas/Hash"); alternatives[1].get("not").and_then(|schema| schema.get("pattern")) => Text("^hash:0{63}1#"); }
    let previous = contract_property(schemas, "NativeAmxParticipantSettlement", "previous_native_settlement_hash");
    let previous_variants = contract_array(previous.get("oneOf"), "required optional Native hash");
    count_contracts! { previous_variants.len() => 2; }
    scalar_contracts! { previous_variants[0].get("type") => Text("null"); }
    assert_eq!(previous_variants[1].get("allOf"), incarnation.get("allOf"));
    let rules = contract_array(participant.get("allOf"), "first-control rule");
    count_contracts! { rules.len() => 1; }
    scalar_contracts! {
        rules[0].get("if").and_then(|value| value.get("properties")).and_then(|value| value.get("participant_lane_block_height")).and_then(|value| value.get("const")) => Unsigned(1);
        rules[0].get("then").and_then(|value| value.get("properties")).and_then(|value| value.get("previous_native_settlement_hash")).and_then(|value| value.get("type")) => Text("null");
    }
    member_contracts! { schemas; Absent => ["NativeAmxParticipantSettlementCommitment"]; Absent => ["NativeAmxParticipantSettlementReceipt"]; }
    let qc = contract_object(contract_schema(schemas, "NativeAmxAttestationQc").get("properties"), "native AMX QC properties");
    for (field, minimum, maximum, unique, item) in contract_rows! { "validator_set", 1, 128, Some(true), "#/components/schemas/SumeragiV2BlsValidatorId"; "validator_set_pops", 1, 128, None, "#/components/schemas/SumeragiV2BlsProof"; } {
        let array = contract_object(qc.get(field), &format!("{field} schema"));
        assert_array_bounds(array, minimum, maximum, unique);
        assert_item_ref(array, item);
    }
    assert_array_bounds(contract_object(qc.get("signers_bitmap"), "signers bitmap"), 1, 16, None);
    scalar_contracts! { qc.get("bls_aggregate_signature").and_then(|schema| schema.get("$ref")) => Text("#/components/schemas/SumeragiV2BlsProof"); }
    for field in contract_words("accepted_candidate_indices accepted_transaction_hashes") {
        assert_array_bounds(contract_property(schemas, "NativeAmxParticipantLaneBlockDescriptor", field), 1, 4_096, Some(true));
    }
    let body = contract_schema(schemas, "NativeAmxAttestationBody");
    assert_required_inventory(body, "native.body.required");
    let body_required = schema_fields(body, "required", "native AMX body");
    string_members! { body_required; Absent => &["coordinator_lane_block_height"]; };
    for field in contract_words("participant_validator_count participant_min_quorum") {
        scalar_contracts! { contract_property(schemas, "NativeAmxAttestationBody", field).get("maximum") => Unsigned(128); }
    }
    scalar_contracts! { contract_property(schemas, "NativeAmxAttestationBody", "phase").get("$ref") => Text("#/components/schemas/NativeAmxPhase"); }
    let phase = contract_object(contract_schema(schemas, "NativeAmxPhase").get("properties"), "native phase properties");
    let phase_values = contract_array(phase.get("phase").and_then(|tag| tag.get("enum")), "phase enum");
    for value in contract_words("prepare commit") {
        string_members! { phase_values; Present => &[value]; };
    }
    scalar_contracts! { phase.get("detail").and_then(|detail| detail.get("type")) => Text("null"); }
    for (name, tag, expected) in contract_rows! { "LaneLiquidityProfile", "profile", &contract_words("Tier1 Tier2 Tier3")[..]; "LaneVolatilityClass", "bucket", &contract_words("Stable Elevated Dislocated"); } {
        let values = contract_array(contract_property(schemas, name, tag).get("enum"), "tag enum");
        for expected in expected {
            string_members! { values; Present => &[*expected]; };
        }
    }
    property_refs!(schemas;
        "LaneSwapMetadata", "liquidity_profile", "#/components/schemas/LaneLiquidityProfile";
        "LaneSwapMetadata", "volatility_class", "#/components/schemas/LaneVolatilityClass";
    );
    for field in contract_words("total_local_amount total_xor_due total_xor_after_haircut total_xor_variance") {
        scalar_contracts! { settlement_properties.get(field).and_then(|property| property.get("$ref")) => Text("#/components/schemas/Quantity"); }
    }
    member_contracts! { settlement_properties; Absent => contract_words(concat!("total_local_micro total_xor_due_micro total_xor_after_haircut_micro ", "total_xor_variance_micro")); }
    let receipt_properties = contract_object(contract_schema(schemas, "LaneSettlementReceipt").get("properties"), "settlement receipt properties");
    scalar_contracts! { receipt_properties.get("source_id").and_then(|schema| schema.get("pattern")) => Text("^[0-9A-F]{64}$"); }
    for (field, retired) in contract_rows! { "local_amount", "local_amount_micro"; "xor_due", "xor_due_micro"; "xor_after_haircut", "xor_after_haircut_micro"; "xor_variance", "xor_variance_micro"; } {
        scalar_contracts! { receipt_properties.get(field).and_then(|schema| schema.get("$ref")) => Text("#/components/schemas/Quantity"); }
        member_contracts! { receipt_properties; Absent => [retired]; }
    }
    for (owner, field) in contract_rows! { "NexusFeeReceipt", "lane_id"; "NativeAmxAttestationBody", "coordinator_lane_id"; "NativeAmxAttestationBody", "participant_lane_id"; "NativeAmxLegRecord", "lane_id"; "NativeAmxReceipt", "lane_id"; "LaneSettlementCommitment", "lane_id"; "LaneRelayEnvelope", "lane_id"; } {
        scalar_contracts! { contract_property(schemas, owner, field).get("maximum") => Unsigned(u64::from(u32::MAX)); }
    }
}
#[test]
#[expect(clippy::too_many_lines, reason = "one cohesive exact Soracloud priority-contract inventory")]
fn generated_spec_documents_exact_soracloud_priority_contracts() {
    let document = canonical_document();
    let paths = contract_object(document.get("paths"), "paths");
    count_contracts! { paths.keys().filter(|path| path.starts_with("/v1/soracloud/")).count() => 51; }
    member_contracts! { paths; Absent => contract_words(concat!("/v1/soracloud/agent/autonomy/run /v1/soracloud/agent/autonomy/run/finalize ", "/v1/soracloud/model-host/advertise /v1/soracloud/model-host/heartbeat ", "/v1/soracloud/model-host/withdraw /v1/soracloud/model-host/status")); }
    for path in contract_words("/v1/soracloud/deploy /v1/soracloud/upgrade") {
        let operation = openapi_operation(&document, path, "post");
        text_contracts! { operation_request_schema_ref(operation, path) => "#/components/schemas/SignedBundleRequest"; operation_response_schema_ref(operation, "200", path) => "#/components/schemas/SoracloudMutationDraftResponse"; }
    }
    let schemas = component_schemas(&document);
    member_contracts! { schemas; Absent => contract_words("AgentAutonomyRunPayload SignedAgentAutonomyRunRequest AgentAutonomyFinalizeRequest"); }
    inline_shapes! { schemas; "SignedBundleRequest", &contract_words("bundle initial_service_configs initial_service_secrets provenance"), &[]; }
    let status_operation = openapi_operation(&document, "/v1/soracloud/status", "get");
    text_contracts! { operation_response_schema_ref(status_operation, "200", "/v1/soracloud/status") => "#/components/schemas/SoracloudStatusV1"; }
    let status = contract_text(status_operation.get("description"), "Soracloud status description");
    for phrase in ["configured_lane_count", "declared_lane_count", "active lane ids/count", "autoscale-capacity lane ids/count"] {
        assert!(status.contains(phrase));
    }
    let join = openapi_operation(&document, "/v1/soracloud/hf/lease/join", "post");
    text_contracts! { operation_request_schema_ref(join, "/v1/soracloud/hf/lease/join") => "#/components/schemas/SignedHfSharedLeaseJoinRequest"; operation_response_schema_ref(join, "200", "/v1/soracloud/hf/lease/join") => "#/components/schemas/SoracloudMutationDraftResponse"; }
    let headers = operation_parameters(join);
    for name in contract_strings("hf.headers") {
        assert!(headers.iter().any(|parameter| parameter.get("name").and_then(Value::as_str) == Some(name)), "HF shared-lease join missing {name}");
    }
    inline_shapes! { schemas; "SignedHfSharedLeaseJoinRequest", &contract_words("payload provenance"), &[]; "HfSharedLeaseJoinPayload", &contract_words(concat!("repo_id revision service_name apartment_name storage_class lease_term_ms ", "lease_asset_definition_id base_fee")), &[]; }
    let apartment_name = contract_array(contract_property(schemas, "HfSharedLeaseJoinPayload", "apartment_name").get("anyOf"), "HF apartment_name nullable union");
    count_contracts! { apartment_name.len() => 2; }
    scalar_contracts! { apartment_name[0].get("type") => Text("string"); apartment_name[1].get("type") => Text("null"); }
    inline_shapes! { schemas; "SoracloudMutationDraftResponse", &contract_words("ok authority signed_by tx_instructions"), &[]; }
    assert_item_ref(contract_property(schemas, "SoracloudMutationDraftResponse", "tx_instructions"), "#/components/schemas/SoracloudTxInstruction");
    scalar_contracts! { contract_property(schemas, "SoracloudMutationDraftResponse", "tx_instructions").get("minItems") => Unsigned(1); contract_property(schemas, "SoracloudTxInstruction", "payload_hex").get("pattern") => Text("^(?:[0-9a-f]{2})+$"); }
    let wallet_spend = openapi_operation(&document, "/v1/soracloud/agent/wallet/spend", "post");
    text_contracts! { operation_request_schema_ref(wallet_spend, "agent wallet spend") => "#/components/schemas/SignedAgentWalletSpendRequest"; }
    inline_shapes! { schemas; "AgentWalletSpendPayload", &contract_words("apartment_name request_id asset_definition amount"), &[]; "SoracloudStatusV1", &contract_words(concat!("schema_version service_health routing hosted_http_topology resource_pressure ", "failed_admissions runtime_manager control_plane")), &[]; }
    let topology = contract_property(schemas, "SoracloudStatusV1", "hosted_http_topology");
    scalar_contracts! { topology.get("additionalProperties") => Flag(false); }
    let topology_fields = contract_word_set("active_capability_adverts hosted_replica_count placed_host_count unavailable_replica_count");
    assert_eq!(object_field_set(topology.get("properties").and_then(Value::as_object).expect("Soracloud hosted HTTP topology properties")), topology_fields);
    set_contracts! { schema_fields(topology, "required", "Soracloud hosted HTTP topology").iter().map(|field| field.as_str().expect("topology field")).collect::<BTreeSet<_>>() => topology_fields; }
    for field in topology_fields {
        let property = contract_object(topology.get("properties").and_then(Value::as_object).and_then(|properties| properties.get(field)), &format!("hosted topology {field}"));
        scalar_contracts! { property.get("type") => Text("integer"); property.get("format") => Text("uint64"); property.get("minimum") => Unsigned(0); }
    }
    let routing = contract_property(schemas, "SoracloudStatusV1", "routing");
    scalar_contracts! { routing.get("additionalProperties") => Flag(false); }
    let routing_properties = contract_object(routing.get("properties"), "Soracloud status routing properties");
    let routing_fields = contract_word_set("configured_lane_count declared_lane_count active_lane_count active_lane_ids autoscale_capacity_lane_count autoscale_capacity_lane_ids dataspace_count routing_rules default_lane_id default_dataspace_id");
    assert_eq!(object_field_set(routing_properties), routing_fields);
    set_contracts! { schema_fields(routing, "required", "Soracloud status routing").iter().map(|field| field.as_str().expect("routing field")).collect::<BTreeSet<_>>() => routing_fields; }
    text_contracts! { property_ref(schemas, "SoracloudStatusV1", "runtime_manager") => "#/components/schemas/SoracloudStatusRuntimeManagerV1"; }
    assert_exact_closed_required_schema_fields(schemas, "ControlPlaneAuditEvent",
        &contract_words(concat!("sequence action service_name from_version to_version service_manifest_hash ", "container_manifest_hash process_generation config_generation secret_generation ", "config_snapshot_hash secret_snapshot_hash binding_name state_key ",
            "config_mutations secret_mutations governance_tx_hash rollout_state policy_name ", "policy_snapshot_hash jurisdiction_tag consent_evidence_hash break_glass ", "break_glass_reason lease_usage service_lease_commitment ", "lease_reporting_epoch_rollover signed_by")));
    text_contracts! { nullable_property_ref(schemas, "ControlPlaneAuditEvent", "lease_reporting_epoch_rollover") => "#/components/schemas/SoraServiceLeaseReportingEpochRolloverV1"; }
    assert_exact_closed_required_schema_fields(schemas, "SoraServiceLeaseReportingEpochRolloverV1", &contract_words(concat!("schema_version economic_clock lease_started_height previous_reporting_epoch ", "new_reporting_epoch reporter_account_id active_service_version replica_slot ", "placement_incarnation finalized_checkpoint_count settled_egress_bytes_delta ", "settled_egress_bytes")));
    text_contracts! { property_ref(schemas, "SoraServiceLeaseReportingEpochRolloverV1", "economic_clock") => "#/components/schemas/SoraServiceLeaseClockV1"; property_ref(schemas, "SoraServiceLeaseReportingEpochRolloverV1", "placement_incarnation") => "#/components/schemas/Hash"; }
    let action_variants = contract_array(contract_schema(schemas, "SoracloudAction").get("oneOf"), "SoracloudAction variants").iter()
        .map(|variant| contract_text(variant.get("properties").and_then(Value::as_object).and_then(|properties| properties.get("action")).and_then(Value::as_object).and_then(|action| action.get("const")), "SoracloudAction const")).collect::<BTreeSet<_>>();
    set_contracts! { action_variants => contract_word_set("CiphertextQuery ConfigMutation DecryptionRequest Deploy FheJobRun FhePolicyRegister FhePolicyRevoke FhePolicyRotate LeaseUsage LeaseReportingEpochRollover Rollback Rollout SecretMutation StateMutation Upgrade"); }
    for (path, method, request, response) in contract_rows! { "/v1/soracloud/agent/autonomy/allow", "post", Some("#/components/schemas/SignedAgentArtifactAllowRequest"), "#/components/schemas/SoracloudMutationDraftResponse"; "/v1/soracloud/agent/autonomy/status", "get", None, "#/components/schemas/AgentAutonomyStatusResponse"; } {
        let operation = openapi_operation(&document, path, method);
        if let Some(request) = request {
            text_contracts! { operation_request_schema_ref(operation, path) => request; }
        }
        text_contracts! { operation_response_schema_ref(operation, "200", path) => response; }
    }
    inline_shapes! { schemas; "SignedAgentArtifactAllowRequest", &contract_words("payload provenance"), &[]; "AgentArtifactAllowPayload", &contract_words("apartment_name artifact_hash provenance_hash"), &[]; }
    assert_strict_object_schema(
        schemas,
        "AgentAutonomyStatusResponse",
        &contract_words(concat!("apartment_name sequence status lease_expires_height lease_remaining_blocks ", "manifest_hash revoked_policy_capability_count budget_ceiling_units ", "budget_remaining_units allowlist_count run_count process_generation ",
            "process_started_sequence last_active_sequence last_checkpoint_sequence ", "checkpoint_count persistent_state_total_bytes persistent_state_key_count ", "allowlist recent_runs")),
        &[],
    );
}
#[test]
fn generated_spec_documents_app_query_page_metadata() {
    let document = generate_spec();
    assert_operation_response_contracts(
        &document,
        &response_rows! {
        "/v1/accounts", "get", "200", "#/components/schemas/AccountListResponse";
        "/v1/accounts/query", "post", "200", "#/components/schemas/AccountQueryResponse";
        "/v1/domains", "get", "200", "#/components/schemas/DomainListResponse";
        "/v1/domains/query", "post", "200", "#/components/schemas/DomainQueryResponse";
        "/v1/accounts/{account_id}/assets", "get", "200", "#/components/schemas/AccountAssetListResponse";
        "/v1/accounts/{account_id}/assets/query", "post", "200", "#/components/schemas/AccountAssetQueryResponse";
        "/v1/assets/definitions", "get", "200", "#/components/schemas/AssetDefinitionListResponse";
        "/v1/assets/definitions/query", "post", "200", "#/components/schemas/AssetDefinitionQueryResponse";
        "/v1/assets/{definition_id}/holders", "get", "200", "#/components/schemas/AssetHolderListResponse";
        "/v1/assets/{definition_id}/holders/query", "post", "200", "#/components/schemas/AssetHolderQueryResponse";
        "/v1/nfts", "get", "200", "#/components/schemas/NftListResponse";
        "/v1/nfts/query", "post", "200", "#/components/schemas/NftQueryResponse";
        "/v1/rwas", "get", "200", "#/components/schemas/RwaListResponse";
        "/v1/rwas/query", "post", "200", "#/components/schemas/RwaQueryResponse";
        "/v1/repo/agreements", "get", "200", "#/components/schemas/RepoAgreementListResponse";
        "/v1/repo/agreements/query", "post", "200", "#/components/schemas/RepoAgreementListResponse";
    },
    );
    let schemas = component_schemas(&document);
    let metadata = contract_schema(schemas, "AppPageMetadata");
    assert_required_inventory(metadata, "app.page.required");
    let required = schema_fields(metadata, "required", "app page metadata");
    string_members! { required; Absent => &["total"]; };
    let properties = contract_object(metadata.get("properties"), "metadata properties");
    member_contracts! { properties; Present => contract_strings("app.page.properties"); }
    let repo = contract_schema(schemas, "RepoAgreement");
    let repo_required = schema_fields(repo, "required", "repo agreement");
    let repo_properties = contract_object(repo.get("properties"), "repo properties");
    for field in contract_strings("repo.agreement.fields") {
        string_members! { repo_required; Present => &[field]; };
        member_contracts! { repo_properties; Present => [field]; }
    }
    let query = contract_object(contract_schema(schemas, "RepoAgreementsQueryRequest").get("properties"), "repo query properties");
    member_contracts! { query; Present => contract_strings("repo.query.fields"); }
}
#[test]
fn alias_openapi_documents_optional_public_and_exact_restricted_auth() {
    let document = generate_spec();
    for path in contract_words("/v1/aliases/resolve-index /v1/aliases/by-account") {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_alias_auth_required_response(operation, path);
        assert!(operation_responses(operation).contains_key("403"));
    }
    let lookup = contract_text(openapi_operation(&document, "/v1/aliases/by-account", "post").get("description"), "alias description");
    assert!(lookup.contains("Canonical authentication is optional") && lookup.contains("required for restricted data"));
    let resolve = openapi_operation(&document, "/v1/aliases/resolve", "post");
    assert_eq!(operation_header_requirements(resolve), canonical_account_header_requirements(false));
    assert_alias_auth_required_response(resolve, "/v1/aliases/resolve");
    assert!(resolve.get("description").and_then(Value::as_str).is_some_and(|text| text.contains("Public dataspaces may be read unsigned")));
    assert!(operation_responses(resolve).contains_key("403"));
    for path in contract_words("/v1/aliases/setup/plan /v1/aliases/lease/renew/plan /v1/aliases/auto-renew/plan") {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_alias_auth_required_response(operation, path);
        assert!(operation_responses(operation).contains_key("409"));
    }
    for (path, reject, forbidden) in contract_rows! { "/v1/retail/recipients/lookup", "recipient_lookup_signature_required", true; "/v1/retail/recipients/route", "recipient_lookup_signature_required", true; "/v1/fee-sponsor-programs/by-id", "fee_sponsor_program_signature_required", false; "/v1/fees/quote", "fee_quote_signature_required", false; } {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, reject);
        assert_eq!(operation_responses(operation).contains_key("403"), forbidden);
    }
}
#[test]
fn protected_contract_identity_openapi_is_signed_and_exact() {
    let document = generate_spec();
    for (path, method, response_schema, reject, statuses) in contract_rows! {
        "/v1/contracts/aliases/resolve", "post", "#/components/schemas/ContractAliasResolveResponse", "alias_auth_required", &contract_words("200 400 401 404 429 500")[..];
        "/v1/gov/contracts/{contract_address}", "get", "#/components/schemas/GovernedContractResponse", "contract_code_auth_required", &contract_words("200 400 401 404 429 500");
        "/v1/contracts/code-bytes/{code_hash}", "get", "#/components/schemas/JsonValue", "contract_code_auth_required", &contract_words("200 400 401 404 429");
    } {
        let operation = openapi_operation(&document, path, method);
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, reject);
        text_contracts! { operation_response_schema_ref(operation, "200", path) => response_schema; }
        set_contracts! { operation_responses(operation).keys().map(String::as_str).collect::<BTreeSet<_>>() => statuses.iter().copied().collect::<BTreeSet<_>>(); }
    }
    text_contracts! { operation_request_schema_ref(openapi_operation(&document, "/v1/contracts/aliases/resolve", "post"), "/v1/contracts/aliases/resolve") => "#/components/schemas/ContractAliasResolveRequest"; }
    let schemas = component_schemas(&document);
    schema_shapes!(schemas;
        "ContractAliasResolveRequest", "contract.alias.request.required", None;
        "ContractAliasBinding", "contract.alias.binding.required", Some("contract.alias.binding.optional");
        "ContractAliasResolveResponse", "contract.alias.response.required", None;
    );
    let variants = contract_array(contract_schema(schemas, "GovernedContractResponse").get("oneOf"), "governed variants");
    count_contracts! { variants.len() => 3; }
    for (variant, inventory) in variants.iter().zip(["governed.found.fields", "governed.inactive.fields", "governed.missing.fields"]) {
        let variant = variant.as_object().expect("governed variant");
        assert_eq!(variant.get("additionalProperties"), Some(&Value::Bool(false)));
        let expected = asset_field_set(inventory);
        set_contracts! { schema_fields(variant, "required", "governed variant").iter().filter_map(Value::as_str).collect::<BTreeSet<_>>() => expected; }
        assert_eq!(variant.get("properties").and_then(Value::as_object).map(object_field_set), Some(expected));
    }
    for (schema, expected) in contract_rows! {
        "GovernedContractLifecycleV1", contract_words(concat!("version origin origin_account origin_proposal_content_id_hex ", "origin_governance_attempt_id_hex owner pending_owner parliament_delegated ", "active_code_hash_hex revision emergency_hold")) .into_iter() .collect::<BTreeSet<_>>();
        "GovernedContractEmergencyHoldV1", contract_words(concat!("incident_digest_hex proposal_content_id_hex governance_attempt_id_hex reason ", "imposed_at_height expires_at_height")) .into_iter() .collect::<BTreeSet<_>>();
    } {
        let schema = contract_schema(schemas, schema);
        assert_eq!(schema.get("additionalProperties"), Some(&Value::Bool(false)));
        set_contracts! { schema_fields(schema, "required", "governed lifecycle component") .iter() .filter_map(Value::as_str) .collect::<BTreeSet<_>>() => expected; }
    }
}
#[test]
fn multisig_read_auth_contract_is_path_specific() {
    let document = generate_spec();
    for path in contract_words("/v1/multisig/spec /v1/multisig/proposals/query /v1/multisig/proposals/resolve") {
        let operation = openapi_operation(&document, path, "post");
        assert_eq!(operation_header_requirements(operation), canonical_account_header_requirements(false));
        assert_canonical_auth_required_response(operation, path, "multisig_read_auth_required");
        let description = contract_text(operation.get("description"), "multisig read description");
        for phrase in ["Both canonical `multisig_account_id`", "`multisig_account_alias` selectors require", "body fields never establish signer identity"] {
            assert!(description.contains(phrase));
        }
    }
    for path in contract_words(concat!("/v1/multisig/propose /v1/multisig/approve /v1/multisig/cancel ", "/v1/contracts/call/multisig/propose /v1/contracts/call/multisig/approve")) {
        assert!(operation_header_requirements(openapi_operation(&document, path, "post")).is_empty(), "POST {path} body-authenticated write contract");
    }
}
}
