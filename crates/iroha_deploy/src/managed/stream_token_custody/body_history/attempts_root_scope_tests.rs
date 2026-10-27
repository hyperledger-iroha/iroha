//! Genuine attempts-root inventories retain every row, native owner and decoder boundary.
use super::*;
#[cfg(unix)]
use crate::managed::Error;
use iroha_fs::CustodyWorkV1;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::managed) enum Point {
    RootInventory,
    RowInventory,
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Counts {
    roots: usize,
    leaves: Vec<Point>,
}
type Hook = Box<dyn FnMut(Point) -> Result<()>>;
struct State {
    original: bool,
    counts: Counts,
    hook: Option<Hook>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
pub(in crate::managed) fn original_recipe() -> bool {
    STATE.with(|slot| slot.borrow().as_ref().is_some_and(|state| state.original))
}
pub(in crate::managed) fn root_bracket() {
    STATE.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.counts.roots += 1;
        }
    });
}
pub(in crate::managed) fn after_leaf(point: Point) -> Result<()> {
    let mut hook = STATE.with(|slot| {
        slot.borrow_mut().as_mut().and_then(|state| {
            state.counts.leaves.push(point);
            state.hook.take()
        })
    });
    let result = hook.as_mut().map_or(Ok(()), |hook| hook(point));
    STATE.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.hook = hook;
        }
    });
    result
}
fn observe<T>(original: bool, hook: Option<Hook>, read: impl FnOnce() -> T) -> (T, Counts) {
    struct Reset(Option<State>);
    impl Drop for Reset {
        fn drop(&mut self) {
            STATE.with(|slot| slot.replace(self.0.take()));
        }
    }
    let _reset = Reset(STATE.with(|slot| {
        slot.replace(Some(State {
            original,
            counts: Counts::default(),
            hook,
        }))
    }));
    let result = read();
    let counts = STATE.with(|slot| slot.take().unwrap().counts);
    (result, counts)
}
pub(super) fn count_roots<T>(read: impl FnOnce() -> T) -> (T, usize) {
    let (result, counts) = observe(false, None, read);
    (result, counts.roots)
}
fn work<T>(read: impl FnOnce() -> T) -> (T, CustodyWorkV1) {
    let before = CustodyWorkV1::current_thread();
    let result = read();
    (result, CustodyWorkV1::current_thread().since(before))
}
fn outcome(result: Result<()>) -> std::result::Result<(), String> {
    result.map_err(|error| error.to_string())
}
fn limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        MAX_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        allocation,
        64,
    )
}
fn read(
    native: &History,
    snapshot: &SnapshotReadPass<'_>,
    original: bool,
) -> ((std::result::Result<(), String>, [usize; 4]), Counts) {
    observe(original, None, || {
        History::test_operation_records(|| outcome(native.test_local_census(snapshot)))
    })
}

#[test]
fn attempts_root_scope_shares_only_four_actual_walks_with_identical_native_records() {
    let _resources = crate::managed::native_test_guard();
    let (_fixture, history) = shared_snapshot_tests::history_with_bodies(3);
    let head = history.current_history().unwrap();
    let owned = head.test_owned_predecessor_closure_history().unwrap();
    let snapshot = SnapshotReadPass {
        head: &history.bodies.last().unwrap().snapshots,
    };
    let root = owned.test_attempts_root().unwrap();
    let (_, suffix) = history
        .root
        .read_tree_scope(|tree| -> Result<_> {
            Ok(work(|| tree.revalidate_directory(root).unwrap()))
        })
        .unwrap();
    assert_eq!(suffix.ancestry_walks, 2);
    let (((old, old_counts), old_work), ((new, new_counts), new_work)) = (
        work(|| read(&owned, &snapshot, true)),
        work(|| read(&owned, &snapshot, false)),
    );
    assert_eq!(old, new);
    assert!(new.0.is_ok());
    assert!(new.1[0] > 0 && new.1[1] > 0);
    assert_eq!(old_counts.leaves, new_counts.leaves);
    assert_eq!(
        new_counts.leaves,
        [
            Point::RootInventory,
            Point::RowInventory,
            Point::RootInventory
        ]
    );
    assert_eq!((old_counts.roots, new_counts.roots), (0, 1));
    assert_eq!(old_work.ancestry_walks, new_work.ancestry_walks + 4);
    assert_eq!(
        old_work.ancestry_opens,
        new_work.ancestry_opens + 2 * suffix.ancestry_opens
    );
    // A pristine genuine head has no attempts root and cannot enter the new scope.
    let old = work(|| read(head, &snapshot, true));
    let new = work(|| read(head, &snapshot, false));
    assert_eq!(old, new);
    assert!(new.0.0.0.is_ok());
    assert_eq!(new.0.1.roots, 0);
}

#[test]
fn attempts_root_scope_retains_inherited_late_missing_and_foreign_admission() {
    let _resources = crate::managed::native_test_guard();
    let (fixture, history) = shared_snapshot_tests::history_with_bodies(3);
    let native = history
        .current_history()
        .unwrap()
        .test_owned_predecessor_closure_history()
        .unwrap();
    let snapshot = SnapshotReadPass {
        head: &history.bodies.last().unwrap().snapshots,
    };
    let nested = |original| {
        native
            .test_attempts_after_operation_admission(&snapshot, None, |read| {
                work(|| {
                    observe(original, None, || {
                        History::test_operation_records(|| outcome(read()))
                    })
                })
            })
            .unwrap()
    };
    let old = nested(true);
    let new = nested(false);
    assert_eq!(old.0.0, new.0.0);
    assert!(new.0.0.0.is_ok());
    assert_eq!(new.0.1.roots, 1);
    assert_eq!(old.1.ancestry_walks, new.1.ancestry_walks + 4);
    let run = |original, allocation| {
        norito::core::with_decode_limits_measured(limits(allocation), || {
            work(|| read(&native, &snapshot, original))
        })
    };
    let wide = run(true, usize::MAX);
    assert!(wide.0.0.0.0.is_ok());
    let exact = wide.1.total_allocated_bytes();
    assert!(exact > 1);
    for allocation in [0, 1, exact - 1, exact] {
        let old = run(true, allocation);
        let new = run(false, allocation);
        assert_eq!(old, new);
        assert_eq!(new.0.0.0.0.is_ok(), allocation == exact);
        let late = |original| {
            native
                .test_attempts_after_operation_admission(&snapshot, None, |read| {
                    norito::core::with_decode_limits_measured(limits(allocation), || {
                        work(|| {
                            observe(original, None, || {
                                History::test_operation_records(|| outcome(read()))
                            })
                        })
                    })
                })
                .unwrap()
        };
        let old = late(true);
        let new = late(false);
        assert_eq!(old, new);
        assert_eq!(new.0.0.1.roots, 0);
        if allocation <= 1 {
            assert!(new.0.0.0.0.is_err());
        }
        if allocation == exact {
            assert!(new.0.0.0.0.is_ok());
        }
    }
    let no_tree = |original| {
        work(|| {
            observe(original, None, || {
                History::test_operation_records(|| {
                    outcome(native.test_graph_local_census_in_tree(Some(&snapshot), None))
                })
            })
        })
    };
    let old = no_tree(true);
    let new = no_tree(false);
    assert_eq!(old, new);
    assert!(new.0.0.0.is_ok());
    assert_eq!(new.0.1.roots, 0);
    let reopened = history.read_current(&fixture.owner).unwrap();
    let foreign = SnapshotReadPass {
        head: &reopened.bodies.last().unwrap().snapshots,
    };
    for pass in [None, Some(&foreign)] {
        let run = |original| {
            work(|| {
                observe(original, None, || {
                    History::test_operation_records(|| {
                        outcome(native.test_native_read_tree(|tree| {
                            native.test_graph_local_census_in_tree(pass, tree)
                        }))
                    })
                })
            })
        };
        let old = run(true);
        let new = run(false);
        assert_eq!(old, new);
        assert!(new.0.0.0.is_ok());
        assert_eq!(new.0.1.roots, 0);
    }
    let operation = PrivateDirectory::open(history.bodies[1].directory.path()).unwrap();
    let run = |original| {
        native
            .test_attempts_after_operation_admission(&snapshot, Some(&operation), |read| {
                work(|| {
                    observe(original, None, || {
                        History::test_operation_records(|| outcome(read()))
                    })
                })
            })
            .unwrap()
    };
    let old = run(true);
    let new = run(false);
    assert_eq!(old, new);
    assert!(new.0.0.0.is_ok());
    assert_eq!(new.0.1.roots, 0);
}

#[cfg(unix)]
#[test]
fn attempts_root_scope_closes_row_errors_mutations_and_original_retry() {
    use std::{fs, os::unix::fs::PermissionsExt as _};
    let _resources = crate::managed::native_test_guard();
    let (_fixture, history) = shared_snapshot_tests::history_with_bodies(3);
    let native = history
        .current_history()
        .unwrap()
        .test_owned_predecessor_closure_history()
        .unwrap();
    let snapshot = SnapshotReadPass {
        head: &history.bodies.last().unwrap().snapshots,
    };
    let root = native.test_attempts_root().unwrap();
    let identity = root.identity().unwrap();
    for attacked in [history.root.path(), root.path()] {
        for ordinary in [false, true] {
            let run = |original| {
                let path = attacked.to_owned();
                let mut once = false;
                let result = observe(
                    original,
                    Some(Box::new(move |point| {
                        if point == Point::RowInventory && !once {
                            once = true;
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
                            if ordinary {
                                return Err(invalid("ordinary attempts refusal"));
                            }
                        }
                        Ok(())
                    })),
                    || native.test_local_census(&snapshot),
                )
                .0;
                fs::set_permissions(attacked, fs::Permissions::from_mode(0o700)).unwrap();
                assert!(
                    matches!(&result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied)
                );
                outcome(result)
            };
            assert_eq!(run(true), run(false));
            native.test_local_census(&snapshot).unwrap();
            assert_eq!(root.identity().unwrap(), identity);
        }
    }
    let extra = root.create_child("0002").unwrap();
    assert_eq!(
        outcome(native.test_local_census(&snapshot)),
        Err(invalid("dispatch inventory changed during native operation").to_string())
    );
    drop(extra);
    fs::remove_dir(root.path().join("0002")).unwrap();
    native.test_local_census(&snapshot).unwrap();
    // Both root observations precede operation metadata exit, so changed current bytes refuse.
    let path = history.bodies[1].directory.path().join("operation.nrt");
    let bytes = fs::read(&path).unwrap();
    let changed = path.clone();
    let mut once = false;
    let result = observe(
        false,
        Some(Box::new(move |point| {
            if point == Point::RootInventory && !once {
                once = true;
                let mut bytes = fs::read(&changed).unwrap();
                bytes[0] ^= 1;
                fs::write(&changed, bytes).unwrap();
            }
            Ok(())
        })),
        || native.test_local_census(&snapshot),
    )
    .0;
    fs::write(path, bytes).unwrap();
    assert!(result.is_err());
    native.test_local_census(&snapshot).unwrap();
}

#[test]
fn attempts_root_scope_keeps_genuine_reserved_absent_and_empty_tail_owners() {
    let _resources = crate::managed::native_test_guard();
    shared_snapshot_tests::history_with_bodies_in_final_turn(
        2,
        |fixture, history, authorization, _| {
            let head = history.current_history().unwrap();
            let (_, original, _) = history.dispatch().unwrap();
            let Action::Enroll { validity, .. } = &original.action else {
                unreachable!()
            };
            let pending = head
                .test_attempts_pending(
                    authorization.origin().unwrap(),
                    authorization
                        .terms(fixture.options.deadline, Some(validity.expires_at_unix_ms))
                        .unwrap(),
                )
                .unwrap();
            let snapshot = SnapshotReadPass {
                head: &history.bodies.last().unwrap().snapshots,
            };
            let root = pending.test_attempts_root().unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                // With no rows, an ordinary early refusal used to leave the root after its
                // first inventory. The new closed scope observes it before exposing that error.
                for original in [true, false] {
                    let path = root.path().to_owned();
                    let result = observe(
                        original,
                        Some(Box::new(move |point| {
                            if point == Point::RootInventory {
                                std::fs::set_permissions(
                                    &path,
                                    std::fs::Permissions::from_mode(0o755),
                                )
                                .unwrap();
                                return Err(invalid("ordinary empty attempts refusal"));
                            }
                            Ok(())
                        })),
                        || pending.test_local_census(&snapshot),
                    )
                    .0;
                    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
                        .unwrap();
                    if original {
                        assert_eq!(
                            outcome(result),
                            Err(invalid("ordinary empty attempts refusal").to_string())
                        );
                    } else {
                        assert!(
                            matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied)
                        );
                    }
                    pending.test_local_census(&snapshot).unwrap();
                }
            }
            for present in [false, true] {
                let tail = present.then(|| root.create_child("0001").unwrap());
                let old = work(|| read(&pending, &snapshot, true));
                let new = work(|| read(&pending, &snapshot, false));
                assert_eq!(old.0.0, new.0.0);
                assert!(new.0.0.0.is_ok());
                assert_eq!(old.0.1.leaves, new.0.1.leaves);
                assert_eq!(new.0.1.leaves, [Point::RootInventory, Point::RootInventory]);
                assert_eq!(new.0.1.roots, 1);
                assert_eq!(old.1.ancestry_walks, new.1.ancestry_walks + 4);
                if let Some(tail) = tail {
                    tail.write_atomic("foreign", b"unsafe empty tail", PublishMode::CreateNew)
                        .unwrap();
                    let old = read(&pending, &snapshot, true);
                    let new = read(&pending, &snapshot, false);
                    assert_eq!(old.0, new.0);
                    assert!(new.0.0.is_err());
                    std::fs::remove_file(tail.path().join("foreign")).unwrap();
                    pending.test_local_census(&snapshot).unwrap();
                }
            }
        },
    );
}
