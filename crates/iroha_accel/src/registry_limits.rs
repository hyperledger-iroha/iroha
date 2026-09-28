//! File-owned finite process resource policy, independent of performance qualification.

use crate::GpuResourceLimits;

/// Finite physical-owner bounds, supplied by the process configuration owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryLimits {
    /// Exact Rust registry/control allocation budget (separate from payloads).
    pub metadata_bytes: usize,
    /// Lifetime-observed physical UUID records, including quarantined devices.
    pub devices: usize,
    /// Maximum ordinal probes in one bounded discovery pass.
    pub discovery_ordinals: u32,
    /// Simultaneously retained native module owners across all devices.
    pub modules: usize,
    /// Simultaneously retained native nonblocking streams across all devices.
    pub streams: usize,
    /// Maximum admitted immutable PTX byte length, including its terminating NUL.
    pub artifact_bytes: usize,
    /// Aggregate payload and in-flight bounds.
    pub work: GpuResourceLimits,
}

impl RegistryLimits {
    /// Ordinary enabled process envelope. These are resource policy ceilings,
    /// not benchmark claims or measurements of driver-private allocations.
    /// Callers still pass them explicitly to the one physical owner constructor.
    pub const STANDARD: Self = Self {
        metadata_bytes: 16 * 1024 * 1024,
        devices: 16,
        discovery_ordinals: 64,
        // Aggregate ceiling shared by all backend registries and consumers.
        modules: 16 * 19,
        streams: 16,
        artifact_bytes: 16 * 1024 * 1024,
        work: GpuResourceLimits {
            host_bytes: 256 * 1024 * 1024,
            pinned_bytes: 256 * 1024 * 1024,
            device_bytes: 1024 * 1024 * 1024,
            in_flight: 16,
        },
    };
}
