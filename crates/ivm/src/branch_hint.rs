//! Branch-weight hints for hot interpreter and memory paths.
//!
//! These mirror the unstable `core::hint::{likely, unlikely}`: the condition is
//! returned unchanged and only the unexpected arm is marked cold with the
//! stable [`core::hint::cold_path`], so code layout may change but behaviour
//! cannot.

/// Hint that `condition` is usually `true`; returns it unchanged.
#[expect(
    clippy::inline_always,
    reason = "a branch hint only shapes the caller's code once inlined into it"
)]
#[inline(always)]
pub(crate) const fn likely(condition: bool) -> bool {
    if condition {
        true
    } else {
        core::hint::cold_path();
        false
    }
}

/// Hint that `condition` is usually `false`; returns it unchanged.
#[expect(
    clippy::inline_always,
    reason = "a branch hint only shapes the caller's code once inlined into it"
)]
#[inline(always)]
pub(crate) const fn unlikely(condition: bool) -> bool {
    if condition {
        core::hint::cold_path();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{likely, unlikely};

    #[test]
    fn likely_returns_its_input() {
        assert!(likely(true));
        assert!(!likely(false));
    }

    #[test]
    fn unlikely_returns_its_input() {
        assert!(unlikely(true));
        assert!(!unlikely(false));
    }

    #[test]
    fn hints_evaluate_in_const_context() {
        const {
            assert!(likely(true));
            assert!(!unlikely(false));
        }
    }
}
