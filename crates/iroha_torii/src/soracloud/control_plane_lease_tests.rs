fn fixture_control_plane_service_lease(
    service_version: &str,
    validator_seed: u8,
) -> SoraServiceLeaseStateV1 {
    let validator_public_key = checked_test_keypair(validator_seed).public_key().clone();
    let validator_account_id = AccountId::new(validator_public_key.clone());
    let validator_peer_id = iroha_data_model::peer::PeerId::from(validator_public_key).to_string();
    SoraServiceLeaseStateV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_STATE_VERSION_V1,
        economic_clock:
            iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
        status: SoraServiceLeaseStatusV1::Active,
        quota_class: "taira-open".to_owned(),
        replica_count: std::num::NonZeroU16::new(4).expect("non-zero replica count"),
        deployment_deposit: "1".parse().expect("deployment deposit quantity"),
        prepaid_runtime_balance: "50".parse().expect("prepaid runtime quantity"),
        runtime_price_per_block: "0.00025".parse().expect("runtime price quantity"),
        storage_price_per_gib_block: "0.000025".parse().expect("storage price quantity"),
        egress_price_per_mib: "0.000005".parse().expect("egress price quantity"),
        lease_started_height: 1,
        lease_expires_height: 100,
        reporting_epoch: 3,
        settled_egress_bytes: 1_024,
        egress_reporter_checkpoints: vec![
            iroha_data_model::soracloud::SoraServiceLeaseEgressCheckpointV1 {
                reporting_epoch: 3,
                assignment:
                    iroha_data_model::soracloud::SoraServiceLeaseReporterAssignmentV1 {
                        schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
                        service_version: service_version.to_owned(),
                        placement: iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                            replica_slot: 1,
                            economic_clock: iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
                            lease_started_height: 1,
                            placement_incarnation: Hash::new(b"control-plane-lease-placement"),
                            host_availability: iroha_data_model::soracloud::SoraInrouReplicaHostAvailabilityV1::Available,
                            validator_account_id,
                            peer_id: validator_peer_id,
                            selected_guest_isa:
                                iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
                        },
                        placement_reconciled_at_ms: 1,
                    },
                accounted_egress_bytes: 256,
                last_updated_height: 1,
                finalize_reporter: false,
            },
        ],
        accounted_egress_bytes: 1_280,
        last_status_reason: None,
    }
}

#[test]
fn control_plane_snapshot_projects_full_hosted_service_lease() -> Result<(), eyre::Report> {
    use iroha_core::state::World;

    let mut world = World::default();
    let bundle = fixture_hosted_http_inrou_bundle("2026.08.0");
    let service_name = bundle.service.service_name.clone();
    let lease = fixture_control_plane_service_lease(&bundle.service.service_version, 0x74);
    lease.validate()?;
    insert_revision(&mut world, &bundle, service_name.to_string());
    let mut deployment = fixture_service_deployment(&bundle);
    deployment.service_lease = Some(lease.clone());
    deployment.lease_volume_states = fixture_service_lease_volume_states(&bundle, Some(&lease));
    deployment.validate()?;
    iroha_core::soracloud_runtime::validate_soracloud_deployment_lease_volume_bindings(
        &deployment,
        &bundle,
    )
    .map_err(eyre::Report::msg)?;
    let accounted_storage_bytes = deployment.accounted_storage_bytes()?;
    let mut invalid_deployment = deployment.clone();
    invalid_deployment
        .lease_volume_states
        .pop()
        .expect("hosted deployment has a lease-volume row");
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(service_name.clone(), deployment);
    let mut audit = fixture_service_deploy_audit_event(&bundle);
    audit.sequence = 9;
    audit.block_height = 9;
    audit.block_timestamp_ms = 9;
    world
        .soracloud_service_audit_events_mut_for_testing()
        .insert(audit.sequence, audit.clone());

    let mut invalid_volume_world = World::default();
    insert_revision(&mut invalid_volume_world, &bundle, service_name.to_string());
    invalid_volume_world
        .soracloud_service_deployments_mut_for_testing()
        .insert(service_name.clone(), invalid_deployment);
    invalid_volume_world
        .soracloud_service_audit_events_mut_for_testing()
        .insert(audit.sequence, audit);

    let app = mk_app_state_for_tests_with_world(world);
    let current_height = u64::try_from(app.state.view().height()).unwrap_or(u64::MAX);
    let snapshot = control_plane_snapshot(&app, Some(service_name.as_ref()), 10)?;
    let projected = snapshot.services[0]
        .service_lease
        .as_ref()
        .expect("hosted service lease projection");
    assert_eq!(projected.authoritative_state.reporting_epoch, 3);
    assert_eq!(projected.authoritative_state.settled_egress_bytes, 1_024);
    assert_eq!(projected.authoritative_state.accounted_egress_bytes, 1_280);
    assert_eq!(
        projected.authoritative_state.egress_reporter_checkpoints,
        lease.egress_reporter_checkpoints
    );
    assert_eq!(projected.effective_status, SoraServiceLeaseStatusV1::Active);
    assert_eq!(
        projected.remaining_runtime_balance,
        lease.remaining_balance(current_height, accounted_storage_bytes)?
    );
    let invalid_volume_app = mk_app_state_for_tests_with_world(invalid_volume_world);
    let error = control_plane_snapshot(&invalid_volume_app, Some(service_name.as_ref()), 10)
        .expect_err("control-plane accounting must reject non-exact authoritative storage rows");
    assert!(
        error
            .message
            .contains("invalid authoritative lease-volume state"),
        "unexpected invalid-volume control-plane error: {error:?}"
    );
    Ok(())
}

#[test]
fn control_plane_audit_event_projects_lease_reporting_epoch_rollover() {
    let bundle = fixture_bundle("1.0.0");
    let mut event = fixture_service_deploy_audit_event(&bundle);
    event.sequence = 2;
    event.action = SoraServiceLifecycleActionV1::LeaseReportingEpochRollover;
    event.from_version = Some(event.to_version.clone());
    let reporter_account_id = AccountId::new(event.signer.clone());
    let reporter_peer_id = iroha_data_model::peer::PeerId::from(event.signer.clone()).to_string();
    let placement_incarnation = Hash::new(b"control-plane-rollover-placement");
    event.lease_usage = Some(iroha_data_model::soracloud::SoraServiceLeaseUsageAuditV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_LEASE_USAGE_AUDIT_VERSION_V1,
        reporting_epoch: 4,
        assignment: iroha_data_model::soracloud::SoraServiceLeaseReporterAssignmentV1 {
            schema_version:
                iroha_data_model::soracloud::SORA_SERVICE_LEASE_REPORTER_ASSIGNMENT_VERSION_V1,
            service_version: event.to_version.clone(),
            placement: iroha_data_model::soracloud::SoraInrouReplicaPlacementV1 {
                replica_slot: 2,
                economic_clock:
                    iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
                lease_started_height: 1,
                placement_incarnation,
                host_availability:
                    iroha_data_model::soracloud::SoraInrouReplicaHostAvailabilityV1::Available,
                validator_account_id: reporter_account_id.clone(),
                peer_id: reporter_peer_id,
                selected_guest_isa: iroha_data_model::soracloud::SoraInrouGuestIsaV1::X8664,
            },
            placement_reconciled_at_ms: 1,
        },
        replica_accounted_egress_bytes: 0,
        finalize_reporter: false,
    });
    event.service_lease_commitment = Some(Hash::new(b"rollover lease state"));
    event.lease_reporting_epoch_rollover = Some(SoraServiceLeaseReportingEpochRolloverV1 {
        schema_version:
            iroha_data_model::soracloud::SORA_SERVICE_LEASE_REPORTING_EPOCH_ROLLOVER_VERSION_V1,
        economic_clock: iroha_data_model::soracloud::SoraServiceLeaseClockV1::CanonicalBlockHeight,
        lease_started_height: 1,
        previous_reporting_epoch: 3,
        new_reporting_epoch: 4,
        reporter_account_id,
        active_service_version: event.to_version.clone(),
        replica_slot: 2,
        placement_incarnation,
        finalized_checkpoint_count: u32::try_from(
            iroha_data_model::soracloud::SORA_SERVICE_LEASE_MAX_EGRESS_REPORTER_CHECKPOINTS_V1,
        )
        .expect("reporter checkpoint limit fits u32"),
        settled_egress_bytes_delta: 4_096,
        settled_egress_bytes: 8_192,
    });
    event
        .validate()
        .expect("valid reporting epoch rollover event");

    let projected = audit_event_to_control_plane_audit_event(&event);
    assert_eq!(
        projected.action,
        SoracloudAction::LeaseReportingEpochRollover
    );
    assert_eq!(
        projected.lease_reporting_epoch_rollover,
        event.lease_reporting_epoch_rollover
    );
    let value = norito::json::to_value(&projected).expect("serialize control-plane audit event");
    let rollover = value
        .as_object()
        .and_then(|event| event.get("lease_reporting_epoch_rollover"))
        .and_then(norito::json::Value::as_object)
        .expect("typed reporting epoch rollover payload");
    assert_eq!(
        rollover
            .get("new_reporting_epoch")
            .and_then(norito::json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        rollover
            .get("settled_egress_bytes")
            .and_then(norito::json::Value::as_u64),
        Some(8_192)
    );
}

#[test]
fn control_plane_snapshot_serializes_authoritative_lease_accounting() {
    let mut authoritative_state = fixture_control_plane_service_lease("1.0.0", 0x73);
    authoritative_state.prepaid_runtime_balance =
        "340282366920938463463374607431768211456.0000000001"
            .parse()
            .expect("wide exact prepaid quantity");
    authoritative_state.reporting_epoch = 7;
    authoritative_state.settled_egress_bytes = 2_048;
    authoritative_state.egress_reporter_checkpoints[0].reporting_epoch = 7;
    authoritative_state.egress_reporter_checkpoints[0].accounted_egress_bytes = 512;
    authoritative_state.accounted_egress_bytes = 2_560;
    let snapshot = ControlPlaneServiceSnapshot {
        service_name: "web_portal".to_owned(),
        current_version: "1.0.0".to_owned(),
        revision_count: 1,
        config_generation: 0,
        secret_generation: 0,
        config_entry_count: 0,
        secret_entry_count: 0,
        service_lease: Some(ControlPlaneServiceLeaseSnapshot {
            authoritative_state,
            effective_status: SoraServiceLeaseStatusV1::Active,
            remaining_runtime_balance: "0.0000000000000000000000000001"
                .parse()
                .expect("scale-28 remaining quantity"),
        }),
        public_discovery_content_cid: None,
        public_discovery_url: None,
        public_discovery_cid_host_url: None,
        latest_revision: None,
        active_rollout: None,
        last_rollout: None,
    };
    let value = norito::json::to_value(&snapshot).expect("serialize control-plane snapshot");
    let object = value.as_object().expect("control-plane snapshot object");
    let lease = object["service_lease"]
        .as_object()
        .expect("typed service lease snapshot");
    let state = lease["authoritative_state"]
        .as_object()
        .expect("authoritative service lease state");
    assert!(object["active_rollout"].is_null());
    assert!(object["last_rollout"].is_null());
    assert_eq!(
        state["prepaid_runtime_balance"].as_str(),
        Some("340282366920938463463374607431768211456.0000000001")
    );
    assert_eq!(state["reporting_epoch"].as_u64(), Some(7));
    assert_eq!(state["settled_egress_bytes"].as_u64(), Some(2_048));
    assert_eq!(state["accounted_egress_bytes"].as_u64(), Some(2_560));
    assert_eq!(
        state["egress_reporter_checkpoints"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        lease["remaining_runtime_balance"].as_str(),
        Some("0.0000000000000000000000000001")
    );
    for retired in [
        "quota_class",
        "service_lease_status",
        "lease_expires_height",
        "prepaid_runtime_balance",
        "remaining_runtime_balance",
        "prepaid_runtime_balance_nanos",
        "remaining_runtime_balance_nanos",
    ] {
        assert!(
            !object.contains_key(retired),
            "retired flattened field {retired}"
        );
    }
}
