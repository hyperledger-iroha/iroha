//! The checkpoints a node produces and keeps under its root's committed checkpoint policy
//! (`specs/live_runtime_upgrades.md` §7.3; `runtime_checkpoint_policy_v1`).
//!
//! A checkpoint at height `h` is the published State after block `h`, where `h` is the first
//! block committed in a cadence interval of the policy. When the snapshot maker publishes a
//! generation at such a height, it keeps that generation's five canonical artifacts under
//! `checkpoints/<height>-<committed_ms>` as hard links, beside the rolling generations that a
//! restart restores from. The rolling GC never touches this directory, and keeping a checkpoint
//! copies no bytes and re-hashes nothing.
//!
//! After each new checkpoint the store keeps what the policy retains (the floor's own
//! checkpoint, the newest one, every one younger than `retain_all_ms` and one per
//! `retain_spacing_ms` above the floor) and every floor still waiting out its notice, and
//! removes the rest. A root without a committed policy produces and keeps no checkpoint.
//!
//! Production does not hold back block publication: the maker writes the tip when it finds the
//! tip at a checkpoint height, and if the next block publishes first the capture is overtaken
//! and that node holds no checkpoint at that height (a floor needs every validator to hold it).
//! TODO(§7.3): capture the published State of a checkpoint height from a view pinned at its
//! publication, so a chain whose blocks commit faster than a capture still yields every
//! checkpoint.

use std::{
    collections::BTreeSet,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader, parameter::RuntimeCheckpointPolicyV1,
    runtime::checkpoint::RuntimeBoundaryV1,
};
use mv::storage::StorageReadOnly as _;

use super::{
    SNAPSHOT_CURRENT_FILE_NAME, SNAPSHOT_CURRENT_MAX_BYTES, SNAPSHOT_DIGEST_FILE_NAME,
    SNAPSHOT_FAST_MANIFEST_FILE_NAME, SNAPSHOT_FILE_NAME, SNAPSHOT_GENERATION_GC_MAX_ENTRIES,
    SNAPSHOT_GENERATIONS_DIR_NAME, SNAPSHOT_MERKLE_FILE_NAME, SNAPSHOT_SIGNATURE_FILE_NAME,
    SnapshotMaker, bind_snapshot_file, parse_snapshot_current_pointer, signed_generation_manifest,
};
use crate::state::{State, StateReadOnly, WorldReadOnly};

/// How often the snapshot maker looks for a checkpoint height at the tip.
pub(crate) const CHECKPOINT_POLL: Duration = Duration::from_millis(500);

/// The directory of a snapshot store that holds its checkpoints.
pub(crate) const CHECKPOINTS_DIR_NAME: &str = "checkpoints";

/// The five canonical artifacts of a generation, which a checkpoint keeps.
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
    /// The schedule of the root `state` executes, if it has activated the lifecycle State and
    /// committed a well-formed checkpoint policy.
    pub(crate) fn of(state: &State) -> Option<Self> {
        let now_ms = state.latest_block_creation_time_ms_fast()?;
        let view = state.view();
        let world = view.world();
        let policy = crate::runtime::checkpoint::committed_policy(
            world.runtime_selection(),
            WorldReadOnly::parameters(world),
        )?;
        Some(Self::new(
            policy,
            world
                .runtime_boundaries()
                .iter()
                .map(|(_, boundary)| boundary),
            now_ms,
        ))
    }

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

    /// Whether the block committed at `committed_ms`, whose parent was committed at
    /// `parent_ms`, is a checkpoint height.
    pub(crate) const fn is_checkpoint(&self, parent_ms: u64, committed_ms: u64) -> bool {
        self.policy.is_checkpoint(parent_ms, committed_ms)
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

/// A tip the snapshot maker classified: its hash, and its height, committed time and the
/// schedule in force when it is a checkpoint height.
type ClassifiedTip = (HashOf<BlockHeader>, Option<(u64, u64, CheckpointSchedule)>);

/// The snapshot maker's checkpoint bookkeeping.
#[derive(Debug, Default)]
pub(crate) struct MakerCheckpoints {
    /// When the maker last wrote its periodic snapshot.
    last_periodic: Option<Instant>,
    /// The tip the maker last classified. Committed State changes only with the tip, so the
    /// maker reads it once per tip.
    classified: Option<ClassifiedTip>,
    /// Height of the last checkpoint the maker kept, or tried to keep: it tries once per height.
    kept: Option<u64>,
}

impl SnapshotMaker {
    /// One tick of the maker: write the tip when it sits at a checkpoint height of the root's
    /// committed policy, or when the periodic interval has passed or the node shuts down, and
    /// keep a checkpoint it wrote.
    pub(super) fn tick(&mut self, shutting_down: bool) {
        let due = self.due_checkpoint();
        let periodic = shutting_down
            || self
                .checkpoints
                .last_periodic
                .is_none_or(|at| at.elapsed() >= self.create_every);
        if due.is_none() && !periodic {
            return;
        }
        if periodic {
            self.checkpoints.last_periodic = Some(Instant::now());
        }
        self.create_snapshot();
        if let Some((height, committed_ms, tip, schedule)) = due
            && self.latest_block_hash == Some(tip)
        {
            self.keep_checkpoint(&schedule, height, committed_ms);
        }
    }

    /// The tip's height, committed time and hash, with the schedule in force, when it is a
    /// checkpoint height the maker has not kept yet.
    fn due_checkpoint(&mut self) -> Option<(u64, u64, HashOf<BlockHeader>, CheckpointSchedule)> {
        let latest = self.state.latest_block_hash_fast()?;
        if self
            .checkpoints
            .classified
            .as_ref()
            .is_none_or(|(tip, _)| *tip != latest)
        {
            self.checkpoints.classified = Some(self.classify_tip(latest));
        }
        let (tip, classified) = self.checkpoints.classified.as_ref()?;
        let (height, committed_ms, schedule) = classified.as_ref()?;
        (self.checkpoints.kept != Some(*height))
            .then(|| (*height, *committed_ms, *tip, schedule.clone()))
    }

    /// The committed tip's hash, and its height, committed time and the schedule in force if the
    /// root's committed policy makes it a checkpoint height: the first block committed in its
    /// cadence interval. A root without a committed policy has no checkpoint height; `latest`
    /// names a tip without a native execution record.
    fn classify_tip(&self, latest: HashOf<BlockHeader>) -> ClassifiedTip {
        let schedule = CheckpointSchedule::of(&self.state);
        let view = self.state.view();
        let Some(tip) = StateReadOnly::native_execution_tip(&view) else {
            return (latest, None);
        };
        let Some(schedule) = schedule else {
            return (tip.iroha_hash(), None);
        };
        let height = tip.height();
        let committed_ms = tip.creation_time_ms();
        let parent_ms = match *view.native_execution_tip_predecessor.get() {
            Some(Some(parent)) if parent.height().checked_add(1) == Some(height) => {
                Some(parent.creation_time_ms())
            }
            _ => usize::try_from(height.saturating_sub(1))
                .ok()
                .and_then(NonZeroUsize::new)
                .and_then(|parent| {
                    view.kura()
                        .get_block(parent, &self.read_buffer_budget)
                        .ok()
                        .flatten()
                })
                .and_then(|block| u64::try_from(block.header().creation_time().as_millis()).ok()),
        };
        let checkpoint = parent_ms
            .filter(|parent_ms| schedule.is_checkpoint(*parent_ms, committed_ms))
            .map(|_| (height, committed_ms, schedule));
        (tip.iroha_hash(), checkpoint)
    }

    /// Keep the current generation, which the maker just published at checkpoint `height`, as a
    /// checkpoint, then apply the schedule's retention.
    fn keep_checkpoint(&mut self, schedule: &CheckpointSchedule, height: u64, committed_ms: u64) {
        let result = (|| -> Result<Vec<u64>, String> {
            let pointer_path = self.store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
            let (_, pointer) = bind_snapshot_file(&pointer_path, SNAPSHOT_CURRENT_MAX_BYTES)
                .map_err(|error| error.to_string())?
                .ok_or("no current snapshot generation")?;
            let name = parse_snapshot_current_pointer(&pointer, &pointer_path)
                .map_err(|error| error.to_string())?;
            let generation = self
                .store_dir
                .join(SNAPSHOT_GENERATIONS_DIR_NAME)
                .join(name);
            let manifest = signed_generation_manifest(&generation, self.signing_key.public_key())
                .ok_or("the current generation's manifest does not verify")?;
            if manifest.committed_height != height {
                return Err("the current generation is not at the checkpoint height".into());
            }
            keep(&self.store_dir, &generation, height, committed_ms)
                .map_err(|error| error.to_string())?;
            retain(&self.store_dir, schedule).map_err(|error| error.to_string())
        })();
        self.checkpoints.kept = Some(height);
        match result {
            Ok(removed) => {
                iroha_logger::info!(height, ?removed, "Kept a runtime checkpoint");
            }
            Err(error) => {
                iroha_logger::warn!(height, %error, "Failed to keep a runtime checkpoint");
            }
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

/// Keep the published generation at `generation_dir`, of the State after the block at
/// checkpoint height `height` committed at `committed_ms`, as a checkpoint of the store at
/// `store_dir`. The artifacts are hard links, so this copies no bytes; keeping the same
/// checkpoint again changes nothing.
///
/// # Errors
/// The directory cannot be created, an artifact cannot be linked, or the result cannot be made
/// durable.
pub(crate) fn keep(
    store_dir: &Path,
    generation_dir: &Path,
    height: u64,
    committed_ms: u64,
) -> std::io::Result<()> {
    let directory = checkpoints_dir(store_dir);
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory)?;
    let name = entry_name(height, committed_ms);
    let destination = directory.join(&name);
    if destination.is_dir() {
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
    for artifact in ARTIFACTS {
        std::fs::hard_link(generation_dir.join(artifact), staging.join(artifact))?;
    }
    sync_directory(&staging)?;
    std::fs::rename(&staging, &destination)?;
    sync_directory(&directory)
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
        let mut generations = Vec::new();
        for height in [10_u64, 20, 30] {
            let generation = store.path().join("generations").join(format!("g{height}"));
            std::fs::create_dir_all(&generation).unwrap();
            for artifact in ARTIFACTS {
                std::fs::write(generation.join(artifact), format!("{artifact}@{height}")).unwrap();
            }
            keep(store.path(), &generation, height, height * HOUR_MS).unwrap();
            // Keeping it again changes nothing.
            keep(store.path(), &generation, height, height * HOUR_MS).unwrap();
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
}
