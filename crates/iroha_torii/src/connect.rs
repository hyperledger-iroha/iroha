#![allow(clippy::redundant_pub_crate)]
//! Iroha Connect WS handlers and minimal in-node relay (Bus).
//!
//! This module provides a feature-gated WS endpoint for a WalletConnect-like
//! flow and a relay bus that bridges app↔wallet connections locally and, when
//! enabled, propagates frames over the Iroha P2P network between nodes.
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use base64::Engine;
use core::future::Future;
use futures::{SinkExt, StreamExt};
use iroha_core as corelib;
use iroha_crypto::{Algorithm, MerkleTree, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::proofs::{BlockProofs, BlockReceiptProof},
    prelude::HashOf,
    transaction::TransactionEntrypoint,
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_logger::prelude::*;
use iroha_torii_shared::{connect as proto, connect_sdk};
use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
// no direct HTTP responses here
use crate::json_macros::JsonSerialize;
/// Length in bytes of a Connect session identifier.
pub const SID_LEN: usize = 32;
/// Connect session identifier stored as raw bytes.
pub type Sid = [u8; SID_LEN];
// Stable WS close codes for Connect
pub(crate) const CLOSE_CODE_TTL: u16 = 4001; // application-defined; stable for TTL expiry
pub(crate) const CLOSE_REASON_TTL: &str = "connect_ttl_expired";
pub(crate) const CLOSE_CODE_PURGED: u16 = 4002;
pub(crate) const CLOSE_REASON_PURGED: &str = "connect_session_purged";
pub(crate) const CLOSE_CODE_HEARTBEAT: u16 = 4003;
pub(crate) const CLOSE_REASON_HEARTBEAT: &str = "connect_heartbeat_timeout";
pub(crate) const CLOSE_REASON_ROLE_DIRECTION_MISMATCH: &str = "connect_role_direction_mismatch";
pub(crate) const CLOSE_REASON_SEQUENCE_VIOLATION: &str = "connect_sequence_violation";
pub(crate) const CLOSE_REASON_NETWORK_MISMATCH: &str = "connect_network_id_mismatch";
pub(crate) const CLOSE_REASON_OPEN_IDENTITY_MISMATCH: &str = "connect_open_identity_mismatch";
pub(crate) const CLOSE_REASON_OPEN_REPLAY: &str = "connect_open_replayed";
pub(crate) const CLOSE_REASON_APPROVAL_INVALID: &str = "connect_wallet_approval_invalid";
pub(crate) const CLOSE_REASON_APPROVAL_REPLAY: &str = "connect_wallet_approval_replayed";
pub(crate) const CLOSE_REASON_BUFFER_OVERFLOW: &str = "connect_buffer_overflow";
pub(crate) const CLOSE_REASON_REJECTED: &str = "connect_rejected";
pub(crate) const CLOSE_REASON_CLOSED: &str = "connect_closed";
pub(crate) const CLOSE_REASON_DELIVERY_TIMEOUT: &str = "connect_delivery_timeout";
pub(crate) const CLOSE_REASON_TRANSPORT_CLOSED: &str = "connect_transport_closed";
fn spawn_background_task<F>(task: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(task);
    } else {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("connect background runtime");
            runtime.block_on(task);
        });
    }
}
fn token_kind_for_role(role: proto::Role) -> connect_sdk::TokenKind {
    match role {
        proto::Role::App => connect_sdk::TokenKind::App,
        proto::Role::Wallet => connect_sdk::TokenKind::Wallet,
    }
}
fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn expires_at_ms(ttl: Duration) -> u64 {
    let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
    unix_time_ms().saturating_add(ttl_ms)
}
#[derive(Clone)]
pub struct Bus {
    network_id: NetworkId,
    inner: Arc<RwLock<HashMap<Vec<u8>, Arc<Session>>>>,
    p2p: Arc<RwLock<Option<corelib::IrohaNetwork>>>,
    seen: Arc<Mutex<SeenCache>>,
    policy: Policy,
    shared: Arc<BusShared>,
    per_ip_counts: Arc<Mutex<HashMap<IpAddr, usize>>>,
    handshake_buckets: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
}
/// Reservation for a Connect WebSocket slot, released on failure/close.
pub(crate) struct WsPermit {
    bus: Bus,
    ip: IpAddr,
    released: bool,
}

/// Reservation for a one-time Connect role token.
///
/// Dropping an uncommitted reservation makes the token available again. The
/// token itself is consumed only when the WebSocket endpoint is attached.
pub(crate) struct RoleTokenReservation {
    bus: Bus,
    session: Arc<Session>,
    sid: Sid,
    role: proto::Role,
    finished: bool,
}

struct EndpointLease {
    bus: Bus,
    session: Arc<Session>,
    sid: Sid,
    role: proto::Role,
    sender: mpsc::Sender<proto::ConnectFrameV1>,
    released: bool,
}

impl EndpointLease {
    fn release_in_background(&mut self) -> Option<oneshot::Receiver<()>> {
        if self.released {
            return None;
        }
        self.released = true;
        let bus = self.bus.clone();
        let session = self.session.clone();
        let sid = self.sid;
        let role = self.role;
        let sender = self.sender.clone();
        let (finished_tx, finished_rx) = oneshot::channel();
        spawn_background_task(async move {
            bus.release_endpoint(session, sid, role, &sender).await;
            let _ = finished_tx.send(());
        });
        Some(finished_rx)
    }

    async fn release(&mut self) {
        if let Some(finished) = self.release_in_background() {
            let _ = finished.await;
        }
    }
}

impl Drop for EndpointLease {
    fn drop(&mut self) {
        let _ = self.release_in_background();
    }
}

impl RoleTokenReservation {
    fn identity(&self) -> (Sid, proto::Role) {
        (self.sid, self.role)
    }

    async fn commit_and_attach(&mut self) -> Result<(ConnectInbox, EndpointLease), String> {
        let sessions = self.bus.inner.read().await;
        let Some(current) = sessions.get(&self.sid.to_vec()) else {
            return Err("connect session ended before websocket attachment".to_owned());
        };
        if !Arc::ptr_eq(current, &self.session) {
            return Err("connect session changed before websocket attachment".to_owned());
        }

        // Acquire every fallible async guard before changing token, endpoint,
        // or buffer state. Cancellation before this point simply drops this
        // reservation and leaves the one-time token available for retry.
        let mut buffer = self.session.buffer.lock().await;
        let mut buffer_bytes = self.session.buffer_bytes.lock().await;
        let mut endpoint = match self.role {
            proto::Role::App => self.session.app_tx.lock().await,
            proto::Role::Wallet => self.session.wallet_tx.lock().await,
        };
        let mut token_hash = match self.role {
            proto::Role::App => self.session.app_token_hash.lock().await,
            proto::Role::Wallet => self.session.wallet_token_hash.lock().await,
        };
        let mut last_activity = self.session.last_activity.lock().await;
        if self.session.peer_claim_expired_at(unix_time_ms()) {
            return Err(
                "connect peer session claim expired before websocket attachment".to_owned(),
            );
        }
        if endpoint.is_some() {
            return Err("connect role is already attached".to_owned());
        }
        if token_hash.is_none() {
            return Err("connect role token was consumed before attachment".to_owned());
        }

        let (tx, rx) = mpsc::channel::<proto::ConnectFrameV1>(64);
        let mut buffered = VecDeque::new();
        let mut kept = VecDeque::new();
        while let Some((frame, len)) = buffer.pop_front() {
            let target = match frame.dir {
                proto::Dir::AppToWallet => proto::Role::Wallet,
                proto::Dir::WalletToApp => proto::Role::App,
            };
            if target == self.role {
                buffered.push_back(frame);
                *buffer_bytes = buffer_bytes.saturating_sub(len);
            } else {
                kept.push_back((frame, len));
            }
        }
        *buffer = kept;
        *endpoint = Some(tx.clone());
        *token_hash = None;
        *last_activity = Instant::now();
        self.finished = true;
        self.reservation_flag().store(false, Ordering::Release);
        drop(last_activity);
        drop(token_hash);
        drop(endpoint);
        drop(buffer_bytes);
        drop(buffer);
        drop(sessions);

        let bus = self.bus.clone();
        let sid = self.sid;
        let role = self.role;
        // Once local attachment and consumption commit, cancellation of the
        // upgrade callback must not suppress cross-peer consumption gossip.
        spawn_background_task(async move {
            bus.broadcast_p2p_message(proto::ConnectP2pMessageV1::RoleConsumed(
                proto::ConnectSessionRoleConsumedV1 { sid, role },
            ))
            .await;
        });
        Ok((
            ConnectInbox { buffered, live: rx },
            EndpointLease {
                bus: self.bus.clone(),
                session: self.session.clone(),
                sid: self.sid,
                role: self.role,
                sender: tx,
                released: false,
            },
        ))
    }

    fn reservation_flag(&self) -> &AtomicBool {
        match self.role {
            proto::Role::App => &self.session.app_token_reserved,
            proto::Role::Wallet => &self.session.wallet_token_reserved,
        }
    }
}

impl Drop for RoleTokenReservation {
    fn drop(&mut self) {
        if !self.finished {
            self.reservation_flag().store(false, Ordering::Release);
        }
    }
}
impl WsPermit {
    fn release_in_background(&mut self) -> Option<oneshot::Receiver<()>> {
        if self.released {
            return None;
        }
        self.released = true;
        let bus = self.bus.clone();
        let ip = self.ip;
        let (finished_tx, finished_rx) = oneshot::channel();
        spawn_background_task(async move {
            bus.session_closed(ip).await;
            let _ = finished_tx.send(());
        });
        Some(finished_rx)
    }

    pub(crate) async fn release(&mut self) {
        if let Some(finished) = self.release_in_background() {
            // The cleanup task owns the actual release. If this future is
            // cancelled while waiting, the task still completes and cannot
            // leak the global or per-IP reservation.
            let _ = finished.await;
        }
    }
}
impl Drop for WsPermit {
    fn drop(&mut self) {
        // WebSocket upgrade callbacks are cancellation points. Release the
        // reservation even when the callback never starts or its task is
        // aborted before it can run the explicit async cleanup.
        let _ = self.release_in_background();
    }
}
#[derive(Default)]
#[allow(clippy::struct_field_names)]
struct BusShared {
    sessions_total: AtomicUsize,
    frames_in_total: AtomicU64,
    frames_out_total: AtomicU64,
    ciphertext_total: AtomicU64,
    dedupe_drops_total: AtomicU64,
    buffer_drops_total: AtomicU64,
    plaintext_control_drops_total: AtomicU64,
    monotonic_drops_total: AtomicU64,
    sequence_violation_closes_total: AtomicU64,
    role_direction_mismatch_total: AtomicU64,
    ping_miss_total: AtomicU64,
    p2p_rebroadcasts_total: AtomicU64,
    p2p_rebroadcast_skipped_total: AtomicU64,
    p2p_auth_failures_total: AtomicU64,
    p2p_ttl_drops_total: AtomicU64,
    p2p_unknown_session_drops_total: AtomicU64,
    p2p_session_claims_in_total: AtomicU64,
    p2p_session_claims_installed_total: AtomicU64,
    p2p_session_claim_conflicts_total: AtomicU64,
    p2p_role_consumed_total: AtomicU64,
    p2p_session_terminated_total: AtomicU64,
}
#[derive(Clone, Copy, Debug)]
struct HeartbeatEntry {
    nonce: u64,
    sent_at: Instant,
}
#[derive(Default, Debug)]
struct HeartbeatQueue {
    pending: VecDeque<HeartbeatEntry>,
    last_ping: Option<Instant>,
    last_pong: Option<Instant>,
}
impl HeartbeatQueue {
    fn record_ping(&mut self, nonce: u64, now: Instant, tolerance: u32) {
        let pending_base = usize::try_from(tolerance.max(1)).unwrap_or(usize::MAX);
        let max_pending = pending_base.saturating_mul(4).max(8);
        while self.pending.len() >= max_pending {
            self.pending.pop_front();
        }
        self.pending.push_back(HeartbeatEntry {
            nonce,
            sent_at: now,
        });
        self.last_ping = Some(now);
    }
    fn record_pong(&mut self, nonce: u64, now: Instant) -> bool {
        if let Some(pos) = self.pending.iter().position(|entry| entry.nonce == nonce) {
            self.pending.remove(pos);
            self.last_pong = Some(now);
            true
        } else {
            false
        }
    }
}
struct Session {
    // Channels to deliver frames to local endpoints.
    app_tx: Mutex<Option<mpsc::Sender<proto::ConnectFrameV1>>>,
    wallet_tx: Mutex<Option<mpsc::Sender<proto::ConnectFrameV1>>>,
    // Last observed activity timestamp (send/receive/buffer)
    last_activity: Mutex<Instant>,
    // Buffered frames when target is offline; tracked with byte budget
    buffer: Mutex<VecDeque<(proto::ConnectFrameV1, usize)>>,
    buffer_bytes: Mutex<usize>,
    // One-time token hashes per role; consumed on first successful WS attach.
    app_token_hash: Mutex<Option<[u8; 32]>>,
    wallet_token_hash: Mutex<Option<[u8; 32]>>,
    app_token_reserved: AtomicBool,
    wallet_token_reserved: AtomicBool,
    // Stable bearer token hash for session management operations.
    management_token_hash: Mutex<Option<[u8; 32]>>,
    // Relay MAC key derived from the session relay token.
    relay_key: Mutex<Option<[u8; 32]>>,
    // Canonical approval-preimage relay binding derived from the raw relay token.
    relay_auth_hash: Mutex<Option<[u8; 32]>>,
    // Application public key committed by the canonical registration SID.
    expected_app_pk: Mutex<Option<[u8; 32]>>,
    // Whether this session was created locally or learned from P2P.
    origin: SessionOrigin,
    // Absolute validity ceiling advertised by the peer session claim. Local
    // sessions continue to use the configured idle TTL; a peer shadow must
    // never outlive the claim which authorized its installation.
    peer_claim_expires_at_ms: Option<u64>,
    // Requested/accepted permissions for diagnostics
    req_perms: Mutex<Option<proto::PermissionsV1>>,
    acc_perms: Mutex<Option<proto::PermissionsV1>>,
    // Immutable application identity and constraints from the one-shot Open.
    open_binding: Mutex<Option<OpenBinding>>,
    // Handshake approval observed
    approved: Mutex<bool>,
    // Last seen peer sequence per direction
    last_seq_app_to_wallet: Mutex<Option<u64>>,
    last_seq_wallet_to_app: Mutex<Option<u64>>,
    // Server-generated sequence per direction (control events, close)
    server_seq_app_to_wallet: Mutex<u64>,
    server_seq_wallet_to_app: Mutex<u64>,
    // Outstanding ping expectations per role
    heartbeat_app: Mutex<HeartbeatQueue>,
    heartbeat_wallet: Mutex<HeartbeatQueue>,
}
struct ConnectInbox {
    buffered: VecDeque<proto::ConnectFrameV1>,
    live: mpsc::Receiver<proto::ConnectFrameV1>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalDelivery {
    Delivered,
    Offline,
    StaleSession,
}
impl ConnectInbox {
    async fn recv(&mut self) -> Option<proto::ConnectFrameV1> {
        if let Some(frame) = self.buffered.pop_front() {
            return Some(frame);
        }
        self.live.recv().await
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionOrigin {
    Local,
    PeerClaimed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct OpenBinding {
    app_pk: [u8; 32],
    constraints: proto::Constraints,
}
impl SessionOrigin {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::PeerClaimed => "peer_claimed",
        }
    }
}
/// Errors raised when registering a session in the Connect bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterSessionError {
    /// Session already exists for the given SID.
    Exists,
    /// Session capacity reached.
    Capacity,
    /// SID does not commit the supplied application key, nonce, and network.
    InvalidIdentity,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            app_tx: Mutex::new(None),
            wallet_tx: Mutex::new(None),
            last_activity: Mutex::new(Instant::now()),
            buffer: Mutex::new(VecDeque::new()),
            buffer_bytes: Mutex::new(0),
            app_token_hash: Mutex::new(None),
            wallet_token_hash: Mutex::new(None),
            app_token_reserved: AtomicBool::new(false),
            wallet_token_reserved: AtomicBool::new(false),
            management_token_hash: Mutex::new(None),
            relay_key: Mutex::new(None),
            relay_auth_hash: Mutex::new(None),
            expected_app_pk: Mutex::new(None),
            origin: SessionOrigin::Local,
            peer_claim_expires_at_ms: None,
            req_perms: Mutex::new(None),
            acc_perms: Mutex::new(None),
            open_binding: Mutex::new(None),
            approved: Mutex::new(false),
            last_seq_app_to_wallet: Mutex::new(None),
            last_seq_wallet_to_app: Mutex::new(None),
            server_seq_app_to_wallet: Mutex::new(0),
            server_seq_wallet_to_app: Mutex::new(0),
            heartbeat_app: Mutex::new(HeartbeatQueue::default()),
            heartbeat_wallet: Mutex::new(HeartbeatQueue::default()),
        }
    }
}
impl Session {
    fn new(origin: SessionOrigin, peer_claim_expires_at_ms: Option<u64>) -> Self {
        Self {
            origin,
            peer_claim_expires_at_ms,
            ..Self::default()
        }
    }

    fn peer_claim_expired_at(&self, now_ms: u64) -> bool {
        self.peer_claim_expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
    }
}
#[derive(Clone, Copy, Debug)]
struct HeartbeatFailure {
    misses: usize,
    oldest_elapsed: Duration,
}
impl Session {
    async fn next_server_seq(&self, dir: proto::Dir) -> u64 {
        match dir {
            proto::Dir::AppToWallet => {
                let mut last = self.server_seq_app_to_wallet.lock().await;
                *last = last.saturating_add(1);
                *last
            }
            proto::Dir::WalletToApp => {
                let mut last = self.server_seq_wallet_to_app.lock().await;
                *last = last.saturating_add(1);
                *last
            }
        }
    }
    async fn record_ping(&self, target: proto::Role, nonce: u64, now: Instant, tolerance: u32) {
        match target {
            proto::Role::App => self
                .heartbeat_app
                .lock()
                .await
                .record_ping(nonce, now, tolerance),
            proto::Role::Wallet => self
                .heartbeat_wallet
                .lock()
                .await
                .record_ping(nonce, now, tolerance),
        }
    }
    async fn record_pong(&self, responder: proto::Role, nonce: u64, now: Instant) -> bool {
        match responder {
            proto::Role::App => self.heartbeat_app.lock().await.record_pong(nonce, now),
            proto::Role::Wallet => self.heartbeat_wallet.lock().await.record_pong(nonce, now),
        }
    }
    async fn heartbeat_queue(
        &self,
        role: proto::Role,
    ) -> tokio::sync::MutexGuard<'_, HeartbeatQueue> {
        match role {
            proto::Role::App => self.heartbeat_app.lock().await,
            proto::Role::Wallet => self.heartbeat_wallet.lock().await,
        }
    }
}
impl Bus {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            network_id: test_network_id(),
            inner: Arc::new(RwLock::new(HashMap::new())),
            p2p: Arc::new(RwLock::new(None)),
            seen: Arc::new(Mutex::new(SeenCache::new(8192, Duration::from_mins(2)))),
            policy: Policy::default(),
            shared: Arc::new(BusShared::default()),
            per_ip_counts: Arc::new(Mutex::new(HashMap::new())),
            handshake_buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    /// Build an inert Connect bus from validated runtime configuration.
    ///
    /// Background services are started explicitly by Torii after the HTTP
    /// listener and router have both been prepared successfully.
    pub fn from_config(
        cfg: &iroha_config::parameters::actual::Connect,
        network_id: NetworkId,
    ) -> Self {
        Self {
            network_id,
            inner: Arc::new(RwLock::new(HashMap::new())),
            p2p: Arc::new(RwLock::new(None)),
            seen: Arc::new(Mutex::new(SeenCache::new(cfg.dedupe_cap, cfg.dedupe_ttl))),
            policy: Policy {
                frame_max_bytes: cfg.frame_max_bytes,
                relay_enabled: cfg.relay_enabled,
                relay_strategy: RelayStrategy::from_config(cfg.relay_strategy),
                ws_max_sessions: cfg.ws_max_sessions,
                ws_per_ip_max_sessions: cfg.ws_per_ip_max_sessions,
                ws_rate_per_ip_per_min: cfg.ws_rate_per_ip_per_min,
                session_ttl: cfg.session_ttl,
                session_buffer_max_bytes: cfg.session_buffer_max_bytes,
                heartbeat_interval: {
                    let interval = cfg.ping_interval.max(cfg.ping_min_interval);
                    if interval.is_zero() {
                        Duration::from_secs(1)
                    } else {
                        interval
                    }
                },
                heartbeat_miss_tolerance: cfg.ping_miss_tolerance.max(1),
                heartbeat_min_interval: cfg.ping_min_interval,
                p2p_ttl_hops: cfg.p2p_ttl_hops,
            },
            shared: Arc::new(BusShared::default()),
            per_ip_counts: Arc::new(Mutex::new(HashMap::new())),
            handshake_buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub(crate) fn start_cleaner(
        &self,
        shutdown_signal: ShutdownSignal,
    ) -> tokio::task::JoinHandle<crate::ToriiCriticalWorkerExit> {
        let me = self.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(30);
            loop {
                tokio::select! {
                    biased;
                    () = shutdown_signal.receive() => {
                        return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                    }
                    () = async {
                        tokio::time::sleep(interval).await;
                        let now = Instant::now();
                        let _ = me.prune_expired_sessions(now).await;
                        let _ = me.prune_handshake_buckets(now).await;
                    } => {}
                }
            }
        })
    }
    fn handshake_bucket_ttl(&self) -> Duration {
        // Cap idle retention so per-IP buckets don't grow without bound.
        let min_ttl = Duration::from_secs(60);
        let max_ttl = Duration::from_secs(600);
        self.policy.session_ttl.max(min_ttl).min(max_ttl)
    }
    async fn prune_handshake_buckets(&self, now: Instant) -> usize {
        let ttl = self.handshake_bucket_ttl();
        let mut buckets = self.handshake_buckets.lock().await;
        let before = buckets.len();
        buckets.retain(|_, bucket| now.saturating_duration_since(bucket.last_refill) <= ttl);
        before.saturating_sub(buckets.len())
    }
    #[cfg(test)]
    async fn session_expired(&self, sid: &Sid, now: Instant) -> bool {
        self.session_expired_at(sid, None, now, unix_time_ms())
            .await
    }
    async fn session_expired_for(&self, sid: &Sid, expected: &Arc<Session>, now: Instant) -> bool {
        self.session_expired_at(sid, Some(expected), now, unix_time_ms())
            .await
    }
    async fn session_expired_at(
        &self,
        sid: &Sid,
        expected: Option<&Arc<Session>>,
        now: Instant,
        now_ms: u64,
    ) -> bool {
        let map_read = self.inner.read().await;
        let Some(sess) = map_read.get(&sid.to_vec()) else {
            return true;
        };
        if expected.is_some_and(|expected| !Arc::ptr_eq(sess, expected)) {
            return true;
        }
        if sess.peer_claim_expired_at(now_ms) {
            return true;
        }
        let last = *sess.last_activity.lock().await;
        now.saturating_duration_since(last) > self.policy.session_ttl
    }
    async fn prune_expired_sessions(&self, now: Instant) -> usize {
        self.prune_expired_sessions_at(now, unix_time_ms()).await
    }
    async fn prune_expired_sessions_at(&self, now: Instant, now_ms: u64) -> usize {
        let ttl = self.policy.session_ttl;
        let mut candidates = Vec::new();
        {
            let map_read = self.inner.read().await;
            for (k, sess) in map_read.iter() {
                let ts = *sess.last_activity.lock().await;
                let claim_expired = sess.peer_claim_expired_at(now_ms);
                if claim_expired || now.saturating_duration_since(ts) > ttl {
                    let app_active = sess.app_tx.lock().await.is_some();
                    let wallet_active = sess.wallet_tx.lock().await.is_some();
                    if claim_expired || (!app_active && !wallet_active) {
                        candidates.push((k.clone(), sess.clone()));
                    }
                }
            }
        }
        self.remove_expired_candidates_at(now, now_ms, ttl, candidates)
            .await
    }
    #[cfg(test)]
    async fn remove_expired_candidates(
        &self,
        now: Instant,
        ttl: Duration,
        candidates: Vec<(Vec<u8>, Arc<Session>)>,
    ) -> usize {
        self.remove_expired_candidates_at(now, unix_time_ms(), ttl, candidates)
            .await
    }
    async fn remove_expired_candidates_at(
        &self,
        now: Instant,
        now_ms: u64,
        ttl: Duration,
        candidates: Vec<(Vec<u8>, Arc<Session>)>,
    ) -> usize {
        let mut removed = 0usize;
        let mut map_write = self.inner.write().await;
        for (key, candidate) in candidates {
            let should_remove = if let Some(current) = map_write.get(&key) {
                if !Arc::ptr_eq(current, &candidate) {
                    false
                } else {
                    let last = *current.last_activity.lock().await;
                    let app_active = current.app_tx.lock().await.is_some();
                    let wallet_active = current.wallet_tx.lock().await.is_some();
                    current.peer_claim_expired_at(now_ms)
                        || (now.saturating_duration_since(last) > ttl
                            && !app_active
                            && !wallet_active)
                }
            } else {
                false
            };
            if should_remove {
                map_write.remove(&key);
                removed = removed.saturating_add(1);
            }
        }
        removed
    }
    /// Pre-handshake gate: enforce global/per-IP session caps and handshake rate.
    pub async fn pre_ws_handshake(
        &self,
        ip: IpAddr,
    ) -> Result<WsPermit, (axum::http::StatusCode, String)> {
        let mut permit = self.reserve_ws_slot(ip).await?;
        if let Err(err) = self.consume_handshake_token(ip).await {
            permit.release().await;
            return Err(err);
        }
        Ok(permit)
    }
    /// Pre-creation gate for REST session provisioning.
    pub async fn pre_session_create(
        &self,
        ip: IpAddr,
    ) -> Result<(), (axum::http::StatusCode, String)> {
        self.check_creation_cap().await?;
        self.consume_handshake_token(ip).await?;
        Ok(())
    }
    async fn reserve_ws_slot(
        &self,
        ip: IpAddr,
    ) -> Result<WsPermit, (axum::http::StatusCode, String)> {
        if self.policy.ws_max_sessions == 0 {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: global session cap".into(),
            ));
        }
        let mut counts = self.per_ip_counts.lock().await;
        if self.shared.sessions_total.load(Ordering::Acquire) >= self.policy.ws_max_sessions {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: global session cap".into(),
            ));
        }
        let entry = counts.entry(ip).or_insert(0);
        if self.policy.ws_per_ip_max_sessions > 0 && *entry >= self.policy.ws_per_ip_max_sessions {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: per-ip session cap".into(),
            ));
        }
        *entry = entry.saturating_add(1);
        self.shared.sessions_total.fetch_add(1, Ordering::Release);
        drop(counts);
        Ok(WsPermit {
            bus: self.clone(),
            ip,
            released: false,
        })
    }
    async fn check_creation_cap(&self) -> Result<(), (axum::http::StatusCode, String)> {
        // Use inner map length to count provisioned sessions; this is stricter than active WS.
        if self.inner.read().await.len() >= self.policy.ws_max_sessions {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: global session cap".into(),
            ));
        }
        Ok(())
    }
    async fn consume_handshake_token(
        &self,
        ip: IpAddr,
    ) -> Result<(), (axum::http::StatusCode, String)> {
        if self.policy.ws_rate_per_ip_per_min == 0 {
            return Ok(());
        }
        // Handshake rate per minute token bucket
        let mut buckets = self.handshake_buckets.lock().await;
        let rate_per_sec = f64::from(self.policy.ws_rate_per_ip_per_min) / 60.0;
        let burst = f64::from(self.policy.ws_rate_per_ip_per_min.max(1));
        let bucket = buckets
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(rate_per_sec, burst));
        if !bucket.allow(1.0) {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: per-ip handshake rate".into(),
            ));
        }
        Ok(())
    }
    /// Record a newly opened WS session.
    pub async fn session_opened(&self, ip: IpAddr) {
        let mut counts = self.per_ip_counts.lock().await;
        let count = counts.entry(ip).or_insert(0);
        *count = count.saturating_add(1);
        self.shared.sessions_total.fetch_add(1, Ordering::Release);
    }
    /// Record a closed WS session.
    pub async fn session_closed(&self, ip: IpAddr) {
        let mut counts = self.per_ip_counts.lock().await;
        if let Some(v) = counts.get_mut(&ip) {
            if *v > 0 {
                *v -= 1;
                let _ = self.shared.sessions_total.fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |total| total.checked_sub(1),
                );
            }
            if *v == 0 {
                counts.remove(&ip);
            }
        }
    }
    // Use derived Clone
    #[cfg(test)]
    async fn get_or_create(&self, sid: &Sid) -> Arc<Session> {
        let key = sid.to_vec();
        if let Some(sess) = self.inner.read().await.get(&key) {
            return sess.clone();
        }
        let mut w = self.inner.write().await;
        let entry = w.entry(key).or_insert_with(|| Arc::new(Session::default()));
        entry.clone()
    }
    /// Register one-time tokens for a new session.
    pub async fn register_tokens(
        &self,
        sid: Sid,
        app_pk: [u8; 32],
        nonce: [u8; 16],
        token_app: String,
        token_wallet: String,
        token_management: String,
        token_relay: String,
    ) -> Result<(), RegisterSessionError> {
        if connect_sdk::derive_session_id(&self.network_id, &app_pk, &nonce) != sid {
            return Err(RegisterSessionError::InvalidIdentity);
        }
        let app_hash = connect_sdk::token_auth_hash(connect_sdk::TokenKind::App, &sid, &token_app);
        let wallet_hash =
            connect_sdk::token_auth_hash(connect_sdk::TokenKind::Wallet, &sid, &token_wallet);
        let management_hash = connect_sdk::token_auth_hash(
            connect_sdk::TokenKind::Management,
            &sid,
            &token_management,
        );
        let relay_key = connect_sdk::derive_relay_mac_key(&sid, &token_relay);
        let relay_auth_hash = connect_sdk::relay_auth_hash(&sid, &token_relay);
        let claim = proto::ConnectSessionClaimV1 {
            sid,
            network_id: self.network_id,
            app_pk,
            nonce,
            token_app_hash: app_hash,
            token_wallet_hash: wallet_hash,
            token_management_hash: management_hash,
            relay_mac_key: relay_key,
            relay_auth_hash,
            expires_at_ms: expires_at_ms(self.policy.session_ttl),
        };
        let sess = Arc::new(Session::new(SessionOrigin::Local, None));
        *sess.app_token_hash.lock().await = Some(app_hash);
        *sess.wallet_token_hash.lock().await = Some(wallet_hash);
        *sess.management_token_hash.lock().await = Some(management_hash);
        *sess.relay_key.lock().await = Some(relay_key);
        *sess.relay_auth_hash.lock().await = Some(relay_auth_hash);
        *sess.expected_app_pk.lock().await = Some(app_pk);
        *sess.last_activity.lock().await = Instant::now();
        let mut map = self.inner.write().await;
        if map.contains_key(&sid.to_vec()) {
            return Err(RegisterSessionError::Exists);
        }
        if map.len() >= self.policy.ws_max_sessions {
            return Err(RegisterSessionError::Capacity);
        }
        map.insert(sid.to_vec(), sess);
        drop(map);
        self.broadcast_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        Ok(())
    }
    /// Reserve a one-time role token until its WebSocket endpoint is attached.
    pub(crate) async fn reserve_token(
        &self,
        sid: Sid,
        role: proto::Role,
        token: &str,
    ) -> Result<RoleTokenReservation, (axum::http::StatusCode, String)> {
        let Some(sess) = self.inner.read().await.get(&sid.to_vec()).cloned() else {
            iroha_logger::debug!(
                sid = %hex::encode(sid),
                "connect: reserve_token rejected unknown sid"
            );
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "connect: unknown sid".into(),
            ));
        };
        if sess.peer_claim_expired_at(unix_time_ms()) {
            iroha_logger::debug!(
                sid = %hex::encode(sid),
                "connect: reserve_token rejected expired peer session claim"
            );
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "connect: expired session".into(),
            ));
        }
        let token_hash = match role {
            proto::Role::App => sess.app_token_hash.lock().await,
            proto::Role::Wallet => sess.wallet_token_hash.lock().await,
        };
        let supplied = connect_sdk::token_auth_hash(token_kind_for_role(role), &sid, token);
        let valid = token_hash
            .as_ref()
            .is_some_and(|stored| connect_sdk::constant_time_eq(stored, &supplied));
        let reservation_flag = match role {
            proto::Role::App => &sess.app_token_reserved,
            proto::Role::Wallet => &sess.wallet_token_reserved,
        };
        let reserved = valid
            && reservation_flag
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
        drop(token_hash);
        if !reserved {
            iroha_logger::debug!(
                sid = %hex::encode(sid),
                role = ?role,
                "connect: reserve_token rejected unavailable token"
            );
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "connect: bad token".into(),
            ));
        }
        Ok(RoleTokenReservation {
            bus: self.clone(),
            session: sess,
            sid,
            role,
            finished: false,
        })
    }
    /// Validate a stable session management token.
    pub async fn authorize_management_token(&self, sid: Sid, token: &str) -> bool {
        let Some(sess) = self.inner.read().await.get(&sid.to_vec()).cloned() else {
            return false;
        };
        if sess.peer_claim_expired_at(unix_time_ms()) {
            return false;
        }
        let supplied =
            connect_sdk::token_auth_hash(connect_sdk::TokenKind::Management, &sid, token);
        sess.management_token_hash
            .lock()
            .await
            .as_ref()
            .is_some_and(|stored| connect_sdk::constant_time_eq(stored, &supplied))
    }
    /// Return token-gated status for a single Connect session.
    pub async fn session_status(&self, sid: Sid, token: &str) -> Option<ConnectSessionStatus> {
        let sess = self.inner.read().await.get(&sid.to_vec()).cloned()?;
        if sess.peer_claim_expired_at(unix_time_ms()) {
            return None;
        }
        let supplied =
            connect_sdk::token_auth_hash(connect_sdk::TokenKind::Management, &sid, token);
        let authorized = sess
            .management_token_hash
            .lock()
            .await
            .as_ref()
            .is_some_and(|stored| connect_sdk::constant_time_eq(stored, &supplied));
        if !authorized {
            return None;
        }
        Some(ConnectSessionStatus {
            sid: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sid),
            app_attached: sess.app_tx.lock().await.is_some(),
            wallet_attached: sess.wallet_tx.lock().await.is_some(),
            approved: *sess.approved.lock().await,
            buffered_frames: sess.buffer.lock().await.len(),
            buffered_bytes: *sess.buffer_bytes.lock().await,
            last_seq_app_to_wallet: *sess.last_seq_app_to_wallet.lock().await,
            last_seq_wallet_to_app: *sess.last_seq_wallet_to_app.lock().await,
            origin: sess.origin.as_str(),
        })
    }
    async fn detach_if_empty(&self, sid: &Sid) {
        let key = sid.to_vec();
        let mut w = self.inner.write().await;
        if let Some(sess) = w.get(&key) {
            let app_empty = sess.app_tx.lock().await.is_none();
            let wallet_empty = sess.wallet_tx.lock().await.is_none();
            let provisioned = sess.management_token_hash.lock().await.is_some();
            if app_empty && wallet_empty && !provisioned {
                w.remove(&key);
            }
        }
    }
    async fn release_endpoint(
        &self,
        session: Arc<Session>,
        sid: Sid,
        role: proto::Role,
        sender: &mpsc::Sender<proto::ConnectFrameV1>,
    ) {
        let removed = {
            let mut sessions = self.inner.write().await;
            let Some(current) = sessions.get(&sid.to_vec()) else {
                return;
            };
            if !Arc::ptr_eq(current, &session) {
                return;
            }
            let mut endpoint = match role {
                proto::Role::App => current.app_tx.lock().await,
                proto::Role::Wallet => current.wallet_tx.lock().await,
            };
            if !endpoint
                .as_ref()
                .is_some_and(|current| current.same_channel(sender))
            {
                return;
            }
            // Do not try to send the terminal control back through the endpoint
            // whose transport is already gone. The surviving role remains
            // published long enough for `finish_terminated_session` to notify it.
            *endpoint = None;
            drop(endpoint);
            sessions.remove(&sid.to_vec())
        };
        if let Some(removed) = removed {
            self.finish_terminated_session(removed, sid, CLOSE_REASON_TRANSPORT_CLOSED, true)
                .await;
        }
    }
    #[cfg(test)]
    async fn attach_session(sess: Arc<Session>, role: proto::Role) -> ConnectInbox {
        let (tx, rx) = mpsc::channel::<proto::ConnectFrameV1>(64);
        match role {
            proto::Role::App => {
                *sess.app_tx.lock().await = Some(tx);
            }
            proto::Role::Wallet => {
                *sess.wallet_tx.lock().await = Some(tx);
            }
        }
        // Move pre-attach frames into a receiver-owned prefix instead of
        // sending them into the bounded live channel before its receiver is
        // returned. The latter deadlocks as soon as more than 64 frames were
        // buffered. New frames enter `live` only after the prefix is fixed, so
        // `ConnectInbox::recv` preserves their order.
        let buffered = Self::take_buffered_for_role(sess, role).await;
        ConnectInbox { buffered, live: rx }
    }
    #[cfg(test)]
    async fn attach(&self, sid: Sid, role: proto::Role) -> ConnectInbox {
        let sess = self.get_or_create(&sid).await;
        Self::attach_session(sess, role).await
    }
    async fn detach(&self, sid: Sid, role: proto::Role) {
        if let Some(sess) = self.inner.read().await.get(&sid.to_vec()) {
            match role {
                proto::Role::App => {
                    *sess.app_tx.lock().await = None;
                }
                proto::Role::Wallet => {
                    *sess.wallet_tx.lock().await = None;
                }
            }
        }
        self.detach_if_empty(&sid).await;
    }
    pub async fn terminate_session(&self, sid: Sid, reason: &str) -> bool {
        self.terminate_session_inner(sid, reason, true).await
    }
    /// Terminate a session only when the supplied management token belongs to
    /// the same session incarnation removed by this operation.
    pub(crate) async fn terminate_session_authorized(
        &self,
        sid: Sid,
        token: &str,
        reason: &str,
    ) -> bool {
        let supplied =
            connect_sdk::token_auth_hash(connect_sdk::TokenKind::Management, &sid, token);
        let session = {
            let mut sessions = self.inner.write().await;
            let Some(session) = sessions.get(&sid.to_vec()).cloned() else {
                return false;
            };
            let authorized = session
                .management_token_hash
                .lock()
                .await
                .as_ref()
                .is_some_and(|stored| connect_sdk::constant_time_eq(stored, &supplied));
            if !authorized {
                return false;
            }
            sessions.remove(&sid.to_vec())
        };
        let Some(session) = session else {
            return false;
        };
        self.finish_terminated_session(session, sid, reason, true)
            .await;
        true
    }
    async fn terminate_session_inner(&self, sid: Sid, reason: &str, broadcast: bool) -> bool {
        let sess = {
            let mut map = self.inner.write().await;
            map.remove(&sid.to_vec())
        };
        if let Some(sess) = sess {
            self.finish_terminated_session(sess, sid, reason, broadcast)
                .await;
            true
        } else {
            false
        }
    }
    async fn terminate_session_if_current(
        &self,
        sid: Sid,
        expected: &Arc<Session>,
        reason: &str,
        broadcast: bool,
    ) -> bool {
        let sess = {
            let mut map = self.inner.write().await;
            let Some(current) = map.get(&sid.to_vec()) else {
                return false;
            };
            if !Arc::ptr_eq(current, expected) {
                return false;
            }
            map.remove(&sid.to_vec())
        };
        let Some(sess) = sess else {
            return false;
        };
        self.finish_terminated_session(sess, sid, reason, broadcast)
            .await;
        true
    }
    async fn finish_terminated_session(
        &self,
        sess: Arc<Session>,
        sid: Sid,
        reason: &str,
        broadcast: bool,
    ) {
        // Clear tokens so new WS joins cannot reuse them.
        *sess.app_token_hash.lock().await = None;
        *sess.wallet_token_hash.lock().await = None;
        sess.app_token_reserved.store(false, Ordering::Release);
        sess.wallet_token_reserved.store(false, Ordering::Release);
        *sess.management_token_hash.lock().await = None;
        *sess.relay_key.lock().await = None;
        *sess.relay_auth_hash.lock().await = None;
        *sess.expected_app_pk.lock().await = None;
        *sess.open_binding.lock().await = None;
        self.notify_close(sess.clone(), sid, proto::Role::Wallet, reason)
            .await;
        self.notify_close(sess, sid, proto::Role::App, reason).await;
        if broadcast {
            self.broadcast_p2p_message(proto::ConnectP2pMessageV1::SessionTerminated(
                proto::ConnectSessionTerminatedV1 {
                    sid,
                    reason: reason.to_owned(),
                },
            ))
            .await;
        }
    }
    async fn notify_close(&self, sess: Arc<Session>, sid: Sid, target: proto::Role, reason: &str) {
        let (dir, initiator) = match target {
            proto::Role::App => (proto::Dir::WalletToApp, proto::Role::Wallet),
            proto::Role::Wallet => (proto::Dir::AppToWallet, proto::Role::App),
        };
        let seq = sess.next_server_seq(dir).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir,
            seq,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Close {
                who: initiator,
                code: CLOSE_CODE_PURGED,
                reason: reason.to_owned(),
                retryable: false,
            }),
        };
        let tx_opt = match target {
            proto::Role::App => sess.app_tx.lock().await.clone(),
            proto::Role::Wallet => sess.wallet_tx.lock().await.clone(),
        };
        if let Some(tx) = tx_opt {
            if matches!(
                tokio::time::timeout(self.local_delivery_timeout(), tx.send(frame)).await,
                Ok(Ok(()))
            ) {
                *sess.last_activity.lock().await = Instant::now();
            }
        }
    }
    fn local_delivery_timeout(&self) -> Duration {
        self.policy
            .heartbeat_interval
            .min(Duration::from_secs(1))
            .max(Duration::from_millis(10))
    }
    async fn deliver_local_only(
        &self,
        expected: &Arc<Session>,
        frame: &proto::ConnectFrameV1,
    ) -> Result<LocalDelivery, ()> {
        let sid_key = frame.sid.to_vec();
        let target = match frame.dir {
            proto::Dir::AppToWallet => proto::Role::Wallet,
            proto::Dir::WalletToApp => proto::Role::App,
        };
        // Resolve only the expected incarnation. The sender below is taken
        // directly from that session, never from a second SID lookup, so a
        // concurrent replacement cannot receive the old frame. Release the
        // global map guard before the bounded send so an unrelated session
        // update is not serialized behind a full endpoint inbox.
        let sessions = self.inner.read().await;
        let Some(current) = sessions.get(&sid_key) else {
            return Ok(LocalDelivery::StaleSession);
        };
        if !Arc::ptr_eq(current, expected) {
            return Ok(LocalDelivery::StaleSession);
        }
        drop(sessions);
        let tx_opt = match target {
            proto::Role::App => expected.app_tx.lock().await.clone(),
            proto::Role::Wallet => expected.wallet_tx.lock().await.clone(),
        };
        if let Some(tx) = tx_opt {
            return match tokio::time::timeout(self.local_delivery_timeout(), tx.send(frame.clone()))
                .await
            {
                Ok(Ok(())) => Ok(LocalDelivery::Delivered),
                Ok(Err(_)) => Ok(LocalDelivery::Offline),
                Err(_) => Err(()),
            };
        }
        Ok(LocalDelivery::Offline)
    }
    #[cfg(test)]
    async fn relay(&self, frame: proto::ConnectFrameV1) {
        self.relay_with_p2p_ttl(frame, self.policy.p2p_ttl_hops)
            .await;
    }
    #[cfg(test)]
    async fn relay_with_p2p_ttl(&self, frame: proto::ConnectFrameV1, p2p_ttl: u8) {
        let Some(sess) = self.inner.read().await.get(&frame.sid.to_vec()).cloned() else {
            debug!(sid = ?hex::encode(frame.sid), "connect: dropping frame for unknown session");
            return;
        };
        self.relay_with_session(frame, p2p_ttl, sess).await;
    }
    async fn relay_with_session(
        &self,
        frame: proto::ConnectFrameV1,
        p2p_ttl: u8,
        sess: Arc<Session>,
    ) {
        if !self.session_is_current(&frame.sid, &sess).await {
            return;
        }
        if sess.peer_claim_expired_at(unix_time_ms()) {
            debug!(sid = ?hex::encode(frame.sid), "connect: dropping frame for expired peer session claim");
            return;
        }
        if matches!(
            &frame.kind,
            proto::FrameKind::Ciphertext(ciphertext) if ciphertext.dir != frame.dir
        ) {
            self.shared
                .role_direction_mismatch_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                sid = ?hex::encode(frame.sid),
                frame_dir = ?frame.dir,
                "connect: closing session on substituted ciphertext direction"
            );
            self.terminate_session_if_current(
                frame.sid,
                &sess,
                CLOSE_REASON_ROLE_DIRECTION_MISMATCH,
                true,
            )
            .await;
            return;
        }
        let Some(enc_len) = encoded_len(&frame) else {
            warn!(
                sid = ?hex::encode(frame.sid),
                "connect: failed to encode frame for size accounting, dropping"
            );
            return;
        };
        if enc_len > self.policy.frame_max_bytes {
            warn!(
                sid = ?hex::encode(frame.sid),
                len = enc_len,
                cap = self.policy.frame_max_bytes,
                "connect: dropping oversized frame"
            );
            return;
        }
        let mut terminal_reason = None;
        if let proto::FrameKind::Control(control) = &frame.kind {
            let sender = match frame.dir {
                proto::Dir::AppToWallet => proto::Role::App,
                proto::Dir::WalletToApp => proto::Role::Wallet,
            };
            let valid_owner = match control {
                proto::ConnectControlV1::Open { .. } => sender == proto::Role::App,
                proto::ConnectControlV1::Approve { .. }
                | proto::ConnectControlV1::Reject { .. } => sender == proto::Role::Wallet,
                proto::ConnectControlV1::Close { who, .. } => *who == sender,
                proto::ConnectControlV1::Ping { .. } | proto::ConnectControlV1::Pong { .. } => true,
                // Server events use an independent Torii-owned sequence and
                // are delivered only by `send_server_event`.
                proto::ConnectControlV1::ServerEvent { .. } => false,
            };
            if !valid_owner {
                self.shared
                    .role_direction_mismatch_total
                    .fetch_add(1, Ordering::Relaxed);
                warn!(
                    sid = ?hex::encode(frame.sid),
                    frame_dir = ?frame.dir,
                    "connect: closing session on control-frame owner substitution"
                );
                self.terminate_session_if_current(
                    frame.sid,
                    &sess,
                    CLOSE_REASON_ROLE_DIRECTION_MISMATCH,
                    true,
                )
                .await;
                return;
            }
        }
        self.shared.frames_in_total.fetch_add(1, Ordering::Relaxed);
        if matches!(frame.kind, proto::FrameKind::Ciphertext(_)) {
            self.shared.ciphertext_total.fetch_add(1, Ordering::Relaxed);
        }
        let key = SeenKey {
            sid: frame.sid,
            dir: frame.dir,
            seq: frame.seq,
        };
        let mut seen = self.seen.lock().await;
        if !seen.record_if_new(key) {
            self.shared
                .dedupe_drops_total
                .fetch_add(1, Ordering::Relaxed);
            return; // duplicate
        }
        drop(seen);
        if let proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce }) = &frame.kind {
            let target = match frame.dir {
                proto::Dir::AppToWallet => proto::Role::Wallet,
                proto::Dir::WalletToApp => proto::Role::App,
            };
            let now = Instant::now();
            sess.record_ping(target, *nonce, now, self.policy.heartbeat_miss_tolerance)
                .await;
        }
        if let proto::FrameKind::Control(proto::ConnectControlV1::Pong { nonce }) = &frame.kind {
            let responder = match frame.dir {
                proto::Dir::AppToWallet => proto::Role::App,
                proto::Dir::WalletToApp => proto::Role::Wallet,
            };
            let now = Instant::now();
            if !sess.record_pong(responder, *nonce, now).await {
                warn!(
                    sid = ?hex::encode(frame.sid),
                    nonce = *nonce,
                    ?responder,
                    "connect: received unmatched heartbeat pong"
                );
            }
        }
        // Enforce strict contiguous seq progression per direction.
        let (sequence_violation, expected_seq) = match frame.dir {
            proto::Dir::AppToWallet => {
                let mut last = sess.last_seq_app_to_wallet.lock().await;
                let expected = match *last {
                    Some(prev) => prev.checked_add(1),
                    None => Some(1),
                };
                let violation = expected != Some(frame.seq);
                if !violation {
                    *last = Some(frame.seq);
                }
                (violation, expected)
            }
            proto::Dir::WalletToApp => {
                let mut last = sess.last_seq_wallet_to_app.lock().await;
                let expected = match *last {
                    Some(prev) => prev.checked_add(1),
                    None => Some(1),
                };
                let violation = expected != Some(frame.seq);
                if !violation {
                    *last = Some(frame.seq);
                }
                (violation, expected)
            }
        };
        if sequence_violation {
            self.shared
                .monotonic_drops_total
                .fetch_add(1, Ordering::Relaxed);
            self.shared
                .sequence_violation_closes_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                sid = ?hex::encode(frame.sid),
                seq = frame.seq,
                expected_seq = ?expected_seq,
                "connect: closing session on non-contiguous seq frame"
            );
            self.terminate_session_if_current(
                frame.sid,
                &sess,
                CLOSE_REASON_SEQUENCE_VIOLATION,
                true,
            )
            .await;
            return;
        }
        if matches!(frame.kind, proto::FrameKind::Ciphertext(_)) && !*sess.approved.lock().await {
            warn!(
                sid = ?hex::encode(frame.sid),
                "connect: rejecting ciphertext before a verified wallet approval"
            );
            self.terminate_session_if_current(
                frame.sid,
                &sess,
                CLOSE_REASON_APPROVAL_INVALID,
                true,
            )
            .await;
            return;
        }
        // If control frame, capture permissions for diagnostics
        if let proto::FrameKind::Control(ctrl) = &frame.kind {
            match ctrl {
                proto::ConnectControlV1::Open {
                    app_pk,
                    constraints,
                    permissions,
                    ..
                } => {
                    if constraints.network_id != self.network_id {
                        warn!(
                            sid = ?hex::encode(frame.sid),
                            expected_network_id = %self.network_id,
                            supplied_network_id = %constraints.network_id,
                            "connect: rejecting Open for a different network"
                        );
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_NETWORK_MISMATCH,
                            true,
                        )
                        .await;
                        return;
                    }
                    if *sess.expected_app_pk.lock().await != Some(*app_pk) {
                        warn!(
                            sid = ?hex::encode(frame.sid),
                            "connect: rejecting Open whose application key is not bound to the session id"
                        );
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_OPEN_IDENTITY_MISMATCH,
                            true,
                        )
                        .await;
                        return;
                    }
                    let mut open_binding = sess.open_binding.lock().await;
                    if open_binding.is_some() {
                        drop(open_binding);
                        warn!(sid = ?hex::encode(frame.sid), "connect: rejecting repeated Open");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_OPEN_REPLAY,
                            true,
                        )
                        .await;
                        return;
                    }
                    *open_binding = Some(OpenBinding {
                        app_pk: *app_pk,
                        constraints: constraints.clone(),
                    });
                    drop(open_binding);
                    (*sess.req_perms.lock().await).clone_from(permissions);
                    if permissions.is_some() {
                        debug!(sid = ?hex::encode(frame.sid), "connect: Open with permissions requested");
                    }
                }
                proto::ConnectControlV1::Approve {
                    wallet_pk,
                    account_id,
                    permissions,
                    proof,
                    sig_wallet,
                } => {
                    let mut approved = sess.approved.lock().await;
                    if *approved {
                        drop(approved);
                        warn!(sid = ?hex::encode(frame.sid), "connect: rejecting repeated wallet approval");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_APPROVAL_REPLAY,
                            true,
                        )
                        .await;
                        return;
                    }
                    let Some(open_binding) = sess.open_binding.lock().await.clone() else {
                        drop(approved);
                        warn!(sid = ?hex::encode(frame.sid), "connect: rejecting approval before Open");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_APPROVAL_INVALID,
                            true,
                        )
                        .await;
                        return;
                    };
                    let Some(relay_auth_hash) = *sess.relay_auth_hash.lock().await else {
                        drop(approved);
                        warn!(sid = ?hex::encode(frame.sid), "connect: rejecting approval without relay binding");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_APPROVAL_INVALID,
                            true,
                        )
                        .await;
                        return;
                    };
                    let account = match AccountId::parse_encoded(&account_id) {
                        Ok(parsed) if parsed.to_string() == account_id.as_str() => parsed,
                        Ok(_) => {
                            drop(approved);
                            warn!(sid = ?hex::encode(frame.sid), "connect: rejecting approval with noncanonical account id");
                            self.terminate_session_if_current(
                                frame.sid,
                                &sess,
                                CLOSE_REASON_APPROVAL_INVALID,
                                true,
                            )
                            .await;
                            return;
                        }
                        Err(error) => {
                            drop(approved);
                            warn!(sid = ?hex::encode(frame.sid), ?error, "connect: rejecting approval with malformed account id");
                            self.terminate_session_if_current(
                                frame.sid,
                                &sess,
                                CLOSE_REASON_APPROVAL_INVALID,
                                true,
                            )
                            .await;
                            return;
                        }
                    };
                    let Some(signatory) = account.try_signatory() else {
                        drop(approved);
                        warn!(sid = ?hex::encode(frame.sid), "connect: rejecting approval for a multisig account without a typed intent");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_APPROVAL_INVALID,
                            true,
                        )
                        .await;
                        return;
                    };
                    if let Err(error) = connect_sdk::verify_wallet_approval_signature(
                        signatory,
                        &open_binding.constraints,
                        &frame.sid,
                        &open_binding.app_pk,
                        wallet_pk,
                        account_id,
                        permissions.as_ref(),
                        proof.as_ref(),
                        &relay_auth_hash,
                        sig_wallet,
                    ) {
                        drop(approved);
                        warn!(sid = ?hex::encode(frame.sid), error, "connect: rejecting forged or substituted wallet approval");
                        self.terminate_session_if_current(
                            frame.sid,
                            &sess,
                            CLOSE_REASON_APPROVAL_INVALID,
                            true,
                        )
                        .await;
                        return;
                    }
                    (*sess.acc_perms.lock().await).clone_from(permissions);
                    *approved = true;
                    drop(approved);
                    if permissions.is_some() {
                        debug!(sid = ?hex::encode(frame.sid), "connect: Approve with permissions provided");
                    }
                    // Compare if we have both sides
                    let req = sess.req_perms.lock().await.clone();
                    let acc = sess.acc_perms.lock().await.clone();
                    if let (Some(r), Some(a)) = (req, acc) {
                        let (extra_m, missing_m) = diff(&r.methods, &a.methods);
                        let (extra_e, missing_e) = diff(&r.events, &a.events);
                        if !extra_m.is_empty() || !extra_e.is_empty() {
                            warn!(sid = ?hex::encode(frame.sid), extra_methods = ?extra_m, extra_events = ?extra_e, "connect: wallet approved permissions not requested by app");
                        }
                        if !missing_m.is_empty() || !missing_e.is_empty() {
                            info!(sid = ?hex::encode(frame.sid), dropped_methods = ?missing_m, dropped_events = ?missing_e, "connect: wallet narrowed requested permissions");
                        }
                    }
                }
                proto::ConnectControlV1::Close { .. } | proto::ConnectControlV1::Reject { .. } => {
                    if *sess.approved.lock().await {
                        self.shared
                            .plaintext_control_drops_total
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(sid = ?hex::encode(frame.sid), "connect: dropping plaintext Close/Reject after approval");
                        return;
                    }
                    terminal_reason = Some(match ctrl {
                        proto::ConnectControlV1::Reject { .. } => CLOSE_REASON_REJECTED,
                        proto::ConnectControlV1::Close { .. } => CLOSE_REASON_CLOSED,
                        _ => unreachable!("matched terminal control"),
                    });
                }
                _ => {}
            }
        }
        // Deliver locally (best effort)
        let mut delivered = match self.deliver_local_only(&sess, &frame).await {
            Ok(LocalDelivery::Delivered) => true,
            Ok(LocalDelivery::Offline) => false,
            Ok(LocalDelivery::StaleSession) => return,
            Err(()) => {
                warn!(
                    sid = ?hex::encode(frame.sid),
                    "connect: terminating session after bounded local delivery timed out"
                );
                self.terminate_session_if_current(
                    frame.sid,
                    &sess,
                    CLOSE_REASON_DELIVERY_TIMEOUT,
                    true,
                )
                .await;
                return;
            }
        };
        let mut buffer_overflow = false;
        let mut delivery_timed_out = false;
        if !delivered {
            // Buffer frame for the session if target is offline
            let cap = self.policy.session_buffer_max_bytes;
            {
                // Keep this incarnation current through the buffer-or-deliver
                // transition. A concurrent delete can proceed afterwards, but
                // the frame can never leak into a replacement with the same SID.
                let sessions = self.inner.read().await;
                let Some(current) = sessions.get(&frame.sid.to_vec()) else {
                    return;
                };
                if !Arc::ptr_eq(current, &sess) {
                    return;
                }
                let mut buf = sess.buffer.lock().await;
                let mut bytes = sess.buffer_bytes.lock().await;
                // `attach` publishes its sender before taking this buffer lock.
                // Re-check under the lock so a frame which raced that publish
                // cannot be stranded in the offline buffer until a later reconnect.
                let tx_opt = match frame.dir {
                    proto::Dir::AppToWallet => sess.wallet_tx.lock().await.clone(),
                    proto::Dir::WalletToApp => sess.app_tx.lock().await.clone(),
                };
                if let Some(tx) = tx_opt {
                    match tokio::time::timeout(
                        self.local_delivery_timeout(),
                        tx.send(frame.clone()),
                    )
                    .await
                    {
                        Ok(result) => delivered = result.is_ok(),
                        Err(_) => delivery_timed_out = true,
                    }
                }
                if !delivered && !delivery_timed_out {
                    if bytes.saturating_add(enc_len) > cap {
                        self.shared
                            .buffer_drops_total
                            .fetch_add(1, Ordering::Relaxed);
                        buffer_overflow = true;
                    } else {
                        buf.push_back((frame.clone(), enc_len));
                        *bytes = bytes.saturating_add(enc_len);
                    }
                }
            }
            if buffer_overflow || delivery_timed_out {
                warn!(
                    sid = ?hex::encode(frame.sid),
                    cap,
                    "connect: terminating session instead of creating a delivery gap"
                );
                let reason = if delivery_timed_out {
                    CLOSE_REASON_DELIVERY_TIMEOUT
                } else {
                    CLOSE_REASON_BUFFER_OVERFLOW
                };
                self.terminate_session_if_current(frame.sid, &sess, reason, true)
                    .await;
                return;
            }
            *sess.last_activity.lock().await = Instant::now();
        }
        if delivered {
            self.shared.frames_out_total.fetch_add(1, Ordering::Relaxed);
        }
        // Re-broadcast authenticated envelopes to peers (best effort).
        let sessions = self.inner.read().await;
        let session_is_current = sessions
            .get(&frame.sid.to_vec())
            .is_some_and(|current| Arc::ptr_eq(current, &sess));
        if session_is_current
            && self.policy.relay_enabled
            && self.policy.relay_strategy == RelayStrategy::Broadcast
        {
            if p2p_ttl == 0 {
                self.shared
                    .p2p_ttl_drops_total
                    .fetch_add(1, Ordering::Relaxed);
            } else if let Some(net) = self.p2p.read().await.as_ref() {
                if let Some(relay_key) = *sess.relay_key.lock().await {
                    match connect_sdk::seal_relay_envelope(&relay_key, frame.clone(), p2p_ttl) {
                        Ok(envelope) => {
                            self.shared
                                .p2p_rebroadcasts_total
                                .fetch_add(1, Ordering::Relaxed);
                            net.broadcast(iroha_p2p::Broadcast {
                                data: corelib::NetworkMessage::Connect(Box::new(
                                    proto::ConnectP2pMessageV1::RelayEnvelope(envelope),
                                )),
                                priority: iroha_p2p::Priority::Low,
                            });
                        }
                        Err(err) => {
                            warn!(
                                sid = ?hex::encode(frame.sid),
                                ?err,
                                "connect: failed to authenticate P2P relay envelope"
                            );
                        }
                    }
                } else {
                    self.shared
                        .p2p_rebroadcast_skipped_total
                        .fetch_add(1, Ordering::Relaxed);
                }
            } else {
                self.shared
                    .p2p_rebroadcast_skipped_total
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        drop(sessions);
        if let Some(reason) = terminal_reason {
            self.terminate_session_if_current(frame.sid, &sess, reason, true)
                .await;
        }
    }
    async fn broadcast_p2p_message(&self, message: proto::ConnectP2pMessageV1) {
        if !self.policy.relay_enabled || self.policy.relay_strategy != RelayStrategy::Broadcast {
            return;
        }
        let Some(net) = self.p2p.read().await.as_ref().cloned() else {
            return;
        };
        net.broadcast(iroha_p2p::Broadcast {
            data: corelib::NetworkMessage::Connect(Box::new(message)),
            priority: iroha_p2p::Priority::Low,
        });
    }
    async fn handle_p2p_message(&self, message: proto::ConnectP2pMessageV1) {
        match message {
            proto::ConnectP2pMessageV1::RelayEnvelope(envelope) => {
                self.relay_from_p2p(envelope).await;
            }
            proto::ConnectP2pMessageV1::SessionClaim(claim) => {
                self.install_peer_session_claim(claim).await;
            }
            proto::ConnectP2pMessageV1::RoleConsumed(event) => {
                self.apply_role_consumed(event).await;
            }
            proto::ConnectP2pMessageV1::SessionTerminated(event) => {
                self.apply_session_terminated(event).await;
            }
        }
    }
    async fn claim_matches_session(
        session: &Session,
        claim: &proto::ConnectSessionClaimV1,
    ) -> bool {
        if session.origin == SessionOrigin::PeerClaimed
            && session.peer_claim_expires_at_ms != Some(claim.expires_at_ms)
        {
            return false;
        }
        let relay_key = *session.relay_key.lock().await;
        if relay_key != Some(claim.relay_mac_key) {
            return false;
        }
        let relay_auth_hash = *session.relay_auth_hash.lock().await;
        if relay_auth_hash != Some(claim.relay_auth_hash) {
            return false;
        }
        let expected_app_pk = *session.expected_app_pk.lock().await;
        if expected_app_pk != Some(claim.app_pk) {
            return false;
        }
        let management_hash = *session.management_token_hash.lock().await;
        if management_hash.is_some_and(|stored| stored != claim.token_management_hash) {
            return false;
        }
        let app_hash = *session.app_token_hash.lock().await;
        if app_hash.is_some_and(|stored| stored != claim.token_app_hash) {
            return false;
        }
        let wallet_hash = *session.wallet_token_hash.lock().await;
        !wallet_hash.is_some_and(|stored| stored != claim.token_wallet_hash)
    }
    async fn install_peer_session_claim(&self, claim: proto::ConnectSessionClaimV1) {
        self.shared
            .p2p_session_claims_in_total
            .fetch_add(1, Ordering::Relaxed);
        if claim.network_id != self.network_id {
            self.shared
                .p2p_session_claim_conflicts_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                sid = ?hex::encode(claim.sid),
                expected_network_id = %self.network_id,
                claimed_network_id = %claim.network_id,
                "connect: dropping cross-network P2P session claim"
            );
            return;
        }
        if connect_sdk::derive_session_id(&claim.network_id, &claim.app_pk, &claim.nonce)
            != claim.sid
        {
            self.shared
                .p2p_session_claim_conflicts_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                sid = ?hex::encode(claim.sid),
                "connect: dropping P2P session claim with an unbound application identity"
            );
            return;
        }
        if claim.expires_at_ms <= unix_time_ms() {
            debug!(
                sid = ?hex::encode(claim.sid),
                "connect: dropping expired P2P session claim"
            );
            return;
        }
        if let Some(existing) = self.inner.read().await.get(&claim.sid.to_vec()).cloned() {
            if !Self::claim_matches_session(&existing, &claim).await {
                self.shared
                    .p2p_session_claim_conflicts_total
                    .fetch_add(1, Ordering::Relaxed);
                warn!(
                    sid = ?hex::encode(claim.sid),
                    "connect: ignoring conflicting P2P session claim"
                );
            }
            return;
        }
        {
            let map = self.inner.read().await;
            if map.len() >= self.policy.ws_max_sessions {
                self.shared
                    .p2p_session_claim_conflicts_total
                    .fetch_add(1, Ordering::Relaxed);
                warn!(
                    sid = ?hex::encode(claim.sid),
                    "connect: dropping P2P session claim at capacity"
                );
                return;
            }
        }
        let session = Arc::new(Session::new(
            SessionOrigin::PeerClaimed,
            Some(claim.expires_at_ms),
        ));
        *session.app_token_hash.lock().await = Some(claim.token_app_hash);
        *session.wallet_token_hash.lock().await = Some(claim.token_wallet_hash);
        *session.management_token_hash.lock().await = Some(claim.token_management_hash);
        *session.relay_key.lock().await = Some(claim.relay_mac_key);
        *session.relay_auth_hash.lock().await = Some(claim.relay_auth_hash);
        *session.expected_app_pk.lock().await = Some(claim.app_pk);
        *session.last_activity.lock().await = Instant::now();
        let mut map = self.inner.write().await;
        match map.get(&claim.sid.to_vec()).cloned() {
            Some(existing) => {
                drop(map);
                if !Self::claim_matches_session(&existing, &claim).await {
                    self.shared
                        .p2p_session_claim_conflicts_total
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(
                        sid = ?hex::encode(claim.sid),
                        "connect: ignoring racing conflicting P2P session claim"
                    );
                }
            }
            None => {
                if map.len() >= self.policy.ws_max_sessions {
                    self.shared
                        .p2p_session_claim_conflicts_total
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(
                        sid = ?hex::encode(claim.sid),
                        "connect: dropping racing P2P session claim at capacity"
                    );
                    return;
                }
                map.insert(claim.sid.to_vec(), session);
                self.shared
                    .p2p_session_claims_installed_total
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    async fn apply_role_consumed(&self, event: proto::ConnectSessionRoleConsumedV1) {
        let Some(session) = self.inner.read().await.get(&event.sid.to_vec()).cloned() else {
            return;
        };
        match event.role {
            proto::Role::App => {
                *session.app_token_hash.lock().await = None;
            }
            proto::Role::Wallet => {
                *session.wallet_token_hash.lock().await = None;
            }
        }
        self.shared
            .p2p_role_consumed_total
            .fetch_add(1, Ordering::Relaxed);
    }
    async fn apply_session_terminated(&self, event: proto::ConnectSessionTerminatedV1) {
        if self
            .terminate_session_inner(event.sid, &event.reason, false)
            .await
        {
            self.shared
                .p2p_session_terminated_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }
    async fn relay_from_p2p(&self, envelope: proto::ConnectRelayEnvelopeV1) {
        if envelope.ttl == 0 {
            self.shared
                .p2p_ttl_drops_total
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        let sid = envelope.frame.sid;
        let Some(sess) = self.inner.read().await.get(&sid.to_vec()).cloned() else {
            self.shared
                .p2p_unknown_session_drops_total
                .fetch_add(1, Ordering::Relaxed);
            debug!(sid = ?hex::encode(sid), "connect: dropping P2P relay for unknown session");
            return;
        };
        if sess.peer_claim_expired_at(unix_time_ms()) {
            debug!(sid = ?hex::encode(sid), "connect: dropping P2P relay for expired peer session claim");
            return;
        }
        let Some(relay_key) = *sess.relay_key.lock().await else {
            self.shared
                .p2p_auth_failures_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(sid = ?hex::encode(sid), "connect: dropping P2P relay without local relay key");
            return;
        };
        match connect_sdk::verify_relay_envelope(&relay_key, &envelope) {
            Ok(true) => {
                let next_ttl = envelope.ttl.saturating_sub(1);
                self.relay_with_session(envelope.frame, next_ttl, sess)
                    .await;
            }
            Ok(false) => {
                self.shared
                    .p2p_auth_failures_total
                    .fetch_add(1, Ordering::Relaxed);
                warn!(sid = ?hex::encode(sid), "connect: dropping P2P relay with invalid MAC");
            }
            Err(err) => {
                self.shared
                    .p2p_auth_failures_total
                    .fetch_add(1, Ordering::Relaxed);
                warn!(sid = ?hex::encode(sid), ?err, "connect: failed to verify P2P relay MAC");
            }
        }
    }
    async fn relay_from_role_for_session(
        &self,
        session: &Arc<Session>,
        role: proto::Role,
        frame: proto::ConnectFrameV1,
    ) -> bool {
        if !self.session_is_current(&frame.sid, session).await {
            return false;
        }
        let expected_dir = expected_direction_for_role(role);
        if frame.dir != expected_dir {
            self.shared
                .role_direction_mismatch_total
                .fetch_add(1, Ordering::Relaxed);
            warn!(
                sid = ?hex::encode(frame.sid),
                ?role,
                frame_dir = ?frame.dir,
                expected_dir = ?expected_dir,
                "connect: closing session on role/direction mismatch"
            );
            self.terminate_session_if_current(
                frame.sid,
                session,
                CLOSE_REASON_ROLE_DIRECTION_MISMATCH,
                true,
            )
            .await;
            return false;
        }
        self.relay_with_session(frame, self.policy.p2p_ttl_hops, session.clone())
            .await;
        true
    }
    #[cfg(test)]
    async fn relay_from_role(&self, role: proto::Role, frame: proto::ConnectFrameV1) -> bool {
        let Some(session) = self.inner.read().await.get(&frame.sid.to_vec()).cloned() else {
            return false;
        };
        self.relay_from_role_for_session(&session, role, frame)
            .await
    }
    async fn session_is_current(&self, sid: &Sid, expected: &Arc<Session>) -> bool {
        self.inner
            .read()
            .await
            .get(&sid.to_vec())
            .is_some_and(|current| Arc::ptr_eq(current, expected))
    }
    async fn touch_session_if_current(&self, sid: &Sid, expected: &Arc<Session>) -> bool {
        let sessions = self.inner.read().await;
        let Some(current) = sessions.get(&sid.to_vec()) else {
            return false;
        };
        if !Arc::ptr_eq(current, expected) || current.peer_claim_expired_at(unix_time_ms()) {
            return false;
        }
        *current.last_activity.lock().await = Instant::now();
        true
    }
    #[cfg(test)]
    async fn take_buffered_for_role(
        sess: Arc<Session>,
        role: proto::Role,
    ) -> VecDeque<proto::ConnectFrameV1> {
        let mut out = VecDeque::new();
        {
            let mut buf = sess.buffer.lock().await;
            let mut bytes = sess.buffer_bytes.lock().await;
            let mut kept = VecDeque::new();
            while let Some((f, l)) = buf.pop_front() {
                let target = match f.dir {
                    proto::Dir::AppToWallet => proto::Role::Wallet,
                    proto::Dir::WalletToApp => proto::Role::App,
                };
                if target == role {
                    out.push_back(f);
                    *bytes = bytes.saturating_sub(l);
                } else {
                    kept.push_back((f, l));
                }
            }
            *buf = kept;
        }
        if !out.is_empty() {
            *sess.last_activity.lock().await = Instant::now();
        }
        out
    }
    #[cfg(test)]
    async fn evaluate_heartbeat(
        &self,
        sid: &Sid,
        role: proto::Role,
        now: Instant,
    ) -> Option<HeartbeatFailure> {
        let session = self.inner.read().await.get(&sid.to_vec()).cloned()?;
        self.evaluate_heartbeat_for(sid, &session, role, now).await
    }
    async fn evaluate_heartbeat_for(
        &self,
        sid: &Sid,
        expected: &Arc<Session>,
        role: proto::Role,
        now: Instant,
    ) -> Option<HeartbeatFailure> {
        let tolerance = usize::try_from(self.policy.heartbeat_miss_tolerance).unwrap_or(usize::MAX);
        if tolerance == 0 {
            return None;
        }
        let sessions = self.inner.read().await;
        let current = sessions.get(&sid.to_vec())?;
        if !Arc::ptr_eq(current, expected) || current.peer_claim_expired_at(unix_time_ms()) {
            return None;
        }
        let queue = current.heartbeat_queue(role).await;
        if queue.pending.is_empty() {
            return None;
        }
        let mut misses = 0usize;
        let mut oldest = Duration::from_secs(0);
        for entry in queue.pending.iter() {
            let elapsed = now.saturating_duration_since(entry.sent_at);
            if elapsed >= self.policy.heartbeat_interval {
                misses += 1;
                if elapsed > oldest {
                    oldest = elapsed;
                }
            } else {
                break;
            }
        }
        drop(queue);
        if misses >= tolerance {
            Some(HeartbeatFailure {
                misses,
                oldest_elapsed: oldest,
            })
        } else {
            None
        }
    }

    /// Apply the configured Connect frame ceiling before the WebSocket is upgraded.
    ///
    /// The in-session decoder enforces the same limit for binary protocol messages,
    /// while this transport-level cap prevents oversized binary or text messages from
    /// being buffered by the WebSocket implementation first.
    pub(crate) fn configure_websocket(
        &self,
        upgrade: axum::extract::WebSocketUpgrade,
    ) -> axum::extract::WebSocketUpgrade {
        upgrade
            .max_message_size(self.policy.frame_max_bytes)
            .max_frame_size(self.policy.frame_max_bytes)
    }
}
#[cfg(test)]
fn test_network_id() -> NetworkId {
    NetworkId::from_genesis_hash(
        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"torii-connect-test-genesis"),
        ),
    )
}
fn diff(req: &Vec<String>, acc: &Vec<String>) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;
    let r: HashSet<_> = req.iter().cloned().collect();
    let a: HashSet<_> = acc.iter().cloned().collect();
    let extra: Vec<String> = a.difference(&r).cloned().collect();
    let missing: Vec<String> = r.difference(&a).cloned().collect();
    (extra, missing)
}
/// WS handler: receives frames from the client and forwards via Bus; delivers frames from Bus to WS.
pub(crate) async fn handle_ws(
    bus: Bus,
    reservation: RoleTokenReservation,
    ws: WebSocket,
    send_timeout: Duration,
) -> Result<(), String> {
    recover_ws_session(handle_ws_inner(bus, reservation, ws, send_timeout)).await
}

async fn recover_ws_session<F>(session: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    match crate::panic_recovery::catch_async_recoverable(session).await {
        Ok(result) => result,
        Err(_) => Err("connect websocket session panicked".to_owned()),
    }
}

async fn drive_ws_halves<R, W>(reader: R, writer: W) -> Result<(), String>
where
    R: Future<Output = Result<(), String>>,
    W: Future<Output = Result<(), String>>,
{
    tokio::pin!(reader);
    tokio::pin!(writer);
    tokio::select! {
        result = &mut reader => result,
        result = &mut writer => result,
    }
}

async fn handle_ws_inner(
    bus: Bus,
    mut reservation: RoleTokenReservation,
    ws: WebSocket,
    send_timeout: Duration,
) -> Result<(), String> {
    let (sid, role) = reservation.identity();
    let (mut inbox, mut endpoint_lease) = reservation.commit_and_attach().await?;
    let bound_session = endpoint_lease.session.clone();
    // Split WS into sender and receiver halves
    let (mut ws_sender, mut ws_receiver) = ws.split();
    // Writer: forward frames from Bus to WS
    let sid_for_writer = sid;
    let bus_for_writer = bus.clone();
    let session_for_writer = bound_session.clone();
    let role_for_writer = role;
    let policy_for_writer = bus.policy;
    let writer = async move {
        let mut ticker_period =
            std::cmp::min(policy_for_writer.heartbeat_interval, Duration::from_secs(5));
        if ticker_period.is_zero() {
            ticker_period = Duration::from_secs(5);
        }
        ticker_period = ticker_period.max(Duration::from_millis(10));
        let mut ticker = tokio::time::interval(ticker_period);
        loop {
            tokio::select! {
                maybe_frame = inbox.recv() => {
                    match maybe_frame {
                        Some(frame) => {
                            let terminal_control = is_terminal_peer_control(&frame);
                            if !terminal_control
                                && !bus_for_writer
                                    .session_is_current(&sid_for_writer, &session_for_writer)
                                    .await
                            {
                                break;
                            }
                            match proto::encode_connect_frame_bare(&frame) {
                                Ok(bytes) => {
                                    match tokio::time::timeout(
                                        send_timeout,
                                        ws_sender.send(Message::Binary(axum::body::Bytes::from(bytes))),
                                    )
                                    .await
                                    {
                                        Ok(Ok(())) => {}
                                        Ok(Err(error)) => {
                                            return Err(format!("ws send failed: {error}"));
                                        }
                                        Err(_) => return Err("ws send timed out".to_owned()),
                                    }
                                    if terminal_control {
                                        break;
                                    }
                                    if !bus_for_writer
                                        .touch_session_if_current(
                                            &sid_for_writer,
                                            &session_for_writer,
                                        )
                                        .await
                                    {
                                        break;
                                    }
                                }
                                Err(err) => {
                                    warn!(
                                        sid = ?hex::encode(sid_for_writer),
                                        ?role_for_writer,
                                        ?err,
                                        "connect: failed to encode frame for websocket delivery"
                                    );
                                    break;
                                }
                            }
                        }
                        None => break, // inbox closed
                    }
                }
                _ = ticker.tick() => {
                    // TTL check
                    let expired = bus_for_writer
                        .session_expired_for(
                            &sid_for_writer,
                            &session_for_writer,
                            Instant::now(),
                        )
                        .await;
                    if expired {
                        // Best-effort Close
                        let _ = tokio::time::timeout(
                            send_timeout,
                            ws_sender.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                code: CLOSE_CODE_TTL,
                                reason: Utf8Bytes::from(CLOSE_REASON_TTL.to_string()),
                            }))),
                        )
                        .await;
                        break;
                    }
                    if let Some(failure) = bus_for_writer
                        .evaluate_heartbeat_for(
                            &sid_for_writer,
                            &session_for_writer,
                            role_for_writer,
                            Instant::now(),
                        )
                        .await
                    {
                        bus_for_writer
                            .shared
                            .ping_miss_total
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(
                            sid = ?hex::encode(sid_for_writer),
                            ?role_for_writer,
                            misses = failure.misses,
                            oldest_ms = failure.oldest_elapsed.as_millis(),
                            "connect: closing websocket after heartbeat timeout"
                        );
                        let _ = tokio::time::timeout(
                            send_timeout,
                            ws_sender.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                code: CLOSE_CODE_HEARTBEAT,
                                reason: Utf8Bytes::from(CLOSE_REASON_HEARTBEAT.to_string()),
                            }))),
                        )
                        .await;
                        break;
                    }
                }
            }
        }
        Ok::<(), String>(())
    };
    // Keep both request-owned halves in this task. The first completion drops the
    // other future immediately, without a nested Tokio task that could escape the
    // session panic boundary.
    let reader = async {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(b)) => {
                    if !bus.touch_session_if_current(&sid, &bound_session).await {
                        break;
                    }
                    if b.len() > bus.policy.frame_max_bytes {
                        break;
                    }
                    match proto::decode_connect_frame_bare(&b) {
                        Ok(frame) => {
                            if frame.sid != sid {
                                break;
                            }
                            if !bus
                                .relay_from_role_for_session(&bound_session, role, frame)
                                .await
                            {
                                break;
                            }
                        }
                        Err(err) => {
                            warn!(
                                sid = ?hex::encode(sid),
                                ?role,
                                len = b.len(),
                                ?err,
                                "connect: failed to decode websocket frame"
                            );
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(_)) => { /* ignore ping in reader */ }
                Ok(Message::Text(_)) => {
                    // Connect is a binary-only protocol. The upgrade-level cap
                    // bounds this frame before it reaches the handler; terminate
                    // rather than letting repeated text traffic occupy a session.
                    break;
                }
                Err(e) => return Err(format!("ws error: {e}")),
                _ => {}
            }
        }
        Ok(())
    };
    let result = drive_ws_halves(reader, writer).await;
    endpoint_lease.release().await;
    result
}
fn expected_direction_for_role(role: proto::Role) -> proto::Dir {
    match role {
        proto::Role::App => proto::Dir::AppToWallet,
        proto::Role::Wallet => proto::Dir::WalletToApp,
    }
}

fn is_terminal_peer_control(frame: &proto::ConnectFrameV1) -> bool {
    matches!(
        &frame.kind,
        proto::FrameKind::Control(
            proto::ConnectControlV1::Close { .. } | proto::ConnectControlV1::Reject { .. }
        )
    )
}
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn decode_sid(s: &str) -> Result<Sid, String> {
    // Expect base64url (no padding).
    let v = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| "sid must be base64url".to_string())?;
    if v.len() != 32 {
        return Err("sid must be 32 bytes".into());
    }
    let mut sid = [0u8; 32];
    sid.copy_from_slice(&v);
    Ok(sid)
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use iroha_crypto::{Hash, KeyPair};
    use std::{collections::BTreeMap, num::NonZeroU64};
    use tokio::time::{Duration, timeout};
    fn test_session_identity(seed: u8) -> (Sid, [u8; 32], [u8; 16]) {
        let app_pk = [seed.max(1); 32];
        let nonce = [seed.wrapping_add(1).max(1); 16];
        let sid = connect_sdk::derive_session_id(&test_network_id(), &app_pk, &nonce);
        (sid, app_pk, nonce)
    }
    fn enabled_test_config() -> iroha_config::parameters::actual::Connect {
        iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 16,
            ws_per_ip_max_sessions: 8,
            ws_rate_per_ip_per_min: 60,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 256_000,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 1,
        }
    }

    #[test]
    fn configured_bus_is_inert_until_torii_starts_it() {
        let bus = Bus::from_config(&enabled_test_config(), test_network_id());
        assert_eq!(
            std::sync::Arc::strong_count(&bus.inner),
            1,
            "construction must not detach a self-retaining cleaner"
        );
    }

    #[tokio::test]
    async fn cleaner_stops_and_releases_its_bus_on_shutdown() {
        let bus = Bus::from_config(&enabled_test_config(), test_network_id());
        let shutdown = ShutdownSignal::new();
        let cleaner = bus.start_cleaner(shutdown.clone());
        assert_eq!(std::sync::Arc::strong_count(&bus.inner), 2);
        shutdown.send();
        let exit = timeout(Duration::from_secs(1), cleaner)
            .await
            .expect("Connect cleaner must observe shutdown")
            .expect("Connect cleaner must not panic");
        assert_eq!(exit, crate::ToriiCriticalWorkerExit::StoppedByShutdown);
        assert_eq!(std::sync::Arc::strong_count(&bus.inner), 1);
    }

    #[tokio::test]
    async fn websocket_writer_panic_is_contained_and_disconnects_session() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x6d);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        let mut reservation = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("reserve app endpoint");
        let session = reservation.session.clone();
        let (_inbox, endpoint_lease) = reservation
            .commit_and_attach()
            .await
            .expect("attach app endpoint");

        let outcome = recover_ws_session(async move {
            let _endpoint_lease = endpoint_lease;
            drive_ws_halves(std::future::pending::<Result<(), String>>(), async {
                assert!(
                    iroha_core::panic_hook::is_suppressed(),
                    "the physical writer future must run inside the session recovery boundary"
                );
                panic!("injected Connect websocket writer panic");
                #[allow(unreachable_code)]
                Ok(())
            })
            .await
        })
        .await;

        assert_eq!(
            outcome,
            Err("connect websocket session panicked".to_owned())
        );
        assert!(
            !iroha_core::panic_hook::is_suppressed(),
            "panic-hook suppression must not leak into the caller"
        );
        timeout(Duration::from_secs(1), async {
            while bus.session_is_current(&sid, &session).await {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("panicked writer releases and disconnects its endpoint");
    }

    fn test_claim(
        seed: u8,
        app_token: &str,
        wallet_token: &str,
        management_token: &str,
        relay_token: &str,
    ) -> proto::ConnectSessionClaimV1 {
        let (sid, app_pk, nonce) = test_session_identity(seed);
        proto::ConnectSessionClaimV1 {
            sid,
            network_id: test_network_id(),
            app_pk,
            nonce,
            token_app_hash: connect_sdk::token_auth_hash(
                connect_sdk::TokenKind::App,
                &sid,
                app_token,
            ),
            token_wallet_hash: connect_sdk::token_auth_hash(
                connect_sdk::TokenKind::Wallet,
                &sid,
                wallet_token,
            ),
            token_management_hash: connect_sdk::token_auth_hash(
                connect_sdk::TokenKind::Management,
                &sid,
                management_token,
            ),
            relay_mac_key: connect_sdk::derive_relay_mac_key(&sid, relay_token),
            relay_auth_hash: connect_sdk::relay_auth_hash(&sid, relay_token),
            expires_at_ms: expires_at_ms(Duration::from_mins(5)),
        }
    }
    fn signed_approval_control(
        key_pair: &KeyPair,
        constraints: &proto::Constraints,
        sid: &Sid,
        app_pk: &[u8; 32],
        wallet_pk: [u8; 32],
        relay_token: &str,
    ) -> proto::ConnectControlV1 {
        let account_id = AccountId::new(key_pair.public_key().clone()).to_string();
        let relay_auth = connect_sdk::relay_auth_hash(sid, relay_token);
        let preimage = connect_sdk::build_approve_preimage(
            constraints,
            sid,
            app_pk,
            &wallet_pk,
            &account_id,
            None,
            None,
            &relay_auth,
        );
        let sig_wallet = proto::WalletSignatureV1::new(
            Algorithm::Ed25519,
            Signature::try_new(key_pair.private_key(), &preimage).expect("approval fixture signs"),
        );
        proto::ConnectControlV1::Approve {
            wallet_pk,
            account_id,
            permissions: None,
            proof: None,
            sig_wallet,
        }
    }
    #[tokio::test]
    async fn register_tokens_rejects_duplicate_sid() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x11);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "t-app".into(),
            "t-wallet".into(),
            "t-management".into(),
            "t-relay".into(),
        )
        .await
        .expect("first registration succeeds");
        let err = bus
            .register_tokens(
                sid,
                app_pk,
                nonce,
                "t-app-2".into(),
                "t-wallet-2".into(),
                "t-management-2".into(),
                "t-relay-2".into(),
            )
            .await
            .expect_err("duplicate sid should be rejected");
        assert_eq!(err, RegisterSessionError::Exists);
    }
    #[tokio::test]
    async fn concurrent_registration_cannot_replace_session_identity() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x31);
        let first = bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token-a".into(),
            "wallet-token-a".into(),
            "management-token-a".into(),
            "relay-token-a".into(),
        );
        let second = bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token-b".into(),
            "wallet-token-b".into(),
            "management-token-b".into(),
            "relay-token-b".into(),
        );
        let (first, second) = tokio::join!(first, second);
        assert!(
            matches!(
                (&first, &second),
                (Ok(()), Err(RegisterSessionError::Exists))
                    | (Err(RegisterSessionError::Exists), Ok(()))
            ),
            "exactly one registration must win: first={first:?}, second={second:?}"
        );
        assert_eq!(bus.inner.read().await.len(), 1);
        let accepted_a = bus
            .authorize_management_token(sid, "management-token-a")
            .await;
        let accepted_b = bus
            .authorize_management_token(sid, "management-token-b")
            .await;
        assert_ne!(
            accepted_a, accepted_b,
            "the losing registration must not replace the winning token binding"
        );
    }
    #[tokio::test]
    async fn register_tokens_stores_token_hashes() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x51);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        let session = bus
            .inner
            .read()
            .await
            .get(&sid.to_vec())
            .cloned()
            .expect("session registered");
        assert_eq!(
            *session.app_token_hash.lock().await,
            Some(connect_sdk::token_auth_hash(
                connect_sdk::TokenKind::App,
                &sid,
                "app-token"
            ))
        );
        assert_eq!(
            *session.management_token_hash.lock().await,
            Some(connect_sdk::token_auth_hash(
                connect_sdk::TokenKind::Management,
                &sid,
                "management-token"
            ))
        );
        assert!(
            bus.authorize_management_token(sid, "management-token")
                .await
        );
        assert!(!bus.authorize_management_token(sid, "wrong-token").await);
        let mut reservation = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("app token accepted");
        let _app_inbox = reservation
            .commit_and_attach()
            .await
            .expect("app endpoint attaches");
        assert_eq!(*session.app_token_hash.lock().await, None);
        assert!(
            bus.reserve_token(sid, proto::Role::App, "app-token")
                .await
                .is_err(),
            "role token is one-time"
        );
    }
    #[tokio::test]
    async fn dropped_role_token_reservation_can_be_retried() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x61);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");

        let reservation = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("first reservation succeeds");
        assert!(
            bus.reserve_token(sid, proto::Role::App, "app-token")
                .await
                .is_err(),
            "a concurrent upgrade must not share a one-time token"
        );
        drop(reservation);

        let retry = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("dropping an uncommitted upgrade restores availability");
        drop(retry);
    }
    #[tokio::test]
    async fn remote_role_consumption_wins_over_reservation_rollback() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x62);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        let reservation = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("reserve app role");
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::RoleConsumed(
            proto::ConnectSessionRoleConsumedV1 {
                sid,
                role: proto::Role::App,
            },
        ))
        .await;
        drop(reservation);
        assert!(
            bus.reserve_token(sid, proto::Role::App, "app-token")
                .await
                .is_err(),
            "rolling back a stale local reservation must not resurrect a remotely consumed token"
        );
    }
    #[tokio::test]
    async fn terminated_reserved_session_cannot_be_reattached_or_replace_retry() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x63);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "old-app-token".into(),
            "old-wallet-token".into(),
            "old-management-token".into(),
            "old-relay-token".into(),
        )
        .await
        .expect("register old session");
        let mut stale = bus
            .reserve_token(sid, proto::Role::App, "old-app-token")
            .await
            .expect("reserve old session");
        assert!(bus.terminate_session(sid, "test termination").await);
        assert!(stale.commit_and_attach().await.is_err());

        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "new-app-token".into(),
            "new-wallet-token".into(),
            "new-management-token".into(),
            "new-relay-token".into(),
        )
        .await
        .expect("register replacement session");
        drop(stale);
        let retry = bus
            .reserve_token(sid, proto::Role::App, "new-app-token")
            .await
            .expect("stale rollback cannot affect replacement session");
        drop(retry);
    }
    #[tokio::test]
    async fn stale_management_token_cannot_terminate_replacement_session() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x69);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "old-app-token".into(),
            "old-wallet-token".into(),
            "old-management-token".into(),
            "old-relay-token".into(),
        )
        .await
        .expect("register old session");
        assert!(bus.terminate_session(sid, "replace in test").await);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "new-app-token".into(),
            "new-wallet-token".into(),
            "new-management-token".into(),
            "new-relay-token".into(),
        )
        .await
        .expect("register replacement session");

        assert!(
            !bus.terminate_session_authorized(sid, "old-management-token", "stale delete",)
                .await
        );
        assert!(
            bus.session_status(sid, "new-management-token")
                .await
                .is_some(),
            "the stale authorized operation must not remove a new SID incarnation"
        );
        assert!(
            bus.terminate_session_authorized(sid, "new-management-token", "current delete",)
                .await
        );
    }
    #[tokio::test]
    async fn stale_relay_work_cannot_touch_deliver_to_or_terminate_replacement() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x6A);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "old-app-token".into(),
            "old-wallet-token".into(),
            "old-management-token".into(),
            "old-relay-token".into(),
        )
        .await
        .expect("register old session");
        let stale = bus
            .inner
            .read()
            .await
            .get(&sid.to_vec())
            .cloned()
            .expect("old session");
        assert!(bus.terminate_session(sid, "replace in test").await);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "new-app-token".into(),
            "new-wallet-token".into(),
            "new-management-token".into(),
            "new-relay-token".into(),
        )
        .await
        .expect("register replacement session");
        let replacement = bus
            .inner
            .read()
            .await
            .get(&sid.to_vec())
            .cloned()
            .expect("replacement session");
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let replacement_activity = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("test instant");
        *replacement.last_activity.lock().await = replacement_activity;

        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        bus.relay_with_session(frame, 0, stale.clone()).await;
        assert!(
            timeout(Duration::from_millis(20), wallet_inbox.recv())
                .await
                .is_err(),
            "stale relay work must not deliver into the replacement endpoint"
        );
        assert_eq!(*replacement.last_seq_app_to_wallet.lock().await, None);
        assert_eq!(
            *replacement.last_activity.lock().await,
            replacement_activity
        );
        assert!(!bus.touch_session_if_current(&sid, &stale).await);
        assert!(
            bus.session_expired_for(&sid, &stale, Instant::now()).await,
            "a replaced incarnation is terminal for its old websocket"
        );
        assert!(
            !bus.terminate_session_if_current(sid, &stale, "stale relay", true)
                .await
        );
        assert!(
            bus.session_status(sid, "new-management-token")
                .await
                .is_some()
        );
    }
    #[tokio::test]
    async fn committed_attach_drains_more_than_channel_capacity_in_order() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x64);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        for seq in 1..=65 {
            bus.relay(proto::ConnectFrameV1 {
                sid,
                dir: proto::Dir::AppToWallet,
                seq,
                kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: seq }),
            })
            .await;
        }
        let mut reservation = bus
            .reserve_token(sid, proto::Role::Wallet, "wallet-token")
            .await
            .expect("reserve wallet role");
        let (mut inbox, _endpoint_lease) =
            timeout(Duration::from_millis(100), reservation.commit_and_attach())
                .await
                .expect("attach must not block on the 64-frame live channel")
                .expect("attach succeeds");
        for seq in 1..=65 {
            let frame = inbox.recv().await.expect("buffered frame");
            assert_eq!(frame.seq, seq);
        }
    }
    #[tokio::test]
    async fn offline_buffer_overflow_terminates_instead_of_creating_sequence_gap() {
        let (sid, app_pk, nonce) = test_session_identity(0x68);
        let first = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        let second = proto::ConnectFrameV1 {
            seq: 2,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 2 }),
            ..first.clone()
        };
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 16,
            ws_per_ip_max_sessions: 16,
            ws_rate_per_ip_per_min: 0,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: encoded_len(&first).expect("encoded frame size"),
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        bus.relay(first).await;
        assert_eq!(
            bus.session_status(sid, "management-token")
                .await
                .expect("session remains after first buffered frame")
                .buffered_frames,
            1
        );

        bus.relay(second).await;
        assert!(
            bus.session_status(sid, "management-token").await.is_none(),
            "overflow must remove the irrecoverably gapped session"
        );
        let close = timeout(Duration::from_millis(100), app_inbox.recv())
            .await
            .expect("sender receives overflow close")
            .expect("close frame");
        assert!(matches!(
            close.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { reason, .. })
                if reason == CLOSE_REASON_BUFFER_OVERFLOW
        ));
        assert_eq!(bus.status().await.buffer_drops_total, 1);
    }
    #[tokio::test]
    async fn p2p_session_claim_installs_shadow_session() {
        let bus = Bus::new();
        let claim = test_claim(
            0x52,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let sid = claim.sid;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        let status = bus
            .session_status(sid, "management-token")
            .await
            .expect("management token works on peer-claimed session");
        assert_eq!(status.origin, "peer_claimed");
        assert_eq!(bus.status().await.p2p_session_claims_installed_total, 1);
        let mut reservation = bus
            .reserve_token(sid, proto::Role::Wallet, "wallet-token")
            .await
            .expect("wallet can attach through peer-claimed session");
        let _wallet_inbox = reservation
            .commit_and_attach()
            .await
            .expect("wallet endpoint attaches");
    }
    #[tokio::test]
    async fn peer_claim_absolute_expiry_is_retained_and_pruned() {
        let bus = Bus::new();
        let mut claim = test_claim(
            0x62,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        claim.expires_at_ms = unix_time_ms().saturating_add(60_000);
        let sid = claim.sid;
        let expires_at_ms = claim.expires_at_ms;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        let session = bus
            .inner
            .read()
            .await
            .get(&sid.to_vec())
            .cloned()
            .expect("peer shadow installed");
        assert_eq!(session.peer_claim_expires_at_ms, Some(expires_at_ms));

        let mut reservation = bus
            .reserve_token(sid, proto::Role::Wallet, "wallet-token")
            .await
            .expect("unexpired peer token reserves");
        let (_inbox, _lease) = reservation
            .commit_and_attach()
            .await
            .expect("unexpired peer endpoint attaches");
        assert_eq!(
            bus.prune_expired_sessions_at(Instant::now(), expires_at_ms)
                .await,
            1,
            "the absolute claim deadline overrides activity and attachment"
        );
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn expired_peer_claim_rejects_tokens_and_management_reads() {
        let bus = Bus::new();
        let claim = test_claim(
            0x63,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let session = Arc::new(Session::new(
            SessionOrigin::PeerClaimed,
            Some(unix_time_ms()),
        ));
        *session.app_token_hash.lock().await = Some(claim.token_app_hash);
        *session.management_token_hash.lock().await = Some(claim.token_management_hash);
        bus.inner.write().await.insert(claim.sid.to_vec(), session);

        assert!(
            bus.reserve_token(claim.sid, proto::Role::App, "app-token")
                .await
                .is_err()
        );
        assert!(
            !bus.authorize_management_token(claim.sid, "management-token")
                .await
        );
        assert!(
            bus.session_status(claim.sid, "management-token")
                .await
                .is_none()
        );
    }
    #[tokio::test]
    async fn p2p_role_consumed_clears_peer_token_hash() {
        let bus = Bus::new();
        let claim = test_claim(
            0x53,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let sid = claim.sid;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::RoleConsumed(
            proto::ConnectSessionRoleConsumedV1 {
                sid,
                role: proto::Role::App,
            },
        ))
        .await;
        assert!(
            bus.reserve_token(sid, proto::Role::App, "app-token")
                .await
                .is_err(),
            "consumed role gossip prevents duplicate app attach"
        );
        assert_eq!(bus.status().await.p2p_role_consumed_total, 1);
    }
    #[tokio::test]
    async fn p2p_session_terminated_removes_peer_session() {
        let bus = Bus::new();
        let claim = test_claim(
            0x54,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let sid = claim.sid;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionTerminated(
            proto::ConnectSessionTerminatedV1 {
                sid,
                reason: "connect_session_revoked_by_test".into(),
            },
        ))
        .await;
        assert!(bus.session_status(sid, "management-token").await.is_none());
        assert_eq!(bus.status().await.p2p_session_terminated_total, 1);
    }
    #[tokio::test]
    async fn p2p_conflicting_session_claim_is_ignored() {
        let bus = Bus::new();
        let claim = test_claim(
            0x55,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let sid = claim.sid;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        let conflict = test_claim(
            0x55,
            "app-token",
            "wallet-token",
            "management-token",
            "other-relay-token",
        );
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(conflict))
            .await;
        let status = bus.status().await;
        assert_eq!(status.p2p_session_claim_conflicts_total, 1);
        assert!(
            bus.authorize_management_token(sid, "management-token")
                .await
        );
    }
    #[tokio::test]
    async fn p2p_matching_claim_cannot_extend_absolute_deadline() {
        let bus = Bus::new();
        let claim = test_claim(
            0x65,
            "app-token",
            "wallet-token",
            "management-token",
            "relay-token",
        );
        let sid = claim.sid;
        let original_expiry = claim.expires_at_ms;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        let mut extension = claim;
        extension.expires_at_ms = extension.expires_at_ms.saturating_add(60_000);
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(extension))
            .await;

        let session = bus
            .inner
            .read()
            .await
            .get(&sid.to_vec())
            .cloned()
            .expect("original peer shadow remains");
        assert_eq!(session.peer_claim_expires_at_ms, Some(original_expiry));
        assert_eq!(bus.status().await.p2p_session_claim_conflicts_total, 1);
    }
    #[tokio::test]
    async fn p2p_relay_requires_prior_session_claim() {
        let bus = Bus::new();
        let (sid, _, _) = test_session_identity(0x56);
        let relay_token = "relay-token";
        let relay_key = connect_sdk::derive_relay_mac_key(&sid, relay_token);
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 42 }),
        };
        let envelope =
            connect_sdk::seal_relay_envelope(&relay_key, frame.clone(), 1).expect("envelope");
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::RelayEnvelope(envelope.clone()))
            .await;
        assert_eq!(bus.status().await.p2p_unknown_session_drops_total, 1);
        let claim = test_claim(
            0x56,
            "app-token",
            "wallet-token",
            "management-token",
            relay_token,
        );
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::SessionClaim(claim))
            .await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        bus.handle_p2p_message(proto::ConnectP2pMessageV1::RelayEnvelope(envelope))
            .await;
        let delivered = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("wallet receives relay after claim")
            .expect("frame delivered");
        assert_eq!(delivered, frame);
    }
    #[tokio::test]
    async fn session_creation_rate_limited() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 16,
            ws_per_ip_max_sessions: 16,
            ws_rate_per_ip_per_min: 1,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "198.51.100.7".parse().unwrap();
        bus.pre_session_create(ip).await.expect("first create ok");
        let err = bus.pre_session_create(ip).await.expect_err("rate limit");
        assert_eq!(err.0, axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
    #[tokio::test]
    async fn session_creation_respects_global_cap() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1,
            ws_per_ip_max_sessions: 16,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let (sid, app_pk, nonce) = test_session_identity(0xAB);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "t-app".into(),
            "t-wallet".into(),
            "t-management".into(),
            "t-relay".into(),
        )
        .await
        .expect("first registration ok");
        let err = bus
            .pre_session_create("203.0.113.1".parse().unwrap())
            .await
            .expect_err("cap enforced");
        assert_eq!(err.0, axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
    #[tokio::test]
    async fn bus_attach_forward_detach() {
        let bus = Bus::new();
        let sid = [7u8; 32];
        // Attach app and wallet endpoints
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        // Send a frame from app to wallet
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        bus.relay(frame.clone()).await;
        let got = wallet_inbox.recv().await.expect("wallet should receive");
        assert_eq!(got, frame);
        // Detach
        bus.detach(sid, proto::Role::App).await;
        bus.detach(sid, proto::Role::Wallet).await;
    }
    #[tokio::test]
    async fn per_ip_session_cap_enforced() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 2,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        // Two sessions should be allowed
        let mut first = bus.pre_ws_handshake(ip).await.expect("first ok");
        let mut second = bus.pre_ws_handshake(ip).await.expect("second ok");
        // Third should be rejected by per-ip cap
        assert!(bus.pre_ws_handshake(ip).await.is_err());
        // Close one and attempt again
        first.release().await;
        let mut third = bus
            .pre_ws_handshake(ip)
            .await
            .expect("third ok after release");
        second.release().await;
        third.release().await;
    }
    #[tokio::test]
    async fn dropped_ws_permit_releases_capacity() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1,
            ws_per_ip_max_sessions: 1,
            ws_rate_per_ip_per_min: 0,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "192.0.2.44".parse().expect("test IP");
        let permit = bus.pre_ws_handshake(ip).await.expect("reserve slot");
        drop(permit);
        timeout(Duration::from_millis(100), async {
            loop {
                if bus.status().await.sessions_total == 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("drop cleanup releases the slot");
        let mut retry = bus
            .pre_ws_handshake(ip)
            .await
            .expect("capacity is reusable after cancelled upgrade");
        retry.release().await;
    }
    #[tokio::test]
    async fn panicking_upgrade_callback_releases_ws_permit_capacity() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1,
            ws_per_ip_max_sessions: 1,
            ws_rate_per_ip_per_min: 0,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "192.0.2.45".parse().expect("test IP");
        let permit = bus.pre_ws_handshake(ip).await.expect("reserve slot");
        let result = crate::panic_recovery::catch_async_recoverable(async move {
            let _permit = permit;
            panic!("injected upgrade callback panic");
        })
        .await;
        assert!(result.is_err());
        timeout(Duration::from_millis(100), async {
            while bus.status().await.sessions_total != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("panic cleanup releases the slot");
        let mut retry = bus
            .pre_ws_handshake(ip)
            .await
            .expect("capacity is reusable after a callback panic");
        retry.release().await;
    }
    #[tokio::test]
    async fn cancelled_ws_reservation_cannot_leak_capacity_at_lock_boundaries() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1,
            ws_per_ip_max_sessions: 1,
            ws_rate_per_ip_per_min: 1,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "198.51.100.77".parse().expect("test IP");

        let counts_guard = bus.per_ip_counts.lock().await;
        let blocked_bus = bus.clone();
        let blocked = tokio::spawn(async move { blocked_bus.pre_ws_handshake(ip).await });
        tokio::task::yield_now().await;
        blocked.abort();
        drop(counts_guard);
        assert!(blocked.await.is_err());
        assert_eq!(bus.status().await.sessions_total, 0);

        let buckets_guard = bus.handshake_buckets.lock().await;
        let blocked_bus = bus.clone();
        let blocked = tokio::spawn(async move { blocked_bus.pre_ws_handshake(ip).await });
        timeout(Duration::from_millis(100), async {
            while bus.status().await.sessions_total == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("reservation reaches the rate bucket");
        blocked.abort();
        drop(buckets_guard);
        assert!(blocked.await.is_err());
        timeout(Duration::from_millis(100), async {
            while bus.status().await.sessions_total != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("permit drop releases cancellation at rate bucket");

        let mut retry = bus
            .pre_ws_handshake(ip)
            .await
            .expect("capacity remains reusable");
        retry.release().await;
    }
    #[tokio::test]
    async fn blocked_delivery_does_not_hold_global_session_map() {
        let bus = Bus::new();
        let sid = [0x65; SID_LEN];
        let wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        let session = bus.get_or_create(&sid).await;
        for _ in 0..64 {
            assert_eq!(
                bus.deliver_local_only(&session, &frame)
                    .await
                    .expect("delivery does not time out before capacity"),
                LocalDelivery::Delivered
            );
        }
        let blocked_bus = bus.clone();
        let blocked_session = session.clone();
        let blocked_frame = frame.clone();
        let blocked = tokio::spawn(async move {
            blocked_bus
                .deliver_local_only(&blocked_session, &blocked_frame)
                .await
        });
        tokio::task::yield_now().await;

        timeout(
            Duration::from_millis(100),
            bus.terminate_session([0x66; SID_LEN], "unrelated test session"),
        )
        .await
        .expect("an unrelated map writer must not wait for a full role inbox");
        drop(wallet_inbox);
        assert_eq!(
            timeout(Duration::from_millis(100), blocked)
                .await
                .expect("blocked sender observes receiver close")
                .expect("delivery task completes")
                .expect("closed receiver is not a delivery timeout"),
            LocalDelivery::Offline
        );
    }
    #[tokio::test]
    async fn terminate_session_sends_close_frames() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x42);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session tokens");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let removed = bus
            .terminate_session(sid, "connect_session_revoked_by_test")
            .await;
        assert!(removed);
        let close_to_app = timeout(Duration::from_millis(100), app_inbox.recv())
            .await
            .expect("app should receive close")
            .expect("close frame");
        assert_eq!(close_to_app.dir, proto::Dir::WalletToApp);
        assert!(matches!(
            close_to_app.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        let close_to_wallet = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("wallet should receive close")
            .expect("close frame");
        assert_eq!(close_to_wallet.dir, proto::Dir::AppToWallet);
        assert!(matches!(
            close_to_wallet.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        let second = bus
            .terminate_session(sid, "connect_session_revoked_by_test")
            .await;
        assert!(
            !second,
            "subsequent termination should report session missing"
        );
    }
    #[tokio::test]
    async fn attached_transport_loss_terminates_session_and_notifies_survivor() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x67);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session");
        let mut reservation = bus
            .reserve_token(sid, proto::Role::App, "app-token")
            .await
            .expect("reserve app role");
        let (app_inbox, app_endpoint_lease) = reservation
            .commit_and_attach()
            .await
            .expect("attach app role");
        let mut wallet_reservation = bus
            .reserve_token(sid, proto::Role::Wallet, "wallet-token")
            .await
            .expect("reserve wallet role");
        let (mut wallet_inbox, wallet_endpoint_lease) = wallet_reservation
            .commit_and_attach()
            .await
            .expect("attach wallet role");

        drop(app_inbox);
        drop(app_endpoint_lease);
        let close = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("surviving endpoint receives terminal control")
            .expect("terminal close frame");
        assert!(matches!(
            close.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { reason, .. })
                if reason == CLOSE_REASON_TRANSPORT_CLOSED
        ));
        assert!(
            bus.session_status(sid, "management-token").await.is_none(),
            "a consumed role cannot reconnect, so transport loss must discard the SID"
        );
        drop(wallet_endpoint_lease);
    }
    #[tokio::test]
    async fn clones_share_session_counters() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1,
            ws_per_ip_max_sessions: 5,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus_primary = Bus::from_config(&cfg, test_network_id());
        let bus_clone = bus_primary.clone();
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        let mut permit = bus_primary.pre_ws_handshake(ip).await.unwrap();
        let status_from_clone = bus_clone.status().await;
        assert_eq!(status_from_clone.sessions_total, 1);
        assert!(bus_clone.pre_ws_handshake(ip).await.is_err());
        permit.release().await;
        let status_after_close = bus_primary.status().await;
        assert_eq!(status_after_close.sessions_total, 0);
        // Once closed, the clone should permit another handshake.
        let mut reopened = bus_clone.pre_ws_handshake(ip).await.expect("reopen ok");
        reopened.release().await;
    }
    #[tokio::test]
    async fn session_expired_returns_true_when_missing() {
        let bus = Bus::new();
        let sid = [0x10u8; 32];
        let expired = bus.session_expired(&sid, Instant::now()).await;
        assert!(expired, "missing sessions should be treated as expired");
    }
    #[tokio::test]
    async fn prune_expired_sessions_skips_active_endpoints() {
        let bus = Bus::new();
        let sid = [0x21u8; 32];
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let sess = bus.get_or_create(&sid).await;
        *sess.last_activity.lock().await = Instant::now()
            .checked_sub(Duration::from_mins(10))
            .expect("activity instant fits");
        let removed = bus.prune_expired_sessions(Instant::now()).await;
        assert_eq!(removed, 0);
        assert!(bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn prune_expired_sessions_removes_inactive_sessions() {
        let bus = Bus::new();
        let sid = [0x22u8; 32];
        let sess = bus.get_or_create(&sid).await;
        *sess.last_activity.lock().await = Instant::now()
            .checked_sub(Duration::from_mins(10))
            .expect("activity instant fits");
        let removed = bus.prune_expired_sessions(Instant::now()).await;
        assert_eq!(removed, 1);
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn expired_candidate_recheck_preserves_attached_or_replaced_session() {
        let bus = Bus::new();
        let now = Instant::now();
        let ttl = Duration::from_secs(1);

        let attached_sid = [0x23u8; SID_LEN];
        let attached_candidate = bus.get_or_create(&attached_sid).await;
        *attached_candidate.last_activity.lock().await = now
            .checked_sub(Duration::from_secs(2))
            .expect("test instant");
        let _attached = bus.attach(attached_sid, proto::Role::App).await;
        assert_eq!(
            bus.remove_expired_candidates(
                now,
                ttl,
                vec![(attached_sid.to_vec(), attached_candidate)],
            )
            .await,
            0
        );
        assert!(bus.inner.read().await.contains_key(&attached_sid.to_vec()));

        let replaced_sid = [0x24u8; SID_LEN];
        let stale_candidate = bus.get_or_create(&replaced_sid).await;
        *stale_candidate.last_activity.lock().await = now
            .checked_sub(Duration::from_secs(2))
            .expect("test instant");
        let replacement = Arc::new(Session::default());
        bus.inner
            .write()
            .await
            .insert(replaced_sid.to_vec(), replacement.clone());
        assert_eq!(
            bus.remove_expired_candidates(
                now,
                ttl,
                vec![(replaced_sid.to_vec(), stale_candidate)],
            )
            .await,
            0
        );
        let current = bus
            .inner
            .read()
            .await
            .get(&replaced_sid.to_vec())
            .cloned()
            .expect("replacement remains");
        assert!(Arc::ptr_eq(&current, &replacement));
    }
    #[tokio::test]
    async fn prune_handshake_buckets_removes_idle_entries() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 1000,
            ws_rate_per_ip_per_min: 1,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "203.0.113.99".parse().unwrap();
        let mut permit = bus.pre_ws_handshake(ip).await.expect("handshake ok");
        let expiry = Instant::now() + bus.handshake_bucket_ttl() + Duration::from_secs(1);
        let removed = bus.prune_handshake_buckets(expiry).await;
        assert_eq!(removed, 1);
        let removed_again = bus.prune_handshake_buckets(expiry).await;
        assert_eq!(removed_again, 0);
        permit.release().await;
    }
    #[tokio::test]
    async fn handshake_rate_zero_disables_limit() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 1000,
            ws_rate_per_ip_per_min: 0,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "203.0.113.10".parse().unwrap();
        for _ in 0..4 {
            let mut permit = bus.pre_ws_handshake(ip).await.expect("handshake ok");
            permit.release().await;
        }
    }
    #[tokio::test]
    async fn per_ip_session_cap_zero_disables_limit() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 0,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "198.51.100.1".parse().unwrap();
        let mut first = bus.pre_ws_handshake(ip).await.expect("first ok");
        let mut second = bus.pre_ws_handshake(ip).await.expect("second ok");
        let mut third = bus.pre_ws_handshake(ip).await.expect("third ok");
        first.release().await;
        second.release().await;
        third.release().await;
    }
    #[tokio::test]
    async fn handshake_rate_limited() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 1000,
            ws_rate_per_ip_per_min: 2,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        // Two immediate handshakes allowed (burst = 2)
        let mut first = bus.pre_ws_handshake(ip).await.expect("first ok");
        let mut second = bus.pre_ws_handshake(ip).await.expect("second ok");
        // Third should be rate-limited
        assert!(bus.pre_ws_handshake(ip).await.is_err());
        first.release().await;
        second.release().await;
    }
    #[tokio::test]
    async fn heartbeat_failure_detected() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 16,
            ws_per_ip_max_sessions: 16,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(1),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_millis(100),
            ping_miss_tolerance: 2,
            ping_min_interval: Duration::from_millis(50),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let sid = [0xAAu8; 32];
        let mut _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut _wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        for nonce in 1..=2u64 {
            let frame = proto::ConnectFrameV1 {
                sid,
                dir: proto::Dir::WalletToApp,
                seq: nonce,
                kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce }),
            };
            bus.relay(frame).await;
        }
        {
            let session = {
                let map = bus.inner.read().await;
                map.get(&sid.to_vec()).cloned().expect("session exists")
            };
            let mut queue = session.heartbeat_queue(proto::Role::App).await;
            let now = Instant::now();
            for (idx, entry) in queue.pending.iter_mut().enumerate() {
                let factor = (idx as f32) + 2.0;
                entry.sent_at = now
                    .checked_sub(cfg.ping_interval.mul_f32(factor))
                    .expect("ping interval scaling stays within instant range");
            }
        }
        let failure = bus
            .evaluate_heartbeat(&sid, proto::Role::App, Instant::now())
            .await;
        assert!(
            matches!(failure, Some(f) if f.misses >= 2),
            "expected heartbeat misses to be detected"
        );
    }
    #[tokio::test]
    async fn closes_session_on_non_contiguous_seq_frames() {
        let bus = Bus::new();
        let sid = [5u8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        // Send first contiguous frame.
        let f1 = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 7 }),
        };
        bus.relay(f1).await;
        let got = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive seq=1");
        assert_eq!(got.seq, 1);
        // Skip seq=2 and send seq=3; session should be terminated.
        let f3 = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 3,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 8 }),
        };
        bus.relay(f3).await;
        let close_to_wallet = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("wallet close")
            .expect("close frame");
        let close_to_app = timeout(Duration::from_millis(100), app_inbox.recv())
            .await
            .expect("app close")
            .expect("close frame");
        assert!(matches!(
            close_to_wallet.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        assert!(matches!(
            close_to_app.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        let st = bus.status().await;
        assert!(st.monotonic_drops_total >= 1);
        assert!(st.sequence_violation_closes_total >= 1);
    }
    #[tokio::test]
    async fn duplicate_frame_does_not_close_session() {
        let bus = Bus::new();
        let sid = [0x6Au8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let f1 = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 11 }),
        };
        bus.relay(f1.clone()).await;
        let got1 = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive first frame");
        assert_eq!(got1.seq, 1);
        // Duplicate seq=1 should be dropped by dedupe, not treated as sequence violation.
        bus.relay(f1).await;
        assert!(
            timeout(Duration::from_millis(50), wallet_inbox.recv())
                .await
                .is_err(),
            "duplicate frame should not be delivered"
        );
        assert!(
            timeout(Duration::from_millis(50), app_inbox.recv())
                .await
                .is_err(),
            "duplicate frame should not close the session"
        );
        let f2 = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 2,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 12 }),
        };
        bus.relay(f2).await;
        let got2 = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive next contiguous frame");
        assert_eq!(got2.seq, 2);
        let st = bus.status().await;
        assert!(st.dedupe_drops_total >= 1);
        assert_eq!(st.sequence_violation_closes_total, 0);
    }
    #[tokio::test]
    async fn closes_session_on_role_direction_mismatch() {
        let bus = Bus::new();
        let sid = [0x9Au8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        // App role is only allowed to send AppToWallet, so this must close the session.
        let mismatch = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        let accepted = bus.relay_from_role(proto::Role::App, mismatch).await;
        assert!(!accepted, "mismatched role/direction must be rejected");
        let close_to_wallet = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("wallet close")
            .expect("close frame");
        let close_to_app = timeout(Duration::from_millis(100), app_inbox.recv())
            .await
            .expect("app close")
            .expect("close frame");
        assert!(matches!(
            close_to_wallet.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        assert!(matches!(
            close_to_app.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        let st = bus.status().await;
        assert!(st.role_direction_mismatch_total >= 1);
    }
    #[test]
    fn websocket_writer_treats_reject_and_close_as_terminal() {
        let frame = |kind| proto::ConnectFrameV1 {
            sid: [0x9B; SID_LEN],
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(kind),
        };
        assert!(is_terminal_peer_control(&frame(
            proto::ConnectControlV1::Reject {
                code: 1,
                code_id: "USER_DENIED".to_owned(),
                reason: "denied in test".to_owned(),
            },
        )));
        assert!(is_terminal_peer_control(&frame(
            proto::ConnectControlV1::Close {
                who: proto::Role::Wallet,
                code: CLOSE_CODE_PURGED,
                reason: CLOSE_REASON_PURGED.to_owned(),
                retryable: false,
            },
        )));
        assert!(!is_terminal_peer_control(&frame(
            proto::ConnectControlV1::Ping { nonce: 7 },
        )));
    }
    #[tokio::test]
    async fn preapproval_reject_is_terminal_after_peer_delivery() {
        let bus = Bus::new();
        let sid = [0x9Bu8; SID_LEN];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let reject = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Reject {
                code: 1,
                code_id: "USER_DENIED".to_owned(),
                reason: "denied in test".to_owned(),
            }),
        };
        assert!(bus.relay_from_role(proto::Role::Wallet, reject).await);
        let delivered = app_inbox.recv().await.expect("app receives rejection");
        assert!(matches!(
            delivered.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Reject { .. })
        ));
        let close_to_app = app_inbox.recv().await.expect("app receives terminal close");
        let close_to_wallet = wallet_inbox
            .recv()
            .await
            .expect("wallet receives terminal close");
        for close in [close_to_app, close_to_wallet] {
            assert!(matches!(
                close.kind,
                proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                    if reason == CLOSE_REASON_REJECTED
            ));
        }
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn ciphertext_direction_substitution_terminates_session() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x6A);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register direction-substitution fixture");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Ciphertext(proto::ConnectCiphertextV1 {
                dir: proto::Dir::WalletToApp,
                aead: vec![0xA5; 32],
            }),
        })
        .await;
        for inbox in [&mut app_inbox, &mut wallet_inbox] {
            let closed = timeout(Duration::from_millis(50), inbox.recv())
                .await
                .expect("peer receives direction-substitution close")
                .expect("close frame");
            assert!(matches!(
                closed.kind,
                proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                    if reason == CLOSE_REASON_ROLE_DIRECTION_MISMATCH
            ));
        }
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
        assert!(bus.status().await.role_direction_mismatch_total >= 1);
    }
    #[test]
    fn expected_direction_matches_role() {
        assert_eq!(
            expected_direction_for_role(proto::Role::App),
            proto::Dir::AppToWallet
        );
        assert_eq!(
            expected_direction_for_role(proto::Role::Wallet),
            proto::Dir::WalletToApp
        );
    }
    #[test]
    fn relay_strategy_parser_accepts_exact_v1_names() {
        assert_eq!(
            RelayStrategy::from_config(
                iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            ),
            RelayStrategy::Broadcast
        );
        assert_eq!(
            RelayStrategy::from_config(
                iroha_config::parameters::actual::ConnectRelayStrategy::LocalOnly,
            ),
            RelayStrategy::LocalOnly
        );
    }
    #[tokio::test]
    async fn broadcast_strategy_with_zero_ttl_reports_local_only_when_p2p_attached() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        {
            let mut p2p = bus.p2p.write().await;
            *p2p = Some(corelib::IrohaNetwork::closed_for_tests());
        }
        let status = bus.status().await;
        assert_eq!(status.policy.relay_strategy, "broadcast");
        assert!(status.policy.relay_p2p_attached);
        assert_eq!(status.policy.p2p_ttl_hops, 0);
        assert_eq!(status.policy.relay_effective_strategy, "local_only");
    }
    #[tokio::test]
    async fn broadcast_strategy_records_p2p_rebroadcast_when_network_attached() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 1,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        {
            let mut p2p = bus.p2p.write().await;
            *p2p = Some(corelib::IrohaNetwork::closed_for_tests());
        }
        let (sid, app_pk, nonce) = test_session_identity(0xB1);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register session tokens");
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 42 }),
        };
        bus.relay(frame).await;
        let got = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive frame");
        assert_eq!(got.seq, 1);
        let status = bus.status().await;
        assert!(status.policy.relay_p2p_attached);
        assert_eq!(status.policy.relay_effective_strategy, "broadcast");
        assert_eq!(status.p2p_rebroadcasts_total, 1);
        assert_eq!(status.p2p_rebroadcast_skipped_total, 0);
    }
    #[tokio::test]
    async fn broadcast_strategy_without_network_does_not_increment_rebroadcast_counter() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 1,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let sid = [0xB4u8; 32];
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 45 }),
        };
        bus.relay(frame).await;
        let got = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive frame");
        assert_eq!(got.seq, 1);
        let status = bus.status().await;
        assert_eq!(
            status.policy.relay_strategy, "broadcast",
            "policy should still report broadcast"
        );
        assert!(!status.policy.relay_p2p_attached);
        assert_eq!(
            status.policy.relay_effective_strategy, "local_only",
            "without a P2P network, broadcast falls back to local-only delivery"
        );
        assert_eq!(status.p2p_rebroadcasts_total, 0);
        assert_eq!(
            status.p2p_rebroadcast_skipped_total, 1,
            "rebroadcast should be skipped when no P2P network is attached"
        );
    }
    #[tokio::test]
    async fn local_only_strategy_skips_p2p_rebroadcast_when_network_attached() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: true,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::LocalOnly,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        {
            let mut p2p = bus.p2p.write().await;
            *p2p = Some(corelib::IrohaNetwork::closed_for_tests());
        }
        let sid = [0xB2u8; 32];
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 43 }),
        };
        bus.relay(frame).await;
        let got = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive frame");
        assert_eq!(got.seq, 1);
        let status = bus.status().await;
        assert_eq!(
            status.policy.relay_strategy, "local_only",
            "local-only policy must remain local"
        );
        assert!(status.policy.relay_p2p_attached);
        assert_eq!(status.policy.relay_effective_strategy, "local_only");
        assert_eq!(status.p2p_rebroadcasts_total, 0);
        assert_eq!(status.p2p_rebroadcast_skipped_total, 0);
    }
    #[tokio::test]
    async fn relay_disabled_skips_p2p_rebroadcast_when_network_attached() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 1000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_secs(30),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_secs(15),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        {
            let mut p2p = bus.p2p.write().await;
            *p2p = Some(corelib::IrohaNetwork::closed_for_tests());
        }
        let sid = [0xB3u8; 32];
        let _app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 44 }),
        };
        bus.relay(frame).await;
        let got = wallet_inbox
            .recv()
            .await
            .expect("wallet should receive frame");
        assert_eq!(got.seq, 1);
        let status = bus.status().await;
        assert!(!status.policy.relay_enabled);
        assert!(status.policy.relay_p2p_attached);
        assert_eq!(status.policy.relay_effective_strategy, "local_only");
        assert_eq!(status.p2p_rebroadcasts_total, 0);
        assert_eq!(status.p2p_rebroadcast_skipped_total, 0);
    }
    #[tokio::test]
    async fn drops_oversized_frames_on_p2p_ingress() {
        let bus = Bus::new();
        let sid = [0x77u8; 32];
        // Ensure session exists so relay does not drop early.
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let oversized = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Ciphertext(proto::ConnectCiphertextV1 {
                dir: proto::Dir::AppToWallet,
                aead: vec![0u8; 70_000], // exceeds 64_000 default cap once encoded
            }),
        };
        bus.relay(oversized).await;
        assert!(
            timeout(Duration::from_millis(50), wallet_inbox.recv())
                .await
                .is_err(),
            "oversized frame should be dropped before delivery"
        );
        let status = bus.status().await;
        assert_eq!(status.frames_out_total, 0, "no frames delivered");
    }
    #[tokio::test]
    async fn ciphertext_before_verified_approval_terminates_session() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x70);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register pre-approval ciphertext fixture");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Ciphertext(proto::ConnectCiphertextV1 {
                dir: proto::Dir::AppToWallet,
                aead: vec![0xA5; 32],
            }),
        })
        .await;
        for inbox in [&mut app_inbox, &mut wallet_inbox] {
            let closed = timeout(Duration::from_millis(50), inbox.recv())
                .await
                .expect("peer receives pre-approval rejection close")
                .expect("close frame");
            assert!(matches!(
                closed.kind,
                proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                    if reason == CLOSE_REASON_APPROVAL_INVALID
            ));
        }
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn drops_plaintext_control_after_approve() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(6);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register approval fixture session");
        // Attach both sides
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let constraints = proto::Constraints {
            network_id: test_network_id(),
        };
        let open = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: constraints.clone(),
                permissions: None,
            }),
        };
        bus.relay(open).await;
        wallet_inbox
            .recv()
            .await
            .expect("wallet should receive Open");
        let key_pair = KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Ed25519)
            .expect("approval fixture keypair");
        let account_id = AccountId::new(key_pair.public_key().clone()).to_string();
        let wallet_pk = [1u8; 32];
        let relay_auth = connect_sdk::relay_auth_hash(&sid, "relay-token");
        let preimage = connect_sdk::build_approve_preimage(
            &constraints,
            &sid,
            &app_pk,
            &wallet_pk,
            &account_id,
            None,
            None,
            &relay_auth,
        );
        let sig_wallet = proto::WalletSignatureV1::new(
            Algorithm::Ed25519,
            Signature::try_new(key_pair.private_key(), &preimage).expect("approval fixture signs"),
        );
        // Send Approve from wallet to app (seq=1)
        let approve = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Approve {
                wallet_pk,
                account_id,
                permissions: None,
                proof: None,
                sig_wallet,
            }),
        };
        bus.relay(approve.clone()).await;
        // App should receive Approve
        let got = app_inbox.recv().await.expect("app should receive Approve");
        assert!(matches!(
            got.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { .. })
        ));
        // Now send plaintext Close after approval; should be dropped.
        // This is the first App->Wallet frame in this test, so seq must start at 1.
        let close = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Close {
                who: proto::Role::App,
                code: 1000,
                reason: "test".into(),
                retryable: false,
            }),
        };
        bus.relay(close).await;
        // Wallet should not receive within timeout
        assert!(
            timeout(Duration::from_millis(50), wallet_inbox.recv())
                .await
                .is_err()
        );
        let st = bus.status().await;
        assert!(st.plaintext_control_drops_total >= 1);
    }
    #[tokio::test]
    async fn wrong_network_open_is_rejected_before_wallet_delivery() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x71);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register wrong-network fixture session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let foreign_network = NetworkId::from_genesis_hash(HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"torii-connect-test-foreign-genesis"),
        ));
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: proto::Constraints {
                    network_id: foreign_network,
                },
                permissions: None,
            }),
        })
        .await;
        let wallet_closed = timeout(Duration::from_millis(50), wallet_inbox.recv())
            .await
            .expect("wallet receives rejection close")
            .expect("close frame");
        assert!(matches!(
            wallet_closed.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                if reason == CLOSE_REASON_NETWORK_MISMATCH
        ));
        let closed = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app receives rejection close")
            .expect("close frame");
        assert!(matches!(
            closed.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                if reason == CLOSE_REASON_NETWORK_MISMATCH
        ));
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn substituted_wallet_approval_is_rejected_before_app_delivery() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x72);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register substitution fixture session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let constraints = proto::Constraints {
            network_id: test_network_id(),
        };
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: constraints.clone(),
                permissions: None,
            }),
        })
        .await;
        wallet_inbox.recv().await.expect("wallet receives Open");
        let key_pair = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
            .expect("approval substitution fixture keypair");
        let mut approve = signed_approval_control(
            &key_pair,
            &constraints,
            &sid,
            &app_pk,
            [0x74; 32],
            "relay-token",
        );
        let proto::ConnectControlV1::Approve { wallet_pk, .. } = &mut approve else {
            unreachable!("approval helper must return Approve")
        };
        *wallet_pk = [0x75; 32];
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(approve),
        })
        .await;
        let closed = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app receives invalid-approval close")
            .expect("close frame");
        assert!(matches!(
            closed.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                if reason == CLOSE_REASON_APPROVAL_INVALID
        ));
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn noncanonical_wallet_account_id_is_rejected_before_app_delivery() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0xA2);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register noncanonical-account fixture session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let constraints = proto::Constraints {
            network_id: test_network_id(),
        };
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: constraints.clone(),
                permissions: None,
            }),
        })
        .await;
        wallet_inbox.recv().await.expect("wallet receives Open");
        let key_pair = KeyPair::try_from_seed(vec![0xA3; 32], Algorithm::Ed25519)
            .expect("noncanonical-account fixture keypair");
        let wallet_pk = [0xA4; 32];
        let canonical = AccountId::new(key_pair.public_key().clone()).to_string();
        let account_id = format!(" {canonical}\t");
        let relay_auth = connect_sdk::relay_auth_hash(&sid, "relay-token");
        let preimage = connect_sdk::build_approve_preimage(
            &constraints,
            &sid,
            &app_pk,
            &wallet_pk,
            &account_id,
            None,
            None,
            &relay_auth,
        );
        let sig_wallet = proto::WalletSignatureV1::new(
            Algorithm::Ed25519,
            Signature::try_new(key_pair.private_key(), &preimage)
                .expect("sign noncanonical account spelling exactly"),
        );
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Approve {
                wallet_pk,
                account_id,
                permissions: None,
                proof: None,
                sig_wallet,
            }),
        })
        .await;
        let closed = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app receives noncanonical-account rejection close")
            .expect("close frame");
        assert!(matches!(
            closed.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                if reason == CLOSE_REASON_APPROVAL_INVALID
        ));
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn repeated_valid_wallet_approval_terminates_the_session() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x76);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register replay fixture session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let constraints = proto::Constraints {
            network_id: test_network_id(),
        };
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: constraints.clone(),
                permissions: None,
            }),
        })
        .await;
        wallet_inbox.recv().await.expect("wallet receives Open");
        let key_pair = KeyPair::try_from_seed(vec![0x77; 32], Algorithm::Ed25519)
            .expect("approval replay fixture keypair");
        let approve = signed_approval_control(
            &key_pair,
            &constraints,
            &sid,
            &app_pk,
            [0x78; 32],
            "relay-token",
        );
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(approve.clone()),
        })
        .await;
        assert!(matches!(
            app_inbox
                .recv()
                .await
                .expect("app receives first approval")
                .kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { .. })
        ));
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 2,
            kind: proto::FrameKind::Control(approve),
        })
        .await;
        let closed = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app receives replay close")
            .expect("close frame");
        assert!(matches!(
            closed.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { ref reason, .. })
                if reason == CLOSE_REASON_APPROVAL_REPLAY
        ));
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn concurrent_wallet_approvals_cannot_both_reach_the_app() {
        let bus = Bus::new();
        let (sid, app_pk, nonce) = test_session_identity(0x79);
        bus.register_tokens(
            sid,
            app_pk,
            nonce,
            "app-token".into(),
            "wallet-token".into(),
            "management-token".into(),
            "relay-token".into(),
        )
        .await
        .expect("register concurrent approval fixture session");
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let constraints = proto::Constraints {
            network_id: test_network_id(),
        };
        bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::AppToWallet,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Open {
                app_pk,
                app_meta: None,
                constraints: constraints.clone(),
                permissions: None,
            }),
        })
        .await;
        wallet_inbox.recv().await.expect("wallet receives Open");
        let key_pair = KeyPair::try_from_seed(vec![0x7A; 32], Algorithm::Ed25519)
            .expect("concurrent approval fixture keypair");
        let approve = signed_approval_control(
            &key_pair,
            &constraints,
            &sid,
            &app_pk,
            [0x7B; 32],
            "relay-token",
        );
        let first = bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(approve.clone()),
        });
        let second = bus.relay(proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 2,
            kind: proto::FrameKind::Control(approve),
        });
        tokio::join!(first, second);
        let mut approvals = 0;
        let mut closes = 0;
        while let Ok(Some(frame)) = timeout(Duration::from_millis(10), app_inbox.recv()).await {
            match frame.kind {
                proto::FrameKind::Control(proto::ConnectControlV1::Approve { .. }) => {
                    approvals += 1;
                }
                proto::FrameKind::Control(proto::ConnectControlV1::Close { .. }) => closes += 1,
                _ => {}
            }
        }
        assert!(
            approvals <= 1,
            "approval gate delivered {approvals} replays"
        );
        assert!(closes >= 1, "approval race must close the session");
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
    }
    #[tokio::test]
    async fn server_events_do_not_advance_peer_seq() {
        let bus = Bus::new();
        let sid = [0xACu8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let session = bus.get_or_create(&sid).await;
        let initial = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 1 }),
        };
        bus.relay(initial).await;
        let got = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app frame")
            .expect("frame");
        assert_eq!(got.seq, 1);
        assert_eq!(*session.last_seq_wallet_to_app.lock().await, Some(1));
        let before_activity = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .expect("activity instant fits");
        *session.last_activity.lock().await = before_activity;
        let control = proto::ConnectControlV1::ServerEvent {
            event: proto::ServerEventV1::BlockProofs {
                height: 1,
                entry_hash: "00".into(),
                proofs_json: "{}".into(),
            },
        };
        bus.send_server_event(
            &sid,
            session.clone(),
            proto::Dir::WalletToApp,
            &control,
            proto::Role::App,
        )
        .await;
        let server_frame = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("server event")
            .expect("frame");
        assert!(matches!(
            server_frame.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::ServerEvent { .. })
        ));
        assert_eq!(*session.last_seq_wallet_to_app.lock().await, Some(1));
        let after_activity = *session.last_activity.lock().await;
        assert!(
            after_activity > before_activity,
            "server events should update session activity"
        );
        let next = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 2,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 2 }),
        };
        bus.relay(next).await;
        let got_next = timeout(Duration::from_millis(50), app_inbox.recv())
            .await
            .expect("app frame")
            .expect("frame");
        assert_eq!(got_next.seq, 2);
    }
    #[tokio::test]
    async fn stalled_server_event_delivery_is_bounded_and_quarantines_exact_session() {
        let cfg = iroha_config::parameters::actual::Connect {
            enabled: true,
            ws_max_sessions: 16,
            ws_per_ip_max_sessions: 16,
            ws_rate_per_ip_per_min: 0,
            session_ttl: Duration::from_mins(5),
            frame_max_bytes: 64_000,
            session_buffer_max_bytes: 262_144,
            ping_interval: Duration::from_millis(10),
            ping_miss_tolerance: 3,
            ping_min_interval: Duration::from_millis(10),
            dedupe_ttl: Duration::from_mins(2),
            dedupe_cap: 8192,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::actual::ConnectRelayStrategy::LocalOnly,
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg, test_network_id());
        let sid = [0xAE; SID_LEN];
        let _stalled_inbox = bus.attach(sid, proto::Role::App).await;
        let session = bus.get_or_create(&sid).await;
        let control = proto::ConnectControlV1::ServerEvent {
            event: proto::ServerEventV1::BlockProofs {
                height: 1,
                entry_hash: "00".into(),
                proofs_json: "{}".into(),
            },
        };
        for _ in 0..64 {
            bus.send_server_event(
                &sid,
                session.clone(),
                proto::Dir::WalletToApp,
                &control,
                proto::Role::App,
            )
            .await;
        }
        assert_eq!(bus.status().await.frames_out_total, 64);

        timeout(
            Duration::from_millis(100),
            bus.send_server_event(
                &sid,
                session,
                proto::Dir::WalletToApp,
                &control,
                proto::Role::App,
            ),
        )
        .await
        .expect("a full local endpoint cannot block server-event fanout");
        assert!(!bus.inner.read().await.contains_key(&sid.to_vec()));
        assert_eq!(
            bus.status().await.frames_out_total,
            64,
            "timed-out sends are not counted as delivered"
        );
    }
    #[tokio::test]
    async fn notify_close_updates_activity_without_touching_peer_seq() {
        let bus = Bus::new();
        let sid = [0xADu8; 32];
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let session = bus.get_or_create(&sid).await;
        *session.last_seq_app_to_wallet.lock().await = Some(7);
        let before_activity = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("activity instant fits");
        *session.last_activity.lock().await = before_activity;
        bus.notify_close(session.clone(), sid, proto::Role::Wallet, "test close")
            .await;
        let close_frame = timeout(Duration::from_millis(50), wallet_inbox.recv())
            .await
            .expect("close frame")
            .expect("frame");
        assert!(matches!(
            close_frame.kind,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. })
        ));
        assert_eq!(close_frame.seq, 1);
        assert_eq!(*session.last_seq_app_to_wallet.lock().await, Some(7));
        let after_activity = *session.last_activity.lock().await;
        assert!(
            after_activity > before_activity,
            "close frames should update session activity"
        );
    }
    #[tokio::test]
    async fn broadcasts_block_proofs_to_app_and_wallet() {
        use iroha_data_model::{
            block::proofs::ExecutionReceiptProof, transaction::signed::TransactionResult,
        };
        let bus = Bus::new();
        let sid = [0xBCu8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;
        let entry_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed([0x11u8; 32]));
        let entry_tree: MerkleTree<TransactionEntrypoint> = [entry_hash].into_iter().collect();
        let entry_commitment = entry_tree.commitment().expect("entry commitment");
        let entry_proof: BlockReceiptProof =
            BlockReceiptProof::new(entry_hash, entry_tree.get_proof(0).expect("entry proof"));
        let result_hash =
            HashOf::<TransactionResult>::from_untyped_unchecked(Hash::prehashed([0x23u8; 32]));
        let result_tree: MerkleTree<TransactionResult> = [result_hash].into_iter().collect();
        let result_commitment = result_tree.commitment().expect("result commitment");
        let result_proof = ExecutionReceiptProof::new(
            result_hash,
            result_tree.get_proof(0).expect("result proof"),
        );
        let proofs = BlockProofs {
            block_height: NonZeroU64::new(1).expect("non-zero height"),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"connect carrier block")),
            executed_block_wire_hash: Hash::new(b"connect executed block wire"),
            entry_hash,
            entry_commitment,
            entry_proof,
            result_commitment,
            result_proof,
            fastpq_transcripts: BTreeMap::new(),
        };
        let expected_entry_hex = hex::encode(entry_hash.as_ref());
        let expected_json = norito::json::to_json(&proofs).expect("serialize proofs");
        bus.broadcast_block_proof(&proofs)
            .await
            .expect("broadcast block proof");
        let to_app = timeout(Duration::from_millis(100), app_inbox.recv())
            .await
            .expect("app frame")
            .expect("frame");
        assert_eq!(to_app.dir, proto::Dir::WalletToApp);
        if let proto::FrameKind::Control(proto::ConnectControlV1::ServerEvent { event }) =
            to_app.kind
        {
            let proto::ServerEventV1::BlockProofs {
                height,
                entry_hash,
                proofs_json,
            } = event;
            assert_eq!(height, 1);
            assert_eq!(entry_hash, expected_entry_hex);
            assert_eq!(proofs_json, expected_json);
        } else {
            panic!("expected server event frame for app");
        }
        let to_wallet = timeout(Duration::from_millis(100), wallet_inbox.recv())
            .await
            .expect("wallet frame")
            .expect("frame");
        assert_eq!(to_wallet.dir, proto::Dir::AppToWallet);
        if let proto::FrameKind::Control(proto::ConnectControlV1::ServerEvent { event }) =
            to_wallet.kind
        {
            let proto::ServerEventV1::BlockProofs {
                height,
                entry_hash,
                proofs_json,
            } = event;
            assert_eq!(height, 1);
            assert_eq!(entry_hash, expected_entry_hex);
            assert_eq!(proofs_json, expected_json);
        } else {
            panic!("expected server event frame for wallet");
        }
    }
    #[test]
    fn decode_sid_accepts_base64url() {
        let sid = [0x11u8; 32];
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sid);
        let decoded = decode_sid(&encoded).expect("decode base64url sid");
        assert_eq!(decoded, sid);
    }
    #[test]
    fn decode_sid_rejects_hex() {
        let sid = [0x22u8; 32];
        let hex = hex::encode(sid);
        assert!(decode_sid(&hex).is_err(), "hex should be rejected");
    }
}
impl Bus {
    /// Attach a P2P network handle and start the supervised inbound subscriber.
    pub(crate) async fn attach_network(
        &self,
        network: corelib::IrohaNetwork,
        shutdown_signal: ShutdownSignal,
    ) -> tokio::task::JoinHandle<crate::ToriiCriticalWorkerExit> {
        use iroha_p2p::network::{
            SubscriberFilter,
            message::{SubscriberRoute, Topic},
        };
        *self.p2p.write().await = Some(network.clone());
        let me = self.clone();
        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(network.subscriber_queue_cap().get());
            let filter =
                SubscriberFilter::topics_for_route([Topic::Health], SubscriberRoute::Connect);
            let mut tx = tx;
            loop {
                if shutdown_signal.is_sent() {
                    return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                }
                match network.subscribe_to_peers_messages_with_filter(tx, filter.clone()) {
                    Ok(()) => break,
                    Err(returned) => {
                        iroha_logger::warn!("retrying Torii Connect relay subscription to P2P bus");
                        tx = returned;
                        tokio::select! {
                            biased;
                            () = shutdown_signal.receive() => {
                                return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                            }
                            () = tokio::time::sleep(Duration::from_millis(50)) => {}
                        }
                    }
                }
            }
            loop {
                let msg = tokio::select! {
                    biased;
                    () = shutdown_signal.receive() => {
                        return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                    }
                    msg = rx.recv() => msg,
                };
                let Some(msg) = msg else {
                    return if shutdown_signal.is_sent() {
                        crate::ToriiCriticalWorkerExit::StoppedByShutdown
                    } else {
                        crate::ToriiCriticalWorkerExit::UnexpectedExit
                    };
                };
                let payload = msg.payload;
                if let corelib::NetworkMessage::Connect(message) = payload {
                    tokio::select! {
                        biased;
                        () = shutdown_signal.receive() => {
                            return crate::ToriiCriticalWorkerExit::StoppedByShutdown;
                        }
                        () = me.handle_p2p_message(*message) => {}
                    }
                }
            }
        })
    }
    /// Snapshot current metrics for ops.
    pub async fn status(&self) -> ConnectStatus {
        // Aggregate buffer stats
        let mut total_buffer_bytes = 0usize;
        let mut buffered_sessions = 0usize;
        let session_count;
        {
            let map = self.inner.read().await;
            session_count = map.len();
            for (_k, sess) in map.iter() {
                let b = *sess.buffer_bytes.lock().await;
                if b > 0 {
                    buffered_sessions += 1;
                }
                total_buffer_bytes += b;
            }
        }
        let per_ip_sessions = {
            let m = self.per_ip_counts.lock().await;
            m.iter()
                .map(|(ip, c)| PerIpSessionsEntry {
                    ip: ip.to_string(),
                    sessions: *c,
                })
                .collect()
        };
        let dedupe_size = {
            let seen = self.seen.lock().await;
            seen.map.len()
        };
        let relay_p2p_attached = self.p2p.read().await.is_some();
        let relay_effective_strategy = self
            .policy
            .effective_relay_strategy(relay_p2p_attached)
            .as_str();
        ConnectStatus {
            enabled: true,
            sessions_total: self.shared.sessions_total.load(Ordering::Relaxed),
            sessions_active: session_count,
            per_ip_sessions,
            buffered_sessions,
            total_buffer_bytes,
            dedupe_size,
            policy: ConnectPolicyStatus {
                ws_max_sessions: self.policy.ws_max_sessions,
                ws_per_ip_max_sessions: self.policy.ws_per_ip_max_sessions,
                ws_rate_per_ip_per_min: self.policy.ws_rate_per_ip_per_min,
                session_ttl_ms: self.policy.session_ttl.as_millis() as u64,
                frame_max_bytes: self.policy.frame_max_bytes,
                session_buffer_max_bytes: self.policy.session_buffer_max_bytes,
                relay_enabled: self.policy.relay_enabled,
                relay_strategy: self.policy.relay_strategy.as_str(),
                relay_effective_strategy,
                relay_p2p_attached,
                p2p_ttl_hops: self.policy.p2p_ttl_hops,
                heartbeat_interval_ms: self.policy.heartbeat_interval.as_millis() as u64,
                heartbeat_miss_tolerance: self.policy.heartbeat_miss_tolerance,
                heartbeat_min_interval_ms: self.policy.heartbeat_min_interval.as_millis() as u64,
            },
            frames_in_total: self.shared.frames_in_total.load(Ordering::Relaxed),
            frames_out_total: self.shared.frames_out_total.load(Ordering::Relaxed),
            ciphertext_total: self.shared.ciphertext_total.load(Ordering::Relaxed),
            dedupe_drops_total: self.shared.dedupe_drops_total.load(Ordering::Relaxed),
            buffer_drops_total: self.shared.buffer_drops_total.load(Ordering::Relaxed),
            plaintext_control_drops_total: self
                .shared
                .plaintext_control_drops_total
                .load(Ordering::Relaxed),
            monotonic_drops_total: self.shared.monotonic_drops_total.load(Ordering::Relaxed),
            sequence_violation_closes_total: self
                .shared
                .sequence_violation_closes_total
                .load(Ordering::Relaxed),
            role_direction_mismatch_total: self
                .shared
                .role_direction_mismatch_total
                .load(Ordering::Relaxed),
            ping_miss_total: self.shared.ping_miss_total.load(Ordering::Relaxed),
            p2p_rebroadcasts_total: self.shared.p2p_rebroadcasts_total.load(Ordering::Relaxed),
            p2p_rebroadcast_skipped_total: self
                .shared
                .p2p_rebroadcast_skipped_total
                .load(Ordering::Relaxed),
            p2p_auth_failures_total: self.shared.p2p_auth_failures_total.load(Ordering::Relaxed),
            p2p_ttl_drops_total: self.shared.p2p_ttl_drops_total.load(Ordering::Relaxed),
            p2p_unknown_session_drops_total: self
                .shared
                .p2p_unknown_session_drops_total
                .load(Ordering::Relaxed),
            p2p_session_claims_in_total: self
                .shared
                .p2p_session_claims_in_total
                .load(Ordering::Relaxed),
            p2p_session_claims_installed_total: self
                .shared
                .p2p_session_claims_installed_total
                .load(Ordering::Relaxed),
            p2p_session_claim_conflicts_total: self
                .shared
                .p2p_session_claim_conflicts_total
                .load(Ordering::Relaxed),
            p2p_role_consumed_total: self.shared.p2p_role_consumed_total.load(Ordering::Relaxed),
            p2p_session_terminated_total: self
                .shared
                .p2p_session_terminated_total
                .load(Ordering::Relaxed),
        }
    }
    /// Broadcast a block proof payload to locally attached Connect peers.
    pub async fn broadcast_block_proof(
        &self,
        proofs: &BlockProofs,
    ) -> Result<(), norito::json::Error> {
        let proofs_json = norito::json::to_json(proofs)?;
        let event = proto::ServerEventV1::BlockProofs {
            height: proofs.block_height.get(),
            entry_hash: hex::encode(proofs.entry_hash.as_ref()),
            proofs_json,
        };
        let control = proto::ConnectControlV1::ServerEvent { event };
        let sessions: Vec<(Vec<u8>, Arc<Session>)> = {
            let map = self.inner.read().await;
            map.iter()
                .map(|(sid, sess)| (sid.clone(), sess.clone()))
                .collect()
        };
        for (sid, session) in sessions {
            self.send_server_event(
                &sid,
                session.clone(),
                proto::Dir::WalletToApp,
                &control,
                proto::Role::App,
            )
            .await;
            self.send_server_event(
                &sid,
                session,
                proto::Dir::AppToWallet,
                &control,
                proto::Role::Wallet,
            )
            .await;
        }
        Ok(())
    }
    async fn send_server_event(
        &self,
        sid: &[u8],
        session: Arc<Session>,
        dir: proto::Dir,
        control: &proto::ConnectControlV1,
        target: proto::Role,
    ) {
        let Ok(sid): Result<Sid, _> = sid.try_into() else {
            warn!(
                len = sid.len(),
                "connect: refusing server event for malformed sid"
            );
            return;
        };
        let sessions = self.inner.read().await;
        let Some(current) = sessions.get(&sid.to_vec()) else {
            return;
        };
        if !Arc::ptr_eq(current, &session) || current.peer_claim_expired_at(unix_time_ms()) {
            return;
        }
        let seq = session.next_server_seq(dir).await;
        let frame = proto::ConnectFrameV1 {
            sid,
            dir,
            seq,
            kind: proto::FrameKind::Control(control.clone()),
        };
        let tx_opt = match target {
            proto::Role::App => session.app_tx.lock().await.clone(),
            proto::Role::Wallet => session.wallet_tx.lock().await.clone(),
        };
        if let Some(tx) = tx_opt {
            let delivered =
                tokio::time::timeout(self.local_delivery_timeout(), tx.send(frame)).await;
            if matches!(&delivered, Ok(Ok(()))) {
                *session.last_activity.lock().await = Instant::now();
                self.shared.frames_out_total.fetch_add(1, Ordering::Relaxed);
                return;
            }
            drop(sessions);
            if delivered.is_err() {
                warn!(sid = ?hex::encode(sid), ?target, "connect: terminating session after server-event delivery timed out");
                self.terminate_session_if_current(
                    sid,
                    &session,
                    CLOSE_REASON_DELIVERY_TIMEOUT,
                    true,
                )
                .await;
            }
        }
    }
}
#[derive(JsonSerialize)]
pub struct PerIpSessionsEntry {
    pub ip: String,
    pub sessions: usize,
}
#[derive(JsonSerialize)]
pub struct ConnectStatus {
    pub enabled: bool,
    pub sessions_total: usize,
    pub sessions_active: usize,
    pub per_ip_sessions: Vec<PerIpSessionsEntry>,
    pub buffered_sessions: usize,
    pub total_buffer_bytes: usize,
    pub dedupe_size: usize,
    pub policy: ConnectPolicyStatus,
    pub frames_in_total: u64,
    pub frames_out_total: u64,
    pub ciphertext_total: u64,
    pub dedupe_drops_total: u64,
    pub buffer_drops_total: u64,
    pub plaintext_control_drops_total: u64,
    pub monotonic_drops_total: u64,
    pub sequence_violation_closes_total: u64,
    pub role_direction_mismatch_total: u64,
    pub ping_miss_total: u64,
    pub p2p_rebroadcasts_total: u64,
    pub p2p_rebroadcast_skipped_total: u64,
    pub p2p_auth_failures_total: u64,
    pub p2p_ttl_drops_total: u64,
    pub p2p_unknown_session_drops_total: u64,
    pub p2p_session_claims_in_total: u64,
    pub p2p_session_claims_installed_total: u64,
    pub p2p_session_claim_conflicts_total: u64,
    pub p2p_role_consumed_total: u64,
    pub p2p_session_terminated_total: u64,
}
#[derive(JsonSerialize)]
pub struct ConnectSessionStatus {
    pub sid: String,
    pub app_attached: bool,
    pub wallet_attached: bool,
    pub approved: bool,
    pub buffered_frames: usize,
    pub buffered_bytes: usize,
    pub last_seq_app_to_wallet: Option<u64>,
    pub last_seq_wallet_to_app: Option<u64>,
    pub origin: &'static str,
}
#[derive(Clone, Copy, JsonSerialize)]
pub struct ConnectPolicyStatus {
    pub ws_max_sessions: usize,
    pub ws_per_ip_max_sessions: usize,
    pub ws_rate_per_ip_per_min: u32,
    pub session_ttl_ms: u64,
    pub frame_max_bytes: usize,
    pub session_buffer_max_bytes: usize,
    pub relay_enabled: bool,
    pub relay_strategy: &'static str,
    pub relay_effective_strategy: &'static str,
    pub relay_p2p_attached: bool,
    pub p2p_ttl_hops: u8,
    pub heartbeat_interval_ms: u64,
    pub heartbeat_miss_tolerance: u32,
    pub heartbeat_min_interval_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SeenKey {
    sid: [u8; 32],
    dir: proto::Dir,
    seq: u64,
}
struct SeenCache {
    map: HashMap<SeenKey, Instant>,
    queue: VecDeque<(SeenKey, Instant)>,
    /// Maximum number of entries before pruning oldest.
    cap: usize,
    /// Time-to-live for seen entries.
    ttl: Duration,
}
#[derive(Clone, Copy, Debug)]
struct Policy {
    frame_max_bytes: usize,
    relay_enabled: bool,
    relay_strategy: RelayStrategy,
    ws_max_sessions: usize,
    ws_per_ip_max_sessions: usize,
    ws_rate_per_ip_per_min: u32,
    session_ttl: Duration,
    session_buffer_max_bytes: usize,
    heartbeat_interval: Duration,
    heartbeat_miss_tolerance: u32,
    heartbeat_min_interval: Duration,
    p2p_ttl_hops: u8,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            frame_max_bytes: 64_000,
            relay_enabled: true,
            relay_strategy: RelayStrategy::Broadcast,
            ws_max_sessions: 10_000,
            ws_per_ip_max_sessions: 10,
            ws_rate_per_ip_per_min: 120,
            session_ttl: Duration::from_mins(5),
            session_buffer_max_bytes: 262_144,
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_miss_tolerance: 3,
            heartbeat_min_interval: Duration::from_secs(15),
            p2p_ttl_hops: iroha_config::parameters::defaults::connect::P2P_TTL_HOPS,
        }
    }
}
impl Policy {
    fn effective_relay_strategy(self, relay_p2p_attached: bool) -> RelayStrategy {
        if !self.relay_enabled {
            return RelayStrategy::LocalOnly;
        }
        match self.relay_strategy {
            RelayStrategy::Broadcast if relay_p2p_attached && self.p2p_ttl_hops > 0 => {
                RelayStrategy::Broadcast
            }
            _ => RelayStrategy::LocalOnly,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelayStrategy {
    Broadcast,
    LocalOnly,
}
impl RelayStrategy {
    fn from_config(raw: iroha_config::parameters::actual::ConnectRelayStrategy) -> Self {
        match raw {
            iroha_config::parameters::actual::ConnectRelayStrategy::Broadcast => Self::Broadcast,
            iroha_config::parameters::actual::ConnectRelayStrategy::LocalOnly => Self::LocalOnly,
        }
    }
    const fn as_str(self) -> &'static str {
        match self {
            Self::Broadcast => "broadcast",
            Self::LocalOnly => "local_only",
        }
    }
}
struct TokenBucket {
    rate_per_sec: f64,
    burst: f64,
    tokens: f64,
    last_refill: Instant,
}
impl TokenBucket {
    fn new(rate_per_sec: f64, burst: f64) -> Self {
        Self {
            rate_per_sec,
            burst,
            tokens: burst,
            last_refill: Instant::now(),
        }
    }
    fn allow(&mut self, amount: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.rate_per_sec).min(self.burst);
        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }
}
impl SeenCache {
    fn new(cap: usize, ttl: Duration) -> Self {
        Self {
            map: HashMap::new(),
            queue: VecDeque::new(),
            cap,
            ttl,
        }
    }
    fn record_if_new(&mut self, key: SeenKey) -> bool {
        let now = Instant::now();
        self.prune(now);
        if self.map.contains_key(&key) {
            return false;
        }
        self.map.insert(key, now);
        self.queue.push_back((key, now));
        if self.queue.len() > self.cap {
            self.pop_front_until(self.queue.len() - self.cap);
        }
        true
    }
    fn prune(&mut self, now: Instant) {
        while let Some(&(k, t)) = self.queue.front() {
            if now.duration_since(t) > self.ttl {
                self.queue.pop_front();
                self.map.remove(&k);
            } else {
                break;
            }
        }
    }
    fn pop_front_until(&mut self, n: usize) {
        for _ in 0..n {
            if let Some((k, _)) = self.queue.pop_front() {
                self.map.remove(&k);
            } else {
                break;
            }
        }
    }
}
fn encoded_len(frame: &proto::ConnectFrameV1) -> Option<usize> {
    proto::encode_connect_frame_bare(frame)
        .map(|bytes| bytes.len())
        .ok()
}
