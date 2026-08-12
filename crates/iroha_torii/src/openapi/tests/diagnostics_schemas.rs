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
