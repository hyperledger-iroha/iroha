//! The host seam of the simulator (spec §13.5): the node program the world runs for each
//! replica, behind the [`Host`] trait.
//!
//! A host owns what a production driver owns between the hardware and the core: the ingress
//! queues with the O5 priorities and the O6/O8 bounds and classes, the consensus core itself,
//! and the O1/O2 persist-before-effect barrier. The world keeps everything else as fake
//! backends — network, clocks, the write device and record store with the installation log,
//! the body store, the block store with O3 apply, the O4 executor, the payload builder — and
//! assembles the `Init` of every (re)start from the durable stores.
//!
//! The default host, [`FakeHost`], is the simulator's fake driver. An external node
//! implementation (the production driver of `iroha_core`, run against the world as its
//! hardware) is another [`Host`], chosen per replica by [`Scenario::host`](super::Scenario).
//!
//! TODO(WP4): when the production driver's persistence, serving or `Init` assembly are to run
//! in the simulator too, move them behind this seam as backend traits of the same shape.

use super::driver::{Barrier, Lanes};
use crate::{
    Core,
    api::{Action, ConfigError, Event, Init, LocalParams},
    crypto::{Attestation, Crypto, Signer},
    message::{TrafficClass, WireMessage},
    types::{Millis, PublicKey},
};

/// What the world hands a host that (re)starts: the core's configuration and startup input
/// (§12.1), assembled by the world from the machine's durable stores, and the machine profile's
/// ingress mode.
pub struct Start {
    /// Local parameters (§12.4).
    pub local: LocalParams,
    /// Startup input built from the durable stores (§7.4).
    pub init: Init,
    /// The configured signing keys.
    pub signers: Vec<Box<dyn Signer>>,
    /// Crypto (counting, with provenance).
    pub crypto: Box<dyn Crypto>,
    /// The commit-attestation extension (§3.7).
    pub attestation: Attestation,
    /// Local time of the start.
    pub now: Millis,
    /// One FIFO for all ingress, ticks behind queued messages (the ML12 fault, F29 control).
    pub fifo_ingress: bool,
}

/// A node implementation hosted by the simulated world for one replica (§13.5). The world
/// calls it from its single-threaded scheduler; every method returns at once.
pub trait Host {
    /// Start (or restart after a crash) from `start`; returns the start-up actions, which the
    /// world executes like those of [`Host::handle`].
    ///
    /// # Errors
    /// The core refused its configuration.
    fn start(&mut self, start: Start) -> Result<Vec<Action>, ConfigError>;
    /// Crash: lose the core, every queued input and every held effect.
    fn crash(&mut self);
    /// Whether the node is running (started and not crashed).
    fn running(&self) -> bool;
    /// A network message arrived from the authenticated peer `from` (its O8 class attached).
    fn receive(&mut self, from: PublicKey, msg: WireMessage, class: TrafficClass);
    /// A local event arrived (executor, builder, block store, body store); never dropped (O6).
    fn deliver(&mut self, event: Event);
    /// Whether an input is queued.
    fn has_input(&self) -> bool;
    /// The next input to handle at local time `now`: the due `Tick` first (O5), then the queued
    /// inputs by priority. `None` if there is nothing to do.
    fn next_input(&mut self, now: Millis) -> Option<Event>;
    /// Handle one input; the world executes the returned actions in order (O1) through
    /// [`Host::persisting`] and [`Host::gate`].
    fn handle(&mut self, now: Millis, event: Event) -> Vec<Action>;
    /// Local time of the next wanted `Tick` (`Millis::MAX` for none).
    fn next_wakeup(&self) -> Millis;
    /// The `PersistSafety` of write `write` went to the write device (O2).
    fn persisting(&mut self, write: u64);
    /// An externally visible effect the node wants to take place: `Some` to perform it now,
    /// `None` if it is held behind a pending record (O2).
    fn gate(&mut self, effect: Action) -> Option<Action>;
    /// The writes up to `write` are durable: the held effects to perform now, in order.
    fn durable(&mut self, write: u64) -> Vec<Action>;
    /// The node's core, for the oracles' read-only observations.
    fn core(&self) -> Option<&Core>;
    /// The effects held behind the O2 barrier, in order (an observation for tests).
    fn held(&self) -> Vec<Action>;
    /// Messages dropped by the ingress bounds (O6).
    fn ingress_drops(&self) -> u64;
}

/// Creates the host of a replica: `(machine, instance index)` → host.
pub type HostFactory = fn(usize, usize) -> Box<dyn Host>;

/// The default [`HostFactory`]: every replica runs a [`FakeHost`].
pub fn fake_host(_machine: usize, _instance: usize) -> Box<dyn Host> {
    Box::new(FakeHost::default())
}

/// The simulator's fake driver as a host: ingress [`Lanes`], the core and the O2 [`Barrier`].
#[derive(Default)]
pub struct FakeHost {
    core: Option<Core>,
    lanes: Lanes,
    barrier: Barrier,
}

impl Host for FakeHost {
    fn start(&mut self, start: Start) -> Result<Vec<Action>, ConfigError> {
        let (core, actions) = Core::new(
            start.local,
            start.init,
            start.signers,
            start.crypto,
            start.attestation,
            start.now,
        )?;
        self.core = Some(core);
        self.lanes.fifo = start.fifo_ingress;
        Ok(actions)
    }

    fn crash(&mut self) {
        self.core = None;
        self.lanes.clear();
        self.barrier.clear();
    }

    fn running(&self) -> bool {
        self.core.is_some()
    }

    fn receive(&mut self, from: PublicKey, msg: WireMessage, class: TrafficClass) {
        self.lanes.push_message(from, msg, class);
    }

    fn deliver(&mut self, event: Event) {
        self.lanes.push_local(event);
    }

    fn has_input(&self) -> bool {
        !self.lanes.is_empty()
    }

    fn next_input(&mut self, now: Millis) -> Option<Event> {
        let core = self.core.as_ref()?;
        let tick_first = !self.lanes.fifo || self.lanes.is_empty();
        if core.next_wakeup() <= now && tick_first {
            return Some(Event::Tick);
        }
        self.lanes.pop()
    }

    fn handle(&mut self, now: Millis, event: Event) -> Vec<Action> {
        self.core
            .as_mut()
            .map_or_else(Vec::new, |core| core.handle(now, event))
    }

    fn next_wakeup(&self) -> Millis {
        self.core.as_ref().map_or(Millis::MAX, Core::next_wakeup)
    }

    fn persisting(&mut self, write: u64) {
        self.barrier.persisting(write);
    }

    fn gate(&mut self, effect: Action) -> Option<Action> {
        self.barrier.hold(effect)
    }

    fn durable(&mut self, write: u64) -> Vec<Action> {
        self.barrier.release(write)
    }

    fn core(&self) -> Option<&Core> {
        self.core.as_ref()
    }

    fn held(&self) -> Vec<Action> {
        self.barrier.held.iter().map(|(_, a)| a.clone()).collect()
    }

    fn ingress_drops(&self) -> u64 {
        self.lanes.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::HaltReason,
        message::{BlockRequest, WireMessage},
        sim::{Scenario, World},
        types::Hash32,
    };

    /// The fake driver behind the seam: ingress priorities (a due `Tick` first, local events
    /// before messages), the O2 barrier and a crash that loses the core, the queues and the held
    /// effects.
    #[test]
    fn fake_host_ingress_barrier_and_crash() {
        let mut world = World::new(Scenario::base("host", 1, 4));
        let host = &mut world.replicas[0].host;
        assert!(host.running() && host.core().is_some());
        let wake = host.next_wakeup();
        assert!(wake < Millis::MAX);
        let key = PublicKey::new(vec![7; 32]).unwrap();
        let request = WireMessage::BlockRequest(BlockRequest {
            instance: Hash32::ZERO,
            height: 1,
            block_hash: Hash32::ZERO,
        });
        host.receive(key.clone(), request.clone(), TrafficClass::Control);
        host.deliver(Event::PayloadReady { req: 99 });
        assert!(host.has_input());
        assert_eq!(host.next_input(wake), Some(Event::Tick), "a due Tick first");
        host.handle(wake, Event::Tick);
        let now = wake;
        assert!(host.next_wakeup() > now, "the Tick consumed its deadline");
        assert_eq!(host.next_input(now), Some(Event::PayloadReady { req: 99 }));
        assert_eq!(
            host.next_input(now),
            Some(Event::Message {
                from: key,
                msg: request
            })
        );
        assert!(!host.has_input());
        assert_eq!(host.next_input(now), None);
        assert!(host.handle(now, Event::PayloadReady { req: 99 }).is_empty());
        // O2: effects wait for the pending record and leave in order once it is durable.
        let effect = Action::Halt(HaltReason::DriverAnomaly);
        assert!(host.gate(effect.clone()).is_some(), "no pending record");
        host.persisting(5);
        assert!(host.gate(effect.clone()).is_none());
        assert_eq!(host.held(), vec![effect.clone()]);
        assert!(host.durable(4).is_empty());
        assert_eq!(host.durable(5), vec![effect.clone()]);
        host.persisting(6);
        assert!(host.gate(effect).is_none());
        assert_eq!(host.ingress_drops(), 0);
        host.crash();
        assert!(!host.running() && host.core().is_none() && host.held().is_empty());
        assert_eq!(host.next_wakeup(), Millis::MAX);
        assert_eq!(host.next_input(Millis::MAX), None);
        assert!(host.handle(0, Event::Tick).is_empty());
    }

    /// A host that refuses its configuration reports the error.
    #[test]
    fn fake_host_start_errors() {
        let world = World::new(Scenario::base("host", 1, 4));
        let mut init = world.init_for(0);
        init.demotion_window = 0;
        let start = Start {
            local: world.instances[0].local,
            init,
            signers: Vec::new(),
            crypto: Box::new(world.replicas[0].crypto.clone()),
            attestation: Attestation::none(),
            now: 0,
            fifo_ingress: false,
        };
        let mut host = FakeHost::default();
        assert!(host.start(start).is_err());
        assert!(!host.running());
        assert!(
            fake_host(0, 0).core().is_none(),
            "a new host is not running"
        );
    }
}
