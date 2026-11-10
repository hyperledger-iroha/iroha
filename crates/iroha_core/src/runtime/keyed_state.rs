//! Governed definition identity for the ordered radix State commitment.

use iroha_data_model::runtime::definition::{DefinitionLookup, RootSelectionV1};
pub use iroha_data_model::runtime::keyed_state::profile_descriptor as descriptor;
use iroha_data_model::{
    NetworkId,
    runtime::{
        keyed_state::{adoption_kernel, profile_id},
        lifecycle::{ChangeSetV1, RootScopeV1, SelectionTransitionV1},
    },
};

/// Whether the committed selection explicitly selects this construction.
pub fn selected(selection: &RootSelectionV1) -> bool {
    selection.commitment_profile == profile_id()
}

/// A known keyed successor has exactly the compiled State owners. The predecessor
/// must explicitly adopt the kernel before its first profile cutover. A downgrade to
/// the historical known profile would abandon the indexed State without a migration.
pub(super) fn validate_transition(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
) -> Result<(), String> {
    let catalog = super::release_catalog().map_err(|e| e.to_string())?;
    if selected(previous) && next.commitment_profile == catalog.baseline.commitment_profile {
        return Err(
            "cannot restore the predecessor commitment without a governed migration".into(),
        );
    }
    if !selected(next) {
        if next.commitment_profile == catalog.baseline.commitment_profile
            && catalog
                .keyed_storage
                .iter()
                .any(|id| next.installed_storage.binary_search(id).is_ok())
        {
            return Err(
                "the World-only commitment cannot install State-level keyed storage".into(),
            );
        }
        return Ok(());
    }
    let adoption = adoption_kernel(&catalog.keyed_storage)
        .id()
        .map_err(|e| e.to_string())?;
    if !selected(previous)
        && (previous.kernel != adoption
            || previous.commitment_profile != catalog.baseline.commitment_profile
            || next.kernel != adoption
            || next.services_profile != previous.services_profile
            || next.bindings != previous.bindings
            || !previous
                .installed_storage
                .iter()
                .all(|id| next.installed_storage.contains(id)))
    {
        return Err(
            "keyed State adoption requires the original storage, kernel, and application bindings"
                .into(),
        );
    }
    schema_digest(next)?;
    Ok(())
}

/// Resolve the keyed schema from the release descriptors and an independently chosen selection.
/// # Errors
/// Unknown or incomplete keyed selection.
pub fn schema_digest(selection: &RootSelectionV1) -> Result<[u8; 32], String> {
    crate::state::keyed_state::schema_for(selection).map(|schema| schema.digest)
}

/// Draft one of the two governed transitions using the caller's authenticated
/// selection and published definitions. The first installs the adoption kernel.
/// The second imports the complete original State during the commitment switch.
/// Neither transition changes a running network until its ordinary governance
/// and validator attestations have authorized the exact proposal.
/// # Errors
/// The predecessor is unsupported, qualification or height is missing, or the
/// resulting selection would strand or drop a definition.
pub fn next_change_set(
    network: NetworkId,
    previous: &RootSelectionV1,
    published: &impl DefinitionLookup,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, String> {
    if effective_height < 2 || qualification == [0; 32] {
        return Err("a qualified successor height is required".into());
    }
    let catalog = super::release_catalog().map_err(|e| e.to_string())?;
    let kernel = adoption_kernel(&catalog.keyed_storage)
        .id()
        .map_err(|e| e.to_string())?;
    if previous.commitment_profile != catalog.baseline.commitment_profile
        || catalog.unsupported(previous).is_some()
    {
        return Err("unsupported keyed State activation predecessor".into());
    }
    let mut successor = previous.clone();
    let mut needed = Vec::new();
    let mut transitions = Vec::new();
    if previous.kernel == catalog.baseline.kernel {
        needed.push(kernel);
        transitions.push(SelectionTransitionV1::SetKernel(kernel));
        successor.kernel = kernel;
    } else if previous.kernel == kernel {
        if catalog
            .keyed_storage
            .iter()
            .any(|id| previous.installed_storage.binary_search(id).is_ok())
        {
            return Err("a partial State storage import cannot be activated".into());
        }
        let permission = super::state_row_permission::descriptor()
            .id()
            .map_err(|e| e.to_string())?;
        needed.extend([profile_id(), permission]);
        needed.extend_from_slice(&catalog.keyed_storage);
        transitions.extend(
            catalog
                .keyed_storage
                .iter()
                .copied()
                .map(SelectionTransitionV1::InstallStorage),
        );
        transitions.push(SelectionTransitionV1::SetCommitmentProfile(profile_id()));
        transitions.push(SelectionTransitionV1::Activate(permission));
        successor.commitment_profile = profile_id();
        successor.active.push(permission);
        successor.active.sort_unstable();
        successor
            .installed_storage
            .extend_from_slice(&catalog.keyed_storage);
        successor.installed_storage.sort_unstable();
    } else {
        return Err("selected kernel cannot import the keyed State owners".into());
    }
    needed.sort_unstable();
    let publish = needed
        .into_iter()
        .filter(|id| published.definition(id).is_none())
        .map(|id| {
            catalog
                .definitions
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("missing compiled definition {id}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let change = ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: previous.id().map_err(|e| e.to_string())?,
        successor: successor.id().map_err(|e| e.to_string())?,
        effective_height,
        publish,
        transitions,
        qualification,
    };
    change
        .apply(previous, published)
        .map_err(|e| e.to_string())?;
    Ok(change)
}

/// Install the profile in an isolated genesis fixture, retaining all of its active applications.
/// Production transitions require governed publication and installation of the same owners.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn install_for_test(world: &mut crate::state::World) {
    use mv::cell::Cell;
    let catalog = super::release_catalog().expect("compiled catalog");
    let mut selection = world.runtime_selection.view().get().clone();
    selection.commitment_profile = descriptor().id().expect("compiled profile id");
    selection
        .installed_storage
        .extend_from_slice(&catalog.keyed_storage);
    selection.installed_storage.sort_unstable();
    selection.installed_storage.dedup();
    selection.active.push(
        super::state_row_permission::descriptor()
            .id()
            .expect("compiled permission"),
    );
    selection.active.sort_unstable();
    selection.active.dedup();
    world.runtime_definitions = catalog
        .definitions
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    world.runtime_selection = Cell::new(selection);
}

/// Exercise the approved kernel and commitment-profile transitions on a genuine certified test chain.
/// The fixture seeds each scheduled four-signature record; native activation and publication remain mandatory.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn activate_for_test(chain: &mut crate::sumeragi::test_chain::CertifiedTestChain) {
    use crate::state::{StorageReadOnly, WorldReadOnly};
    for step in 0..2 {
        let height = chain.height() + 2;
        let change = {
            let view = chain.state().view();
            let published: std::collections::BTreeMap<_, _> = view
                .world()
                .runtime_definitions()
                .iter()
                .map(|(id, definition)| (*id, definition.clone()))
                .collect();
            next_change_set(
                chain.network_id(),
                view.world().runtime_selection(),
                &published,
                height,
                [step + 1; 32],
            )
            .unwrap()
        };
        let at = chain.committed(chain.height()).block_time_ms() + 1;
        super::ironwood::test_activation::schedule_change(chain, at, change.clone());
        chain.commit_at(at + 1, vec![]);
        chain.commit_at(at + 2, vec![]);
        assert_eq!(chain.height(), height);
        assert_eq!(
            chain
                .state()
                .view()
                .world()
                .runtime_selection()
                .id()
                .unwrap(),
            change.successor
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyed_profile_is_a_distinct_immutable_selection() {
        let catalog = crate::runtime::release_catalog().unwrap();
        let mut selection = catalog.baseline.clone();
        assert!(!selected(&selection));
        selection.commitment_profile = descriptor().id().unwrap();
        assert!(selected(&selection));
        assert_ne!(selection.id().unwrap(), catalog.baseline_id);
    }
    #[test]
    fn governed_kernel_then_profile_import_the_exact_registered_owners() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::runtime::definition::DefinitionDescriptorV1;
        use std::collections::BTreeMap;
        let catalog = crate::runtime::release_catalog().unwrap();
        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"keyed transition test",
        )));
        let mut published: BTreeMap<_, DefinitionDescriptorV1> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let first = next_change_set(network, &catalog.baseline, &published, 10, [1; 32]).unwrap();
        let upgraded = first.apply(&catalog.baseline, &published).unwrap();
        assert_eq!(
            upgraded.selection.commitment_profile,
            catalog.baseline.commitment_profile
        );
        published.extend(upgraded.published);
        let second =
            next_change_set(network, &upgraded.selection, &published, 20, [2; 32]).unwrap();
        let imported = second.apply(&upgraded.selection, &published).unwrap();
        assert!(selected(&imported.selection));
        validate_transition(&upgraded.selection, &imported.selection).unwrap();
        assert!(validate_transition(&catalog.baseline, &imported.selection).is_err());
        let mut partial = imported.selection.clone();
        partial
            .installed_storage
            .retain(|id| *id != catalog.keyed_storage[0]);
        assert!(schema_digest(&partial).is_err());
        let mut partial_on_lthash = catalog.baseline.clone();
        partial_on_lthash
            .installed_storage
            .push(catalog.keyed_storage[0]);
        partial_on_lthash.installed_storage.sort_unstable();
        assert!(validate_transition(&catalog.baseline, &partial_on_lthash).is_err());
        let mut incomplete = second;
        incomplete
            .transitions
            .retain(|t| *t != SelectionTransitionV1::InstallStorage(catalog.keyed_storage[0]));
        assert!(incomplete.apply(&upgraded.selection, &published).is_err());
        assert!(validate_transition(&imported.selection, &upgraded.selection).is_err());
    }

    #[test]
    fn independent_executions_agree_on_keyed_roots_in_every_insertion_order() {
        use crate::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };

        let mut source = None;
        let mut reference = None;
        for offset in 0..4 {
            let mut world = World::new();
            {
                let mut rows = world.smart_contract_state.block();
                for i in 0..4 {
                    let value = (offset + i) % 4;
                    rows.insert(format!("fixture/row/{value}").parse().unwrap(), vec![value]);
                }
                rows.commit();
            }
            install_for_test(&mut world);
            let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
            if offset == 3 {
                chain.replay_from(source.as_ref().unwrap()).unwrap();
            } else {
                chain.commit_at(2000, vec![]);
                chain.commit_at(3000, vec![]);
            }
            let roots: Vec<_> = (1..=3)
                .map(|height| {
                    let tip = chain.committed(height);
                    let execution = tip.commitment().execution;
                    (
                        tip.result(),
                        execution.parent_world_state_root,
                        execution.world_state_root,
                    )
                })
                .collect();
            let published = chain.state().verify_replayed_state_commitment().unwrap();
            let observed = (roots, published);
            assert_eq!(&observed, reference.get_or_insert_with(|| observed.clone()));
            if source.is_none() {
                source = Some(chain);
            }
        }
    }

    #[test]
    fn certified_blocks_adopt_keyed_state_without_changing_the_predecessor_root() {
        use crate::{
            state::{StorageReadOnly, World, WorldReadOnly},
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        use iroha_allocation::AllocationBudget;
        use std::collections::BTreeMap;
        let mut world = World::new();
        super::super::ironwood::install_for_test(&mut world);
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
        for step in 0..2 {
            let height = chain.height() + 2;
            let change = {
                let view = chain.state().view();
                let published: BTreeMap<_, _> = view
                    .world()
                    .runtime_definitions()
                    .iter()
                    .map(|(id, definition)| (*id, definition.clone()))
                    .collect();
                next_change_set(
                    chain.network_id(),
                    view.world().runtime_selection(),
                    &published,
                    height,
                    [step + 1; 32],
                )
                .unwrap()
            };
            let at = chain.committed(chain.height()).block_time_ms() + 1000;
            super::super::ironwood::test_activation::schedule_change(&chain, at, change.clone());
            chain.commit_at(at, vec![]);
            let before = chain.state().verify_world_state_accumulator().unwrap();
            chain.commit_at(at + 1000, vec![]);
            let tip = chain.committed(height);
            assert_eq!(tip.commitment().execution.parent_world_state_root, before);
            let view = chain.state().view();
            assert_eq!(
                view.world().runtime_selection().id().unwrap(),
                change.successor
            );
            drop(view);
            let rows = vec![(String::from("state.transactions.frontier"), vec![])];
            let answer =
                chain
                    .state()
                    .keyed_state_rows(&tip, &rows, &AllocationBudget::new(1 << 20));
            if step == 0 {
                assert!(answer.is_err());
            } else {
                let answer = answer.unwrap();
                assert_eq!(
                    answer.rows[0].value.as_ref().unwrap(),
                    &norito::codec::encode_adaptive(&height)
                );
                assert!(answer.rows[0].witness.verify(
                    &answer.schema_hash,
                    tip.commitment().execution.world_state_root.payload(),
                    &answer.rows[0].position,
                    answer.rows[0].value.as_deref()
                ));
                let applied = chain.state().verify_replayed_state_commitment().unwrap();
                chain.commit_at(at + 2000, vec![]);
                assert_eq!(
                    chain
                        .committed(height + 1)
                        .commitment()
                        .execution
                        .parent_world_state_root,
                    applied
                );
            }
        }
    }
}
