//! Canonical encoding, exact roster signatures and complete plan binding.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_model_base::domain::DomainId;

use crate::{
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId},
    isi::{InstructionBox, instruction_wire_id, taira_activation::ScheduleTairaTimeIronwoodV1},
    privacy::PrivacyPoolIdV1,
    runtime::{
        definition::{DefinitionId, SelectionId},
        lifecycle::SelectionTransitionV1,
    },
};

fn keys() -> [KeyPair; 4] {
    std::array::from_fn(|i| KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::Ed25519))
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn plan() -> TairaActivationPlanV1 {
    TairaActivationPlanV1 {
        version: TAIRA_ACTIVATION_VERSION_V1,
        chain_id: ChainId::from("taira-activation-test"),
        network_id: network(10),
        operation_id: [11; 32],
        change_set: ChangeSetV1 {
            network: network(10),
            root: RootScopeV1::Universal,
            predecessor: SelectionId([15; 32]),
            successor: SelectionId([16; 32]),
            effective_height: 20,
            publish: vec![],
            transitions: vec![SelectionTransitionV1::SetServicesProfile(DefinitionId(
                [17; 32],
            ))],
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
        pool: PrivacyIronwoodPoolBootstrapV1::new(
            PrivacyPoolIdV1::new([22; 32]),
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("privacy", "universal").unwrap(),
                "ironwood_core".parse().unwrap(),
            ),
            AssetBalanceScope::Global,
            AccountId::new(keys()[0].public_key().clone()),
        )
        .unwrap(),
    }
}

fn certificate() -> TairaActivationCertificateV1 {
    let plan = plan();
    let bytes = plan.signing_bytes().unwrap();
    let signatures = keys().map(|key| Signature::new(key.private_key(), &bytes));
    TairaActivationCertificateV1 { plan, signatures }
}

#[test]
fn plan_and_certificate_round_trip_only_as_exact_canonical_frames() {
    let plan = plan();
    let bytes = plan.to_bytes().unwrap();
    assert_eq!(TairaActivationPlanV1::from_bytes(&bytes).unwrap(), plan);
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(TairaActivationPlanV1::from_bytes(&trailing).is_err());

    let certificate = certificate();
    certificate.verify().unwrap();
    let bytes = certificate.to_bytes().unwrap();
    assert_eq!(
        TairaActivationCertificateV1::from_bytes(&bytes).unwrap(),
        certificate
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(TairaActivationCertificateV1::from_bytes(&trailing).is_err());
}

#[test]
fn every_signature_must_verify_in_its_own_roster_slot() {
    let good = certificate();
    let mut swapped = good.clone();
    swapped.signatures.swap(0, 1);
    assert_eq!(
        swapped.verify(),
        Err(TairaActivationErrorV1::BadSignature(0))
    );
    let mut repeated = good.clone();
    repeated.signatures[3] = repeated.signatures[0].clone();
    assert_eq!(
        repeated.verify(),
        Err(TairaActivationErrorV1::BadSignature(3))
    );
    let mut changed = good;
    changed.plan.expires_at_unix_ms += 1;
    assert_eq!(
        changed.verify(),
        Err(TairaActivationErrorV1::BadSignature(0))
    );
}

#[test]
fn plan_id_and_signature_cover_every_field() {
    let base = plan();
    let base_id = base.id().unwrap();
    let base_signed = base.signing_bytes().unwrap();
    let mutations: Vec<TairaActivationPlanV1> = vec![
        TairaActivationPlanV1 {
            chain_id: ChainId::from("another"),
            ..base.clone()
        },
        TairaActivationPlanV1 {
            operation_id: [99; 32],
            ..base.clone()
        },
        TairaActivationPlanV1 {
            notice: RuntimeNoticeV1 {
                blocks: 6,
                ..base.notice
            },
            ..base.clone()
        },
        TairaActivationPlanV1 {
            expires_at_unix_ms: 100_001,
            ..base.clone()
        },
        TairaActivationPlanV1 {
            authorization_digest: [99; 32],
            ..base.clone()
        },
        TairaActivationPlanV1 {
            release_digest: [99; 32],
            ..base.clone()
        },
        TairaActivationPlanV1 {
            source_closure_digest: [99; 32],
            ..base.clone()
        },
        TairaActivationPlanV1 {
            pool: PrivacyIronwoodPoolBootstrapV1::new(
                PrivacyPoolIdV1::new([99; 32]),
                base.pool.asset_definition_id.clone(),
                base.pool.public_balance_scope,
                base.pool.reserve_account.clone(),
            )
            .unwrap(),
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
    let refused = |change: &dyn Fn(&mut TairaActivationPlanV1)| {
        let mut plan = plan();
        change(&mut plan);
        assert!(plan.validate().is_err());
    };
    refused(&|plan| plan.version = 2);
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

#[test]
fn the_instruction_keeps_its_own_wire_id_and_frame() {
    let schedule = ScheduleTairaTimeIronwoodV1 {
        certificate: certificate(),
    };
    let instruction: InstructionBox = schedule.clone().into();
    assert_eq!(
        instruction_wire_id(&instruction),
        Some(ScheduleTairaTimeIronwoodV1::WIRE_ID)
    );
    assert_eq!(
        instruction
            .as_any()
            .downcast_ref::<ScheduleTairaTimeIronwoodV1>(),
        Some(&schedule)
    );
}

#[test]
fn certificate_json_keeps_exactly_four_roster_items() {
    let certificate = certificate();
    let json = norito::json::to_json(&certificate).unwrap();
    assert_eq!(
        norito::json::from_str::<TairaActivationCertificateV1>(&json).unwrap(),
        certificate
    );
}
