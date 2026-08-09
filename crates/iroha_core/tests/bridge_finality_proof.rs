//! Exact Sumeragi-v2 bridge finality proof construction and verification tests.

use std::{num::NonZeroU64, sync::Arc};

use iroha_core::bridge::{
    BridgeFinalityError, BridgeStateReadOnly, FinalityProofVerificationConfig,
    VerifiedV2FinalityArtifact, build_finality_proof, verify_finality_proof,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, BlockSignature, SignedBlock,
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
        },
    },
    peer::PeerId,
};

struct Fixture {
    network_id: NetworkId,
    block: Arc<SignedBlock>,
    artifact: V2FinalityArtifact,
    pops: Vec<Vec<u8>>,
}

fn checked_bls_validator_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("generate checked bridge finality BLS validator fixture keypair")
}

fn fixture_network_id(seed: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        seed,
    )))
}

#[test]
fn bridge_finality_validator_fixture_uses_checked_bls_key_generation() {
    let key_pair = checked_bls_validator_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture validator public key has a valid algorithm");

    assert_eq!(algorithm, Algorithm::BlsNormal);
}

fn fixture() -> Fixture {
    let network_id = fixture_network_id(b"bridge-v2-core-test genesis");
    let mut keys = (0..4)
        .map(|_| checked_bls_validator_fixture())
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let powers = [1, 1, 1, 1];
    let roster = keys
        .iter()
        .zip(powers)
        .map(|(key, power)| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();

    let block_key = KeyPair::try_random().expect("block fixture key");
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_key.private_key(), header.hash())
            .expect("sign block header"),
    );
    let block = Arc::new(SignedBlock::presigned(
        block_signature,
        header.clone(),
        Vec::new(),
    ));
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid roster"),
        roster,
        nexus_amx_context_hash: Hash::new(b"bridge core v2 context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x42; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("canonical bridge proposal block wire"),
    };
    let signers = vec![0, 1, 2];
    let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"bridge core v2 parent state"),
        Hash::new(b"bridge core v2 post state"),
        Hash::new(b"bridge core v2 ordinary writes"),
        u64::try_from(block.encode_wire().expect("bridge block wire").len())
            .expect("bridge block wire length fits u64"),
        block
            .executed_block_wire_hash()
            .expect("canonical bridge executed block wire"),
    );
    let round = ConsensusRound {
        context_id: context.id(),
        height: 1,
        view: 0,
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers,
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid certificate signer");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).expect("fixture index")].private_key(),
                &preimage,
            )
            .expect("sign commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate commit votes");
    let pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("derive validator PoP")
        })
        .collect::<Vec<_>>();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, pops.clone());
    Fixture {
        network_id,
        block,
        artifact,
        pops,
    }
}

struct TestState {
    network_id: NetworkId,
    retained_header: Option<BlockHeader>,
    artifact: Result<Option<V2FinalityArtifact>, String>,
}

impl BridgeStateReadOnly for TestState {
    fn bridge_network_id(&self) -> &NetworkId {
        &self.network_id
    }

    fn bridge_verified_v2_finality_artifact(
        &self,
        _height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.artifact
            .clone()?
            .zip(self.retained_header.clone())
            .map(|(artifact, header)| {
                VerifiedV2FinalityArtifact::verify_for_header(header, artifact)
            })
            .transpose()
            .map_err(|error| format!("test storage rejected finality artifact: {error}"))
    }

    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<iroha_core::bridge::ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        Ok(self
            .bridge_verified_v2_finality_artifact(height)?
            .map(|verified| (verified, Vec::new())))
    }
}

fn state_from_fixture(fixture: &Fixture) -> TestState {
    TestState {
        network_id: fixture.network_id,
        retained_header: Some(fixture.block.header()),
        artifact: Ok(Some(fixture.artifact.clone())),
    }
}

#[test]
fn builder_returns_only_the_exact_self_contained_v2_artifact() {
    let fixture = fixture();
    let state = state_from_fixture(&fixture);

    let proof = build_finality_proof(&state, 1).expect("build exact proof");

    assert_eq!(proof.block_header, fixture.block.header());
    assert_eq!(proof.finality_artifact, fixture.artifact);
    assert_eq!(proof.finality_artifact.validator_set_pops, fixture.pops);
    proof
        .finality_artifact
        .verify()
        .expect("builder output verifies");
}

#[test]
fn builder_fails_closed_for_absent_or_unreadable_v2_artifact() {
    let fixture = fixture();
    let mut state = state_from_fixture(&fixture);
    state.artifact = Ok(None);
    assert_eq!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactNotFound(1))
    );

    state.artifact = Err("corrupt durable sidecar".to_owned());
    assert!(matches!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactRead { height: 1, .. })
    ));
}

#[test]
fn builder_rejects_artifact_block_network_and_durable_pop_attacks() {
    let fixture = fixture();
    let mut state = state_from_fixture(&fixture);
    let mut mismatched = fixture.artifact.clone();
    mismatched.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong canonical block"));
    state.artifact = Ok(Some(mismatched));
    assert!(matches!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactRead { height: 1, .. })
    ));

    let mut state = state_from_fixture(&fixture);
    state.network_id = fixture_network_id(b"wrong bridge network genesis");
    assert_eq!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactMismatch { height: 1 })
    );

    let mut state = state_from_fixture(&fixture);
    let mut missing_pop = fixture.artifact.clone();
    missing_pop.validator_set_pops.pop();
    state.artifact = Ok(Some(missing_pop));
    assert!(matches!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactRead { height: 1, .. })
    ));

    let mut state = state_from_fixture(&fixture);
    let mut forged_pop = fixture.artifact.clone();
    forged_pop.validator_set_pops[0][0] ^= 0x80;
    state.artifact = Ok(Some(forged_pop));
    assert!(matches!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactRead { height: 1, .. })
    ));
}

#[test]
fn stateless_verifier_enforces_height_and_context_anchor() {
    let fixture = fixture();
    let state = state_from_fixture(&fixture);
    let proof = build_finality_proof(&state, 1).expect("build exact proof");
    let config = FinalityProofVerificationConfig {
        expected_network_id: &fixture.network_id,
        expected_height: Some(1),
        trusted_context_id: fixture.artifact.context_id(),
    };
    verify_finality_proof(&proof, &config).expect("anchored proof verifies");

    let wrong_height = FinalityProofVerificationConfig {
        expected_height: Some(2),
        ..config.clone()
    };
    assert!(verify_finality_proof(&proof, &wrong_height).is_err());
}

#[test]
fn builder_rejects_cryptographically_invalid_durable_artifact() {
    let fixture = fixture();
    let mut state = state_from_fixture(&fixture);
    let mut artifact = fixture.artifact;
    artifact.commit_qc.aggregate_signature[0] ^= 0x80;
    state.artifact = Ok(Some(artifact));

    assert!(matches!(
        build_finality_proof(&state, 1),
        Err(BridgeFinalityError::FinalityArtifactRead { height: 1, .. })
    ));
}
