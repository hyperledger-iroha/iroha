fn dataspace_id_for_alias_segment(
    catalog: &DataSpaceCatalog,
    dataspace_alias: &str,
) -> Option<DataSpaceId> {
    if dataspace_alias.eq_ignore_ascii_case("universal") {
        return Some(DataSpaceId::UNIVERSAL);
    }
    catalog.by_alias(dataspace_alias).map(|entry| entry.id)
}
fn asset_definition_home_dataspace_id(
    state_transaction: &StateTransaction<'_, '_>,
    definition: &AssetDefinition,
) -> Result<Option<DataSpaceId>, Error> {
    if let Some(dataspace) = state_transaction
        .world
        .asset_definition_dataspace(definition.id())
        .map_err(|error| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid asset-definition home: {error}").into(),
            )
        })?
    {
        return Ok(Some(dataspace));
    }
    let dataspace_alias = state_transaction
        .world
        .asset_definition_domains
        .get(definition.id())
        .map(|domain| domain.dataspace().as_ref().to_owned())
        .or_else(|| {
            definition
                .owning_domain()
                .as_ref()
                .map(|domain| domain.dataspace().as_ref().to_owned())
        });
    Ok(match dataspace_alias {
        Some(alias) => {
            dataspace_id_for_alias_segment(&state_transaction.nexus.dataspace_catalog, &alias)
        }
        None if definition.balance_scope_policy() == AssetBalancePolicy::Global => {
            Some(DataSpaceId::UNIVERSAL)
        }
        None => None,
    })
}
fn coherent_execution_dataspace(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<DataSpaceId>, Error> {
    if state_transaction.current_dataspace_id != state_transaction.world.current_dataspace_id {
        return Err(InstructionExecutionError::InvariantViolation(
            "transaction and world execution dataspaces are inconsistent".into(),
        ));
    }
    Ok(state_transaction.current_dataspace_id)
}
/// Validate a proof- or governance-committed transparent balance partition.
///
/// This path never consults account bindings or mutable asset aliases. A
/// restricted definition must name one exact non-universal dataspace, and
/// a non-universal execution route must be that same dataspace.
pub(crate) fn validate_committed_public_balance_scope(
    state_transaction: &StateTransaction<'_, '_>,
    definition_id: &AssetDefinitionId,
    scope: AssetBalanceScope,
    operation: &str,
) -> Result<(), Error> {
    let definition = state_transaction
        .world
        .asset_definition(definition_id)
        .map_err(Error::from)?;
    let execution_dataspace = coherent_execution_dataspace(state_transaction)?;
    match (definition.balance_scope_policy(), scope) {
        (AssetBalancePolicy::Global, AssetBalanceScope::Global) => {
            if let Some(route) = execution_dataspace
                && route != DataSpaceId::UNIVERSAL
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "global public balance scope {operation} must execute on the universal coordinator; current route is {}",
                        route.as_u64(),
                    )
                    .into(),
                ));
            }
        }
        (AssetBalancePolicy::DataspaceRestricted, AssetBalanceScope::Dataspace(dataspace)) => {
            if dataspace == DataSpaceId::UNIVERSAL {
                return Err(InstructionExecutionError::InvariantViolation(
                    "the universal coordinator is not a restricted public balance scope".into(),
                ));
            }
            if let Some(route) = execution_dataspace
                && route != DataSpaceId::UNIVERSAL
                && route != dataspace
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "committed public balance scope {} does not match execution dataspace {}",
                        dataspace.as_u64(),
                        route.as_u64(),
                    )
                    .into(),
                ));
            }
        }
        (AssetBalancePolicy::Global, AssetBalanceScope::Dataspace(_)) => {
            return Err(InstructionExecutionError::InvariantViolation(
                "global asset definition requires the global public balance scope".into(),
            ));
        }
        (AssetBalancePolicy::DataspaceRestricted, AssetBalanceScope::Global) => {
            return Err(InstructionExecutionError::InvariantViolation(
                "dataspace-restricted asset definition requires an exact public balance scope"
                    .into(),
            ));
        }
    }
    Ok(())
}
fn bare_restricted_asset_home_dataspace_hint(
    state_transaction: &StateTransaction<'_, '_>,
    asset_id: &AssetId,
) -> Result<Option<DataSpaceId>, Error> {
    if !matches!(
        asset_id.scope(),
        iroha_data_model::asset::AssetBalanceScope::Global
    ) {
        return Ok(None);
    }
    let definition = state_transaction
        .world
        .asset_definition(asset_id.definition())
        .map_err(Error::from)?;
    if definition.balance_scope_policy() != AssetBalancePolicy::DataspaceRestricted {
        return Ok(None);
    }
    Ok(
        asset_definition_home_dataspace_id(state_transaction, &definition)?
            .filter(|dataspace| *dataspace != DataSpaceId::UNIVERSAL),
    )
}
fn ensure_global_asset_write_on_authoritative_route(
    state_transaction: &StateTransaction<'_, '_>,
    definition_id: &AssetDefinitionId,
    operation: &str,
) -> Result<(), Error> {
    let definition = state_transaction
        .world
        .asset_definition(definition_id)
        .map_err(Error::from)?;
    if definition.balance_scope_policy() != AssetBalancePolicy::Global {
        return Ok(());
    }
    let home_dataspace = asset_definition_home_dataspace_id(state_transaction, &definition)?
        .unwrap_or(DataSpaceId::UNIVERSAL);
    let route_dataspace = state_transaction
        .current_dataspace_id
        .or(state_transaction.world.current_dataspace_id);
    if let Some(route_dataspace) = route_dataspace
        && route_dataspace != home_dataspace
        && route_dataspace != DataSpaceId::UNIVERSAL
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "global asset {definition_id} {operation} must execute on authoritative dataspace {} or the universal AMX coordinator; current route is {}",
                home_dataspace.as_u64(),
                route_dataspace.as_u64()
            )
            .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod direct_dataspace_home_tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_data_model::block::BlockHeader;
    use iroha_model_base::domain::DomainId;
    use iroha_test_samples::ALICE_ID;

    #[test]
    fn direct_home_resolves_bare_restricted_balance_without_rewriting_explicit_bucket() {
        let home = DataSpaceId::new(10);
        let foreign_bucket = DataSpaceId::new(12);
        let domain = DomainId::try_new("cash", "universal").expect("id seed");
        let id = AssetDefinitionId::derive_from_components(
            domain.clone(),
            "kina".parse().expect("name"),
        );
        let definition = AssetDefinition::numeric(
            id.clone(),
            "Kina",
            AssetBalancePolicy::DataspaceRestricted,
            None,
        )
        .build(&ALICE_ID);
        let mut world = World::default();
        world
            .insert_direct_asset_definition_with_assets_for_testing(definition.clone(), home, [])
            .expect("direct home fixture");
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(BlockHeader::new(
            std::num::NonZeroU64::new(2).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let transaction = block.transaction();
        let bare = AssetId::of(id.clone(), ALICE_ID.clone());
        assert_eq!(
            asset_definition_home_dataspace_id(&transaction, &definition).unwrap(),
            Some(home)
        );
        assert_eq!(
            bare_restricted_asset_home_dataspace_hint(&transaction, &bare).unwrap(),
            Some(home)
        );
        assert!(
            validate_committed_public_balance_scope(
                &transaction,
                &id,
                AssetBalanceScope::Dataspace(foreign_bucket),
                "bilateral settlement",
            )
            .is_ok(),
            "definition namespace and explicit settlement bucket are independent"
        );
    }
}
