//! Borrowed-name lookup, bounded traversal and exact original-pool custody.

use super::*;
use crate::encoding::wide as enc;
use crate::error::{VmDeferral, VmTrap};
use crate::instruction::wide;
use iroha_allocation::AllocationRefusal;

fn descriptor(name: &str, entry_pc: u64) -> EmbeddedEntrypointDescriptor {
    EmbeddedEntrypointDescriptor {
        name: name.into(),
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        authorization:
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: None,
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc,
    }
}

fn encoded(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn instruction_graph(bytes: &[u8]) -> (ValidatedInstructions<'_>, PreparedControlFlow) {
    let instructions = ValidatedInstructions::new(bytes).unwrap();
    let graph = PreparedControlFlow::from_decoded(
        &instructions.iter().collect::<Vec<_>>(),
        None,
        crate::IvmEngine::C10,
    )
    .unwrap();
    (instructions, graph)
}

#[test]
fn sorted_indexes_borrow_original_names_and_reset_private_reachability() {
    let bytes = encoded(&[
        enc::encode_branch(wide::control::BEQ, 2, 3, 2),
        enc::encode_offset24(wide::control::JMP, -1),
        enc::encode_scall(crate::syscalls::SYSCALL_GET_PRIVATE_INPUT),
        enc::encode_halt(),
    ]);
    let (instructions, graph) = instruction_graph(&bytes);
    let descriptors = [
        descriptor("zeta", 0),
        descriptor("alpha", 12),
        descriptor("middle", 4),
    ];
    let budget = AllocationBudget::new(4096);
    for budget in [None, Some(&budget)] {
        let index = Entrypoints::prepare(
            &descriptors,
            instructions,
            &graph,
            40,
            budget,
            crate::IvmEngine::C10,
        )
        .unwrap();
        assert_eq!(index.len(), 3);
        for (name, original, pc, private) in [
            ("alpha", 1, 52, false),
            ("middle", 2, 44, true),
            ("zeta", 0, 40, true),
        ] {
            let entry = index.get(&descriptors, name).unwrap();
            assert_eq!(entry.descriptor_index, original);
            assert_eq!(entry.absolute_pc, pc);
            assert_eq!(entry.requires_private_inputs, private);
        }
        assert!(index.get(&descriptors, "absent").is_none());
    }
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn duplicate_successors_cycles_and_indirect_edges_do_not_grow_the_queue() {
    let bytes = encoded(&[
        enc::encode_branch(wide::control::BEQ, 2, 3, 1),
        enc::encode_offset24(wide::control::JMP, -1),
        enc::encode_rr(wide::control::JALR, 0, 1, 0),
        enc::encode_scall(crate::syscalls::SYSCALL_GET_PRIVATE_INPUT),
        enc::encode_halt(),
    ]);
    let (instructions, graph) = instruction_graph(&bytes);
    let descriptors = [
        descriptor("cycle", 0),
        descriptor("indirect", 8),
        descriptor("private", 12),
    ];
    let bytes = std::mem::size_of::<EntrypointIndex>() * descriptors.len();
    let scratch = std::mem::size_of::<Visit>() * instructions.len();
    let budget = AllocationBudget::new(bytes + scratch);
    let index = Entrypoints::prepare(
        &descriptors,
        instructions,
        &graph,
        0,
        Some(&budget),
        crate::IvmEngine::C10,
    )
    .unwrap();
    assert!(
        !index
            .get(&descriptors, "cycle")
            .unwrap()
            .requires_private_inputs
    );
    assert!(
        !index
            .get(&descriptors, "indirect")
            .unwrap()
            .requires_private_inputs
    );
    assert!(
        index
            .get(&descriptors, "private")
            .unwrap()
            .requires_private_inputs
    );
    assert_eq!(budget.peak_reserved_bytes(), bytes + scratch);
    assert_eq!(
        budget.reserved_bytes(),
        bytes,
        "traversal scratch must be gone before publication"
    );
    drop(index);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_pool_refusals_refund_index_and_retry_the_same_descriptors() {
    let bytes = encoded(&[enc::encode_halt()]);
    let (instructions, graph) = instruction_graph(&bytes);
    let descriptors = [descriptor("run", 0)];
    let bytes = std::mem::size_of::<EntrypointIndex>();
    let scratch = std::mem::size_of::<Visit>();
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        Entrypoints::prepare(
            &descriptors,
            instructions,
            &graph,
            0,
            Some(&budget),
            crate::IvmEngine::C10
        ),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(
            AllocationRefusal::ExceedsLimit { .. }
        )))
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
    budget.set_limit_bytes(bytes + scratch - 1);
    assert!(
        matches!(Entrypoints::prepare(&descriptors, instructions, &graph, 0, Some(&budget), crate::IvmEngine::C10), Err(VMError::Deferred(VmDeferral::AllocationDeferred(AllocationRefusal::Capacity { requested_bytes, .. }))) if requested_bytes == scratch)
    );
    assert_eq!(budget.peak_reserved_bytes(), bytes);
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(bytes + scratch);
    let occupied = budget.try_reserve_bytes(bytes + scratch).unwrap();
    assert!(matches!(
        Entrypoints::prepare(
            &descriptors,
            instructions,
            &graph,
            0,
            Some(&budget),
            crate::IvmEngine::C10
        ),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(
            AllocationRefusal::Capacity { .. }
        )))
    ));
    drop(occupied);
    let index = Entrypoints::prepare(
        &descriptors,
        instructions,
        &graph,
        0,
        Some(&budget),
        crate::IvmEngine::C10,
    )
    .unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    budget.set_limit_bytes(0);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(index);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn malformed_indexes_refund_both_allocations_and_empty_index_needs_no_credit() {
    let bytes = encoded(&[enc::encode_halt()]);
    let (instructions, graph) = instruction_graph(&bytes);
    let budget = AllocationBudget::new(4096);
    for (descriptors, base) in [
        (vec![descriptor("same", 0), descriptor("same", 0)], 0),
        (vec![descriptor("bad", 2)], 0),
        (vec![descriptor("overflow", 4)], u64::MAX),
    ] {
        assert!(matches!(
            Entrypoints::prepare(
                &descriptors,
                instructions,
                &graph,
                base,
                Some(&budget),
                crate::IvmEngine::C10
            ),
            Err(VMError::Trap(VmTrap::DecodeError))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
    let zero = AllocationBudget::new(0);
    let index = Entrypoints::prepare(
        &[],
        instructions,
        &graph,
        0,
        Some(&zero),
        crate::IvmEngine::C10,
    )
    .unwrap();
    assert_eq!(index.len(), 0);
    assert_eq!(zero.peak_reserved_bytes(), 0);
}
