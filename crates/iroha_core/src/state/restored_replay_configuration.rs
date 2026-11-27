// Shared restored-State startup preparation; never applies a height-zero geometry branch.

/// Original configuration refusal before a restored State enters ordinary replay.
#[derive(Debug, thiserror::Error)]
pub enum RestoredReplayConfigurationError {
    /// Only an actually decoded snapshot has restored geometry authority.
    #[error("restored replay preparation requires a decoded positive-height snapshot")]
    NotRestored,
    /// Frozen compliance custody differs from the configured enabled/audit policy.
    #[error("restored replay compliance owner differs from its configured policy")]
    ComplianceOwner,
    /// Original materialized manifest, catalog, geometry or storage failure.
    #[error(transparent)]
    Lane(#[from] LaneLifecycleError),
    /// The frozen compliance source does not cover the effective restored catalog.
    #[error(transparent)]
    Compliance(#[from] crate::compliance::LaneComplianceCoverageError),
    /// Original committed staking policy does not cover SCCP's retained liability.
    #[error(transparent)]
    Staking(#[from] crate::smartcontracts::isi::sccp::params::SccpPolicyError),
}

impl State {
    /// Install the same frozen policies and restored geometry for startup and offline replay.
    ///
    /// The decoded snapshot retains its effective catalogs and autoscale cooldown. Configured
    /// baselines and node policy remain explicit inputs; committed additions are reconstructed
    /// by their ordinary owners. This helper never falls back to genesis or scans policy files.
    /// Complete-State authentication and certified execution must still follow.
    ///
    /// # Errors
    /// A non-restored State, mismatched frozen compliance source, invalid effective policy,
    /// or the original manifest, geometry, storage or staking-policy refusal.
    pub fn prepare_restored_replay_configuration(
        &mut self,
        mut configured: iroha_config::parameters::actual::Nexus,
        baseline: &LaneManifestRegistryHandle,
        compliance: Option<Arc<LaneComplianceEngine>>,
    ) -> Result<(), RestoredReplayConfigurationError> {
        if !self.nexus_runtime_restored_from_snapshot() || self.committed_height() == 0 {
            return Err(RestoredReplayConfigurationError::NotRestored);
        }
        if configured.compliance.enabled != compliance.is_some()
            || compliance
                .as_ref()
                .is_some_and(|engine| engine.audit_only() != configured.compliance.audit_only)
        {
            return Err(RestoredReplayConfigurationError::ComplianceOwner);
        }
        let restored = self.nexus_snapshot();
        configured.lane_catalog = restored.lane_catalog;
        configured.lane_config = restored.lane_config;
        configured.dataspace_catalog = restored.dataspace_catalog;
        configured.autoscale.last_transition_height = restored.autoscale.last_transition_height;
        let nexus = self.nexus_with_committed_catalog(configured)?;
        let manifests = self.lane_manifests_with_committed_catalog(baseline, &nexus)?;
        if let Some(engine) = &compliance {
            engine.validate_active_catalog(&nexus.lane_catalog)?;
        }
        self.install_materialized_lane_manifests_for_catalog(
            &manifests,
            &nexus.lane_catalog,
            &nexus.governance,
        )?;
        self.install_lane_compliance_engine(compliance);
        self.prepare_restored_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)?;
        self.restore_kura_lane_segments_from_nexus()?;
        crate::smartcontracts::isi::sccp::params::validate_committed_staking_policy(
            &self.world_view(),
        )?;
        self.set_nexus_from_config(nexus)?;
        Ok(())
    }
}
