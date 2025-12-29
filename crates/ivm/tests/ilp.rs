use ivm::IVM;

fn encode_add(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}

fn encode_halt() -> [u8; 2] {
    0u16.to_le_bytes()
}

#[test]
fn parallel_independent_adds() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    vm.set_register(3, 3);
    vm.set_register(4, 4);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_add(10, 1, 2));
    code.extend_from_slice(&encode_add(11, 3, 4));
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    vm.run_simple().unwrap();
    assert_eq!(vm.register(10), 3);
    assert_eq!(vm.register(11), 7);
}

#[test]
fn dependency_serialization() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 5);
    vm.set_register(5, 1);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_add(2, 1, 5));
    code.extend_from_slice(&encode_add(3, 2, 5));
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    vm.run_simple().unwrap();
    assert_eq!(vm.register(3), 7);
}

fn encode_load(rd: u16, base: u16) -> [u8; 2] {
    let word = ((0x3u16) << 12) | ((rd & 0xf) << 8) | ((base & 0xf) << 4);
    word.to_le_bytes()
}

#[test]
fn error_stops_batch() {
    let mut vm = IVM::new(u64::MAX);
    // set addr_reg r1 to invalid address beyond memory size (0x1000 when mem is 0)
    vm.set_register(1, 0x1000);
    vm.set_register(2, 1);
    vm.set_register(3, 2);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_load(4, 1));
    code.extend_from_slice(&encode_add(5, 2, 3));
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    let result = vm.run_simple();
    assert!(result.is_err());
    // second instruction must not have executed
    assert_eq!(vm.register(5), 0);
}
