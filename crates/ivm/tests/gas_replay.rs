//! Replay harness to ensure gas metering remains deterministic across runs.

fn gas_used_for_program(program: &[u8], gas_limit: u64) -> u64 {
    let mut vm = ivm::IVM::new(0);
    vm.set_gas_limit(gas_limit);
    vm.load_program(program).expect("load program");
    vm.run().expect("run program");
    gas_limit.saturating_sub(vm.remaining_gas())
}

#[test]
fn gas_replays_consistently_across_vms() {
    let meta = ivm::ProgramMetadata {
        max_cycles: 32,
        ..ivm::ProgramMetadata::default()
    };
    // Encode a small program: ADD + MUL + HALT to exercise multiple cost tiers.
    let mut program = meta.encode();
    program.extend_from_slice(
        &ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::ADD, 1, 2, 3)
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::MUL, 1, 1, 1)
            .to_le_bytes(),
    );
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

    let gas_limit = 128;
    let used_first = gas_used_for_program(&program, gas_limit);
    let used_second = gas_used_for_program(&program, gas_limit);

    assert!(used_first > 0);
    assert_eq!(used_first, used_second);
}
