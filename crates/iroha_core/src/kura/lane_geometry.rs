//! Crash-atomic Kura lane-geometry transitions.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{ErrorKind, Read, Write},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
};

use iroha_config::{
    kura::FsyncMode,
    parameters::actual::{LaneConfig, LaneConfigEntry},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::BlockHeader, nexus::LaneId};
use norito::codec::{Decode, Encode};

use super::{BlockStore, Error, Kura, Result, create_dir_all_with_context, sync_dir};

const LEGACY_JOURNAL_VERSION: u8 = 1;
const JOURNAL_VERSION: u8 = 2;
const MARKER_VERSION: u8 = 1;
const CHECKPOINT_VERSION: u8 = 1;
const JOURNAL_FILE_NAME: &str = "lane_geometry_journal.norito";
const JOURNAL_TEMP_FILE_NAME: &str = "lane_geometry_journal.norito.tmp";
const MARKER_FILE_NAME: &str = ".lane-incarnation.norito";
const TRANSITION_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-transition:v1\0";
const CATALOG_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-catalog:v1\0";
const CHECKPOINT_DOMAIN: &[u8] = b"iroha:kura:lane-geometry-checkpoint:v1\0";

const GC_FAIL_AFTER_COMPACTION_INTENT: usize = 1;
const GC_FAIL_AFTER_ARCHIVE_DELETION: usize = 2;
const GC_FAIL_AFTER_COMPLETION: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneGeometryPhase {
    Intent,
    FilesApplied,
    CatalogPublished,
    RolledBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneGeometryOperationKind {
    Create,
    Retire,
    Replace,
    Relabel,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryBinding {
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
    blocks_path: String,
    merge_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryOperation {
    kind: LaneGeometryOperationKind,
    lane_id: LaneId,
    previous: Option<LaneGeometryBinding>,
    updated: Option<LaneGeometryBinding>,
    archived_blocks_path: String,
    archived_merge_path: String,
    unpublished_blocks_path: String,
    unpublished_merge_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryIntent {
    transition_id: Hash,
    previous_catalog: Hash,
    updated_catalog: Hash,
    previous_bindings: Vec<LaneGeometryBinding>,
    updated_bindings: Vec<LaneGeometryBinding>,
    phase: LaneGeometryPhase,
    operations: Vec<LaneGeometryOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometrySnapshotCheckpoint {
    version: u8,
    snapshot_height: u64,
    snapshot_block_hash: Option<HashOf<BlockHeader>>,
    snapshot_state_hash: Hash,
    catalog: Hash,
    transition_previous_catalog: Option<Hash>,
    transition_id: Option<Hash>,
    bindings: Vec<LaneGeometryBinding>,
    commitment: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryPendingArchiveGc {
    transition_id: Hash,
    previous_catalog: Hash,
    updated_catalog: Hash,
    lane_ids: Vec<LaneId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LegacyLaneGeometryJournalV1 {
    version: u8,
    records: Vec<LaneGeometryIntent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneGeometryJournal {
    version: u8,
    checkpoint: Option<LaneGeometrySnapshotCheckpoint>,
    pending_archive_gc: Vec<LaneGeometryPendingArchiveGc>,
    records: Vec<LaneGeometryIntent>,
}

impl Default for LaneGeometryJournal {
    fn default() -> Self {
        Self {
            version: JOURNAL_VERSION,
            checkpoint: None,
            pending_archive_gc: Vec::new(),
            records: Vec::new(),
        }
    }
}

/// Result of one durable snapshot checkpoint and archive-GC pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneGeometryGcSummary {
    /// Geometry transition records removed from the recoverable journal prefix.
    pub(crate) compacted_transitions: usize,
    /// Transition archive roots durably removed in this pass.
    pub(crate) removed_archive_roots: usize,
    /// Regular-file bytes removed from authenticated transition archive roots.
    pub(crate) reclaimed_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct LaneIncarnationMarker {
    version: u8,
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
}

impl Kura {
    /// Apply one lane geometry transition under a durable, replayable intent.
    ///
    /// The intent remains in the journal after publication so a snapshot that
    /// predates several committed transitions can roll their filesystem effects
    /// back before block replay and deterministically reapply them afterwards.
    pub(crate) fn apply_lane_geometry_transition(
        &self,
        previous: &LaneConfig,
        updated: &LaneConfig,
        previous_incarnations: &BTreeMap<LaneId, Hash>,
        updated_incarnations: &BTreeMap<LaneId, Hash>,
        previous_activation_heights: &BTreeMap<LaneId, u64>,
        updated_activation_heights: &BTreeMap<LaneId, u64>,
        replaced_lane_ids: &BTreeSet<LaneId>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();

        let previous_bindings =
            self.geometry_bindings(previous, previous_incarnations, previous_activation_heights)?;
        let updated_bindings =
            self.geometry_bindings(updated, updated_incarnations, updated_activation_heights)?;
        let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
        let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        self.reconcile_lane_geometry_history(&mut journal, previous_catalog)?;
        self.ensure_authoritative_lane_markers(
            previous,
            previous_incarnations,
            previous_activation_heights,
        )?;
        if previous_catalog == updated_catalog {
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return self.write_lane_geometry_journal(&journal);
        }

        let transition_id = geometry_transition_id(previous_catalog, updated_catalog);
        if let Some(existing) = journal
            .records
            .iter_mut()
            .find(|record| record.transition_id == transition_id)
        {
            self.apply_geometry_operations_forward(&existing.operations)?;
            existing.phase = LaneGeometryPhase::FilesApplied;
            self.write_lane_geometry_journal(&journal)?;
            *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
            return Ok(());
        }

        let operations = self.build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            replaced_lane_ids,
        )?;
        let intent = LaneGeometryIntent {
            transition_id,
            previous_catalog,
            updated_catalog,
            previous_bindings,
            updated_bindings,
            phase: LaneGeometryPhase::Intent,
            operations,
        };
        journal.records.push(intent);
        self.write_lane_geometry_journal(&journal)?;

        let record_index = journal.records.len() - 1;
        if let Err(error) =
            self.apply_geometry_operations_forward(&journal.records[record_index].operations)
        {
            let rollback =
                self.apply_geometry_operations_rollback(&journal.records[record_index].operations);
            journal.records[record_index].phase = LaneGeometryPhase::RolledBack;
            let journal_result = self.write_lane_geometry_journal(&journal);
            if let Err(rollback_error) = rollback {
                return Err(Error::IO(
                    std::io::Error::other(format!(
                        "lane geometry apply failed ({error}); rollback failed ({rollback_error})"
                    )),
                    self.lane_geometry_journal_path(),
                ));
            }
            journal_result?;
            return Err(error);
        }

        journal.records[record_index].phase = LaneGeometryPhase::FilesApplied;
        self.write_lane_geometry_journal(&journal)?;
        *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(updated);
        Ok(())
    }

    /// Mark the transition targeting the authoritative catalog as published.
    pub(crate) fn mark_lane_geometry_catalog_published(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        #[cfg(test)]
        if self
            .fail_next_lane_geometry_publication
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(self.geometry_error(
                ErrorKind::Other,
                "lane geometry publication failed for test injection",
            ));
        }
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let fingerprint = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        let Some(record) = journal
            .records
            .iter_mut()
            .rev()
            .find(|record| record.updated_catalog == fingerprint)
        else {
            return Ok(());
        };
        record.phase = LaneGeometryPhase::CatalogPublished;
        self.write_lane_geometry_journal(&journal)
    }

    #[cfg(test)]
    pub(crate) fn fail_next_lane_geometry_publication_for_test(&self) {
        self.fail_next_lane_geometry_publication
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Recover every retained geometry intent against a restored authoritative catalog.
    pub(crate) fn recover_lane_geometry_journal(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<()> {
        if self.store_root.as_os_str().is_empty() {
            *self.lane_storage_entries.lock() =
                Self::lane_storage_entries_from_config(authoritative);
            return Ok(());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let fingerprint = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let _ = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        self.reconcile_lane_geometry_history(&mut journal, fingerprint)?;
        self.ensure_authoritative_lane_markers(authoritative, incarnations, activation_heights)?;
        *self.lane_storage_entries.lock() = Self::lane_storage_entries_from_config(authoritative);
        self.write_lane_geometry_journal(&journal)
    }

    /// Checkpoint recoverable lane geometry after a complete snapshot bundle is durable.
    ///
    /// The caller must invoke this only after the snapshot data, digest, signature, Merkle
    /// metadata, and snapshot directory entry have all been synchronized. Kura independently
    /// joins the supplied snapshot identity to its canonical block hash and WSV checkpoint before
    /// compacting any transition history. Archive deletion is then replayable from the compacted
    /// journal and can never run ahead of that checkpoint.
    ///
    /// # Errors
    /// Returns an error without deleting recovery evidence when the snapshot identity is stale,
    /// does not match the durable block/WSV checkpoint, names an unreachable geometry catalog, or
    /// archive contents fail authenticated path/type validation.
    pub(crate) fn checkpoint_lane_geometry_after_durable_snapshot(
        &self,
        authoritative: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
    ) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        if snapshot_state_hash.as_ref().iter().all(|byte| *byte == 0)
            || (snapshot_height == 0) != snapshot_block_hash.is_none()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "snapshot geometry checkpoint has an invalid height, block hash, or state hash",
            ));
        }
        if snapshot_height > 0 {
            let height = usize::try_from(snapshot_height)?;
            let height = NonZeroUsize::new(height).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidInput,
                    "snapshot geometry checkpoint height is not representable",
                )
            })?;
            let expected_block_hash = snapshot_block_hash.expect("non-zero height has block hash");
            let durable_block_hash = self.get_durable_block_hash(height).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::NotFound,
                    "snapshot geometry checkpoint has no durable canonical block",
                )
            })?;
            if durable_block_hash != expected_block_hash {
                return Err(Error::BlockHeightConflict {
                    height: snapshot_height,
                    expected: durable_block_hash,
                    actual: expected_block_hash,
                });
            }
            let checkpoint = self.wsv_checkpoint(snapshot_height)?.ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::NotFound,
                    "snapshot geometry checkpoint has no durable WSV checkpoint",
                )
            })?;
            if checkpoint.state_hash() != snapshot_state_hash {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "snapshot geometry state hash does not match the durable WSV checkpoint",
                ));
            }
        }

        let bindings = self.geometry_bindings(authoritative, incarnations, activation_heights)?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        self.checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
        )
    }

    /// Resume archive deletions that were proven safe by an already durable checkpoint.
    ///
    /// This never creates a new checkpoint or broadens the deletable set, so storage-budget
    /// maintenance may call it safely. A missing/corrupt journal or tampered archive fails closed.
    pub(super) fn resume_proven_lane_geometry_archive_gc(
        &self,
    ) -> Result<LaneGeometryGcSummary> {
        if self.store_root.as_os_str().is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let mut journal = self.read_lane_geometry_journal()?;
        self.finish_pending_lane_geometry_gc_locked(&mut journal)
    }

    fn checkpoint_lane_geometry_with_proven_snapshot(
        &self,
        bindings: Vec<LaneGeometryBinding>,
        snapshot_height: u64,
        snapshot_block_hash: Option<HashOf<BlockHeader>>,
        snapshot_state_hash: Hash,
    ) -> Result<LaneGeometryGcSummary> {
        let catalog = geometry_catalog_fingerprint(&bindings);
        let mut journal = self.read_lane_geometry_journal()?;
        let mut summary = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;

        if let Some(existing) = journal.checkpoint.as_ref() {
            if snapshot_height < existing.snapshot_height {
                return Err(self.geometry_error(
                    ErrorKind::InvalidInput,
                    "refusing a stale lane geometry snapshot checkpoint",
                ));
            }
            if snapshot_height == existing.snapshot_height
                && (snapshot_block_hash != existing.snapshot_block_hash
                    || snapshot_state_hash != existing.snapshot_state_hash
                    || catalog != existing.catalog
                    || bindings != existing.bindings)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry snapshot checkpoint identity changed at the same height",
                ));
            }
        }

        if journal.records.is_empty() && journal.checkpoint.is_none() {
            // There is no transition history or recovery archive to compact. Avoid creating a
            // sidecar solely for the static genesis geometry.
            return Ok(summary);
        }

        let prune_count = journal
            .records
            .iter()
            .rposition(|record| record.updated_catalog == catalog)
            .map_or_else(
                || {
                    if journal
                        .records
                        .first()
                        .is_some_and(|record| record.previous_catalog == catalog)
                        || journal
                            .checkpoint
                            .as_ref()
                            .is_some_and(|checkpoint| checkpoint.catalog == catalog)
                    {
                        Ok(0)
                    } else {
                        Err(self.geometry_error(
                            ErrorKind::InvalidData,
                            "snapshot geometry catalog is not reachable from retained transition history",
                        ))
                    }
                },
                |index| Ok(index + 1),
            )?;

        let exact_bindings_match = if prune_count > 0 {
            journal.records[prune_count - 1].updated_bindings == bindings
        } else if let Some(first) = journal.records.first() {
            first.previous_bindings == bindings
        } else {
            journal
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.bindings == bindings)
        };
        if !exact_bindings_match
            || journal.records[..prune_count]
                .iter()
                .any(|record| record.phase != LaneGeometryPhase::CatalogPublished)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "snapshot geometry does not exactly match published transition history",
            ));
        }
        if bindings
            .iter()
            .any(|binding| binding.activation_height > snapshot_height)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidInput,
                "snapshot predates an active lane incarnation",
            ));
        }

        let (transition_previous_catalog, transition_id) = if prune_count > 0 {
            let latest = &journal.records[prune_count - 1];
            (Some(latest.previous_catalog), Some(latest.transition_id))
        } else {
            journal.checkpoint.as_ref().map_or((None, None), |checkpoint| {
                (
                    checkpoint.transition_previous_catalog,
                    checkpoint.transition_id,
                )
            })
        };
        let checkpoint = lane_geometry_snapshot_checkpoint(
            snapshot_height,
            snapshot_block_hash,
            snapshot_state_hash,
            bindings,
            transition_previous_catalog,
            transition_id,
        );
        self.validate_lane_geometry_checkpoint(&checkpoint)?;

        let pending_archive_gc = journal.records[..prune_count]
            .iter()
            .map(|record| LaneGeometryPendingArchiveGc {
                transition_id: record.transition_id,
                previous_catalog: record.previous_catalog,
                updated_catalog: record.updated_catalog,
                lane_ids: record
                    .operations
                    .iter()
                    .map(|operation| operation.lane_id)
                    .collect(),
            })
            .collect::<Vec<_>>();
        journal.records.drain(..prune_count);
        journal.checkpoint = Some(checkpoint);
        journal.pending_archive_gc = pending_archive_gc;
        self.validate_lane_geometry_journal(&journal)?;

        if matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "lane geometry archive GC requires durable sidecar fsync",
            ));
        }
        self.write_lane_geometry_journal(&journal)?;
        summary.compacted_transitions = summary
            .compacted_transitions
            .saturating_add(prune_count);
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT)?;

        let finished = self.finish_pending_lane_geometry_gc_locked(&mut journal)?;
        summary.removed_archive_roots = summary
            .removed_archive_roots
            .saturating_add(finished.removed_archive_roots);
        summary.reclaimed_bytes = summary
            .reclaimed_bytes
            .saturating_add(finished.reclaimed_bytes);
        Ok(summary)
    }

    fn finish_pending_lane_geometry_gc_locked(
        &self,
        journal: &mut LaneGeometryJournal,
    ) -> Result<LaneGeometryGcSummary> {
        if journal.pending_archive_gc.is_empty() {
            return Ok(LaneGeometryGcSummary::default());
        }
        if matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "lane geometry archive GC requires durable sidecar fsync",
            ));
        }
        self.validate_lane_geometry_journal(journal)?;
        let pending = journal.pending_archive_gc.clone();
        let mut summary = LaneGeometryGcSummary::default();
        for archive in &pending {
            let (bytes, existed) = self.remove_authenticated_geometry_archive(archive)?;
            summary.reclaimed_bytes = summary.reclaimed_bytes.saturating_add(bytes);
            summary.removed_archive_roots = summary
                .removed_archive_roots
                .saturating_add(usize::from(existed));
        }
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_ARCHIVE_DELETION)?;
        journal.pending_archive_gc.clear();
        self.validate_lane_geometry_journal(journal)?;
        self.write_lane_geometry_journal(journal)?;
        let _ = self.refresh_disk_usage_bytes()?;
        self.fail_lane_geometry_gc_stage_for_test(GC_FAIL_AFTER_COMPLETION)?;
        Ok(summary)
    }

    #[cfg(test)]
    fn fail_next_lane_geometry_gc_at_stage_for_test(&self, stage: usize) {
        self.fail_lane_geometry_gc_stage
            .store(stage, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    fn fail_lane_geometry_gc_stage_for_test(&self, stage: usize) -> Result<()> {
        if self
            .fail_lane_geometry_gc_stage
            .compare_exchange(
                stage,
                0,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            return Err(self.geometry_error(
                ErrorKind::Interrupted,
                "lane geometry archive GC crash boundary injected for testing",
            ));
        }
        Ok(())
    }

    #[cfg(not(test))]
    fn fail_lane_geometry_gc_stage_for_test(&self, _stage: usize) -> Result<()> {
        Ok(())
    }

    fn reconcile_lane_geometry_history(
        &self,
        journal: &mut LaneGeometryJournal,
        authoritative: Hash,
    ) -> Result<()> {
        if journal.records.is_empty() {
            if journal
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.catalog != authoritative)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "authoritative geometry does not match the compacted snapshot checkpoint",
                ));
            }
            return Ok(());
        }
        for pair in journal.records.windows(2) {
            if pair[0].updated_catalog != pair[1].previous_catalog {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal transition chain is not contiguous",
                ));
            }
        }
        let applied_count = if journal.records[0].previous_catalog == authoritative {
            0
        } else {
            journal
                .records
                .iter()
                .rposition(|record| record.updated_catalog == authoritative)
                .map(|index| index + 1)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal does not contain the authoritative catalog",
                    )
                })?
        };

        for record in journal.records[..applied_count].iter_mut() {
            self.apply_geometry_operations_forward(&record.operations)?;
            record.phase = LaneGeometryPhase::CatalogPublished;
        }
        for record in journal.records[applied_count..].iter_mut().rev() {
            self.apply_geometry_operations_rollback(&record.operations)?;
            record.phase = LaneGeometryPhase::RolledBack;
        }
        Ok(())
    }

    fn build_geometry_operations(
        &self,
        transition_id: Hash,
        previous: &[LaneGeometryBinding],
        updated: &[LaneGeometryBinding],
        replaced_lane_ids: &BTreeSet<LaneId>,
    ) -> Result<Vec<LaneGeometryOperation>> {
        let previous_by_lane = previous
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let updated_by_lane = updated
            .iter()
            .map(|binding| (binding.lane_id, binding))
            .collect::<BTreeMap<_, _>>();
        let lane_ids = previous_by_lane
            .keys()
            .chain(updated_by_lane.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let transition_hex = hex::encode(transition_id.as_ref());
        let mut operations = Vec::new();
        for lane_id in &lane_ids {
            let before = previous_by_lane
                .get(lane_id)
                .map(|binding| (*binding).clone());
            let after = updated_by_lane
                .get(lane_id)
                .map(|binding| (*binding).clone());
            if replaced_lane_ids.contains(lane_id)
                && before
                    .as_ref()
                    .zip(after.as_ref())
                    .is_some_and(|(previous, updated)| {
                        previous.incarnation == updated.incarnation
                            && previous.activation_height == updated.activation_height
                    })
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidInput,
                    "lane replacement must use a fresh incarnation and activation",
                ));
            }
            let kind = match (&before, &after) {
                (None, Some(_)) => LaneGeometryOperationKind::Create,
                (Some(_), None) => LaneGeometryOperationKind::Retire,
                (Some(before), Some(after))
                    if replaced_lane_ids.contains(lane_id)
                        || before.incarnation != after.incarnation
                        || before.activation_height != after.activation_height =>
                {
                    LaneGeometryOperationKind::Replace
                }
                (Some(before), Some(after))
                    if before.blocks_path != after.blocks_path
                        || before.merge_path != after.merge_path =>
                {
                    LaneGeometryOperationKind::Relabel
                }
                _ => continue,
            };
            let archive_root = format!(
                "retired/lane_geometry/{transition_hex}/lane_{:010}",
                lane_id.as_u32()
            );
            let operation = LaneGeometryOperation {
                kind,
                lane_id: *lane_id,
                previous: before,
                updated: after,
                archived_blocks_path: format!("{archive_root}/previous_blocks"),
                archived_merge_path: format!("{archive_root}/previous_merge.log"),
                unpublished_blocks_path: format!("{archive_root}/unpublished_blocks"),
                unpublished_merge_path: format!("{archive_root}/unpublished_merge.log"),
            };
            self.preflight_geometry_operation(&operation)?;
            operations.push(operation);
        }
        Ok(operations)
    }

    fn preflight_geometry_operation(&self, operation: &LaneGeometryOperation) -> Result<()> {
        for (relative, directory) in [
            (&operation.archived_blocks_path, true),
            (&operation.archived_merge_path, false),
            (&operation.unpublished_blocks_path, true),
            (&operation.unpublished_merge_path, false),
        ] {
            let retained = self.resolve_relative_path(relative)?;
            if self.validate_path_kind(&retained, directory)? {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "lane geometry archive target already exists",
                    ),
                    retained,
                ));
            }
        }
        if let Some(previous) = operation.previous.as_ref() {
            self.validate_binding_paths(previous)?;
            if self.binding_blocks_path(previous).exists() {
                self.require_lane_marker(previous)?;
            }
        }
        if let Some(updated) = operation.updated.as_ref() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.validate_binding_paths(updated)?;
                    let blocks = self.binding_blocks_path(updated);
                    let merge = self.binding_merge_path(updated);
                    if blocks.exists() || merge.exists() {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane storage already exists at a create target",
                        ));
                    }
                }
                LaneGeometryOperationKind::Replace | LaneGeometryOperationKind::Relabel => {
                    if operation
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.blocks_path != updated.blocks_path)
                        && self.binding_blocks_path(updated).exists()
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane geometry target block path already exists",
                        ));
                    }
                    if operation
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.merge_path != updated.merge_path)
                        && self.binding_merge_path(updated).exists()
                    {
                        return Err(self.geometry_error(
                            ErrorKind::AlreadyExists,
                            "lane geometry target merge path already exists",
                        ));
                    }
                }
                LaneGeometryOperationKind::Retire => {}
            }
        }
        Ok(())
    }

    fn apply_geometry_operations_forward(
        &self,
        operations: &[LaneGeometryOperation],
    ) -> Result<()> {
        for operation in operations {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    self.restore_unpublished_or_provision(operation)?;
                }
                LaneGeometryOperationKind::Retire => {
                    let previous = operation
                        .previous
                        .as_ref()
                        .expect("retire has previous binding");
                    self.archive_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Replace => {
                    let previous = operation
                        .previous
                        .as_ref()
                        .expect("replace has previous binding");
                    self.archive_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                    self.restore_unpublished_or_provision(operation)?;
                }
                LaneGeometryOperationKind::Relabel => {
                    self.move_geometry_binding(
                        operation.previous.as_ref().expect("relabel previous"),
                        operation.updated.as_ref().expect("relabel updated"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn apply_geometry_operations_rollback(
        &self,
        operations: &[LaneGeometryOperation],
    ) -> Result<()> {
        for operation in operations.iter().rev() {
            match operation.kind {
                LaneGeometryOperationKind::Create => {
                    let updated = operation.updated.as_ref().expect("create updated");
                    self.archive_geometry_binding(
                        updated,
                        &operation.unpublished_blocks_path,
                        &operation.unpublished_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Retire => {
                    let previous = operation.previous.as_ref().expect("retire previous");
                    self.restore_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Replace => {
                    let updated = operation.updated.as_ref().expect("replace updated");
                    self.archive_geometry_binding(
                        updated,
                        &operation.unpublished_blocks_path,
                        &operation.unpublished_merge_path,
                    )?;
                    let previous = operation.previous.as_ref().expect("replace previous");
                    self.restore_geometry_binding(
                        previous,
                        &operation.archived_blocks_path,
                        &operation.archived_merge_path,
                    )?;
                }
                LaneGeometryOperationKind::Relabel => {
                    self.move_geometry_binding(
                        operation.updated.as_ref().expect("relabel updated"),
                        operation.previous.as_ref().expect("relabel previous"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn archive_geometry_binding(
        &self,
        binding: &LaneGeometryBinding,
        archived_blocks: &str,
        archived_merge: &str,
    ) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        if blocks == *self.active_blocks_dir.lock() {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "refusing to archive the active primary block store",
            ));
        }
        let merge = self.binding_merge_path(binding);
        if merge == *self.active_merge_path.lock() {
            return Err(self.geometry_error(
                ErrorKind::PermissionDenied,
                "refusing to archive the active primary merge log",
            ));
        }
        self.move_geometry_path(&blocks, &self.resolve_relative_path(archived_blocks)?, true)?;
        self.move_geometry_path(&merge, &self.resolve_relative_path(archived_merge)?, false)
    }

    fn restore_geometry_binding(
        &self,
        binding: &LaneGeometryBinding,
        archived_blocks: &str,
        archived_merge: &str,
    ) -> Result<()> {
        self.move_geometry_path(
            &self.resolve_relative_path(archived_blocks)?,
            &self.binding_blocks_path(binding),
            true,
        )?;
        self.move_geometry_path(
            &self.resolve_relative_path(archived_merge)?,
            &self.binding_merge_path(binding),
            false,
        )?;
        self.require_lane_marker(binding)
    }

    fn restore_unpublished_or_provision(&self, operation: &LaneGeometryOperation) -> Result<()> {
        let updated = operation
            .updated
            .as_ref()
            .expect("create and replace operations have an updated binding");
        let unpublished_blocks = self.resolve_relative_path(&operation.unpublished_blocks_path)?;
        let unpublished_merge = self.resolve_relative_path(&operation.unpublished_merge_path)?;
        if self.validate_path_kind(&unpublished_blocks, true)?
            || self.validate_path_kind(&unpublished_merge, false)?
        {
            self.restore_geometry_binding(
                updated,
                &operation.unpublished_blocks_path,
                &operation.unpublished_merge_path,
            )
        } else {
            self.provision_geometry_binding(updated)
        }
    }

    fn move_geometry_binding(
        &self,
        previous: &LaneGeometryBinding,
        updated: &LaneGeometryBinding,
    ) -> Result<()> {
        let old_blocks = self.binding_blocks_path(previous);
        let new_blocks = self.binding_blocks_path(updated);
        let old_merge = self.binding_merge_path(previous);
        let new_merge = self.binding_merge_path(updated);
        self.move_geometry_path(&old_blocks, &new_blocks, true)?;
        self.move_geometry_path(&old_merge, &new_merge, false)?;
        self.retarget_active_geometry_paths(&old_blocks, &new_blocks, &old_merge, &new_merge)?;
        self.require_lane_marker(updated)
    }

    fn remove_authenticated_geometry_archive(
        &self,
        pending: &LaneGeometryPendingArchiveGc,
    ) -> Result<(u64, bool)> {
        let transition_hex = hex::encode(pending.transition_id.as_ref());
        let root = self.resolve_relative_path(&format!(
            "retired/lane_geometry/{transition_hex}"
        ))?;
        if !self.validate_path_kind(&root, true)? {
            return Ok((0, false));
        }

        let expected_lane_dirs = pending
            .lane_ids
            .iter()
            .map(|lane_id| (format!("lane_{:010}", lane_id.as_u32()), *lane_id))
            .collect::<BTreeMap<_, _>>();
        let mut bytes = 0_u64;
        let entries = fs::read_dir(&root).map_err(|error| Error::IO(error, root.clone()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, root.clone()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive contains a non-UTF-8 entry",
                    ),
                    path.clone(),
                )
            })?;
            if !expected_lane_dirs.contains_key(&name) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive contains an unauthenticated lane directory",
                    ),
                    path,
                ));
            }
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry archive lane entry is not a regular directory",
                    ),
                    path,
                ));
            }
            bytes = bytes.saturating_add(self.authenticated_geometry_lane_archive_bytes(&path)?);
        }

        fs::remove_dir_all(&root).map_err(|error| Error::IO(error, root.clone()))?;
        self.sync_geometry_parent(root.parent())?;
        Ok((bytes, true))
    }

    fn authenticated_geometry_lane_archive_bytes(&self, lane_root: &Path) -> Result<u64> {
        self.validate_path_kind(lane_root, true)?;
        let mut bytes = 0_u64;
        let entries = fs::read_dir(lane_root)
            .map_err(|error| Error::IO(error, lane_root.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, lane_root.to_path_buf()))?;
            let path = entry.path();
            let name = entry.file_name().into_string().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry lane archive contains a non-UTF-8 entry",
                    ),
                    path.clone(),
                )
            })?;
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            match name.as_str() {
                "previous_blocks" | "unpublished_blocks"
                    if file_type.is_dir() && !file_type.is_symlink() =>
                {
                    bytes = bytes
                        .saturating_add(Self::regular_geometry_archive_tree_bytes(&path)?);
                }
                "previous_merge.log" | "unpublished_merge.log"
                    if file_type.is_file() && !file_type.is_symlink() =>
                {
                    bytes = bytes.saturating_add(
                        entry
                            .metadata()
                            .map_err(|error| Error::IO(error, path.clone()))?
                            .len(),
                    );
                }
                _ => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry lane archive contains an unauthenticated or unsafe entry",
                        ),
                        path,
                    ));
                }
            }
        }
        Ok(bytes)
    }

    fn regular_geometry_archive_tree_bytes(root: &Path) -> Result<u64> {
        let metadata = fs::symlink_metadata(root)
            .map_err(|error| Error::IO(error, root.to_path_buf()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry block archive root is not a regular directory",
                ),
                root.to_path_buf(),
            ));
        }
        let mut bytes = 0_u64;
        let entries =
            fs::read_dir(root).map_err(|error| Error::IO(error, root.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, root.to_path_buf()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if file_type.is_symlink() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry block archive contains a symbolic link",
                    ),
                    path,
                ));
            }
            if file_type.is_file() {
                bytes = bytes.saturating_add(
                    entry
                        .metadata()
                        .map_err(|error| Error::IO(error, path.clone()))?
                        .len(),
                );
            } else if file_type.is_dir() {
                bytes = bytes.saturating_add(Self::regular_geometry_archive_tree_bytes(&path)?);
            } else {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "lane geometry block archive contains a non-regular filesystem entry",
                    ),
                    path,
                ));
            }
        }
        Ok(bytes)
    }

    fn move_geometry_path(&self, source: &Path, target: &Path, directory: bool) -> Result<()> {
        let source_exists = self.validate_path_kind(source, directory)?;
        let target_exists = self.validate_path_kind(target, directory)?;
        match (source_exists, target_exists) {
            (false, false) | (false, true) => return Ok(()),
            (true, true) => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "both source and target exist during idempotent geometry move",
                    ),
                    target.to_path_buf(),
                ));
            }
            (true, false) => {}
        }
        if let Some(parent) = target.parent() {
            create_dir_all_with_context(parent)?;
        }
        fs::rename(source, target).map_err(|error| Error::IO(error, source.to_path_buf()))?;
        self.sync_geometry_parent(source.parent())?;
        if source.parent() != target.parent() {
            self.sync_geometry_parent(target.parent())?;
        }
        Ok(())
    }

    fn provision_geometry_binding(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        if blocks.exists() {
            self.require_lane_marker(binding)?;
        } else {
            if let Some(parent) = blocks.parent() {
                create_dir_all_with_context(parent)?;
            }
            let mut store = BlockStore::new(&blocks);
            store.create_files_if_they_do_not_exist()?;
            self.write_lane_marker(binding)?;
        }
        if let Some(parent) = merge.parent() {
            create_dir_all_with_context(parent)?;
        }
        if !merge.exists() {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&merge)
                .map_err(|error| Error::IO(error, merge.clone()))?;
            self.sync_geometry_parent(merge.parent())?;
        } else {
            self.validate_path_kind(&merge, false)?;
        }
        Ok(())
    }

    fn ensure_authoritative_lane_markers(
        &self,
        lane_config: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<()> {
        for entry in lane_config.entries() {
            let binding = self.geometry_binding(entry, incarnations, activation_heights)?;
            let blocks = self.binding_blocks_path(&binding);
            let merge = self.binding_merge_path(&binding);
            let blocks_exists = self.validate_path_kind(&blocks, true)?;
            let merge_exists = self.validate_path_kind(&merge, false)?;
            if blocks_exists != merge_exists {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "active lane block and merge storage are only partially present",
                ));
            }
            if !blocks_exists {
                if binding.activation_height != 0 {
                    return Err(self.geometry_error(
                        ErrorKind::NotFound,
                        "active dynamic lane storage is missing; refusing to provision an empty replacement",
                    ));
                }
                self.provision_geometry_binding(&binding)?;
                continue;
            }
            let marker_path = blocks.join(MARKER_FILE_NAME);
            if !self.validate_path_kind(&marker_path, false)? {
                if binding.activation_height != 0 {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "active dynamic lane storage has no incarnation marker",
                        ),
                        marker_path,
                    ));
                }
                self.write_lane_marker(&binding)?;
            } else {
                self.require_lane_marker(&binding)?;
            }
        }
        Ok(())
    }

    fn require_lane_marker(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let path = self.binding_blocks_path(binding).join(MARKER_FILE_NAME);
        let marker = self.read_lane_marker(&path)?;
        if marker.version != MARKER_VERSION
            || marker.lane_id != binding.lane_id
            || marker.incarnation != binding.incarnation
            || marker.activation_height != binding.activation_height
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane storage incarnation marker does not match authoritative binding",
                ),
                path,
            ));
        }
        Ok(())
    }

    fn read_lane_marker(&self, path: &Path) -> Result<LaneIncarnationMarker> {
        self.validate_path_kind(path, false)?;
        let mut bytes = Vec::new();
        fs::File::open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        decode_exact(&bytes).map_err(Error::NoritoFrame)
    }

    fn write_lane_marker(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        create_dir_all_with_context(&blocks)?;
        let path = blocks.join(MARKER_FILE_NAME);
        let temp = blocks.join(format!("{MARKER_FILE_NAME}.tmp"));
        self.validate_path_kind(&path, false)?;
        let marker = LaneIncarnationMarker {
            version: MARKER_VERSION,
            lane_id: binding.lane_id,
            incarnation: binding.incarnation,
            activation_height: binding.activation_height,
        };
        self.atomic_write_geometry_file(&path, &temp, &marker.encode())
    }

    fn geometry_bindings(
        &self,
        lane_config: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<Vec<LaneGeometryBinding>> {
        let bindings = lane_config
            .entries()
            .iter()
            .map(|entry| self.geometry_binding(entry, incarnations, activation_heights))
            .collect::<Result<Vec<_>>>()?;
        self.validate_geometry_binding_set(&bindings)?;
        Ok(bindings)
    }

    fn geometry_binding(
        &self,
        entry: &LaneConfigEntry,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
    ) -> Result<LaneGeometryBinding> {
        let incarnation = incarnations.get(&entry.lane_id).copied().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry is missing an incarnation commitment",
            )
        })?;
        let activation_height =
            activation_heights
                .get(&entry.lane_id)
                .copied()
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidInput,
                        "lane geometry is missing an incarnation activation height",
                    )
                })?;
        Ok(LaneGeometryBinding {
            lane_id: entry.lane_id,
            incarnation,
            activation_height,
            blocks_path: self.relative_geometry_path(&entry.blocks_dir(&self.store_root))?,
            merge_path: self.relative_geometry_path(&entry.merge_log_path(&self.store_root))?,
        })
    }

    fn relative_geometry_path(&self, path: &Path) -> Result<String> {
        let relative = path.strip_prefix(&self.store_root).map_err(|_| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path escapes the Kura store root",
            )
        })?;
        validate_relative_path(relative)?;
        relative.to_str().map(str::to_owned).ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path is not valid UTF-8",
            )
        })
    }

    fn resolve_relative_path(&self, relative: &str) -> Result<PathBuf> {
        let relative = Path::new(relative);
        validate_relative_path(relative)?;
        Ok(self.store_root.join(relative))
    }

    fn binding_blocks_path(&self, binding: &LaneGeometryBinding) -> PathBuf {
        self.store_root.join(&binding.blocks_path)
    }

    fn binding_merge_path(&self, binding: &LaneGeometryBinding) -> PathBuf {
        self.store_root.join(&binding.merge_path)
    }

    fn validate_binding_paths(&self, binding: &LaneGeometryBinding) -> Result<()> {
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        let blocks_exists = self.validate_path_kind(&blocks, true)?;
        let merge_exists = self.validate_path_kind(&merge, false)?;
        if blocks_exists != merge_exists {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane storage block and merge paths are only partially present",
            ));
        }
        Ok(())
    }

    fn validate_path_kind(&self, path: &Path, directory: bool) -> Result<bool> {
        self.validate_geometry_ancestors(path)?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(Error::IO(error, path.to_path_buf())),
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink()
            || (directory && !file_type.is_dir())
            || (!directory && !file_type.is_file())
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "lane geometry path is a symlink or has the wrong file type",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(true)
    }

    fn validate_geometry_ancestors(&self, path: &Path) -> Result<()> {
        let relative = path.strip_prefix(&self.store_root).map_err(|_| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "lane geometry path escapes the Kura store root",
            )
        })?;
        validate_relative_path(relative)?;
        let mut cursor = self.store_root.clone();
        let components = relative.components().collect::<Vec<_>>();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            cursor.push(component.as_os_str());
            match fs::symlink_metadata(&cursor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry path traverses a symlink",
                        ),
                        cursor,
                    ));
                }
                Ok(metadata) if !metadata.file_type().is_dir() => {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "lane geometry path traverses a non-directory",
                        ),
                        cursor,
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(error) => return Err(Error::IO(error, cursor)),
            }
        }
        Ok(())
    }

    fn retarget_active_geometry_paths(
        &self,
        old_blocks: &Path,
        new_blocks: &Path,
        old_merge: &Path,
        new_merge: &Path,
    ) -> Result<()> {
        {
            let mut active = self.active_blocks_dir.lock();
            if active.as_path() == old_blocks {
                active.clone_from(&new_blocks.to_path_buf());
                let _write_guard = self.block_store_write_lock.lock();
                self.block_store
                    .lock()
                    .retarget_path(new_blocks.to_path_buf())?;
                self.invalidate_durable_budget_snapshot();
            }
        }
        {
            let mut active = self.active_merge_path.lock();
            if active.as_path() == old_merge {
                active.clone_from(&new_merge.to_path_buf());
            }
        }
        {
            let mut plain_text = self.block_plain_text_path.lock();
            if let Some(path) = plain_text.as_mut()
                && let Ok(suffix) = path.strip_prefix(old_blocks)
            {
                *path = new_blocks.join(suffix);
            }
        }
        Ok(())
    }

    fn read_lane_geometry_journal(&self) -> Result<LaneGeometryJournal> {
        let path = self.lane_geometry_journal_path();
        if !self.validate_path_kind(&path, false)? {
            return Ok(LaneGeometryJournal::default());
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|error| Error::IO(error, path.clone()))?;
        let journal = match decode_exact::<LaneGeometryJournal>(&bytes) {
            Ok(journal) if journal.version == JOURNAL_VERSION => journal,
            Ok(journal) => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "unsupported lane geometry journal version {}",
                            journal.version
                        ),
                    ),
                    path,
                ));
            }
            Err(version_two_error) => match decode_exact::<LegacyLaneGeometryJournalV1>(&bytes) {
                Ok(legacy) if legacy.version == LEGACY_JOURNAL_VERSION => LaneGeometryJournal {
                    version: JOURNAL_VERSION,
                    checkpoint: None,
                    pending_archive_gc: Vec::new(),
                    records: legacy.records,
                },
                _ => return Err(Error::NoritoFrame(version_two_error)),
            },
        };
        self.validate_lane_geometry_journal(&journal)?;
        Ok(journal)
    }

    fn validate_lane_geometry_journal(&self, journal: &LaneGeometryJournal) -> Result<()> {
        if journal.version != JOURNAL_VERSION {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal has an unsupported version",
            ));
        }
        if let Some(checkpoint) = journal.checkpoint.as_ref() {
            self.validate_lane_geometry_checkpoint(checkpoint)?;
            if journal
                .records
                .first()
                .is_some_and(|record| record.previous_catalog != checkpoint.catalog)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal retained history does not start at its checkpoint catalog",
                ));
            }
        } else if !journal.pending_archive_gc.is_empty() {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal has pending archive GC without a durable checkpoint",
            ));
        }
        self.validate_pending_lane_geometry_gc(journal)?;
        let mut transition_ids = BTreeSet::new();
        let mut retained_paths = BTreeSet::new();
        for (record_index, record) in journal.records.iter().enumerate() {
            if record.transition_id
                != geometry_transition_id(record.previous_catalog, record.updated_catalog)
                || record.previous_catalog == record.updated_catalog
                || !transition_ids.insert(record.transition_id)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal contains an invalid or duplicate transition",
                ));
            }
            for bindings in [&record.previous_bindings, &record.updated_bindings] {
                self.validate_geometry_binding_set(bindings)?;
            }
            if geometry_catalog_fingerprint(&record.previous_bindings) != record.previous_catalog
                || geometry_catalog_fingerprint(&record.updated_bindings) != record.updated_catalog
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal catalog fingerprint does not match its bindings",
                ));
            }
            if record_index > 0
                && journal.records[record_index - 1].updated_catalog != record.previous_catalog
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal transition chain is not contiguous",
                ));
            }
            let transition_hex = hex::encode(record.transition_id.as_ref());
            let previous_by_lane = record
                .previous_bindings
                .iter()
                .map(|binding| (binding.lane_id, binding))
                .collect::<BTreeMap<_, _>>();
            let updated_by_lane = record
                .updated_bindings
                .iter()
                .map(|binding| (binding.lane_id, binding))
                .collect::<BTreeMap<_, _>>();
            if record
                .operations
                .windows(2)
                .any(|pair| pair[0].lane_id >= pair[1].lane_id)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal operations are duplicated or unsorted",
                ));
            }
            let mut lane_ids = BTreeSet::new();
            for operation in &record.operations {
                if !lane_ids.insert(operation.lane_id)
                    || operation
                        .previous
                        .as_ref()
                        .is_some_and(|binding| binding.lane_id != operation.lane_id)
                    || operation
                        .updated
                        .as_ref()
                        .is_some_and(|binding| binding.lane_id != operation.lane_id)
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains duplicate or mismatched lane operations",
                    ));
                }
                let expected_root = format!(
                    "retired/lane_geometry/{transition_hex}/lane_{:010}",
                    operation.lane_id.as_u32()
                );
                let expected_paths = [
                    format!("{expected_root}/previous_blocks"),
                    format!("{expected_root}/previous_merge.log"),
                    format!("{expected_root}/unpublished_blocks"),
                    format!("{expected_root}/unpublished_merge.log"),
                ];
                let actual_paths = [
                    &operation.archived_blocks_path,
                    &operation.archived_merge_path,
                    &operation.unpublished_blocks_path,
                    &operation.unpublished_merge_path,
                ];
                if actual_paths
                    .iter()
                    .zip(expected_paths.iter())
                    .any(|(actual, expected)| *actual != expected)
                    || actual_paths
                        .iter()
                        .any(|path| !retained_paths.insert((*path).clone()))
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains forged or colliding archive paths",
                    ));
                }
                for binding in operation.previous.iter().chain(operation.updated.iter()) {
                    self.validate_geometry_binding_from_journal(binding)?;
                }
                if operation.previous.as_ref() != previous_by_lane.get(&operation.lane_id).copied()
                    || operation.updated.as_ref()
                        != updated_by_lane.get(&operation.lane_id).copied()
                {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry operation does not match its authenticated catalog bindings",
                    ));
                }
                let shape_is_valid = match operation.kind {
                    LaneGeometryOperationKind::Create => {
                        operation.previous.is_none() && operation.updated.is_some()
                    }
                    LaneGeometryOperationKind::Retire => {
                        operation.previous.is_some() && operation.updated.is_none()
                    }
                    LaneGeometryOperationKind::Replace => operation
                        .previous
                        .as_ref()
                        .zip(operation.updated.as_ref())
                        .is_some_and(|(previous, updated)| {
                            previous.incarnation != updated.incarnation
                                || previous.activation_height != updated.activation_height
                        }),
                    LaneGeometryOperationKind::Relabel => operation
                        .previous
                        .as_ref()
                        .zip(operation.updated.as_ref())
                        .is_some_and(|(previous, updated)| {
                            previous.incarnation == updated.incarnation
                                && previous.activation_height == updated.activation_height
                                && (previous.blocks_path != updated.blocks_path
                                    || previous.merge_path != updated.merge_path)
                        }),
                };
                if !shape_is_valid {
                    return Err(self.geometry_error(
                        ErrorKind::InvalidData,
                        "lane geometry journal contains an invalid operation shape",
                    ));
                }
            }
            let expected_changed_lanes = previous_by_lane
                .keys()
                .chain(updated_by_lane.keys())
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|lane_id| {
                    previous_by_lane.get(lane_id).copied() != updated_by_lane.get(lane_id).copied()
                })
                .count();
            if record.operations.len() != expected_changed_lanes {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal omits or invents a catalog binding operation",
                ));
            }
        }
        Ok(())
    }

    fn validate_lane_geometry_checkpoint(
        &self,
        checkpoint: &LaneGeometrySnapshotCheckpoint,
    ) -> Result<()> {
        if checkpoint.version != CHECKPOINT_VERSION
            || checkpoint.snapshot_state_hash.as_ref().iter().all(|byte| *byte == 0)
            || checkpoint.catalog != geometry_catalog_fingerprint(&checkpoint.bindings)
            || checkpoint.commitment != geometry_checkpoint_commitment(checkpoint)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint commitment or catalog is invalid",
            ));
        }
        self.validate_geometry_binding_set(&checkpoint.bindings)?;
        if (checkpoint.snapshot_height == 0) != checkpoint.snapshot_block_hash.is_none()
            || checkpoint
                .snapshot_block_hash
                .is_some_and(|hash| hash.as_ref().iter().all(|byte| *byte == 0))
            || checkpoint
                .bindings
                .iter()
                .any(|binding| binding.activation_height > checkpoint.snapshot_height)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry checkpoint height, block hash, or activation is invalid",
            ));
        }
        match (
            checkpoint.transition_previous_catalog,
            checkpoint.transition_id,
        ) {
            (None, None) => {}
            (Some(previous), Some(transition_id))
                if transition_id == geometry_transition_id(previous, checkpoint.catalog) => {}
            _ => {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry checkpoint transition binding is invalid",
                ));
            }
        }
        Ok(())
    }

    fn validate_pending_lane_geometry_gc(&self, journal: &LaneGeometryJournal) -> Result<()> {
        if journal.pending_archive_gc.is_empty() {
            return Ok(());
        }
        let checkpoint = journal.checkpoint.as_ref().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidData,
                "pending lane geometry GC has no checkpoint",
            )
        })?;
        let retained_ids = journal
            .records
            .iter()
            .map(|record| record.transition_id)
            .collect::<BTreeSet<_>>();
        let mut pending_ids = BTreeSet::new();
        for (index, pending) in journal.pending_archive_gc.iter().enumerate() {
            if pending.transition_id
                != geometry_transition_id(pending.previous_catalog, pending.updated_catalog)
                || pending.previous_catalog == pending.updated_catalog
                || !pending_ids.insert(pending.transition_id)
                || retained_ids.contains(&pending.transition_id)
                || pending.lane_ids.is_empty()
                || pending
                    .lane_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || index > 0
                    && journal.pending_archive_gc[index - 1].updated_catalog
                        != pending.previous_catalog
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry journal has forged or non-contiguous pending archive GC",
                ));
            }
        }
        let last = journal
            .pending_archive_gc
            .last()
            .expect("non-empty pending archive GC");
        if last.updated_catalog != checkpoint.catalog
            || checkpoint.transition_previous_catalog != Some(last.previous_catalog)
            || checkpoint.transition_id != Some(last.transition_id)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry pending archive GC does not terminate at its checkpoint",
            ));
        }
        Ok(())
    }

    fn validate_geometry_binding_from_journal(&self, binding: &LaneGeometryBinding) -> Result<()> {
        if binding.incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry journal contains a zero incarnation",
            ));
        }
        self.resolve_relative_path(&binding.blocks_path)?;
        self.resolve_relative_path(&binding.merge_path)?;
        Ok(())
    }

    fn validate_geometry_binding_set(&self, bindings: &[LaneGeometryBinding]) -> Result<()> {
        if bindings.is_empty()
            || bindings
                .windows(2)
                .any(|pair| pair[0].lane_id >= pair[1].lane_id)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "lane geometry catalog bindings are empty, duplicated, or unsorted",
            ));
        }
        let mut incarnations = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for binding in bindings {
            self.validate_geometry_binding_from_journal(binding)?;
            if !incarnations.insert(binding.incarnation)
                || !paths.insert(binding.blocks_path.clone())
                || !paths.insert(binding.merge_path.clone())
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "lane geometry catalog contains duplicate incarnations or storage paths",
                ));
            }
        }
        Ok(())
    }

    fn write_lane_geometry_journal(&self, journal: &LaneGeometryJournal) -> Result<()> {
        self.validate_lane_geometry_journal(journal)?;
        let path = self.lane_geometry_journal_path();
        let temp = self.store_root.join(JOURNAL_TEMP_FILE_NAME);
        self.atomic_write_geometry_file(&path, &temp, &journal.encode())
    }

    fn atomic_write_geometry_file(&self, path: &Path, temp: &Path, bytes: &[u8]) -> Result<()> {
        if let Ok(metadata) = fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.file_type().is_file())
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "geometry sidecar target has an unsafe file type",
                ),
                path.to_path_buf(),
            ));
        }
        if let Ok(metadata) = fs::symlink_metadata(temp) {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "geometry journal temporary path has an unsafe file type",
                    ),
                    temp.to_path_buf(),
                ));
            }
            fs::remove_file(temp).map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        }
        if let Some(parent) = path.parent() {
            create_dir_all_with_context(parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        file.write_all(bytes)
            .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        // Geometry intents are ordering barriers, not throughput-oriented
        // sidecars. `Batched` therefore has the same durability semantics as
        // `On` here: the intent must reach stable storage before any rename.
        if !matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            file.sync_all()
                .map_err(|error| Error::IO(error, temp.to_path_buf()))?;
        }
        drop(file);
        fs::rename(temp, path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        self.sync_geometry_parent(path.parent())
    }

    fn sync_geometry_parent(&self, parent: Option<&Path>) -> Result<()> {
        if matches!(self.sidecar_fsync_mode(), FsyncMode::Off) {
            return Ok(());
        }
        if let Some(parent) = parent {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        Ok(())
    }

    pub(crate) fn lane_geometry_journal_path(&self) -> PathBuf {
        self.store_root.join(JOURNAL_FILE_NAME)
    }

    fn geometry_error(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error::IO(
            std::io::Error::new(kind, message),
            self.lane_geometry_journal_path(),
        )
    }
}

fn geometry_catalog_fingerprint(bindings: &[LaneGeometryBinding]) -> Hash {
    let encoded = bindings.to_vec().encode();
    Hash::new_from_chunks(&[CATALOG_DOMAIN, encoded.as_slice()])
}

fn geometry_transition_id(previous: Hash, updated: Hash) -> Hash {
    Hash::new_from_chunks(&[TRANSITION_DOMAIN, previous.as_ref(), updated.as_ref()])
}

fn geometry_checkpoint_commitment(checkpoint: &LaneGeometrySnapshotCheckpoint) -> Hash {
    let mut payload = Vec::new();
    payload.push(checkpoint.version);
    payload.extend_from_slice(&checkpoint.snapshot_height.to_le_bytes());
    match checkpoint.snapshot_block_hash {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(checkpoint.snapshot_state_hash.as_ref());
    payload.extend_from_slice(checkpoint.catalog.as_ref());
    match checkpoint.transition_previous_catalog {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    match checkpoint.transition_id {
        Some(hash) => {
            payload.push(1);
            payload.extend_from_slice(hash.as_ref());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(&checkpoint.bindings.clone().encode());
    Hash::new_from_chunks(&[CHECKPOINT_DOMAIN, payload.as_slice()])
}

fn lane_geometry_snapshot_checkpoint(
    snapshot_height: u64,
    snapshot_block_hash: Option<HashOf<BlockHeader>>,
    snapshot_state_hash: Hash,
    bindings: Vec<LaneGeometryBinding>,
    transition_previous_catalog: Option<Hash>,
    transition_id: Option<Hash>,
) -> LaneGeometrySnapshotCheckpoint {
    let mut checkpoint = LaneGeometrySnapshotCheckpoint {
        version: CHECKPOINT_VERSION,
        snapshot_height,
        snapshot_block_hash,
        snapshot_state_hash,
        catalog: geometry_catalog_fingerprint(&bindings),
        transition_previous_catalog,
        transition_id,
        bindings,
        commitment: Hash::prehashed([0; Hash::LENGTH]),
    };
    checkpoint.commitment = geometry_checkpoint_commitment(&checkpoint);
    checkpoint
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "lane geometry journal contains an unsafe relative path",
            ),
            path.to_path_buf(),
        ));
    }
    Ok(())
}

fn decode_exact<T: Decode>(bytes: &[u8]) -> std::result::Result<T, norito::core::Error> {
    let mut input = bytes;
    let value = T::decode(&mut input)?;
    if !input.is_empty() {
        return Err(norito::core::Error::Message(
            "trailing bytes in lane geometry sidecar".to_owned(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, num::NonZeroU32, sync::Arc};

    use iroha_config::{
        base::WithOrigin,
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura::{
                BLOCK_SYNC_ROSTER_RETENTION, BLOCKS_IN_MEMORY, EVICTION_REQUIRED_REPLICAS,
                FSYNC_INTERVAL, MAX_DISK_USAGE_BYTES, MERGE_LEDGER_CACHE_CAPACITY,
                ROSTER_SIDECAR_RETENTION,
            },
        },
    };
    use iroha_data_model::nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId};
    use tempfile::TempDir;

    use super::*;

    fn open_kura(root: &Path, lane_config: &RuntimeLaneConfig) -> Arc<Kura> {
        let config = KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(root.to_path_buf()),
            max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::On,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas: EVICTION_REQUIRED_REPLICAS,
        };
        Kura::new(&config, lane_config).expect("open test Kura").0
    }

    fn initial_and_extended_configs() -> (RuntimeLaneConfig, RuntimeLaneConfig) {
        let lane0 = ModelLaneConfig::default();
        let lane1 = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "elastic-one".to_owned(),
            ..ModelLaneConfig::default()
        };
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let initial = LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("initial catalog");
        let extended = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("extended catalog");
        (
            RuntimeLaneConfig::from_catalog(&initial),
            RuntimeLaneConfig::from_catalog(&extended),
        )
    }

    fn initial_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
        (
            BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH]))]),
            BTreeMap::from([(LaneId::SINGLE, 0)]),
        )
    }

    fn extended_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
        (
            BTreeMap::from([
                (LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH])),
                (LaneId::new(1), Hash::prehashed([0x22; Hash::LENGTH])),
            ]),
            BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 9)]),
        )
    }

    struct RetiredGeometryFixture {
        initial: RuntimeLaneConfig,
        extended: RuntimeLaneConfig,
        initial_incarnations: BTreeMap<LaneId, Hash>,
        initial_activations: BTreeMap<LaneId, u64>,
        extended_incarnations: BTreeMap<LaneId, Hash>,
        extended_activations: BTreeMap<LaneId, u64>,
        archive_root: PathBuf,
    }

    fn prepare_retired_geometry_archive(kura: &Kura, root: &Path) -> RetiredGeometryFixture {
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("create elastic lane");
        kura.mark_lane_geometry_catalog_published(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("publish elastic lane catalog");
        let lane_one_blocks = extended
            .entry(LaneId::new(1))
            .expect("elastic lane")
            .blocks_dir(root);
        fs::write(lane_one_blocks.join("gc-payload.norito"), [0xA5; 37])
            .expect("seed archived payload bytes");

        kura.apply_lane_geometry_transition(
            &extended,
            &initial,
            &extended_incarnations,
            &initial_incarnations,
            &extended_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect("retire elastic lane");
        kura.mark_lane_geometry_catalog_published(
            &initial,
            &initial_incarnations,
            &initial_activations,
        )
        .expect("publish retired catalog");
        let journal = kura.read_lane_geometry_journal().expect("geometry journal");
        let retired = journal.records.last().expect("retire transition");
        let archive_root = root
            .join("retired/lane_geometry")
            .join(hex::encode(retired.transition_id.as_ref()));
        assert!(archive_root.exists(), "retired lane archive exists");
        RetiredGeometryFixture {
            initial,
            extended,
            initial_incarnations,
            initial_activations,
            extended_incarnations,
            extended_activations,
            archive_root,
        }
    }

    fn checkpoint_retired_geometry(
        kura: &Kura,
        fixture: &RetiredGeometryFixture,
        height: u64,
    ) -> Result<LaneGeometryGcSummary> {
        let bindings = kura.geometry_bindings(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )?;
        kura.checkpoint_lane_geometry_with_proven_snapshot(
            bindings,
            height,
            Some(HashOf::from_untyped_unchecked(Hash::new([
                0xB0,
                u8::try_from(height).unwrap_or(u8::MAX),
            ]))),
            Hash::new([0xC0, u8::try_from(height).unwrap_or(u8::MAX)]),
        )
    }

    #[test]
    fn recovery_rolls_back_partial_unpublished_create_and_replays_it_idempotently() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare journaled create");
        let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
        assert!(lane1.blocks_dir(&root).exists());

        kura.apply_lane_geometry_transition(
            &initial,
            &initial,
            &initial_incarnations,
            &initial_incarnations,
            &initial_activations,
            &initial_activations,
            &BTreeSet::new(),
        )
        .expect("same-catalog startup rolls back unpublished create");
        assert!(!lane1.blocks_dir(&root).exists());

        // Model a process dying after restoring only the block directory from
        // the unpublished archive. Recovery of the old catalog must complete
        // the inverse operation without duplicating or dropping either path.
        let journal = kura.read_lane_geometry_journal().expect("read journal");
        let operation = journal.records[0].operations[0].clone();
        kura.move_geometry_path(
            &kura
                .resolve_relative_path(&operation.unpublished_blocks_path)
                .expect("unpublished blocks path"),
            &lane1.blocks_dir(&root),
            true,
        )
        .expect("inject partial roll-forward");
        assert!(lane1.blocks_dir(&root).exists());

        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &initial,
                &initial_incarnations,
                &initial_activations,
            )
            .expect("idempotent rollback recovery");
            assert!(!lane1.blocks_dir(&root).exists());
            assert!(!lane1.merge_log_path(&root).exists());
        }

        // The catalog is now authoritative (as after snapshot/block replay),
        // so the same retained intent must roll forward and recover the exact
        // unpublished segment instead of provisioning an empty replacement.
        for _ in 0..2 {
            kura.recover_lane_geometry_journal(
                &extended,
                &extended_incarnations,
                &extended_activations,
            )
            .expect("idempotent roll-forward recovery");
            assert!(lane1.blocks_dir(&root).exists());
            assert!(lane1.merge_log_path(&root).exists());
        }
        let recovered = kura.read_lane_geometry_journal().expect("read journal");
        assert_eq!(
            recovered.records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }

    #[test]
    fn files_applied_phase_rolls_forward_when_catalog_is_already_authoritative() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare transition");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::FilesApplied
        );

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect("recover post-catalog crash");
        assert_eq!(
            kura.read_lane_geometry_journal().expect("journal").records[0].phase,
            LaneGeometryPhase::CatalogPublished
        );
    }

    #[test]
    fn recovery_rejects_stale_incarnation_marker() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare transition");

        let mut stale_incarnations = extended_incarnations.clone();
        stale_incarnations.insert(LaneId::new(1), Hash::prehashed([0x77; Hash::LENGTH]));
        let stale = kura
            .geometry_binding(
                extended.entry(LaneId::new(1)).expect("lane one"),
                &stale_incarnations,
                &extended_activations,
            )
            .expect("stale binding");
        kura.write_lane_marker(&stale).expect("write stale marker");

        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("stale incarnation marker must fail closed");
    }

    #[test]
    fn transition_rejects_reserved_archive_collision_before_mutation() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let previous_bindings = kura
            .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
            .expect("initial bindings");
        let updated_bindings = kura
            .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
            .expect("updated bindings");
        let transition = geometry_transition_id(
            geometry_catalog_fingerprint(&previous_bindings),
            geometry_catalog_fingerprint(&updated_bindings),
        );
        let collision = root
            .join("retired/lane_geometry")
            .join(hex::encode(transition.as_ref()))
            .join("lane_0000000001/previous_blocks");
        fs::create_dir_all(&collision).expect("seed archive collision");

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("archive collision must fail before applying files");
        assert!(
            !extended
                .entry(LaneId::new(1))
                .expect("lane one")
                .blocks_dir(&root)
                .exists()
        );
        assert!(
            kura.read_lane_geometry_journal()
                .expect("journal remains readable")
                .records
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn transition_rejects_symlink_lane_target() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside directory");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let target = extended
            .entry(LaneId::new(1))
            .expect("lane one")
            .blocks_dir(&root);
        fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
        symlink(&outside, &target).expect("seed symlink target");

        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("symlink target must fail closed");
        assert!(
            outside
                .read_dir()
                .expect("outside remains readable")
                .next()
                .is_none()
        );
    }

    #[test]
    fn snapshot_checkpoint_compacts_only_proven_history_and_preserves_latest_recovery() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);

        // Before checkpoint publication, both the old and current authoritative catalogs remain
        // recoverable from the retained transition chain.
        kura.recover_lane_geometry_journal(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
        )
        .expect("old snapshot geometry remains recoverable before GC");
        kura.recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect("restore current snapshot geometry");

        let cached_before = kura.refresh_disk_usage_bytes().expect("usage before GC");
        let summary = checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect("checkpoint current snapshot geometry");
        assert_eq!(summary.compacted_transitions, 2);
        assert_eq!(summary.removed_archive_roots, 1);
        assert!(summary.reclaimed_bytes >= 37);
        assert!(!fixture.archive_root.exists());
        let journal = kura.read_lane_geometry_journal().expect("compacted journal");
        assert!(journal.records.is_empty());
        assert!(journal.pending_archive_gc.is_empty());
        assert_eq!(
            journal.checkpoint.as_ref().map(|checkpoint| checkpoint.catalog),
            Some(geometry_catalog_fingerprint(
                &kura
                    .geometry_bindings(
                        &fixture.initial,
                        &fixture.initial_incarnations,
                        &fixture.initial_activations,
                    )
                    .expect("initial bindings")
            ))
        );
        let cached_after = kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(cached_after, kura.kura_disk_usage_bytes().expect("exact usage scan"));
        assert!(cached_after < cached_before);

        assert_eq!(
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect("checkpoint replay is idempotent"),
            LaneGeometryGcSummary::default()
        );
        kura.recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect("new snapshot remains recoverable");
        kura.recover_lane_geometry_journal(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
        )
        .expect_err("checkpointed-away old snapshot must not synthesize empty lane storage");

        drop(kura);
        let restarted = open_kura(&root, &fixture.initial);
        restarted
            .recover_lane_geometry_journal(
                &fixture.initial,
                &fixture.initial_incarnations,
                &fixture.initial_activations,
            )
            .expect("restart recovers checkpoint-authoritative geometry");
    }

    #[test]
    fn checkpoint_rejects_stale_height_and_lane_incarnation_aba() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        checkpoint_retired_geometry(&kura, &fixture, 20).expect("initial checkpoint");
        checkpoint_retired_geometry(&kura, &fixture, 19)
            .expect_err("older snapshot checkpoint must fail closed");

        let mut recreated_incarnations = fixture.extended_incarnations.clone();
        recreated_incarnations.insert(LaneId::new(1), Hash::prehashed([0x33; Hash::LENGTH]));
        let mut recreated_activations = fixture.extended_activations.clone();
        recreated_activations.insert(LaneId::new(1), 21);
        kura.apply_lane_geometry_transition(
            &fixture.initial,
            &fixture.extended,
            &fixture.initial_incarnations,
            &recreated_incarnations,
            &fixture.initial_activations,
            &recreated_activations,
            &BTreeSet::new(),
        )
        .expect("recreate lane id with fresh incarnation");
        kura.mark_lane_geometry_catalog_published(
            &fixture.extended,
            &recreated_incarnations,
            &recreated_activations,
        )
        .expect("publish recreated lane");

        let stale_bindings = kura
            .geometry_bindings(
                &fixture.extended,
                &fixture.extended_incarnations,
                &fixture.extended_activations,
            )
            .expect("stale bindings");
        kura.checkpoint_lane_geometry_with_proven_snapshot(
            stale_bindings,
            30,
            Some(HashOf::from_untyped_unchecked(Hash::new(b"stale-block"))),
            Hash::new(b"stale-state"),
        )
        .expect_err("same lane id with an old incarnation is not a reachable checkpoint");

        let recreated_bindings = kura
            .geometry_bindings(
                &fixture.extended,
                &recreated_incarnations,
                &recreated_activations,
            )
            .expect("recreated bindings");
        let summary = kura
            .checkpoint_lane_geometry_with_proven_snapshot(
                recreated_bindings,
                30,
                Some(HashOf::from_untyped_unchecked(Hash::new(b"new-block"))),
                Hash::new(b"new-state"),
            )
            .expect("fresh incarnation checkpoint");
        assert_eq!(summary.compacted_transitions, 1);
    }

    #[test]
    fn geometry_gc_crash_boundaries_replay_safely_after_restart() {
        for stage in [
            GC_FAIL_AFTER_COMPACTION_INTENT,
            GC_FAIL_AFTER_ARCHIVE_DELETION,
            GC_FAIL_AFTER_COMPLETION,
        ] {
            let temp = TempDir::new().expect("temporary directory");
            let root = temp.path().join(format!("kura-stage-{stage}"));
            let kura = open_kura(&root, &initial_and_extended_configs().0);
            let fixture = prepare_retired_geometry_archive(&kura, &root);
            kura.fail_next_lane_geometry_gc_at_stage_for_test(stage);
            checkpoint_retired_geometry(&kura, &fixture, 20)
                .expect_err("injected GC boundary must interrupt acknowledgement");
            let after_failure = kura.read_lane_geometry_journal().expect("journal after crash");
            assert!(after_failure.records.is_empty());
            if stage == GC_FAIL_AFTER_COMPACTION_INTENT {
                assert!(fixture.archive_root.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else if stage == GC_FAIL_AFTER_ARCHIVE_DELETION {
                assert!(!fixture.archive_root.exists());
                assert!(!after_failure.pending_archive_gc.is_empty());
            } else {
                assert!(!fixture.archive_root.exists());
                assert!(after_failure.pending_archive_gc.is_empty());
            }

            drop(kura);
            let restarted = open_kura(&root, &fixture.initial);
            restarted
                .recover_lane_geometry_journal(
                    &fixture.initial,
                    &fixture.initial_incarnations,
                    &fixture.initial_activations,
                )
                .expect("restart completes or observes completed GC");
            let recovered = restarted
                .read_lane_geometry_journal()
                .expect("recovered journal");
            assert!(recovered.pending_archive_gc.is_empty());
            assert!(!fixture.archive_root.exists());
        }
    }

    #[test]
    fn storage_budget_purge_only_resumes_snapshot_proven_geometry_gc() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("leave durable pending-GC intent");
        assert!(fixture.archive_root.exists());

        assert!(
            kura.purge_retired_segments(),
            "budget purge resumes only the already-proven archive deletion"
        );
        assert!(!fixture.archive_root.exists());
        assert!(
            kura.read_lane_geometry_journal()
                .expect("journal after budget purge")
                .pending_archive_gc
                .is_empty()
        );
        assert_eq!(
            kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
            kura.kura_disk_usage_bytes().expect("exact usage after purge")
        );
    }

    #[test]
    fn geometry_gc_rejects_unauthenticated_archive_collision_without_deleting_it() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let collision = fixture.archive_root.join("operator-data.txt");
        fs::write(&collision, b"must not delete").expect("seed unauthenticated collision");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("unexpected archive content must fail closed");
        assert_eq!(
            fs::read(&collision).expect("collision retained"),
            b"must not delete"
        );
        let journal = kura.read_lane_geometry_journal().expect("pending journal");
        assert!(journal.records.is_empty());
        assert!(!journal.pending_archive_gc.is_empty());

        fs::remove_file(&collision).expect("operator resolves collision");
        let resumed = kura
            .resume_proven_lane_geometry_archive_gc()
            .expect("resume proven GC after repair");
        assert_eq!(resumed.removed_archive_roots, 1);
    }

    #[cfg(unix)]
    #[test]
    fn geometry_gc_rejects_symlink_inside_archive_tree() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join("outside.txt");
        fs::write(&outside, b"outside").expect("outside sentinel");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        let archived_blocks = fixture
            .archive_root
            .join("lane_0000000001/previous_blocks");
        let link = archived_blocks.join("escape");
        symlink(&outside, &link).expect("seed archive symlink");

        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("archive symlink must fail closed");
        assert_eq!(fs::read(&outside).expect("outside retained"), b"outside");
        assert!(link.exists());
    }

    #[test]
    fn recovery_rejects_corrupt_and_forged_journals() {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        fs::write(kura.lane_geometry_journal_path(), b"not norito").expect("write corrupt journal");
        kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
            .expect_err("corrupt journal must fail closed");

        fs::remove_file(kura.lane_geometry_journal_path()).expect("remove corrupt journal");
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect("prepare valid journal");
        let mut forged = kura.read_lane_geometry_journal().expect("valid journal");
        forged.records[0].operations[0].archived_blocks_path = "../escape".to_owned();
        fs::write(kura.lane_geometry_journal_path(), forged.encode())
            .expect("write forged journal bytes");
        kura.recover_lane_geometry_journal(
            &extended,
            &extended_incarnations,
            &extended_activations,
        )
        .expect_err("forged archive path must fail closed");
    }
}
