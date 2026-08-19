#[test]
fn deploy_dry_run_does_not_consume_and_apply_and_finalize_are_once_only() {
    let parent = temporary_parent();
    let artifact = create_artifact(parent.path(), "deployment.json", b"deployment");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::DeployIssuance,
        "deployment",
        &[("deployment.json", b"deployment")],
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);
    let after_assignment = service.provenance().expect("assigned provenance");

    let dry_run_request = fixture.request_json_with_deploy(Some("dry-run"), None);
    let dry_run = authorize(
        &service,
        &dry_run_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1,
    )
    .expect("dry-run authorization");
    assert_eq!(dry_run.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&dry_run), "verified");
    assert_eq!(
        service.provenance().expect("dry-run provenance"),
        after_assignment,
        "dry-run must neither consume nor sign"
    );

    let finalization_result = [0x81; 32];
    let finalize_request =
        fixture.request_json_with_deploy(Some("finalize"), Some(("success", finalization_result)));
    assert_eq!(
        authorize(
            &service,
            &finalize_request,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 1,
        ),
        Err(TairaAuthorityErrorV1::Rejected),
        "a verified dry-run is not an applied authorization"
    );

    let apply_request = fixture.request_json_with_deploy(Some("apply"), None);
    let applied = authorize(
        &service,
        &apply_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 2,
    )
    .expect("apply authorization");
    assert_eq!(applied.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&applied), "authorized");
    let after_apply = service.provenance().expect("apply provenance");
    assert_eq!(
        after_apply.audit_sequence,
        after_assignment.audit_sequence + 2
    );

    let apply_replay = authorize(
        &service,
        &apply_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 3,
    )
    .expect("apply retry");
    assert_eq!(apply_replay.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&apply_replay, "authority_envelope"),
        sidecar_bytes(&applied, "authority_envelope")
    );
    assert_eq!(
        sidecar_bytes(&apply_replay, "durable_receipt"),
        sidecar_bytes(&applied, "durable_receipt")
    );
    assert_eq!(
        service.provenance().expect("apply replay provenance"),
        after_apply
    );

    let finalized = authorize(
        &service,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 4,
    )
    .expect("finalize deployment");
    assert_eq!(finalized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&finalized), "finalized");
    let final_envelope = sidecar_bytes(&finalized, "authority_envelope");
    let final_receipt = sidecar_bytes(&finalized, "durable_receipt");
    let after_finalize = service.provenance().expect("finalized provenance");
    assert_eq!(
        after_finalize.audit_sequence,
        after_apply.audit_sequence + 2
    );

    let finalize_replay = authorize(
        &service,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 5,
    )
    .expect("finalization retry");
    assert_eq!(finalize_replay.status, OperationStatusV1::Replayed);
    assert_eq!(result_status(&finalize_replay), "replayed");
    assert_eq!(
        sidecar_bytes(&finalize_replay, "authority_envelope"),
        final_envelope
    );
    assert_eq!(
        sidecar_bytes(&finalize_replay, "durable_receipt"),
        final_receipt
    );
    assert_eq!(
        service
            .provenance()
            .expect("finalization replay provenance"),
        after_finalize
    );

    let conflicting_finalize =
        fixture.request_json_with_deploy(Some("finalize"), Some(("rolled-back", [0x82; 32])));
    assert_eq!(
        authorize(
            &service,
            &conflicting_finalize,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 6,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    drop(service);
    let recovered = TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
        .expect("recover finalized deployment authority");
    let recovered_replay = authorize(
        &recovered,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 7,
    )
    .expect("replay finalization after recovery");
    assert_eq!(recovered_replay.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&recovered_replay, "authority_envelope"),
        final_envelope
    );
    assert_eq!(
        sidecar_bytes(&recovered_replay, "durable_receipt"),
        final_receipt
    );
}

#[test]
fn deployment_finalization_recovers_every_crash_phase_and_terminal_outcome_after_expiry() {
    for (phase, committed_finalization_records) in [
        (
            DeploymentFinalizationCrashPhaseV1::AfterInputPersistence,
            0_u64,
        ),
        (
            DeploymentFinalizationCrashPhaseV1::AfterDecisionSignerCommit,
            1,
        ),
        (
            DeploymentFinalizationCrashPhaseV1::AfterDurableReceiptSignerCommit,
            2,
        ),
    ] {
        for (outcome, result_byte) in [
            ("success", 0x91_u8),
            ("rolled-back", 0x92),
            ("rollback-failed", 0x93),
        ] {
            let parent = temporary_parent();
            let artifact = create_artifact(parent.path(), "deployment.json", b"deployment");
            let fixture = ClientRequestFixtureV1::new(
                TairaAuthorityRoleV1::DeployIssuance,
                &format!("deployment-{phase:?}-{outcome}"),
                &[("deployment.json", b"deployment")],
            );
            let service = provision(parent.path(), fixture.role);
            assign_active_run(&service, &fixture);
            let apply_request = fixture.request_json_with_deploy(Some("apply"), None);
            let applied = authorize(
                &service,
                &apply_request,
                read_only_descriptors(&[&artifact]),
                TEST_NOW_MILLIS_V1 + 1,
            )
            .expect("apply deployment before crash boundary");
            let apply_envelope = sidecar_bytes(&applied, "authority_envelope");
            let after_apply = service.provenance().expect("post-apply provenance");
            let finalize_request = fixture
                .request_json_with_deploy(Some("finalize"), Some((outcome, [result_byte; 32])));
            let finalized_at = TEST_NOW_MILLIS_V1 + 2;
            service
                .inject_deployment_finalization_crash_for_test(phase)
                .expect("configure isolated deployment-finalization crash");
            assert_eq!(
                authorize(&service, &finalize_request, Vec::new(), finalized_at),
                Err(TairaAuthorityErrorV1::State),
                "finalization did not stop at {phase:?} for {outcome}"
            );
            let at_crash = service
                .provenance()
                .expect("deployment-finalization crash provenance");
            assert_eq!(
                at_crash.audit_sequence,
                after_apply.audit_sequence + committed_finalization_records
            );
            let blocked_assignment = ClientRequestFixtureV1::new(
                TairaAuthorityRoleV1::DeployIssuance,
                &format!("deployment-{phase:?}-{outcome}-blocked"),
                &[],
            );
            assert_eq!(
                service.assign_run_json(
                    &blocked_assignment.assignment_json(
                        &service,
                        TEST_NOW_MILLIS_V1 - 1,
                        TEST_NOW_MILLIS_V1,
                        TEST_NOW_MILLIS_V1 + 60_000,
                    ),
                    TEST_NOW_MILLIS_V1,
                ),
                Err(TairaAuthorityErrorV1::Conflict),
                "assign-run appended across incomplete finalization at {phase:?} for {outcome}"
            );
            let state = state_directory(parent.path());
            let inputs: BTreeMap<[u8; 32], StoredDeploymentFinalizationInputV1> =
                load_canonical_records(&state.join("authority-deployment-finalization-inputs-v1"))
                    .expect("load durable finalization input");
            let input = inputs
                .get(&fixture.operation_id)
                .expect("crash-persisted finalization input");
            assert_eq!(input.finalized_at_unix_millis, finalized_at);
            assert_eq!(input.outcome, outcome);
            assert_eq!(input.result_sha256, [result_byte; 32]);
            let finalizations: BTreeMap<[u8; 32], StoredDeploymentFinalizationV1> =
                load_canonical_records(&state.join("authority-deployment-finalizations-v1"))
                    .expect("load absent crash finalization");
            assert!(finalizations.is_empty());
            drop(service);

            let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
                .expect("recover terminal finalization after crash");
            let recovered_provenance = recovered
                .provenance()
                .expect("recovered finalization provenance");
            assert_eq!(
                recovered_provenance.audit_sequence,
                after_apply.audit_sequence + 2,
                "recovery must produce exactly two terminal commits"
            );
            if committed_finalization_records == 2 {
                assert_eq!(recovered_provenance.audit_head, at_crash.audit_head);
            }
            let replay = authorize(
                &recovered,
                &finalize_request,
                Vec::new(),
                TEST_NOW_MILLIS_V1 + 60_001,
            )
            .expect("replay recovered finalization after assignment expiry");
            assert_eq!(replay.status, OperationStatusV1::Replayed);
            assert_eq!(result_status(&replay), "replayed");
            assert_eq!(
                sidecar_bytes(&replay, "authority_envelope"),
                apply_envelope,
                "finalization must preserve the applied authority envelope"
            );
            let replay_envelope = sidecar_bytes(&replay, "authority_envelope");
            let replay_receipt = sidecar_bytes(&replay, "durable_receipt");
            let audit_after_replay = recovered
                .provenance()
                .expect("recovered finalization replay provenance");
            assert_eq!(audit_after_replay, recovered_provenance);

            let exact_retry = authorize(
                &recovered,
                &finalize_request,
                Vec::new(),
                TEST_NOW_MILLIS_V1 + 60_002,
            )
            .expect("second exact recovered finalization retry");
            assert_eq!(
                sidecar_bytes(&exact_retry, "authority_envelope"),
                replay_envelope
            );
            assert_eq!(
                sidecar_bytes(&exact_retry, "durable_receipt"),
                replay_receipt
            );
            assert_eq!(
                recovered
                    .provenance()
                    .expect("second recovered retry provenance"),
                recovered_provenance,
                "exact retries must not append audit records"
            );

            let conflict = fixture.request_json_with_deploy(
                Some("finalize"),
                Some((
                    if outcome == "success" {
                        "rolled-back"
                    } else {
                        "success"
                    },
                    [result_byte.wrapping_add(1); 32],
                )),
            );
            assert_eq!(
                authorize(
                    &recovered,
                    &conflict,
                    Vec::new(),
                    TEST_NOW_MILLIS_V1 + 60_003,
                ),
                Err(TairaAuthorityErrorV1::Conflict)
            );
        }
    }
}

#[test]
fn deployment_finalization_rejects_canonical_input_sidecar_and_receipt_substitution() {
    let parent = temporary_parent();
    let artifact = create_artifact(parent.path(), "deployment.json", b"deployment");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::DeployIssuance,
        "deployment-finalization-substitution",
        &[("deployment.json", b"deployment")],
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);
    authorize(
        &service,
        &fixture.request_json_with_deploy(Some("apply"), None),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 1,
    )
    .expect("apply substitution fixture");
    let finalize_request =
        fixture.request_json_with_deploy(Some("finalize"), Some(("rollback-failed", [0xA1; 32])));
    authorize(
        &service,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 2,
    )
    .expect("finalize substitution fixture");
    let state = state_directory(parent.path());
    let inputs: BTreeMap<[u8; 32], StoredDeploymentFinalizationInputV1> =
        load_canonical_records(&state.join("authority-deployment-finalization-inputs-v1"))
            .expect("load finalization input fixture");
    let finalizations: BTreeMap<[u8; 32], StoredDeploymentFinalizationV1> =
        load_canonical_records(&state.join("authority-deployment-finalizations-v1"))
            .expect("load finalization fixture");
    let input = inputs
        .get(&fixture.operation_id)
        .expect("stored finalization input");
    let stored = finalizations
        .get(&fixture.operation_id)
        .expect("stored finalization");

    macro_rules! reject_input_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = input.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_stored_deployment_finalization_for_test(&mutated, stored)
                    .is_err(),
                "mutated finalization input was accepted: {}",
                $label
            );
        }};
    }
    reject_input_mutation!(
        "terminal request",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            let mut request = parse_json(&value.finalization_request_json);
            request
                .as_object_mut()
                .expect("terminal request object")
                .insert("disposition".into(), Value::from("apply"));
            value.finalization_request_json = canonical_json_line(&request);
        }
    );
    reject_input_mutation!(
        "terminal request digest",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.finalization_request_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "terminal timestamp",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.finalized_at_unix_millis += 1;
        }
    );
    reject_input_mutation!(
        "terminal outcome",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.outcome = "success".to_owned();
        }
    );
    reject_input_mutation!(
        "terminal result",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.result_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "signer policy",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "audit predecessor",
        |value: &mut StoredDeploymentFinalizationInputV1| {
            value.previous_audit_head[0] ^= 1;
        }
    );

    macro_rules! reject_stored_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = stored.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_stored_deployment_finalization_for_test(input, &mutated)
                    .is_err(),
                "mutated finalization sidecar was accepted: {}",
                $label
            );
        }};
    }
    reject_stored_mutation!(
        "stored outcome",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.outcome = "success".to_owned();
        }
    );
    reject_stored_mutation!(
        "stored result digest",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.result_sha256[0] ^= 1;
        }
    );
    reject_stored_mutation!(
        "stored timestamp",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.finalized_at_unix_millis += 1;
        }
    );
    reject_stored_mutation!(
        "authority envelope",
        |value: &mut StoredDeploymentFinalizationV1| {
            let mut envelope = parse_json(&value.authority_envelope_json);
            envelope
                .as_object_mut()
                .expect("authority envelope object")
                .insert("unexpected".into(), Value::from(true));
            value.authority_envelope_json = canonical_json_line(&envelope);
        }
    );
    reject_stored_mutation!(
        "durable receipt claims",
        |value: &mut StoredDeploymentFinalizationV1| {
            let mut receipt = parse_json(&value.durable_receipt_json);
            receipt
                .as_object_mut()
                .and_then(|object| object.get_mut("claims"))
                .and_then(Value::as_object_mut)
                .expect("durable receipt claims")
                .insert("outcome".into(), Value::from("success"));
            value.durable_receipt_json = canonical_json_line(&receipt);
        }
    );
    reject_stored_mutation!(
        "decision payload",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.decision_signing_payload[0] ^= 1;
        }
    );
    reject_stored_mutation!(
        "decision receipt",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.decision_receipt.commit_audit_head[0] ^= 1;
        }
    );
    reject_stored_mutation!(
        "receipt payload",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.receipt_signing_payload[0] ^= 1;
        }
    );
    reject_stored_mutation!(
        "durable receipt",
        |value: &mut StoredDeploymentFinalizationV1| {
            value.durable_receipt.signature[0] ^= 1;
        }
    );
    reject_stored_mutation!(
        "result envelope",
        |value: &mut StoredDeploymentFinalizationV1| {
            let mut result = parse_json(&value.result_json);
            result
                .as_object_mut()
                .expect("finalization result object")
                .insert("authority_envelope".into(), Value::Object(Map::new()));
            value.result_json = canonical_json_line(&result);
        }
    );
    reject_stored_mutation!(
        "result receipt",
        |value: &mut StoredDeploymentFinalizationV1| {
            let mut result = parse_json(&value.result_json);
            result
                .as_object_mut()
                .expect("finalization result object")
                .insert("durable_receipt".into(), Value::Object(Map::new()));
            value.result_json = canonical_json_line(&result);
        }
    );

    drop(service);
    let directory = state.join("authority-deployment-finalizations-v1");
    fs::remove_file(record_path(&directory, fixture.operation_id))
        .expect("remove valid finalization record");
    let mut substituted = stored.clone();
    let mut result = parse_json(&substituted.result_json);
    result
        .as_object_mut()
        .expect("finalization result object")
        .insert("status".into(), Value::from("replayed"));
    substituted.result_json = canonical_json_line(&result);
    assert_eq!(
        persist_canonical_once(&directory, fixture.operation_id, &substituted),
        Ok(PersistOutcomeV1::Fresh)
    );
    assert!(matches!(
        TairaAuthorityServiceV1::open(&state, wrapping_key()),
        Err(TairaAuthorityErrorV1::State)
    ));

    fs::remove_file(record_path(&directory, fixture.operation_id))
        .expect("remove substituted finalization record");
    assert_eq!(
        persist_canonical_once(&directory, fixture.operation_id, stored),
        Ok(PersistOutcomeV1::Fresh)
    );
    let input_directory = state.join("authority-deployment-finalization-inputs-v1");
    fs::remove_file(record_path(&input_directory, fixture.operation_id))
        .expect("remove valid finalization input");
    let mut substituted_input = input.clone();
    substituted_input.finalized_at_unix_millis += 1;
    assert_eq!(
        persist_canonical_once(&input_directory, fixture.operation_id, &substituted_input),
        Ok(PersistOutcomeV1::Fresh)
    );
    assert!(matches!(
        TairaAuthorityServiceV1::open(&state, wrapping_key()),
        Err(TairaAuthorityErrorV1::State)
    ));
}
