//! Iroha integration patch for externally seeded, proof-scoped randomness.
//!
//! The pinned upstream API obtains randomness through `thread_rng()` inside
//! the PCS, relaxed-mask, and IPA layers. Iroha enters this scope with a seed
//! from its fallible, health-checked cryptographic source. The proof lock
//! prevents another proof from replacing the stream while Rayon workers are
//! consuming it. Randomness requested outside an explicitly seeded scope
//! fails closed.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, MutexGuard},
};

use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};

static PROOF_SCOPE: Mutex<()> = Mutex::new(());
static ACTIVE_RNG: Mutex<Option<StdRng>> = Mutex::new(None);

/// A panic was caught at the vendored prover boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalRngScopePanicked;

/// Run one complete preparation/proving operation with an externally supplied
/// 256-bit CSPRNG seed, converting any internal panic into an explicit error.
pub fn with_external_seed<T>(
    seed: [u8; 32],
    operation: impl FnOnce() -> T,
) -> Result<T, ExternalRngScopePanicked> {
    let proof_guard = lock(&PROOF_SCOPE);
    *lock(&ACTIVE_RNG) = Some(StdRng::from_seed(seed));
    let scope = ActiveScope {
        _proof_guard: proof_guard,
    };
    let output = catch_unwind(AssertUnwindSafe(operation)).map_err(|_| ExternalRngScopePanicked);
    drop(scope);
    output
}

/// Run a test operation in a deterministic externally seeded proof scope.
#[cfg(test)]
pub(crate) fn with_deterministic_test_seed<T>(seed_byte: u8, operation: impl FnOnce() -> T) -> T {
    with_external_seed([seed_byte; 32], operation)
        .expect("deterministically seeded Vega test scope must complete")
}

/// RNG facade used by the patched PCS, mask, and IPA call sites.
pub(crate) struct ScopedRng;

impl RngCore for ScopedRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let mut active = lock(&ACTIVE_RNG);
        let Some(active) = active.as_mut() else {
            drop(active);
            panic!("Vega randomness requested outside an externally seeded proof scope");
        };
        active.fill_bytes(destination);
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for ScopedRng {}

/// Fill bytes from the active externally seeded stream.
///
/// # Panics
///
/// Panics when called outside [`with_external_seed`]. The Iroha adapter catches
/// this panic at the vendored prover boundary and converts it into a typed
/// proving error.
pub(crate) fn fill_bytes(destination: &mut [u8]) {
    ScopedRng.fill_bytes(destination);
}

struct ActiveScope<'a> {
    _proof_guard: MutexGuard<'a, ()>,
}

impl Drop for ActiveScope<'_> {
    fn drop(&mut self) {
        *lock(&ACTIVE_RNG) = None;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_external_seeds_produce_identical_streams() {
        let draw = || {
            with_external_seed([0x5a; 32], || {
                let mut bytes = [0_u8; 64];
                fill_bytes(&mut bytes);
                bytes
            })
            .expect("seeded scope must complete")
        };

        assert_eq!(draw(), draw());
    }

    #[test]
    fn randomness_outside_a_seeded_scope_fails_closed() {
        let proof_guard = lock(&PROOF_SCOPE);
        *lock(&ACTIVE_RNG) = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut byte = [0_u8; 1];
            fill_bytes(&mut byte);
        }));
        drop(proof_guard);

        assert!(result.is_err());
    }

    #[test]
    fn panicking_operation_clears_the_seeded_scope() {
        let result = with_external_seed([0xa5; 32], || panic!("adversarial prover panic"));
        assert_eq!(result, Err(ExternalRngScopePanicked));

        let proof_guard = lock(&PROOF_SCOPE);
        assert!(lock(&ACTIVE_RNG).is_none());
        drop(proof_guard);
    }
}
