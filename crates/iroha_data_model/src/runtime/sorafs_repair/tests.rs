//! Canonical encoding, exact roster signatures and complete repair-context binding.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};

use crate::{
    account::AccountId,
    isi::{InstructionBox, instruction_wire_id, sorafs_repair::ApplySorafsRepairV1},
    runtime::{
        definition::{DefinitionId, SelectionId},
        lifecycle::SelectionTransitionV1,
    },
    sorafs::bootstrap::SorafsBootstrapProviderV1,
};

fn keys() -> [KeyPair; 4] {
    std::array::from_fn(|i| KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::Ed25519))
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn plan() -> SorafsRepairPlanV1 {
    SorafsRepairPlanV1 {
        version: SORAFS_REPAIR_VERSION_V1,
        chain_id: ChainId::from("sorafs-repair-test"),
        network_id: network(10),
        operation_id: [11; 32],
        parent_height: 12,
        parent_block_hash: [13; 32],
        parent_result_hash: [14; 32],
        change_set: ChangeSetV1 {
            network: network(10),
            root: RootScopeV1::Universal,
            predecessor: SelectionId([15; 32]),
            successor: SelectionId([16; 32]),
            effective_height: 13,
            publish: vec![],
            transitions: vec![SelectionTransitionV1::SetKernel(DefinitionId([17; 32]))],
            qualification: [18; 32],
        },
        committee: keys().map(|key| key.public_key().clone()),
        expires_at_unix_ms: 100_000,
        authorization_digest: [19; 32],
        release_digest: [20; 32],
        source_closure_digest: [21; 32],
        payload: SorafsBootstrapPayloadV1 {
            council: vec![1, 2, 3],
            providers: vec![SorafsBootstrapProviderV1 {
                owner: AccountId::new(keys()[0].public_key().clone()),
                envelope: vec![4, 5, 6],
            }],
            grants: vec![],
            compliance_operators: vec![],
        },
    }
}

fn certificate() -> SorafsRepairCertificateV1 {
    let plan = plan();
    let bytes = plan.signing_bytes().expect("valid signing plan");
    SorafsRepairCertificateV1 {
        plan,
        signatures: keys().map(|key| Signature::try_new(key.private_key(), &bytes).expect("sign")),
    }
}

#[test]
fn plan_certificate_and_typed_instruction_roundtrip() {
    let original = certificate();
    assert_eq!(original.verify(), Ok(()));
    let plan_bytes = original.plan.to_bytes().expect("plan frame");
    assert_eq!(
        SorafsRepairPlanV1::from_bytes(&plan_bytes).expect("plan decode"),
        original.plan
    );
    let frame = original.to_bytes().expect("certificate frame");
    assert_eq!(
        SorafsRepairCertificateV1::from_bytes(&frame).expect("certificate decode"),
        original
    );
    let json = norito::json::to_json(&original).expect("json");
    assert_eq!(
        norito::json::to_json_bounded(&original, json.len()).expect("exact JSON bound"),
        json
    );
    assert_eq!(
        norito::json::to_json_bounded(&original, json.len() - 1),
        Err(norito::json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(
        norito::json::from_str::<SorafsRepairCertificateV1>(&json).expect("json decode"),
        original
    );
    let json = norito::json::to_json(&original.plan).expect("plan json");
    assert_eq!(
        norito::json::from_str::<SorafsRepairPlanV1>(&json).expect("plan json decode"),
        original.plan
    );
    let instruction: InstructionBox = ApplySorafsRepairV1 {
        certificate: original,
    }
    .into();
    assert_eq!(
        instruction_wire_id(&instruction),
        Some(ApplySorafsRepairV1::WIRE_ID)
    );
    let frame = norito::encode_canonical(&instruction).expect("instruction frame");
    assert_eq!(
        norito::decode_canonical::<InstructionBox>(&frame).expect("instruction decode"),
        instruction
    );
    let json = norito::json::to_json(&instruction).expect("instruction json");
    assert_eq!(
        norito::json::from_str::<InstructionBox>(&json).expect("instruction json decode"),
        instruction
    );
}

#[test]
fn signature_domain_and_plan_identity_are_exact() {
    let plan = plan();
    let frame = plan.to_bytes().expect("frame");
    let expected_signing = [SORAFS_REPAIR_SIGNATURE_DOMAIN_V1, frame.as_slice()].concat();
    assert_eq!(
        plan.signing_bytes().expect("signing bytes"),
        expected_signing
    );
    let expected_id: [u8; 32] = Hash::new_from_chunks(&[SORAFS_REPAIR_ID_DOMAIN_V1, &frame]).into();
    assert_eq!(plan.id().expect("identity"), expected_id);
    let mut wrong_domain = certificate();
    wrong_domain.signatures[0] =
        Signature::try_new(keys()[0].private_key(), &frame).expect("sign raw frame");
    assert_eq!(
        wrong_domain.verify(),
        Err(SorafsRepairErrorV1::BadSignature(0))
    );
}

#[test]
fn every_parent_selection_evidence_and_payload_field_is_signed() {
    let original = certificate();
    let original_id = original.plan.id().expect("identity");
    let mutations: Vec<Box<dyn Fn(&mut SorafsRepairPlanV1)>> = vec![
        Box::new(|p| p.chain_id = ChainId::from("other-chain")),
        Box::new(|p| {
            p.network_id = network(22);
            p.change_set.network = p.network_id.clone();
        }),
        Box::new(|p| p.operation_id[0] ^= 1),
        Box::new(|p| {
            p.parent_height += 1;
            p.change_set.effective_height += 1;
        }),
        Box::new(|p| p.parent_block_hash[0] ^= 1),
        Box::new(|p| p.parent_result_hash[0] ^= 1),
        Box::new(|p| p.change_set.predecessor.0[0] ^= 1),
        Box::new(|p| p.change_set.successor.0[0] ^= 1),
        Box::new(|p| {
            p.change_set
                .transitions
                .push(SelectionTransitionV1::Activate(DefinitionId([23; 32])))
        }),
        Box::new(|p| p.change_set.qualification[0] ^= 1),
        Box::new(|p| p.committee.swap(0, 1)),
        Box::new(|p| p.expires_at_unix_ms += 1),
        Box::new(|p| p.authorization_digest[0] ^= 1),
        Box::new(|p| p.release_digest[0] ^= 1),
        Box::new(|p| p.source_closure_digest[0] ^= 1),
        Box::new(|p| p.payload.council.push(7)),
        Box::new(|p| p.payload.providers[0].envelope.push(8)),
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed.plan);
        assert_ne!(
            changed.plan.id().expect("structurally valid altered plan"),
            original_id
        );
        assert_eq!(changed.verify(), Err(SorafsRepairErrorV1::BadSignature(0)));
    }
}

#[test]
fn requires_four_distinct_exact_roster_signatures() {
    let original = certificate();
    for index in 0..4 {
        let mut changed = original.clone();
        changed.signatures[index] = original.signatures[(index + 1) % 4].clone();
        assert_eq!(
            changed.verify(),
            Err(SorafsRepairErrorV1::BadSignature(index))
        );
    }
    let mut changed = original.clone();
    changed.signatures.swap(2, 3);
    assert_eq!(changed.verify(), Err(SorafsRepairErrorV1::BadSignature(2)));
    changed.plan.committee[3] = changed.plan.committee[0].clone();
    assert_eq!(
        changed.verify(),
        Err(SorafsRepairErrorV1::Shape("repeated committee key"))
    );
    for count in [0, 1, 2, 3, 5] {
        let shares: Vec<_> = (0..count)
            .map(|i| original.signatures[i % 4].clone())
            .collect();
        let json = format!(
            "{{\"plan\":{},\"signatures\":{}}}",
            norito::json::to_json(&original.plan).expect("plan json"),
            norito::json::to_json(&shares).expect("shares json")
        );
        assert!(
            norito::json::from_str::<SorafsRepairCertificateV1>(&json).is_err(),
            "count {count}"
        );
    }
}

#[test]
fn rejects_inconsistent_and_uncommitted_plans() {
    let mutations: Vec<Box<dyn Fn(&mut SorafsRepairPlanV1)>> = vec![
        Box::new(|p| p.version = 2),
        Box::new(|p| p.parent_height = 0),
        Box::new(|p| p.parent_height = u64::MAX),
        Box::new(|p| p.change_set.effective_height += 1),
        Box::new(|p| p.change_set.network = network(22)),
        Box::new(|p| p.change_set.successor = p.change_set.predecessor),
        Box::new(|p| p.operation_id = [0; 32]),
        Box::new(|p| p.parent_block_hash = [0; 32]),
        Box::new(|p| p.parent_result_hash = [0; 32]),
        Box::new(|p| p.change_set.qualification = [0; 32]),
        Box::new(|p| p.expires_at_unix_ms = 0),
        Box::new(|p| p.authorization_digest = [0; 32]),
        Box::new(|p| p.release_digest = [0; 32]),
        Box::new(|p| p.source_closure_digest = [0; 32]),
        Box::new(|p| p.payload.providers.clear()),
    ];
    for mutate in mutations {
        let mut changed = plan();
        mutate(&mut changed);
        assert!(changed.validate().is_err());
        assert!(changed.signing_bytes().is_err());
        assert!(changed.id().is_err());
    }
}

#[test]
fn rejects_oversized_malformed_and_trailing_frames_before_use() {
    let original = certificate();
    let oversized = vec![0; SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1 + 1];
    assert_eq!(
        SorafsRepairPlanV1::from_bytes(&oversized),
        Err(SorafsRepairErrorV1::TooLarge)
    );
    assert_eq!(
        SorafsRepairCertificateV1::from_bytes(&oversized),
        Err(SorafsRepairErrorV1::TooLarge)
    );
    let mut oversized_plan = original.plan.clone();
    oversized_plan.payload.council = oversized;
    assert_eq!(oversized_plan.validate(), Err(SorafsRepairErrorV1::Payload));
    let mut oversized_certificate = original.clone();
    oversized_certificate.plan = oversized_plan;
    assert_eq!(
        oversized_certificate.verify(),
        Err(SorafsRepairErrorV1::TooLarge)
    );
    for mut bytes in [
        original.plan.to_bytes().expect("plan"),
        original.to_bytes().expect("certificate"),
    ] {
        bytes.push(0);
        assert!(SorafsRepairPlanV1::from_bytes(&bytes).is_err());
        assert!(SorafsRepairCertificateV1::from_bytes(&bytes).is_err());
    }
    for bytes in [vec![], vec![0], vec![1, 2, 3]] {
        assert_eq!(
            SorafsRepairPlanV1::from_bytes(&bytes),
            Err(SorafsRepairErrorV1::Encoding)
        );
        assert_eq!(
            SorafsRepairCertificateV1::from_bytes(&bytes),
            Err(SorafsRepairErrorV1::Encoding)
        );
    }
}
