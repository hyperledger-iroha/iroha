//! Suspend-inclusive native app-attempt expiry. Never UTC, a financial lease or bootstrap time.
//! Uses the same actual platform clocks as the coordinator's existing native deadline; no uptime
//! or handset wall-clock fallback is permitted. A reading cannot be decoded or supplied by JNI.
use super::{Custody, Result};
#[derive(Clone, Copy)]
pub(in crate::kagemusha_v1_state::ordinary_app_identity) struct Reading {
    process: u32,
    nanos: u128,
}
impl Reading {
    pub(in crate::kagemusha_v1_state::ordinary_app_identity) fn now() -> Result<Self> {
        let process = std::process::id();
        let nanos = platform_nanos()?;
        if process == 0 || process != std::process::id() {
            return Err(Custody);
        }
        Ok(Self { process, nanos })
    }
    pub(in crate::kagemusha_v1_state::ordinary_app_identity) fn elapsed_ms(
        self,
        earlier: Self,
    ) -> Result<u64> {
        if self.process != earlier.process || self.process != std::process::id() {
            return Err(Custody);
        }
        u64::try_from(self.nanos.checked_sub(earlier.nanos).ok_or(Custody)? / 1_000_000)
            .map_err(|_| Custody)
    }
}
fn platform_nanos() -> Result<u128> {
    iroha_primitives::time::native_continuous_clock_nanos().map_err(|_| Custody)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(
        target_vendor = "apple",
        target_os = "android",
        target_os = "linux",
        windows
    ))]
    #[test]
    fn native_readings_retain_the_original_process_and_monotonic_order() {
        let first = Reading::now().expect("supported native continuous clock");
        let second = Reading::now().expect("same native continuous clock");
        assert_eq!(first.process, std::process::id());
        assert_eq!(second.process, first.process);
        assert!(second.nanos >= first.nanos);
        assert_eq!(
            second.elapsed_ms(first).unwrap(),
            u64::try_from((second.nanos - first.nanos) / 1_000_000).unwrap()
        );
    }
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
