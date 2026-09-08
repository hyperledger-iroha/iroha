//! Deterministic lifecycle races. Integer keys are structural kernel fixtures and never
//! substitute for authenticated enrollment or a native hardware owner.

use super::*;
use std::{sync::mpsc, thread, time::Duration};

type Registry = SessionRegistry<u8, Vec<u8>, u8>;

fn deadline() -> NativeDeadlineV1 {
    NativeDeadlineV1::start(Duration::from_secs(30)).unwrap()
}

fn finish(registry: &Registry, id: u64) -> Result<u64> {
    registry.finish(
        registry.take_completion(id)?,
        Ok,
        |_, value| Ok(value),
        |owner, value| {
            owner.push(value);
            Ok(())
        },
    )
}

fn open(registry: &Registry, key: u8, owner: &Arc<Mutex<Vec<u8>>>) -> u64 {
    finish(
        registry,
        registry
            .begin(deadline(), |_| Ok((key, Arc::clone(owner), key)))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn same_owner_survives_all_handles_and_rejects_duplicate_or_rebound_objects() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let first = open(&registry, 1, &owner);
    let second = open(&registry, 1, &owner);
    registry.close(first).unwrap();
    registry.close(second).unwrap();
    registry.close(first).unwrap();
    assert_eq!(
        registry.begin(deadline(), |_| Ok((1, Arc::new(Mutex::new(vec![])), 1))),
        Err(RegistryError::Rejected)
    );
    assert_eq!(
        registry.begin(deadline(), |_| Ok((2, Arc::clone(&owner), 1))),
        Err(RegistryError::Rejected)
    );
    let third = open(&registry, 1, &owner);
    assert!(third > second);
    assert_eq!(*owner.lock().unwrap(), vec![1, 1, 1]);
    assert!(registry.invocation(first).is_err());
}

#[test]
fn possession_is_consumed_once_and_new_attempt_supersedes_old_completion() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let first = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 7)))
        .unwrap();
    let ticket = registry.take_completion(first).unwrap();
    assert!(registry.take_completion(first).is_err());
    let second = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 8)))
        .unwrap();
    assert_eq!(
        registry.finish(ticket, Ok, |_, x| Ok(x), |_, _| panic!("stale install")),
        Err(RegistryError::Rejected)
    );
    registry.cancel(first).unwrap();
    finish(&registry, second).unwrap();
    assert_eq!(*owner.lock().unwrap(), vec![8]);
}

#[test]
fn account_a_b_a_does_not_revive_any_old_handle_or_pending_ticket() {
    let registry = Registry::new();
    let a = Arc::new(Mutex::new(vec![]));
    let b = Arc::new(Mutex::new(vec![]));
    let old_a = open(&registry, 1, &a);
    let queued_a = registry.invocation(old_a).unwrap();
    let pending = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&a), 9)))
        .unwrap();
    let completion = registry.take_completion(pending).unwrap();
    let old_b = open(&registry, 2, &b);
    let current_a = open(&registry, 1, &a);
    assert!(registry.invocation(old_a).is_err());
    assert!(registry.invocation(old_b).is_err());
    assert!(registry.invocation(current_a).is_ok());
    assert!(
        registry
            .dispatch(queued_a, |_| panic!("revoked dispatch"))
            .is_err()
    );
    assert_eq!(
        registry.finish(
            completion,
            Ok,
            |_, x| Ok(x),
            |_, _| panic!("revived completion")
        ),
        Err(RegistryError::Rejected)
    );
    assert_eq!(*a.lock().unwrap(), vec![1, 1]);
}

#[test]
fn logout_during_native_source_validation_prevents_publication_without_io_lock() {
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let id = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 4)))
        .unwrap();
    let completion = registry.take_completion(id).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let worker_registry = Arc::clone(&registry);
    let worker = thread::spawn(move || {
        worker_registry.finish(
            completion,
            Ok,
            |_, value| {
                entered_tx.send(()).unwrap();
                resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(value)
            },
            |_, _| panic!("logout must fence installation"),
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    registry.revoke_selection().unwrap();
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert!(owner.lock().unwrap().is_empty());
}

#[test]
fn dispatched_completion_stays_with_original_owner_after_account_switch() {
    let registry = Arc::new(Registry::new());
    let a = Arc::new(Mutex::new(vec![]));
    let b = Arc::new(Mutex::new(vec![]));
    let handle_a = open(&registry, 1, &a);
    let invocation = registry.invocation(handle_a).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let worker_registry = Arc::clone(&registry);
    let worker = thread::spawn(move || {
        worker_registry.dispatch(invocation, |original| {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            original.push(42); // Represents retention in the exact dispatched owner's journal.
            42
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let handle_b = open(&registry, 2, &b);
    resume_tx.send(()).unwrap();
    let completed = worker.join().unwrap().unwrap();
    assert_eq!(completed.value, 42);
    assert!(!completed.session_is_current);
    assert_eq!(*a.lock().unwrap(), vec![1, 42]);
    assert_eq!(*b.lock().unwrap(), vec![2]);
    assert!(registry.invocation(handle_a).is_err());
    assert!(registry.invocation(handle_b).is_ok());
}

#[test]
fn queued_invocation_rechecks_close_after_waiting_for_original_owner() {
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let handle = open(&registry, 1, &owner);
    let invocation = registry.invocation(handle).unwrap();
    let held = owner.lock().unwrap();
    let worker_registry = Arc::clone(&registry);
    let worker = thread::spawn(move || {
        worker_registry.dispatch(invocation, |_| panic!("closed queue dispatched"))
    });
    registry.close(handle).unwrap();
    drop(held);
    assert!(matches!(
        worker.join().unwrap(),
        Err(RegistryError::Rejected)
    ));
}

#[test]
fn cancellation_and_expiry_reject_final_install() {
    for cancel in [false, true] {
        let registry = Registry::new();
        let owner = Arc::new(Mutex::new(vec![]));
        let id = registry
            .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 4)))
            .unwrap();
        let mut completion = registry.take_completion(id).unwrap();
        if cancel {
            registry.cancel(id).unwrap();
        } else {
            completion.deadline = NativeDeadlineV1::expired_for_test();
        }
        assert_eq!(
            registry.finish(
                completion,
                Ok,
                |_, x| Ok(x),
                |_, _| panic!("invalid install")
            ),
            Err(RegistryError::Rejected)
        );
        assert!(owner.lock().unwrap().is_empty());
    }
    let registry = Registry::new();
    assert_eq!(
        registry.begin(NativeDeadlineV1::expired_for_test(), |_| Ok((
            1,
            Arc::new(Mutex::new(vec![])),
            1
        ))),
        Err(RegistryError::Rejected)
    );
}

#[test]
fn rejected_proof_or_source_never_calls_install_and_cannot_reuse_attempt() {
    for proof_failure in [false, true] {
        let registry = Registry::new();
        let owner = Arc::new(Mutex::new(vec![]));
        let id = registry
            .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 4)))
            .unwrap();
        let completion = registry.take_completion(id).unwrap();
        assert_eq!(
            registry.finish(
                completion,
                |x| if proof_failure {
                    Err(RegistryError::Rejected)
                } else {
                    Ok(x)
                },
                |_, _| Err::<(), _>(RegistryError::Rejected),
                |_, _| panic!("invalid source")
            ),
            Err(RegistryError::Rejected)
        );
        assert!(registry.take_completion(id).is_err());
        assert!(owner.lock().unwrap().is_empty());
    }
}

#[test]
fn capacities_and_id_exhaustion_reject_without_revoking_current_session() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let handle = open(&registry, 1, &owner);
    registry.lock().unwrap().next_id = u64::MAX;
    assert_eq!(
        registry.begin(deadline(), |_| Ok((2, Arc::new(Mutex::new(vec![])), 2))),
        Err(RegistryError::Capacity)
    );
    assert!(registry.invocation(handle).is_ok());
    let registry = Registry::new();
    let mut owners = Vec::new();
    for key in 0..MAX_OWNERS {
        let owner = Arc::new(Mutex::new(vec![]));
        open(&registry, key as u8, &owner);
        owners.push(owner);
    }
    assert_eq!(
        registry.begin(deadline(), |_| Ok((255, Arc::new(Mutex::new(vec![])), 1))),
        Err(RegistryError::Capacity)
    );
    let current = open(&registry, (MAX_OWNERS - 1) as u8, owners.last().unwrap());
    assert!(registry.invocation(current).is_ok());
}

#[test]
fn poisoned_owner_never_dispatches_or_publishes_another_session() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let handle = open(&registry, 1, &owner);
    let invocation = registry.invocation(handle).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = registry.dispatch(invocation, |_| panic!("failed owner"));
    }));
    assert!(result.is_err());
    assert!(matches!(
        registry.dispatch(registry.invocation(handle).unwrap(), |_| ()),
        Err(RegistryError::Poisoned)
    ));
    let id = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 2)))
        .unwrap();
    assert_eq!(finish(&registry, id), Err(RegistryError::Poisoned));
    registry.close(handle).unwrap();
}

#[test]
fn failed_observer_install_fences_existing_handles_and_the_original_owner() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let old = open(&registry, 1, &owner);
    let queued = registry.invocation(old).unwrap();
    let id = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 2)))
        .unwrap();
    let completion = registry.take_completion(id).unwrap();
    assert_eq!(
        registry.finish(
            completion,
            Ok,
            |_, value| Ok(value),
            |state, value| {
                state.push(value);
                Err(RegistryError::Rejected)
            }
        ),
        Err(RegistryError::Rejected)
    );
    assert!(registry.invocation(old).is_err());
    assert!(
        registry
            .dispatch(queued, |_| panic!("failed install owner reused"))
            .is_err()
    );
    assert_eq!(
        registry.begin(deadline(), |_| Ok((1, Arc::clone(&owner), 3))),
        Err(RegistryError::Rejected)
    );
    assert_eq!(
        registry.begin(deadline(), |_| Ok((1, Arc::new(Mutex::new(vec![])), 3))),
        Err(RegistryError::Rejected)
    );
}

#[test]
fn live_handle_bound_does_not_install_unpublished_observers() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let first = open(&registry, 1, &owner);
    for _ in 1..MAX_HANDLES {
        open(&registry, 1, &owner);
    }
    let id = registry
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 2)))
        .unwrap();
    assert_eq!(finish(&registry, id), Err(RegistryError::Capacity));
    assert_eq!(owner.lock().unwrap().len(), MAX_HANDLES);
    registry.close(first).unwrap();
    open(&registry, 1, &owner);
    assert_eq!(owner.lock().unwrap().len(), MAX_HANDLES + 1);
}

#[test]
fn matching_numeric_ids_never_transfer_between_registry_instances() {
    let a = Registry::new();
    let b = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let id_a = a
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 1)))
        .unwrap();
    let id_b = b
        .begin(deadline(), |_| Ok((1, Arc::clone(&owner), 1)))
        .unwrap();
    assert_eq!(id_a, id_b);
    let completion = a.take_completion(id_a).unwrap();
    assert_eq!(
        b.finish(
            completion,
            |_| panic!("foreign proof"),
            |_, _: ()| Ok(()),
            |_, _| Ok(())
        ),
        Err(RegistryError::Rejected)
    );
    let handle_b = finish(&b, id_b).unwrap();
    let handle_a = open(&a, 1, &owner);
    // Match all structural fields deliberately; only native registry identity differs.
    let mut invocation = a.invocation(handle_a).unwrap();
    invocation.handle = handle_b;
    assert!(matches!(
        b.dispatch(invocation, |_| panic!("foreign invocation")),
        Err(RegistryError::Rejected)
    ));
    assert!(
        b.dispatch(b.invocation(handle_b).unwrap(), |_| ())
            .unwrap()
            .session_is_current
    );
}

#[test]
fn enrollment_phase_advances_once_without_publishing_a_handle_or_replacing_owner() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let first = registry
        .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
        .unwrap();
    let completion = registry.take_completion(first).unwrap();
    let second = registry
        .advance(completion, Ok, |original, value| {
            assert!(original.is_empty());
            Ok(value + 1)
        })
        .unwrap();
    assert_ne!(first, second);
    assert!(registry.invocation(first).is_err());
    assert!(registry.invocation(second).is_err());
    assert!(registry.take_completion(first).is_err());
    registry.cancel(first).unwrap();
    assert!(owner.lock().unwrap().is_empty());
    let handle = finish(&registry, second).unwrap();
    assert!(registry.invocation(handle).is_ok());
    assert_eq!(*owner.lock().unwrap(), vec![8]);
}

#[test]
fn revocation_or_replacement_during_enrollment_verification_cannot_publish_a_new_phase() {
    for revoke in [false, true] {
        let registry = Arc::new(Registry::new());
        let owner = Arc::new(Mutex::new(vec![]));
        let first = registry
            .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
            .unwrap();
        let completion = registry.take_completion(first).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker_registry = registry.clone();
        let worker = thread::spawn(move || {
            worker_registry.advance(
                completion,
                |value| {
                    entered_tx.send(()).unwrap();
                    resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(value)
                },
                |_, value| Ok(value),
            )
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let replacement = if revoke {
            registry.revoke_selection().unwrap();
            None
        } else {
            Some(
                registry
                    .begin(deadline(), |_| Ok((1, owner.clone(), 9)))
                    .unwrap(),
            )
        };
        resume_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
        if let Some(id) = replacement {
            finish(&registry, id).unwrap();
        }
        assert_eq!(
            *owner.lock().unwrap(),
            if revoke { vec![] } else { vec![9] }
        );
    }
}

#[test]
fn cancellation_during_enrollment_source_validation_does_not_wait_for_its_owner_lock() {
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let id = registry
        .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
        .unwrap();
    let completion = registry.take_completion(id).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let worker_registry = registry.clone();
    let worker = thread::spawn(move || {
        worker_registry.advance(completion, Ok, |_, value| {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(value)
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    registry.cancel(id).unwrap();
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert!(registry.take_completion(id).is_err());
    assert!(owner.lock().unwrap().is_empty());
}

#[test]
fn advancing_does_not_extend_the_original_clock_deadline() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let original = NativeDeadlineV1::start(Duration::from_millis(100)).unwrap();
    let first = registry
        .begin(original.clone(), |_| Ok((1, owner, 7)))
        .unwrap();
    let second = registry
        .advance(registry.take_completion(first).unwrap(), Ok, |_, v| Ok(v))
        .unwrap();
    while original.check().is_ok() {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(registry.take_completion(second).is_err());
}

#[test]
fn foreign_ticket_or_failed_enrollment_checks_never_create_a_new_phase() {
    for failure in 0..3 {
        let registry = Registry::new();
        let other = Registry::new();
        let owner = Arc::new(Mutex::new(vec![]));
        let first = registry
            .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
            .unwrap();
        let completion = registry.take_completion(first).unwrap();
        let target = if failure == 0 { &other } else { &registry };
        assert_eq!(
            target.advance(
                completion,
                |v| if failure == 1 {
                    Err(RegistryError::Rejected)
                } else {
                    Ok(v)
                },
                |_, _| Err(RegistryError::Rejected)
            ),
            Err(RegistryError::Rejected)
        );
        assert!(registry.take_completion(first).is_err());
        assert!(owner.lock().unwrap().is_empty());
    }
}

#[test]
fn phase_id_exhaustion_retains_current_handles_and_never_retries_consumed_attempt() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let handle = open(&registry, 1, &owner);
    let first = registry
        .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
        .unwrap();
    let completion = registry.take_completion(first).unwrap();
    registry.lock().unwrap().next_id = u64::MAX;
    assert_eq!(
        registry.advance(completion, Ok, |_, v| Ok(v)),
        Err(RegistryError::Capacity)
    );
    assert!(registry.invocation(handle).is_ok());
    assert!(registry.take_completion(first).is_err());
    assert_eq!(*owner.lock().unwrap(), vec![1]);
}

#[test]
fn expiry_while_reserving_never_adds_an_owner_or_revokes_the_current_handle() {
    // Pause at key comparison inside begin after its first clock check. This deterministically
    // models suspension while reserving without changing the actual native clock or registry.
    type Pause = Arc<Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>>;
    struct Key {
        value: u8,
        pause: Pause,
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            let pause = self.pause.lock().unwrap().take();
            if let Some((entered, resume)) = pause {
                entered.send(()).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            self.value == other.value
        }
    }
    impl Eq for Key {}
    let registry = Arc::new(SessionRegistry::<Key, Vec<u8>, u8>::new());
    let pause: Pause = Arc::new(Mutex::new(None));
    let owner = Arc::new(Mutex::new(vec![]));
    let first = registry
        .begin(deadline(), |_| {
            Ok((
                Key {
                    value: 1,
                    pause: pause.clone(),
                },
                owner.clone(),
                1,
            ))
        })
        .unwrap();
    let handle = registry
        .finish(
            registry.take_completion(first).unwrap(),
            Ok,
            |_, v| Ok(v),
            |owner, v| {
                owner.push(v);
                Ok(())
            },
        )
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    *pause.lock().unwrap() = Some((entered_tx, resume_rx));
    let original = NativeDeadlineV1::start(Duration::from_millis(100)).unwrap();
    let worker_deadline = original.clone();
    let worker_registry = registry.clone();
    let worker = thread::spawn(move || {
        worker_registry.begin(worker_deadline, |_| {
            Ok((Key { value: 2, pause }, Arc::new(Mutex::new(vec![])), 2))
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    while original.check().is_ok() {
        thread::sleep(Duration::from_millis(10));
    }
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert_eq!(registry.lock().unwrap().owners.len(), 1);
    assert!(registry.invocation(handle).is_ok());
    assert_eq!(*owner.lock().unwrap(), vec![1]);
}

#[derive(Clone, Copy)]
enum PreparationReplacement {
    Revoke,
    SameOwner,
    OtherOwner,
    RejectedPreparation,
}

fn preparation_race(replacement: PreparationReplacement) {
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let old_handle = open(&registry, 1, &owner);
    let worker_registry = registry.clone();
    let worker_owner = owner.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        worker_registry.begin(deadline(), |_| {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok((1, worker_owner, 7))
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let current = match replacement {
        PreparationReplacement::Revoke => {
            registry.revoke_selection().unwrap();
            None
        }
        PreparationReplacement::SameOwner => Some(open(&registry, 1, &owner)),
        PreparationReplacement::OtherOwner => {
            Some(open(&registry, 2, &Arc::new(Mutex::new(vec![]))))
        }
        PreparationReplacement::RejectedPreparation => {
            assert_eq!(
                registry.begin(deadline(), |_| Err(RegistryError::Rejected)),
                Err(RegistryError::Rejected)
            );
            Some(old_handle)
        }
    };
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert!(registry.lock().unwrap().pending.is_none());
    if let Some(current) = current {
        assert!(registry.invocation(current).is_ok());
    } else {
        assert!(registry.invocation(old_handle).is_err());
        assert!(registry.lock().unwrap().selected_owner.is_none());
    }
    assert!(!owner.lock().unwrap().contains(&7));
}

#[test]
fn revoke_during_native_preparation_cannot_reselect_old_account() {
    preparation_race(PreparationReplacement::Revoke);
}

#[test]
fn later_same_account_preparation_rejects_the_older_source_read() {
    preparation_race(PreparationReplacement::SameOwner);
}

#[test]
fn later_other_account_preparation_preserves_its_handle_against_old_source_read() {
    preparation_race(PreparationReplacement::OtherOwner);
}

#[test]
fn failed_newer_preparation_cannot_revive_an_older_preparation() {
    preparation_race(PreparationReplacement::RejectedPreparation);
}

#[test]
fn original_deadline_includes_source_preparation_without_revoking_current_handle() {
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let handle = open(&registry, 1, &owner);
    let original = NativeDeadlineV1::start(Duration::from_millis(100)).unwrap();
    let worker_deadline = original.clone();
    let worker_registry = registry.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        worker_registry.begin(worker_deadline, |_| {
            entered_tx.send(()).unwrap();
            resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok((2, Arc::new(Mutex::new(vec![])), 7))
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    while original.check().is_ok() {
        thread::sleep(Duration::from_millis(10));
    }
    resume_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert_eq!(registry.lock().unwrap().owners.len(), 1);
    assert!(registry.invocation(handle).is_ok());
}

#[test]
fn expired_or_exhausted_preparation_never_reads_native_source_or_cancels_pending() {
    let registry = Registry::new();
    let owner = Arc::new(Mutex::new(vec![]));
    let id = registry.begin(deadline(), |_| Ok((1, owner, 1))).unwrap();
    assert_eq!(
        registry.begin(NativeDeadlineV1::expired_for_test(), |_| panic!(
            "expired source read"
        )),
        Err(RegistryError::Rejected)
    );
    registry.lock().unwrap().next_id = u64::MAX;
    assert_eq!(
        registry.begin(deadline(), |_| panic!("exhausted source read")),
        Err(RegistryError::Capacity)
    );
    assert!(registry.take_completion(id).is_ok());
}

fn complete_stale_phase(
    registry: &Registry,
    advance: bool,
    completion: OpenCompletion<Vec<u8>, u8>,
    verify: impl FnOnce(u8) -> Result<u8>,
    prepare: impl FnOnce(&Vec<u8>, u8) -> Result<u8>,
) -> Result<u64> {
    if advance {
        registry.advance(completion, verify, prepare)
    } else {
        registry.finish(completion, verify, prepare, |_, _| {
            panic!("stale completion must never install")
        })
    }
}

fn stale_completion_never_starts_work(advance: bool) {
    for invalidation in 0..4 {
        let registry = Registry::new();
        let owner = Arc::new(Mutex::new(vec![]));
        let id = registry
            .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
            .unwrap();
        let mut completion = registry.take_completion(id).unwrap();
        let replacement = match invalidation {
            0 => {
                registry.cancel(id).unwrap();
                None
            }
            1 => {
                registry.revoke_selection().unwrap();
                None
            }
            2 => Some(
                registry
                    .begin(deadline(), |_| Ok((1, owner.clone(), 9)))
                    .unwrap(),
            ),
            _ => {
                completion.deadline = NativeDeadlineV1::expired_for_test();
                None
            }
        };
        let verified = std::cell::Cell::new(0);
        let prepared = std::cell::Cell::new(0);
        assert_eq!(
            complete_stale_phase(
                &registry,
                advance,
                completion,
                |value| {
                    verified.set(verified.get() + 1);
                    Ok(value)
                },
                |_, value| {
                    prepared.set(prepared.get() + 1);
                    Ok(value)
                },
            ),
            Err(RegistryError::Rejected),
        );
        assert_eq!(
            verified.get(),
            0,
            "stale proof parser/verifier ran: invalidation {invalidation}"
        );
        assert_eq!(
            prepared.get(),
            0,
            "stale native source read ran: invalidation {invalidation}"
        );
        assert!(registry.take_completion(id).is_err());
        assert!(owner.lock().unwrap().is_empty());
        if let Some(replacement) = replacement {
            finish(&registry, replacement).unwrap();
            assert_eq!(*owner.lock().unwrap(), vec![9]);
        }
    }
}

#[test]
fn stale_advance_never_starts_verification_or_source_read() {
    stale_completion_never_starts_work(true);
}

#[test]
fn stale_finish_never_starts_verification_or_source_read() {
    stale_completion_never_starts_work(false);
}

fn completion_rechecks_after_owner_wait(advance: bool) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for revoke in [false, true] {
        let registry = Arc::new(Registry::new());
        let owner = Arc::new(Mutex::new(vec![]));
        let id = registry
            .begin(deadline(), |_| Ok((1, owner.clone(), 7)))
            .unwrap();
        let completion = registry.take_completion(id).unwrap();
        let held = owner.lock().unwrap();
        let (verified_tx, verified_rx) = mpsc::channel();
        let prepared = Arc::new(AtomicUsize::new(0));
        let worker_prepared = prepared.clone();
        let worker_registry = registry.clone();
        let worker = thread::spawn(move || {
            complete_stale_phase(
                &worker_registry,
                advance,
                completion,
                |value| {
                    verified_tx.send(()).unwrap();
                    Ok(value)
                },
                |_, value| {
                    worker_prepared.fetch_add(1, Ordering::SeqCst);
                    Ok(value)
                },
            )
        });
        // Verification has passed the first dispatch check. The held original
        // owner excludes source preparation until cancellation is committed.
        verified_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if revoke {
            registry.revoke_selection().unwrap();
        } else {
            registry.cancel(id).unwrap();
        }
        drop(held);
        assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
        assert_eq!(
            prepared.load(Ordering::SeqCst),
            0,
            "cancelled queued completion read the native source"
        );
        assert!(registry.take_completion(id).is_err());
        assert!(owner.lock().unwrap().is_empty());
    }
}

#[test]
fn advance_rechecks_cancellation_after_waiting_for_original_owner() {
    completion_rechecks_after_owner_wait(true);
}

#[test]
fn finish_rechecks_cancellation_after_waiting_for_original_owner() {
    completion_rechecks_after_owner_wait(false);
}

#[test]
fn preparation_permit_rejects_exact_cancellation_revocation_and_failed_replacement() {
    for invalidation in 0..3 {
        let registry = Registry::new();
        assert_eq!(
            registry.begin(deadline(), |permit| {
                permit.require_current().unwrap();
                match invalidation {
                    0 => registry.cancel(permit.id).unwrap(),
                    1 => registry.revoke_selection().unwrap(),
                    _ => assert_eq!(
                        registry.begin(deadline(), |_| Err(RegistryError::Rejected)),
                        Err(RegistryError::Rejected)
                    ),
                }
                assert_eq!(permit.require_current(), Err(RegistryError::Rejected));
                Err(RegistryError::Rejected)
            }),
            Err(RegistryError::Rejected)
        );
    }
}

#[test]
fn preparation_permit_never_restarts_the_original_deadline() {
    let registry = Registry::new();
    let original = NativeDeadlineV1::start(Duration::from_millis(100)).unwrap();
    assert_eq!(
        registry.begin(original.clone(), |permit| {
            permit.require_current().unwrap();
            while original.check().is_ok() {
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(permit.require_current(), Err(RegistryError::Rejected));
            Err(RegistryError::Rejected)
        }),
        Err(RegistryError::Rejected)
    );
}

fn queued_begin_never_reads_after_owner_wait(replacement: bool) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let registry = Arc::new(Registry::new());
    let owner = Arc::new(Mutex::new(vec![]));
    let held = owner.lock().unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let worker_registry = registry.clone();
    let worker_owner = owner.clone();
    let worker_reads = reads.clone();
    let worker = thread::spawn(move || {
        worker_registry.begin(deadline(), |permit| {
            entered_tx.send(()).unwrap();
            let source = worker_owner.lock().unwrap();
            permit.require_current()?;
            worker_reads.fetch_add(1, Ordering::SeqCst);
            drop(source);
            Ok((1, worker_owner, 7))
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let newer = if replacement {
        let new_owner = Arc::new(Mutex::new(vec![]));
        Some(open(&registry, 2, &new_owner))
    } else {
        registry.revoke_selection().unwrap();
        None
    };
    drop(held);
    assert_eq!(worker.join().unwrap(), Err(RegistryError::Rejected));
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert!(owner.lock().unwrap().is_empty());
    if let Some(handle) = newer {
        assert!(registry.invocation(handle).is_ok());
    }
}

#[test]
fn queued_begin_rechecks_revocation_before_source_read() {
    queued_begin_never_reads_after_owner_wait(false);
}

#[test]
fn queued_begin_rechecks_new_begin_before_source_read() {
    queued_begin_never_reads_after_owner_wait(true);
}
