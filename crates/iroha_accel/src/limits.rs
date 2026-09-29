//! Required finite explicit-payload admission limits.

/// Required finite inputs for explicit buffer custody; zero disables that use.
///
/// There is deliberately no default or automatic free-memory heuristic here.
/// Configuration owners must provide reviewed ceilings before constructing the
/// process owner; the contract-cache budget is unrelated to these pools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuResourceLimits {
    /// Aggregate explicitly allocated ordinary host buffer bytes.
    pub host_bytes: usize,
    /// Aggregate requested page-locked host buffer bytes.
    pub pinned_bytes: usize,
    /// Aggregate requested device buffer bytes.
    pub device_bytes: usize,
    /// Work aggregates concurrently prepared, submitted, or retained after failure.
    pub in_flight: usize,
}
