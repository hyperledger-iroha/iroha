//! Retained lifecycle projections, separate from State visibility and post work.
//!
//! Preparation uses the original accepted lifecycle under the enclosing carrier's
//! admission. These values grant no publication authority. Runtime index mutation
//! and cursor snapshot allocation still require their own complete admission.

use super::*;

/// Exact immutable manifest/privacy and reset projections, with no State borrow.
pub(super) struct PreparedLaneLifecycleEffects {
    manifests: LaneManifestRegistryHandle,
    privacy: LanePrivacyRegistryHandle,
    lanes_to_reset: BTreeSet<LaneId>,
    active_reset_lanes: BTreeSet<LaneId>,
    lane_config: iroha_config::parameters::actual::LaneConfig,
    transition: PendingAutoscaleTransition,
    transition_height: u64,
    #[cfg(feature = "telemetry")]
    nexus: iroha_config::parameters::actual::Nexus,
}

/// Original post-publication work, consumed after closing the State generation.
/// The enclosing commit/lifecycle fences must remain held through this work.
pub(super) struct LaneLifecyclePostPublication {
    lane_config: iroha_config::parameters::actual::LaneConfig,
    persist_cursor_journal: bool,
    transition: PendingAutoscaleTransition,
    transition_height: u64,
    #[cfg(feature = "telemetry")]
    nexus: iroha_config::parameters::actual::Nexus,
}

impl PreparedLaneLifecycleEffects {
    /// Capture once under the caller's existing admission, without reading State
    /// or creating an alternative capacity policy. The retained manifest owner
    /// remains the source of the privacy projection throughout publication.
    pub(super) fn prepare(
        pending: &PendingAutoscaleLaneLifecycle,
        nexus: &iroha_config::parameters::actual::Nexus,
    ) -> Self {
        #[cfg(not(feature = "telemetry"))]
        let _ = nexus;
        Self {
            manifests: Arc::clone(&pending.updated_lane_manifests),
            privacy: Arc::new(LanePrivacyRegistry::from_manifest_registry(
                &pending.updated_lane_manifests,
            )),
            lanes_to_reset: pending.catalog_update.lanes_to_reset.clone(),
            active_reset_lanes: State::active_reset_lanes(
                &pending.catalog_update.lanes_to_reset,
                &pending.catalog_update.updated_lane_config,
            ),
            lane_config: pending.catalog_update.updated_lane_config.clone(),
            transition: pending.transition.clone(),
            transition_height: pending.transition_height,
            #[cfg(feature = "telemetry")]
            nexus: nexus.clone(),
        }
    }

    /// Install retained projections inside the caller's original generation.
    /// Process reset precedes same-carrier DA metric updates. Cursor disk I/O,
    /// catalog telemetry and logging remain owned by the returned continuation;
    /// replay prevalidation suppresses process reset and cursor persistence.
    pub(super) fn publish(
        self,
        state: &State,
        publication: &StateViewGenerationWriteGuard<'_>,
        publish_process_runtime: bool,
    ) -> LaneLifecyclePostPublication {
        state.install_prepared_lane_manifests_in_publication(
            self.manifests,
            self.privacy,
            publication,
        );
        state.reset_lane_scoped_runtime_indexes(&self.lanes_to_reset);
        if publish_process_runtime {
            state.publish_lane_scoped_runtime_reset(&self.lanes_to_reset);
        }
        let records_reset = !self.active_reset_lanes.is_empty() && self.transition_height != 0;
        if records_reset {
            state
                .da_shard_cursors
                .write()
                .mark_lanes_canonically_reset(&self.active_reset_lanes, self.transition_height);
        }
        let persist_cursor_journal = publish_process_runtime
            && (records_reset
                || (!self.lanes_to_reset.is_empty() && state.da_indexes_hydrated.read().is_some()));
        LaneLifecyclePostPublication {
            lane_config: self.lane_config,
            persist_cursor_journal,
            transition: self.transition,
            transition_height: self.transition_height,
            #[cfg(feature = "telemetry")]
            nexus: self.nexus,
        }
    }
}

impl LaneLifecyclePostPublication {
    /// Finish under the original commit/lifecycle fences after generation close.
    /// Capture the final cursor index after all same-carrier DA effects, using
    /// this lifecycle's retained mapping rather than a newer live Nexus view.
    pub(super) fn publish(self, state: &State) {
        if self.persist_cursor_journal {
            state.persist_da_shard_cursor_journal_with_config(&self.lane_config);
        }
        // Preserve existing lifecycle telemetry and log behavior even for replay
        // prevalidation; only cursor/process publication was suppressed before.
        #[cfg(feature = "telemetry")]
        {
            state
                .telemetry
                .set_nexus_catalogs(&self.nexus.lane_catalog, &self.nexus.dataspace_catalog);
            if let Some(event) = state.telemetry.record_nexus_config_diff(&self.nexus) {
                match norito::json::to_string(&event) {
                    Ok(payload) => {
                        iroha_logger::telemetry!(msg = "nexus.config.diff", event = payload);
                    }
                    Err(err) => {
                        iroha_logger::error!(
                            ?err,
                            "failed to serialize autoscale nexus config diff event for telemetry log"
                        );
                    }
                }
            }
        }
        self.transition.log(self.transition_height);
    }
}

#[cfg(test)]
#[path = "carrier_lifecycle_effects_tests.rs"]
mod tests;
