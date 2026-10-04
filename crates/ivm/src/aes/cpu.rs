//! Process-owned CPU AES admission, policy and original-input publication.

use std::{
    cell::Cell,
    sync::{
        Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

mod native;

const UNTESTED: u8 = 0;
const ADMITTED: u8 = 1;
const QUARANTINED: u8 = 2;
const PROBE_CASES: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Encrypt,
    Decrypt,
}

impl Direction {
    fn index(self) -> usize {
        match self {
            Self::Encrypt => 0,
            Self::Decrypt => 1,
        }
    }

    pub(crate) fn scalar(self, state: [u8; 16], key: [u8; 16]) -> [u8; 16] {
        match self {
            Self::Encrypt => super::aesenc_impl(state, key),
            Self::Decrypt => super::aesdec_impl(state, key),
        }
    }
}

/// The current process has exactly one compiled native CPU AES implementation;
/// its encryption/decryption owners cannot be replaced or reset by policy reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
pub(crate) enum Backend {
    Scalar,
    Native,
}

struct NativeRound {
    phase: AtomicU8,
    qualification: Mutex<()>,
    completions: AtomicU64,
    synthetic_completions: AtomicU64,
}

struct Attempt<'a> {
    owner: &'a NativeRound,
    completed: bool,
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.owner.phase.store(QUARANTINED, Ordering::Release);
        }
    }
}

impl NativeRound {
    const fn new() -> Self {
        Self {
            phase: AtomicU8::new(UNTESTED),
            qualification: Mutex::new(()),
            completions: AtomicU64::new(0),
            synthetic_completions: AtomicU64::new(0),
        }
    }

    fn qualify(
        &self,
        direction: Direction,
        native: &impl Fn([u8; 16], [u8; 16]) -> [u8; 16],
    ) -> bool {
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
        // Fixed public states/keys cover zero, all bits, independent key bytes,
        // dependent rounds and the different inverse-key semantics of AESDEC.
        let mut state = [0; 16];
        for seed in 0..PROBE_CASES {
            let key =
                std::array::from_fn(|lane| (seed.wrapping_mul(37)).wrapping_add(lane as u8 * 13));
            let input = match seed {
                0 => [0; 16],
                1 => [0xff; 16],
                _ => state,
            };
            state = direction.scalar(input, key);
            if native(input, key) != state {
                return false;
            }
        }
        let _ =
            self.phase
                .compare_exchange(UNTESTED, ADMITTED, Ordering::AcqRel, Ordering::Acquire);
        attempt.completed = true;
        self.phase.load(Ordering::Acquire) == ADMITTED
    }

    fn try_round(
        &self,
        direction: Direction,
        input: ([u8; 16], [u8; 16]),
        allowed: impl Fn() -> bool,
        supported: bool,
        production: bool,
        native: impl Fn([u8; 16], [u8; 16]) -> [u8; 16],
    ) -> Option<[u8; 16]> {
        if !allowed() || !supported || !self.qualify(direction, &native) || !allowed() {
            return None;
        }
        let mut attempt = Attempt {
            owner: self,
            completed: false,
        };
        let staged = native(input.0, input.1);
        if self.phase.load(Ordering::Acquire) != ADMITTED {
            return None;
        }
        // A local opt-out does not revoke a successfully qualified implementation.
        // The result is discarded and the original value remains available to CPU fallback.
        attempt.completed = true;
        if !allowed() {
            return None;
        }
        // Calibration pays the same accounting operation without earning a
        // production receipt. Admission probes never reach either counter.
        let counter = if production {
            &self.completions
        } else {
            &self.synthetic_completions
        };
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            Some(n.saturating_add(1))
        });
        Some(staged)
    }
}

static OWNERS: [NativeRound; 2] = [const { NativeRound::new() }; 2];
thread_local! { static CALIBRATION: Cell<bool> = const { Cell::new(false) }; }

fn allowed() -> bool {
    crate::vector::simd_policy_enabled()
        && !matches!(
            crate::vector::forced_simd_choice(),
            Some(crate::vector::SimdChoice::Scalar)
        )
}

fn supported() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        std::arch::is_aarch64_feature_detected!("aes")
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("aes") && std::is_x86_feature_detected!("sse2")
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

fn native_round(direction: Direction, state: [u8; 16], key: [u8; 16]) -> [u8; 16] {
    // SAFETY: every native entry follows supported() through qualification or a
    // production attempt. Policy and runtime capabilities precede both routes.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        match direction {
            Direction::Encrypt => native::aesenc_armv8(state, key),
            Direction::Decrypt => native::aesdec_armv8(state, key),
        }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        match direction {
            Direction::Encrypt => native::aesenc_aesni(state, key),
            Direction::Decrypt => native::aesdec_aesni(state, key),
        }
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
    {
        let _ = (direction, state, key);
        unreachable!("unsupported CPU AES cannot enter native code")
    }
}

/// Observe the actual currently admitted implementation, including forced scalar.
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
pub(crate) fn backend(direction: Direction) -> Backend {
    if allowed()
        && supported()
        && OWNERS[direction.index()]
            .qualify(direction, &|state, key| native_round(direction, state, key))
        && allowed()
    {
        Backend::Native
    } else {
        Backend::Scalar
    }
}

pub(super) fn round(direction: Direction, state: [u8; 16], key: [u8; 16]) -> [u8; 16] {
    let output = OWNERS[direction.index()].try_round(
        direction,
        (state, key),
        allowed,
        supported(),
        !CALIBRATION.with(Cell::get),
        |state, key| native_round(direction, state, key),
    );
    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    measurement::observe(direction, output.is_some());
    output.unwrap_or_else(|| direction.scalar(state, key))
}

/// Public synthetic CPU measurements must not masquerade as production rounds.
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
pub(crate) fn with_calibration<T>(call: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CALIBRATION.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(CALIBRATION.with(|slot| slot.replace(true)));
    call()
}

#[cfg(test)]
mod tests;

#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
mod measurement;
#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) use measurement::measure_backend;
