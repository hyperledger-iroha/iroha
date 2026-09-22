//! Four-cell installation custody; these structural tests grant no State finality.

use super::*;
use mv::{BlockMode, PublicationPreparationError};
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

type Blocks<'state> = (
    CellBlock<'state, SnapshotNexusRuntime>,
    CellBlock<'state, Vec<PeerId>>,
    CellBlock<'state, Vec<PeerId>>,
    CellBlock<'state, LaneConsensusContextsV1>,
);

#[derive(Debug, PartialEq, Eq)]
struct Images {
    canonical_runtime: (SnapshotNexusRuntime, Option<SnapshotNexusRuntime>),
    commit_topology: (Vec<PeerId>, Option<Vec<PeerId>>),
    prev_commit_topology: (Vec<PeerId>, Option<Vec<PeerId>>),
    lane_consensus_contexts: (LaneConsensusContextsV1, Option<LaneConsensusContextsV1>),
}

fn images(state: &State) -> Images {
    fn image<V: mv::Value>(cell: &Cell<V>) -> (V, Option<V>) {
        (
            cell.view().get().clone(),
            cell.predecessor_view().get().clone(),
        )
    }
    Images {
        canonical_runtime: image(&state.canonical_runtime),
        commit_topology: image(&state.commit_topology),
        prev_commit_topology: image(&state.prev_commit_topology),
        lane_consensus_contexts: image(&state.lane_consensus_contexts),
    }
}

fn blocks(state: &State, mode: BlockMode) -> Blocks<'_> {
    match mode {
        BlockMode::Ordinary => (
            state.canonical_runtime.block(),
            state.commit_topology.block(),
            state.prev_commit_topology.block(),
            state.lane_consensus_contexts.block(),
        ),
        BlockMode::Replace => (
            state.canonical_runtime.block_and_revert(),
            state.commit_topology.block_and_revert(),
            state.prev_commit_topology.block_and_revert(),
            state.lane_consensus_contexts.block_and_revert(),
        ),
    }
}

fn commit((runtime, topology, previous, contexts): Blocks<'_>) {
    runtime.commit();
    topology.commit();
    previous.commit();
    contexts.commit();
}

fn mutate((runtime, topology, previous, contexts): &mut Blocks<'_>, value: u8) {
    let mut context = crate::state::lane_consensus_context::frozen_lane_context_fixture_for_test();
    context.leader_seed = [value; 32];
    runtime.get_mut().autoscale_last_transition_height = u64::from(value);
    let mut child = topology.transaction();
    *child.get_mut() = context.committee.clone();
    child.get_mut().rotate_left(usize::from(value) % 4);
    child.apply();
    *previous.get_mut() = context.committee.clone();
    previous.get_mut().rotate_right(usize::from(value) % 4);
    // This child must not erase the preceding actual parent change.
    let mut aborted = previous.transaction();
    aborted.get_mut().clear();
    drop(aborted);
    *contexts.get_mut() = LaneConsensusContextsV1::new(vec![context]).unwrap();
}

fn fixture() -> Box<State> {
    let (state, _, _, _) = crate::state::carrier_preparation::tests::fixture();
    let mut initial = blocks(&state, BlockMode::Ordinary);
    mutate(&mut initial, 1);
    commit(initial);
    state
}

fn capture<Admission>(
    (runtime, topology, previous, contexts): Blocks<'_>,
    admission: Admission,
) -> RuntimeJournals<Admission> {
    RuntimeJournals::capture(runtime, topology, previous, contexts, |inputs| {
        let mode = inputs.canonical_runtime().mode();
        assert_eq!(inputs.commit_topology().mode(), mode);
        assert_eq!(inputs.prev_commit_topology().mode(), mode);
        assert_eq!(inputs.lane_consensus_contexts().mode(), mode);
        Ok::<_, Infallible>(admission)
    })
    .unwrap()
}

fn prepare<Admission>(
    journal: RuntimeJournals<Admission>,
    state: &State,
) -> PreparedRuntimeJournals<'_, Admission, ()> {
    journal
        .try_prepare_publication(state, |_, _| Ok::<_, Infallible>(()))
        .unwrap_or_else(|(_, error, _)| panic!("runtime publication: {error:?}"))
}

fn original_topology_allocation<Admission>(journal: &RuntimeJournals<Admission>) -> *const PeerId {
    journal
        .commit_topology
        .touched_value()
        .expect("original applied child touch")
        .after
        .as_ptr()
}

fn assert_writers_released_except(state: &State, except: Option<&str>) {
    macro_rules! released {
        ($field:ident) => {
            if except != Some(stringify!($field)) {
                drop(state.$field.block());
            }
        };
    }
    released!(canonical_runtime);
    released!(commit_topology);
    released!(prev_commit_topology);
    released!(lane_consensus_contexts);
}

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct ResourceGuard<'state> {
    state: &'state State,
    released: Arc<AtomicBool>,
    except: Option<&'static str>,
}

impl Drop for ResourceGuard<'_> {
    fn drop(&mut self) {
        assert_writers_released_except(self.state, self.except);
        assert!(!self.released.swap(true, Ordering::SeqCst));
    }
}

#[test]
fn runtime_publication_holds_all_four_writers_and_matches_direct_current_and_undo() {
    let state = fixture();
    let direct = fixture();
    let before = images(&state);
    let probes = capture(blocks(&state, BlockMode::Ordinary), ());
    let mut original = blocks(&state, BlockMode::Ordinary);
    let mut reference = blocks(&direct, BlockMode::Ordinary);
    mutate(&mut original, 2);
    mutate(&mut reference, 2);
    // Capture allocation identity before detachment, not merely after it. Value
    // equality would let a reconstructed successor/preimage pass this regression.
    macro_rules! allocations {
        ($runtime:expr, $topology:expr, $previous:expr, $contexts:expr) => {{
            [
                std::ptr::from_ref($runtime.touched_value().unwrap().before).cast::<()>(),
                std::ptr::from_ref($runtime.touched_value().unwrap().after).cast::<()>(),
                std::ptr::from_ref($topology.touched_value().unwrap().before).cast::<()>(),
                std::ptr::from_ref($topology.touched_value().unwrap().after).cast::<()>(),
                std::ptr::from_ref($previous.touched_value().unwrap().before).cast::<()>(),
                std::ptr::from_ref($previous.touched_value().unwrap().after).cast::<()>(),
                std::ptr::from_ref($contexts.touched_value().unwrap().before).cast::<()>(),
                std::ptr::from_ref($contexts.touched_value().unwrap().after).cast::<()>(),
            ]
        }};
    }
    let originals = allocations!(original.0, original.1, original.2, original.3);
    let journal = capture(original, ());
    let retained = |journal: &RuntimeJournals<()>| {
        allocations!(
            journal.canonical_runtime,
            journal.commit_topology,
            journal.prev_commit_topology,
            journal.lane_consensus_contexts
        )
    };
    assert_eq!(retained(&journal), originals);
    let pointer = original_topology_allocation(&journal);
    let prepared = prepare(journal, &state);
    assert_eq!(images(&state), before);
    macro_rules! held {
        ($field:ident) => {
            assert!(matches!(
                probes
                    .$field
                    .try_prepare_publication(&state.$field, |_, _| Ok::<_, Infallible>(())),
                Err((_, PublicationPreparationError::Busy(_), _))
            ));
        };
    }
    held!(canonical_runtime);
    held!(commit_topology);
    held!(prev_commit_topology);
    held!(lane_consensus_contexts);
    let journal = prepared.abort().0;
    assert_eq!(retained(&journal), originals);
    assert_eq!(original_topology_allocation(&journal), pointer);
    assert!(journal.matches_current(&state));
    assert_writers_released_except(&state, None);
    commit(reference);
    prepare(journal, &state).publish();
    macro_rules! published {
        ($field:ident, $index:expr) => {
            assert_eq!(
                std::ptr::from_ref(state.$field.predecessor_view().get().as_ref().unwrap())
                    .cast::<()>(),
                originals[$index]
            );
            assert_eq!(
                std::ptr::from_ref(state.$field.view().get()).cast::<()>(),
                originals[$index + 1]
            );
        };
    }
    published!(canonical_runtime, 0);
    published!(commit_topology, 2);
    published!(prev_commit_topology, 4);
    published!(lane_consensus_contexts, 6);
    assert_eq!(images(&state), images(&direct));
    assert_writers_released_except(&state, None);
}

#[test]
fn runtime_replacement_preserves_untouched_discarded_tip_and_real_undo() {
    for touched in [false, true] {
        let state = fixture();
        let direct = fixture();
        for target in [&state, &direct] {
            let mut tip = blocks(target, BlockMode::Ordinary);
            mutate(&mut tip, 3);
            commit(tip);
        }
        let mut original = blocks(&state, BlockMode::Replace);
        let mut reference = blocks(&direct, BlockMode::Replace);
        if touched {
            // Leave the last field untouched: replacement itself must restore
            // its predecessor instead of retaining the discarded tip context.
            original.0.get_mut().autoscale_last_transition_height = 9;
            reference.0.get_mut().autoscale_last_transition_height = 9;
        }
        let journal = capture(original, ());
        assert_eq!(journal.canonical_runtime.mode(), BlockMode::Replace);
        assert!(journal.lane_consensus_contexts.touched_value().is_none());
        commit(reference);
        prepare(journal, &state).publish();
        assert_eq!(images(&state), images(&direct));
        commit(blocks(&state, BlockMode::Replace));
        commit(blocks(&direct, BlockMode::Replace));
        assert_eq!(images(&state), images(&direct));
    }
}

#[test]
fn runtime_busy_at_each_field_returns_exact_journals_and_releases_earlier_writers() {
    let state = fixture();
    let before = images(&state);
    let retained = Arc::new(AtomicBool::new(false));
    let mut original = blocks(&state, BlockMode::Ordinary);
    mutate(&mut original, 2);
    let mut journal = capture(
        original,
        ResourceGuard {
            state: &state,
            released: Arc::clone(&retained),
            except: None,
        },
    );
    let pointer = original_topology_allocation(&journal);
    macro_rules! busy {
        ($field:ident) => {{
            let busy = state.$field.block();
            let released = Arc::new(AtomicBool::new(false));
            let (returned, error, _cleanup) = journal
                .try_prepare_publication(&state, |_, _| {
                    Ok::<_, Infallible>(ResourceGuard {
                        state: &state,
                        released: Arc::clone(&released),
                        except: Some(stringify!($field)),
                    })
                })
                .err()
                .expect("busy original component");
            drop(_cleanup);
            let RuntimePublicationError::Component {
                field,
                cause: PublicationPreparationError::Busy(wait),
            } = error
            else {
                panic!("expected original writer contention, got {error:?}");
            };
            assert_eq!(field, stringify!($field));
            assert!(released.load(Ordering::SeqCst));
            assert!(!retained.load(Ordering::SeqCst));
            // The returned wait belongs to the actual refused writer, not the
            // earlier fields released during rollback or a periodic timer.
            let wake = Arc::new(WakeCount::default());
            let waker = Waker::from(Arc::clone(&wake));
            let mut context = Context::from_waker(&waker);
            let mut wait = wait.wait_for_release();
            assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
            drop(busy);
            assert_eq!(wake.0.load(Ordering::SeqCst), 1);
            assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
            returned
        }};
    }
    for field in 0..4 {
        journal = match field {
            0 => busy!(canonical_runtime),
            1 => busy!(commit_topology),
            2 => busy!(prev_commit_topology),
            _ => busy!(lane_consensus_contexts),
        };
        assert_eq!(original_topology_allocation(&journal), pointer);
        assert_eq!(images(&state), before);
        assert!(journal.matches_current(&state));
        assert_writers_released_except(&state, None);
    }
    let guards = prepare(journal, &state).publish();
    assert!(!retained.load(Ordering::SeqCst));
    drop(guards);
    assert!(retained.load(Ordering::SeqCst));
}

#[test]
fn runtime_late_changed_field_restores_original_custody_and_releases_installation_last() {
    let state = fixture();
    let before = images(&state);
    let retained = Arc::new(AtomicBool::new(false));
    let installation = Arc::new(AtomicBool::new(false));
    let mut original = blocks(&state, BlockMode::Ordinary);
    mutate(&mut original, 2);
    let journal = capture(
        original,
        ResourceGuard {
            state: &state,
            released: Arc::clone(&retained),
            except: None,
        },
    );
    let pointer = original_topology_allocation(&journal);
    let (journal, error, _cleanup) = journal
        .try_prepare_publication(&state, |_, target| {
            assert_writers_released_except(target, None);
            // Same visible value, different actual current/undo identity in the
            // final acquisition; every earlier prepared field must be aborted.
            target.lane_consensus_contexts.block().commit();
            Ok::<_, Infallible>(ResourceGuard {
                state: &state,
                released: Arc::clone(&installation),
                except: None,
            })
        })
        .err()
        .expect("late exact identity mismatch");
    drop(_cleanup);
    assert!(matches!(
        error,
        RuntimePublicationError::Component {
            field: "lane_consensus_contexts",
            cause: PublicationPreparationError::Changed,
        }
    ));
    assert_eq!(original_topology_allocation(&journal), pointer);
    assert!(installation.load(Ordering::SeqCst));
    assert!(!retained.load(Ordering::SeqCst));
    let after = images(&state);
    assert_eq!(after.canonical_runtime, before.canonical_runtime);
    assert_eq!(after.commit_topology, before.commit_topology);
    assert_eq!(after.prev_commit_topology, before.prev_commit_topology);
    assert_eq!(
        after.lane_consensus_contexts.0,
        before.lane_consensus_contexts.0
    );
    assert_eq!(after.lane_consensus_contexts.1, None);
    assert!(!journal.matches_current(&state));
    assert!(
        journal
            .canonical_runtime
            .matches_current(&state.canonical_runtime)
    );
    drop(journal);
    assert!(retained.load(Ordering::SeqCst));
}

#[test]
fn runtime_installation_refusal_precedes_all_writer_acquisition_and_preserves_original() {
    let state = fixture();
    let before = images(&state);
    let mut original = blocks(&state, BlockMode::Ordinary);
    mutate(&mut original, 2);
    let journal = capture(original, ());
    let pointer = original_topology_allocation(&journal);
    let mut called = 0;
    let (journal, error, _cleanup) = journal
        .try_prepare_publication(&state, |candidate, target| {
            called += 1;
            assert_eq!(original_topology_allocation(candidate), pointer);
            assert_writers_released_except(target, None);
            Err::<(), _>("bounded installation exhausted")
        })
        .err()
        .expect("required capacity refusal");
    drop(_cleanup);
    assert_eq!(called, 1);
    assert!(matches!(
        error,
        RuntimePublicationError::Admission("bounded installation exhausted")
    ));
    assert_eq!(original_topology_allocation(&journal), pointer);
    assert!(journal.matches_current(&state));
    assert_eq!(images(&state), before);
    prepare(journal, &state).publish();
    assert_ne!(images(&state), before);
}

#[test]
fn runtime_capture_and_installation_guards_outlive_all_writers_on_drop_abort_and_publish() {
    for operation in 0..3 {
        let state = fixture();
        let before = images(&state);
        let retained = Arc::new(AtomicBool::new(false));
        let installation = Arc::new(AtomicBool::new(false));
        let mut original = blocks(&state, BlockMode::Ordinary);
        mutate(&mut original, 2);
        let journal = capture(
            original,
            ResourceGuard {
                state: &state,
                released: Arc::clone(&retained),
                except: None,
            },
        );
        let prepared = journal
            .try_prepare_publication(&state, |_, _| {
                Ok::<_, Infallible>(ResourceGuard {
                    state: &state,
                    released: Arc::clone(&installation),
                    except: None,
                })
            })
            .unwrap_or_else(|_| panic!("prepare original runtime journals"));
        match operation {
            0 => drop(prepared),
            1 => {
                let journal = prepared.abort().0;
                assert!(installation.load(Ordering::SeqCst));
                assert!(!retained.load(Ordering::SeqCst));
                assert!(journal.matches_current(&state));
                drop(journal);
            }
            _ => {
                let guards = prepared.publish();
                assert!(!installation.load(Ordering::SeqCst));
                assert!(!retained.load(Ordering::SeqCst));
                assert_writers_released_except(&state, None);
                assert_ne!(images(&state), before);
                drop(guards);
            }
        }
        assert!(retained.load(Ordering::SeqCst));
        assert!(installation.load(Ordering::SeqCst));
        assert_writers_released_except(&state, None);
        if operation != 2 {
            assert_eq!(images(&state), before);
        }
    }
}

#[test]
fn runtime_equal_values_from_another_state_do_not_supply_original_publication_identity() {
    let state = fixture();
    let foreign = fixture();
    assert_eq!(images(&state), images(&foreign));
    let mut original = blocks(&state, BlockMode::Ordinary);
    mutate(&mut original, 2);
    let journal = capture(original, ());
    let (journal, error, _cleanup) = journal
        .try_prepare_publication(&foreign, |_, _| Ok::<_, Infallible>(()))
        .err()
        .expect("equal foreign owner");
    drop(_cleanup);
    assert!(matches!(
        error,
        RuntimePublicationError::Component {
            field: "canonical_runtime",
            cause: PublicationPreparationError::Changed,
        }
    ));
    assert!(journal.matches_current(&state));
    assert_writers_released_except(&foreign, None);
    prepare(journal, &state).publish();
    assert_ne!(images(&state), images(&foreign));
}

#[test]
fn runtime_abandonment_releases_every_component_before_callbacks_and_capacity() {
    use std::{
        future::Future,
        pin::Pin,
        sync::Mutex,
        task::{Context, Wake, Waker},
    };

    struct Capacity(Arc<AtomicUsize>, usize);
    impl Drop for Capacity {
        fn drop(&mut self) {
            self.0.fetch_or(self.1, Ordering::SeqCst);
        }
    }
    struct Reenter {
        target: Arc<State>,
        journals: Mutex<Option<RuntimeJournals<()>>>,
        released: Arc<AtomicUsize>,
        checks: AtomicUsize,
        failures: AtomicUsize,
        wakes: AtomicUsize,
        unwind: bool,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::SeqCst);
            if self.released.load(Ordering::SeqCst) != 0 {
                self.failures.fetch_add(1, Ordering::SeqCst);
            }
            let journals = self
                .journals
                .lock()
                .unwrap()
                .take()
                .expect("one original wake");
            // Probe each original component independently: a poisoned earlier
            // component must not hide a later sibling whose lock is still held.
            macro_rules! check {
                ($field:ident) => {{
                    match journals
                        .$field
                        .try_prepare_publication(&self.target.$field, |_, _| Ok::<_, ()>(()))
                    {
                        Ok(prepared) => drop(prepared.abort()),
                        Err((_, PublicationPreparationError::Poisoned, _)) if self.unwind => {}
                        Err(_) => {
                            self.failures.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    self.checks.fetch_add(1, Ordering::SeqCst);
                }};
            }
            check!(canonical_runtime);
            check!(commit_topology);
            check!(prev_commit_topology);
            check!(lane_consensus_contexts);
        }
    }

    for unwind in [false, true] {
        for component in 0..4 {
            let released = Arc::new(AtomicUsize::new(0));
            let target: Arc<State> = Arc::from(fixture());
            let before = images(&target);
            let watcher = capture(blocks(&target, BlockMode::Ordinary), ());
            let reentry = capture(blocks(&target, BlockMode::Ordinary), ());
            let mut block = blocks(&target, BlockMode::Ordinary);
            mutate(&mut block, 2);
            let journal = capture(block, Capacity(Arc::clone(&released), 1));
            let prepared = journal
                .try_prepare_publication(&target, |_, _| {
                    Ok::<_, ()>(Capacity(Arc::clone(&released), 2))
                })
                .unwrap_or_else(|_| panic!("prepare original aggregate"));
            macro_rules! observe {
                ($field:ident) => {{
                    let (_, error, cleanup) = watcher
                        .$field
                        .try_prepare_publication(&target.$field, |_, _| Ok::<_, ()>(()))
                        .err()
                        .expect("original prepared component held");
                    drop(cleanup);
                    let PublicationPreparationError::Busy(wait) = error else {
                        panic!("expected original prepared identity contention");
                    };
                    wait.wait_for_release()
                }};
            }
            let mut wait = match component {
                0 => observe!(canonical_runtime),
                1 => observe!(commit_topology),
                2 => observe!(prev_commit_topology),
                3 => observe!(lane_consensus_contexts),
                _ => unreachable!(),
            };
            let callback = Arc::new(Reenter {
                target: Arc::clone(&target),
                journals: Mutex::new(Some(reentry)),
                released: Arc::clone(&released),
                checks: AtomicUsize::new(0),
                failures: AtomicUsize::new(0),
                wakes: AtomicUsize::new(0),
                unwind,
            });
            let waker = Waker::from(Arc::clone(&callback));
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            if unwind {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _original = prepared;
                    panic!("abandon original aggregate during unwind");
                }));
                assert!(result.is_err());
            } else {
                drop(prepared);
                assert_eq!(images(&target), before);
            }
            assert_eq!(
                callback.wakes.load(Ordering::SeqCst),
                1,
                "component {component}, unwind {unwind}"
            );
            assert_eq!(callback.checks.load(Ordering::SeqCst), 4);
            assert_eq!(
                callback.failures.load(Ordering::SeqCst),
                0,
                "component {component}, unwind {unwind}"
            );
            assert_eq!(released.load(Ordering::SeqCst), 3);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_ready()
            );
        }
    }
}

#[path = "runtime_capture_tests.rs"]
mod capture_tests;

#[path = "runtime_publication_slot_tests.rs"]
mod slot_tests;
