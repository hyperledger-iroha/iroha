//! The checkpoints a node produces and keeps under its root's committed checkpoint policy
//! (`specs/live_runtime_upgrades.md` §7.3; `runtime_checkpoint_policy_v1`).
//!
//! A checkpoint at height `h` is the published State after block `h`, where `h` is the first
//! block committed in a cadence interval of the policy. When the snapshot maker publishes a
//! generation at such a height, it keeps that generation's five canonical artifacts under
//! `checkpoints/<height>-<committed_ms>` as private independent files, beside the rolling
//! generations that a restart restores from. The rolling GC never touches this directory.
//! Bounded copying preserves each artifact's exact bytes and single-link custody.
//!
//! After each new checkpoint the store keeps what the policy retains (the floor's own
//! checkpoint, the newest one, every one younger than `retain_all_ms` and one per
//! `retain_spacing_ms` above the floor) and every floor still waiting out its notice, and
//! removes the rest. A root without a committed policy produces and keeps no checkpoint.
//!
//! The publication owner captures exact canonical bytes from the finalized overlay at each
//! checkpoint height. The writer durably copies the five signed artifacts into a private
//! two-slot pending spool before acknowledging visibility. Queueing, signing, and disk I/O
//! run outside State/World writers and the commit fence; failure retains the original image.
//! Original visibility or strict post-recovery authentication authorizes atomic promotion
//! into this retained store. Pending artifacts never authorize retention or a startup floor.

use std::{
    collections::BTreeSet,
    io::{Read as _, Seek as _, Write as _},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use iroha_data_model::{
    parameter::RuntimeCheckpointPolicyV1, runtime::checkpoint::RuntimeBoundaryV1,
};

use super::{
    SNAPSHOT_DIGEST_FILE_NAME, SNAPSHOT_FAST_MANIFEST_FILE_NAME, SNAPSHOT_FILE_NAME,
    SNAPSHOT_GENERATION_GC_MAX_ENTRIES, SNAPSHOT_MERKLE_FILE_NAME, SNAPSHOT_SIGNATURE_FILE_NAME,
    SnapshotMaker,
};

/// How often the snapshot maker drains original checkpoint captures.
pub(crate) const CHECKPOINT_POLL: Duration = Duration::from_millis(500);

/// The directory of a snapshot store that holds its checkpoints.
pub(crate) const CHECKPOINTS_DIR_NAME: &str = "checkpoints";

/// The five canonical artifacts of a generation, which a checkpoint keeps.
#[cfg(test)]
const ARTIFACTS: [&str; 5] = [
    SNAPSHOT_FILE_NAME,
    SNAPSHOT_DIGEST_FILE_NAME,
    SNAPSHOT_SIGNATURE_FILE_NAME,
    SNAPSHOT_FAST_MANIFEST_FILE_NAME,
    SNAPSHOT_MERKLE_FILE_NAME,
];

/// Prefix of a checkpoint directory being assembled.
const STAGING_PREFIX: &str = ".staging-";

/// What the root's committed checkpoint policy decides at its committed tip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckpointSchedule {
    policy: RuntimeCheckpointPolicyV1,
    /// Height of the startup floor in force.
    floor: Option<u64>,
    /// Heights of the floors waiting out their notice.
    pending_floors: Vec<u64>,
    /// Committed time of the tip, in milliseconds.
    now_ms: u64,
}

impl CheckpointSchedule {
    /// The schedule `policy` sets with the root's `boundaries` at committed time `now_ms`.
    pub(crate) fn new<'a>(
        policy: RuntimeCheckpointPolicyV1,
        boundaries: impl IntoIterator<Item = &'a RuntimeBoundaryV1>,
        now_ms: u64,
    ) -> Self {
        let mut floor = None;
        let mut pending_floors = Vec::new();
        for boundary in boundaries {
            if let RuntimeBoundaryV1::StartupFloor(raised) = boundary {
                if boundary.in_force(now_ms) {
                    floor = floor.max(Some(raised.floor.height));
                } else {
                    pending_floors.push(raised.floor.height);
                }
            }
        }
        Self {
            policy,
            floor,
            pending_floors,
            now_ms,
        }
    }

    /// The heights of `held` checkpoints (height and committed time, sorted by height) the store
    /// keeps: what the policy retains and every floor waiting out its notice.
    pub(crate) fn retained(&self, held: &[(u64, u64)]) -> BTreeSet<u64> {
        let mut kept: BTreeSet<u64> = self
            .policy
            .retained(held, self.floor, self.now_ms)
            .into_iter()
            .collect();
        kept.extend(
            self.pending_floors
                .iter()
                .copied()
                .filter(|height| held.iter().any(|(held, _)| held == height)),
        );
        kept
    }
}

/// The snapshot maker's checkpoint bookkeeping. A failed write retains its original image.
#[derive(Default)]
pub(crate) struct MakerCheckpoints {
    /// When the maker last wrote its periodic snapshot.
    last_periodic: Option<Instant>,
    pub(super) receiver:
        Option<std::sync::mpsc::Receiver<std::sync::Arc<super::checkpoint_capture::CheckpointJob>>>,
    pending: Option<std::sync::Arc<super::checkpoint_capture::CheckpointJob>>,
}

impl SnapshotMaker {
    /// Arm the exact configured writer for certified replay. The caller owns both replay and
    /// this writer, so no separate worker, live recapture or successful-startup claim is needed.
    pub(crate) fn begin_checkpoint_replay(
        &mut self,
        state: &std::sync::Arc<crate::state::State>,
    ) -> Result<(), super::CheckpointDeferral> {
        if !std::sync::Arc::ptr_eq(&self.state, state) || self.checkpoints.receiver.is_none() {
            return Err(super::CheckpointDeferral::Persistence {
                height: state.committed_height() as u64,
                kind: std::io::ErrorKind::InvalidInput,
            });
        }
        self.state.runtime_checkpoint_capture.activate();
        self.state.runtime_checkpoint_capture.writer_ready();
        Ok(())
    }

    /// Drain only original replay jobs after publication released all its fences. Failed
    /// writes retain their exact image and typed I/O refusal. Certified visibility permits
    /// atomic relocation, but no rolling pointer, retention or orphan reconciliation runs.
    pub(crate) fn checkpoint_replay_step(
        &mut self,
    ) -> Result<(), crate::state::storage_transactions::TransactionsBlockError> {
        loop {
            let pending = self.checkpoints.pending.take().or_else(|| {
                self.checkpoints
                    .receiver
                    .as_ref()
                    .and_then(|receiver| receiver.try_recv().ok())
            });
            let Some(job) = pending else {
                return Ok(());
            };
            if job.abandoned() {
                continue;
            }
            if job.needs_write() {
                let result = self.persist_pending_checkpoint(&job.capture);
                job.acknowledge(result);
                if let Some(refusal) = job.take_refusal() {
                    self.checkpoints.pending = Some(job);
                    return Err(refusal);
                }
            }
            if job.visible() {
                if let Err(error) = self.promote_replayed_checkpoint(&job.capture) {
                    let refusal = error.into_publication(job.capture.height);
                    self.checkpoints.pending = Some(job);
                    return Err(refusal);
                }
            } else {
                self.checkpoints.pending = Some(job);
                return Ok(());
            }
        }
    }

    /// Persist every original publication image in order, then perform ordinary rolling work.
    /// A local I/O failure retains the image and the bounded producer backpressures instead
    /// of replacing a required checkpoint with a later tip.
    pub(super) fn tick(&mut self, shutting_down: bool) {
        self.state.runtime_checkpoint_capture.writer_ready();
        let owned = self
            .checkpoints
            .pending
            .as_ref()
            .map(|job| (job.capture.height, job.capture.committed_ms));
        if let Err(error) =
            tokio::task::block_in_place(|| self.reconcile_pending_checkpoints(owned))
        {
            iroha_logger::warn!(%error, "Pending checkpoint reconciliation refused; preserving every candidate");
            let job = self.checkpoints.pending.take().or_else(|| {
                self.checkpoints
                    .receiver
                    .as_ref()
                    .and_then(|receiver| receiver.try_recv().ok())
            });
            if let Some(job) = job {
                if job.needs_write() {
                    job.acknowledge(Err(error));
                }
                self.checkpoints.pending = Some(job);
            }
            return;
        }
        loop {
            let pending = self.checkpoints.pending.take().or_else(|| {
                self.checkpoints
                    .receiver
                    .as_ref()
                    .and_then(|receiver| receiver.try_recv().ok())
            });
            let Some(job) = pending else {
                break;
            };
            if job.abandoned() {
                continue;
            }
            if job.needs_write() {
                let result =
                    tokio::task::block_in_place(|| self.persist_pending_checkpoint(&job.capture));
                if let Err(error) = &result {
                    iroha_logger::warn!(height = job.capture.height, %error, "Retaining original checkpoint for durability retry");
                }
                job.acknowledge(result);
            }
            if job.visible() {
                if let Err(error) =
                    tokio::task::block_in_place(|| self.promote_visible_checkpoint(&job.capture))
                {
                    iroha_logger::warn!(height = job.capture.height, %error, "Retaining visible checkpoint for promotion retry");
                    self.checkpoints.pending = Some(job);
                    return;
                }
            } else {
                self.checkpoints.pending = Some(job);
                break;
            }
        }
        let periodic = shutting_down
            || self
                .checkpoints
                .last_periodic
                .is_none_or(|at| at.elapsed() >= self.create_every);
        if periodic {
            self.checkpoints.last_periodic = Some(Instant::now());
            self.create_snapshot();
        }
    }
}

/// The name of the checkpoint directory at `height`, committed at `committed_ms`.
fn entry_name(height: u64, committed_ms: u64) -> String {
    format!("{height}-{committed_ms}")
}

/// The height and committed time a checkpoint directory name states: two decimal numbers
/// without leading zeros, joined by one hyphen.
pub(crate) fn parse_entry_name(name: &str) -> Option<(u64, u64)> {
    let (height, committed_ms) = name.split_once('-')?;
    let number = |text: &str| {
        let canonical = !text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit())
            && (text == "0" || !text.starts_with('0'));
        canonical.then(|| text.parse::<u64>().ok()).flatten()
    };
    Some((number(height)?, number(committed_ms)?))
}

/// The checkpoints directory of the store at `store_dir`.
pub(crate) fn checkpoints_dir(store_dir: &Path) -> PathBuf {
    store_dir.join(CHECKPOINTS_DIR_NAME)
}

/// The checkpoints the store at `store_dir` keeps: each directory's height, committed time and
/// path, sorted by height. Staging directories and foreign names are not checkpoints.
///
/// # Errors
/// The directory cannot be read, or holds more entries than any store keeps.
pub(crate) fn entries(store_dir: &Path) -> std::io::Result<Vec<(u64, u64, PathBuf)>> {
    let directory = checkpoints_dir(store_dir);
    let listing = match std::fs::read_dir(&directory) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut entries = Vec::new();
    for (scanned, entry) in listing.enumerate() {
        if scanned == SNAPSHOT_GENERATION_GC_MAX_ENTRIES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "checkpoint entry count exceeds the hard scan bound",
            ));
        }
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let Some((height, committed_ms)) = entry.file_name().to_str().and_then(parse_entry_name)
        else {
            continue;
        };
        entries.push((height, committed_ms, entry.path()));
    }
    entries.sort_unstable_by_key(|(height, committed_ms, _)| (*height, *committed_ms));
    Ok(entries)
}

fn sync_directory(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// Test fixture: keep the published generation at `generation_dir`, of the State after the
/// block at checkpoint height `height` committed at `committed_ms`, directly as a retained
/// checkpoint of the store at `store_dir`. Production never retains a generation this way: it
/// promotes the original capture from the pending spool once visibility authorizes it. Bounded
/// copies keep both the source generation and retained checkpoint single-linked. Keeping an
/// identical checkpoint again verifies it without rewriting it.
///
/// # Errors
/// The directory or artifacts violate custody, an artifact exceeds its configured bound or
/// changes during copying, or the result cannot be made durable.
#[cfg(test)]
pub(crate) fn keep(
    store_dir: &Path,
    generation_dir: &Path,
    height: u64,
    committed_ms: u64,
    max_payload_bytes: std::num::NonZeroUsize,
    merkle_chunk_size: std::num::NonZeroUsize,
) -> std::io::Result<()> {
    copy_to_container(
        store_dir,
        generation_dir,
        CHECKPOINTS_DIR_NAME,
        height,
        committed_ms,
        max_payload_bytes,
        merkle_chunk_size,
    )
}

/// Copy the same canonical private artifacts into an explicit checkpoint container.
/// The caller owns any pending quota and whether committed retention is authorized.
pub(super) fn copy_to_container(
    store_dir: &Path,
    generation_dir: &Path,
    container: &str,
    height: u64,
    committed_ms: u64,
    max_payload_bytes: std::num::NonZeroUsize,
    merkle_chunk_size: std::num::NonZeroUsize,
) -> std::io::Result<()> {
    let invalid = |error: super::TryReadError| match error {
        super::TryReadError::IO(original, _) => original,
        other => std::io::Error::new(std::io::ErrorKind::InvalidData, other.to_string()),
    };
    let store_identity = super::direct_snapshot_directory_identity(store_dir).map_err(invalid)?;
    let source_identity =
        super::direct_snapshot_directory_identity(generation_dir).map_err(invalid)?;
    let payload_limit = u64::try_from(max_payload_bytes.get()).unwrap_or(u64::MAX);
    let payload = bind_copy_source(&generation_dir.join(SNAPSHOT_FILE_NAME), payload_limit)?;
    let merkle_limit = super::snapshot_merkle_max_bytes(payload.len, merkle_chunk_size);
    let mut sources = vec![(SNAPSHOT_FILE_NAME, payload)];
    for (artifact, limit) in [
        (SNAPSHOT_DIGEST_FILE_NAME, super::SNAPSHOT_DIGEST_MAX_BYTES),
        (
            SNAPSHOT_SIGNATURE_FILE_NAME,
            super::SNAPSHOT_SIGNATURE_MAX_BYTES,
        ),
        (
            SNAPSHOT_FAST_MANIFEST_FILE_NAME,
            super::SNAPSHOT_FAST_MANIFEST_MAX_BYTES,
        ),
        (SNAPSHOT_MERKLE_FILE_NAME, merkle_limit),
    ] {
        sources.push((
            artifact,
            bind_copy_source(&generation_dir.join(artifact), limit)?,
        ));
    }
    let directory = store_dir.join(container);
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory)?;
    sync_directory(store_dir)?;
    let directory_identity =
        super::direct_snapshot_directory_identity(&directory).map_err(invalid)?;
    let name = entry_name(height, committed_ms);
    let destination = directory.join(&name);
    let exists = match std::fs::symlink_metadata(&destination) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if exists {
        super::direct_snapshot_directory_identity(&destination).map_err(invalid)?;
        if !super::snapshot_generation_has_exact_artifact_inventory(&destination)
            .map_err(invalid)?
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "checkpoint artifact inventory changed",
            ));
        }
        for (artifact, source) in &sources {
            let retained = bind_copy_source(&destination.join(artifact), source.max_bytes)?;
            if retained.len != source.len || retained.bytes_sha256 != source.bytes_sha256 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "retained checkpoint differs from its generation",
                ));
            }
        }
        if super::direct_snapshot_directory_identity(store_dir).map_err(invalid)? != store_identity
            || super::direct_snapshot_directory_identity(generation_dir).map_err(invalid)?
                != source_identity
            || super::direct_snapshot_directory_identity(&directory).map_err(invalid)?
                != directory_identity
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "checkpoint directory changed during verification",
            ));
        }
        for (_, source) in &sources {
            source.handle.sync_all()?;
        }
        for (artifact, source) in &sources {
            bind_copy_source(&destination.join(artifact), source.max_bytes)?
                .handle
                .sync_all()?;
        }
        sync_directory(&destination)?;
        sync_directory(&directory)?;
        sync_directory(store_dir)?;
        return Ok(());
    }
    let staging = directory.join(format!("{STAGING_PREFIX}{name}"));
    match std::fs::remove_dir_all(&staging) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    builder.recursive(false);
    builder.create(&staging)?;
    let staging_identity = super::direct_snapshot_directory_identity(&staging).map_err(invalid)?;
    for (artifact, source) in &sources {
        copy_artifact(source, &staging.join(artifact))?;
    }
    if super::direct_snapshot_directory_identity(store_dir).map_err(invalid)? != store_identity
        || super::direct_snapshot_directory_identity(generation_dir).map_err(invalid)?
            != source_identity
        || super::direct_snapshot_directory_identity(&directory).map_err(invalid)?
            != directory_identity
        || super::direct_snapshot_directory_identity(&staging).map_err(invalid)? != staging_identity
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "checkpoint directory changed during copying",
        ));
    }
    sync_directory(&staging)?;
    std::fs::rename(&staging, &destination)?;
    sync_directory(&directory)
}

fn bind_copy_source(path: &Path, maximum: u64) -> std::io::Result<super::BoundSnapshotFile> {
    super::bind_snapshot_file_handle(path, maximum)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "checkpoint artifact is missing",
        )
    })
}

/// Copy one pre-bound artifact with fixed scratch, exact length/digest, and private custody.
fn copy_artifact(source: &super::BoundSnapshotFile, destination: &Path) -> std::io::Result<()> {
    use sha2::{Digest as _, Sha256};

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut output = options.open(destination)?;
    let mut input = source.handle.as_ref();
    input.seek(std::io::SeekFrom::Start(0))?;
    let mut remaining = source.len;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        let read = input.read(&mut buffer[..requested])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "checkpoint artifact shortened during copying",
            ));
        }
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut excess = [0_u8; 1];
    let actual: [u8; 32] = digest.finalize().into();
    if input.read(&mut excess)? != 0 || source.bytes_sha256 != Some(actual) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "checkpoint artifact changed during copying",
        ));
    }
    super::verify_bound_snapshot_file_metadata_at(&source.path, source)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))?;
    output.sync_all()?;
    let retained = bind_copy_source(destination, source.max_bytes)?;
    if retained.len != source.len || retained.bytes_sha256 != Some(actual) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "checkpoint copy differs from its source",
        ));
    }
    Ok(())
}

/// Remove the checkpoints of the store at `store_dir` that `schedule` no longer retains, and
/// any staging directory an interrupted [`keep`] left. Returns the heights removed.
///
/// # Errors
/// The directory cannot be read, or a checkpoint cannot be removed.
pub(crate) fn retain(store_dir: &Path, schedule: &CheckpointSchedule) -> std::io::Result<Vec<u64>> {
    let held = entries(store_dir)?;
    let times: Vec<(u64, u64)> = held
        .iter()
        .map(|(height, committed_ms, _)| (*height, *committed_ms))
        .collect();
    let kept = schedule.retained(&times);
    let mut removed = Vec::new();
    for (height, _, path) in held {
        if !kept.contains(&height) {
            std::fs::remove_dir_all(&path)?;
            removed.push(height);
        }
    }
    let directory = checkpoints_dir(store_dir);
    if let Ok(listing) = std::fs::read_dir(&directory) {
        for entry in listing.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(STAGING_PREFIX))
            {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
    }
    if !removed.is_empty() {
        sync_directory(&directory)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        runtime::checkpoint::{RuntimeStartupFloorV1, StartupFloorV1},
    };

    use super::*;

    const HOUR_MS: u64 = 60 * 60 * 1000;

    fn policy() -> RuntimeCheckpointPolicyV1 {
        RuntimeCheckpointPolicyV1 {
            cadence_ms: HOUR_MS,
            retain_all_ms: 3 * HOUR_MS,
            retain_spacing_ms: 24 * HOUR_MS,
            recovery_window_ms: 24 * HOUR_MS,
            floor_notice_ms: HOUR_MS,
        }
    }

    fn floor(height: u64, effective_ms: u64) -> RuntimeBoundaryV1 {
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])));
        let floor = StartupFloorV1 {
            network,
            height,
            block_hash: [2; 32],
            committed_ms: 0,
        };
        let key = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        RuntimeBoundaryV1::StartupFloor(RuntimeStartupFloorV1 {
            attestations: vec![
                RuntimeStartupFloorV1::statement(&floor, key.public_key().clone())
                    .unwrap()
                    .sign(key.private_key())
                    .unwrap(),
            ],
            floor,
            effective_ms,
        })
    }

    #[test]
    fn checkpoint_names_state_their_height_and_committed_time() {
        assert_eq!(parse_entry_name(&entry_name(42, 7_000)), Some((42, 7_000)));
        assert_eq!(parse_entry_name("0-0"), Some((0, 0)));
        for foreign in [
            "42",
            "042-7",
            "42-07",
            "42--7",
            "-42-7",
            "42-7-1",
            "a-7",
            ".staging-42-7",
        ] {
            assert_eq!(parse_entry_name(foreign), None, "{foreign}");
        }
    }

    #[test]
    fn the_schedule_keeps_what_the_policy_retains_and_every_pending_floor() {
        let now = 100 * HOUR_MS;
        // Checkpoints every hour from the third hour.
        let held: Vec<(u64, u64)> = (3..=100).map(|hour| (hour * 10, hour * HOUR_MS)).collect();
        let in_force = floor(500, now - HOUR_MS);
        let pending = floor(600, now + HOUR_MS);
        let schedule = CheckpointSchedule::new(policy(), [&in_force, &pending], now);
        let kept = schedule.retained(&held);
        assert!(kept.contains(&500), "the floor in force");
        assert!(kept.contains(&600), "the floor waiting out its notice");
        assert!(kept.contains(&1000), "the newest");
        assert!(
            (980..=1000)
                .step_by(10)
                .all(|height| kept.contains(&height)),
            "every checkpoint younger than three hours"
        );
        assert!(
            kept.iter().all(|height| *height >= 500),
            "nothing below the floor in force"
        );
        assert!(
            !kept.contains(&970),
            "an old checkpoint inside a spacing interval"
        );
        // Without boundaries the policy alone decides.
        let unbounded = CheckpointSchedule::new(policy(), std::iter::empty(), now);
        assert!(unbounded.retained(&held).contains(&30));
    }

    #[test]
    fn a_kept_checkpoint_survives_its_generation_and_retention_removes_the_rest() {
        let store = tempfile::tempdir().unwrap();
        let maximum = std::num::NonZeroUsize::new(1024 * 1024).unwrap();
        let chunk = std::num::NonZeroUsize::new(4096).unwrap();
        let mut generations = Vec::new();
        for height in [10_u64, 20, 30] {
            let generation = store.path().join("generations").join(format!("g{height}"));
            std::fs::create_dir_all(&generation).unwrap();
            for artifact in ARTIFACTS {
                std::fs::write(generation.join(artifact), format!("{artifact}@{height}")).unwrap();
            }
            keep(
                store.path(),
                &generation,
                height,
                height * HOUR_MS,
                maximum,
                chunk,
            )
            .unwrap();
            // Keeping it again changes nothing.
            keep(
                store.path(),
                &generation,
                height,
                height * HOUR_MS,
                maximum,
                chunk,
            )
            .unwrap();
            for artifact in ARTIFACTS {
                let source =
                    bind_copy_source(&generation.join(artifact), maximum.get() as u64).unwrap();
                let retained = bind_copy_source(
                    &checkpoints_dir(store.path())
                        .join(entry_name(height, height * HOUR_MS))
                        .join(artifact),
                    maximum.get() as u64,
                )
                .unwrap();
                assert_eq!(source.bytes_sha256, retained.bytes_sha256);
                assert_ne!(
                    source.identity, retained.identity,
                    "checkpoint must have independent custody"
                );
            }
            generations.push(generation);
        }
        // The rolling GC removes a generation; its checkpoint keeps every artifact.
        std::fs::remove_dir_all(&generations[0]).unwrap();
        let held = entries(store.path()).unwrap();
        assert_eq!(
            held.iter()
                .map(|(height, committed_ms, _)| (*height, *committed_ms))
                .collect::<Vec<_>>(),
            [(10, 10 * HOUR_MS), (20, 20 * HOUR_MS), (30, 30 * HOUR_MS)]
        );
        assert_eq!(
            std::fs::read_to_string(held[0].2.join(SNAPSHOT_FILE_NAME)).unwrap(),
            format!("{SNAPSHOT_FILE_NAME}@10")
        );
        // An interrupted keep leaves a staging directory, which retention clears.
        std::fs::create_dir_all(checkpoints_dir(store.path()).join(".staging-40-1")).unwrap();
        let schedule = CheckpointSchedule::new(
            RuntimeCheckpointPolicyV1 {
                retain_all_ms: HOUR_MS,
                ..policy()
            },
            std::iter::empty(),
            30 * HOUR_MS,
        );
        assert_eq!(retain(store.path(), &schedule).unwrap(), [20]);
        assert_eq!(
            entries(store.path())
                .unwrap()
                .iter()
                .map(|(height, _, _)| *height)
                .collect::<Vec<_>>(),
            [10, 30]
        );
        assert!(!checkpoints_dir(store.path()).join(".staging-40-1").exists());
    }

    #[test]
    fn checkpoint_copy_refuses_oversized_or_changed_source_artifacts() {
        let root = tempfile::tempdir().unwrap();
        let source_path = root.path().join("source");
        std::fs::write(&source_path, b"original").unwrap();
        assert!(bind_copy_source(&source_path, 7).is_err());
        let source = bind_copy_source(&source_path, 8).unwrap();
        std::fs::write(&source_path, b"tampered").unwrap();
        assert!(copy_artifact(&source, &root.path().join("copy")).is_err());
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&source_path, &alias).unwrap();
            assert!(bind_copy_source(&alias, 8).is_err());
            std::fs::remove_file(&alias).unwrap();
            std::fs::hard_link(&source_path, &alias).unwrap();
            assert!(bind_copy_source(&source_path, 8).is_err());
        }
    }
}
