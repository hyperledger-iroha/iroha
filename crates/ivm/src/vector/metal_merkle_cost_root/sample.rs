//! Original process funding and identical ordinary byte-root operation timing.

use super::*;
use iroha_accel::{HostOutput, ProcessResources};
use iroha_allocation::AllocationReservation;
use iroha_crypto::MerkleTree;
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

/// Field order drops the actual foreign tree before returning its original credit.
struct CpuTree {
    tree: MerkleTree<[u8; 32]>,
    _credit: AllocationReservation,
}
impl CpuTree {
    fn reserve(
        owner: &ProcessResources,
        geometry: Geometry,
    ) -> Result<AllocationReservation, Failure> {
        let bytes = MerkleTree::<[u8; 32]>::repeated_sha256_node_allocation_bytes(geometry.leaves)
            .map_err(|_| Failure::Allocation)?;
        owner
            .try_host_reservation(Layout::array::<u8>(bytes).map_err(|_| Failure::Allocation)?)
            .map_err(|_| Failure::Allocation)
    }
    fn new(
        data: &[u8],
        geometry: Geometry,
        credit: AllocationReservation,
    ) -> Result<Self, Failure> {
        let tree =
            MerkleTree::from_byte_chunks(data, geometry.chunk).map_err(|_| Failure::Allocation)?;
        Ok(Self {
            tree,
            _credit: credit,
        })
    }
    fn finish(self, started: Instant) -> Result<([u8; 32], u64), Failure> {
        let root = *self
            .tree
            .root()
            .expect("bounded nonempty byte-root")
            .as_ref();
        let Self {
            tree,
            _credit: credit,
        } = self;
        drop(tree);
        let time = elapsed(started)?;
        drop(credit);
        Ok((root, time))
    }
}

/// Only the checked byte count and chunk width enter synthetic construction.
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
                // Admission is outside timing: ordinary CPU fallback owns its
                // destination elsewhere. Its actual construction and drop are timed.
                let credit = CpuTree::reserve(owner, geometry)?;
                let cpu_start = Instant::now();
                let tree = CpuTree::new(black_box(input), geometry, credit)?;
                let (cpu, time) = tree.finish(cpu_start)?;
                cpu_ns[pattern][trial] = time;
                check_deadline(started)?;
                // Identical complete production operation, including admission,
                // block preparation, transfers, native work, readback and cleanup.
                let (actual, time) = super::super::metal_receipts::with_synthetic(|| {
                    let gpu_start = Instant::now();
                    let actual = super::super::metal_merkle::root_from_bytes(
                        black_box(input),
                        geometry.chunk,
                    );
                    (actual, elapsed(gpu_start))
                });
                let actual = actual.ok_or(Failure::Backend)?;
                metal_ns[pattern][trial] = time?;
                if actual != cpu {
                    super::super::record_metal_disable(
                        "synthetic exact-geometry Merkle root parity mismatch",
                    );
                    return Err(Failure::Parity);
                }
                check_deadline(started)?;
            }
        }
        Profile::from_trials(geometry, cpu_ns, metal_ns)
    })
}

#[cfg(test)]
mod tests;
