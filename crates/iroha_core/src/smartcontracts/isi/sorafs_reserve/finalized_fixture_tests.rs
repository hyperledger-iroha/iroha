// Finalized reserve fixtures bind consensus identities and exact native timestamp rejections.

#[test]
fn committed_event_query_is_finalized_cursor_bounded_and_deterministic() {
    let governance = account(&keypair(0x75));
    let provider = account(&keypair(0x76));
    let custody = account(&keypair(0x77));
    let treasury = account(&keypair(0x78));
    let mut state = state_fixture(&governance, &provider, &custody, &treasury);
    let first = policy(1, None, custody, treasury, &governance);
    let first_digest = first.digest().expect("reserve policy digest");
    transact(&mut state, 1, NOW, |transaction| {
        SetSorafsReservePolicy::new(first).execute(&governance, transaction)
    })
    .expect("commit reserve policy");
    transact(&mut state, 2, NOW + 1, |transaction| {
        RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
            .execute(&governance, transaction)?;
        RequestSorafsReserveMovement::new(
            [0x81; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(10_000_000),
            1,
            first_digest,
        )
        .execute(&provider, transaction)
    })
    .expect("commit provider registration and movement request");
    transact(&mut state, 3, NOW + 2, |transaction| {
        DecideSorafsReserveMovement::new([0x81; 32], 2, first_digest, false, "declined".to_owned())
            .execute(&governance, transaction)
    })
    .expect("commit reserve movement decision");
    let view = state.view();
    let first_page = FindSorafsReserveEvents::new(None, None, 2)
        .execute(&view)
        .expect("query first committed reserve event page");
    assert_eq!(first_page.finalized_cursor.height, 3);
    assert_eq!(first_page.events.len(), 2);
    assert!(first_page.has_more);
    assert_eq!(
        first_page
            .events
            .iter()
            .map(|event| (event.sequence, event.block_height, event.event_index))
            .collect::<Vec<_>>(),
        vec![(1, 1, 0), (2, 2, 0)]
    );
    let anchor = first_page.finalized_cursor;
    let cursor = first_page.next_after.expect("event continuation");
    let second_page = FindSorafsReserveEvents::new(Some(anchor), Some(cursor), 2)
        .execute(&view)
        .expect("query second committed reserve event page");
    assert_eq!(
        second_page
            .events
            .iter()
            .map(|event| (event.sequence, event.block_height, event.event_index))
            .collect::<Vec<_>>(),
        vec![(3, 2, 1), (4, 3, 0)]
    );
    assert!(!second_page.has_more);
    assert!(second_page.next_after.is_none());
    assert_eq!(
        first_page.events[0].event.kind,
        SorafsReserveLedgerEventKind::PolicyActivated
    );
    assert_eq!(
        second_page.events[1].event.kind,
        SorafsReserveLedgerEventKind::MovementRejected
    );
    assert_eq!(
        first_page
            .events
            .iter()
            .chain(&second_page.events)
            .map(|event| event.event.resulting_lifecycle_stage)
            .collect::<Vec<_>>(),
        vec![
            None,
            Some(ReserveLifecycleStage::Warning),
            Some(ReserveLifecycleStage::Warning),
            Some(ReserveLifecycleStage::Warning),
        ]
    );
    let expected_hashes = [
        *block_header_at(1, NOW).hash().as_ref(),
        *block_header_at(2, NOW + 1).hash().as_ref(),
        *block_header_at(3, NOW + 2).hash().as_ref(),
    ];
    assert_eq!(first_page.events[0].block_hash, expected_hashes[0]);
    assert_eq!(first_page.events[1].block_hash, expected_hashes[1]);
    assert_eq!(second_page.events[0].block_hash, expected_hashes[1]);
    assert_eq!(second_page.events[1].block_hash, expected_hashes[2]);
    assert_eq!(anchor.block_hash, expected_hashes[2]);
    let mut stale_anchor = anchor;
    stale_anchor.block_hash[0] ^= 0xFF;
    assert_eq!(
        FindSorafsReserveEvents::new(Some(stale_anchor), None, 1).execute(&view),
        Err(QueryExecutionFail::Expired)
    );
    let mut tampered_cursor = cursor;
    tampered_cursor.event_index += 1;
    assert_eq!(
        FindSorafsReserveEvents::new(Some(anchor), Some(tampered_cursor), 1).execute(&view),
        Err(QueryExecutionFail::Expired)
    );
    for invalid_limit in [0, RESERVE_QUERY_MAX_ITEMS_V1 + 1] {
        assert!(matches!(
            FindSorafsReserveEvents::new(Some(anchor), None, invalid_limit).execute(&view),
            Err(QueryExecutionFail::Conversion(_))
        ));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_provider_mutation_rejects_regressed_block_time() {
    let governance = account(&keypair(0xE1));
    let provider = account(&keypair(0xE2));
    let custody = account(&keypair(0xE3));
    let treasury = account(&keypair(0xE4));
    let mut state = state_fixture(&governance, &provider, &custody, &treasury);
    let configured = policy(1, None, custody, treasury, &governance);
    let policy_digest = configured.digest().expect("reserve policy digest");
    transact(&mut state, 1, NOW, |transaction| {
        SetSorafsReservePolicy::new(configured).execute(&governance, transaction)?;
        RegisterSorafsReserveAccount::new(terms(provider.clone()), policy_digest)
            .execute(&governance, transaction)
    })
    .expect("activate policy and register provider");
    let updated_at = NOW + 100;
    transact(&mut state, 2, updated_at, |transaction| {
        RequestSorafsReserveMovement::new(
            [0xE5; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(1_000_000),
            1,
            policy_digest,
        )
        .execute(&provider, transaction)?;
        RequestSorafsReserveMovement::new(
            [0xE6; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::Withdrawal,
            xor_micro(1),
            2,
            policy_digest,
        )
        .execute(&provider, transaction)?;
        SubmitSorafsReserveAppeal::new(
            [0xE7; 32],
            PROVIDER_ID,
            3,
            ReserveLifecycleStage::Active,
            "review provider lifecycle".to_owned(),
            Some([0xE8; 32]),
            policy_digest,
        )
        .execute(&provider, transaction)?;
        DrawSorafsReserveCredit::new(PROVIDER_ID, 4, xor_micro(1_000_000), policy_digest)
            .execute(&governance, transaction)
    })
    .expect("establish pending records and a later provider timestamp");
    let baseline_provider = read_provider(state.view().world(), PROVIDER_ID)
        .expect("read provider baseline")
        .expect("provider exists");
    assert_eq!(baseline_provider.updated_at_unix, updated_at);
    assert_eq!(baseline_provider.revision, 5);
    let baseline_top_up = read_movement(state.view().world(), [0xE5; 32])
        .expect("read pending top-up")
        .expect("top-up exists");
    let baseline_withdrawal = read_movement(state.view().world(), [0xE6; 32])
        .expect("read pending withdrawal")
        .expect("withdrawal exists");
    let baseline_appeal = read_appeal(state.view().world(), [0xE7; 32])
        .expect("read pending appeal")
        .expect("appeal exists");
    let baseline_reserve_state = read_reserve_state(state.view().world())
        .expect("read reserve state")
        .expect("reserve state exists");
    let regressed_at = updated_at - 1;
    let header = block_header_at(3, regressed_at);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let assert_regression = |result: Result<(), InstructionExecutionError>, operation: &str| {
        let error = result.expect_err(operation);
        assert_eq!(
            error,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!(
                    "reserve block timestamp {regressed_at} predates provider update {updated_at}"
                )
            )),
            "{operation} must reject the exact timestamp regression"
        );
    };
    assert_regression(
        RequestSorafsReserveMovement::new(
            [0xE9; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(1),
            5,
            policy_digest,
        )
        .execute(&provider, &mut transaction),
        "regressed top-up request",
    );
    assert_regression(
        RequestSorafsReserveMovement::new(
            [0xEA; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::Withdrawal,
            xor_micro(1),
            5,
            policy_digest,
        )
        .execute(&provider, &mut transaction),
        "regressed withdrawal request",
    );
    assert_regression(
        DecideSorafsReserveMovement::new(
            [0xE5; 32],
            5,
            policy_digest,
            true,
            "approve top-up".to_owned(),
        )
        .execute(&governance, &mut transaction),
        "regressed top-up decision",
    );
    assert_regression(
        DecideSorafsReserveMovement::new(
            [0xE6; 32],
            5,
            policy_digest,
            false,
            "reject withdrawal".to_owned(),
        )
        .execute(&governance, &mut transaction),
        "regressed withdrawal decision",
    );
    assert_regression(
        DrawSorafsReserveCredit::new(PROVIDER_ID, 5, xor_micro(1), policy_digest)
            .execute(&governance, &mut transaction),
        "regressed credit draw",
    );
    assert_regression(
        RepaySorafsReserveCredit::new(PROVIDER_ID, 5, xor_micro(1), policy_digest)
            .execute(&provider, &mut transaction),
        "regressed credit repayment",
    );
    assert_regression(
        SubmitSorafsReserveAppeal::new(
            [0xEB; 32],
            PROVIDER_ID,
            5,
            ReserveLifecycleStage::Warning,
            "review timestamp regression".to_owned(),
            Some([0xEC; 32]),
            policy_digest,
        )
        .execute(&provider, &mut transaction),
        "regressed appeal submission",
    );
    assert_regression(
        DecideSorafsReserveAppeal::new(
            [0xE7; 32],
            5,
            policy_digest,
            true,
            "accept appeal".to_owned(),
        )
        .execute(&governance, &mut transaction),
        "regressed appeal decision",
    );
    assert_eq!(
        read_provider(transaction.world(), PROVIDER_ID)
            .expect("read provider after rejected mutations")
            .expect("provider remains"),
        baseline_provider
    );
    assert_eq!(
        read_movement(transaction.world(), [0xE5; 32])
            .expect("read top-up after rejected mutations")
            .expect("top-up remains"),
        baseline_top_up
    );
    assert_eq!(
        read_movement(transaction.world(), [0xE6; 32])
            .expect("read withdrawal after rejected mutations")
            .expect("withdrawal remains"),
        baseline_withdrawal
    );
    assert_eq!(
        read_appeal(transaction.world(), [0xE7; 32])
            .expect("read appeal after rejected mutations")
            .expect("appeal remains"),
        baseline_appeal
    );
    assert!(
        read_movement(transaction.world(), [0xE9; 32])
            .expect("read rejected top-up request")
            .is_none()
    );
    assert!(
        read_movement(transaction.world(), [0xEA; 32])
            .expect("read rejected withdrawal request")
            .is_none()
    );
    assert!(
        read_appeal(transaction.world(), [0xEB; 32])
            .expect("read rejected appeal")
            .is_none()
    );
    assert_eq!(
        read_reserve_state(transaction.world())
            .expect("read reserve state after rejected mutations")
            .expect("reserve state remains"),
        baseline_reserve_state
    );
}
