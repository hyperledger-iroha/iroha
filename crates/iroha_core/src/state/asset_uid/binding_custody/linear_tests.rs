//! The original funded node zipper advances and rewinds without replacing physical custody.
use super::*;
use crate::smartcontracts::isi::sccp::test_support::authority;

struct Input {
    value: AssetUidBindingV1,
    path: StatePath,
    bytes: Vec<u8>,
}
fn input(seed: u8) -> Input {
    let mut uid = [seed; 32];
    uid[0] = 1;
    let value = AssetUidBindingV1 {
        asset_definition_id: iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id(),
        // Zero is a valid fixture ordinal, but not valid Ed25519 seed material.
        account_id: authority(seed.checked_add(1).expect("fixture account seed fits")),
        uid,
    };
    Input {
        path: binding_key(&value.asset_definition_id, &value.account_id),
        bytes: encode(&value).unwrap(),
        value,
    }
}
fn read(
    invocation: &BindingInvocation,
    input: &Input,
    present: bool,
    pool: &AllocationBudget,
) -> Result<Option<[u8; 32]>, ExecutionAttemptError<Error>> {
    invocation.read(
        &input.path,
        present.then_some(input.bytes.as_slice()),
        &input.value.asset_definition_id,
        &input.value.account_id,
        pool,
    )
}

#[derive(Debug, PartialEq, Eq)]
struct Physical {
    identity: (usize, usize),
    path: Option<usize>,
    row: Option<usize>,
    key: Option<usize>,
}
fn physical(original: &BindingPreparations) -> (Vec<Physical>, Vec<usize>) {
    fn payload(entry: &Entry) -> Physical {
        Physical {
            identity: entry.identity,
            path: entry
                .path
                .as_ref()
                .map(|value| value.as_slice().as_ptr() as usize),
            row: entry
                .row
                .as_ref()
                .map(|value| value.as_slice().as_ptr() as usize),
            key: entry.complete.as_ref().map(|value| {
                value
                    .canonical()
                    .account_id
                    .try_signatory()
                    .unwrap()
                    .try_to_bytes()
                    .unwrap()
                    .1
                    .as_ptr() as usize
            }),
        }
    }
    let bank = original.0.lock().unwrap();
    let mut values = Vec::new();
    let mut nodes = Vec::new();
    if let Some(current) = bank.current.as_ref() {
        values.push(payload(current));
    }
    for mut next in [
        bank.current.as_ref().and_then(|entry| entry.next.as_ref()),
        bank.previous.as_ref(),
    ] {
        while let Some(node) = next {
            nodes.push(node.as_slice().as_ptr() as usize);
            let entry = node.as_slice().first().unwrap();
            values.push(payload(entry));
            next = entry.next.as_ref();
        }
    }
    values.sort_unstable_by_key(|value| value.identity);
    nodes.sort_unstable();
    (values, nodes)
}

#[test]
fn native_binding_zipper_traversal_is_linear_and_preserves_original_physical_owners() {
    const COUNT: usize = 128;
    let inputs: Vec<_> = (0..COUNT)
        .map(|i| input(u8::try_from(i).unwrap()))
        .collect();
    let pool = AllocationBudget::new(4 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    let mut run = original.run(&pool).unwrap();
    let invocation = run.transaction();
    for input in &inputs {
        assert_eq!(read(&invocation, input, false, &pool).unwrap(), None);
        assert_eq!(
            read(&invocation, input, true, &pool).unwrap(),
            Some(input.value.uid)
        );
    }
    assert!(original.0.lock().unwrap().traversals <= 2 * COUNT);
    let retained = physical(&original);
    let retained_bytes = pool.reserved_bytes();
    assert_eq!(retained.0.len(), COUNT);
    assert_eq!(retained.1.len(), COUNT - 1);
    drop(invocation);
    drop(run);
    original.0.lock().unwrap().traversals = 0;
    let allocations = crate::test_allocations::allocations_during(|| {
        let mut replay = original.run(&pool).unwrap();
        let invocation = replay.transaction();
        for input in &inputs {
            assert_eq!(read(&invocation, input, false, &pool).unwrap(), None);
            assert_eq!(
                read(&invocation, input, true, &pool).unwrap(),
                Some(input.value.uid)
            );
        }
    });
    assert_eq!(
        allocations, 0,
        "rewind and completed replay reuse every original allocation"
    );
    assert!(
        original.0.lock().unwrap().traversals <= 5 * COUNT,
        "one rewind and one monotonic replay must perform only linear node visits"
    );
    assert_eq!(physical(&original), retained);
    assert_eq!(pool.reserved_bytes(), retained_bytes);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_binding_zipper_moves_partial_controls_without_reallocation_or_source_replacement() {
    for control in norito::core::PreparedDecodeWorkspace::allocation_layouts() {
        let inputs: Vec<_> = (0..8).map(input).collect();
        let pool = AllocationBudget::new(1 << 20);
        let original = BindingPreparations::try_new(&pool).unwrap();
        let mut run = original.run(&pool).unwrap();
        let invocation = run.transaction();
        for input in &inputs[..7] {
            assert_eq!(
                read(&invocation, input, true, &pool).unwrap(),
                Some(input.value.uid)
            );
        }
        let last = &inputs[7];
        let controls: usize = norito::core::PreparedDecodeWorkspace::allocation_layouts()
            .iter()
            .map(std::alloc::Layout::size)
            .sum();
        let demand = std::alloc::Layout::array::<Entry>(1).unwrap().size()
            + last.path.as_ref().len()
            + last.bytes.len()
            + controls;
        let held = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - demand)
            .unwrap();
        let Err(ExecutionAttemptError::Deferred(refusal)) = read(&invocation, last, true, &pool)
        else {
            panic!("actual account leaf refuses after metadata, capture and both controls fit");
        };
        assert!(
            matches!(refusal.allocation_refusal(), Some(iroha_allocation::AllocationRefusal::Capacity { requested_bytes, .. }) if *requested_bytes == last.value.account_id.try_signatory().unwrap().retained_allocation_layout().size())
        );
        let before = physical(&original);
        assert_eq!(before.0.last().unwrap().key, None);
        drop(refusal);
        drop(invocation);
        drop(run);
        drop(held);
        let ((), replaced) =
            crate::test_allocations::refuse_layout_occurrence_during(control, 0, || {
                let mut replay = original.run(&pool).unwrap();
                let invocation = replay.transaction();
                for input in &inputs {
                    assert_eq!(
                        read(&invocation, input, true, &pool).unwrap(),
                        Some(input.value.uid)
                    );
                }
            });
        assert!(
            !replaced,
            "the exact original decoder controls must survive zipper movement"
        );
        let after = physical(&original);
        assert_eq!(
            before.1, after.1,
            "the same metadata nodes retain their physical charges"
        );
        for (before, after) in before.0.iter().zip(&after.0) {
            assert_eq!(before.identity, after.identity);
            assert_eq!(before.path, after.path);
            assert_eq!(before.row, after.row);
            if before.key.is_some() {
                assert_eq!(before.key, after.key);
            }
        }
        assert!(after.0.last().unwrap().key.is_some());
        drop(original);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn native_binding_zipper_rejects_stale_runs_skipped_occurrences_and_changed_authority() {
    let first = input(1);
    let second = input(2);
    let pool = AllocationBudget::new(1 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    let mut stale_run = original.run(&pool).unwrap();
    let stale = stale_run.transaction();
    read(&stale, &first, true, &pool).unwrap();
    read(&stale, &second, true, &pool).unwrap();
    let before = physical(&original);
    let mut active = original.run(&pool).unwrap();
    let current = active.transaction();
    assert!(matches!(
        read(&stale, &first, true, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    let stale_again = stale_run.transaction();
    assert!(matches!(
        read(&stale_again, &first, true, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    assert_eq!(
        read(&current, &first, true, &pool).unwrap(),
        Some(first.value.uid)
    );
    // Advancing transactions before the retained second read skips established history.
    let skipped = active.transaction();
    assert!(matches!(
        read(&skipped, &second, true, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    // An earlier live transaction cannot resume after the later transaction's read.
    assert!(matches!(
        read(&current, &second, true, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    assert_eq!(physical(&original), before);
    let mut active = original.run(&pool).unwrap();
    let current = active.transaction();
    assert!(matches!(
        read(&current, &first, false, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    let mut active = original.run(&pool).unwrap();
    let current = active.transaction();
    assert!(matches!(
        read(&current, &second, true, &pool),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    assert_eq!(physical(&original), before);
    let retained = pool.reserved_bytes();
    original.0.lock().unwrap().generation = usize::MAX;
    assert!(original.run(&pool).is_err());
    assert_eq!(pool.reserved_bytes(), retained);
}

#[test]
fn native_binding_zipper_append_refusal_and_unwind_keep_both_original_chains_sound() {
    let inputs: Vec<_> = (0..16).map(input).collect();
    let pool = AllocationBudget::new(1 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    let mut run = original.run(&pool).unwrap();
    let invocation = run.transaction();
    for input in &inputs[..15] {
        read(&invocation, input, true, &pool).unwrap();
    }
    let before = physical(&original);
    let retained = pool.reserved_bytes();
    let (refused, actual) = crate::test_allocations::refuse_layout_occurrence_during(
        std::alloc::Layout::array::<Entry>(1).unwrap(),
        0,
        || read(&invocation, &inputs[15], true, &pool),
    );
    assert!(
        actual,
        "append must admit one actual metadata backing allocation"
    );
    assert!(matches!(refused, Err(ExecutionAttemptError::Deferred(_))));
    assert_eq!(physical(&original), before);
    assert_eq!(pool.reserved_bytes(), retained);
    drop(refused);
    drop(invocation);
    drop(run);
    let watch = pool.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let mut replay = original.run(&pool).unwrap();
        let invocation = replay.transaction();
        for input in &inputs[..8] {
            read(&invocation, input, true, &pool).unwrap();
        }
        {
            let bank = original.0.lock().unwrap();
            assert!(bank.previous.is_some());
            assert!(bank.current.as_ref().unwrap().next.is_some());
        }
        panic!("cancel actual zipper with both original chains populated");
    }));
    assert!(result.is_err());
    assert_eq!(
        watch.reserved_bytes(),
        0,
        "unwinding retires both actual zipper chains and controls"
    );
}
