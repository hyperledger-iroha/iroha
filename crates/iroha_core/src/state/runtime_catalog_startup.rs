impl State {
    /// Install a semantic-preserving manifest refresh without racing a catalog publication.
    pub(crate) fn install_lane_manifests_if_consensus_compatible(
        &self,
        manifests: &LaneManifestRegistryHandle,
    ) -> bool {
        let privacy = Arc::new(LanePrivacyRegistry::from_manifest_registry(manifests));
        let manifests = Arc::clone(manifests);
        let _state_write_lock = self.state_write_lock.lock();
        let publication = self.begin_state_view_write();
        if !manifests.is_bound_to_catalog(&self.nexus_ownership_projection().lane_catalog) {
            return false;
        }
        {
            let current = self.lane_manifests.read();
            if current.consensus_policy_digest() != manifests.consensus_policy_digest()
                || current.baseline_consensus_policy_digest()
                    != manifests.baseline_consensus_policy_digest()
            {
                return false;
            }
        }
        self.install_prepared_lane_manifests_in_publication(manifests, privacy, &publication);
        true
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
        let runtime = runtime_catalog_from_world(&self.world.view())?;
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
        let runtime = runtime_catalog_from_world(&self.world.view())?;
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
                baseline
                    .with_runtime_additions(
                        &runtime.manifests,
                        &nexus.lane_catalog,
                        &nexus.dataspace_catalog,
                        &nexus.governance,
                    )
                    .map_err(runtime_catalog_invalid)?
            }
            None => baseline
                .with_runtime_additions(
                    &[],
                    &nexus.lane_catalog,
                    &nexus.dataspace_catalog,
                    &nexus.governance,
                )
                .map_err(runtime_catalog_invalid)?,
        };
        registry
            .validate_active_coverage_for_catalog(&nexus.lane_catalog)
            .map_err(|error| LaneLifecycleError::ManifestPolicyUnavailable {
                lane: error.lane,
                reason: error.message(),
            })?;
        Ok(Arc::new(registry))
    }
}
