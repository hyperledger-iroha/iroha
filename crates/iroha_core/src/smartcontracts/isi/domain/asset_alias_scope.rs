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
    state_transaction: &mut StateTransaction<'_, '_>,
    dataspace_alias: &str,
) -> Result<Option<DataSpaceId>, InstructionExecutionError> {
    match crate::sns::resolve_active_dataspace_id_by_alias(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        dataspace_alias,
        state_transaction.block_unix_timestamp_ms(),
    ) {
        Ok(id) => Ok(Some(id)),
        Err(crate::sns::SnsError::NotFound(_)) => Ok(None),
        Err(error) => Err(error.retain_in_instruction(state_transaction)),
    }
}
fn asset_definition_home_dataspace(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Result<Option<DataSpaceId>, InstructionExecutionError> {
    if let Some(dataspace_id) = state_transaction
        .world
        .asset_definition_dataspace(definition.id())
        .map_err(|error| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid authoritative asset definition home: {error}").into(),
            )
        })?
    {
        if definition.owning_domain().is_some() {
            return Err(InstructionExecutionError::InvariantViolation(
                "asset definition has conflicting domain and dataspace homes".into(),
            ));
        }
        return Ok(Some(dataspace_id));
    }
    match definition.owning_domain() {
        Some(domain) => {
            dataspace_id_for_alias_segment(state_transaction, domain.dataspace().as_ref())
        }
        None => Ok(Some(DataSpaceId::UNIVERSAL)),
    }
}
/// Exact Phase A refusal for a restricted home, shared by the direct and domain paths.
///
/// Keep this text stable: the post-reset replay qualification scans committed rejections for it.
const RESTRICTED_HOME_REFUSAL: &str =
    "direct-dataspace registration currently requires a public home dataspace";
/// Refuse a home that a definition with this balance policy may not use.
///
/// A global definition can never be homed in a restricted dataspace: every global balance
/// operation executes on the universal coordinator, so its payloads are always public. A
/// dataspace-restricted definition is refused a restricted home until restricted-home read
/// gates, route-scoped reads and the restricted-home write postcondition are in force.
// TODO: admit DataspaceRestricted definitions in restricted homes together with those rules.
fn ensure_home_admissible(
    state_transaction: &StateTransaction<'_, '_>,
    definition_id: &AssetDefinitionId,
    balance_scope_policy: AssetBalancePolicy,
    home_dataspace: DataSpaceId,
) -> Result<(), InstructionExecutionError> {
    if !crate::read_scope::dataspace_is_restricted(
        &state_transaction.nexus.lane_catalog,
        home_dataspace,
    ) {
        return Ok(());
    }
    Err(InstructionExecutionError::InvariantViolation(
        match balance_scope_policy {
            AssetBalancePolicy::Global => format!(
                "global asset definition {definition_id} cannot be registered in restricted dataspace {}; use DataspaceRestricted balance policy",
                home_dataspace.as_u64()
            ),
            AssetBalancePolicy::DataspaceRestricted => RESTRICTED_HOME_REFUSAL.to_owned(),
        }
        .into(),
    ))
}
/// Keep an alias inside its definition's namespace whenever either side is restricted.
///
/// A public definition may carry an alias in another public namespace, but an alias that names
/// a restricted dataspace, or a definition homed in one, must name exactly the home.
fn ensure_alias_namespace_matches_home(
    state_transaction: &StateTransaction<'_, '_>,
    alias: &AssetDefinitionAlias,
    alias_dataspace: DataSpaceId,
    home_dataspace: DataSpaceId,
) -> Result<(), InstructionExecutionError> {
    let catalog = &state_transaction.nexus.lane_catalog;
    if alias_dataspace != home_dataspace
        && (crate::read_scope::dataspace_is_restricted(catalog, alias_dataspace)
            || crate::read_scope::dataspace_is_restricted(catalog, home_dataspace))
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "asset definition alias `{alias}` must use home dataspace {}: a restricted dataspace namespace cannot alias another dataspace",
                home_dataspace.as_u64()
            )
            .into(),
        ));
    }
    Ok(())
}
fn ensure_asset_definition_registered_on_authoritative_route(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Result<(), InstructionExecutionError> {
    let home_dataspace = asset_definition_home_dataspace(state_transaction, definition)?
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition {} owning domain has no active dataspace",
                    definition.id()
                )
                .into(),
            )
        })?;
    ensure_home_admissible(
        state_transaction,
        definition.id(),
        definition.balance_scope_policy(),
        home_dataspace,
    )?;
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
