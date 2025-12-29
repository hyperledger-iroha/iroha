use ivm::{IVM, Instruction, Memory, VMError};

#[test]
fn branch_on_private_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 5);
    vm.registers.set_tag(1, true);
    vm.set_register(2, 5);
    vm.registers.set_tag(2, false);
    let res = vm.execute_instruction(Instruction::Beq {
        rs: 1,
        rt: 2,
        offset: 1,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}

#[test]
fn load_private_address_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::HEAP_START);
    vm.registers.set_tag(1, true);
    let res = vm.execute_instruction(Instruction::Load {
        rd: 2,
        addr_reg: 1,
        offset: 0,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}

#[test]
fn add_private_succeeds() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 3);
    vm.registers.set_tag(1, true);
    vm.set_register(2, 4);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Add {
        rd: 3,
        rs: 1,
        rt: 2,
    })
    .unwrap();
    assert_eq!(vm.register(3), 7);
    assert!(vm.registers.tag(3));
}
