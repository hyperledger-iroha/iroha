//! Shared work predicate exercised with original signed queue-admission inputs.
use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, builder::BlockBuilder as WireBlockBuilder, lane_admission::*},
    isi::Log,
    level::Level,
    transaction::{
        FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder, TransactionEntrypoint,
    },
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use std::{collections::BTreeSet, num::NonZeroU64};

#[test]
fn signed_admission_is_work_without_a_network_execution_leaf() {
    let parent = WireBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        1,
        0,
    ))
    .build(BTreeSet::new());
    let network = NetworkId::from_genesis_hash(parent.hash());
    let mut keys: Vec<_> = (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let validators: Vec<_> = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect();
    let transaction = TransactionBuilder::new(
        network,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .with_instructions([Log::new(Level::INFO, "original queued work".into())])
    .sign(ALICE_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);
    let route = RoutingDecision::new(LaneId::new(0), DataSpaceId::new(0));
    let routing = RoutingPlan::single(route);
    let context = QueuePlanAdmissionContextV1 {
        version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
        authority_height: 1,
        proposal_height: 2,
        predecessor_block_hash: Some(parent.hash()),
        routing_plan_digest: routing.digest(),
        route_incarnations: vec![QueuePlanRouteIncarnationV1 {
            leg: routing.coordinator_leg(),
            lane_incarnation: Hash::new(b"admitted lane incarnation"),
            validator_set_hash_version: 1,
            validator_set_hash: HashOf::new(&validators),
            validator_set: validators,
            validator_count: 4,
            durability_threshold: 2,
        }],
    };
    let binding = crate::torii_proxy::new_queue_plan_admission_binding(
        &network,
        &entrypoint,
        &routing,
        context,
        0,
    )
    .unwrap();
    let attestations = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, key)| QueuePlanAdmissionAttestationV1 {
            version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
            validator_index: index as u16,
            signature: Signature::new(
                key.private_key(),
                &crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(
                    binding.canonical_hash(),
                    index as u16,
                )
                .unwrap(),
            ),
        })
        .collect();
    let certificate = QueuePlanAdmissionCertificateV1 {
        version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
        binding,
        attestations,
    };
    crate::torii_proxy::validate_queue_plan_admission_certificate_v1(
        &network,
        certificate.clone(),
        crate::torii_proxy::QueuePlanAdmissionCertificateStrengthV1::Quorum,
    )
    .unwrap();
    let input = LaneAdmittedInputV1 {
        entrypoint,
        certificate,
    };
    let mut builder = WireBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(parent.hash()),
        None,
        2,
        0,
    ));
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::default()
            .with_queue_plan_admissions(vec![norito::encode_canonical(&input).unwrap()]),
    ));
    let block = builder.build(BTreeSet::new());
    assert_eq!(block.network_entrypoint_count(), 0);
    assert!(block.has_consensus_work());
    let encoded = encode(&block).unwrap();
    let decoded = decode(&encoded).unwrap();
    assert!(decoded.has_consensus_work());
    assert_eq!(decoded.encode_wire().unwrap(), encoded);
    let mut substituted = input.certificate;
    substituted.binding.enqueue_timestamp_ms += 1;
    assert!(
        crate::torii_proxy::validate_queue_plan_admission_certificate_v1(
            &network,
            substituted,
            crate::torii_proxy::QueuePlanAdmissionCertificateStrengthV1::Quorum
        )
        .is_err()
    );
}

#[test]
fn outputs_and_empty_context_cannot_create_proposal_work() {
    let mut builder = WireBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        19,
    ));
    builder.set_execution_context(Some(BlockExecutionContextBundle::default()));
    let block = builder.build(BTreeSet::new());
    assert!(!block.has_consensus_work());
    assert_eq!(encode(&block), Err(PayloadError::EmptyBlock));
    assert!(matches!(
        decode(&block.encode_wire().unwrap()),
        Err(PayloadError::EmptyBlock)
    ));
}
