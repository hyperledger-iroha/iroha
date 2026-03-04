use ivm::{IVM, Instruction, VMError};

fn encode_add(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}

fn encode_jump(target: u32) -> [u8; 4] {
    let hi = ((target >> 16) & 0x7ff) as u16;
    let lo = (target & 0xffff) as u16;
    let first = ((0x1fu16) << 11) | hi;
    let mut bytes = [0u8; 4];
    bytes[..2].copy_from_slice(&first.to_le_bytes());
    bytes[2..].copy_from_slice(&lo.to_le_bytes());
    bytes
}

#[test]
fn decode_add_and_pc() {
    let mut vm = IVM::new(u64::MAX);
    let code = encode_add(1, 2, 3);
    vm.load_code(&code).unwrap();
    let (inst, len) = vm.decode_next().unwrap();
    assert_eq!(
        inst,
        Instruction::Add {
            rd: 1,
            rs: 2,
            rt: 3
        }
    );
    assert_eq!(len, 2);
    assert_eq!(vm.pc, 0);
}

#[test]
fn decode_jump32() {
    let mut vm = IVM::new(u64::MAX);
    let code = encode_jump(0x123456);
    vm.load_code(&code).unwrap();
    let (inst, len) = vm.decode_next().unwrap();
    assert_eq!(inst, Instruction::Jump { target: 0x123456 });
    assert_eq!(len, 4);
    assert_eq!(vm.pc, 0);
}

#[test]
fn decode_invalid_opcode() {
    let mut vm = IVM::new(u64::MAX);
    vm.load_code(&[0xff, 0xff]).unwrap();
    let res = vm.decode_next();
    assert!(matches!(res, Err(VMError::DecodeError)));
}

#[test]
fn decode_oob() {
    let mut vm = IVM::new(u64::MAX);
    vm.load_code(&[]).unwrap();
    let res = vm.decode_next();
    assert!(matches!(res, Err(VMError::DecodeError)));
}
