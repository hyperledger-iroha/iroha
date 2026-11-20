use super::*;
use iroha_data_model::{
    isi::kagemusha_wallet::KagemushaWalletLedgerActionV1 as Action,
    runtime::definition::{DefinitionId, RestrictionEntryV1, RestrictionV1},
};

pub(super) fn restrict(
    selection: &RootSelectionV1,
    definition: DefinitionId,
    restriction: RestrictionV1,
) -> RootSelectionV1 {
    let mut next = selection.clone();
    next.restrictions
        .retain(|entry| entry.definition != definition);
    next.restrictions.push(RestrictionEntryV1 {
        definition,
        restriction,
    });
    next.restrictions.sort_by_key(|entry| entry.definition);
    next
}

fn load(install: &Install) -> KagemushaWalletLedgerV1 {
    KagemushaWalletLedgerV1::new(
        install.scheme,
        Action::IssueLoad {
            wallet: [5; 32],
            asset: install.asset,
            ordinal: 0,
            request_id: [1; 32],
            amount: 1,
            charge: None,
        },
    )
}

#[test]
fn only_the_installed_unload_is_a_completion_at_the_runtime_resolver() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let active = selected(&state);
    let bound = active
        .bindings
        .iter()
        .find(|b| b.wire_id == Unload::WIRE_ID)
        .unwrap();
    let install = specimen(&memory);
    let request = Unload {
        scheme: install.scheme,
        asset: install.asset,
        manifest_digest: install.manifest_digest,
        claim: vec![1],
    };
    let activate = Activate {
        scheme: install.scheme,
        asset: install.asset,
        manifest_digest: install.manifest_digest,
        wallet: [5; 32],
        activation: vec![1],
    };
    transact(&mut state, 2, |tx| {
        for (definition, restriction) in [
            (bound.interface, RestrictionV1::AdmissionClosed(2)),
            (bound.implementation, RestrictionV1::AdmissionClosed(2)),
            (bound.implementation, RestrictionV1::CompletionOnly(2)),
        ] {
            select_published(tx, restrict(&active, definition, restriction));
            tx.world.kagemusha_spartan_programs.remove(install.scheme);
            assert!(crate::runtime::resolve_executing(tx, &request.clone().into()).is_err());
            tx.world
                .kagemusha_spartan_programs
                .insert(install.scheme, install.clone());
            assert_eq!(
                crate::runtime::resolve_executing(tx, &request.clone().into()).unwrap(),
                Some(bound.implementation)
            );
            for mismatch in [
                Unload {
                    scheme: [0; 32],
                    ..request.clone()
                },
                Unload {
                    asset: [0; 32],
                    ..request.clone()
                },
                Unload {
                    manifest_digest: [0; 32],
                    ..request.clone()
                },
                Unload {
                    claim: Vec::new(),
                    ..request.clone()
                },
            ] {
                assert!(crate::runtime::resolve_executing(tx, &mismatch.into()).is_err());
            }
            if definition == bound.implementation {
                for admission in [
                    activate.clone().into(),
                    install.clone().into(),
                    load(&install).into(),
                ] {
                    assert!(crate::runtime::resolve_executing(tx, &admission).is_err());
                }
            }
            select_published(
                tx,
                restrict(&active, definition, RestrictionV1::Disabled(2)),
            );
            assert!(crate::runtime::resolve_executing(tx, &request.clone().into()).is_err());
        }
        let mut partial = restrict(
            &active,
            bound.implementation,
            RestrictionV1::CompletionOnly(2),
        );
        partial
            .bindings
            .iter_mut()
            .find(|b| b.wire_id == Activate::WIRE_ID)
            .unwrap()
            .interface = bound.interface;
        select_published(tx, partial);
        assert!(crate::runtime::resolve_executing(tx, &request.into()).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_disable_checks_the_scoped_reserve_and_closes_new_funding_even_before_its_height() {
    for balance in [0, 4] {
        let mut memory = Memory::new();
        memory
            .balances
            .insert(memory.registration.reserve.clone(), balance);
        let mut state = world_state(&memory, true);
        let baseline = state.view().world.runtime_selection().clone();
        let active = selected(&state);
        let bound = active
            .bindings
            .iter()
            .find(|b| b.wire_id == Unload::WIRE_ID)
            .unwrap();
        let install = specimen(&memory);
        let catalog = crate::runtime::release_catalog().unwrap();
        let other = *catalog
            .spartan_predecessors
            .values()
            .find(|owner| **owner != bound.implementation)
            .unwrap();
        let mut rebound = active.clone();
        for entry in &mut rebound.bindings {
            if entry.implementation == bound.implementation {
                entry.implementation = other;
            }
        }
        // The published native implementation declares baseline interfaces as
        // callees. A changed callee matters even with an unchanged Unload binding.
        let baseline_callee = active
            .bindings
            .iter()
            .find(|b| {
                b.implementation != bound.implementation
                    && baseline.bindings.iter().any(|before| before == *b)
            })
            .unwrap()
            .interface;
        register(&mut state, &memory, 1).unwrap();
        transact(&mut state, 2, |tx| {
            for definition in [bound.interface, bound.implementation, baseline_callee] {
                for disable_at in [2, 100] {
                    select_published(tx, active.clone());
                    tx.world
                        .kagemusha_spartan_programs
                        .insert(install.scheme, install.clone());
                    let next = restrict(&active, definition, RestrictionV1::Disabled(disable_at));
                    assert!(spartan::check_transition(&*tx.world, &baseline).is_err());
                    assert!(spartan::check_transition(&*tx.world, &rebound).is_err());
                    assert_eq!(
                        spartan::check_transition(&*tx.world, &next).is_ok(),
                        balance == 0
                    );
                    if balance == 0 {
                        select_published(tx, next);
                        let error =
                            crate::smartcontracts::isi::kagemusha_wallet::dispatch_spartan_ledger(
                                &load(&install).into(),
                                &memory.authority,
                                tx,
                            )
                            .unwrap()
                            .unwrap_err();
                        assert!(
                            error.to_string().contains(&Error::Lifecycle.to_string()),
                            "{error}"
                        );
                        tx.world.kagemusha_spartan_programs.remove(install.scheme);
                        let legacy_error =
                            crate::smartcontracts::isi::kagemusha_wallet::dispatch_spartan_ledger(
                                &load(&install).into(),
                                &memory.authority,
                                tx,
                            )
                            .unwrap()
                            .unwrap_err();
                        assert!(
                            legacy_error.to_string().contains("exact signed payer"),
                            "{legacy_error}"
                        );
                        assert!(matches!(
                            spartan::install(tx, &memory.registration.reserve, &install),
                            Err(Error::Lifecycle)
                        ));
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    }
}
