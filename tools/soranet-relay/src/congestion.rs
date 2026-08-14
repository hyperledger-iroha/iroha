//! Simple per-client congestion control for the SoraNet relay.
//!
//! The controller throttles repeated handshake attempts from the same remote
//! peer and limits the number of simultaneous circuits each remote may
//! establish. It is intentionally conservative; production operators should
//! tune the limits via configuration once traffic characteristics are known.
use crate::config::{CONGESTION_MAX_ACTIVE_CIRCUITS_V1, CongestionConfig};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use thiserror::Error;
#[derive(Debug)]
/// Per-remote circuit accounting state.
struct ClientState {
    active: u32,
    last_attempt: Instant,
}
#[derive(Debug, Default)]
struct CongestionState {
    clients: HashMap<SocketAddr, ClientState>,
    active_circuits: usize,
}
#[derive(Debug)]
struct CongestionInner {
    limits: CongestionConfig,
    cooldown: Duration,
    state: Mutex<CongestionState>,
}
impl CongestionInner {
    fn new(mut config: CongestionConfig) -> Self {
        // RelayConfig rejects invalid values. Clamp direct programmatic callers
        // as a second line of defense so this constructor can never recreate
        // an unbounded pre-admission map.
        config.apply_defaults();
        config.max_active_circuits = config
            .max_active_circuits
            .min(CONGESTION_MAX_ACTIVE_CIRCUITS_V1);
        config.max_circuits_per_client = config
            .max_circuits_per_client
            .min(u32::try_from(config.max_active_circuits).unwrap_or(u32::MAX));
        let cooldown = Duration::from_millis(config.handshake_cooldown_millis);
        Self {
            limits: config,
            cooldown,
            state: Mutex::new(CongestionState::default()),
        }
    }
    fn reserve(
        self: &Arc<Self>,
        remote: SocketAddr,
        now: Instant,
    ) -> Result<Reservation, CongestionError> {
        let mut guard = self.state.lock().expect("congestion state poisoned");
        if guard.active_circuits >= self.limits.max_active_circuits {
            return Err(CongestionError::GlobalCircuitCapacity {
                limit: self.limits.max_active_circuits,
            });
        }
        if !guard.clients.contains_key(&remote) {
            guard
                .clients
                .try_reserve(1)
                .map_err(|_| CongestionError::GlobalCircuitCapacity {
                    limit: self.limits.max_active_circuits,
                })?;
            guard.clients.insert(
                remote,
                ClientState {
                    active: 1,
                    last_attempt: now,
                },
            );
            guard.active_circuits += 1;
            drop(guard);
            return Ok(Reservation {
                inner: Arc::clone(self),
                remote,
                active: true,
            });
        }
        let entry = guard
            .clients
            .get_mut(&remote)
            .expect("client inserted before congestion checks");
        if entry.active >= self.limits.max_circuits_per_client {
            return Err(CongestionError::TooManyCircuits {
                limit: self.limits.max_circuits_per_client,
            });
        }
        let since_last = now
            .checked_duration_since(entry.last_attempt)
            .unwrap_or_default();
        if since_last < self.cooldown {
            entry.last_attempt = now;
            return Err(CongestionError::HandshakeCooldown {
                cooldown_millis: self.limits.handshake_cooldown_millis,
                observed_gap_millis: since_last.as_millis() as u64,
            });
        }
        entry.last_attempt = now;
        entry.active += 1;
        guard.active_circuits += 1;
        drop(guard);
        Ok(Reservation {
            inner: Arc::clone(self),
            remote,
            active: true,
        })
    }
    fn release(self: &Arc<Self>, remote: SocketAddr) {
        let mut guard = self.state.lock().expect("congestion state poisoned");
        let (released, remove) = if let Some(entry) = guard.clients.get_mut(&remote) {
            let released = entry.active > 0;
            if entry.active > 0 {
                entry.active -= 1;
            }
            (released, entry.active == 0)
        } else {
            (false, false)
        };
        if released {
            guard.active_circuits = guard.active_circuits.saturating_sub(1);
        }
        if remove {
            guard.clients.remove(&remote);
        }
    }
}
/// Enforces per-client handshake throttling and circuit limits.
#[derive(Debug, Clone)]
pub struct CongestionController {
    inner: Arc<CongestionInner>,
}
impl CongestionController {
    /// Create a new controller instance.
    #[must_use]
    pub fn new(config: CongestionConfig) -> Self {
        Self {
            inner: Arc::new(CongestionInner::new(config)),
        }
    }
    /// Attempt to reserve capacity for a handshake originating from `remote`.
    ///
    /// Returns a [`Reservation`] guard when successful. Dropping the guard will
    /// release the reserved slot unless it has been converted into a
    /// [`CongestionLease`] after the handshake succeeds.
    pub fn reserve(
        &self,
        remote: SocketAddr,
        now: Instant,
    ) -> Result<Reservation, CongestionError> {
        self.inner.reserve(remote, now)
    }
    fn release(&self, remote: SocketAddr) {
        self.inner.release(remote);
    }
}
/// Reservation guard returned when capacity is available.
pub struct Reservation {
    inner: Arc<CongestionInner>,
    remote: SocketAddr,
    active: bool,
}
impl Reservation {
    /// Commit the reservation and obtain a lease that keeps the circuit counted
    /// until it is dropped (typically when the QUIC connection closes).
    pub fn into_lease(mut self) -> CongestionLease {
        self.active = false;
        CongestionLease {
            controller: CongestionController {
                inner: Arc::clone(&self.inner),
            },
            remote: self.remote,
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.active {
            let controller = CongestionController {
                inner: Arc::clone(&self.inner),
            };
            let remote = self.remote;
            controller.release(remote);
        }
    }
}
/// Active circuit lease. Dropping the lease releases the slot.
pub struct CongestionLease {
    controller: CongestionController,
    remote: SocketAddr,
}
impl CongestionLease {
    /// Remote address tracked by this lease.
    #[allow(dead_code)]
    pub fn remote(&self) -> SocketAddr {
        self.remote
    }
}
impl Drop for CongestionLease {
    fn drop(&mut self) {
        let controller = self.controller.clone();
        let remote = self.remote;
        controller.release(remote);
    }
}
/// Errors returned when a handshake is throttled by congestion control.
#[derive(Debug, Error)]
pub enum CongestionError {
    /// The relay reached its global active-circuit memory corridor.
    #[error("maximum simultaneous relay circuits exceeded (limit: {limit})")]
    GlobalCircuitCapacity { limit: usize },
    /// The remote attempted to exceed the configured maximum number of active circuits.
    #[error("maximum simultaneous circuits per client exceeded (limit: {limit})")]
    TooManyCircuits { limit: u32 },
    /// Handshakes are occurring faster than the configured cooldown interval.
    #[error(
        "handshake attempts throttled; cooldown {cooldown_millis} ms, observed gap {observed_gap_millis} ms"
    )]
    HandshakeCooldown {
        cooldown_millis: u64,
        observed_gap_millis: u64,
    },
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    fn controller(max_active_circuits: usize) -> CongestionController {
        CongestionController::new(CongestionConfig {
            max_circuits_per_client: 2,
            max_active_circuits,
            handshake_cooldown_millis: 1,
        })
    }
    #[test]
    fn global_capacity_rejects_before_map_overshoot_and_recovers_on_release() {
        let controller = controller(2);
        let now = Instant::now();
        let first = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_001);
        let second = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_002);
        let overflow = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_003);
        let first_reservation = controller.reserve(first, now).expect("first slot");
        let _second_reservation = controller.reserve(second, now).expect("second slot");
        assert!(matches!(
            controller.reserve(overflow, now),
            Err(CongestionError::GlobalCircuitCapacity { limit: 2 })
        ));
        {
            let state = controller.inner.state.lock().expect("congestion state");
            assert_eq!(state.active_circuits, 2);
            assert_eq!(state.clients.len(), 2);
            assert!(!state.clients.contains_key(&overflow));
        }
        drop(first_reservation);
        let _replacement = controller
            .reserve(overflow, now)
            .expect("released capacity is reusable");
        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 2);
        assert_eq!(state.clients.len(), 2);
        assert!(state.clients.contains_key(&overflow));
    }
    #[test]
    fn programmatic_configuration_is_clamped_to_memory_corridor() {
        let controller = CongestionController::new(CongestionConfig {
            max_circuits_per_client: u32::MAX,
            max_active_circuits: usize::MAX,
            handshake_cooldown_millis: u64::MAX,
        });
        assert_eq!(
            controller.inner.limits.max_active_circuits,
            CONGESTION_MAX_ACTIVE_CIRCUITS_V1
        );
        assert_eq!(
            usize::try_from(controller.inner.limits.max_circuits_per_client)
                .expect("u32 fits usize on supported targets"),
            CONGESTION_MAX_ACTIVE_CIRCUITS_V1
        );
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_004);
        let _reservation = controller
            .reserve(remote, Instant::now())
            .expect("a first attempt must not be retained as an inactive cooldown entry");
        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 1);
        assert_eq!(state.clients.len(), 1);
    }
}
