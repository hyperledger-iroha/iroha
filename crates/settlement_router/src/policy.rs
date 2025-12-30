//! Buffer sizing and guard-rail policies.

use derive_more::{Display, From};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{JsonDeserialize, JsonSerialize},
};
use rust_decimal::Decimal;

use crate::MicroXor;

/// Outcome of evaluating the remaining buffer against the configured policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferStatus {
    /// Buffer has ≥ the configured healthy percentage (default 75 %).
    Normal,
    /// Buffer slid below the healthy guard rail but remains above throttling.
    Alert,
    /// Buffer breached the throttle guard rail (default: <25 %).
    Throttle,
    /// Buffer is critically low and only XOR-denominated inclusion is allowed (default: <10 %).
    XorOnly,
    /// Buffer is nearly empty and settlement should halt until refilled (default: <2 %).
    Halt,
}

/// Capacity of the DS buffer expressed in XOR and hours of coverage.
#[derive(
    Clone, Debug, Eq, PartialEq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct BufferCapacity {
    /// Nominal available XOR (micro-units).
    pub available_xor: MicroXor,
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
    /// Percentage threshold that triggers an alert (defaults to 75 %).
    #[norito(rename = "alert_pct")]
    pub alert: u8,
    /// Percentage threshold that enables throttling (defaults to 25 %).
    #[norito(rename = "throttle_pct")]
    pub throttle: u8,
    /// Percentage threshold that enforces XOR-only inclusion (defaults to 10 %).
    #[norito(rename = "xor_only_pct")]
    pub xor_only: u8,
    /// Percentage threshold that halts inclusion entirely (defaults to 2 %).
    #[norito(rename = "halt_pct")]
    pub halt: u8,
}

impl BufferPolicy {
    /// Roadmap default (alert at 75 %, throttle at 25 %, XOR-only at 10 %, halt at 2 %).
    #[must_use]
    pub const fn roadmap_default() -> Self {
        Self {
            alert: 75,
            throttle: 25,
            xor_only: 10,
            halt: 2,
        }
    }

    fn is_below(remaining: &Decimal, capacity: &Decimal, pct: u8) -> bool {
        remaining * Decimal::from(100_u16) < capacity * Decimal::from(pct)
    }

    /// Evaluate the current buffer status against the roadmap thresholds.
    #[must_use]
    pub fn evaluate(self, remaining: &MicroXor, capacity: &MicroXor) -> BufferStatus {
        if capacity.is_zero() {
            return BufferStatus::Halt;
        }
        if Self::is_below(
            &remaining.into_decimal(),
            &capacity.into_decimal(),
            self.halt,
        ) {
            BufferStatus::Halt
        } else if Self::is_below(
            &remaining.into_decimal(),
            &capacity.into_decimal(),
            self.xor_only,
        ) {
            BufferStatus::XorOnly
        } else if Self::is_below(
            &remaining.into_decimal(),
            &capacity.into_decimal(),
            self.throttle,
        ) {
            BufferStatus::Throttle
        } else if Self::is_below(
            &remaining.into_decimal(),
            &capacity.into_decimal(),
            self.alert,
        ) {
            BufferStatus::Alert
        } else {
            BufferStatus::Normal
        }
    }

    /// Whether the buffer has fallen below the alert threshold.
    ///
    /// Treats any state other than [`BufferStatus::Normal`] as a soft breach so
    /// operators can warn before throttling kicks in.
    #[must_use]
    pub fn is_soft_breached(self, remaining: &MicroXor, capacity: &MicroXor) -> bool {
        !matches!(self.evaluate(remaining, capacity), BufferStatus::Normal)
    }

    /// Whether the buffer is low enough to require XOR-only inclusion or a halt.
    #[must_use]
    pub fn is_hard_breached(self, remaining: &MicroXor, capacity: &MicroXor) -> bool {
        matches!(
            self.evaluate(remaining, capacity),
            BufferStatus::XorOnly | BufferStatus::Halt
        )
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{BufferPolicy, BufferStatus};
    use crate::MicroXor;

    fn micro(value: i64) -> MicroXor {
        MicroXor::new(Decimal::from(value))
    }

    #[test]
    fn evaluate_buffer_thresholds() {
        let policy = BufferPolicy::roadmap_default();
        let capacity = micro(1_000_000);
        let healthy = micro(2_000_000);
        assert_eq!(policy.evaluate(&healthy, &capacity), BufferStatus::Normal);

        let alert = micro(700_000);
        assert_eq!(policy.evaluate(&alert, &capacity), BufferStatus::Alert);

        let throttle = micro(200_000);
        assert_eq!(
            policy.evaluate(&throttle, &capacity),
            BufferStatus::Throttle
        );

        let xor_only = micro(50_000);
        assert_eq!(policy.evaluate(&xor_only, &capacity), BufferStatus::XorOnly);

        let halt = micro(5_000);
        assert_eq!(policy.evaluate(&halt, &capacity), BufferStatus::Halt);
    }

    #[test]
    fn soft_and_hard_breach_helpers() {
        let policy = BufferPolicy::roadmap_default();
        let capacity = micro(1_000_000);

        assert!(!policy.is_soft_breached(&micro(2_000_000), &capacity));
        assert!(!policy.is_hard_breached(&micro(2_000_000), &capacity));

        assert!(policy.is_soft_breached(&micro(700_000), &capacity));
        assert!(!policy.is_hard_breached(&micro(700_000), &capacity));

        assert!(policy.is_soft_breached(&micro(50_000), &capacity));
        assert!(policy.is_hard_breached(&micro(50_000), &capacity));
    }
}
