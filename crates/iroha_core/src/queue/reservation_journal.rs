//! Crash-safe local journal for lane-owned queue reservations.
//!
//! A reservation is local scheduling state rather than consensus state, but losing it can make
//! the same transaction eligible for both the global scheduler and an independently ticking lane.
//! The journal therefore uses checksummed, length-delimited frames and synchronizes every state
//! transition before the queue exposes it to callers.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;
use iroha_data_model::nexus::LaneId;
use norito::codec::{Decode, Encode};

#[cfg(test)]
use super::LaneQueueFifoOrderV3;
use super::{
    LaneQueueReservationKeyV1, LaneQueueReservationRecordV3, LaneQueueReservationReleaseBarrierV2,
    LaneQueueReservationReleaseCompletionV3,
};

const RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v3";
const RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL3";
const RESERVATION_JOURNAL_FRAME_COMMIT: [u8; 8] = *b"IRQRDONE";
const MAX_FRAME_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
const FRAME_HEADER_BYTES: u64 = 12;
const FRAME_TRAILER_BYTES: u64 = Hash::LENGTH as u64 + 8;

/// Version of durable lane queue reservation records.
pub const LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 3;

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
enum LaneQueueReservationJournalFrameV3 {
    /// Complete compacted state; only emitted into a newly rewritten journal.
    Snapshot {
        /// Reservations that still own queue transactions.
        live: Vec<LaneQueueReservationRecordV3>,
        /// Exact commits retained until the pending-plan tombstone is durable.
        committed: Vec<LaneQueueReservationKeyV1>,
        /// Ordered release claims prepared against exact live reservations.
        release_barriers: Vec<LaneQueueReservationReleaseBarrierV2>,
        /// Completed releases retained until FIFO restoration is acknowledged.
        completed_releases: Vec<LaneQueueReservationReleaseCompletionV3>,
    },
    /// Atomically install one or more live reservations.
    PutBatch(Vec<LaneQueueReservationRecordV3>),
    /// Release one exact reservation back to normal queue ownership.
    ///
    /// This remains available for generic orphan reconciliation. Autonomous
    /// ordered release uses the three-phase barrier protocol below.
    Release(LaneQueueReservationKeyV1),
    /// Permanently consume one exact reservation.
    Commit(LaneQueueReservationKeyV1),
    /// Forget a commit barrier after the queue-plan tombstone is independently durable.
    ForgetCommit(LaneQueueReservationKeyV1),
    /// Release only reservations owned by this exact lane incarnation.
    Prune {
        /// Lane being retired or reconciled.
        lane_id: LaneId,
        /// Exact retired incarnation; a recreated lane has a different value.
        lane_incarnation: Hash,
    },
    /// Durably claim an exact FIFO-ordered live reservation set for release.
    PrepareRelease(LaneQueueReservationReleaseBarrierV2),
    /// Atomically move the exact prepared live records into restartable completion state.
    CompleteRelease(LaneQueueReservationReleaseCompletionV3),
    /// Forget only the completion bound to this exact release identity.
    ForgetRelease(LaneQueueReservationReleaseBarrierV2),
}

/// Replayed live reservation set.
#[derive(Clone, Debug, Default)]
pub(super) struct LaneQueueReservationReplay {
    records: Vec<LaneQueueReservationRecordV3>,
    committed: Vec<LaneQueueReservationKeyV1>,
    release_barriers: Vec<LaneQueueReservationReleaseBarrierV2>,
    completed_releases: Vec<LaneQueueReservationReleaseCompletionV3>,
}

impl LaneQueueReservationReplay {
    /// Borrow replayed live records.
    pub(super) fn records(&self) -> &[LaneQueueReservationRecordV3] {
        &self.records
    }

    /// Borrow exact commit barriers awaiting or protecting queue-plan cleanup.
    pub(super) fn committed(&self) -> &[LaneQueueReservationKeyV1] {
        &self.committed
    }

    /// Borrow exact prepared ordered-release barriers.
    pub(super) fn release_barriers(&self) -> &[LaneQueueReservationReleaseBarrierV2] {
        &self.release_barriers
    }

    /// Borrow completed releases awaiting or protecting FIFO restoration.
    pub(super) fn completed_releases(&self) -> &[LaneQueueReservationReleaseCompletionV3] {
        &self.completed_releases
    }
}

/// Append-only reservation journal with crash repair and atomic compaction.
pub(super) struct LaneQueueReservationJournal {
    path: PathBuf,
    max_bytes_before_compact: u64,
    file: File,
    terminal_frames: u64,
    poisoned: bool,
    #[cfg(test)]
    next_append_fault: Option<ReservationJournalAppendFault>,
    #[cfg(test)]
    next_compaction_fault: Option<ReservationJournalCompactionFault>,
}

impl LaneQueueReservationJournal {
    /// Open, repair, and replay a reservation journal.
    pub(super) fn open(
        path: impl AsRef<Path>,
        max_bytes_before_compact: u64,
    ) -> io::Result<(Self, LaneQueueReservationReplay)> {
        let path = path.as_ref().to_path_buf();
        prepare_regular_journal_path(&path)?;
        repair_suffix(&path)?;
        let replay = replay_path(&path)?;
        let file = open_regular_append(&path)?;
        Ok((
            Self {
                path,
                max_bytes_before_compact,
                file,
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
        records: Vec<LaneQueueReservationRecordV3>,
    ) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.append_durable(&LaneQueueReservationJournalFrameV3::PutBatch(records))
    }

    /// Durably release one exact reservation.
    pub(super) fn release(&mut self, key: LaneQueueReservationKeyV1) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV3::Release(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably commit one exact reservation.
    pub(super) fn commit(&mut self, key: LaneQueueReservationKeyV1) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV3::Commit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably forget one exact commit barrier after queue-plan cleanup.
    pub(super) fn forget_commit(&mut self, key: LaneQueueReservationKeyV1) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV3::ForgetCommit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably release every reservation for an exact lane incarnation.
    pub(super) fn prune(&mut self, lane_id: LaneId, lane_incarnation: Hash) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV3::Prune {
            lane_id,
            lane_incarnation,
        })?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably prepare an exact FIFO-ordered release claim.
    pub(super) fn prepare_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV2,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV3::PrepareRelease(barrier))
    }

    /// Durably complete an exact prepared release as one atomic journal transition.
    pub(super) fn complete_release(
        &mut self,
        completion: LaneQueueReservationReleaseCompletionV3,
    ) -> io::Result<()> {
        completion.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV3::CompleteRelease(
            completion,
        ))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    /// Durably forget only the completion for this exact full release barrier.
    pub(super) fn forget_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV2,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV3::ForgetRelease(barrier))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }

    fn append_durable(&mut self, frame: &LaneQueueReservationJournalFrameV3) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        #[cfg(test)]
        if let Some(fault) = self.next_append_fault.take() {
            let encoded = encode_frame(frame)?;
            let write_result = match fault {
                ReservationJournalAppendFault::PartialWrite => {
                    let prefix_len = encoded.len().div_ceil(2);
                    self.file.write_all(&encoded[..prefix_len])
                }
                ReservationJournalAppendFault::SyncAfterFullWrite => self.file.write_all(&encoded),
            };
            self.poisoned = true;
            write_result?;
            return Err(io::Error::other(match fault {
                ReservationJournalAppendFault::PartialWrite => {
                    "injected partial lane reservation journal write failure"
                }
                ReservationJournalAppendFault::SyncAfterFullWrite => {
                    "injected lane reservation journal sync failure after a complete write"
                }
            }));
        }
        // Encoding is a pre-write validation boundary and therefore cannot make ownership
        // ambiguous. Only poison after bytes may have reached the journal inode.
        let encoded = encode_frame(frame)?;
        if let Err(error) = self.file.write_all(&encoded) {
            self.poisoned = true;
            return Err(error);
        }
        // A successful API return is the durability boundary used by queue selection.
        if let Err(error) = self.file.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
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
        live: &[LaneQueueReservationRecordV3],
        committed: &[LaneQueueReservationKeyV1],
        release_barriers: &[LaneQueueReservationReleaseBarrierV2],
        completed_releases: &[LaneQueueReservationReleaseCompletionV3],
    ) -> io::Result<bool> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        let file_size = self.file.metadata()?.len();
        let retained_state_len = live
            .len()
            .saturating_add(committed.len())
            .saturating_add(release_barriers.len())
            .saturating_add(completed_releases.len());
        if file_size <= self.max_bytes_before_compact
            && self.terminal_frames <= u64::try_from(retained_state_len).unwrap_or(u64::MAX)
        {
            return Ok(false);
        }

        let tmp = self.path.with_extension("reservation-compact.tmp");
        reject_existing_compaction_temp(&tmp)?;
        let snapshot =
            (retained_state_len != 0).then(|| LaneQueueReservationJournalFrameV3::Snapshot {
                live: live.to_vec(),
                committed: committed.to_vec(),
                release_barriers: release_barriers.to_vec(),
                completed_releases: completed_releases.to_vec(),
            });
        if let Some(frame) = snapshot.clone() {
            validate_snapshot_frame(frame)?;
        }
        let tmp_file = {
            let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
            verify_open_regular_path(&tmp, &file)?;
            if let Some(frame) = snapshot.as_ref() {
                write_frame(&mut file, frame)?;
            }
            file.sync_all()?;
            file
        };
        verify_open_regular_path(&tmp, &tmp_file)?;
        fs::rename(&tmp, &self.path)?;
        #[cfg(test)]
        if let Some(ReservationJournalCompactionFault::AfterRenameBeforeParentSync) =
            self.next_compaction_fault.take()
        {
            self.poisoned = true;
            return Err(io::Error::other(
                "injected lane reservation journal compaction failure after rename",
            ));
        }
        // Keep the renamed inode open until the directory entry is synced. This also makes the
        // intended create -> write -> file sync -> rename -> directory sync ordering explicit.
        if let Err(error) = tmp_file.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = sync_parent_directory(&self.path) {
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
        self.file = reopened;
        self.terminal_frames = 0;
        Ok(true)
    }
}

fn validate_snapshot_frame(frame: LaneQueueReservationJournalFrameV3) -> io::Result<()> {
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

fn replay_path(path: &Path) -> io::Result<LaneQueueReservationReplay> {
    let mut records = Vec::<LaneQueueReservationRecordV3>::new();
    let mut committed = Vec::<LaneQueueReservationKeyV1>::new();
    let mut release_barriers = Vec::<LaneQueueReservationReleaseBarrierV2>::new();
    let mut completed_releases = Vec::<LaneQueueReservationReleaseCompletionV3>::new();
    for_each_frame(path, |frame| {
        apply_frame(
            &mut records,
            &mut committed,
            &mut release_barriers,
            &mut completed_releases,
            frame,
        )
    })?;
    Ok(LaneQueueReservationReplay {
        records,
        committed,
        release_barriers,
        completed_releases,
    })
}

fn apply_frame(
    records: &mut Vec<LaneQueueReservationRecordV3>,
    committed: &mut Vec<LaneQueueReservationKeyV1>,
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV2>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV3>,
    frame: LaneQueueReservationJournalFrameV3,
) -> io::Result<()> {
    match frame {
        LaneQueueReservationJournalFrameV3::Snapshot {
            live,
            committed: snapshot_committed,
            release_barriers: snapshot_release_barriers,
            completed_releases: snapshot_completed_releases,
        } => {
            let mut snapshot_live = Vec::<LaneQueueReservationRecordV3>::new();
            let mut validated_committed = Vec::<LaneQueueReservationKeyV1>::new();
            let mut validated_release_barriers = Vec::<LaneQueueReservationReleaseBarrierV2>::new();
            let mut validated_completed_releases =
                Vec::<LaneQueueReservationReleaseCompletionV3>::new();
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
        LaneQueueReservationJournalFrameV3::PutBatch(batch) => {
            apply_put_batch(
                records,
                committed,
                release_barriers,
                completed_releases,
                batch,
            )?;
        }
        LaneQueueReservationJournalFrameV3::Release(key) => {
            key.validate().map_err(invalid_data)?;
            if release_barriers
                .iter()
                .any(|barrier| barrier_contains_signed_hash(barrier, &key))
            {
                return Err(invalid_data(
                    "immediate release overlaps a prepared ordered release barrier",
                ));
            }
            // An exact tombstone is deliberately harmless when replayed twice and must never
            // remove a later reservation with the same signed hash but a different full plan.
            records.retain(|record| record.key != key);
        }
        LaneQueueReservationJournalFrameV3::Commit(key) => {
            apply_commit(
                records,
                committed,
                release_barriers,
                completed_releases,
                key,
            )?;
        }
        LaneQueueReservationJournalFrameV3::ForgetCommit(key) => {
            key.validate().map_err(invalid_data)?;
            committed.retain(|committed_key| *committed_key != key);
        }
        LaneQueueReservationJournalFrameV3::Prune {
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
        LaneQueueReservationJournalFrameV3::PrepareRelease(barrier) => {
            apply_prepare_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                barrier,
            )?;
        }
        LaneQueueReservationJournalFrameV3::CompleteRelease(completion) => {
            apply_complete_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                completion,
            )?;
        }
        LaneQueueReservationJournalFrameV3::ForgetRelease(barrier) => {
            apply_forget_release(release_barriers, completed_releases, barrier)?;
        }
    }
    Ok(())
}

fn apply_put_batch(
    records: &mut Vec<LaneQueueReservationRecordV3>,
    committed: &[LaneQueueReservationKeyV1],
    release_barriers: &[LaneQueueReservationReleaseBarrierV2],
    completed_releases: &[LaneQueueReservationReleaseCompletionV3],
    batch: Vec<LaneQueueReservationRecordV3>,
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
    records: &mut Vec<LaneQueueReservationRecordV3>,
    committed: &mut Vec<LaneQueueReservationKeyV1>,
    release_barriers: &[LaneQueueReservationReleaseBarrierV2],
    completed_releases: &[LaneQueueReservationReleaseCompletionV3],
    key: LaneQueueReservationKeyV1,
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
    records: &[LaneQueueReservationRecordV3],
    committed: &[LaneQueueReservationKeyV1],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV2>,
    completed_releases: &[LaneQueueReservationReleaseCompletionV3],
    barrier: LaneQueueReservationReleaseBarrierV2,
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
    records: &mut Vec<LaneQueueReservationRecordV3>,
    committed: &[LaneQueueReservationKeyV1],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV2>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV3>,
    completion: LaneQueueReservationReleaseCompletionV3,
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
    records: &[LaneQueueReservationRecordV3],
    committed: &[LaneQueueReservationKeyV1],
    release_barriers: &[LaneQueueReservationReleaseBarrierV2],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV3>,
    completion: LaneQueueReservationReleaseCompletionV3,
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
    left: &LaneQueueReservationReleaseCompletionV3,
    right: &LaneQueueReservationReleaseCompletionV3,
) -> bool {
    left.ordered_records.iter().any(|left_record| {
        right.ordered_records.iter().any(|right_record| {
            left_record.key.signed_transaction_hash != right_record.key.signed_transaction_hash
                && left_record.fifo_order.ordinal == right_record.fifo_order.ordinal
        })
    })
}

fn apply_forget_release(
    release_barriers: &[LaneQueueReservationReleaseBarrierV2],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV3>,
    barrier: LaneQueueReservationReleaseBarrierV2,
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
    barrier: &LaneQueueReservationReleaseBarrierV2,
    key: &LaneQueueReservationKeyV1,
) -> bool {
    barrier
        .ordered_keys
        .iter()
        .any(|barrier_key| barrier_key.signed_transaction_hash == key.signed_transaction_hash)
}

fn release_barriers_overlap(
    left: &LaneQueueReservationReleaseBarrierV2,
    right: &LaneQueueReservationReleaseBarrierV2,
) -> bool {
    left.ordered_keys
        .iter()
        .any(|key| barrier_contains_signed_hash(right, key))
}

fn encode_frame(frame: &LaneQueueReservationJournalFrameV3) -> io::Result<Vec<u8>> {
    let payload = norito::to_bytes(frame).map_err(io::Error::other)?;
    let len = u32::try_from(payload.len())
        .map_err(|_| invalid_data("lane reservation journal frame is too large"))?;
    let checksum = frame_checksum(&len.to_le_bytes(), &payload);
    let mut framed = Vec::with_capacity(
        RESERVATION_JOURNAL_FRAME_MAGIC
            .len()
            .saturating_add(4)
            .saturating_add(payload.len())
            .saturating_add(Hash::LENGTH)
            .saturating_add(RESERVATION_JOURNAL_FRAME_COMMIT.len()),
    );
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_MAGIC);
    framed.extend_from_slice(&len.to_le_bytes());
    framed.extend_from_slice(&payload);
    framed.extend_from_slice(checksum.as_ref());
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
    Ok(framed)
}

fn write_frame(file: &mut File, frame: &LaneQueueReservationJournalFrameV3) -> io::Result<()> {
    file.write_all(&encode_frame(frame)?)
}

fn frame_checksum(len: &[u8; 4], payload: &[u8]) -> Hash {
    let mut preimage = Vec::with_capacity(
        RESERVATION_JOURNAL_FRAME_DOMAIN
            .len()
            .saturating_add(len.len())
            .saturating_add(payload.len()),
    );
    preimage.extend_from_slice(RESERVATION_JOURNAL_FRAME_DOMAIN);
    preimage.extend_from_slice(len);
    preimage.extend_from_slice(payload);
    Hash::new(preimage)
}

fn repair_suffix(path: &Path) -> io::Result<()> {
    let mut file = open_regular_read_write(path)?;
    let file_len = file.metadata()?.len();
    let mut position = 0_u64;
    let mut valid_end = 0_u64;

    while position < file_len {
        file.seek(SeekFrom::Start(position))?;
        let remaining = file_len.saturating_sub(position);
        if remaining < FRAME_HEADER_BYTES {
            return truncate_suffix(&mut file, valid_end);
        }
        let mut magic = [0_u8; RESERVATION_JOURNAL_FRAME_MAGIC.len()];
        file.read_exact(&mut magic)?;
        if magic != RESERVATION_JOURNAL_FRAME_MAGIC {
            return Err(invalid_data(
                "lane reservation journal frame magic mismatch",
            ));
        }
        let mut len_bytes = [0_u8; 4];
        file.read_exact(&mut len_bytes)?;
        let payload_len = u64::from(u32::from_le_bytes(len_bytes));
        if payload_len > MAX_FRAME_PAYLOAD_BYTES {
            return Err(invalid_data(
                "lane reservation journal frame exceeds the payload limit",
            ));
        }
        let frame_len = FRAME_HEADER_BYTES
            .checked_add(payload_len)
            .and_then(|len| len.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_data("lane reservation journal frame length overflow"))?;
        let frame_end = position
            .checked_add(frame_len)
            .ok_or_else(|| invalid_data("lane reservation journal position overflow"))?;
        if frame_end > file_len {
            return truncate_suffix(&mut file, valid_end);
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
        if frame_checksum(&len_bytes, &payload).as_ref() != &checksum {
            return Err(invalid_data("lane reservation journal checksum mismatch"));
        }
        if norito::decode_from_bytes::<LaneQueueReservationJournalFrameV3>(&payload).is_err() {
            return Err(invalid_data(
                "lane reservation journal payload cannot be decoded",
            ));
        }
        valid_end = frame_end;
        position = frame_end;
    }
    Ok(())
}

fn truncate_suffix(file: &mut File, valid_end: u64) -> io::Result<()> {
    file.set_len(valid_end)?;
    file.sync_all()
}

fn for_each_frame<F>(path: &Path, mut handle: F) -> io::Result<()>
where
    F: FnMut(LaneQueueReservationJournalFrameV3) -> io::Result<()>,
{
    let file = match open_regular_read(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut reader = BufReader::new(file);
    loop {
        let mut magic = [0_u8; RESERVATION_JOURNAL_FRAME_MAGIC.len()];
        match reader.read_exact(&mut magic) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error),
        }
        if magic != RESERVATION_JOURNAL_FRAME_MAGIC {
            return Err(invalid_data(
                "lane reservation journal frame magic mismatch",
            ));
        }
        let mut len_bytes = [0_u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u64::from(u32::from_le_bytes(len_bytes));
        if len > MAX_FRAME_PAYLOAD_BYTES {
            return Err(invalid_data("lane reservation journal frame exceeds limit"));
        }
        let len = usize::try_from(len)
            .map_err(|_| invalid_data("lane reservation journal frame length exceeds usize"))?;
        let mut payload = vec![0_u8; len];
        reader.read_exact(&mut payload)?;
        let mut checksum = [0_u8; Hash::LENGTH];
        reader.read_exact(&mut checksum)?;
        let mut commit = [0_u8; RESERVATION_JOURNAL_FRAME_COMMIT.len()];
        reader.read_exact(&mut commit)?;
        if commit != RESERVATION_JOURNAL_FRAME_COMMIT {
            return Err(invalid_data(
                "lane reservation journal commit marker mismatch",
            ));
        }
        if frame_checksum(&len_bytes, &payload).as_ref() != &checksum {
            return Err(invalid_data("lane reservation journal checksum mismatch"));
        }
        let frame = norito::decode_from_bytes::<LaneQueueReservationJournalFrameV3>(&payload)
            .map_err(io::Error::other)?;
        handle(frame)?;
    }
    Ok(())
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_data(
            "lane reservation journal parent must be a non-symlink directory",
        ));
    }
    File::open(parent)?.sync_all()
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn prepare_regular_journal_path(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    fs::create_dir_all(parent)?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(invalid_data(
            "lane reservation journal parent must be a non-symlink directory",
        ));
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(invalid_data(
                    "lane reservation journal path must be a non-symlink regular file",
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
            // File contents/metadata must reach stable storage before the parent entry is synced.
            file.sync_all()?;
        }
        Err(error) => return Err(error),
    }
    // Sync unconditionally: this covers the create path and repairs an entry created by a process
    // that crashed before its directory fsync.
    sync_parent_directory(path)
}

fn reject_existing_compaction_temp(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) => {
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
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
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_data(
            "lane reservation journal path must be a non-symlink regular file",
        ));
    }
    Ok(())
}

fn verify_open_regular_path(path: &Path, file: &File) -> io::Result<()> {
    validate_regular_path(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(invalid_data(
            "opened lane reservation journal is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let path_metadata = fs::metadata(path)?;
        if opened.dev() != path_metadata.dev() || opened.ino() != path_metadata.ino() {
            return Err(invalid_data(
                "lane reservation journal path changed while it was being opened",
            ));
        }
    }
    Ok(())
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

    fn typed_hash<T>(label: &[u8]) -> HashOf<T> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn record(seed: u8, incarnation_seed: u8) -> LaneQueueReservationRecordV3 {
        let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(7));
        LaneQueueReservationRecordV3 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            key: LaneQueueReservationKeyV1 {
                version: LaneQueueReservationKeyV1::VERSION,
                signed_transaction_hash: typed_hash::<SignedTransaction>(&[seed, 1]),
                entrypoint_hash: typed_hash::<TransactionEntrypoint>(&[seed, 2]),
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
            fifo_order: LaneQueueFifoOrderV3 {
                version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                ordinal: u64::from(seed),
            },
        }
    }

    fn release_barrier(
        records: &[LaneQueueReservationRecordV3],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseBarrierV2 {
        let first = records.first().expect("release fixture is non-empty");
        LaneQueueReservationReleaseBarrierV2 {
            version: LaneQueueReservationReleaseBarrierV2::VERSION,
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
        records: &[LaneQueueReservationRecordV3],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseCompletionV3 {
        LaneQueueReservationReleaseCompletionV3 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            barrier: release_barrier(records, release_seed),
            ordered_records: records.to_vec(),
        }
    }

    fn apply_unprotected_frame(
        records: &mut Vec<LaneQueueReservationRecordV3>,
        committed: &mut Vec<LaneQueueReservationKeyV1>,
        frame: LaneQueueReservationJournalFrameV3,
    ) -> io::Result<()> {
        apply_frame(records, committed, &mut Vec::new(), &mut Vec::new(), frame)
    }

    #[test]
    fn crash_at_every_operation_frame_write_boundary_is_prefix_atomic() {
        let first = record(1, 1);
        let second = record(2, 1);
        let barrier = release_barrier(core::slice::from_ref(&first), 1);
        let completion = release_completion(core::slice::from_ref(&first), 1);
        let first_frame = encode_frame(&LaneQueueReservationJournalFrameV3::PutBatch(vec![
            first.clone(),
        ]))
        .expect("encode first frame");
        let cases = [
            (
                "put",
                LaneQueueReservationJournalFrameV3::PutBatch(vec![second]),
            ),
            (
                "release",
                LaneQueueReservationJournalFrameV3::Release(first.key),
            ),
            (
                "commit",
                LaneQueueReservationJournalFrameV3::Commit(first.key),
            ),
            (
                "forget-commit",
                LaneQueueReservationJournalFrameV3::ForgetCommit(first.key),
            ),
            (
                "prune",
                LaneQueueReservationJournalFrameV3::Prune {
                    lane_id: first.key.lane_id,
                    lane_incarnation: first.key.lane_incarnation,
                },
            ),
            (
                "snapshot",
                LaneQueueReservationJournalFrameV3::Snapshot {
                    live: Vec::new(),
                    committed: vec![first.key],
                    release_barriers: Vec::new(),
                    completed_releases: Vec::new(),
                },
            ),
            (
                "prepare-release",
                LaneQueueReservationJournalFrameV3::PrepareRelease(barrier.clone()),
            ),
            (
                "complete-release",
                LaneQueueReservationJournalFrameV3::CompleteRelease(completion),
            ),
            (
                "forget-release",
                LaneQueueReservationJournalFrameV3::ForgetRelease(barrier),
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
        let first_frame = encode_frame(&LaneQueueReservationJournalFrameV3::PutBatch(vec![
            first.clone(),
        ]))
        .expect("encode first");
        let mut corrupt = encode_frame(&LaneQueueReservationJournalFrameV3::PutBatch(vec![second]))
            .expect("encode second");
        let third_frame = encode_frame(&LaneQueueReservationJournalFrameV3::PutBatch(vec![third]))
            .expect("encode third");
        let corrupt_index = corrupt.len() - 1;
        corrupt[corrupt_index] ^= 0x80;
        let mut file = File::create(&path).expect("create journal");
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
                "{label} evidence must not be rewritten as a V3 journal"
            );
        }
    }

    #[test]
    fn duplicate_exact_replay_is_idempotent_but_conflicting_owner_is_rejected() {
        let exact = record(1, 1);
        let mut records = Vec::new();
        let mut committed = Vec::new();
        apply_unprotected_frame(
            &mut records,
            &mut committed,
            LaneQueueReservationJournalFrameV3::PutBatch(vec![exact.clone(), exact.clone()]),
        )
        .expect("duplicate exact record");
        assert_eq!(records, vec![exact.clone()]);

        let mut conflicting = exact;
        conflicting.key.reservation_owner_hash = Hash::new(b"conflicting-owner");
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV3::PutBatch(vec![conflicting]),
            )
            .is_err()
        );

        let mut conflicting_plan = records[0].clone();
        conflicting_plan.key.routing_plan_digest = Hash::new(b"conflicting-plan");
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV3::PutBatch(vec![conflicting_plan]),
            )
            .is_err()
        );

        let mut conflicting_fifo_order = record(3, 1);
        conflicting_fifo_order.fifo_order = records[0].fifo_order;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV3::PutBatch(vec![conflicting_fifo_order]),
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
                LaneQueueReservationJournalFrameV3::PutBatch(vec![participant]),
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
                LaneQueueReservationJournalFrameV3::PutBatch(vec![legacy]),
            )
            .is_err()
        );
        let mut zero = record(5, 1);
        zero.fifo_order.ordinal = 0;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV3::PutBatch(vec![zero]),
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
                LaneQueueReservationJournalFrameV3::CompleteRelease(completion.clone()),
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
            LaneQueueReservationJournalFrameV3::PrepareRelease(barrier.clone()),
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
                LaneQueueReservationJournalFrameV3::PrepareRelease(conflicting_barrier),
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
                LaneQueueReservationJournalFrameV3::CompleteRelease(wrong_records),
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
            LaneQueueReservationJournalFrameV3::CompleteRelease(completion.clone()),
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
                LaneQueueReservationJournalFrameV3::PutBatch(vec![recreated]),
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
            LaneQueueReservationJournalFrameV3::ForgetRelease(stale_forget),
        )
        .expect("stale full identity is a harmless no-op");
        assert_eq!(completed, vec![completion]);

        apply_frame(
            &mut live,
            &mut committed,
            &mut barriers,
            &mut completed,
            LaneQueueReservationJournalFrameV3::ForgetRelease(barrier),
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
                LaneQueueReservationJournalFrameV3::Snapshot {
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
            LaneQueueReservationJournalFrameV3::Release(old.key),
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
            LaneQueueReservationJournalFrameV3::Commit(old.key),
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
                LaneQueueReservationJournalFrameV3::PutBatch(vec![replacement]),
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
            LaneQueueReservationJournalFrameV3::Prune {
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
                .expect("compact all V3 release state")
        );
        drop(journal);

        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, 1).expect("replay compacted V3 snapshot");
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
}
