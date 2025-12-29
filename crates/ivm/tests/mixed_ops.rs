use ivm::{IVM, Memory, simple_instruction::Instruction};

fn encode_add(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}

fn encode_load(rd: u16, base: u16) -> [u8; 2] {
    let word = ((0x3u16) << 12) | ((rd & 0xf) << 8) | ((base & 0xf) << 4);
    word.to_le_bytes()
}

fn encode_store(rs: u16, base: u16) -> [u8; 2] {
    let word = ((0x4u16) << 12) | ((rs & 0xf) << 8) | ((base & 0xf) << 4);
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
fn mixed_operations_sequence() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 10);
    vm.set_register(2, 32);
    vm.set_register(5, Memory::HEAP_START);
    let mut code = Vec::new();
    code.extend_from_slice(&encode_add(3, 1, 2));
    code.extend_from_slice(&encode_store(3, 5));
    code.extend_from_slice(&encode_load(4, 5));
    code.extend_from_slice(&encode_beq(3, 4, 1));
    code.extend_from_slice(&encode_add(6, 0, 1)); // skipped if branch taken
    code.extend_from_slice(&encode_halt());
    vm.load_code(&code).unwrap();
    vm.run_simple().unwrap();
    assert_eq!(vm.register(3), 42);
    assert_eq!(vm.register(4), 42);
    assert_eq!(vm.register(6), 0);

    // compute SHA-256 over stored value
    vm.execute_instruction(Instruction::Sha256 {
        dest: 7,
        src_addr: Memory::HEAP_START,
        len: 8,
    })
    .unwrap();
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(42u64.to_le_bytes());
    for i in 0..4 {
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&digest[i * 8..(i + 1) * 8]);
        assert_eq!(vm.register(7 + i), u64::from_le_bytes(chunk));
    }
}
