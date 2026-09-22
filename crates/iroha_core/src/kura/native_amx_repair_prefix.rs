// Included at Kura module scope. A prefix is discardable only after independent
// indexed-publication authentication, never because it occupies a temporary path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeAmxPrefixRecoveryScope {
    Indexed,
    Startup,
}
struct NativeAmxIndexedPrefixFile {
    component: NativeAmxPublicationComponent,
    path: PathBuf,
    metadata: StableSidecarMetadata,
    opened: std::fs::File,
    prefix: Vec<u8>,
}
struct NativeAmxIndexedPublicationPrefix {
    target: lane_geometry::NativeAmxReservationPhysicalTarget,
    namespace: BoundProgressNamespace,
    file: NativeAmxIndexedPrefixFile,
}

impl Kura {
    /// Same-process retry and Strict startup share the original indexed owner.
    /// Pristine unindexed admission still has no authority to remove a temporary.
    fn recover_native_amx_indexed_publication_prefixes_under_publication_guard(
        &self,
        block: &SignedBlock,
    ) -> Result<()> {
        let _canonical = self.canonical_chain_lock.lock();
        self.recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(
            &[Self::native_amx_publication_carrier(block)?],
            NativeAmxPrefixRecoveryScope::Indexed,
        )
    }

    /// A CanonicalWrite may precede finality and every evidence write. Discover
    /// only its exact bounded artifact paths before demanding writer authority.
    /// Path presence alone never authorizes either acceptance or deletion.
    fn native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        merge: Option<&MergeLedgerEntry>,
    ) -> Result<bool> {
        let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, merge)
            .map_err(|error| Error::PruneIntentConflict(format!("Native indexed prefix discovery manifest: {error}")))?;
        let artifacts = native_amx_participant_application_artifacts(
            &manifest,
            native_amx_participant_application_finality_placeholder_hash(),
        )
        .ok_or_else(|| {
            Error::PruneIntentConflict(
                "Native indexed prefix discovery lacks an artifact plan".to_owned(),
            )
        })?;
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        for (_, receipt) in artifacts {
            let descriptor = &receipt.participant_proposal.descriptor;
            let target = self.native_amx_reservation_physical_target_from_journal(descriptor)?;
            let manifest_path = Self::native_amx_application_manifest_path_for_entry(
                &target,
                &self.store_root,
                descriptor.lane_block_height,
            );
            let receipt_path = Self::native_amx_participant_receipt_path_for_entry(
                &target,
                &self.store_root,
                descriptor.lane_block_height,
            );
            if self.bound_progress_sidecar_directory_is_absent(&manifest_path, &receipt_path)? {
                continue;
            }
            let directory = manifest_path.parent().ok_or_else(|| {
                Error::PruneIntentConflict("Native indexed prefix path has no parent".to_owned())
            })?;
            for path in [
                manifest_path.with_extension("norito.tmp"),
                receipt_path.with_extension("norito.tmp"),
                directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE),
            ] {
                if Self::regular_sidecar_metadata_for(&self.store_root, &path, directory)?.is_some()
                {
                    self.require_native_amx_reservation_physical_target(&target)?;
                    return Ok(true);
                }
            }
            self.require_native_amx_reservation_physical_target(&target)?;
        }
        Ok(false)
    }

    /// Caller retains prune and canonical ownership. Authenticate the complete
    /// bounded carrier set and all routes before the first unlink. Complete
    /// temporaries remain for the existing strict promotion engine.
    fn recover_native_amx_indexed_publication_prefixes_under_prune_and_canonical_guards(
        &self,
        carriers: &[NativeAmxPublicationCarrier],
        scope: NativeAmxPrefixRecoveryScope,
    ) -> Result<()> {
        let index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        let selected_marker = {
            let mut store = self.block_store.lock();
            let count = store.read_exact_durable_index_count()?;
            store.commit_marker_for_count(count)?
        };
        let mut authenticated = Vec::new();
        for carrier in carriers.iter().copied().collect::<BTreeSet<_>>() {
            let Some(record) = index.records.get(&carrier) else {
                continue;
            };
            let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native indexed prefix has zero carrier height".to_owned(),
                )
            })?;
            let block = self
                .get_block_without_merge_sidecar(height)
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native indexed prefix lost its canonical carrier".to_owned(),
                    )
                })?;
            if record.classify_resolved_carrier(
                &selected_marker,
                Some(Self::native_amx_publication_carrier(&block)?),
            )? != NativeAmxPublicationIndexResolution::Committed
            {
                return Err(Error::PruneIntentConflict(
                    "Native indexed prefix lacks committed selected-wire authority".to_owned(),
                ));
            }
            let merge =
                self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;
            if record.merge_entry_hash != merge.as_ref().map(MergeLedgerEntry::canonical_hash) {
                return Err(Error::PruneIntentConflict(
                    "Native indexed prefix changed its original merge association".to_owned(),
                ));
            }
            if record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite
                && !self
                    .native_amx_indexed_publication_has_temporary_under_prune_and_canonical_guards(
                        &block,
                        merge.as_ref(),
                    )?
            {
                // No derived writer ran: preserve the legitimate pre-finality cut.
                continue;
            }
            let artifacts = self
                .native_amx_indexed_publication_artifacts_under_prune_and_canonical_guards(
                    &block,
                    merge.as_ref(),
                    record,
                )?;
            authenticated.push((record, artifacts));
        }
        if authenticated.is_empty() && scope == NativeAmxPrefixRecoveryScope::Indexed {
            return Ok(());
        }
        let indexed_routes = authenticated
            .iter()
            .flat_map(|(_, artifacts)| {
                artifacts.iter().map(|(_, receipt)| {
                    let descriptor = &receipt.participant_proposal.descriptor;
                    (descriptor.lane_id, descriptor.lane_incarnation)
                })
            })
            .collect::<BTreeSet<_>>();
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        let mut prefixes = Vec::new();
        for (record, artifacts) in authenticated {
            for (manifest, receipt) in artifacts {
                if record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair
                    && !self
                        .native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?
                {
                    return Err(Error::PruneIntentConflict(
                        "Native completed-repair prefix lacks its finalized WSV join".to_owned(),
                    ));
                }
                if !self.native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards(&manifest) {
                    return Err(Error::PruneIntentConflict("Native indexed prefix differs from authenticated finality".to_owned()));
                }
                let manifest_bytes = manifest.encode_framed()?;
                let receipt_bytes = receipt.encode_framed()?;
                if !self.native_amx_participant_evidence_pair_fits_stable_bytes(
                    manifest_bytes.len(),
                    receipt_bytes.len(),
                ) {
                    return Err(Error::PruneIntentConflict(
                        "Native indexed prefix exceeds the authenticated pair byte bound"
                            .to_owned(),
                    ));
                }
                let descriptor = &receipt.participant_proposal.descriptor;
                let target =
                    self.native_amx_reservation_physical_target_from_journal(descriptor)?;
                let namespace = self.native_amx_evidence_namespace_for_entry(&target)?;
                self.require_active_lane_artifact(&target, descriptor)?;
                self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;
                let manifest_path = Self::native_amx_application_manifest_path_for_entry(
                    &target,
                    &self.store_root,
                    descriptor.lane_block_height,
                );
                let receipt_path = Self::native_amx_participant_receipt_path_for_entry(
                    &target,
                    &self.store_root,
                    descriptor.lane_block_height,
                );
                let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(
                    &target,
                    &self.store_root,
                );
                let latest_temp = latest_path
                    .parent()
                    .expect("bound latest directory")
                    .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);
                let latest_bytes = norito::encode_canonical(
                    &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt),
                )?;
                let mut route_prefix = None;
                for (component, stable, path, expected) in [
                    (
                        NativeAmxPublicationComponent::Manifest,
                        &manifest_path,
                        manifest_path.with_extension("norito.tmp"),
                        manifest_bytes.as_slice(),
                    ),
                    (
                        NativeAmxPublicationComponent::Receipt,
                        &receipt_path,
                        receipt_path.with_extension("norito.tmp"),
                        receipt_bytes.as_slice(),
                    ),
                    (
                        NativeAmxPublicationComponent::Latest,
                        &latest_path,
                        latest_temp,
                        latest_bytes.as_slice(),
                    ),
                ] {
                    if let Some(prefix) = self.open_native_amx_indexed_publication_prefix_locked(
                        &namespace, component, stable, &path, expected,
                    )? {
                        if route_prefix.replace(prefix).is_some() {
                            return Err(Error::PruneIntentConflict(
                                "Native indexed route has ambiguous simultaneous partial writes"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                let evidence_prefix = route_prefix.as_ref().and_then(|prefix| {
                    let kind = match prefix.component {
                        NativeAmxPublicationComponent::Manifest => NativeAmxEvidenceKind::Manifest,
                        NativeAmxPublicationComponent::Receipt => NativeAmxEvidenceKind::Receipt,
                        NativeAmxPublicationComponent::Latest => return None,
                    };
                    Some(NativeAmxEvidenceFile {
                        kind,
                        participant_height: descriptor.lane_block_height,
                        path: prefix.path.clone(),
                        metadata: prefix.metadata.clone(),
                    })
                });
                let mut inventory = self.inventory_native_amx_evidence_with_indexed_prefix_locked(
                    &namespace,
                    true,
                    evidence_prefix.as_ref(),
                )?;
                // Bounds include the original prefix bytes before the strictly
                // authenticated incomplete object is omitted from decode planning.
                if let Some(temporary) = &evidence_prefix {
                    let removed = inventory.temporaries.remove(&temporary.kind);
                    if !removed.as_ref().is_some_and(|file| {
                        file.path == temporary.path
                            && Self::stable_sidecar_metadata_unchanged(
                                &file.metadata,
                                &temporary.metadata,
                            )
                    }) {
                        return Err(Error::PruneIntentConflict(
                            "Native indexed prefix differs from its bounded inventory".to_owned(),
                        ));
                    }
                }
                let latest_prefix = route_prefix
                    .as_ref()
                    .filter(|prefix| prefix.component == NativeAmxPublicationComponent::Latest);
                // Use the original incoming-pair validator for every route. It
                // checks complete/wrong temporaries and all retained continuity.
                if record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite
                    || route_prefix.is_some()
                {
                    self.preflight_native_amx_incoming_artifacts_locked(
                        &target, &namespace, &inventory, &manifest, &receipt,
                    )?;
                }
                if self
                    .native_amx_route_publication_capacity_with_inventory_locked(
                        &target,
                        &manifest,
                        &receipt,
                        Some((&namespace, &inventory)),
                        latest_prefix,
                    )?
                    .is_some()
                {
                    if record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair {
                        self.require_native_amx_completed_repair_receipt_with_inventory_locked(
                            &target,
                            &receipt,
                            Some((record, &manifest)),
                            &namespace,
                            &inventory,
                        )?;
                    }
                } else if route_prefix.is_some() {
                    return Err(Error::PruneIntentConflict(
                        "Native indexed prefix cannot rewrite a later published frontier"
                            .to_owned(),
                    ));
                }
                self.require_native_amx_reservation_physical_target(&target)?;
                if let Some(file) = route_prefix {
                    prefixes.push(NativeAmxIndexedPublicationPrefix {
                        target,
                        namespace,
                        file,
                    });
                }
            }
        }
        if scope == NativeAmxPrefixRecoveryScope::Startup {
            self.collect_native_amx_completed_pair_latest_prefixes_locked(
                &index,
                &indexed_routes,
                &mut prefixes,
            )?;
        }
        // All authorities and routes passed without mutation. Retain exact file
        // and directory objects through both this pass and every durable unlink.
        for prefix in &mut prefixes {
            self.require_native_amx_reservation_physical_target(&prefix.target)?;
            self.verify_bound_open_regular_file_exact_bytes_locked(
                &prefix.namespace,
                &prefix.file.path,
                &mut prefix.file.opened,
                &prefix.file.metadata,
                &prefix.file.prefix,
                prefix.file.prefix.len(),
                "Native indexed publication prefix",
            )?;
        }
        if prefixes.is_empty() {
            return Ok(());
        }
        self.durable_mutation_authorized()?;
        let resources = self.begin_total_disk_usage_mutation().with_resource_paths(
            prefixes
                .iter()
                .map(|prefix| prefix.file.path.clone())
                .collect(),
        );
        for prefix in &mut prefixes {
            self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(
                &prefix.namespace,
                &prefix.file.path,
                &mut prefix.file.opened,
                &prefix.file.metadata,
                &prefix.file.prefix,
                prefix.file.prefix.len(),
                "Native indexed publication prefix",
            )?;
            Self::remove_bound_progress_file_if_matches(
                &prefix.namespace,
                &prefix.file.path,
                &prefix.file.opened,
                &prefix.file.metadata,
            )
            .map_err(|error| Error::IO(error, prefix.file.path.clone()))?;
            self.sync_native_amx_evidence_namespace(
                &prefix.namespace,
                "Native indexed prefix removal",
            )?;
            self.require_native_amx_reservation_physical_target(&prefix.target)?;
        }
        resources.finish_resources_before_disk_rescan();
        Ok(())
    }

    /// Strict startup may repair only a derived latest pointer of an already
    /// completed pair without an index. Collect every bounded journal route
    /// before the shared descriptor verification/unlink pass; live indexed
    /// callers never gain this separate maintenance authority.
    fn collect_native_amx_completed_pair_latest_prefixes_locked(
        &self,
        index: &NativeAmxPublicationIndexInventory,
        indexed_routes: &BTreeSet<(LaneId, Hash)>,
        prefixes: &mut Vec<NativeAmxIndexedPublicationPrefix>,
    ) -> Result<()> {
        for location in self.native_amx_evidence_physical_locations_from_journal()? {
            if indexed_routes.contains(&(location.lane_id(), location.incarnation())) {
                continue;
            }
            let directory = Self::lane_artifact_dir(location.blocks_path());
            let latest_path = directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE);
            let latest_temp =
                directory.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);
            if Self::regular_sidecar_metadata_for(&self.store_root, &latest_temp, &directory)?
                .is_none()
            {
                continue;
            }
            self.require_native_amx_evidence_physical_location(&location)?;
            let manifest_path = directory.join(Self::native_amx_evidence_file_name(
                NativeAmxEvidenceKind::Manifest,
                1,
            ));
            let receipt_path = directory.join(Self::native_amx_evidence_file_name(
                NativeAmxEvidenceKind::Receipt,
                1,
            ));
            let namespace = self.open_bound_progress_namespace(&manifest_path, &receipt_path)?;
            let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;
            if !inventory.temporaries.is_empty() {
                return Err(Error::PruneIntentConflict(
                    "Native maintenance latest prefix overlaps unindexed pair publication"
                        .to_owned(),
                ));
            }
            let height = inventory
                .manifests
                .keys()
                .chain(inventory.receipts.keys())
                .copied()
                .max()
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native maintenance latest prefix lacks its stable pair".to_owned(),
                    )
                })?;
            let manifest_file = inventory.manifests.get(&height).ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native maintenance latest prefix lacks its highest stable manifest".to_owned(),
                )
            })?;
            let receipt_file = inventory.receipts.get(&height).ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native maintenance latest prefix lacks its highest stable receipt".to_owned(),
                )
            })?;
            let receipt_bytes =
                self.read_native_amx_evidence_file_bytes_locked(&namespace, receipt_file)?;
            let observed =
                norito::decode_canonical::<NativeAmxParticipantApplicationReceiptArtifact>(
                    &receipt_bytes,
                )
                .map_err(|error| {
                    Self::invalid_lane_artifact_error(
                        receipt_file.path.clone(),
                        format!(
                            "Native maintenance receipt discovery failed exact decode: {error}"
                        ),
                    )
                })?;
            let target = self.native_amx_reservation_physical_target_from_location(
                &location,
                observed.participant_proposal.descriptor.dataspace_id,
            )?;
            let manifest =
                self.decode_native_amx_manifest_file_locked(&target, &namespace, manifest_file)?;
            let receipt =
                self.decode_native_amx_receipt_file_locked(&target, &namespace, receipt_file)?;
            let carrier = NativeAmxPublicationCarrier {
                height: receipt.application_block_height,
                block_hash: receipt.application_block_hash,
                executed_wire_hash: receipt.executed_block_wire_hash,
            };
            if index.records.contains_key(&carrier) {
                return Err(Error::PruneIntentConflict(
                    "Native maintenance latest prefix cannot borrow indexed publication authority"
                        .to_owned(),
                ));
            }
            self.ensure_durable_block_at_height(carrier.height, carrier.block_hash)?;
            if !self.native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards(&receipt, &manifest)
                || !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)?
            {
                return Err(Error::PruneIntentConflict(
                    "Native maintenance latest prefix lacks completed-pair authority".to_owned(),
                ));
            }
            let expected = norito::encode_canonical(
                &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt),
            )?;
            let prefix = self.open_native_amx_indexed_publication_prefix_locked(
                &namespace,
                NativeAmxPublicationComponent::Latest,
                &latest_path,
                &latest_temp,
                &expected,
            )?;
            self.preflight_native_amx_incoming_artifacts_locked(
                &target, &namespace, &inventory, &manifest, &receipt,
            )?;
            let Some((_, capacity)) = self
                .native_amx_route_publication_capacity_with_inventory_locked(
                    &target,
                    &manifest,
                    &receipt,
                    Some((&namespace, &inventory)),
                    prefix.as_ref(),
                )?
            else {
                return Err(Error::PruneIntentConflict(
                    "Native maintenance latest prefix cannot rewrite a later frontier".to_owned(),
                ));
            };
            if capacity
                .outstanding_components
                .iter()
                .any(|component| *component != NativeAmxPublicationComponent::Latest)
            {
                return Err(Error::PruneIntentConflict(
                    "Native maintenance latest prefix cannot authorize missing pair publication"
                        .to_owned(),
                ));
            }
            self.require_native_amx_reservation_physical_target(&target)?;
            if let Some(file) = prefix {
                prefixes.push(NativeAmxIndexedPublicationPrefix {
                    target,
                    namespace,
                    file,
                });
            }
        }
        Ok(())
    }

    /// Open only a strict proper prefix of independently reconstructed bytes.
    /// Latest may have a stable predecessor; pair files must still be absent.
    fn open_native_amx_indexed_publication_prefix_locked(
        &self,
        namespace: &BoundProgressNamespace,
        component: NativeAmxPublicationComponent,
        stable: &Path,
        path: &Path,
        expected: &[u8],
    ) -> Result<Option<NativeAmxIndexedPrefixFile>> {
        let directory = path.parent().ok_or_else(|| {
            Error::PruneIntentConflict("Native indexed prefix has no parent".to_owned())
        })?;
        let Some(metadata) = Self::regular_sidecar_metadata_for(&self.store_root, path, directory)?
        else {
            return Ok(None);
        };
        let len = usize::try_from(metadata.file.len())?;
        if len >= expected.len() {
            return Ok(None);
        }
        let limit = if component == NativeAmxPublicationComponent::Latest {
            u64::try_from(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES)?
        } else {
            self.native_amx_participant_evidence_file_bytes()
        };
        if metadata.file.len() > STRICT_INIT_MAX_BLOCK_BYTES
            || metadata.file.len() > limit
            || (component != NativeAmxPublicationComponent::Latest
                && Self::regular_sidecar_metadata_for(&self.store_root, stable, directory)?
                    .is_some())
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "Native indexed prefix is oversized or overlaps a stable pair file",
            ));
        }
        let mut opened = Self::open_bound_progress_file(namespace, path, &metadata)?;
        let mut prefix = Vec::new();
        prefix.try_reserve_exact(len)?;
        prefix.resize(len, 0);
        opened
            .read_exact(&mut prefix)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !expected.starts_with(&prefix) {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "Native indexed temporary is not an exact canonical prefix",
            ));
        }
        self.verify_bound_open_regular_file_exact_bytes_locked(
            namespace,
            path,
            &mut opened,
            &metadata,
            &prefix,
            len,
            "Native indexed publication prefix",
        )?;
        Ok(Some(NativeAmxIndexedPrefixFile {
            component,
            path: path.to_path_buf(),
            metadata,
            opened,
            prefix,
        }))
    }

    /// A latest prefix is excluded only from its original capacity preflight,
    /// after both exact stable artifacts and the non-overlapping phase join.
    fn require_native_amx_indexed_latest_prefix_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
        namespace: &BoundProgressNamespace,
        inventory: &NativeAmxEvidenceInventory,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
        prefix: &NativeAmxIndexedPrefixFile,
    ) -> Result<()> {
        let expected_path = namespace
            .data_path
            .parent()
            .expect("bound Native directory")
            .join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE);
        let height = receipt.participant_proposal.descriptor.lane_block_height;
        if prefix.component != NativeAmxPublicationComponent::Latest
            || prefix.path != expected_path
            || !inventory.temporaries.is_empty()
        {
            return Err(Error::PruneIntentConflict(
                "Native latest prefix ambiguously overlaps another publication phase".to_owned(),
            ));
        }
        self.require_native_amx_evidence_prune_intent_absent_locked(namespace)?;
        let retained_manifest = inventory.manifests.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict("Native latest prefix lacks its stable manifest".to_owned())
        })?;
        let retained_receipt = inventory.receipts.get(&height).ok_or_else(|| {
            Error::PruneIntentConflict("Native latest prefix lacks its stable receipt".to_owned())
        })?;
        if self.decode_native_amx_manifest_file_locked(entry, namespace, retained_manifest)?
            != *manifest
            || self.decode_native_amx_receipt_file_locked(entry, namespace, retained_receipt)?
                != *receipt
        {
            return Err(Error::PruneIntentConflict(
                "Native latest prefix differs from its exact stable pair".to_owned(),
            ));
        }
        Ok(())
    }
}
