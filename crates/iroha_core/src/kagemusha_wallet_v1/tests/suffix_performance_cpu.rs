//! Test-only process CPU clock; no authority or timeout decisions.
// POSIX process CPU includes the verifier's worker threads. This diagnostic runs alone;
// it does not divide parallel CPU by a guessed core count or mistake wall time for CPU.
#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", target_pointer_width = "64")
))]
#[allow(unsafe_code)]
pub(super) fn cpu_ns() -> Option<u128> {
    use std::ffi::{c_int, c_long};
    #[repr(C)]
    struct Timespec {
        seconds: c_long,
        nanoseconds: c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: c_int, result: *mut Timespec) -> c_int;
    }
    #[cfg(target_os = "macos")]
    const PROCESS_CPU_CLOCK: c_int = 12;
    #[cfg(target_os = "linux")]
    const PROCESS_CPU_CLOCK: c_int = 2;
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: this target's POSIX timespec is two c_long fields; `time` is valid writable
    // storage for the complete synchronous call. The clock ID is process CPU, not a handle.
    if unsafe { clock_gettime(PROCESS_CPU_CLOCK, &mut time) } != 0
        || time.seconds < 0
        || !(0..1_000_000_000).contains(&time.nanoseconds)
    {
        return None;
    }
    Some(
        u128::try_from(time.seconds).ok()? * 1_000_000_000
            + u128::try_from(time.nanoseconds).ok()?,
    )
}
#[cfg(not(any(
    target_os = "macos",
    all(target_os = "linux", target_pointer_width = "64")
)))]
pub(super) fn cpu_ns() -> Option<u128> {
    None
}
