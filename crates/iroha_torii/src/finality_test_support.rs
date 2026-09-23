//! Genuine exact-wire finality certificates for structural Torii transport fixtures.
//! This signer does not execute State or prove economic effects.
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    NetworkId,
    block::{
        SignedBlock,
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DualQuorum, ExecutionCommitment,
            GlobalPhase, HeightContext, PROTOCOL_VERSION, QuorumCertificate, ValidatorPower,
            finality::V2FinalityArtifact,
        },
    },
};
use iroha_model_base::peer::PeerId;

/// Sign the exact canonical proposal and executed wire with three of four equal
/// validators, RS16 availability, PoPs, and the required mint-finality seals.
/// The optional parent must be the immediately preceding exact certificate.
/// These fixed fixture keys are not production trust or State execution authority.
pub(crate) fn torii_proof_finality_for_block(
    block: &SignedBlock,
    network_id: NetworkId,
    parent: Option<&V2FinalityArtifact>,
) -> V2FinalityArtifact {
    use iroha_core::zk::kagemusha_v1_recursion::{
        KagemushaMintFinalitySignerV1, build_kagemusha_mint_finality_seal_message_v1,
        derive_kagemusha_mint_finality_validator_keys_v1, sign_kagemusha_mint_finality_seal_v1,
        verify_kagemusha_mint_finality_seal_bundle_v1,
    };
    use iroha_data_model::{
        block::consensus_v2::{
            KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1, Vote,
            encode_kagemusha_consensus_signature_envelope_v1,
        },
        isi::kagemusha_v1::{
            KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalitySealBundleV1,
        },
    };
    use norito::codec::Encode as _;
    let mut keys = (1_u8..=4)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let epoch = KagemushaMintFinalityAuthorityGenerationV1 {
        version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation: 0,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| {
                let seed = 0xA0_u8 + u8::try_from(index).unwrap();
                derive_kagemusha_mint_finality_validator_keys_v1(
                    &[seed; 32],
                    0,
                    validator.validator.clone(),
                )
                .unwrap()
            })
            .collect(),
    };
    let height = block.header().height().get();
    assert_eq!(height, parent.map_or(1, |parent| parent.height + 1));
    assert_eq!(
        block.header().prev_block_hash(),
        parent.map(|parent| parent.block_hash)
    );
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: parent.map(|parent| parent.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster,
        kagemusha_mint_finality_authorization: {
            let authority = &epoch;
            let authorization = iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
                version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
                network_id: authority.network_id,
                epoch: 0,
                first_height: 1,
                last_height: 100,
                authority_generation: authority.generation,
                authority_id: authority.authority_id().expect("fixture authority identity"),
                beacon: iroha_data_model::isi::kagemusha_v1::BeaconEpochBindingV1::Bootstrap,
                previous_authorization_id: [0; 32],
                transition_id: [0; 32],
                decision: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::Genesis,
            };
            authorization.validate_against_authority(authority).expect("complete genesis fixture authorization");
            authorization
        },
        kagemusha_mint_finality_authority: epoch,
        nexus_amx_context_hash: Hash::new(b"Torii exact proof test context"),
        execution_policy_hash: Hash::new(b"Torii exact proof test execution policy"),
        da_layout: iroha_data_model::block::consensus_v2::recommended_data_availability_layout(),
        leader_seed: [0x42; 32],
    };
    context.validate().unwrap();
    let wire = block.encode_wire().unwrap();
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: block.header().view_change_index(),
    };
    let mut qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"Torii proof test prestate"),
            Hash::new(b"Torii proof test poststate"),
            Hash::new(b"Torii proof test writes"),
            u64::try_from(wire.len()).unwrap(),
            Hash::new(&wire),
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
    let signatures = qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).unwrap()].private_key(),
                &vote.signature_preimage(),
            )
            .unwrap()
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let aggregate = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
    let epoch = &context.kagemusha_mint_finality_authority;
    qc.aggregate_signature = if let Some(message) =
        build_kagemusha_mint_finality_seal_message_v1(epoch, &context, &vote).unwrap()
    {
        let seals = qc
            .signers
            .iter()
            .map(|index| {
                let seed = 0xA0_u8 + u8::try_from(*index).unwrap();
                let signer =
                    KagemushaMintFinalitySignerV1::from_seed([seed; 32].into(), *index, epoch)
                        .unwrap();
                sign_kagemusha_mint_finality_seal_v1(&signer, &message).unwrap()
            })
            .collect();
        let bundle = KagemushaMintFinalitySealBundleV1 { message, seals };
        verify_kagemusha_mint_finality_seal_bundle_v1(epoch, &context, &qc, &bundle).unwrap();
        encode_kagemusha_consensus_signature_envelope_v1(
            KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
            &aggregate,
            &bundle.encode(),
        )
        .unwrap()
    } else {
        aggregate
    };
    let pops = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, qc, pops);
    artifact
        .verify()
        .expect("genuine three-of-four BLS, PoPs and required mint-finality seals");
    artifact.validate_for_header(&block.header()).unwrap();
    artifact
}
