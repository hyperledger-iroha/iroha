//! Process-owned native SHA-256 qualification and original-state publication.

use crate::sha256_ref::sha256_compress_scalar_ref;
use std::sync::{
    Mutex,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

pub(super) mod context;
use context::{Sha256Backend, Sha256Context};

#[cfg(target_arch = "aarch64")]
mod arm;
#[cfg(target_arch = "x86_64")]
mod x86;

const UNTESTED: u8 = 0;
const ADMITTED: u8 = 1;
const QUARANTINED: u8 = 2;
const INITIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

struct NativeSha256 {
    phase: AtomicU8,
    qualification: Mutex<()>,
    completions: AtomicU64,
    synthetic_completions: AtomicU64,
}

struct Attempt<'a> {
    owner: &'a NativeSha256,
    completed: bool,
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.owner.phase.store(QUARANTINED, Ordering::Release);
        }
    }
}

impl NativeSha256 {
    const fn new() -> Self {
        Self {
            phase: AtomicU8::new(UNTESTED),
            qualification: Mutex::new(()),
            completions: AtomicU64::new(0),
            synthetic_completions: AtomicU64::new(0),
        }
    }

    fn qualify(&self, native: &impl Fn(&mut [u32; 8], &[u8; 64])) -> bool {
        match self.phase.load(Ordering::Acquire) {
            ADMITTED => return true,
            QUARANTINED => return false,
            _ => {}
        }
        let _gate = match self.qualification.try_lock() {
            Ok(gate) => gate,
            Err(std::sync::TryLockError::WouldBlock) => return false,
            Err(std::sync::TryLockError::Poisoned(_)) => {
                self.phase.store(QUARANTINED, Ordering::Release);
                return false;
            }
        };
        if self.phase.load(Ordering::Acquire) != UNTESTED {
            return self.phase.load(Ordering::Acquire) == ADMITTED;
        }
        let mut attempt = Attempt {
            owner: self,
            completed: false,
        };
        // Public fixed inputs cover the known digest, schedule expansion, arbitrary
        // chaining values and dependent blocks. Guest contents never drive admission.
        let mut abc = [0; 64];
        abc[..3].copy_from_slice(b"abc");
        abc[3] = 0x80;
        abc[63] = 24;
        let pattern = std::array::from_fn(|index| (index as u8).wrapping_mul(37).wrapping_add(13));
        let mut expected = INITIAL;
        let mut actual = INITIAL;
        for block in [abc, pattern, [0xff; 64]] {
            sha256_compress_scalar_ref(&mut expected, &block);
            native(&mut actual, &block);
            if actual != expected {
                return false;
            }
        }
        expected = std::array::from_fn(|index| (index as u32).wrapping_mul(0x9e37_79b9));
        actual = expected;
        sha256_compress_scalar_ref(&mut expected, &pattern);
        native(&mut actual, &pattern);
        if actual != expected {
            return false;
        }
        self.phase
            .compare_exchange(UNTESTED, ADMITTED, Ordering::AcqRel, Ordering::Acquire)
            .ok();
        attempt.completed = true;
        self.phase.load(Ordering::Acquire) == ADMITTED
    }

    fn try_compress(
        &self,
        state: &mut [u32; 8],
        block: &[u8; 64],
        allowed: impl Fn() -> bool,
        supported: bool,
        synthetic: bool,
        native: impl Fn(&mut [u32; 8], &[u8; 64]),
    ) -> bool {
        if !allowed() || !supported || !self.qualify(&native) {
            return false;
        }
        let mut attempt = Attempt {
            owner: self,
            completed: false,
        };
        let mut staged = *state;
        native(&mut staged, block);
        if self.phase.load(Ordering::Acquire) != ADMITTED {
            return false;
        }
        // Operator policy can change while the native work is completing.
        // This is a local refusal, not a backend mismatch or quarantine.
        if !allowed() {
            attempt.completed = true;
            return false;
        }
        *state = staged;
        // Both modes pay the exact same saturating atomic accounting operation.
        let bank = if synthetic {
            &self.synthetic_completions
        } else {
            &self.completions
        };
        let _ = bank.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            Some(n.saturating_add(1))
        });
        attempt.completed = true;
        true
    }
}

static OWNER: NativeSha256 = NativeSha256::new();

fn supported() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        std::arch::is_aarch64_feature_detected!("sha2")
    }
    #[cfg(target_arch = "x86_64")]
    {
        std::is_x86_feature_detected!("sha") && std::is_x86_feature_detected!("ssse3")
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        false
    }
}

fn native_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    // SAFETY: every caller checks the required CPU features before either
    // qualification or production reaches this routine.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        arm::compress(state, block);
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        x86::compress(state, block);
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let _ = (state, block);
        unreachable!("unsupported native SHA routine cannot be called");
    }
}

#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
fn current_backend(context: Sha256Context) -> Option<Sha256Backend> {
    if !context.allowed() || !supported() {
        return Some(Sha256Backend::Scalar);
    }
    match OWNER.phase.load(Ordering::Acquire) {
        ADMITTED => Some(Sha256Backend::Native),
        QUARANTINED => Some(Sha256Backend::Scalar),
        _ => None,
    }
}

#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
fn qualify_backend(context: Sha256Context) -> Option<Sha256Backend> {
    if context.allowed() && supported() {
        OWNER.qualify(&native_compress);
    }
    current_backend(context)
}

/// Complete the native attempt or the original-input scalar fallback under the
/// same explicit caller context. No synthetic work earns production receipts.
fn compress(state: &mut [u32; 8], block: &[u8; 64], context: Sha256Context) -> Sha256Backend {
    if OWNER.try_compress(
        state,
        block,
        || context.allowed(),
        supported(),
        context.is_synthetic(),
        native_compress,
    ) {
        Sha256Backend::Native
    } else {
        sha256_compress_scalar_ref(state, block);
        Sha256Backend::Scalar
    }
}

#[cfg(test)]
mod tests;
