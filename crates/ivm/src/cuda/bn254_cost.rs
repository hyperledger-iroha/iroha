//! Bounded synthetic whole-operation measurements; no production values or timings.

use super::*;
use crate::{
    bn254_vec::{BatchOperation, cpu_batch_into},
    cuda_cost::{CalibrationFailure, CostProfile, SIZES, TRIALS, TrialSample},
    field_dispatch::{FieldArithmetic, field_impl},
};
use iroha_accel::ProcessResources;
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

pub(super) fn calibrate(
    operation: BatchOperation,
    cpu: &'static dyn FieldArithmetic,
    deadline: Instant,
) -> Result<CostProfile, CalibrationFailure> {
    checked_now(deadline, cpu)?;
    let _probe =
        super::super::imp::SelftestRunningGuard::enter().ok_or(CalibrationFailure::Deferred)?;
    let resources = ProcessResources::get().ok_or(CalibrationFailure::Deferred)?;
    let allocate = || {
        resources
            .try_host_output::<[u64; 4]>(SIZES[SIZES.len() - 1])
            .map_err(|_| CalibrationFailure::Deferred)
    };
    // All scratch retains original process reservations; no real operands are
    // copied here and no uncharged Vec is constructed before resource admission.
    let mut left = allocate()?;
    let mut right = allocate()?;
    let mut cpu_output = allocate()?;
    let mut gpu_output = allocate()?;
    let (golden_left, golden_right) = golden_operands();
    let kernel = kernel(operation);
    let expected =
        golden_output(kernel, &golden_left, &golden_right).ok_or(CalibrationFailure::Deferred)?;
    for index in 0..left.len() {
        left[index] = golden_left[index % golden_left.len()];
        right[index] = golden_right[index % golden_right.len()];
    }
    let mut trials = [TrialSample::default(); SIZES.len()];
    for (sample, count) in SIZES.into_iter().enumerate() {
        for trial in 0..TRIALS {
            let start = checked_now(deadline, cpu)?;
            cpu_batch_into(
                operation,
                cpu,
                black_box(&left[..count]),
                black_box(&right[..count]),
                black_box(&mut cpu_output[..count]),
            );
            trials[sample].cpu_ns[trial] = elapsed(start)?;
            // CPU selection also needs the independent relation to hold. A bad
            // CPU baseline supplies no cost authority and does not blame a GPU.
            if cpu_output[..count]
                .iter()
                .enumerate()
                .any(|(index, value)| *value != expected[index % expected.len()])
            {
                return Err(CalibrationFailure::Deferred);
            }
            let start = checked_now(deadline, cpu)?;
            // The ordinary adapter repeats canonical validation before selection;
            // include that extra scan here. The outer batch validation is common
            // to CPU and GPU, but this second scan is GPU-path work.
            if !crate::bn254_vec::valid_batch(
                black_box(&left[..count]),
                black_box(&right[..count]),
                count,
            ) {
                return Err(CalibrationFailure::Deferred);
            }
            complete_current(
                kernel,
                black_box(&left[..count]),
                black_box(&right[..count]),
                black_box(&mut gpu_output[..count]),
                || cpu.type_id() == field_impl().type_id(),
            )
            .map_err(|error| match error {
                CudaFailure::Busy | CudaFailure::Capacity | CudaFailure::Unavailable => {
                    CalibrationFailure::Deferred
                }
                _ => CalibrationFailure::Backend,
            })?;
            // complete_current includes transfers, launch, waits, shape/canonical
            // validation, caller copyback and final native/host staging cleanup.
            trials[sample].gpu_ns[trial] = elapsed(start)?;
            if gpu_output[..count]
                .iter()
                .enumerate()
                .any(|(index, value)| *value != expected[index % expected.len()])
            {
                crate::cuda_dispatch::quarantine_current_kernel();
                return Err(CalibrationFailure::Parity);
            }
            checked_now(deadline, cpu)?;
        }
    }
    CostProfile::from_trials(trials).ok_or(CalibrationFailure::Deferred)
}
