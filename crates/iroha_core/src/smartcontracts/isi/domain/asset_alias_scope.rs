fn alias_grace_until_ms(lease_expiry_ms: Option<u64>) -> Option<u64> {
    lease_expiry_ms.map(|expiry| expiry.saturating_add(ASSET_ALIAS_GRACE_MS))
}
fn validate_alias_for_asset_definition(
    alias: Option<&AssetDefinitionAlias>,
    definition: &AssetDefinition,
) -> Result<(), InstructionExecutionError> {
    validate_asset_alias_against_names(alias, [definition.name().as_str()]).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("invalid asset definition alias: {err}").into(),
        )
    })
}
fn dataspace_id_for_alias_segment(
    state_transaction: &StateTransaction<'_, '_>,
    dataspace_alias: &str,
) -> Option<DataSpaceId> {
    crate::sns::active_dataspace_id_by_alias(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        dataspace_alias,
        state_transaction.block_unix_timestamp_ms(),
    )
    .or_else(|| {
        if dataspace_alias.eq_ignore_ascii_case("universal") {
            Some(DataSpaceId::UNIVERSAL)
        } else {
            state_transaction
                .nexus
                .dataspace_catalog
                .by_alias(dataspace_alias)
                .map(|entry| entry.id)
        }
    })
}
fn asset_definition_home_dataspace(
    state_transaction: &StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Option<DataSpaceId> {
    definition
        .owning_domain()
        .as_ref()
        .map_or(Some(DataSpaceId::UNIVERSAL), |domain| {
            dataspace_id_for_alias_segment(state_transaction, domain.dataspace().as_ref())
        })
}
fn dataspace_is_public_or_universal(
    state_transaction: &StateTransaction<'_, '_>,
    dataspace_id: DataSpaceId,
) -> bool {
    dataspace_id == DataSpaceId::UNIVERSAL
        || state_transaction
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .any(|lane| {
                lane.dataspace_id == dataspace_id && lane.visibility == LaneVisibility::Public
            })
}
fn ensure_global_asset_definition_home_is_public_or_universal(
    state_transaction: &StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Result<(), InstructionExecutionError> {
    if definition.balance_scope_policy() != AssetBalancePolicy::Global {
        return Ok(());
    }
    let home_dataspace = asset_definition_home_dataspace(state_transaction, definition)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition {} owning domain has no active dataspace",
                    definition.id()
                )
                .into(),
            )
        })?;
    if !dataspace_is_public_or_universal(state_transaction, home_dataspace) {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "global asset definition {} cannot be registered in restricted dataspace {}; use DataspaceRestricted balance policy",
                definition.id(),
                home_dataspace.as_u64()
            )
            .into(),
        ));
    }
    Ok(())
}
fn ensure_global_asset_definition_registered_on_authoritative_route(
    state_transaction: &StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Result<(), InstructionExecutionError> {
    ensure_global_asset_definition_home_is_public_or_universal(state_transaction, definition)?;
    let home_dataspace = asset_definition_home_dataspace(state_transaction, definition)
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition {} owning domain has no active dataspace",
                    definition.id()
                )
                .into(),
            )
        })?;
    let route_dataspace = state_transaction
        .current_dataspace_id
        .or(state_transaction.world.current_dataspace_id);
    if let Some(route_dataspace) = route_dataspace
        && route_dataspace != home_dataspace
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "global asset definition {} must be registered on its authoritative dataspace {}; current route is {}",
                definition.id(),
                home_dataspace.as_u64(),
                route_dataspace.as_u64()
            )
            .into(),
        ));
    }
    Ok(())
}
