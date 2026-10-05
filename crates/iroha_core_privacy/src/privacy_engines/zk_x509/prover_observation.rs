//! Bounded public observations for isolated native prover diagnostics.
//!
//! Collection is explicitly scoped to the calling test thread. It never
//! records field values, source bytes, random masks or witness-dependent row
//! positions. Durations of nested phases overlap and must not be summed.
//!
//! Every timer also opens a phase in the shared `iroha_measurement` tree of
//! its observation, so the same instrumentation yields inclusive and exclusive
//! times and the unattributed share. The public geometry counters and the
//! proof size are recorded in the same record as work and byte counters. The
//! record carries only the static labels below and never returns a value to
//! the prover.
//!
//! When a harness directory is given, the session itself writes the record
//! there, so a producer that fails, returns early or unwinds still leaves a
//! failed, abandoned or unwound record for the harness.

use core::cell::RefCell;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use iroha_measurement::{
    ByteKind, CollectingSink, DirectoryReport, DirectorySink, FlowKind, MeasurementRecord,
    PhaseGuard, RecordSink, RunContext, RunIdentity, Session, TeeSink, WorkerDeclaration,
};

/// Fixed public implementation phases; the receipt cannot grow with the witness.
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub(super) enum PhaseV1 {
    Preparation,
    Assembly,
    BaseSources,
    BaseSampleAndCommit,
    CompactCa,
    BoundSources,
    DerBinding,
    RfcBinding,
    AuxSampleAndCommit,
    Composition,
    DeepAndFri,
    QueryOpenings,
    EnvelopeAndSelfCheck,
    SampleSourceColumns,
    SampleMaskDraws,
    CompositionRegistration,
    CompositionTraceCache,
    CompositionDenominators,
    CompositionBaseReplay,
    CompositionAuxReplay,
    CompositionFixedReplay,
    CompositionResiduesAndFold,
    CompositionInverseTransform,
    CompositionDegreeChunks,
    InitialJoinedSourceBatch,
    InitialJoinedTransform,
    QueryJoinedSourceBatch,
    QueryJoinedTransform,
    CompositionArithmeticFixedRows,
    CompositionArithmeticFixedInverseTransform,
    CompositionProviders,
    CompositionRegisteredProviders,
    CompositionRegistrationFold,
    CompositionTerminalLinks,
    CompositionKeyLinks,
    CompositionShaUnion,
    CompositionCaRetention,
    CompositionCaLinks,
    CompositionBlinding,
    CompositionFp4Evaluations,
    CompositionCommitment,
    SourceByteMemoryBase,
    SourceByteMemoryAux,
    SourceStrictDerBase,
    SourceStrictDerAux,
    SourceRfc5280Base,
    SourceRfc5280Aux,
    SourceSha256CallBusBase,
    SourceSha256CallBusAux,
    SourceCaAccumulatorBase,
    SourceCaAccumulatorAux,
    SourceProjectionBase,
    SourceProjectionAux,
    SourceP256ArithmeticBase,
    SourceP256ArithmeticAux,
    SourceP256ReductionBase,
    SourceP256ReductionAux,
    SourceP256LowSBase,
    SourceP256LowSAux,
    SourceP256WindowBase,
    SourceP256WindowAux,
    SourceP256ValueBusBase,
    SourceP256ValueBusAux,
    SourceP256ScalarBitBusBase,
    SourceP256ScalarBitBusAux,
}
const PHASES: [PhaseV1; 65] = [
    PhaseV1::Preparation,
    PhaseV1::Assembly,
    PhaseV1::BaseSources,
    PhaseV1::BaseSampleAndCommit,
    PhaseV1::CompactCa,
    PhaseV1::BoundSources,
    PhaseV1::DerBinding,
    PhaseV1::RfcBinding,
    PhaseV1::AuxSampleAndCommit,
    PhaseV1::Composition,
    PhaseV1::DeepAndFri,
    PhaseV1::QueryOpenings,
    PhaseV1::EnvelopeAndSelfCheck,
    PhaseV1::SampleSourceColumns,
    PhaseV1::SampleMaskDraws,
    PhaseV1::CompositionRegistration,
    PhaseV1::CompositionTraceCache,
    PhaseV1::CompositionDenominators,
    PhaseV1::CompositionBaseReplay,
    PhaseV1::CompositionAuxReplay,
    PhaseV1::CompositionFixedReplay,
    PhaseV1::CompositionResiduesAndFold,
    PhaseV1::CompositionInverseTransform,
    PhaseV1::CompositionDegreeChunks,
    PhaseV1::InitialJoinedSourceBatch,
    PhaseV1::InitialJoinedTransform,
    PhaseV1::QueryJoinedSourceBatch,
    PhaseV1::QueryJoinedTransform,
    PhaseV1::CompositionArithmeticFixedRows,
    PhaseV1::CompositionArithmeticFixedInverseTransform,
    PhaseV1::CompositionProviders,
    PhaseV1::CompositionRegisteredProviders,
    PhaseV1::CompositionRegistrationFold,
    PhaseV1::CompositionTerminalLinks,
    PhaseV1::CompositionKeyLinks,
    PhaseV1::CompositionShaUnion,
    PhaseV1::CompositionCaRetention,
    PhaseV1::CompositionCaLinks,
    PhaseV1::CompositionBlinding,
    PhaseV1::CompositionFp4Evaluations,
    PhaseV1::CompositionCommitment,
    PhaseV1::SourceByteMemoryBase,
    PhaseV1::SourceByteMemoryAux,
    PhaseV1::SourceStrictDerBase,
    PhaseV1::SourceStrictDerAux,
    PhaseV1::SourceRfc5280Base,
    PhaseV1::SourceRfc5280Aux,
    PhaseV1::SourceSha256CallBusBase,
    PhaseV1::SourceSha256CallBusAux,
    PhaseV1::SourceCaAccumulatorBase,
    PhaseV1::SourceCaAccumulatorAux,
    PhaseV1::SourceProjectionBase,
    PhaseV1::SourceProjectionAux,
    PhaseV1::SourceP256ArithmeticBase,
    PhaseV1::SourceP256ArithmeticAux,
    PhaseV1::SourceP256ReductionBase,
    PhaseV1::SourceP256ReductionAux,
    PhaseV1::SourceP256LowSBase,
    PhaseV1::SourceP256LowSAux,
    PhaseV1::SourceP256WindowBase,
    PhaseV1::SourceP256WindowAux,
    PhaseV1::SourceP256ValueBusBase,
    PhaseV1::SourceP256ValueBusAux,
    PhaseV1::SourceP256ScalarBitBusBase,
    PhaseV1::SourceP256ScalarBitBusAux,
];

/// Static public labels of [`PHASES`] in the shared phase tree, in that order.
const PHASE_LABELS_V1: [&str; PHASES.len()] = [
    "Preparation",
    "Assembly",
    "BaseSources",
    "BaseSampleAndCommit",
    "CompactCa",
    "BoundSources",
    "DerBinding",
    "RfcBinding",
    "AuxSampleAndCommit",
    "Composition",
    "DeepAndFri",
    "QueryOpenings",
    "EnvelopeAndSelfCheck",
    "SampleSourceColumns",
    "SampleMaskDraws",
    "CompositionRegistration",
    "CompositionTraceCache",
    "CompositionDenominators",
    "CompositionBaseReplay",
    "CompositionAuxReplay",
    "CompositionFixedReplay",
    "CompositionResiduesAndFold",
    "CompositionInverseTransform",
    "CompositionDegreeChunks",
    "InitialJoinedSourceBatch",
    "InitialJoinedTransform",
    "QueryJoinedSourceBatch",
    "QueryJoinedTransform",
    "CompositionArithmeticFixedRows",
    "CompositionArithmeticFixedInverseTransform",
    "CompositionProviders",
    "CompositionRegisteredProviders",
    "CompositionRegistrationFold",
    "CompositionTerminalLinks",
    "CompositionKeyLinks",
    "CompositionShaUnion",
    "CompositionCaRetention",
    "CompositionCaLinks",
    "CompositionBlinding",
    "CompositionFp4Evaluations",
    "CompositionCommitment",
    "SourceByteMemoryBase",
    "SourceByteMemoryAux",
    "SourceStrictDerBase",
    "SourceStrictDerAux",
    "SourceRfc5280Base",
    "SourceRfc5280Aux",
    "SourceSha256CallBusBase",
    "SourceSha256CallBusAux",
    "SourceCaAccumulatorBase",
    "SourceCaAccumulatorAux",
    "SourceProjectionBase",
    "SourceProjectionAux",
    "SourceP256ArithmeticBase",
    "SourceP256ArithmeticAux",
    "SourceP256ReductionBase",
    "SourceP256ReductionAux",
    "SourceP256LowSBase",
    "SourceP256LowSAux",
    "SourceP256WindowBase",
    "SourceP256WindowAux",
    "SourceP256ValueBusBase",
    "SourceP256ValueBusAux",
    "SourceP256ScalarBitBusBase",
    "SourceP256ScalarBitBusAux",
];

/// Workload label of the X509 credential prover in the shared phase tree.
const MEASUREMENT_WORKLOAD_V1: &str = "zk_x509_credential_prove";
/// Root phase of one observation: everything between begin and finish.
const MEASUREMENT_ROOT_V1: &str = "zk_x509_observation";
/// Adapter identity written into every record this site emits.
const MEASUREMENT_EMITTER_V1: &str = "rust.iroha_core_privacy.zk_x509";
/// File stem of the records this site writes into a harness directory.
const MEASUREMENT_RECORD_STEM_V1: &str = "zk_x509-phase-tree";

#[derive(Clone, Copy, Default)]
struct PhaseCountV1 {
    calls: u64,
    completed: u64,
    interrupted: u64,
    unwound: u64,
    elapsed: Duration,
}

/// Public counters observed from completed MAIN transform calls, separated by use.
pub(super) struct ReceiptV1 {
    // Identity prevents an accidentally long-lived timer from writing into a
    // later diagnostic on this same thread. The token contains no data.
    scope: std::rc::Rc<()>,
    phases: [PhaseCountV1; PHASES.len()],
    // Index zero means the admitted CPU path; 1..=8 are maximum Metal
    // columns per device submission admitted by the unchanged payload plan.
    policies: [u64; 9],
    cpu_calls: u64,
    cpu_columns: u64,
    metal_calls: u64,
    metal_columns: u64,
    failures: u64,
    fixed_backend_columns: [[u64; 2]; 2],
    quotient_backend_columns: [u64; 2],
    native_replay_backend_columns: [u64; 2],
    fixed_failures: u64,
    fixed_forward_columns: u64,
    fixed_inverse_columns: u64,
    fixed_forward_butterflies: u64,
    fixed_inverse_butterflies: u64,
    // Shared phase tree: live while the observation collects, then the
    // finished public record. Boxed so the receipt itself stays small.
    session: Option<Session>,
    sink: CollectingSink,
    measurement: Option<Box<MeasurementRecord>>,
    // What the session wrote into the harness directory, if one was given.
    harness: Option<DirectoryReport>,
}
impl Default for ReceiptV1 {
    fn default() -> Self {
        Self {
            session: None,
            sink: CollectingSink::new(),
            measurement: None,
            harness: None,
            scope: std::rc::Rc::new(()),
            phases: [PhaseCountV1::default(); PHASES.len()],
            policies: [0; 9],
            cpu_calls: 0,
            cpu_columns: 0,
            metal_calls: 0,
            metal_columns: 0,
            failures: 0,
            fixed_backend_columns: [[0; 2]; 2],
            quotient_backend_columns: [0; 2],
            native_replay_backend_columns: [0; 2],
            fixed_failures: 0,
            fixed_forward_columns: 0,
            fixed_inverse_columns: 0,
            fixed_forward_butterflies: 0,
            fixed_inverse_butterflies: 0,
        }
    }
}
thread_local! {
    static ACTIVE: RefCell<Option<ReceiptV1>> = const { RefCell::new(None) };
}

/// One diagnostic owns collection; unrelated parallel tests remain unobserved.
pub(super) struct ObservationV1(core::marker::PhantomData<std::rc::Rc<()>>);
impl ObservationV1 {
    /// Observe without a harness: the record's identity stays unbound.
    pub(super) fn begin_v1() -> Self {
        Self::begin_with_context_v1(RunContext::unbound())
    }
    /// Observe a run whose source, artifact, profile, configuration, hardware
    /// and cache identity the harness supplied before the prover starts.
    pub(super) fn begin_with_context_v1(context: RunContext) -> Self {
        Self::begin_with_harness_v1(context, None)
    }
    /// Observe a run and have the session itself write its record into the
    /// harness directory when it ends, however it ends: an observation that is
    /// dropped on an error return or during an unwind still leaves an
    /// abandoned or unwound record there.
    pub(super) fn begin_with_harness_v1(
        context: RunContext,
        harness_directory: Option<PathBuf>,
    ) -> Self {
        ACTIVE.with(|active| {
            assert!(active.borrow().is_none(), "nested prover observation");
            let mut receipt = ReceiptV1::default();
            let memory: Box<dyn RecordSink> = Box::new(receipt.sink.clone());
            let sink = match harness_directory {
                Some(directory) => {
                    let files = DirectorySink::new(directory, MEASUREMENT_RECORD_STEM_V1);
                    receipt.harness = Some(files.report());
                    Box::new(TeeSink::new(memory, Box::new(files)))
                }
                None => memory,
            };
            receipt.session = Some(Session::begin(
                RunIdentity::new(
                    context,
                    MEASUREMENT_WORKLOAD_V1,
                    FlowKind::Proof,
                    MEASUREMENT_EMITTER_V1,
                ),
                MEASUREMENT_ROOT_V1,
                WorkerDeclaration {
                    workers: u32::try_from(rayon::current_num_threads()).unwrap_or(u32::MAX),
                    provenance: "rayon.current_num_threads",
                },
                sink,
            ));
            *active.borrow_mut() = Some(receipt);
        });
        Self(core::marker::PhantomData)
    }
    /// Record the size of the encoded public proof. Only the length is taken.
    pub(super) fn record_proof_bytes_v1(&self, bytes: u64) {
        ACTIVE.with(|active| {
            if let Some(session) = active
                .borrow()
                .as_ref()
                .and_then(|receipt| receipt.session.as_ref())
            {
                session.record_bytes(ByteKind::Proof, "credential_proof", bytes);
            }
        });
    }
    /// Retain a raw producer failure in the shared record. Both arguments are
    /// literals: an error payload cannot be recorded.
    pub(super) fn record_failure_v1(&self, stage: &'static str, code: &'static str) {
        ACTIVE.with(|active| {
            if let Some(session) = active
                .borrow()
                .as_ref()
                .and_then(|receipt| receipt.session.as_ref())
            {
                session.record_failure(stage, code);
            }
        });
    }
    pub(super) fn finish_v1(self) -> ReceiptV1 {
        let mut receipt = ACTIVE.with(|active| {
            active
                .borrow_mut()
                .take()
                .expect("active prover observation")
        });
        if let Some(session) = receipt.session.take() {
            // The thread-local borrow has ended: closing the root delivers
            // the finished record to this receipt's own collector.
            session.finish();
            receipt.measurement = receipt.sink.take().pop().map(Box::new);
        }
        receipt
    }
}
impl Drop for ObservationV1 {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.borrow_mut().take();
        });
    }
}

/// RAII records errors and unwinds without needing an error payload.
pub(super) struct PhaseTimerV1 {
    active: Option<(PhaseV1, Instant, std::rc::Rc<()>, Option<PhaseGuard>)>,
    complete: bool,
    thread_bound: core::marker::PhantomData<std::rc::Rc<()>>,
}
impl PhaseTimerV1 {
    pub(super) fn start_v1(phase: PhaseV1) -> Self {
        Self {
            active: ACTIVE.with(|active| {
                active.borrow().as_ref().map(|receipt| {
                    (
                        phase,
                        Instant::now(),
                        std::rc::Rc::clone(&receipt.scope),
                        receipt
                            .session
                            .as_ref()
                            .map(|session| session.enter(PHASE_LABELS_V1[phase as usize])),
                    )
                })
            }),
            complete: false,
            thread_bound: core::marker::PhantomData,
        }
    }
    pub(super) fn complete_v1(mut self) {
        self.complete = true;
    }
}
impl Drop for PhaseTimerV1 {
    fn drop(&mut self) {
        if let Some((phase, started, scope, guard)) = self.active.take() {
            // Close the shared-tree phase first. A guard whose observation
            // has already finished is stale and writes nothing.
            match guard {
                Some(guard) if self.complete => guard.complete(),
                other => drop(other),
            }
            ACTIVE.with(|active| {
                if let Some(receipt) = active
                    .borrow_mut()
                    .as_mut()
                    .filter(|receipt| std::rc::Rc::ptr_eq(&scope, &receipt.scope))
                {
                    let count = &mut receipt.phases[phase as usize];
                    count.calls += 1;
                    count.completed += u64::from(self.complete);
                    count.interrupted += u64::from(!self.complete);
                    count.unwound += u64::from(std::thread::panicking());
                    count.elapsed += started.elapsed();
                }
            });
        }
    }
}

pub(super) fn policy_v1(device_columns: usize) {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            assert!(
                device_columns < receipt.policies.len(),
                "bounded device policy"
            );
            receipt.policies[device_columns] += 1;
            receipt.work_v1("transform_policy_max_device_columns", device_columns as u64);
        }
    });
}
pub(super) fn completed_transform_v1(metal: bool, columns: usize) {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            assert!(
                (1..=8).contains(&columns),
                "bounded common-domain transform"
            );
            if metal {
                receipt.metal_calls += 1;
                receipt.metal_columns += columns as u64;
                receipt.work_v1("transform_metal_columns", columns as u64);
            } else {
                receipt.cpu_calls += 1;
                receipt.cpu_columns += columns as u64;
                receipt.work_v1("transform_cpu_columns", columns as u64);
            }
        }
    });
}
/// Completed public fixed-column work, recorded after each bounded batch.
/// Recovery IFFTs are additional work introduced by the bounded matrix owner.
pub(super) fn completed_fixed_coset_v1(columns: usize, rows: usize, recovery: bool) {
    assert!((1..=8).contains(&columns) && rows.is_power_of_two());
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            let columns = columns as u64;
            let butterflies = columns * (rows / 2) as u64 * u64::from(rows.ilog2());
            receipt.fixed_forward_columns += columns;
            receipt.fixed_forward_butterflies += butterflies;
            receipt.work_v1("fixed_coset_forward_columns", columns);
            receipt.work_v1("fixed_coset_forward_butterflies", butterflies);
            if recovery {
                receipt.fixed_inverse_columns += columns;
                receipt.fixed_inverse_butterflies += butterflies;
                receipt.work_v1("fixed_coset_recovery_inverse_columns", columns);
                receipt.work_v1("fixed_coset_recovery_inverse_butterflies", butterflies);
            }
        }
    });
}

/// Actual completed fixed-coset arithmetic, separated by direction and backend.
pub(super) fn completed_fixed_backend_v1(metal: bool, inverse: bool, columns: usize) {
    assert!((1..=8).contains(&columns));
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.fixed_backend_columns[usize::from(metal)][usize::from(inverse)] +=
                columns as u64;
            receipt.work_v1(
                match (metal, inverse) {
                    (false, false) => "fixed_coset_cpu_forward_columns",
                    (false, true) => "fixed_coset_cpu_inverse_columns",
                    (true, false) => "fixed_coset_metal_forward_columns",
                    (true, true) => "fixed_coset_metal_inverse_columns",
                },
                columns as u64,
            );
        }
    });
}

/// Completed private quotient stripe FFTs; fixed and common-domain counters stay separate.
pub(super) fn completed_quotient_backend_v1(metal: bool, columns: usize) {
    assert!((1..=8).contains(&columns));
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.quotient_backend_columns[usize::from(metal)] += columns as u64;
            receipt.work_v1(
                if metal {
                    "quotient_stripe_metal_forward_columns"
                } else {
                    "quotient_stripe_cpu_forward_columns"
                },
                columns as u64,
            );
        }
    });
}

/// Completed private native replay IFFTs, before any mask application.
pub(super) fn completed_native_replay_backend_v1(metal: bool, columns: usize) {
    assert!((1..=8).contains(&columns));
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.native_replay_backend_columns[usize::from(metal)] += columns as u64;
            receipt.work_v1(
                if metal {
                    "native_replay_metal_inverse_columns"
                } else {
                    "native_replay_cpu_inverse_columns"
                },
                columns as u64,
            );
        }
    });
}

/// Failed public fixed-coset production calls; distinct from MAIN replay work.
pub(super) fn failed_fixed_coset_v1() {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.fixed_failures += 1;
            receipt.work_v1("fixed_coset_failed_calls", 1);
        }
    });
}

pub(super) fn failed_transform_v1() {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.failures += 1;
            receipt.work_v1("transform_failed_calls", 1);
        }
    });
}
impl ReceiptV1 {
    /// Record one public geometry count in the shared record, in the phase
    /// that is open on this thread. The label is a literal of this file.
    // TODO: X.3 — the shared record of this adapter carries no allocation
    // source yet: the prover's allocation ledger is not exposed to this
    // diagnostic. Observe it with `observe_allocation_budget` once it is.
    fn work_v1(&self, label: &'static str, units: u64) {
        if let Some(session) = &self.session {
            session.record_work(label, units);
        }
    }
    /// The shared public phase-tree record of this finished observation.
    pub(super) fn measurement_v1(&self) -> Option<&MeasurementRecord> {
        self.measurement.as_deref()
    }
    /// What the session wrote into the harness directory, when one was given.
    pub(super) fn harness_v1(&self) -> Option<&DirectoryReport> {
        self.harness.as_ref()
    }
    pub(super) fn public_text_v1(&self) -> String {
        let mut text = format!(
            "observation_scope=calling-thread-public-geometry-counters\nphase_durations=nested-not-additive\ntransform_scope=MAIN-common-domain-FFT-only\ntransform_policy_counts_by_max_device_columns={:?}\ntransform_cpu_calls={}\ntransform_cpu_columns={}\ntransform_metal_calls={}\ntransform_metal_columns={}\ntransform_failed_calls={}\nfixed_coset_backend=observed\nfixed_coset_failed_calls={}\nfixed_coset_cpu_forward_columns={}\nfixed_coset_cpu_inverse_columns={}\nfixed_coset_metal_forward_columns={}\nfixed_coset_metal_inverse_columns={}\nfixed_coset_forward_columns={}\nfixed_coset_recovery_inverse_columns={}\nfixed_coset_forward_butterflies={}\nfixed_coset_recovery_inverse_butterflies={}\nquotient_stripe_backend=observed\nquotient_stripe_cpu_forward_columns={}\nquotient_stripe_metal_forward_columns={}\nnative_replay_backend=observed\nnative_replay_scope=initial-resident-and-original-mask-replay\nnative_replay_cpu_inverse_columns={}\nnative_replay_metal_inverse_columns={}\nother_transform_backends=unobserved",
            self.policies,
            self.cpu_calls,
            self.cpu_columns,
            self.metal_calls,
            self.metal_columns,
            self.failures,
            self.fixed_failures,
            self.fixed_backend_columns[0][0],
            self.fixed_backend_columns[0][1],
            self.fixed_backend_columns[1][0],
            self.fixed_backend_columns[1][1],
            self.fixed_forward_columns,
            self.fixed_inverse_columns,
            self.fixed_forward_butterflies,
            self.fixed_inverse_butterflies,
            self.quotient_backend_columns[0],
            self.quotient_backend_columns[1],
            self.native_replay_backend_columns[0],
            self.native_replay_backend_columns[1],
        );
        for (phase, count) in PHASES.iter().zip(self.phases) {
            use core::fmt::Write as _;
            writeln!(
                text,
                "\nphase={phase:?} calls={} completed={} interrupted={} unwound={} seconds={:.6}",
                count.calls,
                count.completed,
                count.interrupted,
                count.unwound,
                count.elapsed.as_secs_f64()
            )
            .expect("String formatting");
        }
        text
    }
    pub(super) fn assert_complete_main_coverage_v1(&self, expected_columns: usize) {
        assert_eq!(
            self.cpu_columns + self.metal_columns,
            expected_columns as u64,
            "diagnostic must observe both commitments and both opening replays"
        );
        assert_eq!(self.failures, 0);
        assert!(self.policies.iter().sum::<u64>() >= 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fine_phase_nested_counts_remain_separate_without_recording_payloads() {
        let observation = ObservationV1::begin_v1();
        let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
        let middle = PhaseTimerV1::start_v1(PhaseV1::CompositionKeyLinks);
        PhaseTimerV1::start_v1(PhaseV1::SourceP256ValueBusBase).complete_v1();
        middle.complete_v1();
        outer.complete_v1();
        let receipt = observation.finish_v1();
        let parent = receipt.phases[PhaseV1::Composition as usize];
        let child = receipt.phases[PhaseV1::CompositionKeyLinks as usize];
        let source = receipt.phases[PhaseV1::SourceP256ValueBusBase as usize];
        assert!(parent.elapsed >= child.elapsed && child.elapsed >= source.elapsed);
        assert_eq!(
            receipt.phases.iter().map(|count| count.calls).sum::<u64>(),
            3
        );
        assert_eq!(
            receipt
                .phases
                .iter()
                .map(|count| count.completed)
                .sum::<u64>(),
            3
        );
        let text = receipt.public_text_v1();
        for line in text.lines().filter(|line| line.starts_with("phase=")) {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 6);
            let label = fields[0].strip_prefix("phase=").unwrap();
            assert!(PHASES.iter().any(|phase| format!("{phase:?}") == label));
            for (field, key) in
                fields[1..]
                    .iter()
                    .zip(["calls", "completed", "interrupted", "unwound", "seconds"])
            {
                let (actual_key, value) = field.split_once('=').unwrap();
                assert_eq!(actual_key, key);
                assert!(value.chars().all(|c| c.is_ascii_digit() || c == '.'));
            }
        }
    }

    #[test]
    fn fine_phase_inventory_is_fixed_unique_and_preserves_every_old_phase() {
        assert_eq!(PHASES.len(), 65);
        let observation = ObservationV1::begin_v1();
        for (index, phase) in PHASES.iter().copied().enumerate() {
            assert_eq!(phase as usize, index);
            PhaseTimerV1::start_v1(phase).complete_v1();
        }
        let receipt = observation.finish_v1();
        assert!(receipt.phases.iter().all(|count| count.calls == 1
            && count.completed == 1
            && count.interrupted == 0
            && count.unwound == 0));
        let text = receipt.public_text_v1();
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("phase="))
                .count(),
            65
        );
        for phase in [
            PhaseV1::Composition,
            PhaseV1::CompositionRegistration,
            PhaseV1::CompositionCaLinks,
            PhaseV1::CompositionFp4Evaluations,
            PhaseV1::SourceStrictDerBase,
            PhaseV1::SourceP256ScalarBitBusAux,
        ] {
            assert!(text.contains(&format!("phase={phase:?} calls=1 completed=1")));
        }
    }

    #[test]
    fn first_pass_observation_reports_combined_phases_and_keeps_sampling_subphases() {
        let observation = ObservationV1::begin_v1();
        PhaseTimerV1::start_v1(PhaseV1::BaseSampleAndCommit).complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::AuxSampleAndCommit).complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::SampleSourceColumns).complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::SampleMaskDraws).complete_v1();
        let receipt = observation.finish_v1();
        for phase in [
            PhaseV1::BaseSampleAndCommit,
            PhaseV1::AuxSampleAndCommit,
            PhaseV1::SampleSourceColumns,
            PhaseV1::SampleMaskDraws,
        ] {
            assert_eq!(receipt.phases[phase as usize].completed, 1);
        }
        let text = receipt.public_text_v1();
        assert!(text.contains("BaseSampleAndCommit") && text.contains("AuxSampleAndCommit"));
        assert!(!text.contains("BaseMasks") && !text.contains("AuxMasks"));
        assert!(text.contains("native_replay_scope=initial-resident-and-original-mask-replay"));
    }

    #[test]
    fn public_observation_counts_success_error_unwind_and_actual_backends() {
        let observation = ObservationV1::begin_v1();
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        drop(PhaseTimerV1::start_v1(PhaseV1::Assembly));
        let _ = std::panic::catch_unwind(|| {
            let _timer = PhaseTimerV1::start_v1(PhaseV1::BoundSources);
            panic!("synthetic observer unwind");
        });
        for columns in [0, 2, 2, 0] {
            policy_v1(columns);
        }
        completed_transform_v1(false, 8);
        completed_transform_v1(true, 2);
        let receipt = observation.finish_v1();
        receipt.assert_complete_main_coverage_v1(10);
        assert_eq!(receipt.phases[PhaseV1::Preparation as usize].completed, 1);
        assert_eq!(receipt.phases[PhaseV1::Assembly as usize].interrupted, 1);
        assert_eq!(receipt.phases[PhaseV1::BoundSources as usize].unwound, 1);
        assert!(
            receipt
                .public_text_v1()
                .contains("transform_metal_columns=2")
        );
        let observer = ObservationV1::begin_v1();
        failed_transform_v1();
        assert_eq!(observer.finish_v1().failures, 1);
    }
    #[test]
    fn fixed_coset_observation_counts_extra_recovery_without_changing_main_coverage() {
        let observation = ObservationV1::begin_v1();
        completed_fixed_coset_v1(8, 16, false);
        completed_fixed_coset_v1(8, 16, true);
        completed_fixed_coset_v1(3, 16, true);
        let receipt = observation.finish_v1();
        assert_eq!(receipt.fixed_forward_columns, 19);
        assert_eq!(receipt.fixed_inverse_columns, 11);
        assert_eq!(receipt.fixed_forward_butterflies, 19 * 8 * 4);
        assert_eq!(receipt.fixed_inverse_butterflies, 11 * 8 * 4);
        assert_eq!(receipt.cpu_columns + receipt.metal_columns, 0);
        assert!(
            receipt
                .public_text_v1()
                .contains("fixed_coset_recovery_inverse_columns=11")
        );
    }

    #[test]
    fn native_replay_observation_separates_inverse_work_from_all_forward_counters() {
        let observation = ObservationV1::begin_v1();
        completed_native_replay_backend_v1(false, 8);
        completed_native_replay_backend_v1(true, 4);
        let receipt = observation.finish_v1();
        assert_eq!(receipt.native_replay_backend_columns, [8, 4]);
        assert_eq!(receipt.quotient_backend_columns, [0, 0]);
        assert_eq!(receipt.fixed_backend_columns, [[0, 0], [0, 0]]);
        assert_eq!(receipt.cpu_columns + receipt.metal_columns, 0);
        assert!(
            receipt
                .public_text_v1()
                .contains("native_replay_cpu_inverse_columns=8")
        );
        assert!(
            receipt
                .public_text_v1()
                .contains("native_replay_metal_inverse_columns=4")
        );
    }

    #[test]
    fn quotient_backend_observation_does_not_count_fixed_or_common_domain_work() {
        let observation = ObservationV1::begin_v1();
        completed_quotient_backend_v1(false, 8);
        completed_quotient_backend_v1(true, 3);
        let receipt = observation.finish_v1();
        assert_eq!(receipt.quotient_backend_columns, [8, 3]);
        assert_eq!(receipt.fixed_backend_columns, [[0, 0], [0, 0]]);
        assert_eq!(receipt.cpu_columns + receipt.metal_columns, 0);
        assert!(
            receipt
                .public_text_v1()
                .contains("quotient_stripe_cpu_forward_columns=8")
        );
        assert!(
            receipt
                .public_text_v1()
                .contains("quotient_stripe_metal_forward_columns=3")
        );
    }

    #[test]
    fn fixed_backend_observation_counts_only_reported_directions() {
        let observation = ObservationV1::begin_v1();
        completed_fixed_backend_v1(false, false, 8);
        completed_fixed_backend_v1(false, true, 2);
        completed_fixed_backend_v1(true, false, 3);
        completed_fixed_backend_v1(true, true, 1);
        failed_fixed_coset_v1();
        let receipt = observation.finish_v1();
        assert_eq!(receipt.fixed_backend_columns, [[8, 2], [3, 1]]);
        assert_eq!(receipt.fixed_failures, 1);
        assert_eq!(receipt.cpu_columns + receipt.metal_columns, 0);
        let text = receipt.public_text_v1();
        for count in [
            "cpu_forward_columns=8",
            "cpu_inverse_columns=2",
            "metal_forward_columns=3",
            "metal_inverse_columns=1",
            "failed_calls=1",
        ] {
            assert!(text.contains(&format!("fixed_coset_{count}")));
        }
    }

    #[test]
    fn observation_is_bounded_scoped_and_does_not_claim_worker_thread_coverage() {
        // Diagnostic-only fixed public counters: 65 phase slots, never witness-sized.
        assert!(core::mem::size_of::<ReceiptV1>() < 4_096);
        let observation = ObservationV1::begin_v1();
        std::thread::spawn(|| completed_transform_v1(true, 8))
            .join()
            .unwrap();
        let receipt = observation.finish_v1();
        assert_eq!(receipt.metal_columns, 0);
        assert!(std::panic::catch_unwind(|| receipt.assert_complete_main_coverage_v1(8)).is_err());
        drop(ObservationV1::begin_v1());
        assert_eq!(ObservationV1::begin_v1().finish_v1().cpu_columns, 0);
    }

    #[test]
    fn a_timer_cannot_outlive_its_observation_and_write_into_a_later_one() {
        let first = ObservationV1::begin_v1();
        let stale = PhaseTimerV1::start_v1(PhaseV1::Assembly);
        assert_eq!(
            first.finish_v1().phases[PhaseV1::Assembly as usize].calls,
            0
        );
        let second = ObservationV1::begin_v1();
        stale.complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        let receipt = second.finish_v1();
        assert_eq!(receipt.phases[PhaseV1::Assembly as usize].calls, 0);
        assert_eq!(receipt.phases[PhaseV1::Preparation as usize].completed, 1);
    }
    #[test]
    fn composition_subphases_preserve_bounded_thread_scoped_error_and_unwind_counts() {
        // These observations are wall time around the original serial caller's
        // whole parallel operation, never per-worker field data or CPU time.
        let phases = [
            PhaseV1::CompositionArithmeticFixedRows,
            PhaseV1::CompositionArithmeticFixedInverseTransform,
            PhaseV1::CompositionRegistration,
            PhaseV1::CompositionTraceCache,
            PhaseV1::CompositionDenominators,
            PhaseV1::CompositionBaseReplay,
            PhaseV1::CompositionAuxReplay,
            PhaseV1::CompositionFixedReplay,
            PhaseV1::CompositionResiduesAndFold,
            PhaseV1::CompositionInverseTransform,
            PhaseV1::CompositionDegreeChunks,
        ];
        let observation = ObservationV1::begin_v1();
        for phase in phases {
            PhaseTimerV1::start_v1(phase).complete_v1();
            drop(PhaseTimerV1::start_v1(phase));
            let _ = std::panic::catch_unwind(|| {
                let _timer = PhaseTimerV1::start_v1(phase);
                panic!("synthetic composition subphase unwind");
            });
        }
        std::thread::spawn(|| {
            PhaseTimerV1::start_v1(PhaseV1::CompositionResiduesAndFold).complete_v1();
        })
        .join()
        .unwrap();
        let receipt = observation.finish_v1();
        // Diagnostic-only fixed public counters: 65 phase slots, never witness-sized.
        assert!(core::mem::size_of::<ReceiptV1>() < 4_096);
        for phase in phases {
            let count = receipt.phases[phase as usize];
            assert_eq!(count.calls, 3);
            assert_eq!(count.completed, 1);
            assert_eq!(count.interrupted, 2);
            assert_eq!(count.unwound, 1);
            assert!(receipt.public_text_v1().contains(&format!(
                "phase={phase:?} calls=3 completed=1 interrupted=2 unwound=1"
            )));
        }
        assert_eq!(receipt.phases[PhaseV1::Composition as usize].calls, 0);
        assert_eq!(receipt.cpu_columns + receipt.metal_columns, 0);
    }

    #[test]
    fn joined_replay_observation_separates_initial_query_and_failed_work() {
        let phases = [
            PhaseV1::InitialJoinedSourceBatch,
            PhaseV1::InitialJoinedTransform,
            PhaseV1::QueryJoinedSourceBatch,
            PhaseV1::QueryJoinedTransform,
        ];
        let observation = ObservationV1::begin_v1();
        for phase in phases {
            PhaseTimerV1::start_v1(phase).complete_v1();
            drop(PhaseTimerV1::start_v1(phase));
            let _ = std::panic::catch_unwind(|| {
                let _timer = PhaseTimerV1::start_v1(phase);
                panic!("synthetic public replay interruption");
            });
        }
        let stale = PhaseTimerV1::start_v1(PhaseV1::QueryJoinedTransform);
        let receipt = observation.finish_v1();
        for phase in phases {
            let count = receipt.phases[phase as usize];
            assert_eq!(count.calls, 3);
            assert_eq!(count.completed, 1);
            assert_eq!(count.interrupted, 2);
            assert_eq!(count.unwound, 1);
        }
        assert_eq!(receipt.phases[PhaseV1::QueryOpenings as usize].calls, 0);
        // Diagnostic-only fixed public counters: 65 phase slots, never witness-sized.
        assert!(core::mem::size_of::<ReceiptV1>() < 4_096);
        let next = ObservationV1::begin_v1();
        stale.complete_v1();
        let receipt = next.finish_v1();
        for phase in phases {
            assert_eq!(receipt.phases[phase as usize].calls, 0);
        }
    }

    #[test]
    fn composition_subphase_timer_cannot_cross_observation_epochs() {
        let first = ObservationV1::begin_v1();
        let stale = PhaseTimerV1::start_v1(PhaseV1::CompositionRegistration);
        assert_eq!(
            first.finish_v1().phases[PhaseV1::CompositionRegistration as usize].calls,
            0
        );
        let second = ObservationV1::begin_v1();
        stale.complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::CompositionTraceCache).complete_v1();
        let receipt = second.finish_v1();
        assert_eq!(
            receipt.phases[PhaseV1::CompositionRegistration as usize].calls,
            0
        );
        assert_eq!(
            receipt.phases[PhaseV1::CompositionTraceCache as usize].completed,
            1
        );
    }

    fn tree_node<'a>(
        record: &'a MeasurementRecord,
        label: &str,
    ) -> (u32, &'a iroha_measurement::PhaseNode) {
        let mut found = record
            .phase_tree
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.label == label);
        let (index, node) = found.next().unwrap_or_else(|| panic!("no phase {label}"));
        assert!(found.next().is_none(), "phase {label} appears twice");
        (u32::try_from(index).unwrap(), node)
    }

    #[test]
    fn shared_tree_labels_are_the_static_names_of_the_fixed_phase_inventory() {
        assert_eq!(PHASE_LABELS_V1.len(), PHASES.len());
        for (phase, label) in PHASES.iter().zip(PHASE_LABELS_V1) {
            assert_eq!(format!("{phase:?}"), label);
            assert!(iroha_measurement::text::is_public_label(label));
        }
        for label in [
            MEASUREMENT_WORKLOAD_V1,
            MEASUREMENT_ROOT_V1,
            MEASUREMENT_EMITTER_V1,
        ] {
            assert!(iroha_measurement::text::is_public_label(label));
        }
    }

    #[test]
    fn timers_build_the_shared_phase_tree_with_exact_exclusive_time() {
        let observation = ObservationV1::begin_v1();
        let outer = PhaseTimerV1::start_v1(PhaseV1::Composition);
        let middle = PhaseTimerV1::start_v1(PhaseV1::CompositionKeyLinks);
        for _ in 0..3 {
            PhaseTimerV1::start_v1(PhaseV1::SourceP256ValueBusBase).complete_v1();
        }
        middle.complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::CompositionCommitment).complete_v1();
        outer.complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::DeepAndFri).complete_v1();
        let receipt = observation.finish_v1();
        let record = receipt.measurement_v1().expect("shared phase tree record");

        assert_eq!(record.identity.workload, MEASUREMENT_WORKLOAD_V1);
        assert_eq!(record.identity.flow, FlowKind::Proof);
        assert_eq!(record.identity.emitter, MEASUREMENT_EMITTER_V1);
        assert_eq!(record.identity.context, RunContext::unbound());
        assert_eq!(record.outcome, iroha_measurement::RunOutcome::Succeeded);
        assert_eq!(
            record.scheduling.workers,
            u32::try_from(rayon::current_num_threads()).unwrap()
        );

        let (root, observed) = tree_node(record, MEASUREMENT_ROOT_V1);
        let (composition, outer) = tree_node(record, "Composition");
        let (key_links, middle) = tree_node(record, "CompositionKeyLinks");
        let (_, source) = tree_node(record, "SourceP256ValueBusBase");
        let (_, commitment) = tree_node(record, "CompositionCommitment");
        let (_, fri) = tree_node(record, "DeepAndFri");
        assert_eq!((root, observed.parent), (0, None));
        assert_eq!(record.phase_tree.nodes.len(), 6);
        // The tree records the nesting that the flat counters cannot.
        assert_eq!(outer.parent, Some(0));
        assert_eq!(middle.parent, Some(composition));
        assert_eq!(source.parent, Some(key_links));
        assert_eq!(commitment.parent, Some(composition));
        assert_eq!(fri.parent, Some(0));
        assert_eq!((source.calls, source.completed), (3, 3));
        // Exclusive time is exactly inclusive minus the sequential children.
        assert_eq!(
            outer.wall_exclusive_ns,
            outer.wall_inclusive_ns - middle.wall_inclusive_ns - commitment.wall_inclusive_ns
        );
        assert_eq!(
            middle.wall_exclusive_ns,
            middle.wall_inclusive_ns - source.wall_inclusive_ns
        );
        assert_eq!(
            observed.wall_exclusive_ns,
            observed.wall_inclusive_ns - outer.wall_inclusive_ns - fri.wall_inclusive_ns
        );
        let attribution = record.attribution().unwrap();
        assert_eq!(attribution.root_wall_ns, observed.wall_inclusive_ns);
        assert_eq!(attribution.unattributed_wall_ns, observed.wall_exclusive_ns);
        // The flat X509 counters and the shared tree observe the same calls.
        for (phase, label) in PHASES.iter().zip(PHASE_LABELS_V1) {
            let flat = receipt.phases[*phase as usize];
            let (calls, completed): (u64, u64) = record
                .phase_tree
                .nodes
                .iter()
                .filter(|node| node.label == label)
                .fold((0, 0), |sum, node| {
                    (sum.0 + node.calls, sum.1 + node.completed)
                });
            assert_eq!((flat.calls, flat.completed), (calls, completed), "{label}");
        }
        // Only an unbound identity and timing coverage can be reported: the
        // record's accounting itself has no finding.
        assert!(record.findings().iter().all(|finding| matches!(
            finding,
            iroha_measurement::Finding::IdentityIncomplete { .. }
                | iroha_measurement::Finding::DirtyDigestMismatch
                | iroha_measurement::Finding::UnattributedExceedsLimit { .. }
        )));
        assert!(iroha_measurement::unclassified_numbers(&record.to_json_value()).is_empty());
    }

    #[test]
    fn shared_tree_counts_error_returns_and_unwinds_like_the_flat_receipt() {
        let observation = ObservationV1::begin_v1();
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        drop(PhaseTimerV1::start_v1(PhaseV1::Assembly));
        let _ = std::panic::catch_unwind(|| {
            let _timer = PhaseTimerV1::start_v1(PhaseV1::BoundSources);
            panic!("synthetic observer unwind");
        });
        observation.record_failure_v1("producer", "error");
        let receipt = observation.finish_v1();
        let record = receipt.measurement_v1().unwrap();
        let (_, preparation) = tree_node(record, "Preparation");
        let (_, assembly) = tree_node(record, "Assembly");
        let (_, bound) = tree_node(record, "BoundSources");
        assert_eq!((preparation.completed, preparation.interrupted), (1, 0));
        assert_eq!((assembly.completed, assembly.interrupted), (0, 1));
        assert_eq!((assembly.unwound, bound.unwound), (0, 1));
        assert_eq!(bound.interrupted, 1);
        for phase in [
            PhaseV1::Preparation,
            PhaseV1::Assembly,
            PhaseV1::BoundSources,
        ] {
            let flat = receipt.phases[phase as usize];
            let (_, node) = tree_node(record, PHASE_LABELS_V1[phase as usize]);
            assert_eq!(
                (flat.calls, flat.completed, flat.interrupted, flat.unwound),
                (node.calls, node.completed, node.interrupted, node.unwound)
            );
        }
        // The raw failure is retained and makes the run a failed one.
        assert_eq!(record.outcome, iroha_measurement::RunOutcome::Failed);
        assert_eq!(record.failures.entries.len(), 1);
        assert_eq!(record.failures.entries[0].stage, "producer");
        assert_eq!(record.failures.entries[0].code, "error");
        assert!(
            record
                .findings()
                .contains(&iroha_measurement::Finding::RunNotSucceeded {
                    outcome: iroha_measurement::RunOutcome::Failed
                })
        );
    }

    #[test]
    fn shared_tree_is_thread_scoped_and_closed_to_stale_timers() {
        let first = ObservationV1::begin_v1();
        let stale = PhaseTimerV1::start_v1(PhaseV1::Assembly);
        std::thread::spawn(|| {
            // No observation is active on this thread: nothing is recorded.
            PhaseTimerV1::start_v1(PhaseV1::CompositionResiduesAndFold).complete_v1();
        })
        .join()
        .unwrap();
        let first = first.finish_v1();
        let record = first.measurement_v1().unwrap();
        // The timer still open at finish is truncated in the first tree ...
        let (_, assembly) = tree_node(record, "Assembly");
        assert_eq!(
            (assembly.calls, assembly.interrupted, assembly.truncated),
            (1, 1, 1)
        );
        assert!(
            record
                .phase_tree
                .nodes
                .iter()
                .all(|node| node.label != "CompositionResiduesAndFold")
        );
        let second = ObservationV1::begin_v1();
        // ... and cannot write into the tree of a later observation.
        stale.complete_v1();
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        let second = second.finish_v1();
        let labels: Vec<_> = second
            .measurement_v1()
            .unwrap()
            .phase_tree
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect();
        assert_eq!(labels, [MEASUREMENT_ROOT_V1, "Preparation"]);
        // An observation dropped without finishing leaves nothing active.
        drop(ObservationV1::begin_v1());
        assert!(
            ObservationV1::begin_v1()
                .finish_v1()
                .measurement_v1()
                .is_some()
        );
    }

    #[test]
    fn harness_context_binds_the_identity_of_the_shared_record() {
        let context = RunContext {
            source_commit: "7e93d3e049".repeat(4),
            source_dirty: false,
            source_dirty_digest: None,
            artifact: "sha256:x509-test-binary".into(),
            profile: "complete49-MAIN-plus-compactCA".into(),
            config: "taira_default".into(),
            hardware: "reference/unit-test-host".into(),
            cache_policy: iroha_measurement::CachePolicy::Cold,
        };
        let observation = ObservationV1::begin_with_context_v1(context.clone());
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        let receipt = observation.finish_v1();
        let record = receipt.measurement_v1().unwrap();
        assert_eq!(record.identity.context, context);
        let bytes = record.to_norito_bytes().unwrap();
        assert_eq!(
            &MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
            record
        );
        assert!(!record.findings().iter().any(|finding| matches!(
            finding,
            iroha_measurement::Finding::IdentityIncomplete { .. }
        )));
    }

    fn work_total(record: &MeasurementRecord, label: &str) -> (u64, u64) {
        record
            .work_counters
            .entries
            .iter()
            .filter(|counter| counter.label == label)
            .fold((0, 0), |sum, counter| {
                (sum.0 + counter.count, sum.1 + counter.total_units)
            })
    }

    #[test]
    fn public_geometry_counters_and_proof_size_are_in_the_shared_record() {
        let observation = ObservationV1::begin_v1();
        for columns in [0, 2, 2] {
            policy_v1(columns);
        }
        let phase = PhaseTimerV1::start_v1(PhaseV1::BaseSampleAndCommit);
        completed_transform_v1(false, 8);
        completed_transform_v1(false, 3);
        completed_transform_v1(true, 2);
        phase.complete_v1();
        completed_fixed_coset_v1(8, 16, false);
        completed_fixed_coset_v1(3, 16, true);
        completed_fixed_backend_v1(false, false, 8);
        completed_fixed_backend_v1(false, true, 2);
        completed_fixed_backend_v1(true, false, 3);
        completed_fixed_backend_v1(true, true, 1);
        completed_quotient_backend_v1(false, 8);
        completed_quotient_backend_v1(true, 3);
        completed_native_replay_backend_v1(false, 8);
        completed_native_replay_backend_v1(true, 4);
        failed_fixed_coset_v1();
        failed_transform_v1();
        failed_transform_v1();
        observation.record_proof_bytes_v1(8_123_456);
        let receipt = observation.finish_v1();
        let record = receipt.measurement_v1().unwrap();
        // Every flat counter of the receipt has the same value in the record:
        // (number of reports, sum of units).
        for (label, expected) in [
            (
                "transform_cpu_columns",
                (receipt.cpu_calls, receipt.cpu_columns),
            ),
            (
                "transform_metal_columns",
                (receipt.metal_calls, receipt.metal_columns),
            ),
            ("transform_failed_calls", (2, receipt.failures)),
            ("transform_policy_max_device_columns", (3, 4)),
            (
                "fixed_coset_forward_columns",
                (2, receipt.fixed_forward_columns),
            ),
            (
                "fixed_coset_forward_butterflies",
                (2, receipt.fixed_forward_butterflies),
            ),
            (
                "fixed_coset_recovery_inverse_columns",
                (1, receipt.fixed_inverse_columns),
            ),
            (
                "fixed_coset_recovery_inverse_butterflies",
                (1, receipt.fixed_inverse_butterflies),
            ),
            (
                "fixed_coset_cpu_forward_columns",
                (1, receipt.fixed_backend_columns[0][0]),
            ),
            (
                "fixed_coset_cpu_inverse_columns",
                (1, receipt.fixed_backend_columns[0][1]),
            ),
            (
                "fixed_coset_metal_forward_columns",
                (1, receipt.fixed_backend_columns[1][0]),
            ),
            (
                "fixed_coset_metal_inverse_columns",
                (1, receipt.fixed_backend_columns[1][1]),
            ),
            ("fixed_coset_failed_calls", (1, receipt.fixed_failures)),
            (
                "quotient_stripe_cpu_forward_columns",
                (1, receipt.quotient_backend_columns[0]),
            ),
            (
                "quotient_stripe_metal_forward_columns",
                (1, receipt.quotient_backend_columns[1]),
            ),
            (
                "native_replay_cpu_inverse_columns",
                (1, receipt.native_replay_backend_columns[0]),
            ),
            (
                "native_replay_metal_inverse_columns",
                (1, receipt.native_replay_backend_columns[1]),
            ),
        ] {
            assert_eq!(work_total(record, label), expected, "{label}");
            assert!(iroha_measurement::text::is_public_label(label));
        }
        assert_eq!(
            (
                receipt.cpu_calls,
                receipt.cpu_columns,
                receipt.metal_columns
            ),
            (2, 11, 2)
        );
        assert_eq!(
            record
                .work_counters
                .entries
                .iter()
                .map(|counter| counter.label.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            17
        );
        // A counter is attributed to the phase that was open when it was reported.
        let (commit, _) = tree_node(record, "BaseSampleAndCommit");
        let transform = record
            .work_counters
            .entries
            .iter()
            .find(|counter| counter.label == "transform_cpu_columns")
            .unwrap();
        assert_eq!(
            (transform.phase, transform.min_units, transform.max_units),
            (commit, 3, 8)
        );
        // The proof is recorded as its size in the root phase, never as bytes.
        assert_eq!(record.byte_counters.entries.len(), 1);
        let proof = &record.byte_counters.entries[0];
        assert_eq!(
            (
                proof.kind,
                proof.label.as_str(),
                proof.phase,
                proof.total_bytes
            ),
            (ByteKind::Proof, "credential_proof", 0, 8_123_456)
        );
        assert!(record.findings().iter().all(|finding| matches!(
            finding,
            iroha_measurement::Finding::IdentityIncomplete { .. }
                | iroha_measurement::Finding::DirtyDigestMismatch
                | iroha_measurement::Finding::NoPhases
                | iroha_measurement::Finding::UnattributedExceedsLimit { .. }
        )));
        assert!(iroha_measurement::unclassified_numbers(&record.to_json_value()).is_empty());
        // Counters reported with no active observation change nothing.
        completed_transform_v1(false, 8);
        failed_transform_v1();
        assert_eq!(ObservationV1::begin_v1().finish_v1().cpu_columns, 0);
    }

    fn harness_records(directory: &std::path::Path) -> Vec<MeasurementRecord> {
        let mut paths: Vec<_> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|kind| kind == "norito"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|path| {
                let record =
                    MeasurementRecord::from_norito_bytes(&std::fs::read(path).unwrap()).unwrap();
                // The JSON view the harness reads is written beside it.
                let view = std::fs::read_to_string(path.with_extension("json")).unwrap();
                assert_eq!(MeasurementRecord::from_json_view(&view).unwrap(), record);
                record
            })
            .collect()
    }

    #[test]
    fn finished_abandoned_and_unwound_observations_all_reach_the_harness_directory() {
        let directory = tempfile::tempdir().unwrap();
        let harness = || Some(directory.path().to_path_buf());
        // A finished observation with a producer failure: written once.
        let observation = ObservationV1::begin_with_harness_v1(RunContext::unbound(), harness());
        PhaseTimerV1::start_v1(PhaseV1::Preparation).complete_v1();
        observation.record_failure_v1("producer", "error");
        let receipt = observation.finish_v1();
        let written = receipt.harness_v1().unwrap();
        assert!(written.errors().is_empty() && written.partial().is_empty());
        assert_eq!(written.written().len(), 1);
        assert_eq!(
            harness_records(directory.path()),
            [receipt.measurement_v1().unwrap().clone()]
        );
        // An observation dropped without finishing, as on an error return.
        let abandoned = ObservationV1::begin_with_harness_v1(RunContext::unbound(), harness());
        let open = PhaseTimerV1::start_v1(PhaseV1::Composition);
        drop(abandoned);
        drop(open);
        // An observation dropped while the producer unwinds.
        let unwound = std::panic::catch_unwind(|| {
            let _observation =
                ObservationV1::begin_with_harness_v1(RunContext::unbound(), harness());
            let _timer = PhaseTimerV1::start_v1(PhaseV1::DeepAndFri);
            panic!("synthetic producer unwind");
        });
        assert!(unwound.is_err());
        let records = harness_records(directory.path());
        assert_eq!(
            records
                .iter()
                .map(|record| record.outcome)
                .collect::<Vec<_>>(),
            [
                iroha_measurement::RunOutcome::Failed,
                iroha_measurement::RunOutcome::Abandoned,
                iroha_measurement::RunOutcome::Unwound,
            ]
        );
        // The interrupted records keep the phases that were open.
        let (_, composition) = tree_node(&records[1], "Composition");
        assert_eq!((composition.interrupted, composition.truncated), (1, 1));
        let (_, fri) = tree_node(&records[2], "DeepAndFri");
        assert_eq!((fri.interrupted, fri.unwound), (1, 1));
        for record in &records[1..] {
            assert!(record.findings().iter().any(|finding| matches!(
                finding,
                iroha_measurement::Finding::RunNotSucceeded { .. }
            )));
        }
        // Without a harness directory nothing is written anywhere.
        let receipt = ObservationV1::begin_v1().finish_v1();
        assert!(receipt.harness_v1().is_none());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 6);
    }
}
