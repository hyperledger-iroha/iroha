// Physical execution retirement is separate from immutable native storage history.

fn runtime_catalog_active_manifests(
    runtime: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
) -> Vec<iroha_data_model::nexus::RuntimeLaneManifestV1> {
    let retired: BTreeSet<_> = runtime
        .retired_lanes
        .iter()
        .map(|entry| entry.lane.id)
        .collect();
    runtime
        .manifests
        .iter()
        .filter(|entry| !retired.contains(&entry.lane_id))
        .cloned()
        .collect()
}

fn validate_runtime_retirement_delta(
    previous: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
    pending: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
    nexus: &iroha_config::parameters::actual::Nexus,
    plan: &iroha_data_model::nexus::LaneLifecyclePlan,
) -> Result<(), LaneLifecycleError> {
    let old_dataspaces = previous.map_or(&[][..], |catalog| catalog.retired_dataspaces.as_slice());
    let old_lanes = previous.map_or(&[][..], |catalog| catalog.retired_lanes.as_slice());
    if old_dataspaces
        .iter()
        .any(|entry| !pending.retired_dataspaces.contains(entry))
        || old_lanes
            .iter()
            .any(|entry| !pending.retired_lanes.contains(entry))
    {
        return Err(runtime_catalog_invalid(
            "physical retirement cannot replace or erase authenticated history",
        ));
    }
    let new_dataspaces: Vec<_> = pending
        .retired_dataspaces
        .iter()
        .filter(|entry| !old_dataspaces.contains(entry))
        .collect();
    let new_lanes: Vec<_> = pending
        .retired_lanes
        .iter()
        .filter(|entry| !old_lanes.contains(entry))
        .collect();
    if plan.retire.is_empty() {
        if !new_dataspaces.is_empty() || !new_lanes.is_empty() {
            return Err(runtime_catalog_invalid(
                "physical retirement history changed without a signed retirement plan",
            ));
        }
        return Ok(());
    }
    if !plan.additions.is_empty()
        || new_dataspaces.is_empty()
        || previous.map_or(&[][..], |catalog| catalog.dataspaces.as_slice()) != pending.dataspaces
        || previous.map_or(&[][..], |catalog| catalog.manifests.as_slice()) != pending.manifests
    {
        return Err(runtime_catalog_invalid(
            "physical retirement must preserve additions and precede a separate child registration",
        ));
    }
    let dataspace_ids: BTreeSet<_> = new_dataspaces
        .iter()
        .map(|entry| entry.retirement.dataspace_id)
        .collect();
    for entry in &new_dataspaces {
        let descriptor = nexus
            .dataspace_catalog
            .by_id(entry.retirement.dataspace_id)
            .ok_or_else(|| {
                runtime_catalog_invalid("retirement targets an inactive or unknown dataspace")
            })?;
        if descriptor.alias != entry.retirement.alias
            || descriptor.id == nexus.routing_policy.default_dataspace
        {
            return Err(runtime_catalog_invalid(
                "physical retirement descriptor differs from its current catalog or claims the default dataspace",
            ));
        }
    }
    let expected_lanes: BTreeSet<_> = nexus
        .lane_catalog
        .lanes()
        .iter()
        .filter(|lane| dataspace_ids.contains(&lane.dataspace_id))
        .map(|lane| lane.id)
        .collect();
    let selected_lanes: BTreeSet<_> = plan.retire.iter().copied().collect();
    let recorded_lanes: BTreeSet<_> = new_lanes.iter().map(|entry| entry.lane.id).collect();
    if expected_lanes.is_empty()
        || expected_lanes != selected_lanes
        || selected_lanes != recorded_lanes
        || selected_lanes.contains(&nexus.routing_policy.default_lane)
    {
        return Err(runtime_catalog_invalid(
            "physical retirement must remove exactly every active lane of its selected dataspaces",
        ));
    }
    for entry in &new_lanes {
        let original = nexus
            .lane_catalog
            .lanes()
            .iter()
            .find(|lane| lane.id == entry.lane.id);
        let dataspace = new_dataspaces
            .iter()
            .find(|dataspace| dataspace.retirement.dataspace_id == entry.lane.dataspace_id);
        if original != Some(&entry.lane)
            || dataspace
                .is_none_or(|dataspace| dataspace.retirement_height != entry.retirement_height)
        {
            return Err(runtime_catalog_invalid(
                "physical retirement history differs from exact original lane authority",
            ));
        }
    }
    Ok(())
}

fn ensure_retired_native_policy_absent(
    world: &impl WorldReadOnly,
    dataspaces: &BTreeSet<iroha_model_base::topology::DataSpaceId>,
    lanes: &BTreeSet<LaneId>,
) -> Result<(), LaneLifecycleError> {
    use iroha_data_model::sumeragi_lanes::SumeragiLanePolicy;
    let Some(custom) = world
        .parameters()
        .custom()
        .get(&SumeragiLanePolicy::parameter_id())
    else {
        return Ok(());
    };
    let policy = SumeragiLanePolicy::from_custom_parameter(custom)
        .ok_or_else(|| runtime_catalog_invalid("native lane policy identity mismatch"))?
        .map_err(runtime_catalog_invalid)?;
    if policy
        .fixed
        .iter()
        .any(|lane| dataspaces.contains(&lane.dataspace) || lanes.contains(&lane.lane))
        || policy
            .routes
            .iter()
            .any(|route| lanes.contains(&route.lane))
        || lanes.iter().any(|lane| policy.is_elastic(*lane))
        || policy
            .autoscale
            .as_ref()
            .is_some_and(|autoscale| dataspaces.contains(&autoscale.dataspace))
    {
        return Err(runtime_catalog_invalid(
            "retired physical execution is still reserved by native fixed, routing or autoscale policy",
        ));
    }
    Ok(())
}

fn validate_physical_retirement_accepted_world(
    previous: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
    pending: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
    original_nexus: &iroha_config::parameters::actual::Nexus,
    world: &impl WorldReadOnly,
    update: &LaneLifecycleCatalogUpdate,
    height: u64,
    now_ms: u64,
) -> Result<(), LaneLifecycleError> {
    let previous_dataspaces =
        previous.map_or(&[][..], |catalog| catalog.retired_dataspaces.as_slice());
    let previous_lanes = previous.map_or(&[][..], |catalog| catalog.retired_lanes.as_slice());
    let targets: BTreeSet<_> = pending
        .retired_dataspaces
        .iter()
        .filter(|entry| !previous_dataspaces.contains(entry))
        .map(|entry| entry.retirement.dataspace_id)
        .collect();
    let lanes: BTreeSet<_> = pending
        .retired_lanes
        .iter()
        .filter(|entry| !previous_lanes.contains(entry))
        .map(|entry| entry.lane.id)
        .collect();
    if targets.is_empty() {
        return Ok(());
    }
    for record in pending
        .retired_dataspaces
        .iter()
        .filter(|entry| !previous_dataspaces.contains(entry))
    {
        if record.retirement_height != height {
            return Err(runtime_catalog_invalid(
                "physical retirement height differs from its accepted block",
            ));
        }
        ensure_physical_retirement_owner(
            world,
            &original_nexus.dataspace_catalog,
            &record.retirement,
            &record.retirement.owner,
            now_ms,
        )?;
    }
    for record in pending
        .retired_lanes
        .iter()
        .filter(|entry| !previous_lanes.contains(entry))
    {
        if record.retirement_height != height
            || update.previous_lane_incarnations.get(&record.lane.id) != Some(&record.incarnation)
            || update
                .previous_lane_incarnation_activation_heights
                .get(&record.lane.id)
                != Some(&record.activation_height)
        {
            return Err(runtime_catalog_invalid(
                "retained storage binding differs from its authenticated retirement predecessor",
            ));
        }
    }
    ensure_physical_retirement_closed(world, &targets, &lanes, height)?;
    ensure_retired_native_policy_absent(world, &targets, &lanes)?;
    physical_dataspace_retirement_safety::ensure_physical_dataspace_retirement_safe(
        &targets,
        &lanes,
        world,
        original_nexus,
        height,
    )
    .map_err(runtime_catalog_invalid)
}

fn runtime_catalog_project_retired_routes(
    nexus: &mut iroha_config::parameters::actual::Nexus,
    runtime: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
) -> Result<(), LaneLifecycleError> {
    let Some(runtime) = runtime else {
        return Ok(());
    };
    let dataspaces: BTreeSet<_> = runtime
        .retired_dataspaces
        .iter()
        .map(|entry| entry.retirement.dataspace_id)
        .collect();
    let lanes: BTreeSet<_> = runtime
        .retired_lanes
        .iter()
        .map(|entry| entry.lane.id)
        .collect();
    if dataspaces.contains(&nexus.routing_policy.default_dataspace)
        || lanes.contains(&nexus.routing_policy.default_lane)
    {
        return Err(runtime_catalog_invalid(
            "retirement cannot remove default execution authority",
        ));
    }
    nexus.routing_policy.rules.retain(|rule| {
        !lanes.contains(&rule.lane) && rule.dataspace.is_none_or(|id| !dataspaces.contains(&id))
    });
    nexus
        .dataspace_fee_sponsor_program_ids
        .retain(|id, _| !dataspaces.contains(id));
    Ok(())
}

fn ensure_physical_retirement_closed(
    world: &impl WorldReadOnly,
    dataspaces: &BTreeSet<iroha_model_base::topology::DataSpaceId>,
    lanes: &BTreeSet<LaneId>,
    block_height: u64,
) -> Result<(), LaneLifecycleError> {
    use iroha_data_model::block::consensus::SumeragiRootScope;
    if crate::sumeragi::lanes::routing::committed_root_scope(world)
        != Some(SumeragiRootScope::Global)
    {
        return Err(runtime_catalog_invalid(
            "physical retirement requires the committed global root",
        ));
    }
    if world
        .sumeragi_lanes()
        .lanes
        .iter()
        .any(|record| dataspaces.contains(&record.dataspace) || lanes.contains(&record.lane))
    {
        return Err(runtime_catalog_invalid(
            "physical retirement requires native lane closure",
        ));
    }
    for custody in &world.sumeragi_lanes().custody {
        if lanes.contains(&custody.lane) {
            custody.validate().map_err(runtime_catalog_invalid)?;
            if custody.retired_at.is_none()
                || custody
                    .release_height()
                    .map_err(runtime_catalog_invalid)?
                    .is_none_or(|height| height > block_height)
            {
                return Err(runtime_catalog_invalid(
                    "physical retirement requires released original native signer custody",
                ));
            }
        }
    }
    Ok(())
}

fn ensure_physical_retirement_owner(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    request: &iroha_data_model::nexus::RuntimeDataSpaceRetirementV1,
    authority: &AccountId,
    now_ms: u64,
) -> Result<(), LaneLifecycleError> {
    let descriptor = catalog.by_id(request.dataspace_id).ok_or_else(|| {
        runtime_catalog_invalid("retirement targets an unknown physical dataspace")
    })?;
    let selector = crate::sns::selector_for_dataspace_alias(&request.alias)
        .map_err(runtime_catalog_invalid)?;
    if descriptor.alias != request.alias || selector.normalized_label() != request.alias {
        return Err(runtime_catalog_invalid(
            "retirement alias differs from its physical catalog and paid SNS identity",
        ));
    }
    let owner =
        crate::sns::active_dataspace_owner_and_generation_by_alias(world, &request.alias, now_ms)
            .map_err(runtime_catalog_invalid)?;
    if owner != Some((request.owner.clone(), request.expected_ownership_generation))
        || authority != &request.owner
    {
        return Err(runtime_catalog_invalid(
            "retirement requires the exact current SNS owner and ownership generation",
        ));
    }
    // The physical ID is independently selected by its catalog commitment. SNS authenticates
    // the paid canonical label and its owner, not a rewrite of historical physical identity.
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoricalLaneGeometry {
    config: iroha_config::parameters::actual::LaneConfig,
    incarnations: BTreeMap<LaneId, Hash>,
    activation_heights: BTreeMap<LaneId, u64>,
}

fn historical_lane_geometry(
    active: &LaneCatalog,
    incarnations: &BTreeMap<LaneId, Hash>,
    activation_heights: &BTreeMap<LaneId, u64>,
    runtime: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
) -> Result<HistoricalLaneGeometry, LaneLifecycleError> {
    let mut lanes = active.lanes().to_vec();
    let mut incarnations = incarnations.clone();
    let mut activation_heights = activation_heights.clone();
    if let Some(runtime) = runtime {
        runtime
            .validate_structure()
            .map_err(runtime_catalog_invalid)?;
        for record in &runtime.retired_lanes {
            if lanes.iter().any(|lane| lane.id == record.lane.id)
                || incarnations
                    .insert(record.lane.id, record.incarnation)
                    .is_some()
                || activation_heights
                    .insert(record.lane.id, record.activation_height)
                    .is_some()
            {
                return Err(runtime_catalog_invalid(
                    "retired storage identity conflicts with an active lane",
                ));
            }
            lanes.push(record.lane.clone());
        }
    }
    lanes.sort_by_key(|lane| lane.id);
    let bound = lanes
        .last()
        .and_then(|lane| lane.id.as_u32().checked_add(1))
        .and_then(std::num::NonZeroU32::new)
        .ok_or_else(|| runtime_catalog_invalid("historical lane bound overflows"))?;
    let catalog = LaneCatalog::new(bound, lanes)?;
    Ok(HistoricalLaneGeometry {
        config: iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog),
        incarnations,
        activation_heights,
    })
}

pub(crate) fn retained_lane_geometry_from_parameters(
    active: &LaneCatalog,
    incarnations: &BTreeMap<LaneId, Hash>,
    activation_heights: &BTreeMap<LaneId, u64>,
    lineage: &BTreeMap<LaneId, LaneIncarnationLineage>,
    parameters: &Parameters,
    height: u64,
) -> Result<
    (
        iroha_config::parameters::actual::LaneConfig,
        BTreeMap<LaneId, Hash>,
        BTreeMap<LaneId, u64>,
    ),
    LaneLifecycleError,
> {
    let runtime = runtime_catalog_from_parameters(parameters)?;
    if let Some(runtime) = &runtime {
        for record in &runtime.retired_lanes {
            if record.retirement_height > height
                || lineage.get(&record.lane.id).is_none_or(|entry| {
                    entry.incarnation != record.incarnation
                        || entry.activation_height != record.activation_height
                })
            {
                return Err(runtime_catalog_invalid(
                    "retired lane storage differs from its retained lineage or snapshot height",
                ));
            }
        }
    }
    let geometry =
        historical_lane_geometry(active, incarnations, activation_heights, runtime.as_ref())?;
    Ok((
        geometry.config,
        geometry.incarnations,
        geometry.activation_heights,
    ))
}

fn bind_runtime_catalog_update(
    update: &mut LaneLifecycleCatalogUpdate,
    original: &iroha_config::parameters::actual::Nexus,
    prospective: &iroha_config::parameters::actual::Nexus,
    previous: Option<&iroha_data_model::nexus::NexusRuntimeCatalogV1>,
    pending: &iroha_data_model::nexus::NexusRuntimeCatalogV1,
) -> Result<(), LaneLifecycleError> {
    update.previous_dataspace_catalog = original.dataspace_catalog.clone();
    update.updated_dataspace_catalog = prospective.dataspace_catalog.clone();
    update.previous_routing_policy = original.routing_policy.clone();
    update.updated_routing_policy = prospective.routing_policy.clone();
    update.previous_autoscale = original.autoscale;
    update.updated_autoscale = prospective.autoscale;
    update.previous_storage_geometry = historical_lane_geometry(
        &update.previous_catalog,
        &update.previous_lane_incarnations,
        &update.previous_lane_incarnation_activation_heights,
        previous,
    )?;
    update.updated_storage_geometry = historical_lane_geometry(
        &update.updated_catalog,
        &update.updated_lane_incarnations,
        &update.updated_lane_incarnation_activation_heights,
        Some(pending),
    )?;
    // Historical ledgers, cursors, completed settlements and custody originals survive retirement.
    for record in &pending.retired_lanes {
        update.lanes_to_reset.remove(&record.lane.id);
    }
    Ok(())
}
