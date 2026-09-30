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
            .expect("valid fixture proposal projection")
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
    // Retained bytes omit construction scratch, so discover the exact peak from the
    // original pool's concrete refusals. Each retry owns the same source and State.
    let original = carrier.encode_wire().unwrap();
    let mut peak_owner_bytes = complete_owner_bytes;
    loop {
        let occupied_bytes = free - peak_owner_bytes;
        let occupied = budget.try_reserve_bytes(occupied_bytes).unwrap();
        let result = fixture.validate(carrier.clone()).unpack(|_| {});
        match result {
            Ok((_, overlay)) => {
                assert_eq!(
                    budget.reserved_bytes(),
                    baseline + occupied_bytes + complete_owner_bytes
                );
                assert_eq!(budget.peak_reserved_bytes(), budget.limit_bytes());
                drop(overlay);
                drop(occupied);
                break;
            }
            Err((retained, error)) => {
                assert_eq!(retained.encode_wire().unwrap(), original);
                let BlockValidationError::MembershipAdmission(
                    crate::state::MembershipAdmissionError::Capacity(
                        mv::allocation::AllocationRefusal::Capacity {
                            requested_bytes,
                            reserved_bytes,
                            ..
                        },
                    ),
                ) = *error
                else {
                    panic!("expected an exact original pool refusal, got {error:?}");
                };
                let required = reserved_bytes + requested_bytes - baseline - occupied_bytes;
                assert!(required > peak_owner_bytes && required < free);
                peak_owner_bytes = required;
                drop(occupied);
            }
        }
        // Capacity refusal retains the original unfinished history preparation.
        // Its credits belong to the next retry; they retire after attachment.
        assert!((baseline..=baseline + peak_owner_bytes).contains(&budget.reserved_bytes()));
        assert_eq!(fixture.chain.state().view().height(), 2);
    }
    assert_eq!(budget.reserved_bytes(), baseline);
    let occupied = budget
        .try_reserve_bytes(free - peak_owner_bytes + 1)
        .unwrap();
    let (retained, error) = fixture.validate(carrier).unpack(|_| {}).err().unwrap();
    assert_eq!(retained.encode_wire().unwrap(), original);
    assert!(matches!(
        *error,
        BlockValidationError::MembershipAdmission(
            crate::state::MembershipAdmissionError::Capacity(
                mv::allocation::AllocationRefusal::Capacity { .. }
            )
        )
    ));
    drop(occupied);
    let (_, retried) = fixture.validate(*retained).unpack(|_| {}).unwrap();
    drop(retried);
    assert_eq!(budget.reserved_bytes(), baseline);
    assert_eq!(fixture.chain.state().view().height(), 2);
}
