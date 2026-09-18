// Exact auxiliary-identity transition for a prevalidated State replay receipt.
// Included in lane_geometry: operation paths are taken from its authenticated journal.

pub(crate) struct ReplayGeometryBindingRequest<'a> {
    pub previous: &'a LaneConfig,
    pub updated: &'a LaneConfig,
    pub previous_incarnations: &'a BTreeMap<LaneId, Hash>,
    pub updated_incarnations: &'a BTreeMap<LaneId, Hash>,
    pub previous_activation_heights: &'a BTreeMap<LaneId, u64>,
    pub updated_activation_heights: &'a BTreeMap<LaneId, u64>,
    pub previous_lineage_root: Hash,
    pub updated_lineage_root: Hash,
    pub transition_height: u64,
}

/// Private pre-publication expectation. It cannot adopt identities observed after a move.
pub(crate) struct StartupReplayGeometryTransition {
    binding: super::V2StartupReplayStorageBinding,
    expected_paths: BTreeMap<PathBuf, super::StableSidecarDirectoryInventory>,
    final_auxiliary: BTreeMap<PathBuf, super::StableSidecarDirectoryInventory>,
    expected_blocks: BTreeMap<PathBuf, Option<GeometryFileIdentity>>,
    missing_namespaces: Vec<StartupReplayMissingNamespace>,
    created_namespaces: Vec<StartupReplayNamespaceCreation>,
}

/// Effect receipt emitted only by the existing native namespace creator.
pub(crate) struct StartupReplayNamespaceCreation {
    blocks_identity: GeometryFileIdentity,
    held: BoundProgressDirectory,
    inventory: super::StableSidecarDirectoryInventory,
}
struct StartupReplayMissingNamespace {
    blocks: PathBuf,
    merge: PathBuf,
    blocks_identity: GeometryFileIdentity,
    binding: LaneGeometryBinding,
    original_marker: LaneIncarnationMarker,
    original_block_digest: Hash,
    original_merge_digest: Hash,
    final_path: PathBuf,
}

/// Exact post-publication identities retained until active-height recovery consumes the plan.
#[derive(Debug)]
pub(crate) struct StartupReplayGeometryPublication {
    expected_paths: BTreeMap<PathBuf, super::StableSidecarDirectoryInventory>,
    active_auxiliary: BTreeMap<PathBuf, super::StableSidecarDirectoryInventory>,
    expected_blocks: BTreeMap<PathBuf, Option<GeometryFileIdentity>>,
}
impl StartupReplayGeometryPublication {
    pub(super) fn active_auxiliary(
        &self,
    ) -> &BTreeMap<PathBuf, super::StableSidecarDirectoryInventory> {
        &self.active_auxiliary
    }
}

impl Kura {
    fn capture_startup_geometry_paths(
        &self,
        blocks_paths: &BTreeSet<PathBuf>,
    ) -> Result<BTreeMap<PathBuf, super::StableSidecarDirectoryInventory>> {
        let mut snapshots = BTreeMap::new();
        let mut records = 0_usize;
        let mut bytes = 0_u64;
        for blocks in blocks_paths {
            let lane = Self::lane_artifact_dir(blocks);
            let historical = lane.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
            let inventory = self.stable_sidecar_directory_inventory_with_recognized_child(
                &lane,
                Some(&historical),
            )?;
            snapshots.insert(lane, inventory);
            let (inventory, count, len) = self
                .stable_historical_autonomous_recovery_directory_inventory(
                    &historical,
                    HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS
                        .checked_sub(records)
                        .ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "startup geometry record bound exceeded",
                            )
                        })?,
                    self.historical_autonomous_recovery_aggregate_byte_limit()
                        .checked_sub(bytes)
                        .ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "startup geometry byte bound exceeded",
                            )
                        })?,
                )?;
            records = records.checked_add(count).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "startup geometry record count overflow",
                )
            })?;
            bytes = bytes.checked_add(len).ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "startup geometry byte count overflow",
                )
            })?;
            snapshots.insert(historical, inventory);
        }
        Ok(snapshots)
    }

    /// Pin the complete old binding and exact retained geometry moves before State publication.
    /// Only State's private, fully prevalidated replay receipt calls this operation.
    pub(crate) fn begin_startup_replay_geometry_transition(
        &self,
        binding: &super::V2StartupReplayStorageBinding,
        requests: &[ReplayGeometryBindingRequest<'_>],
    ) -> Result<StartupReplayGeometryTransition> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        self.validate_v2_startup_replay_storage_binding_unlocked(binding)?;
        let (_, original_auxiliary) = binding.strict_parts().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "emergency Fast cannot publish replay geometry",
            )
        })?;
        let _geometry = self.lane_geometry_lock.lock();
        let journal = self.read_lane_geometry_journal()?;
        // The shared audit covers every journal-retained instance, including
        // inactive and future replay references. Only the captured active map
        // determines whether a missing optional namespace is active corruption.
        let initial_lanes = self.lane_storage_entries.lock().clone();
        let active_lane_paths = initial_lanes
            .values()
            .map(|entry| Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)))
            .collect::<BTreeSet<_>>();
        let mut sources = BTreeMap::new();
        let mut paths = BTreeSet::new();
        let mut prior_updated = None;
        let mut prior_index = None;
        let mut final_lanes = initial_lanes;
        for request in requests {
            let previous = self.geometry_bindings(
                request.previous,
                request.previous_incarnations,
                request.previous_activation_heights,
            )?;
            let updated = self.geometry_bindings(
                request.updated,
                request.updated_incarnations,
                request.updated_activation_heights,
            )?;
            if prior_updated
                .as_ref()
                .is_some_and(|expected| expected != &previous)
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay geometry receipts are not contiguous",
                ));
            }
            if prior_updated.is_none()
                && final_lanes
                    != self.lane_storage_entries_from_geometry(
                        request.previous,
                        request.previous_incarnations,
                        request.previous_activation_heights,
                    )?
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay geometry receipt starts from another lane map",
                ));
            }
            let mut matches = journal.records.iter().enumerate().filter(|(_, record)| {
                record.transition_height == request.transition_height
                    && record.previous_bindings == previous
                    && record.updated_bindings == updated
                    && record.previous_catalog == geometry_catalog_fingerprint(&previous)
                    && record.updated_catalog == geometry_catalog_fingerprint(&updated)
                    && record.previous_lineage_root == request.previous_lineage_root
                    && record.updated_lineage_root == request.updated_lineage_root
            });
            let (index, record) = matches.next().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay geometry receipt has no exact retained journal operation",
                )
            })?;
            if matches.next().is_some() || prior_index.is_some_and(|prior| index != prior + 1) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay geometry journal operations are ambiguous or noncontiguous",
                ));
            }
            for operation in &record.operations {
                for instance in operation.previous.iter().chain(operation.updated.iter()) {
                    let path = self.binding_blocks_path(instance);
                    paths.insert(path.clone());
                    sources
                        .entry(path)
                        .or_insert_with(|| (self.binding_merge_path(instance), instance.clone()));
                }
            }
            prior_updated = Some(updated);
            prior_index = Some(index);
            final_lanes = self.lane_storage_entries_from_geometry(
                request.updated,
                request.updated_incarnations,
                request.updated_activation_heights,
            )?;
        }
        let mut block_identities = paths
            .iter()
            .map(|path| {
                if self.validate_path_kind(path, true)? {
                    self.geometry_path_identity(path, true)
                        .map(|identity| (path.clone(), Some(identity)))
                } else {
                    Ok((path.clone(), None))
                }
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut expected_paths = self.capture_startup_geometry_paths(&paths)?;
        if let super::V2StartupReplayStorageBinding::StrictAfterGeometryPublication {
            publication,
            ..
        } = binding
        {
            for (path, expected) in &publication.expected_paths {
                expected_paths
                    .entry(path.clone())
                    .or_insert_with(|| expected.clone());
            }
            for (path, expected) in &publication.expected_blocks {
                block_identities.entry(path.clone()).or_insert(*expected);
            }
        }
        let mut missing_namespaces = Vec::new();
        for (blocks, (merge, native_binding)) in sources {
            let Some(identity) = block_identities[&blocks] else {
                continue;
            };
            let path = Self::lane_artifact_dir(&blocks);
            if expected_paths[&path].directory.metadata.is_some() {
                continue;
            }
            if active_lane_paths.contains(&path) {
                return Err(self.startup_auxiliary_identity_error(
                    &path,
                    "active namespace absence is not an archived replay creation",
                ));
            }
            self.require_lane_marker_at(&blocks, &native_binding)?;
            let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
            self.require_complete_geometry_binding_at(&native_binding, &blocks, &merge)?;
            if !self.lane_marker_is_unsealed_at(&blocks, &native_binding)? {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay instance is already owned by collection",
                ));
            }
            let original_block_digest = self.geometry_block_store_digest(&blocks)?;
            let original_merge_digest = self.geometry_merge_log_digest(&merge)?;
            missing_namespaces.push(StartupReplayMissingNamespace {
                blocks,
                merge,
                blocks_identity: identity,
                binding: native_binding,
                original_marker: marker,
                original_block_digest,
                original_merge_digest,
                final_path: path,
            });
        }
        for (path, old) in original_auxiliary {
            if let Some(current) = expected_paths.get(path)
                && !Self::stable_sidecar_directory_inventory_unchanged(old, current)
            {
                return Err(self.startup_auxiliary_identity_error(
                    path,
                    "changed before geometry publication",
                ));
            }
            expected_paths.insert(path.clone(), old.clone());
        }
        // Publishing a reference never retires physical evidence. Keep every
        // original retained path pinned; only an exact native creation receipt
        // may replace a captured absence at finish. No post-publication scan
        // can supply a new before-image or forget a retired instance.
        let mut final_auxiliary = original_auxiliary.clone();
        for entry in final_lanes.values() {
            let lane = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
            for path in [
                lane.clone(),
                lane.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1),
            ] {
                let expected = expected_paths.get(&path).ok_or_else(|| {
                    self.startup_auxiliary_identity_error(
                        &path,
                        "new lane lacks a pre-pinned geometry source",
                    )
                })?;
                final_auxiliary.insert(path, expected.clone());
            }
        }
        for missing in &mut missing_namespaces {
            let mut final_blocks = block_identities.iter().filter_map(|(path, identity)| {
                (*identity == Some(missing.blocks_identity)).then_some(path)
            });
            let final_blocks = final_blocks.next().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "missing namespace lost its exact retained block inode",
                )
            })?;
            missing.final_path = Self::lane_artifact_dir(final_blocks);
        }
        // A second check prevents adopting drift while the before-image was captured.
        // The canonical lease remains held; the derived binding validator takes geometry itself.
        drop(_geometry);
        self.validate_v2_startup_replay_storage_binding_unlocked(binding)?;
        Ok(StartupReplayGeometryTransition {
            binding: binding.clone(),
            expected_paths,
            final_auxiliary,
            expected_blocks: block_identities,
            missing_namespaces,
            created_namespaces: Vec::new(),
        })
    }

    /// Consume native geometry effects into the private replay preparation.
    pub(crate) fn apply_startup_replay_geometry_transition(
        &self,
        request: &ReplayGeometryBindingRequest<'_>,
        replaced_lane_ids: &BTreeSet<LaneId>,
        transition: &mut StartupReplayGeometryTransition,
    ) -> Result<()> {
        self.apply_lane_geometry_transition_with_lineage_roots_and_certified_retirements_inner(
            request.previous,
            request.updated,
            request.previous_incarnations,
            request.updated_incarnations,
            request.previous_activation_heights,
            request.updated_activation_heights,
            request.previous_lineage_root,
            request.updated_lineage_root,
            replaced_lane_ids,
            &BTreeSet::new(),
            Some(request.transition_height),
            Some(&mut transition.created_namespaces),
        )
    }

    fn retarget_created_namespace(
        &self,
        receipt: &StartupReplayNamespaceCreation,
        path: &Path,
    ) -> Result<super::StableSidecarDirectoryInventory> {
        let opened = secure_file_metadata::from_file(&receipt.held.file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !Self::sidecar_directory_metadata_unchanged(&receipt.held.metadata, &opened)
            || !receipt.inventory.files.is_empty()
        {
            return Err(self.startup_auxiliary_identity_error(
                path,
                "native created namespace changed after its receipt",
            ));
        }
        let mut expected = receipt.inventory.clone();
        expected.directory.expected_path = path.to_path_buf();
        let canonical_root = self
            .store_root
            .canonicalize()
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        expected.directory.canonical_path = Some(canonical_root.join(
            path.strip_prefix(&self.store_root).map_err(|_| {
                self.geometry_error(ErrorKind::InvalidData, "created namespace escaped Kura")
            })?,
        ));
        let historical = path.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
        let current =
            self.stable_sidecar_directory_inventory_with_recognized_child(path, Some(&historical))?;
        Self::require_startup_auxiliary_identity(&expected, &current)?;
        if self.validate_path_kind(&historical, true)? {
            return Err(self.startup_auxiliary_identity_error(
                path,
                "native created namespace gained a child",
            ));
        }
        Ok(expected)
    }

    /// Geometry rollback restores each original archive first. Remove only the exact
    /// native-created empty inode, then restore the original already-authenticated seal.
    pub(crate) fn rollback_startup_replay_geometry_preparation(
        &self,
        transition: &StartupReplayGeometryTransition,
    ) -> Result<()> {
        let _prune = self.prune_lock.lock();
        let _canonical = self.canonical_chain_lock.lock();
        let _geometry = self.lane_geometry_lock.lock();
        for receipt in transition.created_namespaces.iter().rev() {
            let missing = transition
                .missing_namespaces
                .iter()
                .find(|missing| missing.blocks_identity == receipt.blocks_identity)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "cleanup lacks exact native creation authority",
                    )
                })?;
            self.require_geometry_path_identity(&missing.blocks, true, missing.blocks_identity)?;
            let path = Self::lane_artifact_dir(&missing.blocks);
            self.retarget_created_namespace(receipt, &path)?;
            let current_marker = self.read_lane_marker(&missing.blocks.join(MARKER_FILE_NAME))?;
            self.require_complete_geometry_binding_at(
                &missing.binding,
                &missing.blocks,
                &missing.merge,
            )?;
            if current_marker != missing.original_marker {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "replay instance marker changed during namespace creation",
                ));
            }
            fs::remove_dir(&path).map_err(|error| Error::IO(error, path.clone()))?;
            self.sync_geometry_parent(Some(&missing.blocks))?;
            if self.geometry_block_store_digest(&missing.blocks)? != missing.original_block_digest
                || self.geometry_merge_log_digest(&missing.merge)? != missing.original_merge_digest
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "namespace cleanup did not restore the exact authenticated archive",
                ));
            }
            self.require_geometry_path_identity(&missing.blocks, true, missing.blocks_identity)?;
            self.require_complete_geometry_binding_at(
                &missing.binding,
                &missing.blocks,
                &missing.merge,
            )?;
        }
        Ok(())
    }

    /// Caller holds the final canonical publication lease; every failure precedes WSV install.
    pub(crate) fn finish_startup_replay_geometry_transition(
        &self,
        transition: &StartupReplayGeometryTransition,
    ) -> Result<super::V2StartupReplayStorageBinding> {
        let (original, _) = transition.binding.strict_parts().ok_or_else(|| {
            self.geometry_error(
                ErrorKind::InvalidInput,
                "geometry transition lost its Strict binding",
            )
        })?;
        let mut expected_paths = transition.expected_paths.clone();
        let mut auxiliary = transition.final_auxiliary.clone();
        let mut seen = BTreeSet::new();
        for receipt in &transition.created_namespaces {
            let missing = transition
                .missing_namespaces
                .iter()
                .find(|missing| missing.blocks_identity == receipt.blocks_identity)
                .ok_or_else(|| {
                    self.geometry_error(
                        ErrorKind::InvalidData,
                        "native namespace creation was outside the exact replay receipt",
                    )
                })?;
            if !seen.insert(missing.final_path.clone()) {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "duplicate native namespace creation receipt",
                ));
            }
            let inventory = self.retarget_created_namespace(receipt, &missing.final_path)?;
            expected_paths.insert(missing.final_path.clone(), inventory.clone());
            if auxiliary.contains_key(&missing.final_path) {
                auxiliary.insert(missing.final_path.clone(), inventory);
            }
        }
        let next = super::V2StartupReplayStorageBinding::StrictAfterGeometryPublication {
            inventory: Arc::clone(original),
            publication: Arc::new(StartupReplayGeometryPublication {
                expected_paths,
                active_auxiliary: auxiliary,
                expected_blocks: transition.expected_blocks.clone(),
            }),
        };
        self.validate_v2_startup_replay_storage_binding_unlocked(&next)?;
        // Snapshot/install/clear all take this same inventory-first lock order.
        // No session can pair a publication with an independently replaced audit.
        let installed = self.v2_startup_finality_verification_inventory.lock();
        if !installed
            .as_ref()
            .is_some_and(|installed| Arc::ptr_eq(installed, original))
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "startup geometry publication lost its original installed audit",
            ));
        }
        let mut publication = self.v2_startup_replay_geometry_publication.lock();
        let super::V2StartupReplayStorageBinding::StrictAfterGeometryPublication {
            publication: next_publication,
            ..
        } = &next
        else {
            unreachable!("native geometry minted a derived binding")
        };
        *publication = Some(Arc::clone(next_publication));
        Ok(next)
    }
    pub(super) fn validate_startup_geometry_publication(
        &self,
        publication: &StartupReplayGeometryPublication,
    ) -> Result<()> {
        let _geometry = self.lane_geometry_lock.lock();
        for (path, expected) in &publication.expected_blocks {
            match expected {
                Some(identity) => self.require_geometry_path_identity(path, true, *identity)?,
                None if self.validate_path_kind(path, true)? => {
                    return Err(self.startup_auxiliary_identity_error(
                        path,
                        "moved geometry source reappeared",
                    ));
                }
                None => {}
            }
        }
        for (path, expected) in &publication.expected_paths {
            let historical = path.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1);
            let current = if path
                .file_name()
                .is_some_and(|name| name == HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
            {
                self.stable_historical_autonomous_recovery_directory_inventory(
                    path,
                    HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
                    self.historical_autonomous_recovery_aggregate_byte_limit(),
                )?
                .0
            } else {
                self.stable_sidecar_directory_inventory_with_recognized_child(
                    path,
                    Some(&historical),
                )?
            };
            Self::require_startup_auxiliary_identity(expected, &current)?;
        }
        Ok(())
    }
}
