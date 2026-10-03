//! Original process credit for live sample inputs and both complete output trees.

use super::*;
use crate::{ByteMerkleTree, vector::sha256_cpu::context::Sha256Observed};
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

/// Physical leaves and canonical nodes drop before their original host credit.
/// The output survives the timer, exactly as in ordinary tree construction.
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

    fn measure(
        credit: AllocationReservation,
        build: impl FnOnce() -> Result<(ByteMerkleTree, Sha256Observed), Failure>,
    ) -> Result<(Self, Sha256Observed, u64), Failure> {
        let started = Instant::now();
        let (tree, observed) = build()?;
        let tree = Self {
            tree,
            _credit: credit,
        };
        let time = elapsed(started)?;
        Ok((tree, observed, time))
    }
}

pub(super) fn with_inputs<T>(
    geometry: Geometry,
    started: Instant,
    call: impl FnOnce(&ProcessResources, &mut [u8]) -> Result<T, Failure>,
) -> Result<T, Failure> {
    check_deadline(started)?;
    let owner = ProcessResources::get().ok_or(Failure::Allocation)?;
    let mut input: HostOutput<u8> = owner
        .try_host_output(geometry.byte_len)
        .map_err(|_| Failure::Allocation)?;
    call(owner, &mut input)
}

pub(in crate::vector) fn calibrate(
    geometry: Geometry,
    baseline: Sha256Baseline,
    context: Sha256Context,
    started: Instant,
) -> Result<Profile, Failure> {
    with_inputs(geometry, started, |owner, input| {
        let mut cpu_ns = [[0; TRIALS]; 2];
        let mut metal_ns = [[0; TRIALS]; 2];
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
                // Both outputs coexist for parity. Admit both exact backings
                // before constructing either; do not borrow future refunds.
                let cpu_credit = SampleTree::reserve(owner, geometry)?;
                let metal_credit = SampleTree::reserve(owner, geometry)?;
                let (cpu, observed, time) = SampleTree::measure(cpu_credit, || {
                    ByteMerkleTree::from_bytes_parallel_in_context(
                        black_box(input),
                        geometry.chunk,
                        context.synthetic(),
                    )
                    .map_err(|_| Failure::Allocation)
                })?;
                cpu_ns[pattern][trial] = time;
                if !baseline.accepts(observed, pattern != 0) || !baseline.is_current(context) {
                    return Err(Failure::CpuChanged);
                }
                check_deadline(started)?;
                let (metal, _, time) = super::super::metal_receipts::with_synthetic(|| {
                    SampleTree::measure(metal_credit, || {
                        super::super::metal_merkle::tree_from_bytes(
                            black_box(input),
                            geometry.chunk,
                        )
                        .map(|tree| (tree, Sha256Observed::default()))
                        .ok_or(Failure::Backend)
                    })
                })?;
                metal_ns[pattern][trial] = time;
                if metal.tree.root() != cpu.tree.root() {
                    super::super::record_metal_disable(
                        "synthetic exact-geometry Merkle tree parity mismatch",
                    );
                    return Err(Failure::Parity);
                }
                if !baseline.is_current(context) {
                    return Err(Failure::CpuChanged);
                }
                // Ordinary BuildTree returns a live tree. Its destruction is
                // deliberately excluded from both complete construction timers.
                drop(metal);
                drop(cpu);
                check_deadline(started)?;
            }
        }
        Profile::from_trials(geometry, baseline, cpu_ns, metal_ns)
    })
}

#[cfg(test)]
mod tests;
