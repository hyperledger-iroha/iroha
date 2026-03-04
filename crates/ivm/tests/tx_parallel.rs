use ivm::{IVM, Memory, PostRunPhase, Transaction, execute_transactions_parallel};

fn encode_add(rd: u16, rs: u16, rt: u16) -> [u8; 2] {
    let word = ((0x1u16) << 12) | ((rd & 0xf) << 8) | ((rs & 0xf) << 4) | (rt & 0xf);
    word.to_le_bytes()
}
fn encode_load(rd: u16, base: u16) -> [u8; 2] {
    let word = ((0x3u16) << 12) | ((rd & 0xf) << 8) | ((base & 0xf) << 4);
    word.to_le_bytes()
}
fn encode_store(rs: u16, base: u16) -> [u8; 2] {
    let word = ((0x4u16) << 12) | ((rs & 0xf) << 8) | ((base & 0xf) << 4);
    word.to_le_bytes()
}
fn encode_halt() -> [u8; 2] {
    0u16.to_le_bytes()
}

fn build_increment_tx(id: usize, addr: u64) -> Transaction {
    let mut code = Vec::new();
    code.extend_from_slice(&encode_load(1, 3));
    code.extend_from_slice(&encode_add(1, 1, 2));
    code.extend_from_slice(&encode_store(1, 3));
    code.extend_from_slice(&encode_halt());
    let mut ivm = IVM::new(u64::MAX);
    ivm.load_code(&code).unwrap();
    ivm.set_register(2, 1);
    ivm.set_register(3, addr);
    ivm.memory.store_u64(addr, 0).unwrap();
    let base = ivm.clone();
    Transaction {
        id,
        ivm,
        base,
        read_set: Vec::new(),
        write_set: Vec::new(),
        post_run: None,
        result: Ok(()),
    }
}

#[test]
fn parallel_conflict_increments() {
    let addr = Memory::HEAP_START + 0x100;
    let mut txs = vec![build_increment_tx(0, addr), build_increment_tx(1, addr)];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    let val = mem.load_u64(addr).unwrap();
    assert_eq!(val, 2);
}

#[test]
fn parallel_non_conflicting() {
    let addr1 = Memory::HEAP_START + 0x100;
    let addr2 = Memory::HEAP_START + 0x108;
    let mut txs = vec![build_increment_tx(0, addr1), build_increment_tx(1, addr2)];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    assert_eq!(mem.load_u64(addr1).unwrap(), 1);
    assert_eq!(mem.load_u64(addr2).unwrap(), 1);
}

fn build_write_tx(id: usize, addr: u64, value: u64) -> Transaction {
    let mut code = Vec::new();
    code.extend_from_slice(&encode_store(2, 3));
    code.extend_from_slice(&encode_halt());
    let mut ivm = IVM::new(u64::MAX);
    ivm.load_code(&code).unwrap();
    ivm.set_register(2, value);
    ivm.set_register(3, addr);
    ivm.memory.store_u64(addr, 5).unwrap();
    let base = ivm.clone();
    Transaction {
        id,
        ivm,
        base,
        read_set: Vec::new(),
        write_set: Vec::new(),
        post_run: None,
        result: Ok(()),
    }
}

fn build_read_then_write(id: usize, read_addr: u64, write_addr: u64) -> Transaction {
    let mut code = Vec::new();
    code.extend_from_slice(&encode_load(1, 3));
    code.extend_from_slice(&encode_store(1, 4));
    code.extend_from_slice(&encode_halt());
    let mut ivm = IVM::new(u64::MAX);
    ivm.load_code(&code).unwrap();
    ivm.set_register(3, read_addr);
    ivm.set_register(4, write_addr);
    ivm.memory.store_u64(read_addr, 5).unwrap();
    ivm.memory.store_u64(write_addr, 0).unwrap();
    let base = ivm.clone();
    Transaction {
        id,
        ivm,
        base,
        read_set: Vec::new(),
        write_set: Vec::new(),
        post_run: None,
        result: Ok(()),
    }
}

fn build_post_bytes_tx(id: usize, dest: u64, payload: Vec<u8>) -> Transaction {
    let mut code = Vec::new();
    code.extend_from_slice(&encode_halt());
    let mut ivm = IVM::new(u64::MAX);
    ivm.load_code(&code).unwrap();
    let base = ivm.clone();
    let write_payload = payload;
    Transaction {
        id,
        ivm,
        base,
        read_set: Vec::new(),
        write_set: Vec::new(),
        post_run: Some(Box::new(move |vm: &mut IVM, phase: PostRunPhase| {
            if matches!(phase, PostRunPhase::Speculative) {
                vm.memory
                    .store_bytes(dest, &write_payload)
                    .expect("store bytes post-run");
            }
        })),
        result: Ok(()),
    }
}

fn build_post_u32_tx(id: usize, dest: u64, value: u32) -> Transaction {
    let mut code = Vec::new();
    code.extend_from_slice(&encode_halt());
    let mut ivm = IVM::new(u64::MAX);
    ivm.load_code(&code).unwrap();
    let base = ivm.clone();
    Transaction {
        id,
        ivm,
        base,
        read_set: Vec::new(),
        write_set: Vec::new(),
        post_run: Some(Box::new(move |vm: &mut IVM, phase: PostRunPhase| {
            if matches!(phase, PostRunPhase::Speculative) {
                vm.memory
                    .store_u32(dest, value)
                    .expect("store u32 post-run");
            }
        })),
        result: Ok(()),
    }
}

#[test]
fn read_write_conflict_replay() {
    let addr_a = Memory::HEAP_START + 0x200;
    let addr_b = Memory::HEAP_START + 0x208;
    let tx1 = build_write_tx(0, addr_a, 10);
    let tx2 = build_read_then_write(1, addr_a, addr_b);
    let mut txs = vec![tx1, tx2];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    assert_eq!(mem.load_u64(addr_b).unwrap(), 10);
}

#[test]
fn store_bytes_conflict_detected() {
    let dest = Memory::HEAP_START + 0x380;
    let mut txs = vec![
        build_post_bytes_tx(0, dest, vec![1, 2, 3, 4]),
        build_post_bytes_tx(1, dest, vec![9u8; 5]),
    ];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    assert!(
        !txs[0].write_set.is_empty(),
        "tx0 write set empty: {:?}",
        txs[0].write_set
    );
    assert!(
        !txs[1].write_set.is_empty(),
        "tx1 write set empty: {:?}",
        txs[1].write_set
    );
    let mut buf = vec![0u8; 5];
    mem.load_bytes(dest, &mut buf).unwrap();
    assert_eq!(buf, vec![9u8; 5]);
}

#[test]
fn store_u32_conflict_detected() {
    let dest = Memory::HEAP_START + 0x300;
    let mut txs = vec![
        build_post_u32_tx(0, dest, 0x1111_2222),
        build_post_u32_tx(1, dest, 0x3333_4444),
    ];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    assert_eq!(mem.load_u32(dest).unwrap(), 0x3333_4444);
}

#[test]
fn post_run_final_invoked_once_per_tx() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let dest = Memory::HEAP_START + 0x3C0;
    let spec_counter = Arc::new(AtomicUsize::new(0));
    let final_counter = Arc::new(AtomicUsize::new(0));

    let make_tx = |id: usize, value: u32| {
        let mut code = Vec::new();
        code.extend_from_slice(&encode_halt());
        let mut ivm = IVM::new(u64::MAX);
        ivm.load_code(&code).unwrap();
        let base = ivm.clone();
        let spec_counter = Arc::clone(&spec_counter);
        let final_counter = Arc::clone(&final_counter);
        Transaction {
            id,
            ivm,
            base,
            read_set: Vec::new(),
            write_set: Vec::new(),
            post_run: Some(Box::new(
                move |vm: &mut IVM, phase: PostRunPhase| match phase {
                    PostRunPhase::Speculative => {
                        spec_counter.fetch_add(1, Ordering::SeqCst);
                        vm.memory
                            .store_u32(dest, value)
                            .expect("speculative write succeeds");
                    }
                    PostRunPhase::Final => {
                        final_counter.fetch_add(1, Ordering::SeqCst);
                    }
                },
            )),
            result: Ok(()),
        }
    };

    let mut txs = vec![make_tx(0, 0xAAAA5555), make_tx(1, 0xBBBB6666)];
    let mem = execute_transactions_parallel(&mut txs).unwrap();
    assert_eq!(mem.load_u32(dest).unwrap(), 0xBBBB6666);
    assert_eq!(final_counter.load(Ordering::SeqCst), 2);
    assert!(spec_counter.load(Ordering::SeqCst) >= 2);
}
