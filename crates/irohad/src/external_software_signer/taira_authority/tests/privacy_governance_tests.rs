// Privacy-governance authority tests included in the parent test module.

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "governance signing and history regression"
)]
fn privacy_governance_retained_key_authorizes_real_transaction_and_verifies_historically() {
    const GENESIS_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    const GENESIS_PRIVATE_KEY: &str =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";
    const GOVERNANCE_ISSUED_AT_MILLIS: u64 = 1_800_000_000_000;
    const GOVERNANCE_EXPIRES_AT_MILLIS: u64 = 1_800_000_060_000;
    const GOLDEN_REQUEST: &[u8] = include_bytes!(
        "../../../../../../scripts/tests/fixtures/taira_privacy_governance_request_v1.json"
    );

    let parent = temporary_parent();
    let retained_genesis_key = KeyPair::new(
        GENESIS_PUBLIC_KEY.parse().expect("fixture public key"),
        GENESIS_PRIVATE_KEY.parse().expect("fixture private key"),
    )
    .expect("matching retained genesis test key");
    let state = state_directory(parent.path());
    let service = TairaAuthorityServiceV1::provision_with_retained_genesis_key(
        &state,
        provisioning(TairaAuthorityRoleV1::PrivacyGovernance),
        wrapping_key(),
        retained_genesis_key,
    )
    .expect("provision retained-key governance authority");
    let binding = service.public_binding().expect("governance binding");
    assert_eq!(binding.signer.public_key.to_string(), GENESIS_PUBLIC_KEY);
    assert_eq!(
        service
            .provenance()
            .expect("finalized genesis provenance")
            .audit_sequence,
        2,
        "provisioning must durably commit FinalizePrivacyGenesisV1"
    );

    let fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PrivacyGovernance,
        parse_json(GOLDEN_REQUEST),
        Value::Array(Vec::new()),
    );
    let assignment = service
        .assign_run_json(
            &fixture.assignment_json(
                &service,
                GOVERNANCE_ISSUED_AT_MILLIS - 20,
                GOVERNANCE_ISSUED_AT_MILLIS - 10,
                GOVERNANCE_EXPIRES_AT_MILLIS + 30_000,
            ),
            GOVERNANCE_ISSUED_AT_MILLIS - 5,
        )
        .expect("assign exact governance request");
    assert_eq!(assignment.status, OperationStatusV1::Ok);

    let authorized = service
        .authorize_json(
            &fixture.request_json(),
            Vec::new(),
            binding.signer.client_uid,
            GOVERNANCE_ISSUED_AT_MILLIS + 1,
        )
        .expect("authorize exact native TransactionPayload");
    assert_eq!(authorized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&authorized), "authorized");
    let result = parse_json(&authorized.result_json);
    let durable_receipt = result
        .get("durable_receipt")
        .and_then(Value::as_object)
        .expect("governance durable receipt");
    assert_eq!(
        durable_receipt
            .get("authority_public_key")
            .and_then(Value::as_str),
        Some(GENESIS_PUBLIC_KEY)
    );
    assert_eq!(
        durable_receipt.get("status").and_then(Value::as_str),
        Some("signed")
    );
    let envelope = sidecar_bytes(&authorized, "authority_envelope");
    let receipt = sidecar_bytes(&authorized, "durable_receipt");
    let verification = verification_json(&fixture, &authorized);
    let after_authorization = service.provenance().expect("authorization provenance");

    drop(service);
    let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
        .expect("recover retained-key governance authority");
    let verified = recovered
        .verify_json(&verification, Vec::new(), binding.signer.client_uid)
        .expect("historically verify signed governance transaction");
    assert_eq!(verified.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&verified), "valid");
    assert_eq!(
        recovered
            .provenance()
            .expect("historical verification provenance"),
        after_authorization,
        "historical verification must neither consume replay state nor re-sign"
    );

    let retry = recovered
        .authorize_json(
            &fixture.request_json(),
            Vec::new(),
            binding.signer.client_uid,
            GOVERNANCE_EXPIRES_AT_MILLIS + 1,
        )
        .expect("recover byte-identical result after request expiry");
    assert_eq!(retry.status, OperationStatusV1::Replayed);
    assert_eq!(sidecar_bytes(&retry, "authority_envelope"), envelope);
    assert_eq!(sidecar_bytes(&retry, "durable_receipt"), receipt);
    assert_eq!(
        recovered.provenance().expect("retry provenance"),
        after_authorization,
        "an exact retry must not re-sign"
    );
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "governance durable crash-boundary matrix"
)]
fn privacy_governance_recovers_the_durable_admission_across_crash_boundaries() {
    const GENESIS_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    const GENESIS_PRIVATE_KEY: &str =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";
    const GOVERNANCE_ISSUED_AT_MILLIS: u64 = 1_800_000_000_000;
    const GOVERNANCE_EXPIRES_AT_MILLIS: u64 = 1_800_000_060_000;
    const GOLDEN_REQUEST: &[u8] = include_bytes!(
        "../../../../../../scripts/tests/fixtures/taira_privacy_governance_request_v1.json"
    );

    for crash_after_signer_commit in [false, true] {
        let parent = temporary_parent();
        let retained_genesis_key = KeyPair::new(
            GENESIS_PUBLIC_KEY.parse().expect("fixture public key"),
            GENESIS_PRIVATE_KEY.parse().expect("fixture private key"),
        )
        .expect("matching retained genesis test key");
        let state = state_directory(parent.path());
        let service = TairaAuthorityServiceV1::provision_with_retained_genesis_key(
            &state,
            provisioning(TairaAuthorityRoleV1::PrivacyGovernance),
            wrapping_key(),
            retained_genesis_key,
        )
        .expect("provision retained-key governance authority");
        let binding = service.public_binding().expect("governance binding");

        let subject = parse_json(GOLDEN_REQUEST);
        let manifest = Value::Array(Vec::new());
        let subject_sha256: [u8; 32] = Sha256::digest(canonical_json_core(&subject)).into();
        let manifest_sha256: [u8; 32] = Sha256::digest(canonical_json_core(&manifest)).into();
        let role = TairaAuthorityRoleV1::PrivacyGovernance;
        let run_id = digest_parts_sha256(
            RUN_ID_DOMAIN_V1,
            &[role.as_str().as_bytes(), &subject_sha256],
        );
        let operation_id = digest_parts_sha256(
            OPERATION_ID_DOMAIN_V1,
            &[
                role.as_str().as_bytes(),
                &run_id,
                &subject_sha256,
                &manifest_sha256,
            ],
        );
        let fixture = ClientRequestFixtureV1 {
            role,
            subject,
            manifest,
            run_id,
            operation_id,
            subject_sha256,
            manifest_sha256,
        };
        service
            .assign_run_json(
                &fixture.assignment_json(
                    &service,
                    GOVERNANCE_ISSUED_AT_MILLIS - 20,
                    GOVERNANCE_ISSUED_AT_MILLIS - 10,
                    GOVERNANCE_EXPIRES_AT_MILLIS + 30_000,
                ),
                GOVERNANCE_ISSUED_AT_MILLIS - 5,
            )
            .expect("assign exact governance request");
        let request_json = fixture.request_json();
        let admitted_at_unix_millis = GOVERNANCE_ISSUED_AT_MILLIS + 1;
        let before_authorization = service.provenance().expect("pre-authorization provenance");

        let signer_commit = if crash_after_signer_commit {
            let receipts = state.join("authority-receipts-v1");
            let parked_receipts = state.join("authority-receipts-v1.parked");
            fs::rename(&receipts, &parked_receipts).expect("park receipt directory");
            fs::write(&receipts, b"block receipt persistence")
                .expect("block receipt directory recreation");
            assert_eq!(
                service.authorize_json(
                    &request_json,
                    Vec::new(),
                    binding.signer.client_uid,
                    admitted_at_unix_millis,
                ),
                Err(TairaAuthorityErrorV1::State),
                "receipt persistence failure must stop after the signer commit"
            );
            let committed = service.provenance().expect("post-signer-crash provenance");
            assert_eq!(
                committed.audit_sequence,
                before_authorization.audit_sequence + 1,
                "the injected persistence failure must occur after the signer commit"
            );
            fs::remove_file(&receipts).expect("remove receipt persistence blocker");
            fs::rename(&parked_receipts, &receipts).expect("restore receipt directory");
            Some(committed)
        } else {
            let consumption = ReplayConsumptionV1 {
                run_id,
                operation_id,
                request_sha256: Sha256::digest(&request_json).into(),
                subject_sha256,
                artifact_manifest_sha256: manifest_sha256,
                consumed_at_unix_millis: admitted_at_unix_millis,
            };
            assert_eq!(
                persist_canonical_once(
                    &state.join("authority-replay-consumptions-v1"),
                    run_id,
                    &consumption,
                ),
                Ok(PersistOutcomeV1::Fresh),
                "simulate a crash immediately after durable consumption"
            );
            None
        };

        drop(service);
        let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
            .expect("recover governance authority at durable boundary");
        let authorized = recovered
            .authorize_json(
                &request_json,
                Vec::new(),
                binding.signer.client_uid,
                admitted_at_unix_millis + 1,
            )
            .expect("complete the durably admitted governance request");
        assert_eq!(authorized.status, OperationStatusV1::Ok);
        let after_recovery = recovered.provenance().expect("recovered provenance");
        if let Some(signer_commit) = signer_commit {
            assert_eq!(
                after_recovery, signer_commit,
                "recovery must replay the signer commit rather than append another"
            );
        } else {
            assert_eq!(
                after_recovery.audit_sequence,
                before_authorization.audit_sequence + 1,
                "recovery after consumption must append exactly one signer commit"
            );
        }

        let stored: BTreeMap<[u8; 32], StoredAuthorizationV1> =
            load_canonical_records(&state.join("authority-receipts-v1"))
                .expect("load recovered governance authorization");
        let stored = stored
            .get(&operation_id)
            .expect("recovered governance authorization");
        assert_eq!(
            stored.admitted_at_unix_millis, admitted_at_unix_millis,
            "stored admission must be the durable consumption time"
        );
        assert_eq!(
            stored.admitted_at_unix_millis,
            stored.consumption.consumed_at_unix_millis
        );
        assert_ne!(
            stored.admitted_at_unix_millis, GOVERNANCE_ISSUED_AT_MILLIS,
            "request issuance and durable admission are distinct timestamps"
        );

        if !crash_after_signer_commit {
            let mut tampered = stored.clone();
            tampered.admitted_at_unix_millis += 1;
            drop(recovered);
            let receipt_path = record_path(&state.join("authority-receipts-v1"), operation_id);
            fs::remove_file(&receipt_path).expect("remove valid authorization fixture");
            assert_eq!(
                persist_canonical_once(
                    &state.join("authority-receipts-v1"),
                    operation_id,
                    &tampered,
                ),
                Ok(PersistOutcomeV1::Fresh)
            );
            assert!(
                matches!(
                    TairaAuthorityServiceV1::open(&state, wrapping_key()),
                    Err(TairaAuthorityErrorV1::State)
                ),
                "recovery must reject admission timestamps detached from consumption"
            );
        }
    }
}
