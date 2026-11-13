//! Native memory request obligations for the unfinished constrained machine relation.
//!
//! These execute the real interpreter, not a diagnostic packet list. They grant
//! no AIR linkage, initialization authority, frame ownership or proof admission.
use ivm::{
    IVM, Memory, Perm, ProgramMetadata, VMError, encoding::wide as encode, instruction::wide,
    ivm_mode,
};

const LOW: u64 = 0x0123_4567_89ab_cdef;
const HIGH: u64 = 0xfedc_ba98_7654_3210;

fn machine(words: &[u32], mode: u8, gas: u64) -> IVM {
    machine_with_max_cycles(words, mode, gas, 2)
}

fn machine_with_max_cycles(words: &[u32], mode: u8, gas: u64, max_cycles: u64) -> IVM {
    // Only ZK headers carry a trace bound.
    let max_cycles = if mode & ivm_mode::ZK != 0 { max_cycles } else { 0 };
    let mut program = ProgramMetadata {
        mode,
        max_cycles,
        ..ProgramMetadata::default()
    }
    .encode();
    for word in words {
        program.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(gas);
    vm.load_program(&program).unwrap();
    vm
}

fn registers(vm: &IVM) -> ([u64; ivm::parallel::REGISTER_COUNT], [bool; ivm::parallel::REGISTER_COUNT]) {
    (
        core::array::from_fn(|i| vm.registers.get(i)),
        core::array::from_fn(|i| vm.registers.tag(i)),
    )
}

#[test]
fn memory_trap_priority_preserves_architecture_after_exact_gas_debit() {
    let s64 = encode::encode_store(wide::memory::STORE64, 3, 1, 0);
    let l64 = encode::encode_load(wide::memory::LOAD64, 1, 3, 0);
    let cases = [
        (s64, u64::MAX, VMError::PrivacyViolation, 3),
        (
            l64,
            u64::MAX,
            VMError::MisalignedAccess { addr: u32::MAX },
            3,
        ),
    ];
    for (word, address, expected_error, cost) in cases {
        for insufficient_gas in [false, true] {
            let gas = if insufficient_gas { cost - 1 } else { 100 };
            let mut vm = machine(&[word, encode::encode_halt()], ivm_mode::ZK, gas);
            vm.set_register(1, LOW);
            vm.set_register(2, HIGH);
            vm.set_register(3, address);
            let before = registers(&vm);
            let result = vm.run();
            let error = result.expect_err("the invalid request must fail before publication");
            if insufficient_gas {
                assert!(matches!(error, VMError::OutOfGas));
            } else {
                match (&error, &expected_error) {
                    (VMError::PrivacyViolation, VMError::PrivacyViolation) => {}
                    (
                        VMError::MisalignedAccess { addr },
                        VMError::MisalignedAccess { addr: expected },
                    ) => assert_eq!(addr, expected),
                    _ => panic!("request returned {error:?}, expected {expected_error:?}"),
                }
            }
            assert_eq!(registers(&vm), before);
            assert_eq!((vm.pc(), vm.get_cycle_count()), (0, 0));
            assert_eq!(
                vm.gas_remaining,
                if insufficient_gas { gas } else { gas - cost }
            );
            assert_eq!(vm.memory.load_u128(Memory::HEAP_START).unwrap(), 0);
            assert_eq!(vm.memory.output_used_len(), 0);
        }
    }
}

#[test]
fn public_half_store_preserves_other_half_private_tag() {
    for half in [0_i16, 8] {
        let words = [
            encode::encode_store(wide::memory::STORE64, 3, 1, 0),
            encode::encode_halt(),
            encode::encode_store(wide::memory::STORE64, 3, 2, 8),
            encode::encode_halt(),
            encode::encode_store(wide::memory::STORE64, 3, 4, half),
            encode::encode_halt(),
            encode::encode_load(wide::memory::LOAD64, 5, 3, 0),
            encode::encode_halt(),
            encode::encode_load(wide::memory::LOAD64, 6, 3, 8),
            encode::encode_halt(),
        ];
        let mut vm = machine(&words, ivm_mode::ZK, 1_000);
        vm.set_register(1, LOW);
        vm.set_register(2, HIGH);
        vm.set_register(3, Memory::STACK_START);
        vm.set_register(4, 7);
        vm.registers.set_tag(1, true);
        vm.registers.set_tag(2, true);
        vm.run().unwrap();
        for segment in [8, 16, 24, 32] {
            vm.set_program_counter(segment).unwrap();
            vm.run().unwrap();
        }
        assert_eq!(vm.registers.get(5), if half == 0 { 7 } else { LOW });
        assert_eq!(vm.registers.get(6), if half == 8 { 7 } else { HIGH });
        assert_eq!(vm.registers.tag(5), half != 0);
        assert_eq!(vm.registers.tag(6), half != 8);
    }
}

#[test]
fn output_forward_gap_is_preserved_and_rewind_traps_without_mutation() {
    let store = encode::encode_store(wide::memory::STORE64, 3, 1, 0);
    let mut vm = machine(
        &[store, encode::encode_halt(), store, encode::encode_halt()],
        ivm_mode::ZK,
        100,
    );
    vm.set_register(1, LOW);
    vm.set_register(3, Memory::OUTPUT_START + 16);
    vm.run().unwrap();
    assert_eq!(vm.memory.output_used_len(), 24);
    assert_eq!(&vm.memory.read_output_used()[..16], &[0; 16]);
    assert_eq!(&vm.memory.read_output_used()[16..], &LOW.to_le_bytes());
    let original = vm.memory.read_output_used().to_vec();
    vm.set_register(3, Memory::OUTPUT_START + 8);
    vm.set_program_counter(8).unwrap();
    let before = registers(&vm);
    let gas = vm.gas_remaining;
    assert!(matches!((&(vm.run()), &(VMError::MemoryAccessViolation {
            addr: (Memory::OUTPUT_START + 8) as u32,
            perm: Perm::WRITE
        })), (Err(VMError::MemoryAccessViolation { addr: __actual_addr, perm: __actual_perm }), VMError::MemoryAccessViolation { addr: __expected_addr, perm: __expected_perm }) if __actual_addr == __expected_addr && __actual_perm == __expected_perm));
    assert_eq!(vm.memory.read_output_used(), original.as_slice());
    assert_eq!(vm.memory.output_used_len(), 24);
    assert_eq!(registers(&vm), before);
    assert_eq!(vm.pc(), 8);
    assert_eq!(vm.gas_remaining, gas - 3);
}

#[test]
fn signed_immediate_wrap_resolves_the_address_before_region_checks() {
    for mode in [0, ivm_mode::ZK] {
        for (base, immediate, address) in [
            (Memory::HEAP_START + 128, -128_i16, Memory::HEAP_START),
            (Memory::HEAP_START - 127, 127, Memory::HEAP_START),
            (u64::MAX, 1, 0),
            (u64::MAX - 7, 8, 0),
        ] {
            let load = encode::encode_load(wide::memory::LOAD64, 1, 3, immediate);
            let mut vm = machine(&[load, encode::encode_halt()], mode, 100);
            vm.memory.store_u64(Memory::HEAP_START, LOW).unwrap();
            vm.set_register(3, base);
            let expected_value = vm.memory.load_u64(address).unwrap();
            let (mut expected, tags) = registers(&vm);
            expected[1] = expected_value;
            vm.run().unwrap();
            assert_eq!(registers(&vm), (expected, tags));
            assert_eq!(vm.memory.load_u64(address).unwrap(), expected_value);
            assert_eq!(vm.gas_remaining, 97);
            assert_eq!((vm.pc(), vm.get_cycle_count()), (8, 2));
        }
        let store = encode::encode_store(wide::memory::STORE64, 3, 1, 1);
        let mut vm = machine(&[store, encode::encode_halt()], mode, 100);
        vm.set_register(1, LOW);
        vm.set_register(3, u64::MAX);
        let before = registers(&vm);
        let code = vm.memory.load_u64(0).unwrap();
        assert!(matches!((&(vm.run()), &(VMError::MemoryAccessViolation {
                addr: 0,
                perm: Perm::WRITE
            })), (Err(VMError::MemoryAccessViolation { addr: __actual_addr, perm: __actual_perm }), VMError::MemoryAccessViolation { addr: __expected_addr, perm: __expected_perm }) if __actual_addr == __expected_addr && __actual_perm == __expected_perm));
        assert_eq!(registers(&vm), before);
        assert_eq!(vm.memory.load_u64(0).unwrap(), code);
        assert_eq!(vm.gas_remaining, 97);
        assert_eq!((vm.pc(), vm.get_cycle_count()), (0, 0));
    }
}

#[test]
fn zk_padding_gas_failure_preserves_completed_memory_effects_and_padded_cycles() {
    let store = encode::encode_store(wide::memory::STORE64, 3, 1, 0);
    for (gas, outcome, remaining) in [(100, Ok(()), 95), (4, Err(VMError::OutOfGas), 1)] {
        let mut vm = machine_with_max_cycles(&[store, encode::encode_halt()], ivm_mode::ZK, gas, 4);
        let address = Memory::HEAP_START + 0x80;
        vm.set_register(1, LOW);
        vm.set_register(3, address);
        let before = registers(&vm);
        assert_eq!(vm.memory.load_u64(address).unwrap(), 0);
        assert!(matches!(
            (vm.run(), outcome),
            (Ok(()), Ok(())) | (Err(VMError::OutOfGas), Err(VMError::OutOfGas))
        ));
        assert_eq!(registers(&vm), before);
        assert_eq!(vm.memory.load_u64(address).unwrap(), LOW);
        assert_eq!(vm.gas_remaining, remaining);
        assert_eq!((vm.pc(), vm.get_cycle_count()), (8, 4));
    }
}
