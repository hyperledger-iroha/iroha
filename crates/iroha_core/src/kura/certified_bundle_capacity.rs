impl Kura {
    #[cfg(test)]
    fn write_certified_lane_block_artifact(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
    ) -> Result<()> {
        self.write_certified_lane_block_artifact_with_authority(artifact, None, None)
    }
    // The inflight source contract audits this authority-bearing publication boundary directly.
    #[allow(dead_code)]
    fn write_certified_lane_block_artifact_with_authority(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
        authority: Option<&crate::state::CertifiedLaneBlockPersistenceAuthority>,
        lane_commit_authorization: Option<AutonomousLaneCommitPersistenceAuthorization>,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.write_certified_lane_block_artifact_with_authority_under_prune_guard(
            artifact,
            authority,
            lane_commit_authorization,
        )
    }
    /// Read a certified standalone lane block by lane and lane-local block height.
    ///
    /// Returns `None` when the artifact is absent, malformed, belongs to a different
    /// lane/height or active geometry incarnation, fails proposal/QC consistency checks,
    /// or cannot pass the complete progress-sidecar durability barrier sequence.
    #[must_use]
    pub fn read_certified_lane_block_artifact(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Option<CertifiedLaneBlockArtifact> {
        if self.prune_recovery_is_required() {
            return None;
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id).ok()?;
        let (data_path, index_path) =
            Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
        let _guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        self.read_active_certified_lane_block_artifact_from_paths_durability_attested_locked(
            &entry,
            lane_block_height,
            &data_path,
            &index_path,
            true,
        )
    }
    fn publish_certified_frontier_and_consume_capacity_locked(
        &self,
        entry: &LaneConfigEntry,
        artifact: &CertifiedLaneBlockArtifact,
        authority: Option<&crate::state::CertifiedLaneBlockPersistenceAuthority>,
        autonomous_certificate: bool,
        data_path: &Path,
    ) -> Result<bool> {
        #[cfg(not(test))]
        let _ = data_path;
        let frontier_changed =
            self.publish_latest_certified_lane_block_frontier_locked(entry, artifact, authority)?;
        if autonomous_certificate {
            let durable_frontier = self
                .read_latest_certified_lane_block_frontier_structural_locked(entry, false)?
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        Self::latest_certified_lane_block_frontier_paths_for_entry(
                            entry,
                            &self.store_root,
                        )
                        .0,
                        "autonomous certified frontier disappeared before capacity consumption",
                    )
                })?;
            if durable_frontier.frontier.artifact != *artifact {
                return Err(Self::invalid_lane_artifact_error(
                    Self::latest_certified_lane_block_frontier_paths_for_entry(
                        entry,
                        &self.store_root,
                    )
                    .0,
                    "autonomous certified frontier differs before capacity consumption",
                ));
            }
            self.confirm_latest_certified_lane_block_frontier_read_locked(
                entry,
                &durable_frontier.snapshot,
            )?;
            self.consume_certified_bundle_frontier_capacity(artifact)?;
        }
        #[cfg(test)]
        if autonomous_certificate
            && FAIL_AFTER_NEXT_AUTONOMOUS_CERTIFIED_FRONTIER.with(|flag| flag.replace(false))
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                "injected failure after autonomous certified frontier publication",
            ));
        }
        Ok(frontier_changed)
    }
    fn decode_latest_certified_lane_block_frontier(
        path: &Path,
        bytes: &[u8],
    ) -> Result<LatestCertifiedLaneBlockFrontierV1> {
        let byte_limit = usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX);
        if bytes.is_empty() || bytes.len() > byte_limit {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "latest certified lane block frontier has an invalid byte length",
            ));
        }
        let frontier = norito::decode_canonical::<LatestCertifiedLaneBlockFrontierV1>(bytes)
            .map_err(|error| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    format!(
                        "latest certified lane block frontier is not exact framed Norito: {error}"
                    ),
                )
            })?;
        if frontier.version != LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_VERSION
            || frontier.computed_integrity_hash() != Some(frontier.integrity_hash)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "latest certified lane block frontier is non-canonical or has invalid integrity",
            ));
        }
        Ok(frontier)
    }
    fn read_latest_certified_lane_block_frontier_structural_locked(
        &self,
        entry: &LaneConfigEntry,
        recover_build: bool,
    ) -> Result<Option<LatestCertifiedLaneBlockFrontierRead>> {
        if self
            .latest_certified_frontier_storage_unknown
            .load(Ordering::Acquire)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "latest certified lane block frontier storage is ambiguous until restart",
            ));
        }
        let (frontier_path, build_path) =
            Self::latest_certified_lane_block_frontier_paths_for_entry(entry, &self.store_root);
        let directory = frontier_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    frontier_path.clone(),
                    "latest certified lane block frontier path has no parent",
                )
            })?;
        let namespace = self.open_bound_progress_namespace(&frontier_path, &build_path)?;
        if recover_build {
            self.recover_latest_certified_lane_block_frontier_build_locked(
                entry,
                &namespace,
                &frontier_path,
                &build_path,
            )?;
        } else {
            match secure_file_metadata::from_path(&build_path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, build_path)),
                Ok(_) => {
                    return Err(Self::invalid_lane_artifact_error(
                        build_path,
                        "latest certified lane block frontier has an unresolved build",
                    ));
                }
            }
        }
        let Some(snapshot) = self.read_regular_sidecar_snapshot(
            &frontier_path,
            &directory,
            usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX),
        )?
        else {
            return Ok(None);
        };
        let frontier =
            Self::decode_latest_certified_lane_block_frontier(&frontier_path, &snapshot.bytes)?;
        if !self.certified_frontier_artifact_validation_is_attested(
            entry.lane_id,
            &frontier.artifact,
            &snapshot,
        ) {
            Self::validate_certified_lane_block_artifact(&frontier.artifact).map_err(
                |message| {
                    Self::invalid_lane_artifact_error(
                        frontier_path.clone(),
                        format!("latest certified lane block frontier is invalid: {message}"),
                    )
                },
            )?;
        }
        Ok(Some(LatestCertifiedLaneBlockFrontierRead {
            frontier,
            snapshot,
        }))
    }
    fn recover_latest_certified_lane_block_frontier_build_locked(
        &self,
        entry: &LaneConfigEntry,
        namespace: &BoundProgressNamespace,
        frontier_path: &Path,
        build_path: &Path,
    ) -> Result<()> {
        let Some(mut build) = self.open_optional_bound_progress_file(namespace, build_path)? else {
            return Ok(());
        };
        let build_len = build
            .metadata()
            .map_err(|error| Error::IO(error, build_path.to_path_buf()))?
            .len();
        if build_len == 0 || build_len > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Self::invalid_lane_artifact_error(
                build_path.to_path_buf(),
                "latest certified lane block frontier build has an invalid byte length",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(build_len)?);
        build
            .seek(SeekFrom::Start(0))
            .and_then(|_| build.read_to_end(&mut bytes))
            .map_err(|error| Error::IO(error, build_path.to_path_buf()))?;
        if u64::try_from(bytes.len())? != build_len {
            return Err(Self::invalid_lane_artifact_error(
                build_path.to_path_buf(),
                "latest certified lane block frontier build changed during recovery",
            ));
        }
        let recovered = Self::decode_latest_certified_lane_block_frontier(build_path, &bytes)?;
        Self::validate_certified_lane_block_artifact(&recovered.artifact).map_err(|message| {
            Self::invalid_lane_artifact_error(
                build_path.to_path_buf(),
                format!("latest certified lane block frontier build is invalid: {message}"),
            )
        })?;
        self.require_active_lane_artifact(entry, &recovered.artifact.proposal.descriptor)?;
        let directory = frontier_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                frontier_path.to_path_buf(),
                "latest certified lane block frontier path has no parent",
            )
        })?;
        if let Some(existing) = self.read_regular_sidecar_snapshot(
            frontier_path,
            directory,
            usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX),
        )? {
            let existing_frontier =
                Self::decode_latest_certified_lane_block_frontier(frontier_path, &existing.bytes)?;
            if existing.bytes != bytes || existing_frontier != recovered {
                return Err(Self::invalid_lane_artifact_error(
                    build_path.to_path_buf(),
                    "latest certified lane block frontier build conflicts with the durable frontier",
                ));
            }
            let accounting_mutation = self.begin_total_disk_usage_mutation();
            Self::remove_bound_progress_temp_if_present(namespace, build_path)
                .map_err(|error| Error::IO(error, build_path.to_path_buf()))?;
            Self::sync_bound_progress_intent_directories(namespace)
                .map_err(|error| Error::IO(error, build_path.to_path_buf()))?;
            self.update_disk_usage_delta(build_len, 0);
            accounting_mutation.finish();
            return Ok(());
        }
        build
            .sync_all()
            .map_err(|error| Error::IO(error, build_path.to_path_buf()))?;
        if let Err(error) =
            Self::promote_bound_progress_temp(namespace, build_path, frontier_path, &build)
        {
            if error.published {
                self.latest_certified_frontier_storage_unknown
                    .store(true, Ordering::Release);
            }
            return Err(Error::IO(error.source, frontier_path.to_path_buf()));
        }
        build
            .sync_all()
            .and_then(|_| Self::sync_bound_progress_intent_directories(namespace))
            .map_err(|error| {
                self.latest_certified_frontier_storage_unknown
                    .store(true, Ordering::Release);
                Error::IO(error, frontier_path.to_path_buf())
            })?;
        let published = self.read_regular_sidecar_snapshot(
            frontier_path,
            directory,
            usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX),
        )?;
        if published.as_ref().map(|snapshot| &snapshot.bytes) != Some(&bytes)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            self.latest_certified_frontier_storage_unknown
                .store(true, Ordering::Release);
            return Err(Self::invalid_lane_artifact_error(
                frontier_path.to_path_buf(),
                "recovered latest certified lane block frontier changed before readback",
            ));
        }
        Ok(())
    }
    fn persist_committed_lane_block_session_inner(
        &self,
        session: &crate::lane_consensus::CommittedLaneBlockSession,
        signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
        authority: Option<&crate::state::CertifiedLaneBlockPersistenceAuthority>,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let artifact = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
        // Reject malformed or unauthorized certificates before installing a
        // process-local capacity obligation.  The writer repeats these checks
        // at its own mutation boundary, but a failure there must not let an
        // invalid caller strand an otherwise unrepairable reservation.
        Self::validate_certified_lane_block_artifact(&artifact).map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
        })?;
        if authority.is_some_and(|authority| !authority.authorizes_proposal(&artifact.proposal)) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified lane block persistence authority does not match the proposal",
            ));
        }
        let autonomous_context = artifact
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .map(|availability| (availability.body.network_id, availability.body.epoch));
        let autonomous_source = if let Some((network_id, epoch)) = autonomous_context.as_ref() {
            let descriptor = &artifact.proposal.descriptor;
            Some(
                self.durable_autonomous_lane_merge_source_under_prune_guard(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    *network_id,
                    *epoch,
                    Some(&artifact),
                    false,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
                })?,
            )
        } else {
            None
        };
        let lane_commit_authorization = autonomous_source
            .as_ref()
            .map(|source| Self::authorize_autonomous_lane_commit_persistence(source, &artifact))
            .transpose()
            .map_err(|message| {
                Self::invalid_lane_artifact_error(self.store_root.clone(), message)
            })?;
        if let Some(source) = autonomous_source.as_ref() {
            self.ensure_certified_bundle_capacity_reservation_under_prune_guard(
                &artifact, source, authority,
            )?;
        }
        // Admission and all three durable publications share one uninterrupted
        // prune corridor.  In particular, an ordinary certificate must not be
        // able to advance this route's frontier between installing the READY
        // reservation and publishing its exact certified frontier/pair.
        self.write_certified_lane_block_artifact_with_authority_under_prune_guard(
            &artifact,
            authority,
            lane_commit_authorization,
        )?;
        self.ensure_prune_recovery_not_required()?;
        if let Some((network_id, epoch)) = autonomous_context.as_ref() {
            let descriptor = &artifact.proposal.descriptor;
            let source = self
                .durable_autonomous_lane_merge_source_under_prune_guard(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    *network_id,
                    *epoch,
                    None,
                    false,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
                })?;
            if source.bundle.certified != artifact {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "durable autonomous merge source retained another certificate".to_owned(),
                ));
            }
            self.ensure_certified_bundle_capacity_reservation_under_prune_guard(
                &artifact, &source, authority,
            )?;
            self.persist_autonomous_lane_merge_bundle_under_prune_guard(&source)?;
            let published = self
                .durable_autonomous_lane_merge_source_under_prune_guard(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    *network_id,
                    *epoch,
                    None,
                    true,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
                })?;
            if published != source {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "published autonomous merge bundle differs from its exact source".to_owned(),
                ));
            }
            self.ensure_certified_bundle_capacity_reservation_under_prune_guard(
                &artifact, &published, authority,
            )?;
        }
        Ok(())
    }
    fn certified_bundle_pair_remaining_capacity_locked(
        &self,
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
    ) -> Result<(u64, u64, u64, bool)> {
        if let Some(recovery) = self.certified_bundle_pair_has_exact_append_recovery_locked(
            data_path, index_path, height, payload, kind,
        )? {
            let intent = &recovery.intent;
            let payload_len = u64::try_from(payload.len())?;
            let index_growth = intent
                .new_index_len
                .checked_sub(intent.old_index_len)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} recovery index length regresses"),
                    )
                })?;
            let mut pair = self.open_bound_progress_pair(data_path, index_path)?;
            let (data_len, index_len, pair_is_present) = match &mut pair {
                BoundProgressPair::Absent(_) => (0, 0, false),
                BoundProgressPair::Present(bound) => (
                    bound
                        .data
                        .metadata()
                        .map_err(|error| Error::IO(error, data_path.to_path_buf()))?
                        .len(),
                    bound
                        .index
                        .metadata()
                        .map_err(|error| Error::IO(error, index_path.to_path_buf()))?
                        .len(),
                    true,
                ),
            };
            if intent.pair_was_present && !pair_is_present {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} recovery lost its authenticated stable pair"),
                ));
            }
            if !(intent.old_data_len..=intent.new_data_len).contains(&data_len)
                || !(intent.old_index_len..=intent.new_index_len).contains(&index_len)
            {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} recovery main files exceed their authenticated transition"),
                ));
            }
            if !recovery.has_durable_intent
                && (data_len != intent.old_data_len
                    || index_len != intent.old_index_len
                    || pair_is_present != intent.pair_was_present)
            {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} append build precedes any authenticated main-file mutation"),
                ));
            }
            let physical_stable = if recovery.has_durable_intent {
                data_len
                    .saturating_sub(intent.old_data_len)
                    .min(payload_len)
                    .checked_add(
                        index_len
                            .saturating_sub(intent.old_index_len)
                            .min(index_growth),
                    )
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.to_path_buf(),
                            format!("{kind} recovery physical growth overflows"),
                        )
                    })?
            } else {
                0
            };
            let stable_growth = payload_len.checked_add(index_growth).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} stable growth overflows"),
                )
            })?;
            let transient = u64::try_from(
                norito::encode_canonical(intent)
                    .map_err(Error::NoritoFrame)?
                    .len(),
            )?;
            if !self.certified_bundle_capacity_pair_read_unchanged(&pair)? {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} recovery pair changed during capacity planning"),
                ));
            }
            return Ok((
                stable_growth,
                transient,
                physical_stable
                    .checked_add(recovery.physical_temp_bytes)
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.to_path_buf(),
                            format!("{kind} startup physical credit overflows"),
                        )
                    })?,
                true,
            ));
        }
        let namespace = self.open_bound_progress_namespace(data_path, index_path)?;
        let namespace_components = namespace
            .stable_relative_components(data_path, index_path)
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} capacity namespace is invalid: {message}"),
                )
            })?;
        let mut pair = self.open_bound_progress_pair(data_path, index_path)?;
        let (mut data, mut index, layout, old_data_len, old_index_len, pair_was_present) =
            match &mut pair {
                BoundProgressPair::Absent(_) => (None, None, None, 0, 0, false),
                BoundProgressPair::Present(bound) => {
                    let old_data_len = bound
                        .data
                        .metadata()
                        .map_err(|error| Error::IO(error, data_path.to_path_buf()))?
                        .len();
                    let old_index_len = bound
                        .index
                        .metadata()
                        .map_err(|error| Error::IO(error, index_path.to_path_buf()))?
                        .len();
                    let layout = SidecarIndexLayout::read_from(&mut bound.index, old_index_len)
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(
                                index_path.to_path_buf(),
                                format!("{kind} capacity index is invalid: {message}"),
                            )
                        })?;
                    if layout.aligned_len != old_index_len {
                        return Err(Self::invalid_lane_artifact_error(
                            index_path.to_path_buf(),
                            format!("{kind} capacity index is misaligned"),
                        ));
                    }
                    (
                        Some(&mut bound.data),
                        Some(&mut bound.index),
                        Some(layout),
                        old_data_len,
                        old_index_len,
                        true,
                    )
                }
            };
        let payload_len = u64::try_from(payload.len())?;
        let (new_index_len, index_write_offset, old_index_bytes, new_index_bytes) =
            if let Some(entry_pos) = layout.and_then(|layout| layout.entry_position(height)) {
                let index_file = index.as_mut().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} capacity lost its index"),
                    )
                })?;
                let mut encoded = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
                index_file
                    .seek(SeekFrom::Start(entry_pos))
                    .and_then(|_| index_file.read_exact(&mut encoded))
                    .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
                let entry = SidecarIndexEntry::from_bytes(encoded);
                if entry.len > 0 {
                    let end = entry.offset.checked_add(entry.len).ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.to_path_buf(),
                            format!("{kind} existing payload range overflows"),
                        )
                    })?;
                    if end > old_data_len {
                        return Err(Self::invalid_lane_artifact_error(
                            data_path.to_path_buf(),
                            format!("{kind} existing payload range is invalid"),
                        ));
                    }
                    let mut existing = vec![0_u8; usize::try_from(entry.len)?];
                    let data_file = data.as_mut().ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.to_path_buf(),
                            format!("{kind} capacity lost its data file"),
                        )
                    })?;
                    data_file
                        .seek(SeekFrom::Start(entry.offset))
                        .and_then(|_| data_file.read_exact(&mut existing))
                        .map_err(|error| Error::IO(error, data_path.to_path_buf()))?;
                    if existing == payload {
                        drop(index);
                        drop(data);
                        if !self.certified_bundle_capacity_pair_read_unchanged(&pair)? {
                            return Err(Self::invalid_lane_artifact_error(
                                data_path.to_path_buf(),
                                format!("{kind} pair changed during exact-slot capacity readback"),
                            ));
                        }
                        return Ok((0, 0, 0, false));
                    }
                }
                (
                    old_index_len,
                    entry_pos,
                    entry.to_bytes().to_vec(),
                    SidecarIndexEntry {
                        offset: old_data_len,
                        len: payload_len,
                    }
                    .to_bytes()
                    .to_vec(),
                )
            } else {
                if let Some(layout) = layout
                    && height < layout.base_height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!(
                            "{kind} composite capacity does not admit a backward index prepend"
                        ),
                    ));
                }
                let mut new_index_bytes = Vec::new();
                let (layout, index_write_offset) = match layout {
                    Some(layout) => (layout, old_index_len),
                    None => {
                        new_index_bytes.extend_from_slice(&SidecarIndexLayout::base_header(height));
                        let layout =
                            SidecarIndexLayout::based(height, INDEXED_SIDECAR_BASE_HEADER_SIZE_U64)
                                .map_err(|message| {
                                    Self::invalid_lane_artifact_error(
                                        index_path.to_path_buf(),
                                        format!("{kind} initial V1 index is invalid: {message}"),
                                    )
                                })?;
                        (layout, 0)
                    }
                };
                let expected_height = layout.next_height().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} next height overflows"),
                    )
                })?;
                let missing = height.checked_sub(expected_height).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} target precedes its index"),
                    )
                })?;
                if missing > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} exact index gap exceeds its bound"),
                    ));
                }
                new_index_bytes.resize(
                    new_index_bytes
                        .len()
                        .checked_add(usize::try_from(
                            missing
                                .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        index_path.to_path_buf(),
                                        format!("{kind} index gap overflows"),
                                    )
                                })?,
                        )?)
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                index_path.to_path_buf(),
                                format!("{kind} index buffer overflows"),
                            )
                        })?,
                    0,
                );
                new_index_bytes.extend_from_slice(
                    &SidecarIndexEntry {
                        offset: old_data_len,
                        len: payload_len,
                    }
                    .to_bytes(),
                );
                let new_index_len = index_write_offset
                    .checked_add(u64::try_from(new_index_bytes.len())?)
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            index_path.to_path_buf(),
                            format!("{kind} new index length overflows"),
                        )
                    })?;
                (
                    new_index_len,
                    index_write_offset,
                    Vec::new(),
                    new_index_bytes,
                )
            };
        let intent = BoundProgressAppendIntentV1 {
            version: BOUND_PROGRESS_APPEND_INTENT_VERSION,
            namespace_components,
            data_file: data_path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        data_path.to_path_buf(),
                        format!("{kind} data name is not UTF-8"),
                    )
                })?
                .to_owned(),
            index_file: index_path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} index name is not UTF-8"),
                    )
                })?
                .to_owned(),
            height,
            pair_was_present,
            old_data_len,
            new_data_len: old_data_len.checked_add(payload_len).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} new data length overflows"),
                )
            })?,
            payload_hash: BoundProgressAppendIntentV1::payload_digest(payload),
            old_index_len,
            new_index_len,
            index_write_offset,
            old_index_bytes,
            new_index_bytes,
            integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
        }
        .seal();
        intent
            .validate_for(&namespace, data_path, index_path)
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} exact capacity intent is invalid: {message}"),
                )
            })?;
        intent
            .validate_against_old_layout(if old_index_len == 0 {
                None
            } else {
                Some(
                    SidecarIndexLayout::read_from(
                        index.as_mut().expect("present index"),
                        old_index_len,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            index_path.to_path_buf(),
                            format!("{kind} old layout changed: {message}"),
                        )
                    })?,
                )
            })
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} exact capacity intent mismatches its old layout: {message}"),
                )
            })?;
        let stable = payload_len
            .checked_add(new_index_len.saturating_sub(old_index_len))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    format!("{kind} exact stable growth overflows"),
                )
            })?;
        let transient = u64::try_from(
            norito::encode_canonical(&intent)
                .map_err(Error::NoritoFrame)?
                .len(),
        )?;
        drop(index);
        drop(data);
        if !self.certified_bundle_capacity_pair_read_unchanged(&pair)? {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                format!("{kind} pair changed during capacity planning"),
            ));
        }
        Ok((stable, transient, 0, false))
    }
    fn certified_bundle_capacity_pair_read_unchanged(
        &self,
        pair: &BoundProgressPair,
    ) -> Result<bool> {
        match pair {
            BoundProgressPair::Present(bound) => Ok(self.bound_progress_sidecar_unchanged(bound)),
            BoundProgressPair::Absent(namespace) => Ok(self
                .open_optional_bound_progress_file(namespace, &namespace.data_path)?
                .is_none()
                && self
                    .open_optional_bound_progress_file(namespace, &namespace.index_path)?
                    .is_none()
                && self.bound_progress_namespace_unchanged(namespace)),
        }
    }
    fn certified_bundle_capacity_plan(
        &self,
        entry: &LaneConfigEntry,
        artifact: &CertifiedLaneBlockArtifact,
        source: &DurableAutonomousLaneMergeSource,
    ) -> Result<CertifiedBundleCapacityPlan> {
        if source.bundle.certified != *artifact
            || source.bundle.encode_framed()? != source.source_bundle
            || source.bundle.bundle_hash()? != source.bundle_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle capacity plan changed its exact autonomous source",
            ));
        }
        let availability = artifact
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "certified/bundle capacity plan requires READY-bearing certification",
                )
            })?;
        let descriptor = &artifact.proposal.descriptor;
        let certified_bytes = artifact.encode_framed()?;
        let frontier =
            LatestCertifiedLaneBlockFrontierV1::new(artifact.clone()).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "certified/bundle capacity plan cannot seal its certified frontier",
                )
            })?;
        let frontier_bytes = norito::encode_canonical(&frontier).map_err(Error::NoritoFrame)?;
        if frontier_bytes.is_empty()
            || frontier_bytes.len()
                > usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle capacity frontier exceeds its hard byte limit",
            ));
        }
        let (certified_data_path, certified_index_path) =
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
        let (certified_component, certified_transient, certified_credit, certified_recovery) = self
            .certified_bundle_pair_remaining_capacity_locked(
                &certified_data_path,
                &certified_index_path,
                descriptor.lane_block_height,
                &certified_bytes,
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
            )?;
        let (bundle_data_path, bundle_index_path) =
            Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root);
        let (bundle_component, bundle_transient, bundle_credit, bundle_recovery) = self
            .certified_bundle_pair_remaining_capacity_locked(
                &bundle_data_path,
                &bundle_index_path,
                descriptor.lane_block_height,
                &source.source_bundle,
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            )?;
        if certified_recovery && bundle_recovery {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified and bundle pairs cannot both have in-flight append recovery",
            ));
        }
        let component_bytes = BTreeMap::from([
            (
                CertifiedBundleCapacityComponent::LatestCertifiedFrontier,
                u64::try_from(frontier_bytes.len())?,
            ),
            (
                CertifiedBundleCapacityComponent::CertifiedPair,
                certified_component,
            ),
            (
                CertifiedBundleCapacityComponent::AutonomousBundlePair,
                bundle_component,
            ),
        ]);
        let component_transient_bytes = BTreeMap::from([
            (CertifiedBundleCapacityComponent::LatestCertifiedFrontier, 0),
            (
                CertifiedBundleCapacityComponent::CertifiedPair,
                certified_transient,
            ),
            (
                CertifiedBundleCapacityComponent::AutonomousBundlePair,
                bundle_transient,
            ),
        ]);
        Ok(CertifiedBundleCapacityPlan {
            identity: CertifiedBundleCapacityIdentity {
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                proposal_height: descriptor.proposal_height,
                lane_block_height: descriptor.lane_block_height,
                lane_block_view: descriptor.lane_block_view,
                lane_block_descriptor_hash: descriptor.descriptor_hash,
                proposal_hash: artifact.proposal.proposal_hash,
                autonomous_network_id: availability.body.network_id,
                autonomous_epoch: availability.body.epoch,
            },
            certified_bytes_hash: Hash::new(&certified_bytes),
            frontier_bytes_hash: Hash::new(&frontier_bytes),
            bundle_bytes_hash: Hash::new(&source.source_bundle),
            component_bytes,
            component_transient_bytes,
            startup_physical_credit_bytes: certified_credit.checked_add(bundle_credit).ok_or_else(
                || {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "certified/bundle startup physical credit overflows",
                    )
                },
            )?,
        })
    }
    /// Read-only recognition of an exact append crash journal. Other rewrite
    /// temporaries are deliberately fail-closed: they do not authenticate the
    /// payload whose composite envelope is being reconstructed.
    fn certified_bundle_pair_has_exact_append_recovery_locked(
        &self,
        data_path: &Path,
        index_path: &Path,
        height: u64,
        payload: &[u8],
        kind: &str,
    ) -> Result<Option<CertifiedBundleAppendRecovery>> {
        let namespace = self.open_bound_progress_namespace(data_path, index_path)?;
        let temp_data_path = data_path.with_extension("norito.tmp");
        let temp_index_path = index_path.with_extension("index.tmp");
        let prepend_index_path = index_path.with_extension("index.prepend.tmp");
        let append_build_path = Self::bound_progress_append_build_path(index_path);
        let append_intent_path = Self::bound_progress_append_intent_path(index_path);
        for path in [&temp_data_path, &temp_index_path, &prepend_index_path] {
            if self
                .open_optional_bound_progress_file(&namespace, path)?
                .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    path.clone(),
                    format!(
                        "{kind} has an unauthenticated rewrite temporary during composite capacity planning"
                    ),
                ));
            }
        }
        let mut append_build =
            self.open_optional_bound_progress_file(&namespace, &append_build_path)?;
        let mut append_intent =
            self.open_optional_bound_progress_file(&namespace, &append_intent_path)?;
        let append_build_metadata = append_build
            .as_ref()
            .map(|file| {
                secure_file_metadata::from_file(file)
                    .map_err(|error| Error::IO(error, append_build_path.clone()))
            })
            .transpose()?;
        let append_intent_metadata = append_intent
            .as_ref()
            .map(|file| {
                secure_file_metadata::from_file(file)
                    .map_err(|error| Error::IO(error, append_intent_path.clone()))
            })
            .transpose()?;
        if append_build_metadata.as_ref().is_some_and(|metadata| {
            metadata.len()
                > u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES).unwrap_or(u64::MAX)
        }) {
            return Err(Self::invalid_lane_artifact_error(
                append_build_path.clone(),
                format!("{kind} append build exceeds its hard byte limit"),
            ));
        }
        let has_durable_intent = append_intent.is_some();
        let (intent, intent_path) = if let Some(intent_file) = append_intent.as_mut() {
            (
                Self::decode_bound_progress_append_intent(
                    intent_file,
                    &append_intent_path,
                    &namespace,
                    data_path,
                    index_path,
                    kind,
                )
                .map_err(|_| {
                    Self::invalid_lane_artifact_error(
                        append_intent_path.clone(),
                        format!("{kind} append intent is malformed or unauthenticated"),
                    )
                })?,
                append_intent_path.clone(),
            )
        } else if let Some(build_file) = append_build.as_mut() {
            (
                Self::decode_bound_progress_append_intent(
                    build_file,
                    &append_build_path,
                    &namespace,
                    data_path,
                    index_path,
                    kind,
                )
                .map_err(|_| {
                    Self::invalid_lane_artifact_error(
                        append_build_path.clone(),
                        format!("{kind} lone append build is malformed or unauthenticated"),
                    )
                })?,
                append_build_path.clone(),
            )
        } else {
            return Ok(None);
        };
        if intent.height != height
            || intent.payload_hash != BoundProgressAppendIntentV1::payload_digest(payload)
            || intent.payload_len() != u64::try_from(payload.len()).ok()
        {
            return Err(Self::invalid_lane_artifact_error(
                intent_path,
                format!("{kind} append intent names another payload or lane slot"),
            ));
        }
        if has_durable_intent && let Some(build_file) = append_build.as_mut() {
            let build_intent = Self::decode_bound_progress_append_intent(
                build_file,
                &append_build_path,
                &namespace,
                data_path,
                index_path,
                kind,
            )
            .map_err(|_| {
                Self::invalid_lane_artifact_error(
                    append_build_path.clone(),
                    format!("{kind} append build is malformed or unauthenticated"),
                )
            })?;
            if build_intent != intent {
                return Err(Self::invalid_lane_artifact_error(
                    append_build_path,
                    format!("{kind} append build conflicts with its durable intent"),
                ));
            }
        }
        let file_unchanged =
            |file: &std::fs::File, path: &Path, before: &SecureMetadata| -> Result<bool> {
            let opened_after = secure_file_metadata::from_file(file)
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                let path_after = secure_file_metadata::from_path(path)
                    .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                Ok(Self::sidecar_file_metadata_unchanged(before, &opened_after)
                    && Self::sidecar_file_metadata_unchanged(&opened_after, &path_after))
            };
        if append_build
            .as_ref()
            .zip(append_build_metadata.as_ref())
            .is_some_and(|(file, before)| {
                !file_unchanged(file, &append_build_path, before).unwrap_or(false)
            })
            || append_intent
                .as_ref()
                .zip(append_intent_metadata.as_ref())
                .is_some_and(|(file, before)| {
                    !file_unchanged(file, &append_intent_path, before).unwrap_or(false)
                })
            || !self.bound_progress_namespace_unchanged(&namespace)
        {
            return Err(Self::invalid_lane_artifact_error(
                intent_path,
                format!("{kind} append recovery file changed during authentication"),
            ));
        }
        let physical_temp_bytes = append_build_metadata
            .as_ref()
            .map_or(0, |metadata| metadata.len())
            .checked_add(
                append_intent_metadata
                    .as_ref()
                    .map_or(0, |metadata| metadata.len()),
            )
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    intent_path.clone(),
                    format!("{kind} append recovery physical bytes overflow"),
                )
            })?;
        Ok(Some(CertifiedBundleAppendRecovery {
            intent,
            has_durable_intent,
            physical_temp_bytes,
        }))
    }
    /// Read the immutable preimage named by an authenticated append intent
    /// without rolling either main file backward or forward. This lets startup
    /// validate older stable history before the exact target append consumes
    /// any reserved capacity.
    fn certified_bundle_append_preimage_payloads_locked(
        &self,
        data_path: &Path,
        index_path: &Path,
        intent: &BoundProgressAppendIntentV1,
        kind: &str,
    ) -> Result<BTreeMap<u64, Vec<u8>>> {
        if !intent.pair_was_present {
            return Ok(BTreeMap::new());
        }
        let mut pair = self.open_bound_progress_pair(data_path, index_path)?;
        let BoundProgressPair::Present(bound) = &mut pair else {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                format!("{kind} append intent lost its stable preimage pair"),
            ));
        };
        let data_len = bound
            .data
            .metadata()
            .map_err(|error| Error::IO(error, data_path.to_path_buf()))?
            .len();
        let index_len = bound
            .index
            .metadata()
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?
            .len();
        if data_len < intent.old_data_len || index_len < intent.old_index_len {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                format!("{kind} append intent cannot reconstruct its stable preimage"),
            ));
        }
        let mut index = vec![0_u8; usize::try_from(intent.old_index_len)?];
        bound
            .index
            .seek(SeekFrom::Start(0))
            .and_then(|_| bound.index.read_exact(&mut index))
            .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
        let old_window_start = usize::try_from(intent.index_write_offset)?;
        let old_window_end = old_window_start
            .checked_add(intent.old_index_bytes.len())
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage index window overflows"),
                )
            })?;
        let old_window = index
            .get_mut(old_window_start..old_window_end)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage index window is outside its old layout"),
                )
            })?;
        old_window.copy_from_slice(&intent.old_index_bytes);
        if index.len() < INDEXED_SIDECAR_BASE_HEADER_SIZE {
            return Err(Self::invalid_lane_artifact_error(
                index_path.to_path_buf(),
                format!("{kind} append preimage has a truncated V1 index header"),
            ));
        }
        let first = SidecarIndexEntry::from_bytes(
            index[..PIPELINE_INDEX_ENTRY_SIZE]
                .try_into()
                .expect("fixed index entry width"),
        );
        if first.offset != u64::MAX || first.len != u64::MAX {
            return Err(Self::invalid_lane_artifact_error(
                index_path.to_path_buf(),
                format!("{kind} append preimage is missing its V1 index marker"),
            ));
        }
        let metadata = SidecarIndexEntry::from_bytes(
            index[PIPELINE_INDEX_ENTRY_SIZE..INDEXED_SIDECAR_BASE_HEADER_SIZE]
                .try_into()
                .expect("fixed V1 index metadata width"),
        );
        if metadata.len != metadata.offset ^ INDEXED_SIDECAR_BASE_CHECK_MASK {
            return Err(Self::invalid_lane_artifact_error(
                index_path.to_path_buf(),
                format!("{kind} append preimage has an invalid base-height checksum"),
            ));
        }
        let layout = SidecarIndexLayout::based(metadata.offset, u64::try_from(index.len())?)
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage index is invalid: {message}"),
                )
            })?;
        if layout.aligned_len != intent.old_index_len
            || usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
                > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
        {
            return Err(Self::invalid_lane_artifact_error(
                index_path.to_path_buf(),
                format!("{kind} append preimage index is misaligned or oversized"),
            ));
        }
        intent
            .validate_against_old_layout(Some(layout))
            .map_err(|message| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append intent names another stable preimage: {message}"),
                )
            })?;
        let mut payloads = BTreeMap::new();
        let mut indexed_end = 0_u64;
        let mut ranges = Vec::new();
        for offset in 0..layout.entry_count {
            let position = layout
                .entries_offset
                .checked_add(offset.saturating_mul(PIPELINE_INDEX_ENTRY_SIZE_U64))
                .and_then(|position| usize::try_from(position).ok())
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} append preimage index position overflows"),
                    )
                })?;
            let entry_end = position
                .checked_add(PIPELINE_INDEX_ENTRY_SIZE)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} append preimage index entry end overflows"),
                    )
                })?;
            let encoded: [u8; PIPELINE_INDEX_ENTRY_SIZE] = index
                .get(position..entry_end)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} append preimage index entry is truncated"),
                    )
                })?
                .try_into()
                .expect("validated index entry width");
            let entry = SidecarIndexEntry::from_bytes(encoded);
            if entry.len == 0 {
                if entry.offset != 0 {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("{kind} append preimage has a noncanonical empty slot"),
                    ));
                }
                continue;
            }
            let end = entry.offset.checked_add(entry.len).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage payload range overflows"),
                )
            })?;
            if entry.len > STRICT_INIT_MAX_BLOCK_BYTES || end > intent.old_data_len {
                return Err(Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage payload range is invalid"),
                ));
            }
            ranges.push((entry.offset, end));
            indexed_end = indexed_end.max(end);
            let height = layout.base_height.checked_add(offset).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    format!("{kind} append preimage height overflows"),
                )
            })?;
            let mut payload = vec![0_u8; usize::try_from(entry.len)?];
            bound
                .data
                .seek(SeekFrom::Start(entry.offset))
                .and_then(|_| bound.data.read_exact(&mut payload))
                .map_err(|error| Error::IO(error, data_path.to_path_buf()))?;
            payloads.insert(height, payload);
        }
        ranges.sort_unstable();
        if indexed_end != intent.old_data_len
            || ranges.windows(2).any(|pair| pair[1].0 < pair[0].1)
            || ranges.first().is_some_and(|range| range.0 != 0)
            || ranges.windows(2).any(|pair| pair[0].1 != pair[1].0)
            || !self.bound_progress_sidecar_unchanged(bound)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                format!("{kind} append preimage is overlapping, incomplete, or changed"),
            ));
        }
        Ok(payloads)
    }
    fn certified_bundle_pair_has_any_recovery_locked(
        &self,
        data_path: &Path,
        index_path: &Path,
    ) -> Result<bool> {
        let namespace = self.open_bound_progress_namespace(data_path, index_path)?;
        for path in [
            data_path.with_extension("norito.tmp"),
            index_path.with_extension("index.tmp"),
            index_path.with_extension("index.prepend.tmp"),
            Self::bound_progress_append_build_path(index_path),
            Self::bound_progress_append_intent_path(index_path),
        ] {
            if self
                .open_optional_bound_progress_file(&namespace, &path)?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Validate all clean certified/bundle slots before startup repair mutates
    /// either pair. Exact target append journals are accounted by the composite
    /// plan and deferred to the existing journal recovery path; every other
    /// temporary or cross-slot orphan fails closed here.
    fn preflight_certified_bundle_inventory_locked(
        &self,
        entry: &LaneConfigEntry,
        frontier: Option<&CertifiedLaneBlockArtifact>,
        frontier_source: Option<&DurableAutonomousLaneMergeSource>,
    ) -> Result<Vec<(u64, iroha_data_model::NetworkId, u64)>> {
        let (certified_data_path, certified_index_path) =
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
        let certified_recovery = if let Some(frontier) = frontier {
            self.certified_bundle_pair_has_exact_append_recovery_locked(
                &certified_data_path,
                &certified_index_path,
                frontier.proposal.descriptor.lane_block_height,
                &frontier.encode_framed()?,
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
            )?
        } else {
            if self.certified_bundle_pair_has_any_recovery_locked(
                &certified_data_path,
                &certified_index_path,
            )? {
                return Err(Self::invalid_lane_artifact_error(
                    certified_data_path,
                    "certified recovery state exists without a durable certified frontier",
                ));
            }
            None
        };
        let (bundle_data_path, bundle_index_path) =
            Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root);
        let bundle_recovery = if let Some(source) = frontier_source {
            self.certified_bundle_pair_has_exact_append_recovery_locked(
                &bundle_data_path,
                &bundle_index_path,
                source
                    .bundle
                    .certified
                    .proposal
                    .descriptor
                    .lane_block_height,
                &source.source_bundle,
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            )?
        } else {
            if self.certified_bundle_pair_has_any_recovery_locked(
                &bundle_data_path,
                &bundle_index_path,
            )? {
                return Err(Self::invalid_lane_artifact_error(
                    bundle_data_path,
                    "autonomous bundle recovery state lacks a READY-bearing frontier source",
                ));
            }
            None
        };
        // Recovery journals describe only the target append. The stable
        // prefixes beneath them remain independently security-relevant and
        // must still be enumerated before either journal is allowed to mutate
        // its pair during startup repair.
        let mut certified = BTreeMap::new();
        if let Some(recovery) = certified_recovery.as_ref() {
            for (height, bytes) in self.certified_bundle_append_preimage_payloads_locked(
                &certified_data_path,
                &certified_index_path,
                &recovery.intent,
                CertifiedLaneBlockArtifact::FORMAT_LABEL,
            )? {
                let artifact = norito::decode_canonical::<CertifiedLaneBlockArtifact>(&bytes)
                    .map_err(|error| {
                        Self::invalid_lane_artifact_error(
                            certified_data_path.clone(),
                            format!("startup certified preimage is malformed: {error}"),
                        )
                    })?;
                if artifact.encode_framed()? != bytes
                    || artifact.proposal.descriptor.lane_id != entry.lane_id
                    || artifact.proposal.descriptor.lane_block_height != height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_data_path.clone(),
                        "startup certified preimage slot is noncanonical or misaddressed",
                    ));
                }
                Self::validate_certified_lane_block_artifact(&artifact).map_err(|message| {
                    Self::invalid_lane_artifact_error(
                        certified_data_path.clone(),
                        format!("startup certified preimage slot is invalid: {message}"),
                    )
                })?;
                self.require_active_lane_artifact(entry, &artifact.proposal.descriptor)?;
                if certified.insert(height, artifact).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_index_path.clone(),
                        "startup certified pair duplicates one lane height",
                    ));
                }
            }
        } else {
            let mut certified_pair =
                self.open_bound_progress_pair(&certified_data_path, &certified_index_path)?;
            if let BoundProgressPair::Present(bound) = &mut certified_pair {
                let heights = self.bound_indexed_sidecar_payload_heights(
                    bound,
                    CertifiedLaneBlockArtifact::FORMAT_LABEL,
                    MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
                )?;
                for height in heights {
                    let artifact = self
                        .read_active_certified_lane_block_artifact_from_bound_locked(
                            entry, height, bound,
                        )
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                certified_data_path.clone(),
                                "startup certified pair contains a malformed or stale-incarnation slot",
                            )
                        })?;
                    if certified.insert(height, artifact).is_some() {
                        return Err(Self::invalid_lane_artifact_error(
                            certified_index_path.clone(),
                            "startup certified pair duplicates one lane height",
                        ));
                    }
                }
                if !self.bound_progress_sidecar_unchanged(bound) {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_index_path.clone(),
                        "startup certified pair changed during read-only preflight",
                    ));
                }
            }
        }
        let mut bundles = BTreeMap::new();
        if let Some(recovery) = bundle_recovery.as_ref() {
            for (height, bytes) in self.certified_bundle_append_preimage_payloads_locked(
                &bundle_data_path,
                &bundle_index_path,
                &recovery.intent,
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            )? {
                let bundle = norito::decode_canonical::<AutonomousLaneMergeBundleV1>(&bytes)
                    .map_err(|error| {
                        Self::invalid_lane_artifact_error(
                            bundle_data_path.clone(),
                            format!("startup bundle preimage is malformed: {error}"),
                        )
                    })?;
                if bundle.encode_framed()? != bytes
                    || bundle.certified.proposal.descriptor.lane_id != entry.lane_id
                    || bundle.certified.proposal.descriptor.lane_block_height != height
                {
                    return Err(Self::invalid_lane_artifact_error(
                        bundle_data_path.clone(),
                        "startup bundle preimage slot is noncanonical or misaddressed",
                    ));
                }
                Self::validate_autonomous_lane_merge_bundle(
                    &bundle,
                    bundle.executable_payload().network_id,
                    bundle.executable_payload().epoch,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(
                        bundle_data_path.clone(),
                        format!("startup bundle preimage slot is invalid: {message}"),
                    )
                })?;
                self.require_active_lane_artifact(entry, &bundle.certified.proposal.descriptor)?;
                if bundles.insert(height, bundle).is_some() {
                    return Err(Self::invalid_lane_artifact_error(
                        bundle_index_path.clone(),
                        "startup bundle preimage duplicates one lane height",
                    ));
                }
            }
        } else {
            let mut bundle_pair =
                self.open_bound_progress_pair(&bundle_data_path, &bundle_index_path)?;
            if let BoundProgressPair::Present(bound) = &mut bundle_pair {
                let (_, heights) = self
                    .validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            bundle_data_path.clone(),
                            message.to_owned(),
                        )
                    })?;
                for height in heights {
                    let (bundle, _) = self
                        .read_autonomous_lane_merge_bundle_from_bound_locked(
                            entry.lane_id,
                            height,
                            bound,
                        )
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                message.to_owned(),
                            )
                        })?
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                "startup bundle inventory lost an enumerated slot",
                            )
                        })?;
                    self.require_active_lane_artifact(
                        entry,
                        &bundle.certified.proposal.descriptor,
                    )?;
                    if bundles.insert(height, bundle).is_some() {
                        return Err(Self::invalid_lane_artifact_error(
                            bundle_index_path.clone(),
                            "startup bundle pair duplicates one lane height",
                        ));
                    }
                }
                if !self.bound_progress_sidecar_unchanged(bound) {
                    return Err(Self::invalid_lane_artifact_error(
                        bundle_index_path.clone(),
                        "startup bundle pair changed during read-only preflight",
                    ));
                }
            }
        }
        let Some(frontier) = frontier else {
            if !certified.is_empty() || !bundles.is_empty() {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "certified or bundle history exists without its mandatory durable frontier",
                ));
            }
            return Ok(Vec::new());
        };
        let frontier_height = frontier.proposal.descriptor.lane_block_height;
        if certified.keys().any(|height| *height > frontier_height) {
            return Err(Self::invalid_lane_artifact_error(
                certified_data_path,
                "certified history advances beyond its durable frontier",
            ));
        }
        if let Some(existing) = certified.get(&frontier_height)
            && existing != frontier
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "durable certified frontier conflicts with its indexed lane slot",
            ));
        }
        let mut persisted = Vec::new();
        for (height, bundle) in &bundles {
            let Some(artifact) = certified.get(height) else {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous bundle exists without its exact certified lane slot",
                ));
            };
            let Some(availability) = artifact.prepare_qc.payload_availability_qc.as_ref() else {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous bundle exists for an ordinary certified lane slot",
                ));
            };
            if bundle.certified != *artifact {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous bundle differs from its exact certified lane slot",
                ));
            }
            persisted.push((
                *height,
                availability.body.network_id,
                availability.body.epoch,
            ));
        }
        for (height, artifact) in &certified {
            if artifact.prepare_qc.payload_availability_qc.is_some()
                && !bundles.contains_key(height)
                && (*height != frontier_height || artifact != frontier)
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "non-frontier autonomous certificate lacks its durable bundle",
                ));
            }
        }
        Ok(persisted)
    }
    fn certified_bundle_capacity_consumed_components_locked(
        &self,
        entry: &LaneConfigEntry,
        artifact: &CertifiedLaneBlockArtifact,
        source: &DurableAutonomousLaneMergeSource,
        authority: Option<&crate::state::CertifiedLaneBlockPersistenceAuthority>,
    ) -> Result<BTreeSet<CertifiedBundleCapacityComponent>> {
        let descriptor = &artifact.proposal.descriptor;
        let mut consumed = BTreeSet::new();
        let frontier_read =
            self.read_latest_certified_lane_block_frontier_structural_locked(entry, false)?;
        if let Some(frontier_read) = frontier_read {
            let existing = &frontier_read.frontier.artifact;
            if existing == artifact {
                self.confirm_latest_certified_lane_block_frontier_read_locked(
                    entry,
                    &frontier_read.snapshot,
                )?;
                consumed.insert(CertifiedBundleCapacityComponent::LatestCertifiedFrontier);
            } else {
                let existing_descriptor = &existing.proposal.descriptor;
                let existing_is_active = self
                    .require_active_lane_artifact(entry, existing_descriptor)
                    .is_ok();
                let reset_authorized = authority.is_some_and(|authority| {
                    authority.permits_frontier_replacement(existing_descriptor, descriptor)
                });
                if (existing_is_active
                    && !reset_authorized
                    && (existing_descriptor.lane_block_height >= descriptor.lane_block_height
                        || existing_descriptor.proposal_height > descriptor.proposal_height
                        || existing_descriptor.lane_incarnation != descriptor.lane_incarnation
                        || existing_descriptor.dataspace_id != descriptor.dataspace_id))
                    || (!existing_is_active && !reset_authorized)
                {
                    return Err(Self::invalid_lane_artifact_error(
                        Self::latest_certified_lane_block_frontier_paths_for_entry(
                            entry,
                            &self.store_root,
                        )
                        .0,
                        "certified/bundle capacity identity conflicts with the durable certified frontier",
                    ));
                }
            }
        }
        let certified_bytes = artifact.encode_framed()?;
        let (certified_data_path, certified_index_path) =
            Self::certified_lane_block_paths_for_entry(entry, &self.store_root);
        let certified_has_recovery = self.certified_bundle_pair_has_exact_append_recovery_locked(
            &certified_data_path,
            &certified_index_path,
            descriptor.lane_block_height,
            &certified_bytes,
            CertifiedLaneBlockArtifact::FORMAT_LABEL,
        )?;
        if certified_has_recovery.is_none() {
            let mut pair =
                self.open_bound_progress_pair(&certified_data_path, &certified_index_path)?;
            if let BoundProgressPair::Present(bound) = &mut pair {
                self.bound_indexed_sidecar_height_range(
                    bound,
                    CertifiedLaneBlockArtifact::FORMAT_LABEL,
                )?;
                if let Some(existing) = self
                    .read_certified_lane_block_artifact_structural_from_bound_locked(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                        bound,
                    )
                {
                    if existing == *artifact {
                        if !self.bound_progress_sidecar_unchanged(bound) {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_index_path,
                                "certified pair changed during composite capacity readback",
                            ));
                        }
                        consumed.insert(CertifiedBundleCapacityComponent::CertifiedPair);
                    } else {
                        let existing_descriptor = &existing.proposal.descriptor;
                        let reset_authorized = authority.is_some_and(|authority| {
                            authority.permits_slot_replacement(existing_descriptor, descriptor)
                        });
                        let existing_is_active = self
                            .require_active_lane_artifact(entry, existing_descriptor)
                            .is_ok();
                        if existing_is_active && !reset_authorized {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_data_path,
                                "certified/bundle capacity slot aliases another certificate",
                            ));
                        }
                    }
                }
            }
        }
        let (bundle_data_path, bundle_index_path) =
            Self::autonomous_lane_merge_bundle_paths_for_entry(entry, &self.store_root);
        let bundle_has_recovery = self.certified_bundle_pair_has_exact_append_recovery_locked(
            &bundle_data_path,
            &bundle_index_path,
            descriptor.lane_block_height,
            &source.source_bundle,
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
        )?;
        if bundle_has_recovery.is_none() {
            let mut pair = self.open_bound_progress_pair(&bundle_data_path, &bundle_index_path)?;
            if let BoundProgressPair::Present(bound) = &mut pair {
                self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            bundle_data_path.clone(),
                            message.to_owned(),
                        )
                    })?;
                if let Some((existing, existing_bytes)) = self
                    .read_autonomous_lane_merge_bundle_from_bound_locked(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                        bound,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            bundle_data_path.clone(),
                            message.to_owned(),
                        )
                    })?
                {
                    if existing != source.bundle || existing_bytes != source.source_bundle {
                        return Err(Self::invalid_lane_artifact_error(
                            bundle_data_path,
                            "certified/bundle capacity slot aliases another autonomous bundle",
                        ));
                    }
                    if !self.bound_progress_sidecar_unchanged(bound) {
                        return Err(Self::invalid_lane_artifact_error(
                            bundle_index_path,
                            "autonomous bundle pair changed during composite capacity readback",
                        ));
                    }
                    consumed.insert(CertifiedBundleCapacityComponent::AutonomousBundlePair);
                }
            }
        }
        Ok(consumed)
    }
    fn ensure_certified_bundle_capacity_plan_locked(
        &self,
        plan: CertifiedBundleCapacityPlan,
        consumed: &BTreeSet<CertifiedBundleCapacityComponent>,
        pending_block_bytes: u64,
        physical_bytes: u64,
        terminal_reserved_bytes: u64,
        post_wsv_reserved_bytes: u64,
    ) -> Result<u64> {
        let mut reservations = self.certified_bundle_capacity_reservations.lock();
        if reservations.keys().any(|identity| {
            identity.lane_id == plan.identity.lane_id
                && identity.dataspace_id == plan.identity.dataspace_id
                && *identity != plan.identity
        }) {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "another certified/bundle capacity identity is still outstanding for this route",
            ));
        }
        if let Some(reservation) = reservations.get_mut(&plan.identity) {
            if reservation.plan.identity != plan.identity
                || reservation.plan.certified_bytes_hash != plan.certified_bytes_hash
                || reservation.plan.frontier_bytes_hash != plan.frontier_bytes_hash
                || reservation.plan.bundle_bytes_hash != plan.bundle_bytes_hash
                || reservation.outstanding_components.iter().any(|component| {
                    !consumed.contains(component)
                        && (reservation.plan.component_bytes.get(component)
                            != plan.component_bytes.get(component)
                            || reservation.plan.component_transient_bytes.get(component)
                                != plan.component_transient_bytes.get(component))
                })
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "certified/bundle capacity retry changed its immutable byte plan",
                ));
            }
            for component in reservation.plan.component_bytes.keys() {
                if !reservation.outstanding_components.contains(component)
                    && !consumed.contains(component)
                {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "a durability-attested certified/bundle component disappeared",
                    ));
                }
            }
            reservation
                .outstanding_components
                .retain(|component| !consumed.contains(component));
            if reservation.outstanding_components.is_empty() {
                reservations.remove(&plan.identity);
                return Ok(0);
            }
            return reservation.reserved_bytes().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "certified/bundle remaining reservation bytes overflowed",
                )
            });
        }
        let outstanding_components = plan
            .component_bytes
            .keys()
            .filter(|component| !consumed.contains(component))
            .copied()
            .collect::<BTreeSet<_>>();
        if outstanding_components.is_empty() {
            return Ok(0);
        }
        if reservations.len() >= MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle reservation inventory exceeds its hard bound",
            ));
        }
        let reservation = CertifiedBundleCapacityReservation {
            plan,
            outstanding_components,
        };
        let new_reserved = reservation.reserved_bytes().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "new certified/bundle reservation bytes overflowed",
            )
        })?;
        if self.max_disk_usage_bytes != 0 && !self.store_root.as_os_str().is_empty() {
            let existing_reserved = reservations
                .values()
                .try_fold(0_u64, |total, existing| {
                    total.checked_add(existing.reserved_bytes()?)
                })
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "existing certified/bundle reservation bytes overflowed",
                    )
                })?;
            let required = physical_bytes
                .checked_add(pending_block_bytes)
                .and_then(|bytes| bytes.checked_add(terminal_reserved_bytes))
                .and_then(|bytes| bytes.checked_add(post_wsv_reserved_bytes))
                .and_then(|bytes| {
                    bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
                })
                .and_then(|bytes| bytes.checked_add(existing_reserved))
                .and_then(|bytes| bytes.checked_add(new_reserved))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "certified/bundle configured-capacity accounting overflowed",
                    )
                })?;
            if required > self.max_disk_usage_bytes {
                return Err(Error::StorageBudgetExceeded {
                    limit: self.max_disk_usage_bytes,
                    used: physical_bytes,
                    required,
                });
            }
        }
        let identity = reservation.plan.identity;
        reservations.insert(identity, reservation);
        Ok(new_reserved)
    }
    fn ensure_certified_bundle_capacity_reservation_under_prune_guard(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
        source: &DurableAutonomousLaneMergeSource,
        authority: Option<&crate::state::CertifiedLaneBlockPersistenceAuthority>,
    ) -> Result<u64> {
        // `prune_lock` is already held. Snapshot the canonical pending budget
        // before taking geometry/sidecar so this path preserves the global
        // prune -> canonical/block metadata -> geometry -> sidecar order.
        let configured_capacity =
            self.max_disk_usage_bytes != 0 && !self.store_root.as_os_str().is_empty();
        let (pending_block_bytes, physical_bytes) = if configured_capacity {
            let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
            (
                self.pending_block_bytes(persisted_count, unindexed_bytes)?,
                self.kura_disk_usage_bytes()?,
            )
        } else {
            (0, 0)
        };
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(artifact.proposal.descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, &artifact.proposal.descriptor)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        // Snapshot other reservation families before taking this family map.
        // This avoids a post-WSV -> certified/bundle / certified/bundle ->
        // post-WSV mutex inversion while `prune_lock` keeps the aggregate
        // capacity state stable for the admission decision.
        let (terminal_reserved_bytes, post_wsv_reserved_bytes) = if configured_capacity {
            (
                self.autonomous_global_terminal_outcome_reserved_bytes_locked()?,
                self.post_wsv_lane_artifact_budget_reserved_bytes()?,
            )
        } else {
            (0, 0)
        };
        let plan = self.certified_bundle_capacity_plan(&entry, artifact, source)?;
        let consumed = self.certified_bundle_capacity_consumed_components_locked(
            &entry, artifact, source, authority,
        )?;
        self.ensure_certified_bundle_capacity_plan_locked(
            plan,
            &consumed,
            pending_block_bytes,
            physical_bytes,
            terminal_reserved_bytes,
            post_wsv_reserved_bytes,
        )
    }
    fn certified_bundle_capacity_reserved_bytes(&self) -> Result<u64> {
        self.certified_bundle_capacity_reservations
            .lock()
            .values()
            .try_fold(0_u64, |total, reservation| {
                let bytes = reservation.reserved_bytes().ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "certified/bundle reservation accounting overflowed",
                    )
                })?;
                total.checked_add(bytes).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "certified/bundle reservation total overflows",
                    )
                })
            })
    }
    fn ensure_lane_has_no_certified_bundle_capacity_reservation(
        &self,
        entry: &LaneConfigEntry,
    ) -> Result<()> {
        if self
            .certified_bundle_capacity_reservations
            .lock()
            .keys()
            .any(|identity| {
                identity.lane_id == entry.lane_id && identity.dataspace_id == entry.dataspace_id
            })
        {
            return Err(Self::invalid_lane_artifact_error(
                entry.blocks_dir(&self.store_root),
                "lane retirement is blocked by an outstanding certified/bundle capacity obligation",
            ));
        }
        Ok(())
    }
    fn consume_certified_bundle_capacity_component(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
        component: CertifiedBundleCapacityComponent,
        durable_bytes_hash: Hash,
    ) -> Result<()> {
        let descriptor = &artifact.proposal.descriptor;
        let certified_hash = Hash::new(&artifact.encode_framed()?);
        let mut reservations = self.certified_bundle_capacity_reservations.lock();
        let identities = reservations
            .keys()
            .filter(|identity| {
                identity.lane_id == descriptor.lane_id
                    && identity.dataspace_id == descriptor.dataspace_id
                    && identity.lane_incarnation == descriptor.lane_incarnation
                    && identity.proposal_height == descriptor.proposal_height
                    && identity.lane_block_height == descriptor.lane_block_height
                    && identity.lane_block_view == descriptor.lane_block_view
                    && identity.lane_block_descriptor_hash == descriptor.descriptor_hash
                    && identity.proposal_hash == artifact.proposal.proposal_hash
            })
            .copied()
            .collect::<Vec<_>>();
        if identities.len() > 1 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle component readback aliases multiple reservation identities",
            ));
        }
        let Some(identity) = identities.first().copied() else {
            return Ok(());
        };
        let reservation = reservations
            .get_mut(&identity)
            .expect("collected certified/bundle reservation identity exists");
        if reservation.plan.certified_bytes_hash != certified_hash {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle component readback changed its exact certificate",
            ));
        }
        let expected_hash = match component {
            CertifiedBundleCapacityComponent::LatestCertifiedFrontier => {
                reservation.plan.frontier_bytes_hash
            }
            CertifiedBundleCapacityComponent::CertifiedPair => {
                reservation.plan.certified_bytes_hash
            }
            CertifiedBundleCapacityComponent::AutonomousBundlePair => {
                reservation.plan.bundle_bytes_hash
            }
        };
        if durable_bytes_hash != expected_hash {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "certified/bundle component durable bytes differ from their reservation plan",
            ));
        }
        reservation.outstanding_components.remove(&component);
        if reservation.outstanding_components.is_empty() {
            reservations.remove(&identity);
        }
        Ok(())
    }
    fn consume_certified_bundle_frontier_capacity(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
    ) -> Result<()> {
        let frontier =
            LatestCertifiedLaneBlockFrontierV1::new(artifact.clone()).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "durable certified frontier cannot reconstruct its capacity identity",
                )
            })?;
        let bytes = norito::encode_canonical(&frontier).map_err(Error::NoritoFrame)?;
        self.consume_certified_bundle_capacity_component(
            artifact,
            CertifiedBundleCapacityComponent::LatestCertifiedFrontier,
            Hash::new(&bytes),
        )
    }
    fn consume_certified_bundle_pair_capacity(
        &self,
        artifact: &CertifiedLaneBlockArtifact,
    ) -> Result<()> {
        let bytes = artifact.encode_framed()?;
        self.consume_certified_bundle_capacity_component(
            artifact,
            CertifiedBundleCapacityComponent::CertifiedPair,
            Hash::new(&bytes),
        )
    }
    fn consume_autonomous_bundle_pair_capacity(
        &self,
        source: &DurableAutonomousLaneMergeSource,
    ) -> Result<()> {
        self.consume_certified_bundle_capacity_component(
            &source.bundle.certified,
            CertifiedBundleCapacityComponent::AutonomousBundlePair,
            Hash::new(&source.source_bundle),
        )
    }
    /// Reconstruct every active READY-bearing publication obligation from its
    /// authenticated durable frontier before either certified-pair or bundle
    /// repair may mutate storage.
    fn rebuild_certified_bundle_capacity_reservations_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let entries = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            self.lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let mut rebuilt =
            BTreeMap::<CertifiedBundleCapacityIdentity, CertifiedBundleCapacityReservation>::new();
        for entry in entries {
            let artifact = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let active = self.lane_storage_entry(entry.lane_id)?;
                if active != entry {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "lane geometry changed during certified/bundle reservation rebuild",
                    ));
                }
                let _sidecar_guard = self.sidecar_lock.lock();
                let frontier =
                    self.read_latest_certified_lane_block_frontier_locked(&active, true)?;
                if let Some(frontier) = frontier {
                    self.confirm_latest_certified_lane_block_frontier_read_locked(
                        &active,
                        &frontier.snapshot,
                    )?;
                    Some(frontier.frontier.artifact)
                } else {
                    None
                }
            };
            let Some(artifact) = artifact else {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let active = self.lane_storage_entry(entry.lane_id)?;
                let _sidecar_guard = self.sidecar_lock.lock();
                self.preflight_certified_bundle_inventory_locked(&active, None, None)?;
                continue;
            };
            let Some(availability) = artifact.prepare_qc.payload_availability_qc.as_ref() else {
                let persisted = {
                    let _geometry_guard = self.lane_geometry_lock.lock();
                    let active = self.lane_storage_entry(entry.lane_id)?;
                    self.require_active_lane_artifact(&active, &artifact.proposal.descriptor)?;
                    let _sidecar_guard = self.sidecar_lock.lock();
                    self.preflight_certified_bundle_inventory_locked(
                        &active,
                        Some(&artifact),
                        None,
                    )?
                };
                for (height, network_id, epoch) in persisted {
                    self.durable_autonomous_lane_merge_source_under_prune_guard(
                        entry.lane_id,
                        height,
                        network_id,
                        epoch,
                        None,
                        true,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            format!("startup persisted autonomous bundle is invalid: {message}"),
                        )
                    })?;
                }
                continue;
            };
            let descriptor = &artifact.proposal.descriptor;
            let source = self
                .durable_autonomous_lane_merge_source_under_prune_guard(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    availability.body.network_id,
                    availability.body.epoch,
                    Some(&artifact),
                    false,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        format!(
                            "certified/bundle reservation startup source is invalid: {message}"
                        ),
                    )
                })?;
            let (plan, consumed, persisted) = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let active = self.lane_storage_entry(descriptor.lane_id)?;
                self.require_active_lane_artifact(&active, descriptor)?;
                let _sidecar_guard = self.sidecar_lock.lock();
                let plan = self.certified_bundle_capacity_plan(&active, &artifact, &source)?;
                let consumed = self.certified_bundle_capacity_consumed_components_locked(
                    &active, &artifact, &source, None,
                )?;
                let persisted = self.preflight_certified_bundle_inventory_locked(
                    &active,
                    Some(&artifact),
                    Some(&source),
                )?;
                (plan, consumed, persisted)
            };
            for (height, network_id, epoch) in persisted {
                self.durable_autonomous_lane_merge_source_under_prune_guard(
                    entry.lane_id,
                    height,
                    network_id,
                    epoch,
                    None,
                    true,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        format!("startup persisted autonomous bundle is invalid: {message}"),
                    )
                })?;
            }
            if rebuilt.keys().any(|identity| {
                identity.lane_id == plan.identity.lane_id
                    && identity.dataspace_id == plan.identity.dataspace_id
                    && *identity != plan.identity
            }) {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "startup certified/bundle inventory has conflicting identities for one route",
                ));
            }
            let outstanding_components = plan
                .component_bytes
                .keys()
                .filter(|component| !consumed.contains(component))
                .copied()
                .collect::<BTreeSet<_>>();
            if outstanding_components.is_empty() {
                continue;
            }
            if rebuilt.len() >= MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "startup certified/bundle reservation inventory exceeds its hard bound",
                ));
            }
            let identity = plan.identity;
            if rebuilt
                .insert(
                    identity,
                    CertifiedBundleCapacityReservation {
                        plan,
                        outstanding_components,
                    },
                )
                .is_some()
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "startup certified/bundle reservation inventory duplicates one identity",
                ));
            }
        }
        if self.max_disk_usage_bytes != 0 && !self.store_root.as_os_str().is_empty() {
            // `used` already contains any authenticated partial append and its
            // intent/build files.  Keep the in-memory reservation at its full
            // envelope, but credit those exact identity-local bytes once for
            // startup admission.  This computes the larger of the physical
            // crash stage and the post-recovery publication envelope instead
            // of counting the same bytes twice.
            let rebuilt_effective_reserved = rebuilt
                .values()
                .try_fold(0_u64, |total, reservation| {
                    let reserved = reservation.reserved_bytes()?;
                    total.checked_add(reserved.saturating_sub(
                        reservation.plan.startup_physical_credit_bytes.min(reserved),
                    ))
                })
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup certified/bundle effective reservation total overflows",
                    )
                })?;
            let used = self.kura_disk_usage_bytes()?;
            let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
            let pending_block_bytes = self.pending_block_bytes(persisted_count, unindexed_bytes)?;
            let terminal = self.autonomous_global_terminal_outcome_reserved_bytes()?;
            let post_wsv = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
            let required = used
                .checked_add(pending_block_bytes)
                .and_then(|bytes| bytes.checked_add(terminal))
                .and_then(|bytes| bytes.checked_add(post_wsv))
                .and_then(|bytes| {
                    bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
                })
                .and_then(|bytes| bytes.checked_add(rebuilt_effective_reserved))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup certified/bundle configured-capacity accounting overflowed",
                    )
                })?;
            if required > self.max_disk_usage_bytes {
                return Err(Error::StorageBudgetExceeded {
                    limit: self.max_disk_usage_bytes,
                    used,
                    required,
                });
            }
        }
        *self.certified_bundle_capacity_reservations.lock() = rebuilt;
        Ok(())
    }
}
