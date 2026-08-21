fn replay_record(case: u8) -> ReplayConsumptionV1 {
    ReplayConsumptionV1 {
        run_id: [case; 32],
        operation_id: [case.wrapping_add(1); 32],
        request_sha256: [case.wrapping_add(2); 32],
        subject_sha256: [case.wrapping_add(3); 32],
        artifact_manifest_sha256: [case.wrapping_add(4); 32],
        consumed_at_unix_millis: TEST_NOW_MILLIS_V1 + u64::from(case),
    }
}

#[test]
fn public_soak_binding_anchor_is_write_ahead_signed_and_open_stable() {
    let parent = temporary_parent();
    let state = public_soak_anchor_state_directory(parent.path());
    let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());
    assert_eq!(input.operation_id, anchor.operation_id);
    assert_eq!(input.replay_binding, anchor.replay_binding);
    assert_eq!(input.observation_binding, anchor.observation_binding);
    assert_eq!(input.signing_payload, anchor.signing_payload);
    assert_eq!(anchor.receipt.commit_sequence, 2);
    assert_eq!(
        anchor.previous_audit_head,
        input.replay_binding.signer.audit_genesis_digest
    );

    let provisioned = service.provenance().expect("provisioned anchor provenance");
    assert_eq!(provisioned.audit_sequence, 2);
    service
        .ensure_public_soak_observation_binding_anchor_for_test()
        .expect("exact anchor retry");
    assert_eq!(
        service.provenance().expect("retried anchor provenance"),
        provisioned,
        "an exact anchor retry must not append"
    );
    drop(service);

    let reopened = TairaAuthorityServiceV1::open(&state, wrapping_key())
        .expect("open signed observation binding anchor");
    assert_eq!(
        reopened.provenance().expect("reopened anchor provenance"),
        provisioned,
        "open must verify the persisted anchor without signing"
    );
    let reopened_inputs: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingInputV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-input-v1"))
            .expect("reload observation binding input");
    let reopened_anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-v1"))
            .expect("reload observation binding anchor");
    assert_eq!(
        reopened_inputs,
        BTreeMap::from([(input.operation_id, input)])
    );
    assert_eq!(
        reopened_anchors,
        BTreeMap::from([(anchor.operation_id, anchor)])
    );
}

#[test]
fn public_soak_binding_anchor_recovers_both_write_ahead_crash_phases() {
    for phase in [
        PublicSoakBindingCrashPhaseV1::AfterInputPersistence,
        PublicSoakBindingCrashPhaseV1::AfterSignerCommit,
    ] {
        let parent = temporary_parent();
        let state = public_soak_anchor_state_directory(parent.path());
        let observation_binding = independent_public_soak_observation_binding(parent.path());
        assert!(matches!(
            TairaAuthorityServiceV1::provision_with_public_soak_observation_binding_crash_for_test(
                &state,
                provisioning(TairaAuthorityRoleV1::PublicSoakReplayAdmission),
                wrapping_key(),
                observation_binding,
                phase,
            ),
            Err(TairaAuthorityErrorV1::State)
        ));
        let inputs: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingInputV1> =
            load_canonical_records(&state.join("public-soak-observation-binding-input-v1"))
                .expect("load crash-persisted binding input");
        assert_eq!(inputs.len(), 1);
        let anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
            load_canonical_records(&state.join("public-soak-observation-binding-v1"))
                .expect("load absent crash anchor directory");
        assert!(anchors.is_empty());
        let committed_audit_path = audit_record_path(&state, 2);
        let committed_audit_before_open =
            if phase == PublicSoakBindingCrashPhaseV1::AfterSignerCommit {
                Some(fs::read(&committed_audit_path).expect("read exact pre-crash signer commit"))
            } else {
                assert!(!committed_audit_path.exists());
                None
            };

        let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
            .expect("recover observation binding anchor from write-ahead input");
        let recovered_provenance = recovered
            .provenance()
            .expect("recovered binding anchor provenance");
        assert_eq!(recovered_provenance.audit_sequence, 2);
        if let Some(committed_audit_before_open) = committed_audit_before_open {
            assert_eq!(
                fs::read(&committed_audit_path).expect("read recovered exact signer commit"),
                committed_audit_before_open,
                "recovery after the signer commit must replay it without mutation"
            );
        }
        let anchor_path = state.join("public-soak-observation-binding-v1");
        let recovered_anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
            load_canonical_records(&anchor_path).expect("load recovered binding anchor");
        assert_eq!(recovered_anchors.len(), 1);
        let recovered_anchor_bytes = fs::read(record_path(
            &anchor_path,
            recovered_anchors
                .values()
                .next()
                .expect("recovered binding anchor")
                .operation_id,
        ))
        .expect("read recovered binding anchor bytes");
        recovered
            .ensure_public_soak_observation_binding_anchor_for_test()
            .expect("exact recovered anchor retry");
        assert_eq!(
            recovered
                .provenance()
                .expect("exact recovered anchor retry provenance"),
            recovered_provenance
        );
        drop(recovered);

        let reopened = TairaAuthorityServiceV1::open(&state, wrapping_key())
            .expect("reopen recovered binding anchor");
        assert_eq!(
            reopened
                .provenance()
                .expect("reopened recovered anchor provenance"),
            recovered_provenance
        );
        assert_eq!(
            fs::read(record_path(
                &anchor_path,
                recovered_anchors
                    .values()
                    .next()
                    .expect("stable recovered binding anchor")
                    .operation_id,
            ))
            .expect("reread recovered binding anchor bytes"),
            recovered_anchor_bytes,
            "subsequent opens must preserve byte-identical anchor state"
        );
    }
}

#[test]
fn public_soak_binding_anchor_rejects_full_binding_and_receipt_mutations() {
    let parent = temporary_parent();
    let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());

    macro_rules! reject_input_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = input.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_public_soak_observation_binding_anchor_for_test(&mutated, &anchor)
                    .is_err(),
                "write-ahead input mutation was accepted: {}",
                $label
            );
        }};
    }
    reject_input_mutation!(
        "operation_id",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.operation_id[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay outer magic",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.magic[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay outer version",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.version += 1;
        }
    );
    reject_input_mutation!(
        "replay outer role",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.role = TairaAuthorityRoleV1::PublicSoakObservation;
        }
    );
    reject_input_mutation!(
        "replay signer magic",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.magic[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay signer version",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.version += 1;
        }
    );
    reject_input_mutation!(
        "replay handle",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.handle.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay service identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .replay_binding
                .signer
                .service_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay administrator identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .replay_binding
                .signer
                .administrator_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay service UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.service_uid += 100;
        }
    );
    reject_input_mutation!(
        "replay client UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.client_uid += 101;
        }
    );
    reject_input_mutation!(
        "replay administrator UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.administrator_uid += 102;
        }
    );
    reject_input_mutation!(
        "replay signer role",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.role = SoftwareSignerRoleV1::EvidenceViewer;
        }
    );
    reject_input_mutation!(
        "replay purpose binding",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            let SoftwareSignerPurposeBindingV1::TairaAuthority { role } =
                &mut value.replay_binding.signer.purpose_binding
            else {
                unreachable!("Taira fixture purpose binding")
            };
            role.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay signing domain",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.domain.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay key algorithm",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.key_algorithm = SoftwareSignerKeyAlgorithmV1::MlDsa;
        }
    );
    reject_input_mutation!(
        "replay key revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.key_revision += 1;
        }
    );
    reject_input_mutation!(
        "replay policy revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.policy_revision += 1;
        }
    );
    reject_input_mutation!(
        "replay policy digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay public key",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.public_key =
                KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
                    .expect("replacement public key")
                    .public_key()
                    .clone();
        }
    );
    reject_input_mutation!(
        "replay public key digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.public_key_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay audit genesis",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.audit_genesis_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay maximum request bytes",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.max_request_bytes -= 1;
        }
    );
    reject_input_mutation!(
        "observation service identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .observation_binding
                .signer
                .service_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "observation administrator identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .observation_binding
                .signer
                .administrator_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "observation service UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.service_uid += 100;
        }
    );
    reject_input_mutation!(
        "observation client UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.client_uid += 101;
        }
    );
    reject_input_mutation!(
        "observation administrator UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.administrator_uid += 102;
        }
    );
    reject_input_mutation!(
        "observation key revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.key_revision += 1;
        }
    );
    reject_input_mutation!(
        "observation policy revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.policy_revision += 1;
        }
    );
    reject_input_mutation!(
        "observation policy digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "observation public key digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.public_key_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay binding digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "observation binding digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "signing payload",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.signing_payload[0] ^= 1;
        }
    );

    macro_rules! reject_anchor_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = anchor.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_public_soak_observation_binding_anchor_for_test(&input, &mutated)
                    .is_err(),
                "signed anchor mutation was accepted: {}",
                $label
            );
        }};
    }
    reject_anchor_mutation!(
        "operation ID",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.operation_id[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "replay binding",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.replay_binding.signer.policy_revision += 1;
        }
    );
    reject_anchor_mutation!(
        "replay binding digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.replay_binding_sha256[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "observation binding",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.observation_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "observation binding digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.observation_binding_sha256[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "audit predecessor",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.previous_audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "signing payload",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.signing_payload[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt operation",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.operation_id[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt request digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.request_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt payload digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.payload_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt payload length",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.payload_length += 1;
        }
    );
    reject_anchor_mutation!(
        "receipt signature",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.signature[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt sequence",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.commit_sequence += 1;
        }
    );
    reject_anchor_mutation!(
        "receipt audit head",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.commit_audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt replay status",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.replayed = !value.receipt.replayed;
        }
    );
    reject_anchor_mutation!(
        "receipt provenance",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.provenance.audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt response digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.response_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt response attestation",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.response_attestation[0] ^= 1;
        }
    );
}

#[test]
fn public_soak_binding_anchor_open_rejects_policy_substitution() {
    for mutate_input in [true, false] {
        let parent = temporary_parent();
        let state = public_soak_anchor_state_directory(parent.path());
        let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());
        drop(service);
        if mutate_input {
            let directory = state.join("public-soak-observation-binding-input-v1");
            fs::remove_file(record_path(&directory, input.operation_id))
                .expect("remove valid binding input");
            let mut substituted = input.clone();
            substituted.observation_binding.signer.policy_revision += 1;
            persist_canonical_once(&directory, input.operation_id, &substituted)
                .expect("persist substituted binding input");
        } else {
            let directory = state.join("public-soak-observation-binding-v1");
            fs::remove_file(record_path(&directory, anchor.operation_id))
                .expect("remove valid signed anchor");
            let mut substituted = anchor.clone();
            substituted.replay_binding.signer.policy_digest[0] ^= 1;
            persist_canonical_once(&directory, anchor.operation_id, &substituted)
                .expect("persist substituted signed anchor");
        }
        assert!(matches!(
            TairaAuthorityServiceV1::open(&state, wrapping_key()),
            Err(TairaAuthorityErrorV1::State)
        ));
    }
}

#[test]
fn canonical_store_recovers_pending_records_and_exact_retries() {
    let parent = temporary_parent();
    let records_directory = parent.path().join("records");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&records_directory)
        .expect("create private records directory");
    let key = [0x31; 32];
    let record = replay_record(0x31);
    assert_eq!(
        persist_canonical_once(&records_directory, key, &record),
        Ok(PersistOutcomeV1::Fresh)
    );
    assert_eq!(
        persist_canonical_once(&records_directory, key, &record),
        Ok(PersistOutcomeV1::Existing)
    );

    let final_path = record_path(&records_directory, key);
    let pending_path = pending_record_path(&records_directory, key);
    fs::rename(&final_path, &pending_path).expect("simulate crash before pending promotion");
    let recovered: BTreeMap<[u8; 32], ReplayConsumptionV1> =
        load_canonical_records(&records_directory).expect("recover pending record");
    assert_eq!(recovered, BTreeMap::from([(key, record)]));
    assert!(final_path.is_file());
    assert!(!pending_path.exists());
}

#[test]
fn canonical_store_rejects_conflicting_pending_recovery() {
    let parent = temporary_parent();
    let records_directory = parent.path().join("records");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&records_directory)
        .expect("create private records directory");
    let key = [0x41; 32];
    let accepted = replay_record(0x41);
    persist_canonical_once(&records_directory, key, &accepted).expect("persist accepted record");

    let conflicting = replay_record(0x42);
    let conflicting_bytes = norito::encode_canonical(&conflicting).expect("encode conflict");
    let mut pending = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(pending_record_path(&records_directory, key))
        .expect("create conflicting pending record");
    pending
        .write_all(&conflicting_bytes)
        .expect("write conflicting pending record");
    pending.sync_all().expect("sync conflicting pending record");
    drop(pending);

    assert!(
        load_canonical_records::<ReplayConsumptionV1>(&records_directory).is_err(),
        "recovery must not choose between two different durable values"
    );
}
