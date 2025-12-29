use ivm::{IVM, Memory, encoding, instruction, syscalls};
mod common;
use common::assemble;

const HALT: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();

#[test]
fn test_heap_growth_allocation() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(10, 0x20_000); // grow by 128 KB
    vm.set_register(11, 0x14_000); // allocation size 80 KB
    vm.set_register(2, 0xDEAD_BEEF);
    let mut prog = Vec::new();
    // grow heap
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_GROW_HEAP as u8,
        )
        .to_le_bytes(),
    );
    // move alloc size into r10 (`ADD r10 = r11 + r0`)
    prog.extend_from_slice(&ivm::kotodama::wide::encode_add(10, 11, 0).to_le_bytes());
    // alloc
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_ALLOC as u8,
        )
        .to_le_bytes(),
    );
    // copy returned addr to r1 (`ADD r1 = r10 + r0`)
    prog.extend_from_slice(&ivm::kotodama::wide::encode_add(1, 10, 0).to_le_bytes());
    // store r2 at [r1]
    prog.extend_from_slice(&ivm::kotodama::wide::encode_store64(1, 2, 0).to_le_bytes());
    // load into r3 from [r1]
    prog.extend_from_slice(&ivm::kotodama::wide::encode_load64(1, 3, 0).to_le_bytes());
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    assert_eq!(vm.register(3), 0xDEAD_BEEF);
    assert!(vm.memory.heap_limit() >= Memory::HEAP_SIZE + 0x20_000);
}

#[test]
fn test_heap_grow_oob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(10, Memory::HEAP_MAX_SIZE * 2);
    let mut prog = Vec::new();
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_GROW_HEAP as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(ivm::VMError::OutOfMemory)));
}

#[test]
fn test_heap_alloc_exact_limit() {
    let mut vm = IVM::new(u64::MAX);
    // Grow heap to maximum size
    let grow = Memory::HEAP_MAX_SIZE - Memory::HEAP_SIZE;
    vm.set_register(10, grow);
    vm.set_register(11, Memory::HEAP_MAX_SIZE);
    let mut prog = Vec::new();
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_GROW_HEAP as u8,
        )
        .to_le_bytes(),
    );
    // move allocation size into r10 (`ADD r10 = r11 + r0`)
    prog.extend_from_slice(&ivm::kotodama::wide::encode_add(10, 11, 0).to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_ALLOC as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().expect("alloc exact limit");
    assert_eq!(vm.register(10), Memory::HEAP_START);
    assert_eq!(vm.memory.heap_limit(), Memory::HEAP_MAX_SIZE);
}

#[test]
fn test_heap_alloc_oob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(10, Memory::HEAP_MAX_SIZE + 8);
    let mut prog = Vec::new();
    prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_ALLOC as u8,
        )
        .to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(ivm::VMError::OutOfMemory)));
}
