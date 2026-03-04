use ivm::{IVM, Instruction, Memory, VMError};

#[test]
fn execute_add_sub() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 5);
    vm.set_register(2, 7);
    vm.execute_instruction(Instruction::Add {
        rd: 3,
        rs: 1,
        rt: 2,
    })
    .unwrap();
    assert_eq!(vm.register(3), 12);
    vm.set_register(4, 10);
    vm.execute_instruction(Instruction::Sub {
        rd: 5,
        rs: 4,
        rt: 1,
    })
    .unwrap();
    assert_eq!(vm.register(5), 5);
}

#[test]
fn execute_r0_operand() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(2, 42);
    vm.execute_instruction(Instruction::Add {
        rd: 1,
        rs: 0,
        rt: 2,
    })
    .unwrap();
    assert_eq!(vm.register(1), 42);
}

#[test]
fn privacy_violation() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 1);
    vm.registers.set_tag(1, true);
    vm.set_register(2, 2);
    vm.registers.set_tag(2, false);
    let res = vm.execute_instruction(Instruction::Add {
        rd: 3,
        rs: 1,
        rt: 2,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}

#[test]
fn load_store() {
    let mut vm = IVM::new(u64::MAX);
    let addr = Memory::HEAP_START;
    vm.memory.store_u64(addr, 0xdead_beef_dead_beef).unwrap();
    vm.set_register(1, addr);
    vm.execute_instruction(Instruction::Load {
        rd: 2,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();
    assert_eq!(vm.register(2), 0xdead_beef_dead_beef);
    vm.set_register(3, addr + 8);
    vm.set_register(4, 0x1234_5678_9abc_def0);
    vm.execute_instruction(Instruction::Store {
        rs: 4,
        addr_reg: 3,
        offset: 0,
    })
    .unwrap();
    assert_eq!(vm.memory.load_u64(addr + 8).unwrap(), 0x1234_5678_9abc_def0);
}

#[test]
fn out_of_gas() {
    let mut vm = IVM::new(0); // no gas
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let res = vm.execute_instruction(Instruction::Add {
        rd: 3,
        rs: 1,
        rt: 2,
    });
    assert!(matches!(res, Err(VMError::OutOfGas)));
    // register should remain zero
    assert_eq!(vm.register(3), 0);
}
