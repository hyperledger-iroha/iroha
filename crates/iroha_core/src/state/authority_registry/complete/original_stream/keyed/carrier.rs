//! Original physical rows projected under their exact selected virtual carrier owners.
//!
//! This path belongs only to the new commitment profile. Its complete build sorts borrowed
//! row positions with original-pool scratch; physical wrapper rows never become keyed leaves.

use super::*;
use crate::state::{StorageReadOnly, WorldReadOnly};
use iroha_data_model::runtime::{
    carrier::{
        borrow_carrier_storage_key_v1, borrow_carrier_storage_value_v1, keyed::STORAGE_OWNER,
        validated_carrier_key_v1, validated_carrier_payload_v1,
    },
    definition::DefinitionDescriptorV1,
};
use std::alloc::Layout;

struct BorrowedRow<'a> {
    table: &'a str,
    key: &'a [u8],
    value: Option<&'a [u8]>,
}

#[cfg(test)]
mod tests;

struct FundedKey {
    key: StateKeyV1,
    _reservation: iroha_allocation::AllocationReservation,
}

impl BorrowedRow<'_> {
    fn owned_key(&self, budget: &AllocationBudget) -> Result<FundedKey, KeyedCaptureError> {
        let table_layout = Layout::array::<u8>(self.table.len()).map_err(|_| {
            KeyedCaptureError::SourceMismatch("unrepresentable carrier table".into())
        })?;
        let key_layout = Layout::array::<u8>(self.key.len())
            .map_err(|_| KeyedCaptureError::SourceMismatch("unrepresentable carrier key".into()))?;
        let reservation = budget
            .try_reserve_layouts([table_layout, key_layout])
            .map_err(|original| KeyedCaptureError::Deferred(original.into()))?;
        let mut table = String::new();
        table
            .try_reserve_exact(self.table.len())
            .map_err(|original| {
                KeyedCaptureError::Deferred(
                    iroha_allocation::AllocatorRefusal::from_vec_reserve(original, table_layout)
                        .into(),
                )
            })?;
        table.push_str(self.table);
        let mut key = Vec::new();
        key.try_reserve_exact(self.key.len()).map_err(|original| {
            KeyedCaptureError::Deferred(
                iroha_allocation::AllocatorRefusal::from_vec_reserve(original, key_layout).into(),
            )
        })?;
        key.extend_from_slice(self.key);
        Ok(FundedKey {
            key: StateKeyV1 { table, key },
            _reservation: reservation,
        })
    }
}

fn project<'a>(
    physical: &'a Row,
    world: &'a impl WorldReadOnly,
) -> Result<BorrowedRow<'a>, KeyedCaptureError> {
    let (storage, key) =
        borrow_carrier_storage_key_v1(physical.key.bytes()).map_err(KeyedCaptureError::Codec)?;
    let published = crate::runtime::change_set::PublishedDefinitions(world.runtime_definitions());
    let (key_schema, value_schema) = crate::runtime::generic_carriers::storage::row_schema_parts(
        world.runtime_selection(),
        &published,
        storage,
    )
    .map_err(|error| match error {
        crate::runtime::generic_carriers::Error::Codec(original) => {
            KeyedCaptureError::Codec(original)
        }
        other => KeyedCaptureError::SourceMismatch(other.to_string()),
    })?;
    let Some(DefinitionDescriptorV1::Storage(descriptor)) =
        world.runtime_definitions().get(&storage)
    else {
        return Err(KeyedCaptureError::SourceMismatch(STORAGE_OWNER.into()));
    };
    validated_carrier_key_v1(key_schema, key).map_err(KeyedCaptureError::Codec)?;
    let value = physical
        .value
        .as_ref()
        .map(|value| {
            let frame =
                borrow_carrier_storage_value_v1(value.bytes()).map_err(KeyedCaptureError::Codec)?;
            validated_carrier_payload_v1(value_schema, frame).map_err(KeyedCaptureError::Codec)
        })
        .transpose()?;
    Ok(BorrowedRow {
        table: &descriptor.namespace,
        key,
        value,
    })
}

impl Collector {
    pub(super) fn build_carrier(
        &self,
        schema: &SelectedStateSchema,
        storage_budget: &AllocationBudget,
        world: &impl WorldReadOnly,
    ) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
        let count = self
            .fields
            .as_slice()
            .iter()
            .try_fold(0_usize, |count, field| {
                count.checked_add(field.current.as_slice().len())
            })
            .ok_or(CaptureError::CountLimit)?;
        let mut rows =
            ChargedBuffer::new(count, self.budget()).map_err(CaptureError::Allocation)?;
        let mut installed = 0;
        let mut saw_backing = false;
        for field in self.fields.as_slice() {
            if field.id == STORAGE_OWNER {
                if saw_backing {
                    return Err(KeyedCaptureError::SourceMismatch(STORAGE_OWNER.into()));
                }
                saw_backing = true;
                installed += schema
                    .carrier_tables
                    .ok_or_else(|| KeyedCaptureError::SourceMismatch(STORAGE_OWNER.into()))?;
                for row in field.current.as_slice() {
                    let row = project(row, world)?;
                    if row.value.is_none() {
                        return Err(KeyedCaptureError::SourceMismatch(STORAGE_OWNER.into()));
                    }
                    schema
                        .schema
                        .check_key(row.table, row.key)
                        .map_err(|_| KeyedCaptureError::SourceMismatch(row.table.into()))?;
                    assert!(rows.try_push(row).is_ok(), "admitted original row count");
                }
                continue;
            }
            let Some((_, descriptor)) = schema.schema.table(field.id) else {
                if matches!(
                    Collector::canonical(field.id)?.map(|f| f.role),
                    Some(Role::Canonical(Canonical::Table { .. }))
                ) && !field.current.as_slice().is_empty()
                {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                continue;
            };
            installed += 1;
            if descriptor.shape() == TableShape::Cell && field.current.as_slice().len() != 1 {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            for row in field.current.as_slice() {
                schema
                    .schema
                    .check_key(field.id, row.key.bytes())
                    .map_err(|_| KeyedCaptureError::SourceMismatch(field.id.into()))?;
                if row.value.is_none() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                assert!(
                    rows.try_push(BorrowedRow {
                        table: field.id,
                        key: row.key.bytes(),
                        value: row.value.as_ref().map(Frame::bytes)
                    })
                    .is_ok(),
                    "admitted original row count"
                );
            }
        }
        if !saw_backing || installed != schema.schema.tables().len() {
            return Err(KeyedCaptureError::SourceMismatch(
                "carrier schema has no exact original source".into(),
            ));
        }
        rows.as_mut_slice()
            .sort_unstable_by(|a, b| (a.table.as_bytes(), a.key).cmp(&(b.table.as_bytes(), b.key)));
        let mut builder = KeyedStateBuilderV1::new(schema.digest, storage_budget);
        for row in rows.as_slice() {
            let key = row.owned_key(self.budget())?;
            builder.push(&key.key, row.value.expect("complete original value"))?;
        }
        Ok(builder.finish()?)
    }

    pub(super) fn advance_carrier(
        &self,
        schema: &SelectedStateSchema,
        predecessor: &KeyedStateTreeV1,
        world: &impl WorldReadOnly,
    ) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
        let mut next = predecessor.clone();
        for field in self.fields.as_slice() {
            if field.replacement {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            let carrier = field.id == STORAGE_OWNER;
            if !carrier && schema.schema.table(field.id).is_none() {
                if !field.undo.as_slice().is_empty() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                continue;
            }
            if field.current.as_slice().len() != field.undo.as_slice().len() {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            for (old, new) in field.undo.as_slice().iter().zip(field.current.as_slice()) {
                if old.key.bytes() != new.key.bytes() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                let (before, after) = if carrier {
                    (project(old, world)?, project(new, world)?)
                } else {
                    (
                        BorrowedRow {
                            table: field.id,
                            key: old.key.bytes(),
                            value: old.value.as_ref().map(Frame::bytes),
                        },
                        BorrowedRow {
                            table: field.id,
                            key: new.key.bytes(),
                            value: new.value.as_ref().map(Frame::bytes),
                        },
                    )
                };
                schema
                    .schema
                    .check_key(before.table, before.key)
                    .map_err(|_| KeyedCaptureError::SourceMismatch(before.table.into()))?;
                if before.table != after.table || before.key != after.key {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                let key = before.owned_key(self.budget())?;
                if predecessor.get(&key.key) != before.value {
                    return Err(KeyedCaptureError::SourceMismatch(before.table.into()));
                }
                if before.value != after.value {
                    next.replace(&key.key, after.value)?;
                }
            }
        }
        Ok(next)
    }
}
