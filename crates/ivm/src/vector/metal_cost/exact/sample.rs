//! Four original-process sample backings retained across both public patterns.

use super::*;
use iroha_accel::{HostOutput, ProcessResources};

fn check_deadline(started: Instant) -> Result<(), CalibrationFailure> {
    if started.elapsed() >= MAX_CALIBRATION {
        Err(CalibrationFailure::Deadline)
    } else {
        Ok(())
    }
}

fn elapsed(started: Instant) -> Result<u64, CalibrationFailure> {
    u64::try_from(started.elapsed().as_nanos())
        .ok()
        .filter(|&time| time > 0)
        .ok_or(CalibrationFailure::Overloaded)
}

/// Public keys, immutable trial inputs and both independently owned destinations.
struct Inputs {
    keys: HostOutput<[u8; 16]>,
    blocks: HostOutput<[u8; 16]>,
    expected: HostOutput<[u8; 16]>,
    actual: HostOutput<[u8; 16]>,
}

impl Inputs {
    fn prepare(geometry: Geometry, started: Instant) -> Result<Self, CalibrationFailure> {
        check_deadline(started)?;
        let owner = ProcessResources::get().ok_or(CalibrationFailure::Allocation)?;
        let allocate = |len| {
            owner
                .try_host_output(len)
                .map_err(|_| CalibrationFailure::Allocation)
        };
        // Every backing reserves from the same original process owner before
        // allocation. Acquire all four before preparing or timing public work.
        let inputs = Self {
            keys: allocate(geometry.work.rounds())?,
            blocks: allocate(geometry.blocks)?,
            expected: allocate(geometry.blocks)?,
            actual: allocate(geometry.blocks)?,
        };
        check_deadline(started)?;
        Ok(inputs)
    }

    fn fill_pattern(&mut self, nonzero: bool) {
        for (round, key) in self.keys.iter_mut().enumerate() {
            for (lane, byte) in key.iter_mut().enumerate() {
                *byte = if nonzero {
                    (round as u8)
                        .wrapping_mul(19)
                        .wrapping_add(lane as u8)
                        .wrapping_add(11)
                } else {
                    0
                };
            }
        }
        for (block, value) in self.blocks.iter_mut().enumerate() {
            for (lane, byte) in value.iter_mut().enumerate() {
                *byte = if nonzero {
                    (block as u8)
                        .wrapping_mul(37)
                        .wrapping_add((block >> 8) as u8)
                        .wrapping_add(lane as u8)
                } else {
                    0
                };
            }
        }
    }
}

pub(in crate::vector) fn calibrate(
    geometry: Geometry,
    baseline: AesCpuBaseline,
    started: Instant,
) -> Result<Profile, CalibrationFailure> {
    check_deadline(started)?;
    if geometry.work != baseline.work() || !baseline.is_current() {
        return Err(CalibrationFailure::CpuChanged);
    }
    let mut inputs = Inputs::prepare(geometry, started)?;
    let mut cpu_ns = [[0; TRIALS]; PATTERNS];
    let mut metal_ns = [[0; TRIALS]; PATTERNS];
    for pattern in 0..PATTERNS {
        // Public generation and per-trial destination reset are outside the
        // ordinary in-place operation timers; all four owners stay alive.
        inputs.fill_pattern(pattern != 0);
        for trial in 0..TRIALS {
            check_deadline(started)?;
            if !baseline.is_current() {
                return Err(CalibrationFailure::CpuChanged);
            }
            inputs.expected.copy_from_slice(&inputs.blocks);
            cpu_ns[pattern][trial] =
                crate::aes::cpu::measure_backend(geometry.work.direction(), baseline.cpu, || {
                    let started = Instant::now();
                    crate::aes::rounds_cpu_in_place(
                        &mut inputs.expected,
                        &inputs.keys,
                        geometry.work.decrypt(),
                    );
                    elapsed(started)
                })
                .ok_or(CalibrationFailure::CpuChanged)??;
            check_deadline(started)?;
            if !baseline.is_current() {
                return Err(CalibrationFailure::CpuChanged);
            }
            inputs.actual.copy_from_slice(&inputs.blocks);
            let (completed, time) = crate::vector::metal_receipts::with_synthetic(|| {
                let started = Instant::now();
                let completed = crate::vector::metal_aes::measured_in_place(
                    &mut inputs.actual,
                    &inputs.keys,
                    baseline,
                );
                (completed, elapsed(started))
            });
            if !baseline.is_current() {
                return Err(CalibrationFailure::CpuChanged);
            }
            if !completed {
                return Err(CalibrationFailure::BackendUnavailable);
            }
            metal_ns[pattern][trial] = time?;
            if inputs.actual.as_slice() != inputs.expected.as_slice() {
                crate::vector::record_metal_disable("synthetic exact-geometry AES parity mismatch");
                return Err(CalibrationFailure::ParityMismatch);
            }
            check_deadline(started)?;
        }
    }
    Profile::from_trials(geometry, baseline, cpu_ns, metal_ns)
}

#[cfg(test)]
mod tests;
