struct GeneratedHfAutonomyScenario<'a> {
    service_name: &'a str,
    apartment_name: &'a str,
    local_peer_id: &'a str,
    run_id: &'a str,
    artifact_hash: &'a str,
    provenance_hash: &'a str,
    budget_units: u64,
    run_label: &'a str,
    workflow_input_json: &'a str,
    approved_sequence: u64,
    request_commitment_seed: &'a [u8],
    response_prefix: &'a str,
}

struct PreparedGeneratedHfAutonomyScenario {
    state: Arc<State>,
    service: GeneratedHfServiceFixture,
    apartment_name: Name,
    process_generation: u64,
    run: SoraAgentAutonomyRunRecordV1,
    temp_dir: tempfile::TempDir,
    manager: SoracloudRuntimeManager,
}

impl PreparedGeneratedHfAutonomyScenario {
    fn handle(&self) -> SoracloudRuntimeManagerHandle {
        test_runtime_handle(&self.manager, Arc::clone(&self.state))
    }
}

fn autonomy_run_fixture(spec: &GeneratedHfAutonomyScenario<'_>) -> SoraAgentAutonomyRunRecordV1 {
    SoraAgentAutonomyRunRecordV1 {
        run_id: spec.run_id.to_owned(),
        artifact_hash: spec.artifact_hash.to_owned(),
        provenance_hash: Some(spec.provenance_hash.to_owned()),
        budget_units: spec.budget_units,
        run_label: spec.run_label.to_owned(),
        workflow_input_json: Some(spec.workflow_input_json.to_owned()),
        approved_process_generation: 1,
        request_commitment: Hash::new(spec.request_commitment_seed),
        approved_sequence: spec.approved_sequence,
    }
}

fn autonomy_apartment_fixture(
    manifest: AgentApartmentManifestV1,
    run: &SoraAgentAutonomyRunRecordV1,
) -> SoraAgentApartmentRecordV1 {
    SoraAgentApartmentRecordV1 {
        schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        manifest_hash: Hash::new(Encode::encode(&manifest)),
        status: SoraAgentRuntimeStatusV1::Running,
        deployed_sequence: 1,
        lease_started_sequence: 1,
        lease_expires_sequence: 400,
        last_renewed_sequence: 1,
        restart_count: 0,
        last_restart_sequence: None,
        last_restart_reason: None,
        process_generation: 1,
        process_started_sequence: 1,
        last_active_sequence: run.approved_sequence,
        last_checkpoint_sequence: Some(run.approved_sequence),
        checkpoint_count: 1,
        persistent_state: SoraAgentPersistentStateV1 {
            total_bytes: 128,
            key_sizes: BTreeMap::from([(format!("/autonomy/{}", run.run_id), 128)]),
        },
        revoked_policy_capabilities: BTreeSet::new(),
        pending_wallet_requests: BTreeMap::new(),
        wallet_daily_spend: BTreeMap::new(),
        mailbox_queue: Vec::new(),
        autonomy_budget_ceiling_units: 1_000,
        autonomy_budget_remaining_units: 1_000 - run.budget_units,
        artifact_allowlist: BTreeMap::from([(
            run.artifact_hash.clone(),
            SoraAgentArtifactAllowRuleV1 {
                artifact_hash: run.artifact_hash.clone(),
                provenance_hash: run.provenance_hash.clone(),
                added_sequence: run.approved_sequence.saturating_sub(1),
            },
        )]),
        autonomy_run_history: vec![run.clone()],
        manifest,
    }
}

fn write_hf_autonomy_echo_source(
    state_dir: &Path,
    source_id: Hash,
    response_prefix: &str,
) -> Result<()> {
    let source_root = state_dir
        .join("hf_sources")
        .join(sanitize_path_component(&source_id.to_string()));
    let files_root = source_root.join("files");
    fs::create_dir_all(&files_root)?;
    let config_json = format!(
        "{{\n  \"model_type\": \"gpt2\",\n  \"_soracloud_fixture\": {{\n    \"mode\": \"echo\",\n    \"prefix\": \"{response_prefix}\"\n  }}\n}}"
    );
    let config_path = files_root.join("config.json");
    write_bytes_atomic(&config_path, config_json.as_bytes())?;
    write_json_atomic(
        &source_root.join("import_manifest.json"),
        &HfLocalImportManifestV1 {
            schema_version: HF_LOCAL_IMPORT_SCHEMA_VERSION_V1,
            source_id: source_id.to_string(),
            repo_id: "openai-community/gpt2".to_owned(),
            requested_revision: "main".to_owned(),
            resolved_commit: Some("main".to_owned()),
            model_name: "gpt2".to_owned(),
            adapter_id: "hf.shared.v1".to_owned(),
            pipeline_tag: Some("text-generation".to_owned()),
            library_name: Some("transformers".to_owned()),
            tags: vec!["text-generation".to_owned()],
            imported_at_ms: 20,
            imported_files: vec![HfImportedFileV1 {
                path: "config.json".to_owned(),
                content_length: u64::try_from(config_json.len()).unwrap_or(u64::MAX),
                payload_hash: Hash::new(config_json.as_bytes()).to_string(),
                local_path: config_path.display().to_string(),
            }],
            skipped_files: Vec::new(),
            raw_model_info_path: None,
            import_error: None,
        },
    )?;
    Ok(())
}

fn prepare_generated_hf_autonomy_scenario(
    spec: &GeneratedHfAutonomyScenario<'_>,
) -> Result<PreparedGeneratedHfAutonomyScenario> {
    let mut state = test_state();
    let service = insert_generated_hf_service_fixture(
        &mut state,
        spec.service_name,
        "openai-community/gpt2",
        "main",
        "gpt2",
    )?;
    set_generated_hf_service_route_visibility(&mut state, &service, SoraRouteVisibilityV1::Public);
    let apartment_name: Name = spec.apartment_name.parse().expect("valid apartment name");
    let manifest = iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
        apartment_name.clone(),
        &service.bundle,
    );
    let run = autonomy_run_fixture(spec);
    let apartment = autonomy_apartment_fixture(manifest, &run);
    Arc::get_mut(&mut state)
        .expect("unique test state")
        .world
        .soracloud_agent_apartments_mut_for_testing()
        .insert(apartment_name.to_string(), apartment.clone());
    insert_generated_hf_placement_fixture(
        &mut state,
        &service,
        SoraHfPlacementHostRoleV1::Primary,
        SoraHfPlacementHostStatusV1::Warm,
        spec.local_peer_id,
    );
    let temp_dir = tempfile::tempdir()?;
    write_hf_autonomy_echo_source(temp_dir.path(), service.source_id, spec.response_prefix)?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), spec.local_peer_id),
        Arc::clone(&state),
    );
    manager.reconcile_once()?;
    Ok(PreparedGeneratedHfAutonomyScenario {
        state,
        service,
        apartment_name,
        process_generation: apartment.process_generation,
        run,
        temp_dir,
        manager,
    })
}

fn assert_single_hf_autonomy_summary(
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
    service_name: &str,
) {
    assert!(summary.succeeded);
    assert!(summary.workflow_steps.is_empty());
    assert_eq!(summary.service_name.as_deref(), Some(service_name));
    let runtime_receipt = summary
        .runtime_receipt
        .as_ref()
        .expect("runtime receipt persisted");
    assert_eq!(runtime_receipt.service_name.as_ref(), service_name);
    assert_eq!(runtime_receipt.handler_name.as_ref(), "infer");
    let response_json = summary.response_json.as_ref().expect("response json");
    assert_eq!(
        response_json
            .get("backend")
            .and_then(norito::json::Value::as_str),
        Some("local_fixture")
    );
    assert_eq!(
        response_json
            .get("inputs")
            .and_then(norito::json::Value::as_array)
            .and_then(|inputs| inputs.first())
            .and_then(norito::json::Value::as_str),
        Some("alpha")
    );
    assert_eq!(
        response_json
            .get("inputs")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        response_json
            .get("parameters")
            .and_then(norito::json::Value::as_object)
            .and_then(|parameters| parameters.get("max_new_tokens"))
            .and_then(norito::json::Value::as_u64),
        Some(4)
    );
    assert_eq!(
        response_json
            .get("text")
            .and_then(norito::json::Value::as_str),
        Some("agent:['alpha', 'beta']")
    );
}

#[test]
fn execute_apartment_generated_hf_autonomy_run_executes_locally_and_persists_summary() -> Result<()>
{
    let scenario = prepare_generated_hf_autonomy_scenario(&GeneratedHfAutonomyScenario {
        service_name: "hf_agent_service",
        apartment_name: "hf_agent",
        local_peer_id: "12D3KooWHfAgentAutonomyRuntimeHost",
        run_id: "hf_agent:autonomy:42",
        artifact_hash: "hash:HFAGENT#01",
        provenance_hash: "hash:HFPROV#01",
        budget_units: 75,
        run_label: "fallback label",
        workflow_input_json: "{\"inputs\":[\"alpha\",\"beta\"],\"parameters\":{\"max_new_tokens\":4}}",
        approved_sequence: 42,
        request_commitment_seed: b"hf-agent-run",
        response_prefix: "agent:",
    })?;
    let handle = scenario.handle();
    let run = &scenario.run;
    let request = SoracloudApartmentExecutionRequest {
        observed_height: 0,
        observed_block_hash: None,
        apartment_name: scenario.apartment_name.to_string(),
        process_generation: scenario.process_generation,
        operation: format!("autonomy-run:{}", run.run_id),
        request_commitment: run.request_commitment,
    };
    let result = handle
        .execute_apartment(request.clone())
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert_eq!(result.status, SoraAgentRuntimeStatusV1::Running);
    assert!(result.checkpoint_artifact_hash.is_some());
    assert!(result.journal_artifact_hash.is_some());
    let (summary, journal_hash) = read_apartment_autonomy_execution_summary(
        scenario.temp_dir.path(),
        scenario.apartment_name.as_ref(),
        &run.run_id,
    )
    .map_err(|error| eyre::eyre!("{error:?}"))?
    .expect("persisted autonomy execution summary");
    assert_single_hf_autonomy_summary(
        &summary,
        scenario.service.bundle.service.service_name.as_ref(),
    );
    assert_eq!(result.journal_artifact_hash, Some(journal_hash));
    assert_eq!(
        result.checkpoint_artifact_hash,
        summary.checkpoint_artifact_hash
    );
    let second = handle
        .execute_apartment(request)
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert_eq!(second.result_commitment, result.result_commitment);
    assert_eq!(second.journal_artifact_hash, result.journal_artifact_hash);
    assert_eq!(
        second.checkpoint_artifact_hash,
        result.checkpoint_artifact_hash
    );
    Ok(())
}
#[test]
fn execute_apartment_generated_hf_autonomy_workflow_executes_multiple_steps_locally() -> Result<()>
{
    let scenario = prepare_generated_hf_autonomy_scenario(&GeneratedHfAutonomyScenario {
        service_name: "hf_agent_workflow_service",
        apartment_name: "hf_workflow_agent",
        local_peer_id: "12D3KooWHfWorkflowAutonomyRuntimeHost",
        run_id: "hf_workflow_agent:autonomy:9",
        artifact_hash: "hash:HFAGENT#WF",
        provenance_hash: "hash:HFPROV#WF",
        budget_units: 90,
        run_label: "workflow",
        workflow_input_json: "{\"workflow_version\":1,\"steps\":[{\"step_id\":\"draft\",\"request\":{\"inputs\":\"alpha\"}},{\"step_id\":\"refine\",\"request\":{\"inputs\":\"${steps.draft.text}\",\"parameters\":{\"max_new_tokens\":2}}}]}",
        approved_sequence: 9,
        request_commitment_seed: b"hf-agent-workflow-run",
        response_prefix: "wf:",
    })?;
    let handle = scenario.handle();
    let run = &scenario.run;
    let result = handle
        .execute_apartment(SoracloudApartmentExecutionRequest {
            observed_height: 0,
            observed_block_hash: None,
            apartment_name: scenario.apartment_name.to_string(),
            process_generation: scenario.process_generation,
            operation: format!("autonomy-run:{}", run.run_id),
            request_commitment: run.request_commitment,
        })
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    let (summary, _journal_hash) = read_apartment_autonomy_execution_summary(
        scenario.temp_dir.path(),
        scenario.apartment_name.as_ref(),
        &run.run_id,
    )
    .map_err(|error| eyre::eyre!("{error:?}"))?
    .expect("persisted workflow summary");
    assert!(summary.succeeded);
    assert_eq!(summary.workflow_steps.len(), 2);
    assert_eq!(summary.workflow_steps[0].step_id.as_deref(), Some("draft"));
    assert_eq!(
        summary.workflow_steps[0]
            .response_json
            .as_ref()
            .and_then(|value| value.get("text"))
            .and_then(norito::json::Value::as_str),
        Some("wf:alpha")
    );
    assert_eq!(summary.workflow_steps[1].step_id.as_deref(), Some("refine"));
    assert_eq!(
        summary.workflow_steps[1]
            .response_json
            .as_ref()
            .and_then(|value| value.get("inputs"))
            .and_then(norito::json::Value::as_str),
        Some("wf:alpha")
    );
    let response_json = summary
        .response_json
        .as_ref()
        .expect("workflow response json");
    assert_eq!(
        response_json
            .get("step_count")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        response_json
            .get("final_response")
            .and_then(|value| value.get("text"))
            .and_then(norito::json::Value::as_str),
        Some("wf:wf:alpha")
    );
    assert_eq!(
        result.checkpoint_artifact_hash,
        summary.checkpoint_artifact_hash
    );
    Ok(())
}
