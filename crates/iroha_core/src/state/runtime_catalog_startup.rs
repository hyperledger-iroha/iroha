impl State {
    /// Install a frozen manifest source against the startup or replay catalog.
    ///
    /// The caller must supply the catalog reconstructed from configured genesis
    /// authority or the authenticated replay state. Validation holds the State writer.
    /// An identical complete authority retains the original State handles and generation,
    /// including during the post-replay Queue handoff. Only an actual source change begins
    /// a publication; refused sources leave the certified World cut unchanged.
    /// The returned owner is the exact installed privacy graph, shared with Queue
    /// without a second allocation, including the unchanged-authority handoff.
    ///
    /// # Errors
    /// Rejects status-only registries, altered source bodies or digests,
    /// mismatched catalog bindings, and incomplete active-lane coverage.
    pub fn install_materialized_lane_manifests_for_catalog(
        &self,
        manifests: &LaneManifestRegistryHandle,
        catalog: &LaneCatalog,
        governance: &iroha_config::parameters::actual::GovernanceCatalog,
    ) -> Result<LanePrivacyRegistry, LaneLifecycleError> {
        let pool = self.ivm_execution_budget();
        // This scope outlives every physical publication guard on all exits.
        pool.with_deferred_refund_notifications(|_| {
            let mut publication_notice = self.state_view_publication();
            let mut releases = LaneLifecycleReleases::new(self);
            let mut state_write_release = self.state_write_lock.defer_notifications();
            let _state_write_lock = state_write_release.lock();
            let candidate_bytes = manifests
                .canonical_materialized_authority_preimage(catalog, governance)
                .map_err(runtime_catalog_invalid)?;
            manifests
                .validate_active_coverage_for_catalog(catalog)
                .map_err(|error| LaneLifecycleError::ManifestPolicyUnavailable {
                    lane: error.lane,
                    reason: error.message(),
                })?;
            {
                let current = releases.manifests.read();
                if current
                    .canonical_materialized_authority_preimage(catalog, governance)
                    .is_ok_and(|current_bytes| current_bytes == candidate_bytes)
                {
                    let installed = releases.privacy.read().clone();
                    if !installed.belongs_to(&pool) {
                        return Err(LaneLifecycleError::ParameterDeferred(
                            ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
                        ));
                    }
                    return Ok(installed);
                }
            }
            let privacy = LanePrivacyRegistry::try_from_manifest_registry(manifests, &pool)
                .map_err(LaneLifecycleError::ParameterDeferred)?;
            let publication = publication_notice.begin();
            Self::install_prepared_lane_manifests_in_publication(
                Arc::clone(manifests),
                privacy.clone(),
                &publication,
                &mut releases,
            );
            Ok(privacy)
        })
    }

    /// Accept a semantic-preserving refresh without replacing installed State authority.
    ///
    /// Queue and telemetry own refreshed local paths and diagnostics. State retains the
    /// original immutable sources after authenticating the candidate's complete materialized
    /// authority under its writer. Neither acceptance nor refusal publishes a new State
    /// generation: an unchanged manifest must not invalidate the certified World cut.
    pub(crate) fn install_lane_manifests_if_consensus_compatible(
        &self,
        manifests: &LaneManifestRegistryHandle,
    ) -> Result<LanePrivacyRegistry, LaneLifecycleError> {
        let pool = self.ivm_execution_budget();
        let mut releases = LaneLifecycleReleases::new(self);
        let mut state_write_release = self.state_write_lock.defer_notifications();
        let _state_write_lock = state_write_release.lock();
        let nexus = self
            .canonical_runtime
            .view()
            .nexus_projection(&releases.nexus.read())?;
        let candidate_bytes = manifests
            .canonical_materialized_authority_preimage(&nexus.lane_catalog, &nexus.governance)
            .map_err(runtime_catalog_invalid)?;
        let current = releases.manifests.read();
        let current_bytes = current
            .canonical_materialized_authority_preimage(&nexus.lane_catalog, &nexus.governance)
            .map_err(runtime_catalog_invalid)?;
        if current_bytes != candidate_bytes {
            return Err(runtime_catalog_invalid(
                "refreshed manifest changes installed authority",
            ));
        }
        let installed = releases.privacy.read().clone();
        if !installed.belongs_to(&pool) {
            return Err(LaneLifecycleError::ParameterDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation.into(),
            ));
        }
        Ok(installed)
    }

    /// Derive effective dataspaces from the configured baseline and protected committed catalog.
    ///
    /// This read-only projection can be checked before an imported snapshot authorizes runtime
    /// publication. It never promotes restored or locally supplied additions into static policy.
    ///
    /// # Errors
    /// Rejects malformed committed additions or a configured physical baseline
    /// that differs from retained snapshot or post-genesis authority.
    pub fn nexus_with_committed_catalog(
        &self,
        mut nexus: iroha_config::parameters::actual::Nexus,
    ) -> Result<iroha_config::parameters::actual::Nexus, LaneLifecycleError> {
        // Decode from the same protected parameter-cell version without acquiring the
        // unrelated World inventory; all catalog and baseline checks below stay native.
        let runtime = runtime_catalog_from_parameters(self.world.parameters.view().get())?;
        nexus.dataspace_catalog =
            runtime_catalog_dataspaces(&nexus.configured_dataspace_catalog, runtime.as_ref())?;
        // An absent overlay still has an authoritative physical baseline once
        // State is restored or committed. Only fresh H0 construction/replay may
        // replace placeholder defaults with the configured initial baseline.
        if self.nexus_runtime_restored_from_snapshot || self.committed_height() != 0 {
            let retained = self.nexus_snapshot();
            if SnapshotNexusOwnerPolicy::from_nexus(&nexus).dataspaces
                != SnapshotNexusOwnerPolicy::from_nexus(&retained).dataspaces
            {
                return Err(runtime_catalog_invalid(
                    "configured catalog differs from retained physical dataspace authority",
                ));
            }
        }
        Ok(nexus)
    }

    /// Reconstruct effective manifests from frozen baseline sources and committed additions.
    ///
    /// # Errors
    /// Rejects changed baseline policy, inconsistent effective dataspaces, or invalid manifests.
    pub fn lane_manifests_with_committed_catalog(
        &self,
        baseline: &LaneManifestRegistryHandle,
        nexus: &iroha_config::parameters::actual::Nexus,
    ) -> Result<LaneManifestRegistryHandle, LaneLifecycleError> {
        baseline
            .validate_materialized_source_projection()
            .map_err(runtime_catalog_invalid)?;
        // Decode from the same protected parameter-cell version without acquiring the
        // unrelated World inventory; all catalog and baseline checks below stay native.
        let runtime = runtime_catalog_from_parameters(self.world.parameters.view().get())?;
        materialized_lane_manifests(baseline, nexus, runtime)
    }
}

/// The configured lane policy a fresh State installs before its genesis: the static baseline
/// and no committed runtime catalog. A replay that starts again from signed genesis (the
/// isolated FASTPQ replay) needs it after a snapshot restore, whose State instead carries the
/// effective catalogs of its height.
///
/// # Errors
/// Rejects a changed baseline projection, incomplete active-lane coverage or invalid manifests.
pub fn pregenesis_lane_policy(
    mut nexus: iroha_config::parameters::actual::Nexus,
    baseline: &LaneManifestRegistryHandle,
) -> Result<
    (
        iroha_config::parameters::actual::Nexus,
        LaneManifestRegistryHandle,
    ),
    LaneLifecycleError,
> {
    baseline
        .validate_materialized_source_projection()
        .map_err(runtime_catalog_invalid)?;
    nexus.dataspace_catalog =
        runtime_catalog_dataspaces(&nexus.configured_dataspace_catalog, None)?;
    let manifests = materialized_lane_manifests(baseline, &nexus, None)?;
    Ok((nexus, manifests))
}

// Effective manifests from frozen baseline sources and, once committed, the runtime catalog.
fn materialized_lane_manifests(
    baseline: &LaneManifestRegistryHandle,
    nexus: &iroha_config::parameters::actual::Nexus,
    runtime: Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>,
) -> Result<LaneManifestRegistryHandle, LaneLifecycleError> {
    let expected_dataspaces =
        runtime_catalog_dataspaces(&nexus.configured_dataspace_catalog, runtime.as_ref())?;
    if expected_dataspaces != nexus.dataspace_catalog {
        return Err(runtime_catalog_invalid(
            "effective dataspaces differ from the committed catalog",
        ));
    }
    let registry = match runtime {
        Some(runtime) => {
            if runtime.baseline_manifests_hash
                != Hash::prehashed(baseline.baseline_consensus_policy_digest())
            {
                return Err(runtime_catalog_invalid(
                    "configured manifest baseline differs from committed catalog authority",
                ));
            }
            runtime_catalog_manifest_registry(
                baseline.as_ref(),
                &runtime.manifests,
                &nexus.lane_catalog,
                &nexus.dataspace_catalog,
                &nexus.governance,
            )?
        }
        None => runtime_catalog_manifest_registry(
            baseline.as_ref(),
            &[],
            &nexus.lane_catalog,
            &nexus.dataspace_catalog,
            &nexus.governance,
        )?,
    };
    registry
        .validate_active_coverage_for_catalog(&nexus.lane_catalog)
        .map_err(|error| LaneLifecycleError::ManifestPolicyUnavailable {
            lane: error.lane,
            reason: error.message(),
        })?;
    registry
        .canonical_materialized_authority_preimage(&nexus.lane_catalog, &nexus.governance)
        .map_err(runtime_catalog_invalid)?;
    Ok(Arc::new(registry))
}
