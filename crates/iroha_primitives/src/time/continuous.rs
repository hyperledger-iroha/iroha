//! Native suspend-inclusive elapsed time, without a Unix-clock or uptime fallback.

use std::{io, time::Duration};

/// One process-bound reading of the operating system's suspend-inclusive elapsed clock.
///
/// This carries elapsed time only. It cannot authenticate a Unix timestamp, node, financial
/// lease or installation. It has no decoder or caller-supplied reading constructor.
#[derive(Clone, Copy, Debug)]
pub struct NativeContinuousReading {
    process: u32,
    nanos: u128,
}

impl NativeContinuousReading {
    /// Read the actual platform clock. Apple uses Mach continuous time; Linux/Android use
    /// `CLOCK_BOOTTIME`, so a suspended handset still exhausts a retained response budget.
    /// # Errors
    /// Refuses unavailable/unsupported clocks, invalid readings or a changed process identity.
    pub fn now() -> io::Result<Self> {
        let process = std::process::id();
        let nanos = platform_nanos()?;
        if process == 0 || process != std::process::id() {
            return Err(invalid());
        }
        Ok(Self { process, nanos })
    }

    /// Compute elapsed time from the same original process and clock.
    /// # Errors
    /// Refuses a foreign process, clock regression or duration overflow.
    pub fn elapsed_since(&self, earlier: &Self) -> io::Result<Duration> {
        if self.process != earlier.process || self.process != std::process::id() {
            return Err(invalid());
        }
        let nanos = self.nanos.checked_sub(earlier.nanos).ok_or_else(invalid)?;
        Ok(Duration::new(
            u64::try_from(nanos / 1_000_000_000).map_err(|_| invalid())?,
            (nanos % 1_000_000_000) as u32,
        ))
    }

    /// Sample actual elapsed time since this retained original reading.
    /// # Errors
    /// Refuses the same unavailable, foreign-process or regressing clock as `elapsed_since`.
    pub fn elapsed(&self) -> io::Result<Duration> {
        Self::now()?.elapsed_since(self)
    }
}

fn invalid() -> io::Error {
    io::Error::other("native suspend-inclusive clock is unavailable or changed")
}

#[cfg(target_vendor = "apple")]
#[allow(
    unsafe_code,
    reason = "Apple exposes its suspend-inclusive clock through native Mach APIs"
)]
fn platform_nanos() -> io::Result<u128> {
    #[repr(C)]
    struct MachTimebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebase) -> i32;
    }
    static TIMEBASE: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    let (numer, denom) = TIMEBASE
        .get_or_init(|| {
            let mut info = MachTimebase { numer: 0, denom: 0 };
            // SAFETY: the initialized native SDK struct remains writable for this call.
            if unsafe { mach_timebase_info(&raw mut info) } != 0
                || info.numer == 0
                || info.denom == 0
            {
                None
            } else {
                Some((info.numer, info.denom))
            }
        })
        .ok_or_else(invalid)?;
    // SAFETY: the public native clock takes no pointers and includes machine suspension.
    u128::from(unsafe { mach_continuous_time() })
        .checked_mul(u128::from(numer))
        .and_then(|value| value.checked_div(u128::from(denom)))
        .ok_or_else(invalid)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[allow(
    unsafe_code,
    reason = "the actual suspend-inclusive Linux clock is supplied by libc"
)]
fn platform_nanos() -> io::Result<u128> {
    let mut reading = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: this initialized libc timespec is writable for the synchronous native call.
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &raw mut reading) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if reading.tv_sec < 0 || !(0..1_000_000_000).contains(&reading.tv_nsec) {
        return Err(invalid());
    }
    (reading.tv_sec as u128)
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(reading.tv_nsec as u128))
        .ok_or_else(invalid)
}

#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
fn platform_nanos() -> io::Result<u128> {
    Err(invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_vendor = "apple", target_os = "android", target_os = "linux"))]
    #[test]
    fn actual_native_elapsed_readings_retain_same_process_and_order() {
        let first = NativeContinuousReading::now().unwrap();
        let second = NativeContinuousReading::now().unwrap();
        assert_eq!(second.process, first.process);
        assert!(second.elapsed_since(&first).is_ok());
        assert!(first.elapsed().is_ok());
    }

    #[test]
    fn native_elapsed_includes_retained_suspend_interval_and_submillisecond_precision() {
        let first = NativeContinuousReading {
            process: std::process::id(),
            nanos: 1_000_000,
        };
        let second = NativeContinuousReading {
            process: first.process,
            nanos: 120_001_000_731,
        };
        assert_eq!(
            second.elapsed_since(&first).unwrap(),
            Duration::new(120, 731)
        );
    }

    #[test]
    fn native_elapsed_rejects_foreign_process_and_regression() {
        let first = NativeContinuousReading {
            process: std::process::id(),
            nanos: 20,
        };
        let mut second = first;
        second.nanos = 19;
        assert!(second.elapsed_since(&first).is_err());
        second.nanos = 21;
        second.process = first.process.checked_add(1).unwrap();
        assert!(second.elapsed_since(&first).is_err());
    }
}
