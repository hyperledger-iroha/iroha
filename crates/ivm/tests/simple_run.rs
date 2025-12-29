use ivm::IVM;

fn encode_add(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}

fn encode_beq(rs: u16, rt: u16, off: i8) -> [u8; 2] {
    let imm = (off as u8) & 0xf;
    let word = ((0x5u16) << 12) | ((rs & 0xf) << 8) | ((rt & 0xf) << 4) | imm as u16;
    word.to_le_bytes()
}

fn encode_halt() -> [u8; 2] {
    0u16.to_le_bytes()
}

#[test]
fn branch_skip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 5);
    vm.set_register(2, 5);
    vm.set_register(4, 1);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_beq(1, 2, 1));
    code.extend_from_slice(&encode_add(3, 0, 4));
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    vm.run_simple().unwrap();
    assert_eq!(vm.register(3), 0);
}

#[test]
fn branch_not_taken() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 5);
    vm.set_register(2, 6);
    vm.set_register(4, 1);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_beq(1, 2, 1));
    code.extend_from_slice(&encode_add(3, 0, 4));
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    vm.run_simple().unwrap();
    assert_eq!(vm.register(3), 1);
}
