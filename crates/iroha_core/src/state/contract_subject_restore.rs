//! Restore the inverse only after validating both original subject source images.
//!
//! Source/undo remains borrowed through construction. Reconstructed reverse undo
//! preserves the difference between both logical images. Lifecycle-only writes
//! do not touch the reverse index; redundant binding touches cannot manufacture
//! reverse undo entries. Arbitrary redundant inverse writes are not inferred.
//! TODO: admit startup validation work and reconstruction allocations through the
//! original snapshot owner with the remaining infallible derived-index restores.

use super::{
    World,
    contract_subject_validation::{self as relation, Work},
};
use mv::storage::{Storage, StorageReadOnly};
use std::collections::BTreeMap;

/// Validate both authoritative images before replacing the derived inverse.
pub(crate) fn rebuild(world: &mut World) -> Result<(), String> {
    let world = &mut world.0;
    let rows = world.contract_subject_bindings.history();
    let accounts = world.accounts.history();
    let instances = world.contract_instances.history();
    relation::validate_sources(&rows, &accounts, &instances, &mut Work::startup())
        .map_err(relation::restore_error)?;
    let mut current = BTreeMap::new();
    let mut previous = BTreeMap::new();
    for (address, binding) in rows.current().iter() {
        if let Some(other) = current.insert(binding.subject.clone(), address.clone()) {
            return Err(format!(
                "contract subject `{}` is bound to both `{other}` and `{address}`",
                binding.subject
            ));
        }
    }
    for (address, binding) in rows.iter_before_block() {
        if let Some(other) = previous.insert(binding.subject.clone(), address.clone()) {
            return Err(format!(
                "predecessor contract subject `{}` is bound to both `{other}` and `{address}`",
                binding.subject
            ));
        }
    }
    let mut undo = BTreeMap::new();
    for (subject, address) in &previous {
        if current.get(subject) != Some(address) {
            undo.insert(subject.clone(), Some(address.clone()));
        }
    }
    for subject in current.keys() {
        if !previous.contains_key(subject) {
            undo.insert(subject.clone(), None);
        }
    }
    let reverse = Storage::from_snapshot_parts(current, undo);
    // All validation and allocation finished before publishing the replacement.
    world.contract_subject_addresses = reverse;
    Ok(())
}

#[cfg(test)]
mod tests;
