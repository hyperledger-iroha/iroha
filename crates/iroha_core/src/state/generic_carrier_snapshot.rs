//! Validate both physical carrier generations without granting execution authority.

use super::*;

pub(super) fn validate_world(
    world: &World,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), StateRestoreError> {
    fn validate(
        cut: &impl WorldReadOnly,
        field: &str,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<(), StateRestoreError> {
        use crate::runtime::generic_carriers::{Error, storage};
        let invalid = |message: String| json::Error::InvalidField {
            field: field.to_owned(),
            message,
        };
        let catalog =
            crate::runtime::release_catalog().map_err(|error| invalid(error.to_string()))?;
        let profile = catalog
            .generic_carriers
            .ok_or_else(|| invalid("missing carrier profile owner".into()))?;
        let selection = cut.runtime_selection();
        let published = crate::runtime::change_set::PublishedDefinitions(cut.runtime_definitions());
        let installed = selection
            .installed_storage
            .binary_search(&profile.backing)
            .is_ok();
        if installed {
            if selection.kernel != profile.kernel
                || selection.commitment_profile != profile.commitment
                || cut.runtime_definitions().get(&profile.backing)
                    != catalog.definitions.get(&profile.backing)
            {
                return Err(invalid(
                    "carrier backing has no exact selected kernel and storage owner".into(),
                )
                .into());
            }
        } else if cut.runtime_generic_values().iter().next().is_some() {
            return Err(invalid("uninstalled carrier backing contains state".into()).into());
        }
        for (key, frame) in cut.runtime_generic_values().iter() {
            if let Err(error) = storage::validate_row(selection, &published, key, frame) {
                return Err(match error {
                    Error::Codec(original) => {
                        match crate::execution_attempt::canonical_decode_attempt_error(
                            original,
                            |error| invalid(error.to_string()),
                        ) {
                            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => {
                                error.into()
                            }
                            crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
                                StateRestoreError::ExecutionDeferred(original)
                            }
                        }
                    }
                    error => invalid(error.to_string()).into(),
                });
            }
        }
        for (id, bytes) in cut.runtime_generic_artifacts().iter() {
            crate::runtime::generic_carriers::artifact_staging::validate_stored(
                cut, id, bytes, budget,
            )
            .map_err(|error| match error {
                crate::execution_attempt::ExecutionAttemptError::Rejected(message) => {
                    StateRestoreError::Serialization(invalid(message))
                }
                crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
                    StateRestoreError::ExecutionDeferred(original)
                }
            })?;
        }
        Ok(())
    }
    validate(
        &world.view(),
        "world.runtime_generic_storage.blocks",
        budget,
    )?;
    let previous = world.try_block_and_revert(budget)?;
    validate(&previous, "world.runtime_generic_storage.revert", budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::runtime::{
        carrier::{CarrierStorageKeyV1, CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
        definition::{
            CarrierNodeV1, CarrierSchemaV1, DefinitionDescriptorV1, ProjectionV1, SchemaV1,
            StorageDescriptorV1, StorageShapeV1,
        },
    };

    fn fixture() -> (World, CarrierStorageKeyV1, Vec<u8>) {
        let world = World::default();
        let catalog = crate::runtime::release_catalog().unwrap();
        let profile = catalog.generic_carriers.unwrap();
        let schema = CarrierSchemaV1 {
            nodes: vec![CarrierNodeV1::U16],
        };
        let descriptor = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: "test.snapshot.carrier".into(),
            shape: StorageShapeV1::Table {
                key: SchemaV1::Carrier(schema.clone()),
            },
            value: SchemaV1::Carrier(schema.clone()),
            initial: None,
            invariants: [3; 32],
            projection: ProjectionV1::BareNorito,
        });
        let storage = descriptor.id().unwrap();
        let key = CarrierStorageKeyV1 {
            storage,
            key: encode_carrier_key_v1(&schema, &CarrierValueV1::U16(256)).unwrap(),
        };
        let value = encode_carrier_v1(&schema, &CarrierValueV1::U16(9)).unwrap();
        {
            let mut block = world.block();
            block.runtime_definitions.insert(storage, descriptor);
            for id in [profile.kernel, profile.backing, profile.commitment] {
                block
                    .runtime_definitions
                    .insert(id, catalog.definitions[&id].clone());
            }
            let selection = block.runtime_selection.get_mut();
            selection.kernel = profile.kernel;
            selection.commitment_profile = profile.commitment;
            selection
                .installed_storage
                .extend([profile.backing, storage]);
            selection.installed_storage.sort();
            block
                .runtime_generic_values
                .insert(key.clone(), value.clone());
            block.commit();
        }
        (world, key, value)
    }

    fn budget(world: &World) -> iroha_allocation::AllocationBudget {
        world.runtime_generic_values.allocation_budget().clone()
    }

    #[test]
    fn physical_rows_restore_only_with_exact_selected_virtual_owners() {
        let (world, key, value) = fixture();
        validate_world(&world, &budget(&world)).unwrap();
        let mut block = world.block();
        block
            .runtime_selection
            .get_mut()
            .installed_storage
            .retain(|id| *id != key.storage);
        block.commit();
        assert!(matches!(
            validate_world(&world, &budget(&world)),
            Err(StateRestoreError::Serialization(_))
        ));

        let baseline = World::default();
        let mut block = baseline.block();
        block.runtime_generic_values.insert(key, value);
        block.commit();
        assert!(matches!(
            validate_world(&baseline, &budget(&baseline)),
            Err(StateRestoreError::Serialization(_))
        ));
    }

    #[test]
    fn valid_current_rows_do_not_hide_a_malformed_original_undo_cut() {
        let (world, key, value) = fixture();
        let mut block = world.block();
        block.runtime_generic_values.insert(key.clone(), vec![0]);
        block.commit();
        assert!(validate_world(&world, &budget(&world)).is_err());
        let mut block = world.block();
        block.runtime_generic_values.insert(key, value);
        block.commit();
        let error = validate_world(&world, &budget(&world)).unwrap_err();
        assert!(
            matches!(error, StateRestoreError::Serialization(json::Error::InvalidField { ref field, .. }) if field.ends_with(".revert"))
        );
    }
}
