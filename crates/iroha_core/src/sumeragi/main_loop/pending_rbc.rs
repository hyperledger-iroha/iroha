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
}

#[derive(Debug)]
pub(super) enum PendingChunkOutcome {
    Inserted {
        pending_chunks: usize,
        pending_bytes: usize,
        evicted_chunks: u64,
        evicted_bytes: u64,
    },
    Dropped {
        dropped_bytes: u64,
        evicted_chunks: u64,
        evicted_bytes: u64,
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
        }
    }

    #[allow(clippy::unused_self)]
    pub(super) fn touch(&mut self, _now: Instant) {
        // TTL is anchored to `first_seen` for pre-INIT stashes; active sessions bypass TTL.
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
        ttl > Duration::ZERO && now.saturating_duration_since(self.first_seen) > ttl
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
                dropped_bytes: u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX),
            };
        }

        let mut evicted_chunks = 0u64;
        let mut evicted_bytes = 0u64;
        let chunk_len = chunk.bytes.len();
        while (self.chunks.len().saturating_add(1) > max_chunks)
            || (self.pending_bytes.saturating_add(chunk_len) > max_bytes)
        {
            if let Some(evicted) = self.chunks.pop_front() {
                evicted_chunks = evicted_chunks.saturating_add(1);
                let evicted_len = evicted.chunk.bytes.len();
                evicted_bytes =
                    evicted_bytes.saturating_add(u64::try_from(evicted_len).unwrap_or(u64::MAX));
                self.pending_bytes = self.pending_bytes.saturating_sub(evicted_len);
                self.record_drop(evicted_len, now);
            } else {
                break;
            }
        }

        let would_exceed_chunks = self.chunks.len().saturating_add(1) > max_chunks;
        let would_exceed_bytes = self.pending_bytes.saturating_add(chunk_len) > max_bytes;
        if would_exceed_chunks || would_exceed_bytes {
            let dropped_bytes = u64::try_from(chunk_len).unwrap_or(u64::MAX);
            self.record_drop(chunk_len, now);
            return PendingChunkOutcome::Dropped {
                evicted_chunks,
                evicted_bytes,
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
        }
    }

    pub(super) fn push_ready_capped(
        &mut self,
        ready: RbcReady,
        max_bytes: usize,
        now: Instant,
    ) -> (bool, usize) {
        let size = rbc_ready_stash_bytes(&ready);
        if size == 0 || self.pending_bytes.saturating_add(size) > max_bytes {
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
        if size == 0 || self.pending_bytes.saturating_add(size) > max_bytes {
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
        let ttl = self.config.rbc_pending_ttl;
        let session_cap = PENDING_RBC_STASH_LIMIT;
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
                        limit = PENDING_RBC_STASH_LIMIT,
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

    pub(super) fn clear_pending_rbc(&mut self, key: &SessionKey) {
        self.subsystems.da_rbc.rbc.pending.remove(key);
    }

    pub(super) fn flush_pending_rbc(&mut self, key: SessionKey) -> Result<()> {
        let Some(pending) = self.subsystems.da_rbc.rbc.pending.remove(&key) else {
            return Ok(());
        };

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
        .map(|entry| std::mem::size_of::<u32>().saturating_add(entry.signature.len()))
        .sum::<usize>();
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

/// Hard cap on how many pending RBC stashes we buffer; active-session stashes are retained,
/// and new sessions are rejected once the cap is reached.
pub(super) const PENDING_RBC_STASH_LIMIT: usize = 256;

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;

    use super::{Actor, PendingRbcMessages};
    use crate::sumeragi::{main_loop::RbcSession, rbc_store::SessionKey};

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
}
