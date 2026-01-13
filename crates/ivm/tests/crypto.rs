//! Crypto opcode and helper regression tests.

use ivm::{
    AccelerationConfig, ByteMerkleTree, IVM, Memory, SimdChoice, acceleration_runtime_status,
    ec::{ec_add_truncated, ec_mul_truncated},
    encoding, field, instruction, pairing_check_truncated, poseidon2, poseidon6, vector_supported,
};
mod common;
use std::sync::{Mutex, MutexGuard, OnceLock};

use common::assemble;

const HALT_WORD: u32 = encoding::wide::encode_halt();

fn words(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for &word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

struct AccelConfigGuard {
    _lock: MutexGuard<'static, ()>,
    original: AccelerationConfig,
}

impl AccelConfigGuard {
    fn new() -> Self {
        fn accel_test_lock() -> &'static Mutex<()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            LOCK.get_or_init(|| Mutex::new(()))
        }
        let lock = accel_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self {
            _lock: lock,
            original: ivm::acceleration_config(),
        }
    }
}

impl Drop for AccelConfigGuard {
    fn drop(&mut self) {
        ivm::set_acceleration_config(self.original);
    }
}

fn run_poseidon2_program(a: u64, b: u64) -> u64 {
    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.set_register(1, base);
    let instr = encoding::wide::encode_load(instruction::wide::crypto::POSEIDON2, 2, 1, 0);
    let prog = assemble(&words(&[instr, HALT_WORD]));
    vm.load_program(&prog).unwrap();
    vm.store_u64(base, a).unwrap();
    vm.store_u64(base + 8, b).unwrap();
    vm.run().unwrap();
    vm.register(2)
}

fn run_poseidon6_program(vals: [u64; 6]) -> u64 {
    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.set_register(1, base);
    let instr = encoding::wide::encode_load(instruction::wide::crypto::POSEIDON6, 2, 1, 0);
    let prog = assemble(&words(&[instr, HALT_WORD]));
    vm.load_program(&prog).unwrap();
    for (idx, val) in vals.iter().enumerate() {
        vm.store_u64(base + 8 * idx as u64, *val).unwrap();
    }
    vm.run().unwrap();
    vm.register(2)
}

#[test]
fn test_poseidon2() {
    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.set_register(1, base);
    let instr = encoding::wide::encode_load(instruction::wide::crypto::POSEIDON2, 2, 1, 0);
    let prog = assemble(&words(&[instr, HALT_WORD]));
    vm.load_program(&prog).unwrap();
    vm.store_u64(base, 5).unwrap();
    vm.store_u64(base + 8, 7).unwrap();
    vm.run().unwrap();
    let expected = poseidon2(5, 7);
    assert_eq!(vm.register(2), expected);
}

#[test]
fn test_poseidon6() {
    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.set_register(1, base);
    let instr = encoding::wide::encode_load(instruction::wide::crypto::POSEIDON6, 2, 1, 0);
    let prog = assemble(&words(&[instr, HALT_WORD]));
    vm.load_program(&prog).unwrap();
    for i in 0..6u64 {
        vm.store_u64(base + 8 * i, i + 1).unwrap();
    }
    vm.run().unwrap();
    let mut vals = [0u64; 6];
    for i in 0..6u64 {
        vals[i as usize] = i + 1;
    }
    let expected = poseidon6(vals);
    assert_eq!(vm.register(2), expected);
}

#[test]
fn poseidon_instructions_match_across_acceleration_configs() {
    let guard = AccelConfigGuard::new();
    let inputs2 = (11u64, 17u64);
    let inputs6 = [1u64, 2, 3, 4, 5, 6];

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let cpu_poseidon2 = run_poseidon2_program(inputs2.0, inputs2.1);
    let cpu_poseidon6 = run_poseidon6_program(inputs6);
    let status_cpu = acceleration_runtime_status();
    assert!(
        !status_cpu.cuda.configured,
        "CUDA should be marked disabled in CPU-only config"
    );
    assert!(
        !status_cpu.metal.configured,
        "Metal should be marked disabled in CPU-only config"
    );

    ivm::set_acceleration_config(AccelerationConfig {
        enable_cuda: true,
        enable_metal: true,
        ..guard.original
    });
    let accel_poseidon2 = run_poseidon2_program(inputs2.0, inputs2.1);
    let accel_poseidon6 = run_poseidon6_program(inputs6);
    let status_accel = acceleration_runtime_status();
    assert!(
        status_accel.cuda.configured,
        "CUDA should be marked enabled when acceleration is allowed"
    );
    assert!(
        status_accel.metal.configured,
        "Metal should be marked enabled when acceleration is allowed"
    );

    assert_eq!(accel_poseidon2, cpu_poseidon2);
    assert_eq!(accel_poseidon6, cpu_poseidon6);

    #[cfg(feature = "cuda")]
    {
        if ivm::cuda_available() {
            if let Some(cuda_poseidon2) = ivm::poseidon2_cuda(inputs2.0, inputs2.1) {
                assert_eq!(cuda_poseidon2, accel_poseidon2);
            }
            if let Some(cuda_poseidon6) = ivm::poseidon6_cuda(inputs6) {
                assert_eq!(cuda_poseidon6, accel_poseidon6);
            }
        }
    }
}

#[test]
fn simd_disable_forces_scalar_without_affecting_outputs() {
    let guard = AccelConfigGuard::new();
    let inputs = (3u64, 9u64);

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        ..guard.original
    });
    let scalar_status = acceleration_runtime_status();
    assert!(
        !scalar_status.simd.configured,
        "SIMD backend should report disabled when enable_simd = false"
    );
    let scalar_poseidon = run_poseidon2_program(inputs.0, inputs.1);

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        ..guard.original
    });
    let simd_status = acceleration_runtime_status();
    assert!(
        simd_status.simd.configured,
        "SIMD backend should report configured when re-enabled"
    );
    let simd_poseidon = run_poseidon2_program(inputs.0, inputs.1);

    assert_eq!(
        scalar_poseidon, simd_poseidon,
        "Disabling SIMD must not change cryptographic outputs"
    );
}

#[test]
fn merkle_roots_match_across_simd_modes() {
    let guard = AccelConfigGuard::new();
    let payload: Vec<u8> = (0..(32 * 128))
        .map(|i| (i as u8).wrapping_mul(13).wrapping_add(7))
        .collect();

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let simd_status = acceleration_runtime_status();
    assert!(simd_status.simd.configured);
    assert_eq!(simd_status.simd.available, vector_supported());
    let simd_root = ByteMerkleTree::from_bytes_accel(&payload, 32).root();

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let scalar_status = acceleration_runtime_status();
    assert!(!scalar_status.simd.configured);
    assert_eq!(ivm::simd_choice(), SimdChoice::Scalar);
    assert!(!scalar_status.simd.available);
    let scalar_root = ByteMerkleTree::from_bytes_accel(&payload, 32).root();

    assert_eq!(scalar_root, simd_root);
}

#[test]
fn test_pubkgen_valcom_ecops() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 9); // scalar/value
    vm.set_register(2, 4); // random

    // PUBKGEN r3, r1
    let pubk = encoding::wide::encode_rr(instruction::wide::crypto::PUBKGEN, 3, 1, 0);
    // VALCOM r4, r1, r2
    let valcom = encoding::wide::encode_rr(instruction::wide::crypto::VALCOM, 4, 1, 2);
    // ECADD r5, r3, r4
    let ecadd = encoding::wide::encode_rr(instruction::wide::crypto::ECADD, 5, 3, 4);
    // ECMUL_VAR r6, r3, r1
    let ecmul = encoding::wide::encode_rr(instruction::wide::crypto::ECMUL_VAR, 6, 3, 1);

    let mut seq = Vec::new();
    for ins in [pubk, valcom, ecadd, ecmul] {
        seq.extend_from_slice(&ins.to_le_bytes());
    }
    seq.extend_from_slice(&HALT_WORD.to_le_bytes());
    let prog = assemble(&seq);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let expected_pubk = field::mul(9, 2);
    assert_eq!(vm.register(3), expected_pubk);
    let expected_valcom = ivm::pedersen_commit_truncated(9, 4);
    assert_eq!(vm.register(4), expected_valcom);
    let expected_ecadd = ec_add_truncated(expected_pubk, expected_valcom);
    assert_eq!(vm.register(5), expected_ecadd);
    let expected_ecmul = ec_mul_truncated(expected_pubk, 9);
    assert_eq!(vm.register(6), expected_ecmul);
}

#[test]
fn sha256_instruction() {
    use sha2::{Digest, Sha256};
    let mut vm = IVM::new(u64::MAX);
    let addr = Memory::HEAP_START;
    vm.memory.store_bytes(addr, b"abc").unwrap();
    let instr = ivm::simple_instruction::Instruction::Sha256 {
        dest: 1,
        src_addr: addr,
        len: 3,
    };
    vm.execute_instruction(instr).unwrap();
    let digest = Sha256::digest(b"abc");
    for i in 0..4 {
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&digest[i * 8..(i + 1) * 8]);
        let expected = u64::from_le_bytes(chunk);
        assert_eq!(vm.register(i + 1), expected);
    }
}

#[test]
fn ed25519_verify_instruction() {
    use ed25519_dalek::{Signer, SigningKey};
    let secret = [0x42u8; 32];
    let keypair = SigningKey::from_bytes(&secret);
    let msg = b"hello";
    let sig = keypair.sign(msg);
    let pubkey = keypair.verifying_key();

    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.memory.store_bytes(base, pubkey.as_bytes()).unwrap();
    vm.memory
        .store_bytes(base + 32, sig.to_bytes().as_slice())
        .unwrap();
    vm.memory.store_bytes(base + 96, msg).unwrap();

    let instr = ivm::simple_instruction::Instruction::Ed25519Verify {
        pubkey_addr: base,
        sig_addr: base + 32,
        msg_addr: base + 96,
        msg_len: msg.len() as u64,
        result_reg: 0,
    };
    vm.execute_instruction(instr).unwrap();
    assert_eq!(vm.register(0), 1);

    // tamper message
    vm.memory.store_u8(base + 96, 0xFF).unwrap();
    let instr2 = ivm::simple_instruction::Instruction::Ed25519Verify {
        pubkey_addr: base,
        sig_addr: base + 32,
        msg_addr: base + 96,
        msg_len: msg.len() as u64,
        result_reg: 1,
    };
    vm.execute_instruction(instr2).unwrap();
    assert_eq!(vm.register(1), 0);
}

#[test]
fn dilithium_verify_instruction() {
    use pqcrypto_dilithium::dilithium2;
    use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};

    let (pk, sk) = dilithium2::keypair();
    let msg = b"hello";
    let sig = dilithium2::detached_sign(msg, &sk);

    let mut vm = IVM::new(u64::MAX);
    let base = Memory::HEAP_START;
    vm.memory.store_bytes(base, pk.as_bytes()).unwrap();
    vm.memory
        .store_bytes(base + dilithium2::public_key_bytes() as u64, sig.as_bytes())
        .unwrap();
    vm.memory.store_bytes(base + 4096, msg).unwrap();

    let instr = ivm::simple_instruction::Instruction::DilithiumVerify {
        level: 2,
        pubkey_addr: base,
        sig_addr: base + dilithium2::public_key_bytes() as u64,
        msg_addr: base + 4096,
        msg_len: msg.len() as u64,
        result_reg: 0,
    };
    vm.execute_instruction(instr).unwrap();
    assert_eq!(vm.register(0), 1);

    // tamper message
    vm.memory.store_u8(base + 4096, 0xFF).unwrap();
    let instr2 = ivm::simple_instruction::Instruction::DilithiumVerify {
        level: 2,
        pubkey_addr: base,
        sig_addr: base + dilithium2::public_key_bytes() as u64,
        msg_addr: base + 4096,
        msg_len: msg.len() as u64,
        result_reg: 1,
    };
    vm.execute_instruction(instr2).unwrap();
    assert_eq!(vm.register(1), 0);
}

#[test]
fn test_pairing_opcode() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 2);
    vm.set_register(2, 3);
    let instr = encoding::wide::encode_rr(instruction::wide::crypto::PAIRING, 3, 1, 2);
    let prog = assemble(&words(&[instr, HALT_WORD]));
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    let expected = pairing_check_truncated(2, 3);
    assert_eq!(vm.register(3), expected);
}
