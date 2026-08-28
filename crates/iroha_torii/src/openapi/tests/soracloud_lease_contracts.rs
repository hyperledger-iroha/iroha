#[test]
fn soracloud_control_plane_openapi_exposes_authoritative_lease_accounting() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let contracts: &[(&str, &[&str])] = &[
        (
            "ControlPlaneAuditEvent",
            &[
                "sequence",
                "action",
                "service_name",
                "from_version",
                "to_version",
                "service_manifest_hash",
                "container_manifest_hash",
                "process_generation",
                "config_generation",
                "secret_generation",
                "config_snapshot_hash",
                "secret_snapshot_hash",
                "binding_name",
                "state_key",
                "config_mutations",
                "secret_mutations",
                "governance_tx_hash",
                "rollout_state",
                "policy_name",
                "policy_snapshot_hash",
                "jurisdiction_tag",
                "consent_evidence_hash",
                "break_glass",
                "break_glass_reason",
                "lease_usage",
                "service_lease_commitment",
                "lease_reporting_epoch_rollover",
                "signed_by",
            ],
        ),
        (
            "ControlPlaneServiceLeaseSnapshot",
            &[
                "authoritative_state",
                "effective_status",
                "remaining_runtime_balance",
            ],
        ),
        (
            "SoraInrouReplicaPlacementV1",
            &[
                "replica_slot",
                "economic_clock",
                "lease_started_height",
                "placement_incarnation",
                "host_availability",
                "validator_account_id",
                "peer_id",
                "selected_guest_isa",
            ],
        ),
        (
            "SoraServiceConfigEntryV1",
            &[
                "schema_version",
                "config_name",
                "value_json",
                "value_hash",
                "last_update_sequence",
            ],
        ),
        (
            "SoraServiceLeaseEgressCheckpointV1",
            &[
                "reporting_epoch",
                "assignment",
                "accounted_egress_bytes",
                "last_updated_height",
                "finalize_reporter",
            ],
        ),
        (
            "SoraServiceLeaseReporterAssignmentV1",
            &[
                "schema_version",
                "service_version",
                "placement",
                "placement_reconciled_at_ms",
            ],
        ),
        (
            "SoraServiceLeaseReportingEpochRolloverV1",
            &[
                "schema_version",
                "economic_clock",
                "lease_started_height",
                "previous_reporting_epoch",
                "new_reporting_epoch",
                "reporter_account_id",
                "active_service_version",
                "replica_slot",
                "placement_incarnation",
                "finalized_checkpoint_count",
                "settled_egress_bytes_delta",
                "settled_egress_bytes",
            ],
        ),
        (
            "SoraServiceLeaseStateV1",
            &[
                "schema_version",
                "economic_clock",
                "status",
                "quota_class",
                "replica_count",
                "deployment_deposit",
                "prepaid_runtime_balance",
                "runtime_price_per_block",
                "storage_price_per_gib_block",
                "egress_price_per_mib",
                "lease_started_height",
                "lease_expires_height",
                "reporting_epoch",
                "settled_egress_bytes",
                "egress_reporter_checkpoints",
                "accounted_egress_bytes",
                "last_status_reason",
            ],
        ),
        (
            "SoraServiceLeaseUsageAuditV1",
            &[
                "schema_version",
                "reporting_epoch",
                "assignment",
                "replica_accounted_egress_bytes",
                "finalize_reporter",
            ],
        ),
        (
            "SoraServiceSecretEntryV1",
            &[
                "schema_version",
                "secret_name",
                "envelope",
                "last_update_sequence",
            ],
        ),
    ];
    for (name, fields) in contracts {
        assert_strict_object_schema(schemas, name, fields, &[]);
    }
    let lease_state = component_properties(schemas, "SoraServiceLeaseStateV1");
    assert_eq!(
        lease_state["economic_clock"]["$ref"].as_str(),
        Some("#/components/schemas/SoraServiceLeaseClockV1")
    );
    assert_eq!(lease_state["replica_count"]["minimum"].as_u64(), Some(1));
    assert_eq!(
        lease_state["replica_count"]["maximum"].as_u64(),
        Some(u64::from(
            iroha_data_model::soracloud::SORA_HTTP_SERVICE_REPLICA_MAX_V1
        ))
    );
    assert_eq!(
        lease_state["egress_reporter_checkpoints"]["maxItems"].as_u64(),
        Some(
            iroha_data_model::soracloud::SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1
                as u64
        )
    );
    let reporter_assignment = component_properties(schemas, "SoraServiceLeaseReporterAssignmentV1");
    assert_eq!(
        reporter_assignment["service_version"]["minLength"].as_u64(),
        Some(1)
    );
    assert_eq!(
        reporter_assignment["placement_reconciled_at_ms"]["minimum"].as_u64(),
        Some(1)
    );
    let lease_usage = component_properties(schemas, "SoraServiceLeaseUsageAuditV1");
    assert_eq!(lease_usage["reporting_epoch"]["minimum"].as_u64(), Some(1));
    assert_eq!(
        nullable_property_ref(schemas, "ControlPlaneServiceSnapshot", "service_lease"),
        "#/components/schemas/ControlPlaneServiceLeaseSnapshot"
    );
    assert_eq!(
        nullable_property_ref(
            schemas,
            "ControlPlaneAuditEvent",
            "lease_reporting_epoch_rollover",
        ),
        "#/components/schemas/SoraServiceLeaseReportingEpochRolloverV1"
    );
    assert_eq!(
        nullable_property_ref(schemas, "ControlPlaneAuditEvent", "lease_usage"),
        "#/components/schemas/SoraServiceLeaseUsageAuditV1"
    );
    assert_eq!(
        nullable_property_ref(
            schemas,
            "ControlPlaneAuditEvent",
            "service_lease_commitment",
        ),
        "#/components/schemas/Hash"
    );
    let actions = schemas["SoracloudAction"]["oneOf"]
        .as_array()
        .expect("Soracloud action variants")
        .iter()
        .map(|variant| {
            variant["properties"]["action"]["const"]
                .as_str()
                .expect("Soracloud action tag")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actions,
        BTreeSet::from([
            "CiphertextQuery",
            "ConfigMutation",
            "DecryptionRequest",
            "Deploy",
            "FheJobRun",
            "FhePolicyRegister",
            "FhePolicyRevoke",
            "FhePolicyRotate",
            "LeaseUsage",
            "LeaseReportingEpochRollover",
            "Rollback",
            "Rollout",
            "SecretMutation",
            "StateMutation",
            "Upgrade",
        ])
    );
    let service_fields = component_properties(schemas, "ControlPlaneServiceSnapshot");
    for retired in [
        "quota_class",
        "service_lease_status",
        "lease_expires_height",
        "prepaid_runtime_balance",
        "remaining_runtime_balance",
    ] {
        assert!(
            !service_fields.contains_key(retired),
            "retired flattened service lease field remains: {retired}"
        );
    }
}

#[test]
fn soracloud_openapi_uses_the_consensus_block_lease_clock() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let contracts: &[(&str, &[&str], &[&str])] = &[
        ("AgentDeployPayload", &["lease_blocks"], &["lease_ticks"]),
        (
            "AgentLeaseRenewPayload",
            &["lease_blocks"],
            &["lease_ticks"],
        ),
        (
            "AgentApartmentStatusEntry",
            &[
                "lease_started_height",
                "lease_expires_height",
                "lease_remaining_blocks",
            ],
            &[
                "lease_started_sequence",
                "lease_expires_sequence",
                "lease_remaining_ticks",
            ],
        ),
        (
            "AgentAutonomyStatusResponse",
            &["lease_expires_height", "lease_remaining_blocks"],
            &["lease_expires_sequence", "lease_remaining_ticks"],
        ),
        (
            "SoraHttpServiceEconomicsV1",
            &[
                "lease_duration_blocks",
                "runtime_price_per_block",
                "storage_price_per_gib_block",
            ],
            &[
                "lease_duration_sequences",
                "runtime_price_per_sequence",
                "storage_price_per_gib_sequence",
            ],
        ),
        (
            "SoraMailboxContractV1",
            &["retention_blocks"],
            &["retention_sequences"],
        ),
        (
            "SoraServiceLeaseReportingEpochRolloverV1",
            &["lease_started_height"],
            &["lease_started_sequence"],
        ),
        (
            "SoraServiceLeaseStateV1",
            &[
                "runtime_price_per_block",
                "storage_price_per_gib_block",
                "lease_started_height",
                "lease_expires_height",
            ],
            &[
                "runtime_price_per_sequence",
                "storage_price_per_gib_sequence",
                "lease_started_sequence",
                "lease_expires_sequence",
            ],
        ),
        (
            "SoracloudRuntimeMailboxPlan",
            &["retention_blocks"],
            &["retention_sequences"],
        ),
        (
            "SoracloudRuntimeServicePlan",
            &[
                "lease_expires_height",
                "authoritative_pending_mailbox_messages",
            ],
            &[
                "lease_expires_sequence",
                "reported_pending_mailbox_messages",
            ],
        ),
        (
            "SoracloudRuntimeLeaseVolumePlan",
            &["lease_expires_height"],
            &["lease_expires_sequence"],
        ),
        (
            "SoracloudRuntimeApartmentPlan",
            &["lease_expires_height"],
            &["lease_expires_sequence"],
        ),
    ];
    for (name, current, retired) in contracts {
        let fields = component_properties(schemas, name);
        for field in *current {
            assert!(
                fields.contains_key(*field),
                "{name} is missing first-release block-clock field {field}"
            );
        }
        for field in *retired {
            assert!(
                !fields.contains_key(*field),
                "{name} retains retired sequence-clock field {field}"
            );
        }
    }

    let enabled_pressure = schemas["SoracloudStatusRuntimePressureV1"]["oneOf"]
        .as_array()
        .expect("runtime pressure variants")
        .iter()
        .find(|variant| variant["properties"]["enabled"]["const"].as_bool() == Some(true))
        .and_then(|variant| variant["properties"].as_object())
        .expect("enabled runtime pressure properties");
    assert!(enabled_pressure.contains_key("authoritative_pending_mailbox_messages"));
    assert!(!enabled_pressure.contains_key("reported_pending_mailbox_messages"));
}

#[test]
fn soracloud_runtime_execution_host_openapi_is_first_release_exact() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "SoraRuntimeDeterministicValidatorHostV1",
        &["lane_id", "validator_account_id", "peer_id"],
        &[],
    );
    let variants =
        contract_property(schemas, "AgentRuntimeReceiptRecord", "execution_host")["anyOf"]
            .as_array()
            .expect("nullable deterministic-validator host")
            .iter()
            .collect::<Vec<_>>();
    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants[0]["$ref"].as_str(),
        Some("#/components/schemas/SoraRuntimeDeterministicValidatorHostV1")
    );
    assert_eq!(variants[1]["type"].as_str(), Some("null"));
    for retired in [
        "SoraRuntimeExecutionHostV1",
        "SoraRuntimeHfModelHostV1",
        "SoraRuntimeInrouReplicaHostV1",
    ] {
        assert!(
            !schemas.contains_key(retired),
            "retired runtime-host schema `{retired}` must not remain public"
        );
    }
}

#[test]
fn soracloud_mailbox_and_agent_request_openapi_are_first_release_exact() {
    let document = canonical_document();
    let schemas = component_schemas(&document);

    assert_strict_object_schema(
        schemas,
        "SoraMailboxContractV1",
        &[
            "queue_name",
            "max_pending_messages",
            "max_message_bytes",
            "retention_blocks",
        ],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SoracloudRuntimeMailboxPlan",
        &[
            "handler_name",
            "queue_name",
            "max_pending_messages",
            "max_message_bytes",
            "retention_blocks",
        ],
        &[],
    );
    for owner in ["SoraMailboxContractV1", "SoracloudRuntimeMailboxPlan"] {
        assert!(
            !component_properties(schemas, owner).contains_key("retention_sequences"),
            "{owner} must reject the retired sequence-clock field"
        );
    }

    assert_strict_object_schema(
        schemas,
        "AgentDeployPayload",
        &["manifest", "lease_blocks", "autonomy_budget_units"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "AgentLeaseRenewPayload",
        &["apartment_name", "lease_blocks"],
        &[],
    );

    let minimum = |owner: &str, property: &str| {
        component_properties(schemas, owner)[property]["minimum"]
            .as_u64()
            .unwrap_or_else(|| panic!("{owner}.{property} minimum"))
    };
    assert_eq!(minimum("SoraMailboxContractV1", "max_message_bytes"), 16);
    assert_eq!(minimum("SoraMailboxContractV1", "retention_blocks"), 1);
    assert_eq!(
        minimum("SoracloudRuntimeMailboxPlan", "max_pending_messages"),
        1
    );
    assert_eq!(
        minimum("SoracloudRuntimeMailboxPlan", "max_message_bytes"),
        16
    );
    assert_eq!(
        minimum("SoracloudRuntimeMailboxPlan", "retention_blocks"),
        1
    );
    assert_eq!(minimum("AgentDeployPayload", "lease_blocks"), 1);
    assert_eq!(minimum("AgentDeployPayload", "autonomy_budget_units"), 1);
    assert_eq!(minimum("AgentLeaseRenewPayload", "lease_blocks"), 1);
}
