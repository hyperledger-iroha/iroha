// Real WAL/ledger/body-store crash cuts around installed timeout retirement.
#[test]
fn real_cold_owner_cancels_timeout_superseded_body_before_replay() {
    run_durable_recovery_test_on_stack(|| real_timeout_body_recovery_fixture(0));
}

#[test]
fn real_cold_owner_preserves_current_body_after_timeout_recovery() {
    run_durable_recovery_test_on_stack(|| real_timeout_body_recovery_fixture(1));
}

#[test]
fn real_cold_owner_rejects_future_body_generation_without_retirement() {
    run_durable_recovery_test_on_stack(|| real_timeout_body_recovery_fixture(2));
}

fn real_timeout_body_recovery_fixture(case: u8) {
    use crate::sumeragi::v2::{
        AdapterEffect, AdapterFingerprints, DeferredAdmissionOrdinalSource, SumeragiV2Adapter,
    };

    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    let fixture = RecoveryFixture::new("timeout-body-retirement", 0x6a);
    let context = fixture.verified.context();
    let body_directory = TempDir::new().expect("timeout body directory");
    let mut body_store = fixture.open_store(&body_directory);
    let (view, generation) = match case {
        0 => (0, 0x6a),
        1 => (1, 0),
        2 => (1, 1),
        _ => unreachable!(),
    };
    let record = standalone_validate_record(
        &fixture,
        &mut body_store,
        view,
        generation,
        9,
        StandaloneValidateOriginFixture::LocalBody,
    );
    let expected_record = record.clone();
    let ledger_directory = TempDir::new().expect("timeout ledger directory");
    let ledger = fixture.ledger(vec![record]);
    drop(fixture.persist_ledger(&ledger_directory, &ledger));
    drop(body_store);
    let safety_directory = TempDir::new().expect("timeout safety directory");
    let wal_path = safety_directory.path().join("timeout.wal");
    let open_adapter = || {
        SumeragiV2Adapter::open(
            &wal_path,
            fixture.verified.clone(),
            Some(context.leader(1)),
            Generation::new(0x6a),
            [0x6a; 32],
            AdapterFingerprints {
                node: Hash::new(b"timeout recovery node"),
                build: Hash::new(b"timeout recovery build"),
                config: Hash::new(b"timeout recovery config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open timeout adapter")
    };
    let (mut warm, effects) = open_adapter();
    assert!(effects.is_empty());
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = fixture.keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let certificate = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate timeout quorum"),
        }],
    };
    let authenticated = warm
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ))
        .expect("authenticate timeout quorum");
    warm.receive_authenticated(authenticated)
        .expect("persist timeout before body cancellation");
    assert_eq!(
        warm.current_tag(),
        EventTag::new(context.height, 1, Generation::INITIAL)
    );
    if case == 0 {
        let frontier = warm
            .leader_wire_recovery_authority()
            .expect("actual durable timeout frontier");
        let terminal_directory = TempDir::new().expect("isolated terminal body directory");
        let mut terminal_store = fixture.open_store(&terminal_directory);
        let terminal = fixture.terminal_validate_record(&mut terminal_store, 0, 0x70, 1);
        assert!(terminal.authenticates_retired_terminal_validate_source(
            &fixture.verified,
            frontier,
            &terminal_store,
        ));
        let empty_directory = TempDir::new().expect("missing terminal body directory");
        let empty_store = fixture.open_store(&empty_directory);
        assert!(!terminal.authenticates_retired_terminal_validate_source(
            &fixture.verified,
            frontier,
            &empty_store,
        ));
        let foreign = fixture.terminal_validate_record(&mut terminal_store, 0, 0x71, 2);
        for mutation in 0..5 {
            let mut changed = terminal.clone();
            match mutation {
                0 => changed = changed.with_work_class_for_test(LifecycleWorkClass::Fetch),
                1 => changed = changed.with_terminal_for_test(None),
                2 => {
                    changed.continuation =
                        PersistedDurableContinuationV1::from_schema(DurableContinuation::None);
                }
                3 => changed.stage_kind_code = u16::MAX,
                4 => changed.payload_reference = foreign.payload_reference.clone(),
                _ => unreachable!(),
            }
            assert!(
                !changed.authenticates_retired_terminal_validate_source(
                    &fixture.verified,
                    frontier,
                    &terminal_store,
                ),
                "retired terminal row mutation {mutation} must fail closed",
            );
        }
    }
    drop(warm);
    let wal_before = fs::read(&wal_path).expect("read installed timeout WAL");
    let body_store = fixture.open_store(&body_directory);
    let bodies_before = body_store
        .recovery_catalog()
        .expect("retained body catalog");
    let (ledger_store, ledger) =
        LifecycleLedgerStoreV1::open(ledger_directory.path(), fixture.lifecycle_context())
            .expect("reopen pre-cancellation ledger");
    let retained_store = ledger_store.clone();
    let payload_directory = TempDir::new().expect("timeout Serve directory");
    let (payload_store, payloads) =
        fixture.open_empty_serve_payloads(&payload_directory, &body_store);
    let cut = ledger
        .into_durable_certified_body_pipeline_storage_recovery_cut(
            fixture.verified.clone(),
            ledger_store,
            body_store,
        )
        .expect("authenticate original body census before cancellation");
    let (cold, effects) = open_adapter();
    assert!(effects.is_empty());
    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 0)
        .expect("body owner capacity");
    let result = cut.open_owner_with_authority(
        authority,
        payload_store,
        payloads,
        ProductionLifecycleAdapterStartupV1::recovered_for_test(cold, effects),
    );
    assert_eq!(
        fs::read(&wal_path).expect("unchanged timeout WAL"),
        wal_before
    );
    if case == 2 {
        assert!(
            result.is_err(),
            "future execution is never retired or retagged"
        );
        assert_eq!(retained_store.load().unwrap().records(), &[expected_record]);
        return;
    }
    let mut owner = result.expect("real cold timeout recovery opens");
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    let observed = retained_store
        .load()
        .expect("published timeout recovery ledger");
    let row = observed
        .records()
        .iter()
        .find(|row| row.ordinal() == 9)
        .unwrap();
    assert_eq!(row.owner(), expected_record.owner());
    assert_eq!(row.replay_authority, expected_record.replay_authority);
    assert_eq!(row.durable_payload(), expected_record.durable_payload());
    if case == 0 {
        assert_eq!(row.terminal(), Some(Some(TerminalOutcome::Cancelled)));
        assert_eq!(owner.live_body_pipeline_counts_for_test(), (0, 0, 0));
        assert!(
            owner.has_owner_open_successor_for_test(),
            "cancellation remains in the real CAS chain"
        );
    } else {
        assert_eq!(row.terminal(), Some(None));
        assert_eq!(owner.live_body_pipeline_counts_for_test(), (0, 0, 1));
        assert!(matches!(
            owner.plan_direct_registry_turn(),
            Err(ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal: 9 })
        ));
    }
    let (mut cold, effects) = owner
        .adapter_startup
        .take()
        .unwrap()
        .into_adapter_for_test();
    assert!(effects.is_empty());
    let effects = cold
        .retransmit_elapsed(cold.current_tag())
        .unwrap()
        .into_effects();
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, AdapterEffect::ValidateBody { .. }))
            .count(),
        0,
        "LocalBody Validate is owned by the recovered registry; custody alone cannot create a Proposal or QC retry"
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::FetchBody { .. }))
    );
    assert_eq!(
        owner
            .body_store
            .as_ref()
            .unwrap()
            .recovery_catalog()
            .unwrap(),
        bodies_before,
        "retirement preserves authenticated body bytes and receipts"
    );
}
