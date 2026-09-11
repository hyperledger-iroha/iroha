// Crash-safe, capacity-bounded lane-history compaction.
/// Exact canonical application authority for an obsolete lane-history prefix.
/// Only the constructor below may turn a persisted cursor into this proof.
struct AuthenticatedLaneHistoryRetention {
    entry: LaneConfigEntry,
    frontier: LaneMergeApplicationFrontierV1,
    first_retained_height: u64,
}

impl AuthenticatedLaneHistoryRetention {
    fn permits_discard(&self, descriptor: &LaneBlockDescriptorV1) -> bool {
        descriptor.lane_id == self.frontier.lane_id
            && descriptor.dataspace_id == self.frontier.dataspace_id
            && descriptor.lane_incarnation == self.frontier.lane_incarnation
            && descriptor.proposal_height <= self.frontier.proposal_height
            && descriptor.lane_block_height > 0
            && descriptor.lane_block_height < self.first_retained_height
    }
}

#[derive(Clone, Copy)]
enum LaneHistoryTerminalEvidenceRole {
    Unpinned,
    CanonicalReplica,
    ApplicationReceipt,
}

impl Kura {
    /// Rebuild every capacity obligation and finish owned publication recovery
    /// before terminal history compaction can remove source dependencies.
    fn recover_lane_histories_on_startup(&self) -> Result<()> {
        self.rebuild_post_wsv_lane_artifact_budget_reservations_on_startup()?;
        self.rebuild_certified_bundle_capacity_reservations_on_startup()?;
        self.repair_autonomous_lane_merge_bundles_on_startup()?;
        self.repair_lane_merge_application_frontiers_on_startup()?;
        self.rebuild_autonomous_lane_route_latest_attempt_indexes_on_startup()
    }

    /// Authenticate a retention floor without performing compaction or source repair.
    /// The cursor's exact carrier coordinates require canonical finality and its
    /// full merge entry; a missing derived reverse index remains startup repair
    /// work, while a conflicting retained index rejects the floor.
    /// The caller holds `prune_lock`, but no geometry or sidecar lock.
    fn authenticated_lane_history_retention_under_prune_guard(
        &self,
        entry: &LaneConfigEntry,
    ) -> Result<Option<AuthenticatedLaneHistoryRetention>> {
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let path = Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root);
        let frontier = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            if self.lane_storage_entry(entry.lane_id)? != *entry {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "lane geometry changed during retention authority authentication",
                ));
            }
            let _sidecar_guard = self.sidecar_lock.lock();
            if self.bound_progress_sidecar_directory_is_absent(&path, &path)? {
                None
            } else {
                self.decode_lane_merge_application_frontier(entry, &path)?
            }
        };
        let Some(frontier) = frontier else {
            return Ok(None);
        };
        if self
            .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                &frontier,
            )
            .is_none()
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "lane-history retention frontier has no exact finality-authenticated merge carrier",
            ));
        }
        Ok(Some(AuthenticatedLaneHistoryRetention {
            entry: entry.clone(),
            frontier,
            first_retained_height: frontier
                .lane_block_height
                .saturating_sub(self.lane_history_retention.get() as u64)
                .saturating_add(1),
        }))
    }

    /// Finish only existing compaction rewrite protocols before capacity inventory.
    /// Both resulting images are authenticated before either pair is promoted.
    /// The caller holds `prune_lock`, but no geometry or sidecar lock.
    fn recover_certified_bundle_history_rewrites_under_prune_guard(
        &self,
        entry: &LaneConfigEntry,
        retention: Option<&AuthenticatedLaneHistoryRetention>,
        frontier: Option<&CertifiedLaneBlockArtifact>,
    ) -> Result<()> {
        struct RewriteImage {
            namespace: BoundProgressNamespace,
            data_path: PathBuf,
            index_path: PathBuf,
            kind: &'static str,
            has_data_temp: bool,
            has_index_temp: bool,
            original: BoundProgressPair,
            candidate: Option<BoundProgressPair>,
            inventory: BTreeMap<u64, (CertifiedLaneBlockArtifact, Hash)>,
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        if self.lane_storage_entry(entry.lane_id)? != *entry {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "lane geometry changed during certified-history rewrite recovery",
            ));
        }
        let _sidecar_guard = self.sidecar_lock.lock();
        let pairs = [
            (
                Self::certified_lane_block_paths_for_entry(entry, &self.store_root),
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
                true,
            ),
            (
                Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root),
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                false,
            ),
        ];
        let mut bound_pairs = Vec::with_capacity(pairs.len());
        for ((data_path, index_path), kind, certified_pair) in pairs {
            if self.bound_progress_sidecar_directory_is_absent(&data_path, &index_path)? {
                return Ok(());
            }
            let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
            let has_data = self
                .open_optional_bound_progress_file(
                    &namespace,
                    &data_path.with_extension("norito.tmp"),
                )?
                .is_some();
            let has_index = self
                .open_optional_bound_progress_file(
                    &namespace,
                    &index_path.with_extension("index.tmp"),
                )?
                .is_some();
            bound_pairs.push((
                namespace,
                data_path,
                index_path,
                kind,
                certified_pair,
                has_data,
                has_index,
            ));
        }
        if !bound_pairs
            .iter()
            .any(|(_, _, _, _, _, data, index)| *data || *index)
        {
            return Ok(());
        }
        let Some(retention) = retention else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified-history rewrite has no authenticated terminal retention authority",
            ));
        };
        let frontier_path =
            Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root);
        if retention.entry != *entry
            || self.decode_lane_merge_application_frontier(entry, &frontier_path)?
                != Some(retention.frontier)
        {
            return Err(Self::invalid_lane_artifact_error(
                frontier_path,
                "lane-history retention authority changed before rewrite recovery",
            ));
        }
        let mut images = Vec::with_capacity(bound_pairs.len());
        for (namespace, data_path, index_path, kind, certified_pair, has_data, has_index) in
            bound_pairs
        {
            for conflicting in [
                index_path.with_extension("index.prepend.tmp"),
                Self::bound_progress_append_build_path(&index_path),
                Self::bound_progress_append_intent_path(&index_path),
            ] {
                if self
                    .open_optional_bound_progress_file(&namespace, &conflicting)?
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        conflicting,
                        "terminal rewrite recovery conflicts with an outstanding append or prepend",
                    ));
                }
            }
            let mut original = self.open_bound_progress_pair(&data_path, &index_path)?;
            let mut candidate = if has_index {
                let recovery_data = if has_data {
                    data_path.with_extension("norito.tmp")
                } else {
                    data_path.clone()
                };
                Some(self.open_bound_progress_pair(
                    &recovery_data,
                    &index_path.with_extension("index.tmp"),
                )?)
            } else {
                None
            };
            let inventory = match candidate.as_mut().unwrap_or(&mut original) {
                BoundProgressPair::Present(bound) => {
                    self.certified_history_rewrite_inventory_locked(entry, bound, certified_pair)?
                }
                BoundProgressPair::Absent(_) if !has_data && !has_index => BTreeMap::new(),
                BoundProgressPair::Absent(_) => {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path,
                        "terminal rewrite has no complete original or candidate pair",
                    ));
                }
            };
            if has_index {
                // After data promotion the original index is still available,
                // but its old offsets cannot address the replacement data. Its
                // populated retained heights remain mandatory independently.
                let BoundProgressPair::Present(original_bound) = &mut original else {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path,
                        "terminal rewrite has no original index",
                    ));
                };
                let original_index_len = original_bound
                    .index
                    .metadata()
                    .map_err(|error| Error::IO(error, index_path.clone()))?
                    .len();
                let layout =
                    SidecarIndexLayout::read_from(&mut original_bound.index, original_index_len)
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(index_path.clone(), message)
                        })?;
                if layout.aligned_len != original_index_len
                    || usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
                        > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
                {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path,
                        "terminal rewrite original index exceeds its canonical bound",
                    ));
                }
                original_bound
                    .index
                    .seek(SeekFrom::Start(layout.entries_offset))
                    .map_err(|error| Error::IO(error, index_path.clone()))?;
                for offset in 0..layout.entry_count {
                    let mut bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
                    original_bound
                        .index
                        .read_exact(&mut bytes)
                        .map_err(|error| Error::IO(error, index_path.clone()))?;
                    let indexed = SidecarIndexEntry::from_bytes(bytes);
                    let height = layout.base_height.checked_add(offset).ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            index_path.clone(),
                            "terminal rewrite original index height overflows",
                        )
                    })?;
                    if (indexed.len == 0 && indexed.offset != 0)
                        || (indexed.len != 0
                            && height >= retention.first_retained_height
                            && !inventory.contains_key(&height))
                    {
                        return Err(Self::invalid_lane_artifact_error(
                            index_path,
                            "terminal rewrite omits a retained original slot or has a noncanonical preimage",
                        ));
                    }
                }
                if has_data {
                    let original_inventory = self.certified_history_rewrite_inventory_locked(
                        entry,
                        original_bound,
                        certified_pair,
                    )?;
                    if original_inventory.iter().any(|(height, (artifact, hash))| {
                        !retention.permits_discard(&artifact.proposal.descriptor)
                            && inventory
                                .get(height)
                                .map(|(_, candidate_hash)| candidate_hash)
                                != Some(hash)
                    }) {
                        return Err(Self::invalid_lane_artifact_error(
                            index_path,
                            "terminal rewrite changes a retained original payload",
                        ));
                    }
                }
            }
            images.push(RewriteImage {
                namespace,
                data_path,
                index_path,
                kind,
                has_data_temp: has_data,
                has_index_temp: has_index,
                original,
                candidate,
                inventory,
            });
        }
        let certified = &images[0].inventory;
        let bundles = &images[1].inventory;
        let Some(frontier) = frontier else {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "terminal certified-history rewrite has no mandatory durable certified frontier",
            ));
        };
        let frontier_height = frontier.proposal.descriptor.lane_block_height;
        if certified.keys().any(|height| *height > frontier_height)
            || certified
                .get(&frontier_height)
                .is_some_and(|(artifact, _)| artifact != frontier)
            || (!retention.permits_discard(&frontier.proposal.descriptor)
                && !certified.contains_key(&frontier_height))
        {
            return Err(Self::invalid_lane_artifact_error(
                images[0].index_path.clone(),
                "terminal rewrite conflicts with its exact current certified frontier",
            ));
        }
        // In the data-promoted crash cut the old payload no longer exists.
        // Authenticate its retained identity against the opposite resulting
        // image and singleton, including when both images await promotion.
        for (height, (bundle_certificate, _)) in bundles {
            if retention.permits_discard(&bundle_certificate.proposal.descriptor) {
                continue;
            }
            if bundle_certificate
                .prepare_qc
                .payload_availability_qc
                .is_none()
                || certified.get(height).map(|(artifact, _)| artifact) != Some(bundle_certificate)
            {
                return Err(Self::invalid_lane_artifact_error(
                    images[1].index_path.clone(),
                    "terminal rewrite bundle differs from its exact retained certified slot",
                ));
            }
        }
        for (height, (artifact, _)) in certified {
            if !retention.permits_discard(&artifact.proposal.descriptor)
                && artifact.prepare_qc.payload_availability_qc.is_some()
                && !bundles.contains_key(height)
                && artifact != frontier
            {
                return Err(Self::invalid_lane_artifact_error(
                    images[0].index_path.clone(),
                    "terminal rewrite omits a retained historical autonomous bundle",
                ));
            }
        }
        // Check every observed image before the first write. Bound promotion
        // revalidates directory and file identities and only renames existing
        // bytes; a lone data temp is an uncommitted rewrite and is discarded.
        for image in &images {
            for pair in std::iter::once(&image.original).chain(image.candidate.iter()) {
                let unchanged = match pair {
                    BoundProgressPair::Present(bound) => {
                        self.bound_progress_sidecar_unchanged(bound)
                    }
                    BoundProgressPair::Absent(namespace) => {
                        self.bound_progress_namespace_unchanged(namespace)
                    }
                };
                if !unchanged {
                    return Err(Self::invalid_lane_artifact_error(
                        image.index_path.clone(),
                        "terminal rewrite image changed during authentication",
                    ));
                }
            }
        }
        for image in images {
            if !image.has_data_temp && !image.has_index_temp {
                continue;
            }
            let before = Self::sidecar_tracked_bytes(&image.data_path, &image.index_path)?;
            let accounting = self
                .begin_total_disk_usage_mutation()
                .with_resource_paths(vec![
                    image.data_path.clone(),
                    image.index_path.clone(),
                    image.data_path.with_extension("norito.tmp"),
                    image.index_path.with_extension("index.tmp"),
                ]);
            if !self.recover_bound_progress_sidecar_artifacts_in_namespace(
                &image.namespace,
                &image.data_path,
                &image.index_path,
                image.kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    image.index_path,
                    "authenticated terminal rewrite recovery failed",
                ));
            }
            self.update_disk_usage_delta(
                before,
                Self::sidecar_tracked_bytes(&image.data_path, &image.index_path)?,
            );
            accounting.finish();
        }
        Ok(())
    }

    /// Authenticate every complete image before a terminal rewrite is promoted.
    fn certified_history_rewrite_inventory_locked(
        &self,
        entry: &LaneConfigEntry,
        bound: &mut BoundProgressSidecar,
        certified_pair: bool,
    ) -> Result<BTreeMap<u64, (CertifiedLaneBlockArtifact, Hash)>> {
        let heights = if certified_pair {
            self.bound_indexed_sidecar_payload_heights(
                bound,
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
                MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
            )?
        } else {
            self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(bound.namespace.index_path.clone(), message)
                })?
                .1
        };
        let mut inventory = BTreeMap::new();
        for height in heights {
            let (artifact, bytes) = if certified_pair {
                let artifact = self
                    .read_active_certified_lane_block_artifact_from_bound_locked(
                        entry, height, bound,
                    )
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            bound.namespace.data_path.clone(),
                            "terminal rewrite certificate is malformed or targets stale geometry",
                        )
                    })?;
                let bytes = artifact.encode_framed()?;
                (artifact, bytes)
            } else {
                let (bundle, bytes) = self
                    .read_autonomous_lane_merge_bundle_from_bound_locked(
                        entry.lane_id,
                        height,
                        bound,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            bound.namespace.data_path.clone(),
                            message,
                        )
                    })?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            bound.namespace.data_path.clone(),
                            "terminal rewrite bundle slot disappeared",
                        )
                    })?;
                self.require_active_lane_artifact(entry, &bundle.certified.proposal.descriptor)?;
                (bundle.certified, bytes)
            };
            inventory.insert(height, (artifact, Hash::new(&bytes)));
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err(Self::invalid_lane_artifact_error(
                bound.namespace.index_path.clone(),
                "terminal rewrite payload pair changed during authentication",
            ));
        }
        Ok(inventory)
    }

    fn compact_lane_histories_through_merge_frontier_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        frontier: &LaneMergeApplicationFrontierV1,
    ) -> Result<LaneHistoryCompactionOutcome> {
        if self
            .lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
                frontier,
            )
            .is_none()
        {
            return Err(Self::invalid_lane_artifact_error(
                Self::lane_merge_application_frontier_path_for_entry(entry, &self.store_root),
                "lane merge application frontier does not match its merge entry and carrier",
            ));
        }
        let retention = self.lane_history_retention;
        // Terminal files are independent crash-safe records. Collect their
        // references before recovering any pair temp so an older, otherwise
        // well-formed rewrite cannot be promoted after omitting newly durable
        // evidence. Both Pending and Complete stages retain dependencies.
        let terminal_references =
            self.active_autonomous_terminal_evidence_references_locked(entry)?;
        let empty_required_heights = BTreeSet::new();
        let pairs = [
            (
                Self::lane_artifact_paths_for_entry(entry, &self.store_root),
                LaneBlockArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::lane_block_execution_input_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionInputArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::lane_block_execution_preflight_paths_for_entry(entry, &self.store_root),
                LaneBlockExecutionPreflightArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::certified_lane_block_paths_for_entry(entry, &self.store_root),
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root),
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::Unpinned,
            ),
            (
                Self::canonical_autonomous_lane_replica_paths_for_entry(entry, &self.store_root),
                CANONICAL_AUTONOMOUS_LANE_REPLICA_FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica,
            ),
            (
                Self::lane_block_application_receipt_paths_for_entry(entry, &self.store_root),
                LaneBlockApplicationReceiptArtifact::FORMAT_LABEL,
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt,
            ),
        ];
        // Finish every already-durable rewrite before optional capacity
        // preflight or fresh pruning. Required heights are checked against the
        // recovery candidate itself, including the data-promoted/index-temp
        // crash boundary.
        for ((data_path, index_path), kind, role) in &pairs {
            let required_heights = match role {
                LaneHistoryTerminalEvidenceRole::Unpinned => &empty_required_heights,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica => {
                    &terminal_references.replica_heights
                }
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt => {
                    &terminal_references.receipt_heights
                }
            };
            // Complete any already-durable rewrite before deciding whether a
            // fresh optional compaction can afford another temporary pair.
            // Otherwise the crash temp is counted once in physical usage and
            // again as projected headroom, so a tight-cap startup can refuse
            // the very promotion needed to make retirement drainable.
            let before_recovery = Self::sidecar_tracked_bytes(data_path, index_path)?;
            let recovery_accounting =
                self.begin_total_disk_usage_mutation()
                    .with_resource_paths(vec![
                        data_path.to_path_buf(),
                        index_path.to_path_buf(),
                        data_path.with_extension("norito.tmp"),
                        index_path.with_extension("index.tmp"),
                    ]);
            if !Self::recover_indexed_sidecar_artifacts_with_required_heights(
                data_path,
                index_path,
                required_heights,
                kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    format!("{kind} terminal-frontier recovery failed"),
                ));
            }
            let before = Self::sidecar_tracked_bytes(data_path, index_path)?;
            self.update_disk_usage_delta(before_recovery, before);
            recovery_accounting.finish();
        }
        self.validate_active_autonomous_terminal_evidence_references_locked(
            entry,
            &terminal_references,
        )?;

        // Rewrites publish one data/index temp pair at a time. Preflight the
        // largest pair against the unchanged pre-compaction state so a
        // configured-capacity refusal is byte-exact and cannot leave an early
        // pair compacted while a later pair is rejected.
        if self.max_disk_usage_bytes != 0 {
            let temp_peak = pairs
                .iter()
                .try_fold(0_u64, |peak, ((data, index), _, _)| {
                    Self::sidecar_tracked_bytes(data, index).map(|bytes| peak.max(bytes))
                })?;
            let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
            let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;
            let terminal_reservations =
                self.autonomous_global_terminal_outcome_reserved_bytes_locked()?;
            let required = self
                .kura_disk_usage_bytes()?
                .checked_add(pending_canonical_bytes)
                .and_then(|bytes| bytes.checked_add(terminal_reservations))
                .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
                .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
                .and_then(|bytes| {
                    bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
                })
                .and_then(|bytes| bytes.checked_add(temp_peak))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root)),
                        "lane history compaction configured accounting overflowed",
                    )
                })?;
            if required > self.max_disk_usage_bytes {
                iroha_logger::debug!(
                    lane = %entry.lane_id.as_u32(),
                    required,
                    limit = self.max_disk_usage_bytes,
                    "skipping bounded lane-history compaction without configured temp headroom"
                );
                return Ok(LaneHistoryCompactionOutcome::CapacityBlocked);
            }
        }

        for ((data_path, index_path), kind, role) in pairs {
            let required_heights = match role {
                LaneHistoryTerminalEvidenceRole::Unpinned => &empty_required_heights,
                LaneHistoryTerminalEvidenceRole::CanonicalReplica => {
                    &terminal_references.replica_heights
                }
                LaneHistoryTerminalEvidenceRole::ApplicationReceipt => {
                    &terminal_references.receipt_heights
                }
            };
            let before = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
            let accounting_mutation =
                self.begin_total_disk_usage_mutation()
                    .with_resource_paths(vec![
                        data_path.to_path_buf(),
                        index_path.to_path_buf(),
                        data_path.with_extension("norito.tmp"),
                        index_path.with_extension("index.tmp"),
                    ]);
            if !Self::prune_indexed_sidecars_through_terminal_frontier_with_required_heights(
                &data_path,
                &index_path,
                frontier.lane_block_height,
                retention,
                required_heights,
                kind,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    format!("{kind} terminal-frontier compaction failed"),
                ));
            }
            let after = Self::sidecar_tracked_bytes(&data_path, &index_path)?;
            self.update_disk_usage_delta(before, after);
            accounting_mutation.finish();
        }
        self.validate_active_autonomous_terminal_evidence_references_locked(
            entry,
            &terminal_references,
        )?;
        // Autonomous attempt/cursor/outcome files form one crash-sensitive
        // evidence unit. Even though this maintenance runs only after the
        // receipt frontier (and, on the live carrier path, after terminal
        // completion), it must not unlink only the payload/view half of a
        // lifecycle unit. Keep the bounded namespace intact; lane
        // archive/removal moves the whole directory atomically after terminal
        // validation. A future compactor may remove complete units only behind
        // its own durable intent.
        Ok(LaneHistoryCompactionOutcome::Complete)
    }
}
