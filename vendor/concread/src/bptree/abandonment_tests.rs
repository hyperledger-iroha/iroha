//! Failed private cursor cleanup can outlive its original physical writer.

use super::*;
use std::{
    cmp::Ordering,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicBool, Ordering as AtomicOrdering},
};

#[test]
fn failed_cursor_abandonment_unlocks_without_reopening_publication_authority() {
    static FAIL_COMPARE: AtomicBool = AtomicBool::new(false);
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Key(usize);
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            assert!(
                !FAIL_COMPARE.load(AtomicOrdering::SeqCst),
                "injected original key comparison"
            );
            self.0.cmp(&other.0)
        }
    }
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    let map = BptreeMap::new();
    let mut initial = map.write();
    initial.insert(Key(0), 10usize);
    initial.commit();
    let reader = map.read();
    let mut writer = map.write();
    writer.insert(Key(1), 11);
    FAIL_COMPARE.store(true, AtomicOrdering::SeqCst);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = writer.inner.as_mut().try_update_private(&Key(0), 20);
    }));
    FAIL_COMPARE.store(false, AtomicOrdering::SeqCst);
    assert!(result.is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
    let retirement = crate::internals::bptree::node::allocation_tests::without_allocations(|| {
        writer.abort_retaining()
    });
    assert!(map.try_acquire_writer().is_some());
    assert_eq!(reader.get(&Key(0)), Some(&10));
    assert_eq!(map.read().len(), 1);
    drop(retirement);
    assert_eq!(map.read().get(&Key(1)), None);
}
