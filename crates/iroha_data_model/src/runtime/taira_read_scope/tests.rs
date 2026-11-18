//! Canonical encoding, exact roster signatures and complete plan binding.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};

use crate::{
    isi::{InstructionBox, instruction_wire_id, taira_read_scope::ScheduleTairaReadScopeV1},
    runtime::{
        definition::{PermissionDescriptorV1, SchemaV1, SelectionId},
        lifecycle::SelectionTransitionV1,
        taira_activation::{TAIRA_ACTIVATION_ID_DOMAIN_V1, TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1},
    },
};

fn keys() -> [KeyPair; 4] {
    std::array::from_fn(|i| KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::Ed25519))
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn plan() -> TairaReadScopePlanV1 {
    let descriptor = DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
        name: "CanReadStateScope".into(),
        payload: SchemaV1::Codec("semantic:iroha.state-scope-read.json.v1@0.0.2".into()),
    });
    let permission = descriptor.id().unwrap();
    TairaReadScopePlanV1 {
        version: TAIRA_READ_SCOPE_VERSION_V1,
        chain_id: ChainId::from("taira-read-scope-test"),
        network_id: network(10),
        operation_id: [11; 32],
        change_set: ChangeSetV1 {
            network: network(10),
            root: RootScopeV1::Universal,
            predecessor: KEYED_PREDECESSOR,
            successor: SCOPED_SUCCESSOR,
            effective_height: 20,
            publish: vec![descriptor],
            transitions: vec![SelectionTransitionV1::Activate(permission)],
            qualification: [18; 32],
        },
        notice: RuntimeNoticeV1 {
            blocks: 5,
            ms: 1,
            approval_cushion_ms: 1,
        },
        committee: keys().map(|key| key.public_key().clone()),
        expires_at_unix_ms: 100_000,
        authorization_digest: [19; 32],
        release_digest: [20; 32],
        source_closure_digest: [21; 32],
    }
}

fn certificate() -> TairaReadScopeCertificateV1 {
    let plan = plan();
    let bytes = plan.signing_bytes().unwrap();
    let signatures = keys().map(|key| Signature::new(key.private_key(), &bytes));
    TairaReadScopeCertificateV1 { plan, signatures }
}

#[test]
fn plan_and_certificate_round_trip_only_as_exact_canonical_frames() {
    let plan = plan();
    let bytes = plan.to_bytes().unwrap();
    assert_eq!(TairaReadScopePlanV1::from_bytes(&bytes).unwrap(), plan);
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(TairaReadScopePlanV1::from_bytes(&trailing).is_err());

    let certificate = certificate();
    certificate.verify().unwrap();
    let bytes = certificate.to_bytes().unwrap();
    assert_eq!(
        TairaReadScopeCertificateV1::from_bytes(&bytes).unwrap(),
        certificate
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(TairaReadScopeCertificateV1::from_bytes(&trailing).is_err());
}

#[test]
fn every_signature_must_verify_in_its_own_roster_slot() {
    let good = certificate();
    let mut swapped = good.clone();
    swapped.signatures.swap(0, 1);
    assert_eq!(
        swapped.verify(),
        Err(TairaReadScopeErrorV1::BadSignature(0))
    );
    let mut repeated = good.clone();
    repeated.signatures[3] = repeated.signatures[0].clone();
    assert_eq!(
        repeated.verify(),
        Err(TairaReadScopeErrorV1::BadSignature(3))
    );
    let mut changed = good;
    changed.plan.expires_at_unix_ms += 1;
    assert_eq!(
        changed.verify(),
        Err(TairaReadScopeErrorV1::BadSignature(0))
    );
}

#[test]
fn plan_id_and_signature_cover_every_field() {
    let base = plan();
    let base_id = base.id().unwrap();
    let base_signed = base.signing_bytes().unwrap();
    let mut other_change = base.change_set.clone();
    other_change.effective_height += 1;
    let mut other_committee = base.committee.clone();
    other_committee.swap(0, 1);
    let mutations: Vec<TairaReadScopePlanV1> = vec![
        TairaReadScopePlanV1 {
            chain_id: ChainId::from("another"),
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            operation_id: [99; 32],
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            change_set: other_change,
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            notice: RuntimeNoticeV1 {
                blocks: 6,
                ..base.notice
            },
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            committee: other_committee,
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            expires_at_unix_ms: 100_001,
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            authorization_digest: [99; 32],
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            release_digest: [99; 32],
            ..base.clone()
        },
        TairaReadScopePlanV1 {
            source_closure_digest: [99; 32],
            ..base.clone()
        },
    ];
    for changed in mutations {
        assert_ne!(changed.id().unwrap(), base_id);
        assert_ne!(changed.signing_bytes().unwrap(), base_signed);
    }
}

#[test]
fn shape_rules_refuse_what_core_must_never_see() {
    let refused = |change: &dyn Fn(&mut TairaReadScopePlanV1)| {
        let mut plan = plan();
        change(&mut plan);
        assert!(plan.validate().is_err());
    };
    refused(&|plan| plan.version = 2);
    refused(&|plan| plan.change_set.predecessor = SelectionId([0xab; 32]));
    refused(&|plan| {
        plan.change_set
            .transitions
            .push(plan.change_set.transitions[0].clone())
    });
    refused(&|plan| plan.change_set.publish.clear());
    refused(&|plan| {
        if let DefinitionDescriptorV1::Permission(permission) = &mut plan.change_set.publish[0] {
            permission.name = "CanReadAllLedgerData".into();
        }
    });
    refused(&|plan| {
        if let DefinitionDescriptorV1::Permission(permission) = &mut plan.change_set.publish[0] {
            permission.payload = SchemaV1::Codec("semantic:unrelated@0.0.2".into());
        }
        let foreign = plan.change_set.publish[0].id().unwrap();
        plan.change_set.transitions = vec![SelectionTransitionV1::Activate(foreign)];
    });
    refused(&|plan| plan.change_set.successor = SelectionId([0xfe; 32]));
    refused(&|plan| plan.change_set.root = RootScopeV1::DataSpace(Default::default()));
    refused(&|plan| plan.change_set.network = network(99));
    refused(&|plan| plan.change_set.successor = plan.change_set.predecessor);
    refused(&|plan| plan.notice.blocks = 0);
    refused(&|plan| plan.change_set.effective_height = 1);
    refused(&|plan| plan.expires_at_unix_ms = 0);
    refused(&|plan| plan.operation_id = [0; 32]);
    refused(&|plan| plan.change_set.qualification = [0; 32]);
    refused(&|plan| plan.authorization_digest = [0; 32]);
    refused(&|plan| plan.release_digest = [0; 32]);
    refused(&|plan| plan.source_closure_digest = [0; 32]);
    refused(&|plan| plan.committee[3] = plan.committee[0].clone());
}

/// A signature over this plan is never a signature over a Time and Ironwood plan, and the two
/// plan identities never share a preimage prefix.
#[test]
fn the_signature_and_identity_domains_are_this_transition_s_own() {
    assert_ne!(
        TAIRA_READ_SCOPE_SIGNATURE_DOMAIN_V1,
        TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1
    );
    assert_ne!(TAIRA_READ_SCOPE_ID_DOMAIN_V1, TAIRA_ACTIVATION_ID_DOMAIN_V1);
    let plan = plan();
    assert!(
        plan.signing_bytes()
            .unwrap()
            .starts_with(TAIRA_READ_SCOPE_SIGNATURE_DOMAIN_V1)
    );
}

#[test]
fn the_instruction_keeps_its_own_wire_id_and_frame() {
    let schedule = ScheduleTairaReadScopeV1 {
        certificate: certificate(),
    };
    let instruction: InstructionBox = schedule.clone().into();
    assert_eq!(
        instruction_wire_id(&instruction),
        Some(ScheduleTairaReadScopeV1::WIRE_ID)
    );
    assert_eq!(
        instruction
            .as_any()
            .downcast_ref::<ScheduleTairaReadScopeV1>(),
        Some(&schedule)
    );
}

#[test]
fn certificate_json_keeps_exactly_four_roster_items() {
    let certificate = certificate();
    let json = norito::json::to_json(&certificate).unwrap();
    assert_eq!(
        norito::json::from_str::<TairaReadScopeCertificateV1>(&json).unwrap(),
        certificate
    );
}
