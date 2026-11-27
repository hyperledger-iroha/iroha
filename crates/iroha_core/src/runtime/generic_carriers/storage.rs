//! Definition-owned physical rows; a serialized key is never an access capability.

use super::*;
pub(crate) use crate::state::generic_carrier_storage::{PreparedWrite, prepare, stage_artifact};
use iroha_data_model::runtime::carrier::{CarrierStorageKeyV1, validated_carrier_key_v1};

/// Resolve a physical row's exact installed virtual owner before reading its payload.
/// Restrictions govern invocations, not whether retained canonical State can be restored.
pub(crate) fn row_schema<'a>(
    selection: &RootSelectionV1,
    published: &'a impl DefinitionLookup,
    key: &CarrierStorageKeyV1,
) -> Result<
    (
        &'a iroha_data_model::runtime::definition::CarrierSchemaV1,
        &'a iroha_data_model::runtime::definition::CarrierSchemaV1,
    ),
    Error,
> {
    row_schema_parts(selection, published, key.storage)
}

/// Resolve the same selected owner from a borrowed physical key without copying its bytes.
pub(crate) fn row_schema_parts<'a>(
    selection: &RootSelectionV1,
    published: &'a impl DefinitionLookup,
    storage_id: DefinitionId,
) -> Result<
    (
        &'a iroha_data_model::runtime::definition::CarrierSchemaV1,
        &'a iroha_data_model::runtime::definition::CarrierSchemaV1,
    ),
    Error,
> {
    require(
        selection
            .installed_storage
            .binary_search(&storage_id)
            .is_ok(),
        "physical row owner is not installed",
    )?;
    let Some(definition @ DefinitionDescriptorV1::Storage(storage)) =
        published.definition(&storage_id)
    else {
        return Err(Error::Contract(
            "physical row owner is not published storage",
        ));
    };
    require(
        iroha_data_model::runtime::carrier::carrier_definition_id_v1(definition)
            .map_err(Error::Codec)?
            == storage_id,
        "physical row owner identity differs from its descriptor",
    )?;
    let StorageShapeV1::Table {
        key: SchemaV1::Carrier(key_schema),
    } = &storage.shape
    else {
        return Err(Error::Contract("physical row owner has no carrier key"));
    };
    let SchemaV1::Carrier(value_schema) = &storage.value else {
        return Err(Error::Contract("physical row owner has no carrier value"));
    };
    require(
        storage.initial.is_none() && storage.projection == ProjectionV1::BareNorito,
        "physical row owner has a different initial value or projection",
    )?;
    Ok((key_schema, value_schema))
}

/// Validate canonical key/value bytes under the actual selected immutable definition.
pub(crate) fn validate_row(
    selection: &RootSelectionV1,
    published: &impl DefinitionLookup,
    key: &CarrierStorageKeyV1,
    frame: &[u8],
) -> Result<(), Error> {
    let (key_schema, value_schema) = row_schema(selection, published, key)?;
    validated_carrier_key_v1(key_schema, &key.key).map_err(Error::Codec)?;
    validated_carrier_payload_v1(value_schema, frame).map_err(Error::Codec)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::runtime::{
        carrier::{CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
        definition::{CarrierNodeV1, CarrierSchemaV1, StorageDescriptorV1},
    };
    use std::collections::BTreeMap;

    #[test]
    fn a_row_requires_its_exact_installed_owner_and_canonical_key_and_value() {
        let schema = CarrierSchemaV1 {
            nodes: vec![CarrierNodeV1::U16],
        };
        let definition = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: "test.carrier.rows".into(),
            shape: StorageShapeV1::Table {
                key: SchemaV1::Carrier(schema.clone()),
            },
            value: SchemaV1::Carrier(schema.clone()),
            initial: None,
            invariants: [5; 32],
            projection: ProjectionV1::BareNorito,
        });
        let id = definition.id().unwrap();
        let mut selection = crate::runtime::release_catalog().unwrap().baseline.clone();
        let mut published = BTreeMap::from([(id, definition.clone())]);
        let mut key = CarrierStorageKeyV1 {
            storage: id,
            key: encode_carrier_key_v1(&schema, &CarrierValueV1::U16(256)).unwrap(),
        };
        let value = encode_carrier_v1(&schema, &CarrierValueV1::U16(7)).unwrap();
        assert!(row_schema(&selection, &published, &key).is_err());
        selection.installed_storage.push(id);
        selection.installed_storage.sort();
        validate_row(&selection, &published, &key, &value).unwrap();
        key.key.push(0);
        assert!(validate_row(&selection, &published, &key, &value).is_err());
        key.key.pop();
        assert!(validate_row(&selection, &published, &key, &[0]).is_err());
        let mut substituted = definition;
        let DefinitionDescriptorV1::Storage(storage) = &mut substituted else {
            unreachable!()
        };
        storage.invariants[0] ^= 1;
        published.insert(id, substituted);
        assert!(row_schema(&selection, &published, &key).is_err());
    }
}
