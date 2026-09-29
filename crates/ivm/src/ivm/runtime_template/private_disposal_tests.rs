//! The original data Arc must erase inline plaintext before its real deallocation.

use super::{RuntimeTemplate, RuntimeTemplateData};
use crate::{
    IVM,
    encoding::wide,
    memory::private_disposal::tests::{
        assert_erased_and_freed, budget, capture_layout, captured_allocation, freed,
        original_credit_at_free, serial, watch, watch_second_span,
    },
};
use std::{alloc::Layout, sync::Barrier};

fn funded_private_template() -> RuntimeTemplate {
    let mut vm = IVM::try_new_with_memory_budget(10_000, budget()).unwrap();
    vm.load_code(&wide::encode_halt().to_le_bytes()).unwrap();
    vm.set_zk_mode(true).unwrap();
    vm.registers.set(2, 0xCAFE_BABE_DEAD_BEEF);
    vm.registers.set_tag(2, true);
    vm.try_runtime_template().unwrap()
}

fn data_layout() -> Layout {
    Layout::from_size_align(
        norito::core::owned_arc_allocation_bytes::<RuntimeTemplateData>().unwrap(),
        std::mem::align_of::<RuntimeTemplateData>().max(std::mem::align_of::<usize>()),
    )
    .unwrap()
}

fn observe_inline_values(
    template: &RuntimeTemplate,
) -> crate::memory::private_disposal::tests::Observation {
    let base = captured_allocation();
    let data_address = std::sync::Arc::as_ptr(&template.data) as usize;
    assert!(data_address >= base as usize);
    assert!(
        data_address + std::mem::size_of::<RuntimeTemplateData>()
            <= base as usize + data_layout().size()
    );
    let [gpr, tags] = template.data.registers.disposal_spans_for_testing();
    let observation = watch(base, data_layout(), gpr.0 as usize - base as usize, gpr.1);
    watch_second_span(tags.0 as usize - base as usize, tags.1);
    observation
}

#[test]
fn final_data_arc_erases_inline_registers_and_tags_before_its_original_credit_returns() {
    let _serial = serial();
    assert_eq!(budget().reserved_bytes(), 0);
    let _capture = capture_layout(data_layout());
    let template = funded_private_template();
    let borrower = template.clone();
    let remaining = template
        .backing
        ._allocation_lease
        .as_ref()
        .unwrap()
        .remaining_bytes();
    let _watch = observe_inline_values(&template);
    drop(template);
    assert!(!freed());
    assert_eq!(borrower.data.registers.get(2), 0xCAFE_BABE_DEAD_BEEF);
    assert!(borrower.data.registers.tag(2));
    drop(borrower);
    assert_erased_and_freed();
    assert_eq!(
        original_credit_at_free(),
        remaining,
        "the exact original aggregate lease survives data Arc deallocation"
    );
    assert!(remaining >= RuntimeTemplate::owner_allocation_bytes().unwrap());
    assert_eq!(budget().reserved_bytes(), 0);
}

#[test]
fn concurrent_final_data_arc_release_erases_in_place_and_keeps_paired_credit_live() {
    let _serial = serial();
    assert_eq!(budget().reserved_bytes(), 0);
    let _capture = capture_layout(data_layout());
    let template = funded_private_template();
    let remaining = template
        .backing
        ._allocation_lease
        .as_ref()
        .unwrap()
        .remaining_bytes();
    let _watch = observe_inline_values(&template);
    let barrier = Barrier::new(9);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let borrower = template.clone();
            let barrier = &barrier;
            scope.spawn(move || {
                assert_eq!(borrower.data.registers.get(2), 0xCAFE_BABE_DEAD_BEEF);
                assert!(borrower.data.registers.tag(2));
                barrier.wait();
                drop(borrower);
            });
        }
        drop(template);
        assert!(!freed());
        barrier.wait();
    });
    assert_erased_and_freed();
    assert_eq!(
        original_credit_at_free(),
        remaining,
        "the exact original aggregate lease survives data Arc deallocation"
    );
    assert!(remaining >= RuntimeTemplate::owner_allocation_bytes().unwrap());
    assert_eq!(budget().reserved_bytes(), 0);
}
