//! Pending-RBC stash (chunks/ready/deliver seen before INIT or roster resolution).

use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

use eyre::Result;
use iroha_data_model::peer::PeerId;
use iroha_logger::prelude::*;

use super::Actor;
use crate::sumeragi::{
    consensus::{RbcChunk, RbcDeliver, RbcReady},
    rbc_store::SessionKey,
    status,
};

#[derive(Debug)]
pub(super) struct PendingRbcChunk {
    pub(super) chunk: RbcChunk,
    pub(super) sender: Option<PeerId>,
}

#[derive(Debug)]
pub(super) struct PendingRbcMessages {
    pub(super) chunks: VecDeque<PendingRbcChunk>,
    pub(super) ready: Vec<RbcReady>,
    pub(super) deliver: Vec<RbcDeliver>,
    pending_bytes: usize,
    dropped_chunks: u64,
    dropped_bytes: u64,
    dropped_ready: u64,
    dropped_deliver: u64,
    first_seen: Instant,
    last_seen: Instant,
}

#[derive(Debug)]
pub(super) enum PendingChunkOutcome {
    Inserted {
        pending_chunks: usize,
        pending_bytes: usize,
        evicted_chunks: u64,
        evicted_bytes: u64,
        evicted: Vec<PendingRbcChunk>,
    },
    Dropped {
        dropped_bytes: u64,
        evicted_chunks: u64,
        evicted_bytes: u64,
        evicted: Vec<PendingRbcChunk>,
    },
}

impl PendingRbcMessages {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            chunks: VecDeque::new(),
            ready: Vec::new(),
            deliver: Vec::new(),
            pending_bytes: 0,
            dropped_chunks: 0,
            dropped_bytes: 0,
            dropped_ready: 0,
            dropped_deliver: 0,
            first_seen: now,
            last_seen: now,
        }
    }

    pub(super) fn touch(&mut self, now: Instant) {
        // Extend the pending TTL on new pre-INIT traffic to retain payload evidence.
        self.last_seen = now;
    }

    pub(super) fn pending_bytes(&self) -> usize {
        self.pending_bytes
    }

    pub(super) fn pending_chunks(&self) -> usize {
        self.chunks.len()
    }

    #[allow(dead_code)]
    pub(super) fn dropped_counts(&self) -> (u64, u64) {
        (self.dropped_chunks, self.dropped_bytes)
    }

    pub(super) fn drop_breakdown(&self) -> (u64, u64, u64, u64) {
        (
            self.dropped_chunks,
            self.dropped_ready,
            self.dropped_deliver,
            self.dropped_bytes,
        )
    }

    pub(super) fn age_ms(&self, now: Instant) -> u64 {
        u64::try_from(
            now.saturating_duration_since(self.first_seen)
                .as_millis()
                .min(u128::from(u64::MAX)),
        )
        .unwrap_or(u64::MAX)
    }

    pub(super) fn first_seen(&self) -> Instant {
        self.first_seen
    }

    pub(super) fn expired(&self, ttl: Duration, now: Instant) -> bool {
        ttl > Duration::ZERO && now.saturating_duration_since(self.last_seen) > ttl
    }

    fn record_drop(&mut self, bytes: usize, now: Instant) {
        self.dropped_chunks = self.dropped_chunks.saturating_add(1);
        self.dropped_bytes = self
            .dropped_bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
        self.touch(now);
    }

    fn record_ready_drop(&mut self, bytes: usize, now: Instant) {
        self.dropped_ready = self.dropped_ready.saturating_add(1);
        self.dropped_bytes = self
            .dropped_bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
        self.touch(now);
    }

    fn record_deliver_drop(&mut self, bytes: usize, now: Instant) {
        self.dropped_deliver = self.dropped_deliver.saturating_add(1);
        self.dropped_bytes = self
            .dropped_bytes
            .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
        self.touch(now);
    }

    /// Attempts to stash a chunk respecting per-session caps.
    pub(super) fn push_chunk_capped(
        &mut self,
        chunk: RbcChunk,
        sender: Option<PeerId>,
        max_chunks: usize,
        max_bytes: usize,
        now: Instant,
    ) -> PendingChunkOutcome {
        if max_chunks == 0 || max_bytes == 0 {
            self.record_drop(chunk.bytes.len(), now);
            return PendingChunkOutcome::Dropped {
                evicted_chunks: 0,
                evicted_bytes: 0,
                evicted: Vec::new(),
                dropped_bytes: u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX),
            };
        }

        let mut evicted_chunks = 0u64;
        let mut evicted_bytes = 0u64;
        let mut evicted_entries = Vec::new();
        let chunk_len = chunk.bytes.len();
        while (self.chunks.len().saturating_add(1) > max_chunks)
            || pending_bytes_would_exceed(self.pending_bytes, chunk_len, max_bytes)
        {
            if let Some(evicted) = self.chunks.pop_front() {
                evicted_chunks = evicted_chunks.saturating_add(1);
                let evicted_len = evicted.chunk.bytes.len();
                evicted_bytes =
                    evicted_bytes.saturating_add(u64::try_from(evicted_len).unwrap_or(u64::MAX));
                self.pending_bytes = self.pending_bytes.saturating_sub(evicted_len);
                self.record_drop(evicted_len, now);
                evicted_entries.push(evicted);
            } else {
                break;
            }
        }

        let would_exceed_chunks = self.chunks.len().saturating_add(1) > max_chunks;
        let would_exceed_bytes =
            pending_bytes_would_exceed(self.pending_bytes, chunk_len, max_bytes);
        if would_exceed_chunks || would_exceed_bytes {
            let dropped_bytes = u64::try_from(chunk_len).unwrap_or(u64::MAX);
            self.record_drop(chunk_len, now);
            return PendingChunkOutcome::Dropped {
                evicted_chunks,
                evicted_bytes,
                evicted: evicted_entries,
                dropped_bytes,
            };
        }

        self.touch(now);
        self.pending_bytes = self.pending_bytes.saturating_add(chunk_len);
        self.chunks.push_back(PendingRbcChunk { chunk, sender });
        PendingChunkOutcome::Inserted {
            pending_chunks: self.chunks.len(),
            pending_bytes: self.pending_bytes,
            evicted_chunks,
            evicted_bytes,
            evicted: evicted_entries,
        }
    }

    pub(super) fn push_ready_capped(
        &mut self,
        ready: RbcReady,
        max_bytes: usize,
        now: Instant,
    ) -> (bool, usize) {
        let size = rbc_ready_stash_bytes(&ready);
        if size == 0 || pending_bytes_would_exceed(self.pending_bytes, size, max_bytes) {
            if size > 0 {
                self.record_ready_drop(size, now);
            }
            return (false, size);
        }
        self.touch(now);
        self.pending_bytes = self.pending_bytes.saturating_add(size);
        self.ready.push(ready);
        (true, 0)
    }

    pub(super) fn push_deliver_capped(
        &mut self,
        deliver: RbcDeliver,
        max_bytes: usize,
        now: Instant,
    ) -> (bool, usize) {
        let size = rbc_deliver_stash_bytes(&deliver);
        if size == 0 || pending_bytes_would_exceed(self.pending_bytes, size, max_bytes) {
            if size > 0 {
                self.record_deliver_drop(size, now);
            }
            return (false, size);
        }
        self.touch(now);
        self.pending_bytes = self.pending_bytes.saturating_add(size);
        self.deliver.push(deliver);
        (true, 0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PendingRbcDropReason {
    Cap,
    Ttl,
    SessionLimit,
}

impl PendingRbcDropReason {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Cap => "cap",
            Self::SessionLimit => "session_cap",
            Self::Ttl => "ttl",
        }
    }
}

pub(super) struct PendingRbcEviction {
    pub(super) key: SessionKey,
    pub(super) reason: PendingRbcDropReason,
    pub(super) removed: PendingRbcMessages,
}

impl core::fmt::Debug for PendingRbcEviction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PendingRbcEviction")
            .field("key", &self.key)
            .field("reason", &self.reason.as_str())
            .field("pending_chunks", &self.removed.pending_chunks())
            .field("pending_bytes", &self.removed.pending_bytes())
            .finish()
    }
}

impl Actor {
    pub(super) fn apply_pending_rbc_housekeeping(
        pending: &mut BTreeMap<SessionKey, PendingRbcMessages>,
        active_sessions: Option<&BTreeMap<SessionKey, super::RbcSession>>,
        key: SessionKey,
        session_cap: usize,
        ttl: Duration,
        now: Instant,
    ) -> Vec<PendingRbcEviction> {
        let mut evictions = Vec::new();
        if ttl > Duration::ZERO && !pending.is_empty() {
            let expired: Vec<_> = pending
                .iter()
                .filter(|(session_key, entry)| {
                    if let Some(sessions) = active_sessions {
                        if sessions.contains_key(session_key) {
                            return false;
                        }
                    }
                    entry.expired(ttl, now)
                })
                .map(|(session_key, _)| *session_key)
                .collect();
            for session_key in expired {
                if let Some(removed) = pending.remove(&session_key) {
                    evictions.push(PendingRbcEviction {
                        key: session_key,
                        reason: PendingRbcDropReason::Ttl,
                        removed,
                    });
                }
            }
        }

        if session_cap > 0 && pending.len() >= session_cap && !pending.contains_key(&key) {
            let oldest = pending
                .iter()
                .filter(|(session_key, _)| {
                    active_sessions.is_none_or(|sessions| !sessions.contains_key(session_key))
                })
                .min_by_key(|(_, entry)| entry.first_seen())
                .map(|(session_key, _)| *session_key);
            if let Some(oldest) = oldest {
                if let Some(removed) = pending.remove(&oldest) {
                    evictions.push(PendingRbcEviction {
                        key: oldest,
                        reason: PendingRbcDropReason::SessionLimit,
                        removed,
                    });
                }
            }
        }

        evictions
    }

    #[allow(clippy::type_complexity)]
    #[allow(dead_code)]
    pub(super) fn take_pending_rbc_slot<'a>(
        pending: &'a mut BTreeMap<SessionKey, PendingRbcMessages>,
        active_sessions: Option<&BTreeMap<SessionKey, super::RbcSession>>,
        key: SessionKey,
        session_cap: usize,
        ttl: Duration,
        now: Instant,
    ) -> (Option<&'a mut PendingRbcMessages>, Vec<PendingRbcEviction>) {
        let evictions = Self::apply_pending_rbc_housekeeping(
            pending,
            active_sessions,
            key,
            session_cap,
            ttl,
            now,
        );
        if session_cap > 0 && !pending.contains_key(&key) && pending.len() >= session_cap {
            return (None, evictions);
        }
        let pending_slot = pending
            .entry(key)
            .or_insert_with(|| PendingRbcMessages::new(now));
        pending_slot.touch(now);
        (Some(pending_slot), evictions)
    }

    pub(super) fn pending_rbc_slot(&mut self, key: SessionKey) -> Option<&mut PendingRbcMessages> {
        let now = Instant::now();
        let ttl = self.config.rbc.pending_ttl;
        let session_cap = self.config.rbc.pending_session_limit;
        let evictions = Self::apply_pending_rbc_housekeeping(
            &mut self.subsystems.da_rbc.rbc.pending,
            Some(&self.subsystems.da_rbc.rbc.sessions),
            key,
            session_cap,
            ttl,
            now,
        );

        if !evictions.is_empty() {
            for eviction in evictions {
                match eviction.reason {
                    PendingRbcDropReason::SessionLimit => warn!(
                        ?eviction.key,
                        limit = session_cap,
                        pending_chunks = eviction.removed.pending_chunks(),
                        pending_bytes = eviction.removed.pending_bytes(),
                        "dropping oldest pending RBC stash to enforce limit"
                    ),
                    PendingRbcDropReason::Ttl => warn!(
                        ?eviction.key,
                        ttl_ms = ttl.as_millis(),
                        pending_chunks = eviction.removed.pending_chunks(),
                        pending_bytes = eviction.removed.pending_bytes(),
                        "evicting pending RBC stash after TTL elapsed without INIT"
                    ),
                    PendingRbcDropReason::Cap => {}
                }
                self.release_pending_rbc_dedup(&eviction.removed);
                Self::record_pending_drop(
                    self.telemetry_handle(),
                    eviction.reason,
                    &eviction.removed,
                );
                self.request_missing_block_after_rbc_drop(
                    eviction.key,
                    eviction.reason,
                    "pending_rbc_eviction",
                );
            }
            self.publish_rbc_backlog_snapshot();
        }

        if session_cap > 0
            && !self.subsystems.da_rbc.rbc.pending.contains_key(&key)
            && self.subsystems.da_rbc.rbc.pending.len() >= session_cap
        {
            return None;
        }
        let pending_entry = self
            .subsystems
            .da_rbc
            .rbc
            .pending
            .entry(key)
            .or_insert_with(|| PendingRbcMessages::new(now));
        pending_entry.touch(now);
        Some(pending_entry)
    }

    pub(super) fn prune_expired_pending_rbc(&mut self) -> bool {
        let ttl = self.config.rbc.pending_ttl;
        if ttl == Duration::ZERO || self.subsystems.da_rbc.rbc.pending.is_empty() {
            return false;
        }
        let housekeeping_key = self
            .subsystems
            .da_rbc
            .rbc
            .pending
            .keys()
            .next()
            .copied()
            .expect("pending RBC map checked as non-empty");
        let evictions = Self::apply_pending_rbc_housekeeping(
            &mut self.subsystems.da_rbc.rbc.pending,
            Some(&self.subsystems.da_rbc.rbc.sessions),
            housekeeping_key,
            0,
            ttl,
            Instant::now(),
        );
        if evictions.is_empty() {
            return false;
        }

        for eviction in evictions {
            warn!(
                ?eviction.key,
                ttl_ms = ttl.as_millis(),
                pending_chunks = eviction.removed.pending_chunks(),
                pending_bytes = eviction.removed.pending_bytes(),
                "evicting pending RBC stash after TTL elapsed without INIT"
            );
            self.release_pending_rbc_dedup(&eviction.removed);
            Self::record_pending_drop(self.telemetry_handle(), eviction.reason, &eviction.removed);
            self.request_missing_block_after_rbc_drop(
                eviction.key,
                eviction.reason,
                "pending_rbc_ttl_sweep",
            );
        }
        self.publish_rbc_backlog_snapshot();
        true
    }

    pub(super) fn clear_pending_rbc(&mut self, key: &SessionKey) {
        if let Some(pending) = self.subsystems.da_rbc.rbc.pending.remove(key) {
            self.release_pending_rbc_dedup(&pending);
            self.publish_rbc_backlog_snapshot();
        }
    }

    pub(super) fn clear_all_pending_rbc(&mut self) {
        let pending = std::mem::take(&mut self.subsystems.da_rbc.rbc.pending);
        let had_pending = !pending.is_empty();
        for pending in pending.into_values() {
            self.release_pending_rbc_dedup(&pending);
        }
        if had_pending {
            self.publish_rbc_backlog_snapshot();
        }
    }

    pub(super) fn flush_pending_rbc(&mut self, key: SessionKey) -> Result<()> {
        let Some(pending) = self.subsystems.da_rbc.rbc.pending.remove(&key) else {
            return Ok(());
        };
        self.release_pending_rbc_dedup(&pending);
        self.publish_rbc_backlog_snapshot();

        for entry in pending.chunks {
            self.handle_rbc_chunk(entry.chunk, entry.sender)?;
        }
        for ready in pending.ready {
            self.handle_rbc_ready(ready)?;
        }
        for deliver in pending.deliver {
            self.handle_rbc_deliver(deliver)?;
        }

        Ok(())
    }

    fn record_pending_drop(
        telemetry: Option<&crate::telemetry::Telemetry>,
        reason: PendingRbcDropReason,
        removed: &PendingRbcMessages,
    ) {
        let dropped_frames = removed
            .pending_chunks()
            .saturating_add(removed.ready.len())
            .saturating_add(removed.deliver.len());
        let dropped_chunks = u64::try_from(dropped_frames).unwrap_or(u64::MAX);
        let dropped_bytes = u64::try_from(removed.pending_bytes()).unwrap_or(u64::MAX);
        Self::record_pending_drop_counts(telemetry, reason, dropped_chunks, dropped_bytes);
        status::inc_pending_rbc_evicted(1);
        if let Some(telemetry) = telemetry {
            telemetry.inc_rbc_pending_evicted(1);
        }
    }

    pub(super) fn record_pending_drop_counts(
        telemetry: Option<&crate::telemetry::Telemetry>,
        reason: PendingRbcDropReason,
        dropped_chunks: u64,
        dropped_bytes: u64,
    ) {
        status::inc_rbc_pending_drop(reason.as_str(), dropped_chunks, dropped_bytes);
        if let Some(telemetry) = telemetry {
            telemetry.inc_rbc_pending_drop(reason.as_str(), dropped_chunks, dropped_bytes);
        }
    }
}

pub(super) fn rbc_ready_stash_bytes(ready: &RbcReady) -> usize {
    ready
        .signature
        .len()
        .saturating_add(ready.roster_hash.as_ref().len())
        .saturating_add(ready.chunk_root.as_ref().len())
        .saturating_add(ready.block_hash.as_ref().as_ref().len())
        .saturating_add(std::mem::size_of::<u64>() * 3)
        .saturating_add(std::mem::size_of::<u32>())
}

pub(super) fn rbc_deliver_stash_bytes(deliver: &RbcDeliver) -> usize {
    let ready_bytes = deliver
        .ready_signatures
        .iter()
        .map(|entry| rbc_ready_signature_stash_bytes(entry.signature.len()))
        .fold(0usize, usize::saturating_add);
    deliver
        .signature
        .len()
        .saturating_add(ready_bytes)
        .saturating_add(deliver.roster_hash.as_ref().len())
        .saturating_add(deliver.chunk_root.as_ref().len())
        .saturating_add(deliver.block_hash.as_ref().as_ref().len())
        .saturating_add(std::mem::size_of::<u64>() * 3)
        .saturating_add(std::mem::size_of::<u32>())
}

fn rbc_ready_signature_stash_bytes(signature_len: usize) -> usize {
    std::mem::size_of::<u32>().saturating_add(signature_len)
}

fn pending_bytes_would_exceed(current: usize, added: usize, max_bytes: usize) -> bool {
    current
        .checked_add(added)
        .is_none_or(|total| total > max_bytes)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::{BlockHeader, consensus::RbcReadySignature};

    use super::{
        Actor, PendingRbcMessages, rbc_deliver_stash_bytes, rbc_ready_signature_stash_bytes,
        rbc_ready_stash_bytes,
    };
    use crate::sumeragi::consensus::{RbcChunk, RbcDeliver, RbcReady};
    use crate::sumeragi::{main_loop::RbcSession, rbc_store::SessionKey};

    fn sample_block_hash(tag: &[u8]) -> HashOf<BlockHeader> {
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(tag))
    }

    fn sample_ready(signature_len: usize) -> RbcReady {
        RbcReady {
            block_hash: sample_block_hash(b"pending-rbc-ready"),
            height: 7,
            view: 2,
            epoch: 3,
            roster_hash: Hash::new(b"pending-rbc-ready-roster"),
            chunk_root: Hash::new(b"pending-rbc-ready-root"),
            sender: 1,
            signature: vec![0xA5; signature_len],
        }
    }

    fn sample_deliver(signature_len: usize, ready_signature_len: usize) -> RbcDeliver {
        RbcDeliver {
            block_hash: sample_block_hash(b"pending-rbc-deliver"),
            height: 8,
            view: 3,
            epoch: 4,
            roster_hash: Hash::new(b"pending-rbc-deliver-roster"),
            chunk_root: Hash::new(b"pending-rbc-deliver-root"),
            sender: 2,
            signature: vec![0xB6; signature_len],
            ready_signatures: vec![RbcReadySignature {
                sender: 1,
                signature: vec![0xC7; ready_signature_len],
            }],
        }
    }

    #[test]
    fn take_pending_rbc_slot_inserts_entry() {
        let key: SessionKey = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pending-rbc-slot")),
            1,
            2,
        );
        let mut pending = BTreeMap::new();
        let now = Instant::now();
        {
            let (slot, evictions) = Actor::take_pending_rbc_slot(
                &mut pending,
                None,
                key,
                4,
                Duration::from_secs(5),
                now,
            );
            assert!(evictions.is_empty());
            let slot = slot.expect("pending slot should be available");
            assert_eq!(slot.pending_chunks(), 0);
        }
        assert!(pending.contains_key(&key));
    }

    #[test]
    fn take_pending_rbc_slot_rejects_new_entry_when_cap_reached_by_active_sessions() {
        let key_a: SessionKey = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pending-rbc-slot-a")),
            1,
            1,
        );
        let key_b: SessionKey = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pending-rbc-slot-b")),
            2,
            1,
        );
        let key_c: SessionKey = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pending-rbc-slot-c")),
            3,
            1,
        );
        let mut pending = BTreeMap::new();
        let mut sessions = BTreeMap::new();
        let now = Instant::now();

        pending.insert(key_a, PendingRbcMessages::new(now));
        pending.insert(
            key_b,
            PendingRbcMessages::new(now + Duration::from_millis(1)),
        );
        sessions.insert(key_a, RbcSession::test_new(1, None, None, 0));
        sessions.insert(key_b, RbcSession::test_new(1, None, None, 0));

        let (slot, evictions) = Actor::take_pending_rbc_slot(
            &mut pending,
            Some(&sessions),
            key_c,
            2,
            Duration::from_secs(1),
            now + Duration::from_millis(2),
        );

        assert!(slot.is_none());
        assert!(evictions.is_empty());
        assert_eq!(pending.len(), 2);
        assert!(pending.contains_key(&key_a));
        assert!(pending.contains_key(&key_b));
    }

    #[test]
    fn push_ready_capped_accounts_and_drops_oversized_ready() {
        let now = Instant::now();
        let mut pending = PendingRbcMessages::new(now);
        let accepted = sample_ready(16);
        let accepted_size = rbc_ready_stash_bytes(&accepted);

        let (inserted, dropped_bytes) =
            pending.push_ready_capped(accepted.clone(), accepted_size, now);

        assert!(inserted);
        assert_eq!(dropped_bytes, 0);
        assert_eq!(pending.ready, vec![accepted]);
        assert_eq!(pending.pending_bytes(), accepted_size);
        assert_eq!(pending.drop_breakdown(), (0, 0, 0, 0));

        let oversized = sample_ready(32);
        let oversized_size = rbc_ready_stash_bytes(&oversized);
        let (inserted, dropped_bytes) =
            pending.push_ready_capped(oversized, accepted_size, now + Duration::from_millis(1));

        assert!(!inserted);
        assert_eq!(dropped_bytes, oversized_size);
        assert_eq!(pending.ready.len(), 1);
        assert_eq!(pending.pending_bytes(), accepted_size);
        assert_eq!(
            pending.drop_breakdown(),
            (0, 1, 0, u64::try_from(oversized_size).unwrap())
        );
    }

    #[test]
    fn push_ready_capped_rejects_pending_byte_counter_overflow() {
        let now = Instant::now();
        let mut pending = PendingRbcMessages::new(now);
        pending.pending_bytes = usize::MAX;

        let ready = sample_ready(16);
        let ready_size = rbc_ready_stash_bytes(&ready);
        let (inserted, dropped_bytes) = pending.push_ready_capped(ready, usize::MAX, now);

        assert!(!inserted);
        assert_eq!(dropped_bytes, ready_size);
        assert!(pending.ready.is_empty());
        assert_eq!(pending.pending_bytes(), usize::MAX);
        assert_eq!(
            pending.drop_breakdown(),
            (0, 1, 0, u64::try_from(ready_size).unwrap())
        );
    }

    #[test]
    fn push_deliver_capped_accounts_and_drops_oversized_deliver() {
        let now = Instant::now();
        let mut pending = PendingRbcMessages::new(now);
        let accepted = sample_deliver(16, 8);
        let accepted_size = rbc_deliver_stash_bytes(&accepted);

        let (inserted, dropped_bytes) =
            pending.push_deliver_capped(accepted.clone(), accepted_size, now);

        assert!(inserted);
        assert_eq!(dropped_bytes, 0);
        assert_eq!(pending.deliver, vec![accepted]);
        assert_eq!(pending.pending_bytes(), accepted_size);
        assert_eq!(pending.drop_breakdown(), (0, 0, 0, 0));

        let oversized = sample_deliver(32, 16);
        let oversized_size = rbc_deliver_stash_bytes(&oversized);
        let (inserted, dropped_bytes) =
            pending.push_deliver_capped(oversized, accepted_size, now + Duration::from_millis(1));

        assert!(!inserted);
        assert_eq!(dropped_bytes, oversized_size);
        assert_eq!(pending.deliver.len(), 1);
        assert_eq!(pending.pending_bytes(), accepted_size);
        assert_eq!(
            pending.drop_breakdown(),
            (0, 0, 1, u64::try_from(oversized_size).unwrap())
        );
    }

    #[test]
    fn push_deliver_capped_rejects_pending_byte_counter_overflow() {
        let now = Instant::now();
        let mut pending = PendingRbcMessages::new(now);
        pending.pending_bytes = usize::MAX;

        let deliver = sample_deliver(16, 8);
        let deliver_size = rbc_deliver_stash_bytes(&deliver);
        let (inserted, dropped_bytes) = pending.push_deliver_capped(deliver, usize::MAX, now);

        assert!(!inserted);
        assert_eq!(dropped_bytes, deliver_size);
        assert!(pending.deliver.is_empty());
        assert_eq!(pending.pending_bytes(), usize::MAX);
        assert_eq!(
            pending.drop_breakdown(),
            (0, 0, 1, u64::try_from(deliver_size).unwrap())
        );
    }

    #[test]
    fn push_chunk_capped_rejects_pending_byte_counter_overflow() {
        let now = Instant::now();
        let mut pending = PendingRbcMessages::new(now);
        pending.pending_bytes = usize::MAX;

        let chunk_bytes = vec![0xD0; 16];
        let chunk = RbcChunk {
            block_hash: sample_block_hash(b"pending-rbc-overflow-chunk"),
            height: 9,
            view: 4,
            epoch: 5,
            idx: 0,
            bytes: chunk_bytes.clone(),
        };
        let outcome = pending.push_chunk_capped(chunk, None, 4, usize::MAX, now);

        match outcome {
            super::PendingChunkOutcome::Dropped {
                dropped_bytes,
                evicted_chunks,
                evicted_bytes,
                evicted,
            } => {
                assert_eq!(dropped_bytes, u64::try_from(chunk_bytes.len()).unwrap());
                assert_eq!(evicted_chunks, 0);
                assert_eq!(evicted_bytes, 0);
                assert!(evicted.is_empty());
            }
            other => panic!("overflowed pending byte counter should reject chunk, got {other:?}"),
        }
        assert!(pending.chunks.is_empty());
        assert_eq!(pending.pending_bytes(), usize::MAX);
        assert_eq!(
            pending.drop_breakdown(),
            (1, 0, 0, u64::try_from(chunk_bytes.len()).unwrap())
        );
    }

    #[test]
    fn rbc_deliver_ready_signature_stash_bytes_saturates_without_wrapping() {
        let ready_bytes = [usize::MAX, 1]
            .into_iter()
            .map(rbc_ready_signature_stash_bytes)
            .fold(0usize, usize::saturating_add);

        assert_eq!(ready_bytes, usize::MAX);
    }
}
