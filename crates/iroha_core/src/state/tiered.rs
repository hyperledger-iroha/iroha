//! Tiered storage backend for the World State View (WSV).
//!
//! The backend promotes frequently updated keys to a hot in-memory tier while
//! demoting colder entries to an on-disk spill. Each snapshot computes
//! recency-based priorities, writes cold payloads using the canonical Norito
//! encoding, and emits a manifest so hosts can hydrate cold shards lazily.
//! Snapshots can be built incrementally from per-block diffs to avoid full WSV scans,
//! and heavy snapshot work can be offloaded after commit to reduce block latency.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{BufWriter, ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result};
use hex::ToHex as _;
use iroha_config::parameters::actual::{LaneConfig, LaneConfigEntry};
use iroha_data_model::prelude::Name;
use mv::storage::StorageReadOnly;
use norito::{
    core::NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use sha2::{Digest as _, Sha256};

use super::World;
use crate::telemetry::StateTelemetry;

const WSV_COLD_COMPONENT: &str = "wsv_cold";
const DA_CACHE_HIT: &str = "hit";
const DA_CACHE_MISS: &str = "miss";
const DA_CHURN_EVICTED: &str = "evicted";
const DA_CHURN_REHYDRATED: &str = "rehydrated";

/// Lightweight handle describing a hot/cold storage split.
#[derive(Debug, Clone, Default)]
pub struct TieredStateBackend {
    /// Enable tiering; when false snapshots are skipped regardless of other knobs.
    enabled: bool,
    /// Maximum number of keys to keep hot (0 = unlimited).
    hot_retained_keys: usize,
    /// Hot-tier byte budget based on deterministic in-memory WSV sizing (0 = unlimited).
    hot_retained_bytes: u64,
    /// Minimum snapshots to retain newly hot entries before demotion (0 = disabled).
    hot_retained_grace_snapshots: u64,
    /// Optional on-disk spill root for cold shards.
    cold_store_root: Option<PathBuf>,
    /// Optional on-disk spill root for DA-backed cold shards.
    da_store_root: Option<PathBuf>,
    /// Number of snapshot directories to retain (0 = keep all).
    max_snapshots: usize,
    /// Optional cold-tier byte budget across snapshots (0 = unlimited).
    max_cold_bytes: u64,
    /// Monotonically increasing snapshot counter.
    snapshot_counter: u64,
    /// Whether the snapshot counter has been seeded from disk.
    snapshot_counter_seeded: bool,
    /// Per-entry metadata tracking heat and payload hashes.
    entries: BTreeMap<TieredEntryId, EntryMetadata>,
    /// Stable key metadata required to rebuild snapshot manifests incrementally.
    entry_keys: BTreeMap<TieredEntryId, EntryKey>,
    /// Cached manifest of the latest snapshot for diagnostics.
    last_manifest: Option<TieredSnapshotManifest>,
    /// Optional telemetry sink for DA-backed cold storage activity.
    telemetry: Option<StateTelemetry>,
}

#[derive(Debug)]
struct ColdEntryPlan {
    rel_path: PathBuf,
    entry: EntryScore,
    manifest_index: usize,
    reuse_source: Option<PathBuf>,
}

#[derive(Debug)]
struct TieredSnapshotPlan {
    root: PathBuf,
    snapshot_dir: PathBuf,
    manifest: TieredSnapshotManifest,
    cold_entries: Vec<ColdEntryPlan>,
}

#[derive(Default)]
struct SnapshotPayloadCache {
    payloads: BTreeMap<TieredEntryId, Vec<u8>>,
}

/// Changed WSV keys captured during a block for incremental snapshotting.
#[derive(Debug, Default, Clone)]
pub(crate) struct TieredSnapshotDiff {
    entries: Vec<TieredKeyHandle>,
}

impl TieredSnapshotDiff {
    /// Record a touched entry.
    pub(crate) fn push(&mut self, entry: TieredKeyHandle) {
        self.entries.push(entry);
    }

    /// Read the captured entries.
    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[TieredKeyHandle] {
        &self.entries
    }
}

/// Changed WSV keys captured with their updated payloads for background snapshots.
#[derive(Default)]
pub(crate) struct TieredSnapshotPayload {
    entries: Vec<TieredSnapshotPayloadEntry>,
}

struct TieredSnapshotPayloadEntry {
    key: TieredKeyHandle,
    value: Option<Box<dyn TieredSnapshotValue>>,
}

impl TieredSnapshotPayload {
    pub(crate) fn push_value<T>(&mut self, key: TieredKeyHandle, value: Option<T>)
    where
        T: json::JsonSerialize + MeasuredBytes + Send + Sync + 'static,
    {
        let value = value.map(|value| {
            let boxed: Box<dyn TieredSnapshotValue> = Box::new(value);
            boxed
        });
        self.entries.push(TieredSnapshotPayloadEntry { key, value });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl From<&TieredSnapshotPayload> for TieredSnapshotDiff {
    fn from(payload: &TieredSnapshotPayload) -> Self {
        let mut diff = TieredSnapshotDiff::default();
        for entry in &payload.entries {
            diff.push(entry.key.clone());
        }
        diff
    }
}

trait TieredSnapshotValue: Send + Sync {
    fn measured_bytes(&self) -> usize;
    fn encode_json(&self) -> Result<Vec<u8>>;
}

impl<T> TieredSnapshotValue for T
where
    T: json::JsonSerialize + MeasuredBytes + Send + Sync + 'static,
{
    fn measured_bytes(&self) -> usize {
        MeasuredBytes::measured_bytes(self)
    }

    fn encode_json(&self) -> Result<Vec<u8>> {
        json::to_vec(self).wrap_err("failed to encode snapshot value as JSON")
    }
}

struct CollectContext<'a> {
    snapshot_idx: u64,
    scores: &'a mut Vec<EntryScore>,
    seen: &'a mut BTreeSet<TieredEntryId>,
}

impl TieredStateBackend {
    /// Construct a backend with explicit limits.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enabled: bool,
        hot_retained_keys: usize,
        hot_retained_bytes: u64,
        hot_retained_grace_snapshots: u64,
        cold_store_root: Option<PathBuf>,
        da_store_root: Option<PathBuf>,
        max_snapshots: usize,
        max_cold_bytes: u64,
    ) -> Self {
        let mut backend = Self {
            enabled,
            hot_retained_keys,
            hot_retained_bytes,
            hot_retained_grace_snapshots,
            cold_store_root,
            da_store_root,
            max_snapshots,
            max_cold_bytes,
            snapshot_counter: 0,
            snapshot_counter_seeded: false,
            entries: BTreeMap::new(),
            entry_keys: BTreeMap::new(),
            last_manifest: None,
            telemetry: None,
        };
        if backend.enabled {
            backend.ensure_cold_roots().ok();
            let _ = backend.seed_snapshot_counter_if_needed();
        }
        backend
    }

    /// Attach a telemetry sink for DA-backed cold storage activity.
    pub fn attach_telemetry(&mut self, telemetry: StateTelemetry) {
        self.telemetry = Some(telemetry);
    }

    /// Record a snapshot of the current world.
    pub fn record_world_snapshot(&mut self, world: &World) -> Result<()> {
        if let Some(plan) = self.plan_world_snapshot(world)? {
            self.execute_snapshot_plan(plan, world)?;
        }
        Ok(())
    }

    /// Record a snapshot using a diff of touched entries.
    pub(crate) fn record_world_snapshot_with_diff(
        &mut self,
        world: &World,
        diff: &TieredSnapshotDiff,
    ) -> Result<()> {
        if self.entries.is_empty() {
            return self.record_world_snapshot(world);
        }
        if let Some(plan) = self.plan_world_snapshot_with_diff(world, diff)? {
            self.execute_snapshot_plan(plan, world)?;
        }
        Ok(())
    }

    /// Record a snapshot using a diff payload without accessing the live world.
    pub(crate) fn record_world_snapshot_with_payload(
        &mut self,
        payload: &TieredSnapshotPayload,
    ) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let Some((root, snapshot_idx, snapshot_dir)) = self.prepare_snapshot()? else {
            return Ok(());
        };

        let payload_cache = if payload.is_empty() {
            SnapshotPayloadCache::default()
        } else {
            self.apply_snapshot_payload(snapshot_idx, payload)?
        };

        let scores = self.build_scores_from_keys(snapshot_idx);
        let plan = self.build_snapshot_plan(
            root,
            snapshot_idx,
            snapshot_dir,
            scores,
            Some(&payload_cache.payloads),
        )?;
        self.execute_snapshot_plan_with_payload(plan, &payload_cache.payloads)?;
        Ok(())
    }

    fn seed_snapshot_counter_if_needed(&mut self) -> Result<()> {
        if self.snapshot_counter_seeded {
            return Ok(());
        }

        let roots = self.cold_roots();
        if roots.is_empty() {
            self.snapshot_counter_seeded = true;
            return Ok(());
        }

        self.ensure_cold_roots()
            .wrap_err("failed to prepare cold tier root directory")?;
        for root in &roots {
            if root.exists() {
                self.recover_snapshot_artifacts(root)?;
            }
        }

        let mut max_idx = 0u64;
        for root in roots {
            if !root.exists() {
                continue;
            }
            for entry in fs::read_dir(root).wrap_err_with(|| {
                format!(
                    "failed to read cold tier root {path}",
                    path = root.display()
                )
            })? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if !file_type.is_dir() {
                    continue;
                }
                if let Some(idx) = Self::parse_snapshot_dir_name(&entry.file_name()) {
                    max_idx = max_idx.max(idx);
                }
            }
        }

        self.snapshot_counter = max_idx;
        self.snapshot_counter_seeded = true;
        Ok(())
    }

    #[allow(clippy::unused_self)]
    #[allow(clippy::too_many_lines)]
    fn recover_snapshot_artifacts(&self, root: &Path) -> Result<()> {
        #[derive(Default)]
        struct SnapshotArtifacts {
            live: Option<PathBuf>,
            backup: Option<PathBuf>,
            staging: Option<PathBuf>,
        }

        let mut artifacts: BTreeMap<u64, SnapshotArtifacts> = BTreeMap::new();

        for entry in fs::read_dir(root).wrap_err_with(|| {
            format!(
                "failed to read cold tier root {path}",
                path = root.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name();
            if let Some(idx) = Self::parse_snapshot_dir_name(&name) {
                artifacts.entry(idx).or_default().live = Some(path);
                continue;
            }
            let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if ext != "bak" && ext != "staging" {
                continue;
            }
            let Some(stem) = path.file_stem() else {
                continue;
            };
            let Some(idx) = Self::parse_snapshot_dir_name(stem) else {
                continue;
            };
            let entry = artifacts.entry(idx).or_default();
            if ext == "bak" {
                entry.backup = Some(path);
            } else {
                entry.staging = Some(path);
            }
        }

        let mut touched = false;
        for (idx, entry) in artifacts {
            let live_path = root.join(format!("{idx:020}"));
            if entry.live.is_some() {
                if let Some(backup) = entry.backup {
                    if let Err(err) = fs::remove_dir_all(&backup) {
                        iroha_logger::warn!(
                            ?err,
                            path = %backup.display(),
                            "tiered-state: failed to remove stale snapshot backup"
                        );
                    } else {
                        touched = true;
                    }
                }
                if let Some(staging) = entry.staging {
                    if let Err(err) = fs::remove_dir_all(&staging) {
                        iroha_logger::warn!(
                            ?err,
                            path = %staging.display(),
                            "tiered-state: failed to remove stale snapshot staging directory"
                        );
                    } else {
                        touched = true;
                    }
                }
                continue;
            }

            if let Some(backup) = entry.backup {
                match fs::rename(&backup, &live_path) {
                    Ok(()) => {
                        iroha_logger::info!(
                            from = %backup.display(),
                            to = %live_path.display(),
                            "tiered-state: restored snapshot backup"
                        );
                        touched = true;
                    }
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            from = %backup.display(),
                            to = %live_path.display(),
                            "tiered-state: failed to restore snapshot backup"
                        );
                    }
                }
            }

            if let Some(staging) = entry.staging {
                if let Err(err) = fs::remove_dir_all(&staging) {
                    iroha_logger::warn!(
                        ?err,
                        path = %staging.display(),
                        "tiered-state: failed to remove stale snapshot staging directory"
                    );
                } else {
                    touched = true;
                }
            }
        }

        if touched {
            if let Err(err) = Self::sync_dir(root) {
                iroha_logger::warn!(
                    ?err,
                    path = %root.display(),
                    "tiered-state: failed to sync snapshot root after recovery"
                );
            }
        }

        Ok(())
    }

    fn plan_world_snapshot(&mut self, world: &World) -> Result<Option<TieredSnapshotPlan>> {
        let Some((root, snapshot_idx, snapshot_dir)) = self.prepare_snapshot()? else {
            return Ok(None);
        };

        let mut scores = Vec::new();
        let mut seen = BTreeSet::new();

        self.collect_world_entries(world, snapshot_idx, &mut scores, &mut seen)?;
        self.entries.retain(|id, _| seen.contains(id));
        self.entry_keys.retain(|id, _| seen.contains(id));

        let plan = self.build_snapshot_plan(root, snapshot_idx, snapshot_dir, scores, None)?;
        Ok(Some(plan))
    }

    fn plan_world_snapshot_with_diff(
        &mut self,
        world: &World,
        diff: &TieredSnapshotDiff,
    ) -> Result<Option<TieredSnapshotPlan>> {
        let Some((root, snapshot_idx, snapshot_dir)) = self.prepare_snapshot()? else {
            return Ok(None);
        };

        if !diff.entries.is_empty() {
            self.apply_snapshot_diff(world, snapshot_idx, diff)?;
        }

        let scores = self.build_scores_from_keys(snapshot_idx);
        let plan = self.build_snapshot_plan(root, snapshot_idx, snapshot_dir, scores, None)?;
        Ok(Some(plan))
    }

    fn prepare_snapshot(&mut self) -> Result<Option<(PathBuf, u64, PathBuf)>> {
        if !self.enabled {
            return Ok(None);
        }

        if self.hot_retained_keys == 0
            && self.hot_retained_bytes == 0
            && self.primary_cold_root().is_none()
        {
            return Ok(None);
        }

        let Some(root) = self.primary_cold_root().cloned() else {
            if self.hot_retained_keys > 0 || self.hot_retained_bytes > 0 {
                iroha_logger::warn!(
                    "tiered-state: hot tier limit set but cold_store_root/da_store_root missing; skipping snapshot"
                );
            }
            return Ok(None);
        };

        self.seed_snapshot_counter_if_needed()?;
        self.snapshot_counter = self.snapshot_counter.saturating_add(1);
        let snapshot_idx = self.snapshot_counter;
        let snapshot_dir = root.join(format!("{snapshot_idx:020}"));
        Ok(Some((root, snapshot_idx, snapshot_dir)))
    }

    fn build_scores_from_keys(&mut self, snapshot_idx: u64) -> Vec<EntryScore> {
        let mut scores = Vec::with_capacity(self.entry_keys.len());
        for (id, entry_key) in &self.entry_keys {
            let meta = self.entries.get_mut(id).expect("metadata populated");
            meta.last_present_snapshot = snapshot_idx;
            scores.push(entry_key.score(*id));
        }
        scores
    }

    #[allow(clippy::too_many_lines)]
    fn build_snapshot_plan(
        &mut self,
        root: PathBuf,
        snapshot_idx: u64,
        snapshot_dir: PathBuf,
        mut scores: Vec<EntryScore>,
        payloads: Option<&BTreeMap<TieredEntryId, Vec<u8>>>,
    ) -> Result<TieredSnapshotPlan> {
        if scores.is_empty() {
            let manifest = TieredSnapshotManifest {
                snapshot_index: snapshot_idx,
                total_entries: 0,
                hot_entries: Vec::new(),
                cold_entries: Vec::new(),
                cold_bytes_total: 0,
                cold_reused_entries: 0,
                cold_reused_bytes: 0,
                hot_promotions: 0,
                hot_demotions: 0,
                hot_grace_overflow_keys: 0,
                hot_grace_overflow_bytes: 0,
            };
            return Ok(TieredSnapshotPlan {
                root,
                snapshot_dir,
                manifest,
                cold_entries: Vec::new(),
            });
        }

        scores.sort_by(|a, b| {
            let meta_a = self.entries.get(&a.id).expect("metadata populated");
            let meta_b = self.entries.get(&b.id).expect("metadata populated");
            meta_b.cmp(meta_a).then_with(|| a.id.cmp(&b.id))
        });

        let max_keys = if self.hot_retained_keys == 0 {
            usize::MAX
        } else {
            self.hot_retained_keys
        };
        let max_bytes = self.hot_retained_bytes;

        #[allow(clippy::too_many_arguments)]
        fn try_select_entry(
            entry: &EntryScore,
            entries: &BTreeMap<TieredEntryId, EntryMetadata>,
            hot_ids: &mut BTreeSet<TieredEntryId>,
            hot_list: &mut Vec<TieredEntryId>,
            retained_bytes: &mut u64,
            enforce_budget: bool,
            max_keys: usize,
            max_bytes: u64,
        ) {
            let meta = entries.get(&entry.id).expect("metadata populated");
            let entry_bytes = meta.value_size_bytes as u64;
            if enforce_budget {
                if hot_ids.len() >= max_keys {
                    return;
                }
                if max_bytes != 0 && retained_bytes.saturating_add(entry_bytes) > max_bytes {
                    return;
                }
            }
            if hot_ids.insert(entry.id) {
                *retained_bytes = retained_bytes.saturating_add(entry_bytes);
                hot_list.push(entry.id);
            }
        }

        let mut hot_ids = BTreeSet::new();
        let mut hot_list = Vec::new();
        let mut retained_bytes = 0u64;

        if self.hot_retained_grace_snapshots > 0 {
            for entry in &scores {
                let meta = self.entries.get(&entry.id).expect("metadata populated");
                if meta.hot_until_snapshot >= snapshot_idx && meta.hot_until_snapshot > 0 {
                    try_select_entry(
                        entry,
                        &self.entries,
                        &mut hot_ids,
                        &mut hot_list,
                        &mut retained_bytes,
                        false,
                        max_keys,
                        max_bytes,
                    );
                }
            }
        }

        for entry in &scores {
            if !hot_ids.contains(&entry.id) {
                try_select_entry(
                    entry,
                    &self.entries,
                    &mut hot_ids,
                    &mut hot_list,
                    &mut retained_bytes,
                    true,
                    max_keys,
                    max_bytes,
                );
            }
        }

        let mut hot_manifest_entries = Vec::with_capacity(hot_list.len());
        let mut cold_manifest_entries = Vec::with_capacity(scores.len());
        let mut cold_plans = Vec::with_capacity(scores.len());

        for entry in &scores {
            let meta = self.entries.get(&entry.id).expect("metadata populated");
            if hot_ids.contains(&entry.id) {
                hot_manifest_entries.push(entry.manifest_entry(meta, None));
            } else {
                let rel_path = entry.relative_payload_path(snapshot_idx);
                let reuse_source = if meta.last_cold_snapshot > 0
                    && meta.last_cold_snapshot >= meta.last_mutated_snapshot
                {
                    meta.last_cold_rel_path.as_ref().and_then(|rel_path| {
                        let candidate = root
                            .join(format!("{index:020}", index = meta.last_cold_snapshot))
                            .join(rel_path);
                        candidate.exists().then_some(candidate)
                    })
                } else {
                    None
                };
                if let Some(payloads) = payloads
                    && reuse_source.is_none()
                    && !payloads.contains_key(&entry.id)
                {
                    if hot_ids.insert(entry.id) {
                        retained_bytes = retained_bytes.saturating_add(
                            u64::try_from(meta.value_size_bytes).unwrap_or(u64::MAX),
                        );
                        hot_list.push(entry.id);
                    }
                    hot_manifest_entries.push(entry.manifest_entry(meta, None));
                    continue;
                }
                let manifest_index = cold_manifest_entries.len();
                let mut manifest_entry = entry.manifest_entry(meta, Some((rel_path.clone(), 0)));
                manifest_entry.spill_bytes = None;
                cold_manifest_entries.push(manifest_entry);
                cold_plans.push(ColdEntryPlan {
                    rel_path,
                    entry: entry.clone(),
                    manifest_index,
                    reuse_source,
                });
            }
        }

        let mut hot_promotions = 0usize;
        let mut hot_demotions = 0usize;
        for entry in &scores {
            let meta = self.entries.get(&entry.id).expect("metadata populated");
            let was_hot_last = meta.last_hot_snapshot > 0
                && meta.last_hot_snapshot == snapshot_idx.saturating_sub(1);
            let is_hot_now = hot_ids.contains(&entry.id);
            if was_hot_last && !is_hot_now {
                hot_demotions = hot_demotions.saturating_add(1);
            } else if !was_hot_last && is_hot_now {
                hot_promotions = hot_promotions.saturating_add(1);
            }
        }

        for id in &hot_list {
            if let Some(meta) = self.entries.get_mut(id) {
                let was_hot_last = meta.last_hot_snapshot > 0
                    && meta.last_hot_snapshot == snapshot_idx.saturating_sub(1);
                if self.hot_retained_grace_snapshots > 0 && !was_hot_last {
                    meta.hot_until_snapshot =
                        snapshot_idx.saturating_add(self.hot_retained_grace_snapshots);
                }
                meta.last_hot_snapshot = snapshot_idx;
            }
        }

        let hot_grace_overflow_keys = if self.hot_retained_keys == 0 {
            0
        } else {
            hot_ids.len().saturating_sub(self.hot_retained_keys)
        };
        let hot_grace_overflow_bytes = if self.hot_retained_bytes == 0 {
            0
        } else {
            retained_bytes.saturating_sub(self.hot_retained_bytes)
        };
        if (self.hot_retained_keys > 0 && hot_grace_overflow_keys > 0)
            || (self.hot_retained_bytes > 0 && hot_grace_overflow_bytes > 0)
        {
            iroha_logger::warn!(
                hot_limit_keys = self.hot_retained_keys,
                hot_limit_bytes = self.hot_retained_bytes,
                hot_entries = hot_ids.len(),
                hot_bytes = retained_bytes,
                hot_grace_overflow_keys,
                hot_grace_overflow_bytes,
                "tiered-state: hot tier budget exceeded due to grace retention"
            );
        }

        let manifest = TieredSnapshotManifest {
            snapshot_index: snapshot_idx,
            total_entries: scores.len(),
            hot_entries: hot_manifest_entries,
            cold_entries: cold_manifest_entries,
            cold_bytes_total: 0,
            cold_reused_entries: 0,
            cold_reused_bytes: 0,
            hot_promotions,
            hot_demotions,
            hot_grace_overflow_keys,
            hot_grace_overflow_bytes,
        };

        Ok(TieredSnapshotPlan {
            root,
            snapshot_dir,
            manifest,
            cold_entries: cold_plans,
        })
    }

    fn apply_snapshot_diff(
        &mut self,
        world: &World,
        snapshot_idx: u64,
        diff: &TieredSnapshotDiff,
    ) -> Result<()> {
        for entry in &diff.entries {
            let (id, key_encoded) = entry.entry_id()?;
            let Some((value_hash, value_size_bytes)) = entry.measure_value(world)? else {
                self.entries.remove(&id);
                self.entry_keys.remove(&id);
                continue;
            };

            let meta = self
                .entries
                .entry(id)
                .or_insert_with(|| EntryMetadata::new(snapshot_idx, value_hash, value_size_bytes));
            if meta.last_value_hash != value_hash {
                meta.last_value_hash = value_hash;
                meta.last_mutated_snapshot = snapshot_idx;
            }
            meta.value_size_bytes = value_size_bytes;
            self.entry_keys.insert(
                id,
                EntryKey {
                    key: entry.clone(),
                    key_encoded,
                },
            );
        }
        Ok(())
    }

    fn apply_snapshot_payload(
        &mut self,
        snapshot_idx: u64,
        payload: &TieredSnapshotPayload,
    ) -> Result<SnapshotPayloadCache> {
        let mut cache = SnapshotPayloadCache::default();
        for entry in &payload.entries {
            let (id, key_encoded) = entry.key.entry_id()?;
            let Some(value) = entry.value.as_ref() else {
                self.entries.remove(&id);
                self.entry_keys.remove(&id);
                continue;
            };

            let payload = value.encode_json()?;
            let value_hash = sha256(&payload);
            let value_size_bytes = value.measured_bytes();

            let meta = self
                .entries
                .entry(id)
                .or_insert_with(|| EntryMetadata::new(snapshot_idx, value_hash, value_size_bytes));
            if meta.last_value_hash != value_hash {
                meta.last_value_hash = value_hash;
                meta.last_mutated_snapshot = snapshot_idx;
            }
            meta.value_size_bytes = value_size_bytes;
            self.entry_keys.insert(
                id,
                EntryKey {
                    key: entry.key.clone(),
                    key_encoded,
                },
            );
            cache.payloads.insert(id, payload);
        }
        Ok(cache)
    }

    #[allow(clippy::too_many_lines)]
    fn execute_snapshot_plan(&mut self, mut plan: TieredSnapshotPlan, world: &World) -> Result<()> {
        self.ensure_cold_roots()
            .wrap_err("failed to prepare cold tier root directory")?;

        let staging_dir = plan.snapshot_dir.with_extension("staging");
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).wrap_err_with(|| {
                format!(
                    "failed to clear previous staging directory {path}",
                    path = staging_dir.display()
                )
            })?;
        }
        fs::create_dir_all(&staging_dir).wrap_err_with(|| {
            format!(
                "failed to create staging directory {path}",
                path = staging_dir.display()
            )
        })?;

        let mut cold_bytes_total: u64 = 0;
        let mut cold_reused_entries: usize = 0;
        let mut cold_reused_bytes: u64 = 0;
        let mut dirs_to_sync = BTreeSet::new();
        for cold in &plan.cold_entries {
            let abs_path = staging_dir.join(&cold.rel_path);
            let mut parent_dirs = Vec::new();
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to create cold shard directory {dir}",
                        dir = parent.display()
                    )
                })?;
                for ancestor in parent.ancestors() {
                    parent_dirs.push(ancestor.to_path_buf());
                    if ancestor == staging_dir {
                        break;
                    }
                }
            }
            let mut payload_len = None;
            let mut reused = false;
            if let Some(source) = cold.reuse_source.as_ref() {
                match Self::try_reuse_cold_payload(source, &abs_path) {
                    Ok(Some(bytes)) => {
                        payload_len = Some(bytes);
                        reused = true;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            source = %source.display(),
                            target = %abs_path.display(),
                            "tiered-state: failed to reuse cold shard payload"
                        );
                    }
                }
            }

            let payload_len = if let Some(bytes) = payload_len {
                bytes
            } else {
                let payload = cold.entry.encode_value(world).with_context(|| {
                    format!(
                        "failed to encode value for cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                let mut file = BufWriter::new(fs::File::create(&abs_path).wrap_err_with(|| {
                    format!(
                        "failed to open cold shard {path} for writing",
                        path = abs_path.display()
                    )
                })?);
                file.write_all(&payload).wrap_err_with(|| {
                    format!(
                        "failed to persist cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                file.flush().wrap_err_with(|| {
                    format!(
                        "failed to flush cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                file.get_ref().sync_all().wrap_err_with(|| {
                    format!(
                        "failed to sync cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                payload.len() as u64
            };
            cold_bytes_total = cold_bytes_total.saturating_add(payload_len);
            if reused {
                cold_reused_entries = cold_reused_entries.saturating_add(1);
                cold_reused_bytes = cold_reused_bytes.saturating_add(payload_len);
            }
            if let Some(entry) = plan.manifest.cold_entries.get_mut(cold.manifest_index) {
                entry.spill_bytes = Some(payload_len);
            }
            if let Some(meta) = self.entries.get_mut(&cold.entry.id) {
                meta.last_cold_snapshot = plan.manifest.snapshot_index;
                meta.last_cold_rel_path = Some(cold.rel_path.clone());
            }
            for dir in parent_dirs {
                dirs_to_sync.insert(dir);
            }
        }

        plan.manifest.cold_bytes_total = cold_bytes_total;
        plan.manifest.cold_reused_entries = cold_reused_entries;
        plan.manifest.cold_reused_bytes = cold_reused_bytes;

        for dir in dirs_to_sync {
            Self::sync_dir(&dir).wrap_err_with(|| {
                format!(
                    "failed to sync cold shard directory {path}",
                    path = dir.display()
                )
            })?;
        }

        Self::write_manifest(&staging_dir, &plan.manifest)?;
        Self::sync_dir(&staging_dir).wrap_err_with(|| {
            format!(
                "failed to sync staging directory {path}",
                path = staging_dir.display()
            )
        })?;

        let backup = if plan.snapshot_dir.exists() {
            let backup_path = plan.snapshot_dir.with_extension("bak");
            if backup_path.exists() {
                fs::remove_dir_all(&backup_path).wrap_err_with(|| {
                    format!(
                        "failed to clear previous backup {path}",
                        path = backup_path.display()
                    )
                })?;
            }
            fs::rename(&plan.snapshot_dir, &backup_path).wrap_err_with(|| {
                format!(
                    "failed to move existing snapshot to backup {path}",
                    path = backup_path.display()
                )
            })?;
            Some(backup_path)
        } else {
            None
        };

        fs::rename(&staging_dir, &plan.snapshot_dir).wrap_err_with(|| {
            format!(
                "failed to promote staging snapshot into place at {path}",
                path = plan.snapshot_dir.display()
            )
        })?;

        Self::sync_dir(&plan.snapshot_dir).wrap_err_with(|| {
            format!(
                "failed to sync snapshot directory {path}",
                path = plan.snapshot_dir.display()
            )
        })?;
        if let Some(parent) = plan.snapshot_dir.parent() {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync snapshot parent {path}",
                    path = parent.display()
                )
            })?;
        }
        if let Some(backup_path) = backup {
            if let Err(err) = fs::remove_dir_all(&backup_path) {
                iroha_logger::warn!(
                    ?err,
                    path = %backup_path.display(),
                    "tiered-state: failed to remove snapshot backup directory"
                );
            }
        }

        self.last_manifest = Some(plan.manifest);
        self.prune_old_snapshots(&plan.root)?;
        self.prune_to_cold_bytes(&plan.root)?;

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn execute_snapshot_plan_with_payload(
        &mut self,
        mut plan: TieredSnapshotPlan,
        payloads: &BTreeMap<TieredEntryId, Vec<u8>>,
    ) -> Result<()> {
        self.ensure_cold_roots()
            .wrap_err("failed to prepare cold tier root directory")?;

        let staging_dir = plan.snapshot_dir.with_extension("staging");
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir).wrap_err_with(|| {
                format!(
                    "failed to clear previous staging directory {path}",
                    path = staging_dir.display()
                )
            })?;
        }
        fs::create_dir_all(&staging_dir).wrap_err_with(|| {
            format!(
                "failed to create staging directory {path}",
                path = staging_dir.display()
            )
        })?;

        let mut cold_bytes_total: u64 = 0;
        let mut cold_reused_entries: usize = 0;
        let mut cold_reused_bytes: u64 = 0;
        let mut dirs_to_sync = BTreeSet::new();
        for cold in &plan.cold_entries {
            let abs_path = staging_dir.join(&cold.rel_path);
            let mut parent_dirs = Vec::new();
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to create cold shard directory {dir}",
                        dir = parent.display()
                    )
                })?;
                for ancestor in parent.ancestors() {
                    parent_dirs.push(ancestor.to_path_buf());
                    if ancestor == staging_dir {
                        break;
                    }
                }
            }
            let mut payload_len = None;
            let mut reused = false;
            if let Some(source) = cold.reuse_source.as_ref() {
                match Self::try_reuse_cold_payload(source, &abs_path) {
                    Ok(Some(bytes)) => {
                        payload_len = Some(bytes);
                        reused = true;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        iroha_logger::warn!(
                            ?err,
                            source = %source.display(),
                            target = %abs_path.display(),
                            "tiered-state: failed to reuse cold shard payload"
                        );
                    }
                }
            }

            let payload_len = if let Some(bytes) = payload_len {
                bytes
            } else {
                let payload = payloads.get(&cold.entry.id).ok_or_else(|| {
                    eyre::eyre!("tiered-state: missing payload for {}", cold.entry.key)
                })?;
                let mut file = BufWriter::new(fs::File::create(&abs_path).wrap_err_with(|| {
                    format!(
                        "failed to open cold shard {path} for writing",
                        path = abs_path.display()
                    )
                })?);
                file.write_all(payload).wrap_err_with(|| {
                    format!(
                        "failed to persist cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                file.flush().wrap_err_with(|| {
                    format!(
                        "failed to flush cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                file.get_ref().sync_all().wrap_err_with(|| {
                    format!(
                        "failed to sync cold shard {path}",
                        path = abs_path.display()
                    )
                })?;
                payload.len() as u64
            };
            cold_bytes_total = cold_bytes_total.saturating_add(payload_len);
            if reused {
                cold_reused_entries = cold_reused_entries.saturating_add(1);
                cold_reused_bytes = cold_reused_bytes.saturating_add(payload_len);
            }
            if let Some(entry) = plan.manifest.cold_entries.get_mut(cold.manifest_index) {
                entry.spill_bytes = Some(payload_len);
            }
            if let Some(meta) = self.entries.get_mut(&cold.entry.id) {
                meta.last_cold_snapshot = plan.manifest.snapshot_index;
                meta.last_cold_rel_path = Some(cold.rel_path.clone());
            }
            for dir in parent_dirs {
                dirs_to_sync.insert(dir);
            }
        }

        plan.manifest.cold_bytes_total = cold_bytes_total;
        plan.manifest.cold_reused_entries = cold_reused_entries;
        plan.manifest.cold_reused_bytes = cold_reused_bytes;

        for dir in dirs_to_sync {
            Self::sync_dir(&dir).wrap_err_with(|| {
                format!(
                    "failed to sync cold shard directory {path}",
                    path = dir.display()
                )
            })?;
        }

        Self::write_manifest(&staging_dir, &plan.manifest)?;
        Self::sync_dir(&staging_dir).wrap_err_with(|| {
            format!(
                "failed to sync staging directory {path}",
                path = staging_dir.display()
            )
        })?;

        let backup = if plan.snapshot_dir.exists() {
            let backup_path = plan.snapshot_dir.with_extension("bak");
            if backup_path.exists() {
                fs::remove_dir_all(&backup_path).wrap_err_with(|| {
                    format!(
                        "failed to clear previous backup {path}",
                        path = backup_path.display()
                    )
                })?;
            }
            fs::rename(&plan.snapshot_dir, &backup_path).wrap_err_with(|| {
                format!(
                    "failed to move existing snapshot to backup {path}",
                    path = backup_path.display()
                )
            })?;
            Some(backup_path)
        } else {
            None
        };

        fs::rename(&staging_dir, &plan.snapshot_dir).wrap_err_with(|| {
            format!(
                "failed to promote staging snapshot into place at {path}",
                path = plan.snapshot_dir.display()
            )
        })?;

        Self::sync_dir(&plan.snapshot_dir).wrap_err_with(|| {
            format!(
                "failed to sync snapshot directory {path}",
                path = plan.snapshot_dir.display()
            )
        })?;
        if let Some(parent) = plan.snapshot_dir.parent() {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync snapshot parent {path}",
                    path = parent.display()
                )
            })?;
        }
        if let Some(backup_path) = backup {
            if let Err(err) = fs::remove_dir_all(&backup_path) {
                iroha_logger::warn!(
                    ?err,
                    path = %backup_path.display(),
                    "tiered-state: failed to remove snapshot backup directory"
                );
            }
        }

        self.last_manifest = Some(plan.manifest);
        self.prune_old_snapshots(&plan.root)?;
        self.prune_to_cold_bytes(&plan.root)?;

        Ok(())
    }

    /// Returns the currently configured hot key retention limit.
    #[must_use]
    pub fn hot_retained_keys(&self) -> usize {
        self.hot_retained_keys
    }

    /// Returns the currently configured hot byte retention limit.
    #[must_use]
    pub fn hot_retained_bytes(&self) -> u64 {
        self.hot_retained_bytes
    }

    /// Returns the currently configured cold snapshot byte budget.
    #[must_use]
    pub fn max_cold_bytes(&self) -> u64 {
        self.max_cold_bytes
    }

    fn cold_roots(&self) -> Vec<&PathBuf> {
        let mut roots = Vec::new();
        if let Some(root) = self.cold_store_root.as_ref() {
            roots.push(root);
        }
        if let Some(root) = self.da_store_root.as_ref()
            && !roots.contains(&root)
        {
            roots.push(root);
        }
        roots
    }

    fn da_store_root_for_offload(&self) -> Option<&PathBuf> {
        match (&self.cold_store_root, &self.da_store_root) {
            (Some(cold_root), Some(da_root)) if cold_root != da_root => Some(da_root),
            _ => None,
        }
    }

    fn primary_cold_root(&self) -> Option<&PathBuf> {
        self.cold_store_root
            .as_ref()
            .or(self.da_store_root.as_ref())
    }

    /// Returns true when tiering is enabled and a cold tier is configured.
    #[must_use]
    pub fn is_cold_tier_enabled(&self) -> bool {
        self.enabled && self.primary_cold_root().is_some()
    }

    /// Returns whether tiering is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the backend has been seeded with at least one entry.
    #[must_use]
    pub(crate) fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Returns the cached manifest of the latest snapshot, if any.
    #[must_use]
    pub fn last_manifest(&self) -> Option<&TieredSnapshotManifest> {
        self.last_manifest.as_ref()
    }

    /// Return the total cold-tier bytes currently stored on disk.
    pub fn cold_store_bytes(&self) -> Result<Option<u64>> {
        let Some(root) = self.primary_cold_root() else {
            return Ok(None);
        };
        if !root.exists() {
            return Ok(Some(0));
        }
        let mut total = 0u64;
        for entry in fs::read_dir(root).wrap_err_with(|| {
            format!(
                "failed to read tiered snapshot root {path}",
                path = root.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            if Self::parse_snapshot_dir_name(&entry.file_name()).is_some() {
                total = total.saturating_add(Self::snapshot_dir_size(&entry.path())?);
            }
        }
        Ok(Some(total))
    }

    /// Load a cold payload from the configured cold roots.
    pub fn read_cold_payload(
        &self,
        snapshot_index: u64,
        entry: &TieredManifestEntry,
    ) -> Result<Option<Vec<u8>>> {
        let Some(rel_path) = entry.spill_path.as_ref() else {
            return Ok(None);
        };
        let cache_enabled = self.da_store_root_for_offload().is_some();
        let cold_root = self.cold_store_root.as_ref();
        if let Some(root) = cold_root {
            let path = root.join(format!("{snapshot_index:020}")).join(rel_path);
            match fs::read(&path) {
                Ok(bytes) => {
                    if cache_enabled {
                        self.record_da_cache(DA_CACHE_HIT);
                    }
                    return Ok(Some(bytes));
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).wrap_err_with(|| {
                        format!("failed to read cold payload {path}", path = path.display())
                    });
                }
            }
        }

        let da_root = self.da_store_root.as_ref();
        let same_root = match (cold_root, da_root) {
            (Some(cold), Some(da)) => cold == da,
            _ => false,
        };
        if let Some(root) = da_root
            && !same_root
        {
            let path = root.join(format!("{snapshot_index:020}")).join(rel_path);
            match fs::read(&path) {
                Ok(bytes) => {
                    if cache_enabled {
                        self.record_da_cache(DA_CACHE_MISS);
                        match self.try_rehydrate_cold_payload(snapshot_index, rel_path, &bytes) {
                            Ok(Some(written)) => {
                                self.record_da_churn(DA_CHURN_REHYDRATED, written);
                            }
                            Ok(None) => {}
                            Err(err) => {
                                iroha_logger::warn!(
                                    ?err,
                                    path = %path.display(),
                                    "tiered-state: failed to rehydrate cold payload"
                                );
                            }
                        }
                    }
                    return Ok(Some(bytes));
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).wrap_err_with(|| {
                        format!("failed to read cold payload {path}", path = path.display())
                    });
                }
            }
        }
        Ok(None)
    }

    fn try_rehydrate_cold_payload(
        &self,
        snapshot_index: u64,
        rel_path: &Path,
        payload: &[u8],
    ) -> Result<Option<u64>> {
        let Some(cold_root) = self.cold_store_root.as_ref() else {
            return Ok(None);
        };
        if let Some(da_root) = self.da_store_root.as_ref()
            && da_root == cold_root
        {
            return Ok(None);
        }
        let snapshot_dir = cold_root.join(format!("{snapshot_index:020}"));
        let dest = snapshot_dir.join(rel_path);
        if dest.exists() {
            return Ok(None);
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create cold shard directory {dir}",
                    dir = parent.display()
                )
            })?;
        }
        let tmp_path = dest.with_extension("rehydrate.tmp");
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => return Ok(None),
            Err(err) => {
                return Err(err).wrap_err_with(|| {
                    format!(
                        "failed to open temp cold shard {path} for rehydration",
                        path = tmp_path.display()
                    )
                });
            }
        };
        file.write_all(payload).wrap_err_with(|| {
            format!(
                "failed to write cold shard {path} during rehydration",
                path = tmp_path.display()
            )
        })?;
        file.flush().wrap_err_with(|| {
            format!(
                "failed to flush cold shard {path} during rehydration",
                path = tmp_path.display()
            )
        })?;
        file.sync_all().wrap_err_with(|| {
            format!(
                "failed to sync cold shard {path} during rehydration",
                path = tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, &dest).wrap_err_with(|| {
            format!(
                "failed to persist cold shard {path} during rehydration",
                path = dest.display()
            )
        })?;
        if let Some(parent) = dest.parent() {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync cold shard directory {path}",
                    path = parent.display()
                )
            })?;
        }
        Ok(Some(u64::try_from(payload.len()).unwrap_or(u64::MAX)))
    }

    fn record_da_cache(&self, outcome: &'static str) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.inc_storage_da_cache(WSV_COLD_COMPONENT, outcome);
        }
    }

    fn record_da_churn(&self, event: &'static str, bytes: u64) {
        if bytes == 0 {
            return;
        }
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.add_storage_da_churn_bytes(WSV_COLD_COMPONENT, event, bytes);
        }
    }

    /// Update configuration knobs at runtime.
    #[allow(clippy::too_many_arguments)]
    pub fn reconfigure(
        &mut self,
        enabled: bool,
        hot_retained_keys: usize,
        hot_retained_bytes: u64,
        hot_retained_grace_snapshots: u64,
        cold_store_root: Option<PathBuf>,
        da_store_root: Option<PathBuf>,
        max_snapshots: usize,
        max_cold_bytes: u64,
    ) {
        let cold_root_changed =
            self.cold_store_root != cold_store_root || self.da_store_root != da_store_root;
        let grace_changed = self.hot_retained_grace_snapshots != hot_retained_grace_snapshots;
        self.enabled = enabled;
        self.hot_retained_keys = hot_retained_keys;
        self.hot_retained_bytes = hot_retained_bytes;
        self.hot_retained_grace_snapshots = hot_retained_grace_snapshots;
        self.cold_store_root = cold_store_root;
        self.da_store_root = da_store_root;
        self.max_snapshots = max_snapshots;
        self.max_cold_bytes = max_cold_bytes;
        if grace_changed {
            for meta in self.entries.values_mut() {
                meta.hot_until_snapshot = 0;
            }
        }
        if cold_root_changed {
            self.entries.clear();
            self.entry_keys.clear();
            self.snapshot_counter = 0;
            self.snapshot_counter_seeded = false;
            self.last_manifest = None;
        }
        if !self.enabled {
            return;
        }
        if cold_root_changed {
            if let Err(err) = self.ensure_cold_roots() {
                iroha_logger::warn!(
                    ?err,
                    "tiered-state: failed to prepare cold tier root after reconfigure"
                );
            }
        }
    }

    /// Ensure tiered snapshot directories reflect the configured lane geometry.
    pub fn reconcile_lane_geometry(
        &mut self,
        previous: &LaneConfig,
        current: &LaneConfig,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let Some(root) = self.primary_cold_root().cloned() else {
            return Ok(());
        };
        self.ensure_cold_roots()?;

        let mut previous_map = BTreeMap::new();
        for entry in previous.entries() {
            previous_map.insert(entry.lane_id, entry);
        }
        let mut current_map = BTreeMap::new();
        for entry in current.entries() {
            current_map.insert(entry.lane_id, entry);
        }

        let added: Vec<&LaneConfigEntry> = current_map
            .iter()
            .filter(|(id, _)| !previous_map.contains_key(id))
            .map(|(_, entry)| *entry)
            .collect();
        let retired: Vec<&LaneConfigEntry> = previous_map
            .iter()
            .filter(|(id, _)| !current_map.contains_key(id))
            .map(|(_, entry)| *entry)
            .collect();

        let lanes_root = root.join("lanes");
        fs::create_dir_all(&lanes_root).wrap_err_with(|| {
            format!(
                "failed to create tiered lanes root {path}",
                path = lanes_root.display()
            )
        })?;

        for entry in added {
            self.ensure_lane_snapshot_dir(&lanes_root, entry)?;
        }

        for entry in current.entries() {
            let dir = lane_snapshot_dir(&lanes_root, entry);
            if dir.exists() {
                continue;
            }
            let has_prev_lane_dir = previous_map
                .get(&entry.lane_id)
                .is_some_and(|prev| lane_snapshot_dir(&lanes_root, prev).exists());
            if has_prev_lane_dir {
                continue;
            }
            self.ensure_lane_snapshot_dir(&lanes_root, entry)?;
        }

        for entry in retired {
            self.retire_lane_snapshot_dir(&root, &lanes_root, entry)?;
        }

        Ok(())
    }

    /// Relabel snapshot directories when lane aliases (and therefore slugs) change.
    #[allow(clippy::too_many_lines)]
    pub fn relabel_lane_geometry(
        &mut self,
        migrations: &[(&LaneConfigEntry, &LaneConfigEntry)],
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let Some(root) = self.primary_cold_root().cloned() else {
            return Ok(());
        };
        if migrations.is_empty() {
            return Ok(());
        }
        let lanes_root = root.join("lanes");
        fs::create_dir_all(&lanes_root).wrap_err_with(|| {
            format!(
                "failed to create tiered lanes root {path}",
                path = lanes_root.display()
            )
        })?;

        for (previous, current) in migrations {
            let old_dir = lane_snapshot_dir(&lanes_root, previous);
            let new_dir = lane_snapshot_dir(&lanes_root, current);
            if old_dir == new_dir || !old_dir.exists() {
                continue;
            }
            if let Some(parent) = new_dir.parent() {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to prepare parent {path} for lane snapshot relabel",
                        path = parent.display()
                    )
                })?;
            }
            if new_dir.exists() {
                let retired_root = root.join("retired").join("lanes");
                fs::create_dir_all(&retired_root).wrap_err_with(|| {
                    format!(
                        "failed to prepare retired lane root {path}",
                        path = retired_root.display()
                    )
                })?;
                let archive = unique_retired_lane_path(&retired_root, &current.kura_segment);
                fs::rename(&new_dir, &archive).wrap_err_with(|| {
                    format!(
                        "failed to archive conflicting lane snapshot dir {path}",
                        path = new_dir.display()
                    )
                })?;
                let archive_parent = archive.parent();
                let new_parent = new_dir.parent();
                if let Some(parent) = archive_parent {
                    Self::sync_dir(parent).wrap_err_with(|| {
                        format!(
                            "failed to sync retired lane snapshot dir {path}",
                            path = parent.display()
                        )
                    })?;
                }
                if let Some(parent) = new_parent {
                    if Some(parent) != archive_parent {
                        Self::sync_dir(parent).wrap_err_with(|| {
                            format!(
                                "failed to sync lane snapshot directory {path}",
                                path = parent.display()
                            )
                        })?;
                    }
                }
            }
            fs::rename(&old_dir, &new_dir).wrap_err_with(|| {
                format!(
                    "failed to relabel lane snapshot dir from {src} to {dst}",
                    src = old_dir.display(),
                    dst = new_dir.display()
                )
            })?;
            let new_parent = new_dir.parent();
            let old_parent = old_dir.parent();
            if let Some(parent) = new_parent {
                Self::sync_dir(parent).wrap_err_with(|| {
                    format!(
                        "failed to sync lane snapshot directory {path}",
                        path = parent.display()
                    )
                })?;
            }
            if let Some(parent) = old_parent {
                if Some(parent) != new_parent {
                    Self::sync_dir(parent).wrap_err_with(|| {
                        format!(
                            "failed to sync lane snapshot directory {path}",
                            path = parent.display()
                        )
                    })?;
                }
            }
            iroha_logger::info!(
                lane = %current.lane_id.as_u32(),
                alias_before = previous.alias,
                alias_after = current.alias,
                dir = %new_dir.display(),
                "tiered-state: lane snapshot directory relabelled"
            );
        }

        Ok(())
    }

    fn ensure_cold_roots(&self) -> Result<()> {
        for root in self.cold_store_root.iter().chain(self.da_store_root.iter()) {
            fs::create_dir_all(root).wrap_err_with(|| {
                format!(
                    "failed to create cold tier root {path}",
                    path = root.display()
                )
            })?;
        }
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn ensure_lane_snapshot_dir(&self, lanes_root: &Path, entry: &LaneConfigEntry) -> Result<()> {
        let dir = lane_snapshot_dir(lanes_root, entry);
        fs::create_dir_all(&dir).wrap_err_with(|| {
            format!(
                "failed to prepare lane snapshot directory {path}",
                path = dir.display()
            )
        })?;
        iroha_logger::info!(
            lane = %entry.lane_id.as_u32(),
            alias = entry.alias,
            dir = %dir.display(),
            "tiered-state: lane snapshot directory provisioned"
        );
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn retire_lane_snapshot_dir(
        &mut self,
        root: &Path,
        lanes_root: &Path,
        entry: &LaneConfigEntry,
    ) -> Result<()> {
        let dir = lane_snapshot_dir(lanes_root, entry);
        if !dir.exists() {
            return Ok(());
        }

        let retired_root = root.join("retired").join("lanes");
        fs::create_dir_all(&retired_root).wrap_err_with(|| {
            format!(
                "failed to create retired lane directory {path}",
                path = retired_root.display()
            )
        })?;
        let dest = unique_retired_lane_path(&retired_root, &entry.kura_segment);
        fs::rename(&dir, &dest).wrap_err_with(|| {
            format!(
                "failed to archive retired lane directory {path}",
                path = dir.display()
            )
        })?;
        let dest_parent = dest.parent();
        let dir_parent = dir.parent();
        if let Some(parent) = dest_parent {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync retired lane snapshot directory {path}",
                    path = parent.display()
                )
            })?;
        }
        if let Some(parent) = dir_parent {
            if Some(parent) != dest_parent {
                Self::sync_dir(parent).wrap_err_with(|| {
                    format!(
                        "failed to sync lane snapshot directory {path}",
                        path = parent.display()
                    )
                })?;
            }
        }
        iroha_logger::info!(
            lane = %entry.lane_id.as_u32(),
            alias = entry.alias,
            source = %dir.display(),
            target = %dest.display(),
            "tiered-state: retired lane snapshot directory"
        );
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn collect_world_entries(
        &mut self,
        world: &World,
        snapshot_idx: u64,
        scores: &mut Vec<EntryScore>,
        seen: &mut BTreeSet<TieredEntryId>,
    ) -> Result<()> {
        let mut ctx = CollectContext {
            snapshot_idx,
            scores,
            seen,
        };
        macro_rules! collect_map {
            ($segment:expr, $variant:ident, $storage:expr) => {{
                let view = $storage.view();
                for (key, value) in view.iter() {
                    let key_handle = TieredKeyHandle::$variant((*key).clone());
                    self.collect_entry($segment, &key_handle, key, value, &mut ctx)?;
                }
            }};
        }

        collect_map!(TieredSegment::Domains, Domain, world.domains);
        collect_map!(TieredSegment::Accounts, Account, world.accounts);
        collect_map!(
            TieredSegment::AccountRekeyRecords,
            AccountRekey,
            world.account_rekey_records
        );
        collect_map!(
            TieredSegment::AssetDefinitions,
            AssetDefinition,
            world.asset_definitions
        );
        collect_map!(TieredSegment::Assets, Asset, world.assets);
        collect_map!(
            TieredSegment::AssetMetadata,
            AssetMetadata,
            world.asset_metadata
        );
        collect_map!(TieredSegment::Nfts, Nft, world.nfts);
        collect_map!(TieredSegment::Roles, Role, world.roles);
        collect_map!(
            TieredSegment::AccountPermissions,
            AccountPermission,
            world.account_permissions
        );
        collect_map!(
            TieredSegment::AccountRoles,
            AccountRole,
            world.account_roles
        );
        collect_map!(TieredSegment::TxSequences, TxSequence, world.tx_sequences);
        collect_map!(
            TieredSegment::VerifyingKeys,
            VerifyingKey,
            world.verifying_keys
        );
        collect_map!(
            TieredSegment::RuntimeUpgrades,
            RuntimeUpgrade,
            world.runtime_upgrades
        );
        collect_map!(TieredSegment::Proofs, Proof, world.proofs);
        collect_map!(TieredSegment::ProofTags, ProofTag, world.proof_tags);
        collect_map!(TieredSegment::ProofsByTag, ProofByTag, world.proofs_by_tag);
        collect_map!(TieredSegment::CommitQcs, CommitQc, world.commit_qcs);
        collect_map!(
            TieredSegment::ContractManifests,
            ContractManifest,
            world.contract_manifests
        );
        collect_map!(
            TieredSegment::ContractCode,
            ContractCode,
            world.contract_code
        );
        {
            let view = world.contract_instances.view();
            for (key, value) in view.iter() {
                let key_handle = TieredKeyHandle::ContractInstance((*key).clone());
                let key_payload = json::to_vec(&vec![key.0.clone(), key.1.clone()])
                    .wrap_err("failed to encode contract instance key for tiered snapshot")?;
                self.collect_entry_with_encoded_key(
                    TieredSegment::ContractInstances,
                    &key_handle,
                    key_payload,
                    value,
                    &mut ctx,
                )?;
            }
        }
        collect_map!(
            TieredSegment::SmartContractState,
            SmartContractState,
            world.smart_contract_state
        );
        collect_map!(TieredSegment::ZkAssets, ZkAsset, world.zk_assets);
        collect_map!(TieredSegment::Elections, Election, world.elections);
        collect_map!(
            TieredSegment::GovernanceProposals,
            GovernanceProposal,
            world.governance_proposals
        );
        collect_map!(
            TieredSegment::GovernanceReferenda,
            GovernanceReferendum,
            world.governance_referenda
        );
        collect_map!(
            TieredSegment::GovernanceLocks,
            GovernanceLock,
            world.governance_locks
        );
        collect_map!(
            TieredSegment::GovernanceSlashes,
            GovernanceSlash,
            world.governance_slashes
        );
        collect_map!(TieredSegment::Council, Council, world.council);
        collect_map!(
            TieredSegment::ParliamentBodies,
            ParliamentBodies,
            world.parliament_bodies
        );
        collect_map!(
            TieredSegment::OfflineAllowances,
            OfflineAllowance,
            world.offline_allowances
        );
        collect_map!(
            TieredSegment::OfflineVerdictRevocations,
            OfflineVerdictRevocation,
            world.offline_verdict_revocations
        );
        collect_map!(
            TieredSegment::OfflineConsumedBuildClaimIds,
            OfflineConsumedBuildClaimId,
            world.offline_consumed_build_claim_ids
        );
        collect_map!(
            TieredSegment::OfflineTransfers,
            OfflineTransfer,
            world.offline_to_online_transfers
        );

        Ok(())
    }

    fn collect_entry<K, V>(
        &mut self,
        segment: TieredSegment,
        key_handle: &TieredKeyHandle,
        key: &K,
        value: &V,
        ctx: &mut CollectContext,
    ) -> Result<()>
    where
        K: norito::codec::Encode,
        V: json::JsonSerialize + NoritoSerialize + MeasuredBytes,
    {
        let key_encoded = norito::codec::Encode::encode(key);
        self.collect_entry_with_encoded_key(segment, key_handle, key_encoded, value, ctx)
    }

    fn collect_entry_with_encoded_key<V>(
        &mut self,
        segment: TieredSegment,
        key_handle: &TieredKeyHandle,
        key_encoded: Vec<u8>,
        value: &V,
        ctx: &mut CollectContext,
    ) -> Result<()>
    where
        V: json::JsonSerialize + NoritoSerialize + MeasuredBytes,
    {
        let key_hash = sha256(&key_encoded);
        let id = TieredEntryId::new(segment, key_hash);

        let (value_hash, _json_len) =
            compute_json_hash(value).wrap_err("failed to encode value for tiered snapshot")?;
        let value_size_bytes =
            compute_hot_bytes(value).wrap_err("failed to compute WSV hot-tier size estimate")?;

        let meta = self
            .entries
            .entry(id)
            .or_insert_with(|| EntryMetadata::new(ctx.snapshot_idx, value_hash, value_size_bytes));
        if meta.last_value_hash != value_hash {
            meta.last_value_hash = value_hash;
            meta.last_mutated_snapshot = ctx.snapshot_idx;
        }
        meta.last_present_snapshot = ctx.snapshot_idx;
        meta.value_size_bytes = value_size_bytes;
        self.entry_keys.insert(
            id,
            EntryKey {
                key: key_handle.clone(),
                key_encoded: key_encoded.clone(),
            },
        );
        ctx.seen.insert(id);

        ctx.scores.push(EntryScore {
            id,
            segment,
            key: key_handle.clone(),
            key_encoded,
        });

        Ok(())
    }

    fn write_manifest(snapshot_dir: &Path, manifest: &TieredSnapshotManifest) -> Result<()> {
        let manifest_bytes = norito::json::to_vec_pretty(manifest)
            .wrap_err("failed to serialize tiered state manifest")?;
        let manifest_path = snapshot_dir.join("manifest.json");
        let temp_path = snapshot_dir.join("manifest.json.tmp");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .wrap_err_with(|| {
                format!(
                    "failed to open manifest temp file {path}",
                    path = temp_path.display()
                )
            })?;
        file.write_all(&manifest_bytes).wrap_err_with(|| {
            format!(
                "failed to write manifest temp file {path}",
                path = temp_path.display()
            )
        })?;
        file.flush().wrap_err_with(|| {
            format!(
                "failed to flush manifest temp file {path}",
                path = temp_path.display()
            )
        })?;
        file.sync_data().wrap_err_with(|| {
            format!(
                "failed to sync manifest temp file {path}",
                path = temp_path.display()
            )
        })?;
        fs::rename(&temp_path, &manifest_path).wrap_err_with(|| {
            format!(
                "failed to promote manifest file {path}",
                path = manifest_path.display()
            )
        })
    }

    /// Prune cold snapshots to fit within an explicit byte budget.
    pub(crate) fn prune_cold_snapshots_to_bytes(&self, max_bytes: u64) -> Result<()> {
        let Some(root) = self.primary_cold_root().cloned() else {
            return Ok(());
        };
        if !root.exists() {
            return Ok(());
        }
        self.prune_snapshots_to_bytes(&root, max_bytes)
    }

    fn prune_old_snapshots(&self, root: &Path) -> Result<()> {
        if self.max_snapshots == 0 {
            return Ok(());
        }

        let mut pruned = false;
        let mut entries = fs::read_dir(root)
            .wrap_err_with(|| {
                format!(
                    "failed to read cold tier root {path}",
                    path = root.display()
                )
            })?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(std::fs::FileType::is_dir)
                    .and_then(|_| Self::parse_snapshot_dir_name(&entry.file_name()))
                    .map(|idx| (idx, entry))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(idx, _)| *idx);
        while entries.len() > self.max_snapshots {
            if let Some((_, entry)) = entries.first() {
                fs::remove_dir_all(entry.path()).with_context(|| {
                    format!(
                        "failed to prune tiered snapshot directory {path}",
                        path = entry.path().display()
                    )
                })?;
                pruned = true;
            }
            entries.remove(0);
        }
        if pruned {
            Self::sync_dir(root).wrap_err_with(|| {
                format!(
                    "failed to sync tiered snapshot root after prune {path}",
                    path = root.display()
                )
            })?;
        }
        Ok(())
    }

    fn prune_to_cold_bytes(&self, root: &Path) -> Result<()> {
        if self.max_cold_bytes == 0 {
            return Ok(());
        }
        self.prune_snapshots_to_bytes(root, self.max_cold_bytes)
    }

    fn prune_snapshots_to_bytes(&self, root: &Path, max_bytes: u64) -> Result<()> {
        if max_bytes == 0 && !root.exists() {
            return Ok(());
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(root).wrap_err_with(|| {
            format!(
                "failed to read tiered snapshot root {path}",
                path = root.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            if let Some(idx) = Self::parse_snapshot_dir_name(&entry.file_name()) {
                entries.push((idx, entry.path()));
            }
        }

        entries.sort_by_key(|(idx, _)| *idx);

        let mut total_bytes = 0u64;
        let mut sizes = Vec::with_capacity(entries.len());
        for (idx, path) in entries {
            let size = Self::snapshot_dir_size(&path)?;
            total_bytes = total_bytes.saturating_add(size);
            sizes.push((idx, path, size));
        }

        let mut pruned = false;
        while total_bytes > max_bytes && sizes.len() > 1 {
            let (idx, path, size) = sizes.remove(0);
            if let Some(da_root) = self.da_store_root_for_offload() {
                Self::offload_snapshot_dir(&path, da_root, idx)?;
                self.record_da_churn(DA_CHURN_EVICTED, size);
            } else {
                fs::remove_dir_all(&path).wrap_err_with(|| {
                    format!(
                        "failed to prune tiered snapshot directory {path}",
                        path = path.display()
                    )
                })?;
            }
            total_bytes = total_bytes.saturating_sub(size);
            pruned = true;
        }

        if total_bytes > max_bytes && sizes.len() == 1 {
            let (_, path, size) = &sizes[0];
            iroha_logger::warn!(
                budget = max_bytes,
                remaining = *size,
                path = %path.display(),
                "tiered-state: cold snapshot exceeds configured byte budget"
            );
        }

        if pruned {
            Self::sync_dir(root).wrap_err_with(|| {
                format!(
                    "failed to sync tiered snapshot root after cold prune {path}",
                    path = root.display()
                )
            })?;
        }
        Ok(())
    }

    fn snapshot_dir_size(path: &Path) -> Result<u64> {
        let mut total = 0u64;
        for entry in fs::read_dir(path).wrap_err_with(|| {
            format!(
                "failed to read snapshot directory {path}",
                path = path.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let entry_path = entry.path();
            if file_type.is_dir() {
                total = total.saturating_add(Self::snapshot_dir_size(&entry_path)?);
            } else if file_type.is_file() {
                total = total.saturating_add(entry.metadata()?.len());
            }
        }
        Ok(total)
    }

    fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
        fs::create_dir_all(dest).wrap_err_with(|| {
            format!(
                "failed to create snapshot offload directory {path}",
                path = dest.display()
            )
        })?;
        for entry in fs::read_dir(source).wrap_err_with(|| {
            format!(
                "failed to read snapshot directory {path}",
                path = source.display()
            )
        })? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let from = entry.path();
            let to = dest.join(entry.file_name());
            if file_type.is_dir() {
                Self::copy_dir_recursive(&from, &to)?;
            } else if file_type.is_file() {
                if to.exists() {
                    fs::remove_file(&to).wrap_err_with(|| {
                        format!("failed to remove existing file {path}", path = to.display())
                    })?;
                }
                fs::copy(&from, &to).wrap_err_with(|| {
                    format!(
                        "failed to copy snapshot file from {from} to {to}",
                        from = from.display(),
                        to = to.display()
                    )
                })?;
            } else {
                iroha_logger::warn!(
                    path = %from.display(),
                    "tiered-state: skipping unsupported snapshot entry during offload"
                );
            }
        }
        Ok(())
    }

    fn is_cross_device_link(err: &std::io::Error) -> bool {
        #[cfg(unix)]
        {
            err.raw_os_error() == Some(18)
        }
        #[cfg(windows)]
        {
            err.raw_os_error() == Some(17)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = err;
            false
        }
    }

    fn offload_snapshot_dir(source: &Path, dest_root: &Path, idx: u64) -> Result<()> {
        fs::create_dir_all(dest_root).wrap_err_with(|| {
            format!(
                "failed to prepare DA snapshot root {path}",
                path = dest_root.display()
            )
        })?;
        let dest = dest_root.join(format!("{idx:020}"));
        if dest.exists() {
            fs::remove_dir_all(&dest).wrap_err_with(|| {
                format!(
                    "failed to remove existing DA snapshot directory {path}",
                    path = dest.display()
                )
            })?;
        }

        match fs::rename(source, &dest) {
            Ok(()) => {}
            Err(err) if Self::is_cross_device_link(&err) => {
                let staging = dest.with_extension("staging");
                if staging.exists() {
                    fs::remove_dir_all(&staging).wrap_err_with(|| {
                        format!(
                            "failed to clear DA snapshot staging directory {path}",
                            path = staging.display()
                        )
                    })?;
                }
                Self::copy_dir_recursive(source, &staging)?;
                fs::rename(&staging, &dest).wrap_err_with(|| {
                    format!(
                        "failed to promote DA snapshot directory {path}",
                        path = dest.display()
                    )
                })?;
                fs::remove_dir_all(source).wrap_err_with(|| {
                    format!(
                        "failed to remove cold snapshot directory {path}",
                        path = source.display()
                    )
                })?;
            }
            Err(err) => {
                return Err(err).wrap_err_with(|| {
                    format!(
                        "failed to offload snapshot directory {path}",
                        path = source.display()
                    )
                });
            }
        }

        if let Some(parent) = dest.parent() {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync DA snapshot root {path}",
                    path = parent.display()
                )
            })?;
        }
        if let Some(parent) = source.parent() {
            Self::sync_dir(parent).wrap_err_with(|| {
                format!(
                    "failed to sync cold snapshot root {path}",
                    path = parent.display()
                )
            })?;
        }

        Ok(())
    }

    fn try_reuse_cold_payload(source: &Path, dest: &Path) -> Result<Option<u64>> {
        let metadata = match fs::metadata(source) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).wrap_err_with(|| {
                    format!(
                        "failed to read cold shard metadata from {path}",
                        path = source.display()
                    )
                });
            }
        };
        if !metadata.is_file() {
            return Ok(None);
        }
        if dest.exists() {
            fs::remove_file(dest).wrap_err_with(|| {
                format!(
                    "failed to remove existing cold shard at {path}",
                    path = dest.display()
                )
            })?;
        }
        match fs::hard_link(source, dest) {
            Ok(()) => {
                let file = fs::File::open(dest).wrap_err_with(|| {
                    format!(
                        "failed to open hard-linked cold shard {path} for syncing",
                        path = dest.display()
                    )
                })?;
                file.sync_all().wrap_err_with(|| {
                    format!(
                        "failed to sync hard-linked cold shard {path}",
                        path = dest.display()
                    )
                })?;
                Ok(Some(metadata.len()))
            }
            Err(link_err) => {
                iroha_logger::debug!(
                    ?link_err,
                    source = %source.display(),
                    dest = %dest.display(),
                    "tiered-state: hard-link reuse failed; falling back to copy"
                );
                let bytes = fs::copy(source, dest).wrap_err_with(|| {
                    format!(
                        "failed to copy cold shard from {source} to {dest}",
                        source = source.display(),
                        dest = dest.display()
                    )
                })?;
                let file = fs::File::open(dest).wrap_err_with(|| {
                    format!(
                        "failed to open copied cold shard {path} for syncing",
                        path = dest.display()
                    )
                })?;
                file.sync_all().wrap_err_with(|| {
                    format!(
                        "failed to sync copied cold shard {path}",
                        path = dest.display()
                    )
                })?;
                Ok(Some(bytes))
            }
        }
    }

    fn parse_snapshot_dir_name(name: &std::ffi::OsStr) -> Option<u64> {
        let name = name.to_str()?;
        if name.len() != 20 || !name.as_bytes().iter().all(u8::is_ascii_digit) {
            return None;
        }
        name.parse::<u64>().ok()
    }

    fn sync_dir(path: &Path) -> Result<()> {
        let file = fs::File::open(path).wrap_err_with(|| {
            format!(
                "failed to open directory {path} for syncing",
                path = path.display()
            )
        })?;
        file.sync_all()
            .wrap_err_with(|| format!("failed to sync directory {path}", path = path.display()))
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

fn compute_json_hash(value: &impl json::JsonSerialize) -> Result<([u8; 32], usize)> {
    let encoded = json::to_vec(value).wrap_err("failed to encode snapshot value as JSON")?;
    Ok((sha256(&encoded), encoded.len()))
}

/// Deterministic byte estimate for hot-tier sizing.
pub(crate) trait MeasuredBytes {
    /// Return the total measured byte footprint used for hot-tier budgeting.
    fn measured_bytes(&self) -> usize;

    /// Return bytes beyond the stack size of the value.
    fn measured_bytes_extra(&self) -> usize
    where
        Self: Sized,
    {
        self.measured_bytes()
            .saturating_sub(std::mem::size_of::<Self>())
    }
}

#[allow(clippy::unnecessary_wraps)]
fn compute_hot_bytes(value: &impl MeasuredBytes) -> Result<usize> {
    Ok(value.measured_bytes())
}

mod measured_bytes_impls {
    use super::MeasuredBytes;
    use std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        mem::size_of,
        sync::Arc,
    };

    use iroha_crypto::{
        Hash, HashOf, LaneCommitmentId, MerkleProof, MerkleTree, PublicKey, Signature, SignatureOf,
    };
    use iroha_data_model::{
        account::{
            AccountController, AccountDetails, AccountId, AccountLabel, AccountRekeyRecord,
            MultisigMember, MultisigPolicy, OpaqueAccountId,
        },
        asset::{
            AssetDefinition, AssetDefinitionId, AssetId,
            definition::{
                AssetConfidentialPolicy, ConfidentialPolicyMode, ConfidentialPolicyTransition,
                Mintable,
            },
        },
        bridge::{
            BridgeHashFunction, BridgeIcsProof, BridgeProof, BridgeProofPayload, BridgeProofRange,
            BridgeProofRecord, BridgeTransparentProof,
        },
        common::Owned,
        confidential::ConfidentialStatus,
        consensus::{CertPhase, Qc, QcAggregate, QcRef},
        domain::{Domain, DomainId},
        events::EventFilterBox,
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
            ParliamentBodies, ParliamentBody, ParliamentRoster, ProposalKind,
            RuntimeUpgradeProposal,
        },
        ipfs::IpfsPath,
        isi::governance::CouncilDerivationKind,
        isi::zk::ZkAssetMode,
        metadata::Metadata,
        name::Name,
        nexus::{
            LanePrivacyMerkleWitness, LanePrivacyProof, LanePrivacySnarkWitness,
            LanePrivacyWitness, UniversalAccountId,
        },
        nft::NftData,
        offline::{
            AggregateProofEnvelope, AndroidMarkerKeyProof, AndroidProvisionedProof,
            AppleAppAttestProof, OfflineAllowanceCommitment, OfflineAllowanceRecord,
            OfflineBalanceProof, OfflineCounterState, OfflinePlatformProof,
            OfflinePlatformTokenSnapshot, OfflineSpendReceipt, OfflineToOnlineTransfer,
            OfflineTransferLifecycleEntry, OfflineTransferRecord, OfflineTransferStatus,
            OfflineVerdictRevocation, OfflineVerdictRevocationReason, OfflineVerdictSnapshot,
            OfflineWalletCertificate, OfflineWalletPolicy, PoseidonDigest,
        },
        peer::PeerId,
        permission::Permission,
        proof::{
            ProofAttachment, ProofAttachmentList, ProofBox, ProofId, ProofRecord, ProofStatus,
            VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord,
        },
        role::{Role, RoleId},
        runtime::{
            RuntimeUpgradeId, RuntimeUpgradeManifest, RuntimeUpgradeRecord,
            RuntimeUpgradeSbomDigest, RuntimeUpgradeStatus,
        },
        smart_contract::manifest::{
            AccessSetHints, ContractManifest, EntryPointKind, EntrypointDescriptor,
            KotobaTranslation, KotobaTranslationEntry, ManifestProvenance, TriggerCallback,
            TriggerDescriptor,
        },
        sorafs_uri::SorafsUri,
        trigger::{TriggerId, action::Repeats},
        zk::BackendTag,
    };
    use iroha_primitives::{
        bigint::BigInt,
        const_vec::ConstVec,
        conststr::ConstString,
        json::Json,
        numeric::{Numeric, NumericSpec},
    };

    use crate::{
        governance::state::ParliamentTerm,
        state::{
            ElectionState, FrontierCheckpoint, GovernanceLockRecord, GovernanceLocksForReferendum,
            GovernanceParliamentSnapshot, GovernancePipeline, GovernanceProposalRecord,
            GovernanceProposalStatus, GovernanceReferendumMode, GovernanceReferendumRecord,
            GovernanceReferendumStatus, GovernanceSlashEntry, GovernanceSlashLedger,
            GovernanceStage, GovernanceStageApproval, GovernanceStageApprovals,
            GovernanceStageFailure, GovernanceStageRecord, ZkAssetState, ZkAssetVerifierBinding,
        },
    };

    macro_rules! impl_measured_bytes_copy {
        ($($ty:ty),+ $(,)?) => {
            $(
                impl MeasuredBytes for $ty {
                    fn measured_bytes(&self) -> usize {
                        size_of::<$ty>()
                    }
                }
            )+
        };
    }

    fn slice_bytes<T: MeasuredBytes>(slice: &[T]) -> usize {
        let mut total = slice.len().saturating_mul(size_of::<T>());
        for item in slice {
            total = total.saturating_add(item.measured_bytes_extra());
        }
        total
    }

    impl_measured_bytes_copy!(
        (),
        bool,
        char,
        u8,
        u16,
        u32,
        u64,
        u128,
        usize,
        i8,
        i16,
        i32,
        i64,
        i128,
        isize,
        AbiVersion,
        BackendTag,
        BridgeHashFunction,
        CertPhase,
        ConfidentialPolicyMode,
        ConfidentialPolicyTransition,
        ConfidentialStatus,
        ContractAbiHash,
        ContractCodeHash,
        CouncilDerivationKind,
        EntryPointKind,
        GovernanceReferendumMode,
        GovernanceReferendumStatus,
        GovernanceProposalStatus,
        GovernanceStage,
        GovernanceStageFailure,
        LaneCommitmentId,
        Mintable,
        NumericSpec,
        OfflineTransferStatus,
        OfflineVerdictRevocationReason,
        ParliamentBody,
        ProofStatus,
        QcRef,
        Repeats,
        RuntimeUpgradeId,
        RuntimeUpgradeStatus,
        ZkAssetMode,
    );

    impl<T: MeasuredBytes, const N: usize> MeasuredBytes for [T; N] {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<[T; N]>();
            for item in self {
                total = total.saturating_add(item.measured_bytes_extra());
            }
            total
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for Option<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Option<T>>();
            if let Some(value) = self {
                total = total.saturating_add(value.measured_bytes_extra());
            }
            total
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for Owned<T> {
        fn measured_bytes(&self) -> usize {
            size_of::<Owned<T>>().saturating_add(self.0.measured_bytes_extra())
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for Vec<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Vec<T>>();
            total = total.saturating_add(self.capacity().saturating_mul(size_of::<T>()));
            for item in self {
                total = total.saturating_add(item.measured_bytes_extra());
            }
            total
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for VecDeque<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<VecDeque<T>>();
            total = total.saturating_add(self.capacity().saturating_mul(size_of::<T>()));
            for item in self {
                total = total.saturating_add(item.measured_bytes_extra());
            }
            total
        }
    }

    impl<K: MeasuredBytes, V: MeasuredBytes> MeasuredBytes for BTreeMap<K, V> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BTreeMap<K, V>>();
            for (key, value) in self {
                total = total.saturating_add(size_of::<K>());
                total = total.saturating_add(key.measured_bytes_extra());
                total = total.saturating_add(size_of::<V>());
                total = total.saturating_add(value.measured_bytes_extra());
            }
            total
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for BTreeSet<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BTreeSet<T>>();
            for item in self {
                total = total.saturating_add(size_of::<T>());
                total = total.saturating_add(item.measured_bytes_extra());
            }
            total
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for Box<T> {
        fn measured_bytes(&self) -> usize {
            size_of::<Box<T>>().saturating_add((**self).measured_bytes())
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for Arc<T> {
        fn measured_bytes(&self) -> usize {
            size_of::<Arc<T>>().saturating_add((**self).measured_bytes())
        }
    }

    impl<T: MeasuredBytes> MeasuredBytes for ConstVec<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ConstVec<T>>();
            total = total.saturating_add(self.len().saturating_mul(size_of::<T>()));
            for item in self {
                total = total.saturating_add(item.measured_bytes_extra());
            }
            total
        }
    }

    impl MeasuredBytes for ConstString {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ConstString>();
            if !self.is_inlined() {
                total = total.saturating_add(self.len());
            }
            total
        }
    }

    impl MeasuredBytes for String {
        fn measured_bytes(&self) -> usize {
            size_of::<String>().saturating_add(self.capacity())
        }
    }

    impl MeasuredBytes for Json {
        fn measured_bytes(&self) -> usize {
            size_of::<Json>().saturating_add(self.get().capacity())
        }
    }

    impl MeasuredBytes for BigInt {
        fn measured_bytes(&self) -> usize {
            let bits = self.bit_len();
            let bytes = bits.saturating_add(7) / 8;
            size_of::<BigInt>().saturating_add(bytes)
        }
    }

    impl MeasuredBytes for Numeric {
        fn measured_bytes(&self) -> usize {
            size_of::<Numeric>().saturating_add(self.mantissa().measured_bytes_extra())
        }
    }

    impl<T> MeasuredBytes for HashOf<T> {
        fn measured_bytes(&self) -> usize {
            size_of::<HashOf<T>>()
        }
    }

    impl MeasuredBytes for Hash {
        fn measured_bytes(&self) -> usize {
            size_of::<Hash>()
        }
    }

    impl MeasuredBytes for OpaqueAccountId {
        fn measured_bytes(&self) -> usize {
            size_of::<OpaqueAccountId>()
        }
    }

    impl MeasuredBytes for PublicKey {
        fn measured_bytes(&self) -> usize {
            let payload_len = self.to_bytes().1.len();
            size_of::<PublicKey>().saturating_add(payload_len)
        }
    }

    impl MeasuredBytes for Signature {
        fn measured_bytes(&self) -> usize {
            size_of::<Signature>().saturating_add(self.payload().len())
        }
    }

    impl<T> MeasuredBytes for SignatureOf<T> {
        fn measured_bytes(&self) -> usize {
            size_of::<SignatureOf<T>>().saturating_add((**self).measured_bytes_extra())
        }
    }

    impl<T> MeasuredBytes for MerkleProof<T> {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<MerkleProof<T>>();
            total = total.saturating_add(
                self.audit_path()
                    .len()
                    .saturating_mul(size_of::<Option<HashOf<T>>>()),
            );
            total
        }
    }

    impl<T> MeasuredBytes for MerkleTree<T> {
        fn measured_bytes(&self) -> usize {
            let depth = self.depth();
            let nodes = if depth as usize >= usize::BITS as usize - 1 {
                usize::MAX
            } else {
                (1_usize << (depth + 1)) - 1
            };
            size_of::<MerkleTree<T>>()
                .saturating_add(nodes.saturating_mul(size_of::<Option<HashOf<T>>>()))
        }
    }

    impl MeasuredBytes for Name {
        fn measured_bytes(&self) -> usize {
            size_of::<Name>().saturating_add(self.as_ref().len())
        }
    }

    impl MeasuredBytes for TriggerId {
        fn measured_bytes(&self) -> usize {
            size_of::<TriggerId>().saturating_add(self.name.measured_bytes_extra())
        }
    }

    impl MeasuredBytes for IpfsPath {
        fn measured_bytes(&self) -> usize {
            size_of::<IpfsPath>().saturating_add(self.as_ref().len())
        }
    }

    impl MeasuredBytes for SorafsUri {
        fn measured_bytes(&self) -> usize {
            size_of::<SorafsUri>().saturating_add(self.as_ref().len())
        }
    }

    impl MeasuredBytes for DomainId {
        fn measured_bytes(&self) -> usize {
            size_of::<DomainId>().saturating_add(self.name.measured_bytes_extra())
        }
    }

    impl MeasuredBytes for AccountLabel {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccountLabel>();
            total = total.saturating_add(self.domain.measured_bytes_extra());
            total = total.saturating_add(self.label.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for UniversalAccountId {
        fn measured_bytes(&self) -> usize {
            size_of::<UniversalAccountId>()
        }
    }

    impl MeasuredBytes for AccountController {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccountController>();
            match self {
                AccountController::Single(key) => {
                    total = total.saturating_add(key.measured_bytes_extra());
                }
                AccountController::Multisig(policy) => {
                    total = total.saturating_add(policy.measured_bytes_extra());
                }
            }
            total
        }
    }

    impl MeasuredBytes for MultisigMember {
        fn measured_bytes(&self) -> usize {
            size_of::<MultisigMember>().saturating_add(self.public_key().measured_bytes_extra())
        }
    }

    impl MeasuredBytes for MultisigPolicy {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<MultisigPolicy>();
            total = total.saturating_add(slice_bytes(self.members()));
            total
        }
    }

    impl MeasuredBytes for AccountId {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccountId>();
            total = total.saturating_add(self.controller.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AssetDefinitionId {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AssetDefinitionId>();
            total = total.saturating_add(self.domain.measured_bytes_extra());
            total = total.saturating_add(self.name.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AssetId {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AssetId>();
            total = total.saturating_add(self.account.measured_bytes_extra());
            total = total.saturating_add(self.definition.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for RoleId {
        fn measured_bytes(&self) -> usize {
            size_of::<RoleId>().saturating_add(self.name.measured_bytes_extra())
        }
    }

    impl MeasuredBytes for PeerId {
        fn measured_bytes(&self) -> usize {
            size_of::<PeerId>().saturating_add(self.public_key.measured_bytes_extra())
        }
    }

    impl MeasuredBytes for Metadata {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Metadata>();
            for (key, value) in self.iter() {
                total = total.saturating_add(size_of::<Name>());
                total = total.saturating_add(key.measured_bytes_extra());
                total = total.saturating_add(size_of::<Json>());
                total = total.saturating_add(value.measured_bytes_extra());
            }
            total
        }
    }

    impl MeasuredBytes for EventFilterBox {
        fn measured_bytes(&self) -> usize {
            size_of::<EventFilterBox>()
        }
    }

    impl MeasuredBytes for Permission {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Permission>();
            total = total.saturating_add(self.name.measured_bytes_extra());
            total = total.saturating_add(self.payload.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AccountDetails {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccountDetails>();
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total = total.saturating_add(self.label.measured_bytes_extra());
            total = total.saturating_add(self.uaid.measured_bytes_extra());
            total = total.saturating_add(self.opaque_ids.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AccountRekeyRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccountRekeyRecord>();
            total = total.saturating_add(self.label.measured_bytes_extra());
            total = total.saturating_add(self.active_signatory.measured_bytes_extra());
            total = total.saturating_add(self.previous_signatories.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AssetConfidentialPolicy {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AssetConfidentialPolicy>();
            total = total.saturating_add(self.mode.measured_bytes_extra());
            total = total.saturating_add(self.vk_set_hash.measured_bytes_extra());
            total = total.saturating_add(self.poseidon_params_id.measured_bytes_extra());
            total = total.saturating_add(self.pedersen_params_id.measured_bytes_extra());
            total = total.saturating_add(self.pending_transition.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AssetDefinition {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AssetDefinition>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.spec.measured_bytes_extra());
            total = total.saturating_add(self.mintable.measured_bytes_extra());
            total = total.saturating_add(self.logo.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total = total.saturating_add(self.owned_by.measured_bytes_extra());
            total = total.saturating_add(self.total_quantity.measured_bytes_extra());
            total = total.saturating_add(self.confidential_policy.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for Domain {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Domain>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.logo.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total = total.saturating_add(self.owned_by.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for NftData {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<NftData>();
            total = total.saturating_add(self.content.measured_bytes_extra());
            total = total.saturating_add(self.owned_by.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for Role {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Role>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.permissions.measured_bytes_extra());
            total = total.saturating_add(self.permission_epochs.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for VerifyingKeyId {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<VerifyingKeyId>();
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.name.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for VerifyingKeyBox {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<VerifyingKeyBox>();
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.bytes.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for VerifyingKeyRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<VerifyingKeyRecord>();
            total = total.saturating_add(self.circuit_id.measured_bytes_extra());
            total = total.saturating_add(self.owner_manifest_id.measured_bytes_extra());
            total = total.saturating_add(self.namespace.measured_bytes_extra());
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.curve.measured_bytes_extra());
            total = total.saturating_add(self.public_inputs_schema_hash.measured_bytes_extra());
            total = total.saturating_add(self.commitment.measured_bytes_extra());
            total = total.saturating_add(self.vk_len.measured_bytes_extra());
            total = total.saturating_add(self.max_proof_bytes.measured_bytes_extra());
            total = total.saturating_add(self.gas_schedule_id.measured_bytes_extra());
            total = total.saturating_add(self.metadata_uri_cid.measured_bytes_extra());
            total = total.saturating_add(self.vk_bytes_cid.measured_bytes_extra());
            total = total.saturating_add(self.activation_height.measured_bytes_extra());
            total = total.saturating_add(self.withdraw_height.measured_bytes_extra());
            total = total.saturating_add(self.key.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProofBox {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ProofBox>();
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.bytes.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProofId {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ProofId>();
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.proof_hash.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for BridgeIcsProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BridgeIcsProof>();
            total = total.saturating_add(self.state_root.measured_bytes_extra());
            total = total.saturating_add(self.leaf_hash.measured_bytes_extra());
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total = total.saturating_add(self.hash_function.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for BridgeTransparentProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BridgeTransparentProof>();
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total = total.saturating_add(self.recursion_depth.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for BridgeProofPayload {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BridgeProofPayload>();
            match self {
                BridgeProofPayload::Ics(proof) => {
                    total = total.saturating_add(proof.measured_bytes_extra());
                }
                BridgeProofPayload::TransparentZk(proof) => {
                    total = total.saturating_add(proof.measured_bytes_extra());
                }
            }
            total
        }
    }

    impl MeasuredBytes for BridgeProofRange {
        fn measured_bytes(&self) -> usize {
            size_of::<BridgeProofRange>()
        }
    }

    impl MeasuredBytes for BridgeProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BridgeProof>();
            total = total.saturating_add(self.range.measured_bytes_extra());
            total = total.saturating_add(self.manifest_hash.measured_bytes_extra());
            total = total.saturating_add(self.payload.measured_bytes_extra());
            total = total.saturating_add(self.pinned.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for BridgeProofRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<BridgeProofRecord>();
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total = total.saturating_add(self.commitment.measured_bytes_extra());
            total = total.saturating_add(self.size_bytes.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProofRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ProofRecord>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.vk_ref.measured_bytes_extra());
            total = total.saturating_add(self.vk_commitment.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.verified_at_height.measured_bytes_extra());
            total = total.saturating_add(self.bridge.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for RuntimeUpgradeSbomDigest {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<RuntimeUpgradeSbomDigest>();
            total = total.saturating_add(self.algorithm.measured_bytes_extra());
            total = total.saturating_add(self.digest.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for RuntimeUpgradeManifest {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<RuntimeUpgradeManifest>();
            total = total.saturating_add(self.name.measured_bytes_extra());
            total = total.saturating_add(self.description.measured_bytes_extra());
            total = total.saturating_add(self.abi_version.measured_bytes_extra());
            total = total.saturating_add(self.abi_hash.measured_bytes_extra());
            total = total.saturating_add(self.added_syscalls.measured_bytes_extra());
            total = total.saturating_add(self.added_pointer_types.measured_bytes_extra());
            total = total.saturating_add(self.start_height.measured_bytes_extra());
            total = total.saturating_add(self.end_height.measured_bytes_extra());
            total = total.saturating_add(self.sbom_digests.measured_bytes_extra());
            total = total.saturating_add(self.slsa_attestation.measured_bytes_extra());
            total = total.saturating_add(self.provenance.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for RuntimeUpgradeRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<RuntimeUpgradeRecord>();
            total = total.saturating_add(self.manifest.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.proposer.measured_bytes_extra());
            total = total.saturating_add(self.created_height.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AccessSetHints {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AccessSetHints>();
            total = total.saturating_add(self.read_keys.measured_bytes_extra());
            total = total.saturating_add(self.write_keys.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for TriggerCallback {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<TriggerCallback>();
            total = total.saturating_add(self.namespace.measured_bytes_extra());
            total = total.saturating_add(self.entrypoint.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for TriggerDescriptor {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<TriggerDescriptor>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.repeats.measured_bytes_extra());
            total = total.saturating_add(self.filter.measured_bytes_extra());
            total = total.saturating_add(self.authority.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total = total.saturating_add(self.callback.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for EntrypointDescriptor {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<EntrypointDescriptor>();
            total = total.saturating_add(self.name.measured_bytes_extra());
            total = total.saturating_add(self.kind.measured_bytes_extra());
            total = total.saturating_add(self.permission.measured_bytes_extra());
            total = total.saturating_add(self.read_keys.measured_bytes_extra());
            total = total.saturating_add(self.write_keys.measured_bytes_extra());
            total = total.saturating_add(self.access_hints_complete.measured_bytes_extra());
            total = total.saturating_add(self.access_hints_skipped.measured_bytes_extra());
            total = total.saturating_add(self.triggers.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for KotobaTranslation {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<KotobaTranslation>();
            total = total.saturating_add(self.lang.measured_bytes_extra());
            total = total.saturating_add(self.text.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for KotobaTranslationEntry {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<KotobaTranslationEntry>();
            total = total.saturating_add(self.msg_id.measured_bytes_extra());
            total = total.saturating_add(self.translations.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ManifestProvenance {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ManifestProvenance>();
            total = total.saturating_add(self.signer.measured_bytes_extra());
            total = total.saturating_add(self.signature.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ContractManifest {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ContractManifest>();
            total = total.saturating_add(self.code_hash.measured_bytes_extra());
            total = total.saturating_add(self.abi_hash.measured_bytes_extra());
            total = total.saturating_add(self.compiler_fingerprint.measured_bytes_extra());
            total = total.saturating_add(self.features_bitmap.measured_bytes_extra());
            total = total.saturating_add(self.access_set_hints.measured_bytes_extra());
            total = total.saturating_add(self.entrypoints.measured_bytes_extra());
            total = total.saturating_add(self.kotoba.measured_bytes_extra());
            total = total.saturating_add(self.provenance.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for QcAggregate {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<QcAggregate>();
            total = total.saturating_add(self.signers_bitmap.measured_bytes_extra());
            total = total.saturating_add(self.bls_aggregate_signature.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for Qc {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<Qc>();
            total = total.saturating_add(self.phase.measured_bytes_extra());
            total = total.saturating_add(self.subject_block_hash.measured_bytes_extra());
            total = total.saturating_add(self.parent_state_root.measured_bytes_extra());
            total = total.saturating_add(self.post_state_root.measured_bytes_extra());
            total = total.saturating_add(self.height.measured_bytes_extra());
            total = total.saturating_add(self.view.measured_bytes_extra());
            total = total.saturating_add(self.epoch.measured_bytes_extra());
            total = total.saturating_add(self.mode_tag.measured_bytes_extra());
            total = total.saturating_add(self.highest_qc.measured_bytes_extra());
            total = total.saturating_add(self.validator_set_hash.measured_bytes_extra());
            total = total.saturating_add(self.validator_set_hash_version.measured_bytes_extra());
            total = total.saturating_add(self.validator_set.measured_bytes_extra());
            total = total.saturating_add(self.aggregate.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ZkAssetVerifierBinding {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ZkAssetVerifierBinding>();
            total = total.saturating_add(self.id.measured_bytes_extra());
            total = total.saturating_add(self.commitment.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for FrontierCheckpoint {
        fn measured_bytes(&self) -> usize {
            size_of::<FrontierCheckpoint>()
        }
    }

    impl MeasuredBytes for ZkAssetState {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ZkAssetState>();
            total = total.saturating_add(self.mode.measured_bytes_extra());
            total = total.saturating_add(self.allow_shield.measured_bytes_extra());
            total = total.saturating_add(self.allow_unshield.measured_bytes_extra());
            total = total.saturating_add(self.commitments.measured_bytes_extra());
            total = total.saturating_add(self.root_history.measured_bytes_extra());
            total = total.saturating_add(self.nullifiers.measured_bytes_extra());
            total = total.saturating_add(self.vk_transfer.measured_bytes_extra());
            total = total.saturating_add(self.vk_unshield.measured_bytes_extra());
            total = total.saturating_add(self.vk_shield.measured_bytes_extra());
            total = total.saturating_add(self.frontier_checkpoints.measured_bytes_extra());
            total = total.saturating_add(self.tree.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ElectionState {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ElectionState>();
            total = total.saturating_add(self.options.measured_bytes_extra());
            total = total.saturating_add(self.eligible_root.measured_bytes_extra());
            total = total.saturating_add(self.start_ts.measured_bytes_extra());
            total = total.saturating_add(self.end_ts.measured_bytes_extra());
            total = total.saturating_add(self.finalized.measured_bytes_extra());
            total = total.saturating_add(self.tally.measured_bytes_extra());
            total = total.saturating_add(self.ballot_nullifiers.measured_bytes_extra());
            total = total.saturating_add(self.ciphertexts.measured_bytes_extra());
            total = total.saturating_add(self.vk_ballot.measured_bytes_extra());
            total = total.saturating_add(self.vk_ballot_commitment.measured_bytes_extra());
            total = total.saturating_add(self.vk_tally.measured_bytes_extra());
            total = total.saturating_add(self.vk_tally_commitment.measured_bytes_extra());
            total = total.saturating_add(self.domain_tag.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for DeployContractProposal {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<DeployContractProposal>();
            total = total.saturating_add(self.namespace.measured_bytes_extra());
            total = total.saturating_add(self.contract_id.measured_bytes_extra());
            total = total.saturating_add(self.code_hash_hex.measured_bytes_extra());
            total = total.saturating_add(self.abi_hash_hex.measured_bytes_extra());
            total = total.saturating_add(self.abi_version.measured_bytes_extra());
            total = total.saturating_add(self.manifest_provenance.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for RuntimeUpgradeProposal {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<RuntimeUpgradeProposal>();
            total = total.saturating_add(self.manifest.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProposalKind {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ProposalKind>();
            match self {
                ProposalKind::DeployContract(payload) => {
                    total = total.saturating_add(payload.measured_bytes_extra());
                }
                ProposalKind::RuntimeUpgrade(payload) => {
                    total = total.saturating_add(payload.measured_bytes_extra());
                }
            }
            total
        }
    }

    impl MeasuredBytes for GovernanceStageApproval {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceStageApproval>();
            total = total.saturating_add(self.approvers.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceStageRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceStageRecord>();
            total = total.saturating_add(self.stage.measured_bytes_extra());
            total = total.saturating_add(self.started_at.measured_bytes_extra());
            total = total.saturating_add(self.deadline.measured_bytes_extra());
            total = total.saturating_add(self.completed_at.measured_bytes_extra());
            total = total.saturating_add(self.failure.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceStageApprovals {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceStageApprovals>();
            total = total.saturating_add(self.stages.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernancePipeline {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernancePipeline>();
            total = total.saturating_add(self.stages.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceParliamentSnapshot {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceParliamentSnapshot>();
            total = total.saturating_add(self.selection_epoch.measured_bytes_extra());
            total = total.saturating_add(self.beacon.measured_bytes_extra());
            total = total.saturating_add(self.roster_root.measured_bytes_extra());
            total = total.saturating_add(self.bodies.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceProposalRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceProposalRecord>();
            total = total.saturating_add(self.proposer.measured_bytes_extra());
            total = total.saturating_add(self.kind.measured_bytes_extra());
            total = total.saturating_add(self.created_height.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.pipeline.measured_bytes_extra());
            total = total.saturating_add(self.parliament_snapshot.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceReferendumRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceReferendumRecord>();
            total = total.saturating_add(self.h_start.measured_bytes_extra());
            total = total.saturating_add(self.h_end.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.mode.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceLockRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceLockRecord>();
            total = total.saturating_add(self.owner.measured_bytes_extra());
            total = total.saturating_add(self.amount.measured_bytes_extra());
            total = total.saturating_add(self.slashed.measured_bytes_extra());
            total = total.saturating_add(self.expiry_height.measured_bytes_extra());
            total = total.saturating_add(self.direction.measured_bytes_extra());
            total = total.saturating_add(self.duration_blocks.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceLocksForReferendum {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceLocksForReferendum>();
            total = total.saturating_add(self.locks.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for GovernanceSlashEntry {
        fn measured_bytes(&self) -> usize {
            size_of::<GovernanceSlashEntry>()
        }
    }

    impl MeasuredBytes for GovernanceSlashLedger {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<GovernanceSlashLedger>();
            total = total.saturating_add(self.slashes.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ParliamentRoster {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ParliamentRoster>();
            total = total.saturating_add(self.body.measured_bytes_extra());
            total = total.saturating_add(self.epoch.measured_bytes_extra());
            total = total.saturating_add(self.members.measured_bytes_extra());
            total = total.saturating_add(self.alternates.measured_bytes_extra());
            total = total.saturating_add(self.verified.measured_bytes_extra());
            total = total.saturating_add(self.candidate_count.measured_bytes_extra());
            total = total.saturating_add(self.derived_by.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ParliamentBodies {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ParliamentBodies>();
            total = total.saturating_add(self.selection_epoch.measured_bytes_extra());
            total = total.saturating_add(self.rosters.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ParliamentTerm {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ParliamentTerm>();
            total = total.saturating_add(self.epoch.measured_bytes_extra());
            total = total.saturating_add(self.members.measured_bytes_extra());
            total = total.saturating_add(self.alternates.measured_bytes_extra());
            total = total.saturating_add(self.verified.measured_bytes_extra());
            total = total.saturating_add(self.candidate_count.measured_bytes_extra());
            total = total.saturating_add(self.derived_by.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for PoseidonDigest {
        fn measured_bytes(&self) -> usize {
            size_of::<PoseidonDigest>()
        }
    }

    impl MeasuredBytes for OfflineAllowanceCommitment {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineAllowanceCommitment>();
            total = total.saturating_add(self.asset.measured_bytes_extra());
            total = total.saturating_add(self.amount.measured_bytes_extra());
            total = total.saturating_add(self.commitment.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineWalletPolicy {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineWalletPolicy>();
            total = total.saturating_add(self.max_balance.measured_bytes_extra());
            total = total.saturating_add(self.max_tx_value.measured_bytes_extra());
            total = total.saturating_add(self.expires_at_ms.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineWalletCertificate {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineWalletCertificate>();
            total = total.saturating_add(self.controller.measured_bytes_extra());
            total = total.saturating_add(self.operator.measured_bytes_extra());
            total = total.saturating_add(self.allowance.measured_bytes_extra());
            total = total.saturating_add(self.spend_public_key.measured_bytes_extra());
            total = total.saturating_add(self.attestation_report.measured_bytes_extra());
            total = total.saturating_add(self.issued_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.expires_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.policy.measured_bytes_extra());
            total = total.saturating_add(self.operator_signature.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total = total.saturating_add(self.verdict_id.measured_bytes_extra());
            total = total.saturating_add(self.attestation_nonce.measured_bytes_extra());
            total = total.saturating_add(self.refresh_at_ms.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineCounterState {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineCounterState>();
            total = total.saturating_add(self.apple_key_counters.measured_bytes_extra());
            total = total.saturating_add(self.android_series_counters.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineAllowanceRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineAllowanceRecord>();
            total = total.saturating_add(self.certificate.measured_bytes_extra());
            total = total.saturating_add(self.current_commitment.measured_bytes_extra());
            total = total.saturating_add(self.registered_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.remaining_amount.measured_bytes_extra());
            total = total.saturating_add(self.counter_state.measured_bytes_extra());
            total = total.saturating_add(self.verdict_id.measured_bytes_extra());
            total = total.saturating_add(self.attestation_nonce.measured_bytes_extra());
            total = total.saturating_add(self.refresh_at_ms.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AppleAppAttestProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AppleAppAttestProof>();
            total = total.saturating_add(self.key_id.measured_bytes_extra());
            total = total.saturating_add(self.counter.measured_bytes_extra());
            total = total.saturating_add(self.assertion.measured_bytes_extra());
            total = total.saturating_add(self.challenge_hash.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AndroidMarkerKeyProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AndroidMarkerKeyProof>();
            total = total.saturating_add(self.series.measured_bytes_extra());
            total = total.saturating_add(self.counter.measured_bytes_extra());
            total = total.saturating_add(self.marker_public_key.measured_bytes_extra());
            total = total.saturating_add(self.marker_signature.measured_bytes_extra());
            total = total.saturating_add(self.attestation.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AndroidProvisionedProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AndroidProvisionedProof>();
            total = total.saturating_add(self.manifest_schema.measured_bytes_extra());
            total = total.saturating_add(self.manifest_version.measured_bytes_extra());
            total = total.saturating_add(self.manifest_issued_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.challenge_hash.measured_bytes_extra());
            total = total.saturating_add(self.counter.measured_bytes_extra());
            total = total.saturating_add(self.device_manifest.measured_bytes_extra());
            total = total.saturating_add(self.inspector_signature.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflinePlatformProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflinePlatformProof>();
            match self {
                OfflinePlatformProof::AppleAppAttest(payload) => {
                    total = total.saturating_add(payload.measured_bytes_extra());
                }
                OfflinePlatformProof::AndroidMarkerKey(payload) => {
                    total = total.saturating_add(payload.measured_bytes_extra());
                }
                OfflinePlatformProof::Provisioned(payload) => {
                    total = total.saturating_add(payload.measured_bytes_extra());
                }
            }
            total
        }
    }

    impl MeasuredBytes for OfflineBalanceProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineBalanceProof>();
            total = total.saturating_add(self.initial_commitment.measured_bytes_extra());
            total = total.saturating_add(self.resulting_commitment.measured_bytes_extra());
            total = total.saturating_add(self.claimed_delta.measured_bytes_extra());
            total = total.saturating_add(self.zk_proof.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineSpendReceipt {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineSpendReceipt>();
            total = total.saturating_add(self.tx_id.measured_bytes_extra());
            total = total.saturating_add(self.from.measured_bytes_extra());
            total = total.saturating_add(self.to.measured_bytes_extra());
            total = total.saturating_add(self.asset.measured_bytes_extra());
            total = total.saturating_add(self.amount.measured_bytes_extra());
            total = total.saturating_add(self.issued_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.invoice_id.measured_bytes_extra());
            total = total.saturating_add(self.platform_proof.measured_bytes_extra());
            total = total.saturating_add(self.platform_snapshot.measured_bytes_extra());
            total = total.saturating_add(self.sender_certificate_id.measured_bytes_extra());
            total = total.saturating_add(self.sender_signature.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for AggregateProofEnvelope {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<AggregateProofEnvelope>();
            total = total.saturating_add(self.version.measured_bytes_extra());
            total = total.saturating_add(self.receipts_root.measured_bytes_extra());
            total = total.saturating_add(self.proof_sum.measured_bytes_extra());
            total = total.saturating_add(self.proof_counter.measured_bytes_extra());
            total = total.saturating_add(self.proof_replay.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProofAttachment {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<ProofAttachment>();
            total = total.saturating_add(self.backend.measured_bytes_extra());
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total = total.saturating_add(self.vk_ref.measured_bytes_extra());
            total = total.saturating_add(self.vk_inline.measured_bytes_extra());
            total = total.saturating_add(self.vk_commitment.measured_bytes_extra());
            total = total.saturating_add(self.envelope_hash.measured_bytes_extra());
            total = total.saturating_add(self.lane_privacy.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for ProofAttachmentList {
        fn measured_bytes(&self) -> usize {
            size_of::<ProofAttachmentList>().saturating_add(self.0.measured_bytes_extra())
        }
    }

    impl MeasuredBytes for LanePrivacyMerkleWitness {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<LanePrivacyMerkleWitness>();
            total = total.saturating_add(self.leaf.measured_bytes_extra());
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for LanePrivacySnarkWitness {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<LanePrivacySnarkWitness>();
            total = total.saturating_add(self.public_inputs.measured_bytes_extra());
            total = total.saturating_add(self.proof.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for LanePrivacyWitness {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<LanePrivacyWitness>();
            match self {
                LanePrivacyWitness::Merkle(witness) => {
                    total = total.saturating_add(witness.measured_bytes_extra());
                }
                LanePrivacyWitness::Snark(witness) => {
                    total = total.saturating_add(witness.measured_bytes_extra());
                }
            }
            total
        }
    }

    impl MeasuredBytes for LanePrivacyProof {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<LanePrivacyProof>();
            total = total.saturating_add(self.commitment_id.measured_bytes_extra());
            total = total.saturating_add(self.witness.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflinePlatformTokenSnapshot {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflinePlatformTokenSnapshot>();
            total = total.saturating_add(self.policy.measured_bytes_extra());
            total = total.saturating_add(self.attestation_jws_b64.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineVerdictSnapshot {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineVerdictSnapshot>();
            total = total.saturating_add(self.certificate_id.measured_bytes_extra());
            total = total.saturating_add(self.verdict_id.measured_bytes_extra());
            total = total.saturating_add(self.attestation_nonce.measured_bytes_extra());
            total = total.saturating_add(self.refresh_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.certificate_expires_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.policy_expires_at_ms.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineTransferLifecycleEntry {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineTransferLifecycleEntry>();
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.transitioned_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.verdict_snapshot.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineToOnlineTransfer {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineToOnlineTransfer>();
            total = total.saturating_add(self.bundle_id.measured_bytes_extra());
            total = total.saturating_add(self.receiver.measured_bytes_extra());
            total = total.saturating_add(self.deposit_account.measured_bytes_extra());
            total = total.saturating_add(self.receipts.measured_bytes_extra());
            total = total.saturating_add(self.balance_proof.measured_bytes_extra());
            total = total.saturating_add(self.aggregate_proof.measured_bytes_extra());
            total = total.saturating_add(self.attachments.measured_bytes_extra());
            total = total.saturating_add(self.platform_snapshot.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineTransferRecord {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineTransferRecord>();
            total = total.saturating_add(self.transfer.measured_bytes_extra());
            total = total.saturating_add(self.controller.measured_bytes_extra());
            total = total.saturating_add(self.status.measured_bytes_extra());
            total = total.saturating_add(self.recorded_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.recorded_at_height.measured_bytes_extra());
            total = total.saturating_add(self.archived_at_height.measured_bytes_extra());
            total = total.saturating_add(self.history.measured_bytes_extra());
            total = total.saturating_add(self.pos_verdict_snapshots.measured_bytes_extra());
            total = total.saturating_add(self.verdict_snapshot.measured_bytes_extra());
            total = total.saturating_add(self.platform_snapshot.measured_bytes_extra());
            total
        }
    }

    impl MeasuredBytes for OfflineVerdictRevocation {
        fn measured_bytes(&self) -> usize {
            let mut total = size_of::<OfflineVerdictRevocation>();
            total = total.saturating_add(self.verdict_id.measured_bytes_extra());
            total = total.saturating_add(self.issuer.measured_bytes_extra());
            total = total.saturating_add(self.revoked_at_ms.measured_bytes_extra());
            total = total.saturating_add(self.reason.measured_bytes_extra());
            total = total.saturating_add(self.note.measured_bytes_extra());
            total = total.saturating_add(self.metadata.measured_bytes_extra());
            total
        }
    }
}

fn lane_snapshot_dir(root: &Path, entry: &LaneConfigEntry) -> PathBuf {
    root.join(&entry.kura_segment)
}

fn unique_retired_lane_path(base: &Path, stem: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs())
        .unwrap_or(0);
    let mut counter = 0u32;
    loop {
        let name = if counter == 0 {
            format!("{stem}_{stamp}")
        } else {
            format!("{stem}_{stamp}_{counter}")
        };
        let candidate = base.join(&name);
        if !candidate.exists() {
            return candidate;
        }
        counter = counter.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TieredSegment {
    Domains,
    Accounts,
    AccountRekeyRecords,
    AssetDefinitions,
    Assets,
    AssetMetadata,
    Nfts,
    Roles,
    AccountPermissions,
    AccountRoles,
    TxSequences,
    VerifyingKeys,
    RuntimeUpgrades,
    Proofs,
    ProofTags,
    ProofsByTag,
    CommitQcs,
    ContractManifests,
    ContractCode,
    ContractInstances,
    SmartContractState,
    ZkAssets,
    Elections,
    GovernanceProposals,
    GovernanceReferenda,
    GovernanceLocks,
    GovernanceSlashes,
    Council,
    ParliamentBodies,
    OfflineAllowances,
    OfflineVerdictRevocations,
    OfflineTransfers,
    OfflineConsumedBuildClaimIds,
}

impl TieredSegment {
    fn dir_name(self) -> &'static str {
        match self {
            TieredSegment::Domains => "domains",
            TieredSegment::Accounts => "accounts",
            TieredSegment::AccountRekeyRecords => "account_rekey_records",
            TieredSegment::AssetDefinitions => "asset_definitions",
            TieredSegment::Assets => "assets",
            TieredSegment::AssetMetadata => "asset_metadata",
            TieredSegment::Nfts => "nfts",
            TieredSegment::Roles => "roles",
            TieredSegment::AccountPermissions => "account_permissions",
            TieredSegment::AccountRoles => "account_roles",
            TieredSegment::TxSequences => "tx_sequences",
            TieredSegment::VerifyingKeys => "verifying_keys",
            TieredSegment::RuntimeUpgrades => "runtime_upgrades",
            TieredSegment::Proofs => "proofs",
            TieredSegment::ProofTags => "proof_tags",
            TieredSegment::ProofsByTag => "proofs_by_tag",
            TieredSegment::CommitQcs => "commit_qcs",
            TieredSegment::ContractManifests => "contract_manifests",
            TieredSegment::ContractCode => "contract_code",
            TieredSegment::ContractInstances => "contract_instances",
            TieredSegment::SmartContractState => "smart_contract_state",
            TieredSegment::ZkAssets => "zk_assets",
            TieredSegment::Elections => "elections",
            TieredSegment::GovernanceProposals => "governance_proposals",
            TieredSegment::GovernanceReferenda => "governance_referenda",
            TieredSegment::GovernanceLocks => "governance_locks",
            TieredSegment::GovernanceSlashes => "governance_slashes",
            TieredSegment::Council => "council",
            TieredSegment::ParliamentBodies => "parliament_bodies",
            TieredSegment::OfflineAllowances => "offline_allowances",
            TieredSegment::OfflineVerdictRevocations => "offline_verdict_revocations",
            TieredSegment::OfflineTransfers => "offline_transfers",
            TieredSegment::OfflineConsumedBuildClaimIds => "offline_consumed_build_claim_ids",
        }
    }
}

impl norito::json::JsonSerialize for TieredSegment {
    fn json_serialize(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.dir_name(), out);
    }
}

impl norito::json::JsonDeserialize for TieredSegment {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let name = <String as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        let segment = match name.as_str() {
            "domains" => TieredSegment::Domains,
            "accounts" => TieredSegment::Accounts,
            "account_rekey_records" => TieredSegment::AccountRekeyRecords,
            "asset_definitions" => TieredSegment::AssetDefinitions,
            "assets" => TieredSegment::Assets,
            "asset_metadata" => TieredSegment::AssetMetadata,
            "nfts" => TieredSegment::Nfts,
            "roles" => TieredSegment::Roles,
            "account_permissions" => TieredSegment::AccountPermissions,
            "account_roles" => TieredSegment::AccountRoles,
            "tx_sequences" => TieredSegment::TxSequences,
            "verifying_keys" => TieredSegment::VerifyingKeys,
            "runtime_upgrades" => TieredSegment::RuntimeUpgrades,
            "proofs" => TieredSegment::Proofs,
            "proof_tags" => TieredSegment::ProofTags,
            "proofs_by_tag" => TieredSegment::ProofsByTag,
            "commit_qcs" => TieredSegment::CommitQcs,
            "contract_manifests" => TieredSegment::ContractManifests,
            "contract_code" => TieredSegment::ContractCode,
            "contract_instances" => TieredSegment::ContractInstances,
            "smart_contract_state" => TieredSegment::SmartContractState,
            "zk_assets" => TieredSegment::ZkAssets,
            "elections" => TieredSegment::Elections,
            "governance_proposals" => TieredSegment::GovernanceProposals,
            "governance_referenda" => TieredSegment::GovernanceReferenda,
            "governance_locks" => TieredSegment::GovernanceLocks,
            "governance_slashes" => TieredSegment::GovernanceSlashes,
            "council" => TieredSegment::Council,
            "parliament_bodies" => TieredSegment::ParliamentBodies,
            "offline_allowances" => TieredSegment::OfflineAllowances,
            "offline_verdict_revocations" => TieredSegment::OfflineVerdictRevocations,
            "offline_transfers" => TieredSegment::OfflineTransfers,
            "offline_consumed_build_claim_ids" => TieredSegment::OfflineConsumedBuildClaimIds,
            other => {
                return Err(norito::json::Error::InvalidField {
                    field: "segment".into(),
                    message: format!("unknown tiered segment `{other}`"),
                });
            }
        };
        Ok(segment)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TieredEntryId {
    segment: TieredSegment,
    key_hash: [u8; 32],
}

impl TieredEntryId {
    fn new(segment: TieredSegment, key_hash: [u8; 32]) -> Self {
        Self { segment, key_hash }
    }
}

impl fmt::Display for TieredEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.segment.dir_name(),
            self.key_hash.encode_hex::<String>()
        )
    }
}

#[derive(Debug, Clone)]
struct EntryMetadata {
    last_present_snapshot: u64,
    last_mutated_snapshot: u64,
    last_hot_snapshot: u64,
    hot_until_snapshot: u64,
    last_cold_snapshot: u64,
    last_cold_rel_path: Option<PathBuf>,
    last_value_hash: [u8; 32],
    value_size_bytes: usize,
}

impl EntryMetadata {
    fn new(snapshot_idx: u64, value_hash: [u8; 32], value_size_bytes: usize) -> Self {
        Self {
            last_present_snapshot: snapshot_idx,
            last_mutated_snapshot: snapshot_idx,
            last_hot_snapshot: 0,
            hot_until_snapshot: 0,
            last_cold_snapshot: 0,
            last_cold_rel_path: None,
            last_value_hash: value_hash,
            value_size_bytes,
        }
    }
}

impl PartialEq for EntryMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.last_present_snapshot == other.last_present_snapshot
            && self.last_mutated_snapshot == other.last_mutated_snapshot
            && self.last_hot_snapshot == other.last_hot_snapshot
            && self.hot_until_snapshot == other.hot_until_snapshot
            && self.last_cold_snapshot == other.last_cold_snapshot
            && self.last_cold_rel_path == other.last_cold_rel_path
            && self.last_value_hash == other.last_value_hash
            && self.value_size_bytes == other.value_size_bytes
    }
}

impl Eq for EntryMetadata {}

impl PartialOrd for EntryMetadata {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EntryMetadata {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.last_mutated_snapshot
            .cmp(&other.last_mutated_snapshot)
            .then_with(|| self.last_present_snapshot.cmp(&other.last_present_snapshot))
            .then_with(|| self.last_hot_snapshot.cmp(&other.last_hot_snapshot))
            .then_with(|| self.hot_until_snapshot.cmp(&other.hot_until_snapshot))
            .then_with(|| other.value_size_bytes.cmp(&self.value_size_bytes))
    }
}

#[derive(Debug, Clone)]
struct EntryKey {
    key: TieredKeyHandle,
    key_encoded: Vec<u8>,
}

impl EntryKey {
    fn score(&self, id: TieredEntryId) -> EntryScore {
        EntryScore {
            id,
            segment: id.segment,
            key: self.key.clone(),
            key_encoded: self.key_encoded.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct EntryScore {
    id: TieredEntryId,
    segment: TieredSegment,
    key: TieredKeyHandle,
    key_encoded: Vec<u8>,
}

impl EntryScore {
    fn manifest_entry(
        &self,
        meta: &EntryMetadata,
        spill: Option<(PathBuf, u64)>,
    ) -> TieredManifestEntry {
        let (spill_path, spill_bytes) = match spill {
            Some((path, bytes)) => (Some(path), Some(bytes)),
            None => (None, None),
        };
        TieredManifestEntry {
            segment: self.segment,
            key_hash_hex: self.id.key_hash.encode_hex::<String>(),
            key_payload: self.key_encoded.clone(),
            value_size_bytes: meta.value_size_bytes,
            last_present_snapshot: meta.last_present_snapshot,
            last_mutated_snapshot: meta.last_mutated_snapshot,
            value_hash_hex: meta.last_value_hash.encode_hex::<String>(),
            spill_path,
            spill_bytes,
        }
    }

    fn relative_payload_path(&self, snapshot_idx: u64) -> PathBuf {
        let mut path = PathBuf::from(self.segment.dir_name());
        path.push(format!(
            "{}-{}.bin",
            snapshot_idx,
            self.id.key_hash.encode_hex::<String>()
        ));
        path
    }

    fn encode_value(&self, world: &World) -> Result<Vec<u8>> {
        self.key.encode_value(world)
    }
}

/// Key handle for tiered snapshot entries.
#[derive(Debug, Clone)]
pub(crate) enum TieredKeyHandle {
    Domain(iroha_data_model::domain::DomainId),
    Account(iroha_data_model::account::AccountId),
    AccountRekey(iroha_data_model::account::rekey::AccountLabel),
    AssetDefinition(iroha_data_model::asset::AssetDefinitionId),
    Asset(iroha_data_model::asset::AssetId),
    AssetMetadata(iroha_data_model::asset::AssetId),
    Nft(iroha_data_model::nft::NftId),
    Role(iroha_data_model::role::RoleId),
    AccountPermission(iroha_data_model::account::AccountId),
    AccountRole(crate::role::RoleIdWithOwner),
    TxSequence(iroha_data_model::account::AccountId),
    VerifyingKey(iroha_data_model::proof::VerifyingKeyId),
    RuntimeUpgrade(iroha_data_model::runtime::RuntimeUpgradeId),
    Proof(iroha_data_model::proof::ProofId),
    ProofTag(iroha_data_model::proof::ProofId),
    ProofByTag([u8; 4]),
    CommitQc(iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>),
    ContractManifest(iroha_crypto::Hash),
    ContractCode(iroha_crypto::Hash),
    ContractInstance((String, String)),
    SmartContractState(Name),
    ZkAsset(iroha_data_model::asset::AssetDefinitionId),
    Election(String),
    GovernanceProposal([u8; 32]),
    GovernanceReferendum(String),
    GovernanceLock(String),
    GovernanceSlash(String),
    Council(u64),
    ParliamentBodies(u64),
    OfflineAllowance(iroha_crypto::Hash),
    OfflineVerdictRevocation(iroha_crypto::Hash),
    OfflineTransfer(iroha_crypto::Hash),
    OfflineConsumedBuildClaimId(iroha_crypto::Hash),
}

impl TieredKeyHandle {
    fn segment(&self) -> TieredSegment {
        match self {
            TieredKeyHandle::Domain(_) => TieredSegment::Domains,
            TieredKeyHandle::Account(_) => TieredSegment::Accounts,
            TieredKeyHandle::AccountRekey(_) => TieredSegment::AccountRekeyRecords,
            TieredKeyHandle::AssetDefinition(_) => TieredSegment::AssetDefinitions,
            TieredKeyHandle::Asset(_) => TieredSegment::Assets,
            TieredKeyHandle::AssetMetadata(_) => TieredSegment::AssetMetadata,
            TieredKeyHandle::Nft(_) => TieredSegment::Nfts,
            TieredKeyHandle::Role(_) => TieredSegment::Roles,
            TieredKeyHandle::AccountPermission(_) => TieredSegment::AccountPermissions,
            TieredKeyHandle::AccountRole(_) => TieredSegment::AccountRoles,
            TieredKeyHandle::TxSequence(_) => TieredSegment::TxSequences,
            TieredKeyHandle::VerifyingKey(_) => TieredSegment::VerifyingKeys,
            TieredKeyHandle::RuntimeUpgrade(_) => TieredSegment::RuntimeUpgrades,
            TieredKeyHandle::Proof(_) => TieredSegment::Proofs,
            TieredKeyHandle::ProofTag(_) => TieredSegment::ProofTags,
            TieredKeyHandle::ProofByTag(_) => TieredSegment::ProofsByTag,
            TieredKeyHandle::CommitQc(_) => TieredSegment::CommitQcs,
            TieredKeyHandle::ContractManifest(_) => TieredSegment::ContractManifests,
            TieredKeyHandle::ContractCode(_) => TieredSegment::ContractCode,
            TieredKeyHandle::ContractInstance(_) => TieredSegment::ContractInstances,
            TieredKeyHandle::SmartContractState(_) => TieredSegment::SmartContractState,
            TieredKeyHandle::ZkAsset(_) => TieredSegment::ZkAssets,
            TieredKeyHandle::Election(_) => TieredSegment::Elections,
            TieredKeyHandle::GovernanceProposal(_) => TieredSegment::GovernanceProposals,
            TieredKeyHandle::GovernanceReferendum(_) => TieredSegment::GovernanceReferenda,
            TieredKeyHandle::GovernanceLock(_) => TieredSegment::GovernanceLocks,
            TieredKeyHandle::GovernanceSlash(_) => TieredSegment::GovernanceSlashes,
            TieredKeyHandle::Council(_) => TieredSegment::Council,
            TieredKeyHandle::ParliamentBodies(_) => TieredSegment::ParliamentBodies,
            TieredKeyHandle::OfflineAllowance(_) => TieredSegment::OfflineAllowances,
            TieredKeyHandle::OfflineVerdictRevocation(_) => {
                TieredSegment::OfflineVerdictRevocations
            }
            TieredKeyHandle::OfflineTransfer(_) => TieredSegment::OfflineTransfers,
            TieredKeyHandle::OfflineConsumedBuildClaimId(_) => {
                TieredSegment::OfflineConsumedBuildClaimIds
            }
        }
    }

    fn encode_key(&self) -> Result<Vec<u8>> {
        match self {
            TieredKeyHandle::ContractInstance(key) => {
                json::to_vec(&vec![key.0.clone(), key.1.clone()])
                    .wrap_err("failed to encode contract instance key for tiered snapshot")
            }
            TieredKeyHandle::Domain(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Account(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::AccountRekey(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::AssetDefinition(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Asset(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::AssetMetadata(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Nft(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Role(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::AccountPermission(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::AccountRole(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::TxSequence(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::VerifyingKey(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::RuntimeUpgrade(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Proof(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ProofTag(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ProofByTag(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::CommitQc(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ContractManifest(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ContractCode(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::SmartContractState(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ZkAsset(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Election(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::GovernanceProposal(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::GovernanceReferendum(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::GovernanceLock(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::GovernanceSlash(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::Council(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::ParliamentBodies(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::OfflineAllowance(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::OfflineVerdictRevocation(key) => {
                Ok(norito::codec::Encode::encode(key))
            }
            TieredKeyHandle::OfflineTransfer(key) => Ok(norito::codec::Encode::encode(key)),
            TieredKeyHandle::OfflineConsumedBuildClaimId(key) => {
                Ok(norito::codec::Encode::encode(key))
            }
        }
    }

    fn entry_id(&self) -> Result<(TieredEntryId, Vec<u8>)> {
        let key_encoded = self.encode_key()?;
        let key_hash = sha256(&key_encoded);
        Ok((TieredEntryId::new(self.segment(), key_hash), key_encoded))
    }

    fn measure_value(&self, world: &World) -> Result<Option<([u8; 32], usize)>> {
        macro_rules! fetch {
            ($storage:expr, $key:expr) => {{
                let view = $storage.view();
                let Some(value) = view.get($key) else {
                    return Ok(None);
                };
                let (value_hash, _json_len) = compute_json_hash(value)
                    .wrap_err("failed to encode value for tiered snapshot")?;
                let value_size_bytes = compute_hot_bytes(value)
                    .wrap_err("failed to compute WSV hot-tier size estimate")?;
                Ok(Some((value_hash, value_size_bytes)))
            }};
        }

        match self {
            TieredKeyHandle::Domain(id) => fetch!(world.domains, id),
            TieredKeyHandle::Account(id) => fetch!(world.accounts, id),
            TieredKeyHandle::AccountRekey(id) => fetch!(world.account_rekey_records, id),
            TieredKeyHandle::AssetDefinition(id) => fetch!(world.asset_definitions, id),
            TieredKeyHandle::Asset(id) => fetch!(world.assets, id),
            TieredKeyHandle::AssetMetadata(id) => fetch!(world.asset_metadata, id),
            TieredKeyHandle::Nft(id) => fetch!(world.nfts, id),
            TieredKeyHandle::Role(id) => fetch!(world.roles, id),
            TieredKeyHandle::AccountPermission(id) => fetch!(world.account_permissions, id),
            TieredKeyHandle::AccountRole(id) => fetch!(world.account_roles, id),
            TieredKeyHandle::TxSequence(id) => fetch!(world.tx_sequences, id),
            TieredKeyHandle::VerifyingKey(id) => fetch!(world.verifying_keys, id),
            TieredKeyHandle::RuntimeUpgrade(id) => fetch!(world.runtime_upgrades, id),
            TieredKeyHandle::Proof(id) => fetch!(world.proofs, id),
            TieredKeyHandle::ProofTag(id) => fetch!(world.proof_tags, id),
            TieredKeyHandle::ProofByTag(tag) => fetch!(world.proofs_by_tag, tag),
            TieredKeyHandle::CommitQc(hash) => fetch!(world.commit_qcs, hash),
            TieredKeyHandle::ContractManifest(hash) => fetch!(world.contract_manifests, hash),
            TieredKeyHandle::ContractCode(hash) => fetch!(world.contract_code, hash),
            TieredKeyHandle::ContractInstance(key) => fetch!(world.contract_instances, key),
            TieredKeyHandle::SmartContractState(key) => fetch!(world.smart_contract_state, key),
            TieredKeyHandle::ZkAsset(id) => fetch!(world.zk_assets, id),
            TieredKeyHandle::Election(id) => fetch!(world.elections, id),
            TieredKeyHandle::GovernanceProposal(id) => fetch!(world.governance_proposals, id),
            TieredKeyHandle::GovernanceReferendum(id) => fetch!(world.governance_referenda, id),
            TieredKeyHandle::GovernanceLock(id) => fetch!(world.governance_locks, id),
            TieredKeyHandle::GovernanceSlash(id) => fetch!(world.governance_slashes, id),
            TieredKeyHandle::Council(id) => fetch!(world.council, id),
            TieredKeyHandle::ParliamentBodies(id) => fetch!(world.parliament_bodies, id),
            TieredKeyHandle::OfflineAllowance(id) => fetch!(world.offline_allowances, id),
            TieredKeyHandle::OfflineVerdictRevocation(id) => {
                fetch!(world.offline_verdict_revocations, id)
            }
            TieredKeyHandle::OfflineTransfer(id) => fetch!(world.offline_to_online_transfers, id),
            TieredKeyHandle::OfflineConsumedBuildClaimId(id) => {
                fetch!(world.offline_consumed_build_claim_ids, id)
            }
        }
    }

    fn encode_value(&self, world: &World) -> Result<Vec<u8>> {
        macro_rules! fetch {
            ($storage:expr, $key:expr) => {{
                let view = $storage.view();
                let Some(value) = view.get($key) else {
                    return Err(eyre::eyre!("tiered-state: missing value for {}", self));
                };
                let bytes = json::to_vec(value).wrap_err_with(|| {
                    format!("tiered-state: failed to encode payload for {}", self)
                })?;
                Ok(bytes)
            }};
        }

        match self {
            TieredKeyHandle::Domain(id) => fetch!(world.domains, id),
            TieredKeyHandle::Account(id) => fetch!(world.accounts, id),
            TieredKeyHandle::AccountRekey(id) => fetch!(world.account_rekey_records, id),
            TieredKeyHandle::AssetDefinition(id) => fetch!(world.asset_definitions, id),
            TieredKeyHandle::Asset(id) => fetch!(world.assets, id),
            TieredKeyHandle::AssetMetadata(id) => fetch!(world.asset_metadata, id),
            TieredKeyHandle::Nft(id) => fetch!(world.nfts, id),
            TieredKeyHandle::Role(id) => fetch!(world.roles, id),
            TieredKeyHandle::AccountPermission(id) => fetch!(world.account_permissions, id),
            TieredKeyHandle::AccountRole(id) => fetch!(world.account_roles, id),
            TieredKeyHandle::TxSequence(id) => fetch!(world.tx_sequences, id),
            TieredKeyHandle::VerifyingKey(id) => fetch!(world.verifying_keys, id),
            TieredKeyHandle::RuntimeUpgrade(id) => fetch!(world.runtime_upgrades, id),
            TieredKeyHandle::Proof(id) => fetch!(world.proofs, id),
            TieredKeyHandle::ProofTag(id) => fetch!(world.proof_tags, id),
            TieredKeyHandle::ProofByTag(tag) => fetch!(world.proofs_by_tag, tag),
            TieredKeyHandle::CommitQc(hash) => fetch!(world.commit_qcs, hash),
            TieredKeyHandle::ContractManifest(hash) => fetch!(world.contract_manifests, hash),
            TieredKeyHandle::ContractCode(hash) => fetch!(world.contract_code, hash),
            TieredKeyHandle::ContractInstance(key) => fetch!(world.contract_instances, key),
            TieredKeyHandle::SmartContractState(key) => fetch!(world.smart_contract_state, key),
            TieredKeyHandle::ZkAsset(id) => fetch!(world.zk_assets, id),
            TieredKeyHandle::Election(id) => fetch!(world.elections, id),
            TieredKeyHandle::GovernanceProposal(id) => fetch!(world.governance_proposals, id),
            TieredKeyHandle::GovernanceReferendum(id) => fetch!(world.governance_referenda, id),
            TieredKeyHandle::GovernanceLock(id) => fetch!(world.governance_locks, id),
            TieredKeyHandle::GovernanceSlash(id) => fetch!(world.governance_slashes, id),
            TieredKeyHandle::Council(id) => fetch!(world.council, id),
            TieredKeyHandle::ParliamentBodies(id) => fetch!(world.parliament_bodies, id),
            TieredKeyHandle::OfflineAllowance(id) => fetch!(world.offline_allowances, id),
            TieredKeyHandle::OfflineVerdictRevocation(id) => {
                fetch!(world.offline_verdict_revocations, id)
            }
            TieredKeyHandle::OfflineTransfer(id) => fetch!(world.offline_to_online_transfers, id),
            TieredKeyHandle::OfflineConsumedBuildClaimId(id) => {
                fetch!(world.offline_consumed_build_claim_ids, id)
            }
        }
    }
}

impl fmt::Display for TieredKeyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TieredKeyHandle::Domain(id) => write!(f, "domain:{id}"),
            TieredKeyHandle::Account(id) => write!(f, "account:{id}"),
            TieredKeyHandle::AccountRekey(id) => write!(f, "account_rekey:{id:?}"),
            TieredKeyHandle::AssetDefinition(id) => write!(f, "asset_definition:{id}"),
            TieredKeyHandle::Asset(id) => write!(f, "asset:{id}"),
            TieredKeyHandle::AssetMetadata(id) => write!(f, "asset_metadata:{id}"),
            TieredKeyHandle::Nft(id) => write!(f, "nft:{id}"),
            TieredKeyHandle::Role(id) => write!(f, "role:{id}"),
            TieredKeyHandle::AccountPermission(id) => write!(f, "account_permission:{id}"),
            TieredKeyHandle::AccountRole(id) => write!(f, "account_role:{id}"),
            TieredKeyHandle::TxSequence(id) => write!(f, "tx_sequence:{id}"),
            TieredKeyHandle::VerifyingKey(id) => write!(f, "verifying_key:{id:?}"),
            TieredKeyHandle::RuntimeUpgrade(id) => write!(f, "runtime_upgrade:{id:?}"),
            TieredKeyHandle::Proof(id) => write!(f, "proof:{id}"),
            TieredKeyHandle::ProofTag(id) => write!(f, "proof_tag:{id}"),
            TieredKeyHandle::ProofByTag(tag) => write!(f, "proofs_by_tag:{tag:?}"),
            TieredKeyHandle::CommitQc(hash) => write!(f, "commit_qc:{hash}"),
            TieredKeyHandle::ContractManifest(hash) => write!(f, "contract_manifest:{hash}"),
            TieredKeyHandle::ContractCode(hash) => write!(f, "contract_code:{hash}"),
            TieredKeyHandle::ContractInstance(key) => write!(f, "contract_instance:{key:?}"),
            TieredKeyHandle::SmartContractState(key) => write!(f, "smart_contract_state:{key}"),
            TieredKeyHandle::ZkAsset(id) => write!(f, "zk_asset:{id}"),
            TieredKeyHandle::Election(id) => write!(f, "election:{id}"),
            TieredKeyHandle::GovernanceProposal(id) => write!(f, "gov_proposal:{id:?}"),
            TieredKeyHandle::GovernanceReferendum(id) => write!(f, "gov_referendum:{id}"),
            TieredKeyHandle::GovernanceLock(id) => write!(f, "gov_lock:{id}"),
            TieredKeyHandle::GovernanceSlash(id) => write!(f, "gov_slash:{id}"),
            TieredKeyHandle::Council(id) => write!(f, "council:{id}"),
            TieredKeyHandle::ParliamentBodies(id) => write!(f, "parliament_bodies:{id}"),
            TieredKeyHandle::OfflineAllowance(id) => write!(f, "offline_allowance:{id}"),
            TieredKeyHandle::OfflineVerdictRevocation(id) => {
                write!(f, "offline_verdict_revocation:{id}")
            }
            TieredKeyHandle::OfflineTransfer(id) => write!(f, "offline_transfer:{id}"),
            TieredKeyHandle::OfflineConsumedBuildClaimId(id) => {
                write!(f, "offline_consumed_build_claim_id:{id}")
            }
        }
    }
}

/// Snapshot manifest describing hot/cold partitioning for a single capture.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct TieredSnapshotManifest {
    /// Monotonic snapshot index.
    pub snapshot_index: u64,
    /// Total number of tracked entries across storages.
    pub total_entries: usize,
    /// Entries retained in the hot tier.
    pub hot_entries: Vec<TieredManifestEntry>,
    /// Entries spilled to disk.
    pub cold_entries: Vec<TieredManifestEntry>,
    /// Total bytes written to the cold tier in the latest snapshot.
    pub cold_bytes_total: u64,
    /// Entries reused from a previous cold snapshot without re-encoding.
    pub cold_reused_entries: usize,
    /// Total bytes reused from a previous cold snapshot.
    pub cold_reused_bytes: u64,
    /// Entries promoted into the hot tier since the previous snapshot.
    pub hot_promotions: usize,
    /// Entries demoted into the cold tier since the previous snapshot.
    pub hot_demotions: usize,
    /// Hot-tier key budget overflow caused by grace retention.
    pub hot_grace_overflow_keys: usize,
    /// Hot-tier byte budget overflow caused by grace retention.
    pub hot_grace_overflow_bytes: u64,
}

/// Per-entry metadata persisted in manifests.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct TieredManifestEntry {
    segment: TieredSegment,
    key_hash_hex: String,
    key_payload: Vec<u8>,
    value_size_bytes: usize,
    last_present_snapshot: u64,
    last_mutated_snapshot: u64,
    value_hash_hex: String,
    spill_path: Option<PathBuf>,
    spill_bytes: Option<u64>,
}

impl TieredManifestEntry {
    /// Returns the deterministic payload size for the entry.
    #[must_use]
    pub fn value_size_bytes(&self) -> usize {
        self.value_size_bytes
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, num::NonZeroU32};

    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    use iroha_config::parameters::actual::LaneConfig as RuntimeLaneConfig;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        account::OpaqueAccountId,
        block::BlockHeader,
        consensus::{Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
        nexus::{LaneCatalog, LaneConfig, LaneId},
        peer::PeerId,
    };
    use nonzero_ext::nonzero;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn streamed_hash_matches_canonical_json() {
        let value: norito::json::Value = norito::json::from_str(
            r#"{
                "domain": "wonderland",
                "accounts": [
                    {"id": "i105-subject-alice", "metadata": {"email": "alice@example.com"}},
                    {"id": "i105-subject-bob", "metadata": {"roles": ["admin", "auditor"]}}
                ],
                "supply": 42,
                "flags": {
                    "enabled": true,
                    "threshold": 0.5
                }
            }"#,
        )
        .expect("valid JSON fixture");
        let (stream_hash, stream_len) =
            compute_json_hash(&value).expect("streamed hash computation");
        let encoded = norito::json::to_vec(&value).expect("direct encode");
        assert_eq!(stream_len, encoded.len());
        assert_eq!(stream_hash, sha256(&encoded));
    }

    #[test]
    fn hot_bytes_account_for_vec_capacity() {
        let mut value = Vec::with_capacity(12);
        value.extend_from_slice(&[1_u8, 2, 3, 4, 5, 6, 7, 8]);
        let expected =
            std::mem::size_of::<Vec<u8>>() + value.capacity() * std::mem::size_of::<u8>();
        let measured = compute_hot_bytes(&value).expect("hot byte measurement");
        assert_eq!(measured, expected);
    }

    #[test]
    fn measured_bytes_track_governance_approval_sizes() {
        use std::collections::BTreeSet;

        let mut approval = crate::state::GovernanceStageApproval {
            epoch: 1,
            approvers: BTreeSet::new(),
            required: 2,
            quorum_bps: 5000,
        };
        let empty_bytes = MeasuredBytes::measured_bytes(&approval);
        let keypair = iroha_crypto::KeyPair::from_seed(
            b"tiered-approval".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        );
        approval
            .approvers
            .insert(iroha_data_model::account::AccountId::new(
                keypair.public_key().clone(),
            ));
        let filled_bytes = MeasuredBytes::measured_bytes(&approval);
        assert!(filled_bytes >= empty_bytes);

        let mut approvals = crate::state::GovernanceStageApprovals::default();
        let base_bytes = MeasuredBytes::measured_bytes(&approvals);
        approvals.stages.insert(
            iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
            approval,
        );
        let updated_bytes = MeasuredBytes::measured_bytes(&approvals);
        assert!(updated_bytes >= base_bytes);
    }

    #[test]
    fn measured_bytes_cover_trigger_filters() {
        use iroha_data_model::{
            events::{EventFilterBox, data::DataEventFilter},
            trigger::{TriggerId, action::Repeats},
        };

        let trigger_id: TriggerId = "audit_trigger".parse().expect("trigger id");
        let repeats = Repeats::Exactly(1);
        let filter = EventFilterBox::Data(DataEventFilter::Any);

        assert!(MeasuredBytes::measured_bytes(&trigger_id) >= std::mem::size_of::<TriggerId>());
        assert_eq!(
            MeasuredBytes::measured_bytes(&repeats),
            std::mem::size_of::<Repeats>()
        );
        assert!(MeasuredBytes::measured_bytes(&filter) >= std::mem::size_of::<EventFilterBox>());
    }

    #[test]
    fn measured_bytes_cover_opaque_account_id() {
        let opaque = OpaqueAccountId::from_hash(Hash::new([7_u8; 32]));
        assert_eq!(
            MeasuredBytes::measured_bytes(&opaque),
            std::mem::size_of::<OpaqueAccountId>()
        );
    }

    fn dummy_qc(seed: u8) -> Qc {
        let validator_set: Vec<PeerId> = Vec::new();
        Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::new([seed; 4]),
            ),
            parent_state_root: Hash::new([seed.wrapping_add(1); 8]),
            post_state_root: Hash::new([seed; 8]),
            height: u64::from(seed),
            view: u64::from(seed),
            epoch: 0,
            mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![seed],
                bls_aggregate_signature: vec![seed, seed + 1],
            },
        }
    }

    #[test]
    fn snapshot_failure_leaves_existing_snapshot_intact() {
        let temp = tempdir().expect("tmpdir");
        let root = temp.path().to_path_buf();
        let mut backend = TieredStateBackend::new(true, 0, 0, 0, Some(root.clone()), None, 0, 0);

        let existing_dir = root.join(format!("{:020}", 1_u64));
        fs::create_dir_all(&existing_dir).expect("create existing snapshot");
        let marker = existing_dir.join("marker.txt");
        fs::write(&marker, b"keep me").expect("write marker");

        let plan = TieredSnapshotPlan {
            root: root.clone(),
            snapshot_dir: existing_dir.clone(),
            manifest: TieredSnapshotManifest {
                snapshot_index: 1,
                total_entries: 0,
                hot_entries: Vec::new(),
                cold_entries: Vec::new(),
                cold_bytes_total: 0,
                cold_reused_entries: 0,
                cold_reused_bytes: 0,
                hot_promotions: 0,
                hot_demotions: 0,
                hot_grace_overflow_keys: 0,
                hot_grace_overflow_bytes: 0,
            },
            cold_entries: Vec::new(),
        };

        let staging_path = plan.snapshot_dir.with_extension("staging");
        fs::write(&staging_path, b"block staging dir creation").expect("write staging file");

        let result = backend.execute_snapshot_plan(plan, &World::default());
        assert!(
            result.is_err(),
            "expected staging collision to fail snapshot"
        );
        assert!(
            marker.exists(),
            "existing snapshot directory should remain when staging fails"
        );
    }

    #[test]
    fn persists_cold_entries_and_prunes_old_snapshots() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 1, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.total_entries, 2);
        assert_eq!(manifest.cold_entries.len(), 1);
        assert_eq!(manifest.hot_entries.len(), 1);
        assert!(manifest.hot_entries[0].value_size_bytes() > 0);
        assert!(manifest.cold_bytes_total > 0);
        assert!(manifest.cold_entries[0].spill_bytes.is_some());

        let cold_entry = &manifest.cold_entries[0];
        let snapshot_dir = temp
            .path()
            .join(format!("{index:020}", index = manifest.snapshot_index));
        let manifest_path = snapshot_dir.join("manifest.json");
        let manifest_tmp = snapshot_dir.join("manifest.json.tmp");
        assert!(manifest_path.exists(), "manifest should be persisted");
        assert!(
            !manifest_tmp.exists(),
            "manifest temp file should be removed after snapshot"
        );
        let spill_path = cold_entry
            .spill_path
            .as_ref()
            .expect("cold entry has spill path");
        let payload_path = snapshot_dir.join(spill_path);
        assert!(payload_path.exists());
        let encoded = fs::read(&payload_path).expect("payload read");
        let decoded: Qc = json::from_slice(&encoded).expect("cold payload decodes");
        assert!(decoded == qc1 || decoded == qc2);

        // Mutate the other record so the hot entry flips on the next snapshot.
        let mut updated = qc2.clone();
        updated.view = 99;
        world
            .commit_qcs
            .insert(updated.subject_block_hash, updated.clone());

        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");
        let manifest2 = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest2.total_entries, 2);
        assert_eq!(manifest2.cold_entries.len(), 1);
        // Only the latest snapshot directory should be retained (max_snapshots = 1).
        let retained = fs::read_dir(temp.path())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_type().unwrap().is_dir())
            .count();
        assert_eq!(retained, 1);
        assert_eq!(manifest2.snapshot_index, 2);
    }

    #[test]
    fn record_world_snapshot_with_diff_updates_touched_entries() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("initial snapshot");
        let snapshot1 = backend
            .last_manifest()
            .expect("manifest recorded")
            .snapshot_index;

        let mut qc1_updated = qc1.clone();
        qc1_updated.view = qc1_updated.view.saturating_add(1);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1_updated);

        let mut diff = TieredSnapshotDiff::default();
        diff.push(TieredKeyHandle::CommitQc(qc1.subject_block_hash));

        backend
            .record_world_snapshot_with_diff(&world, &diff)
            .expect("diff snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");

        let qc1_key = TieredKeyHandle::CommitQc(qc1.subject_block_hash)
            .encode_key()
            .expect("qc1 key encode");
        let qc2_key = TieredKeyHandle::CommitQc(qc2.subject_block_hash)
            .encode_key()
            .expect("qc2 key encode");

        let mut entries = manifest.hot_entries.iter().chain(&manifest.cold_entries);
        let entry1 = entries
            .clone()
            .find(|entry| entry.key_payload == qc1_key)
            .expect("qc1 entry present");
        let entry2 = entries
            .find(|entry| entry.key_payload == qc2_key)
            .expect("qc2 entry present");

        assert_eq!(entry1.last_mutated_snapshot, manifest.snapshot_index);
        assert_eq!(entry2.last_mutated_snapshot, snapshot1);
    }

    #[test]
    fn record_world_snapshot_with_payload_updates_touched_entries() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("initial snapshot");
        let snapshot1 = backend
            .last_manifest()
            .expect("manifest recorded")
            .snapshot_index;

        let mut qc1_updated = qc1.clone();
        qc1_updated.view = qc1_updated.view.saturating_add(1);
        world
            .commit_qcs
            .insert(qc1.subject_block_hash, qc1_updated.clone());

        let mut payload = TieredSnapshotPayload::default();
        payload.push_value(
            TieredKeyHandle::CommitQc(qc1.subject_block_hash),
            Some(qc1_updated),
        );

        backend
            .record_world_snapshot_with_payload(&payload)
            .expect("payload snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");

        let qc1_key = TieredKeyHandle::CommitQc(qc1.subject_block_hash)
            .encode_key()
            .expect("qc1 key encode");
        let qc2_key = TieredKeyHandle::CommitQc(qc2.subject_block_hash)
            .encode_key()
            .expect("qc2 key encode");

        let mut entries = manifest.hot_entries.iter().chain(&manifest.cold_entries);
        let entry1 = entries
            .clone()
            .find(|entry| entry.key_payload == qc1_key)
            .expect("qc1 entry present");
        let entry2 = entries
            .find(|entry| entry.key_payload == qc2_key)
            .expect("qc2 entry present");

        assert_eq!(entry1.last_mutated_snapshot, manifest.snapshot_index);
        assert_eq!(entry2.last_mutated_snapshot, snapshot1);
    }

    #[test]
    fn record_world_snapshot_with_payload_keeps_unspillable_entries_hot() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());

        backend
            .record_world_snapshot(&world)
            .expect("initial snapshot");

        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        let mut payload = TieredSnapshotPayload::default();
        payload.push_value(TieredKeyHandle::CommitQc(qc2.subject_block_hash), Some(qc2));

        backend
            .record_world_snapshot_with_payload(&payload)
            .expect("payload snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");

        assert_eq!(manifest.hot_entries.len(), 2);
        assert!(manifest.cold_entries.is_empty());
    }

    #[test]
    fn da_store_root_used_when_cold_root_missing() {
        let temp = tempdir().expect("tmpdir");
        let da_root = temp.path().join("da");
        let mut backend = TieredStateBackend::new(true, 1, 0, 0, None, Some(da_root.clone()), 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.total_entries, 2);
        assert_eq!(manifest.cold_entries.len(), 1);

        let snapshot_dir = da_root.join(format!("{:020}", manifest.snapshot_index));
        assert!(snapshot_dir.exists(), "snapshot directory should exist");

        let cold_entry = &manifest.cold_entries[0];
        let spill_path = cold_entry
            .spill_path
            .as_ref()
            .expect("cold entry has spill path");
        let payload_path = snapshot_dir.join(spill_path);
        assert!(
            payload_path.exists(),
            "cold payload should be stored under da root"
        );

        let encoded = backend
            .read_cold_payload(manifest.snapshot_index, cold_entry)
            .expect("read cold payload")
            .expect("payload present");
        let decoded: Qc = json::from_slice(&encoded).expect("cold payload decodes");
        assert!(decoded == qc1 || decoded == qc2);
    }

    #[test]
    fn cold_snapshots_offload_to_da_root_and_rehydrate_on_read() {
        let temp = tempdir().expect("tmpdir");
        let cold_root = temp.path().join("cold");
        let da_root = temp.path().join("da");
        let mut backend = TieredStateBackend::new(
            true,
            1,
            0,
            0,
            Some(cold_root.clone()),
            Some(da_root.clone()),
            0,
            1,
        );
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let manifest1 = backend.last_manifest().expect("manifest recorded").clone();
        assert!(
            !manifest1.cold_entries.is_empty(),
            "expected cold entries in first snapshot"
        );
        let first_index = manifest1.snapshot_index;

        let mut updated = qc2.clone();
        updated.view = 99;
        world.commit_qcs.insert(updated.subject_block_hash, updated);

        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");
        let second_index = backend
            .last_manifest()
            .expect("manifest recorded")
            .snapshot_index;
        assert_ne!(first_index, second_index);

        let cold_dir = cold_root.join(format!("{first_index:020}"));
        assert!(
            !cold_dir.exists(),
            "old snapshot should be offloaded from cold storage"
        );
        let da_dir = da_root.join(format!("{first_index:020}"));
        assert!(
            da_dir.exists(),
            "offloaded snapshot should exist in DA root"
        );

        let cold_entry = &manifest1.cold_entries[0];
        let spill_path = cold_entry
            .spill_path
            .as_ref()
            .expect("cold entry has spill path");
        let encoded = backend
            .read_cold_payload(first_index, cold_entry)
            .expect("read cold payload")
            .expect("payload present");
        let decoded: Qc = json::from_slice(&encoded).expect("cold payload decodes");
        assert!(decoded == qc1 || decoded == qc2);

        let rehydrated_path = cold_root
            .join(format!("{first_index:020}"))
            .join(spill_path);
        assert!(
            rehydrated_path.exists(),
            "expected rehydrated cold payload in cold root"
        );
    }

    #[test]
    fn da_cold_payload_rehydrates_into_primary_root() {
        let temp = tempdir().expect("tmpdir");
        let cold_root = temp.path().join("cold");
        let da_root = temp.path().join("da");
        let mut backend = TieredStateBackend::new(
            true,
            1,
            0,
            0,
            Some(cold_root.clone()),
            Some(da_root.clone()),
            0,
            0,
        );
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        let qc1_hash = qc1.subject_block_hash;
        let qc2_hash = qc2.subject_block_hash;
        world.commit_qcs.insert(qc1_hash, qc1);
        world.commit_qcs.insert(qc2_hash, qc2);

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        let manifest = backend.last_manifest().expect("manifest recorded");
        let cold_entry = &manifest.cold_entries[0];
        let spill_path = cold_entry
            .spill_path
            .as_ref()
            .expect("cold entry has spill path");

        let snapshot_dir = cold_root.join(format!("{:020}", manifest.snapshot_index));
        let cold_payload_path = snapshot_dir.join(spill_path);
        assert!(cold_payload_path.exists(), "cold payload written");

        let da_snapshot_dir = da_root.join(format!("{:020}", manifest.snapshot_index));
        let da_payload_path = da_snapshot_dir.join(spill_path);
        if let Some(parent) = da_payload_path.parent() {
            fs::create_dir_all(parent).expect("da payload parent created");
        }
        fs::copy(&cold_payload_path, &da_payload_path).expect("cold payload copied to DA root");
        fs::remove_file(&cold_payload_path).expect("cold payload evicted");

        let encoded = backend
            .read_cold_payload(manifest.snapshot_index, cold_entry)
            .expect("read cold payload")
            .expect("payload present");
        let decoded: Qc = json::from_slice(&encoded).expect("cold payload decodes");
        assert!(decoded.subject_block_hash == qc1_hash || decoded.subject_block_hash == qc2_hash);
        assert!(
            cold_payload_path.exists(),
            "cold payload rehydrated into primary root"
        );
    }

    #[test]
    fn hot_byte_budget_demotes_entries_to_cold() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 0, 1, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1);
        world.commit_qcs.insert(qc2.subject_block_hash, qc2);

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert!(!manifest.cold_entries.is_empty());
        assert!(
            manifest.hot_entries.len() < manifest.total_entries,
            "byte budget should force some entries into cold tier"
        );
    }

    #[test]
    fn hot_grace_snapshots_keep_recent_hot_entry() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 1, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.hot_entries.len(), 1);

        let qc1_hash = hex::encode(sha256(&norito::codec::Encode::encode(
            &qc1.subject_block_hash,
        )));
        let qc2_hash = hex::encode(sha256(&norito::codec::Encode::encode(
            &qc2.subject_block_hash,
        )));
        let hot_hash = manifest.hot_entries[0].key_hash_hex.clone();

        assert!(
            hot_hash == qc1_hash || hot_hash == qc2_hash,
            "hot entry should match one of the recent QC hashes"
        );
        let mutate = if hot_hash == qc1_hash { qc2 } else { qc1 };
        let mut updated = mutate.clone();
        updated.view = 99;
        world.commit_qcs.insert(updated.subject_block_hash, updated);

        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");
        let manifest2 = backend.last_manifest().expect("manifest recorded");
        let hot_hash2 = manifest2.hot_entries[0].key_hash_hex.clone();

        assert_eq!(hot_hash2, hot_hash, "hot grace should preserve hot entry");
    }

    #[test]
    fn hot_grace_allows_budget_overflow_for_previous_hot_entries() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 2, 0, 1, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1);
        world.commit_qcs.insert(qc2.subject_block_hash, qc2);

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");

        backend.reconfigure(true, 1, 0, 1, Some(temp.path().to_path_buf()), None, 0, 0);
        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");

        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.hot_entries.len(), 2);
        assert_eq!(manifest.hot_grace_overflow_keys, 1);
    }

    #[test]
    fn tiered_backend_reports_budget_limits_and_cold_bytes() {
        let temp = tempdir().expect("tmpdir");
        let mut backend = TieredStateBackend::new(
            true,
            1,
            256,
            0,
            Some(temp.path().to_path_buf()),
            None,
            0,
            512,
        );

        assert_eq!(backend.hot_retained_bytes(), 256);
        assert_eq!(backend.max_cold_bytes(), 512);

        let mut world = World::default();
        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1);
        world.commit_qcs.insert(qc2.subject_block_hash, qc2);

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        let manifest = backend.last_manifest().expect("manifest recorded");
        let cold_bytes = backend
            .cold_store_bytes()
            .expect("cold store bytes")
            .expect("cold tier enabled");
        assert!(
            cold_bytes >= manifest.cold_bytes_total,
            "cold store usage should cover cold payload bytes"
        );
    }

    #[test]
    fn cold_payload_reuse_is_recorded_for_unchanged_entries() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1);
        world.commit_qcs.insert(qc2.subject_block_hash, qc2);

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.cold_reused_entries, 0);

        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");
        let manifest2 = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest2.cold_reused_entries, 1);
        assert!(manifest2.cold_reused_bytes > 0);
    }

    #[cfg(unix)]
    #[test]
    fn cold_payload_reuse_prefers_hard_links_when_supported() {
        let temp = tempdir().expect("tmpdir");
        let probe_source = temp.path().join("probe_source");
        let probe_dest = temp.path().join("probe_dest");
        fs::write(&probe_source, b"probe").expect("write probe");
        if fs::hard_link(&probe_source, &probe_dest).is_err() {
            return;
        }
        fs::remove_file(&probe_dest).expect("cleanup probe link");

        let source = temp.path().join("source");
        let dest = temp.path().join("dest");
        let payload = b"cold payload";
        fs::write(&source, payload).expect("write source");

        let bytes = TieredStateBackend::try_reuse_cold_payload(&source, &dest)
            .expect("reuse payload")
            .expect("reuse should return bytes");
        assert_eq!(bytes, payload.len() as u64);

        let source_meta = fs::metadata(&source).expect("source metadata");
        let dest_meta = fs::metadata(&dest).expect("dest metadata");
        assert_eq!(source_meta.len(), dest_meta.len());
        assert_eq!(source_meta.nlink(), 2);
        assert_eq!(dest_meta.nlink(), 2);
    }

    #[test]
    fn prune_snapshots_to_cold_byte_budget() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 0, 1);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.commit_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.commit_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let first_index = backend
            .last_manifest()
            .expect("manifest recorded")
            .snapshot_index;

        let mut updated = qc2.clone();
        updated.view = 99;
        world.commit_qcs.insert(updated.subject_block_hash, updated);

        backend
            .record_world_snapshot(&world)
            .expect("second snapshot");
        let second_index = backend
            .last_manifest()
            .expect("manifest recorded")
            .snapshot_index;

        let mut snapshots = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                TieredStateBackend::parse_snapshot_dir_name(&entry.file_name())
            })
            .collect::<Vec<_>>();
        snapshots.sort_unstable();
        assert_eq!(snapshots, vec![second_index]);
        assert_ne!(first_index, second_index);
    }

    #[test]
    fn prune_cold_snapshots_to_bytes_prunes_oldest_first() {
        let temp = tempdir().expect("tmpdir");
        let cold_root = temp.path().join("cold");
        fs::create_dir_all(&cold_root).expect("create cold root");

        for idx in 1..=3u64 {
            let dir = cold_root.join(format!("{idx:020}"));
            fs::create_dir_all(&dir).expect("create snapshot dir");
            fs::write(dir.join("payload.norito"), vec![0u8; 50]).expect("write snapshot payload");
        }

        let backend = TieredStateBackend::new(true, 0, 0, 0, Some(cold_root.clone()), None, 0, 0);
        backend
            .prune_cold_snapshots_to_bytes(80)
            .expect("prune snapshots to budget");

        let mut snapshots = fs::read_dir(&cold_root)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                TieredStateBackend::parse_snapshot_dir_name(&entry.file_name())
            })
            .collect::<Vec<_>>();
        snapshots.sort_unstable();
        assert_eq!(snapshots, vec![3]);
    }

    #[test]
    fn snapshot_counter_seeds_from_existing_dirs() {
        let temp = tempdir().expect("tmpdir");
        let root = temp.path().to_path_buf();
        let snapshot_dir = root.join(format!("{:020}", 7_u64));
        fs::create_dir_all(&snapshot_dir).expect("seed snapshot");
        fs::create_dir_all(root.join("lanes")).expect("lanes dir");
        fs::create_dir_all(root.join("retired")).expect("retired dir");

        let mut backend = TieredStateBackend::new(true, 0, 0, 0, Some(root.clone()), None, 0, 0);
        let world = World::default();

        backend.record_world_snapshot(&world).expect("snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.snapshot_index, 8);
        let new_dir = root.join(format!("{:020}", manifest.snapshot_index));
        assert!(new_dir.exists(), "expected seeded snapshot directory");
    }

    #[test]
    fn snapshot_counter_seeds_from_da_root_dirs() {
        let temp = tempdir().expect("tmpdir");
        let da_root = temp.path().join("da");
        let snapshot_dir = da_root.join(format!("{:020}", 5_u64));
        fs::create_dir_all(&snapshot_dir).expect("seed snapshot");

        let mut backend = TieredStateBackend::new(true, 0, 0, 0, None, Some(da_root.clone()), 0, 0);
        let world = World::default();

        backend.record_world_snapshot(&world).expect("snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.snapshot_index, 6);
        let new_dir = da_root.join(format!("{:020}", manifest.snapshot_index));
        assert!(new_dir.exists(), "expected seeded snapshot directory");
    }

    #[test]
    fn recover_snapshot_artifacts_restores_backups_and_cleans_staging() {
        let temp = tempdir().expect("tmpdir");
        let root = temp.path().to_path_buf();

        let live_dir = root.join(format!("{:020}", 1_u64));
        fs::create_dir_all(&live_dir).expect("live dir");
        let live_marker = live_dir.join("marker.txt");
        fs::write(&live_marker, b"live").expect("live marker");

        let live_backup = live_dir.with_extension("bak");
        fs::create_dir_all(&live_backup).expect("live backup");
        fs::write(live_backup.join("marker.txt"), b"backup").expect("backup marker");
        let live_staging = live_dir.with_extension("staging");
        fs::create_dir_all(&live_staging).expect("live staging");
        fs::write(live_staging.join("marker.txt"), b"staging").expect("staging marker");

        let backup_only = root.join(format!("{:020}.bak", 2_u64));
        fs::create_dir_all(&backup_only).expect("backup only");
        fs::write(backup_only.join("marker.txt"), b"backup only").expect("backup only marker");

        let staging_only = root.join(format!("{:020}.staging", 3_u64));
        fs::create_dir_all(&staging_only).expect("staging only");
        fs::write(staging_only.join("marker.txt"), b"staging only").expect("staging only marker");

        let lanes_root = root.join("lanes");
        fs::create_dir_all(&lanes_root).expect("lanes dir");

        let backend = TieredStateBackend::new(false, 0, 0, 0, None, None, 0, 0);
        backend
            .recover_snapshot_artifacts(&root)
            .expect("recover artifacts");

        assert!(live_dir.exists(), "live dir should remain");
        assert!(live_marker.exists(), "live marker should remain");
        assert!(!live_backup.exists(), "live backup should be cleaned");
        assert!(!live_staging.exists(), "live staging should be cleaned");

        let restored = root.join(format!("{:020}", 2_u64));
        assert!(restored.exists(), "backup should be restored");
        assert!(
            restored.join("marker.txt").exists(),
            "backup contents should be preserved on restore"
        );
        assert!(!backup_only.exists(), "backup directory should be renamed");

        assert!(!staging_only.exists(), "staging-only dir should be removed");
        assert!(lanes_root.exists(), "lanes dir should be preserved");
    }

    #[test]
    fn reconfigure_clears_entries_on_cold_root_change() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        let mut world = World::default();
        let qc = dummy_qc(1);
        world.commit_qcs.insert(qc.subject_block_hash, qc);

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        assert!(!backend.entries.is_empty());

        let new_root = tempdir().expect("tmpdir");
        backend.reconfigure(
            true,
            1,
            0,
            0,
            Some(new_root.path().to_path_buf()),
            None,
            0,
            0,
        );

        assert!(backend.entries.is_empty());
        assert_eq!(backend.snapshot_counter, 0);
        assert!(backend.last_manifest().is_none());
    }

    #[test]
    fn reconfigure_clears_entries_on_da_root_change() {
        let temp = tempdir().expect("tmpdir");
        let da_root = temp.path().join("da");
        let mut backend = TieredStateBackend::new(true, 1, 0, 0, None, Some(da_root), 0, 0);
        let mut world = World::default();
        let qc = dummy_qc(1);
        world.commit_qcs.insert(qc.subject_block_hash, qc);

        backend
            .record_world_snapshot(&world)
            .expect("snapshot recorded");
        assert!(!backend.entries.is_empty());

        let new_root = tempdir().expect("tmpdir");
        backend.reconfigure(
            true,
            1,
            0,
            0,
            None,
            Some(new_root.path().to_path_buf()),
            0,
            0,
        );

        assert!(backend.entries.is_empty());
        assert_eq!(backend.snapshot_counter, 0);
        assert!(backend.last_manifest().is_none());
    }

    #[test]
    fn prune_old_snapshots_ignores_lane_and_retired_dirs() {
        let temp = tempdir().expect("tmpdir");
        let backend =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 1, 0);

        let snapshot1 = temp.path().join(format!("{:020}", 1_u64));
        let snapshot2 = temp.path().join(format!("{:020}", 2_u64));
        fs::create_dir_all(&snapshot1).expect("snapshot1");
        fs::create_dir_all(&snapshot2).expect("snapshot2");

        let lanes_root = temp.path().join("lanes");
        let retired_root = temp.path().join("retired");
        fs::create_dir_all(&lanes_root).expect("lanes dir");
        fs::create_dir_all(&retired_root).expect("retired dir");
        let lanes_marker = lanes_root.join("marker.txt");
        let retired_marker = retired_root.join("marker.txt");
        fs::write(&lanes_marker, b"lanes").expect("lanes marker");
        fs::write(&retired_marker, b"retired").expect("retired marker");

        backend
            .prune_old_snapshots(temp.path())
            .expect("prune snapshots");

        assert!(!snapshot1.exists(), "oldest snapshot should be pruned");
        assert!(snapshot2.exists(), "latest snapshot should remain");
        assert!(lanes_root.exists(), "lanes directory should remain");
        assert!(retired_root.exists(), "retired directory should remain");
        assert!(lanes_marker.exists(), "lanes contents should remain");
        assert!(retired_marker.exists(), "retired contents should remain");
    }

    #[test]
    fn reconcile_lane_geometry_manages_lane_directories() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 1, 0, 0, Some(temp.path().to_path_buf()), None, 4, 0);

        let lane_count = NonZeroU32::new(4).expect("lane count");
        let lane0 = LaneConfig::default();
        let lane1 = LaneConfig {
            id: LaneId::from(1),
            alias: "beta".to_string(),
            ..LaneConfig::default()
        };
        let initial_catalog =
            LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()]).expect("catalog");
        let initial_cfg = RuntimeLaneConfig::from_catalog(&initial_catalog);

        let lane2 = LaneConfig {
            id: LaneId::from(2),
            alias: "gamma".to_string(),
            ..LaneConfig::default()
        };
        let extended_catalog = LaneCatalog::new(
            lane_count,
            vec![lane0.clone(), lane1.clone(), lane2.clone()],
        )
        .expect("catalog");
        let extended_cfg = RuntimeLaneConfig::from_catalog(&extended_catalog);

        backend
            .reconcile_lane_geometry(&initial_cfg, &extended_cfg)
            .expect("lane add reconcile");

        let lanes_root = temp.path().join("lanes");
        let lane2_entry = extended_cfg.entry(LaneId::from(2)).expect("lane 2 entry");
        let lane2_path = lanes_root.join(&lane2_entry.kura_segment);
        assert!(
            lane2_path.exists(),
            "expected lane 2 snapshot directory provisioned"
        );

        backend
            .reconcile_lane_geometry(&extended_cfg, &initial_cfg)
            .expect("lane retire reconcile");

        assert!(
            !lane2_path.exists(),
            "expected lane 2 snapshot directory to be retired"
        );
        let retired_root = temp.path().join("retired").join("lanes");
        let retired_entries: Vec<_> = std::fs::read_dir(&retired_root)
            .expect("retired dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect retired entries");
        assert!(
            !retired_entries.is_empty(),
            "expected retired lane snapshot archive"
        );
    }

    #[test]
    fn lane_snapshot_dirs_relabel_on_alias_change() {
        let temp = tempdir().expect("tmpdir");
        let mut backend =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 1, 0);

        let initial_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                alias: "Alpha Lane".to_string(),
                ..LaneConfig::default()
            }],
        )
        .expect("initial catalog");
        let initial_cfg = RuntimeLaneConfig::from_catalog(&initial_catalog);
        backend
            .reconcile_lane_geometry(&RuntimeLaneConfig::default(), &initial_cfg)
            .expect("provision initial snapshot");

        let old_entry = initial_cfg
            .entry(LaneId::SINGLE)
            .expect("initial lane entry");
        let cold_root = backend
            .cold_store_root
            .clone()
            .expect("cold root configured");
        let lanes_root = cold_root.join("lanes");
        let old_dir = lane_snapshot_dir(&lanes_root, old_entry);
        assert!(
            old_dir.exists(),
            "expected snapshot directory for initial alias"
        );

        let updated_catalog = LaneCatalog::new(
            nonzero!(1_u32),
            vec![LaneConfig {
                alias: "Payments Lane".to_string(),
                ..LaneConfig::default()
            }],
        )
        .expect("updated catalog");
        let updated_cfg = RuntimeLaneConfig::from_catalog(&updated_catalog);
        let new_entry = updated_cfg
            .entry(LaneId::SINGLE)
            .expect("updated lane entry");

        backend
            .relabel_lane_geometry(&[(old_entry, new_entry)])
            .expect("relabel snapshot directories");

        let new_dir = lane_snapshot_dir(&lanes_root, new_entry);
        assert!(
            new_dir.exists(),
            "expected snapshot directory with updated alias"
        );
        assert!(
            !old_dir.exists(),
            "old snapshot directory should be moved away"
        );
    }
}
