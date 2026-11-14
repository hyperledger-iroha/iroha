//! Scripted stages: ordering, overlap, joins and leg-ordered failure, without native proofs.

use super::*;
use std::{
    cell::RefCell,
    sync::{Arc, Condvar},
    thread::ThreadId,
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Admit(usize),
    ProveStart(usize),
    ProveEnd(usize),
    HelperTlsGone(ThreadId),
    Complete(usize),
}

/// Shared script: an event log plus a gate that opens only when enough provers overlap.
struct Script {
    legs: usize,
    // For each leg, how many provers must have entered before its own may proceed.
    gates: Vec<usize>,
    patience: Duration,
    events: Mutex<Vec<Event>>,
    // Provers entered so far, provers inside `prove` now, and the widest overlap seen.
    proving: Mutex<(usize, usize, usize)>,
    changed: Condvar,
    caller: ThreadId,
}

struct HelperTls(Arc<Script>);
impl Drop for HelperTls {
    fn drop(&mut self) {
        self.0
            .record(Event::HelperTlsGone(std::thread::current().id()));
    }
}
std::thread_local! {
    static HELPER_TLS: RefCell<Option<HelperTls>> = const { RefCell::new(None) };
}

impl Script {
    /// `gates[leg]` provers must be proving together before `leg`'s prover returns.
    fn new(gates: &[usize]) -> Arc<Self> {
        Self::with_patience(gates, Duration::from_secs(10))
    }
    fn with_patience(gates: &[usize], patience: Duration) -> Arc<Self> {
        Arc::new(Self {
            legs: gates.len(),
            gates: gates.to_vec(),
            patience,
            events: Mutex::new(Vec::new()),
            proving: Mutex::new((0, 0, 0)),
            changed: Condvar::new(),
            caller: std::thread::current().id(),
        })
    }
    fn record(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }
    fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }
    fn position(&self, event: &Event) -> usize {
        self.events()
            .iter()
            .position(|recorded| recorded == event)
            .unwrap_or_else(|| panic!("{event:?} was never recorded"))
    }
    /// Greatest number of provers that were inside `prove` together.
    fn widest(&self) -> usize {
        self.proving.lock().unwrap().2
    }
    // Enter a prover and wait until its gate's provers have all entered: a bounded gate
    // makes leg-by-leg proving fail instead of deadlocking the suite.
    fn enter(self: &Arc<Self>, index: usize) -> Result<(), String> {
        self.record(Event::ProveStart(index));
        if std::thread::current().id() != self.caller {
            HELPER_TLS.with(|slot| {
                slot.borrow_mut()
                    .get_or_insert_with(|| HelperTls(Arc::clone(self)));
            });
        }
        let mut proving = self.proving.lock().unwrap();
        proving.0 += 1;
        proving.1 += 1;
        proving.2 = proving.2.max(proving.1);
        let gate = self.gates[index];
        self.changed.notify_all();
        let (mut entered, _) = self
            .changed
            .wait_timeout_while(proving, self.patience, |proving| proving.0 < gate)
            .unwrap();
        if entered.0 < gate {
            // This prover leaves `prove` here, without reaching `leave`.
            entered.1 -= 1;
            return Err(format!("leg {index} proved without its window"));
        }
        Ok(())
    }
    fn leave(&self, index: usize) {
        self.proving.lock().unwrap().1 -= 1;
        self.record(Event::ProveEnd(index));
    }
}

/// Run the pipeline with every leg admitted; `refuse` scripts one stage's result per leg.
fn run(
    script: &Arc<Script>,
    helpers: &ProverHelpers,
    refuse: impl Fn(&'static str, usize) -> Option<String> + Sync,
) -> Result<Option<Vec<usize>>, String> {
    run_legs(
        script.legs,
        helpers,
        |index| {
            assert_eq!(std::thread::current().id(), script.caller);
            script.record(Event::Admit(index));
            match refuse("admit", index).as_deref() {
                Some("panic") => panic!("scripted wallet panic at leg {index}"),
                Some("absent") => Ok(Admission::Absent),
                Some("reopened") => Ok(Admission::Reopened(index + 100)),
                Some(error) => Err(error.to_owned()),
                None => Ok(Admission::Admitted(index)),
            }
        },
        |index, job: usize| {
            assert_eq!(job, index, "a prover receives exactly its own admitted leg");
            script.enter(index)?;
            let refusal = refuse("prove", index);
            script.leave(index);
            if refusal.as_deref() == Some("panic") {
                panic!("scripted prover panic at leg {index}");
            }
            refusal.map_or(Ok(index + 200), Err)
        },
        |index, ready| {
            assert_eq!(std::thread::current().id(), script.caller);
            script.record(Event::Complete(index));
            if let Some(error) = refuse("complete", index) {
                return Err(error);
            }
            Ok(match ready {
                Ready::Reopened(original) => original,
                Ready::Proved(proof) => proof,
            })
        },
    )
}

fn free(helpers: &ProverHelpers) -> usize {
    helpers.0.load(Ordering::SeqCst)
}

#[test]
fn admitted_legs_prove_together_between_ordered_admission_and_completion() {
    // The gate refuses unless all three provers are inside `prove` at once.
    let script = Script::new(&[3, 3, 3]);
    let helpers = ProverHelpers::new(LEG_WINDOW - 1);
    let proofs = run(&script, &helpers, |_, _| None)
        .expect("every leg proves while the others are still proving")
        .expect("no leg is absent");
    assert_eq!(proofs, vec![200, 201, 202], "results keep leg order");
    assert_eq!(script.widest(), 3);
    let events = script.events();
    // Admission of the whole window precedes every proof, in leg order.
    assert_eq!(
        events[..3],
        [Event::Admit(0), Event::Admit(1), Event::Admit(2)]
    );
    // Proof first: nothing is completed until every prover has finished and joined.
    let first_complete = script.position(&Event::Complete(0));
    for leg in 0..3 {
        assert!(script.position(&Event::ProveEnd(leg)) < first_complete);
    }
    let gone = events[..first_complete]
        .iter()
        .filter(|event| matches!(event, Event::HelperTlsGone(_)))
        .count();
    assert_eq!(
        gone, 2,
        "both helper threads, thread-locals included, exited before any proof was used"
    );
    assert_eq!(
        events[first_complete..],
        [Event::Complete(0), Event::Complete(1), Event::Complete(2)],
        "completion is leg-ordered and nothing else follows it"
    );
    assert_eq!(free(&helpers), LEG_WINDOW - 1, "helpers are returned");
}

#[test]
fn an_exhausted_helper_allowance_proves_one_leg_at_a_time_on_the_original_worker() {
    // The same script with no helper is the former leg-by-leg behaviour.
    let script = Script::with_patience(&[3, 3, 3], Duration::from_millis(200));
    let helpers = ProverHelpers::new(0);
    let refused = run(&script, &helpers, |_, _| None).unwrap_err();
    assert_eq!(refused, "leg 0 proved without its window");
    assert_eq!(script.widest(), 1);
    // Without the overlap requirement it completes, still one proof at a time.
    let script = Script::new(&[0, 0, 0]);
    let proofs = run(&script, &helpers, |_, _| None).unwrap().unwrap();
    assert_eq!(proofs, vec![200, 201, 202]);
    assert_eq!(script.widest(), 1);
    assert!(
        !script
            .events()
            .iter()
            .any(|event| matches!(event, Event::HelperTlsGone(_))),
        "every proof ran on the original worker"
    );
    // Another operation holding part of the allowance narrows this one, never blocks it.
    let helpers = ProverHelpers::new(2);
    let held = helpers.try_acquire().unwrap();
    let script = Script::new(&[2, 2, 0]);
    run(&script, &helpers, |_, _| None).unwrap().unwrap();
    assert_eq!(script.widest(), 2);
    assert_eq!(free(&helpers), 1);
    drop(held);
    assert_eq!(free(&helpers), 2);
}

#[test]
fn a_later_window_is_admitted_only_after_the_previous_one_is_complete() {
    // Leg 1 is reopened, so the first window proves three legs and the second two.
    let script = Script::new(&[3, 0, 3, 3, 5, 5]);
    assert_eq!(script.legs, LEG_WINDOW + 2);
    // More helpers than a window could use: the window, not the allowance, is the bound.
    let helpers = ProverHelpers::new(2 * LEG_WINDOW);
    let proofs = run(&script, &helpers, |stage, index| {
        (stage == "admit" && index == 1).then(|| "reopened".to_owned())
    })
    .unwrap()
    .unwrap();
    assert_eq!(proofs, vec![200, 101, 202, 203, 204, 205]);
    assert_eq!(
        script.widest(),
        LEG_WINDOW - 1,
        "the reopened leg is not proved again"
    );
    assert!(
        !script.events().contains(&Event::ProveStart(1)),
        "a durable original is never proved again"
    );
    let next_window = script.position(&Event::Admit(LEG_WINDOW));
    for leg in 0..LEG_WINDOW {
        assert!(script.position(&Event::Complete(leg)) < next_window);
    }
    assert_eq!(free(&helpers), 2 * LEG_WINDOW);
}

#[test]
fn the_first_leg_in_order_decides_and_the_legs_before_it_are_complete() {
    let helpers = ProverHelpers::new(LEG_WINDOW - 1);
    let completed = |script: &Script| {
        script
            .events()
            .into_iter()
            .filter_map(|event| match event {
                Event::Complete(leg) => Some(leg),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    // A refused admission stops admission; the admitted legs before it still finish.
    let script = Script::new(&[2, 2, 0, 0]);
    let error = run(&script, &helpers, |stage, index| {
        (stage == "admit" && index == 2).then(|| "wallet refused leg 2".to_owned())
    })
    .unwrap_err();
    assert_eq!(error, "wallet refused leg 2");
    assert!(
        !script.events().contains(&Event::Admit(3)),
        "no wallet call follows a refusal"
    );
    assert_eq!(completed(&script), [0, 1]);

    // An absent leg is reported only after the legs before it are complete.
    let script = Script::new(&[0, 0, 0]);
    let absent = run(&script, &helpers, |stage, index| {
        (stage == "admit").then(|| (if index == 1 { "absent" } else { "reopened" }).to_owned())
    })
    .unwrap();
    assert_eq!(absent, None);
    assert_eq!(completed(&script), [0]);
    assert!(!script.events().contains(&Event::Admit(2)));

    // A refused proof: the leg before it is completed, the joined later proof is dropped.
    let script = Script::new(&[3, 3, 3]);
    let error = run(&script, &helpers, |stage, index| {
        (stage == "prove" && index == 1).then(|| "proof refused at leg 1".to_owned())
    })
    .unwrap_err();
    assert_eq!(error, "proof refused at leg 1");
    assert_eq!(completed(&script), [0]);
    assert!(
        script.events().contains(&Event::ProveEnd(2)),
        "the later prover was joined"
    );

    // A failed completion publishes nothing after it.
    let script = Script::new(&[3, 3, 3]);
    let error = run(&script, &helpers, |stage, index| {
        (stage == "complete" && index == 1).then(|| "budget exhausted at leg 1".to_owned())
    })
    .unwrap_err();
    assert_eq!(error, "budget exhausted at leg 1");
    assert_eq!(completed(&script), [0, 1]);

    // An earlier ordinary refusal outranks a later panic and a later admission refusal.
    let script = Script::new(&[3, 3, 3, 0]);
    let error = run(&script, &helpers, |stage, index| match (stage, index) {
        ("prove", 1) => Some("proof refused at leg 1".to_owned()),
        ("prove", 2) => Some("panic".to_owned()),
        ("admit", 3) => Some("wallet refused leg 3".to_owned()),
        _ => None,
    })
    .unwrap_err();
    assert_eq!(error, "proof refused at leg 1");
    assert_eq!(completed(&script), [0]);
    assert_eq!(free(&helpers), LEG_WINDOW - 1);
}

#[test]
fn a_prover_panic_resumes_on_the_original_worker_after_earlier_legs_complete() {
    let script = Script::new(&[3, 3, 3]);
    let helpers = ProverHelpers::new(LEG_WINDOW - 1);
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        run(&script, &helpers, |stage, index| {
            (stage == "prove" && index == 1).then(|| "panic".to_owned())
        })
    }))
    .expect_err("the original panic must not become an ordinary refusal");
    assert_eq!(
        unwound.downcast_ref::<String>().map(String::as_str),
        Some("scripted prover panic at leg 1"),
        "the original payload is preserved"
    );
    let events = script.events();
    assert!(events.contains(&Event::Complete(0)));
    assert!(!events.contains(&Event::Complete(1)));
    assert!(!events.contains(&Event::Complete(2)));
    for leg in 0..3 {
        assert!(
            events.contains(&Event::ProveEnd(leg)),
            "every prover was joined before the panic resumed"
        );
    }
    assert_eq!(free(&helpers), LEG_WINDOW - 1);
}

#[test]
fn an_admission_panic_resumes_after_the_admitted_legs_before_it_are_complete() {
    let script = Script::new(&[2, 2, 0, 0]);
    let helpers = ProverHelpers::new(LEG_WINDOW - 1);
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        run(&script, &helpers, |stage, index| {
            (stage == "admit" && index == 2).then(|| "panic".to_owned())
        })
    }))
    .expect_err("the original wallet panic must not become an ordinary refusal");
    assert_eq!(
        unwound.downcast_ref::<String>().map(String::as_str),
        Some("scripted wallet panic at leg 2")
    );
    let events = script.events();
    assert!(
        !events.contains(&Event::Admit(3)),
        "no wallet call follows the panic"
    );
    assert!(events.contains(&Event::Complete(0)) && events.contains(&Event::Complete(1)));
    assert_eq!(free(&helpers), LEG_WINDOW - 1);
}

#[test]
fn the_process_allowance_adds_at_most_one_window_of_helpers() {
    assert_eq!(LEG_WINDOW, 4);
    // Other tests may be proving: the shared allowance never exceeds its bound.
    assert!(free(&PROCESS_PROVER_HELPERS) <= LEG_WINDOW - 1);
}
