//! Synthetic signed chains of 4, 7 and 10 validators with committee rotations,
//! and challenge-bound attestations over them.
//!
//! The four-validator cases port the scenarios of the frozen `iroha_cli`
//! Taira verifiers (`taira_authenticated_height_tests.rs`,
//! `taira_dataspace_deploy_finality_tests.rs`); their fingerprint, endpoint,
//! journal and deadline cases have no counterpart here (spec D-7).

use std::{cell::RefCell, collections::BTreeMap, ops::Range};

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::consensus_v2::{
        self as wire, BlockSubject, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, HeightContext, PayloadEncoding, QuorumCertificate,
        SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus,
        SumeragiV2LivenessStatus, SumeragiV2Status, SumeragiV2StatusPhase, Vote,
    },
    bridge::{
        BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2,
        BridgeFinalityAttestationBodyV1,
    },
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1, KagemushaMintFinalityValidatorKeysV1,
    },
    nexus::ValidatorCommitteePreparationV1,
};
use norito::codec::Encode as _;

use super::*;

/// Committee sizes exercised by the generalized cases.
const SIZES: [usize; 3] = [4, 7, 10];
const CHALLENGE: [u8; 32] = [0xC1; 32];
const GENESIS_MS: u64 = 1_700_000_000_000;

fn key(index: usize) -> KeyPair {
    let seed = Hash::new(format!("finality fixture validator {index}"));
    KeyPair::try_from_seed(seed.as_ref().to_vec(), Algorithm::BlsNormal).unwrap()
}

fn peer(key: &KeyPair) -> PeerId {
    PeerId::new(key.public_key().clone())
}

fn nz(height: u64) -> NonZeroU64 {
    NonZeroU64::new(height).unwrap()
}

fn signers(range: Range<usize>) -> Vec<u32> {
    range.map(|index| u32::try_from(index).unwrap()).collect()
}

/// One epoch: its members' keys in roster order and its frozen inputs.
struct Epoch {
    keys: Vec<KeyPair>,
    committee: FinalizedNextEpochSnapshot,
}

/// A chain from genesis to a tip, each block finalized by the first `2f + 1`
/// members of its epoch.
struct Chain {
    network: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    epochs: Vec<Epoch>,
    proofs: Vec<BridgeFinalityProof>,
}

impl Chain {
    /// `epochs[i]` is `(validator key indices, last height)`. Blocks run from
    /// 1 to `tip`, which must be before the last epoch ends. Consecutive
    /// epochs with the same roster retain the mint authority; others activate
    /// the next generation.
    fn new(epochs: &[(Range<usize>, u64)], tip: u64) -> Self {
        Self::with_genesis_time(epochs, tip, GENESIS_MS)
    }

    fn with_genesis_time(epochs: &[(Range<usize>, u64)], tip: u64, genesis_ms: u64) -> Self {
        let genesis = BlockHeader::new(nz(1), None, None, genesis_ms, 0);
        let genesis_hash = genesis.hash();
        let network = NetworkId::from_genesis_hash(genesis_hash);
        let mut built: Vec<Epoch> = Vec::new();
        for (index, (members, end)) in epochs.iter().enumerate() {
            let mut keys = members
                .clone()
                .map(|member| (member, key(member)))
                .collect::<Vec<_>>();
            keys.sort_by_key(|(_, key)| peer(key));
            let roster = keys
                .iter()
                .map(|(_, key)| ValidatorPower {
                    validator: peer(key),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let previous = built.last().map(|epoch| &epoch.committee);
            let retained = previous.is_some_and(|previous| previous.roster == roster);
            let generation = previous.map_or(0, |previous| {
                previous.kagemusha_mint_finality_authority.generation + u64::from(!retained)
            });
            let authority = KagemushaMintFinalityAuthorityGenerationV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                network_id: network,
                generation,
                validators: keys
                    .iter()
                    .map(|(member, key)| KagemushaMintFinalityValidatorKeysV1 {
                        validator: peer(key),
                        eq_proof_public_key: [u8::try_from(member + 1).unwrap(); 32],
                        ep_proof_public_key: [u8::try_from(member + 129).unwrap(); 32],
                    })
                    .collect(),
            };
            let authorization = previous.map_or_else(
                || KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, *end).unwrap(),
                |previous| {
                    let previous = previous.kagemusha_mint_finality_authorization;
                    KagemushaMintFinalityEpochAuthorizationV1 {
                        epoch: previous.epoch + 1,
                        first_height: previous.last_height + 1,
                        last_height: *end,
                        authority_generation: generation,
                        authority_id: authority.authority_id().unwrap(),
                        beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                            session_id: [7; 32],
                            transcript_hash: [8; 32],
                        }),
                        previous_authorization_id: previous.authorization_id().unwrap(),
                        transition_id: if retained { [0; 32] } else { [9; 32] },
                        decision: if retained {
                            KagemushaMintFinalityEpochDecisionV1::Retain
                        } else {
                            KagemushaMintFinalityEpochDecisionV1::Activate
                        },
                        ..previous
                    }
                },
            );
            let committee = FinalizedNextEpochSnapshot {
                committee_preparation: None,
                epoch: u64::try_from(index).unwrap(),
                kagemusha_mint_finality_authorization: authorization,
                kagemusha_mint_finality_authority: authority,
                epoch_end_height: *end,
                mode: ConsensusMode::Npos,
                quorum: DualQuorum::from_roster(&roster).unwrap(),
                validator_set_pops: keys
                    .iter()
                    .map(|(_, key)| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
                    .collect(),
                roster,
                leader_seed: [u8::try_from(index + 0x50).unwrap(); 32],
            };
            built.push(Epoch {
                keys: keys.into_iter().map(|(_, key)| key).collect(),
                committee,
            });
        }
        assert!(tip < built.last().unwrap().committee.epoch_end_height);
        let mut chain = Self {
            network,
            genesis_hash,
            epochs: built,
            proofs: Vec::new(),
        };
        for height in 1..=tip {
            let header = if height == 1 {
                genesis
            } else {
                chain.successor_header(height, genesis_ms + height)
            };
            let proof = chain.proof(&header, b"payload");
            chain.proofs.push(proof);
        }
        chain
    }

    fn successor_header(&self, height: u64, creation_ms: u64) -> BlockHeader {
        let parent = self.proof_at(height - 1).block_header.hash();
        BlockHeader::new(nz(height), Some(parent), None, creation_ms, 0)
    }

    fn epoch_at(&self, height: u64) -> usize {
        self.epochs
            .iter()
            .position(|epoch| height <= epoch.committee.epoch_end_height)
            .unwrap()
    }

    fn proof_at(&self, height: u64) -> &BridgeFinalityProof {
        &self.proofs[usize::try_from(height - 1).unwrap()]
    }

    /// The finality proof for `header`, signed by the first `2f + 1` members.
    fn proof(&self, header: &BlockHeader, payload: &[u8]) -> BridgeFinalityProof {
        let height = header.height().get();
        let index = self.epoch_at(height);
        let epoch = &self.epochs[index];
        let committee = &epoch.committee;
        let context = HeightContext {
            network_id: self.network,
            protocol_version: wire::PROTOCOL_VERSION,
            height,
            epoch: committee.epoch,
            kagemusha_mint_finality_authorization: committee.kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority: committee.kagemusha_mint_finality_authority.clone(),
            epoch_end_height: committee.epoch_end_height,
            next_epoch_snapshot: (height == committee.epoch_end_height)
                .then(|| self.epochs[index + 1].committee.clone()),
            mode: committee.mode,
            parent_commit_qc: (height > 1).then(|| {
                self.proof_at(height - 1)
                    .finality_artifact
                    .commit_qc
                    .clone()
            }),
            snapshot_bootstrap: None,
            roster: committee.roster.clone(),
            quorum: committee.quorum,
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
            leader_seed: committee.leader_seed,
        };
        let subject = BlockSubject {
            parent_block_hash: header.prev_block_hash(),
            block_hash: header.hash(),
            payload_hash: Hash::new(payload),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: 0,
        };
        let commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"parent state"),
                Hash::new(b"post state"),
                Hash::new(b"writes"),
                1,
                Hash::new(b"executed wire"),
            ),
            signers: Vec::new(),
            aggregate_signature: Vec::new(),
        };
        let mut proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: *header,
            finality_artifact: V2FinalityArtifact::new(
                context,
                subject,
                commit_qc,
                committee.validator_set_pops.clone(),
            ),
        };
        let quorum = CommitteeSize::new(epoch.keys.len()).unwrap().quorum();
        sign(&mut proof, &epoch.keys, &signers(0..quorum), 0);
        proof
    }

    /// The same decision at `height` certified by another valid witness: a
    /// different view and signer subset (rotated by `shift`).
    fn witness(&self, height: u64, shift: usize) -> BridgeFinalityProof {
        let epoch = &self.epochs[self.epoch_at(height)];
        let members = epoch.keys.len();
        let quorum = CommitteeSize::new(members).unwrap().quorum();
        let mut chosen = (0..quorum)
            .map(|offset| u32::try_from((offset + shift) % members).unwrap())
            .collect::<Vec<_>>();
        chosen.sort_unstable();
        let mut proof = self.proof_at(height).clone();
        sign(
            &mut proof,
            &epoch.keys,
            &chosen,
            2 + u64::try_from(shift).unwrap(),
        );
        proof
    }

    fn keys_of(&self, epoch: usize) -> &[KeyPair] {
        &self.epochs[epoch].keys
    }

    fn members(&self, epoch: usize) -> Vec<PeerId> {
        self.keys_of(epoch).iter().map(peer).collect()
    }

    fn key_of(&self, node: &PeerId) -> &KeyPair {
        self.epochs
            .iter()
            .flat_map(|epoch| &epoch.keys)
            .find(|key| peer(key) == *node)
            .unwrap()
    }

    /// `node`'s attestation of the block at `height`.
    fn attest(
        &self,
        node: &PeerId,
        height: u64,
        challenge: [u8; 32],
    ) -> BridgeFinalityAttestationV1 {
        attestation(
            self.key_of(node),
            self.proofs[0].clone(),
            self.proof_at(height).clone(),
            challenge,
        )
    }

    fn anchor(&self) -> GenesisAnchor {
        let committee = &self.epochs[0].committee;
        GenesisAnchor {
            network_id: self.network,
            genesis_block_hash: self.genesis_hash,
            mode: ConsensusMode::Npos,
            validators: committee
                .roster
                .iter()
                .zip(&committee.validator_set_pops)
                .map(|(member, pop)| (member.validator.public_key().clone(), pop.clone()))
                .collect(),
        }
    }

    fn verifier(&self) -> FinalityVerifier {
        FinalityVerifier::from_genesis(&self.anchor(), &self.proofs[0]).unwrap()
    }

    /// A verifier already advanced to `height`.
    fn verifier_at(&self, height: u64) -> FinalityVerifier {
        let mut verifier = self.verifier();
        verifier
            .advance(&Source::new(self), self.proof_at(height))
            .unwrap();
        verifier
    }
}

/// Sign `proof`'s `CommitQC` in `view` with the roster indices `signers`,
/// taking each signer's key from `keys` at the same index.
fn sign(proof: &mut BridgeFinalityProof, keys: &[KeyPair], signers: &[u32], view: u64) {
    let artifact = &mut proof.finality_artifact;
    let context_id = artifact.height_context.id();
    let certificate = &mut artifact.commit_qc;
    certificate.round.context_id = context_id;
    certificate.round.view = view;
    certificate.proposal_round = certificate.round;
    certificate.signers = signers.to_vec();
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
    let shares = signers
        .iter()
        .map(|&index| {
            let key = &keys[usize::try_from(index).unwrap()];
            Signature::try_new(key.private_key(), &preimage)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
}

/// A signed attestation by `key` of `tip`, with the node status the
/// data model requires for it.
fn attestation(
    key: &KeyPair,
    genesis: BridgeFinalityProof,
    tip: BridgeFinalityProof,
    challenge: [u8; 32],
) -> BridgeFinalityAttestationV1 {
    let artifact = &tip.finality_artifact;
    let context = &artifact.height_context;
    let node_id = peer(key);
    let node_fingerprint = Hash::new(node_id.encode());
    let members = u32::try_from(context.roster.len()).unwrap();
    let status = SumeragiV2Status {
        protocol_version: wire::PROTOCOL_VERSION,
        node_fingerprint,
        build_fingerprint: Hash::new(b"fixture build"),
        config_fingerprint: Hash::new(b"fixture config"),
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
            validator_count: members,
            quorum: context.quorum,
        },
        last_commit_qc: Some(SumeragiV2CommitQcStatus {
            certificate: artifact.commit_qc.as_ref(),
            validator_count: members,
            signer_count: context.quorum.min_signers,
            min_signers: context.quorum.min_signers,
            signed_power: u64::from(context.quorum.min_signers),
            total_power: u64::from(members),
        }),
        liveness: SumeragiV2LivenessStatus::default(),
        beacon_horizon: None,
    };
    let body = BridgeFinalityAttestationBodyV1 {
        version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
        challenge,
        network_id: context.network_id,
        node_fingerprint,
        node_id,
        genesis_block_hash: genesis.block_header.hash(),
        genesis_finality_proof: genesis,
        status,
        finality_proof: tip,
    };
    let signature = SignatureOf::try_from_hash(key.private_key(), body.signing_hash()).unwrap();
    BridgeFinalityAttestationV1 { body, signature }
}

fn resign(
    mut attestation: BridgeFinalityAttestationV1,
    key: &KeyPair,
) -> BridgeFinalityAttestationV1 {
    attestation.signature =
        SignatureOf::try_from_hash(key.private_key(), attestation.body.signing_hash()).unwrap();
    attestation
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct Unavailable(String);

/// In-memory source over a chain. `tips[node]` is the height each node
/// attests to; nodes without a tip are unreachable. `proofs` and
/// `attestations` replace what the chain would serve.
struct Source<'a> {
    chain: &'a Chain,
    tips: BTreeMap<PeerId, u64>,
    proofs: BTreeMap<u64, BridgeFinalityProof>,
    attestations: BTreeMap<PeerId, BridgeFinalityAttestationV1>,
    proof_calls: RefCell<Vec<u64>>,
    attestation_calls: RefCell<Vec<PeerId>>,
}

impl<'a> Source<'a> {
    fn new(chain: &'a Chain) -> Self {
        Self {
            chain,
            tips: BTreeMap::new(),
            proofs: BTreeMap::new(),
            attestations: BTreeMap::new(),
            proof_calls: RefCell::new(Vec::new()),
            attestation_calls: RefCell::new(Vec::new()),
        }
    }

    fn with_tips(mut self, nodes: &[PeerId], height: u64) -> Self {
        self.tips
            .extend(nodes.iter().map(|node| (node.clone(), height)));
        self
    }

    fn proof_calls(&self) -> Vec<u64> {
        self.proof_calls.borrow().clone()
    }
}

impl FinalitySource for Source<'_> {
    type Error = Unavailable;

    fn finality_proof(&self, height: NonZeroU64) -> Result<BridgeFinalityProof, Unavailable> {
        self.proof_calls.borrow_mut().push(height.get());
        self.proofs
            .get(&height.get())
            .or_else(|| {
                self.chain
                    .proofs
                    .get(usize::try_from(height.get() - 1).ok()?)
            })
            .cloned()
            .ok_or_else(|| Unavailable(format!("no proof at height {height}")))
    }

    fn latest_attestation(
        &self,
        node: &PeerId,
        challenge: &[u8; 32],
    ) -> Result<BridgeFinalityAttestationV1, Unavailable> {
        self.attestation_calls.borrow_mut().push(node.clone());
        if let Some(attestation) = self.attestations.get(node) {
            return Ok(attestation.clone());
        }
        let height = self
            .tips
            .get(node)
            .ok_or_else(|| Unavailable(format!("{node} is unreachable")))?;
        Ok(self.chain.attest(node, *height, *challenge))
    }
}

fn verified(quorum: &AttestationQuorum) -> Vec<u64> {
    quorum
        .peers
        .iter()
        .filter_map(|(_, outcome)| match outcome {
            AttestationOutcome::Verified(tip) => Some(tip.height.get()),
            _ => None,
        })
        .collect()
}

fn insufficient(result: Result<AttestationQuorum, FinalityError>) -> AttestationQuorum {
    match result {
        Err(FinalityError::InsufficientAttestations(report)) => *report,
        other => panic!("expected too few attestations, got {other:?}"),
    }
}

// --- Committee geometry ---------------------------------------------------

#[test]
fn committee_size_accepts_exact_3f_plus_1_from_4_to_128() {
    for members in 0..=200 {
        let expected = (4..=128).contains(&members) && members % 3 == 1;
        assert_eq!(CommitteeSize::new(members).is_ok(), expected, "{members}");
    }
    for (members, faults, quorum) in [
        (4, 1, 3),
        (7, 2, 5),
        (10, 3, 7),
        (31, 10, 21),
        (127, 42, 85),
    ] {
        let size = CommitteeSize::new(members).unwrap();
        assert_eq!(
            (size.members(), size.faults(), size.quorum()),
            (members, faults, quorum)
        );
        assert_eq!(
            DualQuorum::count_threshold(u32::try_from(members).unwrap()),
            Some(u32::try_from(quorum).unwrap()),
            "the data-model threshold agrees"
        );
    }
    assert!(matches!(
        CommitteeSize::new(3),
        Err(FinalityError::CommitteeSize { members: 3 })
    ));
}

// --- Genesis anchor --------------------------------------------------------

#[test]
fn genesis_anchor_pins_sorted_roster_network_and_genesis() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 1);
        let verifier = chain.verifier();
        let checkpoint = verifier.checkpoint();
        assert_eq!(checkpoint.height.get(), 1);
        assert_eq!(checkpoint.block_hash, chain.genesis_hash);
        assert_eq!(
            checkpoint.network_id,
            NetworkId::from_genesis_hash(chain.genesis_hash)
        );
        assert_eq!(checkpoint.decision, checkpoint.genesis_decision);
        assert_eq!(checkpoint.committee, chain.epochs[0].committee);
        assert_eq!(checkpoint.next_committee, None);
        assert_eq!(checkpoint.previous_epoch, None);
        assert_eq!(verifier.committee_size().members(), members);
        let roster = &checkpoint.committee.roster;
        assert!(
            roster
                .windows(2)
                .all(|pair| pair[0].validator < pair[1].validator)
        );
        for (member, pop) in roster.iter().zip(&checkpoint.committee.validator_set_pops) {
            assert_eq!(member.power, 1);
            iroha_crypto::bls_normal_pop_verify(member.validator.public_key(), pop).unwrap();
        }
    }
}

#[test]
fn genesis_anchor_rejects_other_networks_rosters_and_modes() {
    let chain = Chain::new(&[(0..4, 20)], 2);
    let genesis = &chain.proofs[0];
    let reject = |anchor: &GenesisAnchor, proof: &BridgeFinalityProof| {
        FinalityVerifier::from_genesis(anchor, proof).unwrap_err()
    };

    let mut anchor = chain.anchor();
    anchor.network_id = Chain::with_genesis_time(&[(0..4, 20)], 1, 7).network;
    assert!(matches!(
        reject(&anchor, genesis),
        FinalityError::WrongNetwork { .. }
    ));

    let other = Chain::with_genesis_time(&[(0..4, 20)], 1, 7);
    assert!(matches!(
        reject(&other.anchor(), genesis),
        FinalityError::WrongGenesis
    ));
    assert!(matches!(
        reject(&chain.anchor(), chain.proof_at(2)),
        FinalityError::UnexpectedHeight {
            expected: 1,
            actual: 2
        }
    ));

    for (change, members) in [(-1, 3), (1, 5)] {
        let mut anchor = chain.anchor();
        if change < 0 {
            let first = anchor.validators.keys().next().unwrap().clone();
            anchor.validators.remove(&first);
        } else {
            let extra = key(90);
            anchor.validators.insert(
                extra.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(extra.private_key()).unwrap(),
            );
        }
        assert!(matches!(
            reject(&anchor, genesis),
            FinalityError::CommitteeSize { members: found } if found == members
        ));
    }

    // A self-consistent foreign validator cannot join the pinned roster.
    let mut anchor = chain.anchor();
    let first = anchor.validators.keys().next().unwrap().clone();
    anchor.validators.remove(&first);
    let foreign = key(91);
    anchor.validators.insert(
        foreign.public_key().clone(),
        iroha_crypto::bls_normal_pop_prove(foreign.private_key()).unwrap(),
    );
    assert!(matches!(
        reject(&anchor, genesis),
        FinalityError::CommitteeMismatch { height: 1 }
    ));

    let mut anchor = chain.anchor();
    let mut pops = anchor.validators.values().cloned().collect::<Vec<_>>();
    pops.swap(0, 1);
    for (pop, slot) in pops.into_iter().zip(anchor.validators.values_mut()) {
        *slot = pop;
    }
    assert!(matches!(
        reject(&anchor, genesis),
        FinalityError::CommitteeMismatch { height: 1 }
    ));

    let mut anchor = chain.anchor();
    anchor.mode = ConsensusMode::Permissioned;
    assert!(matches!(
        reject(&anchor, genesis),
        FinalityError::CommitteeMismatch { height: 1 }
    ));

    let mut short = genesis.clone();
    sign(&mut short, chain.keys_of(0), &signers(0..2), 0);
    assert!(matches!(
        reject(&chain.anchor(), &short),
        FinalityError::SignerCount {
            height: 1,
            expected: 3,
            actual: 2
        }
    ));
}

// --- Advancing -------------------------------------------------------------

#[test]
fn advance_within_an_epoch_fetches_nothing() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 5);
        let source = Source::new(&chain);
        let mut verifier = chain.verifier();
        assert_eq!(verifier.advance(&source, chain.proof_at(5)).unwrap(), 0);
        assert!(source.proof_calls().is_empty());
        let checkpoint = verifier.checkpoint();
        assert_eq!(checkpoint.height.get(), 5);
        assert_eq!(checkpoint.block_hash, chain.proof_at(5).block_header.hash());
        assert_eq!(
            checkpoint.decision,
            chain.proof_at(5).finality_artifact.commit_qc.as_ref()
        );
        assert_eq!(checkpoint.committee, chain.epochs[0].committee);
    }
}

#[test]
fn advance_is_idempotent_and_never_moves_back() {
    let chain = Chain::new(&[(0..4, 20)], 3);
    let source = Source::new(&chain);
    let mut verifier = chain.verifier_at(3);
    let at_three = verifier.clone();

    // The same decision under another certificate witness is accepted.
    for shift in 0..4 {
        assert_eq!(
            verifier.advance(&source, &chain.witness(3, shift)).unwrap(),
            0
        );
        assert_eq!(verifier, at_three);
    }
    assert!(matches!(
        verifier.advance(&source, chain.proof_at(2)),
        Err(FinalityError::StaleTip {
            checkpoint: 3,
            height: 2
        })
    ));
    // A validly signed different block at the checkpoint height conflicts.
    let conflicting = chain.proof(&chain.successor_header(3, GENESIS_MS + 99), b"other");
    assert!(matches!(
        verifier.advance(&source, &conflicting),
        Err(FinalityError::ConflictingDecision { height: 3 })
    ));
    assert_eq!(verifier, at_three);
    assert!(source.proof_calls().is_empty());
}

#[test]
fn advance_fetches_only_epoch_terminal_proofs_across_roster_changes() {
    // 4 -> 7 (overlapping) -> 10 -> 10 retained.
    let epochs = [(0..4, 3), (2..9, 6), (0..10, 9), (0..10, 30)];
    let chain = Chain::new(&epochs, 12);
    let source = Source::new(&chain);
    let mut verifier = chain.verifier();
    assert_eq!(verifier.advance(&source, chain.proof_at(12)).unwrap(), 3);
    assert_eq!(source.proof_calls(), vec![3, 6, 9]);
    let checkpoint = verifier.checkpoint();
    assert_eq!(checkpoint.height.get(), 12);
    assert_eq!(checkpoint.committee, chain.epochs[3].committee);
    assert_eq!(verifier.committee_size().members(), 10);
    // Only the epoch just crossed is kept, with its verified terminal decision.
    let previous = checkpoint.previous_epoch.as_ref().unwrap();
    assert_eq!(previous.committee, chain.epochs[2].committee);
    assert_eq!(
        previous.terminal_decision,
        chain.proof_at(9).finality_artifact.commit_qc.as_ref()
    );
    assert_eq!(
        checkpoint
            .committee
            .kagemusha_mint_finality_authorization
            .decision,
        KagemushaMintFinalityEpochDecisionV1::Retain
    );

    // Stopping on an epoch-terminal block pins the next committee too.
    let source = Source::new(&chain);
    let mut verifier = chain.verifier();
    assert_eq!(verifier.advance(&source, chain.proof_at(6)).unwrap(), 1);
    assert_eq!(source.proof_calls(), vec![3]);
    let checkpoint = verifier.checkpoint();
    assert_eq!(checkpoint.committee, chain.epochs[1].committee);
    assert_eq!(
        checkpoint.next_committee.as_ref(),
        Some(&chain.epochs[2].committee)
    );
    assert_eq!(checkpoint.governing_committee(), &chain.epochs[2].committee);
    assert_eq!(
        checkpoint
            .previous_epoch
            .as_ref()
            .map(|previous| &previous.committee),
        Some(&chain.epochs[0].committee)
    );

    // Resuming from the stored checkpoint costs only the new epochs.
    let bytes = checkpoint.to_bytes().unwrap();
    let mut resumed = FinalityVerifier::from_checkpoint(
        FinalityCheckpointV1::from_bytes(&bytes).unwrap(),
        chain.network,
    )
    .unwrap();
    let source = Source::new(&chain);
    assert_eq!(resumed.advance(&source, chain.proof_at(12)).unwrap(), 1);
    assert_eq!(source.proof_calls(), vec![9]);
    assert_eq!(resumed.checkpoint().height.get(), 12);
}

#[test]
fn advance_rejects_tips_and_transitions_not_signed_by_the_pinned_committee() {
    let chain = Chain::new(&[(0..4, 3), (4..11, 20)], 5);
    // Same genesis committee, but it hands over to another roster.
    let fork = Chain::new(&[(0..4, 3), (20..27, 20)], 5);
    let start = chain.verifier();
    let rejected = |source: &Source<'_>, tip: &BridgeFinalityProof| {
        let mut verifier = start.clone();
        let error = verifier.advance(source, tip).unwrap_err();
        assert_eq!(verifier, start, "a failed advance keeps the checkpoint");
        error
    };

    // The fork's tip is signed by a committee this chain never elected.
    assert!(matches!(
        rejected(&Source::new(&chain), fork.proof_at(5)),
        FinalityError::CommitteeMismatch { height: 5 }
    ));

    // A forged hand-over re-signed by the new roster, not by the incumbents.
    let mut forged = chain.proof_at(3).clone();
    forged.finality_artifact.height_context.next_epoch_snapshot =
        Some(fork.epochs[1].committee.clone());
    let quorum = signers(0..3);
    sign(&mut forged, fork.keys_of(1), &quorum, 0);
    let mut source = Source::new(&chain);
    source.proofs.insert(3, forged);
    assert!(matches!(
        rejected(&source, chain.proof_at(5)),
        FinalityError::Proof { height: 3, .. }
    ));

    // The same hand-over without a new certificate breaks its context binding.
    let mut unsigned = chain.proof_at(3).clone();
    unsigned
        .finality_artifact
        .height_context
        .next_epoch_snapshot = Some(fork.epochs[1].committee.clone());
    let mut source = Source::new(&chain);
    source.proofs.insert(3, unsigned);
    assert!(matches!(
        rejected(&source, chain.proof_at(5)),
        FinalityError::Proof { height: 3, .. }
    ));

    // A tip whose context claims the old roster in the new epoch.
    let mut stale_roster = chain.proof_at(5).clone();
    let context = &mut stale_roster.finality_artifact.height_context;
    context.roster = chain.epochs[0].committee.roster.clone();
    context.quorum = chain.epochs[0].committee.quorum;
    stale_roster.finality_artifact.validator_set_pops =
        chain.epochs[0].committee.validator_set_pops.clone();
    sign(&mut stale_roster, chain.keys_of(0), &quorum, 0);
    assert!(matches!(
        rejected(&Source::new(&chain), &stale_roster),
        FinalityError::Proof { height: 5, .. } | FinalityError::CommitteeMismatch { height: 5 }
    ));
}

#[test]
fn advance_rejects_truncated_or_reordered_chains() {
    let chain = Chain::new(&[(0..4, 3), (4..11, 20)], 5);
    let start = chain.verifier();

    // The source stops before the epoch-terminal proof.
    let truncated = Chain::new(&[(0..4, 3), (4..11, 20)], 2);
    let mut verifier = start.clone();
    assert!(matches!(
        verifier.advance(&Source::new(&truncated), chain.proof_at(5)),
        Err(FinalityError::Source(_))
    ));
    assert_eq!(verifier, start);

    // The source answers with another height.
    let mut source = Source::new(&chain);
    source.proofs.insert(3, chain.proof_at(2).clone());
    let mut verifier = start.clone();
    assert!(matches!(
        verifier.advance(&source, chain.proof_at(5)),
        Err(FinalityError::UnexpectedHeight {
            expected: 3,
            actual: 2
        })
    ));
    assert_eq!(verifier, start);
}

#[test]
fn certificates_need_exactly_2f_plus_1_distinct_roster_signers() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 3);
        let keys = chain.keys_of(0);
        let quorum = CommitteeSize::new(members).unwrap().quorum();
        let start = chain.verifier();
        let source = Source::new(&chain);
        let check = |certificate: &[u32], signing_keys: &[KeyPair]| {
            let mut tip = chain.proof_at(3).clone();
            sign(&mut tip, signing_keys, certificate, 0);
            start.clone().advance(&source, &tip).map(|_| ())
        };

        check(&signers(0..quorum), keys).unwrap();
        check(&signers(members - quorum..members), keys).unwrap();
        assert!(matches!(
            check(&signers(0..quorum - 1), keys),
            Err(FinalityError::SignerCount { height: 3, expected, actual })
                if expected == quorum && actual == quorum - 1
        ));
        assert!(matches!(
            check(&signers(0..quorum + 1), keys),
            Err(FinalityError::SignerCount { height: 3, expected, actual })
                if expected == quorum && actual == quorum + 1
        ));
        let mut repeated = signers(0..quorum);
        repeated[1] = repeated[0];
        assert!(matches!(
            check(&repeated, keys),
            Err(FinalityError::NonCanonicalSigners { height: 3 })
        ));
        let mut unordered = signers(0..quorum);
        unordered.swap(0, 1);
        assert!(matches!(
            check(&unordered, keys),
            Err(FinalityError::NonCanonicalSigners { height: 3 })
        ));

        // An index past the roster, signed as if it were a member.
        let mut outside = chain.proof_at(3).clone();
        let mut extended = keys.to_vec();
        extended.push(key(95));
        let mut claimed = signers(0..quorum - 1);
        claimed.push(u32::try_from(members).unwrap());
        sign(&mut outside, &extended, &claimed, 0);
        assert!(matches!(
            start.clone().advance(&source, &outside),
            Err(FinalityError::SignerOutsideRoster { height: 3, signer, members: found })
                if signer == claimed[quorum - 1] && found == members
        ));

        // Claimed signers differ from the keys that actually signed.
        let mut shifted = keys.to_vec();
        shifted.rotate_left(1);
        assert!(matches!(
            check(&signers(0..quorum), &shifted),
            Err(FinalityError::Proof { height: 3, .. })
        ));
        // A foreign key in place of a member.
        let mut foreign = keys.to_vec();
        foreign[0] = key(96);
        assert!(matches!(
            check(&signers(0..quorum), &foreign),
            Err(FinalityError::Proof { height: 3, .. })
        ));
    }
}

#[test]
fn advance_rejects_mutated_proofs_and_keeps_the_checkpoint() {
    let chain = Chain::new(&[(0..4, 20)], 3);
    let source = Source::new(&chain);
    let start = chain.verifier();
    let quorum = signers(0..3);
    let other_network = Chain::with_genesis_time(&[(0..4, 20)], 1, 7).network;
    for mutation in 0..6 {
        let mut proof = chain.proof_at(2).clone();
        match mutation {
            0 => proof.finality_artifact.commit_qc.aggregate_signature[0] ^= 1,
            1 => proof.finality_artifact.validator_set_pops[0][0] ^= 1,
            2 => {
                proof.finality_artifact.height_context.network_id = other_network;
                sign(&mut proof, chain.keys_of(0), &quorum, 0);
            }
            3 => proof.version ^= 1,
            4 => {
                let header = BlockHeader::new(
                    nz(2),
                    Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign parent"))),
                    None,
                    GENESIS_MS + 2,
                    0,
                );
                proof = chain.proof(&header, b"payload");
            }
            _ => {
                proof.finality_artifact.height_context.leader_seed = [0xEE; 32];
                sign(&mut proof, chain.keys_of(0), &quorum, 0);
            }
        }
        let mut verifier = start.clone();
        assert!(
            verifier.advance(&source, &proof).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(verifier, start, "mutation {mutation}");
    }
    let mut verifier = start.clone();
    verifier.advance(&source, chain.proof_at(2)).unwrap();
    assert_eq!(verifier.checkpoint().height.get(), 2);
}

// --- Checkpoint ------------------------------------------------------------

#[test]
fn checkpoint_roundtrips_through_norito() {
    let chain = Chain::new(&[(0..4, 3), (2..9, 20)], 5);
    for height in [1, 3, 5] {
        let checkpoint = chain.verifier_at(height).checkpoint().clone();
        let bytes = checkpoint.to_bytes().unwrap();
        assert_eq!(
            FinalityCheckpointV1::from_bytes(&bytes).unwrap(),
            checkpoint
        );
        let resumed = FinalityVerifier::from_checkpoint(checkpoint.clone(), chain.network).unwrap();
        assert_eq!(resumed.checkpoint(), &checkpoint);

        let other = Chain::with_genesis_time(&[(0..4, 20)], 1, 7).network;
        assert!(matches!(
            FinalityVerifier::from_checkpoint(checkpoint.clone(), other),
            Err(FinalityError::WrongNetwork { .. })
        ));
        assert!(matches!(
            FinalityCheckpointV1::from_bytes(&bytes[..bytes.len() - 1]),
            Err(FinalityError::Codec(_))
        ));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            FinalityCheckpointV1::from_bytes(&trailing),
            Err(FinalityError::Codec(_))
        ));
    }
}

/// A copy of `base` changed by `change`.
fn mutated(
    base: &FinalityCheckpointV1,
    change: impl FnOnce(&mut FinalityCheckpointV1),
) -> FinalityCheckpointV1 {
    let mut changed = base.clone();
    change(&mut changed);
    changed
}

/// Assert that every checkpoint in `broken` fails validation on decode.
#[track_caller]
fn assert_invalid_checkpoints(broken: Vec<FinalityCheckpointV1>) {
    for (index, checkpoint) in broken.into_iter().enumerate() {
        assert!(
            matches!(
                FinalityCheckpointV1::from_bytes(&checkpoint.to_bytes().unwrap()),
                Err(FinalityError::InvalidCheckpoint(_))
            ),
            "case {index}"
        );
    }
}

#[test]
fn checkpoint_validation_rejects_each_inconsistency() {
    let chain = Chain::new(&[(0..4, 3), (2..9, 20)], 5);
    let genesis = chain.verifier_at(1).checkpoint().clone();
    let terminal = chain.verifier_at(3).checkpoint().clone();
    let middle = chain.verifier_at(5).checkpoint().clone();
    let other_network = Chain::with_genesis_time(&[(0..4, 20)], 1, 7).network;
    let other_decision = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"other parent state"),
        Hash::new(b"other post state"),
        Hash::new(b"other writes"),
        1,
        Hash::new(b"other executed wire"),
    );
    assert_invalid_checkpoints(vec![
        mutated(&terminal, |c| c.next_committee = None),
        mutated(&middle, |c| {
            c.next_committee = Some(chain.epochs[1].committee.clone());
        }),
        mutated(&middle, |c| c.height = nz(21)),
        mutated(&middle, |c| {
            c.block_hash = chain.proof_at(4).block_header.hash();
        }),
        mutated(&middle, |c| c.network_id = other_network),
        mutated(&middle, |c| c.genesis_decision = c.decision),
        mutated(&middle, |c| {
            c.committee.validator_set_pops.pop();
        }),
        // The next committee follows the terminal epoch in number, height
        // and mode, and is canonical.
        mutated(&terminal, |c| c.next_committee.as_mut().unwrap().epoch += 1),
        mutated(&terminal, |c| {
            c.next_committee.as_mut().unwrap().epoch_end_height = 3;
        }),
        mutated(&terminal, |c| {
            c.next_committee.as_mut().unwrap().mode = ConsensusMode::Permissioned;
        }),
        mutated(&terminal, |c| {
            c.next_committee.as_mut().unwrap().roster.swap(0, 1);
        }),
        // A height-one checkpoint is the genesis decision itself.
        mutated(&genesis, |c| {
            c.decision.execution_commitment = other_decision
        }),
        // Every committee is a canonical roster: sorted, one vote each, with
        // the canonical quorum.
        mutated(&middle, |c| c.committee.roster.swap(0, 1)),
        mutated(&middle, |c| c.committee.roster[0].power = 2),
        mutated(&middle, |c| c.committee.quorum.min_signers += 1),
        mutated(&middle, |c| c.committee.quorum.total_power += 1),
        // A committee holds only its own epoch's inputs.
        mutated(&middle, |c| {
            c.committee.committee_preparation = Some(preparation(&chain, 1));
        }),
        mutated(&terminal, |c| {
            c.next_committee.as_mut().unwrap().committee_preparation = Some(preparation(&chain, 3));
        }),
    ]);
    let small = mutated(&middle, |c| {
        c.committee.roster.pop();
        c.committee.validator_set_pops.pop();
    });
    assert!(matches!(
        small.validate(),
        Err(FinalityError::CommitteeSize { members: 6 })
    ));
}

/// A shape-only committee preparation selected at `selection_height`.
fn preparation(chain: &Chain, selection_height: u64) -> ValidatorCommitteePreparationV1 {
    let committee = &chain.epochs[1].committee;
    ValidatorCommitteePreparationV1 {
        version: 1,
        network_id: chain.network,
        selection_epoch: 0,
        selection_height,
        selection_anchor: chain.proof_at(1).block_header.hash(),
        target_epoch: 2,
        first_height: committee.epoch_end_height + 1,
        last_height: committee.epoch_end_height + 20,
        authority_generation: 1,
        preparing_authorization_id: [0x5A; 32],
        election_seed: [0x5B; 32],
        eligibility: iroha_data_model::nexus::ValidatorElectionPolicyV1 {
            epoch_length_blocks: 20,
            ..iroha_data_model::nexus::ValidatorElectionPolicyV1::from_npos_parameters(
                &iroha_data_model::parameter::system::SumeragiNposParameters::default(),
            )
            .unwrap()
        },
        committee: committee
            .roster
            .iter()
            .zip(&committee.validator_set_pops)
            .map(
                |(seat, pop)| iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1 {
                    validator: seat.validator.clone(),
                    proof_of_possession: pop.clone(),
                },
            )
            .collect(),
    }
}

#[test]
fn epoch_inputs_drop_the_later_epochs_preparation() {
    let chain = Chain::new(&[(0..4, 3), (2..9, 20)], 5);
    let next = chain.epochs[1].committee.clone();
    let prepared = FinalizedNextEpochSnapshot {
        committee_preparation: Some(preparation(&chain, 3)),
        ..next.clone()
    };
    assert_eq!(epoch_inputs(&prepared), next);
    // The governed epoch's own height contexts project to the same inputs.
    assert_eq!(committee_of(&chain.proof_at(4).finality_artifact), next);
    assert!(validate_committee(&epoch_inputs(&prepared)).is_ok());
    assert!(matches!(
        validate_committee(&prepared),
        Err(FinalityError::InvalidCheckpoint(_))
    ));
}

#[test]
fn checkpoint_previous_epoch_precedes_it_and_ends_at_its_decision() {
    let chain = Chain::new(&[(0..4, 3), (2..9, 20)], 5);
    let genesis = chain.verifier_at(1).checkpoint().clone();
    let middle = chain.verifier_at(5).checkpoint().clone();
    let previous = |c: &FinalityCheckpointV1| c.previous_epoch.clone().unwrap();
    let kept = previous(&middle);
    assert_eq!(kept.committee, chain.epochs[0].committee);
    assert_eq!(
        kept.terminal_decision,
        chain.proof_at(3).finality_artifact.commit_qc.as_ref()
    );
    let with = |change: fn(&mut PreviousEpochV1, &FinalityCheckpointV1)| {
        mutated(&middle, |c| {
            let mut kept = previous(c);
            change(&mut kept, c);
            c.previous_epoch = Some(kept);
        })
    };
    assert_invalid_checkpoints(vec![
        with(|kept, _| kept.committee.epoch += 1),
        with(|kept, _| kept.committee.epoch_end_height = 5),
        with(|kept, _| kept.committee.mode = ConsensusMode::Permissioned),
        with(|kept, _| kept.terminal_decision.round.height = 2),
        with(|kept, c| kept.terminal_decision = c.decision),
        with(|kept, _| kept.committee.roster.swap(0, 1)),
        mutated(&genesis, |c| {
            c.previous_epoch = middle.previous_epoch.clone()
        }),
    ]);
}

#[test]
fn checkpoint_at_the_last_representable_height_needs_no_successor() {
    let at = |height: u64| {
        mutated(
            Chain::new(&[(0..4, 20)], 2).verifier_at(2).checkpoint(),
            |c| {
                c.height = nz(height);
                c.decision.round.height = height;
                c.decision.proposal_round = c.decision.round;
                c.committee.epoch_end_height = height;
            },
        )
    };
    at(u64::MAX).validate().unwrap();
    assert!(matches!(
        at(u64::MAX - 1).validate(),
        Err(FinalityError::InvalidCheckpoint(_))
    ));
}

// --- Attestations ------------------------------------------------------------

#[test]
fn attestations_need_2f_plus_1_distinct_committee_members() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 3);
        let verifier = chain.verifier_at(3);
        let quorum = CommitteeSize::new(members).unwrap().quorum();
        let all = chain
            .members(0)
            .iter()
            .map(|node| chain.attest(node, 3, CHALLENGE))
            .collect::<Vec<_>>();

        let report = verifier.attestation_quorum(&CHALLENGE, &all).unwrap();
        assert_eq!(report.verified(), members);
        assert_eq!(report.required, quorum);
        assert_eq!(report.height.get(), 3);
        assert_eq!(verified(&report), vec![3; members]);

        let exact = verifier
            .attestation_quorum(&CHALLENGE, &all[..quorum])
            .unwrap();
        assert_eq!(exact.verified(), quorum);

        let report = insufficient(verifier.attestation_quorum(&CHALLENGE, &all[..quorum - 1]));
        assert_eq!((report.verified(), report.required), (quorum - 1, quorum));
        assert!(
            report.peers[quorum - 1..]
                .iter()
                .all(|(_, outcome)| matches!(outcome, AttestationOutcome::Missing))
        );

        // Repeats from one member and attestations from outsiders never count.
        let outsider = key(97);
        let foreign = attestation(
            &outsider,
            chain.proofs[0].clone(),
            chain.proof_at(3).clone(),
            CHALLENGE,
        );
        assert!(matches!(
            verifier.verify_attestation(&CHALLENGE, &foreign),
            Err(FinalityError::NotInCommittee { .. })
        ));
        let mut padded = all[..quorum - 1].to_vec();
        padded.extend(std::iter::repeat_n(all[0].clone(), quorum));
        padded.push(foreign);
        let report = insufficient(verifier.attestation_quorum(&CHALLENGE, &padded));
        assert_eq!(report.verified(), quorum - 1);
    }
}

#[test]
fn attestations_bind_challenge_signature_network_and_status() {
    let chain = Chain::new(&[(0..4, 20)], 3);
    let verifier = chain.verifier_at(3);
    let node = &chain.members(0)[0];
    let valid = chain.attest(node, 3, CHALLENGE);
    verifier.verify_attestation(&CHALLENGE, &valid).unwrap();

    assert!(matches!(
        verifier.verify_attestation(&[0; 32], &valid),
        Err(FinalityError::ZeroChallenge)
    ));
    assert!(matches!(
        verifier.attestation_quorum(&[0; 32], std::slice::from_ref(&valid)),
        Err(FinalityError::ZeroChallenge)
    ));
    // Signed for an earlier challenge: a replay.
    assert!(matches!(
        verifier.verify_attestation(&[0xC2; 32], &valid),
        Err(FinalityError::StaleChallenge)
    ));
    // Changed after signing.
    let mut changed = valid.clone();
    changed.body.challenge[0] ^= 1;
    let challenge = changed.body.challenge;
    assert!(matches!(
        verifier.verify_attestation(&challenge, &changed),
        Err(FinalityError::Attestation(
            BridgeFinalityAttestationValidationError::InvalidNodeSignature
        ))
    ));
    // Signed by another member than the one it names.
    let impostor = resign(valid.clone(), chain.key_of(&chain.members(0)[1]));
    assert!(matches!(
        verifier.verify_attestation(&CHALLENGE, &impostor),
        Err(FinalityError::Attestation(
            BridgeFinalityAttestationValidationError::InvalidNodeSignature
        ))
    ));
    // The same member on another network.
    let other = Chain::with_genesis_time(&[(0..4, 20)], 3, 7);
    assert!(matches!(
        verifier.verify_attestation(&CHALLENGE, &other.attest(node, 3, CHALLENGE)),
        Err(FinalityError::WrongNetwork { .. })
    ));
    // A fail-stopped node.
    let mut stopped = valid;
    stopped.body.status.restart_required = true;
    let stopped = resign(stopped, chain.key_of(node));
    assert!(matches!(
        verifier.verify_attestation(&CHALLENGE, &stopped),
        Err(FinalityError::Attestation(
            BridgeFinalityAttestationValidationError::RestartRequired
        ))
    ));
}

#[test]
fn attestations_accept_independent_certificate_witnesses() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 3);
        let verifier = chain.verifier_at(3);
        let attestations = chain
            .keys_of(0)
            .iter()
            .enumerate()
            .map(|(shift, key)| {
                let genesis = chain.witness(1, shift + 1);
                let tip = chain.witness(3, shift);
                assert_ne!(&tip, chain.proof_at(3));
                attestation(key, genesis, tip, CHALLENGE)
            })
            .collect::<Vec<_>>();
        let report = verifier
            .attestation_quorum(&CHALLENGE, &attestations)
            .unwrap();
        assert_eq!(report.verified(), members);
    }
}

#[test]
fn attestations_reject_signed_conflicting_decisions() {
    let chain = Chain::new(&[(0..4, 20)], 3);
    let verifier = chain.verifier_at(3);
    let key = &chain.keys_of(0)[0];
    for height in [1, 3] {
        for conflict in 0..3 {
            let mut proof = chain.proof_at(height).clone();
            let artifact = &mut proof.finality_artifact;
            match conflict {
                0 => {
                    artifact.commit_qc.execution_commitment =
                        ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                            Hash::new(b"other parent state"),
                            Hash::new(b"other post state"),
                            Hash::new(b"other writes"),
                            1,
                            Hash::new(b"other executed wire"),
                        );
                }
                1 => {
                    artifact.subject.payload_hash = Hash::new(b"other payload");
                    artifact.commit_qc.subject = artifact.subject;
                }
                _ => {
                    artifact.height_context.nexus_amx_context_hash = Hash::new(b"other nexus");
                }
            }
            sign(&mut proof, chain.keys_of(0), &signers(0..3), 1);
            assert_eq!(
                proof.block_header.hash(),
                chain.proof_at(height).block_header.hash(),
                "the block hash alone cannot identify a decision"
            );
            verify_bridge_finality_proof(&proof, &chain.network)
                .expect("the conflicting decision carries a genuine certificate");
            let (genesis, tip) = if height == 1 {
                (proof, chain.proof_at(3).clone())
            } else {
                (chain.proofs[0].clone(), proof)
            };
            let conflicting = attestation(key, genesis, tip.clone(), CHALLENGE);
            assert!(
                matches!(
                    verifier.verify_attestation(&CHALLENGE, &conflicting),
                    Err(FinalityError::ConflictingDecision { height: found }) if found == height
                ),
                "height {height}, conflict {conflict}"
            );
            if height == 3 {
                assert!(matches!(
                    verifier.clone().advance(&Source::new(&chain), &tip),
                    Err(FinalityError::ConflictingDecision { height: 3 })
                ));
            }
        }
    }
}

#[test]
fn attested_tips_ahead_or_before_the_previous_epoch_do_not_count() {
    // Keys 0..4 serve in every epoch.
    let chain = Chain::new(&[(0..4, 3), (0..4, 6), (0..7, 20)], 9);
    let verifier = chain.verifier_at(8);
    let node = &chain.members(0)[0];
    assert!(chain.members(2).contains(node));
    let at =
        |height| verifier.verify_attestation(&CHALLENGE, &chain.attest(node, height, CHALLENGE));

    assert!(matches!(
        at(9),
        Err(FinalityError::AheadOfCheckpoint {
            checkpoint: 8,
            height: 9
        })
    ));
    assert_eq!(at(8).unwrap().height.get(), 8);
    assert_eq!(
        at(7).unwrap().height.get(),
        7,
        "a lagging node in the same epoch counts"
    );
    // The previous epoch is kept: its terminal and inner tips count.
    for height in [4, 5, 6] {
        assert_eq!(at(height).unwrap().height.get(), height);
    }
    for height in [2, 3] {
        assert!(matches!(
            at(height),
            Err(FinalityError::EarlierEpoch { height: found, epoch: 0 }) if found == height
        ));
    }

    // A validly signed different block at the previous epoch's end conflicts
    // with the terminal decision verified there.
    let conflicting = chain.proof(&chain.successor_header(6, GENESIS_MS + 99), b"other");
    let attested = attestation(
        chain.key_of(node),
        chain.proofs[0].clone(),
        conflicting,
        CHALLENGE,
    );
    assert!(matches!(
        verifier.verify_attestation(&CHALLENGE, &attested),
        Err(FinalityError::ConflictingDecision { height: 6 })
    ));
}

// --- Observation -------------------------------------------------------------

#[test]
fn observe_verifies_fresh_attestations_from_every_member() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 4);
        let nodes = chain.members(0);
        let source = Source::new(&chain).with_tips(&nodes, 4);
        let mut verifier = chain.verifier();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.verified(), members);
        assert_eq!(report.height.get(), 4);
        assert_eq!(verifier.checkpoint().height.get(), 4);
        assert!(source.proof_calls().is_empty());
        assert_eq!(source.attestation_calls.borrow().len(), members);

        // The same height again, under a new challenge, fetches no proofs.
        let fresh = [0xC3; 32];
        let report = verifier.observe(&source, &fresh).unwrap();
        assert_eq!(report.verified(), members);
        assert_eq!(verifier.checkpoint().height.get(), 4);
        assert!(source.proof_calls().is_empty());

        // Lagging members inside the epoch still count.
        let mut source = Source::new(&chain).with_tips(&nodes, 4);
        source
            .tips
            .extend(nodes[..members / 2].iter().map(|node| (node.clone(), 3)));
        let mut verifier = chain.verifier();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.verified(), members);
        let mut heights = verified(&report);
        heights.sort_unstable();
        assert_eq!(heights[0], 3);
        assert_eq!(heights[members - 1], 4);
    }
}

#[test]
fn observe_follows_committee_rotation_to_the_new_members() {
    // Retired members stopped at the hand-over, which names the new
    // committee; the new members are at 5.
    let chain = Chain::new(&[(0..4, 3), (4..11, 20)], 5);
    let source = Source::new(&chain)
        .with_tips(&chain.members(0), 3)
        .with_tips(&chain.members(1), 5);
    let mut verifier = chain.verifier();
    let report = verifier.observe(&source, &CHALLENGE).unwrap();
    assert_eq!(report.height.get(), 5);
    assert_eq!(report.verified(), 7);
    assert_eq!(report.required, 5);
    assert_eq!(verifier.checkpoint().committee, chain.epochs[1].committee);
    assert!(source.proof_calls().is_empty());
    assert_eq!(source.attestation_calls.borrow().len(), 11);

    // Every node already follows the new epoch: the hand-over is fetched, and
    // members of the new committee who were not asked yet are asked.
    let chain = Chain::new(&[(0..4, 3), (2..9, 20)], 5);
    let nodes = (0..9).map(|index| peer(&key(index))).collect::<Vec<_>>();
    let source = Source::new(&chain).with_tips(&nodes, 5);
    let mut verifier = chain.verifier();
    let report = verifier.observe(&source, &CHALLENGE).unwrap();
    assert_eq!(report.height.get(), 5);
    assert_eq!(report.verified(), 7);
    assert_eq!(verifier.checkpoint().committee, chain.epochs[1].committee);
    assert_eq!(source.proof_calls(), vec![3]);
    assert_eq!(source.attestation_calls.borrow().len(), 9);
}

#[test]
fn observe_counts_members_still_behind_a_boundary_that_just_committed() {
    // The boundary at 3 commits while attestations are read: f + 1 members
    // still report the old epoch (one inside it, the rest at its end) and
    // only 2f report the new one.
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 3), (0..members, 20)], 5);
        let nodes = chain.members(0);
        let behind = CommitteeSize::new(members).unwrap().faults() + 1;
        let source = Source::new(&chain)
            .with_tips(&nodes[1..behind], 3)
            .with_tips(&nodes[..1], 2)
            .with_tips(&nodes[behind..], 5);
        let mut verifier = chain.verifier();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.height.get(), 5);
        assert_eq!(report.verified(), members);
        assert_eq!(source.proof_calls(), vec![3]);
        let mut heights = verified(&report);
        heights.sort_unstable();
        assert_eq!(heights[0], 2);
        assert!(heights[1..behind].iter().all(|height| *height == 3));
        assert!(heights[behind..].iter().all(|height| *height == 5));
        assert_eq!(verifier.checkpoint().committee, chain.epochs[1].committee);
    }
}

#[test]
fn observe_tolerates_f_faults_but_not_more() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 20)], 9);
        let nodes = chain.members(0);
        let faults = CommitteeSize::new(members).unwrap().faults();
        let start = chain.verifier();

        // f unreachable members.
        let source = Source::new(&chain).with_tips(&nodes[faults..], 4);
        let mut verifier = start.clone();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.verified(), members - faults);
        assert!(
            report.peers[..faults]
                .iter()
                .all(|(_, outcome)| matches!(outcome, AttestationOutcome::Unreachable(_)))
        );

        // A Byzantine member with an unsigned higher tip does not block the
        // others; its own attestation is rejected.
        let byzantine = &nodes[members - 1];
        let mut bogus = chain.proof_at(9).clone();
        bogus.finality_artifact.commit_qc.aggregate_signature[0] ^= 1;
        let mut source = Source::new(&chain).with_tips(&nodes, 4);
        source.attestations.insert(
            byzantine.clone(),
            attestation(
                chain.key_of(byzantine),
                chain.proofs[0].clone(),
                bogus,
                CHALLENGE,
            ),
        );
        let mut verifier = start.clone();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.height.get(), 4);
        assert_eq!(report.verified(), members - 1);
        assert!(matches!(
            report
                .peers
                .iter()
                .find(|(node, _)| node == byzantine)
                .unwrap()
                .1,
            AttestationOutcome::Rejected(FinalityError::AheadOfCheckpoint { .. })
        ));

        // f unreachable plus one replayed attestation is too few, and a
        // disconnected member cannot mask the replay. The checkpoint stays.
        let replayer = &nodes[faults];
        let mut source = Source::new(&chain).with_tips(&nodes[faults..], 4);
        source
            .attestations
            .insert(replayer.clone(), chain.attest(replayer, 4, [0xC4; 32]));
        let mut verifier = start.clone();
        let report = insufficient(verifier.observe(&source, &CHALLENGE));
        assert_eq!(report.verified(), members - faults - 1);
        assert!(matches!(
            report.peers[faults].1,
            AttestationOutcome::Rejected(FinalityError::StaleChallenge)
        ));
        assert_eq!(verifier, start);
    }
}

#[test]
fn observe_rejects_zero_challenges_and_substituted_nodes() {
    let chain = Chain::new(&[(0..4, 20)], 2);
    let nodes = chain.members(0);
    let mut source = Source::new(&chain).with_tips(&nodes, 2);
    let mut verifier = chain.verifier();
    assert!(matches!(
        verifier.observe(&source, &[0; 32]),
        Err(FinalityError::ZeroChallenge)
    ));
    assert!(source.attestation_calls.borrow().is_empty());

    // Asked node 0, answered by node 1.
    source
        .attestations
        .insert(nodes[0].clone(), chain.attest(&nodes[1], 2, CHALLENGE));
    let report = verifier.observe(&source, &CHALLENGE).unwrap();
    assert_eq!(report.verified(), 3);
    assert!(matches!(
        report.peers[0].1,
        AttestationOutcome::Rejected(FinalityError::UnexpectedPeer { .. })
    ));
}
