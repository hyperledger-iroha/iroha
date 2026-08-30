#[test]
fn pipeline_status_openapi_exposes_only_the_exact_first_release_scope() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let response = schemas
        .get("PipelineTransactionStatusResponse")
        .and_then(Value::as_object)
        .expect("pipeline transaction status response schema");
    assert_eq!(response.get("type"), Some(&Value::from("object")));
    assert_eq!(
        response.get("additionalProperties"),
        Some(&Value::from(false))
    );
    let properties = response
        .get("properties")
        .and_then(Value::as_object)
        .expect("pipeline transaction status response properties");
    let exact_response_fields = BTreeSet::from(["hash", "resolved_from", "scope", "status"]);
    assert_eq!(
        properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        exact_response_fields
    );
    assert_eq!(
        response["required"]
            .as_array()
            .expect("pipeline transaction status required fields")
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .expect("pipeline transaction status required field name")
            })
            .collect::<BTreeSet<_>>(),
        exact_response_fields
    );
    assert_eq!(properties["scope"]["type"], Value::from("string"));
    assert_eq!(
        properties["scope"]["enum"],
        Value::Array(vec![Value::from("local"), Value::from("global")])
    );
    assert_eq!(properties["hash"]["type"], Value::from("string"));
    assert_eq!(properties["hash"]["minLength"], Value::from(64_u64));
    assert_eq!(properties["hash"]["maxLength"], Value::from(64_u64));
    assert_eq!(
        properties["hash"]["pattern"],
        Value::from("^[0-9a-f]{63}[13579bdf]$")
    );
    assert!(
        properties["hash"]["description"]
            .as_str()
            .expect("pipeline response hash description")
            .contains("Iroha hash marker set")
    );

    let operation = &document["paths"]["/v1/pipeline/transactions/status"]["get"];
    assert_eq!(
        operation["responses"]
            .as_object()
            .expect("pipeline status responses")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["200", "404"])
    );
    let parameters = operation["parameters"]
        .as_array()
        .expect("pipeline status parameters");
    assert_eq!(
        parameters
            .iter()
            .map(|parameter| {
                parameter["name"]
                    .as_str()
                    .expect("pipeline status parameter name")
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["hash", "scope"])
    );
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter["in"] == Value::from("query"))
    );
    let scope = parameters
        .iter()
        .find(|parameter| parameter["name"].as_str() == Some("scope"))
        .expect("pipeline status scope parameter");
    assert_eq!(scope["required"], Value::from(false));
    assert_eq!(scope["schema"]["type"], Value::from("string"));
    assert_eq!(scope["schema"]["default"], Value::from("global"));
    assert_eq!(
        scope["schema"]["enum"],
        Value::Array(vec![Value::from("local"), Value::from("global")])
    );
    assert!(
        scope["description"]
            .as_str()
            .expect("pipeline status scope description")
            .contains("omission selects `global`")
    );
    let hash = parameters
        .iter()
        .find(|parameter| parameter["name"].as_str() == Some("hash"))
        .expect("pipeline status hash parameter");
    assert_eq!(hash["required"], Value::from(true));
    assert_eq!(hash["schema"]["type"], Value::from("string"));
    assert_eq!(hash["schema"]["minLength"], Value::from(64_u64));
    assert_eq!(hash["schema"]["maxLength"], Value::from(64_u64));
    assert_eq!(
        hash["schema"]["pattern"],
        Value::from("^[0-9a-f]{63}[13579bdf]$")
    );
    assert!(
        hash["description"]
            .as_str()
            .expect("pipeline request hash description")
            .contains("Iroha hash marker set")
    );
}

#[test]
fn npos_schema_excludes_retired_process_local_and_vrf_surfaces() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let retired = [
        "vrf_penalty_epoch",
        "vrf_committed_no_reveal_total",
        "vrf_no_participation_total",
        "vrf_late_reveals_total",
    ];
    let npos = schemas
        .get("SumeragiNposDiagnostics")
        .and_then(Value::as_object)
        .expect("NPoS diagnostics schema");
    let npos_properties = npos
        .get("properties")
        .and_then(Value::as_object)
        .expect("NPoS diagnostics properties");
    assert_eq!(
        npos_properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "epoch_length_blocks",
            "epoch_seed",
            "prf_height",
            "prf_view",
            "vrf_commit_deadline_offset",
            "vrf_reveal_deadline_offset",
        ])
    );
    for field in retired {
        assert!(
            !npos_properties.contains_key(field),
            "retired process-local VRF counter remains in OpenAPI: {field}"
        );
    }
    assert!(
        !schemas.contains_key("SumeragiVrfPenaltiesReport"),
        "retired VRF penalty report schema remains in OpenAPI"
    );
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    assert!(
        paths
            .keys()
            .all(|path| !path.starts_with("/v1/sumeragi/vrf/")),
        "retired Sumeragi VRF path remains in OpenAPI"
    );
}

#[test]
fn native_amx_participant_diagnostics_schema_is_closed_and_bounded() {
    let schemas = openapi_schemas();
    let response = schemas
        .get("SumeragiDiagnosticsResponse")
        .and_then(Value::as_object)
        .expect("diagnostics response schema");
    let applications = response
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("native_amx_participant_applications"))
        .and_then(Value::as_object)
        .expect("Native AMX participant diagnostics vector schema");
    assert_eq!(applications.get("maxItems"), Some(&Value::from(1_024_u64)));
    assert_eq!(
        applications
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiNativeAmxParticipantApplication")
    );
    let row = schemas
        .get("SumeragiNativeAmxParticipantApplication")
        .and_then(Value::as_object)
        .expect("Native AMX participant diagnostics row schema");
    assert_eq!(row.get("additionalProperties"), Some(&Value::from(false)));
    let state_geometry = row
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("Native AMX participant state/carrier geometry");
    assert_eq!(state_geometry.len(), 4);
    assert_eq!(
        state_geometry
            .iter()
            .filter_map(|case| { case.get("properties")?.get("state")?.get("const")?.as_str() })
            .collect::<Vec<_>>(),
        vec![
            "certified_pending_carrier",
            "committed_evidence_pending",
            "durably_applied",
            "conflict",
        ]
    );
    for index in [0_usize, 3] {
        assert_eq!(
            state_geometry[index]["properties"]["application_block_height"]["type"],
            Value::from("null")
        );
    }
    for index in [1_usize, 2] {
        let required = state_geometry[index]["required"]
            .as_array()
            .expect("committed Native application carrier fields");
        assert!(required.contains(&Value::from("application_block_height")));
        assert!(required.contains(&Value::from("application_block_hash")));
    }
    let source_count = row
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("source_count"))
        .and_then(Value::as_object)
        .expect("source count schema");
    assert_eq!(source_count.get("minimum"), Some(&Value::from(1_u64)));
    assert_eq!(source_count.get("maximum"), Some(&Value::from(4_096_u64)));
    let states = schemas
        .get("SumeragiNativeAmxParticipantApplicationState")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("enum"))
        .and_then(Value::as_array)
        .expect("Native AMX participant diagnostics states");
    assert_eq!(
        states,
        &vec![
            Value::from("certified_pending_carrier"),
            Value::from("committed_evidence_pending"),
            Value::from("durably_applied"),
            Value::from("conflict"),
        ]
    );
}
#[test]
fn autonomous_lane_execution_diagnostics_schema_is_closed_and_bounded() {
    let schemas = openapi_schemas();
    let response = schemas
        .get("SumeragiDiagnosticsResponse")
        .and_then(Value::as_object)
        .expect("diagnostics response schema");
    let executions = response
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("autonomous_lane_executions"))
        .and_then(Value::as_object)
        .expect("autonomous execution diagnostics vector schema");
    assert_eq!(executions.get("maxItems"), Some(&Value::from(128_u64)));
    assert_eq!(
        executions
            .get("items")
            .and_then(Value::as_object)
            .and_then(|items| items.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/SumeragiAutonomousLaneExecution")
    );
    let row = schemas
        .get("SumeragiAutonomousLaneExecution")
        .and_then(Value::as_object)
        .expect("autonomous execution row schema");
    assert_eq!(row.get("additionalProperties"), Some(&Value::from(false)));
    let required = row
        .get("required")
        .and_then(Value::as_array)
        .expect("autonomous execution required fields");
    let properties = row
        .get("properties")
        .and_then(Value::as_object)
        .expect("autonomous execution properties");
    for field in [
        "reservation_owner_hash",
        "proposal_identity_hash",
        "reservation_group_hash",
    ] {
        assert!(
            required.contains(&Value::from(field)),
            "{field} must be present at the Queue fsync boundary"
        );
    }
    for field in ["proposal_view", "proposal_hash", "descriptor_hash"] {
        assert!(
            !required.contains(&Value::from(field)),
            "{field} remains absent until the finalized proposal is durable"
        );
    }
    assert!(
        !required.contains(&Value::from("stuck_reason")),
        "stuck reason remains optional for a future independently proven terminal stage"
    );
    assert!(
        row.get("allOf")
            .and_then(Value::as_array)
            .is_some_and(|constraints| constraints.len() >= 4),
        "autonomous row must expose identity-pair and reservation-stage constraints"
    );
    for payload_field in [
        "reservation_keys",
        "entrypoints",
        "routing_plans",
        "source_bundle",
    ] {
        assert!(
            !properties.contains_key(payload_field),
            "{payload_field} payload bytes must not leak through diagnostics"
        );
    }
    let stages = schemas
        .get("SumeragiAutonomousLaneExecutionStage")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("enum"))
        .and_then(Value::as_array)
        .expect("autonomous stage enum");
    let expected_stages = [
        "reservations_durable",
        "executable_payload_durable",
        "payload_availability_certified",
        "lane_certified",
        "certified_bundle_durable",
        "merge_candidate_durable",
        "global_carrier_committed",
        "kura_wsv_application_receipt_durable",
        "queue_finalized",
        "conflict",
    ]
    .map(Value::from);
    assert_eq!(stages.as_slice(), expected_stages.as_slice());
    let reasons = schemas
        .get("SumeragiAutonomousLaneExecutionStuckReason")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("enum"))
        .and_then(Value::as_array)
        .expect("autonomous stuck-reason enum");
    let expected_reasons = [
        "awaiting_executable_payload",
        "awaiting_payload_availability",
        "awaiting_lane_certification",
        "certified_bundle_unavailable",
        "awaiting_merge_selection",
        "awaiting_global_carrier",
        "awaiting_application_receipt",
        "queue_finalization_unverifiable",
        "evidence_conflict",
    ]
    .map(Value::from);
    assert_eq!(reasons.as_slice(), expected_reasons.as_slice());
}

#[test]
fn status_openapi_uses_only_exact_scalar_probe_paths() {
    let document = canonical_document();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    assert_eq!(
        paths
            .keys()
            .filter(|path| path.as_str() == "/status" || path.starts_with("/status/"))
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["/status", "/status/blocks", "/status/peers"])
    );
    assert!(!paths.contains_key("/status/{tail}"));
    for retired in [
        "/v1/node/query/projection/checkpoint/plan",
        "/v1/node/query/projection/checkpoint/publish",
    ] {
        assert!(
            !paths.contains_key(retired),
            "retired unverified projection route leaked into OpenAPI: {retired}"
        );
    }
    for path in ["/status/blocks", "/status/peers"] {
        let item = paths
            .get(path)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} path item"));
        assert_eq!(
            item.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["get"]),
            "{path} must expose only GET"
        );
        let operation = item
            .get("get")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} GET operation"));
        assert!(
            operation.get("parameters").is_none(),
            "exact status probes do not accept path selectors"
        );
        let schema = &operation["responses"]["200"]["content"]["application/json"]["schema"];
        assert_eq!(schema["type"], Value::from("integer"));
        assert_eq!(schema["format"], Value::from("uint64"));
        assert_eq!(schema["minimum"], Value::from(0_u64));
    }
}

#[test]
fn operator_webauthn_openapi_is_closed_bounded_and_capacity_aware() {
    fn object_fields<'a>(schema: &'a Value, label: &str) -> BTreeSet<&'a str> {
        schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{label} properties"))
            .keys()
            .map(String::as_str)
            .collect()
    }
    fn required_fields<'a>(schema: &'a Value, label: &str) -> BTreeSet<&'a str> {
        schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{label} required fields"))
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .unwrap_or_else(|| panic!("{label} required field name"))
            })
            .collect()
    }
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let credential_fields = BTreeSet::from(["id", "rawId", "response", "type"]);
    for (request_name, response_name, response_fields) in [
        (
            "OperatorWebAuthnAssertionRequest",
            "OperatorWebAuthnAssertionResponse",
            BTreeSet::from(["authenticatorData", "clientDataJSON", "signature"]),
        ),
        (
            "OperatorWebAuthnAttestationRequest",
            "OperatorWebAuthnAttestationResponse",
            BTreeSet::from(["attestationObject", "clientDataJSON"]),
        ),
    ] {
        let request = schemas
            .get(request_name)
            .unwrap_or_else(|| panic!("{request_name} schema"));
        assert_eq!(request["type"], Value::from("object"));
        assert_eq!(request["additionalProperties"], Value::from(false));
        assert_eq!(object_fields(request, request_name), credential_fields);
        assert_eq!(required_fields(request, request_name), credential_fields);
        assert_eq!(
            request["properties"]["type"]["const"],
            Value::from("public-key")
        );
        for id_field in ["id", "rawId"] {
            assert_eq!(
                request["properties"][id_field]["$ref"],
                Value::from("#/components/schemas/OperatorWebAuthnCredentialId")
            );
        }
        assert_eq!(
            request["properties"]["response"]["$ref"],
            Value::from(format!("#/components/schemas/{response_name}"))
        );
        let response = schemas
            .get(response_name)
            .unwrap_or_else(|| panic!("{response_name} schema"));
        assert_eq!(response["additionalProperties"], Value::from(false));
        assert_eq!(object_fields(response, response_name), response_fields);
        assert_eq!(required_fields(response, response_name), response_fields);
        for field in response_fields {
            assert_eq!(
                response["properties"][field]["$ref"],
                Value::from("#/components/schemas/OperatorWebAuthnBinary")
            );
        }
    }
    let binary = &schemas["OperatorWebAuthnBinary"];
    assert_eq!(binary["minLength"], Value::from(1_u64));
    assert_eq!(binary["maxLength"], Value::from(65_536_u64));
    assert_eq!(
        binary["pattern"],
        Value::from("^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{2}|[A-Za-z0-9_-]{3})?$")
    );
    let credential_id = &schemas["OperatorWebAuthnCredentialId"];
    assert_eq!(credential_id["minLength"], Value::from(1_u64));
    assert_eq!(credential_id["maxLength"], Value::from(1_366_u64));
    assert_eq!(
        credential_id["pattern"],
        Value::from("^(?:[A-Za-z0-9_-]{4})*(?:[A-Za-z0-9_-]{2}|[A-Za-z0-9_-]{3})?$")
    );

    let paths = document["paths"].as_object().expect("OpenAPI paths");
    for (path, request_name, expected_responses) in [
        (
            "/v1/operator/auth/login/verify",
            "OperatorWebAuthnAssertionRequest",
            BTreeSet::from(["200", "400", "401", "403", "413", "415", "429", "503"]),
        ),
        (
            "/v1/operator/auth/registration/verify",
            "OperatorWebAuthnAttestationRequest",
            BTreeSet::from([
                "200", "400", "401", "403", "409", "413", "415", "429", "503",
            ]),
        ),
    ] {
        let operation = &paths[path]["post"];
        assert_eq!(
            operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            Value::from(format!("#/components/schemas/{request_name}"))
        );
        assert_eq!(operation["requestBody"]["required"], Value::from(true));
        assert_eq!(
            operation["x-iroha-max-request-bytes"],
            Value::from(65_536_u64)
        );
        let responses = operation["responses"]
            .as_object()
            .unwrap_or_else(|| panic!("{path} responses"));
        assert_eq!(
            responses
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            expected_responses
        );
        assert!(
            responses["413"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("request_payload_too_large"))
        );
        assert!(
            responses["503"]["description"]
                .as_str()
                .is_some_and(|description| {
                    description.contains("operator_auth_state_capacity_exhausted")
                })
        );
    }
    assert!(
        paths["/v1/operator/auth/registration/verify"]["post"]["responses"]["409"]["description"]
            .as_str()
            .is_some_and(|description| {
                description.contains("operator_webauthn_credential_capacity_exhausted")
            })
    );
    for path in [
        "/v1/operator/auth/login/options",
        "/v1/operator/auth/registration/options",
    ] {
        assert!(
            paths[path]["post"]["responses"]["503"]["description"]
                .as_str()
                .is_some_and(|description| {
                    description.contains("operator_auth_state_capacity_exhausted")
                })
        );
    }
}
