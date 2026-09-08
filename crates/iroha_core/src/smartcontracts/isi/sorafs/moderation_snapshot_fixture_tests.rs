// These storage/WSV fixtures execute instructions directly in the overlay and persist
// signed, result-bearing empty blocks for ledger-time and DA-index replay queries.
// They do not stand in for consensus certificates or payload-availability evidence.
fn panel_fixture_with_kura() -> PanelFixture {
    let fixture = PanelFixture::new();
    let foundation = iroha_data_model::block::builder::BlockBuilder::new(header(1, 1_000_000))
        .try_build_with_signature(0, fixture.manager.private_key())
        .expect("sign moderation foundation block with execution results");
    assert_eq!(
        fixture.state.view().latest_block_hash(),
        Some(foundation.hash()),
        "persist the exact foundation header already executed by the panel fixture"
    );
    let foundation_header = foundation.header();
    fixture
        .state
        .kura()
        .store_block(foundation)
        .expect("persist moderation foundation block");
    cache_moderation_fixture_header(&fixture.state, foundation_header);
    fixture
}

fn run_panel_kura_block(
    fixture: &mut PanelFixture,
    now: u64,
    operation: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<(), InstructionExecutionError>,
) -> Result<(), InstructionExecutionError> {
    let height = fixture.next_height;
    let chained_header = BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero moderation fixture height"),
        fixture.state.view().latest_block_hash(),
        None,
        None,
        now,
        0,
    );
    let signed_block = iroha_data_model::block::builder::BlockBuilder::new(chained_header)
        .try_build_with_signature(0, fixture.manager.private_key())
        .expect("sign chained moderation block with execution results");
    let mut block = fixture.state.block(signed_block.header());
    let mut transaction = block.transaction();
    transaction.tx_call_hash = Some(iroha_crypto::Hash::new(
        [height.to_le_bytes(), now.to_le_bytes()].concat(),
    ));
    operation(&mut transaction)?;
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit Kura-backed moderation overlay");
    let committed_header = signed_block.header();
    assert_eq!(committed_header.hash(), signed_block.hash());
    fixture
        .state
        .kura()
        .store_block(signed_block)
        .expect("persist exact executed moderation header and body");
    retain_moderation_fixture_header(&mut fixture.state, committed_header);
    fixture.next_height += 1;
    Ok(())
}

#[test]
fn snapshot_time_requires_the_exact_committed_header_cache_even_when_kura_is_present() {
    let fixture = panel_fixture_with_kura();
    let committed = fixture
        .state
        .view()
        .latest_block()
        .expect("signed committed foundation")
        .header();
    let expected = FindSorafsModerationSnapshot::new(8, 16)
        .execute(&fixture.state.view())
        .expect("snapshot has an exact authenticated header cache");
    assert_eq!(expected.finalized_height, committed.height().get());
    assert_eq!(expected.finalized_at_unix_ms, committed.creation_time_ms);
    let wrong = header(committed.height().get(), committed.creation_time_ms + 1);
    assert_ne!(wrong.hash(), committed.hash());
    fixture
        .state
        .update_latest_block_header_cache_for_tests(wrong);
    assert_eq!(
        fixture.state.view().latest_block().unwrap().hash(),
        committed.hash(),
        "the exact Kura block remains available during cache substitution"
    );
    assert_eq!(
        FindSorafsModerationSnapshot::new(8, 16).execute(&fixture.state.view()),
        Err(QueryExecutionFail::Conversion(
            "finalized moderation snapshot state anchor has no ledger time".to_owned()
        ))
    );
    cache_moderation_fixture_header(&fixture.state, committed);
    assert_eq!(
        FindSorafsModerationSnapshot::new(8, 16)
            .execute(&fixture.state.view())
            .expect("recover the exact committed header cache"),
        expected
    );
}

#[test]
fn snapshot_rebuilds_complete_chain_projection_in_logical_order() {
    let mut fixture = panel_fixture_with_kura();
    let appellant = fixture.appellant_id();
    let z_intake = panel_intake(&fixture.appellant, "z-case", 1, 0, 1, 0x91);
    run_panel_kura_block(&mut fixture, 1_001_000, |transaction| {
        SubmitSorafsModerationAppeal::new(z_intake).execute(&appellant, transaction)
    })
    .expect("submit z appeal");
    let mut a_intake = panel_intake(&fixture.appellant, "a-case", 1, 0, 1, 0x92);
    a_intake.proof_token_digest = [0x35; 32];
    run_panel_kura_block(&mut fixture, 1_001_001, |transaction| {
        SubmitSorafsModerationAppeal::new(a_intake).execute(&appellant, transaction)
    })
    .expect("submit a appeal");
    let view = fixture.state.view();
    let snapshot = FindSorafsModerationSnapshot::new(8, 16)
        .execute(&view)
        .expect("rebuild complete finalized moderation snapshot");
    assert_eq!(snapshot.finalized_height, 3);
    assert_eq!(
        snapshot.finalized_at_unix_ms,
        view.latest_block()
            .expect("exact finalized block")
            .header()
            .creation_time_ms
    );
    assert_eq!(
        snapshot
            .appeals
            .iter()
            .map(|appeal| appeal.appeal.intake.case_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a-case", "z-case"]
    );
    assert!(snapshot.cases.is_empty());
    assert_eq!(snapshot.events.len(), 3);
    assert_eq!(
        snapshot
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(
        FindSorafsModerationSnapshot::new(1, 16)
            .execute(&view)
            .is_err(),
        "a complete snapshot must fail instead of truncating cases"
    );
    let appeal = snapshot.appeals[0].appeal.clone();
    drop(view);
    fixture.state.world.smart_contract_state.insert(
        digest_key(APPEAL_STATE_KEY_PREFIX, [0xEE; 32]),
        encode_state(&appeal, "corrupt duplicate appeal").expect("encode corrupt fixture"),
    );
    assert!(
        FindSorafsModerationSnapshot::new(8, 16)
            .execute(&fixture.state.view())
            .is_err(),
        "a mismatched persisted key must fail the complete projection"
    );
}
#[test]
fn snapshot_includes_all_eligibility_and_latest_typed_events() {
    let mut fixture = PanelFixture::new();
    fixture.submit(1, 0, 1);
    fixture.register_juror();
    fixture.finalize_single_juror_sortition();
    let anchor = fixture
        .appeal()
        .sortition_anchor
        .expect("retained sortition anchor");
    let carrier = fixture
        .state
        .latest_block_header_fast()
        .expect("finalized sortition carrier");
    assert_eq!(anchor.block_height, 4);
    assert_eq!(carrier.height().get(), 5);
    assert!(carrier.height().get() > anchor.block_height);
    let snapshot = FindSorafsModerationSnapshot::new(8, 16)
        .execute(&fixture.state.view())
        .expect("rebuild eligibility-bearing moderation snapshot");
    assert_eq!(snapshot.appeals.len(), 1);
    assert_eq!(snapshot.appeals[0].eligibility.len(), 1);
    assert_eq!(snapshot.appeals[0].eligibility[0].juror, fixture.juror_id());
    assert_eq!(snapshot.events.len(), 4);
    assert_eq!(
        snapshot.events.last().map(|event| event.event.kind),
        Some(SorafsModerationLedgerEventKind::SortitionFinalized)
    );
    assert_eq!(snapshot.finalized_height, carrier.height().get());
    assert_eq!(snapshot.finalized_at_unix_ms, carrier.creation_time_ms);
    assert_eq!(
        snapshot
            .events
            .last()
            .map(|event| (event.block_height, event.block_hash)),
        Some((carrier.height().get(), *carrier.hash().as_ref()))
    );
}
