//! Genuine committee-possession, incumbent-certificate and contiguous-finality fixtures.

use super::*;
use crate::{
    kagemusha_v1_test_fixtures::mint_finality_authorization,
    state::{WorldReadOnly as _, threshold_key_lifecycle_certificate_preimage_v1},
    zk::kagemusha_v1_recursion::{
        KagemushaMintFinalitySignerV1, build_kagemusha_mint_finality_seal_message_v1,
        sign_kagemusha_mint_finality_seal_v1,
    },
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{
            self as wire, BlockSubject, ConsensusMode, ConsensusRound, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, QuorumCertificate, ValidatorPower,
            Vote,
            finality::{FinalizedNextEpochSnapshot, V2FinalityArtifact},
        },
    },
    bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
    isi::{
        consensus_keys::ThresholdKeyLifecycleSignatureV1,
        kagemusha_v1::KagemushaMintFinalitySealBundleV1,
    },
    nexus::ValidatorCommitteeSelectionStatusV1,
};
use mv::storage::StorageReadOnly as _;
use std::num::NonZeroU64;

fn signed_artifact(
    header: &BlockHeader,
    context: HeightContext,
    keys: &[KeyPair],
) -> V2FinalityArtifact {
    context.validate().unwrap();
    let subject = BlockSubject {
        parent_block_hash: header.prev_block_hash(),
        block_hash: header.hash(),
        payload_hash: Hash::new(b"custody fixture proposal"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"pre"),
            Hash::new(b"post"),
            Hash::new(b"writes"),
            1,
            Hash::new(b"wire"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    let vote = Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: qc.execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &vote.signature_preimage())
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let aggregate = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
    qc.aggregate_signature = if let Some(message) = build_kagemusha_mint_finality_seal_message_v1(
        &context.kagemusha_mint_finality_authority,
        &context,
        &vote,
    )
    .unwrap()
    {
        let seals = (0..3)
            .map(|index| {
                let signer = KagemushaMintFinalitySignerV1::from_seed(
                    [0xA0 + index as u8; 32].into(),
                    index,
                    &context.kagemusha_mint_finality_authority,
                )
                .unwrap();
                sign_kagemusha_mint_finality_seal_v1(&signer, &message).unwrap()
            })
            .collect();
        let bundle = KagemushaMintFinalitySealBundleV1 { message, seals };
        wire::encode_kagemusha_consensus_signature_envelope_v1(
            wire::KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
            &aggregate,
            &bundle.encode(),
        )
        .unwrap()
    } else {
        aggregate
    };
    let artifact = V2FinalityArtifact::new(
        context,
        subject,
        qc,
        keys.iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect(),
    );
    artifact.verify().unwrap();
    artifact.validate_for_header(header).unwrap();
    artifact
}

fn evidence_fixture() -> ValidatorCommitteeProvisioningEvidenceV1 {
    let mut headers: Vec<BlockHeader> = Vec::new();
    for height in 1..=14 {
        headers.push(BlockHeader::new(
            NonZeroU64::new(height).unwrap(),
            headers.last().map(BlockHeader::hash),
            None,
            height,
            0,
        ));
    }
    let fixture = crate::state::validator_committee::tests::fixture_with_selection_anchor(
        7,
        headers[8].hash(),
    );
    let network = fixture.authorization.network_id;
    let prep = &fixture.transition.preparation;
    let mut keys = (1..=7_u8)
        .map(|index| KeyPair::from_seed(vec![index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    keys.truncate(4);
    let roster = fixture
        .incumbent
        .validators
        .iter()
        .map(|seat| ValidatorPower {
            validator: seat.validator.clone(),
            power: 1,
        })
        .collect::<Vec<_>>();
    let pops = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
        .collect::<Vec<_>>();
    let selecting = FinalizedNextEpochSnapshot {
        committee_preparation: Some(prep.clone()),
        epoch: 1,
        kagemusha_mint_finality_authorization: fixture.authorization,
        kagemusha_mint_finality_authority: fixture.incumbent.clone(),
        epoch_end_height: 20,
        mode: ConsensusMode::Npos,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster: roster.clone(),
        validator_set_pops: pops,
        leader_seed: [0x51; 32],
    };
    let mut chain: Vec<BridgeFinalityProof> = Vec::new();
    for header in headers {
        let height = header.height().get();
        let previous = chain.last();
        let context = HeightContext {
            network_id: network,
            protocol_version: wire::PROTOCOL_VERSION,
            height,
            epoch: if height <= 10 { 0 } else { 1 },
            epoch_end_height: if height <= 10 { 10 } else { 20 },
            next_epoch_snapshot: (height == 10).then(|| selecting.clone()),
            mode: ConsensusMode::Npos,
            parent_commit_qc: previous.map(|proof| proof.finality_artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).unwrap(),
            roster: roster.clone(),
            kagemusha_mint_finality_authority: fixture.incumbent.clone(),
            kagemusha_mint_finality_authorization: if height <= 10 {
                mint_finality_authorization(&fixture.incumbent, 0, 1, 10)
            } else {
                fixture.authorization
            },
            nexus_amx_context_hash: Hash::new(b"custody amx"),
            execution_policy_hash: Hash::new(b"custody policy"),
            da_layout: wire::recommended_data_availability_layout(),
            leader_seed: [0x51; 32],
        };
        chain.push(BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            finality_artifact: signed_artifact(&header, context, &keys),
            block_header: header,
        });
    }
    let chain = chain.split_off(9);
    let view = fixture.world.view();
    let record = view
        .global_beacon_key_sessions()
        .get(&prep.beacon_session_id().unwrap())
        .unwrap();
    let BeaconEpochBindingV1::Installed(active) = fixture.authorization.beacon else {
        unreachable!()
    };
    let mut certificate = ThresholdKeyLifecycleCertificateV1 {
        version: 1,
        action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        expected_active_session_id: Some(active.session_id),
        effective_height: 14,
        network_id: network,
        roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(
            &roster
                .iter()
                .map(|row| row.validator.clone())
                .collect::<Vec<_>>(),
        ),
        committee_size: 4,
        quorum: 3,
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
        public_state: norito::encode_canonical(record).unwrap(),
        signatures: Vec::new(),
    };
    let preimage = threshold_key_lifecycle_certificate_preimage_v1(&certificate).unwrap();
    certificate.signatures = keys[..3]
        .iter()
        .enumerate()
        .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
            signer_index: index as u16,
            signature: Signature::new(key.private_key(), &preimage),
        })
        .collect();
    let candidates = prep
        .roster
        .iter()
        .map(|seat| {
            view.validator_candidate_keys()
                .get(&ValidatorCandidateKeysV1::key_id(
                    network,
                    1,
                    &seat.validator,
                ))
                .unwrap()
                .clone()
        })
        .collect();
    ValidatorCommitteeProvisioningEvidenceV1 {
        status: ValidatorCommitteeStatusV1 {
            network_id: network,
            target_epoch: 2,
            latest_finality: chain.last().unwrap().finality_artifact.clone(),
            selected: Some(ValidatorCommitteeSelectionStatusV1 {
                transition: fixture.transition.clone(),
                selecting_finality: chain[0].finality_artifact.clone(),
            }),
            candidate_keys: candidates,
            pending_beacon_session: Some(record.session.clone()),
        },
        finality_chain: chain,
        beacon_finalization: certificate,
    }
}

fn selection_evidence_fixture() -> ValidatorCommitteeSelectionEvidenceV1 {
    let fixture = evidence_fixture();
    let mut status = fixture.status;
    let selected = status.selected.as_mut().expect("frozen selection fixture");
    selected.transition.credentials = None;
    selected.transition.readiness.clear();
    status.candidate_keys.clear();
    status.pending_beacon_session = None;
    ValidatorCommitteeSelectionEvidenceV1 {
        status,
        finality_chain: fixture.finality_chain,
    }
}

#[test]
fn selection_evidence_authenticates_only_frozen_roster_before_any_dkg_credentials() {
    let evidence = selection_evidence_fixture();
    let network = evidence.status.network_id;
    let context = evidence.finality_chain[0].finality_artifact.context_id();
    let preparation = &evidence
        .status
        .selected
        .as_ref()
        .expect("selection")
        .transition
        .preparation;
    let attempt = preparation.transition_id().unwrap();
    let verify = |evidence: &ValidatorCommitteeSelectionEvidenceV1| {
        verify_validator_committee_selection_evidence_v1(evidence, network, context, 10, 2, attempt)
    };
    let selected = verify(&evidence).expect("contiguous incumbent-certified E+1 selection");
    assert_eq!(selected.preparation(), preparation);
    assert_eq!(selected.observed_height(), 14);
    assert_eq!(selected.incumbent_authority().validators.len(), 4);
    assert_ne!(
        selected.incumbent_beacon().session_id,
        selected.preparation().beacon_session_id().unwrap()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &ValidatorCommitteeProvisioningEvidenceV1 {
                status: evidence.status.clone(),
                finality_chain: evidence.finality_chain.clone(),
                beacon_finalization: evidence_fixture().beacon_finalization,
            },
            network,
            context,
            10,
            2,
            attempt,
        )
        .is_err(),
        "selection-only proof cannot authorize private-share import"
    );
    let binary = norito::encode_canonical(&evidence).unwrap();
    assert_eq!(
        norito::decode_canonical::<ValidatorCommitteeSelectionEvidenceV1>(&binary).unwrap(),
        evidence
    );
    let json = norito::json::to_vec(&evidence).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeSelectionEvidenceV1>(&json).unwrap(),
        evidence
    );
    for mutation in 0..5 {
        let mut changed = evidence.clone();
        match mutation {
            0 => {
                changed
                    .status
                    .selected
                    .as_mut()
                    .unwrap()
                    .transition
                    .preparation
                    .election_seed[0] ^= 1
            }
            1 => changed.status.latest_finality.height += 1,
            2 => {
                changed.finality_chain.remove(1);
            }
            3 => {
                changed
                    .status
                    .selected
                    .as_mut()
                    .unwrap()
                    .selecting_finality
                    .height += 1
            }
            _ => {
                changed.finality_chain[0]
                    .finality_artifact
                    .height_context
                    .next_epoch_snapshot
                    .as_mut()
                    .unwrap()
                    .committee_preparation
                    .as_mut()
                    .unwrap()
                    .election_seed[0] ^= 1
            }
        }
        assert!(verify(&changed).is_err(), "selection mutation {mutation}");
    }
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence, network, context, 11, 2, attempt
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence, network, context, 10, 3, attempt
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence, network, context, 10, 2, [0xEE; 32]
        )
        .is_err()
    );
}

#[test]
fn committee_custody_evidence_authenticates_exact_pending_attempt_and_rejects_substitution() {
    let evidence = evidence_fixture();
    let network = evidence.status.network_id;
    let context = evidence.finality_chain[0].finality_artifact.context_id();
    let attempt = evidence
        .status
        .selected
        .as_ref()
        .unwrap()
        .transition
        .preparation
        .transition_id()
        .unwrap();
    let verify = |evidence: &ValidatorCommitteeProvisioningEvidenceV1| {
        verify_validator_committee_provisioning_evidence_v1(
            evidence, network, context, 10, 2, attempt,
        )
    };
    let accepted = verify(&evidence).unwrap();
    assert_eq!(accepted.observed_height(), 14);
    assert_eq!(
        accepted.session(),
        evidence.status.pending_beacon_session.as_ref().unwrap()
    );
    assert_eq!(accepted.transition().preparation.target_epoch, 2);
    assert_eq!(accepted.incumbent_authority().validators.len(), 4);
    assert_ne!(
        accepted.incumbent_beacon().session_id,
        accepted.session().session_id
    );
    let binary = norito::encode_canonical(&evidence).unwrap();
    assert_eq!(
        norito::decode_canonical::<ValidatorCommitteeProvisioningEvidenceV1>(&binary).unwrap(),
        evidence
    );
    let json = norito::json::to_vec(&evidence).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeProvisioningEvidenceV1>(&json).unwrap(),
        evidence
    );
    for mutation in 0..8 {
        let mut changed = evidence.clone();
        match mutation {
            0 => changed
                .beacon_finalization
                .signatures
                .pop()
                .map(|_| ())
                .unwrap(),
            1 => changed.beacon_finalization.expected_active_session_id = Some([0xEE; 32]),
            2 => changed.beacon_finalization.transcript_hash[0] ^= 1,
            3 => {
                changed.finality_chain.remove(1);
            }
            4 => changed.status.candidate_keys.swap(0, 1),
            5 => {
                changed
                    .status
                    .pending_beacon_session
                    .as_mut()
                    .unwrap()
                    .session_id[0] ^= 1
            }
            6 => {
                changed
                    .status
                    .selected
                    .as_mut()
                    .unwrap()
                    .selecting_finality
                    .subject
                    .block_hash =
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"substituted"))
            }
            _ => changed.status.latest_finality.height += 1,
        }
        assert!(verify(&changed).is_err(), "mutation {mutation}");
    }
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence, network, context, 11, 2, attempt
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence, network, context, 10, 3, attempt
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence, network, context, 10, 2, [0xEE; 32]
        )
        .is_err()
    );
}
