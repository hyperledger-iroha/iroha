//! Crash-safe local journal for lane-owned queue reservations.
//!
//! A reservation is local scheduling state rather than consensus state, but losing it can make
//! the same transaction eligible for both the global scheduler and an independently ticking lane.
//! The journal therefore uses checksummed, length-delimited frames and synchronizes every state
//! transition before the queue exposes it to callers. The first-release admission-bound layout is V5
//! only: earlier or unknown frame envelopes are retained and rejected without legacy decoding.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;
use iroha_data_model::nexus::LaneId;
use norito::codec::{Decode, Encode};

#[cfg(test)]
use super::LaneQueueFifoOrderV5;
use super::{
    LaneQueueReservationKeyV2, LaneQueueReservationRecordV5, LaneQueueReservationReleaseBarrierV3,
    LaneQueueReservationReleaseCompletionV5,
};

const RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v5";
const RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-bootstrap:v5";
const RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL5";
const RESERVATION_JOURNAL_FRAME_COMMIT: [u8; 8] = *b"IRQRDONE";
const RESERVATION_JOURNAL_FRAME_FORMAT_VERSION: u16 = 5;
const FRAME_HEADER_BYTES: u64 = 8 + 2 + 4 + 4;
const FRAME_TRAILER_BYTES: u64 = Hash::LENGTH as u64 + 8;
const FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT: usize = 1;
const FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT: usize = 26;
const FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 64 * 1024;

/// Version of the durable lane queue reservation journal and its retained records.
pub const LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5;

/// Explicit resource limits for reservation-journal append and startup replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneQueueReservationJournalLimits {
    /// File size at which compaction should be considered.
    pub(super) max_bytes_before_compact: u64,
    /// Maximum canonical Norito payload bytes in one frame.
    pub(super) max_frame_payload_bytes: u64,
    /// Maximum total file bytes accepted or appended.
    pub(super) max_file_bytes: u64,
    /// Maximum queue-owned transaction identities retained at any replay prefix.
    pub(super) max_owned_transactions: usize,
}

impl LaneQueueReservationJournalLimits {
    /// Construct explicit reservation-journal limits.
    pub(super) const fn new(
        max_bytes_before_compact: u64,
        max_frame_payload_bytes: u64,
        max_file_bytes: u64,
        max_owned_transactions: usize,
    ) -> Self {
        Self {
            max_bytes_before_compact,
            max_frame_payload_bytes,
            max_file_bytes,
            max_owned_transactions,
        }
    }

    fn validate(self) -> io::Result<Self> {
        if self.max_bytes_before_compact == 0 {
            return Err(invalid_input(
                "lane reservation journal compaction threshold must be nonzero",
            ));
        }
        if self.max_frame_payload_bytes == 0 || self.max_frame_payload_bytes > u64::from(u32::MAX) {
            return Err(invalid_input(
                "lane reservation journal frame payload limit must be in 1..=u32::MAX",
            ));
        }
        if self.max_owned_transactions == 0 {
            return Err(invalid_input(
                "lane reservation journal ownership limit must be nonzero",
            ));
        }
        if self.max_bytes_before_compact > self.max_file_bytes {
            return Err(invalid_input(
                "lane reservation journal compaction threshold exceeds its file limit",
            ));
        }
        let bootstrap_payload = norito::to_bytes(&bootstrap_frame()).map_err(|error| {
            invalid_input(format!(
                "lane reservation journal bootstrap cannot be encoded: {error}"
            ))
        })?;
        let bootstrap_payload_bytes = u64::try_from(bootstrap_payload.len())
            .map_err(|_| invalid_input("lane reservation journal bootstrap exceeds u64"))?;
        if bootstrap_payload_bytes == 0 || bootstrap_payload_bytes > self.max_frame_payload_bytes {
            return Err(invalid_input(
                "lane reservation journal frame limit cannot hold the V5 bootstrap payload",
            ));
        }
        let bootstrap_frame_bytes = FRAME_HEADER_BYTES
            .checked_add(bootstrap_payload_bytes)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_input("lane reservation journal bootstrap size overflow"))?;
        if bootstrap_frame_bytes > self.max_file_bytes {
            return Err(invalid_input(
                "lane reservation journal file limit cannot hold the V5 bootstrap frame",
            ));
        }
        Ok(self)
    }
}

/// Test-only durability boundary injected into the next append.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationJournalAppendFault {
    /// Persist only a prefix of the encoded frame, as if the write failed partway through.
    PartialWrite,
    /// Persist the complete frame, then report the same ambiguity as a failed `sync_all`.
    SyncAfterFullWrite,
}

/// Test-only durability boundary injected into the next compaction.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationJournalCompactionFault {
    /// Replace the journal inode, then fail before the parent-directory sync is acknowledged.
    AfterRenameBeforeParentSync,
}

/// One append-only reservation journal operation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneQueueReservationJournalFrameV5 {
    /// Typed file marker. Every initialized V5 journal begins with exactly this frame.
    Bootstrap {
        /// Exact first-release persistence format version.
        version: u16,
        /// Domain-separated identity of the V5 envelope and operation schema.
        format_digest: Hash,
    },
    /// Complete compacted state; only emitted into a newly rewritten journal.
    Snapshot {
        /// Reservations that still own queue transactions.
        live: Vec<LaneQueueReservationRecordV5>,
        /// Exact commits retained until the pending-plan tombstone is durable.
        committed: Vec<LaneQueueReservationKeyV2>,
        /// Ordered release claims prepared against exact live reservations.
        release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
        /// Completed releases retained until FIFO restoration is acknowledged.
        completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
    },
    /// Atomically install one or more live reservations.
    PutBatch(Vec<LaneQueueReservationRecordV5>),
    /// Atomically release one or more exact reservations back to normal queue ownership.
    ReleaseBatch(Vec<LaneQueueReservationKeyV2>),
    /// Permanently consume one exact reservation.
    Commit(LaneQueueReservationKeyV2),
    /// Forget a commit barrier after the queue-plan tombstone is independently durable.
    ForgetCommit(LaneQueueReservationKeyV2),
    /// Release only reservations owned by this exact lane incarnation.
    Prune {
        /// Lane being retired or reconciled.
        lane_id: LaneId,
        /// Exact retired incarnation; a recreated lane has a different value.
        lane_incarnation: Hash,
    },
    /// Durably claim an exact FIFO-ordered live reservation set for release.
    PrepareRelease(LaneQueueReservationReleaseBarrierV3),
    /// Atomically move the exact prepared live records into restartable completion state.
    CompleteRelease(LaneQueueReservationReleaseCompletionV5),
    /// Forget only the completion bound to this exact release identity.
    ForgetRelease(LaneQueueReservationReleaseBarrierV3),
}

/// Replayed live reservation set.
#[derive(Clone, Debug, Default)]
pub(super) struct LaneQueueReservationReplay {
    records: Vec<LaneQueueReservationRecordV5>,
    committed: Vec<LaneQueueReservationKeyV2>,
    release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
}

impl LaneQueueReservationReplay {
    /// Borrow replayed live records.
    pub(super) fn records(&self) -> &[LaneQueueReservationRecordV5] {
        &self.records
    }

    /// Borrow exact commit barriers awaiting or protecting queue-plan cleanup.
    pub(super) fn committed(&self) -> &[LaneQueueReservationKeyV2] {
        &self.committed
    }

    /// Borrow exact prepared ordered-release barriers.
    pub(super) fn release_barriers(&self) -> &[LaneQueueReservationReleaseBarrierV3] {
        &self.release_barriers
    }

    /// Borrow completed releases awaiting or protecting FIFO restoration.
    pub(super) fn completed_releases(&self) -> &[LaneQueueReservationReleaseCompletionV5] {
        &self.completed_releases
    }
}

/// Append-only reservation journal with crash repair and atomic compaction.
pub(super) struct LaneQueueReservationJournal {
    path: PathBuf,
    limits: LaneQueueReservationJournalLimits,
    file: File,
    file_identity: JournalFileIdentity,
    known_len: u64,
    parent: File,
    parent_identity: JournalFileIdentity,
    terminal_frames: u64,
    poisoned: bool,
    #[cfg(test)]
    next_append_fault: Option<ReservationJournalAppendFault>,
    #[cfg(test)]
    next_compaction_fault: Option<ReservationJournalCompactionFault>,
}

impl LaneQueueReservationJournal {
    /// Open, repair, and replay a reservation journal.
    #[cfg(test)]
    pub(super) fn open(
        path: impl AsRef<Path>,
        max_bytes_before_compact: u64,
    ) -> io::Result<(Self, LaneQueueReservationReplay)> {
        let max_bytes_before_compact =
            max_bytes_before_compact.max(minimum_bootstrap_frame_bytes()?);
        Self::open_with_limits(
            path,
            LaneQueueReservationJournalLimits::new(
                max_bytes_before_compact,
                u64::from(u32::MAX),
                u64::MAX,
                usize::MAX,
            ),
        )
    }

    /// Open, repair, and replay using the exact configured runtime budgets.
    pub(super) fn open_with_limits(
        path: impl AsRef<Path>,
        limits: LaneQueueReservationJournalLimits,
    ) -> io::Result<(Self, LaneQueueReservationReplay)> {
        let limits = limits.validate()?;
        let requested_path = path.as_ref();
        prepare_regular_journal_parent(requested_path)?;
        let path = canonical_journal_path(requested_path)?;
        prepare_regular_journal_parent(&path)?;
        reject_missing_canonical_with_compaction_temp(&path)?;
        prepare_regular_journal_path(&path)?;
        ensure_durable_v5_bootstrap(&path, limits)?;
        repair_suffix(&path, limits)?;
        reconcile_compaction_temp(&path, limits)?;
        let replay = replay_path(&path, limits)?;
        let file = open_regular_append(&path)?;
        let file_identity = verify_open_regular_path(&path, &file)?;
        let known_len = file.metadata()?.len();
        ensure_file_bound(known_len, limits)?;
        let parent = open_regular_parent(&path)?;
        let parent_identity = verify_open_regular_parent(&path, &parent)?;
        validate_file_snapshot(
            &path,
            &file,
            file_identity,
            known_len,
            &parent,
            parent_identity,
        )?;
        Ok((
            Self {
                path,
                limits,
                file,
                file_identity,
                known_len,
                parent,
                parent_identity,
                terminal_frames: 0,
                poisoned: false,
                #[cfg(test)]
                next_append_fault: None,
                #[cfg(test)]
                next_compaction_fault: None,
            },
            replay,
        ))
    }

    /// Durably append an atomic reservation batch.
    pub(super) fn put_batch(
        &mut self,
        records: Vec<LaneQueueReservationRecordV5>,
    ) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.append_durable(&LaneQueueReservationJournalFrameV5::PutBatch(records))
    }

    /// Durably release one exact reservation.
    pub(super) fn release(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.release_batch(vec![key])
    }

    /// Durably release one exact reservation batch as one journal transition.
    pub(super) fn release_batch(&mut self, keys: Vec<LaneQueueReservationKeyV2>) -> io::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        self.append_durable(&LaneQueueReservationJournalFrameV5::ReleaseBatch(keys))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably commit one exact reservation.
    pub(super) fn commit(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV5::Commit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably forget one exact commit barrier after queue-plan cleanup.
    pub(super) fn forget_commit(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV5::ForgetCommit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably release every reservation for an exact lane incarnation.
    pub(super) fn prune(&mut self, lane_id: LaneId, lane_incarnation: Hash) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV5::Prune {
            lane_id,
            lane_incarnation,
        })?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably prepare an exact FIFO-ordered release claim.
    pub(super) fn prepare_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV3,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV5::PrepareRelease(barrier))
    }

    /// Durably complete an exact prepared release as one atomic journal transition.
    pub(super) fn complete_release(
        &mut self,
        completion: LaneQueueReservationReleaseCompletionV5,
    ) -> io::Result<()> {
        completion.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV5::CompleteRelease(
            completion,
        ))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably forget only the completion for this exact full release barrier.
    pub(super) fn forget_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV3,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV5::ForgetRelease(barrier))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    fn append_durable(&mut self, frame: &LaneQueueReservationJournalFrameV5) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        let encoded = encode_frame_with_limit(frame, self.limits.max_frame_payload_bytes)?;
        if let Err(error) = self.append_staged(&encoded) {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    fn append_staged(&mut self, encoded: &[u8]) -> io::Result<()> {
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| invalid_data("lane reservation journal frame length exceeds u64"))?;
        let expected_end = self
            .known_len
            .checked_add(encoded_len)
            .ok_or_else(|| invalid_data("lane reservation journal append length overflow"))?;
        ensure_file_bound(expected_end, self.limits)?;
        let header_end_in_frame = usize::try_from(FRAME_HEADER_BYTES)
            .expect("reservation frame header length fits usize");
        let commit_len = RESERVATION_JOURNAL_FRAME_COMMIT.len();
        let commit_start_in_frame = encoded
            .len()
            .checked_sub(commit_len)
            .ok_or_else(|| invalid_data("lane reservation journal frame is shorter than commit"))?;
        let header_end = self
            .known_len
            .checked_add(FRAME_HEADER_BYTES)
            .ok_or_else(|| invalid_data("lane reservation journal header position overflow"))?;
        let body_end =
            self.known_len
                .checked_add(u64::try_from(commit_start_in_frame).map_err(|_| {
                    invalid_data("lane reservation journal body position exceeds u64")
                })?)
                .ok_or_else(|| invalid_data("lane reservation journal body position overflow"))?;

        self.verify_cached_storage_at_len(self.known_len)?;

        #[cfg(test)]
        let injected_fault = self.next_append_fault.take();
        #[cfg(test)]
        let inject_partial = matches!(
            injected_fault,
            Some(ReservationJournalAppendFault::PartialWrite)
        );
        #[cfg(not(test))]
        let inject_partial = false;
        #[cfg(test)]
        let inject_after_full_write = matches!(
            injected_fault,
            Some(ReservationJournalAppendFault::SyncAfterFullWrite)
        );
        #[cfg(not(test))]
        let inject_after_full_write = false;

        if inject_partial {
            // The durable header establishes that any following short body is one interrupted
            // append rather than an unknown file format.
            self.file.write_all(&encoded[..header_end_in_frame])?;
            self.file.sync_all()?;
            self.verify_cached_storage_at_len(header_end)?;
            let body = &encoded[header_end_in_frame..commit_start_in_frame];
            let prefix_len = body.len().div_ceil(2).min(body.len().saturating_sub(1));
            self.file.write_all(&body[..prefix_len])?;
            return Err(io::Error::other(
                "injected partial lane reservation journal staged-body failure",
            ));
        }

        self.file.write_all(&encoded[..header_end_in_frame])?;
        self.verify_cached_storage_at_len(header_end)?;
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(header_end)?;

        self.file
            .write_all(&encoded[header_end_in_frame..commit_start_in_frame])?;
        self.verify_cached_storage_at_len(body_end)?;
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(body_end)?;

        self.file.write_all(&encoded[commit_start_in_frame..])?;
        self.verify_cached_storage_at_len(expected_end)?;
        if inject_after_full_write {
            return Err(io::Error::other(
                "injected lane reservation journal sync failure after a complete staged frame",
            ));
        }
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(expected_end)?;
        self.parent.sync_all()?;
        self.verify_cached_storage_at_len(expected_end)?;
        self.known_len = expected_end;
        Ok(())
    }

    fn verify_cached_storage_at_len(&self, expected_len: u64) -> io::Result<()> {
        validate_file_snapshot(
            &self.path,
            &self.file,
            self.file_identity,
            expected_len,
            &self.parent,
            self.parent_identity,
        )
    }

    /// Inject one ambiguous append boundary for queue-level fail-closed tests.
    #[cfg(test)]
    pub(super) fn inject_next_append_fault(&mut self, fault: ReservationJournalAppendFault) {
        self.next_append_fault = Some(fault);
    }

    /// Inject one ambiguous compaction boundary for queue-level fail-closed tests.
    #[cfg(test)]
    pub(super) fn inject_next_compaction_fault(
        &mut self,
        fault: ReservationJournalCompactionFault,
    ) {
        self.next_compaction_fault = Some(fault);
    }

    /// Whether an append may have crossed the durability boundary without acknowledgement.
    pub(super) const fn durability_ambiguous(&self) -> bool {
        self.poisoned
    }

    /// Atomically rewrite only the currently live exact records when worthwhile.
    pub(super) fn compact_if_needed(
        &mut self,
        live: &[LaneQueueReservationRecordV5],
        committed: &[LaneQueueReservationKeyV2],
        release_barriers: &[LaneQueueReservationReleaseBarrierV3],
        completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    ) -> io::Result<bool> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        self.verify_cached_storage_at_len(self.known_len)?;
        let file_size = self.file.metadata()?.len();
        let retained_state_len = live
            .len()
            .saturating_add(committed.len())
            .saturating_add(release_barriers.len())
            .saturating_add(completed_releases.len());
        if file_size <= self.limits.max_bytes_before_compact
            && self.terminal_frames <= u64::try_from(retained_state_len).unwrap_or(u64::MAX)
        {
            return Ok(false);
        }

        let tmp = self.path.with_extension("reservation-compact.tmp");
        reject_existing_compaction_temp(&tmp)?;
        let snapshot = canonical_snapshot(live, committed, release_barriers, completed_releases)?;
        if let Some(frame) = snapshot.clone() {
            validate_snapshot_frame(frame)?;
        }
        let canonical_replay = replay_path(&self.path, self.limits)?;
        if canonical_snapshot(
            canonical_replay.records(),
            canonical_replay.committed(),
            canonical_replay.release_barriers(),
            canonical_replay.completed_releases(),
        )? != snapshot
        {
            return Err(invalid_data(
                "lane reservation compaction input does not match the exact durable journal state",
            ));
        }
        self.verify_cached_storage_at_len(self.known_len)?;
        let compacted = encode_compacted_journal_with_limits(snapshot.as_ref(), self.limits)?;
        ensure_file_bound(
            u64::try_from(compacted.len())
                .map_err(|_| invalid_data("lane reservation compacted journal exceeds u64"))?,
            self.limits,
        )?;
        let tmp_file = {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
            let tmp_identity = verify_open_regular_path(&tmp, &file)?;
            write_staged_bytes(&mut file, &compacted)?;
            file.sync_all()?;
            if verify_open_regular_path(&tmp, &file)? != tmp_identity
                || file.metadata()?.len()
                    != u64::try_from(compacted.len()).map_err(|_| {
                        invalid_data("lane reservation compacted journal exceeds u64")
                    })?
                || verify_open_regular_parent(&tmp, &self.parent)? != self.parent_identity
            {
                return Err(invalid_data(
                    "lane reservation compaction temp identity or length changed while writing",
                ));
            }
            file
        };
        let tmp_identity = verify_open_regular_path(&tmp, &tmp_file)?;
        persist_atomic_replacement(&tmp, &self.path)?;
        if verify_open_regular_path(&self.path, &tmp_file)? != tmp_identity
            || tmp_file.metadata()?.len()
                != u64::try_from(compacted.len())
                    .map_err(|_| invalid_data("compacted journal exceeds u64"))?
            || verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity
        {
            self.poisoned = true;
            return Err(invalid_data(
                "lane reservation compaction replacement changed during promotion",
            ));
        }
        #[cfg(test)]
        if let Some(ReservationJournalCompactionFault::AfterRenameBeforeParentSync) =
            self.next_compaction_fault.take()
        {
            self.poisoned = true;
            return Err(io::Error::other(
                "injected lane reservation journal compaction failure after rename",
            ));
        }
        if let Err(error) = tmp_file.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.parent.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        let reopened = match open_regular_append(&self.path) {
            Ok(file) => file,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let reopened_identity = match verify_open_regular_path(&self.path, &reopened) {
            Ok(identity) => identity,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.file = reopened;
        self.file_identity = reopened_identity;
        self.known_len = u64::try_from(compacted.len())
            .map_err(|_| invalid_data("compacted journal exceeds u64"))?;
        if let Err(error) = self.verify_cached_storage_at_len(self.known_len) {
            self.poisoned = true;
            return Err(error);
        }
        self.terminal_frames = 0;
        Ok(true)
    }
}

fn validate_snapshot_frame(frame: LaneQueueReservationJournalFrameV5) -> io::Result<()> {
    let mut records = Vec::new();
    let mut committed = Vec::new();
    let mut release_barriers = Vec::new();
    let mut completed_releases = Vec::new();
    apply_frame(
        &mut records,
        &mut committed,
        &mut release_barriers,
        &mut completed_releases,
        frame,
    )
}

fn replay_path(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<LaneQueueReservationReplay> {
    let mut records = Vec::<LaneQueueReservationRecordV5>::new();
    let mut committed = Vec::<LaneQueueReservationKeyV2>::new();
    let mut release_barriers = Vec::<LaneQueueReservationReleaseBarrierV3>::new();
    let mut completed_releases = Vec::<LaneQueueReservationReleaseCompletionV5>::new();
    for_each_frame(path, limits, |frame| {
        apply_frame(
            &mut records,
            &mut committed,
            &mut release_barriers,
            &mut completed_releases,
            frame,
        )?;
        ensure_replay_ownership_bound(
            &records,
            &committed,
            &release_barriers,
            &completed_releases,
            limits.max_owned_transactions,
        )
    })?;
    Ok(LaneQueueReservationReplay {
        records,
        committed,
        release_barriers,
        completed_releases,
    })
}

fn ensure_replay_ownership_bound(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    maximum: usize,
) -> io::Result<()> {
    let mut owned = BTreeSet::new();
    owned.extend(
        records
            .iter()
            .map(|record| record.key.signed_transaction_hash),
    );
    owned.extend(committed.iter().map(|key| key.signed_transaction_hash));
    for barrier in release_barriers {
        owned.extend(
            barrier
                .ordered_keys
                .iter()
                .map(|key| key.signed_transaction_hash),
        );
    }
    for completion in completed_releases {
        owned.extend(
            completion
                .records
                .iter()
                .map(|record| record.key.signed_transaction_hash),
        );
    }
    if owned.len() > maximum {
        return Err(invalid_data(format!(
            "lane reservation replay owns {} transactions, above configured limit {maximum}",
            owned.len()
        )));
    }
    Ok(())
}

fn apply_frame(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    frame: LaneQueueReservationJournalFrameV5,
) -> io::Result<()> {
    match frame {
        LaneQueueReservationJournalFrameV5::Bootstrap { .. } => {
            return Err(invalid_data(
                "lane reservation journal bootstrap cannot appear as a state operation",
            ));
        }
        LaneQueueReservationJournalFrameV5::Snapshot {
            live,
            committed: snapshot_committed,
            release_barriers: snapshot_release_barriers,
            completed_releases: snapshot_completed_releases,
        } => {
            let mut snapshot_live = Vec::<LaneQueueReservationRecordV5>::new();
            let mut validated_committed = Vec::<LaneQueueReservationKeyV2>::new();
            let mut validated_release_barriers = Vec::<LaneQueueReservationReleaseBarrierV3>::new();
            let mut validated_completed_releases =
                Vec::<LaneQueueReservationReleaseCompletionV5>::new();
            for key in snapshot_committed {
                key.validate().map_err(invalid_data)?;
                if let Some(existing) = validated_committed.iter().find(|existing| {
                    existing.signed_transaction_hash == key.signed_transaction_hash
                }) {
                    if *existing != key {
                        return Err(invalid_data(
                            "snapshot contains conflicting commit barriers for one signed transaction hash",
                        ));
                    }
                } else {
                    validated_committed.push(key);
                }
            }
            apply_put_batch(
                &mut snapshot_live,
                &validated_committed,
                &validated_release_barriers,
                &validated_completed_releases,
                live,
            )?;
            for barrier in snapshot_release_barriers {
                apply_prepare_release(
                    &snapshot_live,
                    &validated_committed,
                    &mut validated_release_barriers,
                    &validated_completed_releases,
                    barrier,
                )?;
            }
            for completion in snapshot_completed_releases {
                apply_snapshot_completion(
                    &snapshot_live,
                    &validated_committed,
                    &validated_release_barriers,
                    &mut validated_completed_releases,
                    completion,
                )?;
            }
            *records = snapshot_live;
            *committed = validated_committed;
            *release_barriers = validated_release_barriers;
            *completed_releases = validated_completed_releases;
        }
        LaneQueueReservationJournalFrameV5::PutBatch(batch) => {
            apply_put_batch(
                records,
                committed,
                release_barriers,
                completed_releases,
                batch,
            )?;
        }
        LaneQueueReservationJournalFrameV5::ReleaseBatch(keys) => {
            if keys.is_empty() {
                return Err(invalid_data(
                    "lane reservation release batch must not be empty",
                ));
            }
            let mut signed_hashes = BTreeSet::new();
            for key in &keys {
                key.validate().map_err(invalid_data)?;
                if !signed_hashes.insert(key.signed_transaction_hash) {
                    return Err(invalid_data(
                        "lane reservation release batch contains a duplicate signed transaction",
                    ));
                }
                if release_barriers
                    .iter()
                    .any(|barrier| barrier_contains_signed_hash(barrier, key))
                {
                    return Err(invalid_data(
                        "immediate release overlaps a prepared ordered release barrier",
                    ));
                }
            }
            // Exact tombstones are deliberately harmless when replayed twice and must never
            // remove a later reservation with the same signed hash but a different full plan.
            let exact_keys = keys
                .into_iter()
                .map(|key| (key.signed_transaction_hash, key))
                .collect::<BTreeMap<_, _>>();
            records.retain(|record| {
                exact_keys
                    .get(&record.key.signed_transaction_hash)
                    .is_none_or(|key| *key != record.key)
            });
        }
        LaneQueueReservationJournalFrameV5::Commit(key) => {
            apply_commit(
                records,
                committed,
                release_barriers,
                completed_releases,
                key,
            )?;
        }
        LaneQueueReservationJournalFrameV5::ForgetCommit(key) => {
            key.validate().map_err(invalid_data)?;
            committed.retain(|committed_key| *committed_key != key);
        }
        LaneQueueReservationJournalFrameV5::Prune {
            lane_id,
            lane_incarnation,
        } => {
            if records.iter().any(|record| {
                record.key.lane_id == lane_id
                    && record.key.lane_incarnation == lane_incarnation
                    && release_barriers
                        .iter()
                        .any(|barrier| barrier_contains_signed_hash(barrier, &record.key))
            }) {
                return Err(invalid_data(
                    "lane reservation prune overlaps a prepared ordered release barrier",
                ));
            }
            records.retain(|record| {
                record.key.lane_id != lane_id || record.key.lane_incarnation != lane_incarnation
            });
        }
        LaneQueueReservationJournalFrameV5::PrepareRelease(barrier) => {
            apply_prepare_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                barrier,
            )?;
        }
        LaneQueueReservationJournalFrameV5::CompleteRelease(completion) => {
            apply_complete_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                completion,
            )?;
        }
        LaneQueueReservationJournalFrameV5::ForgetRelease(barrier) => {
            apply_forget_release(release_barriers, completed_releases, barrier)?;
        }
    }
    Ok(())
}

fn apply_put_batch(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    batch: Vec<LaneQueueReservationRecordV5>,
) -> io::Result<()> {
    // Validate the entire frame before applying any record. A valid frame is one atomic
    // transition even when it contains multiple lane candidates.
    for record in &batch {
        record.validate().map_err(invalid_data)?;
        if committed
            .iter()
            .any(|key| key.signed_transaction_hash == record.key.signed_transaction_hash)
        {
            return Err(invalid_data(
                "reservation put reuses a signed transaction protected by a commit barrier",
            ));
        }
        if release_barriers
            .iter()
            .any(|barrier| barrier_contains_signed_hash(barrier, &record.key))
        {
            return Err(invalid_data(
                "reservation put overlaps a prepared ordered release barrier",
            ));
        }
        if completed_releases
            .iter()
            .any(|completion| barrier_contains_signed_hash(&completion.barrier, &record.key))
        {
            return Err(invalid_data(
                "reservation put overlaps a completed release awaiting durable cleanup",
            ));
        }
        if let Some(existing) = records.iter().find(|existing| {
            existing.key.signed_transaction_hash == record.key.signed_transaction_hash
        }) && existing != record
        {
            return Err(invalid_data(
                "conflicting live reservation for one signed transaction hash",
            ));
        }
        if records.iter().any(|existing| {
            existing.key.signed_transaction_hash != record.key.signed_transaction_hash
                && existing.fifo_order.ordinal == record.fifo_order.ordinal
        }) || completed_releases.iter().any(|completion| {
            completion.ordered_records.iter().any(|existing| {
                existing.key.signed_transaction_hash != record.key.signed_transaction_hash
                    && existing.fifo_order.ordinal == record.fifo_order.ordinal
            })
        }) {
            return Err(invalid_data(
                "reservation put reuses a durable FIFO ordinal",
            ));
        }
        if batch.iter().any(|other| {
            !core::ptr::eq(other, record)
                && other.key.signed_transaction_hash == record.key.signed_transaction_hash
                && other != record
        }) {
            return Err(invalid_data(
                "reservation batch contains conflicting transaction identities",
            ));
        }
        if batch.iter().any(|other| {
            !core::ptr::eq(other, record)
                && other.key.signed_transaction_hash != record.key.signed_transaction_hash
                && other.fifo_order.ordinal == record.fifo_order.ordinal
        }) {
            return Err(invalid_data(
                "reservation batch contains duplicate durable FIFO ordinals",
            ));
        }
    }
    for record in batch {
        if !records.iter().any(|existing| existing == &record) {
            records.push(record);
        }
    }
    Ok(())
}

fn apply_commit(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    key: LaneQueueReservationKeyV2,
) -> io::Result<()> {
    key.validate().map_err(invalid_data)?;
    if release_barriers
        .iter()
        .any(|barrier| barrier_contains_signed_hash(barrier, &key))
        || completed_releases
            .iter()
            .any(|completion| barrier_contains_signed_hash(&completion.barrier, &key))
    {
        return Err(invalid_data(
            "reservation commit overlaps an ordered release claim",
        ));
    }
    if let Some(existing) = records
        .iter()
        .find(|record| record.key.signed_transaction_hash == key.signed_transaction_hash)
        && existing.key != key
    {
        return Err(invalid_data(
            "reservation commit conflicts with a different live reservation identity",
        ));
    }
    if let Some(existing) = committed
        .iter()
        .find(|existing| existing.signed_transaction_hash == key.signed_transaction_hash)
    {
        if *existing != key {
            return Err(invalid_data(
                "reservation commit conflicts with an existing commit barrier",
            ));
        }
        records.retain(|record| record.key != key);
        return Ok(());
    }
    records.retain(|record| record.key != key);
    committed.push(key);
    Ok(())
}

fn apply_prepare_release(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    barrier: LaneQueueReservationReleaseBarrierV3,
) -> io::Result<()> {
    barrier.validate().map_err(invalid_data)?;
    if committed
        .iter()
        .any(|key| barrier_contains_signed_hash(&barrier, key))
    {
        return Err(invalid_data(
            "ordered release barrier overlaps a committed reservation",
        ));
    }
    for existing in release_barriers.iter() {
        if release_barriers_overlap(existing, &barrier) && existing != &barrier {
            return Err(invalid_data(
                "conflicting ordered release barriers overlap one signed transaction",
            ));
        }
    }
    let completed_exact = completed_releases
        .iter()
        .any(|completion| completion.barrier == barrier);
    for completion in completed_releases {
        if release_barriers_overlap(&completion.barrier, &barrier) && completion.barrier != barrier
        {
            return Err(invalid_data(
                "ordered release barrier overlaps a conflicting completed release",
            ));
        }
    }
    // A retry after CompleteRelease must be harmless even though ownership has
    // already moved out of the live set.
    if completed_exact {
        return Ok(());
    }
    for key in &barrier.ordered_keys {
        let Some(record) = records
            .iter()
            .find(|record| record.key.signed_transaction_hash == key.signed_transaction_hash)
        else {
            return Err(invalid_data(
                "ordered release barrier references a missing live reservation",
            ));
        };
        if record.key != *key {
            return Err(invalid_data(
                "ordered release barrier conflicts with the exact live reservation",
            ));
        }
    }
    if !release_barriers.contains(&barrier) {
        release_barriers.push(barrier);
    }
    Ok(())
}

fn apply_complete_release(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    completion: LaneQueueReservationReleaseCompletionV5,
) -> io::Result<()> {
    completion.validate().map_err(invalid_data)?;
    if committed
        .iter()
        .any(|key| barrier_contains_signed_hash(&completion.barrier, key))
    {
        return Err(invalid_data(
            "ordered release completion overlaps a committed reservation",
        ));
    }
    let mut completed_exact = false;
    for existing in completed_releases.iter() {
        if release_barriers_overlap(&existing.barrier, &completion.barrier) {
            if existing != &completion {
                return Err(invalid_data(
                    "conflicting completed releases overlap one signed transaction",
                ));
            }
            completed_exact = true;
        }
        if completed_fifo_orders_overlap(existing, &completion) && existing != &completion {
            return Err(invalid_data(
                "completed releases reuse one durable FIFO ordinal",
            ));
        }
    }
    if completed_exact {
        return Ok(());
    }
    let Some(barrier_position) = release_barriers
        .iter()
        .position(|barrier| barrier == &completion.barrier)
    else {
        return Err(invalid_data(
            "ordered release completion has no exact prepared barrier",
        ));
    };
    if release_barriers.iter().any(|barrier| {
        barrier != &completion.barrier && release_barriers_overlap(barrier, &completion.barrier)
    }) {
        return Err(invalid_data(
            "ordered release completion overlaps a conflicting prepared barrier",
        ));
    }
    for expected in &completion.ordered_records {
        let Some(live) = records.iter().find(|record| {
            record.key.signed_transaction_hash == expected.key.signed_transaction_hash
        }) else {
            return Err(invalid_data(
                "ordered release completion references a missing live reservation",
            ));
        };
        if live != expected {
            return Err(invalid_data(
                "ordered release completion record differs from exact live ownership",
            ));
        }
    }

    records.retain(|record| !completion.ordered_records.contains(record));
    release_barriers.remove(barrier_position);
    completed_releases.push(completion);
    Ok(())
}

fn apply_snapshot_completion(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    completion: LaneQueueReservationReleaseCompletionV5,
) -> io::Result<()> {
    completion.validate().map_err(invalid_data)?;
    for key in &completion.barrier.ordered_keys {
        if records
            .iter()
            .any(|record| record.key.signed_transaction_hash == key.signed_transaction_hash)
            || committed
                .iter()
                .any(|committed| committed.signed_transaction_hash == key.signed_transaction_hash)
            || release_barriers
                .iter()
                .any(|barrier| barrier_contains_signed_hash(barrier, key))
        {
            return Err(invalid_data(
                "snapshot completed release overlaps live, committed, or prepared ownership",
            ));
        }
    }
    for existing in completed_releases.iter() {
        if release_barriers_overlap(&existing.barrier, &completion.barrier)
            && existing != &completion
        {
            return Err(invalid_data(
                "snapshot contains conflicting overlapping completed releases",
            ));
        }
        if completed_fifo_orders_overlap(existing, &completion) && existing != &completion {
            return Err(invalid_data(
                "snapshot completed releases reuse one durable FIFO ordinal",
            ));
        }
    }
    if completion.ordered_records.iter().any(|completed| {
        records.iter().any(|live| {
            live.key.signed_transaction_hash != completed.key.signed_transaction_hash
                && live.fifo_order.ordinal == completed.fifo_order.ordinal
        })
    }) {
        return Err(invalid_data(
            "snapshot completed release reuses a live durable FIFO ordinal",
        ));
    }
    if !completed_releases.contains(&completion) {
        completed_releases.push(completion);
    }
    Ok(())
}

fn completed_fifo_orders_overlap(
    left: &LaneQueueReservationReleaseCompletionV5,
    right: &LaneQueueReservationReleaseCompletionV5,
) -> bool {
    left.ordered_records.iter().any(|left_record| {
        right.ordered_records.iter().any(|right_record| {
            left_record.key.signed_transaction_hash != right_record.key.signed_transaction_hash
                && left_record.fifo_order.ordinal == right_record.fifo_order.ordinal
        })
    })
}

fn apply_forget_release(
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    barrier: LaneQueueReservationReleaseBarrierV3,
) -> io::Result<()> {
    barrier.validate().map_err(invalid_data)?;
    if release_barriers.contains(&barrier) {
        return Err(invalid_data(
            "cannot forget an ordered release before exact completion",
        ));
    }
    completed_releases.retain(|completion| completion.barrier != barrier);
    Ok(())
}

fn barrier_contains_signed_hash(
    barrier: &LaneQueueReservationReleaseBarrierV3,
    key: &LaneQueueReservationKeyV2,
) -> bool {
    barrier
        .ordered_keys
        .iter()
        .any(|barrier_key| barrier_key.signed_transaction_hash == key.signed_transaction_hash)
}

fn release_barriers_overlap(
    left: &LaneQueueReservationReleaseBarrierV3,
    right: &LaneQueueReservationReleaseBarrierV3,
) -> bool {
    left.ordered_keys
        .iter()
        .any(|key| barrier_contains_signed_hash(right, key))
}

fn encode_frame(frame: &LaneQueueReservationJournalFrameV5) -> io::Result<Vec<u8>> {
    encode_frame_with_limit(frame, u64::from(u32::MAX))
}

fn encode_frame_with_limit(
    frame: &LaneQueueReservationJournalFrameV5,
    max_frame_payload_bytes: u64,
) -> io::Result<Vec<u8>> {
    let payload = norito::to_bytes(frame).map_err(io::Error::other)?;
    if payload.is_empty() {
        return Err(invalid_data(
            "lane reservation journal frame payload must not be empty",
        ));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| invalid_data("lane reservation journal frame is too large"))?;
    if u64::from(len) > max_frame_payload_bytes {
        return Err(invalid_data(
            "lane reservation journal frame exceeds the configured payload limit",
        ));
    }
    let version_bytes = RESERVATION_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes();
    let len_bytes = len.to_le_bytes();
    let len_guard = (!len).to_le_bytes();
    let checksum = frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload);
    let mut framed = Vec::with_capacity(
        RESERVATION_JOURNAL_FRAME_MAGIC
            .len()
            .saturating_add(version_bytes.len())
            .saturating_add(len_bytes.len())
            .saturating_add(len_guard.len())
            .saturating_add(payload.len())
            .saturating_add(Hash::LENGTH)
            .saturating_add(RESERVATION_JOURNAL_FRAME_COMMIT.len()),
    );
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_MAGIC);
    framed.extend_from_slice(&version_bytes);
    framed.extend_from_slice(&len_bytes);
    framed.extend_from_slice(&len_guard);
    framed.extend_from_slice(&payload);
    framed.extend_from_slice(checksum.as_ref());
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
    Ok(framed)
}

fn bootstrap_frame() -> LaneQueueReservationJournalFrameV5 {
    let version = RESERVATION_JOURNAL_FRAME_FORMAT_VERSION;
    let version_bytes = version.to_le_bytes();
    LaneQueueReservationJournalFrameV5::Bootstrap {
        version,
        format_digest: Hash::new_from_chunks(&[
            RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN,
            &RESERVATION_JOURNAL_FRAME_MAGIC,
            &version_bytes,
            &RESERVATION_JOURNAL_FRAME_COMMIT,
        ]),
    }
}

fn minimum_bootstrap_frame_bytes() -> io::Result<u64> {
    u64::try_from(encode_frame(&bootstrap_frame())?.len())
        .map_err(|_| invalid_input("lane reservation bootstrap frame exceeds u64"))
}

fn validate_bootstrap(frame: &LaneQueueReservationJournalFrameV5) -> io::Result<()> {
    if frame == &bootstrap_frame() {
        Ok(())
    } else {
        Err(invalid_data(
            "lane reservation journal has an invalid V5 bootstrap claim",
        ))
    }
}

fn frame_checksum(version: &[u8; 2], len: &[u8; 4], len_guard: &[u8; 4], payload: &[u8]) -> Hash {
    let mut preimage = Vec::with_capacity(
        RESERVATION_JOURNAL_FRAME_DOMAIN
            .len()
            .saturating_add(version.len())
            .saturating_add(len.len())
            .saturating_add(len_guard.len())
            .saturating_add(payload.len()),
    );
    preimage.extend_from_slice(RESERVATION_JOURNAL_FRAME_DOMAIN);
    preimage.extend_from_slice(version);
    preimage.extend_from_slice(len);
    preimage.extend_from_slice(len_guard);
    preimage.extend_from_slice(payload);
    Hash::new(preimage)
}

fn encode_compacted_journal(
    snapshot: Option<&LaneQueueReservationJournalFrameV5>,
) -> io::Result<Vec<u8>> {
    let limits =
        LaneQueueReservationJournalLimits::new(u64::MAX, u64::from(u32::MAX), u64::MAX, usize::MAX);
    encode_compacted_journal_with_limits(snapshot, limits)
}

fn encode_compacted_journal_with_limits(
    snapshot: Option<&LaneQueueReservationJournalFrameV5>,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<Vec<u8>> {
    let mut encoded = encode_frame_with_limit(&bootstrap_frame(), limits.max_frame_payload_bytes)?;
    if let Some(snapshot) = snapshot {
        encoded.extend_from_slice(&encode_frame_with_limit(
            snapshot,
            limits.max_frame_payload_bytes,
        )?);
    }
    Ok(encoded)
}

fn canonical_snapshot(
    live: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
) -> io::Result<Option<LaneQueueReservationJournalFrameV5>> {
    if live.is_empty()
        && committed.is_empty()
        && release_barriers.is_empty()
        && completed_releases.is_empty()
    {
        return Ok(None);
    }
    let mut live = live.to_vec();
    live.sort_by(|left, right| {
        left.fifo_order
            .ordinal
            .cmp(&right.fifo_order.ordinal)
            .then_with(|| {
                left.key
                    .signed_transaction_hash
                    .cmp(&right.key.signed_transaction_hash)
            })
    });
    let mut committed = committed.to_vec();
    committed.sort_by_key(|key| key.signed_transaction_hash);
    let mut release_barriers = release_barriers.to_vec();
    release_barriers.sort_by_key(|barrier| {
        barrier
            .ordered_keys
            .first()
            .map(|key| key.signed_transaction_hash)
    });
    let mut completed_releases = completed_releases.to_vec();
    completed_releases.sort_by_key(|completion| {
        completion
            .barrier
            .ordered_keys
            .first()
            .map(|key| key.signed_transaction_hash)
    });
    Ok(Some(LaneQueueReservationJournalFrameV5::Snapshot {
        live,
        committed,
        release_barriers,
        completed_releases,
    }))
}

fn write_staged_encoded_frame(file: &mut File, encoded: &[u8]) -> io::Result<()> {
    let header_end =
        usize::try_from(FRAME_HEADER_BYTES).expect("reservation frame header fits usize");
    let commit_start = encoded
        .len()
        .checked_sub(RESERVATION_JOURNAL_FRAME_COMMIT.len())
        .ok_or_else(|| invalid_data("lane reservation frame is shorter than its commit marker"))?;
    if commit_start < header_end {
        return Err(invalid_data(
            "lane reservation frame is shorter than its staged envelope",
        ));
    }
    file.write_all(&encoded[..header_end])?;
    file.sync_all()?;
    file.write_all(&encoded[header_end..commit_start])?;
    file.sync_all()?;
    file.write_all(&encoded[commit_start..])?;
    file.sync_all()
}

fn write_staged_bytes(file: &mut File, encoded: &[u8]) -> io::Result<()> {
    file.write_all(encoded)?;
    file.sync_all()
}

fn ensure_durable_v5_bootstrap(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<()> {
    let expected = encode_frame_with_limit(&bootstrap_frame(), limits.max_frame_payload_bytes)?;
    let mut file = open_regular_read_write(path)?;
    let identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let len = file.metadata()?.len();
    ensure_file_bound(len, limits)?;
    let expected_len = u64::try_from(expected.len())
        .map_err(|_| invalid_data("lane reservation bootstrap exceeds u64"))?;
    if len == 0 {
        file.seek(SeekFrom::Start(0))?;
        write_staged_encoded_frame(&mut file, &expected)?;
    } else if len < expected_len {
        let actual_len = usize::try_from(len)
            .map_err(|_| invalid_data("lane reservation bootstrap prefix exceeds usize"))?;
        let mut actual = vec![0_u8; actual_len];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut actual)?;
        if !expected.starts_with(&actual) {
            return Err(invalid_data(
                "lane reservation journal has a corrupt or unsupported initial V5 frame",
            ));
        }
        file.set_len(0)?;
        file.sync_all()?;
        parent.sync_all()?;
        file.seek(SeekFrom::Start(0))?;
        write_staged_encoded_frame(&mut file, &expected)?;
    }
    file.sync_all()?;
    parent.sync_all()?;
    let final_len = file.metadata()?.len();
    if verify_open_regular_path(path, &file)? != identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || final_len < expected_len
    {
        return Err(invalid_data(
            "lane reservation journal storage changed while establishing its V5 bootstrap",
        ));
    }
    Ok(())
}

fn repair_suffix(path: &Path, limits: LaneQueueReservationJournalLimits) -> io::Result<()> {
    let mut file = open_regular_read_write(path)?;
    let file_identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let file_len = file.metadata()?.len();
    ensure_file_bound(file_len, limits)?;
    let (_, repaired_len) = scan_frames(&mut file, file_len, limits, Some(path))?;
    // A fully formed frame may have been observed after an indeterminate final `sync_all`.
    // Adopt it only after retrying both durability boundaries; this closes the two-crash window
    // where startup could otherwise publish page-cache bytes and lose them on the next restart.
    file.sync_all()?;
    parent.sync_all()?;
    validate_file_snapshot(
        path,
        &file,
        file_identity,
        repaired_len,
        &parent,
        parent_identity,
    )
}

fn scan_frames(
    file: &mut File,
    scan_len: u64,
    limits: LaneQueueReservationJournalLimits,
    repair_path: Option<&Path>,
) -> io::Result<(Vec<LaneQueueReservationJournalFrameV5>, u64)> {
    ensure_file_bound(scan_len, limits)?;
    let mut frames = Vec::new();
    let mut position = 0_u64;
    let mut saw_bootstrap = false;
    while position < scan_len {
        let remaining = scan_len
            .checked_sub(position)
            .ok_or_else(|| invalid_data("lane reservation journal scan position underflow"))?;
        // Phase one writes and syncs at most one fixed header. At any nonzero frame boundary an
        // arbitrary suffix no longer than that header is therefore an interrupted append,
        // including a full-length torn header whose magic/length guard cannot be parsed.
        if repair_path.is_some() && position != 0 && remaining <= FRAME_HEADER_BYTES {
            let path = repair_path.expect("repair path checked above");
            truncate_suffix(file, position, path)?;
            return Ok((frames, position));
        }
        if remaining < FRAME_HEADER_BYTES {
            return Err(invalid_data(
                "lane reservation journal has an incomplete or legacy frame header",
            ));
        }
        file.seek(SeekFrom::Start(position))?;
        let mut magic = [0_u8; RESERVATION_JOURNAL_FRAME_MAGIC.len()];
        file.read_exact(&mut magic)?;
        if magic != RESERVATION_JOURNAL_FRAME_MAGIC {
            return Err(invalid_data(
                "lane reservation journal frame magic mismatch; only bootstrapped V5 is supported",
            ));
        }
        let mut version_bytes = [0_u8; 2];
        file.read_exact(&mut version_bytes)?;
        if u16::from_le_bytes(version_bytes) != RESERVATION_JOURNAL_FRAME_FORMAT_VERSION {
            return Err(invalid_data(
                "lane reservation journal frame version mismatch; only V5 is supported",
            ));
        }
        let mut len_bytes = [0_u8; 4];
        file.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes);
        let mut len_guard = [0_u8; 4];
        file.read_exact(&mut len_guard)?;
        if u32::from_le_bytes(len_guard) != !len {
            return Err(invalid_data(
                "lane reservation journal frame length guard mismatch",
            ));
        }
        let payload_len = u64::from(len);
        if payload_len == 0 || payload_len > limits.max_frame_payload_bytes {
            return Err(invalid_data(
                "lane reservation journal frame exceeds the configured payload limit",
            ));
        }
        let frame_len = FRAME_HEADER_BYTES
            .checked_add(payload_len)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_data("lane reservation journal frame length overflow"))?;
        let frame_end = position
            .checked_add(frame_len)
            .ok_or_else(|| invalid_data("lane reservation journal frame position overflow"))?;
        if frame_end > scan_len {
            if let Some(path) = repair_path.filter(|_| position != 0) {
                truncate_suffix(file, position, path)?;
                return Ok((frames, position));
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "lane reservation journal has an incomplete frame",
            ));
        }
        let payload_len = usize::try_from(payload_len)
            .map_err(|_| invalid_data("lane reservation journal frame length exceeds usize"))?;
        let mut payload = vec![0_u8; payload_len];
        file.read_exact(&mut payload)?;
        let mut checksum = [0_u8; Hash::LENGTH];
        file.read_exact(&mut checksum)?;
        let mut commit = [0_u8; RESERVATION_JOURNAL_FRAME_COMMIT.len()];
        file.read_exact(&mut commit)?;
        if commit != RESERVATION_JOURNAL_FRAME_COMMIT {
            return Err(invalid_data(
                "lane reservation journal commit marker mismatch",
            ));
        }
        if frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload).as_ref() != &checksum {
            return Err(invalid_data("lane reservation journal checksum mismatch"));
        }
        let frame = decode_frame(&payload, limits)?;
        match &frame {
            LaneQueueReservationJournalFrameV5::Bootstrap { .. }
                if position == 0 && !saw_bootstrap =>
            {
                validate_bootstrap(&frame)?;
                saw_bootstrap = true;
            }
            LaneQueueReservationJournalFrameV5::Bootstrap { .. } => {
                return Err(invalid_data(
                    "lane reservation journal bootstrap must appear exactly once at offset zero",
                ));
            }
            _ if !saw_bootstrap => {
                return Err(invalid_data(
                    "lane reservation journal operation appears before its V5 bootstrap",
                ));
            }
            _ => {}
        }
        frames.push(frame);
        position = frame_end;
    }
    if !saw_bootstrap {
        return Err(invalid_data(
            "lane reservation journal is missing its durable V5 bootstrap",
        ));
    }
    Ok((frames, position))
}

fn truncate_suffix(file: &mut File, valid_end: u64, path: &Path) -> io::Result<()> {
    let identity = verify_open_regular_path(path, file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    file.set_len(valid_end)?;
    file.sync_all()?;
    parent.sync_all()?;
    validate_file_snapshot(path, file, identity, valid_end, &parent, parent_identity)
}

fn decode_frame(
    payload: &[u8],
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<LaneQueueReservationJournalFrameV5> {
    let configured_payload_limit =
        usize::try_from(limits.max_frame_payload_bytes).unwrap_or(usize::MAX);
    if payload.is_empty() || payload.len() > configured_payload_limit {
        return Err(invalid_data(
            "lane reservation journal payload exceeds the configured frame limit",
        ));
    }
    norito::core::from_bytes_view(payload).map_err(|error| {
        invalid_data(format!(
            "lane reservation journal payload is not a canonical uncompressed archive: {error}"
        ))
    })?;
    let payload_budget = payload.len();
    let aggregate_element_budget =
        payload_budget.saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
    let aggregate_allocation_budget = payload_budget
        .checked_mul(FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT)
        .and_then(|bytes| bytes.checked_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES))
        .ok_or_else(|| {
            invalid_data("lane reservation journal payload allocation budget overflow")
        })?;
    let decode_limits = norito::DecodeLimits::new(
        payload_budget,
        payload_budget,
        aggregate_element_budget,
        aggregate_allocation_budget,
        128,
    );
    let frame = norito::decode_from_bytes_with_limits::<LaneQueueReservationJournalFrameV5>(
        payload,
        decode_limits,
    )
    .map_err(|error| {
        invalid_data(format!(
            "lane reservation journal payload cannot be decoded: {error}"
        ))
    })?;
    if norito::to_bytes(&frame).map_err(io::Error::other)? != payload {
        return Err(invalid_data(
            "lane reservation journal payload is not canonically encoded",
        ));
    }
    Ok(frame)
}

fn for_each_frame<F>(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
    mut handle: F,
) -> io::Result<()>
where
    F: FnMut(LaneQueueReservationJournalFrameV5) -> io::Result<()>,
{
    let mut file = open_regular_read(path)?;
    let identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let len = file.metadata()?.len();
    ensure_file_bound(len, limits)?;
    let (frames, scanned_len) = scan_frames(&mut file, len, limits, None)?;
    validate_file_snapshot(path, &file, identity, scanned_len, &parent, parent_identity)?;
    for frame in frames {
        if !matches!(frame, LaneQueueReservationJournalFrameV5::Bootstrap { .. }) {
            handle(frame)?;
        }
    }
    Ok(())
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = open_regular_parent(path)?;
    let identity = verify_open_regular_parent(path, &parent)?;
    parent.sync_all()?;
    if verify_open_regular_parent(path, &parent)? != identity {
        return Err(invalid_data(
            "lane reservation journal parent identity changed while synchronizing",
        ));
    }
    Ok(())
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn canonical_journal_path(path: &Path) -> io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        invalid_data("lane reservation journal path must end in a regular file name")
    })?;
    let parent = fs::canonicalize(parent_directory(path))?;
    Ok(parent.join(file_name))
}

#[cfg(unix)]
type JournalFileIdentity = (u64, u64);
#[cfg(windows)]
type JournalFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type JournalFileIdentity = ();

#[cfg(unix)]
fn journal_file_identity(metadata: &fs::Metadata) -> JournalFileIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn journal_file_identity(metadata: &fs::Metadata) -> JournalFileIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn journal_file_identity(_metadata: &fs::Metadata) -> JournalFileIdentity {}

#[cfg(unix)]
const fn journal_file_identity_available(_identity: JournalFileIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn journal_file_identity_available(identity: JournalFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn journal_file_identity_available(_identity: JournalFileIdentity) -> bool {
    false
}

fn journal_file_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn journal_file_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn journal_file_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn journal_file_is_indirect(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || journal_file_is_reparse_point(metadata)
}

fn verify_open_regular_directory(path: &Path, directory: &File) -> io::Result<JournalFileIdentity> {
    let path_metadata = fs::symlink_metadata(path)?;
    let opened = directory.metadata()?;
    let path_identity = journal_file_identity(&path_metadata);
    let opened_identity = journal_file_identity(&opened);
    if journal_file_is_indirect(&path_metadata)
        || journal_file_is_indirect(&opened)
        || !path_metadata.is_dir()
        || !opened.is_dir()
        || !journal_file_identity_available(path_identity)
        || !journal_file_identity_available(opened_identity)
    {
        return Err(invalid_data(
            "lane reservation journal parent must be a direct directory with stable identity",
        ));
    }
    if path_identity != opened_identity {
        return Err(invalid_data(
            "lane reservation journal parent path changed while opening its handle",
        ));
    }
    Ok(opened_identity)
}

#[cfg(any(unix, windows))]
fn open_regular_directory(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(
            (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        options.write(true);
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    }
    let directory = options.open(path)?;
    verify_open_regular_directory(path, &directory)?;
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
fn open_regular_directory(_path: &Path) -> io::Result<File> {
    Err(invalid_data(
        "lane reservation journal directory identity is unsupported on this platform",
    ))
}

fn open_regular_parent(path: &Path) -> io::Result<File> {
    open_regular_directory(parent_directory(path))
}

fn verify_open_regular_parent(path: &Path, parent: &File) -> io::Result<JournalFileIdentity> {
    verify_open_regular_directory(parent_directory(path), parent)
}

fn prepare_regular_journal_parent(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    // Do not create a directory chain here: a journal cannot prove that every newly linked
    // ancestor survived without independently syncing each ancestor. The storage owner must
    // establish its durable parent before opening this journal.
    let directory = open_regular_directory(parent).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "lane reservation journal requires a pre-existing direct durable parent {}: {error}",
                parent.display()
            ),
        )
    })?;
    verify_open_regular_directory(parent, &directory)?;
    Ok(())
}

fn prepare_regular_journal_path(path: &Path) -> io::Result<()> {
    prepare_regular_journal_parent(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if journal_file_is_indirect(&metadata) || !metadata.is_file() {
                return Err(invalid_data(
                    "lane reservation journal path must be a direct regular file",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(path)?;
            verify_open_regular_path(path, &file)?;
            file.sync_all()?;
        }
        Err(error) => return Err(error),
    }
    sync_parent_directory(path)
}

fn reject_missing_canonical_with_compaction_temp(path: &Path) -> io::Result<()> {
    let tmp = path.with_extension("reservation-compact.tmp");
    let canonical_missing =
        fs::symlink_metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    if canonical_missing && fs::symlink_metadata(&tmp).is_ok() {
        return Err(invalid_data(
            "lane reservation compaction temp cannot recreate a missing unauthenticated canonical journal",
        ));
    }
    Ok(())
}

fn reconcile_compaction_temp(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<()> {
    let tmp = path.with_extension("reservation-compact.tmp");
    let metadata = match fs::symlink_metadata(&tmp) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Ok(metadata) => metadata,
        Err(error) => return Err(error),
    };
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_is_single_link(&metadata)
    {
        return Err(invalid_data(
            "lane reservation compaction temp must be a direct single-link regular file",
        ));
    }
    ensure_file_bound(metadata.len(), limits)?;

    let replay = replay_path(path, limits)?;
    let snapshot = canonical_snapshot(
        replay.records(),
        replay.committed(),
        replay.release_barriers(),
        replay.completed_releases(),
    )?;
    let expected = encode_compacted_journal_with_limits(snapshot.as_ref(), limits)?;
    let temp_len = metadata.len();
    let expected_len = u64::try_from(expected.len())
        .map_err(|_| invalid_data("lane reservation compacted journal exceeds u64"))?;
    if temp_len > expected_len {
        return Err(invalid_data(
            "lane reservation compaction temp exceeds the authenticated compacted state",
        ));
    }

    let mut temp = open_regular_read(&tmp)?;
    let temp_identity = verify_open_regular_path(&tmp, &temp)?;
    let actual_len = usize::try_from(temp_len)
        .map_err(|_| invalid_data("lane reservation compaction temp exceeds usize"))?;
    let mut actual = Vec::with_capacity(actual_len);
    temp.read_to_end(&mut actual)?;
    if !expected.starts_with(&actual) {
        return Err(invalid_data(
            "lane reservation compaction temp is not an authenticated prefix of canonical state",
        ));
    }
    if verify_open_regular_path(&tmp, &temp)? != temp_identity || temp.metadata()?.len() != temp_len
    {
        return Err(invalid_data(
            "lane reservation compaction temp identity or length changed during reconciliation",
        ));
    }
    drop(temp);
    let before_remove = fs::symlink_metadata(&tmp)?;
    if journal_file_identity(&before_remove) != temp_identity
        || !journal_file_is_single_link(&before_remove)
    {
        return Err(invalid_data(
            "lane reservation compaction temp changed before reconciliation cleanup",
        ));
    }
    fs::remove_file(&tmp)?;
    sync_parent_directory(path)?;
    match fs::symlink_metadata(&tmp) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(invalid_data(
            "lane reservation compaction temp reappeared during reconciliation",
        )),
        Err(error) => Err(error),
    }
}

fn reject_existing_compaction_temp(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) => {
            let kind = if journal_file_is_indirect(&metadata) {
                "symlink or reparse point"
            } else if metadata.is_file() {
                "regular file"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "non-regular file"
            };
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("lane reservation compaction temp collision with {kind}"),
            ))
        }
        Err(error) => Err(error),
    }
}

fn validate_regular_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let identity = journal_file_identity(&metadata);
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_identity_available(identity)
    {
        return Err(invalid_data(
            "lane reservation journal path must be a direct regular file with stable identity",
        ));
    }
    if !journal_file_is_single_link(&metadata) {
        return Err(invalid_data(
            "lane reservation journal must have exactly one filesystem link",
        ));
    }
    Ok(())
}

fn verify_open_regular_path(path: &Path, file: &File) -> io::Result<JournalFileIdentity> {
    let path_metadata = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    let path_identity = journal_file_identity(&path_metadata);
    let opened_identity = journal_file_identity(&opened);
    if journal_file_is_indirect(&path_metadata)
        || journal_file_is_indirect(&opened)
        || !path_metadata.is_file()
        || !opened.is_file()
        || !journal_file_identity_available(path_identity)
        || !journal_file_identity_available(opened_identity)
    {
        return Err(invalid_data(
            "opened lane reservation journal and path must be direct regular files with stable identities",
        ));
    }
    if !journal_file_is_single_link(&path_metadata) || !journal_file_is_single_link(&opened) {
        return Err(invalid_data(
            "lane reservation journal must have exactly one filesystem link",
        ));
    }
    if path_identity != opened_identity {
        return Err(invalid_data(
            "lane reservation journal path changed while its handle was open",
        ));
    }
    Ok(opened_identity)
}

fn validate_file_snapshot(
    path: &Path,
    file: &File,
    file_identity: JournalFileIdentity,
    expected_len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
) -> io::Result<()> {
    if verify_open_regular_path(path, file)? != file_identity
        || file.metadata()?.len() != expected_len
        || verify_open_regular_parent(path, parent)? != parent_identity
    {
        return Err(invalid_data(
            "lane reservation journal file, parent identity, or length changed",
        ));
    }
    Ok(())
}

fn persist_atomic_replacement(temporary: &Path, destination: &Path) -> io::Result<()> {
    // `rename` cannot replace an existing destination on Windows. `TempPath::persist` selects
    // native replacement semantics on both supported platforms and preserves failed artifacts for
    // authenticated startup reconciliation.
    let mut temporary = tempfile::TempPath::try_from_path(temporary)?;
    temporary.disable_cleanup(true);
    temporary.persist(destination).map_err(|error| error.error)
}

fn open_regular_append(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = OpenOptions::new().append(true).read(true).open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read_write(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = File::open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn ensure_file_bound(file_len: u64, limits: LaneQueueReservationJournalLimits) -> io::Result<()> {
    if file_len > limits.max_file_bytes {
        Err(invalid_data(format!(
            "lane reservation journal file size {file_len} exceeds configured limit {}",
            limits.max_file_bytes
        )))
    } else {
        Ok(())
    }
}

fn invalid_input(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}

fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write};

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        nexus::{DataSpaceId, LaneId},
        transaction::{SignedTransaction, TransactionEntrypoint},
    };

    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision};

    const V3_RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v3";
    const V3_RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL3";
    const V4_RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v4";
    const V4_RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN: &[u8] =
        b"iroha:queue-lane-reservation-bootstrap:v4";
    const V4_RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL4";

    /// Exact prefix of the retired V3 frame schema needed to encode its old Put and Release tags.
    #[allow(dead_code)]
    #[derive(Clone, Debug, PartialEq, Eq, Encode)]
    enum LaneQueueReservationJournalFrameV3Fixture {
        Snapshot {
            live: Vec<LaneQueueReservationRecordV5>,
            committed: Vec<LaneQueueReservationKeyV2>,
            release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
            completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
        },
        PutBatch(Vec<LaneQueueReservationRecordV5>),
        Release(LaneQueueReservationKeyV2),
    }

    /// Retired V4 bootstrap envelope. Its complete bytes must be retained and rejected.
    #[derive(Clone, Debug, PartialEq, Eq, Encode)]
    enum LaneQueueReservationJournalFrameV4Fixture {
        Bootstrap { version: u16, format_digest: Hash },
    }

    fn typed_hash<T>(label: &[u8]) -> HashOf<T> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn record(seed: u8, incarnation_seed: u8) -> LaneQueueReservationRecordV5 {
        let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(7));
        LaneQueueReservationRecordV5 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            key: LaneQueueReservationKeyV2 {
                version: LaneQueueReservationKeyV2::VERSION,
                signed_transaction_hash: typed_hash::<SignedTransaction>(&[seed, 1]),
                entrypoint_hash: typed_hash::<TransactionEntrypoint>(&[seed, 2]),
                queue_plan_admission_binding_hash: Hash::new([seed, 9]),
                routing_plan_digest: Hash::new([seed, 3]),
                coordinator_leg: RouteLeg::new(route, RouteLegRole::Coordinator),
                lane_id: route.lane_id,
                dataspace_id: route.dataspace_id,
                lane_incarnation: Hash::new([incarnation_seed, 4]),
                proposal_height: 11,
                lane_block_height: 5,
                lane_block_view: 2,
                reservation_owner_hash: Hash::new([seed, 5]),
                proposal_identity_hash: Hash::new([seed, 6]),
            },
            enqueue_timestamp_ms: 42,
            fifo_order: LaneQueueFifoOrderV5 {
                version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                ordinal: u64::from(seed),
            },
        }
    }

    fn release_barrier(
        records: &[LaneQueueReservationRecordV5],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseBarrierV3 {
        let first = records.first().expect("release fixture is non-empty");
        LaneQueueReservationReleaseBarrierV3 {
            version: LaneQueueReservationReleaseBarrierV3::VERSION,
            chain_id_hash: Hash::new([release_seed, 7]),
            epoch: 3,
            lane_id: first.key.lane_id,
            dataspace_id: first.key.dataspace_id,
            lane_incarnation: first.key.lane_incarnation,
            proposal_height: first.key.proposal_height,
            lane_block_height: first.key.lane_block_height,
            lane_block_view: first.key.lane_block_view,
            origin_descriptor_hash: Hash::new([release_seed, 8]),
            origin_proposal_hash: Hash::new([release_seed, 9]),
            executable_payload_hash: Hash::new([release_seed, 10]),
            retirement_hash: Hash::new([release_seed, 11]),
            ordered_keys: records.iter().map(|record| record.key).collect(),
        }
    }

    fn release_completion(
        records: &[LaneQueueReservationRecordV5],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseCompletionV5 {
        LaneQueueReservationReleaseCompletionV5 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            barrier: release_barrier(records, release_seed),
            ordered_records: records.to_vec(),
        }
    }

    fn encode_v3_fixture_frame(frame: &LaneQueueReservationJournalFrameV3Fixture) -> Vec<u8> {
        let payload = norito::to_bytes(frame).expect("encode V3 reservation frame fixture");
        let len = u32::try_from(payload.len()).expect("bounded V3 fixture length");
        let len_bytes = len.to_le_bytes();
        let mut checksum_preimage = Vec::new();
        checksum_preimage.extend_from_slice(V3_RESERVATION_JOURNAL_FRAME_DOMAIN);
        checksum_preimage.extend_from_slice(&len_bytes);
        checksum_preimage.extend_from_slice(&payload);
        let checksum = Hash::new(checksum_preimage);
        let mut framed = Vec::new();
        framed.extend_from_slice(&V3_RESERVATION_JOURNAL_FRAME_MAGIC);
        framed.extend_from_slice(&len_bytes);
        framed.extend_from_slice(&payload);
        framed.extend_from_slice(checksum.as_ref());
        framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
        framed
    }

    fn encode_v4_bootstrap_fixture() -> Vec<u8> {
        let version = 4_u16;
        let version_bytes = version.to_le_bytes();
        let frame = LaneQueueReservationJournalFrameV4Fixture::Bootstrap {
            version,
            format_digest: Hash::new_from_chunks(&[
                V4_RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN,
                &V4_RESERVATION_JOURNAL_FRAME_MAGIC,
                &version_bytes,
                &RESERVATION_JOURNAL_FRAME_COMMIT,
            ]),
        };
        let payload = norito::to_bytes(&frame).expect("encode V4 reservation bootstrap fixture");
        let len = u32::try_from(payload.len()).expect("bounded V4 fixture length");
        let len_bytes = len.to_le_bytes();
        let len_guard = (!len).to_le_bytes();
        let checksum = Hash::new_from_chunks(&[
            V4_RESERVATION_JOURNAL_FRAME_DOMAIN,
            &version_bytes,
            &len_bytes,
            &len_guard,
            &payload,
        ]);
        let mut framed = Vec::new();
        framed.extend_from_slice(&V4_RESERVATION_JOURNAL_FRAME_MAGIC);
        framed.extend_from_slice(&version_bytes);
        framed.extend_from_slice(&len_bytes);
        framed.extend_from_slice(&len_guard);
        framed.extend_from_slice(&payload);
        framed.extend_from_slice(checksum.as_ref());
        framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
        framed
    }

    fn apply_unprotected_frame(
        records: &mut Vec<LaneQueueReservationRecordV5>,
        committed: &mut Vec<LaneQueueReservationKeyV2>,
        frame: LaneQueueReservationJournalFrameV5,
    ) -> io::Result<()> {
        apply_frame(records, committed, &mut Vec::new(), &mut Vec::new(), frame)
    }

    #[test]
    fn crash_at_every_operation_frame_write_boundary_is_prefix_atomic() {
        let first = record(1, 1);
        let second = record(2, 1);
        let barrier = release_barrier(core::slice::from_ref(&first), 1);
        let completion = release_completion(core::slice::from_ref(&first), 1);
        let first_frame = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![
            first.clone(),
        ]))
        .expect("encode first frame");
        let bootstrap = encode_frame(&bootstrap_frame()).expect("encode V5 bootstrap");
        let cases = [
            (
                "put",
                LaneQueueReservationJournalFrameV5::PutBatch(vec![second]),
            ),
            (
                "release",
                LaneQueueReservationJournalFrameV5::ReleaseBatch(vec![first.key]),
            ),
            (
                "commit",
                LaneQueueReservationJournalFrameV5::Commit(first.key),
            ),
            (
                "forget-commit",
                LaneQueueReservationJournalFrameV5::ForgetCommit(first.key),
            ),
            (
                "prune",
                LaneQueueReservationJournalFrameV5::Prune {
                    lane_id: first.key.lane_id,
                    lane_incarnation: first.key.lane_incarnation,
                },
            ),
            (
                "snapshot",
                LaneQueueReservationJournalFrameV5::Snapshot {
                    live: Vec::new(),
                    committed: vec![first.key],
                    release_barriers: Vec::new(),
                    completed_releases: Vec::new(),
                },
            ),
            (
                "prepare-release",
                LaneQueueReservationJournalFrameV5::PrepareRelease(barrier.clone()),
            ),
            (
                "complete-release",
                LaneQueueReservationJournalFrameV5::CompleteRelease(completion),
            ),
            (
                "forget-release",
                LaneQueueReservationJournalFrameV5::ForgetRelease(barrier),
            ),
        ];

        for (label, operation) in cases {
            let operation_frame = encode_frame(&operation).expect("encode operation frame");
            for written in 0..operation_frame.len() {
                let dir = tempfile::tempdir().expect("tempdir");
                let path = dir.path().join("reservations.norito");
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&path)
                    .expect("open raw journal");
                file.write_all(&bootstrap).expect("write V5 bootstrap");
                file.write_all(&first_frame).expect("write first frame");
                file.write_all(&operation_frame[..written])
                    .expect("write partial operation frame");
                drop(file);

                let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
                    .expect("repair truncated boundary");
                assert_eq!(
                    replay.records(),
                    &[first.clone()],
                    "{label} boundary {written} must expose only the preceding durable frame"
                );
                assert!(replay.committed().is_empty());
                assert!(replay.release_barriers().is_empty());
                assert!(replay.completed_releases().is_empty());
            }
        }
    }

    #[test]
    fn corrupt_complete_suffix_fails_closed_without_truncation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("reservations.norito");
        let first = record(1, 1);
        let second = record(2, 1);
        let third = record(3, 1);
        let first_frame = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![
            first.clone(),
        ]))
        .expect("encode first");
        let mut corrupt = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![second]))
            .expect("encode second");
        let third_frame = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![third]))
            .expect("encode third");
        let bootstrap = encode_frame(&bootstrap_frame()).expect("encode V5 bootstrap");
        let corrupt_index = corrupt.len() - 1;
        corrupt[corrupt_index] ^= 0x80;
        let mut file = File::create(&path).expect("create journal");
        file.write_all(&bootstrap).expect("write V5 bootstrap");
        file.write_all(&first_frame).expect("write first");
        file.write_all(&corrupt).expect("write corrupt second");
        file.write_all(&third_frame).expect("write trailing third");
        drop(file);

        let corrupt_len = path.metadata().expect("metadata").len();
        assert!(
            LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
            "a fully written frame with a bad commit/checksum is corruption, not a torn write"
        );
        assert_eq!(
            path.metadata().expect("metadata after rejection").len(),
            corrupt_len,
            "fail-closed recovery must retain corrupt evidence for operator repair"
        );
    }

    #[test]
    fn legacy_and_unknown_frame_magic_are_rejected_without_rewrite() {
        for (label, magic) in [
            ("v1", *b"IRQRJNL1"),
            ("v2", *b"IRQRJNL2"),
            ("v3", *b"IRQRJNL3"),
            ("unknown", *b"IRQRJNL9"),
        ] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("{label}.norito"));
            let mut bytes = magic.to_vec();
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            fs::write(&path, &bytes).expect("write legacy or unknown header");

            assert!(
                LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
                "{label} journal magic must fail closed"
            );
            assert_eq!(
                fs::read(&path).expect("retain rejected bytes"),
                bytes,
                "{label} evidence must not be rewritten as a V5 journal"
            );
        }
    }

    #[test]
    fn complete_v3_frames_are_rejected_without_repair_or_rewrite() {
        let mut legacy_record = record(1, 1);
        legacy_record.version = 3;
        legacy_record.fifo_order.version = 3;
        let frames = [
            encode_v3_fixture_frame(&LaneQueueReservationJournalFrameV3Fixture::PutBatch(vec![
                legacy_record.clone(),
            ])),
            encode_v3_fixture_frame(&LaneQueueReservationJournalFrameV3Fixture::Release(
                legacy_record.key,
            )),
        ];

        for (index, bytes) in frames.into_iter().enumerate() {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("v3-frame-{index}.norito"));
            fs::write(&path, &bytes).expect("write complete V3 frame fixture");
            let original_len = path.metadata().expect("V3 metadata").len();

            let error = LaneQueueReservationJournal::open(&path, u64::MAX)
                .err()
                .expect("a complete V3 frame must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(
                error.to_string().contains("frame magic mismatch"),
                "unexpected V3 rejection: {error}"
            );
            assert_eq!(
                path.metadata().expect("metadata after rejection").len(),
                original_len,
                "complete V3 evidence must not be truncated"
            );
            assert_eq!(
                fs::read(&path).expect("retain complete V3 evidence"),
                bytes,
                "complete V3 evidence must not be rewritten as V5"
            );
        }
    }

    #[test]
    fn complete_v4_bootstrap_is_rejected_without_repair_or_rewrite() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v4-bootstrap.norito");
        let bytes = encode_v4_bootstrap_fixture();
        fs::write(&path, &bytes).expect("write complete V4 bootstrap fixture");

        let error = LaneQueueReservationJournal::open(&path, u64::MAX)
            .err()
            .expect("a complete V4 bootstrap must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("frame magic mismatch"),
            "unexpected V4 rejection: {error}"
        );
        assert_eq!(
            fs::read(&path).expect("retain complete V4 evidence"),
            bytes,
            "complete V4 evidence must not be rewritten as V5"
        );
    }

    #[test]
    fn v5_envelope_rejects_unsupported_record_versions_without_rewrite() {
        for unsupported_version in [3, 4, 6] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir
                .path()
                .join(format!("v5-envelope-v{unsupported_version}-record.norito"));
            let mut unsupported = record(1, 1);
            unsupported.version = unsupported_version;
            unsupported.fifo_order.version = unsupported_version;
            let bytes = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![
                unsupported,
            ]))
            .expect("encode V5 envelope around unsupported record");
            let mut journal_bytes = encode_frame(&bootstrap_frame()).expect("encode V5 bootstrap");
            journal_bytes.extend_from_slice(&bytes);
            fs::write(&path, &journal_bytes).expect("write version-mismatched frame");

            let error = LaneQueueReservationJournal::open(&path, u64::MAX)
                .err()
                .expect("unsupported record inside a V5 envelope must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                fs::read(&path).expect("retain version-mismatched evidence"),
                journal_bytes,
                "version-mismatched evidence must not be rewritten"
            );
        }
    }

    #[test]
    fn v5_release_batch_replay_is_atomic_idempotent_and_exact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v5-release-batch.norito");
        let first = record(1, 1);
        let second = record(2, 1);
        let third = record(3, 1);
        let released = vec![first.key, third.key];

        {
            let (mut journal, replay) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create V5 journal");
            assert!(replay.records().is_empty());
            journal
                .put_batch(vec![first.clone(), second.clone(), third])
                .expect("persist V5 reservation batch");
            journal
                .release_batch(released.clone())
                .expect("atomically release two exact reservations");
        }

        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay V5 release batch");
        assert_eq!(
            replay.records(),
            core::slice::from_ref(&second),
            "one V5 ReleaseBatch frame must remove every exact member"
        );

        let mut replacement = first;
        replacement.key.routing_plan_digest = Hash::new(b"replacement-plan");
        replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal");
        journal
            .put_batch(vec![replacement.clone()])
            .expect("re-admit same hash under a distinct exact owner");
        journal
            .release_batch(released.clone())
            .expect("replay stale exact release batch");
        journal
            .release_batch(released)
            .expect("repeat stale exact release batch idempotently");
        drop(journal);

        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay exact V5 history");
        assert_eq!(
            replay.records(),
            &[second, replacement],
            "a repeated V5 batch must not remove a later non-identical reservation"
        );
        assert!(replay.committed().is_empty());
        assert!(replay.release_barriers().is_empty());
        assert!(replay.completed_releases().is_empty());
    }

    #[test]
    fn duplicate_exact_replay_is_idempotent_but_conflicting_owner_is_rejected() {
        let exact = record(1, 1);
        let mut records = Vec::new();
        let mut committed = Vec::new();
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV5::PutBatch(vec![exact.clone(), exact.clone()]),
        )
        .expect("duplicate exact record");
        assert_eq!(records, vec![exact.clone()]);

        let mut conflicting = exact;
        conflicting.key.reservation_owner_hash = Hash::new(b"conflicting-owner");
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![conflicting]),
            )
            .is_err()
        );

        let mut conflicting_plan = records[0].clone();
        conflicting_plan.key.routing_plan_digest = Hash::new(b"conflicting-plan");
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![conflicting_plan]),
            )
            .is_err()
        );

        let mut conflicting_fifo_order = record(3, 1);
        conflicting_fifo_order.fifo_order = records[0].fifo_order;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![conflicting_fifo_order]),
            )
            .is_err(),
            "one durable FIFO ordinal cannot identify two transaction hashes"
        );

        let mut participant = record(2, 1);
        participant.key.coordinator_leg.role = RouteLegRole::Participant;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![participant]),
            )
            .is_err(),
            "participant legs must never become full-transaction reservations"
        );
    }

    #[test]
    fn reservation_record_rejects_legacy_or_zero_fifo_order_identity() {
        let mut records = Vec::new();
        let mut committed = Vec::new();
        let mut legacy = record(4, 1);
        legacy.fifo_order.version = LANE_QUEUE_RESERVATION_JOURNAL_VERSION - 1;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![legacy]),
            )
            .is_err()
        );
        let mut zero = record(5, 1);
        zero.fifo_order.ordinal = 0;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![zero]),
            )
            .is_err()
        );
        assert!(records.is_empty());
    }

    #[test]
    fn ordered_release_survives_every_restart_phase_and_exact_retries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ordered-release.norito");
        let records = vec![record(1, 1), record(2, 1)];
        let barrier = release_barrier(&records, 1);
        let completion = release_completion(&records, 1);

        {
            let (mut journal, replay) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            assert!(replay.records().is_empty());
            journal
                .put_batch(records.clone())
                .expect("persist exact reservation batch");
            journal
                .prepare_release(barrier.clone())
                .expect("prepare ordered release");
            journal
                .prepare_release(barrier.clone())
                .expect("repeat exact prepare");
        }
        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay prepared release");
        assert_eq!(replay.records(), records.as_slice());
        assert_eq!(replay.release_barriers(), &[barrier.clone()]);
        assert!(replay.completed_releases().is_empty());

        journal
            .complete_release(completion.clone())
            .expect("complete ordered release");
        journal
            .complete_release(completion.clone())
            .expect("repeat exact completion");
        journal
            .prepare_release(barrier.clone())
            .expect("retry prepare after completion");
        drop(journal);

        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay completed release");
        assert!(replay.records().is_empty());
        assert!(replay.release_barriers().is_empty());
        assert_eq!(replay.completed_releases(), &[completion]);

        journal
            .forget_release(barrier.clone())
            .expect("forget exact completion");
        journal
            .forget_release(barrier)
            .expect("repeat exact forget");
        drop(journal);

        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("replay forgotten release");
        assert!(replay.records().is_empty());
        assert!(replay.release_barriers().is_empty());
        assert!(replay.completed_releases().is_empty());
    }

    #[test]
    fn ordered_release_rejects_conflicts_partial_completion_and_aba_reuse() {
        let records = vec![record(1, 1), record(2, 1)];
        let barrier = release_barrier(&records, 1);
        let completion = release_completion(&records, 1);
        let mut live = records.clone();
        let mut committed = Vec::new();
        let mut barriers = Vec::new();
        let mut completed = Vec::new();

        assert!(
            apply_frame(
                &mut live,
                &mut committed,
                &mut barriers,
                &mut completed,
                LaneQueueReservationJournalFrameV5::CompleteRelease(completion.clone()),
            )
            .is_err(),
            "completion must require its exact prepared barrier"
        );
        assert_eq!(live, records);

        apply_frame(
            &mut live,
            &mut committed,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV5::PrepareRelease(barrier.clone()),
        )
        .expect("prepare exact release");

        let mut conflicting_barrier = barrier.clone();
        conflicting_barrier.retirement_hash = Hash::new(b"conflicting-retirement");
        assert!(
            apply_frame(
                &mut live,
                &mut committed,
                &mut barriers,
                &mut completed,
                LaneQueueReservationJournalFrameV5::PrepareRelease(conflicting_barrier),
            )
            .is_err(),
            "overlapping release identities must fail closed"
        );

        let mut wrong_records = completion.clone();
        wrong_records.ordered_records[0].enqueue_timestamp_ms = wrong_records.ordered_records[0]
            .enqueue_timestamp_ms
            .saturating_add(1);
        assert!(
            apply_frame(
                &mut live,
                &mut committed,
                &mut barriers,
                &mut completed,
                LaneQueueReservationJournalFrameV5::CompleteRelease(wrong_records),
            )
            .is_err(),
            "completion must match exact live records, including FIFO timestamps"
        );
        assert_eq!(live, records);
        assert_eq!(barriers, vec![barrier.clone()]);
        assert!(completed.is_empty());

        apply_frame(
            &mut live,
            &mut committed,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV5::CompleteRelease(completion.clone()),
        )
        .expect("complete exact release");
        assert!(live.is_empty());
        assert!(barriers.is_empty());
        assert_eq!(completed, vec![completion.clone()]);

        let recreated = record(1, 2);
        assert!(
            apply_frame(
                &mut live,
                &mut committed,
                &mut barriers,
                &mut completed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![recreated]),
            )
            .is_err(),
            "completed release must block same-hash ABA reservation reuse"
        );

        let mut stale_forget = barrier.clone();
        stale_forget.retirement_hash = Hash::new(b"stale-forget-retirement");
        apply_frame(
            &mut live,
            &mut committed,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV5::ForgetRelease(stale_forget),
        )
        .expect("stale full identity is a harmless no-op");
        assert_eq!(completed, vec![completion]);

        apply_frame(
            &mut live,
            &mut committed,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV5::ForgetRelease(barrier),
        )
        .expect("forget exact completion");
        assert!(completed.is_empty());
    }

    #[test]
    fn snapshot_rejects_completed_release_overlapping_live_ownership() {
        let record = record(1, 1);
        let completion = release_completion(core::slice::from_ref(&record), 1);
        let mut live = Vec::new();
        let mut committed = Vec::new();
        let mut barriers = Vec::new();
        let mut completed = Vec::new();
        assert!(
            apply_frame(
                &mut live,
                &mut committed,
                &mut barriers,
                &mut completed,
                LaneQueueReservationJournalFrameV5::Snapshot {
                    live: vec![record],
                    committed: Vec::new(),
                    release_barriers: Vec::new(),
                    completed_releases: vec![completion],
                },
            )
            .is_err()
        );
        assert!(live.is_empty(), "invalid snapshot must apply atomically");
        assert!(committed.is_empty());
        assert!(barriers.is_empty());
        assert!(completed.is_empty());
    }

    #[test]
    fn exact_tombstone_does_not_remove_readmitted_hash_with_new_plan() {
        let old = record(1, 1);
        let mut replacement = old.clone();
        replacement.key.routing_plan_digest = Hash::new(b"replacement-plan");
        replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal");
        let mut records = vec![replacement.clone()];
        let mut committed = Vec::new();
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV5::ReleaseBatch(vec![old.key]),
        )
        .expect("stale release is idempotent");
        assert_eq!(records, vec![replacement]);
    }

    #[test]
    fn put_rejects_same_hash_reuse_behind_commit_barrier() {
        let old = record(1, 1);
        let mut live = vec![old.clone()];
        let mut committed = Vec::new();
        apply_unprotected_frame(
            &mut live,
            &mut committed,
            LaneQueueReservationJournalFrameV5::Commit(old.key),
        )
        .expect("commit exact live reservation");
        assert!(live.is_empty());

        let mut replacement = old;
        replacement.key.routing_plan_digest = Hash::new(b"replacement-plan-after-commit");
        replacement.key.proposal_identity_hash = Hash::new(b"replacement-proposal-after-commit");
        assert!(
            apply_unprotected_frame(
                &mut live,
                &mut committed,
                LaneQueueReservationJournalFrameV5::PutBatch(vec![replacement]),
            )
            .is_err(),
            "commit cleanup must block all same-hash reservation identities"
        );
    }

    #[test]
    fn prune_is_exact_to_lane_incarnation() {
        let retired = record(1, 1);
        let recreated = record(2, 2);
        let mut records = vec![retired.clone(), recreated.clone()];
        let mut committed = Vec::new();
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV5::Prune {
                lane_id: retired.key.lane_id,
                lane_incarnation: retired.key.lane_incarnation,
            },
        )
        .expect("apply prune");
        assert_eq!(records, vec![recreated]);
    }

    #[test]
    fn compaction_is_replay_equivalent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("reservations.norito");
        let first = record(1, 1);
        let second = record(2, 1);
        let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
        journal
            .put_batch(vec![first.clone(), second.clone()])
            .expect("put records");
        journal.release(first.key).expect("release first");
        assert!(
            journal
                .compact_if_needed(core::slice::from_ref(&second), &[], &[], &[])
                .expect("compact")
        );
        drop(journal);

        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, 1).expect("reopen compacted journal");
        assert_eq!(replay.records(), &[second]);
    }

    #[test]
    fn compaction_preserves_prepared_and_completed_release_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("release-compaction.norito");
        let prepared_records = vec![record(1, 1), record(2, 1)];
        let completed_records = vec![record(3, 1), record(4, 1)];
        let prepared = release_barrier(&prepared_records, 1);
        let completed = release_completion(&completed_records, 2);
        let mut all_records = prepared_records.clone();
        all_records.extend(completed_records);

        let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
        journal
            .put_batch(all_records)
            .expect("persist all reservation ownership");
        journal
            .prepare_release(prepared.clone())
            .expect("prepare first release");
        journal
            .prepare_release(completed.barrier.clone())
            .expect("prepare second release");
        journal
            .complete_release(completed.clone())
            .expect("complete second release");
        assert!(
            journal
                .compact_if_needed(
                    &prepared_records,
                    &[],
                    core::slice::from_ref(&prepared),
                    core::slice::from_ref(&completed),
                )
                .expect("compact all V5 release state")
        );
        drop(journal);

        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, 1).expect("replay compacted V5 snapshot");
        assert_eq!(replay.records(), prepared_records.as_slice());
        assert!(replay.committed().is_empty());
        assert_eq!(replay.release_barriers(), &[prepared]);
        assert_eq!(replay.completed_releases(), &[completed]);
    }

    #[test]
    fn commit_barrier_survives_restart_until_exact_forget_is_durable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("commit-barrier.norito");
        let record = record(9, 1);
        {
            let (mut journal, _) =
                LaneQueueReservationJournal::open(&path, 1).expect("open journal");
            journal
                .put_batch(vec![record.clone()])
                .expect("put reservation");
            journal.commit(record.key).expect("commit reservation");
        }
        let (mut journal, replay) =
            LaneQueueReservationJournal::open(&path, 1).expect("replay commit barrier");
        assert!(replay.records().is_empty());
        assert_eq!(replay.committed(), &[record.key]);

        journal
            .forget_commit(record.key)
            .expect("forget after independent queue-plan durability");
        journal
            .compact_if_needed(&[], &[], &[], &[])
            .expect("compact forgotten barrier");
        drop(journal);
        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, 1).expect("reopen forgotten barrier");
        assert!(replay.records().is_empty());
        assert!(replay.committed().is_empty());
    }

    #[test]
    fn newly_created_journal_survives_immediate_close_and_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("newly-created.norito");
        let record = record(11, 1);
        {
            let (mut journal, replay) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            assert!(replay.records().is_empty());
            assert!(
                fs::symlink_metadata(&path)
                    .expect("journal metadata")
                    .is_file()
            );
            journal
                .put_batch(vec![record.clone()])
                .expect("power-loss durability boundary");
        }
        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("reopen journal");
        assert_eq!(replay.records(), &[record]);
    }

    #[test]
    fn journal_rejects_non_regular_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("journal-directory");
        fs::create_dir(&path).expect("create path directory");
        assert!(
            LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
            "a directory must never be opened or truncated as a journal"
        );
    }

    #[cfg(unix)]
    #[test]
    fn journal_rejects_symlink_path_and_symlink_parent() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target");
        File::create(&target).expect("create target");
        let path_link = dir.path().join("journal-link");
        symlink(&target, &path_link).expect("create journal symlink");
        assert!(LaneQueueReservationJournal::open(&path_link, u64::MAX).is_err());

        let real_parent = dir.path().join("real-parent");
        fs::create_dir(&real_parent).expect("create real parent");
        let linked_parent = dir.path().join("linked-parent");
        symlink(&real_parent, &linked_parent).expect("create parent symlink");
        assert!(
            LaneQueueReservationJournal::open(linked_parent.join("journal"), u64::MAX).is_err(),
            "journal creation must not follow a symlink parent"
        );
    }

    #[test]
    fn compaction_rejects_preexisting_regular_temp_collision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("collision.norito");
        let record = record(12, 1);
        let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
        journal
            .put_batch(vec![record.clone()])
            .expect("write live record");
        let tmp = path.with_extension("reservation-compact.tmp");
        File::create(&tmp).expect("create colliding temp");
        assert!(
            journal
                .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[])
                .is_err(),
            "compaction must never truncate a predictable preexisting temp path"
        );
        assert_eq!(journal.path, path);
    }

    #[cfg(unix)]
    #[test]
    fn compaction_rejects_symlink_temp_collision_without_touching_target() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("symlink-collision.norito");
        let target = dir.path().join("do-not-truncate");
        fs::write(&target, b"sentinel").expect("write target sentinel");
        let record = record(13, 1);
        let (mut journal, _) = LaneQueueReservationJournal::open(&path, 1).expect("open journal");
        journal
            .put_batch(vec![record.clone()])
            .expect("write live record");
        let tmp = path.with_extension("reservation-compact.tmp");
        symlink(&target, &tmp).expect("create malicious temp symlink");
        assert!(
            journal
                .compact_if_needed(core::slice::from_ref(&record), &[], &[], &[])
                .is_err()
        );
        assert_eq!(fs::read(&target).expect("read sentinel"), b"sentinel");
    }

    #[test]
    fn initial_bootstrap_recovers_every_recognizable_staged_prefix() {
        let expected = encode_frame(&bootstrap_frame()).expect("encode canonical V5 bootstrap");
        for written in 0..expected.len() {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir
                .path()
                .join(format!("bootstrap-prefix-{written}.norito"));
            fs::write(&path, &expected[..written]).expect("write interrupted bootstrap prefix");

            let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
                .expect("recover canonical bootstrap prefix");
            assert!(replay.records().is_empty());
            assert_eq!(
                fs::read(&path).expect("read repaired bootstrap"),
                expected,
                "bootstrap prefix {written} must be replaced by the exact durable V5 marker"
            );
        }
    }

    #[test]
    fn full_length_torn_terminal_header_is_repaired_without_parsing_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("full-header-tear.norito");
        let first = record(21, 1);
        {
            let (mut journal, _) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            journal
                .put_batch(vec![first.clone()])
                .expect("persist preceding frame");
        }
        let durable_len = path.metadata().expect("journal metadata").len();
        let torn_header =
            vec![0xA5; usize::try_from(FRAME_HEADER_BYTES).expect("header fits usize")];
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open journal append")
            .write_all(&torn_header)
            .expect("write full-length torn header");

        let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("repair full-length staged header");
        assert_eq!(replay.records(), &[first]);
        assert_eq!(
            path.metadata().expect("repaired metadata").len(),
            durable_len
        );
    }

    #[test]
    fn complete_indeterminate_frame_is_synced_before_two_restart_adoption() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("two-crash-adoption.norito");
        let first = record(22, 1);
        let second = record(23, 1);
        {
            let (mut journal, _) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            journal
                .put_batch(vec![first.clone()])
                .expect("persist first record");
        }
        let second_frame = encode_frame(&LaneQueueReservationJournalFrameV5::PutBatch(vec![
            second.clone(),
        ]))
        .expect("encode indeterminate complete frame");
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open raw append")
            .write_all(&second_frame)
            .expect("materialize complete pre-sync frame");

        {
            let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
                .expect("first restart adopts and synchronizes complete frame");
            assert_eq!(replay.records(), &[first.clone(), second.clone()]);
        }
        let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("second restart retains adopted frame");
        assert_eq!(replay.records(), &[first, second]);
    }

    #[test]
    fn authenticated_truncated_compaction_temp_is_discarded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("truncated-temp.norito");
        let first = record(24, 1);
        {
            let (mut journal, _) =
                LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
            journal
                .put_batch(vec![first.clone()])
                .expect("persist record");
        }
        let snapshot = canonical_snapshot(core::slice::from_ref(&first), &[], &[], &[])
            .expect("build canonical snapshot");
        let compacted =
            encode_compacted_journal(snapshot.as_ref()).expect("encode canonical compaction");
        let tmp = path.with_extension("reservation-compact.tmp");
        fs::write(&tmp, &compacted[..compacted.len() / 2])
            .expect("write authenticated compaction prefix");

        let (_journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("reconcile interrupted compaction");
        assert_eq!(replay.records(), &[first]);
        assert!(
            !tmp.exists(),
            "authenticated prefix must be durably removed"
        );
    }

    #[test]
    fn corrupt_or_oversized_compaction_temp_fails_closed_and_is_retained() {
        for oversized in [false, true] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(if oversized {
                "oversized-temp.norito"
            } else {
                "corrupt-temp.norito"
            });
            let first = record(25, 1);
            {
                let (mut journal, _) =
                    LaneQueueReservationJournal::open(&path, u64::MAX).expect("create journal");
                journal
                    .put_batch(vec![first.clone()])
                    .expect("persist record");
            }
            let snapshot = canonical_snapshot(core::slice::from_ref(&first), &[], &[], &[])
                .expect("build canonical snapshot");
            let mut compacted =
                encode_compacted_journal(snapshot.as_ref()).expect("encode canonical compaction");
            if oversized {
                compacted.push(0);
            } else {
                compacted[0] ^= 0x80;
            }
            let tmp = path.with_extension("reservation-compact.tmp");
            fs::write(&tmp, &compacted).expect("write invalid compaction temp");
            let canonical = fs::read(&path).expect("read canonical before rejection");

            assert!(
                LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
                "invalid compaction temp must fail closed"
            );
            assert_eq!(
                fs::read(&tmp).expect("retain invalid temp evidence"),
                compacted
            );
            assert_eq!(
                fs::read(&path).expect("retain canonical evidence"),
                canonical
            );
        }
    }

    #[test]
    fn compaction_temp_cannot_recreate_missing_canonical_journal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing-canonical.norito");
        let tmp = path.with_extension("reservation-compact.tmp");
        let compacted =
            encode_compacted_journal(None).expect("encode an otherwise valid empty compaction");
        fs::write(&tmp, &compacted).expect("write orphan compaction temp");

        assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
        assert!(
            !path.exists(),
            "startup must not synthesize a canonical owner"
        );
        assert_eq!(fs::read(&tmp).expect("retain orphan evidence"), compacted);
    }

    #[test]
    fn portable_atomic_replacement_replaces_existing_destination() {
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("destination");
        let temporary = dir.path().join("temporary");
        fs::write(&destination, b"old").expect("write old destination");
        fs::write(&temporary, b"new").expect("write replacement");

        persist_atomic_replacement(&temporary, &destination).expect("replace destination");
        assert_eq!(fs::read(&destination).expect("read replacement"), b"new");
        assert!(!temporary.exists());
    }

    #[cfg(unix)]
    #[test]
    fn journal_rejects_existing_and_new_hardlinks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("hardlink-journal.norito");
        let alias = dir.path().join("hardlink-alias.norito");
        let first = record(26, 1);
        let mut journal = LaneQueueReservationJournal::open(&path, u64::MAX)
            .expect("create journal")
            .0;
        fs::hard_link(&path, &alias).expect("create unexpected hardlink");
        assert!(
            journal.put_batch(vec![first]).is_err(),
            "cached append handle must reject a link-count change"
        );
        assert!(journal.durability_ambiguous());
        drop(journal);
        assert!(
            LaneQueueReservationJournal::open(&path, u64::MAX).is_err(),
            "startup must reject a multiply linked journal"
        );
        fs::remove_file(&alias).expect("remove hardlink alias");
        assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_ok());
    }

    #[test]
    fn journal_requires_preexisting_durable_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing_parent = dir.path().join("missing").join("nested");
        let path = missing_parent.join("reservations.norito");
        assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
        assert!(
            !missing_parent.exists(),
            "journal open must not create an ancestor chain it cannot durably link"
        );
    }

    #[cfg(windows)]
    #[test]
    fn journal_rejects_reparse_point_file_when_platform_allows_fixture() {
        use std::os::windows::fs::symlink_file;

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target");
        File::create(&target).expect("create target");
        let path = dir.path().join("journal-reparse");
        match symlink_file(&target, &path) {
            Ok(()) => {
                let metadata = fs::symlink_metadata(&path).expect("reparse metadata");
                assert!(journal_file_is_reparse_point(&metadata));
                assert!(LaneQueueReservationJournal::open(&path, u64::MAX).is_err());
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
            Err(error) => panic!("create reparse fixture: {error}"),
        }
    }
}
