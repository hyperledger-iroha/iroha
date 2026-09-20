//! Real signed genesis, three-of-four certificates, and challenge-bound node attestations.

use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::consensus_v2::{self as wire, *},
    bridge::{
        BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2,
        BridgeFinalityAttestationBodyV1,
    },
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

struct Fixture {
    genesis: iroha_genesis::ValidatedGenesisBundle,
    keys: Vec<KeyPair>,
    peers: Vec<PeerV1>,
    proofs: Vec<BridgeFinalityProof>,
}

impl Fixture {
    fn new() -> Self {
        let mut keys = (110..114)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        // Reuse the canonical executed-and-signed fixture and its exact manifest binding.
        // A raw signed proposal has no deterministic outputs and cannot anchor deployment trust.
        let genesis = crate::taira_public_reset::deployment_validated_genesis_fixture();
        assert_eq!(
            genesis
                .validator_pops()
                .keys()
                .cloned()
                .map(PeerId::new)
                .collect::<std::collections::BTreeSet<_>>(),
            keys.iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect::<std::collections::BTreeSet<_>>(),
            "signed genesis must use the actual four fixture signing keys",
        );
        let block = genesis.block().clone();
        let peers = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let peer_id = PeerId::new(key.public_key().clone());
                PeerV1 {
                    torii_origin: format!("http://127.0.0.1:{}/", 18080 + index),
                    node_fingerprint: Hash::new(peer_id.encode()),
                    peer_id,
                    build_fingerprint: Hash::new(b"selected build"),
                    config_fingerprint: Hash::new(b"selected config"),
                }
            })
            .collect();
        let mut fixture = Self {
            genesis,
            keys,
            peers,
            proofs: Vec::new(),
        };
        let genesis_ms = u64::try_from(block.header().creation_time().as_millis()).unwrap();
        let first = fixture.proof(block.header().clone(), None);
        let second = fixture.proof(
            BlockHeader::new(
                NonZeroU64::new(2).unwrap(),
                Some(first.block_header.hash()),
                None,
                genesis_ms + 1,
                0,
            ),
            Some(&first),
        );
        let third = fixture.proof(
            BlockHeader::new(
                NonZeroU64::new(3).unwrap(),
                Some(second.block_header.hash()),
                None,
                genesis_ms + 2,
                0,
            ),
            Some(&second),
        );
        fixture.proofs = vec![first, second, third];
        fixture
    }

    fn observer(&self) -> AuthenticatedHeightObserverV1 {
        AuthenticatedHeightObserverV1::new(&self.genesis, self.peers.clone()).unwrap()
    }

    fn proof(
        &self,
        header: BlockHeader,
        previous: Option<&BridgeFinalityProof>,
    ) -> BridgeFinalityProof {
        let network = NetworkId::from_genesis_hash(self.genesis.expected_hash());
        let mint = self
            .genesis
            .consensus_metadata()
            .kagemusha_mint_finality
            .epoch_roster
            .bind_network_id(network)
            .unwrap();
        let roster = self
            .keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            network_id: network,
            protocol_version: wire::PROTOCOL_VERSION,
            height: header.height().get(),
            epoch: 0,
            kagemusha_mint_finality_epoch_id: mint.finality_epoch_id().unwrap(),
            kagemusha_mint_finality_epoch_roster: mint,
            epoch_end_height: 20,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: previous.map(|p| p.finality_artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).unwrap(),
            roster,
            nexus_amx_context_hash: Hash::new(b"fixture nexus"),
            execution_policy_hash: Hash::new(b"fixture execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x5A; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: header.prev_block_hash(),
            block_hash: header.hash(),
            payload_hash: Hash::new(b"fixture payload"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: header.height().get(),
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"parent state"),
            Hash::new(b"post state"),
            Hash::new(b"writes"),
            1,
            Hash::new(b"executed wire"),
        );
        let preimage = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = self.keys[..3]
            .iter()
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .unwrap()
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&refs).unwrap(),
        };
        let pops = self
            .keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect();
        BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: header,
            finality_artifact: wire::finality::V2FinalityArtifact::new(context, subject, qc, pops),
        }
    }

    fn resign_certificate(&self, certificate: &mut QuorumCertificate, view: u64, omitted: usize) {
        certificate.round.view = view;
        certificate.proposal_round = certificate.round;
        certificate.signers = (0..4)
            .filter(|index| *index != omitted)
            .map(|index| index as u32)
            .collect();
        let preimage = Vote {
            round: certificate.round,
            proposal_round: certificate.proposal_round,
            phase: certificate.phase,
            subject: certificate.subject,
            execution_commitment: certificate.execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(self.keys[*index as usize].private_key(), &preimage)
                    .unwrap()
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .unwrap();
    }

    fn witness_variant(
        &self,
        proof: &BridgeFinalityProof,
        view: u64,
        omitted: usize,
    ) -> BridgeFinalityProof {
        let mut proof = proof.clone();
        if let Some(parent) = proof
            .finality_artifact
            .height_context
            .parent_commit_qc
            .as_mut()
        {
            self.resign_certificate(parent, view + 1, (omitted + 1) % 4);
        }
        self.resign_certificate(&mut proof.finality_artifact.commit_qc, view, omitted);
        proof
    }

    fn attest(
        &self,
        index: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
    ) -> BridgeFinalityAttestationV1 {
        self.attest_proofs(
            index,
            self.proofs[0].clone(),
            self.proofs[usize::try_from(height.get() - 1).unwrap()].clone(),
            challenge,
        )
    }

    fn attest_proofs(
        &self,
        index: usize,
        genesis: BridgeFinalityProof,
        proof: BridgeFinalityProof,
        challenge: [u8; 32],
    ) -> BridgeFinalityAttestationV1 {
        let artifact = &proof.finality_artifact;
        let context = &artifact.height_context;
        let peer = &self.peers[index];
        let status = SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: peer.node_fingerprint,
            build_fingerprint: peer.build_fingerprint,
            config_fingerprint: peer.config_fingerprint,
            restart_required: false,
            height_context_id: context.id(),
            height: artifact.height,
            view: artifact.commit_qc.round.view,
            phase: SumeragiV2StatusPhase::PendingApply,
            leader: context.leader(artifact.commit_qc.round.view),
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Applied,
            pending_persistence_id: None,
            last_committed_height: artifact.height,
            last_committed_subject: Some(artifact.subject),
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: 4,
                quorum: context.quorum,
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate: artifact.commit_qc.as_ref(),
                validator_count: 4,
                signer_count: 3,
                min_signers: 3,
                signed_power: 3,
                total_power: 4,
            }),
            liveness: SumeragiV2LivenessStatus::default(),
        };
        let body = BridgeFinalityAttestationBodyV1 {
            version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
            challenge,
            network_id: context.network_id,
            node_fingerprint: peer.node_fingerprint,
            node_id: peer.peer_id.clone(),
            genesis_block_hash: self.genesis.expected_hash(),
            genesis_finality_proof: genesis,
            status,
            finality_proof: proof,
        };
        let signature =
            SignatureOf::try_from_hash(self.keys[index].private_key(), body.signing_hash())
                .unwrap();
        BridgeFinalityAttestationV1 { body, signature }
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    WrongIdentity,
    InvalidSignature,
    SkipProof,
    GenericFailure,
    TipRace,
    TipRaceWithChangedIdentity,
    Late,
}

struct Reads<'a> {
    fixture: &'a Fixture,
    heights: [u64; 4],
    after_heights: [u64; 4],
    attest_calls: [AtomicUsize; 4],
    proof_calls: Mutex<Vec<u64>>,
    calls: AtomicUsize,
    fault: Fault,
    distinct_witnesses: bool,
}

impl<'a> Reads<'a> {
    fn new(fixture: &'a Fixture, height: u64) -> Self {
        Self {
            fixture,
            heights: [height; 4],
            after_heights: [height; 4],
            attest_calls: std::array::from_fn(|_| AtomicUsize::new(0)),
            proof_calls: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
            fault: Fault::None,
            distinct_witnesses: false,
        }
    }
}

impl HeightReads for Reads<'_> {
    fn tip(&self, peer: usize) -> Result<u64> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(if self.attest_calls[peer].load(Ordering::Relaxed) == 0 {
            self.heights[peer]
        } else {
            self.after_heights[peer]
        })
    }
    fn attest(
        &self,
        peer: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
        _: &PeerId,
    ) -> Result<BridgeFinalityAttestationV1> {
        let round = self.attest_calls[peer].fetch_add(1, Ordering::Relaxed);
        if peer == 0
            && matches!(
                self.fault,
                Fault::TipRace | Fault::TipRaceWithChangedIdentity
            )
        {
            let expected = &self.fixture.peers[peer];
            let response =
                iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
                    requested_height: height.get(),
                    applied_height: height.get() + 1,
                    status_height: height.get(),
                    challenge,
                    node_id: expected.peer_id.clone(),
                    network_id: NetworkId::from_genesis_hash(self.fixture.genesis.expected_hash()),
                };
            return Err(
                iroha::client::BridgeFinalityAttestationTipMismatch::from_response(
                    response,
                    height,
                    challenge,
                    &expected.peer_id,
                    NetworkId::from_genesis_hash(self.fixture.genesis.expected_hash()),
                )
                .unwrap()
                .into(),
            );
        }
        if peer == 0 && matches!(self.fault, Fault::GenericFailure) {
            return Err(eyre!("generic404 is fixed"));
        }
        if matches!(self.fault, Fault::Late) {
            std::thread::sleep(Duration::from_millis(30));
        }
        let mut value = if self.distinct_witnesses {
            let view = 2 + peer as u64 + 4 * round as u64;
            let genesis = self
                .fixture
                .witness_variant(&self.fixture.proofs[0], view, peer);
            let proof = self.fixture.witness_variant(
                &self.fixture.proofs[usize::try_from(height.get() - 1).unwrap()],
                view,
                peer,
            );
            self.fixture.attest_proofs(peer, genesis, proof, challenge)
        } else {
            self.fixture.attest(peer, height, challenge)
        };
        if peer == 3
            && matches!(
                self.fault,
                Fault::WrongIdentity | Fault::TipRaceWithChangedIdentity
            )
        {
            value.body.status.config_fingerprint = Hash::new(b"changed config");
            value.signature = SignatureOf::try_from_hash(
                self.fixture.keys[peer].private_key(),
                value.body.signing_hash(),
            )
            .unwrap();
        }
        if peer == 3 && matches!(self.fault, Fault::InvalidSignature) {
            value.body.challenge[0] ^= 1;
        }
        Ok(value)
    }
    fn next_proof(
        &self,
        _: usize,
        height: NonZeroU64,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof> {
        self.proof_calls.lock().unwrap().push(height.get());
        let index = if matches!(self.fault, Fault::SkipProof) {
            height.get()
        } else {
            height.get() - 1
        };
        let proof = self.fixture.proofs[usize::try_from(index).unwrap()].clone();
        verifier.verify(&proof)?;
        Ok(proof)
    }
}

fn poll(
    observer: &mut AuthenticatedHeightObserverV1,
    reads: &Reads<'_>,
) -> Result<HeightObservationV1> {
    observer.observe_with(
        reads,
        369,
        Instant::now() + Duration::from_secs(10),
        [1; 32],
        [2; 32],
    )
}

#[test]
fn authenticated_height_requires_prepared_genesis_roster_and_exact_peer_selection() {
    let fixture = Fixture::new();
    fixture.observer();
    let mut peers = fixture.peers.clone();
    peers[1] = peers[0].clone();
    assert!(AuthenticatedHeightObserverV1::new(&fixture.genesis, peers).is_err());
    let mut peers = fixture.peers.clone();
    peers[0].node_fingerprint = Hash::new(b"foreign node");
    assert!(AuthenticatedHeightObserverV1::new(&fixture.genesis, peers).is_err());
    let mut peers = fixture.peers.clone();
    peers[0].torii_origin = "http://secret@127.0.0.1/".into();
    assert!(AuthenticatedHeightObserverV1::new(&fixture.genesis, peers).is_err());
}

#[test]
fn authenticated_height_verifies_contiguous_chain_and_fresh_four_peer_evidence() {
    let fixture = Fixture::new();
    let mut observer = fixture.observer();
    let reads = Reads::new(&fixture, 2);
    let HeightObservationV1::Verified(evidence) = poll(&mut observer, &reads).unwrap() else {
        panic!("expected authenticated height")
    };
    assert_eq!(evidence.committed_height().get(), 2);
    assert_eq!(evidence.block_hash, fixture.proofs[1].block_header.hash());
    assert_eq!(evidence.proofs, fixture.proofs[..2]);
    for height in 1..=2 {
        assert_eq!(
            evidence.proof_at(NonZeroU64::new(height).unwrap()),
            Some(&fixture.proofs[usize::try_from(height - 1).unwrap()])
        );
    }
    assert!(evidence.proof_at(NonZeroU64::new(3).unwrap()).is_none());
    assert!(
        evidence
            .proof_at(NonZeroU64::new(u64::MAX).unwrap())
            .is_none()
    );

    assert_eq!(*reads.proof_calls.lock().unwrap(), vec![2]);
    assert_eq!(evidence.peers.len(), 4);
    for peer in &evidence.peers {
        peer.before.verify().unwrap();
        peer.after.verify().unwrap();
        assert_eq!(peer.before.body.challenge, [1; 32]);
        assert_eq!(peer.after.body.challenge, [2; 32]);
    }
    assert!(!norito::json::to_vec(&evidence).unwrap().is_empty());
    assert!(matches!(
        poll(&mut observer, &reads).unwrap(),
        HeightObservationV1::Pending
    ));
    let reads = Reads::new(&fixture, 3);
    let HeightObservationV1::Verified(evidence) = poll(&mut observer, &reads).unwrap() else {
        panic!("expected successor")
    };
    assert_eq!(evidence.committed_height().get(), 3);
    assert_eq!(
        *reads.proof_calls.lock().unwrap(),
        vec![3],
        "verified history is retained but each peer is read again"
    );
}

#[test]
fn authenticated_height_rejects_skips_signatures_and_changed_identity() {
    let fixture = Fixture::new();
    for fault in [
        Fault::SkipProof,
        Fault::InvalidSignature,
        Fault::WrongIdentity,
        Fault::GenericFailure,
        Fault::TipRaceWithChangedIdentity,
    ] {
        let mut reads = Reads::new(&fixture, 2);
        reads.fault = fault;
        let mut observer = fixture.observer();
        assert!(poll(&mut observer, &reads).is_err());
        assert_eq!(observer.emitted_height, 0);
    }
}

#[test]
fn authenticated_height_progress_never_emits_a_mixed_or_racing_checkpoint() {
    let fixture = Fixture::new();
    let mut reads = Reads::new(&fixture, 2);
    reads.heights[0] = 1;
    assert!(matches!(
        poll(&mut fixture.observer(), &reads).unwrap(),
        HeightObservationV1::Pending
    ));
    let mut reads = Reads::new(&fixture, 2);
    reads.after_heights[0] = 3;
    assert!(matches!(
        poll(&mut fixture.observer(), &reads).unwrap(),
        HeightObservationV1::Pending
    ));
    assert_eq!(*reads.proof_calls.lock().unwrap(), vec![2, 3]);
    let mut reads = Reads::new(&fixture, 2);
    reads.fault = Fault::TipRace;
    assert!(matches!(
        poll(&mut fixture.observer(), &reads).unwrap(),
        HeightObservationV1::Pending
    ));
}

#[test]
fn authenticated_height_deadline_prevents_dispatch_and_late_completion() {
    let fixture = Fixture::new();
    let mut observer = fixture.observer();
    let reads = Reads::new(&fixture, 2);
    assert!(
        observer
            .observe_with(&reads, 369, Instant::now(), [1; 32], [2; 32])
            .is_err()
    );
    assert_eq!(reads.calls.load(Ordering::Relaxed), 0);
    assert!(
        observer
            .observe_with(
                &reads,
                369,
                Instant::now() + Duration::from_secs(1),
                [1; 32],
                [1; 32]
            )
            .is_err()
    );
    assert_eq!(reads.calls.load(Ordering::Relaxed), 0);
    let mut reads = Reads::new(&fixture, 2);
    reads.fault = Fault::Late;
    assert!(
        observer
            .observe_with(
                &reads,
                369,
                Instant::now() + Duration::from_millis(10),
                [1; 32],
                [2; 32]
            )
            .is_err()
    );
    assert_eq!(observer.emitted_height, 0);
}

#[test]
fn authenticated_height_repeat_current_preserves_freshness_and_advancing_contract() {
    let fixture = Fixture::new();
    let mut observer = fixture.observer();
    let first = Reads::new(&fixture, 2);
    assert!(matches!(
        poll(&mut observer, &first).unwrap(),
        HeightObservationV1::Verified(_)
    ));
    let repeat = Reads::new(&fixture, 2);
    let HeightObservationV1::Verified(evidence) = observer
        .observe_with_policy(
            &repeat,
            369,
            Instant::now() + Duration::from_secs(10),
            [3; 32],
            [4; 32],
            true,
        )
        .unwrap()
    else {
        panic!("same height must have fresh evidence");
    };
    assert_eq!(evidence.proofs, fixture.proofs[..2]);
    assert!(repeat.proof_calls.lock().unwrap().is_empty());
    for peer in &evidence.peers {
        assert_eq!(peer.before.body.challenge, [3; 32]);
        assert_eq!(peer.after.body.challenge, [4; 32]);
        peer.before.verify().unwrap();
        peer.after.verify().unwrap();
    }
    assert!(
        repeat
            .attest_calls
            .iter()
            .all(|calls| calls.load(Ordering::Relaxed) == 2)
    );
    assert!(
        matches!(
            poll(&mut observer, &Reads::new(&fixture, 2)).unwrap(),
            HeightObservationV1::Pending
        ),
        "original API remains strictly advancing"
    );
    assert!(
        matches!(
            observer
                .observe_with_policy(
                    &Reads::new(&fixture, 1),
                    369,
                    Instant::now() + Duration::from_secs(10),
                    [5; 32],
                    [6; 32],
                    true
                )
                .unwrap(),
            HeightObservationV1::Pending
        ),
        "fresh-current cannot regress"
    );
}

struct RestartReads<'a>(Reads<'a>);
impl HeightReads for RestartReads<'_> {
    fn transport_pending(&self, error: &eyre::Report) -> bool {
        crate::taira::observation_transport_unavailable(error)
    }
    fn tip(&self, peer: usize) -> Result<u64> {
        if peer == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into());
        }
        self.0.tip(peer)
    }
    fn attest(
        &self,
        peer: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
        identity: &PeerId,
    ) -> Result<BridgeFinalityAttestationV1> {
        self.0.attest(peer, height, challenge, identity)
    }
    fn next_proof(
        &self,
        peer: usize,
        height: NonZeroU64,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof> {
        self.0.next_proof(peer, height, verifier)
    }
}

#[test]
fn authenticated_height_restart_transport_never_masks_fixed_peer_identity() {
    let fixture = Fixture::new();
    let mut observer = fixture.observer();
    let reads = RestartReads(Reads::new(&fixture, 2));
    assert!(matches!(
        observer
            .observe_with_policy(
                &reads,
                369,
                Instant::now() + Duration::from_secs(10),
                [7; 32],
                [8; 32],
                true
            )
            .unwrap(),
        HeightObservationV1::Pending
    ));
    let mut reads = RestartReads(Reads::new(&fixture, 2));
    reads.0.fault = Fault::WrongIdentity;
    assert!(
        observer
            .observe_with_policy(
                &reads,
                369,
                Instant::now() + Duration::from_secs(10),
                [9; 32],
                [10; 32],
                true
            )
            .is_err(),
        "a disconnected peer cannot mask another peer's substituted identity"
    );
    assert!(
        matches!(
            observer
                .observe_with_policy(
                    &Reads::new(&fixture, 2),
                    369,
                    Instant::now() + Duration::from_secs(10),
                    [11; 32],
                    [12; 32],
                    true
                )
                .unwrap(),
            HeightObservationV1::Verified(_)
        ),
        "reconnect resumes the authenticated prefix without a write"
    );
}

#[test]
fn authenticated_height_accepts_independent_certificate_witnesses() {
    let fixture = Fixture::new();
    let mut observer = fixture.observer();
    let mut reads = Reads::new(&fixture, 3);
    reads.distinct_witnesses = true;
    let HeightObservationV1::Verified(evidence) = poll(&mut observer, &reads).unwrap() else {
        panic!("independent valid quorum witnesses must certify one checkpoint")
    };
    assert_eq!(evidence.committed_height().get(), 3);
    assert_eq!(evidence.block_hash, fixture.proofs[2].block_header.hash());
    assert_eq!(*reads.proof_calls.lock().unwrap(), vec![2, 3]);
    assert_ne!(evidence.proofs[0], fixture.proofs[0]);
    for peer in evidence.peers {
        assert_ne!(
            peer.before.body.genesis_finality_proof,
            peer.after.body.genesis_finality_proof
        );
        assert_ne!(
            peer.before.body.finality_proof,
            peer.after.body.finality_proof
        );
        assert_ne!(peer.before.body.finality_proof, fixture.proofs[2]);
        peer.before.verify().unwrap();
        peer.after.verify().unwrap();
    }
}

#[test]
fn authenticated_height_rejects_invalid_current_and_parent_witnesses() {
    let fixture = Fixture::new();
    let observer = fixture.observer();
    let retained = &fixture.proofs[1];
    let predecessor = &fixture.proofs[0];
    let valid = fixture.witness_variant(retained, 2, 0);
    for parent in [false, true] {
        let mut invalid = valid.clone();
        let certificate = if parent {
            invalid
                .finality_artifact
                .height_context
                .parent_commit_qc
                .as_mut()
                .unwrap()
        } else {
            &mut invalid.finality_artifact.commit_qc
        };
        let mut different_round = certificate.clone();
        fixture.resign_certificate(&mut different_round, certificate.round.view + 1, 0);
        certificate.aggregate_signature = different_round.aggregate_signature;
        if parent {
            // The current certificate remains valid: context identity deliberately excludes
            // parent witness bytes. The contiguous verifier must verify this parent itself.
            iroha_data_model::bridge::verify_bridge_finality_proof(
                &invalid,
                &observer.authority.network,
            )
            .unwrap();
        }
        assert!(
            observer
                .authority
                .verify_same_decision(retained, Some(predecessor), &invalid)
                .is_err(),
            "parent={parent}"
        );
    }
}

#[test]
fn authenticated_height_rejects_signed_conflicting_decisions() {
    let fixture = Fixture::new();
    let observer = fixture.observer();
    for index in 0_usize..2 {
        let retained = &fixture.proofs[index];
        let predecessor = index.checked_sub(1).map(|index| &fixture.proofs[index]);
        for conflict in 0..3 {
            let mut candidate = fixture.witness_variant(retained, 2, 0);
            let artifact = &mut candidate.finality_artifact;
            match conflict {
                0 => {
                    artifact.commit_qc.execution_commitment =
                        ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                            Hash::new(b"different parent state"),
                            Hash::new(b"different post state"),
                            Hash::new(b"different writes"),
                            1,
                            Hash::new(b"different executed wire"),
                        );
                }
                1 => {
                    artifact.subject.payload_hash = Hash::new(b"different payload");
                    artifact.commit_qc.subject = artifact.subject;
                }
                2 => {
                    artifact.height_context.nexus_amx_context_hash =
                        Hash::new(b"different context");
                    artifact.commit_qc.round.context_id = artifact.height_context.id();
                }
                _ => unreachable!(),
            }
            fixture.resign_certificate(&mut artifact.commit_qc, 2, 0);
            if let Some(predecessor) = predecessor {
                let mut independent = observer.authority.anchor(predecessor).unwrap();
                independent
                    .verify(&candidate)
                    .expect("conflicting decision has valid current and parent certificates");
            } else {
                observer
                    .authority
                    .anchor(&candidate)
                    .expect("conflicting genesis decision has a valid certificate");
            }
            assert_eq!(
                candidate.block_header.hash(),
                retained.block_header.hash(),
                "block hash alone cannot identify the finality decision"
            );
            assert!(
                observer
                    .authority
                    .verify_same_decision(retained, predecessor, &candidate)
                    .is_err(),
                "conflict={conflict}"
            );
        }
    }
}

#[test]
fn authenticated_height_requires_authenticated_predecessor_for_alternate_witnesses() {
    let fixture = Fixture::new();
    let observer = fixture.observer();
    let retained = &fixture.proofs[2];
    let alternate = fixture.witness_variant(retained, 2, 0);
    assert!(
        observer
            .authority
            .verify_same_decision(retained, None, &alternate)
            .is_err()
    );
    assert!(
        observer
            .authority
            .verify_same_decision(retained, Some(&fixture.proofs[0]), &alternate)
            .is_err()
    );
    observer
        .authority
        .verify_same_decision(retained, Some(&fixture.proofs[1]), &alternate)
        .unwrap();
    observer
        .authority
        .verify_same_decision(
            &fixture.proofs[0],
            None,
            &fixture.witness_variant(&fixture.proofs[0], 2, 0),
        )
        .unwrap();
    let mut wrong_pops = alternate;
    wrong_pops.finality_artifact.validator_set_pops.swap(0, 1);
    assert!(
        observer
            .authority
            .verify_same_decision(retained, Some(&fixture.proofs[1]), &wrong_pops)
            .is_err()
    );
}

#[cfg(unix)]
mod deployment_prefix {
    use super::*;
    use crate::taira_dataspace_deploy::{
        Journal,
        finality::{Authority, MAX_NEW_PROOFS, ProofPrefix, TrustV1},
    };
    use std::{os::unix::fs::PermissionsExt as _, time::Instant};

    fn fixture() -> &'static Fixture {
        static FIXTURE: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
        FIXTURE.get_or_init(Fixture::new)
    }

    fn authority() -> Authority {
        let fixture = fixture();
        TrustV1 {
            genesis_public_key: fixture.genesis.public_key().clone(),
            genesis_signed_wire_hex: hex::encode(fixture.genesis.canonical_wire()),
            peers: fixture.peers.clone(),
        }
        .authority(NetworkId::from_genesis_hash(
            fixture.genesis.expected_hash(),
        ))
        .unwrap()
    }

    fn journal() -> (tempfile::TempDir, Journal) {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let journal = Journal::open(&root.path().join("operation"), true).unwrap();
        (root, journal)
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    fn synchronize(
        prefix: &mut ProofPrefix,
        authority: &Authority,
        journal: &Journal,
        height: usize,
        budget: usize,
    ) -> eyre::Result<bool> {
        let fixture = fixture();
        prefix.synchronize(
            authority,
            journal,
            &fixture.proofs[height - 1],
            &fixture.proofs[0],
            deadline(),
            budget,
            |height, trial| {
                let proof = fixture.proofs[usize::try_from(height.get() - 1).unwrap()].clone();
                trial.verify(&proof)?;
                Ok(proof)
            },
        )
    }

    fn assert_still_at_genesis(prefix: &ProofPrefix) {
        assert_eq!(prefix.proofs.len(), 1);
        assert_eq!(prefix.proofs.get(&1), Some(&fixture().proofs[0]));
        let mut trial = prefix.verifier.clone().unwrap();
        trial
            .verify(&fixture().proofs[1])
            .expect("next exact successor still admissible");
    }

    #[test]
    fn deployment_prefix_batches_and_pending_retries_authenticate_each_height_once() {
        let (_root, journal) = journal();
        let authority = authority();
        let mut prefix = ProofPrefix::default();
        for budget in [0, MAX_NEW_PROOFS + 1] {
            assert!(synchronize(&mut prefix, &authority, &journal, 3, budget).is_err());
            assert!(prefix.proofs.is_empty());
            assert!(prefix.verifier.is_none());
            assert_eq!(prefix.authenticated_rows, 0);
        }
        assert!(!synchronize(&mut prefix, &authority, &journal, 3, 2).unwrap());
        assert_eq!(prefix.proofs.len(), 2);
        assert_eq!(prefix.authenticated_rows, 2);
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 2).unwrap());
        assert_eq!(prefix.proofs.len(), 3);
        assert_eq!(prefix.authenticated_rows, 3);
        for _ in 0..2 {
            assert!(
                prefix
                    .synchronize(
                        &authority,
                        &journal,
                        &fixture().proofs[2],
                        &fixture().proofs[0],
                        deadline(),
                        2,
                        |_, _| panic!("late peer retry must not refetch a retained prefix"),
                    )
                    .unwrap()
            );
            assert_eq!(
                prefix.authenticated_rows, 3,
                "late peer retries must not repeat crypto admission"
            );
        }
    }

    #[test]
    fn deployment_prefix_rejects_changed_or_deleted_authenticated_disk_proof() {
        let (root, journal) = journal();
        let authority = authority();
        let mut prefix = ProofPrefix::default();
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap());
        let path = root
            .path()
            .join("operation/proof-00000000000000000001.json");
        let original = std::fs::read(&path).unwrap();
        let mut changed = fixture().proofs[0].clone();
        changed.finality_artifact.commit_qc.aggregate_signature[0] ^= 1;
        std::fs::write(&path, norito::json::to_vec(&changed).unwrap()).unwrap();
        let error = synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap_err();
        assert!(error.to_string().contains("retained proof cache changed"));
        assert_eq!(prefix.authenticated_rows, 3);
        assert_eq!(prefix.proofs.get(&1), Some(&fixture().proofs[0]));
        // A fresh peer may use another valid certificate witness for this decision,
        // but immutable disk evidence must remain exactly the object already admitted.
        let variant = fixture().witness_variant(&fixture().proofs[0], 1, 1);
        authority.anchor(&variant).unwrap();
        std::fs::write(&path, norito::json::to_vec(&variant).unwrap()).unwrap();
        let error = synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap_err();
        assert!(error.to_string().contains("retained proof cache changed"));
        assert_eq!(prefix.authenticated_rows, 3);
        std::fs::write(&path, &original).unwrap();
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap());
        std::fs::remove_file(path).unwrap();
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 3).is_err());
        assert_eq!(prefix.authenticated_rows, 3);
        assert_eq!(prefix.proofs.len(), 3);
    }

    #[test]
    fn deployment_prefix_fresh_owner_reauthenticates_corrupted_disk_prefix() {
        let (root, journal) = journal();
        let mut prefix = ProofPrefix::default();
        assert!(synchronize(&mut prefix, &authority(), &journal, 3, 3).unwrap());
        drop(prefix);
        drop(journal);
        let mut changed = fixture().proofs[1].clone();
        changed.finality_artifact.commit_qc.aggregate_signature[0] ^= 1;
        std::fs::write(
            root.path()
                .join("operation/proof-00000000000000000002.json"),
            norito::json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        let journal = Journal::open(&root.path().join("operation"), false).unwrap();
        let mut fresh = ProofPrefix::default();
        assert!(synchronize(&mut fresh, &authority(), &journal, 3, 3).is_err());
        assert_eq!(
            fresh.authenticated_rows, 1,
            "a fresh owner verifies genesis before rejecting the corrupt successor"
        );
        assert_still_at_genesis(&fresh);
    }

    #[test]
    fn deployment_prefix_invalid_successor_does_not_advance_retained_verifier() {
        let (_root, journal) = journal();
        let authority = authority();
        let mut prefix = ProofPrefix::default();
        assert!(synchronize(&mut prefix, &authority, &journal, 1, 1).unwrap());
        for mutation in 0..5 {
            let mut proof = fixture().proofs[1].clone();
            match mutation {
                0 => proof.finality_artifact.commit_qc.aggregate_signature[0] ^= 1,
                1 => proof = fixture().proofs[2].clone(),
                2 => proof.finality_artifact.validator_set_pops[0][0] ^= 1,
                3 => {
                    proof.finality_artifact.height_context.network_id =
                        NetworkId::from_genesis_hash(
                            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                                b"foreign network",
                            )),
                        );
                }
                _ => {
                    let header = BlockHeader::new(
                        NonZeroU64::new(2).unwrap(),
                        Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign parent"))),
                        None,
                        fixture().proofs[1].block_header.creation_time_ms,
                        0,
                    );
                    proof = fixture().proof(header, Some(&fixture().proofs[0]));
                }
            }
            assert!(
                prefix
                    .synchronize(
                        &authority,
                        &journal,
                        &fixture().proofs[1],
                        &fixture().proofs[0],
                        deadline(),
                        1,
                        |_, trial| {
                            trial.verify(&proof)?;
                            Ok(proof.clone())
                        },
                    )
                    .is_err(),
                "mutation {mutation}"
            );
            assert_eq!(prefix.authenticated_rows, 1);
            assert_still_at_genesis(&prefix);
        }
        assert!(synchronize(&mut prefix, &authority, &journal, 2, 1).unwrap());
        assert_eq!(prefix.authenticated_rows, 2);
    }

    #[test]
    fn deployment_prefix_publication_or_deadline_failure_does_not_commit_trial() {
        for expire in [false, true] {
            let (_root, journal) = journal();
            let authority = authority();
            let mut prefix = ProofPrefix::default();
            assert!(synchronize(&mut prefix, &authority, &journal, 1, 1).unwrap());
            let verified = std::cell::Cell::new(false);
            let result = if expire {
                let proof = fixture().proofs[1].clone();
                authority.roster(&proof).unwrap();
                let mut trial = prefix.verifier.clone().unwrap();
                trial.verify(&proof).unwrap();
                verified.set(true);
                // Exercise the production publication boundary after genuine verification,
                // with an already elapsed fixed deadline and no timing-dependent sleep.
                prefix.publish_verified(&journal, proof, trial, true, Instant::now())
            } else {
                prefix
                    .synchronize(
                        &authority,
                        &journal,
                        &fixture().proofs[1],
                        &fixture().proofs[0],
                        deadline(),
                        1,
                        |_, trial| {
                            let proof = fixture().proofs[1].clone();
                            trial.verify(&proof)?;
                            verified.set(true);
                            journal.install_json("proof-00000000000000000002.json", &proof)?;
                            Ok(proof)
                        },
                    )
                    .map(|_| ())
            };
            let error = result.unwrap_err();
            if expire {
                assert_eq!(
                    error.downcast_ref::<std::io::Error>().unwrap().kind(),
                    std::io::ErrorKind::TimedOut
                );
                assert!(
                    journal
                        .optional_json::<BridgeFinalityProof>("proof-00000000000000000002.json")
                        .unwrap()
                        .is_none()
                );
            }
            assert_still_at_genesis(&prefix);
            assert!(
                verified.get(),
                "failure control must follow genuine native verification"
            );
            assert_eq!(
                prefix.authenticated_rows, 2,
                "failed publication or elapsed deadline must follow verification without committing it"
            );
        }
    }

    #[test]
    fn deployment_prefix_lower_tip_keeps_frontier_and_rejects_conflicting_decision() {
        let (_root, journal) = journal();
        let authority = authority();
        let mut prefix = ProofPrefix::default();
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap());
        assert!(synchronize(&mut prefix, &authority, &journal, 2, 1).unwrap());
        assert_eq!(prefix.proofs.len(), 3);
        assert_eq!(prefix.authenticated_rows, 3);
        let retained = prefix.proofs.get(&2).unwrap();
        let parent = prefix.proofs.get(&1).unwrap();
        authority
            .verify_same_decision(retained, Some(parent), &fixture().proofs[1])
            .unwrap();
        let header = BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(parent.block_header.hash()),
            None,
            retained.block_header.creation_time_ms + 1,
            0,
        );
        let conflicting = fixture().proof(header, Some(parent));
        let mut branch = authority.anchor(parent).unwrap();
        branch
            .verify(&conflicting)
            .expect("competing decision has a genuine valid certificate");
        assert!(
            authority
                .verify_same_decision(retained, Some(parent), &conflicting)
                .is_err()
        );
        let mut trial = prefix.verifier.clone().unwrap();
        assert!(
            trial.verify(&fixture().proofs[2]).is_err(),
            "lower tip must not rewind the retained verifier"
        );
        assert!(synchronize(&mut prefix, &authority, &journal, 3, 1).unwrap());
        assert_eq!(prefix.authenticated_rows, 3);
    }
}
