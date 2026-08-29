//! Period for re-entrant polling
use std::time::Duration;
/// Period for re-entrant polling
pub(crate) struct RetryPeriod {
    /// The minimum period
    min_period: Duration,
    /// The maximum exponent
    max_exponent: u8,
    /// The current exponent
    exponent: u8,
}
impl RetryPeriod {
    /// Constructs a new object
    pub(crate) const fn new(min_period: Duration, max_exponent: u8) -> Self {
        Self {
            min_period,
            max_exponent,
            exponent: 0,
        }
    }
    /// Return the current delay, then increase the delay for the next failure.
    pub(crate) fn next_period(&mut self) -> Duration {
        let period = self.period();
        self.exponent = self.exponent.saturating_add(1).min(self.max_exponent);
        period
    }

    /// Reset backoff after a successful operation.
    pub(crate) fn reset(&mut self) {
        self.exponent = 0;
    }

    /// Retry period that is calculated as `min_period * 2 ^ min(exponent, max_exponent)`
    pub(crate) fn period(&self) -> Duration {
        let mult = 2_u32.saturating_pow(self.exponent.into());
        self.min_period.saturating_mul(mult)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn increase_exponent_saturates() {
        let mut value = RetryPeriod::new(Duration::from_secs(42), 10);
        let mut last_period = value.next_period();
        for _ in 0..value.max_exponent {
            let new_period = value.next_period();
            assert!(new_period >= last_period);
            last_period = new_period;
        }
        // Further increases should saturate at the maximum exponent
        assert_eq!(value.next_period(), last_period);
        // Repeated calls shouldn't change the period anymore
        for _ in 0..3 {
            assert_eq!(value.next_period(), last_period);
        }
    }

    #[test]
    fn delays_start_at_minimum_then_double_and_reset() {
        let mut value = RetryPeriod::new(Duration::from_secs(3), 4);
        assert_eq!(value.next_period(), Duration::from_secs(3));
        assert_eq!(value.next_period(), Duration::from_secs(6));
        assert_eq!(value.next_period(), Duration::from_secs(12));
        value.reset();
        assert_eq!(value.next_period(), Duration::from_secs(3));
    }
}
