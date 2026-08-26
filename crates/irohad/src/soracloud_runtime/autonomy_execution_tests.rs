#[test]
fn autonomy_workflow_json_is_exact_and_requires_explicit_nullable_step_id() -> Result<()> {
    let canonical = norito::json!({
        "workflow_version": 1,
        "steps": [{"step_id": null, "request": {"inputs": "alpha"}}],
    });
    let parsed = parse_apartment_autonomy_workflow_spec(&canonical)
        .map_err(|error| eyre::eyre!("{error:?}"))?
        .expect("canonical workflow must be recognized");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].step_id, None);

    for noncanonical in [
        norito::json!({
            "workflow_version": 1,
            "steps": [{"request": {"inputs": "alpha"}}],
        }),
        norito::json!({
            "workflow_version": 1,
            "steps": [{
                "step_id": null,
                "request": {"inputs": "alpha"},
                "allow_bridge_fallback": false,
            }],
        }),
        norito::json!({
            "workflow_version": 1,
            "steps": [{
                "step_id": null,
                "request": {"inputs": "alpha"},
                "allow_bridge_fallback": "yes",
            }],
        }),
        norito::json!({
            "workflow_version": 1,
            "steps": [{"step_id": null, "request": {"inputs": "alpha"}}],
            "legacy_mode": true,
        }),
    ] {
        let error = parse_apartment_autonomy_workflow_spec(&noncanonical)
            .expect_err("noncanonical workflow JSON must fail closed");
        assert_eq!(
            error.kind,
            SoracloudRuntimeExecutionErrorKind::InvalidRequest
        );
        assert!(error.message.contains("not canonical V1 JSON"));
        assert!(
            error
                .to_string()
                .starts_with("Soracloud runtime invalid request error:"),
            "unexpected structured runtime error display: {error}"
        );
    }
    Ok(())
}

#[test]
fn execute_apartment_generated_hf_autonomy_run_stays_inert_and_persists_failure() -> Result<()> {
    let mut state = test_state_at_height_one()?;
    let fixture = insert_generated_hf_service_fixture(
        &mut state,
        "hf_agent_service",
        "openai-community/gpt2",
        TEST_HF_COMMIT_OID,
        "gpt2",
    )?;
    set_generated_hf_service_route_visibility(&mut state, &fixture, SoraRouteVisibilityV1::Public);
    let apartment_name: Name = "hf_agent".parse().expect("valid apartment name");
    let manifest = iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
        apartment_name.clone(),
        &fixture.bundle,
    );
    let run = SoraAgentAutonomyRunRecordV1 {
        run_id: "hf_agent:autonomy:42".to_owned(),
        artifact_hash: "hash:HFAGENT#01".to_owned(),
        provenance_hash: Some("hash:HFPROV#01".to_owned()),
        budget_units: 75,
        run_label: "fallback label".to_owned(),
        workflow_input_json: Some(
            "{\"inputs\":[\"alpha\",\"beta\"],\"parameters\":{\"max_new_tokens\":4}}".to_owned(),
        ),
        approved_process_generation: 1,
        request_commitment: Hash::new(b"hf-agent-run"),
        approved_sequence: 42,
    };
    let apartment = SoraAgentApartmentRecordV1 {
        schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        manifest_hash: Hash::new(Encode::encode(&manifest)),
        deployed_sequence: 1,
        lease_started_height: 1,
        lease_expires_height: 400,
        last_renewed_height: 1,
        restart_count: 0,
        last_restart_sequence: None,
        last_restart_reason: None,
        process_generation: 1,
        process_started_sequence: 1,
        last_active_sequence: 42,
        last_checkpoint_sequence: Some(42),
        checkpoint_count: 1,
        persistent_state: SoraAgentPersistentStateV1 {
            total_bytes: 128,
            key_sizes: BTreeMap::from([("/autonomy/hf_agent:autonomy:42".to_owned(), 128)]),
        },
        revoked_policy_capabilities: BTreeSet::new(),
        pending_wallet_requests: BTreeMap::new(),
        wallet_daily_spend: BTreeMap::new(),
        mailbox_queue: Vec::new(),
        autonomy_budget_ceiling_units: 1_000,
        autonomy_budget_remaining_units: 925,
        artifact_allowlist: BTreeMap::from([(
            run.artifact_hash.clone(),
            SoraAgentArtifactAllowRuleV1 {
                artifact_hash: run.artifact_hash.clone(),
                provenance_hash: run.provenance_hash.clone(),
                added_sequence: 41,
            },
        )]),
        autonomy_run_history: vec![run.clone()],
        manifest,
    };
    {
        let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
        world
            .soracloud_agent_apartments_mut_for_testing()
            .insert(apartment_name.to_string(), apartment.clone());
    }
    let local_peer_id = "12D3KooWHfAgentAutonomyRuntimeHost";
    insert_generated_hf_placement_fixture(
        &mut state,
        &fixture,
        SoraHfPlacementHostRoleV1::Primary,
        SoraHfPlacementHostStatusV1::Warm,
        local_peer_id,
    );
    let temp_dir = tempfile::tempdir()?;
    let _generation = publish_single_weight_hf_test_generation(
        temp_dir.path(),
        &fixture.source_id.to_string(),
        "openai-community/gpt2",
        "gpt2",
        TEST_HF_WEIGHT_PAYLOAD,
    )?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
        Arc::clone(&state),
    );
    manager.reconcile_once()?;
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let request = SoracloudApartmentExecutionRequest {
        observed_height: 1,
        observed_block_hash: committed_block_hash(&state.view()),
        apartment_name: apartment_name.to_string(),
        process_generation: apartment.process_generation,
        operation: format!("autonomy-run:{}", run.run_id),
        request_commitment: run.request_commitment,
    };
    let result = handle.execute_apartment(request.clone())?;
    assert_eq!(result.status, SoraAgentRuntimeStatusV1::Running);
    assert!(result.checkpoint_artifact_hash.is_none());
    assert!(result.journal_artifact_hash.is_some());
    let (summary, journal_hash) = read_apartment_autonomy_execution_summary(
        temp_dir.path(),
        apartment_name.as_ref(),
        &run.run_id,
        run.approved_process_generation,
        run.request_commitment,
    )?
    .expect("persisted autonomy execution summary");
    assert!(!summary.succeeded);
    assert!(summary.workflow_steps.is_empty());
    assert_eq!(
        summary.service_name.as_deref(),
        Some(fixture.bundle.service.service_name.as_ref())
    );
    assert!(summary.runtime_receipt.is_none());
    assert!(summary.response_json.is_none());
    assert!(summary.response_text.is_none());
    assert!(
        summary.error.as_deref().is_some_and(
            |error| error.contains("no enabled or explicitly requested runtime backend")
        )
    );
    assert_eq!(result.journal_artifact_hash, Some(journal_hash));
    assert_eq!(
        result.checkpoint_artifact_hash,
        summary.checkpoint_artifact_hash
    );
    let second = handle.execute_apartment(request)?;
    assert_eq!(second.result_commitment, result.result_commitment);
    assert_eq!(second.journal_artifact_hash, result.journal_artifact_hash);
    assert_eq!(
        second.checkpoint_artifact_hash,
        result.checkpoint_artifact_hash
    );
    Ok(())
}
#[test]
fn execute_apartment_generated_hf_autonomy_workflow_stays_inert_before_first_step() -> Result<()> {
    let mut state = test_state_at_height_one()?;
    let fixture = insert_generated_hf_service_fixture(
        &mut state,
        "hf_agent_workflow_service",
        "openai-community/gpt2",
        TEST_HF_COMMIT_OID,
        "gpt2",
    )?;
    set_generated_hf_service_route_visibility(&mut state, &fixture, SoraRouteVisibilityV1::Public);
    let apartment_name: Name = "hf_workflow_agent".parse().expect("valid apartment name");
    let manifest = iroha_core::soracloud_runtime::build_soracloud_hf_generated_agent_manifest(
        apartment_name.clone(),
        &fixture.bundle,
    );
    let run = SoraAgentAutonomyRunRecordV1 {
            run_id: "hf_workflow_agent:autonomy:9".to_owned(),
            artifact_hash: "hash:HFAGENT#WF".to_owned(),
            provenance_hash: Some("hash:HFPROV#WF".to_owned()),
            budget_units: 90,
            run_label: "workflow".to_owned(),
            workflow_input_json: Some(
                "{\"workflow_version\":1,\"steps\":[{\"step_id\":\"draft\",\"request\":{\"inputs\":\"alpha\"}},{\"step_id\":\"refine\",\"request\":{\"inputs\":\"${steps.draft.text}\",\"parameters\":{\"max_new_tokens\":2}}}]}"
                    .to_owned(),
            ),
            approved_process_generation: 1,
            request_commitment: Hash::new(b"hf-agent-workflow-run"),
            approved_sequence: 9,
        };
    let apartment = SoraAgentApartmentRecordV1 {
        schema_version: SORA_AGENT_APARTMENT_RECORD_VERSION_V1,
        manifest_hash: Hash::new(Encode::encode(&manifest)),
        deployed_sequence: 1,
        lease_started_height: 1,
        lease_expires_height: 400,
        last_renewed_height: 1,
        restart_count: 0,
        last_restart_sequence: None,
        last_restart_reason: None,
        process_generation: 1,
        process_started_sequence: 1,
        last_active_sequence: 9,
        last_checkpoint_sequence: Some(9),
        checkpoint_count: 1,
        persistent_state: SoraAgentPersistentStateV1 {
            total_bytes: 128,
            key_sizes: BTreeMap::from([("/autonomy/hf_workflow_agent:autonomy:9".to_owned(), 128)]),
        },
        revoked_policy_capabilities: BTreeSet::new(),
        pending_wallet_requests: BTreeMap::new(),
        wallet_daily_spend: BTreeMap::new(),
        mailbox_queue: Vec::new(),
        autonomy_budget_ceiling_units: 1_000,
        autonomy_budget_remaining_units: 910,
        artifact_allowlist: BTreeMap::from([(
            run.artifact_hash.clone(),
            SoraAgentArtifactAllowRuleV1 {
                artifact_hash: run.artifact_hash.clone(),
                provenance_hash: run.provenance_hash.clone(),
                added_sequence: 8,
            },
        )]),
        autonomy_run_history: vec![run.clone()],
        manifest,
    };
    {
        let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
        world
            .soracloud_agent_apartments_mut_for_testing()
            .insert(apartment_name.to_string(), apartment.clone());
    }
    let local_peer_id = "12D3KooWHfWorkflowAutonomyRuntimeHost";
    insert_generated_hf_placement_fixture(
        &mut state,
        &fixture,
        SoraHfPlacementHostRoleV1::Primary,
        SoraHfPlacementHostStatusV1::Warm,
        local_peer_id,
    );
    let temp_dir = tempfile::tempdir()?;
    let _generation = publish_single_weight_hf_test_generation(
        temp_dir.path(),
        &fixture.source_id.to_string(),
        "openai-community/gpt2",
        "gpt2",
        TEST_HF_WEIGHT_PAYLOAD,
    )?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf())
            .with_local_host_identity(ALICE_ID.clone(), local_peer_id),
        Arc::clone(&state),
    );
    manager.reconcile_once()?;
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let result = handle.execute_apartment(SoracloudApartmentExecutionRequest {
        observed_height: 1,
        observed_block_hash: committed_block_hash(&state.view()),
        apartment_name: apartment_name.to_string(),
        process_generation: apartment.process_generation,
        operation: format!("autonomy-run:{}", run.run_id),
        request_commitment: run.request_commitment,
    })?;
    let (summary, _journal_hash) = read_apartment_autonomy_execution_summary(
        temp_dir.path(),
        apartment_name.as_ref(),
        &run.run_id,
        run.approved_process_generation,
        run.request_commitment,
    )?
    .expect("persisted workflow summary");
    assert!(!summary.succeeded);
    assert!(summary.workflow_steps.is_empty());
    assert!(summary.runtime_receipt.is_none());
    assert!(summary.response_json.is_none());
    assert!(
        summary.error.as_deref().is_some_and(
            |error| error.contains("no enabled or explicitly requested runtime backend")
        )
    );
    assert!(result.checkpoint_artifact_hash.is_none());
    assert_eq!(
        result.checkpoint_artifact_hash,
        summary.checkpoint_artifact_hash
    );
    Ok(())
}
