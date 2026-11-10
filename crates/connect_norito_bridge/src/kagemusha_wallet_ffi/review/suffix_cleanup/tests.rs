use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Entry {
    suffix: bool,
    ended: bool,
    drops: Arc<AtomicUsize>,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
type Table = Tokens<OriginBound<Entry>>;

fn insert(
    table: &mut Table,
    token: u64,
    origin: u64,
    suffix: bool,
    ended: bool,
) -> Arc<AtomicUsize> {
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(
        table
            .values
            .insert(
                token,
                OriginBound::new(
                    origin,
                    Entry {
                        suffix,
                        ended,
                        drops: Arc::clone(&drops),
                    },
                ),
            )
            .is_none()
    );
    drops
}
fn cleanup(table: &mut Table, origin: u64, token: u64) -> Result<()> {
    discard(table, origin, token, |entry| entry.suffix)
}
fn prune_ended(table: &mut Table) {
    prune(table, |entry| entry.suffix && entry.ended);
}
fn invalid(result: Result<()>) {
    assert_eq!(result.expect_err("expected refusal").status, INVALID);
}

#[test]
fn suffix_discard_is_exact_origin_idempotent_after_pruning() {
    let mut table = Table::default();
    let drops = insert(&mut table, 1, 40, true, true);
    prune_ended(&mut table);
    assert!(table.values.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    cleanup(&mut table, 40, 1).unwrap();
    cleanup(&mut table, 40, 1).unwrap();
    invalid(cleanup(&mut table, 41, 1));
    invalid(cleanup(&mut table, 0, 1));
    invalid(cleanup(&mut table, 40, 0));
    assert_eq!(table.suffix_retired.len(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn direct_suffix_discard_keeps_only_metadata_and_foreign_does_not_consume() {
    let mut table = Table::default();
    let drops = insert(&mut table, 8, 3, true, false);
    invalid(cleanup(&mut table, 4, 8));
    assert!(table.values.contains_key(&8));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    cleanup(&mut table, 3, 8).unwrap();
    cleanup(&mut table, 3, 8).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(table.values.is_empty());
}

#[test]
fn ordinary_discard_remains_one_use_and_is_never_pruned_as_suffix() {
    let mut table = Table::default();
    let drops = insert(&mut table, 20, 5, false, true);
    prune_ended(&mut table);
    assert!(table.values.contains_key(&20));
    invalid(cleanup(&mut table, 6, 20));
    cleanup(&mut table, 5, 20).unwrap();
    invalid(cleanup(&mut table, 5, 20));
    assert!(table.suffix_retired.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn pruning_preserves_live_suffix_and_other_origin_ordinary_entries() {
    let mut table = Table::default();
    let expired = insert(&mut table, 1, 10, true, true);
    let live = insert(&mut table, 2, 11, true, false);
    let ordinary = insert(&mut table, 3, 12, false, false);
    prune_ended(&mut table);
    assert_eq!(expired.load(Ordering::SeqCst), 1);
    assert_eq!(live.load(Ordering::SeqCst), 0);
    assert_eq!(ordinary.load(Ordering::SeqCst), 0);
    assert_eq!(table.values.keys().copied().collect::<Vec<_>>(), vec![2, 3]);
}

#[test]
fn tombstones_are_bounded_and_repeated_cleanup_cannot_renew_oldest() {
    let mut table = Table::default();
    for token in 1..=MAX_RETIRED_SUFFIX_REVIEWS as u64 {
        insert(&mut table, token, token + 100, true, true);
        prune_ended(&mut table);
    }
    cleanup(&mut table, 101, 1).unwrap();
    let next = MAX_RETIRED_SUFFIX_REVIEWS as u64 + 1;
    insert(&mut table, next, next + 100, true, true);
    prune_ended(&mut table);
    assert_eq!(table.suffix_retired.len(), MAX_RETIRED_SUFFIX_REVIEWS);
    invalid(cleanup(&mut table, 101, 1));
    cleanup(&mut table, 102, 2).unwrap();
    cleanup(&mut table, next + 100, next).unwrap();
    invalid(cleanup(&mut table, next + 101, next));
}

#[test]
fn ended_suffix_cells_cannot_exhaust_ordinary_review_admission() {
    let mut table = Table::default();
    let drops: Vec<_> = (1..=8)
        .map(|token| insert(&mut table, token, token + 100, true, true))
        .collect();
    assert!(table.capacity().is_err());
    // Ordinary review admission must run this cleanup before checking shared capacity.
    // It does not require another suffix preparation or a successful explicit discard.
    prune_ended(&mut table);
    table.capacity().unwrap();
    let ordinary = insert(&mut table, 9, 109, false, false);
    assert!(table.values.contains_key(&9));
    assert!(drops.iter().all(|drops| drops.load(Ordering::SeqCst) == 1));
    for token in 1..=8 {
        cleanup(&mut table, token + 100, token).unwrap();
    }
    assert!(table.values.contains_key(&9));
    assert_eq!(ordinary.load(Ordering::SeqCst), 0);
}
