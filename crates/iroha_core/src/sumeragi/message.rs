//! Contains message structures for p2p communication during consensus.
use iroha_crypto::{Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, SignedBlock,
        consensus::{LaneBlockCertificateV1, LaneBlockProposalV1, LaneBlockQcV1},
        consensus_v2::{
            ConsensusMessageV2, ExecutionCommitment, MAX_EXECUTED_BLOCK_WIRE_BYTES,
            finality::V2FinalityArtifact,
        },
    },
    peer::PeerId,
};
use iroha_macro::*;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    core as ncore,
};
use std::{collections::BTreeMap, sync::Arc};
#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
/// Messages used by peers to communicate during the consensus process.
#[derive(Debug, Clone, Decode, Encode, FromVariant)]
pub enum BlockMessage {
    /// Advertisement that a peer durably retains a canonical committed block body.
    #[codec(index = 16)]
    KuraReplicaAdvert(#[skip_try_from] KuraReplicaAdvertV1),
    /// Standalone lane-local block proposal.
    #[codec(index = 19)]
    LaneBlockProposal(#[skip_try_from] super::consensus::LaneBlockProposalV1),
    /// Producer-authenticated executable payload for a standalone lane block.
    #[codec(index = 20)]
    LaneExecutablePayload(#[skip_try_from] crate::lane_consensus::LaneExecutablePayloadV1),
    /// Individual lane-committee vote authorizing the next lane-local view.
    #[codec(index = 21)]
    LaneBlockNewViewVote(#[skip_try_from] crate::lane_consensus::LaneBlockNewViewVoteV1),
    /// Aggregate lane-committee certificate authorizing the next lane-local view.
    #[codec(index = 22)]
    LaneBlockNewViewCertificate(
        #[skip_try_from] crate::lane_consensus::LaneBlockNewViewCertificateV1,
    ),
    /// Standalone lane-local block vote carrying a BLS signature.
    #[codec(index = 25)]
    LaneBlockVote(#[skip_try_from] crate::lane_consensus::LaneBlockVoteV1),
    /// Standalone lane-local block QC aggregating lane-validator BLS signatures.
    #[codec(index = 26)]
    LaneBlockQc(#[skip_try_from] super::consensus::LaneBlockQcV1),
    /// Complete Kura-backed lane certificate returned for exact proposal recovery.
    #[codec(index = 27)]
    LaneBlockCertificate(#[skip_try_from] Box<super::consensus::LaneBlockCertificateV1>),
    /// Exact authenticated request for a missing historical canonical body or autonomous payload.
    #[codec(index = 28)]
    LaneHistoricalRecoveryRequest(#[skip_try_from] Box<LaneHistoricalRecoveryRequestV1>),
    /// Bounded proof-carrying response to an outstanding historical recovery request.
    #[codec(index = 29)]
    LaneHistoricalRecoveryResponse(#[skip_try_from] Box<LaneHistoricalRecoveryResponseV1>),
    /// Explicitly versioned global Sumeragi v2 message.
    #[codec(index = 30)]
    V2(#[skip_try_from] ConsensusMessageV2),
}
impl BlockMessage {
    /// Whether this belongs to the independent lane-local consensus protocol.
    pub(crate) const fn is_lane_local(&self) -> bool {
        matches!(
            self,
            Self::LaneBlockProposal(_)
                | Self::LaneExecutablePayload(_)
                | Self::LaneBlockNewViewVote(_)
                | Self::LaneBlockNewViewCertificate(_)
                | Self::LaneBlockVote(_)
                | Self::LaneBlockQc(_)
                | Self::LaneBlockCertificate(_)
                | Self::LaneHistoricalRecoveryRequest(_)
                | Self::LaneHistoricalRecoveryResponse(_)
        )
    }
    /// Whether this is bounded live consensus traffic which does not enter
    /// either the global reducer or an autonomous lane reducer.
    pub(crate) const fn is_live_auxiliary(&self) -> bool {
        matches!(self, Self::KuraReplicaAdvert(_))
    }
    /// Validate a message before it enters a canonical live wire frame.
    pub(crate) fn ensure_live_outbound(&self) -> Result<(), ncore::Error> {
        match self {
            Self::V2(message) => message.validate_version().map_err(|error| {
                ncore::Error::Message(format!(
                    "refusing to emit non-canonical Sumeragi v2 message: {error}"
                ))
            }),
            Self::KuraReplicaAdvert(advert) => advert.verify_keeper_signature().map_err(|error| {
                ncore::Error::Message(format!(
                    "refusing to emit an invalid Kura replica advert: {error}"
                ))
            }),
            Self::LaneBlockProposal(_)
            | Self::LaneExecutablePayload(_)
            | Self::LaneBlockNewViewVote(_)
            | Self::LaneBlockNewViewCertificate(_)
            | Self::LaneBlockVote(_)
            | Self::LaneBlockQc(_)
            | Self::LaneBlockCertificate(_)
            | Self::LaneHistoricalRecoveryRequest(_)
            | Self::LaneHistoricalRecoveryResponse(_) => Ok(()),
        }
    }
    fn ensure_supported_wire_version(&self) -> Result<(), ncore::Error> {
        match self {
            Self::V2(message) => message.validate_version().map_err(|error| {
                ncore::Error::Message(format!("unsupported Sumeragi v2 message version: {error}"))
            }),
            _ => Ok(()),
        }
    }
    /// Return whether this message belongs to an admitted live protocol.
    ///
    /// Lane-local traffic remains independent from global v2 finality and is
    /// admitted by the lane adapter.
    #[must_use]
    pub fn is_authoritative_v2_ingress(&self) -> bool {
        match self {
            Self::V2(message) => message.validate_version().is_ok(),
            _ => true,
        }
    }
    /// Return whether asynchronous ingress must preserve this live message.
    #[must_use]
    pub fn requires_blocking_ingress(&self) -> bool {
        self.is_authoritative_v2_ingress()
    }
    /// Network priority for this consensus message.
    pub fn priority(&self) -> iroha_p2p::Priority {
        iroha_p2p::Priority::High
    }
}
impl<'a> ncore::DecodeFromSlice<'a> for BlockMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut cursor = bytes;
        let value: Self = Decode::decode(&mut cursor)?;
        value.ensure_supported_wire_version()?;
        let consumed = bytes.len().saturating_sub(cursor.len());
        Ok((value, consumed))
    }
}
/// Wire wrapper for consensus payloads.
///
/// Cached bytes always store a full Norito-framed [`BlockMessage`] so the payload remains
/// self-describing even when it is forwarded through other framed envelopes.
/// Decoding the payload alone does not create an authenticated consensus-ingress envelope:
///
/// ```compile_fail
/// use iroha_core::sumeragi::{SumeragiHandle, message::BlockMessageWire};
///
/// fn submit_senderless_frame(handle: &SumeragiHandle, decoded: BlockMessageWire) {
///     handle.try_incoming_block_message_from_owned(decoded.into_message());
/// }
/// ```
///
/// A network consumer must pair the decoded payload with its transport-authenticated peer
/// through one of the identity-requiring `SumeragiHandle` entry points.
#[derive(Debug, Clone)]
pub struct BlockMessageWire {
    message: Arc<BlockMessage>,
    encoded: Option<Arc<Vec<u8>>>,
}
impl BlockMessageWire {
    /// Wrap a consensus message without cached bytes.
    pub fn new(message: BlockMessage) -> Self {
        Self {
            message: Arc::new(message),
            encoded: None,
        }
    }
    /// Wrap an `Arc`-backed live message and cache its canonical full-frame bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-canonical v2 protocol version or an invalid
    /// authenticated auxiliary message.
    pub(crate) fn try_preencoded(message: Arc<BlockMessage>) -> Result<Self, ncore::Error> {
        let encoded = Arc::new(Self::try_encode_live_message(message.as_ref())?);
        Ok(Self {
            message,
            encoded: Some(encoded),
        })
    }
    /// Borrow the underlying consensus message.
    pub fn as_message(&self) -> &BlockMessage {
        self.message.as_ref()
    }
    /// Acquire a mutable reference, clearing cached encoded bytes.
    pub fn make_mut(&mut self) -> &mut BlockMessage {
        self.encoded = None;
        Arc::make_mut(&mut self.message)
    }
    /// Consume the wrapper and return the consensus message.
    pub fn into_message(self) -> BlockMessage {
        Arc::try_unwrap(self.message).unwrap_or_else(|message| (*message).clone())
    }
    /// Cached encoded length if available.
    pub fn encoded_len(&self) -> Option<usize> {
        self.encoded.as_ref().map(|bytes| bytes.len())
    }
    fn framed_prefix_len(bytes: &[u8]) -> Result<usize, ncore::Error> {
        const LEN_OFF: usize = 4 + 1 + 1 + 16 + 1;
        if bytes.len() < ncore::Header::SIZE {
            return Err(ncore::Error::LengthMismatch);
        }
        if bytes[..4] != ncore::MAGIC {
            return Err(ncore::Error::InvalidMagic);
        }
        if bytes.get(4) != Some(&ncore::VERSION_MAJOR) {
            return Err(ncore::Error::UnsupportedVersion {
                found: bytes[4],
                expected: ncore::VERSION_MAJOR,
            });
        }
        if bytes.get(5) != Some(&ncore::VERSION_MINOR) {
            return Err(ncore::Error::UnsupportedMinorVersion {
                found: bytes[5],
                supported: ncore::VERSION_MINOR,
            });
        }
        let schema = bytes.get(6..22).ok_or(ncore::Error::LengthMismatch)?;
        if schema != <BlockMessage as NoritoSerialize>::schema_hash().as_slice() {
            return Err(ncore::Error::SchemaMismatch);
        }
        let compression = *bytes.get(22).ok_or(ncore::Error::LengthMismatch)?;
        if compression != ncore::Compression::None as u8 {
            return Err(ncore::Error::unsupported_compression_with(
                compression,
                &[ncore::Compression::None],
            ));
        }
        let len_bytes = bytes
            .get(LEN_OFF..LEN_OFF + 8)
            .ok_or(ncore::Error::LengthMismatch)?;
        let mut length = [0u8; 8];
        length.copy_from_slice(len_bytes);
        let payload_len = usize::try_from(u64::from_le_bytes(length))
            .map_err(|_| ncore::Error::LengthMismatch)?;
        let align = ncore::archived_payload_align::<BlockMessage>();
        let padding = if align <= 1 {
            0
        } else {
            let rem = ncore::Header::SIZE % align;
            if rem == 0 { 0 } else { align - rem }
        };
        ncore::Header::SIZE
            .checked_add(padding)
            .and_then(|size| size.checked_add(payload_len))
            .filter(|size| *size <= bytes.len())
            .ok_or(ncore::Error::LengthMismatch)
    }
    pub(crate) fn try_encode_live_message(message: &BlockMessage) -> Result<Vec<u8>, ncore::Error> {
        message.ensure_live_outbound()?;
        ncore::to_bytes(message)
    }
}
impl AsRef<BlockMessage> for BlockMessageWire {
    fn as_ref(&self) -> &BlockMessage {
        self.message.as_ref()
    }
}
impl std::ops::Deref for BlockMessageWire {
    type Target = BlockMessage;
    fn deref(&self) -> &Self::Target {
        self.message.as_ref()
    }
}
impl From<BlockMessage> for BlockMessageWire {
    fn from(message: BlockMessage) -> Self {
        Self::new(message)
    }
}
impl NoritoSerialize for BlockMessageWire {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        self.message.ensure_live_outbound()?;
        if let Some(encoded) = self.encoded.as_ref() {
            writer.write_all(encoded)?;
            return Ok(());
        }
        let encoded = Self::try_encode_live_message(self.message.as_ref())?;
        writer.write_all(&encoded)?;
        Ok(())
    }
}
impl<'a> NoritoDeserialize<'a> for BlockMessageWire {
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("decode canonical Sumeragi block message")
    }
    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let view = ncore::from_bytes_view(bytes)?;
        let message = view.decode::<BlockMessage>()?;
        message.ensure_supported_wire_version()?;
        let encoded = Arc::new(bytes.to_vec());
        Ok(Self {
            message: Arc::new(message),
            encoded: Some(encoded),
        })
    }
}
impl<'a> ncore::DecodeFromSlice<'a> for BlockMessageWire {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let consumed = Self::framed_prefix_len(bytes)?;
        let framed = bytes.get(..consumed).ok_or(ncore::Error::LengthMismatch)?;
        let message = ncore::decode_from_bytes::<BlockMessage>(framed)?;
        message.ensure_supported_wire_version()?;
        let encoded = Arc::new(framed.to_vec());
        Ok((
            Self {
                message: Arc::new(message),
                encoded: Some(encoded),
            },
            consumed,
        ))
    }
}
/// Current clean-break layout for lane, Native, and canonical executed-block
/// historical recovery transport.
///
/// Version 1 encoded a mandatory lane certificate. Version 2 added Native
/// participant authority. Version 3 added certificate-free recovery of an
/// exact finality-authenticated canonical executed block. Version 4 is the
/// coordinated clean break which carries protocol-v4 finality artifacts;
/// older layouts are not accepted.
pub const LANE_HISTORICAL_RECOVERY_VERSION_V4: u16 = 4;
/// Exact canonical executed body needed by a durable-evidence repair owner.
///
/// Every field is copied from one locally verified durable finality artifact.
/// The execution commitment (and therefore `executed_block_wire_hash`) was
/// signed by the exact CommitQC named by `finality_artifact_hash`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub struct CanonicalExecutedBlockNeedV1 {
    /// Canonical global block height.
    pub height: u64,
    /// Exact canonical block-header hash retained by State and finality.
    pub block_hash: HashOf<BlockHeader>,
    /// Hash of the requester's locally durable verified finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Exact transition commitment authenticated by the CommitQC.
    pub execution_commitment: ExecutionCommitment,
    /// Exact non-zero canonical result-bearing `SignedBlockWire` length.
    pub executed_block_wire_len: u64,
    /// Exact canonical result-bearing `SignedBlockWire` hash.
    pub executed_block_wire_hash: Hash,
}
/// Exact durable dependency named by a historical lane recovery request.
///
/// Lane-owned variants bind the immutable lane certificate carried by
/// [`LaneHistoricalRecoveryRequestV1`]. Certificate-free variants instead
/// bind exact locally durable global finality and execution authority. The
/// additional hashes prevent a response for another canonical body or READY
/// payload from being correlated merely because it names the same height.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub enum LaneHistoricalRecoveryKindV1 {
    /// Rehydrate the result-bearing canonical block selected by global finality.
    #[codec(index = 1)]
    CanonicalBlock {
        /// Hash of the immutable V2 finality artifact expected by the requester.
        finality_artifact_hash: HashOf<V2FinalityArtifact>,
    },
    /// Rehydrate the producer-authenticated payload and origin READY sidecar.
    #[codec(index = 2)]
    AutonomousPayload {
        /// View-neutral digest of the exact executable payload.
        executable_payload_hash: Hash,
        /// Hash of the origin PrepareQC carrying the READY aggregate.
        prepare_qc_hash: HashOf<LaneBlockQcV1>,
        /// Hash of the matching CommitQC.
        commit_qc_hash: HashOf<LaneBlockQcV1>,
    },
    // Codec index 3 is intentionally retired. It was the unbounded Native-only
    // whole-block recovery corridor; current startup repair uses the generic
    // chunked canonical executed-block dependency below.
    /// Rehydrate one pruned canonical executed block needed by a durable
    /// evidence repair owner.
    #[codec(index = 4)]
    CanonicalExecutedBlock {
        /// Exact finality-authenticated body identity.
        need: Box<CanonicalExecutedBlockNeedV1>,
        /// Zero-based fixed-size body chunk requested in this round trip.
        chunk_index: u32,
    },
}
/// Versioned request for one exact historical lane recovery dependency.
///
/// The outer P2P envelope authenticates `requester`; ingress rejects any
/// mismatch. Lane-owned requests carry a complete certificate and exact signer
/// PoPs. Certificate-free canonical-body repair carries no lane certificate;
/// its kind binds exact global finality and execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct LaneHistoricalRecoveryRequestV1 {
    /// Current-only layout version.
    pub version: u16,
    /// Authenticated peer requesting the exact durable evidence.
    pub requester: PeerId,
    /// Complete historical lane certificate which owns this dependency.
    ///
    /// This is present exactly for `CanonicalBlock` and `AutonomousPayload`,
    /// and absent for both certificate-free canonical-body variants.
    pub certificate: Option<LaneBlockCertificateV1>,
    /// Exact historical PoPs for the union of Prepare/Commit QC signers.
    ///
    /// Certificate-free canonical-body recovery requires this map to be empty.
    pub signer_pops: BTreeMap<PublicKey, Vec<u8>>,
    /// Exact missing durable dependency.
    pub kind: LaneHistoricalRecoveryKindV1,
}
impl LaneHistoricalRecoveryRequestV1 {
    /// Immutable lane proposal which owns this request.
    #[must_use]
    pub const fn proposal(&self) -> Option<&LaneBlockProposalV1> {
        match &self.certificate {
            Some(certificate) => Some(&certificate.proposal),
            None => None,
        }
    }
    /// Exact global height whose durable dependency is requested.
    #[must_use]
    pub const fn source_height(&self) -> u64 {
        match (&self.certificate, &self.kind) {
            (Some(certificate), _) => certificate.proposal.descriptor.proposal_height,
            (None, LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { need, .. }) => {
                need.height
            }
            (None, _) => 0,
        }
    }
}
/// Proof-carrying payload returned for one outstanding historical request.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub enum LaneHistoricalRecoveryPayloadV1 {
    /// Result-bearing canonical block plus its complete frozen finality proof.
    #[codec(index = 1)]
    CanonicalBlock {
        /// Exact canonical signed block bytes.
        block: SignedBlock,
        /// Frozen context, CommitQC, ordered roster, and aligned PoPs.
        finality_artifact: V2FinalityArtifact,
    },
    /// Producer payload plus exact origin Prepare/Commit proof material.
    ///
    /// Signer PoPs live only in the hash-bound request, avoiding two
    /// independently mutable copies of the same historical authority.
    #[codec(index = 2)]
    AutonomousPayload {
        /// Exact producer-authenticated executable payload.
        payload: crate::lane_consensus::LaneExecutablePayloadV1,
        /// Origin PrepareQC carrying the READY aggregate.
        prepare_qc: LaneBlockQcV1,
        /// Matching origin CommitQC.
        commit_qc: LaneBlockQcV1,
    },
    /// One bounded chunk of an exact canonical result-bearing block wire.
    #[codec(index = 3)]
    CanonicalExecutedBlockChunk {
        /// Requester's exact local finality artifact, repeated for independent
        /// response validation before accepting any chunk bytes.
        finality_artifact: V2FinalityArtifact,
        /// Total canonical `SignedBlockWire` length.
        wire_len: u64,
        /// Zero-based chunk index answered by this response.
        chunk_index: u32,
        /// Exact number of chunks in the canonical wire.
        chunk_count: u32,
        /// Bounded contiguous canonical wire bytes.
        bytes: Vec<u8>,
    },
}
/// Versioned response to one exact outstanding historical recovery request.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct LaneHistoricalRecoveryResponseV1 {
    /// Current-only layout version.
    pub version: u16,
    /// Hash of the exact request retained by the receiver.
    pub request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
    /// Bounded independently verifiable recovery evidence.
    pub payload: LaneHistoricalRecoveryPayloadV1,
}
/// Current clean-break layout version for authenticated Kura replica adverts.
pub const KURA_REPLICA_ADVERT_VERSION_V1: u16 = 1;
/// Hard admission bound for one encoded Kura replica advert.
pub const MAX_KURA_REPLICA_ADVERT_WIRE_BYTES: usize = 16 * 1024;
const KURA_REPLICA_ADVERT_SIGNATURE_BYTES: usize = 96;
const KURA_REPLICA_ADVERT_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:kura-replica-advert:v1";
#[derive(Encode)]
struct KuraReplicaAdvertSignaturePreimageV1 {
    domain: Vec<u8>,
    version: u16,
    network_id: NetworkId,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_len: u64,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    keeper_index: u32,
    keeper: PeerId,
}
/// Authenticated durable-replica claim for one exact canonical Kura body.
///
/// Every identity needed for safe eviction is signed.  The advert is useful
/// only when Kura independently revalidates the exact retained finality
/// artifact and deterministically selects `keeper` from that artifact's
/// CommitQC signers.
#[derive(Debug, Clone, Decode, Encode, PartialEq, Eq)]
pub struct KuraReplicaAdvertV1 {
    /// Advert layout version; must equal [`KURA_REPLICA_ADVERT_VERSION_V1`].
    pub version: u16,
    /// Finalized genesis-derived network identity, preventing cross-network replay.
    pub network_id: NetworkId,
    /// Height of the advertised canonical block.
    pub height: u64,
    /// Hash of the advertised canonical block.
    pub block_hash: HashOf<BlockHeader>,
    /// Canonical framed executed-block wire length retained by the keeper.
    pub executed_block_wire_len: u64,
    /// Hash of those exact canonical framed executed-block bytes.
    pub executed_block_wire_hash: Hash,
    /// Typed identity of the exact cryptographically verified finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Keeper's exact index in the finality artifact's frozen roster.
    pub keeper_index: u32,
    /// Keeper identity; also the only valid direct transport origin.
    pub keeper: PeerId,
    /// Keeper signature over [`Self::signature_preimage`].
    pub signature: Vec<u8>,
}
impl KuraReplicaAdvertV1 {
    /// Return the domain-separated canonical signing bytes.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        KuraReplicaAdvertSignaturePreimageV1 {
            domain: KURA_REPLICA_ADVERT_SIGNATURE_DOMAIN_V1.to_vec(),
            version: self.version,
            network_id: self.network_id,
            height: self.height,
            block_hash: self.block_hash,
            executed_block_wire_len: self.executed_block_wire_len,
            executed_block_wire_hash: self.executed_block_wire_hash,
            finality_artifact_hash: self.finality_artifact_hash,
            keeper_index: self.keeper_index,
            keeper: self.keeper.clone(),
        }
        .encode()
    }
    /// Verify fixed bounds, the clean-break version, and the keeper signature.
    ///
    /// Kura must additionally authenticate the canonical block, finality,
    /// CommitQC signer membership, and deterministic keeper selection.
    pub(crate) fn verify_keeper_signature(&self) -> Result<(), String> {
        if self.version != KURA_REPLICA_ADVERT_VERSION_V1 {
            return Err(format!(
                "unsupported Kura replica advert version {}; expected {}",
                self.version, KURA_REPLICA_ADVERT_VERSION_V1
            ));
        }
        if self.height == 0
            || self.executed_block_wire_len == 0
            || self.executed_block_wire_len > MAX_EXECUTED_BLOCK_WIRE_BYTES
        {
            return Err("Kura replica advert has an invalid executed-wire bound".to_owned());
        }
        if self.keeper.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal
            || self.signature.len() != KURA_REPLICA_ADVERT_SIGNATURE_BYTES
            || self.encode().len() > MAX_KURA_REPLICA_ADVERT_WIRE_BYTES
        {
            return Err(
                "Kura replica advert must carry one exact BLS-normal keeper signature within its wire bound"
                    .to_owned(),
            );
        }
        let signature = Signature::try_from_bytes(&self.signature)
            .map_err(|error| format!("invalid Kura replica advert signature bytes: {error}"))?;
        signature
            .verify(self.keeper.public_key(), &self.signature_preimage())
            .map_err(|error| format!("invalid Kura replica advert keeper signature: {error}"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::consensus;
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        nexus::{DataSpaceId, LaneId},
    };
    use norito::{core as norito_core, decode_from_bytes};
    use std::sync::Arc;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("Sumeragi message fixture key generation should succeed")
    }
    fn checked_random_peer_id() -> PeerId {
        PeerId::from(checked_random_keypair().public_key().clone())
    }
    fn sample_lane_block_messages(
        seed: u8,
    ) -> (
        consensus::LaneBlockProposalV1,
        crate::lane_consensus::LaneBlockVoteV1,
        consensus::LaneBlockQcV1,
    ) {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("derive lane-block fixture key");
        let validator = PeerId::from(keypair.public_key().clone());
        let validator_set = vec![validator.clone()];
        let mut descriptor = consensus::LaneBlockDescriptorV1 {
            lane_id: LaneId::new(u32::from(seed % 11) + 1),
            dataspace_id: DataSpaceId::new(u64::from(seed % 13) + 1),
            lane_incarnation: Hash::new(format!("message-lane-incarnation-{seed}").as_bytes()),
            proposal_height: u64::from(seed).saturating_add(1),
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: u64::from(seed).saturating_add(1),
            lane_block_view: u64::from(seed % 3),
            subject_hash: Hash::prehashed([seed.wrapping_add(1); Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([seed.wrapping_add(2); Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([seed.wrapping_add(3); Hash::LENGTH]),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::prehashed(
                [seed.wrapping_add(4); Hash::LENGTH],
            )],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:lane:fixture".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = consensus::LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let body = proposal.vote_body(consensus::Phase::Prepare);
        let signature = Signature::try_new(keypair.private_key(), &body.signature_preimage())
            .expect("sign lane-block fixture vote");
        let vote = crate::lane_consensus::LaneBlockVoteV1 {
            body: body.clone(),
            payload_availability_vote: None,
            signer: validator,
            bls_signature: signature.payload().to_vec(),
        };
        let qc = consensus::LaneBlockQcV1 {
            body,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            signers_bitmap: vec![1],
            bls_aggregate_signature: vote.bls_signature.clone(),
            payload_availability_qc: None,
        };
        (proposal, vote, qc)
    }
    fn sample_v2_vrf_message() -> BlockMessage {
        use iroha_data_model::block::consensus_v2 as wire;
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::VrfCommit(wire::VrfCommit {
                epoch: 7,
                commitment: [0x71; 32],
                signer: 3,
                bls_sig: vec![0x72],
            }),
        ))
    }
    fn retagged_live_block_message_frame(tag: u32) -> Vec<u8> {
        let mut encoded = norito_core::to_bytes(&sample_v2_vrf_message())
            .expect("encode canonical Sumeragi v2 fixture");
        let align = norito_core::archived_payload_align::<BlockMessage>();
        let padding = if align <= 1 {
            0
        } else {
            let remainder = norito_core::Header::SIZE % align;
            if remainder == 0 { 0 } else { align - remainder }
        };
        let tag_offset = norito_core::Header::SIZE + padding;
        encoded[tag_offset..tag_offset + core::mem::size_of::<u32>()]
            .copy_from_slice(&tag.to_le_bytes());
        encoded
    }
    fn roundtrip_live_block_message_over_network_message(
        message: BlockMessage,
    ) -> crate::NetworkMessage {
        let wire = BlockMessageWire::try_preencoded(Arc::new(message))
            .expect("live block message must pre-encode canonically");
        let network = crate::NetworkMessage::SumeragiBlock(Arc::new(wire));
        let bytes = norito_core::to_bytes(&network).expect("encode live network message");
        decode_from_bytes(&bytes).expect("decode live network message")
    }
    #[test]
    fn retired_global_v1_block_discriminants_fail_decode() {
        for tag in [
            0_u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17, 18, 23, 24,
        ] {
            let frame = retagged_live_block_message_frame(tag);
            assert!(
                norito_core::decode_from_bytes::<BlockMessage>(&frame).is_err(),
                "retired global-v1 block discriminant {tag} must fail typed decode"
            );
            assert!(
                <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(&frame)
                    .is_err(),
                "retired global-v1 block discriminant {tag} must fail cached-wire decode"
            );
        }
    }
    #[test]
    fn block_message_priority_is_high_for_current_protocols() {
        let (lane_proposal, lane_vote, lane_qc) = sample_lane_block_messages(0xA7);
        let messages = vec![
            (
                "KuraReplicaAdvert",
                BlockMessage::KuraReplicaAdvert(signed_kura_replica_advert_fixture()),
            ),
            ("V2", sample_v2_vrf_message()),
            (
                "LaneBlockProposal",
                BlockMessage::LaneBlockProposal(lane_proposal),
            ),
            ("LaneBlockVote", BlockMessage::LaneBlockVote(lane_vote)),
            ("LaneBlockQc", BlockMessage::LaneBlockQc(lane_qc)),
        ];
        for (variant, message) in messages {
            assert_eq!(
                message.priority(),
                iroha_p2p::Priority::High,
                "{variant} priority changed"
            );
        }
    }
    #[test]
    fn block_message_wire_roundtrips_current_v2_payload() {
        let msg = sample_v2_vrf_message();
        let encoded = BlockMessageWire::try_encode_live_message(&msg)
            .expect("encode current Sumeragi v2 message");
        let wire = <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(&encoded)
            .expect("decode current Sumeragi v2 message")
            .0;
        assert!(encoded.starts_with(&norito_core::MAGIC));
        assert_eq!(wire.encoded_len(), Some(encoded.len()));
        assert!(matches!(wire.as_ref(), BlockMessage::V2(_)));
        assert_eq!(wire.encode(), encoded);
    }
    #[test]
    fn noncanonical_v2_protocol_version_is_not_live_encodable() {
        use iroha_data_model::block::consensus_v2::{
            ConsensusMessageV2Payload, PayloadChunk, PayloadManifest,
        };
        let mut message =
            ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: HashOf::<PayloadManifest>::from_untyped_unchecked(Hash::new(
                    b"wrong-version-manifest",
                )),
                index: 0,
                bytes: vec![1, 2, 3],
                sender: 0,
                signature: vec![4],
            }));
        message.protocol_version = message.protocol_version.saturating_sub(1);
        let message = Arc::new(BlockMessage::V2(message));
        assert!(BlockMessageWire::try_preencoded(Arc::clone(&message)).is_err());
        assert!(norito_core::to_bytes(&BlockMessageWire::new((*message).clone())).is_err());
        let raw = norito_core::to_bytes(message.as_ref())
            .expect("encode non-canonical version as an adversarial raw fixture");
        assert!(
            <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(&raw).is_err(),
            "a non-canonical v2 version must fail during decode"
        );
    }
    #[test]
    fn block_message_wire_into_message_clones_shared_arc() {
        let (proposal, _, _) = sample_lane_block_messages(0x43);
        let msg = Arc::new(BlockMessage::LaneBlockProposal(proposal));
        let wire = BlockMessageWire::try_preencoded(Arc::clone(&msg))
            .expect("lane-local fixture is live traffic");
        let message = wire.into_message();
        assert!(matches!(message, BlockMessage::LaneBlockProposal(_)));
        assert_eq!(Arc::strong_count(&msg), 1);
    }
    #[test]
    fn block_message_wire_decode_gate_is_strict_for_current_frames() {
        fn assert_current_v2(label: &str, message: &BlockMessage) {
            assert!(
                matches!(message, BlockMessage::V2(_)),
                "{label}: expected current v2 marker, got {message:?}"
            );
        }
        fn assert_rejects_frame(label: &str, bytes: Vec<u8>) {
            assert!(
                <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(&bytes)
                    .is_err(),
                "{label} frame should be rejected"
            );
        }
        const LEN_OFF: usize = 4 + 1 + 1 + 16 + 1;
        let wrapped = sample_v2_vrf_message();
        let wrapped_encoded =
            BlockMessageWire::try_encode_live_message(&wrapped).expect("encode current v2 marker");
        assert!(wrapped_encoded.starts_with(&norito_core::MAGIC));
        assert_eq!(wrapped_encoded[4], norito_core::VERSION_MAJOR);
        assert_eq!(wrapped_encoded[5], norito_core::VERSION_MINOR);
        assert_eq!(
            &wrapped_encoded[6..22],
            <BlockMessage as NoritoSerialize>::schema_hash().as_slice()
        );
        assert_eq!(wrapped_encoded[22], norito_core::Compression::None as u8);
        assert!(LEN_OFF + 8 <= norito_core::Header::SIZE);
        let mut framed_with_trailing = wrapped_encoded.clone();
        framed_with_trailing.extend_from_slice(&[0xAA; 7]);
        let (decoded, consumed) =
            <BlockMessageWire as norito_core::DecodeFromSlice>::decode_from_slice(
                &framed_with_trailing,
            )
            .expect("decode framed block message prefix");
        assert_eq!(
            consumed,
            wrapped_encoded.len(),
            "decode_from_slice must consume exactly the framed prefix"
        );
        assert!(
            consumed < framed_with_trailing.len(),
            "trailing envelope bytes must remain unconsumed"
        );
        assert_current_v2("decode_from_slice message", decoded.as_message());
        assert_eq!(decoded.encoded_len(), Some(wrapped_encoded.len()));
        assert!(
            norito_core::to_bytes(&decoded).is_ok(),
            "a decoded current frame must remain canonically encodable"
        );
        let decoded_via_decode: BlockMessageWire =
            Decode::decode(&mut wrapped_encoded.as_slice()).expect("decode block message wire");
        assert_current_v2("Decode::decode message", decoded_via_decode.as_message());
        assert_eq!(
            decoded_via_decode.encoded_len(),
            Some(wrapped_encoded.len())
        );
        assert!(norito_core::to_bytes(&decoded_via_decode).is_ok());
        let decoded_payload =
            decode_from_bytes::<BlockMessage>(&wrapped_encoded).expect("decode cached payload");
        assert_current_v2("cached payload", &decoded_payload);
        let mut bad_magic = wrapped_encoded.clone();
        bad_magic[0] ^= 0xFF;
        assert_rejects_frame("bad magic", bad_magic);
        let mut bad_major = wrapped_encoded.clone();
        bad_major[4] = bad_major[4].wrapping_add(1);
        assert_rejects_frame("bad major version", bad_major);
        let mut bad_minor = wrapped_encoded.clone();
        bad_minor[5] = bad_minor[5].wrapping_add(1);
        assert_rejects_frame("bad minor version", bad_minor);
        let mut bad_schema = wrapped_encoded.clone();
        bad_schema[6] ^= 0x01;
        assert_rejects_frame("bad schema hash", bad_schema);
        let mut compressed = wrapped_encoded.clone();
        compressed[22] = norito_core::Compression::Zstd as u8;
        assert_rejects_frame("compressed frame", compressed);
        let mut missing_len = wrapped_encoded.clone();
        missing_len.truncate(norito_core::Header::SIZE - 1);
        assert_rejects_frame("missing length", missing_len);
        let mut length_overflow = wrapped_encoded.clone();
        length_overflow[LEN_OFF..LEN_OFF + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_rejects_frame("length overflow", length_overflow);
        let mut payload_unavailable = wrapped_encoded.clone();
        let too_large_payload = u64::try_from(wrapped_encoded.len()).expect("test frame length");
        payload_unavailable[LEN_OFF..LEN_OFF + 8].copy_from_slice(&too_large_payload.to_le_bytes());
        assert_rejects_frame("payload unavailable", payload_unavailable);
    }
    #[test]
    fn live_block_message_cache_is_canonical_and_mutation_clears_it() {
        let (proposal, _, _) = sample_lane_block_messages(0x44);
        let message = BlockMessage::LaneBlockProposal(proposal);
        let canonical = BlockMessageWire::try_encode_live_message(&message)
            .expect("lane-local proposal is live traffic");
        let wire = BlockMessageWire::try_preencoded(Arc::new(message))
            .expect("live preencoder must bind canonical bytes to the message");
        assert_eq!(wire.encode(), canonical);
        let (alternate, _, _) = sample_lane_block_messages(0x45);
        let alternate = BlockMessage::LaneBlockProposal(alternate);
        let mut mutated = wire;
        *mutated.make_mut() = alternate;
        assert_eq!(mutated.encoded_len(), None);
        assert!(mutated.encode().starts_with(&norito_core::MAGIC));
    }
    #[test]
    fn block_message_wire_network_roundtrip_cached_lane_block_messages() {
        let (proposal, vote, qc) = sample_lane_block_messages(0x71);
        let certificate = consensus::LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: qc.clone(),
            commit_qc: qc.clone(),
        };
        let historical_signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::BlsNormal)
            .expect("derive historical request signer");
        let historical_request = LaneHistoricalRecoveryRequestV1 {
            version: LANE_HISTORICAL_RECOVERY_VERSION_V4,
            requester: certificate.commit_qc.validator_set[0].clone(),
            certificate: Some(certificate.clone()),
            signer_pops: BTreeMap::from([(
                historical_signer.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(historical_signer.private_key())
                    .expect("derive historical request signer PoP"),
            )]),
            kind: LaneHistoricalRecoveryKindV1::CanonicalBlock {
                finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"message historical recovery finality",
                )),
            },
        };
        let cases = vec![
            ("lane proposal", BlockMessage::LaneBlockProposal(proposal)),
            ("lane vote", BlockMessage::LaneBlockVote(vote)),
            ("lane QC", BlockMessage::LaneBlockQc(qc)),
            (
                "lane certificate",
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
            ),
            (
                "historical recovery request",
                BlockMessage::LaneHistoricalRecoveryRequest(Box::new(historical_request)),
            ),
        ];
        for (label, message) in cases {
            let framed = norito_core::to_bytes(&message).expect("encode raw lane-topic fixture");
            assert_eq!(
                crate::inbound_sumeragi_topic(&framed)
                    .expect("classify an actual encoded lane message"),
                iroha_p2p::network::message::Topic::Consensus,
                "{label} must reach the reliable raw inbound corridor"
            );
            let decoded = roundtrip_live_block_message_over_network_message(message);
            match decoded {
                crate::NetworkMessage::SumeragiBlock(wire) => {
                    let matches_variant = match (label, wire.as_ref().as_message()) {
                        ("lane proposal", BlockMessage::LaneBlockProposal(_))
                        | ("lane vote", BlockMessage::LaneBlockVote(_))
                        | ("lane QC", BlockMessage::LaneBlockQc(_))
                        | ("lane certificate", BlockMessage::LaneBlockCertificate(_))
                        | (
                            "historical recovery request",
                            BlockMessage::LaneHistoricalRecoveryRequest(_),
                        ) => true,
                        _ => false,
                    };
                    assert!(matches_variant, "{label} roundtrip changed variant");
                    assert!(
                        wire.as_ref()
                            .encoded_len()
                            .is_some_and(|len| len >= norito_core::Header::SIZE)
                    );
                }
                other => panic!("expected cached sumeragi block message, got {other:?}"),
            }
        }
    }
    fn signed_kura_replica_advert_fixture() -> KuraReplicaAdvertV1 {
        let key = KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::BlsNormal)
            .expect("derive Kura replica advert signer");
        let mut advert = KuraReplicaAdvertV1 {
            version: KURA_REPLICA_ADVERT_VERSION_V1,
            network_id: crate::sumeragi::synthetic_network_id("kura-replica-advert-test"),
            height: 17,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-block")),
            executed_block_wire_len: 4096,
            executed_block_wire_hash: Hash::new(b"replica-executed-wire"),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(b"replica-finality")),
            keeper_index: 2,
            keeper: PeerId::new(key.public_key().clone()),
            signature: Vec::new(),
        };
        advert.signature = Signature::new(key.private_key(), &advert.signature_preimage())
            .payload()
            .to_vec();
        advert
    }
    #[test]
    fn kura_replica_advert_signature_binds_every_eviction_identity() {
        let advert = signed_kura_replica_advert_fixture();
        advert
            .verify_keeper_signature()
            .expect("exact signed advert is valid");
        let mut mutations = Vec::new();
        let mut wrong_network = advert.clone();
        wrong_network.network_id = crate::sumeragi::synthetic_network_id("other-network");
        mutations.push(wrong_network);
        let mut wrong_height = advert.clone();
        wrong_height.height += 1;
        mutations.push(wrong_height);
        let mut wrong_block = advert.clone();
        wrong_block.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"other-replica-block"));
        mutations.push(wrong_block);
        let mut wrong_wire_len = advert.clone();
        wrong_wire_len.executed_block_wire_len += 1;
        mutations.push(wrong_wire_len);
        let mut wrong_wire_hash = advert.clone();
        wrong_wire_hash.executed_block_wire_hash = Hash::new(b"other-executed-wire");
        mutations.push(wrong_wire_hash);
        let mut wrong_finality = advert.clone();
        wrong_finality.finality_artifact_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"other-finality"));
        mutations.push(wrong_finality);
        let mut wrong_index = advert.clone();
        wrong_index.keeper_index += 1;
        mutations.push(wrong_index);
        let mut wrong_keeper = advert.clone();
        wrong_keeper.keeper = checked_random_peer_id();
        mutations.push(wrong_keeper);
        for mutation in mutations {
            assert!(
                mutation.verify_keeper_signature().is_err(),
                "any changed eviction identity must invalidate the keeper signature"
            );
        }
    }
    #[test]
    fn kura_replica_advert_is_live_auxiliary_not_lane_or_global_v2() {
        let advert = signed_kura_replica_advert_fixture();
        let message = BlockMessage::KuraReplicaAdvert(advert.clone());
        assert!(message.is_live_auxiliary());
        assert!(!message.is_lane_local());
        assert!(message.is_authoritative_v2_ingress());
        assert!(message.requires_blocking_ingress());
        message
            .ensure_live_outbound()
            .expect("authenticated replica advert is an admitted live auxiliary type");
        let mut invalid = advert;
        invalid.executed_block_wire_len += 1;
        assert!(
            BlockMessage::KuraReplicaAdvert(invalid)
                .ensure_live_outbound()
                .is_err(),
            "live outbound admission must reject a forged replica advert"
        );
    }
    #[cfg(feature = "bls")]
    #[test]
    fn bls_aggregate_disabled_with_mixed_backends() {}
}
