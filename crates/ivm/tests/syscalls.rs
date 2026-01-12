use iroha_crypto::{Hash, HashOf, MerkleProof};
use ivm::{IVM, PointerType, VMError, encoding, instruction, syscalls};
mod common;
use common::assemble;

const HALT: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();

fn assemble_syscall(syscall: u8) -> Vec<u8> {
    let mut code = Vec::with_capacity(8);
    let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, syscall);
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&HALT);
    assemble(&code)
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn debug_print_is_handled_by_default_host() {
    let mut vm = IVM::new(u64::MAX);
    // Program: SCALL 0x00; HALT
    let prog = assemble_syscall(syscalls::SYSCALL_DEBUG_PRINT as u8);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(result.is_ok(), "debug_print should be handled");
}

#[test]
fn syscall_policy_gating_allows_known_and_rejects_unknown_v1() {
    // Build a tiny program making a known allowed syscall (ALLOC)
    // and then HALT. Set ABI version to 1 in the header to exercise policy.
    let mut bytes = assemble_syscall(syscalls::SYSCALL_ALLOC as u8);
    // Header layout: magic(4) major(1) minor(1) mode(1) vl(1) max_cycles(8) abi(1)
    // Set abi_version (last header byte) to 1
    bytes[4 + 1 + 1 + 1 + 1 + 8] = 1;
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&bytes).unwrap();
    vm.run().expect("known syscall should be allowed under V1");

    // Replace syscall with an unknown number and expect UnknownSyscall
    let mut bad = assemble_syscall(0xAB);
    bad[4 + 1 + 1 + 1 + 1 + 8] = 1; // ABI v1
    let mut vm2 = IVM::new(u64::MAX);
    vm2.load_program(&bad).unwrap();
    let res = vm2.run();
    assert!(matches!(res, Err(VMError::UnknownSyscall(0xAB))));
}

#[test]
fn syscall_exit_halts_execution() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(10, 5);
    let mut code = Vec::new();
    let exit_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_EXIT as u8,
    );
    code.extend_from_slice(&exit_sys.to_le_bytes());
    let addi = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 9);
    code.extend_from_slice(&addi.to_le_bytes());
    code.extend_from_slice(&HALT);
    let prog = assemble(&code);
    vm.load_program(&prog).unwrap();
    vm.run().expect("exit syscall should halt");
    assert_eq!(vm.register(10), 5);
}

#[test]
fn syscall_abort_marks_failure() {
    let mut vm = IVM::new(u64::MAX);
    let prog = assemble_syscall(syscalls::SYSCALL_ABORT as u8);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::AssertionFailed)));
}

#[test]
fn debug_log_accepts_json_tlv() {
    let mut vm = IVM::new(u64::MAX);
    let payload = br#"{"msg":"hello"}"#;
    let tlv = make_tlv(PointerType::Json as u16, payload);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);
    let prog = assemble_syscall(syscalls::SYSCALL_DEBUG_LOG as u8);
    vm.load_program(&prog).unwrap();
    vm.run().expect("debug log should succeed");
}

struct AddHost;

impl ivm::IVMHost for AddHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if number == 1 {
            let a0 = vm.register(10);
            let a1 = vm.register(11);
            vm.set_register(10, a0.wrapping_add(a1));
            Ok(0)
        } else {
            Err(VMError::UnknownSyscall(number))
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn downcast_add_host() {
    let mut host: Box<dyn ivm::IVMHost> = Box::new(AddHost);
    assert!(host.as_any().downcast_mut::<AddHost>().is_some());
}

#[test]
fn test_custom_syscall() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(AddHost);
    vm.set_register(10, 5);
    vm.set_register(11, 7);
    let prog = assemble_syscall(1);
    vm.load_program(&prog).unwrap();
    vm.run().expect("custom syscall failed");
    assert_eq!(vm.register(10), 12);
}

#[test]
fn removed_hw_probe_syscalls_reject() {
    let mut vm = IVM::new(u64::MAX);
    let prog = assemble_syscall(0xF2);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(VMError::UnknownSyscall(0xF2))));

    let mut vm2 = IVM::new(u64::MAX);
    let prog2 = assemble_syscall(0xF3);
    vm2.load_program(&prog2).unwrap();
    let res2 = vm2.run();
    assert!(matches!(res2, Err(VMError::UnknownSyscall(0xF3))));

    let mut vm3 = IVM::new(u64::MAX);
    let prog3 = assemble_syscall(0xF8);
    vm3.load_program(&prog3).unwrap();
    let res3 = vm3.run();
    assert!(matches!(res3, Err(VMError::UnknownSyscall(0xF8))));
}

#[test]
fn test_verify_proof_syscall() {
    use common::assemble_zk;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        &encoding::wide::encode_rr(instruction::wide::zk::ASSERT, 0, 1, 0).to_le_bytes(),
    );
    let syscall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_VERIFY_PROOF as u8,
    );
    bytes.extend_from_slice(&syscall.to_le_bytes());
    bytes.extend_from_slice(&HALT);
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 0);
    vm.load_program(&prog).unwrap();
    vm.run().expect("verify proof syscall failed");
    assert_eq!(vm.register(10), 1);
}

#[test]
fn test_prove_execution_syscall() {
    let mut vm = IVM::new(u64::MAX);
    let prog = assemble_syscall(syscalls::SYSCALL_PROVE_EXECUTION as u8);
    vm.load_program(&prog).unwrap();
    vm.run().expect("prove execution syscall failed");
}

#[test]
fn test_get_merkle_path_syscall() {
    let mut vm = IVM::new(u64::MAX);
    let addr = ivm::Memory::HEAP_START;
    vm.memory.store_u32(addr, 0xABCD).unwrap();
    vm.memory.commit();
    vm.set_register(10, addr);
    vm.set_register(11, ivm::Memory::OUTPUT_START);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_MERKLE_PATH as u8);
    vm.load_program(&prog).unwrap();
    let expected = vm.memory.merkle_path(addr);
    vm.run().expect("merkle path syscall failed");
    let len = vm.register(10) as usize;
    assert_eq!(len, expected.len());
    let mut out = vec![0u8; expected.len() * 32];
    vm.memory
        .load_bytes(ivm::Memory::OUTPUT_START, &mut out)
        .unwrap();
    for (i, node) in expected.iter().enumerate() {
        assert_eq!(&out[i * 32..(i + 1) * 32], node);
    }
}

#[test]
fn test_get_merkle_path_with_root_syscall() {
    let mut vm = IVM::new(u64::MAX);
    let addr = ivm::Memory::HEAP_START;
    vm.memory.store_u64(addr, 0x0123_4567_89AB_CDEF).unwrap();
    vm.memory.commit();
    let root_out = ivm::Memory::OUTPUT_START + 1024;
    vm.set_register(10, addr);
    vm.set_register(11, ivm::Memory::OUTPUT_START);
    vm.set_register(12, root_out);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_MERKLE_PATH as u8);
    vm.load_program(&prog).unwrap();
    let expected_root = vm.memory.current_root();
    vm.run().expect("merkle path syscall failed");
    let mut out_root = [0u8; 32];
    vm.memory.load_bytes(root_out, &mut out_root).unwrap();
    assert_eq!(out_root, *expected_root.as_ref());
}

#[test]
fn test_get_merkle_compact_syscall() {
    let mut vm = IVM::new(u64::MAX);
    let addr = ivm::Memory::HEAP_START + 64; // pick a different leaf
    vm.memory.store_u32(addr, 0xAAAA5555).unwrap();
    vm.memory.commit();
    let out = ivm::Memory::OUTPUT_START;
    let root_out = ivm::Memory::OUTPUT_START + 2048;
    vm.set_register(10, addr);
    vm.set_register(11, out);
    vm.set_register(12, 16); // cap depth to 16
    vm.set_register(13, root_out);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_MERKLE_COMPACT as u8);
    vm.load_program(&prog).unwrap();
    let path = vm.memory.merkle_path(addr);
    let root = vm.memory.current_root();
    vm.run().expect("merkle compact syscall failed");
    // Parse header
    let mut hdr = [0u8; 1 + 4 + 4];
    vm.memory.load_bytes(out, &mut hdr).unwrap();
    let depth = hdr[0] as usize;
    let dirs = u32::from_le_bytes(hdr[1..5].try_into().unwrap());
    let count = u32::from_le_bytes(hdr[5..9].try_into().unwrap()) as usize;
    assert_eq!(depth, count);
    assert!(depth <= 16 && depth <= path.len());
    // Check siblings equal to truncated path
    let mut sibs = vec![0u8; depth * 32];
    vm.memory
        .load_bytes(out + (1 + 4 + 4) as u64, &mut sibs)
        .unwrap();
    for i in 0..depth {
        assert_eq!(&sibs[i * 32..(i + 1) * 32], &path[i]);
    }
    // Check root
    let mut out_root = [0u8; 32];
    vm.memory.load_bytes(root_out, &mut out_root).unwrap();
    let leaf_idx = (addr / 32) as u32;
    let mask = (1u64 << depth) - 1;
    assert_eq!(dirs as u64, (leaf_idx as u64) & mask);
    let siblings: Vec<Option<HashOf<[u8; 32]>>> = path
        .iter()
        .take(depth)
        .map(|b| {
            if *b == [0u8; 32] {
                None
            } else {
                Some(HashOf::from_untyped_unchecked(Hash::prehashed(*b)))
            }
        })
        .collect();
    let proof = MerkleProof::from_audit_path(leaf_idx, siblings);
    let mut chunk = [0u8; 32];
    vm.memory.load_bytes((addr / 32) * 32, &mut chunk).unwrap();
    let leaf_digest = ivm::merkle_utils::compute_memory_leaf_digest(&chunk);
    let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
    let expected_root = if depth < path.len() {
        proof.compute_root_sha256(&leaf_hash, depth).unwrap_or(root)
    } else {
        root
    };
    assert_eq!(out_root, *expected_root.as_ref());
    // Basic sanity on dirs: at least has bits set within range
    assert_eq!(dirs >> depth, 0);
}

#[test]
fn test_get_merkle_path_syscall_out_of_bounds() {
    let mut vm = IVM::new(u64::MAX);
    let max_addr = vm
        .memory
        .stack_top()
        .saturating_add(ivm::Memory::STACK_SLOP);
    vm.set_register(10, max_addr);
    vm.set_register(11, ivm::Memory::OUTPUT_START);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_MERKLE_PATH as u8);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(VMError::MemoryOutOfBounds)));
}

#[test]
fn test_get_merkle_compact_syscall_out_of_bounds() {
    let mut vm = IVM::new(u64::MAX);
    let max_addr = vm
        .memory
        .stack_top()
        .saturating_add(ivm::Memory::STACK_SLOP);
    let out = ivm::Memory::OUTPUT_START;
    let root_out = out + 2048;
    vm.set_register(10, max_addr);
    vm.set_register(11, out);
    vm.set_register(12, 0);
    vm.set_register(13, root_out);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_MERKLE_COMPACT as u8);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(VMError::MemoryOutOfBounds)));
}

#[test]
fn test_get_register_merkle_compact_syscall_out_of_bounds() {
    let mut vm = IVM::new(u64::MAX);
    let out = ivm::Memory::OUTPUT_START;
    let root_out = out + 2048;
    vm.set_register(10, ivm::parallel::REGISTER_COUNT as u64);
    vm.set_register(11, out);
    vm.set_register(12, 0);
    vm.set_register(13, root_out);
    let prog = assemble_syscall(syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT as u8);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(VMError::RegisterOutOfBounds)));
}
