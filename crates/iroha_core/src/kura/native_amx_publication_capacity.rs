/// Exact canonical carrier whose Native outputs remain owned across publication phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NativeAmxPublicationCarrier {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    executed_wire_hash: Hash,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NativeAmxPublicationRoute {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    incarnation: Hash,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NativeAmxPublicationComponent {
    Manifest,
    Receipt,
    Latest,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAmxRoutePublicationCapacity {
    participant_height: u64,
    proposal_hash: Hash,
    settlement_hash: HashOf<iroha_data_model::block::consensus::NativeAmxParticipantSettlement>,
    component_bytes: BTreeMap<NativeAmxPublicationComponent, u64>,
    outstanding_components: BTreeSet<NativeAmxPublicationComponent>,
    component_allocation_bytes: BTreeMap<NativeAmxPublicationComponent, u64>,
    prune_journal_bytes: u64,
    // Authenticated prune journals and latest temporaries are physical, but their
    // cleanup still owns a resident operation until its exact durability barrier.
    physical_cleanup_pending: bool,
    cleanup_complete: bool,
}
impl NativeAmxRoutePublicationCapacity {
    fn reserved_bytes(&self) -> Option<u64> {
        if self.cleanup_complete {
            return Some(0);
        }
        self.outstanding_components
            .iter()
            .try_fold(self.prune_journal_bytes, |total, kind| {
                total.checked_add(*self.component_allocation_bytes.get(kind)?)
            })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAmxPublicationCapacityReservation {
    index_record: Option<NativeAmxPublicationIndexRecord>,
    index_additional_bytes: u64,
    routes: BTreeMap<NativeAmxPublicationRoute, NativeAmxRoutePublicationCapacity>,
}
impl NativeAmxPublicationCapacityReservation {
    fn reserved_bytes(&self) -> Option<u64> {
        self.routes
            .values()
            .try_fold(self.index_additional_bytes, |total, route| {
                total.checked_add(route.reserved_bytes()?)
            })
    }
}
/// Roll back only a newly admitted carrier proven not to have entered durable block I/O.
/// An ambiguous write or a committed carrier keeps its envelope for exact recovery.
struct NativeAmxStoreCapacityGuard<'a> {
    kura: &'a Kura,
    carrier: NativeAmxPublicationCarrier,
    rollback_new_reservation: bool,
    index_write_started: bool,
    canonical_write_started: bool,
    rollback_proven: bool,
    index_publication: NativeAmxPublicationIndexPublication,
}
impl NativeAmxStoreCapacityGuard<'_> {
    fn durable_write_started(&mut self) {
        self.rollback_new_reservation = false;
        self.canonical_write_started = true;
    }
    /// The canonical operation selected the old marker. Any remaining cleanup
    /// failure must preserve this index and stop later mutation of that marker.
    fn canonical_write_proven_uncommitted(&mut self) {
        self.canonical_write_started = false;
    }
    /// Persist the exact discovery record before any canonical or lane-artifact write.
    fn publish_pending_index(&mut self) -> Result<()> {
        // An error can occur after a directory entry became durable. Keep ownership
        // until exact removal proves rollback; Drop cannot infer publication failure.
        self.rollback_new_reservation = false;
        self.index_write_started = true;
        self.kura
            .publish_native_amx_publication_index(&self.index_publication)?;
        let mut reservations = self
            .kura
            .native_amx_publication_capacity_reservations
            .lock();
        let mut reservation = reservations.get_mut(&self.carrier).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native AMX index publication lost its capacity owner".to_owned(),
            )
        })?;
        if reservation.index_record.as_ref() != Some(&self.index_publication.record) {
            return Err(Error::PruneIntentConflict(
                "Native AMX index publication changed its exact owner".to_owned(),
            ));
        }
        // The successfully published record is now counted by physical disk usage.
        reservation.index_additional_bytes = 0;
        Ok(())
    }
    fn finish_exact_replacement_retirement(&self, block: &SignedBlock) -> Result<()> {
        if let Some(old) = self.index_publication.record.replaced {
            self.kura
                .complete_native_amx_replacement_capacity(old, block)?;
        }
        Ok(())
    }
    fn rollback_after_proven_uncommitted_write(&mut self) -> Result<()> {
        self.rollback_new_reservation = false;
        self.canonical_write_proven_uncommitted();
        self.kura
            .remove_native_amx_publication_index_exact(&self.index_publication.record)?;
        self.kura
            .native_amx_publication_capacity_reservations
            .lock()
            .remove(&self.carrier);
        self.rollback_proven = true;
        Ok(())
    }
}
impl Drop for NativeAmxStoreCapacityGuard<'_> {
    fn drop(&mut self) {
        if self.index_write_started && !self.canonical_write_started && !self.rollback_proven {
            // A later ordinary append must not erase the exact before-marker
            // proof for this abandoned precommit record. Cold recovery classifies
            // and durably removes it before admitting another canonical mutation.
            self.kura.poison_canonical_storage(
                "Native publication abandoned before canonical commit",
                &Error::PruneIntentConflict(
                    "pending Native publication index requires exact precommit recovery".to_owned(),
                ),
            );
        }
        if self.rollback_new_reservation {
            self.kura
                .native_amx_publication_capacity_reservations
                .lock()
                .remove(&self.carrier);
        }
    }
}
#[derive(Clone, Copy)]
enum NativeAmxPublicationStorage {
    Active,
    JournalPhysical,
}
impl Kura {
    fn native_amx_publication_carrier(block: &SignedBlock) -> Result<NativeAmxPublicationCarrier> {
        Ok(NativeAmxPublicationCarrier {
            height: block.header().height().get(),
            block_hash: block.hash(),
            executed_wire_hash: Hash::new(block.encode_wire()?),
        })
    }
    fn native_amx_publication_capacity_reserved_bytes(&self) -> Result<u64> {
        self.native_amx_publication_capacity_reservations
            .lock()
            .values()
            .try_fold(0_u64, |total, reservation| {
                total
                    .checked_add(reservation.reserved_bytes().ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native AMX publication reservation overflowed".to_owned(),
                        )
                    })?)
                    .ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native AMX publication reservations overflowed".to_owned(),
                        )
                    })
            })
    }
    /// Sum separate typed carrier families without holding their map locks together.
    fn lane_publication_budget_reserved_bytes(&self) -> Result<u64> {
        let merge = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
        let native = self.native_amx_publication_capacity_reserved_bytes()?;
        merge.checked_add(native).ok_or_else(|| {
            Error::PruneIntentConflict("lane publication reservation sum overflowed".to_owned())
        })
    }
    /// Read the stable route inventory without creating a prospective publication namespace.
    /// Callers hold prune, canonical, geometry and sidecar ownership in that order.
    fn native_amx_publication_inventory_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
    ) -> Result<Option<(BoundProgressNamespace, NativeAmxEvidenceInventory)>> {
        let manifest =
            Self::native_amx_application_manifest_path_for_entry(entry, &self.store_root, 1);
        let receipt =
            Self::native_amx_participant_receipt_path_for_entry(entry, &self.store_root, 1);
        if self.bound_progress_sidecar_directory_is_absent(&manifest, &receipt)? {
            return Ok(None);
        }
        let namespace = self.native_amx_evidence_namespace_for_entry(entry)?;
        let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;
        Ok(Some((namespace, inventory)))
    }
    fn native_amx_route_publication_capacity_locked(
        &self,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Result<Option<(NativeAmxPublicationRoute, NativeAmxRoutePublicationCapacity)>> {
        self.native_amx_route_publication_capacity_for_storage_locked(
            manifest,
            receipt,
            NativeAmxPublicationStorage::Active,
        )
    }
    fn native_amx_route_publication_capacity_for_storage_locked(
        &self,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
        storage: NativeAmxPublicationStorage,
    ) -> Result<Option<(NativeAmxPublicationRoute, NativeAmxRoutePublicationCapacity)>> {
        let descriptor = &receipt.participant_proposal.descriptor;
        match storage {
            NativeAmxPublicationStorage::Active => {
                let entry = self.lane_storage_entry(descriptor.lane_id)?;
                self.native_amx_route_publication_capacity_at_target_locked(
                    &entry, manifest, receipt,
                )
            }
            NativeAmxPublicationStorage::JournalPhysical => {
                let target =
                    self.native_amx_reservation_physical_target_from_journal(descriptor)?;
                let result = self.native_amx_route_publication_capacity_at_target_locked(
                    &target, manifest, receipt,
                )?;
                self.require_native_amx_reservation_physical_target(&target)?;
                Ok(result)
            }
        }
    }
    fn native_amx_route_publication_capacity_at_target_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Result<Option<(NativeAmxPublicationRoute, NativeAmxRoutePublicationCapacity)>> {
        let descriptor = &receipt.participant_proposal.descriptor;
        self.require_active_lane_artifact(entry, descriptor)?;
        let route = NativeAmxPublicationRoute {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            incarnation: descriptor.lane_incarnation,
        };
        let mut manifests = BTreeMap::new();
        let mut receipts = BTreeMap::new();
        let mut latest = None;
        let mut latest_temporary = None;
        let mut temporary_manifests = BTreeMap::new();
        let mut temporary_receipts = BTreeMap::new();
        let mut pending_prune = None;
        if let Some((namespace, inventory)) =
            self.native_amx_publication_inventory_locked(&entry)?
        {
            for (height, file) in &inventory.manifests {
                manifests.insert(
                    *height,
                    self.decode_native_amx_manifest_file_locked(&entry, &namespace, file)?,
                );
            }
            for (height, file) in &inventory.receipts {
                receipts.insert(
                    *height,
                    self.decode_native_amx_receipt_file_locked(&entry, &namespace, file)?,
                );
            }
            for file in inventory.temporaries.values() {
                match file.kind {
                    NativeAmxEvidenceKind::Manifest => {
                        temporary_manifests.insert(
                            file.participant_height,
                            self.decode_native_amx_manifest_file_locked(&entry, &namespace, file)?,
                        );
                    }
                    NativeAmxEvidenceKind::Receipt => {
                        temporary_receipts.insert(
                            file.participant_height,
                            self.decode_native_amx_receipt_file_locked(&entry, &namespace, file)?,
                        );
                    }
                }
            }
            let path = Self::native_amx_participant_receipt_latest_index_path_for_entry(
                &entry,
                &self.store_root,
            );
            latest = self.decode_bound_native_amx_participant_receipt_latest_index_locked(
                &entry, &path, &namespace,
            )?;
            if let Some(bytes) = self.native_amx_latest_index_temp_bytes_locked(&namespace)? {
                self.require_native_amx_latest_index_temp_recovery_unambiguous_locked(&namespace)?;
                let path = namespace
                    .data_path
                    .parent()
                    .expect("bound Native directory")
                    .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);
                latest_temporary = Some(
                    Self::decode_native_amx_participant_receipt_latest_index_bytes_for_route(
                        entry.lane_id(),
                        entry.dataspace_id(),
                        &path,
                        &bytes,
                    )?,
                );
            }
            let directory = namespace
                .data_path
                .parent()
                .expect("bound Native namespace has a parent");
            for filename in [
                NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE,
                NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE,
            ] {
                let path = directory.join(filename);
                if let Some(bytes) = self.read_bound_regular_file_bytes_locked(
                    &namespace,
                    &path,
                    self.native_amx_evidence_prune_intent_max_bytes(),
                    "Native AMX reserved prune journal",
                )? {
                    let intent =
                        Self::decode_native_amx_evidence_prune_intent_bytes(&path, &bytes)?;
                    self.validate_native_amx_evidence_prune_intent_locked(
                        &entry, &namespace, &intent,
                    )?;
                    if pending_prune.as_ref().is_some_and(|other| other != &intent) {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX reservation found conflicting prune journals".to_owned(),
                        ));
                    }
                    pending_prune = Some(intent);
                }
            }
            if latest
                .is_some_and(|identity| identity.lane_block_height > descriptor.lane_block_height)
            {
                // History may be skipped only after the later frontier's full authority,
                // cleanup and namespace durability establish an exact completion tombstone.
                self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;
                self.require_native_amx_latest_index_temp_absent_locked(&namespace)?;
                let strict = self.inventory_native_amx_evidence_files_locked(&namespace, false)?;
                let protected = self.derive_native_amx_evidence_prune_protected_latest_locked(
                    &entry, &namespace, &strict,
                )?;
                let (newest_manifest, newest_receipt) = self
                    .native_amx_fully_authenticated_evidence_for_latest_locked(
                        &entry,
                        &namespace,
                        protected.identity,
                    )?;
                if !self.native_amx_publication_wsv_join_is_complete_locked(
                    &newest_manifest,
                    &newest_receipt,
                )? {
                    return Err(Error::PruneIntentConflict(
                        "Native AMX historical capacity lacks the later WSV authority".to_owned(),
                    ));
                }
                // This read-only planner cannot manufacture a durability attestation. The
                // later committed pointer is already authority for the prior retained prefix.
                return Ok(None);
            }
        }
        let height = descriptor.lane_block_height;
        let mut expected_manifest = manifest.clone();
        let mut expected_receipt = receipt.clone();
        // Pre-finality hashes have fixed widths. On exact retry retain the actual durable hash,
        // then require every other canonical artifact field to agree with the carrier.
        if let Some(finality_hash) = manifests
            .get(&height)
            .or_else(|| temporary_manifests.get(&height))
            .map(|retained| retained.finality_artifact_hash)
            .or_else(|| {
                receipts
                    .get(&height)
                    .or_else(|| temporary_receipts.get(&height))
                    .map(|retained| retained.finality_artifact_hash)
            })
        {
            expected_manifest.finality_artifact_hash = finality_hash;
            expected_receipt.finality_artifact_hash = finality_hash;
            expected_receipt.manifest_artifact_hash = HashOf::new(&expected_manifest);
            if manifests
                .get(&height)
                .is_some_and(|retained| *retained != expected_manifest)
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX reserved manifest conflicts with retained bytes".to_owned(),
                ));
            }
        }
        if receipts
            .get(&height)
            .is_some_and(|retained| *retained != expected_receipt)
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX reserved receipt conflicts with retained bytes".to_owned(),
            ));
        }
        if temporary_manifests
            .iter()
            .any(|(candidate_height, retained)| {
                *candidate_height != height || *retained != expected_manifest
            })
            || temporary_receipts
                .iter()
                .any(|(candidate_height, retained)| {
                    *candidate_height != height || *retained != expected_receipt
                })
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX publication temporary differs from its exact incoming carrier"
                    .to_owned(),
            ));
        }
        let expected_latest =
            NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&expected_receipt);
        if latest.is_some_and(|retained| {
            retained.lane_block_height == height && retained != expected_latest
        }) {
            return Err(Error::PruneIntentConflict(
                "Native AMX reserved latest pointer conflicts with retained bytes".to_owned(),
            ));
        }
        if let Some(temporary) = latest_temporary {
            if temporary != expected_latest || !manifests.contains_key(&height) || !receipts.contains_key(&height)
                || !Self::native_amx_participant_receipt_matches_manifest_leaf(&expected_receipt, &expected_manifest.leaf)
                || !self.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(&expected_manifest) {
                return Err(Error::PruneIntentConflict("Native AMX latest temporary lacks its exact authenticated stable pair".to_owned()));
            }
        }
        let mut outstanding_components = BTreeSet::new();
        if !manifests.contains_key(&height) {
            outstanding_components.insert(NativeAmxPublicationComponent::Manifest);
        }
        if !receipts.contains_key(&height) {
            outstanding_components.insert(NativeAmxPublicationComponent::Receipt);
        }
        if latest != Some(expected_latest) {
            outstanding_components.insert(NativeAmxPublicationComponent::Latest);
        }
        let component_bytes = BTreeMap::from([
            (
                NativeAmxPublicationComponent::Manifest,
                u64::try_from(expected_manifest.encode_framed()?.len())?,
            ),
            (
                NativeAmxPublicationComponent::Receipt,
                u64::try_from(expected_receipt.encode_framed()?.len())?,
            ),
            (
                NativeAmxPublicationComponent::Latest,
                u64::try_from(norito::encode_canonical(&expected_latest)?.len())?,
            ),
        ]);
        let mut component_allocation_bytes = component_bytes.clone();
        if temporary_manifests.contains_key(&height) {
            component_allocation_bytes.insert(NativeAmxPublicationComponent::Manifest, 0);
        }
        if temporary_receipts.contains_key(&height) {
            component_allocation_bytes.insert(NativeAmxPublicationComponent::Receipt, 0);
        }
        if latest_temporary.is_some() {
            component_allocation_bytes.insert(NativeAmxPublicationComponent::Latest, 0);
        }
        // Stable files and authenticated atomic temporaries are already in physical usage.
        for (kind, bytes) in &mut component_allocation_bytes {
            if !outstanding_components.contains(kind) {
                *bytes = 0;
            }
        }
        manifests.insert(height, expected_manifest.clone());
        receipts.insert(height, expected_receipt.clone());
        let journal = if let Some(intent) = &pending_prune {
            if intent.protected_latest.identity != expected_latest {
                return Err(Error::PruneIntentConflict(
                    "Native AMX publication cannot pass an unfinished prune frontier".to_owned(),
                ));
            }
            // This exact journal is already physical; completion only renames/unlinks
            // and synchronizes. Its bytes remain in measured usage, not a second reserve.
            None
        } else {
            Self::plan_native_amx_evidence_prune_intent_from_artifacts(
                self.native_amx_participant_evidence_retention(),
                self.native_amx_participant_evidence_file_bytes(),
                self.native_amx_evidence_prune_intent_max_bytes(),
                &manifests,
                &receipts,
            )?
        };
        let prune_journal_bytes = match journal {
            Some(intent) => u64::try_from(norito::encode_canonical(&intent)?.len())?,
            None => 0,
        };
        Ok(Some((
            route,
            NativeAmxRoutePublicationCapacity {
                participant_height: height,
                proposal_hash: receipt.participant_proposal.proposal_hash,
                settlement_hash: receipt.participant_settlement_hash,
                component_bytes,
                component_allocation_bytes,
                outstanding_components,
                prune_journal_bytes,
                physical_cleanup_pending: pending_prune.is_some() || latest_temporary.is_some(),
                cleanup_complete: false,
            },
        )))
    }
    fn native_amx_publication_wsv_join_is_complete_locked(
        &self,
        manifest_artifact: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Result<bool> {
        let height = receipt.application_block_height;
        let Some(manifest) = self.commit_manifest_under_sidecar_guard(height)? else {
            return Ok(false);
        };
        let Some(checkpoint) = self.wsv_checkpoint_under_sidecar_guard(height)? else {
            return Ok(false);
        };
        let Some((_, finality, _)) =
            self.v2_finality_artifact_with_archive_under_prune_and_canonical_guards(height)?
        else {
            return Ok(false);
        };
        let execution = &finality.commit_qc.execution_commitment;
        Ok(manifest.block_hash == receipt.application_block_hash
            && checkpoint.block_hash == receipt.application_block_hash
            && checkpoint.state_hash == manifest.wsv_checkpoint_hash
            && checkpoint.commit_manifest_hash == Some(manifest.encoded_hash())
            && manifest.parent_state_root == Some(execution.parent_state_root)
            && manifest.post_state_root == Some(execution.post_state_root)
            && manifest.commit_qc_hash == Some(Hash::new(finality.commit_qc.encode()))
            && manifest.commit_authority_hash == Some(v2_commit_authority_hash(&finality))
            && self.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(manifest_artifact))
    }
    fn native_amx_publication_plan_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<
        Option<(
            NativeAmxPublicationCarrier,
            NativeAmxPublicationCapacityReservation,
        )>,
    > {
        self.native_amx_publication_plan_for_storage_under_prune_and_canonical_guards(
            block,
            merge_entry,
            NativeAmxPublicationStorage::Active,
        )
    }
    fn native_amx_publication_plan_for_storage_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        storage: NativeAmxPublicationStorage,
    ) -> Result<
        Option<(
            NativeAmxPublicationCarrier,
            NativeAmxPublicationCapacityReservation,
        )>,
    > {
        if !block.has_results() {
            return Ok(None);
        }
        let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, merge_entry)
            .map_err(|error| Error::PruneIntentConflict(format!("cannot plan Native AMX publication capacity: {error}")))?;
        self.validate_native_amx_participant_application_evidence_byte_budget(&manifest, None)
            .map_err(|error| {
                Error::PruneIntentConflict(format!(
                    "Native AMX publication pair byte bound: {error}"
                ))
            })?;
        let artifacts = native_amx_participant_application_artifacts(
            &manifest,
            native_amx_participant_application_finality_placeholder_hash(),
        )
        .ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native AMX publication lacks exact manifest proofs".to_owned(),
            )
        })?;
        if artifacts.is_empty() {
            return Ok(None);
        }
        let carrier = NativeAmxPublicationCarrier {
            height: block.header().height().get(),
            block_hash: block.hash(),
            executed_wire_hash: manifest.executed_block_wire_hash(),
        };
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        let mut routes = BTreeMap::new();
        for (manifest, receipt) in &artifacts {
            if let Some((route, capacity)) = self
                .native_amx_route_publication_capacity_for_storage_locked(
                    manifest, receipt, storage,
                )?
            {
                if routes.insert(route, capacity).is_some() {
                    return Err(Error::PruneIntentConflict(
                        "Native AMX publication repeats a participant route".to_owned(),
                    ));
                }
            }
        }
        Ok(Some((
            carrier,
            NativeAmxPublicationCapacityReservation {
                index_record: None,
                index_additional_bytes: 0,
                routes,
            },
        )))
    }
    /// Index absence is never enough: a repeated completed publication must
    /// authenticate every exact stable route, WSV join and cleanup barrier.
    fn native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards(
        &self,
        carrier: NativeAmxPublicationCarrier,
        plan: &NativeAmxPublicationCapacityReservation,
    ) -> Result<bool> {
        if plan
            .routes
            .values()
            .any(|route| !route.outstanding_components.is_empty() || route.prune_journal_bytes != 0)
        {
            return Ok(false);
        }
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        if Self::read_native_amx_publication_index_for_store(&self.store_root)?
            .records
            .contains_key(&carrier)
        {
            return Ok(false);
        }
        for (route, capacity) in &plan.routes {
            let entry = self.lane_storage_entry(route.lane_id)?;
            if entry.dataspace_id != route.dataspace_id {
                return Err(Error::PruneIntentConflict(
                    "Native AMX completed retry changed its dataspace".to_owned(),
                ));
            }
            let namespace = self.native_amx_evidence_namespace_for_entry(&entry)?;
            let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, false)?;
            self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;
            self.require_native_amx_latest_index_temp_absent_locked(&namespace)?;
            let protected = self.derive_native_amx_evidence_prune_protected_latest_locked(
                &entry, &namespace, &inventory,
            )?;
            let latest = protected.identity;
            if latest.application_block_height != carrier.height
                || latest.application_block_hash != carrier.block_hash
                || latest.executed_block_wire_hash != carrier.executed_wire_hash
                || latest.lane_incarnation != route.incarnation
                || latest.lane_block_height != capacity.participant_height
                || latest.participant_proposal_hash != capacity.proposal_hash
                || latest.participant_settlement_hash != capacity.settlement_hash
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX completed retry differs from exact latest authority".to_owned(),
                ));
            }
            let manifest_file = inventory
                .manifests
                .get(&capacity.participant_height)
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native AMX completed retry lacks its manifest".to_owned(),
                    )
                })?;
            let receipt_file = inventory
                .receipts
                .get(&capacity.participant_height)
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native AMX completed retry lacks its receipt".to_owned(),
                    )
                })?;
            let manifest =
                self.decode_native_amx_manifest_file_locked(&entry, &namespace, manifest_file)?;
            let receipt =
                self.decode_native_amx_receipt_file_locked(&entry, &namespace, receipt_file)?;
            if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? {
                return Ok(false);
            }
            self.sync_native_amx_evidence_namespace(
                &namespace,
                "Native AMX completed publication retry",
            )?;
            self.inventory_native_amx_evidence_files_locked(&namespace, false)?;
        }
        // An earlier exact unlink may have succeeded before its directory sync
        // failed. Re-establish that barrier before releasing any surviving map pin.
        let record = self
            .native_amx_publication_capacity_reservations
            .lock()
            .get(&carrier)
            .and_then(|reservation| reservation.index_record.clone());
        if let Some(record) = record {
            self.remove_native_amx_publication_index_exact_locked(&record)?;
        }
        self.native_amx_publication_capacity_reservations
            .lock()
            .remove(&carrier);
        Ok(true)
    }
    fn begin_native_amx_store_capacity_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        replaced: Option<&SignedBlock>,
    ) -> Result<Option<NativeAmxStoreCapacityGuard<'_>>> {
        let (carrier, plan) = match self
            .native_amx_publication_plan_under_prune_and_canonical_guards(block, merge_entry)?
        {
            Some(plan) => plan,
            None => {
                let carrier = Self::native_amx_publication_carrier(block)?;
                let inventory =
                    Self::read_native_amx_publication_index_for_store(&self.store_root)?;
                let exact_retirement_retry = inventory
                    .records
                    .get(&carrier)
                    .is_some_and(|record| record.replaced.is_some());
                let retiring_old = replaced
                    .map(Self::native_amx_publication_carrier)
                    .transpose()?
                    .is_some_and(|old| inventory.records.contains_key(&old));
                if !exact_retirement_retry && !retiring_old {
                    return Ok(None);
                }
                (
                    carrier,
                    NativeAmxPublicationCapacityReservation {
                        index_record: None,
                        index_additional_bytes: 0,
                        routes: BTreeMap::new(),
                    },
                )
            }
        };
        if !plan.routes.is_empty()
            && self
                .native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards(
                    carrier, &plan,
                )?
        {
            return Ok(None);
        }
        let publication =
            self.prepare_native_amx_publication_index(block, merge_entry, replaced)?;
        let replaced = replaced
            .map(Self::native_amx_publication_carrier)
            .transpose()?;
        self.admit_native_amx_publication_capacity_plan(carrier, plan, replaced, publication)
    }
    fn admit_native_amx_publication_capacity_plan(
        &self,
        carrier: NativeAmxPublicationCarrier,
        mut plan: NativeAmxPublicationCapacityReservation,
        replaced: Option<NativeAmxPublicationCarrier>,
        publication: NativeAmxPublicationIndexPublication,
    ) -> Result<Option<NativeAmxStoreCapacityGuard<'_>>> {
        if publication.record.carrier != carrier {
            return Err(Error::PruneIntentConflict(
                "Native AMX index and capacity carriers differ".to_owned(),
            ));
        }
        plan.index_record = Some(publication.record.clone());
        plan.index_additional_bytes = publication.additional_bytes;
        let mut reservations = self.native_amx_publication_capacity_reservations.lock();
        let created = !reservations.contains_key(&carrier);
        if let Some(existing) = reservations.get(&carrier) {
            if existing
                .index_record
                .as_ref()
                .is_some_and(|record| record != &publication.record)
                || (existing.index_record.is_some()
                    && plan.index_additional_bytes > existing.index_additional_bytes)
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX exact retry changed its pending index allocation or identity"
                        .to_owned(),
                ));
            }
            for (route, old) in &existing.routes {
                if let Some(new) = plan.routes.get_mut(route) {
                    if old.participant_height != new.participant_height
                        || old.proposal_hash != new.proposal_hash
                        || old.settlement_hash != new.settlement_hash
                        || old.component_bytes != new.component_bytes
                        // Physical cleanup may appear as admitted reserved work is
                        // published, and disappear after authenticated recovery. It
                        // must never reopen a route whose cleanup already completed.
                        || (old.cleanup_complete && new.physical_cleanup_pending)
                        || !new
                            .outstanding_components
                            .is_subset(&old.outstanding_components)
                        || new.prune_journal_bytes > old.prune_journal_bytes
                        || new.component_allocation_bytes.iter().any(|(kind, bytes)| {
                            old.component_allocation_bytes
                                .get(kind)
                                .is_none_or(|old_bytes| bytes > old_bytes)
                        })
                    {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX exact retry changed an owned publication component"
                                .to_owned(),
                        ));
                    }
                    new.cleanup_complete = old.cleanup_complete;
                } else {
                    return Err(Error::PruneIntentConflict(
                        "Native AMX owned route became historical before exact completion"
                            .to_owned(),
                    ));
                }
            }
            if plan
                .routes
                .keys()
                .any(|route| !existing.routes.contains_key(route))
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX exact retry gained another participant route".to_owned(),
                ));
            }
        }
        for (other_carrier, other) in reservations.iter() {
            if *other_carrier != carrier
                && Some(*other_carrier) != replaced
                && plan
                    .routes
                    .keys()
                    .any(|route| other.routes.contains_key(route))
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX route still owns an incomplete earlier publication".to_owned(),
                ));
            }
        }
        if reservations.len() >= 2 * iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES && created
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX publication carrier association bound exceeded".to_owned(),
            ));
        }
        reservations.insert(carrier, plan);
        Ok(Some(NativeAmxStoreCapacityGuard {
            kura: self,
            carrier,
            rollback_new_reservation: created,
            index_write_started: false,
            canonical_write_started: false,
            rollback_proven: false,
            index_publication: publication,
        }))
    }
    /// Existing canonical carriers already contribute their block bytes to physical usage.
    /// Snapshot other families before opening the Native map; no map lock is nested.
    fn check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards(
        &self,
    ) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let used = self.kura_disk_usage_bytes()?;
        let pending = self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let lane = self.lane_publication_budget_reserved_bytes()?;
        let certified = self.certified_bundle_capacity_reserved_bytes()?;
        let terminal = self.autonomous_global_terminal_outcome_reserved_bytes()?;
        let required = [
            pending,
            lane,
            certified,
            terminal,
            Self::canonical_prune_intent_maintenance_headroom_bytes(),
        ]
        .into_iter()
        .try_fold(used, |total, bytes| total.checked_add(bytes))
        .ok_or_else(|| {
            Error::PruneIntentConflict("Native AMX existing carrier capacity overflowed".to_owned())
        })?;
        if required > self.max_disk_usage_bytes {
            return Err(Error::StorageBudgetExceeded {
                limit: self.max_disk_usage_bytes,
                used,
                required,
            });
        }
        Ok(())
    }
    fn ensure_native_amx_publication_capacity_under_publication_guard(
        &self,
        block: &SignedBlock,
        evidence: &NativeAmxParticipantApplicationEvidencePlan,
        target_indices: &[usize],
    ) -> Result<bool> {
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_durable_block_at_height(block.header().height().get(), block.hash())?;
        let mut guard = match self
            .native_amx_capacity_plan_from_evidence_under_prune_and_canonical_guards(
                block,
                evidence,
                target_indices,
            )? {
            Some((carrier, mut plan)) => {
                // Target selection authorizes current namespace access, not retirement of
                // another route's outstanding operation. Preserve every untargeted owner
                // without reading its potentially retired or reincarnated lane directory.
                let existing = self
                    .native_amx_publication_capacity_reservations
                    .lock()
                    .get(&carrier)
                    .cloned();
                if let Some(existing) = existing {
                    for (route, capacity) in existing.routes {
                        plan.routes.entry(route).or_insert(capacity);
                    }
                } else if !plan.routes.is_empty()
                    && self.native_amx_publication_plan_is_durably_complete_under_prune_and_canonical_guards(carrier, &plan)? {
                    return Ok(false);
                } else if target_indices.len() != evidence.artifacts.len()
                    && Self::read_native_amx_publication_index_for_store(&self.store_root)?
                        .records.get(&carrier).is_some_and(|record|
                            record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite)
                {
                    // An unfinished original write still needs its complete
                    // carrier owner. Only separately authenticated completed
                    // repair may prove every non-target already terminal.
                    return Err(Error::PruneIntentConflict(
                        "Native AMX targeted repair lacks the complete carrier reservation".to_owned(),
                    ));
                }
                let merge =
                    self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(block)?;
                let publication = self.prepare_native_amx_repair_publication_index(
                    block,
                    merge.as_ref(),
                    evidence,
                    target_indices,
                )?;
                self.admit_native_amx_publication_capacity_plan(carrier, plan, None, publication)?
            }
            None => None,
        };
        // This canonical recovery source predates this call, so failure retains its reservation.
        if let Some(guard) = &mut guard {
            guard.durable_write_started();
        }
        self.check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards()?;
        if let Some(guard) = &mut guard {
            guard.publish_pending_index()?;
        }
        Ok(guard.is_some())
    }
    /// Called only after the exact stable file's publication/readback and usage delta.
    fn consume_native_amx_publication_component_after_durable_publication(
        &self,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
        component: NativeAmxPublicationComponent,
        encoded_len: usize,
    ) -> Result<()> {
        let descriptor = &receipt.participant_proposal.descriptor;
        let carrier = NativeAmxPublicationCarrier {
            height: receipt.application_block_height,
            block_hash: receipt.application_block_hash,
            executed_wire_hash: receipt.executed_block_wire_hash,
        };
        let route = NativeAmxPublicationRoute {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            incarnation: descriptor.lane_incarnation,
        };
        let mut reservations = self.native_amx_publication_capacity_reservations.lock();
        let Some(mut reservation) = reservations.get_mut(&carrier) else {
            return Err(Error::PruneIntentConflict(
                "Native AMX durable publication lacks its admitted carrier".to_owned(),
            ));
        };
        let capacity = reservation.routes.get_mut(&route).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native AMX durable component lacks its reserved route".to_owned(),
            )
        })?;
        if capacity.participant_height != descriptor.lane_block_height
            || capacity.proposal_hash != receipt.participant_proposal.proposal_hash
            || capacity.settlement_hash != receipt.participant_settlement_hash
            || capacity.component_bytes.get(&component).copied()
                != Some(u64::try_from(encoded_len)?)
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX durable component differs from its exact carrier reservation"
                    .to_owned(),
            ));
        }
        let allocation = capacity
            .component_allocation_bytes
            .get_mut(&component)
            .ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native AMX durable component lacks its exact allocation entry".to_owned(),
                )
            })?;
        // This allocation is now physical. Keep the operation's in-memory shape
        // identical to a fresh exact plan, without reviving credit on retry.
        *allocation = 0;
        capacity.outstanding_components.remove(&component);
        Ok(())
    }
    fn complete_native_amx_publication_route_capacity_locked(
        &self,
        entry: &LaneConfigEntry,
        namespace: &BoundProgressNamespace,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Result<()> {
        let carrier = NativeAmxPublicationCarrier {
            height: receipt.application_block_height,
            block_hash: receipt.application_block_hash,
            executed_wire_hash: receipt.executed_block_wire_hash,
        };
        let inventory = self.inventory_native_amx_evidence_files_locked(namespace, false)?;
        self.require_native_amx_evidence_prune_intent_absent_locked(namespace)?;
        self.require_native_amx_latest_index_temp_absent_locked(namespace)?;
        let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(
            entry,
            &self.store_root,
        );
        let latest = self.decode_bound_native_amx_participant_receipt_latest_index_locked(
            entry,
            &latest_path,
            namespace,
        )?;
        if latest
            != Some(NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                receipt,
            ))
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX capacity release lacks its exact latest frontier".to_owned(),
            ));
        }
        let protected = self.derive_native_amx_evidence_prune_protected_latest_locked(
            entry, namespace, &inventory,
        )?;
        if Some(protected.identity) != latest
            || protected.receipt_artifact_hash != HashOf::new(receipt)
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX capacity release protects another frontier".to_owned(),
            ));
        }
        let manifest_file = inventory
            .manifests
            .get(&protected.identity.lane_block_height)
            .ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native AMX capacity release lost its protected manifest".to_owned(),
                )
            })?;
        let manifest =
            self.decode_native_amx_manifest_file_locked(entry, namespace, manifest_file)?;
        if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, receipt)? {
            // Pre-WSV startup shape retains ownership until State recovery establishes this join.
            return Ok(());
        }
        self.sync_native_amx_evidence_namespace(namespace, "Native AMX capacity completion")?;
        self.inventory_native_amx_evidence_files_locked(namespace, false)?;
        let descriptor = &receipt.participant_proposal.descriptor;
        let route = NativeAmxPublicationRoute {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            incarnation: descriptor.lane_incarnation,
        };
        let mut reservations = self.native_amx_publication_capacity_reservations.lock();
        let all_complete = {
            let Some(mut reservation) = reservations.get_mut(&carrier) else {
                // The full exact receipt/latest/WSV/cleanup/durability join above is a
                // durable completion tombstone; absence alone never authorizes release.
                return Ok(());
            };
            let maintenance_only = reservation.index_record.is_none();
            let Some(capacity) = reservation.routes.get_mut(&route) else {
                if maintenance_only {
                    // A retained maintenance owner may cover only another route of
                    // this already completed carrier. The full exact completion join
                    // above authenticates this route without granting publication work.
                    return Ok(());
                }
                return Err(Error::PruneIntentConflict(
                    "Native AMX capacity completion lost its owned route".to_owned(),
                ));
            };
            if !capacity.outstanding_components.is_empty()
                || capacity.participant_height != descriptor.lane_block_height
                || capacity.proposal_hash != receipt.participant_proposal.proposal_hash
                || capacity.settlement_hash != receipt.participant_settlement_hash
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX capacity completion still owns an unfinished component".to_owned(),
                ));
            }
            // The journal/temp absence, WSV, directory sync and inventory checks
            // above close physical cleanup without granting any new byte credit.
            capacity.physical_cleanup_pending = false;
            capacity.cleanup_complete = true;
            reservation
                .routes
                .values()
                .all(|route| route.cleanup_complete)
        };
        let completed_record = all_complete
            .then(|| {
                reservations
                    .get(&carrier)
                    .and_then(|reservation| reservation.index_record.clone())
            })
            .flatten();
        drop(reservations);
        if all_complete {
            if let Some(record) = completed_record {
                self.remove_native_amx_publication_index_exact_locked(&record)?;
            }
            self.native_amx_publication_capacity_reservations
                .lock()
                .remove(&carrier);
        }
        Ok(())
    }
    /// Resolve the exact compact input without publishing associations during reservation recovery.
    fn native_amx_capacity_merge_entry_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
    ) -> Result<Option<MergeLedgerEntry>> {
        let Some(reference) = Self::block_merge_reference(block) else {
            return Ok(None);
        };
        let pending = {
            let _sidecar = self.sidecar_lock.lock();
            self.read_pending_merge_entry_path(
                &self.pending_merge_entry_path(reference.entry_hash),
                Some(reference.entry_hash),
            )?
        };
        let entry = match pending {
            Some(entry) => Some(entry),
            None => self
                .merge_log
                .lock()
                .entry_by_hash_without_append_repair(reference.entry_hash)?,
        };
        let entry = match entry {
            Some(entry) => entry,
            None => {
                let stage = self.read_canonical_association_stage()?.ok_or(
                    Error::MissingCertifiedMergeSidecar {
                        entry_hash: reference.entry_hash,
                    },
                )?;
                let staged = self.validate_canonical_association_stage(&stage)?;
                if staged.encode_wire()? != block.encode_wire()? {
                    return Err(Error::PruneIntentConflict(
                        "Native AMX recovery stage differs from canonical executed carrier"
                            .to_owned(),
                    ));
                }
                stage
                    .merge_entry
                    .ok_or(Error::MissingCertifiedMergeSidecar {
                        entry_hash: reference.entry_hash,
                    })?
            }
        };
        if !reference.matches_entry(&entry) {
            return Err(Error::MergeReferenceMismatch(
                "Native AMX recovery compact input differs from canonical reference".to_owned(),
            ));
        }
        Self::validate_merge_transaction_uniqueness(block, &entry)?;
        Ok(Some(entry))
    }
    /// Refuse a canonical Native tip whose pending locator disappeared before WSV
    /// completion, even when no participant evidence file was published yet. This is
    /// consistency validation only: it never derives a reservation or resolves geometry.
    fn validate_native_amx_unindexed_tip_completion_under_prune_and_canonical_guards(
        &self,
        index: &NativeAmxPublicationIndexInventory,
    ) -> Result<()> {
        let Some(height) = NonZeroUsize::new(self.exact_durable_blocks_count()?) else {
            return Ok(());
        };
        let block = self
            .get_block_without_merge_sidecar(height)
            .ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native startup cannot authenticate its canonical tip".to_owned(),
                )
            })?;
        let carrier = Self::native_amx_publication_carrier(&block)?;
        if !block.has_results() || index.records.contains_key(&carrier) {
            return Ok(());
        }
        let merge =
            self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;
        let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            &block, merge.as_ref(),
        ).map_err(|error| Error::PruneIntentConflict(format!("Native tip commitment is invalid: {error}")))?;
        if manifest.count() == 0 {
            return Ok(());
        }
        let missing_index = || {
            Error::PruneIntentConflict(
                "Native AMX incomplete canonical tip lacks its exact pending index".to_owned(),
            )
        };
        let _sidecar = self.sidecar_lock.lock();
        let Some((_, finality, _)) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier.height)?
        else {
            return Err(missing_index());
        };
        let artifacts =
            native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))
                .ok_or_else(missing_index)?;
        let (manifest_artifact, receipt) = artifacts.first().ok_or_else(missing_index)?;
        // The shared carrier join binds canonical block, exact executed Native root/count,
        // finality, both state roots, QC authority and checkpoint-bound commit manifest.
        // Complete tips need no lane lookup merely because their index was retired.
        if !self.native_amx_publication_wsv_join_is_complete_locked(manifest_artifact, receipt)? {
            return Err(missing_index());
        }
        Ok(())
    }
    /// Reconstruct bounded operation ownership read-only, before any capacity-growing recovery.
    /// The bounded durable pending index closes every store-before-publication crash window,
    /// including carriers below an ordinary tip. Retained route evidence supplies exact cleanup
    /// state; no unbounded canonical history scan or tip-only discovery is permitted.
    fn rebuild_native_amx_publication_capacity_on_startup(&self) -> Result<()> {
        let resource_fence = self
            .resource_inventory
            .begin(resource_inventory::Family::ResidentFrontier.mask())
            .ok();
        self.native_amx_resident_recovery_complete
            .store(false, Ordering::Release);
        let result = (|| {
            let _prune = self.prune_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            let _canonical = self.canonical_chain_lock.lock();
            let mut plans = BTreeMap::<
                NativeAmxPublicationCarrier,
                NativeAmxPublicationCapacityReservation,
            >::new();
            // Initial publication persists its index before the canonical carrier;
            // authenticated later repairs persist a new index before any repair write.
            // A completed tip has no remaining publication obligation; deriving one
            // from its body would resurrect retired routes during geometry replay.
            // Durable locators include unfinished carriers below an ordinary tip.
            // Classify every record against the independently resolved canonical marker;
            // an omitted body pin or a different hash never authorizes retirement.
            let pending_index =
                Self::read_native_amx_publication_index_for_store(&self.store_root)?;
            self.validate_native_amx_unindexed_tip_completion_under_prune_and_canonical_guards(
                &pending_index,
            )?;
            let selected_marker = {
                let mut store = self.block_store.lock();
                let count = store.read_exact_durable_index_count()?;
                store.commit_marker_for_count(count)?
            };
            let prune_inventory =
                Self::canonical_prune_intent_artifact_inventory(&self.store_root)?;
            if prune_inventory.temporary.is_some() {
                return Err(Error::PruneIntentConflict(
                    "Native capacity reconstruction requires resolved canonical prune publication"
                        .to_owned(),
                ));
            }
            let prune_intent = prune_inventory
                .stable
                .as_ref()
                .map(|artifact| &artifact.intent);
            let mut resolutions = BTreeMap::new();
            for (carrier, record) in &pending_index.records {
                let selected = if carrier.height <= selected_marker.count {
                    let height =
                        NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {
                            Error::PruneIntentConflict(
                                "Native pending carrier has zero height".to_owned(),
                            )
                        })?;
                    let block = self
                        .get_block_without_merge_sidecar(height)
                        .ok_or_else(|| {
                            Error::PruneIntentConflict(
                                "Native pending carrier lost its selected local body".to_owned(),
                            )
                        })?;
                    Some(Self::native_amx_publication_carrier(&block)?)
                } else {
                    None
                };
                resolutions.insert(
                    *carrier,
                    record.classify_resolved_carrier(&selected_marker, selected)?,
                );
            }
            let committed_replacements = pending_index
                .records
                .iter()
                .filter(|(carrier, _)| {
                    resolutions.get(carrier)
                        == Some(&NativeAmxPublicationIndexResolution::Committed)
                })
                .filter_map(|(_, record)| record.replaced)
                .collect::<BTreeSet<_>>();
            let mut index_records_to_retire = Vec::new();
            let mut committed_index_carriers = BTreeSet::new();
            for (carrier, record) in &pending_index.records {
                match resolutions
                    .get(carrier)
                    .expect("every bounded index record was classified")
                {
                    NativeAmxPublicationIndexResolution::Committed => {
                        committed_index_carriers.insert(*carrier);
                    }
                    NativeAmxPublicationIndexResolution::ProvenUncommitted => {
                        index_records_to_retire.push(record.clone())
                    }
                    NativeAmxPublicationIndexResolution::RequiresRetirementProof => {
                        if committed_replacements.contains(carrier) {
                            index_records_to_retire.push(record.clone());
                        } else if let Some(intent) = prune_intent {
                            let digest = Hash::new(record.encoded()?);
                            if selected_marker.count != intent.target_height
                                || selected_marker.tip_hash != intent.target_tip_hash
                                || carrier.height <= intent.target_height
                                || intent
                                    .native_amx_retirement_record_hashes
                                    .binary_search(&digest)
                                    .is_err()
                            {
                                return Err(Error::PruneIntentConflict("Native index mismatch lacks exact canonical prune authorization".to_owned()));
                            }
                            // The existing prune transaction removes only its listed residue
                            // after full completed-prune validation, before clearing its intent.
                        } else {
                            return Err(Error::PruneIntentConflict(
                                "Native pending carrier requires an authenticated retirement proof"
                                    .to_owned(),
                            ));
                        }
                    }
                }
            }
            let incomplete_carriers = committed_index_carriers;
            {
                let _geometry = self.lane_geometry_lock.lock();
                // State has not published its secondary live map yet. Inspect only
                // bounded locations authenticated by the admitted physical journal,
                // including exact retained pairs during no-snapshot rollback.
                let locations = self.native_amx_evidence_physical_locations_from_journal()?;
                let _sidecar = self.sidecar_lock.lock();
                for location in locations {
                    let directory = Self::lane_artifact_dir(location.blocks_path());
                    let manifest_path = directory.join(Self::native_amx_evidence_file_name(
                        NativeAmxEvidenceKind::Manifest,
                        1,
                    ));
                    let receipt_path = directory.join(Self::native_amx_evidence_file_name(
                        NativeAmxEvidenceKind::Receipt,
                        1,
                    ));
                    if self
                        .bound_progress_sidecar_directory_is_absent(&manifest_path, &receipt_path)?
                    {
                        continue;
                    }
                    self.require_native_amx_evidence_physical_location(&location)?;
                    let namespace =
                        self.open_bound_progress_namespace(&manifest_path, &receipt_path)?;
                    let inventory =
                        self.inventory_native_amx_evidence_files_locked(&namespace, true)?;
                    let highest = inventory
                        .manifests
                        .keys()
                        .chain(inventory.receipts.keys())
                        .copied()
                        .chain(
                            inventory
                                .temporaries
                                .values()
                                .map(|file| file.participant_height),
                        )
                        .max();
                    let Some(height) = highest else {
                        self.require_native_amx_evidence_physical_location(&location)?;
                        continue;
                    };
                    // The journal owns paths/incarnations, not dataspace IDs. Read one
                    // bounded canonical frame to discover that value, then authenticate
                    // both artifacts using the exact physical target and finality below.
                    let observed_dataspace = if let Some(file) =
                        inventory.receipts.get(&height).or_else(|| {
                            inventory
                                .temporary(NativeAmxEvidenceKind::Receipt)
                                .filter(|file| file.participant_height == height)
                        }) {
                        let bytes =
                            self.read_native_amx_evidence_file_bytes_locked(&namespace, file)?;
                        norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(
                            &bytes,
                        )
                        .map_err(|error| {
                            Self::invalid_lane_artifact_error(
                                file.path.clone(),
                                format!(
                                    "Native receipt discovery failed exact Norito decode: {error}"
                                ),
                            )
                        })?
                        .participant_proposal
                        .descriptor
                        .dataspace_id
                    } else {
                        let file = inventory
                            .manifests
                            .get(&height)
                            .or_else(|| {
                                inventory
                                    .temporary(NativeAmxEvidenceKind::Manifest)
                                    .filter(|file| file.participant_height == height)
                            })
                            .ok_or_else(|| {
                                Error::PruneIntentConflict(
                                    "Native highest discovery evidence disappeared".to_owned(),
                                )
                            })?;
                        let bytes =
                            self.read_native_amx_evidence_file_bytes_locked(&namespace, file)?;
                        norito::decode_canonical::<NativeAmxParticipantApplicationManifestArtifactV1>(&bytes)
                            .map_err(|error| Self::invalid_lane_artifact_error(file.path.clone(),
                                format!("Native manifest discovery failed exact Norito decode: {error}")))?
                            .leaf.dataspace_id
                    };
                    let entry = self.native_amx_reservation_physical_target_from_location(
                        &location,
                        observed_dataspace,
                    )?;
                    let manifest = inventory
                        .manifests
                        .get(&height)
                        .or_else(|| {
                            inventory
                                .temporary(NativeAmxEvidenceKind::Manifest)
                                .filter(|file| file.participant_height == height)
                        })
                        .map(|file| {
                            self.decode_native_amx_manifest_file_locked(&entry, &namespace, file)
                        })
                        .transpose()?;
                    let receipt = inventory
                        .receipts
                        .get(&height)
                        .or_else(|| {
                            inventory
                                .temporary(NativeAmxEvidenceKind::Receipt)
                                .filter(|file| file.participant_height == height)
                        })
                        .map(|file| {
                            self.decode_native_amx_receipt_file_locked(&entry, &namespace, file)
                        })
                        .transpose()?;
                    let (carrier, route) = if let Some(receipt) = &receipt {
                        let descriptor = &receipt.participant_proposal.descriptor;
                        (
                            NativeAmxPublicationCarrier {
                                height: receipt.application_block_height,
                                block_hash: receipt.application_block_hash,
                                executed_wire_hash: receipt.executed_block_wire_hash,
                            },
                            NativeAmxPublicationRoute {
                                lane_id: descriptor.lane_id,
                                dataspace_id: descriptor.dataspace_id,
                                incarnation: descriptor.lane_incarnation,
                            },
                        )
                    } else {
                        let manifest = manifest.as_ref().ok_or_else(|| {
                            Error::PruneIntentConflict(
                                "Native AMX highest recovery evidence disappeared".to_owned(),
                            )
                        })?;
                        (
                            NativeAmxPublicationCarrier {
                                height: manifest.leaf.application_block_height,
                                block_hash: manifest.leaf.application_block_hash,
                                executed_wire_hash: manifest.leaf.executed_block_wire_hash,
                            },
                            NativeAmxPublicationRoute {
                                lane_id: manifest.leaf.lane_id,
                                dataspace_id: manifest.leaf.dataspace_id,
                                incarnation: manifest.leaf.lane_incarnation,
                            },
                        )
                    };
                    self.ensure_durable_block_at_height(carrier.height, carrier.block_hash)?;
                    if incomplete_carriers.contains(&carrier) {
                        // The exact indexed carrier is reconstructed across all its routes
                        // below, including journal-retained paths before State replay.
                        self.require_native_amx_reservation_physical_target(&entry)?;
                        continue;
                    }
                    if let (Some(manifest), Some(receipt)) = (manifest, receipt)
                        && inventory.manifests.contains_key(&height)
                        && inventory.receipts.contains_key(&height)
                    {
                        if !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(&receipt, &manifest) {
                            return Err(Error::PruneIntentConflict("Native AMX recovery pair lacks exact finality authority".to_owned()));
                        }
                        let Some((planned_route, capacity)) = self
                            .native_amx_route_publication_capacity_at_target_locked(
                                &entry, &manifest, &receipt,
                            )?
                        else {
                            return Err(Error::PruneIntentConflict(
                                "Native AMX highest recovery pair became historical".to_owned(),
                            ));
                        };
                        if planned_route != route {
                            return Err(Error::PruneIntentConflict(
                                "Native AMX recovery route changed".to_owned(),
                            ));
                        }
                        if capacity
                            .outstanding_components
                            .iter()
                            .any(|component| *component != NativeAmxPublicationComponent::Latest)
                            || !self.native_amx_publication_wsv_join_is_complete_locked(
                                &manifest, &receipt,
                            )?
                        {
                            return Err(Error::PruneIntentConflict(
                                "Native AMX unfinished evidence lacks its exact pending index"
                                    .to_owned(),
                            ));
                        }
                        // A complete authoritative pair may reconstruct its derived latest
                        // pointer or compact tightened retention. This bounded maintenance
                        // never authorizes missing manifest/receipt publication. A clean
                        // completed pair needs no synthetic resident owner.
                        if capacity.prune_journal_bytes != 0
                            || !capacity.outstanding_components.is_empty()
                            || capacity.physical_cleanup_pending
                        {
                            plans
                                .entry(carrier)
                                .or_insert_with(|| NativeAmxPublicationCapacityReservation {
                                    index_record: None,
                                    index_additional_bytes: 0,
                                    routes: BTreeMap::new(),
                                })
                                .routes
                                .insert(route, capacity);
                        }
                    } else {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX incomplete evidence lacks its exact pending index"
                                .to_owned(),
                        ));
                    }
                    self.require_native_amx_reservation_physical_target(&entry)?;
                }
            }
            for expected in incomplete_carriers {
                let height =
                    NonZeroUsize::new(usize::try_from(expected.height)?).ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native AMX recovery carrier has zero height".to_owned(),
                        )
                    })?;
                let block = self
                    .get_block_without_merge_sidecar(height)
                    .ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native AMX incomplete recovery lacks its canonical body".to_owned(),
                        )
                    })?;
                if Self::native_amx_publication_carrier(&block)? != expected {
                    return Err(Error::PruneIntentConflict(
                        "Native AMX incomplete recovery executed wire changed".to_owned(),
                    ));
                }
                let merge =
                    self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;
                if let Some(record) = pending_index.records.get(&expected)
                    && record.merge_entry_hash
                        != merge.as_ref().map(MergeLedgerEntry::canonical_hash)
                {
                    return Err(Error::PruneIntentConflict(
                        "Native recovery compact association changed its recorded hash".to_owned(),
                    ));
                }
                if let Some(record) = pending_index.records.get(&expected)
                    && record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair
                {
                    self.authenticate_native_amx_completed_repair_on_startup(
                        &block,
                        merge.as_ref(),
                        record,
                    )?;
                }
                let (carrier, plan) = match self
                    .native_amx_publication_plan_for_storage_under_prune_and_canonical_guards(
                        &block,
                        merge.as_ref(),
                        NativeAmxPublicationStorage::JournalPhysical,
                    )? {
                    Some(plan) => plan,
                    None if pending_index
                        .records
                        .get(&expected)
                        .is_some_and(|record| record.replaced.is_some()) =>
                    {
                        (
                            expected,
                            NativeAmxPublicationCapacityReservation {
                                index_record: None,
                                index_additional_bytes: 0,
                                routes: BTreeMap::new(),
                            },
                        )
                    }
                    None => {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX incomplete recovery carrier has no Native outputs"
                                .to_owned(),
                        ));
                    }
                };
                let existing = plans.entry(carrier).or_insert_with(|| {
                    NativeAmxPublicationCapacityReservation {
                        index_record: None,
                        index_additional_bytes: 0,
                        routes: BTreeMap::new(),
                    }
                });
                for (route, capacity) in plan.routes {
                    if let Some(old) = existing.routes.insert(route, capacity.clone())
                        && old != capacity
                    {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX recovery sources disagree about exact route capacity"
                                .to_owned(),
                        ));
                    }
                }
            }
            for (carrier, plan) in &mut plans {
                plan.index_record = pending_index.records.get(carrier).cloned();
                plan.index_additional_bytes = 0; // every retained index file is already physical
            }
            if plans.len() > 2 * iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES {
                return Err(Error::PruneIntentConflict(
                    "Native AMX recovered carrier association bound exceeded".to_owned(),
                ));
            }
            let mut routes = BTreeSet::new();
            for plan in plans.values() {
                for route in plan.routes.keys() {
                    if !routes.insert(*route) {
                        return Err(Error::PruneIntentConflict(
                            "Native AMX recovery found competing incomplete route owners"
                                .to_owned(),
                        ));
                    }
                }
            }
            {
                let mut reservations = self.native_amx_publication_capacity_reservations.lock();
                reservations.retain(|_, _| false);
                for (carrier, plan) in plans {
                    reservations.insert(carrier, plan);
                }
            }
            // All reconstructed owners are installed before any other family can admit growth.
            self.cleanup_native_amx_publication_index_orphan_temporaries(&pending_index)?;
            self.check_native_amx_existing_carrier_capacity_under_prune_and_canonical_guards()?;
            // Only now may proven rollback/replacement residues be removed. Every
            // remaining capacity-growing recovery already has its typed reservation.
            for record in index_records_to_retire {
                self.remove_native_amx_publication_index_exact(&record)?;
            }
            let empty_completed = self
                .native_amx_publication_capacity_reservations
                .lock()
                .iter()
                .filter(|(_, plan)| plan.routes.is_empty())
                .filter_map(|(carrier, plan)| {
                    plan.index_record.clone().map(|record| (*carrier, record))
                })
                .collect::<Vec<_>>();
            for (carrier, record) in empty_completed {
                self.remove_native_amx_publication_index_exact(&record)?;
                self.native_amx_publication_capacity_reservations
                    .lock()
                    .remove(&carrier);
            }
            Ok(())
        })();
        if result.is_ok() {
            self.native_amx_resident_recovery_complete
                .store(true, Ordering::Release);
            if let Some(resource_fence) = resource_fence {
                let _ = resource_fence.publish(&[(
                    resource_inventory::Family::ResidentFrontier,
                    resource_inventory::Usage::default(),
                    resource_inventory::Usage::default(),
                )]);
            }
        }
        result
    }
    /// Finality-backed publication already owns an authenticated artifact plan. Revalidate its
    /// exact canonical carrier, proof population and bindings instead of inventing another plan.
    fn native_amx_capacity_plan_from_evidence_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        plan: &NativeAmxParticipantApplicationEvidencePlan,
        target_indices: &[usize],
    ) -> Result<
        Option<(
            NativeAmxPublicationCarrier,
            NativeAmxPublicationCapacityReservation,
        )>,
    > {
        let targets = target_indices.iter().copied().collect::<BTreeSet<_>>();
        if (targets.is_empty() && !plan.artifacts.is_empty())
            || targets.len() != target_indices.len()
            || targets.iter().any(|index| *index >= plan.artifacts.len())
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX capacity targets must be unique members of the authenticated plan"
                    .to_owned(),
            ));
        }
        let carrier = Self::native_amx_publication_carrier(block)?;
        if plan.application_block_height != carrier.height
            || plan.application_block_hash != carrier.block_hash
            || usize::try_from(plan.manifest_leaf_count).ok() != Some(plan.artifacts.len())
        {
            return Err(Error::PruneIntentConflict(
                "Native AMX publication capacity plan differs from its canonical carrier"
                    .to_owned(),
            ));
        }
        self.ensure_existing_block_wire_matches(block, carrier.height, carrier.block_hash)?;
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        let mut routes = BTreeMap::new();
        for (index, (manifest, receipt)) in plan.artifacts.iter().enumerate() {
            if usize::try_from(manifest.leaf_index).ok() != Some(index)
                || manifest.manifest_root != plan.manifest_root
                || manifest.manifest_leaf_count != plan.manifest_leaf_count
                || manifest.leaf.application_block_height != carrier.height
                || manifest.leaf.application_block_hash != carrier.block_hash
                || manifest.leaf.executed_block_wire_hash != carrier.executed_wire_hash
                || HashOf::new(manifest) != receipt.manifest_artifact_hash
                || manifest.finality_artifact_hash != receipt.finality_artifact_hash
                || Self::validate_native_amx_participant_application_receipt_artifact(receipt).is_err()
                || !Self::native_amx_participant_receipt_matches_manifest_leaf(receipt, &manifest.leaf)
                || !self.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(manifest) {
                return Err(Error::PruneIntentConflict("Native AMX publication capacity lacks its exact finality-backed artifact plan".to_owned()));
            }
            let (manifest_bytes, receipt_bytes) =
                native_amx_participant_application_pair_framed_bytes(manifest, receipt)?;
            self.validate_native_amx_participant_application_pair_byte_lengths(
                manifest_bytes.len(),
                receipt_bytes.len(),
                STRICT_INIT_MAX_BLOCK_BYTES,
            )
            .map_err(|error| {
                Error::PruneIntentConflict(format!(
                    "Native AMX publication pair byte bound: {error}"
                ))
            })?;
            // Authenticate all leaf/proof/finality bindings above. Only selected
            // State-owned targets may consult current lane storage below.
            if targets.contains(&index)
                && let Some((route, capacity)) =
                    self.native_amx_route_publication_capacity_locked(manifest, receipt)?
                && routes.insert(route, capacity).is_some()
            {
                return Err(Error::PruneIntentConflict(
                    "Native AMX finality plan repeats a publication route".to_owned(),
                ));
            }
        }
        Ok((!routes.is_empty()).then_some((
            carrier,
            NativeAmxPublicationCapacityReservation {
                index_record: None,
                index_additional_bytes: 0,
                routes,
            },
        )))
    }
    /// Capture exact retirement authority before canonical suffix bytes disappear.
    fn native_amx_publication_prune_record_hashes(&self, target: u64) -> Result<Vec<Hash>> {
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        let marker = {
            let mut store = self.block_store.lock();
            let count = store.read_exact_durable_index_count()?;
            store.commit_marker_for_count(count)?
        };
        let mut digests = Vec::new();
        for record in inventory
            .records
            .values()
            .filter(|record| record.carrier.height > target)
        {
            let selected = if record.carrier.height <= marker.count {
                let height = NonZeroUsize::new(usize::try_from(record.carrier.height)?)
                    .ok_or_else(|| {
                        Error::PruneIntentConflict("Native prune record has zero height".to_owned())
                    })?;
                let block = self
                    .get_block_without_merge_sidecar(height)
                    .ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native prune retirement lost its exact canonical body".to_owned(),
                        )
                    })?;
                let selected = Self::native_amx_publication_carrier(&block)?;
                if selected == record.carrier {
                    let merge = self
                        .native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;
                    if record.merge_entry_hash
                        != merge.as_ref().map(MergeLedgerEntry::canonical_hash)
                    {
                        return Err(Error::PruneIntentConflict(
                            "Native prune retirement changed its compact association".to_owned(),
                        ));
                    }
                }
                Some(selected)
            } else {
                None
            };
            if record.classify_resolved_carrier(&marker, selected)?
                == NativeAmxPublicationIndexResolution::RequiresRetirementProof
            {
                return Err(Error::PruneIntentConflict(
                    "Native canonical prune encountered an unclassified pending carrier".to_owned(),
                ));
            }
            digests.push(Hash::new(record.encoded()?));
        }
        digests.sort_unstable();
        if digests.len() > MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS
            || digests.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::PruneIntentConflict(
                "Native canonical prune record population is not bounded and unique".to_owned(),
            ));
        }
        Ok(digests)
    }
    /// The existing canonical prune journal authorizes an exact subset of immutable
    /// records. Already-unlinked listed records are valid partial forward progress.
    fn retire_native_amx_publication_records_for_completed_prune(
        &self,
        intent: &KuraPruneIntentV3,
    ) -> Result<()> {
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        for record in inventory.records.values() {
            let listed = intent
                .native_amx_retirement_record_hashes
                .binary_search(&Hash::new(record.encoded()?))
                .is_ok();
            if listed && record.carrier.height <= intent.target_height {
                return Err(Error::PruneIntentConflict(
                    "Native prune retirement list includes a retained carrier".to_owned(),
                ));
            }
            if record.carrier.height > intent.target_height && !listed {
                return Err(Error::PruneIntentConflict(
                    "Native prune residue is not authorized by the exact intent".to_owned(),
                ));
            }
        }
        for record in inventory
            .records
            .values()
            .filter(|record| record.carrier.height > intent.target_height)
        {
            self.remove_native_amx_publication_index_exact(record)?;
        }
        if Self::read_native_amx_publication_index_for_store(&self.store_root)?
            .records
            .keys()
            .any(|carrier| carrier.height > intent.target_height)
        {
            return Err(Error::PruneIntentConflict(
                "Native canonical prune retirement remains incomplete".to_owned(),
            ));
        }
        Ok(())
    }
    fn release_native_amx_capacity_after_completed_prune(&self, target: u64) {
        self.native_amx_publication_capacity_reservations
            .lock()
            .retain(|carrier, _| carrier.height <= target);
    }
    /// New canonical wire and its durable replacement locator prove retirement of
    /// the exact previous tip. Retain both owners on any ambiguous unlink/fsync.
    fn complete_native_amx_replacement_capacity(
        &self,
        old: NativeAmxPublicationCarrier,
        new: &SignedBlock,
    ) -> Result<()> {
        let new_carrier = Self::native_amx_publication_carrier(new)?;
        self.ensure_durable_block_at_height(new_carrier.height, new_carrier.block_hash)?;
        self.ensure_existing_block_wire_matches(new, new_carrier.height, new_carrier.block_hash)?;
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        let old_record = inventory.records.get(&old);
        let new_record = inventory.records.get(&new_carrier);
        if let Some(old_record) = old_record {
            if !new_record.is_some_and(|record| record.replaced == Some(old)) {
                return Err(Error::PruneIntentConflict(
                    "Native replacement lacks its exact retirement locator".to_owned(),
                ));
            }
            self.remove_native_amx_publication_index_exact(old_record)?;
        }
        self.native_amx_publication_capacity_reservations
            .lock()
            .remove(&old);
        let only_retirement = self
            .native_amx_publication_capacity_reservations
            .lock()
            .get(&new_carrier)
            .is_some_and(|plan| plan.routes.is_empty());
        if only_retirement {
            let record = new_record.ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native index-only replacement lost its durable locator".to_owned(),
                )
            })?;
            self.remove_native_amx_publication_index_exact(record)?;
            self.native_amx_publication_capacity_reservations
                .lock()
                .remove(&new_carrier);
        }
        Ok(())
    }
}

impl Kura {
    /// Refresh only the completed inventory after owned pointer/prune mutations.
    /// The protected files must still be the exact objects and bytes admitted
    /// before those sibling mutations. Caller holds prune/canonical/geometry/sidecar.
    fn validate_native_amx_startup_completed_pair_locked(
        &self,
        entry: &LaneConfigEntry,
        namespace: &BoundProgressNamespace,
        admitted: &NativeAmxEvidenceInventory,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
    ) -> Result<()> {
        let height = receipt.participant_proposal.descriptor.lane_block_height;
        let completed = self.inventory_native_amx_evidence_files_locked(namespace, false)?;
        let admitted_manifest = admitted.manifests.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native startup completion lost its admitted manifest".to_owned(),
            )
        })?;
        let admitted_receipt = admitted.receipts.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native startup completion lost its admitted receipt".to_owned(),
            )
        })?;
        let current_manifest = completed.manifests.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native startup completion lost its stable manifest".to_owned(),
            )
        })?;
        let current_receipt = completed.receipts.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native startup completion lost its stable receipt".to_owned(),
            )
        })?;
        for (before, after) in [
            (admitted_manifest, current_manifest),
            (admitted_receipt, current_receipt),
        ] {
            if !Self::stable_sidecar_file_binding_unchanged(&before.metadata, &after.metadata) {
                return Err(Self::invalid_lane_artifact_error(
                    after.path.clone(),
                    "Native startup completion changed an admitted protected file",
                ));
            }
        }
        let current_manifest =
            self.decode_native_amx_manifest_file_locked(entry, namespace, current_manifest)?;
        let current_receipt =
            self.decode_native_amx_receipt_file_locked(entry, namespace, current_receipt)?;
        if &current_manifest != manifest || &current_receipt != receipt {
            return Err(Error::PruneIntentConflict(
                "Native startup completion changed authenticated protected pair bytes".to_owned(),
            ));
        }
        Ok(())
    }
}
