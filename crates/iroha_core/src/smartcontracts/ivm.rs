/// IVM integration helpers.
///
/// This module currently only exposes a runtime cache used by other
/// components for the Iroha Virtual Machine (IVM).
pub mod cache;
/// Host adapter for IVM. See module docs for design and current limitations.
pub mod host;

/// Compute a conservative gas limit for a given cycle budget.
///
/// The interpreter pads traces to exactly `max_cycles` when cycle limits are
/// enabled, charging one unit of gas per padded cycle in addition to the
/// per‑instruction gas schedule. To ensure padding cannot exhaust gas after
/// executing costlier instructions, use the worst-case instruction cost as the
/// multiplier.
#[must_use]
pub fn gas_limit_for_cycles(cycles: u64) -> u64 {
    if cycles == 0 {
        0
    } else {
        cycles.saturating_mul(ivm::gas::max_instruction_cost())
    }
}

/// Convenience helper to derive a gas limit from program metadata.
#[must_use]
pub fn gas_limit_for_meta(meta: &ivm::ProgramMetadata) -> u64 {
    gas_limit_for_cycles(meta.max_cycles)
}
