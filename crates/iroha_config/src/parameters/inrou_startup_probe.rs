//! Checked startup probe geometry derived from the configured host ceiling.
//! No free-capacity observation or alternate default supplies authority.

use iroha_data_model::soracloud::{
    SORA_INROU_CPU_MILLIS_ALIGNMENT_V1, SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1,
    SORA_INROU_MAX_CPU_MILLIS_V1, SORA_INROU_MAX_VCPUS_V1, SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1,
    SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1, SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1,
    SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1, SoraResourceLimitsV1,
};
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

/// Exact backend probe envelope derived from one selected hosted allocation.
///
/// This is a projection, not a successful startup attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InrouStartupProbeShapeV1 {
    resources: SoraResourceLimitsV1,
    vcpus: u32,
    memory_mib: u64,
}
impl InrouStartupProbeShapeV1 {
    /// Derive the largest protocol-valid probe inside an exact configured host envelope.
    /// This projection grants no allocation, signature or successful-probe authority.
    ///
    /// # Errors
    /// Rejects envelopes unable to contain the mandatory VMM overhead and guest minima.
    pub fn from_host_envelope(cpu_millis: u64, memory_bytes: u64) -> Result<Self, &'static str> {
        let cpu = cpu_millis
            .checked_sub(SORA_INROU_VMM_CPU_OVERHEAD_MILLIS_V1)
            .ok_or("CPU overhead underflow")?
            .min(u64::from(SORA_INROU_MAX_CPU_MILLIS_V1));
        let alignment = u64::from(SORA_INROU_CPU_MILLIS_ALIGNMENT_V1);
        let cpu =
            u32::try_from(cpu / alignment * alignment).map_err(|_| "CPU projection overflow")?;
        let memory = memory_bytes
            .checked_sub(SORA_INROU_VMM_MEMORY_OVERHEAD_BYTES_V1)
            .ok_or("memory overhead underflow")?;
        let memory =
            memory / SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1 * SORA_INROU_MEMORY_ALIGNMENT_BYTES_V1;
        let resources = SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(cpu).ok_or("zero probe CPU")?,
            memory_bytes: NonZeroU64::new(memory).ok_or("zero probe memory")?,
            ephemeral_storage_bytes: NonZeroU64::new(
                SORA_INROU_EPHEMERAL_STORAGE_ALIGNMENT_BYTES_V1,
            )
            .expect("protocol alignment is positive"),
            max_open_files_per_process: NonZeroU32::new(SORA_INROU_MIN_OPEN_FILES_PER_PROCESS_V1)
                .expect("protocol descriptors are positive"),
            max_tasks: NonZeroU16::new(1).expect("one probe task"),
        };
        resources
            .validate_for_inrou()
            .map_err(|_| "probe projection is not enforceable")?;
        let vcpus = cpu.div_ceil(1000);
        if vcpus == 0
            || vcpus > SORA_INROU_MAX_VCPUS_V1
            || resources
                .checked_inrou_host_cpu_millis()
                .is_none_or(|value| value > cpu_millis)
            || resources
                .checked_inrou_host_memory_bytes()
                .is_none_or(|value| value > memory_bytes)
        {
            return Err("probe exceeds its selected host allocation");
        }
        Ok(InrouStartupProbeShapeV1 {
            resources,
            vcpus,
            memory_mib: memory / (1024 * 1024),
        })
    }
    /// Guest resource limits used to derive the real cgroup limits.
    #[must_use]
    pub const fn resources(self) -> SoraResourceLimitsV1 {
        self.resources
    }
    /// Exact QEMU virtual-CPU count for this probe envelope.
    #[must_use]
    pub const fn vcpus(self) -> u32 {
        self.vcpus
    }
    /// Exact QEMU guest RAM in MiB.
    #[must_use]
    pub const fn memory_mib(self) -> u64 {
        self.memory_mib
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIB: u64 = 1024 * 1024;
    #[test]
    fn compact_canary_probe_fits_exact_cpu_memory_and_vmm_ceiling() {
        let shape = InrouStartupProbeShapeV1::from_host_envelope(1000, 768 * MIB).unwrap();
        assert_eq!(shape.resources().cpu_millis.get(), 750);
        assert_eq!(shape.resources().memory_bytes.get(), 512 * MIB);
        assert_eq!(
            shape.resources().checked_inrou_host_cpu_millis(),
            Some(1000)
        );
        assert_eq!(
            shape.resources().checked_inrou_host_memory_bytes(),
            Some(768 * MIB)
        );
        assert_eq!(shape.vcpus(), 1);
        assert_eq!(shape.memory_mib(), 512);
    }
    #[test]
    fn absent_overhead_and_below_minimum_envelopes_are_rejected() {
        for (cpu, memory) in [
            (0, 768 * MIB),
            (250, 768 * MIB),
            (259, 768 * MIB),
            (1000, 0),
            (1000, 256 * MIB),
            (1000, 384 * MIB - 1),
        ] {
            assert!(InrouStartupProbeShapeV1::from_host_envelope(cpu, memory).is_err());
        }
    }
    #[test]
    fn geometry_stays_inside_exact_envelope_at_alignment_and_vcpu_edges() {
        for (host_cpu, expected_cpu, vcpus) in [
            (260, 10, 1),
            (1019, 760, 1),
            (1250, 1000, 1),
            (1260, 1010, 2),
            (4250, 4000, 4),
            (8000, 4000, 4),
        ] {
            let shape =
                InrouStartupProbeShapeV1::from_host_envelope(host_cpu, 768 * MIB + 1).unwrap();
            assert_eq!(shape.resources().cpu_millis.get(), expected_cpu);
            assert_eq!(shape.vcpus(), vcpus);
            assert_eq!(shape.memory_mib(), 512);
            assert!(shape.resources().checked_inrou_host_cpu_millis().unwrap() <= host_cpu);
        }
    }
}
