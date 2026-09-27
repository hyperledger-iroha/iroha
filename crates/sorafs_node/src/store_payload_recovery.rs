// Verified payload recovery for metadata-authenticated, unavailable manifests.

fn payload_integrity_error(error: &StorageError) -> bool {
    matches!(
        error,
        StorageError::ChunkStore(
            ChunkStoreError::Io(_)
                | ChunkStoreError::UnexpectedEof { .. }
                | ChunkStoreError::DigestMismatch { .. }
                | ChunkStoreError::LengthMismatch { .. }
        )
    )
}

fn rebuild_verified_runtime_trees(
    manifest: &StoredManifest,
    profile: ChunkProfile,
) -> Result<(Arc<PorMerkleTree>, Option<Arc<PdpMerkleTreeV1>>), StorageError> {
    let (por, pdp) = rebuild_runtime_trees(manifest, profile)?;
    let metadata_path = manifest.manifest_path().with_file_name(METADATA_FILE_NAME);
    validate_rebuilt_por(
        manifest.por_commitment.as_ref().ok_or_else(|| {
            corrupt_storage_state(&metadata_path, "runtime PoR commitment is missing")
        })?,
        &por,
        &metadata_path,
    )?;
    validate_rebuilt_pdp(
        manifest.pdp_commitment.as_ref(),
        pdp.as_deref(),
        &metadata_path,
    )?;
    Ok((por, pdp))
}

impl StorageBackend {
    /// Rebuild both proof indexes and check the complete canonical CAR before publishing a
    /// repaired runtime snapshot. The finalized repair worker holds this manifest's lifecycle
    /// read lease, and its single-flight owner excludes concurrent repair of the same payload.
    pub(crate) fn publish_verified_repair(
        &self,
        manifest: &StoredManifest,
        check_authority: &dyn Fn() -> Result<
            (),
            crate::native_repair_worker::NativeRepairExecutionErrorV1,
        >,
    ) -> Result<(), crate::native_repair_worker::NativeRepairExecutionErrorV1> {
        self.ensure_durability_healthy()?;
        let (canonical_manifest, _) = manifest.read_manifest_with_bytes()?;
        let profile = chunk_profile_from_manifest(&canonical_manifest)?;
        let plan = manifest.try_to_car_plan(profile)?;
        plan.verify_manifest_metadata(&canonical_manifest)
            .map_err(|error| {
                corrupt_storage_state(
                    manifest.manifest_path(),
                    format!("repair plan identity mismatch: {error}"),
                )
            })?;
        let (por, pdp) = rebuild_verified_runtime_trees(manifest, profile)?;
        let record = manifest.to_record()?;
        let chunks_dir = manifest.manifest_path().with_file_name(CHUNKS_DIR_NAME);
        verify_staged_manifest_car_archive(
            &canonical_manifest,
            &plan,
            &record.chunk_files,
            &chunks_dir,
        )?;
        check_authority()?;
        let mut state = self.state.write().expect("storage state poisoned");
        self.ensure_durability_healthy()?;
        let current = state
            .manifests
            .get_mut(manifest.manifest_id())
            .ok_or_else(|| StorageError::ManifestNotFound {
                manifest_id: manifest.manifest_id().to_owned(),
            })?;
        // Metadata updates and eviction take the exclusive lifecycle lease, so this identity
        // must remain unchanged for the caller's complete repair operation.
        if !Arc::ptr_eq(&current.io_lock, &manifest.io_lock) {
            return Err(corrupt_storage_state(
                manifest.manifest_path(),
                "repair lifecycle changed",
            )
            .into());
        }
        let mut repaired = current.try_clone_runtime()?;
        repaired.por_tree = por;
        repaired.pdp_tree = pdp;
        repaired.payload_available = Arc::new(AtomicBool::new(true));
        *current = Arc::new(repaired);
        Ok(())
    }
}
