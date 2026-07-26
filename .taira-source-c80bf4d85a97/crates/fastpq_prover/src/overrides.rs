//! Runtime overrides for GPU-related FASTPQ knobs.
//!
//! These helpers let hosts configure Metal tuning parameters via configuration
//! before the GPU backend initialises, avoiding ad-hoc runtime environment
//! shims in production. Environment fallbacks are still available for
//! development tools and benches when no override is supplied.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
#[cfg(test)]
use std::{cell::RefCell, collections::HashMap};

#[cfg(test)]
thread_local! {
    static TEST_ENV_OVERRIDES: RefCell<HashMap<String, String>> =
        RefCell::new(HashMap::new());
}

#[cfg(any(test, debug_assertions))]
#[allow(dead_code)]
fn parse_bool_env(name: &str) -> Option<bool> {
    debug_env_string(name).map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(not(any(test, debug_assertions)))]
#[allow(dead_code)]
fn parse_bool_env(_name: &str) -> Option<bool> {
    None
}

#[cfg(any(test, debug_assertions))]
pub fn debug_env_string(name: &str) -> Option<String> {
    #[cfg(test)]
    if let Some(value) = test_env_override(name) {
        return Some(value);
    }

    std::env::var(name).ok()
}

#[cfg(not(any(test, debug_assertions)))]
pub fn debug_env_string(_name: &str) -> Option<String> {
    None
}

#[cfg(any(test, debug_assertions))]
#[allow(dead_code)]
pub fn debug_env_bool(name: &str) -> Option<bool> {
    parse_bool_env(name)
}

#[cfg(not(any(test, debug_assertions)))]
#[allow(dead_code)]
pub fn debug_env_bool(_name: &str) -> Option<bool> {
    None
}

/// Optional overrides for the Metal backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MetalOverrides {
    /// Optional cap on concurrent Metal command buffers (None = derive automatically).
    pub max_in_flight: Option<usize>,
    /// Optional override for threadgroup width (None = derive from pipeline hints).
    pub threadgroup_size: Option<u64>,
    /// Whether to emit per-dispatch trace logs.
    pub dispatch_trace: bool,
    /// Whether to print Metal device enumeration details during GPU discovery.
    pub debug_enum: bool,
    /// Whether to dump fused Poseidon pipeline failures to stderr.
    pub debug_fused: bool,
}

static METAL_MAX_IN_FLIGHT_OVERRIDE: OnceLock<Option<usize>> = OnceLock::new();
static METAL_THREADGROUP_OVERRIDE: OnceLock<Option<u64>> = OnceLock::new();
static METAL_DISPATCH_TRACE_OVERRIDE: OnceLock<Option<bool>> = OnceLock::new();
static METAL_DEBUG_ENUM_OVERRIDE: OnceLock<Option<bool>> = OnceLock::new();
static METAL_DEBUG_FUSED_OVERRIDE: OnceLock<Option<bool>> = OnceLock::new();
static ENV_OVERRIDES_LOCKED: AtomicBool = AtomicBool::new(false);

/// Apply Metal overrides provided by the host configuration.
///
/// # Errors
/// Returns an error if conflicting overrides were previously configured.
pub fn apply_metal_overrides(overrides: MetalOverrides) -> Result<(), &'static str> {
    set_option_override(
        &METAL_MAX_IN_FLIGHT_OVERRIDE,
        overrides.max_in_flight,
        "FASTPQ Metal max-in-flight override already configured",
    )?;
    set_option_override(
        &METAL_THREADGROUP_OVERRIDE,
        overrides.threadgroup_size,
        "FASTPQ Metal threadgroup override already configured",
    )?;
    set_bool_override(
        &METAL_DISPATCH_TRACE_OVERRIDE,
        overrides.dispatch_trace,
        "FASTPQ Metal dispatch-trace override already configured",
    )?;
    set_bool_override(
        &METAL_DEBUG_ENUM_OVERRIDE,
        overrides.debug_enum,
        "FASTPQ Metal enumeration-debug override already configured",
    )?;
    set_bool_override(
        &METAL_DEBUG_FUSED_OVERRIDE,
        overrides.debug_fused,
        "FASTPQ Poseidon fused-debug override already configured",
    )?;
    freeze_env_overrides();
    Ok(())
}

/// Resolve the configured Metal command-buffer cap override, if any.
#[allow(dead_code)]
pub fn metal_max_in_flight_override() -> Option<usize> {
    METAL_MAX_IN_FLIGHT_OVERRIDE.get().copied().flatten()
}

/// Resolve the configured Metal threadgroup width override, if any.
#[allow(dead_code)]
pub fn metal_threadgroup_override() -> Option<u64> {
    METAL_THREADGROUP_OVERRIDE.get().copied().flatten()
}

/// Resolve the configured Metal dispatch-trace override, if any.
#[allow(dead_code)]
pub fn metal_dispatch_trace_override() -> Option<bool> {
    METAL_DISPATCH_TRACE_OVERRIDE.get().copied().flatten()
}

/// Resolve the configured Metal enumeration-debug override, if any.
#[allow(dead_code)]
pub fn metal_debug_enum_override() -> Option<bool> {
    METAL_DEBUG_ENUM_OVERRIDE.get().copied().flatten()
}

/// Resolve the configured Poseidon fused-debug override, if any.
#[allow(dead_code)]
pub fn metal_debug_fused_override() -> Option<bool> {
    METAL_DEBUG_FUSED_OVERRIDE.get().copied().flatten()
}

/// Whether configuration has been applied, freezing runtime env fallbacks.
#[allow(dead_code)]
pub fn env_overrides_locked() -> bool {
    ENV_OVERRIDES_LOCKED.load(Ordering::Acquire)
}

/// Guarded environment fallback: returns `None` once configuration has been applied.
#[allow(dead_code)]
pub fn guard_env_override<T>(loader: impl FnOnce() -> Option<T>) -> Option<T> {
    if env_overrides_locked() {
        return None;
    }
    loader()
}

fn freeze_env_overrides() {
    ENV_OVERRIDES_LOCKED.store(true, Ordering::Release);
}

#[cfg(test)]
fn set_test_env_override(name: &str, value: Option<&str>) {
    TEST_ENV_OVERRIDES.with(|vars| {
        let mut vars = vars.borrow_mut();
        match value {
            Some(value) => {
                vars.insert(name.to_owned(), value.to_owned());
            }
            None => {
                vars.remove(name);
            }
        }
    });
}

#[cfg(test)]
fn test_env_override(name: &str) -> Option<String> {
    TEST_ENV_OVERRIDES.with(|vars| vars.borrow().get(name).cloned())
}

fn set_option_override<T: Copy + PartialEq>(
    cell: &OnceLock<Option<T>>,
    value: Option<T>,
    error: &'static str,
) -> Result<(), &'static str> {
    match cell.set(value) {
        Ok(()) => Ok(()),
        Err(_) if cell.get() == Some(&value) => Ok(()),
        Err(_) => Err(error),
    }
}

fn set_bool_override(
    cell: &OnceLock<Option<bool>>,
    value: bool,
    error: &'static str,
) -> Result<(), &'static str> {
    match cell.set(Some(value)) {
        Ok(()) => Ok(()),
        Err(_) if cell.get() == Some(&Some(value)) => Ok(()),
        Err(_) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_and_resolves_overrides() {
        let guard_hits = std::cell::Cell::new(0usize);
        // Before overrides, env fallback paths remain available.
        assert!(!env_overrides_locked());
        assert_eq!(
            guard_env_override(|| {
                guard_hits.set(guard_hits.get() + 1);
                Some(7usize)
            }),
            Some(7)
        );
        assert_eq!(metal_max_in_flight_override(), None);
        assert_eq!(metal_threadgroup_override(), None);
        assert_eq!(metal_dispatch_trace_override(), None);
        assert_eq!(metal_debug_enum_override(), None);
        assert_eq!(metal_debug_fused_override(), None);

        apply_metal_overrides(MetalOverrides {
            max_in_flight: Some(4),
            threadgroup_size: Some(64),
            dispatch_trace: true,
            debug_enum: true,
            debug_fused: true,
        })
        .expect("overrides apply");

        assert_eq!(metal_max_in_flight_override(), Some(4));
        assert_eq!(metal_threadgroup_override(), Some(64));
        assert_eq!(metal_dispatch_trace_override(), Some(true));
        assert_eq!(metal_debug_enum_override(), Some(true));
        assert_eq!(metal_debug_fused_override(), Some(true));
        assert!(env_overrides_locked());
        assert_eq!(guard_env_override(|| Some(9usize)), None);
        assert_eq!(guard_hits.get(), 1);

        // Re-applying identical values is a no-op.
        apply_metal_overrides(MetalOverrides {
            max_in_flight: Some(4),
            threadgroup_size: Some(64),
            dispatch_trace: true,
            debug_enum: true,
            debug_fused: true,
        })
        .expect("idempotent overrides");
    }

    #[test]
    fn debug_env_helpers_respect_debug_builds() {
        const NAME: &str = "FASTPQ_OVERRIDE_TEST_FLAG";
        set_test_env_override(NAME, None);
        assert_eq!(debug_env_bool(NAME), None);
        set_test_env_override(NAME, Some("true"));
        assert_eq!(debug_env_bool(NAME), Some(true));
        set_test_env_override(NAME, Some("false"));
        assert_eq!(debug_env_bool(NAME), Some(false));
        set_test_env_override(NAME, None);

        set_test_env_override(NAME, None);
        assert_eq!(debug_env_string(NAME), None);
        set_test_env_override(NAME, Some("value"));
        assert_eq!(debug_env_string(NAME).as_deref(), Some("value"));
        set_test_env_override(NAME, None);
    }
}
