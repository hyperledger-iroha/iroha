/// Canonical, host-independent catalog publication effects certified with an autonomous batch.
/// The full protected parameter is also part of the World delta. This component binds the
/// block-owned lifecycle effect which publishes its derived runtime projections atomically.
#[derive(Encode)]
struct MergeRuntimeCatalogEffectsV1 {
    version: u8,
    runtime_catalog: iroha_data_model::nexus::NexusRuntimeCatalogV1,
    plan: iroha_data_model::nexus::LaneLifecyclePlan,
    transition_height: u64,
    expected_incarnation_root: Hash,
    previous_catalog_hash: Hash,
    updated_catalog_hash: Hash,
    previous_dataspaces_hash: Hash,
    updated_dataspaces_hash: Hash,
    previous_incarnations: BTreeMap<LaneId, Hash>,
    updated_incarnations: BTreeMap<LaneId, Hash>,
    previous_lineage: Vec<(LaneId, u64, Hash, u64)>,
    updated_lineage: Vec<(LaneId, u64, Hash, u64)>,
    previous_activation_heights: BTreeMap<LaneId, u64>,
    updated_activation_heights: BTreeMap<LaneId, u64>,
    lanes_to_reset: BTreeSet<LaneId>,
    replaced_lane_ids: BTreeSet<LaneId>,
    manifest_policy_hash: Hash,
}
impl MergeRuntimeCatalogEffectsV1 {
    fn from_pending(pending: &PendingAutoscaleLaneLifecycle) -> Option<Self> {
        let PendingAutoscaleLaneLifecycle {
            catalog_update,
            updated_lane_manifests,
            plan,
            transition: _, // The surface validator requires exactly Manual.
            transition_height,
            expected_incarnation_root,
            runtime_catalog,
        } = pending;
        let runtime_catalog = runtime_catalog.clone()?;
        let LaneLifecycleCatalogUpdate {
            previous_catalog,
            previous_dataspace_catalog,
            updated_dataspace_catalog,
            previous_routing_policy: _, // Revalidated against the authenticated base.
            previous_autoscale: _,      // Revalidated against the authenticated base.
            updated_catalog,
            previous_lane_config: _, // Exactly recomputed from the bound catalog.
            updated_lane_config: _,  // Exactly recomputed from the bound catalog.
            previous_lane_incarnations,
            updated_lane_incarnations,
            previous_lane_incarnation_lineage,
            updated_lane_incarnation_lineage,
            previous_lane_incarnation_activation_heights,
            updated_lane_incarnation_activation_heights,
            lanes_to_reset,
            replaced_lane_ids,
        } = catalog_update;
        let lineage = |entries: &BTreeMap<LaneId, LaneIncarnationLineage>| {
            entries
                .iter()
                .map(|(&lane, entry)| {
                    (
                        lane,
                        entry.generation,
                        entry.incarnation,
                        entry.activation_height,
                    )
                })
                .collect()
        };
        Some(Self {
            version: 1,
            runtime_catalog,
            plan: plan.clone(),
            transition_height: *transition_height,
            expected_incarnation_root: *expected_incarnation_root,
            previous_catalog_hash: iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
                previous_catalog,
            ),
            updated_catalog_hash: iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
                updated_catalog,
            ),
            previous_dataspaces_hash: iroha_data_model::nexus::dataspace_catalog_hash(
                previous_dataspace_catalog,
            ),
            updated_dataspaces_hash: iroha_data_model::nexus::dataspace_catalog_hash(
                updated_dataspace_catalog,
            ),
            previous_incarnations: previous_lane_incarnations.clone(),
            updated_incarnations: updated_lane_incarnations.clone(),
            previous_lineage: lineage(previous_lane_incarnation_lineage),
            updated_lineage: lineage(updated_lane_incarnation_lineage),
            previous_activation_heights: previous_lane_incarnation_activation_heights.clone(),
            updated_activation_heights: updated_lane_incarnation_activation_heights.clone(),
            lanes_to_reset: lanes_to_reset.clone(),
            replaced_lane_ids: replaced_lane_ids.clone(),
            manifest_policy_hash: Hash::prehashed(updated_lane_manifests.consensus_policy_digest()),
        })
    }
}
impl StateBlock<'_> {
    fn merge_execution_runtime_effects(&self) -> Option<MergeRuntimeCatalogEffectsV1> {
        self.pending_autoscale_lifecycle
            .as_ref()
            .and_then(MergeRuntimeCatalogEffectsV1::from_pending)
    }

    /// Every non-WSV projection must be the exact native derivation of the certified effect.
    /// No unrelated lifecycle, runtime ZK change, or host-local configuration is authorized here.
    fn validate_merge_runtime_catalog_effects(&self) -> Result<(), MergeLedgerCommitError> {
        let invalid =
            |reason: &str| MergeLedgerCommitError::ExecutionBatchInvalid(reason.to_owned());
        let lifecycle_error = |error: LaneLifecycleError| {
            MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                "autonomous runtime catalog effect is invalid: {error}"
            ))
        };
        self.state_ref
            .validate_runtime_catalog_block_overlay(
                &self.world,
                self.pending_autoscale_lifecycle.as_ref(),
                self._curr_block.height().get(),
            )
            .map_err(lifecycle_error)?;
        let state_nexus = self.state_ref.nexus_snapshot();
        let mut expected_nexus = state_nexus.clone();
        let state_manifests = self.state_ref.lane_manifests.read().clone();
        let expected_manifests = if let Some(pending) = &self.pending_autoscale_lifecycle {
            if pending.runtime_catalog.is_none()
                || pending.transition != PendingAutoscaleTransition::Manual
            {
                return Err(invalid(
                    "autonomous merge execution staged an unsupported lifecycle effect",
                ));
            }
            // Both initial pre-execution and certified replay execute under the exact stripped
            // application header. The carrier roots are added only after these effects exist.
            self.state_ref
                .validate_committed_autoscale_lane_lifecycle(
                    pending,
                    self._curr_block.height().get(),
                    crate::merge::merge_application_header_from_carrier(&self._curr_block).hash(),
                    None,
                )
                .map_err(lifecycle_error)?;
            let update = &pending.catalog_update;
            expected_nexus.lane_catalog = update.updated_catalog.clone();
            expected_nexus.lane_config = update.updated_lane_config.clone();
            expected_nexus.dataspace_catalog = update.updated_dataspace_catalog.clone();
            if self.lane_incarnations != update.updated_lane_incarnations
                || self.lane_incarnation_lineage != update.updated_lane_incarnation_lineage
                || self.lane_incarnation_activation_heights
                    != update.updated_lane_incarnation_activation_heights
            {
                return Err(invalid(
                    "autonomous runtime catalog incarnation projections differ from the bound effect",
                ));
            }
            &pending.updated_lane_manifests
        } else {
            if self.lane_incarnations != self.state_ref.lane_incarnations_snapshot()
                || self.lane_incarnation_lineage
                    != self.state_ref.lane_incarnation_lineage_snapshot()
                || self.lane_incarnation_activation_heights
                    != self
                        .state_ref
                        .lane_incarnation_activation_heights_snapshot()
            {
                return Err(invalid(
                    "autonomous merge execution changed unbound lane incarnations",
                ));
            }
            &state_manifests
        };
        // Use the maintained digest with the actual compliance binding; comparing two failed
        // digest Results would otherwise conceal a configuration difference.
        let digest = |nexus| {
            iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
                nexus,
                self.lane_compliance
                    .as_deref()
                    .map(LaneComplianceEngine::consensus_policy_digest),
                Some(expected_manifests.baseline_consensus_policy_digest()),
            )
            .map_err(|error| {
                invalid(&format!(
                    "autonomous runtime policy cannot be bound: {error}"
                ))
            })
        };
        if self.nexus.lane_catalog != expected_nexus.lane_catalog
            || self.nexus.dataspace_catalog != expected_nexus.dataspace_catalog
            || self.world.dataspace_catalog != expected_nexus.dataspace_catalog
            || !lane_config_entries_match(&self.nexus.lane_config, &expected_nexus.lane_config)
            || digest(&self.nexus)? != digest(&expected_nexus)?
            || self.lane_manifests.consensus_policy_digest()
                != expected_manifests.consensus_policy_digest()
            || self.lane_manifests.baseline_consensus_policy_digest()
                != expected_manifests.baseline_consensus_policy_digest()
            || compute_zk_consensus_policy_hash(&self.zk)
                != compute_zk_consensus_policy_hash(&self.state_ref.zk_snapshot())
        {
            return Err(invalid(
                "autonomous merge execution changed an unbound runtime configuration or catalog projection",
            ));
        }
        let expected_privacy = LanePrivacyRegistry::from_manifest_registry(expected_manifests);
        if self.lane_privacy_registry.as_ref() != &expected_privacy {
            return Err(invalid(
                "autonomous runtime catalog privacy projection differs from its manifest",
            ));
        }
        Ok(())
    }
}
