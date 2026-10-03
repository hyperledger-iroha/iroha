//! Explicit caller policy and receipt custody across parallel SHA leaf work.

use super::super::SimdChoice;

/// The routine that actually completed one CPU compression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Sha256Backend {
    Scalar,
    Native,
}

/// Captured caller restrictions travel with Rayon work; worker-local overrides
/// from an unrelated invocation cannot relax or replace this operation's policy.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sha256Context {
    caller_override: Option<SimdChoice>,
    scalar_only: bool,
    synthetic: bool,
}

impl Sha256Context {
    pub(crate) fn production() -> Self {
        let caller_override = super::super::TLS_SIMD_OVERRIDE
            .with(|value| super::super::decode_override(value.get()));
        Self {
            caller_override,
            scalar_only: !super::super::simd_policy_enabled()
                || matches!(super::super::forced_simd_choice(), Some(SimdChoice::Scalar)),
            synthetic: false,
        }
    }

    /// Receipt routing is an explicit part of the same operation, not ambient
    /// TLS that disappears when Rayon schedules a closure on another worker.
    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    pub(crate) fn synthetic(self) -> Self {
        Self {
            synthetic: true,
            ..self
        }
    }

    pub(super) fn is_synthetic(self) -> bool {
        self.synthetic
    }

    pub(super) fn allowed(self) -> bool {
        if self.scalar_only || !super::super::simd_policy_enabled() {
            return false;
        }
        let choice = self.caller_override.or_else(|| {
            super::super::decode_override(
                super::super::SIMD_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed),
            )
        });
        !matches!(
            choice.map(super::super::clamp_to_supported),
            Some(SimdChoice::Scalar)
        )
    }

    /// Both production and calibration traverse this exact native/scalar path.
    pub(crate) fn compress(self, state: &mut [u32; 8], block: &[u8; 64]) -> Sha256Backend {
        super::compress(state, block, self)
    }
}

/// Allocation-free evidence reduced through the ordinary CPU leaf traversal.
/// Zero leaves use the canonical zero-hash shortcut and report no compression.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Sha256Observed(u8);

impl Sha256Observed {
    pub(crate) fn completed(backend: Sha256Backend) -> Self {
        Self(match backend {
            Sha256Backend::Scalar => 1,
            Sha256Backend::Native => 2,
        })
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    pub(crate) fn matches(self, backend: Sha256Backend, require_work: bool) -> bool {
        let expected = Self::completed(backend).0;
        self.0 & !expected == 0 && (!require_work || self.0 == expected)
    }
}

/// Persistent cost profiles use only the immutable process-global Rayon pool.
/// An invocation already inside a custom or nested pool keeps its real CPU
/// fallback until that pool has an explicit profile owner of its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
pub(crate) struct Sha256Baseline {
    backend: Sha256Backend,
    workers: usize,
}

#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
impl Sha256Baseline {
    pub(crate) fn capture(context: Sha256Context) -> Option<Self> {
        if rayon::current_thread_index().is_some() {
            return None;
        }
        Some(Self {
            backend: super::qualify_backend(context)?,
            workers: rayon::current_num_threads(),
        })
    }

    pub(crate) fn is_current(self, context: Sha256Context) -> bool {
        rayon::current_thread_index().is_none()
            && rayon::current_num_threads() == self.workers
            && super::current_backend(context) == Some(self.backend)
    }

    pub(crate) fn accepts(self, observed: Sha256Observed, require_work: bool) -> bool {
        observed.matches(self.backend, require_work)
    }
}

#[cfg(test)]
mod tests;
