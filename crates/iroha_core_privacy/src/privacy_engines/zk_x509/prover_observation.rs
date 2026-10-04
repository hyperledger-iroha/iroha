//! Bounded public observations for isolated native prover diagnostics.
//!
//! Collection is explicitly scoped to the calling test thread. It never
//! records field values, source bytes, random masks or witness-dependent row
//! positions. Durations of nested phases overlap and must not be summed.

use core::cell::RefCell;
use std::time::{Duration, Instant};

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
}
impl Default for ReceiptV1 {
    fn default() -> Self {
        Self {
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
    pub(super) fn begin_v1() -> Self {
        ACTIVE.with(|active| {
            assert!(active.borrow().is_none(), "nested prover observation");
            *active.borrow_mut() = Some(ReceiptV1::default());
        });
        Self(core::marker::PhantomData)
    }
    pub(super) fn finish_v1(self) -> ReceiptV1 {
        ACTIVE.with(|active| {
            active
                .borrow_mut()
                .take()
                .expect("active prover observation")
        })
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
    active: Option<(PhaseV1, Instant, std::rc::Rc<()>)>,
    complete: bool,
    thread_bound: core::marker::PhantomData<std::rc::Rc<()>>,
}
impl PhaseTimerV1 {
    pub(super) fn start_v1(phase: PhaseV1) -> Self {
        Self {
            active: ACTIVE.with(|active| {
                active
                    .borrow()
                    .as_ref()
                    .map(|receipt| (phase, Instant::now(), std::rc::Rc::clone(&receipt.scope)))
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
        if let Some((phase, started, scope)) = self.active.take() {
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
            } else {
                receipt.cpu_calls += 1;
                receipt.cpu_columns += columns as u64;
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
            if recovery {
                receipt.fixed_inverse_columns += columns;
                receipt.fixed_inverse_butterflies += butterflies;
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
        }
    });
}

/// Completed private quotient stripe FFTs; fixed and common-domain counters stay separate.
pub(super) fn completed_quotient_backend_v1(metal: bool, columns: usize) {
    assert!((1..=8).contains(&columns));
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.quotient_backend_columns[usize::from(metal)] += columns as u64;
        }
    });
}

/// Completed private native replay IFFTs, before any mask application.
pub(super) fn completed_native_replay_backend_v1(metal: bool, columns: usize) {
    assert!((1..=8).contains(&columns));
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.native_replay_backend_columns[usize::from(metal)] += columns as u64;
        }
    });
}

/// Failed public fixed-coset production calls; distinct from MAIN replay work.
pub(super) fn failed_fixed_coset_v1() {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.fixed_failures += 1;
        }
    });
}

pub(super) fn failed_transform_v1() {
    ACTIVE.with(|active| {
        if let Some(receipt) = active.borrow_mut().as_mut() {
            receipt.failures += 1;
        }
    });
}
impl ReceiptV1 {
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
}
