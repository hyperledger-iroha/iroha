//! Circuit tracking and padding scheduler for the SoraNet relay runtime.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use bytes::Bytes;
use quinn::Connection;
use thiserror::Error;
use tokio::{
    task::JoinHandle,
    time::{self, Duration, MissedTickBehavior},
};
use tracing::{debug, warn};

use crate::{
    capability::{ConstantRateCapability, KemId, NegotiatedCapabilities, SignatureId},
    config::PaddingConfig,
    metrics::Metrics,
};

/// Tracks active circuits for introspection/debugging.
#[derive(Debug)]
pub struct CircuitRegistry {
    next_id: AtomicU64,
    inner: RwLock<CircuitRegistryInner>,
}

#[derive(Debug, Default)]
struct CircuitRegistryInner {
    entries: HashMap<u64, CircuitState>,
    constant_rate_active: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CircuitState {
    /// Remote socket address associated with the circuit.
    pub remote: SocketAddr,
    /// Instant when the circuit was registered.
    pub opened_at: Instant,
    /// Negotiated KEM identifier.
    pub kem: KemId,
    /// Negotiated signature suites.
    pub signatures: Vec<KemSignature>,
    /// Padding cell size agreed for the circuit.
    pub padding: u16,
    /// Optional constant-rate capability details.
    pub constant_rate: Option<ConstantRateCapability>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct KemSignature {
    /// Signature algorithm identifier.
    pub id: SignatureId,
    /// Whether the signature algorithm was marked required.
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterCircuitOutcome {
    /// Assigned circuit identifier.
    pub circuit_id: u64,
    /// Updated count of active constant-rate neighbors, if applicable.
    pub constant_rate_active: Option<u64>,
}

#[derive(Debug)]
pub struct CircuitRemoval {
    /// Removed circuit state.
    pub state: CircuitState,
    /// Updated count of active constant-rate neighbors, if applicable.
    pub constant_rate_active: Option<u64>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CircuitAdmissionError {
    /// The relay reached the configured constant-rate neighbor cap.
    #[error("constant-rate neighbor cap {limit} reached")]
    ConstantRateNeighborCap { limit: u16 },
}

impl Default for CircuitRegistry {
    fn default() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            inner: RwLock::new(CircuitRegistryInner::default()),
        }
    }
}

impl CircuitRegistry {
    /// Registers a new circuit and returns its identifier.
    pub fn register(
        &self,
        remote: SocketAddr,
        negotiated: &NegotiatedCapabilities,
        constant_rate_neighbor_cap: Option<u16>,
    ) -> Result<RegisterCircuitOutcome, CircuitAdmissionError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let signatures = negotiated
            .signatures
            .iter()
            .map(|sig| KemSignature {
                id: sig.id,
                required: sig.required,
            })
            .collect::<Vec<_>>();

        let state = CircuitState {
            remote,
            opened_at: Instant::now(),
            kem: negotiated.kem.id,
            signatures,
            padding: negotiated.padding,
            constant_rate: negotiated.constant_rate,
        };
        let is_constant_rate = state.constant_rate.is_some();
        let mut guard = self.inner.write().expect("circuit registry poisoned");
        if let Some(cap) = constant_rate_neighbor_cap
            .filter(|_| is_constant_rate)
            .filter(|cap| guard.constant_rate_active >= u64::from(*cap))
        {
            return Err(CircuitAdmissionError::ConstantRateNeighborCap { limit: cap });
        }
        if is_constant_rate {
            guard.constant_rate_active = guard.constant_rate_active.saturating_add(1);
        }
        guard.entries.insert(id, state);
        Ok(RegisterCircuitOutcome {
            circuit_id: id,
            constant_rate_active: is_constant_rate.then_some(guard.constant_rate_active),
        })
    }

    /// Removes a circuit entry if present.
    pub fn remove(&self, circuit_id: u64) -> Option<CircuitRemoval> {
        let mut guard = self.inner.write().expect("circuit registry poisoned");
        guard.entries.remove(&circuit_id).map(|state| {
            let constant_rate_active = if state.constant_rate.is_some() {
                guard.constant_rate_active = guard.constant_rate_active.saturating_sub(1);
                Some(guard.constant_rate_active)
            } else {
                None
            };
            CircuitRemoval {
                state,
                constant_rate_active,
            }
        })
    }

    /// Returns the number of active circuits currently tracked.
    #[must_use]
    pub fn active_len(&self) -> usize {
        let guard = self.inner.read().expect("circuit registry poisoned");
        guard.entries.len()
    }

    /// Returns the number of active constant-rate circuits.
    #[must_use]
    pub fn constant_rate_active_len(&self) -> u64 {
        let guard = self.inner.read().expect("circuit registry poisoned");
        guard.constant_rate_active
    }

    /// Returns the list of active constant-rate neighbors sorted deterministically.
    #[must_use]
    pub fn constant_rate_neighbors(&self) -> Vec<SocketAddr> {
        let guard = self.inner.read().expect("circuit registry poisoned");
        let mut neighbors = guard
            .entries
            .values()
            .filter(|state| state.constant_rate.is_some())
            .map(|state| state.remote)
            .collect::<Vec<_>>();
        neighbors.sort();
        neighbors.dedup();
        neighbors
    }
}

/// Spawn a task that periodically emits padding cells on an active QUIC connection.
pub fn spawn_padding_task(
    connection: Connection,
    cell_size: u16,
    max_idle_millis: u64,
    remote: SocketAddr,
    metrics: Arc<Metrics>,
    budget: Option<Arc<PaddingBudget>>,
) -> Option<JoinHandle<()>> {
    let safe_cell_size = PaddingConfig::clamp_cell_size(cell_size);
    if safe_cell_size == 0 || max_idle_millis == 0 {
        return None;
    }

    if safe_cell_size != cell_size {
        warn!(
            requested = cell_size,
            clamped = safe_cell_size,
            "clamping padding cell size to MTU-safe limit"
        );
    }

    let payload = Bytes::from(vec![0u8; safe_cell_size as usize]);
    let interval = Duration::from_millis(max_idle_millis);
    Some(tokio::spawn(async move {
        let mut ticker = time::interval(interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Some(budget) = budget.as_ref()
                && !budget.try_acquire(safe_cell_size as u64, Instant::now())
            {
                metrics.record_padding_cell_throttled();
                continue;
            }
            match connection.send_datagram(payload.clone()) {
                Ok(()) => {
                    metrics.record_padding_cell_sent(safe_cell_size as u64);
                    debug!(
                        remote = %remote,
                        size = safe_cell_size,
                        "padding datagram sent"
                    );
                }
                Err(error) => {
                    warn!(remote = %remote, ?error, "failed to send padding datagram");
                    break;
                }
            }
        }
    }))
}

/// Abort a running padding task if present.
pub fn abort_padding_task(task: Option<JoinHandle<()>>) {
    if let Some(handle) = task {
        handle.abort();
    }
}

/// Token bucket limiting padding egress per second with burst allowance.
#[derive(Debug)]
pub struct PaddingBudget {
    limit_per_sec: u64,
    burst_bytes: u64,
    state: Mutex<TokenBucketState>,
}

#[derive(Debug)]
struct TokenBucketState {
    last_refill: Instant,
    tokens: u64,
}

impl PaddingBudget {
    /// Create a new token bucket limiting padding bytes per second.
    #[must_use]
    pub fn new(limit_per_sec: u64, burst_bytes: u64) -> Self {
        let burst = burst_bytes.max(limit_per_sec).max(1);
        Self {
            limit_per_sec,
            burst_bytes: burst,
            state: Mutex::new(TokenBucketState {
                last_refill: Instant::now(),
                tokens: burst,
            }),
        }
    }

    /// Build a [`PaddingBudget`] from the relay padding configuration when enabled.
    #[must_use]
    pub fn from_config(config: &PaddingConfig) -> Option<Self> {
        let limit = config.global_rate_limit_bytes_per_sec;
        if limit == 0 {
            return None;
        }

        let mut burst = if config.burst_bytes == 0 {
            limit
        } else {
            config.burst_bytes
        };
        let min_cell = u64::from(config.cell_size);
        if burst < min_cell {
            burst = min_cell;
        }

        Some(Self::new(limit, burst))
    }

    /// Attempt to reserve `cost` bytes from the bucket. Returns `true` when
    /// sufficient tokens exist, otherwise `false`.
    pub fn try_acquire(&self, cost: u64, now: Instant) -> bool {
        if self.limit_per_sec == 0 {
            return true;
        }

        let mut guard = self.state.lock().expect("padding budget lock poisoned");
        let elapsed = now.saturating_duration_since(guard.last_refill);
        if elapsed.as_nanos() > 0 {
            let added = ((self.limit_per_sec as u128) * elapsed.as_nanos()) / 1_000_000_000u128;
            if added > 0 {
                let capped = (guard.tokens as u128 + added).min(self.burst_bytes as u128);
                guard.tokens = capped as u64;
                guard.last_refill = now;
            }
        }

        if guard.tokens >= cost {
            guard.tokens -= cost;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn limit_per_sec(&self) -> u64 {
        self.limit_per_sec
    }

    #[must_use]
    pub fn burst_bytes(&self) -> u64 {
        self.burst_bytes
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };

    use super::*;
    use crate::{
        capability::{ConstantRateMode, KemAdvertisement, SignatureAdvertisement},
        constant_rate::CONSTANT_RATE_CELL_BYTES,
    };

    fn sample_negotiated() -> NegotiatedCapabilities {
        NegotiatedCapabilities {
            kem: KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            },
            signatures: vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            padding: 1024,
            descriptor_commit: None,
            grease: Vec::new(),
            constant_rate: None,
        }
    }

    fn strict_constant_rate() -> ConstantRateCapability {
        ConstantRateCapability {
            version: 1,
            mode: ConstantRateMode::Strict,
            cell_bytes: CONSTANT_RATE_CELL_BYTES as u16,
        }
    }

    #[test]
    fn registry_registers_and_removes_entries() {
        let registry = CircuitRegistry::default();
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433);
        let negotiated = sample_negotiated();

        let outcome = registry
            .register(remote, &negotiated, None)
            .expect("registry insert");
        assert_eq!(outcome.circuit_id, 0);
        assert!(outcome.constant_rate_active.is_none());

        let removed = registry.remove(outcome.circuit_id);
        assert!(removed.is_some());
        assert!(
            registry
                .inner
                .read()
                .expect("registry lock")
                .entries
                .is_empty()
        );
    }

    #[test]
    fn registry_rejects_when_constant_rate_cap_reached() {
        let registry = CircuitRegistry::default();
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433);
        let mut negotiated = sample_negotiated();
        negotiated.constant_rate = Some(strict_constant_rate());

        for expected in 0..2 {
            let outcome = registry
                .register(remote, &negotiated, Some(2))
                .expect("registry insert");
            assert_eq!(outcome.constant_rate_active, Some(expected + 1));
        }
        let err = registry
            .register(remote, &negotiated, Some(2))
            .expect_err("cap reached");
        assert_eq!(
            err,
            CircuitAdmissionError::ConstantRateNeighborCap { limit: 2 }
        );
    }

    #[test]
    fn padding_budget_blocks_when_exhausted() {
        let budget = PaddingBudget::new(1024, 2048);
        let now = Instant::now();
        assert!(budget.try_acquire(1024, now));
        assert!(budget.try_acquire(1024, now));
        assert!(!budget.try_acquire(1024, now));
    }

    #[test]
    fn padding_budget_refills_after_interval() {
        let budget = PaddingBudget::new(1024, 2048);
        let now = Instant::now();
        assert!(budget.try_acquire(1024, now));
        let later = now + Duration::from_millis(1_000);
        assert!(budget.try_acquire(1024, later));
    }

    #[test]
    fn padding_budget_from_config_enforces_cell_floor() {
        let config = PaddingConfig {
            cell_size: 1500,
            max_idle_millis: 250,
            global_rate_limit_bytes_per_sec: 512,
            burst_bytes: 0,
        };
        let budget = PaddingBudget::from_config(&config).expect("budget enabled");
        let now = Instant::now();
        assert!(budget.try_acquire(1500, now));
    }

    #[test]
    fn registry_reports_constant_rate_neighbors() {
        let registry = CircuitRegistry::default();
        let mut negotiated = sample_negotiated();
        negotiated.constant_rate = Some(strict_constant_rate());

        let remote_a = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 4433);
        let remote_b = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 4433);
        let remote_c = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3)), 4433);

        registry
            .register(remote_a, &negotiated, Some(4))
            .expect("insert remote_a");
        registry
            .register(remote_b, &negotiated, Some(4))
            .expect("insert remote_b");

        // Non constant-rate circuits should be ignored.
        registry
            .register(remote_c, &sample_negotiated(), None)
            .expect("insert remote_c");

        let mut neighbors = registry.constant_rate_neighbors();
        neighbors.sort();

        assert_eq!(neighbors, vec![remote_a, remote_b]);
    }
}
