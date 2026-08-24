#[test]
fn authoritative_agent_autonomy_status_includes_runtime_recent_runs() -> Result<(), eyre::Report> {
    use iroha_core::state::World;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let temp_dir = tempfile::tempdir()?;
        let mut world = World::default();
        let manifest = fixture_agent_manifest();
        let request_commitment =
            iroha_data_model::soracloud::derive_agent_autonomy_request_commitment(
                "ops_agent",
                "hash:artifact#1",
                Some("hash:prov#1"),
                25,
                "ops_agent:autonomy:7",
                "nightly",
                Some("{\"inputs\":\"nightly\"}"),
                1,
            );
        let run = SoraAgentAutonomyRunRecordV1 {
            run_id: "ops_agent:autonomy:7".to_owned(),
            artifact_hash: "hash:artifact#1".to_owned(),
            provenance_hash: Some("hash:prov#1".to_owned()),
            budget_units: 25,
            run_label: "nightly".to_owned(),
            workflow_input_json: Some("{\"inputs\":\"nightly\"}".to_owned()),
            approved_process_generation: 1,
            request_commitment,
            approved_sequence: 7,
        };
        world.soracloud_agent_apartments_mut_for_testing().insert(
            manifest.apartment_name.to_string(),
            SoraAgentApartmentRecordV1 {
                schema_version: iroha_data_model::soracloud::SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
                manifest_hash: Hash::new(b"agent-manifest"),
                status: SoraAgentRuntimeStatusV1::Running,
                deployed_sequence: 1,
                lease_started_sequence: 1,
                lease_expires_sequence: 100,
                last_renewed_sequence: 1,
                restart_count: 0,
                last_restart_sequence: None,
                last_restart_reason: None,
                process_generation: 1,
                process_started_sequence: 1,
                last_active_sequence: 7,
                last_checkpoint_sequence: Some(7),
                checkpoint_count: 1,
                persistent_state: SoraAgentPersistentStateV1 {
                    total_bytes: 64,
                    key_sizes: BTreeMap::from([("/agent/checkpoint/7".to_owned(), 64)]),
                },
                revoked_policy_capabilities: BTreeSet::new(),
                pending_wallet_requests: BTreeMap::new(),
                wallet_daily_spend: BTreeMap::new(),
                mailbox_queue: Vec::new(),
                autonomy_budget_ceiling_units: 100,
                autonomy_budget_remaining_units: 75,
                artifact_allowlist: BTreeMap::new(),
                autonomy_run_history: vec![run.clone()],
                manifest,
            },
        );
        world.soracloud_runtime_receipts_mut_for_testing().insert(
            Hash::new(b"ops-agent-authoritative-receipt"),
            SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(b"ops-agent-authoritative-receipt"),
                service_name: "hf_agent_service".parse().expect("valid service name"),
                service_version: "hf.generated.v1".to_owned(),
                handler_name: "infer".parse().expect("valid handler name"),
                handler_class: SoraServiceHandlerClassV1::Query,
                request_commitment: run.request_commitment,
                result_commitment: Hash::new(b"authoritative-runtime-result"),
                certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                emitted_sequence: 77,
                mailbox_message_id: None,
                journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                execution_host: None,
            },
        );
        world
            .soracloud_agent_apartment_audit_events_mut_for_testing()
            .insert(
                78,
                SoraAgentApartmentAuditEventV1 {
                    schema_version: SORA_AGENT_APARTMENT_AUDIT_EVENT_VERSION_V1,
                    sequence: 78,
                    action: SoraAgentApartmentActionV1::AutonomyRunExecuted,
                    apartment_name: "ops_agent".parse().expect("valid apartment name"),
                    status: SoraAgentRuntimeStatusV1::Running,
                    lease_expires_sequence: 100,
                    manifest_hash: Hash::new(b"agent-manifest"),
                    restart_count: 0,
                    signer: checked_test_keypair(0xB4).public_key().clone(),
                    request_id: Some(run.run_id.clone()),
                    asset_definition: None,
                    amount: None,
                    capability: None,
                    reason: None,
                    from_apartment: None,
                    to_apartment: None,
                    channel: None,
                    payload_hash: None,
                    artifact_hash: Some(run.artifact_hash.clone()),
                    provenance_hash: run.provenance_hash.clone(),
                    run_id: Some(run.run_id.clone()),
                    run_label: Some(run.run_label.clone()),
                    budget_units: Some(run.budget_units),
                    service_name: Some("hf_agent_service".to_owned()),
                    service_version: Some("hf.generated.v1".to_owned()),
                    handler_name: Some("infer".to_owned()),
                    result_commitment: Some(Hash::new(b"authoritative-runtime-result")),
                    runtime_receipt_id: Some(Hash::new(b"ops-agent-authoritative-receipt")),
                    journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                    checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                    succeeded: Some(true),
                },
            );
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestHfRuntimeHandle {
            snapshot: SoracloudRuntimeSnapshot::default(),
            state_dir: temp_dir.path().to_path_buf(),
        }));
        let summary_dir = temp_dir
            .path()
            .join("apartments")
            .join(sanitize_runtime_path_component("ops_agent"))
            .join("runs")
            .join(sanitize_runtime_path_component(&run.run_id));
        fs::create_dir_all(&summary_dir)?;
        let summary = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: "ops_agent".to_owned(),
            run_id: run.run_id.clone(),
            service_name: Some("hf_agent_service".to_owned()),
            service_version: Some("hf.generated.v1".to_owned()),
            handler_name: Some("infer".to_owned()),
            succeeded: true,
            result_commitment: Hash::new(b"runtime-result"),
            checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
            runtime_receipt: Some(SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(b"ops-agent-authoritative-receipt"),
                service_name: "hf_agent_service".parse().expect("valid service name"),
                service_version: "hf.generated.v1".to_owned(),
                handler_name: "infer".parse().expect("valid handler name"),
                handler_class: SoraServiceHandlerClassV1::Query,
                request_commitment: run.request_commitment,
                result_commitment: Hash::new(b"authoritative-runtime-result"),
                certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                emitted_sequence: 77,
                mailbox_message_id: None,
                journal_artifact_hash: Some(Hash::new(b"ops-agent-authoritative-journal")),
                checkpoint_artifact_hash: Some(Hash::new(br#"{"text":"ok"}"#)),
                execution_host: None,
            }),
            workflow_steps: Vec::new(),
            content_type: Some("application/json".to_owned()),
            response_json: Some(norito::json!({"text":"ok","backend":"local_fixture"})),
            response_text: Some(r#"{"text":"ok","backend":"local_fixture"}"#.to_owned()),
            error: None,
        };
        let summary_bytes = norito::json::to_vec_pretty(&summary)?;
        fs::write(summary_dir.join("execution_summary.json"), &summary_bytes)?;
        let status = authoritative_agent_autonomy_status_response(&app, "ops_agent")
            .map_err(|err| eyre::eyre!("agent autonomy status failed: {err:?}"))?;
        assert_eq!(
            status.recent_runs[0].workflow_input_json.as_deref(),
            Some("{\"inputs\":\"nightly\"}")
        );
        assert_eq!(
            status.recent_runs[0]
                .authoritative_runtime_receipt
                .as_ref()
                .map(|receipt| receipt.receipt_id),
            Some(Hash::new(b"ops-agent-authoritative-receipt"))
        );
        assert_eq!(
            status.recent_runs[0]
                .authoritative_execution_audit
                .as_ref()
                .map(|audit| audit.sequence),
            Some(78)
        );
        assert_eq!(
            status.recent_runs[0]
                .authoritative_execution_audit
                .as_ref()
                .and_then(|audit| audit.runtime_receipt_id),
            Some(Hash::new(b"ops-agent-authoritative-receipt"))
        );
        assert_eq!(
            status.recent_runs[0]
                .authoritative_execution_audit
                .as_ref()
                .map(|audit| audit.succeeded),
            Some(true)
        );
        assert_eq!(status.runtime_recent_runs.len(), 1);
        assert_eq!(
            status.runtime_recent_runs[0].service_name.as_deref(),
            Some("hf_agent_service")
        );
        assert_eq!(
            status.runtime_recent_runs[0]
                .runtime_receipt
                .as_ref()
                .map(|receipt| receipt.request_commitment),
            Some(run.request_commitment)
        );
        assert_eq!(
            status.runtime_recent_runs[0]
                .response_json
                .as_ref()
                .and_then(|value| value.get("backend"))
                .and_then(norito::json::Value::as_str),
            Some("local_fixture")
        );
        assert_eq!(
            status.runtime_recent_runs[0].journal_artifact_hash,
            Hash::new(&summary_bytes)
        );
        Ok(())
    })
}
