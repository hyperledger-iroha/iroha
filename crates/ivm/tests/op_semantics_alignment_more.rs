use ivm::{IVM, Memory, ProgramMetadata, VMError, encoding, instruction};

const CLASSIC_LOAD_OPCODE: u32 = 0x03;
const CLASSIC_STORE_OPCODE: u32 = 0x23;

fn classic_load(rd: u8, rs1: u8, imm12: i16, funct3: u8) -> u32 {
    let imm = (imm12 as u16 as u32) & 0x0fff;
    CLASSIC_LOAD_OPCODE
        | ((rd as u32 & 0x1f) << 7)
        | ((funct3 as u32 & 0x7) << 12)
        | ((rs1 as u32 & 0x1f) << 15)
        | (imm << 20)
}

fn classic_store(rs1: u8, rs2: u8, imm12: i16, funct3: u8) -> u32 {
    let imm = (imm12 as u16 as u32) & 0x0fff;
    let imm5 = imm & 0x1f;
    let imm7 = (imm >> 5) & 0x7f;
    CLASSIC_STORE_OPCODE
        | (imm5 << 7)
        | ((funct3 as u32 & 0x7) << 12)
        | ((rs1 as u32 & 0x1f) << 15)
        | ((rs2 as u32 & 0x1f) << 20)
        | (imm7 << 25)
}

#[test]
fn wide_load_store_alignment() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0)
        .expect("encode addi");
    let store64 = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 3, 0);
    let load64 = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 4, 2, 0);
    let halt = encoding::wide::encode_halt();

    let mut bytes = ProgramMetadata::default().encode();
    for word in [addi_base, store64, load64, halt] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }

    let mut vm = IVM::new(128);
    vm.load_program(&bytes).expect("load");
    vm.set_register(2, Memory::HEAP_START);
    vm.set_register(3, 0xDEAD_BEEFCAFEBABEu64);
    vm.run().expect("run");
    assert_eq!(vm.register(4), 0xDEAD_BEEFCAFEBABE);

    let store_misal = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 3, 1);
    let mut bytes_misal = ProgramMetadata::default().encode();
    for word in [addi_base, store_misal, halt] {
        bytes_misal.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm_store = IVM::new(64);
    vm_store.load_program(&bytes_misal).expect("load");
    vm_store.set_register(2, Memory::HEAP_START);
    vm_store.set_register(3, 0xABCDu64);
    let res = vm_store.run();
    assert!(matches!(
        res,
        Err(VMError::MisalignedAccess { .. }) | Err(VMError::UnalignedAccess)
    ));

    // Prepare memory so an aligned store succeeds, then attempt a misaligned load.
    let mut vm_load = IVM::new(256);
    vm_load.load_program(&bytes).expect("load aligned program");
    vm_load.set_register(2, Memory::HEAP_START);
    vm_load.set_register(3, 0x0123_4567_89AB_CDEF);
    vm_load.run().expect("store aligned");

    let load_misal = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 4, 2, 1);
    let mut bytes_load = ProgramMetadata::default().encode();
    for word in [addi_base, load_misal, halt] {
        bytes_load.extend_from_slice(&word.to_le_bytes());
    }
    vm_load
        .load_program(&bytes_load)
        .expect("load misaligned program");
    vm_load.set_register(2, Memory::HEAP_START);
    let res = vm_load.run();
    assert!(matches!(
        res,
        Err(VMError::MisalignedAccess { .. }) | Err(VMError::UnalignedAccess)
    ));
}

#[test]
fn classic_halfword_ops_rejected() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0)
        .expect("encode addi");
    let sh = classic_store(2, 3, 2, 0x1);
    let lh = classic_load(4, 2, 2, 0x1);
    let lhu = classic_load(5, 2, 2, 0x5);
    let mut bytes = ProgramMetadata::default().encode();
    for word in [addi_base, sh, lh, lhu] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(64);
    let err = vm
        .load_program(&bytes)
        .expect_err("classic halfword ops must be rejected");
    assert!(matches!(err, VMError::InvalidOpcode(_)));
}

#[test]
fn classic_byte_ops_rejected() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0)
        .expect("encode addi");
    let lb = classic_load(3, 2, 0, 0x0);
    let lbu = classic_load(4, 2, 0, 0x4);
    let mut bytes = ProgramMetadata::default().encode();
    for word in [addi_base, lb, lbu] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(64);
    let err = vm
        .load_program(&bytes)
        .expect_err("classic byte ops must be rejected");
    assert!(matches!(err, VMError::InvalidOpcode(_)));
}

#[test]
fn classic_misaligned_halfword_ops_rejected() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0)
        .expect("encode addi");
    let lh_mis = classic_load(4, 2, 1, 0x1);
    let lhu_mis = classic_load(5, 2, 1, 0x5);
    let mut bytes = ProgramMetadata::default().encode();
    for word in [addi_base, lh_mis, lhu_mis] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(64);
    let err = vm
        .load_program(&bytes)
        .expect_err("classic misaligned halfword ops must be rejected");
    assert!(matches!(err, VMError::InvalidOpcode(_)));
}
