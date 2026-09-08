//! Direct adversarial tests for Native AMX participant application classification.

use super::*;
use iroha_data_model::block::consensus::{LaneBlockCommitment, NativeAmxParticipantSettlement};
use iroha_primitives::numeric::Quantity;

const INCONSISTENT_IDENTITY: &str =
    "Native AMX participant leg identity is internally inconsistent";
const SAME_ROUTE_DRIFT: &str = "Native AMX same-route leg differs from the coordinator identity";

fn fixture_receipt() -> NativeAmxReceipt {
    let coordinator = RoutingDecision::new(LaneId::new(7), DataSpaceId::new(11));
    let participant = RoutingDecision::new(LaneId::new(8), DataSpaceId::new(12));
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            RouteLeg::new(coordinator, RouteLegRole::Participant),
            RouteLeg::new(participant, RouteLegRole::Participant),
        ],
    );
    let keypairs = (1_u8..=4)
        .map(|seed| {
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("classification fixture BLS keypair")
        })
        .collect::<Vec<_>>();
    let mut receipt = crate::block::tests::signed_native_amx_receipt(
        [0xAB; Hash::LENGTH],
        HashOf::from_untyped_unchecked(Hash::prehashed([0x61; Hash::LENGTH])),
        &plan,
        42,
        &keypairs,
    );
    // Both roles need a retained predecessor so removing its hash is a real mutation.
    let remote = receipt
        .legs
        .iter_mut()
        .find(|leg| leg.lane_id == participant.lane_id)
        .expect("classification fixture has a separate participant");
    remote
        .participant_proposal
        .descriptor
        .previous_lane_block_height = 41;
    remote
        .participant_proposal
        .descriptor
        .previous_lane_block_descriptor_hash = Some(Hash::new(b"classification-predecessor"));
    remote.participant_proposal.descriptor.lane_block_height = 42;
    rebind_participant_identity(remote);
    for qc in [&mut remote.prepare_qc, &mut remote.commit_qc] {
        let min_signers = usize::try_from(qc.body.participant_min_quorum).expect("fixture quorum");
        let votes = keypairs
            .iter()
            .take(min_signers)
            .map(|keypair| NativeAmxVoteV2 {
                body: qc.body,
                signer: PeerId::new(keypair.public_key().clone()),
                bls_signature: Signature::try_new(
                    keypair.private_key(),
                    &qc.body.signature_preimage(),
                )
                .expect("sign classification fixture vote")
                .payload()
                .to_vec(),
            })
            .collect::<Vec<_>>();
        *qc = aggregate_votes_to_qc(
            qc.body,
            qc.validator_set().to_vec(),
            qc.validator_set_pops().to_vec(),
            &votes,
            min_signers,
        )
        .expect("aggregate classification fixture quorum");
    }
    let commitment = LaneBlockCommitment {
        block_height: receipt.lane_block_height,
        lane_id: receipt.lane_id,
        lane_incarnation: receipt.lane_incarnation,
        dataspace_id: receipt.dataspace_id,
        tx_count: 1,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: vec![receipt],
    };
    commitment
        .validate_native_amx_receipts()
        .expect("classification fixture has valid grouped receipt structure");
    commitment
        .native_amx_receipts
        .into_iter()
        .next()
        .expect("fixture contains a receipt")
}

fn leg_index(receipt: &NativeAmxReceipt, role: NativeAmxParticipantApplicationRole) -> usize {
    receipt
        .legs
        .iter()
        .position(|leg| native_amx_participant_application_role(receipt, leg) == Ok(role))
        .expect("fixture contains the requested application role")
}

/// Keep all participant-side identities coherent while changing a test proposal.
fn rebind_participant_identity(leg: &mut NativeAmxLegRecordV2) {
    let descriptor = &mut leg.participant_proposal.descriptor;
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    leg.lane_id = descriptor.lane_id;
    leg.dataspace_id = descriptor.dataspace_id;
    leg.participant_settlement = NativeAmxParticipantSettlement::try_new(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        descriptor.proposal_height,
        leg.participant_settlement.previous_native_settlement_hash(),
        leg.participant_settlement.source_ids().to_vec(),
    )
    .expect("mutated fixture has valid participant control coordinates");
    leg.participant_settlement_hash = leg
        .participant_settlement
        .computed_hash()
        .expect("mutated fixture settlement hashes");
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    let descriptor = &leg.participant_proposal.descriptor;
    for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
        body.participant_lane_id = descriptor.lane_id;
        body.participant_dataspace_id = descriptor.dataspace_id;
        body.participant_lane_incarnation = descriptor.lane_incarnation;
        body.authority_context_height = descriptor.proposal_height;
        body.participant_previous_block_height = descriptor.previous_lane_block_height;
        body.participant_previous_block_descriptor_hash =
            descriptor.previous_lane_block_descriptor_hash;
        body.participant_lane_block_height = descriptor.lane_block_height;
        body.participant_lane_block_view = descriptor.lane_block_view;
        body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
        body.participant_settlement_commitment = Hash::from(leg.participant_settlement_hash);
    }
}

#[test]
fn participant_application_role_classifies_exact_routes_and_incarnations() {
    let receipt = fixture_receipt();
    for role in [
        NativeAmxParticipantApplicationRole::Coordinator,
        NativeAmxParticipantApplicationRole::SeparateParticipant,
    ] {
        let leg = &receipt.legs[leg_index(&receipt, role)];
        let descriptor = &leg.participant_proposal.descriptor;
        assert_eq!(
            native_amx_participant_application_role(&receipt, leg),
            Ok(role)
        );
        assert_eq!(
            native_amx_receipt_requires_separate_participant_application_for(
                &receipt,
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
            ),
            Ok(role == NativeAmxParticipantApplicationRole::SeparateParticipant),
        );
        for (label, lane_id, dataspace_id, incarnation) in [
            (
                "unknown lane",
                LaneId::new(90),
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
            ),
            (
                "different dataspace",
                descriptor.lane_id,
                DataSpaceId::new(90),
                descriptor.lane_incarnation,
            ),
            (
                "stale incarnation",
                descriptor.lane_id,
                descriptor.dataspace_id,
                Hash::new(b"classifier-stale-incarnation"),
            ),
        ] {
            assert_eq!(
                native_amx_receipt_requires_separate_participant_application_for(
                    &receipt,
                    lane_id,
                    dataspace_id,
                    incarnation,
                ),
                Ok(false),
                "{role:?}: {label} must not match the exact application route",
            );
        }
    }
}

#[test]
fn participant_application_role_keeps_each_route_coordinate_distinct() {
    let receipt = fixture_receipt();
    let index = leg_index(
        &receipt,
        NativeAmxParticipantApplicationRole::SeparateParticipant,
    );
    for (lane_id, dataspace_id) in [
        (receipt.lane_id, receipt.legs[index].dataspace_id),
        (receipt.legs[index].lane_id, receipt.dataspace_id),
    ] {
        let mut receipt = receipt.clone();
        let leg = &mut receipt.legs[index];
        leg.participant_proposal.descriptor.lane_id = lane_id;
        leg.participant_proposal.descriptor.dataspace_id = dataspace_id;
        rebind_participant_identity(leg);
        assert_eq!(
            native_amx_participant_application_role(&receipt, &receipt.legs[index]),
            Ok(NativeAmxParticipantApplicationRole::SeparateParticipant),
        );
        assert_eq!(
            native_amx_receipt_requires_separate_participant_application_for(
                &receipt,
                lane_id,
                dataspace_id,
                receipt.legs[index]
                    .participant_proposal
                    .descriptor
                    .lane_incarnation,
            ),
            Ok(true),
        );
    }
}

type BodyIdentityMutation = (&'static str, fn(&mut NativeAmxAttestationBodyV2));
const BODY_IDENTITY_MUTATIONS: &[BodyIdentityMutation] = &[
    ("participant lane", |body| {
        body.participant_lane_id = LaneId::new(90)
    }),
    ("participant dataspace", |body| {
        body.participant_dataspace_id = DataSpaceId::new(90)
    }),
    ("participant incarnation", |body| {
        body.participant_lane_incarnation = Hash::new(b"phase-incarnation-drift")
    }),
    ("authority height", |body| {
        body.authority_context_height += 1
    }),
    ("predecessor height", |body| {
        body.participant_previous_block_height += 1
    }),
    ("missing predecessor hash", |body| {
        body.participant_previous_block_descriptor_hash = None
    }),
    ("predecessor hash", |body| {
        body.participant_previous_block_descriptor_hash =
            Some(Hash::new(b"phase-predecessor-drift"))
    }),
    ("participant height", |body| {
        body.participant_lane_block_height += 1
    }),
    ("participant view", |body| {
        body.participant_lane_block_view += 1
    }),
    ("participant proposal", |body| {
        body.participant_proposal_hash = Hash::new(b"phase-proposal-drift")
    }),
    ("participant settlement", |body| {
        body.participant_settlement_commitment = Hash::new(b"phase-settlement-drift")
    }),
    ("coordinator lane", |body| {
        body.coordinator_lane_id = LaneId::new(90)
    }),
    ("coordinator dataspace", |body| {
        body.coordinator_dataspace_id = DataSpaceId::new(90)
    }),
    ("coordinator incarnation", |body| {
        body.coordinator_lane_incarnation = Hash::new(b"phase-coordinator-incarnation-drift")
    }),
    ("coordinator height", |body| {
        body.planned_coordinator_block_height += 1
    }),
    ("coordinator view", |body| {
        body.coordinator_lane_block_view += 1
    }),
    ("coordinator proposal", |body| {
        body.coordinator_proposal_hash = Hash::new(b"phase-coordinator-proposal-drift")
    }),
];

#[test]
fn participant_application_role_rejects_independent_prepare_and_commit_identity_drift() {
    let receipt = fixture_receipt();
    for index in 0..receipt.legs.len() {
        for phase in [NativeAmxPhase::Prepare, NativeAmxPhase::Commit] {
            for &(label, mutate) in BODY_IDENTITY_MUTATIONS {
                let mut altered = receipt.clone();
                let leg = &mut altered.legs[index];
                let body = match phase {
                    NativeAmxPhase::Prepare => &mut leg.prepare_qc.body,
                    NativeAmxPhase::Commit => &mut leg.commit_qc.body,
                };
                mutate(body);
                assert_eq!(
                    native_amx_participant_application_role(&altered, &altered.legs[index]),
                    Err(INCONSISTENT_IDENTITY),
                    "leg {index}, {phase:?}: {label} drift must fail closed",
                );
            }
        }
    }
}

#[test]
fn participant_application_role_rejects_coherent_same_route_coordinator_drift() {
    use iroha_data_model::block::consensus::LaneBlockDescriptorV1;

    type Mutation = (&'static str, fn(&mut LaneBlockDescriptorV1));
    let mutations: &[Mutation] = &[
        ("incarnation", |descriptor| {
            descriptor.lane_incarnation = Hash::new(b"same-route-incarnation-drift")
        }),
        ("height", |descriptor| descriptor.lane_block_height += 1),
        ("view", |descriptor| descriptor.lane_block_view += 1),
        ("proposal", |descriptor| {
            descriptor.subject_hash = Hash::new(b"same-route-proposal-drift")
        }),
    ];
    let receipt = fixture_receipt();
    let index = leg_index(&receipt, NativeAmxParticipantApplicationRole::Coordinator);
    for &(label, mutate) in mutations {
        let mut altered = receipt.clone();
        let leg = &mut altered.legs[index];
        mutate(&mut leg.participant_proposal.descriptor);
        rebind_participant_identity(leg);
        assert_eq!(
            native_amx_participant_application_role(&altered, &altered.legs[index]),
            Err(SAME_ROUTE_DRIFT),
            "coherent participant-side {label} drift must not become a separate application",
        );
    }
}

struct ParticipantSettlementFields {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    participant_lane_block_height: u64,
    authority_context_height: u64,
    source_ids: Vec<[u8; Hash::LENGTH]>,
}

fn mutate_participant_settlement(
    leg: &mut NativeAmxLegRecordV2,
    mutate: impl FnOnce(&mut ParticipantSettlementFields),
) {
    let settlement = &leg.participant_settlement;
    let mut fields = ParticipantSettlementFields {
        lane_id: settlement.lane_id(),
        dataspace_id: settlement.dataspace_id(),
        lane_incarnation: settlement.lane_incarnation(),
        participant_lane_block_height: settlement.participant_lane_block_height(),
        authority_context_height: settlement.authority_context_height(),
        source_ids: settlement.source_ids().to_vec(),
    };
    mutate(&mut fields);
    leg.participant_settlement = NativeAmxParticipantSettlement::try_new(
        fields.lane_id,
        fields.dataspace_id,
        fields.lane_incarnation,
        fields.participant_lane_block_height,
        fields.authority_context_height,
        settlement.previous_native_settlement_hash(),
        fields.source_ids,
    )
    .expect("tampered control still satisfies intrinsic constructor invariants");
}

#[test]
fn participant_application_role_rejects_settlement_identity_and_content_tampering() {
    type Mutation = (&'static str, fn(&mut NativeAmxLegRecordV2));
    let mutations: &[Mutation] = &[
        ("lane", |leg| {
            mutate_participant_settlement(leg, |fields| fields.lane_id = LaneId::new(90))
        }),
        ("dataspace", |leg| {
            mutate_participant_settlement(leg, |fields| fields.dataspace_id = DataSpaceId::new(90))
        }),
        ("incarnation", |leg| {
            mutate_participant_settlement(leg, |fields| {
                fields.lane_incarnation = Hash::new(b"settlement-incarnation-drift")
            })
        }),
        ("height", |leg| {
            mutate_participant_settlement(leg, |fields| fields.participant_lane_block_height += 1)
        }),
        ("source", |leg| {
            mutate_participant_settlement(leg, |fields| fields.source_ids[0] = [0x11; Hash::LENGTH])
        }),
        ("authority height", |leg| {
            mutate_participant_settlement(leg, |fields| fields.authority_context_height += 1)
        }),
        ("advertised hash", |leg| {
            leg.participant_settlement_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"settlement-hash-drift"))
        }),
    ];
    let receipt = fixture_receipt();
    for index in 0..receipt.legs.len() {
        for &(label, mutate) in mutations {
            let mut altered = receipt.clone();
            mutate(&mut altered.legs[index]);
            assert_eq!(
                native_amx_participant_application_role(&altered, &altered.legs[index]),
                Err(INCONSISTENT_IDENTITY),
                "leg {index}: settlement {label} tampering must fail closed",
            );
        }
    }
}

#[test]
fn participant_application_lookup_validates_later_legs_after_an_exact_match() {
    let mut receipt = fixture_receipt();
    let matching_index = leg_index(
        &receipt,
        NativeAmxParticipantApplicationRole::SeparateParticipant,
    );
    receipt.legs.swap(0, matching_index);
    let descriptor = &receipt.legs[0].participant_proposal.descriptor;
    let route = (
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
    );
    assert_eq!(
        native_amx_receipt_requires_separate_participant_application_for(
            &receipt, route.0, route.1, route.2,
        ),
        Ok(true),
    );
    receipt.legs[1]
        .commit_qc
        .body
        .participant_previous_block_height += 1;
    for lane_id in [route.0, LaneId::new(90)] {
        assert_eq!(
            native_amx_receipt_requires_separate_participant_application_for(
                &receipt, lane_id, route.1, route.2,
            ),
            Err(INCONSISTENT_IDENTITY),
            "a malformed later leg must fail lookup even if the queried route matched or is absent",
        );
    }
}
