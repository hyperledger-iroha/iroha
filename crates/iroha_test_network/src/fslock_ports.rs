//! Cross-process test port allocation using lifetime-held OS file leases.
//!
//! Reservations live outside the source tree and remain exclusive before the
//! peer binds its socket. Dropping an allocation or exiting its process releases
//! the lease automatically; the small lock files retain stable inodes.
mod port_lease;
use port_lease::{PortLease, PortLeasePool, runtime_lease_directory};
use std::{
    fmt,
    ops::Deref,
    sync::Once,
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(unix)]
const TARGET_NOFILE_LIMIT: u64 = 4_096;
const PORT_RANGE_START: u16 = 30_000;
const PORT_RANGE_PREFERRED_END: u16 = 49_151;
const PORT_RANGE_FALLBACK_END: u16 = 65_535;
const PORT_RANGE_EPHEMERAL_START: u16 = 49_152;

fn randomized_port_start() -> u16 {
    let range = u128::from(PORT_RANGE_PREFERRED_END - PORT_RANGE_START + 1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let offset = (now ^ (u128::from(std::process::id()) << 32)) % range;
    PORT_RANGE_START + u16::try_from(offset).expect("port offset fits preferred range")
}

fn allocate(count: u16) -> Vec<PortLease> {
    ensure_fd_limit();
    assert!(count > 0, "port block must reserve at least one port");
    let owner = nix::unistd::Uid::effective().as_raw();
    let directory =
        runtime_lease_directory(owner).expect("host runtime temporary directory is available");
    let pool =
        PortLeasePool::open(&directory, owner).expect("owner-private test port lease directory");
    pool.allocate(
        count,
        PORT_RANGE_START..=PORT_RANGE_PREFERRED_END,
        randomized_port_start(),
    )
    .and_then(|leases| match leases {
        Some(leases) => Ok(Some(leases)),
        None => pool.allocate(
            count,
            PORT_RANGE_EPHEMERAL_START..=PORT_RANGE_FALLBACK_END,
            PORT_RANGE_EPHEMERAL_START,
        ),
    })
    .expect("test port allocation requires local socket and OS file-lock access")
    .expect("no free contiguous test port range is available")
}

/// One reserved port; its OS lease is held until this value is dropped.
#[derive(Debug)]
pub struct AllocatedPort {
    port: u16,
    _lease: PortLease,
}
impl AllocatedPort {
    /// Allocate an available port and retain its cross-process reservation.
    #[allow(clippy::new_without_default)] // has side effects
    pub fn new() -> Self {
        let lease = allocate(1).pop().expect("one port was allocated");
        Self {
            port: lease.port(),
            _lease: lease,
        }
    }
}
impl Deref for AllocatedPort {
    type Target = u16;
    fn deref(&self) -> &u16 {
        &self.port
    }
}
impl fmt::Display for AllocatedPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.port, formatter)
    }
}

/// A contiguous block whose individual port leases are retained for its lifetime.
#[derive(Debug)]
pub struct AllocatedPortBlock {
    base: u16,
    count: u16,
    _leases: Vec<PortLease>,
}
impl AllocatedPortBlock {
    /// Reserve `count` consecutive ports; a partially available block is never returned.
    pub fn new(count: u16) -> Self {
        let leases = allocate(count);
        Self {
            base: leases[0].port(),
            count,
            _leases: leases,
        }
    }
    /// First port in the reserved block.
    pub const fn base(&self) -> u16 {
        self.base
    }
    /// Number of ports reserved in the block.
    pub const fn count(&self) -> u16 {
        self.count
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
    fn public_port_api_retains_deref_display_and_block_geometry() {
        let port = AllocatedPort::new();
        assert!(*port >= PORT_RANGE_START);
        assert_eq!(port.to_string(), (*port).to_string());
        let block = AllocatedPortBlock::new(4);
        assert_eq!(block.count(), 4);
        assert!(block.base() >= PORT_RANGE_START);
        assert!(block.base().checked_add(block.count() - 1).is_some());
        let owner = nix::unistd::Uid::effective().as_raw();
        let pool = PortLeasePool::open(&runtime_lease_directory(owner).unwrap(), owner).unwrap();
        assert!(pool.try_port(*port).unwrap().is_none());
        for port in block.base()..=block.base() + block.count() - 1 {
            assert!(pool.try_port(port).unwrap().is_none());
        }
    }
    #[test]
    fn randomized_port_start_is_within_preferred_range() {
        assert!((PORT_RANGE_START..=PORT_RANGE_PREFERRED_END).contains(&randomized_port_start()));
    }
}
