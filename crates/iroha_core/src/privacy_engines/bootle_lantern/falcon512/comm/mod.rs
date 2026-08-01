//! Minimal portable common arithmetic from `fn-dsa-comm` 0.3.0.

pub(super) mod chacha;
pub(super) mod mq;
pub(super) mod shake;

/// Deterministic pseudorandom generator interface used by the vendored core.
pub(super) trait Prng {
    fn new(seed: &[u8]) -> Self;
    fn next_u8(&mut self) -> u8;
    fn next_u16(&mut self) -> u16;
    fn next_u64(&mut self) -> u64;
    fn zeroize(&mut self);
}

// Preserve the upstream spelling inside selectively vendored source.
pub(super) use Prng as PRNG;
