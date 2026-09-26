//! Scenario descriptions: everything a run needs besides the seed-derived randomness — the
//! committee and its schedule, network, clocks, per-machine profiles, Byzantine strategies,
//! scripted faults, crash churn, the workload and which oracles and bounds apply.

use super::{
    byz::{NetRule, Strategy},
    driver::Clock,
    host::{HostFactory, fake_host},
    net::NetConfig,
    world::World,
};
use crate::{
    api::LocalParams,
    types::{ChainParams, Millis},
};

/// Per-machine resources of the fake driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // independent fault switches
pub struct Profile {
    /// Execution latency of an empty block.
    pub exec_base: Millis,
    /// Extra execution latency per KiB of payload.
    pub exec_per_kib: Millis,
    /// Extra execution latency of every non-empty payload, unknown to the payload builder
    /// (its `exec_budget` estimate is wrong: a machine slower than calibrated; F15).
    pub exec_nonempty: Millis,
    /// O4 with preemption: a `DiscardExecution` that leaves the running job out of `keep`
    /// aborts it and answers `Cancelled` at once, so the next `Execute` starts without waiting
    /// for the discarded work (otherwise the running job finishes first; F15).
    pub abort_discarded: bool,
    /// Probability (ppm) that an execution reports `Failed`.
    pub exec_fail_ppm: u32,
    /// Divergent (nondeterministic) executor (F21).
    pub divergent: bool,
    /// A deterministic executor defect: every non-empty payload is `Invalid` (ML13).
    pub reject_nonempty: bool,
    /// Probability (ppm), per finished execution, of evicting a random cached post-state.
    pub evict_ppm: u32,
    /// Write latency range.
    pub write_min: Millis,
    /// Write latency range.
    pub write_max: Millis,
    /// Probability (ppm) that a write fails and is retried (ENOSPC/EIO, F27).
    pub write_fail_ppm: u32,
    /// Retry backoff of a failed write.
    pub write_retry: Millis,
    /// Virtual CPU cost per pairing (µs).
    pub cpu_us_per_pairing: u64,
    /// Virtual CPU cost per handled event (µs).
    pub cpu_us_per_event: u64,
    /// Payload build latency range.
    pub build_min: Millis,
    /// Payload build latency range.
    pub build_max: Millis,
    /// Apply latency after the block store write.
    pub apply_ms: Millis,
    /// Extra latency of block-store writes (F32).
    pub block_write_extra: Millis,
    /// Ingress without priority lanes (FIFO; only to show that `det_l12` detects ML12).
    pub fifo_ingress: bool,
    /// The machine's commit-attestation authority (§3.7, F37).
    pub authority: Authority,
}

/// A machine's commit-attestation authority (§3.7, F37).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Authority {
    /// Attests genuinely for every key.
    #[default]
    Full,
    /// Holds no authority: attests nothing (so it does not Commit-vote on flagged blocks).
    Missing,
    /// A broken authority whose attestations never verify (the node's own verifier rejects
    /// them, so it does not Commit-vote on flagged blocks either).
    Forging,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            exec_base: 10,
            exec_per_kib: 1,
            exec_nonempty: 0,
            abort_discarded: false,
            exec_fail_ppm: 0,
            divergent: false,
            reject_nonempty: false,
            evict_ppm: 0,
            write_min: 1,
            write_max: 4,
            write_fail_ppm: 0,
            write_retry: 20,
            cpu_us_per_pairing: 150,
            cpu_us_per_event: 50,
            build_min: 1,
            build_max: 5,
            apply_ms: 5,
            block_write_extra: 0,
            fifo_ingress: false,
            authority: Authority::Full,
        }
    }
}

/// A scripted fault or intervention at a fixed time.
pub enum Fault {
    /// Crash a machine (non-durable writes are lost).
    Crash(usize),
    /// Restart a crashed machine from its durable stores.
    Restart(usize),
    /// Crash every machine at once.
    CrashAll,
    /// Restart every crashed machine.
    RestartAll,
    /// Corrupt the durable safety record of a machine's replica of an instance (R1).
    CorruptRecord(usize, usize),
    /// Delete the durable safety records of a machine's replica of an instance (R2).
    DeleteRecord(usize, usize),
    /// Restore a machine's key store (with its installation log) from its snapshot; the record
    /// files and the store id are untouched (§7.4 record provenance rule 3).
    RestoreKeyStore(usize),
    /// Replace a machine's record store by a new, empty one (all record files and the store id
    /// are gone).
    ReplaceRecordStore(usize),
    /// Reinstall a machine's keys from a KMS onto a new disk: a new key store in which every
    /// key is imported, and an empty record store (every record `Absent`).
    ReinstallKey(usize),
    /// Stop configuring machine `m`'s key of slot `slot` for signing: it becomes retired (its
    /// record and log entries are kept, §7.4 Keys). Effective at the next restart.
    RetireKey(usize, usize),
    /// Drop the last `k` blocks of a replica's block store (Kura tail loss, R5/R6).
    TruncateStore(usize, usize, usize),
    /// Replace the parent `CommitQC` in a replica's durable record by a forged one, with a
    /// valid checksum (a consistent but wrong record; R5 must refuse it).
    ForgeRecordParent(usize, usize),
    /// Anything else, with full access to the world.
    Custom(Box<dyn Fn(&mut World)>),
}

impl core::fmt::Debug for Fault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Crash(m) => write!(f, "Crash({m})"),
            Self::Restart(m) => write!(f, "Restart({m})"),
            Self::CrashAll => write!(f, "CrashAll"),
            Self::RestartAll => write!(f, "RestartAll"),
            Self::CorruptRecord(m, i) => write!(f, "CorruptRecord({m}, {i})"),
            Self::DeleteRecord(m, i) => write!(f, "DeleteRecord({m}, {i})"),
            Self::RestoreKeyStore(m) => write!(f, "RestoreKeyStore({m})"),
            Self::ReplaceRecordStore(m) => write!(f, "ReplaceRecordStore({m})"),
            Self::ReinstallKey(m) => write!(f, "ReinstallKey({m})"),
            Self::RetireKey(m, slot) => write!(f, "RetireKey({m}, {slot})"),
            Self::TruncateStore(m, i, k) => write!(f, "TruncateStore({m}, {i}, {k})"),
            Self::ForgeRecordParent(m, i) => write!(f, "ForgeRecordParent({m}, {i})"),
            Self::Custom(_) => write!(f, "Custom"),
        }
    }
}

/// Where churn crashes land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashPoint {
    /// Random: in the middle of any action list or at any write completion.
    Random,
    /// Right after a `PersistSafety` recording a proposal (before durability).
    ProposalRecord,
    /// Right after a `PersistSafety` recording a new vote (before durability).
    VoteRecord,
    /// Right after a `PersistSafety` recording a timeout (before durability).
    TimeoutRecord,
    /// Right after a write becomes durable, before held effects leave.
    AfterDurable,
    /// Between the block store write and `BlockApplied` (inside apply).
    InsideApply,
}

/// Random crash-restart churn (F13).
#[derive(Clone, Debug)]
pub struct Churn {
    /// Window start.
    pub from: Millis,
    /// Window end (no crash after it; every machine is restarted by then).
    pub until: Millis,
    /// Crash probability (ppm) per eligible point.
    pub ppm: u32,
    /// Down time range.
    pub down_min: Millis,
    /// Down time range.
    pub down_max: Millis,
    /// At most this many machines down at once.
    pub max_down: usize,
    /// Crash points.
    pub points: Vec<CrashPoint>,
    /// Machines eligible (empty = all honest).
    pub targets: Vec<usize>,
}

/// Transaction workload of an instance.
#[derive(Clone, Copy, Debug)]
pub struct Workload {
    /// Interval between transactions (uniform range).
    pub every_min: Millis,
    /// Interval between transactions (uniform range).
    pub every_max: Millis,
    /// Padding bytes per transaction.
    pub pad: u16,
    /// Probability (ppm) that a transaction is poison (F19).
    pub poison_ppm: u32,
    /// No transactions after this time.
    pub until: Millis,
    /// Bit mask of the machines that receive transactions (0 = all; F35 local-queue
    /// asymmetry).
    pub targets: u64,
    /// Every `mint_every`-th transaction needs mint finality, so the block holding it is
    /// flagged (§3.7, F37); 0 = none.
    pub mint_every: u64,
}

impl Workload {
    /// Whether transaction `id` needs mint finality.
    pub fn mints(&self, id: u64) -> bool {
        self.mint_every > 0 && id.is_multiple_of(self.mint_every)
    }
}

impl Default for Workload {
    fn default() -> Self {
        Self {
            every_min: 100,
            every_max: 300,
            pad: 32,
            poison_ppm: 0,
            until: Millis::MAX,
            targets: 0,
            mint_every: 0,
        }
    }
}

/// Performance bound checked by O-PERF (§8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perf {
    /// No performance bound.
    None,
    /// P1: all honest, loss ≤ 1 %.
    P1,
    /// P2: one vote-withholding member.
    P2,
    /// P3: silent or withholding proxy tail.
    P3,
    /// P4: one crashed member.
    P4,
    /// P5: first commit after heal.
    P5,
    /// P6: tick lateness under a flood.
    P6,
    /// Every gap within the P4 leader-turn bound (at most one failed view per height; ML18),
    /// without P4's count limit.
    OneViewFailure,
}

/// Which oracles and bounds apply to a scenario.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // independent oracle switches
pub struct Checks {
    /// O-LIVE after `heal_at`.
    pub liveness: bool,
    /// O-PERF bound.
    pub perf: Perf,
    /// Sanity: every honest running node commits at least this many heights after heal.
    pub progress: u64,
    /// O-TXP (poison present).
    pub txp: bool,
    /// O-CQ (no crashes).
    pub cq: bool,
    /// Machines allowed to halt (injected corruption or divergence), for O-HALT.
    pub may_halt: Vec<usize>,
    /// Machines allowed to report executor faults (O-FAULT).
    pub may_fault: Vec<usize>,
    /// No view change after heal: every committed block is proposed and committed in view 0
    /// (F35: no timer may move).
    pub no_view_change: bool,
    /// Instances exempt from O-LIVE/progress until the given time (F31 stalls).
    pub stalled: Vec<(usize, Millis)>,
    /// `(instance, from, until, heights)`: at least `heights` commits in the window.
    pub windows: Vec<(usize, Millis, Millis, usize)>,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            liveness: true,
            perf: Perf::None,
            progress: 3,
            txp: false,
            cq: false,
            may_halt: Vec::new(),
            may_fault: Vec::new(),
            no_view_change: false,
            stalled: Vec::new(),
            windows: Vec::new(),
        }
    }
}

/// A complete scenario.
pub struct Scenario {
    /// Name (e.g. `F5`).
    pub name: String,
    /// Seed of this run.
    pub seed: u64,
    /// Size of the initial committee (machines `0..n`).
    pub n: usize,
    /// Machines outside the initial committee (joiners, observers).
    pub extra: usize,
    /// Number of consensus instances in the world.
    pub instances: usize,
    /// Every instance uses the same keys (F20).
    pub shared_keys: bool,
    /// Simulated duration.
    pub duration: Millis,
    /// Heal time / GST `t_g`.
    pub heal_at: Millis,
    /// Local parameters of every node.
    pub local: LocalParams,
    /// Chain parameters.
    pub params: ChainParams,
    /// The demotion window `W` (a genesis constant of every instance, §2.1).
    pub demotion_window: u64,
    /// Take a snapshot of every key store after the keys are installed and before any instance
    /// starts (restored by [`Fault::RestoreKeyStore`]).
    pub keystore_snapshot: bool,
    /// Network.
    pub net: NetConfig,
    /// Per-machine clocks (missing entries: default clock).
    pub clocks: Vec<Clock>,
    /// Per-machine profiles (missing entries: default profile).
    pub profiles: Vec<Profile>,
    /// Byzantine machines and their strategies.
    pub byz: Vec<(usize, Vec<Strategy>)>,
    /// Rules of the network adversary (before heal).
    pub net_rules: Vec<NetRule>,
    /// `Strategy::SilentLeader` stays honest before this time.
    pub silent_leader_from: Millis,
    /// Scripted faults.
    pub script: Vec<(Millis, Fault)>,
    /// Random crash churn.
    pub churn: Option<Churn>,
    /// Workload (per instance), `None` = idle.
    pub workload: Option<Workload>,
    /// Committee schedule: from height → members as `(machine, key slot)`; the first entry
    /// must start at height 0 (it is the genesis committee).
    pub committees: Vec<(u64, Vec<(usize, usize)>)>,
    /// Oracles and bounds.
    pub checks: Checks,
    /// Pre-built committed chain length (F17): machines in `prebuilt_holders` start with it.
    pub prebuilt: u64,
    /// Machines whose stores hold the pre-built chain.
    pub prebuilt_holders: Vec<usize>,
    /// The node implementation of every replica (§13.5; default: the fake driver).
    pub host: HostFactory,
}

impl Scenario {
    /// A lossless all-honest scenario of `n` validators with default parameters.
    pub fn base(name: &str, seed: u64, n: usize) -> Self {
        let n = n.max(1);
        Self {
            name: name.to_owned(),
            seed,
            n,
            extra: 0,
            instances: 1,
            shared_keys: false,
            duration: 60_000,
            heal_at: 0,
            local: LocalParams::for_committee_size(n),
            params: ChainParams::default(),
            demotion_window: 128,
            keystore_snapshot: false,
            net: NetConfig::default(),
            clocks: Vec::new(),
            profiles: Vec::new(),
            byz: Vec::new(),
            net_rules: Vec::new(),
            silent_leader_from: 0,
            script: Vec::new(),
            churn: None,
            workload: Some(Workload::default()),
            committees: vec![(0, (0..n).map(|m| (m, 0)).collect())],
            checks: Checks::default(),
            prebuilt: 0,
            prebuilt_holders: Vec::new(),
            host: fake_host,
        }
    }

    /// Machines in the world.
    pub fn machines(&self) -> usize {
        self.n + self.extra
    }

    /// Profile of machine `m`.
    pub fn profile(&self, m: usize) -> Profile {
        self.profiles.get(m).copied().unwrap_or_default()
    }

    /// Whether machine `m` is Byzantine.
    pub fn is_byz(&self, m: usize) -> bool {
        self.byz.iter().any(|(b, _)| *b == m)
    }

    /// Set the profile of machine `m`.
    pub fn set_profile(&mut self, m: usize, profile: Profile) {
        if self.profiles.len() <= m {
            self.profiles.resize(m + 1, Profile::default());
        }
        self.profiles[m] = profile;
    }
}
