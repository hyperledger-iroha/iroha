#[tokio::test]
async fn tx_order_same_in_validation_and_revalidation() {
    // Predefined world state
    let (alice_id, alice_keypair) = gen_account_in("wonderland");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
    let account = Account::new(alice_id.clone()).build(&alice_id);
    let domain = Domain::new(domain_id).build(&alice_id);
    let domain_a_id = DomainId::try_new("domain-a", "universal").unwrap();
    let domain_b_id = DomainId::try_new("domain-b", "universal").unwrap();
    let mut world = World::with([domain], [account], []);
    seed_domain_name_lease(&mut world, &alice_id, &domain_a_id);
    seed_domain_name_lease(&mut world, &alice_id, &domain_b_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);
    install_test_lane_manifests(&state);
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    // Two independent register instructions (no ordering dependencies)
    let domain_a = Register::domain(Domain::new(domain_a_id));
    let domain_b = Register::domain(Domain::new(domain_b_id));
    let tx = TransactionBuilder::new(
        state.network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions::<InstructionBox>([domain_a.into()])
    .sign(alice_keypair.private_key());
    let crypto_cfg = state.crypto();
    let tx = AcceptedTransaction::accept(
        tx,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Valid");
    let fail_domain_id = DomainId::try_new("missing-domain", "universal").expect("valid id");
    let fail_instruction = Unregister::domain(fail_domain_id);
    let succeed_instruction = domain_b;
    let tx0 = TransactionBuilder::new(
        state.network_id,
        alice_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions::<InstructionBox>([fail_instruction.into()])
    .sign(alice_keypair.private_key());
    let tx0 = AcceptedTransaction::accept(
        tx0,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Valid");
    let tx2 = TransactionBuilder::new(
        state.network_id,
        alice_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions::<InstructionBox>([succeed_instruction.into()])
    .sign(alice_keypair.private_key());
    let tx2 = AcceptedTransaction::accept(
        tx2,
        &state.network_id,
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("Valid");
    let fail_hash = tx0.as_ref().hash_as_entrypoint();
    let register_hash = tx.as_ref().hash_as_entrypoint();
    let succeed_hash = tx2.as_ref().hash_as_entrypoint();
    // Creating a block of two identical transactions and validating it
    let transactions = vec![tx0, tx, tx2];
    let unverified_block = BlockBuilder::new(transactions)
        .chain(0, state.view().latest_block().as_deref())
        .sign(alice_keypair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified_block.header);
    let valid_block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    state_block.commit().unwrap();
    // The 1st transaction should fail and 2nd succeed
    let block_ref = valid_block.as_ref();
    let outcomes: Vec<_> = block_ref
        .entrypoint_hashes()
        .zip(block_ref.results())
        .collect();
    let lookup = |hash: &_, label: &str| {
        outcomes
            .iter()
            .find(|(entry_hash, _)| entry_hash == hash)
            .unwrap_or_else(|| panic!("missing result for {label}"))
            .1
            .as_ref()
    };
    let fail_result = lookup(&fail_hash, "fail tx");
    assert!(fail_result.is_err(), "fail tx must be rejected");
    let register_result = lookup(&register_hash, "register tx");
    assert!(
        register_result.is_ok(),
        "register tx must succeed, got {register_result:?}"
    );
    let succeed_result = lookup(&succeed_hash, "succeed tx");
    assert!(
        succeed_result.is_ok(),
        "succeed tx must succeed, got {succeed_result:?}"
    );
}
