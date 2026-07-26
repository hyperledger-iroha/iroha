#![cfg(feature = "ivm_zk_tests")]
use ivm::{
    Memory,
    halo2::{LoadCircuit, StoreCircuit, VectorLoadCircuit, VectorStoreCircuit},
};

#[test]
fn test_load_circuit_ok() {
    let mut mem = Memory::new(0);
    let addr = Memory::HEAP_START;
    mem.store_u32(addr, 0x11223344).unwrap();
    mem.commit();
    let root = *mem.root().as_ref();
    let path = mem.merkle_path(addr);
    let mut leaf = [0u8; 32];
    leaf[..4].copy_from_slice(&0x11223344u32.to_le_bytes());
    let c = LoadCircuit {
        root,
        addr,
        value: 0x11223344,
        size: 4,
        leaf,
        path,
        code_len: 0,
        heap_limit: Memory::HEAP_SIZE,
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_store_circuit_updates_root() {
    let mut mem = Memory::new(0);
    let addr = Memory::HEAP_START;
    let path = mem.merkle_path(addr);
    let root_before = *mem.root().as_ref();
    let old_leaf = [0u8; 32];
    mem.store_u32(addr, 0xdeadbeef).unwrap();
    mem.commit();
    let root_after = *mem.root().as_ref();
    let c = StoreCircuit {
        root_before,
        root_after,
        addr,
        value: 0xdeadbeefu128,
        size: 4,
        old_leaf,
        path,
        code_len: 0,
        heap_limit: Memory::HEAP_SIZE,
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_store_circuit_permission_violation() {
    let mut mem = Memory::new(8);
    let addr = 0; // code region
    let path = mem.merkle_path(addr);
    let root_before = *mem.root().as_ref();
    let old_leaf = [0u8; 32];
    // root_after unused but set
    let c = StoreCircuit {
        root_before,
        root_after: root_before,
        addr,
        value: 1,
        size: 4,
        old_leaf,
        path,
        code_len: 8,
        heap_limit: Memory::HEAP_SIZE,
    };
    assert!(c.verify().is_err());
}

#[test]
fn test_vector_load_store_circuit() {
    let mut mem = Memory::new(0);
    let addr = Memory::HEAP_START;
    let mut leaf = [0u8; 32];
    let data = [1u8; 16];
    mem.store_bytes(addr, &data).unwrap();
    mem.commit();
    let root_after_store = *mem.root().as_ref();
    let path = mem.merkle_path(addr);
    leaf[..16].copy_from_slice(&data);
    let load = VectorLoadCircuit {
        inner: LoadCircuit {
            root: root_after_store,
            addr,
            value: u128::from_le_bytes(data.clone()),
            size: 16,
            leaf,
            path: path.clone(),
            code_len: 0,
            heap_limit: Memory::HEAP_SIZE,
        },
    };
    assert!(load.verify().is_ok());
}

#[test]
fn test_vector_store_circuit_ok() {
    let mut mem = Memory::new(0);
    let addr = Memory::HEAP_START;
    let path = mem.merkle_path(addr);
    let root_before = *mem.root().as_ref();
    let old_leaf = [0u8; 32];
    let data = [0xABu8; 16];
    mem.store_bytes(addr, &data).unwrap();
    mem.commit();
    let root_after = *mem.root().as_ref();
    let store = VectorStoreCircuit {
        inner: StoreCircuit {
            root_before,
            root_after,
            addr,
            value: u128::from_le_bytes(data),
            size: 16,
            old_leaf,
            path,
            code_len: 0,
            heap_limit: Memory::HEAP_SIZE,
        },
    };
    assert!(store.verify().is_ok());
}

#[test]
fn test_vector_store_circuit_invalid_size() {
    let bad = VectorStoreCircuit {
        inner: StoreCircuit {
            root_before: [0u8; 32],
            root_after: [0u8; 32],
            addr: Memory::HEAP_START,
            value: 0,
            size: 8,
            old_leaf: [0u8; 32],
            path: Vec::new(),
            code_len: 0,
            heap_limit: Memory::HEAP_SIZE,
        },
    };
    assert!(bad.verify().is_err());
}
