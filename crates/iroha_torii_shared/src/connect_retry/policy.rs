use core::{convert::TryFrom, time::Duration};

/// Default base delay (in milliseconds) for Connect retry policies.
pub const DEFAULT_BASE_DELAY_MS: u64 = 5_000;
/// Maximum back-off cap (in milliseconds) for Connect retry policies.
pub const DEFAULT_MAX_DELAY_MS: u64 = 60_000;

/// Deterministic exponential back-off with full jitter.
///
/// The policy follows the "full jitter" variant described by the AWS Architecture
/// blog. Each attempt doubles the delay until the configured cap is reached, and
/// a per-attempt pseudo-random sample is picked in the `[0, cap]` range.
///
/// Instead of maintaining RNG state, we derive jitter deterministically from the
/// Connect session identifier (or any caller-provided seed) plus the attempt
/// counter. This keeps reconnection behaviour consistent across SDKs and makes
/// the algorithm easy to port. The sampling primitive is a `SplitMix64` round.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl Policy {
    /// Construct a policy with the provided base delay and cap (milliseconds).
    #[must_use]
    pub const fn new(base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            base_delay_ms,
            max_delay_ms,
        }
    }

    /// Default Connect policy: base 5 s, cap 60 s.
    #[must_use]
    pub const fn default() -> Self {
        Self::new(DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_DELAY_MS)
    }

    /// Return the maximum (capped) delay in milliseconds for `attempt`.
    #[must_use]
    pub fn cap_for_attempt_ms(&self, attempt: u32) -> u64 {
        // Clamp shift to avoid undefined behaviour when attempt ≥ 128.
        let shift = attempt.min(63);
        let base = u128::from(self.base_delay_ms);
        let cap = u128::from(self.max_delay_ms);
        let raw = base << shift;
        u64::try_from(raw.min(cap)).expect("cap is sourced from a u64 configuration")
    }

    /// Compute the deterministic delay (milliseconds) for `attempt` using `seed`.
    ///
    /// The seed should be stable per session (e.g., the Connect session ID) so
    /// that every client samples an identical jitter series.
    #[must_use]
    pub fn delay_ms(&self, attempt: u32, seed: impl AsRef<[u8]>) -> u64 {
        let cap = self.cap_for_attempt_ms(attempt);
        if cap == 0 {
            return 0;
        }
        // Full jitter: sample uniformly in [0, cap].
        let span = cap + 1;
        let sample = deterministic_sample(seed.as_ref(), attempt);
        sample % span
    }

    /// Same as [`Self::delay_ms`] but returns a [`Duration`].
    #[must_use]
    pub fn delay(&self, attempt: u32, seed: impl AsRef<[u8]>) -> Duration {
        Duration::from_millis(self.delay_ms(attempt, seed))
    }

    /// Accessor for the configured base delay (milliseconds).
    #[must_use]
    pub const fn base_delay_ms(&self) -> u64 {
        self.base_delay_ms
    }

    /// Accessor for the configured cap (milliseconds).
    #[must_use]
    pub const fn max_delay_ms(&self) -> u64 {
        self.max_delay_ms
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_DELAY_MS)
    }
}

const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const ATTEMPT_MIX: u64 = 0xD1B5_4A32_D192_ED03;
const SEED_INIT: u64 = 0xA076_1D64_78BD_642F;

fn deterministic_sample(seed: &[u8], attempt: u32) -> u64 {
    // Fold the arbitrary-length seed into a 64-bit state using SplitMix64 rounds.
    let mut state = SEED_INIT;
    for chunk in seed.chunks(8) {
        state = state.wrapping_add(GAMMA);
        state ^= load_le(chunk).wrapping_mul(GAMMA);
        state = splitmix64(state);
    }
    state = state.wrapping_add(GAMMA);
    state ^= u64::from(attempt).wrapping_mul(ATTEMPT_MIX);
    splitmix64(state)
}

fn load_le(chunk: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf[..chunk.len()].copy_from_slice(chunk);
    u64::from_le_bytes(buf)
}

fn splitmix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_DELAY_MS, Policy, deterministic_sample};

    const SID_ZERO: [u8; 32] = [0; 32];
    const SID_SEQ: [u8; 32] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
        0x1E, 0x1F,
    ];

    #[test]
    fn cap_saturates_at_max() {
        let policy = Policy::default();
        assert_eq!(policy.base_delay_ms(), DEFAULT_BASE_DELAY_MS);
        assert_eq!(policy.max_delay_ms(), DEFAULT_MAX_DELAY_MS);
        assert_eq!(policy.cap_for_attempt_ms(0), 5_000);
        assert_eq!(policy.cap_for_attempt_ms(1), 10_000);
        assert_eq!(policy.cap_for_attempt_ms(2), 20_000);
        assert_eq!(policy.cap_for_attempt_ms(3), 40_000);
        assert_eq!(policy.cap_for_attempt_ms(4), 60_000);
        assert_eq!(policy.cap_for_attempt_ms(30), 60_000);
    }

    #[test]
    fn deterministic_series_zero_seed() {
        let policy = Policy::default();
        let expected = [2_236u64, 4_203, 9_051, 15_827, 44_159, 3_907];
        for (attempt, exp) in expected.iter().enumerate() {
            let attempt_u32 =
                u32::try_from(attempt).expect("test iteration count fits in u32 for jitter proof");
            let delay = policy.delay_ms(attempt_u32, SID_ZERO);
            assert_eq!(delay, *exp, "attempt {attempt}");
        }
    }

    #[test]
    fn deterministic_series_incrementing_seed() {
        let policy = Policy::default();
        let expected = [4_133u64, 1_579, 16_071, 30_438, 7_169, 20_790];
        for (attempt, exp) in expected.iter().enumerate() {
            let attempt_u32 =
                u32::try_from(attempt).expect("test iteration count fits in u32 for jitter proof");
            let delay = policy.delay_ms(attempt_u32, SID_SEQ);
            assert_eq!(delay, *exp, "attempt {attempt}");
        }
    }

    #[test]
    fn deterministic_sampler_matches_between_calls() {
        for attempt in 0..8 {
            let left = deterministic_sample(&SID_SEQ, attempt);
            let right = deterministic_sample(&SID_SEQ, attempt);
            assert_eq!(left, right);
        }
    }

    #[test]
    fn delay_never_exceeds_cap() {
        let policy = Policy::default();
        let seeds: [&[u8]; 2] = [&SID_ZERO, &SID_SEQ];
        for attempt in 0..12 {
            let cap = policy.cap_for_attempt_ms(attempt);
            for seed in seeds {
                let delay = policy.delay_ms(attempt, seed);
                assert!(delay <= cap, "attempt {attempt} exceeded cap {cap}");
            }
        }
    }
}
