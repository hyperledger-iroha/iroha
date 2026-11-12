//! Successor grammar, predecessor byte preservation and stored-proposal exclusion.

use super::*;
use iroha_crypto::HashOf;
use iroha_data_model::{
    isi::InstructionBox,
    transaction::{Executable, ExecutableBatchItem},
};
use iroha_executor_data_model::isi::multisig::{
    MultisigProposalId, MultisigProposalNonce, MultisigProposalState, MultisigPropose,
};
use mv::storage::Storage;
use std::collections::{BTreeMap, BTreeSet};

const PINNED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs/runtime_ironwood_catalog.json"
);

fn catalog_identity() -> String {
    let catalog = super::super::release_catalog().unwrap();
    let extension = catalog.ironwood.as_ref().unwrap();
    let definitions: Vec<_> = extension
        .definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    let bytes = norito::encode_canonical(&definitions).unwrap();
    format!(
        "{{\n  \"note\": \"Ironwood successor only; predecessor identities remain in runtime_baseline_catalog.json. See ironwood_runtime_transition.md.\",\n  \"predecessor\": \"{}\",\n  \"selection\": \"{}\",\n  \"migration\": \"{}\",\n  \"definitions\": {},\n  \"canonical_definitions_hash\": \"{}\"\n}}\n",
        catalog.baseline_id,
        extension.selection.id().unwrap(),
        extension.migration,
        definitions.len(),
        Hash::new(bytes)
    )
}

#[test]
fn ironwood_successor_catalog_is_pinned_separately_from_the_predecessor() {
    assert_eq!(catalog_identity(), std::fs::read_to_string(PINNED).unwrap());
}

#[test]
#[ignore = "explicit maintenance capture of the new immutable Ironwood successor"]
fn regenerate_ironwood_catalog_identity() {
    std::fs::write(PINNED, catalog_identity()).unwrap();
}

fn proposals(instruction: InstructionBox) -> (InstructionBox, MultisigProposalState) {
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x16; 32])));
    let account = iroha_test_samples::ALICE_ID.clone();
    let nonce = MultisigProposalNonce::new([0x17; 32]).unwrap();
    let values = vec![instruction];
    let proposal = MultisigProposalState {
        network_id: network,
        multisig_account_id: account.clone(),
        proposal_nonce: nonce,
        proposal_id: MultisigProposalId::derive(&network, &account, &nonce),
        instructions_hash: HashOf::new(&values),
        transaction_ttl_ms: None,
        instructions: values.clone(),
        proposed_at_ms: 0,
        expires_at_ms: 100,
        approvals: BTreeSet::new(),
    };
    (
        MultisigPropose::new(account, nonce, values, None).into(),
        proposal,
    )
}

#[test]
fn ironwood_grammar_gate_covers_tags_mixed_batches_and_multisig_before_activation() {
    let catalog = super::super::release_catalog().unwrap();
    let (old, new, bootstrap) = super::super::catalog::tests::privacy_resolution_instructions();
    let baseline = &catalog.baseline;
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    admission::instruction(&old, baseline).unwrap();
    for value in [new, bootstrap] {
        // The same canonical bytes are inspected without normalization or replacing the proof.
        let bytes = norito::encode_canonical(&value).unwrap();
        let decoded: InstructionBox = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
        assert!(admission::instruction(&decoded, baseline).is_err());
        admission::instruction(&decoded, successor).unwrap();
        let batch = Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(old.clone()),
                ExecutableBatchItem::Instruction(decoded.clone()),
            ]
            .into(),
        );
        assert!(admission::executable(&batch, baseline).is_err());
        admission::executable(&batch, successor).unwrap();
        let (proposal, _) = proposals(decoded);
        // Custom JSON was opaque to the predecessor wire decoder. Its selected native parser
        // retains the old execution rejection rather than invalidating historical blocks.
        admission::instruction(&proposal, baseline).unwrap();
        admission::instruction(&proposal, successor).unwrap();
    }
}

#[test]
fn ironwood_additions_preserve_the_actual_frozen_protocol_decoder() {
    use iroha_data_model::privacy::{PrivacyProtocolIdV1, PrivacyRuntimeProtocolIdV1};
    use norito::codec::{DecodeAll as _, Encode as _};
    for protocol in PrivacyProtocolIdV1::ALL {
        let bytes = protocol.encode();
        let decoded = PrivacyProtocolIdV1::decode_all(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, protocol);
        assert_eq!(decoded.encode(), bytes);
        let runtime = PrivacyRuntimeProtocolIdV1::from(protocol);
        assert_eq!(
            runtime.encode(),
            bytes,
            "original storage protocol bytes are preserved"
        );
    }
    let extended = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1.encode();
    assert!(PrivacyProtocolIdV1::decode_all(&mut extended.as_slice()).is_err());
    let catalog = super::super::release_catalog().unwrap();
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    for wire in [
        BootstrapPrivacyIronwoodPoolV1::WIRE_ID,
        SubmitPrivacyIronwoodProofV1::WIRE_ID,
        SchedulePrivacyIronwoodLimitsUpdateV1::WIRE_ID,
    ] {
        assert!(
            !catalog
                .baseline
                .bindings
                .iter()
                .any(|binding| binding.wire_id == wire)
        );
        assert!(
            successor
                .bindings
                .iter()
                .any(|binding| binding.wire_id == wire)
        );
    }
    for previous in &catalog.baseline.bindings {
        let next = successor
            .bindings
            .iter()
            .find(|binding| binding.wire_id == previous.wire_id)
            .unwrap();
        let (Descriptor::Interface(before), Descriptor::Interface(after)) = (
            &catalog.definitions[&previous.interface],
            &catalog.definitions[&next.interface],
        ) else {
            panic!("both bindings must name an interface")
        };
        assert_eq!(
            before.payload, after.payload,
            "baseline payload {} stays frozen",
            previous.wire_id
        );
    }
}

#[test]
fn ironwood_proposals_in_current_or_undo_world_are_rejected_without_rewriting_bytes() {
    let (_, new, _) = super::super::catalog::tests::privacy_resolution_instructions();
    let (_, proposal) = proposals(new);
    let bytes = norito::encode_canonical(&proposal).unwrap();
    let key: iroha_model_base::state_path::StatePath = "multisig/proposal/fixture".parse().unwrap();
    for undo in [false, true] {
        let mut world = crate::state::World::default();
        world.smart_contract_state = if undo {
            Storage::from_snapshot_parts(
                BTreeMap::new(),
                BTreeMap::from([(key.clone(), Some(bytes.clone()))]),
            )
        } else {
            BTreeMap::from([(key.clone(), bytes.clone())])
                .into_iter()
                .collect()
        };
        let before = norito::json::to_json(&world.smart_contract_state).unwrap();
        assert!(admission::world(&mut world).is_err());
        assert_eq!(
            norito::json::to_json(&world.smart_contract_state).unwrap(),
            before
        );
        install_for_test(&mut world);
        admission::world(&mut world).unwrap();
    }
}

#[test]
fn ironwood_cutover_requires_its_exact_published_descriptor_and_complete_storage_set() {
    let catalog = super::super::release_catalog().unwrap();
    let extension = catalog.ironwood.as_ref().unwrap();
    assert_eq!(extension.world_storage.len(), 12);
    validate_cutover(
        &catalog.baseline,
        &extension.selection,
        &catalog.definitions,
    )
    .unwrap();
    let mut wrong_services = extension.selection.clone();
    wrong_services.services_profile = catalog.baseline.services_profile;
    // The stored grammar remains recognizable, but this is not the approved first cutover.
    assert!(enabled(&wrong_services));
    assert!(validate_cutover(&catalog.baseline, &wrong_services, &catalog.definitions).is_err());
    let mut unpublished = catalog.definitions.clone();
    unpublished.remove(&extension.migration);
    assert!(validate_cutover(&catalog.baseline, &extension.selection, &unpublished).is_err());
    for id in extension.world_storage.values() {
        let mut incomplete = extension.selection.clone();
        incomplete.installed_storage.retain(|item| item != id);
        assert!(!enabled(&incomplete));
        assert!(validate_cutover(&catalog.baseline, &incomplete, &catalog.definitions).is_err());
    }
    let change = activation_change_set(
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([9; 32]))),
        200,
        [7; 32],
    )
    .unwrap();
    let predecessor_definitions: BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    let applied = change
        .apply(&catalog.baseline, &predecessor_definitions)
        .unwrap();
    assert_eq!(applied.selection, extension.selection);
}

#[test]
fn ironwood_grammar_survives_compatible_services_and_storage_successors() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let extension = catalog.ironwood.as_ref().unwrap();
    let mut next = extension.selection.clone();
    // Grammar is a property of installed immutable codecs, not this services profile identity.
    next.services_profile = catalog.baseline.services_profile;
    assert!(enabled(&next));
    let mut published = catalog.definitions.clone();
    for original in extension.world_storage.values() {
        let Descriptor::Storage(mut descriptor) = published[original].clone() else {
            unreachable!()
        };
        descriptor.invariants = [0x71; 32];
        let descriptor = Descriptor::Storage(descriptor);
        let replacement = descriptor.id().unwrap();
        published.insert(replacement, descriptor);
        next.installed_storage.retain(|id| id != original);
        next.installed_storage.push(replacement);
    }
    next.installed_storage.sort_unstable();
    assert!(enabled_with_definitions(&next, &published));
    validate_cutover(&extension.selection, &next, &published).unwrap();
    // Original selections continue to use their original codec under later catalogs.
    assert!(enabled_with_definitions(&extension.selection, &published));
    assert!(!enabled_with_definitions(&catalog.baseline, &published));
    let id = next.installed_storage.iter().find(|id| {
        matches!(published.get(*id), Some(Descriptor::Storage(storage)) if storage.namespace == "world.smart_contract_state")
    }).copied().unwrap();
    next.installed_storage.retain(|item| *item != id);
    next.installed_storage
        .push(catalog.world_storage["world.smart_contract_state"]);
    next.installed_storage.sort_unstable();
    assert!(!enabled_with_definitions(&next, &published));
    assert!(validate_cutover(&extension.selection, &next, &published).is_err());
}

#[test]
fn ironwood_preflight_reports_exact_observed_and_required_selection() {
    let catalog = crate::runtime::release_catalog().unwrap();
    check_activation_predecessor(&catalog.baseline).unwrap();
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    let error = check_activation_predecessor(successor).unwrap_err();
    assert!(error.contains(&catalog.baseline_id.to_string()));
    assert!(error.contains(&successor.id().unwrap().to_string()));
    let mut changed = catalog.baseline.clone();
    changed.bindings.pop();
    assert!(check_activation_predecessor(&changed).is_err());
}

#[test]
fn ironwood_inactive_diagnostics_identify_the_actual_operation_and_protocol() {
    let catalog = crate::runtime::release_catalog().unwrap();
    for operation in [
        SubmitPrivacyIronwoodProofV1::WIRE_ID,
        SchedulePrivacyIronwoodLimitsUpdateV1::WIRE_ID,
        BootstrapPrivacyIronwoodPoolV1::WIRE_ID,
    ] {
        let error = crate::runtime::ensure_privacy_protocol_storage_v1(
            iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            &catalog.baseline,
            operation,
        )
        .unwrap_err();
        let iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message),
        ) = error
        else {
            panic!("protocol refusal must not call an active wire ID inactive")
        };
        assert!(message.contains(operation));
        assert!(message.contains("Ironwood protocol storage is inactive"));
        assert!(message.contains("ironwood-halo2-actions-v1"));
    }
}

#[test]
fn ironwood_successor_admission_rejects_codec_retirement_before_attestation() {
    use iroha_data_model::runtime::{
        definition::ImplementationCodeV1,
        lifecycle::{
            ChangeSetV1, RestrictionKindV1, RootScopeV1, RuntimeNoticeV1, SelectionTransitionV1,
        },
    };
    use mv::storage::Storage;
    let catalog = crate::runtime::release_catalog().unwrap();
    let extension = catalog.ironwood.as_ref().unwrap();
    let previous = &extension.selection;
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x77; 32])));
    for compatible_storage in [None, Some(true), Some(false)] {
        let mut next = previous.clone();
        let mut publish = Vec::new();
        let Descriptor::ServicesProfile(mut service) =
            catalog.definitions[&previous.services_profile].clone()
        else {
            unreachable!()
        };
        service.services[0].reference_closure = [0x72; 32];
        let service = Descriptor::ServicesProfile(service);
        next.services_profile = service.id().unwrap();
        publish.push(service);
        let old_bundle = previous.bindings[0].implementation;
        let Descriptor::Implementation(mut bundle) = catalog.definitions[&old_bundle].clone()
        else {
            unreachable!()
        };
        bundle.services_profile = next.services_profile;
        bundle.code = ImplementationCodeV1::Native {
            module: "fixture.ironwood.successor".into(),
            reference_closure: [0x73; 32],
        };
        let mut transitions = vec![
            SelectionTransitionV1::Restrict {
                definition: old_bundle,
                restriction: RestrictionKindV1::Disabled,
            },
            SelectionTransitionV1::Deactivate(old_bundle),
        ];
        if let Some(compatible) = compatible_storage {
            let old = extension.world_storage["world.smart_contract_state"];
            let template = if compatible {
                old
            } else {
                catalog.world_storage["world.smart_contract_state"]
            };
            let Descriptor::Storage(mut storage) = catalog.definitions[&template].clone() else {
                unreachable!()
            };
            storage.invariants = [0x74; 32];
            let storage = Descriptor::Storage(storage);
            let id = storage.id().unwrap();
            publish.push(storage);
            next.installed_storage.retain(|value| *value != old);
            next.installed_storage.push(id);
            next.installed_storage.sort_unstable();
            for accesses in [&mut bundle.reads, &mut bundle.writes, &mut bundle.scans] {
                for value in accesses.iter_mut() {
                    if *value == old {
                        *value = id;
                    }
                }
                accesses.sort_unstable();
            }
            transitions.extend([
                SelectionTransitionV1::RetireStorage(old),
                SelectionTransitionV1::InstallStorage(id),
            ]);
        }
        let bundle = Descriptor::Implementation(bundle);
        let id = bundle.id().unwrap();
        publish.push(bundle);
        next.active.retain(|value| *value != old_bundle);
        next.active.push(id);
        next.active.sort_unstable();
        // Deactivation removes the temporary disable restriction as well as the active ID.
        next.restrictions
            .retain(|entry| entry.definition != old_bundle);
        for binding in &mut next.bindings {
            binding.implementation = id;
        }
        transitions.extend([
            SelectionTransitionV1::SetServicesProfile(next.services_profile),
            SelectionTransitionV1::Activate(id),
        ]);
        transitions.extend(
            next.bindings
                .iter()
                .cloned()
                .map(SelectionTransitionV1::Bind),
        );
        publish.sort_by_key(|descriptor| descriptor.id().unwrap());
        let change = ChangeSetV1 {
            network,
            root: RootScopeV1::Universal,
            predecessor: previous.id().unwrap(),
            successor: next.id().unwrap(),
            effective_height: 200,
            publish,
            transitions,
            qualification: [0x76; 32],
        };
        let applied = change
            .apply(previous, &catalog.definitions)
            .expect("generic closure accepts this future definition set");
        assert_eq!(applied.selection, next);
        let storage: Storage<_, _> = catalog
            .definitions
            .iter()
            .map(|(id, value)| (*id, value.clone()))
            .collect();
        let result = crate::runtime::change_set::admit(
            &change,
            &network,
            100,
            RuntimeNoticeV1 {
                blocks: 1,
                ms: 0,
                approval_cushion_ms: 0,
            },
            previous,
            &BTreeMap::new(),
            &crate::runtime::change_set::PublishedDefinitions(&storage.view()),
        );
        // The scheduled predecessor is checked under the same transition support
        // rules as the committed predecessor; projection does not implement migration.
        let activation = super::activation_change_set(network, 150, [0x78; 32]).unwrap();
        // Compiled support is not committed publication: these definitions are first
        // published by the scheduled activation when projecting the future predecessor.
        let activation_ids: std::collections::BTreeSet<_> = activation
            .publish
            .iter()
            .map(|definition| definition.id().unwrap())
            .collect();
        let preactivation_storage: Storage<_, _> = catalog
            .definitions
            .iter()
            .filter(|(id, _)| !activation_ids.contains(id))
            .map(|(id, value)| (*id, value.clone()))
            .collect();
        let scheduled = BTreeMap::from([(
            activation.id().unwrap(),
            iroha_data_model::runtime::lifecycle::ChangeSetRecordV1 {
                change_set: activation,
                status: iroha_data_model::runtime::lifecycle::ChangeSetStatusV1::Scheduled,
                proposed_ms: 0,
                approved_height: 90,
                approved_ms: 0,
                notice: RuntimeNoticeV1 {
                    blocks: 1,
                    ms: 0,
                    approval_cushion_ms: 0,
                },
                attestations: vec![],
            },
        )]);
        let projected_result = crate::runtime::change_set::admit(
            &change,
            &network,
            100,
            RuntimeNoticeV1 {
                blocks: 1,
                ms: 0,
                approval_cushion_ms: 0,
            },
            &catalog.baseline,
            &scheduled,
            &crate::runtime::change_set::PublishedDefinitions(&preactivation_storage.view()),
        );
        if compatible_storage.is_some() {
            for refusal in [result, projected_result] {
                let error = format!("{:?}", refusal.unwrap_err());
                assert!(
                    error.contains("unimplemented storage migration cutover")
                        || (compatible_storage == Some(false) && error.contains("Ironwood")),
                    "an unimplemented retirement must refuse: {error}"
                );
            }
        } else {
            result.expect("a service successor without storage retirement is admissible");
            projected_result.expect("projection retains the same supported service transition");
            let mut definitions = catalog.definitions.clone();
            definitions.extend(applied.published);
            assert!(enabled_with_definitions(&next, &definitions));
            validate_cutover(previous, &next, &definitions).unwrap();
        }
    }
}

#[test]
fn ironwood_compiled_owner_index_matches_archived_contract_classification() {
    let catalog = crate::runtime::release_catalog().unwrap();
    for selection in [
        &catalog.baseline,
        &catalog.ironwood.as_ref().unwrap().selection,
    ] {
        assert_eq!(
            enabled(selection),
            enabled_with_definitions(selection, &catalog.definitions)
        );
        let mut duplicate = selection.clone();
        duplicate
            .installed_storage
            .push(catalog.world_storage["world.smart_contract_state"]);
        duplicate.installed_storage.sort_unstable();
        duplicate.installed_storage.dedup();
        assert_eq!(
            enabled(&duplicate),
            enabled_with_definitions(&duplicate, &catalog.definitions)
        );
    }
    let mut next = catalog.ironwood.as_ref().unwrap().selection.clone();
    let mut definitions = catalog.definitions.clone();
    let old = catalog.ironwood.as_ref().unwrap().world_storage["world.smart_contract_state"];
    let Descriptor::Storage(mut future) = definitions[&old].clone() else {
        unreachable!()
    };
    future.value = super::extended_codec(&future.value).unwrap();
    let future = Descriptor::Storage(future);
    let id = future.id().unwrap();
    definitions.insert(id, future);
    next.installed_storage.retain(|candidate| *candidate != old);
    next.installed_storage.push(id);
    next.installed_storage.sort_unstable();
    validate_admitted_successor(&next, &definitions).expect(
        "future codec publication is allowed; activation still requires implementation support",
    );
    assert!(
        !enabled(&next),
        "this release must not claim an unknown codec implementation"
    );
}

#[test]
fn ironwood_compiled_storage_validation_cache_hits_and_bounds_memory() {
    let catalog = super::super::release_catalog().unwrap();
    let mut cache = super::StorageValidationCache::default();
    let selection = &catalog.ironwood.as_ref().unwrap().selection;
    let mut checks = 0;
    for _ in 0..3 {
        cache
            .validate(selection, || {
                checks += 1;
                super::validate_cutover(selection, selection, &catalog.definitions)
            })
            .unwrap();
    }
    assert_eq!(
        checks, 1,
        "all three accumulator passes share one immutable validation"
    );
    for i in 0..70 {
        let mut selection = selection.clone();
        selection.services_profile = DefinitionId([i; 32]);
        assert!(
            cache
                .validate(&selection, || Err("test refusal".into()))
                .is_err()
        );
    }
    assert_eq!(cache.0.len(), 64);
}

#[test]
fn ironwood_native_schema_family_matches_portable_client_policy() {
    let native = crate::state::State::native_world_schema_hashes_v1().unwrap();
    assert_eq!(
        native,
        iroha_data_model::sumeragi_finality::native_world_schema_family_v1()
    );
    for schema in native {
        assert_eq!(
            crate::state::State::native_world_schemas_for_anchor_v1(schema).unwrap(),
            iroha_data_model::sumeragi_finality::native_world_schemas_for_anchor_v1(schema)
        );
    }
}

/// Use the actual merged Time service and current-selection constructor.
const TIME_SERVICE_CLOSURE: &[u8] = iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE;

fn time_service_successor(
    catalog: &mut ReleaseCatalog,
    previous: &RootSelectionV1,
) -> RootSelectionV1 {
    catalog.lifecycle_successor(previous).unwrap()
}

/// `selection_change_set` gained `Unbind` for the Taira activation, whose predecessor binds a wire
/// its successor drops. No other compiled change set drops one, so its transitions, and with them
/// its identity, are unchanged. The pinned identities catch any later change to either.
#[test]
fn only_the_taira_activation_unbinds_a_wire() {
    let base = super::super::release_catalog().unwrap();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"composition")));
    let baseline_published: BTreeMap<_, _> = base
        .baseline_definitions
        .iter()
        .map(|id| (*id, base.definitions[id].clone()))
        .collect();
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    let time_published: BTreeMap<_, _> = catalog
        .definitions
        .iter()
        .filter(|(id, _)| {
            base.baseline_definitions.contains(id)
                || **id == time.services_profile
                || **id == time.bindings[0].implementation
        })
        .map(|(id, d)| (*id, d.clone()))
        .collect();
    let ironwood = activation_change_set(network, 90, [6; 32]).unwrap();
    let ironwood_after_time =
        activation_change_set_for_selection(network, 100, [7; 32], &time, &time_published).unwrap();
    let lifecycle = super::super::lifecycle_activation::from_selection(
        base.clone(),
        &base.baseline,
        &baseline_published,
        network,
        90,
        [8; 32],
    )
    .unwrap();
    let changes = [&ironwood, &ironwood_after_time, &lifecycle];
    for change in changes {
        assert!(
            !change
                .transitions
                .iter()
                .any(|transition| matches!(transition, SelectionTransitionV1::Unbind(_))),
            "every other compiled change set keeps its bindings",
        );
    }
    assert_eq!(
        changes.map(|change| change.id().unwrap().to_string()),
        PINNED_CHANGE_SET_IDS,
    );
}

/// Ironwood, Ironwood after Time, and lifecycle, in the order the test computes them. Each
/// publishes only what its successor relies on, so a dormant addition the release also compiles
/// moves none of them.
const PINNED_CHANGE_SET_IDS: [&str; 3] = [
    "1e4c77b2821cea5891c3b5738dd9aee94ee28a4fc1a77795c50fbd08df7f972b",
    "d33f4c93aa85214e3d08345f6c4dfe74013b3e417f8528e1667c11a42a791889",
    "9f6b930d1c808c44f48b007de7f884db20de35aa889340ce944a39009de68573",
];

#[test]
fn ironwood_and_time_service_closure_compose_in_both_current_selection_orders() {
    let base = super::super::release_catalog().unwrap();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"composition")));
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    // Only definitions actually published by the predecessor release are on chain.
    let published: BTreeMap<_, _> = catalog
        .definitions
        .iter()
        .filter(|(id, _)| {
            base.baseline_definitions.contains(id)
                || **id == time.services_profile
                || **id == time.bindings[0].implementation
        })
        .map(|(id, d)| (*id, d.clone()))
        .collect();
    let change =
        activation_change_set_for_selection(network, 100, [7; 32], &time, &published).unwrap();
    // An otherwise identical release without these two descriptors must refuse it.
    let combined = catalog
        .lifecycle_successor(&base.ironwood.as_ref().unwrap().selection)
        .unwrap();
    catalog.definitions.remove(&combined.services_profile);
    catalog
        .definitions
        .remove(&combined.bindings[0].implementation);
    let uncomposed_catalog = catalog.clone();
    register_service_compositions(&mut catalog, std::slice::from_ref(&time)).unwrap();
    // On-chain publication must never manufacture support in the compiled release.
    let compiled_catalog = catalog;
    assert_eq!(change.predecessor, time.id().unwrap());
    assert_ne!(change.predecessor, base.baseline_id);
    let applied = change.apply(&time, &published).unwrap();
    let mut time_published = published;
    time_published.extend(applied.published);
    let time_then_ironwood = applied.selection;
    assert!(
        compiled_catalog.unsupported(&time_then_ironwood).is_none(),
        "merged catalog must register the actual composition before attestations"
    );
    validate_cutover(&time, &time_then_ironwood, &time_published).unwrap();
    let ironwood = &base.ironwood.as_ref().unwrap().selection;
    let first = activation_change_set(network, 90, [6; 32]).unwrap();
    let baseline_published: BTreeMap<_, _> = base
        .baseline_definitions
        .iter()
        .map(|id| (*id, base.definitions[id].clone()))
        .collect();
    let applied_first = first.apply(&base.baseline, &baseline_published).unwrap();
    assert_eq!(&applied_first.selection, ironwood);
    let mut ironwood_published = baseline_published;
    ironwood_published.extend(applied_first.published);
    let reverse = service_change_set_in_catalog(
        &compiled_catalog,
        network,
        101,
        [8; 32],
        ironwood,
        &time,
        &ironwood_published,
    )
    .unwrap();
    assert_eq!(reverse.predecessor, ironwood.id().unwrap());
    let reverse = reverse.apply(ironwood, &ironwood_published).unwrap();
    ironwood_published.extend(reverse.published);
    let ironwood_then_time = reverse.selection;
    validate_admitted_successor(&ironwood_then_time, &ironwood_published).unwrap();
    validate_cutover(ironwood, &ironwood_then_time, &ironwood_published).unwrap();
    assert_eq!(
        time_then_ironwood.id().unwrap(),
        ironwood_then_time.id().unwrap(),
        "both orders reach the same content-addressed composition"
    );
    assert_eq!(
        time_then_ironwood.services_profile,
        ironwood_then_time.services_profile
    );
    assert_eq!(
        time_then_ironwood.installed_storage,
        ironwood_then_time.installed_storage
    );
    assert!(enabled(&time_then_ironwood) && enabled(&ironwood_then_time));
    for (selection, published) in [
        (&time_then_ironwood, &time_published),
        (&ironwood_then_time, &ironwood_published),
    ] {
        selection.check_closure(published).unwrap();
        selection
            .check_closure(&compiled_catalog.definitions)
            .unwrap();
        assert!(compiled_catalog.unsupported(selection).is_none());
        assert!(
            uncomposed_catalog.unsupported(selection).is_some(),
            "publication alone cannot qualify an unregistered composition"
        );
        assert!(
            compiled_catalog
                .service_enabled(
                    selection.services_profile,
                    "trigger_lifecycle",
                    Hash::new(TIME_SERVICE_CLOSURE).into(),
                )
                .unwrap(),
            "dispatch and capacity-attestation predicate must retain Time in both orders"
        );
        for field in PRIVACY_FIELDS {
            assert!(
                selection
                    .installed_storage
                    .contains(&base.ironwood.as_ref().unwrap().world_storage[field])
            );
        }
    }
    assert_eq!(base.baseline.id().unwrap(), base.baseline_id);
    for id in &base.baseline_definitions {
        assert_eq!(compiled_catalog.definitions[id], base.definitions[id]);
    }
}

#[test]
fn ironwood_composition_constructors_refuse_restricted_replacements_without_changing_predecessor() {
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let catalog = super::super::release_catalog().unwrap();
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"restricted composition",
    )));
    for add_ironwood in [true, false] {
        let original = if add_ironwood {
            catalog.lifecycle.clone()
        } else {
            catalog.ironwood.as_ref().unwrap().selection.clone()
        };
        let mut published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        if add_ironwood {
            for id in [
                original.services_profile,
                original.bindings[0].implementation,
            ] {
                published.insert(id, catalog.definitions[&id].clone());
            }
        } else {
            for id in &catalog.ironwood.as_ref().unwrap().definitions {
                published.insert(*id, catalog.definitions[id].clone());
            }
        }
        // Restrictions target installed storage or active definitions. A services
        // profile is retained by the selection but cannot itself be restricted.
        let mut replaced = vec![original.bindings[0].implementation];
        if add_ironwood {
            replaced.extend([
                original.bindings[0].interface,
                catalog.world_storage["triggers.time"],
            ]);
        }
        for definition in replaced {
            let mut previous = original.clone();
            previous.restrictions = vec![RestrictionEntryV1 {
                definition,
                restriction: RestrictionV1::AdmissionClosed(10_000),
            }];
            previous.check_closure(&published).unwrap();
            let original_wire = norito::encode_canonical(&previous).unwrap();
            let original_definitions = published.clone();
            let result = if add_ironwood {
                activation_change_set_for_selection(network, 100, [7; 32], &previous, &published)
            } else {
                service_change_set_for_selection(
                    network,
                    100,
                    [8; 32],
                    &previous,
                    &catalog.lifecycle,
                    &published,
                )
            };
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(&format!(
                    "composition cannot replace restricted definition {definition}"
                )),
                "{error}"
            );
            assert_eq!(norito::encode_canonical(&previous).unwrap(), original_wire);
            assert_eq!(published, original_definitions);
        }
    }
}

#[test]
fn ironwood_composition_constructors_preserve_retained_restrictions_and_future_heights() {
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let catalog = super::super::release_catalog().unwrap();
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"retained composition restriction",
    )));
    for add_ironwood in [true, false] {
        let mut previous = if add_ironwood {
            catalog.lifecycle.clone()
        } else {
            catalog.ironwood.as_ref().unwrap().selection.clone()
        };
        let mut published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        if add_ironwood {
            for id in [
                previous.services_profile,
                previous.bindings[0].implementation,
            ] {
                published.insert(id, catalog.definitions[&id].clone());
            }
        } else {
            for id in &catalog.ironwood.as_ref().unwrap().definitions {
                published.insert(*id, catalog.definitions[id].clone());
            }
        }
        previous.restrictions = vec![RestrictionEntryV1 {
            definition: catalog.world_storage["world.accounts"],
            restriction: RestrictionV1::CompletionOnly(10_000),
        }];
        previous.check_closure(&published).unwrap();
        let original_wire = norito::encode_canonical(&previous).unwrap();
        let original_definitions = published.clone();
        let change = if add_ironwood {
            activation_change_set_for_selection(network, 100, [7; 32], &previous, &published)
        } else {
            service_change_set_for_selection(
                network,
                100,
                [8; 32],
                &previous,
                &catalog.lifecycle,
                &published,
            )
        }
        .unwrap();
        let applied = change.apply(&previous, &published).unwrap();
        assert_eq!(applied.selection.restrictions, previous.restrictions);
        assert_eq!(
            applied.selection.completion_closures,
            previous.completion_closures
        );
        assert_eq!(norito::encode_canonical(&previous).unwrap(), original_wire);
        assert_eq!(published, original_definitions);
        assert!(catalog.unsupported(&applied.selection).is_none());
        assert!(super::super::trigger_lifecycle_enabled(&applied.selection).unwrap());
        assert!(enabled(&applied.selection));
    }
}

#[test]
fn ironwood_composed_service_dispatch_refuses_unknown_profiles_and_changed_closures() {
    let base = super::super::release_catalog().unwrap();
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    let required = Hash::new(TIME_SERVICE_CLOSURE).into();
    for selection in [&base.baseline, &base.ironwood.as_ref().unwrap().selection] {
        assert!(
            !catalog
                .service_enabled(selection.services_profile, "trigger_lifecycle", required)
                .unwrap(),
            "supported baseline and Ironwood-only profiles retain predecessor Time semantics"
        );
    }
    assert!(
        catalog
            .service_enabled(time.services_profile, "trigger_lifecycle", required)
            .unwrap()
    );
    let mut without_time = base.clone();
    without_time.definitions.remove(&time.services_profile);
    assert!(
        without_time
            .service_enabled(time.services_profile, "trigger_lifecycle", required)
            .is_err(),
        "an unmerged release must not advertise the Time implementation"
    );
    assert!(
        base.service_enabled(time.services_profile, "trigger_lifecycle", required)
            .unwrap()
    );
    assert!(
        catalog
            .service_enabled(base.baseline.kernel, "trigger_lifecycle", required)
            .is_err(),
        "an unrelated compiled identity is not a services profile"
    );
    let Descriptor::ServicesProfile(mut changed) =
        catalog.definitions[&time.services_profile].clone()
    else {
        unreachable!()
    };
    changed
        .services
        .iter_mut()
        .find(|entry| entry.name == "trigger_lifecycle")
        .unwrap()
        .reference_closure = Hash::new(b"different lifecycle semantics").into();
    let changed = catalog
        .publish(Descriptor::ServicesProfile(changed))
        .unwrap();
    assert!(
        catalog
            .service_enabled(changed, "trigger_lifecycle", required)
            .is_err(),
        "a future compiled profile cannot silently use the old service implementation"
    );
}

#[test]
fn ironwood_restore_checks_immutable_storage_preimages_even_after_cache_hit() {
    let catalog = super::super::release_catalog().unwrap();
    let selection = &catalog.ironwood.as_ref().unwrap().selection;
    validate_compiled_storage_selection(selection).unwrap();
    let mut world = crate::state::World::new();
    install_for_test(&mut world);
    super::super::validate_privacy_world_storage_v1(&mut world).unwrap();
    let selected = catalog.ironwood.as_ref().unwrap().world_storage["world.privacy_roots"];
    world.runtime_definitions.insert(
        selected,
        catalog.definitions[&catalog.world_storage["world.privacy_roots"]].clone(),
    );
    assert!(
        super::super::validate_privacy_world_storage_v1(&mut world)
            .unwrap_err()
            .contains("immutable compiled identity")
    );
}

#[test]
fn ironwood_current_selection_authentication_binds_the_actual_certified_preimage() {
    use iroha_data_model::sumeragi_finality::{
        WorldStateElementKindV1, WorldStateSnapshotEntryV1, WorldStateSnapshotV1,
        test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    };
    let catalog = super::super::release_catalog().unwrap();
    let mut modeled = catalog.clone();
    let time = time_service_successor(&mut modeled, &catalog.baseline);
    let snapshot = WorldStateSnapshotV1 {
        schema_hash: crate::state::State::native_world_schema_hash_v1().unwrap(),
        entries: vec![WorldStateSnapshotEntryV1 {
            field_id: "world.runtime_selection".into(),
            kind: WorldStateElementKindV1::Cell,
            key_hash: None,
            value_hash: world_state_value_hash_v1(&time).unwrap(),
        }],
    };
    let mut native = NativeFinalityFixture::new();
    let block = native.block_with_submitted_work(native.next_header());
    let proof = native.certify_with_world_root(block, snapshot.root().unwrap());
    let verified = native.verifier().verify_retained_decision(&proof).unwrap();
    let world = snapshot.authenticate(&verified).unwrap();
    authenticate_runtime_selection_at(&world, &time).unwrap();
    assert!(authenticate_runtime_selection_at(&world, &catalog.baseline).is_err());
    assert!(
        check_activation_predecessor_at(&world, &time).is_err(),
        "the separately named baseline-only comparator stays exact"
    );
}

#[test]
fn ironwood_service_composition_ignores_non_native_references() {
    let base = super::super::release_catalog().unwrap();
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    let Descriptor::Implementation(mut ivm) =
        catalog.definitions[&time.bindings[0].implementation].clone()
    else {
        unreachable!()
    };
    ivm.code = ImplementationCodeV1::Ivm {
        code_hash: [7; 32],
        entrypoint: "main".into(),
    };
    ivm.host_profile = Some(
        *catalog
            .definitions
            .iter()
            .find_map(|(id, definition)| {
                matches!(definition, Descriptor::HostProfile(_)).then_some(id)
            })
            .unwrap(),
    );
    // Use a well-formed IVM definition: unrelated valid catalog entries must not be
    // composed or advertised as native service implementations.
    let reference_id = catalog.publish(Descriptor::Implementation(ivm)).unwrap();
    let mut reference = time.clone();
    reference
        .active
        .retain(|id| *id != time.bindings[0].implementation);
    reference.active.push(reference_id);
    reference.active.sort_unstable();
    for binding in &mut reference.bindings {
        binding.implementation = reference_id;
    }
    register_service_compositions(&mut catalog, std::slice::from_ref(&time)).unwrap();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"non-native")));
    assert!(
        service_change_set_in_catalog(
            &catalog,
            network,
            100,
            [7; 32],
            &base.baseline,
            &reference,
            &catalog.definitions
        )
        .is_err()
    );
}

#[test]
fn ironwood_service_composition_refuses_a_valid_selection_without_bindings() {
    let base = super::super::release_catalog().unwrap();
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    register_service_compositions(&mut catalog, std::slice::from_ref(&time)).unwrap();
    let mut previous = base.baseline.clone();
    previous.active.clear();
    previous.bindings.clear();
    previous.check_closure(&catalog.definitions).unwrap();
    assert!(catalog.unsupported(&previous).is_none());
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"empty-bindings")));
    let error = service_change_set_in_catalog(
        &catalog,
        network,
        100,
        [7; 32],
        &previous,
        &time,
        &catalog.definitions,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("requires an active native bundle")
    );
}

#[test]
fn ironwood_catalog_skips_non_additive_native_service_candidates() {
    let mut catalog = super::super::release_catalog().unwrap().clone();
    let baseline = catalog.baseline.clone();
    let mut reference = time_service_successor(&mut catalog, &baseline);
    let Descriptor::ServicesProfile(mut services) =
        catalog.definitions[&reference.services_profile].clone()
    else {
        unreachable!()
    };
    services.services.remove(0);
    let services = catalog
        .publish(Descriptor::ServicesProfile(services))
        .unwrap();
    let Descriptor::Implementation(mut bundle) =
        catalog.definitions[&reference.bindings[0].implementation].clone()
    else {
        unreachable!()
    };
    bundle.services_profile = services;
    let implementation = catalog.publish(Descriptor::Implementation(bundle)).unwrap();
    reference.services_profile = services;
    let old = reference.bindings[0].implementation;
    reference.active.retain(|id| *id != old);
    reference.active.push(implementation);
    reference.active.sort_unstable();
    for binding in &mut reference.bindings {
        binding.implementation = implementation;
    }
    // A non-additive candidate is neither fatal nor advertised as an Ironwood composition.
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"skip-non-additive",
    )));
    assert!(
        activation_change_set_in_catalog(
            &catalog,
            network,
            100,
            [7; 32],
            &reference,
            &catalog.definitions
        )
        .is_err()
    );
    register_service_compositions(&mut catalog, std::slice::from_ref(&reference)).unwrap();
    assert!(
        activation_change_set_in_catalog(
            &catalog,
            network,
            100,
            [7; 32],
            &reference,
            &catalog.definitions
        )
        .is_err()
    );
}

#[test]
fn ironwood_service_reference_refuses_extra_active_and_storage_effects() {
    let base = super::super::release_catalog().unwrap();
    let mut catalog = base.clone();
    let time = time_service_successor(&mut catalog, &base.baseline);
    register_service_compositions(&mut catalog, std::slice::from_ref(&time)).unwrap();
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"service-only")));
    for storage in [false, true] {
        let mut mixed = time.clone();
        if storage {
            let Descriptor::Storage(mut extra) =
                catalog.definitions[&base.world_storage["world.smart_contract_state"]].clone()
            else {
                unreachable!()
            };
            // Keep every dependency installed and add a distinct, valid namespace.
            extra.namespace.push_str(".unrelated");
            let extra = catalog.publish(Descriptor::Storage(extra)).unwrap();
            mixed.installed_storage.push(extra);
            mixed.installed_storage.sort_unstable();
        } else {
            let Descriptor::Implementation(mut extra) =
                catalog.definitions[&time.bindings[0].implementation].clone()
            else {
                unreachable!()
            };
            // An extra unbound bundle has the same services and satisfied dependencies.
            let ImplementationCodeV1::Native {
                reference_closure, ..
            } = &mut extra.code
            else {
                unreachable!()
            };
            *reference_closure = Hash::new(b"independent-unbound-service").into();
            let extra = catalog.publish(Descriptor::Implementation(extra)).unwrap();
            mixed.active.push(extra);
            mixed.active.sort_unstable();
        }
        mixed.check_closure(&catalog.definitions).unwrap();
        let error = service_change_set_in_catalog(
            &catalog,
            network,
            100,
            [7; 32],
            &base.baseline,
            &mixed,
            &catalog.definitions,
        )
        .unwrap_err();
        let CatalogError::Parameters(message) = error else {
            panic!("closure-valid reference must fail the service-only qualification guard")
        };
        assert_eq!(message, "reference is not a service-only selection");
    }
}
