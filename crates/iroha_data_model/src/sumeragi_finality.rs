//! Portable current-consensus proofs rooted in an independently authenticated signed genesis.
//!
//! Proofs carry the canonical block and its embedded commit certificate. A structural
//! decode is never an authenticated execution capability: only the contiguous verifier
//! constructs [`VerifiedSumeragiBlock`]. Genesis has no quorum certificate; its execution
//! result is authenticated by a successor's parent-result binding or independent node
//! attestations, not by inventing a genesis quorum certificate.

mod commitment;
pub use commitment::*;

use std::collections::BTreeMap;

use iroha_crypto::{
    Algorithm, BlsNormalPopVerifiedKey, Hash, HashOf, PublicKey, SignatureOf,
    bls_normal_aggregate_signatures, bls_normal_verify_preaggregated_multi_message,
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    crypto::{Crypto, NoAttestation, verify_qc},
    message::{BlockHeader as CoreHeader, Qc, VoteKind},
    preimage::{InstanceKind, committee_digest_preimage, instance_id, payload_hash},
    types::{AggregateSignature, Committee, Hash32, PublicKey as CoreKey, Signature},
};
use norito::{
    Decode, Encode,
    codec::Encode as _,
    derive::{JsonDeserialize, JsonSerialize},
};

use crate::{
    NetworkId,
    block::{BlockHeader, SignedBlock, decode_versioned_signed_block},
    query::CommittedTransaction,
    sumeragi::SumeragiStatus,
    transaction::TransactionEntrypoint,
};

/// Maximum canonical certified block accepted by a portable proof reader.
pub const MAX_FINALITY_BLOCK_BYTES: usize = 32 * 1024 * 1024;
/// Domain of current node finality statements.
pub const FINALITY_ATTESTATION_DOMAIN: &[u8] = b"iroha:sumeragi-finality-attestation:v1\0";

/// A failed current proof, trust binding, certificate or node statement.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("current finality: {0}")]
pub struct FinalityError(pub String);

fn need(condition: bool, reason: &str) -> Result<(), FinalityError> {
    if condition {
        Ok(())
    } else {
        Err(FinalityError(reason.into()))
    }
}
fn malformed(error: impl std::fmt::Display) -> FinalityError {
    FinalityError(error.to_string())
}

/// A consensus key and its proof of possession; its authority comes from the trusted schedule.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::FinalityValidator")]
pub struct FinalityValidator {
    /// BLS-normal validator key.
    pub public_key: PublicKey,
    /// Canonical proof of possession for this key.
    pub proof_of_possession: Vec<u8>,
}

/// The one canonical current block frame and the candidate committee of its height.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiFinalityProof")]
pub struct SumeragiFinalityProof {
    /// Header, cross-checked against the complete canonical frame.
    pub block_header: BlockHeader,
    /// Result-bearing canonical SignedBlockWire, including the embedded current certificate.
    pub block_wire: Vec<u8>,
    /// Committee, admitted only against the signed genesis or an authenticated lag-2 digest.
    pub committee: Vec<FinalityValidator>,
}

/// A network-qualified current proof, without a second consensus-context layout.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiFinalityBundle")]
pub struct SumeragiFinalityBundle {
    /// Independently selected genesis-derived network identity.
    pub network_id: NetworkId,
    /// Current embedded-certificate proof.
    pub finality_proof: SumeragiFinalityProof,
}

/// Checked structure and certificate under a proof's candidate committee, without a trust root.
#[derive(Debug, Clone)]
pub struct DecodedSumeragiBlock {
    block: SignedBlock,
    header: Option<CoreHeader>,
    core_hash: Hash32,
    result: Hash32,
    commitment: ExecutionResultCommitment,
    committee_digest: [u8; 32],
}

impl SumeragiFinalityProof {
    /// One-based height claimed by the proof; a verifier checks its frame binding.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.block_header.height().get()
    }

    /// Decode and check canonical framing, exact execution identity and the candidate QC.
    /// This does not authenticate the committee or genesis. Use the contiguous verifier.
    ///
    /// # Errors
    /// Malformed, empty, inconsistent, oversized or cryptographically invalid material.
    pub fn decode_checked(&self) -> Result<DecodedSumeragiBlock, FinalityError> {
        need(
            !self.block_wire.is_empty() && self.block_wire.len() <= MAX_FINALITY_BLOCK_BYTES,
            "block frame exceeds its finite bound",
        )?;
        let block = norito::core::with_decode_limits_scope(
            norito::canonical_decode_limits(self.block_wire.len()),
            || decode_versioned_signed_block(&self.block_wire),
        )
        .map_err(malformed)?;
        need(
            block.encode_wire().map_err(malformed)? == self.block_wire,
            "block frame is not canonical",
        )?;
        need(
            block.header() == self.block_header && block.has_results(),
            "header or execution-result binding differs",
        )?;
        block.validate_proposal_commitments().map_err(malformed)?;
        block.validate_output_merkle_cache().map_err(malformed)?;
        let (crypto, committee) = ProofCrypto::new(&self.committee)?;
        let committee_digest = chain_hash(&committee_digest_preimage(&committee)).0;
        let certificate = block
            .commit_certificate()
            .ok_or_else(|| FinalityError("embedded commit certificate missing".into()))?;
        let commitment =
            ExecutionResultCommitment::decode(&certificate.result_preimage).map_err(malformed)?;
        commitment.next_params.validate().map_err(malformed)?;
        let result = result_of_preimage(&certificate.result_preimage);
        let (len, hash) = block.executed_block_wire_identity().map_err(malformed)?;
        need(
            commitment.execution.executed_block_wire_len == len
                && commitment.execution.executed_block_wire_hash == hash
                && commitment.execution.transaction_input_commitment
                    == block.network_input_merkle_commitment()
                && commitment.execution.transaction_output_commitment
                    == block.output_merkle_commitment(),
            "execution commitment differs from canonical result-bearing block",
        )?;
        let (header, core_hash) = if self.height() == 1 {
            need(
                certificate.consensus_header.is_empty() && certificate.commit_qc.is_empty(),
                "genesis requires a result-only certificate",
            )?;
            (None, Hash32(Hash::from(block.hash()).into()))
        } else {
            need(
                block.network_entrypoint_count() > 0,
                "empty blocks are invalid",
            )?;
            let header: CoreHeader =
                norito::decode_canonical(&certificate.consensus_header).map_err(malformed)?;
            let qc: Qc = norito::decode_canonical(&certificate.commit_qc).map_err(malformed)?;
            let payload = block
                .canonical_resultless_proposal()
                .encode_wire()
                .map_err(malformed)?;
            let core_hash = header.hash(&crypto);
            need(
                header.height == self.height()
                    && usize::try_from(header.payload_len).ok() == Some(payload.len())
                    && header.payload_hash == payload_hash(&crypto, &payload)
                    && qc.kind == VoteKind::Commit
                    && qc.height == header.height
                    && qc.instance == header.instance
                    && qc.block_hash == core_hash
                    && qc.result == result
                    && qc.attest == header.attest,
                "current certificate does not bind this block and execution",
            )?;
            // No unverified application attestation can be relayed as a verified proof.
            verify_qc(&crypto, &NoAttestation, &header.instance, &committee, &qc)
                .map_err(|error| FinalityError(format!("commit certificate: {error:?}")))?;
            (Some(header), core_hash)
        };
        Ok(DecodedSumeragiBlock {
            block,
            header,
            core_hash,
            result,
            commitment,
            committee_digest,
        })
    }
}

/// Execution authenticated by the verifier's independently anchored contiguous prefix.
#[derive(Debug, Clone)]
pub struct VerifiedSumeragiBlock(DecodedSumeragiBlock);
impl VerifiedSumeragiBlock {
    /// The authenticated Iroha header.
    #[must_use]
    pub fn header(&self) -> BlockHeader {
        self.0.block.header()
    }
    /// The authenticated full frame (its certificate witness can differ between nodes).
    #[must_use]
    pub fn block(&self) -> &SignedBlock {
        &self.0.block
    }
    /// One-based authenticated height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.0.block.header().height().get()
    }
    /// The core header hash, or selected genesis hash at height one.
    #[must_use]
    pub fn core_hash(&self) -> Hash32 {
        self.0.core_hash
    }
    /// The certified execution-result hash.
    #[must_use]
    pub fn result(&self) -> Hash32 {
        self.0.result
    }
    /// The current execution commitment, authenticated without a V2 projection.
    #[must_use]
    pub fn execution(&self) -> &ExecutionCommitment {
        &self.0.commitment.execution
    }
    /// Complete current result and next schedule commitment.
    #[must_use]
    pub fn commitment(&self) -> &ExecutionResultCommitment {
        &self.0.commitment
    }
    /// Canonical executed bytes with only the node-local certificate removed.
    ///
    /// # Errors
    /// Canonical wire encoding fails.
    pub fn canonical_executed_wire(&self) -> Result<Vec<u8>, FinalityError> {
        self.0
            .block
            .clone()
            .with_commit_certificate(None)
            .encode_wire()
            .map_err(malformed)
    }
    /// Verify a successful external transaction's exact input/output membership and signature.
    ///
    /// # Errors
    /// Rejection, foreign network, invalid signature, substituted output or invalid inclusion.
    pub fn verify_committed_transaction(
        &self,
        network: &NetworkId,
        committed: &CommittedTransaction,
    ) -> Result<(), FinalityError> {
        need(
            self.height() > 1,
            "genesis execution needs independent node attestations or a certified successor",
        )?;
        let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
            return Err(FinalityError(
                "expected external committed transaction".into(),
            ));
        };
        need(
            committed.result().is_ok()
                && transaction.network_id() == Some(network)
                && transaction.verify_signature().is_ok()
                && committed.verify_inclusion_in_block(&self.0.block),
            "transaction does not match successful authenticated execution",
        )
    }
}

#[derive(Debug, Clone)]
struct Decision {
    block_hash: HashOf<BlockHeader>,
    core_hash: Hash32,
    result: Hash32,
    committee_digest: [u8; 32],
    next_committee_digest: [u8; 32],
    executed_hash: Hash,
    executed_len: u64,
}

/// Independent current-certificate verifier retaining only authenticated prefix decisions.
#[derive(Debug, Clone)]
pub struct SumeragiFinalityVerifier {
    genesis: SignedBlock,
    instance: Hash32,
    genesis_committee_digest: [u8; 32],
    decisions: BTreeMap<u64, Decision>,
}
impl SumeragiFinalityVerifier {
    /// Begin with a caller-authenticated signed genesis and its independently selected roster.
    /// The caller must validate the genesis signature before selecting this trust root.
    ///
    /// # Errors
    /// The root is not genesis or its committee keys/PoPs are invalid.
    pub fn new(
        trusted_genesis: &SignedBlock,
        chain_id: &str,
        validators: Vec<FinalityValidator>,
    ) -> Result<Self, FinalityError> {
        need(
            trusted_genesis.header().is_genesis(),
            "trust root must be signed genesis",
        )?;
        let (crypto, committee) = ProofCrypto::new(&validators)?;
        let instance = instance_id(
            &crypto,
            &Hash32(Hash::from(trusted_genesis.hash()).into()),
            chain_id.as_bytes(),
            InstanceKind::Global,
            0,
        );
        Ok(Self {
            genesis: trusted_genesis.clone(),
            instance,
            genesis_committee_digest: chain_hash(&committee_digest_preimage(&committee)).0,
            decisions: BTreeMap::new(),
        })
    }
    /// The selected current consensus instance.
    #[must_use]
    pub fn instance(&self) -> Hash32 {
        self.instance
    }
    /// Admit exactly the next proof into the authenticated contiguous prefix.
    ///
    /// # Errors
    /// Gaps, altered roots, wrong committee/instance, or any invalid certificate/binding.
    pub fn verify(
        &mut self,
        proof: &SumeragiFinalityProof,
    ) -> Result<VerifiedSumeragiBlock, FinalityError> {
        let next = self
            .decisions
            .last_key_value()
            .map_or(1, |(height, _)| height.saturating_add(1));
        need(
            proof.height() == next,
            "proof must immediately extend the authenticated prefix",
        )?;
        let decoded = self.check(proof)?;
        self.decisions.insert(next, Self::decision(&decoded));
        Ok(VerifiedSumeragiBlock(decoded))
    }
    /// Re-verify an alternate certificate witness for a decision already in this prefix.
    /// Equal inputs still undergo cryptographic verification and prefix-membership checks.
    ///
    /// # Errors
    /// Either proof is outside this prefix or carries a different decision or invalid witness.
    pub fn verify_same_decision(
        &self,
        retained: &SumeragiFinalityProof,
        candidate: &SumeragiFinalityProof,
    ) -> Result<VerifiedSumeragiBlock, FinalityError> {
        need(
            retained.height() == candidate.height(),
            "alternate proof height differs",
        )?;
        let expected = self
            .decisions
            .get(&retained.height())
            .ok_or_else(|| FinalityError("decision is outside authenticated prefix".into()))?;
        let retained = self.check(retained)?;
        let candidate = self.check(candidate)?;
        for value in [&retained, &candidate] {
            let found = Self::decision(value);
            need(
                found.block_hash == expected.block_hash
                    && found.core_hash == expected.core_hash
                    && found.result == expected.result
                    && found.committee_digest == expected.committee_digest
                    && found.next_committee_digest == expected.next_committee_digest
                    && found.executed_hash == expected.executed_hash
                    && found.executed_len == expected.executed_len,
                "alternate proof differs from authenticated decision",
            )?;
        }
        Ok(VerifiedSumeragiBlock(candidate))
    }
    fn decision(value: &DecodedSumeragiBlock) -> Decision {
        Decision {
            block_hash: value.block.hash(),
            core_hash: value.core_hash,
            result: value.result,
            committee_digest: value.committee_digest,
            next_committee_digest: value.commitment.next_committee_digest,
            executed_hash: value.commitment.execution.executed_block_wire_hash,
            executed_len: value.commitment.execution.executed_block_wire_len,
        }
    }
    fn check(&self, proof: &SumeragiFinalityProof) -> Result<DecodedSumeragiBlock, FinalityError> {
        let decoded = proof.decode_checked()?;
        let height = proof.height();
        if height == 1 {
            need(
                proof.block_header.hash() == self.genesis.hash()
                    && decoded
                        .block
                        .canonical_resultless_proposal()
                        .encode_wire()
                        .map_err(malformed)?
                        == self
                            .genesis
                            .canonical_resultless_proposal()
                            .encode_wire()
                            .map_err(malformed)?
                    && decoded.committee_digest == self.genesis_committee_digest,
                "genesis proof differs from independently selected signed root",
            )?;
        } else {
            let parent = self
                .decisions
                .get(&(height - 1))
                .ok_or_else(|| FinalityError("authenticated parent is missing".into()))?;
            let expected_committee = if height == 2 {
                self.genesis_committee_digest
            } else {
                self.decisions
                    .get(&(height - 2))
                    .ok_or_else(|| FinalityError("authenticated schedule is missing".into()))?
                    .next_committee_digest
            };
            let header = decoded
                .header
                .as_ref()
                .ok_or_else(|| FinalityError("current header missing".into()))?;
            need(
                header.instance == self.instance
                    && header.parent_hash == parent.core_hash
                    && header.parent_result == parent.result
                    && decoded.block.header().prev_block_hash() == Some(parent.block_hash)
                    && decoded.committee_digest == expected_committee,
                "proof breaks authenticated instance, parent/result or lag-2 committee binding",
            )?;
        }
        Ok(decoded)
    }
}

/// One current node's challenge-bound immutable tip capture and runtime identities.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiFinalityAttestationBody")]
pub struct SumeragiFinalityAttestationBody {
    /// Fresh, nonzero caller challenge.
    pub challenge: [u8; 32],
    /// Genesis-derived selected network.
    pub network_id: NetworkId,
    /// Signing node's BLS identity.
    pub node_id: PeerId,
    /// Hash of canonical encoded node identity.
    pub node_fingerprint: Hash,
    /// Installed executable identity.
    pub build_fingerprint: Hash,
    /// Effective consensus configuration identity.
    pub config_fingerprint: Hash,
    /// The captured state's genesis hash.
    pub genesis_block_hash: HashOf<BlockHeader>,
    /// Current result-only genesis frame.
    pub genesis_finality_proof: SumeragiFinalityProof,
    /// Live NodeHandle status captured for this tip.
    pub status: SumeragiStatus,
    /// Current committed tip frame and certificate.
    pub finality_proof: SumeragiFinalityProof,
}
impl SumeragiFinalityAttestationBody {
    /// Domain-separated hash signed by the reporting node.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            FINALITY_ATTESTATION_DOMAIN,
            &self.encode(),
        ]))
    }
    /// Validate structural duplicate bindings; authority still requires the selected node signature.
    ///
    /// # Errors
    /// Wrong challenge, runtime identity, status, genesis, height or certificate binding.
    pub fn validate_consistency(&self) -> Result<(), FinalityError> {
        need(
            self.challenge != [0; 32]
                && self.node_fingerprint == Hash::new(self.node_id.encode())
                && self.node_id.public_key().algorithm() == Algorithm::BlsNormal
                && self.network_id == NetworkId::from_genesis_hash(self.genesis_block_hash)
                && self.genesis_finality_proof.height() == 1
                && self.genesis_finality_proof.block_header.hash() == self.genesis_block_hash
                && self.status.halted.is_none()
                && self
                    .status
                    .signer
                    .as_ref()
                    .is_none_or(|key| key == self.node_id.public_key())
                && self.status.committed_height == self.finality_proof.height()
                && self.status.applied_height == self.status.committed_height,
            "attestation challenge, identity or durable-tip binding differs",
        )?;
        let genesis = self.genesis_finality_proof.decode_checked()?;
        let tip = self.finality_proof.decode_checked()?;
        if let Some(header) = tip.header.as_ref() {
            need(
                header.instance.0 == self.status.instance,
                "attestation status instance differs",
            )?;
        } else {
            need(
                tip.core_hash == genesis.core_hash
                    && tip.result == genesis.result
                    && tip.commitment == genesis.commitment,
                "height-one attestation carries different genesis execution",
            )?;
        }
        Ok(())
    }
}

/// BLS node signature over one exact current finality capture.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation")]
pub struct SumeragiFinalityAttestation {
    /// Complete challenged statement.
    pub body: SumeragiFinalityAttestationBody,
    /// Reporting node's signature over the domain-separated body hash.
    pub signature: SignatureOf<SumeragiFinalityAttestationBody>,
}
impl SumeragiFinalityAttestation {
    /// Check body consistency and its declared node's signature; callers must select that node independently.
    ///
    /// # Errors
    /// Malformed body or invalid BLS signature.
    pub fn verify(&self) -> Result<(), FinalityError> {
        self.body.validate_consistency()?;
        self.signature
            .verify_hash(self.body.node_id.public_key(), self.body.signing_hash())
            .map_err(malformed)
    }
}

struct ProofCrypto {
    keys: BTreeMap<CoreKey, BlsNormalPopVerifiedKey>,
}
impl ProofCrypto {
    fn new(validators: &[FinalityValidator]) -> Result<(Self, Committee), FinalityError> {
        need(
            !validators.is_empty() && validators.len() <= iroha_sumeragi::types::MAX_COMMITTEE_SIZE,
            "committee exceeds its finite bound",
        )?;
        let mut keys = BTreeMap::new();
        let mut ordered = Vec::new();
        for validator in validators {
            let (algorithm, bytes) = validator.public_key.try_to_bytes().map_err(malformed)?;
            need(
                algorithm == Algorithm::BlsNormal,
                "committee key is not BLS-normal",
            )?;
            let key = CoreKey::new(bytes.to_vec()).map_err(malformed)?;
            let verified =
                BlsNormalPopVerifiedKey::new(&validator.public_key, &validator.proof_of_possession)
                    .map_err(malformed)?;
            need(
                keys.insert(key.clone(), verified).is_none(),
                "duplicate committee key",
            )?;
            ordered.push(key);
        }
        let committee = Committee::new(ordered.clone()).map_err(malformed)?;
        need(
            ordered == committee.members(),
            "committee is not in canonical consensus key order",
        )?;
        Ok((Self { keys }, committee))
    }
}
impl Crypto for ProofCrypto {
    fn hash(&self, bytes: &[u8]) -> Hash32 {
        chain_hash(bytes)
    }
    fn verify(&self, pk: &CoreKey, msg: &[u8], signature: &Signature) -> bool {
        PublicKey::from_bytes(Algorithm::BlsNormal, pk.as_bytes()).is_ok_and(|key| {
            iroha_crypto::Signature::from_bytes(&signature.0)
                .verify(&key, msg)
                .is_ok()
        })
    }
    fn aggregate(&self, signatures: &[Signature]) -> AggregateSignature {
        let slices: Vec<&[u8]> = signatures
            .iter()
            .map(|signature| signature.0.as_slice())
            .collect();
        AggregateSignature(
            bls_normal_aggregate_signatures(&slices)
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .unwrap_or([0; 96]),
        )
    }
    fn verify_aggregate(
        &self,
        keys: &[&CoreKey],
        message: &[u8],
        signature: &AggregateSignature,
    ) -> bool {
        self.verify_aggregate_multi(&[(keys.to_vec(), message.to_vec())], signature)
    }
    fn verify_aggregate_multi(
        &self,
        groups: &[(Vec<&CoreKey>, Vec<u8>)],
        signature: &AggregateSignature,
    ) -> bool {
        let Some(keys) = groups
            .iter()
            .map(|(keys, _)| {
                keys.iter()
                    .map(|key| self.keys.get(*key))
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let groups: Vec<(&[&BlsNormalPopVerifiedKey], &[u8])> = keys
            .iter()
            .zip(groups)
            .map(|(keys, (_, message))| (keys.as_slice(), message.as_slice()))
            .collect();
        bls_normal_verify_preaggregated_multi_message(&groups, &signature.0).is_ok()
    }
}

#[cfg(all(test, feature = "transparent_api"))]
mod tests;
