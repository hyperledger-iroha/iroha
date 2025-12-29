use ivm::{Memory, VMError, decode, encoding, instruction};

#[test]
fn zero_word_decodes_as_32bit() {
    let mut mem = Memory::new(4);
    mem.load_code(&[0x00, 0x00, 0x00, 0x00]);
    let (inst, len) = decode(&mem, 0).expect("decode");
    assert_eq!(len, 4);
    assert_eq!(inst, 0);
}

#[test]
fn wide_halt_decodes() {
    let halt = encoding::wide::encode_halt();
    let mut mem = Memory::new(4);
    mem.load_code(&halt.to_le_bytes());
    let (inst, len) = decode(&mem, 0).expect("decode");
    assert_eq!(len, 4);
    assert_eq!((inst >> 24) as u8, instruction::wide::control::HALT);
}

#[test]
fn misaligned_pc_is_error() {
    let mut mem = Memory::new(4);
    mem.load_code(&[0x00, 0x00, 0x00, 0x4C]);
    let err = decode(&mem, 1).unwrap_err();
    assert!(matches!(
        err,
        VMError::MemoryAccessViolation { addr: 1, .. }
    ));
}

#[test]
fn misaligned_pc_various_odd_addresses() {
    let mut mem = Memory::new(12);
    mem.load_code(&[
        0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x40, 0x00, 0x01, 0x02, 0x03, 0x04,
    ]);
    for &pc in &[1u64, 3, 5, 7, 9, 11] {
        let err = decode(&mem, pc).unwrap_err();
        assert!(matches!(err, VMError::MemoryAccessViolation { addr, .. } if addr as u64 == pc));
    }
}

#[test]
fn decode_oob_reports_violation() {
    let mut mem = Memory::new(4);
    mem.load_code(&[0xAA, 0xBB, 0xCC, 0xDD]);
    let err = decode(&mem, 4).unwrap_err();
    assert!(matches!(err, VMError::MemoryAccessViolation { .. }));
}

#[test]
fn decode_tail_requires_full_word() {
    let mut mem = Memory::new(3);
    mem.load_code(&[0x01, 0x02, 0x03]);
    let err = decode(&mem, 0).unwrap_err();
    assert!(matches!(err, VMError::MemoryAccessViolation { .. }));
}
