//! Paid native retirement effects after an all-validator scheduled service activation.
//!
//! The test-only setup corridor installs the already approved change-set record. The ordinary
//! activation hook, signed floor instruction, paid retirement, archive and finality paths run.

use super::*;
use crate::{
    runtime::{catalog::additions, change_set::record_attestation},
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    isi::{InstructionBox, Log, runtime_boundary::RaiseRuntimeStartupFloorV1},
    parameter::{Parameter, RuntimeCheckpointPolicyV1},
    runtime::{
        checkpoint::{StartupFloorAttestationV1, StartupFloorV1},
        lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, RuntimeNoticeV1, SupportAttestationV1,
            committee_digest,
        },
    },
};

fn key() -> KeyPair {
    KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519)
}
fn commit(chain: &mut CertifiedTestChain, instruction: impl Into<InstructionBox>) -> bool {
    let at = 1_000 + chain.height() * 1_000;
    let signed = chain.sign(&key(), vec![instruction.into()], at - 1);
    let outcomes = chain.commit_at(at, vec![signed]);
    if outcomes != vec![true] {
        eprintln!(
            "retirement instruction result at {}: {:?}",
            chain.height(),
            chain.committed(chain.height()).block().execution_outputs()
        );
    }
    outcomes == vec![true]
}
fn log(chain: &mut CertifiedTestChain) {
    assert!(commit(
        chain,
        Log::new(iroha_logger::Level::INFO, "retirement fixture work".into())
    ));
}

#[test]
fn governed_exclusion_requires_original_ordered_intent_and_respects_lift_or_reactivation() {
    let mut previous = crate::runtime::release_catalog().unwrap().baseline.clone();
    let id = previous.bindings[0].implementation;
    let disable = SelectionTransitionV1::Restrict {
        definition: id,
        restriction: RestrictionKindV1::Disabled,
    };
    let deactivate = SelectionTransitionV1::Deactivate(id);
    assert!(governed_exclusions(&previous, &[]).is_empty());
    assert!(governed_exclusions(&previous, &[deactivate.clone()]).is_empty());
    assert_eq!(
        governed_exclusions(&previous, &[disable.clone(), deactivate.clone()]),
        BTreeSet::from([id])
    );
    assert!(
        governed_exclusions(
            &previous,
            &[disable.clone(), SelectionTransitionV1::Lift(id)]
        )
        .is_empty()
    );
    assert!(
        governed_exclusions(
            &previous,
            &[
                disable,
                deactivate.clone(),
                SelectionTransitionV1::Activate(id)
            ]
        )
        .is_empty()
    );
    previous
        .restrictions
        .push(iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition: id,
            restriction: RestrictionV1::Disabled(2),
        });
    assert!(
        governed_exclusions(&previous, &[]).is_empty(),
        "old restrictions alone confer no new authorization"
    );
    assert_eq!(
        governed_exclusions(&previous, &[deactivate]),
        BTreeSet::from([id])
    );
}

#[test]
fn retirement_publication_capacity_counts_only_genuinely_new_definitions_before_mutation() {
    let world = World::new();
    crate::runtime::activate_additions_for_testing(&world, additions::RUNTIME_LIFECYCLE.module)
        .unwrap();
    crate::runtime::activate_additions_for_testing(&world, additions::RUNTIME_RETIREMENT.module)
        .unwrap();
    let mut block = world.block();
    let definitions = block.runtime_definitions.iter().count();
    for counter in 0..MAX_RETIREMENT_DEFINITIONS - definitions {
        let id = DefinitionId(Hash::new(counter.to_le_bytes()).into());
        assert!(block.runtime_definitions.get(&id).is_none());
        block.runtime_retirements.insert(
            id,
            RuntimeRetirementStateV1 {
                history_started: 2,
                last_required: Some(2),
                governed_excluded_at: None,
                receipt: None,
            },
        );
    }
    let empty = std::collections::BTreeMap::new();
    check_publication_capacity(&block, block.runtime_selection.get(), &empty).unwrap();
    let (id, descriptor) = block.runtime_definitions.iter().next().unwrap();
    let duplicate = std::collections::BTreeMap::from([(*id, descriptor.clone())]);
    check_publication_capacity(&block, block.runtime_selection.get(), &duplicate).unwrap();
    let (id, descriptor) = crate::runtime::release_catalog()
        .unwrap()
        .definitions
        .iter()
        .find(|(id, _)| {
            block.runtime_definitions.get(id).is_none()
                && block.runtime_retirements.get(id).is_none()
        })
        .unwrap();
    let published = std::collections::BTreeMap::from([(*id, descriptor.clone())]);
    assert!(check_publication_capacity(&block, block.runtime_selection.get(), &published).is_err());
    assert!(
        block.runtime_definitions.get(id).is_none(),
        "rejected preflight cannot publish"
    );
}

#[test]
fn governed_retirement_records_last_use_effective_floor_original_proof_and_permanent_exclusion() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let world = World::new();
            crate::runtime::activate_additions_for_testing(
                &world,
                additions::RUNTIME_LIFECYCLE.module,
            )
            .unwrap();
            let old = world.view().runtime_selection().bindings[0].implementation;
            let mut config = TestChainConfig::new(world, 1_000);
            config.clock_bootstrap_in_genesis = true;
            config.genesis_parameters.push(Parameter::Custom(
                RuntimeCheckpointPolicyV1 {
                    cadence_ms: 1,
                    retain_all_ms: 10_000,
                    retain_spacing_ms: 0,
                    recovery_window_ms: 1,
                    floor_notice_ms: 0,
                }
                .to_custom_parameter()
                .unwrap(),
            ));
            let mut chain = CertifiedTestChain::start(config).unwrap();
            let change = {
                let view = chain.state().view();
                crate::runtime::release_catalog()
                    .unwrap()
                    .activation(
                        additions::RUNTIME_RETIREMENT.module,
                        view.world().runtime_selection(),
                        &PublishedDefinitions(view.world().runtime_definitions()),
                        chain.network_id(),
                        3,
                        [6; 32],
                    )
                    .unwrap()
            };
            let (never_active, never_active_descriptor) = {
                let view = chain.state().view();
                crate::runtime::release_catalog()
                    .unwrap()
                    .definitions
                    .iter()
                    .find(|(id, descriptor)| {
                        matches!(descriptor, DefinitionDescriptorV1::Implementation(_))
                            && view.world().runtime_definitions().get(id).is_none()
                            && !change
                                .publish
                                .iter()
                                .any(|value| value.id().unwrap() == **id)
                    })
                    .map(|(id, descriptor)| (*id, descriptor.clone()))
                    .expect("an independently compiled, never-selected native definition")
            };
            let committee: Vec<_> = fixture_keys()
                .into_iter()
                .map(|key| key.public_key().clone())
                .collect();
            let mut record = ChangeSetRecordV1 {
                change_set: change.clone(),
                status: ChangeSetStatusV1::Scheduled,
                proposed_ms: 0,
                approved_height: 1,
                approved_ms: 0,
                notice: RuntimeNoticeV1 {
                    blocks: 1,
                    ms: 0,
                    approval_cushion_ms: 0,
                },
                attestations: Vec::new(),
            };
            for validator in fixture_keys() {
                record_attestation(
                    &mut record,
                    SupportAttestationV1 {
                        network: chain.network_id(),
                        root: change.root,
                        change_set: change.id().unwrap(),
                        predecessor: change.predecessor,
                        successor: change.successor,
                        committee: committee_digest(&committee).unwrap(),
                        tested_checkpoint: 1,
                        valid_from: 0,
                        valid_until: u64::MAX,
                        release: [7; 32],
                        validator: validator.public_key().clone(),
                    }
                    .sign(validator.private_key())
                    .unwrap(),
                );
            }
            chain.setup_world_at(2_000, |tx| {
                tx.world
                    .runtime_definitions
                    .insert(never_active, never_active_descriptor);
                tx.world
                    .runtime_change_sets
                    .insert(change.id().unwrap(), record);
            });
            log(&mut chain); // H2: original service, scheduled authorized transition retained.
            log(&mut chain); // H3: real activation seeds the complete conservative census.
            assert!(enabled(chain.state().view().world().runtime_selection()).unwrap());
            {
                let view = chain.state().view();
                // The first census includes inactive and disabled immutable definitions,
                // not merely the new active closure. No earlier use can become absence.
                for (id, _) in view.world().runtime_definitions().iter() {
                    let row = view.world().runtime_retirements().get(id).unwrap();
                    assert_eq!(row.history_started, 3);
                    assert_eq!(row.last_required, Some(3));
                    assert!(row.receipt.is_none());
                }
            }
            let last = chain
                .state()
                .view()
                .world()
                .runtime_retirements()
                .get(&old)
                .unwrap()
                .last_required;
            assert_eq!(last, Some(3));
            assert_eq!(
                chain
                    .state()
                    .view()
                    .world()
                    .runtime_retirements()
                    .get(&old)
                    .unwrap()
                    .governed_excluded_at,
                Some(3),
                "the successful governed Disable/Deactivate survives removal of its restriction"
            );
            log(&mut chain); // H4: disabled metadata must stop advancing the old code high water.
            assert_eq!(
                chain
                    .state()
                    .view()
                    .world()
                    .runtime_retirements()
                    .get(&old)
                    .unwrap()
                    .last_required,
                last
            );
            assert!(!commit(
                &mut chain,
                RetireRuntimeDefinitionV1 { definition: old }
            ));
            let floor_block = chain.committed(4);
            let floor = StartupFloorV1 {
                network: chain.network_id(),
                height: 4,
                block_hash: *floor_block.block_hash().as_ref(),
                committed_ms: floor_block.block_time_ms(),
            };
            let mut attestations: Vec<_> = fixture_keys()
                .into_iter()
                .map(|validator| {
                    StartupFloorAttestationV1 {
                        floor: floor.id().unwrap(),
                        validator: validator.public_key().clone(),
                    }
                    .sign(validator.private_key())
                    .unwrap()
                })
                .collect();
            attestations.sort_by(|left, right| {
                left.attestation.validator.cmp(&right.attestation.validator)
            });
            assert!(commit(
                &mut chain,
                RaiseRuntimeStartupFloorV1 {
                    floor,
                    attestations
                }
            ));
            assert!(!commit(
                &mut chain,
                RetireRuntimeDefinitionV1 {
                    definition: never_active
                }
            ));
            let unapproved = chain
                .state()
                .view()
                .world()
                .runtime_retirements()
                .get(&never_active)
                .unwrap()
                .clone();
            assert_eq!(unapproved.governed_excluded_at, None);
            assert!(
                unapproved.receipt.is_none(),
                "publication and inactivity cannot authorize censorship"
            );
            assert!(commit(
                &mut chain,
                RetireRuntimeDefinitionV1 { definition: old }
            ));
            let first = {
                let view = chain.state().view();
                let proof = crate::runtime::retirement::receipt_proof(&view, old)
                    .unwrap()
                    .unwrap();
                let coverage =
                    crate::runtime::retirement::verify_coverage(&view, old, &proof, &mut |_| {
                        panic!("no registered child root")
                    })
                    .unwrap();
                assert_eq!(coverage.definition(), old);
                assert_eq!(coverage.roots(), 1);
                assert_ne!(coverage.evidence(), Hash::new([]));
                let mut missing_alias = None;
                let code_coverage = crate::runtime::retirement::verify_native_code_coverage(
                    &view,
                    old,
                    &mut |alias, root| {
                        assert_eq!(root, proof.receipt.root);
                        if alias == old {
                            Ok(Some(proof.clone()))
                        } else {
                            missing_alias = Some(alias);
                            Ok(None)
                        }
                    },
                );
                assert!(code_coverage.is_err());
                let missing_alias = missing_alias.expect("shared native alias must block removal");
                let catalog = crate::runtime::release_catalog().unwrap();
                let DefinitionDescriptorV1::Implementation(old_implementation) =
                    &catalog.definitions[&old]
                else {
                    panic!("fixture target is native");
                };
                let DefinitionDescriptorV1::Implementation(other) =
                    &catalog.definitions[&missing_alias]
                else {
                    panic!("missing alias is native");
                };
                assert_eq!(old_implementation.code, other.code);
                assert_eq!(proof.receipt.last_required, Some(3));
                assert_eq!(proof.receipt.startup_floor, floor);
                validate_root(view.world(), proof.receipt.root, chain.height()).unwrap();
                let mut foreign = proof.receipt.root;
                foreign.instance[0] ^= 1;
                assert!(validate_root(view.world(), foreign, chain.height()).is_err());
                let mut lifted = view.world().runtime_selection().clone();
                lifted.active.push(old);
                lifted.active.sort_unstable();
                assert!(
                    check_transition(view.world(), view.world().runtime_selection(), &lifted)
                        .is_err()
                );
                proof
            };
            assert!(commit(
                &mut chain,
                RetireRuntimeDefinitionV1 { definition: old }
            ));
            let view = chain.state().view();
            let refreshed = crate::runtime::retirement::receipt_proof(&view, old)
                .unwrap()
                .unwrap();
            assert_eq!(refreshed.receipt.excluded_at, first.receipt.excluded_at);
            assert_eq!(refreshed.receipt.last_required, first.receipt.last_required);
            assert!(refreshed.receipt.receipt_height > first.receipt.receipt_height);
            assert!(
                crate::runtime::retirement::verify_coverage(
                    &view,
                    old,
                    &first,
                    &mut |_| unreachable!()
                )
                .is_err()
            );
            let mut substituted = refreshed;
            substituted.receipt.startup_floor.block_hash[0] ^= 1;
            assert!(
                crate::runtime::retirement::verify_coverage(
                    &view,
                    old,
                    &substituted,
                    &mut |_| unreachable!()
                )
                .is_err()
            );
            drop(view);
            // Authenticated restore cannot erase a member of the conservative census.
            chain.setup_world_at(20_000, |tx| {
                tx.world.runtime_retirements.remove(old);
            });
            let view = chain.state().view();
            assert!(validate_root(view.world(), first.receipt.root, chain.height()).is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
