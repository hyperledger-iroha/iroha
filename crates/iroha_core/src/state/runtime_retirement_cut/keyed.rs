//! Complete retirement census from the original immutable keyed execution tree.

use super::*;
use crate::state::{StorageReadOnly as _, keyed_state::KeyedCut};
use iroha_data_model::{
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionId, SchemaV1, StorageShapeV1},
        retirement::MAX_RETIREMENT_DEFINITIONS,
    },
    sumeragi_finality::{
        keyed_schema::TableShape,
        keyed_state::{KeyedStateTreeV1, StateTableVisitError},
    },
};

fn exact<T: norito::codec::Encode>(value: &T, original: &[u8]) -> Result<(), RetirementCutError> {
    norito::core::classify_decode_attempt(|| norito::codec::verify_exact_payload(value, original))
        .map_err(RetirementCutError::Codec)
}

fn visit_error(error: StateTableVisitError<RetirementCutError>) -> RetirementCutError {
    match error {
        StateTableVisitError::Tree(original) => RetirementCutError::Tree(original),
        StateTableVisitError::Consumer(original) => original,
    }
}

fn cell<T: norito::codec::Encode>(
    tree: &KeyedStateTreeV1,
    field: &str,
    value: &T,
) -> Result<(), RetirementCutError> {
    let count = tree
        .visit_table(field, 1, |key, original| {
            if !key.is_empty() {
                return Err(RetirementCutError::Invalid("singleton key differs"));
            }
            exact(value, original)
        })
        .map_err(visit_error)?;
    if count != 1 {
        return Err(RetirementCutError::Invalid("original singleton is absent"));
    }
    Ok(())
}

fn table<'a, T: norito::codec::Encode + 'a>(
    tree: &KeyedStateTreeV1,
    field: &str,
    mut current: impl Iterator<Item = (&'a DefinitionId, &'a T)>,
) -> Result<(), RetirementCutError> {
    // These two tables have the same fixed DefinitionId key codec. Its canonical byte
    // order is the retained key order; exact streaming comparison checks every pair.
    tree.visit_table(field, MAX_RETIREMENT_DEFINITIONS as u64, |key, original| {
        let Some((id, value)) = current.next() else {
            return Err(RetirementCutError::Invalid(
                "original census contains an omitted row",
            ));
        };
        exact(id, key)?;
        exact(value, original)
    })
    .map_err(visit_error)?;
    if current.next().is_some() {
        return Err(RetirementCutError::Invalid(
            "current census adds an uncommitted row",
        ));
    }
    Ok(())
}

fn native_schema(
    cut: &KeyedCut,
    view: &StateQueryView<'_>,
    field: &str,
    allow_absent: bool,
) -> Result<(), RetirementCutError> {
    let catalog = crate::runtime::release_catalog()?;
    let id = catalog
        .world_storage
        .get(field)
        .ok_or(RetirementCutError::Invalid(
            "census table has no compiled owner",
        ))?;
    let selected = view
        .world()
        .runtime_selection()
        .installed_storage
        .binary_search(id)
        .is_ok();
    let descriptor = cut
        .schema
        .schema
        .table(field)
        .map(|(_, descriptor)| descriptor);
    if allow_absent && !selected && descriptor.is_none() {
        return Ok(());
    }
    let descriptor = descriptor.ok_or(RetirementCutError::Invalid(
        "selected native census table is absent",
    ))?;
    let definition = catalog
        .definitions
        .get(id)
        .ok_or(RetirementCutError::Invalid("native census owner is absent"))?;
    if !selected || view.world().runtime_definitions().get(id) != Some(definition) {
        return Err(RetirementCutError::Invalid(
            "native census owner is not the exact selected definition",
        ));
    }
    let DefinitionDescriptorV1::Storage(storage) = definition else {
        return Err(RetirementCutError::Invalid(
            "native census owner is not storage",
        ));
    };
    let SchemaV1::Codec(value_schema) = &storage.value else {
        return Err(RetirementCutError::Invalid(
            "native census value codec differs",
        ));
    };
    if descriptor.value_schema() != value_schema || storage.namespace != field {
        return Err(RetirementCutError::Invalid(
            "native census value identity differs",
        ));
    }
    match &storage.shape {
        StorageShapeV1::Cell
            if descriptor.shape() == TableShape::Cell && descriptor.key_schema().is_empty() =>
        {
            Ok(())
        }
        StorageShapeV1::Table {
            key: SchemaV1::Codec(key_schema),
        } if descriptor.shape() == TableShape::Table && descriptor.key_schema() == key_schema => {
            Ok(())
        }
        _ => Err(RetirementCutError::Invalid(
            "native census key identity differs",
        )),
    }
}

pub(super) fn authenticate(
    state: &State,
    view: &StateQueryView<'_>,
    tip: &crate::sumeragi::certified_chain::CommittedBlock,
) -> Result<KeyedCut, RetirementCutError> {
    let native = view
        .native_execution_tip()
        .ok_or(RetirementCutError::Unexecuted)?;
    // The native tip names the committed block's hash `iroha_hash`.
    let same_block = native.iroha_hash() == tip.block_hash();
    if native.height() != tip.height()
        || !same_block
        || native.core_hash() != tip.core_hash()
        || native.result() != tip.result()
        || native.root_scope() != tip.root_scope()
        || view.height() as u64 != tip.height()
        || view.latest_block_hash() != Some(tip.block_hash())
        || *view.network_id() != tip.commitment().schedule.current.network_id
    {
        return Err(RetirementCutError::Invalid(
            "keyed census differs from original execution",
        ));
    }
    let cut = state
        .keyed_versions
        .lock()
        .published_at(native)
        .cloned()
        .ok_or(RetirementCutError::Invalid(
            "original keyed version is absent",
        ))?;
    let execution = cut.execution.as_ref().ok_or(RetirementCutError::Invalid(
        "restored published State has no original execution census",
    ))?;
    if &execution.root() != tip.commitment().execution.world_state_root.as_ref()
        || execution.schema_digest() != cut.schema.digest
        || cut.published.schema_digest() != cut.schema.digest
        || cut.schema.schema.digest() != cut.schema.digest
    {
        return Err(RetirementCutError::Invalid(
            "original keyed schema or execution root differs",
        ));
    }
    native_schema(&cut, view, "world.runtime_selection", false)?;
    native_schema(&cut, view, "world.private_dataspaces", false)?;
    native_schema(
        &cut,
        view,
        "world.runtime_definitions",
        view.world().runtime_definitions().iter().next().is_none(),
    )?;
    native_schema(
        &cut,
        view,
        "world.runtime_retirements",
        view.world().runtime_retirements().iter().next().is_none(),
    )?;
    cell(
        execution,
        "world.runtime_selection",
        view.world().runtime_selection(),
    )?;
    cell(
        execution,
        "world.private_dataspaces",
        view.world().private_dataspaces(),
    )?;
    table(
        execution,
        "world.runtime_definitions",
        view.world().runtime_definitions().iter(),
    )?;
    table(
        execution,
        "world.runtime_retirements",
        view.world().runtime_retirements().iter(),
    )?;
    Ok(cut)
}

#[cfg(test)]
mod tests;
