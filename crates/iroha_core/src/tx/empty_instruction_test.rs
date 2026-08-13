#[test]
fn state_rejects_empty_instruction_transactions() {
    let chain: ChainId = "empty-instructions-chain".parse().unwrap();
    let (world, authority, keypair) = world_with_authority("wonderland");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_chain(world, kura, query_handle, chain.clone());
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(std::iter::empty::<InstructionBox>())
    .sign(keypair.private_key());
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = IvmCache::new();
    let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
    match result {
        Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
            assert!(
                msg.contains("at least one instruction"),
                "expected empty-instruction rejection, got {msg}"
            );
        }
        other => panic!("expected empty-instruction rejection, got {other:?}"),
    }
}
