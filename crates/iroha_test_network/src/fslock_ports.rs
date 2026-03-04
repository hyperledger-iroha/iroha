//! [`fslock`]-based socket ports locking for test network peers,
//! supporting inter-process and intra-process test execution scenarios.
//!
//! Known issues:
//! - `.lock` file persists and is not deleted
//! - locking and recreating JSON file for each port each time is suboptimal, could be optimised
//! - _sometimes_ locked ports aren't cleaned after test execution (_usually_ on failures)

use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::{self, Read, Write},
    sync::Once,
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::{Result, eyre::Context};
use derive_more::{Deref, Display};
use norito::json::{JsonDeserialize, JsonSerialize};

#[cfg(unix)]
const TARGET_NOFILE_LIMIT: u64 = 4_096;

// Prefer ports below the OS ephemeral range to avoid racing outbound client sockets.
const PORT_RANGE_START: u16 = 30_000;
const PORT_RANGE_PREFERRED_END: u16 = 49_151;
const PORT_RANGE_FALLBACK_END: u16 = 65_535;
const PORT_RANGE_EPHEMERAL_START: u16 = 49_152;

const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.iroha_test_network_run.json");
const LOCK_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.iroha_test_network_run.json.lock"
);

#[derive(JsonSerialize, JsonDeserialize, Default)]
struct LockContent {
    ports_in_use: BTreeSet<u16>,
}

fn randomized_port_start() -> u16 {
    let range = u64::from(PORT_RANGE_PREFERRED_END - PORT_RANGE_START + 1);
    if range == 0 {
        return PORT_RANGE_START;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let pid = u128::from(std::process::id());
    let seed = now ^ (pid << 32);
    let offset = u64::try_from(seed % u128::from(range)).unwrap_or(0);
    PORT_RANGE_START.saturating_add(offset as u16)
}

impl LockContent {
    fn read() -> Result<Self> {
        if !std::fs::exists(DATA_FILE)? {
            return Ok(LockContent::default());
        }

        let mut content = String::new();
        match OpenOptions::new()
            .read(true)
            .open(DATA_FILE)
            .wrap_err("failed to open file")
            .and_then(|mut file| {
                file.read_to_string(&mut content)
                    .wrap_err("failed to read file")
            }) {
            Ok(_) => {
                if content.trim().is_empty() {
                    tracing::info!("Lock file {DATA_FILE} was empty; resetting to defaults");
                    return Ok(LockContent::default());
                }
                match norito::json::from_str(&content) {
                    Ok(parsed) => Ok(Self::prune_stale_ports(parsed)),
                    Err(parse_err) => {
                        tracing::info!(
                            ?parse_err,
                            "Lock file {DATA_FILE} was corrupt; resetting to defaults"
                        );
                        Ok(LockContent::default())
                    }
                }
            }
            Err(err) => Result::<LockContent>::Err(err).wrap_err_with(|| {
                format!(
                    "Failed to read lock file at {}. Remove it manually to proceed.",
                    DATA_FILE
                )
            }),
        }
    }

    fn prune_stale_ports(mut content: Self) -> Self {
        content.ports_in_use.retain(|port| {
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, *port)).is_err() || {
                tracing::info!(port, "dropping stale port reservation");
                false
            }
        });
        content
    }

    fn write(&self) -> Result<()> {
        if std::fs::exists(DATA_FILE)? {
            std::fs::remove_file(DATA_FILE)?;
        }
        if self.ports_in_use.is_empty() {
            return Ok(());
        };
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(DATA_FILE)?;
        file.write_all(norito::json::to_json(self).unwrap().as_bytes())?;
        Ok(())
    }
}

/// Releases the port on [`Drop`].
#[derive(Debug, Deref, Display)]
pub struct AllocatedPort(u16);

impl AllocatedPort {
    #[allow(clippy::new_without_default)] // has side effects
    pub fn new() -> Self {
        ensure_fd_limit();

        static START_UNIQUE_PORT: Once = Once::new();
        START_UNIQUE_PORT.call_once(|| {
            // Randomize the starting port to reduce collisions across overlapping test runs.
            let _ = unique_port::set_port_index(randomized_port_start());
        });

        let mut lock = fslock::LockFile::open(LOCK_FILE).expect("path is valid");
        lock.lock().expect("this handle doesn't own the file yet");

        let mut value = LockContent::read().expect("should be able to read the data");
        // Track ports that must be avoided for this allocation attempt. Start with the
        // persisted set but do not persist transient failures.
        let mut excluded = value.ports_in_use.clone();

        fn try_os_ephemeral() -> Option<u16> {
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .ok()
                .and_then(|sock| sock.local_addr().ok().map(|addr| addr.port()))
        }

        fn get_free_port(avoid: &BTreeSet<u16>) -> Option<u16> {
            // 1) Try helper crate
            if let Ok(p) = unique_port::get_unique_free_port()
                && !avoid.contains(&p)
            {
                return Some(p);
            }
            // 2) Try OS-provided ephemeral bind; if it lands in the preferred range, accept it.
            if let Some(p) = try_os_ephemeral()
                && !avoid.contains(&p)
            {
                return Some(p);
            }
            // 3) Last resort: scan preferred range first, then fall back to the rest.
            (PORT_RANGE_START..=PORT_RANGE_PREFERRED_END)
                .chain(PORT_RANGE_EPHEMERAL_START..=PORT_RANGE_FALLBACK_END)
                .find(|&p| !avoid.contains(&p))
        }

        let mut i = 0;
        let port = loop {
            let Some(port) = get_free_port(&excluded) else {
                // If all strategies failed, abort with a clear error to avoid using port 0
                panic!("Failed to get empty port");
            };
            if excluded.contains(&port) {
                i += 1;
                assert!(i < 1000, "cannot find a free port");
                continue;
            }
            // Double-check by binding to the candidate to avoid reusing a busy socket when
            // the lock file is stale or another process bypassed the allocator.
            match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) {
                Ok(listener) => drop(listener),
                Err(err) => {
                    if err.kind() == io::ErrorKind::PermissionDenied {
                        panic!(
                            "port allocation failed: binding to 127.0.0.1:{port} was denied \
                             (Operation not permitted). Integration tests require local \
                             socket binds; allow loopback networking or run outside the sandbox."
                        );
                    }
                    tracing::warn!(port, %err, "port allocation skipped: bind failed");
                    excluded.insert(port);
                    i += 1;
                    assert!(i < 1000, "cannot find a free port");
                    continue;
                }
            }
            break port;
        };

        value.ports_in_use.insert(port);

        value.write().expect("should be able to write the data");
        lock.unlock().expect("this handle still holds the lock");

        Self(port)
    }
}

impl Drop for AllocatedPort {
    fn drop(&mut self) {
        let mut lock = fslock::LockFile::open(LOCK_FILE).expect("path is valid");
        lock.lock().expect("doesn't hold it yet");
        let mut value = LockContent::read().expect("should read fine");
        value.ports_in_use.remove(&self.0);
        value.write().expect("should save the result filne");
        lock.unlock().expect("still holds it");
    }
}

/// Reserves a contiguous block of ports and releases them on drop.
#[derive(Debug)]
pub struct AllocatedPortBlock {
    base: u16,
    count: u16,
}

impl AllocatedPortBlock {
    #[allow(clippy::new_without_default)] // has side effects
    pub fn new(count: u16) -> Self {
        ensure_fd_limit();
        assert!(count > 0, "port block must reserve at least one port");

        let mut lock = fslock::LockFile::open(LOCK_FILE).expect("path is valid");
        lock.lock().expect("this handle doesn't own the file yet");

        let mut value = LockContent::read().expect("should be able to read the data");
        let excluded = value.ports_in_use.clone();

        fn block_end(base: u16, count: u16) -> Option<u16> {
            base.checked_add(count.saturating_sub(1))
        }

        fn block_available(base: u16, count: u16, excluded: &BTreeSet<u16>) -> bool {
            let Some(end) = block_end(base, count) else {
                return false;
            };
            !(base..=end).any(|port| excluded.contains(&port))
        }

        fn block_bindable(base: u16, count: u16) -> bool {
            let Some(end) = block_end(base, count) else {
                return false;
            };
            for port in base..=end {
                match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) {
                    Ok(listener) => drop(listener),
                    Err(err) => {
                        if err.kind() == io::ErrorKind::PermissionDenied {
                            panic!(
                                "port allocation failed: binding to 127.0.0.1:{port} was denied \
                                 (Operation not permitted). Integration tests require local \
                                 socket binds; allow loopback networking or run outside the sandbox."
                            );
                        }
                        tracing::warn!(port, %err, "port block allocation skipped: bind failed");
                        return false;
                    }
                }
            }
            true
        }

        fn try_range(start: u16, end: u16, count: u16, excluded: &BTreeSet<u16>) -> Option<u16> {
            let span = end.saturating_sub(start).saturating_add(1);
            if span < count {
                return None;
            }
            let max_base = end.saturating_sub(count.saturating_sub(1));
            let mut candidate = if start == PORT_RANGE_START {
                randomized_port_start()
            } else {
                start
            };
            if candidate < start {
                candidate = start;
            }
            if candidate > max_base {
                candidate = start;
            }
            let is_candidate =
                |base| block_available(base, count, excluded) && block_bindable(base, count);
            (candidate..=max_base)
                .find(|&base| is_candidate(base))
                .or_else(|| (start..candidate).find(|&base| is_candidate(base)))
        }

        let base = try_range(PORT_RANGE_START, PORT_RANGE_PREFERRED_END, count, &excluded)
            .or_else(|| {
                try_range(
                    PORT_RANGE_EPHEMERAL_START,
                    PORT_RANGE_FALLBACK_END,
                    count,
                    &excluded,
                )
            })
            .unwrap_or_else(|| panic!("Failed to get empty port block"));

        let end = block_end(base, count).expect("block range should fit in u16");
        value.ports_in_use.extend(base..=end);
        value.write().expect("should be able to write the data");
        lock.unlock().expect("this handle still holds the lock");

        Self { base, count }
    }

    /// First port in the reserved block.
    pub fn base(&self) -> u16 {
        self.base
    }

    /// Number of ports reserved in the block.
    pub fn count(&self) -> u16 {
        self.count
    }
}

impl Drop for AllocatedPortBlock {
    fn drop(&mut self) {
        let mut lock = fslock::LockFile::open(LOCK_FILE).expect("path is valid");
        lock.lock().expect("doesn't hold it yet");
        let mut value = LockContent::read().expect("should read fine");
        if let Some(end) = self.base.checked_add(self.count.saturating_sub(1)) {
            for port in self.base..=end {
                value.ports_in_use.remove(&port);
            }
        }
        value.write().expect("should save the result file");
        lock.unlock().expect("still holds it");
    }
}

fn ensure_fd_limit() {
    #[cfg(unix)]
    {
        use nix::sys::resource::{Resource, getrlimit, setrlimit};

        static RAISE_NOFILE_LIMIT: Once = Once::new();
        RAISE_NOFILE_LIMIT.call_once(|| {
            let Ok((soft, hard)) = getrlimit(Resource::RLIMIT_NOFILE) else {
                tracing::debug!("failed to query RLIMIT_NOFILE");
                return;
            };

            let desired = desired_soft_limit(hard);

            if soft >= desired {
                tracing::trace!(
                    soft_limit = soft,
                    desired_limit = desired,
                    hard_limit = hard,
                    "RLIMIT_NOFILE already satisfies requirement"
                );
                return;
            }

            if let Err(error) = setrlimit(Resource::RLIMIT_NOFILE, desired, hard) {
                tracing::warn!(
                    %error,
                    soft_limit = soft,
                    desired_limit = desired,
                    hard_limit = hard,
                    "failed to raise RLIMIT_NOFILE soft limit"
                );
            } else {
                tracing::debug!(
                    soft_before = soft,
                    soft_after = desired,
                    hard_limit = hard,
                    "raised RLIMIT_NOFILE soft limit for test network"
                );
            }
        });
    }
}

#[cfg(unix)]
fn desired_soft_limit(hard: u64) -> u64 {
    if hard == nix::sys::resource::RLIM_INFINITY {
        TARGET_NOFILE_LIMIT
    } else {
        hard.min(TARGET_NOFILE_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn desired_soft_limit_respects_hard_cap() {
        assert_eq!(desired_soft_limit(1_024), 1_024);
        assert_eq!(desired_soft_limit(8_192), TARGET_NOFILE_LIMIT);
        assert_eq!(
            desired_soft_limit(nix::sys::resource::RLIM_INFINITY),
            TARGET_NOFILE_LIMIT
        );
    }

    #[test]
    fn allocated_port_stays_below_ephemeral_range() {
        let port = AllocatedPort::new();
        assert!(port.0 >= PORT_RANGE_START);
        assert!(
            port.0 < PORT_RANGE_EPHEMERAL_START,
            "allocated port should avoid OS ephemeral range; got {}",
            port.0
        );
    }

    #[test]
    fn allocated_port_block_reserves_consecutive_ports() {
        let block = AllocatedPortBlock::new(4);
        let base = block.base();
        let count = block.count();
        let end = base
            .checked_add(count.saturating_sub(1))
            .expect("block range fits in u16");
        assert!(base >= PORT_RANGE_START);
        assert_eq!(
            end.saturating_sub(base).saturating_add(1),
            count,
            "allocated port block should be contiguous"
        );
    }

    #[test]
    fn randomized_port_start_is_within_preferred_range() {
        let start = randomized_port_start();
        assert!(start >= PORT_RANGE_START);
        assert!(start <= PORT_RANGE_PREFERRED_END);
    }
}
