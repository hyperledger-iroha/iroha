//! Ensure AES wide instructions execute without decode faults.

use ivm::{IVM, ProgramMetadata, aesdec, aesenc, encoding, instruction, ivm_mode};

const STATE_LO: u64 = 0x7766_5544_3322_1100;
const STATE_HI: u64 = 0xffee_ddcc_bbaa_9988;
const KEY_LO: u64 = 0xc971_150f_59e8_d947;
const KEY_HI: u64 = 0x9867_7faf_d6ad_b70c;

fn assemble_single(op: u8) -> Vec<u8> {
    let mut metadata = ProgramMetadata::default();
    metadata.mode |= ivm_mode::VECTOR;
    let mut bytes = metadata.encode();
    bytes.extend_from_slice(&encoding::wide::encode_rr(op, 5, 1, 3).to_le_bytes());
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}

fn run_single(op: u8) -> [u8; 16] {
    let program = assemble_single(op);
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&program).expect("program load");
    vm.set_register(1, STATE_LO);
    vm.set_register(2, STATE_HI);
    vm.set_register(3, KEY_LO);
    vm.set_register(4, KEY_HI);
    vm.run().expect("vm run");
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&vm.register(5).to_le_bytes());
    out[8..].copy_from_slice(&vm.register(6).to_le_bytes());
    out
}

fn state_bytes() -> [u8; 16] {
    let mut state = [0u8; 16];
    state[..8].copy_from_slice(&STATE_LO.to_le_bytes());
    state[8..].copy_from_slice(&STATE_HI.to_le_bytes());
    state
}

fn key_bytes() -> [u8; 16] {
    let mut key = [0u8; 16];
    key[..8].copy_from_slice(&KEY_LO.to_le_bytes());
    key[8..].copy_from_slice(&KEY_HI.to_le_bytes());
    key
}

#[test]
fn vm_executes_wide_aesenc_round() {
    let cpu = aesenc(state_bytes(), key_bytes());
    let vm_out = run_single(instruction::wide::crypto::AESENC);
    assert_eq!(vm_out, cpu);
}

#[test]
fn vm_executes_wide_aesdec_round() {
    let cpu = aesdec(state_bytes(), key_bytes());
    let vm_out = run_single(instruction::wide::crypto::AESDEC);
    assert_eq!(vm_out, cpu);
}
