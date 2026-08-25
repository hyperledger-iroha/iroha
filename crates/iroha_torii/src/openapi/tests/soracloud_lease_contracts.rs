#[test]
fn soracloud_control_plane_openapi_exposes_authoritative_lease_accounting() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let contracts: &[(&str, &[&str])] = &[
        (
            "ControlPlaneServiceLeaseSnapshot",
            &[
                "authoritative_state",
                "effective_status",
                "remaining_runtime_balance",
            ],
        ),
        (
            "SoraServiceLeaseEgressCheckpointV1",
            &[
                "reporting_epoch",
                "active_service_version",
                "replica_slot",
                "validator_account_id",
                "accounted_egress_bytes",
                "finalize_reporter",
            ],
        ),
        (
            "SoraServiceLeaseReportingEpochRolloverV1",
            &[
                "schema_version",
                "lease_started_height",
                "previous_reporting_epoch",
                "new_reporting_epoch",
                "reporter_account_id",
                "active_service_version",
                "replica_slot",
                "finalized_checkpoint_count",
                "settled_egress_bytes_delta",
                "settled_egress_bytes",
            ],
        ),
        (
            "SoraServiceLeaseStateV1",
            &[
                "schema_version",
                "status",
                "quota_class",
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
    ];
    for (name, fields) in contracts {
        assert_strict_object_schema(schemas, name, fields, &[]);
    }
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
fn soracloud_runtime_execution_host_openapi_is_first_release_exact() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "SoraRuntimeDeterministicValidatorHostV1",
        &["lane_id", "validator_account_id", "peer_id"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "SoraRuntimeHfModelHostV1",
        &[
            "placement_id",
            "source_id",
            "pool_id",
            "selection_seed_hash",
            "validator_account_id",
            "peer_id",
        ],
        &[],
    );
    let variants = schemas["SoraRuntimeExecutionHostV1"]["oneOf"]
        .as_array()
        .expect("runtime execution host variants")
        .iter()
        .map(|variant| {
            (
                variant["properties"]["host_kind"]["const"]
                    .as_str()
                    .expect("runtime host tag")
                    .to_owned(),
                variant["properties"]["value"]["$ref"]
                    .as_str()
                    .expect("runtime host value schema")
                    .to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        variants,
        BTreeSet::from([
            (
                "DeterministicValidator".to_owned(),
                "#/components/schemas/SoraRuntimeDeterministicValidatorHostV1".to_owned(),
            ),
            (
                "HfModelHost".to_owned(),
                "#/components/schemas/SoraRuntimeHfModelHostV1".to_owned(),
            ),
        ])
    );
    assert!(
        !schemas.contains_key("SoraRuntimeInrouReplicaHostV1"),
        "retired Inrou runtime-host schema must not remain public"
    );
}
