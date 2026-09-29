//! Observe fixed memory diagnostics at actual final-owner allocation release.

use super::super::*;
use crate::memory::private_disposal::tests::{
    assert_erased_and_freed, budget, freed, original_credit_at_free, serial, watch,
    watch_second_span,
};
use std::alloc::Layout;

#[test]
fn attached_memory_rows_survive_shared_owners_then_erase_before_original_refund() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    for row_index in 0..2 {
        let mut memory = Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
        memory
            .store_bytes(Memory::HEAP_START, &[0xA1, 0xA2])
            .unwrap();
        let recorder = DiagnosticMemoryAccessRecorder::try_new(3, budget).unwrap();
        recorder.begin_run(true).unwrap();
        memory
            .install_diagnostic_access_recorder(recorder.shared())
            .unwrap();
        memory
            .store_bytes(Memory::HEAP_START, &[0xB1, 0xB2])
            .unwrap();
        let borrower = recorder.shared();
        let _watch = recorder.with_records(|rows| {
            let base = rows.as_ptr().cast::<u8>();
            let row = &rows[row_index];
            assert_eq!(
                (row.before, row.after),
                (0xA1 + row_index as u8, 0xB1 + row_index as u8)
            );
            let guard = watch(
                base,
                Layout::array::<DiagnosticMemoryAccess>(3).unwrap(),
                (&raw const row.before) as usize - base as usize,
                1,
            );
            watch_second_span((&raw const row.after) as usize - base as usize, 1);
            guard
        });
        drop(recorder);
        assert!(!freed());
        memory.clear_diagnostic_access_recorder();
        assert!(!freed());
        borrower.with_records(|rows| {
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[row_index].after, 0xB1 + row_index as u8);
        });
        let bytes = 3 * std::mem::size_of::<DiagnosticMemoryAccess>();
        assert_eq!(budget.reserved_bytes(), bytes);
        drop(borrower);
        assert_erased_and_freed();
        assert_eq!(original_credit_at_free(), bytes);
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(
            memory.load_region(Memory::HEAP_START, 2).unwrap(),
            &[0xB1, 0xB2]
        );
    }
}

#[test]
fn complete_initial_image_erases_after_final_borrower_without_touching_source() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    let mut memory = Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
    memory.preload_input(7, &[0xA9]).unwrap();
    memory.store_u8(Memory::HEAP_START + 9, 0xB8).unwrap();
    let bytes = usize::try_from(memory.stack_top()).unwrap();
    let recorder =
        DiagnosticMemoryAccessRecorder::try_new_with_initial_image(0, bytes, budget).unwrap();
    recorder.begin_run(true).unwrap();
    memory.capture_diagnostic_initial_image(&recorder).unwrap();
    let borrower = recorder.shared();
    let _watch = recorder.with_initial_image(|image| {
        let (state, image) = image.unwrap();
        assert_eq!(state.image_bytes, bytes);
        assert_eq!(image.len(), bytes);
        assert_eq!(image[(Memory::INPUT_START + 7) as usize], 0xA9);
        assert_eq!(image[(Memory::HEAP_START + 9) as usize], 0xB8);
        watch(
            image.as_ptr(),
            Layout::array::<u8>(bytes).unwrap(),
            0,
            bytes,
        )
    });
    drop(recorder);
    assert!(!freed());
    borrower.with_initial_image(|image| {
        assert_eq!(image.unwrap().1[(Memory::INPUT_START + 7) as usize], 0xA9)
    });
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(borrower);
    assert_erased_and_freed();
    assert_eq!(original_credit_at_free(), bytes);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(memory.load_u8(Memory::INPUT_START + 7).unwrap(), 0xA9);
    assert_eq!(memory.load_u8(Memory::HEAP_START + 9).unwrap(), 0xB8);
}

#[test]
fn memory_recorder_unwind_erases_retained_rows_before_original_refund() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    let recorder = DiagnosticMemoryAccessRecorder::try_new(1, budget).unwrap();
    recorder.begin_run(true).unwrap();
    recorder
        .record_write(
            Memory::HEAP_START,
            &[0xB8],
            &[0xC9],
            DiagnosticMemoryAccessKind::Write,
        )
        .unwrap();
    let _watch = recorder.with_records(|rows| {
        let base = rows.as_ptr().cast::<u8>();
        let row = &rows[0];
        let guard = watch(
            base,
            Layout::array::<DiagnosticMemoryAccess>(1).unwrap(),
            (&raw const row.before) as usize - base as usize,
            1,
        );
        watch_second_span((&raw const row.after) as usize - base as usize, 1);
        guard
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _recorder = recorder;
        panic!("unwind memory diagnostic recorder");
    }));
    assert!(result.is_err());
    assert_erased_and_freed();
    assert_eq!(
        original_credit_at_free(),
        std::mem::size_of::<DiagnosticMemoryAccess>()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn field_erasure_preserves_valid_enum_values_and_clears_private_scalars() {
    let mut row = DiagnosticMemoryAccess {
        step_ordinal: Some(11),
        access_ordinal: 12,
        byte_offset: 13,
        address: 14,
        before: 0xA5,
        after: 0xB6,
        kind: DiagnosticMemoryAccessKind::PrivateReset,
        privacy_tag: DiagnosticMemoryPrivacyTag::Private,
    };
    row.scrub();
    assert_eq!(
        (
            row.before,
            row.after,
            row.address,
            row.byte_offset,
            row.access_ordinal
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(row.step_ordinal, None);
    assert_eq!(row.kind, DiagnosticMemoryAccessKind::Read);
    assert_eq!(row.privacy_tag, DiagnosticMemoryPrivacyTag::Unknown);
}
