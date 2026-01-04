//! Tiered storage backend for the World State View (WSV).
//!
//! The backend promotes frequently updated keys to a hot in-memory tier while
//! demoting colder entries to an on-disk spill. Each snapshot computes
//! recency-based priorities, writes cold payloads using the canonical Norito
//! encoding, and emits a manifest so hosts can hydrate cold shards lazily.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result};
use hex::ToHex as _;
use iroha_config::parameters::actual::{LaneConfig, LaneConfigEntry};
use iroha_data_model::prelude::Name;
use itoa::Buffer as ItoaBuffer;
use mv::storage::StorageReadOnly;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use sha2::{Digest as _, Sha256};

use super::World;

/// Lightweight handle describing a hot/cold storage split.
#[derive(Debug, Clone, Default)]
pub struct TieredStateBackend {
    /// Enable tiering; when false snapshots are skipped regardless of other knobs.
    enabled: bool,
    /// Maximum number of keys to keep hot (0 = unlimited).
    hot_retained_keys: usize,
    /// Optional on-disk spill root for cold shards.
    cold_store_root: Option<PathBuf>,
    /// Number of snapshot directories to retain (0 = keep all).
    max_snapshots: usize,
    /// Monotonically increasing snapshot counter.
    snapshot_counter: u64,
    /// Per-entry metadata tracking heat and payload hashes.
    entries: BTreeMap<TieredEntryId, EntryMetadata>,
    /// Cached manifest of the latest snapshot for diagnostics.
    last_manifest: Option<TieredSnapshotManifest>,
}

#[derive(Debug)]
struct ColdEntryPlan {
    rel_path: PathBuf,
    entry: EntryScore,
    manifest_index: usize,
}

#[derive(Debug)]
struct TieredSnapshotPlan {
    root: PathBuf,
    snapshot_dir: PathBuf,
    manifest: TieredSnapshotManifest,
    cold_entries: Vec<ColdEntryPlan>,
}

struct CollectContext<'a> {
    snapshot_idx: u64,
    scores: &'a mut Vec<EntryScore>,
    seen: &'a mut BTreeSet<TieredEntryId>,
}

impl TieredStateBackend {
    /// Construct a backend with explicit limits.
    #[must_use]
    pub fn new(
        enabled: bool,
        hot_retained_keys: usize,
        cold_store_root: Option<PathBuf>,
        max_snapshots: usize,
    ) -> Self {
        let backend = Self {
            enabled,
            hot_retained_keys,
            cold_store_root,
            max_snapshots,
            snapshot_counter: 0,
            entries: BTreeMap::new(),
            last_manifest: None,
        };
        if backend.enabled {
            backend.ensure_cold_root().ok();
        }
        backend
    }

    /// Record a snapshot of the current world.
    pub fn record_world_snapshot(&mut self, world: &World) -> Result<()> {
        if let Some(plan) = self.plan_world_snapshot(world)? {
            self.execute_snapshot_plan(plan, world)?;
        }
        Ok(())
    }

    fn plan_world_snapshot(&mut self, world: &World) -> Result<Option<TieredSnapshotPlan>> {
        if !self.enabled {
            return Ok(None);
        }

        if self.hot_retained_keys == 0 && self.cold_store_root.is_none() {
            return Ok(None);
        }

        let Some(root) = self.cold_store_root.clone() else {
            if self.hot_retained_keys > 0 {
                iroha_logger::warn!(
                    "tiered-state: hot tier limit set but cold_store_root missing; skipping snapshot"
                );
            }
            return Ok(None);
        };

        self.snapshot_counter = self.snapshot_counter.saturating_add(1);
        let snapshot_idx = self.snapshot_counter;
        let snapshot_dir = root.join(format!("{snapshot_idx:020}"));

        let mut scores = Vec::new();
        let mut seen = BTreeSet::new();

        self.collect_world_entries(world, snapshot_idx, &mut scores, &mut seen)?;
        self.entries.retain(|id, _| seen.contains(id));

        if scores.is_empty() {
            let manifest = TieredSnapshotManifest {
                snapshot_index: snapshot_idx,
                total_entries: 0,
                hot_entries: Vec::new(),
                cold_entries: Vec::new(),
                cold_bytes_total: 0,
            };
            return Ok(Some(TieredSnapshotPlan {
                root,
                snapshot_dir,
                manifest,
                cold_entries: Vec::new(),
            }));
        }

        scores.sort_by(|a, b| {
            let meta_a = self.entries.get(&a.id).expect("metadata populated");
            let meta_b = self.entries.get(&b.id).expect("metadata populated");
            meta_b.cmp(meta_a).then_with(|| a.id.cmp(&b.id))
        });

        let hot_limit = if self.hot_retained_keys == 0 {
            scores.len()
        } else {
            self.hot_retained_keys.min(scores.len())
        };

        let (hot_scores, cold_scores) = scores.split_at(hot_limit);
        let mut hot_manifest_entries = Vec::with_capacity(hot_scores.len());
        let mut cold_manifest_entries = Vec::with_capacity(cold_scores.len());
        let mut cold_plans = Vec::with_capacity(cold_scores.len());

        for entry in hot_scores {
            let meta = self.entries.get(&entry.id).expect("metadata populated");
            hot_manifest_entries.push(entry.manifest_entry(meta, None));
        }

        for entry in cold_scores {
            let meta = self.entries.get(&entry.id).expect("metadata populated");
            let rel_path = entry.relative_payload_path(snapshot_idx);
            let manifest_index = cold_manifest_entries.len();
            let mut manifest_entry = entry.manifest_entry(meta, Some((rel_path.clone(), 0)));
            manifest_entry.spill_bytes = None;
            cold_manifest_entries.push(manifest_entry);
            cold_plans.push(ColdEntryPlan {
                rel_path,
                entry: entry.clone(),
                manifest_index,
            });
        }

        let manifest = TieredSnapshotManifest {
            snapshot_index: snapshot_idx,
            total_entries: scores.len(),
            hot_entries: hot_manifest_entries,
            cold_entries: cold_manifest_entries,
            cold_bytes_total: 0,
        };

        Ok(Some(TieredSnapshotPlan {
            root,
            snapshot_dir,
            manifest,
            cold_entries: cold_plans,
        }))
    }

    #[allow(clippy::too_many_lines)]
    fn execute_snapshot_plan(&mut self, mut plan: TieredSnapshotPlan, world: &World) -> Result<()> {
        self.ensure_cold_root()
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
        for cold in &plan.cold_entries {
            let abs_path = staging_dir.join(&cold.rel_path);
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to create cold shard directory {dir}",
                        dir = parent.display()
                    )
                })?;
            }
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

            let payload_len = payload.len() as u64;
            cold_bytes_total = cold_bytes_total.saturating_add(payload_len);
            if let Some(entry) = plan.manifest.cold_entries.get_mut(cold.manifest_index) {
                entry.spill_bytes = Some(payload_len);
            }
        }

        plan.manifest.cold_bytes_total = cold_bytes_total;

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

        Ok(())
    }

    /// Returns the currently configured hot key retention limit.
    #[must_use]
    pub fn hot_retained_keys(&self) -> usize {
        self.hot_retained_keys
    }

    /// Returns true when tiering is enabled and a cold tier is configured.
    #[must_use]
    pub fn is_cold_tier_enabled(&self) -> bool {
        self.enabled && self.cold_store_root.is_some()
    }

    /// Returns whether tiering is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the cached manifest of the latest snapshot, if any.
    #[must_use]
    pub fn last_manifest(&self) -> Option<&TieredSnapshotManifest> {
        self.last_manifest.as_ref()
    }

    /// Update configuration knobs at runtime.
    pub fn reconfigure(
        &mut self,
        enabled: bool,
        hot_retained_keys: usize,
        cold_store_root: Option<PathBuf>,
        max_snapshots: usize,
    ) {
        let cold_root_changed = self.cold_store_root != cold_store_root;
        self.enabled = enabled;
        self.hot_retained_keys = hot_retained_keys;
        self.cold_store_root = cold_store_root;
        self.max_snapshots = max_snapshots;
        if !self.enabled {
            return;
        }
        if cold_root_changed {
            self.snapshot_counter = 0;
            self.last_manifest = None;
            if let Err(err) = self.ensure_cold_root() {
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

        let Some(root) = self.cold_store_root.clone() else {
            return Ok(());
        };
        self.ensure_cold_root()?;

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
    pub fn relabel_lane_geometry(
        &mut self,
        migrations: &[(&LaneConfigEntry, &LaneConfigEntry)],
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let Some(root) = self.cold_store_root.clone() else {
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
            }
            fs::rename(&old_dir, &new_dir).wrap_err_with(|| {
                format!(
                    "failed to relabel lane snapshot dir from {src} to {dst}",
                    src = old_dir.display(),
                    dst = new_dir.display()
                )
            })?;
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

    fn ensure_cold_root(&self) -> Result<()> {
        if let Some(root) = &self.cold_store_root {
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
        collect_map!(TieredSegment::ExecRoots, ExecRoot, world.exec_roots);
        collect_map!(TieredSegment::ExecQcs, ExecQc, world.exec_qcs);
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
        V: json::JsonSerialize,
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
        V: json::JsonSerialize,
    {
        let key_hash = sha256(&key_encoded);
        let id = TieredEntryId::new(segment, key_hash);

        let (value_hash, approx_size) =
            compute_json_hash(value).wrap_err("failed to encode value for tiered snapshot")?;

        let meta = self
            .entries
            .entry(id)
            .or_insert_with(|| EntryMetadata::new(ctx.snapshot_idx, value_hash, approx_size));
        if meta.last_value_hash != value_hash {
            meta.last_value_hash = value_hash;
            meta.last_mutated_snapshot = ctx.snapshot_idx;
        }
        meta.last_present_snapshot = ctx.snapshot_idx;
        meta.approx_size = approx_size;
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
        fs::write(&manifest_path, manifest_bytes).wrap_err_with(|| {
            format!(
                "failed to persist manifest {path}",
                path = manifest_path.display()
            )
        })
    }

    fn prune_old_snapshots(&self, root: &Path) -> Result<()> {
        if self.max_snapshots == 0 {
            return Ok(());
        }
        fn parse_snapshot_dir_name(name: &std::ffi::OsStr) -> Option<u64> {
            let name = name.to_str()?;
            if name.len() != 20 || !name.as_bytes().iter().all(|b| b.is_ascii_digit()) {
                return None;
            }
            name.parse::<u64>().ok()
        }

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
                    .filter(|ft| ft.is_dir())
                    .and_then(|_| parse_snapshot_dir_name(&entry.file_name()))
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
            }
            entries.remove(0);
        }
        Ok(())
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
    let json_value = json::to_value(value)
        .wrap_err("failed to convert snapshot value into Norito JSON value")?;
    let mut state = JsonStreamHasher::default();
    stream_json_value(&json_value, &mut state);
    Ok(state.finalize())
}

#[derive(Default)]
struct JsonStreamHasher {
    hasher: Sha256,
    len: usize,
}

impl JsonStreamHasher {
    fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.hasher.update(bytes);
        self.len += bytes.len();
    }

    fn finalize(self) -> ([u8; 32], usize) {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.hasher.finalize());
        (out, self.len)
    }
}

fn stream_json_value(value: &json::Value, state: &mut JsonStreamHasher) {
    match value {
        json::Value::Null => state.write(b"null"),
        json::Value::Bool(true) => state.write(b"true"),
        json::Value::Bool(false) => state.write(b"false"),
        json::Value::Number(number) => write_number_bytes(number, state),
        json::Value::String(s) => write_json_string_bytes(s, state),
        json::Value::Array(items) => {
            state.write(b"[");
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    state.write(b",");
                }
                stream_json_value(item, state);
            }
            state.write(b"]");
        }
        json::Value::Object(map) => {
            state.write(b"{");
            for (idx, (key, value)) in map.iter().enumerate() {
                if idx > 0 {
                    state.write(b",");
                }
                write_json_string_bytes(key, state);
                state.write(b":");
                stream_json_value(value, state);
            }
            state.write(b"}");
        }
    }
}

fn write_number_bytes(number: &json::Number, state: &mut JsonStreamHasher) {
    match number {
        json::Number::I64(value) => {
            let mut buf = ItoaBuffer::new();
            state.write(buf.format(*value).as_bytes());
        }
        json::Number::U64(value) => {
            let mut buf = ItoaBuffer::new();
            state.write(buf.format(*value).as_bytes());
        }
        json::Number::F64(value) => {
            const F64_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
            let rendered =
                if value.is_finite() && value.fract() == 0.0 && value.abs() <= F64_SAFE_INT {
                    format!("{value:.1}")
                } else {
                    format!("{value:?}")
                };
            state.write(rendered.as_bytes());
        }
    }
}

fn write_json_string_bytes(value: &str, state: &mut JsonStreamHasher) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    state.write(b"\"");
    let bytes = value.as_bytes();
    let mut start = 0usize;
    for (idx, &byte) in bytes.iter().enumerate() {
        let escape = match byte {
            b'"' => Some(br#"\""#.as_ref()),
            b'\\' => Some(br"\\".as_ref()),
            b'\n' => Some(br"\n".as_ref()),
            b'\r' => Some(br"\r".as_ref()),
            b'\t' => Some(br"\t".as_ref()),
            0x08 => Some(br"\b".as_ref()),
            0x0C => Some(br"\f".as_ref()),
            _ => None,
        };
        if let Some(seq) = escape {
            if start < idx {
                state.write(&bytes[start..idx]);
            }
            state.write(seq);
            start = idx + 1;
        } else if byte < 0x20 {
            if start < idx {
                state.write(&bytes[start..idx]);
            }
            let mut buf = [b'\\', b'u', b'0', b'0', 0, 0];
            buf[4] = HEX[(byte >> 4) as usize];
            buf[5] = HEX[(byte & 0x0F) as usize];
            state.write(&buf);
            start = idx + 1;
        }
    }
    if start < bytes.len() {
        state.write(&bytes[start..]);
    }
    state.write(b"\"");
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
    ExecRoots,
    ExecQcs,
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
            TieredSegment::ExecRoots => "exec_roots",
            TieredSegment::ExecQcs => "exec_qcs",
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
            "exec_roots" => TieredSegment::ExecRoots,
            "exec_qcs" => TieredSegment::ExecQcs,
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
    last_value_hash: [u8; 32],
    approx_size: usize,
}

impl EntryMetadata {
    fn new(snapshot_idx: u64, value_hash: [u8; 32], approx_size: usize) -> Self {
        Self {
            last_present_snapshot: snapshot_idx,
            last_mutated_snapshot: snapshot_idx,
            last_value_hash: value_hash,
            approx_size,
        }
    }
}

impl PartialEq for EntryMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.last_present_snapshot == other.last_present_snapshot
            && self.last_mutated_snapshot == other.last_mutated_snapshot
            && self.last_value_hash == other.last_value_hash
            && self.approx_size == other.approx_size
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
            .then_with(|| other.approx_size.cmp(&self.approx_size))
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
            approx_value_size: meta.approx_size,
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

#[derive(Debug, Clone)]
enum TieredKeyHandle {
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
    ExecRoot(iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>),
    ExecQc(iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>),
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
}

impl TieredKeyHandle {
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
            TieredKeyHandle::ExecRoot(hash) => fetch!(world.exec_roots, hash),
            TieredKeyHandle::ExecQc(hash) => fetch!(world.exec_qcs, hash),
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
            TieredKeyHandle::ExecRoot(hash) => write!(f, "exec_root:{hash}"),
            TieredKeyHandle::ExecQc(hash) => write!(f, "exec_qc:{hash}"),
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
}

/// Per-entry metadata persisted in manifests.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct TieredManifestEntry {
    segment: TieredSegment,
    key_hash_hex: String,
    key_payload: Vec<u8>,
    approx_value_size: usize,
    last_present_snapshot: u64,
    last_mutated_snapshot: u64,
    value_hash_hex: String,
    spill_path: Option<PathBuf>,
    spill_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use iroha_config::parameters::actual::LaneConfig as RuntimeLaneConfig;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        block::BlockHeader,
        consensus::ExecutionQcRecord,
        nexus::{LaneCatalog, LaneConfig, LaneId},
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
                    {"id": "alice@wonderland", "metadata": {"email": "alice@example.com"}},
                    {"id": "bob@wonderland", "metadata": {"roles": ["admin", "auditor"]}}
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

    fn dummy_qc(seed: u8) -> ExecutionQcRecord {
        ExecutionQcRecord {
            subject_block_hash: iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::new([seed; 4]),
            ),
            parent_state_root: Hash::new([seed.wrapping_add(1); 8]),
            post_state_root: Hash::new([seed; 8]),
            height: u64::from(seed),
            view: u64::from(seed),
            epoch: 0,
            signers_bitmap: vec![seed],
            bls_aggregate_signature: vec![seed, seed + 1],
        }
    }

    #[test]
    fn snapshot_failure_leaves_existing_snapshot_intact() {
        let temp = tempdir().expect("tmpdir");
        let root = temp.path().to_path_buf();
        let mut backend = TieredStateBackend::new(true, 0, Some(root.clone()), 0);

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
        let mut backend = TieredStateBackend::new(true, 1, Some(temp.path().to_path_buf()), 1);
        let mut world = World::default();

        let qc1 = dummy_qc(1);
        let qc2 = dummy_qc(2);
        world.exec_qcs.insert(qc1.subject_block_hash, qc1.clone());
        world.exec_qcs.insert(qc2.subject_block_hash, qc2.clone());

        backend
            .record_world_snapshot(&world)
            .expect("first snapshot");
        let manifest = backend.last_manifest().expect("manifest recorded");
        assert_eq!(manifest.total_entries, 2);
        assert_eq!(manifest.cold_entries.len(), 1);
        assert_eq!(manifest.hot_entries.len(), 1);
        assert!(manifest.cold_bytes_total > 0);
        assert!(manifest.cold_entries[0].spill_bytes.is_some());

        let cold_entry = &manifest.cold_entries[0];
        let snapshot_dir = temp
            .path()
            .join(format!("{index:020}", index = manifest.snapshot_index));
        let spill_path = cold_entry
            .spill_path
            .as_ref()
            .expect("cold entry has spill path");
        let payload_path = snapshot_dir.join(spill_path);
        assert!(payload_path.exists());
        let encoded = fs::read(&payload_path).expect("payload read");
        let decoded: ExecutionQcRecord = json::from_slice(&encoded).expect("cold payload decodes");
        assert!(decoded == qc1 || decoded == qc2);

        // Mutate the other record so the hot entry flips on the next snapshot.
        let mut updated = qc2.clone();
        updated.view = 99;
        world
            .exec_qcs
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
    fn prune_old_snapshots_ignores_lane_and_retired_dirs() {
        let temp = tempdir().expect("tmpdir");
        let backend = TieredStateBackend::new(true, 0, Some(temp.path().to_path_buf()), 1);

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
        let mut backend = TieredStateBackend::new(true, 1, Some(temp.path().to_path_buf()), 4);

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
        let mut backend = TieredStateBackend::new(true, 0, Some(temp.path().to_path_buf()), 1);

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
