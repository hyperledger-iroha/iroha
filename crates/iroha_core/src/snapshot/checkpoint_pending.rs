//! Private durable checkpoint candidates. Only authenticated committed candidates become
//! retained checkpoints; pending artifacts never authorize retention or a startup floor.

use super::{
    SnapshotLocation, SnapshotMaker, checkpoint_capture::CheckpointWriteError, checkpoint_store,
};
use std::{
    io,
    path::{Path, PathBuf},
};

/// This container is excluded from rolling-generation and retained-checkpoint collection.
pub(super) const DIRECTORY: &str = "checkpoint-pending";
/// One visible image awaiting promotion and one new original checkpoint may coexist.
const MAX_PENDING: usize = 2;

pub(super) struct PendingEntry {
    pub(super) height: u64,
    pub(super) committed_ms: u64,
    pub(super) path: PathBuf,
    pub(super) location: SnapshotLocation,
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// Inspect the complete bounded spool. Foreign names, aliases, excess slots, or incomplete
/// canonical candidates are refusals; no recovery path silently discards them.
pub(super) fn entries(store: &Path) -> io::Result<Vec<PendingEntry>> {
    inspect(store).map(|(entries, _)| entries)
}

fn inspect(
    store: &Path,
) -> io::Result<(Vec<PendingEntry>, std::collections::BTreeSet<(u64, u64)>)> {
    let root = match super::direct_snapshot_directory_identity(store) {
        Ok(identity) => identity,
        Err(super::TryReadError::IO(error, _)) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Default::default()));
        }
        Err(error) => return Err(invalid(error)),
    };
    let directory = store.join(DIRECTORY);
    let parent = match super::direct_snapshot_directory_identity(&directory) {
        Ok(identity) => identity,
        Err(super::TryReadError::IO(error, _)) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Default::default()));
        }
        Err(error) => return Err(invalid(error)),
    };
    let mut result = Vec::new();
    let mut stages = 0;
    let mut slots = std::collections::BTreeSet::new();
    for (count, entry) in std::fs::read_dir(&directory)?.enumerate() {
        if count >= 2 * MAX_PENDING {
            return Err(invalid("pending checkpoint scan bound exceeded"));
        }
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("noncanonical pending checkpoint name"))?;
        super::direct_snapshot_directory_identity(&entry.path()).map_err(invalid)?;
        if let Some(name) = name.strip_prefix(".staging-") {
            let identity = checkpoint_store::parse_entry_name(name)
                .ok_or_else(|| invalid("noncanonical pending staging name"))?;
            slots.insert(identity);
            stages += 1;
            if stages > MAX_PENDING {
                return Err(invalid("pending staging quota exceeded"));
            }
            continue;
        }
        let (height, committed_ms) = checkpoint_store::parse_entry_name(&name)
            .ok_or_else(|| invalid("noncanonical pending checkpoint name"))?;
        slots.insert((height, committed_ms));
        if result.len() == MAX_PENDING {
            return Err(invalid("pending checkpoint quota exceeded"));
        }
        let path = entry.path();
        let digest_path = path.join(super::SNAPSHOT_DIGEST_FILE_NAME);
        let (_, bytes) =
            super::bind_required_snapshot_file(&digest_path, super::SNAPSHOT_DIGEST_MAX_BYTES)
                .map_err(invalid)?;
        let digest =
            super::parse_snapshot_current_pointer(&bytes, &digest_path).map_err(invalid)?;
        result.push(PendingEntry {
            height,
            committed_ms,
            path,
            location: SnapshotLocation::PendingCheckpoint { name, digest },
        });
    }
    if super::direct_snapshot_directory_identity(store).map_err(invalid)? != root
        || super::direct_snapshot_directory_identity(&directory).map_err(invalid)? != parent
    {
        return Err(invalid("pending checkpoint parent custody changed"));
    }
    result.sort_by_key(|entry| (entry.height, entry.committed_ms));
    if result
        .windows(2)
        .any(|pair| pair[0].height == pair[1].height)
    {
        return Err(invalid("pending checkpoints repeat a height"));
    }
    if slots.len() > MAX_PENDING {
        return Err(invalid("pending checkpoint slot quota exceeded"));
    }
    let mut heights = std::collections::BTreeSet::new();
    if slots.iter().any(|(height, _)| !heights.insert(*height)) {
        return Err(invalid("pending checkpoint slots repeat a height"));
    }
    Ok((result, slots))
}

pub(super) fn reserve(store: &Path, height: u64, committed_ms: u64) -> io::Result<()> {
    let (_, slots) = inspect(store)?;
    if slots
        .iter()
        .any(|&(held_height, held_time)| held_height == height && held_time != committed_ms)
    {
        return Err(invalid("pending checkpoint height has a conflicting time"));
    }
    if slots.len() >= MAX_PENDING && !slots.contains(&(height, committed_ms)) {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "pending checkpoint quota is occupied",
        ));
    }
    Ok(())
}

type WriterResult<T> = Result<T, CheckpointWriteError>;
fn writer_invalid(error: impl std::fmt::Display) -> CheckpointWriteError {
    CheckpointWriteError::Read(super::TryReadError::SnapshotGenerationInvalid {
        path: PathBuf::new(),
        reason: error.to_string(),
    })
}

impl SnapshotMaker {
    fn checkpoint_io<T>(&self, action: impl FnOnce() -> WriterResult<T>) -> WriterResult<T> {
        let (hashes, membership) = self.state.history_allocation_budgets();
        membership.with_deferred_refund_notifications(|_| {
            hashes.with_deferred_refund_notifications(|_| {
                self.read_buffer_budget
                    .with_deferred_refund_notifications(|_| action())
            })
        })
    }

    pub(super) fn persist_pending_checkpoint(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
    ) -> WriterResult<()> {
        self.checkpoint_io(|| self.persist_pending_checkpoint_inner(capture))
    }

    pub(super) fn promote_visible_checkpoint(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
    ) -> WriterResult<()> {
        self.checkpoint_io(|| self.promote_visible_checkpoint_inner(capture, true))
    }

    /// A certified replay owner grants visibility authority for its exact original image.
    /// Relocate that image without running policy retention before startup completes.
    pub(super) fn promote_replayed_checkpoint(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
    ) -> WriterResult<()> {
        self.checkpoint_io(|| self.promote_visible_checkpoint_inner(capture, false))
    }

    pub(super) fn reconcile_pending_checkpoints(
        &self,
        owned: Option<(u64, u64)>,
    ) -> WriterResult<()> {
        self.checkpoint_io(|| self.reconcile_pending_checkpoints_inner(owned))
    }

    fn current_checkpoint_schedule(&self) -> WriterResult<checkpoint_store::CheckpointSchedule> {
        use crate::state::{StateReadOnly as _, WorldReadOnly as _};
        use mv::storage::StorageReadOnly as _;
        let view = self.state.view();
        let world = view.world();
        let tip = view
            .native_execution_tip()
            .ok_or_else(|| writer_invalid("recovered State has no native tip"))?;
        let policy = crate::runtime::checkpoint::committed_policy(
            world.runtime_selection(),
            world.parameters(),
        )
        .ok_or_else(|| writer_invalid("recovered State has no checkpoint retention policy"))?;
        Ok(checkpoint_store::CheckpointSchedule::new(
            policy,
            world
                .runtime_boundaries()
                .iter()
                .map(|(_, boundary)| boundary),
            tip.creation_time_ms(),
        ))
    }

    /// Persist the original frozen bytes with no rolling-pointer or retention mutation.
    fn persist_pending_checkpoint_inner(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
    ) -> WriterResult<()> {
        if !capture.snapshot.identity.native_tip.is_some_and(|tip| {
            tip.height() == capture.height && tip.creation_time_ms() == capture.committed_ms
        }) {
            return Err(writer_invalid(
                "pending checkpoint name differs from the original native tip",
            ));
        }
        let _publication = super::SNAPSHOT_PUBLICATION_LOCK.lock();
        // A previous certified replay may have promoted this exact image before startup
        // finished. Its original signed bytes are sufficient for another acknowledgement;
        // never create a duplicate pending name that would conflict during promotion.
        if self.sync_retained_original(capture)? {
            return Ok(());
        }
        reserve(&self.store_dir, capture.height, capture.committed_ms)?;
        super::try_write_snapshot_payload_with_limit_locked(
            &self.state,
            &self.store_dir,
            &self.signing_key,
            (
                self.merkle_chunk_size,
                self.max_payload_bytes,
                self.resource_policy,
            ),
            capture.snapshot.json.as_bytes(),
            capture.snapshot.identity.clone(),
            &self.read_buffer_budget,
            false,
        )
        .map_err(CheckpointWriteError::from)?;
        use sha2::{Digest as _, Sha256};
        let generation = self
            .store_dir
            .join(super::SNAPSHOT_GENERATIONS_DIR_NAME)
            .join(hex::encode(Sha256::digest(
                capture.snapshot.json.as_bytes(),
            )));
        checkpoint_store::copy_to_container(
            &self.store_dir,
            &generation,
            DIRECTORY,
            capture.height,
            capture.committed_ms,
            self.max_payload_bytes,
            self.merkle_chunk_size,
        )
        .map_err(CheckpointWriteError::from)
    }

    /// After the original owner reports visibility, its original acknowledgement already
    /// authenticates the exact image. Promotion alone grants retained-checkpoint authority.
    fn promote_visible_checkpoint_inner(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
        apply_retention: bool,
    ) -> WriterResult<()> {
        let _publication = super::SNAPSHOT_PUBLICATION_LOCK.lock();
        let schedule = apply_retention
            .then(|| self.current_checkpoint_schedule())
            .transpose()?;
        let pending = entries(&self.store_dir)?.into_iter().find(|entry| {
            entry.height == capture.height && entry.committed_ms == capture.committed_ms
        });
        let Some(pending) = pending else {
            if !self.sync_retained_original(capture)? {
                return Err(writer_invalid(
                    "acknowledged checkpoint has neither pending nor retained custody",
                ));
            }
            if let Some(schedule) = schedule.as_ref() {
                checkpoint_store::retain(&self.store_dir, schedule)?;
            }
            return Ok(());
        };
        let bound = self.bind_pending(&pending)?;
        capture
            .snapshot
            .identity
            .validate_bytes(capture.snapshot.json.as_bytes())
            .map_err(CheckpointWriteError::from)?;
        use sha2::{Digest as _, Sha256};
        let digest: [u8; 32] = Sha256::digest(capture.snapshot.json.as_bytes()).into();
        if bound.payload.bytes_sha256 != Some(digest) {
            return Err(writer_invalid(
                "pending checkpoint differs from the original acknowledged image",
            ));
        }
        self.finish_pending(&pending, &bound, schedule.as_ref())
    }

    /// Verify a repeated original's exact named retained image and repeat durability barriers.
    /// This grants no authority to unrelated stored candidates and never runs retention.
    fn sync_retained_original(
        &self,
        capture: &super::checkpoint_capture::CheckpointCapture,
    ) -> WriterResult<bool> {
        use sha2::{Digest as _, Sha256};
        let name = format!("{}-{}", capture.height, capture.committed_ms);
        let path = checkpoint_store::checkpoints_dir(&self.store_dir).join(&name);
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let digest: [u8; 32] = Sha256::digest(capture.snapshot.json.as_bytes()).into();
        let location = SnapshotLocation::Checkpoint {
            name,
            digest: hex::encode(digest),
        };
        let bound = super::bind_snapshot_location_with_mode(
            &self.store_dir,
            self.max_payload_bytes.get() as u64,
            self.merkle_chunk_size,
            false,
            Some(&location),
        )
        .map_err(CheckpointWriteError::from)?;
        self.verify_pending_envelope(&bound)?;
        if bound.payload.bytes_sha256 != Some(digest) {
            return Err(writer_invalid(
                "retained checkpoint differs from the original certified replay image",
            ));
        }
        bound
            .verify_generation_unchanged()
            .map_err(CheckpointWriteError::from)?;
        bound.payload.handle.sync_all()?;
        for artifact in &bound.artifacts {
            artifact.handle.sync_all()?;
        }
        super::sync_snapshot_directory(&bound.snapshot_directory, bound.snapshot_identity)
            .map_err(CheckpointWriteError::from)?;
        super::sync_snapshot_directory(&bound.generations_dir, bound.generations_dir_identity)
            .map_err(CheckpointWriteError::from)?;
        let pending = self.store_dir.join(DIRECTORY);
        match super::direct_snapshot_directory_identity(&pending) {
            Ok(identity) => super::sync_snapshot_directory(&pending, identity)
                .map_err(CheckpointWriteError::from)?,
            Err(super::TryReadError::IO(error, _)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        super::sync_snapshot_directory(&self.store_dir, bound.store_dir_identity)
            .map_err(CheckpointWriteError::from)?;
        Ok(true)
    }

    fn bind_pending(&self, entry: &PendingEntry) -> WriterResult<super::BoundSnapshotGeneration> {
        super::bind_snapshot_location_with_mode(
            &self.store_dir,
            self.max_payload_bytes
                .get()
                .min(self.resource_policy.max_transient_bytes.get()) as u64,
            self.merkle_chunk_size,
            false,
            Some(&entry.location),
        )
        .map_err(CheckpointWriteError::from)
    }

    fn finish_pending(
        &self,
        entry: &PendingEntry,
        bound: &super::BoundSnapshotGeneration,
        schedule: Option<&checkpoint_store::CheckpointSchedule>,
    ) -> WriterResult<()> {
        bound
            .verify_generation_unchanged()
            .map_err(CheckpointWriteError::from)?;
        let directory = checkpoint_store::checkpoints_dir(&self.store_dir);
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        let parent = super::direct_snapshot_directory_identity(&directory)
            .map_err(CheckpointWriteError::from)?;
        super::sync_snapshot_directory(&self.store_dir, bound.store_dir_identity)
            .map_err(CheckpointWriteError::from)?;
        let destination = directory.join(format!("{}-{}", entry.height, entry.committed_ms));
        match std::fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(writer_invalid(
                    "pending checkpoint conflicts with an existing retained name",
                ));
            }
        }
        bound
            .verify_generation_unchanged()
            .map_err(CheckpointWriteError::from)?;
        // The artifacts are already durable and single-linked. Atomic relocation has no
        // partially deleted canonical spool state after a crash.
        std::fs::rename(&entry.path, &destination)?;
        super::sync_snapshot_directory(&directory, parent).map_err(CheckpointWriteError::from)?;
        super::sync_snapshot_directory(&bound.generations_dir, bound.generations_dir_identity)
            .map_err(CheckpointWriteError::from)?;
        if let Some(schedule) = schedule {
            checkpoint_store::retain(&self.store_dir, schedule)?;
        }
        Ok(())
    }

    /// Run only after successful startup recovery, before rolling GC. A tip candidate with
    /// no actual certified successor remains private; the second bounded slot lets the
    /// live chain make progress without manufacturing a successor block.
    fn reconcile_pending_checkpoints_inner(&self, owned: Option<(u64, u64)>) -> WriterResult<()> {
        let _publication = super::SNAPSHOT_PUBLICATION_LOCK.lock();
        for entry in entries(&self.store_dir)? {
            if owned == Some((entry.height, entry.committed_ms)) {
                continue;
            }
            let bound = self.bind_pending(&entry)?;
            self.verify_pending_envelope(&bound)?;
            let count = self
                .state
                .exact_durable_block_count()
                .map_err(CheckpointWriteError::from)?;
            if entry.height > self.state.committed_height() as u64 || entry.height >= count as u64 {
                // Only absence of visibility/successor is deferred here. Custody, signature,
                // exact bytes, canonical Merkle and bounded JSON were verified above.
                bound
                    .verify_generation_unchanged()
                    .map_err(CheckpointWriteError::from)?;
                continue;
            }
            let kura = self.state.kura_handle();
            if kura.emergency_fast_startup_enabled() {
                return Err(writer_invalid(
                    "pending checkpoints require Strict authenticated recovery",
                ));
            }
            let manifests = self.state.lane_manifests.read().clone();
            let nexus = self.state.nexus_snapshot();
            let outcome = super::try_read_snapshot_bundle(
                &self.state.ivm_execution_budget(),
                &bound,
                super::SnapshotRestorePurpose::CheckpointReconciliation,
                &kura,
                &manifests,
                &nexus,
                &self.state.query_handle,
                count,
                self.merkle_chunk_size,
                self.resource_policy,
                self.signing_key.public_key(),
                self.state.chain_id_ref(),
                self.state.network_id_ref(),
                &|restored| {
                    restored
                        .install_snapshot_reconciliation_configuration(&self.state)
                        .map_err(|error| super::TryReadError::ReconciliationConfiguration(error))
                },
                #[cfg(feature = "telemetry")]
                crate::telemetry::StateTelemetry::default(),
                &self.read_buffer_budget,
            )
            .map_err(CheckpointWriteError::from)?;
            outcome
                .state
                .authenticate_restored_keyed_state()
                .map_err(CheckpointWriteError::from)?;
            let view = outcome.state.view();
            let current = *view.native_execution_tip.get();
            let previous = outcome.state.native_execution_tip.predecessor_view();
            let prior = *previous.get();
            let world = view.world();
            use crate::state::WorldReadOnly as _;
            let policy = crate::runtime::checkpoint::committed_policy(
                world.runtime_selection(),
                world.parameters(),
            )
            .ok_or_else(|| {
                writer_invalid("pending checkpoint has no committed checkpoint policy")
            })?;
            if !current.zip(prior.flatten()).is_some_and(|(tip, parent)| {
                parent.height().checked_add(1) == Some(tip.height())
                    && tip.height() == entry.height
                    && tip.creation_time_ms() == entry.committed_ms
                    && policy.is_checkpoint(parent.creation_time_ms(), tip.creation_time_ms())
            }) {
                return Err(writer_invalid(
                    "pending checkpoint differs from its authenticated original cadence",
                ));
            }
            drop(previous);
            drop(view);
            drop(outcome);
            let schedule = self.current_checkpoint_schedule()?;
            self.finish_pending(&entry, &bound, Some(&schedule))?;
        }
        Ok(())
    }

    /// Verify all bounded signed artifacts before classifying an absent H+1 as a wait.
    /// This grants no State or retention authority; only the ordinary strict decoder and
    /// complete-State authentication above can grant those.
    fn verify_pending_envelope(
        &self,
        generation: &super::BoundSnapshotGeneration,
    ) -> WriterResult<()> {
        let manifest_path = generation
            .snapshot_directory
            .join(super::SNAPSHOT_FAST_MANIFEST_FILE_NAME);
        let manifest =
            super::decode_emergency_fast_manifest(&generation.bytes.fast_manifest, &manifest_path)
                .map_err(CheckpointWriteError::from)?;
        super::verify_snapshot_identity(
            &manifest.chain_id,
            manifest.network_id,
            self.state.chain_id_ref(),
            *self.state.network_id_ref(),
        )
        .map_err(CheckpointWriteError::from)?;
        if generation
            .checkpoint_identity
            .is_none_or(|(height, _)| height != manifest.committed_height)
            || manifest.payload_len != generation.payload.len
            || manifest.sccp_policy_hash != crate::state::sccp_genesis_policy_hash_v1()
        {
            return Err(writer_invalid(
                "pending manifest differs from its canonical identity",
            ));
        }
        let digest = super::parse_snapshot_digest_bytes(
            &generation.bytes.digest,
            &generation
                .snapshot_directory
                .join(super::SNAPSHOT_DIGEST_FILE_NAME),
        )
        .map_err(CheckpointWriteError::from)?;
        if generation.payload.bytes_sha256 != Some(digest) {
            return Err(writer_invalid("pending payload digest mismatch"));
        }
        let signature =
            std::str::from_utf8(&generation.bytes.signature).map_err(CheckpointWriteError::from)?;
        super::verify_signature_hex(
            signature,
            &super::snapshot_bundle_auth_digest(&digest, &generation.bytes.fast_manifest),
            self.signing_key.public_key(),
        )
        .map_err(CheckpointWriteError::from)?;
        let payload =
            super::read_bound_snapshot_payload(&generation.payload, &self.read_buffer_budget)
                .map_err(CheckpointWriteError::from)?
                .0;
        let value = norito::json::from_slice::<norito::json::Value>(&generation.bytes.merkle)
            .map_err(CheckpointWriteError::from)?;
        let merkle = super::SnapshotMerkleMetadata::from_json_value(&value)
            .map_err(CheckpointWriteError::from)?;
        if norito::json::to_json(&merkle)
            .map_err(CheckpointWriteError::from)?
            .as_bytes()
            != generation.bytes.merkle
        {
            return Err(writer_invalid("pending Merkle metadata is not canonical"));
        }
        merkle
            .verify_against_bytes(payload.as_slice(), self.merkle_chunk_size)
            .map_err(CheckpointWriteError::from)?;
        let summary =
            super::validate_snapshot_json_resources(payload.as_slice(), self.resource_policy)
                .map_err(CheckpointWriteError::from)?;
        let input = std::str::from_utf8(payload.as_slice()).map_err(CheckpointWriteError::from)?;
        super::verify_snapshot_restore_preflight(
            input,
            summary.block_hash_count,
            self.state.chain_id_ref(),
            *self.state.network_id_ref(),
        )
        .map_err(CheckpointWriteError::from)?;
        let geometry = super::geometry_checkpoint_from_snapshot(payload.as_slice())
            .map_err(CheckpointWriteError::from)?;
        if geometry.height != manifest.committed_height
            || geometry.block_hash != manifest.tip_hash
            || !summary.has_space_directory_manifests
        {
            return Err(writer_invalid(
                "pending payload identity differs from signed manifest",
            ));
        }
        generation
            .verify_generation_unchanged()
            .map_err(CheckpointWriteError::from)
    }
}

#[cfg(test)]
mod tests;
