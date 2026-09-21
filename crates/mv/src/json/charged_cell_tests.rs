//! Prepaid restoration using the sole Norito Cell parser and real EBR owners.

use super::*;
use crate::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    time::{Duration, Instant},
};

type ChargedCell<V> = Cell<V, AllocationCharge>;

fn pair_bytes<V: Value>() -> usize {
    ChargedCell::<V>::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum()
}

fn charges<V: Value>(
    budget: &AllocationBudget,
) -> Result<CellAllocationCharges<AllocationCharge>, AllocationRefusal> {
    let [current, undo] = ChargedCell::<V>::allocation_layouts();
    let mut reserved = budget.try_reserve_layouts([current, undo])?;
    Ok(CellAllocationCharges::new(
        reserved.try_split(current).expect("prepaid current layout"),
        reserved.try_split(undo).expect("prepaid undo layout"),
    ))
}

fn collect_until_empty(budget: &AllocationBudget) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while budget.reserved_bytes() != 0 {
        assert!(
            Instant::now() < deadline,
            "restored EBR owners still retained"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

struct NoCloneValue(u64);

impl Clone for NoCloneValue {
    fn clone(&self) -> Self {
        panic!("restoration must move its exact decoded values")
    }
}

impl JsonSerialize for NoCloneValue {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
}

#[derive(Clone)]
struct ObservedSeed {
    calls: Arc<AtomicUsize>,
    budget: AllocationBudget,
    panic: bool,
}

impl ValueSeed for ObservedSeed {
    type Value = NoCloneValue;

    fn parse_value(&self, _: json::Value) -> Result<Self::Value, json::Error> {
        panic!("charged restoration must retain streaming decode")
    }

    fn parse_parser(&self, parser: &mut json::Parser<'_>) -> Result<Self::Value, json::Error> {
        assert_eq!(self.budget.reserved_bytes(), pair_bytes::<NoCloneValue>());
        self.calls.fetch_add(1, SeqCst);
        assert!(!self.panic, "injected value-seed panic");
        u64::json_deserialize(parser).map(NoCloneValue)
    }
}

#[test]
fn charged_restore_moves_exact_undo_without_cloning_or_changing_wire_shape() {
    for (wire, undo, calls) in [
        (r#"{"revert":7,"blocks":11}"#, Some(7), 2),
        (r#"{"revert":null,"blocks":11}"#, None, 1),
    ] {
        let budget = AllocationBudget::new(pair_bytes::<NoCloneValue>());
        let parsed = Arc::new(AtomicUsize::new(0));
        let seed = CellSeeded {
            seed: ObservedSeed {
                calls: Arc::clone(&parsed),
                budget: budget.clone(),
                panic: false,
            },
        };
        let prepaid = charges::<NoCloneValue>(&budget).unwrap();
        let mut parser = json::Parser::new(wire);
        let cell = seed.deserialize_charged(&mut parser, prepaid).unwrap();
        assert!(parser.eof());
        assert_eq!(parsed.load(SeqCst), calls);
        assert_eq!(cell.view().0, 11);
        assert_eq!(cell.predecessor_view().as_ref().map(|value| value.0), undo);
        let untracked: Cell<u64> = json::from_str(wire).unwrap();
        assert_eq!(json::to_json(&cell).unwrap(), wire);
        assert_eq!(
            json::to_json(&cell).unwrap(),
            json::to_json(&untracked).unwrap()
        );
        assert_eq!(budget.reserved_bytes(), pair_bytes::<NoCloneValue>());
        drop(cell);
        collect_until_empty(&budget);
    }
}

#[test]
fn charged_restore_parse_failures_preserve_diagnostics_and_refund_unused_pair() {
    let seed = CellSeeded {
        seed: ValueFromJson::<u64>::new(),
    };
    for wire in [
        r#"[]"#,
        r#"{"blocks":11}"#,
        r#"{"revert":7}"#,
        r#"{"revert":7,"revert":8,"blocks":11}"#,
        r#"{"revert":7,"blocks":11,"blocks":12}"#,
        r#"{"revert":7,"blocks":11,"unknown":0}"#,
        r#"{"revert":"invalid","blocks":11}"#,
        r#"{"revert":7,"blocks":"invalid"}"#,
        r#"{"revert":7,"blocks":11"#,
    ] {
        let budget = AllocationBudget::new(pair_bytes::<u64>());
        let charged_error = seed
            .deserialize_charged(
                &mut json::Parser::new(wire),
                charges::<u64>(&budget).unwrap(),
            )
            .err()
            .expect("malformed charged Cell must fail");
        let ordinary_error = seed
            .deserialize(&mut json::Parser::new(wire))
            .err()
            .expect("same parser must reject untracked Cell");
        assert_eq!(charged_error.to_string(), ordinary_error.to_string());
        assert_eq!(budget.reserved_bytes(), 0, "unused prepaid pair for {wire}");
    }
}

#[test]
fn charged_restore_capacity_refuses_before_parse_and_retries_the_original_input() {
    let pair = pair_bytes::<NoCloneValue>();
    let budget = AllocationBudget::new(pair);
    let occupied = budget.try_reserve_bytes(pair).unwrap();
    let parsed = Arc::new(AtomicUsize::new(0));
    let seed = CellSeeded {
        seed: ObservedSeed {
            calls: Arc::clone(&parsed),
            budget: budget.clone(),
            panic: false,
        },
    };
    let mut parser = json::Parser::new(r#"{"revert":7,"blocks":11}"#);
    let refused = charges::<NoCloneValue>(&budget)
        .map(|prepaid| seed.deserialize_charged(&mut parser, prepaid));
    assert!(matches!(
        refused,
        Err(AllocationRefusal::Capacity {
            requested_bytes,
            reserved_bytes,
            limit_bytes,
            ..
        }) if requested_bytes == pair && reserved_bytes == pair && limit_bytes == pair
    ));
    assert_eq!(parsed.load(SeqCst), 0);
    drop(occupied);
    let cell = seed
        .deserialize_charged(&mut parser, charges::<NoCloneValue>(&budget).unwrap())
        .unwrap();
    assert!(parser.eof());
    assert_eq!(parsed.load(SeqCst), 2);
    assert_eq!(cell.view().0, 11);
    drop(cell);
    collect_until_empty(&budget);
}

#[test]
fn charged_restore_seed_unwind_refunds_only_uninstalled_ebr_owners() {
    let budget = AllocationBudget::new(pair_bytes::<NoCloneValue>());
    let seed = CellSeeded {
        seed: ObservedSeed {
            calls: Arc::new(AtomicUsize::new(0)),
            budget: budget.clone(),
            panic: true,
        },
    };
    let panic = catch_unwind(AssertUnwindSafe(|| {
        seed.deserialize_charged(
            &mut json::Parser::new(r#"{"revert":7,"blocks":11}"#),
            charges::<NoCloneValue>(&budget).unwrap(),
        )
    }));
    assert!(panic.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn charged_restore_serializes_staged_undo_and_retains_retired_generations() {
    let pair = pair_bytes::<u64>();
    let budget = AllocationBudget::new(2 * pair);
    let seed = CellSeeded {
        seed: ValueFromJson::<u64>::new(),
    };
    let cell = seed
        .deserialize_charged(
            &mut json::Parser::new(r#"{"revert":4,"blocks":7}"#),
            charges::<u64>(&budget).unwrap(),
        )
        .unwrap();
    let unrelated = crossbeam_epoch::pin();
    let original = cell.view();
    let predecessor = cell.predecessor_view();
    let mut block = cell.block_charged(charges::<u64>(&budget).unwrap());
    let mut transaction = block.transaction();
    *transaction.get_mut() = 9;
    transaction.apply();
    assert_eq!(json::to_json(&block).unwrap(), r#"{"revert":7,"blocks":9}"#);
    block.commit();
    assert_eq!(json::to_json(&cell).unwrap(), r#"{"revert":7,"blocks":9}"#);
    assert_eq!(*original, 7);
    assert_eq!(*predecessor, Some(4));
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    drop((original, predecessor));
    drop(cell);
    unrelated.flush();
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    drop(unrelated);
    collect_until_empty(&budget);
}
