//! Scoped phase recorder.
//!
//! One [`Session`] measures one root flow. Instrumented code opens phases with
//! static labels, reports public sizes and counts, and receives nothing back:
//! every recording method returns `()` or an opaque guard. The finished record
//! goes to the sink supplied at [`Session::begin`], and the session's own
//! buffers are overwritten and released whether the session finishes, is
//! dropped on an error return, or is dropped while its thread unwinds.
//!
//! All clocks are sampled in one event order under the session lock, so
//! inclusive and exclusive times are exact integer nanoseconds and exclusive
//! time is never negative, including when children run on other threads.

use std::{
    marker::PhantomData,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
    time::Instant,
};

use iroha_allocation::AllocationBudget;

use crate::{
    platform::{self, ResourceUsage},
    schema::{
        AddressSpaceObservation, AllocationObservation, AllocationSource, AllocationSourceKind,
        ByteCounter, ByteCounters, ByteKind, Classification, DeclaredQuantity, FailureLog,
        MeasurementRecord, PeakRssSource, PhaseNode, PhaseTree, PhaseWorkers, ProcessObservation,
        ProvenanceKind, RECORD_SCHEMA_V1, RawFailure, RecorderHealth, RunIdentity, RunOutcome,
        Scheduling, ThermalState, Unit, WorkCounter, WorkCounters,
    },
    sink::RecordSink,
    text,
};

/// Most distinct phases (parent and label pairs) one session records.
pub const MAX_NODES: usize = 4096;
/// Most phases open at once across all threads of one session.
pub const MAX_OPEN_SPANS: usize = 1024;
/// Most distinct named byte counters one session records.
pub const MAX_BYTE_COUNTERS: usize = 1024;
/// Most distinct named work counters one session records.
pub const MAX_WORK_COUNTERS: usize = 1024;
/// Most allocation sources one session observes.
pub const MAX_ALLOCATION_SOURCES: usize = 64;
/// Most caller-declared quantities one session records.
pub const MAX_DECLARED: usize = 256;
/// Most raw failures one session retains before counting the excess.
pub const MAX_FAILURES: usize = 256;
/// Most per-phase worker declarations one session records.
pub const MAX_PHASE_WORKERS: usize = 256;

const NONE: u32 = u32::MAX;
const BUDGET_LIMIT_PROVENANCE: &str = "iroha_allocation.AllocationBudget.limit_bytes";

/// Worker count the caller scheduled for the session and what set it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerDeclaration {
    /// Number of workers.
    pub workers: u32,
    /// Static public reference to what set that number.
    pub provenance: &'static str,
}

/// What a caller-declared quantity is and what establishes it.
///
/// A deterministic consensus bound can only be declared with protocol or
/// committed-State provenance: a label alone establishes no bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclaredClass {
    /// A modelled or extrapolated value.
    Projection,
    /// A performance goal stated by the plan or a specification.
    EngineeringTarget,
    /// A consensus bound fixed by a protocol constant.
    ConsensusBoundFromProtocolConstant,
    /// A consensus bound read from committed protocol State.
    ConsensusBoundFromCommittedState,
    /// A node-local configuration value.
    LocalConfiguration,
    /// A limit the operating system enforces on this process.
    OperatingSystemLimit,
}

impl DeclaredClass {
    /// Classification and provenance kind written into the record.
    pub const fn schema_pair(self) -> (Classification, ProvenanceKind) {
        match self {
            Self::Projection => (Classification::Projection, ProvenanceKind::ModelProjection),
            Self::EngineeringTarget => (
                Classification::EngineeringTarget,
                ProvenanceKind::PlanTarget,
            ),
            Self::ConsensusBoundFromProtocolConstant => (
                Classification::DeterministicConsensusBound,
                ProvenanceKind::ProtocolConstant,
            ),
            Self::ConsensusBoundFromCommittedState => (
                Classification::DeterministicConsensusBound,
                ProvenanceKind::CommittedState,
            ),
            Self::LocalConfiguration => (
                Classification::LocalScheduling,
                ProvenanceKind::LocalConfiguration,
            ),
            Self::OperatingSystemLimit => (
                Classification::LocalScheduling,
                ProvenanceKind::OperatingSystem,
            ),
        }
    }
}

/// One caller-declared quantity: never a measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// What the value is and what establishes it.
    pub class: DeclaredClass,
    /// Static public label of the quantity.
    pub label: &'static str,
    /// Unit of `value`.
    pub unit: Unit,
    /// The declared value.
    pub value: u64,
    /// Static reference to the establishing source (path and symbol, State key).
    pub provenance: &'static str,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Node {
    parent: u32,
    label: &'static str,
    calls: u64,
    completed: u64,
    interrupted: u64,
    unwound: u64,
    truncated: u64,
    wall_inclusive_ns: u64,
    wall_exclusive_ns: u64,
    thread_cpu_inclusive_ns: u64,
    thread_cpu_exclusive_ns: u64,
    process_cpu_inclusive_ns: u64,
    process_cpu_exclusive_ns: u64,
    peak_concurrent_children: u32,
    active_threads_max: u32,
    entered: bool,
    load_first: u64,
    load_last: u64,
    load_max: u64,
    allocations: u64,
    allocated_bytes: u64,
    live_high_water: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Span {
    open: bool,
    generation: u64,
    sequence: u64,
    node: u32,
    parent: u32,
    thread: Option<ThreadId>,
    start_wall: u64,
    start_process_cpu: u64,
    start_thread_cpu: Option<u64>,
    cover_start_wall: u64,
    cover_start_process_cpu: u64,
    covered_wall: u64,
    covered_process_cpu: u64,
    open_children: u32,
    peak_children: u32,
    child_thread_cpu: u64,
    live_high_water: u64,
}

struct Source {
    label: &'static str,
    kind: AllocationSourceKind,
    budget: Option<AllocationBudget>,
    allocations: u64,
    allocated_bytes: u64,
    frees: u64,
    freed_bytes: u64,
    live_bytes: u64,
    live_high_water: u64,
    live_buffers: u64,
    live_buffers_high_water: u64,
}

impl Source {
    const fn empty() -> Self {
        Self {
            label: "",
            kind: AllocationSourceKind::ScopedCounter,
            budget: None,
            allocations: 0,
            allocated_bytes: 0,
            frees: 0,
            freed_bytes: 0,
            live_bytes: 0,
            live_high_water: 0,
            live_buffers: 0,
            live_buffers_high_water: 0,
        }
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.label.is_empty()
            && self.budget.is_none()
            && self.allocations == 0
            && self.allocated_bytes == 0
            && self.frees == 0
            && self.freed_bytes == 0
            && self.live_bytes == 0
            && self.live_high_water == 0
            && self.live_buffers == 0
            && self.live_buffers_high_water == 0
    }

    fn live_now(&self) -> u64 {
        self.budget.as_ref().map_or(self.live_bytes, |budget| {
            u64::try_from(budget.reserved_bytes()).unwrap_or(u64::MAX)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ByteEntry {
    node: u32,
    kind: ByteKind,
    label: &'static str,
    count: u64,
    total: u64,
    min: u64,
    max: u64,
}

impl ByteEntry {
    const EMPTY: Self = Self {
        node: 0,
        kind: ByteKind::Other,
        label: "",
        count: 0,
        total: 0,
        min: 0,
        max: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkEntry {
    node: u32,
    label: &'static str,
    count: u64,
    total: u64,
    min: u64,
    max: u64,
}

impl WorkEntry {
    const EMPTY: Self = Self {
        node: 0,
        label: "",
        count: 0,
        total: 0,
        min: 0,
        max: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Declared {
    class: DeclaredClass,
    label: &'static str,
    unit: Unit,
    value: u64,
    provenance: &'static str,
}

impl Declared {
    const EMPTY: Self = Self {
        class: DeclaredClass::Projection,
        label: "",
        unit: Unit::Count,
        value: 0,
        provenance: "",
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Failure {
    node: u32,
    stage: &'static str,
    code: &'static str,
}

/// Lifecycle of the buffers a session owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Recording.
    Live,
    /// Every element was overwritten with its empty value; lengths unchanged.
    Scrubbed,
    /// Every buffer was emptied and its allocation returned.
    Released,
}

struct State {
    stage: Stage,
    identity: Option<RunIdentity>,
    sink: Option<Box<dyn RecordSink>>,
    nodes: Vec<Node>,
    spans: Vec<Span>,
    sources: Vec<Source>,
    bytes: Vec<ByteEntry>,
    work: Vec<WorkEntry>,
    declared: Vec<Declared>,
    failures: Vec<Failure>,
    phase_workers: Vec<PhaseWorkers>,
    threads: Vec<ThreadId>,
    dropped_failures: u64,
    workers: u32,
    workers_provenance: &'static str,
    health: RecorderHealth,
    next_generation: u64,
    next_sequence: u64,
    last_wall: u64,
    last_process_cpu: u64,
    begin_usage: Option<ResourceUsage>,
    begin_load: u64,
    begin_thermal: Option<ThermalState>,
    root_slot: u32,
}

/// Source of every clock and load sample a session takes.
///
/// Production sessions read the operating system. Unit tests inject a manual
/// clock so that exact interval arithmetic, including overlapping parallel
/// children and the one-percent boundary, is checked without real time.
trait Clock: Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe {
    /// Monotonic wall time since the session's origin.
    fn wall_ns(&self) -> u64;
    /// CPU time of the whole process, when the platform reports it.
    fn process_cpu_ns(&self) -> Option<u64>;
    /// CPU time of the calling thread, when the platform reports it.
    fn thread_cpu_ns(&self) -> Option<u64>;
    /// One-minute system load in thousandths.
    fn load_milli(&self) -> u64;
}

/// The operating system's clocks, with wall time counted from `origin`.
struct SystemClock {
    origin: Instant,
}

impl Clock for SystemClock {
    fn wall_ns(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn process_cpu_ns(&self) -> Option<u64> {
        platform::process_cpu_ns()
    }

    fn thread_cpu_ns(&self) -> Option<u64> {
        platform::thread_cpu_ns()
    }

    fn load_milli(&self) -> u64 {
        platform::load_average_milli()
    }
}

/// Boundary samples taken by the calling thread before it takes the lock.
#[derive(Clone, Copy)]
struct Boundary {
    thread: ThreadId,
    thread_cpu: Option<u64>,
    load: u64,
}

impl Boundary {
    fn sample(clock: &dyn Clock) -> Self {
        Self {
            thread: std::thread::current().id(),
            thread_cpu: clock.thread_cpu_ns(),
            load: clock.load_milli(),
        }
    }
}

/// Replace identity text outside its grammar before the session records it.
///
/// Identity is the only caller-supplied owned text a session accepts. A value
/// outside the closed grammar is overwritten and replaced by the fixed
/// invalid label, exactly as a malformed phase label is, so off-grammar text
/// never reaches a record through the recorder. The unbound sentinel stays.
fn public_identity(mut identity: RunIdentity) -> (RunIdentity, u64) {
    fn replace(text: &mut String, valid: bool) -> u64 {
        if valid {
            return 0;
        }
        let mut rejected = core::mem::take(text).into_bytes();
        rejected.fill(0);
        std::hint::black_box(&rejected);
        text::INVALID_LABEL.clone_into(text);
        1
    }
    let mut replaced = 0;
    let valid = text::is_public_label(&identity.workload);
    replaced += replace(&mut identity.workload, valid);
    let valid = text::is_public_label(&identity.emitter);
    replaced += replace(&mut identity.emitter, valid);
    let context = &mut identity.context;
    let valid =
        text::is_source_commit(&context.source_commit) || context.source_commit == text::UNBOUND;
    replaced += replace(&mut context.source_commit, valid);
    for field in [
        &mut context.artifact,
        &mut context.profile,
        &mut context.config,
        &mut context.hardware,
    ] {
        let valid = text::is_identity_text(field);
        replaced += replace(field, valid);
    }
    if let Some(digest) = &mut context.source_dirty_digest {
        let valid = text::is_sha256_hex(digest);
        replaced += replace(digest, valid);
    }
    (identity, replaced)
}

fn index(value: u32) -> usize {
    value as usize
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(NONE)
}

impl State {
    fn new(
        identity: RunIdentity,
        workers: WorkerDeclaration,
        sink: Box<dyn RecordSink>,
        clock: &dyn Clock,
    ) -> Self {
        let (provenance, valid) = identity_or_invalid(workers.provenance);
        let (identity, replaced_identity_text) = public_identity(identity);
        let mut state = Self {
            stage: Stage::Live,
            identity: Some(identity),
            sink: Some(sink),
            nodes: Vec::new(),
            spans: Vec::new(),
            sources: Vec::new(),
            bytes: Vec::new(),
            work: Vec::new(),
            declared: Vec::new(),
            failures: Vec::new(),
            phase_workers: Vec::new(),
            threads: Vec::new(),
            dropped_failures: 0,
            workers: workers.workers,
            workers_provenance: provenance,
            health: RecorderHealth::default(),
            next_generation: 1,
            next_sequence: 1,
            last_wall: 0,
            last_process_cpu: 0,
            begin_usage: platform::resource_usage(),
            begin_load: clock.load_milli(),
            begin_thermal: Some(platform::thermal_state().0),
            root_slot: NONE,
        };
        state.health.invalid_label_events += u64::from(!valid) + replaced_identity_text;
        state
    }

    /// Sample the wall and process CPU clocks in event order.
    fn sample(&mut self, clock: &dyn Clock) -> (u64, u64) {
        let wall = clock.wall_ns();
        let process_cpu = clock.process_cpu_ns().unwrap_or(self.last_process_cpu);
        if wall < self.last_wall || process_cpu < self.last_process_cpu {
            self.health.clock_anomaly_events += 1;
        }
        self.last_wall = self.last_wall.max(wall);
        self.last_process_cpu = self.last_process_cpu.max(process_cpu);
        (self.last_wall, self.last_process_cpu)
    }

    fn span_is(&self, slot: u32, generation: u64) -> bool {
        self.spans
            .get(index(slot))
            .is_some_and(|span| span.open && span.generation == generation)
    }

    /// The most recently opened phase still open on `thread`.
    fn innermost_on(&self, thread: ThreadId) -> Option<u32> {
        self.spans
            .iter()
            .enumerate()
            .filter(|(_, span)| span.open && span.thread == Some(thread))
            .max_by_key(|(_, span)| span.sequence)
            .map(|(slot, _)| narrow(slot))
    }

    fn attribution_slot(&self, thread: ThreadId) -> u32 {
        self.innermost_on(thread).unwrap_or(self.root_slot)
    }

    fn live_total(&self) -> u64 {
        self.sources.iter().fold(0_u64, |total, source| {
            total.saturating_add(source.live_now())
        })
    }

    /// Distinct threads that currently have an open phase.
    fn active_threads(&mut self) -> u32 {
        self.threads.clear();
        for span in &self.spans {
            if let (true, Some(thread)) = (span.open, span.thread)
                && !self.threads.contains(&thread)
            {
                self.threads.push(thread);
            }
        }
        narrow(self.threads.len())
    }

    fn node_for(&mut self, parent: u32, label: &'static str) -> Option<u32> {
        if let Some(found) = self
            .nodes
            .iter()
            .position(|node| node.parent == parent && node.label == label)
        {
            return Some(narrow(found));
        }
        if self.nodes.len() >= MAX_NODES {
            self.health.node_overflow_events += 1;
            return None;
        }
        self.nodes.push(Node {
            parent,
            label,
            ..Node::default()
        });
        Some(narrow(self.nodes.len() - 1))
    }

    fn free_slot(&mut self) -> Option<u32> {
        if let Some(found) = self.spans.iter().position(|span| !span.open) {
            return Some(narrow(found));
        }
        if self.spans.len() >= MAX_OPEN_SPANS {
            self.health.span_overflow_events += 1;
            return None;
        }
        self.spans.push(Span::default());
        Some(narrow(self.spans.len() - 1))
    }

    fn open_span(
        &mut self,
        clock: &dyn Clock,
        label: &'static str,
        explicit_parent: Option<(u32, u64)>,
        boundary: Boundary,
    ) -> Option<(u32, u64)> {
        if self.stage != Stage::Live {
            return None;
        }
        let (label, valid) = text::public_or_invalid(label);
        if !valid {
            self.health.invalid_label_events += 1;
        }
        let parent_slot = match explicit_parent {
            Some((slot, generation)) if self.span_is(slot, generation) => slot,
            Some(_) => {
                self.health.stale_parent_events += 1;
                self.root_slot
            }
            None if self.root_slot == NONE => NONE,
            None => self.attribution_slot(boundary.thread),
        };
        let parent_node = if parent_slot == NONE {
            NONE
        } else {
            self.spans.get(index(parent_slot))?.node
        };
        let slot = self.free_slot()?;
        let node = self.node_for(parent_node, label)?;
        let (wall, process_cpu) = self.sample(clock);
        if let Some(parent) = self.spans.get_mut(index(parent_slot)) {
            if parent.open_children == 0 {
                parent.cover_start_wall = wall;
                parent.cover_start_process_cpu = process_cpu;
            }
            parent.open_children = parent.open_children.saturating_add(1);
            parent.peak_children = parent.peak_children.max(parent.open_children);
        }
        let generation = self.next_generation;
        self.next_generation += 1;
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let live = self.live_total();
        *self.spans.get_mut(index(slot))? = Span {
            open: true,
            generation,
            sequence,
            node,
            parent: parent_slot,
            thread: Some(boundary.thread),
            start_wall: wall,
            start_process_cpu: process_cpu,
            start_thread_cpu: boundary.thread_cpu,
            live_high_water: live,
            ..Span::default()
        };
        let threads = self.active_threads();
        let entry = self.nodes.get_mut(index(node))?;
        entry.calls += 1;
        if !entry.entered {
            entry.entered = true;
            entry.load_first = boundary.load;
        }
        entry.load_max = entry.load_max.max(boundary.load);
        entry.active_threads_max = entry.active_threads_max.max(threads);
        Some((slot, generation))
    }

    /// Close one open phase after first closing every phase still open under it.
    #[allow(clippy::too_many_arguments)]
    fn close_span(
        &mut self,
        slot: u32,
        wall: u64,
        process_cpu: u64,
        boundary: Boundary,
        completed: bool,
        unwinding: bool,
        forced: bool,
    ) {
        while let Some(child) = self
            .spans
            .iter()
            .position(|span| span.open && span.parent == slot)
        {
            self.close_span(
                narrow(child),
                wall,
                process_cpu,
                boundary,
                false,
                unwinding,
                true,
            );
        }
        let Some(span) = self.spans.get(index(slot)).copied() else {
            return;
        };
        if !span.open {
            return;
        }
        let live = self.live_total();
        let threads = self.active_threads();
        let wall_inclusive = wall.saturating_sub(span.start_wall);
        let wall_covered = span.covered_wall.min(wall_inclusive);
        let process_inclusive = process_cpu.saturating_sub(span.start_process_cpu);
        let process_covered = span.covered_process_cpu.min(process_inclusive);
        // Only the opening thread can read its own CPU clock. A phase closed
        // from another thread keeps the CPU of its already closed same-thread
        // children and attributes no further CPU to itself.
        let own_thread = span.thread == Some(boundary.thread);
        let thread_inclusive = match (own_thread, boundary.thread_cpu, span.start_thread_cpu) {
            (true, Some(end), Some(start)) => end.saturating_sub(start).max(span.child_thread_cpu),
            _ => span.child_thread_cpu,
        };
        let live_high_water = span.live_high_water.max(live);
        if let Some(node) = self.nodes.get_mut(index(span.node)) {
            if completed {
                node.completed += 1;
            } else {
                node.interrupted += 1;
                node.unwound += u64::from(unwinding);
                node.truncated += u64::from(forced);
            }
            node.wall_inclusive_ns = node.wall_inclusive_ns.saturating_add(wall_inclusive);
            node.wall_exclusive_ns = node
                .wall_exclusive_ns
                .saturating_add(wall_inclusive - wall_covered);
            node.thread_cpu_inclusive_ns = node
                .thread_cpu_inclusive_ns
                .saturating_add(thread_inclusive);
            node.thread_cpu_exclusive_ns = node
                .thread_cpu_exclusive_ns
                .saturating_add(thread_inclusive - span.child_thread_cpu);
            node.process_cpu_inclusive_ns = node
                .process_cpu_inclusive_ns
                .saturating_add(process_inclusive);
            node.process_cpu_exclusive_ns = node
                .process_cpu_exclusive_ns
                .saturating_add(process_inclusive - process_covered);
            node.peak_concurrent_children = node.peak_concurrent_children.max(span.peak_children);
            node.active_threads_max = node.active_threads_max.max(threads);
            node.load_last = boundary.load;
            node.load_max = node.load_max.max(boundary.load);
            node.live_high_water = node.live_high_water.max(live_high_water);
        }
        if forced {
            self.health.forced_closes += 1;
        }
        if let Some(parent) = self.spans.get_mut(index(span.parent)) {
            parent.open_children = parent.open_children.saturating_sub(1);
            if parent.open_children == 0 {
                parent.covered_wall = parent
                    .covered_wall
                    .saturating_add(wall.saturating_sub(parent.cover_start_wall));
                parent.covered_process_cpu = parent
                    .covered_process_cpu
                    .saturating_add(process_cpu.saturating_sub(parent.cover_start_process_cpu));
            }
            if parent.thread == span.thread {
                parent.child_thread_cpu = parent.child_thread_cpu.saturating_add(thread_inclusive);
            }
            parent.live_high_water = parent.live_high_water.max(live_high_water);
        }
        if let Some(entry) = self.spans.get_mut(index(slot)) {
            *entry = Span::default();
        }
    }

    fn build_record(
        &mut self,
        outcome: RunOutcome,
        usage: Option<ResourceUsage>,
        load: u64,
        thermal: (ThermalState, &'static str),
    ) -> Option<MeasurementRecord> {
        let identity = self.identity.take()?;
        let nodes = self
            .nodes
            .iter()
            .map(|node| PhaseNode {
                parent: (node.parent != NONE).then_some(node.parent),
                label: node.label.to_owned(),
                calls: node.calls,
                completed: node.completed,
                interrupted: node.interrupted,
                unwound: node.unwound,
                truncated: node.truncated,
                wall_inclusive_ns: node.wall_inclusive_ns,
                wall_exclusive_ns: node.wall_exclusive_ns,
                thread_cpu_inclusive_ns: node.thread_cpu_inclusive_ns,
                thread_cpu_exclusive_ns: node.thread_cpu_exclusive_ns,
                process_cpu_window_inclusive_ns: node.process_cpu_inclusive_ns,
                process_cpu_window_exclusive_ns: node.process_cpu_exclusive_ns,
                peak_concurrent_children: node.peak_concurrent_children,
                active_threads_max: node.active_threads_max,
                load_milli_first_enter: node.load_first,
                load_milli_last_exit: node.load_last,
                load_milli_max: node.load_max,
                allocations: node.allocations,
                allocated_bytes: node.allocated_bytes,
                live_bytes_high_water: node.live_high_water,
            })
            .collect();
        let sources = self
            .sources
            .iter()
            .map(|source| {
                let (live_bytes, live_bytes_high_water) = source.budget.as_ref().map_or(
                    (source.live_bytes, source.live_high_water),
                    |budget| {
                        (
                            u64::try_from(budget.reserved_bytes()).unwrap_or(u64::MAX),
                            u64::try_from(budget.peak_reserved_bytes()).unwrap_or(u64::MAX),
                        )
                    },
                );
                AllocationSource {
                    label: source.label.to_owned(),
                    kind: source.kind,
                    allocations: source.allocations,
                    allocated_bytes: source.allocated_bytes,
                    frees: source.frees,
                    freed_bytes: source.freed_bytes,
                    live_bytes,
                    live_bytes_high_water,
                    live_buffers: source.live_buffers,
                    live_buffers_high_water: source.live_buffers_high_water,
                }
            })
            .collect();
        let begin = self.begin_usage;
        let delta = |end: Option<u64>, start: Option<u64>| match (end, start) {
            (Some(end), Some(start)) => end.saturating_sub(start),
            _ => 0,
        };
        let limit = platform::address_space_limit();
        Some(MeasurementRecord {
            schema: RECORD_SCHEMA_V1.to_owned(),
            identity,
            outcome,
            failures: FailureLog {
                classification: Classification::Measured,
                entries: self
                    .failures
                    .iter()
                    .map(|failure| RawFailure {
                        phase: failure.node,
                        stage: failure.stage.to_owned(),
                        code: failure.code.to_owned(),
                    })
                    .collect(),
                dropped: self.dropped_failures,
            },
            phase_tree: PhaseTree {
                classification: Classification::Measured,
                nodes,
            },
            byte_counters: ByteCounters {
                classification: Classification::Measured,
                entries: self
                    .bytes
                    .iter()
                    .map(|entry| ByteCounter {
                        phase: entry.node,
                        kind: entry.kind,
                        label: entry.label.to_owned(),
                        count: entry.count,
                        total_bytes: entry.total,
                        min_bytes: entry.min,
                        max_bytes: entry.max,
                    })
                    .collect(),
            },
            work_counters: WorkCounters {
                classification: Classification::Measured,
                entries: self
                    .work
                    .iter()
                    .map(|entry| WorkCounter {
                        phase: entry.node,
                        label: entry.label.to_owned(),
                        count: entry.count,
                        total_units: entry.total,
                        min_units: entry.min,
                        max_units: entry.max,
                    })
                    .collect(),
            },
            allocations: AllocationObservation {
                classification: Classification::Measured,
                sources,
            },
            process: ProcessObservation {
                classification: Classification::Measured,
                pid: std::process::id(),
                logical_cpus: platform::logical_cpus(),
                cpu_user_ns: delta(
                    usage.map(|value| value.user_ns),
                    begin.map(|value| value.user_ns),
                ),
                cpu_system_ns: delta(
                    usage.map(|value| value.system_ns),
                    begin.map(|value| value.system_ns),
                ),
                peak_rss_bytes_at_begin: begin.map_or(0, |value| value.peak_rss_bytes),
                peak_rss_bytes: usage.map_or(0, |value| value.peak_rss_bytes),
                peak_rss_source: if usage.is_some() {
                    PeakRssSource::KernelLifetimeHighWater
                } else {
                    PeakRssSource::Unavailable
                },
                load_milli_begin: self.begin_load,
                load_milli_finish: load,
                thermal_begin: self.begin_thermal.unwrap_or(ThermalState::Unavailable),
                thermal_finish: thermal.0,
                thermal_source: thermal.1.to_owned(),
            },
            scheduling: Scheduling {
                classification: Classification::LocalScheduling,
                workers: self.workers,
                workers_provenance: self.workers_provenance.to_owned(),
                phase_workers: self.phase_workers.clone(),
            },
            address_space: AddressSpaceObservation {
                classification: Classification::LocalScheduling,
                source: if limit.is_some() {
                    platform::ADDRESS_SPACE_SOURCE.to_owned()
                } else {
                    "unavailable".to_owned()
                },
                soft_limit_bytes: limit.and_then(|value| value.soft_bytes),
                hard_limit_bytes: limit.and_then(|value| value.hard_bytes),
                enforced: limit.is_some_and(|value| value.soft_bytes.is_some()),
            },
            declared: self
                .declared
                .iter()
                .map(|entry| {
                    let (classification, provenance_kind) = entry.class.schema_pair();
                    DeclaredQuantity {
                        classification,
                        label: entry.label.to_owned(),
                        unit: entry.unit,
                        value: entry.value,
                        provenance_kind,
                        provenance: entry.provenance.to_owned(),
                    }
                })
                .collect(),
            recorder: self.health,
        })
    }

    /// Overwrite every owned element with its empty value, keeping lengths.
    fn scrub(&mut self) {
        self.nodes.fill(Node::default());
        self.spans.fill(Span::default());
        for source in &mut self.sources {
            *source = Source::empty();
        }
        self.bytes.fill(ByteEntry::EMPTY);
        self.work.fill(WorkEntry::EMPTY);
        self.declared.fill(Declared::EMPTY);
        self.failures.fill(Failure::default());
        self.phase_workers.fill(PhaseWorkers {
            phase: 0,
            workers: 0,
        });
        self.threads.clear();
        self.identity = None;
        self.dropped_failures = 0;
        self.workers = 0;
        self.workers_provenance = "";
        self.health = RecorderHealth::default();
        self.next_generation = 0;
        self.next_sequence = 0;
        self.last_wall = 0;
        self.last_process_cpu = 0;
        self.begin_usage = None;
        self.begin_load = 0;
        self.begin_thermal = None;
        self.root_slot = NONE;
        self.stage = Stage::Scrubbed;
    }

    /// Whether every owned element holds its empty value.
    #[cfg(test)]
    fn is_scrubbed(&self) -> bool {
        self.nodes.iter().all(|node| *node == Node::default())
            && self.spans.iter().all(|span| *span == Span::default())
            && self.sources.iter().all(Source::is_empty)
            && self.bytes.iter().all(|entry| *entry == ByteEntry::EMPTY)
            && self.work.iter().all(|entry| *entry == WorkEntry::EMPTY)
            && self.declared.iter().all(|entry| *entry == Declared::EMPTY)
            && self
                .failures
                .iter()
                .all(|failure| *failure == Failure::default())
            && self
                .phase_workers
                .iter()
                .all(|entry| entry.phase == 0 && entry.workers == 0)
            && self.threads.is_empty()
            && self.identity.is_none()
            && self.dropped_failures == 0
            && self.workers == 0
            && self.workers_provenance.is_empty()
            && self.health == RecorderHealth::default()
            && self.begin_usage.is_none()
            && self.begin_thermal.is_none()
            && self.last_wall == 0
            && self.last_process_cpu == 0
    }

    /// Empty every buffer and return its allocation.
    fn release(&mut self) {
        self.nodes = Vec::new();
        self.spans = Vec::new();
        self.sources = Vec::new();
        self.bytes = Vec::new();
        self.work = Vec::new();
        self.declared = Vec::new();
        self.failures = Vec::new();
        self.phase_workers = Vec::new();
        self.threads = Vec::new();
        self.stage = Stage::Released;
    }

    /// Whether every buffer is empty and holds no allocation.
    #[cfg(test)]
    fn is_released(&self) -> bool {
        self.stage == Stage::Released
            && self.sink.is_none()
            && self.nodes.capacity() == 0
            && self.spans.capacity() == 0
            && self.sources.capacity() == 0
            && self.bytes.capacity() == 0
            && self.work.capacity() == 0
            && self.declared.capacity() == 0
            && self.failures.capacity() == 0
            && self.phase_workers.capacity() == 0
            && self.threads.capacity() == 0
    }
}

fn identity_or_invalid(text: &'static str) -> (&'static str, bool) {
    if text::is_identity_text(text) {
        (text, true)
    } else {
        (text::INVALID_LABEL, false)
    }
}

struct Shared {
    state: Mutex<State>,
    clock: Box<dyn Clock>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn enter(
        self: &Arc<Self>,
        label: &'static str,
        explicit_parent: Option<(u32, u64)>,
    ) -> PhaseGuard {
        let boundary = Boundary::sample(&*self.clock);
        let opened = self
            .lock()
            .open_span(&*self.clock, label, explicit_parent, boundary);
        PhaseGuard {
            open: opened.map(|(slot, generation)| OpenPhase {
                shared: Arc::clone(self),
                slot,
                generation,
            }),
            completed: false,
            thread: PhantomData,
        }
    }

    /// End the session: close the root, deliver the record, clear the buffers.
    fn end(&self, finished: bool) {
        let boundary = Boundary::sample(&*self.clock);
        let unwinding = std::thread::panicking();
        let mut state = self.lock();
        if state.stage != Stage::Live {
            return;
        }
        let (wall, process_cpu) = state.sample(&*self.clock);
        let root = state.root_slot;
        state.close_span(
            root,
            wall,
            process_cpu,
            boundary,
            finished,
            unwinding && !finished,
            false,
        );
        // The root is closed: the slower process probes below are outside
        // the measured window and cannot appear as unattributed time.
        let usage = platform::resource_usage();
        let thermal = platform::thermal_state();
        let outcome = if !finished && unwinding {
            RunOutcome::Unwound
        } else if !finished {
            RunOutcome::Abandoned
        } else if state.failures.is_empty() && state.dropped_failures == 0 {
            RunOutcome::Succeeded
        } else {
            RunOutcome::Failed
        };
        let record = state.build_record(outcome, usage, boundary.load, thermal);
        let sink = state.sink.take();
        state.scrub();
        state.release();
        drop(state);
        if let (Some(mut sink), Some(record)) = (sink, record) {
            sink.accept(record);
        }
    }
}

struct OpenPhase {
    shared: Arc<Shared>,
    slot: u32,
    generation: u64,
}

/// One open call of a phase on the thread that opened it.
///
/// Call [`PhaseGuard::complete`] when the phase finished its work. Dropping
/// the guard without completing it records an interrupted call, and an
/// unwound one when the thread is panicking.
///
/// The guard cannot leave its thread, because the phase's CPU time is read
/// from the opening thread's clock:
/// ```compile_fail,E0277
/// use iroha_measurement::{
///     CollectingSink, FlowKind, RunContext, RunIdentity, Session, WorkerDeclaration,
/// };
/// let session = Session::begin(
///     RunIdentity::new(RunContext::unbound(), "doc", FlowKind::Native, "doc"),
///     "root",
///     WorkerDeclaration { workers: 1, provenance: "doc" },
///     Box::new(CollectingSink::new()),
/// );
/// let guard = session.enter("phase");
/// std::thread::spawn(move || drop(guard));
/// ```
pub struct PhaseGuard {
    open: Option<OpenPhase>,
    completed: bool,
    thread: PhantomData<*const ()>,
}

impl PhaseGuard {
    /// Record that this call of the phase finished its work.
    pub fn complete(mut self) {
        self.completed = true;
    }

    /// A token under which other threads can open child phases of this call.
    pub fn parent(&self) -> PhaseParent {
        PhaseParent {
            open: self.open.as_ref().map(|open| OpenPhase {
                shared: Arc::clone(&open.shared),
                slot: open.slot,
                generation: open.generation,
            }),
        }
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        let boundary = Boundary::sample(&*open.shared.clock);
        let unwinding = std::thread::panicking();
        let mut state = open.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        if !state.span_is(open.slot, open.generation) {
            state.health.stale_guard_events += 1;
            return;
        }
        let (wall, process_cpu) = state.sample(&*open.shared.clock);
        state.close_span(
            open.slot,
            wall,
            process_cpu,
            boundary,
            self.completed,
            unwinding && !self.completed,
            false,
        );
    }
}

/// Sendable reference to one open phase call, for worker threads.
///
/// A child opened through a token whose phase has already ended is attached
/// to the root and counted in the recorder health section.
pub struct PhaseParent {
    open: Option<OpenPhase>,
}

impl PhaseParent {
    /// Open a child phase of the referenced call on the calling thread.
    pub fn enter(&self, label: &'static str) -> PhaseGuard {
        self.open.as_ref().map_or(
            PhaseGuard {
                open: None,
                completed: false,
                thread: PhantomData,
            },
            |open| open.shared.enter(label, Some((open.slot, open.generation))),
        )
    }
}

impl Clone for PhaseParent {
    fn clone(&self) -> Self {
        Self {
            open: self.open.as_ref().map(|open| OpenPhase {
                shared: Arc::clone(&open.shared),
                slot: open.slot,
                generation: open.generation,
            }),
        }
    }
}

/// Scoped allocation counter for buffers that no allocation budget accounts.
///
/// The instrumented code reports each buffer it allocates and releases. This
/// is explicit accounting: the global allocator is never replaced, so
/// production allocation behaviour is unchanged.
pub struct AllocationCounter {
    shared: Arc<Shared>,
    source: u32,
}

impl AllocationCounter {
    /// Report one allocation of `bytes`.
    pub fn allocated(&self, bytes: u64) {
        let thread = std::thread::current().id();
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        let Some(source) = state.sources.get_mut(index(self.source)) else {
            return;
        };
        source.allocations += 1;
        source.allocated_bytes = source.allocated_bytes.saturating_add(bytes);
        source.live_bytes = source.live_bytes.saturating_add(bytes);
        source.live_high_water = source.live_high_water.max(source.live_bytes);
        source.live_buffers += 1;
        source.live_buffers_high_water = source.live_buffers_high_water.max(source.live_buffers);
        let live = state.live_total();
        let slot = state.attribution_slot(thread);
        let Some(span) = state.spans.get_mut(index(slot)) else {
            return;
        };
        span.live_high_water = span.live_high_water.max(live);
        let node = span.node;
        if let Some(node) = state.nodes.get_mut(index(node)) {
            node.allocations += 1;
            node.allocated_bytes = node.allocated_bytes.saturating_add(bytes);
        }
    }

    /// Report the release of one buffer of `bytes`.
    pub fn freed(&self, bytes: u64) {
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        if let Some(source) = state.sources.get_mut(index(self.source)) {
            source.frees += 1;
            source.freed_bytes = source.freed_bytes.saturating_add(bytes);
            source.live_bytes = source.live_bytes.saturating_sub(bytes);
            source.live_buffers = source.live_buffers.saturating_sub(1);
        }
    }

    /// Report one live buffer of `bytes` that is released when the guard drops.
    pub fn buffer(&self, bytes: u64) -> LiveBuffer {
        self.allocated(bytes);
        LiveBuffer {
            counter: Self {
                shared: Arc::clone(&self.shared),
                source: self.source,
            },
            bytes,
        }
    }
}

/// A live buffer reported to an [`AllocationCounter`]; dropping it reports the release.
pub struct LiveBuffer {
    counter: AllocationCounter,
    bytes: u64,
}

impl Drop for LiveBuffer {
    fn drop(&mut self) {
        self.counter.freed(self.bytes);
    }
}

/// Sendable recording interface of one session.
///
/// Every method returns `()` or an opaque guard. Nothing recorded can be read
/// back through this type.
#[derive(Clone)]
pub struct SessionHandle {
    shared: Arc<Shared>,
}

impl SessionHandle {
    /// Open a phase under the innermost phase open on the calling thread.
    ///
    /// A thread with no open phase attaches to the root. The label must be a
    /// literal: text built at run time cannot be passed without deliberately
    /// leaking it.
    /// ```compile_fail,E0597
    /// use iroha_measurement::{
    ///     CollectingSink, FlowKind, RunContext, RunIdentity, Session, WorkerDeclaration,
    /// };
    /// let session = Session::begin(
    ///     RunIdentity::new(RunContext::unbound(), "doc", FlowKind::Native, "doc"),
    ///     "root",
    ///     WorkerDeclaration { workers: 1, provenance: "doc" },
    ///     Box::new(CollectingSink::new()),
    /// );
    /// let witness_derived = String::from("secret");
    /// let _guard = session.enter(witness_derived.as_str());
    /// drop(witness_derived);
    /// ```
    pub fn enter(&self, label: &'static str) -> PhaseGuard {
        self.shared.enter(label, None)
    }

    /// Report the size of a public encoded object inside the current phase.
    ///
    /// Only the size is accepted; the bytes themselves cannot be passed.
    /// ```compile_fail,E0308
    /// use iroha_measurement::{
    ///     ByteKind, CollectingSink, FlowKind, RunContext, RunIdentity, Session,
    ///     WorkerDeclaration,
    /// };
    /// let session = Session::begin(
    ///     RunIdentity::new(RunContext::unbound(), "doc", FlowKind::Native, "doc"),
    ///     "root",
    ///     WorkerDeclaration { workers: 1, provenance: "doc" },
    ///     Box::new(CollectingSink::new()),
    /// );
    /// let key = [0_u8; 32];
    /// session.record_bytes(ByteKind::Key, "secret_key", &key[..]);
    /// ```
    pub fn record_bytes(&self, kind: ByteKind, label: &'static str, bytes: u64) {
        let thread = std::thread::current().id();
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        let (label, valid) = text::public_or_invalid(label);
        if !valid {
            state.health.invalid_label_events += 1;
        }
        let slot = state.attribution_slot(thread);
        let Some(node) = state.spans.get(index(slot)).map(|span| span.node) else {
            return;
        };
        if let Some(entry) = state
            .bytes
            .iter_mut()
            .find(|entry| entry.node == node && entry.kind == kind && entry.label == label)
        {
            entry.count += 1;
            entry.total = entry.total.saturating_add(bytes);
            entry.min = entry.min.min(bytes);
            entry.max = entry.max.max(bytes);
        } else if state.bytes.len() < MAX_BYTE_COUNTERS {
            state.bytes.push(ByteEntry {
                node,
                kind,
                label,
                count: 1,
                total: bytes,
                min: bytes,
                max: bytes,
            });
        } else {
            state.health.counter_overflow_events += 1;
        }
    }

    /// Report a count of completed public work inside the current phase:
    /// transform calls, columns, rows, constraints.
    ///
    /// Only an unsigned count is accepted. The count must be public geometry
    /// of the statement or the profile, never a witness-dependent position
    /// or value.
    pub fn record_work(&self, label: &'static str, units: u64) {
        let thread = std::thread::current().id();
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        let (label, valid) = text::public_or_invalid(label);
        if !valid {
            state.health.invalid_label_events += 1;
        }
        let slot = state.attribution_slot(thread);
        let Some(node) = state.spans.get(index(slot)).map(|span| span.node) else {
            return;
        };
        if let Some(entry) = state
            .work
            .iter_mut()
            .find(|entry| entry.node == node && entry.label == label)
        {
            entry.count += 1;
            entry.total = entry.total.saturating_add(units);
            entry.min = entry.min.min(units);
            entry.max = entry.max.max(units);
        } else if state.work.len() < MAX_WORK_COUNTERS {
            state.work.push(WorkEntry {
                node,
                label,
                count: 1,
                total: units,
                min: units,
                max: units,
            });
        } else {
            state.health.counter_overflow_events += 1;
        }
    }

    /// Retain one raw failure. A recorded failure is never dropped: beyond
    /// the retention bound it is counted, and that count rejects the report.
    ///
    /// Both arguments are literals; an error's formatted text, which could
    /// contain private data, cannot be passed.
    pub fn record_failure(&self, stage: &'static str, code: &'static str) {
        let thread = std::thread::current().id();
        let mut recorder = self.shared.lock();
        if recorder.stage != Stage::Live {
            return;
        }
        let (stage, stage_valid) = text::public_or_invalid(stage);
        let (code, code_valid) = text::public_or_invalid(code);
        recorder.health.invalid_label_events += u64::from(!stage_valid) + u64::from(!code_valid);
        let slot = recorder.attribution_slot(thread);
        let node = recorder.spans.get(index(slot)).map_or(0, |span| span.node);
        if recorder.failures.len() < MAX_FAILURES {
            recorder.failures.push(Failure { node, stage, code });
        } else {
            recorder.dropped_failures += 1;
        }
    }

    /// Record a value that was declared rather than measured.
    pub fn declare(&self, declaration: Declaration) {
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        let (label, label_valid) = text::public_or_invalid(declaration.label);
        let (provenance, provenance_valid) = identity_or_invalid(declaration.provenance);
        state.health.invalid_label_events += u64::from(!label_valid) + u64::from(!provenance_valid);
        if state.declared.len() < MAX_DECLARED {
            state.declared.push(Declared {
                class: declaration.class,
                label,
                unit: declaration.unit,
                value: declaration.value,
                provenance,
            });
        } else {
            state.health.counter_overflow_events += 1;
        }
    }

    /// Declare the worker count scheduled for the innermost open phase.
    pub fn declare_phase_workers(&self, workers: u32) {
        let thread = std::thread::current().id();
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return;
        }
        let slot = state.attribution_slot(thread);
        let Some(phase) = state.spans.get(index(slot)).map(|span| span.node) else {
            return;
        };
        if let Some(entry) = state
            .phase_workers
            .iter_mut()
            .find(|entry| entry.phase == phase)
        {
            entry.workers = workers;
        } else if state.phase_workers.len() < MAX_PHASE_WORKERS {
            state.phase_workers.push(PhaseWorkers { phase, workers });
        } else {
            state.health.counter_overflow_events += 1;
        }
    }

    fn add_source(
        &self,
        label: &'static str,
        kind: AllocationSourceKind,
        budget: Option<AllocationBudget>,
    ) -> Option<u32> {
        let mut state = self.shared.lock();
        if state.stage != Stage::Live {
            return None;
        }
        let (label, valid) = text::public_or_invalid(label);
        if !valid {
            state.health.invalid_label_events += 1;
        }
        if state.sources.len() >= MAX_ALLOCATION_SOURCES {
            state.health.counter_overflow_events += 1;
            return None;
        }
        if let Some(budget) = &budget {
            if state.declared.len() < MAX_DECLARED {
                state.declared.push(Declared {
                    class: DeclaredClass::LocalConfiguration,
                    label,
                    unit: Unit::Bytes,
                    value: u64::try_from(budget.limit_bytes()).unwrap_or(u64::MAX),
                    provenance: BUDGET_LIMIT_PROVENANCE,
                });
            } else {
                state.health.counter_overflow_events += 1;
            }
        }
        state.sources.push(Source {
            label,
            kind,
            budget,
            ..Source::empty()
        });
        Some(narrow(state.sources.len() - 1))
    }

    /// Create a scoped allocation counter with a static public label.
    pub fn allocation_counter(&self, label: &'static str) -> AllocationCounter {
        AllocationCounter {
            shared: Arc::clone(&self.shared),
            source: self
                .add_source(label, AllocationSourceKind::ScopedCounter, None)
                .unwrap_or(NONE),
        }
    }

    /// Observe an existing allocation budget at every phase boundary.
    ///
    /// The session keeps a clone of the budget handle until it ends, reads its
    /// reserved and peak bytes, and declares its limit as a local
    /// configuration value. It never reserves from the budget.
    pub fn observe_allocation_budget(&self, label: &'static str, budget: &AllocationBudget) {
        let _ = self.add_source(
            label,
            AllocationSourceKind::AllocationBudget,
            Some(budget.clone()),
        );
    }
}

/// Owner of one measured root flow.
///
/// The session belongs to the thread that began it, because the root phase's
/// CPU time is read from that thread's clock. Use [`Session::handle`] and
/// [`PhaseGuard::parent`] to record from worker threads.
pub struct Session {
    handle: SessionHandle,
    // Atomic rather than `Cell` so a session, and a diagnostic receipt that
    // owns one, can be observed across `catch_unwind`.
    ended: AtomicBool,
    thread: PhantomData<*const ()>,
}

impl Session {
    /// Start measuring a root flow; the root phase opens now.
    ///
    /// The identity is fixed here, before the workload runs. Identity text
    /// outside its grammar is replaced by the fixed invalid label and counted,
    /// so the record reports it as incomplete instead of carrying it. After
    /// this call the recorder accepts only literals and integers.
    pub fn begin(
        identity: RunIdentity,
        root: &'static str,
        workers: WorkerDeclaration,
        sink: Box<dyn RecordSink>,
    ) -> Self {
        Self::begin_with_clock(
            identity,
            root,
            workers,
            sink,
            Box::new(SystemClock {
                origin: Instant::now(),
            }),
        )
    }

    fn begin_with_clock(
        identity: RunIdentity,
        root: &'static str,
        workers: WorkerDeclaration,
        sink: Box<dyn RecordSink>,
        clock: Box<dyn Clock>,
    ) -> Self {
        let state = State::new(identity, workers, sink, &*clock);
        let shared = Arc::new(Shared {
            state: Mutex::new(state),
            clock,
        });
        let boundary = Boundary::sample(&*shared.clock);
        {
            let mut state = shared.lock();
            let opened = state.open_span(&*shared.clock, root, None, boundary);
            state.root_slot = opened.map_or(NONE, |(slot, _)| slot);
        }
        Self {
            handle: SessionHandle { shared },
            ended: AtomicBool::new(false),
            thread: PhantomData,
        }
    }

    /// A sendable recording interface for worker threads.
    pub fn handle(&self) -> SessionHandle {
        self.handle.clone()
    }

    /// End the root flow and deliver the record to the sink.
    ///
    /// The outcome is `succeeded` when no failure was recorded, otherwise
    /// `failed`. Phases still open are closed as interrupted and truncated.
    pub fn finish(self) {
        self.ended.store(true, Ordering::Relaxed);
        self.handle.shared.end(true);
    }
}

impl core::ops::Deref for Session {
    type Target = SessionHandle;

    fn deref(&self) -> &SessionHandle {
        &self.handle
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.ended.swap(true, Ordering::Relaxed) {
            // An error return or an unwind: the run is still recorded.
            self.handle.shared.end(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        report::Finding,
        schema::FlowKind,
        sink::CollectingSink,
        test_support::{bound_context, spin_for},
    };

    const WORKERS: WorkerDeclaration = WorkerDeclaration {
        workers: 4,
        provenance: "test.fixed_workers",
    };

    fn begin(sink: &CollectingSink) -> Session {
        Session::begin(
            RunIdentity::new(bound_context(), "unit", FlowKind::Proof, "rust.test"),
            "prove",
            WORKERS,
            Box::new(sink.clone()),
        )
    }

    fn only(sink: &CollectingSink) -> MeasurementRecord {
        let mut records = sink.take();
        assert_eq!(records.len(), 1);
        records.remove(0)
    }

    fn node<'a>(record: &'a MeasurementRecord, label: &str) -> (u32, &'a PhaseNode) {
        let mut found = record
            .phase_tree
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.label == label);
        let (position, node) = found.next().unwrap_or_else(|| panic!("no phase {label}"));
        assert!(found.next().is_none(), "phase {label} is not unique");
        (narrow(position), node)
    }

    fn children_wall(record: &MeasurementRecord, parent: u32) -> u64 {
        record
            .phase_tree
            .nodes
            .iter()
            .filter(|node| node.parent == Some(parent))
            .map(|node| node.wall_inclusive_ns)
            .sum()
    }

    fn assert_no_findings_except_attribution(record: &MeasurementRecord) {
        let findings: Vec<_> = record
            .findings()
            .into_iter()
            .filter(|finding| !matches!(finding, Finding::UnattributedExceedsLimit { .. }))
            .collect();
        assert!(findings.is_empty(), "{findings:?}");
    }

    /// A clock the test advances by hand: wall time, the process CPU clock,
    /// per-thread CPU and the system load are exactly what the test set.
    #[derive(Clone, Default)]
    struct ManualClock {
        time: Arc<ManualTime>,
    }

    #[derive(Default)]
    struct ManualTime {
        wall: std::sync::atomic::AtomicU64,
        process_cpu: std::sync::atomic::AtomicU64,
        load: std::sync::atomic::AtomicU64,
        thread_cpu: Mutex<Vec<(ThreadId, u64)>>,
    }

    impl ManualClock {
        /// Advance wall time and the process CPU clock.
        fn advance(&self, wall_ns: u64, process_cpu_ns: u64) {
            self.time.wall.fetch_add(wall_ns, Ordering::SeqCst);
            self.time
                .process_cpu
                .fetch_add(process_cpu_ns, Ordering::SeqCst);
        }

        /// Charge CPU time to the calling thread.
        fn burn(&self, cpu_ns: u64) {
            let thread = std::thread::current().id();
            let mut threads = self.time.thread_cpu.lock().unwrap();
            match threads.iter_mut().find(|(owner, _)| *owner == thread) {
                Some((_, total)) => *total += cpu_ns,
                None => threads.push((thread, cpu_ns)),
            }
        }

        fn set_load(&self, load_milli: u64) {
            self.time.load.store(load_milli, Ordering::SeqCst);
        }
    }

    impl Clock for ManualClock {
        fn wall_ns(&self) -> u64 {
            self.time.wall.load(Ordering::SeqCst)
        }

        fn process_cpu_ns(&self) -> Option<u64> {
            Some(self.time.process_cpu.load(Ordering::SeqCst))
        }

        fn thread_cpu_ns(&self) -> Option<u64> {
            let thread = std::thread::current().id();
            let threads = self.time.thread_cpu.lock().unwrap();
            Some(
                threads
                    .iter()
                    .find(|(owner, _)| *owner == thread)
                    .map_or(0, |(_, total)| *total),
            )
        }

        fn load_milli(&self) -> u64 {
            self.time.load.load(Ordering::SeqCst)
        }
    }

    fn begin_manual(sink: &CollectingSink, clock: &ManualClock) -> Session {
        Session::begin_with_clock(
            RunIdentity::new(bound_context(), "unit", FlowKind::Proof, "rust.test"),
            "prove",
            WORKERS,
            Box::new(sink.clone()),
            Box::new(clock.clone()),
        )
    }

    #[test]
    fn a_root_covered_by_its_phases_passes_the_one_percent_rule() {
        // Two phases cover 99 ms. With exactly 1 ms of a 100 ms root outside
        // them the record is accepted; one nanosecond more is rejected. The
        // injected clock makes the boundary exact on any host.
        for (outside_ns, accepted) in [(1_000_000_u64, true), (1_000_001, false)] {
            let (sink, clock) = (CollectingSink::new(), ManualClock::default());
            let session = begin_manual(&sink, &clock);
            clock.advance(outside_ns / 2, 0);
            let prepare = session.enter("prepare");
            clock.advance(20_000_000, 0);
            prepare.complete();
            let prove = session.enter("commit_and_open");
            clock.advance(79_000_000, 0);
            prove.complete();
            clock.advance(outside_ns - outside_ns / 2, 0);
            session.finish();
            let record = only(&sink);
            let attribution = record.attribution().unwrap();
            assert_eq!(attribution.root_wall_ns, 99_000_000 + outside_ns);
            assert_eq!(attribution.unattributed_wall_ns, outside_ns);
            assert_eq!(attribution.within_limit(), accepted);
            let expected = if accepted {
                Vec::new()
            } else {
                vec![Finding::UnattributedExceedsLimit {
                    root_wall_ns: 100_000_001,
                    unattributed_wall_ns: 1_000_001,
                }]
            };
            assert_eq!(record.findings(), expected);
            let largest = record.largest_undivided_phase().unwrap();
            assert_eq!((largest.phase, largest.wall_exclusive_ns), (2, 79_000_000));
        }
    }

    enum Step {
        Enter,
        Burn(u64),
        Exit,
    }

    /// A worker thread that performs one step at a time on request, so the
    /// order of events across threads is fixed by the test.
    fn stepped_worker(
        parent: PhaseParent,
        clock: ManualClock,
    ) -> (impl Fn(Step), std::thread::JoinHandle<()>) {
        let (send_step, steps) = std::sync::mpsc::channel::<Step>();
        let (send_done, done) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let mut guard = None;
            for step in steps {
                match step {
                    Step::Enter => guard = Some(parent.enter("worker")),
                    Step::Burn(cpu_ns) => clock.burn(cpu_ns),
                    Step::Exit => {
                        if let Some(open) = guard.take() {
                            open.complete();
                        }
                    }
                }
                send_done.send(()).unwrap();
            }
        });
        let step = move |step: Step| {
            send_step.send(step).unwrap();
            done.recv().unwrap();
        };
        (step, thread)
    }

    #[test]
    fn overlapping_parallel_children_are_charged_their_exact_union() {
        let (sink, clock) = (CollectingSink::new(), ManualClock::default());
        clock.set_load(1_500);
        let session = begin_manual(&sink, &clock);
        clock.advance(5, 5);
        let fold = session.enter("fold"); // wall 5, process CPU 5
        let (first, first_thread) = stepped_worker(fold.parent(), clock.clone());
        let (second, second_thread) = stepped_worker(fold.parent(), clock.clone());
        clock.advance(5, 5);
        first(Step::Enter); // wall 10, process CPU 10
        clock.advance(10, 20);
        clock.set_load(2_500);
        second(Step::Enter); // wall 20, process CPU 30
        first(Step::Burn(30));
        clock.advance(30, 60);
        first(Step::Exit); // wall 50, process CPU 90
        second(Step::Burn(45));
        clock.advance(20, 20);
        second(Step::Exit); // wall 70, process CPU 110
        clock.advance(10, 10);
        clock.burn(7);
        clock.set_load(2_000);
        fold.complete(); // wall 80, process CPU 120
        clock.advance(20, 0);
        session.finish(); // wall 100
        drop((first, second));
        first_thread.join().unwrap();
        second_thread.join().unwrap();

        let record = only(&sink);
        let (_, root) = node(&record, "prove");
        let (fold_index, fold) = node(&record, "fold");
        let (_, worker) = node(&record, "worker");
        assert_eq!(worker.parent, Some(fold_index));
        // Two calls of 40 ns and 50 ns overlap for 30 ns.
        assert_eq!((worker.calls, worker.completed), (2, 2));
        assert_eq!(
            (worker.wall_inclusive_ns, worker.wall_exclusive_ns),
            (90, 90)
        );
        assert_eq!(
            (
                worker.thread_cpu_inclusive_ns,
                worker.thread_cpu_exclusive_ns
            ),
            (75, 75)
        );
        // Each call's window on the process clock: 90 - 10 and 110 - 30.
        assert_eq!(worker.process_cpu_window_inclusive_ns, 160);
        assert_eq!(worker.active_threads_max, 3);
        // The parent is charged the union 10..70 once, never the 90 ns sum.
        assert_eq!(fold.peak_concurrent_children, 2);
        assert_eq!((fold.wall_inclusive_ns, fold.wall_exclusive_ns), (75, 15));
        assert_eq!(
            (
                fold.process_cpu_window_inclusive_ns,
                fold.process_cpu_window_exclusive_ns
            ),
            (115, 15)
        );
        // Children on other threads are not subtracted from the opener's CPU.
        assert_eq!(
            (fold.thread_cpu_inclusive_ns, fold.thread_cpu_exclusive_ns),
            (7, 7)
        );
        assert_eq!(
            (
                fold.load_milli_first_enter,
                fold.load_milli_last_exit,
                fold.load_milli_max
            ),
            (1_500, 2_000, 2_000)
        );
        assert_eq!(worker.load_milli_max, 2_500);
        assert_eq!((root.wall_inclusive_ns, root.wall_exclusive_ns), (100, 25));
        assert_eq!(
            (root.thread_cpu_inclusive_ns, root.thread_cpu_exclusive_ns),
            (7, 0)
        );
        assert_eq!(
            record.findings(),
            vec![Finding::UnattributedExceedsLimit {
                root_wall_ns: 100,
                unattributed_wall_ns: 25,
            }]
        );
    }

    #[test]
    fn work_counters_aggregate_per_phase_and_label() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.record_work("statement_rows", 8);
        let phase = session.enter("transform");
        for columns in [8, 4, 8] {
            session.record_work("columns", columns);
        }
        session.record_work("butterflies", 0);
        phase.complete();
        session.finish();
        let record = only(&sink);
        let (transform, _) = node(&record, "transform");
        assert_eq!(
            record.work_counters,
            WorkCounters {
                classification: Classification::Measured,
                entries: vec![
                    WorkCounter {
                        phase: 0,
                        label: "statement_rows".into(),
                        count: 1,
                        total_units: 8,
                        min_units: 8,
                        max_units: 8,
                    },
                    WorkCounter {
                        phase: transform,
                        label: "columns".into(),
                        count: 3,
                        total_units: 20,
                        min_units: 4,
                        max_units: 8,
                    },
                    WorkCounter {
                        phase: transform,
                        label: "butterflies".into(),
                        count: 1,
                        total_units: 0,
                        min_units: 0,
                        max_units: 0,
                    },
                ],
            }
        );
        assert_no_findings_except_attribution(&record);
    }

    #[test]
    fn identity_text_outside_its_grammar_is_replaced_before_it_is_recorded() {
        let mut identity = RunIdentity::new(bound_context(), "unit", FlowKind::Proof, "rust.test");
        // The fields are public, so a caller can bypass `RunIdentity::new`.
        identity.workload = "secret key 0xdeadbeef".into();
        identity.context.hardware = "two words".into();
        identity.context.profile = "p".repeat(text::MAX_IDENTITY_BYTES + 1);
        identity.context.source_commit = "not a commit".into();
        identity.context.source_dirty_digest = Some("witness bytes".into());
        let (cleaned, replaced) = public_identity(identity.clone());
        assert_eq!(replaced, 5);
        assert_eq!(cleaned.emitter, "rust.test");
        assert_eq!(cleaned.context.artifact, bound_context().artifact);

        let sink = CollectingSink::new();
        let session = Session::begin(identity, "prove", WORKERS, Box::new(sink.clone()));
        session.enter("phase").complete();
        session.finish();
        let record = only(&sink);
        assert_eq!(record.identity, cleaned);
        assert_eq!(record.identity.workload, text::INVALID_LABEL);
        assert_eq!(record.identity.context.hardware, text::INVALID_LABEL);
        assert_eq!(record.identity.context.profile, text::INVALID_LABEL);
        assert_eq!(record.identity.context.source_commit, text::INVALID_LABEL);
        assert_eq!(
            record.identity.context.source_dirty_digest.as_deref(),
            Some(text::INVALID_LABEL)
        );
        let view = record.to_json_view();
        for rejected in ["secret", "two words", "not a commit", "witness"] {
            assert!(!view.contains(rejected), "{rejected}");
        }
        assert_eq!(record.recorder.invalid_label_events, 5);
        let findings = record.findings();
        for field in ["workload", "source_commit", "profile", "hardware"] {
            assert!(findings.contains(&Finding::IdentityIncomplete { field }));
        }
        assert!(findings.contains(&Finding::DirtyDigestMismatch));
        assert!(findings.contains(&Finding::LabelNotPublic {
            section: "recorder",
            index: 0
        }));
        // A complete identity and the unbound sentinel pass through unchanged.
        let bound = RunIdentity::new(bound_context(), "unit", FlowKind::Proof, "rust.test");
        assert_eq!(public_identity(bound.clone()), (bound, 0));
        let unbound = RunIdentity::new(
            crate::schema::RunContext::unbound(),
            "unit",
            FlowKind::Proof,
            "rust.test",
        );
        assert_eq!(public_identity(unbound.clone()), (unbound, 0));
    }

    #[test]
    fn nested_sequential_phases_have_exact_inclusive_and_exclusive_time() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let commit = session.enter("commit");
        spin_for(Duration::from_millis(2));
        for _ in 0..3 {
            let column = session.enter("column");
            spin_for(Duration::from_millis(1));
            column.complete();
        }
        commit.complete();
        let fri = session.enter("fri");
        spin_for(Duration::from_millis(2));
        fri.complete();
        session.finish();

        let record = only(&sink);
        assert_eq!(record.outcome, RunOutcome::Succeeded);
        assert_eq!(record.phase_tree.nodes.len(), 4);
        let (root_index, root) = node(&record, "prove");
        let (commit_index, commit) = node(&record, "commit");
        let (_, column) = node(&record, "column");
        let (_, fri) = node(&record, "fri");
        assert_eq!((root_index, root.parent), (0, None));
        assert_eq!(commit.parent, Some(0));
        assert_eq!(column.parent, Some(commit_index));
        assert_eq!(fri.parent, Some(0));
        assert_eq!(
            (root.calls, commit.calls, column.calls, fri.calls),
            (1, 1, 3, 1)
        );
        assert_eq!(column.completed, 3);
        // Sequential children: exclusive is exactly inclusive minus the sum of
        // the children's inclusive time, in integer nanoseconds.
        assert_eq!(
            root.wall_exclusive_ns,
            root.wall_inclusive_ns - commit.wall_inclusive_ns - fri.wall_inclusive_ns
        );
        assert_eq!(
            commit.wall_exclusive_ns,
            commit.wall_inclusive_ns - column.wall_inclusive_ns
        );
        assert_eq!(column.wall_exclusive_ns, column.wall_inclusive_ns);
        assert_eq!(fri.wall_exclusive_ns, fri.wall_inclusive_ns);
        assert_eq!(
            children_wall(&record, 0),
            commit.wall_inclusive_ns + fri.wall_inclusive_ns
        );
        assert!(column.wall_inclusive_ns >= 3_000_000);
        assert!(commit.wall_exclusive_ns >= 2_000_000);
        assert_eq!(
            (
                root.peak_concurrent_children,
                commit.peak_concurrent_children
            ),
            (1, 1)
        );
        assert_eq!(column.peak_concurrent_children, 0);
        // Opening-thread CPU follows the same exact subtraction on one thread.
        assert_eq!(
            commit.thread_cpu_exclusive_ns,
            commit.thread_cpu_inclusive_ns - column.thread_cpu_inclusive_ns
        );
        assert_eq!(
            root.thread_cpu_exclusive_ns,
            root.thread_cpu_inclusive_ns
                - commit.thread_cpu_inclusive_ns
                - fri.thread_cpu_inclusive_ns
        );
        assert!(column.thread_cpu_inclusive_ns >= 2_000_000);
        assert_eq!(
            root.process_cpu_window_exclusive_ns,
            root.process_cpu_window_inclusive_ns
                - commit.process_cpu_window_inclusive_ns
                - fri.process_cpu_window_inclusive_ns
        );
        assert_eq!(root.active_threads_max, 1);
        assert_no_findings_except_attribution(&record);
        let attribution = record.attribution().unwrap();
        assert_eq!(attribution.root_wall_ns, root.wall_inclusive_ns);
        assert_eq!(attribution.unattributed_wall_ns, root.wall_exclusive_ns);
        // Whether the gaps between these phases stay under one percent of a
        // seven-millisecond root depends on the scheduler, so it is not
        // asserted with the real clock. The same tree is checked to the
        // nanosecond below with an injected clock.
    }

    #[test]
    fn nested_sequential_phases_are_exact_to_the_nanosecond_with_an_injected_clock() {
        let (sink, clock) = (CollectingSink::new(), ManualClock::default());
        let session = begin_manual(&sink, &clock);
        let commit = session.enter("commit");
        clock.advance(2_000_000, 2_100_000);
        clock.burn(1_900_000);
        for _ in 0..3 {
            let column = session.enter("column");
            clock.advance(1_000_000, 1_000_000);
            clock.burn(900_000);
            column.complete();
        }
        commit.complete();
        let fri = session.enter("fri");
        clock.advance(2_000_000, 2_000_000);
        clock.burn(2_000_000);
        fri.complete();
        session.finish();

        let record = only(&sink);
        let (_, root) = node(&record, "prove");
        let (_, commit) = node(&record, "commit");
        let (_, column) = node(&record, "column");
        let (_, fri) = node(&record, "fri");
        let times = |node: &PhaseNode| {
            [
                node.wall_inclusive_ns,
                node.wall_exclusive_ns,
                node.thread_cpu_inclusive_ns,
                node.thread_cpu_exclusive_ns,
                node.process_cpu_window_inclusive_ns,
                node.process_cpu_window_exclusive_ns,
            ]
        };
        assert_eq!(
            times(column),
            [
                3_000_000, 3_000_000, 2_700_000, 2_700_000, 3_000_000, 3_000_000
            ]
        );
        assert_eq!(
            times(commit),
            [
                5_000_000, 2_000_000, 4_600_000, 1_900_000, 5_100_000, 2_100_000
            ]
        );
        assert_eq!(
            times(fri),
            [
                2_000_000, 2_000_000, 2_000_000, 2_000_000, 2_000_000, 2_000_000
            ]
        );
        // The phases follow each other without a gap: nothing is unattributed.
        assert_eq!(times(root), [7_000_000, 0, 6_600_000, 0, 7_100_000, 0]);
        let attribution = record.attribution().unwrap();
        assert_eq!(
            (attribution.root_wall_ns, attribution.unattributed_wall_ns),
            (7_000_000, 0)
        );
        assert!(attribution.within_limit());
        assert_eq!(record.findings(), Vec::new());
    }

    #[test]
    fn parallel_children_are_covered_by_their_union_not_their_sum() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let fold = session.enter("fold");
        let parent = fold.parent();
        let barrier = Arc::new(std::sync::Barrier::new(4));
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let parent = parent.clone();
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    let worker = parent.enter("worker");
                    barrier.wait();
                    spin_for(Duration::from_millis(4));
                    worker.complete();
                });
            }
        });
        fold.complete();
        session.finish();

        let record = only(&sink);
        let (fold_index, fold) = node(&record, "fold");
        let (_, worker) = node(&record, "worker");
        assert_eq!(worker.parent, Some(fold_index));
        assert_eq!((worker.calls, worker.completed), (4, 4));
        assert_eq!(fold.peak_concurrent_children, 4);
        assert!(worker.active_threads_max >= 4);
        let covered = fold.wall_inclusive_ns - fold.wall_exclusive_ns;
        // All four workers overlap, so their summed time exceeds the wall
        // time they cover; the parent is charged the union exactly once.
        assert!(worker.wall_inclusive_ns >= 16_000_000);
        assert!(covered < worker.wall_inclusive_ns);
        assert!(covered <= fold.wall_inclusive_ns);
        assert!(covered.saturating_mul(4) >= worker.wall_inclusive_ns);
        // Opening-thread CPU of the parent excludes work done on other threads.
        assert_eq!(fold.thread_cpu_exclusive_ns, fold.thread_cpu_inclusive_ns);
        let process_covered =
            fold.process_cpu_window_inclusive_ns - fold.process_cpu_window_exclusive_ns;
        assert!(process_covered <= worker.process_cpu_window_inclusive_ns);
        assert!(worker.thread_cpu_inclusive_ns >= 8_000_000);
        assert_no_findings_except_attribution(&record);
    }

    #[test]
    fn disjoint_worker_intervals_under_one_parent_sum_exactly() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let stage = session.enter("stage");
        let parent = stage.parent();
        for _ in 0..3 {
            let parent = parent.clone();
            std::thread::spawn(move || {
                let worker = parent.enter("worker");
                spin_for(Duration::from_millis(1));
                worker.complete();
            })
            .join()
            .unwrap();
        }
        stage.complete();
        session.finish();
        let record = only(&sink);
        let (_, stage) = node(&record, "stage");
        let (_, worker) = node(&record, "worker");
        assert_eq!(stage.peak_concurrent_children, 1);
        assert_eq!(
            stage.wall_inclusive_ns - stage.wall_exclusive_ns,
            worker.wall_inclusive_ns
        );
        assert_eq!(
            stage.process_cpu_window_inclusive_ns - stage.process_cpu_window_exclusive_ns,
            worker.process_cpu_window_inclusive_ns
        );
        // Other-thread children are not subtracted from the opener's CPU.
        assert_eq!(stage.thread_cpu_exclusive_ns, stage.thread_cpu_inclusive_ns);
        assert_no_findings_except_attribution(&record);
    }

    #[test]
    fn unattributed_time_above_one_percent_fails_the_report() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        spin_for(Duration::from_millis(5));
        let phase = session.enter("covered");
        spin_for(Duration::from_millis(5));
        phase.complete();
        session.finish();
        let record = only(&sink);
        let attribution = record.attribution().unwrap();
        assert!(attribution.unattributed_wall_ns >= 5_000_000);
        assert!(!attribution.within_limit());
        let findings = record.findings();
        assert_eq!(
            findings,
            vec![Finding::UnattributedExceedsLimit {
                root_wall_ns: attribution.root_wall_ns,
                unattributed_wall_ns: attribution.unattributed_wall_ns,
            }]
        );
    }

    #[test]
    fn same_label_under_different_parents_stays_separate() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        for parent in ["base", "auxiliary"] {
            let outer = session.enter(parent);
            session.enter("transform").complete();
            outer.complete();
        }
        session.finish();
        let record = only(&sink);
        let transforms: Vec<_> = record
            .phase_tree
            .nodes
            .iter()
            .filter(|node| node.label == "transform")
            .collect();
        assert_eq!(transforms.len(), 2);
        assert_ne!(transforms[0].parent, transforms[1].parent);
        assert_eq!(record.phase_tree.nodes.len(), 5);
    }

    #[test]
    fn dropped_guard_is_interrupted_and_a_panicking_drop_is_unwound() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.enter("done").complete();
        drop(session.enter("error_return"));
        let handle = session.handle();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = handle.enter("panicking");
            panic!("synthetic phase unwind");
        }));
        assert!(unwound.is_err());
        session.finish();
        let record = only(&sink);
        let (_, done) = node(&record, "done");
        let (_, error) = node(&record, "error_return");
        let (_, panicking) = node(&record, "panicking");
        assert_eq!((done.completed, done.interrupted, done.unwound), (1, 0, 0));
        assert_eq!(
            (error.completed, error.interrupted, error.unwound),
            (0, 1, 0)
        );
        assert_eq!(
            (
                panicking.completed,
                panicking.interrupted,
                panicking.unwound
            ),
            (0, 1, 1)
        );
        assert_eq!(record.outcome, RunOutcome::Succeeded);
        assert_eq!(record.recorder.forced_closes, 0);
    }

    #[test]
    fn closing_a_parent_truncates_children_still_open_and_keeps_arithmetic_exact() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let outer = session.enter("outer");
        let inner = session.enter("inner");
        let deepest = session.enter("deepest");
        spin_for(Duration::from_millis(1));
        outer.complete();
        // The guards of the truncated phases are now stale and must not
        // write into the tree a second time.
        deepest.complete();
        drop(inner);
        session.enter("after").complete();
        session.finish();
        let record = only(&sink);
        let (outer_index, outer) = node(&record, "outer");
        let (inner_index, inner) = node(&record, "inner");
        let (_, deepest) = node(&record, "deepest");
        assert_eq!(inner.parent, Some(outer_index));
        assert_eq!(deepest.parent, Some(inner_index));
        assert_eq!((outer.completed, outer.interrupted), (1, 0));
        assert_eq!((inner.calls, inner.interrupted, inner.truncated), (1, 1, 1));
        assert_eq!(
            (deepest.calls, deepest.interrupted, deepest.truncated),
            (1, 1, 1)
        );
        assert_eq!(record.recorder.forced_closes, 2);
        assert_eq!(record.recorder.stale_guard_events, 2);
        assert_eq!(
            outer.wall_exclusive_ns,
            outer.wall_inclusive_ns - inner.wall_inclusive_ns
        );
        assert_eq!(
            inner.wall_exclusive_ns,
            inner.wall_inclusive_ns - deepest.wall_inclusive_ns
        );
        assert_eq!(
            outer.thread_cpu_exclusive_ns,
            outer.thread_cpu_inclusive_ns - inner.thread_cpu_inclusive_ns
        );
        let (_, after) = node(&record, "after");
        assert_eq!(after.parent, Some(0));
    }

    #[test]
    fn finishing_with_open_phases_on_other_threads_truncates_them_without_their_cpu() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let handle = session.handle();
        let (ready_sender, ready) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || {
            let guard = handle.enter("worker_phase");
            handle.enter("worker_child").complete();
            ready_sender.send(()).unwrap();
            released.recv().unwrap();
            drop(guard);
        });
        ready.recv().unwrap();
        session.finish();
        release.send(()).unwrap();
        worker.join().unwrap();
        let record = only(&sink);
        let (worker_index, phase) = node(&record, "worker_phase");
        let (_, child) = node(&record, "worker_child");
        assert_eq!(phase.parent, Some(0));
        assert_eq!(child.parent, Some(worker_index));
        assert_eq!((phase.interrupted, phase.truncated), (1, 1));
        // The worker's CPU clock cannot be read from the finishing thread:
        // only its already closed same-thread child is attributed.
        assert_eq!(phase.thread_cpu_inclusive_ns, child.thread_cpu_inclusive_ns);
        assert_eq!(phase.thread_cpu_exclusive_ns, 0);
        assert_eq!(record.recorder.forced_closes, 1);
        assert!(
            record
                .findings()
                .iter()
                .all(|finding| matches!(finding, Finding::UnattributedExceedsLimit { .. }))
        );
    }

    #[test]
    fn stale_parent_token_falls_back_to_the_root_and_is_counted() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let ended = session.enter("ended");
        let token = ended.parent();
        ended.complete();
        token.enter("late_child").complete();
        session.finish();
        let record = only(&sink);
        let (_, late) = node(&record, "late_child");
        assert_eq!(late.parent, Some(0));
        assert_eq!(record.recorder.stale_parent_events, 1);
    }

    #[test]
    fn recorded_failures_are_retained_and_make_the_outcome_failed() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let phase = session.enter("fri");
        session.record_failure("fri.fold", "degree_mismatch");
        drop(phase);
        session.record_failure("envelope", "self_check_failed");
        session.finish();
        let record = only(&sink);
        assert_eq!(record.outcome, RunOutcome::Failed);
        let (fri_index, _) = node(&record, "fri");
        assert_eq!(
            record.failures.entries,
            vec![
                RawFailure {
                    phase: fri_index,
                    stage: "fri.fold".into(),
                    code: "degree_mismatch".into(),
                },
                RawFailure {
                    phase: 0,
                    stage: "envelope".into(),
                    code: "self_check_failed".into(),
                },
            ]
        );
        assert_eq!(record.failures.dropped, 0);
        assert!(record.findings().contains(&Finding::RunNotSucceeded {
            outcome: RunOutcome::Failed
        }));
    }

    #[test]
    fn failures_beyond_the_retention_bound_are_counted_never_silently_lost() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        for _ in 0..MAX_FAILURES + 3 {
            session.record_failure("loop", "repeated");
        }
        session.finish();
        let record = only(&sink);
        assert_eq!(record.failures.entries.len(), MAX_FAILURES);
        assert_eq!(record.failures.dropped, 3);
        assert_eq!(record.outcome, RunOutcome::Failed);
        assert!(
            record
                .findings()
                .contains(&Finding::FailuresDropped { dropped: 3 })
        );
    }

    #[test]
    fn a_session_dropped_on_an_error_return_is_recorded_as_abandoned() {
        let sink = CollectingSink::new();
        let fallible = |sink: &CollectingSink| -> Result<(), ()> {
            let session = begin(sink);
            let _phase = session.enter("preflight");
            session.record_failure("preflight", "rejected");
            Err(())
        };
        assert!(fallible(&sink).is_err());
        let record = only(&sink);
        assert_eq!(record.outcome, RunOutcome::Abandoned);
        let (_, root) = node(&record, "prove");
        assert_eq!((root.completed, root.interrupted, root.unwound), (0, 1, 0));
        let (_, preflight) = node(&record, "preflight");
        assert_eq!(preflight.interrupted, 1);
        assert_eq!(record.failures.entries.len(), 1);
    }

    #[test]
    fn a_session_dropped_during_a_panic_is_recorded_as_unwound() {
        let sink = CollectingSink::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let session = begin(&sink);
            let _phase = session.enter("composition");
            panic!("synthetic prover unwind");
        }));
        assert!(result.is_err());
        let record = only(&sink);
        assert_eq!(record.outcome, RunOutcome::Unwound);
        let (_, root) = node(&record, "prove");
        assert_eq!((root.interrupted, root.unwound), (1, 1));
        let (_, composition) = node(&record, "composition");
        assert_eq!((composition.interrupted, composition.unwound), (1, 1));
        assert!(record.findings().contains(&Finding::RunNotSucceeded {
            outcome: RunOutcome::Unwound
        }));
    }

    fn populate(session: &Session) {
        let phase = session.enter("phase");
        session.record_bytes(ByteKind::Proof, "proof", 1024);
        session.record_work("columns", 8);
        session.record_failure("stage", "code");
        session.declare(Declaration {
            class: DeclaredClass::EngineeringTarget,
            label: "target",
            unit: Unit::Seconds,
            value: 300,
            provenance: "specs/zk_delivery_plan.md",
        });
        session.declare_phase_workers(8);
        let counter = session.allocation_counter("scratch");
        counter.allocated(4096);
        session.observe_allocation_budget("pool", &AllocationBudget::new(1 << 20));
        phase.complete();
    }

    #[test]
    fn scrub_overwrites_every_owned_element_before_release_returns_the_memory() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        populate(&session);
        let shared = Arc::clone(&session.handle.shared);
        {
            let mut state = shared.lock();
            assert!(!state.is_scrubbed());
            let lengths = (
                state.nodes.len(),
                state.spans.len(),
                state.sources.len(),
                state.bytes.len(),
                state.work.len(),
                state.declared.len(),
                state.failures.len(),
                state.phase_workers.len(),
            );
            assert_eq!(lengths, (2, 2, 2, 1, 1, 2, 1, 1));
            state.scrub();
            // Lengths are unchanged: the old contents were overwritten in
            // place, not merely made unreachable.
            assert_eq!(
                lengths,
                (
                    state.nodes.len(),
                    state.spans.len(),
                    state.sources.len(),
                    state.bytes.len(),
                    state.work.len(),
                    state.declared.len(),
                    state.failures.len(),
                    state.phase_workers.len(),
                )
            );
            assert!(state.is_scrubbed());
            assert_eq!(state.stage, Stage::Scrubbed);
            assert!(!state.is_released());
            state.sink = None;
            state.release();
            assert!(state.is_released());
        }
        // The session is already cleared; ending it now emits nothing.
        session.finish();
        assert!(sink.take().is_empty());
    }

    fn assert_cleared(shared: &Arc<Shared>) {
        let state = shared.lock();
        assert_eq!(state.stage, Stage::Released);
        assert!(state.is_scrubbed());
        assert!(state.is_released());
    }

    #[test]
    fn owned_buffers_are_cleared_on_success() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        populate(&session);
        let shared = Arc::clone(&session.handle.shared);
        session.finish();
        assert_cleared(&shared);
        assert_eq!(only(&sink).outcome, RunOutcome::Failed);
    }

    #[test]
    fn owned_buffers_are_cleared_on_an_error_return() {
        let sink = CollectingSink::new();
        let mut kept = None;
        let fallible = |kept: &mut Option<Arc<Shared>>| -> Result<(), ()> {
            let session = begin(&sink);
            populate(&session);
            *kept = Some(Arc::clone(&session.handle.shared));
            Err(())
        };
        assert!(fallible(&mut kept).is_err());
        assert_cleared(&kept.unwrap());
        assert_eq!(only(&sink).outcome, RunOutcome::Abandoned);
    }

    #[test]
    fn owned_buffers_are_cleared_on_unwind() {
        let sink = CollectingSink::new();
        let kept = Arc::new(Mutex::new(None));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let session = begin(&sink);
            populate(&session);
            let _open = session.enter("open_at_panic");
            *kept.lock().unwrap() = Some(Arc::clone(&session.handle.shared));
            panic!("synthetic unwind with populated buffers");
        }));
        assert!(result.is_err());
        let shared = kept.lock().unwrap().take().unwrap();
        assert_cleared(&shared);
        assert_eq!(only(&sink).outcome, RunOutcome::Unwound);
    }

    #[test]
    fn recording_after_the_session_ended_changes_nothing() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let handle = session.handle();
        let counter = session.allocation_counter("scratch");
        let stale = session.enter("stale");
        let token = stale.parent();
        let shared = Arc::clone(&session.handle.shared);
        session.finish();
        handle.enter("late").complete();
        token.enter("late_child").complete();
        handle.record_bytes(ByteKind::Key, "late", 1);
        handle.record_work("late", 1);
        handle.record_failure("late", "late");
        handle.declare_phase_workers(2);
        handle.declare(Declaration {
            class: DeclaredClass::Projection,
            label: "late",
            unit: Unit::Count,
            value: 1,
            provenance: "late",
        });
        handle.observe_allocation_budget("late", &AllocationBudget::new(8));
        let late_counter = handle.allocation_counter("late");
        late_counter.allocated(8);
        late_counter.freed(8);
        drop(counter.buffer(16));
        drop(stale);
        assert_cleared(&shared);
        let record = only(&sink);
        assert!(
            record
                .phase_tree
                .nodes
                .iter()
                .all(|node| node.label != "late")
        );
        assert!(sink.take().is_empty());
    }

    #[test]
    fn byte_counters_aggregate_per_phase_kind_and_label() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.record_bytes(ByteKind::Transaction, "transfer", 700);
        let phase = session.enter("encode");
        for bytes in [900, 1100, 1000] {
            session.record_bytes(ByteKind::Proof, "frame", bytes);
        }
        session.record_bytes(ByteKind::Key, "frame", 32);
        session.record_bytes(ByteKind::Ciphertext, "payload", 0);
        phase.complete();
        session.finish();
        let record = only(&sink);
        let (encode, _) = node(&record, "encode");
        let entries = &record.byte_counters.entries;
        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries[0],
            ByteCounter {
                phase: 0,
                kind: ByteKind::Transaction,
                label: "transfer".into(),
                count: 1,
                total_bytes: 700,
                min_bytes: 700,
                max_bytes: 700,
            }
        );
        assert_eq!(
            entries[1],
            ByteCounter {
                phase: encode,
                kind: ByteKind::Proof,
                label: "frame".into(),
                count: 3,
                total_bytes: 3000,
                min_bytes: 900,
                max_bytes: 1100,
            }
        );
        assert_eq!(
            (entries[2].kind, entries[2].total_bytes),
            (ByteKind::Key, 32)
        );
        assert_eq!((entries[3].count, entries[3].max_bytes), (1, 0));
    }

    #[test]
    fn scoped_counters_track_counts_bytes_and_live_high_water_per_phase() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let counter = session.allocation_counter("lde_columns");
        let outer = session.enter("outer");
        let first = counter.buffer(1000);
        let inner = session.enter("inner");
        let second = counter.buffer(3000);
        drop(second);
        inner.complete();
        counter.allocated(500);
        drop(first);
        outer.complete();
        counter.freed(500);
        session.finish();
        let record = only(&sink);
        let source = &record.allocations.sources[0];
        assert_eq!(
            *source,
            AllocationSource {
                label: "lde_columns".into(),
                kind: AllocationSourceKind::ScopedCounter,
                allocations: 3,
                allocated_bytes: 4500,
                frees: 3,
                freed_bytes: 4500,
                live_bytes: 0,
                live_bytes_high_water: 4000,
                live_buffers: 0,
                live_buffers_high_water: 2,
            }
        );
        let (_, outer) = node(&record, "outer");
        let (_, inner) = node(&record, "inner");
        let (_, root) = node(&record, "prove");
        assert_eq!((outer.allocations, outer.allocated_bytes), (2, 1500));
        assert_eq!((inner.allocations, inner.allocated_bytes), (1, 3000));
        assert_eq!((root.allocations, root.allocated_bytes), (0, 0));
        assert_eq!(inner.live_bytes_high_water, 4000);
        assert_eq!(outer.live_bytes_high_water, 4000);
        assert_eq!(root.live_bytes_high_water, 4000);
        assert_no_findings_except_attribution(&record);
    }

    #[test]
    fn allocation_budget_is_observed_without_reserving_from_it() {
        let budget = AllocationBudget::new(64 * 1024);
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.observe_allocation_budget("prover_pool", &budget);
        let phase = session.enter("reserve");
        let reservation = budget.try_reserve_bytes(40_000).unwrap();
        session.enter("inside").complete();
        drop(reservation);
        let held = budget.try_reserve_bytes(1_000).unwrap();
        phase.complete();
        session.finish();
        // The observer took no credit and returned its handle.
        assert_eq!(budget.reserved_bytes(), 1_000);
        drop(held);
        assert_eq!(budget.reserved_bytes(), 0);
        let record = only(&sink);
        assert_eq!(
            record.allocations.sources,
            vec![AllocationSource {
                label: "prover_pool".into(),
                kind: AllocationSourceKind::AllocationBudget,
                allocations: 0,
                allocated_bytes: 0,
                frees: 0,
                freed_bytes: 0,
                live_bytes: 1_000,
                live_bytes_high_water: 40_000,
                live_buffers: 0,
                live_buffers_high_water: 0,
            }]
        );
        let (_, inside) = node(&record, "inside");
        assert_eq!(inside.live_bytes_high_water, 40_000);
        assert_eq!(
            record.declared,
            vec![DeclaredQuantity {
                classification: Classification::LocalScheduling,
                label: "prover_pool".into(),
                unit: Unit::Bytes,
                value: 64 * 1024,
                provenance_kind: ProvenanceKind::LocalConfiguration,
                provenance: BUDGET_LIMIT_PROVENANCE.into(),
            }]
        );
    }

    #[test]
    fn declared_classes_map_to_classification_and_permitted_provenance() {
        let expected = [
            (
                DeclaredClass::Projection,
                Classification::Projection,
                ProvenanceKind::ModelProjection,
            ),
            (
                DeclaredClass::EngineeringTarget,
                Classification::EngineeringTarget,
                ProvenanceKind::PlanTarget,
            ),
            (
                DeclaredClass::ConsensusBoundFromProtocolConstant,
                Classification::DeterministicConsensusBound,
                ProvenanceKind::ProtocolConstant,
            ),
            (
                DeclaredClass::ConsensusBoundFromCommittedState,
                Classification::DeterministicConsensusBound,
                ProvenanceKind::CommittedState,
            ),
            (
                DeclaredClass::LocalConfiguration,
                Classification::LocalScheduling,
                ProvenanceKind::LocalConfiguration,
            ),
            (
                DeclaredClass::OperatingSystemLimit,
                Classification::LocalScheduling,
                ProvenanceKind::OperatingSystem,
            ),
        ];
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.enter("phase").complete();
        for (class, _, _) in expected {
            assert_ne!(class.schema_pair().0, Classification::Measured);
            session.declare(Declaration {
                class,
                label: "quantity",
                unit: Unit::Bytes,
                value: 9_437_184,
                provenance: "crates/a/src/profile.rs:LIMIT",
            });
        }
        session.finish();
        let record = only(&sink);
        assert_eq!(record.declared.len(), expected.len());
        for (declared, (class, classification, provenance_kind)) in
            record.declared.iter().zip(expected)
        {
            assert_eq!(class.schema_pair(), (classification, provenance_kind));
            assert_eq!(declared.classification, classification);
            assert_eq!(declared.provenance_kind, provenance_kind);
            assert_eq!(declared.value, 9_437_184);
        }
        assert_no_findings_except_attribution(&record);
    }

    #[test]
    fn worker_declarations_are_recorded_as_local_scheduling() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        session.declare_phase_workers(20);
        let phase = session.enter("fold");
        session.declare_phase_workers(8);
        session.declare_phase_workers(6);
        phase.complete();
        session.finish();
        let record = only(&sink);
        let (fold, _) = node(&record, "fold");
        assert_eq!(
            record.scheduling,
            Scheduling {
                classification: Classification::LocalScheduling,
                workers: 4,
                workers_provenance: "test.fixed_workers".into(),
                phase_workers: vec![
                    PhaseWorkers {
                        phase: 0,
                        workers: 20
                    },
                    PhaseWorkers {
                        phase: fold,
                        workers: 6
                    },
                ],
            }
        );
    }

    #[test]
    fn malformed_literals_are_replaced_counted_and_reported() {
        let sink = CollectingSink::new();
        let session = Session::begin(
            RunIdentity::new(bound_context(), "unit", FlowKind::Proof, "rust.test"),
            "root with spaces",
            WorkerDeclaration {
                workers: 1,
                provenance: "has spaces",
            },
            Box::new(sink.clone()),
        );
        session.enter("bad label").complete();
        session.record_bytes(ByteKind::Proof, "bad label", 1);
        session.record_work("bad label", 1);
        session.record_failure("bad stage", "bad code");
        session.declare(Declaration {
            class: DeclaredClass::Projection,
            label: "bad label",
            unit: Unit::Count,
            value: 1,
            provenance: "bad provenance",
        });
        let _counter = session.allocation_counter("bad label");
        session.finish();
        let record = only(&sink);
        assert_eq!(record.recorder.invalid_label_events, 10);
        assert_eq!(record.work_counters.entries[0].label, text::INVALID_LABEL);
        assert!(
            record
                .phase_tree
                .nodes
                .iter()
                .all(|node| node.label == text::INVALID_LABEL)
        );
        assert_eq!(record.scheduling.workers_provenance, text::INVALID_LABEL);
        assert!(record.findings().contains(&Finding::LabelNotPublic {
            section: "recorder",
            index: 0
        }));
    }

    #[test]
    fn bounded_tables_refuse_excess_entries_and_report_the_overflow() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        {
            let mut state = session.handle.shared.lock();
            while state.nodes.len() < MAX_NODES {
                state.nodes.push(Node {
                    parent: 0,
                    label: "filler",
                    ..Node::default()
                });
            }
            while state.bytes.len() < MAX_BYTE_COUNTERS {
                state.bytes.push(ByteEntry {
                    node: 0,
                    count: 1,
                    ..ByteEntry::EMPTY
                });
            }
            while state.work.len() < MAX_WORK_COUNTERS {
                state.work.push(WorkEntry {
                    count: 1,
                    ..WorkEntry::EMPTY
                });
            }
            while state.declared.len() < MAX_DECLARED {
                state.declared.push(Declared::EMPTY);
            }
            while state.phase_workers.len() < MAX_PHASE_WORKERS {
                let phase = narrow(state.phase_workers.len() + 1);
                state.phase_workers.push(PhaseWorkers { phase, workers: 1 });
            }
            while state.sources.len() < MAX_ALLOCATION_SOURCES {
                state.sources.push(Source::empty());
            }
        }
        session.enter("one_too_many").complete();
        session.record_bytes(ByteKind::Proof, "one_too_many", 1);
        session.record_work("one_too_many", 1);
        session.declare(Declaration {
            class: DeclaredClass::Projection,
            label: "one_too_many",
            unit: Unit::Count,
            value: 1,
            provenance: "model",
        });
        session.declare_phase_workers(3);
        let refused = session.allocation_counter("one_too_many");
        refused.allocated(64);
        refused.freed(64);
        session.observe_allocation_budget("one_too_many", &AllocationBudget::new(1));
        {
            let state = session.handle.shared.lock();
            assert_eq!(state.health.node_overflow_events, 1);
            assert_eq!(state.health.counter_overflow_events, 6);
            assert_eq!(state.nodes.len(), MAX_NODES);
            assert_eq!(state.bytes.len(), MAX_BYTE_COUNTERS);
            assert_eq!(state.work.len(), MAX_WORK_COUNTERS);
            assert_eq!(state.declared.len(), MAX_DECLARED);
            assert_eq!(state.sources.len(), MAX_ALLOCATION_SOURCES);
        }
        session.finish();
        let record = only(&sink);
        assert_eq!(record.recorder.node_overflow_events, 1);
        assert!(record.findings().contains(&Finding::RecorderOverflow));
    }

    #[test]
    fn open_phase_table_is_bounded_and_reports_the_overflow() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let mut guards = Vec::new();
        for _ in 0..MAX_OPEN_SPANS + 2 {
            guards.push(session.enter("nested"));
        }
        {
            let state = session.handle.shared.lock();
            assert_eq!(state.spans.len(), MAX_OPEN_SPANS);
            // The root holds one slot, so three entries were refused.
            assert_eq!(state.health.span_overflow_events, 3);
        }
        while let Some(guard) = guards.pop() {
            guard.complete();
        }
        session.finish();
        let record = only(&sink);
        assert_eq!(record.recorder.span_overflow_events, 3);
        assert_eq!(record.recorder.stale_guard_events, 0);
        assert_eq!(record.recorder.forced_closes, 0);
        assert!(record.findings().contains(&Finding::RecorderOverflow));
    }

    #[test]
    fn process_and_address_space_observations_are_taken_from_the_platform() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let phase = session.enter("work");
        spin_for(Duration::from_millis(3));
        phase.complete();
        session.finish();
        let record = only(&sink);
        let process = &record.process;
        assert_eq!(process.classification, Classification::Measured);
        assert_eq!(process.pid, std::process::id());
        assert!(process.logical_cpus >= 1);
        assert_eq!(
            record.address_space.classification,
            Classification::LocalScheduling
        );
        assert_eq!(
            record.address_space.enforced,
            record.address_space.soft_limit_bytes.is_some()
        );
        #[cfg(unix)]
        {
            assert_eq!(
                process.peak_rss_source,
                PeakRssSource::KernelLifetimeHighWater
            );
            assert!(process.peak_rss_bytes >= process.peak_rss_bytes_at_begin);
            assert!(process.peak_rss_bytes > 1 << 20);
            assert!(process.cpu_user_ns + process.cpu_system_ns >= 1_000_000);
            assert_eq!(record.address_space.source, "getrlimit.RLIMIT_AS");
            let limit = platform::address_space_limit().unwrap();
            assert_eq!(record.address_space.soft_limit_bytes, limit.soft_bytes);
            assert_eq!(record.address_space.hard_limit_bytes, limit.hard_bytes);
        }
        assert_eq!(
            process.thermal_finish == ThermalState::Unavailable,
            process.thermal_source == "unavailable"
        );
        let (_, root) = node(&record, "prove");
        assert!(root.load_milli_max >= root.load_milli_first_enter);
        assert!(root.load_milli_max >= root.load_milli_last_exit);
    }

    #[test]
    fn clock_samples_are_clamped_monotonic_and_anomalies_are_counted() {
        let sink = CollectingSink::new();
        let session = begin(&sink);
        let shared = Arc::clone(&session.handle.shared);
        {
            let mut state = shared.lock();
            let (wall, process_cpu) = state.sample(&*shared.clock);
            assert_eq!(state.health.clock_anomaly_events, 0);
            state.last_wall = wall + 3_600_000_000_000;
            state.last_process_cpu = process_cpu + 3_600_000_000_000;
            let clamped = state.sample(&*shared.clock);
            assert_eq!(clamped, (state.last_wall, state.last_process_cpu));
            assert_eq!(state.health.clock_anomaly_events, 1);
        }
        session.finish();
        let record = only(&sink);
        // Ending the session takes one more sample below the forced maximum.
        assert_eq!(record.recorder.clock_anomaly_events, 2);
        assert!(record.findings().contains(&Finding::ClockAnomaly));
    }

    #[test]
    fn helper_predicates_and_conversions_behave_at_their_edges() {
        assert_eq!(index(7), 7);
        assert_eq!(narrow(7), 7);
        assert_eq!(narrow(usize::MAX), NONE);
        assert_eq!(identity_or_invalid("a/b"), ("a/b", true));
        assert_eq!(identity_or_invalid("a b"), (text::INVALID_LABEL, false));
        assert!(Source::empty().is_empty());
        let budget = AllocationBudget::new(100);
        let held = budget.try_reserve_bytes(60).unwrap();
        let observed = Source {
            budget: Some(budget.clone()),
            live_bytes: 5,
            ..Source::empty()
        };
        assert!(!observed.is_empty());
        assert_eq!(observed.live_now(), 60);
        drop(held);
        assert_eq!(observed.live_now(), 0);
        let scoped = Source {
            live_bytes: 5,
            ..Source::empty()
        };
        assert_eq!(scoped.live_now(), 5);
        let system = SystemClock {
            origin: Instant::now(),
        };
        let boundary = Boundary::sample(&system);
        assert_eq!(boundary.thread, std::thread::current().id());
        assert_eq!(boundary.thread_cpu.is_some(), cfg!(unix));
        assert_eq!(system.process_cpu_ns().is_some(), cfg!(unix));
        let earlier = system.wall_ns();
        assert!(system.wall_ns() >= earlier);
        assert!(system.load_milli() < 100_000_000);
    }
}
