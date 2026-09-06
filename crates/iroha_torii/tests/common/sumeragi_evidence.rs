//! Signed four-validator evidence fixtures shared by the telemetry endpoints.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, bls_normal_pop_prove};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus::{Evidence, SumeragiV2EquivocationEvidence},
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DualQuorum, ExecutionCommitment,
            GlobalPhase, HeightContext, PROTOCOL_VERSION, SnapshotBootstrapAnchor,
            SumeragiV2Equivocation, ValidatorPower, Vote, recommended_data_availability_layout,
        },
    },
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
        KagemushaMintFinalityValidatorKeysV1,
    },
    peer::PeerId,
};

/// Build conflicting Prepare votes with aligned mint-finality keys and valid BLS proofs.
pub(super) fn make_phase_vote_evidence(height: u64, seed: u8) -> Evidence {
    assert!(height > 0);
    let mut keys = (0..4_u8)
        .map(|index| {
            KeyPair::try_from_seed(vec![seed, index], Algorithm::BlsNormal)
                .expect("derive evidence fixture key")
        })
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([seed; Hash::LENGTH]),
    ));
    let mint_roster = KagemushaMintFinalityEpochRosterV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        epoch: 0,
        validators: roster
            .iter()
            .zip(1..=4_u8)
            .map(|(validator, index)| KagemushaMintFinalityValidatorKeysV1 {
                validator: validator.validator.clone(),
                // Prepare evidence authenticates these committed key records without a
                // Commit-vote mint-finality seal. Follow the shared wire fixture encoding.
                eq_proof_public_key: [index; 32],
                ep_proof_public_key: [index + 16; 32],
            })
            .collect(),
    };
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        kagemusha_mint_finality_epoch_id: mint_roster
            .finality_epoch_id()
            .expect("valid fixture mint-finality roster"),
        kagemusha_mint_finality_epoch_roster: mint_roster,
        epoch_end_height: height.checked_add(1).expect("nonterminal fixture height"),
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: (height > 1).then(|| SnapshotBootstrapAnchor {
            snapshot_height: height - 1,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 0])),
            snapshot_block_creation_time_ms: 0,
            snapshot_state_hash: Hash::new([seed, 1]),
        }),
        quorum: DualQuorum::from_roster(&roster).expect("four-validator fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"evidence nexus context"),
        execution_policy_hash: Hash::new(b"evidence execution policy"),
        da_layout: recommended_data_availability_layout(),
        leader_seed: [seed; Hash::LENGTH],
    };
    context.validate().expect("valid evidence height context");
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: 0,
    };
    let execution_commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"evidence parent state"),
        Hash::new(b"evidence post state"),
        Hash::new(b"evidence ordinary writes"),
        1,
        Hash::new([seed]),
    );
    let vote = |subject_seed: u8| {
        let mut vote = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Prepare,
            subject: BlockSubject {
                parent_block_hash: context
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash),
                block_hash: HashOf::from_untyped_unchecked(Hash::new([subject_seed, 2])),
                payload_hash: Hash::new([subject_seed, 3]),
            },
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        vote
    };
    Evidence {
        equivocation: SumeragiV2EquivocationEvidence {
            conflict: SumeragiV2Equivocation::PhaseVote {
                first: vote(seed),
                second: vote(seed.wrapping_add(1)),
            },
            context,
            proofs_of_possession: keys
                .iter()
                .map(|key| bls_normal_pop_prove(key.private_key()).expect("fixture BLS proof"))
                .collect(),
        },
    }
}

#[test]
fn evidence_fixture_authenticates_and_rejects_modified_signed_bytes() {
    use iroha_crypto::bls_normal_pop_verify;

    for height in [1, 2, 30] {
        let mut evidence = make_phase_vote_evidence(height, 0xA1);
        let context = &evidence.equivocation.context;
        context
            .validate()
            .expect("structurally valid fixture context");
        assert_eq!(context.roster.len(), 4);
        assert_eq!(evidence.equivocation.proofs_of_possession.len(), 4);
        for (validator, proof) in context
            .roster
            .iter()
            .zip(&evidence.equivocation.proofs_of_possession)
        {
            bls_normal_pop_verify(validator.validator.public_key(), proof)
                .expect("valid roster-aligned proof of possession");
        }
        let SumeragiV2Equivocation::PhaseVote { first, second } =
            &mut evidence.equivocation.conflict
        else {
            panic!("fixture must contain phase votes");
        };
        assert_eq!(first.round, second.round);
        assert_eq!(first.signer, second.signer);
        assert_ne!(first.subject, second.subject);
        let public_key = context.roster[0].validator.public_key();
        for vote in [&*first, &*second] {
            vote.validate(context).expect("structurally valid vote");
            Signature::from_bytes(&vote.signature)
                .verify(public_key, &vote.signature_preimage())
                .expect("vote authenticates its exact preimage");
        }
        second.subject.payload_hash = Hash::new(b"unsigned replacement");
        assert!(
            Signature::from_bytes(&second.signature)
                .verify(public_key, &second.signature_preimage())
                .is_err(),
        );
    }
}
