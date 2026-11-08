//! Selection, dependency and restriction controls for the authority addition.

use super::*;
use iroha_crypto::{Hash, HashOf};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x75; 32])))
}

fn published_baseline(catalog: &ReleaseCatalog) -> BTreeMap<DefinitionId, DefinitionDescriptorV1> {
    catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect()
}

#[test]
fn authority_draft_selects_services_and_three_tables_without_game_dependencies() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let published = published_baseline(catalog);
    let before = catalog.baseline.clone();
    let change = draft(catalog, &before, &published, network(), 50, [1; 32]).unwrap();
    let applied = change.apply(&before, &published).unwrap();
    let after = &applied.selection;
    assert_ne!(after.services_profile, before.services_profile);
    assert!(!crate::runtime::sorafs_authority_enabled(&before).unwrap());
    assert!(crate::runtime::sorafs_authority_enabled(after).unwrap());
    assert_eq!(
        after.installed_storage.len(),
        before.installed_storage.len() + 3
    );
    assert!(catalog.unsupported(after).is_none());
    for wire in [
        "iroha.sorafs.authority.propose.v1",
        "iroha.sorafs.authority.attempt.create.v1",
    ] {
        assert!(before.bindings.iter().all(|b| b.wire_id != wire));
        assert!(after.bindings.iter().any(|b| b.wire_id == wire));
    }
    assert_eq!(after.kernel, before.kernel);
    assert_eq!(after.commitment_profile, before.commitment_profile);
    let old_other_wires: Vec<_> = before
        .bindings
        .iter()
        .map(|binding| &binding.wire_id)
        .collect();
    for wire in old_other_wires {
        assert!(
            after
                .bindings
                .iter()
                .any(|binding| &binding.wire_id == wire)
        );
    }
    let mut after_published = published.clone();
    after_published.extend(applied.published);
    after.check_closure(&after_published).unwrap();
    for (id, descriptor) in &published {
        assert_eq!(after_published.get(id), Some(descriptor));
    }
    assert!(draft(catalog, after, &after_published, network(), 51, [2; 32]).is_err());
    // A node must own each exact selected definition, not just recognize its service name.
    let mut unsupported = catalog.clone();
    unsupported.definitions.remove(&after.services_profile);
    assert!(unsupported.unsupported(after).is_some());
}

#[test]
fn authority_instructions_require_the_exact_selected_successor() {
    use iroha_data_model::{
        isi::{
            InstructionBox,
            error::InstructionExecutionError,
            sorafs_authority::{
                CreateSorafsAuthorityParliamentAttemptV1, ProposeSorafsAuthorityGovernanceV1,
                SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsOperatingAuthorityV1,
            },
        },
        runtime::lifecycle::RuntimeRejectionV1,
    };

    let catalog = crate::runtime::release_catalog().unwrap();
    let before = &catalog.baseline;
    let published = published_baseline(catalog);
    let change = draft(catalog, before, &published, network(), 50, [7; 32]).unwrap();
    let after = change.apply(before, &published).unwrap().selection;
    let proposal = SorafsAuthorityProposalV1 {
        grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
            epoch_length_blocks: 100,
        },
        term: 1,
        network: network(),
        root: RootScopeV1::Universal,
        account: iroha_test_samples::ALICE_ID.clone(),
        authority: SorafsOperatingAuthorityV1::SetReservePolicy,
        action: SorafsAuthorityActionV1::Grant,
        expected_revision: 0,
        completion_policy: None,
    };
    let instructions: [InstructionBox; 2] = [
        ProposeSorafsAuthorityGovernanceV1 {
            proposal: proposal.clone(),
        }
        .into(),
        CreateSorafsAuthorityParliamentAttemptV1 {
            proposal,
            attempt_sequence: 0,
        }
        .into(),
    ];
    for instruction in &instructions {
        let wire_id = iroha_data_model::isi::instruction_wire_id(instruction).unwrap();
        let inactive = InstructionExecutionError::Runtime(RuntimeRejectionV1::DefinitionInactive {
            wire_id: wire_id.into(),
        });
        assert_eq!(
            crate::runtime::resolve(instruction, before, 50, || None),
            Err(inactive.clone())
        );
        let implementation = after
            .bindings
            .iter()
            .find(|binding| binding.wire_id == wire_id)
            .unwrap()
            .implementation;
        assert_eq!(
            crate::runtime::resolve(instruction, &after, 50, || None),
            Ok(Some(implementation))
        );
        let DefinitionDescriptorV1::Implementation(descriptor) =
            &catalog.definitions[&implementation]
        else {
            panic!("selected native implementation")
        };
        assert!(matches!(
            &descriptor.code,
            ImplementationCodeV1::Native { module, .. } if module == SORAFS_AUTHORITY.module
        ));
        assert_eq!(descriptor.services_profile, after.services_profile);
    }
}

#[test]
fn authority_draft_carries_existing_restrictions_to_replacement_implementations() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut before = catalog.baseline.clone();
    let published = published_baseline(catalog);
    let binding = before.bindings[0].clone();
    before.restrictions = vec![
        RestrictionEntryV1 {
            definition: binding.implementation,
            restriction: RestrictionV1::AdmissionClosed(1),
        },
        RestrictionEntryV1 {
            definition: binding.interface,
            restriction: RestrictionV1::Disabled(2),
        },
    ];
    before.restrictions.sort_by_key(|entry| entry.definition);
    let change = draft(catalog, &before, &published, network(), 50, [6; 32]).unwrap();
    let applied = change.apply(&before, &published).unwrap();
    let replacement = applied
        .selection
        .bindings
        .iter()
        .find(|b| b.wire_id == binding.wire_id)
        .unwrap();
    assert_ne!(replacement.implementation, binding.implementation);
    assert!(
        applied
            .selection
            .restrictions
            .contains(&RestrictionEntryV1 {
                definition: replacement.implementation,
                restriction: RestrictionV1::AdmissionClosed(50),
            })
    );
    assert!(
        applied
            .selection
            .restrictions
            .contains(&RestrictionEntryV1 {
                definition: binding.interface,
                restriction: RestrictionV1::Disabled(2),
            })
    );
    assert!(!applied.selection.active.contains(&binding.implementation));
}

#[test]
fn authority_composition_rejects_foreign_storage_interface_and_service_identity() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut wrong = catalog.baseline.clone();
    wrong.installed_storage[0] = DefinitionId([0x55; 32]);
    assert!(representative(catalog, &wrong).is_err());
    wrong = catalog.baseline.clone();
    wrong.bindings[0].interface = DefinitionId([0x55; 32]);
    assert!(representative(catalog, &wrong).is_err());
    wrong = catalog.baseline.clone();
    wrong.services_profile = DefinitionId([0x55; 32]);
    assert!(representative(catalog, &wrong).is_err());
    assert!(crate::runtime::sorafs_authority_enabled(&wrong).is_err());
    let mut catalog = catalog.clone();
    let id = catalog.baseline.installed_storage[0];
    let DefinitionDescriptorV1::Storage(mut descriptor) = catalog.definitions[&id].clone() else {
        panic!("storage")
    };
    descriptor.invariants = [0x66; 32];
    let duplicate = catalog
        .publish(DefinitionDescriptorV1::Storage(descriptor))
        .unwrap();
    wrong = catalog.baseline.clone();
    wrong.installed_storage.push(duplicate);
    assert!(selected_storage(&catalog, &wrong).is_err());
}

#[test]
fn authority_draft_preserves_the_certified_repair_kernel_and_existing_additions() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut published = published_baseline(catalog);
    let repair = catalog
        .sorafs_repair_activation(&catalog.baseline, &published, network(), 20, [3; 32])
        .unwrap();
    let applied = repair.apply(&catalog.baseline, &published).unwrap();
    published.extend(applied.published);
    let before = applied.selection;
    let change = draft(catalog, &before, &published, network(), 50, [4; 32]).unwrap();
    let applied = change.apply(&before, &published).unwrap();
    let after = applied.selection;
    assert_eq!(after.kernel, before.kernel);
    assert_ne!(after.kernel, catalog.baseline.kernel);
    assert_eq!(after.commitment_profile, before.commitment_profile);
    assert!(catalog.unsupported(&after).is_none());
    let repair_wire = "iroha.sorafs.bootstrap.repair.apply.v1";
    let original = before
        .bindings
        .iter()
        .find(|binding| binding.wire_id == repair_wire)
        .unwrap();
    let selected = after
        .bindings
        .iter()
        .find(|binding| binding.wire_id == repair_wire)
        .unwrap();
    assert_eq!(selected.interface, original.interface);
    assert_eq!(
        catalog.native_module(&selected.implementation),
        Some(crate::runtime::catalog::additions::SORAFS_REPAIR.module)
    );
    assert_ne!(selected.implementation, original.implementation);
    published.extend(applied.published);
    after.check_closure(&published).unwrap();
    assert!(
        catalog
            .sorafs_repair_activation(&after, &published, network(), 60, [5; 32])
            .is_err()
    );
    for (id, descriptor) in repair
        .publish
        .iter()
        .map(|descriptor| (descriptor.id().unwrap(), descriptor))
    {
        assert_eq!(published.get(&id), Some(descriptor));
    }
    assert!(
        after
            .bindings
            .iter()
            .all(|binding| !binding.wire_id.starts_with("iroha.game_partition."))
    );
}

#[test]
fn authority_activation_cannot_rebind_live_escrow_or_amx_completion_custody() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let published = published_baseline(catalog);
    let before = &catalog.baseline;
    let change = draft(catalog, before, &published, network(), 50, [8; 32]).unwrap();
    let after = change.apply(before, &published).unwrap().selection;
    for live in [
        crate::runtime::obligations::LiveObligations::with_escrows(1),
        crate::runtime::obligations::LiveObligations::with_amx(1, 0),
        crate::runtime::obligations::LiveObligations::with_amx(0, 1),
    ] {
        assert!(crate::runtime::obligations::stranded(before, &after, &published, &live).is_some());
    }
}
