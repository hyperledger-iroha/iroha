//! Validate persisted Soracloud runtime, rollout, mailbox and agent state.

use super::*;

macro_rules! zero_prehash_field_rejection_test {
    ($name:ident, $zero:ident, $factory:expr; $($field:ident = $value:expr => ($reported:literal, $message:literal);)+) => {
        #[test]
        fn $name() {
            let $zero = zero_prehash_statement_hash();
            $(
                let mut subject = $factory;
                subject.$field = $value;
                let error = subject.validate().expect_err($message);
                assert_zero_prehash_digest_error(&error, $reported);
            )+
        }
    };
}

#[test]
fn service_runtime_state_validate_rejects_load_out_of_range() {
    let runtime_state = SoraServiceRuntimeStateV1 {
        schema_version: SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        active_service_version: "2026.1".to_string(),
        health_status: SoraServiceHealthStatusV1::Healthy,
        load_factor_bps: 10_001,
        materialized_bundle_hash: sample_hash(160),
    };
    let error = runtime_state
        .validate()
        .expect_err("load factor above 10_000 bps must fail");
    assert_soracloud_invalid_field(error, "load_factor_bps");
}
zero_prehash_field_rejection_test! {
    service_runtime_state_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_runtime_state();
    materialized_bundle_hash = zero_digest =>
        ("materialized_bundle_hash", "materialized bundle placeholder hash must fail admission");
}
#[test]
fn inrou_host_capability_record_validate_accepts_hosting_advert() {
    sample_inrou_host_capability_record()
        .validate()
        .expect("valid Inrou host capability advert should pass");
}
#[test]
fn inrou_host_capability_record_validate_accepts_exact_minimum_physical_capacity() {
    let mut capability = sample_inrou_host_capability_record();
    capability.max_cpu_millis = u32::try_from(
        u64::from(SORA_INROU_MIN_CPU_MILLIS_V1) + SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1,
    )
    .expect("minimum physical CPU capacity fits u32");
    capability.max_memory_bytes =
        SORA_INROU_MIN_MEMORY_BYTES_V1 + SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1;
    capability.max_storage_bytes = SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1;
    capability
        .validate()
        .expect("an advert covering exactly one minimum guest and VMM must validate");
}
#[test]
fn inrou_host_capability_record_validate_rejects_one_below_physical_minima() {
    let minimum_cpu_millis = u32::try_from(
        u64::from(SORA_INROU_MIN_CPU_MILLIS_V1) + SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1,
    )
    .expect("minimum physical CPU capacity fits u32");
    let minimum_memory_bytes =
        SORA_INROU_MIN_MEMORY_BYTES_V1 + SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1;
    let minimum_storage_bytes = SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1;
    let cases = [
        {
            let mut capability = sample_inrou_host_capability_record();
            capability.max_cpu_millis = minimum_cpu_millis - 1;
            (capability, "max_cpu_millis")
        },
        {
            let mut capability = sample_inrou_host_capability_record();
            capability.max_memory_bytes = minimum_memory_bytes - 1;
            (capability, "max_memory_bytes")
        },
        {
            let mut capability = sample_inrou_host_capability_record();
            capability.max_storage_bytes = minimum_storage_bytes - 1;
            (capability, "max_storage_bytes")
        },
    ];
    for (capability, field) in cases {
        let error = capability
            .validate()
            .expect_err("one unit below a physical host minimum must fail closed");
        assert_soracloud_invalid_field(error, field);
        assert!(
            !capability.can_host_replicas_at(capability.advertised_at_ms),
            "an advert below {field} minimum must never remain placement-eligible"
        );
    }
}
#[test]
fn inrou_host_capability_record_validate_rejects_zero_capacity() {
    let mut capability = sample_inrou_host_capability_record();
    capability.max_hosted_replica_capacity = 0;
    let error = capability
        .validate()
        .expect_err("zero-capacity adverts must fail");
    assert_soracloud_invalid_field(error, "max_hosted_replica_capacity");
}
#[test]
fn inrou_host_capability_record_validate_rejects_capacity_above_v1() {
    let mut capability = sample_inrou_host_capability_record();
    capability.max_hosted_replica_capacity = SORA_INROU_HOSTED_REPLICA_CAPACITY_V1 + 1;
    let error = capability
        .validate()
        .expect_err("multi-replica Inrou host adverts must fail in V1");
    assert_soracloud_invalid_field(error, "max_hosted_replica_capacity");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "invalid multi-replica adverts must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_validate_rejects_multiple_guest_isas() {
    let mut capability = sample_inrou_host_capability_record();
    capability
        .supported_guest_isas
        .insert(SoraInrouGuestIsaV1::Aarch64);
    let error = capability
        .validate()
        .expect_err("one Inrou V1 host advert cannot alias multiple guest ISAs");
    assert_soracloud_invalid_field(error, "supported_guest_isas");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "invalid multi-ISA adverts must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_validate_rejects_noncanonical_peer_id() {
    let mut capability = sample_inrou_host_capability_record();
    capability.peer_id = "12D3KooWLegacyAlias".to_owned();
    let error = capability
        .validate()
        .expect_err("noncanonical Inrou peer aliases must fail");
    assert_soracloud_invalid_field(error, "peer_id");
    assert!(
        !capability.can_host_replicas_at(capability.advertised_at_ms),
        "an invalid peer route must never remain placement-eligible"
    );
}
#[test]
fn inrou_host_capability_record_accepts_independent_consensus_peer() {
    let mut capability = sample_inrou_host_capability_record();
    capability.peer_id = sample_bls_peer_id(0xD2);
    capability
        .validate()
        .expect("the authoritative validator record, not key derivation, binds account and peer");
    assert!(
        capability.can_host_replicas_at(capability.advertised_at_ms),
        "a separately canonical consensus peer remains placement-eligible"
    );
}
#[test]
fn inrou_service_placement_record_validate_rejects_duplicate_slots() {
    let mut placement = sample_inrou_service_placement_record();
    placement.placements.push(placement.placements[0].clone());
    let error = placement
        .validate()
        .expect_err("duplicate replica slots must fail validation");
    assert_soracloud_invalid_field(error, "placements");
}
#[test]
fn inrou_service_placement_record_accepts_independent_consensus_peer() {
    let mut placement = sample_inrou_service_placement_record();
    placement.placements[0].peer_id = sample_bls_peer_id(0xD2);
    placement
        .validate()
        .expect("placement records carry the independently authoritative consensus peer");
}
#[test]
fn inrou_replica_runtime_state_validate_rejects_missing_peer_id() {
    let mut runtime_state = sample_inrou_replica_runtime_state();
    runtime_state.peer_id.clear();
    let error = runtime_state
        .validate()
        .expect_err("empty peer_id must fail validation");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            field: "peer_id",
            ..
        }
    ));
}
#[test]
fn inrou_replica_runtime_state_accepts_independent_consensus_peer() {
    let mut runtime_state = sample_inrou_replica_runtime_state();
    runtime_state.peer_id = sample_bls_peer_id(0xD2);
    runtime_state
        .validate()
        .expect("runtime state carries the peer bound by the active validator record");
}
zero_prehash_field_rejection_test! {
    inrou_replica_runtime_state_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_inrou_replica_runtime_state();
    materialized_bundle_hash = zero_digest =>
        ("materialized_bundle_hash", "materialized bundle placeholder hash must fail admission");
}
#[test]
fn service_rollout_state_validate_rejects_promoted_partial_traffic() {
    let rollout = SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:1".to_string(),
        baseline_version: "1.0.0".to_string(),
        candidate_version: "1.1.0".to_string(),
        canary_percent: 10,
        traffic_percent: 50,
        stage: SoraRolloutStageV1::Promoted,
        health_failures: 0,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 1,
        updated_sequence: 1,
    };
    let error = rollout
        .validate()
        .expect_err("promoted rollouts must serve 100 percent of traffic");
    assert_soracloud_invalid_field(error, "traffic_percent");
}
fn sample_active_rollout_deployment() -> SoraServiceDeploymentStateV1 {
    let mut deployment = sample_service_deployment_state();
    deployment.active_rollout = Some(SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:7".to_string(),
        baseline_version: "1.0.0".to_string(),
        candidate_version: deployment.current_service_version.clone(),
        canary_percent: 25,
        traffic_percent: 25,
        stage: SoraRolloutStageV1::Canary,
        health_failures: 0,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 7,
        updated_sequence: 7,
    });
    deployment.last_rollout = deployment.active_rollout.clone();
    deployment
}
#[test]
fn service_rollout_state_validate_rejects_missing_or_reused_baseline() {
    for baseline_version in ["", "1.1.0"] {
        let mut deployment = sample_active_rollout_deployment();
        deployment
            .active_rollout
            .as_mut()
            .expect("active rollout")
            .baseline_version = baseline_version.to_owned();
        let error = deployment
            .validate()
            .expect_err("baseline must be present and distinct from the candidate");
        if baseline_version.is_empty() {
            assert_eq!(
                error,
                SoracloudManifestError::EmptyField {
                    manifest: "sora service rollout state",
                    field: "baseline_version",
                }
            );
        } else {
            assert_soracloud_invalid_field(error, "baseline_version");
        }
    }
}
#[test]
fn service_deployment_state_validate_rejects_active_candidate_different_from_current() {
    let mut deployment = sample_active_rollout_deployment();
    deployment
        .active_rollout
        .as_mut()
        .expect("active rollout")
        .candidate_version = "1.2.0".to_owned();
    let error = deployment
        .validate()
        .expect_err("active candidate must be the deployment current version");
    assert_soracloud_invalid_field(error, "active_rollout.candidate_version");
}
#[test]
fn service_deployment_state_validate_rejects_zero_or_full_canary_allocations() {
    for canary_percent in [0, 100] {
        let mut deployment = sample_active_rollout_deployment();
        let rollout = deployment.active_rollout.as_mut().expect("active rollout");
        rollout.canary_percent = canary_percent;
        rollout.traffic_percent = canary_percent;
        let error = deployment
            .validate()
            .expect_err("active canary policy must use a partial nonzero allocation");
        assert_soracloud_invalid_field(error, "canary_percent");
    }
    for traffic_percent in [0, 100] {
        let mut deployment = sample_active_rollout_deployment();
        deployment
            .active_rollout
            .as_mut()
            .expect("active rollout")
            .traffic_percent = traffic_percent;
        let error = deployment
            .validate()
            .expect_err("active canary traffic must use a partial nonzero allocation");
        assert_soracloud_invalid_field(error, "traffic_percent");
    }
}
fn assert_deployment_and_rollout_json_fields(deployment: &SoraServiceDeploymentStateV1) {
    {
        let canonical = norito::json::to_value(deployment).expect("serialize deployment state");
        for field in [
            "config_generation",
            "secret_generation",
            "service_configs",
            "service_secrets",
            "active_rollout",
            "last_rollout",
            "service_lease",
            "lease_volume_states",
        ] {
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("deployment JSON object")
                    .remove(field)
                    .is_some(),
                "canonical deployment must emit `{field}`"
            );
            norito::json::from_value::<SoraServiceDeploymentStateV1>(missing)
                .expect_err("deployment state must reject every omitted V1 field");
        }
        let mut unknown = canonical;
        unknown
            .as_object_mut()
            .expect("deployment JSON object")
            .insert("retired_v0".to_owned(), norito::json!(true));
        norito::json::from_value::<SoraServiceDeploymentStateV1>(unknown)
            .expect_err("deployment state must reject unknown fields");

        let rollout = deployment.active_rollout.as_ref().expect("active rollout");
        let mut missing_baseline =
            norito::json::to_value(rollout).expect("serialize rollout state");
        assert!(
            missing_baseline
                .as_object_mut()
                .expect("rollout JSON object")
                .remove("baseline_version")
                .is_some()
        );
        norito::json::from_value::<SoraServiceRolloutStateV1>(missing_baseline)
            .expect_err("rollout state must require the nullable baseline key");
    }
}

#[test]
fn service_deployment_state_validate_requires_exact_active_canary_relation() {
    let mut deployment = SoraServiceDeploymentStateV1 {
        schema_version: SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        service_name: "portal".parse().expect("valid name"),
        current_service_version: "1.1.0".to_string(),
        current_service_manifest_hash: sample_hash(170),
        current_container_manifest_hash: sample_hash(171),
        revision_count: 2,
        process_generation: 2,
        process_started_sequence: 7,
        active_rollout: Some(SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: "portal:rollout:7".to_string(),
            baseline_version: "1.0.0".to_string(),
            candidate_version: "1.1.0".to_string(),
            canary_percent: 25,
            traffic_percent: 100,
            stage: SoraRolloutStageV1::Promoted,
            health_failures: 0,
            max_health_failures: 2,
            health_window_secs: 30,
            created_sequence: 7,
            updated_sequence: 7,
        }),
        last_rollout: None,
        config_generation: 0,
        secret_generation: 0,
        service_configs: BTreeMap::new(),
        service_secrets: BTreeMap::new(),
        fhe_policy_records: BTreeMap::new(),
        service_lease: None,
        lease_volume_states: Vec::new(),
    };
    let error = deployment
        .validate()
        .expect_err("active rollout must remain in canary state");
    assert_soracloud_invalid_field(error, "active_rollout.stage");

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.stage = SoraRolloutStageV1::Canary;
    active_rollout.traffic_percent = 25;
    active_rollout.candidate_version = "1.2.0".to_owned();
    let error = deployment
        .validate()
        .expect_err("active rollout candidate must equal the current revision");
    assert_soracloud_invalid_field(error, "active_rollout.candidate_version");

    let current_version = deployment.current_service_version.clone();
    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.candidate_version = current_version.clone();
    active_rollout.baseline_version.clear();
    let error = deployment
        .validate()
        .expect_err("active rollout must name its baseline revision");
    assert_eq!(
        error,
        SoracloudManifestError::EmptyField {
            manifest: "sora service rollout state",
            field: "baseline_version",
        }
    );

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.baseline_version = current_version;
    let error = deployment
        .validate()
        .expect_err("active rollout baseline and candidate must differ");
    assert_soracloud_invalid_field(error, "baseline_version");

    let active_rollout = deployment.active_rollout.as_mut().expect("active rollout");
    active_rollout.baseline_version = "1.0.0".to_owned();
    active_rollout.traffic_percent = 0;
    let error = deployment
        .validate()
        .expect_err("active canary traffic may not be zero");
    assert_soracloud_invalid_field(error, "traffic_percent");

    deployment
        .active_rollout
        .as_mut()
        .expect("active rollout")
        .traffic_percent = 25;
    deployment.last_rollout = deployment.active_rollout.clone();
    deployment
        .validate()
        .expect("exact active canary relation must pass");

    assert_deployment_and_rollout_json_fields(&deployment);
}
zero_prehash_field_rejection_test! {
    service_deployment_state_validate_rejects_zero_prehash_manifest_hash_sentinels,
    zero_digest,
    sample_service_deployment_state();
    current_service_manifest_hash = zero_digest =>
        ("current_service_manifest_hash", "current service manifest placeholder hash must fail admission");
    current_container_manifest_hash = zero_digest =>
        ("current_container_manifest_hash", "current container manifest placeholder hash must fail admission");
}
#[test]
fn service_audit_event_validate_rejects_zero_sequence() {
    let event = SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 0,
        block_height: 1,
        block_timestamp_ms: 1,
        action: SoraServiceLifecycleActionV1::Deploy,
        service_name: "portal".parse().expect("valid name"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(172),
        container_manifest_hash: sample_hash(173),
        process_generation: 1,
        config_generation: 0,
        secret_generation: 0,
        config_snapshot_hash: derive_soracloud_service_config_snapshot_hash_v1(&BTreeMap::new()),
        secret_snapshot_hash: derive_soracloud_service_secret_snapshot_hash_v1(&BTreeMap::new()),
        governance_tx_hash: None,
        binding_name: None,
        state_key: None,
        config_mutations: Vec::new(),
        secret_mutations: Vec::new(),
        rollout_state: None,
        policy_name: None,
        policy_snapshot_hash: None,
        jurisdiction_tag: None,
        consent_evidence_hash: None,
        break_glass: None,
        break_glass_reason: None,
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signer: sample_signer(),
    };
    let error = event
        .validate()
        .expect_err("audit sequences must be greater than zero");
    assert_soracloud_invalid_field(error, "sequence");
}
zero_prehash_field_rejection_test! {
    service_audit_event_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_audit_event();
    service_manifest_hash = zero_digest =>
        ("service_manifest_hash", "service manifest placeholder hash must fail admission");
    container_manifest_hash = zero_digest =>
        ("container_manifest_hash", "container manifest placeholder hash must fail admission");
    governance_tx_hash = Some(zero_digest) =>
        ("governance_tx_hash", "governance transaction placeholder hash must fail admission");
    policy_snapshot_hash = Some(zero_digest) =>
        ("policy_snapshot_hash", "policy snapshot placeholder hash must fail admission");
    consent_evidence_hash = Some(zero_digest) =>
        ("consent_evidence_hash", "consent evidence placeholder hash must fail admission");
    config_snapshot_hash = zero_digest =>
        ("config_snapshot_hash", "config snapshot placeholder hash must fail admission");
    secret_snapshot_hash = zero_digest =>
        ("secret_snapshot_hash", "secret snapshot placeholder hash must fail admission");
}
#[test]
fn service_state_entry_validate_allows_plaintext_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::Plaintext;
    entry
        .validate()
        .expect("state entries must support plaintext bindings");
}
#[test]
fn service_state_entry_validate_rejects_fhe_residual_bound_on_non_fhe_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    entry.fhe_residual_multiple_bound = Some(17);
    let error = entry
        .validate()
        .expect_err("BFV residual bounds must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
}
#[test]
fn service_state_entry_validate_rejects_fhe_public_key_digest_on_non_fhe_rows() {
    let mut entry = sample_state_entry();
    entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    entry.fhe_public_key_digest = Some(sample_hash(149));
    let error = entry
        .validate()
        .expect_err("BFV public-key digests must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_public_key_digest");
}
#[test]
fn service_state_entry_validate_rejects_zero_fhe_public_key_digest() {
    let mut entry = sample_state_entry();
    entry.fhe_public_key_digest = Some(zero_prehash_statement_hash());
    let error = entry
        .validate()
        .expect_err("BFV public-key digest placeholders must fail admission");
    assert_zero_prehash_digest_error(&error, "fhe_public_key_digest");
}
#[test]
fn service_state_entry_validate_rejects_zero_prehash_governance_hash_sentinel() {
    let mut entry = sample_state_entry();
    entry.governance_tx_hash = zero_prehash_statement_hash();
    let error = entry
        .validate()
        .expect_err("governance transaction placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "governance_tx_hash");
}
#[test]
fn service_state_entry_validate_rejects_fhe_bound_mode_without_fhe_bound() {
    let mut non_fhe_entry = sample_state_entry();
    non_fhe_entry.encryption = SoraStateEncryptionV1::ClientCiphertext;
    non_fhe_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = non_fhe_entry
        .validate()
        .expect_err("BFV bound modes must only annotate FHE rows");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
    let mut missing_bound_entry = sample_state_entry();
    missing_bound_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = missing_bound_entry
        .validate()
        .expect_err("BFV bound mode must require a bound value");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
}
#[test]
fn service_state_entry_validate_rejects_over_capacity_fhe_bounds() {
    let mut missing_mode_entry = sample_state_entry();
    missing_mode_entry.fhe_public_key_digest = Some(sample_hash(150));
    missing_mode_entry.fhe_residual_multiple_bound = Some(17);
    missing_mode_entry.fhe_bound_mode = None;
    let error = missing_mode_entry
        .validate()
        .expect_err("FHE bounds must explicitly advertise their semantics");
    assert_soracloud_invalid_field(error, "fhe_bound_mode");
    let mut exact_entry = sample_state_entry();
    exact_entry.fhe_public_key_digest = Some(sample_hash(151));
    exact_entry.fhe_residual_multiple_bound = Some(u128::MAX);
    exact_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::ExactResidualMultiple);
    let error = exact_entry
        .validate()
        .expect_err("over-capacity exact FHE bound must be rejected");
    assert!(
        error.to_string().contains("exact residual"),
        "unexpected error: {error}"
    );
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
    let mut bounded_entry = sample_state_entry();
    bounded_entry.fhe_public_key_digest = Some(sample_hash(152));
    bounded_entry.fhe_residual_multiple_bound = Some(u128::MAX);
    bounded_entry.fhe_bound_mode = Some(BfvCiphertextBoundModeV1::BoundedNoise);
    let error = bounded_entry
        .validate()
        .expect_err("bounded-noise FHE bound above capacity must be rejected");
    assert!(
        error.to_string().contains("bounded-noise"),
        "unexpected error: {error}"
    );
    assert_soracloud_invalid_field(error, "fhe_residual_multiple_bound");
}
#[test]
fn decryption_request_record_policy_snapshot_hash_is_deterministic() {
    let record = sample_decryption_request_record();
    let expected = Hash::new(Encode::encode(&record.policy));
    assert_eq!(record.policy_snapshot_hash(), expected);
    record
        .validate()
        .expect("request record should validate against its policy");
}
#[test]
fn service_audit_event_validate_requires_break_glass_reason_when_enabled() {
    let event = SoraServiceAuditEventV1 {
        schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
        sequence: 1,
        block_height: 1,
        block_timestamp_ms: 1,
        action: SoraServiceLifecycleActionV1::DecryptionRequest,
        service_name: "portal".parse().expect("valid name"),
        from_version: None,
        to_version: "1.0.0".to_string(),
        service_manifest_hash: sample_hash(174),
        container_manifest_hash: sample_hash(175),
        process_generation: 1,
        config_generation: 0,
        secret_generation: 0,
        config_snapshot_hash: derive_soracloud_service_config_snapshot_hash_v1(&BTreeMap::new()),
        secret_snapshot_hash: derive_soracloud_service_secret_snapshot_hash_v1(&BTreeMap::new()),
        governance_tx_hash: Some(sample_hash(176)),
        binding_name: Some("private_state".parse().expect("valid name")),
        state_key: Some("/state/private/patient-1".to_string()),
        config_mutations: Vec::new(),
        secret_mutations: Vec::new(),
        rollout_state: None,
        policy_name: Some("phi_threshold_policy".parse().expect("valid name")),
        policy_snapshot_hash: Some(sample_hash(177)),
        jurisdiction_tag: Some("us_hipaa".to_string()),
        consent_evidence_hash: None,
        break_glass: Some(true),
        break_glass_reason: None,
        lease_usage: None,
        service_lease_commitment: None,
        lease_reporting_epoch_rollover: None,
        signer: sample_signer(),
    };
    let error = event
        .validate()
        .expect_err("break_glass events require a reason");
    assert_soracloud_invalid_field(error, "break_glass_reason");
}
#[test]
fn service_mailbox_message_validate_rejects_expired_message() {
    let message = SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: sample_hash(162),
        from_service: "portal".parse().expect("valid name"),
        from_service_version: "2026.1".to_string(),
        from_handler: "update".parse().expect("valid name"),
        to_service: "audit".parse().expect("valid name"),
        to_service_version: "2026.1".to_string(),
        to_handler: "ciphertext_update".parse().expect("valid name"),
        payload_bytes: b"ciphertext".to_vec(),
        payload_commitment: Hash::new(b"ciphertext"),
        delivery_delay_blocks: 2,
        enqueue_sequence: 10,
        enqueue_height: 10,
        available_after_height: 12,
        expires_at_height: 12,
    };
    let error = message
        .validate()
        .expect_err("message expiry must be after availability");
    assert_soracloud_invalid_field(error, "expires_at_height");
}
#[test]
fn service_mailbox_message_validate_rejects_payload_commitment_mismatch() {
    let message = SoraServiceMailboxMessageV1 {
        schema_version: SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: sample_hash(162),
        from_service: "portal".parse().expect("valid name"),
        from_service_version: "2026.1".to_string(),
        from_handler: "update".parse().expect("valid name"),
        to_service: "audit".parse().expect("valid name"),
        to_service_version: "2026.1".to_string(),
        to_handler: "ciphertext_update".parse().expect("valid name"),
        payload_bytes: b"ciphertext".to_vec(),
        payload_commitment: sample_hash(163),
        delivery_delay_blocks: 0,
        enqueue_sequence: 10,
        enqueue_height: 10,
        available_after_height: 10,
        expires_at_height: 12,
    };
    let error = message
        .validate()
        .expect_err("message commitment must bind the authoritative payload bytes");
    assert_soracloud_invalid_field(error, "payload_commitment");
}
#[test]
fn service_mailbox_message_validation_separates_submission_and_persisted_schedule_states() {
    let mut message = sample_service_mailbox_message();
    message
        .validate()
        .expect("canonical ledger-assigned mailbox message must validate");
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry a caller-selected identifier");
    assert_soracloud_invalid_field(error, "message_id");

    message.message_id = Hash::prehashed([0; Hash::LENGTH]);
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry ledger-bound service versions");
    assert_soracloud_invalid_field(error, "from_service_version");

    message.from_service_version.clear();
    message.to_service_version.clear();
    let error = message
        .validate_submission()
        .expect_err("mailbox submission must not carry a caller-selected schedule");
    assert_soracloud_invalid_field(error, "enqueue_sequence");

    message.enqueue_sequence = 0;
    message.enqueue_height = 0;
    message.available_after_height = 0;
    message.expires_at_height = 0;
    message
        .validate_submission()
        .expect("zero-sentinel mailbox submission must validate");
    message.message_id = sample_hash(162);
    message.from_service_version = "2026.1".to_owned();
    message.to_service_version = "2026.1".to_owned();
    let error = message
        .validate()
        .expect_err("persisted mailbox message requires a ledger-assigned schedule");
    assert_soracloud_invalid_field(error, "enqueue_sequence");
}
#[test]
fn service_mailbox_message_id_binds_every_immutable_field() {
    let message = sample_service_mailbox_message();
    let canonical_id = message.message_id;

    macro_rules! assert_field_bound {
        ($field:literal, $mutate:expr) => {{
            let mut changed = message.clone();
            ($mutate)(&mut changed);
            assert_ne!(
                derive_soracloud_mailbox_message_id_v1(&changed),
                canonical_id,
                "mailbox message identity must bind {}",
                $field
            );
        }};
    }
    assert_field_bound!(
        "schema_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.schema_version += 1;
        }
    );
    assert_field_bound!(
        "from_service",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_service = "other_source".parse().expect("valid source name");
        }
    );
    assert_field_bound!(
        "from_service_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_service_version = "2026.2".to_owned();
        }
    );
    assert_field_bound!(
        "from_handler",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.from_handler = "other_update".parse().expect("valid handler name");
        }
    );
    assert_field_bound!("to_service", |changed: &mut SoraServiceMailboxMessageV1| {
        changed.to_service = "other_destination".parse().expect("valid service name");
    });
    assert_field_bound!(
        "to_service_version",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.to_service_version = "2026.2".to_owned();
        }
    );
    assert_field_bound!("to_handler", |changed: &mut SoraServiceMailboxMessageV1| {
        changed.to_handler = "other_handler".parse().expect("valid handler name");
    });
    assert_field_bound!(
        "payload_bytes",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.payload_bytes = b"other ciphertext".to_vec();
        }
    );
    assert_field_bound!(
        "payload_commitment",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.payload_commitment = sample_hash(163);
        }
    );
    assert_field_bound!(
        "delivery_delay_blocks",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.delivery_delay_blocks = 1;
        }
    );
    assert_field_bound!(
        "enqueue_sequence",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.enqueue_sequence += 1;
        }
    );
    assert_field_bound!(
        "available_after_height",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.available_after_height += 1;
        }
    );
    assert_field_bound!(
        "expires_at_height",
        |changed: &mut SoraServiceMailboxMessageV1| {
            changed.expires_at_height += 1;
        }
    );

    let mut substituted = message;
    substituted.payload_bytes = b"attacker-selected ciphertext".to_vec();
    substituted.payload_commitment = Hash::new(&substituted.payload_bytes);
    assert_eq!(substituted.message_id, canonical_id);
    let error = substituted
        .validate()
        .expect_err("payload substitution under a canonical message id must fail");
    assert_soracloud_invalid_field(error, "message_id");
}
zero_prehash_field_rejection_test! {
    service_mailbox_message_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_service_mailbox_message();
    message_id = zero_digest =>
        ("message_id", "message placeholder id must fail admission");
    payload_commitment = zero_digest =>
        ("payload_commitment", "payload placeholder commitment must fail admission");
}
#[test]
fn runtime_receipt_validate_rejects_uncertified_query_receipt() {
    let receipt = SoraRuntimeReceiptV1 {
        schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: sample_hash(164),
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        handler_name: "query".parse().expect("valid name"),
        handler_class: SoraServiceHandlerClassV1::Query,
        request_commitment: sample_hash(165),
        result_commitment: sample_hash(166),
        certified_by: SoraCertifiedResponsePolicyV1::None,
        emitted_sequence: 44,
        execution_host: None,
        mailbox_message_id: None,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
    };
    let error = receipt
        .validate()
        .expect_err("query receipts must remain certified");
    assert_soracloud_invalid_field(error, "certified_by");
}
#[test]
fn runtime_receipt_validation_separates_submission_and_persisted_sequence_states() {
    let mut receipt = sample_runtime_receipt();
    receipt.emitted_sequence = 0;
    receipt
        .validate_submission()
        .expect("an unassigned runtime receipt is valid for ledger submission");
    let error = receipt
        .validate()
        .expect_err("a persisted runtime receipt requires a ledger-assigned sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
    receipt.emitted_sequence = 1;
    let error = receipt
        .validate_submission()
        .expect_err("a submission must not select its authoritative sequence");
    assert_soracloud_invalid_field(error, "emitted_sequence");
}

#[test]
fn runtime_receipt_validate_rejects_invalid_host_attribution() {
    let mut receipt = SoraRuntimeReceiptV1 {
        schema_version: SORA_RUNTIME_RECEIPT_VERSION_V1,
        receipt_id: sample_hash(167),
        service_name: "portal".parse().expect("valid name"),
        service_version: "2026.1".to_string(),
        handler_name: "query".parse().expect("valid name"),
        handler_class: SoraServiceHandlerClassV1::Query,
        request_commitment: sample_hash(168),
        result_commitment: sample_hash(169),
        certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
        emitted_sequence: 45,
        execution_host: Some(SoraRuntimeDeterministicValidatorHostV1 {
            lane_id: LaneId::SINGLE,
            validator_account_id: sample_account_id(171),
            peer_id: " ".to_owned(),
        }),
        mailbox_message_id: None,
        journal_artifact_hash: None,
        checkpoint_artifact_hash: None,
    };
    let error = receipt
        .validate()
        .expect_err("invalid host attribution must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            manifest: "sora runtime deterministic validator host",
            field: "peer_id",
        }
    ));
    receipt
        .execution_host
        .as_mut()
        .expect("fixture carries host attribution")
        .peer_id = "invalid-peer-key".to_owned();
    let error = receipt
        .validate()
        .expect_err("malformed host attribution must be rejected");
    assert_soracloud_invalid_field(error, "peer_id");
}
#[test]
fn deterministic_validator_host_accepts_independently_canonical_account_and_peer() {
    let mut receipt = sample_runtime_receipt();
    receipt.execution_host = Some(SoraRuntimeDeterministicValidatorHostV1 {
        lane_id: LaneId::SINGLE,
        validator_account_id: sample_account_id(171),
        peer_id: sample_peer_id(171),
    });
    receipt
        .validate()
        .expect("matching single-signatory validator host must validate");

    let host = receipt
        .execution_host
        .as_mut()
        .expect("fixture carries deterministic-validator attribution");
    host.peer_id = sample_bls_peer_id(172);
    receipt
        .validate()
        .expect("the ledger validator record, not account-key derivation, binds the peer");
}
zero_prehash_field_rejection_test! {
    runtime_receipt_validate_rejects_zero_prehash_digest_sentinels,
    zero_digest,
    sample_runtime_receipt();
    receipt_id = zero_digest => ("receipt_id", "receipt placeholder id must fail admission");
    request_commitment = zero_digest =>
        ("request_commitment", "request placeholder commitment must fail admission");
    result_commitment = zero_digest =>
        ("result_commitment", "result placeholder commitment must fail admission");
    mailbox_message_id = Some(zero_digest) =>
        ("mailbox_message_id", "mailbox message placeholder id must fail admission");
    journal_artifact_hash = Some(zero_digest) =>
        ("journal_artifact_hash", "journal artifact placeholder hash must fail admission");
    checkpoint_artifact_hash = Some(zero_digest) =>
        ("checkpoint_artifact_hash", "checkpoint artifact placeholder hash must fail admission");
}

#[test]
fn runtime_receipt_host_attribution_rejects_unknown_fields() {
    let mut value = norito::json::to_value(&sample_runtime_receipt())
        .expect("serialize runtime receipt with host attribution");
    value
        .get_mut("execution_host")
        .and_then(norito::json::Value::as_object_mut)
        .expect("execution host JSON object")
        .insert("retired_v0".to_owned(), norito::json!(true));
    norito::json::from_value::<SoraRuntimeReceiptV1>(value)
        .expect_err("execution host attribution must reject unknown fields");
}
#[test]
fn agent_apartment_manifest_validate_rejects_duplicate_tool_capabilities() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.tool_capabilities.push(AgentToolCapabilityV1 {
        tool: "soracloud.deploy".to_string(),
        max_invocations_per_epoch: NonZeroU32::new(1).expect("nonzero"),
        allow_network: false,
        allow_filesystem_write: false,
    });
    let error = manifest
        .validate()
        .expect_err("duplicate tool capabilities must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::DuplicateToolCapability { .. }
    ));
}
#[test]
fn agent_apartment_manifest_rejects_noncanonical_text_fields() {
    let mut padded_tool = sample_agent_apartment_manifest();
    padded_tool.tool_capabilities[0].tool.push(' ');
    let error = padded_tool
        .validate()
        .expect_err("tool capability whitespace must not be normalized");
    assert_soracloud_invalid_field(error, "tool_capabilities.tool");

    let mut padded_asset = sample_agent_apartment_manifest();
    padded_asset.spend_limits[0].asset_definition.insert(0, ' ');
    let error = padded_asset
        .validate()
        .expect_err("spend asset whitespace must not be normalized");
    assert_soracloud_invalid_field(error, "spend_limits.asset_definition");

    let mut padded_host = sample_agent_apartment_manifest();
    let SoraNetworkPolicyV1::Allowlist(entries) = &mut padded_host.network_egress else {
        panic!("fixture must use an allowlist");
    };
    entries[0].host.push(' ');
    let error = padded_host
        .validate()
        .expect_err("allowlist host whitespace must not be normalized");
    assert_soracloud_invalid_field(error, "network_egress");
}

#[test]
fn signed_agent_deploy_and_service_handler_v1_json_is_closed_and_requires_collections() {
    macro_rules! assert_unknown_rejected {
        ($value:expr, $ty:ty, $label:literal) => {{
            let mut value = norito::json::to_value(&$value).expect(concat!("serialize ", $label));
            value
                .as_object_mut()
                .expect(concat!($label, " JSON object"))
                .insert("retired_v0".to_owned(), norito::json!(true));
            let error = norito::json::from_value::<$ty>(value)
                .expect_err(concat!($label, " must reject unknown fields"));
            assert!(
                matches!(
                    error,
                    json::Error::UnknownField { ref field } if field == "retired_v0"
                ),
                "{} reported the wrong unknown-field error: {error:?}",
                $label
            );
        }};
    }

    let populated = sample_agent_apartment_manifest();
    assert_unknown_rejected!(
        populated.clone(),
        AgentApartmentManifestV1,
        "agent apartment manifest"
    );
    assert_unknown_rejected!(
        populated.tool_capabilities[0].clone(),
        AgentToolCapabilityV1,
        "agent tool capability"
    );
    assert_unknown_rejected!(
        populated.spend_limits[0].clone(),
        AgentSpendLimitV1,
        "agent spend limit"
    );
    assert_unknown_rejected!(
        AgentUpgradePolicyV1::Governed,
        AgentUpgradePolicyV1,
        "agent upgrade policy"
    );
    assert_unknown_rejected!(
        SoraServiceHandlerClassV1::Update,
        SoraServiceHandlerClassV1,
        "service handler class"
    );
    assert_unknown_rejected!(
        SoraCertifiedResponsePolicyV1::AuditReceipt,
        SoraCertifiedResponsePolicyV1,
        "certified response policy"
    );

    let mut empty = populated;
    empty.tool_capabilities.clear();
    empty.policy_capabilities.clear();
    empty.spend_limits.clear();
    let canonical =
        norito::json::to_value(&empty).expect("serialize canonical empty apartment policy");
    assert_eq!(
        norito::json::from_value::<AgentApartmentManifestV1>(canonical.clone())
            .expect("decode explicit empty apartment policy collections"),
        empty
    );
    for field in ["tool_capabilities", "policy_capabilities", "spend_limits"] {
        assert_eq!(
            canonical
                .get(field)
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "canonical empty apartment policy must emit `{field}`"
        );
        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("agent apartment manifest JSON object")
                .remove(field)
                .is_some()
        );
        norito::json::from_value::<AgentApartmentManifestV1>(missing)
            .expect_err("agent apartment manifest must reject omitted V1 collections");
    }
}
#[test]
fn agent_apartment_manifest_validate_rejects_excessive_per_tx_limit() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.spend_limits[0].max_per_tx = xor_quantity_from_nanos(50_000_000);
    let error = manifest
        .validate()
        .expect_err("per-tx spend limit above daily limit must fail");
    assert_soracloud_invalid_field(error, "spend_limits.max_per_tx");
}
#[test]
fn agent_apartment_manifest_validate_rejects_zero_prehash_container_ref_sentinel() {
    let mut manifest = sample_agent_apartment_manifest();
    manifest.container.manifest_hash = zero_prehash_statement_hash();
    let error = manifest
        .validate()
        .expect_err("agent apartment container placeholder hash must fail admission");
    assert_zero_prehash_digest_error(&error, "container.manifest_hash");
}
#[test]
fn agent_apartment_manifest_validate_accepts_consistent_policy() {
    let manifest = sample_agent_apartment_manifest();
    assert!(
        manifest.validate().is_ok(),
        "valid agent apartment manifest must pass"
    );
}
#[test]
fn agent_apartment_manifest_hash_uses_canonical_encoding() {
    let manifest = sample_agent_apartment_manifest();
    assert_eq!(
        manifest.manifest_hash(),
        Hash::new(Encode::encode(&manifest))
    );
}
#[test]
fn agent_apartment_record_validation_accepts_consistent_state() {
    let record = sample_agent_apartment_record();
    assert!(
        record.validate().is_ok(),
        "valid agent apartment record must pass"
    );
}

#[test]
fn agent_apartment_record_json_rejects_removed_persisted_status() {
    let mut value = norito::json::to_value(&sample_agent_apartment_record())
        .expect("serialize agent apartment record");
    value
        .as_object_mut()
        .expect("agent apartment record JSON object")
        .insert("status".to_owned(), norito::json!("Running"));
    let error = norito::json::from_value::<SoraAgentApartmentRecordV1>(value)
        .expect_err("persisted apartment runtime status must not be accepted");
    assert!(
        matches!(error, json::Error::UnknownField { ref field } if field == "status"),
        "unexpected removed-status rejection: {error}"
    );
}

#[test]
fn agent_apartment_current_view_status_uses_half_open_lease_height() {
    let record = sample_agent_apartment_record();
    assert_eq!(
        record.runtime_status_at_current_height(record.lease_expires_height - 1),
        SoraAgentRuntimeStatusV1::Running
    );
    assert_eq!(
        record.runtime_status_at_current_height(record.lease_expires_height),
        SoraAgentRuntimeStatusV1::LeaseExpired
    );
    assert_eq!(
        record.runtime_status_at_current_height(record.lease_expires_height + 1),
        SoraAgentRuntimeStatusV1::LeaseExpired
    );
}

#[test]
fn agent_apartment_current_view_status_fails_closed_before_latest_renewal() {
    let mut record = sample_agent_apartment_record();
    record.last_renewed_height = 50;
    assert_eq!(
        record.runtime_status_at_current_height(record.lease_started_height - 1),
        SoraAgentRuntimeStatusV1::LeaseExpired,
        "a row cannot be active before its deployment"
    );
    assert_eq!(
        record.runtime_status_at_current_height(record.last_renewed_height - 1),
        SoraAgentRuntimeStatusV1::LeaseExpired,
        "a post-renewal row must fail closed when paired with an older view"
    );
    assert_eq!(
        record.runtime_status_at_current_height(record.last_renewed_height),
        SoraAgentRuntimeStatusV1::Running
    );
}

#[test]
fn agent_apartment_current_view_status_preserves_a_late_renewal_gap() {
    let mut before_renewal = sample_agent_apartment_record();
    before_renewal.lease_expires_height = 40;
    assert_eq!(
        before_renewal.runtime_status_at_current_height(45),
        SoraAgentRuntimeStatusV1::LeaseExpired,
        "the pre-renewal view must expose the elapsed lease gap"
    );

    let renewal_height = 50;
    let mut after_renewal = before_renewal;
    after_renewal.last_renewed_height = renewal_height;
    after_renewal.lease_expires_height = 80;
    assert_eq!(
        after_renewal.runtime_status_at_current_height(45),
        SoraAgentRuntimeStatusV1::LeaseExpired,
        "a post-renewal row must not retroactively fill the earlier gap"
    );
    assert_eq!(
        after_renewal.runtime_status_at_current_height(renewal_height),
        SoraAgentRuntimeStatusV1::Running,
        "the renewed lease becomes active at its current-view renewal height"
    );
}

#[test]
fn agent_apartment_record_validation_rejects_manifest_hash_mismatch() {
    let mut record = sample_agent_apartment_record();
    record.manifest_hash = sample_hash(42);
    let error = record
        .validate()
        .expect_err("manifest hash must match embedded manifest");
    assert_soracloud_invalid_field(error, "manifest_hash");
}

#[test]
fn agent_apartment_record_validation_rejects_non_runnable_latest_renewal() {
    let mut record = sample_agent_apartment_record();
    record.last_renewed_height = record.lease_expires_height;
    let error = record
        .validate()
        .expect_err("latest renewal must leave a non-empty runnable interval");
    assert_soracloud_invalid_field(error, "last_renewed_height");
}

#[test]
fn agent_apartment_record_validation_rejects_mailbox_payload_hash_mismatch() {
    let mut record = sample_agent_apartment_record();
    record.mailbox_queue[0].payload.push_str("tampered");
    let error = record
        .validate()
        .expect_err("mailbox payload hash must match payload bytes");
    assert_soracloud_invalid_field(error, "mailbox_queue.payload_hash");
}
#[test]
fn agent_apartment_record_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_record_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut record = sample_agent_apartment_record();
            $assign(&mut record, zero_digest);
            let error = record
                .validate()
                .expect_err("agent apartment placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_record_digest_rejects!(
        "manifest_hash",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.manifest_hash = value;
        }
    );
    assert_record_digest_rejects!(
        "mailbox_queue.payload_hash",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.mailbox_queue[0].payload_hash = value;
        }
    );
    assert_record_digest_rejects!(
        "autonomy_run_history.request_commitment",
        |record: &mut SoraAgentApartmentRecordV1, value| {
            record.autonomy_run_history[0].request_commitment = value;
        }
    );
}
#[test]
fn agent_apartment_record_validation_rejects_invalid_workflow_input_json() {
    let mut record = sample_agent_apartment_record();
    record.autonomy_run_history[0].workflow_input_json = Some("{invalid".to_string());
    let error = record
        .validate()
        .expect_err("invalid workflow_input_json must be rejected");
    assert_soracloud_invalid_field(error, "autonomy_run_history");
}
#[test]
fn agent_apartment_record_rejects_noncanonical_persisted_identifiers() {
    macro_rules! assert_rejected {
        ($field:literal, $mutate:expr) => {{
            let mut record = sample_agent_apartment_record();
            $mutate(&mut record);
            let error = record
                .validate()
                .expect_err("noncanonical persisted identifier must fail closed");
            assert_soracloud_invalid_field(error, $field);
        }};
    }

    assert_rejected!(
        "revoked_policy_capabilities",
        |record: &mut SoraAgentApartmentRecordV1| {
            record.revoked_policy_capabilities.clear();
            record
                .revoked_policy_capabilities
                .insert(" wallet.sign".to_owned());
        }
    );
    assert_rejected!(
        "pending_wallet_requests.asset_definition",
        |record: &mut SoraAgentApartmentRecordV1| {
            record
                .pending_wallet_requests
                .values_mut()
                .next()
                .expect("pending request")
                .asset_definition
                .push(' ');
        }
    );
    assert_rejected!(
        "wallet_daily_spend.key",
        |record: &mut SoraAgentApartmentRecordV1| {
            let (_, entry) = record
                .wallet_daily_spend
                .pop_first()
                .expect("daily spend entry");
            record
                .wallet_daily_spend
                .insert(" padded-daily-key".to_owned(), entry);
        }
    );
    assert_rejected!(
        "mailbox_queue.channel",
        |record: &mut SoraAgentApartmentRecordV1| {
            record.mailbox_queue[0].channel.push(' ');
        }
    );
    assert_rejected!(
        "artifact_allowlist.artifact_hash",
        |record: &mut SoraAgentApartmentRecordV1| {
            record
                .artifact_allowlist
                .values_mut()
                .next()
                .expect("artifact rule")
                .artifact_hash
                .push(' ');
        }
    );
    assert_rejected!(
        "autonomy_run_history.run_label",
        |record: &mut SoraAgentApartmentRecordV1| {
            record.autonomy_run_history[0].run_label.push(' ');
        }
    );
}
#[test]
fn agent_apartment_record_requires_canonical_workflow_input_json() {
    for noncanonical in [" {\"inputs\":\"nightly\"}", "{ \"inputs\" : \"nightly\" }"] {
        let mut record = sample_agent_apartment_record();
        record.autonomy_run_history[0].workflow_input_json = Some(noncanonical.to_owned());
        let error = record
            .validate()
            .expect_err("workflow input JSON must already be canonical");
        assert_soracloud_invalid_field(error, "autonomy_run_history");
    }
}
#[test]
fn agent_apartment_audit_event_validation_rejects_empty_reason() {
    let mut event = sample_agent_apartment_audit_event();
    event.reason = Some(String::new());
    let error = event
        .validate()
        .expect_err("empty optional reason must be rejected");
    assert_soracloud_invalid_field(error, "reason");
}
#[test]
fn agent_apartment_audit_event_rejects_noncanonical_identifier_text() {
    let mut event = sample_agent_apartment_audit_event();
    event.capability = Some(" wallet.sign".to_owned());
    let error = event
        .validate()
        .expect_err("audit identifiers must not be normalized");
    assert_soracloud_invalid_field(error, "capability");
}
#[test]
fn agent_apartment_audit_event_validation_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    macro_rules! assert_event_digest_rejects {
        ($field:literal, $assign:expr) => {{
            let mut event = sample_agent_apartment_audit_event();
            $assign(&mut event, zero_digest);
            let error = event
                .validate()
                .expect_err("agent audit placeholder digest must fail admission");
            assert_zero_prehash_digest_error(&error, $field);
        }};
    }
    assert_event_digest_rejects!(
        "manifest_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.manifest_hash = value;
        }
    );
    assert_event_digest_rejects!(
        "payload_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.payload_hash = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "result_commitment",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.result_commitment = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "runtime_receipt_id",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.runtime_receipt_id = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "journal_artifact_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.journal_artifact_hash = Some(value);
        }
    );
    assert_event_digest_rejects!(
        "checkpoint_artifact_hash",
        |event: &mut SoraAgentApartmentAuditEventV1, value| {
            event.checkpoint_artifact_hash = Some(value);
        }
    );
}
#[test]
fn agent_apartment_audit_event_validation_requires_execution_fields() {
    let mut event = sample_agent_apartment_audit_event();
    event.action = SoraAgentApartmentActionV1::AutonomyRunExecuted;
    event.run_id = Some("ops_agent:autonomy:9".to_owned());
    event.request_id = event.run_id.clone();
    event.artifact_hash = Some(hex::encode([0x51; 32]));
    event.run_label = Some("approved_run".to_owned());
    event.budget_units = Some(1);
    event.service_name = Some("ops_service".to_owned());
    event.service_version = Some("1.0.0".to_owned());
    event.handler_name = Some("execute".to_owned());
    event.result_commitment = Some(sample_hash(52));
    event.runtime_receipt_id = Some(sample_hash(53));
    event.journal_artifact_hash = Some(sample_hash(54));
    event.checkpoint_artifact_hash = Some(sample_hash(55));
    event.succeeded = Some(true);
    event.reason = None;
    event
        .validate()
        .expect("complete execution attribution must validate");
    event.result_commitment = None;
    let error = event
        .validate()
        .expect_err("execution audit events must carry a result commitment");
    assert_soracloud_invalid_field(error, "result_commitment");
    event.result_commitment = Some(sample_hash(171));
    event
        .validate()
        .expect("complete execution audit events must validate");
}
#[test]
fn fhe_param_set_validate_rejects_unregistered_backend() {
    let mut param_set = sample_fhe_param_set();
    param_set.backend = "fhe/bfv-rns/v2".to_string();
    let error = param_set
        .validate()
        .expect_err("first-release parameter-set admission must reject unregistered backends");
    assert_soracloud_invalid_field(error, "backend");
}
#[test]
fn fhe_param_set_validate_rejects_unsupported_scheme() {
    let mut param_set = sample_fhe_param_set();
    param_set.scheme = FheSchemeV1::Ckks;
    let error = param_set
        .validate()
        .expect_err("first-release parameter-set admission must reject non-BFV schemes");
    assert_soracloud_invalid_field(error, "scheme");
}
#[test]
fn fhe_param_set_validate_rejects_empty_modulus_chain() {
    let mut param_set = sample_fhe_param_set();
    param_set.ciphertext_modulus_bits.clear();
    let error = param_set
        .validate()
        .expect_err("empty modulus chain must be rejected");
    assert!(matches!(
        error,
        SoracloudManifestError::EmptyField {
            field: "ciphertext_modulus_bits",
            ..
        }
    ));
}
#[test]
fn fhe_param_set_validate_rejects_zero_prehash_digest_sentinels() {
    let zero_digest = zero_prehash_statement_hash();
    let mut parameter_digest = sample_fhe_param_set();
    parameter_digest.parameter_digest = zero_digest;
    let error = parameter_digest
        .validate()
        .expect_err("parameter digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "parameter_digest");
    let mut rns_digest = sample_fhe_param_set();
    rns_digest.rns_modulus_chain_digest = zero_digest;
    let error = rns_digest
        .validate()
        .expect_err("RNS modulus-chain digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "rns_modulus_chain_digest");
    let mut decomposition_digest = sample_fhe_param_set();
    decomposition_digest.key_switch_decomposition_chain_digest = zero_digest;
    let error = decomposition_digest
        .validate()
        .expect_err("key-switch decomposition digest placeholder must fail admission");
    assert!(error.to_string().contains("zero prehash sentinel"));
    assert_soracloud_invalid_field(error, "key_switch_decomposition_chain_digest");
}
#[test]
fn fhe_param_set_validate_rejects_withdrawal_before_activation() {
    let mut param_set = sample_fhe_param_set();
    param_set.withdraw_height = Some(8_000);
    let error = param_set
        .validate()
        .expect_err("withdraw height before activation must be rejected");
    assert_soracloud_invalid_field(error, "withdraw_height");
}
#[test]
fn fhe_param_set_validate_rejects_adversarial_structural_fields() {
    let mut ascending_chain = sample_fhe_param_set();
    ascending_chain.ciphertext_modulus_bits = vec![
        NonZeroU16::new(40).expect("nonzero"),
        NonZeroU16::new(50).expect("nonzero"),
    ];
    let error = ascending_chain
        .validate()
        .expect_err("ascending modulus chains must be rejected");
    assert_soracloud_invalid_field(error, "ciphertext_modulus_bits");
    let mut plaintext_not_smaller = sample_fhe_param_set();
    plaintext_not_smaller.plaintext_modulus_bits = NonZeroU16::new(60).expect("nonzero");
    let error = plaintext_not_smaller
        .validate()
        .expect_err("plaintext modulus must be below ciphertext modulus");
    assert_soracloud_invalid_field(error, "plaintext_modulus_bits");
    let mut slot_overflow = sample_fhe_param_set();
    slot_overflow.slot_count = NonZeroU32::new(8_193).expect("nonzero");
    let error = slot_overflow
        .validate()
        .expect_err("slot count above polynomial degree must be rejected");
    assert_soracloud_invalid_field(error, "slot_count");
    let mut exhausted_depth = sample_fhe_param_set();
    exhausted_depth.max_multiplicative_depth = NonZeroU16::new(3).expect("nonzero");
    let error = exhausted_depth
        .validate()
        .expect_err("depth consuming the whole modulus chain must be rejected");
    assert_soracloud_invalid_field(error, "max_multiplicative_depth");
    let evaluator_budget = BfvEvaluationBudget::exact_evaluator_v1();
    let mut over_evaluator_depth = sample_fhe_param_set();
    over_evaluator_depth.ciphertext_modulus_bits =
        vec![
            NonZeroU16::new(60).expect("nonzero");
            usize::from(evaluator_budget.max_multiplicative_depth) + 2
        ];
    over_evaluator_depth.max_multiplicative_depth =
        NonZeroU16::new(evaluator_budget.max_multiplicative_depth + 1).expect("nonzero");
    let error = over_evaluator_depth
        .validate()
        .expect_err("depth above exact evaluator budget must be rejected");
    assert_soracloud_invalid_field(error, "max_multiplicative_depth");
}
#[test]
fn fhe_param_set_validate_rejects_adversarial_lifecycle_claims() {
    let mut proposed_with_withdraw = sample_fhe_param_set();
    proposed_with_withdraw.lifecycle = FheParamLifecycleV1::Proposed;
    let error = proposed_with_withdraw
        .validate()
        .expect_err("proposed parameter sets cannot carry withdrawal metadata");
    assert_soracloud_invalid_field(error, "lifecycle");
    let mut active_without_activation = sample_fhe_param_set();
    active_without_activation.activation_height = None;
    active_without_activation.withdraw_height = None;
    let error = active_without_activation
        .validate()
        .expect_err("active parameter sets must declare activation height");
    assert_soracloud_invalid_field(error, "lifecycle");
    let mut withdrawn_without_withdraw_height = sample_fhe_param_set();
    withdrawn_without_withdraw_height.lifecycle = FheParamLifecycleV1::Withdrawn;
    withdrawn_without_withdraw_height.withdraw_height = None;
    let error = withdrawn_without_withdraw_height
        .validate()
        .expect_err("withdrawn parameter sets must carry withdraw height");
    assert_soracloud_invalid_field(error, "lifecycle");
}
#[test]
fn validation_helper_schema_version_preserves_error_details() {
    assert_eq!(validate_schema_version("test manifest", 1, 1), Ok(()));
    assert_eq!(
        validate_schema_version("test manifest", 2, 1),
        Err(SoracloudManifestError::UnsupportedVersion {
            manifest: "test manifest",
            expected: 1,
            found: 2,
        })
    );
}
#[test]
fn validation_helper_nonblank_field_requires_exact_stable_text() {
    assert!(matches!(
        validate_nonblank_field("test manifest", "name", " value "),
        Err(SoracloudManifestError::InvalidField { field: "name", .. })
    ));
    assert!(matches!(
        validate_nonblank_field("test manifest", "name", "value\0suffix"),
        Err(SoracloudManifestError::InvalidField { field: "name", .. })
    ));
    assert_eq!(
        validate_nonblank_field("test manifest", "name", "value"),
        Ok(())
    );
    assert_eq!(
        validate_nonblank_field("test manifest", "name", " \t\n"),
        Err(SoracloudManifestError::EmptyField {
            manifest: "test manifest",
            field: "name",
        })
    );
}
#[test]
fn validation_helper_invalid_field_preserves_error_details_and_message() {
    let error = invalid_field("test manifest", "name", "must be canonical");
    assert_eq!(
        error,
        SoracloudManifestError::InvalidField {
            manifest: "test manifest",
            field: "name",
            reason: "must be canonical".to_owned(),
        }
    );
    assert_eq!(
        error.to_string(),
        "test manifest field `name` is invalid: must be canonical"
    );
}
