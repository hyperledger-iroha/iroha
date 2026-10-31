//! Native Windows CPU clocks and peak resident memory for the current process.
#![allow(
    unsafe_code,
    reason = "read-only Win32 process and thread resource queries"
)]

use super::ResourceUsage;
use windows_sys::Win32::{
    Foundation::{FILETIME, HANDLE},
    System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::{GetCurrentProcess, GetCurrentThread, GetProcessTimes, GetThreadTimes},
    },
};

type CpuTimesQuery = unsafe extern "system" fn(
    HANDLE,
    *mut FILETIME,
    *mut FILETIME,
    *mut FILETIME,
    *mut FILETIME,
) -> i32;

fn filetime_ns(value: FILETIME) -> Option<u64> {
    let ticks = (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime);
    ticks.checked_mul(100)
}

fn cpu_times(handle: HANDLE, query: CpuTimesQuery) -> Option<(u64, u64)> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: the selected Win32 query writes four initialized, distinct FILETIME outputs;
    // the caller supplies the current process/thread pseudo handle, which remains valid.
    if unsafe {
        query(
            handle,
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    } == 0
    {
        return None;
    }
    Some((filetime_ns(user)?, filetime_ns(kernel)?))
}

pub(super) fn thread_cpu_ns() -> Option<u64> {
    // SAFETY: the pseudo handle is borrowed from the operating system and must not be closed.
    let (user, kernel) = cpu_times(unsafe { GetCurrentThread() }, GetThreadTimes)?;
    user.checked_add(kernel)
}

pub(super) fn process_cpu_ns() -> Option<u64> {
    // SAFETY: the pseudo handle is borrowed from the operating system and must not be closed.
    let (user, kernel) = cpu_times(unsafe { GetCurrentProcess() }, GetProcessTimes)?;
    user.checked_add(kernel)
}

pub(super) fn resource_usage() -> Option<ResourceUsage> {
    // SAFETY: querying the current process needs no opened or inherited external handle.
    let process = unsafe { GetCurrentProcess() };
    let (user_ns, system_ns) = cpu_times(process, GetProcessTimes)?;
    let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..Default::default()
    };
    // SAFETY: the initialized aligned buffer has exactly the advertised structure size.
    if unsafe { GetProcessMemoryInfo(process, &raw mut memory, size) } == 0 {
        return None;
    }
    Some(ResourceUsage {
        user_ns,
        system_ns,
        peak_rss_bytes: u64::try_from(memory.PeakWorkingSetSize).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };

    #[test]
    fn filetime_conversion_combines_both_words_and_refuses_nanosecond_overflow() {
        assert_eq!(filetime_ns(FILETIME::default()), Some(0));
        assert_eq!(
            filetime_ns(FILETIME {
                dwLowDateTime: 7,
                dwHighDateTime: 1
            }),
            Some(((1_u64 << 32) + 7) * 100)
        );
        assert_eq!(
            filetime_ns(FILETIME {
                dwLowDateTime: u32::MAX,
                dwHighDateTime: u32::MAX
            }),
            None
        );
    }

    #[test]
    fn invalid_kernel_handles_remain_unavailable() {
        assert!(cpu_times(std::ptr::null_mut(), GetProcessTimes).is_none());
        assert!(cpu_times(std::ptr::null_mut(), GetThreadTimes).is_none());
    }

    #[test]
    fn native_cpu_clocks_advance_with_real_work() {
        let thread_before = thread_cpu_ns().expect("native thread CPU clock");
        let process_before = process_cpu_ns().expect("native process CPU clock");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut word = 1_u64;
        loop {
            for value in 0..100_000_u64 {
                word = black_box(word.wrapping_mul(31).wrapping_add(value));
            }
            let thread_after = thread_cpu_ns().unwrap();
            let process_after = process_cpu_ns().unwrap();
            assert!(thread_after >= thread_before);
            assert!(process_after >= process_before);
            if thread_after > thread_before && process_after > process_before {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "native CPU clocks did not observe the work"
            );
        }
    }

    #[test]
    fn native_peak_resident_bytes_cover_touched_pages_and_never_decrease() {
        let before = resource_usage().expect("native process resource usage");
        assert!(before.peak_rss_bytes > 0);
        let mut pages = vec![0_u8; 16 * 1024 * 1024];
        for page in pages.chunks_mut(4096) {
            page[0] = black_box(1);
        }
        black_box(&pages);
        let after = resource_usage().unwrap();
        assert!(after.peak_rss_bytes >= before.peak_rss_bytes);
        assert!(after.peak_rss_bytes >= pages.len() as u64);
        assert!(after.user_ns >= before.user_ns);
        assert!(after.system_ns >= before.system_ns);
    }

    #[test]
    fn unavailable_unix_limits_load_and_thermal_state_are_not_synthesized() {
        assert_eq!(super::super::ADDRESS_SPACE_SOURCE, "unavailable");
        assert!(super::super::address_space_limit().is_none());
        assert!(super::super::try_load_average_milli().is_none());
        assert_eq!(
            super::super::thermal_state(),
            (crate::schema::ThermalState::Unavailable, "unavailable")
        );
    }
}
