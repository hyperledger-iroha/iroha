//! Fallible field and curve sampling propagates every entropy failure.

use ff::Field;
use group::Group;
use iroha_pasta::{Ep, Eq, Fp, Fq};
use rand_core::TryRng;

/// The unchanged error returned by the test entropy source.
#[derive(Debug, PartialEq, Eq)]
struct EntropyUnavailable;

impl core::fmt::Display for EntropyUnavailable {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("entropy unavailable")
    }
}

impl std::error::Error for EntropyUnavailable {}

/// Rejects exactly the requested word draw without manufacturing entropy.
struct FailingRng {
    remaining: usize,
}

impl TryRng for FailingRng {
    type Error = EntropyUnavailable;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        self.try_next_u64().map(|word| word as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        if self.remaining == 0 {
            return Err(EntropyUnavailable);
        }
        self.remaining -= 1;
        Ok(0)
    }

    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Self::Error> {
        panic!("sampling must retain its established word-draw sequence")
    }
}

#[test]
fn field_sampling_propagates_each_word_failure() {
    for remaining in 0..8 {
        assert_eq!(
            Fp::try_random(&mut FailingRng { remaining }),
            Err(EntropyUnavailable)
        );
        assert_eq!(
            Fq::try_random(&mut FailingRng { remaining }),
            Err(EntropyUnavailable)
        );
    }
}

#[test]
fn curve_sampling_propagates_coordinate_and_sign_failures() {
    for remaining in 0..9 {
        assert_eq!(
            Ep::try_random(&mut FailingRng { remaining }),
            Err(EntropyUnavailable)
        );
        assert_eq!(
            Eq::try_random(&mut FailingRng { remaining }),
            Err(EntropyUnavailable)
        );
    }
}
