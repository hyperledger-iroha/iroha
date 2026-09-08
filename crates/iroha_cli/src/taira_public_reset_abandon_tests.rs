// Included in executor_model::tests; uses its real durable-journal fixtures.

#[test]
fn abandonment_admits_original_signed_revision_without_relaxing_current_dispatcher_identity() {
    // Release 83 is an actual retained target. The current controller must use its own
    // compiled provenance, while the original dispatcher keeps this exact target revision.
    const ORIGINAL_COMMIT: &str = "57cafebd445a3635a09929ad61a9ecf54c2751cc";
    let compiled = crate::compiled_build_identity()
        .unwrap()
        .release_source_commit()
        .unwrap();
    assert_ne!(
        compiled, ORIGINAL_COMMIT,
        "regression requires a successor controller"
    );
    let fixture = sample_inventory();
    let bytes = String::from_utf8(canonical_inventory_bytes(&fixture).unwrap()).unwrap();
    let (mut inventory, _chain_guard) = decode_inventory(
        bytes.replace(compiled, ORIGINAL_COMMIT).as_bytes(),
        "retained target fixture",
    )
    .unwrap();
    let host_key =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHRhaXJhLWZpeHR1cmUtaG9zdC1rZXktMDAwMDAwMDAw";
    let mut known_hosts = String::new();
    for endpoint in inventory
        .validators
        .iter_mut()
        .map(|validator| &mut validator.endpoint)
        .chain(std::iter::once(&mut inventory.edge.endpoint))
    {
        let line = format!("{} {host_key}", endpoint.hostname);
        endpoint.known_host_line_sha256 = sha256_hex(line.as_bytes());
        endpoint.host_identity_sha256 = sha256_hex(host_key.as_bytes());
        known_hosts.push_str(&line);
        known_hosts.push('\n');
    }
    inventory.artifact_closure_sha256 = artifact_closure_sha256(&inventory);
    validate_inventory_for_controller(&inventory, ControllerAdmission::AbandonOriginalTarget)
        .expect("original target keeps the full first-release structural contract");
    assert!(
        validate_inventory(&inventory)
            .unwrap_err()
            .to_string()
            .contains("compiled CLI SHA"),
        "normal Apply and the retained host dispatcher must keep their executable identity check"
    );

    let inventory_bytes = canonical_inventory_bytes(&inventory).unwrap();
    let key = KeyPair::try_from_seed(vec![0x83; 32], Algorithm::Ed25519).unwrap();
    let claims = sample_claims(&inventory, &sha256_hex(&inventory_bytes));
    let signature =
        Signature::try_new(key.private_key(), &authorization_message(&claims).unwrap()).unwrap();
    let mut authorization = AuthorizationEnvelopeV1 {
        schema: AUTHORIZATION_SCHEMA_V1.to_owned(),
        claims,
        signature_hex: hex::encode(signature.payload()),
    };
    let trusted = TrustedKeyV1 {
        schema: TRUSTED_KEY_SCHEMA_V1.to_owned(),
        algorithm: "ed25519".to_owned(),
        public_key: key.public_key().to_string(),
    };
    let directory = private_tempdir();
    let paths = [
        "inventory.json",
        "authorization.json",
        "trusted.json",
        "identity",
        "known-hosts",
    ]
    .map(|name| directory.path().join(name));
    let contents = [
        inventory_bytes.clone(),
        json::to_vec(&authorization).unwrap(),
        json::to_vec(&trusted).unwrap(),
        b"fixture-only SSH custody; never dispatched\n".to_vec(),
        known_hosts.into_bytes(),
    ];
    for (path, bytes) in paths.iter().zip(contents) {
        let mut file = create_private_new(path).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
    }
    let (admitted, _guard) = admit_signed_inputs_for_controller(
        &paths[0],
        &paths[1],
        &paths[2],
        &paths[3],
        &paths[4],
        ControllerAdmission::AbandonOriginalTarget,
    )
    .expect("real successor admission authenticates the original inventory and signing authority");
    assert_eq!(admitted.inventory_bytes, inventory_bytes);
    assert_eq!(admitted.inventory.revision.commit, ORIGINAL_COMMIT);
    assert!(
        admitted
            .inventory
            .validators
            .iter()
            .all(|validator| validator.endpoint.remote_cli.contains(ORIGINAL_COMMIT))
    );
    assert!(
        admitted.pinned_artifacts.is_empty(),
        "rollback must not admit successor artifacts"
    );
    assert!(admit_signed_inputs(&paths[0], &paths[1], &paths[2], &paths[3], &paths[4]).is_err());

    authorization.signature_hex = "00".repeat(64);
    fs::write(&paths[1], json::to_vec(&authorization).unwrap()).unwrap();
    assert!(
        admit_signed_inputs_for_controller(
            &paths[0],
            &paths[1],
            &paths[2],
            &paths[3],
            &paths[4],
            ControllerAdmission::AbandonOriginalTarget,
        )
        .is_err(),
        "historical target admission still verifies the original signature"
    );
    let mut wrong_target = inventory.clone();
    wrong_target.revision.target = "aarch64-apple-darwin".to_owned();
    assert!(
        validate_inventory_for_controller(
            &wrong_target,
            ControllerAdmission::AbandonOriginalTarget
        )
        .is_err()
    );
    let mut wrong_dispatcher = inventory;
    wrong_dispatcher.validators[0].endpoint.remote_cli =
        format!("/srv/taira/taira-validator-1/releases/{compiled}/bin/iroha");
    assert!(
        validate_inventory_for_controller(
            &wrong_dispatcher,
            ControllerAdmission::AbandonOriginalTarget
        )
        .is_err(),
        "the controller cannot substitute itself for the retained original dispatcher"
    );
}

fn pending_abandonment_fixture() -> (tempfile::TempDir, AdmittedReset, DurableJournal, Vec<u8>) {
    let directory = private_tempdir();
    let admitted = admitted(sample_inventory());
    let mut journal = DurableJournal::open(directory.path(), &admitted).expect("open journal");
    let mut state = journal.state.clone();
    state.status = "recovery_pending".to_owned();
    state.phase = ExecutionStep::Canary.label().to_owned();
    state.next_step = u16::try_from(
        EXECUTION_STEPS
            .iter()
            .position(|step| *step == ExecutionStep::Canary)
            .unwrap(),
    )
    .unwrap();
    state.touched_validators = VALIDATOR_SLUGS
        .iter()
        .map(|slug| (*slug).to_owned())
        .collect();
    let mut intent = test_recovery_intent(ExecutionStep::Canary);
    intent.mutations[0].state = RecoveryMutationStateV1::Submitted;
    state.recovery_intent = Some(intent);
    validate_resumable_journal(&state, &journal.state).expect("valid pending fixture");
    journal.replace(state).expect("publish pending fixture");
    let bytes = fs::read(&journal.current_path).expect("exact original journal bytes");
    (directory, admitted, journal, bytes)
}

#[test]
fn abandonment_cli_requires_explicit_flag_digest_and_original_authority() {
    use clap::Parser;
    #[derive(clap::Parser)]
    struct Parser {
        #[command(flatten)]
        reset: PublicReset,
    }
    let digest = "ab".repeat(32);
    let mut arguments = vec![
        "public-reset",
        "abandon",
        "--inventory",
        "/inventory",
        "--authorization",
        "/authorization",
        "--trusted-public-key",
        "/trust",
        "--ssh-identity",
        "/identity",
        "--known-hosts",
        "/hosts",
        "--expected-journal-sha256",
        &digest,
    ];
    assert!(
        Parser::try_parse_from(&arguments).is_err(),
        "destructive intent must be explicit"
    );
    arguments.push("--abandon-pending-mutations");
    let parsed = Parser::try_parse_from(&arguments).expect("exact abandonment arguments");
    assert!(matches!(
        parsed.reset.command,
        PublicResetCommand::Abandon(PublicResetAbandon {
            abandon_pending_mutations: true,
            ..
        })
    ));
    for option in [
        "--expected-journal-sha256",
        "--authorization",
        "--trusted-public-key",
        "--inventory",
        "--ssh-identity",
        "--known-hosts",
    ] {
        let mut missing = arguments.clone();
        let index = missing
            .iter()
            .position(|argument| *argument == option)
            .unwrap();
        missing.drain(index..index + 2);
        assert!(
            Parser::try_parse_from(missing).is_err(),
            "missing {option} must fail"
        );
    }
    arguments.push("--runtime-client-config");
    arguments.push("/unused");
    assert!(
        Parser::try_parse_from(arguments).is_err(),
        "abandonment admits no ledger signing inputs"
    );
}

#[test]
fn abandonment_preserves_exact_unresolved_evidence_before_rollback_and_after_crash() {
    let (directory, admitted, journal, original_bytes) = pending_abandonment_fixture();
    let digest = sha256_hex(&original_bytes);
    let original_state = journal.state.clone();
    let receipt_path = journal
        .preserve_abandonment(&digest)
        .expect("durable snapshot before transition");
    assert_eq!(journal.state, original_state);
    assert_eq!(fs::read(&journal.current_path).unwrap(), original_bytes);
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let receipt = read_private_json::<AbandonmentReceiptV1>(&receipt_path, "test receipt")
        .unwrap()
        .0;
    assert_eq!(receipt.journal_json.as_bytes(), original_bytes);
    assert_eq!(receipt.transaction_outcome, "unresolved");
    let original: JournalV1 = json::from_str(&receipt.journal_json).unwrap();
    assert_eq!(
        original.recovery_intent.as_ref().unwrap().mutations[0].state,
        RecoveryMutationStateV1::Submitted
    );
    drop(journal); // Crash after the immutable receipt, before any transition or host action.
    let mut resumed = DurableJournal::open(directory.path(), &admitted).unwrap();
    let mut transport = MockTransport::default();
    assert_eq!(
        abandon_pending_attempt(&admitted.inventory, &mut transport, &mut resumed, &digest)
            .unwrap(),
        receipt_path
    );
    assert_eq!(
        transport.events,
        [
            "rollback:taira-validator-4",
            "rollback:taira-validator-3",
            "rollback:taira-validator-2",
            "rollback:taira-validator-1"
        ]
    );
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert_eq!(resumed.state.status, "rolled_back");
    assert!(!resumed.current_path.exists());
    assert!(resumed.rollback_receipt_path.exists());
    drop(resumed);
    assert!(
        DurableJournal::classify(directory.path(), &admitted).is_err(),
        "terminal authorization cannot be replayed"
    );
}

#[test]
fn abandonment_rejects_wrong_digest_edge_and_proven_state_without_host_actions() {
    for case in [
        "digest",
        "edge",
        "proven",
        "sealed",
        "resolved",
        "foreign_receipt",
    ] {
        let (_directory, admitted, mut journal, original_bytes) = pending_abandonment_fixture();
        let mut digest = sha256_hex(&original_bytes);
        match case {
            "digest" => digest = "00".repeat(32),
            "edge" => {
                let mut changed = journal.state.clone();
                changed.edge_touched = true;
                journal.replace(changed).unwrap();
            }
            "proven" => {
                publish_json_no_replace(&journal.deployment_receipt_path, &journal.state).unwrap()
            }
            "sealed" => {
                let mut changed = journal.state.clone();
                changed.status = "sealing".to_owned();
                changed.phase = "seal".to_owned();
                journal.replace(changed).unwrap();
            }
            "resolved" => {
                let mut changed = journal.state.clone();
                let intent = changed.recovery_intent.as_mut().unwrap();
                for mutation in &mut intent.mutations {
                    mutation.state = RecoveryMutationStateV1::Applied;
                }
                intent.next_mutation = u16::try_from(intent.mutations.len()).unwrap();
                journal.replace(changed).unwrap();
            }
            "foreign_receipt" => {
                let receipt_path = journal.preserve_abandonment(&digest).unwrap();
                let mut receipt =
                    read_private_json::<AbandonmentReceiptV1>(&receipt_path, "test receipt")
                        .unwrap()
                        .0;
                receipt.transaction_outcome = "rejected".to_owned();
                publish_json(receipt_path.parent().unwrap(), &receipt_path, &receipt).unwrap();
            }
            _ => unreachable!(),
        }
        // For state guards, supply the actual changed digest so hashing cannot mask the check.
        if matches!(case, "edge" | "sealed" | "resolved") {
            digest = sha256_hex(&fs::read(&journal.current_path).unwrap());
        }
        let before = fs::read(&journal.current_path).unwrap();
        let mut transport = MockTransport::default();
        assert!(
            abandon_pending_attempt(&admitted.inventory, &mut transport, &mut journal, &digest)
                .is_err(),
            "must reject {case}"
        );
        assert!(transport.events.is_empty());
        assert_eq!(fs::read(&journal.current_path).unwrap(), before);
    }
}

#[test]
fn abandonment_partial_rollback_resumes_only_remaining_hosts_with_original_digest() {
    let (directory, admitted, mut journal, original_bytes) = pending_abandonment_fixture();
    let digest = sha256_hex(&original_bytes);
    let mut failing = MockTransport {
        fail: Some("rollback:taira-validator-3".to_owned()),
        ..MockTransport::default()
    };
    assert!(
        abandon_pending_attempt(&admitted.inventory, &mut failing, &mut journal, &digest).is_err()
    );
    assert_eq!(
        failing.events,
        ["rollback:taira-validator-4", "rollback:taira-validator-3"]
    );
    assert_eq!(journal.state.status, "rolling_back");
    assert_eq!(journal.state.rollback_next_validator, 1);
    assert_eq!(journal.state.rollback_failures.len(), 1);
    let receipt_path = journal
        .directory
        .join("abandoned")
        .join(format!("{}.json", journal.state.authorization_sha256));
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let successor_digest = sha256_hex(&fs::read(&journal.current_path).unwrap());
    drop(journal);
    let mut resumed = DurableJournal::open(directory.path(), &admitted).unwrap();
    let mut transport = MockTransport::default();
    assert!(
        abandon_pending_attempt(
            &admitted.inventory,
            &mut transport,
            &mut resumed,
            &successor_digest
        )
        .is_err(),
        "retry still requires the original unresolved journal digest"
    );
    assert!(transport.events.is_empty());
    abandon_pending_attempt(&admitted.inventory, &mut transport, &mut resumed, &digest).unwrap();
    assert_eq!(
        transport.events,
        [
            "rollback:taira-validator-3",
            "rollback:taira-validator-2",
            "rollback:taira-validator-1"
        ]
    );
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert_eq!(resumed.state.status, "rolled_back");
}
