use super::super::serviced_candidate_store::ProducerContinuationSourceClass;
use super::*;
use crate::sumeragi::{
    v2_chunks::encode_payload,
    v2_runtime::{
        PendingRuntimeEffectBinding, RuntimeEffectOwnership, bind_adapter_effect_batch_ownership,
    },
};
use iroha_config::parameters::actual::{
    SUMERAGI_V2_CONFIG_FORMAT_VERSION, SumeragiV2Config, SumeragiV2KeyPolicy, SumeragiV2Limits,
};
use iroha_crypto::{Algorithm, HashOf, KeyPair, SignatureOf};
use iroha_data_model::block::{BlockHeader, BlockSignature, SignedBlock};
#[cfg(feature = "bls")]
use std::collections::BTreeMap;
use std::{fs::OpenOptions, io::Write as _, num::NonZeroU64, time::Duration};
use tempfile::TempDir;
fn test_network_id(seed: u8) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(Hash::prehashed(
        [seed; Hash::LENGTH],
    )))
}
#[test]
fn recovered_lifecycle_kura_binding_releases_paths_only_to_exact_kura() {
    let kura = Kura::blank_kura_for_testing();
    let foreign_kura = Kura::blank_kura_for_testing();
    let local_signer = KeyPair::random();
    let foreign_signer = KeyPair::random();
    let binding =
        RecoveredLifecycleOwnerKuraBindingV1::for_test(kura.as_ref(), Some(&local_signer));
    let storage_root = kura.sumeragi_v2_storage_root();
    let paths = binding
        .storage_paths_for_launch(kura.as_ref())
        .expect("the exact Kura projects its sealed launch paths");
    assert_eq!(
        paths.wal_path(),
        storage_root.join("wal").join(format!("{:020}.wal", 1_u64))
    );
    assert!(binding.matches_launch_identity(kura.as_ref(), &local_signer));
    assert!(!binding.matches_launch_identity(kura.as_ref(), &foreign_signer));
    assert!(!binding.matches_launch_identity(foreign_kura.as_ref(), &local_signer));
    assert!(
        binding
            .storage_paths_for_launch(foreign_kura.as_ref())
            .is_none(),
        "a foreign Kura must not project launch storage paths"
    );
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
    let network_id = test_network_id(0x61);
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 1, &roster);
    wire::HeightContext {
        network_id,
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
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
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
fn run_marker_replay_test_on_stack() {
    let handle = std::thread::Builder::new()
        .name("production-lifecycle-marker-replay".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies)
        .expect("spawn production lifecycle replay test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

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
    let network_id = test_network_id(0x62);
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 3, &roster);
    let context = wire::HeightContext {
        network_id,
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
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
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
fn rebind_production_serve_execution_commitment(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    requester_key: &KeyPair,
    authenticated: crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
    execution_commitment: wire::ExecutionCommitment,
) -> crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest {
    let mut request = authenticated.request().clone();
    request.certificate.execution_commitment = execution_commitment;
    let vote_preimage = wire::Vote {
        round: request.certificate.round,
        proposal_round: request.certificate.proposal_round,
        phase: request.certificate.phase,
        subject: request.certificate.subject,
        execution_commitment,
        signer: *request
            .certificate
            .signers
            .first()
            .expect("production Serve fixture has a signer"),
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = request
        .certificate
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("fixture signer index fits usize");
            Signature::new(keys[index].private_key(), &vote_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    request.certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate rebound production Serve certificate");
    request.signature = Signature::new(requester_key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let requester = request.requester.clone();
    crate::sumeragi::v2_transport::authenticate_certified_body_request(
        context,
        request,
        &requester,
        |_, _| Ok::<(), &'static str>(()),
    )
    .expect("authenticate rebound production Serve request")
}

#[cfg(feature = "bls")]
fn production_serve_requests_for_execution_commitment(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    local_validator: wire::ValidatorIndex,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
) -> (
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
) {
    let local = usize::try_from(local_validator).expect("fixture validator index fits usize");
    let rejected_requester = (local + 1) % keys.len();
    let admitted_requester = (local + 2) % keys.len();
    let non_local = (0..keys.len())
        .filter(|index| *index != local)
        .collect::<Vec<_>>();
    let exact_quorum = super::super::network_topology::commit_quorum_from_len(keys.len());
    let mut local_quorum = std::iter::once(local)
        .chain((0..keys.len()).filter(|index| *index != local))
        .take(exact_quorum)
        .collect::<Vec<_>>();
    local_quorum.sort_unstable();
    let build = |requester: usize, signers: &[usize]| {
        let (request, _) = super::super::v2_worker::tests::production_authenticated_serve_request(
            context,
            keys,
            &keys[requester],
            round,
            subject,
            wire::GlobalPhase::Commit,
            signers,
        );
        rebind_production_serve_execution_commitment(
            context,
            keys,
            &keys[requester],
            request,
            execution_commitment,
        )
    };
    (
        build(rejected_requester, &non_local),
        build(admitted_requester, &local_quorum),
    )
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
fn authenticated_timeout_certificate(
    round: wire::ConsensusRound,
    highest_prepare_qc: Option<wire::QuorumCertificate>,
    signers: Vec<wire::ValidatorIndex>,
    keys: &[KeyPair],
) -> wire::TimeoutCertificate {
    let signer = signers
        .first()
        .copied()
        .expect("fixture timeout certificate has signers");
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: highest_prepare_qc.clone(),
        signer,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            let index = usize::try_from(*signer).expect("small timeout signer index");
            Signature::new(keys[index].private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate fixture timeout certificate");
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc,
            signers,
            aggregate_signature,
        }],
    }
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
    let next_epoch = context.epoch + 1;
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
            context.network_id,
            next_epoch,
            &context.roster,
        );
    context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
        epoch: next_epoch,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
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
    let conflicting_commitment = execution_commitment(0x22);
    assert_ne!(conflicting_commitment, parent_qc.execution_commitment);
    let conflicting_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: parent_subject,
        execution_commitment: conflicting_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let conflicting_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &conflicting_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let conflicting_refs = conflicting_shares
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let mut conflicting_parent_qc = parent_qc.clone();
    conflicting_parent_qc.execution_commitment = conflicting_commitment;
    conflicting_parent_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&conflicting_refs)
            .expect("aggregate conflicting parent CommitQC");
    verify_quorum_certificate(&parent_context, &conflicting_parent_qc, &proofs)
        .expect("conflicting parent CommitQC remains independently valid");
    let mut conflicting_successor = successor.clone();
    conflicting_successor.parent_commit_qc = Some(conflicting_parent_qc);
    assert!(matches!(
        VerifiedHeightContext::successor(
            conflicting_successor,
            proofs.clone(),
            &artifact,
            &receipt,
            &proofs,
        ),
        Err(AdapterError::ParentContextMismatch)
    ));
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
    proposal_subject.parent_block_hash = Some(parent_subject.block_hash);
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
        Some(proposer),
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
    wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
#[test]
fn production_leader_wire_launch_authority_requires_exact_wal_and_opens_gate() {
    let directory = TempDir::new().expect("temporary leader-wire launch directory");
    let wal_path = directory.path().join("safety.wal");
    let (adapter, effects) = open_test(&directory).expect("open exact adapter");
    assert!(effects.is_empty());
    let context = adapter.wire_context.clone();
    let mut startup = ProductionLifecycleAdapterStartupV1::recovered(adapter, effects);
    assert!(
        startup
            .prepare_leader_wire_launch(&directory.path().join("foreign.wal"))
            .is_err(),
        "a foreign WAL path must not project leader-wire launch authority"
    );
    let launch = startup
        .prepare_leader_wire_launch(&wal_path)
        .expect("exact WAL projects one leader-wire launch authority");
    assert!(
        startup.prepare_leader_wire_launch(&wal_path).is_err(),
        "the adapter can mint its leader-wire launch authority only once"
    );
    assert_eq!(launch.restored_producer_ordinal_high_watermark(), None);
    let (gate, restore, _service_recovery_authority) = launch
        .open_gate(
            &context,
            &super::super::v2_body_store::V2BodyStore::open_with_policy(
                directory.path().join("bodies"),
                context.clone(),
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            )
            .expect("open exact empty body store"),
        )
        .expect("adapter-bound authority opens the adjacent gate");
    assert_eq!(restore.scheduler_ordinal_high_watermark(), 0);
    assert!(gate.restore().is_ok());
}
fn open_recovered_startup_test(
    directory: &TempDir,
) -> Result<RecoveredAdapterStartup, AdapterError> {
    open_recovered_startup_at_test_path(directory.path().join("safety.wal"))
}
fn open_recovered_startup_at_test_path(
    wal_path: impl Into<PathBuf>,
) -> Result<RecoveredAdapterStartup, AdapterError> {
    SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        wal_path,
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(1),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
}
fn open_recovered_leader_startup_test(
    directory: &TempDir,
) -> Result<RecoveredAdapterStartup, AdapterError> {
    let context = context();
    let leader = context.leader(0);
    SumeragiV2Adapter::open_recovered_startup_with_aggregator(
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
