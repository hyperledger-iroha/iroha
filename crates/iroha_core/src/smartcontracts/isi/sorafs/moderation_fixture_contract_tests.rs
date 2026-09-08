// Direct overlay fixtures retain the same consensus header in the journal and immutable query cache.

fn cache_moderation_fixture_header(state: &State, header: BlockHeader) {
    assert_eq!(state.view().latest_block_hash(), Some(header.hash()));
    let now = header.creation_time_ms;
    state.update_latest_block_header_cache_for_tests(header);
    assert_eq!(
        crate::state::StateReadOnly::authenticated_query_ledger_time_ms(&state.view()),
        Some(now)
    );
}
fn retain_moderation_fixture_header(state: &mut State, header: BlockHeader) {
    state.push_block_hash_for_testing(header.hash());
    cache_moderation_fixture_header(state, header);
}

#[test]
fn moderation_payload_decoder_rejects_alternate_norito_layout() {
    let juror = account(&keypair(0xA1));
    let case = spec(vec![juror.clone()], 1);
    let reveal = reveal(&case, &juror, SoraFsModerationVoteChoice::Uphold, 0xA2);
    let commit = commit(&reveal);
    let canonical = encode_payload(&commit, "moderation commit").expect("encode canonical commit");
    let alternate = encode_alternate_layout(&commit);
    assert_ne!(
        alternate, canonical,
        "fixture must exercise a distinct advertised Norito layout"
    );
    decode_from_bytes_with_limits::<SoraFsModerationBallotCommitV1>(&alternate, PAYLOAD_LIMITS)
        .expect("ordinary bounded Norito accepts the advertised alternate layout");
    let error = decode_payload::<SoraFsModerationBallotCommitV1>(&alternate, "moderation commit")
        .err()
        .expect("alternate-layout moderation payload must fail");
    assert_eq!(
        parameter_error_message(&error),
        "moderation commit payload is not exact canonical Norito"
    );
}

#[test]
fn insufficient_challenge_bond_rejects_without_balances_records_or_counters() {
    let mut fixture = Fixture::new(1);
    let challenger = account(&fixture.outsider);
    let manager = fixture.manager_id();
    let challenger_asset = AssetId::new(
        fixture.state.gov.voting_asset_id.clone(),
        challenger.clone(),
    );
    fixture
        .run(1_500, |transaction| {
            Transfer::asset_quantity(challenger_asset, 851_u32, manager)
                .execute(&challenger, transaction)
        })
        .expect("reduce the challenger balance to one unit below the fixed bond");
    assert_eq!(
        voting_asset_balance(&fixture.state, &challenger),
        Quantity::from(149_u32)
    );
    let case_before = FindSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
        .execute(&fixture.state.view())
        .expect("fixture case");
    let status_before = FindSorafsModerationStatus
        .execute(&fixture.state.view())
        .expect("fixture status");
    let error = fixture
        .run(2_500, |transaction| {
            RaiseSorafsModerationChallenge::new(
                "case-1".to_owned(),
                "round-1".to_owned(),
                "challenge-underfunded".to_owned(),
                ModerationChallengeKindV1::EvidenceMismatch,
                None,
                [0x63; 32],
                "bond is one unit short".to_owned(),
            )
            .execute(&challenger, transaction)
        })
        .expect_err("a 149-unit balance cannot fund the fixed 150-unit bond");
    assert_eq!(
        error,
        InstructionExecutionError::Math(iroha_data_model::isi::error::MathError::NotEnoughQuantity)
    );
    assert_eq!(
        FindSorafsModerationCase::new("case-1".to_owned(), "round-1".to_owned())
            .execute(&fixture.state.view())
            .expect("fixture case after rejection"),
        case_before
    );
    assert_eq!(
        FindSorafsModerationStatus
            .execute(&fixture.state.view())
            .expect("fixture status after rejection"),
        status_before
    );
    assert!(
        FindSorafsModerationChallenge::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
            "challenge-underfunded".to_owned(),
        )
        .execute(&fixture.state.view())
        .is_err()
    );
    let current_policy = policy();
    assert_eq!(
        voting_asset_balance(&fixture.state, &challenger),
        Quantity::from(149_u32)
    );
    assert_eq!(
        voting_asset_balance(&fixture.state, &current_policy.challenge_escrow_account),
        Quantity::zero()
    );
    assert_eq!(
        voting_asset_balance(
            &fixture.state,
            &current_policy.challenge_slash_receiver_account,
        ),
        Quantity::zero()
    );
}
