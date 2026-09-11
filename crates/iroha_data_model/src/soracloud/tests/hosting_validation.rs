//! Hosting audit attribution, canonical URL boundaries, and first-error contracts.
use super::*;

#[test]
fn wallet_audit_metadata_precedes_spend_attribution_for_both_actions() {
    for action in [
        SoraAgentApartmentActionV1::WalletSpendRequested,
        SoraAgentApartmentActionV1::WalletSpendApproved,
    ] {
        let mut event = sample_agent_apartment_audit_event();
        event.action = action;
        event.reason = Some(" ".to_owned());
        event.budget_units = Some(0);
        event.request_id = Some("r".repeat(SORA_AGENT_WALLET_REQUEST_ID_MAX_BYTES_V1 + 1));
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "reason");
        event.reason = None;
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "budget_units");
        event.budget_units = Some(1);
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "request_id");
        event.request_id = Some("ops_agent:wallet:35".to_owned());
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "asset_definition");
        event.asset_definition = Some("wallet_asset".to_owned());
        event.amount = Some(Quantity::zero());
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "amount");
        event.amount = Some(Quantity::from(1_u32));
        assert_eq!(event.validate(), Ok(()));
        event.request_id = Some("r".repeat(SORA_AGENT_WALLET_REQUEST_ID_MAX_BYTES_V1));
        assert_eq!(event.validate(), Ok(()));
        event.request_id.as_mut().unwrap().push('r');
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "request_id");
    }
}

fn execution_audit() -> SoraAgentApartmentAuditEventV1 {
    let mut event = sample_agent_apartment_audit_event();
    event.action = SoraAgentApartmentActionV1::AutonomyRunExecuted;
    event.run_id = Some("ops_agent:autonomy:9".to_owned());
    event.request_id.clone_from(&event.run_id);
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
}

#[test]
fn execution_audit_preserves_attribution_result_and_outcome_error_order() {
    let baseline = execution_audit();
    assert_eq!(baseline.validate(), Ok(()));
    let mut event = baseline.clone();
    event.request_id = None;
    event.result_commitment = None;
    event.succeeded = None;
    event.service_name = None;
    event.journal_artifact_hash = None;
    event.reason = Some("failure".to_owned());
    event.amount = Some(Quantity::from(1_u32));
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "run_id");
    event.request_id.clone_from(&baseline.request_id);
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "result_commitment");
    event.result_commitment = baseline.result_commitment;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "succeeded");
    event.succeeded = baseline.succeeded;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "service_name");
    event.service_name.clone_from(&baseline.service_name);
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "journal_artifact_hash");
    event.journal_artifact_hash = baseline.journal_artifact_hash;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "succeeded");
    event.reason = None;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "action");
    event.amount = None;
    assert_eq!(event.validate(), Ok(()));
}

#[test]
fn execution_outcomes_require_complete_context_and_exclusive_success_evidence() {
    let baseline = execution_audit();
    for success in [false, true] {
        for context_fields in 0..=3 {
            let mut event = baseline.clone();
            event.succeeded = Some(success);
            if !success {
                event.reason = Some("execution failed".to_owned());
                event.runtime_receipt_id = None;
                event.checkpoint_artifact_hash = None;
            }
            if context_fields < 3 {
                event.handler_name = None;
            }
            if context_fields < 2 {
                event.service_version = None;
            }
            if context_fields < 1 {
                event.service_name = None;
            }
            let expected = context_fields == 3 || (!success && context_fields == 0);
            assert_eq!(
                event.validate().is_ok(),
                expected,
                "success {success}, context {context_fields}"
            );
        }
    }
    let mut failed = baseline;
    failed.succeeded = Some(false);
    failed.reason = Some("execution failed".to_owned());
    assert_soracloud_invalid_field(failed.validate().unwrap_err(), "succeeded");
    failed.runtime_receipt_id = None;
    assert_soracloud_invalid_field(failed.validate().unwrap_err(), "succeeded");
    failed.checkpoint_artifact_hash = None;
    assert_eq!(failed.validate(), Ok(()));
}

#[test]
fn public_url_authorities_preserve_exact_host_port_and_path_boundaries() {
    for url in [
        "http://example.com",
        "https://example.com:65535/a?key=value",
        "https://example.com/?key=value",
        "http://127.0.0.1:443",
        "https://127.0.0.1:80",
        "https://[::1]",
        "http://[2001:db8::1]:8080/a?key=value",
    ] {
        assert_eq!(validate_public_url("fixture", "url", url), Ok(()), "{url}");
    }
    for url in [
        "http://example.com:80",
        "https://example.com:443",
        "https://example.com:0",
        "https://example.com:0444",
        "https://example.com:65536",
        "https://example.com:",
        "https://EXAMPLE.com",
        "https://127.000.0.1",
        "https://[2001:0db8::1]",
        "http://::1",
        "http://[::1",
        "http://[::1]tail",
        "http://[::1]:80",
        "https://user@example.com",
        "https:///path",
        "https://example.com/",
        "https://example.com?key=value",
        "https://example.com/a?",
        "https://example.com/a#fragment",
        "https://example.com/a%2fb",
        "https://example.com/a/../b",
        "https://example.com/a//b",
    ] {
        assert!(validate_public_url("fixture", "url", url).is_err(), "{url}");
    }
}

#[test]
fn public_url_failure_order_keeps_scheme_authority_port_and_suffix_phases() {
    for (url, reason) in [
        (
            "HTTPS://user@example.com:443/%",
            "must start with exact lowercase http:// or https://",
        ),
        (
            "https://user@example.com:443/%",
            "must include a nonempty public host without user information",
        ),
        (
            "https://example.com:443/%",
            "TCP port must be nonzero, non-default, and use exact decimal spelling",
        ),
        (
            "https://example.com:444/%",
            "URL path and query must be exact ASCII without fragments, escapes, or backslashes",
        ),
        (
            "https://example.com:444/?",
            "URL queries require a nonempty query and explicit canonical path",
        ),
        (
            "https://example.com:444/",
            "root URLs must omit the trailing slash",
        ),
    ] {
        assert_eq!(
            validate_public_url("fixture", "url", url),
            Err(invalid_field("fixture", "url", reason)),
            "{url}"
        );
    }
    assert_eq!(
        validate_public_url("fixture", "url", "https://example.com:444"),
        Ok(())
    );
}

#[test]
fn lifecycle_presence_precedes_material_and_version_errors() {
    let baseline = sample_service_audit_event();
    assert_eq!(baseline.validate(), Ok(()));
    let mut event = baseline.clone();
    event.governance_tx_hash = None;
    event
        .config_mutations
        .push(SoraServiceConfigMutationV1::Delete("feature".to_owned()));
    event.from_version = Some("1.0.0".to_owned());
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "governance_tx_hash");
    event.governance_tx_hash = baseline.governance_tx_hash;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "config_mutations");
    event.config_mutations.clear();
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "from_version");
    event.from_version = None;
    assert_eq!(event.validate(), Ok(()));
    event.action = SoraServiceLifecycleActionV1::CiphertextQuery;
    event.governance_tx_hash = None;
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "action");
}

#[test]
fn lifecycle_material_phase_enforces_exact_mutation_count_and_kind() {
    let mut event = sample_service_audit_event();
    event.action = SoraServiceLifecycleActionV1::ConfigMutation;
    assert_soracloud_invalid_field(
        event.validate_action_material_deltas().unwrap_err(),
        "config_mutations",
    );
    event
        .config_mutations
        .push(SoraServiceConfigMutationV1::Delete("first".to_owned()));
    assert_eq!(event.validate_action_material_deltas(), Ok(()));
    event
        .config_mutations
        .push(SoraServiceConfigMutationV1::Delete("second".to_owned()));
    assert_soracloud_invalid_field(
        event.validate_action_material_deltas().unwrap_err(),
        "config_mutations",
    );
    event.config_mutations.truncate(1);
    for action in [
        SoraServiceLifecycleActionV1::Deploy,
        SoraServiceLifecycleActionV1::Upgrade,
    ] {
        event.action = action;
        assert_soracloud_invalid_field(
            event.validate_action_material_deltas().unwrap_err(),
            "config_mutations",
        );
    }
    event.config_mutations.clear();
    event.action = SoraServiceLifecycleActionV1::SecretMutation;
    assert_soracloud_invalid_field(
        event.validate_action_material_deltas().unwrap_err(),
        "config_mutations",
    );
    event
        .secret_mutations
        .push(SoraServiceSecretMutationV1::Delete("secret".to_owned()));
    assert_eq!(event.validate_action_material_deltas(), Ok(()));
    event
        .config_mutations
        .push(SoraServiceConfigMutationV1::Delete("feature".to_owned()));
    assert_soracloud_invalid_field(
        event.validate_action_material_deltas().unwrap_err(),
        "config_mutations",
    );
}

fn rollout_audit(action: SoraServiceLifecycleActionV1) -> SoraServiceAuditEventV1 {
    let mut event = sample_service_audit_event();
    event.action = action;
    event.sequence = 3;
    event.binding_name = None;
    event.state_key = None;
    event.policy_name = None;
    event.policy_snapshot_hash = None;
    event.jurisdiction_tag = None;
    event.consent_evidence_hash = None;
    event.break_glass = None;
    event.break_glass_reason = None;
    let (from, to, stage, traffic, failures) = match action {
        SoraServiceLifecycleActionV1::Upgrade => {
            event.governance_tx_hash = None;
            ("1.0.0", "2.0.0", SoraRolloutStageV1::Canary, 10, 0)
        }
        SoraServiceLifecycleActionV1::Rollout => {
            ("2.0.0", "2.0.0", SoraRolloutStageV1::Promoted, 100, 0)
        }
        SoraServiceLifecycleActionV1::Rollback => {
            ("2.0.0", "1.0.0", SoraRolloutStageV1::RolledBack, 0, 2)
        }
        other => panic!("not a rollout action: {other:?}"),
    };
    event.from_version = Some(from.to_owned());
    event.to_version = to.to_owned();
    event.rollout_state = Some(SoraServiceRolloutStateV1 {
        schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        rollout_handle: "portal:rollout:3".to_owned(),
        baseline_version: "1.0.0".to_owned(),
        candidate_version: "2.0.0".to_owned(),
        canary_percent: 10,
        traffic_percent: traffic,
        stage,
        health_failures: failures,
        max_health_failures: 2,
        health_window_secs: 30,
        created_sequence: 3,
        updated_sequence: 3,
    });
    event
}

#[test]
fn rollout_audit_binds_namespace_creation_sequence_and_exact_transition() {
    for action in [
        SoraServiceLifecycleActionV1::Upgrade,
        SoraServiceLifecycleActionV1::Rollout,
        SoraServiceLifecycleActionV1::Rollback,
    ] {
        let baseline = rollout_audit(action);
        assert_eq!(baseline.validate(), Ok(()), "{action:?}");
        for handle in [
            "other:rollout:3",
            "portal:rollout:0",
            "portal:rollout:03",
            "portal:rollout:18446744073709551616",
        ] {
            let mut event = baseline.clone();
            event.rollout_state.as_mut().unwrap().rollout_handle = handle.to_owned();
            assert_soracloud_invalid_field(
                event.validate().unwrap_err(),
                "rollout_state.rollout_handle",
            );
        }
        let mut event = baseline.clone();
        event.rollout_state.as_mut().unwrap().created_sequence = 2;
        event.rollout_state.as_mut().unwrap().candidate_version = "3.0.0".to_owned();
        assert_soracloud_invalid_field(
            event.validate().unwrap_err(),
            "rollout_state.created_sequence",
        );
        event.rollout_state.as_mut().unwrap().created_sequence = 3;
        assert_soracloud_invalid_field(event.validate().unwrap_err(), "rollout_state");
        event.rollout_state = baseline.rollout_state;
        assert_eq!(event.validate(), Ok(()));
    }
}

#[test]
fn rollback_pair_and_lease_guards_precede_rollout_namespace_validation() {
    let baseline = rollout_audit(SoraServiceLifecycleActionV1::Rollback);
    let mut event = baseline.clone();
    event.rollout_state = None;
    event.service_lease_commitment = Some(sample_hash(184));
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "rollout_state");
    event.rollout_state = baseline.rollout_state;
    event.rollout_state.as_mut().unwrap().rollout_handle = "other:rollout:3".to_owned();
    assert_soracloud_invalid_field(event.validate().unwrap_err(), "service_lease_commitment");
    event.service_lease_commitment = None;
    assert_soracloud_invalid_field(
        event.validate().unwrap_err(),
        "rollout_state.rollout_handle",
    );
    event.rollout_state.as_mut().unwrap().rollout_handle = "portal:rollout:3".to_owned();
    assert_eq!(event.validate(), Ok(()));
}

#[test]
fn version_phase_distinguishes_revisions_from_same_deployment_accounting() {
    let mut event = sample_service_audit_event();
    for action in [
        SoraServiceLifecycleActionV1::Upgrade,
        SoraServiceLifecycleActionV1::Rollback,
    ] {
        event.action = action;
        event.from_version = None;
        assert_soracloud_invalid_field(
            event.validate_action_version_transition().unwrap_err(),
            "from_version",
        );
        event.from_version = Some(event.to_version.clone());
        assert_soracloud_invalid_field(
            event.validate_action_version_transition().unwrap_err(),
            "from_version",
        );
        event.from_version = Some("previous".to_owned());
        assert_eq!(event.validate_action_version_transition(), Ok(()));
    }
    for action in [
        SoraServiceLifecycleActionV1::Rollout,
        SoraServiceLifecycleActionV1::LeaseUsage,
        SoraServiceLifecycleActionV1::LeaseReportingEpochRollover,
    ] {
        event.action = action;
        assert_soracloud_invalid_field(
            event.validate_action_version_transition().unwrap_err(),
            "from_version",
        );
        event.from_version = Some(event.to_version.clone());
        assert_eq!(event.validate_action_version_transition(), Ok(()));
        event.from_version = Some("previous".to_owned());
    }
}
