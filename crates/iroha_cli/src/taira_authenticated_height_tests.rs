//! Real signed genesis, three-of-four certificates, and challenge-bound node attestations.

use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::consensus_v2::{self as wire, *},
    bridge::{
        BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2,
        BridgeFinalityAttestationBodyV1,
    },
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
        KagemushaMintFinalityGenesisParametersV1,
    },
    parameter::system::SumeragiConsensusMode,
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
        let validators = keys.iter().enumerate().map(|(index, key)| {
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[u8::try_from(index + 10).unwrap(); 32], 0, PeerId::new(key.public_key().clone()),
            ).unwrap()
        }).collect();
        let mint = KagemushaMintFinalityGenesisParametersV1 {
            epoch_roster: KagemushaMintFinalityEpochRosterTemplateV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                epoch: 0,
                validators,
            },
            next_epoch_roster: None,
        };
        let topology = keys
            .iter()
            .map(|key| {
                iroha_genesis::GenesisTopologyEntry::new(
                    PeerId::new(key.public_key().clone()),
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                )
            })
            .collect();
        let mut npos = iroha_data_model::parameter::system::SumeragiNposParameters::default();
        npos.max_validators = 4;
        npos.epoch_length_blocks = NonZeroU64::new(20).unwrap();
        npos.evidence_horizon_blocks = 20;
        npos.slashing_delay_blocks = 1;
        npos.validate().expect("valid signed short-epoch fixture parameters");
        let manifest = iroha_genesis::GenesisBuilder::new_without_executor(
            "authenticated-height-fixture".into(),
            ".",
        )
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(mint)
        .set_topology(topology)
        .append_parameter(iroha_data_model::parameter::Parameter::Custom(
            npos.into_custom_parameter(),
        ))
        .build_raw()
        .unwrap()
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta();
        let genesis_key = KeyPair::try_from_seed(vec![101; 32], Algorithm::Ed25519).unwrap();
        let block = manifest.clone().build_and_sign(&genesis_key).unwrap().0;
        let genesis = iroha_genesis::validate_prepared_genesis_bundle(
            &block.encode_wire().unwrap(),
            &manifest,
            genesis_key.public_key(),
            block.hash(),
        )
        .unwrap();
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

    fn attest(
        &self,
        index: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
    ) -> BridgeFinalityAttestationV1 {
        let proof = self.proofs[usize::try_from(height.get() - 1).unwrap()].clone();
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
            view: 0,
            phase: SumeragiV2StatusPhase::PendingApply,
            leader: context.leader(0),
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
            genesis_finality_proof: self.proofs[0].clone(),
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
        self.attest_calls[peer].fetch_add(1, Ordering::Relaxed);
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
        let mut value = self.fixture.attest(peer, height, challenge);
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
    assert_eq!(evidence.block_hash(), fixture.proofs[1].block_header.hash());
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
