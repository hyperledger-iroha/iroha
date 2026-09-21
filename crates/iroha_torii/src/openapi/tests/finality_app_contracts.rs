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
fn sumeragi_v2_da_schema_requires_reed_solomon16_without_plain_compatibility() {
    let schemas = openapi_schemas();
    let encoding = contract_schema(&schemas, "SumeragiV2PayloadEncoding");
    let allowed = contract_array(contract_property(&schemas, "SumeragiV2PayloadEncoding", "encoding").get("enum"), "payload encoding enum");
    sequence_contracts! { allowed => [Value::from("reed_solomon16")]; }
    string_members! { allowed; Absent => &["plain"]; };
    let layout = contract_schema(&schemas, "SumeragiV2DataAvailabilityLayout");
    assert!(layout.get("oneOf").is_none() && layout.get("anyOf").is_none(), "RS16 layout compatibility union");
    assert_required_inventory(layout, "sumeragi.da.required");
    scalar_contracts! { contract_property(&schemas, "SumeragiV2DataAvailabilityLayout", "encoding").get("$ref") => Text("#/components/schemas/SumeragiV2PayloadEncoding"); }
    for field in contract_words("data_shards parity_shards") {
        scalar_contracts! { contract_property(&schemas, "SumeragiV2DataAvailabilityLayout", field).get("minimum") => Unsigned(1); }
    }
    let serialized = norito::json::to_string(&Value::Object(layout.clone())).expect("serialize DA layout");
    assert!(!serialized.contains("\"plain\""), "retired Plain layout branch");
    member_contracts! { encoding; Present => ["properties"]; }
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
fn bridge_finality_v2_schemas_are_exact_closed_and_bounded() {
    let schemas = openapi_schemas();
    let max_validators = u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound");
    schema_shapes!(&schemas;
        "BridgeFinalityProof", "bridge.proof.required", None;
        "BridgeFinalityAttestationV1", "bridge.attestation.required", None;
        "SumeragiV2FinalityArtifact", "finality.artifact.required", None;
        "SumeragiV2HeightContext", "height.context.required", None;
        "SumeragiV2ValidatorPower", "validator.power.required", None;
        "SumeragiV2DualQuorum", "dual.quorum.required", None;
        "SumeragiV2BlockSubject", "block.subject.required", None;
        "SumeragiV2MergeCarrierCommitment", "merge.carrier.required", None;
        "SumeragiV2ExecutionCommitment", "execution.required", None;
        "SumeragiV2QuorumCertificate", "qc.required", None;
        "SumeragiV2CommitQuorumCertificate", "qc.required", None;
        "SumeragiV2SnapshotBootstrapAnchor", "snapshot.bootstrap.required", None;
        "SumeragiV2FinalizedNextEpochSnapshot", "next.epoch.required", None;
        "BridgeCommitment", "bridge.commitment.required", None;
        "BridgeFinalityBundle", "bridge.bundle.required", None;
        "BlockHeader", "block.header.required", None;
    );
    for (owner, inventory) in contract_rows! {
        "SumeragiV2HeightContext", "height.context.nullable";
        "SumeragiV2BlockSubject", "block.subject.nullable";
        "SumeragiV2ExecutionCommitment", "execution.nullable";
        "BlockHeader", "block.header.nullable";
    } {
        for field in contract_strings(inventory) { let _ = nullable_property_ref(&schemas, owner, field); }
    }
    assert_eq!(contract_property(&schemas, "BridgeFinalityProof", "version").get("enum").and_then(Value::as_array), Some(&vec![Value::from(2_u64)]));
    for (owner, field) in contract_rows! { "SumeragiV2FinalityArtifact", "format_version"; "SumeragiV2FinalityArtifact", "protocol_version"; "SumeragiV2HeightContext", "protocol_version"; } {
        assert_eq!(contract_property(&schemas, owner, field).get("enum").and_then(Value::as_array), Some(&vec![Value::from(4_u64)]), "{owner}.{field} version");
    }
    let roster = contract_property(&schemas, "SumeragiV2HeightContext", "roster");
    assert_array_bounds(roster, 4, max_validators, Some(true));
    assert_item_ref(roster, "#/components/schemas/SumeragiV2ValidatorPower");
    assert_eq!(contract_property(&schemas, "SumeragiV2ValidatorPower", "power").get("enum").and_then(Value::as_array), Some(&vec![Value::from(1_u64)]));
    property_refs!(&schemas;
        "SumeragiV2ValidatorPower", "validator", "#/components/schemas/SumeragiV2BlsValidatorId";
        "SumeragiV2MergeCarrierCommitment", "entry_hash", "#/components/schemas/Hash";
        "SumeragiV2FinalityArtifact", "commit_qc", "#/components/schemas/SumeragiV2CommitQuorumCertificate";
        "SumeragiV2QuorumCertificate", "execution_commitment", "#/components/schemas/SumeragiV2ExecutionCommitment";
        "SumeragiV2CommitQuorumCertificate", "execution_commitment", "#/components/schemas/SumeragiV2ExecutionCommitment";
    );
    scalar_contracts! { contract_schema(&schemas, "SumeragiV2BlsValidatorId").get("pattern") => Text("^ea0130[0-9A-F]{96}$"); }
    for owner in contract_words("SumeragiV2FinalityArtifact SumeragiV2FinalizedNextEpochSnapshot") {
        let pops = contract_property(&schemas, owner, "validator_set_pops");
        assert_array_bounds(pops, 4, max_validators, None);
        assert_item_ref(pops, "#/components/schemas/SumeragiV2BlsProof");
    }
    let proof = contract_schema(&schemas, "SumeragiV2BlsProof");
    assert_array_bounds(proof, 96, 96, None);
    let byte = contract_object(proof.get("items"), "BLS proof byte");
    scalar_contracts! { byte.get("minimum") => Unsigned(0); byte.get("maximum") => Unsigned(255); }
    for owner in contract_words("SumeragiV2QuorumCertificate SumeragiV2CommitQuorumCertificate") {
        assert_array_bounds(contract_property(&schemas, owner, "signers"), 1, max_validators, Some(true));
    }
    assert_array_bounds(contract_property(&schemas, "SumeragiV2FinalizedNextEpochSnapshot", "roster"), 4, max_validators, None);
    scalar_contracts! { contract_property(&schemas, "SumeragiV2DualQuorum", "min_signers").get("maximum") => Unsigned(max_validators); contract_property(&schemas, "SumeragiV2MergeCarrierCommitment", "version").get("const") => Unsigned(1); contract_property(&schemas, "SumeragiV2ExecutionCommitment", "merge_carrier").get("oneOf") => Length(2); }
    let wire_len = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "executed_block_wire_len");
    scalar_contracts! { wire_len.get("format") => Text("uint64"); wire_len.get("minimum") => Unsigned(1); }
    let topups = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "kagemusha_top_up_count");
    scalar_contracts! { topups.get("minimum") => Unsigned(0); }
    assert!(topups.get("maximum").is_none(), "block bytes, not a special KAGEMUSHA count cap, bound top-ups");
    let execution = contract_schema(&schemas, "SumeragiV2ExecutionCommitment");
    scalar_contracts! { execution.get("oneOf") => Length(2); contract_property(&schemas, "SumeragiV2ExecutionCommitment", "native_amx_application_manifest_version").get("const") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION)); }
    let manifest_count = contract_property(&schemas, "SumeragiV2ExecutionCommitment", "native_amx_application_manifest_count");
    scalar_contracts! { manifest_count.get("minimum") => Unsigned(0); manifest_count.get("maximum") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES)); }
    assert_exact_closed_required_schema_fields( &schemas, "SumeragiV2LaneFinalityManifestCommitment", &contract_words("root leaf_count"), );
    text_contracts! { property_ref(&schemas, "SumeragiV2LaneFinalityManifestCommitment", "root") => "#/components/schemas/Hash"; }
    let lane_leaf_count = contract_property(&schemas, "SumeragiV2LaneFinalityManifestCommitment", "leaf_count");
    scalar_contracts! {
        lane_leaf_count.get("minimum") => Unsigned(1);
        lane_leaf_count.get("maximum") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::MAX_LANE_FINALITY_STATEMENTS_PER_BLOCK));
        execution.get("allOf").and_then(Value::as_array).and_then(|all| all.first()).and_then(Value::as_object).and_then(|contract| contract.get("oneOf")) => Length(2);
    }
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
    set_contracts! { object_field_set(proof_object) => asset_field_set("bridge.proof.required"); }
    let header = contract_object(proof_object.get("block_header"), "block header");
    member_contracts! { header; Present => contract_strings("fixture.header.required"); Absent => ["result_merkle_root"]; }
    assert!( header .get("npos_effects_hash") .and_then(Value::as_str) .is_some(), "the exact SCCP V1 fixture must carry the active NPoS effects commitment" );
    assert!( header .get("execution_context_hash") .is_some_and(Value::is_null), "the exact SCCP V1 fixture must carry the required nullable execution-context slot" );
    let expected_root = hex::encode_upper(fixture.bundle.commitment_root);
    scalar_contracts! { header.get("sccp_commitment_root") => Text(expected_root.as_str()); }
    let document = generate_spec();
    let schemas = component_schemas(&document);
    let bytes32 = contract_schema(schemas, "SumeragiV2Bytes32");
    scalar_contracts! { bytes32.get("type") => Text("string"); bytes32.get("minLength") => Unsigned(64); bytes32.get("maxLength") => Unsigned(64); bytes32.get("pattern") => Text("^[0-9A-F]{64}$"); }
    let fixed = contract_schema(schemas, "SumeragiV2Fixed32ByteArray");
    scalar_contracts! { fixed.get("type") => Text("array"); }
    assert_array_bounds(fixed, 32, 32, None);
    let artifact = contract_object(proof_object.get("finality_artifact"), "artifact object");
    set_contracts! { object_field_set(artifact) => asset_field_set("fixture.artifact.fields"); }
    scalar_contracts! { artifact.get("format_version") => Unsigned(4); artifact.get("protocol_version") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)); }
    let context = contract_object(artifact.get("height_context"), "height context");
    scalar_contracts! { context.get("protocol_version") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)); }
    for nullable in contract_strings("height.context.nullable") {
        assert!( context.get(nullable).is_some_and(Value::is_null), "exact SCCP V1 fixture must carry null height-context slot `{nullable}`" );
    }
    let mode = contract_object(context.get("mode"), "consensus mode");
    scalar_contracts! { mode.get("mode") => Text("npos"); }
    assert_eq!(mode.get("details"), Some(&Value::Null));
    let encoding = contract_object(context.get("da_layout").and_then(Value::as_object).and_then(|layout| layout.get("encoding")), "payload encoding");
    scalar_contracts! { encoding.get("encoding") => Text("reed_solomon16"); }
    assert_eq!(encoding.get("details"), Some(&Value::Null));
    let roster = contract_array(context.get("roster"), "powered roster");
    count_contracts! { roster.len() => 4; }
    for validator in roster {
        let validator = validator.as_object().expect("validator-power object");
        let key = contract_text(validator.get("validator"), "validator id");
        count_contracts! { key.len() => 102; }
        assert!(key.starts_with("ea0130"));
        assert!(key[6..].bytes().all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)));
        assert!(validator.get("power").and_then(Value::as_u64).is_some());
    }
    let quorum = contract_object(context.get("quorum"), "dual quorum");
    count_contracts! { quorum.len() => 2; }
    assert!(quorum.get("min_signers").and_then(Value::as_u64).is_some());
    assert!(quorum.get("total_power").and_then(Value::as_u64).is_some());
    let subject = contract_object(artifact.get("subject"), "block subject");
    assert!(subject.get("parent_block_hash").is_some_and(Value::is_null));
    for field in contract_words("block_hash payload_hash") {
        let hash = contract_text(subject.get(field), "canonical hash");
        assert!(hash.starts_with("hash:") && hash.len() == 74);
    }
    let qc = contract_object(artifact.get("commit_qc"), "commit QC");
    let phase = contract_object(qc.get("phase"), "commit phase");
    scalar_contracts! { phase.get("phase") => Text("commit"); }
    assert_eq!(phase.get("details"), Some(&Value::Null));
    assert!(qc.get("round").and_then(Value::as_object).and_then(|round| round.get("context_id")).and_then(Value::as_array).is_some_and(|id| id.len() == 1));
    assert_eq!(qc.get("proposal_round"), qc.get("round"));
    let execution = contract_object(qc.get("execution_commitment"), "execution commitment");
    set_contracts! { object_field_set(execution) => asset_field_set("fixture.execution.fields"); }
    for root in contract_words(concat!("parent_state_root post_state_root ordinary_writes_root ", "native_amx_application_manifest_root")) {
        assert!(execution.get(root).and_then(Value::as_str).is_some_and(|hash| hash.starts_with("hash:") && hash.len() == 74), "canonical {root}");
    }
    for nullable in contract_strings("execution.nullable") {
        assert_eq!( execution.get(nullable), Some(&Value::Null), "exact SCCP V1 fixture must carry null execution slot `{nullable}`" );
    }
    assert!(execution.get("executed_block_wire_len").and_then(Value::as_u64).is_some_and(|length| length > 0));
    scalar_contracts! { execution.get("kagemusha_top_up_count") => Unsigned(0); execution.get("native_amx_application_manifest_version") => Unsigned(u64::from(iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION)); execution.get("native_amx_application_manifest_count") => Unsigned(0); }
    let empty_root = norito::json::to_value(&iroha_data_model::block::consensus_v2::native_amx_application_manifest_empty_root()).expect("serialize empty root");
    assert_eq!(execution.get("native_amx_application_manifest_root"), Some(&empty_root));
    assert!(qc.get("aggregate_signature").and_then(Value::as_array).is_some_and(|signature| signature.len() == 96));
    let pops = contract_array(artifact.get("validator_set_pops"), "validator PoPs");
    count_contracts! { pops.len() => roster.len(); }
    assert!(pops.iter().all(|pop| pop.as_array().is_some_and(|bytes| { bytes.len() == 96 && bytes.iter().all(|byte| byte.as_u64().is_some_and(|byte| byte <= 255)) })));
    let mut missing = value.clone();
    let removed = missing.as_object_mut().and_then(|proof| proof.get_mut("finality_artifact")).and_then(Value::as_object_mut).and_then(|artifact| artifact.get_mut("commit_qc")).and_then(Value::as_object_mut).expect("mutable CommitQC").remove("execution_commitment");
    assert!(removed.is_some());
    assert!(norito::json::from_value::<iroha_data_model::bridge::BridgeFinalityProof>(missing).is_err(), "decoder accepted missing execution commitment");
    let mut retired_header = header.clone();
    retired_header.insert("result_merkle_root".to_owned(), Value::Null);
    assert!(norito::json::from_value::<iroha_data_model::block::BlockHeader>(Value::Object(retired_header)).is_err(), "decoder accepted retired result root");
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
    property_refs!(schemas;
        "StateFinalityResponse", "block_hash", "#/components/schemas/Hash";
        "StateFinalityResponse", "state_root", "#/components/schemas/Hash";
        "StateFinalityResponse", "block_header", "#/components/schemas/BlockHeader";
        "StateFinalityResponse", "finality_artifact", "#/components/schemas/SumeragiV2FinalityArtifact";
    );
    let schema = contract_schema(schemas, "StateFinalityResponse");
    let properties = contract_object(schema.get("properties"), "state finality properties");
    member_contracts! { properties; Absent => contract_strings("ledger.state_finality.retired"); }
    member_contracts! { schemas; Absent => contract_words("StateRootResponse StateProofResponse"); }
    scalar_contracts! { contract_property(schemas, "StateFinalityResponse", "height").get("minimum") => Unsigned(1); }
    for path in contract_words("/v1/ledger/state/{height} /v1/ledger/state-proof/{height}") {
        let operation = openapi_operation(&document, path, "get");
        let description = contract_text(operation.get("description"), "ledger state endpoint description");
        assert!(description.contains("Sumeragi-v2"));
        assert!(description.contains("fails closed"));
        text_contracts! { operation_response_schema_ref(operation, "200", path) => "#/components/schemas/StateFinalityResponse"; }
        let content = response_content(operation, "200");
        set_contracts! { content.keys().map(String::as_str).collect::<BTreeSet<_>>() => contract_word_set("application/json application/x-norito"); }
        let norito = contract_object(content.get("application/x-norito").and_then(|media| media.get("schema")), "Norito response schema");
        scalar_contracts! { norito.get("type") => Text("string"); norito.get("format") => Text("binary"); }
    }
    let paths = contract_object(document.get("paths"), "OpenAPI paths");
    member_contracts! { paths; Absent => contract_strings("ledger.state_finality.retired_paths"); }
    member_contracts! { schemas; Absent => contract_strings("ledger.state_finality.retired_schemas"); }
}
#[test]
fn bridge_finality_operations_describe_durable_v2_evidence() {
    let document = generate_spec();
    for contract in response_rows! {
        "/v1/bridge/finality/{height}", "get", "200", "#/components/schemas/BridgeFinalityProof";
        "/v1/bridge/finality/attestation/{height}", "get", "200", "#/components/schemas/BridgeFinalityAttestationV1";
        "/v1/bridge/finality/bundle/{height}", "get", "200", "#/components/schemas/BridgeFinalityBundle";
    } {
        let operation = openapi_operation(&document, contract.path, contract.method);
        let description = contract_text(operation.get("description"), "bridge description");
        assert!(description.contains("Sumeragi-v2") && description.contains("durable"));
        assert!(!description.contains("validator set signatures") && !description.contains("block header and commit certificate"));
        text_contracts! { operation_response_schema_ref(operation, "200", contract.path) => contract.schema_ref; }
        let norito = contract_object(response_content(operation, "200").get("application/x-norito").and_then(|media| media.get("schema")), "Norito response schema");
        scalar_contracts! { norito.get("type") => Text("string"); norito.get("format") => Text("binary"); }
    }
    let path = "/v1/bridge/finality/attestation/{height}";
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
#[test]
fn generated_spec_documents_read_only_nexus_lifecycle_status() {
    let document = generate_spec();
    let paths = contract_object(document.get("paths"), "paths");
    let path = contract_object(paths.get("/v1/nexus/lifecycle"), "lifecycle path");
    let operation = contract_object(path.get("get"), "lifecycle GET");
    text_contracts! { operation_response_schema_ref(operation, "200", "/v1/nexus/lifecycle") => "#/components/schemas/NexusLaneLifecycleStatusV1"; }
    let content = response_content(operation, "200");
    member_contracts! { content; Present => ["application/json", "application/x-norito"]; }
    member_contracts! { path; Absent => ["post"]; }
    let schemas = component_schemas(&document);
    let schema = contract_schema(schemas, "NexusLaneLifecycleStatusV1");
    assert_eq!(schema.get("additionalProperties"), Some(&Value::Bool(false)));
    assert_required_inventory(schema, "lifecycle.required");
    let properties = contract_object(schema.get("properties"), "lifecycle properties");
    set_contracts! { object_field_set(properties) => contract_words(concat!("catalog_hash incarnation_root incarnations lane_count lanes ", "runtime_catalog_hash version")).into_iter().collect(); }
    scalar_contracts! { properties.get("lanes").and_then(|property| property.get("type")) => Text("array"); }
    member_contracts! { properties; Absent => ["nexus_enabled"]; Present => contract_words("catalog_hash incarnations incarnation_root runtime_catalog_hash"); }
    let runtime_root = properties.get("runtime_catalog_hash").expect("required runtime root");
    assert_eq!(runtime_root.get("type"), Some(&norito::json!(["string", "null"])));
    scalar_contracts! { properties.get("incarnations").and_then(|property| property.get("items")).and_then(|items| items.get("$ref")) => Text("#/components/schemas/NexusLaneLifecycleIncarnationEntry"); }
}
#[test]
fn generated_spec_documents_exact_authoritative_sumeragi_v2_status() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    let max_validators = u64::try_from(iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound");
    for (path, expected, label) in contract_rows! { "/v1/sumeragi/status", "#/components/schemas/SumeragiStatusResponse", "authoritative status"; "/v1/sumeragi/diagnostics", "#/components/schemas/SumeragiDiagnosticsResponse", "operator diagnostics"; } {
        let actual = document.get("paths").and_then(Value::as_object).and_then(|paths| paths.get(path)).and_then(Value::as_object).and_then(|item| item.get("get")).and_then(Value::as_object).map(|operation| operation_response_schema_ref(operation, "200", path));
        assert_eq!(actual, catalog_openapi_route_enabled(CatalogHttpMethod::Get, path).then_some(expected), "{label} catalog projection");
    }
    let status = contract_schema(schemas, "SumeragiStatusResponse");
    scalar_contracts! { status.get("additionalProperties") => Flag(false); }
    let properties = contract_object(status.get("properties"), "status properties");
    scalar_contracts! { properties.get("protocol_version").and_then(|schema| schema.get("minimum")) => Unsigned(4); }
    member_contracts! { properties; Present => contract_strings("status.present"); Absent => contract_strings("status.absent"); }
    for field in contract_words("node_fingerprint build_fingerprint config_fingerprint") {
        scalar_contracts! { properties.get(field).and_then(|schema| schema.get("$ref")) => Text("#/components/schemas/Hash"); }
    }
    scalar_contracts! { properties.get("height_context_id").and_then(|schema| schema.get("$ref")) => Text("#/components/schemas/SumeragiV2HeightContextId"); }
    for contract in [
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "phase", expected: "#/components/schemas/SumeragiV2StatusPhase" },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "body_state", expected: "#/components/schemas/SumeragiV2BodyState", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "locked_prepare_qc", expected: "#/components/schemas/SumeragiV2QuorumCertificateRef", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "highest_prepare_qc", expected: "#/components/schemas/SumeragiV2QuorumCertificateRef", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "last_timeout_certificate", expected: "#/components/schemas/SumeragiV2TimeoutCertificateRef", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "last_committed_subject", expected: "#/components/schemas/SumeragiV2BlockSubject", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "height_context", expected: "#/components/schemas/SumeragiV2HeightContextStatus", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "last_commit_qc", expected: "#/components/schemas/SumeragiV2CommitQcStatus", },
        PropertyRefContract { owner: "SumeragiStatusResponse", property: "liveness", expected: "#/components/schemas/SumeragiV2LivenessStatus", },
    ] {
        scalar_contracts! { contract_property(schemas, contract.owner, contract.property).get("$ref") => Text(contract.expected); }
    }
    let required = contract_array(status.get("required"), "status required");
    for field in contract_words("restart_required height_context liveness") {
        string_members! { required; Present => &[field]; };
    }
    string_members! { required; Absent => &["last_commit_qc"]; };
    property_refs!(schemas;
        "SumeragiV2HeightContextStatus", "mode", "#/components/schemas/SumeragiV2ConsensusMode";
        "SumeragiV2HeightContextStatus", "quorum", "#/components/schemas/SumeragiV2DualQuorum";
        "SumeragiV2CommitQcStatus", "certificate", "#/components/schemas/SumeragiV2QuorumCertificateRef";
    );
    scalar_contracts! { contract_property(schemas, "SumeragiV2HeightContextStatus", "validator_count").get("maximum") => Unsigned(max_validators); }
    for field in contract_words("validator_count signer_count min_signers") {
        scalar_contracts! { contract_property(schemas, "SumeragiV2CommitQcStatus", field).get("maximum") => Unsigned(max_validators); }
    }
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
