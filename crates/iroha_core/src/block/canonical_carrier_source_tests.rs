// Original signed-carrier custody and finite-pool refusal at the production
// ordinary `state_block_for_execution` entry, before its pristine callback.

fn funded_membership_test_state() -> (State, Arc<Kura>) {
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    (state, kura)
}

fn funded_membership_test_carrier(state: &State) -> SignedBlock {
    use iroha_data_model::transaction::FeePaymentIntent;

    let transaction = TransactionBuilder::new(
        state.network_id,
        iroha_test_samples::ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([Log::new(Level::INFO, "funded membership".to_owned())])
    .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        1,
        0,
    ));
    builder.push_transaction(transaction);
    builder.build_with_signature(0, iroha_test_samples::ALICE_KEYPAIR.private_key())
}

fn ordinary_membership_source_bytes(carrier: &SignedBlock) -> usize {
    std::alloc::Layout::array::<HashOf<TransactionEntrypoint>>(
        carrier.external_entrypoint_count() * 2,
    )
    .unwrap()
    .size()
}

#[test]
fn ordinary_signed_carrier_capacity_refusal_keeps_original_source_for_retry() {
    let (state, kura) = funded_membership_test_state();
    let carrier = funded_membership_test_carrier(&state);
    let original_hash = carrier.hash();
    let budget = kura.transaction_history_budget();
    let baseline = budget.reserved_bytes();
    let source_bytes = ordinary_membership_source_bytes(&carrier);
    let free = budget.limit_bytes() - baseline;
    assert!(free > source_bytes);
    let occupied = budget
        .try_reserve_bytes(free - source_bytes + 1)
        .expect("leave one byte less than the exact source backing");
    let refusal = ValidBlock::state_block_for_execution(&carrier, &state, false, None, None, None)
        .err()
        .expect("original history pool must refuse before block start");
    assert!(matches!(
        refusal,
        BlockValidationError::MembershipAdmission(
            crate::state::MembershipAdmissionError::Capacity(
                mv::allocation::AllocationRefusal::Capacity { requested_bytes, .. }
            )
        ) if requested_bytes == source_bytes
    ));
    assert_eq!(carrier.hash(), original_hash);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(kura.blocks_count(), 0);
    assert_eq!(
        budget.reserved_bytes(),
        budget.limit_bytes() - source_bytes + 1
    );
    drop(occupied);
    let mut retry =
        ValidBlock::state_block_for_execution(&carrier, &state, false, None, None, None)
            .expect("same signed carrier retries after original capacity is released");
    assert_eq!(retry._curr_block, carrier.header());
    assert!(budget.reserved_bytes() >= baseline + source_bytes);
    retry
        .stage_prepaid_ordinary_carrier_membership(&carrier, nonzero!(1_usize))
        .expect("first funded source stages the canonical tip");
    let first_stage_bytes = budget.reserved_bytes();
    retry
        .stage_prepaid_ordinary_carrier_membership(&carrier, nonzero!(1_usize))
        .expect("second output tail reuses the exact charged tip");
    assert_eq!(budget.reserved_bytes(), first_stage_bytes);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn ordinary_signed_carrier_admits_exact_source_and_block_owner_boundary() {
    let (state, kura) = funded_membership_test_state();
    let carrier = funded_membership_test_carrier(&state);
    let budget = kura.transaction_history_budget();
    let baseline = budget.reserved_bytes();
    let source_bytes = ordinary_membership_source_bytes(&carrier);
    let probe = state
        .try_block(carrier.header())
        .expect("baseline block owner");
    let block_owner_bytes = budget.reserved_bytes() - baseline;
    drop(probe);
    assert_eq!(budget.reserved_bytes(), baseline);
    let free = budget.limit_bytes() - baseline;
    assert!(free > source_bytes + block_owner_bytes);
    let occupied = budget
        .try_reserve_bytes(free - source_bytes - block_owner_bytes)
        .expect("leave exactly the source and acquired block owner demand");
    let retry = ValidBlock::state_block_for_execution(&carrier, &state, false, None, None, None)
        .expect("exact original history-pool boundary must admit");
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert_eq!(retry._curr_block, carrier.header());
    drop(retry);
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), baseline);
}
