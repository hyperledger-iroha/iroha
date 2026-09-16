//! Fresh, four-peer committed-height evidence rooted in a native prepared genesis bundle.

use super::*;
use std::time::Instant;

/// A poll can report progress without authorizing an unauthenticated height.
pub(crate) enum HeightObservationV1 {
    Pending,
    Verified(VerifiedCommittedHeightV1),
}

/// Only this module can construct a height accepted by the bootstrap controller.
/// Its public evidence contains every immediate successor from the signed genesis.
#[derive(JsonSerialize)]
pub(crate) struct VerifiedCommittedHeightV1 {
    schema: String,
    network_id: NetworkId,
    genesis_block_hash: HashOf<BlockHeader>,
    committed_height: NonZeroU64,
    block_hash: HashOf<BlockHeader>,
    before_challenge: [u8; 32],
    after_challenge: [u8; 32],
    proofs: Vec<BridgeFinalityProof>,
    peers: Vec<PeerHeightEvidenceV1>,
}

impl VerifiedCommittedHeightV1 {
    pub(crate) fn committed_height(&self) -> NonZeroU64 {
        self.committed_height
    }

    pub(crate) fn block_hash(&self) -> HashOf<BlockHeader> {
        self.block_hash
    }

    /// Retrieve only a proof admitted into this observation's contiguous prefix.
    pub(crate) fn proof_at(&self, height: NonZeroU64) -> Option<&BridgeFinalityProof> {
        self.proofs.get(usize::try_from(height.get() - 1).ok()?)
    }
}

#[derive(JsonSerialize)]
struct PeerHeightEvidenceV1 {
    peer: PeerV1,
    before: BridgeFinalityAttestationV1,
    after: BridgeFinalityAttestationV1,
}

/// A verified chain is retained only in memory; every emitted height requires new peer reads.
pub(crate) struct AuthenticatedHeightObserverV1 {
    authority: Authority,
    peers: Vec<PeerV1>,
    verifier: Option<BridgeFinalityVerifier>,
    proofs: Vec<BridgeFinalityProof>,
    emitted_height: u64,
}

trait HeightReads: Sync {
    fn tip(&self, peer: usize) -> Result<u64>;
    fn attest(
        &self,
        peer: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
        identity: &PeerId,
    ) -> Result<BridgeFinalityAttestationV1>;
    fn next_proof(
        &self,
        peer: usize,
        height: NonZeroU64,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof>;
}

struct NativeReads {
    clients: [Client; VERIFICATION_PEERS],
}

impl HeightReads for NativeReads {
    fn tip(&self, peer: usize) -> Result<u64> {
        Ok(self.clients[peer]
            .get_sumeragi_status()?
            .last_committed_height)
    }

    fn attest(
        &self,
        peer: usize,
        height: NonZeroU64,
        challenge: [u8; 32],
        identity: &PeerId,
    ) -> Result<BridgeFinalityAttestationV1> {
        self.clients[peer].get_bridge_finality_attestation(height, challenge, identity)
    }

    fn next_proof(
        &self,
        peer: usize,
        height: NonZeroU64,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof> {
        self.clients[peer].get_next_bridge_finality_proof(height, verifier)
    }
}

impl AuthenticatedHeightObserverV1 {
    /// The opaque bundle can only be obtained through native signed-manifest validation.
    pub(crate) fn new(
        genesis: &iroha_genesis::ValidatedGenesisBundle,
        peers: Vec<PeerV1>,
    ) -> Result<Self> {
        require(
            genesis.consensus_metadata().mode
                == iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
            "authenticated height requires the prepared NPoS genesis",
        )?;
        let validators = genesis
            .validator_pops()
            .iter()
            .map(|(key, pop)| (PeerId::new(key.clone()), pop.clone()))
            .collect::<BTreeMap<_, _>>();
        validate_peer_selection(&peers, &validators)?;
        let (roster, pops) = validators
            .into_iter()
            .map(|(validator, pop)| {
                (
                    ValidatorPower {
                        validator,
                        power: 1,
                    },
                    pop,
                )
            })
            .unzip();
        Ok(Self {
            authority: Authority {
                network: NetworkId::from_genesis_hash(genesis.expected_hash()),
                genesis: genesis.expected_hash(),
                roster,
                pops,
            },
            peers,
            verifier: None,
            proofs: Vec::new(),
            emitted_height: 0,
        })
    }

    /// Observe one bounded turn using the caller's independently configured four native clients.
    pub(crate) fn observe(
        &mut self,
        clients: &[Client; VERIFICATION_PEERS],
        discriminant: u16,
        deadline: Instant,
    ) -> Result<HeightObservationV1> {
        require_operation_budget(deadline, "starting authenticated height observation")?;
        let readers = NativeReads {
            clients: std::array::from_fn(|index| clients[index].with_request_deadline(deadline)),
        };
        self.observe_with(
            &readers,
            discriminant,
            deadline,
            rand::random(),
            rand::random(),
        )
    }

    fn capture(
        &self,
        reads: &impl HeightReads,
        discriminant: u16,
        deadline: Instant,
        challenge: [u8; 32],
    ) -> Result<Vec<PeerRead<BridgeFinalityAttestationV1>>> {
        read_four_peers(&self.peers, discriminant, |index, peer| {
            require_operation_budget(deadline, "reading authenticated validator height")?;
            let Some(height) = NonZeroU64::new(reads.tip(index)?) else {
                return Ok(PeerRead::Pending);
            };
            require_operation_budget(deadline, "challenging validator finality")?;
            let attestation = match reads.attest(index, height, challenge, &peer.peer_id) {
                Ok(value) => value,
                Err(error)
                    if error
                        .downcast_ref::<iroha::client::BridgeFinalityAttestationTipMismatch>()
                        .is_some() =>
                {
                    require_operation_budget(deadline, "validator finality tip is moving")?;
                    return Ok(PeerRead::Pending);
                }
                Err(error) => return Err(error),
            };
            validate_attestation(&self.authority, peer, challenge, &attestation)?;
            require(
                attestation.body.finality_proof.block_header.height() == height,
                "attested proof differs from requested height",
            )?;
            // Even a peer that is behind another peer must supply a valid certificate.
            attestation.body.finality_proof.finality_artifact.verify()?;
            require_operation_budget(deadline, "verified validator height attestation")?;
            Ok(PeerRead::Verified(attestation))
        })
    }

    fn observe_with(
        &mut self,
        reads: &impl HeightReads,
        discriminant: u16,
        deadline: Instant,
        before_challenge: [u8; 32],
        after_challenge: [u8; 32],
    ) -> Result<HeightObservationV1> {
        require_operation_budget(deadline, "starting authenticated height observation")?;
        require(
            before_challenge != [0; 32]
                && after_challenge != [0; 32]
                && before_challenge != after_challenge,
            "height observation needs two distinct fresh challenges",
        )?;
        let before = self.capture(reads, discriminant, deadline, before_challenge)?;
        require_operation_budget(deadline, "captured validator height attestations")?;
        let Some((source_index, source)) = before
            .iter()
            .enumerate()
            .filter_map(|(index, read)| match read {
                PeerRead::Verified(value) => Some((index, value)),
                PeerRead::Pending => None,
            })
            .max_by_key(|(_, proof)| proof.body.finality_proof.block_header.height())
        else {
            return Ok(HeightObservationV1::Pending);
        };
        let target_height = source.body.finality_proof.block_header.height();
        let mut new_proofs = 0;
        if !self.synchronize(reads, source_index, source, deadline, &mut new_proofs)? {
            return Ok(HeightObservationV1::Pending);
        }
        // Reject a conflicting proof before classifying any other peer as still progressing.
        for read in &before {
            if let PeerRead::Verified(attestation) = read {
                self.require_chain_tip(attestation)?;
            }
        }
        let Some(before) = before
            .into_iter()
            .map(|read| match read {
                PeerRead::Verified(value) => Some(value),
                PeerRead::Pending => None,
            })
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(HeightObservationV1::Pending);
        };
        if before.iter().any(|attestation| {
            attestation.body.finality_proof.block_header.height() != target_height
        }) || target_height.get() <= self.emitted_height
        {
            return Ok(HeightObservationV1::Pending);
        }
        let after = self.capture(reads, discriminant, deadline, after_challenge)?;
        require_operation_budget(deadline, "rechecked authenticated validator heights")?;
        if let Some((index, tip)) = after
            .iter()
            .enumerate()
            .filter_map(|(index, read)| match read {
                PeerRead::Verified(value) => Some((index, value)),
                PeerRead::Pending => None,
            })
            .max_by_key(|(_, proof)| proof.body.finality_proof.block_header.height())
        {
            if !self.synchronize(reads, index, tip, deadline, &mut new_proofs)? {
                return Ok(HeightObservationV1::Pending);
            }
        }
        let mut pending = false;
        for read in &after {
            match read {
                PeerRead::Pending => pending = true,
                PeerRead::Verified(attestation) => {
                    let height = attestation.body.finality_proof.block_header.height();
                    self.require_chain_tip(attestation)?;
                    pending |= height != target_height;
                }
            }
        }
        if pending {
            return Ok(HeightObservationV1::Pending);
        }
        let peers = before
            .into_iter()
            .zip(after)
            .enumerate()
            .map(|(index, (before, after))| {
                let PeerRead::Verified(after) = after else {
                    unreachable!("pending captures returned above")
                };
                PeerHeightEvidenceV1 {
                    peer: self.peers[index].clone(),
                    before,
                    after,
                }
            })
            .collect();
        let proof = self
            .proofs
            .get(usize::try_from(target_height.get() - 1)?)
            .ok_or_else(|| eyre!("missing authenticated target proof"))?;
        let evidence = VerifiedCommittedHeightV1 {
            schema: "iroha.taira.authenticated-committed-height.v1".to_owned(),
            network_id: self.authority.network,
            genesis_block_hash: self.authority.genesis,
            committed_height: target_height,
            block_hash: proof.block_header.hash(),
            before_challenge,
            after_challenge,
            proofs: self.proofs[..usize::try_from(target_height.get())?].to_vec(),
            peers,
        };
        require_operation_budget(deadline, "publishing authenticated committed height")?;
        self.emitted_height = target_height.get();
        Ok(HeightObservationV1::Verified(evidence))
    }

    fn synchronize(
        &mut self,
        reads: &impl HeightReads,
        source_index: usize,
        source: &BridgeFinalityAttestationV1,
        deadline: Instant,
        new_proofs: &mut usize,
    ) -> Result<bool> {
        let target_height = source.body.finality_proof.block_header.height();
        while u64::try_from(self.proofs.len())? < target_height.get() {
            require_operation_budget(deadline, "synchronizing authenticated height proof chain")?;
            if *new_proofs == MAX_NEW_PROOFS {
                return Ok(false);
            }
            let next = NonZeroU64::new(
                u64::try_from(self.proofs.len())?
                    .checked_add(1)
                    .ok_or_else(|| eyre!("authenticated proof height overflow"))?,
            )
            .unwrap();
            let (proof, advanced) = if next.get() == 1 {
                let proof = source.body.genesis_finality_proof.clone();
                let verifier = self.authority.anchor(&proof)?;
                (proof, verifier)
            } else {
                let mut verifier = self
                    .verifier
                    .clone()
                    .ok_or_else(|| eyre!("missing genesis verifier"))?;
                let proof = reads.next_proof(source_index, next, &mut verifier)?;
                (proof, verifier)
            };
            require(
                proof.block_header.height() == next,
                "finality chain skipped or reordered a height",
            )?;
            self.authority.roster(&proof)?;
            require_operation_budget(deadline, "verified authenticated successor proof")?;
            self.verifier = Some(advanced);
            self.proofs.push(proof);
            *new_proofs += 1;
        }
        Ok(true)
    }

    fn require_chain_tip(&self, attestation: &BridgeFinalityAttestationV1) -> Result<()> {
        let tip = &attestation.body.finality_proof;
        require(
            self.proofs.first() == Some(&attestation.body.genesis_finality_proof)
                && self
                    .proofs
                    .get(usize::try_from(tip.block_header.height().get() - 1)?)
                    == Some(tip),
            "validator proof conflicts with the authenticated contiguous chain",
        )
    }
}

#[cfg(test)]
#[path = "taira_authenticated_height_tests.rs"]
mod tests;
