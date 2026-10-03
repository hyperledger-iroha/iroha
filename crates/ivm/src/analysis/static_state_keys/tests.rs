//! Exact key output, first-site evidence and original-pool lifetime regressions.

use super::super::static_state_literals::TestLiterals;
use super::*;

fn decoded(numbers: &[u32]) -> Vec<DecodedOp> {
    numbers
        .iter()
        .enumerate()
        .map(|(index, number)| DecodedOp {
            pc: index as u64 * 4,
            inst: crate::encoding::wide::encode_syscallx(*number),
        })
        .collect()
}

fn literals() -> TestLiterals<'static> {
    TestLiterals {
        names: &[Some("é"), Some("Map"), Some("Map/00ff"), Some("Map")],
        paths: &[Some("Map/00ff"), Some("é")],
        envelopes: &[Some(&[0xab, 0xcd])],
        payloads: &[Some(&[0, 255])],
    }
}

#[test]
fn sorted_unique_output_preserves_text_hex_wildcards_and_shared_final_owner() {
    use crate::syscalls::{
        SYSCALL_STATE_GET as READ, SYSCALL_STATE_SCAN as SCAN, SYSCALL_STATE_SET as WRITE,
    };
    let literals = literals();
    let budget = AllocationBudget::new(1024 * 1024);
    let mut scratch =
        KeyScratch::new(&decoded(&[READ, READ, WRITE, WRITE, SCAN, READ]), &budget).unwrap();
    let records = [
        (StaticStatePath::Literal(0), READ),
        (
            StaticStatePath::MapChild {
                base: 1,
                key: StaticNoritoKey::LiteralPayload(0),
            },
            READ,
        ),
        (StaticStatePath::FromName(0), WRITE),
        (
            StaticStatePath::MapChild {
                base: 1,
                key: StaticNoritoKey::PointerEnvelope(0),
            },
            WRITE,
        ),
        (StaticStatePath::FromName(3), SCAN),
        (StaticStatePath::Literal(1), READ),
    ];
    for (index, (path, syscall)) in records.into_iter().enumerate() {
        scratch
            .record(
                index as u64 * 4,
                Descriptor::new(path, syscall, &literals).unwrap(),
                &literals,
            )
            .unwrap();
    }
    let (reads, writes) = scratch.finish(&literals, &budget).unwrap();
    assert_eq!(
        reads.iter().collect::<Vec<_>>(),
        ["state:Map/00ff", "state:Map[*]", "state:é"]
    );
    assert_eq!(
        writes.iter().collect::<Vec<_>>(),
        ["state:Map/abcd", "state:é"]
    );
    assert!(reads.contains("state:Map[*]"));
    assert!(!reads.contains("state:Map"));
    assert!(ChargedShared::ptr_eq(
        reads.owner.as_ref().unwrap(),
        writes.owner.as_ref().unwrap()
    ));
    let expected = ChargedShared::<KeyStorage>::allocation_layout().size()
        + 5 * std::mem::size_of::<KeyRange>()
        + reads
            .iter()
            .chain(writes.iter())
            .map(str::len)
            .sum::<usize>();
    assert_eq!(budget.reserved_bytes(), expected);
    let final_borrower = writes.clone();
    budget.set_limit_bytes(0);
    drop(reads);
    drop(writes);
    assert_eq!(budget.reserved_bytes(), expected);
    assert_eq!(final_borrower.len(), 2);
    drop(final_borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exact_output_admission_refuses_before_any_destination_and_retries() {
    let literals = literals();
    let budget = AllocationBudget::new(1024 * 1024);
    let code = decoded(&[crate::syscalls::SYSCALL_STATE_GET]);
    let key = Descriptor::new(
        StaticStatePath::Literal(0),
        crate::syscalls::SYSCALL_STATE_GET,
        &literals,
    )
    .unwrap();
    let make = || {
        let mut scratch = KeyScratch::new(&code, &budget).unwrap();
        scratch.record(0, key, &literals).unwrap();
        scratch
    };
    let scratch = make();
    let scratch_bytes = budget.reserved_bytes();
    let output_bytes = ChargedShared::<KeyStorage>::allocation_layout().size()
        + std::mem::size_of::<KeyRange>()
        + "state:Map/00ff".len();
    budget.set_limit_bytes(scratch_bytes + output_bytes - 1);
    assert!(
        matches!(scratch.finish(&literals, &budget), Err(VMError::AllocationDeferred(AllocationRefusal::Capacity { requested_bytes, .. })) if requested_bytes == output_bytes)
    );
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(scratch_bytes + output_bytes);
    let (reads, writes) = make().finish(&literals, &budget).unwrap();
    assert!(writes.is_empty() && writes.owner.is_none());
    assert_eq!(budget.reserved_bytes(), output_bytes);
    drop(reads);
    assert_eq!(budget.reserved_bytes(), 0);
    let zero = AllocationBudget::new(0);
    assert!(matches!(
        KeyScratch::new(&code, &zero),
        Err(VMError::AllocationDeferred(_))
    ));
    let empty = KeyScratch::new(&[], &zero)
        .unwrap()
        .finish(&literals, &zero)
        .unwrap();
    assert!(empty.0.owner.is_none() && empty.1.owner.is_none());
    assert_eq!(zero.reserved_bytes(), 0);
}

#[test]
fn first_site_key_remains_after_ambiguity_and_different_exact_key_is_rejected() {
    let literals = literals();
    let budget = AllocationBudget::new(1024 * 1024);
    let read = crate::syscalls::SYSCALL_STATE_GET;
    let mut scratch = KeyScratch::new(&decoded(&[read]), &budget).unwrap();
    let original = Descriptor::new(StaticStatePath::Literal(0), read, &literals).unwrap();
    scratch.record(0, original, &literals).unwrap();
    // Distinct provenance with the same logical text is the same output key.
    let equivalent = Descriptor::new(
        StaticStatePath::MapChild {
            base: 1,
            key: StaticNoritoKey::LiteralPayload(0),
        },
        read,
        &literals,
    )
    .unwrap();
    scratch.record(0, equivalent, &literals).unwrap();
    let different = Descriptor::new(StaticStatePath::Literal(1), read, &literals).unwrap();
    assert!(matches!(
        scratch.record(0, different, &literals),
        Err(VMError::DecodeError)
    ));
    assert!(
        Descriptor::new(
            StaticStatePath::Literal(0),
            crate::syscalls::SYSCALL_STATE_SCAN,
            &literals
        )
        .is_none()
    );
    let (reads, writes) = scratch.finish(&literals, &budget).unwrap();
    assert_eq!(reads.iter().collect::<Vec<_>>(), ["state:Map/00ff"]);
    drop((reads, writes));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_retention_and_each_partial_output_failure_reclaim_only_original_pool() {
    let _limits = crate::ivm_cache::CacheLimitsGuard::new(crate::ivm_cache::CacheLimits {
        capacity: 0,
        max_bytes: 0,
        max_decoded_ops: 0,
    });
    let literals = literals();
    let budget = AllocationBudget::new(1024 * 1024);
    let original = budget.try_reserve_bytes(19).unwrap();
    let read = crate::syscalls::SYSCALL_STATE_GET;
    let code = decoded(&[read]);
    let key = Descriptor::new(StaticStatePath::Literal(0), read, &literals).unwrap();
    for fault in 1..=4 {
        let mut scratch = KeyScratch::new(&code, &budget).unwrap();
        scratch.record(0, key, &literals).unwrap();
        OUTPUT_FAULT.set(fault);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scratch.finish(&literals, &budget)
        }));
        if fault == 4 {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(VMError::ExecutionDeferred(
                    ExecutionDeferral::AllocationUnavailable
                )))
            ));
        }
        assert_eq!(OUTPUT_FAULT.get(), 0);
        assert_eq!(budget.reserved_bytes(), 19);
    }
    let mut scratch = KeyScratch::new(&code, &budget).unwrap();
    scratch.record(0, key, &literals).unwrap();
    let (reads, writes) = scratch.finish(&literals, &budget).unwrap();
    let retained = budget.reserved_bytes();
    assert!(retained > 19, "zero retention cannot refund a live output");
    let borrower = reads.clone();
    drop((reads, writes));
    assert_eq!(budget.reserved_bytes(), retained);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 19);
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
}

// The former output representation is kept only as an independent test oracle.
fn reference_key(descriptor: Descriptor, literals: &impl LiteralSource) -> String {
    let name = match descriptor.path {
        StaticStatePath::Literal(index) => literals.path(usize::from(index)).unwrap().to_owned(),
        StaticStatePath::FromName(index) => literals.name(usize::from(index)).unwrap().to_owned(),
        StaticStatePath::MapChild { base, key } => {
            let bytes = match key {
                StaticNoritoKey::PointerEnvelope(index) => {
                    literals.envelope(usize::from(index)).unwrap()
                }
                StaticNoritoKey::LiteralPayload(index) => {
                    literals.payload(usize::from(index)).unwrap()
                }
            };
            format!(
                "{}/{}",
                literals.name(usize::from(base)).unwrap(),
                hex::encode(bytes)
            )
        }
    };
    if descriptor.wildcard {
        format!("state:{name}[*]")
    } else {
        format!("state:{name}")
    }
}

#[test]
fn diamond_and_loop_match_old_collecting_reference_with_first_key_per_site() {
    use super::super::{
        StaticStateAccessAnalysis, StaticStateFacts, static_state_workspace::Workspace,
        transfer_static_state_facts,
    };
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    // The short diamond arm reaches the first read before the longer arm can
    // merge a different literal. A loop then revisits that same site with less
    // information. Its original exact key must remain in an incomplete result.
    const EDGES: &[&[usize]] = &[&[1], &[2, 5], &[3], &[4], &[2, 8], &[6], &[7], &[2], &[]];
    let copy = crate::encoding::wide::encode_ri(wide::arithmetic::ADDI, 20, 0, 0);
    let words = [
        crate::encoding::wide::encode_literal(wide::memory::LDLIT, 10, 0),
        copy,
        crate::encoding::wide::encode_syscallx(crate::syscalls::SYSCALL_STATE_GET),
        crate::encoding::wide::encode_literal(wide::memory::LDLIT, 10, 0),
        copy,
        crate::encoding::wide::encode_literal(wide::memory::LDLIT, 10, 1),
        copy,
        copy,
        crate::encoding::wide::encode_syscallx(crate::syscalls::SYSCALL_STATE_SET),
    ];
    let code: Vec<_> = words
        .into_iter()
        .enumerate()
        .map(|(index, inst)| DecodedOp {
            pc: index as u64 * 4,
            inst,
        })
        .collect();
    for same_path in [false, true] {
        let paths = [
            Some("Map/00ff"),
            Some(if same_path { "Map/00ff" } else { "é" }),
        ];
        let literals = TestLiterals {
            names: &[],
            paths: &paths,
            envelopes: &[],
            payloads: &[],
        };
        let mut incoming = BTreeMap::from([(0, StaticStateFacts::entrypoint())]);
        let mut pending = VecDeque::from([0]);
        let mut reference = [BTreeSet::new(), BTreeSet::new()];
        let mut by_site: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
        let mut expected = StaticStateAccessAnalysis {
            complete: true,
            ..Default::default()
        };
        let mut old_visits = 0;
        while let Some(index) = pending.pop_front() {
            old_visits += 1;
            assert!(old_visits < 100);
            let mut facts = incoming[&index].clone();
            if let Some(key) =
                transfer_static_state_facts(&code[index], &literals, &mut facts, &mut expected)
            {
                let text = reference_key(key, &literals);
                reference[usize::from(key.write)].insert(text.clone());
                let keys = by_site.entry(code[index].pc).or_default();
                keys.insert(text);
                assert_eq!(
                    keys.len(),
                    1,
                    "flat facts cannot change one exact site spelling"
                );
            }
            for next in EDGES[index] {
                match incoming.entry(*next) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(facts.clone());
                        pending.push_back(*next);
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        if entry.get_mut().merge_from(&facts) {
                            pending.push_back(*next);
                        }
                    }
                }
            }
        }
        let budget = AllocationBudget::new(4 * 1024 * 1024);
        let mut workspace = Workspace::new(code.len(), &budget).unwrap();
        let mut scratch = KeyScratch::new(&code, &budget).unwrap();
        let mut actual = StaticStateAccessAnalysis {
            complete: true,
            ..Default::default()
        };
        workspace.merge(0, &StaticStateFacts::entrypoint()).unwrap();
        let mut visits = 0;
        while let Some((index, mut facts)) = workspace.pop() {
            visits += 1;
            assert!(visits < 100);
            if let Some(key) =
                transfer_static_state_facts(&code[index], &literals, &mut facts, &mut actual)
            {
                scratch.record(code[index].pc, key, &literals).unwrap();
            }
            for next in EDGES[index] {
                workspace.merge(*next, &facts).unwrap();
            }
        }
        assert!(
            visits > code.len(),
            "the loop actually revisits instruction facts"
        );
        assert!(
            !actual.complete,
            "a changed literal index is ambiguous even when its text matches"
        );
        assert_eq!(actual.complete, expected.complete);
        assert_eq!(actual.has_state_reads, expected.has_state_reads);
        assert_eq!(actual.has_state_writes, expected.has_state_writes);
        (actual.read_keys, actual.write_keys) = scratch.finish(&literals, &budget).unwrap();
        assert!(
            actual
                .read_keys
                .iter()
                .eq(reference[0].iter().map(String::as_str))
        );
        assert!(
            actual
                .write_keys
                .iter()
                .eq(reference[1].iter().map(String::as_str))
        );
        assert!(actual.read_keys.contains("state:Map/00ff"));
        drop(workspace);
        drop(actual);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
