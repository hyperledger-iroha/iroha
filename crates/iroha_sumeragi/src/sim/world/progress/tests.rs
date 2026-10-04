//! Regressions proving observation never purchases extra time for a stalled implementation.

use super::*;
use crate::sim::{
    host::{FakeHost, Host, Start},
    scenario::Scenario,
};

/// A deliberately broken host that loses every outgoing message while continuing core steps
/// and durable writes. The original core remains running and signable, so O-LIVE still applies.
#[derive(Default)]
struct NoncommittingHost(FakeHost);

impl Host for NoncommittingHost {
    fn start(
        &mut self,
        start: Start,
    ) -> Result<Vec<crate::api::Action>, Box<dyn std::error::Error>> {
        self.0.start(start)
    }
    fn crash(&mut self) {
        self.0.crash();
    }
    fn running(&self) -> bool {
        self.0.running()
    }
    fn receive(
        &mut self,
        from: crate::types::PublicKey,
        msg: crate::message::WireMessage,
        class: crate::message::TrafficClass,
    ) {
        self.0.receive(from, msg, class);
    }
    fn deliver(&mut self, event: crate::api::Event) {
        self.0.deliver(event);
    }
    fn has_input(&self) -> bool {
        self.0.has_input()
    }
    fn next_input(&mut self, now: Millis) -> Option<crate::api::Event> {
        self.0.next_input(now)
    }
    fn handle(&mut self, now: Millis, event: crate::api::Event) -> Vec<crate::api::Action> {
        self.0.handle(now, event)
    }
    fn next_wakeup(&self) -> Millis {
        self.0.next_wakeup()
    }
    fn persisting(&mut self, write: u64) {
        self.0.persisting(write);
    }
    fn gate(&mut self, effect: crate::api::Action) -> Option<crate::api::Action> {
        if matches!(
            effect,
            crate::api::Action::Send { .. } | crate::api::Action::Broadcast { .. }
        ) {
            None
        } else {
            self.0.gate(effect)
        }
    }
    fn durable(&mut self, write: u64) -> Vec<crate::api::Action> {
        self.0.durable(write)
    }
    fn core(&self) -> Option<&crate::Core> {
        self.0.core()
    }
    fn held(&self) -> Vec<crate::api::Action> {
        self.0.held()
    }
    fn ingress_drops(&self) -> u64 {
        self.0.ingress_drops()
    }
}

fn stalled_host() -> World {
    let mut sc = Scenario::base("stalled host observation", 0, 4);
    sc.duration = 1_000;
    sc.checks.progress = 8;
    sc.checks.complete_progress = true;
    sc.host = |_, _| Box::new(NoncommittingHost::default());
    let mut world = World::new(sc);
    world.run_until(world.duration);
    assert!(world.failure.is_none(), "{:?}", world.failure);
    assert!(world.live_precondition(0));
    assert!(world.honest().iter().all(|r| world.committed(*r) == 0));
    world
}

#[test]
fn f04_progress_stalled_implementation_fails_at_original_live_deadline() {
    let mut world = stalled_host();
    let original: Vec<_> = world.oracle.reps.iter().map(|obs| obs.deadline).collect();
    let first = *original.iter().min().unwrap();
    assert!(first > world.duration);
    let report = world
        .run()
        .expect_err("noncommitting implementation must fail");
    assert!(report.contains("O-LIVE"), "{report}");
    assert_eq!(world.now, first + 1);
    assert_eq!(world.duration, 1_000);
    assert_eq!(
        world
            .oracle
            .reps
            .iter()
            .map(|obs| obs.deadline)
            .collect::<Vec<_>>(),
        original
    );
    assert!(world.honest().iter().all(|r| world.committed(*r) == 0));
}

#[test]
fn f04_progress_empty_event_queue_fails_instead_of_passing_or_waiting_forever() {
    let mut world = stalled_host();
    let first = world
        .oracle
        .reps
        .iter()
        .map(|obs| obs.deadline)
        .min()
        .unwrap();
    world.queue.clear();
    world.ready.fill(Millis::MAX);
    let report = world
        .run()
        .expect_err("no events cannot meet a progress obligation");
    assert!(report.contains("captured deadline"), "{report}");
    assert_eq!(world.now, first + 1);
    assert_eq!(world.duration, 1_000);
}

#[test]
fn f04_progress_changed_oracle_window_cannot_move_captured_deadline() {
    let mut world = World::new(Scenario::base("fixed observation", 0, 4));
    let mut watch = ProgressWatch {
        replica: 0,
        target: 5,
        committed: 3,
        deadline: 200,
    };
    let obs = &mut world.oracle.reps[0];
    obs.committed = 3;
    obs.deadline = 600; // Restarting a timer or precondition without a commit grants no time.
    world.now = 200;
    assert_eq!(watch.observe(&world), Ok(false), "deadline is inclusive");
    assert_eq!(watch.deadline, 200);
    world.now = 201;
    assert!(watch.observe(&world).unwrap_err().contains("deadline 200"));
    assert_eq!(watch.deadline, 200);
}

#[test]
fn f04_progress_only_timely_commits_advance_the_original_window() {
    let mut world = World::new(Scenario::base("commit observation", 0, 4));
    let mut watch = ProgressWatch {
        replica: 0,
        target: 5,
        committed: 3,
        deadline: 200,
    };
    let obs = &mut world.oracle.reps[0];
    obs.committed = 4;
    obs.last_commit = 200;
    obs.deadline = 600;
    world.now = 200;
    assert_eq!(watch.observe(&world), Ok(false));
    assert_eq!(watch.deadline, 600);
    assert_eq!(watch.committed, 4);
    world.oracle.reps[0].committed = 5;
    world.oracle.reps[0].last_commit = 601;
    world.oracle.reps[0].deadline = 900;
    world.now = 601;
    assert!(
        watch.observe(&world).is_err(),
        "late target commit must still fail"
    );
    world.oracle.reps[0].last_commit = 600;
    world.now = 600;
    assert_eq!(watch.observe(&world), Ok(true));
}
