//! Emergency containment: panel authorization, the narrowing it imposes, its ratification case,
//! renewal and expiry.

use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::{
    block::BlockHeader,
    parameter::Parameter,
    runtime::{
        definition::DefinitionId,
        lifecycle::{ContainmentV1, EndorsementV1},
    },
};

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    runtime::release_catalog,
    state::{State, World},
};

const HOUR_MS: u64 = 60 * 60 * 1000;
/// Committed time of the authorizing block.
const NOW_MS: u64 = 1_000 * HOUR_MS;

fn members(seeds: std::ops::Range<u8>) -> Vec<KeyPair> {
    let mut pairs: Vec<KeyPair> = seeds
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    pairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    pairs
}

fn keys(pairs: &[KeyPair]) -> Vec<PublicKey> {
    pairs.iter().map(|pair| pair.public_key().clone()).collect()
}

fn panel(primary: &[KeyPair], backup: &[KeyPair]) -> RuntimeEmergencyPanelV1 {
    RuntimeEmergencyPanelV1 {
        primary: keys(primary),
        backup: keys(backup),
        threshold: 6,
        term_until_ms: NOW_MS + 7 * 24 * HOUR_MS,
        min_lease_ms: 48 * HOUR_MS,
        max_lease_ms: 14 * 24 * HOUR_MS,
    }
}

/// The baseline interface bound to `wire_id`.
fn interface(wire_id: &str) -> DefinitionId {
    release_catalog()
        .expect("release catalog")
        .baseline
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire_id)
        .expect("baseline binding")
        .interface
}

fn header(height: u64, ms: u64) -> BlockHeader {
    BlockHeader::new(
        std::num::NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        ms,
        0,
    )
}

fn containment(
    state: &State,
    incident: u8,
    targets: Vec<DefinitionId>,
    lease_until_ms: u64,
) -> ContainmentV1 {
    let mut targets = targets;
    targets.sort();
    ContainmentV1 {
        network: *crate::state::StateReadOnly::network_id(&state.view()),
        root: RootScopeV1::Universal,
        incident: [incident; 32],
        targets,
        kind: ContainmentKindV1::AdmissionClosed,
        lease_until_ms,
    }
}

fn endorse(containment: &ContainmentV1, signers: &[KeyPair], at_ms: u64) -> Vec<EndorsementV1> {
    let id = containment.id().expect("containment identity");
    signers
        .iter()
        .map(|pair| {
            EndorsementV1::sign(id, at_ms, pair.public_key().clone(), pair.private_key())
                .expect("endorse")
        })
        .collect()
}

/// A State whose committed parameters hold `panel`.
fn state_with(panel: Option<&RuntimeEmergencyPanelV1>) -> State {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(header(1, 0));
    if let Some(panel) = panel {
        let mut stx = block.transaction();
        stx.world
            .set_parameter_for_testing(Parameter::Custom(
                panel.to_custom_parameter().expect("valid panel"),
            ))
            .expect("commit the panel");
        stx.apply();
    }
    block.commit_empty_block_for_testing().expect("commit");
    state
}

/// Run `instruction` in a block at `height` and committed time `ms`, committing on success.
fn submit(state: &State, height: u64, ms: u64, instruction: ContainRuntimeV1) -> Result<(), Error> {
    let mut block = state.block(header(height, ms));
    let mut stx = block.transaction();
    contain(&mut stx, instruction)?;
    stx.apply();
    block.commit_empty_block_for_testing().expect("commit");
    Ok(())
}

fn committed_restriction(state: &State, definition: &DefinitionId) -> Option<RestrictionV1> {
    restriction_of(state.world.runtime_selection.view().get(), definition)
}

/// A change set of `state`'s universal root that restricts `definition`: what Parliament would
/// admit or approve to ratify a containment of it. Review reads only its transitions.
fn ratifying(state: &State, definition: DefinitionId) -> ChangeSetV1 {
    let selection = state
        .world
        .runtime_selection
        .view()
        .get()
        .id()
        .expect("selection identity");
    ChangeSetV1 {
        network: *crate::state::StateReadOnly::network_id(&state.view()),
        root: RootScopeV1::Universal,
        predecessor: selection,
        successor: selection,
        effective_height: 1_000,
        publish: Vec::new(),
        transitions: vec![SelectionTransitionV1::Restrict {
            definition,
            restriction: RestrictionKindV1::AdmissionClosed,
        }],
        qualification: [0x5c; 32],
    }
}

/// Record, in a block at `height` and committed time `ms`, governance progress on `change_set`.
fn review(state: &State, height: u64, ms: u64, change_set: &ChangeSetV1) -> ChangeSetId {
    let id = change_set.id().expect("change-set identity");
    let mut block = state.block(header(height, ms));
    let mut stx = block.transaction();
    record_review(&mut stx.world, id, change_set, height);
    stx.apply();
    block.commit_empty_block_for_testing().expect("commit");
    id
}

/// Start and commit an empty block at `height` and committed time `ms`; block start expires
/// due containments.
fn pass(state: &State, height: u64, ms: u64) {
    state
        .block(header(height, ms))
        .commit_empty_block_for_testing()
        .expect("commit");
}

#[test]
fn a_root_without_a_committed_panel_admits_no_containment() {
    let state = state_with(None);
    let primary = members(1..10);
    let target = containment(
        &state,
        1,
        vec![interface("iroha.log")],
        NOW_MS + 72 * HOUR_MS,
    );
    let endorsements = endorse(&target, &primary[..6], NOW_MS - 1_000);
    let error = submit(
        &state,
        2,
        NOW_MS,
        ContainRuntimeV1 {
            containment: target,
            endorsements,
        },
    )
    .expect_err("no panel");
    assert!(format!("{error:?}").contains("no runtime emergency panel is committed"));
}

#[test]
fn six_fresh_endorsements_close_admission_from_the_next_block() {
    let primary = members(1..10);
    let backup = members(10..19);
    let state = state_with(Some(&panel(&primary, &backup)));
    let log = interface("iroha.log");
    let target = containment(&state, 1, vec![log], NOW_MS + 72 * HOUR_MS);

    let five = endorse(&target, &primary[..5], NOW_MS - 1_000);
    assert!(
        submit(
            &state,
            2,
            NOW_MS,
            ContainRuntimeV1 {
                containment: target.clone(),
                endorsements: five,
            },
        )
        .is_err(),
        "five of nine is short of the threshold"
    );
    let stale = endorse(&target, &backup[..6], NOW_MS - 31 * 60 * 1000);
    assert!(
        submit(
            &state,
            2,
            NOW_MS,
            ContainRuntimeV1 {
                containment: target.clone(),
                endorsements: stale,
            },
        )
        .is_err(),
        "endorsements expire after 30 minutes"
    );

    let backup_six = endorse(&target, &backup[3..9], NOW_MS - 1_000);
    submit(
        &state,
        2,
        NOW_MS,
        ContainRuntimeV1 {
            containment: target.clone(),
            endorsements: backup_six,
        },
    )
    .expect("six backup members authorize containment");
    assert_eq!(
        committed_restriction(&state, &log),
        Some(RestrictionV1::AdmissionClosed(3)),
        "admission closes from the block after the authorization"
    );
    let record = state
        .world
        .runtime_containments
        .view()
        .get(&target.id().unwrap())
        .cloned()
        .expect("recorded containment");
    assert_eq!(record.status, ContainmentStatusV1::Active);
    assert_eq!(record.contained.len(), 1);
    assert_eq!(record.contained[0].previous, None);
}

#[test]
fn governance_and_lifecycle_interfaces_cannot_be_contained() {
    let primary = members(1..10);
    let state = state_with(Some(&panel(&primary, &members(10..19))));
    for wire_id in [
        "iroha.set_parameter",
        "iroha.runtime.support.attest.v1",
        "iroha.instruction.v1::governance::ProposeRuntimeUpgradeProposal",
    ] {
        let target = containment(&state, 2, vec![interface(wire_id)], NOW_MS + 72 * HOUR_MS);
        let endorsements = endorse(&target, &primary[..6], NOW_MS - 1_000);
        let error = submit(
            &state,
            2,
            NOW_MS,
            ContainRuntimeV1 {
                containment: target,
                endorsements,
            },
        )
        .expect_err("protected interface");
        assert!(
            format!("{error:?}").contains("stay available to repair an incident"),
            "{wire_id}: {error:?}"
        );
    }
    let bundle = release_catalog().unwrap().baseline.bindings[0].implementation;
    let target = containment(&state, 3, vec![bundle], NOW_MS + 72 * HOUR_MS);
    let endorsements = endorse(&target, &primary[..6], NOW_MS - 1_000);
    assert!(
        submit(
            &state,
            2,
            NOW_MS,
            ContainRuntimeV1 {
                containment: target,
                endorsements,
            },
        )
        .is_err(),
        "the bundle answering governance is never contained"
    );
}

#[test]
fn the_lease_stays_inside_the_panel_bounds() {
    let primary = members(1..10);
    let state = state_with(Some(&panel(&primary, &members(10..19))));
    for lease_until_ms in [NOW_MS + 47 * HOUR_MS, NOW_MS + 15 * 24 * HOUR_MS] {
        let target = containment(&state, 4, vec![interface("iroha.log")], lease_until_ms);
        let endorsements = endorse(&target, &primary[..6], NOW_MS - 1_000);
        assert!(
            submit(
                &state,
                2,
                NOW_MS,
                ContainRuntimeV1 {
                    containment: target,
                    endorsements,
                },
            )
            .is_err(),
            "lease {lease_until_ms}"
        );
    }
}

#[test]
fn expiry_restores_held_restrictions_and_renewal_takes_the_lease_over() {
    let primary = members(1..10);
    let state = state_with(Some(&panel(&primary, &members(10..19))));
    let log = interface("iroha.log");
    let lease_until_ms = NOW_MS + 72 * HOUR_MS;
    let first = containment(&state, 5, vec![log], lease_until_ms);
    submit(
        &state,
        2,
        NOW_MS,
        ContainRuntimeV1 {
            containment: first.clone(),
            endorsements: endorse(&first, &primary[..6], NOW_MS - 1_000),
        },
    )
    .expect("contain");

    // Renewal opens halfway through the lease.
    let early_ms = NOW_MS + 30 * HOUR_MS;
    let early = containment(&state, 5, vec![log], early_ms + 72 * HOUR_MS);
    assert!(
        submit(
            &state,
            3,
            early_ms,
            ContainRuntimeV1 {
                containment: early.clone(),
                endorsements: endorse(&early, &primary[..6], early_ms - 1_000),
            },
        )
        .is_err(),
        "a renewal before half the lease is refused"
    );
    let renew_ms = NOW_MS + 40 * HOUR_MS;
    let renewal = containment(&state, 5, vec![log], renew_ms + 72 * HOUR_MS);
    let renew = |height| {
        submit(
            &state,
            height,
            renew_ms,
            ContainRuntimeV1 {
                containment: renewal.clone(),
                endorsements: endorse(&renewal, &primary[..6], renew_ms - 1_000),
            },
        )
    };
    let refusal = renew(3).expect_err("a renewal without review progress");
    assert!(
        format!("{refusal:?}").contains("a renewal needs review progress"),
        "{refusal:?}"
    );
    let reviewed = review(&state, 3, renew_ms - 2_000, &ratifying(&state, log));
    renew(4).expect("renewal with fresh endorsements and review progress");
    let records = state.world.runtime_containments.view();
    assert_eq!(
        records
            .get(&first.id().unwrap())
            .map(|record| record.status),
        Some(ContainmentStatusV1::Renewed(4))
    );
    assert_eq!(
        records
            .get(&renewal.id().unwrap())
            .and_then(|record| record.review),
        Some(ContainmentReviewV1 {
            change_set: reviewed,
            height: 3,
        }),
        "the renewal carries the case's progress over"
    );
    drop(records);

    // The first lease passing no longer matters once renewed; the renewal's does.
    pass(&state, 5, lease_until_ms + 1);
    assert_eq!(
        committed_restriction(&state, &log),
        Some(RestrictionV1::AdmissionClosed(3))
    );
    pass(&state, 6, renewal.lease_until_ms);
    assert_eq!(committed_restriction(&state, &log), None, "the lease ended");
    assert_eq!(
        state
            .world
            .runtime_containments
            .view()
            .get(&renewal.id().unwrap())
            .map(|record| record.status),
        Some(ContainmentStatusV1::Expired(6))
    );
}

#[test]
fn governance_review_and_takeover_resolve_the_ratification_case() {
    let primary = members(1..10);
    let state = state_with(Some(&panel(&primary, &members(10..19))));
    let log = interface("iroha.log");
    let target = containment(&state, 6, vec![log], NOW_MS + 72 * HOUR_MS);
    submit(
        &state,
        2,
        NOW_MS,
        ContainRuntimeV1 {
            containment: target.clone(),
            endorsements: endorse(&target, &primary[..6], NOW_MS - 1_000),
        },
    )
    .expect("contain");
    let id = target.id().unwrap();
    let record = |state: &State| {
        state
            .world
            .runtime_containments
            .view()
            .get(&id)
            .cloned()
            .expect("recorded containment")
    };
    assert_eq!(
        record(&state).review,
        None,
        "the case opens without progress"
    );

    // A change set that addresses another definition is no progress on this case.
    let other = ratifying(&state, interface("iroha.alias.ensure"));
    review(&state, 3, NOW_MS + HOUR_MS, &other);
    assert_eq!(record(&state).review, None);
    let ratification = ratifying(&state, log);
    let reviewed = review(&state, 4, NOW_MS + 2 * HOUR_MS, &ratification);
    assert_eq!(
        record(&state).review,
        Some(ContainmentReviewV1 {
            change_set: reviewed,
            height: 4,
        })
    );

    // Governance takes the restriction over: the containment no longer holds what it imposed.
    let mut block = state.block(header(5, NOW_MS + 3 * HOUR_MS));
    let mut stx = block.transaction();
    let holding_before = holding(&stx.world);
    assert_eq!(holding_before, vec![id]);
    let mut selection = stx.world.runtime_selection.get().clone();
    set_restriction(&mut selection, log, Some(RestrictionV1::Disabled(5)));
    *stx.world.runtime_selection.get_mut() = selection;
    resolve_taken_over(&mut stx.world, 5, &holding_before);
    stx.apply();
    block.commit_empty_block_for_testing().expect("commit");
    assert_eq!(record(&state).status, ContainmentStatusV1::Resolved(5));

    // A resolved containment never expires: the lease ending restores nothing.
    pass(&state, 6, target.lease_until_ms + 1);
    assert_eq!(
        committed_restriction(&state, &log),
        Some(RestrictionV1::Disabled(5))
    );
    assert_eq!(record(&state).status, ContainmentStatusV1::Resolved(5));
}

#[test]
fn expiry_leaves_a_restriction_governance_took_over() {
    let held = Some(RestrictionV1::AdmissionClosed(3));
    let imposed = RestrictionV1::AdmissionClosed(3);
    let target = DefinitionId([7; 32]);
    let mut selection = release_catalog().unwrap().baseline.clone();
    set_restriction(&mut selection, target, held);
    assert_eq!(restriction_of(&selection, &target), Some(imposed));
    set_restriction(&mut selection, target, Some(RestrictionV1::Disabled(9)));
    assert_ne!(
        restriction_of(&selection, &target),
        Some(imposed),
        "a ratified restriction is no longer the one containment imposed"
    );
    assert!(already_restricted(
        Some(RestrictionV1::Disabled(2)),
        RestrictionV1::AdmissionClosed(3)
    ));
    assert!(!already_restricted(
        Some(RestrictionV1::Deprecated(2)),
        RestrictionV1::AdmissionClosed(3)
    ));
    assert!(!already_restricted(
        Some(RestrictionV1::Disabled(9)),
        RestrictionV1::AdmissionClosed(3)
    ));
}
