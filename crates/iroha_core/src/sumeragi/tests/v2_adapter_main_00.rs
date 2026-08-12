use std::{fs::OpenOptions, io::Write as _, time::Duration};

use iroha_crypto::{Algorithm, HashOf, KeyPair};
use tempfile::TempDir;

use super::super::serviced_candidate_store::ProducerContinuationSourceClass;
use super::*;
use crate::sumeragi::v2_chunks::encode_payload;

fn test_network_id(seed: u8) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(Hash::prehashed(
        [seed; Hash::LENGTH],
    )))
}

#[derive(Debug)]
struct TestAggregator;

impl SignatureAggregator for TestAggregator {
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String> {
        let mut aggregate = Vec::new();
        for signature in signatures {
            aggregate.extend_from_slice(
                &u32::try_from(signature.len())
                    .map_err(|error| error.to_string())?
                    .to_le_bytes(),
            );
            aggregate.extend_from_slice(signature);
        }
        Ok(aggregate)
    }
}

fn peer(seed: u8) -> PeerId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("deterministic peer key");
    PeerId::new(key.public_key().clone())
}

fn context() -> wire::HeightContext {
    let mut roster = (1_u8..=4)
        .map(|seed| wire::ValidatorPower {
            validator: peer(seed),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort();
    wire::HeightContext {
        network_id: test_network_id(0x61),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 1,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 512 * 1024,
            max_chunk_count: 1024,
        },
        leader_seed: [0xA5; 32],
    }
}

fn verified_genesis(context: wire::HeightContext) -> VerifiedHeightContext {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert!(
        keys.iter()
            .zip(&context.roster)
            .all(|(key, entry)| key.public_key() == entry.validator.public_key())
    );
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS proof of possession")
        })
        .collect();
    VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context")
}

include!("v2_adapter_activation_context.rs");

#[cfg(feature = "bls")]
fn authenticated_context() -> (wire::HeightContext, Vec<KeyPair>, Vec<Vec<u8>>) {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS proof of possession")
        })
        .collect::<Vec<_>>();
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = wire::HeightContext {
        network_id: test_network_id(0x62),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 3,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"authenticated nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 512 * 1024,
            max_chunk_count: 1024,
        },
        leader_seed: [0x5A; 32],
    };
    (context, keys, pops)
}

#[cfg(feature = "bls")]
fn authenticate_qc(certificate: &mut wire::QuorumCertificate, keys: &[KeyPair]) {
    let signer = certificate
        .signers
        .first()
        .copied()
        .expect("fixture certificate has signers");
    let preimage = wire::Vote {
        round: certificate.round,
        proposal_round: certificate.proposal_round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = certificate
        .signers
        .iter()
        .map(|signer| {
            let index = usize::try_from(*signer).expect("small fixture signer index");
            Signature::new(keys[index].private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate fixture certificate");
}

#[cfg(feature = "bls")]
#[test]
fn height_context_rejects_missing_and_rogue_proofs_of_possession() {
    let (context, _keys, mut proofs) = authenticated_context();
    assert!(matches!(
        VerifiedHeightContext::genesis(context.clone(), proofs[..3].to_vec()),
        Err(AdapterError::ProofOfPossessionCount {
            expected: 4,
            actual: 3
        })
    ));
    proofs.swap(0, 1);
    assert!(matches!(
        VerifiedHeightContext::genesis(context, proofs),
        Err(AdapterError::Cryptography(_))
    ));
}

#[cfg(feature = "bls")]
#[test]
fn aggregate_verification_rejects_signer_without_aligned_pop() {
    let (context, _keys, proofs) = authenticated_context();
    let signer = u32::try_from(context.roster.len() - 1).expect("small fixture roster");

    assert!(matches!(
        verify_aggregate_signature(
            &context,
            &[signer],
            &[],
            b"missing aligned proof of possession",
            &proofs[..proofs.len() - 1],
        ),
        Err(AdapterError::ValidatorIndexOutOfRange(index)) if index == signer
    ));
}

#[cfg(feature = "bls")]
#[test]
fn boundary_context_rejects_missing_invalid_and_foreign_future_pops_before_voting() {
    let (mut context, _keys, proofs) = authenticated_context();
    context.epoch_end_height = context.height;
    context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
        epoch: context.epoch + 1,
        epoch_end_height: context.height + 10,
        mode: context.mode,
        roster: context.roster.clone(),
        validator_set_pops: proofs.clone(),
        quorum: context.quorum,
        leader_seed: [0x6A; 32],
    });
    VerifiedHeightContext::genesis(context.clone(), proofs.clone())
        .expect("valid future PoPs are admitted before voting");

    let mut missing = context.clone();
    missing
        .next_epoch_snapshot
        .as_mut()
        .expect("boundary snapshot")
        .validator_set_pops
        .pop();
    assert!(matches!(
        VerifiedHeightContext::genesis(missing, proofs.clone()),
        Err(AdapterError::WireValidation(
            wire::ValidationError::NextEpochProofOfPossessionCount
        ))
    ));

    let foreign_key =
        KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).expect("foreign BLS key");
    let foreign_pop =
        iroha_crypto::bls_normal_pop_prove(foreign_key.private_key()).expect("foreign PoP");
    let mut foreign = context.clone();
    foreign
        .next_epoch_snapshot
        .as_mut()
        .expect("boundary snapshot")
        .validator_set_pops[0] = foreign_pop;
    assert!(matches!(
        VerifiedHeightContext::genesis(foreign, proofs.clone()),
        Err(AdapterError::Cryptography(_))
    ));

    let mut corrupted = context;
    corrupted
        .next_epoch_snapshot
        .as_mut()
        .expect("boundary snapshot")
        .validator_set_pops[0][0] ^= 0x80;
    assert!(matches!(
        VerifiedHeightContext::genesis(corrupted, proofs),
        Err(AdapterError::Cryptography(_))
    ));
}

#[cfg(feature = "bls")]
#[test]
fn successor_context_requires_the_durable_cryptographic_parent() {
    let (parent_context, keys, proofs) = authenticated_context();
    let parent_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent block")),
        payload_hash: Hash::new(b"parent payload"),
    };
    let round = wire::ConsensusRound {
        context_id: parent_context.id(),
        height: parent_context.height,
        view: 0,
    };
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x21),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let parent_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x21),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate parent CommitQC"),
    };
    let artifact = wire::finality::V2FinalityArtifact::new(
        parent_context.clone(),
        parent_subject,
        parent_qc.clone(),
        proofs.clone(),
    );
    artifact.validate().expect("valid parent artifact");
    let receipt = KuraV2CommitReceipt::for_test(&artifact);
    let mut successor = parent_context.clone();
    successor.height = 2;
    successor.parent_commit_qc = Some(parent_qc.clone());

    let verified_successor = VerifiedHeightContext::successor(
        successor.clone(),
        proofs.clone(),
        &artifact,
        &receipt,
        &proofs,
    )
    .expect("durable verified parent anchors successor");
    assert_eq!(
        verified_successor.verified_predecessor_context(),
        Some(&parent_context)
    );

    let mut substituted_execution_policy = successor.clone();
    substituted_execution_policy.execution_policy_hash =
        Hash::new(b"substituted successor execution policy");
    assert!(matches!(
        VerifiedHeightContext::successor(
            substituted_execution_policy,
            proofs.clone(),
            &artifact,
            &receipt,
            &proofs,
        ),
        Err(AdapterError::ParentContextMismatch)
    ));

    let mut substituted_successor_pops = proofs.clone();
    substituted_successor_pops.swap(0, 1);
    assert!(matches!(
        VerifiedHeightContext::successor(
            successor.clone(),
            substituted_successor_pops,
            &artifact,
            &receipt,
            &proofs,
        ),
        Err(AdapterError::EpochTransitionMismatch)
    ));

    let mut substituted_parent_artifact = artifact.clone();
    substituted_parent_artifact.validator_set_pops.swap(0, 1);
    let substituted_receipt = KuraV2CommitReceipt::for_test(&substituted_parent_artifact);
    assert!(matches!(
        VerifiedHeightContext::successor(
            successor.clone(),
            proofs.clone(),
            &substituted_parent_artifact,
            &substituted_receipt,
            &proofs,
        ),
        Err(AdapterError::ParentContextMismatch)
    ));

    // The same parent decision can acquire a valid CommitQC in another
    // view. Semantic proposal admission accepts it, but the authentication
    // boundary must still verify that alternate certificate under the
    // retained parent roster rather than trusting the leader signature.
    let alternate_round = wire::ConsensusRound {
        view: round.view + 1,
        ..round
    };
    let alternate_preimage = wire::Vote {
        round: alternate_round,
        proposal_round: alternate_round,
        phase: wire::GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x21),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let alternate_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &alternate_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let alternate_refs = alternate_shares
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let alternate_parent_qc = wire::QuorumCertificate {
        round: alternate_round,
        proposal_round: alternate_round,
        phase: wire::GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: execution_commitment(0x21),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&alternate_refs)
            .expect("aggregate alternate parent CommitQC"),
    };
    let proposal_round = wire::ConsensusRound {
        context_id: successor.id(),
        height: successor.height,
        view: 0,
    };
    let mut proposal_subject = subject(0x72);
    let proposal_body = b"parent-auth-body".to_vec();
    proposal_subject.payload_hash = Hash::new(&proposal_body);
    let manifest = encode_payload(&successor, proposal_round, proposal_subject, &proposal_body)
        .expect("encode successor fixture payload")
        .manifest()
        .clone();
    let proposer = successor.leader(0);
    let mut proposal = wire::Proposal {
        round: proposal_round,
        proposer,
        subject: proposal_subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: Some(alternate_parent_qc),
        }),
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("successor-safety.wal"),
        verified_successor.clone(),
        None,
        reducer::Generation::new(2),
        [0x62; 32],
        fingerprints(),
        Box::new(TestAggregator),
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open successor adapter");
    assert!(startup.is_empty());
    let authenticated = adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("alternate-view parent CommitQC is cryptographically verified");

    let mut alternate_registry = adapter.registry.clone();
    alternate_registry
        .proposal_to_core(&proposal, &successor)
        .expect("alternate-view parent CommitQC retains the durable parent decision");

    let foreign_parent_subject = subject(0x73);
    let foreign_parent_commitment = execution_commitment(0x73);
    let foreign_preimage = wire::Vote {
        round: alternate_round,
        proposal_round: alternate_round,
        phase: wire::GlobalPhase::Commit,
        subject: foreign_parent_subject,
        execution_commitment: foreign_parent_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let foreign_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &foreign_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let foreign_refs = foreign_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let foreign_parent_qc = wire::QuorumCertificate {
        round: alternate_round,
        proposal_round: alternate_round,
        phase: wire::GlobalPhase::Commit,
        subject: foreign_parent_subject,
        execution_commitment: foreign_parent_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&foreign_refs)
            .expect("aggregate foreign parent CommitQC"),
    };
    let mut retargeted_proposal = proposal.clone();
    let wire::ProposalJustification::ParentCommit(parent) = &mut retargeted_proposal.justification
    else {
        unreachable!("fixture carries a parent certificate")
    };
    parent.certificate = Some(foreign_parent_qc);
    retargeted_proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &retargeted_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let retargeted_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(retargeted_proposal),
    );
    assert!(matches!(
        adapter.authenticate(retargeted_message.clone()),
        Err(AdapterError::WireValidation(
            wire::ValidationError::InvalidProposalJustification
        ))
    ));
    let registry_before_retargeting = adapter.registry.clone();
    // Exercise the staged conversion defense directly. Production ingress
    // reaches this only after `authenticate`, whose structural check above
    // already rejects the retargeting; the inner conversion must still be
    // fail-closed if a caller violates that private precondition.
    assert!(matches!(
        adapter.receive_verified(retargeted_message),
        Err(AdapterError::ParentContextMismatch)
    ));
    assert_registry_eq(&adapter.registry, &registry_before_retargeting);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());

    let admitted = adapter
        .receive_authenticated(authenticated)
        .expect("parent CommitQC remains bound to the predecessor during conversion");
    assert!(matches!(
        admitted.effects(),
        [AdapterEffect::FetchBody { manifest: Some(manifest), .. }]
            if manifest.round == proposal.round && manifest.subject == proposal.subject
    ));
    assert!(matches!(
        verify_authenticated_message(
            &successor,
            None,
            &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
            &proofs,
        ),
        Err(AdapterError::ParentContextMismatch)
    ));

    if let wire::ProposalJustification::ParentCommit(parent) = &mut proposal.justification {
        parent
            .certificate
            .as_mut()
            .expect("alternate parent certificate")
            .aggregate_signature[0] ^= 0x20;
    } else {
        unreachable!("fixture carries a parent certificate")
    }
    proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal),
        )),
        Err(AdapterError::Cryptography(_))
    ));

    successor
        .parent_commit_qc
        .as_mut()
        .expect("parent QC")
        .aggregate_signature[0] ^= 0x80;
    assert!(matches!(
        VerifiedHeightContext::successor(successor, proofs.clone(), &artifact, &receipt, &proofs,),
        Err(AdapterError::Cryptography(_))
    ));

    let mut different_artifact = artifact.clone();
    different_artifact.commit_qc.aggregate_signature[0] ^= 0x40;
    let wrong_receipt = KuraV2CommitReceipt::for_test(&different_artifact);
    let mut successor = parent_context;
    successor.height = 2;
    successor.parent_commit_qc = Some(parent_qc);
    assert!(matches!(
        VerifiedHeightContext::successor(
            successor,
            proofs.clone(),
            &artifact,
            &wrong_receipt,
            &proofs,
        ),
        Err(AdapterError::ParentContextMismatch)
    ));
}

fn fingerprints() -> AdapterFingerprints {
    AdapterFingerprints {
        node: Hash::new(b"node"),
        build: Hash::new(b"build"),
        config: Hash::new(b"config"),
    }
}

fn subject(byte: u8) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([byte, 0]))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new([byte, 1])),
        payload_hash: Hash::new([byte, 2]),
    }
}

fn execution_commitment(byte: u8) -> wire::ExecutionCommitment {
    wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new([byte, 3]),
        Hash::new([byte, 4]),
        Hash::new([byte, 5]),
        1,
        Hash::new([byte, 6]),
    )
}

#[test]
fn commit_qc_status_reports_equal_vote_projection_in_npos_mode() {
    let mut context = context();
    context.mode = wire::ConsensusMode::Npos;
    let certificate = wire::QuorumCertificate {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        proposal_round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        phase: wire::GlobalPhase::Commit,
        subject: subject(0x31),
        execution_commitment: execution_commitment(0x31),
        signers: vec![0, 2, 3],
        aggregate_signature: vec![0xA5; 48],
    };

    let summary = commit_qc_status(&certificate, &context).expect("valid CommitQC summary");

    assert_eq!(summary.certificate, certificate.as_ref());
    assert_eq!(summary.validator_count, 4);
    assert_eq!(summary.signer_count, 3);
    assert_eq!(summary.min_signers, 3);
    assert_eq!(summary.signed_power, 3);
    assert_eq!(summary.total_power, 4);
}

#[test]
fn vote_body_ownership_uses_the_authenticated_proposal_origin() {
    let context = context();
    let proposal_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 1,
    };
    let finality_round = wire::ConsensusRound {
        view: 3,
        ..proposal_round
    };
    let request = SignRequest::Vote(wire::Vote {
        round: finality_round,
        proposal_round,
        phase: wire::GlobalPhase::Commit,
        subject: subject(0x30),
        execution_commitment: execution_commitment(0x30),
        signer: 0,
        signature: Vec::new(),
    });

    assert_eq!(request.body_round(), Some(proposal_round));
}

#[test]
fn locked_subject_reproposal_and_strict_higher_prepare_are_safe() {
    let context = context();
    let locked_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 1,
    };
    let locked_subject = subject(0x32);
    let locked_payload = [0x32, 2];
    let exact_manifest = encode_payload(&context, locked_round, locked_subject, &locked_payload)
        .expect("encode exact locked payload")
        .manifest()
        .clone();
    let exact = wire::Proposal {
        round: locked_round,
        proposer: context.leader(locked_round.view),
        subject: locked_subject,
        manifest: exact_manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    assert!(proposal_is_safe_for_lock(
        &exact,
        locked_round,
        locked_subject
    ));

    let later_round = wire::ConsensusRound {
        view: locked_round.view + 1,
        ..locked_round
    };
    let later = wire::Proposal {
        round: later_round,
        proposer: context.leader(later_round.view),
        manifest: encode_payload(&context, later_round, locked_subject, &locked_payload)
            .expect("encode later same-subject payload")
            .manifest()
            .clone(),
        ..exact
    };
    assert!(proposal_is_safe_for_lock(
        &later,
        locked_round,
        locked_subject
    ));

    let prepared_subject = subject(0x33);
    let prepared_round = wire::ConsensusRound {
        view: locked_round.view + 1,
        ..locked_round
    };
    let proposal_round = wire::ConsensusRound {
        view: prepared_round.view + 1,
        ..prepared_round
    };
    let prepared_payload = [0x33, 2];
    let highest_prepare = wire::QuorumCertificate {
        round: prepared_round,
        proposal_round: prepared_round,
        phase: wire::GlobalPhase::Prepare,
        subject: prepared_subject,
        execution_commitment: execution_commitment(0x33),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x33; 48],
    };
    let prepared_proposal = wire::Proposal {
        round: proposal_round,
        proposer: context.leader(proposal_round.view),
        subject: prepared_subject,
        manifest: encode_payload(
            &context,
            proposal_round,
            prepared_subject,
            &prepared_payload,
        )
        .expect("encode prepared-subject payload")
        .manifest()
        .clone(),
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: wire::TimeoutCertificate {
                round: prepared_round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: Some(highest_prepare.clone()),
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0x34; 48],
                }],
            },
            highest_prepare_qc: Some(highest_prepare),
        }),
        signature: vec![0x35; 48],
    };
    assert!(proposal_is_safe_for_lock(
        &prepared_proposal,
        locked_round,
        locked_subject
    ));
    let mut registry = WireRegistry::new(&context).expect("wire registry");
    registry
        .justification_to_core(&prepared_proposal.justification, &context)
        .expect("matching strict-higher PrepareQC authorizes the proposal subject");

    let mut missing_repeated_high = prepared_proposal.clone();
    let wire::ProposalJustification::Timeout(timeout) = &mut missing_repeated_high.justification
    else {
        unreachable!("prepared fixture carries a timeout")
    };
    timeout.highest_prepare_qc = None;
    assert!(
        !proposal_is_safe_for_lock(&missing_repeated_high, locked_round, locked_subject),
        "safe-value admission must reject a TC-selected high omitted by the proposal"
    );
    let mut missing_registry = WireRegistry::new(&context).expect("wire registry");
    assert!(matches!(
        missing_registry.justification_to_core(&missing_repeated_high.justification, &context),
        Err(AdapterError::InvalidProposalJustification)
    ));
    assert!(
        missing_registry.subjects.is_empty()
            && missing_registry.execution_commitments.is_empty()
            && missing_registry.certificates.is_empty(),
        "the omitted repeated-QC gate must reject before registry mutation"
    );

    let mut invented_repeated_high = prepared_proposal.clone();
    let wire::ProposalJustification::Timeout(timeout) = &mut invented_repeated_high.justification
    else {
        unreachable!("prepared fixture carries a timeout")
    };
    timeout.timeout_certificate.groups[0].highest_prepare_qc = None;
    assert!(
        !proposal_is_safe_for_lock(&invented_repeated_high, locked_round, locked_subject),
        "safe-value admission must reject a repeated high absent from the TC"
    );
    let mut invented_registry = WireRegistry::new(&context).expect("wire registry");
    assert!(matches!(
        invented_registry.justification_to_core(&invented_repeated_high.justification, &context),
        Err(AdapterError::InvalidProposalJustification)
    ));
    assert!(
        invented_registry.subjects.is_empty()
            && invented_registry.execution_commitments.is_empty()
            && invented_registry.certificates.is_empty(),
        "the invented repeated-QC gate must reject before registry mutation"
    );

    let mut alternate_evidence = prepared_proposal.clone();
    let wire::ProposalJustification::Timeout(timeout) = &mut alternate_evidence.justification
    else {
        unreachable!("prepared fixture carries a timeout")
    };
    let tc_selected = timeout
        .timeout_certificate
        .highest_prepare_qc()
        .expect("prepared fixture TC carries a high QC")
        .clone();
    let repeated = timeout
        .highest_prepare_qc
        .as_mut()
        .expect("prepared fixture repeats the high QC");
    repeated.signers = vec![0, 1, 3];
    repeated.aggregate_signature = vec![0x36; 48];
    assert_eq!(repeated.as_ref(), tc_selected.as_ref());
    assert_ne!(repeated, &tc_selected);
    assert!(
        !proposal_is_safe_for_lock(&alternate_evidence, locked_round, locked_subject),
        "safe-value admission must reject same-reference alternate evidence"
    );
    let mut alternate_registry = WireRegistry::new(&context).expect("wire registry");
    assert!(matches!(
        alternate_registry.justification_to_core(&alternate_evidence.justification, &context),
        Err(AdapterError::InvalidProposalJustification)
    ));
    assert!(
        alternate_registry.subjects.is_empty()
            && alternate_registry.execution_commitments.is_empty()
            && alternate_registry.certificates.is_empty(),
        "the exact repeated-QC gate must reject before registry mutation"
    );

    let mut equal_rank = prepared_proposal.clone();
    let wire::ProposalJustification::Timeout(timeout) = &mut equal_rank.justification else {
        unreachable!("prepared fixture carries a timeout")
    };
    let selected = timeout
        .highest_prepare_qc
        .as_mut()
        .expect("prepared fixture carries a high QC");
    selected.round = locked_round;
    selected.proposal_round = locked_round;
    timeout.timeout_certificate.groups[0].highest_prepare_qc = Some(selected.clone());
    assert!(
        !proposal_is_safe_for_lock(&equal_rank, locked_round, locked_subject),
        "an equal-rank PrepareQC cannot release a different lock subject"
    );
}

fn proposal(
    context: &wire::HeightContext,
    proposer: wire::ValidatorIndex,
    subject: wire::BlockSubject,
) -> wire::ConsensusMessageV2 {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let payload = b"chunk";
    let chunks = wire::encode_payload_chunks(context.da_layout, payload)
        .expect("encode complete canonical fixture chunks");
    let manifest = wire::PayloadManifest::derive(
        context,
        round,
        subject,
        u64::try_from(payload.len()).expect("fixture payload length fits u64"),
        &chunks,
    )
    .expect("valid fixture manifest");
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
        round,
        proposer,
        subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![0x91],
    }))
}

#[test]
fn adapter_equivocation_evidence_derives_authority_from_all_three_signed_pairs() {
    let context = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let proposer = context.leader(0);
    let wire::ConsensusMessageV2Payload::Proposal(first_proposal) =
        proposal(&context, proposer, subject(0xE1)).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second_proposal) =
        proposal(&context, proposer, subject(0xE2)).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };

    let first_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xE3),
        execution_commitment: execution_commitment(0xE3),
        signer: 1,
        signature: vec![0xE3],
    };
    let second_vote = wire::Vote {
        subject: subject(0xE4),
        execution_commitment: execution_commitment(0xE4),
        signature: vec![0xE4],
        ..first_vote.clone()
    };

    let high_prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xE5),
        execution_commitment: execution_commitment(0xE5),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xE5],
    };
    let timeout_round = wire::ConsensusRound { view: 1, ..round };
    let first_timeout = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: None,
        signer: 2,
        signature: vec![0xE6],
    };
    let second_timeout = wire::TimeoutVote {
        highest_prepare_qc: Some(high_prepare),
        signature: vec![0xE7],
        ..first_timeout.clone()
    };

    let cases = [
        (
            AdapterEquivocationEvidence::proposal(first_proposal, second_proposal),
            reducer::EquivocationKind::Proposal,
            proposer,
            round,
        ),
        (
            AdapterEquivocationEvidence::vote(first_vote, second_vote),
            reducer::EquivocationKind::Vote,
            1,
            round,
        ),
        (
            AdapterEquivocationEvidence::timeout_vote(first_timeout, second_timeout),
            reducer::EquivocationKind::Timeout,
            2,
            timeout_round,
        ),
    ];
    for (evidence, kind, offender, expected_round) in cases {
        evidence
            .validate_structure(&context)
            .expect("complete signed pair is structurally valid equivocation evidence");
        assert_eq!(evidence.kind(), kind);
        assert_eq!(evidence.offender_index(), offender);
        assert_eq!(evidence.round(), expected_round);
        let (first, second) = evidence.canonical_unsigned_statement_pair();
        assert!(
            first < second,
            "conflicting statements have canonical order"
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn forged_conflict_cannot_mint_adapter_equivocation_evidence() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified = VerifiedHeightContext::genesis(context.clone(), pops).expect("verified context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("forged-equivocation-safety.wal"),
        verified,
        None,
        reducer::Generation::new(1),
        [0xE8; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let proposer = context.leader(0);
    let proposer_index = usize::try_from(proposer).expect("small proposer index");
    let mut first = proposal(&context, proposer, subject(0xE8));
    let wire::ConsensusMessageV2Payload::Proposal(first_proposal) = &mut first.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    first_proposal.signature = Signature::new(
        keys[proposer_index].private_key(),
        &first_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let expected_first = first_proposal.clone();
    let authenticated_first = adapter
        .authenticate(first)
        .expect("authenticate the first proposal");
    adapter
        .receive_authenticated(authenticated_first)
        .expect("admit the first proposal");

    let mut conflicting = proposal(&context, proposer, subject(0xE9));
    let wrong_index = (proposer_index + 1) % keys.len();
    let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) = &mut conflicting.payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    conflicting_proposal.signature = Signature::new(
        keys[wrong_index].private_key(),
        &conflicting_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(conflicting.clone()),
        Err(AdapterError::Cryptography(_))
    ));
    let ingress_key = IngressSemanticKey::Proposal {
        round: expected_first.round,
        proposer,
    };
    assert!(
        !adapter
            .ingress_equivocations
            .get(&ingress_key)
            .expect("first authenticated proposal owns the semantic key")
            .equivocation_reported,
        "a forged conflicting signature cannot consume the one evidence report"
    );

    let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) = &mut conflicting.payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    conflicting_proposal.signature = Signature::new(
        keys[proposer_index].private_key(),
        &conflicting_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let expected_second = conflicting_proposal.clone();
    let authenticated_conflict = adapter
        .authenticate(conflicting)
        .expect("authenticate the genuinely conflicting proposal");
    let outcome = adapter
        .receive_authenticated(authenticated_conflict)
        .expect("emit exact authenticated equivocation evidence");
    let [AdapterEffect::ReportEquivocation { evidence }] = outcome.effects() else {
        panic!("authenticated proposal conflict must emit one evidence effect")
    };
    let (retained_first, retained_second) = evidence
        .proposal_pair()
        .expect("proposal conflict carries a sealed proposal pair");
    assert_eq!(retained_first, &expected_first);
    assert_eq!(retained_second, &expected_second);
}

fn synthetic_ingress_proposal(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    proposer: wire::ValidatorIndex,
    salt: usize,
) -> IngressEquivocationArtifact {
    let salt = u8::try_from(salt % usize::from(u8::MAX)).expect("bounded fixture salt");
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        proposal(context, context.leader(round.view), subject(salt)).payload
    else {
        unreachable!("proposal fixture")
    };
    proposal.round = round;
    proposal.manifest.round = round;
    proposal.proposer = proposer;
    proposal.signature = vec![salt];
    IngressEquivocationArtifact::Proposal(Arc::new(proposal))
}

fn authenticated_wire_identity(payload: wire::ConsensusMessageV2Payload) -> Arc<[u8]> {
    Arc::from(wire::ConsensusMessageV2::new(payload).encode())
}

fn durable_body_receipt(
    adapter: &SumeragiV2Adapter,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> DurableBodyReceipt {
    let manifest = adapter
        .registry
        .manifests
        .values()
        .find(|manifest| manifest.round == round && manifest.subject == subject)
        .expect("registered proposal manifest");
    DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        round,
        subject,
        HashOf::new(manifest),
    )
}

fn validated_receipts_for_manifest(
    context: &wire::HeightContext,
    manifest: &wire::PayloadManifest,
) -> (DurableBodyReceipt, ValidatedBodyReceipt) {
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    (durable, validated)
}

fn deferred_admission_ordinals() -> DeferredAdmissionOrdinalSource {
    DeferredAdmissionOrdinalSource::new(1)
}

struct ProcessOnlyProducerReplacement {
    address: ProducerContinuationAddress,
    incumbent: ProducerContinuationRecord,
    candidate: (ServicedCandidateKey, wire::View, ServicedCandidatePolicy),
    reservation: ProducerReservationToken,
}

fn reserve_process_only_producer_replacement(
    adapter: &mut SumeragiV2Adapter,
    marker: u8,
) -> ProcessOnlyProducerReplacement {
    let event = reducer::Event::TimeoutElapsed {
        tag: adapter.current_tag(),
    };
    let candidate = adapter
        .serviced_candidate(&event, DeferredPriority::Completion, None, None)
        .expect("timeout has a producer stage");
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"process-only predecessor"), 1)
        .expect("bind process-only predecessor");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("reserve process-only predecessor")
        .expect("tracked predecessor reserves");
    let handoff = adapter
        .record_serviced_candidate(Some(candidate), false, false, Some(reservation))
        .expect("drain process-only predecessor")
        .expect("drained predecessor retains its exact reservation");
    let address = handoff.address();
    adapter
        .acknowledge_producer_handoff(
            handoff,
            ProducerContinuationHandoffEvidence::VolatileTerminal,
        )
        .expect("terminalize process-only predecessor");
    let incumbent = adapter.producer_continuations[&address].clone();
    assert_eq!(incumbent.status(), ProducerContinuationStatus::Terminal);
    assert!(
        !adapter
            .durable_producer_continuations
            .contains_key(&address),
        "volatile predecessor must not have restart-stable state"
    );

    let replacement_key = ServicedCandidateKey::new(
        adapter.wire_context.id(),
        adapter.wire_context.height,
        adapter.fingerprints.node.into(),
        adapter.wire_context.leader(1),
        1,
        Some([marker; 32]),
        0,
        ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
        DeferredEventKind::TimeoutElapsed.code(),
        [marker; 32],
    );
    let candidate = (replacement_key, 1, ServicedCandidatePolicy::Suppress);
    adapter.clear_selected_producer_lifecycle();
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"newer replacement"), 2)
        .expect("bind newer replacement");
    let reservation = adapter
        .reserve_selected_producer_continuation(Some(candidate))
        .expect("replace process-only terminal")
        .expect("tracked replacement reserves");
    assert_eq!(reservation.address, address);
    let ProducerReservationChange::ReplacedTerminal {
        process_previous,
        durable_previous,
    } = &reservation.change
    else {
        panic!("newer lifecycle must replace the process-only terminal");
    };
    assert_eq!(process_previous, &incumbent);
    assert!(
        durable_previous.is_none(),
        "replacement must retain the absence of durable predecessor state"
    );

    ProcessOnlyProducerReplacement {
        address,
        incumbent,
        candidate,
        reservation,
    }
}

fn assert_process_only_predecessor_absent_after_restart(directory: &TempDir) {
    let (restarted, startup) = open_test(directory).expect("restart adapter");
    assert!(startup.is_empty());
    assert!(
        restarted.producer_continuations.is_empty()
            && restarted.durable_producer_continuations.is_empty()
            && restarted.restored_dormant_producer_continuations.is_empty(),
        "a process-only predecessor must not be synthesized during restart"
    );
}

fn open_test(directory: &TempDir) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
    SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(1),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
}

fn open_test_with_capacity_geometry(
    directory: &TempDir,
    capacity_geometry: ServicedCandidateCapacityGeometry,
) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
    SumeragiV2Adapter::open_with_aggregator_and_publication_with_capacity(
        directory.path().join("capacity-safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(1),
        [0x12; 32],
        fingerprints(),
        Box::new(TestAggregator),
        true,
        capacity_geometry,
        deferred_admission_ordinals(),
    )
}

fn assert_registry_eq(actual: &WireRegistry, expected: &WireRegistry) {
    assert_eq!(actual.wire_context, expected.wire_context);
    assert_eq!(actual.context_id, expected.context_id);
    assert_eq!(actual.peers, expected.peers);
    assert_eq!(actual.validators, expected.validators);
    assert_eq!(actual.subjects, expected.subjects);
    assert_eq!(actual.manifests, expected.manifests);
    assert_eq!(actual.execution_commitments, expected.execution_commitments);
    assert_eq!(actual.certificates, expected.certificates);
    assert_eq!(actual.proposals, expected.proposals);
}

fn open_test_as_leader(
    directory: &TempDir,
) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
    let context = context();
    let leader = context.leader(0);
    SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(1),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
}

fn unowned_body_event(adapter: &SumeragiV2Adapter, marker: u8) -> reducer::Event {
    reducer::Event::BodyAvailable {
        tag: adapter.current_tag(),
        round: reducer::Round::new(adapter.wire_context.height, adapter.current_tag().view()),
        subject: reducer::Subject::repeat(marker),
    }
}

fn durably_retire_unowned_body_event(adapter: &mut SumeragiV2Adapter, marker: u8) {
    let event = unowned_body_event(adapter, marker);
    assert!(
        adapter
            .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
            .expect("retain the terminal candidate under exact deferred ownership")
            .is_some()
    );
    assert!(
        adapter
            .drain_deferred()
            .expect("durably retire the terminal candidate")
            .is_empty()
    );
}

#[test]
fn direct_internal_discard_tombstones_a_b_a_and_survives_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let a_marker = 0x31;
    let b_marker = 0x32;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let initial = adapter.serviced_candidate_count_for_test();
        let a = unowned_body_event(&adapter, a_marker);
        let b = unowned_body_event(&adapter, b_marker);
        assert_ne!(a, b);
        assert_ne!(
            adapter
                .step(a.clone())
                .expect("service candidate A")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_ne!(
            adapter
                .step(b)
                .expect("service equal-rank replacement B")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
        assert_eq!(adapter.durable_serviced_candidates.len(), initial + 2);
        assert_eq!(
            adapter
                .step(a)
                .expect("coalesce resurrected candidate A")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
    }

    let context = context();
    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context.clone()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen with exact direct-discard terminal records");
    assert!(startup.is_empty());
    let retained = restarted.serviced_candidate_count_for_test();
    assert_eq!(
        retained, 2,
        "direct and deferred internal NoMatchingWork discards are restart-stable"
    );
    let restarted_a = unowned_body_event(&restarted, a_marker);
    assert_eq!(
        restarted
            .step(restarted_a)
            .expect("coalesce A after process generation changes")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
}

#[test]
fn nonquorum_vote_retransmission_rebuilds_volatile_pool_after_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let vote = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x35),
        execution_commitment: execution_commitment(0x35),
        signer: 1,
        signature: vec![0x35],
    }));
    let replacement =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x35),
            execution_commitment: execution_commitment(0x35),
            signer: 2,
            signature: vec![0x36],
        }));
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                .expect("admit one nonquorum Prepare vote")
                .disposition(),
            reducer::StepDisposition::Applied
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 1);
        assert!(
            adapter.durable_serviced_candidates.is_empty(),
            "a volatile quorum contribution is process-local, never a restart tombstone"
        );
        let first_key = IngressSemanticKey::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            signer: 1,
        };
        adapter.ingress_deliveries.remove(&first_key);
        adapter.ingress_equivocations.remove(&first_key);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(replacement,))
                .expect("service equal-rank candidate B")
                .disposition(),
            reducer::StepDisposition::Applied
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
        let replacement_key = IngressSemanticKey::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            signer: 2,
        };
        adapter.ingress_deliveries.remove(&replacement_key);
        adapter.ingress_equivocations.remove(&replacement_key);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                .expect("coalesce candidate A after equal-rank replacement B")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
            "same-generation A -> B -> A service must not resurrect A"
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
        assert!(
            adapter
                .status()
                .expect("one-vote status")
                .liveness
                .prepare_quorums
                .iter()
                .any(|quorum| quorum.round == round && quorum.signer_count == 2)
        );
    }

    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart after losing the volatile vote pool");
    assert!(startup.is_empty());
    assert_eq!(restarted.serviced_candidate_count_for_test(), 0);
    assert!(
        restarted
            .status()
            .expect("empty post-restart pool")
            .liveness
            .prepare_quorums
            .is_empty()
    );
    assert_eq!(
        restarted
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote))
            .expect("retransmission reconstructs the lost vote owner")
            .disposition(),
        reducer::StepDisposition::Applied
    );
    assert!(
        restarted
            .status()
            .expect("rebuilt vote pool")
            .liveness
            .prepare_quorums
            .iter()
            .any(|quorum| quorum.round == round && quorum.signer_count == 1)
    );
}

#[test]
fn deferred_discard_tombstones_before_owner_release_and_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let marker = 0x33;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let initial = adapter.serviced_candidate_count_for_test();
        let discarded = unowned_body_event(&adapter, marker);
        assert!(
            adapter
                .enqueue_deferred(
                    discarded.clone(),
                    false,
                    DeferredPriority::Completion,
                    None,
                    None,
                    None,
                )
                .expect("retain the candidate under deferred ownership")
                .is_some()
        );
        assert_eq!(adapter.deferred_completions.len(), 1);

        let effects = adapter
            .drain_deferred()
            .expect("service the nondispatchable candidate exactly once");
        assert!(effects.is_empty());
        assert!(adapter.deferred_completions.is_empty());
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            initial + 1,
            "the terminal discard must be durable before the deferred owner is released"
        );
        assert_eq!(
            adapter
                .step(discarded)
                .expect("coalesce retransmission after deferred drain")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 1);
    }

    let context = context();
    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restore the terminal candidate tombstone");
    assert!(startup.is_empty());
    let retained = restarted.serviced_candidate_count_for_test();
    let retransmitted = unowned_body_event(&restarted, marker);
    assert_eq!(
        restarted
            .step(retransmitted)
            .expect("coalesce retransmission after same-height restart")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
}

#[test]
fn serviced_candidate_write_failure_is_fail_closed_and_retains_deferred_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    durably_retire_unowned_body_event(&mut adapter, 0x40);
    let event = unowned_body_event(&adapter, 0x41);
    assert!(
        adapter
            .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
            .expect("retain candidate in deferred ownership")
            .is_some()
    );
    let path = adapter
        .serviced_candidate_store_path_for_test()
        .to_path_buf();
    std::fs::remove_file(&path).expect("remove published snapshot");
    std::fs::create_dir(&path).expect("replace snapshot target with a directory");
    let retained = adapter.deferred_completions.len();
    assert!(matches!(
        adapter.drain_deferred(),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
    assert!(adapter.fail_closed);
    assert_eq!(
        adapter.deferred_completions.len(),
        retained,
        "failed publication retains the selected owner before fail-stop"
    );
}

#[test]
fn restored_producer_reuses_runtime_key_and_ordinal_and_does_not_resurrect() {
    let directory = TempDir::new().expect("temporary directory");
    let causal_key;
    {
        let (adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let started_at = Instant::now();
        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (mut runtime, startup) =
            super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                adapter,
                startup,
                started_at,
                Duration::from_secs(4),
                super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                lifecycle_ordinals,
            )
            .expect("construct the original serialized runtime");
        assert!(startup.is_empty());
        runtime
            .arm_live_clocks(started_at)
            .expect("arm the original runtime");
        let owner = runtime
            .frozen_timeout_owner_for_test(started_at + Duration::from_secs(4))
            .expect("freeze the deterministic original timeout owner");
        causal_key = owner.causal_origin().lifecycle_key;
        assert_eq!(owner.lifecycle_ordinal(), 1);
        let mut adapter = runtime.into_driver();
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(causal_key, owner.lifecycle_ordinal())
            .expect("bind selected source");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve before source retirement")
            .expect("tracked candidate reserves an address");
        let address = reservation.address;
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Reserved
        );
        assert_eq!(
            adapter.durable_producer_continuations.get(&address),
            adapter.producer_continuations.get(&address),
            "reservation is synchronized before its source can retire"
        );
    }

    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart with exact active admission metadata");
    assert!(startup.is_empty());
    let restored = restarted
        .producer_continuations
        .values()
        .next()
        .expect("active producer metadata reopens");
    assert_eq!(restored.status(), ProducerContinuationStatus::Reserved);
    assert_eq!(restored.identity().admission_ordinal(), 1);
    let restored_address = restored.identity().address();
    assert_eq!(restored_address.lifecycle_slot(), 1);
    assert_eq!(
        restarted.restored_producer_continuation_ordinal_high_watermark(),
        Some(1)
    );
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
    assert!(
        restarted
            .dormant_local_fifo_reservations()
            .expect("validate restored timeout metadata")
            .is_empty(),
        "a restart-dormant timeout remains a non-FIFO clock root"
    );

    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            startup,
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct the restarted serialized runtime");
    assert!(startup.is_empty());
    assert_eq!(
        runtime.remaining_completion_capacity(),
        6,
        "the non-FIFO timeout root cannot consume completion capacity"
    );
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime");
    let step = runtime
        .step(started_at + Duration::from_secs(4))
        .expect("replayed timeout reuses and crosses the exact runtime handoff");
    let super::super::v2_runtime::RuntimeStep::Advanced(effects) = step else {
        panic!("the exact replayed timeout must advance");
    };
    assert!(!effects.is_empty(), "timeout retains a concrete successor");
    let scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("timeout publishes exact scheduler ownership");
    assert_eq!(
        scheduler.selected,
        super::super::v2_runtime::RuntimeSelectedOwnerKind::Timeout
    );
    let effect_ownership = runtime
        .take_effect_ownership(effects.len())
        .expect("take the concrete successor ownership");
    assert!(
        effect_ownership
            .iter()
            .all(|ownership| ownership.owner().lifecycle_ordinal() == 1),
        "every concrete successor retains the original owner 1"
    );
    let retained = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("runtime acknowledgement retains its process-local terminal");
    assert_eq!(
        retained.identity().admission_ordinal(),
        1,
        "restart cannot replace the immutable first-admission ordinal"
    );
    assert_eq!(
        retained.identity().causal_lifecycle_key(),
        effect_ownership[0].owner().causal_origin().lifecycle_key
    );
    assert_eq!(
        retained.identity().causal_lifecycle_key(),
        causal_key,
        "the exact retry retains its persisted causal identity"
    );
    assert_eq!(retained.status(), ProducerContinuationStatus::Terminal);
    assert!(
        !runtime
            .driver()
            .durable_producer_continuations
            .contains_key(&restored_address),
        "a concrete volatile successor removes the dormant restart record"
    );
    assert!(
        !runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );

    drop(runtime.into_driver());
    let (restarted_again, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart after the runtime handoff");
    assert!(
        matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ),
        "restart reconstructs the durable exact successor instead of the drained timeout stage"
    );
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "the drained logical request cannot be recreated at its old stage"
    );
}

struct StageSevenCrashCut {
    wire_context: wire::HeightContext,
    round: wire::ConsensusRound,
    body_subject: wire::BlockSubject,
    manifest: wire::PayloadManifest,
    logical_key: Hash,
    logical_ordinal: u128,
    restored_address: ProducerContinuationAddress,
}

fn persist_stage_seven_crash_cut(directory: &TempDir, marker: u8) -> StageSevenCrashCut {
    let wire_context = context();
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let body_subject = subject(marker);
    let payload = vec![marker; 32];
    let chunks = wire::encode_payload_chunks(wire_context.da_layout, &payload)
        .expect("encode canonical body chunks");
    let manifest = wire::PayloadManifest::derive(
        &wire_context,
        round,
        body_subject,
        u64::try_from(payload.len()).expect("fixture payload length fits u64"),
        &chunks,
    )
    .expect("derive canonical body manifest");
    let logical_key;
    let logical_ordinal;
    let restored_address;
    {
        let (adapter, startup) = open_test(directory).expect("open original adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject: body_subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let started_at = Instant::now();
        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (mut runtime, startup) =
            super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                adapter,
                vec![fetch.clone()],
                started_at,
                Duration::from_secs(4),
                super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                lifecycle_ordinals,
            )
            .expect("construct runtime with the original body fetch");
        assert_eq!(startup, vec![fetch]);
        let mut ownership = runtime
            .take_effect_ownership(1)
            .expect("take the original body-fetch ownership");
        let fetch_ownership = ownership.pop().expect("one body-fetch owner");
        assert!(ownership.is_empty());
        logical_key = fetch_ownership.owner().causal_origin().lifecycle_key;
        logical_ordinal = fetch_ownership.owner().lifecycle_ordinal();
        assert_eq!(logical_ordinal, 1);

        let mut adapter = runtime.into_driver();
        let event = reducer::Event::BodyAvailable {
            tag,
            round: reducer::Round::new(round.height, round.view),
            subject: reducer::Subject::new(Hash::new(body_subject.encode()).into()),
        };
        let completion_evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let candidate = adapter
            .serviced_candidate(
                &event,
                DeferredPriority::Completion,
                Some(&completion_evidence),
                None,
            )
            .expect("BodyAvailable has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(logical_key, logical_ordinal)
            .expect("bind the body-fetch lifecycle");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("persist before the BodyAvailable reducer step")
            .expect("BodyAvailable reserves a producer continuation");
        restored_address = reservation.address;
        let record = &adapter.producer_continuations[&restored_address];
        assert_eq!(record.status(), ProducerContinuationStatus::Reserved);
        assert_eq!(
            record.identity().stage(),
            ServicedCandidateStage::BodyAvailable as u8
        );
        assert_eq!(
            record.source_class(),
            ProducerContinuationSourceClass::VolatileBody
        );
        assert_eq!(
            adapter
                .durable_producer_continuations
                .get(&restored_address),
            Some(record),
            "the stage-7 crash cut must be durable before reducer service"
        );
    }
    StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    }
}

#[test]
fn body_rebind_coalescence_preserves_the_only_persistent_producer() {
    let directory = TempDir::new().expect("temporary durable coalescence directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, 0xBC);
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 coalescence crash cut");
    assert!(startup.is_empty());
    let previous = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"coalescence parent state"),
            Hash::new(b"coalescence post state"),
            Hash::new(b"coalescence writes"),
            1,
            Hash::new(b"coalescence executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xBC; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("coalescence reconstruction certificate is structurally valid");
    let protected_lock = wire::QuorumCertificate {
        phase: wire::GlobalPhase::Prepare,
        ..certificate.clone()
    };
    let fetch = AdapterEffect::FetchBody {
        tag: previous,
        round,
        subject: body_subject,
        manifest: Some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct runtime for durable coalescence");
    assert_eq!(startup, vec![fetch]);
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take the reconstructed body-fetch owner");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    let reservation = runtime
        .reserve_body_available_with_owner(previous, manifest.clone(), &fetch_ownership)
        .expect("reserve the restart-restored body completion");
    runtime
        .commit_body_available(reservation)
        .expect("materialize the restart-restored source owner");

    let rebound = reducer::EventTag::new(
        previous.height(),
        previous.view() + 1,
        reducer::Generation::new(previous.generation().get() + 1),
    );
    runtime
        .observe_effects_with_test_ownership(
            started_at,
            &[AdapterEffect::EnterView {
                tag: rebound,
                certificate: wire::TimeoutCertificate {
                    round,
                    groups: vec![wire::TimeoutVoteGroup {
                        highest_prepare_qc: None,
                        signers: vec![0, 1, 2],
                        aggregate_signature: vec![0xCD; 96],
                    }],
                },
                protected_lock: Some(protected_lock),
            }],
        )
        .expect("install the certified destination incarnation");
    runtime
        .enqueue_volatile_body_available_for_test(rebound, manifest.clone())
        .expect("stage an independently volatile destination owner");
    assert_eq!(runtime.queued_commands(), 2);

    assert!(
        runtime
            .rebind_body_available(previous, rebound, &manifest)
            .expect("coalesce while retaining the persistent source")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        !runtime
            .rebind_body_available(previous, rebound, &manifest)
            .expect("the old source tag is now vacant")
    );
    let retained = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("coalescence retains the process producer record");
    assert_eq!(retained.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(retained.identity().admission_ordinal(), logical_ordinal);
    assert_eq!(
        runtime
            .driver()
            .durable_producer_continuations
            .get(&restored_address),
        Some(retained),
    );
    assert!(
        runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address),
        "the rebound volatile carrier aliases the same restart-dormant producer",
    );

    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after durable-owner coalescence");
    let reopened = restarted_again
        .producer_continuations
        .get(&restored_address)
        .expect("the surviving producer remains restart-recoverable");
    assert_eq!(reopened.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(reopened.identity().admission_ordinal(), logical_ordinal);
    assert!(
        restarted_again
            .restored_dormant_producer_continuations
            .contains(&restored_address),
    );
}

#[test]
fn restored_body_available_reuses_logical_lifecycle_spends_one_fresh_slot_and_does_not_resurrect() {
    let directory = TempDir::new().expect("temporary directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, 0xB7);

    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 crash cut");
    assert!(startup.is_empty());
    let restored = restarted
        .producer_continuations
        .get(&restored_address)
        .expect("stage-7 logical lifecycle reopens");
    assert_eq!(restored.status(), ProducerContinuationStatus::Reserved);
    assert_eq!(restored.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(restored.identity().admission_ordinal(), logical_ordinal);
    assert_eq!(
        restored.source_class(),
        ProducerContinuationSourceClass::VolatileBody
    );
    assert!(
        restarted
            .dormant_local_fifo_reservations()
            .expect("validate restored BodyAvailable metadata")
            .is_empty(),
        "stage 7 preserves logical identity without a latent FIFO slot"
    );

    let restarted_tag = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"stage-seven parent state"),
            Hash::new(b"stage-seven post state"),
            Hash::new(b"stage-seven writes"),
            1,
            Hash::new(b"stage-seven executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB7; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("certified reconstruction is structurally valid");
    let reconstructed_fetch = AdapterEffect::FetchBody {
        tag: restarted_tag,
        round,
        subject: body_subject,
        manifest: Some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![reconstructed_fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with the reconstructed body fetch");
    assert_eq!(startup, vec![reconstructed_fetch.clone()]);
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime");
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take reconstructed body-fetch ownership");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    assert!(ownership.is_empty());
    assert_ne!(
        fetch_ownership.owner().causal_origin().lifecycle_key,
        logical_key,
        "certified reconstruction owns a different physical Fetch lifecycle"
    );
    assert_eq!(
        fetch_ownership.owner().lifecycle_ordinal(),
        logical_ordinal + 1
    );
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect the shared source before completion admission"),
        Some(logical_ordinal + 2),
        "the certified Fetch owns one new external lifecycle before completion admission"
    );

    let capacity_before = runtime.remaining_completion_capacity();
    let reservation = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest.clone(), &fetch_ownership)
        .expect("reserve the reconstructed stage-7 completion");
    assert!(reservation.owns_new_slot());
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    let source_after_reserve = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect the shared source after completion admission");
    assert_eq!(source_after_reserve, Some(logical_ordinal + 3));

    let retry = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest.clone(), &fetch_ownership)
        .expect("exact reconstruction retry coalesces with its token");
    assert_eq!(retry, reservation);
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect the shared source after exact retry"),
        source_after_reserve,
        "an exact retry cannot spend a second physical admission position"
    );

    runtime
        .commit_body_available(retry)
        .expect("materialize the reconstructed completion");
    assert_eq!(runtime.queued_commands(), 1);
    let step = runtime
        .step(started_at)
        .expect("service the restored BodyAvailable handoff");
    let super::super::v2_runtime::RuntimeStep::Advanced(effects) = step else {
        panic!("the restored BodyAvailable completion must dispatch");
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyAvailable dispatch publishes scheduler ownership");
    if !effects.is_empty() {
        runtime
            .take_effect_ownership(effects.len())
            .expect("take BodyAvailable successor ownership");
    }
    let terminal = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("service acknowledgement retains a process-local terminal");
    assert_eq!(terminal.status(), ProducerContinuationStatus::Terminal);
    assert_eq!(terminal.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(terminal.identity().admission_ordinal(), logical_ordinal);
    assert!(
        !runtime
            .driver()
            .durable_producer_continuations
            .contains_key(&restored_address),
        "the service handoff removes the restart-stable stage-7 record"
    );

    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after the stage-7 service handoff");
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "the serviced old stage cannot resurrect on a second restart"
    );
}

fn assert_restored_stage_seven_retirement_does_not_resurrect(
    marker: u8,
    reserve_completion: bool,
    materialize_before_retirement: bool,
    inject_persistence_failure: bool,
) {
    let directory = TempDir::new().expect("temporary stage-7 retirement directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, marker);
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 retirement crash cut");
    assert!(startup.is_empty());
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );

    let restarted_tag = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 3]),
            Hash::new([marker, 4]),
            Hash::new([marker, 5]),
            1,
            Hash::new([marker, 6]),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("certified retirement reconstruction is structurally valid");
    let reconstructed_fetch = AdapterEffect::FetchBody {
        tag: restarted_tag,
        round,
        subject: body_subject,
        manifest: (marker != 0xBD).then_some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![reconstructed_fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct runtime for restored stage-7 retirement");
    assert_eq!(startup, vec![reconstructed_fetch.clone()]);
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the stage-7 retirement runtime");
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take reconstructed retirement fetch ownership");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    assert!(ownership.is_empty());
    assert_ne!(
        fetch_ownership.owner().causal_origin().lifecycle_key,
        logical_key
    );

    let capacity_before = runtime.remaining_completion_capacity();
    if !reserve_completion {
        assert!(
            runtime
                .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
                .expect("persist terminal restored fetch-parent retirement")
        );
        assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
        assert!(
            !runtime
                .driver()
                .producer_continuations
                .contains_key(&restored_address)
                && !runtime
                    .driver()
                    .durable_producer_continuations
                    .contains_key(&restored_address)
                && !runtime
                    .driver()
                    .restored_dormant_producer_continuations
                    .contains(&restored_address),
            "terminal fetch cancellation must remove its dormant stage-7 parent"
        );
        drop(runtime.into_driver());
        let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(3),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("reopen after terminal restored fetch cancellation");
        assert!(restarted_again.producer_continuations.is_empty());
        return;
    }
    let reservation = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest, &fetch_ownership)
        .expect("reserve the restored completion before terminal retirement");
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    assert!(
        !inject_persistence_failure || !materialize_before_retirement,
        "the persistence-failure seam targets the unpublished token"
    );
    let sabotaged_snapshot = inject_persistence_failure.then(|| {
        let path = runtime
            .driver()
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
        let bytes = std::fs::read(&path).expect("read the stage-7 producer snapshot");
        std::fs::remove_file(&path).expect("remove the published producer snapshot");
        std::fs::create_dir(&path).expect("replace producer snapshot with a directory");
        (path, bytes)
    });
    let retired = if materialize_before_retirement {
        runtime
            .commit_body_available(reservation)
            .expect("materialize restored completion before pipeline retirement");
        runtime
            .retire_body_pipeline_completions(restarted_tag, round, body_subject)
            .map(|retired| retired.body_available())
    } else {
        runtime.retire_unpublished_body_available(restarted_tag, round, body_subject)
    };
    if let Some((path, bytes)) = sabotaged_snapshot {
        assert!(
            retired.is_err(),
            "a failed durable release cannot publish volatile token retirement"
        );
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before - 1,
            "failed persistence retains the exact unpublished physical owner"
        );
        assert!(runtime.driver().fail_closed);
        assert_eq!(
            runtime
                .driver()
                .producer_continuations
                .get(&restored_address),
            runtime
                .driver()
                .durable_producer_continuations
                .get(&restored_address),
            "failed persistence restores both in-memory producer aliases"
        );
        assert!(
            runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address)
        );
        std::fs::remove_dir(&path).expect("remove sabotaged producer directory");
        std::fs::write(&path, bytes).expect("restore the pre-retirement producer snapshot");
        drop(runtime.into_driver());
        let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(3),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("reopen the retained stage-7 producer after failed retirement");
        assert!(
            restarted_again
                .restored_dormant_producer_continuations
                .contains(&restored_address),
            "failed retirement must reopen the old owner instead of losing it"
        );
        return;
    }
    assert!(retired.expect("persist and retire the restored body completion"));
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
    assert!(
        !runtime
            .driver()
            .producer_continuations
            .contains_key(&restored_address)
            && !runtime
                .driver()
                .durable_producer_continuations
                .contains_key(&restored_address)
            && !runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address),
        "terminal runtime retirement must persistently release the restored producer"
    );

    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after terminal stage-7 retirement");
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "a terminally retired stage-7 producer cannot resurrect on restart"
    );
}
