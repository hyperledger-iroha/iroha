use ivm::{Memory, Perm, VMError, decode};

fn decode_from_bytes(bytes: &[u8]) -> Result<(u32, u32), VMError> {
    let mut mem = Memory::new(bytes.len() as u64);
    mem.load_code(bytes);
    decode(&mem, 0)
}

fn encode_r16(op: u8, rd: u8, rs1: u8, rs2: u8) -> u16 {
    ((op & 0xF) as u16) << 12
        | ((rd & 0xF) as u16) << 8
        | ((rs1 & 0xF) as u16) << 4
        | (rs2 & 0xF) as u16
}

fn encode_i16(op: u8, rd: u8, rs: u8, imm4: i8) -> u16 {
    let imm = (imm4 as u8) & 0xF;
    ((op & 0xF) as u16) << 12 | ((rd & 0xF) as u16) << 8 | ((rs & 0xF) as u16) << 4 | imm as u16
}

fn encode_li16(op: u8, rd: u8, imm8: i8) -> u16 {
    ((op & 0xF) as u16) << 12 | ((rd & 0xF) as u16) << 8 | (imm8 as u8 as u16)
}

#[test]
fn compressed_forms_are_rejected_in_wide_mode() {
    let cases = [
        ("c.add", encode_r16(0x1, 1, 2, 3)),
        ("c.sub", encode_r16(0x2, 4, 5, 6)),
        ("c.xor", encode_r16(0x6, 7, 8, 9)),
        ("c.li.pos", encode_li16(0x3, 10, 7)),
        ("c.li.neg", encode_li16(0x3, 11, -4)),
        ("c.beq", encode_i16(0x5, 3, 4, 2)),
    ];

    for (name, inst16) in cases {
        let bytes = inst16.to_le_bytes();
        let err = decode_from_bytes(&bytes).expect_err(name);
        match err {
            VMError::MemoryAccessViolation {
                perm: Perm::EXECUTE,
                ..
            } => {}
            other => panic!("{name} returned unexpected error: {other:?}"),
        }
    }
}
