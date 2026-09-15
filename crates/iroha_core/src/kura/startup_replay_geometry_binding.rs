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
        let original_lane_paths = self
            .v2_startup_replay_lane_auxiliary_sidecar_directories()?
            .into_iter()
            .flat_map(|(lane, historical)| [lane, historical])
            .collect::<BTreeSet<_>>();
        let mut moves = Vec::new();
        let mut sources = BTreeMap::new();
        let mut paths = BTreeSet::new();
        let mut prior_updated = None;
        let mut prior_index = None;
        let mut final_lanes = self.lane_storage_entries.lock().clone();
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
                && final_lanes != Self::lane_storage_entries_from_config(request.previous)
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
                let mut add = |source: PathBuf,
                               target: PathBuf,
                               merge: PathBuf,
                               binding: &LaneGeometryBinding| {
                    paths.insert(source.clone());
                    paths.insert(target.clone());
                    sources
                        .entry(source.clone())
                        .or_insert_with(|| (merge, binding.clone()));
                    moves.push((source, target));
                };
                match operation.kind {
                    LaneGeometryOperationKind::Create => add(
                        self.resolve_relative_path(&operation.unpublished_blocks_path)?,
                        self.binding_blocks_path(
                            operation.updated.as_ref().expect("validated create"),
                        ),
                        self.resolve_relative_path(&operation.unpublished_merge_path)?,
                        operation.updated.as_ref().expect("validated create"),
                    ),
                    LaneGeometryOperationKind::Retire => add(
                        self.binding_blocks_path(
                            operation.previous.as_ref().expect("validated retire"),
                        ),
                        self.resolve_relative_path(&operation.archived_blocks_path)?,
                        self.binding_merge_path(
                            operation.previous.as_ref().expect("validated retire"),
                        ),
                        operation.previous.as_ref().expect("validated retire"),
                    ),
                    LaneGeometryOperationKind::Replace => {
                        add(
                            self.binding_blocks_path(
                                operation.previous.as_ref().expect("validated replace"),
                            ),
                            self.resolve_relative_path(&operation.archived_blocks_path)?,
                            self.binding_merge_path(
                                operation.previous.as_ref().expect("validated replace"),
                            ),
                            operation.previous.as_ref().expect("validated replace"),
                        );
                        add(
                            self.resolve_relative_path(&operation.unpublished_blocks_path)?,
                            self.binding_blocks_path(
                                operation.updated.as_ref().expect("validated replace"),
                            ),
                            self.resolve_relative_path(&operation.unpublished_merge_path)?,
                            operation.updated.as_ref().expect("validated replace"),
                        );
                    }
                    LaneGeometryOperationKind::Relabel => add(
                        self.binding_blocks_path(
                            operation.previous.as_ref().expect("validated relabel"),
                        ),
                        self.binding_blocks_path(
                            operation.updated.as_ref().expect("validated relabel"),
                        ),
                        self.binding_merge_path(
                            operation.previous.as_ref().expect("validated relabel"),
                        ),
                        operation.previous.as_ref().expect("validated relabel"),
                    ),
                }
            }
            prior_updated = Some(updated);
            prior_index = Some(index);
            final_lanes = Self::lane_storage_entries_from_config(request.updated);
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
            if original_lane_paths.contains(&path) {
                return Err(self.startup_auxiliary_identity_error(
                    &path,
                    "active namespace absence is not an archived replay creation",
                ));
            }
            self.require_lane_marker_at(&blocks, &native_binding)?;
            let marker = self.read_lane_marker(&blocks.join(MARKER_FILE_NAME))?;
            let target_blocks = marker.move_target_blocks.as_ref().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "missing namespace source has no authenticated archive seal",
                )
            })?;
            let target_merge = marker.move_target_merge.as_ref().ok_or_else(|| {
                self.geometry_error(
                    ErrorKind::InvalidData,
                    "missing namespace source has no authenticated merge seal",
                )
            })?;
            self.require_sealed_geometry_pair_at(
                &native_binding,
                &blocks,
                &merge,
                &self.resolve_relative_path(target_blocks)?,
                &self.resolve_relative_path(target_merge)?,
            )?;
            missing_namespaces.push(StartupReplayMissingNamespace {
                blocks,
                merge,
                blocks_identity: identity,
                binding: native_binding,
                original_marker: marker,
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
        let canonical_root = self
            .store_root
            .canonicalize()
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        for (source, target) in moves {
            let source_lane = Self::lane_artifact_dir(&source);
            let target_lane = Self::lane_artifact_dir(&target);
            let source_present = block_identities[&source].is_some();
            let target_present = block_identities[&target].is_some();
            if source == target || (!source_present && target_present) {
                continue; // Exact durable operation was already applied; identities remain pinned.
            }
            if !source_present || target_present {
                return Err(self.startup_auxiliary_identity_error(
                    &source_lane,
                    "retained geometry source missing or destination already occupied",
                ));
            }
            let identity = block_identities[&source];
            block_identities.insert(source, None);
            block_identities.insert(target, identity);
            for (from, to) in [
                (source_lane.clone(), target_lane.clone()),
                (
                    source_lane.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1),
                    target_lane.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1),
                ),
            ] {
                let old = expected_paths[&from].clone();
                let mut moved = old.clone();
                moved.directory.expected_path = to.clone();
                let canonical_to =
                    canonical_root.join(to.strip_prefix(&self.store_root).map_err(|_| {
                        self.startup_auxiliary_identity_error(
                            &to,
                            "geometry target escaped store root",
                        )
                    })?);
                if moved.directory.canonical_path.is_some() {
                    moved.directory.canonical_path = Some(canonical_to.clone());
                }
                moved.files = old
                    .files
                    .into_iter()
                    .map(|(path, mut metadata)| {
                        let name = path.file_name().expect("immediate sidecar child");
                        metadata.canonical_path = canonical_to.join(name);
                        (to.join(name), metadata)
                    })
                    .collect();
                expected_paths.insert(to, moved);
                expected_paths.insert(
                    from.clone(),
                    super::StableSidecarDirectoryInventory {
                        directory: super::StableSidecarDirectoryMetadata {
                            expected_path: from,
                            canonical_path: None,
                            metadata: None,
                        },
                        files: BTreeMap::new(),
                    },
                );
            }
        }
        let mut final_auxiliary = original_auxiliary
            .iter()
            .filter(|(path, _)| !original_lane_paths.contains(*path))
            .map(|(path, value)| (path.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
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
            self.require_sealed_geometry_pair_at(
                &missing.binding,
                &missing.blocks,
                &missing.merge,
                &self.resolve_relative_path(
                    current_marker
                        .move_target_blocks
                        .as_deref()
                        .ok_or_else(|| {
                            self.geometry_error(
                                ErrorKind::InvalidData,
                                "rollback archive is unsealed",
                            )
                        })?,
                )?,
                &self.resolve_relative_path(
                    current_marker.move_target_merge.as_deref().ok_or_else(|| {
                        self.geometry_error(ErrorKind::InvalidData, "rollback merge is unsealed")
                    })?,
                )?,
            )?;
            fs::remove_dir(&path).map_err(|error| Error::IO(error, path.clone()))?;
            self.sync_geometry_parent(Some(&missing.blocks))?;
            if self.geometry_block_store_digest(&missing.blocks)?
                != missing.original_marker.block_store_digest
                || self.geometry_merge_log_digest(&missing.merge)?
                    != missing.original_marker.merge_log_digest
            {
                return Err(self.geometry_error(
                    ErrorKind::InvalidData,
                    "namespace cleanup did not restore the exact authenticated archive",
                ));
            }
            self.atomic_write_geometry_file(
                &missing.blocks.join(MARKER_FILE_NAME),
                &missing.blocks.join(MARKER_TEMP_FILE_NAME),
                &missing.original_marker.encode(),
            )?;
            self.require_geometry_path_identity(&missing.blocks, true, missing.blocks_identity)?;
            self.require_sealed_geometry_pair_at(
                &missing.binding,
                &missing.blocks,
                &missing.merge,
                &self.resolve_relative_path(
                    missing
                        .original_marker
                        .move_target_blocks
                        .as_deref()
                        .expect("prevalidated archive"),
                )?,
                &self.resolve_relative_path(
                    missing
                        .original_marker
                        .move_target_merge
                        .as_deref()
                        .expect("prevalidated archive"),
                )?,
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
