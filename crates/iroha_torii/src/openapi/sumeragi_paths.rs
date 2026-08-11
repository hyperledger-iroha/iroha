fn sumeragi_operator_get_operation(mut methods: Map) -> Map {
    let Some(Value::Object(operation)) = methods.get_mut("get") else {
        unreachable!("Sumeragi operator GET operation must define GET")
    };
    let parameters = operation
        .entry("parameters".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("Sumeragi GET parameters must be an array");
    parameters.extend(operator_signature_header_parameters());
    insert_operator_signature_auth_contract(operation);
    let responses = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .expect("Sumeragi GET responses must be an object");
    responses.insert(
        "401".into(),
        json_response(
            "The exact NetworkId-bound operator signature is missing, malformed, stale, or replayed.",
            error_schema_reference(),
        ),
    );
    responses.insert(
        "403".into(),
        json_response(
            "The operator signing key is not trusted by this node.",
            error_schema_reference(),
        ),
    );
    methods
}

fn sumeragi_evidence_list_query_parameters() -> Vec<Value> {
    vec![
        norito::json!({
            "name": "limit",
            "in": "query",
            "required": false,
            "description": "Maximum evidence rows returned; defaults to 50 and is capped at 1000 before state scanning.",
            "schema": { "type": "integer", "format": "uint64", "minimum": 1, "maximum": 1000, "default": 50 }
        }),
        norito::json!({
            "name": "offset",
            "in": "query",
            "required": false,
            "description": "Number of newest matching rows skipped; defaults to 0 and is capped at 10000 before state scanning.",
            "schema": { "type": "integer", "format": "uint64", "minimum": 0, "maximum": 10000, "default": 0 }
        }),
        norito::json!({
            "name": "kind",
            "in": "query",
            "required": false,
            "description": "Exact evidence-kind filter.",
            "schema": {
                "type": "string",
                "enum": ["DoublePrepare", "DoubleCommit", "InvalidQc", "InvalidProposal", "Censorship", "SumeragiV2Equivocation"]
            }
        }),
    ]
}

fn sumeragi_paths() -> Map {
    let mut paths = Map::new();
    paths.insert(
        "/v1/sumeragi/evidence/count".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Count evidence entries.",
            "Return the total number of evidence entries after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/evidence".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "List evidence entries.",
            "List a bounded newest-first evidence page after exact NetworkId-bound operator authentication. The offset is rejected above 10000 before scanning state; at most 1000 selected records are cloned into the response.",
            "#/components/schemas/JsonValue",
            sumeragi_evidence_list_query_parameters(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/status".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch authoritative Sumeragi v2 status.",
            "Return the exact reducer-owned Sumeragi v2 status snapshot after exact NetworkId-bound operator authentication.",
            "#/components/schemas/SumeragiStatusResponse",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/diagnostics".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi operator diagnostics.",
            "After exact NetworkId-bound operator authentication, return non-authoritative pipeline, queue, NPoS election, and Nexus lane diagnostics. Reducer phase, height, view, leader, and certificates are available only from the status endpoint.",
            "#/components/schemas/SumeragiDiagnosticsResponse",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/status/sse".to_owned(),
        Value::Object(event_stream_get_operation(
            "Sumeragi",
            "Stream Sumeragi status.",
            "Stream Sumeragi status via SSE.",
        )),
    );
    paths.insert(
        "/v1/sumeragi/leader".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch current leader.",
            "Return the current Sumeragi leader snapshot after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/bls-keys".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch consensus BLS keys.",
            "Return the current voting roster's network-to-BLS-key map after exact NetworkId-bound operator authentication. The roster is intrinsically capped at the protocol maximum of 31 validators; the global peer registry is not cloned or traversed.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/qc".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch QC snapshots.",
            "Return quorum certificate snapshots after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/checkpoints".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch checkpoint snapshots.",
            "Return checkpoint snapshots after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/consensus-keys".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch consensus keys.",
            "Return at most the newest 128 consensus-key records after exact NetworkId-bound operator authentication. Selection is streaming and allocation-bounded before records are cloned into the response.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/commit-certificates".to_owned(),
        Value::Object(sumeragi_operator_get_operation(
            sumeragi_commit_qcs_operation(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/pacemaker".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch pacemaker status.",
            "Return pacemaker status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/phases".to_owned(),
        Value::Object(json_get_operation(
            "Sumeragi",
            "Fetch phase status.",
            "Return consensus phase status snapshot.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/bridge/finality/{height}".to_owned(),
        Value::Object(bridge_finality_operation()),
    );
    paths.insert(
        "/v1/bridge/finality/attestation/{height}".to_owned(),
        Value::Object(bridge_finality_attestation_operation()),
    );
    paths.insert(
        "/v1/bridge/finality/bundle/{height}".to_owned(),
        Value::Object(bridge_finality_bundle_operation()),
    );
    paths.insert(
        "/v1/sccp/proofs/message/{message_id}".to_owned(),
        Value::Object(sccp_message_bundle_operation()),
    );
    paths.insert(
        "/v1/sccp/capabilities".to_owned(),
        Value::Object(sccp_capabilities_operation()),
    );
    paths.insert(
        "/v1/sccp/registry".to_owned(),
        Value::Object(sccp_registry_operation()),
    );
    paths.insert(
        "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material"
            .to_owned(),
        Value::Object(sccp_sora_outbound_material_operation()),
    );
    paths.insert(
        "/v1/sccp/proof-requests/{message_id}".to_owned(),
        Value::Object(sccp_proof_request_operation()),
    );
    paths.insert(
        "/v1/bridge/proofs/submit".to_owned(),
        Value::Object(sccp_bridge_submit_operation(
            "bridgeProofSubmit",
            "Prepare or submit an SCCP destination-proof transaction.",
            "The JSON-only request carries a canonical destination proof. Preparation provides neither detached-signing value (the optional fields may be absent or null); direct submission must provide both `signature_b64` and the byte-identical prepared `transaction_payload_b64`, together with the exact positive `creation_time_ms` returned by preparation.",
            "#/components/schemas/SccpBridgeProofSubmitRequest",
        )),
    );
    paths.insert(
        "/v1/bridge/messages".to_owned(),
        Value::Object(sccp_bridge_submit_operation(
            "bridgeMessageSubmit",
            "Prepare or submit a protocol-native SCCP admission transaction.",
            "The JSON-only request carries one canonical native proof. Preparation provides neither detached-signing value (the optional fields may be absent or null); direct submission must provide both `signature_b64` and the byte-identical prepared `transaction_payload_b64`, together with the exact positive `creation_time_ms` returned by preparation.",
            "#/components/schemas/SccpBridgeMessageSubmitRequest",
        )),
    );
    paths.insert(
        "/v1/sccp/messages/recent".to_owned(),
        Value::Object(sccp_recent_messages_operation()),
    );
    paths.insert(
        "/v1/sumeragi/validator-sets".to_owned(),
        Value::Object(sumeragi_operator_get_operation(
            sumeragi_validator_sets_operation(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/validator-sets/{height}".to_owned(),
        Value::Object(sumeragi_operator_get_operation(
            sumeragi_validator_set_by_height_operation(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/key-lifecycle".to_owned(),
        Value::Object(sumeragi_operator_get_operation(
            sumeragi_key_lifecycle_operation(),
        )),
    );
    paths.insert(
        "/v1/sumeragi/telemetry".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi telemetry.",
            "Return Sumeragi telemetry snapshot after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/params".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch Sumeragi parameters.",
            "Return Sumeragi consensus parameters after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            Vec::new(),
        ))),
    );
    paths.insert(
        "/v1/sumeragi/commit-qcs/{block_hash}".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch commit QC.",
            "Fetch commit QC by block hash after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            vec![string_path_param("block_hash", "Block hash (hex).")],
        ))),
    );
    paths.insert(
        "/v1/sumeragi/vrf/penalties/{epoch}".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch VRF penalties.",
            "Fetch VRF penalties for an epoch after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "epoch",
                "Epoch identifier.",
                Some("uint64"),
            )],
        ))),
    );
    paths.insert(
        "/v1/sumeragi/vrf/epoch/{epoch}".to_owned(),
        Value::Object(sumeragi_operator_get_operation(json_get_operation(
            "Sumeragi",
            "Fetch VRF epoch data.",
            "Fetch VRF epoch data after exact NetworkId-bound operator authentication.",
            "#/components/schemas/JsonValue",
            vec![integer_path_param(
                "epoch",
                "Epoch identifier.",
                Some("uint64"),
            )],
        ))),
    );
    paths
}
