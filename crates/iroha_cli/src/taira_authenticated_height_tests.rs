//! Real executed blocks, current three-of-four certificates, and challenge-bound node attestations.

use super::*;
use iroha_core::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::{CommitCertificate, decode_versioned_signed_block},
    sumeragi::{SumeragiFootprint, SumeragiStatus},
    sumeragi_finality::SumeragiFinalityAttestationBody,
};
use iroha_sumeragi::{
    message::Qc,
    types::{AggregateSignature, Bitmap},
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
    proofs: Vec<SumeragiFinalityProof>,
    conflicting: SumeragiFinalityProof,
    instance: [u8; 32],
}

impl Fixture {
    fn chain() -> CertifiedTestChain {
        let mut config = TestChainConfig::new(World::new(), 10_000);
        config.chain_id = "fc56984b-2be7-431d-840e-21514d1883f0".into();
        config.consensus_mode = iroha_data_model::parameter::system::SumeragiConsensusMode::Npos;
        CertifiedTestChain::start(config).expect("real NPoS fixture chain")
    }

    fn new() -> Self {
        let mut keys = (0xC1..=0xC4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let mut chain = Self::chain();
        let genesis = chain.validated_genesis().clone();
        let instance = chain.instance().0;
        chain.commit_at(20_000, Vec::new());
        chain.commit_at(30_000, Vec::new());
        let proofs = (1..=3)
            .map(|height| {
                iroha_core::sumeragi::finality::build_proof(&chain.state().view(), height).unwrap()
            })
            .collect();
        let mut branch = Self::chain();
        branch.commit_at(21_000, Vec::new());
        let conflicting =
            iroha_core::sumeragi::finality::build_proof(&branch.state().view(), 2).unwrap();
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
        Self {
            genesis,
            keys,
            peers,
            proofs,
            conflicting,
            instance,
        }
    }

    fn observer(&self) -> AuthenticatedHeightObserverV1 {
        AuthenticatedHeightObserverV1::new(&self.genesis, self.peers.clone()).unwrap()
    }

    fn resign_certificate(&self, certificate: &mut Qc, view: u64, omitted: usize) {
        certificate.view = view;
        let chosen: Vec<u32> = (0..4)
            .filter(|index| *index != omitted)
            .map(|index| index as u32)
            .collect();
        certificate.signers = Bitmap::from_indices(4, chosen.iter().copied()).unwrap();
        let signatures: Vec<_> = chosen
            .iter()
            .map(|index| {
                Signature::try_new(
                    self.keys[*index as usize].private_key(),
                    &certificate.preimage(),
                )
                .unwrap()
            })
            .collect();
        certificate.agg_sig = AggregateSignature(
            iroha_crypto::bls_normal_aggregate_signatures(
                &signatures
                    .iter()
                    .map(|signature| signature.payload())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
            .try_into()
            .unwrap(),
        );
    }

    fn edit_certificate(
        proof: &mut SumeragiFinalityProof,
        edit: impl FnOnce(&mut CommitCertificate),
    ) {
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let mut certificate = block.commit_certificate().unwrap().clone();
        edit(&mut certificate);
        block.set_commit_certificate(Some(certificate));
        proof.block_wire = block.encode_wire().unwrap();
    }

    fn witness_variant(
        &self,
        proof: &SumeragiFinalityProof,
        view: u64,
        omitted: usize,
    ) -> SumeragiFinalityProof {
        let mut proof = proof.clone();
        if proof.height() > 1 {
            Self::edit_certificate(&mut proof, |certificate| {
                let mut qc: Qc = norito::decode_from_bytes(&certificate.commit_qc).unwrap();
                self.resign_certificate(&mut qc, view, omitted);
                certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
            });
        }
        proof
    }

    fn corrupt_signature(proof: &mut SumeragiFinalityProof) {
        Self::edit_certificate(proof, |certificate| {
            let mut qc: Qc = norito::decode_from_bytes(&certificate.commit_qc).unwrap();
            qc.agg_sig.0[0] ^= 1;
            certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
        });
    }

    fn attest(
        &self,
        index: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
    ) -> SumeragiFinalityAttestation {
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
        genesis: SumeragiFinalityProof,
        proof: SumeragiFinalityProof,
        challenge: [u8; 32],
    ) -> SumeragiFinalityAttestation {
        let peer = &self.peers[index];
        let status = SumeragiStatus {
            instance: self.instance,
            height: proof.height() + 1,
            view: 0,
            stage: 0,
            leader: Some(self.keys[0].public_key().clone()),
            proxy_tail: None,
            high_qc_view: None,
            level: 0,
            start_level: 0,
            t_retx_ms: 100,
            committed_height: proof.height(),
            applied_height: proof.height(),
            awaiting: false,
            signer: Some(peer.peer_id.public_key().clone()),
            unanchored: false,
            abstaining: false,
            halted: None,
            footprint: SumeragiFootprint::default(),
        };
        let body = SumeragiFinalityAttestationBody {
            challenge,
            network_id: NetworkId::from_genesis_hash(self.genesis.expected_hash()),
            node_fingerprint: peer.node_fingerprint,
            node_id: peer.peer_id.clone(),
            build_fingerprint: peer.build_fingerprint,
            config_fingerprint: peer.config_fingerprint,
            genesis_block_hash: self.genesis.expected_hash(),
            genesis_finality_proof: genesis,
            status,
            finality_proof: proof,
        };
        let signature =
            SignatureOf::try_from_hash(self.keys[index].private_key(), body.signing_hash())
                .unwrap();
        SumeragiFinalityAttestation { body, signature }
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    WrongIdentity,
    WrongInstance,
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
    ) -> Result<SumeragiFinalityAttestation> {
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
            value.body.config_fingerprint = Hash::new(b"changed config");
            value.signature = SignatureOf::try_from_hash(
                self.fixture.keys[peer].private_key(),
                value.body.signing_hash(),
            )
            .unwrap();
        }
        if peer == 3 && matches!(self.fault, Fault::WrongInstance) {
            value.body.status.instance[0] ^= 1;
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
        verifier: &mut SumeragiFinalityVerifier,
    ) -> Result<SumeragiFinalityProof> {
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

pub(super) fn convergence_evidence_fixture(
    height: u64,
) -> (
    iroha_genesis::ValidatedGenesisBundle,
    Vec<PeerV1>,
    json::Value,
) {
    let fixture = Fixture::new();
    let mut reads = Reads::new(&fixture, height);
    reads.distinct_witnesses = true;
    let HeightObservationV1::Verified(evidence) = poll(&mut fixture.observer(), &reads).unwrap()
    else {
        panic!("verified current convergence fixture")
    };
    (
        fixture.genesis,
        fixture.peers,
        json::to_value(&evidence).unwrap(),
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
fn authenticated_height_retained_evidence_revalidates_every_binding_and_certificate() {
    let fixture = Fixture::new();
    let HeightObservationV1::Verified(evidence) =
        poll(&mut fixture.observer(), &Reads::new(&fixture, 2)).unwrap()
    else {
        panic!("verified evidence")
    };
    let encoded = json::to_value(&evidence).unwrap();
    let restored = VerifiedCommittedHeightV1::validate_retained(
        &fixture.genesis,
        fixture.peers.clone(),
        encoded.clone(),
    )
    .unwrap();
    assert_eq!(restored.block_hash(), evidence.block_hash);
    assert_eq!(restored.committed_height(), evidence.committed_height());
    for mutation in 0..9 {
        let mut raw: RetainedCommittedHeightV1 = json::from_value(encoded.clone()).unwrap();
        match mutation {
            0 => raw.schema.push('x'),
            1 => raw.before_challenge = raw.after_challenge,
            2 => {
                raw.proofs.remove(0);
            }
            3 => Fixture::corrupt_signature(&mut raw.proofs[1]),
            4 => raw.peers.swap(0, 1),
            5 => raw.peers[3].after.body.challenge[0] ^= 1,
            6 => raw.block_hash = fixture.proofs[0].block_header.hash(),
            7 => raw.committed_height = NonZeroU64::new(1).unwrap(),
            _ => raw.peers[0].before.body.config_fingerprint = Hash::new(b"changed config"),
        };
        let changed = VerifiedCommittedHeightV1 {
            schema: raw.schema,
            network_id: raw.network_id,
            genesis_block_hash: raw.genesis_block_hash,
            committed_height: raw.committed_height,
            block_hash: raw.block_hash,
            before_challenge: raw.before_challenge,
            after_challenge: raw.after_challenge,
            proofs: raw.proofs,
            peers: raw.peers,
        };
        assert!(
            VerifiedCommittedHeightV1::validate_retained(
                &fixture.genesis,
                fixture.peers.clone(),
                json::to_value(&changed).unwrap()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
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
fn authenticated_height_rejects_foreign_instance_even_at_signed_genesis() {
    let fixture = Fixture::new();
    for height in [1, 2] {
        let mut reads = Reads::new(&fixture, height);
        reads.fault = Fault::WrongInstance;
        assert!(poll(&mut fixture.observer(), &reads).is_err());
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
    ) -> Result<SumeragiFinalityAttestation> {
        self.0.attest(peer, height, challenge, identity)
    }
    fn next_proof(
        &self,
        peer: usize,
        height: NonZeroU64,
        verifier: &mut SumeragiFinalityVerifier,
    ) -> Result<SumeragiFinalityProof> {
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
    assert_eq!(evidence.proofs[0], fixture.proofs[0]);
    for peer in evidence.peers {
        assert_eq!(
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
    let mut verifier = fixture.observer().authority.verifier().unwrap();
    for proof in &fixture.proofs {
        verifier.verify(proof).unwrap();
    }
    let retained = &fixture.proofs[1];
    let mut invalid = fixture.witness_variant(retained, 2, 0);
    Fixture::corrupt_signature(&mut invalid);
    assert!(verifier.verify_same_decision(retained, &invalid).is_err());
    let mut invalid = fixture.proofs[2].clone();
    Fixture::edit_certificate(&mut invalid, |certificate| {
        let mut header: iroha_sumeragi::message::BlockHeader =
            norito::decode_from_bytes(&certificate.consensus_header).unwrap();
        header.parent_result.0[0] ^= 1;
        certificate.consensus_header = norito::encode_canonical(&header).unwrap();
    });
    assert!(
        verifier
            .verify_same_decision(&fixture.proofs[2], &invalid)
            .is_err()
    );
}

#[test]
fn authenticated_height_rejects_signed_conflicting_decisions() {
    let fixture = Fixture::new();
    let authority = fixture.observer().authority;
    let mut branch = authority.anchor(&fixture.proofs[0]).unwrap();
    branch
        .verify(&fixture.conflicting)
        .expect("competing branch genuinely certified");
    let mut verifier = authority.anchor(&fixture.proofs[0]).unwrap();
    verifier.verify(&fixture.proofs[1]).unwrap();
    assert!(
        verifier
            .verify_same_decision(&fixture.proofs[1], &fixture.conflicting)
            .is_err()
    );
}

#[test]
fn authenticated_height_requires_authenticated_predecessor_for_alternate_witnesses() {
    let fixture = Fixture::new();
    let retained = &fixture.proofs[2];
    let alternate = fixture.witness_variant(retained, 2, 0);
    let mut verifier = fixture.observer().authority.verifier().unwrap();
    assert!(verifier.verify_same_decision(retained, &alternate).is_err());
    verifier.verify(&fixture.proofs[0]).unwrap();
    assert!(verifier.verify_same_decision(retained, &alternate).is_err());
    verifier.verify(&fixture.proofs[1]).unwrap();
    assert!(verifier.verify_same_decision(retained, &alternate).is_err());
    verifier.verify(retained).unwrap();
    verifier.verify_same_decision(retained, &alternate).unwrap();
    let mut wrong_pops = alternate;
    wrong_pops.committee.swap(0, 1);
    assert!(
        verifier
            .verify_same_decision(retained, &wrong_pops)
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
            .join("operation/proof-00000000000000000002.json");
        let original = std::fs::read(&path).unwrap();
        let mut changed = fixture().proofs[1].clone();
        Fixture::corrupt_signature(&mut changed);
        std::fs::write(&path, norito::json::to_vec(&changed).unwrap()).unwrap();
        let error = synchronize(&mut prefix, &authority, &journal, 3, 3).unwrap_err();
        assert!(error.to_string().contains("retained proof cache changed"));
        assert_eq!(prefix.authenticated_rows, 3);
        assert_eq!(prefix.proofs.get(&1), Some(&fixture().proofs[0]));
        // A fresh peer may use another valid certificate witness for this decision,
        // but immutable disk evidence must remain exactly the object already admitted.
        let variant = fixture().witness_variant(&fixture().proofs[1], 1, 1);
        prefix
            .verifier
            .as_ref()
            .unwrap()
            .verify_same_decision(&fixture().proofs[1], &variant)
            .unwrap();
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
        Fixture::corrupt_signature(&mut changed);
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
                0 => Fixture::corrupt_signature(&mut proof),
                1 => proof = fixture().proofs[2].clone(),
                2 => proof.committee[0].proof_of_possession[0] ^= 1,
                3 | 4 => Fixture::edit_certificate(&mut proof, |certificate| {
                    let mut header: iroha_sumeragi::message::BlockHeader =
                        norito::decode_from_bytes(&certificate.consensus_header).unwrap();
                    if mutation == 3 {
                        header.instance.0[0] ^= 1;
                    } else {
                        header.parent_hash.0[0] ^= 1;
                    }
                    certificate.consensus_header = norito::encode_canonical(&header).unwrap();
                }),
                _ => unreachable!(),
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
                        .optional_json::<SumeragiFinalityProof>("proof-00000000000000000002.json")
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
        prefix
            .verifier
            .as_ref()
            .unwrap()
            .verify_same_decision(retained, &fixture().proofs[1])
            .unwrap();
        let conflicting = fixture().conflicting.clone();
        let mut branch = authority.anchor(parent).unwrap();
        branch
            .verify(&conflicting)
            .expect("competing decision has a genuine valid certificate");
        assert!(
            prefix
                .verifier
                .as_ref()
                .unwrap()
                .verify_same_decision(retained, &conflicting)
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
