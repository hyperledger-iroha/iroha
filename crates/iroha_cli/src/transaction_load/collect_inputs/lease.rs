//! Original private client custody and one launcher-owned absolute monotonic deadline.

use super::super::output::canonical_inputs::{OriginalInputBinding, RetainedOriginalInput};
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const MAX_TRIAL_NS: u64 = 7200 * 1_000_000_000;
pub(super) const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// One immutable conversion of the launcher's original host monotonic deadline.
pub(super) struct Deadline {
    end_ns: u64,
    instant: Instant,
}
impl Deadline {
    pub(super) fn admit(end_ns: u64) -> Result<Self> {
        // Sample Instant first so conversion can only shorten the SDK deadline.
        let instant = Instant::now();
        Self::at(end_ns, monotonic_ns()?, instant)
    }
    fn at(end_ns: u64, now_ns: u64, instant: Instant) -> Result<Self> {
        let remaining = end_ns
            .checked_sub(now_ns)
            .filter(|value| *value > 0 && *value <= MAX_TRIAL_NS)
            .ok_or_else(|| eyre!("invalid original collection deadline"))?;
        Ok(Self {
            end_ns,
            instant: instant
                .checked_add(Duration::from_nanos(remaining))
                .ok_or_else(|| eyre!("collection deadline overflow"))?,
        })
    }
    pub(super) fn check(&self) -> Result<()> {
        ensure!(
            monotonic_ns()? < self.end_ns && Instant::now() < self.instant,
            "original collection deadline exceeded"
        );
        Ok(())
    }
    pub(super) fn instant(&self) -> Instant {
        self.instant
    }
}

/// The exact retained private file underlying the already-granted descriptor.
pub(super) struct OriginalClient {
    input: RetainedOriginalInput,
    fd: u32,
    deadline: Deadline,
}
impl OriginalClient {
    pub(super) fn admit(args: &Args, fd: u32, path: &Path) -> Result<(Self, Config)> {
        let deadline = Deadline::admit(args.deadline_monotonic_ns)?;
        let input = RetainedOriginalInput::open(OriginalInputBinding {
            path: path.to_owned(),
            raw_sha256: raw_sha256(&args.client_config_sha256)?,
            max_bytes: args.client_config_max_bytes,
        })?;
        let owner = Self {
            input,
            fd,
            deadline,
        };
        owner.check()?;
        owner.input.with_bytes(|bytes| {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| eyre!("invalid original client encoding"))?;
            let table: toml::Table = text
                .parse()
                .map_err(|_| eyre!("invalid original client TOML"))?;
            fixed_config(&PrivateTable(table).0)
        })?;
        // This is the existing native inherited loader, with no environment override or fallback.
        let (config, _) = crate::client_config::load_inherited(fd, path)
            .map_err(|_| eyre!("original scaling client failed semantic admission"))?;
        ensure!(
            config.network_id == args.network_id,
            "original scaling client network mismatch"
        );
        owner.check()?;
        Ok((owner, config))
    }
    pub(super) fn check(&self) -> Result<()> {
        self.deadline.check()?;
        self.input.require_descriptor(self.fd)?;
        self.deadline.check()
    }
    pub(super) fn client(&self, config: &Config) -> Result<Client> {
        self.check()?;
        let client = Client::builder(config.clone())
            .build()?
            .with_request_deadline(self.deadline.instant());
        self.check()?;
        Ok(client)
    }
}

struct PrivateTable(toml::Table);
impl zeroize::Zeroize for PrivateTable {
    fn zeroize(&mut self) {
        fn erase(value: &mut toml::Value) {
            match value {
                toml::Value::String(text) => zeroize::Zeroize::zeroize(text),
                toml::Value::Array(values) => values.iter_mut().for_each(erase),
                toml::Value::Table(values) => values.iter_mut().for_each(|(_, value)| erase(value)),
                _ => {}
            }
        }
        self.0.iter_mut().for_each(|(_, value)| erase(value));
    }
}
impl Drop for PrivateTable {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(self);
    }
}

fn exact_keys(table: &toml::Table, keys: &[&str]) -> Result<()> {
    ensure!(
        table.len() == keys.len() && keys.iter().all(|key| table.contains_key(*key)),
        "original scaling client has unsupported fields"
    );
    Ok(())
}
fn fixed_config(table: &toml::Table) -> Result<()> {
    // Sole current fixed-generator profile: no inheritance or any external configuration source.
    exact_keys(
        table,
        &[
            "chain",
            "network_id",
            "torii_url",
            "transaction",
            "account",
            "basic_auth",
        ],
    )?;
    for (name, keys) in [
        (
            "transaction",
            &["time_to_live_ms", "status_timeout_ms", "nonce"][..],
        ),
        (
            "account",
            &["domain", "chain_discriminant", "private_key", "public_key"][..],
        ),
        ("basic_auth", &["password", "web_login"][..]),
    ] {
        exact_keys(
            table
                .get(name)
                .and_then(toml::Value::as_table)
                .ok_or_else(|| eyre!("original scaling client requires fixed tables"))?,
            keys,
        )?;
    }
    Ok(())
}

// Match Python time.monotonic_ns() on the two supported launcher hosts. Instant is an opaque
// process-local value and must never be serialized or mistaken for the host clock's epoch.
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "read Darwin's fixed mach_absolute_time and timebase APIs without mutable global state"
)]
fn monotonic_ns() -> Result<u64> {
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> std::ffi::c_int;
    }
    let mut base = Timebase { numer: 0, denom: 0 };
    // SAFETY: the platform function writes one correctly sized timebase value.
    ensure!(
        unsafe { mach_timebase_info(&raw mut base) } == 0 && base.numer > 0 && base.denom > 0,
        "host monotonic timebase unavailable"
    );
    // SAFETY: this platform clock function has no arguments or caller-owned storage.
    let ticks = unsafe { mach_absolute_time() };
    let ns = u128::from(ticks) * u128::from(base.numer) / u128::from(base.denom);
    u64::try_from(ns).map_err(|_| eyre!("host monotonic clock overflow"))
}
#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "read Linux's fixed CLOCK_MONOTONIC API matching the launcher clock"
)]
fn monotonic_ns() -> Result<u64> {
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanos: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: std::ffi::c_int, value: *mut Timespec) -> std::ffi::c_int;
    }
    let mut value = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // SAFETY: Linux CLOCK_MONOTONIC writes one native timespec to valid caller storage.
    ensure!(
        unsafe { clock_gettime(1, &raw mut value) } == 0,
        "host monotonic clock unavailable"
    );
    let seconds = u64::try_from(value.seconds).map_err(|_| eyre!("invalid monotonic seconds"))?;
    let nanos = u64::try_from(value.nanos).map_err(|_| eyre!("invalid monotonic nanoseconds"))?;
    ensure!(nanos < 1_000_000_000, "invalid monotonic nanoseconds");
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(nanos))
        .ok_or_else(|| eyre!("host monotonic clock overflow"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn monotonic_ns() -> Result<u64> {
    Err(eyre!("fixed scaling clock is unsupported on this host"))
}

#[cfg(test)]
mod tests;
