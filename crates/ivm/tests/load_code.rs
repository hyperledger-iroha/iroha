use ivm::IVM;

#[test]
fn load_code_into_memory() {
    let mut vm = IVM::new(u64::MAX);
    let code = [0x01u8, 0x02, 0x03];
    vm.load_code(&code).expect("load failed");
    assert_eq!(vm.memory.load_u8(0).unwrap(), 0x01);
    assert_eq!(vm.memory.load_u8(1).unwrap(), 0x02);
    assert_eq!(vm.memory.load_u8(2).unwrap(), 0x03);
    assert_eq!(vm.pc, 0);
}
