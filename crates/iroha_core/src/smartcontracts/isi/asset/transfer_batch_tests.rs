// Boundary coverage for deterministic batch-transfer admission.

#[test]
fn transfer_batch_rejects_empty_entries() {
    let state = State::new(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let err = TransferAssetBatch::new(Vec::new())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("empty transfer batches must be rejected");
    match err {
        InstructionExecutionError::InvariantViolation(message) => {
            assert!(message.contains("requires at least one entry"), "{message}")
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
