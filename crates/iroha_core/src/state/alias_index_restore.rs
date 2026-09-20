//! Restore alias lookup indexes from both retained authoritative images.

use super::*;
use mv::storage::History;

/// Validate each image before publishing its inverse lookup and undo entries.
fn inverse<K: mv::Key, V: mv::Value, A: mv::Key + std::fmt::Display>(
    history: &History<'_, K, V>,
    alias: impl Fn(&V) -> &A,
    validate: impl Fn(bool, &K, &V) -> Result<(), String>,
) -> Result<Storage<A, K>, String> {
    let mut current = BTreeMap::new();
    let mut previous = BTreeMap::new();
    for (prior, output) in [(false, &mut current), (true, &mut previous)] {
        let mut insert = |key: &K, value: &V| {
            validate(prior, key, value)?;
            let alias = alias(value);
            if output.insert(alias.clone(), key.clone()).is_some() {
                return Err(format!("Alias `{alias}` is bound to multiple targets"));
            }
            Ok(())
        };
        if prior {
            for (key, value) in history.iter_before_block() {
                insert(key, value)?;
            }
        } else {
            for (key, value) in history.current().iter() {
                insert(key, value)?;
            }
        }
    }
    let mut touched = BTreeSet::new();
    for (key, prior) in history.revert_map().iter() {
        for value in [prior.as_ref(), history.current().get(key)]
            .into_iter()
            .flatten()
        {
            touched.insert(alias(value).clone());
        }
    }
    let undo = touched
        .into_iter()
        .map(|alias| {
            let prior = previous.remove(&alias);
            (alias, prior)
        })
        .collect();
    Ok(Storage::from_snapshot_parts(current, undo))
}

pub(super) fn assets(world: &mut World) -> Result<(), String> {
    let definitions = world.asset_definitions.history();
    let domains = world.domains.history();
    let bindings = world.asset_definition_alias_bindings.history();
    // Inline aliases are invalid in either image, including deleted definitions.
    for (id, definition) in definitions
        .current()
        .iter()
        .chain(definitions.iter_before_block())
    {
        if let Some(alias) = definition.alias().as_ref() {
            return Err(format!(
                "Asset definition {id} stores inline alias `{alias}`; persist aliases only in asset_definition_alias_bindings"
            ));
        }
    }
    let index = inverse(
        &bindings,
        |binding| &binding.alias,
        |prior, id, binding| {
            validate_alias_lease_window(
                binding.lease_expiry_ms,
                binding.grace_until_ms,
                binding.bound_at_ms,
            )
            .map_err(|error| {
                format!(
                    "Asset alias binding `{}` has an invalid lease window: {error}",
                    binding.alias
                )
            })?;
            let definition = if prior {
                definitions.get_before_block(id)
            } else {
                definitions.current().get(id)
            };
            if definition.is_none() {
                return Err(format!(
                    "Asset alias binding `{}` references missing asset definition {id}",
                    binding.alias
                ));
            }
            if let Some(domain_name) = binding.alias.domain_segment() {
                let domain_id = DomainId::try_new(domain_name, binding.alias.dataspace_segment())
                    .map_err(|error| {
                    format!(
                        "Asset alias binding `{}` has an invalid domain: {error}",
                        binding.alias
                    )
                })?;
                let domain = if prior {
                    domains.get_before_block(&domain_id)
                } else {
                    domains.current().get(&domain_id)
                };
                if domain.is_none() {
                    return Err(format!(
                        "Asset alias binding `{}` references missing domain {domain_id}",
                        binding.alias
                    ));
                }
            }
            Ok(())
        },
    )?;
    world.asset_definition_aliases = index;
    Ok(())
}

pub(super) fn contracts(world: &mut World) -> Result<(), String> {
    let bindings = world.contract_alias_bindings.history();
    let index = inverse(
        &bindings,
        |binding| &binding.alias,
        |_, _, binding| {
            validate_alias_lease_window(
                binding.lease_expiry_ms,
                binding.grace_until_ms,
                binding.bound_at_ms,
            )
            .map_err(|error| {
                format!(
                    "Contract alias binding `{}` has an invalid lease window: {error}",
                    binding.alias
                )
            })
        },
    )?;
    world.contract_aliases = index;
    Ok(())
}

#[cfg(test)]
#[path = "alias_index_restore_tests.rs"]
mod tests;
