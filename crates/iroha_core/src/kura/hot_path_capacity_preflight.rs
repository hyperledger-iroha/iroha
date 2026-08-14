/// One prepared canonical association stage and its exact physical addition.
struct CanonicalAssociationStagePublication {
    bytes: Vec<u8>,
    additional_bytes: u64,
}
/// Ordered physical-byte projection for a group of claim mutations.
///
/// `current_delta` describes the durable namespace after the mutations already
/// projected, relative to the namespace inspected before the first mutation.
/// `peak_delta` also observes atomic temporaries while the replaced stable file
/// is still present.
#[derive(Default)]
struct AutonomousClaimMutationPeak {
    current_delta: i128,
    peak_delta: i128,
}
impl AutonomousClaimMutationPeak {
    fn add_physical(&mut self, bytes: u64) -> std::result::Result<(), &'static str> {
        self.current_delta = self
            .current_delta
            .checked_add(i128::from(bytes))
            .ok_or("autonomous claim capacity addition overflows")?;
        self.peak_delta = self.peak_delta.max(self.current_delta);
        Ok(())
    }
    fn remove_physical(&mut self, bytes: u64) -> std::result::Result<(), &'static str> {
        self.current_delta = self
            .current_delta
            .checked_sub(i128::from(bytes))
            .ok_or("autonomous claim capacity removal overflows")?;
        Ok(())
    }
    /// Project promotion of an already-accounted named temp over a stable main.
    fn promote_named_temp_over_main(
        &mut self,
        replaced_main_bytes: u64,
    ) -> std::result::Result<(), &'static str> {
        self.remove_physical(replaced_main_bytes)
    }
    /// Project an atomic replacement: the new temp overlaps the old main until rename.
    fn atomic_replace(
        &mut self,
        replaced_main_bytes: u64,
        replacement_bytes: u64,
    ) -> std::result::Result<(), &'static str> {
        self.add_physical(replacement_bytes)?;
        self.remove_physical(replaced_main_bytes)
    }
    fn additional_peak_bytes(&self) -> std::result::Result<u64, &'static str> {
        u64::try_from(self.peak_delta).map_err(|_| "autonomous claim capacity peak is outside u64")
    }
}
impl Kura {
    fn prepare_canonical_association_stage(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<CanonicalAssociationStagePublication> {
        let block_wire = block.encode_wire()?;
        let stage = CanonicalAssociationStageV1 {
            format_version: CANONICAL_ASSOCIATION_STAGE_VERSION,
            height: block.header().height().get(),
            block_hash: block.hash(),
            canonical_wire_hash: Hash::new(&block_wire),
            block_wire,
            merge_entry: merge_entry.cloned(),
        };
        let _ = self.validate_canonical_association_stage(&stage)?;
        let bytes = norito::encode_canonical(&stage).map_err(Error::NoritoFrame)?;
        let encoded_len = u64::try_from(bytes.len())?;
        if encoded_len > MAX_CANONICAL_ASSOCIATION_STAGE_BYTES {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "canonical association stage exceeds its hard size limit",
                ),
                self.canonical_association_stage_path(),
            ));
        }
        let additional_bytes = match self.read_canonical_association_stage()? {
            Some(existing) if existing == stage => 0,
            Some(_) => {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::AlreadyExists,
                        "a conflicting canonical association stage already exists",
                    ),
                    self.canonical_association_stage_path(),
                ));
            }
            None => encoded_len,
        };
        Ok(CanonicalAssociationStagePublication {
            bytes,
            additional_bytes,
        })
    }
    /// Return the exact encoded bytes which would be added by the association stage.
    fn canonical_association_stage_additional_bytes(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<u64> {
        Ok(self
            .prepare_canonical_association_stage(block, merge_entry)?
            .additional_bytes)
    }
    /// Validate the complete live namespace and projected crash-staging peak
    /// before claim preparation mutates any file.
    fn preflight_autonomous_lane_entrypoint_claims_locked(
        &self,
        pending_canonical_bytes: u64,
        payload: &LaneExecutablePayloadV1,
        max_files: usize,
    ) -> Result<()> {
        let mut projected_files =
            self.inspect_autonomous_lane_entrypoint_claim_inventory(max_files)?;
        let mut unique_paths = BTreeSet::new();
        let capacity_path = self.store_root.join("blocks");
        let mut capacity = AutonomousClaimMutationPeak::default();
        for entrypoint_hash in &payload.entrypoint_hashes {
            let incoming = AutonomousLaneEntrypointClaimV3::new(payload, *entrypoint_hash);
            let path = Self::autonomous_lane_entrypoint_claim_path(
                &self.store_root,
                &incoming.network_id,
                &incoming.entrypoint_hash,
            );
            if !unique_paths.insert(path.clone()) {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous payload repeats an entrypoint claim path",
                ));
            }
            let temp_path = Self::autonomous_lane_entrypoint_claim_temp_path(&path);
            let existing = if Self::autonomous_lane_entrypoint_claim_file_exists(&path)? {
                let existing = Self::decode_autonomous_lane_entrypoint_claim(&path)
                    .map_err(|message| Self::invalid_lane_artifact_error(path.clone(), message))?;
                if !self.autonomous_lane_entrypoint_claim_path_matches(&existing, &path) {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous entrypoint claim has a mismatched hash path",
                    ));
                }
                Some(existing)
            } else {
                None
            };
            let existing_bytes = if existing.is_some() {
                Self::file_len_or_zero(&path)?
            } else {
                0
            };
            let pending = if Self::autonomous_lane_entrypoint_claim_file_exists(&temp_path)? {
                let pending = Self::decode_autonomous_lane_entrypoint_claim(&temp_path).map_err(
                    |message| Self::invalid_lane_artifact_error(temp_path.clone(), message),
                )?;
                if !self.autonomous_lane_entrypoint_claim_path_matches(&pending, &path)
                    || !matches!(pending.state, AutonomousLaneEntrypointClaimStateV3::Active)
                {
                    return Err(Self::invalid_lane_artifact_error(
                        temp_path,
                        "autonomous entrypoint temp claim has a mismatched or released identity",
                    ));
                }
                Some(pending)
            } else {
                None
            };
            let pending_bytes = if pending.is_some() {
                Self::file_len_or_zero(&temp_path)?
            } else {
                0
            };
            if existing
                .as_ref()
                .is_some_and(|claim| claim.active_for_payload(payload))
            {
                if let Some(pending) = pending {
                    if self.autonomous_lane_claim_target_may_be_durable_locked(&pending)
                        && !pending.active_for_payload(payload)
                    {
                        return Err(Self::invalid_lane_artifact_error(
                            temp_path,
                            "durable autonomous entrypoint temp conflicts with the exact main owner",
                        ));
                    }
                    projected_files = projected_files.checked_sub(1).ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous claim inventory projection underflows",
                        )
                    })?;
                    capacity.remove_physical(pending_bytes).map_err(|message| {
                        Self::invalid_lane_artifact_error(capacity_path.clone(), message)
                    })?;
                }
                continue;
            }
            if existing
                .as_ref()
                .is_some_and(|claim| claim.owns_payload(payload))
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous entrypoint belongs to a durably retired lane payload",
                ));
            }
            if let Some(existing) = existing.as_ref()
                && !matches!(
                    existing.state,
                    AutonomousLaneEntrypointClaimStateV3::Released(_)
                )
                && !self
                    .autonomous_lane_entrypoint_claim_is_superseded_by_active_recreation_locked(
                        existing, &incoming,
                    )?
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous entrypoint is already claimed by another lane payload",
                ));
            }
            if let Some(pending) = pending {
                if self.autonomous_lane_claim_target_may_be_durable_locked(&pending) {
                    if !pending.active_for_payload(payload) {
                        return Err(Self::invalid_lane_artifact_error(
                            path,
                            "autonomous entrypoint is already claimed by another lane payload",
                        ));
                    }
                    if existing.is_some() {
                        projected_files = projected_files.checked_sub(1).ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                path.clone(),
                                "autonomous claim inventory projection underflows",
                            )
                        })?;
                    }
                    capacity
                        .promote_named_temp_over_main(existing_bytes)
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(capacity_path.clone(), message)
                        })?;
                    continue;
                }
                // The orphan temp is removed before the exact replacement is
                // staged, so this path does not raise the inventory peak.
                capacity.remove_physical(pending_bytes).map_err(|message| {
                    Self::invalid_lane_artifact_error(capacity_path.clone(), message)
                })?;
            } else {
                projected_files = projected_files.checked_add(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "autonomous claim inventory projection overflows",
                    )
                })?;
                if projected_files > max_files {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.join("blocks"),
                        "autonomous claim staging would exceed its hard file-count limit",
                    ));
                }
            }
            let bytes = norito::encode_canonical(&incoming).map_err(Error::NoritoFrame)?;
            if bytes.is_empty() || bytes.len() > AUTONOMOUS_LANE_ENTRYPOINT_CLAIM_MAX_BYTES {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous entrypoint claim exceeds hard byte limit",
                ));
            }
            capacity
                .add_physical(u64::try_from(bytes.len())?)
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(capacity_path.clone(), message)
                })?;
        }
        let additional_peak_bytes = capacity
            .additional_peak_bytes()
            .map_err(|message| Self::invalid_lane_artifact_error(capacity_path.clone(), message))?;
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            additional_peak_bytes,
            false,
            false,
            &capacity_path,
        )?;
        Ok(())
    }
}
