// Actual ordinary admission checks against native certified execution history.

#[test]
fn native_validation_rejects_duplicate_original_transactions() {
    let fixture = NativeValidationFixture::new();
    let tx = fixture.transaction(2_001, None);
    let proposal = fixture.proposal(vec![tx.clone(), tx], fixture.cadence());
    let (_, error) = fixture.validate(proposal).unpack(|_| {}).err().unwrap();
    assert!(matches!(
        *error,
        BlockValidationError::DuplicateTransactions
    ));
    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_replayed_sealed_signed_identity_from_actual_history() {
    let mut fixture = NativeValidationFixture::new();
    let authority = AccountId::new(fixture.user.public_key().clone());
    let (_, reveal) = crate::block::tests::sealed_set_key_entrypoints(
        fixture.chain.network_id(),
        &authority,
        &fixture.user,
        1,
        9,
        Name::from_str("sealed_replay_native").unwrap(),
    );
    let TransactionEntrypoint::SealedReveal(sealed) = &reveal else {
        unreachable!()
    };
    // The signed identity is in real committed history. The different sealed carrier has
    // never committed, so checking only its outer hash would miss the replay.
    assert_eq!(
        fixture
            .chain
            .commit(vec![sealed.signed_transaction().clone()]),
        vec![true]
    );
    let proposal = fixture.proposal_from_inputs(
        vec![AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            reveal,
        ))],
        fixture.cadence(),
    );
    let prepared = ValidBlock::prepare_external_transactions(&proposal);
    {
        let view = fixture.chain.state().view();
        let transactions = crate::state::StateReadOnlyWithTransactions::transactions(&view);
        let signed_heights =
            ValidBlock::committed_heights_for_prepared_transactions(&prepared, transactions);
        let carrier_heights =
            ValidBlock::committed_heights_for_entrypoint_carriers(&proposal, transactions);
        assert!(signed_heights[0].is_some());
        assert!(carrier_heights[0].is_none());
    }
    let (_, error) = fixture.validate(proposal).unpack(|_| {}).err().unwrap();
    assert!(matches!(
        *error,
        BlockValidationError::HasCommittedTransactions
    ));
    assert_eq!(fixture.chain.state().view().height(), 3);
}

#[test]
fn native_validation_rejects_missing_original_execution_context() {
    let fixture = NativeValidationFixture::new();
    let mut proposal = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    proposal.set_execution_context(None);
    let (_, error) = fixture.validate(proposal).unpack(|_| {}).err().unwrap();
    assert!(
        matches!(*error, BlockValidationError::ExecutionContextInvalid(ref message) if message.contains("missing execution context"))
    );
    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_execution_context_route_substitution() {
    let fixture = NativeValidationFixture::new();
    let tx = fixture.transaction(2_001, None);
    let mut proposal = fixture.proposal(vec![tx.clone()], fixture.cadence());
    proposal.set_execution_context(Some(BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::new(
            tx.hash_as_entrypoint(),
            LaneId::new(7),
            DataSpaceId::UNIVERSAL,
        ),
    ])));
    let (_, error) = fixture.validate(proposal).unpack(|_| {}).err().unwrap();
    assert!(
        matches!(*error, BlockValidationError::ExecutionContextInvalid(ref message) if message.contains("native committed execution route"))
    );
    assert_eq!(fixture.chain.state().view().height(), 2);
}
