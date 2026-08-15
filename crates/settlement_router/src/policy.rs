//! Buffer sizing and guard-rail policies.
use crate::{Numeric, XorQuantity, XorQuantityError};
use derive_more::{Display, From};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
/// Outcome of evaluating the remaining buffer against the configured policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferStatus {
    /// Buffer has at least the configured healthy percentage.
    Normal,
    /// Buffer is below the healthy guard rail but remains above throttling.
    Alert,
    /// Buffer breached the throttle guard rail.
    Throttle,
    /// Buffer is critically low and only XOR-denominated inclusion is allowed.
    XorOnly,
    /// Buffer is nearly empty and settlement must halt until refilled.
    Halt,
}
/// Buffer-policy evaluation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BufferPolicyError {
    /// Thresholds must be within 0..=100 and ordered from highest to lowest.
    #[error("buffer thresholds must satisfy 100 >= alert >= throttle >= xor_only >= halt")]
    InvalidThresholds,
    /// Threshold calculation exceeded the exact XOR domain.
    #[error("buffer threshold arithmetic failed: {0}")]
    Arithmetic(#[from] XorQuantityError),
}
/// Capacity of the dataspace buffer expressed in exact XOR and coverage hours.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct BufferCapacity {
    /// Nominal available XOR.
    pub available_xor: XorQuantity,
    /// Rolling window the buffer is expected to cover.
    pub horizon_hours: u16,
}
/// Thresholds used when classifying the remaining buffer into operational states.
#[derive(
    Clone,
    Copy,
    Debug,
    Display,
    Eq,
    PartialEq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    From,
)]
#[display(
    "alert {}%, throttle {}%, xor-only {}%, halt {}%",
    alert,
    throttle,
    xor_only,
    halt
)]
pub struct BufferPolicy {
    /// Percentage threshold that triggers an alert (defaults to 75%).
    #[norito(rename = "alert_pct")]
    pub alert: u8,
    /// Percentage threshold that enables throttling (defaults to 25%).
    #[norito(rename = "throttle_pct")]
    pub throttle: u8,
    /// Percentage threshold that enforces XOR-only inclusion (defaults to 10%).
    #[norito(rename = "xor_only_pct")]
    pub xor_only: u8,
    /// Percentage threshold that halts inclusion (defaults to 2%).
    #[norito(rename = "halt_pct")]
    pub halt: u8,
}
impl BufferPolicy {
    /// Roadmap default (alert 75%, throttle 25%, XOR-only 10%, halt 2%).
    #[must_use]
    pub const fn roadmap_default() -> Self {
        Self {
            alert: 75,
            throttle: 25,
            xor_only: 10,
            halt: 2,
        }
    }
    /// Validate threshold bounds and ordering.
    pub const fn validate(self) -> Result<(), BufferPolicyError> {
        if self.alert > 100
            || self.alert < self.throttle
            || self.throttle < self.xor_only
            || self.xor_only < self.halt
        {
            return Err(BufferPolicyError::InvalidThresholds);
        }
        Ok(())
    }
    fn is_below(
        remaining: &XorQuantity,
        capacity: &XorQuantity,
        pct: u8,
    ) -> Result<bool, BufferPolicyError> {
        let factor = Numeric::new(u128::from(pct), 2);
        let threshold = capacity
            .as_quantity()
            .try_mul_decimal(&factor)
            .map_err(XorQuantityError::from)
            .and_then(XorQuantity::try_from_quantity)?;
        Ok(remaining < &threshold)
    }
    /// Evaluate the current buffer status against the configured thresholds.
    ///
    /// A zero-capacity buffer is always halted. Boundary equality belongs to
    /// the less severe state; only values strictly below a threshold breach it.
    pub fn evaluate(
        self,
        remaining: &XorQuantity,
        capacity: &XorQuantity,
    ) -> Result<BufferStatus, BufferPolicyError> {
        self.validate()?;
        if capacity.is_zero() {
            return Ok(BufferStatus::Halt);
        }
        if Self::is_below(remaining, capacity, self.halt)? {
            Ok(BufferStatus::Halt)
        } else if Self::is_below(remaining, capacity, self.xor_only)? {
            Ok(BufferStatus::XorOnly)
        } else if Self::is_below(remaining, capacity, self.throttle)? {
            Ok(BufferStatus::Throttle)
        } else if Self::is_below(remaining, capacity, self.alert)? {
            Ok(BufferStatus::Alert)
        } else {
            Ok(BufferStatus::Normal)
        }
    }
    /// Whether the buffer has fallen below the alert threshold.
    pub fn is_soft_breached(
        self,
        remaining: &XorQuantity,
        capacity: &XorQuantity,
    ) -> Result<bool, BufferPolicyError> {
        Ok(!matches!(
            self.evaluate(remaining, capacity)?,
            BufferStatus::Normal
        ))
    }
    /// Whether the buffer requires XOR-only inclusion or a halt.
    pub fn is_hard_breached(
        self,
        remaining: &XorQuantity,
        capacity: &XorQuantity,
    ) -> Result<bool, BufferPolicyError> {
        Ok(matches!(
            self.evaluate(remaining, capacity)?,
            BufferStatus::XorOnly | BufferStatus::Halt
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::{BufferPolicy, BufferPolicyError, BufferStatus};
    use crate::XorQuantity;
    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }
    #[test]
    fn evaluate_buffer_thresholds_and_exact_boundaries() {
        let policy = BufferPolicy::roadmap_default();
        let capacity = xor("1000000");
        assert_eq!(
            policy.evaluate(&xor("2000000"), &capacity),
            Ok(BufferStatus::Normal)
        );
        assert_eq!(
            policy.evaluate(&xor("750000"), &capacity),
            Ok(BufferStatus::Normal)
        );
        assert_eq!(
            policy.evaluate(&xor("749999.999999999"), &capacity),
            Ok(BufferStatus::Alert)
        );
        assert_eq!(
            policy.evaluate(&xor("200000"), &capacity),
            Ok(BufferStatus::Throttle)
        );
        assert_eq!(
            policy.evaluate(&xor("50000"), &capacity),
            Ok(BufferStatus::XorOnly)
        );
        assert_eq!(
            policy.evaluate(&xor("5000"), &capacity),
            Ok(BufferStatus::Halt)
        );
    }
    #[test]
    fn zero_capacity_halts() {
        assert_eq!(
            BufferPolicy::roadmap_default().evaluate(&xor("999"), &XorQuantity::zero()),
            Ok(BufferStatus::Halt)
        );
    }
    #[test]
    fn rejects_invalid_threshold_order_and_range() {
        for policy in [
            BufferPolicy {
                alert: 101,
                ..BufferPolicy::roadmap_default()
            },
            BufferPolicy {
                alert: 20,
                throttle: 30,
                ..BufferPolicy::roadmap_default()
            },
            BufferPolicy {
                xor_only: 1,
                halt: 2,
                ..BufferPolicy::roadmap_default()
            },
        ] {
            assert_eq!(
                policy.evaluate(&xor("1"), &xor("10")),
                Err(BufferPolicyError::InvalidThresholds)
            );
        }
    }
    #[test]
    fn soft_and_hard_breach_helpers_propagate_results() {
        let policy = BufferPolicy::roadmap_default();
        let capacity = xor("1000000");
        assert_eq!(
            policy.is_soft_breached(&xor("2000000"), &capacity),
            Ok(false)
        );
        assert_eq!(
            policy.is_hard_breached(&xor("2000000"), &capacity),
            Ok(false)
        );
        assert_eq!(policy.is_soft_breached(&xor("700000"), &capacity), Ok(true));
        assert_eq!(
            policy.is_hard_breached(&xor("700000"), &capacity),
            Ok(false)
        );
        assert_eq!(policy.is_hard_breached(&xor("50000"), &capacity), Ok(true));
    }
}
