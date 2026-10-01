//! Suspend-inclusive native app-attempt expiry. Never UTC, a financial lease or bootstrap time.
//! Uses the same actual platform clocks as the coordinator's existing native deadline; no uptime
//! or handset wall-clock fallback is permitted. A reading cannot be decoded or supplied by JNI.
use super::{Custody, Result};
#[derive(Clone, Copy)]
pub(super) struct Reading {
    process: u32,
    nanos: u128,
}
impl Reading {
    pub(super) fn now() -> Result<Self> {
        let process = std::process::id();
        let nanos = platform_nanos()?;
        if process == 0 || process != std::process::id() {
            return Err(Custody);
        }
        Ok(Self { process, nanos })
    }
    pub(super) fn elapsed_ms(self, earlier: Self) -> Result<u64> {
        if self.process != earlier.process || self.process != std::process::id() {
            return Err(Custody);
        }
        u64::try_from(self.nanos.checked_sub(earlier.nanos).ok_or(Custody)? / 1_000_000)
            .map_err(|_| Custody)
    }
}
#[cfg(target_vendor = "apple")]
fn platform_nanos() -> Result<u128> {
    #[repr(C)]
    struct MachTimebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebase) -> i32;
    }
    static TIMEBASE: std::sync::OnceLock<Result<(u32, u32)>> = std::sync::OnceLock::new();
    let (numer, denom) = *TIMEBASE
        .get_or_init(|| {
            let mut info = MachTimebase { numer: 0, denom: 0 };
            // SAFETY: actual public SDK struct is initialized and writable for the whole native call.
            if unsafe { mach_timebase_info(&mut info) } != 0 || info.numer == 0 || info.denom == 0 {
                Err(Custody)
            } else {
                Ok((info.numer, info.denom))
            }
        })
        .as_ref()
        .map_err(|e| *e)?;
    // SAFETY: native no-pointer continuous boot clock, including handset sleep.
    u128::from(unsafe { mach_continuous_time() })
        .checked_mul(u128::from(numer))
        .and_then(|v| v.checked_div(u128::from(denom)))
        .ok_or(Custody)
}
#[cfg(any(target_os = "android", target_os = "linux"))]
fn platform_nanos() -> Result<u128> {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Boottime);
    if time.tv_sec < 0 || !(0..1_000_000_000).contains(&time.tv_nsec) {
        return Err(Custody);
    }
    (time.tv_sec as u128)
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(time.tv_nsec as u128))
        .ok_or(Custody)
}
#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
fn platform_nanos() -> Result<u128> {
    Err(Custody)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elapsed_readings_reject_backwards_and_foreign_process() {
        let process = std::process::id();
        let original = Reading {
            process,
            nanos: 1_000_000,
        };
        assert_eq!(
            Reading {
                process,
                nanos: 2_999_999
            }
            .elapsed_ms(original)
            .unwrap(),
            1
        );
        assert!(
            Reading {
                process,
                nanos: 999_999
            }
            .elapsed_ms(original)
            .is_err()
        );
        assert!(
            Reading {
                process: process.checked_add(1).unwrap(),
                nanos: 3_000_000
            }
            .elapsed_ms(original)
            .is_err()
        );
    }
}
