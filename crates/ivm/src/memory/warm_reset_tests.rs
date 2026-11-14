//! Warm memory resets preserve pending template writes and canonical commitments,
//! whether or not either image has allocated its Merkle storage.

use super::*;

const ADDRESSES: [u64; 4] = [
    Memory::HEAP_START,
    Memory::HEAP_START + 63 * 32,
    Memory::HEAP_START + 64 * 32,
    Memory::STACK_START + Memory::MIN_STACK_SIZE - 32,
];

/// How a template's Merkle storage relates to its bytes when it is captured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplateState {
    /// No root was ever requested: every written leaf is pending.
    Deferred,
    /// Storage is allocated and every leaf is committed.
    AllocatedClean,
    /// Storage is allocated and two later writes are still pending.
    AllocatedPending,
}

fn template_in(state: TemplateState) -> Memory {
    let mut template = Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
    for (index, &address) in ADDRESSES.iter().enumerate().take(2) {
        template.store_u8(address, 0xa0 + index as u8).unwrap();
    }
    if state == TemplateState::AllocatedPending {
        template.root().unwrap();
    }
    for (index, &address) in ADDRESSES.iter().enumerate().skip(2) {
        template.store_u8(address, 0xa0 + index as u8).unwrap();
    }
    if state == TemplateState::AllocatedClean {
        template.root().unwrap();
    }
    // Runtime templates can retain pending Merkle writes after their
    // modification tracking has become the invocation baseline.
    template.mark_template_clean();
    assert_eq!(
        template.merkle_storage_allocated(),
        state != TemplateState::Deferred
    );
    assert_eq!(
        template.dirty_chunks.len(),
        match state {
            TemplateState::Deferred => 4,
            TemplateState::AllocatedClean => 0,
            TemplateState::AllocatedPending => 2,
        }
    );
    template
}

/// A worker holding the template's bytes with the opposite allocation state.
fn worker_with_other_state(template: &Memory) -> Memory {
    if template.merkle_storage_allocated() {
        // Replay the template's writes into an image that never requested a root.
        let mut worker = Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
        for (index, &address) in ADDRESSES.iter().enumerate() {
            worker.store_u8(address, 0xa0 + index as u8).unwrap();
        }
        worker.mark_template_clean();
        assert!(!worker.merkle_storage_allocated());
        worker
    } else {
        let mut worker = template.try_clone_for_runtime_template(None).unwrap();
        worker.root().unwrap();
        assert!(worker.merkle_storage_allocated());
        worker
    }
}

#[test]
fn warm_reset_roots_and_proofs_match_templates_in_every_allocation_state() {
    for state in [
        TemplateState::Deferred,
        TemplateState::AllocatedClean,
        TemplateState::AllocatedPending,
    ] {
        let template = template_in(state);
        let mut expected = template.try_clone_for_runtime_template(None).unwrap();
        let expected_root = expected.root().unwrap();
        let expected_proofs: Vec<_> = ADDRESSES
            .iter()
            .map(|&address| expected.merkle_root_and_path(address).unwrap())
            .collect();
        for same_state in [true, false] {
            let mut worker = if same_state {
                template.try_clone_for_runtime_template(None).unwrap()
            } else {
                worker_with_other_state(&template)
            };
            assert_eq!(
                worker.merkle_storage_allocated() == template.merkle_storage_allocated(),
                same_state
            );
            let memory_pointer = worker.data.as_ptr();
            let dirty_pointer = worker.dirty_chunks.words().as_ptr();
            let modified_pointer = worker.modified_chunks.words().as_ptr();
            for iteration in 0..6 {
                if iteration != 2 {
                    // Some writes overlap pending template leaves; one deliberately
                    // does not. The untouched dirty leaves must survive every cycle.
                    worker
                        .store_u8(ADDRESSES[iteration % ADDRESSES.len()], 0xe1)
                        .unwrap();
                    worker.store_u8(Memory::HEAP_START + 32, 0xf2).unwrap();
                }
                if iteration % 2 == 0 {
                    worker.commit();
                }
                let allocated = worker.merkle_storage_allocated();
                worker.reset_from_template(&template).unwrap();
                assert_eq!(&*worker.data, &*template.data);
                // A reset never allocates or releases Merkle storage.
                assert_eq!(worker.merkle_storage_allocated(), allocated);
                if allocated == template.merkle_storage_allocated() {
                    assert_eq!(worker.dirty_chunks, template.dirty_chunks);
                }
                assert!(worker.modified_chunks.is_empty());
                assert_eq!(worker.data.as_ptr(), memory_pointer);
                assert_eq!(worker.dirty_chunks.words().as_ptr(), dirty_pointer);
                assert_eq!(worker.modified_chunks.words().as_ptr(), modified_pointer);
                if iteration % 3 != 1 {
                    // Skip the root on some cycles so a reset also meets a
                    // worker whose previous reset is still pending. The first
                    // request allocates a deferred worker's storage.
                    assert_eq!(
                        worker.root().unwrap(),
                        expected_root,
                        "{state:?} same_state={same_state} iteration={iteration}"
                    );
                    for (&address, expected_proof) in ADDRESSES.iter().zip(&expected_proofs) {
                        assert_eq!(
                            worker.merkle_root_and_path(address).unwrap(),
                            *expected_proof
                        );
                    }
                }
            }
            assert_eq!(worker.root().unwrap(), expected_root);
        }
        assert_eq!(
            template.merkle_storage_allocated(),
            state != TemplateState::Deferred
        );
    }
}

#[test]
fn reset_between_images_with_different_allocation_states_keeps_stale_leaves_pending() {
    // An allocated worker restored from a deferred template has no template
    // digests to copy: its restored and still-pending leaves stay pending.
    let template = template_in(TemplateState::Deferred);
    let mut worker = template.try_clone_for_runtime_template(None).unwrap();
    worker.root().unwrap();
    worker.store_u8(Memory::HEAP_START + 5 * 32, 0x77).unwrap();
    worker.root().unwrap();
    worker.store_u8(Memory::HEAP_START + 9 * 32, 0x88).unwrap();
    worker.reset_from_template(&template).unwrap();
    let heap_leaf = Memory::HEAP_START as usize / 32;
    assert_eq!(
        worker.dirty_chunks.iter().collect::<Vec<_>>(),
        [heap_leaf + 5, heap_leaf + 9]
    );
    assert!(worker.dirty_for_testing());
    // A deferred worker restored from an allocated template keeps pending
    // against the zero tree, so every non-zero leaf must stay in its bitmap.
    let template = template_in(TemplateState::AllocatedClean);
    let mut worker = worker_with_other_state(&template);
    let baseline = worker.dirty_chunks.try_copy(None).unwrap();
    assert_eq!(baseline.len(), 4);
    worker.store_u8(Memory::HEAP_START + 5 * 32, 0x77).unwrap();
    worker.reset_from_template(&template).unwrap();
    assert!(!worker.merkle_storage_allocated());
    assert_eq!(worker.dirty_chunks.len(), 5);
    for index in baseline.iter() {
        assert!(worker.dirty_chunks.iter().any(|pending| pending == index));
    }
    let mut expected = template.try_clone_for_runtime_template(None).unwrap();
    assert_eq!(worker.root().unwrap(), expected.root().unwrap());
}
