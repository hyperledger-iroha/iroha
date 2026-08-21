// Generic authorization and run-assignment tests included in the parent test module.

#[test]
fn identifiers_match_python_length_framed_golden_vector() {
    let subject_sha256: [u8; 32] = Sha256::digest(br#"{"a":1,"b":2}"#).into();
    let manifest_sha256: [u8; 32] = Sha256::digest(
        br#"[{"name":"evidence/one","ordinal":0,"sha256":"7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed","size":3}]"#,
    )
    .into();
    let run_id = digest_parts_sha256(
        b"iroha:taira:authority-run-id:v1\0",
        &[b"native-evidence", &subject_sha256],
    );
    assert_eq!(
        hex::encode(run_id),
        "e0f3893153fa637143efe8ba0119af50776b822bab7300d638b424e74096b357"
    );
    assert_eq!(
        hex::encode(digest_parts_sha256(
            b"iroha:taira:authority-operation-id:v1\0",
            &[
                b"native-evidence",
                &run_id,
                &subject_sha256,
                &manifest_sha256,
            ],
        )),
        "b7bb54043e91ce00c7333bf880c2c0b2dd2fab5d2edf4ca48525a30917c2f647"
    );
}

#[test]
fn provision_assign_authorize_retry_verify_and_recover() {
    let parent = temporary_parent();
    let artifact = create_artifact(parent.path(), "observation.json", b"observation-v1");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "authorize-recovery",
        &[("observation.json", b"observation-v1")],
    );
    let service = provision(parent.path(), fixture.role);
    assert_eq!(
        parse_json(&service.status_json().expect("initial status"))["status"],
        Value::from("ready")
    );
    assign_active_run(&service, &fixture);

    let fresh = authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1,
    )
    .expect("fresh authorization");
    assert_eq!(fresh.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&fresh), "authorized");
    let envelope = sidecar_bytes(&fresh, "authority_envelope");
    let durable_receipt = sidecar_bytes(&fresh, "durable_receipt");

    let audit_after_fresh = service.provenance().expect("fresh provenance");
    let replay = authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 1,
    )
    .expect("exact retry");
    assert_eq!(replay.status, OperationStatusV1::Replayed);
    assert_eq!(result_status(&replay), "replayed");
    assert_eq!(sidecar_bytes(&replay, "authority_envelope"), envelope);
    assert_eq!(sidecar_bytes(&replay, "durable_receipt"), durable_receipt);
    assert_eq!(
        service.provenance().expect("replay provenance").audit_head,
        audit_after_fresh.audit_head,
        "an exact retry must not append another audit record"
    );

    let verified = service
        .verify_json(
            &verification_json(&fixture, &fresh),
            read_only_descriptors(&[&artifact]),
            service
                .public_binding()
                .expect("verification binding")
                .signer
                .client_uid,
        )
        .expect("historical verification");
    assert_eq!(verified.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&verified), "valid");
    assert_eq!(
        service.provenance().expect("verify provenance").audit_head,
        audit_after_fresh.audit_head,
        "historical verification must not sign"
    );

    drop(service);
    let state = state_directory(parent.path());
    let receipts = state.join("authority-receipts-v1");
    fs::rename(
        record_path(&receipts, fixture.operation_id),
        pending_record_path(&receipts, fixture.operation_id),
    )
    .expect("simulate receipt promotion crash");
    let recovered =
        TairaAuthorityServiceV1::open(&state, wrapping_key()).expect("open recovered authority");
    assert!(record_path(&receipts, fixture.operation_id).is_file());
    assert!(!pending_record_path(&receipts, fixture.operation_id).exists());
    let replay_after_recovery = authorize(
        &recovered,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 2,
    )
    .expect("retry recovered authorization");
    assert_eq!(replay_after_recovery.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&replay_after_recovery, "authority_envelope"),
        envelope
    );
    assert_eq!(
        sidecar_bytes(&replay_after_recovery, "durable_receipt"),
        durable_receipt
    );
    assert_eq!(
        recovered.provenance().expect("recovered provenance"),
        audit_after_fresh
    );
}

#[test]
fn generic_authorization_recovers_each_durable_crash_boundary_after_run_expiry() {
    for (case, phase) in [
        (
            "crash-after-consumption-persistence",
            GenericAuthorizationCrashPhaseV1::AfterConsumptionPersistence,
        ),
        (
            "crash-after-envelope-signer-commit",
            GenericAuthorizationCrashPhaseV1::AfterEnvelopeSignerCommit,
        ),
        (
            "crash-after-durable-receipt-signer-commit",
            GenericAuthorizationCrashPhaseV1::AfterDurableReceiptSignerCommit,
        ),
    ] {
        let parent = temporary_parent();
        let fixture =
            ClientRequestFixtureV1::new(TairaAuthorityRoleV1::RolloutObservation, case, &[]);
        let service = provision(parent.path(), fixture.role);
        assign_active_run(&service, &fixture);
        service
            .inject_generic_authorization_crash_for_test(phase)
            .expect("configure isolated crash phase");
        assert_eq!(
            authorize(
                &service,
                &fixture.request_json(),
                Vec::new(),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::State),
            "generic authorization did not stop at {phase:?}"
        );
        let audit_at_crash = service.provenance().expect("crash-boundary provenance");
        assert_eq!(
            parse_json(
                &service
                    .status_json()
                    .expect("status during incomplete authorization")
            )["status"],
            Value::from("ready"),
            "status must remain available during {phase:?}"
        );
        let blocked_assignment = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::RolloutObservation,
            &format!("{case}-blocked-assignment"),
            &[],
        );
        assert_eq!(
            service.assign_run_json(
                &blocked_assignment.assignment_json(
                    &service,
                    TEST_NOW_MILLIS_V1 - 10,
                    TEST_NOW_MILLIS_V1 - 1,
                    TEST_NOW_MILLIS_V1 + 60_000,
                ),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "assign-run appended across incomplete authorization at {phase:?}"
        );
        assert_eq!(
            service.administer(
                AuthorityAdminCommandV1::Revoke {
                    operation_id: digest_parts_sha256(
                        b"iroha:taira:test-incomplete-revoke:v1\0",
                        &[case.as_bytes()],
                    ),
                    expected_audit_head: audit_at_crash.audit_head,
                    expected_key_revision: service
                        .public_binding()
                        .expect("incomplete revoke binding")
                        .signer
                        .key_revision,
                    reason_digest: [0xE7; 32],
                },
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "revoke appended across incomplete authorization at {phase:?}"
        );
        assert_eq!(
            service.provenance().expect("blocked admin provenance"),
            audit_at_crash,
            "blocked admin command changed the audit head at {phase:?}"
        );
        drop(service);

        let recovered =
            TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
                .expect("recover authority after injected crash");
        let completed = authorize(
            &recovered,
            &fixture.request_json(),
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 60_001,
        )
        .expect("complete persisted admission after original assignment expiry");
        assert_eq!(completed.status, OperationStatusV1::Ok);
        assert_eq!(result_status(&completed), "authorized");
        let envelope = sidecar_bytes(&completed, "authority_envelope");
        let receipt = sidecar_bytes(&completed, "durable_receipt");
        let audit_after_completion = recovered
            .provenance()
            .expect("completed recovery provenance");
        let stored: BTreeMap<[u8; 32], StoredAuthorizationV1> =
            load_canonical_records(&state_directory(parent.path()).join("authority-receipts-v1"))
                .expect("load recovered generic authorization");
        let stored = stored
            .get(&fixture.operation_id)
            .expect("recovered generic authorization record");
        assert_eq!(
            stored.durable_receipt.commit_sequence,
            stored.envelope_receipt.commit_sequence + 1,
            "generic signer commits were not adjacent after {phase:?}"
        );

        let exact_retry = authorize(
            &recovered,
            &fixture.request_json(),
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 60_002,
        )
        .expect("retry completed recovered authorization");
        assert_eq!(exact_retry.status, OperationStatusV1::Replayed);
        assert_eq!(sidecar_bytes(&exact_retry, "authority_envelope"), envelope);
        assert_eq!(sidecar_bytes(&exact_retry, "durable_receipt"), receipt);
        assert_eq!(
            recovered.provenance().expect("exact retry provenance"),
            audit_after_completion,
            "exact retry appended an audit record after {phase:?}"
        );

        let conflict_path = create_artifact(parent.path(), "conflicting-reuse.bin", b"conflict");
        let conflicting = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::RolloutObservation,
            case,
            &[("conflicting-reuse.bin", b"conflict")],
        );
        assert_eq!(conflicting.run_id, fixture.run_id);
        assert_ne!(conflicting.operation_id, fixture.operation_id);
        assert_eq!(
            authorize(
                &recovered,
                &conflicting.request_json(),
                read_only_descriptors(&[&conflict_path]),
                TEST_NOW_MILLIS_V1 + 60_003,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "conflicting replay was accepted after {phase:?}"
        );
    }
}

#[test]
fn assignment_conflicts_and_run_windows_fail_closed() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);
    let active = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "assignment-reuse",
        &[("result.json", b"result")],
    );

    let future_issued = active.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 + 1,
        TEST_NOW_MILLIS_V1 + 1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service.assign_run_json(&future_issued, TEST_NOW_MILLIS_V1),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let active_assignment = active.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service
            .assign_run_json(&active_assignment, TEST_NOW_MILLIS_V1)
            .expect("active assignment")
            .status,
        OperationStatusV1::Ok
    );
    assert_eq!(
        service
            .assign_run_json(&active_assignment, TEST_NOW_MILLIS_V1)
            .expect("assignment retry")
            .status,
        OperationStatusV1::Replayed
    );
    let mut conflicting = parse_json(&active_assignment);
    conflicting
        .as_object_mut()
        .expect("assignment object")
        .insert("subject_sha256".into(), Value::from("91".repeat(32)));
    assert_eq!(
        service.assign_run_json(&canonical_json_line(&conflicting), TEST_NOW_MILLIS_V1),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    let future = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "future-run",
        &[("future.json", b"future")],
    );
    service
        .assign_run_json(
            &future.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1 + 10,
                TEST_NOW_MILLIS_V1 + 100,
            ),
            TEST_NOW_MILLIS_V1,
        )
        .expect("future-not-before assignment");
    let future_artifact = create_artifact(parent.path(), "future.json", b"future");
    assert_eq!(
        authorize(
            &service,
            &future.request_json(),
            read_only_descriptors(&[&future_artifact]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let stale = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "stale-run",
        &[("stale.json", b"stale")],
    );
    service
        .assign_run_json(
            &stale.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 20,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1,
            ),
            TEST_NOW_MILLIS_V1,
        )
        .expect("expired assignment is retained for historical audit");
    let stale_artifact = create_artifact(parent.path(), "stale.json", b"stale");
    assert_eq!(
        authorize(
            &service,
            &stale.request_json(),
            read_only_descriptors(&[&stale_artifact]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let too_long = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "too-long-run",
        &[("too-long.json", b"too-long")],
    );
    assert_eq!(
        service.assign_run_json(
            &too_long.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1 + 24 * 60 * 60 * 1_000 + 1,
            ),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );
}

#[test]
fn native_assignment_authenticates_controller_identity_and_unique_run_nonce() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::NativeEvidence);
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::NativeEvidence,
        "native-controller-assignment",
        &[],
    );
    let assignment = fixture.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 60_000,
    );
    let assigned = service
        .assign_run_json(&assignment, TEST_NOW_MILLIS_V1)
        .expect("assign native controller-bound run");
    assert_eq!(assigned.status, OperationStatusV1::Ok);
    let assigned_result = parse_json(&assigned.result_json);
    let signed_assignment = assigned_result
        .get("assignment")
        .and_then(Value::as_object)
        .expect("signed native assignment");
    assert_eq!(
        signed_assignment.get("controller_digest"),
        Some(&Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)))
    );
    assert_eq!(
        signed_assignment.get("controller_host_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_HOST_ID_V1))
    );
    assert_eq!(
        signed_assignment.get("controller_installation_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_INSTALLATION_ID_V1))
    );
    assert_eq!(
        signed_assignment.get("run_nonce"),
        Some(&Value::from(hex::encode(TEST_NATIVE_RUN_NONCE_V1)))
    );
    assert_eq!(
        service
            .assign_run_json(&assignment, TEST_NOW_MILLIS_V1)
            .expect("retry exact native assignment")
            .status,
        OperationStatusV1::Replayed
    );

    let second = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::NativeEvidence,
        "native-controller-assignment-second-run",
        &[],
    );
    assert_eq!(
        service.assign_run_json(
            &second.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1 - 1,
                TEST_NOW_MILLIS_V1 + 60_000,
            ),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    let reject_field = |field: &str, replacement: Option<Value>| {
        let parent = temporary_parent();
        let service = provision(parent.path(), TairaAuthorityRoleV1::NativeEvidence);
        let fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::NativeEvidence,
            "invalid-native-controller-assignment",
            &[],
        );
        let mut assignment = parse_json(&fixture.assignment_json(
            &service,
            TEST_NOW_MILLIS_V1 - 10,
            TEST_NOW_MILLIS_V1 - 1,
            TEST_NOW_MILLIS_V1 + 60_000,
        ));
        let assignment = assignment
            .as_object_mut()
            .expect("native assignment object");
        if let Some(replacement) = replacement {
            assignment.insert(field.to_owned(), replacement);
        } else {
            assignment.remove(field);
        }
        assert_eq!(
            service.assign_run_json(
                &canonical_json_line(&Value::Object(assignment.clone())),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Rejected),
            "invalid native assignment field was accepted: {field}"
        );
    };
    for field in [
        "controller_digest",
        "controller_host_id",
        "controller_installation_id",
        "run_nonce",
    ] {
        reject_field(field, None);
    }
    reject_field("controller_digest", Some(Value::from("00".repeat(32))));
    reject_field("controller_host_id", Some(Value::from("Uppercase-host")));
    reject_field(
        "controller_installation_id",
        Some(Value::from("installation/with/path")),
    );
    reject_field("run_nonce", Some(Value::from("00".repeat(32))));

    let other_parent = temporary_parent();
    let other_service = provision(
        other_parent.path(),
        TairaAuthorityRoleV1::RolloutObservation,
    );
    let other = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "foreign-controller-binding",
        &[],
    );
    let mut other_assignment = parse_json(&other.assignment_json(
        &other_service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 60_000,
    ));
    other_assignment
        .as_object_mut()
        .expect("other assignment object")
        .insert(
            "controller_digest".into(),
            Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)),
        );
    assert_eq!(
        other_service.assign_run_json(&canonical_json_line(&other_assignment), TEST_NOW_MILLIS_V1,),
        Err(TairaAuthorityErrorV1::Rejected)
    );
}
