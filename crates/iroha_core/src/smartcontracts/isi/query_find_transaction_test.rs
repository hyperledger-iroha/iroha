// Query source identity and physical finality; economic execution is covered by the State owner tests.
#[tokio::test]
async fn find_transaction() -> Result<()> {
    let fixture = crate::smartcontracts::isi::tx::tests::canonical_query_fixture();
    let state_view = fixture.sandbox.state.view();
    let unapplied_tx = TransactionBuilder::new(
        fixture.sandbox.state.network_id,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Unregister::account(gen_account_in("domain").0)])
    .sign(ALICE_KEYPAIR.private_key());
    let wrong_hash = TransactionEntrypoint::from(unapplied_tx).hash();
    let not_found = crate::smartcontracts::isi::tx::execute_transactions_fixture(
        CompoundPredicate::PASS,
        &state_view,
    )?
    .find(|tx| *tx.entrypoint_hash() == wrong_hash);
    assert_eq!(not_found, None, "Transaction should not be found");
    let found_accepted = crate::smartcontracts::isi::tx::execute_transactions_fixture(
        CompoundPredicate::PASS,
        &state_view,
    )?
    .find(|tx| *tx.entrypoint_hash() == fixture.target_entrypoint_hash)
    .expect("Query should return a transaction");
    assert!(
        found_accepted.result().is_err(),
        "selected fixture preserves rejected-source lookup"
    );
    assert_eq!(
        fixture.target_entrypoint_hash,
        found_accepted.entrypoint().hash()
    );
    assert!(
        found_accepted
            .verify_inclusion_in_block(&fixture.store.blocks[fixture.target_height.get() - 1])
    );
    Ok(())
}
