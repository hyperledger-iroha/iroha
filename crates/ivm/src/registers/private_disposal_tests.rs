//! Inline register bytes are wiped in their own owner, observed before the real box is freed.

use super::Registers;
use crate::memory::private_disposal::tests::watch_second_span;
use crate::memory::private_disposal::tests::{assert_erased_and_freed, serial, watch};
use std::alloc::Layout;

#[test]
fn boxed_register_owner_erases_its_entire_initialized_register_span() {
    let _serial = serial();
    let mut registers = Box::new(Registers::new());
    registers.gpr.fill(0xCAFE_BABE_DEAD_BEEF);
    registers.tags.fill(true);
    let base = (&raw const *registers).cast::<u8>();
    let offset = registers.gpr.as_ptr() as usize - base as usize;
    let _watch = watch(
        base,
        Layout::new::<Registers>(),
        offset,
        std::mem::size_of_val(&registers.gpr),
    );
    watch_second_span(
        registers.tags.as_ptr() as usize - base as usize,
        std::mem::size_of_val(&registers.tags),
    );
    drop(registers);
    assert_erased_and_freed();
}

#[test]
fn dropping_register_copy_does_not_erase_the_independent_live_source() {
    let _serial = serial();
    let mut source = Registers::new();
    source.set(7, 0xAABB_CCDD);
    source.set_tag(7, true);
    let copied = Box::new(source.try_clone_for_runtime_template().unwrap());
    let base = (&raw const *copied).cast::<u8>();
    let offset = copied.gpr.as_ptr() as usize - base as usize;
    let _watch = watch(
        base,
        Layout::new::<Registers>(),
        offset,
        std::mem::size_of_val(&copied.gpr),
    );
    watch_second_span(
        copied.tags.as_ptr() as usize - base as usize,
        std::mem::size_of_val(&copied.tags),
    );
    drop(copied);
    assert_erased_and_freed();
    assert_eq!(source.get(7), 0xAABB_CCDD);
    assert!(source.tag(7));
}
