// Original proposal custody and finite history-pool admission through native validation.

fn ordinary_membership_source_bytes(carrier: &SignedBlock) -> usize {
    std::alloc::Layout::array::<HashOf<TransactionEntrypoint>>(
        carrier.external_entrypoint_count() * 2,
    )
    .unwrap()
    .size()
}

#[test]
fn ordinary_signed_carrier_capacity_refusal_keeps_original_source_for_retry() {
    let fixture = NativeValidationFixture::new();
    let carrier = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    let original = carrier.encode_wire().unwrap();
    let state = fixture.chain.state();
    let budget = fixture.chain.kura().transaction_history_budget();
    let baseline = budget.reserved_bytes();
    let source_bytes = ordinary_membership_source_bytes(&carrier);
    let free = budget.limit_bytes() - baseline;
    assert!(free > source_bytes);
    let occupied = budget.try_reserve_bytes(free - source_bytes + 1).unwrap();
    let (retained, refusal) = fixture.validate(carrier).unpack(|_| {}).err().unwrap();
    assert!(
        matches!(*refusal, BlockValidationError::MembershipAdmission(
        crate::state::MembershipAdmissionError::Capacity(
            mv::allocation::AllocationRefusal::Capacity { requested_bytes, .. }
        )
    ) if requested_bytes == source_bytes)
    );
    assert_eq!(retained.encode_wire().unwrap(), original);
    assert_eq!(state.view().height(), 2);
    assert_eq!(fixture.chain.kura().blocks_count(), 2);
    assert_eq!(
        budget.reserved_bytes(),
        budget.limit_bytes() - source_bytes + 1
    );
    drop(occupied);
    let (valid, retry) = fixture.validate(*retained).unpack(|_| {}).unwrap();
    assert_eq!(
        valid
            .as_ref()
            .canonical_resultless_proposal()
            .encode_wire()
            .unwrap(),
        original
    );
    assert!(budget.reserved_bytes() >= baseline + source_bytes);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn ordinary_signed_carrier_admits_exact_source_and_block_owner_boundary() {
    let fixture = NativeValidationFixture::new();
    let carrier = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    let budget = fixture.chain.kura().transaction_history_budget();
    let baseline = budget.reserved_bytes();
    let (_, probe) = fixture.validate(carrier.clone()).unpack(|_| {}).unwrap();
    let complete_owner_bytes = budget.reserved_bytes() - baseline;
    assert!(complete_owner_bytes >= ordinary_membership_source_bytes(&carrier));
    drop(probe);
    assert_eq!(budget.reserved_bytes(), baseline);
    let free = budget.limit_bytes() - baseline;
    assert!(free > complete_owner_bytes);
    let occupied = budget
        .try_reserve_bytes(free - complete_owner_bytes)
        .unwrap();
    let (_, retry) = fixture.validate(carrier).unpack(|_| {}).unwrap();
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(retry);
    drop(occupied);
    assert_eq!(budget.reserved_bytes(), baseline);
    assert_eq!(fixture.chain.state().view().height(), 2);
}
