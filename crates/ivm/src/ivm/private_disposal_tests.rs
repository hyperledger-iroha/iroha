//! Shared templates erase their exclusive image only after the final strong borrower releases it.

use super::{IVM, Memory};
use crate::{
    encoding::wide,
    memory::private_disposal::tests::{assert_erased_and_freed, budget, freed, serial, watch},
};
use std::alloc::Layout;

#[test]
fn final_private_template_borrower_erases_image_after_source_and_cache_handles_drop() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    let mut vm = IVM::try_new_with_memory_budget(10_000, budget).unwrap();
    vm.load_code(&wide::encode_halt().to_le_bytes()).unwrap();
    vm.set_zk_mode(true).unwrap();
    let address = vm.alloc_host_private_tlv(&[0x91, 0x92, 0x93]).unwrap();
    vm.registers.set(2, 0xCAFE_BABE);
    vm.registers.set_tag(2, true);
    let template = vm.try_runtime_template().unwrap();
    let borrower = template.clone();
    let bytes = Memory::image_bytes_for_stack_limit(borrower.data().memory.stack_limit()).unwrap();
    let pointer = borrower.data().memory.load_region(0, 1).unwrap().as_ptr();
    let _watch = watch(pointer, Layout::array::<u8>(bytes).unwrap(), 0, bytes);
    drop(vm);
    drop(template);
    assert!(!freed());
    assert_eq!(
        borrower.data().memory.inspect_region(address, 3).unwrap(),
        &[0x91, 0x92, 0x93]
    );
    assert_eq!(borrower.data().registers.get(2), 0xCAFE_BABE);
    assert!(borrower.data().registers.tag(2));
    assert!(budget.reserved_bytes() >= bytes);
    drop(borrower);
    assert_erased_and_freed();
    assert_eq!(budget.reserved_bytes(), 0);
}
