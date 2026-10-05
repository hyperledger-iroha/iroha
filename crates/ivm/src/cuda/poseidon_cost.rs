//! Bounded public known-answer measurements over complete native Poseidon operations.

use super::{Input, Kernel, complete_current};
use crate::{
    cuda_cost::{CalibrationFailure, CostProfile, SIZES, TRIALS, TrialSample},
    field_dispatch::{FieldArithmetic, field_impl},
    poseidon::golden,
};
use iroha_accel::{ProcessResources, cuda::CudaFailure};
use std::{hint::black_box, time::Instant};

fn checked_now(
    deadline: Instant,
    cpu: &'static dyn FieldArithmetic,
) -> Result<Instant, CalibrationFailure> {
    let now = Instant::now();
    if now >= deadline {
        return Err(CalibrationFailure::Deadline);
    }
    if cpu.type_id() != field_impl().type_id() {
        return Err(CalibrationFailure::Deferred);
    }
    Ok(now)
}

fn elapsed(start: Instant) -> Result<u64, CalibrationFailure> {
    u64::try_from(start.elapsed().as_nanos())
        .ok()
        .filter(|ns| *ns != 0)
        .ok_or(CalibrationFailure::Deferred)
}

fn prefix(input: Input<'_>, count: usize) -> Input<'_> {
    match input {
        Input::Two(values) => Input::Two(&values[..count]),
        Input::Six(values) => Input::Six(&values[..count]),
    }
}

fn cpu_into(input: Input<'_>, output: &mut [u64]) {
    assert_eq!(input.len(), output.len());
    match input {
        Input::Two(values) => {
            for (result, &(a, b)) in output.iter_mut().zip(values) {
                *result = crate::poseidon::poseidon2_simd(a, b);
            }
        }
        Input::Six(values) => {
            for (result, &value) in output.iter_mut().zip(values) {
                *result = crate::poseidon::poseidon6_simd(value);
            }
        }
    }
}

fn expected(kernel: Kernel, index: usize) -> u64 {
    match kernel {
        Kernel::Poseidon2 => golden::TWO[index % golden::TWO.len()].2,
        Kernel::Poseidon6 => golden::SIX[index % golden::SIX.len()].1,
        _ => unreachable!("only the two finite Poseidon families are measured"),
    }
}

fn measure(
    input: Input<'_>,
    cpu: &'static dyn FieldArithmetic,
    deadline: Instant,
    cpu_output: &mut [u64],
    gpu_output: &mut [u64],
) -> Result<CostProfile, CalibrationFailure> {
    checked_now(deadline, cpu)?;
    let mut trials = [TrialSample::default(); SIZES.len()];
    for (sample, count) in SIZES.into_iter().enumerate() {
        let input = prefix(input, count);
        for trial in 0..TRIALS {
            let start = checked_now(deadline, cpu)?;
            cpu_into(black_box(input), black_box(&mut cpu_output[..count]));
            trials[sample].cpu_ns[trial] = elapsed(start)?;
            // Independent fixed known answers, rather than an unchecked local
            // CPU result, supply parity authority for each public synthetic item.
            if cpu_output[..count]
                .iter()
                .enumerate()
                .any(|(index, value)| *value != expected(input.kernel(), index))
            {
                return Err(CalibrationFailure::Deferred);
            }
            let start = checked_now(deadline, cpu)?;
            complete_current(
                black_box(input),
                black_box(&mut gpu_output[..count]),
                || cpu.type_id() == field_impl().type_id(),
            )
            .map_err(|error| match error {
                CudaFailure::Busy | CudaFailure::Capacity | CudaFailure::Unavailable => {
                    CalibrationFailure::Deferred
                }
                _ => CalibrationFailure::Backend,
            })?;
            // Includes request checking, parameter packing/uploads, state transfer,
            // launch/waits, complete validation/copyback and staging owner cleanup.
            trials[sample].gpu_ns[trial] = elapsed(start)?;
            if gpu_output[..count]
                .iter()
                .enumerate()
                .any(|(index, value)| *value != expected(input.kernel(), index))
            {
                crate::cuda_dispatch::quarantine_current_kernel();
                return Err(CalibrationFailure::Parity);
            }
            checked_now(deadline, cpu)?;
        }
    }
    CostProfile::from_trials(trials).ok_or(CalibrationFailure::Deferred)
}

pub(super) fn calibrate(
    kernel: Kernel,
    cpu: &'static dyn FieldArithmetic,
    deadline: Instant,
) -> Result<CostProfile, CalibrationFailure> {
    checked_now(deadline, cpu)?;
    let _probe =
        super::super::imp::SelftestRunningGuard::enter().ok_or(CalibrationFailure::Deferred)?;
    let resources = ProcessResources::get().ok_or(CalibrationFailure::Deferred)?;
    let count = SIZES[SIZES.len() - 1];
    if !matches!(kernel, Kernel::Poseidon2 | Kernel::Poseidon6) {
        return Err(CalibrationFailure::Deferred);
    }
    let allocate = || {
        resources
            .try_host_output::<u64>(count)
            .map_err(|_| CalibrationFailure::Deferred)
    };
    let mut cpu_output = allocate()?;
    let mut gpu_output = allocate()?;
    // Every input/output scratch bank is physically charged to the original
    // process host owner. There are no production values or uncharged input Vecs.
    match kernel {
        Kernel::Poseidon2 => {
            let mut values = resources
                .try_host_output::<(u64, u64)>(count)
                .map_err(|_| CalibrationFailure::Deferred)?;
            for (index, value) in values.iter_mut().enumerate() {
                let (a, b, _) = golden::TWO[index % golden::TWO.len()];
                *value = (a, b);
            }
            measure(
                Input::Two(&values),
                cpu,
                deadline,
                &mut cpu_output,
                &mut gpu_output,
            )
        }
        Kernel::Poseidon6 => {
            let mut values = resources
                .try_host_output::<[u64; 6]>(count)
                .map_err(|_| CalibrationFailure::Deferred)?;
            for (index, value) in values.iter_mut().enumerate() {
                *value = golden::SIX[index % golden::SIX.len()].0;
            }
            measure(
                Input::Six(&values),
                cpu,
                deadline,
                &mut cpu_output,
                &mut gpu_output,
            )
        }
        _ => Err(CalibrationFailure::Deferred),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_public_input_families_match_independent_answers_and_prefixes() {
        let two = golden::TWO.map(|(a, b, _)| (a, b));
        let six = golden::SIX.map(|(input, _)| input);
        for input in [Input::Two(&two), Input::Six(&six)] {
            let mut output = [0; 5];
            cpu_into(input, &mut output);
            for (index, value) in output.into_iter().enumerate() {
                assert_eq!(value, expected(input.kernel(), index));
                assert_eq!(expected(input.kernel(), index + 5), value);
            }
            let partial = prefix(input, 2);
            assert_eq!(partial.len(), 2);
            assert_eq!(partial.kernel(), input.kernel());
            let mut output = [0; 2];
            cpu_into(partial, &mut output);
            assert_eq!(
                output,
                [expected(input.kernel(), 0), expected(input.kernel(), 1)]
            );
        }
    }

    #[test]
    fn expired_calibration_pass_cannot_allocate_or_enter_native_measurement() {
        let cpu = field_impl();
        let now = Instant::now();
        assert_eq!(checked_now(now, cpu), Err(CalibrationFailure::Deadline));
        for kernel in [Kernel::Poseidon2, Kernel::Poseidon6] {
            assert!(matches!(
                calibrate(kernel, cpu, now),
                Err(CalibrationFailure::Deadline)
            ));
        }
        assert!(matches!(
            measure(Input::Two(&[]), cpu, now, &mut [], &mut []),
            Err(CalibrationFailure::Deadline)
        ));
        assert!(elapsed(now).unwrap() > 0);
    }
}
