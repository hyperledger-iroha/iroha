//! Debug/test-only environment flag helpers.
//!
//! Production builds must not honour the `IVM_*` debug env toggles. We
//! gate the checks on `debug_assertions`/tests so release binaries ignore them
//! while keeping the knobs available for local debugging and CI.

/// Return `true` when the provided environment flag is set **and** the build is
/// running with debug assertions (dev/tests). Release builds always return
/// `false` so production nodes ignore debug shims.
#[inline]
pub(crate) fn dev_env_flag(name: &str) -> bool {
    cfg!(any(test, debug_assertions)) && std::env::var_os(name).is_some()
}

#[inline]
pub(crate) fn decode_trace_enabled() -> bool {
    dev_env_flag("IVM_DECODE_TRACE")
}

#[inline]
pub(crate) fn debug_wsv_enabled() -> bool {
    dev_env_flag("IVM_DEBUG_WSV")
}

#[inline]
pub(crate) fn debug_compact_enabled() -> bool {
    dev_env_flag("IVM_DEBUG_COMPACT")
}

#[inline]
pub(crate) fn debug_invalid_enabled() -> bool {
    dev_env_flag("IVM_DEBUG_INVALID")
}

#[inline]
pub(crate) fn debug_regalloc_enabled() -> bool {
    dev_env_flag("IVM_DEBUG_REGALLOC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_env_flag_tracks_env_in_debug_builds() {
        const FLAG: &str = "IVM_DEV_ENV_TEST_FLAG";
        unsafe { std::env::remove_var(FLAG) };
        assert!(!dev_env_flag(FLAG));

        unsafe { std::env::set_var(FLAG, "1") };
        assert!(dev_env_flag(FLAG));

        unsafe { std::env::remove_var(FLAG) };
        assert!(!dev_env_flag(FLAG));
    }

    #[test]
    fn specific_flags_delegate_to_dev_env_helper() {
        unsafe { std::env::remove_var("IVM_DECODE_TRACE") };
        assert!(!decode_trace_enabled());

        unsafe { std::env::set_var("IVM_DECODE_TRACE", "1") };
        assert!(decode_trace_enabled());

        unsafe { std::env::remove_var("IVM_DECODE_TRACE") };
        assert!(!decode_trace_enabled());
    }
}
