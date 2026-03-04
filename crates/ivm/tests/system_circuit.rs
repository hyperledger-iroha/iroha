#![cfg(feature = "ivm_zk_tests")]
use ivm::{
    Memory,
    halo2::{AllocCircuit, GetGasCircuit},
};

#[test]
fn test_alloc_circuit_ok() {
    let c = AllocCircuit {
        heap_alloc_before: 0,
        heap_limit_before: Memory::HEAP_MAX_SIZE,
        size: 16,
        heap_alloc_after: 16,
        addr: Memory::HEAP_START,
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_alloc_circuit_oob() {
    let c = AllocCircuit {
        heap_alloc_before: Memory::HEAP_MAX_SIZE - 8,
        heap_limit_before: Memory::HEAP_MAX_SIZE,
        size: 16,
        heap_alloc_after: Memory::HEAP_MAX_SIZE - 8 + 16,
        addr: Memory::HEAP_START + Memory::HEAP_MAX_SIZE - 8,
    };
    assert!(c.verify().is_err());
}

#[test]
fn test_getgas_circuit() {
    let c = GetGasCircuit {
        initial_gas: 100,
        gas_used: 10,
        reported: 90,
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_getgas_circuit_mismatch() {
    let c = GetGasCircuit {
        initial_gas: 50,
        gas_used: 5,
        reported: 60,
    };
    assert!(c.verify().is_err());
}
