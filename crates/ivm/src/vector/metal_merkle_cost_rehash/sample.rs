//! Original process funding for retained synthetic trees and complete updates.

use super::*;
use crate::ByteMerkleTree;
use iroha_accel::{HostOutput, ProcessResources};
use iroha_allocation::AllocationReservation;
use std::{alloc::Layout, hint::black_box};

fn check_deadline(started: Instant) -> Result<(), Failure> {
    if started.elapsed() >= MAX_CALIBRATION {
        Err(Failure::Deadline)
    } else {
        Ok(())
    }
}

fn elapsed(started: Instant) -> Result<u64, Failure> {
    u64::try_from(started.elapsed().as_nanos())
        .ok()
        .filter(|&value| value > 0)
        .ok_or(Failure::Overloaded)
}

/// Retained leaf/node backing is destroyed before its original process credit.
struct SampleTree {
    tree: ByteMerkleTree,
    _credit: AllocationReservation,
}

impl SampleTree {
    fn reserve(
        owner: &ProcessResources,
        geometry: Geometry,
    ) -> Result<AllocationReservation, Failure> {
        let bytes = ByteMerkleTree::memory_plan(geometry.leaves)
            .map_err(|_| Failure::Allocation)?
            .requested_bytes();
        owner
            .try_host_reservation(Layout::array::<u8>(bytes).map_err(|_| Failure::Allocation)?)
            .map_err(|_| Failure::Allocation)
    }

    fn new(geometry: Geometry, credit: AllocationReservation) -> Result<Self, Failure> {
        Ok(Self {
            tree: ByteMerkleTree::new(geometry.leaves, geometry.chunk)
                .map_err(|_| Failure::Allocation)?,
            _credit: credit,
        })
    }
}

pub(super) fn with_inputs<T>(
    geometry: Geometry,
    started: Instant,
    call: impl FnOnce(&mut [u8], &ByteMerkleTree, &ByteMerkleTree) -> Result<T, Failure>,
) -> Result<T, Failure> {
    check_deadline(started)?;
    let owner = ProcessResources::get().ok_or(Failure::Allocation)?;
    let mut input: HostOutput<u8> = owner
        .try_host_output(geometry.byte_len)
        .map_err(|_| Failure::Allocation)?;
    // No tree is allocated until both simultaneous retained backings are funded.
    let cpu_credit = SampleTree::reserve(owner, geometry)?;
    let metal_credit = SampleTree::reserve(owner, geometry)?;
    let cpu = SampleTree::new(geometry, cpu_credit)?;
    let metal = SampleTree::new(geometry, metal_credit)?;
    check_deadline(started)?;
    call(&mut input, &cpu.tree, &metal.tree)
}

pub(in crate::vector) fn calibrate(
    geometry: Geometry,
    baseline: Sha256Baseline,
    context: Sha256Context,
    started: Instant,
) -> Result<Profile, Failure> {
    // Preserve the original partial arrays when admission or sampling refuses.
    // No receipt is published until all sample timers and cleanup have finished.
    let mut cpu_ns = [[0; TRIALS]; 2];
    let mut metal_ns = [[0; TRIALS]; 2];
    let result = with_inputs(geometry, started, |input, cpu, metal| {
        for pattern in 0..2 {
            for (index, byte) in input.iter_mut().enumerate() {
                *byte = if pattern == 0 {
                    0
                } else {
                    (index as u8)
                        .wrapping_mul(31)
                        .wrapping_add((index >> 8) as u8)
                        .wrapping_add(7)
                };
            }
            for trial in 0..TRIALS {
                check_deadline(started)?;
                if !baseline.is_current(context) {
                    return Err(Failure::CpuChanged);
                }
                let cpu_start = Instant::now();
                let observed =
                    cpu.rehash_parallel_in_context(black_box(input), context.synthetic());
                let expected = cpu.root();
                cpu_ns[pattern][trial] = elapsed(cpu_start)?;
                if !baseline.accepts(observed, pattern != 0 && !input.is_empty())
                    || !baseline.is_current(context)
                {
                    return Err(Failure::CpuChanged);
                }
                check_deadline(started)?;
                let (actual, time) = super::super::metal_receipts::with_synthetic(|| {
                    let gpu_start = Instant::now();
                    let completed = super::super::metal_merkle::rehash_tree(
                        metal,
                        black_box(input),
                        baseline,
                        context,
                    );
                    let actual = completed.then(|| metal.root());
                    (actual, elapsed(gpu_start))
                });
                let actual = actual.ok_or(Failure::Backend)?;
                metal_ns[pattern][trial] = time?;
                if actual != expected {
                    super::super::record_metal_disable(
                        "synthetic exact retained Merkle rehash parity mismatch",
                    );
                    return Err(Failure::Parity);
                }
                if !baseline.is_current(context) {
                    return Err(Failure::CpuChanged);
                }
                check_deadline(started)?;
            }
        }
        // Both fixed trees remain live beyond every update timer.
        Profile::from_trials(geometry, baseline, cpu_ns, metal_ns)
    });
    #[cfg(all(test, feature = "metal-hardware-tests"))]
    super::super::metal_receipts::timing::record(cpu_ns, metal_ns);
    result
}

#[cfg(test)]
mod tests;
