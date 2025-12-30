//! Simple per-client congestion control for the SoraNet relay.
//!
//! The controller throttles repeated handshake attempts from the same remote
//! peer and limits the number of simultaneous circuits each remote may
//! establish. It is intentionally conservative; production operators should
//! tune the limits via configuration once traffic characteristics are known.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::config::CongestionConfig;

#[derive(Debug)]
/// Per-remote circuit accounting state.
struct ClientState {
    active: u32,
    last_attempt: Instant,
}

#[derive(Debug)]
struct CongestionInner {
    limits: CongestionConfig,
    cooldown: Duration,
    state: Mutex<HashMap<SocketAddr, ClientState>>,
}

impl CongestionInner {
    fn new(config: CongestionConfig) -> Self {
        let cooldown = Duration::from_millis(config.handshake_cooldown_millis);
        Self {
            limits: config,
            cooldown,
            state: Mutex::new(HashMap::new()),
        }
    }

    fn reserve(
        self: &Arc<Self>,
        remote: SocketAddr,
        now: Instant,
    ) -> Result<Reservation, CongestionError> {
        let mut guard = self.state.lock().expect("congestion state poisoned");
        let entry = guard.entry(remote).or_insert_with(|| ClientState {
            active: 0,
            last_attempt: now.checked_sub(self.cooldown).unwrap_or(now),
        });

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
        entry.active = entry.active.saturating_add(1);

        drop(guard);

        Ok(Reservation {
            inner: Arc::clone(self),
            remote,
            active: true,
        })
    }

    fn release(self: &Arc<Self>, remote: SocketAddr) {
        let mut guard = self.state.lock().expect("congestion state poisoned");
        if let Some(entry) = guard.get_mut(&remote) {
            if entry.active > 0 {
                entry.active -= 1;
            }
            if entry.active == 0 {
                guard.remove(&remote);
            }
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
