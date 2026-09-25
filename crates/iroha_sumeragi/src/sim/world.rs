//! The simulated world (§13.1): instances, machines and their replicas (one core per machine
//! and instance), a single-threaded discrete-event scheduler in virtual milliseconds, and the
//! fake driver that executes each core's actions under the §12.3 guarantees.
//!
//! Scheduling: arrivals (network deliveries, write completions, executor and builder answers,
//! scripted faults) enter a global time-ordered queue. When a replica is idle it handles, in
//! O5 priority, a due `Tick`, then local events, then control, proposal and bulk messages. A
//! handled event costs virtual CPU time (per pairing counted by the crypto wrapper), during
//! which the replica is busy; its actions take effect at the end of that time.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    rc::Rc,
};

use super::{
    byz::Adversary,
    crypto::{SharedLog, SimCrypto, SimSigner},
    driver::{
        Clock, Executor, Io, Lanes, Write, decode_txs, divergent_exec, encode_tx, reference_exec,
    },
    net::{Fate, NetConfig, Nic, Packet, approx_size, class_of},
    oracle::Oracle,
    records::{KeyStore, StoreId},
    rng::{Rng, seed_of},
    scenario::{Checks, Churn, CrashPoint, Fault, Profile, Scenario, Workload},
};
use crate::{
    Core,
    api::{Action, CommittedTip, Event, ExecOutcome, HaltReason, Init, LocalParams},
    crypto::Signer,
    message::{Block, BlockRequest, BlockResponse, Qc, SyncEntry, SyncResponse, WireMessage},
    safety::{RecordState, SafetyRecord},
    testing::sha256,
    types::{ChainParams, Committee, Hash32, HeightConfig, Millis, PublicKey},
};

/// A consensus instance of the world (§1.8) with its committee schedule (§10).
#[derive(Clone, Debug)]
pub struct Inst {
    /// Instance id `I`.
    pub id: Hash32,
    /// Genesis block hash.
    pub genesis_hash: Hash32,
    /// Genesis result.
    pub genesis_result: Hash32,
    /// `(from height, committee)`, ascending, first entry from height 0.
    pub schedule: Vec<(u64, Committee)>,
    /// Chain parameters.
    pub params: ChainParams,
    /// Demotion window `W` (genesis constant).
    pub window: u64,
    /// Local parameters of every node.
    pub local: LocalParams,
}

impl Inst {
    /// `C_h`.
    pub fn committee(&self, height: u64) -> &Committee {
        let mut out = None;
        for (from, committee) in &self.schedule {
            if *from <= height || out.is_none() {
                out = Some(committee);
            }
        }
        out.unwrap_or_else(|| unreachable!("a schedule has at least one committee"))
    }

    /// The height configuration of `height`.
    pub fn config(&self, height: u64) -> HeightConfig {
        HeightConfig {
            committee: self.committee(height).clone(),
            params: self.params,
        }
    }
}

/// A simulated machine: a clock, resources, and one replica per instance it takes part in.
#[derive(Clone, Debug)]
pub struct Machine {
    /// Running (not crashed).
    pub up: bool,
    /// Incarnation counter; events of an older incarnation are lost.
    pub epoch: u64,
    /// Local clock.
    pub clock: Clock,
    /// Runs a Byzantine strategy.
    pub byz: bool,
    /// Replica index per instance (`None` if not a participant).
    pub replicas: Vec<Option<usize>>,
    /// Resources.
    pub profile: Profile,
    /// Time of the latest (re)start.
    pub started_at: Millis,
    /// Crashes so far.
    pub crashes: u32,
    /// Keys by slot.
    pub keys: Vec<PublicKey>,
    /// Key store with the installation log (§7.4 record provenance).
    pub keystore: KeyStore,
    /// The store-id file next to the record files (`None`: missing).
    pub store_id: Option<StoreId>,
    /// A backup of the key store (restored by `Fault::RestoreKeyStore`).
    pub snapshot: Option<KeyStore>,
    /// Keys no longer configured for signing (retired, §7.4 Keys).
    pub retired: BTreeSet<PublicKey>,
}

/// A durable safety record.
#[derive(Clone, Debug)]
pub struct Durable {
    /// The record.
    pub record: SafetyRecord,
    /// Its encoding.
    pub bytes: Vec<u8>,
}

/// One core instance on one machine with its fake driver state.
pub struct Replica {
    /// Machine.
    pub machine: usize,
    /// Instance index.
    pub inst: usize,
    /// Configured signing keys.
    pub keys: Vec<PublicKey>,
    /// The core (`None` while crashed).
    pub core: Option<Core>,
    /// Counting crypto shared with the core.
    pub crypto: SimCrypto,
    /// Durable safety-record files of this instance per key (part of the machine's record
    /// store; never backed up or restored).
    pub records: BTreeMap<PublicKey, Durable>,
    /// Durable body store.
    pub bodies: BTreeMap<Hash32, Block>,
    /// Durable block store (Kura), consecutive heights from `g + 1`.
    pub store: Vec<(Block, Qc)>,
    /// Ingress lanes.
    pub lanes: Lanes,
    /// Busy until (virtual CPU).
    pub busy_until: Millis,
    cpu_carry_us: u64,
    /// Write device and O2 barrier.
    pub io: Io,
    /// Executor.
    pub exec: Executor,
    /// Egress NIC: packets carry `(target replica, message)`.
    pub nic: Nic<(usize, Rc<WireMessage>)>,
    /// Transaction queue of the payload builder.
    pub txs: BTreeMap<u64, Vec<u8>>,
    /// Quarantined transactions.
    pub quarantine: BTreeSet<u64>,
    /// Applied state: `(height, block hash, result)`.
    pub applied: (u64, Hash32, Hash32),
    /// Current round height of the core (cached after every event).
    pub height: u64,
    /// Halt reason reported by the core.
    pub halted: Option<HaltReason>,
    /// Last record persisted per key (to classify crash points).
    last_persisted: BTreeMap<PublicKey, SafetyRecord>,
    /// Largest `Tick` lateness observed (local ms).
    pub max_tick_late: Millis,
    /// `(next_wakeup, local time it was first reported)`.
    wake_mark: (Millis, Millis),
    /// The builder's latest `BuildPayload` answered `EMPTY` whose `PayloadReady{req}` is still
    /// owed (at most once per request, §12.2).
    pub pending_ready: Option<u64>,
}

/// Time-ordered events of the scheduler.
enum Ev {
    Arrive {
        r: usize,
        from: PublicKey,
        msg: Rc<WireMessage>,
    },
    Local {
        r: usize,
        epoch: u64,
        event: Box<Event>,
    },
    IoDone {
        r: usize,
        epoch: u64,
        id: u64,
    },
    ExecDone {
        r: usize,
        epoch: u64,
        job: u64,
    },
    NicFree {
        r: usize,
        epoch: u64,
    },
    Script(usize),
    Restart(usize),
    TxGen(usize),
    ByzTick(usize),
    Evict(usize),
}

/// A compact trace entry (formatted only on failure).
#[derive(Clone, Debug)]
pub struct TraceEntry {
    /// Global time.
    pub t: Millis,
    /// Replica (or machine for crash/restart).
    pub who: usize,
    /// What happened.
    pub what: String,
}

/// Counters of a run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Events handled by cores.
    pub events: u64,
    /// Packets put on the wire, by class lane.
    pub packets: [u64; 3],
    /// Bytes put on the wire.
    pub bytes: u64,
    /// Packets lost by the network.
    pub lost: u64,
    /// Packets above the frame limit.
    pub oversize: u64,
    /// Crashes.
    pub crashes: u64,
    /// Evidence reports by honest nodes.
    pub evidence: u64,
    /// Proposals broadcast by honest leaders.
    pub proposals: u64,
}

/// Submitted transactions of an instance: id → (submitted at, poison, committed at).
pub type TxLog = BTreeMap<u64, (Millis, bool, Option<Millis>)>;

/// Maximum trace entries kept.
const TRACE_LEN: usize = 60;

/// The simulated world.
pub struct World {
    /// Scenario name.
    pub name: String,
    /// Seed.
    pub seed: u64,
    /// Global virtual time.
    pub now: Millis,
    /// The single PRNG of the run.
    pub rng: Rng,
    queue: BTreeMap<(Millis, u64), Ev>,
    seq: u64,
    /// Heal time `t_g`.
    pub heal_at: Millis,
    /// End of the run.
    pub duration: Millis,
    /// Network.
    pub net: NetConfig,
    /// Instances.
    pub instances: Vec<Inst>,
    /// Machines.
    pub machines: Vec<Machine>,
    /// Replicas.
    pub replicas: Vec<Replica>,
    /// Provenance log.
    pub log: SharedLog,
    /// Hashing for the world (not counted as CPU).
    pub hasher: SimCrypto,
    /// Oracles and bounds.
    pub checks: Checks,
    /// Oracle state.
    pub oracle: Oracle,
    /// Byzantine strategies and the network adversary.
    pub adv: Adversary,
    trace: Vec<TraceEntry>,
    trace_next: usize,
    ready: Vec<Millis>,
    script: Vec<Option<Fault>>,
    churn: Option<Churn>,
    workload: Option<Workload>,
    /// Submitted transactions per instance.
    pub txs: Vec<TxLog>,
    next_tx: u64,
    /// Counters.
    pub stats: Stats,
    /// The first oracle violation.
    pub failure: Option<String>,
    /// Key → machine.
    pub key_owner: BTreeMap<PublicKey, usize>,
    /// Instance id → instance index.
    pub inst_by_id: BTreeMap<Hash32, usize>,
    /// Print every handled event (`SUMERAGI_SIM_TRACE`).
    pub verbose: bool,
    /// Source of `Init.nonce` values (seeded from the PRNG).
    nonce_source: std::cell::Cell<u64>,
}

/// A fresh random 128-bit store id.
fn fresh_id(rng: &mut Rng) -> StoreId {
    (StoreId::from(rng.next_u64()) << 64) | StoreId::from(rng.next_u64())
}

/// Derive a 32-byte fake key.
fn derive_key(seed: u64, machine: usize, slot: usize) -> PublicKey {
    let mut input = b"sumeragi-sim-key".to_vec();
    input.extend_from_slice(&seed.to_be_bytes());
    input.extend_from_slice(&u64::try_from(machine).unwrap_or(0).to_be_bytes());
    input.extend_from_slice(&u64::try_from(slot).unwrap_or(0).to_be_bytes());
    PublicKey::new(sha256(&input).to_vec()).unwrap_or_else(|_| unreachable!("32-byte key"))
}

fn derive_hash(tag: &[u8], seed: u64, index: usize) -> Hash32 {
    let mut input = tag.to_vec();
    input.extend_from_slice(&seed.to_be_bytes());
    input.extend_from_slice(&u64::try_from(index).unwrap_or(0).to_be_bytes());
    Hash32(sha256(&input))
}

/// The topology of `height` of instance 0 that a scenario's world will use, assuming no
/// demotions (static corruption with hindsight, §13.1), and the machine of each canonical index.
pub fn preview(sc: &Scenario, height: u64) -> (crate::topology::Topology, Vec<usize>) {
    let mut rng = Rng::new(seed_of(&sc.name, sc.seed));
    let key_seed = rng.next_u64();
    let members = sc
        .committees
        .iter()
        .rev()
        .find(|(from, _)| *from <= height)
        .or_else(|| sc.committees.first())
        .map(|(_, m)| m.clone())
        .unwrap_or_default();
    let keyed: Vec<(PublicKey, usize)> = members
        .iter()
        .map(|(m, slot)| (derive_key(key_seed, m * 16, *slot), *m))
        .collect();
    let committee = Committee::new(keyed.iter().map(|(k, _)| k.clone()).collect())
        .expect("scenario committees are well formed");
    let machine_of = committee
        .members()
        .iter()
        .map(|k| keyed.iter().find(|(x, _)| x == k).map_or(0, |(_, m)| *m))
        .collect();
    let id = derive_hash(b"sim-instance", key_seed, 0);
    let topo = crate::topology::Topology::compute(
        &SimCrypto::new(),
        &id,
        &committee,
        height,
        0,
        sc.demotion_window,
        &[],
    );
    (topo, machine_of)
}

impl World {
    /// Build the world of a scenario and start every machine.
    ///
    /// # Panics
    /// If the scenario is malformed (empty committee schedule, invalid configuration).
    #[allow(clippy::too_many_lines)] // one straight-line setup of every component
    pub fn new(sc: Scenario) -> Self {
        let mut rng = Rng::new(seed_of(&sc.name, sc.seed));
        let machines_n = sc.machines();
        let key_seed = rng.next_u64();
        let slots = sc
            .committees
            .iter()
            .flat_map(|(_, members)| members.iter().map(|(_, slot)| *slot + 1))
            .max()
            .unwrap_or(1);
        let mut instances = Vec::new();
        for i in 0..sc.instances.max(1) {
            let key_of = |m: usize, slot: usize| {
                let base = if sc.shared_keys { 0 } else { i };
                derive_key(key_seed, m * 16 + base, slot)
            };
            let schedule = sc
                .committees
                .iter()
                .map(|(from, members)| {
                    let keys = members.iter().map(|(m, s)| key_of(*m, *s)).collect();
                    (
                        *from,
                        Committee::new(keys).expect("scenario committees are well formed"),
                    )
                })
                .collect();
            instances.push(Inst {
                id: derive_hash(b"sim-instance", key_seed, i),
                genesis_hash: derive_hash(b"sim-genesis", key_seed, i),
                genesis_result: derive_hash(b"sim-genesis-result", key_seed, i),
                schedule,
                params: sc.params,
                window: sc.demotion_window,
                local: sc.local,
            });
        }
        let mut machines = Vec::new();
        let mut replicas = Vec::new();
        let mut key_owner = BTreeMap::new();
        let log: SharedLog = Rc::default();
        for m in 0..machines_n {
            let mut machine = Machine {
                up: false,
                epoch: 0,
                clock: sc.clocks.get(m).copied().unwrap_or_default(),
                byz: sc.is_byz(m),
                replicas: vec![None; instances.len()],
                profile: sc.profile(m),
                started_at: 0,
                crashes: 0,
                keys: Vec::new(),
                keystore: KeyStore::default(),
                store_id: None,
                snapshot: None,
                retired: BTreeSet::new(),
            };
            for (i, inst) in instances.iter().enumerate() {
                let base = if sc.shared_keys { 0 } else { i };
                let keys: Vec<PublicKey> = (0..slots)
                    .map(|slot| derive_key(key_seed, m * 16 + base, slot))
                    .filter(|key| inst.schedule.iter().any(|(_, c)| c.contains(key)))
                    .collect();
                let observer = m >= sc.n && keys.is_empty();
                let keys = if observer {
                    vec![derive_key(key_seed, m * 16 + base, 0)]
                } else {
                    keys
                };
                if keys.is_empty() {
                    continue;
                }
                for key in &keys {
                    key_owner.insert(key.clone(), m);
                    if !machine.keys.contains(key) {
                        machine.keys.push(key.clone());
                    }
                }
                let crypto = SimCrypto::new();
                let rep = Replica {
                    machine: m,
                    inst: i,
                    keys: keys.clone(),
                    core: None,
                    crypto,
                    records: BTreeMap::new(),
                    bodies: BTreeMap::new(),
                    store: Vec::new(),
                    lanes: Lanes::default(),
                    busy_until: 0,
                    cpu_carry_us: 0,
                    io: Io::default(),
                    exec: Executor::default(),
                    nic: Nic::default(),
                    txs: BTreeMap::new(),
                    quarantine: BTreeSet::new(),
                    applied: (0, inst.genesis_hash, inst.genesis_result),
                    height: 1,
                    halted: None,
                    last_persisted: BTreeMap::new(),
                    max_tick_late: 0,
                    wake_mark: (0, 0),
                    pending_ready: None,
                };
                machine.replicas[i] = Some(replicas.len());
                replicas.push(rep);
            }
            // The keys are generated on the node (§7.4 record provenance); the instances start
            // at the first restart (installation events with initial records).
            for key in machine.keys.clone() {
                let id = fresh_id(&mut rng);
                machine
                    .keystore
                    .install_key(&mut machine.store_id, &key, true, id);
            }
            if sc.keystore_snapshot {
                machine.snapshot = Some(machine.keystore.clone());
            }
            machines.push(machine);
        }
        let inst_by_id = instances
            .iter()
            .enumerate()
            .map(|(i, inst)| (inst.id, i))
            .collect();
        let replica_count = replicas.len();
        let adv = Adversary::new(&sc, &machines);
        let mut world = Self {
            name: sc.name.clone(),
            seed: sc.seed,
            now: 0,
            rng,
            queue: BTreeMap::new(),
            seq: 0,
            heal_at: sc.heal_at,
            duration: sc.duration,
            net: sc.net,
            instances,
            machines,
            replicas,
            log,
            hasher: SimCrypto::new(),
            checks: sc.checks,
            oracle: Oracle::new(replica_count),
            adv,
            trace: Vec::new(),
            trace_next: 0,
            ready: vec![Millis::MAX; replica_count],
            script: Vec::new(),
            churn: sc.churn,
            workload: sc.workload,
            txs: Vec::new(),
            next_tx: 0,
            stats: Stats::default(),
            failure: None,
            key_owner,
            inst_by_id,
            verbose: std::env::var("SUMERAGI_SIM_TRACE").is_ok(),
            nonce_source: std::cell::Cell::new(0),
        };
        world.nonce_source.set(world.rng.next_u64());
        world.txs = vec![BTreeMap::new(); world.instances.len()];
        world.oracle.init(&world.instances);
        if sc.prebuilt > 0 {
            world.prebuild(sc.prebuilt, &sc.prebuilt_holders);
        }
        for (at, fault) in sc.script {
            let index = world.script.len();
            world.script.push(Some(fault));
            world.schedule(at, Ev::Script(index));
        }
        if world.workload.is_some() {
            for i in 0..world.instances.len() {
                world.schedule(1, Ev::TxGen(i));
            }
        }
        for r in 0..world.replicas.len() {
            if world.adv.has_strategy(r) {
                world.schedule(50, Ev::ByzTick(r));
            }
            if world.machines[world.replicas[r].machine].profile.evict_ppm > 0 {
                world.schedule(100, Ev::Evict(r));
            }
        }
        for m in 0..world.machines.len() {
            world.restart(m);
        }
        world
    }

    // ---- scheduling ----------------------------------------------------------------------

    fn schedule(&mut self, at: Millis, ev: Ev) {
        self.seq += 1;
        self.queue.insert((at.max(self.now), self.seq), ev);
    }

    /// Record a trace line.
    pub fn trace(&mut self, who: usize, what: String) {
        if self.verbose && !what.starts_with("<-") && !what.starts_with("Tick") {
            eprintln!("t={:>7} #{who:<3} ** {what}", self.now);
        }
        let entry = TraceEntry {
            t: self.now,
            who,
            what,
        };
        if self.trace.len() < TRACE_LEN {
            self.trace.push(entry);
        } else {
            self.trace[self.trace_next] = entry;
        }
        self.trace_next = (self.trace_next + 1) % TRACE_LEN;
    }

    /// Record the first violation (the run stops).
    pub fn fail(&mut self, what: String) {
        if self.failure.is_none() {
            let mut what = what;
            what.insert_str(0, &format!("t={} ", self.now));
            self.failure = Some(what);
        }
    }

    /// The failure report: scenario, seed, violation, reproduction hint and the last events.
    pub fn report(&self, violation: &str) -> String {
        let mut out = format!(
            "sumeragi simulation failure\n  scenario {} seed {} (reproduce: SUMERAGI_SIM_SEED={} \
             cargo test -p iroha_sumeragi --release sim::)\n  violation: {violation}\n  last events:\n",
            self.name, self.seed, self.seed
        );
        let len = self.trace.len();
        for i in 0..len {
            let entry = &self.trace[(self.trace_next + i) % len];
            let _ = writeln!(out, "    t={:>7} #{:<3} {}", entry.t, entry.who, entry.what);
        }
        out
    }

    /// Run until the duration or the first violation.
    ///
    /// # Errors
    /// The failure report of the first violation.
    pub fn run(&mut self) -> Result<(), String> {
        while self.failure.is_none() && self.step() {}
        if self.failure.is_none() {
            self.now = self.now.max(self.duration);
            self.finish();
        }
        self.failure
            .as_ref()
            .map_or(Ok(()), |violation| Err(self.report(violation)))
    }

    /// Run until `until` (for scripted tests); stops early on a violation.
    pub fn run_until(&mut self, until: Millis) {
        let end = self.duration;
        self.duration = until.min(end);
        while self.failure.is_none() && self.step() {}
        self.now = self.now.max(self.duration);
        self.duration = end;
    }

    fn step(&mut self) -> bool {
        let next_q = self.queue.first_key_value().map(|((t, _), _)| *t);
        let next_r = self
            .ready
            .iter()
            .enumerate()
            .min_by_key(|(_, t)| **t)
            .map(|(r, t)| (*t, r));
        let t_q = next_q.unwrap_or(Millis::MAX);
        let (t_r, r) = next_r.unwrap_or((Millis::MAX, 0));
        let t = t_q.min(t_r);
        if t == Millis::MAX || t > self.duration {
            return false;
        }
        self.now = self.now.max(t);
        self.check_live();
        if self.failure.is_some() {
            return false;
        }
        if t_q <= t_r {
            if let Some((_, ev)) = self.queue.pop_first() {
                self.dispatch(ev);
            }
        } else {
            self.process(r);
        }
        true
    }

    fn dispatch(&mut self, ev: Ev) {
        match ev {
            Ev::Arrive { r, from, msg } => self.arrive(r, from, &msg),
            Ev::Local { r, epoch, event } => {
                if self.alive(r, epoch) {
                    self.replicas[r].lanes.push_local(*event);
                    self.refresh(r);
                }
            }
            Ev::IoDone { r, epoch, id } => {
                if self.alive(r, epoch) {
                    self.io_done(r, id);
                }
            }
            Ev::ExecDone { r, epoch, job } => {
                if self.alive(r, epoch) {
                    self.exec_done(r, job);
                }
            }
            Ev::NicFree { r, epoch } => {
                if self.alive(r, epoch) {
                    self.replicas[r].nic.wakeup_pending = false;
                    self.nic_flush(r, self.now);
                }
            }
            Ev::Script(index) => {
                if let Some(fault) = self.script.get_mut(index).and_then(Option::take) {
                    self.trace(0, format!("script {fault:?}"));
                    self.apply_fault(fault);
                }
            }
            Ev::Restart(m) => {
                if !self.machines[m].up {
                    self.restart(m);
                }
            }
            Ev::TxGen(inst) => self.gen_tx(inst),
            Ev::ByzTick(r) => {
                self.byz_tick(r);
                self.schedule(self.now + 100, Ev::ByzTick(r));
            }
            Ev::Evict(r) => {
                self.evict(r);
                self.schedule(self.now + 100, Ev::Evict(r));
            }
        }
    }

    fn alive(&self, r: usize, epoch: u64) -> bool {
        let m = &self.machines[self.replicas[r].machine];
        m.up && m.epoch == epoch
    }

    /// Current incarnation of replica `r`'s machine.
    pub fn epoch_of(&self, r: usize) -> u64 {
        self.machines[self.replicas[r].machine].epoch
    }

    /// Recompute the time at which replica `r` next has work.
    pub fn refresh(&mut self, r: usize) {
        let rep = &self.replicas[r];
        let machine = &self.machines[rep.machine];
        let t = match rep.core.as_ref() {
            Some(core) if machine.up => {
                let wake = machine.clock.global_at(core.next_wakeup());
                let t = if rep.lanes.is_empty() {
                    wake
                } else {
                    wake.min(self.now)
                };
                if t == Millis::MAX {
                    t
                } else {
                    t.max(rep.busy_until)
                }
            }
            _ => Millis::MAX,
        };
        self.ready[r] = t;
    }

    // ---- handling --------------------------------------------------------------------------

    fn process(&mut self, r: usize) {
        let m = self.replicas[r].machine;
        let local_now = self.machines[m].clock.local(self.now);
        let byz = self.machines[m].byz;
        let rep = &mut self.replicas[r];
        let Some(core) = rep.core.as_mut() else {
            self.ready[r] = Millis::MAX;
            return;
        };
        let wake = core.next_wakeup();
        let tick_first = !rep.lanes.fifo || rep.lanes.is_empty();
        let event = if wake <= local_now && tick_first {
            // The deadline became due at `wake`, or when the core first reported it.
            let due = wake.max(rep.wake_mark.1);
            rep.max_tick_late = rep.max_tick_late.max(local_now.saturating_sub(due));
            Event::Tick
        } else if let Some(event) = rep.lanes.pop() {
            event
        } else {
            self.refresh(r);
            return;
        };
        let mut what = describe_event(&event);
        if let Event::Message { from, .. } = &event
            && let Some(sender) = self.key_owner.get(from)
        {
            let _ = write!(what, " [m{sender}]");
        }
        let is_tick = matches!(event, Event::Tick);
        let before = rep.crypto.pairings();
        let actions = core.handle(local_now, event);
        let after_wake = core.next_wakeup();
        let pairings = rep.crypto.pairings() - before;
        let height = core.status().height;
        rep.height = height;
        let profile = self.machines[m].profile;
        rep.cpu_carry_us += pairings * profile.cpu_us_per_pairing + profile.cpu_us_per_event;
        let cost = rep.cpu_carry_us / 1_000;
        rep.cpu_carry_us %= 1_000;
        let at = self.now + cost;
        rep.busy_until = at;
        if after_wake != rep.wake_mark.0 {
            rep.wake_mark = (after_wake, self.machines[m].clock.local(at));
        }
        self.stats.events += 1;
        if self.verbose {
            let summary = summarize(&actions);
            eprintln!("t={:>7} #{r:<3} m{m:<3} {what} => {summary}", self.now);
        }
        self.trace(r, what);
        if is_tick && after_wake <= local_now && !byz {
            self.fail(format!(
                "replica {r}: Tick did not consume its due deadline ({after_wake} ≤ {local_now})"
            ));
        }
        let actions = if byz {
            self.byz_filter(r, actions, at)
        } else {
            self.after_handle(r, &actions);
            actions
        };
        self.apply_actions(r, actions, at);
        self.refresh(r);
    }

    /// Execute a core's actions in order (O1); a churn crash may cut the list short.
    pub fn apply_actions(&mut self, r: usize, actions: Vec<Action>, at: Millis) {
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        for action in actions {
            if !self.alive(r, epoch) {
                return;
            }
            let point = self.crash_point_of(r, &action);
            self.apply_action(r, action, at);
            if self.churn_hit(m, point) {
                self.crash_by_churn(m);
                return;
            }
        }
    }

    fn crash_point_of(&mut self, r: usize, action: &Action) -> CrashPoint {
        let Action::PersistSafety(record) = action else {
            return CrashPoint::Random;
        };
        let rep = &mut self.replicas[r];
        let point = match rep.last_persisted.get(&record.key) {
            Some(old) if old.height == record.height => {
                if old.proposal != record.proposal {
                    CrashPoint::ProposalRecord
                } else if old.timeout != record.timeout {
                    CrashPoint::TimeoutRecord
                } else if old.prepare != record.prepare || old.lock != record.lock {
                    // A Commit is recorded through the lock (§6.5).
                    CrashPoint::VoteRecord
                } else {
                    CrashPoint::Random
                }
            }
            _ => {
                if record.proposal.is_some() {
                    CrashPoint::ProposalRecord
                } else if record.prepare.is_some() || record.lock.is_some() {
                    CrashPoint::VoteRecord
                } else if record.timeout.is_some() {
                    CrashPoint::TimeoutRecord
                } else {
                    CrashPoint::Random
                }
            }
        };
        rep.last_persisted
            .insert(record.key.clone(), (**record).clone());
        point
    }

    fn write_latency(&mut self, m: usize) -> Millis {
        let p = self.machines[m].profile;
        let mut latency = self.rng.range(p.write_min, p.write_max);
        while self.rng.chance(p.write_fail_ppm) {
            latency = latency.saturating_add(p.write_retry);
        }
        latency
    }

    fn apply_action(&mut self, r: usize, action: Action, at: Millis) {
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        match action {
            Action::PersistSafety(record) => {
                let bytes = match record.encode(&self.hasher) {
                    Ok(bytes) => bytes,
                    Err(e) => return self.fail(format!("replica {r}: record encode: {e}")),
                };
                let latency = self.write_latency(m);
                let (id, done) =
                    self.replicas[r]
                        .io
                        .write(at, latency, Write::Record(record, bytes));
                self.schedule(done, Ev::IoDone { r, epoch, id });
            }
            Action::StoreBody { block } => {
                let latency = self.write_latency(m);
                let (id, done) =
                    self.replicas[r]
                        .io
                        .write(at, latency, Write::Body(Box::new(block)));
                self.schedule(done, Ev::IoDone { r, epoch, id });
            }
            Action::Execute { block, req } => {
                let bh = block.hash(&self.hasher);
                self.replicas[r].exec.submit(bh, req, block);
                self.exec_kick(r, at);
            }
            Action::DiscardExecution { height, keep } => {
                let abort = self.machines[m].profile.abort_discarded;
                let cancelled = self.replicas[r].exec.discard(height, &keep, abort);
                if abort {
                    self.exec_kick(r, at);
                }
                for (bh, req) in cancelled {
                    self.schedule(
                        at,
                        Ev::Local {
                            r,
                            epoch,
                            event: Box::new(Event::Executed {
                                block_hash: bh,
                                req,
                                outcome: ExecOutcome::Cancelled,
                            }),
                        },
                    );
                }
            }
            Action::BuildPayload {
                req,
                max_bytes,
                exec_budget_ms,
                ..
            } => self.build_payload(r, req, max_bytes, exec_budget_ms, at),
            #[cfg(not(sumeragi_mutation = "ML14"))]
            Action::PayloadRejected { block_hash, .. } => self.quarantine(r, &block_hash),
            #[cfg(sumeragi_mutation = "ML14")]
            Action::PayloadRejected { .. } => {}
            Action::LocalFault(_) => {}
            Action::Halt(reason) => {
                self.replicas[r].halted = Some(reason);
                self.trace(r, format!("HALT {reason:?}"));
            }
            effect => {
                if let Some(effect) = self.replicas[r].io.hold(effect) {
                    self.perform(r, effect, at);
                }
            }
        }
    }

    /// Carry out an externally visible effect (after the O2 barrier).
    fn perform(&mut self, r: usize, action: Action, at: Millis) {
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        let inst = self.replicas[r].inst;
        let instance = self.instances[inst].id;
        match action {
            Action::Send { to, msg } => {
                self.expose(r, &msg);
                if matches!(msg, WireMessage::Proposal(_)) && !self.machines[m].byz {
                    self.stats.proposals += 1;
                }
                self.net_send(r, &to, Rc::new(msg), at);
            }
            Action::Broadcast { to, msg } => {
                self.expose(r, &msg);
                if let WireMessage::Proposal(p) = &msg
                    && !self.machines[m].byz
                {
                    self.stats.proposals += 1;
                    let bh = p.block_hash(&self.hasher);
                    self.oracle.proposed.entry(bh).or_insert(at);
                }
                let msg = Rc::new(msg);
                for key in &to {
                    self.net_send(r, key, Rc::clone(&msg), at);
                }
            }
            Action::CommitBlock { block, commit_qc } => {
                self.expose_qc(r, &commit_qc);
                let latency = self.write_latency(m) + self.machines[m].profile.block_write_extra;
                let (id, done) = self.replicas[r].io.write(
                    at,
                    latency,
                    Write::Commit(Box::new((block, commit_qc))),
                );
                self.schedule(done, Ev::IoDone { r, epoch, id });
            }
            Action::FetchBody {
                height,
                block_hash,
                peers,
            } => {
                if let Some(block) = self.local_body(r, height, &block_hash) {
                    self.schedule(
                        at + 1,
                        Ev::Local {
                            r,
                            epoch,
                            event: Box::new(Event::BodyAvailable { block }),
                        },
                    );
                } else {
                    let msg = Rc::new(WireMessage::BlockRequest(BlockRequest {
                        instance,
                        height,
                        block_hash,
                    }));
                    for peer in &peers {
                        self.net_send(r, peer, Rc::clone(&msg), at);
                    }
                }
            }
            Action::ServeBody {
                to,
                height,
                block_hash,
            } => {
                if let Some(block) = self.local_body(r, height, &block_hash) {
                    let msg = WireMessage::BlockResponse(BlockResponse { instance, block });
                    self.net_send(r, &to, Rc::new(msg), at);
                }
            }
            Action::ServeBlocks {
                to,
                from_height,
                max_count,
                max_bytes,
            } => {
                // An empty response means "I hold nothing at from_height" (§3.5).
                let blocks = self.serve_blocks(r, from_height, max_count, max_bytes);
                let msg = WireMessage::SyncResponse(SyncResponse { instance, blocks });
                self.expose(r, &msg);
                self.net_send(r, &to, Rc::new(msg), at);
            }
            Action::ReportEvidence(evidence) => {
                self.expose_evidence(r, &evidence);
            }
            _ => {}
        }
    }

    fn local_body(&self, r: usize, height: u64, bh: &Hash32) -> Option<Block> {
        let rep = &self.replicas[r];
        rep.bodies
            .get(bh)
            .cloned()
            .or_else(|| rep.io.pending_body(bh, &self.hasher))
            .or_else(|| {
                let g = 0u64;
                let index = usize::try_from(height.checked_sub(g + 1)?).ok()?;
                rep.store
                    .get(index)
                    .filter(|(_, qc)| qc.block_hash == *bh)
                    .map(|(b, _)| b.clone())
            })
            .filter(|b| b.header.height == height)
    }

    fn serve_blocks(
        &self,
        r: usize,
        from_height: u64,
        max_count: u16,
        max_bytes: u32,
    ) -> Vec<SyncEntry> {
        let rep = &self.replicas[r];
        let Some(start) = from_height
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok())
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut bytes = 0u64;
        for (block, qc) in rep.store.iter().skip(start).take(usize::from(max_count)) {
            let size = u64::try_from(block.payload.len()).unwrap_or(u64::MAX) + 512;
            if !out.is_empty() && bytes + size > u64::from(max_bytes) {
                break;
            }
            bytes += size;
            out.push(SyncEntry {
                block: block.clone(),
                commit_qc: qc.clone(),
            });
        }
        out
    }

    // ---- network ---------------------------------------------------------------------------

    /// The key a replica uses as its P2P identity (its key in the current committee).
    pub fn net_key(&self, r: usize) -> PublicKey {
        let rep = &self.replicas[r];
        let committee = self.instances[rep.inst].committee(rep.height);
        rep.keys
            .iter()
            .rev()
            .find(|k| committee.contains(k))
            .or_else(|| rep.keys.first())
            .cloned()
            .unwrap_or_else(|| unreachable!("a replica has a key"))
    }

    /// Queue a message from replica `r` to the owner of `to` on `r`'s NIC.
    pub fn net_send(&mut self, r: usize, to: &PublicKey, msg: Rc<WireMessage>, at: Millis) {
        let Some(&tm) = self.key_owner.get(to) else {
            return;
        };
        let inst = self
            .inst_by_id
            .get(msg.instance())
            .copied()
            .unwrap_or(self.replicas[r].inst);
        let Some(target) = self.machines[tm].replicas.get(inst).copied().flatten() else {
            return;
        };
        self.send_to_replica(r, target, msg, at);
    }

    /// Queue a message from replica `r` to replica `target` (bypassing key routing).
    pub fn send_to_replica(&mut self, r: usize, target: usize, msg: Rc<WireMessage>, at: Millis) {
        if target == r {
            return; // O7
        }
        let size = approx_size(&msg);
        if size > self.net.frame_limit {
            self.stats.oversize += 1;
            return;
        }
        let class = class_of(&msg, self.replicas[r].height);
        self.replicas[r].nic.push(
            class,
            Packet {
                item: (target, msg),
                size,
            },
        );
        self.nic_flush(r, at);
    }

    fn nic_flush(&mut self, r: usize, at: Millis) {
        let bandwidth = self.net.bandwidth;
        let (out, next) = self.replicas[r].nic.drain(at, bandwidth);
        for (depart, (target, msg)) in out {
            self.transmit(r, target, msg, depart);
        }
        if let Some(next) = next
            && !self.replicas[r].nic.wakeup_pending
        {
            self.replicas[r].nic.wakeup_pending = true;
            let epoch = self.epoch_of(r);
            self.schedule(next, Ev::NicFree { r, epoch });
        }
    }

    fn transmit(&mut self, r: usize, target: usize, msg: Rc<WireMessage>, depart: Millis) {
        let size = approx_size(&msg);
        let lane = class_of(&msg, self.replicas[r].height).lane();
        self.stats.packets[lane] += 1;
        self.stats.bytes += size;
        let Some((msg, extra)) = self.adv_net(r, target, msg, depart) else {
            self.stats.lost += 1;
            return;
        };
        let a = self.replicas[r].machine;
        let b = self.replicas[target].machine;
        let from = self.net_key(r);
        match self.net.fate(&mut self.rng, depart, self.heal_at, a, b) {
            Fate::Drop => self.stats.lost += 1,
            Fate::Deliver(first, second) => {
                self.schedule(
                    depart + first + extra,
                    Ev::Arrive {
                        r: target,
                        from: from.clone(),
                        msg: Rc::clone(&msg),
                    },
                );
                if let Some(second) = second {
                    self.schedule(
                        depart + second + extra,
                        Ev::Arrive {
                            r: target,
                            from,
                            msg,
                        },
                    );
                }
            }
        }
    }

    /// Deliver a message into the ingress lanes of replica `r` directly (adversary injection).
    pub fn inject(&mut self, r: usize, from: PublicKey, msg: WireMessage, delay: Millis) {
        self.schedule(
            self.now + delay,
            Ev::Arrive {
                r,
                from,
                msg: Rc::new(msg),
            },
        );
    }

    fn arrive(&mut self, r: usize, from: PublicKey, msg: &Rc<WireMessage>) {
        let m = self.replicas[r].machine;
        if !self.machines[m].up || self.replicas[r].core.is_none() {
            return;
        }
        if self.machines[m].byz {
            self.byz_observe(r, &from, msg);
        }
        let class = class_of(msg, self.replicas[r].height);
        self.replicas[r]
            .lanes
            .push_message(from, (**msg).clone(), class);
        self.refresh(r);
    }

    // ---- storage, execution, building -------------------------------------------------------

    fn io_done(&mut self, r: usize, id: u64) {
        let (writes, released) = self.replicas[r].io.complete(id);
        for write in writes {
            match write {
                Write::Record(record, bytes) => {
                    let rep = &mut self.replicas[r];
                    // MS33a: one record file per instance for all keys.
                    #[cfg(sumeragi_mutation = "MS33a")]
                    for durable in rep.records.values_mut() {
                        durable.record = (*record).clone();
                        durable.bytes.clone_from(&bytes);
                    }
                    rep.records.insert(
                        record.key.clone(),
                        Durable {
                            record: *record,
                            bytes,
                        },
                    );
                }
                Write::Body(block) => {
                    let rep = &mut self.replicas[r];
                    if block.header.height > rep.applied.0 {
                        let bh = block.hash(&self.hasher);
                        rep.bodies.insert(bh, *block);
                    }
                }
                Write::Commit(entry) => {
                    let (block, qc) = *entry;
                    self.apply_block(r, &block, &qc);
                    if !self.machines[self.replicas[r].machine].up {
                        return;
                    }
                }
            }
        }
        let m = self.replicas[r].machine;
        if !released.is_empty() && self.churn_hit(m, CrashPoint::AfterDurable) {
            self.crash_by_churn(m);
            return;
        }
        let now = self.now;
        let epoch = self.machines[m].epoch;
        for effect in released {
            if !self.alive(r, epoch) {
                return;
            }
            self.perform(r, effect, now);
        }
        if self.churn_hit(m, CrashPoint::Random) {
            self.crash_by_churn(m);
        }
    }

    /// O3: apply a committed block. The cached post-state of exactly this block is reused if
    /// its commitment equals the certified result; otherwise the block is executed (a
    /// divergent executor then reports `ApplyDiverged`). `BlockApplied` carries the header.
    fn apply_block(&mut self, r: usize, block: &Block, qc: &Qc) {
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        let inst = self.replicas[r].inst;
        let height = block.header.height;
        let bh = qc.block_hash;
        let (tip_height, tip_hash, tip_result) = self.replicas[r].applied;
        if height != tip_height + 1 || block.header.parent_hash != tip_hash {
            let byz = self.machines[m].byz;
            if !byz {
                self.fail(format!(
                    "O3: replica {r} CommitBlock {height} does not extend the applied state {tip_height}"
                ));
            }
            return;
        }
        self.replicas[r].store.push((block.clone(), qc.clone()));
        if self.churn_hit(m, CrashPoint::InsideApply) {
            self.crash_by_churn(m);
            return;
        }
        let profile = self.machines[m].profile;
        let cached = self.replicas[r]
            .exec
            .cache
            .get(&bh)
            .map(|(_, res)| *res)
            .filter(|res| *res == qc.result);
        let local = cached.or_else(|| {
            let outcome = if profile.divergent && !block.payload.is_empty() {
                divergent_exec(&tip_result, &block.payload, &bh)
            } else {
                reference_exec(&tip_result, &block.payload)
            };
            match outcome {
                ExecOutcome::Valid(res) => Some(res),
                _ => None,
            }
        });
        let at = self.now + profile.apply_ms;
        if local != Some(qc.result) && !cfg!(sumeragi_mutation = "MS34") {
            self.schedule(
                at,
                Ev::Local {
                    r,
                    epoch,
                    event: Box::new(Event::ApplyDiverged {
                        height,
                        block_hash: bh,
                        local_result: local.unwrap_or(Hash32::ZERO),
                    }),
                },
            );
            return;
        }
        let rep = &mut self.replicas[r];
        rep.applied = (height, bh, qc.result);
        rep.bodies.retain(|_, b| b.header.height > height);
        rep.exec.cache.retain(|_, (h, _)| *h >= height);
        for (id, _) in decode_txs(&block.payload) {
            rep.txs.remove(&id);
        }
        let config = self.instances[inst].config(height + 2);
        self.schedule(
            at,
            Ev::Local {
                r,
                epoch,
                event: Box::new(Event::BlockApplied {
                    height,
                    block_hash: bh,
                    header: Box::new(block.header.clone()),
                    config_after_next: config,
                }),
            },
        );
        self.exec_unpark(r);
    }

    fn parent_result(&self, r: usize, block: &Block) -> Option<Hash32> {
        let rep = &self.replicas[r];
        if block.header.parent_hash == rep.applied.1 {
            return Some(rep.applied.2);
        }
        rep.exec
            .cache
            .get(&block.header.parent_hash)
            .map(|(_, res)| *res)
    }

    fn exec_kick(&mut self, r: usize, at: Millis) {
        loop {
            if self.replicas[r].exec.running.is_some() {
                return;
            }
            let Some(mut job) = self.replicas[r].exec.queue.pop() else {
                return;
            };
            let Some(parent) = self.parent_result(r, &job.block) else {
                self.replicas[r].exec.parked.push(job);
                continue;
            };
            let m = self.replicas[r].machine;
            let profile = self.machines[m].profile;
            let outcome = if self.rng.chance(profile.exec_fail_ppm) {
                ExecOutcome::Failed("injected".to_owned())
            } else if profile.reject_nonempty && !job.block.payload.is_empty() {
                ExecOutcome::Invalid
            } else if profile.divergent && !job.block.payload.is_empty() {
                divergent_exec(&parent, &job.block.payload, &job.bh)
            } else {
                reference_exec(&parent, &job.block.payload)
            };
            let kib = u64::try_from(job.block.payload.len()).unwrap_or(u64::MAX) / 1024;
            let nonempty = if job.block.payload.is_empty() {
                0
            } else {
                profile.exec_nonempty
            };
            let latency = profile.exec_base + profile.exec_per_kib.saturating_mul(kib) + nonempty;
            job.outcome = Some(outcome);
            let id = job.id;
            let finish = at + latency;
            self.replicas[r].exec.running = Some((job, finish));
            let epoch = self.epoch_of(r);
            self.schedule(finish, Ev::ExecDone { r, epoch, job: id });
            return;
        }
    }

    fn exec_unpark(&mut self, r: usize) {
        let exec = &mut self.replicas[r].exec;
        if exec.parked.is_empty() {
            return;
        }
        let parked = std::mem::take(&mut exec.parked);
        // Keep LIFO service: parked jobs are older than queued ones.
        let queued = std::mem::take(&mut exec.queue);
        exec.queue = parked;
        exec.queue.extend(queued);
        self.exec_kick(r, self.now);
    }

    fn exec_done(&mut self, r: usize, job_id: u64) {
        let epoch = self.epoch_of(r);
        let rep = &mut self.replicas[r];
        let Some((job, _)) = rep.exec.running.take_if(|(job, _)| job.id == job_id) else {
            return;
        };
        let outcome = if job.cancelled {
            ExecOutcome::Cancelled
        } else {
            job.outcome.clone().unwrap_or(ExecOutcome::Cancelled)
        };
        if let ExecOutcome::Valid(res) = &outcome {
            rep.exec
                .cache
                .insert(job.bh, (job.block.header.height, *res));
        }
        rep.lanes.push_local(Event::Executed {
            block_hash: job.bh,
            req: job.req,
            outcome,
        });
        let _ = epoch;
        self.refresh(r);
        self.exec_unpark(r);
        self.exec_kick(r, self.now);
    }

    fn evict(&mut self, r: usize) {
        let m = self.replicas[r].machine;
        let ppm = self.machines[m].profile.evict_ppm;
        if !self.machines[m].up || !self.rng.chance(ppm) {
            return;
        }
        let keys: Vec<Hash32> = self.replicas[r].exec.cache.keys().copied().collect();
        if let Some(bh) = self.rng.pick(&keys).copied() {
            self.replicas[r].exec.cache.remove(&bh);
        }
    }

    /// The payload builder (§12.2): it only peeks at the queue; an `EMPTY` answer to `req`
    /// makes it owe one `PayloadReady{req}` when a transaction arrives.
    fn build_payload(&mut self, r: usize, req: u64, max_bytes: u32, budget_ms: u32, at: Millis) {
        let m = self.replicas[r].machine;
        let epoch = self.machines[m].epoch;
        let profile = self.machines[m].profile;
        let budget_bytes = if profile.exec_per_kib == 0 {
            u64::MAX
        } else {
            u64::from(budget_ms).saturating_sub(profile.exec_base) * 1024 / profile.exec_per_kib
        };
        let limit = u64::from(max_bytes).min(budget_bytes);
        let rep = &self.replicas[r];
        let mut payload = Vec::new();
        for (id, tx) in &rep.txs {
            if rep.quarantine.contains(id) {
                continue;
            }
            let len = u64::try_from(payload.len() + tx.len()).unwrap_or(u64::MAX);
            if len > limit {
                break;
            }
            payload.extend_from_slice(tx);
        }
        let inst = rep.inst;
        let hash = crate::preimage::payload_hash(&self.hasher, &payload);
        self.oracle.built.insert((inst, m, hash));
        self.replicas[r].pending_ready = payload.is_empty().then_some(req);
        let latency = self.rng.range(profile.build_min, profile.build_max);
        self.schedule(
            at + latency,
            Ev::Local {
                r,
                epoch,
                event: Box::new(Event::PayloadBuilt { req, payload }),
            },
        );
    }

    fn quarantine(&mut self, r: usize, bh: &Hash32) {
        let rep = &self.replicas[r];
        let body = rep
            .bodies
            .get(bh)
            .cloned()
            .or_else(|| rep.io.pending_body(bh, &self.hasher));
        let Some(block) = body else {
            return;
        };
        let rep = &mut self.replicas[r];
        for (id, poison) in decode_txs(&block.payload) {
            if poison {
                rep.quarantine.insert(id);
                rep.txs.remove(&id);
            }
        }
    }

    fn gen_tx(&mut self, inst: usize) {
        let Some(workload) = self.workload else {
            return;
        };
        if self.now > workload.until {
            return;
        }
        self.next_tx += 1;
        let id = self.next_tx;
        let poison = self.rng.chance(workload.poison_ppm);
        let tx = encode_tx(id, poison, workload.pad);
        self.txs[inst].insert(id, (self.now, poison, None));
        for r in 0..self.replicas.len() {
            let m = self.replicas[r].machine;
            let targeted = workload.targets == 0
                || u32::try_from(m)
                    .ok()
                    .and_then(|bit| 1u64.checked_shl(bit))
                    .is_some_and(|bit| workload.targets & bit != 0);
            if self.replicas[r].inst == inst && self.machines[m].up && targeted {
                self.offer_tx(r, id, tx.clone());
            }
        }
        let next = self.now + self.rng.range(workload.every_min, workload.every_max);
        self.schedule(next, Ev::TxGen(inst));
    }

    fn offer_tx(&mut self, r: usize, id: u64, tx: Vec<u8>) {
        let rep = &mut self.replicas[r];
        if rep.quarantine.contains(&id) {
            return;
        }
        rep.txs.insert(id, tx);
        if let Some(req) = rep.pending_ready.take() {
            rep.lanes.push_local(Event::PayloadReady { req });
            self.refresh(r);
        }
    }

    // ---- crashes and restarts ---------------------------------------------------------------

    fn churn_hit(&mut self, m: usize, point: CrashPoint) -> bool {
        let Some(churn) = &self.churn else {
            return false;
        };
        if self.now < churn.from || self.now >= churn.until || self.machines[m].byz {
            return false;
        }
        if !churn.targets.is_empty() && !churn.targets.contains(&m) {
            return false;
        }
        let eligible = churn.points.contains(&point)
            || (churn.points.contains(&CrashPoint::Random) && point != CrashPoint::InsideApply);
        if !eligible {
            return false;
        }
        let down = self.machines.iter().filter(|x| !x.up).count();
        if down >= churn.max_down {
            return false;
        }
        let ppm = churn.ppm;
        self.rng.chance(ppm)
    }

    fn crash_by_churn(&mut self, m: usize) {
        let Some(churn) = self.churn.clone() else {
            return;
        };
        self.crash(m);
        let down = self.rng.range(churn.down_min, churn.down_max);
        self.schedule(self.now + down, Ev::Restart(m));
    }

    /// Crash machine `m`: every volatile state and non-durable write is lost.
    pub fn crash(&mut self, m: usize) {
        if !self.machines[m].up {
            return;
        }
        self.stats.crashes += 1;
        let machine = &mut self.machines[m];
        machine.up = false;
        machine.epoch += 1;
        machine.crashes += 1;
        let replicas: Vec<usize> = machine.replicas.iter().flatten().copied().collect();
        for r in replicas {
            let rep = &mut self.replicas[r];
            rep.core = None;
            rep.lanes.clear();
            rep.io.clear();
            rep.exec.clear();
            rep.nic.clear();
            rep.txs.clear();
            rep.busy_until = self.now;
            rep.halted = None;
            rep.last_persisted.clear();
            rep.pending_ready = None;
            self.ready[r] = Millis::MAX;
        }
        let durable: Vec<(PublicKey, SafetyRecord)> = self.machines[m]
            .replicas
            .iter()
            .flatten()
            .flat_map(|r| {
                self.replicas[*r]
                    .records
                    .iter()
                    .map(|(k, d)| (k.clone(), d.record.clone()))
            })
            .collect();
        self.log.borrow_mut().retract(m, |key, slot| {
            durable
                .iter()
                .any(|(k, record)| k == key && super::oracle::covers(record, slot))
        });
        self.trace(m, "CRASH".to_owned());
    }

    /// (Re)start machine `m` from its durable stores (§7.4 Restart): the start-up store-id check
    /// and the installation events (§7.4 record provenance), then one core per instance.
    pub fn restart(&mut self, m: usize) {
        self.machines[m].up = true;
        self.machines[m].started_at = self.now;
        let byz = self.machines[m].byz;
        let replicas: Vec<usize> = self.machines[m]
            .replicas
            .iter()
            .flatten()
            .copied()
            .collect();
        self.install_instances(m, &replicas);
        let local_now = self.machines[m].clock.local(self.now);
        for r in replicas {
            let init = self.init_for(r);
            let rep = &self.replicas[r];
            // Ground truth for the abstention oracle: a lost record (R2) must keep the key
            // from signing wherever it may have signed (Lemma 0), a store behind the record
            // (R6) below the record's height.
            let t = init.tip.height;
            let instance = self.instances[rep.inst].id;
            for key in &rep.keys {
                let below = match rep.records.get(key) {
                    None => self
                        .log
                        .borrow()
                        .max_signed_height(key, &instance)
                        .map(|h| h + 1),
                    Some(d) => Some(d.record.height).filter(|h| *h > t + 2),
                };
                if let Some(below) = below
                    && !byz
                {
                    self.log.borrow_mut().set_abstain(key, instance, below);
                }
            }
            let machine = (!byz).then_some(m);
            let retired = &self.machines[m].retired;
            let signers: Vec<Box<dyn Signer>> = rep
                .keys
                .iter()
                .filter(|key| !retired.contains(*key))
                .map(|key| -> Box<dyn Signer> {
                    Box::new(SimSigner::new(key.clone(), machine, Rc::clone(&self.log)))
                })
                .collect();
            let local = self.instances[rep.inst].local;
            let crypto = rep.crypto.clone();
            match Core::new(local, init, signers, Box::new(crypto), local_now) {
                Ok((core, actions)) => {
                    let fifo =
                        self.machines[m].profile.fifo_ingress || cfg!(sumeragi_mutation = "ML12");
                    let core_wake = core.next_wakeup();
                    let rep = &mut self.replicas[r];
                    rep.lanes.fifo = fifo;
                    rep.wake_mark = (core_wake, local_now);
                    rep.height = core.status().height;
                    rep.core = Some(core);
                    rep.busy_until = self.now;
                    rep.applied = rep.store.last().map_or_else(
                        || {
                            let instance = &self.instances[rep.inst];
                            (0, instance.genesis_hash, instance.genesis_result)
                        },
                        |(block, qc)| (block.header.height, qc.block_hash, qc.result),
                    );
                    let applied = rep.applied.0;
                    rep.bodies.retain(|_, b| b.header.height > applied);
                    self.oracle_on_start(r);
                    if !byz {
                        self.after_handle(r, &actions);
                    }
                    let actions = if byz {
                        self.byz_filter(r, actions, self.now)
                    } else {
                        actions
                    };
                    self.apply_actions(r, actions, self.now);
                    self.refill_txs(r);
                    self.refresh(r);
                }
                Err(e) => {
                    self.fail(format!("replica {r}: Core::new failed: {e}"));
                    return;
                }
            }
        }
        self.trace(m, "RESTART".to_owned());
    }

    /// The start-up check of the store id (§7.4 rule 3), then the installation event of every
    /// `(instance, key)` the log lacks (rule 2): the initial record `{I, K, height: g}` only for
    /// a key generated on this node, never over an existing record file.
    fn install_instances(&mut self, m: usize, replicas: &[usize]) {
        let mut store_id = self.machines[m].store_id;
        let mut keystore = std::mem::take(&mut self.machines[m].keystore);
        let rng = &mut self.rng;
        keystore.check_store_id(&mut store_id, || fresh_id(rng));
        for &r in replicas {
            let inst = self.replicas[r].inst;
            let instance = self.instances[inst].id;
            for key in self.replicas[r].keys.clone() {
                let exists = self.replicas[r].records.contains_key(&key);
                let id = fresh_id(&mut self.rng);
                if keystore.install_instance(&mut store_id, &instance, &key, exists, false, id) {
                    let record = SafetyRecord::fresh(instance, key.clone(), 0, None);
                    let bytes = record
                        .encode(&self.hasher)
                        .expect("encode an initial record");
                    self.replicas[r]
                        .records
                        .insert(key, Durable { record, bytes });
                }
            }
        }
        self.machines[m].keystore = keystore;
        self.machines[m].store_id = store_id;
    }

    fn refill_txs(&mut self, r: usize) {
        let inst = self.replicas[r].inst;
        let pending: Vec<(u64, bool)> = self.txs[inst]
            .iter()
            .filter(|(_, (_, _, committed))| committed.is_none())
            .map(|(id, (_, poison, _))| (*id, *poison))
            .collect();
        let pad = self.workload.map_or(0, |w| w.pad);
        for (id, poison) in pending {
            self.offer_tx(r, id, encode_tx(id, poison, pad));
        }
    }

    /// The `Init` the fake driver builds for replica `r` from its durable stores (§7.4).
    pub fn init_for(&self, r: usize) -> Init {
        let rep = &self.replicas[r];
        let inst = &self.instances[rep.inst];
        let tip = match rep.store.last() {
            None => CommittedTip {
                height: 0,
                block_hash: inst.genesis_hash,
                result: inst.genesis_result,
                header: None,
                commit_qc: None,
            },
            Some((block, qc)) => CommittedTip {
                height: block.header.height,
                block_hash: qc.block_hash,
                result: qc.result,
                header: Some(block.header.clone()),
                commit_qc: Some(qc.clone()),
            },
        };
        let t = tip.height;
        let mut configs = vec![(t + 1, inst.config(t + 1)), (t + 2, inst.config(t + 2))];
        if t > 0 {
            configs.push((t, inst.config(t)));
        }
        let window = usize::try_from(inst.window + 2).unwrap_or(usize::MAX);
        let skip = rep.store.len().saturating_sub(window);
        let recent_headers = rep
            .store
            .iter()
            .skip(skip)
            .map(|(b, _)| b.header.clone())
            .collect();
        let retired = &self.machines[rep.machine].retired;
        let records = rep
            .keys
            .iter()
            .map(|key| {
                let state = rep.records.get(key).map_or(RecordState::Absent, |d| {
                    RecordState::Present(d.bytes.clone())
                });
                (key.clone(), state, retired.contains(key))
            })
            .collect();
        Init {
            instance: inst.id,
            records,
            genesis_height: 0,
            demotion_window: inst.window,
            nonce: self.nonce_for_init(),
            tip,
            configs,
            recent_headers,
        }
    }

    /// A fresh `Init.nonce` from the world's PRNG (§13.1).
    fn nonce_for_init(&self) -> u64 {
        if cfg!(sumeragi_mutation = "MR-fresh-nonce") {
            return 1;
        }
        self.nonce_source.set(
            self.nonce_source
                .get()
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .wrapping_add(0x632b_e59b_d9b4_e019),
        );
        self.nonce_source.get()
    }

    fn apply_fault(&mut self, fault: Fault) {
        match fault {
            Fault::Crash(m) => self.crash(m),
            Fault::Restart(m) => {
                if !self.machines[m].up {
                    self.restart(m);
                }
            }
            Fault::CrashAll => {
                for m in 0..self.machines.len() {
                    self.crash(m);
                }
            }
            Fault::RestartAll => {
                for m in 0..self.machines.len() {
                    if !self.machines[m].up {
                        self.restart(m);
                    }
                }
            }
            Fault::CorruptRecord(m, i) => {
                if let Some(r) = self.machines[m].replicas.get(i).copied().flatten() {
                    for durable in self.replicas[r].records.values_mut() {
                        if let Some(byte) = durable.bytes.get_mut(7) {
                            *byte ^= 0x5a;
                        }
                    }
                }
            }
            Fault::DeleteRecord(m, i) => {
                if let Some(r) = self.machines[m].replicas.get(i).copied().flatten() {
                    self.replicas[r].records.clear();
                }
            }
            Fault::RestoreKeyStore(m) => {
                if let Some(snapshot) = self.machines[m].snapshot.clone() {
                    self.machines[m].keystore = snapshot;
                }
            }
            Fault::ReplaceRecordStore(m) => self.replace_record_store(m),
            Fault::ReinstallKey(m) => {
                self.replace_record_store(m);
                let mut keystore = KeyStore::default();
                let mut store_id = None;
                for key in self.machines[m].keys.clone() {
                    let id = fresh_id(&mut self.rng);
                    keystore.install_key(&mut store_id, &key, false, id);
                }
                self.machines[m].keystore = keystore;
                self.machines[m].store_id = store_id;
            }
            Fault::RetireKey(m, slot) => {
                let key = self.machines[m].keys.get(slot).cloned();
                if let Some(key) = key {
                    self.machines[m].retired.insert(key);
                }
            }
            Fault::ForgeRecordParent(m, i) => {
                if let Some(r) = self.machines[m].replicas.get(i).copied().flatten() {
                    let hasher = self.hasher.clone();
                    for durable in self.replicas[r].records.values_mut() {
                        if let Some(qc) = durable.record.parent_commit_qc.as_mut() {
                            qc.block_hash = Hash32([0x66; 32]);
                            if let Ok(bytes) = durable.record.encode(&hasher) {
                                durable.bytes = bytes;
                            }
                        }
                    }
                }
            }
            Fault::TruncateStore(m, i, k) => {
                if let Some(r) = self.machines[m].replicas.get(i).copied().flatten() {
                    let rep = &mut self.replicas[r];
                    let keep = rep.store.len().saturating_sub(k);
                    rep.store.truncate(keep);
                }
            }
            Fault::Custom(f) => f(self),
        }
    }

    /// A new, empty record store for machine `m`: every record file and the store id are gone.
    fn replace_record_store(&mut self, m: usize) {
        for r in self.machines[m].replicas.clone().into_iter().flatten() {
            self.replicas[r].records.clear();
        }
        self.machines[m].store_id = None;
    }

    /// Pre-build a committed chain of `len` heights signed by the genesis committee's keys
    /// (logged as genuine signatures) into the stores of `holders` (F17).
    fn prebuild(&mut self, len: u64, holders: &[usize]) {
        let inst = self.instances[0].clone();
        let signers: Vec<SimSigner> = inst
            .committee(1)
            .members()
            .iter()
            .map(|k| SimSigner::new(k.clone(), None, Rc::clone(&self.log)))
            .collect();
        let chain = super::oracle::build_chain(&inst, &signers, len, &self.hasher);
        // The pre-built history was exposed long ago.
        for (_, qc) in &chain {
            let msg = qc.preimage();
            if let Some(keys) = inst.committee(qc.height).keys_of(&qc.signers) {
                for key in keys {
                    self.log.borrow_mut().expose(key, &msg);
                }
            }
        }
        for &m in holders {
            if let Some(r) = self.machines[m].replicas.first().copied().flatten() {
                self.replicas[r].store.clone_from(&chain);
            }
        }
        self.oracle.adopt_chain(0, &chain);
    }

    /// Replica of machine `m` in instance `inst`.
    pub fn replica_of(&self, m: usize, inst: usize) -> Option<usize> {
        self.machines.get(m)?.replicas.get(inst).copied().flatten()
    }

    /// Committed height of replica `r` (core tip; store tip while crashed).
    pub fn committed(&self, r: usize) -> u64 {
        let rep = &self.replicas[r];
        rep.core.as_ref().map_or_else(
            || u64::try_from(rep.store.len()).unwrap_or(0),
            |core| core.status().committed_height,
        )
    }

    /// Transaction interval of the workload (upper end), if any.
    pub fn workload_every(&self) -> Option<Millis> {
        self.workload.map(|w| w.every_max)
    }

    /// Ground-truth topology of height `h` of instance `inst` (reference chain headers).
    pub fn ground_topology(&self, inst: usize, h: u64) -> crate::topology::Topology {
        let instance = &self.instances[inst];
        let window = instance.window;
        let lo = h.saturating_sub(1 + window).max(1);
        let headers: Vec<crate::message::BlockHeader> = (lo..h.saturating_sub(1))
            .filter_map(|x| self.oracle.refs[inst].get(&x).map(|b| b.header.clone()))
            .collect();
        crate::topology::Topology::compute(
            &self.hasher,
            &instance.id,
            instance.committee(h),
            h,
            0,
            window,
            &headers,
        )
    }

    /// Honest replicas.
    pub fn honest(&self) -> Vec<usize> {
        (0..self.replicas.len())
            .filter(|r| !self.machines[self.replicas[*r].machine].byz)
            .collect()
    }
}

fn describe_event(event: &Event) -> String {
    match event {
        Event::Tick => "Tick".to_owned(),
        Event::Message { msg, .. } => describe_msg(msg),
        Event::PayloadBuilt { req, payload } => {
            format!("PayloadBuilt req{req} {}B", payload.len())
        }
        Event::PayloadReady { req } => format!("PayloadReady req{req}"),
        Event::Executed { req, outcome, .. } => format!("Executed req{req} {outcome:?}"),
        Event::BodyAvailable { block } => format!("BodyAvailable h{}", block.header.height),
        Event::BlockApplied { height, .. } => format!("BlockApplied h{height}"),
        Event::ApplyDiverged { height, .. } => format!("ApplyDiverged h{height}"),
    }
}

/// A one-line summary of an action list for verbose traces.
fn summarize(actions: &[Action]) -> String {
    let mut out = String::new();
    for action in actions {
        let item = match action {
            Action::PersistSafety(r) => format!(
                "persist(h{} p{:?} l{:?} t{:?})",
                r.height,
                r.prepare.map(|v| v.view),
                r.lock.as_ref().map(|q| q.view),
                r.timeout.as_ref().map(|t| t.view)
            ),
            Action::StoreBody { block } => format!("store(h{})", block.header.height),
            Action::Send { to, msg } => format!(
                "send[{}]{}",
                &format!("{to:?}")[3..11],
                describe_msg(msg).trim_start_matches("<- ")
            ),
            Action::Broadcast { to, msg } => {
                format!(
                    "bcast[{}]{}",
                    to.len(),
                    describe_msg(msg).trim_start_matches("<- ")
                )
            }
            Action::BuildPayload {
                req, height, view, ..
            } => format!("build(req{req} h{height} v{view})"),
            Action::Execute { block, req } => format!("exec(h{} req{req})", block.header.height),
            Action::DiscardExecution { height, keep } => {
                format!("discard(h{height} keep {})", keep.len())
            }
            Action::CommitBlock { block, .. } => format!("COMMIT(h{})", block.header.height),
            Action::FetchBody { height, peers, .. } => {
                format!("fetch(h{height} {} peers)", peers.len())
            }
            Action::ServeBody { height, .. } => format!("serve_body(h{height})"),
            Action::ServeBlocks { from_height, .. } => format!("serve_blocks({from_height})"),
            Action::PayloadRejected { height, .. } => format!("rejected(h{height})"),
            Action::ReportEvidence(_) => "EVIDENCE".to_owned(),
            Action::LocalFault(f) => format!("fault({f:?})"),
            Action::Halt(h) => format!("HALT({h:?})"),
        };
        out.push_str(&item);
        out.push(' ');
    }
    out
}

/// A short description of a message for traces.
pub fn describe_msg(msg: &WireMessage) -> String {
    match msg {
        WireMessage::Proposal(p) => format!(
            "<- Proposal h{} v{} payload {}",
            p.height,
            p.view,
            p.payload
                .as_ref()
                .map_or_else(|| "none".to_owned(), |b| b.len().to_string())
        ),
        WireMessage::Vote(v) => format!(
            "<- Vote {:?} h{} v{} from #{}",
            v.kind, v.height, v.view, v.signer
        ),
        WireMessage::Qc(q) => format!("<- Qc {:?} h{} v{}", q.kind, q.height, q.view),
        WireMessage::Timeout(t) => format!(
            "<- Timeout h{} v{} hq {:?} from #{}",
            t.height,
            t.view,
            t.hq(),
            t.signer
        ),
        WireMessage::Tc(t) => format!("<- Tc h{} v{} max_hq {:?}", t.height, t.view, t.max_hq()),
        WireMessage::Status(s) => format!(
            "<- Status h{} v{} cqc {:?}{}{}{}",
            s.height,
            s.view,
            s.committed_qc.as_ref().map(|q| q.height),
            if s.want_proposal { " want" } else { "" },
            if s.probe.is_some() { " probe" } else { "" },
            if s.echo.is_some() { " echo" } else { "" },
        ),
        WireMessage::SyncRequest(q) => format!("<- SyncRequest from {}", q.from_height),
        WireMessage::SyncResponse(q) => format!("<- SyncResponse {} blocks", q.blocks.len()),
        WireMessage::BlockRequest(q) => format!("<- BlockRequest h{}", q.height),
        WireMessage::BlockResponse(q) => format!("<- BlockResponse h{}", q.block.header.height),
    }
}

/// Seeds to run: `SUMERAGI_SIM_SEED` (one seed) or `SUMERAGI_SIM_SEEDS` (count, from
/// `SUMERAGI_SIM_SEED_BASE`), else `default` seeds from 0.
pub fn seeds(default: u64) -> Vec<u64> {
    let var = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<u64>().ok());
    if let Some(seed) = var("SUMERAGI_SIM_SEED") {
        return vec![seed];
    }
    let base = var("SUMERAGI_SIM_SEED_BASE").unwrap_or(0);
    let count = var("SUMERAGI_SIM_SEEDS").unwrap_or(default);
    (base..base + count).collect()
}
