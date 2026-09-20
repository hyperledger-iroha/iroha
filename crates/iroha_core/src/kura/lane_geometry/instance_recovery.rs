//! Physical recovery of exact immutable instances already owned by a durable journal.
use super::super::StartupRecoveryMutationAuthority;
use super::*;

impl Kura {
    /// Finish only journal-admitted empty creations before any auxiliary inventory reads.
    /// This does not publish an active catalog or derive an identity from configuration.
    /// The ordinary startup token rejects provisional imports; their authenticated
    /// finalizer must supply its existing instance-bound mutation authority instead.
    pub(in crate::kura) fn recover_journal_owned_lane_instances_on_startup(
        &self,
        authority: StartupRecoveryMutationAuthority<'_>,
    ) -> Result<()> {
        authority.validate_for(self)?;
        let _prune_guard = self.prune_lock.lock();
        let _canonical_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        authority.validate_for(self)?;
        let journal = self.read_lane_geometry_journal()?;
        if journal.configured_primary_binding.is_none()
            && journal.records.is_empty()
            && journal.checkpoint.is_none()
            && self.exact_durable_blocks_count()? == 0
        {
            return Ok(());
        }
        let locations = self.native_amx_evidence_physical_locations_in_journal(&journal)?;
        let mut resumable = journal
            .records
            .iter()
            .filter(|record| record.phase == LaneGeometryPhase::Intent)
            .flat_map(|record| &record.operations)
            .filter_map(|operation| operation.updated.as_ref())
            .map(LaneGeometryBinding::identity)
            .collect::<BTreeSet<_>>();
        // A later intent cannot recreate an object that an earlier committed
        // reference, snapshot image or predecessor already requires to exist.
        for binding in journal
            .records
            .iter()
            .flat_map(|record| {
                record.previous_bindings.iter().chain(
                    record
                        .updated_bindings
                        .iter()
                        .filter(move |_| record.phase != LaneGeometryPhase::Intent),
                )
            })
            .chain(journal.checkpoint.iter().flat_map(|checkpoint| {
                checkpoint
                    .bindings
                    .iter()
                    .chain(&checkpoint.recovery_bindings)
            }))
            .chain(journal.pending_archive_gc.iter().flat_map(|pending| {
                pending
                    .intent
                    .previous_bindings
                    .iter()
                    .chain(&pending.intent.updated_bindings)
            }))
        {
            resumable.remove(&binding.identity());
        }
        let h0_creation = self.exact_durable_blocks_count()? == 0
            && journal.records.is_empty()
            && journal.checkpoint.is_none()
            && journal.pending_archive_gc.is_empty();
        if h0_creation {
            if let Some(primary) = &journal.configured_primary_binding {
                let blocks = self.binding_blocks_path(primary);
                let merge = self.binding_merge_path(primary);
                if !self.validate_path_kind(&blocks, true)?
                    || !self.validate_path_kind(&merge, false)?
                {
                    resumable.insert(primary.identity());
                }
            }
        }
        // Validate every completed/rolled-back object before writing any new
        // target. An absent auxiliary directory never excuses a missing pair.
        for location in &locations {
            self.require_lane_instance_network(&location.binding)?;
            if !resumable.contains(&location.binding.identity()) {
                self.authenticate_existing_lane_instance_on_startup(&location.binding)?;
            }
        }
        for location in &locations {
            if resumable.contains(&location.binding.identity()) {
                self.prepare_journal_owned_lane_instance(
                    &location.binding,
                    GeometryEvidencePolicy::AllowJournalIntentProvisioning,
                )?;
                self.require_retained_lane_instance_pair(&location.binding)?;
            }
        }
        // No phase, current reference, or active producer capability is changed.
        Ok(())
    }

    fn authenticate_existing_lane_instance_on_startup(
        &self,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        let mut captured = self.preflight_lane_instance_open(binding)?;
        self.reverify_lane_instance_parents(&captured)?;
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        #[cfg(test)]
        super::super::configured_primary_open_identity_swap_boundary(&blocks)?;
        self.reverify_lane_instance_parents(&captured)?;
        Self::reverify_storage_open_path(&mut captured.paths, &blocks, true, false)?;
        #[cfg(test)]
        super::super::configured_primary_open_identity_swap_boundary(&merge)?;
        self.reverify_lane_instance_parents(&captured)?;
        Self::reverify_storage_open_path(&mut captured.paths, &merge, false, false)?;
        self.require_retained_lane_instance_pair(binding)?;
        self.reverify_lane_instance_parents(&captured)?;
        Self::reverify_storage_open_path(&mut captured.paths, &blocks, true, false)?;
        Self::reverify_storage_open_path(&mut captured.paths, &merge, false, false)
    }

    pub(super) fn require_lane_instance_network(
        &self,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        // Before State is available the durable, configured-catalog-authenticated
        // journal is the recovery owner. Once bound, its original network must
        // also equal the independently authenticated State/snapshot network.
        if self
            .lane_storage_network
            .lock()
            .is_some_and(|network| network != binding.network_id)
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retained lane instance differs from the authenticated network",
            ));
        }
        Ok(())
    }

    pub(super) fn require_retained_lane_instance_pair(
        &self,
        binding: &LaneGeometryBinding,
    ) -> Result<()> {
        self.require_lane_instance_network(binding)?;
        let blocks = self.binding_blocks_path(binding);
        let merge = self.binding_merge_path(binding);
        self.require_complete_geometry_binding_at(binding, &blocks, &merge)?;
        // TODO: Remove these unused base journals and the paired empty merge log.
        // Instance evidence is owned by lane_artifacts; canonical bodies/merge data
        // have their separate stable owner. Until that schema cutover, authenticate
        // the exact empty scaffolding without freezing mutable sidecar contents.
        for name in [DATA_FILE_NAME, INDEX_FILE_NAME, HASHES_FILE_NAME] {
            read_preflight_file_bounded(&blocks.join(name), 0)?;
        }
        read_preflight_file_bounded(&merge, 0)?;
        let count_path = blocks.join(COUNT_FILE_NAME);
        let bytes = read_preflight_file_bounded(&count_path, MAX_BLOCK_STORE_COMMIT_MARKER_BYTES)?;
        let marker = norito::decode_canonical::<BlockStoreCommitMarker>(&bytes)
            .map_err(Error::NoritoFrame)?;
        if marker.version != BlockStoreCommitMarker::VERSION
            || marker.count != 0
            || marker.tip_hash.is_some()
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "immutable instance base journal is not the canonical empty scaffold",
            ));
        }
        if !self.lane_marker_is_unsealed_at(&blocks, binding)? {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "retained lane instance is already owned by collection",
            ));
        }
        Ok(())
    }
}
