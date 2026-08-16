//! Canonical guest-stack policy for the IVM V1 ABI.
//!
//! Guest-visible memory geometry is consensus behavior: programs can observe
//! the stack top in `r31`, and memory permissions are checked against the same
//! boundary.  The first-release ABI therefore has one closed policy instead of
//! accepting node-local limits or gas multipliers.
/// Immutable guest-stack policy selected by the IVM ABI.
///
/// V1 is deliberately the only constructible policy.  A future policy must be
/// introduced together with a new ABI version rather than as a local runtime setting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IvmStackPolicy {
    /// Canonical first-release IVM ABI policy.
    V1,
}
impl IvmStackPolicy {
    /// Minimum guest stack exposed even to very small gas limits.
    #[must_use]
    pub const fn minimum_stack_bytes(self) -> u64 {
        match self {
            Self::V1 => 64 * 1024,
        }
    }
    /// Maximum guest stack exposed by this ABI.
    #[must_use]
    pub const fn maximum_stack_bytes(self) -> u64 {
        match self {
            Self::V1 => 4 * 1024 * 1024,
        }
    }
    /// Stack bytes made available per unit of gas before applying ABI bounds.
    #[must_use]
    pub const fn bytes_per_gas(self) -> u64 {
        match self {
            Self::V1 => 4,
        }
    }
    /// Alignment of the guest stack boundary.
    #[must_use]
    pub const fn stack_alignment_bytes(self) -> u64 {
        match self {
            Self::V1 => 16,
        }
    }
    /// Derive the canonical guest stack limit for a gas limit.
    #[must_use]
    pub const fn stack_limit_for_gas(self, gas_limit: u64) -> u64 {
        let derived = gas_limit.saturating_mul(self.bytes_per_gas());
        let bounded = if derived < self.minimum_stack_bytes() {
            self.minimum_stack_bytes()
        } else if derived > self.maximum_stack_bytes() {
            self.maximum_stack_bytes()
        } else {
            derived
        };
        let alignment = self.stack_alignment_bytes();
        bounded - bounded % alignment
    }
}
