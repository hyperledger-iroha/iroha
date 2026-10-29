//! Live-upgrade lifecycle: change sets, attestations, containment and expectations.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    runtime::{
        definition::{
            BindingV1, CompletionClosureV1, DefinitionDescriptorV1, DefinitionId, RootSelectionV1,
            SelectionId,
        },
        lifecycle::{
            ChangeSetError, ChangeSetV1, ContainmentError, ContainmentKindV1, ContainmentV1,
            ENDORSEMENT_LIFETIME_MS, EndorsementV1, ExpectationV1, RestrictionKindV1, RootScopeV1,
            RuntimeRejectionV1, SelectionTransitionV1, SupportAttestationV1, committee_digest,
            committee_supports,
        },
    },
};

use crate::runtime_definition::{bond_root, implementation, interface, publish, sorted};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x11; 32]),
    ))
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

/// Build the change set that turns `predecessor` into `successor` with `transitions`.
fn change_set(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    publish: Vec<DefinitionDescriptorV1>,
    transitions: Vec<SelectionTransitionV1>,
) -> ChangeSetV1 {
    ChangeSetV1 {
        network: network(),
        root: RootScopeV1::Universal,
        predecessor: predecessor.id().unwrap(),
        successor: successor.id().unwrap(),
        effective_height: 120,
        publish,
        transitions,
        qualification: [0x5a; 32],
    }
}

/// Add `iroha.bond.call.v1` to the bond root: the selection, the change set and the new ids.
fn add_call() -> (
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    RootSelectionV1,
    RootSelectionV1,
    ChangeSetV1,
) {
    let (catalog, ids, predecessor) = bond_root();
    let call_interface = interface("iroha.bond.call.v1", ids.event);
    let call = call_interface.id().unwrap();
    let call_implementation = implementation(&ids, call, "bond.call");
    let call_impl = call_implementation.id().unwrap();

    let mut successor = predecessor.clone();
    successor.active = sorted([successor.active, vec![call, call_impl]].concat());
    successor.bindings.push(BindingV1 {
        wire_id: "iroha.bond.call.v1".into(),
        interface: call,
        implementation: call_impl,
    });
    successor
        .bindings
        .sort_by(|left, right| left.wire_id.cmp(&right.wire_id));

    let mut publish = vec![(call, call_interface), (call_impl, call_implementation)];
    publish.sort_by_key(|(id, _)| *id);
    let change = change_set(
        &predecessor,
        &successor,
        publish
            .into_iter()
            .map(|(_, descriptor)| descriptor)
            .collect(),
        vec![
            SelectionTransitionV1::Activate(call),
            SelectionTransitionV1::Activate(call_impl),
            SelectionTransitionV1::Bind(BindingV1 {
                wire_id: "iroha.bond.call.v1".into(),
                interface: call,
                implementation: call_impl,
            }),
        ],
    );
    (catalog, predecessor, successor, change)
}

#[test]
fn a_change_set_adds_one_instruction() {
    let (catalog, predecessor, successor, change) = add_call();
    let applied = change.apply(&predecessor, &catalog).unwrap();
    assert_eq!(applied.selection, successor);
    assert_eq!(applied.published.len(), 2);
    assert!(
        applied.published.keys().all(|id| !catalog.contains_key(id)),
        "only the new definitions are published"
    );
}

#[test]
fn the_change_set_must_start_and_end_where_it_says() {
    let (catalog, predecessor, _, change) = add_call();

    let mut stale = change.clone();
    stale.predecessor = SelectionId([0; 32]);
    assert_eq!(
        stale.apply(&predecessor, &catalog),
        Err(ChangeSetError::PredecessorMismatch)
    );

    let mut wrong = change.clone();
    wrong.successor = predecessor.id().unwrap();
    assert_eq!(
        wrong.apply(&predecessor, &catalog),
        Err(ChangeSetError::SuccessorMismatch)
    );

    let mut republished = change;
    republished.publish.push(republished.publish[0].clone());
    assert_eq!(
        republished.apply(&predecessor, &catalog),
        Err(ChangeSetError::NonCanonicalPublication)
    );
}

#[test]
fn disabling_needed_code_waits_for_its_obligations() {
    let (catalog, _, selection) = bond_root();
    let claim_impl = selection.bindings[0].implementation;
    let obligation = [0x0b; 32];

    let mut holding = selection.clone();
    holding.completion_closures = vec![CompletionClosureV1 {
        obligation,
        definitions: vec![claim_impl],
    }];
    let mut disabled = holding.clone();
    disabled.restrictions = vec![iroha_data_model::runtime::definition::RestrictionEntryV1 {
        definition: claim_impl,
        restriction: RestrictionKindV1::Disabled.at(120),
    }];
    let stranding = change_set(
        &holding,
        &disabled,
        Vec::new(),
        vec![SelectionTransitionV1::Restrict {
            definition: claim_impl,
            restriction: RestrictionKindV1::Disabled,
        }],
    );
    assert_eq!(
        stranding.apply(&holding, &catalog),
        Err(ChangeSetError::ObligationStranded(claim_impl))
    );

    let mut drained = disabled.clone();
    drained.completion_closures.clear();
    let release_then_disable = change_set(
        &holding,
        &drained,
        Vec::new(),
        vec![
            SelectionTransitionV1::ReleaseCompletion(obligation),
            SelectionTransitionV1::Restrict {
                definition: claim_impl,
                restriction: RestrictionKindV1::Disabled,
            },
        ],
    );
    assert_eq!(
        release_then_disable
            .apply(&holding, &catalog)
            .unwrap()
            .selection,
        drained
    );

    let admission_closed = change_set(
        &holding,
        &holding,
        Vec::new(),
        vec![SelectionTransitionV1::Deactivate(claim_impl)],
    );
    assert!(matches!(
        admission_closed.apply(&holding, &catalog),
        Err(ChangeSetError::Transition { .. })
    ));
}

#[test]
fn rebinding_breaks_a_signed_expectation() {
    let (mut catalog, ids, selection) = bond_root();
    let claim = selection
        .active
        .iter()
        .copied()
        .find(|id| matches!(catalog[id], DefinitionDescriptorV1::Interface(_)))
        .unwrap();
    let old_impl = selection.bindings[0].implementation;
    let expectation = ExpectationV1 {
        bindings: vec![BindingV1 {
            wire_id: "iroha.bond.claim.v1".into(),
            interface: claim,
            implementation: old_impl,
        }],
        services_profile: selection.services_profile,
    };
    assert_eq!(expectation.check(&selection), Ok(()));

    let fixed = implementation(&ids, claim, "bond.claim.fixed");
    let fixed_id = publish(&mut catalog, fixed);
    let mut rebound = selection.clone();
    rebound.active = sorted([rebound.active, vec![fixed_id]].concat());
    rebound.bindings[0].implementation = fixed_id;
    let rebind = change_set(
        &selection,
        &rebound,
        Vec::new(),
        vec![
            SelectionTransitionV1::Activate(fixed_id),
            SelectionTransitionV1::Bind(BindingV1 {
                wire_id: "iroha.bond.claim.v1".into(),
                interface: claim,
                implementation: fixed_id,
            }),
        ],
    );
    assert_eq!(
        rebind.apply(&selection, &catalog).unwrap().selection,
        rebound
    );
    assert_eq!(
        expectation.check(&rebound),
        Err(RuntimeRejectionV1::ExpectationMismatch {
            wire_id: Some("iroha.bond.claim.v1".into())
        })
    );
}

fn attestation(
    change: &ChangeSetV1,
    committee: [u8; 32],
    validator: PublicKey,
) -> SupportAttestationV1 {
    SupportAttestationV1 {
        network: network(),
        root: RootScopeV1::Universal,
        change_set: change.id().unwrap(),
        predecessor: change.predecessor,
        successor: change.successor,
        committee,
        tested_checkpoint: 100,
        valid_from: 101,
        valid_until: 200,
        release: [0x2e; 32],
        validator,
    }
}

#[test]
fn activation_needs_every_validator() {
    let (_, _, _, change) = add_call();
    let id = change.id().unwrap();
    let validators: Vec<KeyPair> = (1..=4).map(key).collect();
    let committee: Vec<PublicKey> = validators
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    let digest = committee_digest(&committee).unwrap();
    let signed: Vec<_> = validators
        .iter()
        .map(|pair| {
            attestation(&change, digest, pair.public_key().clone())
                .sign(pair.private_key())
                .unwrap()
        })
        .collect();

    assert!(committee_supports(&committee, &signed, id, 150));
    assert!(
        !committee_supports(&committee, &signed[..3], id, 150),
        "three of four is a quorum, not readiness"
    );
    assert!(!committee_supports(&committee, &signed, id, 200), "expired");
    assert!(
        !committee_supports(&committee[1..], &signed[1..], id, 150),
        "another committee"
    );

    let mut forged = signed.clone();
    forged[0].attestation.successor = SelectionId([0x99; 32]);
    assert!(forged[0].verify().is_err());
    assert!(!committee_supports(&committee, &forged, id, 150));
}

#[test]
fn containment_needs_fresh_endorsements_from_the_panel() {
    let panel_keys: Vec<KeyPair> = (20..29).map(key).collect();
    let panel: Vec<PublicKey> = panel_keys
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect();
    let containment = ContainmentV1 {
        network: network(),
        root: RootScopeV1::Universal,
        incident: [0x1c; 32],
        targets: vec![DefinitionId([3; 32]), DefinitionId([4; 32])],
        kind: ContainmentKindV1::AdmissionClosed,
        lease_until_ms: 1_000_000_000,
    };
    let id = containment.id().unwrap();
    let now = 10_000_000;
    let endorse = |pair: &KeyPair, at: u64| {
        EndorsementV1::sign(id, at, pair.public_key().clone(), pair.private_key()).unwrap()
    };

    let six: Vec<_> = panel_keys[..6]
        .iter()
        .map(|pair| endorse(pair, now - 60_000))
        .collect();
    assert_eq!(containment.check_endorsements(&panel, 6, now, &six), Ok(()));

    let mut duplicated = six[..5].to_vec();
    duplicated.push(six[0].clone());
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &duplicated),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let mut stale = six[..5].to_vec();
    stale.push(endorse(&panel_keys[5], now - ENDORSEMENT_LIFETIME_MS - 1));
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &stale),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let outsider = key(99);
    let mut foreign = six[..5].to_vec();
    foreign.push(endorse(&outsider, now - 60_000));
    assert_eq!(
        containment.check_endorsements(&panel, 6, now, &foreign),
        Err(ContainmentError::Threshold { have: 5, need: 6 })
    );

    let unordered = ContainmentV1 {
        targets: vec![DefinitionId([4; 32]), DefinitionId([3; 32])],
        ..containment
    };
    assert_eq!(
        unordered.check_endorsements(&panel, 6, now, &six),
        Err(ContainmentError::Targets)
    );
}

#[test]
fn rejection_codes_are_stable() {
    assert_eq!(
        RuntimeRejectionV1::DefinitionInactive {
            wire_id: "iroha.bond.call.v1".into()
        }
        .code(),
        "definition_inactive"
    );
    assert_eq!(
        RuntimeRejectionV1::DefinitionRestricted {
            definition: DefinitionId([1; 32]),
            restriction: RestrictionKindV1::Disabled.at(9),
        }
        .code(),
        "definition_restricted"
    );
    assert_eq!(
        RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.code(),
        "expectation_mismatch"
    );
}
