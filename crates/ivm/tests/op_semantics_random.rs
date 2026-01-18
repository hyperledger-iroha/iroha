use ivm::{IVM, Memory, ProgramMetadata, VMError, encoding, instruction};

const CLASSIC_LOAD_OPCODE: u32 = 0x03;
const CLASSIC_STORE_OPCODE: u32 = 0x23;

fn run_prog(code_words: &[u32]) -> IVM {
    // Patch a leading `addi x2, x0, imm` into a no-op `addi x2, x2, 0` so tests
    // can rely on host-provided x2 base for heap addressing without worrying
    // about 12-bit immediate range.
    let mut words = code_words.to_vec();
    if let Some(first) = words.first_mut() {
        let op_lo = *first & 0x7f;
        let rd = ((*first >> 7) & 0x1f) as u8;
        let funct3 = ((*first >> 12) & 0x7) as u8;
        let rs1 = ((*first >> 15) & 0x1f) as u8;
        if op_lo == 0x13 && rd == 2 && funct3 == 0 && rs1 == 0 {
            *first = ivm::kotodama::compiler::encode_addi(2, 2, 0).expect("encode addi");
        }
    }
    let mut bytes = ProgramMetadata::default().encode();
    for w in &words {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.load_program(&bytes).unwrap();
    // Tests in this module target memory semantics; ensure x2 holds a valid heap base.
    vm.set_register(2, ivm::Memory::HEAP_START);
    let _ = vm.run();
    vm
}

#[test]
fn loads_stores_alignment_semantics() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0).expect("encode addi");
    let store64 = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 3, 0);
    let load64 = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 4, 2, 0);
    let halt = encoding::wide::encode_halt();

    let mut vm = run_prog(&[addi_base, store64, load64, halt]);
    vm.reset();
    vm.set_register(2, ivm::Memory::HEAP_START);
    vm.set_register(3, 0x42);
    vm.run().expect("aligned run");
    assert_eq!(vm.register(4), 0x42);

    let store_misal = encoding::wide::encode_store(instruction::wide::memory::STORE64, 2, 3, 1);
    let mut misaligned_bytes = ProgramMetadata::default().encode();
    for w in [addi_base, store_misal, halt] {
        misaligned_bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm_store = IVM::new(100);
    vm_store.load_program(&misaligned_bytes).unwrap();
    vm_store.set_register(2, ivm::Memory::HEAP_START);
    vm_store.set_register(3, 0x99);
    let res = vm_store.run();
    assert!(matches!(
        res,
        Err(VMError::MisalignedAccess { .. }) | Err(VMError::UnalignedAccess)
    ));

    let load_misal = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 4, 2, 1);
    let mut load_bytes = ProgramMetadata::default().encode();
    for w in [addi_base, store64, load_misal, halt] {
        load_bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut vm_load = IVM::new(100);
    vm_load.load_program(&load_bytes).unwrap();
    vm_load.set_register(2, ivm::Memory::HEAP_START);
    vm_load.set_register(3, 0x55AA);
    let res2 = vm_load.run();
    assert!(matches!(
        res2,
        Err(VMError::MisalignedAccess { .. }) | Err(VMError::UnalignedAccess)
    ));
}

#[test]
fn classic_word_ops_rejected() {
    let addi_base = ivm::kotodama::compiler::encode_addi(2, 2, 0).expect("encode addi");
    let addi_val = ivm::kotodama::compiler::encode_addi(3, 3, 0).expect("encode addi");
    let stw = CLASSIC_STORE_OPCODE | (0x2 << 12) | (2 << 15) | (3 << 20);
    let ldw = CLASSIC_LOAD_OPCODE | (0x2 << 12) | (2 << 15) | (4 << 7);
    let mut bytes = ProgramMetadata::default().encode();
    for word in [addi_base, addi_val, stw, ldw] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(64);
    let err = vm
        .load_program(&bytes)
        .expect_err("classic 32-bit ops must be rejected");
    assert!(matches!(err, VMError::InvalidOpcode(_)));
}

#[test]
fn branches_randomized_consistency() {
    // Simple BEQ with random values. The wide encoding applies the offset in
    // 32-bit instruction words relative to the current PC, so an offset of 2
    // skips exactly one instruction when the branch is taken.
    let mut vm = IVM::new(1000);
    // Build: beq x5,x6,+2; addi x3,x0,7; halt
    let mut bytes = ProgramMetadata::default().encode();
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 5, 6, 2);
    bytes.extend_from_slice(&beq.to_le_bytes());
    bytes.extend_from_slice(
        &ivm::kotodama::compiler::encode_addi(3, 0, 7)
            .expect("encode addi")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    vm.load_program(&bytes).unwrap();

    for a in [0, 1, 7, 42, 255, 1024] {
        for b in [0, 1, 7, 41, 255, 2048] {
            vm.reset();
            vm.set_register(3, 0);
            vm.set_register(5, a as u64);
            vm.set_register(6, b as u64);
            let _ = vm.run();
            // If a==b, branch is taken and skips the `addi` so x3 stays 0.
            // If a!=b, branch is not taken and `addi` executes, setting x3 to 7.
            if a == b {
                assert_eq!(vm.register(3), 0);
            } else {
                assert_eq!(vm.register(3), 7);
            }
        }
    }
}

#[test]
fn vector_ops_mode_gating() {
    let mut meta = ProgramMetadata::default();
    let mut bytes = meta.encode();
    let sha = encoding::wide::encode_rr(instruction::wide::crypto::SHA256BLOCK, 16, 32, 0);
    bytes.extend_from_slice(&sha.to_le_bytes());
    let mut vm = IVM::new(1_000);
    vm.load_program(&bytes).unwrap();
    vm.set_register(32, Memory::HEAP_START);
    let err = vm.run().expect_err("vector op should be gated");
    assert!(matches!(err, VMError::VectorExtensionDisabled));

    meta.mode = ivm::ivm_mode::VECTOR;
    let mut bytes2 = meta.encode();
    bytes2.extend_from_slice(&sha.to_le_bytes());
    bytes2.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm2 = IVM::new(1_000);
    vm2.load_program(&bytes2).unwrap();
    vm2.set_register(32, Memory::HEAP_START);
    vm2.memory
        .store_bytes(Memory::HEAP_START, &[0u8; 64])
        .unwrap();
    vm2.run().expect("vector op succeeds when enabled");
}
