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
    BaseMasks,
    BaseCommitment,
    CompactCa,
    BoundSources,
    DerBinding,
    RfcBinding,
    AuxMasks,
    AuxCommitment,
    Composition,
    DeepAndFri,
    QueryOpenings,
    EnvelopeAndSelfCheck,
    SampleSourceColumns,
    SampleMaskDraws,
}
const PHASES: [PhaseV1; 17] = [
    PhaseV1::Preparation,
    PhaseV1::Assembly,
    PhaseV1::BaseSources,
    PhaseV1::BaseMasks,
    PhaseV1::BaseCommitment,
    PhaseV1::CompactCa,
    PhaseV1::BoundSources,
    PhaseV1::DerBinding,
    PhaseV1::RfcBinding,
    PhaseV1::AuxMasks,
    PhaseV1::AuxCommitment,
    PhaseV1::Composition,
    PhaseV1::DeepAndFri,
    PhaseV1::QueryOpenings,
    PhaseV1::EnvelopeAndSelfCheck,
    PhaseV1::SampleSourceColumns,
    PhaseV1::SampleMaskDraws,
];

#[derive(Clone, Copy, Default)]
struct PhaseCountV1 {
    calls: u64,
    completed: u64,
    interrupted: u64,
    unwound: u64,
    elapsed: Duration,
}

/// Public counters observed from completed MAIN common-domain FFT calls.
#[derive(Default)]
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
    fixed_forward_columns: u64,
    fixed_inverse_columns: u64,
    fixed_forward_butterflies: u64,
    fixed_inverse_butterflies: u64,
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
/// Completed public fixed-column CPU work, recorded after each Rayon join.
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
            "observation_scope=calling-thread-public-fixed-counters\nphase_durations=nested-not-additive\ntransform_scope=MAIN-common-domain-FFT-only\ntransform_policy_counts_by_max_device_columns={:?}\ntransform_cpu_calls={}\ntransform_cpu_columns={}\ntransform_metal_calls={}\ntransform_metal_columns={}\ntransform_failed_calls={}\nfixed_coset_backend=CPU\nfixed_coset_forward_columns={}\nfixed_coset_recovery_inverse_columns={}\nfixed_coset_forward_butterflies={}\nfixed_coset_recovery_inverse_butterflies={}\nother_transform_backends=unobserved",
            self.policies,
            self.cpu_calls,
            self.cpu_columns,
            self.metal_calls,
            self.metal_columns,
            self.failures,
            self.fixed_forward_columns,
            self.fixed_inverse_columns,
            self.fixed_forward_butterflies,
            self.fixed_inverse_butterflies,
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
    fn observation_is_bounded_scoped_and_does_not_claim_worker_thread_coverage() {
        assert!(core::mem::size_of::<ReceiptV1>() < 2_048);
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
}
