//! Report-side checks of a finished record.
//!
//! These functions are for reporting tools, the harness script's Rust twin
//! and tests. They take a finished [`MeasurementRecord`]; nothing here is
//! reachable from the recorder, so instrumented code cannot branch on a
//! measurement. Nothing in this crate may be consulted by validation code:
//! transaction validity, gas, effects and certified roots never depend on a
//! measurement, a target or a local limit recorded here.

use norito::json::Value;

use crate::{
    schema::{
        AllocationSourceKind, Classification, MeasurementRecord, PeakRssSource, PhaseNode,
        ProvenanceKind, RECORD_SCHEMA_V1, RunOutcome, STRUCTURAL_NUMBER_KEYS,
    },
    text,
};

/// Numerator of the largest admissible unattributed share of root wall time.
pub const UNATTRIBUTED_NUMERATOR: u64 = 1;
/// Denominator of the largest admissible unattributed share of root wall time.
pub const UNATTRIBUTED_DENOMINATOR: u64 = 100;

/// Root wall time and the part of it outside every direct child phase.
///
/// The unattributed time is the root's exclusive wall time. A non-root
/// phase's own exclusive time is attributed to that phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribution {
    /// Wall time of the complete measured root.
    pub root_wall_ns: u64,
    /// Root wall time during which no direct child phase was open.
    pub unattributed_wall_ns: u64,
}

impl Attribution {
    /// Whether at most one percent of a nonzero root wall time is unattributed.
    pub fn within_limit(self) -> bool {
        self.root_wall_ns > 0
            && u128::from(self.unattributed_wall_ns) * u128::from(UNATTRIBUTED_DENOMINATOR)
                <= u128::from(self.root_wall_ns) * u128::from(UNATTRIBUTED_NUMERATOR)
    }

    /// Unattributed share in parts per million, rounded up; for display only.
    pub fn unattributed_parts_per_million(self) -> u64 {
        parts_per_million(self.unattributed_wall_ns, self.root_wall_ns)
    }
}

/// `part / whole` in parts per million, rounded up; one million for no whole.
fn parts_per_million(part: u64, whole: u64) -> u64 {
    if whole == 0 {
        return 1_000_000;
    }
    let scaled = u128::from(part) * 1_000_000;
    u64::try_from(scaled.div_ceil(u128::from(whole))).unwrap_or(u64::MAX)
}

/// The phase below the root that holds the most wall time not divided into
/// child phases.
///
/// The one-percent rule bounds only the time outside every phase. A tree
/// whose root has a single wrapper phase satisfies it trivially, so reports
/// also state how coarse the tree is: the largest exclusive time of any
/// non-root phase against the root. Consumers that need finer attribution
/// state a bound on this share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UndividedPhase {
    /// Index of the phase in the tree.
    pub phase: u32,
    /// Exclusive wall time of that phase, summed over its calls.
    pub wall_exclusive_ns: u64,
    /// Wall time of the complete measured root.
    pub root_wall_ns: u64,
}

impl UndividedPhase {
    /// Share of the root in parts per million, rounded up; for display and
    /// for a consumer-stated granularity bound. Calls that overlap in time
    /// are summed, so the share of a parallel phase can exceed one million.
    pub fn share_parts_per_million(self) -> u64 {
        parts_per_million(self.wall_exclusive_ns, self.root_wall_ns)
    }
}

/// How a finding affects the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The record contradicts the schema's invariants and cannot be trusted.
    Malformed,
    /// The record is an honest observation that is not a complete, successful,
    /// fully identified measurement. It is retained and reported as rejected.
    Rejected,
}

/// One reason a record is not an acceptable complete measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finding {
    /// The schema identity is not the supported one.
    SchemaMismatch,
    /// An identity field is unbound or outside its grammar.
    IdentityIncomplete {
        /// Name of the field.
        field: &'static str,
    },
    /// A dirty tree lacks its difference digest, or a clean tree carries one.
    DirtyDigestMismatch,
    /// A label or provenance is outside the public grammar, or the recorder
    /// replaced a malformed literal.
    LabelNotPublic {
        /// Section that holds the label.
        section: &'static str,
        /// Index of the entry inside that section.
        index: u32,
    },
    /// A section does not carry its fixed classification.
    SectionClassification {
        /// Name of the section.
        section: &'static str,
    },
    /// A declared quantity is classified as measured or has provenance its
    /// classification does not admit.
    DeclaredClassification {
        /// Index of the declared quantity.
        index: u32,
    },
    /// The phase tree has no root.
    TreeEmpty,
    /// The root has a parent, a non-root has none, or the root was not
    /// entered exactly once.
    RootShape,
    /// A phase's parent does not precede it.
    ParentOrder {
        /// Index of the phase.
        index: u32,
    },
    /// Two phases under one parent share a label.
    DuplicateSibling {
        /// Index of the later phase.
        index: u32,
    },
    /// Calls, completions, interruptions, unwinds and truncations disagree.
    CallAccounting {
        /// Index of the phase.
        index: u32,
    },
    /// Inclusive, exclusive and child wall times disagree.
    WallAccounting {
        /// Index of the phase.
        index: u32,
    },
    /// Opening-thread CPU times disagree.
    ThreadCpuAccounting {
        /// Index of the phase.
        index: u32,
    },
    /// Process-window CPU times disagree.
    ProcessCpuAccounting {
        /// Index of the phase.
        index: u32,
    },
    /// Load or concurrency observations disagree.
    BoundaryAccounting {
        /// Index of the phase.
        index: u32,
    },
    /// An entry refers to a phase that does not exist.
    PhaseReference {
        /// Section that holds the reference.
        section: &'static str,
        /// Index of the entry inside that section.
        index: u32,
    },
    /// A byte counter's count, total, minimum and maximum disagree.
    ByteCounterAccounting {
        /// Index of the counter.
        index: u32,
    },
    /// A work counter's count, total, minimum and maximum disagree.
    WorkCounterAccounting {
        /// Index of the counter.
        index: u32,
    },
    /// An allocation source or the per-phase allocation totals disagree.
    AllocationAccounting {
        /// Index of the source, or the number of sources for the totals.
        index: u32,
    },
    /// The address-space limits contradict each other.
    AddressSpaceAccounting,
    /// The process observation contradicts itself.
    ProcessAccounting,
    /// A worker declaration is zero.
    SchedulingAccounting,
    /// The outcome contradicts the failure log or the root's completion.
    OutcomeMismatch,
    /// Failures exceeded the retention bound.
    FailuresDropped {
        /// Number of failures that were counted but not retained.
        dropped: u64,
    },
    /// The run did not succeed. The record is retained as a raw failure.
    RunNotSucceeded {
        /// How the run ended.
        outcome: RunOutcome,
    },
    /// The recorder refused entries because a bounded table was full.
    RecorderOverflow,
    /// A clock sample was not monotonic.
    ClockAnomaly,
    /// The platform reported no peak resident set size.
    PeakRssUnavailable,
    /// The root has no child phase.
    NoPhases,
    /// More than one percent of the root wall time is outside every phase.
    UnattributedExceedsLimit {
        /// Wall time of the root.
        root_wall_ns: u64,
        /// Root wall time outside every direct child phase.
        unattributed_wall_ns: u64,
    },
}

impl Finding {
    /// Stable code of this finding, shared with `scripts/zk_resource_harness.py`
    /// and checked against it by `fixtures/record_mutations_v1.json`.
    pub fn code(self) -> String {
        match self {
            Self::SchemaMismatch => "schema_mismatch".to_owned(),
            Self::IdentityIncomplete { field } => format!("identity_incomplete:{field}"),
            Self::DirtyDigestMismatch => "dirty_digest_mismatch".to_owned(),
            Self::LabelNotPublic { section, index } => {
                format!("label_not_public:{section}:{index}")
            }
            Self::SectionClassification { section } => {
                format!("section_classification:{section}")
            }
            Self::DeclaredClassification { index } => format!("declared_classification:{index}"),
            Self::TreeEmpty => "tree_empty".to_owned(),
            Self::RootShape => "root_shape".to_owned(),
            Self::ParentOrder { index } => format!("parent_order:{index}"),
            Self::DuplicateSibling { index } => format!("duplicate_sibling:{index}"),
            Self::CallAccounting { index } => format!("call_accounting:{index}"),
            Self::WallAccounting { index } => format!("wall_accounting:{index}"),
            Self::ThreadCpuAccounting { index } => format!("thread_cpu_accounting:{index}"),
            Self::ProcessCpuAccounting { index } => format!("process_cpu_accounting:{index}"),
            Self::BoundaryAccounting { index } => format!("boundary_accounting:{index}"),
            Self::PhaseReference { section, index } => {
                format!("phase_reference:{section}:{index}")
            }
            Self::ByteCounterAccounting { index } => format!("byte_counter_accounting:{index}"),
            Self::WorkCounterAccounting { index } => format!("work_counter_accounting:{index}"),
            Self::AllocationAccounting { index } => format!("allocation_accounting:{index}"),
            Self::AddressSpaceAccounting => "address_space_accounting".to_owned(),
            Self::ProcessAccounting => "process_accounting".to_owned(),
            Self::SchedulingAccounting => "scheduling_accounting".to_owned(),
            Self::OutcomeMismatch => "outcome_mismatch".to_owned(),
            Self::FailuresDropped { dropped } => format!("failures_dropped:{dropped}"),
            Self::RunNotSucceeded { outcome } => {
                format!("run_not_succeeded:{}", outcome.as_str())
            }
            Self::RecorderOverflow => "recorder_overflow".to_owned(),
            Self::ClockAnomaly => "clock_anomaly".to_owned(),
            Self::PeakRssUnavailable => "peak_rss_unavailable".to_owned(),
            Self::NoPhases => "no_phases".to_owned(),
            Self::UnattributedExceedsLimit {
                root_wall_ns,
                unattributed_wall_ns,
            } => format!("unattributed_exceeds_limit:{unattributed_wall_ns}:{root_wall_ns}"),
        }
    }

    /// Whether the finding marks a malformed record or a rejected observation.
    pub fn severity(self) -> Severity {
        match self {
            Self::IdentityIncomplete { .. }
            | Self::DirtyDigestMismatch
            | Self::FailuresDropped { .. }
            | Self::RunNotSucceeded { .. }
            | Self::RecorderOverflow
            | Self::ClockAnomaly
            | Self::PeakRssUnavailable
            | Self::NoPhases
            | Self::UnattributedExceedsLimit { .. } => Severity::Rejected,
            _ => Severity::Malformed,
        }
    }
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// `covered <= children <= peak * covered`, the exact union bound.
fn union_bound(covered: u64, children: u128, peak: u32) -> bool {
    u128::from(covered) <= children && children <= u128::from(peak) * u128::from(covered)
}

fn node_findings(nodes: &[PhaseNode], findings: &mut Vec<Finding>) {
    for (position, node) in nodes.iter().enumerate() {
        let index = narrow(position);
        match (position, node.parent) {
            (0, None) => {
                if node.calls != 1 {
                    findings.push(Finding::RootShape);
                }
            }
            (0, Some(_)) | (_, None) => findings.push(Finding::RootShape),
            (_, Some(parent)) => {
                if parent >= index {
                    findings.push(Finding::ParentOrder { index });
                }
            }
        }
        if !text::is_public_label(&node.label) {
            findings.push(Finding::LabelNotPublic {
                section: "phase_tree",
                index,
            });
        }
        if nodes[..position]
            .iter()
            .any(|earlier| earlier.parent == node.parent && earlier.label == node.label)
        {
            findings.push(Finding::DuplicateSibling { index });
        }
        if node.completed.checked_add(node.interrupted) != Some(node.calls)
            || node.unwound > node.interrupted
            || node.truncated > node.interrupted
        {
            findings.push(Finding::CallAccounting { index });
        }
        let children = || nodes.iter().filter(|child| child.parent == Some(index));
        let child_calls: u128 = children().map(|child| u128::from(child.calls)).sum();
        let child_wall: u128 = children()
            .map(|child| u128::from(child.wall_inclusive_ns))
            .sum();
        let child_thread: u128 = children()
            .map(|child| u128::from(child.thread_cpu_inclusive_ns))
            .sum();
        let child_process: u128 = children()
            .map(|child| u128::from(child.process_cpu_window_inclusive_ns))
            .sum();
        let peak = node.peak_concurrent_children;
        match node.wall_inclusive_ns.checked_sub(node.wall_exclusive_ns) {
            Some(covered) if union_bound(covered, child_wall, peak) => {}
            _ => findings.push(Finding::WallAccounting { index }),
        }
        match node
            .thread_cpu_inclusive_ns
            .checked_sub(node.thread_cpu_exclusive_ns)
        {
            Some(covered) if u128::from(covered) <= child_thread => {}
            _ => findings.push(Finding::ThreadCpuAccounting { index }),
        }
        match node
            .process_cpu_window_inclusive_ns
            .checked_sub(node.process_cpu_window_exclusive_ns)
        {
            Some(covered) if union_bound(covered, child_process, peak) => {}
            _ => findings.push(Finding::ProcessCpuAccounting { index }),
        }
        if (peak == 0) != (child_calls == 0)
            || u128::from(peak) > child_calls
            || node.load_milli_max < node.load_milli_first_enter
            || node.load_milli_max < node.load_milli_last_exit
            || (node.calls > 0 && node.active_threads_max == 0)
        {
            findings.push(Finding::BoundaryAccounting { index });
        }
    }
}

impl MeasurementRecord {
    /// Root wall time and its unattributed part; `None` for an empty tree.
    pub fn attribution(&self) -> Option<Attribution> {
        self.phase_tree.nodes.first().map(|root| Attribution {
            root_wall_ns: root.wall_inclusive_ns,
            unattributed_wall_ns: root.wall_exclusive_ns,
        })
    }

    /// The non-root phase with the most exclusive wall time; the earliest
    /// such phase on a tie, and `None` when the root has no phase below it.
    pub fn largest_undivided_phase(&self) -> Option<UndividedPhase> {
        let root_wall_ns = self.phase_tree.nodes.first()?.wall_inclusive_ns;
        let mut largest: Option<UndividedPhase> = None;
        for (position, node) in self.phase_tree.nodes.iter().enumerate().skip(1) {
            if largest.is_none_or(|current| node.wall_exclusive_ns > current.wall_exclusive_ns) {
                largest = Some(UndividedPhase {
                    phase: narrow(position),
                    wall_exclusive_ns: node.wall_exclusive_ns,
                    root_wall_ns,
                });
            }
        }
        largest
    }

    fn identity_findings(&self, findings: &mut Vec<Finding>) {
        let identity = &self.identity;
        let context = &identity.context;
        for (field, valid) in [
            ("workload", text::is_public_label(&identity.workload)),
            ("emitter", text::is_public_label(&identity.emitter)),
            (
                "source_commit",
                text::is_source_commit(&context.source_commit),
            ),
            ("artifact", text::is_identity_text(&context.artifact)),
            ("profile", text::is_identity_text(&context.profile)),
            ("config", text::is_identity_text(&context.config)),
            ("hardware", text::is_identity_text(&context.hardware)),
        ] {
            if !valid {
                findings.push(Finding::IdentityIncomplete { field });
            }
        }
        for (field, value) in [
            ("workload", &identity.workload),
            ("emitter", &identity.emitter),
            ("artifact", &context.artifact),
            ("profile", &context.profile),
            ("config", &context.config),
            ("hardware", &context.hardware),
        ] {
            if value == text::UNBOUND || value == text::INVALID_LABEL {
                findings.push(Finding::IdentityIncomplete { field });
            }
        }
        let digest_matches = match (&context.source_dirty_digest, context.source_dirty) {
            (Some(digest), true) => text::is_sha256_hex(digest),
            (None, false) => true,
            _ => false,
        };
        if !digest_matches {
            findings.push(Finding::DirtyDigestMismatch);
        }
    }

    fn section_findings(&self, findings: &mut Vec<Finding>) {
        for (section, actual, expected) in [
            (
                "failures",
                self.failures.classification,
                Classification::Measured,
            ),
            (
                "phase_tree",
                self.phase_tree.classification,
                Classification::Measured,
            ),
            (
                "byte_counters",
                self.byte_counters.classification,
                Classification::Measured,
            ),
            (
                "work_counters",
                self.work_counters.classification,
                Classification::Measured,
            ),
            (
                "allocations",
                self.allocations.classification,
                Classification::Measured,
            ),
            (
                "process",
                self.process.classification,
                Classification::Measured,
            ),
            (
                "scheduling",
                self.scheduling.classification,
                Classification::LocalScheduling,
            ),
            (
                "address_space",
                self.address_space.classification,
                Classification::LocalScheduling,
            ),
        ] {
            if actual != expected {
                findings.push(Finding::SectionClassification { section });
            }
        }
        for (position, declared) in self.declared.iter().enumerate() {
            let index = narrow(position);
            let admitted = match declared.classification {
                Classification::Projection => {
                    declared.provenance_kind == ProvenanceKind::ModelProjection
                }
                Classification::EngineeringTarget => {
                    declared.provenance_kind == ProvenanceKind::PlanTarget
                }
                Classification::DeterministicConsensusBound => matches!(
                    declared.provenance_kind,
                    ProvenanceKind::ProtocolConstant | ProvenanceKind::CommittedState
                ),
                Classification::LocalScheduling => matches!(
                    declared.provenance_kind,
                    ProvenanceKind::LocalConfiguration | ProvenanceKind::OperatingSystem
                ),
                Classification::Measured => false,
            };
            if !admitted {
                findings.push(Finding::DeclaredClassification { index });
            }
            if !text::is_public_label(&declared.label)
                || !text::is_identity_text(&declared.provenance)
                || declared.provenance == text::INVALID_LABEL
            {
                findings.push(Finding::LabelNotPublic {
                    section: "declared",
                    index,
                });
            }
        }
    }

    fn counter_findings(&self, findings: &mut Vec<Finding>) {
        let phases = narrow(self.phase_tree.nodes.len());
        for (position, failure) in self.failures.entries.iter().enumerate() {
            let index = narrow(position);
            if failure.phase >= phases {
                findings.push(Finding::PhaseReference {
                    section: "failures",
                    index,
                });
            }
            if !text::is_public_label(&failure.stage) || !text::is_public_label(&failure.code) {
                findings.push(Finding::LabelNotPublic {
                    section: "failures",
                    index,
                });
            }
        }
        for (position, counter) in self.byte_counters.entries.iter().enumerate() {
            let index = narrow(position);
            if counter.phase >= phases {
                findings.push(Finding::PhaseReference {
                    section: "byte_counters",
                    index,
                });
            }
            if !text::is_public_label(&counter.label) {
                findings.push(Finding::LabelNotPublic {
                    section: "byte_counters",
                    index,
                });
            }
            let (count, total) = (u128::from(counter.count), u128::from(counter.total_bytes));
            if counter.count == 0
                || counter.min_bytes > counter.max_bytes
                || total < count * u128::from(counter.min_bytes)
                || total > count * u128::from(counter.max_bytes)
            {
                findings.push(Finding::ByteCounterAccounting { index });
            }
        }
        for (position, counter) in self.work_counters.entries.iter().enumerate() {
            let index = narrow(position);
            if counter.phase >= phases {
                findings.push(Finding::PhaseReference {
                    section: "work_counters",
                    index,
                });
            }
            if !text::is_public_label(&counter.label) {
                findings.push(Finding::LabelNotPublic {
                    section: "work_counters",
                    index,
                });
            }
            let (count, total) = (u128::from(counter.count), u128::from(counter.total_units));
            if counter.count == 0
                || counter.min_units > counter.max_units
                || total < count * u128::from(counter.min_units)
                || total > count * u128::from(counter.max_units)
            {
                findings.push(Finding::WorkCounterAccounting { index });
            }
        }
        let (mut allocations, mut allocated_bytes) = (0_u128, 0_u128);
        for (position, source) in self.allocations.sources.iter().enumerate() {
            let index = narrow(position);
            if !text::is_public_label(&source.label) {
                findings.push(Finding::LabelNotPublic {
                    section: "allocations",
                    index,
                });
            }
            let consistent = match source.kind {
                AllocationSourceKind::ScopedCounter => {
                    allocations += u128::from(source.allocations);
                    allocated_bytes += u128::from(source.allocated_bytes);
                    source.allocated_bytes.checked_sub(source.freed_bytes)
                        == Some(source.live_bytes)
                        && source.allocations.checked_sub(source.frees) == Some(source.live_buffers)
                        && source.live_bytes_high_water >= source.live_bytes
                        && source.live_bytes_high_water <= source.allocated_bytes
                        && source.live_buffers_high_water >= source.live_buffers
                        && source.live_buffers_high_water <= source.allocations
                }
                AllocationSourceKind::AllocationBudget => {
                    source.allocations == 0
                        && source.allocated_bytes == 0
                        && source.frees == 0
                        && source.freed_bytes == 0
                        && source.live_buffers == 0
                        && source.live_buffers_high_water == 0
                        && source.live_bytes_high_water >= source.live_bytes
                }
            };
            if !consistent {
                findings.push(Finding::AllocationAccounting { index });
            }
        }
        let node_allocations: u128 = self
            .phase_tree
            .nodes
            .iter()
            .map(|node| u128::from(node.allocations))
            .sum();
        let node_bytes: u128 = self
            .phase_tree
            .nodes
            .iter()
            .map(|node| u128::from(node.allocated_bytes))
            .sum();
        if (node_allocations, node_bytes) != (allocations, allocated_bytes) {
            findings.push(Finding::AllocationAccounting {
                index: narrow(self.allocations.sources.len()),
            });
        }
        if self.scheduling.workers == 0
            || self
                .scheduling
                .phase_workers
                .iter()
                .any(|entry| entry.workers == 0)
        {
            findings.push(Finding::SchedulingAccounting);
        }
        if !text::is_identity_text(&self.scheduling.workers_provenance)
            || self.scheduling.workers_provenance == text::INVALID_LABEL
        {
            findings.push(Finding::LabelNotPublic {
                section: "scheduling",
                index: 0,
            });
        }
        for (position, entry) in self.scheduling.phase_workers.iter().enumerate() {
            if entry.phase >= phases {
                findings.push(Finding::PhaseReference {
                    section: "scheduling",
                    index: narrow(position),
                });
            }
        }
    }

    fn platform_findings(&self, findings: &mut Vec<Finding>) {
        let address = &self.address_space;
        let ordered = match (address.soft_limit_bytes, address.hard_limit_bytes) {
            (Some(soft), Some(hard)) => soft <= hard,
            (Some(_) | None, None) => true,
            (None, Some(_)) => false,
        };
        if !ordered || address.enforced != address.soft_limit_bytes.is_some() {
            findings.push(Finding::AddressSpaceAccounting);
        }
        if !text::is_public_label(&address.source) {
            findings.push(Finding::LabelNotPublic {
                section: "address_space",
                index: 0,
            });
        }
        let process = &self.process;
        if !text::is_public_label(&process.thermal_source) {
            findings.push(Finding::LabelNotPublic {
                section: "process",
                index: 0,
            });
        }
        match process.peak_rss_source {
            PeakRssSource::KernelLifetimeHighWater => {
                if process.peak_rss_bytes == 0
                    || process.peak_rss_bytes < process.peak_rss_bytes_at_begin
                {
                    findings.push(Finding::ProcessAccounting);
                }
            }
            PeakRssSource::Unavailable => {
                if process.peak_rss_bytes != 0 || process.peak_rss_bytes_at_begin != 0 {
                    findings.push(Finding::ProcessAccounting);
                }
                findings.push(Finding::PeakRssUnavailable);
            }
        }
    }

    fn outcome_findings(&self, findings: &mut Vec<Finding>) {
        let failed = !self.failures.entries.is_empty() || self.failures.dropped > 0;
        let root = self.phase_tree.nodes.first();
        let consistent = match self.outcome {
            RunOutcome::Succeeded => !failed && root.is_none_or(|root| root.completed == 1),
            RunOutcome::Failed => failed && root.is_none_or(|root| root.completed == 1),
            RunOutcome::Abandoned => {
                root.is_none_or(|root| root.interrupted == 1 && root.unwound == 0)
            }
            RunOutcome::Unwound => root.is_none_or(|root| root.unwound == 1),
        };
        if !consistent {
            findings.push(Finding::OutcomeMismatch);
        }
        if self.failures.dropped > 0 {
            findings.push(Finding::FailuresDropped {
                dropped: self.failures.dropped,
            });
        }
        if self.outcome != RunOutcome::Succeeded {
            findings.push(Finding::RunNotSucceeded {
                outcome: self.outcome,
            });
        }
        let health = &self.recorder;
        if health.node_overflow_events > 0
            || health.span_overflow_events > 0
            || health.counter_overflow_events > 0
        {
            findings.push(Finding::RecorderOverflow);
        }
        if health.invalid_label_events > 0 {
            findings.push(Finding::LabelNotPublic {
                section: "recorder",
                index: 0,
            });
        }
        if health.clock_anomaly_events > 0 {
            findings.push(Finding::ClockAnomaly);
        }
    }

    /// Every reason this record is not an acceptable complete measurement.
    ///
    /// An empty result means: the schema's exact accounting holds, identity is
    /// complete, the run succeeded, nothing was dropped, and at most one
    /// percent of the root wall time is unattributed.
    pub fn findings(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if self.schema != RECORD_SCHEMA_V1 {
            findings.push(Finding::SchemaMismatch);
        }
        self.identity_findings(&mut findings);
        self.section_findings(&mut findings);
        if self.phase_tree.nodes.is_empty() {
            findings.push(Finding::TreeEmpty);
        }
        node_findings(&self.phase_tree.nodes, &mut findings);
        self.counter_findings(&mut findings);
        self.platform_findings(&mut findings);
        self.outcome_findings(&mut findings);
        if let Some(attribution) = self.attribution() {
            if !self
                .phase_tree
                .nodes
                .iter()
                .any(|node| node.parent == Some(0))
            {
                findings.push(Finding::NoPhases);
            } else if !attribution.within_limit() {
                findings.push(Finding::UnattributedExceedsLimit {
                    root_wall_ns: attribution.root_wall_ns,
                    unattributed_wall_ns: attribution.unattributed_wall_ns,
                });
            }
        }
        findings
    }
}

fn collect_unclassified(value: &Value, path: &str, classified: bool, found: &mut Vec<String>) {
    match value {
        Value::Number(_) if !classified => found.push(path.to_owned()),
        Value::Array(values) => {
            for (position, value) in values.iter().enumerate() {
                collect_unclassified(value, &format!("{path}[{position}]"), classified, found);
            }
        }
        Value::Object(map) => {
            let classified = classified || map.contains_key("classification");
            for (key, value) in map {
                let structural =
                    matches!(value, Value::Number(_)) && STRUCTURAL_NUMBER_KEYS.contains(&&**key);
                if !structural {
                    collect_unclassified(value, &format!("{path}.{key}"), classified, found);
                }
            }
        }
        _ => {}
    }
}

/// Paths of numbers in a JSON view that carry no classification.
///
/// A number is classified when it or an enclosing object has a
/// `classification` member. Structural numbers (phase indices and the process
/// identifier) are exempt. The result is empty for every record this crate
/// emits; SDK emitters in other languages are held to the same rule.
pub fn unclassified_numbers(view: &Value) -> Vec<String> {
    let mut found = Vec::new();
    collect_unclassified(view, "record", false, &mut found);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schema::{
            AllocationSource, ByteCounter, ByteKind, DeclaredQuantity, PhaseWorkers, RawFailure,
            ThermalState, Unit, WorkCounter,
        },
        test_support::sample_record,
    };

    type Change = fn(&mut MeasurementRecord);

    /// Coerce a list of non-capturing closures to function pointers.
    fn changes<T, const N: usize>(list: [fn(&mut T); N]) -> [fn(&mut T); N] {
        list
    }

    fn findings_after(change: impl FnOnce(&mut MeasurementRecord)) -> Vec<Finding> {
        let mut record = sample_record();
        change(&mut record);
        record.findings()
    }

    #[test]
    fn the_sample_record_has_no_findings() {
        assert_eq!(sample_record().findings(), Vec::new());
    }

    #[test]
    fn one_percent_limit_is_exact_integer_arithmetic() {
        let exact = Attribution {
            root_wall_ns: 1_000_000_000,
            unattributed_wall_ns: 10_000_000,
        };
        assert!(exact.within_limit());
        assert_eq!(exact.unattributed_parts_per_million(), 10_000);
        let over = Attribution {
            unattributed_wall_ns: 10_000_001,
            ..exact
        };
        assert!(!over.within_limit());
        assert_eq!(over.unattributed_parts_per_million(), 10_001);
        let empty = Attribution {
            root_wall_ns: 0,
            unattributed_wall_ns: 0,
        };
        assert!(!empty.within_limit());
        assert_eq!(empty.unattributed_parts_per_million(), 1_000_000);
        let huge = Attribution {
            root_wall_ns: u64::MAX,
            unattributed_wall_ns: u64::MAX / 100,
        };
        assert!(huge.within_limit());
        assert!(
            !Attribution {
                unattributed_wall_ns: u64::MAX / 100 + 1,
                ..huge
            }
            .within_limit()
        );
        assert_eq!((UNATTRIBUTED_NUMERATOR, UNATTRIBUTED_DENOMINATOR), (1, 100));
    }

    #[test]
    fn largest_undivided_phase_states_how_coarse_the_tree_is() {
        // In the sample tree the three calls of `transform` hold 600 ms that
        // no child phase divides, out of a 1 s root.
        let record = sample_record();
        let largest = record.largest_undivided_phase().unwrap();
        assert_eq!(
            largest,
            UndividedPhase {
                phase: 2,
                wall_exclusive_ns: 600_000_000,
                root_wall_ns: 1_000_000_000,
            }
        );
        assert_eq!(largest.share_parts_per_million(), 600_000);
        // A wrapper-only tree passes the one-percent rule and is reported as
        // one phase holding almost all of the root.
        let mut wrapper = sample_record();
        wrapper.phase_tree.nodes.truncate(2);
        wrapper.phase_tree.nodes[1].wall_inclusive_ns = 999_000_000;
        wrapper.phase_tree.nodes[1].wall_exclusive_ns = 999_000_000;
        assert!(wrapper.attribution().unwrap().within_limit());
        let coarse = wrapper.largest_undivided_phase().unwrap();
        assert_eq!(
            (coarse.phase, coarse.share_parts_per_million()),
            (1, 999_000)
        );
        // The earliest phase wins a tie; overlapping calls can exceed the root.
        let mut tied = sample_record();
        tied.phase_tree.nodes[4].wall_exclusive_ns = 600_000_000;
        assert_eq!(tied.largest_undivided_phase().unwrap().phase, 2);
        tied.phase_tree.nodes[4].wall_exclusive_ns = 1_600_000_000;
        let parallel = tied.largest_undivided_phase().unwrap();
        assert_eq!(
            (parallel.phase, parallel.share_parts_per_million()),
            (4, 1_600_000)
        );
        // No phase below the root, or no root at all, has nothing to report.
        let mut bare = sample_record();
        bare.phase_tree.nodes.truncate(1);
        assert_eq!(bare.largest_undivided_phase(), None);
        bare.phase_tree.nodes.clear();
        assert_eq!(bare.largest_undivided_phase(), None);
        assert_eq!(parts_per_million(1, 3), 333_334);
        assert_eq!(parts_per_million(0, 0), 1_000_000);
        assert_eq!(parts_per_million(u64::MAX, 1), u64::MAX);
    }

    #[test]
    fn unattributed_share_above_the_limit_rejects_the_report() {
        let findings = findings_after(|record| {
            // Move 2% of the root's wall time from its only sequential child
            // group into the root's own exclusive time.
            let nodes = &mut record.phase_tree.nodes;
            nodes[0].wall_inclusive_ns += 20_000_000;
            nodes[0].wall_exclusive_ns += 20_000_000;
        });
        assert_eq!(
            findings,
            vec![Finding::UnattributedExceedsLimit {
                root_wall_ns: 1_020_000_000,
                unattributed_wall_ns: 21_000_000,
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Rejected);
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes.truncate(1)),
            vec![
                Finding::WallAccounting { index: 0 },
                Finding::ThreadCpuAccounting { index: 0 },
                Finding::ProcessCpuAccounting { index: 0 },
                Finding::BoundaryAccounting { index: 0 },
                Finding::PhaseReference {
                    section: "byte_counters",
                    index: 0
                },
                Finding::PhaseReference {
                    section: "work_counters",
                    index: 0
                },
                Finding::PhaseReference {
                    section: "work_counters",
                    index: 1
                },
                Finding::AllocationAccounting { index: 2 },
                Finding::PhaseReference {
                    section: "scheduling",
                    index: 0
                },
                Finding::NoPhases,
            ]
        );
    }

    #[test]
    fn identity_must_be_complete_and_bound() {
        assert_eq!(
            findings_after(|record| record.identity.context = crate::RunContext::unbound()),
            vec![
                Finding::IdentityIncomplete {
                    field: "source_commit"
                },
                Finding::IdentityIncomplete { field: "artifact" },
                Finding::IdentityIncomplete { field: "profile" },
                Finding::IdentityIncomplete { field: "config" },
                Finding::IdentityIncomplete { field: "hardware" },
                Finding::DirtyDigestMismatch,
            ]
        );
        assert_eq!(
            findings_after(|record| record.identity.context.hardware = "two words".into()),
            vec![Finding::IdentityIncomplete { field: "hardware" }]
        );
        assert_eq!(
            findings_after(|record| record.identity.workload = text::INVALID_LABEL.into()),
            vec![Finding::IdentityIncomplete { field: "workload" }]
        );
        assert_eq!(
            findings_after(|record| record.identity.emitter = String::new()),
            vec![Finding::IdentityIncomplete { field: "emitter" }]
        );
        for change in changes::<MeasurementRecord, _>([
            |record: &mut MeasurementRecord| record.identity.context.source_dirty_digest = None,
            |record| record.identity.context.source_dirty = false,
            |record| record.identity.context.source_dirty_digest = Some("abc".into()),
        ]) {
            assert_eq!(findings_after(change), vec![Finding::DirtyDigestMismatch]);
        }
        assert_eq!(
            findings_after(|record| {
                record.identity.context.source_dirty = false;
                record.identity.context.source_dirty_digest = None;
            }),
            Vec::new()
        );
        assert_eq!(
            Finding::IdentityIncomplete { field: "config" }.severity(),
            Severity::Rejected
        );
    }

    #[test]
    fn schema_and_section_classifications_are_fixed() {
        assert_eq!(
            findings_after(|record| record.schema = "iroha.measurement.record.v2".into()),
            vec![Finding::SchemaMismatch]
        );
        let sections: [(&'static str, Change); 8] = [
            ("failures", |record| {
                record.failures.classification = Classification::Projection;
            }),
            ("phase_tree", |record| {
                record.phase_tree.classification = Classification::EngineeringTarget;
            }),
            ("byte_counters", |record| {
                record.byte_counters.classification = Classification::LocalScheduling;
            }),
            ("work_counters", |record| {
                record.work_counters.classification = Classification::EngineeringTarget;
            }),
            ("allocations", |record| {
                record.allocations.classification = Classification::Projection;
            }),
            ("process", |record| {
                record.process.classification = Classification::DeterministicConsensusBound;
            }),
            ("scheduling", |record| {
                record.scheduling.classification = Classification::Measured;
            }),
            ("address_space", |record| {
                record.address_space.classification = Classification::Measured;
            }),
        ];
        for (section, change) in sections {
            assert_eq!(
                findings_after(change),
                vec![Finding::SectionClassification { section }]
            );
        }
        assert_eq!(
            Finding::SectionClassification { section: "process" }.severity(),
            Severity::Malformed
        );
    }

    #[test]
    fn declared_quantities_need_a_non_measured_class_with_admitted_provenance() {
        let admitted = [
            (
                Classification::Projection,
                vec![ProvenanceKind::ModelProjection],
            ),
            (
                Classification::EngineeringTarget,
                vec![ProvenanceKind::PlanTarget],
            ),
            (
                Classification::DeterministicConsensusBound,
                vec![
                    ProvenanceKind::ProtocolConstant,
                    ProvenanceKind::CommittedState,
                ],
            ),
            (
                Classification::LocalScheduling,
                vec![
                    ProvenanceKind::LocalConfiguration,
                    ProvenanceKind::OperatingSystem,
                ],
            ),
            (Classification::Measured, vec![]),
        ];
        for (classification, kinds) in admitted {
            for kind in ProvenanceKind::ALL {
                let findings = findings_after(|record| {
                    record.declared = vec![DeclaredQuantity {
                        classification,
                        label: "bound".into(),
                        unit: Unit::Bytes,
                        value: 1,
                        provenance_kind: *kind,
                        provenance: "crates/a/src/lib.rs:BOUND".into(),
                    }];
                });
                let expected = if kinds.contains(kind) {
                    Vec::new()
                } else {
                    vec![Finding::DeclaredClassification { index: 0 }]
                };
                assert_eq!(findings, expected, "{classification:?} {kind:?}");
            }
        }
        assert_eq!(
            findings_after(|record| record.declared[0].provenance = "a label alone".into()),
            vec![Finding::LabelNotPublic {
                section: "declared",
                index: 0
            }]
        );
        assert_eq!(
            findings_after(|record| record.declared[1].label = String::new()),
            vec![Finding::LabelNotPublic {
                section: "declared",
                index: 1
            }]
        );
    }

    #[test]
    fn tree_shape_violations_are_malformed() {
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes.clear()),
            vec![
                Finding::TreeEmpty,
                Finding::PhaseReference {
                    section: "byte_counters",
                    index: 0
                },
                Finding::PhaseReference {
                    section: "byte_counters",
                    index: 1
                },
                Finding::PhaseReference {
                    section: "work_counters",
                    index: 0
                },
                Finding::PhaseReference {
                    section: "work_counters",
                    index: 1
                },
                Finding::AllocationAccounting { index: 2 },
                Finding::PhaseReference {
                    section: "scheduling",
                    index: 0
                },
            ]
        );
        assert!(
            findings_after(|record| record.phase_tree.nodes[0].parent = Some(0))
                .contains(&Finding::RootShape)
        );
        assert!(
            findings_after(|record| record.phase_tree.nodes[2].parent = None)
                .contains(&Finding::RootShape)
        );
        assert!(
            findings_after(|record| record.phase_tree.nodes[0].calls = 2)
                .contains(&Finding::RootShape)
        );
        assert!(
            findings_after(|record| record.phase_tree.nodes[1].parent = Some(3))
                .contains(&Finding::ParentOrder { index: 1 })
        );
        assert!(
            findings_after(|record| record.phase_tree.nodes[2].parent = Some(2))
                .contains(&Finding::ParentOrder { index: 2 })
        );
        assert!(
            findings_after(|record| {
                let duplicate = record.phase_tree.nodes[1].label.clone();
                record.phase_tree.nodes[3].label = duplicate;
            })
            .contains(&Finding::DuplicateSibling { index: 3 })
        );
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes[1].label = "has space".into()),
            vec![Finding::LabelNotPublic {
                section: "phase_tree",
                index: 1
            }]
        );
        assert_eq!(Finding::RootShape.severity(), Severity::Malformed);
    }

    #[test]
    fn call_accounting_is_exact() {
        for change in changes::<PhaseNode, _>([
            |node: &mut PhaseNode| node.completed += 1,
            |node| node.calls += 1,
            |node| node.unwound = node.interrupted + 1,
            |node| node.truncated = node.interrupted + 1,
            |node| {
                node.completed = u64::MAX;
                node.interrupted = 1;
            },
        ]) {
            assert_eq!(
                findings_after(|record| change(&mut record.phase_tree.nodes[2])),
                vec![Finding::CallAccounting { index: 2 }]
            );
        }
    }

    #[test]
    fn sequential_wall_time_must_equal_inclusive_minus_children_exactly() {
        // Node 1 has the sequential child 2: one nanosecond either way fails.
        for delta in [-1_i64, 1] {
            let findings = findings_after(|record| {
                let node = &mut record.phase_tree.nodes[1];
                node.wall_exclusive_ns = node.wall_exclusive_ns.wrapping_add_signed(delta);
            });
            assert_eq!(findings, vec![Finding::WallAccounting { index: 1 }]);
        }
        assert_eq!(
            findings_after(|record| {
                let node = &mut record.phase_tree.nodes[2];
                node.wall_exclusive_ns = node.wall_inclusive_ns + 1;
            }),
            vec![Finding::WallAccounting { index: 2 }]
        );
        // A leaf has no child to cover any of its time.
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes[2].wall_exclusive_ns -= 1),
            vec![Finding::WallAccounting { index: 2 }]
        );
    }

    #[test]
    fn parallel_wall_time_is_bounded_by_the_union_of_children() {
        // Node 3 has four overlapping calls of child 4: 4 x 100 ms inside a
        // 120 ms parent whose children cover 110 ms.
        assert_eq!(
            sample_record().phase_tree.nodes[3].peak_concurrent_children,
            4
        );
        // Covered time may not exceed the children's summed time ...
        assert_eq!(
            findings_after(|record| {
                record.phase_tree.nodes[4].wall_inclusive_ns = 100_000_000;
                record.phase_tree.nodes[4].wall_exclusive_ns = 100_000_000;
            }),
            vec![Finding::WallAccounting { index: 3 }]
        );
        // ... and the children may not exceed the peak times the covered time.
        assert_eq!(
            findings_after(|record| {
                record.phase_tree.nodes[4].wall_inclusive_ns = 441_000_000;
                record.phase_tree.nodes[4].wall_exclusive_ns = 441_000_000;
            }),
            vec![Finding::WallAccounting { index: 3 }]
        );
        assert_eq!(
            findings_after(|record| {
                record.phase_tree.nodes[4].wall_inclusive_ns = 440_000_000;
                record.phase_tree.nodes[4].wall_exclusive_ns = 440_000_000;
            }),
            Vec::new()
        );
        assert!(union_bound(110, 110, 1));
        assert!(!union_bound(110, 111, 1));
        assert!(!union_bound(110, 109, 4));
        assert!(union_bound(0, 0, 0));
        assert!(!union_bound(0, 1, 9));
        assert!(union_bound(u64::MAX, u128::from(u64::MAX) * 3, 3));
    }

    #[test]
    fn cpu_accounting_is_checked_for_both_clocks() {
        assert_eq!(
            findings_after(|record| {
                let node = &mut record.phase_tree.nodes[2];
                node.thread_cpu_exclusive_ns = node.thread_cpu_inclusive_ns + 1;
            }),
            vec![Finding::ThreadCpuAccounting { index: 2 }]
        );
        // A leaf cannot attribute opening-thread CPU to children it lacks.
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes[2].thread_cpu_exclusive_ns -= 1),
            vec![Finding::ThreadCpuAccounting { index: 2 }]
        );
        for delta in [-1_i64, 1] {
            assert_eq!(
                findings_after(|record| {
                    let node = &mut record.phase_tree.nodes[1];
                    node.process_cpu_window_exclusive_ns = node
                        .process_cpu_window_exclusive_ns
                        .wrapping_add_signed(delta);
                }),
                vec![Finding::ProcessCpuAccounting { index: 1 }]
            );
        }
    }

    #[test]
    fn boundary_observations_must_be_consistent() {
        for change in changes::<PhaseNode, _>([
            |node: &mut PhaseNode| node.peak_concurrent_children = 0,
            |node| node.load_milli_max = node.load_milli_first_enter - 1,
            |node| node.load_milli_last_exit = node.load_milli_max + 1,
            |node| node.active_threads_max = 0,
        ]) {
            let findings = findings_after(|record| change(&mut record.phase_tree.nodes[1]));
            assert!(
                findings.contains(&Finding::BoundaryAccounting { index: 1 }),
                "{findings:?}"
            );
        }
        // More concurrent children than child calls is impossible.
        assert!(
            findings_after(|record| record.phase_tree.nodes[1].peak_concurrent_children = 4)
                .contains(&Finding::BoundaryAccounting { index: 1 })
        );
        // A leaf cannot have had concurrent children.
        assert!(
            findings_after(|record| record.phase_tree.nodes[2].peak_concurrent_children = 1)
                .contains(&Finding::BoundaryAccounting { index: 2 })
        );
    }

    #[test]
    fn counters_must_reference_phases_and_balance() {
        assert_eq!(
            findings_after(|record| record.byte_counters.entries[0].phase = 99),
            vec![Finding::PhaseReference {
                section: "byte_counters",
                index: 0
            }]
        );
        for change in changes::<ByteCounter, _>([
            |counter: &mut ByteCounter| counter.count = 0,
            |counter| counter.min_bytes = counter.max_bytes + 1,
            |counter| counter.total_bytes = counter.count * counter.max_bytes + 1,
            |counter| counter.total_bytes = counter.count * counter.min_bytes - 1,
        ]) {
            let findings = findings_after(|record| change(&mut record.byte_counters.entries[1]));
            assert!(
                findings.contains(&Finding::ByteCounterAccounting { index: 1 }),
                "{findings:?}"
            );
        }
        assert_eq!(
            findings_after(|record| record.byte_counters.entries.push(ByteCounter {
                phase: 0,
                kind: ByteKind::Key,
                label: "not public".into(),
                count: 1,
                total_bytes: 32,
                min_bytes: 32,
                max_bytes: 32,
            })),
            vec![Finding::LabelNotPublic {
                section: "byte_counters",
                index: 2
            }]
        );
        assert_eq!(
            findings_after(|record| record.work_counters.entries[1].phase = 5),
            vec![Finding::PhaseReference {
                section: "work_counters",
                index: 1
            }]
        );
        for change in changes::<WorkCounter, _>([
            |counter: &mut WorkCounter| counter.count = 0,
            |counter| counter.min_units = counter.max_units + 1,
            |counter| counter.total_units = counter.count * counter.max_units + 1,
            |counter| counter.total_units = counter.count * counter.min_units - 1,
        ]) {
            assert_eq!(
                findings_after(|record| change(&mut record.work_counters.entries[0])),
                vec![Finding::WorkCounterAccounting { index: 0 }]
            );
        }
        assert_eq!(
            findings_after(|record| record.work_counters.entries.push(WorkCounter {
                phase: 0,
                label: "0xdeadbeef witness".into(),
                count: 1,
                total_units: 0,
                min_units: 0,
                max_units: 0,
            })),
            vec![Finding::LabelNotPublic {
                section: "work_counters",
                index: 2
            }]
        );
        assert_eq!(
            Finding::WorkCounterAccounting { index: 0 }.severity(),
            Severity::Malformed
        );
        assert_eq!(
            findings_after(|record| record.failures.entries.push(RawFailure {
                phase: 99,
                stage: "bad stage".into(),
                code: "code".into(),
            })),
            vec![
                Finding::PhaseReference {
                    section: "failures",
                    index: 0
                },
                Finding::LabelNotPublic {
                    section: "failures",
                    index: 0
                },
                Finding::OutcomeMismatch,
            ]
        );
    }

    #[test]
    fn allocation_sources_and_phase_totals_must_balance() {
        for change in changes::<AllocationSource, _>([
            |source: &mut AllocationSource| source.live_bytes += 1,
            |source| source.freed_bytes = source.allocated_bytes + 1,
            |source| source.live_buffers += 1,
            |source| source.live_bytes_high_water = source.live_bytes - 1,
            |source| source.live_bytes_high_water = source.allocated_bytes + 1,
            |source| source.live_buffers_high_water = source.allocations + 1,
        ]) {
            let findings = findings_after(|record| change(&mut record.allocations.sources[0]));
            assert_eq!(findings, vec![Finding::AllocationAccounting { index: 0 }]);
        }
        for change in changes::<AllocationSource, _>([
            |source: &mut AllocationSource| source.allocations = 1,
            |source| source.live_buffers_high_water = 1,
            |source| source.live_bytes_high_water = source.live_bytes - 1,
        ]) {
            let findings = findings_after(|record| change(&mut record.allocations.sources[1]));
            assert_eq!(findings, vec![Finding::AllocationAccounting { index: 1 }]);
        }
        // Per-phase allocation totals must equal the scoped counters' totals.
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes[2].allocations += 1),
            vec![Finding::AllocationAccounting { index: 2 }]
        );
        assert_eq!(
            findings_after(|record| record.phase_tree.nodes[2].allocated_bytes -= 1),
            vec![Finding::AllocationAccounting { index: 2 }]
        );
        assert_eq!(
            findings_after(|record| record.allocations.sources[1].label = "a b".into()),
            vec![Finding::LabelNotPublic {
                section: "allocations",
                index: 1
            }]
        );
    }

    #[test]
    fn scheduling_and_platform_sections_are_checked() {
        assert_eq!(
            findings_after(|record| record.scheduling.workers = 0),
            vec![Finding::SchedulingAccounting]
        );
        assert_eq!(
            findings_after(|record| record.scheduling.phase_workers[0].workers = 0),
            vec![Finding::SchedulingAccounting]
        );
        assert_eq!(
            findings_after(|record| record.scheduling.phase_workers.push(PhaseWorkers {
                phase: 5,
                workers: 2
            })),
            vec![Finding::PhaseReference {
                section: "scheduling",
                index: 1
            }]
        );
        assert_eq!(
            findings_after(|record| record.scheduling.workers_provenance = "a b".into()),
            vec![Finding::LabelNotPublic {
                section: "scheduling",
                index: 0
            }]
        );
        for change in changes::<MeasurementRecord, _>([
            |record: &mut MeasurementRecord| record.address_space.enforced = false,
            |record| record.address_space.soft_limit_bytes = Some(u64::MAX),
            |record| {
                record.address_space.soft_limit_bytes = None;
                record.address_space.enforced = false;
            },
        ]) {
            assert_eq!(
                findings_after(change),
                vec![Finding::AddressSpaceAccounting]
            );
        }
        assert_eq!(
            findings_after(|record| {
                record.address_space.soft_limit_bytes = None;
                record.address_space.hard_limit_bytes = None;
                record.address_space.enforced = false;
            }),
            Vec::new()
        );
        assert_eq!(
            findings_after(|record| record.address_space.source = "a b".into()),
            vec![Finding::LabelNotPublic {
                section: "address_space",
                index: 0
            }]
        );
        assert_eq!(
            findings_after(|record| record.process.thermal_source = String::new()),
            vec![Finding::LabelNotPublic {
                section: "process",
                index: 0
            }]
        );
        assert_eq!(
            findings_after(|record| record.process.peak_rss_bytes = 0),
            vec![Finding::ProcessAccounting]
        );
        assert_eq!(
            findings_after(|record| {
                record.process.peak_rss_bytes_at_begin = record.process.peak_rss_bytes + 1;
            }),
            vec![Finding::ProcessAccounting]
        );
        assert_eq!(
            findings_after(|record| record.process.peak_rss_source = PeakRssSource::Unavailable),
            vec![Finding::ProcessAccounting, Finding::PeakRssUnavailable]
        );
        assert_eq!(
            findings_after(|record| {
                record.process.peak_rss_source = PeakRssSource::Unavailable;
                record.process.peak_rss_bytes = 0;
                record.process.peak_rss_bytes_at_begin = 0;
            }),
            vec![Finding::PeakRssUnavailable]
        );
        // An unavailable thermal state is an honest observation, not a finding.
        assert_eq!(
            findings_after(|record| {
                record.process.thermal_begin = ThermalState::Unavailable;
                record.process.thermal_finish = ThermalState::Unavailable;
                record.process.thermal_source = "unavailable".into();
            }),
            Vec::new()
        );
    }

    #[test]
    fn outcome_must_agree_with_failures_and_root_completion() {
        let failure = || RawFailure {
            phase: 1,
            stage: "commit".into(),
            code: "refused".into(),
        };
        assert_eq!(
            findings_after(|record| record.failures.entries.push(failure())),
            vec![Finding::OutcomeMismatch]
        );
        assert_eq!(
            findings_after(|record| record.outcome = RunOutcome::Failed),
            vec![
                Finding::OutcomeMismatch,
                Finding::RunNotSucceeded {
                    outcome: RunOutcome::Failed
                }
            ]
        );
        assert_eq!(
            findings_after(|record| {
                record.outcome = RunOutcome::Failed;
                record.failures.entries.push(failure());
            }),
            vec![Finding::RunNotSucceeded {
                outcome: RunOutcome::Failed
            }]
        );
        assert_eq!(
            findings_after(|record| {
                record.outcome = RunOutcome::Failed;
                record.failures.dropped = 7;
            }),
            vec![
                Finding::FailuresDropped { dropped: 7 },
                Finding::RunNotSucceeded {
                    outcome: RunOutcome::Failed
                }
            ]
        );
        let interrupt = |record: &mut MeasurementRecord, unwound: u64| {
            let root = &mut record.phase_tree.nodes[0];
            root.completed = 0;
            root.interrupted = 1;
            root.unwound = unwound;
        };
        assert_eq!(
            findings_after(|record| {
                record.outcome = RunOutcome::Abandoned;
                interrupt(record, 0);
            }),
            vec![Finding::RunNotSucceeded {
                outcome: RunOutcome::Abandoned
            }]
        );
        assert_eq!(
            findings_after(|record| {
                record.outcome = RunOutcome::Unwound;
                interrupt(record, 1);
            }),
            vec![Finding::RunNotSucceeded {
                outcome: RunOutcome::Unwound
            }]
        );
        assert!(
            findings_after(|record| record.outcome = RunOutcome::Unwound)
                .contains(&Finding::OutcomeMismatch)
        );
        assert!(
            findings_after(|record| {
                record.outcome = RunOutcome::Abandoned;
                interrupt(record, 1);
            })
            .contains(&Finding::OutcomeMismatch)
        );
        assert!(findings_after(|record| interrupt(record, 0)).contains(&Finding::OutcomeMismatch));
        assert_eq!(
            Finding::RunNotSucceeded {
                outcome: RunOutcome::Failed
            }
            .severity(),
            Severity::Rejected
        );
        assert_eq!(Finding::OutcomeMismatch.severity(), Severity::Malformed);
    }

    #[test]
    fn recorder_health_rejects_overflow_replaced_labels_and_clock_anomalies() {
        for change in changes::<MeasurementRecord, _>([
            |record: &mut MeasurementRecord| record.recorder.node_overflow_events = 1,
            |record| record.recorder.span_overflow_events = 1,
            |record| record.recorder.counter_overflow_events = 1,
        ]) {
            assert_eq!(findings_after(change), vec![Finding::RecorderOverflow]);
        }
        assert_eq!(
            findings_after(|record| record.recorder.invalid_label_events = 1),
            vec![Finding::LabelNotPublic {
                section: "recorder",
                index: 0
            }]
        );
        assert_eq!(
            findings_after(|record| record.recorder.clock_anomaly_events = 1),
            vec![Finding::ClockAnomaly]
        );
        // Stale guards, stale parents and forced closes are fully accounted
        // in the tree and are reported without rejecting the record.
        assert_eq!(
            findings_after(|record| {
                record.recorder.stale_guard_events = 3;
                record.recorder.stale_parent_events = 2;
                record.recorder.forced_closes = 1;
            }),
            Vec::new()
        );
    }

    fn member<'a>(value: &'a mut Value, key: &Value) -> &'a mut Value {
        match (value, key) {
            (Value::Object(map), Value::String(name)) => {
                map.entry(name.clone()).or_insert(Value::Null)
            }
            (Value::Array(values), key) => {
                let index = usize::try_from(key.as_u64().expect("list index")).unwrap();
                &mut values[index]
            }
            _ => panic!("mutation path does not match the record"),
        }
    }

    fn walk<'a>(value: &'a mut Value, path: &[Value]) -> &'a mut Value {
        path.iter().fold(value, member)
    }

    /// Apply one mutation case of `fixtures/record_mutations_v1.json`.
    fn mutate(view: &mut Value, case: &norito::json::Map) {
        let operations = |name: &str| {
            case.get(name)
                .map(|list| list.as_array().expect("operation list").clone())
                .unwrap_or_default()
        };
        for operation in operations("set") {
            let operation = operation.as_object().unwrap();
            let path = operation["path"].as_array().unwrap();
            *walk(view, path) = operation["value"].clone();
        }
        for operation in operations("append") {
            let operation = operation.as_object().unwrap();
            let path = operation["path"].as_array().unwrap();
            match walk(view, path) {
                Value::Array(values) => values.push(operation["value"].clone()),
                _ => panic!("append target is not a list"),
            }
        }
        for operation in operations("remove") {
            let path = operation.as_array().unwrap();
            let (last, parents) = path.split_last().unwrap();
            match walk(view, parents) {
                Value::Object(map) => {
                    map.remove(last.as_str().unwrap())
                        .expect("removed key exists");
                }
                _ => panic!("remove target is not an object"),
            }
        }
    }

    #[test]
    fn finding_codes_are_stable_and_distinct() {
        let all = [
            Finding::SchemaMismatch,
            Finding::IdentityIncomplete { field: "config" },
            Finding::DirtyDigestMismatch,
            Finding::LabelNotPublic {
                section: "declared",
                index: 2,
            },
            Finding::SectionClassification { section: "process" },
            Finding::DeclaredClassification { index: 1 },
            Finding::TreeEmpty,
            Finding::RootShape,
            Finding::ParentOrder { index: 4 },
            Finding::DuplicateSibling { index: 3 },
            Finding::CallAccounting { index: 2 },
            Finding::WallAccounting { index: 1 },
            Finding::ThreadCpuAccounting { index: 1 },
            Finding::ProcessCpuAccounting { index: 1 },
            Finding::BoundaryAccounting { index: 1 },
            Finding::PhaseReference {
                section: "failures",
                index: 0,
            },
            Finding::ByteCounterAccounting { index: 0 },
            Finding::WorkCounterAccounting { index: 4 },
            Finding::AllocationAccounting { index: 0 },
            Finding::AddressSpaceAccounting,
            Finding::ProcessAccounting,
            Finding::SchedulingAccounting,
            Finding::OutcomeMismatch,
            Finding::FailuresDropped { dropped: 7 },
            Finding::RunNotSucceeded {
                outcome: RunOutcome::Unwound,
            },
            Finding::RecorderOverflow,
            Finding::ClockAnomaly,
            Finding::PeakRssUnavailable,
            Finding::NoPhases,
            Finding::UnattributedExceedsLimit {
                root_wall_ns: 100,
                unattributed_wall_ns: 2,
            },
        ];
        let codes: Vec<_> = all.iter().map(|finding| finding.code()).collect();
        let mut distinct = codes.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), all.len());
        assert_eq!(codes[1], "identity_incomplete:config");
        assert_eq!(codes[3], "label_not_public:declared:2");
        assert_eq!(codes[15], "phase_reference:failures:0");
        assert_eq!(codes[17], "work_counter_accounting:4");
        assert_eq!(codes[23], "failures_dropped:7");
        assert_eq!(codes[24], "run_not_succeeded:unwound");
        assert_eq!(codes[29], "unattributed_exceeds_limit:2:100");
        for code in codes {
            assert!(
                code.bytes().all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b':')),
                "{code}"
            );
        }
    }

    #[test]
    fn shared_mutation_fixture_yields_exactly_the_listed_finding_codes() {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let read = |name: &str| {
            norito::json::parse_value(&std::fs::read_to_string(fixtures.join(name)).unwrap())
                .unwrap()
        };
        let table = read("record_mutations_v1.json");
        let table = table.as_object().unwrap();
        assert_eq!(
            table["schema"].as_str(),
            Some("iroha.measurement.record_mutations.v1")
        );
        let base = read(table["base"].as_str().unwrap());
        let cases = table["cases"].as_array().unwrap();
        assert!(cases.len() >= 40, "{}", cases.len());
        let mut names = Vec::new();
        let mut seen_codes = std::collections::BTreeSet::new();
        for case in cases {
            let case = case.as_object().unwrap();
            let name = case["name"].as_str().unwrap();
            assert!(!names.contains(&name), "duplicate case {name}");
            names.push(name);
            let mut view = base.clone();
            mutate(&mut view, case);
            let decoded = MeasurementRecord::from_json_value(&view);
            if case.get("decode_error").and_then(Value::as_bool) == Some(true) {
                assert!(decoded.is_err(), "{name}: must not decode");
                assert!(!case.contains_key("findings"), "{name}");
                continue;
            }
            let record = decoded.unwrap_or_else(|error| panic!("{name}: {error}"));
            let codes: Vec<_> = record.findings().iter().map(|f| f.code()).collect();
            let expected: Vec<_> = case["findings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|code| code.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(codes, expected, "{name}");
            seen_codes.extend(
                codes
                    .iter()
                    .map(|code| code.split(':').next().unwrap().to_owned()),
            );
        }
        // The shared fixture exercises every kind of finding a decodable JSON
        // view can produce.
        for kind in [
            "schema_mismatch",
            "identity_incomplete",
            "dirty_digest_mismatch",
            "label_not_public",
            "section_classification",
            "declared_classification",
            "tree_empty",
            "root_shape",
            "parent_order",
            "duplicate_sibling",
            "call_accounting",
            "wall_accounting",
            "thread_cpu_accounting",
            "process_cpu_accounting",
            "boundary_accounting",
            "phase_reference",
            "byte_counter_accounting",
            "work_counter_accounting",
            "allocation_accounting",
            "address_space_accounting",
            "process_accounting",
            "scheduling_accounting",
            "outcome_mismatch",
            "failures_dropped",
            "run_not_succeeded",
            "recorder_overflow",
            "clock_anomaly",
            "peak_rss_unavailable",
            "no_phases",
            "unattributed_exceeds_limit",
        ] {
            assert!(seen_codes.contains(kind), "no mutation case yields {kind}");
        }
    }

    #[test]
    fn every_number_in_the_json_view_is_classified_or_structural() {
        let view = sample_record().to_json_value();
        assert_eq!(unclassified_numbers(&view), Vec::<String>::new());
        let mut stripped = view.clone();
        stripped
            .as_object_mut()
            .unwrap()
            .get_mut("phase_tree")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("classification");
        let unclassified = unclassified_numbers(&stripped);
        // 5 phases with 19 quantitative fields each; `parent` is structural.
        assert_eq!(unclassified.len(), 5 * 19);
        assert!(unclassified.contains(&"record.phase_tree.nodes[0].wall_inclusive_ns".to_owned()));
        assert!(unclassified.iter().all(|path| !path.ends_with(".parent")));
        let mut extra = view;
        extra
            .as_object_mut()
            .unwrap()
            .insert("budget".into(), Value::from(7_u64));
        assert_eq!(
            unclassified_numbers(&extra),
            vec!["record.budget".to_owned()]
        );
        assert_eq!(narrow(3), 3);
        assert_eq!(narrow(usize::MAX), u32::MAX);
    }
}
