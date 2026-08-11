#[tokio::test]
async fn find_transaction() -> Result<()> {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura.clone(), query_handle);
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };

    let crypto_cfg = state.crypto();

    let ok_instruction = Log::new(iroha_logger::Level::INFO, "pass".into());
    let tx = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([ok_instruction])
    .sign(ALICE_KEYPAIR.private_key());

    let va_tx = AcceptedTransaction::accept(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )?;

    let (peer_public_key, _) = bls_test_keypair().into_parts();
    let peer_id = PeerId::new(peer_public_key);
    let topology = Topology::new(vec![peer_id]);
    let unverified_block = BlockBuilder::new(vec![va_tx.clone()])
        .chain(0, state.view().latest_block().as_deref())
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header());
    let vcb = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {})
        .commit(&topology)
        .unpack(|_| {})
        .unwrap();

    let _events = state_block.apply(&vcb, topology.as_ref().to_owned());
    kura.store_block(vcb).expect("store block");
    state_block.commit().unwrap();

    let state_view = state.view();

    let unapplied_tx = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Unregister::account(gen_account_in("domain").0)])
    .sign(ALICE_KEYPAIR.private_key());
    let wrong_hash = TransactionEntrypoint::from(unapplied_tx).hash();

    let not_found = FindTransactions::new()
        .execute(CompoundPredicate::PASS, &state_view)
        .expect("Query execution should not fail")
        .find(|tx| *tx.entrypoint_hash() == wrong_hash);
    assert_eq!(not_found, None, "Transaction should not be found");

    let found_accepted = FindTransactions::new()
        .execute(CompoundPredicate::PASS, &state_view)
        .expect("Query execution should not fail")
        .find(|tx| *tx.entrypoint_hash() == va_tx.as_ref().hash_as_entrypoint())
        .expect("Query should return a transaction");

    if found_accepted.result().is_err() {
        assert_eq!(
            va_tx.as_ref().hash_as_entrypoint(),
            found_accepted.entrypoint().hash(),
        )
    }
    Ok(())
}
