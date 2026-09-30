//! Native committee/checkpoint and exact reserve bindings for portable KAGEMUSHA attachments.
//!
//! These fixtures sign real three-of-four native certificates over explicit test execution
//! commitments. They do not execute World, verify paired Pasta seals, or authorize offline money.

use super::*;
use crate::{
    block::{
        CommitCertificate,
        consensus::{ExecKv, ExecWitness},
        decode_versioned_signed_block,
    },
    sumeragi_finality::{
        ExecutionCommitment, ExecutionResultCommitment, NativeLaneStateProof,
        SUMERAGI_LANE_STATE_WITNESS_KEY, SumeragiLaneStateCommitment, tests::Fixture,
    },
    sumeragi_lanes::SumeragiLaneState,
};
use iroha_allocation::AllocationBudget;
use iroha_sumeragi::{message::Qc, types::AggregateSignature};
use std::collections::BTreeMap;

/// Independently construct the complete receipt path from actual ordinary writes.
fn receipt_path(writes: &[ExecKv], key: &[u8]) -> Vec<Hash> {
    let empty = Hash::new([]);
    let mut nodes: BTreeMap<[u8; 32], Hash> = writes
        .iter()
        .map(|write| {
            let path = Hash::new(&write.key);
            let value = Hash::new(&write.value);
            (
                path.into(),
                Hash::new_from_chunks(&[&[0], path.as_ref(), value.as_ref()]),
            )
        })
        .collect();
    let mut target: [u8; 32] = Hash::new(key).into();
    let mut siblings = Vec::new();
    for bit in (0..256).rev() {
        let byte = bit / 8;
        let mask = 1 << (bit % 8);
        let mut sibling = target;
        sibling[byte] ^= mask;
        siblings.push(nodes.get(&sibling).copied().unwrap_or(empty));
        let mut parents = BTreeMap::new();
        for (path, hash) in &nodes {
            let mut other_path = *path;
            other_path[byte] ^= mask;
            let other = nodes.get(&other_path).copied().unwrap_or(empty);
            let (left, right) = if path[byte] & mask == 0 {
                (*hash, other)
            } else {
                (other, *hash)
            };
            let mut parent = *path;
            parent[byte] &= !mask;
            parents.insert(parent, ordinary_smt_node_hash(left, right));
        }
        nodes = parents;
        target[byte] &= !mask;
    }
    assert_eq!(nodes.len(), 1);
    siblings
}

fn resign_result(
    fixture: &Fixture,
    proof: &SumeragiFinalityProof,
    commitment: &ExecutionResultCommitment,
    decision_view: u64,
) -> SumeragiFinalityProof {
    let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
    let certificate = block.commit_certificate().unwrap();
    let header = certificate.consensus_header().to_vec();
    let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    qc.result = commitment.result().unwrap();
    qc.view = decision_view;
    let signatures: Vec<_> = [0, 1, 2]
        .into_iter()
        .map(|index| {
            iroha_crypto::Signature::try_new(fixture.keys[index].private_key(), &qc.preimage())
                .unwrap()
        })
        .collect();
    qc.agg_sig = AggregateSignature(
        iroha_crypto::bls_normal_aggregate_signatures(
            &signatures
                .iter()
                .map(iroha_crypto::Signature::payload)
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        header,
        norito::encode_canonical(&qc).unwrap(),
        commitment.preimage().unwrap(),
        certificate.availability().to_vec(),
    )));
    let mut proof = proof.clone();
    proof.block_wire = block.encode_wire().unwrap();
    proof
}

fn attachment(
    kind: KagemushaOperationKindV1,
) -> (
    Fixture,
    KagemushaOperationFinalityV1,
    KagemushaFinalityTrustAnchorV1,
) {
    let fixture = Fixture::new();
    let mut receipt = super::tests::reserve_receipt(kind);
    receipt.network_id = fixture.network;
    receipt.liability_pool_id = kagemusha_liability_pool_id_v1(
        &receipt.network_id,
        &receipt.asset,
        receipt.asset_incarnation,
    )
    .unwrap();
    let key = KagemushaReserveReceiptWitnessV1::expected_key(receipt.operation_id);
    let lane_state =
        SumeragiLaneStateCommitment::from_state(fixture.network, 2, &SumeragiLaneState::default())
            .unwrap();
    let witness = ExecWitness {
        writes: vec![
            ExecKv {
                key: SUMERAGI_LANE_STATE_WITNESS_KEY.to_vec(),
                value: norito::encode_canonical(&lane_state).unwrap(),
            },
            ExecKv {
                key: key.clone(),
                value: norito::encode_canonical(&receipt).unwrap(),
            },
        ],
        ..ExecWitness::default()
    };
    let reserve_receipt_witness = KagemushaReserveReceiptWitnessV1 {
        siblings: receipt_path(&witness.writes, &key),
        key,
        receipt,
    };
    let native_lanes =
        NativeLaneStateProof::from_witness(&witness, &AllocationBudget::new(100_000)).unwrap();
    let ordinary_root = native_lanes.computed_root().unwrap();
    assert!(reserve_receipt_witness.verify(ordinary_root));
    let block = decode_versioned_signed_block(&fixture.second.block_wire).unwrap();
    let mut commitment =
        ExecutionResultCommitment::decode(block.commit_certificate().unwrap().result_preimage())
            .unwrap();
    commitment.native_lanes = native_lanes;
    commitment.execution.ordinary_writes_root = ordinary_root;
    commitment.execution.post_state_root = ordinary_root;
    // This shape fixture binds the exact signed root and leaf. A portable consensus
    // verifier deliberately cannot prove the paired Poseidon membership or monetary proof.
    let top_up_membership_witness = (kind == KagemushaOperationKindV1::TopUp).then(|| {
        let receipt = &reserve_receipt_witness.receipt;
        let root = KagemushaPastaStateCommitmentV1 {
            eq: [1; 32],
            ep: [2; 32],
        };
        let top_up_root = kagemusha_mint_finality_root_v1(root);
        commitment.execution.kagemusha_top_up_count = 1;
        commitment.execution.kagemusha_top_up_root = Some(top_up_root);
        commitment.execution.post_state_root =
            ExecutionCommitment::kagemusha_post_state_root(1, ordinary_root, top_up_root);
        KagemushaTopUpMembershipWitnessV1 {
            leaf: KagemushaTopUpLeafV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                operation_id: receipt.operation_id,
                reserve_receipt_digest: receipt.canonical_digest().unwrap(),
                statement_digest: receipt.mint_statement_digest,
                amount: receipt.amount,
            },
            leaf_index: 0,
            root,
            siblings: vec![root; KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1],
        }
    });
    let proof = resign_result(&fixture, &fixture.second, &commitment, 0);
    let mut verifier = fixture.verifier();
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&proof).unwrap();
    let anchor = KagemushaFinalityTrustAnchorV1 {
        network_id: fixture.network,
        checkpoint: verifier.export_checkpoint(&proof).unwrap(),
    };
    let finality = KagemushaOperationFinalityV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: fixture.network,
        finality_proof: proof,
        reserve_receipt_witness,
        top_up_membership_witness,
    };
    (fixture, finality, anchor)
}

#[test]
fn native_finality_attachment_roundtrips_and_binds_the_complete_selected_checkpoint() {
    let (fixture, finality, anchor) = attachment(KagemushaOperationKindV1::Redemption);
    anchor.validate().unwrap();
    finality.validate_against(&anchor).unwrap();
    assert_eq!(finality.finalized_block_height(), 2);
    let identity = "iroha_data_model::isi::kagemusha_v1::KagemushaOperationFinalityV1";
    assert_eq!(
        <KagemushaOperationFinalityV1 as norito::NoritoSchema>::nominal_name(),
        identity
    );
    assert_eq!(
        <KagemushaOperationFinalityV1 as norito::NoritoSchema>::frame_name(),
        identity
    );
    let bytes = norito::encode_canonical(&finality).unwrap();
    assert_eq!(
        norito::decode_canonical::<KagemushaOperationFinalityV1>(&bytes).unwrap(),
        finality
    );
    let json = norito::json::to_vec(&finality).unwrap();
    assert_eq!(
        norito::json::from_slice::<KagemushaOperationFinalityV1>(&json).unwrap(),
        finality
    );

    let mut verifier = fixture.verifier();
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    let different_decision = KagemushaFinalityTrustAnchorV1 {
        network_id: fixture.network,
        checkpoint: verifier.export_checkpoint(&fixture.second).unwrap(),
    };
    // Same valid original committee and height, but a separately signed execution result.
    assert!(finality.validate_against(&different_decision).is_err());
    let mut foreign = anchor.clone();
    foreign.network_id =
        super::tests::reserve_receipt(KagemushaOperationKindV1::Redemption).network_id;
    assert!(foreign.validate().is_err());
    assert!(finality.validate_against(&foreign).is_err());
    let mut wrong_network = finality.clone();
    wrong_network.network_id = foreign.network_id;
    assert!(wrong_network.validate_against(&anchor).is_err());
    let mut wrong_committee = finality;
    wrong_committee.finality_proof.committee.swap(0, 1);
    assert!(wrong_committee.validate_against(&anchor).is_err());
}

#[test]
fn native_finality_refuses_unsigned_genesis_result_and_corrupt_original_certificate() {
    let (fixture, mut finality, anchor) = attachment(KagemushaOperationKindV1::Redemption);
    let mut genesis_verifier = fixture.verifier();
    genesis_verifier.verify(&fixture.first).unwrap();
    let genesis_anchor = KagemushaFinalityTrustAnchorV1 {
        network_id: fixture.network,
        checkpoint: genesis_verifier.export_checkpoint(&fixture.first).unwrap(),
    };
    let mut genesis = finality.clone();
    genesis.finality_proof = fixture.first;
    assert!(genesis.validate_against(&genesis_anchor).is_err());

    let mut block = decode_versioned_signed_block(&finality.finality_proof.block_wire).unwrap();
    let certificate = block.commit_certificate().unwrap();
    let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    qc.agg_sig.0[0] ^= 1;
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        certificate.consensus_header().to_vec(),
        norito::encode_canonical(&qc).unwrap(),
        certificate.result_preimage().to_vec(),
        certificate.availability().to_vec(),
    )));
    finality.finality_proof.block_wire = block.encode_wire().unwrap();
    assert!(finality.validate_against(&anchor).is_err());
}

#[test]
fn native_receipt_membership_rejects_each_monetary_field_key_and_path_substitution() {
    let (_, finality, anchor) = attachment(KagemushaOperationKindV1::Redemption);
    let mutate: &[fn(&mut KagemushaReserveReceiptWitnessV1)] = &[
        |w| w.receipt.operation_id[0] ^= 1,
        |w| w.receipt.request_digest[0] ^= 1,
        |w| w.receipt.amount += 1,
        |w| w.receipt.total_topups += 1,
        |w| w.receipt.total_redemptions += 1,
        |w| w.receipt.previous_pool_receipt_digest[0] ^= 1,
        |w| w.receipt.transaction_hash[0] ^= 1,
        |w| w.receipt.committed_at_ms += 1,
        |w| w.receipt.scale += 1,
        |w| {
            w.receipt.asset = AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("foreign", "universal").unwrap(),
                "xor".parse().unwrap(),
            );
        },
        |w| {
            w.receipt.asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"another actual asset incarnation").as_ref(),
            )
            .unwrap();
        },
        |w| w.receipt.liability_pool_id[0] ^= 1,
        |w| w.key[0] ^= 1,
        |w| w.siblings[0] = Hash::new(b"another leaf"),
        |w| {
            w.siblings.pop();
        },
    ];
    for mutation in mutate {
        let mut changed = finality.clone();
        mutation(&mut changed.reserve_receipt_witness);
        assert!(changed.validate_against(&anchor).is_err());
    }
}

#[test]
fn native_top_up_attachment_preserves_exact_root_count_and_leaf_bindings() {
    let (_, finality, anchor) = attachment(KagemushaOperationKindV1::TopUp);
    finality.validate_against(&anchor).unwrap();
    let mutate: &[fn(&mut KagemushaTopUpMembershipWitnessV1)] = &[
        |w| w.root.eq[0] ^= 1,
        |w| w.root.ep[0] ^= 1,
        |w| w.leaf.operation_id[0] ^= 1,
        |w| w.leaf.reserve_receipt_digest[0] ^= 1,
        |w| w.leaf.statement_digest[0] ^= 1,
        |w| w.leaf.amount += 1,
        |w| w.leaf_index = 1,
        |w| {
            w.siblings.pop();
        },
    ];
    for mutation in mutate {
        let mut changed = finality.clone();
        mutation(changed.top_up_membership_witness.as_mut().unwrap());
        assert!(changed.validate_against(&anchor).is_err());
    }
    let mut missing = finality.clone();
    missing.top_up_membership_witness = None;
    assert!(missing.validate_against(&anchor).is_err());
    let (_, mut redemption, redemption_anchor) = attachment(KagemushaOperationKindV1::Redemption);
    redemption.top_up_membership_witness = finality.top_up_membership_witness;
    assert!(redemption.validate_against(&redemption_anchor).is_err());
}

#[test]
fn native_finality_accepts_another_valid_quorum_witness_for_the_same_selected_decision() {
    let (fixture, mut finality, anchor) = attachment(KagemushaOperationKindV1::Redemption);
    let block = decode_versioned_signed_block(&finality.finality_proof.block_wire).unwrap();
    let commitment =
        ExecutionResultCommitment::decode(block.commit_certificate().unwrap().result_preimage())
            .unwrap();
    let original = finality.finality_proof.block_wire.clone();
    finality.finality_proof = resign_result(&fixture, &finality.finality_proof, &commitment, 1);
    assert_ne!(original, finality.finality_proof.block_wire);
    finality.validate_against(&anchor).unwrap();
}

#[test]
fn native_finality_attachment_rejects_the_removed_artifact_json_field() {
    let (_, finality, _) = attachment(KagemushaOperationKindV1::Redemption);
    let json = norito::json::to_json(&finality).unwrap();
    let with_retired_field = format!("{{\"finality_artifact\":null,{}", &json[1..]);
    assert!(norito::json::from_str::<KagemushaOperationFinalityV1>(&with_retired_field).is_err());
}
