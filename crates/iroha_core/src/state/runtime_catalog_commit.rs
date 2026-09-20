impl State {
    /// Bind the final accepted World to the prepared catalog and activation committee.
    fn validate_runtime_catalog_block_overlay(
        &self,
        accepted_world: &impl WorldReadOnly,
        pending: Option<&PendingAutoscaleLaneLifecycle>,
        block_height: u64,
    ) -> Result<(), LaneLifecycleError> {
        use iroha_data_model::nexus::NexusRuntimeCatalogV1;
        let id = NexusRuntimeCatalogV1::parameter_id();
        let old_world = self.world.view();
        let _old_runtime = runtime_catalog_from_world(&old_world)?;
        let accepted_runtime = runtime_catalog_from_world(accepted_world)?;
        let old_parameter = old_world.parameters().custom().get(&id);
        let accepted_parameter = accepted_world.parameters().custom().get(&id);
        let Some(pending) = pending.filter(|pending| pending.runtime_catalog.is_some()) else {
            if old_parameter != accepted_parameter {
                return Err(runtime_catalog_invalid(
                    "protected runtime catalog changed without a staged catalog transition",
                ));
            }
            return Ok(());
        };
        if block_height <= 1 || pending.transition != PendingAutoscaleTransition::Manual {
            return Err(runtime_catalog_invalid(
                "runtime catalog authority requires a signed post-genesis manual transition",
            ));
        }
        if accepted_runtime.as_ref() != pending.runtime_catalog.as_ref() {
            return Err(runtime_catalog_invalid(
                "accepted World catalog differs from the prepared cumulative catalog",
            ));
        }
        let authority_height = block_height.checked_add(1).ok_or_else(|| {
            runtime_catalog_invalid("runtime catalog activation height overflows")
        })?;
        let mut nexus = self.nexus.read().clone();
        nexus.dataspace_catalog = pending.catalog_update.updated_dataspace_catalog.clone();
        nexus.lane_catalog = pending.catalog_update.updated_catalog.clone();
        nexus.lane_config = pending.catalog_update.updated_lane_config.clone();
        for lane in &pending.plan.additions {
            validate_runtime_catalog_committee(
                accepted_world,
                &self.network_id,
                &nexus,
                &pending.updated_lane_manifests,
                lane,
                authority_height,
            )?;
        }
        Ok(())
    }
}

/// The committed overlay authorizes additive lanes; ordinary lifecycle cannot replace or retire
/// that authority while retaining its signed manifest in the protected catalog.
fn ensure_runtime_catalog_lanes_preserved(
    world: &impl WorldReadOnly,
    previous: &LaneCatalog,
    updated: &LaneCatalog,
) -> Result<(), LaneLifecycleError> {
    let Some(runtime) = runtime_catalog_from_world(world)? else {
        return Ok(());
    };
    for manifest in &runtime.manifests {
        let old = previous
            .lanes()
            .iter()
            .find(|lane| lane.id == manifest.lane_id);
        let new = updated
            .lanes()
            .iter()
            .find(|lane| lane.id == manifest.lane_id);
        if old.is_none() || old != new {
            return Err(runtime_catalog_invalid(
                "ordinary lifecycle cannot replace or retire a committed runtime lane",
            ));
        }
    }
    Ok(())
}
