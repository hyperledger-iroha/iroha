// Included at Kura module scope. A prefix is discardable only after independent
// completed-repair authentication, never because it occupies a temporary path.
struct NativeAmxCompletedRepairPrefix {
    target: lane_geometry::NativeAmxReservationPhysicalTarget,
    namespace: BoundProgressNamespace,
    temporary: NativeAmxEvidenceFile,
    opened: std::fs::File,
    prefix: Vec<u8>,
}

impl Kura {
    /// Same-process retry uses the same narrow recovery as Strict startup.
    /// Pristine publication has no CompletedRepair locator and remains temp-free.
    fn recover_native_amx_completed_repair_prefixes_under_publication_guard(
        &self,
        block: &SignedBlock,
    ) -> Result<()> {
        let _canonical = self.canonical_chain_lock.lock();
        self.recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(&[
            Self::native_amx_publication_carrier(block)?,
        ])
    }

    /// Caller retains prune and canonical ownership. Authenticate the complete
    /// bounded carrier set and every affected route before the first unlink.
    /// Exact complete temporaries remain for the existing promotion engine.
    fn recover_native_amx_completed_repair_prefixes_under_prune_and_canonical_guards(
        &self,
        carriers: &[NativeAmxPublicationCarrier],
    ) -> Result<()> {
        let index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        let selected_marker = {
            let mut store = self.block_store.lock();
            let count = store.read_exact_durable_index_count()?;
            store.commit_marker_for_count(count)?
        };
        let mut authenticated = Vec::new();
        for carrier in carriers.iter().copied().collect::<BTreeSet<_>>() {
            let Some(record) = index.records.get(&carrier).filter(|record| {
                record.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair
            }) else {
                continue;
            };
            let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native repair prefix has zero carrier height".to_owned(),
                )
            })?;
            let block = self
                .read_block_body_under_prune_and_canonical_guards(height)?
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native repair prefix lost its canonical carrier".to_owned(),
                    )
                })?;
            if record.classify_resolved_carrier(
                &selected_marker,
                Some(Self::native_amx_publication_carrier(&block)?),
            )? != NativeAmxPublicationIndexResolution::Committed
            {
                return Err(Error::PruneIntentConflict(
                    "Native repair prefix lacks committed selected-wire authority".to_owned(),
                ));
            }
            let merge =
                self.native_amx_capacity_merge_entry_under_prune_and_canonical_guards(&block)?;
            let artifacts = self
                .native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(
                    &block,
                    merge.as_ref(),
                    record,
                )?;
            authenticated.push((record, artifacts));
        }
        if authenticated.is_empty() {
            return Ok(());
        }
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        let mut prefixes = Vec::new();
        for (record, artifacts) in authenticated {
            for (manifest, receipt) in artifacts {
                if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? {
                    return Err(Error::PruneIntentConflict(
                        "Native repair prefix lacks its finalized WSV join".to_owned(),
                    ));
                }
                if !self.native_amx_participant_evidence_pair_fits_stable_bytes(
                    manifest.encode_framed()?.len(),
                    receipt.encode_framed()?.len(),
                ) {
                    return Err(Error::PruneIntentConflict(
                        "Native repair prefix exceeds the authenticated pair byte bound".to_owned(),
                    ));
                }
                let target = self.native_amx_reservation_physical_target_from_journal(
                    &receipt.participant_proposal.descriptor,
                )?;
                let namespace = self.native_amx_evidence_namespace_for_entry(&target)?;
                self.require_active_lane_artifact(
                    &target,
                    &receipt.participant_proposal.descriptor,
                )?;
                self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;
                let prefix = self.open_native_amx_completed_repair_prefix_locked(
                    &target, &namespace, &manifest,
                )?;
                let mut inventory = self.inventory_native_amx_evidence_with_repair_prefix_locked(
                    &namespace,
                    true,
                    prefix.as_ref().map(|(temporary, _, _)| temporary),
                )?;
                // The full original inventory (including actual prefix bytes) has
                // passed every size/count bound. Validate the existing recovery
                // plan with only the independently proven incomplete object absent.
                if let Some((temporary, _, _)) = &prefix {
                    let removed = inventory
                        .temporaries
                        .remove(&NativeAmxEvidenceKind::Manifest);
                    if !removed.as_ref().is_some_and(|file| {
                        file.path == temporary.path
                            && Self::stable_sidecar_metadata_unchanged(
                                &file.metadata,
                                &temporary.metadata,
                            )
                    }) {
                        return Err(Error::PruneIntentConflict(
                            "Native repair prefix differs from its bounded inventory".to_owned(),
                        ));
                    }
                }
                if self
                    .native_amx_route_publication_capacity_with_inventory_locked(
                        &target,
                        &manifest,
                        &receipt,
                        Some((&namespace, &inventory)),
                    )?
                    .is_some()
                {
                    self.require_native_amx_completed_repair_receipt_with_inventory_locked(
                        &target,
                        &receipt,
                        Some((record, &manifest)),
                        &namespace,
                        &inventory,
                    )?;
                } else if prefix.is_some() {
                    return Err(Error::PruneIntentConflict(
                        "Native repair prefix cannot rewrite a later published frontier".to_owned(),
                    ));
                }
                self.require_native_amx_reservation_physical_target(&target)?;
                if let Some((temporary, opened, prefix)) = prefix {
                    prefixes.push(NativeAmxCompletedRepairPrefix {
                        target,
                        namespace,
                        temporary,
                        opened,
                        prefix,
                    });
                }
            }
        }
        // No mutation occurred in the preceding all-route pass. Recheck every
        // retained descriptor before beginning the exact physical cleanup batch.
        for prefix in &mut prefixes {
            self.require_native_amx_reservation_physical_target(&prefix.target)?;
            self.verify_bound_open_regular_file_exact_bytes_locked(
                &prefix.namespace,
                &prefix.temporary.path,
                &mut prefix.opened,
                &prefix.temporary.metadata,
                &prefix.prefix,
                prefix.prefix.len(),
                "Native completed-repair manifest prefix",
            )?;
        }
        if prefixes.is_empty() {
            return Ok(());
        }
        self.durable_mutation_authorized()?;
        let resources = self.begin_total_disk_usage_mutation().with_resource_paths(
            prefixes
                .iter()
                .map(|prefix| prefix.temporary.path.clone())
                .collect(),
        );
        for prefix in &mut prefixes {
            // Previous unlinks can change a shared parent timestamp. Retain the
            // original directory object and the exact file identity and bytes.
            self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(
                &prefix.namespace,
                &prefix.temporary.path,
                &mut prefix.opened,
                &prefix.temporary.metadata,
                &prefix.prefix,
                prefix.prefix.len(),
                "Native completed-repair manifest prefix",
            )?;
            Self::remove_bound_progress_file_if_matches(
                &prefix.namespace,
                &prefix.temporary.path,
                &prefix.opened,
                &prefix.temporary.metadata,
            )
            .map_err(|error| Error::IO(error, prefix.temporary.path.clone()))?;
            self.sync_native_amx_evidence_namespace(
                &prefix.namespace,
                "Native completed-repair prefix removal",
            )?;
            self.require_native_amx_reservation_physical_target(&prefix.target)?;
        }
        // The failed original write may have invalidated cached physical usage.
        // Publish the actual removal and require its normal bounded rescan.
        resources.finish_resources_before_disk_rescan();
        Ok(())
    }

    /// Bind a strict proper prefix (including empty) of the independently
    /// reconstructed manifest to its original no-follow descriptor. A stable
    /// manifest or a complete/wrong payload cannot enter this cleanup phase.
    fn open_native_amx_completed_repair_prefix_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
        namespace: &BoundProgressNamespace,
        manifest: &NativeAmxParticipantApplicationManifestArtifactV1,
    ) -> Result<Option<(NativeAmxEvidenceFile, std::fs::File, Vec<u8>)>> {
        let stable = Self::native_amx_application_manifest_path_for_entry(
            entry,
            &self.store_root,
            manifest.leaf.participant_height,
        );
        let path = stable.with_extension("norito.tmp");
        let directory = path.parent().ok_or_else(|| {
            Error::PruneIntentConflict("Native repair prefix has no parent".to_owned())
        })?;
        let Some(metadata) =
            Self::regular_sidecar_metadata_for(&self.store_root, &path, directory)?
        else {
            return Ok(None);
        };
        let expected = manifest.encode_framed()?;
        let len = usize::try_from(metadata.file.len())?;
        if len >= expected.len() {
            return Ok(None); // Existing full-frame validation retains its strict rejection.
        }
        if metadata.file.len() > STRICT_INIT_MAX_BLOCK_BYTES
            || metadata.file.len() > self.native_amx_participant_evidence_file_bytes()
            || Self::regular_sidecar_metadata_for(&self.store_root, &stable, directory)?.is_some()
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native repair prefix is oversized or has a stable manifest",
            ));
        }
        let mut opened = Self::open_bound_progress_file(namespace, &path, &metadata)?;
        let mut prefix = Vec::new();
        prefix.try_reserve_exact(len)?;
        prefix.resize(len, 0);
        opened
            .read_exact(&mut prefix)
            .map_err(|error| Error::IO(error, path.clone()))?;
        if !expected.starts_with(&prefix) {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native repair temporary is not an exact canonical manifest prefix",
            ));
        }
        self.verify_bound_open_regular_file_exact_bytes_locked(
            namespace,
            &path,
            &mut opened,
            &metadata,
            &prefix,
            len,
            "Native completed-repair manifest prefix",
        )?;
        Ok(Some((
            NativeAmxEvidenceFile {
                kind: NativeAmxEvidenceKind::Manifest,
                participant_height: manifest.leaf.participant_height,
                path,
                metadata,
            },
            opened,
            prefix,
        )))
    }
}
