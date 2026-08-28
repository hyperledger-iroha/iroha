//! Simple per-client congestion control for the SoraNet relay.
//!
//! The controller throttles repeated handshake attempts from the same remote
//! IP and limits the number of simultaneous circuits each remote IP may
//! establish. It is intentionally conservative; production operators should
//! tune the limits via configuration once traffic characteristics are known.
use crate::{
    canonical_remote_ip,
    config::{CONGESTION_MAX_ACTIVE_CIRCUITS_V1, CongestionConfig},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tracing::warn;
#[derive(Debug)]
/// Per-remote-IP circuit accounting state.
struct ClientState {
    active: u32,
    last_attempt: Instant,
}
#[derive(Debug, Default)]
struct CongestionState {
    // Zero-active entries preserve the cooldown across failed or short-lived
    // handshakes. Admission keeps this map bounded by `max_active_circuits`.
    clients: HashMap<IpAddr, ClientState>,
    active_circuits: usize,
}
#[derive(Debug)]
struct CongestionInner {
    limits: CongestionConfig,
    cooldown: Duration,
    state: Mutex<CongestionState>,
    unavailable: AtomicBool,
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
            unavailable: AtomicBool::new(false),
        }
    }
    fn mark_unavailable(&self) {
        if !self.unavailable.swap(true, Ordering::AcqRel) {
            warn!("congestion state poisoned; rejecting future circuit reservations");
        }
    }
    fn reserve(
        self: &Arc<Self>,
        remote: SocketAddr,
        now: Instant,
    ) -> Result<Reservation, CongestionError> {
        if self.unavailable.load(Ordering::Acquire) {
            return Err(CongestionError::StateUnavailable);
        }
        let mut guard = self.state.lock().map_err(|_| {
            self.mark_unavailable();
            CongestionError::StateUnavailable
        })?;
        if guard.active_circuits >= self.limits.max_active_circuits {
            return Err(CongestionError::GlobalCircuitCapacity {
                limit: self.limits.max_active_circuits,
            });
        }
        let client = canonical_remote_ip(remote);
        if !guard.clients.contains_key(&client) {
            if guard.clients.len() >= self.limits.max_active_circuits {
                // Cooldown history must not consume the entire live circuit
                // corridor. Evict the oldest inactive tombstone (expired or
                // otherwise) while protecting every live reservation.
                let oldest_inactive = guard
                    .clients
                    .iter()
                    .filter(|(_, state)| state.active == 0)
                    .min_by_key(|(_, state)| state.last_attempt)
                    .map(|(ip, _)| *ip);
                let Some(oldest_inactive) = oldest_inactive else {
                    return Err(CongestionError::GlobalCircuitCapacity {
                        limit: self.limits.max_active_circuits,
                    });
                };
                let removed = guard.clients.remove(&oldest_inactive);
                debug_assert!(removed.is_some(), "selected tombstone must remain present");
            }
            guard
                .clients
                .try_reserve(1)
                .map_err(|_| CongestionError::GlobalCircuitCapacity {
                    limit: self.limits.max_active_circuits,
                })?;
            guard.clients.insert(
                client,
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
            .get_mut(&client)
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
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(error) => {
                // Reservations and leases release from Drop. Recover the guard
                // for best-effort accounting only after permanently disabling
                // future admission, so cleanup can never double-panic.
                self.mark_unavailable();
                error.into_inner()
            }
        };
        let client = canonical_remote_ip(remote);
        let released = if let Some(entry) = guard.clients.get_mut(&client) {
            let released = entry.active > 0;
            if entry.active > 0 {
                entry.active -= 1;
            }
            released
        } else {
            false
        };
        if released {
            guard.active_circuits = guard.active_circuits.saturating_sub(1);
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
    /// Congestion accounting state is unavailable and admission cannot proceed safely.
    #[error("congestion state is unavailable")]
    StateUnavailable,
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
        let first = SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 10_001);
        let second = SocketAddr::new(IpAddr::from([127, 0, 0, 2]), 10_002);
        let overflow = SocketAddr::new(IpAddr::from([127, 0, 0, 3]), 10_003);
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
            assert!(!state.clients.contains_key(&overflow.ip()));
        }
        drop(first_reservation);
        let _replacement = controller
            .reserve(overflow, now)
            .expect("inactive cooldown history must not consume live capacity");
        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 2);
        assert_eq!(state.clients.len(), 2);
        assert!(state.clients.contains_key(&overflow.ip()));
    }
    #[test]
    fn inactive_cooldown_saturation_admits_an_unseen_client() {
        let controller = controller(2);
        let now = Instant::now();
        let first = SocketAddr::new(IpAddr::from([192, 0, 2, 1]), 10_030);
        let second = SocketAddr::new(IpAddr::from([192, 0, 2, 2]), 10_031);
        let unseen = SocketAddr::new(IpAddr::from([192, 0, 2, 3]), 10_032);

        drop(controller.reserve(first, now).expect("first tombstone"));
        drop(
            controller
                .reserve(second, now + Duration::from_nanos(1))
                .expect("second tombstone"),
        );
        let _reservation = controller
            .reserve(unseen, now + Duration::from_nanos(2))
            .expect("bounded cooldown history must not deny all unseen clients");

        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 1);
        assert_eq!(state.clients.len(), 2);
        assert!(!state.clients.contains_key(&first.ip()));
        assert!(state.clients.contains_key(&second.ip()));
        assert!(state.clients.contains_key(&unseen.ip()));
        assert_eq!(
            state
                .clients
                .values()
                .filter(|client| client.active == 0)
                .count(),
            1
        );
    }
    #[test]
    fn released_reservation_retains_bounded_cooldown_state() {
        let controller = controller(2);
        let now = Instant::now();
        let first = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_007);
        let rotated = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_008);

        let reservation = controller.reserve(first, now).expect("first slot");
        drop(reservation);
        {
            let state = controller.inner.state.lock().expect("congestion state");
            assert_eq!(state.active_circuits, 0);
            assert_eq!(state.clients.len(), 1);
            assert_eq!(state.clients[&first.ip()].active, 0);
        }
        assert!(matches!(
            controller.reserve(rotated, now),
            Err(CongestionError::HandshakeCooldown { .. })
        ));
        let _reservation = controller
            .reserve(rotated, now + Duration::from_millis(1))
            .expect("same IP is admitted after the cooldown");

        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 1);
        assert_eq!(state.clients.len(), 1);
    }
    #[test]
    fn source_port_rotation_does_not_bypass_client_limits() {
        let controller = controller(4);
        let now = Instant::now();
        let first = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_010);
        let second = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_011);
        let third = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_012);

        let _first = controller.reserve(first, now).expect("first slot");
        assert!(matches!(
            controller.reserve(second, now),
            Err(CongestionError::HandshakeCooldown { .. })
        ));
        let _second = controller
            .reserve(second, now + Duration::from_millis(2))
            .expect("second slot after cooldown");
        assert!(matches!(
            controller.reserve(third, now + Duration::from_millis(4)),
            Err(CongestionError::TooManyCircuits { limit: 2 })
        ));

        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 2);
        assert_eq!(state.clients.len(), 1);
        assert!(state.clients.contains_key(&first.ip()));
    }
    #[test]
    fn ipv4_mapped_ipv6_does_not_create_a_second_client_identity() {
        let controller = controller(4);
        let now = Instant::now();
        let address = Ipv4Addr::new(192, 0, 2, 1);
        let ipv4 = SocketAddr::new(IpAddr::V4(address), 10_020);
        let mapped = SocketAddr::new(IpAddr::V6(address.to_ipv6_mapped()), 10_021);

        let _first = controller.reserve(ipv4, now).expect("first slot");
        assert!(matches!(
            controller.reserve(mapped, now),
            Err(CongestionError::HandshakeCooldown { .. })
        ));
        let _second = controller
            .reserve(mapped, now + Duration::from_millis(2))
            .expect("mapped form shares the same client after cooldown");
        assert!(matches!(
            controller.reserve(ipv4, now + Duration::from_millis(4)),
            Err(CongestionError::TooManyCircuits { limit: 2 })
        ));

        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.clients.len(), 1);
        assert_eq!(state.clients[&IpAddr::V4(address)].active, 2);
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
            .expect("an initial reservation must fit within the clamped bound");
        let state = controller.inner.state.lock().expect("congestion state");
        assert_eq!(state.active_circuits, 1);
        assert_eq!(state.clients.len(), 1);
    }
    #[test]
    fn poisoned_state_rejects_future_reservations_and_drop_does_not_panic() {
        let controller = controller(3);
        let now = Instant::now();
        let first = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_005);
        let reservation = controller.reserve(first, now).expect("initial reservation");
        let poison_target = controller.clone();
        let poisoned = std::thread::spawn(move || {
            let _guard = poison_target
                .inner
                .state
                .lock()
                .expect("congestion state lock");
            panic!("poison congestion state");
        })
        .join();
        assert!(poisoned.is_err(), "poisoning worker must panic");

        let second = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_006);
        assert!(matches!(
            controller.reserve(second, now),
            Err(CongestionError::StateUnavailable)
        ));
        drop(reservation);
        controller.inner.state.clear_poison();
        assert!(matches!(
            controller.reserve(second, now),
            Err(CongestionError::StateUnavailable)
        ));
        let state = controller.inner.state.lock().expect("cleared state lock");
        assert_eq!(state.active_circuits, 0);
        assert_eq!(state.clients.len(), 1);
        assert_eq!(state.clients[&first.ip()].active, 0);
    }
}
