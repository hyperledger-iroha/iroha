#![allow(clippy::redundant_pub_crate)]

//! Iroha Connect WS handlers and minimal in-node relay (Bus).
//!
//! This module provides a feature-gated WS endpoint for a WalletConnect-like
//! flow and a relay bus that bridges app↔wallet connections locally and, when
//! enabled, propagates frames over the Iroha P2P network between nodes.

use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use iroha_core as corelib;
use iroha_crypto::{Algorithm, MerkleTree, Signature};
use iroha_data_model::{
    block::proofs::{BlockProofs, BlockReceiptProof},
    prelude::HashOf,
    transaction::TransactionEntrypoint,
};
use iroha_logger::prelude::*;
use iroha_torii_shared::connect as proto;
use tokio::sync::{Mutex, RwLock, mpsc};

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

#[derive(Clone)]
pub struct Bus {
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

impl WsPermit {
    pub(crate) async fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.bus.session_closed(self.ip).await;
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
    // One-time tokens per role; consumed on first successful WS attach
    app_token: Mutex<Option<String>>,
    wallet_token: Mutex<Option<String>>,
    // Requested/accepted permissions for diagnostics
    req_perms: Mutex<Option<proto::PermissionsV1>>,
    acc_perms: Mutex<Option<proto::PermissionsV1>>,
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

/// Errors raised when registering a session in the Connect bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterSessionError {
    /// Session already exists for the given SID.
    Exists,
    /// Session capacity reached.
    Capacity,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            app_tx: Mutex::new(None),
            wallet_tx: Mutex::new(None),
            last_activity: Mutex::new(Instant::now()),
            buffer: Mutex::new(VecDeque::new()),
            buffer_bytes: Mutex::new(0),
            app_token: Mutex::new(None),
            wallet_token: Mutex::new(None),
            req_perms: Mutex::new(None),
            acc_perms: Mutex::new(None),
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
        #[allow(dead_code)]
        let bus = Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            p2p: Arc::new(RwLock::new(None)),
            seen: Arc::new(Mutex::new(SeenCache::new(8192, Duration::from_mins(2)))),
            policy: Policy::default(),
            shared: Arc::new(BusShared::default()),
            per_ip_counts: Arc::new(Mutex::new(HashMap::new())),
            handshake_buckets: Arc::new(Mutex::new(HashMap::new())),
        };
        bus.start_cleaner();
        bus
    }

    pub fn from_config(cfg: &iroha_config::parameters::actual::Connect) -> Self {
        let bus = Self {
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
            },
            shared: Arc::new(BusShared::default()),
            per_ip_counts: Arc::new(Mutex::new(HashMap::new())),
            handshake_buckets: Arc::new(Mutex::new(HashMap::new())),
        };
        bus.start_cleaner();
        bus
    }

    fn start_cleaner(&self) {
        let me = self.clone();
        tokio::spawn(async move {
            let interval = Duration::from_secs(30);
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let _ = me.prune_expired_sessions(now).await;
                let _ = me.prune_handshake_buckets(now).await;
            }
        });
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

    async fn session_expired(&self, sid: &Sid, now: Instant) -> bool {
        let map_read = self.inner.read().await;
        let Some(sess) = map_read.get(&sid.to_vec()) else {
            return true;
        };
        let last = *sess.last_activity.lock().await;
        now.saturating_duration_since(last) > self.policy.session_ttl
    }

    async fn prune_expired_sessions(&self, now: Instant) -> usize {
        let ttl = self.policy.session_ttl;
        let mut remove_keys = Vec::new();
        {
            let map_read = self.inner.read().await;
            for (k, sess) in map_read.iter() {
                let ts = *sess.last_activity.lock().await;
                if now.saturating_duration_since(ts) > ttl {
                    let app_active = sess.app_tx.lock().await.is_some();
                    let wallet_active = sess.wallet_tx.lock().await.is_some();
                    if !app_active && !wallet_active {
                        remove_keys.push(k.clone());
                    }
                }
            }
        }
        if !remove_keys.is_empty() {
            let mut map_write = self.inner.write().await;
            for k in &remove_keys {
                map_write.remove(k);
            }
        }
        remove_keys.len()
    }

    /// Pre-handshake gate: enforce global/per-IP session caps and handshake rate.
    pub async fn pre_ws_handshake(
        &self,
        ip: IpAddr,
    ) -> Result<WsPermit, (axum::http::StatusCode, String)> {
        self.reserve_ws_slot(ip).await?;
        if let Err(err) = self.consume_handshake_token(ip).await {
            self.session_closed(ip).await;
            return Err(err);
        }
        Ok(WsPermit {
            bus: self.clone(),
            ip,
            released: false,
        })
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

    async fn reserve_ws_slot(&self, ip: IpAddr) -> Result<(), (axum::http::StatusCode, String)> {
        if self.policy.ws_max_sessions == 0 {
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: global session cap".into(),
            ));
        }
        let prev = self.shared.sessions_total.fetch_add(1, Ordering::AcqRel);
        if prev >= self.policy.ws_max_sessions {
            self.shared.sessions_total.fetch_sub(1, Ordering::AcqRel);
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: global session cap".into(),
            ));
        }

        let mut counts = self.per_ip_counts.lock().await;
        let entry = counts.entry(ip).or_insert(0);
        *entry += 1;
        if self.policy.ws_per_ip_max_sessions > 0 && *entry > self.policy.ws_per_ip_max_sessions {
            *entry -= 1;
            if *entry == 0 {
                counts.remove(&ip);
            }
            self.shared.sessions_total.fetch_sub(1, Ordering::AcqRel);
            return Err((
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "connect: per-ip session cap".into(),
            ));
        }
        Ok(())
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
        self.shared.sessions_total.fetch_add(1, Ordering::Relaxed);
        let mut counts = self.per_ip_counts.lock().await;
        *counts.entry(ip).or_insert(0) += 1;
    }

    /// Record a closed WS session.
    pub async fn session_closed(&self, ip: IpAddr) {
        self.shared.sessions_total.fetch_sub(1, Ordering::Relaxed);
        let mut counts = self.per_ip_counts.lock().await;
        if let Some(v) = counts.get_mut(&ip) {
            if *v > 0 {
                *v -= 1;
            }
            if *v == 0 {
                counts.remove(&ip);
            }
        }
    }

    // Use derived Clone

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
    #[allow(dead_code)]
    pub async fn register_tokens(
        &self,
        sid: Sid,
        token_app: String,
        token_wallet: String,
    ) -> Result<(), RegisterSessionError> {
        {
            let map = self.inner.read().await;
            if map.contains_key(&sid.to_vec()) {
                return Err(RegisterSessionError::Exists);
            }
            if map.len() >= self.policy.ws_max_sessions {
                return Err(RegisterSessionError::Capacity);
            }
        }

        let sess = Arc::new(Session::default());
        *sess.app_token.lock().await = Some(token_app);
        *sess.wallet_token.lock().await = Some(token_wallet);
        *sess.last_activity.lock().await = Instant::now();

        let mut map = self.inner.write().await;
        map.insert(sid.to_vec(), sess);
        Ok(())
    }

    /// Authorize WS attach by consuming a one-time token for the given role.
    pub async fn authorize_token(
        &self,
        sid: Sid,
        role: proto::Role,
        token: &str,
    ) -> Result<(), (axum::http::StatusCode, String)> {
        let Some(sess) = self.inner.read().await.get(&sid.to_vec()).cloned() else {
            iroha_logger::debug!(
                sid = %hex::encode(sid),
                "connect: authorize_token rejected unknown sid"
            );
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "connect: unknown sid".into(),
            ));
        };
        let ok = match role {
            proto::Role::App => {
                let mut t = sess.app_token.lock().await;
                t.as_ref()
                    .is_some_and(|s| s == token)
                    .then(|| *t = None)
                    .is_some()
            }
            proto::Role::Wallet => {
                let mut t = sess.wallet_token.lock().await;
                t.as_ref()
                    .is_some_and(|s| s == token)
                    .then(|| *t = None)
                    .is_some()
            }
        };
        if ok {
            Ok(())
        } else {
            iroha_logger::debug!(
                sid = %hex::encode(sid),
                role = ?role,
                "connect: authorize_token rejected bad token"
            );
            Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "connect: bad token".into(),
            ))
        }
    }

    async fn detach_if_empty(&self, sid: &Sid) {
        let key = sid.to_vec();
        let mut w = self.inner.write().await;
        if let Some(sess) = w.get(&key) {
            let app_empty = sess.app_tx.lock().await.is_none();
            let wallet_empty = sess.wallet_tx.lock().await.is_none();
            if app_empty && wallet_empty {
                w.remove(&key);
            }
        }
    }

    async fn attach(&self, sid: Sid, role: proto::Role) -> mpsc::Receiver<proto::ConnectFrameV1> {
        let sess = self.get_or_create(&sid).await;
        let (tx, rx) = mpsc::channel::<proto::ConnectFrameV1>(64);
        match role {
            proto::Role::App => {
                *sess.app_tx.lock().await = Some(tx);
            }
            proto::Role::Wallet => {
                *sess.wallet_tx.lock().await = Some(tx);
            }
        }
        // Drain any buffered frames targeted to this role upon attach
        Self::drain_for_role(sess.clone(), role).await;
        rx
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
        let sess = {
            let mut map = self.inner.write().await;
            map.remove(&sid.to_vec())
        };
        if let Some(sess) = sess {
            // Clear tokens so new WS joins cannot reuse them.
            *sess.app_token.lock().await = None;
            *sess.wallet_token.lock().await = None;

            self.notify_close(sess.clone(), sid, proto::Role::Wallet, reason)
                .await;
            self.notify_close(sess, sid, proto::Role::App, reason).await;
            true
        } else {
            false
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
            if tx.send(frame).await.is_ok() {
                *sess.last_activity.lock().await = Instant::now();
            }
        }
    }

    async fn deliver_local_only(&self, frame: &proto::ConnectFrameV1) -> bool {
        let sid_key = frame.sid.to_vec();
        let target = match frame.dir {
            proto::Dir::AppToWallet => proto::Role::Wallet,
            proto::Dir::WalletToApp => proto::Role::App,
        };
        if let Some(sess) = self.inner.read().await.get(&sid_key) {
            let tx_opt = match target {
                proto::Role::App => sess.app_tx.lock().await.clone(),
                proto::Role::Wallet => sess.wallet_tx.lock().await.clone(),
            };
            if let Some(tx) = tx_opt {
                let _ = tx.send(frame.clone()).await;
                return true;
            }
        }
        false
    }

    async fn relay(&self, frame: proto::ConnectFrameV1) {
        let Some(sess) = self.inner.read().await.get(&frame.sid.to_vec()).cloned() else {
            debug!(sid = ?hex::encode(frame.sid), "connect: dropping frame for unknown session");
            return;
        };

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
            self.terminate_session(frame.sid, CLOSE_REASON_SEQUENCE_VIOLATION)
                .await;
            return;
        }
        // If control frame, capture permissions for diagnostics
        if let proto::FrameKind::Control(ctrl) = &frame.kind {
            match ctrl {
                proto::ConnectControlV1::Open { permissions, .. } => {
                    if frame.dir == proto::Dir::AppToWallet {
                        (*sess.req_perms.lock().await).clone_from(permissions);
                        if permissions.is_some() {
                            debug!(sid = ?hex::encode(frame.sid), "connect: Open with permissions requested");
                        }
                    }
                }
                proto::ConnectControlV1::Approve { permissions, .. } => {
                    if frame.dir == proto::Dir::WalletToApp {
                        (*sess.acc_perms.lock().await).clone_from(permissions);
                        *sess.approved.lock().await = true;
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
                }
                proto::ConnectControlV1::Close { .. } | proto::ConnectControlV1::Reject { .. } => {
                    if *sess.approved.lock().await {
                        self.shared
                            .plaintext_control_drops_total
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(sid = ?hex::encode(frame.sid), "connect: dropping plaintext Close/Reject after approval");
                        return;
                    }
                }
                _ => {}
            }
        }

        // Deliver locally (best effort)
        let delivered = self.deliver_local_only(&frame).await;
        if delivered {
            self.shared.frames_out_total.fetch_add(1, Ordering::Relaxed);
        } else {
            // Buffer frame for the session if target is offline
            let cap = self.policy.session_buffer_max_bytes;
            {
                let mut buf = sess.buffer.lock().await;
                let mut bytes = sess.buffer_bytes.lock().await;
                let mut evicted = 0u64;
                // Evict oldest until it fits under cap
                while *bytes + enc_len > cap {
                    if let Some((_old, old_len)) = buf.pop_front() {
                        *bytes = bytes.saturating_sub(old_len);
                        evicted += 1;
                    } else {
                        break;
                    }
                }
                if evicted > 0 {
                    self.shared
                        .buffer_drops_total
                        .fetch_add(evicted, Ordering::Relaxed);
                }
                if *bytes + enc_len <= cap {
                    buf.push_back((frame.clone(), enc_len));
                    *bytes += enc_len;
                }
            }
            *sess.last_activity.lock().await = Instant::now();
        }
        // Re-broadcast to peers (best effort). Multi-hop flood with dedupe.
        if self.policy.relay_enabled && self.policy.relay_strategy == RelayStrategy::Broadcast {
            if let Some(net) = self.p2p.read().await.as_ref() {
                self.shared
                    .p2p_rebroadcasts_total
                    .fetch_add(1, Ordering::Relaxed);
                net.broadcast(iroha_p2p::Broadcast {
                    data: corelib::NetworkMessage::Connect(Box::new(frame)),
                    priority: iroha_p2p::Priority::Low,
                });
            } else {
                self.shared
                    .p2p_rebroadcast_skipped_total
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn relay_from_role(&self, role: proto::Role, frame: proto::ConnectFrameV1) -> bool {
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
            self.terminate_session(frame.sid, CLOSE_REASON_ROLE_DIRECTION_MISMATCH)
                .await;
            return false;
        }

        self.relay(frame).await;
        true
    }

    async fn touch_session(&self, sid: &Sid) {
        if let Some(sess) = self.inner.read().await.get(&sid.to_vec()) {
            *sess.last_activity.lock().await = Instant::now();
        }
    }

    async fn drain_for_role(sess: Arc<Session>, role: proto::Role) {
        let mut out = Vec::new();
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
                    out.push(f);
                    *bytes = bytes.saturating_sub(l);
                } else {
                    kept.push_back((f, l));
                }
            }
            *buf = kept;
        }
        // Deliver drained frames
        let tx_opt = match role {
            proto::Role::App => sess.app_tx.lock().await.clone(),
            proto::Role::Wallet => sess.wallet_tx.lock().await.clone(),
        };
        if let Some(tx) = tx_opt {
            for f in out {
                let _ = tx.send(f).await;
            }
            *sess.last_activity.lock().await = Instant::now();
        }
    }

    async fn evaluate_heartbeat(
        &self,
        sid: &Sid,
        role: proto::Role,
        now: Instant,
    ) -> Option<HeartbeatFailure> {
        let tolerance = usize::try_from(self.policy.heartbeat_miss_tolerance).unwrap_or(usize::MAX);
        if tolerance == 0 {
            return None;
        }
        let session = {
            let map = self.inner.read().await;
            map.get(&sid.to_vec()).cloned()
        };
        let sess = session?;
        let queue = sess.heartbeat_queue(role).await;
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
pub async fn handle_ws(
    bus: Bus,
    q: crate::routing::ConnectWsQuery,
    ws: WebSocket,
) -> Result<(), String> {
    use tokio::task::JoinSet;

    let sid = decode_sid(&q.sid).map_err(|e| format!("bad sid: {e}"))?;
    let role = match q.role.as_str() {
        "app" | "App" => proto::Role::App,
        "wallet" | "Wallet" => proto::Role::Wallet,
        other => return Err(format!("bad role: {other}")),
    };

    let mut inbox = bus.attach(sid, role).await;

    let mut tasks = JoinSet::new();

    // Split WS into sender and receiver halves
    let (mut ws_sender, mut ws_receiver) = ws.split();
    // Writer: forward frames from Bus to WS
    let sid_for_writer = sid;
    let bus_for_writer = bus.clone();
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
                            match proto::encode_connect_frame_bare(&frame) {
                                Ok(bytes) => {
                                    if let Err(e) = ws_sender
                                        .send(Message::Binary(axum::body::Bytes::from(bytes)))
                                        .await
                                    {
                                        return Err(format!("ws send failed: {e}"));
                                    }
                                    bus_for_writer.touch_session(&sid_for_writer).await;
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
                        .session_expired(&sid_for_writer, Instant::now())
                        .await;
                    if expired {
                        // Best-effort Close
                        let _ = ws_sender
                            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                code: CLOSE_CODE_TTL,
                                reason: Utf8Bytes::from(CLOSE_REASON_TTL.to_string()),
                            })))
                            .await;
                        break;
                    }
                    if let Some(failure) = bus_for_writer
                        .evaluate_heartbeat(&sid_for_writer, role_for_writer, Instant::now())
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
                        let _ = ws_sender
                            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                code: CLOSE_CODE_HEARTBEAT,
                                reason: Utf8Bytes::from(CLOSE_REASON_HEARTBEAT.to_string()),
                            })))
                            .await;
                        break;
                    }
                }
            }
        }
        Ok::<(), String>(())
    };
    tasks.spawn(writer);

    // Reader: parse binary frames and forward.
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Binary(b)) => {
                bus.touch_session(&sid).await;
                if b.len() > bus.policy.frame_max_bytes {
                    break;
                }
                match proto::decode_connect_frame_bare(&b) {
                    Ok(frame) => {
                        if frame.sid != sid {
                            break;
                        }
                        if !bus.relay_from_role(role, frame).await {
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
                // Ignore text frames
            }
            Err(e) => {
                return Err(format!("ws error: {e}"));
            }
            _ => {}
        }
    }

    bus.detach(sid, role).await;
    // Drain writer task
    while let Some(_r) = tasks.join_next().await {}
    Ok(())
}

fn expected_direction_for_role(role: proto::Role) -> proto::Dir {
    match role {
        proto::Role::App => proto::Dir::AppToWallet,
        proto::Role::Wallet => proto::Dir::WalletToApp,
    }
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
    use std::{collections::BTreeMap, num::NonZeroU64};

    use base64::Engine as _;
    use iroha_crypto::Hash;
    use tokio::time::{Duration, timeout};

    use super::*;

    #[tokio::test]
    async fn register_tokens_rejects_duplicate_sid() {
        let bus = Bus::new();
        let sid = [0x11u8; 32];
        bus.register_tokens(sid, "t-app".into(), "t-wallet".into())
            .await
            .expect("first registration succeeds");
        let err = bus
            .register_tokens(sid, "t-app-2".into(), "t-wallet-2".into())
            .await
            .expect_err("duplicate sid should be rejected");
        assert_eq!(err, RegisterSessionError::Exists);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
        let sid = [0xABu8; 32];
        bus.register_tokens(sid, "t-app".into(), "t-wallet".into())
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
    async fn terminate_session_sends_close_frames() {
        let bus = Bus::new();
        let sid = [0x42u8; 32];
        bus.register_tokens(sid, "app-token".into(), "wallet-token".into())
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus_primary = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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

    #[tokio::test]
    async fn unsupported_relay_strategy_forces_local_only() {
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
            relay_strategy: "bogus_strategy",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
        let status = bus.status().await;
        assert_eq!(status.policy.relay_strategy, "local_only");
        assert_eq!(status.policy.relay_effective_strategy, "local_only");
        assert!(!status.policy.relay_p2p_attached);
    }

    #[tokio::test]
    async fn relay_strategy_aliases_normalize_to_local_only() {
        for relay_strategy in ["local_only", "local-only", "local"] {
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
                relay_strategy,
                p2p_ttl_hops: 0,
            };
            let bus = Bus::from_config(&cfg);
            let status = bus.status().await;
            assert_eq!(status.policy.relay_strategy, "local_only");
            assert_eq!(status.policy.relay_effective_strategy, "local_only");
            assert!(!status.policy.relay_p2p_attached);
        }
    }

    #[tokio::test]
    async fn relay_strategy_parser_trims_and_lowercases() {
        let broadcast_cfg = iroha_config::parameters::actual::Connect {
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
            relay_strategy: "  BROADCAST  ",
            p2p_ttl_hops: 0,
        };
        let broadcast_bus = Bus::from_config(&broadcast_cfg);
        let broadcast_status = broadcast_bus.status().await;
        assert_eq!(broadcast_status.policy.relay_strategy, "broadcast");
        assert_eq!(
            broadcast_status.policy.relay_effective_strategy,
            "local_only"
        );
        assert!(!broadcast_status.policy.relay_p2p_attached);

        let local_cfg = iroha_config::parameters::actual::Connect {
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
            relay_strategy: "  LOCAL-ONLY  ",
            p2p_ttl_hops: 0,
        };
        let local_bus = Bus::from_config(&local_cfg);
        let local_status = local_bus.status().await;
        assert_eq!(local_status.policy.relay_strategy, "local_only");
        assert_eq!(local_status.policy.relay_effective_strategy, "local_only");
        assert!(!local_status.policy.relay_p2p_attached);
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
        {
            let mut p2p = bus.p2p.write().await;
            *p2p = Some(corelib::IrohaNetwork::closed_for_tests());
        }
        let sid = [0xB1u8; 32];
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            relay_strategy: "bogus_strategy",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
            "unsupported relay strategy should be forced to local_only"
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
            relay_strategy: "broadcast",
            p2p_ttl_hops: 0,
        };
        let bus = Bus::from_config(&cfg);
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
    async fn drops_plaintext_control_after_approve() {
        let bus = Bus::new();
        let sid = [6u8; 32];
        // Attach both sides
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;

        // Send Approve from wallet to app (seq=1)
        let approve = proto::ConnectFrameV1 {
            sid,
            dir: proto::Dir::WalletToApp,
            seq: 1,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Approve {
                wallet_pk: [1u8; 32],
                account_id: "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .into(),
                permissions: None,
                proof: None,
                sig_wallet: proto::WalletSignatureV1::new(
                    Algorithm::Ed25519,
                    Signature::from_bytes(&[0u8; 64]),
                ),
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
        let bus = Bus::new();
        let sid = [0xBCu8; 32];
        let mut app_inbox = bus.attach(sid, proto::Role::App).await;
        let mut wallet_inbox = bus.attach(sid, proto::Role::Wallet).await;

        let entry_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed([0x11u8; 32]));
        let entry_tree: MerkleTree<TransactionEntrypoint> = [entry_hash].into_iter().collect();
        let entry_root = entry_tree.root().expect("entry root");
        let entry_proof: BlockReceiptProof =
            BlockReceiptProof::new(entry_hash, entry_tree.get_proof(0).expect("entry proof"));
        let proofs = BlockProofs {
            block_height: NonZeroU64::new(1).expect("non-zero height"),
            entry_hash,
            entry_root,
            entry_proof,
            result_root: None,
            result_proof: None,
            fastpq_transcripts: BTreeMap::new(),
        };

        let expected_entry_hex = hex::encode(entry_hash.as_ref());
        let expected_json = norito::json::to_json(&proofs).expect("serialize proofs");

        bus.broadcast_block_proof(NonZeroU64::new(1).unwrap(), &entry_hash, &proofs)
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
    /// Attach a P2P network handle and spawn an inbound subscriber task to deliver
    /// incoming Connect frames to local WS endpoints.
    pub fn attach_network(&self, network: corelib::IrohaNetwork) {
        use iroha_p2p::network::{SubscriberFilter, message::Topic};

        let me = self.clone();
        tokio::spawn(async move {
            {
                let mut w = me.p2p.write().await;
                *w = Some(network.clone());
            }
            let (tx, mut rx) = tokio::sync::mpsc::channel(network.subscriber_queue_cap().get());
            let filter = SubscriberFilter::topics([Topic::Health]);
            let mut tx = tx;
            loop {
                match network.subscribe_to_peers_messages_with_filter(tx, filter.clone()) {
                    Ok(()) => break,
                    Err(returned) => {
                        iroha_logger::warn!("retrying Torii Connect relay subscription to P2P bus");
                        tx = returned;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
            while let Some(msg) = rx.recv().await {
                let payload = msg.payload;
                if let corelib::NetworkMessage::Connect(frame) = payload {
                    me.relay(*frame).await;
                }
            }
        });
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
        }
    }

    /// Broadcast a block proof payload to locally attached Connect peers.
    pub async fn broadcast_block_proof(
        &self,
        height: NonZeroU64,
        entry_hash: &HashOf<TransactionEntrypoint>,
        proofs: &BlockProofs,
    ) -> Result<(), norito::json::Error> {
        let proofs_json = norito::json::to_json(proofs)?;
        let event = proto::ServerEventV1::BlockProofs {
            height: height.get(),
            entry_hash: hex::encode(entry_hash.as_ref()),
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
        let seq = session.next_server_seq(dir).await;
        let frame = proto::ConnectFrameV1 {
            sid: sid
                .try_into()
                .unwrap_or_else(|_| panic!("connect sid wrong length: {}", sid.len())),
            dir,
            seq,
            kind: proto::FrameKind::Control(control.clone()),
        };
        let tx_opt = match target {
            proto::Role::App => session.app_tx.lock().await.clone(),
            proto::Role::Wallet => session.wallet_tx.lock().await.clone(),
        };
        if let Some(tx) = tx_opt {
            if tx.send(frame).await.is_ok() {
                *session.last_activity.lock().await = Instant::now();
            }
            self.shared.frames_out_total.fetch_add(1, Ordering::Relaxed);
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
        }
    }
}

impl Policy {
    fn effective_relay_strategy(self, relay_p2p_attached: bool) -> RelayStrategy {
        if !self.relay_enabled {
            return RelayStrategy::LocalOnly;
        }
        match self.relay_strategy {
            RelayStrategy::Broadcast if relay_p2p_attached => RelayStrategy::Broadcast,
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
    fn from_config(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "broadcast" => Self::Broadcast,
            "local_only" | "local-only" | "local" => Self::LocalOnly,
            other => {
                warn!(
                    relay_strategy = other,
                    "connect: unsupported relay strategy, forcing local-only mode"
                );
                Self::LocalOnly
            }
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
